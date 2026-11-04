//! Costo estimado de un rol cortado por tiempo. Un corte no deja línea `result`, así que el costo
//! sale de los `usage` de los mensajes `assistant` del transcript parcial. El precio sale de
//! `Config.precios`. Sin precio del modelo, rige `PRECIO_PESIMISTA` y el evento lo marca.

use std::collections::HashSet;

use serde_json::{Value, json};

use crate::config::Precio;
use crate::observe;
use crate::tipos::{Evento, TipoEvento, Via};

/// Precio pesimista para un modelo sin precio en `root.toml`: el de Opus, en USD por millón de tokens.
/// Gastar de más en la estimación es mejor que subestimar un corte.
pub const PRECIO_PESIMISTA: Precio = Precio {
    usd_entrada: 15.0,
    usd_salida: 75.0,
    umbral_entrada: None,
    usd_entrada_alto: None,
    usd_salida_alto: None,
};

/// Tokens de un transcript parcial. La entrada suma la de caché: lo que cuesta entrar al modelo.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Uso {
    pub in_tokens: u64,
    pub out_tokens: u64,
}

/// Costo estimado en USD y si el precio fue el supuesto.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Costo {
    pub usd: f64,
    pub precio_supuesto: bool,
}

/// Suma el `usage` de las líneas `assistant` del transcript. Un `message.id` repetido cuenta una vez.
/// Las líneas que no son JSON se ignoran.
pub fn uso_de_transcript(stdout: &[u8]) -> Uso {
    let texto = String::from_utf8_lossy(stdout);
    let mut vistos: HashSet<String> = HashSet::new();
    let mut uso = Uso::default();
    for linea in texto.lines() {
        let Ok(valor) = serde_json::from_str::<Value>(linea) else {
            continue;
        };
        if valor.get("type").and_then(Value::as_str) != Some("assistant") {
            continue;
        }
        let Some(mensaje) = valor.get("message") else {
            continue;
        };
        if let Some(id) = mensaje.get("id").and_then(Value::as_str)
            && !vistos.insert(id.to_string())
        {
            continue;
        }
        let Some(usage) = mensaje.get("usage") else {
            continue;
        };
        let entrada = campo(usage, "input_tokens")
            .saturating_add(campo(usage, "cache_creation_input_tokens"))
            .saturating_add(campo(usage, "cache_read_input_tokens"));
        uso.in_tokens = uso.in_tokens.saturating_add(entrada);
        uso.out_tokens = uso.out_tokens.saturating_add(campo(usage, "output_tokens"));
    }
    uso
}

/// Número entero de `usage` por `clave`. Ausente vale 0.
fn campo(usage: &Value, clave: &str) -> u64 {
    usage.get(clave).and_then(Value::as_u64).unwrap_or(0)
}

/// Costo de `uso` al precio del modelo. Sin precio, usa `PRECIO_PESIMISTA` y marca `precio_supuesto`.
pub fn costo_de_uso(uso: &Uso, precio: Option<&Precio>) -> Costo {
    let (precio, supuesto) = match precio {
        Some(precio) => (precio, false),
        None => (&PRECIO_PESIMISTA, true),
    };
    let (usd_entrada, usd_salida) = precio.tarifa_para(uso.in_tokens);
    let usd = uso.in_tokens as f64 * usd_entrada / 1e6 + uso.out_tokens as f64 * usd_salida / 1e6;
    Costo {
        usd,
        precio_supuesto: supuesto,
    }
}

/// Evento `Llamada` de cupo con el costo estimado de un rol cortado. Lleva `costo_estimado` y, si el
/// precio fue el supuesto, `precio_supuesto`.
pub fn evento_de_corte(corrida: &str, modelo: &str, uso: &Uso, costo: &Costo) -> Evento {
    let mut ev = observe::evento_manual(corrida, TipoEvento::Llamada, None);
    ev.via = Some(Via::Cupo);
    ev.modelo = Some(modelo.into());
    ev.familia = Some("anthropic".into());
    ev.in_tokens = uso.in_tokens;
    ev.out_tokens = uso.out_tokens;
    ev.cost_usd = costo.usd;
    ev.extra.insert("costo_estimado".into(), json!(true));
    if costo.precio_supuesto {
        ev.extra.insert("precio_supuesto".into(), json!(true));
    }
    ev
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn costo_cortado_suma_usage_sin_contar_ids_repetidos() {
        let texto = concat!(
            "{\"type\":\"system\",\"subtype\":\"init\"}\n",
            "{\"type\":\"assistant\",\"message\":{\"id\":\"m1\",\"usage\":{\"input_tokens\":10,\"cache_creation_input_tokens\":2,\"cache_read_input_tokens\":3,\"output_tokens\":5}}}\n",
            "{\"type\":\"assistant\",\"message\":{\"id\":\"m1\",\"usage\":{\"input_tokens\":10,\"cache_creation_input_tokens\":2,\"cache_read_input_tokens\":3,\"output_tokens\":5}}}\n",
            "{\"type\":\"assistant\",\"message\":{\"id\":\"m2\",\"usage\":{\"input_tokens\":100,\"output_tokens\":7}}}\n",
            "no es json\n",
        );
        assert_eq!(
            uso_de_transcript(texto.as_bytes()),
            Uso {
                in_tokens: 115,
                out_tokens: 12
            }
        );
    }

    #[test]
    fn costo_cortado_fixture_cuenta_caché_como_entrada() {
        let stdout = include_bytes!("../../tests/fixtures/transcripts/cortado.jsonl");
        assert_eq!(
            uso_de_transcript(stdout),
            Uso {
                in_tokens: 3500,
                out_tokens: 130
            }
        );
    }

    #[test]
    fn costo_cortado_sin_precio_usa_el_pesimista_y_lo_marca() {
        let uso = Uso {
            in_tokens: 1_000_000,
            out_tokens: 1_000_000,
        };
        let costo = costo_de_uso(&uso, None);
        assert!(costo.precio_supuesto);
        assert!((costo.usd - 90.0).abs() < 1e-9, "{costo:?}");
    }

    #[test]
    fn costo_cortado_con_precio_no_lo_marca_supuesto() {
        let precio = Precio {
            usd_entrada: 2.0,
            usd_salida: 10.0,
            umbral_entrada: None,
            usd_entrada_alto: None,
            usd_salida_alto: None,
        };
        let uso = Uso {
            in_tokens: 500_000,
            out_tokens: 100_000,
        };
        let costo = costo_de_uso(&uso, Some(&precio));
        assert!(!costo.precio_supuesto);
        assert!((costo.usd - 2.0).abs() < 1e-9, "{costo:?}");
    }

    #[test]
    fn costo_cortado_evento_es_llamada_de_cupo_con_costo_estimado() {
        let uso = Uso {
            in_tokens: 3500,
            out_tokens: 130,
        };
        let costo = Costo {
            usd: 0.5,
            precio_supuesto: true,
        };
        let ev = evento_de_corte("corre-1", "haiku", &uso, &costo);
        assert_eq!(ev.tipo, TipoEvento::Llamada);
        assert_eq!(ev.via, Some(Via::Cupo));
        assert_eq!(ev.corrida, "corre-1");
        assert_eq!(ev.modelo.as_deref(), Some("haiku"));
        assert_eq!((ev.in_tokens, ev.out_tokens), (3500, 130));
        assert_eq!(ev.cost_usd, 0.5);
        assert_eq!(ev.extra["costo_estimado"], Value::Bool(true));
        assert_eq!(ev.extra["precio_supuesto"], Value::Bool(true));
    }
}
