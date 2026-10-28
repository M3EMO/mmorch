//! Aprende: calcula la política de ruteo desde las filas y los eventos. El núcleo es puro.
//! Solo `escribir_politica`, `leer_politica` y `leer_eventos` tocan el disco.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Write};
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error as ErrorDerive;

use crate::config::Config;
use crate::observe::jsonl;
use crate::route::politica::nombre_rol;
use crate::route::{Candidato, Politica};
use crate::tipos::{Decision, Estado, Evento, Fila, Proveedor, Rol, TipoEvento, Via};
use crate::track::stats::wilson_inferior;

/// Nombre del archivo de política dentro de `ROOT_HOME`.
pub const NOMBRE_POLITICA: &str = "politica.json";

/// Roles que aprenden de las filas.
const ROLES_FILA: [Rol; 3] = [Rol::Planifica, Rol::Organiza, Rol::Construye];

/// Roles que aprenden de los eventos `Llamada`.
const ROLES_EVENTO: [Rol; 3] = [Rol::Delega, Rol::Verifica, Rol::Juzga];

/// Alias de cupo: `claude -p` los corre, así que llevan proveedor Anthropic.
const ALIAS_CUPO: [&str; 3] = ["opus", "sonnet", "haiku"];

/// Filas mínimas antes de que el replay no avise que hay pocos datos.
const FILAS_MINIMAS: usize = 10;

/// Errores de Aprende. Salen con código 3.
#[derive(Debug, ErrorDerive)]
pub enum ErrorAprende {
    #[error("error de E/S")]
    Io(#[from] io::Error),
    #[error("{ruta}: no se pudo leer")]
    Leer {
        ruta: String,
        #[source]
        fuente: io::Error,
    },
    #[error("{ruta}: JSON inválido")]
    Json {
        ruta: String,
        #[source]
        fuente: serde_json::Error,
    },
    #[error("{archivo}: la línea {linea} es inválida")]
    Linea {
        archivo: String,
        linea: usize,
        #[source]
        fuente: serde_json::Error,
    },
}

/// Métrica de una política sobre las filas de medición que coinciden con su elección.
/// `exito` y `usd_medio` son `None` si no hay filas cubiertas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MetricaPolitica {
    pub cubiertas: u32,
    pub exito: Option<f64>,
    pub usd_medio: Option<f64>,
}

/// Resultado de `aprende --replay`. Mejora positiva en `mejora_usd` significa que la política ahorra.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Replay {
    pub entrenamiento: u32,
    pub medicion: u32,
    pub aprendida: MetricaPolitica,
    pub root_toml: MetricaPolitica,
    pub mejora_exito: Option<f64>,
    pub mejora_usd: Option<f64>,
    pub advertencia: Option<String>,
}

/// Éxito de una fila. Con informe, éxito es `Decision::Listo`. Sin informe, éxito es `estado` verde.
pub fn exito_de_fila(fila: &Fila) -> bool {
    match &fila.veredicto.informe {
        Some(informe) => informe.decision == Decision::Listo,
        None => fila.veredicto.estado == Estado::Verde,
    }
}

/// Orden de preferencia: menor costo, luego mayor Wilson, luego nombre.
fn orden(a: &Candidato, b: &Candidato) -> std::cmp::Ordering {
    a.usd_medio
        .total_cmp(&b.usd_medio)
        .then(b.wilson_inf.total_cmp(&a.wilson_inf))
        .then(a.modelo.cmp(&b.modelo))
}

/// Mejor candidato con Wilson sobre el umbral. Función pura.
pub fn mejor(candidatos: &[Candidato], umbral: f64) -> Option<&Candidato> {
    candidatos
        .iter()
        .filter(|c| c.wilson_inf >= umbral)
        .min_by(|a, b| orden(a, b))
}

/// Modelo que la política aprendida prefiere para `rol` y `tipo`. Si no hay candidato, vale `default`.
pub fn modelo_preferido(
    pol: &Politica,
    rol: Rol,
    tipo: &str,
    default: &str,
    umbral_wilson: f64,
) -> String {
    pol.estadisticas
        .get(&Politica::clave(rol, tipo))
        .and_then(|candidatos| mejor(candidatos, umbral_wilson))
        .map_or_else(|| default.to_string(), |c| c.modelo.clone())
}

/// Recalcula la política desde las filas y los eventos. Función pura: no escribe y no lee el reloj.
pub fn recalcular(filas: &[Fila], eventos: &[Evento]) -> Politica {
    let refs: Vec<&Fila> = filas.iter().collect();
    politica_de(&refs, eventos)
}

/// Acumulado de un grupo: intentos, éxitos y USD totales, con el via y el proveedor del grupo.
struct Acumulado {
    via: Via,
    proveedor: Option<Proveedor>,
    n: u32,
    exitos: u32,
    usd: f64,
}

/// Clave de grupo: clave de política, modelo, via y proveedor en texto. Ordenable y determinista.
type Grupos = BTreeMap<(String, String, String, String), Acumulado>;

fn sumar(
    grupos: &mut Grupos,
    clave: String,
    modelo: &str,
    via: Via,
    proveedor: Option<Proveedor>,
    exito: bool,
    usd: f64,
) {
    let llave = (
        clave,
        modelo.to_string(),
        format!("{via:?}"),
        format!("{proveedor:?}"),
    );
    let acc = grupos.entry(llave).or_insert(Acumulado {
        via,
        proveedor,
        n: 0,
        exitos: 0,
        usd: 0.0,
    });
    acc.n += 1;
    if exito {
        acc.exitos += 1;
    }
    acc.usd += usd;
}

fn politica_de(filas: &[&Fila], eventos: &[Evento]) -> Politica {
    let mut grupos = Grupos::new();
    for fila in filas.iter().filter(|f| !f.resumen.invalida) {
        for rol in ROLES_FILA {
            let Some(modelo) = fila.modelos.get(&rol) else {
                continue;
            };
            let proveedor = ALIAS_CUPO
                .contains(&modelo.as_str())
                .then_some(Proveedor::Anthropic);
            let usd = fila.resumen.usd_api + fila.resumen.usd_cupo;
            sumar(
                &mut grupos,
                Politica::clave(rol, &fila.tarea),
                modelo,
                Via::Cupo,
                proveedor,
                exito_de_fila(fila),
                usd,
            );
        }
    }
    for ev in eventos {
        acumular_evento(&mut grupos, ev);
    }
    let mut estadisticas: BTreeMap<String, Vec<Candidato>> = BTreeMap::new();
    for ((clave, modelo, _, _), acc) in grupos {
        estadisticas.entry(clave).or_default().push(Candidato {
            modelo,
            proveedor: acc.proveedor,
            via: acc.via,
            n: acc.n,
            exitos: acc.exitos,
            wilson_inf: wilson_inferior(acc.exitos, acc.n),
            usd_medio: acc.usd / f64::from(acc.n),
        });
    }
    for candidatos in estadisticas.values_mut() {
        candidatos.sort_by(orden);
    }
    Politica {
        v: crate::tipos::VERSION,
        estadisticas,
    }
}

/// Suma una llamada de un rol de evento. Ignora los eventos sin `extra.rol/tipo/exito`,
/// sin modelo o con una vía que no sea API ni cupo.
fn acumular_evento(grupos: &mut Grupos, ev: &Evento) {
    if ev.tipo != TipoEvento::Llamada {
        return;
    }
    let Some(rol) = rol_de_extra(ev) else {
        return;
    };
    let Some(tipo) = ev.extra.get("tipo").and_then(Value::as_str) else {
        return;
    };
    let Some(exito) = ev.extra.get("exito").and_then(Value::as_bool) else {
        return;
    };
    let (Some(modelo), Some(via)) = (ev.modelo.as_deref(), ev.via) else {
        return;
    };
    if !matches!(via, Via::Api | Via::Cupo) {
        return;
    }
    let proveedor = match ev.familia.as_deref() {
        Some("deepseek") => Some(Proveedor::Deepseek),
        Some("anthropic") => Some(Proveedor::Anthropic),
        _ => None,
    };
    sumar(
        grupos,
        Politica::clave(rol, tipo),
        modelo,
        via,
        proveedor,
        exito,
        ev.cost_usd,
    );
}

/// Rol de evento de `extra.rol`, si es uno de `ROLES_EVENTO`.
fn rol_de_extra(ev: &Evento) -> Option<Rol> {
    let texto = ev.extra.get("rol")?.as_str()?;
    ROLES_EVENTO
        .into_iter()
        .find(|rol| nombre_rol(*rol) == texto)
}

/// Lee `eventos.jsonl` y sus archivos rotados, en orden. Una línea inválida es error.
pub fn leer_eventos(ruta: &Path) -> Result<Vec<Evento>, ErrorAprende> {
    let mut eventos = Vec::new();
    for archivo in jsonl::archivos_en_orden(ruta)? {
        for (indice, linea) in jsonl::leer_texto(&archivo)?.lines().enumerate() {
            if linea.trim().is_empty() {
                continue;
            }
            let ev = serde_json::from_str(linea).map_err(|fuente| ErrorAprende::Linea {
                archivo: archivo.display().to_string(),
                linea: indice + 1,
                fuente,
            })?;
            eventos.push(ev);
        }
    }
    Ok(eventos)
}

/// Escribe `politica.json` en `home` de forma atómica: archivo temporal y `rename`.
pub fn escribir_politica(home: &Path, pol: &Politica) -> io::Result<()> {
    fs::create_dir_all(home)?;
    let mut texto = serde_json::to_string_pretty(pol).map_err(io::Error::other)?;
    texto.push('\n');
    let mut temporal = tempfile::NamedTempFile::new_in(home)?;
    temporal.write_all(texto.as_bytes())?;
    temporal.as_file().sync_all()?;
    temporal
        .persist(home.join(NOMBRE_POLITICA))
        .map_err(|e| e.error)?;
    Ok(())
}

/// Lee `politica.json` de `home`. Si no existe, vale el default. Si no se puede leer o parsear, es error.
pub fn leer_politica(home: &Path) -> Result<Politica, ErrorAprende> {
    let ruta = home.join(NOMBRE_POLITICA);
    match fs::read_to_string(&ruta) {
        Ok(texto) => serde_json::from_str(&texto).map_err(|fuente| ErrorAprende::Json {
            ruta: ruta.display().to_string(),
            fuente,
        }),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Politica::default()),
        Err(fuente) => Err(ErrorAprende::Leer {
            ruta: ruta.display().to_string(),
            fuente,
        }),
    }
}

/// Acumulado de métricas de una política sobre las filas de medición cubiertas.
#[derive(Default)]
struct AcumMetrica {
    n: u32,
    exitos: u32,
    usd: f64,
}

impl AcumMetrica {
    fn sumar(&mut self, fila: &Fila) {
        self.n += 1;
        if exito_de_fila(fila) {
            self.exitos += 1;
        }
        self.usd += fila.resumen.usd_api + fila.resumen.usd_cupo;
    }

    fn metrica(&self) -> MetricaPolitica {
        if self.n == 0 {
            return MetricaPolitica {
                cubiertas: 0,
                exito: None,
                usd_medio: None,
            };
        }
        let n = f64::from(self.n);
        MetricaPolitica {
            cubiertas: self.n,
            exito: Some(f64::from(self.exitos) / n),
            usd_medio: Some(self.usd / n),
        }
    }
}

/// Modelo alias de `root.toml` para un rol de fila.
fn modelo_de_rol(cfg: &Config, rol: Rol) -> &str {
    match rol {
        Rol::Planifica => &cfg.modelos.planifica,
        Rol::Organiza => &cfg.modelos.organiza,
        _ => &cfg.modelos.construye,
    }
}

/// Replay: entrena con el primer 70% de las filas y mide el resto. Función pura.
pub fn replay(filas: &[Fila], cfg: &Config) -> Replay {
    let mut orden_temporal: Vec<&Fila> = filas.iter().collect();
    orden_temporal.sort_by(|a, b| a.ts.total_cmp(&b.ts).then_with(|| a.id.cmp(&b.id)));
    let total = orden_temporal.len();
    let corte = (0.7 * total as f64).floor() as usize;
    let (entrena, mide) = orden_temporal.split_at(corte);
    let pol = politica_de(entrena, &[]);
    let umbral = f64::from(cfg.umbral_wilson);

    let tareas: BTreeSet<&str> = mide
        .iter()
        .filter(|f| !f.resumen.invalida)
        .map(|f| f.tarea.as_str())
        .collect();
    let mut aprendida = AcumMetrica::default();
    let mut root_toml = AcumMetrica::default();
    for rol in ROLES_FILA {
        let default = modelo_de_rol(cfg, rol);
        for tarea in &tareas {
            let eleccion = modelo_preferido(&pol, rol, tarea, default, umbral);
            for fila in mide
                .iter()
                .filter(|f| f.tarea == *tarea && !f.resumen.invalida)
            {
                let Some(modelo) = fila.modelos.get(&rol) else {
                    continue;
                };
                if *modelo == eleccion {
                    aprendida.sumar(fila);
                }
                if modelo.as_str() == default {
                    root_toml.sumar(fila);
                }
            }
        }
    }
    let (aprendida, root_toml) = (aprendida.metrica(), root_toml.metrica());
    let mejora_exito = match (aprendida.exito, root_toml.exito) {
        (Some(a), Some(r)) => Some(a - r),
        _ => None,
    };
    let mejora_usd = match (aprendida.usd_medio, root_toml.usd_medio) {
        (Some(a), Some(r)) => Some(r - a),
        _ => None,
    };
    Replay {
        entrenamiento: u32::try_from(corte).unwrap_or(u32::MAX),
        medicion: u32::try_from(mide.len()).unwrap_or(u32::MAX),
        aprendida,
        root_toml,
        mejora_exito,
        mejora_usd,
        advertencia: (total < FILAS_MINIMAS).then(|| format!("pocos datos: {total} filas")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tipos::{Informe, Resumen, Veredicto};
    use serde_json::json;
    use tempfile::TempDir;

    /// Fila de la tarea "memora" con `modelo` para construye y `usd` de cupo.
    fn fila_construye(modelo: &str, usd: f64, exito: bool) -> Fila {
        let mut f = Fila {
            tarea: "memora".into(),
            veredicto: Veredicto {
                estado: if exito { Estado::Verde } else { Estado::Rojo },
                ..Veredicto::default()
            },
            resumen: Resumen {
                usd_cupo: usd,
                ..Resumen::default()
            },
            ..Fila::default()
        };
        f.modelos.insert(Rol::Construye, modelo.into());
        f
    }

    fn con_ts(mut f: Fila, ts: f64, id: &str) -> Fila {
        f.ts = ts;
        f.id = id.into();
        f
    }

    fn con_informe(mut f: Fila, decision: Decision) -> Fila {
        f.veredicto.informe = Some(Informe {
            decision,
            motivo: String::new(),
            requisitos: Vec::new(),
            bloqueantes: Vec::new(),
            riesgos: Vec::new(),
            proximos_pasos: Vec::new(),
            resumen: String::new(),
        });
        f
    }

    fn repetir(n: usize, f: impl Fn(usize) -> Fila) -> Vec<Fila> {
        (0..n).map(f).collect()
    }

    /// Llamada de un rol de evento con `extra` y costo.
    fn llamada(
        rol: &str,
        tipo: &str,
        modelo: &str,
        via: Via,
        familia: &str,
        exito: Option<bool>,
        costo: f64,
    ) -> Evento {
        let mut extra = serde_json::Map::new();
        extra.insert("rol".into(), json!(rol));
        extra.insert("tipo".into(), json!(tipo));
        if let Some(e) = exito {
            extra.insert("exito".into(), json!(e));
        }
        Evento {
            tipo: TipoEvento::Llamada,
            via: Some(via),
            modelo: Some(modelo.into()),
            familia: Some(familia.into()),
            cost_usd: costo,
            extra,
            corrida: "c".into(),
            ..Evento::default()
        }
    }

    fn candidato_de(pol: &Politica, clave: &str, modelo: &str) -> Candidato {
        pol.estadisticas[clave]
            .iter()
            .find(|c| c.modelo == modelo)
            .cloned()
            .expect("hay candidato")
    }

    /// Diez filas de `modelo` (todas con éxito) con USD por fila.
    fn diez(modelo: &str, usd: f64) -> Vec<Fila> {
        repetir(10, |i| {
            con_ts(
                fila_construye(modelo, usd, true),
                i as f64,
                &format!("{modelo}-{i}"),
            )
        })
    }

    #[test]
    fn aprende_caso_del_plan_elige_haiku_por_costo() {
        let mut filas = diez("haiku", 0.5);
        filas.extend(diez("sonnet", 2.0));
        let pol = recalcular(&filas, &[]);
        let candidatos = &pol.estadisticas[&Politica::clave(Rol::Construye, "memora")];
        assert_eq!(candidatos.len(), 2);
        let h = candidato_de(&pol, "construye:memora", "haiku");
        assert_eq!(h.n, 10);
        assert_eq!(h.exitos, 10);
        assert!((h.wilson_inf - 0.858931).abs() < 1e-5);
        assert_eq!(
            modelo_preferido(&pol, Rol::Construye, "memora", "sonnet", 0.5),
            "haiku"
        );
    }

    #[test]
    fn aprende_haiku_con_3_de_10_vuelve_al_default() {
        let mut filas: Vec<Fila> = repetir(3, |i| {
            con_ts(
                fila_construye("haiku", 0.5, true),
                i as f64,
                &format!("h{i}"),
            )
        });
        filas.extend(repetir(7, |i| {
            con_ts(
                fila_construye("haiku", 0.5, false),
                10.0 + i as f64,
                &format!("g{i}"),
            )
        }));
        filas.extend(diez("sonnet", 2.0));
        let pol = recalcular(&filas, &[]);
        assert_eq!(
            modelo_preferido(&pol, Rol::Construye, "memora", "sonnet", 0.5),
            "sonnet"
        );
    }

    #[test]
    fn aprende_sin_datos_da_el_default() {
        let pol = recalcular(&[], &[]);
        assert!(pol.estadisticas.is_empty());
        assert_eq!(
            modelo_preferido(&pol, Rol::Construye, "memora", "sonnet", 0.5),
            "sonnet"
        );
    }

    #[test]
    fn aprende_wilson_de_1_de_2_no_alcanza_el_umbral() {
        let filas = vec![
            con_ts(fila_construye("haiku", 0.5, true), 1.0, "a"),
            con_ts(fila_construye("haiku", 0.5, false), 2.0, "b"),
        ];
        let pol = recalcular(&filas, &[]);
        assert!(
            (candidato_de(&pol, "construye:memora", "haiku").wilson_inf - 0.164252).abs() < 1e-5
        );
        assert_eq!(
            modelo_preferido(&pol, Rol::Construye, "memora", "sonnet", 0.5),
            "sonnet"
        );
    }

    #[test]
    fn aprende_listo_cuenta_como_exito_y_no_listo_como_fracaso() {
        let filas = vec![
            con_informe(fila_construye("haiku", 0.5, true), Decision::NoListo),
            con_informe(fila_construye("haiku", 0.5, true), Decision::Listo),
        ];
        let pol = recalcular(&filas, &[]);
        let c = candidato_de(&pol, "construye:memora", "haiku");
        assert_eq!((c.n, c.exitos), (2, 1));
    }

    #[test]
    fn aprende_sin_informe_usa_el_estado() {
        let filas = vec![
            fila_construye("haiku", 0.5, true),
            fila_construye("haiku", 0.5, false),
        ];
        let pol = recalcular(&filas, &[]);
        let c = candidato_de(&pol, "construye:memora", "haiku");
        assert_eq!((c.n, c.exitos), (2, 1));
    }

    #[test]
    fn aprende_filas_invalidas_no_cuentan() {
        let mut mala = fila_construye("haiku", 0.5, true);
        mala.resumen.invalida = true;
        let pol = recalcular(&[mala], &[]);
        assert!(pol.estadisticas.is_empty());
    }

    #[test]
    fn aprende_fila_sin_modelo_del_rol_no_aporta() {
        let sin_modelo = Fila {
            tarea: "memora".into(),
            ..Fila::default()
        };
        let pol = recalcular(&[sin_modelo], &[]);
        assert!(pol.estadisticas.is_empty());
    }

    #[test]
    fn aprende_alias_de_cupo_lleva_proveedor_anthropic() {
        let pol = recalcular(&diez("haiku", 0.5), &[]);
        let c = candidato_de(&pol, "construye:memora", "haiku");
        assert_eq!(c.proveedor, Some(Proveedor::Anthropic));
        assert_eq!(c.via, Via::Cupo);
    }

    #[test]
    fn aprende_eventos_eligen_el_barato_con_exito() {
        let mut eventos = Vec::new();
        for i in 0..12 {
            eventos.push(llamada(
                "delega",
                "resumen",
                "deepseek-chat",
                Via::Api,
                "deepseek",
                Some(i != 0),
                0.001,
            ));
            eventos.push(llamada(
                "delega",
                "resumen",
                "claude-haiku",
                Via::Api,
                "anthropic",
                Some(true),
                0.01,
            ));
        }
        let pol = recalcular(&[], &eventos);
        assert_eq!(
            modelo_preferido(&pol, Rol::Delega, "resumen", "claude-haiku", 0.5),
            "deepseek-chat"
        );
        let d = candidato_de(&pol, "delega:resumen", "deepseek-chat");
        assert_eq!(d.proveedor, Some(Proveedor::Deepseek));
        assert_eq!(d.n, 12);
        assert_eq!(d.exitos, 11);
        assert!((d.usd_medio - 0.001).abs() < 1e-12);
    }

    #[test]
    fn aprende_eventos_sin_campos_o_de_otra_via_se_ignoran() {
        let eventos = vec![
            llamada("delega", "resumen", "m", Via::Api, "deepseek", None, 0.1),
            llamada("delega", "resumen", "m", Via::Api, "deepseek", None, 0.1),
            llamada(
                "verifica",
                "bulk",
                "m",
                Via::Oraculo,
                "deepseek",
                Some(true),
                0.1,
            ),
            llamada(
                "juzga",
                "bulk",
                "m",
                Via::Nube,
                "anthropic",
                Some(true),
                0.1,
            ),
            llamada(
                "construye",
                "bulk",
                "m",
                Via::Api,
                "deepseek",
                Some(true),
                0.1,
            ),
        ];
        let pol = recalcular(&[], &eventos);
        assert!(pol.estadisticas.is_empty());
    }

    #[test]
    fn aprende_determinismo_en_distinto_orden() {
        let mut filas = diez("haiku", 0.5);
        filas.extend(diez("sonnet", 2.0));
        let eventos = vec![llamada(
            "delega",
            "resumen",
            "deepseek-chat",
            Via::Api,
            "deepseek",
            Some(true),
            0.001,
        )];
        let a = recalcular(&filas, &eventos);
        filas.reverse();
        let b = recalcular(&filas, &eventos.iter().rev().cloned().collect::<Vec<_>>());
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
    }

    #[test]
    fn aprende_escribir_y_leer_ida_y_vuelta_sin_tmp() {
        let dir = TempDir::new().expect("temp dir");
        let pol = recalcular(&diez("haiku", 0.5), &[]);
        escribir_politica(dir.path(), &pol).expect("escribe");
        let leida = leer_politica(dir.path()).expect("lee");
        assert_eq!(leida, pol);
        let nombres: Vec<String> = fs::read_dir(dir.path())
            .expect("lista")
            .map(|e| {
                e.expect("entrada")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(nombres, vec![NOMBRE_POLITICA.to_string()]);
    }

    #[test]
    fn aprende_leer_inexistente_da_default() {
        let dir = TempDir::new().expect("temp dir");
        assert_eq!(
            leer_politica(dir.path()).expect("default"),
            Politica::default()
        );
    }

    #[test]
    fn aprende_leer_json_roto_es_error() {
        let dir = TempDir::new().expect("temp dir");
        fs::write(dir.path().join(NOMBRE_POLITICA), "{no es json").expect("escribe");
        assert!(matches!(
            leer_politica(dir.path()),
            Err(ErrorAprende::Json { .. })
        ));
    }

    fn cfg_construye_sonnet() -> Config {
        let mut cfg = Config::default();
        cfg.modelos.construye = "sonnet".into();
        cfg
    }

    /// 20 filas: 14 de entrenamiento (7 haiku y 7 sonnet, todas con éxito) y 6 de medición.
    fn veinte_filas() -> Vec<Fila> {
        let mut filas = Vec::new();
        for i in 0..14 {
            let modelo = if i % 2 == 0 { "haiku" } else { "sonnet" };
            let usd = if i % 2 == 0 { 0.5 } else { 2.0 };
            filas.push(con_ts(
                fila_construye(modelo, usd, true),
                i as f64,
                &format!("e{i:02}"),
            ));
        }
        for i in 0..6 {
            let modelo = if i % 2 == 0 { "haiku" } else { "sonnet" };
            let usd = if i % 2 == 0 { 0.5 } else { 2.0 };
            filas.push(con_ts(
                fila_construye(modelo, usd, true),
                100.0 + i as f64,
                &format!("m{i:02}"),
            ));
        }
        filas
    }

    #[test]
    fn aprende_replay_mide_la_mejora_en_usd() {
        let r = replay(&veinte_filas(), &cfg_construye_sonnet());
        assert_eq!(r.entrenamiento, 14);
        assert_eq!(r.medicion, 6);
        assert_eq!(r.aprendida.cubiertas, 3);
        assert_eq!(r.root_toml.cubiertas, 3);
        assert_eq!(r.aprendida.usd_medio, Some(0.5));
        assert_eq!(r.root_toml.usd_medio, Some(2.0));
        assert_eq!(r.mejora_usd, Some(1.5));
        assert_eq!(r.mejora_exito, Some(0.0));
        assert!(r.advertencia.is_none());
    }

    #[test]
    fn aprende_replay_sin_filas_que_coincidan_da_none() {
        // Siete filas enseñan haiku. Las tres de medición son opus: ni la elección ni el default.
        let filas: Vec<Fila> = (0..10)
            .map(|i| {
                let modelo = if i < 7 { "haiku" } else { "opus" };
                con_ts(
                    fila_construye(modelo, 1.0, true),
                    i as f64,
                    &format!("x{i}"),
                )
            })
            .collect();
        let r = replay(&filas, &cfg_construye_sonnet());
        assert_eq!(r.aprendida.cubiertas, 0);
        assert_eq!(r.aprendida.exito, None);
        assert_eq!(r.aprendida.usd_medio, None);
        assert_eq!(r.mejora_usd, None);
        assert_eq!(r.mejora_exito, None);
    }

    #[test]
    fn aprende_replay_con_5_filas_avisa_pocos_datos() {
        let filas: Vec<Fila> = (0..5)
            .map(|i| {
                con_ts(
                    fila_construye("haiku", 0.5, true),
                    i as f64,
                    &format!("p{i}"),
                )
            })
            .collect();
        let r = replay(&filas, &cfg_construye_sonnet());
        assert_eq!(r.advertencia.as_deref(), Some("pocos datos: 5 filas"));
    }

    #[test]
    fn aprende_replay_es_determinista() {
        let filas = veinte_filas();
        let mut invertidas = filas.clone();
        invertidas.reverse();
        let cfg = cfg_construye_sonnet();
        assert_eq!(replay(&filas, &cfg), replay(&invertidas, &cfg));
    }
}
