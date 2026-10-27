use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread;

use root::observe;
use root::tipos::{Evento, TipoEvento, Via};
use serde_json::{Value, json};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude-sesion.json")
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

#[test]
fn observe_16_escritores_concurrentes_no_rompen_lineas() {
    const HILOS: usize = 16;
    const POR_HILO: usize = 50;
    let dir = tempfile::tempdir().expect("temp dir");
    let ruta = dir.path().join("eventos.jsonl");
    let relleno = "x".repeat(4096);

    let manejadores: Vec<_> = (0..HILOS)
        .map(|h| {
            let ruta = ruta.clone();
            let relleno = relleno.clone();
            thread::spawn(move || {
                for i in 0..POR_HILO {
                    let mut ev = Evento {
                        corrida: format!("h{h}"),
                        ..Evento::default()
                    };
                    ev.extra
                        .insert("detalle".into(), json!(format!("{i}-{relleno}")));
                    observe::registrar_en(&ruta, &ev).expect("registra sin error");
                }
            })
        })
        .collect();
    for manejador in manejadores {
        manejador.join().expect("ningún hilo entra en pánico");
    }

    let texto = fs::read_to_string(&ruta).expect("lee el archivo");
    let eventos: Vec<Evento> = texto
        .lines()
        .map(|linea| serde_json::from_str(linea).expect("cada línea parsea completa"))
        .collect();
    assert_eq!(eventos.len(), HILOS * POR_HILO);
}

#[test]
fn observe_fixture_claude_da_llamada_por_modelo_y_sesion_sin_costo() {
    let eventos = observe::claude::leer_sesion_claude(&fixture(), "c1").expect("el fixture parsea");
    assert_eq!(eventos.len(), 2);

    let crudo: Value =
        serde_json::from_str(&fs::read_to_string(fixture()).expect("lee el fixture"))
            .expect("el fixture es JSON");
    let llamada = eventos
        .iter()
        .find(|ev| ev.tipo == TipoEvento::Llamada)
        .expect("hay una Llamada");
    assert_eq!(
        llamada.cost_usd,
        crudo["total_cost_usd"].as_f64().expect("costo")
    );
    assert_eq!(llamada.modelo.as_deref(), Some("claude-haiku-5-5"));
    assert_eq!(llamada.familia.as_deref(), Some("anthropic"));
    assert_eq!(llamada.via, Some(Via::Cupo));
    assert_eq!(llamada.corrida, "c1");
    assert_eq!(llamada.in_tokens, 6 + 47090 + 131756);
    assert_eq!(llamada.out_tokens, 4912);
    assert_eq!(llamada.latency_s, 0.0);

    let sesion = eventos
        .iter()
        .find(|ev| ev.tipo == TipoEvento::Sesion)
        .expect("hay un Sesion");
    assert_eq!(sesion.cost_usd, 0.0);
    assert_eq!(sesion.via, Some(Via::Cupo));
    assert_eq!(sesion.corrida, "c1");
    assert!((sesion.latency_s - 31.342).abs() < 1e-9);
    assert_eq!(sesion.extra["num_turns"], 8);
    assert_eq!(
        sesion.extra["session_id"],
        "7504bf9b-0b4f-4248-a635-bdea1da9049d"
    );
    assert_eq!(sesion.extra["total_cost_usd"], crudo["total_cost_usd"]);
    assert!(sesion.extra.get("modelUsage").is_none());
}

#[test]
fn observe_claude_dos_modelos_da_una_llamada_por_modelo_con_su_costo() {
    let ruta =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude-sesion-2modelos.json");
    let eventos = observe::claude::leer_sesion_claude(&ruta, "c2").expect("el fixture parsea");
    assert_eq!(eventos.len(), 3);

    let crudo: Value = serde_json::from_str(&fs::read_to_string(&ruta).expect("lee el fixture"))
        .expect("el fixture es JSON");
    let suma: f64 = eventos.iter().map(|ev| ev.cost_usd).sum();
    assert_eq!(suma, crudo["total_cost_usd"].as_f64().expect("costo"));

    let llamadas: Vec<&Evento> = eventos
        .iter()
        .filter(|ev| ev.tipo == TipoEvento::Llamada)
        .collect();
    assert_eq!(llamadas.len(), 2);
    let de = |modelo: &str| {
        llamadas
            .iter()
            .find(|ev| ev.modelo.as_deref() == Some(modelo))
            .copied()
            .expect("hay llamada de ese modelo")
    };
    let sonnet = de("claude-sonnet-5-5");
    assert_eq!(sonnet.cost_usd, 0.0421875);
    assert_eq!(sonnet.in_tokens, 7 + 20000 + 1000);
    assert_eq!(sonnet.out_tokens, 700);
    let haiku = de("claude-haiku-5-5");
    assert_eq!(haiku.cost_usd, 0.0032417);
    assert_eq!(haiku.via, Some(Via::Cupo));
    assert_eq!(haiku.in_tokens, 2 + 10500 + 200);
}

#[test]
fn observe_float_sale_igual_tras_ida_y_vuelta_por_json() {
    let costos = [
        0.013192160000000001,
        0.30000000000000004,
        0.0000123456789012345,
        3.141592653589793e-5,
        1.0000000000000002,
    ];
    for costo in costos {
        let ev = Evento {
            cost_usd: costo,
            ..Evento::default()
        };
        let texto = serde_json::to_string(&ev).expect("serializa el evento");
        let vuelta: Evento = serde_json::from_str(&texto).expect("deserializa el evento");
        assert_eq!(
            vuelta.cost_usd, costo,
            "{costo:?} cambió al volver por JSON"
        );
    }
}

#[test]
fn observe_claude_acepta_un_objeto_por_linea() {
    let dir = tempfile::tempdir().expect("temp dir");
    let ruta = dir.path().join("salida.json");
    let objeto = fs::read_to_string(fixture()).expect("lee el fixture");
    fs::write(&ruta, format!("{objeto}{objeto}")).expect("escribe dos objetos");
    let eventos = observe::claude::leer_sesion_claude(&ruta, "c1").expect("parsea los dos");
    // Cada objeto da una Llamada y un Sesion.
    assert_eq!(eventos.len(), 4);
}

#[test]
fn observe_json_invalido_da_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let ruta = dir.path().join("salida.json");
    fs::write(&ruta, "{\"total_cost_usd\": 0.1,").expect("escribe JSON roto");
    assert!(observe::claude::leer_sesion_claude(&ruta, "c1").is_err());
}

#[test]
fn observe_cli_evento_dos_veces_y_resumen_cuenta_intervenciones() {
    let home = tempfile::tempdir().expect("temp dir");
    let plan = root_con_home(
        home.path(),
        &[
            "observa",
            "evento",
            "--corrida",
            "c1",
            "--tipo",
            "plan-aprobado",
        ],
    );
    assert_eq!(codigo(&plan), 0);
    for detalle in ["ajusta el alcance", "cambia el modelo"] {
        let humano = root_con_home(
            home.path(),
            &[
                "observa",
                "evento",
                "--corrida",
                "c1",
                "--tipo",
                "humano",
                "--detalle",
                detalle,
            ],
        );
        assert_eq!(codigo(&humano), 0);
    }

    let salida = root_con_home(
        home.path(),
        &["--json", "observa", "resumen", "--corrida", "c1"],
    );
    assert_eq!(codigo(&salida), 0);
    let resumen: Value = serde_json::from_slice(&salida.stdout).expect("stdout es JSON");
    assert_eq!(resumen["intervenciones"], 2);
    assert_eq!(resumen["invalida"], false);
    assert!(home.path().join("eventos.jsonl").exists());
}

#[test]
fn observe_cli_claude_registra_el_fixture_y_el_resumen_suma_cupo() {
    let home = tempfile::tempdir().expect("temp dir");
    let registro = root_con_home(
        home.path(),
        &[
            "observa",
            "claude",
            "--archivo",
            fixture().to_str().expect("ruta UTF-8"),
            "--corrida",
            "c9",
        ],
    );
    assert_eq!(codigo(&registro), 0);

    let salida = root_con_home(
        home.path(),
        &["--json", "observa", "resumen", "--corrida", "c9"],
    );
    assert_eq!(codigo(&salida), 0);
    let resumen: Value = serde_json::from_slice(&salida.stdout).expect("stdout es JSON");
    let usd_cupo = resumen["usd_cupo"].as_f64().expect("usd_cupo es número");
    assert_eq!(usd_cupo, 0.013192160000000001);
    assert_eq!(resumen["turnos"], 8);
}
