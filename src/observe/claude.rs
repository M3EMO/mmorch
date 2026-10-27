//! Convierte la salida de `claude -p --output-format json` en eventos `Sesion`.

use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::observe::{ErrorObserve, ahora};
use crate::tipos::{Evento, TipoEvento, Via};

/// Lee la salida de `claude -p` y da un evento `Sesion` por objeto JSON.
/// Acepta un objeto con varias líneas o un objeto por línea.
pub fn leer_sesion_claude(json: &Path, corrida: &str) -> Result<Vec<Evento>, ErrorObserve> {
    let texto = fs::read_to_string(json)?;
    let objetos = serde_json::Deserializer::from_str(&texto)
        .into_iter::<Value>()
        .collect::<Result<Vec<Value>, serde_json::Error>>()?;
    Ok(objetos
        .iter()
        .filter(|objeto| objeto.is_object())
        .map(|objeto| evento_desde(objeto, corrida))
        .collect())
}

fn evento_desde(salida: &Value, corrida: &str) -> Evento {
    let (ts, iso) = ahora();
    let mut extra = serde_json::Map::new();
    for clave in [
        "num_turns",
        "duration_ms",
        "session_id",
        "stop_reason",
        "modelUsage",
    ] {
        if let Some(valor) = salida.get(clave) {
            extra.insert(clave.into(), valor.clone());
        }
    }
    let entrada = entero(salida, &["usage", "input_tokens"]);
    let creacion = entero(salida, &["usage", "cache_creation_input_tokens"]);
    let lectura = entero(salida, &["usage", "cache_read_input_tokens"]);
    Evento {
        ts,
        iso,
        corrida: corrida.into(),
        tipo: TipoEvento::Sesion,
        via: Some(Via::Cupo),
        modelo: primera_clave(salida.get("modelUsage")),
        familia: Some("anthropic".into()),
        in_tokens: entrada + creacion + lectura,
        out_tokens: entero(salida, &["usage", "output_tokens"]),
        cost_usd: salida
            .get("total_cost_usd")
            .and_then(Value::as_f64)
            .unwrap_or(0.0),
        latency_s: entero(salida, &["duration_ms"]) as f64 / 1000.0,
        extra,
        ..Evento::default()
    }
}

/// Valor entero en la ruta dada. Ausente vale 0.
fn entero(salida: &Value, ruta: &[&str]) -> u64 {
    ruta.iter()
        .try_fold(salida, |actual, clave| actual.get(*clave))
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

/// Primera clave de `modelUsage`, que es el modelo de la sesión.
fn primera_clave(model_usage: Option<&Value>) -> Option<String> {
    model_usage?.as_object()?.keys().next().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn observe_sesion_con_campos_ausentes_vale_cero_sin_panico() {
        let dir = TempDir::new().expect("temp dir");
        let ruta = dir.path().join("salida.json");
        fs::write(&ruta, "{}\n").expect("escribe");
        let eventos = leer_sesion_claude(&ruta, "c1").expect("un objeto vacío parsea");
        assert_eq!(eventos.len(), 1);
        let ev = &eventos[0];
        assert_eq!(ev.tipo, TipoEvento::Sesion);
        assert_eq!(ev.via, Some(Via::Cupo));
        assert_eq!(ev.cost_usd, 0.0);
        assert_eq!(ev.in_tokens, 0);
        assert_eq!(ev.out_tokens, 0);
        assert_eq!(ev.latency_s, 0.0);
        assert_eq!(ev.modelo, None);
        assert!(ev.extra.is_empty());
    }
}
