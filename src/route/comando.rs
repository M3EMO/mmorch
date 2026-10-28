//! Glue de `r00t route` y `r00t llama`: lee `root.toml` y `politica.json`, llama al núcleo puro
//! `elegir`, chequea el costo antes de llamar y registra un `Gate` en cada rechazo.

use std::fs;
use std::io;
use std::path::Path;

use serde_json::{Map, json};

use crate::config::{Config, root_home};
use crate::error::Error;
use crate::observe::{self, uso};
use crate::proveedores::{self, Prompt};
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
    let home = root_home();
    let cfg = Config::cargar(&home.join("root.toml"))?;
    let pol = cargar_politica(&home.join("politica.json"))?;
    let ahora = observe::ahora().0;
    let offset = uso::offset_local();
    let gasto = uso_actual(&cfg, ahora, offset)?;
    let pedido = Pedido {
        rol,
        tipo: tipo.into(),
        checkeable,
        tokens_estimados: tokens,
    };
    let eleccion = elegir_o_gate(&pedido, &pol, &gasto, &cfg, ahora, offset, CORRIDA_ROUTE)?;
    imprimir_eleccion(json, &eleccion)
}

/// `r00t llama`: rutea, chequea el costo y llama. Devuelve el código de `imprimir_para_cli`.
pub fn llama(
    rol: Rol,
    tipo: &str,
    prompt_file: &Path,
    corrida: &str,
    max_tokens: u32,
) -> Result<u8, Error> {
    let home = root_home();
    let cfg = Config::cargar(&home.join("root.toml"))?;
    let pol = cargar_politica(&home.join("politica.json"))?;
    let usuario = fs::read_to_string(prompt_file).map_err(|e| {
        Error::Uso(format!(
            "no se pudo leer --prompt-file {}: {e}",
            prompt_file.display()
        ))
    })?;
    let ahora = observe::ahora().0;
    let offset = uso::offset_local();
    let gasto = uso_actual(&cfg, ahora, offset)?;
    let pedido = Pedido {
        rol,
        tipo: tipo.into(),
        checkeable: false,
        tokens_estimados: max_tokens,
    };
    let eleccion = elegir_o_gate(&pedido, &pol, &gasto, &cfg, ahora, offset, corrida)?;
    if eleccion.via == Via::Cupo {
        return Err(Error::Uso(
            "este rol va por cupo: lo corre `claude -p`, no `r00t llama`".into(),
        ));
    }
    let modelo = eleccion.modelo.clone().unwrap_or_default();
    chequear_tope(
        &cfg,
        &gasto,
        &modelo,
        usuario.chars().count(),
        max_tokens,
        corrida,
    )?;
    let prompt = Prompt {
        sistema: None,
        usuario,
        max_tokens,
        fase: "llama".into(),
    };
    let resultado = proveedores::llamar(&eleccion, &prompt, corrida);
    Ok(proveedores::imprimir_para_cli(
        resultado,
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
    ))
}

/// Gasto actual de `eventos.jsonl`. Un log ilegible es error: sin gasto no se puede decidir.
fn uso_actual(cfg: &Config, ahora: f64, offset: i32) -> Result<Uso, Error> {
    Ok(uso::uso_en(
        &observe::ruta_eventos(),
        ahora,
        offset,
        cfg.balance.cupo.bloque_horas,
    )?)
}

/// Elige la vía. Con `SinPresupuesto` registra el `Gate` y devuelve el error con la hora de reapertura.
fn elegir_o_gate(
    pedido: &Pedido,
    pol: &Politica,
    gasto: &Uso,
    cfg: &Config,
    ahora: f64,
    offset: i32,
    corrida: &str,
) -> Result<Eleccion, Error> {
    route::elegir(pedido, pol, gasto, cfg, ahora, offset)
        .map_err(|sp| rechazo_presupuesto(&sp, pedido, offset, corrida))
}

fn rechazo_presupuesto(sp: &SinPresupuesto, pedido: &Pedido, offset: i32, corrida: &str) -> Error {
    let reabre = uso::iso(sp.reabre, offset);
    let extra = Map::from_iter([
        ("motivo".to_string(), json!(sp.motivo)),
        ("reabre".to_string(), json!(reabre)),
        ("rol".to_string(), json!(nombre_rol(pedido.rol))),
        ("tipo".to_string(), json!(pedido.tipo)),
    ]);
    let mensaje = format!("sin presupuesto: {}; reabre {reabre}", sp.motivo);
    gate_y_error(corrida, "presupuesto", extra, mensaje)
}

/// Chequeo previo de costo: el peor caso del pedido no puede pasar el margen del tope de API.
/// Sin precio para el modelo no hay estimación, así que se rechaza (falla cerrado).
fn chequear_tope(
    cfg: &Config,
    gasto: &Uso,
    modelo: &str,
    caracteres: usize,
    max_tokens: u32,
    corrida: &str,
) -> Result<(), Error> {
    let tope = cfg.balance.api.tope_usd;
    let margen = tope - gasto.api_mes_usd;
    let Some(precio) = cfg.precios.get(modelo) else {
        let extra = margen_y_modelo(None, margen, modelo);
        let mensaje = format!(
            "sin precio en root.toml para {modelo}: no se puede estimar el costo, la llamada no se envía"
        );
        return Err(gate_y_error(corrida, "tope", extra, mensaje));
    };
    // Estimación conservadora: un token cada tres caracteres, y todo el máximo de salida.
    let in_est = (caracteres as f64 / 3.0).ceil();
    let estimado =
        in_est * precio.usd_entrada / 1e6 + f64::from(max_tokens) * precio.usd_salida / 1e6;
    if estimado > margen {
        let extra = margen_y_modelo(Some(estimado), margen, modelo);
        let mensaje = format!(
            "llamada rechazada antes de enviarla: costo estimado {estimado:.6} USD, margen {margen:.6} USD del tope de API de {tope:.2} USD"
        );
        return Err(gate_y_error(corrida, "tope", extra, mensaje));
    }
    Ok(())
}

fn margen_y_modelo(
    estimado: Option<f64>,
    margen: f64,
    modelo: &str,
) -> Map<String, serde_json::Value> {
    Map::from_iter([
        ("estimado".to_string(), json!(estimado)),
        ("margen".to_string(), json!(margen)),
        ("modelo".to_string(), json!(modelo)),
    ])
}

/// Registra el `Gate` y devuelve el error de tope. Si el registro falla, el mensaje lo dice.
fn gate_y_error(
    corrida: &str,
    fase: &str,
    extra: Map<String, serde_json::Value>,
    mensaje: String,
) -> Error {
    match observe::registrar(&observe::evento_gate(corrida, fase, extra)) {
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

fn etiqueta_proveedor(proveedor: Proveedor) -> &'static str {
    match proveedor {
        Proveedor::Anthropic => "anthropic",
        Proveedor::Deepseek => "deepseek",
    }
}
