//! Costo estimado de un rol cortado por tiempo. Un corte no deja línea `result`, así que el costo
//! sale de los `usage` de los mensajes `assistant` del transcript parcial. El precio sale de
//! `Config.precios`. Sin precio del modelo, rige `PRECIO_PESIMISTA` y el evento lo marca.
//! La caché tiene su propio precio: la lectura cuesta 0,1 y la escritura 1,25 de la entrada.

use std::collections::HashSet;

use serde_json::{Value, json};

use crate::config::{Config, Precio};
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

/// Factor de tarifa de entrada que cobra una lectura de caché.
const FACTOR_LECTURA_CACHE: f64 = 0.1;
/// Factor de tarifa de entrada que cobra una escritura de caché.
const FACTOR_ESCRITURA_CACHE: f64 = 1.25;

/// Tokens de un transcript parcial. `in_tokens` es solo entrada sin caché.
/// La caché va aparte: lectura y escritura. Cada una tiene su factor de precio.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Uso {
    pub in_tokens: u64,
    pub out_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
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
        uso.in_tokens = uso.in_tokens.saturating_add(campo(usage, "input_tokens"));
        uso.out_tokens = uso.out_tokens.saturating_add(campo(usage, "output_tokens"));
        uso.cache_read_tokens = uso
            .cache_read_tokens
            .saturating_add(campo(usage, "cache_read_input_tokens"));
        uso.cache_write_tokens = uso
            .cache_write_tokens
            .saturating_add(campo(usage, "cache_creation_input_tokens"));
    }
    uso
}

/// Número entero de `usage` por `clave`. Ausente vale 0.
fn campo(usage: &Value, clave: &str) -> u64 {
    usage.get(clave).and_then(Value::as_u64).unwrap_or(0)
}

/// Precio del rol cortado. Un alias de cupo se traduce a su id de API con `ruteo.api_equivalente`.
/// Sin traducción, busca el nombre tal cual en `precios`.
pub fn precio_de_rol<'a>(config: &'a Config, modelo: &str) -> Option<&'a Precio> {
    let id = config
        .ruteo
        .api_equivalente
        .get(modelo)
        .map_or(modelo, |api| api.modelo.as_str());
    config.precios.get(id)
}

/// Costo de `uso` al precio del modelo. Sin precio, usa `PRECIO_PESIMISTA` y marca `precio_supuesto`.
pub fn costo_de_uso(uso: &Uso, precio: Option<&Precio>) -> Costo {
    let (precio, supuesto) = match precio {
        Some(precio) => (precio, false),
        None => (&PRECIO_PESIMISTA, true),
    };
    // El umbral de prompt largo cuenta todo lo que entra al modelo, caché incluida.
    let entra_al_modelo = uso
        .in_tokens
        .saturating_add(uso.cache_read_tokens)
        .saturating_add(uso.cache_write_tokens);
    let (usd_entrada, usd_salida) = precio.tarifa_para(entra_al_modelo);
    let entrada_ponderada = uso.in_tokens as f64
        + FACTOR_LECTURA_CACHE * uso.cache_read_tokens as f64
        + FACTOR_ESCRITURA_CACHE * uso.cache_write_tokens as f64;
    let usd = entrada_ponderada * usd_entrada / 1e6 + uso.out_tokens as f64 * usd_salida / 1e6;
    Costo {
        usd,
        precio_supuesto: supuesto,
    }
}

/// Evento `Llamada` de cupo con el costo estimado de un rol cortado. Lleva `costo_estimado` y, si el
/// precio fue el supuesto, `precio_supuesto`. La caché va en `extra`, no en `in_tokens`.
pub fn evento_de_corte(corrida: &str, modelo: &str, uso: &Uso, costo: &Costo) -> Evento {
    let mut ev = observe::evento_manual(corrida, TipoEvento::Llamada, None);
    ev.via = Some(Via::Cupo);
    ev.modelo = Some(modelo.into());
    ev.familia = Some("anthropic".into());
    ev.in_tokens = uso.in_tokens;
    ev.out_tokens = uso.out_tokens;
    ev.cost_usd = costo.usd;
    ev.extra.insert("costo_estimado".into(), json!(true));
    ev.extra
        .insert("cache_read_tokens".into(), json!(uso.cache_read_tokens));
    ev.extra
        .insert("cache_write_tokens".into(), json!(uso.cache_write_tokens));
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
                in_tokens: 110,
                out_tokens: 12,
                cache_read_tokens: 3,
                cache_write_tokens: 2,
            }
        );
    }

    #[test]
    fn costo_cortado_uso_separa_caché_de_entrada() {
        let texto = "{\"type\":\"assistant\",\"message\":{\"id\":\"m1\",\"usage\":{\"input_tokens\":1154,\"cache_creation_input_tokens\":2797230,\"cache_read_input_tokens\":54732360,\"output_tokens\":7181}}}\n";
        assert_eq!(
            uso_de_transcript(texto.as_bytes()),
            Uso {
                in_tokens: 1154,
                out_tokens: 7181,
                cache_read_tokens: 54_732_360,
                cache_write_tokens: 2_797_230,
            }
        );
    }

    #[test]
    fn costo_cortado_fixture_separa_caché_de_entrada() {
        let stdout = include_bytes!("../../tests/fixtures/transcripts/cortado.jsonl");
        assert_eq!(
            uso_de_transcript(stdout),
            Uso {
                in_tokens: 2500,
                out_tokens: 130,
                cache_read_tokens: 800,
                cache_write_tokens: 200,
            }
        );
    }

    #[test]
    fn costo_cortado_con_cache_cuesta_cerca_de_18_usd() {
        let precio = Precio {
            usd_entrada: 2.0,
            usd_salida: 10.0,
            umbral_entrada: None,
            usd_entrada_alto: None,
            usd_salida_alto: None,
        };
        let uso = Uso {
            in_tokens: 1154,
            out_tokens: 7181,
            cache_read_tokens: 54_732_360,
            cache_write_tokens: 2_797_230,
        };
        let costo = costo_de_uso(&uso, Some(&precio));
        let esperado =
            (1154.0 + 0.1 * 54_732_360.0 + 1.25 * 2_797_230.0) / 1e6 * 2.0 + 7181.0 / 1e6 * 10.0;
        assert!(
            (costo.usd - esperado).abs() < 0.05,
            "{costo:?} vs {esperado}"
        );
        assert!(costo.usd > 17.0 && costo.usd < 19.0, "{costo:?}");
    }

    #[test]
    fn costo_cortado_umbral_cuenta_la_caché() {
        let precio = Precio {
            usd_entrada: 1.0,
            usd_salida: 1.0,
            umbral_entrada: Some(200_000),
            usd_entrada_alto: Some(4.0),
            usd_salida_alto: Some(4.0),
        };
        // Entrada pura de 10 tokens: sola no pasa el umbral. Con caché sí.
        let uso = Uso {
            in_tokens: 10,
            cache_read_tokens: 300_000,
            ..Uso::default()
        };
        let costo = costo_de_uso(&uso, Some(&precio));
        let esperado = (10.0 + 0.1 * 300_000.0) / 1e6 * 4.0;
        assert!((costo.usd - esperado).abs() < 1e-9, "{costo:?}");
    }

    #[test]
    fn costo_cortado_sin_precio_usa_el_pesimista_y_lo_marca() {
        let uso = Uso {
            in_tokens: 1_000_000,
            out_tokens: 1_000_000,
            ..Uso::default()
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
            ..Uso::default()
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
            ..Uso::default()
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

    #[test]
    fn costo_cortado_evento_lleva_caché_en_extra() {
        let uso = Uso {
            in_tokens: 1154,
            out_tokens: 7181,
            cache_read_tokens: 54_732_360,
            cache_write_tokens: 2_797_230,
        };
        let costo = Costo {
            usd: 18.0,
            precio_supuesto: false,
        };
        let ev = evento_de_corte("corre-1", "opus", &uso, &costo);
        assert_eq!(
            ev.in_tokens, 1154,
            "la entrada del evento es solo entrada pura"
        );
        assert_eq!(ev.extra["cache_read_tokens"], json!(54_732_360u64));
        assert_eq!(ev.extra["cache_write_tokens"], json!(2_797_230u64));
    }
}
