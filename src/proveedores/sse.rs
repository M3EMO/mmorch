//! Parser SSE chico y puro: recibe bytes en trozos de red y entrega los eventos completos.

/// Un evento SSE completo: el campo `event` si vino, y los `data` unidos con saltos de línea.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct EventoSse {
    pub evento: Option<String>,
    pub datos: String,
}

/// Parser incremental. Guarda la línea incompleta y el evento en curso entre trozos.
#[derive(Debug, Default)]
pub(crate) struct Parser {
    /// Bytes de la línea que todavía no llegó entera.
    pendiente: Vec<u8>,
    /// Campo `event` del evento en curso.
    evento: Option<String>,
    /// Líneas `data` del evento en curso.
    datos: Vec<String>,
}

impl Parser {
    /// Recibe un trozo de bytes y devuelve los eventos que quedaron completos. Un evento sin línea
    /// vacía que lo cierre no está completo: se entrega cuando llega esa línea.
    pub(crate) fn alimentar(&mut self, bytes: &[u8]) -> Vec<EventoSse> {
        let mut bytes_pendientes = std::mem::take(&mut self.pendiente);
        bytes_pendientes.extend_from_slice(bytes);
        let mut eventos = Vec::new();
        let mut inicio = 0;
        while let Some(pos) = bytes_pendientes[inicio..].iter().position(|&b| b == b'\n') {
            let fin = inicio + pos;
            let mut linea = &bytes_pendientes[inicio..fin];
            linea = linea.strip_suffix(b"\r").unwrap_or(linea);
            if let Some(evento) = self.linea(&String::from_utf8_lossy(linea)) {
                eventos.push(evento);
            }
            inicio = fin + 1;
        }
        self.pendiente = bytes_pendientes[inicio..].to_vec();
        eventos
    }

    /// Procesa una línea completa, sin su fin de línea. Una línea vacía cierra el evento en curso.
    fn linea(&mut self, linea: &str) -> Option<EventoSse> {
        if linea.is_empty() {
            let evento = self.evento.take();
            let datos = std::mem::take(&mut self.datos);
            if datos.is_empty() {
                return None;
            }
            let datos = datos.join("\n");
            // `[DONE]` cierra el stream de DeepSeek: no es JSON y no trae nada que acumular.
            if datos == "[DONE]" {
                return None;
            }
            return Some(EventoSse { evento, datos });
        }
        if linea.starts_with(':') {
            return None;
        }
        let (campo, valor) = match linea.split_once(':') {
            Some((campo, valor)) => (campo, valor.strip_prefix(' ').unwrap_or(valor)),
            None => (linea, ""),
        };
        match campo {
            "event" => self.evento = Some(valor.into()),
            "data" => self.datos.push(valor.into()),
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(evento: Option<&str>, datos: &str) -> EventoSse {
        EventoSse {
            evento: evento.map(String::from),
            datos: datos.into(),
        }
    }

    /// Eventos de los trozos dados, en orden.
    fn todos(trozos: &[&[u8]]) -> Vec<EventoSse> {
        let mut parser = Parser::default();
        trozos.iter().flat_map(|t| parser.alimentar(t)).collect()
    }

    #[test]
    fn proveedores_sse_acepta_fin_de_linea_crlf() {
        let eventos = todos(&[b"data: uno\r\n\r\ndata: dos\r\n\r\n"]);
        assert_eq!(eventos, vec![ev(None, "uno"), ev(None, "dos")]);
    }

    #[test]
    fn proveedores_sse_ignora_los_comentarios() {
        let eventos = todos(&[b": ping\n\ndata: x\n\n"]);
        assert_eq!(eventos, vec![ev(None, "x")]);
    }

    #[test]
    fn proveedores_sse_descarta_done() {
        let eventos = todos(&[b"data: {\"a\":1}\n\ndata: [DONE]\n\n"]);
        assert_eq!(eventos, vec![ev(None, "{\"a\":1}")]);
    }

    #[test]
    fn proveedores_sse_junta_un_data_partido_en_dos_trozos_de_red() {
        // El corte cae en medio del dato y entre el \r y el \n de la línea vacía.
        let eventos = todos(&[b"data: {\"a\":", b"1}\r", b"\n\r\n"]);
        assert_eq!(eventos, vec![ev(None, "{\"a\":1}")]);
    }

    #[test]
    fn proveedores_sse_no_parte_un_caracter_utf8_entre_trozos() {
        let texto = "data: ñ\n\n".as_bytes();
        let eventos = todos(&[&texto[..7], &texto[7..]]);
        assert_eq!(eventos, vec![ev(None, "ñ")]);
    }

    #[test]
    fn proveedores_sse_lee_el_campo_event_y_une_las_lineas_data() {
        let eventos = todos(&[b"event: message_start\ndata: a\ndata: b\n\n"]);
        assert_eq!(eventos, vec![ev(Some("message_start"), "a\nb")]);
    }

    #[test]
    fn proveedores_sse_sin_linea_vacia_no_despacha() {
        // Un evento sin línea vacía que lo cierre no está completo: no se entrega.
        assert!(todos(&[b"data: x\n"]).is_empty());
    }
}
