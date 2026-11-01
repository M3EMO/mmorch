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
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::junit::{Junit, ResultadoTest};
use super::{EXCLUIDOS, ErrorOraculo};
use crate::borrado;
use crate::oracle::pendientes::nombre_de_tarea_valido;

/// Carpeta de los historiales dentro de `ROOT_HOME`. Es hermana de `oraculos/`: nunca vive dentro.
pub const CARPETA_EVIDENCIA: &str = "evidencia";
/// Archivo del historial de una versión, dentro de su carpeta.
pub const ARCHIVO_HISTORIAL: &str = "historial.jsonl";
/// Versión del formato de cada línea.
const VERSION_LINEA: u32 = 1;
/// Días sin uso a partir de los cuales `evaluar` borra el historial de una versión.
const CADUCIDAD_HISTORIAL: Duration = Duration::from_secs(30 * 24 * 3600);

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

/// Borra los historiales de las versiones de `tarea` sin uso en 30 días.
///
/// Una versión sin uso tiene su `historial.jsonl` con más de 30 días de antigüedad, respecto de `ahora`.
/// Solo revisa las carpetas con nombre de hash: 64 caracteres hexadecimales en minúsculas. Nunca borra la
/// carpeta de `actual`. No sigue ni borra los enlaces simbólicos. No toca una carpeta sin `historial.jsonl`.
/// Tampoco toca `colab.json` ni los archivos sueltos.
///
/// Ignora los fallos de lectura y de borrado, como `limpiar_temporales`. Un historial viejo que no se
/// borra no impide evaluar.
pub fn borrar_viejos(home: &Path, tarea: &str, actual: &str, ahora: SystemTime) {
    if nombre_de_tarea_valido(tarea).is_err() {
        return;
    }
    let carpeta_tarea = home.join(CARPETA_EVIDENCIA).join(tarea);
    let Ok(entradas) = fs::read_dir(&carpeta_tarea) else {
        return;
    };
    for entrada in entradas.flatten() {
        // `file_type` no sigue los enlaces: un enlace simbólico no es una versión.
        if !entrada.file_type().is_ok_and(|tipo| tipo.is_dir()) {
            continue;
        }
        let nombre = entrada.file_name();
        if nombre.to_str().is_none_or(|n| n == actual || !es_hash(n)) {
            continue;
        }
        let carpeta = entrada.path();
        // `symlink_metadata` no sigue el archivo: un historial que es enlace no cuenta como uso.
        let Ok(meta) = fs::symlink_metadata(carpeta.join(ARCHIVO_HISTORIAL)) else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let Ok(modificado) = meta.modified() else {
            continue;
        };
        let sin_uso = ahora
            .duration_since(modificado)
            .is_ok_and(|edad| edad > CADUCIDAD_HISTORIAL);
        if sin_uso {
            let _ = borrado::borrar_dir_dentro(&carpeta_tarea, &carpeta);
        }
    }
}

/// Nombre de carpeta con forma de hash de versión: 64 caracteres hexadecimales en minúsculas.
fn es_hash(nombre: &str) -> bool {
    nombre.len() == 64
        && nombre
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
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

    /// Hash de 64 caracteres hexadecimales en minúsculas, con la forma de `hash_de_version`.
    fn hash_de(letra: char) -> String {
        letra.to_string().repeat(64)
    }

    /// Fija la fecha de modificación de `ruta` a `dias` días antes de `ahora`.
    fn poner_antiguedad(ruta: &Path, dias: u64, ahora: SystemTime) {
        let cuando = ahora - Duration::from_secs(dias * 24 * 3600);
        filetime::set_file_mtime(ruta, filetime::FileTime::from_system_time(cuando))
            .expect("fija la fecha");
    }

    /// Crea el historial de la versión `version` de `tarea` en `home`, con `dias` días de antigüedad.
    fn historial_hace_dias(
        home: &Path,
        tarea: &str,
        version: &str,
        dias: u64,
        ahora: SystemTime,
    ) -> PathBuf {
        let ruta = home
            .join(CARPETA_EVIDENCIA)
            .join(tarea)
            .join(version)
            .join(ARCHIVO_HISTORIAL);
        fs::create_dir_all(ruta.parent().expect("padre")).expect("carpeta de la versión");
        fs::write(
            &ruta,
            "{\"v\":1,\"suite\":\"oculto\",\"ts\":1.0,\"tests\":{}}\n",
        )
        .expect("historial");
        poner_antiguedad(&ruta, dias, ahora);
        ruta
    }

    #[test]
    fn historial_version_sin_uso_en_31_dias_se_borra() {
        let base = tempfile::tempdir().expect("base");
        let ahora = SystemTime::now();
        let ruta = historial_hace_dias(base.path(), "tarea", &hash_de('a'), 31, ahora);
        let carpeta = ruta.parent().expect("carpeta").to_path_buf();
        borrar_viejos(base.path(), "tarea", &hash_de('b'), ahora);
        assert!(
            !carpeta.exists(),
            "sigue la versión sin uso: {}",
            carpeta.display()
        );
    }

    #[test]
    fn historial_version_usada_hace_29_dias_queda() {
        let base = tempfile::tempdir().expect("base");
        let ahora = SystemTime::now();
        let ruta = historial_hace_dias(base.path(), "tarea", &hash_de('a'), 29, ahora);
        borrar_viejos(base.path(), "tarea", &hash_de('b'), ahora);
        assert!(ruta.is_file(), "se borró: {}", ruta.display());
    }

    #[test]
    fn historial_version_actual_no_se_borra_aunque_sea_vieja() {
        let base = tempfile::tempdir().expect("base");
        let ahora = SystemTime::now();
        let actual = hash_de('a');
        let ruta = historial_hace_dias(base.path(), "tarea", &actual, 60, ahora);
        borrar_viejos(base.path(), "tarea", &actual, ahora);
        assert!(
            ruta.is_file(),
            "se borró la versión actual: {}",
            ruta.display()
        );
    }

    #[test]
    fn historial_borrar_viejos_no_toca_colab_json_ni_carpetas_sin_hash() {
        let base = tempfile::tempdir().expect("base");
        let ahora = SystemTime::now();
        let carpeta_tarea = base.path().join(CARPETA_EVIDENCIA).join("tarea");
        // colab.json viejo: es un archivo, no una versión.
        let colab = carpeta_tarea.join("colab.json");
        fs::create_dir_all(&carpeta_tarea).expect("carpeta de la tarea");
        fs::write(&colab, "{}").expect("colab.json");
        poner_antiguedad(&colab, 60, ahora);
        // Carpeta sin hash, con un historial viejo: su nombre no es un hash de 64 caracteres.
        let otra = historial_hace_dias(base.path(), "tarea", "otra", 60, ahora);
        // Control: una versión vieja con hash sí se borra.
        let vieja = historial_hace_dias(base.path(), "tarea", &hash_de('a'), 60, ahora);
        borrar_viejos(base.path(), "tarea", &hash_de('b'), ahora);
        assert!(colab.is_file(), "se borró colab.json");
        assert!(otra.is_file(), "se borró una carpeta sin hash");
        assert!(
            !vieja.parent().expect("carpeta").exists(),
            "el control con hash no se borró"
        );
    }

    #[test]
    fn historial_carpeta_sin_historial_no_se_toca() {
        let base = tempfile::tempdir().expect("base");
        let ahora = SystemTime::now();
        let carpeta = base
            .path()
            .join(CARPETA_EVIDENCIA)
            .join("tarea")
            .join(hash_de('a'));
        fs::create_dir_all(&carpeta).expect("carpeta sin historial");
        fs::write(carpeta.join("otro.txt"), "x").expect("otro archivo");
        poner_antiguedad(&carpeta, 60, ahora);
        borrar_viejos(base.path(), "tarea", &hash_de('b'), ahora);
        assert!(
            carpeta.join("otro.txt").is_file(),
            "se borró una carpeta sin historial: {}",
            carpeta.display()
        );
    }

    #[test]
    fn historial_enlace_simbolico_de_version_vieja_no_se_sigue_ni_se_borra() {
        let base = tempfile::tempdir().expect("base");
        let ahora = SystemTime::now();
        let home = base.path().join("home");
        // Destino fuera de `evidencia/`, con un historial viejo.
        let destino = base.path().join("fuera");
        fs::create_dir_all(&destino).expect("destino");
        let historial_destino = destino.join(ARCHIVO_HISTORIAL);
        fs::write(&historial_destino, "x").expect("historial del destino");
        poner_antiguedad(&historial_destino, 60, ahora);
        let enlace = home
            .join(CARPETA_EVIDENCIA)
            .join("tarea")
            .join(hash_de('a'));
        fs::create_dir_all(enlace.parent().expect("padre")).expect("carpeta de la tarea");
        std::os::unix::fs::symlink(&destino, &enlace).expect("enlace");
        borrar_viejos(&home, "tarea", &hash_de('b'), ahora);
        assert!(
            fs::symlink_metadata(&enlace).is_ok(),
            "se borró el enlace simbólico"
        );
        assert!(
            historial_destino.is_file(),
            "se borró el destino del enlace"
        );
    }

    #[test]
    fn historial_borrar_viejos_con_tarea_con_ruta_no_toca_nada() {
        let base = tempfile::tempdir().expect("base");
        let ahora = SystemTime::now();
        let home = base.path().join("home");
        // Con `../../otra`, la carpeta caería en `base/otra/`, fuera de `evidencia/`.
        let fuera = base.path().join("otra").join(hash_de('a'));
        fs::create_dir_all(&fuera).expect("carpeta de afuera");
        let historial_fuera = fuera.join(ARCHIVO_HISTORIAL);
        fs::write(&historial_fuera, "x").expect("historial de afuera");
        poner_antiguedad(&historial_fuera, 60, ahora);
        borrar_viejos(&home, "../../otra", &hash_de('b'), ahora);
        assert!(
            historial_fuera.is_file(),
            "un nombre de tarea con ruta borró algo fuera de evidencia/"
        );
    }
}
