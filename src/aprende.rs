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
use crate::route::politica::{
    Diferencia, Evidencia, EvidenciaForma, EvidenciaModelo, Propuesta, PropuestaForma, nombre_rol,
};
use crate::route::{Candidato, Politica};
use crate::tipos::{Decision, Estado, Evento, Fila, Forma, Proveedor, Rol, TipoEvento, Via};
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

/// Modelos de `root.toml` para los roles fijos, y el umbral de Wilson con el que se propone.
#[derive(Debug, Clone, PartialEq)]
pub struct RolesActuales {
    pub planifica: String,
    pub organiza: String,
    pub construye: String,
    pub umbral_wilson: f64,
}

impl Default for RolesActuales {
    fn default() -> Self {
        RolesActuales {
            planifica: "opus".into(),
            organiza: "sonnet".into(),
            construye: "haiku".into(),
            umbral_wilson: 0.6,
        }
    }
}

impl RolesActuales {
    /// Modelos y umbral de `root.toml`.
    pub fn de_config(cfg: &Config) -> Self {
        RolesActuales {
            planifica: cfg.modelos.planifica.clone(),
            organiza: cfg.modelos.organiza.clone(),
            construye: cfg.modelos.construye.clone(),
            umbral_wilson: cfg.umbral_wilson,
        }
    }
}

/// Recalcula la política desde las filas y los eventos. Función pura: no escribe y no lee el reloj.
///
/// Las filas se ordenan por id antes de sumar. Así el resultado no depende del orden de entrada.
pub fn recalcular(filas: &[Fila], eventos: &[Evento], actuales: &RolesActuales) -> Politica {
    let mut refs: Vec<&Fila> = filas.iter().collect();
    refs.sort_by(|a, b| a.id.cmp(&b.id).then(a.ts.total_cmp(&b.ts)));
    let mut pol = politica_de(&refs, eventos);
    pol.propuestas = propuestas_de(&refs, actuales);
    pol.propuestas_forma = propuestas_forma_de(&refs, actuales);
    pol
}

/// Filas mínimas por modelo para proponer un cambio. Con menos, no hay comparación.
const FILAS_PROPUESTA: u32 = 3;

/// Acumulado de un modelo en una tarea de un rol fijo.
#[derive(Default)]
struct AcumEvidencia {
    ids: Vec<String>,
    exitos: u32,
    usd: f64,
}

impl AcumEvidencia {
    /// Cifras del acumulado: n, éxitos, cota de Wilson, USD medio e ids ordenados.
    fn cifras(self) -> (u32, u32, f64, f64, Vec<String>) {
        let n = u32::try_from(self.ids.len()).unwrap_or(u32::MAX);
        let mut filas = self.ids;
        filas.sort();
        (
            n,
            self.exitos,
            wilson_inferior(self.exitos, n),
            self.usd / f64::from(n),
            filas,
        )
    }

    fn evidencia(self, modelo: &str) -> EvidenciaModelo {
        let (n, exitos, wilson_inf, usd_medio, filas) = self.cifras();
        EvidenciaModelo {
            modelo: modelo.into(),
            n,
            exitos,
            wilson_inf,
            usd_medio,
            filas,
        }
    }

    fn evidencia_forma(self, forma: Forma) -> EvidenciaForma {
        let (n, exitos, wilson_inf, usd_medio, filas) = self.cifras();
        EvidenciaForma {
            forma,
            n,
            exitos,
            wilson_inf,
            usd_medio,
            filas,
        }
    }
}

/// Modelo de `root.toml` para un rol fijo. Solo Planifica, Organiza y Construye llegan aquí.
fn modelo_actual(rol: Rol, actuales: &RolesActuales) -> &str {
    match rol {
        Rol::Planifica => &actuales.planifica,
        Rol::Organiza => &actuales.organiza,
        _ => &actuales.construye,
    }
}

/// Tasa de éxito y USD medio de una evidencia, sea de modelo o de forma. Es lo que comparan las propuestas.
trait TasaYCosto {
    fn tasa_y_costo(&self) -> (f64, f64);
}

impl TasaYCosto for EvidenciaModelo {
    fn tasa_y_costo(&self) -> (f64, f64) {
        (f64::from(self.exitos) / f64::from(self.n), self.usd_medio)
    }
}

impl TasaYCosto for EvidenciaForma {
    fn tasa_y_costo(&self) -> (f64, f64) {
        (f64::from(self.exitos) / f64::from(self.n), self.usd_medio)
    }
}

fn tasa<E: TasaYCosto>(e: &E) -> f64 {
    e.tasa_y_costo().0
}

/// Verdadero si `propuesto` acierta más que `actual`, o acierta igual y cuesta menos.
fn mejora_sobre<E: TasaYCosto>(propuesto: &E, actual: &E) -> bool {
    let ((tp, up), (ta, ua)) = (propuesto.tasa_y_costo(), actual.tasa_y_costo());
    tp > ta || (tp == ta && up < ua)
}

/// Mayor tasa de éxito, luego menor costo, luego nombre.
fn orden_propuesta(a: &EvidenciaModelo, b: &EvidenciaModelo) -> std::cmp::Ordering {
    tasa(b)
        .total_cmp(&tasa(a))
        .then(a.usd_medio.total_cmp(&b.usd_medio))
        .then(a.modelo.cmp(&b.modelo))
}

// Propuestas para los roles fijos. Route no las lee: el usuario las aplica en `root.toml`.
// Las propuestas de forma (P18b, decisión 28) viven en `propuestas_forma_de`.
fn propuestas_de(filas: &[&Fila], actuales: &RolesActuales) -> Vec<Propuesta> {
    let mut acum: BTreeMap<(Rol, String), BTreeMap<String, AcumEvidencia>> = BTreeMap::new();
    for fila in filas.iter().filter(|f| !f.resumen.invalida) {
        for rol in ROLES_FILA {
            let Some(modelo) = fila.modelos.get(&rol) else {
                continue;
            };
            let a = acum
                .entry((rol, fila.tarea.clone()))
                .or_default()
                .entry(modelo.clone())
                .or_default();
            a.ids.push(fila.id.clone());
            if exito_de_fila(fila) {
                a.exitos += 1;
            }
            a.usd += fila.resumen.usd_api + fila.resumen.usd_cupo;
        }
    }
    let mut propuestas = Vec::new();
    for ((rol, tarea), modelos) in acum {
        let evidencias: BTreeMap<String, EvidenciaModelo> = modelos
            .into_iter()
            .map(|(modelo, a)| {
                let e = a.evidencia(&modelo);
                (modelo, e)
            })
            .collect();
        let Some(actual) = evidencias.get(modelo_actual(rol, actuales)) else {
            continue;
        };
        if actual.n < FILAS_PROPUESTA {
            continue;
        }
        let mejor = evidencias
            .values()
            .filter(|c| {
                c.modelo != actual.modelo
                    && c.n >= FILAS_PROPUESTA
                    && c.wilson_inf >= actuales.umbral_wilson
                    && mejora_sobre(*c, actual)
            })
            .min_by(|a, b| orden_propuesta(a, b));
        let Some(propuesto) = mejor else {
            continue;
        };
        propuestas.push(Propuesta {
            rol,
            tarea,
            modelo_actual: actual.modelo.clone(),
            modelo_propuesto: propuesto.modelo.clone(),
            diferencia_esperada: Diferencia {
                exito: tasa(propuesto) - tasa(actual),
                usd: actual.usd_medio - propuesto.usd_medio,
            },
            evidencia: Evidencia {
                actual: actual.clone(),
                propuesto: propuesto.clone(),
            },
        });
    }
    propuestas
}

/// Mayor tasa de éxito, luego menor costo, luego la forma. Orden determinista.
fn orden_propuesta_forma(a: &EvidenciaForma, b: &EvidenciaForma) -> std::cmp::Ordering {
    tasa(b)
        .total_cmp(&tasa(a))
        .then(a.usd_medio.total_cmp(&b.usd_medio))
        .then(a.forma.cmp(&b.forma))
}

/// Propuestas de forma por tarea (decisión 28). Route no las lee: el usuario las aplica a mano.
/// Agrupa por (tarea, forma) y compara con la misma regla que `propuestas_de`.
fn propuestas_forma_de(filas: &[&Fila], actuales: &RolesActuales) -> Vec<PropuestaForma> {
    let mut ultima: BTreeMap<&str, &Fila> = BTreeMap::new();
    let mut acum: BTreeMap<&str, BTreeMap<Forma, AcumEvidencia>> = BTreeMap::new();
    for &fila in filas.iter().filter(|f| !f.resumen.invalida) {
        let tarea = fila.tarea.as_str();
        // Forma actual: la de la última fila de la tarea (decisión del organizador; pregunta abierta al planner).
        let reciente = ultima.get(tarea).is_none_or(|previa| {
            fila.ts
                .total_cmp(&previa.ts)
                .then_with(|| fila.id.cmp(&previa.id))
                .is_gt()
        });
        if reciente {
            ultima.insert(tarea, fila);
        }
        let a = acum
            .entry(tarea)
            .or_default()
            .entry(fila.forma.clone())
            .or_default();
        a.ids.push(fila.id.clone());
        if exito_de_fila(fila) {
            a.exitos += 1;
        }
        a.usd += fila.resumen.usd_api + fila.resumen.usd_cupo;
    }
    let mut propuestas = Vec::new();
    for (tarea, formas) in acum {
        let Some(reciente) = ultima.get(tarea) else {
            continue;
        };
        let evidencias: BTreeMap<Forma, EvidenciaForma> = formas
            .into_iter()
            .map(|(forma, a)| (forma.clone(), a.evidencia_forma(forma)))
            .collect();
        let Some(actual) = evidencias.get(&reciente.forma) else {
            continue;
        };
        if actual.n < FILAS_PROPUESTA {
            continue;
        }
        let mejor = evidencias
            .values()
            .filter(|c| {
                c.forma != actual.forma
                    && c.n >= FILAS_PROPUESTA
                    && c.wilson_inf >= actuales.umbral_wilson
                    && mejora_sobre(*c, actual)
            })
            .min_by(|a, b| orden_propuesta_forma(a, b));
        let Some(propuesta) = mejor else {
            continue;
        };
        propuestas.push(PropuestaForma {
            tarea: tarea.to_string(),
            actual: actual.clone(),
            propuesta: propuesta.clone(),
            diferencia_esperada: Diferencia {
                exito: tasa(propuesta) - tasa(actual),
                usd: actual.usd_medio - propuesta.usd_medio,
            },
        });
    }
    propuestas
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
        propuestas: Vec::new(),
        propuestas_forma: Vec::new(),
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
    let umbral = cfg.umbral_wilson;

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
    use crate::tipos::{Forma, Informe, Resumen, Veredicto};
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
        let pol = recalcular(&filas, &[], &RolesActuales::default());
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
        let pol = recalcular(&filas, &[], &RolesActuales::default());
        assert_eq!(
            modelo_preferido(&pol, Rol::Construye, "memora", "sonnet", 0.5),
            "sonnet"
        );
    }

    #[test]
    fn aprende_con_el_umbral_por_defecto_2_de_2_no_entra_y_3_de_3_si() {
        let umbral = Config::default().umbral_wilson;
        let dos = repetir(2, |i| {
            con_ts(
                fila_construye("haiku", 0.5, true),
                i as f64,
                &format!("d{i}"),
            )
        });
        let pol = recalcular(&dos, &[], &RolesActuales::default());
        assert_eq!(
            modelo_preferido(&pol, Rol::Construye, "memora", "sonnet", umbral),
            "sonnet"
        );
        let tres = repetir(3, |i| {
            con_ts(
                fila_construye("haiku", 0.5, true),
                i as f64,
                &format!("t{i}"),
            )
        });
        let pol = recalcular(&tres, &[], &RolesActuales::default());
        assert_eq!(
            modelo_preferido(&pol, Rol::Construye, "memora", "sonnet", umbral),
            "haiku"
        );
    }

    #[test]
    fn aprende_con_el_umbral_por_defecto_3_de_4_no_entra() {
        let umbral = Config::default().umbral_wilson;
        let mut filas = repetir(3, |i| {
            con_ts(
                fila_construye("haiku", 0.5, true),
                i as f64,
                &format!("c{i}"),
            )
        });
        filas.push(con_ts(fila_construye("haiku", 0.5, false), 3.0, "c3"));
        let pol = recalcular(&filas, &[], &RolesActuales::default());
        assert_eq!(
            modelo_preferido(&pol, Rol::Construye, "memora", "sonnet", umbral),
            "sonnet"
        );
    }

    #[test]
    fn aprende_sin_datos_da_el_default() {
        let pol = recalcular(&[], &[], &RolesActuales::default());
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
        let pol = recalcular(&filas, &[], &RolesActuales::default());
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
        let pol = recalcular(&filas, &[], &RolesActuales::default());
        let c = candidato_de(&pol, "construye:memora", "haiku");
        assert_eq!((c.n, c.exitos), (2, 1));
    }

    #[test]
    fn aprende_sin_informe_usa_el_estado() {
        let filas = vec![
            fila_construye("haiku", 0.5, true),
            fila_construye("haiku", 0.5, false),
        ];
        let pol = recalcular(&filas, &[], &RolesActuales::default());
        let c = candidato_de(&pol, "construye:memora", "haiku");
        assert_eq!((c.n, c.exitos), (2, 1));
    }

    #[test]
    fn aprende_filas_invalidas_no_cuentan() {
        let mut mala = fila_construye("haiku", 0.5, true);
        mala.resumen.invalida = true;
        let pol = recalcular(&[mala], &[], &RolesActuales::default());
        assert!(pol.estadisticas.is_empty());
    }

    #[test]
    fn aprende_fila_sin_modelo_del_rol_no_aporta() {
        let sin_modelo = Fila {
            tarea: "memora".into(),
            ..Fila::default()
        };
        let pol = recalcular(&[sin_modelo], &[], &RolesActuales::default());
        assert!(pol.estadisticas.is_empty());
    }

    #[test]
    fn aprende_alias_de_cupo_lleva_proveedor_anthropic() {
        let pol = recalcular(&diez("haiku", 0.5), &[], &RolesActuales::default());
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
        let pol = recalcular(&[], &eventos, &RolesActuales::default());
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
        let pol = recalcular(&[], &eventos, &RolesActuales::default());
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
        let a = recalcular(&filas, &eventos, &RolesActuales::default());
        filas.reverse();
        let b = recalcular(
            &filas,
            &eventos.iter().rev().cloned().collect::<Vec<_>>(),
            &RolesActuales::default(),
        );
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
    }

    #[test]
    fn aprende_escribir_y_leer_ida_y_vuelta_sin_tmp() {
        let dir = TempDir::new().expect("temp dir");
        let pol = recalcular(&diez("haiku", 0.5), &[], &RolesActuales::default());
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

    /// Fila de la tarea "memora" con `modelo` en `rol`, `usd` de cupo, éxito y `id`.
    fn fila_de_rol(rol: Rol, modelo: &str, usd: f64, exito: bool, id: &str) -> Fila {
        let mut f = Fila {
            id: id.into(),
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
        f.modelos.insert(rol, modelo.into());
        f
    }

    /// `n` filas de `modelo` en `rol`, de las que `exitos` aciertan. Ids: `prefijo` y un número.
    fn serie(
        rol: Rol,
        prefijo: &str,
        modelo: &str,
        n: usize,
        exitos: usize,
        usd: f64,
    ) -> Vec<Fila> {
        (0..n)
            .map(|i| fila_de_rol(rol, modelo, usd, i < exitos, &format!("{prefijo}{i:02}")))
            .collect()
    }

    /// Única propuesta de la política. Falla si hay otra cantidad.
    fn unica(pol: &Politica) -> &Propuesta {
        assert_eq!(pol.propuestas.len(), 1, "propuestas: {:?}", pol.propuestas);
        &pol.propuestas[0]
    }

    /// Diez de `haiku` (0.5 USD) y diez de `sonnet` (2 USD) para organiza. Sonnet es el actual.
    fn organiza_haiku_y_sonnet() -> Vec<Fila> {
        let mut filas = serie(Rol::Organiza, "h", "haiku", 10, 10, 0.5);
        filas.extend(serie(Rol::Organiza, "s", "sonnet", 10, 10, 2.0));
        filas
    }

    #[test]
    fn aprende_caso_del_plan_propone_haiku_para_organiza() {
        let pol = recalcular(&organiza_haiku_y_sonnet(), &[], &RolesActuales::default());
        let p = unica(&pol);
        assert_eq!(p.rol, Rol::Organiza);
        assert_eq!(p.tarea, "memora");
        assert_eq!(p.modelo_actual, "sonnet");
        assert_eq!(p.modelo_propuesto, "haiku");
        let (act, prop) = (&p.evidencia.actual, &p.evidencia.propuesto);
        assert_eq!((act.modelo.as_str(), act.n, act.exitos), ("sonnet", 10, 10));
        assert_eq!(
            (prop.modelo.as_str(), prop.n, prop.exitos),
            ("haiku", 10, 10)
        );
        assert!((prop.wilson_inf - 0.858931).abs() < 1e-5);
        assert!((act.wilson_inf - 0.858931).abs() < 1e-5);
        assert!((act.usd_medio - 2.0).abs() < 1e-12);
        assert!((prop.usd_medio - 0.5).abs() < 1e-12);
        assert_eq!(
            prop.filas,
            (0..10).map(|i| format!("h{i:02}")).collect::<Vec<_>>()
        );
        assert_eq!(
            act.filas,
            (0..10).map(|i| format!("s{i:02}")).collect::<Vec<_>>()
        );
        assert!((p.diferencia_esperada.usd - 1.5).abs() < 1e-12);
        assert_eq!(p.diferencia_esperada.exito, 0.0);
    }

    #[test]
    fn aprende_caso_del_plan_route_organiza_sigue_con_sonnet() {
        let pol = recalcular(&organiza_haiku_y_sonnet(), &[], &RolesActuales::default());
        assert_eq!(unica(&pol).modelo_propuesto, "haiku");
        let pedido = crate::tipos::Pedido {
            rol: Rol::Organiza,
            tipo: "memora".into(),
            checkeable: false,
            tokens_estimados: 1000,
        };
        let uso = crate::tipos::Uso {
            cupo_bloque_usd: 0.0,
            cupo_bloque_inicio: None,
            api_mes_usd: 0.0,
            nube_mes_usd: 0.0,
        };
        let eleccion =
            crate::route::elegir(&pedido, &pol, &uso, &Config::default(), 1_792_065_600.0, 0)
                .expect("elige");
        assert_eq!(eleccion.modelo.as_deref(), Some("sonnet"));
    }

    #[test]
    fn aprende_propone_tambien_para_construye() {
        let mut filas = serie(Rol::Construye, "h", "haiku", 10, 10, 2.0);
        filas.extend(serie(Rol::Construye, "s", "sonnet", 10, 10, 0.5));
        let pol = recalcular(&filas, &[], &RolesActuales::default());
        let p = unica(&pol);
        assert_eq!(
            (p.rol, p.modelo_actual.as_str(), p.modelo_propuesto.as_str()),
            (Rol::Construye, "haiku", "sonnet")
        );
    }

    #[test]
    fn aprende_con_2_filas_por_modelo_no_propone() {
        let mut filas = serie(Rol::Organiza, "h", "haiku", 2, 2, 0.5);
        filas.extend(serie(Rol::Organiza, "s", "sonnet", 2, 2, 2.0));
        let pol = recalcular(&filas, &[], &RolesActuales::default());
        assert!(pol.propuestas.is_empty());
    }

    #[test]
    fn aprende_con_3_y_3_filas_propone() {
        let mut filas = serie(Rol::Organiza, "h", "haiku", 3, 3, 0.5);
        filas.extend(serie(Rol::Organiza, "s", "sonnet", 3, 3, 2.0));
        let pol = recalcular(&filas, &[], &RolesActuales::default());
        let p = unica(&pol);
        assert_eq!(p.modelo_propuesto, "haiku");
        assert_eq!(p.evidencia.propuesto.n, 3);
    }

    #[test]
    fn aprende_actual_con_2_filas_y_propuesto_con_10_no_propone() {
        let mut filas = serie(Rol::Organiza, "s", "sonnet", 2, 2, 2.0);
        filas.extend(serie(Rol::Organiza, "h", "haiku", 10, 10, 0.5));
        let pol = recalcular(&filas, &[], &RolesActuales::default());
        assert!(pol.propuestas.is_empty());
    }

    #[test]
    fn aprende_propuesto_que_acierta_menos_no_propone() {
        // Haiku acierta 19 de 20 (0.95). Sonnet acierta 10 de 10 (1.0).
        let mut filas = serie(Rol::Organiza, "s", "sonnet", 10, 10, 2.0);
        filas.extend(serie(Rol::Organiza, "h", "haiku", 20, 19, 0.5));
        let pol = recalcular(&filas, &[], &RolesActuales::default());
        assert!(pol.propuestas.is_empty());
    }

    #[test]
    fn aprende_propuesto_que_acierta_igual_y_cuesta_igual_o_mas_no_propone() {
        let mut filas = serie(Rol::Organiza, "s", "sonnet", 10, 10, 2.0);
        filas.extend(serie(Rol::Organiza, "h", "haiku", 10, 10, 2.0));
        filas.extend(serie(Rol::Organiza, "o", "opus", 10, 10, 3.0));
        let pol = recalcular(&filas, &[], &RolesActuales::default());
        assert!(pol.propuestas.is_empty());
    }

    #[test]
    fn aprende_propuesto_que_acierta_mejor_aunque_cueste_mas_propone() {
        // Sonnet acierta 5 de 10 y cuesta 0.5. Opus acierta 10 de 10 y cuesta 3.
        let mut filas = serie(Rol::Organiza, "s", "sonnet", 10, 5, 0.5);
        filas.extend(serie(Rol::Organiza, "o", "opus", 10, 10, 3.0));
        let pol = recalcular(&filas, &[], &RolesActuales::default());
        let p = unica(&pol);
        assert_eq!(p.modelo_propuesto, "opus");
        assert!((p.diferencia_esperada.exito - 0.5).abs() < 1e-12);
        assert!((p.diferencia_esperada.usd - -2.5).abs() < 1e-12);
    }

    #[test]
    fn aprende_filas_invalidas_no_cuentan_para_la_propuesta() {
        let mut filas = serie(Rol::Organiza, "s", "sonnet", 10, 10, 2.0);
        let mut haiku = serie(Rol::Organiza, "h", "haiku", 10, 10, 0.5);
        for f in haiku.iter_mut().take(7) {
            f.resumen.invalida = true;
        }
        filas.extend(haiku);
        let pol = recalcular(&filas, &[], &RolesActuales::default());
        let p = unica(&pol);
        assert_eq!(p.evidencia.propuesto.n, 3);
        assert_eq!(p.evidencia.propuesto.filas, vec!["h07", "h08", "h09"]);
    }

    #[test]
    fn aprende_varios_candidatos_gana_la_tasa_mas_alta() {
        // "barato" acierta 19 de 20 y cuesta 0.1. "alto" acierta 20 de 20 y cuesta 0.9.
        let mut filas = serie(Rol::Organiza, "s", "sonnet", 10, 10, 2.0);
        filas.extend(serie(Rol::Organiza, "b", "barato", 20, 19, 0.1));
        filas.extend(serie(Rol::Organiza, "a", "alto", 20, 20, 0.9));
        let pol = recalcular(&filas, &[], &RolesActuales::default());
        assert_eq!(unica(&pol).modelo_propuesto, "alto");
    }

    #[test]
    fn aprende_empate_de_tasa_gana_el_mas_barato() {
        let mut filas = serie(Rol::Organiza, "s", "sonnet", 10, 10, 2.0);
        filas.extend(serie(Rol::Organiza, "a", "alfa", 10, 10, 0.9));
        filas.extend(serie(Rol::Organiza, "z", "zeta", 10, 10, 0.5));
        let pol = recalcular(&filas, &[], &RolesActuales::default());
        assert_eq!(unica(&pol).modelo_propuesto, "zeta");
    }

    #[test]
    fn aprende_empate_de_tasa_y_costo_gana_el_nombre_menor() {
        let mut filas = serie(Rol::Organiza, "s", "sonnet", 10, 10, 2.0);
        filas.extend(serie(Rol::Organiza, "z", "zeta", 10, 10, 0.5));
        filas.extend(serie(Rol::Organiza, "b", "beta", 10, 10, 0.5));
        let pol = recalcular(&filas, &[], &RolesActuales::default());
        assert_eq!(unica(&pol).modelo_propuesto, "beta");
    }

    #[test]
    fn aprende_propuestas_son_deterministas_en_distinto_orden() {
        let mut filas = organiza_haiku_y_sonnet();
        filas.extend(serie(Rol::Organiza, "b", "barato", 12, 11, 0.1));
        let a = recalcular(&filas, &[], &RolesActuales::default());
        filas.reverse();
        let b = recalcular(&filas, &[], &RolesActuales::default());
        assert!(!a.propuestas.is_empty());
        assert_eq!(a, b);
    }

    #[test]
    fn aprende_umbral_de_roles_actuales_veta_la_propuesta() {
        // Haiku 3 de 3 tiene Wilson 0.646. Con umbral 0.7 no alcanza.
        let mut filas = serie(Rol::Organiza, "s", "sonnet", 3, 3, 2.0);
        filas.extend(serie(Rol::Organiza, "h", "haiku", 3, 3, 0.5));
        let actuales = RolesActuales {
            umbral_wilson: 0.7,
            ..RolesActuales::default()
        };
        assert!(recalcular(&filas, &[], &actuales).propuestas.is_empty());
    }

    #[test]
    fn aprende_roles_actuales_leen_root_toml() {
        let mut cfg = Config::default();
        cfg.modelos.organiza = "haiku".into();
        cfg.umbral_wilson = 0.7;
        let actuales = RolesActuales::de_config(&cfg);
        assert_eq!(actuales.organiza, "haiku");
        assert_eq!(actuales.umbral_wilson, 0.7);
    }

    #[test]
    fn aprende_wilson_exacto_0_6_entra_con_umbral_de_root_toml() {
        let cfg = Config::desde_texto("umbral_wilson = 0.6").expect("debe parsear");
        let umbral = RolesActuales::de_config(&cfg).umbral_wilson;
        let mut pol = Politica::default();
        pol.estadisticas.insert(
            "construye:memora".into(),
            vec![Candidato {
                modelo: "barato".into(),
                proveedor: None,
                via: crate::tipos::Via::Api,
                n: 5,
                exitos: 3,
                wilson_inf: 0.6,
                usd_medio: 0.0001,
            }],
        );
        assert_eq!(
            modelo_preferido(&pol, Rol::Construye, "memora", "sonnet", umbral),
            "barato"
        );
    }

    #[test]
    fn aprende_umbral_exacto_0_6_de_root_toml_llega_a_roles_actuales() {
        let cfg = Config::desde_texto("umbral_wilson = 0.6").expect("debe parsear");
        assert_eq!(RolesActuales::de_config(&cfg).umbral_wilson, 0.6);
    }

    #[test]
    fn aprende_politica_con_propuestas_ida_y_vuelta() {
        let dir = TempDir::new().expect("temp dir");
        let pol = recalcular(&organiza_haiku_y_sonnet(), &[], &RolesActuales::default());
        assert!(!pol.propuestas.is_empty());
        escribir_politica(dir.path(), &pol).expect("escribe");
        assert_eq!(leer_politica(dir.path()).expect("lee"), pol);
    }

    #[test]
    fn aprende_politica_vieja_sin_propuestas_se_lee() {
        let dir = TempDir::new().expect("temp dir");
        fs::write(
            dir.path().join(NOMBRE_POLITICA),
            r#"{"v":1,"estadisticas":{}}"#,
        )
        .expect("escribe");
        let pol = leer_politica(dir.path()).expect("lee");
        assert!(pol.propuestas.is_empty());
    }

    #[test]
    fn aprende_propuesta_se_serializa_con_sus_campos() {
        // Los tipos de propuesta se construyen a mano desde fuera del módulo de política.
        let ev = EvidenciaModelo {
            modelo: "m".into(),
            n: 3,
            exitos: 3,
            wilson_inf: 0.646,
            usd_medio: 1.0,
            filas: vec!["a".into()],
        };
        let p = Propuesta {
            rol: Rol::Construye,
            tarea: "memora".into(),
            modelo_actual: "haiku".into(),
            modelo_propuesto: "m".into(),
            evidencia: Evidencia {
                actual: ev.clone(),
                propuesto: ev,
            },
            diferencia_esperada: Diferencia {
                exito: 0.0,
                usd: 0.5,
            },
        };
        let texto = serde_json::to_string(&p).expect("serializa");
        assert!(texto.contains("\"modelo_propuesto\":\"m\""));
    }

    /// Forma A: los cuatro roles, con verificador y config "ecc".
    fn forma_a() -> Forma {
        Forma {
            roles: vec![Rol::Planifica, Rol::Organiza, Rol::Construye, Rol::Verifica],
            verificador: true,
            config: "ecc".into(),
        }
    }

    /// Forma B: sin Verifica, sin verificador y con config "ecc".
    fn forma_b() -> Forma {
        Forma {
            roles: vec![Rol::Planifica, Rol::Organiza, Rol::Construye],
            verificador: false,
            config: "ecc".into(),
        }
    }

    /// Fila de "memora" con `forma`, `ts`, `id` y `usd` de cupo. Construye corre en haiku.
    fn fila_de_forma(forma: &Forma, id: &str, ts: f64, usd: f64, exito: bool) -> Fila {
        let mut f = Fila {
            id: id.into(),
            ts,
            tarea: "memora".into(),
            veredicto: Veredicto {
                estado: if exito { Estado::Verde } else { Estado::Rojo },
                ..Veredicto::default()
            },
            resumen: Resumen {
                usd_cupo: usd,
                ..Resumen::default()
            },
            forma: forma.clone(),
            ..Fila::default()
        };
        f.modelos.insert(Rol::Construye, "haiku".into());
        f
    }

    /// `n_b` filas de la forma B (1 USD, ts bajos) y `n_a` de la forma A (2 USD, ts altos). Todas con éxito.
    /// La forma actual es la de la última fila, así que A es la actual.
    fn formas_a_y_b(n_a: usize, n_b: usize) -> Vec<Fila> {
        let mut filas: Vec<Fila> = (0..n_b)
            .map(|i| fila_de_forma(&forma_b(), &format!("b{i}"), 1.0 + i as f64, 1.0, true))
            .collect();
        filas.extend(
            (0..n_a)
                .map(|i| fila_de_forma(&forma_a(), &format!("a{i}"), 6.0 + i as f64, 2.0, true)),
        );
        filas
    }

    /// Roles de `root.toml` con un umbral de Wilson bajo, para que 5 de 5 entre.
    fn umbral_bajo() -> RolesActuales {
        RolesActuales {
            umbral_wilson: 0.4,
            ..RolesActuales::default()
        }
    }

    #[test]
    fn forma_fila_vieja_sin_forma_se_lee() {
        let mut vieja = serde_json::to_value(Fila::default()).expect("serializa");
        vieja.as_object_mut().expect("objeto").remove("forma");
        let fila: Fila = serde_json::from_value(vieja).expect("lee la fila vieja");
        assert_eq!(fila.forma, Forma::default());
    }

    #[test]
    fn forma_sin_verificador_igual_y_mas_barata_da_propuesta_y_route_no_cambia() {
        let pol = recalcular(&formas_a_y_b(5, 5), &[], &umbral_bajo());
        assert_eq!(pol.propuestas_forma.len(), 1, "{:?}", pol.propuestas_forma);
        let p = &pol.propuestas_forma[0];
        assert_eq!(p.tarea, "memora");
        assert_eq!(p.actual.forma, forma_a());
        assert_eq!(p.propuesta.forma, forma_b());
        assert_eq!((p.actual.n, p.actual.exitos), (5, 5));
        assert_eq!((p.propuesta.n, p.propuesta.exitos), (5, 5));
        assert_eq!(p.diferencia_esperada.exito, 0.0);
        assert!((p.diferencia_esperada.usd - 1.0).abs() < 1e-12);
        assert!(pol.propuestas.is_empty());
        // Route no lee `propuestas_forma`: elige lo mismo con esta política que con la vacía.
        let pedido = crate::tipos::Pedido {
            rol: Rol::Construye,
            tipo: "memora".into(),
            checkeable: false,
            tokens_estimados: 1000,
        };
        let uso = crate::tipos::Uso {
            cupo_bloque_usd: 0.0,
            cupo_bloque_inicio: None,
            api_mes_usd: 0.0,
            nube_mes_usd: 0.0,
        };
        let cfg = Config::default();
        let con =
            crate::route::elegir(&pedido, &pol, &uso, &cfg, 1_792_065_600.0, 0).expect("elige");
        let sin = crate::route::elegir(
            &pedido,
            &Politica::default(),
            &uso,
            &cfg,
            1_792_065_600.0,
            0,
        )
        .expect("elige");
        assert_eq!(con, sin);
    }

    #[test]
    fn forma_con_dos_filas_por_forma_no_da_propuesta() {
        let pol = recalcular(&formas_a_y_b(2, 2), &[], &umbral_bajo());
        assert!(pol.propuestas_forma.is_empty());
    }

    #[test]
    fn forma_filas_invalidas_no_cuentan() {
        let mut filas = formas_a_y_b(5, 5);
        for f in filas.iter_mut().filter(|f| f.forma == forma_b()).take(3) {
            f.resumen.invalida = true;
        }
        let pol = recalcular(&filas, &[], &umbral_bajo());
        assert!(pol.propuestas_forma.is_empty());
    }

    #[test]
    fn forma_politica_vieja_sin_propuestas_forma_se_lee() {
        let dir = TempDir::new().expect("temp dir");
        fs::write(
            dir.path().join(NOMBRE_POLITICA),
            r#"{"v":1,"estadisticas":{},"propuestas":[]}"#,
        )
        .expect("escribe");
        let pol = leer_politica(dir.path()).expect("lee");
        assert!(pol.propuestas_forma.is_empty());
    }
}
