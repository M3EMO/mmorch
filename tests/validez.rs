//! Tests de `r00t oraculo validar` (Tarea 12b) con el binario. El evaluador de producción no corre aquí:
//! sin casos no hay nada que evaluar, y el armado a ciegas usa las rúbricas embebidas (hoy ninguna).

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

fn r00t(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_r00t"))
        .env("ROOT_HOME", home)
        .args(args)
        .output()
        .expect("ejecuta r00t")
}

#[test]
fn validez_cli_sin_casos_escribe_sin_medir() {
    let home = TempDir::new().expect("home");
    let casos = TempDir::new().expect("casos");
    let ruta = casos.path().to_str().expect("ruta UTF-8");
    let salida = r00t(home.path(), &["oraculo", "validar", "--casos", ruta]);
    assert!(
        salida.status.success(),
        "sale con 0: {}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let texto = fs::read_to_string(casos.path().join("validez.json")).expect("validez.json");
    let v: Value = serde_json::from_str(&texto).expect("JSON válido");
    for metrica in ["exactitud_orden", "recall", "acuerdo_decision", "spearman"] {
        assert!(
            v[metrica]["valor"].is_null(),
            "{metrica} sin medir, no 0 ni 1"
        );
        assert!(v[metrica]["motivo"].is_string(), "{metrica} lleva motivo");
    }
    assert_eq!(v["kappa_dimension"].as_array().map(Vec::len), Some(9));
}

#[test]
fn validez_cli_preparar_ciego_avisa_sin_rubricas() {
    let home = TempDir::new().expect("home");
    let dir = TempDir::new().expect("dir");
    let repo = dir.path().join("repo");
    fs::create_dir_all(repo.join("src")).expect("repo");
    fs::write(repo.join("src/lib.rs"), "pub fn f() {}\n").expect("lib");
    fs::write(repo.join("AGENTS.md"), "rastro\n").expect("rastro");
    let casos = dir.path().join("casos");
    let manifiesto = dir.path().join("fuentes.toml");
    fs::write(
        &manifiesto,
        format!(
            "[[fuente]]\ntarea = \"memora\"\norigen = \"ecc-sonnet\"\nrepo = \"{}\"\n\n\
             [[fuente]]\ntarea = \"brasa\"\norigen = \"sdlc\"\nrepo = \"{}\"\n",
            repo.display(),
            repo.display()
        ),
    )
    .expect("manifiesto");

    let salida = r00t(
        home.path(),
        &[
            "oraculo",
            "validar",
            "--preparar-ciego",
            "--fuentes",
            manifiesto.to_str().expect("ruta UTF-8"),
            "--casos",
            casos.to_str().expect("ruta UTF-8"),
            "--semilla",
            "7",
        ],
    );
    assert!(
        salida.status.success(),
        "sale con 0: {}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(
        stderr.contains("rúbricas"),
        "avisa que no hay rúbricas: {stderr}"
    );

    let ciego = casos.join("ciego");
    assert!(
        ciego.join("clave.toml").is_file(),
        "la clave queda en casos/ciego"
    );
    let paquetes: Vec<_> = fs::read_dir(&ciego)
        .expect("ciego")
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .collect();
    assert_eq!(paquetes.len(), 2, "dos paquetes, uno por fuente");
    for paquete in paquetes {
        assert!(paquete.path().join("repo/src/lib.rs").is_file());
        assert!(!paquete.path().join("repo/AGENTS.md").exists());
        assert!(paquete.path().join("formulario.toml").is_file());
    }
}

#[test]
fn validez_cli_preparar_sin_fuentes_es_uso() {
    let home = TempDir::new().expect("home");
    let casos = TempDir::new().expect("casos");
    let salida = r00t(
        home.path(),
        &[
            "oraculo",
            "validar",
            "--preparar-ciego",
            "--casos",
            casos.path().to_str().expect("ruta UTF-8"),
        ],
    );
    assert_eq!(
        salida.status.code(),
        Some(2),
        "--preparar-ciego exige --fuentes"
    );
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(
        stderr.contains("--fuentes"),
        "clap nombra la opción que falta: {stderr}"
    );
}

#[test]
fn validez_cli_casos_inexistentes_es_uso() {
    let home = TempDir::new().expect("home");
    let dir = TempDir::new().expect("dir");
    let inexistente = dir.path().join("no-existe");
    let salida = r00t(
        home.path(),
        &[
            "oraculo",
            "validar",
            "--casos",
            inexistente.to_str().expect("ruta UTF-8"),
        ],
    );
    assert_eq!(
        salida.status.code(),
        Some(2),
        "una carpeta de casos inexistente es error de uso"
    );
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(
        stderr.contains("no existe la carpeta de casos"),
        "mensaje de uso: {stderr}"
    );
}
