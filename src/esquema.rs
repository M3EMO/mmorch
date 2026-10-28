//! Imprime el JSON Schema de los tipos públicos de Root.

use schemars::{JsonSchema, schema_for};

use crate::config::Config;
use crate::error::Error;
use crate::ficha::Revision;
use crate::route::Politica;
use crate::tipos::{
    Accion, Conteo, Eleccion, EstadoRequisito, Evento, Fila, Hallazgo, Informe, NivelItem, Pedido,
    Pendiente, Puntaje, Resumen, SinPresupuesto, Uso, Veredicto,
};

/// Nombres que acepta `r00t esquema`.
const TIPOS: &[&str] = &[
    "veredicto",
    "evento",
    "fila",
    "pedido",
    "eleccion",
    "resumen",
    "hallazgo",
    "puntaje",
    "conteo",
    "config",
    "informe",
    "estadorequisito",
    "accion",
    "nivelitem",
    "pendiente",
    "revision",
    "uso",
    "sinpresupuesto",
    "politica",
];

/// JSON Schema (formato pretty) del tipo indicado por su nombre en minúsculas.
pub fn json(tipo: &str) -> Result<String, Error> {
    match tipo {
        "veredicto" => imprimir::<Veredicto>(),
        "evento" => imprimir::<Evento>(),
        "fila" => imprimir::<Fila>(),
        "pedido" => imprimir::<Pedido>(),
        "eleccion" => imprimir::<Eleccion>(),
        "resumen" => imprimir::<Resumen>(),
        "hallazgo" => imprimir::<Hallazgo>(),
        "puntaje" => imprimir::<Puntaje>(),
        "conteo" => imprimir::<Conteo>(),
        "config" => imprimir::<Config>(),
        "informe" => imprimir::<Informe>(),
        "estadorequisito" => imprimir::<EstadoRequisito>(),
        "accion" => imprimir::<Accion>(),
        "nivelitem" => imprimir::<NivelItem>(),
        "pendiente" => imprimir::<Pendiente>(),
        "revision" => imprimir::<Revision>(),
        "uso" => imprimir::<Uso>(),
        "sinpresupuesto" => imprimir::<SinPresupuesto>(),
        "politica" => imprimir::<Politica>(),
        otro => Err(Error::Uso(format!(
            "tipo desconocido '{otro}'. Tipos: {}",
            TIPOS.join(", ")
        ))),
    }
}

fn imprimir<T: JsonSchema>() -> Result<String, Error> {
    Ok(serde_json::to_string_pretty(&schema_for!(T))?)
}
