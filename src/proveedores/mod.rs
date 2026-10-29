//! Proveedores: llamadas a DeepSeek y Anthropic con reintentos, breaker y un `Evento` por llamada.

pub mod anthropic;
pub mod deepseek;

use std::collections::BTreeMap;
use std::fmt;
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock, PoisonError};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use thiserror::Error as ErrorDerive;

use crate::config::{Config, Precio, root_home};
use crate::observe::{self, jsonl, uso};
use crate::tipos::{Eleccion, Evento, Proveedor, TipoEvento, Via};

/// Intentos totales por llamada: el primero más dos reintentos.
const INTENTOS_MAX: u32 = 3;
/// Fallas dentro de la ventana que abren el breaker.
const FALLAS_PARA_ABRIR: usize = 3;
const VENTANA_FALLAS: Duration = Duration::from_secs(60);
const ENFRIAMIENTO: Duration = Duration::from_secs(60);
const ARCHIVO_EVENTOS: &str = "eventos.jsonl";
const ARCHIVO_CONFIG: &str = "root.toml";

/// Tope de tokens de salida de los dos jueces (ficha y Oracle).
/// 32 000: valor sin confirmar en la documentación oficial de DeepSeek (no publica el default; terceros reportan un máximo de salida de 384 K para deepseek-flash y deepseek-v4-pro). Es un techo, no un gasto.
pub const MAX_TOKENS_JUEZ: u32 = 32_000;

/// Lo que se le pide a un modelo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Prompt {
    pub sistema: Option<String>,
    pub usuario: String,
    pub max_tokens: u32,
    /// Fase del flujo que hace la llamada. Va al `Evento`.
    pub fase: String,
}

/// Lo que devuelve un modelo, con su costo medido.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Respuesta {
    pub texto: String,
    pub in_tokens: u64,
    pub out_tokens: u64,
    pub cost_usd: f64,
    pub latency_s: f64,
    pub modelo: String,
    pub intentos: u32,
    /// `true` si el modelo se detuvo por el tope de salida. Es éxito con costo real.
    #[serde(default)]
    pub truncada: bool,
}

/// Lo que un juez recibe del modelo: el texto y si el tope de salida lo cortó.
#[derive(Debug, Clone, PartialEq)]
pub struct RespuestaJuez {
    pub texto: String,
    pub truncada: bool,
}

/// Causa de `sin_veredicto` cuando el modelo llegó al tope de salida. Nombra el tope pedido.
pub fn causa_tope_alcanzado(max_tokens: u32) -> String {
    format!("tope de salida alcanzado ({max_tokens} tokens)")
}

/// Errores de proveedores. Salen con código 3.
#[derive(Debug, ErrorDerive)]
pub enum ErrorProveedor {
    /// Falta la variable de entorno con la clave. Se nombra la variable, nunca su valor.
    #[error("falta la clave: define la variable {0}")]
    SinClave(String),
    #[error("{proveedor} respondió HTTP {estado}")]
    Http { estado: u16, proveedor: String },
    #[error("{proveedor} rechazó la clave: HTTP {estado}")]
    Auth { estado: u16, proveedor: String },
    #[error("{proveedor} rechazó el pedido: HTTP {estado}")]
    Pedido { estado: u16, proveedor: String },
    #[error("no se pudo registrar el evento: {causa}")]
    EventoNoRegistrado {
        respuesta: Box<Respuesta>,
        evento: Box<Evento>,
        causa: String,
    },
    /// El proveedor falló y, además, el evento de esa falla no se pudo escribir.
    /// El mensaje muestra primero el error original. Nunca lleva la clave.
    #[error("{original}; además no se pudo registrar el evento: {causa_registro}")]
    ErrorNoRegistrado {
        original: Box<ErrorProveedor>,
        causa_registro: String,
        evento: Box<Evento>,
    },
    #[error("error de red: {0}")]
    Red(String),
    /// Se agotó un plazo después de enviar el pedido. Puede haber costo; no se reintenta.
    #[error("tiempo agotado después de enviar el pedido: {0}")]
    Timeout(String),
    /// La conexión se cortó después de enviar el pedido. Puede haber costo; no se reintenta.
    #[error("conexión cortada después de enviar el pedido: {0}")]
    Cortada(String),
    #[error("breaker abierto para {0}")]
    BreakerAbierto(String),
    #[error("respuesta inesperada: {0}")]
    Respuesta(String),
    #[error("elección inválida: {0}")]
    Eleccion(String),
    #[error("{0}")]
    Config(String),
    /// Un tope impide la llamada antes de enviarla. El `Gate` ya quedó escrito, salvo que el
    /// gasto no se pudo leer. Sale con código 3.
    #[error("{0}")]
    Tope(String),
}

impl ErrorProveedor {
    /// Clase corta del error. Va a `extra.error` del `Evento`. Nunca lleva cuerpos ni cabeceras.
    fn clase(&self) -> String {
        let corta = match self {
            Self::Http { estado, .. } => return format!("http_{estado}"),
            Self::Auth { estado, .. } => return format!("auth_{estado}"),
            Self::Pedido { estado, .. } => return format!("pedido_{estado}"),
            Self::EventoNoRegistrado { .. } => "evento",
            Self::Red(_) => "red",
            Self::Timeout(_) => "timeout",
            Self::Cortada(_) => "red_cortada",
            Self::Respuesta(_) => "respuesta",
            Self::BreakerAbierto(_) => "breaker",
            Self::SinClave(_) => "sin_clave",
            Self::Eleccion(_) => "eleccion",
            Self::Config(_) => "config",
            Self::Tope(_) => "tope",
            Self::ErrorNoRegistrado { original, .. } => return original.clase(),
        };
        corta.into()
    }

    /// Errores que un reintento puede arreglar: fallas antes de enviar el pedido, 429 y 5xx. Son
    /// también las fallas del breaker: cuando llegan aquí, los reintentos ya se agotaron.
    fn es_transitorio(&self) -> bool {
        match self {
            Self::Red(_) => true,
            Self::Http { estado, .. } => *estado == 429 || (500..=599).contains(estado),
            _ => false,
        }
    }

    /// Fallas que el breaker cuenta: las transitorias y el timeout después de enviar.
    fn cuenta_para_breaker(&self) -> bool {
        self.es_transitorio() || matches!(self, Self::Timeout(_))
    }

    /// Fallas que cuestan el peor caso del intento: el proveedor pudo procesar el pedido. Son los
    /// 5xx, las fallas después de enviar y un 2xx que no se puede leer. Las de antes de enviar, el
    /// 429 y los otros 4xx cuestan cero.
    fn cobra_peor_caso(&self) -> bool {
        match self {
            Self::Http { estado, .. } => (500..=599).contains(estado),
            Self::Timeout(_) | Self::Cortada(_) | Self::Respuesta(_) => true,
            _ => false,
        }
    }
}

/// Clave de API. Su `Debug` no muestra el valor.
#[derive(Clone)]
pub struct Clave(String);

impl Clave {
    pub fn nueva(valor: impl Into<String>) -> Clave {
        Clave(valor.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Clave {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Clave(***)")
    }
}

/// Claves de cada proveedor. Nunca se guardan en archivos ni en eventos.
#[derive(Debug, Clone, Default)]
pub struct Claves {
    pub deepseek: Option<Clave>,
    pub anthropic: Option<Clave>,
}

impl Claves {
    /// Lee `DEEPSEEK_API_KEY` y `ANTHROPIC_API_KEY`. Una variable vacía cuenta como ausente.
    pub fn del_entorno() -> Claves {
        Claves {
            deepseek: leer_clave(variable(Proveedor::Deepseek)),
            anthropic: leer_clave(variable(Proveedor::Anthropic)),
        }
    }

    fn para(&self, proveedor: Proveedor) -> Option<&Clave> {
        match proveedor {
            Proveedor::Deepseek => self.deepseek.as_ref(),
            Proveedor::Anthropic => self.anthropic.as_ref(),
        }
    }
}

fn leer_clave(nombre: &str) -> Option<Clave> {
    std::env::var(nombre)
        .ok()
        .filter(|valor| !valor.is_empty())
        .map(Clave)
}

/// Variable de entorno que guarda la clave de cada proveedor.
fn variable(proveedor: Proveedor) -> &'static str {
    match proveedor {
        Proveedor::Deepseek => "DEEPSEEK_API_KEY",
        Proveedor::Anthropic => "ANTHROPIC_API_KEY",
    }
}

fn familia(proveedor: Proveedor) -> &'static str {
    match proveedor {
        Proveedor::Deepseek => "deepseek",
        Proveedor::Anthropic => "anthropic",
    }
}

/// Estado de una familia de proveedor. Las fallas guardan el instante en que ocurrieron.
#[derive(Debug, Default)]
struct EstadoBreaker {
    fallas: Vec<Instant>,
    abierto_hasta: Option<Instant>,
}

/// Breaker por familia: 3 fallas en 60 s lo abren 60 s desde la última falla.
#[derive(Debug, Default)]
pub struct Breaker {
    estados: Mutex<BTreeMap<String, EstadoBreaker>>,
}

impl Breaker {
    pub fn new() -> Breaker {
        Breaker::default()
    }

    fn con_estado<T>(&self, familia: &str, f: impl FnOnce(&mut EstadoBreaker) -> T) -> T {
        let mut mapa = self.estados.lock().unwrap_or_else(PoisonError::into_inner);
        f(mapa.entry(familia.to_string()).or_default())
    }

    /// `true` si la familia puede recibir una llamada en `ahora`.
    pub fn permite(&self, familia: &str, ahora: Instant) -> bool {
        self.con_estado(familia, |e| {
            e.abierto_hasta.is_none_or(|hasta| ahora >= hasta)
        })
    }

    /// Anota una falla. Con 3 dentro de 60 s, el breaker se abre hasta `ahora + 60 s`.
    pub fn registra_falla(&self, familia: &str, ahora: Instant) {
        self.con_estado(familia, |e| {
            e.fallas
                .retain(|t| ahora.saturating_duration_since(*t) < VENTANA_FALLAS);
            e.fallas.push(ahora);
            if e.fallas.len() >= FALLAS_PARA_ABRIR {
                e.abierto_hasta = Some(ahora + ENFRIAMIENTO);
            }
        });
    }

    /// Un éxito reinicia el contador y cierra el breaker.
    pub fn registra_exito(&self, familia: &str) {
        self.con_estado(familia, |e| *e = EstadoBreaker::default());
    }
}

/// Breaker del proceso. Lo usa `llamar`.
pub fn breaker_global() -> &'static Breaker {
    static BREAKER: OnceLock<Breaker> = OnceLock::new();
    BREAKER.get_or_init(Breaker::new)
}

/// Todo lo que `llamar_con` necesita. Los tests lo arman con un reloj y claves propios.
pub struct Contexto<'a> {
    pub cfg: &'a Config,
    /// Carpeta donde se escribe `eventos.jsonl`.
    pub home: &'a Path,
    pub breaker: &'a Breaker,
    pub claves: &'a Claves,
    pub reloj: &'a dyn Fn() -> Instant,
}

/// Pedido HTTP ya armado, sin enviar.
pub(crate) struct Peticion {
    pub url: String,
    pub cabeceras: Vec<(&'static str, String)>,
    pub cuerpo: String,
}

/// Lo que el proveedor dijo en un intento exitoso, antes de calcular el costo.
#[derive(Debug, PartialEq)]
pub(crate) struct Crudo {
    pub texto: String,
    pub in_tokens: u64,
    pub out_tokens: u64,
    /// `true` si el proveedor cortó la respuesta por el tope de salida.
    pub truncada: bool,
}

/// Campo de texto de una respuesta JSON, por puntero. Ausente es `ErrorProveedor::Respuesta`.
pub(crate) fn texto_en(json: &Value, ruta: &str) -> Result<String, ErrorProveedor> {
    json.pointer(ruta)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ErrorProveedor::Respuesta(format!("falta {ruta}")))
}

/// Campo entero de una respuesta JSON, por puntero. Ausente es `ErrorProveedor::Respuesta`.
pub(crate) fn entero_en(json: &Value, ruta: &str) -> Result<u64, ErrorProveedor> {
    json.pointer(ruta)
        .and_then(Value::as_u64)
        .ok_or_else(|| ErrorProveedor::Respuesta(format!("falta {ruta}")))
}

/// Llama al proveedor de `e` con la configuración de `root_home()/root.toml`. Chequea el tope.
pub fn llamar(e: &Eleccion, prompt: &Prompt, corrida: &str) -> Result<Respuesta, ErrorProveedor> {
    let home = root_home();
    let cfg = Config::cargar(&home.join(ARCHIVO_CONFIG))
        .map_err(|err| ErrorProveedor::Config(err.to_string()))?;
    let claves = Claves::del_entorno();
    let reloj = Instant::now;
    let ctx = Contexto {
        cfg: &cfg,
        home: &home,
        breaker: breaker_global(),
        claves: &claves,
        reloj: &reloj,
    };
    llamar_con(&ctx, e, prompt, corrida)
}

/// Una llamada en curso: lo que cada intento necesita y lo que lleva gastado. Cada intento que
/// llega a enviarse tiene su reserva, y el evento final de la llamada las cierra todas.
struct Llamada<'a> {
    proveedor: Proveedor,
    modelo: &'a str,
    prompt: &'a Prompt,
    corrida: &'a str,
    precio: Precio,
    /// Peor caso de un intento: la entrada estimada y todo `max_tokens`, al precio del modelo.
    peor_caso: f64,
    /// Intentos que llegaron a reservarse, o sea, que se enviaron.
    intentos: u32,
    reservas: Vec<String>,
    /// Costo acumulado de los intentos: real si respondieron, peor caso si costaron sin respuesta.
    cost_usd: f64,
    /// `true` si algún intento cobró el peor caso.
    costo_estimado: bool,
}

/// Llamada con todas las dependencias explícitas. Registra un `Evento` por llamada.
/// Las elecciones inválidas fallan antes de llamar y no registran evento. Un modelo sin precio o
/// un tope que rechaza el primer intento no envían nada: no hay evento `Llamada`, solo un `Gate`.
pub fn llamar_con(
    ctx: &Contexto,
    e: &Eleccion,
    prompt: &Prompt,
    corrida: &str,
) -> Result<Respuesta, ErrorProveedor> {
    let (proveedor, modelo) = elegida(e)?;
    let precio = precio_de(ctx, modelo, corrida)?;
    let familia = familia(proveedor);
    let mut llamada = Llamada {
        proveedor,
        modelo,
        prompt,
        corrida,
        peor_caso: costo_peor_caso(&precio, tokens_entrada_estimados(prompt), prompt.max_tokens),
        precio,
        intentos: 0,
        reservas: Vec::new(),
        cost_usd: 0.0,
        costo_estimado: false,
    };
    let inicio = Instant::now();
    let resultado = ejecutar(ctx, &mut llamada);
    let latency_s = inicio.elapsed().as_secs_f64();
    let mut ev = evento(corrida, prompt, e, familia, latency_s);
    match resultado {
        Ok(crudo) => {
            ctx.breaker.registra_exito(familia);
            llamada.cost_usd += costo_usd(&llamada.precio, crudo.in_tokens, crudo.out_tokens);
            ev.in_tokens = crudo.in_tokens;
            ev.out_tokens = crudo.out_tokens;
            if crudo.truncada {
                ev.extra.insert("truncada".into(), json!(true));
            }
            cerrar(&mut ev, &llamada);
            let respuesta = Respuesta {
                texto: crudo.texto,
                in_tokens: crudo.in_tokens,
                out_tokens: crudo.out_tokens,
                cost_usd: llamada.cost_usd,
                latency_s,
                modelo: modelo.to_string(),
                intentos: llamada.intentos,
                truncada: crudo.truncada,
            };
            match registra(ctx.home, &ev) {
                Ok(()) => Ok(respuesta),
                Err(causa) => Err(ErrorProveedor::EventoNoRegistrado {
                    respuesta: Box::new(respuesta),
                    evento: Box::new(ev),
                    causa: causa.to_string(),
                }),
            }
        }
        Err(err) => {
            if err.cuenta_para_breaker() {
                ctx.breaker.registra_falla(familia, (ctx.reloj)());
            }
            // Un tope que rechaza el primer intento no reservó nada: solo queda el `Gate`.
            if matches!(err, ErrorProveedor::Tope(_)) && llamada.reservas.is_empty() {
                return Err(err);
            }
            cerrar(&mut ev, &llamada);
            ev.extra.insert("error".into(), json!(err.clase()));
            match registra(ctx.home, &ev) {
                Ok(()) => Err(err),
                Err(causa) => Err(ErrorProveedor::ErrorNoRegistrado {
                    original: Box::new(err),
                    causa_registro: causa.to_string(),
                    evento: Box::new(ev),
                }),
            }
        }
    }
}

/// Completa el `Evento` de la llamada con sus intentos, las reservas que cierra y el costo.
fn cerrar(ev: &mut Evento, llamada: &Llamada) {
    ev.cost_usd = llamada.cost_usd;
    ev.extra.insert("intentos".into(), json!(llamada.intentos));
    ev.extra.insert("reservas".into(), json!(llamada.reservas));
    if llamada.costo_estimado {
        ev.extra.insert("costo_estimado".into(), json!(true));
    }
}

/// Precio del modelo. Sin precio no hay estimación: la llamada se rechaza antes de todo, con un
/// `Gate` y sin leer el gasto.
fn precio_de(ctx: &Contexto, modelo: &str, corrida: &str) -> Result<Precio, ErrorProveedor> {
    let Some(precio) = ctx.cfg.precios.get(modelo) else {
        let mensaje = format!(
            "sin precio en root.toml para {modelo}: no se puede estimar el costo, la llamada no se envía"
        );
        return Err(rechazo_tope(ctx, corrida, modelo, None, None, mensaje));
    };
    Ok(precio.clone())
}

/// Decisión del chequeo de tope de un intento.
enum Decision {
    Reservada,
    Rechazada { margen: f64 },
}

/// Chequeo de tope de un intento y su reserva, en un solo tramo con el lock de `eventos.jsonl`:
/// lee el gasto, compara el peor caso con el margen y, si entra, escribe la reserva. Un rechazo
/// registra un `Gate` y devuelve `Tope`. Si el log no se puede leer ni escribir, también rechaza:
/// sin gasto conocido no hay margen que probar.
fn reservar(ctx: &Contexto, llamada: &mut Llamada) -> Result<(), ErrorProveedor> {
    let tope = ctx.cfg.balance.api.tope_usd;
    let offset = uso::offset_local();
    let bloque = ctx.cfg.balance.cupo.bloque_horas;
    let id = nueva_reserva_id(llamada.corrida);
    let ruta = ctx.home.join(ARCHIVO_EVENTOS);
    let decision = uso::con_eventos_bloqueados(&ruta, |eventos, log| {
        // El reloj se lee con el lock tomado: una reserva que otro proceso escribió mientras este
        // esperaba tiene un `ts` posterior, y `calcular_uso` ignora los eventos del futuro.
        let (ahora, _) = observe::ahora();
        let margen = tope - uso::calcular_uso(eventos, ahora, offset, bloque).api_mes_usd;
        if llamada.peor_caso > margen {
            return Ok(Decision::Rechazada { margen });
        }
        let linea =
            serde_json::to_string(&evento_reserva(llamada, &id)).map_err(io::Error::other)?;
        log.agregar(&linea, jsonl::UMBRAL_POR_DEFECTO)?;
        Ok(Decision::Reservada)
    })
    .map_err(|e| {
        ErrorProveedor::Tope(format!(
            "no se pudo leer o reservar el gasto de API: {e}; la llamada no se envía"
        ))
    })?;
    match decision {
        Decision::Reservada => {
            llamada.reservas.push(id);
            llamada.intentos += 1;
            Ok(())
        }
        Decision::Rechazada { margen } => {
            let estimado = llamada.peor_caso;
            let mensaje = format!(
                "llamada rechazada antes de enviarla: costo estimado {estimado:.6} USD, margen {margen:.6} USD del tope de API de {tope:.2} USD"
            );
            Err(rechazo_tope(
                ctx,
                llamada.corrida,
                llamada.modelo,
                Some(margen),
                Some(estimado),
                mensaje,
            ))
        }
    }
}

/// Contador de reservas del proceso. Con el pid y el reloj, el id es único entre procesos.
static CONTADOR_RESERVAS: AtomicU64 = AtomicU64::new(0);

/// Id de una reserva: corrida, pid, nanosegundos y un contador del proceso.
fn nueva_reserva_id(corrida: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let n = CONTADOR_RESERVAS.fetch_add(1, Ordering::Relaxed);
    format!("{corrida}-{}-{nanos}-{n}", std::process::id())
}

/// Evento `Reserva` de un intento. Su peor caso cuenta como gasto hasta que la llamada lo cierre.
fn evento_reserva(llamada: &Llamada, id: &str) -> Evento {
    let mut ev = observe::evento_manual(llamada.corrida, TipoEvento::Reserva, None);
    ev.fase = llamada.prompt.fase.clone();
    ev.via = Some(Via::Api);
    ev.modelo = Some(llamada.modelo.into());
    ev.familia = Some(familia(llamada.proveedor).into());
    ev.cost_usd = llamada.peor_caso;
    ev.extra.insert("reserva".into(), json!(id));
    ev
}

/// Registra el `Gate` del rechazo y devuelve `Tope`. Si el registro falla, el mensaje lo dice.
fn rechazo_tope(
    ctx: &Contexto,
    corrida: &str,
    modelo: &str,
    margen: Option<f64>,
    estimado: Option<f64>,
    mensaje: String,
) -> ErrorProveedor {
    let extra = Map::from_iter([
        ("estimado".to_string(), json!(estimado)),
        ("margen".to_string(), json!(margen)),
        ("modelo".to_string(), json!(modelo)),
    ]);
    let gate = observe::evento_gate(corrida, "tope", extra);
    match registra(ctx.home, &gate) {
        Ok(()) => ErrorProveedor::Tope(mensaje),
        Err(causa) => ErrorProveedor::Tope(format!(
            "{mensaje}; además no se pudo registrar el Gate: {causa}"
        )),
    }
}

/// Tokens de entrada estimados de un pedido: sus bytes UTF-8, sistema y usuario. Un token nunca
/// ocupa menos de un byte, así que es una cota superior de los tokens reales.
pub fn tokens_entrada_estimados(prompt: &Prompt) -> u64 {
    let sistema = prompt.sistema.as_deref().map_or(0, str::len);
    (sistema + prompt.usuario.len()) as u64
}

/// Peor caso en USD de un pedido: toda la entrada estimada y todo el máximo de salida, al precio.
pub fn costo_peor_caso(precio: &Precio, in_est: u64, max_tokens: u32) -> f64 {
    costo_usd(precio, in_est, u64::from(max_tokens))
}

/// Proveedor y modelo de una elección que puede ejecutarse por API.
fn elegida(e: &Eleccion) -> Result<(Proveedor, &str), ErrorProveedor> {
    if e.via != Via::Api {
        return Err(ErrorProveedor::Eleccion(format!(
            "la vía es {:?}, no api",
            e.via
        )));
    }
    let proveedor = e
        .proveedor
        .ok_or_else(|| ErrorProveedor::Eleccion("falta el proveedor".into()))?;
    let modelo = e
        .modelo
        .as_deref()
        .ok_or_else(|| ErrorProveedor::Eleccion("falta el modelo".into()))?;
    Ok((proveedor, modelo))
}

/// Resultado del primer intento que responde o del último que falla. Sin clave o con breaker
/// abierto, no hay intento y no se reserva nada.
fn ejecutar(ctx: &Contexto, llamada: &mut Llamada) -> Result<Crudo, ErrorProveedor> {
    let Some(clave) = ctx.claves.para(llamada.proveedor) else {
        return Err(ErrorProveedor::SinClave(variable(llamada.proveedor).into()));
    };
    if !ctx
        .breaker
        .permite(familia(llamada.proveedor), (ctx.reloj)())
    {
        return Err(ErrorProveedor::BreakerAbierto(
            familia(llamada.proveedor).into(),
        ));
    }
    reintentar(ctx, clave, llamada)
}

/// Hasta `INTENTOS_MAX` intentos. Antes de cada uno, el chequeo de tope con su reserva: un reintento
/// que no entra en el margen no se envía. Espera la base y la duplica entre intentos transitorios.
fn reintentar(
    ctx: &Contexto,
    clave: &Clave,
    llamada: &mut Llamada,
) -> Result<Crudo, ErrorProveedor> {
    let agente = agente(ctx.cfg.proveedores.timeout_s);
    let mut espera = Duration::from_millis(ctx.cfg.proveedores.espera_base_ms);
    loop {
        reservar(ctx, llamada)?;
        let resultado = un_intento(
            &agente,
            ctx.cfg,
            llamada.proveedor,
            clave,
            llamada.modelo,
            llamada.prompt,
        );
        match resultado {
            Ok(crudo) => return Ok(crudo),
            Err(err) => {
                if err.cobra_peor_caso() {
                    llamada.cost_usd += llamada.peor_caso;
                    llamada.costo_estimado = true;
                }
                if !err.es_transitorio() || llamada.intentos >= INTENTOS_MAX {
                    return Err(err);
                }
                thread::sleep(espera);
                espera *= 2;
            }
        }
    }
}

/// Cliente HTTP. Un 4xx o 5xx llega como respuesta, no como error, para clasificar el estado.
fn agente(timeout_s: u64) -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_per_call(Some(Duration::from_secs(timeout_s)))
        .build();
    ureq::Agent::new_with_config(config)
}

/// Un intento HTTP: envía, revisa el estado y lee el cuerpo con el parser del proveedor.
fn un_intento(
    agente: &ureq::Agent,
    cfg: &Config,
    proveedor: Proveedor,
    clave: &Clave,
    modelo: &str,
    prompt: &Prompt,
) -> Result<Crudo, ErrorProveedor> {
    let pet = match proveedor {
        Proveedor::Deepseek => {
            deepseek::armar(&cfg.proveedores.deepseek_url, clave, modelo, prompt)
        }
        Proveedor::Anthropic => {
            anthropic::armar(&cfg.proveedores.anthropic_url, clave, modelo, prompt)
        }
    };
    let mut pedido = agente.post(&pet.url);
    for (nombre, valor) in &pet.cabeceras {
        pedido = pedido.header(*nombre, valor.as_str());
    }
    let mut respuesta = pedido.send(pet.cuerpo.as_str()).map_err(error_al_enviar)?;
    let estado = respuesta.status().as_u16();
    if !(200..300).contains(&estado) {
        let proveedor = familia(proveedor).into();
        return Err(error_de_estado(estado, proveedor));
    }
    let texto = respuesta
        .body_mut()
        .read_to_string()
        .map_err(error_despues_de_enviar)?;
    match proveedor {
        Proveedor::Deepseek => deepseek::leer(&texto),
        Proveedor::Anthropic => anthropic::leer(&texto),
    }
}

/// Falla al enviar el pedido. La resolución, el timeout de conexión, la conexión rechazada o
/// fallida y el host no encontrado pasan antes de enviar: no cuestan y se reintentan. Lo demás
/// puede haber costado: se trata como falla después de enviar.
fn error_al_enviar(err: ureq::Error) -> ErrorProveedor {
    let antes_de_enviar = match &err {
        ureq::Error::Timeout(plazo) => {
            matches!(plazo, ureq::Timeout::Resolve | ureq::Timeout::Connect)
        }
        ureq::Error::ConnectionFailed | ureq::Error::HostNotFound => true,
        ureq::Error::Io(e) => e.kind() == io::ErrorKind::ConnectionRefused,
        _ => false,
    };
    if antes_de_enviar {
        ErrorProveedor::Red(err.to_string())
    } else {
        error_despues_de_enviar(err)
    }
}

/// Falla después de enviar el pedido. Un plazo agotado es `Timeout`; lo demás, `Cortada`.
fn error_despues_de_enviar(err: ureq::Error) -> ErrorProveedor {
    if matches!(err, ureq::Error::Timeout(_)) {
        ErrorProveedor::Timeout(err.to_string())
    } else {
        ErrorProveedor::Cortada(err.to_string())
    }
}

/// 401 y 403 son `Auth`. Los otros 4xx salvo 429 son `Pedido`. 429, 5xx y lo demás son `Http`.
fn error_de_estado(estado: u16, proveedor: String) -> ErrorProveedor {
    match estado {
        401 | 403 => ErrorProveedor::Auth { estado, proveedor },
        400..=499 if estado != 429 => ErrorProveedor::Pedido { estado, proveedor },
        _ => ErrorProveedor::Http { estado, proveedor },
    }
}

/// Costo en USD de unos tokens de entrada y salida, al precio de un millón de tokens.
fn costo_usd(precio: &Precio, in_tokens: u64, out_tokens: u64) -> f64 {
    in_tokens as f64 * precio.usd_entrada / 1e6 + out_tokens as f64 * precio.usd_salida / 1e6
}

/// Evento de la llamada, sin tokens, costo ni intentos. Quien llama completa esos campos.
fn evento(corrida: &str, prompt: &Prompt, e: &Eleccion, familia: &str, latency_s: f64) -> Evento {
    let mut ev = observe::evento_manual(corrida, TipoEvento::Llamada, None);
    ev.fase = prompt.fase.clone();
    ev.via = Some(Via::Api);
    ev.modelo = e.modelo.clone();
    ev.familia = Some(familia.into());
    ev.latency_s = latency_s;
    ev.extra
        .insert("razonamiento".into(), json!(e.razonamiento));
    ev
}

fn registra(home: &Path, ev: &Evento) -> std::io::Result<()> {
    observe::registrar_en(&home.join(ARCHIVO_EVENTOS), ev)
}

/// Contrato de `r00t llama`. Éxito: la respuesta en `out` y 0.
/// Evento no registrado: la respuesta en `out`, la línea JSON del evento en `err` y 3.
/// Error del proveedor con evento no registrado: su mensaje y la línea JSON del evento en `err`, y 3.
/// Otro error: su mensaje en `err` y 3. Ningún mensaje lleva la clave.
pub fn imprimir_para_cli(
    res: Result<Respuesta, ErrorProveedor>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    match res {
        Ok(respuesta) => {
            escribe_linea(out, &respuesta);
            0
        }
        Err(ErrorProveedor::EventoNoRegistrado {
            respuesta, evento, ..
        }) => {
            escribe_linea(out, &respuesta);
            escribe_linea(err, &evento);
            3
        }
        Err(otro) => {
            let _ = writeln!(err, "{otro}");
            if let ErrorProveedor::ErrorNoRegistrado { evento, .. } = &otro {
                escribe_linea(err, &**evento);
            }
            3
        }
    }
}

/// Escribe una línea JSON. Los tipos de `Respuesta` y `Evento` no fallan al serializarse.
fn escribe_linea<T: Serialize>(dest: &mut dyn Write, valor: &T) {
    if let Ok(linea) = serde_json::to_string(valor) {
        let _ = writeln!(dest, "{linea}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: Duration = Duration::from_secs(60);

    #[test]
    fn proveedores_breaker_dos_fallas_no_abren() {
        let b = Breaker::new();
        let t0 = Instant::now();
        b.registra_falla("deepseek", t0);
        b.registra_falla("deepseek", t0 + Duration::from_secs(1));
        assert!(b.permite("deepseek", t0 + Duration::from_secs(2)));
    }

    #[test]
    fn proveedores_breaker_tres_fallas_en_ventana_abren_y_vencen_a_los_60_s() {
        let b = Breaker::new();
        let t0 = Instant::now();
        b.registra_falla("deepseek", t0);
        b.registra_falla("deepseek", t0 + Duration::from_secs(10));
        b.registra_falla("deepseek", t0 + Duration::from_secs(20));
        assert!(!b.permite("deepseek", t0 + Duration::from_secs(30)));
        // Cuenta desde la última falla (t0 + 20 s), no desde la primera.
        let justo_antes = t0 + Duration::from_secs(20) + MIN - Duration::from_millis(1);
        assert!(!b.permite("deepseek", justo_antes));
        assert!(b.permite("deepseek", t0 + Duration::from_secs(20) + MIN));
    }

    #[test]
    fn proveedores_breaker_fallas_fuera_de_la_ventana_no_suman() {
        let b = Breaker::new();
        let t0 = Instant::now();
        b.registra_falla("deepseek", t0);
        b.registra_falla("deepseek", t0 + Duration::from_secs(30));
        b.registra_falla("deepseek", t0 + Duration::from_secs(61));
        // La primera cayó fuera de los 60 s de la tercera: solo hay dos en ventana.
        assert!(b.permite("deepseek", t0 + Duration::from_secs(62)));
    }

    #[test]
    fn proveedores_breaker_es_por_familia() {
        let b = Breaker::new();
        let t0 = Instant::now();
        for _ in 0..3 {
            b.registra_falla("deepseek", t0);
        }
        assert!(!b.permite("deepseek", t0));
        assert!(b.permite("anthropic", t0));
    }

    #[test]
    fn proveedores_breaker_exito_reinicia_el_contador() {
        let b = Breaker::new();
        let t0 = Instant::now();
        b.registra_falla("anthropic", t0);
        b.registra_falla("anthropic", t0);
        b.registra_exito("anthropic");
        b.registra_falla("anthropic", t0);
        assert!(b.permite("anthropic", t0));
    }

    #[test]
    fn proveedores_clave_debug_no_muestra_el_valor() {
        let clave = Clave::nueva("sk-test-SECRETO-123");
        assert!(!format!("{clave:?}").contains("SECRETO"));
        let claves = Claves {
            deepseek: Some(clave),
            anthropic: None,
        };
        assert!(!format!("{claves:?}").contains("SECRETO"));
    }
}
