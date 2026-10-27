use std::fs;
use std::path::Path;
use std::process::{Command, Output};

const GOAL_FIXTURE: &str = "# GOAL de prueba\n\nContenido de fixture.\n";

fn root_en(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_r00t"))
        .current_dir(dir)
        .args(args)
        .output()
        .expect("no se pudo ejecutar el binario root")
}

fn codigo(salida: &Output) -> i32 {
    salida
        .status
        .code()
        .expect("el proceso no terminó con código")
}

#[test]
fn goal_verificar_sin_goal_sale_con_3() {
    let dir = tempfile::tempdir().expect("temp dir");
    let salida = root_en(dir.path(), &["goal", "verificar"]);
    assert_eq!(codigo(&salida), 3);
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(stderr.contains("GOAL no autorizado"), "stderr: {stderr}");
}

#[test]
fn goal_verificar_json_imprime_ok_y_hash() {
    let dir = tempfile::tempdir().expect("temp dir");
    let hash = root::goal::calcular_hash(GOAL_FIXTURE.as_bytes());
    fs::write(dir.path().join("GOAL.md"), GOAL_FIXTURE).expect("escribe GOAL.md");
    fs::write(dir.path().join("GOAL.hash"), format!("{hash}\n")).expect("escribe GOAL.hash");

    let salida = root_en(dir.path(), &["--json", "goal", "verificar"]);
    assert_eq!(codigo(&salida), 0);
    let json: serde_json::Value =
        serde_json::from_slice(&salida.stdout).expect("stdout debe ser JSON válido");
    assert_eq!(json["ok"], true);
    assert_eq!(json["hash"], hash);
}

#[test]
fn goal_autorizar_sin_tty_sale_con_2_y_no_escribe() {
    let dir = tempfile::tempdir().expect("temp dir");
    fs::write(dir.path().join("GOAL.md"), GOAL_FIXTURE).expect("escribe GOAL.md");

    let salida = root_en(dir.path(), &["goal", "autorizar"]);
    assert_eq!(codigo(&salida), 2);
    assert!(!dir.path().join("GOAL.hash").exists());
}
