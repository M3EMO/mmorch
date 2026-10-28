//! Uso: cuánto gastó Root en cada vía, y cuánto margen queda en cada tope.
//!
//! `calcular_uso` y `margenes` son puras. `uso` y `uso_en` leen `eventos.jsonl`.

use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::UtcOffset;

use crate::config::{Balance, Config, root_home};
use crate::error::Error;
use crate::observe::{ErrorObserve, jsonl, ruta_eventos};
use crate::route::proximo_mes;
use crate::tipos::{Evento, Uso, Via};

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
            .filter(|ev| ev.ts >= inicio && ev.via == Some(via))
            .map(|ev| ev.cost_usd)
            .sum()
    };
    Uso {
        cupo_bloque_usd,
        cupo_bloque_inicio,
        api_mes_usd: suma_mes(Via::Api),
        nube_mes_usd: suma_mes(Via::Nube),
    }
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

/// `calcular_uso` sobre `eventos.jsonl` de `ROOT_HOME` y `root.toml`. Usa el desfase local actual.
///
/// Si no se puede leer el desfase local, usa UTC y lo deja dicho aquí.
pub fn uso(ahora: f64) -> Result<Uso, Error> {
    let cfg = Config::cargar(&root_home().join("root.toml"))?;
    // Si el sistema no da el desfase local, se usa UTC.
    let offset_s = UtcOffset::current_local_offset().map_or(0, |o| o.whole_seconds());
    Ok(uso_en(
        &ruta_eventos(),
        ahora,
        offset_s,
        cfg.balance.cupo.bloque_horas,
    )?)
}

/// `calcular_uso` sobre el archivo indicado y todos sus rotados (`<nombre>.N`).
/// Un archivo inexistente da un `Uso` vacío. Una línea inválida es error.
pub fn uso_en(
    ruta: &Path,
    ahora: f64,
    offset_s: i32,
    bloque_horas: u32,
) -> Result<Uso, ErrorObserve> {
    let mut eventos = Vec::new();
    for archivo in jsonl::archivos_en_orden(ruta)? {
        for linea in jsonl::leer_lineas(&archivo)? {
            eventos.push(serde_json::from_str::<Evento>(&linea)?);
        }
    }
    Ok(calcular_uso(&eventos, ahora, offset_s, bloque_horas))
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
