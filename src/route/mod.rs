//! Route: elige el modelo y la vía de cada pedido. Función pura, sin E/S.

pub mod comando;
pub mod politica;

pub use politica::{Candidato, Politica};

use time::{Date, Month, OffsetDateTime, PrimitiveDateTime, Time, UtcOffset};

use crate::config::{Config, ModeloApi, Precio};
use crate::oracle::ErrorOraculo;
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
            rol: Some(p.rol),
        });
    }
    // Invariante 2: Verifica y Juzga siempre razonan, con cualquier modelo y cualquier presupuesto.
    let razonamiento = matches!(p.rol, Rol::Verifica | Rol::Juzga);
    let api_ok = uso.api_mes_usd < cfg.balance.api.tope_usd;
    let cupo = estado_cupo(uso, cfg, ahora);
    let mut elegida = opcion_base(p, pol, cfg);
    let cupo = if elegida.via == Via::Cupo {
        cupo_para_pedido(cupo, p, cfg, &elegida.modelo, ahora)
    } else {
        cupo
    };

    // Preferencia: con el cupo sobre `prefiere_api_sobre`, pasa a la API si hay margen y equivalente.
    let preferir_api = elegida.via == Via::Cupo
        && !cupo.lleno
        && cupo.usd >= cfg.balance.prefiere_api_sobre * cupo.tope
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
        // `root.toml` exige un equivalente por cada alias de cupo (ver `Config::validar`).
        // Este caso solo ocurre con una `Config` armada a mano: se devuelve `SinPresupuesto`.
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
        rol: Some(p.rol),
    })
}

/// Modelos de API que admiten razonamiento. Verifica y Juzga solo pueden usar estos (lista cerrada).
const MODELOS_CON_RAZONAMIENTO: &[&str] = &[
    "deepseek-flash",
    "deepseek-v4-pro",
    "claude-haiku-5-5",
    "claude-sonnet-5-5",
    "claude-opus-5-5",
];

/// Cambia una elección de Verifica o Juzga por el respaldo de su rol si el proveedor principal no
/// está disponible: sin clave o con el breaker abierto. Con el proveedor disponible, o sin respaldo,
/// devuelve la elección igual. El respaldo lleva razonamiento y dice por qué entró.
pub fn con_respaldo(e: Eleccion, cfg: &Config, disponible: &dyn Fn(Proveedor) -> bool) -> Eleccion {
    let (Some(rol), Some(principal)) = (e.rol, e.proveedor) else {
        return e;
    };
    if e.via != Via::Api || !matches!(rol, Rol::Verifica | Rol::Juzga) || disponible(principal) {
        return e;
    }
    let ruteo = match rol {
        Rol::Verifica => &cfg.ruteo.verifica,
        _ => &cfg.ruteo.juzga,
    };
    let Some(respaldo) = ruteo.respaldo.as_deref() else {
        return e;
    };
    Eleccion {
        via: Via::Api,
        proveedor: Some(respaldo.proveedor),
        modelo: Some(respaldo.modelo.clone()),
        razonamiento: true,
        motivo: format!(
            "respaldo de [ruteo.{}]: {} no está disponible (sin clave o con el breaker abierto); usa {}",
            politica::nombre_rol(rol),
            comando::etiqueta_proveedor(principal),
            respaldo.modelo
        ),
        rol: Some(rol),
    }
}

/// Un modelo de API que Verifica o Juzga usan debe admitir razonamiento. Fuera de la lista cerrada,
/// es error de elección (código 2). Las demás elecciones pasan.
pub fn exigir_modelo_que_razona(e: &Eleccion) -> Result<(), ErrorOraculo> {
    if e.via != Via::Api || !matches!(e.rol, Some(Rol::Verifica | Rol::Juzga)) {
        return Ok(());
    }
    let modelo = e.modelo.as_deref().unwrap_or_default();
    if MODELOS_CON_RAZONAMIENTO.contains(&modelo) {
        return Ok(());
    }
    Err(ErrorOraculo::Eleccion(format!(
        "el modelo {modelo:?} no admite razonamiento para {}; los modelos válidos son {}",
        e.rol.map_or("-", politica::nombre_rol),
        MODELOS_CON_RAZONAMIENTO.join(", ")
    )))
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
    let umbral = cfg.umbral_wilson;
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

/// Costo en USD de un pedido de `tokens` de entrada y `tokens` de salida, al precio dado. Con la
/// entrada por encima del umbral del tramo alto, cuenta al precio alto.
fn costo_de_tokens(precio: &Precio, tokens: u32) -> f64 {
    let (entrada, salida) = precio.tarifa_para(u64::from(tokens));
    f64::from(tokens) * (entrada + salida) / 1_000_000.0
}

/// Cupo para un pedido (M2). Si el gasto del bloque más el costo estimado del pedido pasa el tope,
/// el cupo cuenta como lleno para ese pedido. Sin precio para el equivalente, no hay estimación.
fn cupo_para_pedido(cupo: Cupo, p: &Pedido, cfg: &Config, alias: &str, ahora: f64) -> Cupo {
    let Some(precio) = equivalente(cfg, alias).and_then(|eq| cfg.precios.get(&eq.modelo)) else {
        return cupo;
    };
    if cupo.usd + costo_de_tokens(precio, p.tokens_estimados) <= cupo.tope {
        return cupo;
    }
    // El bloque del pedido empieza ahora si no hay uno vigente: así el reabre queda en el futuro.
    let dur = f64::from(cfg.balance.cupo.bloque_horas) * 3600.0;
    Cupo {
        lleno: true,
        fin: Some(cupo.fin.filter(|fin| *fin > ahora).unwrap_or(ahora + dur)),
        ..cupo
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
                candidato("modelo-b", Via::Api, 0.65, 0.0005),
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
        assert_eq!(e.modelo.as_deref(), Some("deepseek-flash"));
        assert_eq!(e.proveedor, Some(Proveedor::Deepseek));
    }

    #[test]
    fn route_umbral_wilson_por_defecto_es_0_6() {
        let umbral: f64 = Config::default().umbral_wilson;
        assert_eq!(umbral, 0.6_f64);
    }

    #[test]
    fn route_cupo_en_0_9_por_35_exacto_pasa_a_api_con_root_toml() {
        // 0,9 × 35,0 = 31,5 exacto. Con f32 el redondeo cae por debajo y este caso pasa igual;
        // el de 0,8 (route_cupo_en_0_8...) es el que expone el error de f32.
        let cfg = Config::desde_texto("[balance]\nprefiere_api_sobre = 0.9").expect("debe parsear");
        let e = elegir_en(
            &pedido(Rol::Organiza, false),
            &Politica::default(),
            &uso(31.5, bloque_vigente(), 10.0),
            &cfg,
        )
        .expect("hay margen en la API");
        assert_eq!(e.via, Via::Api);
    }

    #[test]
    fn route_wilson_exacto_0_6_entra_con_umbral_de_root_toml() {
        let cfg = Config::desde_texto("umbral_wilson = 0.6").expect("debe parsear");
        let pol = politica_con(
            "delega:bulk",
            vec![candidato("barato", Via::Api, 0.6, 0.0001)],
        );
        let e = elegir_en(
            &pedido(Rol::Delega, false),
            &pol,
            &uso(0.0, None, 0.0),
            &cfg,
        )
        .expect("hay margen");
        assert_eq!(e.modelo.as_deref(), Some("barato"));
    }

    #[test]
    fn route_cupo_en_0_8_por_35_exacto_pasa_a_api_con_root_toml() {
        // 0,8 × 35,0 = 28,0 exacto: con el umbral en f32 el cupo queda por debajo y no pasa.
        let cfg = Config::desde_texto("[balance]\nprefiere_api_sobre = 0.8").expect("debe parsear");
        let e = elegir_en(
            &pedido(Rol::Organiza, false),
            &Politica::default(),
            &uso(28.0, bloque_vigente(), 10.0),
            &cfg,
        )
        .expect("hay margen en la API");
        assert_eq!(e.via, Via::Api);
    }

    #[test]
    fn route_candidato_de_dos_de_dos_no_entra_con_el_umbral_por_defecto() {
        let cfg = Config::default();
        let wilson = crate::track::stats::wilson_inferior(2, 2);
        assert!((wilson - 0.549).abs() < 1e-3, "wilson {wilson}");
        let casos = [
            (Rol::Delega, "delega:bulk", "deepseek-flash"),
            (Rol::Verifica, "verifica:bulk", "deepseek-flash"),
            (Rol::Juzga, "juzga:bulk", "deepseek-flash"),
        ];
        for (rol, clave, default) in casos {
            let pol = politica_con(clave, vec![candidato("barato", Via::Api, wilson, 0.0001)]);
            let e = elegir_en(&pedido(rol, false), &pol, &uso(0.0, None, 0.0), &cfg)
                .expect("hay margen");
            assert_eq!(e.modelo.as_deref(), Some(default), "rol {rol:?}");
        }
    }

    #[test]
    fn route_candidato_de_tres_de_tres_entra_con_el_umbral_por_defecto() {
        let cfg = Config::default();
        let wilson = crate::track::stats::wilson_inferior(3, 3);
        assert!((wilson - 0.646).abs() < 1e-3, "wilson {wilson}");
        let pol = politica_con(
            "delega:bulk",
            vec![candidato("barato", Via::Api, wilson, 0.0001)],
        );
        let e = elegir_en(
            &pedido(Rol::Delega, false),
            &pol,
            &uso(0.0, None, 0.0),
            &cfg,
        )
        .expect("hay margen");
        assert_eq!(e.modelo.as_deref(), Some("barato"));
    }

    #[test]
    fn route_candidato_de_tres_de_cuatro_no_entra_con_el_umbral_por_defecto() {
        let cfg = Config::default();
        let wilson = crate::track::stats::wilson_inferior(3, 4);
        assert!((wilson - 0.43).abs() < 1e-2, "wilson {wilson}");
        let pol = politica_con(
            "delega:bulk",
            vec![candidato("barato", Via::Api, wilson, 0.0001)],
        );
        let e = elegir_en(
            &pedido(Rol::Delega, false),
            &pol,
            &uso(0.0, None, 0.0),
            &cfg,
        )
        .expect("hay margen");
        assert_eq!(e.modelo.as_deref(), Some("deepseek-flash"));
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

    /// Estadísticas y propuestas de modelo y de forma: Route no las lee, y la elección no debe cambiar.
    #[test]
    fn route_propuestas_no_cambian_la_eleccion_en_ningun_rol() {
        use crate::route::politica::{
            Diferencia, Evidencia, EvidenciaForma, EvidenciaModelo, Propuesta, PropuestaForma,
        };
        let cfg = Config::default();
        let u = uso(10.0, bloque_vigente(), 0.0);
        let mut con = Politica::default();
        for rol in [
            Rol::Planifica,
            Rol::Organiza,
            Rol::Construye,
            Rol::Verifica,
            Rol::Juzga,
            Rol::Delega,
        ] {
            con.estadisticas.insert(
                Politica::clave(rol, "bulk"),
                vec![
                    candidato("haiku", Via::Api, 0.9, 0.001),
                    candidato("sonnet", Via::Api, 0.5, 0.01),
                ],
            );
        }
        let modelo = |m: &str, exito: f64| EvidenciaModelo {
            modelo: m.into(),
            n: 50,
            exitos: 40,
            wilson_inf: exito,
            usd_medio: 0.01,
            filas: Vec::new(),
        };
        con.propuestas.push(Propuesta {
            rol: Rol::Organiza,
            tarea: "memora".into(),
            modelo_actual: "sonnet".into(),
            modelo_propuesto: "haiku".into(),
            evidencia: Evidencia {
                actual: modelo("sonnet", 0.5),
                propuesto: modelo("haiku", 0.9),
            },
            diferencia_esperada: Diferencia {
                exito: 0.4,
                usd: 0.009,
            },
        });
        let forma = |roles: Vec<Rol>| EvidenciaForma {
            forma: crate::tipos::Forma {
                roles,
                verificador: false,
                config: "ecc".into(),
            },
            n: 50,
            exitos: 40,
            wilson_inf: 0.7,
            usd_medio: 0.01,
            filas: Vec::new(),
        };
        con.propuestas_forma.push(PropuestaForma {
            tarea: "memora".into(),
            actual: forma(vec![Rol::Planifica, Rol::Organiza, Rol::Construye]),
            propuesta: forma(vec![Rol::Planifica]),
            diferencia_esperada: Diferencia {
                exito: 0.0,
                usd: 0.005,
            },
        });
        let mut sin = con.clone();
        sin.propuestas.clear();
        sin.propuestas_forma.clear();
        for rol in [
            Rol::Planifica,
            Rol::Organiza,
            Rol::Construye,
            Rol::Verifica,
            Rol::Juzga,
            Rol::Delega,
        ] {
            let p = pedido(rol, false);
            let con_eleccion = elegir_en(&p, &con, &u, &cfg)
                .ok()
                .map(|e| (e.via, e.modelo));
            let sin_eleccion = elegir_en(&p, &sin, &u, &cfg)
                .ok()
                .map(|e| (e.via, e.modelo));
            assert!(con_eleccion.is_some(), "{rol:?} sin elección");
            assert_eq!(con_eleccion, sin_eleccion, "{rol:?}");
        }
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

    fn pedido_con_tokens(rol: Rol, tokens: u32) -> Pedido {
        Pedido {
            tokens_estimados: tokens,
            ..pedido(rol, false)
        }
    }

    /// Config con el precio de Opus (alias `opus` de cupo): 15 USD de entrada y 75 de salida por millón.
    /// Prefiere la API solo al tope lleno, para aislar el chequeo del pedido.
    fn cfg_con_precio_de_opus() -> Config {
        Config::desde_texto(
            "[balance]\nprefiere_api_sobre = 1.0\n\n[precios.claude-opus-5-5]\nusd_entrada = 15.0\nusd_salida = 75.0\n",
        )
        .expect("debe parsear")
    }

    #[test]
    fn route_pedido_grande_con_cupo_casi_lleno_pasa_a_api() {
        // 1 millón de tokens de entrada y de salida cuestan 90 USD: 34,99 + 90 pasan el tope de 35.
        let e = elegir_en(
            &pedido_con_tokens(Rol::Planifica, 1_000_000),
            &Politica::default(),
            &uso(34.99, bloque_vigente(), 10.0),
            &cfg_con_precio_de_opus(),
        )
        .expect("hay margen en la API");
        assert_eq!(e.via, Via::Api);
        assert_eq!(e.modelo.as_deref(), Some("claude-opus-5-5"));
    }

    #[test]
    fn route_pedido_chico_con_cupo_casi_lleno_sigue_en_cupo() {
        // 100 tokens cuestan 0,009 USD: 34,99 + 0,009 no pasan el tope.
        let e = elegir_en(
            &pedido_con_tokens(Rol::Planifica, 100),
            &Politica::default(),
            &uso(34.99, bloque_vigente(), 10.0),
            &cfg_con_precio_de_opus(),
        )
        .expect("sigue en cupo");
        assert_eq!(e.via, Via::Cupo);
    }

    #[test]
    fn route_sin_precio_del_equivalente_no_estima_y_sigue_en_cupo() {
        let cfg =
            Config::desde_texto("[balance]\nprefiere_api_sobre = 1.0\n").expect("debe parsear");
        let e = elegir_en(
            &pedido_con_tokens(Rol::Planifica, 1_000_000),
            &Politica::default(),
            &uso(34.99, bloque_vigente(), 10.0),
            &cfg,
        )
        .expect("sigue en cupo");
        assert_eq!(e.via, Via::Cupo);
    }

    #[test]
    fn route_pedido_que_pasa_el_cupo_con_api_llena_no_reabre_en_el_pasado() {
        // El bloque venció hace seis horas. El pedido no entra y la API del mes está llena.
        let err = elegir_en(
            &pedido_con_tokens(Rol::Planifica, 1_000_000),
            &Politica::default(),
            &uso(0.0, Some(AHORA - 6.0 * HORA), 100.0),
            &cfg_con_precio_de_opus(),
        )
        .expect_err("cupo y API sin margen");
        assert!(err.reabre > AHORA, "reabre {} debe ser futuro", err.reabre);
    }

    #[test]
    fn route_costo_de_tokens_suma_entrada_y_salida() {
        let precio = crate::config::Precio {
            usd_entrada: 15.0,
            usd_salida: 75.0,
            umbral_entrada: None,
            usd_entrada_alto: None,
            usd_salida_alto: None,
        };
        assert_eq!(costo_de_tokens(&precio, 1_000_000), 90.0);
    }

    #[test]
    fn route_sin_presupuesto_sale_con_codigo_3() {
        let error = crate::error::Error::from(crate::tipos::SinPresupuesto {
            reabre: AHORA,
            motivo: "agotado".into(),
        });
        assert_eq!(error.codigo_de_salida(), 3);
    }

    fn eleccion_api(rol: Rol, proveedor: Proveedor, modelo: &str) -> Eleccion {
        Eleccion {
            via: Via::Api,
            proveedor: Some(proveedor),
            modelo: Some(modelo.into()),
            razonamiento: true,
            motivo: "prueba".into(),
            rol: Some(rol),
        }
    }

    #[test]
    fn route_eleccion_lleva_el_rol_del_pedido() {
        let cfg = Config::default();
        for rol in [
            Rol::Planifica,
            Rol::Organiza,
            Rol::Construye,
            Rol::Verifica,
            Rol::Juzga,
            Rol::Delega,
        ] {
            for checkeable in [false, true] {
                let e = elegir_en(
                    &pedido(rol, checkeable),
                    &Politica::default(),
                    &uso(0.0, None, 0.0),
                    &cfg,
                )
                .expect("hay vía");
                assert_eq!(e.rol, Some(rol), "rol {rol:?} checkeable {checkeable}");
            }
        }
    }

    #[test]
    fn route_con_respaldo_sin_clave_juzga_usa_haiku_con_razonamiento() {
        let cfg = Config::default();
        let e = eleccion_api(Rol::Juzga, Proveedor::Deepseek, "deepseek-flash");
        let sin_deepseek = |p: Proveedor| p != Proveedor::Deepseek;
        let r = con_respaldo(e, &cfg, &sin_deepseek);
        assert_eq!(r.via, Via::Api);
        assert_eq!(r.proveedor, Some(Proveedor::Anthropic));
        assert_eq!(r.modelo.as_deref(), Some("claude-haiku-5-5"));
        assert!(r.razonamiento);
        assert_eq!(r.rol, Some(Rol::Juzga));
        assert!(r.motivo.contains("respaldo"), "{}", r.motivo);
        assert!(r.motivo.contains("deepseek"), "{}", r.motivo);
    }

    #[test]
    fn route_con_respaldo_breaker_abierto_juzga_usa_respaldo() {
        let cfg = Config::default();
        let e = eleccion_api(Rol::Juzga, Proveedor::Deepseek, "deepseek-flash");
        let breaker_abierto = |_: Proveedor| false;
        let r = con_respaldo(e, &cfg, &breaker_abierto);
        assert_eq!(r.modelo.as_deref(), Some("claude-haiku-5-5"));
    }

    #[test]
    fn route_con_respaldo_verifica_usa_su_respaldo() {
        let cfg = Config::default();
        let e = eleccion_api(Rol::Verifica, Proveedor::Deepseek, "deepseek-flash");
        let r = con_respaldo(e, &cfg, &|_| false);
        assert_eq!(r.proveedor, Some(Proveedor::Anthropic));
        assert_eq!(r.rol, Some(Rol::Verifica));
    }

    #[test]
    fn route_con_respaldo_con_proveedor_disponible_no_cambia() {
        let cfg = Config::default();
        let e = eleccion_api(Rol::Juzga, Proveedor::Deepseek, "deepseek-flash");
        let r = con_respaldo(e.clone(), &cfg, &|_| true);
        assert_eq!(r, e);
    }

    #[test]
    fn route_con_respaldo_delega_sin_clave_no_cambia() {
        let cfg = Config::default();
        let e = eleccion_api(Rol::Delega, Proveedor::Deepseek, "deepseek-flash");
        let r = con_respaldo(e.clone(), &cfg, &|_| false);
        assert_eq!(r, e);
    }

    #[test]
    fn route_con_respaldo_cupo_y_oraculo_no_cambian() {
        let cfg = Config::default();
        let cupo = Eleccion {
            via: Via::Cupo,
            proveedor: Some(Proveedor::Anthropic),
            modelo: Some("haiku".into()),
            razonamiento: false,
            motivo: "prueba".into(),
            rol: Some(Rol::Juzga),
        };
        assert_eq!(con_respaldo(cupo.clone(), &cfg, &|_| false), cupo);
    }

    #[test]
    fn route_modelos_que_razonan_pasan_la_lista_cerrada() {
        for modelo in [
            "deepseek-flash",
            "deepseek-v4-pro",
            "claude-haiku-5-5",
            "claude-sonnet-5-5",
            "claude-opus-5-5",
        ] {
            for rol in [Rol::Verifica, Rol::Juzga] {
                let e = eleccion_api(rol, Proveedor::Deepseek, modelo);
                assert!(
                    exigir_modelo_que_razona(&e).is_ok(),
                    "{modelo} para {rol:?}"
                );
            }
        }
    }

    #[test]
    fn route_modelo_fuera_de_la_lista_para_verifica_o_juzga_da_error_de_eleccion() {
        for rol in [Rol::Verifica, Rol::Juzga] {
            let e = eleccion_api(rol, Proveedor::Deepseek, "deepseek-reasoner");
            let error = exigir_modelo_que_razona(&e).expect_err("fuera de la lista");
            assert!(matches!(error, ErrorOraculo::Eleccion(_)), "{error}");
            assert_eq!(error.codigo_de_salida(), 2);
        }
    }

    #[test]
    fn route_delega_no_exige_la_lista_de_razonamiento() {
        let e = eleccion_api(Rol::Delega, Proveedor::Deepseek, "deepseek-chat");
        assert!(exigir_modelo_que_razona(&e).is_ok());
    }

    #[test]
    fn route_respaldo_fuera_de_la_lista_da_error_de_eleccion() {
        let cfg = Config::desde_texto(
            "[ruteo.juzga]\nproveedor = \"deepseek\"\nmodelo = \"deepseek-flash\"\n\
             [ruteo.juzga.respaldo]\nproveedor = \"anthropic\"\nmodelo = \"claude-foo\"\n",
        )
        .expect("carga");
        let e = eleccion_api(Rol::Juzga, Proveedor::Deepseek, "deepseek-flash");
        let elegida = con_respaldo(e, &cfg, &|_| false);
        assert_eq!(elegida.modelo.as_deref(), Some("claude-foo"));
        assert!(exigir_modelo_que_razona(&elegida).is_err());
    }
}
