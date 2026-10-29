//! Anthropic: Messages API en `POST {url}/v1/messages`, con `"stream": true`.

use serde_json::{Value, json};

use super::{Clave, Crudo, ErrorProveedor, Peticion, Prompt, mensaje_de_error};

const VERSION_API: &str = "2023-06-01";

/// Arma el pedido con las cabeceras de la API de Messages. Con razonamiento, pide `adaptive`:
/// `budget_tokens` da 400 en Haiku, Sonnet y Opus 5.5. Sin razonamiento omite `thinking`, porque
/// `disabled` da 400 en Sonnet y Opus 5.5.
pub(crate) fn armar(
    url_base: &str,
    clave: &Clave,
    modelo: &str,
    prompt: &Prompt,
    razonamiento: bool,
) -> Peticion {
    let mut cuerpo = json!({
        "model": modelo,
        "max_tokens": prompt.max_tokens,
        "messages": [{"role": "user", "content": prompt.usuario}],
        "stream": true,
    });
    if let Some(sistema) = &prompt.sistema {
        cuerpo["system"] = json!(sistema);
    }
    if razonamiento {
        cuerpo["thinking"] = json!({"type": "adaptive"});
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

/// Acumula los eventos de un stream de Anthropic hasta el `Crudo` de la llamada. Une los
/// `text_delta` e ignora `thinking_delta` y `signature_delta`. Sin el `usage` final, el stream
/// cuenta como cortado.
#[derive(Debug, Default)]
pub(crate) struct Acumulador {
    texto: String,
    stop: Option<String>,
    in_tokens: Option<u64>,
    out_tokens: Option<u64>,
}

impl Acumulador {
    /// Procesa un evento: su campo `event` si vino, o el `type` del JSON.
    pub(crate) fn consumir(
        &mut self,
        evento: Option<&str>,
        datos: &str,
    ) -> Result<(), ErrorProveedor> {
        let json: Value =
            serde_json::from_str(datos).map_err(|e| ErrorProveedor::Respuesta(e.to_string()))?;
        let tipo = evento
            .or_else(|| json.get("type").and_then(Value::as_str))
            .unwrap_or("");
        match tipo {
            "message_start" => {
                if let Some(n) = json
                    .pointer("/message/usage/input_tokens")
                    .and_then(Value::as_u64)
                {
                    self.in_tokens = Some(n);
                }
            }
            "content_block_delta" => {
                let es_texto =
                    json.pointer("/delta/type").and_then(Value::as_str) == Some("text_delta");
                if let Some(texto) = json.pointer("/delta/text").and_then(Value::as_str)
                    && es_texto
                {
                    self.texto.push_str(texto);
                }
            }
            "message_delta" => {
                if let Some(stop) = json.pointer("/delta/stop_reason").and_then(Value::as_str) {
                    self.stop = Some(stop.into());
                }
                if let Some(n) = json.pointer("/usage/output_tokens").and_then(Value::as_u64) {
                    self.out_tokens = Some(n);
                }
            }
            "error" => {
                let error = json.get("error").unwrap_or(&Value::Null);
                return Err(ErrorProveedor::Respuesta(mensaje_de_error(error)));
            }
            _ => {}
        }
        Ok(())
    }

    /// Cierra el stream. Con `stop_reason` = `max_tokens` el tope cortó la respuesta: es truncada.
    /// Sin el `usage` del `message_delta`, el costo no se midió: la llamada cuenta como cortada.
    pub(crate) fn cerrar(self) -> Result<Crudo, ErrorProveedor> {
        let Some(out_tokens) = self.out_tokens else {
            return Err(ErrorProveedor::Cortada(
                "el stream terminó sin el evento de uso".into(),
            ));
        };
        let in_tokens = self
            .in_tokens
            .ok_or_else(|| ErrorProveedor::Respuesta("falta /message/usage/input_tokens".into()))?;
        Ok(Crudo {
            texto: self.texto,
            in_tokens,
            out_tokens,
            truncada: self.stop.as_deref() == Some("max_tokens"),
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
    fn acumula(eventos: &[(&str, String)]) -> Result<Crudo, ErrorProveedor> {
        let mut acumulador = Acumulador::default();
        for (evento, datos) in eventos {
            acumulador.consumir(Some(evento), datos)?;
        }
        acumulador.cerrar()
    }

    /// Evento de inicio, con el uso de entrada.
    fn inicio() -> (&'static str, String) {
        (
            "message_start",
            r#"{"type":"message_start","message":{"usage":{"input_tokens":10,"output_tokens":1}}}"#
                .into(),
        )
    }

    /// Evento final con su `stop_reason` y el uso de salida.
    fn fin(stop: &str, salida: u64) -> (&'static str, String) {
        (
            "message_delta",
            format!(
                r#"{{"type":"message_delta","delta":{{"stop_reason":"{stop}"}},"usage":{{"output_tokens":{salida}}}}}"#
            ),
        )
    }

    /// Un bloque de texto con un solo `text_delta`.
    fn bloque_de_texto(indice: usize, texto: &str) -> Vec<(&'static str, String)> {
        vec![
            (
                "content_block_start",
                format!(
                    r#"{{"type":"content_block_start","index":{indice},"content_block":{{"type":"text","text":""}}}}"#
                ),
            ),
            (
                "content_block_delta",
                format!(
                    r#"{{"type":"content_block_delta","index":{indice},"delta":{{"type":"text_delta","text":"{texto}"}}}}"#
                ),
            ),
        ]
    }

    #[test]
    fn proveedores_anthropic_cabeceras_llevan_clave_y_version() {
        let pet = armar(
            "http://x/",
            &Clave::nueva("k1"),
            "claude-haiku-5-5",
            &prompt(None),
            false,
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
        let pet = armar("http://x", &Clave::nueva("k"), "m", &prompt(None), false);
        let cuerpo: Value = serde_json::from_str(&pet.cuerpo).expect("JSON");
        assert!(cuerpo.get("system").is_none());
        assert_eq!(cuerpo["messages"][0]["role"], "user");
    }

    #[test]
    fn proveedores_anthropic_con_razonamiento_pide_adaptive_sin_budget_tokens() {
        let pet = armar(
            "http://x",
            &Clave::nueva("k"),
            "claude-haiku-5-5",
            &prompt(None),
            true,
        );
        let cuerpo: Value = serde_json::from_str(&pet.cuerpo).expect("JSON");
        assert_eq!(cuerpo["thinking"], json!({"type": "adaptive"}));
        assert!(!pet.cuerpo.contains("budget_tokens"), "{}", pet.cuerpo);
        assert!(cuerpo.get("output_config").is_none());
    }

    #[test]
    fn proveedores_anthropic_sin_razonamiento_omite_thinking() {
        let pet = armar(
            "http://x",
            &Clave::nueva("k"),
            "claude-haiku-5-5",
            &prompt(None),
            false,
        );
        let cuerpo: Value = serde_json::from_str(&pet.cuerpo).expect("JSON");
        assert!(cuerpo.get("thinking").is_none(), "{}", pet.cuerpo);
    }

    #[test]
    fn proveedores_anthropic_cuerpo_pide_stream() {
        let pet = armar("http://x", &Clave::nueva("k"), "m", &prompt(None), false);
        let cuerpo: Value = serde_json::from_str(&pet.cuerpo).expect("JSON");
        assert_eq!(cuerpo["stream"], true);
    }

    #[test]
    fn proveedores_anthropic_stream_une_bloques_de_texto_e_ignora_el_razonamiento() {
        let mut eventos = vec![inicio()];
        eventos.push((
            "content_block_delta",
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"pensé"}}"#.into(),
        ));
        eventos.push((
            "content_block_delta",
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"firma"}}"#.into(),
        ));
        eventos.extend(bloque_de_texto(1, "ho"));
        eventos.extend(bloque_de_texto(2, "la"));
        eventos.push(fin("end_turn", 5));
        let crudo = acumula(&eventos).expect("acumula");
        assert_eq!(crudo.texto, "hola");
        assert_eq!((crudo.in_tokens, crudo.out_tokens), (10, 5));
        assert!(!crudo.truncada);
    }

    #[test]
    fn proveedores_anthropic_stream_max_tokens_es_truncada() {
        let mut eventos = vec![inicio()];
        eventos.extend(bloque_de_texto(1, "ho"));
        eventos.push(fin("max_tokens", 5));
        let crudo = acumula(&eventos).expect("truncada no es error");
        assert!(crudo.truncada);
        assert_eq!(crudo.texto, "ho");
    }

    #[test]
    fn proveedores_anthropic_stream_sin_uso_final_es_cortada() {
        let error = acumula(&[
            inicio(),
            (
                "message_delta",
                r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"}}"#.into(),
            ),
        ])
        .expect_err("sin uso final no hay costo medido");
        assert!(matches!(error, ErrorProveedor::Cortada(_)), "{error:?}");
    }

    #[test]
    fn proveedores_anthropic_stream_evento_error_es_error_de_respuesta() {
        let error = acumula(&[(
            "error",
            r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#.into(),
        )])
        .expect_err("un evento error es de respuesta");
        assert!(
            matches!(&error, ErrorProveedor::Respuesta(m) if m == "Overloaded"),
            "{error:?}"
        );
    }
}
