//! `r00t mcp` por stdio, contra el binario real y un `ROOT_HOME` temporal.
//! El servidor HTTP falso de `tests/common` responde a los pedidos de `root_llama`.

mod common;

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use common::{Servidor, rompe_el_log_en_el_pedido};
use root::observe;
use root::tipos::{Evento, TipoEvento, Via};
use serde_json::{Value, json};

const CLAVE: &str = "sk-test-SECRETO-MCP";
const DEEPSEEK_OK: &str = r#"{"choices":[{"message":{"content":"hola"}}],"usage":{"prompt_tokens":10,"completion_tokens":5}}"#;
const PRECIO_CARO: &str = "[precios.deepseek-reasoner]\nusd_entrada = 0.0\nusd_salida = 3000.0\n";
const PRECIO_BARATO: &str = "[precios.deepseek-reasoner]\nusd_entrada = 0.0\nusd_salida = 0.001\n";
const TAREA_MINIMA: &str = r#"
nombre = "memora"
lenguaje = "rust"
imagen = "root-oraculo-rust:1"
spec = "SPEC.md"

[contrato]
cmd = "pytest -q"

[oculto]
cmd = "pytest -q /oraculo/oculto"
"#;

fn ahora() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("reloj")
        .as_secs_f64()
}

/// Arranca `r00t mcp`, escribe `entrada` por stdin, cierra stdin y espera la salida.
fn mcp(home: &Path, entrada: &str) -> Output {
    let mut hijo = Command::new(env!("CARGO_BIN_EXE_r00t"))
        .arg("mcp")
        .env("ROOT_HOME", home)
        .env("DEEPSEEK_API_KEY", CLAVE)
        .env_remove("ANTHROPIC_API_KEY")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("no se pudo ejecutar el binario root");
    let mut stdin = hijo.stdin.take().expect("stdin");
    let _ = stdin.write_all(entrada.as_bytes());
    drop(stdin);
    hijo.wait_with_output().expect("espera al proceso")
}

fn linea(valor: Value) -> String {
    format!("{valor}\n")
}

fn llamada(id: u64, nombre: &str, argumentos: Value) -> String {
    linea(json!({
        "jsonrpc": "2.0", "id": id, "method": "tools/call",
        "params": {"name": nombre, "arguments": argumentos}
    }))
}

/// Cada línea de stdout debe ser un objeto JSON válido. Devuelve las respuestas en orden.
fn respuestas(salida: &Output) -> Vec<Value> {
    String::from_utf8_lossy(&salida.stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            serde_json::from_str(l).unwrap_or_else(|e| panic!("stdout no es JSON: {l:?}: {e}"))
        })
        .collect()
}

/// Una sola llamada a una herramienta. Devuelve la respuesta completa.
fn herramienta(home: &Path, nombre: &str, argumentos: Value) -> Value {
    let salida = mcp(home, &llamada(1, nombre, argumentos));
    let mut r = respuestas(&salida);
    assert_eq!(r.len(), 1, "respuestas: {r:?}");
    r.remove(0)
}

/// El texto de `content[0]` parseado como JSON.
fn texto_json(resp: &Value) -> Value {
    let texto = resp["result"]["content"][0]["text"]
        .as_str()
        .expect("content[0].text");
    serde_json::from_str(texto).expect("el texto es JSON")
}

fn texto_plano(resp: &Value) -> String {
    resp["result"]["content"][0]["text"]
        .as_str()
        .expect("content[0].text")
        .to_string()
}

fn es_error_de_herramienta(resp: &Value) -> bool {
    resp["result"]["isError"] == true
}

fn codigo_rpc(resp: &Value) -> i64 {
    resp["error"]["code"].as_i64().expect("código de error")
}

fn root_cli(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_r00t"))
        .env("ROOT_HOME", home)
        .args(args)
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

fn sembrar_gasto(home: &Path, via: Via, ts: f64, cost_usd: f64) {
    let ev = Evento {
        ts,
        corrida: "previa".into(),
        tipo: TipoEvento::Llamada,
        via: Some(via),
        cost_usd,
        ..Evento::default()
    };
    observe::registrar_en(&home.join("eventos.jsonl"), &ev).expect("siembra");
}

/// Siembra un gasto de API de `usd` dólares hace un minuto.
fn gasto_api(home: &Path, usd: f64) {
    sembrar_gasto(home, Via::Api, ahora() - 60.0, usd);
}

/// Escribe `root.toml` con la URL falsa, un precio y el prompt de prueba.
fn preparar(home: &Path, url: &str, precios: &str) {
    let texto = format!(
        "[proveedores]\ndeepseek_url = \"{url}\"\nanthropic_url = \"{url}\"\nespera_base_ms = 1\n\n{precios}"
    );
    fs::write(home.join("root.toml"), texto).expect("root.toml");
    fs::write(home.join("prompt.txt"), "hola").expect("prompt");
}

fn tiene_la_clave(bytes: &[u8]) -> bool {
    let clave = CLAVE.as_bytes();
    bytes.windows(clave.len()).any(|w| w == clave)
}

#[test]
fn mcp_initialize_responde_server_info_y_capacidad_de_herramientas() {
    let home = tempfile::tempdir().expect("home");
    let salida = mcp(
        home.path(),
        &linea(json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {"protocolVersion": "2025-06-18"}
        })),
    );
    assert_eq!(salida.status.code(), Some(0));
    let r = respuestas(&salida);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0]["id"], 1);
    assert_eq!(r[0]["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(r[0]["result"]["serverInfo"]["name"], "root");
    assert_eq!(
        r[0]["result"]["serverInfo"]["version"],
        env!("CARGO_PKG_VERSION")
    );
    assert!(r[0]["result"]["capabilities"]["tools"].is_object());
}

#[test]
fn mcp_notificacion_initialized_no_produce_salida() {
    let home = tempfile::tempdir().expect("home");
    let entrada = linea(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))
        + &linea(json!({"jsonrpc": "2.0", "id": 2, "method": "ping"}));
    let salida = mcp(home.path(), &entrada);
    let r = respuestas(&salida);
    assert_eq!(r.len(), 1, "solo el ping responde: {r:?}");
    assert_eq!(r[0]["id"], 2);
}

#[test]
fn mcp_ping_responde_objeto_vacio() {
    let home = tempfile::tempdir().expect("home");
    let salida = mcp(
        home.path(),
        &linea(json!({"jsonrpc": "2.0", "id": 3, "method": "ping"})),
    );
    let r = respuestas(&salida);
    assert_eq!(r[0]["result"], json!({}));
}

#[test]
fn mcp_tools_list_devuelve_exactamente_las_cuatro_herramientas() {
    let home = tempfile::tempdir().expect("home");
    let salida = mcp(
        home.path(),
        &linea(json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"})),
    );
    let r = respuestas(&salida);
    let tools = r[0]["result"]["tools"].as_array().expect("tools");
    let mut nombres: Vec<&str> = tools
        .iter()
        .map(|t| t["name"].as_str().expect("nombre"))
        .collect();
    nombres.sort_unstable();
    assert_eq!(
        nombres,
        ["root_evaluar", "root_llama", "root_registrar", "root_route"]
    );
    let esperados = [
        ("root_route", vec!["rol", "tipo"]),
        ("root_llama", vec!["prompt", "rol", "tipo"]),
        ("root_evaluar", vec!["repo", "tarea"]),
        ("root_registrar", vec!["corrida", "tipo"]),
    ];
    for (nombre, requeridos) in esperados {
        let tool = tools
            .iter()
            .find(|t| t["name"] == nombre)
            .unwrap_or_else(|| panic!("falta {nombre}"));
        assert!(
            tool["description"].as_str().is_some_and(|d| !d.is_empty()),
            "{nombre} sin descripción"
        );
        assert_eq!(tool["inputSchema"]["type"], "object", "{nombre}");
        let mut req: Vec<&str> = tool["inputSchema"]["required"]
            .as_array()
            .expect("required")
            .iter()
            .map(|v| v.as_str().expect("texto"))
            .collect();
        req.sort_unstable();
        assert_eq!(req, requeridos, "required de {nombre}");
    }
}

#[test]
fn mcp_route_juzga_diseno_devuelve_el_mismo_json_que_la_cli() {
    let home = tempfile::tempdir().expect("home");
    let cli = root_cli(
        home.path(),
        &["--json", "route", "--rol", "juzga", "--tipo", "diseno"],
    );
    assert_eq!(cli.status.code(), Some(0));
    let esperado: Value = serde_json::from_slice(&cli.stdout).expect("stdout es JSON");

    let resp = herramienta(
        home.path(),
        "root_route",
        json!({"rol": "juzga", "tipo": "diseno"}),
    );
    assert_eq!(resp["result"]["isError"], false);
    let eleccion = texto_json(&resp);
    assert_eq!(eleccion, esperado);
    assert_eq!(eleccion["razonamiento"], true);
    assert_eq!(eleccion["via"], "api");
}

#[test]
fn mcp_route_checkeable_da_oraculo() {
    let home = tempfile::tempdir().expect("home");
    let resp = herramienta(
        home.path(),
        "root_route",
        json!({"rol": "organiza", "tipo": "bulk", "checkeable": true}),
    );
    assert_eq!(texto_json(&resp)["via"], "oraculo");
}

#[test]
fn mcp_route_sin_presupuesto_es_error_de_herramienta_y_registra_gate() {
    let home = tempfile::tempdir().expect("home");
    let ts = ahora();
    sembrar_gasto(home.path(), Via::Cupo, ts - 600.0, 35.0);
    sembrar_gasto(home.path(), Via::Api, ts - 60.0, 100.0);

    let resp = herramienta(
        home.path(),
        "root_route",
        json!({"rol": "organiza", "tipo": "bulk"}),
    );
    assert!(es_error_de_herramienta(&resp), "respuesta: {resp}");
    assert!(resp.get("error").is_none());
    let texto = texto_plano(&resp);
    assert!(texto.contains("sin presupuesto"), "texto: {texto}");
    assert!(texto.contains("reabre"), "texto: {texto}");

    let gates: Vec<Evento> = eventos(home.path())
        .into_iter()
        .filter(|ev| ev.tipo == TipoEvento::Gate)
        .collect();
    assert_eq!(gates.len(), 1);
    assert_eq!(gates[0].corrida, "route");
    assert_eq!(gates[0].fase, "presupuesto");
    assert_eq!(gates[0].extra["rol"], "organiza");
}

#[test]
fn mcp_route_rol_invalido_da_32602() {
    let home = tempfile::tempdir().expect("home");
    let resp = herramienta(
        home.path(),
        "root_route",
        json!({"rol": "inventado", "tipo": "x"}),
    );
    assert_eq!(codigo_rpc(&resp), -32602);
}

#[test]
fn mcp_route_tokens_no_entero_da_32602() {
    let home = tempfile::tempdir().expect("home");
    let resp = herramienta(
        home.path(),
        "root_route",
        json!({"rol": "juzga", "tipo": "diseno", "tokens": "mucho"}),
    );
    assert_eq!(codigo_rpc(&resp), -32602);
}

#[test]
fn mcp_registrar_escribe_el_evento_y_el_resumen_lo_cuenta() {
    let home = tempfile::tempdir().expect("home");
    let r1 = herramienta(
        home.path(),
        "root_registrar",
        json!({"corrida": "c9", "tipo": "plan-aprobado"}),
    );
    assert_eq!(
        texto_json(&r1),
        json!({"registrado": true, "corrida": "c9", "tipo": "plan-aprobado"})
    );
    let r2 = herramienta(
        home.path(),
        "root_registrar",
        json!({"corrida": "c9", "tipo": "humano", "detalle": "ajusta"}),
    );
    assert_eq!(texto_json(&r2)["registrado"], true);

    let evs = eventos(home.path());
    assert_eq!(evs.len(), 2);
    assert_eq!(evs[0].tipo, TipoEvento::PlanAprobado);
    assert_eq!(evs[1].tipo, TipoEvento::Humano);
    assert_eq!(evs[1].extra["detalle"], "ajusta");

    let resumen = root_cli(
        home.path(),
        &["--json", "observa", "resumen", "--corrida", "c9"],
    );
    let r: Value = serde_json::from_slice(&resumen.stdout).expect("resumen JSON");
    assert_eq!(r["intervenciones"], 1);
}

#[test]
fn mcp_llama_envia_una_vez_y_registra_con_corrida_mcp() {
    let srv = Servidor::arranca(vec![(200, DEEPSEEK_OK)]);
    let home = tempfile::tempdir().expect("home");
    preparar(home.path(), &srv.url, PRECIO_BARATO);
    gasto_api(home.path(), 98.0);

    let salida = mcp(
        home.path(),
        &llamada(
            1,
            "root_llama",
            json!({"rol": "juzga", "tipo": "diseno", "prompt": "hola", "max_tokens": 1000}),
        ),
    );
    let r = respuestas(&salida);
    assert_eq!(r[0]["result"]["isError"], false, "respuesta: {}", r[0]);
    assert_eq!(texto_json(&r[0])["texto"], "hola");
    assert_eq!(srv.peticiones(), 1);

    let llamadas: Vec<Evento> = eventos(home.path())
        .into_iter()
        .filter(|ev| ev.tipo == TipoEvento::Llamada && ev.corrida == "mcp")
        .collect();
    assert_eq!(llamadas.len(), 1);
    assert_eq!(llamadas[0].via, Some(Via::Api));

    assert!(!tiene_la_clave(&salida.stdout), "la clave salió por stdout");
    assert!(!tiene_la_clave(&salida.stderr), "la clave salió por stderr");
    let log = fs::read(home.path().join("eventos.jsonl")).expect("eventos");
    assert!(!tiene_la_clave(&log), "la clave quedó en eventos.jsonl");
}

#[test]
fn mcp_llama_devuelve_la_misma_respuesta_que_la_cli() {
    let srv = Servidor::arranca(vec![(200, DEEPSEEK_OK), (200, DEEPSEEK_OK)]);
    let home = tempfile::tempdir().expect("home");
    preparar(home.path(), &srv.url, PRECIO_BARATO);

    let cli = Command::new(env!("CARGO_BIN_EXE_r00t"))
        .env("ROOT_HOME", home.path())
        .env("DEEPSEEK_API_KEY", CLAVE)
        .env_remove("ANTHROPIC_API_KEY")
        .args([
            "llama",
            "--rol",
            "juzga",
            "--tipo",
            "diseno",
            "--corrida",
            "c1",
            "--max-tokens",
            "1000",
            "--prompt-file",
        ])
        .arg(home.path().join("prompt.txt"))
        .output()
        .expect("binario");
    assert_eq!(cli.status.code(), Some(0));
    let mut esperado: Value = serde_json::from_slice(&cli.stdout).expect("stdout es JSON");

    let resp = herramienta(
        home.path(),
        "root_llama",
        json!({"rol": "juzga", "tipo": "diseno", "prompt": "hola", "corrida": "c1", "max_tokens": 1000}),
    );
    assert_eq!(resp["result"]["isError"], false);
    let mut obtenido = texto_json(&resp);
    // La latencia cambia entre dos llamadas: no entra en la comparación.
    esperado
        .as_object_mut()
        .expect("objeto")
        .remove("latency_s");
    obtenido
        .as_object_mut()
        .expect("objeto")
        .remove("latency_s");
    assert_eq!(obtenido, esperado);
    assert_eq!(srv.peticiones(), 2);
}

#[test]
fn mcp_llama_tope_es_error_de_herramienta_y_no_envia() {
    let srv = Servidor::arranca(vec![(200, DEEPSEEK_OK)]);
    let home = tempfile::tempdir().expect("home");
    preparar(home.path(), &srv.url, PRECIO_CARO);
    gasto_api(home.path(), 98.0);

    let resp = herramienta(
        home.path(),
        "root_llama",
        json!({"rol": "juzga", "tipo": "diseno", "prompt": "hola", "max_tokens": 1000}),
    );
    assert!(es_error_de_herramienta(&resp), "respuesta: {resp}");
    assert!(texto_plano(&resp).contains("margen"));
    assert_eq!(srv.peticiones(), 0, "la llamada no debe enviarse");
    let gates: Vec<Evento> = eventos(home.path())
        .into_iter()
        .filter(|ev| ev.tipo == TipoEvento::Gate)
        .collect();
    assert_eq!(gates.len(), 1);
    assert_eq!(gates[0].fase, "tope");
    assert_eq!(gates[0].corrida, "mcp");
}

#[test]
fn mcp_llama_rol_de_cupo_es_error_de_herramienta_sin_pedidos() {
    let srv = Servidor::arranca(vec![(200, DEEPSEEK_OK)]);
    let home = tempfile::tempdir().expect("home");
    preparar(home.path(), &srv.url, PRECIO_BARATO);

    let resp = herramienta(
        home.path(),
        "root_llama",
        json!({"rol": "construye", "tipo": "bulk", "prompt": "hola"}),
    );
    assert!(es_error_de_herramienta(&resp), "respuesta: {resp}");
    assert!(texto_plano(&resp).contains("claude -p"));
    assert_eq!(srv.peticiones(), 0);
}

fn tarea_sin_ficha(home: &Path) {
    let dir = home.join("oraculos").join("memora");
    fs::create_dir_all(&dir).expect("dir de la tarea");
    fs::write(dir.join("tarea.toml"), TAREA_MINIMA).expect("tarea.toml");
    fs::write(dir.join("SPEC.md"), "# Spec\n\nSin ficha.\n").expect("SPEC.md");
}

#[test]
fn mcp_evaluar_sin_ficha_es_error_de_herramienta() {
    let home = tempfile::tempdir().expect("home");
    let repo = tempfile::tempdir().expect("repo");
    tarea_sin_ficha(home.path());

    let resp = herramienta(
        home.path(),
        "root_evaluar",
        json!({"tarea": "memora", "repo": repo.path()}),
    );
    assert!(es_error_de_herramienta(&resp), "respuesta: {resp}");
    assert!(texto_plano(&resp).contains("sin ficha"));
}

#[test]
fn mcp_evaluar_tarea_inexistente_es_error_de_herramienta() {
    let home = tempfile::tempdir().expect("home");
    let repo = tempfile::tempdir().expect("repo");
    let resp = herramienta(
        home.path(),
        "root_evaluar",
        json!({"tarea": "no-existe", "repo": repo.path()}),
    );
    assert!(es_error_de_herramienta(&resp), "respuesta: {resp}");
}

#[test]
fn mcp_linea_basura_da_32700_y_el_servidor_sigue() {
    let home = tempfile::tempdir().expect("home");
    let entrada =
        "no es json\n".to_string() + &linea(json!({"jsonrpc": "2.0", "id": 2, "method": "ping"}));
    let salida = mcp(home.path(), &entrada);
    assert_eq!(salida.status.code(), Some(0));
    let r = respuestas(&salida);
    assert_eq!(r.len(), 2);
    assert_eq!(r[0]["error"]["code"], -32700);
    assert_eq!(r[0]["id"], Value::Null);
    assert_eq!(r[1]["id"], 2);
    assert_eq!(r[1]["result"], json!({}));
}

#[test]
fn mcp_metodo_desconocido_da_32601() {
    let home = tempfile::tempdir().expect("home");
    let salida = mcp(
        home.path(),
        &linea(json!({"jsonrpc": "2.0", "id": 3, "method": "foo/bar"})),
    );
    let r = respuestas(&salida);
    assert_eq!(codigo_rpc(&r[0]), -32601);
    assert_eq!(r[0]["id"], 3);
}

#[test]
fn mcp_id_de_cadena_se_devuelve_sin_cambios() {
    let home = tempfile::tempdir().expect("home");
    let salida = mcp(
        home.path(),
        &linea(json!({"jsonrpc": "2.0", "id": "req-7", "method": "ping"})),
    );
    assert_eq!(respuestas(&salida)[0]["id"], "req-7");
}

#[test]
fn mcp_tool_inexistente_y_sin_name_dan_32602() {
    let home = tempfile::tempdir().expect("home");
    let r1 = herramienta(home.path(), "root_inventada", json!({}));
    assert_eq!(codigo_rpc(&r1), -32602);
    let salida = mcp(
        home.path(),
        &linea(json!({"jsonrpc": "2.0", "id": 9, "method": "tools/call", "params": {}})),
    );
    assert_eq!(codigo_rpc(&respuestas(&salida)[0]), -32602);
}

#[test]
fn mcp_eof_sin_mensajes_sale_con_0() {
    let home = tempfile::tempdir().expect("home");
    let salida = mcp(home.path(), "");
    assert_eq!(salida.status.code(), Some(0));
    assert!(salida.stdout.is_empty());
}

#[test]
fn mcp_varios_requests_en_una_escritura_se_responden_en_orden() {
    let home = tempfile::tempdir().expect("home");
    let entrada = linea(json!({"jsonrpc": "2.0", "id": 1, "method": "ping"}))
        + &linea(json!({"jsonrpc": "2.0", "id": 2, "method": "ping"}))
        + &linea(json!({"jsonrpc": "2.0", "id": 3, "method": "ping"}));
    let salida = mcp(home.path(), &entrada);
    let ids: Vec<Value> = respuestas(&salida)
        .iter()
        .map(|r| r["id"].clone())
        .collect();
    assert_eq!(ids, vec![json!(1), json!(2), json!(3)]);
}

#[test]
fn mcp_stdout_tiene_solo_lineas_json_aun_con_errores_mezclados() {
    let home = tempfile::tempdir().expect("home");
    let entrada = "basura\n".to_string()
        + &linea(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}))
        + &llamada(2, "root_route", json!({"rol": "inventado", "tipo": "x"}))
        + &linea(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    let salida = mcp(home.path(), &entrada);
    assert_eq!(salida.status.code(), Some(0));
    // `respuestas` falla si alguna línea de stdout no es JSON.
    let r = respuestas(&salida);
    assert_eq!(r.len(), 3, "respuestas: {r:?}");
}

/// Los bloques de texto de `content`, en orden.
fn bloques_de(resp: &Value) -> Vec<String> {
    resp["result"]["content"]
        .as_array()
        .expect("content")
        .iter()
        .map(|b| b["text"].as_str().expect("texto").to_string())
        .collect()
}

#[test]
fn mcp_llama_evento_no_registrado_entrega_respuesta_y_evento() {
    let home = tempfile::tempdir().expect("home");
    // La reserva se escribe; al recibir el pedido, el log pasa a ser un directorio. La llamada
    // sale, y su evento final no se puede escribir.
    let srv = Servidor::arranca_con(
        vec![(200, DEEPSEEK_OK)],
        Duration::ZERO,
        Some(rompe_el_log_en_el_pedido(home.path(), 1)),
    );
    preparar(home.path(), &srv.url, PRECIO_BARATO);

    let resp = herramienta(
        home.path(),
        "root_llama",
        json!({"rol": "juzga", "tipo": "diseno", "prompt": "hola", "corrida": "c1", "max_tokens": 1000}),
    );
    assert!(es_error_de_herramienta(&resp), "respuesta: {resp}");
    assert_eq!(srv.peticiones(), 1);
    let bloques = bloques_de(&resp);
    assert_eq!(bloques.len(), 3, "bloques: {bloques:?}");
    assert!(
        bloques[0].contains("no se pudo registrar el evento"),
        "bloque 1: {}",
        bloques[0]
    );
    let respuesta: Value = serde_json::from_str(&bloques[1]).expect("bloque 2 es la Respuesta");
    assert_eq!(respuesta["texto"], "hola");
    let evento: Value = serde_json::from_str(&bloques[2]).expect("bloque 3 es el Evento");
    assert_eq!(evento["corrida"], "c1");
    assert!(
        !tiene_la_clave(resp.to_string().as_bytes()),
        "la clave salió en la respuesta"
    );
}

#[test]
fn mcp_llama_error_del_proveedor_sin_evento_entrega_error_y_evento() {
    // Sin cola, el servidor responde 500 a todos los intentos. Al tercero, el log pasa a ser un
    // directorio: el evento de la falla no se puede escribir.
    let home = tempfile::tempdir().expect("home");
    let srv = Servidor::arranca_con(
        vec![],
        Duration::ZERO,
        Some(rompe_el_log_en_el_pedido(home.path(), 3)),
    );
    preparar(home.path(), &srv.url, PRECIO_BARATO);

    let resp = herramienta(
        home.path(),
        "root_llama",
        json!({"rol": "juzga", "tipo": "diseno", "prompt": "hola", "corrida": "c1", "max_tokens": 1000}),
    );
    assert!(es_error_de_herramienta(&resp), "respuesta: {resp}");
    let bloques = bloques_de(&resp);
    assert_eq!(bloques.len(), 3, "bloques: {bloques:?}");
    assert!(bloques[1].contains("HTTP 500"), "bloque 2: {}", bloques[1]);
    let evento: Value = serde_json::from_str(&bloques[2]).expect("bloque 3 es el Evento");
    assert_eq!(evento["extra"]["error"], "http_500");
    assert!(
        !tiene_la_clave(resp.to_string().as_bytes()),
        "la clave salió en la respuesta"
    );
}
