//! Tests pendientes por entorno: el paquete para correrlos en Colab y la importación de su junit.
//! Root no lanza Colab. El usuario corre `correr.sh` y trae el junit de vuelta.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use super::junit::{self, ResultadoTest};
use super::tarea::Tarea;
use super::{ErrorOraculo, comillas, copiar_repo, infra};
use crate::observe;
use crate::tipos::{Pendiente, TipoEvento};

/// Carpeta de los paquetes dentro de `ROOT_HOME`.
const CARPETA_PAQUETES: &str = "pendientes";
/// Carpeta de los resultados importados dentro de `ROOT_HOME`.
const CARPETA_EVIDENCIA: &str = "evidencia";
/// Resultados de Colab de una tarea, dentro de su carpeta de evidencia.
const ARCHIVO_COLAB: &str = "colab.json";
/// Eventos de Root dentro de `ROOT_HOME`.
const ARCHIVO_EVENTOS: &str = "eventos.jsonl";
/// Junit que deja `correr.sh` al lado del paquete.
const XML_PENDIENTES: &str = "pendientes.xml";

/// Resultado de un test pendiente importado de Colab.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColabEntrada {
    /// "pasa", "falla" u "omite".
    pub resultado: String,
    /// Sha256 del junit del que salió el resultado.
    pub junit_sha256: String,
    /// Momento de la importación, en segundos desde la época.
    pub ts: f64,
}

/// Paquete armado para correr los tests pendientes.
#[derive(Debug, Clone, PartialEq)]
pub struct Paquete {
    pub dir: PathBuf,
    /// Ids `archivo::nombre` de los tests del paquete.
    pub tests: Vec<String>,
}

/// Resumen de una importación de junit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Importacion {
    /// Tests de `[[entorno]]` que estaban en el junit.
    pub importados: usize,
    /// Tests del junit que no están en `[[entorno]]`.
    pub ignorados: usize,
}

/// Arma el paquete de los tests de `[[entorno]]` dentro de `home/pendientes/`.
/// Sin tests pendientes no crea nada y da `None`. Todo se valida antes de crear el directorio.
pub fn crear(home: &Path, tarea: &Tarea, repo: &Path) -> Result<Option<Paquete>, ErrorOraculo> {
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

    let segundos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let dir = home
        .join(CARPETA_PAQUETES)
        .join(format!("{}-{segundos}", tarea.nombre));
    fs::create_dir_all(home.join(CARPETA_PAQUETES)).map_err(|e| infra("carpeta de paquetes", e))?;
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
    escribir(&dir.join("correr.sh"), &guion_correr(&nodos), true)?;
    escribir(&dir.join("LEEME.txt"), &leeme(&tarea.nombre, &repo), false)?;
    Ok(Some(Paquete { dir, tests }))
}

/// Importa el junit que dejó `correr.sh`. Guarda el resultado de cada test pendiente
/// en `evidencia/<tarea>/colab.json` y registra un evento `Gate` con origen `colab`.
pub fn importar(home: &Path, tarea: &Tarea, ruta: &Path) -> Result<Importacion, ErrorOraculo> {
    let texto = fs::read_to_string(ruta)
        .map_err(|e| ErrorOraculo::Infra(format!("no se pudo leer {}: {e}", ruta.display())))?;
    let junit = junit::parsear(&texto)?;
    let sha = sha256_hex(texto.as_bytes());
    let (ts, _) = observe::ahora();

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
                },
            );
            resultados.insert(exclusion.test.clone(), etiqueta);
        }
    }
    let importados = resultados.len();
    let ignorados = junit.tests.len().saturating_sub(importados);
    guardar_colab(home, &tarea.nombre, &colab)?;

    let mut evento = observe::evento_manual(
        &format!("pendientes-{}", tarea.nombre),
        TipoEvento::Gate,
        None,
    );
    evento.fase = "pendientes".into();
    evento.extra.insert("origen".into(), json!("colab"));
    evento.extra.insert("tests".into(), json!(resultados));
    evento.extra.insert("junit_sha256".into(), json!(sha));
    observe::registrar_en(&home.join(ARCHIVO_EVENTOS), &evento)
        .map_err(|e| infra("registrar el evento", e))?;
    Ok(Importacion {
        importados,
        ignorados,
    })
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
    let con_ruta =
        nombre.is_empty() || nombre.contains(['/', '\\']) || nombre == "." || nombre == "..";
    if con_ruta {
        return Err(ErrorOraculo::Tarea(format!("nombre inválido: {nombre:?}")));
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

/// Script que corre los nodos pendientes y deja `pendientes.xml` al lado.
/// Si hay una carpeta `bin/` al lado (el binario de respaldo de Colab), la pone al frente del
/// `PATH`. Cómo usan los tests ocultos ese binario en lugar de `cargo` es una pregunta abierta
/// para el planner.
fn guion_correr(nodos: &[String]) -> String {
    let lista: Vec<String> = nodos.iter().map(|n| comillas(n)).collect();
    format!(
        "#!/usr/bin/env bash\n\
         set -euo pipefail\n\
         cd \"$(dirname \"$0\")\"\n\
         if [ -d bin ]; then export PATH=\"$(pwd)/bin:$PATH\"; fi\n\
         export ORACULO_REPO=\"$(pwd)/repo\"\n\
         export CARGO_TARGET_DIR=\"$(pwd)/target\"\n\
         python3 -m pytest -q -p no:cacheprovider --junitxml={XML_PENDIENTES} {}\n",
        lista.join(" ")
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
    fn pendiente_id_sin_separador_es_error_de_tarea() {
        assert!(matches!(nodo("sin_separador"), Err(ErrorOraculo::Tarea(_))));
    }

    #[test]
    fn pendiente_guion_correr_tiene_exports_pytest_y_los_nodos() {
        let nodos = vec!["tests/test_x.py::test_rendimiento[gpu-10000-res1]".to_string()];
        let texto = guion_correr(&nodos);
        assert!(texto.starts_with("#!/usr/bin/env bash\n"));
        assert!(texto.contains("set -euo pipefail"));
        assert!(texto.contains("export ORACULO_REPO=\"$(pwd)/repo\""));
        assert!(texto.contains("export CARGO_TARGET_DIR=\"$(pwd)/target\""));
        assert!(
            texto.contains("python3 -m pytest -q -p no:cacheprovider --junitxml=pendientes.xml")
        );
        assert!(texto.contains("'tests/test_x.py::test_rendimiento[gpu-10000-res1]'"));
    }

    #[test]
    fn pendiente_guion_correr_agrega_bin_al_path_si_existe() {
        let texto = guion_correr(&[]);
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
            },
        );
        let pendientes = desde(&tarea, &colab);
        assert_eq!(pendientes.len(), 2);
        assert_eq!(pendientes[0].colab.as_deref(), Some("pasa"));
        assert_eq!(pendientes[1].colab, None);
        assert_eq!(pendientes[1].motivo, "gpu");
    }
}
