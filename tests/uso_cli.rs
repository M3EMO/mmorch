//! `r00t observa uso` y `r00t observa claude --via` contra el binario real.

use std::fs;
use std::path::{Path, PathBuf};
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

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude-sesion.json")
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

fn gates_de_tope(home: &Path, tope: &str) -> Vec<Evento> {
    eventos(home)
        .into_iter()
        .filter(|ev| {
            ev.tipo == TipoEvento::Gate && ev.fase == "tope-lleno" && ev.extra["tope"] == tope
        })
        .collect()
}

#[test]
fn uso_cli_nube_de_10_suma_al_mes_y_al_bloque() {
    let home = tempfile::tempdir().expect("home");
    sembrar(home.path(), &[gasto(Via::Nube, ahora() - 60.0, 10.0)]);

    let salida = root_con_home(home.path(), &["--json", "observa", "uso"]);

    assert_eq!(codigo(&salida), 0);
    let json: Value = serde_json::from_slice(&salida.stdout).expect("stdout es JSON");
    assert_eq!(json["uso"]["nube_mes_usd"], 10.0);
    assert_eq!(json["uso"]["cupo_bloque_usd"], 10.0);
    assert_eq!(json["margenes"]["nube"]["lleno"], false);
    let aviso = json["aviso"].as_str().expect("aviso");
    assert!(aviso.contains("no suma"), "aviso: {aviso}");
}

#[test]
fn uso_cli_sin_json_imprime_cupo_api_nube_y_aviso() {
    let home = tempfile::tempdir().expect("home");
    let salida = root_con_home(home.path(), &["observa", "uso"]);
    assert_eq!(codigo(&salida), 0);
    let texto = String::from_utf8_lossy(&salida.stdout).to_lowercase();
    for etiqueta in ["cupo", "api", "nube", "no suma"] {
        assert!(texto.contains(etiqueta), "falta {etiqueta} en: {texto}");
    }
}

#[test]
fn uso_cli_nube_lleno_registra_un_gate_y_no_se_repite() {
    let home = tempfile::tempdir().expect("home");
    sembrar(home.path(), &[gasto(Via::Nube, ahora() - 60.0, 250.0)]);

    let primera = root_con_home(home.path(), &["observa", "uso"]);
    assert_eq!(codigo(&primera), 0);
    let gates = gates_de_tope(home.path(), "nube");
    assert_eq!(gates.len(), 1);
    assert_eq!(gates[0].corrida, "uso");
    assert_eq!(gates[0].extra["tope_usd"], 250.0);
    assert_eq!(gates[0].extra["usado"], 250.0);

    let segunda = root_con_home(home.path(), &["observa", "uso"]);
    assert_eq!(codigo(&segunda), 0);
    assert_eq!(
        gates_de_tope(home.path(), "nube").len(),
        1,
        "no debe repetirse"
    );
}

#[test]
fn uso_cli_nube_no_cambia_la_eleccion_de_route() {
    let home = tempfile::tempdir().expect("home");
    // Seis horas atrás: fuera del bloque vigente, así que no ocupa el cupo.
    sembrar(
        home.path(),
        &[gasto(Via::Nube, ahora() - 6.0 * 3600.0, 250.0)],
    );

    let salida = root_con_home(
        home.path(),
        &["--json", "route", "--rol", "organiza", "--tipo", "bulk"],
    );

    assert_eq!(codigo(&salida), 0);
    let eleccion: Value = serde_json::from_slice(&salida.stdout).expect("stdout es JSON");
    assert_eq!(eleccion["via"], "cupo");
}

#[test]
fn uso_cli_observa_claude_via_nube_registra_via_nube() {
    let home = tempfile::tempdir().expect("home");
    let salida = root_con_home(
        home.path(),
        &[
            "observa",
            "claude",
            "--archivo",
            fixture().to_str().expect("ruta UTF-8"),
            "--corrida",
            "cn",
            "--via",
            "nube",
        ],
    );
    assert_eq!(codigo(&salida), 0);
    let texto = String::from_utf8_lossy(&salida.stdout);
    assert!(texto.contains("nube"), "stdout: {texto}");

    let registrados: Vec<Evento> = eventos(home.path())
        .into_iter()
        .filter(|ev| ev.corrida == "cn")
        .collect();
    assert!(!registrados.is_empty());
    assert!(registrados.iter().all(|ev| ev.via == Some(Via::Nube)));
}

#[test]
fn uso_cli_observa_claude_sin_via_registra_cupo() {
    let home = tempfile::tempdir().expect("home");
    let salida = root_con_home(
        home.path(),
        &[
            "observa",
            "claude",
            "--archivo",
            fixture().to_str().expect("ruta UTF-8"),
            "--corrida",
            "cc",
        ],
    );
    assert_eq!(codigo(&salida), 0);

    let registrados: Vec<Evento> = eventos(home.path())
        .into_iter()
        .filter(|ev| ev.corrida == "cc")
        .collect();
    assert!(!registrados.is_empty());
    assert!(registrados.iter().any(|ev| ev.via == Some(Via::Cupo)));
    assert!(registrados.iter().all(|ev| ev.via != Some(Via::Nube)));
}
