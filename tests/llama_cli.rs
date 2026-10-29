//! `r00t llama` contra el servidor HTTP falso y un `ROOT_HOME` temporal.

mod common;

use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use common::{Servidor, sse_deepseek};
use root::observe;
use root::tipos::{Evento, TipoEvento, Via};
use serde_json::Value;

const CLAVE: &str = "sk-test-SECRETO-LLAMA";
fn deepseek_ok() -> String {
    sse_deepseek("hola", 10, 5, "stop")
}

fn ahora() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("reloj")
        .as_secs_f64()
}

fn codigo(salida: &Output) -> i32 {
    salida
        .status
        .code()
        .expect("el proceso no terminó con código")
}

/// Escribe `root.toml` con la URL falsa, un precio opcional y el prompt de prueba.
fn preparar(home: &Path, url: &str, precios: &str) {
    let texto = format!(
        "[proveedores]\ndeepseek_url = \"{url}\"\nanthropic_url = \"{url}\"\nespera_base_ms = 1\n\n{precios}"
    );
    fs::write(home.join("root.toml"), texto).expect("root.toml");
    fs::write(home.join("prompt.txt"), "hola").expect("prompt");
}

/// Siembra un gasto de API de `usd` dólares hace un minuto.
fn gasto_api(home: &Path, usd: f64) {
    let ev = Evento {
        ts: ahora() - 60.0,
        corrida: "previa".into(),
        tipo: TipoEvento::Llamada,
        via: Some(Via::Api),
        cost_usd: usd,
        ..Evento::default()
    };
    observe::registrar_en(&home.join("eventos.jsonl"), &ev).expect("siembra");
}

fn llama(home: &Path, extra: &[&str]) -> Output {
    llama_con_anthropic(home, extra, None)
}

/// `r00t llama` de rol juzga. Con `anthropic`, la clave de Anthropic va en el entorno; si no, no hay.
fn llama_con_anthropic(home: &Path, extra: &[&str], anthropic: Option<&str>) -> Output {
    let mut orden = Command::new(env!("CARGO_BIN_EXE_r00t"));
    orden.env("ROOT_HOME", home).env("DEEPSEEK_API_KEY", CLAVE);
    match anthropic {
        Some(clave) => orden.env("ANTHROPIC_API_KEY", clave),
        None => orden.env_remove("ANTHROPIC_API_KEY"),
    };
    orden
        .args([
            "llama",
            "--rol",
            "juzga",
            "--tipo",
            "diseno",
            "--corrida",
            "c1",
        ])
        .arg("--prompt-file")
        .arg(home.join("prompt.txt"))
        .args(extra)
        .output()
        .expect("no se pudo ejecutar el binario root")
}

fn eventos(home: &Path) -> Vec<Evento> {
    let texto = fs::read_to_string(home.join("eventos.jsonl")).unwrap_or_default();
    texto
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("evento válido"))
        .collect()
}

/// Precio de `deepseek-flash` (el modelo del juez) con costo de salida alto: 1000 tokens cuestan 3 USD.
const PRECIO_CARO: &str = "[precios.deepseek-flash]\nusd_entrada = 0.0\nusd_salida = 3000.0\n";
/// Precio de `deepseek-flash` con costo de salida bajo: 1000 tokens cuestan 0,000001 USD.
const PRECIO_BARATO: &str = "[precios.deepseek-flash]\nusd_entrada = 0.0\nusd_salida = 0.001\n";

#[test]
fn llama_cli_tope_rechaza_la_llamada_sin_enviarla() {
    let srv = Servidor::arranca(vec![(200, deepseek_ok())]);
    let home = tempfile::tempdir().expect("home");
    preparar(home.path(), &srv.url, PRECIO_CARO);
    gasto_api(home.path(), 98.0);

    let salida = llama(home.path(), &["--max-tokens", "1000"]);

    assert_eq!(codigo(&salida), 3);
    assert_eq!(srv.peticiones(), 0, "la llamada no debe enviarse");
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(stderr.contains("margen"), "stderr: {stderr}");
    let gates: Vec<Evento> = eventos(home.path())
        .into_iter()
        .filter(|ev| ev.tipo == TipoEvento::Gate)
        .collect();
    assert_eq!(gates.len(), 1);
    assert_eq!(gates[0].fase, "tope");
    assert_eq!(gates[0].extra["modelo"], "deepseek-flash");
    let estimado = gates[0].extra["estimado"].as_f64().expect("estimado");
    let margen = gates[0].extra["margen"].as_f64().expect("margen");
    assert!((estimado - 3.0).abs() < 1e-9, "estimado {estimado}");
    assert!((margen - 2.0).abs() < 1e-9, "margen {margen}");
}

#[test]
fn llama_cli_dentro_del_margen_envia_imprime_y_registra() {
    let srv = Servidor::arranca(vec![(200, deepseek_ok())]);
    let home = tempfile::tempdir().expect("home");
    preparar(home.path(), &srv.url, PRECIO_BARATO);
    gasto_api(home.path(), 98.0);

    let salida = llama(home.path(), &["--max-tokens", "1000"]);

    assert_eq!(codigo(&salida), 0);
    assert_eq!(srv.peticiones(), 1);
    let respuesta: Value = serde_json::from_slice(&salida.stdout).expect("stdout es JSON");
    assert_eq!(respuesta["texto"], "hola");
    let llamadas: Vec<Evento> = eventos(home.path())
        .into_iter()
        .filter(|ev| ev.tipo == TipoEvento::Llamada && ev.corrida == "c1")
        .collect();
    assert_eq!(llamadas.len(), 1);
    assert_eq!(llamadas[0].via, Some(Via::Api));
    let clave = CLAVE.as_bytes();
    for (nombre, bytes) in [
        ("stdout", &salida.stdout),
        ("stderr", &salida.stderr),
        (
            "eventos",
            &fs::read(home.path().join("eventos.jsonl")).expect("eventos"),
        ),
    ] {
        assert!(
            !bytes.windows(clave.len()).any(|w| w == clave),
            "la clave apareció en {nombre}"
        );
    }
}

#[test]
fn llama_cli_modelo_sin_precio_rechaza_con_gate() {
    let srv = Servidor::arranca(vec![(200, deepseek_ok())]);
    let home = tempfile::tempdir().expect("home");
    // El juez usa un modelo de la lista de razonamiento que no tiene precio por defecto: Root lo
    // rechaza antes de enviar.
    preparar(
        home.path(),
        &srv.url,
        "[ruteo.juzga]\nproveedor = \"anthropic\"\nmodelo = \"claude-opus-5-5\"\n",
    );

    // Con la clave de Anthropic, el juez no pasa a su respaldo y llega al chequeo del precio.
    let salida = llama_con_anthropic(home.path(), &["--max-tokens", "1000"], Some(CLAVE));

    assert_eq!(codigo(&salida), 3);
    assert_eq!(srv.peticiones(), 0);
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(
        stderr.contains("sin precio en root.toml para claude-opus-5-5"),
        "stderr: {stderr}"
    );
    let gates: Vec<Evento> = eventos(home.path())
        .into_iter()
        .filter(|ev| ev.tipo == TipoEvento::Gate)
        .collect();
    assert_eq!(gates.len(), 1);
    assert_eq!(gates[0].fase, "tope");
    assert_eq!(gates[0].extra["modelo"], "claude-opus-5-5");
    assert!(gates[0].extra["estimado"].is_null());
}

#[test]
fn llama_cli_rol_de_cupo_sale_con_2_sin_pedidos() {
    let srv = Servidor::arranca(vec![(200, deepseek_ok())]);
    let home = tempfile::tempdir().expect("home");
    preparar(home.path(), &srv.url, PRECIO_BARATO);

    let salida = Command::new(env!("CARGO_BIN_EXE_r00t"))
        .env("ROOT_HOME", home.path())
        .env("DEEPSEEK_API_KEY", CLAVE)
        .args([
            "llama",
            "--rol",
            "construye",
            "--tipo",
            "bulk",
            "--corrida",
            "c1",
        ])
        .arg("--prompt-file")
        .arg(home.path().join("prompt.txt"))
        .output()
        .expect("no se pudo ejecutar el binario root");

    assert_eq!(codigo(&salida), 2);
    assert_eq!(srv.peticiones(), 0);
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(stderr.contains("claude -p"), "stderr: {stderr}");
}

#[test]
fn llama_cli_prompt_inexistente_sale_con_2() {
    let srv = Servidor::arranca(vec![(200, deepseek_ok())]);
    let home = tempfile::tempdir().expect("home");
    preparar(home.path(), &srv.url, PRECIO_BARATO);
    let salida = Command::new(env!("CARGO_BIN_EXE_r00t"))
        .env("ROOT_HOME", home.path())
        .args([
            "llama",
            "--rol",
            "juzga",
            "--tipo",
            "diseno",
            "--corrida",
            "c1",
        ])
        .arg("--prompt-file")
        .arg(home.path().join("no-existe.txt"))
        .output()
        .expect("no se pudo ejecutar el binario root");
    assert_eq!(codigo(&salida), 2);
    assert_eq!(srv.peticiones(), 0);
}

#[test]
fn llama_cli_reserva_no_escribible_no_envia_nada() {
    let srv = Servidor::arranca(vec![(200, deepseek_ok())]);
    let home = tempfile::tempdir().expect("home");
    preparar(home.path(), &srv.url, PRECIO_BARATO);
    // `eventos.jsonl` es un enlace a una carpeta que no existe: el log se lee como vacío, pero
    // no se puede escribir. La reserva no se escribe y la llamada no sale.
    std::os::unix::fs::symlink(
        home.path()
            .join("carpeta-inexistente")
            .join("eventos.jsonl"),
        home.path().join("eventos.jsonl"),
    )
    .expect("enlace");

    let salida = llama(home.path(), &["--max-tokens", "1000"]);

    assert_eq!(codigo(&salida), 3);
    assert_eq!(srv.peticiones(), 0, "sin reserva escrita no se envía");
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(stderr.contains("gasto de API"), "stderr: {stderr}");
}

/// Respuesta de DeepSeek con 1000 tokens de salida: a 600 USD por millón, cuesta 0,6 USD.
fn deepseek_1000() -> String {
    sse_deepseek("hola", 10, 1000, "stop")
}
/// Precio de `deepseek-flash` con 1000 tokens de salida a 0,6 USD (peor caso de cada pedido).
const PRECIO_SEISCIENTOS: &str =
    "[precios.deepseek-flash]\nusd_entrada = 0.0\nusd_salida = 600.0\n";

#[test]
fn llama_cli_cuatro_procesos_no_pasan_el_tope_de_api() {
    // Cada pedido tarda 300 ms: los cuatro procesos llegan mientras el primero sigue en vuelo.
    let srv = Servidor::arranca_lento(vec![(200, deepseek_1000()); 4], Duration::from_millis(300));
    let home = tempfile::tempdir().expect("home");
    preparar(home.path(), &srv.url, PRECIO_SEISCIENTOS);
    // Tope de 100 USD con 99 gastados: margen de 1 USD, para uno de los cuatro pedidos.
    gasto_api(home.path(), 99.0);

    let hilos: Vec<_> = (0..4)
        .map(|_| {
            let home = home.path().to_path_buf();
            thread::spawn(move || llama(&home, &["--max-tokens", "1000"]))
        })
        .collect();
    let salidas: Vec<Output> = hilos.into_iter().map(|h| h.join().expect("hilo")).collect();

    let enviados = salidas.iter().filter(|s| codigo(s) == 0).count();
    assert_eq!(enviados, 1, "solo entra un pedido en el margen");
    assert!(
        srv.peticiones() <= 1,
        "pedidos recibidos: {}",
        srv.peticiones()
    );
    let gasto = observe::uso::uso_en(
        &home.path().join("eventos.jsonl"),
        ahora(),
        observe::uso::offset_local(),
        5,
    )
    .expect("lee el gasto")
    .api_mes_usd;
    assert!(
        gasto <= 100.0 + 1e-9,
        "el gasto de API pasó el tope: {gasto}"
    );
}

#[test]
fn llama_cli_log_ilegible_no_envia_nada() {
    let srv = Servidor::arranca(vec![(200, deepseek_ok())]);
    let home = tempfile::tempdir().expect("home");
    preparar(home.path(), &srv.url, PRECIO_BARATO);
    // Sin poder leer el log no se conoce el gasto: la llamada no se envía.
    fs::create_dir(home.path().join("eventos.jsonl")).expect("log ilegible");

    let salida = llama(home.path(), &["--max-tokens", "1000"]);

    assert_eq!(codigo(&salida), 3);
    assert_eq!(srv.peticiones(), 0);
}

/// `r00t llama` con un rol dado y el prompt de prueba.
fn llama_con_rol(home: &Path, rol: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_r00t"))
        .env("ROOT_HOME", home)
        .env("DEEPSEEK_API_KEY", CLAVE)
        .env_remove("ANTHROPIC_API_KEY")
        .args([
            "llama",
            "--rol",
            rol,
            "--tipo",
            "diseno",
            "--corrida",
            "c1",
            "--prompt-file",
        ])
        .arg(home.join("prompt.txt"))
        .output()
        .expect("no se pudo ejecutar el binario root")
}

#[test]
fn llama_cli_roles_de_api_con_tope_de_api_lleno_registran_gate_y_salen_con_3() {
    for rol in ["delega", "verifica", "juzga"] {
        let srv = Servidor::arranca(vec![(200, deepseek_ok())]);
        let home = tempfile::tempdir().expect("home");
        preparar(home.path(), &srv.url, "");
        gasto_api(home.path(), 100.0);

        let salida = llama_con_rol(home.path(), rol);

        assert_eq!(codigo(&salida), 3, "rol {rol}");
        assert_eq!(srv.peticiones(), 0, "rol {rol} no debe enviar");
        let stderr = String::from_utf8_lossy(&salida.stderr);
        assert!(
            stderr.contains("tope de API"),
            "rol {rol}, stderr: {stderr}"
        );
        let gates: Vec<Evento> = eventos(home.path())
            .into_iter()
            .filter(|ev| ev.tipo == TipoEvento::Gate && ev.fase == "presupuesto")
            .collect();
        assert_eq!(
            gates.len(),
            1,
            "rol {rol} debe registrar un Gate de presupuesto"
        );
    }
}

#[test]
fn llama_cli_planifica_con_tope_de_api_lleno_sigue_con_codigo_2() {
    let srv = Servidor::arranca(vec![]);
    let home = tempfile::tempdir().expect("home");
    preparar(home.path(), &srv.url, "");
    gasto_api(home.path(), 100.0);

    let salida = llama_con_rol(home.path(), "planifica");

    assert_eq!(codigo(&salida), 2);
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(stderr.contains("va por cupo"), "stderr: {stderr}");
    assert!(
        eventos(home.path())
            .iter()
            .all(|ev| ev.tipo != TipoEvento::Gate)
    );
}

#[test]
fn llama_cli_sin_root_toml_tiene_precio_para_el_juez_y_no_dice_sin_precio() {
    let home = tempfile::tempdir().expect("home");
    fs::write(home.path().join("prompt.txt"), "hola").expect("prompt");
    // Sin clave, la llamada se detiene después del chequeo de precio y antes de enviar nada.
    let salida = Command::new(env!("CARGO_BIN_EXE_r00t"))
        .env("ROOT_HOME", home.path())
        .env_remove("DEEPSEEK_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .args([
            "llama",
            "--rol",
            "juzga",
            "--tipo",
            "diseno",
            "--corrida",
            "c1",
        ])
        .arg("--prompt-file")
        .arg(home.path().join("prompt.txt"))
        .output()
        .expect("no se pudo ejecutar el binario root");
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(!stderr.contains("sin precio"), "stderr: {stderr}");
    assert!(stderr.contains("falta la clave"), "stderr: {stderr}");
    assert_eq!(codigo(&salida), 3);
}
