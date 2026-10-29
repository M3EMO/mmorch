//! Servidor HTTP falso en 127.0.0.1 para las pruebas de la CLI. Ninguna prueba sale a internet.
// Cada crate de test usa una parte distinta de este módulo.
#![allow(dead_code)]

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

/// Responde con la cola en orden y cuenta cada pedido. Cuando la cola se agota, responde 500.
pub struct Servidor {
    pub url: String,
    pedidos: Arc<Mutex<usize>>,
    cuerpos: Arc<Mutex<Vec<String>>>,
}

impl Servidor {
    pub fn arranca(respuestas: Vec<(u16, &'static str)>) -> Servidor {
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
                *cuenta.lock().expect("lock") += 1;
                let (codigo, cuerpo) = cola
                    .lock()
                    .expect("lock")
                    .pop_front()
                    .unwrap_or((500, "{}"));
                let respuesta = format!(
                    "HTTP/1.1 {codigo} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{cuerpo}",
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
