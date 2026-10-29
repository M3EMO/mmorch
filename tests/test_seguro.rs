//! `tools/test-seguro.sh`: la suite corre sin claves, con un `ROOT_HOME` de prueba nuevo y con los
//! temporales y el target bajo `ROOT_TESTS_BASE`. Un `cargo` falso, que vuelca el entorno y los
//! argumentos, reemplaza a cargo por `ROOT_TEST_SEGURO_CARGO`. Así no se paga una compilación anidada.
//! Cada test arma el entorno del hijo con `Command::env`: nada cambia el entorno del proceso.

use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

const SCRIPT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tools/test-seguro.sh");
const PRE_COMMIT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/.githooks/pre-commit");

/// Un `cargo` falso: vuelca su entorno en `env.txt` y sus argumentos en `args.txt`, y sale con `codigo`.
fn falso(dir: &Path, codigo: u8) -> PathBuf {
    let ruta = dir.join(format!("cargo-falso-{codigo}.sh"));
    let texto = format!(
        "#!/usr/bin/env bash\nenv > '{d}/env.txt'\nprintf '%s\\n' \"$@\" > '{d}/args.txt'\nexit {codigo}\n",
        d = dir.display()
    );
    fs::write(&ruta, texto).expect("escribe el cargo falso");
    fs::set_permissions(&ruta, fs::Permissions::from_mode(0o755)).expect("permisos del falso");
    ruta
}

/// Corre el script con `args`, el `cargo` falso `falso` y `extra` en el entorno del hijo.
/// Devuelve el estado y el entorno que vio el falso.
fn correr(
    dir: &Path,
    codigo: u8,
    extra: &[(&str, String)],
    args: &[&str],
) -> (ExitStatus, HashMap<String, String>, Vec<String>) {
    let cargo = falso(dir, codigo);
    let mut orden = Command::new(SCRIPT);
    orden
        .args(args)
        .env("DEEPSEEK_API_KEY", "x")
        .env("ANTHROPIC_API_KEY", "y")
        .env("HOME", dir)
        .env("ROOT_HOME", dir.join("real"))
        .env("ROOT_TESTS_BASE", dir.join("base"))
        .env("ROOT_TEST_SEGURO_CARGO", &cargo);
    for (clave, valor) in extra {
        orden.env(clave, valor);
    }
    let estado = orden.status().expect("corre tools/test-seguro.sh");
    let env_texto = fs::read_to_string(dir.join("env.txt")).expect("el falso dejó su entorno");
    let entorno = env_texto
        .lines()
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let args_texto =
        fs::read_to_string(dir.join("args.txt")).expect("el falso dejó sus argumentos");
    let argumentos = args_texto.lines().map(str::to_string).collect();
    (estado, entorno, argumentos)
}

fn base(dir: &Path) -> PathBuf {
    dir.join("base")
}

#[test]
fn test_seguro_quita_las_claves_del_entorno() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (_, entorno, _) = correr(dir.path(), 0, &[], &["algo"]);
    assert!(!entorno.contains_key("DEEPSEEK_API_KEY"), "{entorno:?}");
    assert!(!entorno.contains_key("ANTHROPIC_API_KEY"), "{entorno:?}");
}

#[test]
fn test_seguro_root_home_es_nuevo_bajo_base_y_se_borra_al_salir() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (_, entorno, _) = correr(dir.path(), 0, &[], &["algo"]);
    let root_home = PathBuf::from(&entorno["ROOT_HOME"]);
    assert!(root_home.starts_with(base(dir.path())), "{root_home:?}");
    assert_ne!(root_home, dir.path().join("real"));
    assert!(
        !root_home.exists(),
        "el ROOT_HOME de prueba se borra al terminar"
    );
}

#[test]
fn test_seguro_temporales_y_target_bajo_base() {
    let dir = tempfile::tempdir().expect("temp dir");
    // Un CARGO_TARGET_DIR fijado por el usuario no gana: el script usa el de base.
    let fijado = dir.path().join("fijado");
    let extra = [("CARGO_TARGET_DIR", fijado.display().to_string())];
    let (_, entorno, _) = correr(dir.path(), 0, &extra, &["algo"]);
    assert!(PathBuf::from(&entorno["TMPDIR"]).starts_with(base(dir.path())));
    let target = PathBuf::from(&entorno["CARGO_TARGET_DIR"]);
    assert!(target.starts_with(base(dir.path())), "{target:?}");
    assert_ne!(target, fijado);
    assert!(target.is_dir(), "el target se crea");
}

#[test]
fn test_seguro_cargo_target_dir_seguro_gana_sobre_base() {
    let dir = tempfile::tempdir().expect("temp dir");
    let propio = dir.path().join("propio-target");
    let extra = [("CARGO_TARGET_DIR_SEGURO", propio.display().to_string())];
    let (_, entorno, _) = correr(dir.path(), 0, &extra, &["algo"]);
    assert_eq!(PathBuf::from(&entorno["CARGO_TARGET_DIR"]), propio);
}

#[test]
fn test_seguro_pasa_los_argumentos_a_cargo_test() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (estado, _, argumentos) = correr(dir.path(), 0, &[], &["algo"]);
    assert!(estado.success());
    assert_eq!(argumentos, ["test", "algo"]);
}

#[test]
fn test_seguro_sale_con_el_codigo_de_cargo() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (estado, _, _) = correr(dir.path(), 7, &[], &["algo"]);
    assert_eq!(estado.code(), Some(7));
}

#[test]
fn test_seguro_root_sin_podman_llega_al_comando() {
    let dir = tempfile::tempdir().expect("temp dir");
    let extra = [("ROOT_SIN_PODMAN", "1".to_string())];
    let (_, entorno, _) = correr(dir.path(), 0, &extra, &["algo"]);
    assert_eq!(
        entorno.get("ROOT_SIN_PODMAN").map(String::as_str),
        Some("1")
    );
}

#[test]
fn test_seguro_pre_commit_usa_el_script_sin_cargo_test_suelto() {
    let texto = fs::read_to_string(PRE_COMMIT).expect("lee .githooks/pre-commit");
    assert!(texto.contains("tools/test-seguro.sh"), "{texto}");
    assert!(
        !texto.lines().any(|l| l.trim() == "cargo test"),
        "el hook no debe correr `cargo test` suelto: {texto}"
    );
    assert!(texto.contains("cargo fmt --check"), "{texto}");
    assert!(
        texto.contains("cargo clippy --all-targets -- -D warnings"),
        "{texto}"
    );
}
