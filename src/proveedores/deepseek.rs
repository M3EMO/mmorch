//! DeepSeek: API compatible con OpenAI en `POST {url}/chat/completions`.

use serde_json::{Value, json};

use super::{Clave, Crudo, ErrorProveedor, Peticion, Prompt, entero_en, texto_en};

/// Arma el pedido. Pide el razonamiento en forma explícita: `enabled` o `disabled`, según la elección.
pub(crate) fn armar(
    url_base: &str,
    clave: &Clave,
    modelo: &str,
    prompt: &Prompt,
    razonamiento: bool,
) -> Peticion {
    let mut mensajes = Vec::new();
    if let Some(sistema) = &prompt.sistema {
        mensajes.push(json!({"role": "system", "content": sistema}));
    }
    mensajes.push(json!({"role": "user", "content": prompt.usuario}));
    let tipo = if razonamiento { "enabled" } else { "disabled" };
    let cuerpo = json!({
        "model": modelo,
        "messages": mensajes,
        "max_tokens": prompt.max_tokens,
        "thinking": {"type": tipo},
    });
    Peticion {
        url: format!("{}/chat/completions", url_base.trim_end_matches('/')),
        cabeceras: vec![
            ("authorization", format!("Bearer {}", clave.as_str())),
            ("content-type", "application/json".into()),
        ],
        cuerpo: cuerpo.to_string(),
    }
}

/// Lee el texto de la respuesta. Ignora `reasoning_content`.
/// Con `finish_reason` = `length` el tope cortó la respuesta. Con razonamiento, el contenido puede venir vacío o nulo.
pub(crate) fn leer(texto: &str) -> Result<Crudo, ErrorProveedor> {
    let json: Value =
        serde_json::from_str(texto).map_err(|e| ErrorProveedor::Respuesta(e.to_string()))?;
    let truncada = json
        .pointer("/choices/0/finish_reason")
        .and_then(Value::as_str)
        == Some("length");
    let texto = if truncada {
        json.pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    } else {
        texto_en(&json, "/choices/0/message/content")?
    };
    Ok(Crudo {
        texto,
        in_tokens: entero_en(&json, "/usage/prompt_tokens")?,
        out_tokens: entero_en(&json, "/usage/completion_tokens")?,
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
    fn proveedores_deepseek_cuerpo_pone_sistema_antes_del_usuario() {
        let pet = armar(
            "http://x/",
            &Clave::nueva("k"),
            "deepseek-chat",
            &prompt(Some("sé breve")),
            false,
        );
        assert_eq!(pet.url, "http://x/chat/completions");
        let cuerpo: Value = serde_json::from_str(&pet.cuerpo).expect("JSON");
        assert_eq!(cuerpo["messages"][0]["role"], "system");
        assert_eq!(cuerpo["messages"][1]["content"], "hola");
        assert_eq!(cuerpo["max_tokens"], 64);
    }

    #[test]
    fn proveedores_deepseek_cuerpo_sin_sistema_manda_solo_el_usuario() {
        let pet = armar(
            "http://x",
            &Clave::nueva("k"),
            "deepseek-chat",
            &prompt(None),
            false,
        );
        let cuerpo: Value = serde_json::from_str(&pet.cuerpo).expect("JSON");
        assert_eq!(cuerpo["messages"].as_array().map(Vec::len), Some(1));
    }

    #[test]
    fn proveedores_deepseek_cuerpo_pide_thinking_segun_la_eleccion() {
        for (razonamiento, esperado) in [(true, "enabled"), (false, "disabled")] {
            let pet = armar(
                "http://x",
                &Clave::nueva("k"),
                "deepseek-flash",
                &prompt(None),
                razonamiento,
            );
            let cuerpo: Value = serde_json::from_str(&pet.cuerpo).expect("JSON");
            assert_eq!(
                cuerpo["thinking"]["type"], esperado,
                "razonamiento {razonamiento}"
            );
        }
    }

    #[test]
    fn proveedores_deepseek_stop_con_contenido_nulo_es_error_de_respuesta() {
        let texto = r#"{"choices":[{"message":{"content":null},"finish_reason":"stop"}],
            "usage":{"prompt_tokens":7,"completion_tokens":3}}"#;
        let error = leer(texto).expect_err("un stop sin texto no es texto vacío");
        assert!(matches!(error, ErrorProveedor::Respuesta(_)), "{error:?}");
    }

    #[test]
    fn proveedores_deepseek_leer_ignora_reasoning_content() {
        let texto = r#"{"choices":[{"message":{"content":"ok","reasoning_content":"pensé"}}],
            "usage":{"prompt_tokens":7,"completion_tokens":3}}"#;
        let crudo = leer(texto).expect("parsea");
        assert_eq!(crudo.texto, "ok");
        assert_eq!((crudo.in_tokens, crudo.out_tokens), (7, 3));
    }
}
