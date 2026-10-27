//! Convierte la salida de `claude -p --output-format json` en eventos `Sesion`.

use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::observe::{ErrorObserve, ahora};
use crate::tipos::{Evento, TipoEvento, Via};

/// Lee la salida de `claude -p` y da, por objeto JSON, una `Llamada` por modelo de `modelUsage`
/// y un `Sesion`. El costo va en las `Llamada`; el `Sesion` lleva costo 0 para no contarlo dos veces.
/// Si `modelUsage` falta o está vacío, el `Sesion` lleva el `total_cost_usd`.
/// Acepta un objeto con varias líneas o un objeto por línea.
pub fn leer_sesion_claude(json: &Path, corrida: &str) -> Result<Vec<Evento>, ErrorObserve> {
    let texto = fs::read_to_string(json)?;
    let objetos = serde_json::Deserializer::from_str(&texto)
        .into_iter::<Value>()
        .collect::<Result<Vec<Value>, serde_json::Error>>()?;
    Ok(objetos
        .iter()
        .filter(|objeto| objeto.is_object())
        .flat_map(|objeto| eventos_desde(objeto, corrida))
        .collect())
}

fn eventos_desde(salida: &Value, corrida: &str) -> Vec<Evento> {
    let (ts, iso) = ahora();
    let modelos = salida
        .get("modelUsage")
        .and_then(Value::as_object)
        .filter(|modelos| !modelos.is_empty());
    let mut eventos: Vec<Evento> = modelos
        .into_iter()
        .flatten()
        .map(|(modelo, uso)| Evento {
            ts,
            iso: iso.clone(),
            corrida: corrida.into(),
            tipo: TipoEvento::Llamada,
            via: Some(Via::Cupo),
            modelo: Some(modelo.clone()),
            familia: Some("anthropic".into()),
            in_tokens: entero(uso, &["inputTokens"])
                + entero(uso, &["cacheReadInputTokens"])
                + entero(uso, &["cacheCreationInputTokens"]),
            out_tokens: entero(uso, &["outputTokens"]),
            cost_usd: uso.get("costUSD").and_then(Value::as_f64).unwrap_or(0.0),
            ..Evento::default()
        })
        .collect();
    eventos.push(evento_sesion(salida, corrida, (ts, iso), modelos.is_some()));
    eventos
}

/// Evento `Sesion` con turnos y duración. Sin `modelUsage`, lleva el costo y los tokens del total.
fn evento_sesion(
    salida: &Value,
    corrida: &str,
    (ts, iso): (f64, String),
    con_modelos: bool,
) -> Evento {
    let mut extra = serde_json::Map::new();
    for clave in ["num_turns", "duration_ms", "session_id", "stop_reason"] {
        if let Some(valor) = salida.get(clave) {
            extra.insert(clave.into(), valor.clone());
        }
    }
    let total = salida
        .get("total_cost_usd")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    if let Some(valor) = salida.get("total_cost_usd") {
        extra.insert("total_cost_usd".into(), valor.clone());
    }
    let (in_tokens, out_tokens, cost_usd) = if con_modelos {
        (0, 0, 0.0)
    } else {
        let entrada = entero(salida, &["usage", "input_tokens"])
            + entero(salida, &["usage", "cache_creation_input_tokens"])
            + entero(salida, &["usage", "cache_read_input_tokens"]);
        (entrada, entero(salida, &["usage", "output_tokens"]), total)
    };
    Evento {
        ts,
        iso,
        corrida: corrida.into(),
        tipo: TipoEvento::Sesion,
        via: Some(Via::Cupo),
        familia: Some("anthropic".into()),
        in_tokens,
        out_tokens,
        cost_usd,
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
