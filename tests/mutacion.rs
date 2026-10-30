//! Sonda de mutación (Tarea 9e). Los fixtures validan el puntaje; la prueba de contenedor corre
//! `sonda_mutacion` contra podman real, sin red. La imagen debe existir localmente.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use root::oracle::mutacion::{parsear_outcomes, sonda_mutacion};
use root::oracle::perfiles::herramientas::EjecutorContenedor;
use root::oracle::tarea::Tarea;

/// Imagen de evaluación de Rust. Hoy no trae `cargo-mutants`.
const RUST: &str = "localhost/root-oraculo-rust:1";
const LIMITE: Duration = Duration::from_secs(120);

fn fixture(nombre: &str) -> String {
    let ruta = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/mutacion")
        .join(nombre);
    fs::read_to_string(&ruta).unwrap_or_else(|e| panic!("lee {}: {e}", ruta.display()))
}

/// Falla con un mensaje claro si la imagen no está. Nunca la baja: el test no tiene red.
fn asegurar_imagen(imagen: &str) {
    let existe = Command::new("podman")
        .args(["image", "exists", imagen])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    assert!(existe, "falta la imagen {imagen} en podman");
}

/// Tarea de Rust mínima, con su `tarea.toml` en una carpeta temporal.
fn tarea_rust(dir: &Path) -> Tarea {
    fs::write(
        dir.join("tarea.toml"),
        "nombre = \"x\"\nlenguaje = \"rust\"\nimagen = \"i\"\nspec = \"s\"\n\
         [contrato]\ncmd = \"c\"\n[oculto]\ncmd = \"o\"\n",
    )
    .expect("escribe tarea.toml");
    Tarea::cargar_de(dir).expect("carga la tarea")
}

#[test]
fn mutacion_fixture_sin_asserts_puntaje_menor_que_con_asserts() {
    let con = parsear_outcomes(&fixture("outcomes-con-asserts.json"), 60, &[]).expect("parsea");
    let sin = parsear_outcomes(&fixture("outcomes-sin-asserts.json"), 60, &[]).expect("parsea");
    let valor_con = con.sonda.valor.expect("valor con asserts");
    let valor_sin = sin.sonda.valor.expect("valor sin asserts");
    assert_eq!(valor_con, 1.0);
    assert!(
        valor_sin < valor_con,
        "{valor_sin} debe ser menor que {valor_con}"
    );
    assert!((valor_sin - 2.0 / 9.0).abs() < 1e-6, "valor {valor_sin}");
    assert_eq!(sin.sonda.hallazgos.len(), 7);
    assert!(con.sonda.hallazgos.is_empty());
}

#[test]
fn mutacion_contenedor_sin_cargo_mutants_da_sin_veredicto() {
    root::saltar_sin_podman!();
    asegurar_imagen(RUST);
    let repo = tempfile::tempdir().expect("temp dir");
    fs::create_dir_all(repo.path().join("src")).expect("crea src");
    fs::write(repo.path().join("src/lib.rs"), "pub fn f() -> u8 { 1 }\n").expect("escribe");
    fs::write(
        repo.path().join("Cargo.toml"),
        "[package]\nname = \"x\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("escribe Cargo.toml");
    let carpeta = tempfile::tempdir().expect("temp dir");
    let tarea = tarea_rust(carpeta.path());
    let ej = EjecutorContenedor {
        imagen: RUST.into(),
        repo: repo.path().into(),
        oraculo: None,
        rustsec: None,
        cache: None,
        timeout: LIMITE,
        home: carpeta.path().into(),
    };
    let r = sonda_mutacion(&ej, &tarea);
    // Hoy la imagen no trae cargo-mutants. Si algún día la trae, este test debe cambiar:
    // el motivo ya no dirá "no está en la imagen de evaluación".
    assert_eq!(r.sonda.valor, None);
    assert_eq!(
        r.sonda.motivo.as_deref(),
        Some("cargo-mutants no está en la imagen de evaluación")
    );
}

/// Imagen con `cargo-mutants` (la `:2`). Solo para la prueba real de mutación sin mutantes.
const RUST_CON_MUTANTS: &str = "localhost/root-oraculo-rust:2";

#[test]
fn mutacion_real_sin_mutantes_no_deja_reporte_y_da_sin_veredicto() {
    root::saltar_sin_podman!();
    asegurar_imagen(RUST_CON_MUTANTS);
    // Un repo sin funciones ni expresiones que mutar: cargo-mutants no escribe outcomes.json.
    let repo = tempfile::tempdir().expect("temp dir");
    fs::create_dir_all(repo.path().join("src")).expect("crea src");
    fs::write(repo.path().join("src/lib.rs"), "pub struct S;\n").expect("escribe");
    fs::write(
        repo.path().join("Cargo.toml"),
        "[package]\nname = \"x\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("escribe Cargo.toml");
    // El preludio copia /cache/config.toml: sin él, el comando no corre y el test mentiría.
    let cache = tempfile::tempdir().expect("temp dir");
    fs::write(cache.path().join("config.toml"), "").expect("escribe config.toml");
    let carpeta = tempfile::tempdir().expect("temp dir");
    let tarea = tarea_rust(carpeta.path());
    let ej = EjecutorContenedor {
        imagen: RUST_CON_MUTANTS.into(),
        repo: repo.path().into(),
        oraculo: None,
        rustsec: None,
        cache: Some(cache.path().into()),
        timeout: LIMITE,
        home: carpeta.path().into(),
    };
    let r = sonda_mutacion(&ej, &tarea);
    assert_eq!(r.sonda.valor, None, "{:?}", r.sonda);
    let motivo = r.sonda.motivo.unwrap_or_default();
    assert!(motivo.contains("no dejó reporte"), "{motivo}");
}
