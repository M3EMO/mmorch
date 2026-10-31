//! Historial de evidencia por versión del repo (Tarea 39).
//!
//! Root guarda el resultado de cada test por corrida de suite en
//! `ROOT_HOME/evidencia/<tarea>/<hash>/historial.jsonl`. El hash sale del contenido de la copia del repo.
//! Una versión nueva arranca con un historial vacío. Solo guarda ids y resultados, nunca el contenido de un
//! test, y vive fuera de `oraculos/`.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::junit::{Junit, ResultadoTest};
use super::{EXCLUIDOS, ErrorOraculo};
use crate::oracle::pendientes::nombre_de_tarea_valido;

/// Carpeta de los historiales dentro de `ROOT_HOME`. Es hermana de `oraculos/`: nunca vive dentro.
pub const CARPETA_EVIDENCIA: &str = "evidencia";
/// Archivo del historial de una versión, dentro de su carpeta.
pub const ARCHIVO_HISTORIAL: &str = "historial.jsonl";
/// Versión del formato de cada línea.
const VERSION_LINEA: u32 = 1;

/// Resultado de un test en una línea del historial: `pasa`, `falla` u `omite`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Resultado {
    Pasa,
    Falla,
    Omite,
}

impl From<ResultadoTest> for Resultado {
    fn from(r: ResultadoTest) -> Self {
        match r {
            ResultadoTest::Pasa => Resultado::Pasa,
            ResultadoTest::Falla => Resultado::Falla,
            ResultadoTest::Omite => Resultado::Omite,
        }
    }
}

impl From<Resultado> for ResultadoTest {
    fn from(r: Resultado) -> Self {
        match r {
            Resultado::Pasa => ResultadoTest::Pasa,
            Resultado::Falla => ResultadoTest::Falla,
            Resultado::Omite => ResultadoTest::Omite,
        }
    }
}

/// Una línea del historial: una corrida de una suite. Solo ids y resultados.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Linea {
    v: u32,
    suite: String,
    ts: f64,
    tests: BTreeMap<String, Resultado>,
}

/// Hash hexadecimal del contenido de `repo`. Entra cada archivo regular, en orden por ruta relativa con
/// `/`: su ruta, un `\0`, su largo en bytes, un `\0` y su contenido. Saltea los enlaces simbólicos y, solo
/// en la raíz, `target/` y `.git/`.
pub fn hash_de_version(repo: &Path) -> io::Result<String> {
    let mut archivos = Vec::new();
    recolectar(repo, repo, true, &mut archivos)?;
    archivos.sort();
    let mut digest = Sha256::new();
    for (relativa, ruta) in &archivos {
        let contenido = fs::read(ruta)?;
        digest.update(relativa);
        digest.update([0]);
        digest.update(contenido.len().to_string().as_bytes());
        digest.update([0]);
        digest.update(&contenido);
    }
    Ok(digest
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// Junta los archivos regulares de `dir` con su ruta relativa a `raiz`, en bytes con `/`.
fn recolectar(
    raiz: &Path,
    dir: &Path,
    es_raiz: bool,
    salida: &mut Vec<(Vec<u8>, PathBuf)>,
) -> io::Result<()> {
    for entrada in fs::read_dir(dir)? {
        let entrada = entrada?;
        let tipo = entrada.file_type()?;
        let nombre = entrada.file_name();
        if tipo.is_symlink() {
            continue;
        }
        if es_raiz && EXCLUIDOS.iter().any(|excluido| nombre == *excluido) {
            continue;
        }
        let ruta = entrada.path();
        if tipo.is_dir() {
            recolectar(raiz, &ruta, false, salida)?;
        } else if tipo.is_file() {
            let relativa = ruta
                .strip_prefix(raiz)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            salida.push((bytes_con_barra(relativa), ruta));
        }
    }
    Ok(())
}

/// Ruta relativa como bytes, con `/` entre componentes, sin importar el separador del sistema.
fn bytes_con_barra(relativa: &Path) -> Vec<u8> {
    let mut bytes = Vec::new();
    for (i, componente) in relativa.components().enumerate() {
        if i > 0 {
            bytes.push(b'/');
        }
        bytes.extend_from_slice(componente.as_os_str().as_bytes());
    }
    bytes
}

/// Ruta del historial de la versión `version` de la tarea `tarea` en `home`. Un nombre de tarea que no
/// puede ser carpeta no da ruta.
pub fn ruta(home: &Path, tarea: &str, version: &str) -> Result<PathBuf, ErrorOraculo> {
    nombre_de_tarea_valido(tarea).map_err(ErrorOraculo::Tarea)?;
    if version.is_empty() || !version.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(ErrorOraculo::Infra(format!(
            "versión del repo inválida: {version:?}"
        )));
    }
    Ok(home
        .join(CARPETA_EVIDENCIA)
        .join(tarea)
        .join(version)
        .join(ARCHIVO_HISTORIAL))
}

/// Corridas de `suite` guardadas en `ruta`, con los avisos de las líneas que no se pudieron leer. Sin
/// archivo, no hay corridas ni avisos. Una línea ilegible o de otra versión se ignora y deja un aviso.
pub fn leer(ruta: &Path, suite: &str) -> Result<(Vec<Junit>, Vec<String>), ErrorOraculo> {
    let bytes = match fs::read(ruta) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok((Vec::new(), Vec::new())),
        Err(e) => {
            return Err(ErrorOraculo::Infra(format!(
                "leer el historial {}: {e}",
                ruta.display()
            )));
        }
    };
    let mut corridas = Vec::new();
    let mut avisos = Vec::new();
    for (indice, linea) in bytes.split(|b| *b == b'\n').enumerate() {
        if linea.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let numero = indice + 1;
        match serde_json::from_slice::<Linea>(linea) {
            Ok(l) if l.v != VERSION_LINEA => avisos.push(format!(
                "historial de evidencia: línea {numero} de versión {} ignorada",
                l.v
            )),
            Ok(l) if l.suite == suite => corridas.push(Junit {
                tests: l
                    .tests
                    .into_iter()
                    .map(|(id, r)| (id, ResultadoTest::from(r)))
                    .collect(),
                ..Junit::default()
            }),
            Ok(_) => {}
            Err(_) => avisos.push(format!(
                "historial de evidencia: línea {numero} ilegible, ignorada"
            )),
        }
    }
    Ok((corridas, avisos))
}

/// Agrega una línea por corrida de `suite` al final de `ruta`. Crea la carpeta si no existe.
pub fn agregar(ruta: &Path, suite: &str, corridas: &[Junit]) -> Result<(), ErrorOraculo> {
    if corridas.is_empty() {
        return Ok(());
    }
    if let Some(padre) = ruta.parent() {
        fs::create_dir_all(padre)
            .map_err(|e| ErrorOraculo::Infra(format!("carpeta del historial: {e}")))?;
    }
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64());
    let mut texto = String::new();
    for corrida in corridas {
        let linea = Linea {
            v: VERSION_LINEA,
            suite: suite.to_string(),
            ts,
            tests: corrida
                .tests
                .iter()
                .map(|(id, r)| (id.clone(), Resultado::from(*r)))
                .collect(),
        };
        let json = serde_json::to_string(&linea)
            .map_err(|e| ErrorOraculo::Infra(format!("serializar el historial: {e}")))?;
        texto.push_str(&json);
        texto.push('\n');
    }
    let mut archivo: File = OpenOptions::new()
        .create(true)
        .append(true)
        .open(ruta)
        .map_err(|e| ErrorOraculo::Infra(format!("abrir el historial {}: {e}", ruta.display())))?;
    archivo
        .write_all(texto.as_bytes())
        .map_err(|e| ErrorOraculo::Infra(format!("escribir el historial: {e}")))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::*;

    fn escribir(repo: &Path, rel: &str, contenido: &[u8]) {
        let ruta = repo.join(rel);
        fs::create_dir_all(ruta.parent().expect("padre")).expect("carpeta");
        fs::write(ruta, contenido).expect("archivo");
    }

    fn repo_base(base: &Path) -> PathBuf {
        let repo = base.join("repo");
        escribir(&repo, "src/lib.rs", b"pub fn f() -> u8 { 1 }\n");
        escribir(&repo, "Cargo.toml", b"[package]\nname = \"x\"\n");
        repo
    }

    fn junit_de(filas: &[(&str, ResultadoTest)]) -> Junit {
        Junit {
            tests: filas
                .iter()
                .map(|(id, r)| ((*id).to_string(), *r))
                .collect(),
            ..Junit::default()
        }
    }

    #[test]
    fn historial_target_y_git_de_la_raiz_no_cambian_el_hash() {
        let base = tempfile::tempdir().expect("base");
        let repo = repo_base(base.path());
        let antes = hash_de_version(&repo).expect("hash");
        escribir(&repo, "target/debug/binario", b"basura");
        escribir(&repo, ".git/HEAD", b"ref: refs/heads/main\n");
        assert_eq!(hash_de_version(&repo).expect("hash"), antes);
    }

    #[test]
    fn historial_target_anidado_cuenta_en_el_hash() {
        let base = tempfile::tempdir().expect("base");
        let repo = repo_base(base.path());
        let antes = hash_de_version(&repo).expect("hash");
        escribir(&repo, "src/target/notas.rs", b"// no es la raiz\n");
        assert_ne!(hash_de_version(&repo).expect("hash"), antes);
    }

    #[test]
    fn historial_un_archivo_cambiado_cambia_el_hash() {
        let base = tempfile::tempdir().expect("base");
        let repo = repo_base(base.path());
        let antes = hash_de_version(&repo).expect("hash");
        escribir(&repo, "src/lib.rs", b"pub fn f() -> u8 { 2 }\n");
        assert_ne!(hash_de_version(&repo).expect("hash"), antes);
    }

    #[test]
    fn historial_renombrar_un_archivo_cambia_el_hash() {
        let base = tempfile::tempdir().expect("base");
        let repo = repo_base(base.path());
        let antes = hash_de_version(&repo).expect("hash");
        fs::rename(repo.join("src/lib.rs"), repo.join("src/otro.rs")).expect("renombra");
        assert_ne!(hash_de_version(&repo).expect("hash"), antes);
    }

    #[test]
    fn historial_enlace_simbolico_no_cambia_el_hash() {
        let base = tempfile::tempdir().expect("base");
        let repo = repo_base(base.path());
        let antes = hash_de_version(&repo).expect("hash");
        std::os::unix::fs::symlink("/etc/hostname", repo.join("enlace")).expect("enlace");
        assert_eq!(hash_de_version(&repo).expect("hash"), antes);
    }

    #[test]
    fn historial_vive_en_evidencia_fuera_de_oraculos() {
        let home = Path::new("/home/x/.root");
        let ruta = ruta(home, "tarea-rust", "abc123").expect("ruta");
        assert_eq!(
            ruta,
            home.join("evidencia")
                .join("tarea-rust")
                .join("abc123")
                .join("historial.jsonl")
        );
        assert!(!ruta.starts_with(home.join("oraculos")));
    }

    #[test]
    fn historial_nombre_de_tarea_invalido_no_da_ruta() {
        let home = Path::new("/home/x/.root");
        assert!(ruta(home, "../otra", "abc123").is_err());
        assert!(ruta(home, "", "abc123").is_err());
    }

    #[test]
    fn historial_leer_sin_archivo_es_vacio_y_sin_avisos() {
        let base = tempfile::tempdir().expect("base");
        let ruta = base.path().join("no-existe").join(ARCHIVO_HISTORIAL);
        let (corridas, avisos) = leer(&ruta, "oculto").expect("lee");
        assert!(corridas.is_empty());
        assert!(avisos.is_empty());
    }

    #[test]
    fn historial_agregar_y_leer_separa_las_suites() {
        let base = tempfile::tempdir().expect("base");
        let ruta = base.path().join("h").join(ARCHIVO_HISTORIAL);
        agregar(
            &ruta,
            "contrato",
            &[
                junit_de(&[("test_a::t1", ResultadoTest::Pasa)]),
                junit_de(&[("test_a::t1", ResultadoTest::Falla)]),
            ],
        )
        .expect("agrega");
        agregar(
            &ruta,
            "oculto",
            &[junit_de(&[("test_b::t3", ResultadoTest::Omite)])],
        )
        .expect("agrega");
        let (contrato, avisos) = leer(&ruta, "contrato").expect("lee");
        assert!(avisos.is_empty());
        assert_eq!(contrato.len(), 2);
        assert_eq!(contrato[0].tests["test_a::t1"], ResultadoTest::Pasa);
        assert_eq!(contrato[1].tests["test_a::t1"], ResultadoTest::Falla);
        let (oculto, _) = leer(&ruta, "oculto").expect("lee");
        assert_eq!(oculto.len(), 1);
        assert_eq!(oculto[0].tests["test_b::t3"], ResultadoTest::Omite);
    }

    #[test]
    fn historial_linea_ilegible_da_aviso_y_no_tumba_la_lectura() {
        let base = tempfile::tempdir().expect("base");
        let ruta = base.path().join(ARCHIVO_HISTORIAL);
        let buena = r#"{"v":1,"suite":"oculto","ts":1.0,"tests":{"test_b::t3":"falla"}}"#;
        fs::write(&ruta, format!("{{no es json\n{buena}\n")).expect("escribe");
        let (corridas, avisos) = leer(&ruta, "oculto").expect("lee");
        assert_eq!(corridas.len(), 1);
        assert_eq!(corridas[0].tests["test_b::t3"], ResultadoTest::Falla);
        assert_eq!(avisos.len(), 1, "{avisos:?}");
        assert!(
            !avisos[0].contains("no es json"),
            "el aviso no cita la línea"
        );
    }

    #[test]
    fn historial_linea_de_otra_version_da_aviso_y_se_ignora() {
        let base = tempfile::tempdir().expect("base");
        let ruta = base.path().join(ARCHIVO_HISTORIAL);
        let futura = r#"{"v":2,"suite":"oculto","ts":1.0,"tests":{"test_b::t3":"falla"}}"#;
        fs::write(&ruta, format!("{futura}\n")).expect("escribe");
        let (corridas, avisos) = leer(&ruta, "oculto").expect("lee");
        assert!(corridas.is_empty());
        assert_eq!(avisos.len(), 1, "{avisos:?}");
    }

    #[test]
    fn historial_guarda_solo_ids_y_resultados() {
        let base = tempfile::tempdir().expect("base");
        let ruta = base.path().join(ARCHIVO_HISTORIAL);
        agregar(
            &ruta,
            "oculto",
            &[junit_de(&[
                ("test_b::t3", ResultadoTest::Pasa),
                ("test_b::t4", ResultadoTest::Falla),
            ])],
        )
        .expect("agrega");
        let texto = fs::read_to_string(&ruta).expect("lee");
        assert_eq!(texto.lines().count(), 1);
        let linea: serde_json::Value = serde_json::from_str(texto.trim()).expect("json");
        let claves: Vec<&str> = linea
            .as_object()
            .expect("objeto")
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(claves, vec!["suite", "tests", "ts", "v"]);
        assert_eq!(linea["v"], 1);
        assert_eq!(linea["suite"], "oculto");
        assert_eq!(linea["tests"]["test_b::t3"], "pasa");
        assert_eq!(linea["tests"]["test_b::t4"], "falla");
    }
}
