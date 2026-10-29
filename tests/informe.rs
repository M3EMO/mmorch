//! Tests del informe del PM (Tarea 13b) que no necesitan podman: la configuración de `[decision]`
//! falla antes de tocar el contenedor.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn r00t(root_home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_r00t"))
        .env("ROOT_HOME", root_home)
        .args(args)
        .output()
        .expect("no se pudo ejecutar el binario r00t")
}

/// `ROOT_HOME` con un `root.toml` dado y la tarea `tarea-x`, sin ficha en su spec.
fn home_con(root_toml: &str) -> tempfile::TempDir {
    let home = tempfile::tempdir().expect("home");
    fs::write(home.path().join("root.toml"), root_toml).expect("root.toml");
    let dir = home.path().join("oraculos").join("tarea-x");
    fs::create_dir_all(&dir).expect("dir");
    fs::write(
        dir.join("tarea.toml"),
        "nombre = \"tarea-x\"\nlenguaje = \"rust\"\nimagen = \"x\"\nspec = \"spec.md\"\n\
         [contrato]\ncmd = 'true'\n[oculto]\ncmd = 'true'\n",
    )
    .expect("tarea.toml");
    fs::write(dir.join("spec.md"), "# Spec\n").expect("spec.md");
    home
}

#[test]
fn oraculo_evaluar_con_completitud_minima_fuera_de_rango_sale_con_2() {
    let home = home_con("[decision]\ncompletitud_minima = 1.5\n");
    let repo = tempfile::tempdir().expect("repo");
    let salida = r00t(
        home.path(),
        &[
            "oraculo",
            "evaluar",
            "--tarea",
            "tarea-x",
            "--repo",
            repo.path().to_str().expect("ruta UTF-8"),
        ],
    );
    assert_eq!(salida.status.code(), Some(2), "{salida:?}");
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(stderr.contains("completitud_minima"), "{stderr}");
}

#[test]
fn oraculo_informe_con_completitud_minima_fuera_de_rango_sale_con_2() {
    let home = home_con("[decision]\ncompletitud_minima = 1.5\n");
    let repo = tempfile::tempdir().expect("repo");
    let salida = r00t(
        home.path(),
        &[
            "oraculo",
            "informe",
            "--tarea",
            "tarea-x",
            "--repo",
            repo.path().to_str().expect("ruta UTF-8"),
        ],
    );
    assert_eq!(salida.status.code(), Some(2), "{salida:?}");
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(stderr.contains("completitud_minima"), "{stderr}");
}
