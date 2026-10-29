//! `r00t route` contra el binario real, con `ROOT_HOME` temporal.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use root::observe;
use root::tipos::{Evento, TipoEvento, Via};
use serde_json::Value;

fn ahora() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("reloj")
        .as_secs_f64()
}

fn root_con_home(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_r00t"))
        .env("ROOT_HOME", home)
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

fn gasto(via: Via, ts: f64, cost_usd: f64) -> Evento {
    Evento {
        ts,
        corrida: "previa".into(),
        tipo: TipoEvento::Llamada,
        via: Some(via),
        cost_usd,
        ..Evento::default()
    }
}

fn sembrar(home: &Path, eventos: &[Evento]) {
    let ruta = home.join("eventos.jsonl");
    for ev in eventos {
        observe::registrar_en(&ruta, ev).expect("siembra un evento");
    }
}

fn eventos(home: &Path) -> Vec<Evento> {
    let texto = fs::read_to_string(home.join("eventos.jsonl")).unwrap_or_default();
    texto
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("evento válido"))
        .collect()
}

fn json(salida: &Output) -> Value {
    serde_json::from_slice(&salida.stdout).expect("stdout es JSON")
}

#[test]
fn route_cli_juzga_diseno_da_api_con_razonamiento() {
    let home = tempfile::tempdir().expect("home");
    let salida = root_con_home(
        home.path(),
        &["--json", "route", "--rol", "juzga", "--tipo", "diseno"],
    );
    assert_eq!(codigo(&salida), 0);
    let eleccion = json(&salida);
    assert_eq!(eleccion["razonamiento"], true);
    assert_eq!(eleccion["via"], "api");
}

#[test]
fn route_cli_checkeable_da_oraculo() {
    let home = tempfile::tempdir().expect("home");
    let salida = root_con_home(
        home.path(),
        &[
            "--json",
            "route",
            "--rol",
            "organiza",
            "--tipo",
            "bulk",
            "--checkeable",
        ],
    );
    assert_eq!(codigo(&salida), 0);
    assert_eq!(json(&salida)["via"], "oraculo");
}

#[test]
fn route_cli_construye_da_alias_de_cupo() {
    let home = tempfile::tempdir().expect("home");
    let salida = root_con_home(
        home.path(),
        &["--json", "route", "--rol", "construye", "--tipo", "bulk"],
    );
    assert_eq!(codigo(&salida), 0);
    let eleccion = json(&salida);
    assert_eq!(eleccion["via"], "cupo");
    assert_eq!(eleccion["modelo"], "haiku");
}

#[test]
fn route_cli_sin_json_imprime_una_linea_legible() {
    let home = tempfile::tempdir().expect("home");
    let salida = root_con_home(
        home.path(),
        &["route", "--rol", "juzga", "--tipo", "diseno"],
    );
    assert_eq!(codigo(&salida), 0);
    let texto = String::from_utf8_lossy(&salida.stdout);
    assert_eq!(texto.lines().count(), 1, "stdout: {texto}");
    assert!(texto.contains("via=api"), "stdout: {texto}");
    assert!(texto.contains("razonamiento=true"), "stdout: {texto}");
}

#[test]
fn route_cli_rol_invalido_sale_con_2() {
    let home = tempfile::tempdir().expect("home");
    let salida = root_con_home(home.path(), &["route", "--rol", "inventado", "--tipo", "x"]);
    assert_eq!(codigo(&salida), 2);
}

#[test]
fn route_cli_sin_presupuesto_sale_con_3_y_registra_gate() {
    let home = tempfile::tempdir().expect("home");
    let ts = ahora();
    sembrar(
        home.path(),
        &[
            gasto(Via::Cupo, ts - 600.0, 35.0),
            gasto(Via::Api, ts - 60.0, 100.0),
        ],
    );
    let salida = root_con_home(
        home.path(),
        &["route", "--rol", "organiza", "--tipo", "bulk"],
    );
    assert_eq!(codigo(&salida), 3);
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(stderr.contains("sin presupuesto"), "stderr: {stderr}");
    assert!(stderr.contains("reabre"), "stderr: {stderr}");

    let gates: Vec<Evento> = eventos(home.path())
        .into_iter()
        .filter(|ev| ev.tipo == TipoEvento::Gate)
        .collect();
    assert_eq!(gates.len(), 1);
    let gate = &gates[0];
    assert_eq!(gate.corrida, "route");
    assert_eq!(gate.fase, "presupuesto");
    assert_eq!(gate.extra["rol"], "organiza");
    assert_eq!(gate.extra["tipo"], "bulk");
    assert!(gate.extra["reabre"].is_string());
    assert!(gate.extra["motivo"].is_string());
}

#[test]
fn route_cli_cupo_32_con_api_con_margen_elige_api() {
    let home = tempfile::tempdir().expect("home");
    let ts = ahora();
    sembrar(
        home.path(),
        &[
            gasto(Via::Cupo, ts - 600.0, 32.0),
            gasto(Via::Api, ts - 60.0, 40.0),
        ],
    );
    let salida = root_con_home(
        home.path(),
        &["--json", "route", "--rol", "organiza", "--tipo", "bulk"],
    );
    assert_eq!(codigo(&salida), 0);
    assert_eq!(json(&salida)["via"], "api");
}

#[test]
fn route_cli_politica_ilegible_sale_con_3() {
    let home = tempfile::tempdir().expect("home");
    fs::create_dir(home.path().join("politica.json")).expect("directorio en lugar de archivo");
    let salida = root_con_home(
        home.path(),
        &["route", "--rol", "juzga", "--tipo", "diseno"],
    );
    assert_eq!(codigo(&salida), 3);
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(stderr.contains("politica.json"), "stderr: {stderr}");
}

#[test]
fn route_cli_tabla_api_sin_tope_sale_con_2_y_nombra_la_tabla() {
    let home = tempfile::tempdir().expect("home");
    fs::write(home.path().join("root.toml"), "[balance.api]\n").expect("root.toml");
    let salida = root_con_home(home.path(), &["route", "--rol", "delega", "--tipo", "bulk"]);
    assert_eq!(codigo(&salida), 2);
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(
        stderr.contains("[balance.api] exige tope_usd"),
        "stderr: {stderr}"
    );
}

#[test]
fn route_cli_precio_invalido_sale_con_2_antes_de_rutear() {
    let casos = [
        ("usd_entrada = 1.0\nusd_salda = 1.1", "usd_salda"),
        ("usd_entrada = nan\nusd_salida = 1.0", "usd_entrada"),
    ];
    for (cuerpo, clave) in casos {
        let home = tempfile::tempdir().expect("home");
        let texto = format!("[precios.deepseek-chat]\n{cuerpo}\n");
        fs::write(home.path().join("root.toml"), texto).expect("root.toml");
        let salida = root_con_home(home.path(), &["route", "--rol", "delega", "--tipo", "bulk"]);
        assert_eq!(codigo(&salida), 2, "{cuerpo}");
        let stderr = String::from_utf8_lossy(&salida.stderr);
        assert!(stderr.contains(clave), "{cuerpo}: stderr: {stderr}");
    }
}

#[test]
fn route_cli_sin_root_toml_usa_defaults_y_sale_con_0() {
    let home = tempfile::tempdir().expect("home");
    let salida = root_con_home(
        home.path(),
        &["--json", "route", "--rol", "delega", "--tipo", "bulk"],
    );
    assert_eq!(codigo(&salida), 0);
    assert_eq!(json(&salida)["modelo"], "deepseek-flash");
}

#[test]
fn route_cli_juzga_sin_root_toml_elige_deepseek_flash_con_razonamiento() {
    let home = tempfile::tempdir().expect("home");
    let salida = root_con_clave(
        home.path(),
        &["--json", "route", "--rol", "juzga", "--tipo", "x"],
        Some("sk-test-SECRETO-ROUTE"),
    );
    assert_eq!(codigo(&salida), 0);
    let j = json(&salida);
    assert_eq!(j["modelo"], "deepseek-flash");
    assert_eq!(j["razonamiento"], true);
}

#[test]
fn route_cli_modelo_viejo_de_deepseek_avisa_en_stderr_y_usa_deepseek_flash() {
    let home = tempfile::tempdir().expect("home");
    let texto = "[ruteo.juzga]\nproveedor = \"deepseek\"\nmodelo = \"deepseek-reasoner\"\n";
    fs::write(home.path().join("root.toml"), texto).expect("root.toml");
    let salida = root_con_clave(
        home.path(),
        &["--json", "route", "--rol", "juzga", "--tipo", "x"],
        Some("sk-test-SECRETO-ROUTE"),
    );
    assert_eq!(codigo(&salida), 0);
    assert_eq!(json(&salida)["modelo"], "deepseek-flash");
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(
        stderr.contains("deepseek-reasoner") && stderr.contains("deepseek-flash"),
        "stderr: {stderr}"
    );
}

/// `r00t` con `ROOT_HOME` de `home` y `DEEPSEEK_API_KEY` dada, o sin la variable si `clave` es `None`.
fn root_con_clave(home: &Path, args: &[&str], clave: Option<&str>) -> Output {
    let mut orden = Command::new(env!("CARGO_BIN_EXE_r00t"));
    orden.env("ROOT_HOME", home).args(args);
    match clave {
        Some(valor) => orden.env("DEEPSEEK_API_KEY", valor),
        None => orden.env_remove("DEEPSEEK_API_KEY"),
    };
    orden.output().expect("no se pudo ejecutar el binario root")
}

#[test]
fn route_cli_juzga_sin_clave_de_deepseek_elige_haiku_con_razonamiento() {
    let home = tempfile::tempdir().expect("home");
    let salida = root_con_clave(
        home.path(),
        &["--json", "route", "--rol", "juzga", "--tipo", "x"],
        None,
    );
    assert_eq!(codigo(&salida), 0);
    let e = json(&salida);
    assert_eq!(e["via"], "api");
    assert_eq!(e["proveedor"], "anthropic");
    assert_eq!(e["modelo"], "claude-haiku-5-5");
    assert_eq!(e["razonamiento"], true);
    assert_eq!(e["rol"], "juzga");
}

#[test]
fn route_cli_juzga_con_clave_de_deepseek_elige_deepseek_flash() {
    let home = tempfile::tempdir().expect("home");
    let salida = root_con_clave(
        home.path(),
        &["--json", "route", "--rol", "juzga", "--tipo", "x"],
        Some("sk-test-SECRETO-ROUTE"),
    );
    assert_eq!(codigo(&salida), 0);
    let e = json(&salida);
    assert_eq!(e["proveedor"], "deepseek");
    assert_eq!(e["modelo"], "deepseek-flash");
    assert_eq!(e["razonamiento"], true);
}

#[test]
fn route_cli_delega_sin_clave_no_cambia_a_respaldo() {
    let home = tempfile::tempdir().expect("home");
    let salida = root_con_clave(
        home.path(),
        &["--json", "route", "--rol", "delega", "--tipo", "x"],
        None,
    );
    assert_eq!(codigo(&salida), 0);
    let e = json(&salida);
    assert_eq!(e["proveedor"], "deepseek");
    assert_eq!(e["modelo"], "deepseek-flash");
    assert_eq!(e["razonamiento"], false);
}

#[test]
fn route_cli_juzga_con_modelo_fuera_de_la_lista_sale_con_2() {
    let home = tempfile::tempdir().expect("home");
    let texto = "[ruteo.juzga]\nproveedor = \"deepseek\"\nmodelo = \"modelo-raro\"\n";
    fs::write(home.path().join("root.toml"), texto).expect("root.toml");
    let salida = root_con_clave(
        home.path(),
        &["route", "--rol", "juzga", "--tipo", "x"],
        Some("sk-test-SECRETO-ROUTE"),
    );
    assert_eq!(codigo(&salida), 2);
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(stderr.contains("modelo-raro"), "stderr: {stderr}");
}

#[test]
fn route_cli_respaldo_fuera_de_la_lista_sale_con_2_cuando_se_usa() {
    let home = tempfile::tempdir().expect("home");
    let texto = "[ruteo.juzga]\nproveedor = \"deepseek\"\nmodelo = \"deepseek-flash\"\n\
                 [ruteo.juzga.respaldo]\nproveedor = \"anthropic\"\nmodelo = \"claude-foo\"\n";
    fs::write(home.path().join("root.toml"), texto).expect("root.toml");
    let salida = root_con_clave(
        home.path(),
        &["route", "--rol", "juzga", "--tipo", "x"],
        None,
    );
    assert_eq!(codigo(&salida), 2);
}
