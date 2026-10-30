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

/// 2026-10-31T12:00:00Z.
const OCTUBRE_FIN_UTC: f64 = 1_793_448_000.0;

fn gasto_de_api_de_octubre(home: &Path) -> f64 {
    observe::uso::uso_en(&home.join("eventos.jsonl"), OCTUBRE_FIN_UTC, 0, 5)
        .expect("lee")
        .api_mes_usd
}

fn sesion_json(home: &Path, nombre: &str, contenido: &str) -> PathBuf {
    let ruta = home.join(nombre);
    fs::write(&ruta, contenido).expect("escribe la sesión");
    ruta
}

#[test]
fn uso_cli_costo_manual_suma_al_gasto_de_api_de_octubre() {
    let home = tempfile::tempdir().expect("home");
    let salida = root_con_home(
        home.path(),
        &[
            "observa",
            "costo",
            "--usd",
            "0.50",
            "--motivo",
            "x",
            "--fecha",
            "2026-10-29",
        ],
    );
    assert_eq!(codigo(&salida), 0);
    let registrados = eventos(home.path());
    assert_eq!(registrados.len(), 1);
    let ev = &registrados[0];
    assert_eq!(ev.tipo, TipoEvento::Llamada);
    assert_eq!(ev.via, Some(Via::Api));
    assert_eq!(ev.extra["manual"], true);
    assert_eq!(ev.extra["motivo"], "x");
    assert!((gasto_de_api_de_octubre(home.path()) - 0.5).abs() < 1e-12);
}

#[test]
fn uso_cli_costo_manual_json_imprime_el_evento() {
    let home = tempfile::tempdir().expect("home");
    let salida = root_con_home(
        home.path(),
        &[
            "--json", "observa", "costo", "--usd", "1.25", "--motivo", "y",
        ],
    );
    assert_eq!(codigo(&salida), 0);
    let ev: Evento = serde_json::from_slice(&salida.stdout).expect("stdout es un evento");
    assert_eq!(ev.cost_usd, 1.25);
    assert_eq!(ev.via, Some(Via::Api));
}

#[test]
fn uso_cli_costo_manual_con_monto_invalido_sale_con_2_y_no_escribe() {
    for usd in ["-1", "0", "nan", "inf"] {
        let home = tempfile::tempdir().expect("home");
        let salida = root_con_home(
            home.path(),
            &["observa", "costo", "--usd", usd, "--motivo", "x"],
        );
        assert_eq!(codigo(&salida), 2, "usd {usd}");
        assert!(
            eventos(home.path()).is_empty(),
            "usd {usd} no debe escribir"
        );
    }
}

#[test]
fn uso_cli_costo_manual_con_fecha_que_no_parsea_sale_con_2_y_no_escribe() {
    let home = tempfile::tempdir().expect("home");
    let salida = root_con_home(
        home.path(),
        &[
            "observa",
            "costo",
            "--usd",
            "0.5",
            "--motivo",
            "x",
            "--fecha",
            "29/10/2026",
        ],
    );
    assert_eq!(codigo(&salida), 2);
    assert!(eventos(home.path()).is_empty());
}

#[test]
fn uso_cli_observa_claude_nube_que_cruza_el_tope_registra_gate() {
    let home = tempfile::tempdir().expect("home");
    let sesion = sesion_json(
        home.path(),
        "sesion.json",
        r#"{"total_cost_usd": 251.0, "num_turns": 3}"#,
    );
    let salida = root_con_home(
        home.path(),
        &[
            "observa",
            "claude",
            "--archivo",
            sesion.to_str().expect("ruta UTF-8"),
            "--corrida",
            "cn",
            "--via",
            "nube",
        ],
    );
    assert_eq!(codigo(&salida), 0);
    let gates = gates_de_tope(home.path(), "nube");
    assert_eq!(gates.len(), 1, "debe registrar el Gate al cruzar el tope");
    assert_eq!(gates[0].extra["tope_usd"], 250.0);
    assert_eq!(gates[0].extra["usado"], 251.0);
}

#[test]
fn uso_cli_observa_claude_nube_bajo_el_tope_no_registra_gate() {
    let home = tempfile::tempdir().expect("home");
    let sesion = sesion_json(home.path(), "sesion.json", r#"{"total_cost_usd": 100.0}"#);
    let salida = root_con_home(
        home.path(),
        &[
            "observa",
            "claude",
            "--archivo",
            sesion.to_str().expect("ruta UTF-8"),
            "--corrida",
            "cn",
            "--via",
            "nube",
        ],
    );
    assert_eq!(codigo(&salida), 0);
    assert!(gates_de_tope(home.path(), "nube").is_empty());
}

#[test]
fn uso_cli_observa_claude_nube_no_repite_el_gate_del_mes() {
    let home = tempfile::tempdir().expect("home");
    for nombre in ["uno.json", "dos.json"] {
        let sesion = sesion_json(home.path(), nombre, r#"{"total_cost_usd": 251.0}"#);
        let salida = root_con_home(
            home.path(),
            &[
                "observa",
                "claude",
                "--archivo",
                sesion.to_str().expect("ruta UTF-8"),
                "--corrida",
                "cn",
                "--via",
                "nube",
            ],
        );
        assert_eq!(codigo(&salida), 0);
    }
    assert_eq!(gates_de_tope(home.path(), "nube").len(), 1);
}

#[test]
fn uso_cli_observa_claude_con_costo_negativo_sale_con_3_y_no_registra_nada() {
    let home = tempfile::tempdir().expect("home");
    let sesion = sesion_json(
        home.path(),
        "sesion.json",
        r#"{"modelUsage": {"claude-haiku-5-5": {"costUSD": -0.5}}}"#,
    );
    let salida = root_con_home(
        home.path(),
        &[
            "observa",
            "claude",
            "--archivo",
            sesion.to_str().expect("ruta UTF-8"),
            "--corrida",
            "cneg",
        ],
    );
    assert_eq!(codigo(&salida), 3);
    assert!(eventos(home.path()).is_empty());
}

/// Reserva de API de `cost_usd` con el id `id`, como la escribe la llamada antes de enviar.
fn reserva_de_api(id: &str, ts: f64, cost_usd: f64) -> Evento {
    Evento {
        ts,
        corrida: "previa".into(),
        tipo: TipoEvento::Reserva,
        via: Some(Via::Api),
        cost_usd,
        extra: serde_json::Map::from_iter([("reserva".to_string(), serde_json::json!(id))]),
        ..Evento::default()
    }
}

/// Evento final de una llamada de API que cierra la reserva `id`.
fn cierre_de_api(id: &str, ts: f64, cost_usd: f64) -> Evento {
    Evento {
        ts,
        corrida: "previa".into(),
        tipo: TipoEvento::Llamada,
        via: Some(Via::Api),
        cost_usd,
        extra: serde_json::Map::from_iter([("reservas".to_string(), serde_json::json!([id]))]),
        ..Evento::default()
    }
}

#[test]
fn uso_cli_lista_la_reserva_abierta_con_tres_timeouts_y_dice_quien_la_corrige() {
    let home = tempfile::tempdir().expect("home");
    // Tres veces el `timeout_juez_s` por defecto (1200 s): más de dos, así que sale en la lista.
    sembrar(
        home.path(),
        &[reserva_de_api("vieja-1", ahora() - 3.0 * 1200.0, 0.75)],
    );

    let texto = root_con_home(home.path(), &["observa", "uso"]);
    assert_eq!(codigo(&texto), 0);
    let texto = String::from_utf8_lossy(&texto.stdout);
    assert!(texto.contains("vieja-1"), "stdout: {texto}");
    assert!(texto.contains("r00t observa costo"), "stdout: {texto}");

    let salida = root_con_home(home.path(), &["--json", "observa", "uso"]);
    assert_eq!(codigo(&salida), 0);
    let json: Value = serde_json::from_slice(&salida.stdout).expect("stdout es JSON");
    assert_eq!(json["reservas_viejas"].as_array().map(Vec::len), Some(1));
    assert_eq!(json["reservas_viejas"][0]["reserva"], "vieja-1");
    assert_eq!(json["reservas_viejas"][0]["usd"], 0.75);
}

#[test]
fn uso_cli_reserva_de_un_timeout_o_cerrada_no_sale_en_la_lista() {
    let home = tempfile::tempdir().expect("home");
    sembrar(
        home.path(),
        &[
            reserva_de_api("reciente", ahora() - 1200.0, 0.1),
            reserva_de_api("cerrada", ahora() - 3.0 * 1200.0, 0.2),
            cierre_de_api("cerrada", ahora() - 60.0, 0.2),
        ],
    );

    let salida = root_con_home(home.path(), &["--json", "observa", "uso"]);
    assert_eq!(codigo(&salida), 0);
    let json: Value = serde_json::from_slice(&salida.stdout).expect("stdout es JSON");
    assert_eq!(json["reservas_viejas"].as_array().map(Vec::len), Some(0));
}

#[test]
fn uso_cli_sesion_sin_costo_deja_gate_ademas_de_la_marca() {
    let home = tempfile::tempdir().expect("home");
    let sesion = sesion_json(
        home.path(),
        "sin-costo.json",
        r#"{"session_id": "s-sin", "modelUsage": {"claude-haiku-5-5": {"inputTokens": 10, "outputTokens": 5}}}"#,
    );
    let salida = root_con_home(
        home.path(),
        &[
            "observa",
            "claude",
            "--archivo",
            sesion.to_str().expect("ruta UTF-8"),
            "--corrida",
            "csc",
        ],
    );
    assert_eq!(codigo(&salida), 0);

    let registrados: Vec<Evento> = eventos(home.path())
        .into_iter()
        .filter(|ev| ev.corrida == "csc")
        .collect();
    let sesion_ev = registrados
        .iter()
        .find(|ev| ev.tipo == TipoEvento::Sesion)
        .expect("el Sesion de la importación");
    assert_eq!(sesion_ev.extra["costo_desconocido"], true);
    let gates: Vec<&Evento> = registrados
        .iter()
        .filter(|ev| ev.tipo == TipoEvento::Gate)
        .collect();
    assert_eq!(
        gates.len(),
        1,
        "debe registrar un Gate por la sesión sin costo"
    );
    assert_eq!(gates[0].fase, "costo-desconocido");
    assert_eq!(gates[0].extra["sesion"], "s-sin");
    let motivo = gates[0].extra["motivo"].as_str().expect("motivo");
    assert!(
        motivo.contains("el tope no la puede sumar"),
        "motivo: {motivo}"
    );
}

#[test]
fn uso_cli_sesion_con_costo_no_deja_gate() {
    let home = tempfile::tempdir().expect("home");
    let sesion = sesion_json(
        home.path(),
        "con-costo.json",
        r#"{"session_id": "s-con", "modelUsage": {"claude-haiku-5-5": {"inputTokens": 10, "costUSD": 0.5}}}"#,
    );
    let salida = root_con_home(
        home.path(),
        &[
            "observa",
            "claude",
            "--archivo",
            sesion.to_str().expect("ruta UTF-8"),
            "--corrida",
            "ccc",
        ],
    );
    assert_eq!(codigo(&salida), 0);

    let registrados: Vec<Evento> = eventos(home.path())
        .into_iter()
        .filter(|ev| ev.corrida == "ccc")
        .collect();
    assert!(!registrados.is_empty());
    assert!(registrados.iter().all(|ev| ev.tipo != TipoEvento::Gate));
}
