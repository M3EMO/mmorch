//! Proveedores contra un servidor HTTP falso en 127.0.0.1. Ninguna prueba sale a internet.

use std::cell::Cell;
use std::collections::VecDeque;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use root::config::Config;
use root::observe;
use root::proveedores::{
    Breaker, Clave, Claves, Contexto, ErrorProveedor, MAX_TOKENS_JUEZ, Plazos, Prompt, Respuesta,
    causa_tope_alcanzado, costo_peor_caso, imprimir_para_cli, llamar_con, tokens_entrada_estimados,
};
use root::tipos::{Eleccion, Evento, Proveedor, Rol, TipoEvento, Via};
use serde_json::{Value, json};

mod common;
use common::{sse_anthropic, sse_anthropic_sin_uso, sse_deepseek, sse_deepseek_sin_uso};

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

/// Función que corre al recibir el pedido número `n` (desde 1), antes de responder.
type Gancho = (usize, Box<dyn Fn() + Send>);

/// Escribe la respuesta de un pedido del servidor con guion.
type Guion = Box<dyn Fn(&mut std::net::TcpStream) + Send>;

impl Servidor {
    /// Responde con la cola en orden. Cuando la cola se agota, responde 500.
    fn arranca(respuestas: Vec<(u16, String)>) -> Servidor {
        Servidor::arranca_con(respuestas, Duration::ZERO, None)
    }

    /// Como `arranca`: espera `demora` antes de responder y corre `gancho` al recibir su pedido.
    fn arranca_con(
        respuestas: Vec<(u16, String)>,
        demora: Duration,
        gancho: Option<Gancho>,
    ) -> Servidor {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind en loopback");
        let url = format!("http://{}", listener.local_addr().expect("dirección"));
        let pedidos = Arc::new(Mutex::new(Vec::new()));
        let cola = Arc::new(Mutex::new(VecDeque::from(respuestas)));
        let registro = Arc::clone(&pedidos);
        thread::spawn(move || {
            let mut recibidos = 0usize;
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let pedido = leer_pedido(&stream);
                registro.lock().expect("lock").push(pedido);
                recibidos += 1;
                if let Some((n, funcion)) = &gancho
                    && *n == recibidos
                {
                    funcion();
                }
                thread::sleep(demora);
                let (codigo, cuerpo) = cola
                    .lock()
                    .expect("lock")
                    .pop_front()
                    .unwrap_or((500, "{}".into()));
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
        Servidor { url, pedidos }
    }

    /// Como `arranca`, pero el guion escribe la respuesta entera (cabeceras y cuerpo) a su gusto:
    /// trozos, pausas y cortes. Sirve para los streams que llegan en partes.
    fn arranca_guion(guion: Guion) -> Servidor {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind en loopback");
        let url = format!("http://{}", listener.local_addr().expect("dirección"));
        let pedidos = Arc::new(Mutex::new(Vec::new()));
        let registro = Arc::clone(&pedidos);
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let pedido = leer_pedido(&stream);
                registro.lock().expect("lock").push(pedido);
                guion(&mut stream);
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
            umbral_entrada: None,
            usd_entrada_alto: None,
            usd_salida_alto: None,
        },
    );
    cfg.precios.insert(
        "claude-haiku-5-5".into(),
        root::config::Precio {
            usd_entrada: 4.0,
            usd_salida: 8.0,
            umbral_entrada: None,
            usd_entrada_alto: None,
            usd_salida_alto: None,
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
        rol: None,
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

/// Un `eventos.jsonl` que no se puede escribir pero sí se lee como vacío: un enlace a una carpeta
/// que no existe. El chequeo de tope lo lee sin gasto y el registro del evento no logra abrirlo.
fn evento_no_escribible(home: &Path) {
    std::os::unix::fs::symlink(
        home.join("carpeta-inexistente").join("eventos.jsonl"),
        home.join("eventos.jsonl"),
    )
    .expect("enlace");
}

/// Todas las líneas de `eventos.jsonl` dentro de `home`, reservas incluidas.
fn todos(home: &Path) -> Vec<Value> {
    let texto = fs::read_to_string(home.join("eventos.jsonl")).unwrap_or_default();
    texto
        .lines()
        .map(|l| serde_json::from_str(l).expect("cada línea es JSON"))
        .collect()
}

/// Eventos de `eventos.jsonl` dentro de `home`, sin las reservas.
fn eventos(home: &Path) -> Vec<Value> {
    todos(home)
        .into_iter()
        .filter(|ev| ev["tipo"] != "reserva")
        .collect()
}

/// Reservas de `eventos.jsonl` dentro de `home`.
fn reservas(home: &Path) -> Vec<Value> {
    todos(home)
        .into_iter()
        .filter(|ev| ev["tipo"] == "reserva")
        .collect()
}

/// Gancho que cambia `eventos.jsonl` por un directorio al recibir el pedido `n`. Las lecturas y
/// reservas anteriores salen bien; el registro final de la llamada ya no se puede escribir.
fn rompe_el_log_en_el_pedido(home: &Path, n: usize) -> Gancho {
    let ruta = home.join("eventos.jsonl");
    (
        n,
        Box::new(move || {
            let _ = fs::remove_file(&ruta);
            let _ = fs::create_dir(&ruta);
        }),
    )
}

/// Gasto de API del mes de `home`, con el mismo cálculo que usa Route.
fn gasto_api_del_mes(home: &Path) -> f64 {
    let ahora = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("reloj")
        .as_secs_f64();
    observe::uso::uso_en(
        &home.join("eventos.jsonl"),
        ahora,
        observe::uso::offset_local(),
        5,
    )
    .expect("lee el gasto")
    .api_mes_usd
}

fn deepseek_ok() -> String {
    sse_deepseek("hola", 100, 50, "stop")
}
fn anthropic_ok() -> String {
    sse_anthropic(&["ho", "la"], 10, 5, "end_turn")
}
fn error_500() -> (u16, String) {
    (500, "{}".into())
}

#[test]
fn proveedores_deepseek_200_da_texto_costo_y_evento() {
    let srv = Servidor::arranca(vec![(200, deepseek_ok())]);
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
        plazos: Plazos::default(),
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
    let srv = Servidor::arranca(vec![(200, anthropic_ok())]);
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
        plazos: Plazos::default(),
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
    let srv = Servidor::arranca(vec![
        (429, "{}".into()),
        (429, "{}".into()),
        (200, deepseek_ok()),
    ]);
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
        plazos: Plazos::default(),
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
    let srv = Servidor::arranca(vec![
        (429, "{}".into()),
        (429, "{}".into()),
        (429, "{}".into()),
    ]);
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
        plazos: Plazos::default(),
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
    let srv = Servidor::arranca(vec![(400, "{}".into())]);
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
        plazos: Plazos::default(),
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
    let mut respuestas = vec![error_500(); 9];
    respuestas.push((200, deepseek_ok()));
    respuestas.extend(vec![error_500(); 2]);
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
        plazos: Plazos::default(),
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
    let srv = Servidor::arranca(vec![
        (500, "{}".into()),
        (500, "{}".into()),
        (500, "{}".into()),
        (401, "{}".into()),
    ]);
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
        plazos: Plazos::default(),
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
        plazos: Plazos::default(),
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
        plazos: Plazos::default(),
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
    let srv = Servidor::arranca(vec![(200, "data: no es json\n\n".into())]);
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
        plazos: Plazos::default(),
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
    let mut respuestas = vec![(401, "{}".into()); 5];
    respuestas.push((200, deepseek_ok()));
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
        plazos: Plazos::default(),
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
    let mut respuestas = vec![(400, "{}".into()); 5];
    respuestas.push((200, deepseek_ok()));
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
        plazos: Plazos::default(),
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
    let srv = Servidor::arranca(vec![
        (403, "{}".into()),
        (404, "{}".into()),
        (422, "{}".into()),
    ]);
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
        plazos: Plazos::default(),
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
    let srv = Servidor::arranca(vec![(503, "{}".into()); 9]);
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
        plazos: Plazos::default(),
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
    let srv = Servidor::arranca(vec![(429, "{}".into()); 9]);
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
        plazos: Plazos::default(),
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
    let mut respuestas = vec![(200, "data: no es json\n\n".into()); 3];
    respuestas.push((200, deepseek_ok()));
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
        plazos: Plazos::default(),
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
    let home = tempfile::tempdir().expect("home");
    let srv = Servidor::arranca_con(
        vec![(200, deepseek_ok())],
        Duration::ZERO,
        Some(rompe_el_log_en_el_pedido(home.path(), 1)),
    );
    let cfg = config(&srv.url, &srv.url);
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
        plazos: Plazos::default(),
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
        truncada: false,
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
    let home = tempfile::tempdir().expect("home");
    let srv = Servidor::arranca_con(
        vec![(200, deepseek_ok())],
        Duration::ZERO,
        Some(rompe_el_log_en_el_pedido(home.path(), 1)),
    );
    let cfg = config(&srv.url, &srv.url);
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
        plazos: Plazos::default(),
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
    let srv = Servidor::arranca(vec![(401, "{}".into())]);
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
        plazos: Plazos::default(),
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

/// Llamada con un servidor que responde 503 siempre y un `eventos.jsonl` que no se puede escribir.
fn llamada_503_con_evento_no_escribible(
    home: &Path,
    corrida: &str,
) -> Result<Respuesta, ErrorProveedor> {
    let srv = Servidor::arranca_con(
        vec![(503, "{}".into()), (503, "{}".into()), (503, "{}".into())],
        Duration::ZERO,
        Some(rompe_el_log_en_el_pedido(home, 3)),
    );
    let cfg = config(&srv.url, &srv.url);
    let breaker = Breaker::new();
    let claves = claves_de_prueba();
    let base = Instant::now();
    let reloj = || base;
    let ctx = Contexto {
        cfg: &cfg,
        home,
        breaker: &breaker,
        claves: &claves,
        reloj: &reloj,
        plazos: Plazos::default(),
    };
    llamar_con(
        &ctx,
        &eleccion(Proveedor::Deepseek, "deepseek-chat"),
        &prompt(),
        corrida,
    )
}

#[test]
fn proveedores_error_original_y_registro_fallido_se_conservan_ambos() {
    let home = tempfile::tempdir().expect("home");
    let err = llamada_503_con_evento_no_escribible(home.path(), "c20").expect_err("503 siempre");

    match &err {
        ErrorProveedor::ErrorNoRegistrado {
            original,
            causa_registro,
            evento,
        } => {
            assert!(
                matches!(**original, ErrorProveedor::Http { estado: 503, .. }),
                "{original:?}"
            );
            assert!(!causa_registro.is_empty());
            assert_eq!(evento.corrida, "c20");
            assert_eq!(evento.extra["error"], "http_503");
        }
        otro => panic!("esperaba ErrorNoRegistrado, llegó {otro:?}"),
    }
    let texto = err.to_string();
    let causa = match &err {
        ErrorProveedor::ErrorNoRegistrado { causa_registro, .. } => causa_registro.clone(),
        _ => unreachable!(),
    };
    let en_original = texto.find("503").expect("el mensaje nombra el 503");
    let en_causa = texto
        .find(&causa)
        .expect("el mensaje trae la causa del registro");
    assert!(en_original < en_causa, "primero el original: {texto}");
    assert!(!texto.contains(CLAVE_PRUEBA));
}

#[test]
fn proveedores_cli_error_no_registrado_linea_del_evento_en_stderr_y_codigo_3() {
    let home = tempfile::tempdir().expect("home");
    let res = llamada_503_con_evento_no_escribible(home.path(), "c21");

    let mut out = Vec::new();
    let mut err = Vec::new();
    let codigo = imprimir_para_cli(res, &mut out, &mut err);

    assert_eq!(codigo, 3);
    assert!(out.is_empty());
    let texto = String::from_utf8(err).expect("UTF-8");
    assert!(texto.contains("503"), "{texto}");
    let ultima = texto.lines().last().expect("hay una línea de evento");
    let linea: Value = serde_json::from_str(ultima).expect("la última línea es el evento JSON");
    assert_eq!(linea["corrida"], "c21");
    assert_eq!(linea["extra"]["error"], "http_503");
    assert!(!texto.contains(CLAVE_PRUEBA));
}

// Respuestas truncadas por el tope de salida (Tarea 10c).

/// DeepSeek gastó todo el tope en razonamiento: contenido vacío y `finish_reason` = `length`.
fn deepseek_length_vacio() -> String {
    sse_deepseek("", 100, 32000, "length")
}
fn anthropic_max_tokens() -> String {
    sse_anthropic(&["ho"], 10, 5, "max_tokens")
}
fn anthropic_end_turn() -> String {
    sse_anthropic(&["hola"], 10, 5, "end_turn")
}

/// Llama una vez al proveedor con `cfg` y devuelve el resultado y el `home` con su `eventos.jsonl`.
fn llamada_con_servidor(
    cfg: &Config,
    proveedor: Proveedor,
    modelo: &str,
    corrida: &str,
) -> (Result<Respuesta, ErrorProveedor>, tempfile::TempDir) {
    llamada_con_plazos(cfg, Plazos::default(), proveedor, modelo, corrida)
}

/// Como `llamada_con_servidor`, con plazos inyectados.
fn llamada_con_plazos(
    cfg: &Config,
    plazos: Plazos,
    proveedor: Proveedor,
    modelo: &str,
    corrida: &str,
) -> (Result<Respuesta, ErrorProveedor>, tempfile::TempDir) {
    let home = tempfile::tempdir().expect("home");
    let breaker = Breaker::new();
    let claves = claves_de_prueba();
    let base = Instant::now();
    let reloj = || base;
    let ctx = Contexto {
        cfg,
        home: home.path(),
        breaker: &breaker,
        claves: &claves,
        reloj: &reloj,
        plazos,
    };
    let res = llamar_con(&ctx, &eleccion(proveedor, modelo), &prompt(), corrida);
    (res, home)
}

/// Plazos de los tests de streaming: inactividad de 120 ms y total de 3 s. Escalan los 120 s y el
/// timeout de la llamada sin esperar minutos.
fn plazos_de_prueba() -> Plazos {
    Plazos {
        inactividad: Duration::from_millis(120),
        total: Some(Duration::from_secs(3)),
    }
}

/// Cabeceras de una respuesta SSE sin longitud: el cuerpo termina cuando cierra la conexión.
fn cabeceras_sse(stream: &mut std::net::TcpStream) {
    let _ = stream.write_all(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
    );
}

/// Escribe un trozo del cuerpo. `false` si el cliente ya cerró la conexión.
fn escribe(stream: &mut std::net::TcpStream, trozo: &str) -> bool {
    stream.write_all(trozo.as_bytes()).is_ok() && stream.flush().is_ok()
}

/// Un trozo de texto de DeepSeek, como lo manda el stream.
fn trozo_deepseek(texto: &str) -> String {
    common::sse_evento(json!({"choices": [{"delta": {"content": texto}, "finish_reason": null}]}))
}

/// El cierre de un stream de DeepSeek con uso: `finish_reason`, el chunk de uso y `[DONE]`.
fn cierre_deepseek(in_tokens: u64, out_tokens: u64) -> String {
    common::sse_evento(json!({"choices": [{"delta": {}, "finish_reason": "stop"}]}))
        + &common::sse_evento(json!({
            "choices": [],
            "usage": {"prompt_tokens": in_tokens, "completion_tokens": out_tokens},
        }))
        + "data: [DONE]\n\n"
}

#[test]
fn proveedores_deepseek_length_con_contenido_vacio_es_exito_truncado() {
    let srv = Servidor::arranca(vec![(200, deepseek_length_vacio())]);
    let cfg = config(&srv.url, &srv.url);
    let (res, home) = llamada_con_servidor(&cfg, Proveedor::Deepseek, "deepseek-chat", "t1");

    let r = res.expect("una respuesta truncada es éxito, no error");
    assert!(r.truncada);
    assert_eq!(r.texto, "");
    assert_eq!(r.intentos, 1);
    // Cobra los tokens reales: 100 de entrada y 32 000 de salida.
    let esperado = 100.0 * 1.0 / 1e6 + 32_000.0 * 2.0 / 1e6;
    assert!((r.cost_usd - esperado).abs() < 1e-12, "{}", r.cost_usd);

    let evs = eventos(home.path());
    assert_eq!(evs.len(), 1);
    assert!(evs[0]["cost_usd"].as_f64().expect("costo") > 0.0);
    assert_eq!(evs[0]["extra"]["truncada"], true);
    assert!(evs[0]["extra"].get("error").is_none());
}

#[test]
fn proveedores_deepseek_length_no_abre_el_breaker() {
    let mut respuestas = vec![(200, deepseek_length_vacio()); 3];
    respuestas.push((200, deepseek_ok()));
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
        plazos: Plazos::default(),
    };
    let e = eleccion(Proveedor::Deepseek, "deepseek-chat");

    for _ in 0..3 {
        let r = llamar_con(&ctx, &e, &prompt(), "t2").expect("truncada no es falla");
        assert!(r.truncada);
    }
    let r = llamar_con(&ctx, &e, &prompt(), "t2").expect("el breaker sigue cerrado");
    assert_eq!(r.texto, "hola");
    assert_eq!(srv.peticiones(), 4);
}

#[test]
fn proveedores_deepseek_length_con_contenido_null_es_truncada_vacia() {
    let cuerpo = common::sse_evento(json!({
        "choices": [{"delta": {"reasoning_content": "pensé mucho"}, "finish_reason": "length"}],
    })) + &common::sse_evento(json!({
        "choices": [],
        "usage": {"prompt_tokens": 100, "completion_tokens": 32000},
    })) + "data: [DONE]\n\n";
    let srv = Servidor::arranca(vec![(200, cuerpo)]);
    let cfg = config(&srv.url, &srv.url);
    let (res, _home) = llamada_con_servidor(&cfg, Proveedor::Deepseek, "deepseek-chat", "t3");

    let r = res.expect("content null con length es truncada, no error de respuesta");
    assert!(r.truncada);
    assert_eq!(r.texto, "");
}

#[test]
fn proveedores_deepseek_stop_no_es_truncada() {
    let srv = Servidor::arranca(vec![(200, deepseek_ok())]);
    let cfg = config(&srv.url, &srv.url);
    let (res, home) = llamada_con_servidor(&cfg, Proveedor::Deepseek, "deepseek-chat", "t4");

    let r = res.expect("200 da respuesta");
    assert!(!r.truncada);
    assert_eq!(r.texto, "hola");
    assert!(eventos(home.path())[0]["extra"].get("truncada").is_none());
}

#[test]
fn proveedores_deepseek_sin_finish_reason_no_es_truncada() {
    let srv = Servidor::arranca(vec![(200, deepseek_ok())]);
    let cfg = config(&srv.url, &srv.url);
    let (res, _home) = llamada_con_servidor(&cfg, Proveedor::Deepseek, "deepseek-chat", "t5");

    assert!(!res.expect("200 da respuesta").truncada);
}

#[test]
fn proveedores_anthropic_max_tokens_es_truncada_con_costo_real() {
    let srv = Servidor::arranca(vec![(200, anthropic_max_tokens())]);
    let cfg = config(&srv.url, &srv.url);
    let (res, home) = llamada_con_servidor(&cfg, Proveedor::Anthropic, "claude-haiku-5-5", "t6");

    let r = res.expect("truncada es éxito");
    assert!(r.truncada);
    assert_eq!(r.texto, "ho");
    assert!((r.cost_usd - (10.0 * 4.0 + 5.0 * 8.0) / 1e6).abs() < 1e-12);
    let evs = eventos(home.path());
    assert!(evs[0]["cost_usd"].as_f64().expect("costo") > 0.0);
    assert_eq!(evs[0]["extra"]["truncada"], true);
}

#[test]
fn proveedores_anthropic_end_turn_no_es_truncada() {
    let srv = Servidor::arranca(vec![(200, anthropic_end_turn())]);
    let cfg = config(&srv.url, &srv.url);
    let (res, _home) = llamada_con_servidor(&cfg, Proveedor::Anthropic, "claude-haiku-5-5", "t7");

    assert!(!res.expect("200 da respuesta").truncada);
}

#[test]
fn proveedores_sin_precio_no_envia_y_registra_gate() {
    let srv = Servidor::arranca(vec![(200, deepseek_ok())]);
    let mut cfg = config(&srv.url, &srv.url);
    cfg.precios.clear();
    let (res, home) = llamada_con_servidor(&cfg, Proveedor::Deepseek, "deepseek-chat", "t8");

    let err = res.expect_err("sin precio no hay estimación: la llamada se rechaza");
    assert!(matches!(err, ErrorProveedor::Tope(_)), "{err}");
    assert!(
        err.to_string()
            .contains("sin precio en root.toml para deepseek-chat")
    );
    assert_eq!(srv.peticiones(), 0, "no debe enviarse nada");
    let evs = eventos(home.path());
    assert_eq!(evs.len(), 1, "solo el Gate, sin evento Llamada");
    assert_eq!(evs[0]["tipo"], "gate");
    assert_eq!(evs[0]["fase"], "tope");
    assert_eq!(evs[0]["extra"]["modelo"], "deepseek-chat");
    assert!(evs[0]["extra"]["estimado"].is_null());
}

#[test]
fn proveedores_modelo_nuevo_sin_precio_no_envia_aunque_otros_tengan_precio() {
    let srv = Servidor::arranca(vec![(200, deepseek_ok())]);
    let cfg = config(&srv.url, &srv.url);
    let (res, home) = llamada_con_servidor(&cfg, Proveedor::Deepseek, "deepseek-reasoner", "t9");

    assert!(matches!(res, Err(ErrorProveedor::Tope(_))));
    assert_eq!(srv.peticiones(), 0);
    let evs = eventos(home.path());
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0]["extra"]["modelo"], "deepseek-reasoner");
}

#[test]
fn proveedores_peor_caso_que_no_entra_en_el_margen_no_envia() {
    let srv = Servidor::arranca(vec![(200, deepseek_ok())]);
    let mut cfg = config(&srv.url, &srv.url);
    cfg.balance.api.tope_usd = 1.0;
    let home = tempfile::tempdir().expect("home");
    // Gasto de 0,99995 USD este mes: margen de 0,00005 USD.
    let gasto = Evento {
        ts: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("reloj")
            .as_secs_f64(),
        corrida: "previa".into(),
        tipo: TipoEvento::Llamada,
        via: Some(Via::Api),
        cost_usd: 0.99995,
        ..Evento::default()
    };
    observe::registrar_en(&home.path().join("eventos.jsonl"), &gasto).expect("siembra");
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
        plazos: Plazos::default(),
    };
    // El peor caso es 64 tokens de salida a 2 USD por millón: 0,000128 USD, más que el margen.
    let err = llamar_con(
        &ctx,
        &eleccion(Proveedor::Deepseek, "deepseek-chat"),
        &prompt(),
        "t10",
    )
    .expect_err("el peor caso no entra en el margen");
    assert!(matches!(err, ErrorProveedor::Tope(_)), "{err}");
    assert_eq!(srv.peticiones(), 0);
    let gates: Vec<Value> = eventos(home.path())
        .into_iter()
        .filter(|ev| ev["tipo"] == "gate")
        .collect();
    assert_eq!(gates.len(), 1);
    let margen = gates[0]["extra"]["margen"].as_f64().expect("margen");
    let estimado = gates[0]["extra"]["estimado"].as_f64().expect("estimado");
    assert!((margen - 0.00005).abs() < 1e-9, "margen {margen}");
    assert!(estimado > margen, "estimado {estimado}");
}

#[test]
fn proveedores_tokens_de_entrada_cuentan_bytes_utf8() {
    // "€" ocupa 3 bytes: 1000 caracteres son 3000 bytes, y un token no ocupa menos de un byte.
    let prompt = Prompt {
        sistema: None,
        usuario: "€".repeat(1000),
        max_tokens: 1,
        fase: "f".into(),
    };
    assert!(tokens_entrada_estimados(&prompt) >= 3000);
}

#[test]
fn proveedores_tokens_de_entrada_suman_sistema_y_usuario() {
    let prompt = Prompt {
        sistema: Some("abc".into()),
        usuario: "de".into(),
        max_tokens: 1,
        fase: "f".into(),
    };
    assert_eq!(tokens_entrada_estimados(&prompt), 5);
}

#[test]
fn proveedores_costo_peor_caso_suma_entrada_y_maximo_de_salida() {
    let precio = root::config::Precio {
        usd_entrada: 1.0,
        usd_salida: 2.0,
        umbral_entrada: None,
        usd_entrada_alto: None,
        usd_salida_alto: None,
    };
    // Un millón de entrada a 1 USD y medio millón de salida a 2 USD por millón.
    let costo = costo_peor_caso(&precio, 1_000_000, 500_000);
    assert!((costo - 2.0).abs() < 1e-12, "{costo}");
}

#[test]
fn proveedores_respuesta_sin_campo_truncada_se_lee_como_falsa() {
    let texto = r#"{"texto":"hola","in_tokens":1,"out_tokens":1,"cost_usd":0.0,"latency_s":0.1,"modelo":"m","intentos":1}"#;
    let r: Respuesta = serde_json::from_str(texto).expect("sin el campo, sigue leyendo");
    assert!(!r.truncada);
}

#[test]
fn proveedores_causa_tope_alcanzado_nombra_los_tokens() {
    assert_eq!(
        causa_tope_alcanzado(32_000),
        "tope de salida alcanzado (32000 tokens)"
    );
}

#[test]
fn proveedores_tope_de_los_jueces_es_32000() {
    assert_eq!(MAX_TOKENS_JUEZ, 32_000);
}

#[test]
fn proveedores_timeout_de_lectura_deja_costo_estimado_y_error_timeout() {
    let srv = Servidor::arranca_con(vec![(200, deepseek_ok())], Duration::from_secs(2), None);
    let mut cfg = config(&srv.url, &srv.url);
    cfg.proveedores.timeout_s = 1;
    let (res, home) = llamada_con_servidor(&cfg, Proveedor::Deepseek, "deepseek-chat", "t21");

    let err = res.expect_err("el servidor tarda más que el timeout");
    assert!(matches!(err, ErrorProveedor::Timeout(_)), "llegó {err:?}");
    let evs = eventos(home.path());
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0]["extra"]["error"], "timeout");
    assert_eq!(evs[0]["extra"]["costo_estimado"], true);
    assert!(evs[0]["cost_usd"].as_f64().expect("costo") > 0.0);
}

#[test]
fn proveedores_timeout_de_lectura_no_se_reintenta() {
    let srv = Servidor::arranca_con(vec![(200, deepseek_ok())], Duration::from_secs(2), None);
    let mut cfg = config(&srv.url, &srv.url);
    cfg.proveedores.timeout_s = 1;
    let (res, _home) = llamada_con_servidor(&cfg, Proveedor::Deepseek, "deepseek-chat", "t22");

    assert!(matches!(res, Err(ErrorProveedor::Timeout(_))));
    assert_eq!(srv.peticiones(), 1, "repetir el pedido repite el costo");
}

#[test]
fn proveedores_conexion_rechazada_se_reintenta_y_vale_cero() {
    // Un puerto libre: el listener se cierra antes de llamar, así que la conexión se rechaza.
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind en loopback");
    let url = format!("http://{}", listener.local_addr().expect("dirección"));
    drop(listener);
    let cfg = config(&url, &url);
    let (res, home) = llamada_con_servidor(&cfg, Proveedor::Deepseek, "deepseek-chat", "t23");

    let err = res.expect_err("nadie escucha en el puerto");
    assert!(matches!(err, ErrorProveedor::Red(_)), "llegó {err:?}");
    let evs = eventos(home.path());
    assert_eq!(evs[0]["extra"]["intentos"], 3);
    assert_eq!(evs[0]["extra"]["error"], "red");
    assert_eq!(evs[0]["cost_usd"].as_f64(), Some(0.0));
    assert!(evs[0]["extra"].get("costo_estimado").is_none());
    assert_eq!(reservas(home.path()).len(), 3);
}

#[test]
fn proveedores_5xx_suma_peor_caso_por_intento_y_reintenta() {
    let srv = Servidor::arranca(vec![error_500(); 3]);
    let cfg = config(&srv.url, &srv.url);
    let (res, home) = llamada_con_servidor(&cfg, Proveedor::Deepseek, "deepseek-chat", "t24");

    let err = res.expect_err("500 agotado");
    assert!(
        matches!(err, ErrorProveedor::Http { estado: 500, .. }),
        "llegó {err:?}"
    );
    assert_eq!(srv.peticiones(), 3);
    let precio = cfg.precios.get("deepseek-chat").expect("precio");
    let peor = costo_peor_caso(precio, tokens_entrada_estimados(&prompt()), 64);
    let evs = eventos(home.path());
    assert_eq!(evs[0]["extra"]["intentos"], 3);
    assert_eq!(evs[0]["extra"]["costo_estimado"], true);
    let costo = evs[0]["cost_usd"].as_f64().expect("costo");
    assert!((costo - 3.0 * peor).abs() < 1e-12, "costo {costo}");
}

#[test]
fn proveedores_200_que_no_parsea_cuesta_peor_caso_y_no_reintenta() {
    let srv = Servidor::arranca(vec![(200, "data: no es json\n\n".into())]);
    let cfg = config(&srv.url, &srv.url);
    let (res, home) = llamada_con_servidor(&cfg, Proveedor::Deepseek, "deepseek-chat", "t25");

    assert!(matches!(res, Err(ErrorProveedor::Respuesta(_))));
    assert_eq!(srv.peticiones(), 1);
    let precio = cfg.precios.get("deepseek-chat").expect("precio");
    let peor = costo_peor_caso(precio, tokens_entrada_estimados(&prompt()), 64);
    let evs = eventos(home.path());
    assert_eq!(evs[0]["extra"]["error"], "respuesta");
    assert_eq!(evs[0]["extra"]["costo_estimado"], true);
    let costo = evs[0]["cost_usd"].as_f64().expect("costo");
    assert!((costo - peor).abs() < 1e-12, "costo {costo}");
}

#[test]
fn proveedores_200_escribe_reserva_y_la_cierra_con_el_costo_real() {
    let srv = Servidor::arranca(vec![(200, deepseek_ok())]);
    let cfg = config(&srv.url, &srv.url);
    let (res, home) = llamada_con_servidor(&cfg, Proveedor::Deepseek, "deepseek-chat", "t26");
    res.expect("200 responde");

    let precio = cfg.precios.get("deepseek-chat").expect("precio");
    let peor = costo_peor_caso(precio, tokens_entrada_estimados(&prompt()), 64);
    let reserva = reservas(home.path());
    assert_eq!(reserva.len(), 1);
    assert_eq!(reserva[0]["via"], "api");
    assert!((reserva[0]["cost_usd"].as_f64().expect("costo") - peor).abs() < 1e-12);
    let id = reserva[0]["extra"]["reserva"].as_str().expect("id");
    let evs = eventos(home.path());
    assert_eq!(evs[0]["extra"]["reservas"], serde_json::json!([id]));
    // La reserva no suma: el gasto del mes es el costo real de la llamada, no real más peor.
    let real = (100.0 * 1.0 + 50.0 * 2.0) / 1e6;
    assert!((gasto_api_del_mes(home.path()) - real).abs() < 1e-12);
}

#[test]
fn proveedores_reintento_con_tope_lleno_no_envia_y_cierra_la_reserva() {
    let srv = Servidor::arranca(vec![error_500(), (200, deepseek_ok())]);
    let mut cfg = config(&srv.url, &srv.url);
    // Margen para un peor caso de deepseek-chat (141 millonésimas de USD), y no para dos.
    cfg.balance.api.tope_usd = 0.0002;
    let (res, home) = llamada_con_servidor(&cfg, Proveedor::Deepseek, "deepseek-chat", "t27");

    let err = res.expect_err("el reintento no entra en el margen");
    assert!(matches!(err, ErrorProveedor::Tope(_)), "llegó {err:?}");
    assert_eq!(srv.peticiones(), 1, "el reintento no se envía");
    let precio = cfg.precios.get("deepseek-chat").expect("precio");
    let peor = costo_peor_caso(precio, tokens_entrada_estimados(&prompt()), 64);
    let evs = eventos(home.path());
    let llamadas: Vec<&Value> = evs.iter().filter(|ev| ev["tipo"] == "llamada").collect();
    assert_eq!(llamadas.len(), 1, "el evento final cierra la llamada");
    assert_eq!(llamadas[0]["extra"]["error"], "tope");
    assert_eq!(llamadas[0]["extra"]["intentos"], 1);
    assert_eq!(evs.iter().filter(|ev| ev["tipo"] == "gate").count(), 1);
    // Solo el primer intento tuvo reserva: el reintento rechazado no llegó a escribirla.
    assert_eq!(reservas(home.path()).len(), 1);
    // La reserva del intento cortado queda cerrada: cuenta una vez, no dos.
    assert!((gasto_api_del_mes(home.path()) - peor).abs() < 1e-12);
}

#[test]
fn proveedores_reserva_no_escribible_no_envia_nada() {
    let srv = Servidor::arranca(vec![(200, deepseek_ok())]);
    let cfg = config(&srv.url, &srv.url);
    let home = tempfile::tempdir().expect("home");
    evento_no_escribible(home.path());
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
        plazos: Plazos::default(),
    };

    let err = llamar_con(
        &ctx,
        &eleccion(Proveedor::Deepseek, "deepseek-chat"),
        &prompt(),
        "t28",
    )
    .expect_err("sin reserva no se envía");
    assert!(matches!(err, ErrorProveedor::Tope(_)), "llegó {err:?}");
    assert_eq!(srv.peticiones(), 0);
}

/// Configuración con los precios por defecto de Root y el servidor falso como proveedor.
fn config_por_defecto(url: &str) -> Config {
    let mut cfg = Config::default();
    cfg.proveedores.deepseek_url = url.into();
    cfg.proveedores.anthropic_url = url.into();
    cfg.proveedores.espera_base_ms = 1;
    cfg
}

/// Una llamada con la elección dada. Devuelve el resultado; el servidor guarda el pedido.
fn llamada_con_eleccion(
    cfg: &Config,
    e: &Eleccion,
    corrida: &str,
) -> Result<Respuesta, ErrorProveedor> {
    let home = tempfile::tempdir().expect("home");
    let breaker = Breaker::new();
    let claves = claves_de_prueba();
    let base = Instant::now();
    let reloj = || base;
    let ctx = Contexto {
        cfg,
        home: home.path(),
        breaker: &breaker,
        claves: &claves,
        reloj: &reloj,
        plazos: Plazos::default(),
    };
    llamar_con(&ctx, e, &prompt(), corrida)
}

/// Elección con razonamiento, como la de Verifica y Juzga.
fn eleccion_con_razonamiento(proveedor: Proveedor, modelo: &str) -> Eleccion {
    Eleccion {
        razonamiento: true,
        ..eleccion(proveedor, modelo)
    }
}

fn anthropic_entrada_alta() -> String {
    sse_anthropic(&["ok"], 150_000, 1000, "end_turn")
}
fn anthropic_entrada_baja() -> String {
    sse_anthropic(&["ok"], 1000, 1000, "end_turn")
}

#[test]
fn proveedores_juzga_deepseek_manda_thinking_enabled() {
    let srv = Servidor::arranca(vec![(200, deepseek_ok())]);
    let cfg = config_por_defecto(&srv.url);
    let e = eleccion_con_razonamiento(Proveedor::Deepseek, "deepseek-flash");
    llamada_con_eleccion(&cfg, &e, "j1").expect("200 da respuesta");
    let cuerpo: Value = serde_json::from_str(&srv.pedido(0).cuerpo).expect("JSON");
    assert_eq!(cuerpo["thinking"], serde_json::json!({"type": "enabled"}));
}

#[test]
fn proveedores_delega_deepseek_manda_thinking_disabled() {
    let srv = Servidor::arranca(vec![(200, deepseek_ok())]);
    let cfg = config_por_defecto(&srv.url);
    let e = eleccion(Proveedor::Deepseek, "deepseek-flash");
    llamada_con_eleccion(&cfg, &e, "d1").expect("200 da respuesta");
    let cuerpo: Value = serde_json::from_str(&srv.pedido(0).cuerpo).expect("JSON");
    assert_eq!(cuerpo["thinking"], serde_json::json!({"type": "disabled"}));
}

#[test]
fn proveedores_anthropic_con_razonamiento_manda_adaptive_sin_budget_tokens() {
    let srv = Servidor::arranca(vec![(200, anthropic_ok())]);
    let cfg = config_por_defecto(&srv.url);
    let e = eleccion_con_razonamiento(Proveedor::Anthropic, "claude-haiku-5-5");
    llamada_con_eleccion(&cfg, &e, "a1").expect("200 da respuesta");
    let pedido = srv.pedido(0);
    let cuerpo: Value = serde_json::from_str(&pedido.cuerpo).expect("JSON");
    assert_eq!(cuerpo["thinking"], serde_json::json!({"type": "adaptive"}));
    assert!(
        !pedido.cuerpo.contains("budget_tokens"),
        "{}",
        pedido.cuerpo
    );
    assert!(cuerpo.get("output_config").is_none());
}

#[test]
fn proveedores_anthropic_sin_razonamiento_no_manda_thinking() {
    let srv = Servidor::arranca(vec![(200, anthropic_ok())]);
    let cfg = config_por_defecto(&srv.url);
    let e = eleccion(Proveedor::Anthropic, "claude-haiku-5-5");
    llamada_con_eleccion(&cfg, &e, "a2").expect("200 da respuesta");
    let cuerpo: Value = serde_json::from_str(&srv.pedido(0).cuerpo).expect("JSON");
    assert!(cuerpo.get("thinking").is_none(), "{cuerpo}");
}

#[test]
fn proveedores_haiku_con_150000_de_entrada_cobra_el_tramo_alto() {
    let srv = Servidor::arranca(vec![(200, anthropic_entrada_alta())]);
    let cfg = config_por_defecto(&srv.url);
    let e = eleccion(Proveedor::Anthropic, "claude-haiku-5-5");
    let r = llamada_con_eleccion(&cfg, &e, "h1").expect("200 da respuesta");
    let esperado = (150_000.0 * 0.50 + 1_000.0 * 2.50) / 1e6;
    assert!(
        (r.cost_usd - esperado).abs() < 1e-12,
        "costo {}",
        r.cost_usd
    );
}

#[test]
fn proveedores_haiku_con_1000_de_entrada_cobra_el_tramo_normal() {
    let srv = Servidor::arranca(vec![(200, anthropic_entrada_baja())]);
    let cfg = config_por_defecto(&srv.url);
    let e = eleccion(Proveedor::Anthropic, "claude-haiku-5-5");
    let r = llamada_con_eleccion(&cfg, &e, "h2").expect("200 da respuesta");
    let esperado = (1_000.0 * 0.10 + 1_000.0 * 0.50) / 1e6;
    assert!(
        (r.cost_usd - esperado).abs() < 1e-12,
        "costo {}",
        r.cost_usd
    );
}

#[test]
fn proveedores_juzga_usa_timeout_juez_s_y_delega_usa_timeout_s() {
    // El servidor tarda 2 s: con timeout_s = 1 Delega se corta, y con timeout_juez_s = 5 Juzga responde.
    let srv = Servidor::arranca_con(
        vec![(200, deepseek_ok()), (200, deepseek_ok())],
        Duration::from_secs(2),
        None,
    );
    let mut cfg = config_por_defecto(&srv.url);
    cfg.proveedores.timeout_s = 1;
    cfg.proveedores.timeout_juez_s = 5;

    let juzga = Eleccion {
        rol: Some(Rol::Juzga),
        ..eleccion_con_razonamiento(Proveedor::Deepseek, "deepseek-flash")
    };
    let r = llamada_con_eleccion(&cfg, &juzga, "tj1");
    assert!(r.is_ok(), "el juez tiene 5 s y llegó {r:?}");

    let delega = Eleccion {
        rol: Some(Rol::Delega),
        ..eleccion(Proveedor::Deepseek, "deepseek-flash")
    };
    let err = llamada_con_eleccion(&cfg, &delega, "td1").expect_err("delega tiene 1 s");
    assert!(matches!(err, ErrorProveedor::Timeout(_)), "llegó {err:?}");
}

#[test]
fn proveedores_stream_con_un_trozo_cada_60_s_no_corta_por_inactividad() {
    // Escalado de 15 minutos con un trozo por minuto: 15 trozos a 60 ms, con inactividad de 120 ms.
    let srv = Servidor::arranca_guion(Box::new(|s| {
        cabeceras_sse(s);
        for _ in 0..15 {
            thread::sleep(Duration::from_millis(60));
            if !escribe(s, &trozo_deepseek("a")) {
                return;
            }
        }
        escribe(s, &cierre_deepseek(7, 3));
    }));
    let cfg = config(&srv.url, &srv.url);
    let inicio = Instant::now();
    let (res, home) = llamada_con_plazos(
        &cfg,
        plazos_de_prueba(),
        Proveedor::Deepseek,
        "deepseek-chat",
        "s1",
    );

    let r = res.expect("un stream que sigue mandando bytes termina bien");
    assert!(
        inicio.elapsed() < Duration::from_secs(5),
        "{:?}",
        inicio.elapsed()
    );
    assert_eq!(r.texto, "a".repeat(15));
    assert_eq!(srv.peticiones(), 1);
    assert!(eventos(home.path())[0]["extra"].get("error").is_none());
}

#[test]
fn proveedores_stream_que_se_calla_mas_que_la_inactividad_corta_y_cobra_peor_caso() {
    // Manda un trozo y se queda callado 1 s; la inactividad de 120 ms lo corta antes.
    let srv = Servidor::arranca_guion(Box::new(|s| {
        cabeceras_sse(s);
        escribe(s, &trozo_deepseek("a"));
        thread::sleep(Duration::from_secs(1));
        escribe(s, &cierre_deepseek(7, 3));
    }));
    let cfg = config(&srv.url, &srv.url);
    let inicio = Instant::now();
    let (res, home) = llamada_con_plazos(
        &cfg,
        plazos_de_prueba(),
        Proveedor::Deepseek,
        "deepseek-chat",
        "s2",
    );

    let err = res.expect_err("la conexión muerta se corta");
    assert!(matches!(err, ErrorProveedor::Timeout(_)), "llegó {err:?}");
    assert!(
        inicio.elapsed() < Duration::from_secs(1),
        "{:?}",
        inicio.elapsed()
    );
    assert_eq!(srv.peticiones(), 1, "repetir el pedido repite el costo");
    let evs = eventos(home.path());
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0]["extra"]["error"], "timeout");
    assert_eq!(evs[0]["extra"]["costo_estimado"], true);
    let precio = &cfg.precios["deepseek-chat"];
    let peor = costo_peor_caso(precio, tokens_entrada_estimados(&prompt()), 64);
    assert!((evs[0]["cost_usd"].as_f64().expect("costo") - peor).abs() < 1e-12);
}

#[test]
fn proveedores_stream_que_nunca_termina_corta_por_plazo_total_con_peor_caso() {
    // Manda bytes cada 20 ms para siempre: la inactividad no salta, y el total de 300 ms sí.
    let srv = Servidor::arranca_guion(Box::new(|s| {
        cabeceras_sse(s);
        for _ in 0..500 {
            if !escribe(s, ": sigo vivo\n\n") {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
    }));
    let cfg = config(&srv.url, &srv.url);
    let plazos = Plazos {
        inactividad: Duration::from_millis(120),
        total: Some(Duration::from_millis(300)),
    };
    let inicio = Instant::now();
    let (res, home) = llamada_con_plazos(&cfg, plazos, Proveedor::Deepseek, "deepseek-chat", "s3");

    let err = res.expect_err("el plazo total corta el stream");
    assert!(matches!(err, ErrorProveedor::Timeout(_)), "llegó {err:?}");
    assert!(
        inicio.elapsed() < Duration::from_secs(2),
        "{:?}",
        inicio.elapsed()
    );
    assert_eq!(srv.peticiones(), 1);
    let evs = eventos(home.path());
    assert_eq!(evs[0]["extra"]["error"], "timeout");
    assert_eq!(evs[0]["extra"]["costo_estimado"], true);
}

#[test]
fn proveedores_deepseek_stream_completo_registra_tokens_del_evento_de_uso() {
    let srv = Servidor::arranca(vec![(200, sse_deepseek("hola", 7, 3, "stop"))]);
    let cfg = config(&srv.url, &srv.url);
    let (res, home) = llamada_con_servidor(&cfg, Proveedor::Deepseek, "deepseek-chat", "s4");

    let r = res.expect("el stream completo da respuesta");
    assert_eq!(r.texto, "hola");
    assert_eq!((r.in_tokens, r.out_tokens), (7, 3));
    assert!((r.cost_usd - (7.0 * 1.0 + 3.0 * 2.0) / 1e6).abs() < 1e-12);
    let evs = eventos(home.path());
    assert_eq!(
        (evs[0]["in_tokens"].as_u64(), evs[0]["out_tokens"].as_u64()),
        (Some(7), Some(3))
    );
    assert!(evs[0]["extra"].get("costo_estimado").is_none());
}

#[test]
fn proveedores_anthropic_stream_completo_registra_tokens_del_evento_de_uso() {
    let srv = Servidor::arranca(vec![(200, sse_anthropic(&["hola"], 7, 3, "end_turn"))]);
    let cfg = config(&srv.url, &srv.url);
    let (res, home) = llamada_con_servidor(&cfg, Proveedor::Anthropic, "claude-haiku-5-5", "s5");

    let r = res.expect("el stream completo da respuesta");
    assert_eq!(r.texto, "hola");
    assert_eq!((r.in_tokens, r.out_tokens), (7, 3));
    assert!((r.cost_usd - (7.0 * 4.0 + 3.0 * 8.0) / 1e6).abs() < 1e-12);
    let evs = eventos(home.path());
    assert_eq!(
        (evs[0]["in_tokens"].as_u64(), evs[0]["out_tokens"].as_u64()),
        (Some(7), Some(3))
    );
}

#[test]
fn proveedores_deepseek_stream_sin_evento_de_uso_registra_peor_caso() {
    let srv = Servidor::arranca(vec![(200, sse_deepseek_sin_uso("hola", "stop"))]);
    let cfg = config(&srv.url, &srv.url);
    let (res, home) = llamada_con_servidor(&cfg, Proveedor::Deepseek, "deepseek-chat", "s6");

    let err = res.expect_err("sin uso la llamada cuenta como cortada");
    assert!(matches!(err, ErrorProveedor::Cortada(_)), "llegó {err:?}");
    assert_eq!(srv.peticiones(), 1, "una cortada no se reintenta");
    let evs = eventos(home.path());
    assert_eq!(evs[0]["extra"]["costo_estimado"], true);
    let peor = costo_peor_caso(
        &cfg.precios["deepseek-chat"],
        tokens_entrada_estimados(&prompt()),
        64,
    );
    assert!((evs[0]["cost_usd"].as_f64().expect("costo") - peor).abs() < 1e-12);
}

#[test]
fn proveedores_anthropic_stream_sin_evento_de_uso_registra_peor_caso() {
    let srv = Servidor::arranca(vec![(200, sse_anthropic_sin_uso(&["hola"], 7, "end_turn"))]);
    let cfg = config(&srv.url, &srv.url);
    let (res, home) = llamada_con_servidor(&cfg, Proveedor::Anthropic, "claude-haiku-5-5", "s7");

    let err = res.expect_err("sin uso la llamada cuenta como cortada");
    assert!(matches!(err, ErrorProveedor::Cortada(_)), "llegó {err:?}");
    assert_eq!(srv.peticiones(), 1);
    let evs = eventos(home.path());
    assert_eq!(evs[0]["extra"]["costo_estimado"], true);
    let peor = costo_peor_caso(
        &cfg.precios["claude-haiku-5-5"],
        tokens_entrada_estimados(&prompt()),
        64,
    );
    assert!((evs[0]["cost_usd"].as_f64().expect("costo") - peor).abs() < 1e-12);
}

#[test]
fn proveedores_deepseek_pedido_pide_stream_e_include_usage() {
    let srv = Servidor::arranca(vec![(200, sse_deepseek("hola", 7, 3, "stop"))]);
    let cfg = config(&srv.url, &srv.url);
    let (res, _home) = llamada_con_servidor(&cfg, Proveedor::Deepseek, "deepseek-chat", "s8");
    res.expect("200 da respuesta");

    let cuerpo: Value = serde_json::from_str(&srv.pedido(0).cuerpo).expect("JSON");
    assert_eq!(cuerpo["stream"], true);
    assert_eq!(cuerpo["stream_options"]["include_usage"], true);
}

#[test]
fn proveedores_anthropic_pedido_pide_stream() {
    let srv = Servidor::arranca(vec![(200, sse_anthropic(&["hola"], 7, 3, "end_turn"))]);
    let cfg = config(&srv.url, &srv.url);
    let (res, _home) = llamada_con_servidor(&cfg, Proveedor::Anthropic, "claude-haiku-5-5", "s9");
    res.expect("200 da respuesta");

    let cuerpo: Value = serde_json::from_str(&srv.pedido(0).cuerpo).expect("JSON");
    assert_eq!(cuerpo["stream"], true);
    assert!(cuerpo.get("stream_options").is_none());
}
