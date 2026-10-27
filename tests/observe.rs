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
    Command::new(env!("CARGO_BIN_EXE_root"))
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
fn observe_fixture_claude_da_evento_sesion_con_costo_de_cupo() {
    let eventos = observe::claude::leer_sesion_claude(&fixture(), "c1").expect("el fixture parsea");
    assert_eq!(eventos.len(), 1);
    let ev = &eventos[0];

    let crudo: Value =
        serde_json::from_str(&fs::read_to_string(fixture()).expect("lee el fixture"))
            .expect("el fixture es JSON");
    assert_eq!(
        ev.cost_usd,
        crudo["total_cost_usd"].as_f64().expect("costo")
    );
    assert_eq!(ev.tipo, TipoEvento::Sesion);
    assert_eq!(ev.modelo.as_deref(), Some("claude-haiku-5-5"));
    assert_eq!(ev.via, Some(Via::Cupo));
    assert_eq!(ev.corrida, "c1");
    assert_eq!(ev.in_tokens, 6 + 47090 + 131756);
    assert_eq!(ev.out_tokens, 4912);
    assert!((ev.latency_s - 31.342).abs() < 1e-9);
    assert_eq!(ev.extra["num_turns"], 8);
    assert_eq!(
        ev.extra["session_id"],
        "7504bf9b-0b4f-4248-a635-bdea1da9049d"
    );
    assert_eq!(
        ev.extra["modelUsage"]["claude-haiku-5-5"]["outputTokens"],
        4912
    );
}

#[test]
fn observe_claude_acepta_un_objeto_por_linea() {
    let dir = tempfile::tempdir().expect("temp dir");
    let ruta = dir.path().join("salida.json");
    let objeto = fs::read_to_string(fixture()).expect("lee el fixture");
    fs::write(&ruta, format!("{objeto}{objeto}")).expect("escribe dos objetos");
    let eventos = observe::claude::leer_sesion_claude(&ruta, "c1").expect("parsea los dos");
    assert_eq!(eventos.len(), 2);
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
            "--salida",
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
    // serde_json sin `float_roundtrip` puede errar 1 ULP al leer decimales.
    let usd_cupo = resumen["usd_cupo"].as_f64().expect("usd_cupo es número");
    assert!((usd_cupo - 0.013192160000000001).abs() < 1e-15);
    assert_eq!(resumen["turnos"], 8);
}
