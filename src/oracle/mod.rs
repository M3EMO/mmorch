//! Oracle: corre los tests en un contenedor sin red y cuenta el resultado desde junit.

pub mod configs;
pub mod contenedor;
pub mod descarga;
pub mod entrega;
pub mod evidencia;
pub mod juez;
pub mod junit;
pub mod mapa;
pub mod mutacion;
pub mod pendientes;
pub mod perfiles;
pub mod requisitos;
pub mod rubrica;
pub mod tarea;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

use thiserror::Error as ErrorDerive;

use crate::config::root_home;
use crate::ficha::{self, ErrorFicha};
use crate::tipos::{Conteo, Estado, Hallazgo, TasaTest, VERSION, Veredicto};
use crate::track::stats::wilson_inferior;
use junit::{Junit, ResultadoTest};
use tarea::{Comando, Tarea};

/// Carpetas de la raíz del repo que no se copian: artefactos del builder e historial de git.
const EXCLUIDOS: &[&str] = &["target", ".git"];
/// Nombre del junit del contrato dentro de `/salida`.
const XML_CONTRATO: &str = "contrato.xml";
/// Nombre del junit del oculto dentro de `/salida`.
const XML_OCULTO: &str = "oculto.xml";
/// Carpeta de temporales dentro de `ROOT_HOME`.
const CARPETA_TEMPORALES: &str = "tmp";
/// Prefijo de las carpetas temporales de `evaluar`.
const PREFIJO_TEMPORAL: &str = "evaluar-";
/// Antigüedad a partir de la cual se borra un temporal.
const CADUCIDAD: Duration = Duration::from_secs(24 * 3600);
/// Carpeta de cada suite dentro del contenedor. Es el último argumento de su `cmd`.
const DIR_CONTRATO: &str = "/oraculo/contrato";
const DIR_OCULTO: &str = "/oraculo/oculto";

/// Errores del Oracle. Salen con código 3, salvo `Ficha` (2). Ante uno de ellos no hay veredicto.
#[derive(Debug, ErrorDerive)]
pub enum ErrorOraculo {
    /// Infraestructura: podman ausente, timeout, error de podman o E/S.
    #[error("error de infraestructura del oráculo: {0}")]
    Infra(String),
    /// El junit falta o está mal formado.
    #[error("junit inválido: {0}")]
    Junit(String),
    /// La tarea no existe, no se lee o su `tarea.toml` es inválido.
    #[error("tarea inválida: {0}")]
    Tarea(String),
    /// Elección de juez inválida: sin razonamiento, sin vía API o sin modelo. Es configuración.
    #[error("elección inválida: {0}")]
    Eleccion(String),
    /// El spec de la tarea no tiene una ficha válida. Sin ficha no se evalúa.
    #[error(transparent)]
    Ficha(#[from] ErrorFicha),
}

impl ErrorOraculo {
    /// Código de salida: 2 si la ficha o la elección del juez son inválidas, 3 en los demás casos.
    pub fn codigo_de_salida(&self) -> u8 {
        match self {
            ErrorOraculo::Ficha(e) => e.codigo_de_salida(),
            ErrorOraculo::Eleccion(_) => 2,
            _ => 3,
        }
    }
}

/// Opciones de una evaluación.
#[derive(Debug, Clone)]
pub struct OpcionesOraculo {
    /// Límite de cada corrida de tests dentro del contenedor.
    pub timeout: Duration,
    /// `ROOT_HOME` donde viven los temporales. Sin valor, usa `root_home()`.
    pub home: Option<PathBuf>,
}

impl Default for OpcionesOraculo {
    fn default() -> Self {
        OpcionesOraculo {
            timeout: Duration::from_secs(3600),
            home: None,
        }
    }
}

/// Lo que cada suite necesita para correr: la copia del repo, la salida, las opciones
/// y las cachés que la fase de descarga dejó listas.
struct Corrida<'a> {
    repo: &'a Path,
    tarea: &'a Tarea,
    salida: &'a Path,
    o: &'a OpcionesOraculo,
    cache: Option<PathBuf>,
    rustsec: Option<PathBuf>,
}

/// Evalúa `repo` con la tarea: corre el contrato y el oculto sobre una copia del repo.
pub fn evaluar(repo: &Path, tarea: &Tarea, o: &OpcionesOraculo) -> Result<Veredicto, ErrorOraculo> {
    verificar_ficha(tarea)?;
    let repo = fs::canonicalize(repo)
        .map_err(|e| ErrorOraculo::Infra(format!("no existe el repo {}: {e}", repo.display())))?;
    // Los temporales van a ROOT_HOME, nunca a TMPDIR ni a /tmp (desempate 6).
    let home = o.home.clone().unwrap_or_else(root_home);
    let (colab, avisos) = pendientes::colab_para_veredicto(&home, &tarea.nombre)?;
    let tmp = home.join(CARPETA_TEMPORALES);
    fs::create_dir_all(&tmp).map_err(|e| infra("directorio temporal de ROOT_HOME", e))?;
    limpiar_temporales(&tmp, SystemTime::now());
    // La copia aísla el repo: un artefacto del builder no altera el original.
    let copia = temporal(&tmp).map_err(|e| infra("directorio temporal", e))?;
    let repo_copia = copia.path().join("repo");
    copiar_repo(&repo, &repo_copia).map_err(|e| infra("copia del repo", e))?;
    // La descarga va antes de cualquier suite: es la única fase con red (decisión 17).
    let (cache, rustsec, hallazgos) =
        match descarga::preparar(&repo_copia, tarea, &home, o.timeout)? {
            descarga::Preparacion::Lista {
                cache,
                rustsec,
                hallazgos,
            } => (cache, rustsec, hallazgos),
            descarga::Preparacion::SinVeredicto(hallazgos) => {
                return Ok(sin_veredicto(&repo, tarea, &colab, hallazgos, avisos));
            }
        };
    let salida = temporal(&tmp).map_err(|e| infra("directorio de salida", e))?;
    // Podman rootless: si el contenedor corre con otro uid, la salida debe ser escribible por todos.
    fs::set_permissions(salida.path(), fs::Permissions::from_mode(0o777))
        .map_err(|e| infra("permisos de la salida", e))?;

    let corrida = Corrida {
        repo: &repo_copia,
        tarea,
        salida: salida.path(),
        o,
        cache,
        rustsec,
    };
    // El contrato no tiene exclusiones: solo el oculto las usa.
    let contrato = evaluar_suite(
        &corrida,
        &tarea.contrato,
        DIR_CONTRATO,
        XML_CONTRATO,
        &BTreeSet::new(),
    )?;
    let excluidos: BTreeSet<String> = tarea
        .entorno
        .iter()
        .chain(&tarea.oraculo_dudoso)
        .map(|e| e.test.clone())
        .collect();
    let oculto = evaluar_suite(&corrida, &tarea.oculto, DIR_OCULTO, XML_OCULTO, &excluidos)?;
    let estado = estado_de(&contrato.conteo, &oculto.conteo);
    let mut tasas = BTreeMap::new();
    for (id, tasa) in contrato.tasas {
        tasas.insert(format!("contrato:{id}"), tasa);
    }
    for (id, tasa) in oculto.tasas {
        tasas.insert(format!("oculto:{id}"), tasa);
    }
    Ok(Veredicto {
        v: VERSION,
        tarea: tarea.nombre.clone(),
        repo: nombre_de(&repo),
        commit: commit_de(&repo),
        contrato: contrato.conteo,
        oculto: oculto.conteo,
        puntajes: Vec::new(),
        hallazgos,
        global: None,
        estado,
        costo_usd: 0.0,
        informe: None,
        rubricas: Vec::new(),
        repeticiones: tarea.repeticiones,
        inestables: oculto.inestables,
        inestables_contrato: contrato.inestables,
        tasas,
        pendientes: pendientes::desde(tarea, &colab),
        avisos,
    })
}

/// Carga la tarea `nombre` de `ROOT_HOME` y la evalúa con las opciones por defecto.
/// La usan `r00t oraculo evaluar` y la herramienta MCP `root_evaluar`.
pub fn evaluar_por_nombre(nombre: &str, repo: &Path) -> Result<Veredicto, ErrorOraculo> {
    let tarea = Tarea::cargar(nombre)?;
    evaluar(repo, &tarea, &OpcionesOraculo::default())
}

/// Lee el spec de la tarea y exige una ficha válida. Va antes de copiar o descargar nada.
fn verificar_ficha(tarea: &Tarea) -> Result<(), ErrorOraculo> {
    let ruta = tarea.dir.join(&tarea.spec);
    let texto = fs::read_to_string(&ruta).map_err(|fuente| ErrorFicha::Leer {
        ruta: ruta.display().to_string(),
        fuente,
    })?;
    ficha::extraer(&texto)?;
    Ok(())
}

/// Veredicto de una evaluación que no corrió: sin conteos, con sus hallazgos y pendientes.
fn sin_veredicto(
    repo: &Path,
    tarea: &Tarea,
    colab: &BTreeMap<String, pendientes::ColabEntrada>,
    hallazgos: Vec<Hallazgo>,
    avisos: Vec<String>,
) -> Veredicto {
    Veredicto {
        tarea: tarea.nombre.clone(),
        repo: nombre_de(repo),
        commit: commit_de(repo),
        repeticiones: tarea.repeticiones,
        hallazgos,
        pendientes: pendientes::desde(tarea, colab),
        avisos,
        ..Veredicto::default()
    }
}

/// Carpeta temporal dentro de `tmp`, con el prefijo de `evaluar`.
fn temporal(tmp: &Path) -> io::Result<tempfile::TempDir> {
    tempfile::Builder::new()
        .prefix(PREFIJO_TEMPORAL)
        .tempdir_in(tmp)
}

/// Borra las carpetas `evaluar-*` de `tmp` con más de 24 horas. Los fallos se ignoran:
/// un temporal viejo que no se borra no impide evaluar.
fn limpiar_temporales(tmp: &Path, ahora: SystemTime) {
    let Ok(entradas) = fs::read_dir(tmp) else {
        return;
    };
    for entrada in entradas.flatten() {
        if !entrada
            .file_name()
            .to_string_lossy()
            .starts_with(PREFIJO_TEMPORAL)
        {
            continue;
        }
        let ruta = entrada.path();
        let Ok(meta) = fs::symlink_metadata(&ruta) else {
            continue;
        };
        if !meta.is_dir() {
            continue;
        }
        let Ok(modificado) = meta.modified() else {
            continue;
        };
        if ahora
            .duration_since(modificado)
            .is_ok_and(|edad| edad > CADUCIDAD)
        {
            let _ = fs::remove_dir_all(&ruta);
        }
    }
}

/// Corre una suite `repeticiones` veces. Después corre solo sus tests inestables,
/// hasta `repeticiones_inestable` corridas en total.
fn evaluar_suite(
    c: &Corrida,
    comando: &Comando,
    dir: &str,
    xml: &str,
    excluidos: &BTreeSet<String>,
) -> Result<Clasificacion, ErrorOraculo> {
    let mut completas = Vec::new();
    for _ in 0..c.tarea.repeticiones {
        completas.push(correr_una(c, &comando.cmd, xml)?);
    }
    let previa = clasificar(&completas, &[], excluidos);
    let mut extras = Vec::new();
    if !previa.inestables.is_empty() && c.tarea.repeticiones_inestable > c.tarea.repeticiones {
        let reintento = reintento_de(comando, dir, &previa.inestables)?;
        for _ in c.tarea.repeticiones..c.tarea.repeticiones_inestable {
            extras.push(correr_una(c, &reintento, xml)?);
        }
    }
    Ok(clasificar(&completas, &extras, excluidos))
}

/// Comando de las corridas extra: corre solo los tests `inestables` de la suite `dir`.
/// Usa `reintento` si existe; si no, reemplaza `dir` al final de `cmd`.
fn reintento_de(
    comando: &Comando,
    dir: &str,
    inestables: &[String],
) -> Result<String, ErrorOraculo> {
    let lista = inestables
        .iter()
        .map(|id| comillas(&node_id(dir, id)))
        .collect::<Vec<_>>()
        .join(" ");
    if let Some(plantilla) = &comando.reintento {
        return Ok(plantilla.replace("{tests}", &lista));
    }
    match comando.cmd.trim_end().strip_suffix(dir) {
        Some(prefijo) => Ok(format!("{prefijo}{lista}")),
        None => Err(ErrorOraculo::Tarea(format!(
            "el comando no termina en {dir} y no tiene reintento: agregalo en tarea.toml"
        ))),
    }
}

/// Node id de pytest para un test `archivo::nombre` de la suite `dir`.
fn node_id(dir: &str, id: &str) -> String {
    match id.split_once("::") {
        Some((archivo, nombre)) => format!("{dir}/{archivo}.py::{nombre}"),
        None => format!("{dir}/{id}"),
    }
}

/// Entrecomilla para `sh`.
fn comillas(texto: &str) -> String {
    format!("'{}'", texto.replace('\'', "'\\''"))
}

/// Resultado de clasificar los tests de una suite a través de sus corridas.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Clasificacion {
    /// Tests estables con su resultado (`Pasa` o `Falla`). Los omitidos no aparecen.
    estables: BTreeMap<String, ResultadoTest>,
    /// Ids ordenados de los tests inestables.
    inestables: Vec<String>,
    /// Tasa de cada test inestable, con la clave `archivo::nombre` (sin prefijo de suite).
    tasas: BTreeMap<String, TasaTest>,
    /// Conteo de la suite. Un inestable no pasa, pero sí cuenta en el total.
    conteo: Conteo,
}

/// Clasifica cada test con sus muestras. Cada corrida completa aporta una muestra por test presente.
/// Un test ausente en una corrida completa es inestable. Las corridas extra solo suman muestras.
/// Un test es inestable si tiene resultados mezclados (Pasa con Falla, o Omite con otro).
/// Los excluidos no entran al cálculo.
fn clasificar(
    completas: &[Junit],
    extras: &[Junit],
    excluidos: &BTreeSet<String>,
) -> Clasificacion {
    let ids: BTreeSet<&str> = completas
        .iter()
        .flat_map(|j| j.tests.keys().map(String::as_str))
        .collect();
    let mut c = Clasificacion::default();
    for id in ids {
        if excluidos.contains(id) {
            continue;
        }
        let mut muestras: Vec<ResultadoTest> = Vec::new();
        let mut ausente = false;
        for j in completas {
            match j.tests.get(id) {
                Some(r) => muestras.push(*r),
                None => ausente = true,
            }
        }
        muestras.extend(extras.iter().filter_map(|j| j.tests.get(id).copied()));
        let mezcla = muestras
            .first()
            .is_some_and(|primera| muestras.iter().any(|m| m != primera));
        if ausente || mezcla {
            let pasa = muestras
                .iter()
                .filter(|m| **m == ResultadoTest::Pasa)
                .count() as u32;
            let total = muestras.len() as u32;
            c.inestables.push(id.to_string());
            c.tasas.insert(
                id.to_string(),
                TasaTest {
                    pasa,
                    total,
                    wilson_inf: wilson_inferior(pasa, total),
                },
            );
        } else if let Some(r) = muestras.first().copied()
            && r != ResultadoTest::Omite
        {
            c.estables.insert(id.to_string(), r);
        }
    }
    let pasa = c
        .estables
        .values()
        .filter(|r| **r == ResultadoTest::Pasa)
        .count();
    c.conteo = Conteo {
        pasa: pasa as u32,
        total: (c.estables.len() + c.inestables.len()) as u32,
    };
    c
}

fn infra(contexto: &str, e: io::Error) -> ErrorOraculo {
    ErrorOraculo::Infra(format!("{contexto}: {e}"))
}

/// Borra el junit de la corrida anterior. Sin borrarlo, una corrida sin XML pasaría con datos viejos.
fn borrar_xml(salida: &Path, xml: &str) -> Result<(), ErrorOraculo> {
    match fs::remove_file(salida.join(xml)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(infra("borrar el junit de la corrida anterior", e)),
    }
}

/// Una corrida de la suite en un contenedor nuevo, con su junit.
fn correr_una(c: &Corrida, cmd: &str, xml: &str) -> Result<Junit, ErrorOraculo> {
    borrar_xml(c.salida, xml)?;
    correr_suite(c, cmd, xml)
}

/// Corre un comando de la tarea y lee su junit. Sin junit no hay conteo: es error de infraestructura.
fn correr_suite(c: &Corrida, cmd: &str, xml: &str) -> Result<Junit, ErrorOraculo> {
    // Con caché de Rust, la suite arranca cargo contra los vendors sin red (decisión 17).
    let cmd = match c.cache {
        Some(_) => format!("{}{cmd}", descarga::PRELUDIO),
        None => cmd.to_string(),
    };
    let ejecucion = contenedor::Ejecucion {
        imagen: c.tarea.imagen.clone(),
        repo: Some(c.repo.to_path_buf()),
        oraculo: Some(c.tarea.dir.clone()),
        salida: c.salida.to_path_buf(),
        cmd,
        env: Vec::new(),
        timeout: c.o.timeout,
        red: false,
        cache: c.cache.clone(),
        cache_rw: false,
        rustsec: c.rustsec.clone(),
    };
    let resultado = contenedor::ejecutar(&ejecucion)?;
    let ruta = c.salida.join(xml);
    if !ruta.is_file() {
        return Err(ErrorOraculo::Infra(format!(
            "el comando no produjo /salida/{xml} (código {}): {}",
            resultado.codigo,
            resultado.stderr.trim()
        )));
    }
    junit::leer(&ruta)
}

/// Copia `origen` a `destino` sin `target/` ni `.git/` de la raíz y sin enlaces simbólicos.
fn copiar_repo(origen: &Path, destino: &Path) -> io::Result<()> {
    fs::create_dir_all(destino)?;
    copiar_dir(origen, destino, true)
}

fn copiar_dir(origen: &Path, destino: &Path, raiz: bool) -> io::Result<()> {
    for entrada in fs::read_dir(origen)? {
        let entrada = entrada?;
        let tipo = entrada.file_type()?;
        let nombre = entrada.file_name();
        if tipo.is_symlink() {
            continue;
        }
        if raiz && EXCLUIDOS.iter().any(|excluido| nombre == *excluido) {
            continue;
        }
        let dest = destino.join(&nombre);
        if tipo.is_dir() {
            fs::create_dir(&dest)?;
            copiar_dir(&entrada.path(), &dest, false)?;
        } else if tipo.is_file() {
            fs::copy(entrada.path(), &dest)?;
        }
    }
    Ok(())
}

fn nombre_de(repo: &Path) -> String {
    repo.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Commit corto del repo original. Vacío si no es un repo git.
fn commit_de(repo: &Path) -> String {
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|salida| salida.status.success())
        .map(|salida| String::from_utf8_lossy(&salida.stdout).trim().to_string())
        .unwrap_or_default()
}

/// `Rojo` si algún conteo tiene fallas. Si ambos están completos, `SinVeredicto`.
/// Nunca da `Verde`: ese estado exige un `global` por encima del umbral, y todavía no hay
/// dimensiones calibradas que lo calculen.
fn estado_de(contrato: &Conteo, oculto: &Conteo) -> Estado {
    if incompleto(contrato) || incompleto(oculto) {
        Estado::Rojo
    } else {
        Estado::SinVeredicto
    }
}

fn incompleto(conteo: &Conteo) -> bool {
    conteo.pasa < conteo.total
}

/// Código de salida de un estado: 0 verde, 1 rojo, 3 sin veredicto.
pub fn codigo_de_estado(estado: Estado) -> u8 {
    match estado {
        Estado::Verde => 0,
        Estado::Rojo => 1,
        Estado::SinVeredicto => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn conteo(pasa: u32, total: u32) -> Conteo {
        Conteo { pasa, total }
    }

    fn listar(dir: &Path) -> Vec<PathBuf> {
        let mut salida = Vec::new();
        for entrada in fs::read_dir(dir).expect("lee dir") {
            let ruta = entrada.expect("entrada").path();
            if ruta.is_dir() {
                salida.extend(listar(&ruta));
            }
            salida.push(ruta);
        }
        salida.sort();
        salida
    }

    #[test]
    fn oraculo_copia_excluye_target_y_git_de_la_raiz() {
        let origen = tempfile::tempdir().expect("temp dir");
        let o = origen.path();
        fs::write(o.join("a.txt"), "uno").expect("escribe");
        fs::create_dir_all(o.join("target")).expect("dir");
        fs::write(o.join("target/x"), "build").expect("escribe");
        fs::create_dir_all(o.join(".git")).expect("dir");
        fs::write(o.join(".git/HEAD"), "ref").expect("escribe");
        fs::create_dir_all(o.join("sub/target")).expect("dir");
        fs::write(o.join("sub/target/y"), "anidado").expect("escribe");

        let destino = tempfile::tempdir().expect("temp dir");
        let d = destino.path().join("repo");
        copiar_repo(o, &d).expect("copia");

        assert_eq!(fs::read_to_string(d.join("a.txt")).expect("lee"), "uno");
        assert!(!d.join("target").exists(), "no copia target de la raíz");
        assert!(!d.join(".git").exists(), "no copia .git");
        assert!(d.join("sub/target/y").exists(), "solo excluye la raíz");
    }

    #[test]
    fn oraculo_copia_ignora_enlaces_simbolicos() {
        let afuera = tempfile::tempdir().expect("temp dir");
        fs::write(afuera.path().join("secreto"), "no copiar").expect("escribe");
        let origen = tempfile::tempdir().expect("temp dir");
        std::os::unix::fs::symlink(afuera.path().join("secreto"), origen.path().join("enlace"))
            .expect("symlink");
        fs::write(origen.path().join("real.txt"), "ok").expect("escribe");

        let destino = tempfile::tempdir().expect("temp dir");
        let d = destino.path().join("repo");
        copiar_repo(origen.path(), &d).expect("copia");

        assert!(!d.join("enlace").exists());
        assert!(d.join("real.txt").exists());
    }

    #[test]
    fn oraculo_copia_deja_el_origen_igual() {
        let origen = tempfile::tempdir().expect("temp dir");
        fs::create_dir_all(origen.path().join("src")).expect("dir");
        fs::write(origen.path().join("src/lib.rs"), "fn x() {}").expect("escribe");
        let antes = listar(origen.path());

        let destino = tempfile::tempdir().expect("temp dir");
        copiar_repo(origen.path(), &destino.path().join("repo")).expect("copia");

        assert_eq!(listar(origen.path()), antes);
    }

    #[test]
    fn oraculo_estado_rojo_si_algun_conteo_tiene_fallas() {
        assert_eq!(estado_de(&conteo(2, 2), &conteo(1, 2)), Estado::Rojo);
        assert_eq!(estado_de(&conteo(1, 2), &conteo(2, 2)), Estado::Rojo);
    }

    #[test]
    fn oraculo_estado_sin_veredicto_si_todo_pasa() {
        assert_eq!(
            estado_de(&conteo(2, 2), &conteo(2, 2)),
            Estado::SinVeredicto
        );
    }

    fn junit_de(filas: &[(&str, ResultadoTest)]) -> Junit {
        let mut j = Junit::default();
        for (clave, resultado) in filas {
            j.tests.insert((*clave).to_string(), *resultado);
        }
        j
    }

    fn sin_exclusiones() -> BTreeSet<String> {
        BTreeSet::new()
    }

    #[test]
    fn repeticiones_todo_pasa_en_todas_cuenta_como_estable() {
        let j = junit_de(&[("a::t1", ResultadoTest::Pasa)]);
        let c = clasificar(&[j.clone(), j.clone(), j], &[], &sin_exclusiones());
        assert_eq!(c.conteo, Conteo { pasa: 1, total: 1 });
        assert!(c.inestables.is_empty());
        assert!(c.tasas.is_empty());
    }

    #[test]
    fn repeticiones_falla_en_todas_es_estable_que_falla() {
        let j = junit_de(&[("a::t1", ResultadoTest::Falla)]);
        let c = clasificar(&[j.clone(), j.clone(), j], &[], &sin_exclusiones());
        assert_eq!(c.conteo, Conteo { pasa: 0, total: 1 });
        assert_eq!(c.estables["a::t1"], ResultadoTest::Falla);
    }

    #[test]
    fn inestable_resultados_mezclados_no_pasan() {
        let r1 = junit_de(&[
            ("a::t_flaky", ResultadoTest::Falla),
            ("a::t_ok", ResultadoTest::Pasa),
        ]);
        let r2 = junit_de(&[
            ("a::t_flaky", ResultadoTest::Pasa),
            ("a::t_ok", ResultadoTest::Pasa),
        ]);
        let r3 = junit_de(&[
            ("a::t_flaky", ResultadoTest::Falla),
            ("a::t_ok", ResultadoTest::Pasa),
        ]);
        let c = clasificar(&[r1, r2, r3], &[], &sin_exclusiones());
        assert_eq!(c.inestables, vec!["a::t_flaky".to_string()]);
        assert_eq!(c.conteo, Conteo { pasa: 1, total: 2 });
    }

    #[test]
    fn repeticiones_omitido_en_todas_no_cuenta() {
        let j = junit_de(&[("a::t1", ResultadoTest::Omite)]);
        let c = clasificar(&[j.clone(), j], &[], &sin_exclusiones());
        assert_eq!(c.conteo, Conteo { pasa: 0, total: 0 });
        assert!(c.inestables.is_empty());
    }

    #[test]
    fn repeticiones_test_ausente_en_una_repeticion_es_inestable() {
        let con = junit_de(&[("a::t1", ResultadoTest::Pasa)]);
        let sin = junit_de(&[]);
        let c = clasificar(&[con, sin], &[], &sin_exclusiones());
        assert_eq!(c.inestables, vec!["a::t1".to_string()]);
    }

    #[test]
    fn inestable_ausente_en_corrida_extra_no_suma_muestra() {
        let con = junit_de(&[("a::t1", ResultadoTest::Pasa)]);
        let sin = junit_de(&[]);
        let c = clasificar(&[con.clone(), con], &[sin], &sin_exclusiones());
        assert!(c.inestables.is_empty());
        assert_eq!(c.conteo, Conteo { pasa: 1, total: 1 });
        assert!(c.tasas.is_empty());
    }

    #[test]
    fn inestable_corridas_extra_suman_muestras_y_tasa_wilson() {
        let f = junit_de(&[("a::t_flaky", ResultadoTest::Falla)]);
        let p = junit_de(&[("a::t_flaky", ResultadoTest::Pasa)]);
        let c = clasificar(
            &[f, p.clone()],
            &[p.clone(), junit_de(&[("a::t_flaky", ResultadoTest::Falla)])],
            &sin_exclusiones(),
        );
        assert_eq!(c.inestables, vec!["a::t_flaky".to_string()]);
        let tasa = &c.tasas["a::t_flaky"];
        assert_eq!((tasa.pasa, tasa.total), (2, 4));
        assert!((tasa.wilson_inf - wilson_inferior(2, 4)).abs() < 1e-12);
    }

    #[test]
    fn inestable_omite_mezclado_con_pasa_es_inestable() {
        let p = junit_de(&[("a::t1", ResultadoTest::Pasa)]);
        let o = junit_de(&[("a::t1", ResultadoTest::Omite)]);
        let c = clasificar(&[p, o], &[], &sin_exclusiones());
        assert_eq!(c.inestables, vec!["a::t1".to_string()]);
        assert_eq!(c.tasas["a::t1"].total, 2);
    }

    #[test]
    fn inestable_test_estable_no_tiene_tasa() {
        let p = junit_de(&[("a::t1", ResultadoTest::Pasa)]);
        let c = clasificar(&[p.clone(), p], &[], &sin_exclusiones());
        assert!(c.tasas.is_empty());
    }

    #[test]
    fn inestable_reintento_derivado_reemplaza_la_cola() {
        let c = Comando {
            cmd: "x --junitxml=/salida/oculto.xml /oraculo/oculto".into(),
            reintento: None,
        };
        let r =
            reintento_de(&c, "/oraculo/oculto", &["test_f::t_flaky".to_string()]).expect("deriva");
        assert_eq!(
            r,
            "x --junitxml=/salida/oculto.xml '/oraculo/oculto/test_f.py::t_flaky'"
        );
    }

    #[test]
    fn inestable_reintento_con_plantilla_reemplaza_tests() {
        let c = Comando {
            cmd: "x".into(),
            reintento: Some("pytest -q {tests}".into()),
        };
        let ids = ["a::t1".to_string(), "b::t2".to_string()];
        let r = reintento_de(&c, "/oraculo/contrato", &ids).expect("plantilla");
        assert_eq!(
            r,
            "pytest -q '/oraculo/contrato/a.py::t1' '/oraculo/contrato/b.py::t2'"
        );
    }

    #[test]
    fn inestable_reintento_sin_cola_conocida_es_error_de_tarea() {
        let c = Comando {
            cmd: "pytest -q".into(),
            reintento: None,
        };
        assert!(matches!(
            reintento_de(&c, "/oraculo/oculto", &["a::t".to_string()]),
            Err(ErrorOraculo::Tarea(_))
        ));
    }

    #[test]
    fn inestable_reintento_escapa_comillas_simples() {
        let c = Comando {
            cmd: "x /oraculo/oculto".into(),
            reintento: None,
        };
        let r = reintento_de(&c, "/oraculo/oculto", &["a::it's".to_string()]).expect("deriva");
        assert_eq!(r, "x '/oraculo/oculto/a.py::it'\\''s'");
    }

    fn poner_mtime(ruta: &Path, cuando: SystemTime) {
        fs::File::open(ruta)
            .expect("abre la carpeta")
            .set_modified(cuando)
            .expect("fija la fecha");
    }

    #[test]
    fn temporales_limpieza_borra_viejas_y_conserva_recientes_y_ajenos() {
        let home = tempfile::tempdir().expect("temp dir");
        let tmp = home.path().join("tmp");
        let viejo = tmp.join("evaluar-viejo");
        let reciente = tmp.join("evaluar-reciente");
        let otra = tmp.join("otra-carpeta");
        let ajeno = tmp.join("ajeno.txt");
        fs::create_dir_all(&viejo).expect("dir");
        fs::write(viejo.join("resto"), "x").expect("escribe");
        fs::create_dir_all(&reciente).expect("dir");
        fs::create_dir_all(&otra).expect("dir");
        fs::write(&ajeno, "x").expect("escribe");
        let ahora = SystemTime::now();
        poner_mtime(&viejo, ahora - Duration::from_secs(25 * 3600));
        poner_mtime(&otra, ahora - Duration::from_secs(48 * 3600));

        limpiar_temporales(&tmp, ahora);

        assert!(!viejo.exists(), "la carpeta de más de 24 h se borra");
        assert!(reciente.exists(), "la carpeta reciente se conserva");
        assert!(otra.exists(), "sin el prefijo no se toca");
        assert!(ajeno.exists(), "un archivo ajeno no se toca");
    }

    #[test]
    fn temporales_limpieza_sin_carpeta_no_falla() {
        let home = tempfile::tempdir().expect("temp dir");
        limpiar_temporales(&home.path().join("no-existe"), SystemTime::now());
    }

    #[test]
    fn exclusion_entorno_no_cuenta_en_el_calculo() {
        let r1 = junit_de(&[
            ("a::t_flaky", ResultadoTest::Falla),
            ("a::t_ok", ResultadoTest::Pasa),
        ]);
        let r2 = junit_de(&[
            ("a::t_flaky", ResultadoTest::Pasa),
            ("a::t_ok", ResultadoTest::Pasa),
        ]);
        let excluidos = BTreeSet::from(["a::t_flaky".to_string()]);
        let c = clasificar(&[r1, r2], &[], &excluidos);
        assert!(c.inestables.is_empty());
        assert_eq!(c.conteo, Conteo { pasa: 1, total: 1 });
    }

    #[test]
    fn exclusion_de_test_inexistente_se_ignora() {
        let j = junit_de(&[("a::t1", ResultadoTest::Pasa)]);
        let excluidos = BTreeSet::from(["a::no_existe".to_string()]);
        let c = clasificar(&[j.clone(), j], &[], &excluidos);
        assert_eq!(c.conteo, Conteo { pasa: 1, total: 1 });
    }

    #[test]
    fn oraculo_codigo_de_estado_verde_rojo_y_sin_veredicto() {
        assert_eq!(codigo_de_estado(Estado::Verde), 0);
        assert_eq!(codigo_de_estado(Estado::Rojo), 1);
        assert_eq!(codigo_de_estado(Estado::SinVeredicto), 3);
    }
}
