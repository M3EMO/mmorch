//! Servidor HTTP falso en 127.0.0.1 para las pruebas de la CLI. Ninguna prueba sale a internet.
// Cada crate de test usa una parte distinta de este módulo.
#![allow(dead_code)]

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};

/// Responde con la cola en orden y cuenta cada pedido. Cuando la cola se agota, responde 500.
pub struct Servidor {
    pub url: String,
    pedidos: Arc<Mutex<usize>>,
    cuerpos: Arc<Mutex<Vec<String>>>,
}

/// Función que corre al recibir el pedido número `n` (desde 1), antes de responder.
pub type Gancho = (usize, Box<dyn Fn() + Send>);

impl Servidor {
    pub fn arranca(respuestas: Vec<(u16, String)>) -> Servidor {
        Servidor::arranca_con(respuestas, Duration::ZERO, None)
    }

    /// Como `arranca`, pero espera `demora` después de recibir cada pedido y antes de responder.
    pub fn arranca_lento(respuestas: Vec<(u16, String)>, demora: Duration) -> Servidor {
        Servidor::arranca_con(respuestas, demora, None)
    }

    /// Como `arranca_lento`, y corre `gancho` al recibir su pedido, antes de responder.
    pub fn arranca_con(
        respuestas: Vec<(u16, String)>,
        demora: Duration,
        gancho: Option<Gancho>,
    ) -> Servidor {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind en loopback");
        let url = format!("http://{}", listener.local_addr().expect("dirección"));
        let pedidos = Arc::new(Mutex::new(0usize));
        let cuerpos = Arc::new(Mutex::new(Vec::new()));
        let cola = Arc::new(Mutex::new(VecDeque::from(respuestas)));
        let cuenta = Arc::clone(&pedidos);
        let registro = Arc::clone(&cuerpos);
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let recibido = leer_pedido(&stream);
                registro.lock().expect("lock").push(recibido);
                let n = {
                    let mut cuenta = cuenta.lock().expect("lock");
                    *cuenta += 1;
                    *cuenta
                };
                if let Some((indice, funcion)) = &gancho
                    && *indice == n
                {
                    funcion();
                }
                thread::sleep(demora);
                let (codigo, cuerpo) = cola
                    .lock()
                    .expect("lock")
                    .pop_front()
                    .unwrap_or((500, "{}".into()));
                // Las respuestas 2xx son SSE, como las de los proveedores con `"stream": true`.
                let tipo = if (200..300).contains(&codigo) {
                    "text/event-stream"
                } else {
                    "application/json"
                };
                let respuesta = format!(
                    "HTTP/1.1 {codigo} X\r\nContent-Type: {tipo}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{cuerpo}",
                    cuerpo.len()
                );
                let _ = stream.write_all(respuesta.as_bytes());
            }
        });
        Servidor {
            url,
            pedidos,
            cuerpos,
        }
    }

    /// Cuántos pedidos llegaron al servidor.
    pub fn peticiones(&self) -> usize {
        *self.pedidos.lock().expect("lock")
    }

    /// Cuerpo del pedido número `indice` (desde 0), tal como llegó.
    pub fn cuerpo(&self, indice: usize) -> String {
        self.cuerpos.lock().expect("lock")[indice].clone()
    }
}

/// Gancho que cambia `eventos.jsonl` por un directorio al recibir el pedido `n`. La reserva y los
/// intentos anteriores se escriben bien; el registro final de la llamada ya no se puede escribir.
pub fn rompe_el_log_en_el_pedido(home: &std::path::Path, n: usize) -> Gancho {
    let ruta = home.join("eventos.jsonl");
    (
        n,
        Box::new(move || {
            let _ = std::fs::remove_file(&ruta);
            let _ = std::fs::create_dir(&ruta);
        }),
    )
}

/// Lee cabeceras y cuerpo del pedido para que el cliente no reciba un cierre prematuro.
/// Devuelve el cuerpo como texto.
fn leer_pedido(stream: &TcpStream) -> String {
    let mut lector = BufReader::new(stream);
    let mut linea = String::new();
    lector.read_line(&mut linea).expect("línea de inicio");
    let mut largo = 0usize;
    loop {
        let mut fila = String::new();
        lector.read_line(&mut fila).expect("cabecera");
        let fila = fila.trim_end();
        if fila.is_empty() {
            break;
        }
        if let Some((k, v)) = fila.split_once(':')
            && k.trim().eq_ignore_ascii_case("content-length")
        {
            largo = v.trim().parse().unwrap_or(0);
        }
    }
    let mut cuerpo = vec![0u8; largo];
    lector.read_exact(&mut cuerpo).expect("cuerpo");
    String::from_utf8_lossy(&cuerpo).into_owned()
}

/// Una línea `data:` de SSE con `valor` como JSON, y su línea vacía.
pub fn sse_evento(valor: Value) -> String {
    format!("data: {valor}\n\n")
}

/// Cuerpo SSE de DeepSeek como lo manda la API con `include_usage`: un chunk de razonamiento que
/// Root ignora, el texto en un chunk, el `finish_reason` y el chunk final de uso con `choices` vacío.
pub fn sse_deepseek(texto: &str, in_tokens: u64, out_tokens: u64, finish: &str) -> String {
    let mut cuerpo = sse_deepseek_sin_uso(texto, finish);
    cuerpo.insert_str(
        cuerpo.len() - "data: [DONE]\n\n".len(),
        &sse_evento(json!({
            "choices": [],
            "usage": {"prompt_tokens": in_tokens, "completion_tokens": out_tokens},
        })),
    );
    cuerpo
}

/// Como `sse_deepseek`, pero sin el chunk de uso: el stream termina sin decir cuánto costó.
pub fn sse_deepseek_sin_uso(texto: &str, finish: &str) -> String {
    let mut cuerpo = sse_evento(json!({
        "choices": [{"delta": {"reasoning_content": "pensé"}, "finish_reason": null}],
    }));
    cuerpo += &sse_evento(json!({
        "choices": [{"delta": {"content": texto}, "finish_reason": null}],
    }));
    cuerpo += &sse_evento(json!({"choices": [{"delta": {}, "finish_reason": finish}]}));
    cuerpo += "data: [DONE]\n\n";
    cuerpo
}

/// Cuerpo SSE de Anthropic: un bloque de razonamiento (que Root ignora), un bloque de texto por
/// cada elemento de `textos`, un `ping` y el `message_delta` con el `stop_reason` y el uso final.
pub fn sse_anthropic(textos: &[&str], in_tokens: u64, out_tokens: u64, stop: &str) -> String {
    arma_anthropic(textos, in_tokens, Some(out_tokens), stop)
}

/// Como `sse_anthropic`, pero el `message_delta` no trae `usage`: el stream no dice cuánto costó.
pub fn sse_anthropic_sin_uso(textos: &[&str], in_tokens: u64, stop: &str) -> String {
    arma_anthropic(textos, in_tokens, None, stop)
}

fn arma_anthropic(textos: &[&str], in_tokens: u64, out_tokens: Option<u64>, stop: &str) -> String {
    let evento = |nombre: &str, valor: Value| format!("event: {nombre}\n{}", sse_evento(valor));
    let mut cuerpo = evento(
        "message_start",
        json!({"type": "message_start", "message": {"usage": {"input_tokens": in_tokens, "output_tokens": 1}}}),
    );
    cuerpo += &evento(
        "content_block_start",
        json!({"type": "content_block_start", "index": 0, "content_block": {"type": "thinking", "thinking": ""}}),
    );
    cuerpo += &evento(
        "content_block_delta",
        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "thinking_delta", "thinking": "pensé"}}),
    );
    cuerpo += &evento(
        "content_block_delta",
        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "signature_delta", "signature": "firma"}}),
    );
    cuerpo += &evento(
        "content_block_stop",
        json!({"type": "content_block_stop", "index": 0}),
    );
    for (i, texto) in textos.iter().enumerate() {
        let index = i + 1;
        cuerpo += &evento(
            "content_block_start",
            json!({"type": "content_block_start", "index": index, "content_block": {"type": "text", "text": ""}}),
        );
        cuerpo += &evento(
            "content_block_delta",
            json!({"type": "content_block_delta", "index": index, "delta": {"type": "text_delta", "text": texto}}),
        );
        cuerpo += &evento(
            "content_block_stop",
            json!({"type": "content_block_stop", "index": index}),
        );
    }
    cuerpo += &evento("ping", json!({"type": "ping"}));
    let uso = out_tokens.map(|out| json!({"output_tokens": out}));
    let mut delta =
        json!({"type": "message_delta", "delta": {"stop_reason": stop, "stop_sequence": null}});
    if let Some(uso) = uso {
        delta["usage"] = uso;
    }
    cuerpo += &evento("message_delta", delta);
    cuerpo += &evento("message_stop", json!({"type": "message_stop"}));
    cuerpo
}
