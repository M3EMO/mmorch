//! Oracle: corre los tests en un contenedor sin red y cuenta el resultado desde junit.

pub mod colab;
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

use crate::borrado;
use crate::config::root_home;
use crate::ficha::{self, ErrorFicha};
use crate::tipos::{Conteo, Estado, Hallazgo, TasaTest, VERSION, Veredicto};
use crate::track::stats::wilson_inferior;
use contenedor::{Ejecucion, Resultado};
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
/// Prefijo de las carpetas de Colab dentro de `tmp`: el junit bajado de cada corrida (B18).
const PREFIJO_COLAB_TEMPORAL: &str = "colab-";
/// Lista de tests que pytest recolecta, dentro de `/salida`. Se compara con el junit (A5).
const ARCHIVO_COLECCION: &str = "coleccion.txt";
/// Carpeta de los registros de stderr de cada tarea, dentro de `oraculos/<tarea>/` (A2).
const CARPETA_REGISTROS: &str = "registros";
/// Carpeta de las tareas dentro de `ROOT_HOME`. El builder no puede leerla.
const CARPETA_ORACULOS: &str = "oraculos";
/// Antigüedad a partir de la cual se borra un temporal.
const CADUCIDAD: Duration = Duration::from_secs(24 * 3600);
/// Carpeta de cada suite dentro del contenedor. Es el último argumento de su `cmd`. Cada suite monta
/// solo su carpeta, nunca la tarea entera (A2).
const DIR_CONTRATO: &str = "/oraculo/contrato";
const DIR_OCULTO: &str = "/oraculo/oculto";

/// Ejecutor de un contenedor. En producción es `contenedor::ejecutar`; los tests le dan uno falso.
type Correr<'a> = &'a dyn Fn(&Ejecucion) -> Result<Resultado, ErrorOraculo>;

fn correr_en_podman(e: &Ejecucion) -> Result<Resultado, ErrorOraculo> {
    contenedor::ejecutar(e)
}

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
    /// Colab: sin sesión de la CLI, GPU que no es T4 o un runtime que falló. Sale con código 3.
    #[error("{0}")]
    Colab(String),
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
    /// `ROOT_HOME`: donde van los registros de stderr, dentro de `oraculos/<tarea>/`.
    home: &'a Path,
}

/// Resultado de una suite: su clasificación, o el motivo por el que no tiene veredicto (A5).
#[derive(Debug)]
enum Suite {
    Clasificada(Clasificacion),
    SinVeredicto(String),
}

/// Evalúa `repo` con la tarea: corre el contrato y el oculto sobre una copia del repo.
pub fn evaluar(repo: &Path, tarea: &Tarea, o: &OpcionesOraculo) -> Result<Veredicto, ErrorOraculo> {
    evaluar_con(repo, tarea, o, &correr_en_podman)
}

/// Como `evaluar`, con el ejecutor de contenedores que se le pase.
fn evaluar_con(
    repo: &Path,
    tarea: &Tarea,
    o: &OpcionesOraculo,
    correr: Correr,
) -> Result<Veredicto, ErrorOraculo> {
    verificar_ficha(tarea)?;
    let repo = fs::canonicalize(repo)
        .map_err(|e| ErrorOraculo::Infra(format!("no existe el repo {}: {e}", repo.display())))?;
    // Los temporales van a ROOT_HOME, nunca a TMPDIR ni a /tmp (desempate 6).
    let home = o.home.clone().unwrap_or_else(root_home);
    let (colab, mut avisos) = pendientes::colab_para_veredicto(&home, &tarea.nombre)?;
    let commit = commit_de(&repo);
    let (colab, aviso_commit) = filtrar_colab_por_commit(colab, &commit);
    avisos.extend(aviso_commit);
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
        home: &home,
    };
    // El contrato no tiene exclusiones: solo el oculto las usa.
    let contrato = match evaluar_suite(
        &corrida,
        &tarea.contrato,
        DIR_CONTRATO,
        XML_CONTRATO,
        &BTreeSet::new(),
        correr,
    )? {
        Suite::Clasificada(c) => c,
        Suite::SinVeredicto(motivo) => {
            avisos.push(format!("contrato: {motivo}"));
            return Ok(sin_veredicto(&repo, tarea, &colab, hallazgos, avisos));
        }
    };
    let excluidos: BTreeSet<String> = tarea
        .entorno
        .iter()
        .chain(&tarea.oraculo_dudoso)
        .map(|e| e.test.clone())
        .collect();
    let oculto = match evaluar_suite(
        &corrida,
        &tarea.oculto,
        DIR_OCULTO,
        XML_OCULTO,
        &excluidos,
        correr,
    )? {
        Suite::Clasificada(c) => c,
        Suite::SinVeredicto(motivo) => {
            avisos.push(format!("oculto: {motivo}"));
            return Ok(sin_veredicto(&repo, tarea, &colab, hallazgos, avisos));
        }
    };
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
        commit,
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

/// Resultados de Colab del commit del repo. Los de otro commit no entran al veredicto (B16). El aviso
/// cuenta cuántos quedaron afuera.
fn filtrar_colab_por_commit(
    colab: BTreeMap<String, pendientes::ColabEntrada>,
    commit: &str,
) -> (BTreeMap<String, pendientes::ColabEntrada>, Option<String>) {
    let total = colab.len();
    let del_commit: BTreeMap<String, pendientes::ColabEntrada> = colab
        .into_iter()
        .filter(|(_, c)| c.commit == commit)
        .collect();
    let aviso = (del_commit.len() < total).then(|| {
        format!(
            "{} resultados de Colab de otro commit no entran al veredicto",
            total - del_commit.len()
        )
    });
    (del_commit, aviso)
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

/// Borra las carpetas `evaluar-*` y `colab-*` de `tmp` con más de 24 horas (B18). Los fallos se
/// ignoran: un temporal viejo que no se borra no impide evaluar.
fn limpiar_temporales(tmp: &Path, ahora: SystemTime) {
    let Ok(entradas) = fs::read_dir(tmp) else {
        return;
    };
    for entrada in entradas.flatten() {
        let nombre = entrada.file_name().to_string_lossy().into_owned();
        if !nombre.starts_with(PREFIJO_TEMPORAL) && !nombre.starts_with(PREFIJO_COLAB_TEMPORAL) {
            continue;
        }
        let ruta = entrada.path();
        let Ok(meta) = fs::symlink_metadata(&ruta) else {
            continue;
        };
        let Ok(modificado) = meta.modified() else {
            continue;
        };
        if !ahora
            .duration_since(modificado)
            .is_ok_and(|edad| edad > CADUCIDAD)
        {
            continue;
        }
        // Un enlace se borra como enlace: su destino puede estar fuera de `tmp` y no se toca.
        let _ = if meta.is_symlink() {
            borrado::borrar_archivo_dentro(tmp, &ruta)
        } else if meta.is_dir() {
            borrado::borrar_dir_dentro(tmp, &ruta)
        } else {
            continue;
        };
    }
}

/// Corre una suite `repeticiones` veces. Después corre solo sus tests inestables,
/// hasta `repeticiones_inestable` corridas en total. Una suite de pytest lista antes lo que pytest
/// recolecta: su junit vale solo si trae exactamente eso y su código cuadra con las fallas (A5).
/// Las demás suites no tienen esa lista: su junit lo escribe el propio comando.
fn evaluar_suite(
    c: &Corrida,
    comando: &Comando,
    dir: &str,
    xml: &str,
    excluidos: &BTreeSet<String>,
    correr: Correr,
) -> Result<Suite, ErrorOraculo> {
    let pytest = es_pytest(&comando.cmd);
    let esperados = if pytest {
        match coleccion_de(c, &comando.cmd, dir, correr)? {
            Ok(nodos) => Some(claves_de_nodos(&nodos)),
            Err(motivo) => return Ok(Suite::SinVeredicto(motivo)),
        }
    } else {
        None
    };
    let mut completas = Vec::new();
    for _ in 0..c.tarea.repeticiones {
        match correr_una(
            c,
            &comando.cmd,
            dir,
            xml,
            pytest,
            esperados.as_ref(),
            correr,
        )? {
            Ok(junit) => completas.push(junit),
            Err(motivo) => return Ok(Suite::SinVeredicto(motivo)),
        }
    }
    let previa = clasificar(&completas, &[], excluidos);
    let mut extras = Vec::new();
    if !previa.inestables.is_empty() && c.tarea.repeticiones_inestable > c.tarea.repeticiones {
        let reintento = reintento_de(comando, dir, &previa.inestables)?;
        for _ in c.tarea.repeticiones..c.tarea.repeticiones_inestable {
            // El reintento corre solo los inestables: su junit no trae la lista completa.
            match correr_una(c, &reintento, dir, xml, pytest, None, correr)? {
                Ok(junit) => extras.push(junit),
                Err(motivo) => return Ok(Suite::SinVeredicto(motivo)),
            }
        }
    }
    Ok(Suite::Clasificada(clasificar(
        &completas, &extras, excluidos,
    )))
}

/// Si el comando corre pytest. Solo entonces hay lista de recolección y código que cotejar (A5).
fn es_pytest(cmd: &str) -> bool {
    cmd.contains("pytest")
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
    /// Tests que se omitieron en todas sus corridas. No cuentan en el conteo, pero sí en el universo de
    /// su requisito, como no verdes (A10).
    omitidos: BTreeSet<String>,
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
        } else if let Some(r) = muestras.first().copied() {
            if r == ResultadoTest::Omite {
                c.omitidos.insert(id.to_string());
            } else {
                c.estables.insert(id.to_string(), r);
            }
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
    borrado::borrar_archivo_dentro(salida, &salida.join(xml))
        .map_err(|e| infra("borrar el junit de la corrida anterior", e))
}

/// Una corrida de la suite en un contenedor nuevo. `Ok(Err(motivo))` si el junit no vale (A5).
/// `Err` solo si la infraestructura falla o si no hay junit (A2: ahí el stderr va a `registros/`).
/// Con `pytest`, el código de salida debe cuadrar con las fallas, y con `esperados` (si hay) el junit
/// trae exactamente esas claves.
fn correr_una(
    c: &Corrida,
    cmd: &str,
    dir: &str,
    xml: &str,
    pytest: bool,
    esperados: Option<&BTreeSet<String>>,
    correr: Correr,
) -> Result<Result<Junit, String>, ErrorOraculo> {
    borrar_xml(c.salida, xml)?;
    let resultado = correr(&ejecucion_de_suite(c, &cmd_con_cache(c, cmd), dir))?;
    let ruta = c.salida.join(xml);
    if !ruta.is_file() {
        return Err(sin_junit(c, xml, &resultado));
    }
    let Ok(crudo) = junit::leer(&ruta) else {
        return Ok(Err("el junit no se pudo leer".into()));
    };
    if pytest {
        let claves: BTreeSet<String> = crudo.tests.keys().cloned().collect();
        if let Some(esperados) = esperados
            && claves != *esperados
        {
            return Ok(Err(format!(
                "el junit trae {} tests y pytest recolectó {}",
                claves.len(),
                esperados.len()
            )));
        }
        let hay_falla = crudo.tests.values().any(|r| *r == ResultadoTest::Falla);
        let cuadra = match resultado.codigo {
            0 => !hay_falla,
            1 => hay_falla,
            _ => false,
        };
        if !cuadra {
            return Ok(Err(format!(
                "el código de pytest ({}) no coincide con las fallas del junit",
                resultado.codigo
            )));
        }
    }
    Ok(Ok(crudo.con_ids()))
}

/// Lista de node ids que pytest recolecta para la suite. `Ok(Err(motivo))` si pytest no recolectó
/// tests o salió con error. Sin lista en `/salida`, o con código 0 y sin nodos, es error de
/// infraestructura (A2): no es una corrida de pytest.
fn coleccion_de(
    c: &Corrida,
    cmd: &str,
    dir: &str,
    correr: Correr,
) -> Result<Result<Vec<String>, String>, ErrorOraculo> {
    let ruta = c.salida.join(ARCHIVO_COLECCION);
    borrado::borrar_archivo_dentro(c.salida, &ruta)
        .map_err(|e| infra("borrar la lista de tests de la corrida anterior", e))?;
    let orden = comando_de_coleccion(&cmd_con_cache(c, cmd));
    let resultado = correr(&ejecucion_de_suite(c, &orden, dir))?;
    if !ruta.is_file() {
        return Err(sin_junit(c, ARCHIVO_COLECCION, &resultado));
    }
    // Código 5 de pytest: ningún test recolectado. Otro código distinto de 0 no da veredicto.
    match resultado.codigo {
        0 => {}
        5 => return Ok(Err("pytest no recolectó tests".into())),
        otro => {
            return Ok(Err(format!(
                "pytest --collect-only salió con código {otro}"
            )));
        }
    }
    let texto = fs::read_to_string(&ruta).map_err(|e| infra("leer la lista de tests", e))?;
    let mut nodos: Vec<String> = Vec::new();
    for linea in texto.lines().map(str::trim) {
        if linea.contains(".py::") && !nodos.iter().any(|n| n == linea) {
            nodos.push(linea.to_string());
        }
    }
    if nodos.is_empty() {
        return Err(sin_junit(c, ARCHIVO_COLECCION, &resultado));
    }
    if nodos.iter().any(|n| junit::clave_de_nodo(n).is_none()) {
        return Ok(Err("la lista de tests recolectados no se pudo leer".into()));
    }
    Ok(Ok(nodos))
}

/// Claves de junit de los node ids recolectados.
fn claves_de_nodos(nodos: &[String]) -> BTreeSet<String> {
    nodos
        .iter()
        .filter_map(|n| junit::clave_de_nodo(n))
        .collect()
}

/// Comando que lista lo que pytest recolecta: el de la suite sin su junit, con `--collect-only -q`.
/// La lista va a `/salida/coleccion.txt`.
fn comando_de_coleccion(cmd: &str) -> String {
    let mut sin_junit = cmd.to_string();
    while let Some(inicio) = sin_junit.find("--junitxml=") {
        let fin = sin_junit[inicio..]
            .find(char::is_whitespace)
            .map_or(sin_junit.len(), |n| inicio + n);
        sin_junit.replace_range(inicio..fin, "");
    }
    format!(
        "{} --collect-only -q > /salida/{ARCHIVO_COLECCION}",
        sin_junit.trim_end()
    )
}

/// El comando con el preludio de la caché cuando la hay (Tarea 6c-B).
fn cmd_con_cache(c: &Corrida, cmd: &str) -> String {
    // Con caché, la suite arranca desde la caché sin red: cargo para Rust, pip para Python (Tarea 6c-B).
    match (c.cache.is_some(), c.tarea.lenguaje.as_str()) {
        (true, "rust") => format!("{}{cmd}", descarga::PRELUDIO),
        (true, "python") => format!("{}{cmd}", descarga::PRELUDIO_PYTHON),
        _ => cmd.to_string(),
    }
}

/// Lo que un contenedor de suite necesita: su carpeta (solo esa, no la tarea) y la salida.
/// `dir` es la carpeta dentro del contenedor, y es el último argumento del comando.
fn ejecucion_de_suite(c: &Corrida, cmd: &str, dir: &str) -> Ejecucion {
    let nombre = dir.rsplit('/').next().unwrap_or_default();
    Ejecucion {
        imagen: c.tarea.imagen.clone(),
        repo: Some(c.repo.to_path_buf()),
        // Sin carpeta de suite, nada se monta: el comando falla con su propio error (A2).
        oraculo: Some(c.tarea.dir.join(nombre)).filter(|ruta| ruta.is_dir()),
        oraculo_destino: Some(dir.to_string()),
        salida: c.salida.to_path_buf(),
        cmd: cmd.to_string(),
        env: Vec::new(),
        timeout: c.o.timeout,
        red: false,
        cache: c.cache.clone(),
        cache_rw: false,
        rustsec: c.rustsec.clone(),
        trabajo: None,
        secretos: Vec::new(),
    }
}

/// Error de una corrida sin junit. El stderr del contenedor va a `registros/` y el mensaje solo
/// nombra el archivo: el texto del stderr puede traer tests ocultos (A2).
fn sin_junit(c: &Corrida, xml: &str, resultado: &Resultado) -> ErrorOraculo {
    let donde = match registrar_stderr(c, xml, &resultado.stderr) {
        Some(ruta) => format!("el stderr del contenedor quedó en {}", ruta.display()),
        None => "no se pudo guardar el stderr del contenedor".into(),
    };
    ErrorOraculo::Infra(format!(
        "el comando no produjo /salida/{xml} (código {}); {donde}",
        resultado.codigo
    ))
}

/// Guarda el stderr de una corrida en `oraculos/<tarea>/registros/`. `None` si no se pudo escribir.
fn registrar_stderr(c: &Corrida, xml: &str, stderr: &str) -> Option<PathBuf> {
    let carpeta = c
        .home
        .join(CARPETA_ORACULOS)
        .join(&c.tarea.nombre)
        .join(CARPETA_REGISTROS);
    fs::create_dir_all(&carpeta).ok()?;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let base = xml.trim_end_matches(".xml");
    let ruta = carpeta.join(format!("{base}-{nanos}.stderr"));
    fs::write(&ruta, stderr).ok()?;
    Some(ruta)
}

/// Veredicto para la herramienta MCP: solo conteos y niveles, sin ids de tests ocultos (A2). La CLI
/// para el humano conserva todo. Campo por campo: `inestables`, `tasas` (`oculto:`), `pendientes` y las
/// evidencias de requisitos y de ítems de puntaje se vacían; un hallazgo que cita un test oculto se
/// quita; el resto (conteos, estado, `avisos`, hallazgos sin ids) se conserva.
pub fn redactar_para_mcp(mut v: Veredicto) -> Veredicto {
    let mut ocultos: BTreeSet<String> = v.inestables.iter().cloned().collect();
    ocultos.extend(
        v.tasas
            .keys()
            .filter_map(|k| k.strip_prefix("oculto:").map(String::from)),
    );
    ocultos.extend(v.pendientes.iter().map(|p| p.test.clone()));
    v.inestables.clear();
    v.tasas.retain(|k, _| !k.starts_with("oculto:"));
    v.pendientes.clear();
    v.hallazgos.retain(|h| {
        !ocultos
            .iter()
            .any(|id| h.cita.contains(id.as_str()) || h.detalle.contains(id.as_str()))
    });
    for puntaje in &mut v.puntajes {
        puntaje.items.clear();
    }
    if let Some(informe) = &mut v.informe {
        for requisito in &mut informe.requisitos {
            requisito.evidencia.clear();
        }
    }
    v
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
pub(crate) fn commit_de(repo: &Path) -> String {
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
    use filetime::{FileTime, set_symlink_file_times};
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
    fn temporales_enlace_viejo_se_borra_como_enlace_y_su_destino_queda() {
        let home = tempfile::tempdir().expect("temp dir");
        let tmp = home.path().join("tmp");
        fs::create_dir_all(&tmp).expect("dir");
        let fuera = tempfile::tempdir().expect("destino fuera de home");
        fs::write(fuera.path().join("dato"), "x").expect("escribe");
        let enlace = tmp.join("evaluar-viejo");
        std::os::unix::fs::symlink(fuera.path(), &enlace).expect("enlace");
        let ahora = SystemTime::now();
        let viejo = FileTime::from_system_time(ahora - Duration::from_secs(25 * 3600));
        set_symlink_file_times(&enlace, viejo, viejo).expect("fija el mtime del enlace");

        limpiar_temporales(&tmp, ahora);

        assert!(
            fs::symlink_metadata(&enlace).is_err(),
            "el enlace de más de 24 h se borra"
        );
        assert_eq!(
            fs::read_to_string(fuera.path().join("dato")).expect("destino"),
            "x",
            "el destino queda intacto"
        );
    }

    #[test]
    fn temporales_enlace_reciente_se_conserva() {
        let home = tempfile::tempdir().expect("temp dir");
        let tmp = home.path().join("tmp");
        fs::create_dir_all(&tmp).expect("dir");
        let fuera = tempfile::tempdir().expect("destino fuera de home");
        let enlace = tmp.join("evaluar-reciente");
        std::os::unix::fs::symlink(fuera.path(), &enlace).expect("enlace");

        limpiar_temporales(&tmp, SystemTime::now());

        assert!(
            fs::symlink_metadata(&enlace).is_ok(),
            "el enlace reciente se conserva"
        );
    }

    #[test]
    fn oraculo_borrar_xml_rechaza_ruta_fuera_de_la_salida() {
        let base = tempfile::tempdir().expect("temp dir");
        let salida = base.path().join("salida");
        fs::create_dir_all(&salida).expect("salida");
        fs::write(base.path().join("fuera.xml"), "x").expect("escribe");
        assert!(borrar_xml(&salida, "../fuera.xml").is_err());
        assert!(base.path().join("fuera.xml").exists());
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

    // --- Tarea 23: suites con ejecutor falso (sin podman) ---

    /// Tarea mínima con `dir` real (con `contrato/` y `oculto/`) bajo `base`.
    fn tarea_de_prueba(base: &Path) -> Tarea {
        fs::create_dir_all(base.join("contrato")).expect("contrato");
        fs::create_dir_all(base.join("oculto")).expect("oculto");
        Tarea {
            nombre: "tarea-prueba".into(),
            lenguaje: "python".into(),
            imagen: "x".into(),
            containerfile: None,
            spec: "s".into(),
            repeticiones: 1,
            repeticiones_inestable: 1,
            contrato: Comando {
                cmd: "python3 -m pytest -q --junitxml=/salida/contrato.xml /oraculo/contrato"
                    .into(),
                reintento: None,
            },
            oculto: Comando {
                cmd: "python3 -m pytest -q --junitxml=/salida/oculto.xml /oraculo/oculto".into(),
                reintento: None,
            },
            entorno: Vec::new(),
            oraculo_dudoso: Vec::new(),
            requisito: Vec::new(),
            arquitectura: None,
            eficiencia: None,
            pruebas: None,
            entrega: None,
            escenario: Vec::new(),
            dir: base.to_path_buf(),
        }
    }

    fn resultado_falso(codigo: i32, stderr: &str) -> contenedor::Resultado {
        contenedor::Resultado {
            codigo,
            stdout: String::new(),
            stderr: stderr.into(),
            segundos: 0.0,
        }
    }

    /// Un ejecutor que responde la recolección con `coleccion` y, si no es recolección, deja `xml`
    /// como junit y devuelve `codigo`.
    fn ejecutor_con(
        coleccion: &'static str,
        xml: Option<&'static str>,
        codigo: i32,
    ) -> impl Fn(&contenedor::Ejecucion) -> Result<contenedor::Resultado, ErrorOraculo> {
        move |e: &contenedor::Ejecucion| {
            if e.cmd.contains("--collect-only") {
                fs::write(e.salida.join(ARCHIVO_COLECCION), coleccion).expect("coleccion");
                return Ok(resultado_falso(0, ""));
            }
            if let Some(texto) = xml {
                fs::write(e.salida.join(XML_OCULTO), texto).expect("junit");
            }
            Ok(resultado_falso(codigo, ""))
        }
    }

    fn correr_suite_de_prueba(
        base: &Path,
        tarea: &Tarea,
        correr: &dyn Fn(&contenedor::Ejecucion) -> Result<contenedor::Resultado, ErrorOraculo>,
    ) -> Result<Suite, ErrorOraculo> {
        let repo = base.join("repo");
        let salida = base.join("salida");
        let home = base.join("home");
        fs::create_dir_all(&repo).expect("repo");
        fs::create_dir_all(&salida).expect("salida");
        fs::create_dir_all(&home).expect("home");
        let o = OpcionesOraculo::default();
        let c = Corrida {
            repo: &repo,
            tarea,
            salida: &salida,
            o: &o,
            cache: None,
            rustsec: None,
            home: &home,
        };
        evaluar_suite(
            &c,
            &tarea.oculto,
            DIR_OCULTO,
            XML_OCULTO,
            &BTreeSet::new(),
            correr,
        )
    }

    fn coleccion_de(n: usize) -> &'static str {
        Box::leak(
            (1..=n)
                .map(|i| format!("tests/test_a.py::t{i}\n"))
                .chain(std::iter::once(format!("{n} tests collected in 0.01s\n")))
                .collect::<String>()
                .into_boxed_str(),
        )
    }

    const JUNIT_TRES: &str = r#"<testsuite>
  <testcase classname="tests.test_a" name="t1"/>
  <testcase classname="tests.test_a" name="t2"/>
  <testcase classname="tests.test_a" name="t3"/>
</testsuite>"#;

    #[test]
    fn oraculo_junit_de_3_con_40_recolectados_es_sin_veredicto() {
        let base = tempfile::tempdir().expect("base");
        let tarea = tarea_de_prueba(base.path());
        let correr = ejecutor_con(coleccion_de(40), Some(JUNIT_TRES), 0);
        let s = correr_suite_de_prueba(base.path(), &tarea, &correr).expect("sin error de infra");
        assert!(
            matches!(s, Suite::SinVeredicto(_)),
            "3 tests de 40 no es veredicto: {s:?}"
        );
    }

    #[test]
    fn oraculo_codigo_de_pytest_que_no_cuadra_con_las_fallas_es_sin_veredicto() {
        let con_falla = r#"<testsuite><testcase classname="tests.test_a" name="t1"><failure message="x"/></testcase></testsuite>"#;
        let sin_falla = r#"<testsuite><testcase classname="tests.test_a" name="t1"/></testsuite>"#;
        let casos = [
            // Código 0 con una falla en el junit: el junit no es de esta corrida.
            (
                ejecutor_con(coleccion_de(1), Some(con_falla), 0),
                "0 con falla",
            ),
            // Código 1 sin fallas en el junit.
            (
                ejecutor_con(coleccion_de(1), Some(sin_falla), 1),
                "1 sin falla",
            ),
            // Código 5: ningún test recolectado. Código 2: interrumpida.
            (ejecutor_con(coleccion_de(1), Some(sin_falla), 5), "5"),
            (ejecutor_con(coleccion_de(1), Some(sin_falla), 2), "2"),
        ];
        for (correr, nombre) in casos {
            let base = tempfile::tempdir().expect("base");
            let tarea = tarea_de_prueba(base.path());
            let s = correr_suite_de_prueba(base.path(), &tarea, &correr).expect("sin infra");
            assert!(matches!(s, Suite::SinVeredicto(_)), "{nombre}: {s:?}");
        }
    }

    #[test]
    fn oraculo_junit_falsificado_con_codigo_de_pytest_valido_no_cuenta_si_no_son_los_recolectados()
    {
        // El producto escribe un junit con otro test; pytest recolectó `test_a.py::t1`.
        let base = tempfile::tempdir().expect("base");
        let tarea = tarea_de_prueba(base.path());
        let falso = r#"<testsuite><testcase classname="tests.otro" name="z"/></testsuite>"#;
        let correr = ejecutor_con(coleccion_de(1), Some(falso), 0);
        let s = correr_suite_de_prueba(base.path(), &tarea, &correr).expect("sin infra");
        assert!(matches!(s, Suite::SinVeredicto(_)), "{s:?}");
    }

    #[test]
    fn oraculo_stderr_con_texto_oculto_no_sale_en_el_error_y_queda_en_registros() {
        const SECRETO: &str = "/oraculo/oculto/test_a.py::t_secreto contenido-filtrado";
        let base = tempfile::tempdir().expect("base");
        let tarea = tarea_de_prueba(base.path());
        // Sin junit: el producto escribió el texto en stderr y mató a pytest.
        let correr = |e: &contenedor::Ejecucion| {
            if e.cmd.contains("--collect-only") {
                fs::write(e.salida.join(ARCHIVO_COLECCION), coleccion_de(1)).expect("coleccion");
                return Ok(resultado_falso(0, ""));
            }
            Ok(resultado_falso(-9, SECRETO))
        };
        let error = correr_suite_de_prueba(base.path(), &tarea, &correr)
            .expect_err("sin junit es error de infraestructura");
        let texto = error.to_string();
        assert!(!texto.contains("contenido-filtrado"), "{texto}");
        assert!(!texto.contains("t_secreto"), "{texto}");
        assert_eq!(error.codigo_de_salida(), 3);
        let registros = base.path().join("home/oraculos/tarea-prueba/registros");
        let archivos: Vec<PathBuf> = fs::read_dir(&registros)
            .expect("registros existe")
            .flatten()
            .map(|e| e.path())
            .collect();
        assert_eq!(archivos.len(), 1, "un archivo de stderr: {archivos:?}");
        let contenido = fs::read_to_string(&archivos[0]).expect("lee el registro");
        assert!(contenido.contains("contenido-filtrado"), "{contenido}");
        let nombre = archivos[0]
            .file_name()
            .expect("nombre")
            .to_string_lossy()
            .into_owned();
        assert!(
            texto.contains(&nombre),
            "el mensaje nombra el archivo: {texto}"
        );
    }

    #[test]
    fn oraculo_test_set_y_test_get_con_el_mismo_nombre_cuentan_dos() {
        let coleccion: &'static str = "tests/test_set.py::TestBasico::test_vacio\n\
             tests/test_get.py::TestBasico::test_vacio\n2 tests collected in 0.01s\n";
        let xml = r#"<testsuite>
  <testcase classname="tests.test_set.TestBasico" name="test_vacio"><failure message="x"/></testcase>
  <testcase classname="tests.test_get.TestBasico" name="test_vacio"/>
</testsuite>"#;
        let base = tempfile::tempdir().expect("base");
        let tarea = tarea_de_prueba(base.path());
        let correr = ejecutor_con(coleccion, Some(xml), 1);
        let s = correr_suite_de_prueba(base.path(), &tarea, &correr).expect("sin infra");
        let Suite::Clasificada(c) = s else {
            panic!("debía clasificar: {s:?}");
        };
        assert_eq!(c.conteo, Conteo { pasa: 1, total: 2 });
        assert!(c.estables.contains_key("test_set::TestBasico::test_vacio"));
        assert!(c.estables.contains_key("test_get::TestBasico::test_vacio"));
    }

    #[test]
    fn oraculo_contrato_no_monta_la_tarea_entera_ni_oculto() {
        let base = tempfile::tempdir().expect("base");
        let tarea = tarea_de_prueba(base.path());
        let repo = base.path().join("repo");
        let salida = base.path().join("salida");
        fs::create_dir_all(&repo).expect("repo");
        fs::create_dir_all(&salida).expect("salida");
        let o = OpcionesOraculo::default();
        let c = Corrida {
            repo: &repo,
            tarea: &tarea,
            salida: &salida,
            o: &o,
            cache: None,
            rustsec: None,
            home: base.path(),
        };
        let e = ejecucion_de_suite(&c, &tarea.contrato.cmd, DIR_CONTRATO);
        let args = contenedor::argumentos(&e, "n");
        assert!(
            !args
                .iter()
                .any(|a| a.starts_with(&format!("{}:/oraculo:", base.path().display()))),
            "no monta la tarea en /oraculo: {args:?}"
        );
        let montajes: Vec<&String> = args
            .windows(2)
            .filter(|p| p[0] == "-v")
            .map(|p| &p[1])
            .collect();
        assert!(
            montajes
                .iter()
                .any(|m| m.ends_with(":/oraculo/contrato:ro,z")),
            "monta solo contrato: {montajes:?}"
        );
        assert!(
            !montajes.iter().any(|m| m.contains("oculto")),
            "{montajes:?}"
        );
    }

    #[test]
    fn oraculo_redaccion_mcp_quita_ids_de_tests_ocultos() {
        let mut v = Veredicto {
            inestables: vec!["test_a::t1".into()],
            inestables_contrato: vec!["test_c::t9".into()],
            pendientes: vec![crate::tipos::Pendiente {
                test: "test_b::x".into(),
                motivo: "gpu".into(),
                colab: None,
            }],
            avisos: vec!["aviso sin ids".into()],
            ..Veredicto::default()
        };
        v.tasas.insert(
            "oculto:test_a::t1".into(),
            crate::tipos::TasaTest {
                pasa: 1,
                total: 2,
                wilson_inf: 0.1,
            },
        );
        v.hallazgos.push(Hallazgo {
            dimension: crate::tipos::Dimension::Seguridad,
            severidad: 2,
            archivo: "src/lib.rs".into(),
            linea: 1,
            cita: "test_a::t1".into(),
            detalle: "falla test_a::t1".into(),
            fuente: crate::tipos::Fuente::Sonda("x".into()),
            id: String::new(),
            impacto: Vec::new(),
        });
        v.hallazgos.push(Hallazgo {
            dimension: crate::tipos::Dimension::Seguridad,
            severidad: 1,
            archivo: "src/lib.rs".into(),
            linea: 2,
            cita: "unwrap()".into(),
            detalle: "unwrap sin control".into(),
            fuente: crate::tipos::Fuente::Sonda("x".into()),
            id: String::new(),
            impacto: Vec::new(),
        });
        let r = redactar_para_mcp(v);
        let texto = serde_json::to_string(&r).expect("json");
        assert!(!texto.contains("test_a::t1"), "{texto}");
        assert!(!texto.contains("test_b::x"), "{texto}");
        assert!(
            texto.contains("test_c::t9"),
            "el contrato no está oculto: {texto}"
        );
        assert!(
            texto.contains("unwrap sin control"),
            "un hallazgo sin ids queda: {texto}"
        );
        assert_eq!(r.hallazgos.len(), 1);
    }

    #[test]
    fn temporales_limpieza_borra_colab_viejas_y_conserva_recientes() {
        let home = tempfile::tempdir().expect("temp dir");
        let tmp = home.path().join("tmp");
        let viejo = tmp.join("colab-viejo");
        let reciente = tmp.join("colab-reciente");
        fs::create_dir_all(&viejo).expect("dir");
        fs::write(viejo.join("pendientes.xml"), "x").expect("escribe");
        fs::create_dir_all(&reciente).expect("dir");
        let ahora = SystemTime::now();
        poner_mtime(&viejo, ahora - Duration::from_secs(25 * 3600));
        limpiar_temporales(&tmp, ahora);
        assert!(!viejo.exists(), "la carpeta colab de más de 24 h se borra");
        assert!(reciente.exists(), "la reciente se conserva");
    }

    #[test]
    fn oraculo_colab_de_otro_commit_no_entra_al_veredicto() {
        let entrada = |commit: &str| pendientes::ColabEntrada {
            resultado: "pasa".into(),
            junit_sha256: "x".into(),
            ts: 1.0,
            commit: commit.into(),
        };
        let colab = BTreeMap::from([
            ("test_a::uno".to_string(), entrada("abc")),
            ("test_a::dos".to_string(), entrada("vieja")),
        ]);
        let (del_commit, aviso) = filtrar_colab_por_commit(colab, "abc");
        assert_eq!(del_commit.keys().collect::<Vec<_>>(), vec!["test_a::uno"]);
        assert_eq!(
            aviso.as_deref(),
            Some("1 resultados de Colab de otro commit no entran al veredicto")
        );
        let (todos, sin_aviso) = filtrar_colab_por_commit(
            BTreeMap::from([("test_a::uno".to_string(), entrada("abc"))]),
            "abc",
        );
        assert_eq!(todos.len(), 1);
        assert_eq!(sin_aviso, None);
    }
}
