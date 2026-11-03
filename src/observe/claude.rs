//! Convierte la salida de `claude -p --output-format json` en eventos `Sesion`.

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use serde_json::{Map, Value, json};

use crate::observe::jsonl;
use crate::observe::uso::{self, iso};
use crate::observe::{ErrorObserve, evento_gate};
use crate::tipos::{Evento, TipoEvento, Via};

/// Fase del `Gate` de una sesión importada sin costo.
pub const FASE_COSTO_DESCONOCIDO: &str = "costo-desconocido";

/// Lee la salida de `claude -p` y da, por objeto JSON, una `Llamada` por modelo de `modelUsage`
/// y un `Sesion`. La hora de los eventos es el inicio de la sesión: el mtime del archivo menos
/// `duration_ms`. Un costo negativo es error y no da eventos.
/// El costo no se cuenta dos veces (B12):
/// - Si cada modelo trae `costUSD`, cada `Llamada` lleva el suyo y el `Sesion` lleva 0.
/// - Si falta alguno y hay `total_cost_usd`, las `Llamada` llevan 0 y el `Sesion` lleva el total.
/// - Si falta alguno y no hay total, cada `Llamada` lleva su `costUSD` y el `Sesion` marca
///   `extra.costo_desconocido`.
///
/// Sin `modelUsage`, el `Sesion` lleva el `total_cost_usd`.
/// Acepta un objeto con varias líneas o un objeto por línea.
pub fn leer_sesion_claude(json: &Path, corrida: &str) -> Result<Vec<Evento>, ErrorObserve> {
    let texto = fs::read_to_string(json)?;
    let objetos = serde_json::Deserializer::from_str(&texto)
        .into_iter::<Value>()
        .collect::<Result<Vec<Value>, serde_json::Error>>()?;
    let objetos: Vec<&Value> = objetos.iter().filter(|objeto| objeto.is_object()).collect();
    for objeto in &objetos {
        validar_costos(objeto)?;
    }
    let fin = fin_de_archivo(json)?;
    Ok(objetos
        .into_iter()
        .flat_map(|objeto| eventos_desde(objeto, corrida, fin))
        .collect())
}

/// Un `Gate` `costo-desconocido` por cada `Sesion` que marca `costo_desconocido`. El tope no puede
/// sumar esa sesión, así que el `Gate` lo dice en la misma corrida y con el id de la sesión.
pub fn gates_de_costo_desconocido(eventos: &[Evento]) -> Vec<Evento> {
    eventos
        .iter()
        .filter(|ev| {
            ev.tipo == TipoEvento::Sesion
                && ev.extra.get("costo_desconocido").and_then(Value::as_bool) == Some(true)
        })
        .map(|ev| {
            let extra = Map::from_iter([
                (
                    "motivo".to_string(),
                    json!("sesión sin costo: el tope no la puede sumar"),
                ),
                (
                    "sesion".to_string(),
                    ev.extra.get("session_id").cloned().unwrap_or(Value::Null),
                ),
            ]);
            evento_gate(&ev.corrida, FASE_COSTO_DESCONOCIDO, extra)
        })
        .collect()
}

/// Lo que `registrar_sesion_claude` dejó en el log.
#[derive(Debug, PartialEq)]
pub struct Registro {
    /// Eventos `Llamada` y `Sesion` agregados al log.
    pub eventos: usize,
    /// `session_id` de las sesiones que el log ya tenía. No se registraron de nuevo.
    pub ya_registradas: Vec<String>,
}

/// Registra en `ruta_log` la salida de `claude -p` de `archivo`: eventos y gates de costo.
/// Una sesión cuyo `session_id` ya está en el log no se registra de nuevo. Leer, decidir y
/// escribir pasan bajo un solo lock, así dos corridas a la vez no la cuentan dos veces.
pub fn registrar_sesion_claude(
    archivo: &Path,
    corrida: &str,
    via: Option<Via>,
    ruta_log: &Path,
) -> Result<Registro, ErrorObserve> {
    let eventos = leer_sesion_claude(archivo, corrida)?;
    let eventos = match via {
        Some(via) => uso::con_via(eventos, via),
        None => eventos,
    };
    uso::con_eventos_bloqueados(ruta_log, |registrados, log| {
        let ya = ids_registrados(registrados);
        let (nuevos, ya_registradas) = sin_sesiones_ya_registradas(eventos, &ya);
        let gates = gates_de_costo_desconocido(&nuevos);
        for ev in nuevos.iter().chain(&gates) {
            let linea = serde_json::to_string(ev)?;
            log.agregar(&linea, jsonl::UMBRAL_POR_DEFECTO)?;
        }
        Ok(Registro {
            eventos: nuevos.len(),
            ya_registradas,
        })
    })
}

/// `session_id` que ya tiene el log: el de cada `Sesion` registrado.
fn ids_registrados(registrados: &[Evento]) -> HashSet<String> {
    registrados
        .iter()
        .filter(|ev| ev.tipo == TipoEvento::Sesion)
        .filter_map(id_de_sesion)
        .map(str::to_string)
        .collect()
}

/// `session_id` de un evento `Sesion`, si lo trae.
fn id_de_sesion(ev: &Evento) -> Option<&str> {
    ev.extra.get("session_id").and_then(Value::as_str)
}

/// Quita los grupos de `eventos` cuyo `session_id` está en `ya`. Un grupo son los eventos de un
/// objeto JSON: sus `Llamada` y, al final, su `Sesion`. Da los grupos nuevos y los ids saltados.
fn sin_sesiones_ya_registradas(
    eventos: Vec<Evento>,
    ya: &HashSet<String>,
) -> (Vec<Evento>, Vec<String>) {
    let mut nuevos = Vec::new();
    let mut saltadas = Vec::new();
    let mut grupo = Vec::new();
    for ev in eventos {
        let cierra = ev.tipo == TipoEvento::Sesion;
        let id = id_de_sesion(&ev).map(str::to_string);
        grupo.push(ev);
        if !cierra {
            continue;
        }
        match id {
            Some(id) if ya.contains(&id) => {
                saltadas.push(id);
                grupo.clear();
            }
            _ => nuevos.append(&mut grupo),
        }
    }
    nuevos.append(&mut grupo);
    (nuevos, saltadas)
}

/// Hora en que terminó la sesión: el mtime del archivo, en epoch segundos.
fn fin_de_archivo(json: &Path) -> Result<f64, ErrorObserve> {
    let mtime = fs::metadata(json)?.modified()?;
    Ok(mtime
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64()))
}

/// Un costo negativo, en `costUSD` o en `total_cost_usd`, es error.
fn validar_costos(salida: &Value) -> Result<(), ErrorObserve> {
    if total_de(salida).is_some_and(|total| total < 0.0) {
        return Err(ErrorObserve::CostoNegativo("total_cost_usd".into()));
    }
    for (modelo, uso) in modelos_de(salida).into_iter().flatten() {
        if costo_de(uso).is_some_and(|costo| costo < 0.0) {
            return Err(ErrorObserve::CostoNegativo(format!(
                "modelUsage.{modelo}.costUSD"
            )));
        }
    }
    Ok(())
}

/// `modelUsage` si trae al menos un modelo.
fn modelos_de(salida: &Value) -> Option<&Map<String, Value>> {
    salida
        .get("modelUsage")
        .and_then(Value::as_object)
        .filter(|modelos| !modelos.is_empty())
}

/// `costUSD` de un modelo de `modelUsage`, si es un número.
fn costo_de(uso: &Value) -> Option<f64> {
    uso.get("costUSD").and_then(Value::as_f64)
}

/// `total_cost_usd` de la salida, si es un número.
fn total_de(salida: &Value) -> Option<f64> {
    salida.get("total_cost_usd").and_then(Value::as_f64)
}

/// `true` si todos los modelos de `modelUsage` traen `costUSD`.
fn costos_completos(modelos: Option<&Map<String, Value>>) -> bool {
    modelos.is_some_and(|modelos| modelos.values().all(|uso| costo_de(uso).is_some()))
}

/// Duración de la sesión en segundos, de `duration_ms`. Ausente o negativa vale 0.
fn duracion_s(salida: &Value) -> f64 {
    salida
        .get("duration_ms")
        .and_then(Value::as_f64)
        .map_or(0.0, |ms| (ms / 1000.0).max(0.0))
}

fn eventos_desde(salida: &Value, corrida: &str, fin: f64) -> Vec<Evento> {
    let ts = fin - duracion_s(salida);
    let iso = iso(ts, 0);
    let modelos = modelos_de(salida);
    // Las `Llamada` llevan su costo, salvo que el total de la sesión lo cubra.
    let costo_en_llamadas = costos_completos(modelos) || total_de(salida).is_none();
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
            cost_usd: if costo_en_llamadas {
                costo_de(uso).unwrap_or(0.0)
            } else {
                0.0
            },
            ..Evento::default()
        })
        .collect();
    eventos.push(evento_sesion(salida, corrida, (ts, iso)));
    eventos
}

/// Evento `Sesion` con turnos y duración. Sin `modelUsage`, lleva el costo y los tokens del total.
fn evento_sesion(salida: &Value, corrida: &str, (ts, iso): (f64, String)) -> Evento {
    let mut extra = Map::new();
    for clave in ["num_turns", "duration_ms", "session_id", "stop_reason"] {
        if let Some(valor) = salida.get(clave) {
            extra.insert(clave.into(), valor.clone());
        }
    }
    if let Some(valor) = salida.get("total_cost_usd") {
        extra.insert("total_cost_usd".into(), valor.clone());
    }
    let modelos = modelos_de(salida);
    let con_modelos = modelos.is_some();
    let (in_tokens, out_tokens) = if con_modelos {
        (0, 0)
    } else {
        let entrada = entero(salida, &["usage", "input_tokens"])
            + entero(salida, &["usage", "cache_creation_input_tokens"])
            + entero(salida, &["usage", "cache_read_input_tokens"]);
        (entrada, entero(salida, &["usage", "output_tokens"]))
    };
    let (cost_usd, desconocido) = costo_de_sesion(salida, costos_completos(modelos), con_modelos);
    if desconocido {
        extra.insert("costo_desconocido".into(), json!(true));
    }
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

/// Costo del `Sesion` y si es desconocido. Sin `modelUsage`, es el total. Con él, el `Sesion` solo
/// lleva costo cuando las `Llamada` no lo cubren y el total sí: así no se cuenta dos veces.
fn costo_de_sesion(salida: &Value, completos: bool, con_modelos: bool) -> (f64, bool) {
    match (con_modelos, completos, total_de(salida)) {
        (false, _, total) => (total.unwrap_or(0.0), false),
        (true, true, _) => (0.0, false),
        (true, false, Some(total)) => (total, false),
        (true, false, None) => (0.0, true),
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

    /// 2026-10-31T12:00:00Z.
    const MTIME_OCTUBRE: i64 = 1_793_448_000;

    fn sesion_con_mtime(contenido: &str, mtime: i64) -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().expect("temp dir");
        let ruta = dir.path().join("salida.json");
        fs::write(&ruta, contenido).expect("escribe");
        filetime::set_file_mtime(&ruta, filetime::FileTime::from_unix_time(mtime, 0))
            .expect("fija el mtime");
        (dir, ruta)
    }

    #[test]
    fn observe_sesion_importada_cuenta_en_la_hora_de_su_inicio() {
        // Una hora de sesión que termina a las 12:00Z: arranca a las 11:00Z de octubre.
        let (_dir, ruta) = sesion_con_mtime(r#"{"duration_ms": 3600000}"#, MTIME_OCTUBRE);
        let eventos = leer_sesion_claude(&ruta, "c1").expect("lee");
        assert_eq!(eventos[0].ts, 1_793_444_400.0);
        assert_eq!(eventos[0].iso, "2026-10-31T11:00:00Z");
    }

    #[test]
    fn observe_sesion_sin_duration_ms_usa_el_mtime() {
        let (_dir, ruta) = sesion_con_mtime("{}\n", MTIME_OCTUBRE);
        let eventos = leer_sesion_claude(&ruta, "c1").expect("lee");
        assert_eq!(eventos[0].ts, MTIME_OCTUBRE as f64);
    }

    #[test]
    fn observe_modelo_sin_costo_usd_con_total_no_suma_cero_ni_cuenta_dos_veces() {
        let (_dir, ruta) = sesion_con_mtime(
            r#"{"total_cost_usd": 0.7, "modelUsage": {"a": {"inputTokens": 10, "outputTokens": 5}, "b": {"inputTokens": 1, "outputTokens": 1, "costUSD": 0.25}}}"#,
            MTIME_OCTUBRE,
        );
        let eventos = leer_sesion_claude(&ruta, "c1").expect("lee");
        let total: f64 = eventos.iter().map(|ev| ev.cost_usd).sum();
        assert!((total - 0.7).abs() < 1e-12, "total {total}");
        let sesion = eventos
            .iter()
            .find(|ev| ev.tipo == TipoEvento::Sesion)
            .expect("sesión");
        assert_eq!(sesion.cost_usd, 0.7);
        assert!(!sesion.extra.contains_key("costo_desconocido"));
    }

    #[test]
    fn observe_modelo_sin_costo_usd_ni_total_marca_costo_desconocido() {
        let (_dir, ruta) = sesion_con_mtime(
            r#"{"modelUsage": {"a": {"inputTokens": 10}, "b": {"costUSD": 0.25}}}"#,
            MTIME_OCTUBRE,
        );
        let eventos = leer_sesion_claude(&ruta, "c1").expect("lee");
        let sesion = eventos
            .iter()
            .find(|ev| ev.tipo == TipoEvento::Sesion)
            .expect("sesión");
        assert_eq!(sesion.extra["costo_desconocido"], true);
        let total: f64 = eventos.iter().map(|ev| ev.cost_usd).sum();
        assert_eq!(total, 0.25, "el costo conocido se conserva");
    }

    #[test]
    fn observe_costo_usd_negativo_es_error_y_no_da_eventos() {
        let (_dir, ruta) =
            sesion_con_mtime(r#"{"modelUsage": {"a": {"costUSD": -0.5}}}"#, MTIME_OCTUBRE);
        assert!(leer_sesion_claude(&ruta, "c1").is_err());
    }

    #[test]
    fn observe_total_cost_usd_negativo_es_error_con_o_sin_modelos() {
        for contenido in [
            r#"{"total_cost_usd": -1.0}"#,
            r#"{"total_cost_usd": -1.0, "modelUsage": {"a": {"costUSD": 0.5}}}"#,
        ] {
            let (_dir, ruta) = sesion_con_mtime(contenido, MTIME_OCTUBRE);
            assert!(leer_sesion_claude(&ruta, "c1").is_err(), "{contenido}");
        }
    }

    #[test]
    fn observe_registrar_dos_veces_la_misma_salida_no_duplica_el_costo() {
        let log = TempDir::new().expect("temp dir");
        let ruta_log = crate::observe::ruta_eventos_en(log.path());
        let (_dir, archivo) = sesion_con_mtime(
            r#"{"total_cost_usd": 0.7, "session_id": "s-1", "modelUsage": {"a": {"costUSD": 0.5}, "b": {"costUSD": 0.2}}}"#,
            MTIME_OCTUBRE,
        );
        let primera = registrar_sesion_claude(&archivo, "c1", None, &ruta_log).expect("primera");
        let segunda = registrar_sesion_claude(&archivo, "c1", None, &ruta_log).expect("segunda");
        assert_eq!(primera.eventos, 3, "dos Llamada y un Sesion");
        assert_eq!(segunda.eventos, 0, "la sesión ya estaba en el log");
        assert_eq!(segunda.ya_registradas, vec!["s-1".to_string()]);
        let registrados = uso::leer_eventos(&ruta_log).expect("lee el log");
        assert_eq!(registrados.len(), 3);
        let total: f64 = registrados.iter().map(|ev| ev.cost_usd).sum();
        assert!((total - 0.7).abs() < 1e-12, "total {total}");
    }

    #[test]
    fn observe_sesion_ya_registrada_se_salta_y_la_nueva_entra() {
        let log = TempDir::new().expect("temp dir");
        let ruta_log = crate::observe::ruta_eventos_en(log.path());
        let (_d1, primera) = sesion_con_mtime(
            r#"{"session_id": "s-1", "modelUsage": {"a": {"costUSD": 0.5}}}"#,
            MTIME_OCTUBRE,
        );
        registrar_sesion_claude(&primera, "c1", None, &ruta_log).expect("primera");
        // Un archivo con dos sesiones: s-1 ya está, s-2 es nueva.
        let (_d2, mezcla) = sesion_con_mtime(
            "{\"session_id\": \"s-1\", \"modelUsage\": {\"a\": {\"costUSD\": 0.5}}}\n{\"session_id\": \"s-2\", \"modelUsage\": {\"a\": {\"costUSD\": 0.2}}}\n",
            MTIME_OCTUBRE,
        );
        let segunda = registrar_sesion_claude(&mezcla, "c1", None, &ruta_log).expect("segunda");
        assert_eq!(segunda.eventos, 2, "solo la Llamada y el Sesion de s-2");
        assert_eq!(segunda.ya_registradas, vec!["s-1".to_string()]);
        let total: f64 = uso::leer_eventos(&ruta_log)
            .expect("lee el log")
            .iter()
            .map(|ev| ev.cost_usd)
            .sum();
        assert!((total - 0.7).abs() < 1e-12, "total {total}");
    }

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
