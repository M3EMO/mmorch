//! DeepSeek: API compatible con OpenAI en `POST {url}/chat/completions`, con `"stream": true`.

use serde_json::{Value, json};

use super::{Clave, Crudo, ErrorProveedor, Peticion, Prompt, entero_en, mensaje_de_error};

/// Arma el pedido. Pide el razonamiento en forma explícita: `enabled` o `disabled`, según la elección.
/// Pide el stream con `include_usage`: el uso llega en el último chunk, antes de `[DONE]`.
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
        "stream": true,
        "stream_options": {"include_usage": true},
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

/// Acumula los chunks de un stream de DeepSeek hasta el `Crudo` de la llamada. Ignora
/// `reasoning_content`. Sin el chunk de uso, el stream cuenta como cortado.
#[derive(Debug, Default)]
pub(crate) struct Acumulador {
    texto: String,
    /// `true` si llegó algún `delta.content`, aunque sea vacío.
    vio_contenido: bool,
    finish: Option<String>,
    in_tokens: Option<u64>,
    out_tokens: Option<u64>,
}

impl Acumulador {
    /// Procesa el JSON de un evento del stream.
    pub(crate) fn consumir(&mut self, datos: &str) -> Result<(), ErrorProveedor> {
        let json: Value =
            serde_json::from_str(datos).map_err(|e| ErrorProveedor::Respuesta(e.to_string()))?;
        if let Some(error) = json.get("error") {
            return Err(ErrorProveedor::Respuesta(mensaje_de_error(error)));
        }
        if let Some(contenido) = json
            .pointer("/choices/0/delta/content")
            .and_then(Value::as_str)
        {
            self.texto.push_str(contenido);
            self.vio_contenido = true;
        }
        if let Some(finish) = json
            .pointer("/choices/0/finish_reason")
            .and_then(Value::as_str)
        {
            self.finish = Some(finish.into());
        }
        if json.get("usage").is_some_and(|uso| !uso.is_null()) {
            self.in_tokens = Some(entero_en(&json, "/usage/prompt_tokens")?);
            self.out_tokens = Some(entero_en(&json, "/usage/completion_tokens")?);
        }
        Ok(())
    }

    /// Cierra el stream. Con `finish_reason` = `length` el tope cortó la respuesta: es truncada, y
    /// puede venir sin texto. Sin uso, el costo no se midió: la llamada cuenta como cortada.
    pub(crate) fn cerrar(self) -> Result<Crudo, ErrorProveedor> {
        let (Some(in_tokens), Some(out_tokens)) = (self.in_tokens, self.out_tokens) else {
            return Err(ErrorProveedor::Cortada(
                "el stream terminó sin el evento de uso".into(),
            ));
        };
        let truncada = self.finish.as_deref() == Some("length");
        if !truncada && !self.vio_contenido {
            return Err(ErrorProveedor::Respuesta(
                "falta /choices/0/delta/content".into(),
            ));
        }
        Ok(Crudo {
            texto: self.texto,
            in_tokens,
            out_tokens,
            truncada,
        })
    }
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

    /// Resultado de acumular los eventos dados, en orden, y cerrar el stream.
    fn acumula(datos: &[&str]) -> Result<Crudo, ErrorProveedor> {
        let mut acumulador = Acumulador::default();
        for dato in datos {
            acumulador.consumir(dato)?;
        }
        acumulador.cerrar()
    }

    const USO: &str = r#"{"choices":[],"usage":{"prompt_tokens":7,"completion_tokens":3}}"#;

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
    fn proveedores_deepseek_cuerpo_pide_stream_con_uso_al_final() {
        let pet = armar(
            "http://x",
            &Clave::nueva("k"),
            "deepseek-chat",
            &prompt(None),
            false,
        );
        let cuerpo: Value = serde_json::from_str(&pet.cuerpo).expect("JSON");
        assert_eq!(cuerpo["stream"], true);
        assert_eq!(cuerpo["stream_options"]["include_usage"], true);
    }

    #[test]
    fn proveedores_deepseek_stream_stop_sin_contenido_es_error_de_respuesta() {
        let error = acumula(&[r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#, USO])
            .expect_err("un stop sin texto no es texto vacío");
        assert!(matches!(error, ErrorProveedor::Respuesta(_)), "{error:?}");
    }

    #[test]
    fn proveedores_deepseek_stream_ignora_reasoning_content() {
        let crudo = acumula(&[
            r#"{"choices":[{"delta":{"reasoning_content":"pensé"},"finish_reason":null}]}"#,
            r#"{"choices":[{"delta":{"content":"ok"},"finish_reason":null}]}"#,
            r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#,
            USO,
        ])
        .expect("acumula");
        assert_eq!(crudo.texto, "ok");
        assert_eq!((crudo.in_tokens, crudo.out_tokens), (7, 3));
        assert!(!crudo.truncada);
    }

    #[test]
    fn proveedores_deepseek_stream_junta_los_trozos_de_contenido() {
        let crudo = acumula(&[
            r#"{"choices":[{"delta":{"content":"ho"},"finish_reason":null}]}"#,
            r#"{"choices":[{"delta":{"content":"la"},"finish_reason":null}]}"#,
            USO,
        ])
        .expect("acumula");
        assert_eq!(crudo.texto, "hola");
    }

    #[test]
    fn proveedores_deepseek_stream_sin_uso_es_cortada() {
        let error =
            acumula(&[r#"{"choices":[{"delta":{"content":"ok"},"finish_reason":"stop"}]}"#])
                .expect_err("sin uso no hay costo medido");
        assert!(matches!(error, ErrorProveedor::Cortada(_)), "{error:?}");
    }

    #[test]
    fn proveedores_deepseek_stream_length_es_truncada_aunque_venga_vacia() {
        let crudo = acumula(&[
            r#"{"choices":[{"delta":{"reasoning_content":"pensé"},"finish_reason":"length"}]}"#,
            r#"{"choices":[],"usage":{"prompt_tokens":100,"completion_tokens":32000}}"#,
        ])
        .expect("truncada no es error");
        assert!(crudo.truncada);
        assert_eq!(crudo.texto, "");
    }

    #[test]
    fn proveedores_deepseek_stream_error_en_un_evento_es_error_de_respuesta() {
        let error = acumula(&[r#"{"error":{"message":"cuota agotada"}}"#])
            .expect_err("un error en el stream es de respuesta");
        assert!(
            matches!(&error, ErrorProveedor::Respuesta(m) if m == "cuota agotada"),
            "{error:?}"
        );
    }
}
