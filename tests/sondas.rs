//! Sondas de herramientas contra podman real, sin red. Las imágenes deben existir localmente.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use root::oracle::perfiles::herramientas::{
    EjecutorContenedor, EjecutorHerramienta, cargo_audit, clippy, eficiencia, gitleaks,
};
use root::oracle::tarea::Tarea;
use root::tipos::Dimension;

const ALPINE: &str = "docker.io/library/alpine:3.20";
/// Imagen de evaluación de Rust. Hoy no trae gitleaks, cargo-audit ni el componente clippy.
const RUST: &str = "localhost/root-oraculo-rust:1";
const LIMITE: Duration = Duration::from_secs(120);

/// Falla con un mensaje claro si la imagen no está. Nunca la baja: el test no tiene red.
fn asegurar_imagen(imagen: &str) {
    let existe = Command::new("podman")
        .args(["image", "exists", imagen])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    assert!(existe, "falta la imagen {imagen} en podman");
}

fn ejecutor(imagen: &str, repo: &Path, rustsec: Option<&Path>) -> EjecutorContenedor {
    EjecutorContenedor {
        imagen: imagen.into(),
        repo: repo.into(),
        oraculo: None,
        rustsec: rustsec.map(Path::to_path_buf),
        cache: None,
        timeout: LIMITE,
    }
}

/// Tarea de Rust. Con `eficiencia`, define su `cmd` con piso 5000.
fn tarea_rust(eficiencia: Option<&str>) -> Tarea {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut texto = String::from(
        "nombre = \"x\"\nlenguaje = \"rust\"\nimagen = \"i\"\nspec = \"s\"\n\
         [contrato]\ncmd = \"c\"\n[oculto]\ncmd = \"o\"\n",
    );
    if let Some(cmd) = eficiencia {
        texto.push_str(&format!("[eficiencia]\ncmd = '{cmd}'\npiso = 5000.0\n"));
    }
    fs::write(dir.path().join("tarea.toml"), texto).expect("escribe tarea.toml");
    Tarea::cargar_de(dir.path()).expect("carga la tarea")
}

/// Repo mínimo de Rust con su `Cargo.lock`.
fn repo_minimo() -> tempfile::TempDir {
    let repo = tempfile::tempdir().expect("temp dir");
    fs::create_dir_all(repo.path().join("src")).expect("crea src");
    fs::write(repo.path().join("src/lib.rs"), "pub fn f() -> u8 { 1 }\n").expect("escribe");
    fs::write(
        repo.path().join("Cargo.toml"),
        "[package]\nname = \"x\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("escribe Cargo.toml");
    fs::write(
        repo.path().join("Cargo.lock"),
        "version = 4\n\n[[package]]\nname = \"x\"\nversion = \"0.1.0\"\n",
    )
    .expect("escribe Cargo.lock");
    repo
}

#[test]
fn sonda_ejecutor_real_corre_un_comando_y_devuelve_stdout() {
    asegurar_imagen(ALPINE);
    let repo = repo_minimo();
    let ej = ejecutor(ALPINE, repo.path(), None);
    let s = ej.ejecutar("echo hola").expect("corre");
    assert_eq!(s.codigo, 0);
    assert_eq!(s.stdout.trim(), "hola");
}

#[test]
fn sonda_ejecutor_real_devuelve_los_archivos_de_salida() {
    asegurar_imagen(ALPINE);
    let repo = repo_minimo();
    let ej = ejecutor(ALPINE, repo.path(), None);
    let s = ej
        .ejecutar("echo '{\"ops_s\": 1}' > /salida/x.json")
        .expect("corre");
    assert_eq!(
        s.archivos.get("x.json").map(|t| t.trim()),
        Some("{\"ops_s\": 1}")
    );
}

#[test]
fn sonda_ejecutor_real_codigo_distinto_de_cero_no_es_error() {
    asegurar_imagen(ALPINE);
    let repo = repo_minimo();
    let ej = ejecutor(ALPINE, repo.path(), None);
    let s = ej
        .ejecutar("exit 3")
        .expect("el código 3 no es error de infra");
    assert_eq!(s.codigo, 3);
}

#[test]
fn sonda_ejecutor_real_corre_sin_red() {
    asegurar_imagen(ALPINE);
    let repo = repo_minimo();
    let ej = ejecutor(ALPINE, repo.path(), None);
    // Sin red el contenedor solo ve la interfaz `lo`.
    let s = ej
        .ejecutar("awk -F: 'NR>2{print $1}' /proc/net/dev | tr -d ' '")
        .expect("corre");
    assert_eq!(s.stdout.trim(), "lo");
}

#[test]
fn sonda_real_sin_herramientas_en_la_imagen_rust_no_hay_veredicto() {
    asegurar_imagen(RUST);
    let repo = repo_minimo();
    let rustsec = tempfile::tempdir().expect("temp dir");
    let ej = ejecutor(RUST, repo.path(), Some(rustsec.path()));
    let tarea = tarea_rust(None);
    let g = gitleaks(&ej, &tarea);
    let a = cargo_audit(&ej, &tarea, repo.path());
    let c = clippy(&ej, &tarea);
    // Hoy la imagen no trae gitleaks, cargo-audit ni clippy. Si algún día las trae, este test
    // se adapta: el motivo ya no dirá "no está en la imagen de evaluación".
    for r in [&g, &a, &c] {
        assert_eq!(r.valor, None);
        let motivo = r.motivo.clone().unwrap_or_default();
        assert!(
            motivo.contains("no está en la imagen de evaluación"),
            "motivo: {motivo}"
        );
    }
    assert_eq!(g.dimension, Dimension::Seguridad);
    assert_eq!(a.dimension, Dimension::Seguridad);
    assert_eq!(c.dimension, Dimension::Estructura);
}

#[test]
fn sonda_real_eficiencia_en_alpine_da_valor_uno() {
    asegurar_imagen(ALPINE);
    let repo = repo_minimo();
    let ej = ejecutor(ALPINE, repo.path(), None);
    let tarea = tarea_rust(Some(r#"echo "{\"ops_s\": 9000}""#));
    let r = eficiencia(&ej, &tarea);
    assert_eq!(r.motivo, None);
    assert_eq!(r.valor, Some(1.0));
    assert!(r.hallazgos.is_empty());
}
