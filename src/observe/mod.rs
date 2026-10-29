//! Observe: registra eventos de cada corrida y calcula su resumen.

pub mod claude;
pub mod jsonl;
pub mod uso;

pub use uso::uso;

use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;
use thiserror::Error as ErrorDerive;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::config::root_home;
use crate::tipos::{Evento, Resumen, TipoEvento, Via};

/// Errores de Observe. Salen con código 3.
#[derive(Debug, ErrorDerive)]
pub enum ErrorObserve {
    #[error("error de E/S")]
    Io(#[from] io::Error),
    #[error("error de JSON")]
    Json(#[from] serde_json::Error),
}

/// Ruta de `eventos.jsonl` dentro de `ROOT_HOME`.
pub fn ruta_eventos() -> PathBuf {
    root_home().join("eventos.jsonl")
}

/// Agrega un evento a `eventos.jsonl` de `ROOT_HOME`.
pub fn registrar(ev: &Evento) -> io::Result<()> {
    registrar_en(&ruta_eventos(), ev)
}

/// Agrega un evento al archivo indicado, con lock y rotación.
pub fn registrar_en(ruta: &Path, ev: &Evento) -> io::Result<()> {
    let linea = serde_json::to_string(ev).map_err(io::Error::other)?;
    jsonl::agregar(ruta, &linea, jsonl::UMBRAL_POR_DEFECTO)
}

/// Resume la corrida indicada de `eventos.jsonl` de `ROOT_HOME`.
pub fn resumir(corrida: &str) -> Result<Resumen, ErrorObserve> {
    resumir_en(&ruta_eventos(), corrida)
}

/// Resume la corrida indicada del archivo dado. Si el archivo no existe, da un resumen vacío.
pub fn resumir_en(ruta: &Path, corrida: &str) -> Result<Resumen, ErrorObserve> {
    let de_la_corrida: Vec<Evento> = uso::leer_eventos(ruta)?
        .into_iter()
        .filter(|ev| ev.corrida == corrida)
        .collect();
    Ok(resumir_eventos(&de_la_corrida))
}

/// Resumen de una lista de eventos de una corrida. Función pura.
pub fn resumir_eventos(eventos: &[Evento]) -> Resumen {
    let plan_aprobado = eventos
        .iter()
        .find(|ev| ev.tipo == TipoEvento::PlanAprobado)
        .map(|ev| ev.ts);
    let mut resumen = Resumen::default();
    for ev in eventos {
        match ev.via {
            Some(Via::Api) if ev.tipo != TipoEvento::Reserva => resumen.usd_api += ev.cost_usd,
            Some(Via::Cupo) => resumen.usd_cupo += ev.cost_usd,
            _ => {}
        }
        // Solo el Sesion y las llamadas por API suman minutos; las de cupo ya están en el Sesion.
        let suma_minutos = ev.tipo == TipoEvento::Sesion
            || (ev.tipo == TipoEvento::Llamada && ev.via == Some(Via::Api));
        if suma_minutos {
            resumen.minutos += ev.latency_s / 60.0;
        }
        if ev.tipo == TipoEvento::Sesion {
            resumen.turnos = resumen.turnos.saturating_add(turnos_de(ev));
        }
        if ev.tipo == TipoEvento::Humano && plan_aprobado.is_some_and(|t| ev.ts > t) {
            resumen.intervenciones += 1;
        }
        resumen.invalida |= ev.tipo == TipoEvento::OraculoLeido;
    }
    resumen
}

/// Turnos de un evento `Sesion`, leídos de `extra.num_turns`. Ausente vale 0.
fn turnos_de(ev: &Evento) -> u32 {
    ev.extra
        .get("num_turns")
        .and_then(Value::as_u64)
        .map_or(0, |n| u32::try_from(n).unwrap_or(u32::MAX))
}

/// Evento escrito a mano por el usuario o por un paso de Root.
pub fn evento_manual(corrida: &str, tipo: TipoEvento, detalle: Option<&str>) -> Evento {
    let (ts, iso) = ahora();
    let mut extra = serde_json::Map::new();
    if let Some(texto) = detalle {
        extra.insert("detalle".into(), Value::String(texto.into()));
    }
    Evento {
        ts,
        iso,
        corrida: corrida.into(),
        tipo,
        extra,
        ..Evento::default()
    }
}

/// Registra un evento manual en `eventos.jsonl` de `ROOT_HOME`. La usan `observa evento` y `root_registrar`.
pub fn registrar_manual(
    corrida: &str,
    tipo: TipoEvento,
    detalle: Option<&str>,
) -> Result<(), ErrorObserve> {
    registrar(&evento_manual(corrida, tipo, detalle)).map_err(ErrorObserve::from)
}

/// Evento `Gate`: un tope o un bloqueo que Root decidió. Lleva la fase y los datos en `extra`.
pub fn evento_gate(corrida: &str, fase: &str, extra: serde_json::Map<String, Value>) -> Evento {
    let mut ev = evento_manual(corrida, TipoEvento::Gate, None);
    ev.fase = fase.into();
    ev.extra = extra;
    ev
}

/// Hora actual: segundos desde la época con decimales, y texto RFC 3339 en UTC.
pub(crate) fn ahora() -> (f64, String) {
    let ahora = OffsetDateTime::now_utc();
    let ts = ahora.unix_timestamp_nanos() as f64 / 1e9;
    let iso = ahora
        .format(&Rfc3339)
        .expect("RFC 3339 formatea cualquier fecha válida");
    (ts, iso)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn evento(tipo: TipoEvento, ts: f64) -> Evento {
        Evento {
            ts,
            corrida: "c1".into(),
            tipo,
            ..Evento::default()
        }
    }

    fn con_costo(via: Via, cost_usd: f64, latency_s: f64) -> Evento {
        Evento {
            tipo: TipoEvento::Llamada,
            via: Some(via),
            cost_usd,
            latency_s,
            corrida: "c1".into(),
            ..Evento::default()
        }
    }

    #[test]
    fn observe_dos_humanos_despues_de_plan_dan_dos_intervenciones() {
        let eventos = [
            evento(TipoEvento::PlanAprobado, 100.0),
            evento(TipoEvento::Humano, 101.0),
            evento(TipoEvento::Humano, 102.0),
        ];
        assert_eq!(resumir_eventos(&eventos).intervenciones, 2);
    }

    #[test]
    fn observe_humano_antes_de_plan_no_cuenta() {
        let eventos = [
            evento(TipoEvento::Humano, 99.0),
            evento(TipoEvento::PlanAprobado, 100.0),
        ];
        assert_eq!(resumir_eventos(&eventos).intervenciones, 0);
    }

    #[test]
    fn observe_humanos_sin_plan_aprobado_dan_cero() {
        let eventos = [
            evento(TipoEvento::Humano, 1.0),
            evento(TipoEvento::Humano, 2.0),
        ];
        assert_eq!(resumir_eventos(&eventos).intervenciones, 0);
    }

    #[test]
    fn observe_oraculo_leido_marca_invalida() {
        assert!(resumir_eventos(&[evento(TipoEvento::OraculoLeido, 1.0)]).invalida);
        assert!(!resumir_eventos(&[evento(TipoEvento::Gate, 1.0)]).invalida);
    }

    #[test]
    fn observe_suma_usd_por_via_y_minutos() {
        let eventos = [
            con_costo(Via::Api, 0.5, 30.0),
            con_costo(Via::Cupo, 1.25, 90.0),
            con_costo(Via::Oraculo, 9.0, 0.0),
        ];
        let resumen = resumir_eventos(&eventos);
        assert_eq!(resumen.usd_api, 0.5);
        assert_eq!(resumen.usd_cupo, 1.25);
        // El cupo no suma minutos: su tiempo ya está en el evento Sesion.
        assert_eq!(resumen.minutos, 0.5);
    }

    #[test]
    fn observe_minutos_no_cuentan_dos_veces_la_llamada_de_cupo() {
        let sesion = Evento {
            tipo: TipoEvento::Sesion,
            corrida: "c1".into(),
            latency_s: 60.0,
            ..Evento::default()
        };
        let llamada_cupo = Evento {
            latency_s: 60.0,
            ..con_costo(Via::Cupo, 0.0, 0.0)
        };
        let resumen = resumir_eventos(&[sesion, llamada_cupo]);
        assert_eq!(resumen.minutos, 1.0);
    }

    #[test]
    fn observe_suma_turnos_solo_de_eventos_sesion() {
        let sesion = |turnos: u64| Evento {
            tipo: TipoEvento::Sesion,
            corrida: "c1".into(),
            extra: serde_json::Map::from_iter([("num_turns".to_string(), json!(turnos))]),
            ..Evento::default()
        };
        let llamada = Evento {
            tipo: TipoEvento::Llamada,
            corrida: "c1".into(),
            extra: serde_json::Map::from_iter([("num_turns".to_string(), json!(100))]),
            ..Evento::default()
        };
        let resumen = resumir_eventos(&[sesion(8), sesion(3), llamada]);
        assert_eq!(resumen.turnos, 11);
    }

    #[test]
    fn observe_resumir_en_ruta_inexistente_da_resumen_vacio() {
        let dir = TempDir::new().expect("temp dir");
        let resumen =
            resumir_en(&dir.path().join("no-existe.jsonl"), "c1").expect("sin archivo no es error");
        assert_eq!(resumen, Resumen::default());
    }

    #[test]
    fn observe_resumir_cuenta_intervenciones_escritas_antes_de_rotar() {
        let dir = TempDir::new().expect("temp dir");
        let ruta = dir.path().join("eventos.jsonl");
        // Umbral de 100 bytes: cada evento supera el umbral, así que cada escritura rota.
        let escribe = |ev: &Evento| {
            let linea = serde_json::to_string(ev).expect("serializa");
            jsonl::agregar(&ruta, &linea, 100).expect("escribe");
        };
        escribe(&evento(TipoEvento::PlanAprobado, 100.0));
        escribe(&evento(TipoEvento::Humano, 101.0));
        escribe(&evento(TipoEvento::Humano, 102.0));
        assert!(dir.path().join("eventos.jsonl.1").exists());
        let resumen = resumir_en(&ruta, "c1").expect("resume");
        assert_eq!(resumen.intervenciones, 2);
    }

    #[test]
    fn observe_resumir_en_filtra_por_corrida() {
        let dir = TempDir::new().expect("temp dir");
        let ruta = dir.path().join("eventos.jsonl");
        let mut api_c2 = con_costo(Via::Api, 7.0, 0.0);
        api_c2.corrida = "c2".into();
        registrar_en(&ruta, &con_costo(Via::Api, 0.5, 0.0)).expect("registra");
        registrar_en(&ruta, &api_c2).expect("registra");
        let resumen = resumir_en(&ruta, "c1").expect("resume");
        assert_eq!(resumen.usd_api, 0.5);
    }

    #[test]
    fn observe_resumir_no_suma_reservas_a_usd_api() {
        let reserva = Evento {
            tipo: TipoEvento::Reserva,
            via: Some(Via::Api),
            cost_usd: 0.9,
            corrida: "c1".into(),
            ..Evento::default()
        };
        let resumen = resumir_eventos(&[reserva, con_costo(Via::Api, 0.5, 0.0)]);
        assert_eq!(resumen.usd_api, 0.5);
    }

    #[test]
    fn observe_evento_manual_guarda_detalle() {
        let ev = evento_manual("c1", TipoEvento::Humano, Some("ajusta el plan"));
        assert_eq!(ev.corrida, "c1");
        assert_eq!(ev.tipo, TipoEvento::Humano);
        assert_eq!(ev.extra["detalle"], "ajusta el plan");
        assert!(ev.ts > 0.0);
        assert!(!ev.iso.is_empty());
    }

    #[test]
    fn observe_evento_manual_sin_detalle_no_tiene_extra() {
        let ev = evento_manual("c1", TipoEvento::Gate, None);
        assert!(ev.extra.is_empty());
    }
}
