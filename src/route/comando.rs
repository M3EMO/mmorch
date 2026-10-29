//! Glue de `r00t route` y `r00t llama`: lee `root.toml` y `politica.json`, llama al núcleo puro
//! `elegir` y registra un `Gate` en cada rechazo de presupuesto. El tope de API lo chequea
//! `proveedores::llamar`, antes de enviar.

use std::fs;
use std::io;
use std::path::Path;
use std::time::Instant;

use serde_json::{Map, json};

use crate::config::{Config, root_home};
use crate::error::Error;
use crate::observe::{self, uso};
use crate::proveedores::{self, Claves, ErrorProveedor, Prompt, Respuesta};
use crate::route::politica::nombre_rol;
use crate::route::{self, Politica};
use crate::tipos::{Eleccion, Pedido, Proveedor, Rol, SinPresupuesto, Uso, Via};

/// Corrida de los `Gate` que escribe `r00t route`.
const CORRIDA_ROUTE: &str = "route";

/// Lee `politica.json`. Si no existe, vale el default. Si no se puede leer o parsear, es error.
pub fn cargar_politica(ruta: &Path) -> Result<Politica, Error> {
    match fs::read_to_string(ruta) {
        Ok(texto) => Ok(serde_json::from_str(&texto)?),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Politica::default()),
        Err(fuente) => Err(Error::ConfigLeer {
            ruta: ruta.display().to_string(),
            fuente,
        }),
    }
}

/// `r00t route`: imprime la elección. Con `SinPresupuesto` registra un `Gate` y sale con 3.
pub fn route(json: bool, rol: Rol, tipo: &str, checkeable: bool, tokens: u32) -> Result<(), Error> {
    let eleccion = rutear(rol, tipo, checkeable, tokens)?;
    imprimir_eleccion(json, &eleccion)
}

/// Elige la vía y el modelo de un pedido con el `ROOT_HOME` del proceso.
/// Con `SinPresupuesto` registra el `Gate`.
/// La usan `r00t route` y la herramienta MCP `root_route`.
pub fn rutear(rol: Rol, tipo: &str, checkeable: bool, tokens: u32) -> Result<Eleccion, Error> {
    rutear_en(&root_home(), rol, tipo, checkeable, tokens)
}

/// Como `rutear`, con `root.toml`, `politica.json` y `eventos.jsonl` de `home`. El `Gate` va a `home`.
pub fn rutear_en(
    home: &Path,
    rol: Rol,
    tipo: &str,
    checkeable: bool,
    tokens: u32,
) -> Result<Eleccion, Error> {
    let cfg = Config::cargar(&home.join("root.toml"))?;
    let pol = cargar_politica(&home.join("politica.json"))?;
    let ahora = observe::ahora().0;
    let offset = uso::offset_local();
    let gasto = uso_actual(home, &cfg, ahora, offset)?;
    let pedido = Pedido {
        rol,
        tipo: tipo.into(),
        checkeable,
        tokens_estimados: tokens,
    };
    elegir_final(
        home,
        &pedido,
        &pol,
        &gasto,
        &cfg,
        ahora,
        offset,
        CORRIDA_ROUTE,
    )
}

/// `r00t llama`: lee el prompt, rutea, chequea el costo y llama. Devuelve el código de `imprimir_para_cli`.
pub fn llama(
    rol: Rol,
    tipo: &str,
    prompt_file: &Path,
    corrida: &str,
    max_tokens: u32,
) -> Result<u8, Error> {
    let usuario = fs::read_to_string(prompt_file).map_err(|e| {
        Error::Uso(format!(
            "no se pudo leer --prompt-file {}: {e}",
            prompt_file.display()
        ))
    })?;
    let resultado = llamar_con_chequeo(rol, tipo, usuario, corrida, max_tokens);
    let mut out = io::stdout().lock();
    let mut err = io::stderr().lock();
    match resultado {
        Err(Error::Proveedor(e)) => Ok(proveedores::imprimir_para_cli(Err(e), &mut out, &mut err)),
        Err(otro) => Err(otro),
        Ok(respuesta) => Ok(proveedores::imprimir_para_cli(
            Ok(respuesta),
            &mut out,
            &mut err,
        )),
    }
}

/// Rutea y llama. Sin vía API, tope o precio, no hay llamada: sale `Err`.
/// Con el tope de API lleno, un rol de API que cae al cupo sale como tope (B9).
/// Los errores del proveedor llegan como `Error::Proveedor`, y un tope como `Error::Tope`.
/// La usan `r00t llama` y `root_llama`.
pub fn llamar_con_chequeo(
    rol: Rol,
    tipo: &str,
    usuario: String,
    corrida: &str,
    max_tokens: u32,
) -> Result<Respuesta, Error> {
    let home = root_home();
    let cfg = Config::cargar(&home.join("root.toml"))?;
    let pol = cargar_politica(&home.join("politica.json"))?;
    let ahora = observe::ahora().0;
    let offset = uso::offset_local();
    let gasto = uso_actual(&home, &cfg, ahora, offset)?;
    let pedido = Pedido {
        rol,
        tipo: tipo.into(),
        checkeable: false,
        tokens_estimados: max_tokens,
    };
    let eleccion = elegir_final(&home, &pedido, &pol, &gasto, &cfg, ahora, offset, corrida)?;
    if eleccion.via == Via::Cupo {
        if let Some(error) = rechazo_api_llena(&home, &pedido, &gasto, &cfg, ahora, offset, corrida)
        {
            return Err(error);
        }
        return Err(Error::Uso(
            "este rol va por cupo: lo corre `claude -p`, no `r00t llama`".into(),
        ));
    }
    let prompt = Prompt {
        sistema: None,
        usuario,
        max_tokens,
        fase: "llama".into(),
    };
    proveedores::llamar_en(&home, &eleccion, &prompt, corrida).map_err(de_proveedor)
}

/// Con el tope de API del mes lleno, un rol que va por API (delega, verifica, juzga) no tiene
/// otra vía en `r00t llama`: registra un `Gate` de presupuesto y da el error de tope (B9).
/// Los demás roles y los topes con margen devuelven `None`.
fn rechazo_api_llena(
    home: &Path,
    pedido: &Pedido,
    gasto: &Uso,
    cfg: &Config,
    ahora: f64,
    offset: i32,
    corrida: &str,
) -> Option<Error> {
    let tope = cfg.balance.api.tope_usd;
    let api_llena = gasto.api_mes_usd >= tope;
    if !api_llena || !matches!(pedido.rol, Rol::Delega | Rol::Verifica | Rol::Juzga) {
        return None;
    }
    let reabre = uso::iso(route::proximo_mes(ahora, offset), offset);
    let usado = gasto.api_mes_usd;
    let extra = Map::from_iter([
        ("motivo".to_string(), json!("tope de API del mes lleno")),
        ("usado".to_string(), json!(usado)),
        ("tope_usd".to_string(), json!(tope)),
        ("reabre".to_string(), json!(reabre)),
        ("rol".to_string(), json!(nombre_rol(pedido.rol))),
        ("tipo".to_string(), json!(pedido.tipo)),
    ]);
    let mensaje = format!(
        "sin presupuesto: el tope de API del mes está lleno ({usado:.2} de {tope:.2} USD); reabre {reabre}"
    );
    Some(gate_y_error(home, corrida, "presupuesto", extra, mensaje))
}

/// Un tope sale como `Error::Tope`, con el mismo código 3 que el chequeo que vivía aquí.
fn de_proveedor(e: ErrorProveedor) -> Error {
    match e {
        ErrorProveedor::Tope(mensaje) => Error::Tope(mensaje),
        otro => Error::Proveedor(otro),
    }
}

/// Gasto actual de `eventos.jsonl` de `home`. Un log ilegible es error: sin gasto no se puede decidir.
fn uso_actual(home: &Path, cfg: &Config, ahora: f64, offset: i32) -> Result<Uso, Error> {
    Ok(uso::uso_en(
        &observe::ruta_eventos_en(home),
        ahora,
        offset,
        cfg.balance.cupo.bloque_horas,
    )?)
}

/// Elige la vía y aplica el respaldo de Verifica y Juzga si el proveedor principal no está disponible.
/// Un modelo fuera de la lista de razonamiento da error de elección (código 2).
#[allow(clippy::too_many_arguments)]
fn elegir_final(
    home: &Path,
    pedido: &Pedido,
    pol: &Politica,
    gasto: &Uso,
    cfg: &Config,
    ahora: f64,
    offset: i32,
    corrida: &str,
) -> Result<Eleccion, Error> {
    let elegida = elegir_o_gate(home, pedido, pol, gasto, cfg, ahora, offset, corrida)?;
    let claves = Claves::del_entorno();
    let instante = Instant::now();
    let breaker = proveedores::breaker_global();
    let disponible = |p: Proveedor| proveedores::disponible(&claves, breaker, p, instante);
    let final_ = route::con_respaldo(elegida, cfg, &disponible);
    route::exigir_modelo_que_razona(&final_)?;
    Ok(final_)
}

/// Elige la vía. Con `SinPresupuesto` registra el `Gate` y devuelve el error con la hora de reapertura.
#[allow(clippy::too_many_arguments)]
fn elegir_o_gate(
    home: &Path,
    pedido: &Pedido,
    pol: &Politica,
    gasto: &Uso,
    cfg: &Config,
    ahora: f64,
    offset: i32,
    corrida: &str,
) -> Result<Eleccion, Error> {
    route::elegir(pedido, pol, gasto, cfg, ahora, offset)
        .map_err(|sp| rechazo_presupuesto(home, &sp, pedido, offset, corrida))
}

fn rechazo_presupuesto(
    home: &Path,
    sp: &SinPresupuesto,
    pedido: &Pedido,
    offset: i32,
    corrida: &str,
) -> Error {
    let reabre = uso::iso(sp.reabre, offset);
    let extra = Map::from_iter([
        ("motivo".to_string(), json!(sp.motivo)),
        ("reabre".to_string(), json!(reabre)),
        ("rol".to_string(), json!(nombre_rol(pedido.rol))),
        ("tipo".to_string(), json!(pedido.tipo)),
    ]);
    let mensaje = format!("sin presupuesto: {}; reabre {reabre}", sp.motivo);
    gate_y_error(home, corrida, "presupuesto", extra, mensaje)
}

/// Registra el `Gate` en `home` y devuelve el error de tope. Si el registro falla, el mensaje lo dice.
fn gate_y_error(
    home: &Path,
    corrida: &str,
    fase: &str,
    extra: Map<String, serde_json::Value>,
    mensaje: String,
) -> Error {
    let ruta = observe::ruta_eventos_en(home);
    match observe::registrar_en(&ruta, &observe::evento_gate(corrida, fase, extra)) {
        Ok(()) => Error::Tope(mensaje),
        Err(causa) => Error::Tope(format!(
            "{mensaje}; además no se pudo registrar el Gate: {causa}"
        )),
    }
}

fn imprimir_eleccion(json: bool, e: &Eleccion) -> Result<(), Error> {
    if json {
        println!("{}", serde_json::to_string_pretty(e)?);
    } else {
        println!(
            "via={} proveedor={} modelo={} razonamiento={} ({})",
            etiqueta_via(e.via),
            e.proveedor.map_or("-", etiqueta_proveedor),
            e.modelo.as_deref().unwrap_or("-"),
            e.razonamiento,
            e.motivo
        );
    }
    Ok(())
}

fn etiqueta_via(via: Via) -> &'static str {
    match via {
        Via::Cupo => "cupo",
        Via::Api => "api",
        Via::Oraculo => "oraculo",
        Via::Nube => "nube",
    }
}

pub(crate) fn etiqueta_proveedor(proveedor: Proveedor) -> &'static str {
    match proveedor {
        Proveedor::Anthropic => "anthropic",
        Proveedor::Deepseek => "deepseek",
    }
}
