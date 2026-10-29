//! Tests pendientes por entorno: el paquete para correrlos en Colab y la importación de su junit.
//! Root no lanza Colab. El usuario corre `correr.sh` y trae el junit de vuelta.
//! Todo lo que trae tests ocultos vive en `$ROOT_HOME/oraculos/<tarea>/` (A1, B18).

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use super::descarga;
use super::junit::{self, ResultadoTest};
use super::tarea::Tarea;
use super::{ErrorOraculo, comillas, commit_de, copiar_repo, infra};
use crate::borrado;
use crate::observe;
use crate::tipos::{Pendiente, TipoEvento};

/// Carpeta de los tests ocultos de cada tarea dentro de `ROOT_HOME`. El builder no puede leerla.
const CARPETA_ORACULOS: &str = "oraculos";
/// Paquete de los tests pendientes, dentro de la carpeta de la tarea.
const CARPETA_PAQUETE: &str = "pendientes";
/// Carpeta de los resultados importados dentro de `ROOT_HOME`.
const CARPETA_EVIDENCIA: &str = "evidencia";
/// Resultados de Colab de una tarea, dentro de su carpeta de evidencia.
const ARCHIVO_COLAB: &str = "colab.json";
/// Eventos de Root dentro de `ROOT_HOME`.
const ARCHIVO_EVENTOS: &str = "eventos.jsonl";
/// Junit que deja `correr.sh` al lado del paquete.
const XML_PENDIENTES: &str = "pendientes.xml";
/// Commit del repo del que se armó el paquete (B16).
pub(crate) const ARCHIVO_COMMIT: &str = "commit.txt";
/// Copia de la caché de dependencias dentro del paquete.
const CARPETA_CACHE_PAQUETE: &str = "cache";
/// Carpeta de temporales de `ROOT_HOME`, donde Colab baja el junit.
const CARPETA_TEMPORALES: &str = "tmp";
/// Prefijo de las carpetas de Colab dentro de `tmp/`.
const PREFIJO_COLAB: &str = "colab-";
/// Aviso del `Gate` y del veredicto cuando Colab no permitió `unshare -rn` (decisión 40).
pub const CONFIANZA_CON_RED: &str = "corrida de Colab con red: el oráculo pudo filtrarse";

/// Resultado de un test pendiente importado de Colab.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColabEntrada {
    /// "pasa", "falla" u "omite".
    pub resultado: String,
    /// Sha256 del junit del que salió el resultado.
    pub junit_sha256: String,
    /// Momento de la importación, en segundos desde la época.
    pub ts: f64,
    /// Commit del repo del que salió el paquete. Un resultado de otro commit no entra al veredicto (B16).
    #[serde(default)]
    pub commit: String,
}

/// Paquete armado para correr los tests pendientes.
#[derive(Debug, Clone, PartialEq)]
pub struct Paquete {
    pub dir: PathBuf,
    /// Ids `archivo::nombre` de los tests del paquete.
    pub tests: Vec<String>,
    /// Commit del repo del que se armó (vacío si no es git).
    pub commit: String,
    /// Avisos de armado: por ejemplo, que no hay caché de dependencias.
    pub avisos: Vec<String>,
}

/// Resumen de una importación de junit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Importacion {
    /// Tests de `[[entorno]]` que estaban en el junit.
    pub importados: usize,
    /// Tests del junit que no están en `[[entorno]]`.
    pub ignorados: usize,
    /// Avisos: un resultado de otro commit no se importa, por ejemplo.
    pub avisos: Vec<String>,
    /// Confianza de la corrida. `Some` si Colab corrió con red (decisión 40).
    pub confianza: Option<String>,
}

/// Dónde vive el paquete de una tarea: `ROOT_HOME/oraculos/<tarea>/pendientes`.
pub fn dir_paquete(home: &Path, tarea: &str) -> PathBuf {
    home.join(CARPETA_ORACULOS)
        .join(tarea)
        .join(CARPETA_PAQUETE)
}

/// Arma el paquete de los tests de `[[entorno]]` dentro de `oraculos/<tarea>/pendientes/`.
/// `cache` es la caché de dependencias de la fase de descarga, si ya existe: el paquete la lleva y
/// `correr.sh` la usa sin red. Sin tests pendientes no crea nada y da `None`.
/// Todo se valida antes de crear el directorio.
pub fn crear(
    home: &Path,
    tarea: &Tarea,
    repo: &Path,
    cache: Option<&Path>,
) -> Result<Option<Paquete>, ErrorOraculo> {
    nombre_de_tarea_valido(&tarea.nombre).map_err(ErrorOraculo::Tarea)?;
    if tarea.entorno.is_empty() {
        return Ok(None);
    }
    let repo = fs::canonicalize(repo)
        .map_err(|e| ErrorOraculo::Infra(format!("no existe el repo {}: {e}", repo.display())))?;
    let mut archivos: BTreeMap<String, PathBuf> = BTreeMap::new();
    let mut nodos = Vec::new();
    let mut tests = Vec::new();
    for exclusion in &tarea.entorno {
        let (archivo, _) = dividir(&exclusion.test)?;
        if !archivos.contains_key(archivo) {
            archivos.insert(archivo.to_string(), localizar(tarea, archivo)?);
        }
        nodos.push(nodo(&exclusion.test)?);
        tests.push(exclusion.test.clone());
    }

    // Un paquete viejo de esta tarea (de una corrida que no llegó a limpiarse) se borra antes.
    let dir = dir_paquete(home, &tarea.nombre);
    let raiz_tarea = home.join(CARPETA_ORACULOS).join(&tarea.nombre);
    borrado::borrar_dir_dentro(&raiz_tarea, &dir)
        .map_err(|e| infra("borrar el paquete viejo", e))?;
    fs::create_dir_all(&raiz_tarea).map_err(|e| infra("carpeta de la tarea", e))?;
    fs::create_dir(&dir).map_err(|e| infra(&format!("crear {}", dir.display()), e))?;

    copiar_repo(&repo, &dir.join("repo")).map_err(|e| infra("copia del repo", e))?;
    let tests_dir = dir.join("tests");
    fs::create_dir(&tests_dir).map_err(|e| infra("carpeta tests", e))?;
    for (archivo, origen) in &archivos {
        fs::copy(origen, tests_dir.join(format!("{archivo}.py")))
            .map_err(|e| infra(&format!("copiar {}", origen.display()), e))?;
    }
    let conftest = tarea.dir.join("oculto").join("conftest.py");
    if conftest.is_file() {
        fs::copy(&conftest, tests_dir.join("conftest.py"))
            .map_err(|e| infra("copiar conftest", e))?;
    }
    let commit = commit_de(&repo);
    escribir(&dir.join(ARCHIVO_COMMIT), &commit, false)?;
    let mut avisos = Vec::new();
    let con_cache = match cache {
        Some(origen) => {
            copiar_repo(origen, &dir.join(CARPETA_CACHE_PAQUETE))
                .map_err(|e| infra("copia de la caché", e))?;
            true
        }
        None => {
            avisos.push(
                "sin caché de dependencias: corré `r00t oraculo evaluar` antes para bajarlas"
                    .into(),
            );
            false
        }
    };
    let guion = if con_cache {
        guion_con_cache(&nodos, tarea.repeticiones, &tarea.lenguaje)
    } else {
        guion_correr(&nodos, tarea.repeticiones, "")
    };
    escribir(&dir.join("correr.sh"), &guion, true)?;
    escribir(&dir.join("LEEME.txt"), &leeme(&tarea.nombre, &repo), false)?;
    Ok(Some(Paquete {
        dir,
        tests,
        commit,
        avisos,
    }))
}

/// Borra lo que Colab dejó de una corrida: el paquete con los tests ocultos y `tmp/colab-<sesion>`
/// con el junit bajado (A1, B18). Cada borrado pasa por `borrado` con su raíz.
pub fn limpiar_paquete(home: &Path, tarea: &Tarea, paquete: &Path, sesion: &str) -> io::Result<()> {
    let raiz_tarea = home.join(CARPETA_ORACULOS).join(&tarea.nombre);
    borrado::borrar_dir_dentro(&raiz_tarea, paquete)?;
    let tmp = home.join(CARPETA_TEMPORALES);
    borrado::borrar_dir_dentro(&tmp, &tmp.join(format!("{PREFIJO_COLAB}{sesion}")))
}

/// Importa el junit que dejó `correr.sh`. `commit` es el del repo al que se importa. Si el paquete
/// de la tarea tiene otro commit, el resultado no se importa y el aviso lo dice (B16). Si entra, guarda
/// el resultado de cada test pendiente en `evidencia/<tarea>/colab.json` y registra un evento `Gate`.
pub fn importar(
    home: &Path,
    tarea: &Tarea,
    ruta: &Path,
    commit: &str,
) -> Result<Importacion, ErrorOraculo> {
    let texto = fs::read_to_string(ruta)
        .map_err(|e| ErrorOraculo::Infra(format!("no se pudo leer {}: {e}", ruta.display())))?;
    let junit = junit::parsear(&texto)?;
    if let Some(del_paquete) = commit_del_paquete(home, &tarea.nombre)
        && del_paquete != commit
    {
        return Ok(Importacion {
            importados: 0,
            ignorados: junit.tests.len(),
            avisos: vec![format!(
                "el junit es del commit {del_paquete} y el repo está en {commit}: no se importa"
            )],
            confianza: None,
        });
    }
    let sha = sha256_hex(texto.as_bytes());
    let (ts, _) = observe::ahora();

    let junit = junit.con_ids();
    let mut colab = colab_de(home, &tarea.nombre)?;
    let mut resultados = BTreeMap::new();
    for exclusion in &tarea.entorno {
        if let Some(resultado) = junit.tests.get(&exclusion.test) {
            let etiqueta = etiqueta(*resultado);
            colab.insert(
                exclusion.test.clone(),
                ColabEntrada {
                    resultado: etiqueta.into(),
                    junit_sha256: sha.clone(),
                    ts,
                    commit: commit.to_string(),
                },
            );
            resultados.insert(exclusion.test.clone(), etiqueta);
        }
    }
    let importados = resultados.len();
    let ignorados = junit.tests.len().saturating_sub(importados);
    guardar_colab(home, &tarea.nombre, &colab)?;

    // Decisión 40: si `unshare -rn` no funcionó en Colab, la corrida tuvo red y su confianza lo dice.
    let confianza =
        (junit.red.as_deref() == Some("con-red")).then(|| CONFIANZA_CON_RED.to_string());
    let mut evento = observe::evento_manual(
        &format!("pendientes-{}", tarea.nombre),
        TipoEvento::Gate,
        None,
    );
    evento.fase = "pendientes".into();
    evento.extra.insert("origen".into(), json!("colab"));
    evento.extra.insert("tests".into(), json!(resultados));
    evento.extra.insert("junit_sha256".into(), json!(sha));
    if let Some(texto) = &confianza {
        evento.extra.insert("confianza".into(), json!(texto));
    }
    observe::registrar_en(&home.join(ARCHIVO_EVENTOS), &evento)
        .map_err(|e| infra("registrar el evento", e))?;
    Ok(Importacion {
        importados,
        ignorados,
        avisos: Vec::new(),
        confianza,
    })
}

/// Commit con el que se armó el paquete de la tarea. `None` si no hay paquete.
fn commit_del_paquete(home: &Path, tarea: &str) -> Option<String> {
    let ruta = dir_paquete(home, tarea).join(ARCHIVO_COMMIT);
    fs::read_to_string(ruta).ok().map(|c| c.trim().to_string())
}

/// Resultados de Colab guardados para una tarea. Sin archivo, vacío.
pub fn colab_de(home: &Path, tarea: &str) -> Result<BTreeMap<String, ColabEntrada>, ErrorOraculo> {
    let ruta = ruta_colab(home, tarea)?;
    match fs::read_to_string(&ruta) {
        Ok(texto) => serde_json::from_str(&texto)
            .map_err(|e| ErrorOraculo::Infra(format!("{}: {e}", ruta.display()))),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(e) => Err(infra(&format!("leer {}", ruta.display()), e)),
    }
}

/// Resultados de Colab para el veredicto. Un `colab.json` ilegible no tumba la evaluación:
/// sus tests quedan pendientes y el aviso va al veredicto. Un nombre de tarea inválido sigue siendo error.
pub fn colab_para_veredicto(
    home: &Path,
    tarea: &str,
) -> Result<(BTreeMap<String, ColabEntrada>, Vec<String>), ErrorOraculo> {
    let ruta = ruta_colab(home, tarea)?;
    let leido: Result<BTreeMap<String, ColabEntrada>, String> = match fs::read_to_string(&ruta) {
        Ok(texto) => serde_json::from_str(&texto).map_err(|e| e.to_string()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Ok((BTreeMap::new(), Vec::new()));
        }
        Err(e) => Err(e.to_string()),
    };
    match leido {
        Ok(colab) => Ok((colab, Vec::new())),
        Err(causa) => Ok((
            BTreeMap::new(),
            vec![format!("{ARCHIVO_COLAB} ilegible: {causa}")],
        )),
    }
}

/// Los tests de `[[entorno]]` con su motivo y, si hay, su resultado de Colab.
pub fn desde(tarea: &Tarea, colab: &BTreeMap<String, ColabEntrada>) -> Vec<Pendiente> {
    tarea
        .entorno
        .iter()
        .map(|exclusion| Pendiente {
            test: exclusion.test.clone(),
            motivo: exclusion.motivo.clone(),
            colab: colab.get(&exclusion.test).map(|c| c.resultado.clone()),
        })
        .collect()
}

fn guardar_colab(
    home: &Path,
    tarea: &str,
    colab: &BTreeMap<String, ColabEntrada>,
) -> Result<(), ErrorOraculo> {
    let ruta = ruta_colab(home, tarea)?;
    if let Some(padre) = ruta.parent() {
        fs::create_dir_all(padre).map_err(|e| infra("carpeta de evidencia", e))?;
    }
    let texto = serde_json::to_string_pretty(colab)
        .map_err(|e| ErrorOraculo::Infra(format!("serializar colab.json: {e}")))?;
    fs::write(&ruta, texto).map_err(|e| infra(&format!("escribir {}", ruta.display()), e))
}

fn ruta_colab(home: &Path, tarea: &str) -> Result<PathBuf, ErrorOraculo> {
    nombre_seguro(tarea)?;
    Ok(home.join(CARPETA_EVIDENCIA).join(tarea).join(ARCHIVO_COLAB))
}

/// Un nombre usado como carpeta no puede traer rutas.
fn nombre_seguro(nombre: &str) -> Result<(), ErrorOraculo> {
    nombre_de_tarea_valido(nombre).map_err(ErrorOraculo::Tarea)
}

/// Nombre de tarea que puede ser una carpeta: sin `/`, `\`, `.` ni `..` (B7). Lo usan el CLI y `crear`.
pub fn nombre_de_tarea_valido(nombre: &str) -> Result<(), String> {
    let con_ruta =
        nombre.is_empty() || nombre.contains(['/', '\\']) || nombre == "." || nombre == "..";
    if con_ruta {
        return Err(format!("nombre inválido: {nombre:?}"));
    }
    Ok(())
}

/// Parte `archivo::nombre`. Sin separador es error de tarea.
fn dividir(id: &str) -> Result<(&str, &str), ErrorOraculo> {
    id.split_once("::").ok_or_else(|| {
        ErrorOraculo::Tarea(format!("el id {id:?} no tiene la forma archivo::nombre"))
    })
}

/// Node id de pytest dentro de la carpeta `tests/` del paquete.
fn nodo(id: &str) -> Result<String, ErrorOraculo> {
    let (archivo, nombre) = dividir(id)?;
    nombre_seguro(archivo)?;
    Ok(format!("tests/{archivo}.py::{nombre}"))
}

/// Busca el archivo del módulo en `oculto/` y, si no está, en `contrato/`.
fn localizar(tarea: &Tarea, archivo: &str) -> Result<PathBuf, ErrorOraculo> {
    nombre_seguro(archivo)?;
    for suite in ["oculto", "contrato"] {
        let ruta = tarea.dir.join(suite).join(format!("{archivo}.py"));
        if ruta.is_file() {
            return Ok(ruta);
        }
    }
    Err(ErrorOraculo::Tarea(format!(
        "no existe {archivo}.py en oculto/ ni en contrato/"
    )))
}

/// Script que junta los junit de cada repetición (`repeticion-N.xml`) en un solo `pendientes.xml`, con
/// todos sus testcases. El lector de junit de Root cuenta los testcases repetidos por id. La marca
/// `red.txt` que deja `correr.sh` va como atributo `root_red` del testsuite (decisión 40).
const GUION_JUNTAR: &str = r#"import glob
import os
import xml.etree.ElementTree as ET
raiz = ET.Element("testsuites")
suite = ET.SubElement(raiz, "testsuite", name="pendientes")
suite.set("root_red", open("red.txt").read().strip() if os.path.exists("red.txt") else "desconocido")
for ruta in sorted(glob.glob("repeticion-*.xml")):
    for caso in ET.parse(ruta).getroot().iter("testcase"):
        suite.append(caso)
ET.ElementTree(raiz).write("pendientes.xml", encoding="utf-8", xml_declaration=True)
"#;

/// Script que corre los nodos pendientes `repeticiones` veces y deja `pendientes.xml` al lado.
/// Los tests corren dentro de `unshare -rn` (sin red, decisión 40). Si Colab no deja crear ese
/// namespace, la corrida sigue con red y `red.txt` dice `con-red`. `prelude` prepara la caché, si la
/// hay, y va antes de `pytest` en la misma línea. Si hay una carpeta `bin/` al lado (el `cargo` mínimo
/// del respaldo de Colab), la pone al frente del `PATH`.
fn guion_correr(nodos: &[String], repeticiones: u32, prelude: &str) -> String {
    guion_con_previo(nodos, repeticiones, "", prelude)
}

/// Guion de un paquete con caché: `ROOT_CACHE` apunta a `cache/` del paquete, y su `config.toml` (y el
/// `--find-links`) se reescriben a esa ruta, porque la caché vino con rutas `/cache` del contenedor.
fn guion_con_cache(nodos: &[String], repeticiones: u32, lenguaje: &str) -> String {
    let previo = "ROOT_CACHE=\"$(pwd)/cache\"\n";
    guion_con_previo(nodos, repeticiones, previo, &preludio_de(lenguaje))
}

/// Preludio de la suite para el lenguaje, adaptado a la caché del paquete (`$ROOT_CACHE`). El
/// `config.toml` de cargo se reescribe a `CARGO_HOME` desde la copia intacta de la caché: así correr
/// `correr.sh` otra vez no vuelve a reescribir rutas ya reescritas.
fn preludio_de(lenguaje: &str) -> String {
    match lenguaje {
        "rust" => "export CARGO_NET_OFFLINE=true && export CARGO_HOME=/tmp/cargo-home && \
             mkdir -p \"$CARGO_HOME\" && sed \"s#/cache#$ROOT_CACHE#g\" \"$ROOT_CACHE/config.toml\" \
             > \"$CARGO_HOME/config.toml\" && "
            .to_string(),
        "python" => descarga::PRELUDIO_PYTHON.replace("/cache", "\"$ROOT_CACHE\""),
        _ => String::new(),
    }
}

fn guion_con_previo(nodos: &[String], repeticiones: u32, previo: &str, prelude: &str) -> String {
    let lista: Vec<String> = nodos.iter().map(|n| comillas(n)).collect();
    format!(
        "#!/usr/bin/env bash\n\
         set -euo pipefail\n\
         cd \"$(dirname \"$0\")\"\n\
         if [ -d bin ]; then export PATH=\"$(pwd)/bin:$PATH\"; fi\n\
         export ORACULO_REPO=\"$(pwd)/repo\"\n\
         export CARGO_TARGET_DIR=\"$(pwd)/target\"\n\
         {previo}\
         if unshare -rn true 2>/dev/null; then ORDEN=\"unshare -rn\"; RED=sin-red; else ORDEN=\"\"; RED=con-red; fi\n\
         echo \"$RED\" > red.txt\n\
         estado=0\n\
         for i in $(seq 1 {repeticiones}); do\n\
         {prelude}$ORDEN python3 -m pytest -q -p no:cacheprovider --junitxml=repeticion-$i.xml {nodos} || estado=$?\n\
         done\n\
         python3 -I - <<'PY'\n\
         {juntar}\
         PY\n\
         exit $estado\n",
        nodos = lista.join(" "),
        juntar = GUION_JUNTAR,
    )
}

/// Cinco líneas: qué hacer en Colab y cómo importar el resultado.
fn leeme(tarea: &str, repo: &Path) -> String {
    format!(
        "Tests pendientes de la tarea {tarea}, generados por Root.\n\
         1. Instala Rust (cargo) y pytest (pip install pytest).\n\
         2. Corre ./correr.sh desde esta carpeta.\n\
         3. Lleva pendientes.xml a la máquina donde corre Root.\n\
         4. Corre: r00t oraculo pendientes --tarea {tarea} --repo {} --importar {XML_PENDIENTES}\n",
        repo.display()
    )
}

fn escribir(ruta: &Path, texto: &str, ejecutable: bool) -> Result<(), ErrorOraculo> {
    fs::write(ruta, texto).map_err(|e| infra(&format!("escribir {}", ruta.display()), e))?;
    if ejecutable {
        fs::set_permissions(ruta, fs::Permissions::from_mode(0o755))
            .map_err(|e| infra("permisos de correr.sh", e))?;
    }
    Ok(())
}

fn sha256_hex(contenido: &[u8]) -> String {
    Sha256::digest(contenido)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn etiqueta(resultado: ResultadoTest) -> &'static str {
    match resultado {
        ResultadoTest::Pasa => "pasa",
        ResultadoTest::Falla => "falla",
        ResultadoTest::Omite => "omite",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oracle::tarea::{Comando, Exclusion};
    use std::process::Command;

    #[test]
    fn pendiente_etiqueta_de_resultado_en_minusculas() {
        assert_eq!(etiqueta(ResultadoTest::Pasa), "pasa");
        assert_eq!(etiqueta(ResultadoTest::Falla), "falla");
        assert_eq!(etiqueta(ResultadoTest::Omite), "omite");
    }

    #[test]
    fn pendiente_nodo_entrecomillado_conserva_corchetes() {
        let id = "test_x::test_rendimiento[gpu-10000-res1]";
        let nodo_pytest = nodo(id).expect("id válido");
        assert_eq!(
            comillas(&nodo_pytest),
            "'tests/test_x.py::test_rendimiento[gpu-10000-res1]'"
        );
    }

    #[test]
    fn pendiente_crear_no_escribe_fuera_de_home_con_nombre_de_tarea_con_ruta() {
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        let tarea_dir = tempfile::tempdir().expect("tarea");
        let texto = "nombre = \"../../fuera\"\nlenguaje = \"rust\"\nimagen = \"i\"\nspec = \"s\"\n\
                     [contrato]\ncmd = \"c\"\n[oculto]\ncmd = \"o\"\n\
                     [[entorno]]\ntest = \"a.py::t\"\nmotivo = \"m\"\n";
        std::fs::write(tarea_dir.path().join("tarea.toml"), texto).expect("escribe");
        let tarea = Tarea::cargar_de(tarea_dir.path()).expect("carga la tarea");
        let r = crear(home.path(), &tarea, repo.path(), None);
        let motivo = r.err().map(|e| e.to_string()).unwrap_or_default();
        assert!(motivo.contains("nombre inválido"), "{motivo}");
        assert!(!home.path().join("oraculos").exists());
    }

    #[test]
    fn pendiente_id_sin_separador_es_error_de_tarea() {
        assert!(matches!(nodo("sin_separador"), Err(ErrorOraculo::Tarea(_))));
    }

    #[test]
    fn pendiente_guion_correr_tiene_exports_pytest_y_los_nodos() {
        let nodos = vec!["tests/test_x.py::test_rendimiento[gpu-10000-res1]".to_string()];
        let texto = guion_correr(&nodos, 1, "");
        assert!(texto.starts_with("#!/usr/bin/env bash\n"));
        assert!(texto.contains("set -euo pipefail"));
        assert!(texto.contains("export ORACULO_REPO=\"$(pwd)/repo\""));
        assert!(texto.contains("export CARGO_TARGET_DIR=\"$(pwd)/target\""));
        assert!(
            texto.contains(
                "python3 -m pytest -q -p no:cacheprovider --junitxml=repeticion-$i.xml 'tests/test_x.py::test_rendimiento[gpu-10000-res1]'"
            ),
            "{texto}"
        );
    }

    #[test]
    fn pendiente_guion_correr_repite_la_suite_y_junta_los_junit() {
        let nodos = vec!["tests/test_x.py::test_a".to_string()];
        let texto = guion_correr(&nodos, 3, "");
        assert!(texto.contains("for i in $(seq 1 3); do"), "{texto}");
        assert!(texto.contains("|| estado=$?"), "{texto}");
        assert!(texto.contains("exit $estado"), "{texto}");
        assert!(texto.contains("python3 -I - <<'PY'"), "{texto}");
        assert!(texto.contains("pendientes.xml"), "{texto}");
    }

    #[test]
    fn pendiente_junit_combinado_trae_los_testcases_de_cada_repeticion() {
        let base = tempfile::tempdir().expect("base");
        let caso = |nombre: &str| {
            format!(
                "<testsuites><testsuite name=\"pytest\"><testcase classname=\"tests.test_x\" name=\"{nombre}\"/></testsuite></testsuites>"
            )
        };
        fs::write(base.path().join("repeticion-1.xml"), caso("test_a")).expect("rep 1");
        fs::write(base.path().join("repeticion-2.xml"), caso("test_a")).expect("rep 2");
        let juntar = base.path().join("juntar.py");
        fs::write(&juntar, GUION_JUNTAR).expect("escribe el guion");
        let estado = Command::new("python3")
            .arg("-I")
            .arg(&juntar)
            .current_dir(base.path())
            .status()
            .expect("python3");
        assert!(estado.success(), "{estado:?}");
        let texto = fs::read_to_string(base.path().join("pendientes.xml")).expect("junit");
        let leido = crate::oracle::junit::parsear(&texto).expect("parsea");
        assert_eq!(leido.testcases["tests.test_x::test_a"], 2, "{texto}");
    }

    #[test]
    fn pendiente_guion_correr_agrega_bin_al_path_si_existe() {
        let texto = guion_correr(&[], 1, "");
        assert!(
            texto.contains("if [ -d bin ]; then export PATH=\"$(pwd)/bin:$PATH\"; fi"),
            "{texto}"
        );
    }

    #[test]
    fn pendiente_nombre_de_tarea_con_ruta_se_rechaza() {
        for nombre in ["", ".", "..", "../otra", "a/b", "a\\b"] {
            assert!(
                matches!(
                    ruta_colab(Path::new("/h"), nombre),
                    Err(ErrorOraculo::Tarea(_))
                ),
                "debe rechazar {nombre:?}"
            );
        }
        assert!(ruta_colab(Path::new("/h"), "memora").is_ok());
    }

    #[test]
    fn pendiente_desde_marca_el_resultado_de_colab_por_test() {
        let tarea = Tarea {
            nombre: "memora".into(),
            lenguaje: "rust".into(),
            imagen: "x".into(),
            containerfile: None,
            spec: "s".into(),
            repeticiones: 1,
            repeticiones_inestable: 10,
            contrato: Comando {
                cmd: "x".into(),
                reintento: None,
            },
            oculto: Comando {
                cmd: "x".into(),
                reintento: None,
            },
            entorno: vec![
                Exclusion {
                    test: "test_a::uno".into(),
                    motivo: "gpu".into(),
                },
                Exclusion {
                    test: "test_a::dos".into(),
                    motivo: "gpu".into(),
                },
            ],
            oraculo_dudoso: Vec::new(),
            requisito: Vec::new(),
            arquitectura: None,
            eficiencia: None,
            pruebas: None,
            entrega: None,
            escenario: Vec::new(),
            dir: PathBuf::new(),
        };
        let mut colab = BTreeMap::new();
        colab.insert(
            "test_a::uno".to_string(),
            ColabEntrada {
                resultado: "pasa".into(),
                junit_sha256: "abc".into(),
                ts: 1.0,
                commit: String::new(),
            },
        );
        let pendientes = desde(&tarea, &colab);
        assert_eq!(pendientes.len(), 2);
        assert_eq!(pendientes[0].colab.as_deref(), Some("pasa"));
        assert_eq!(pendientes[1].colab, None);
        assert_eq!(pendientes[1].motivo, "gpu");
    }
    /// Tarea mínima en `home/oraculos/<nombre>`, con un test de `[[entorno]]` y su archivo oculto.
    fn tarea_en_oraculos(home: &Path, nombre: &str, lenguaje: &str) -> Tarea {
        let dir = home.join("oraculos").join(nombre);
        fs::create_dir_all(dir.join("oculto")).expect("oculto");
        fs::write(
            dir.join("oculto/test_a.py"),
            "def test_uno(): pass  # SECRETO-OCULTO\n",
        )
        .expect("escribe");
        let texto = format!(
            "nombre = \"{nombre}\"\nlenguaje = \"{lenguaje}\"\nimagen = \"i\"\nspec = \"s\"\n\
             [contrato]\ncmd = \"c\"\n[oculto]\ncmd = \"o\"\n\
             [[entorno]]\ntest = \"test_a::uno\"\nmotivo = \"gpu\"\n"
        );
        fs::write(dir.join("tarea.toml"), texto).expect("tarea.toml");
        Tarea::cargar_de(&dir).expect("carga")
    }

    fn contiene_secreto(dir: &Path) -> Vec<PathBuf> {
        let mut hallados = Vec::new();
        let mut pendientes = vec![dir.to_path_buf()];
        while let Some(actual) = pendientes.pop() {
            let Ok(entradas) = fs::read_dir(&actual) else {
                continue;
            };
            for entrada in entradas.flatten() {
                let ruta = entrada.path();
                if ruta.is_dir() {
                    pendientes.push(ruta);
                } else if fs::read_to_string(&ruta).is_ok_and(|t| t.contains("SECRETO-OCULTO")) {
                    hallados.push(ruta);
                }
            }
        }
        hallados
    }

    #[test]
    fn pendiente_paquete_vive_dentro_de_oraculos_y_no_fuera() {
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        fs::write(repo.path().join("a.txt"), "x").expect("repo");
        let tarea = tarea_en_oraculos(home.path(), "tarea-x", "shell");
        let paquete = crear(home.path(), &tarea, repo.path(), None)
            .expect("crea")
            .expect("hay paquete");
        assert!(
            paquete
                .dir
                .starts_with(home.path().join("oraculos/tarea-x/pendientes")),
            "{}",
            paquete.dir.display()
        );
        assert!(
            !home.path().join("pendientes").exists(),
            "no se usa la carpeta vieja pendientes/ de ROOT_HOME"
        );
        assert!(
            paquete.dir.join("tests/test_a.py").is_file(),
            "el paquete trae el oculto"
        );
    }

    #[test]
    fn pendiente_borrar_paquete_y_tmp_no_deja_tests_ocultos_fuera_de_oraculos() {
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        fs::write(repo.path().join("a.txt"), "x").expect("repo");
        let tarea = tarea_en_oraculos(home.path(), "tarea-x", "shell");
        let paquete = crear(home.path(), &tarea, repo.path(), None)
            .expect("crea")
            .expect("paquete");
        // El junit de Colab que se baja a tmp/colab-<sesion> también es material oculto.
        let tmp = home.path().join("tmp/colab-sesion");
        fs::create_dir_all(&tmp).expect("tmp");
        fs::write(tmp.join("pendientes.xml"), "SECRETO-OCULTO en una falla").expect("xml");
        assert!(
            contiene_secreto(home.path()).len() >= 2,
            "antes de limpiar hay material oculto en el paquete y en tmp"
        );
        limpiar_paquete(home.path(), &tarea, &paquete.dir, "sesion").expect("limpia");
        let fuera: Vec<PathBuf> = contiene_secreto(home.path())
            .into_iter()
            .filter(|r| !r.starts_with(home.path().join("oraculos")))
            .collect();
        assert!(
            fuera.is_empty(),
            "queda material oculto fuera de oraculos: {fuera:?}"
        );
    }

    #[test]
    fn pendiente_importar_ignora_resultado_de_otro_commit() {
        let home = tempfile::tempdir().expect("home");
        let tarea = tarea_en_oraculos(home.path(), "tarea-x", "shell");
        let paquete = home.path().join("oraculos/tarea-x/pendientes");
        fs::create_dir_all(&paquete).expect("paquete");
        fs::write(paquete.join(ARCHIVO_COMMIT), "aaa").expect("commit");
        let junit = home.path().join("pendientes.xml");
        fs::write(
            &junit,
            r#"<testsuite><testcase classname="tests.test_a" name="uno"/></testsuite>"#,
        )
        .expect("junit");
        let otro = importar(home.path(), &tarea, &junit, "bbb").expect("importa");
        assert_eq!(
            otro.importados, 0,
            "el resultado de otro commit no se importa"
        );
        assert!(
            otro.avisos.iter().any(|a| a.contains("commit")),
            "{:?}",
            otro.avisos
        );
        assert!(colab_de(home.path(), "tarea-x").expect("lee").is_empty());
        let mismo = importar(home.path(), &tarea, &junit, "aaa").expect("importa");
        assert_eq!(mismo.importados, 1);
        assert_eq!(
            colab_de(home.path(), "tarea-x").expect("lee")["test_a::uno"].commit,
            "aaa"
        );
    }

    fn python3_real() -> PathBuf {
        let path = std::env::var_os("PATH").unwrap_or_default();
        std::env::split_paths(&path)
            .map(|d| d.join("python3"))
            .find(|p| p.is_file())
            .expect("hay python3")
    }

    /// Arma un paquete con `correr.sh` y un `bin/` con `unshare` y `python3` falsos.
    fn paquete_con_falsos(base: &Path, unshare: &str) -> PathBuf {
        let dir = base.join("paquete");
        let bin = dir.join("bin");
        fs::create_dir_all(&bin).expect("bin");
        fs::write(
            dir.join("correr.sh"),
            guion_correr(&["tests/test_a.py::t1".into()], 1, ""),
        )
        .expect("sh");
        fs::write(bin.join("unshare"), unshare).expect("unshare");
        let fake_python = format!(
            "#!/bin/sh\n\
             case \"$*\" in\n\
               *\"-m pytest\"*)\n\
                 for a in \"$@\"; do case \"$a\" in --junitxml=*) xml=\"${{a#--junitxml=}}\";; esac; done\n\
                 printf '<testsuite><testcase classname=\"tests.test_a\" name=\"t1\"/></testsuite>' > \"$xml\"\n\
                 exit 0;;\n\
             esac\n\
             exec {} \"$@\"\n",
            python3_real().display()
        );
        fs::write(bin.join("python3"), fake_python).expect("python3");
        for f in ["unshare", "python3"] {
            fs::set_permissions(bin.join(f), fs::Permissions::from_mode(0o755)).expect("perm");
        }
        dir
    }

    const UNSHARE_FALLA: &str = "#!/bin/sh\nexit 1\n";
    const UNSHARE_SANO: &str =
        "#!/bin/sh\nwhile [ \"${1#-}\" != \"$1\" ]; do shift; done\nexec \"$@\"\n";

    fn correr_paquete(dir: &Path) -> (i32, String) {
        let salida = Command::new("bash")
            .arg(dir.join("correr.sh"))
            .current_dir(dir)
            .output()
            .expect("bash");
        (
            salida.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&salida.stderr).into_owned(),
        )
    }

    #[test]
    fn pendiente_correr_sh_con_unshare_que_falla_sigue_y_deja_marca_con_red() {
        let base = tempfile::tempdir().expect("base");
        let dir = paquete_con_falsos(base.path(), UNSHARE_FALLA);
        let (codigo, error) = correr_paquete(&dir);
        assert_eq!(codigo, 0, "la corrida sigue: {error}");
        assert_eq!(
            fs::read_to_string(dir.join("red.txt"))
                .expect("marca")
                .trim(),
            "con-red"
        );
        let xml = fs::read_to_string(dir.join("pendientes.xml")).expect("junit combinado");
        let leido = junit::parsear(&xml).expect("parsea");
        assert_eq!(leido.red.as_deref(), Some("con-red"));
    }

    #[test]
    fn pendiente_correr_sh_con_unshare_sano_deja_marca_sin_red() {
        let base = tempfile::tempdir().expect("base");
        let dir = paquete_con_falsos(base.path(), UNSHARE_SANO);
        let (codigo, error) = correr_paquete(&dir);
        assert_eq!(codigo, 0, "{error}");
        assert_eq!(
            fs::read_to_string(dir.join("red.txt"))
                .expect("marca")
                .trim(),
            "sin-red"
        );
        let xml = fs::read_to_string(dir.join("pendientes.xml")).expect("junit");
        assert_eq!(
            junit::parsear(&xml).expect("parsea").red.as_deref(),
            Some("sin-red")
        );
    }

    #[test]
    fn pendiente_paquete_con_cache_copia_dependencias_y_aisla_la_corrida() {
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        fs::write(repo.path().join("a.txt"), "x").expect("repo");
        let tarea = tarea_en_oraculos(home.path(), "tarea-x", "rust");
        let cache = tempfile::tempdir().expect("cache");
        fs::create_dir_all(cache.path().join("vendor/serde")).expect("vendor");
        fs::write(
            cache.path().join("config.toml"),
            "[source.x]\ndirectory = \"/cache/vendor\"\n",
        )
        .expect("config");
        let paquete = crear(home.path(), &tarea, repo.path(), Some(cache.path()))
            .expect("crea")
            .expect("paquete");
        assert!(
            paquete.dir.join("cache/config.toml").is_file(),
            "copia la caché"
        );
        assert!(
            paquete.dir.join("cache/vendor/serde").is_dir(),
            "copia el vendor"
        );
        let sh = fs::read_to_string(paquete.dir.join("correr.sh")).expect("correr.sh");
        assert!(sh.contains("unshare -rn"), "{sh}");
        assert!(sh.contains("ROOT_CACHE=\"$(pwd)/cache\""), "{sh}");
        assert!(sh.contains("export CARGO_HOME=/tmp/cargo-home"), "{sh}");
        assert!(
            sh.contains("sed \"s#/cache#$ROOT_CACHE#g\" \"$ROOT_CACHE/config.toml\""),
            "{sh}"
        );
        assert!(
            !sh.contains("sed -i"),
            "la caché intacta no se reescribe en el lugar: {sh}"
        );
        assert!(
            !sh.contains("cp /cache/"),
            "la caché del paquete no apunta a /cache: {sh}"
        );
        assert_eq!(paquete.commit, "", "el repo de prueba no es git");
        let sintaxis = Command::new("bash")
            .arg("-n")
            .arg(paquete.dir.join("correr.sh"))
            .output()
            .expect("bash");
        assert!(
            sintaxis.status.success(),
            "{}",
            String::from_utf8_lossy(&sintaxis.stderr)
        );
    }

    #[test]
    fn pendiente_junit_combinado_lleva_la_marca_de_red() {
        let base = tempfile::tempdir().expect("base");
        fs::write(base.path().join("red.txt"), "con-red\n").expect("red");
        fs::write(
            base.path().join("repeticion-1.xml"),
            r#"<testsuites><testsuite name="pytest"><testcase classname="tests.test_a" name="t1"/></testsuite></testsuites>"#,
        )
        .expect("rep");
        let juntar = base.path().join("juntar.py");
        fs::write(&juntar, GUION_JUNTAR).expect("guion");
        let estado = Command::new("python3")
            .arg("-I")
            .arg(&juntar)
            .current_dir(base.path())
            .status()
            .expect("py");
        assert!(estado.success());
        let texto = fs::read_to_string(base.path().join("pendientes.xml")).expect("xml");
        assert_eq!(
            junit::parsear(&texto).expect("parsea").red.as_deref(),
            Some("con-red")
        );
    }
}
