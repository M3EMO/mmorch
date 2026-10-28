//! Route: elige el modelo y la vía de cada pedido. Función pura, sin E/S.

pub mod politica;

pub use politica::{Candidato, Politica};

use time::{Date, Month, OffsetDateTime, PrimitiveDateTime, Time, UtcOffset};

use crate::config::{Config, ModeloApi};
use crate::tipos::{Eleccion, Pedido, Proveedor, Rol, SinPresupuesto, Uso, Via};

/// Inicio del próximo mes calendario después de `ahora`, en epoch segundos.
///
/// `offset_s` es el desfase local en segundos (0 es UTC). El borde real del mes es hora local,
/// por eso `r00t route` pasa su desfase. Así el mes cierra a las 00:00 locales.
pub fn proximo_mes(ahora: f64, offset_s: i32) -> f64 {
    let desfase = UtcOffset::from_whole_seconds(offset_s).unwrap_or(UtcOffset::UTC);
    let local = OffsetDateTime::from_unix_timestamp(ahora.floor() as i64)
        .unwrap_or(OffsetDateTime::UNIX_EPOCH)
        .to_offset(desfase);
    let (anio, mes) = match local.month() {
        Month::December => (local.year() + 1, Month::January),
        otro => (
            local.year(),
            Month::try_from(otro as u8 + 1).unwrap_or(Month::January),
        ),
    };
    let inicio = Date::from_calendar_date(anio, mes, 1).unwrap_or(local.date());
    PrimitiveDateTime::new(inicio, Time::MIDNIGHT)
        .assume_offset(desfase)
        .unix_timestamp() as f64
}

/// Inicio del mes calendario que contiene `ahora`, en epoch segundos.
///
/// `offset_s` es el desfase local en segundos (0 es UTC). El mes empieza el día 1 a las 00:00 local.
pub fn inicio_mes(ahora: f64, offset_s: i32) -> f64 {
    let desfase = UtcOffset::from_whole_seconds(offset_s).unwrap_or(UtcOffset::UTC);
    let local = OffsetDateTime::from_unix_timestamp(ahora.floor() as i64)
        .unwrap_or(OffsetDateTime::UNIX_EPOCH)
        .to_offset(desfase);
    let inicio = Date::from_calendar_date(local.year(), local.month(), 1).unwrap_or(local.date());
    PrimitiveDateTime::new(inicio, Time::MIDNIGHT)
        .assume_offset(desfase)
        .unix_timestamp() as f64
}

/// Opción de modelo antes del balance.
struct Opcion {
    via: Via,
    proveedor: Option<Proveedor>,
    modelo: String,
    motivo: String,
}

/// Estado del cupo del bloque vigente. Un bloque vencido cuenta como vacío.
struct Cupo {
    usd: f64,
    tope: f64,
    lleno: bool,
    fin: Option<f64>,
}

/// Elige la vía y el modelo de un pedido. Función pura: el reloj y el desfase llegan por parámetro.
///
/// La firma del plan era `elegir(p, pol, uso, balance)`. Agrega `cfg`, `ahora` y `offset_s`
/// para que no lea el reloj ni el archivo de configuración.
pub fn elegir(
    p: &Pedido,
    pol: &Politica,
    uso: &Uso,
    cfg: &Config,
    ahora: f64,
    offset_s: i32,
) -> Result<Eleccion, SinPresupuesto> {
    if p.checkeable {
        return Ok(Eleccion {
            via: Via::Oraculo,
            proveedor: None,
            modelo: None,
            razonamiento: false,
            motivo: "checkeable: el oráculo decide sin llamar a ningún modelo".into(),
        });
    }
    // Invariante 2: Verifica y Juzga siempre razonan, con cualquier modelo y cualquier presupuesto.
    let razonamiento = matches!(p.rol, Rol::Verifica | Rol::Juzga);
    let api_ok = uso.api_mes_usd < cfg.balance.api.tope_usd;
    let cupo = estado_cupo(uso, cfg, ahora);
    let mut elegida = opcion_base(p, pol, cfg);

    // Preferencia: con el cupo sobre `prefiere_api_sobre`, pasa a la API si hay margen y equivalente.
    let preferir_api = elegida.via == Via::Cupo
        && !cupo.lleno
        && cupo.usd >= f64::from(cfg.balance.prefiere_api_sobre) * cupo.tope
        && api_ok;
    if let Some(eq) = equivalente(cfg, &elegida.modelo).filter(|_| preferir_api) {
        elegida = opcion_api(
            eq,
            format!(
                "cupo al {:.0}% del bloque y API con margen: pasa a la API",
                100.0 * cupo.usd / cupo.tope
            ),
        );
    }

    // Vía prohibida: cambia a la otra con margen.
    let prohibida = match elegida.via {
        Via::Cupo => cupo.lleno,
        Via::Api => !api_ok,
        Via::Oraculo | Via::Nube => false,
    };
    if prohibida {
        let alterna = match elegida.via {
            Via::Api if !cupo.lleno => Some(opcion_cupo(
                &cfg.modelos.organiza,
                "API del mes agotada: pasa al cupo del alias de organiza".into(),
            )),
            Via::Cupo if api_ok => equivalente(cfg, &elegida.modelo)
                .map(|eq| opcion_api(eq, "cupo del bloque agotado: pasa a la API".into())),
            _ => None,
        };
        match alterna {
            Some(otra) => elegida = otra,
            None => return Err(sin_presupuesto(&elegida, cupo, api_ok, ahora, offset_s)),
        }
    }

    Ok(Eleccion {
        via: elegida.via,
        proveedor: elegida.proveedor,
        modelo: Some(elegida.modelo),
        razonamiento,
        motivo: elegida.motivo,
    })
}

/// Modelo base del rol. Solo `Delega`, `Verifica` y `Juzga` cambian con la política aprendida.
fn opcion_base(p: &Pedido, pol: &Politica, cfg: &Config) -> Opcion {
    if let Some(c) = aprendido(p, pol, cfg) {
        return Opcion {
            via: c.via,
            proveedor: c.proveedor,
            modelo: c.modelo.clone(),
            motivo: format!(
                "aprendido: {} con Wilson {:.2} sobre el umbral {:.2} y el menor costo",
                c.modelo, c.wilson_inf, cfg.umbral_wilson
            ),
        };
    }
    match p.rol {
        Rol::Planifica => opcion_cupo(
            &cfg.modelos.planifica,
            "root.toml: planifica por cupo".into(),
        ),
        Rol::Organiza => opcion_cupo(&cfg.modelos.organiza, "root.toml: organiza por cupo".into()),
        Rol::Construye => opcion_cupo(
            &cfg.modelos.construye,
            "root.toml: construye por cupo".into(),
        ),
        Rol::Delega => opcion_api(
            &cfg.ruteo.delega,
            "default de [ruteo]: delega por API".into(),
        ),
        Rol::Verifica => opcion_api(
            &cfg.ruteo.verifica,
            "default de [ruteo]: verifica por API".into(),
        ),
        Rol::Juzga => opcion_api(&cfg.ruteo.juzga, "default de [ruteo]: juzga por API".into()),
    }
}

/// Candidato de cupo o API con Wilson sobre el umbral y menor costo. Desempate: Wilson, luego nombre.
fn aprendido(p: &Pedido, pol: &Politica, cfg: &Config) -> Option<Candidato> {
    // Los roles de Planifica, Organiza y Construye conservan su modelo de root.toml (invariante 3).
    if !matches!(p.rol, Rol::Delega | Rol::Verifica | Rol::Juzga) {
        return None;
    }
    let umbral = f64::from(cfg.umbral_wilson);
    pol.estadisticas
        .get(&Politica::clave(p.rol, &p.tipo))?
        .iter()
        .filter(|c| matches!(c.via, Via::Cupo | Via::Api) && c.wilson_inf >= umbral)
        .min_by(|a, b| {
            a.usd_medio
                .total_cmp(&b.usd_medio)
                .then(b.wilson_inf.total_cmp(&a.wilson_inf))
                .then(a.modelo.cmp(&b.modelo))
        })
        .cloned()
}

fn opcion_cupo(alias: &str, motivo: String) -> Opcion {
    Opcion {
        via: Via::Cupo,
        proveedor: Some(Proveedor::Anthropic),
        modelo: alias.into(),
        motivo,
    }
}

fn opcion_api(m: &ModeloApi, motivo: String) -> Opcion {
    Opcion {
        via: Via::Api,
        proveedor: Some(m.proveedor),
        modelo: m.modelo.clone(),
        motivo,
    }
}

/// Equivalente de API de un alias de cupo, si `root.toml` lo define.
fn equivalente<'a>(cfg: &'a Config, alias: &str) -> Option<&'a ModeloApi> {
    cfg.ruteo.api_equivalente.get(alias)
}

fn estado_cupo(uso: &Uso, cfg: &Config, ahora: f64) -> Cupo {
    let tope = cfg.balance.cupo.tope_usd;
    let fin = uso
        .cupo_bloque_inicio
        .map(|inicio| inicio + f64::from(cfg.balance.cupo.bloque_horas) * 3600.0);
    // Sin inicio no hay bloque, así que el cupo no está lleno.
    let vencido = fin.is_some_and(|f| ahora >= f);
    let usd = if vencido { 0.0 } else { uso.cupo_bloque_usd };
    Cupo {
        usd,
        tope,
        lleno: !vencido && fin.is_some() && usd >= tope,
        fin,
    }
}

/// Error con el próximo momento de reapertura: el menor entre las vías prohibidas.
fn sin_presupuesto(
    elegida: &Opcion,
    cupo: Cupo,
    api_ok: bool,
    ahora: f64,
    offset_s: i32,
) -> SinPresupuesto {
    let mut reabres = Vec::new();
    if let Some(fin) = cupo.fin.filter(|_| cupo.lleno) {
        reabres.push(fin);
    }
    if !api_ok {
        reabres.push(proximo_mes(ahora, offset_s));
    }
    let reabre = reabres.into_iter().fold(f64::INFINITY, f64::min);
    let motivo = match elegida.via {
        Via::Cupo if api_ok => format!(
            "cupo del bloque al 100% y sin equivalente de API para {}",
            elegida.modelo
        ),
        _ => "cupo del bloque y API del mes sin margen".into(),
    };
    SinPresupuesto { reabre, motivo }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::tipos::{Pedido, Proveedor, Rol, Uso, Via};

    /// 2026-10-15T12:00:00Z.
    const AHORA: f64 = 1_792_065_600.0;
    /// 2026-11-01T00:00:00Z.
    const PROXIMO_MES_UTC: f64 = 1_793_491_200.0;
    const HORA: f64 = 3600.0;

    fn pedido(rol: Rol, checkeable: bool) -> Pedido {
        Pedido {
            rol,
            tipo: "bulk".into(),
            checkeable,
            tokens_estimados: 1000,
        }
    }

    fn uso(cupo: f64, inicio: Option<f64>, api: f64) -> Uso {
        Uso {
            cupo_bloque_usd: cupo,
            cupo_bloque_inicio: inicio,
            api_mes_usd: api,
            nube_mes_usd: 0.0,
        }
    }

    /// Bloque de cupo que empezó hace una hora.
    fn bloque_vigente() -> Option<f64> {
        Some(AHORA - HORA)
    }

    fn candidato(modelo: &str, via: Via, wilson: f64, usd: f64) -> Candidato {
        Candidato {
            modelo: modelo.into(),
            proveedor: Some(Proveedor::Deepseek),
            via,
            n: 50,
            exitos: 40,
            wilson_inf: wilson,
            usd_medio: usd,
        }
    }

    fn politica_con(clave: &str, candidatos: Vec<Candidato>) -> Politica {
        let mut pol = Politica::default();
        pol.estadisticas.insert(clave.into(), candidatos);
        pol
    }

    fn elegir_en(
        p: &Pedido,
        pol: &Politica,
        u: &Uso,
        cfg: &Config,
    ) -> Result<crate::tipos::Eleccion, crate::tipos::SinPresupuesto> {
        elegir(p, pol, u, cfg, AHORA, 0)
    }

    #[test]
    fn route_checkeable_da_oraculo_aunque_topes_agotados() {
        let cfg = Config::default();
        let agotado = uso(35.0, bloque_vigente(), 100.0);
        for rol in [
            Rol::Planifica,
            Rol::Organiza,
            Rol::Construye,
            Rol::Verifica,
            Rol::Juzga,
            Rol::Delega,
        ] {
            let e = elegir_en(&pedido(rol, true), &Politica::default(), &agotado, &cfg)
                .expect("checkeable nunca devuelve SinPresupuesto");
            assert_eq!(e.via, Via::Oraculo, "rol {rol:?}");
            assert_eq!(e.modelo, None);
            assert_eq!(e.proveedor, None);
            assert!(!e.razonamiento);
            assert!(!e.motivo.is_empty());
        }
    }

    #[test]
    fn route_verifica_y_juzga_siempre_razonan() {
        let cfg = Config::default();
        let usos = [
            uso(0.0, None, 0.0),
            uso(32.0, bloque_vigente(), 10.0),
            uso(35.0, bloque_vigente(), 10.0),
            uso(0.0, None, 100.0),
            uso(35.0, bloque_vigente(), 100.0),
        ];
        let politicas = [
            Politica::default(),
            politica_con(
                "verifica:bulk",
                vec![candidato("modelo-barato", Via::Api, 0.99, 0.0001)],
            ),
            politica_con(
                "juzga:bulk",
                vec![candidato("sin-razonamiento", Via::Api, 0.99, 0.0)],
            ),
        ];
        for rol in [Rol::Verifica, Rol::Juzga] {
            for u in &usos {
                for pol in &politicas {
                    if let Ok(e) = elegir_en(&pedido(rol, false), pol, u, &cfg) {
                        assert!(e.razonamiento, "rol {rol:?} con uso {u:?} sin razonamiento");
                    }
                }
            }
        }
    }

    #[test]
    fn route_cupo_sobre_90_por_ciento_pasa_a_api_equivalente() {
        let cfg = Config::default();
        let e = elegir_en(
            &pedido(Rol::Organiza, false),
            &Politica::default(),
            &uso(32.0, bloque_vigente(), 10.0),
            &cfg,
        )
        .expect("hay margen en la API");
        assert_eq!(e.via, Via::Api);
        assert_eq!(e.proveedor, Some(Proveedor::Anthropic));
        assert_eq!(e.modelo.as_deref(), Some("claude-sonnet-5-5"));
    }

    #[test]
    fn route_cupo_lleno_con_api_con_margen_pasa_a_api() {
        let cfg = Config::default();
        let e = elegir_en(
            &pedido(Rol::Planifica, false),
            &Politica::default(),
            &uso(35.0, bloque_vigente(), 40.0),
            &cfg,
        )
        .expect("API con margen");
        assert_eq!(e.via, Via::Api);
        assert_eq!(e.proveedor, Some(Proveedor::Anthropic));
        assert_eq!(e.modelo.as_deref(), Some("claude-opus-5-5"));
    }

    #[test]
    fn route_cupo_y_api_agotados_reabre_con_fin_de_bloque() {
        let cfg = Config::default();
        let inicio = AHORA - HORA;
        let fin = inicio + 5.0 * 3600.0;
        let err = elegir_en(
            &pedido(Rol::Planifica, false),
            &Politica::default(),
            &uso(35.0, Some(inicio), 100.0),
            &cfg,
        )
        .expect_err("cupo y API agotados");
        assert_eq!(err.reabre, fin.min(PROXIMO_MES_UTC));
        assert_eq!(err.reabre, fin);
        assert!(!err.motivo.is_empty());
    }

    #[test]
    fn route_cupo_y_api_agotados_reabre_con_inicio_de_mes() {
        let cfg = Config::default();
        // 2026-10-31T23:00:00Z: el mes cierra en una hora; el bloque dura cinco.
        let ahora = PROXIMO_MES_UTC - HORA;
        let err = elegir(
            &pedido(Rol::Planifica, false),
            &Politica::default(),
            &uso(35.0, Some(ahora - 60.0), 100.0),
            &cfg,
            ahora,
            0,
        )
        .expect_err("cupo y API agotados");
        assert_eq!(err.reabre, PROXIMO_MES_UTC);
    }

    #[test]
    fn route_api_agotada_delega_pasa_a_cupo_con_organiza() {
        let cfg = Config::default();
        let e = elegir_en(
            &pedido(Rol::Delega, false),
            &Politica::default(),
            &uso(10.0, bloque_vigente(), 100.0),
            &cfg,
        )
        .expect("cupo con margen");
        assert_eq!(e.via, Via::Cupo);
        assert_eq!(e.proveedor, Some(Proveedor::Anthropic));
        assert_eq!(e.modelo.as_deref(), Some("sonnet"));
        assert!(!e.razonamiento);
    }

    #[test]
    fn route_rol_de_cupo_sin_equivalente_queda_en_cupo_hasta_100() {
        let mut cfg = Config::default();
        cfg.ruteo.api_equivalente.clear();
        let e = elegir_en(
            &pedido(Rol::Planifica, false),
            &Politica::default(),
            &uso(32.0, bloque_vigente(), 10.0),
            &cfg,
        )
        .expect("sigue en cupo");
        assert_eq!(e.via, Via::Cupo);
        assert_eq!(e.modelo.as_deref(), Some("opus"));
        assert!(
            elegir_en(
                &pedido(Rol::Planifica, false),
                &Politica::default(),
                &uso(35.0, bloque_vigente(), 10.0),
                &cfg,
            )
            .is_err(),
            "al 100% sin equivalente no hay vía"
        );
    }

    #[test]
    fn route_planifica_organiza_construye_respetan_modelos() {
        let cfg = Config::default();
        let pol = Politica::default();
        let mut pol_mejor = pol.clone();
        for rol in ["planifica", "organiza", "construye"] {
            pol_mejor.estadisticas.insert(
                format!("{rol}:bulk"),
                vec![candidato("deepseek-chat", Via::Api, 0.99, 0.0)],
            );
        }
        let casos = [
            (Rol::Planifica, "opus"),
            (Rol::Organiza, "sonnet"),
            (Rol::Construye, "haiku"),
        ];
        for (rol, alias) in casos {
            let e = elegir_en(&pedido(rol, false), &pol_mejor, &uso(0.0, None, 0.0), &cfg)
                .expect("con margen");
            assert_eq!(e.via, Via::Cupo, "rol {rol:?}");
            assert_eq!(e.modelo.as_deref(), Some(alias), "rol {rol:?}");
        }
    }

    #[test]
    fn route_delega_elige_el_mas_barato_sobre_umbral() {
        let cfg = Config::default();
        let pol = politica_con(
            "delega:bulk",
            vec![
                candidato("modelo-a", Via::Api, 0.7, 0.001),
                candidato("modelo-b", Via::Api, 0.6, 0.0005),
                candidato("modelo-c", Via::Api, 0.3, 0.0001),
            ],
        );
        let e = elegir_en(
            &pedido(Rol::Delega, false),
            &pol,
            &uso(0.0, None, 0.0),
            &cfg,
        )
        .expect("hay margen");
        assert_eq!(e.modelo.as_deref(), Some("modelo-b"));
        assert_eq!(e.via, Via::Api);
    }

    #[test]
    fn route_delega_sin_candidato_sobre_umbral_usa_default() {
        let cfg = Config::default();
        let pol = politica_con(
            "delega:bulk",
            vec![candidato("modelo-c", Via::Api, 0.3, 0.0001)],
        );
        let e = elegir_en(
            &pedido(Rol::Delega, false),
            &pol,
            &uso(0.0, None, 0.0),
            &cfg,
        )
        .expect("hay margen");
        assert_eq!(e.modelo.as_deref(), Some("deepseek-chat"));
        assert_eq!(e.proveedor, Some(Proveedor::Deepseek));
    }

    #[test]
    fn route_nube_no_cambia_la_eleccion() {
        let cfg = Config::default();
        let pol = politica_con(
            "delega:bulk",
            vec![candidato("modelo-b", Via::Api, 0.9, 0.0005)],
        );
        for rol in [Rol::Planifica, Rol::Delega, Rol::Verifica, Rol::Juzga] {
            for (cupo, api) in [(0.0, 0.0), (32.0, 10.0), (35.0, 40.0)] {
                let sin_nube = uso(cupo, bloque_vigente(), api);
                let con_nube = Uso {
                    nube_mes_usd: 250.0,
                    ..sin_nube.clone()
                };
                let a = elegir_en(&pedido(rol, false), &pol, &sin_nube, &cfg);
                let b = elegir_en(&pedido(rol, false), &pol, &con_nube, &cfg);
                assert_eq!(a, b, "rol {rol:?}");
                if let Ok(e) = b {
                    assert_ne!(e.via, Via::Nube);
                }
            }
        }
    }

    #[test]
    fn route_proximo_mes_pasa_de_diciembre_a_enero() {
        // 2026-12-15T12:00:00Z -> 2027-01-01T00:00:00Z
        let dic = 1_797_336_000.0;
        assert_eq!(proximo_mes(dic, 0), 1_798_761_600.0);
    }

    #[test]
    fn route_proximo_mes_en_utc_cae_el_primero_a_las_cero() {
        assert_eq!(proximo_mes(AHORA, 0), PROXIMO_MES_UTC);
    }

    #[test]
    fn route_proximo_mes_con_offset_corre_el_borde() {
        // UTC+2 el 2026-10-15 14:00 local: el mes cierra 2026-10-31T22:00Z.
        assert_eq!(proximo_mes(AHORA, 7200), PROXIMO_MES_UTC - 7200.0);
        // UTC-5 el 2026-10-31 18:00 local: el mes cierra 2026-11-01T05:00Z.
        let ahora = PROXIMO_MES_UTC - HORA;
        assert_eq!(proximo_mes(ahora, -18_000), PROXIMO_MES_UTC + 5.0 * HORA);
    }

    #[test]
    fn route_inicio_mes_en_utc_es_el_primero_a_las_cero() {
        // 2026-10-15T12:00:00Z -> 2026-10-01T00:00:00Z
        assert_eq!(inicio_mes(AHORA, 0), 1_790_812_800.0);
    }

    #[test]
    fn route_inicio_mes_en_el_primer_segundo_es_ese_mismo_instante() {
        let primer_segundo = 1_790_812_800.0;
        assert_eq!(inicio_mes(primer_segundo, 0), primer_segundo);
    }

    #[test]
    fn route_inicio_mes_en_diciembre_no_pasa_de_mes() {
        // 2026-12-15T12:00:00Z -> 2026-12-01T00:00:00Z
        let dic = 1_797_336_000.0;
        assert_eq!(inicio_mes(dic, 0), 1_796_083_200.0);
    }

    #[test]
    fn route_inicio_mes_con_offset_corre_el_borde() {
        // UTC-3: 2026-10-01 00:00 local son las 03:00Z.
        assert_eq!(inicio_mes(AHORA, -10_800), 1_790_812_800.0 + 3.0 * HORA);
        // UTC+2: el 2026-11-01 01:00Z ya es el 1 de noviembre local a las 03:00.
        // El mes empezó el 2026-11-01 00:00 local, que es 2026-10-31T22:00Z.
        let ahora = PROXIMO_MES_UTC + HORA;
        assert_eq!(inicio_mes(ahora, 7200), PROXIMO_MES_UTC - 7200.0);
    }

    #[test]
    fn route_uso_sinpresupuesto_y_politica_ida_y_vuelta_json() {
        let u = Uso {
            cupo_bloque_usd: 12.5,
            cupo_bloque_inicio: Some(AHORA),
            api_mes_usd: 3.0,
            nube_mes_usd: 10.0,
        };
        let texto = serde_json::to_string(&u).expect("serializa");
        assert_eq!(serde_json::from_str::<Uso>(&texto).expect("lee"), u);

        let s = crate::tipos::SinPresupuesto {
            reabre: AHORA,
            motivo: "cupo y API agotados".into(),
        };
        let texto = serde_json::to_string(&s).expect("serializa");
        assert_eq!(
            serde_json::from_str::<crate::tipos::SinPresupuesto>(&texto).expect("lee"),
            s
        );

        let pol = politica_con("delega:bulk", vec![candidato("m", Via::Api, 0.8, 0.1)]);
        let texto = serde_json::to_string(&pol).expect("serializa");
        assert_eq!(serde_json::from_str::<Politica>(&texto).expect("lee"), pol);
    }

    #[test]
    fn route_via_nube_serializa_nube() {
        assert_eq!(serde_json::to_value(Via::Nube).expect("serializa"), "nube");
    }

    #[test]
    fn route_clave_de_politica_usa_rol_en_snake_case() {
        assert_eq!(Politica::clave(Rol::Verifica, "bulk"), "verifica:bulk");
        assert_eq!(Politica::clave(Rol::Planifica, "tests"), "planifica:tests");
        assert_eq!(Politica::default().v, 1);
    }

    #[test]
    fn route_sin_presupuesto_sale_con_codigo_3() {
        let error = crate::error::Error::from(crate::tipos::SinPresupuesto {
            reabre: AHORA,
            motivo: "agotado".into(),
        });
        assert_eq!(error.codigo_de_salida(), 3);
    }
}
