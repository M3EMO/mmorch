//! `root.toml` estricto contra el binario real: una clave mal escrita sale con código 2.

use std::fs;
use std::process::{Command, Output};

fn route_con_root_toml(texto: &str) -> Output {
    let home = tempfile::tempdir().expect("home");
    fs::write(home.path().join("root.toml"), texto).expect("root.toml");
    Command::new(env!("CARGO_BIN_EXE_r00t"))
        .env("ROOT_HOME", home.path())
        .args(["route", "--rol", "delega", "--tipo", "bulk"])
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
fn config_estricta_tope_mal_escrito_en_balance_api_sale_con_2() {
    let salida = route_con_root_toml("[balance.api]\ntope_usdd = 5\n");
    assert_eq!(codigo(&salida), 2);
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(stderr.contains("tope_usd"), "stderr: {stderr}");
}

#[test]
fn config_estricta_clave_extra_en_balance_api_sale_con_2_y_la_nombra() {
    let salida = route_con_root_toml("[balance.api]\ntope_usd = 5\nextra = 1\n");
    assert_eq!(codigo(&salida), 2);
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(stderr.contains("extra"), "stderr: {stderr}");
}

#[test]
fn config_estricta_umbral_wilson_fuera_de_rango_sale_con_2() {
    let salida = route_con_root_toml("umbral_wilson = 1.5\n");
    assert_eq!(codigo(&salida), 2);
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(stderr.contains("umbral_wilson"), "stderr: {stderr}");
}

#[test]
fn config_estricta_umbral_verde_retirado_carga_con_aviso_y_sale_con_0() {
    let salida = route_con_root_toml("umbral_verde = 0.8\n");
    assert_eq!(codigo(&salida), 0);
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(stderr.contains("umbral_verde"), "stderr: {stderr}");
}
