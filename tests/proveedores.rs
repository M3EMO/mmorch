//! Proveedores contra un servidor HTTP falso en 127.0.0.1. Ninguna prueba sale a internet.

use std::cell::Cell;
use std::collections::VecDeque;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use root::config::Config;
use root::proveedores::{
    Breaker, Clave, Claves, Contexto, ErrorProveedor, Prompt, Respuesta, imprimir_para_cli,
    llamar_con,
};
use root::tipos::{Eleccion, Proveedor, Via};
use serde_json::Value;

const CLAVE_PRUEBA: &str = "sk-test-SECRETO-123";

/// Petición recibida: línea de inicio, cabeceras en minúscula y cuerpo.
#[derive(Debug, Clone)]
struct Pedido {
    linea: String,
    cabeceras: Vec<(String, String)>,
    cuerpo: String,
}

impl Pedido {
    fn cabecera(&self, nombre: &str) -> Option<&str> {
        self.cabeceras
            .iter()
            .find(|(k, _)| k == nombre)
            .map(|(_, v)| v.as_str())
    }
}

struct Servidor {
    url: String,
    pedidos: Arc<Mutex<Vec<Pedido>>>,
}

impl Servidor {
    /// Responde con la cola en orden. Cuando la cola se agota, responde 500.
    fn arranca(respuestas: Vec<(u16, &'static str)>) -> Servidor {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind en loopback");
        let url = format!("http://{}", listener.local_addr().expect("dirección"));
        let pedidos = Arc::new(Mutex::new(Vec::new()));
        let cola = Arc::new(Mutex::new(VecDeque::from(respuestas)));
        let registro = Arc::clone(&pedidos);
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let pedido = leer_pedido(&stream);
                registro.lock().expect("lock").push(pedido);
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
        Servidor { url, pedidos }
    }

    fn peticiones(&self) -> usize {
        self.pedidos.lock().expect("lock").len()
    }

    fn pedido(&self, indice: usize) -> Pedido {
        self.pedidos.lock().expect("lock")[indice].clone()
    }
}

fn leer_pedido(stream: &std::net::TcpStream) -> Pedido {
    let mut lector = BufReader::new(stream);
    let mut linea = String::new();
    lector.read_line(&mut linea).expect("línea de inicio");
    let mut cabeceras = Vec::new();
    let mut largo = 0usize;
    loop {
        let mut fila = String::new();
        lector.read_line(&mut fila).expect("cabecera");
        let fila = fila.trim_end();
        if fila.is_empty() {
            break;
        }
        if let Some((k, v)) = fila.split_once(':') {
            let (k, v) = (k.trim().to_ascii_lowercase(), v.trim().to_string());
            if k == "content-length" {
                largo = v.parse().unwrap_or(0);
            }
            cabeceras.push((k, v));
        }
    }
    let mut cuerpo = vec![0u8; largo];
    lector.read_exact(&mut cuerpo).expect("cuerpo");
    Pedido {
        linea: linea.trim_end().to_string(),
        cabeceras,
        cuerpo: String::from_utf8_lossy(&cuerpo).into_owned(),
    }
}

fn config(deepseek: &str, anthropic: &str) -> Config {
    let mut cfg = Config::default();
    cfg.proveedores.deepseek_url = deepseek.into();
    cfg.proveedores.anthropic_url = anthropic.into();
    cfg.proveedores.espera_base_ms = 1;
    cfg.precios.insert(
        "deepseek-chat".into(),
        root::config::Precio {
            usd_entrada: 1.0,
            usd_salida: 2.0,
        },
    );
    cfg.precios.insert(
        "claude-haiku-5-5".into(),
        root::config::Precio {
            usd_entrada: 4.0,
            usd_salida: 8.0,
        },
    );
    cfg
}

fn eleccion(proveedor: Proveedor, modelo: &str) -> Eleccion {
    Eleccion {
        via: Via::Api,
        proveedor: Some(proveedor),
        modelo: Some(modelo.into()),
        razonamiento: false,
        motivo: "prueba".into(),
    }
}

fn prompt() -> Prompt {
    Prompt {
        sistema: Some("sé breve".into()),
        usuario: "hola".into(),
        max_tokens: 64,
        fase: "fase-prueba".into(),
    }
}

fn claves_de_prueba() -> Claves {
    Claves {
        deepseek: Some(Clave::nueva(CLAVE_PRUEBA)),
        anthropic: Some(Clave::nueva(CLAVE_PRUEBA)),
    }
}

/// Eventos de `eventos.jsonl` dentro de `home`.
fn eventos(home: &Path) -> Vec<Value> {
    let texto = fs::read_to_string(home.join("eventos.jsonl")).unwrap_or_default();
    texto
        .lines()
        .map(|l| serde_json::from_str(l).expect("cada línea es JSON"))
        .collect()
}

const DEEPSEEK_OK: &str = r#"{"choices":[{"message":{"content":"hola","reasoning_content":"pensé"}}],"usage":{"prompt_tokens":100,"completion_tokens":50}}"#;
const ANTHROPIC_OK: &str = r#"{"content":[{"type":"text","text":"ho"},{"type":"text","text":"la"}],"usage":{"input_tokens":10,"output_tokens":5}}"#;
const ERROR_500: (u16, &str) = (500, "{}");

#[test]
fn proveedores_deepseek_200_da_texto_costo_y_evento() {
    let srv = Servidor::arranca(vec![(200, DEEPSEEK_OK)]);
    let cfg = config(&srv.url, &srv.url);
    let home = tempfile::tempdir().expect("home");
    let breaker = Breaker::new();
    let claves = claves_de_prueba();
    let base = Instant::now();
    let reloj = || base;
    let ctx = Contexto {
        cfg: &cfg,
        home: home.path(),
        breaker: &breaker,
        claves: &claves,
        reloj: &reloj,
    };

    let r = llamar_con(
        &ctx,
        &eleccion(Proveedor::Deepseek, "deepseek-chat"),
        &prompt(),
        "c1",
    )
    .expect("200 da respuesta");

    assert_eq!(r.texto, "hola");
    assert_eq!((r.in_tokens, r.out_tokens), (100, 50));
    let esperado = 100.0 * 1.0 / 1e6 + 50.0 * 2.0 / 1e6;
    assert!((r.cost_usd - esperado).abs() < 1e-12);
    assert_eq!(r.intentos, 1);
    assert_eq!(srv.peticiones(), 1);
    let cuerpo: Value = serde_json::from_str(&srv.pedido(0).cuerpo).expect("el cuerpo es JSON");
    assert_eq!(cuerpo["model"], "deepseek-chat");
    assert_eq!(cuerpo["messages"][0]["role"], "system");

    let evs = eventos(home.path());
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0]["tipo"], "llamada");
    assert_eq!(evs[0]["via"], "api");
    assert_eq!(evs[0]["familia"], "deepseek");
    assert_eq!(evs[0]["corrida"], "c1");
    assert_eq!(evs[0]["fase"], "fase-prueba");
    assert_eq!(evs[0]["extra"]["intentos"], 1);
    assert_eq!(evs[0]["extra"]["razonamiento"], false);
    assert_eq!(evs[0]["in_tokens"], 100);
    assert!(evs[0]["cost_usd"].as_f64().expect("costo") > 0.0);
}

#[test]
fn proveedores_anthropic_200_manda_cabeceras_y_une_texto() {
    let srv = Servidor::arranca(vec![(200, ANTHROPIC_OK)]);
    let cfg = config(&srv.url, &srv.url);
    let home = tempfile::tempdir().expect("home");
    let breaker = Breaker::new();
    let claves = claves_de_prueba();
    let base = Instant::now();
    let reloj = || base;
    let ctx = Contexto {
        cfg: &cfg,
        home: home.path(),
        breaker: &breaker,
        claves: &claves,
        reloj: &reloj,
    };

    let r = llamar_con(
        &ctx,
        &eleccion(Proveedor::Anthropic, "claude-haiku-5-5"),
        &prompt(),
        "c2",
    )
    .expect("200 da respuesta");

    assert_eq!(r.texto, "hola");
    assert_eq!((r.in_tokens, r.out_tokens), (10, 5));
    assert!((r.cost_usd - (10.0 * 4.0 + 5.0 * 8.0) / 1e6).abs() < 1e-12);
    let pedido = srv.pedido(0);
    assert!(pedido.linea.starts_with("POST /v1/messages "));
    assert_eq!(pedido.cabecera("x-api-key"), Some(CLAVE_PRUEBA));
    assert_eq!(pedido.cabecera("anthropic-version"), Some("2023-06-01"));
    let evs = eventos(home.path());
    assert_eq!(evs[0]["via"], "api");
    assert_eq!(evs[0]["familia"], "anthropic");
}

#[test]
fn proveedores_429_429_200_reintenta_y_cuenta_tres_intentos() {
    let srv = Servidor::arranca(vec![(429, "{}"), (429, "{}"), (200, DEEPSEEK_OK)]);
    let cfg = config(&srv.url, &srv.url);
    let home = tempfile::tempdir().expect("home");
    let breaker = Breaker::new();
    let claves = claves_de_prueba();
    let base = Instant::now();
    let reloj = || base;
    let ctx = Contexto {
        cfg: &cfg,
        home: home.path(),
        breaker: &breaker,
        claves: &claves,
        reloj: &reloj,
    };

    let r = llamar_con(
        &ctx,
        &eleccion(Proveedor::Deepseek, "deepseek-chat"),
        &prompt(),
        "c3",
    )
    .expect("el tercer intento responde 200");

    assert_eq!(r.intentos, 3);
    assert_eq!(srv.peticiones(), 3);
    assert_eq!(eventos(home.path())[0]["extra"]["intentos"], 3);
}

#[test]
fn proveedores_429_tres_veces_da_error_http_y_evento_de_error() {
    let srv = Servidor::arranca(vec![(429, "{}"), (429, "{}"), (429, "{}")]);
    let cfg = config(&srv.url, &srv.url);
    let home = tempfile::tempdir().expect("home");
    let breaker = Breaker::new();
    let claves = claves_de_prueba();
    let base = Instant::now();
    let reloj = || base;
    let ctx = Contexto {
        cfg: &cfg,
        home: home.path(),
        breaker: &breaker,
        claves: &claves,
        reloj: &reloj,
    };

    let err = llamar_con(
        &ctx,
        &eleccion(Proveedor::Deepseek, "deepseek-chat"),
        &prompt(),
        "c4",
    )
    .expect_err("tres 429 fallan");

    assert!(matches!(err, ErrorProveedor::Http { estado: 429, .. }));
    assert_eq!(srv.peticiones(), 3);
    let evs = eventos(home.path());
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0]["extra"]["error"], "http_429");
    assert_eq!(evs[0]["extra"]["intentos"], 3);
    assert_eq!(evs[0]["in_tokens"], 0);
    assert_eq!(evs[0]["cost_usd"], 0.0);
}

#[test]
fn proveedores_400_no_reintenta() {
    let srv = Servidor::arranca(vec![(400, "{}")]);
    let cfg = config(&srv.url, &srv.url);
    let home = tempfile::tempdir().expect("home");
    let breaker = Breaker::new();
    let claves = claves_de_prueba();
    let base = Instant::now();
    let reloj = || base;
    let ctx = Contexto {
        cfg: &cfg,
        home: home.path(),
        breaker: &breaker,
        claves: &claves,
        reloj: &reloj,
    };

    let err = llamar_con(
        &ctx,
        &eleccion(Proveedor::Deepseek, "deepseek-chat"),
        &prompt(),
        "c5",
    )
    .expect_err("400 falla");

    assert!(
        matches!(err, ErrorProveedor::Pedido { .. }),
        "llegó {err:?}"
    );
    assert_eq!(srv.peticiones(), 1);
}

#[test]
fn proveedores_breaker_abre_tras_tres_fallas_y_vuelve_tras_60_s() {
    let mut respuestas = vec![ERROR_500; 9];
    respuestas.push((200, DEEPSEEK_OK));
    respuestas.extend(vec![ERROR_500; 2]);
    let srv = Servidor::arranca(respuestas);
    let cfg = config(&srv.url, &srv.url);
    let home = tempfile::tempdir().expect("home");
    let breaker = Breaker::new();
    let claves = claves_de_prueba();
    let base = Instant::now();
    let desfase = Cell::new(Duration::ZERO);
    let reloj = || base + desfase.get();
    let ctx = Contexto {
        cfg: &cfg,
        home: home.path(),
        breaker: &breaker,
        claves: &claves,
        reloj: &reloj,
    };
    let e = eleccion(Proveedor::Deepseek, "deepseek-chat");

    for _ in 0..3 {
        let err = llamar_con(&ctx, &e, &prompt(), "c6").expect_err("500 falla");
        assert!(matches!(err, ErrorProveedor::Http { estado: 500, .. }));
    }
    assert_eq!(srv.peticiones(), 9);

    let err = llamar_con(&ctx, &e, &prompt(), "c6").expect_err("breaker abierto");
    assert!(matches!(err, ErrorProveedor::BreakerAbierto(_)));
    assert_eq!(
        srv.peticiones(),
        9,
        "con el breaker abierto no sale ninguna petición"
    );
    assert_eq!(
        eventos(home.path()).last().expect("evento")["extra"]["error"],
        "breaker"
    );

    desfase.set(Duration::from_secs(61));
    let r =
        llamar_con(&ctx, &e, &prompt(), "c6").expect("pasado el enfriamiento, pasa una llamada");
    assert_eq!(r.texto, "hola");
    assert_eq!(srv.peticiones(), 10);

    // El éxito reinicia el contador: dos fallas nuevas no abren el breaker.
    for _ in 0..2 {
        let err = llamar_con(&ctx, &e, &prompt(), "c6").expect_err("500 falla");
        assert!(matches!(err, ErrorProveedor::Http { estado: 500, .. }));
    }
}

#[test]
fn proveedores_la_clave_nunca_aparece_en_eventos_ni_errores() {
    let srv = Servidor::arranca(vec![(500, "{}"), (500, "{}"), (500, "{}"), (401, "{}")]);
    let cfg = config(&srv.url, &srv.url);
    let home = tempfile::tempdir().expect("home");
    let breaker = Breaker::new();
    let claves = claves_de_prueba();
    let base = Instant::now();
    let reloj = || base;
    let ctx = Contexto {
        cfg: &cfg,
        home: home.path(),
        breaker: &breaker,
        claves: &claves,
        reloj: &reloj,
    };

    let err = llamar_con(
        &ctx,
        &eleccion(Proveedor::Deepseek, "deepseek-chat"),
        &prompt(),
        "c7",
    )
    .expect_err("500 falla");
    let err_401 = llamar_con(
        &ctx,
        &eleccion(Proveedor::Deepseek, "deepseek-chat"),
        &prompt(),
        "c7",
    )
    .expect_err("401 falla");

    // La clave sí llegó al servidor: la prueba mide algo real.
    assert_eq!(
        srv.pedido(0).cabecera("authorization"),
        Some(format!("Bearer {CLAVE_PRUEBA}").as_str())
    );
    let texto_evs = fs::read_to_string(home.path().join("eventos.jsonl")).expect("eventos");
    assert!(!texto_evs.contains(CLAVE_PRUEBA));
    assert!(!format!("{err:?}").contains(CLAVE_PRUEBA));
    assert!(!err.to_string().contains(CLAVE_PRUEBA));
    assert!(!format!("{err_401:?}").contains(CLAVE_PRUEBA));
    assert!(!err_401.to_string().contains(CLAVE_PRUEBA));
}

#[test]
fn proveedores_sin_clave_da_error_nombrado_y_no_pide_nada() {
    let srv = Servidor::arranca(vec![]);
    let cfg = config(&srv.url, &srv.url);
    let home = tempfile::tempdir().expect("home");
    let breaker = Breaker::new();
    let claves = Claves::default();
    let base = Instant::now();
    let reloj = || base;
    let ctx = Contexto {
        cfg: &cfg,
        home: home.path(),
        breaker: &breaker,
        claves: &claves,
        reloj: &reloj,
    };

    let err = llamar_con(
        &ctx,
        &eleccion(Proveedor::Deepseek, "deepseek-chat"),
        &prompt(),
        "c8",
    )
    .expect_err("sin clave falla");
    match err {
        ErrorProveedor::SinClave(nombre) => assert_eq!(nombre, "DEEPSEEK_API_KEY"),
        otro => panic!("esperaba SinClave, llegó {otro:?}"),
    }
    assert_eq!(srv.peticiones(), 0);
}

#[test]
fn proveedores_eleccion_invalida_no_pide_nada() {
    let srv = Servidor::arranca(vec![]);
    let cfg = config(&srv.url, &srv.url);
    let home = tempfile::tempdir().expect("home");
    let breaker = Breaker::new();
    let claves = claves_de_prueba();
    let base = Instant::now();
    let reloj = || base;
    let ctx = Contexto {
        cfg: &cfg,
        home: home.path(),
        breaker: &breaker,
        claves: &claves,
        reloj: &reloj,
    };

    let mut cupo = eleccion(Proveedor::Deepseek, "deepseek-chat");
    cupo.via = Via::Cupo;
    let sin_proveedor = Eleccion {
        proveedor: None,
        ..eleccion(Proveedor::Deepseek, "m")
    };
    let sin_modelo = Eleccion {
        modelo: None,
        ..eleccion(Proveedor::Deepseek, "m")
    };
    for e in [cupo, sin_proveedor, sin_modelo] {
        let err = llamar_con(&ctx, &e, &prompt(), "c9").expect_err("elección inválida");
        assert!(matches!(err, ErrorProveedor::Eleccion(_)), "llegó {err:?}");
    }
    assert_eq!(srv.peticiones(), 0);
}

#[test]
fn proveedores_json_malformado_en_200_no_reintenta() {
    let srv = Servidor::arranca(vec![(200, "no es json")]);
    let cfg = config(&srv.url, &srv.url);
    let home = tempfile::tempdir().expect("home");
    let breaker = Breaker::new();
    let claves = claves_de_prueba();
    let base = Instant::now();
    let reloj = || base;
    let ctx = Contexto {
        cfg: &cfg,
        home: home.path(),
        breaker: &breaker,
        claves: &claves,
        reloj: &reloj,
    };

    let err = llamar_con(
        &ctx,
        &eleccion(Proveedor::Deepseek, "deepseek-chat"),
        &prompt(),
        "c10",
    )
    .expect_err("JSON malo falla");
    assert!(matches!(err, ErrorProveedor::Respuesta(_)));
    assert_eq!(srv.peticiones(), 1);
}

#[test]
fn proveedores_401_cinco_veces_no_abre_el_breaker() {
    let mut respuestas = vec![(401, "{}"); 5];
    respuestas.push((200, DEEPSEEK_OK));
    let srv = Servidor::arranca(respuestas);
    let cfg = config(&srv.url, &srv.url);
    let home = tempfile::tempdir().expect("home");
    let breaker = Breaker::new();
    let claves = claves_de_prueba();
    let base = Instant::now();
    let reloj = || base;
    let ctx = Contexto {
        cfg: &cfg,
        home: home.path(),
        breaker: &breaker,
        claves: &claves,
        reloj: &reloj,
    };
    let e = eleccion(Proveedor::Deepseek, "deepseek-chat");

    for _ in 0..5 {
        let err = llamar_con(&ctx, &e, &prompt(), "c11").expect_err("401 falla");
        assert!(matches!(err, ErrorProveedor::Auth { .. }), "llegó {err:?}");
    }
    assert_eq!(srv.peticiones(), 5, "un 401 no se reintenta");
    let evs = eventos(home.path());
    assert_eq!(evs.len(), 5);
    assert!(evs.iter().all(|ev| ev["extra"]["error"] == "auth_401"));

    let r = llamar_con(&ctx, &e, &prompt(), "c11").expect("el breaker sigue cerrado");
    assert_eq!(r.texto, "hola");
    assert_eq!(srv.peticiones(), 6);
}

#[test]
fn proveedores_400_cinco_veces_no_abre_el_breaker() {
    let mut respuestas = vec![(400, "{}"); 5];
    respuestas.push((200, DEEPSEEK_OK));
    let srv = Servidor::arranca(respuestas);
    let cfg = config(&srv.url, &srv.url);
    let home = tempfile::tempdir().expect("home");
    let breaker = Breaker::new();
    let claves = claves_de_prueba();
    let base = Instant::now();
    let reloj = || base;
    let ctx = Contexto {
        cfg: &cfg,
        home: home.path(),
        breaker: &breaker,
        claves: &claves,
        reloj: &reloj,
    };
    let e = eleccion(Proveedor::Deepseek, "deepseek-chat");

    for _ in 0..5 {
        let err = llamar_con(&ctx, &e, &prompt(), "c12").expect_err("400 falla");
        assert!(
            matches!(err, ErrorProveedor::Pedido { .. }),
            "llegó {err:?}"
        );
    }
    assert_eq!(srv.peticiones(), 5, "un 400 no se reintenta");
    let evs = eventos(home.path());
    assert!(evs.iter().all(|ev| ev["extra"]["error"] == "pedido_400"));

    let r = llamar_con(&ctx, &e, &prompt(), "c12").expect("el breaker sigue cerrado");
    assert_eq!(r.texto, "hola");
    assert_eq!(srv.peticiones(), 6);
}

#[test]
fn proveedores_403_404_422_dan_auth_o_pedido_sin_reintentar() {
    let srv = Servidor::arranca(vec![(403, "{}"), (404, "{}"), (422, "{}")]);
    let cfg = config(&srv.url, &srv.url);
    let home = tempfile::tempdir().expect("home");
    let breaker = Breaker::new();
    let claves = claves_de_prueba();
    let base = Instant::now();
    let reloj = || base;
    let ctx = Contexto {
        cfg: &cfg,
        home: home.path(),
        breaker: &breaker,
        claves: &claves,
        reloj: &reloj,
    };
    let e = eleccion(Proveedor::Deepseek, "deepseek-chat");

    let err_403 = llamar_con(&ctx, &e, &prompt(), "c13").expect_err("403 falla");
    assert!(
        matches!(err_403, ErrorProveedor::Auth { .. }),
        "llegó {err_403:?}"
    );
    let err_404 = llamar_con(&ctx, &e, &prompt(), "c13").expect_err("404 falla");
    assert!(
        matches!(err_404, ErrorProveedor::Pedido { .. }),
        "llegó {err_404:?}"
    );
    let err_422 = llamar_con(&ctx, &e, &prompt(), "c13").expect_err("422 falla");
    assert!(
        matches!(err_422, ErrorProveedor::Pedido { .. }),
        "llegó {err_422:?}"
    );
    assert_eq!(srv.peticiones(), 3);
}

#[test]
fn proveedores_503_tres_seguidos_abren_el_breaker() {
    let srv = Servidor::arranca(vec![(503, "{}"); 9]);
    let cfg = config(&srv.url, &srv.url);
    let home = tempfile::tempdir().expect("home");
    let breaker = Breaker::new();
    let claves = claves_de_prueba();
    let base = Instant::now();
    let reloj = || base;
    let ctx = Contexto {
        cfg: &cfg,
        home: home.path(),
        breaker: &breaker,
        claves: &claves,
        reloj: &reloj,
    };
    let e = eleccion(Proveedor::Deepseek, "deepseek-chat");

    for _ in 0..3 {
        let err = llamar_con(&ctx, &e, &prompt(), "c14").expect_err("503 falla");
        assert!(
            matches!(err, ErrorProveedor::Http { estado: 503, .. }),
            "llegó {err:?}"
        );
    }
    assert_eq!(srv.peticiones(), 9, "cada llamada reintenta hasta agotar");

    let err = llamar_con(&ctx, &e, &prompt(), "c14").expect_err("breaker abierto");
    assert!(
        matches!(err, ErrorProveedor::BreakerAbierto(_)),
        "llegó {err:?}"
    );
    assert_eq!(
        srv.peticiones(),
        9,
        "con el breaker abierto no sale ningún pedido"
    );
}

#[test]
fn proveedores_429_agotado_cuenta_como_falla_del_breaker() {
    let srv = Servidor::arranca(vec![(429, "{}"); 9]);
    let cfg = config(&srv.url, &srv.url);
    let home = tempfile::tempdir().expect("home");
    let breaker = Breaker::new();
    let claves = claves_de_prueba();
    let base = Instant::now();
    let reloj = || base;
    let ctx = Contexto {
        cfg: &cfg,
        home: home.path(),
        breaker: &breaker,
        claves: &claves,
        reloj: &reloj,
    };
    let e = eleccion(Proveedor::Deepseek, "deepseek-chat");

    for _ in 0..3 {
        let err = llamar_con(&ctx, &e, &prompt(), "c15").expect_err("429 falla");
        assert!(
            matches!(err, ErrorProveedor::Http { estado: 429, .. }),
            "llegó {err:?}"
        );
    }
    assert_eq!(srv.peticiones(), 9);

    let err = llamar_con(&ctx, &e, &prompt(), "c15").expect_err("breaker abierto");
    assert!(
        matches!(err, ErrorProveedor::BreakerAbierto(_)),
        "llegó {err:?}"
    );
    assert_eq!(srv.peticiones(), 9);
}

#[test]
fn proveedores_json_malformado_no_abre_el_breaker() {
    let mut respuestas = vec![(200, "no es json"); 3];
    respuestas.push((200, DEEPSEEK_OK));
    let srv = Servidor::arranca(respuestas);
    let cfg = config(&srv.url, &srv.url);
    let home = tempfile::tempdir().expect("home");
    let breaker = Breaker::new();
    let claves = claves_de_prueba();
    let base = Instant::now();
    let reloj = || base;
    let ctx = Contexto {
        cfg: &cfg,
        home: home.path(),
        breaker: &breaker,
        claves: &claves,
        reloj: &reloj,
    };
    let e = eleccion(Proveedor::Deepseek, "deepseek-chat");

    for _ in 0..3 {
        let err = llamar_con(&ctx, &e, &prompt(), "c16").expect_err("JSON malo falla");
        assert!(matches!(err, ErrorProveedor::Respuesta(_)), "llegó {err:?}");
    }
    let r = llamar_con(&ctx, &e, &prompt(), "c16").expect("el breaker sigue cerrado");
    assert_eq!(r.texto, "hola");
    assert_eq!(srv.peticiones(), 4);
}

#[test]
fn proveedores_evento_no_escribible_devuelve_la_respuesta_completa() {
    let srv = Servidor::arranca(vec![(200, DEEPSEEK_OK)]);
    let cfg = config(&srv.url, &srv.url);
    let home = tempfile::tempdir().expect("home");
    // Un directorio con el nombre del archivo impide abrirlo para escribir, aun como root.
    fs::create_dir(home.path().join("eventos.jsonl")).expect("directorio");
    let breaker = Breaker::new();
    let claves = claves_de_prueba();
    let base = Instant::now();
    let reloj = || base;
    let ctx = Contexto {
        cfg: &cfg,
        home: home.path(),
        breaker: &breaker,
        claves: &claves,
        reloj: &reloj,
    };

    let err = llamar_con(
        &ctx,
        &eleccion(Proveedor::Deepseek, "deepseek-chat"),
        &prompt(),
        "c17",
    )
    .expect_err("el evento no se puede escribir");

    match err {
        ErrorProveedor::EventoNoRegistrado {
            respuesta,
            evento,
            causa,
        } => {
            assert_eq!(respuesta.texto, "hola");
            assert_eq!((respuesta.in_tokens, respuesta.out_tokens), (100, 50));
            assert!(respuesta.cost_usd > 0.0);
            assert_eq!(respuesta.intentos, 1);
            assert_eq!(evento.corrida, "c17");
            assert_eq!(evento.extra["intentos"], 1);
            assert!(!causa.is_empty());
        }
        otro => panic!("esperaba EventoNoRegistrado, llegó {otro:?}"),
    }
    // La llamada salió bien: el breaker sigue cerrado y no hubo reintentos.
    assert_eq!(srv.peticiones(), 1);
}

fn respuesta_de_prueba() -> Respuesta {
    Respuesta {
        texto: "hola".into(),
        in_tokens: 100,
        out_tokens: 50,
        cost_usd: 0.0002,
        latency_s: 0.1,
        modelo: "deepseek-chat".into(),
        intentos: 1,
    }
}

#[test]
fn proveedores_cli_exito_escribe_la_respuesta_y_devuelve_0() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let codigo = imprimir_para_cli(Ok(respuesta_de_prueba()), &mut out, &mut err);

    assert_eq!(codigo, 0);
    let json: Value = serde_json::from_slice(&out).expect("stdout es JSON");
    assert_eq!(json["texto"], "hola");
    assert_eq!(json["in_tokens"], 100);
    assert!(err.is_empty());
}

#[test]
fn proveedores_cli_evento_no_registrado_respuesta_en_stdout_y_evento_en_stderr() {
    let srv = Servidor::arranca(vec![(200, DEEPSEEK_OK)]);
    let cfg = config(&srv.url, &srv.url);
    let home = tempfile::tempdir().expect("home");
    fs::create_dir(home.path().join("eventos.jsonl")).expect("directorio");
    let breaker = Breaker::new();
    let claves = claves_de_prueba();
    let base = Instant::now();
    let reloj = || base;
    let ctx = Contexto {
        cfg: &cfg,
        home: home.path(),
        breaker: &breaker,
        claves: &claves,
        reloj: &reloj,
    };
    let res = llamar_con(
        &ctx,
        &eleccion(Proveedor::Deepseek, "deepseek-chat"),
        &prompt(),
        "c18",
    );

    let mut out = Vec::new();
    let mut err = Vec::new();
    let codigo = imprimir_para_cli(res, &mut out, &mut err);

    assert_eq!(codigo, 3);
    let json: Value = serde_json::from_slice(&out).expect("stdout es JSON");
    assert_eq!(json["texto"], "hola");
    let linea: Value = serde_json::from_slice(&err).expect("stderr es una línea JSON");
    assert_eq!(linea["corrida"], "c18");
    assert_eq!(linea["extra"]["intentos"], 1);
    assert!(!String::from_utf8_lossy(&out).contains(CLAVE_PRUEBA));
    assert!(!String::from_utf8_lossy(&err).contains(CLAVE_PRUEBA));
}

#[test]
fn proveedores_cli_error_da_codigo_3_y_no_muestra_la_clave() {
    let srv = Servidor::arranca(vec![(401, "{}")]);
    let cfg = config(&srv.url, &srv.url);
    let home = tempfile::tempdir().expect("home");
    let breaker = Breaker::new();
    let claves = claves_de_prueba();
    let base = Instant::now();
    let reloj = || base;
    let ctx = Contexto {
        cfg: &cfg,
        home: home.path(),
        breaker: &breaker,
        claves: &claves,
        reloj: &reloj,
    };
    let res = llamar_con(
        &ctx,
        &eleccion(Proveedor::Deepseek, "deepseek-chat"),
        &prompt(),
        "c19",
    );

    let mut out = Vec::new();
    let mut err = Vec::new();
    let codigo = imprimir_para_cli(res, &mut out, &mut err);

    assert_eq!(codigo, 3);
    assert!(out.is_empty());
    let texto_err = String::from_utf8(err).expect("UTF-8");
    assert!(
        texto_err.contains("401"),
        "el mensaje nombra el estado: {texto_err}"
    );
    assert!(!texto_err.contains(CLAVE_PRUEBA));
    assert!(!String::from_utf8_lossy(&out).contains(CLAVE_PRUEBA));
}
