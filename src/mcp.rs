//! Servidor MCP por stdio: JSON-RPC 2.0, un mensaje por línea.
//! Cada herramienta llama a la misma función que su subcomando y devuelve el mismo JSON.

use std::io::{self, BufRead, Write};
use std::path::Path;

use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::error::Error;
use crate::observe;
use crate::oracle;
use crate::proveedores::ErrorProveedor;
use crate::route::comando;
use crate::tipos::{Estado, Rol, TipoEvento};

const VERSION_DEFECTO: &str = "2024-11-05";
const VERSIONES: [&str; 3] = ["2024-11-05", "2025-03-26", "2025-06-18"];
/// Corrida de los eventos de `root_llama` cuando el cliente no da una.
const CORRIDA_MCP: &str = "mcp";
const MAX_TOKENS_DEFECTO: u32 = 4096;

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

/// Código y mensaje de un error JSON-RPC.
type ErrorRpc = (i64, String);
/// Argumentos de una herramienta, ya como objeto JSON.
type Args = Map<String, Value>;

/// Fallo de una herramienta. `Parametros` es -32602; `Herramienta` es `isError: true`.
enum Fallo {
    Parametros(String),
    Herramienta(String),
    /// Varios bloques de texto, todos con `isError: true`. Así no se pierde lo que ya se pagó.
    Bloques(Vec<String>),
}

/// Procesa mensajes en orden hasta EOF. Cada respuesta va en una línea y se escribe al instante.
pub fn servir(mut entrada: impl BufRead, mut salida: impl Write) -> io::Result<()> {
    let mut buf = Vec::new();
    loop {
        buf.clear();
        if entrada.read_until(b'\n', &mut buf)? == 0 {
            return Ok(());
        }
        let linea = buf.trim_ascii();
        if linea.is_empty() {
            continue;
        }
        if let Some(respuesta) = procesar(linea) {
            let texto = respuesta.to_string();
            writeln!(salida, "{texto}")?;
            salida.flush()?;
        }
    }
}

/// Respuesta a un mensaje, o `None` si el mensaje es una notificación o una respuesta del cliente.
fn procesar(linea: &[u8]) -> Option<Value> {
    let Ok(mensaje) = serde_json::from_slice::<Value>(linea) else {
        return Some(error_rpc(Value::Null, PARSE_ERROR, "JSON inválido".into()));
    };
    let Some(objeto) = mensaje.as_object() else {
        let causa = "el mensaje no es un objeto JSON";
        return Some(error_rpc(Value::Null, INVALID_REQUEST, causa.into()));
    };
    let id = match objeto.get("id") {
        None => None,
        Some(v) if v.is_string() || v.is_number() || v.is_null() => Some(v.clone()),
        Some(_) => {
            let causa = "id inválido: debe ser texto, número o null";
            return Some(error_rpc(Value::Null, INVALID_REQUEST, causa.into()));
        }
    };
    let en_error = id.clone().unwrap_or(Value::Null);
    if objeto.get("jsonrpc") != Some(&json!("2.0")) {
        let causa = "falta jsonrpc 2.0";
        return Some(error_rpc(en_error, INVALID_REQUEST, causa.into()));
    }
    let Some(metodo) = objeto.get("method").and_then(Value::as_str) else {
        // Una respuesta del cliente no pide nada. Cualquier otro mensaje es inválido.
        if objeto.contains_key("result") || objeto.contains_key("error") {
            return None;
        }
        return Some(error_rpc(en_error, INVALID_REQUEST, "falta method".into()));
    };
    // Sin `id` es una notificación: se procesa sin ejecutar nada y sin respuesta.
    let id = id?;
    let params = objeto.get("params").cloned().unwrap_or(Value::Null);
    Some(match despachar(metodo, &params) {
        Ok(resultado) => json!({"jsonrpc": "2.0", "id": id, "result": resultado}),
        Err((codigo, mensaje)) => error_rpc(id, codigo, mensaje),
    })
}

fn error_rpc(id: Value, codigo: i64, mensaje: String) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": codigo, "message": mensaje}})
}

fn despachar(metodo: &str, params: &Value) -> Result<Value, ErrorRpc> {
    match metodo {
        "initialize" => Ok(inicializar(params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": herramientas()})),
        "tools/call" => llamar(params),
        otro => Err((METHOD_NOT_FOUND, format!("método desconocido: {otro}"))),
    }
}

/// Negocia la versión: la del cliente si el servidor la conoce, si no la de defecto.
fn inicializar(params: &Value) -> Value {
    let pedida = params.get("protocolVersion").and_then(Value::as_str);
    let version = match pedida {
        Some(v) if VERSIONES.contains(&v) => v,
        _ => VERSION_DEFECTO,
    };
    json!({
        "protocolVersion": version,
        "capabilities": {"tools": {}},
        "serverInfo": {"name": "root", "version": env!("CARGO_PKG_VERSION")}
    })
}

fn llamar(params: &Value) -> Result<Value, ErrorRpc> {
    let nombre = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| (INVALID_PARAMS, "falta name".to_string()))?;
    let args = match params.get("arguments") {
        None | Some(Value::Null) => Map::new(),
        Some(Value::Object(m)) => m.clone(),
        Some(_) => return Err((INVALID_PARAMS, "arguments debe ser un objeto".into())),
    };
    match ejecutar(nombre, &args) {
        Ok(valor) => Ok(contenido(&[valor.to_string()], false)),
        Err(Fallo::Parametros(m)) => Err((INVALID_PARAMS, m)),
        Err(Fallo::Herramienta(m)) => Ok(contenido(&[m], true)),
        Err(Fallo::Bloques(bloques)) => Ok(contenido(&bloques, true)),
    }
}

/// Un bloque de texto por cada elemento. Un solo bloque es lo normal.
fn contenido(textos: &[String], es_error: bool) -> Value {
    let bloques: Vec<Value> = textos
        .iter()
        .map(|t| json!({"type": "text", "text": t}))
        .collect();
    json!({"content": bloques, "isError": es_error})
}

fn ejecutar(nombre: &str, a: &Args) -> Result<Value, Fallo> {
    match nombre {
        "root_route" => root_route(a),
        "root_llama" => root_llama(a),
        "root_evaluar" => root_evaluar(a),
        "root_registrar" => root_registrar(a),
        otro => Err(Fallo::Parametros(format!(
            "herramienta inexistente: {otro}"
        ))),
    }
}

fn root_route(a: &Args) -> Result<Value, Fallo> {
    let rol = rol_req(a)?;
    let tipo = texto_req(a, "tipo")?;
    let checkeable = bool_opt(a, "checkeable")?;
    let tokens = entero_opt(a, "tokens")?.unwrap_or(0);
    let eleccion = comando::rutear(rol, &tipo, checkeable, tokens).map_err(herramienta)?;
    valor(&eleccion)
}

fn root_llama(a: &Args) -> Result<Value, Fallo> {
    let rol = rol_req(a)?;
    let tipo = texto_req(a, "tipo")?;
    let prompt = texto_req(a, "prompt")?;
    let corrida = texto_opt(a, "corrida")?.unwrap_or_else(|| CORRIDA_MCP.into());
    let max_tokens = entero_opt(a, "max_tokens")?.unwrap_or(MAX_TOKENS_DEFECTO);
    match comando::llamar_con_chequeo(rol, &tipo, prompt, &corrida, max_tokens) {
        Ok(respuesta) => valor(&respuesta),
        // La llamada salió y el evento no se escribió: se devuelven la respuesta y el evento,
        // igual que la CLI en stdout y stderr.
        Err(Error::Proveedor(ErrorProveedor::EventoNoRegistrado {
            respuesta,
            evento,
            causa,
        })) => Err(Fallo::Bloques(vec![
            format!("no se pudo registrar el evento: {causa}"),
            texto_json(&*respuesta),
            texto_json(&*evento),
        ])),
        Err(Error::Proveedor(ErrorProveedor::ErrorNoRegistrado {
            original,
            causa_registro,
            evento,
        })) => Err(Fallo::Bloques(vec![
            format!("{original}; además no se pudo registrar el evento: {causa_registro}"),
            original.to_string(),
            texto_json(&*evento),
        ])),
        Err(otro) => Err(herramienta(otro)),
    }
}

/// Texto JSON de un valor para un bloque de `content`. Los tipos de `Respuesta` y `Evento` no fallan.
fn texto_json<T: Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

fn root_evaluar(a: &Args) -> Result<Value, Fallo> {
    let tarea = texto_req(a, "tarea")?;
    let repo = texto_req(a, "repo")?;
    let veredicto = oracle::evaluar_por_nombre(&tarea, Path::new(&repo)).map_err(herramienta)?;
    if veredicto.estado == Estado::SinVeredicto {
        let motivos: Vec<&str> = veredicto
            .hallazgos
            .iter()
            .map(|h| h.detalle.as_str())
            .chain(veredicto.avisos.iter().map(String::as_str))
            .collect();
        let detalle = if motivos.is_empty() {
            "sin detalle".to_string()
        } else {
            motivos.join("; ")
        };
        return Err(Fallo::Herramienta(format!(
            "sin veredicto para {tarea}: {detalle}"
        )));
    }
    // A2: el agente no recibe ids de tests ocultos. La CLI `oraculo evaluar` sí los conserva.
    valor(&oracle::redactar_para_mcp(veredicto))
}

fn root_registrar(a: &Args) -> Result<Value, Fallo> {
    let corrida = texto_req(a, "corrida")?;
    let tipo_texto = texto_req(a, "tipo")?;
    let tipo = match tipo_texto.as_str() {
        "humano" => TipoEvento::Humano,
        "plan-aprobado" => TipoEvento::PlanAprobado,
        "gate" => TipoEvento::Gate,
        otro => {
            return Err(Fallo::Parametros(format!(
                "tipo inválido: {otro}; use humano, plan-aprobado o gate"
            )));
        }
    };
    let detalle = texto_opt(a, "detalle")?;
    observe::registrar_manual(&corrida, tipo, detalle.as_deref()).map_err(herramienta)?;
    Ok(json!({"registrado": true, "corrida": corrida, "tipo": tipo_texto}))
}

/// Serializa el resultado de una herramienta. Un fallo de serialización es fallo de herramienta.
fn valor<T: Serialize>(v: &T) -> Result<Value, Fallo> {
    serde_json::to_value(v).map_err(|e| Fallo::Herramienta(e.to_string()))
}

/// Error de una herramienta: su mensaje va en `isError`.
fn herramienta(e: impl std::fmt::Display) -> Fallo {
    Fallo::Herramienta(e.to_string())
}

fn texto_req(a: &Args, clave: &str) -> Result<String, Fallo> {
    match a.get(clave) {
        Some(Value::String(s)) => Ok(s.clone()),
        _ => Err(Fallo::Parametros(format!("falta {clave} (texto)"))),
    }
}

fn texto_opt(a: &Args, clave: &str) -> Result<Option<String>, Fallo> {
    match a.get(clave) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(Fallo::Parametros(format!("{clave} debe ser texto"))),
    }
}

fn bool_opt(a: &Args, clave: &str) -> Result<bool, Fallo> {
    match a.get(clave) {
        None | Some(Value::Null) => Ok(false),
        Some(Value::Bool(b)) => Ok(*b),
        Some(_) => Err(Fallo::Parametros(format!(
            "{clave} debe ser verdadero o falso"
        ))),
    }
}

fn entero_opt(a: &Args, clave: &str) -> Result<Option<u32>, Fallo> {
    match a.get(clave) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .map(Some)
            .ok_or_else(|| {
                Fallo::Parametros(format!("{clave} debe ser un entero de 0 a {}", u32::MAX))
            }),
    }
}

fn rol_req(a: &Args) -> Result<Rol, Fallo> {
    let texto = texto_req(a, "rol")?;
    serde_json::from_value::<Rol>(Value::String(texto.clone())).map_err(|_| {
        Fallo::Parametros(format!(
            "rol inválido: {texto}; use planifica, organiza, construye, verifica, juzga o delega"
        ))
    })
}

fn esquema_rol() -> Value {
    json!({
        "type": "string",
        "enum": ["planifica", "organiza", "construye", "verifica", "juzga", "delega"],
        "description": "Rol del pedido."
    })
}

/// Lista de herramientas de `tools/list`. Los `required` coinciden con lo que valida cada una.
fn herramientas() -> Value {
    json!([
        {
            "name": "root_route",
            "description": "Elige el modelo y la vía para un rol y tipo de tarea. Devuelve la misma elección que `r00t route --json`.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "rol": esquema_rol(),
                    "tipo": {"type": "string", "description": "Tipo de tarea, por ejemplo diseno o bulk."},
                    "checkeable": {"type": "boolean", "description": "Si la salida se verifica con un oráculo."},
                    "tokens": {"type": "integer", "minimum": 0, "description": "Tokens estimados del pedido."}
                },
                "required": ["rol", "tipo"]
            }
        },
        {
            "name": "root_llama",
            "description": "Llama a un modelo por API con el prompt dado. Devuelve la misma respuesta que `r00t llama`. Un tope insuficiente rechaza la llamada antes de enviarla.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "rol": esquema_rol(),
                    "tipo": {"type": "string", "description": "Tipo de tarea, por ejemplo diseno o bulk."},
                    "prompt": {"type": "string", "description": "Texto que se envía al modelo."},
                    "corrida": {"type": "string", "description": "Corrida de los eventos. Por defecto mcp."},
                    "max_tokens": {"type": "integer", "minimum": 0, "description": "Tope de tokens de salida. Por defecto 4096."}
                },
                "required": ["rol", "tipo", "prompt"]
            }
        },
        {
            "name": "root_evaluar",
            "description": "Evalúa un repositorio con una tarea del Oracle. Devuelve el veredicto sin ids de tests ocultos: conteos, estado, avisos y pendientes del contrato. `r00t --json oraculo evaluar` trae el veredicto completo.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tarea": {"type": "string", "description": "Nombre de la tarea en ROOT_HOME/oraculos."},
                    "repo": {"type": "string", "description": "Ruta del repositorio a evaluar."}
                },
                "required": ["tarea", "repo"]
            }
        },
        {
            "name": "root_registrar",
            "description": "Registra un evento manual de una corrida. Lo mismo que `r00t observa evento`.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "corrida": {"type": "string", "description": "Corrida a la que pertenece el evento."},
                    "tipo": {"type": "string", "enum": ["humano", "plan-aprobado", "gate"], "description": "Tipo del evento."},
                    "detalle": {"type": "string", "description": "Texto libre que acompaña al evento."}
                },
                "required": ["corrida", "tipo"]
            }
        }
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn sesion(entrada: &str) -> Vec<Value> {
        let mut salida = Vec::new();
        servir(entrada.as_bytes(), &mut salida).expect("servir");
        String::from_utf8(salida)
            .expect("utf8")
            .lines()
            .map(|l| serde_json::from_str(l).expect("cada línea es JSON"))
            .collect()
    }

    fn linea(valor: Value) -> String {
        format!("{valor}\n")
    }

    #[test]
    fn mcp_unit_initialize_negocia_la_version_soportada() {
        let r = sesion(&linea(json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {"protocolVersion": "2025-03-26"}
        })));
        assert_eq!(r[0]["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(r[0]["result"]["capabilities"]["tools"], json!({}));
        assert_eq!(r[0]["result"]["serverInfo"]["name"], "root");
    }

    #[test]
    fn mcp_unit_initialize_con_version_desconocida_responde_2024_11_05() {
        let r = sesion(&linea(json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {"protocolVersion": "1999-01-01"}
        })));
        assert_eq!(r[0]["result"]["protocolVersion"], "2024-11-05");
    }

    #[test]
    fn mcp_unit_notificacion_no_produce_respuesta() {
        let r = sesion(&linea(
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        ));
        assert!(r.is_empty(), "salida inesperada: {r:?}");
    }

    #[test]
    fn mcp_unit_linea_que_no_es_json_da_32700_con_id_nulo() {
        let r = sesion("esto no es json\n");
        assert_eq!(r[0]["error"]["code"], -32700);
        assert_eq!(r[0]["id"], Value::Null);
    }

    #[test]
    fn mcp_unit_request_sin_jsonrpc_da_32600() {
        let r = sesion(&linea(json!({"id": 1, "method": "ping"})));
        assert_eq!(r[0]["error"]["code"], -32600);
        assert_eq!(r[0]["id"], 1);
    }

    #[test]
    fn mcp_unit_metodo_desconocido_da_32601() {
        let r = sesion(&linea(
            json!({"jsonrpc": "2.0", "id": 4, "method": "foo/bar"}),
        ));
        assert_eq!(r[0]["error"]["code"], -32601);
        assert_eq!(r[0]["id"], 4);
    }

    #[test]
    fn mcp_unit_tools_call_sin_name_da_32602() {
        let r = sesion(&linea(json!({
            "jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {}
        })));
        assert_eq!(r[0]["error"]["code"], -32602);
    }

    #[test]
    fn mcp_unit_rol_invalido_da_32602_antes_de_tocar_el_disco() {
        let r = sesion(&linea(json!({
            "jsonrpc": "2.0", "id": 6, "method": "tools/call",
            "params": {"name": "root_route", "arguments": {"rol": "inventado", "tipo": "x"}}
        })));
        assert_eq!(r[0]["error"]["code"], -32602);
        assert!(r[0].get("result").is_none());
    }

    #[test]
    fn mcp_unit_ping_responde_objeto_vacio_con_el_mismo_id_de_cadena() {
        let r = sesion(&linea(
            json!({"jsonrpc": "2.0", "id": "req-7", "method": "ping"}),
        ));
        assert_eq!(r[0]["id"], "req-7");
        assert_eq!(r[0]["result"], json!({}));
    }
}
