//! Oracle: corre los tests en un contenedor sin red y cuenta el resultado desde junit.

pub mod contenedor;
pub mod junit;
pub mod tarea;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use thiserror::Error as ErrorDerive;

use crate::tipos::{Conteo, Estado, VERSION, Veredicto};
use junit::{Junit, ResultadoTest};
use tarea::Tarea;

/// Carpetas de la raíz del repo que no se copian: artefactos del builder e historial de git.
const EXCLUIDOS: &[&str] = &["target", ".git"];
/// Nombre del junit del contrato dentro de `/salida`.
const XML_CONTRATO: &str = "contrato.xml";
/// Nombre del junit del oculto dentro de `/salida`.
const XML_OCULTO: &str = "oculto.xml";

/// Errores del Oracle. Salen con código 3. Ante uno de ellos no hay veredicto.
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
}

/// Opciones de una evaluación.
#[derive(Debug, Clone)]
pub struct OpcionesOraculo {
    /// Límite de cada corrida de tests dentro del contenedor.
    pub timeout: Duration,
}

impl Default for OpcionesOraculo {
    fn default() -> Self {
        OpcionesOraculo {
            timeout: Duration::from_secs(3600),
        }
    }
}

/// Evalúa `repo` con la tarea: corre el contrato y el oculto sobre una copia del repo.
pub fn evaluar(repo: &Path, tarea: &Tarea, o: &OpcionesOraculo) -> Result<Veredicto, ErrorOraculo> {
    let repo = fs::canonicalize(repo)
        .map_err(|e| ErrorOraculo::Infra(format!("no existe el repo {}: {e}", repo.display())))?;
    // La copia aísla el repo: un artefacto del builder no altera el original.
    let copia = tempfile::tempdir().map_err(|e| infra("directorio temporal", e))?;
    let repo_copia = copia.path().join("repo");
    copiar_repo(&repo, &repo_copia).map_err(|e| infra("copia del repo", e))?;
    let salida = tempfile::tempdir().map_err(|e| infra("directorio de salida", e))?;
    // Podman rootless: si el contenedor corre con otro uid, la salida debe ser escribible por todos.
    fs::set_permissions(salida.path(), fs::Permissions::from_mode(0o777))
        .map_err(|e| infra("permisos de la salida", e))?;

    // El contrato corre una vez. El oculto corre `repeticiones` veces, cada una en un contenedor nuevo.
    let contrato = correr_suite(
        &repo_copia,
        tarea,
        salida.path(),
        &tarea.contrato.cmd,
        XML_CONTRATO,
        o,
    )?
    .conteo();
    let mut junits = Vec::new();
    for _ in 0..tarea.repeticiones {
        borrar_oculto(salida.path())?;
        junits.push(correr_suite(
            &repo_copia,
            tarea,
            salida.path(),
            &tarea.oculto.cmd,
            XML_OCULTO,
            o,
        )?);
    }
    let excluidos: BTreeSet<String> = tarea
        .entorno
        .iter()
        .chain(&tarea.oraculo_dudoso)
        .map(|e| e.test.clone())
        .collect();
    let clasificado = clasificar(&junits, &excluidos);
    let estado = estado_de(&contrato, &clasificado.conteo);
    Ok(Veredicto {
        v: VERSION,
        tarea: tarea.nombre.clone(),
        repo: nombre_de(&repo),
        commit: commit_de(&repo),
        contrato,
        oculto: clasificado.conteo,
        puntajes: Vec::new(),
        hallazgos: Vec::new(),
        global: None,
        estado,
        costo_usd: 0.0,
        informe: None,
        rubricas: Vec::new(),
        repeticiones: junits.len() as u32,
        inestables: clasificado.inestables,
    })
}

/// Borra el `oculto.xml` de la repetición anterior. Sin borrarlo, una repetición sin XML pasaría con datos viejos.
fn borrar_oculto(salida: &Path) -> Result<(), ErrorOraculo> {
    match fs::remove_file(salida.join(XML_OCULTO)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(infra("borrar oculto.xml", e)),
    }
}

/// Resultado de clasificar los tests del oculto a través de sus repeticiones.
#[derive(Debug, Clone, Default, PartialEq)]
struct Clasificacion {
    /// Tests estables con su resultado (`Pasa` o `Falla`). Los omitidos no aparecen.
    estables: BTreeMap<String, ResultadoTest>,
    /// Ids ordenados de los tests con resultados mezclados.
    inestables: Vec<String>,
    /// Conteo del oculto. Un inestable no pasa, pero sí cuenta en el total.
    conteo: Conteo,
}

/// Clasifica cada test por sus repeticiones. Los excluidos no entran al cálculo.
/// Un test con resultados distintos entre repeticiones, o ausente en alguna, es inestable.
fn clasificar(junits: &[Junit], excluidos: &BTreeSet<String>) -> Clasificacion {
    let ids: BTreeSet<&str> = junits
        .iter()
        .flat_map(|j| j.tests.keys().map(String::as_str))
        .collect();
    let mut c = Clasificacion::default();
    for id in ids {
        if excluidos.contains(id) {
            continue;
        }
        let resultados: Vec<Option<ResultadoTest>> =
            junits.iter().map(|j| j.tests.get(id).copied()).collect();
        match resultados.first().copied().flatten() {
            Some(r) if resultados.iter().all(|x| *x == Some(r)) => {
                if r != ResultadoTest::Omite {
                    c.estables.insert(id.to_string(), r);
                }
            }
            _ => c.inestables.push(id.to_string()),
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

/// Corre un comando de la tarea y lee su junit. Sin junit no hay conteo: es error de infraestructura.
fn correr_suite(
    repo: &Path,
    tarea: &Tarea,
    salida: &Path,
    cmd: &str,
    xml: &str,
    o: &OpcionesOraculo,
) -> Result<Junit, ErrorOraculo> {
    let ejecucion = contenedor::Ejecucion {
        imagen: tarea.imagen.clone(),
        repo: Some(repo.to_path_buf()),
        oraculo: Some(tarea.dir.clone()),
        salida: salida.to_path_buf(),
        cmd: cmd.to_string(),
        env: Vec::new(),
        timeout: o.timeout,
    };
    let resultado = contenedor::ejecutar(&ejecucion)?;
    let ruta = salida.join(xml);
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
        let c = clasificar(&[j.clone(), j.clone(), j], &sin_exclusiones());
        assert_eq!(c.conteo, Conteo { pasa: 1, total: 1 });
        assert!(c.inestables.is_empty());
    }

    #[test]
    fn repeticiones_falla_en_todas_es_estable_que_falla() {
        let j = junit_de(&[("a::t1", ResultadoTest::Falla)]);
        let c = clasificar(&[j.clone(), j.clone(), j], &sin_exclusiones());
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
        let c = clasificar(&[r1, r2, r3], &sin_exclusiones());
        assert_eq!(c.inestables, vec!["a::t_flaky".to_string()]);
        assert_eq!(c.conteo, Conteo { pasa: 1, total: 2 });
    }

    #[test]
    fn repeticiones_omitido_en_todas_no_cuenta() {
        let j = junit_de(&[("a::t1", ResultadoTest::Omite)]);
        let c = clasificar(&[j.clone(), j], &sin_exclusiones());
        assert_eq!(c.conteo, Conteo { pasa: 0, total: 0 });
        assert!(c.inestables.is_empty());
    }

    #[test]
    fn repeticiones_test_ausente_en_una_repeticion_es_inestable() {
        let con = junit_de(&[("a::t1", ResultadoTest::Pasa)]);
        let sin = junit_de(&[]);
        let c = clasificar(&[con, sin], &sin_exclusiones());
        assert_eq!(c.inestables, vec!["a::t1".to_string()]);
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
        let c = clasificar(&[r1, r2], &excluidos);
        assert!(c.inestables.is_empty());
        assert_eq!(c.conteo, Conteo { pasa: 1, total: 1 });
    }

    #[test]
    fn exclusion_de_test_inexistente_se_ignora() {
        let j = junit_de(&[("a::t1", ResultadoTest::Pasa)]);
        let excluidos = BTreeSet::from(["a::no_existe".to_string()]);
        let c = clasificar(&[j.clone(), j], &excluidos);
        assert_eq!(c.conteo, Conteo { pasa: 1, total: 1 });
    }

    #[test]
    fn oraculo_codigo_de_estado_verde_rojo_y_sin_veredicto() {
        assert_eq!(codigo_de_estado(Estado::Verde), 0);
        assert_eq!(codigo_de_estado(Estado::Rojo), 1);
        assert_eq!(codigo_de_estado(Estado::SinVeredicto), 3);
    }
}
