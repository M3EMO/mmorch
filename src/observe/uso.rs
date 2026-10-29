//! Uso: cuánto gastó Root en cada vía, y cuánto margen queda en cada tope.
//!
//! `calcular_uso` y `margenes` son puras. `uso` y `uso_en` leen `eventos.jsonl`.

use std::collections::HashSet;
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use time::format_description::well_known::Rfc3339;
use time::{OffsetDateTime, UtcOffset};

use crate::config::{Balance, Config, root_home};
use crate::error::Error;
use crate::observe::{self, ErrorObserve, jsonl, ruta_eventos};
use crate::route::{inicio_mes, proximo_mes};
use crate::tipos::{Evento, TipoEvento, Uso, Via};

/// Aviso de `r00t observa uso`: una sesión en la nube que nadie registra no suma.
pub const AVISO_NUBE_NO_REGISTRADA: &str = "Root solo cuenta las sesiones en la nube que alguien registra; una sesión sin registrar no suma.";

/// Gasto y margen de un tope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MargenTope {
    pub usado_usd: f64,
    pub tope_usd: f64,
    /// Usado sobre tope. Puede pasar de 1 si el gasto superó el tope.
    pub fraccion: f64,
    /// Lo que queda del tope, sin bajar de cero.
    pub restante_usd: f64,
    /// Cupo: fin del bloque vigente. API y nube: inicio del mes siguiente. `None` si no está lleno.
    pub reabre: Option<f64>,
    /// `usado_usd >= tope_usd`.
    pub lleno: bool,
}

/// Márgenes de los tres topes del balance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Margenes {
    pub cupo: MargenTope,
    pub api: MargenTope,
    pub nube: MargenTope,
}

/// Suma los eventos de una lista y arma el `Uso`. Función pura: el reloj y el desfase llegan por parámetro.
///
/// El bloque de cupo cuenta eventos `Cupo` y `Nube` (P9). Un bloque empieza con el primer evento
/// posterior al fin del bloque anterior. Los meses y las vías de API y nube usan el mes calendario
/// de `offset_s`. `Oraculo` y eventos sin vía no suman. Los eventos con `ts > ahora` se ignoran.
pub fn calcular_uso(eventos: &[Evento], ahora: f64, offset_s: i32, bloque_horas: u32) -> Uso {
    let dur = f64::from(bloque_horas) * 3600.0;
    let visibles = || eventos.iter().filter(|ev| ev.ts <= ahora);
    let mut de_cupo: Vec<&Evento> = visibles()
        .filter(|ev| matches!(ev.via, Some(Via::Cupo | Via::Nube)))
        .collect();
    de_cupo.sort_by(|a, b| a.ts.total_cmp(&b.ts));
    let (cupo_bloque_usd, cupo_bloque_inicio) = bloque_vigente(&de_cupo, ahora, dur);
    let inicio = crate::route::inicio_mes(ahora, offset_s);
    let suma_mes = |via: Via| -> f64 {
        visibles()
            .filter(|ev| ev.ts >= inicio && ev.via == Some(via) && ev.tipo != TipoEvento::Reserva)
            // `fold` evita el -0.0 que `sum` da sin eventos.
            .fold(0.0, |total, ev| total + ev.cost_usd)
    };
    let vigentes: Vec<&Evento> = visibles().collect();
    Uso {
        cupo_bloque_usd,
        cupo_bloque_inicio,
        api_mes_usd: suma_mes(Via::Api) + reservas_abiertas_usd(&vigentes, inicio),
        nube_mes_usd: suma_mes(Via::Nube),
    }
}

/// Costo de las reservas de API del mes que ningún evento cerró. Un evento cierra las reservas que
/// lista en `extra.reservas`. Una reserva sin id, o sin cierre, cuenta como gasto.
fn reservas_abiertas_usd(eventos: &[&Evento], inicio: f64) -> f64 {
    let cerradas: HashSet<&str> = eventos
        .iter()
        .filter_map(|ev| ev.extra.get("reservas")?.as_array())
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    eventos
        .iter()
        .filter(|ev| ev.tipo == TipoEvento::Reserva && ev.via == Some(Via::Api) && ev.ts >= inicio)
        .filter(|ev| {
            !ev.extra
                .get("reserva")
                .and_then(Value::as_str)
                .is_some_and(|id| cerradas.contains(id))
        })
        .fold(0.0, |total, ev| total + ev.cost_usd)
}

/// Gasto e inicio del bloque que contiene `ahora`. `eventos` van ordenados por `ts` y con `ts <= ahora`.
/// Un bloque empieza con el primer evento posterior al fin del anterior. Sin bloque vigente: `(0, None)`.
fn bloque_vigente(eventos: &[&Evento], ahora: f64, dur: f64) -> (f64, Option<f64>) {
    let mut actual: Option<(f64, f64)> = None;
    for ev in eventos {
        actual = match actual {
            Some((inicio, usd)) if ev.ts < inicio + dur => Some((inicio, usd + ev.cost_usd)),
            _ => Some((ev.ts, ev.cost_usd)),
        };
    }
    match actual {
        Some((inicio, usd)) if ahora < inicio + dur => (usd, Some(inicio)),
        _ => (0.0, None),
    }
}

/// Desfase local actual en segundos. Si el sistema no lo da, vale 0 (UTC).
pub fn offset_local() -> i32 {
    UtcOffset::current_local_offset().map_or(0, |o| o.whole_seconds())
}

/// `calcular_uso` sobre `eventos.jsonl` de `ROOT_HOME` y `root.toml`. Usa el desfase local actual.
pub fn uso(ahora: f64) -> Result<Uso, Error> {
    let cfg = Config::cargar(&root_home().join("root.toml"))?;
    Ok(uso_en(
        &ruta_eventos(),
        ahora,
        offset_local(),
        cfg.balance.cupo.bloque_horas,
    )?)
}

/// Todos los eventos del archivo indicado y de sus rotados (`<nombre>.N`), en orden de escritura.
/// Un archivo inexistente da una lista vacía. Una línea inválida es error. Lista y lee con un solo
/// lock compartido, para que una rotación no deje afuera un archivo.
pub fn leer_eventos(ruta: &Path) -> Result<Vec<Evento>, ErrorObserve> {
    jsonl::con_lock_compartido(ruta, |log| -> Result<Vec<Evento>, ErrorObserve> {
        eventos_de(log.lineas()?)
    })
}

/// `f` con los eventos del archivo y sus rotados, leídos bajo el lock exclusivo. `f` escribe con
/// `log`: leer, decidir y escribir pasan en un solo tramo, sin que otro proceso se cuele.
pub fn con_eventos_bloqueados<T>(
    ruta: &Path,
    f: impl FnOnce(&[Evento], &jsonl::Escritura<'_>) -> Result<T, ErrorObserve>,
) -> Result<T, ErrorObserve> {
    jsonl::con_lock_exclusivo(ruta, |log| -> Result<T, ErrorObserve> {
        let eventos = eventos_de(log.lineas()?)?;
        f(&eventos, log)
    })
}

/// Parsea las líneas de un log. Una línea inválida es error.
fn eventos_de(lineas: Vec<String>) -> Result<Vec<Evento>, ErrorObserve> {
    lineas
        .iter()
        .map(|linea| Ok(serde_json::from_str::<Evento>(linea)?))
        .collect()
}

/// `calcular_uso` sobre el archivo indicado y todos sus rotados (`<nombre>.N`).
/// Un archivo inexistente da un `Uso` vacío. Una línea inválida es error.
pub fn uso_en(
    ruta: &Path,
    ahora: f64,
    offset_s: i32,
    bloque_horas: u32,
) -> Result<Uso, ErrorObserve> {
    let eventos = leer_eventos(ruta)?;
    Ok(calcular_uso(&eventos, ahora, offset_s, bloque_horas))
}

/// Salida de `r00t observa uso`: gasto, márgenes y el aviso de la nube.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UsoMargenes {
    pub uso: Uso,
    pub margenes: Margenes,
    pub aviso: String,
}

/// Estado de `observa uso` en el momento `ahora`. Registra un `Gate` `tope-lleno` por cada tope
/// lleno que todavía no tiene uno en su período: el bloque (cupo) o el mes (API y nube).
pub fn estado(ahora: f64) -> Result<UsoMargenes, Error> {
    let cfg = Config::cargar(&root_home().join("root.toml"))?;
    let offset = offset_local();
    let eventos = leer_eventos(&ruta_eventos())?;
    let uso = calcular_uso(&eventos, ahora, offset, cfg.balance.cupo.bloque_horas);
    let margenes = margenes(&uso, &cfg.balance, ahora, offset);
    let mes = inicio_mes(ahora, offset);
    for tope in topes_sin_gate(&eventos, &margenes, uso.cupo_bloque_inicio, mes) {
        registrar_gate_tope(tope, &margenes)?;
    }
    Ok(UsoMargenes {
        uso,
        margenes,
        aviso: AVISO_NUBE_NO_REGISTRADA.into(),
    })
}

/// Tras registrar sesiones en la nube: si el tope de nube del mes está lleno y no tiene `Gate`
/// en ese mes, lo registra. Los topes de cupo y de API no se tocan.
pub fn gate_nube_si_lleno(ahora: f64) -> Result<(), Error> {
    let cfg = Config::cargar(&root_home().join("root.toml"))?;
    let offset = offset_local();
    let eventos = leer_eventos(&ruta_eventos())?;
    let uso = calcular_uso(&eventos, ahora, offset, cfg.balance.cupo.bloque_horas);
    let margenes = margenes(&uso, &cfg.balance, ahora, offset);
    let mes = inicio_mes(ahora, offset);
    if topes_sin_gate(&eventos, &margenes, uso.cupo_bloque_inicio, mes).contains(&"nube") {
        registrar_gate_tope("nube", &margenes)?;
    }
    Ok(())
}

/// Registra el `Gate` `tope-lleno` de un tope, con su gasto y su límite.
fn registrar_gate_tope(tope: &str, margenes: &Margenes) -> Result<(), Error> {
    let m = match tope {
        "cupo" => &margenes.cupo,
        "api" => &margenes.api,
        _ => &margenes.nube,
    };
    let extra = Map::from_iter([
        ("tope".to_string(), json!(tope)),
        ("usado".to_string(), json!(m.usado_usd)),
        ("tope_usd".to_string(), json!(m.tope_usd)),
    ]);
    observe::registrar(&observe::evento_gate("uso", "tope-lleno", extra))
        .map_err(ErrorObserve::from)?;
    Ok(())
}

/// Topes llenos que no tienen un `Gate` `tope-lleno` dentro de su período.
/// El cupo cuenta desde el inicio del bloque; la API y la nube, desde el inicio del mes.
pub fn topes_sin_gate(
    eventos: &[Evento],
    m: &Margenes,
    cupo_inicio: Option<f64>,
    inicio_mes: f64,
) -> Vec<&'static str> {
    let registrado = |tope: &str, desde: f64| {
        eventos.iter().any(|ev| {
            ev.tipo == TipoEvento::Gate
                && ev.fase == "tope-lleno"
                && ev.ts >= desde
                && ev.extra.get("tope").and_then(Value::as_str) == Some(tope)
        })
    };
    let mut pendientes = Vec::new();
    if m.cupo.lleno
        && let Some(inicio) = cupo_inicio
        && !registrado("cupo", inicio)
    {
        pendientes.push("cupo");
    }
    if m.api.lleno && !registrado("api", inicio_mes) {
        pendientes.push("api");
    }
    if m.nube.lleno && !registrado("nube", inicio_mes) {
        pendientes.push("nube");
    }
    pendientes
}

/// Fecha y hora RFC 3339 de un epoch en segundos, con el desfase dado.
pub fn iso(epoch: f64, offset_s: i32) -> String {
    let desfase = UtcOffset::from_whole_seconds(offset_s).unwrap_or(UtcOffset::UTC);
    OffsetDateTime::from_unix_timestamp(epoch.floor() as i64)
        .unwrap_or(OffsetDateTime::UNIX_EPOCH)
        .to_offset(desfase)
        .format(&Rfc3339)
        .unwrap_or_default()
}

/// Márgenes de los tres topes de `balance`, a partir del `uso` y del momento `ahora`.
pub fn margenes(uso: &Uso, b: &Balance, ahora: f64, offset_s: i32) -> Margenes {
    let fin_bloque = uso
        .cupo_bloque_inicio
        .map(|inicio| inicio + f64::from(b.cupo.bloque_horas) * 3600.0);
    // Un bloque vencido cuenta como vacío, igual que en Route.
    let vencido = fin_bloque.is_some_and(|fin| ahora >= fin);
    let cupo_usado = if vencido { 0.0 } else { uso.cupo_bloque_usd };
    let proximo = proximo_mes(ahora, offset_s);
    Margenes {
        cupo: margen(cupo_usado, b.cupo.tope_usd, fin_bloque),
        api: margen(uso.api_mes_usd, b.api.tope_usd, Some(proximo)),
        nube: margen(uso.nube_mes_usd, b.nube.tope_usd, Some(proximo)),
    }
}

/// Margen de un tope. `reabre_si_lleno` solo cuenta si el gasto llegó al tope.
fn margen(usado: f64, tope: f64, reabre_si_lleno: Option<f64>) -> MargenTope {
    let lleno = usado >= tope;
    MargenTope {
        usado_usd: usado,
        tope_usd: tope,
        fraccion: usado / tope,
        restante_usd: (tope - usado).max(0.0),
        reabre: if lleno { reabre_si_lleno } else { None },
        lleno,
    }
}

/// Reasigna la vía de todos los eventos de una sesión. Conserva costos y tokens.
/// Con `Via::Nube` agrega `extra.origen = "nube"`.
pub fn con_via(eventos: Vec<Evento>, via: Via) -> Vec<Evento> {
    eventos
        .into_iter()
        .map(|mut ev| {
            ev.via = Some(via);
            if via == Via::Nube {
                ev.extra
                    .insert("origen".into(), Value::String("nube".into()));
            }
            ev
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observe::claude::leer_sesion_claude;
    use crate::tipos::TipoEvento;
    use std::fs;
    use tempfile::TempDir;

    const HORA: f64 = 3600.0;
    /// 2026-10-15T12:00:00Z.
    const AHORA: f64 = 1_792_065_600.0;
    /// 2026-10-01T00:00:00Z.
    const INICIO_OCTUBRE_UTC: f64 = 1_790_812_800.0;
    /// 2026-11-01T00:00:00Z.
    const PROXIMO_MES_UTC: f64 = 1_793_491_200.0;

    fn evento(via: Option<Via>, ts: f64, cost_usd: f64) -> Evento {
        Evento {
            ts,
            via,
            cost_usd,
            tipo: TipoEvento::Llamada,
            corrida: "c1".into(),
            ..Evento::default()
        }
    }

    fn cupo(ts: f64, cost_usd: f64) -> Evento {
        evento(Some(Via::Cupo), ts, cost_usd)
    }

    fn nube(ts: f64, cost_usd: f64) -> Evento {
        evento(Some(Via::Nube), ts, cost_usd)
    }

    fn api(ts: f64, cost_usd: f64) -> Evento {
        evento(Some(Via::Api), ts, cost_usd)
    }

    fn calcular(eventos: &[Evento]) -> Uso {
        calcular_uso(eventos, AHORA, 0, 5)
    }

    /// Reserva de API de `cost_usd` con el id `id`, como la escribe la llamada antes de enviar.
    fn reserva(id: &str, ts: f64, cost_usd: f64) -> Evento {
        Evento {
            ts,
            via: Some(Via::Api),
            cost_usd,
            tipo: TipoEvento::Reserva,
            corrida: "c1".into(),
            extra: serde_json::Map::from_iter([("reserva".to_string(), json!(id))]),
            ..Evento::default()
        }
    }

    /// Evento final de una llamada que cierra las reservas `ids`.
    fn cierre(ts: f64, cost_usd: f64, ids: &[&str]) -> Evento {
        Evento {
            extra: serde_json::Map::from_iter([("reservas".to_string(), json!(ids))]),
            ..api(ts, cost_usd)
        }
    }

    #[test]
    fn uso_leer_eventos_toma_el_lock_antes_de_listar_los_archivos() {
        use std::fs::OpenOptions;
        use std::thread;
        use std::time::Duration;

        let dir = TempDir::new().expect("temp dir");
        let ruta = dir.path().join("eventos.jsonl");
        let linea = |ev: &Evento| serde_json::to_string(ev).expect("serializa");
        fs::write(&ruta, format!("{}\n", linea(&api(AHORA - HORA, 1.0)))).expect("vigente");
        // Un escritor toma el lock exclusivo y el lector espera detrás de él.
        let escritor = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(dir.path().join("eventos.jsonl.lock"))
            .expect("candado");
        fs4::FileExt::lock(&escritor).expect("lock exclusivo");
        let ruta_lector = ruta.clone();
        let lector = thread::spawn(move || leer_eventos(&ruta_lector));
        // Tiempo para que el lector liste los archivos, si los lista antes de tomar el lock.
        thread::sleep(Duration::from_millis(200));
        // El escritor rota el log con el lock tomado, como en `jsonl::agregar`.
        fs::rename(&ruta, dir.path().join("eventos.jsonl.1")).expect("rota");
        fs::write(&ruta, format!("{}\n", linea(&api(AHORA, 2.0)))).expect("vigente nuevo");
        drop(escritor);
        let eventos = lector.join().expect("hilo").expect("lee");
        assert_eq!(eventos.len(), 2, "el evento rotado no debe perderse");
    }

    #[test]
    fn uso_reserva_abierta_suma_al_mes() {
        let uso = calcular(&[reserva("r1", AHORA - HORA, 0.5)]);
        assert_eq!(uso.api_mes_usd, 0.5);
    }

    #[test]
    fn uso_reserva_cerrada_por_su_llamada_no_suma_dos_veces() {
        let eventos = [
            reserva("r1", AHORA - 2.0 * HORA, 0.5),
            cierre(AHORA - HORA, 0.4, &["r1"]),
        ];
        assert!((calcular(&eventos).api_mes_usd - 0.4).abs() < 1e-12);
    }

    #[test]
    fn uso_reserva_de_otro_mes_no_suma() {
        let uso = calcular(&[reserva("r1", INICIO_OCTUBRE_UTC - HORA, 0.5)]);
        assert_eq!(uso.api_mes_usd, 0.0);
    }

    #[test]
    fn uso_cupo_de_hace_seis_horas_no_cuenta() {
        let uso = calcular(&[cupo(AHORA - 6.0 * HORA, 10.0)]);
        assert_eq!(uso.cupo_bloque_usd, 0.0);
        assert_eq!(uso.cupo_bloque_inicio, None);
    }

    #[test]
    fn uso_dos_cupos_a_tres_horas_suman_en_el_mismo_bloque() {
        let inicio = AHORA - 3.0 * HORA;
        let uso = calcular(&[cupo(inicio, 4.0), cupo(AHORA, 6.0)]);
        assert_eq!(uso.cupo_bloque_usd, 10.0);
        assert_eq!(uso.cupo_bloque_inicio, Some(inicio));
    }

    #[test]
    fn uso_cupo_de_cinco_horas_treinta_y_otro_de_diez_minutos_son_dos_bloques() {
        let inicio_segundo = AHORA - 10.0 * 60.0;
        let uso = calcular(&[cupo(AHORA - 5.5 * HORA, 7.0), cupo(inicio_segundo, 2.0)]);
        assert_eq!(uso.cupo_bloque_usd, 2.0);
        assert_eq!(uso.cupo_bloque_inicio, Some(inicio_segundo));
    }

    #[test]
    fn uso_tercer_bloque_empieza_justo_al_fin_del_anterior() {
        // Bloque 1: [-12h, -7h). Bloque 2: [-7h, -2h). Bloque 3: [-2h, +3h).
        let inicio_tercero = AHORA - 2.0 * HORA;
        let uso = calcular(&[
            cupo(AHORA - 12.0 * HORA, 1.0),
            cupo(AHORA - 7.0 * HORA, 2.0),
            cupo(inicio_tercero, 4.0),
        ]);
        assert_eq!(uso.cupo_bloque_usd, 4.0);
        assert_eq!(uso.cupo_bloque_inicio, Some(inicio_tercero));
    }

    #[test]
    fn uso_cupo_con_ts_futuro_se_ignora() {
        let inicio = AHORA - HORA;
        let uso = calcular(&[cupo(inicio, 3.0), cupo(AHORA + HORA, 5.0)]);
        assert_eq!(uso.cupo_bloque_usd, 3.0);
        assert_eq!(uso.cupo_bloque_inicio, Some(inicio));
    }

    #[test]
    fn uso_sesion_en_la_nube_suma_al_mes_y_al_bloque() {
        let uso = calcular(&[nube(AHORA - HORA, 10.0)]);
        assert_eq!(uso.nube_mes_usd, 10.0);
        assert_eq!(uso.cupo_bloque_usd, 10.0);
        assert_eq!(uso.api_mes_usd, 0.0);
    }

    #[test]
    fn uso_nube_del_mes_anterior_no_cuenta_al_mes() {
        let uso = calcular_uso(
            &[nube(INICIO_OCTUBRE_UTC - 1.0, 9.0)],
            PROXIMO_MES_UTC,
            0,
            5,
        );
        assert_eq!(uso.nube_mes_usd, 0.0);
    }

    #[test]
    fn uso_gasto_de_api_del_mes_anterior_no_cuenta() {
        let uso = calcular(&[api(INICIO_OCTUBRE_UTC - 1.0, 50.0)]);
        assert_eq!(uso.api_mes_usd, 0.0);
    }

    #[test]
    fn uso_api_en_el_primer_segundo_del_mes_cuenta() {
        let uso = calcular(&[api(INICIO_OCTUBRE_UTC, 7.0)]);
        assert_eq!(uso.api_mes_usd, 7.0);
    }

    #[test]
    fn uso_offset_corre_el_borde_del_mes() {
        // Ahora: 2026-11-01T02:30Z, que es 2026-10-31T23:30 en UTC-3.
        let ahora = PROXIMO_MES_UTC + 2.5 * HORA;
        let eventos = [
            // 2026-10-01T02:00Z: en UTC-3 es 2026-09-30 23:00, o sea septiembre.
            api(INICIO_OCTUBRE_UTC + 2.0 * HORA, 5.0),
            // 2026-10-01T04:00Z: en UTC-3 es 2026-10-01 01:00, o sea octubre.
            api(INICIO_OCTUBRE_UTC + 4.0 * HORA, 3.0),
            // 2026-11-01T02:00Z: en UTC-3 todavía es octubre.
            api(PROXIMO_MES_UTC + 2.0 * HORA, 7.0),
        ];
        let en_utc_menos_3 = calcular_uso(&eventos, ahora, -10_800, 5);
        assert_eq!(en_utc_menos_3.api_mes_usd, 10.0);
        let en_utc = calcular_uso(&eventos, ahora, 0, 5);
        assert_eq!(en_utc.api_mes_usd, 7.0);
    }

    #[test]
    fn uso_oraculo_y_sin_via_no_suman() {
        let uso = calcular(&[
            evento(Some(Via::Oraculo), AHORA - HORA, 9.0),
            evento(None, AHORA - HORA, 4.0),
        ]);
        assert_eq!(uso, Uso::default());
    }

    #[test]
    fn uso_sin_eventos_da_default() {
        assert_eq!(calcular(&[]), Uso::default());
    }

    #[test]
    fn uso_en_suma_rotado_y_vigente() {
        let dir = TempDir::new().expect("temp dir");
        let ruta = dir.path().join("eventos.jsonl");
        let linea = |ev: &Evento| format!("{}\n", serde_json::to_string(ev).expect("serializa"));
        fs::write(
            dir.path().join("eventos.jsonl.1"),
            linea(&api(AHORA - 2.0 * HORA, 3.0)) + &linea(&cupo(AHORA - 2.0 * HORA, 1.0)),
        )
        .expect("rotado");
        fs::write(
            &ruta,
            linea(&api(AHORA - HORA, 4.0)) + &linea(&cupo(AHORA - HORA, 2.0)),
        )
        .expect("vigente");
        let uso = uso_en(&ruta, AHORA, 0, 5).expect("lee");
        assert_eq!(uso.api_mes_usd, 7.0);
        assert_eq!(uso.cupo_bloque_usd, 3.0);
    }

    #[test]
    fn uso_en_archivo_inexistente_da_uso_vacio() {
        let dir = TempDir::new().expect("temp dir");
        let uso = uso_en(&dir.path().join("no-existe.jsonl"), AHORA, 0, 5).expect("no es error");
        assert_eq!(uso, Uso::default());
    }

    #[test]
    fn uso_en_linea_invalida_es_error() {
        let dir = TempDir::new().expect("temp dir");
        let ruta = dir.path().join("eventos.jsonl");
        fs::write(&ruta, "esto no es json\n").expect("escribe");
        assert!(uso_en(&ruta, AHORA, 0, 5).is_err());
    }

    fn margenes_de(uso: &Uso) -> Margenes {
        margenes(uso, &Balance::default(), AHORA, 0)
    }

    #[test]
    fn uso_margen_cupo_32_de_35() {
        let uso = Uso {
            cupo_bloque_usd: 32.0,
            cupo_bloque_inicio: Some(AHORA - HORA),
            ..Uso::default()
        };
        let m = margenes_de(&uso).cupo;
        assert!((m.fraccion - 32.0 / 35.0).abs() < 1e-9);
        assert_eq!(m.restante_usd, 3.0);
        assert!(!m.lleno);
        assert_eq!(m.reabre, None);
    }

    #[test]
    fn uso_margen_cupo_lleno_reabre_al_fin_del_bloque() {
        let inicio = AHORA - HORA;
        let uso = Uso {
            cupo_bloque_usd: 35.0,
            cupo_bloque_inicio: Some(inicio),
            ..Uso::default()
        };
        let m = margenes_de(&uso).cupo;
        assert!(m.lleno);
        assert_eq!(m.restante_usd, 0.0);
        assert_eq!(m.reabre, Some(inicio + 5.0 * HORA));
    }

    #[test]
    fn uso_margen_cupo_vencido_no_cuenta() {
        let uso = Uso {
            cupo_bloque_usd: 35.0,
            cupo_bloque_inicio: Some(AHORA - 6.0 * HORA),
            ..Uso::default()
        };
        let m = margenes_de(&uso).cupo;
        assert_eq!(m.usado_usd, 0.0);
        assert!(!m.lleno);
    }

    #[test]
    fn uso_margen_api_lleno_reabre_al_mes_siguiente() {
        let uso = Uso {
            api_mes_usd: 100.0,
            ..Uso::default()
        };
        let m = margenes_de(&uso).api;
        assert!(m.lleno);
        assert_eq!(m.reabre, Some(PROXIMO_MES_UTC));
    }

    #[test]
    fn uso_margen_nube_250_esta_lleno() {
        let uso = Uso {
            nube_mes_usd: 250.0,
            ..Uso::default()
        };
        let m = margenes_de(&uso).nube;
        assert!(m.lleno);
        assert_eq!(m.reabre, Some(PROXIMO_MES_UTC));
    }

    #[test]
    fn uso_margen_sin_gasto_no_reabre_nada() {
        let m = margenes_de(&Uso::default());
        assert!(!m.api.lleno && m.api.reabre.is_none());
        assert!(!m.nube.lleno && m.nube.reabre.is_none());
        assert_eq!(m.api.restante_usd, 100.0);
    }

    #[test]
    fn uso_con_via_cambia_la_via_y_marca_el_origen() {
        let ruta = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude-sesion.json");
        let original = leer_sesion_claude(&ruta, "c1").expect("lee la sesión");
        let nuevos = con_via(original.clone(), Via::Nube);
        assert_eq!(nuevos.len(), original.len());
        for (antes, despues) in original.iter().zip(&nuevos) {
            assert_eq!(despues.via, Some(Via::Nube));
            assert_eq!(despues.extra["origen"], "nube");
            assert_eq!(despues.cost_usd, antes.cost_usd);
            assert_eq!(despues.in_tokens, antes.in_tokens);
            assert_eq!(despues.out_tokens, antes.out_tokens);
            assert_eq!(despues.modelo, antes.modelo);
        }
    }

    #[test]
    fn uso_aviso_nube_no_registrada_dice_que_no_suma() {
        assert!(AVISO_NUBE_NO_REGISTRADA.contains("no suma"));
    }
}

#[cfg(test)]
mod tests_topes {
    use super::*;

    const AHORA: f64 = 1_792_065_600.0;
    const HORA: f64 = 3600.0;

    fn gate(tope: &str, ts: f64) -> Evento {
        Evento {
            ts,
            corrida: "uso".into(),
            tipo: TipoEvento::Gate,
            fase: "tope-lleno".into(),
            extra: Map::from_iter([("tope".to_string(), json!(tope))]),
            ..Evento::default()
        }
    }

    fn lleno(cupo: bool, api: bool, nube: bool) -> Margenes {
        let uso = Uso {
            cupo_bloque_usd: if cupo { 35.0 } else { 0.0 },
            cupo_bloque_inicio: Some(AHORA - HORA),
            api_mes_usd: if api { 100.0 } else { 0.0 },
            nube_mes_usd: if nube { 250.0 } else { 0.0 },
        };
        margenes(&uso, &Balance::default(), AHORA, 0)
    }

    #[test]
    fn topes_sin_gate_lista_los_llenos_sin_registro() {
        let m = lleno(true, true, true);
        let pendientes = topes_sin_gate(&[], &m, Some(AHORA - HORA), 0.0);
        assert_eq!(pendientes, vec!["cupo", "api", "nube"]);
    }

    #[test]
    fn topes_sin_gate_no_repite_si_ya_hay_gate_del_periodo() {
        let m = lleno(true, true, true);
        let eventos = [
            gate("cupo", AHORA - 60.0),
            gate("api", AHORA - 60.0),
            gate("nube", AHORA - 60.0),
        ];
        assert!(topes_sin_gate(&eventos, &m, Some(AHORA - HORA), 0.0).is_empty());
    }

    #[test]
    fn topes_sin_gate_gate_de_otro_periodo_no_cuenta() {
        let m = lleno(true, true, false);
        // Gate del bloque anterior (antes del inicio) y del mes anterior (antes del inicio del mes).
        let inicio_mes = AHORA - 10.0 * 24.0 * HORA;
        let eventos = [
            gate("cupo", AHORA - 2.0 * HORA),
            gate("api", inicio_mes - 1.0),
        ];
        let pendientes = topes_sin_gate(&eventos, &m, Some(AHORA - HORA), inicio_mes);
        assert_eq!(pendientes, vec!["cupo", "api"]);
    }

    #[test]
    fn topes_sin_gate_tope_no_lleno_no_cuenta() {
        let m = lleno(false, false, false);
        assert!(topes_sin_gate(&[], &m, None, 0.0).is_empty());
    }

    #[test]
    fn uso_iso_da_rfc3339_con_el_desfase() {
        assert_eq!(iso(AHORA, -10_800), "2026-10-15T09:00:00-03:00");
    }
}
