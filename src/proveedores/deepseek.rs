//! DeepSeek: API compatible con OpenAI en `POST {url}/chat/completions`.

use serde_json::{Value, json};

use super::{Clave, Crudo, ErrorProveedor, Peticion, Prompt, entero_en, texto_en};

/// Arma el pedido. El razonamiento no cambia nada aquí: lo decide el modelo que elige Route.
pub(crate) fn armar(url_base: &str, clave: &Clave, modelo: &str, prompt: &Prompt) -> Peticion {
    let mut mensajes = Vec::new();
    if let Some(sistema) = &prompt.sistema {
        mensajes.push(json!({"role": "system", "content": sistema}));
    }
    mensajes.push(json!({"role": "user", "content": prompt.usuario}));
    let cuerpo = json!({
        "model": modelo,
        "messages": mensajes,
        "max_tokens": prompt.max_tokens,
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
pub(crate) fn leer(texto: &str) -> Result<Crudo, ErrorProveedor> {
    let json: Value =
        serde_json::from_str(texto).map_err(|e| ErrorProveedor::Respuesta(e.to_string()))?;
    Ok(Crudo {
        texto: texto_en(&json, "/choices/0/message/content")?,
        in_tokens: entero_en(&json, "/usage/prompt_tokens")?,
        out_tokens: entero_en(&json, "/usage/completion_tokens")?,
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
        );
        let cuerpo: Value = serde_json::from_str(&pet.cuerpo).expect("JSON");
        assert_eq!(cuerpo["messages"].as_array().map(Vec::len), Some(1));
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
