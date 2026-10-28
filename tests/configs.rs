//! Sondas de configuración (Tarea 9b) por la API pública. Sin podman ni red.

use std::cell::RefCell;
use std::fs;
use std::path::Path;

use root::oracle::ErrorOraculo;
use root::oracle::configs::sondas_configs;
use root::oracle::perfiles::herramientas::{EjecutorHerramienta, SalidaHerramienta};
use root::oracle::tarea::Tarea;
use root::tipos::Dimension;

/// Ejecutor de mentira: guarda cada comando y responde con la función dada.
struct Falso {
    cmds: RefCell<Vec<String>>,
    responde: Box<dyn Fn(&str) -> SalidaHerramienta>,
}

impl Falso {
    fn nuevo(responde: impl Fn(&str) -> SalidaHerramienta + 'static) -> Self {
        Falso {
            cmds: RefCell::new(Vec::new()),
            responde: Box::new(responde),
        }
    }

    fn cmds(&self) -> Vec<String> {
        self.cmds.borrow().clone()
    }
}

impl EjecutorHerramienta for Falso {
    fn ejecutar(&self, cmd: &str) -> Result<SalidaHerramienta, ErrorOraculo> {
        self.cmds.borrow_mut().push(cmd.to_string());
        Ok((self.responde)(cmd))
    }
}

fn salida(codigo: i32, stdout: &str) -> SalidaHerramienta {
    SalidaHerramienta {
        codigo,
        stdout: stdout.into(),
        ..Default::default()
    }
}

/// Responde "presente" a `command -v hadolint`. Al ejecutar hadolint da `stdout`.
fn con_hadolint(stdout: &'static str) -> Falso {
    Falso::nuevo(move |cmd| {
        if cmd.starts_with("command -v") {
            salida(0, "")
        } else {
            salida(1, stdout)
        }
    })
}

/// Responde "ausente" a las comprobaciones de herramienta.
fn sin_hadolint() -> Falso {
    Falso::nuevo(|_| salida(1, ""))
}

fn escribir(repo: &Path, ruta: &str, texto: &str) {
    let archivo = repo.join(ruta);
    fs::create_dir_all(archivo.parent().expect("con padre")).expect("crea carpeta");
    fs::write(archivo, texto).expect("escribe el archivo");
}

/// Tarea mínima de un lenguaje dado. Los configs no dependen del lenguaje.
fn tarea_de(lenguaje: &str) -> Tarea {
    let dir = tempfile::tempdir().expect("temp dir");
    let texto = format!(
        "nombre = \"x\"\nlenguaje = \"{lenguaje}\"\nimagen = \"i\"\nspec = \"s\"\n\
         [contrato]\ncmd = \"c\"\n[oculto]\ncmd = \"o\"\n"
    );
    fs::write(dir.path().join("tarea.toml"), texto).expect("escribe tarea.toml");
    Tarea::cargar_de(dir.path()).expect("carga la tarea")
}

fn dimensiones(rs: &[root::oracle::perfiles::ResultadoSonda]) -> Vec<Dimension> {
    rs.iter().map(|r| r.dimension).collect()
}

#[test]
fn configs_hadolint_sin_dockerfile_no_ejecuta_nada() {
    let repo = tempfile::tempdir().expect("temp dir");
    escribir(repo.path(), "a.toml", "x = 1\n");
    let ej = sin_hadolint();
    let rs = sondas_configs(repo.path(), &ej, &tarea_de("python"));
    assert_eq!(
        dimensiones(&rs),
        [
            Dimension::Estructura,
            Dimension::Seguridad,
            Dimension::Seguridad,
            Dimension::Entrega
        ]
    );
    assert!(ej.cmds().is_empty(), "no corre nada: {:?}", ej.cmds());
    assert_eq!(rs[3].valor, Some(1.0));
    assert!(rs[3].hallazgos.is_empty());
}

#[test]
fn configs_hadolint_con_dockerfile_y_herramienta_ausente_es_sin_veredicto() {
    let repo = tempfile::tempdir().expect("temp dir");
    escribir(
        repo.path(),
        "Dockerfile",
        "FROM debian:bookworm-slim\nUSER app\n",
    );
    let ej = sin_hadolint();
    let rs = sondas_configs(repo.path(), &ej, &tarea_de("rust"));
    assert_eq!(rs[3].valor, None);
    assert!(
        rs[3]
            .motivo
            .clone()
            .unwrap_or_default()
            .contains("hadolint")
    );
    assert_eq!(ej.cmds(), ["command -v hadolint"]);
}

#[test]
fn configs_hadolint_con_herramienta_da_error_sev_2_y_warning_sev_1() {
    let json = r#"[
        {"code":"DL3006","level":"error","line":1,"message":"m","file":"/repo/Dockerfile"},
        {"code":"DL3008","level":"warning","line":2,"message":"w","file":"/repo/Dockerfile"}
    ]"#;
    let repo = tempfile::tempdir().expect("temp dir");
    escribir(
        repo.path(),
        "Dockerfile",
        "FROM debian:bookworm-slim\nRUN x\nUSER app\n",
    );
    let ej = con_hadolint(json);
    let rs = sondas_configs(repo.path(), &ej, &tarea_de("python"));
    let hs = &rs[3].hallazgos;
    assert!(rs[3].valor.is_some());
    assert_eq!(hs.len(), 2);
    assert_eq!((hs[0].severidad, hs[0].cita.as_str()), (2, "DL3006"));
    assert_eq!((hs[1].severidad, hs[1].cita.as_str()), (1, "DL3008"));
    assert_eq!(hs[0].archivo, "Dockerfile");
    assert_eq!(hs[0].dimension, Dimension::Entrega);
    assert!(
        ej.cmds()
            .iter()
            .any(|c| c == "hadolint --format json /repo/Dockerfile")
    );
}

#[test]
fn configs_hadolint_json_roto_es_sin_veredicto() {
    let repo = tempfile::tempdir().expect("temp dir");
    escribir(
        repo.path(),
        "Dockerfile",
        "FROM debian:bookworm-slim\nUSER app\n",
    );
    let ej = con_hadolint("esto no es json");
    let rs = sondas_configs(repo.path(), &ej, &tarea_de("python"));
    assert_eq!(rs[3].valor, None);
    assert!(rs[3].motivo.is_some());
}

#[test]
fn configs_repo_sin_archivos_de_configuracion_no_recibe_penalizacion() {
    let repo = tempfile::tempdir().expect("temp dir");
    escribir(repo.path(), "src/main.rs", "fn main() {}\n");
    let ej = sin_hadolint();
    let rs = sondas_configs(repo.path(), &ej, &tarea_de("rust"));
    assert_eq!(rs.len(), 4);
    assert!(
        rs.iter()
            .all(|r| r.valor == Some(1.0) && r.hallazgos.is_empty())
    );
    assert!(ej.cmds().is_empty());
}

#[test]
fn configs_ignora_archivos_dentro_de_target_y_git() {
    let repo = tempfile::tempdir().expect("temp dir");
    escribir(repo.path(), "target/Dockerfile", "FROM python:latest\n");
    escribir(repo.path(), "target/x.toml", "password = \"hunter2\"\n");
    escribir(repo.path(), ".git/y.toml", "roto = \n");
    escribir(repo.path(), ".git/Dockerfile", "FROM python:latest\n");
    let ej = sin_hadolint();
    let rs = sondas_configs(repo.path(), &ej, &tarea_de("rust"));
    assert!(
        rs.iter()
            .all(|r| r.valor == Some(1.0) && r.hallazgos.is_empty())
    );
    assert!(ej.cmds().is_empty());
}

#[test]
fn configs_dockerfile_python_latest_sin_user_da_un_hallazgo_por_la_sonda() {
    let repo = tempfile::tempdir().expect("temp dir");
    escribir(repo.path(), "Dockerfile", "FROM python:latest\n");
    let ej = sin_hadolint();
    let rs = sondas_configs(repo.path(), &ej, &tarea_de("python"));
    assert_eq!(rs[2].hallazgos.len(), 1, "solo la imagen sin versión");
}

#[test]
fn configs_yaml_con_tabulacion_y_toml_roto_por_la_sonda_de_estructura() {
    let repo = tempfile::tempdir().expect("temp dir");
    escribir(repo.path(), "a.yaml", "a:\n  b: 1\nc:\n\td: 2\n");
    escribir(repo.path(), "b.toml", "x = 1\ny = \"abc\nz = 2\n");
    let ej = sin_hadolint();
    let rs = sondas_configs(repo.path(), &ej, &tarea_de("python"));
    let hs = &rs[0].hallazgos;
    assert_eq!(hs.len(), 2);
    assert!(hs.iter().any(|h| h.archivo == "a.yaml" && h.linea == 4));
    assert!(hs.iter().any(|h| h.archivo == "b.toml" && h.linea == 2));
}

#[test]
fn configs_secreto_en_toml_y_yaml_por_la_sonda_de_secretos_sin_el_valor() {
    let repo = tempfile::tempdir().expect("temp dir");
    escribir(repo.path(), "a.toml", "password = \"hunter2-valor\"\n");
    escribir(repo.path(), "b.yaml", "api_key: abc123-valor\n");
    let ej = sin_hadolint();
    let rs = sondas_configs(repo.path(), &ej, &tarea_de("python"));
    assert_eq!(rs[1].hallazgos.len(), 2);
    let json = serde_json::to_string(&rs[1].hallazgos).expect("serializa");
    assert!(!json.contains("hunter2-valor"));
    assert!(!json.contains("abc123-valor"));
}
