//! Importación del log de métricas de mmorch: copia de solo lectura y conversión a `Evento`.
//!
//! La salida va a `importado/mmorch-eventos.jsonl`, no a `eventos.jsonl`. Así las llamadas
//! de mmorch no se mezclan con los eventos que registra Root en vivo.

use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error as ErrorDerive;

use crate::config::root_home;
use crate::tipos::{Evento, TipoEvento, VERSION, Via};

/// Corrida con la que se marcan los eventos importados.
const CORRIDA: &str = "mmorch-importado";

/// Errores de la importación. Salen con código 3.
#[derive(Debug, ErrorDerive)]
pub enum ErrorImportar {
    #[error("error de E/S")]
    Io(#[from] io::Error),
    #[error("metrics.jsonl: la línea {linea} no es JSON válido")]
    Linea {
        linea: usize,
        #[source]
        fuente: serde_json::Error,
    },
    #[error("el origen y el destino son el mismo archivo: {0}")]
    MismoArchivo(String),
}

/// Cuántos eventos quedaron escritos tras la importación.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResumenImportacion {
    pub eventos: usize,
}

/// Importa `desde` a `ROOT_HOME`.
pub fn importar_externo(desde: &Path) -> Result<ResumenImportacion, ErrorImportar> {
    importar(desde, &root_home())
}

/// Copia `desde` a `<home>/importado/` y escribe sus eventos en `mmorch-eventos.jsonl`.
pub fn importar(desde: &Path, home: &Path) -> Result<ResumenImportacion, ErrorImportar> {
    // Se lee todo antes de escribir: una línea inválida no deja salidas a medias.
    let texto = fs::read_to_string(desde)?;
    let eventos = convertir(&texto)?;
    let dir = home.join("importado");
    let copia = dir.join("mmorch-metrics.jsonl");
    let salida = dir.join("mmorch-eventos.jsonl");
    for destino in [&copia, &salida] {
        if es_el_mismo(desde, destino) {
            return Err(ErrorImportar::MismoArchivo(destino.display().to_string()));
        }
    }
    fs::create_dir_all(&dir)?;
    fs::write(&copia, &texto)?;
    let mut lineas = String::new();
    for ev in &eventos {
        lineas.push_str(&serde_json::to_string(ev).map_err(io::Error::other)?);
        lineas.push('\n');
    }
    fs::write(&salida, lineas)?;
    Ok(ResumenImportacion {
        eventos: eventos.len(),
    })
}

/// Línea del log de mmorch. Todo campo ausente vale 0 o vacío.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LineaMmorch {
    ts: f64,
    iso: String,
    phase: String,
    pattern: Option<String>,
    node: Option<String>,
    model: Option<String>,
    family: Option<String>,
    in_tokens: u64,
    out_tokens: u64,
    cost_usd: f64,
    latency_s: f64,
    extra: Map<String, Value>,
}

/// Convierte cada línea no vacía del log en un `Evento`.
fn convertir(texto: &str) -> Result<Vec<Evento>, ErrorImportar> {
    let mut eventos = Vec::new();
    for (indice, linea) in texto.lines().enumerate() {
        if linea.trim().is_empty() {
            continue;
        }
        let origen: LineaMmorch =
            serde_json::from_str(linea).map_err(|fuente| ErrorImportar::Linea {
                linea: indice + 1,
                fuente,
            })?;
        eventos.push(evento_de(origen));
    }
    Ok(eventos)
}

fn evento_de(linea: LineaMmorch) -> Evento {
    let mut extra = linea.extra;
    if let Some(pattern) = linea.pattern {
        extra.insert("pattern".into(), Value::String(pattern));
    }
    if let Some(node) = linea.node {
        extra.insert("node".into(), Value::String(node));
    }
    extra.insert("origen".into(), Value::String("mmorch".into()));
    Evento {
        v: VERSION,
        ts: linea.ts,
        iso: linea.iso,
        corrida: CORRIDA.into(),
        fase: linea.phase,
        tipo: TipoEvento::Llamada,
        via: Some(Via::Api),
        modelo: linea.model,
        familia: linea.family,
        in_tokens: linea.in_tokens,
        out_tokens: linea.out_tokens,
        cost_usd: linea.cost_usd,
        latency_s: linea.latency_s,
        extra,
    }
}

/// Dice si dos rutas apuntan al mismo archivo existente.
fn es_el_mismo(a: &Path, b: &Path) -> bool {
    matches!((fs::canonicalize(a), fs::canonicalize(b)), (Ok(x), Ok(y)) if x == y)
}
