//! Anthropic: Messages API en `POST {url}/v1/messages`.

use serde_json::{Value, json};

use super::{Clave, Crudo, ErrorProveedor, Peticion, Prompt, entero_en};

const VERSION_API: &str = "2023-06-01";

/// Arma el pedido con las cabeceras de la API de Messages.
pub(crate) fn armar(url_base: &str, clave: &Clave, modelo: &str, prompt: &Prompt) -> Peticion {
    let mut cuerpo = json!({
        "model": modelo,
        "max_tokens": prompt.max_tokens,
        "messages": [{"role": "user", "content": prompt.usuario}],
    });
    if let Some(sistema) = &prompt.sistema {
        cuerpo["system"] = json!(sistema);
    }
    Peticion {
        url: format!("{}/v1/messages", url_base.trim_end_matches('/')),
        cabeceras: vec![
            ("x-api-key", clave.as_str().to_string()),
            ("anthropic-version", VERSION_API.into()),
            ("content-type", "application/json".into()),
        ],
        cuerpo: cuerpo.to_string(),
    }
}

/// Une los bloques `text` de `content`.
pub(crate) fn leer(texto: &str) -> Result<Crudo, ErrorProveedor> {
    let json: Value =
        serde_json::from_str(texto).map_err(|e| ErrorProveedor::Respuesta(e.to_string()))?;
    let bloques = json
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(|| ErrorProveedor::Respuesta("falta content".into()))?;
    let texto: String = bloques
        .iter()
        .filter(|bloque| bloque.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|bloque| bloque.get("text").and_then(Value::as_str))
        .collect();
    let truncada = json.get("stop_reason").and_then(Value::as_str) == Some("max_tokens");
    Ok(Crudo {
        texto,
        in_tokens: entero_en(&json, "/usage/input_tokens")?,
        out_tokens: entero_en(&json, "/usage/output_tokens")?,
        truncada,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prompt(sistema: Option<&str>) -> Prompt {
        Prompt {
            sistema: sistema.map(String::from),
            usuario: "hola".into(),
            max_tokens: 64,
            fase: "prueba".into(),
        }
    }

    #[test]
    fn proveedores_anthropic_cabeceras_llevan_clave_y_version() {
        let pet = armar(
            "http://x/",
            &Clave::nueva("k1"),
            "claude-haiku-5-5",
            &prompt(None),
        );
        assert_eq!(pet.url, "http://x/v1/messages");
        assert!(pet.cabeceras.contains(&("x-api-key", "k1".to_string())));
        assert!(
            pet.cabeceras
                .contains(&("anthropic-version", "2023-06-01".to_string()))
        );
    }

    #[test]
    fn proveedores_anthropic_cuerpo_sin_sistema_omite_el_campo() {
        let pet = armar("http://x", &Clave::nueva("k"), "m", &prompt(None));
        let cuerpo: Value = serde_json::from_str(&pet.cuerpo).expect("JSON");
        assert!(cuerpo.get("system").is_none());
        assert_eq!(cuerpo["messages"][0]["role"], "user");
    }

    #[test]
    fn proveedores_anthropic_leer_une_bloques_de_texto() {
        let texto = r#"{"content":[{"type":"text","text":"ho"},{"type":"text","text":"la"}],
            "usage":{"input_tokens":10,"output_tokens":5}}"#;
        let crudo = leer(texto).expect("parsea");
        assert_eq!(crudo.texto, "hola");
        assert_eq!((crudo.in_tokens, crudo.out_tokens), (10, 5));
    }
}
