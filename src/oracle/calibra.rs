//! Calibración del Oracle. La Tarea 12 escribe `$ROOT_HOME/calibracion.json` con la sensibilidad, la
//! especificidad y la exactitud balanceada de cada dimensión, medidas con defectos conocidos. La Tarea 13
//! lo lee: una dimensión queda `calibrada` si su exactitud balanceada llega a 0,9, y solo esas entran a
//! `global`. Además exige muestra mínima: 5 defectos aplicados y 5 casos limpios.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{SystemTime, UNIX_EPOCH};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use serde_json::{Map, json};

use super::juez::{LimitesPrompt, MAX_ENTRADA_JUEZ_BYTES};
use super::memo::{Alcance, MemoSondas, ReusoSondas};
use super::mutacion::MutanteMuerto;
use super::tarea::Tarea;
use super::{AVISO_TARGET_FRIO, ErrorOraculo, OpcionesOraculo, Partes, Target, partes_de};
use crate::borrado;
use crate::config::{Config, Precio};
use crate::observe;
use crate::proveedores::{INTENTOS_MAX, MAX_TOKENS_JUEZ, con_caso, costo_peor_caso};
use crate::tipos::{Dimension, Eleccion, Evento, Puntaje, Veredicto};

/// Nombre del archivo de calibración dentro de `ROOT_HOME`.
pub const ARCHIVO: &str = "calibracion.json";
/// Versión del formato. Otra versión es un archivo que esta lectura no entiende: error de infraestructura.
pub const VERSION: u32 = 1;
/// Exactitud balanceada desde la que una dimensión queda calibrada.
pub const UMBRAL: f32 = 0.9;
/// Defectos aplicados mínimos para calibrar una dimensión. Con menos, la medida no calibra.
const MIN_DEFECTOS_APLICABLES: u32 = 5;
/// Casos limpios mínimos para calibrar una dimensión. Con menos, la medida no calibra.
const MIN_LIMPIOS: u32 = 5;

/// Una medición de una dimensión de una tarea, con su muestra.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Fila {
    pub tarea: String,
    pub dimension: Dimension,
    /// Casos medidos: los defectos aplicados más los limpios. No cuenta los `no_aplicable`.
    pub n: u32,
    /// Defectos aplicados y medidos. La sensibilidad sale de ellos.
    #[serde(default)]
    pub defectos: u32,
    /// Copias sin alterar medidas. La especificidad sale de ellas.
    #[serde(default)]
    pub limpios: u32,
    /// Casos que no se pudieron aplicar al repo, por ejemplo sin Dockerfile. No entran a las métricas.
    #[serde(default)]
    pub no_aplicable: u32,
    /// Semilla de la muestra: con la misma semilla, la medición se repite igual.
    pub semilla: u64,
    /// Epoch en segundos de la medición. Gana la fila más reciente.
    pub ts: f64,
    #[serde(default)]
    pub sensibilidad: Option<f32>,
    #[serde(default)]
    pub especificidad: Option<f32>,
    #[serde(default)]
    pub exactitud_balanceada: Option<f32>,
    /// Exactitud balanceada de cada ítem de rúbrica, por ejemplo `seguridad.secretos`.
    #[serde(default)]
    pub items: BTreeMap<String, f32>,
    /// `true` si el tope de gasto detuvo la calibración antes de medir todos sus casos (decisión 48). Una
    /// fila incompleta no calibra la dimensión, aunque su exactitud llegue a `UMBRAL`.
    #[serde(default)]
    pub incompleta: bool,
    /// Motivo por el que la dimensión no se midió: la tarea no la aplica. Una fila así no tiene casos ni
    /// métricas, y no calibra la dimensión. `None` en una medición real.
    #[serde(default)]
    pub no_aplica: Option<String>,
    /// Inyecciones inválidas reemplazadas en la calibración, incluidas las del caso agotado.
    #[serde(default)]
    pub reemplazos: u32,
}

/// Contenido de `calibracion.json`. Sin archivo, no hay filas: nada está calibrado.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Calibraciones {
    pub v: u32,
    #[serde(default)]
    pub filas: Vec<Fila>,
}

impl Default for Calibraciones {
    fn default() -> Self {
        Calibraciones {
            v: VERSION,
            filas: Vec::new(),
        }
    }
}

impl Calibraciones {
    /// Lee `home/calibracion.json`. Sin archivo, vacío. Un archivo que no parsea o de otra versión es
    /// error de infraestructura: no hay veredicto.
    pub fn cargar(home: &Path) -> Result<Calibraciones, ErrorOraculo> {
        let ruta = home.join(ARCHIVO);
        let texto = match fs::read_to_string(&ruta) {
            Ok(texto) => texto,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Calibraciones::default()),
            Err(e) => {
                return Err(ErrorOraculo::Infra(format!("leer {}: {e}", ruta.display())));
            }
        };
        let calibracion: Calibraciones = serde_json::from_str(&texto).map_err(|e| {
            ErrorOraculo::Infra(format!("calibración inválida en {}: {e}", ruta.display()))
        })?;
        if calibracion.v != VERSION {
            return Err(ErrorOraculo::Infra(format!(
                "calibración: la versión {} de {} no se entiende",
                calibracion.v,
                ruta.display()
            )));
        }
        Ok(calibracion)
    }

    /// La fila más reciente (por `ts`) de la tarea y la dimensión. Con `ts` iguales gana la última del archivo.
    pub fn de(&self, tarea: &str, dimension: Dimension) -> Option<&Fila> {
        // `max_by` devuelve la última de las iguales: con `ts` empatados gana la del final del archivo.
        self.filas
            .iter()
            .filter(|f| f.tarea == tarea && f.dimension == dimension)
            .max_by(|a, b| a.ts.total_cmp(&b.ts))
    }

    /// Marca cada puntaje con su calibración: `calibrada` si la exactitud balanceada llega a `UMBRAL`.
    /// Sin fila, o con muestra insuficiente, el puntaje queda sin calibrar.
    pub fn aplicar(&self, tarea: &str, puntajes: &mut [Puntaje]) {
        for puntaje in puntajes {
            // Una fila con muestra corta cuenta como si no existiera.
            let fila = self
                .de(tarea, puntaje.dimension)
                .filter(|f| muestra_suficiente(f));
            puntaje.exactitud_balanceada = fila.and_then(|f| f.exactitud_balanceada);
            // Una fila incompleta no calibra: el tope detuvo la medición antes de su último caso.
            let incompleta = fila.is_some_and(|f| f.incompleta);
            puntaje.calibrada =
                !incompleta && puntaje.exactitud_balanceada.is_some_and(|e| e >= UMBRAL);
        }
    }
}

/// Media de los `valor` de las dimensiones calibradas y con veredicto, con pesos iguales. `None` si ninguna.
pub fn global_de(puntajes: &[Puntaje]) -> Option<f32> {
    let valores: Vec<f32> = puntajes
        .iter()
        .filter(|p| p.calibrada && p.sin_veredicto.is_none())
        .map(|p| p.valor)
        .collect();
    (!valores.is_empty()).then(|| valores.iter().sum::<f32>() / valores.len() as f32)
}

pub mod inyectar;

use inyectar::{Defecto, Inyeccion};

mod estables;
mod target;
#[cfg(test)]
mod target_podman;

/// Carpeta dentro de la temporal de la calibración donde la base deja su `target` (Tarea 59).
const NOMBRE_TARGET_BASE: &str = "target-base";

/// Archivo de candado de `calibracion.json`. Una escritura lo toma en exclusiva.
const CANDADO: &str = "calibracion.lock";
/// Carpeta de `ROOT_HOME` con un `.casos.jsonl` por calibración con casos medidos.
const CARPETA_CASOS: &str = "calibracion";

/// Calibra `dimension` de `tarea` con `n` casos y la semilla `semilla`, sobre el repo bueno `repo_bueno`.
/// `evaluador` corre el Oracle sobre una ruta. Es `Sync`: los casos corren de a `CASOS_EN_PARALELO` hilos.
/// Sin defectos para la dimensión, `n` cero o una base sin veredicto, no hay fila.
pub fn calibrar_con(
    tarea: &Tarea,
    dimension: Dimension,
    n: usize,
    semilla: u64,
    repo_bueno: &Path,
    evaluador: &(dyn Fn(&Path) -> Result<Veredicto, ErrorOraculo> + Sync),
) -> Result<Fila, ErrorOraculo> {
    // Sin home no hay corrida de mutación grabada: no hay muertos que dirigir `AssertBorrado`.
    // Sin memo ni reuso, el tipo de caso no cambia nada: el evaluador de la API de tests lo ignora.
    let sin_tipo = |ruta: &Path, _: TipoCaso, _: Target| evaluador(ruta);
    calibrar_medida(
        tarea,
        dimension,
        n,
        semilla,
        repo_bueno,
        &sin_tipo,
        None,
        &Vec::new,
        &|| None,
    )?
    .fila
    .ok_or_else(|| ErrorOraculo::Infra("la calibración no dejó ninguna fila".into()))
}

/// Un renglón de `calibracion/<tarea>-<dimension>-<ts>.casos.jsonl`: un caso medido, en orden.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CasoGuardado {
    /// Índice `i` del bucle de la calibración. Los casos sin defecto aplicable no entran, así que puede haber huecos.
    pub caso: u32,
    /// Nombre en snake_case del defecto inyectado. `None` en un caso limpio.
    #[serde(default)]
    pub defecto: Option<String>,
    /// Ítem de rúbrica que el defecto debe hacer caer. `None` en un caso limpio.
    #[serde(default)]
    pub item: Option<String>,
    /// Ruta del archivo donde quedó el defecto, relativa a la copia. `None` en un caso limpio.
    #[serde(default)]
    pub archivo: Option<String>,
    /// Línea 1-based del defecto; 0 significa el archivo entero. `None` en un caso limpio.
    #[serde(default)]
    pub linea: Option<u32>,
    pub es_defecto: bool,
    /// Si la dimensión calibrada bajó frente a la base en este caso.
    pub detectado: bool,
    /// Motivo por el que la dimensión calibrada quedó sin veredicto en este caso.
    #[serde(default)]
    pub sin_veredicto: Option<String>,
    /// Inyecciones inválidas reemplazadas antes de este caso. `0` si no hubo ninguna.
    #[serde(default)]
    pub reemplazos: u32,
    /// Si el entorno repitió este caso. Hoy ningún mecanismo repite casos: siempre `false`.
    #[serde(default)]
    pub repetido_por_entorno: bool,
    /// `Some(true)`: el caso reusó el `target` de la base. `Some(false)`: compiló en frío. `None`: renglón de antes
    /// de que la calibración midiera el `target` (Tarea 59).
    #[serde(default)]
    pub target_reusado: Option<bool>,
}

/// Resultado de `calibrar_tarea_con`: la fila y la ruta de los casos guardados, si hubo casos medidos.
#[derive(Debug, Clone)]
pub struct Calibrada {
    pub fila: Fila,
    pub casos: Option<PathBuf>,
}

/// Resultado de una medición: la fila (completa, o la de los casos hechos si el tope la detuvo) y, si el
/// tope la detuvo, su motivo. Sin casos medidos, la fila no existe.
struct Medida {
    fila: Option<Fila>,
    detencion: Option<Detencion>,
    /// Los casos medidos, en orden. Vacío si el tope detuvo la calibración antes del primero.
    casos: Vec<Caso>,
}

/// Por qué el tope detuvo una calibración: el tope, el gasto medido, el peor caso que no entró y los
/// casos medidos (decisión 48).
#[derive(Clone)]
struct Detencion {
    tope_usd: f64,
    gasto_usd: f64,
    peor_caso_usd: f64,
    casos: u32,
}

/// Tope ya resuelto para una medición: el peor caso de un caso, el inicio de la calibración y el medidor.
/// El medidor es `Sync`: los reemplazos lo miran desde los hilos de los casos.
struct Tope<'a> {
    usd: f64,
    peor_caso: f64,
    inicio: f64,
    gasto_desde: &'a (dyn Fn(f64) -> Result<f64, ErrorOraculo> + Sync),
    /// Ids de los casos con reserva de API abierta desde el inicio. Un caso en vuelo con reserva ya cuenta en `gasto_desde`.
    casos_con_reserva: &'a (dyn Fn(f64) -> Result<HashSet<String>, ErrorOraculo> + Sync),
}

/// Gasto comprometido: el gasto medido más el peor caso de cada caso en vuelo sin reserva escrita.
/// Un caso con reserva ya cuenta en `gasto`, así que no suma su peor caso otra vez.
fn gasto_comprometido(gasto: f64, peor_caso: f64, en_vuelo: usize, con_reserva: usize) -> f64 {
    gasto + en_vuelo.saturating_sub(con_reserva) as f64 * peor_caso
}

/// Si el gasto desde el inicio más el peor caso de un caso pasa el tope, la detención. Sin tope, nunca.
fn detencion_si_no_entra(
    tope: Option<&Tope>,
    casos: usize,
) -> Result<Option<Detencion>, ErrorOraculo> {
    let Some(tope) = tope else {
        return Ok(None);
    };
    let gasto = (tope.gasto_desde)(tope.inicio)?;
    if gasto + tope.peor_caso > tope.usd {
        return Ok(Some(Detencion {
            tope_usd: tope.usd,
            gasto_usd: gasto,
            peor_caso_usd: tope.peor_caso,
            casos: casos as u32,
        }));
    }
    Ok(None)
}

/// Qué caso evalúa el evaluador de `calibrar_medida`. Un caso con defecto lleva la dimensión calibrada: de
/// ella sale qué sondas reusa de la base.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TipoCaso {
    Base,
    Defecto(Dimension),
    Limpio,
}

/// Las sondas que un caso reusa del memo de la base. Un caso limpio es una copia idéntica: reusa las sondas
/// deterministas, incluida la mutación. Un defecto de Estructura, Arquitectura, Diseño, Seguridad,
/// Completitud o Entrega no cambia la muestra de mutantes: reusa solo la mutación. Pruebas mide la mutación,
/// y Eficiencia no la usa en su muestra: esos defectos corren todo. Entrega y Eficiencia nunca se reusan.
fn reuso_de(tipo: TipoCaso, memo: &std::sync::Arc<MemoSondas>) -> Option<ReusoSondas> {
    let alcance = match tipo {
        TipoCaso::Base => return None,
        TipoCaso::Limpio => Alcance::Deterministas,
        TipoCaso::Defecto(dimension) => {
            let reusa_mutacion = matches!(
                dimension,
                Dimension::Estructura
                    | Dimension::Arquitectura
                    | Dimension::Diseno
                    | Dimension::Seguridad
                    | Dimension::Completitud
                    | Dimension::Entrega
            );
            if !reusa_mutacion {
                return None;
            }
            Alcance::SoloMutacion
        }
    };
    Some(ReusoSondas {
        memo: memo.clone(),
        alcance,
    })
}

/// Casos que corren a la vez con el juez (Tarea 48, parte 2). Los contenedores siguen en serie: cada
/// `podman` toma el turno de `contenedor::turno_de_contenedor`.
pub const CASOS_EN_PARALELO: usize = 4;

/// La medición de `calibrar_con` y de `calibrar_tarea_con`. Antes de la línea base y de cada caso, si hay
/// tope, compara el gasto más el peor caso con él. Si no entra, para: la fila queda con los casos hechos y
/// marcada `incompleta`. Sin casos hechos, no hay fila. `muertos_de_la_base` se llama una vez, después de la
/// base, y solo si la dimensión tiene `AssertBorrado`: sus muertos son los de todos los casos. Los casos
/// corren de a `CASOS_EN_PARALELO` hilos.
#[allow(clippy::too_many_arguments)]
fn calibrar_medida(
    tarea: &Tarea,
    dimension: Dimension,
    n: usize,
    semilla: u64,
    repo_bueno: &Path,
    evaluador: &(dyn Fn(&Path, TipoCaso, Target) -> Result<Veredicto, ErrorOraculo> + Sync),
    tope: Option<&Tope>,
    muertos_de_la_base: &dyn Fn() -> Vec<MutanteMuerto>,
    leer_digest: &(dyn Fn() -> Option<String> + Sync),
) -> Result<Medida, ErrorOraculo> {
    calibrar_medida_con_hilos(
        tarea,
        dimension,
        n,
        semilla,
        repo_bueno,
        evaluador,
        tope,
        muertos_de_la_base,
        leer_digest,
        CASOS_EN_PARALELO,
    )
}

/// Lo que comparten los casos de una medición. Solo se lee: los casos corren en hilos distintos.
struct Ronda<'a> {
    semilla: u64,
    dimension: Dimension,
    defectos: &'a [Defecto],
    repo_bueno: &'a Path,
    /// Carpeta temporal de la medición. Cada caso usa `caso-{i}` dentro.
    tmp: &'a Path,
    capas: &'a [Vec<String>],
    camino: &'a [String],
    muertos: &'a [MutanteMuerto],
    base: &'a Veredicto,
    items: &'a [&'static str],
    evaluador: &'a (dyn Fn(&Path, TipoCaso, Target) -> Result<Veredicto, ErrorOraculo> + Sync),
    /// Tope de la medición. Cada reemplazo lo revisa antes de correr.
    tope: Option<&'a Tope<'a>>,
    /// `target` de la base (Tarea 59): `Some` solo si la base dejó una carpeta. Los casos solo la leen.
    target_base: Option<PathBuf>,
    /// Digest de la imagen tras la base. Un caso que lo cambia compila en frío.
    digest_base: Option<String>,
    /// Digest de la imagen antes de cada evaluación de un caso.
    leer_digest: &'a (dyn Fn() -> Option<String> + Sync),
    /// Prefijo del id de cada caso: `calibra-{pid}-{inicio_ms}`. El caso `i` usa `{prefijo}-{i}`.
    prefijo: String,
}

impl Ronda<'_> {
    /// Id del caso `i`. Cada reserva que escribe el caso lo lleva en `extra.caso`.
    fn id_de(&self, i: usize) -> String {
        format!("{}-{i}", self.prefijo)
    }
}

/// Lo que midió un caso: los reemplazos de sus inyecciones inválidas y el caso (`None`: no aplicable).
/// Si el tope se alcanzó antes de un reemplazo, `caso` es `None` y `detencion` dice por qué.
struct Medido {
    reemplazos: u32,
    caso: Option<Caso>,
    detencion: Option<Detencion>,
}

/// Evalúa la copia `copia` del caso `i` como `tipo`. Reusa el `target` de la base si `puede_reusar` lo permite.
/// Devuelve el veredicto y si el caso reusó: un aviso de `target` frío cuenta como compilación en frío.
/// La llamada corre con el id del caso fijado, para que sus reservas lo lleven.
fn evaluar_copia(
    r: &Ronda,
    copia: &Path,
    tipo: TipoCaso,
    i: usize,
) -> Result<(Veredicto, bool), ErrorOraculo> {
    let digest_ahora = (r.leer_digest)();
    let reusa = target::puede_reusar(
        r.repo_bueno,
        copia,
        r.target_base.as_deref(),
        r.digest_base.as_deref(),
        digest_ahora.as_deref(),
    );
    let target = match &r.target_base {
        Some(origen) if reusa => Target::Inicial(origen.clone()),
        _ => Target::Ninguno,
    };
    let id = r.id_de(i);
    let veredicto = con_caso(&id, || (r.evaluador)(copia, tipo, target))?;
    let frio = veredicto
        .avisos
        .iter()
        .any(|aviso| aviso.starts_with(AVISO_TARGET_FRIO));
    Ok((veredicto, reusa && !frio))
}

/// Mide el caso `i`. Los pares llevan un defecto: el de `defectos[(i / 2) % len]`, la misma rotación de los
/// casos en serie. Los impares son limpios. Un defecto sin candidato medible da un caso no aplicable.
fn medir_caso(r: &Ronda, i: usize) -> Result<Medido, ErrorOraculo> {
    if i % 2 == 1 {
        let copia = r.tmp.join(format!("caso-{i}"));
        super::copiar_repo(r.repo_bueno, &copia).map_err(|e| super::infra("copia del repo", e))?;
        let (veredicto, reusado) = evaluar_copia(r, &copia, TipoCaso::Limpio, i)?;
        borrado::borrar_dir_dentro(r.tmp, &copia)
            .map_err(|e| super::infra("borrar una copia", e))?;
        let caso = Caso {
            target_reusado: reusado,
            ..Caso::nuevo(i as u32, None, r.base, &veredicto, r.dimension, r.items)
        };
        return Ok(Medido {
            reemplazos: 0,
            caso: Some(caso),
            detencion: None,
        });
    }
    let defecto = r.defectos[(i / 2) % r.defectos.len()];
    let semilla_caso = r.semilla.wrapping_add(i as u64);
    // La tarea no declara límites de estructura: se usan los de la sonda (`sondas_nativas`).
    let limites = crate::oracle::perfiles::rust::LimitesEstructura::default();
    let contexto = inyectar::Contexto {
        capas: r.capas,
        camino_caliente: r.camino,
        limites: &limites,
        muertos: r.muertos,
    };
    // Un defecto que rompe la construcción se reemplaza por el siguiente candidato, hasta
    // `MAX_REEMPLAZOS` veces. Cada intento inválido suma a `invalidos`, también el del caso agotado.
    let mut invalidos = 0u32;
    let mut medido = None;
    for k in 0..=MAX_REEMPLAZOS {
        // Antes de cada reemplazo (no antes de la primera inyección) se mira el tope. Si no entra, no se gasta.
        if k > 0
            && let Some(detencion) = detencion_si_no_entra(r.tope, 0)?
        {
            return Ok(Medido {
                reemplazos: invalidos,
                caso: None,
                detencion: Some(detencion),
            });
        }
        let semilla_k = semilla_caso.wrapping_add(u64::from(k) * 1_000_003);
        let copia = r.tmp.join(format!("caso-{i}"));
        super::copiar_repo(r.repo_bueno, &copia).map_err(|e| super::infra("copia del repo", e))?;
        let aplicado = inyectar::inyectar_con(defecto, &copia, &contexto, semilla_k)
            .map_err(|e| super::infra("inyectar un defecto", e))?;
        let Some(inyeccion) = aplicado else {
            borrado::borrar_dir_dentro(r.tmp, &copia)
                .map_err(|e| super::infra("borrar una copia", e))?;
            break;
        };
        let (veredicto, reusado) = evaluar_copia(r, &copia, TipoCaso::Defecto(r.dimension), i)?;
        borrado::borrar_dir_dentro(r.tmp, &copia)
            .map_err(|e| super::infra("borrar una copia", e))?;
        if rompe_la_compilacion(r.base, &veredicto, r.dimension) {
            invalidos += 1;
            continue;
        }
        medido = Some((inyeccion, veredicto, reusado));
        break;
    }
    let Some((inyeccion, veredicto, reusado)) = medido else {
        return Ok(Medido {
            reemplazos: invalidos,
            caso: None,
            detencion: None,
        });
    };
    let caso = Caso {
        reemplazos: invalidos,
        target_reusado: reusado,
        ..Caso::nuevo(
            i as u32,
            Some(inyeccion),
            r.base,
            &veredicto,
            r.dimension,
            r.items,
        )
    };
    Ok(Medido {
        reemplazos: invalidos,
        caso: Some(caso),
        detencion: None,
    })
}

/// Como `calibrar_medida`, con `hilos` casos a la vez (1 corre en serie). Un coordinador lanza los casos en
/// orden. Antes de cada lanzamiento compara el gasto comprometido más un peor caso con el tope, sin contar dos veces
/// los casos con reserva escrita: si no entra y
/// hay casos en vuelo, espera a uno; si no entra y no hay ninguno, para. Antes de cada reemplazo de una
/// inyección inválida, un caso revisa el tope y, si no entra, la medición se detiene. Un error de caso deja de lanzar y
/// espera a los que están en vuelo; devuelve el error del índice más bajo. Los casos se ordenan por índice
/// antes de armar la fila, así el resultado no depende del orden de término.
#[allow(clippy::too_many_arguments)]
fn calibrar_medida_con_hilos(
    tarea: &Tarea,
    dimension: Dimension,
    n: usize,
    semilla: u64,
    repo_bueno: &Path,
    evaluador: &(dyn Fn(&Path, TipoCaso, Target) -> Result<Veredicto, ErrorOraculo> + Sync),
    tope: Option<&Tope>,
    muertos_de_la_base: &dyn Fn() -> Vec<MutanteMuerto>,
    leer_digest: &(dyn Fn() -> Option<String> + Sync),
    hilos: usize,
) -> Result<Medida, ErrorOraculo> {
    let defectos = inyectar::de_dimension(dimension);
    if defectos.is_empty() {
        return Err(ErrorOraculo::Uso(format!(
            "la dimensión {} no tiene defectos inyectables",
            dimension.clave()
        )));
    }
    if n == 0 {
        return Err(ErrorOraculo::Uso(
            "oraculo calibrar necesita n de 1 o más".into(),
        ));
    }
    // La línea base cuenta como un caso: el tope se mira antes de ella, con cero casos hechos.
    if let Some(detencion) = detencion_si_no_entra(tope, 0)? {
        return Ok(Medida {
            fila: None,
            detencion: Some(detencion),
            casos: Vec::new(),
        });
    }
    // La carpeta temporal va antes de la base: la base deja ahí su `target` (Tarea 59).
    let tmp = tempfile::tempdir().map_err(|e| super::infra("carpeta temporal", e))?;
    let destino_target = tmp.path().join(NOMBRE_TARGET_BASE);
    // Una línea base sin veredicto no mide nada: los defectos no tienen contra qué compararse.
    let base = evaluador(
        repo_bueno,
        TipoCaso::Base,
        Target::Guardar(destino_target.clone()),
    )?;
    if puntaje_de(&base, dimension).is_none_or(|p| p.sin_veredicto.is_some()) {
        return Err(ErrorOraculo::Infra(format!(
            "la línea base no tiene veredicto de {}",
            dimension.clave()
        )));
    }
    // Sin carpeta, la base no dejó `target`: ningún caso reusa nada.
    let target_base = destino_target.is_dir().then_some(destino_target);
    let digest_base = leer_digest();
    let muertos = if defectos.contains(&Defecto::AssertBorrado) {
        muertos_de_la_base()
    } else {
        Vec::new()
    };
    let items: BTreeSet<&'static str> = defectos.iter().map(|d| d.item()).collect();
    let items: Vec<&'static str> = items.into_iter().collect();
    let capas: &[Vec<String>] = tarea
        .arquitectura
        .as_ref()
        .map_or(&[], |a| a.capas.as_slice());
    let camino: &[String] = tarea
        .calibracion
        .as_ref()
        .map_or(&[], |c| c.camino_caliente.as_slice());
    let ronda = Ronda {
        semilla,
        dimension,
        defectos: &defectos,
        repo_bueno,
        tmp: tmp.path(),
        capas,
        camino,
        muertos: &muertos,
        base: &base,
        items: &items,
        evaluador,
        tope,
        target_base,
        digest_base,
        leer_digest,
        prefijo: format!(
            "calibra-{}-{}",
            std::process::id(),
            (ahora() * 1000.0) as u64
        ),
    };
    let hilos = hilos.max(1);
    let (tx, rx) = mpsc::channel::<(usize, Result<Medido, ErrorOraculo>)>();
    // El coordinador suelta su envío cuando ya no lanza: si un hilo muere sin enviar, `recv` falla en vez de
    // esperar para siempre.
    let mut tx_abierto = Some(tx);
    let mut resultados: BTreeMap<usize, Medido> = BTreeMap::new();
    let mut error: Option<(usize, ErrorOraculo)> = None;
    let mut detencion = None;
    let mut siguiente = 0usize;
    let mut en_vuelo = 0usize;
    // Índices de los casos en vuelo. Con sus ids, cuentan cuáles ya tienen reserva escrita.
    let mut vuelo: BTreeSet<usize> = BTreeSet::new();
    std::thread::scope(|s| -> Result<(), ErrorOraculo> {
        let ronda = &ronda;
        loop {
            while error.is_none() && siguiente < n && en_vuelo < hilos && detencion.is_none() {
                if let Some(t) = tope {
                    // Primero los ids con reserva, luego el gasto. Una reserva nueva entre las dos lecturas
                    // cuenta dos veces: sobra, nunca falta.
                    let con_reserva = (t.casos_con_reserva)(t.inicio)?;
                    let gasto = (t.gasto_desde)(t.inicio)?;
                    let vuelo_con_reserva = vuelo
                        .iter()
                        .filter(|j| con_reserva.contains(&ronda.id_de(**j)))
                        .count();
                    let comprometido =
                        gasto_comprometido(gasto, t.peor_caso, en_vuelo, vuelo_con_reserva);
                    if comprometido + t.peor_caso > t.usd {
                        if en_vuelo == 0 {
                            // El conteo de casos medidos lo pone el final, cuando todos terminaron.
                            detencion = Some(Detencion {
                                tope_usd: t.usd,
                                gasto_usd: gasto,
                                peor_caso_usd: t.peor_caso,
                                casos: 0,
                            });
                        }
                        break;
                    }
                }
                let i = siguiente;
                siguiente += 1;
                en_vuelo += 1;
                vuelo.insert(i);
                let envio = tx_abierto
                    .clone()
                    .expect("hay canal mientras se lanzan casos");
                s.spawn(move || {
                    let _ = envio.send((i, medir_caso(ronda, i)));
                });
            }
            if siguiente == n || error.is_some() || detencion.is_some() {
                tx_abierto = None;
            }
            if en_vuelo == 0 {
                break;
            }
            let (i, resultado) = rx.recv().map_err(|_| {
                ErrorOraculo::Infra("un caso de calibración terminó sin resultado".into())
            })?;
            en_vuelo -= 1;
            vuelo.remove(&i);
            match resultado {
                Ok(medido) => {
                    // Un reemplazo que no entró en el tope detiene los lanzamientos. Los casos en vuelo terminan.
                    if detencion.is_none() {
                        detencion = medido.detencion.clone();
                    }
                    resultados.insert(i, medido);
                }
                Err(e) => {
                    if error.as_ref().is_none_or(|(j, _)| i < *j) {
                        error = Some((i, e));
                    }
                }
            }
        }
        Ok(())
    })?;
    if let Some((_, e)) = error {
        return Err(e);
    }
    let mut reemplazos = 0u32;
    let mut no_aplicables = 0u32;
    let mut casos = Vec::with_capacity(resultados.len());
    for medido in resultados.into_values() {
        reemplazos += medido.reemplazos;
        match medido.caso {
            Some(caso) => casos.push(caso),
            // Un caso que el tope detuvo no es no aplicable: no se midió.
            None if medido.detencion.is_some() => {}
            None => no_aplicables += 1,
        }
    }
    if let Some(d) = detencion.as_mut() {
        d.casos = casos.len() as u32;
    }
    let incompleta = detencion.is_some();
    let fila = if incompleta && casos.is_empty() {
        None
    } else {
        Some(Fila {
            reemplazos,
            ..fila_de(
                tarea,
                dimension,
                semilla,
                &casos,
                &items,
                no_aplicables,
                incompleta,
            )
        })
    };
    Ok(Medida {
        fila,
        detencion,
        casos,
    })
}

/// La fila de los casos medidos: sensibilidad, especificidad y exactitud por dimensión y por ítem.
fn fila_de(
    tarea: &Tarea,
    dimension: Dimension,
    semilla: u64,
    casos: &[Caso],
    items: &[&'static str],
    no_aplicables: u32,
    incompleta: bool,
) -> Fila {
    let defectos_medidos = casos.iter().filter(|c| c.es_defecto).count() as u32;
    let limpios = casos.len() as u32 - defectos_medidos;
    let sensibilidad = proporcion(
        casos.iter().filter(|c| c.es_defecto && c.detectado).count() as u32,
        defectos_medidos,
    );
    let especificidad = proporcion(
        casos
            .iter()
            .filter(|c| !c.es_defecto && !c.detectado)
            .count() as u32,
        limpios,
    );
    let mut items_medidos = BTreeMap::new();
    for item in items {
        let positivos: Vec<bool> = casos
            .iter()
            .filter(|c| c.es_defecto && c.item_defecto == Some(*item))
            .filter_map(|c| c.detecta_en(item))
            .collect();
        let negativos: Vec<bool> = casos
            .iter()
            .filter(|c| !c.es_defecto)
            .filter_map(|c| c.detecta_en(item))
            .collect();
        let sens_item = proporcion(
            positivos.iter().filter(|d| **d).count() as u32,
            positivos.len() as u32,
        );
        let espec_item = proporcion(
            negativos.iter().filter(|d| !**d).count() as u32,
            negativos.len() as u32,
        );
        if let Some(exactitud) = exactitud_de(sens_item, espec_item) {
            items_medidos.insert(item.to_string(), exactitud);
        }
    }
    Fila {
        tarea: tarea.nombre.clone(),
        dimension,
        n: casos.len() as u32,
        defectos: defectos_medidos,
        limpios,
        no_aplicable: no_aplicables,
        semilla,
        ts: ahora(),
        sensibilidad,
        especificidad,
        exactitud_balanceada: exactitud_de(sensibilidad, especificidad),
        items: items_medidos,
        incompleta,
        no_aplica: None,
        reemplazos: 0,
    }
}

/// `oraculo calibrar`: resuelve la tarea y el repo bueno en `home`, calibra y agrega la fila a
/// `calibracion.json`. El repo sale de `repo_cli`, o de `[calibracion] repo` de `tarea.toml`. Sin tope.
pub fn calibrar_tarea(
    home: &Path,
    nombre: &str,
    dimension: &str,
    n: u32,
    semilla: u64,
    repo_cli: Option<&Path>,
    evaluador: &(dyn Fn(&Tarea, &Path, &OpcionesOraculo) -> Result<Veredicto, ErrorOraculo> + Sync),
) -> Result<Fila, ErrorOraculo> {
    calibrar_tarea_con(
        home, nombre, dimension, n, semilla, repo_cli, None, evaluador,
    )
    .map(|calibrada| calibrada.fila)
}

/// Tope de gasto de una calibración (decisión 48). Antes de la línea base y de cada caso, se detiene si el
/// gasto de API desde el inicio más el peor caso de un caso pasa `usd`.
pub struct Presupuesto<'a> {
    /// Tope en USD. Debe ser finito y mayor que 0.
    pub usd: f64,
    /// Peor caso en USD de un caso con estas partes. Un modelo sin precio da un error.
    pub peor_caso: &'a dyn Fn(&Partes) -> Result<f64, ErrorOraculo>,
    /// Gasto de API desde un instante (epoch en segundos): llamadas y reservas abiertas. Es `Sync`: los
    /// reemplazos lo miran desde los hilos de los casos.
    pub gasto_desde: &'a (dyn Fn(f64) -> Result<f64, ErrorOraculo> + Sync),
}

/// Elecciones de los jueces que un caso puede llamar. `None`: el juez no se arma y no llama.
#[derive(Debug, Clone, Copy, Default)]
pub struct JuecesCaso<'a> {
    /// Juez de diseño (Tarea 10): llama si las partes lo piden.
    pub diseno: Option<&'a Eleccion>,
    /// Juez de la configuración de Cargo (decisión 30): llama si las partes lo piden.
    pub config: Option<&'a Eleccion>,
}

/// Entrada de cada llamada de juez, en bytes UTF-8. Desde el desempate 24 es un límite real: un prompt más
/// grande no se envía. Se conserva el nombre: es el mismo valor que `juez::MAX_ENTRADA_JUEZ_BYTES`.
pub const ENTRADA_JUEZ_BYTES: u64 = MAX_ENTRADA_JUEZ_BYTES;

/// Gasto de la calibración con tope: el `Gate` que registra su detención.
const CORRIDA_CALIBRACION: &str = "oraculo-calibracion";
/// Fase del `Gate` de tope de una calibración.
const FASE_TOPE_CALIBRACION: &str = "calibracion-tope";

/// Llamadas de juez que un caso puede hacer con sus partes: la elección y el tope de salida de cada una.
fn llamadas_del_caso<'a>(jueces: &JuecesCaso<'a>, partes: &Partes) -> Vec<(&'a Eleccion, u32)> {
    let mut llamadas = Vec::new();
    if partes.juez_diseno
        && let Some(eleccion) = jueces.diseno
    {
        llamadas.push((eleccion, LimitesPrompt::default().max_tokens));
    }
    if partes.juez_config
        && let Some(eleccion) = jueces.config
    {
        llamadas.push((eleccion, MAX_TOKENS_JUEZ));
    }
    llamadas
}

/// Precio del modelo de una elección, si `cfg` lo tiene.
fn precio_de_eleccion<'a>(cfg: &'a Config, eleccion: &Eleccion) -> Option<&'a Precio> {
    cfg.precios.get(eleccion.modelo.as_deref()?)
}

/// Peor caso en USD de un caso con `partes`: cada llamada de juez que las partes piden, con la entrada
/// estimada y todo `max_tokens`, al precio de su modelo. Como `llamar_con`, cuenta cada intento posible. Sin
/// precio para el modelo, el peor caso es infinito. Un caso sin llamadas de API vale 0.
pub fn peor_caso_de_caso(cfg: &Config, jueces: &JuecesCaso, partes: &Partes) -> f64 {
    llamadas_del_caso(jueces, partes)
        .into_iter()
        .fold(0.0, |total, (eleccion, max_tokens)| {
            let costo = match precio_de_eleccion(cfg, eleccion) {
                Some(precio) => {
                    f64::from(INTENTOS_MAX)
                        * costo_peor_caso(precio, MAX_ENTRADA_JUEZ_BYTES, max_tokens)
                }
                None => f64::INFINITY,
            };
            total + costo
        })
}

/// Modelos de las llamadas que el caso puede hacer y no tienen precio en `cfg`.
pub fn modelos_sin_precio(cfg: &Config, jueces: &JuecesCaso, partes: &Partes) -> Vec<String> {
    llamadas_del_caso(jueces, partes)
        .into_iter()
        .filter(|(eleccion, _)| precio_de_eleccion(cfg, eleccion).is_none())
        .map(|(eleccion, _)| {
            eleccion
                .modelo
                .clone()
                .unwrap_or_else(|| "(sin modelo)".into())
        })
        .collect()
}

/// Como `peor_caso_de_caso`, pero un modelo sin precio es un error de configuración que nombra el modelo:
/// la calibración con tope no arranca.
pub fn peor_caso_con_jueces(
    cfg: &Config,
    jueces: &JuecesCaso,
    partes: &Partes,
) -> Result<f64, ErrorOraculo> {
    let sin_precio = modelos_sin_precio(cfg, jueces, partes);
    if !sin_precio.is_empty() {
        return Err(ErrorOraculo::Eleccion(format!(
            "sin precio en root.toml para {}: el peor caso del juez es infinito y la calibración con --tope-usd no arranca. Agregá [precios.<modelo>]",
            sin_precio.join(", ")
        )));
    }
    Ok(peor_caso_de_caso(cfg, jueces, partes))
}

/// Peor caso en USD de un caso con `partes` en `home`: los jueces que `evaluar` armaría con el `root.toml` y
/// las rutas de `home`. Sin partes de juez, vale 0 sin leer nada (decisión 48).
pub fn peor_caso_de_home(home: &Path, partes: &Partes) -> Result<f64, ErrorOraculo> {
    if !partes.juez_diseno && !partes.juez_config {
        return Ok(0.0);
    }
    let cfg = Config::cargar(&home.join("root.toml"))
        .map_err(|e| ErrorOraculo::Configuracion(e.codigo_de_salida(), e.to_string()))?;
    let mut avisos = Vec::new();
    let diseno = partes
        .juez_diseno
        .then(|| super::juez_diseno_de_produccion(home, &mut avisos))
        .flatten();
    let config = partes
        .juez_config
        .then(|| super::juez_config_de_produccion(home, &cfg, &mut avisos))
        .flatten();
    let jueces = JuecesCaso {
        diseno: diseno.as_ref().map(|juez| &juez.eleccion),
        config: config.as_ref().map(|juez| &juez.eleccion),
    };
    peor_caso_con_jueces(&cfg, &jueces, partes)
}

/// `oraculo calibrar` con tope opcional (decisión 48): como `calibrar_tarea`, con las dependencias
/// inyectables. Con `presupuesto`, antes de la línea base y de cada caso compara el gasto más el peor caso
/// con el tope. Si no entra, registra un `Gate` `calibracion-tope`, agrega la fila de los casos hechos
/// marcada `incompleta` (si hubo alguno) y sale con código 3. `evaluador` es `Sync` (Tarea 48, parte 2): los
/// casos corren de a `CASOS_EN_PARALELO` hilos.
#[allow(clippy::too_many_arguments)]
pub fn calibrar_tarea_con(
    home: &Path,
    nombre: &str,
    dimension: &str,
    n: u32,
    semilla: u64,
    repo_cli: Option<&Path>,
    presupuesto: Option<&Presupuesto>,
    evaluador: &(dyn Fn(&Tarea, &Path, &OpcionesOraculo) -> Result<Veredicto, ErrorOraculo> + Sync),
) -> Result<Calibrada, ErrorOraculo> {
    let dim = Dimension::desde_clave(dimension)
        .ok_or_else(|| ErrorOraculo::Uso(format!("dimensión desconocida: {dimension}")))?;
    let tarea = super::tarea::cargar_en(home, nombre)?;
    // Eficiencia sin `[eficiencia]`: la tarea no la aplica. Ni la base ni los casos corren, y la fila lleva
    // solo el motivo. Es el mismo criterio que la sonda usa para no medir.
    if dim == Dimension::Eficiencia && tarea.eficiencia.is_none() {
        let fila = fila_no_aplica(&tarea, dim, semilla);
        agregar(home, &fila)?;
        return Ok(Calibrada { fila, casos: None });
    }
    let repo = repo_de(&tarea, repo_cli)?;
    // La base y cada caso corren las mismas partes: solo la dimensión calibrada se mide.
    let rubricas = super::rubrica::cargar_embebidas()
        .map_err(|e| ErrorOraculo::Infra(format!("rúbricas embebidas: {e}")))?;
    let partes = partes_de(dim, &rubricas, &tarea);
    // Una calibración no escribe historial: cada caso es una versión distinta y se evalúa una vez.
    let opciones = OpcionesOraculo {
        partes: Some(partes),
        historial: false,
        ..OpcionesOraculo::default()
    };
    // La base graba las salidas de sus sondas en `memo`; los casos no graban. Un caso con defecto reusa de
    // `memo` lo que `reuso_de` dice (la mutación, en las dimensiones que no la cambian).
    let memo = std::sync::Arc::new(super::memo::MemoSondas::nuevo());
    let evaluar_caso = |ruta: &Path, tipo: TipoCaso, target: Target| {
        let opciones_de_caso = OpcionesOraculo {
            memo: (tipo == TipoCaso::Base).then(|| memo.clone()),
            reuso: reuso_de(tipo, &memo),
            target,
            ..opciones.clone()
        };
        evaluador(&tarea, ruta, &opciones_de_caso)
    };
    // El tope se resuelve antes de la base: un peor caso sin precio no deja arrancar.
    let inicio = ahora();
    let casos_con_reserva = |desde: f64| {
        observe::uso::casos_con_reserva_abierta_en(&observe::ruta_eventos_en(home), desde)
            .map_err(|e| ErrorOraculo::Infra(format!("leer las reservas de la calibración: {e}")))
    };
    let tope = match presupuesto {
        Some(p) => {
            if !p.usd.is_finite() || p.usd <= 0.0 {
                return Err(ErrorOraculo::Uso(format!(
                    "el tope debe ser un número finito mayor que 0, no {}",
                    p.usd
                )));
            }
            let peor_caso = (p.peor_caso)(&partes)?;
            if !peor_caso.is_finite() {
                return Err(ErrorOraculo::Eleccion(
                    "el peor caso de un caso es infinito: la calibración con tope no arranca"
                        .into(),
                ));
            }
            Some(Tope {
                usd: p.usd,
                peor_caso,
                inicio,
                gasto_desde: p.gasto_desde,
                casos_con_reserva: &casos_con_reserva,
            })
        }
        None => None,
    };
    let medida = calibrar_medida(
        &tarea,
        dim,
        n as usize,
        semilla,
        &repo,
        &evaluar_caso,
        tope.as_ref(),
        &|| memo.mutantes_muertos(&tarea),
        &|| target::digest_de_imagen(&tarea.imagen),
    )?;
    // Los casos se escriben antes de `agregar`, también si el tope detuvo la calibración.
    let casos = match &medida.fila {
        Some(fila) => escribir_casos(home, &tarea.nombre, dim, fila, &medida.casos)?,
        None => None,
    };
    if let Some(detencion) = medida.detencion {
        let gate = gate_de_tope(&tarea.nombre, dim, &detencion);
        observe::registrar_en(&observe::ruta_eventos_en(home), &gate)
            .map_err(|e| ErrorOraculo::Infra(format!("registrar el Gate de calibración: {e}")))?;
        if let Some(fila) = &medida.fila {
            agregar(home, fila)?;
        }
        let archivo =
            casos.map_or_else(|| "ninguno".to_string(), |ruta| ruta.display().to_string());
        return Err(ErrorOraculo::Tope(format!(
            "calibración de {}/{} detenida por el tope de {:.2} USD: gasto de API {:.2} USD y el peor caso de un caso es {:.2} USD. Casos medidos: {}. Archivo de casos: {}.",
            tarea.nombre,
            dim.clave(),
            detencion.tope_usd,
            detencion.gasto_usd,
            detencion.peor_caso_usd,
            detencion.casos,
            archivo
        )));
    }
    let fila = medida
        .fila
        .ok_or_else(|| ErrorOraculo::Infra("la calibración no dejó ninguna fila".into()))?;
    agregar(home, &fila)?;
    Ok(Calibrada { fila, casos })
}

/// Motivo de la fila de Eficiencia en una tarea sin `[eficiencia]`.
const MOTIVO_NO_APLICA_EFICIENCIA: &str = "no aplica: la tarea no define [eficiencia]";

/// Fila de una dimensión que la tarea no aplica: sin casos ni métricas, con el motivo en `no_aplica`.
fn fila_no_aplica(tarea: &Tarea, dimension: Dimension, semilla: u64) -> Fila {
    Fila {
        tarea: tarea.nombre.clone(),
        dimension,
        n: 0,
        defectos: 0,
        limpios: 0,
        no_aplicable: 0,
        semilla,
        ts: ahora(),
        sensibilidad: None,
        especificidad: None,
        exactitud_balanceada: None,
        items: BTreeMap::new(),
        incompleta: false,
        no_aplica: Some(MOTIVO_NO_APLICA_EFICIENCIA.into()),
        reemplazos: 0,
    }
}

/// `Gate` de una calibración detenida por su tope: tarea, dimensión, tope, gasto, peor caso y casos hechos.
fn gate_de_tope(tarea: &str, dimension: Dimension, detencion: &Detencion) -> Evento {
    let extra = Map::from_iter([
        ("tarea".to_string(), json!(tarea)),
        ("dimension".to_string(), json!(dimension.clave())),
        ("tope_usd".to_string(), json!(detencion.tope_usd)),
        ("gasto_usd".to_string(), json!(detencion.gasto_usd)),
        ("peor_caso_usd".to_string(), json!(detencion.peor_caso_usd)),
        ("casos_hechos".to_string(), json!(detencion.casos)),
    ]);
    observe::evento_gate(CORRIDA_CALIBRACION, FASE_TOPE_CALIBRACION, extra)
}

/// El repo bueno: `--repo`, o `[calibracion] repo` de la tarea (relativo a su carpeta). Debe existir.
fn repo_de(tarea: &Tarea, repo_cli: Option<&Path>) -> Result<PathBuf, ErrorOraculo> {
    let repo = match repo_cli {
        Some(ruta) => ruta.to_path_buf(),
        None => match tarea.calibracion.as_ref().and_then(|c| c.repo.as_deref()) {
            // `join` con una ruta absoluta devuelve esa ruta.
            Some(ruta) => tarea.dir.join(ruta),
            None => {
                return Err(ErrorOraculo::Uso(
                    "oraculo calibrar necesita --repo o [calibracion] repo en tarea.toml".into(),
                ));
            }
        },
    };
    if !repo.is_dir() {
        return Err(ErrorOraculo::Uso(format!(
            "el repo bueno no existe: {}",
            repo.display()
        )));
    }
    Ok(repo)
}

/// Agrega `fila` a `home/calibracion.json` bajo cerrojo. No reescribe las filas anteriores: relee el
/// archivo, agrega la fila y lo reemplaza con una escritura atómica.
pub fn agregar(home: &Path, fila: &Fila) -> Result<(), ErrorOraculo> {
    fs::create_dir_all(home).map_err(|e| super::infra("crear ROOT_HOME", e))?;
    let candado = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(home.join(CANDADO))
        .map_err(|e| super::infra("abrir el cerrojo de calibración", e))?;
    fs4::FileExt::lock(&candado).map_err(|e| super::infra("tomar el cerrojo de calibración", e))?;
    let mut calibracion = Calibraciones::cargar(home)?;
    calibracion.filas.push(fila.clone());
    let texto = serde_json::to_string_pretty(&calibracion)
        .map_err(|e| ErrorOraculo::Infra(format!("serializar calibracion.json: {e}")))?;
    let temporal = home.join(format!("{ARCHIVO}.tmp"));
    fs::write(&temporal, format!("{texto}\n"))
        .map_err(|e| super::infra("escribir calibracion.json", e))?;
    fs::rename(&temporal, home.join(ARCHIVO))
        .map_err(|e| super::infra("reemplazar calibracion.json", e))?;
    Ok(())
}

/// Reserva el primer nombre libre `<base>.casos.jsonl`, `<base>-2.casos.jsonl`, ... en `dir`. Crea el archivo
/// vacío con `create_new`: dos calibraciones no pueden elegir el mismo nombre.
fn reservar_destino(dir: &Path, base: &str) -> io::Result<(PathBuf, fs::File)> {
    let mut n = 1u32;
    loop {
        let nombre = if n == 1 {
            format!("{base}.casos.jsonl")
        } else {
            format!("{base}-{n}.casos.jsonl")
        };
        let ruta = dir.join(nombre);
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&ruta)
        {
            Ok(archivo) => return Ok((ruta, archivo)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => n += 1,
            Err(e) => return Err(e),
        }
    }
}

/// Un caso medido: si era un defecto (y cuál ítem apuntaba), si la dimensión lo detectó y, por ítem,
/// si cada ítem bajó (`None` si la base no mide ese ítem).
struct Caso {
    /// Índice `i` del bucle de la calibración.
    indice: u32,
    /// El defecto aplicado, completo (archivo y línea). `None` en un caso limpio.
    inyeccion: Option<Inyeccion>,
    es_defecto: bool,
    item_defecto: Option<&'static str>,
    detectado: bool,
    /// Motivo por el que la dimensión calibrada quedó sin veredicto en este caso.
    sin_veredicto: Option<String>,
    items: BTreeMap<&'static str, Option<bool>>,
    /// Inyecciones inválidas reemplazadas antes de este caso.
    reemplazos: u32,
    /// Si el caso compiló sobre el `target` de la base.
    target_reusado: bool,
}

impl Caso {
    fn nuevo(
        indice: u32,
        inyeccion: Option<Inyeccion>,
        base: &Veredicto,
        veredicto: &Veredicto,
        dimension: Dimension,
        items: &[&'static str],
    ) -> Caso {
        let es_defecto = inyeccion.is_some();
        Caso {
            indice,
            es_defecto,
            item_defecto: inyeccion.as_ref().map(|i| i.defecto.item()),
            detectado: detecta_dimension(base, veredicto, dimension, es_defecto),
            sin_veredicto: motivo_sin_veredicto(veredicto, dimension),
            items: items
                .iter()
                .map(|item| {
                    (
                        *item,
                        detecta_item(base, veredicto, dimension, item, es_defecto),
                    )
                })
                .collect(),
            inyeccion,
            reemplazos: 0,
            target_reusado: false,
        }
    }

    fn detecta_en(&self, item: &str) -> Option<bool> {
        self.items.get(item).copied().flatten()
    }
}

impl CasoGuardado {
    fn de(caso: &Caso) -> CasoGuardado {
        CasoGuardado {
            caso: caso.indice,
            defecto: caso.inyeccion.as_ref().map(|i| nombre_defecto(i.defecto)),
            item: caso.item_defecto.map(str::to_string),
            archivo: caso.inyeccion.as_ref().map(|i| i.archivo.clone()),
            linea: caso.inyeccion.as_ref().map(|i| i.linea),
            es_defecto: caso.es_defecto,
            detectado: caso.detectado,
            sin_veredicto: caso.sin_veredicto.clone(),
            reemplazos: caso.reemplazos,
            // Ningún mecanismo repite casos hoy: el campo queda en `false` hasta que alguno lo haga.
            repetido_por_entorno: false,
            target_reusado: Some(caso.target_reusado),
        }
    }
}

/// Nombre en snake_case de un defecto, a partir de su `{:?}`: `SecretoEnCodigo` da `secreto_en_codigo`.
fn nombre_defecto(defecto: Defecto) -> String {
    let mut nombre = String::new();
    for (i, c) in format!("{defecto:?}").chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                nombre.push('_');
            }
            nombre.extend(c.to_lowercase());
        } else {
            nombre.push(c);
        }
    }
    nombre
}

/// Motivo por el que `dimension` quedó sin veredicto en `veredicto`. Sin puntaje de la dimensión, un motivo fijo.
fn motivo_sin_veredicto(veredicto: &Veredicto, dimension: Dimension) -> Option<String> {
    match puntaje_de(veredicto, dimension) {
        Some(puntaje) => puntaje.sin_veredicto.clone(),
        None => Some("sin puntaje de la dimensión".into()),
    }
}

/// Reemplazos máximos por caso con defecto. El primer candidato no cuenta: un caso prueba hasta
/// `MAX_REEMPLAZOS + 1` inyecciones. Si todas rompen la construcción, el caso es no aplicable.
pub const MAX_REEMPLAZOS: u32 = 5;

/// Marcadores de construcción fallida. Un veredicto con uno de ellos no mide su dimensión: el
/// texto es el de `perfiles/herramientas.rs` (sonda de clippy) y `mutacion.rs` (sonda de mutación).
const MOTIVOS_DE_CONSTRUCCION: [&str; 2] = ["no compila", "fallan sin mutaciones"];

/// Si el defecto rompe la construcción: `dimension` queda sin veredicto, o con ítems faltantes, por un
/// motivo de construcción fallida que la base no tiene. Completitud y Funciona nunca cuentan.
fn rompe_la_compilacion(base: &Veredicto, veredicto: &Veredicto, dimension: Dimension) -> bool {
    if matches!(dimension, Dimension::Completitud | Dimension::Funciona) {
        return false;
    }
    let en_base = motivos_de_construccion(base, dimension);
    motivos_de_construccion(veredicto, dimension)
        .iter()
        .any(|motivo| !en_base.contains(motivo))
}

/// Los marcadores de construcción fallida de `dimension` en `veredicto`: el de `sin_veredicto` y los de
/// `faltantes`. Se comparan marcadores, no textos: el error de rustc cambia de copia a copia.
fn motivos_de_construccion(veredicto: &Veredicto, dimension: Dimension) -> BTreeSet<&'static str> {
    let Some(puntaje) = puntaje_de(veredicto, dimension) else {
        return BTreeSet::new();
    };
    let textos = puntaje
        .sin_veredicto
        .iter()
        .map(String::as_str)
        .chain(puntaje.faltantes.iter().map(|f| f.motivo.as_str()));
    textos
        .flat_map(|texto| {
            MOTIVOS_DE_CONSTRUCCION
                .into_iter()
                .filter(move |marcador| texto.contains(*marcador))
        })
        .collect()
}

/// Escribe los casos medidos en `home/calibracion/<tarea>-<dimension>-<ts>.casos.jsonl`: un renglón JSON por
/// caso, en orden. Escritura atómica: un temporal junto al destino y un `rename`. Sin casos, no escribe y da `None`.
fn escribir_casos(
    home: &Path,
    tarea: &str,
    dimension: Dimension,
    fila: &Fila,
    casos: &[Caso],
) -> Result<Option<PathBuf>, ErrorOraculo> {
    if casos.is_empty() {
        return Ok(None);
    }
    let carpeta = home.join(CARPETA_CASOS);
    fs::create_dir_all(&carpeta).map_err(|e| super::infra("crear la carpeta de casos", e))?;
    let base = format!("{tarea}-{}-{}", dimension.clave(), fila.ts as u64);
    let (destino, _) = reservar_destino(&carpeta, &base)
        .map_err(|e| super::infra("reservar el archivo de casos", e))?;
    let mut texto = String::new();
    for caso in casos {
        let renglon = serde_json::to_string(&CasoGuardado::de(caso))
            .map_err(|e| ErrorOraculo::Infra(format!("serializar un caso: {e}")))?;
        texto.push_str(&renglon);
        texto.push('\n');
    }
    let temporal = PathBuf::from(format!("{}.tmp", destino.display()));
    if let Err(e) = fs::write(&temporal, texto).and_then(|()| fs::rename(&temporal, &destino)) {
        // Limpieza dentro de la carpeta de casos: el temporal y la reserva vacía.
        let _ = borrado::borrar_archivo_dentro(&carpeta, &temporal);
        let _ = borrado::borrar_archivo_dentro(&carpeta, &destino);
        return Err(super::infra("escribir los casos", e));
    }
    Ok(Some(destino))
}

/// La dimensión empeora frente a la base si aparece un hallazgo con id nuevo, baja su `valor` o baja el
/// nivel de algún ítem. Sin veredicto de la dimensión, el defecto es un falso negativo y el limpio un
/// falso positivo: lo más conservador.
fn detecta_dimension(
    base: &Veredicto,
    veredicto: &Veredicto,
    dimension: Dimension,
    es_defecto: bool,
) -> bool {
    let Some(pv) = puntaje_de(veredicto, dimension) else {
        return !es_defecto;
    };
    if pv.sin_veredicto.is_some() {
        return !es_defecto;
    }
    let Some(pb) = puntaje_de(base, dimension) else {
        return false;
    };
    let ids_base: BTreeSet<&str> = base.hallazgos.iter().map(|h| h.id.as_str()).collect();
    let hallazgo_nuevo = veredicto
        .hallazgos
        .iter()
        .any(|h| h.dimension == dimension && !ids_base.contains(h.id.as_str()));
    let item_bajo = pb
        .items
        .iter()
        .any(|ib| nivel_de(pv, &ib.item).is_some_and(|nivel| nivel < ib.nivel));
    (hallazgo_nuevo || pv.valor < pb.valor || item_bajo)
        && estables::permite(base, veredicto, dimension)
}

/// Si el ítem `item` de la dimensión baja frente a la base. `None` si la base no mide el ítem: no entra a
/// la métrica. Sin veredicto de la dimensión, o sin nivel del ítem, cuenta como en `detecta_dimension`.
fn detecta_item(
    base: &Veredicto,
    veredicto: &Veredicto,
    dimension: Dimension,
    item: &str,
    es_defecto: bool,
) -> Option<bool> {
    let nivel_base = nivel_de(puntaje_de(base, dimension)?, item)?;
    let Some(pv) = puntaje_de(veredicto, dimension) else {
        return Some(!es_defecto);
    };
    if pv.sin_veredicto.is_some() {
        return Some(!es_defecto);
    }
    Some(match nivel_de(pv, item) {
        Some(nivel) => nivel < nivel_base && estables::permite(base, veredicto, dimension),
        None => !es_defecto,
    })
}

fn puntaje_de(veredicto: &Veredicto, dimension: Dimension) -> Option<&Puntaje> {
    veredicto.puntajes.iter().find(|p| p.dimension == dimension)
}

fn nivel_de(puntaje: &Puntaje, item: &str) -> Option<u8> {
    puntaje
        .items
        .iter()
        .find(|i| i.item == item)
        .map(|i| i.nivel)
}

/// `aciertos / total`. `None` sin casos: una métrica sin clase no es 0 ni 1.
fn proporcion(aciertos: u32, total: u32) -> Option<f32> {
    (total > 0).then(|| aciertos as f32 / total as f32)
}

/// Una fila calibra solo con muestra mínima: defectos y limpios por encima de sus mínimos.
fn muestra_suficiente(fila: &Fila) -> bool {
    fila.defectos >= MIN_DEFECTOS_APLICABLES && fila.limpios >= MIN_LIMPIOS
}

/// Media de sensibilidad y especificidad. `None` si cualquiera de las dos no tiene casos.
fn exactitud_de(sensibilidad: Option<f32>, especificidad: Option<f32>) -> Option<f32> {
    let (s, e) = (sensibilidad?, especificidad?);
    Some((s + e) / 2.0)
}

/// Segundos desde la época, para `ts`.
fn ahora() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}

#[cfg(test)]
mod defectos;

#[cfg(test)]
mod tests {
    use super::inyectar::pruebas::{FIXTURE, arbol, copia_fixture, tarea_en};
    use super::*;
    use crate::oracle::mutacion::{ARCHIVO_OUTCOMES, construir_comando};
    use crate::oracle::perfiles::herramientas::{
        COMANDO_CARGO_AUDIT, COMANDO_GITLEAKS, EjecutorHerramienta, SalidaHerramienta,
        comando_clippy,
    };
    use crate::tipos::{Fuente, Hallazgo, ItemFaltante, NivelItem};
    use std::collections::HashSet;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};
    use tempfile::TempDir;

    fn fila(tarea: &str, dimension: Dimension, ts: f64, exactitud: Option<f32>) -> Fila {
        Fila {
            tarea: tarea.into(),
            dimension,
            n: 20,
            defectos: 10,
            limpios: 10,
            no_aplicable: 0,
            semilla: 42,
            ts,
            sensibilidad: Some(1.0),
            especificidad: Some(1.0),
            exactitud_balanceada: exactitud,
            items: BTreeMap::new(),
            incompleta: false,
            no_aplica: None,
            reemplazos: 0,
        }
    }

    /// Veredicto de Seguridad sano: valor 1, dos ítems en nivel 3 y sin hallazgos.
    fn veredicto_base() -> Veredicto {
        Veredicto {
            puntajes: vec![Puntaje {
                dimension: Dimension::Seguridad,
                valor: 1.0,
                calibrada: false,
                exactitud_balanceada: None,
                items: vec![
                    nivel_item("seguridad.secretos", 3),
                    nivel_item("seguridad.panico_en_entrada", 3),
                ],
                sin_veredicto: None,
                faltantes: Vec::new(),
            }],
            ..Veredicto::default()
        }
    }

    pub(super) fn nivel_item(item: &str, nivel: u8) -> NivelItem {
        NivelItem {
            item: item.into(),
            nivel,
            evidencia: vec![],
            fuente: Fuente::Sonda("test".into()),
        }
    }

    fn hallazgo(id: &str) -> Hallazgo {
        Hallazgo {
            dimension: Dimension::Seguridad,
            severidad: 2,
            archivo: "src/lib.rs".into(),
            linea: 1,
            cita: "x".into(),
            detalle: String::new(),
            fuente: Fuente::Sonda("test".into()),
            id: id.into(),
            impacto: vec![],
        }
    }

    /// Veredicto en que la seguridad cae: valor 0, ítems en 0 y un hallazgo con id nuevo.
    fn hundido(base: &Veredicto) -> Veredicto {
        let mut v = base.clone();
        v.puntajes[0].valor = 0.0;
        for item in &mut v.puntajes[0].items {
            item.nivel = 0;
        }
        v.hallazgos.push(hallazgo("nuevo"));
        v
    }

    /// Veredicto en que la seguridad no tiene veredicto.
    fn sin(base: &Veredicto) -> Veredicto {
        let mut v = base.clone();
        v.puntajes[0].sin_veredicto = Some("sin sonda".into());
        v
    }

    /// Evaluador falso. `base_dir` es la base; cualquier otra ruta es una copia. `regla` recibe si la copia
    /// es igual a la base (limpia), su contenido y el veredicto base.
    fn falso<'a>(
        base_dir: &'a Path,
        base: &'a Veredicto,
        regla: impl Fn(bool, &BTreeMap<String, String>, &Veredicto) -> Veredicto + Sync + 'a,
    ) -> impl Fn(&Path) -> Result<Veredicto, ErrorOraculo> + Sync + 'a {
        let arbol_base = arbol(base_dir);
        move |ruta: &Path| {
            if ruta == base_dir {
                return Ok(base.clone());
            }
            let copia = arbol(ruta);
            let limpia = copia == arbol_base;
            Ok(regla(limpia, &copia, base))
        }
    }

    /// Tarea de prueba en un directorio temporal, que se devuelve para que viva con el test.
    fn tarea_prueba() -> (TempDir, Tarea) {
        let dir = tempfile::tempdir().expect("temporal");
        let tarea = tarea_en(dir.path(), "");
        (dir, tarea)
    }

    fn calibrar(
        base_dir: &Path,
        base: &Veredicto,
        regla: impl Fn(bool, &BTreeMap<String, String>, &Veredicto) -> Veredicto + Sync,
        n: usize,
    ) -> Fila {
        let (_t, tarea) = tarea_prueba();
        let ev = falso(base_dir, base, regla);
        calibrar_con(&tarea, Dimension::Seguridad, n, 42, base_dir, &ev).expect("calibra")
    }

    #[test]
    fn calibra_detector_perfecto_da_1_en_sensibilidad_especificidad_y_exactitud() {
        let base = veredicto_base();
        let fila = calibrar(
            Path::new(FIXTURE),
            &base,
            |limpia, _, base| if limpia { base.clone() } else { hundido(base) },
            20,
        );
        assert_eq!(fila.sensibilidad, Some(1.0));
        assert_eq!(fila.especificidad, Some(1.0));
        assert_eq!(fila.exactitud_balanceada, Some(1.0));
        assert_eq!(
            (fila.defectos, fila.limpios, fila.no_aplicable),
            (10, 10, 0)
        );
        assert_eq!(fila.items.get("seguridad.secretos"), Some(&1.0));
        assert_eq!(fila.items.get("seguridad.panico_en_entrada"), Some(&1.0));
    }

    #[test]
    fn calibra_detector_nulo_da_sensibilidad_0_especificidad_1_y_exactitud_medio() {
        let base = veredicto_base();
        let fila = calibrar(Path::new(FIXTURE), &base, |_, _, base| base.clone(), 20);
        assert_eq!(fila.sensibilidad, Some(0.0));
        assert_eq!(fila.especificidad, Some(1.0));
        assert_eq!(fila.exactitud_balanceada, Some(0.5));
    }

    #[test]
    fn calibra_limpio_sin_veredicto_cuenta_como_falso_positivo() {
        let base = veredicto_base();
        let fila = calibrar(
            Path::new(FIXTURE),
            &base,
            |limpia, _, base| if limpia { sin(base) } else { hundido(base) },
            20,
        );
        assert_eq!(fila.sensibilidad, Some(1.0));
        assert_eq!(
            fila.especificidad,
            Some(0.0),
            "todo limpio sin veredicto es FP"
        );
        assert_eq!(fila.exactitud_balanceada, Some(0.5));
    }

    #[test]
    fn calibra_defecto_sin_veredicto_es_falso_negativo() {
        let base = veredicto_base();
        let fila = calibrar(
            Path::new(FIXTURE),
            &base,
            |limpia, _, base| if limpia { base.clone() } else { sin(base) },
            20,
        );
        assert_eq!(
            fila.sensibilidad,
            Some(0.0),
            "un defecto sin veredicto es FN"
        );
        assert_eq!(fila.especificidad, Some(1.0));
        assert_eq!(fila.exactitud_balanceada, Some(0.5));
    }

    #[test]
    fn calibra_hallazgo_con_id_nuevo_cuenta_y_el_de_la_base_no() {
        let mut base = veredicto_base();
        base.hallazgos.push(hallazgo("viejo"));
        // Los defectos agregan un hallazgo con id nuevo; el de la base sigue en todas las copias.
        let con_nuevo = calibrar(
            Path::new(FIXTURE),
            &base,
            |limpia, _, base| {
                let mut v = base.clone();
                if !limpia {
                    v.hallazgos.push(hallazgo("nuevo"));
                }
                v
            },
            20,
        );
        assert_eq!(con_nuevo.sensibilidad, Some(1.0));
        assert_eq!(con_nuevo.especificidad, Some(1.0));
        // Sin id nuevo, el hallazgo de la base no cuenta como detección.
        let solo_base = calibrar(Path::new(FIXTURE), &base, |_, _, base| base.clone(), 20);
        assert_eq!(solo_base.sensibilidad, Some(0.0));
    }

    #[test]
    fn calibra_por_item_mide_cada_item_y_omite_lo_no_medido() {
        let base = veredicto_base();
        // Solo el secreto baja su ítem; el panico no se detecta nunca.
        let fila = calibrar(
            Path::new(FIXTURE),
            &base,
            |_, copia, base| {
                let mut v = base.clone();
                if copia.values().any(|t| t.contains("AKIA")) {
                    for item in &mut v.puntajes[0].items {
                        if item.item == "seguridad.secretos" {
                            item.nivel = 0;
                        }
                    }
                }
                v
            },
            20,
        );
        assert_eq!(fila.items.get("seguridad.secretos"), Some(&1.0));
        assert_eq!(fila.items.get("seguridad.panico_en_entrada"), Some(&0.5));
        assert_eq!(
            fila.sensibilidad,
            Some(0.5),
            "5 de 10 defectos, los del secreto"
        );
        assert_eq!(fila.exactitud_balanceada, Some(0.75));
    }

    #[test]
    fn calibra_sin_casos_limpios_deja_especificidad_none() {
        let base = veredicto_base();
        let fila = calibrar(
            Path::new(FIXTURE),
            &base,
            |limpia, _, base| if limpia { base.clone() } else { hundido(base) },
            1,
        );
        assert_eq!((fila.defectos, fila.limpios), (1, 0));
        assert_eq!(fila.sensibilidad, Some(1.0));
        assert_eq!(fila.especificidad, None, "sin limpios no hay especificidad");
        assert_eq!(fila.exactitud_balanceada, None);
        assert!(fila.items.is_empty(), "un ítem sin su clase no se mide");
    }

    #[test]
    fn calibra_misma_semilla_da_la_misma_fila_salvo_ts() {
        let base = veredicto_base();
        let regla = |limpia: bool, _: &BTreeMap<String, String>, base: &Veredicto| {
            if limpia { base.clone() } else { hundido(base) }
        };
        let a = calibrar(Path::new(FIXTURE), &base, regla, 20);
        let b = calibrar(Path::new(FIXTURE), &base, regla, 20);
        assert_eq!(Fila { ts: 0.0, ..a }, Fila { ts: 0.0, ..b });
    }

    #[test]
    fn calibra_sin_defectos_aplicables_los_defectos_son_no_aplicables() {
        let dir = copia_fixture();
        crate::borrado::borrar_archivo_dentro(dir.path(), &dir.path().join("Dockerfile"))
            .expect("borra el Dockerfile");
        let base = Veredicto {
            puntajes: vec![Puntaje {
                dimension: Dimension::Entrega,
                valor: 1.0,
                calibrada: false,
                exactitud_balanceada: None,
                items: vec![],
                sin_veredicto: None,
                faltantes: Vec::new(),
            }],
            ..Veredicto::default()
        };
        let (_t, tarea) = tarea_prueba();
        let ev = falso(dir.path(), &base, |_, _, base| base.clone());
        let fila = calibrar_con(&tarea, Dimension::Entrega, 20, 42, dir.path(), &ev)
            .expect("sin defectos aplicables igual hay fila");
        assert_eq!(
            (fila.defectos, fila.limpios, fila.no_aplicable),
            (0, 10, 10)
        );
        assert_eq!(fila.n, 10);
        assert_eq!(fila.sensibilidad, None);
        assert_eq!(fila.exactitud_balanceada, None);
    }

    #[test]
    fn calibra_camino_caliente_sin_funciones_cuenta_no_aplicables() {
        let dir = copia_fixture();
        let base = Veredicto {
            puntajes: vec![Puntaje {
                dimension: Dimension::Eficiencia,
                valor: 1.0,
                calibrada: false,
                exactitud_balanceada: None,
                items: vec![],
                sin_veredicto: None,
                faltantes: Vec::new(),
            }],
            ..Veredicto::default()
        };
        let tarea = tarea_en(
            dir.path(),
            "\n[calibracion]\ncamino_caliente = [\"no_existe\"]\n",
        );
        let ev = falso(dir.path(), &base, |_, _, base| base.clone());
        let fila = calibrar_con(&tarea, Dimension::Eficiencia, 10, 42, dir.path(), &ev)
            .expect("sin función del camino igual hay fila");
        assert_eq!(
            (fila.defectos, fila.limpios, fila.no_aplicable),
            (0, 5, 5),
            "el bucle no tiene dónde caer: cada caso con defecto es no aplicable"
        );
    }

    #[test]
    fn calibra_camino_caliente_con_funcion_existente_aplica_el_bucle() {
        let dir = copia_fixture();
        let base = Veredicto {
            puntajes: vec![Puntaje {
                dimension: Dimension::Eficiencia,
                valor: 1.0,
                calibrada: false,
                exactitud_balanceada: None,
                items: vec![],
                sin_veredicto: None,
                faltantes: Vec::new(),
            }],
            ..Veredicto::default()
        };
        let tarea = tarea_en(
            dir.path(),
            "\n[calibracion]\ncamino_caliente = [\"ejecutar\"]\n",
        );
        let ev = falso(dir.path(), &base, |_, _, base| base.clone());
        let fila =
            calibrar_con(&tarea, Dimension::Eficiencia, 10, 42, dir.path(), &ev).expect("hay fila");
        assert_eq!((fila.defectos, fila.no_aplicable), (5, 0));
    }

    #[test]
    fn calibra_base_sin_veredicto_es_infraestructura() {
        let base = sin(&veredicto_base());
        let (_t, tarea) = tarea_prueba();
        let ev = falso(Path::new(FIXTURE), &base, |_, _, base| base.clone());
        let error = calibrar_con(
            &tarea,
            Dimension::Seguridad,
            20,
            42,
            Path::new(FIXTURE),
            &ev,
        )
        .expect_err("sin base no hay medición");
        assert!(matches!(error, ErrorOraculo::Infra(_)), "{error:?}");
        assert_eq!(error.codigo_de_salida(), 3);
    }

    #[test]
    fn calibra_dimension_sin_defectos_o_n_cero_es_uso() {
        let base = veredicto_base();
        let (_t, tarea) = tarea_prueba();
        let ev = falso(Path::new(FIXTURE), &base, |_, _, base| base.clone());
        let sin_defectos =
            calibrar_con(&tarea, Dimension::Funciona, 20, 42, Path::new(FIXTURE), &ev)
                .expect_err("funciona no tiene defectos");
        assert_eq!(sin_defectos.codigo_de_salida(), 2);
        let n_cero = calibrar_con(&tarea, Dimension::Seguridad, 0, 42, Path::new(FIXTURE), &ev)
            .expect_err("n cero");
        assert_eq!(n_cero.codigo_de_salida(), 2);
    }

    #[test]
    fn calibra_repo_de_tarea_se_resuelve_desde_su_carpeta() {
        let home = tempfile::tempdir().expect("home");
        let carpeta = home.path().join("oraculos").join("calibra-fixture");
        std::fs::create_dir_all(&carpeta).expect("carpeta");
        let repo = carpeta.join("repo-bueno");
        std::fs::create_dir_all(&repo).expect("repo");
        super::inyectar::pruebas::copiar(Path::new(FIXTURE), &repo);
        let _ = tarea_en(&carpeta, "\n[calibracion]\nrepo = \"repo-bueno\"\n");
        let vistos = Mutex::new(Vec::<PathBuf>::new());
        let evaluador = |_: &Tarea, ruta: &Path, _: &OpcionesOraculo| {
            vistos.lock().expect("lock").push(ruta.to_path_buf());
            Ok(veredicto_base())
        };
        calibrar_tarea(
            home.path(),
            "calibra-fixture",
            "seguridad",
            2,
            42,
            None,
            &evaluador,
        )
        .expect("calibra con el repo de la tarea");
        assert_eq!(vistos.lock().expect("lock")[0], repo);
    }

    /// `tests/protocolo.rs` de la calibración de Pruebas: un test de `leer_numero` con dos asserts.
    const TEST_DE_LEER_NUMERO: &str = "use calibra_fixture::protocolo::leer_numero;\n\n#[test]\nfn leer_numero_lee_el_numero() {\n    assert!(leer_numero(\"42\").is_ok());\n    assert_eq!(leer_numero(\"7\").unwrap(), 7);\n}\n";

    /// Veredicto de Pruebas sano: valor 1 y el ítem de mutación en nivel 3.
    fn veredicto_de_pruebas() -> Veredicto {
        Veredicto {
            puntajes: vec![Puntaje {
                dimension: Dimension::Pruebas,
                valor: 1.0,
                calibrada: false,
                exactitud_balanceada: None,
                items: vec![nivel_item("pruebas.mutacion", 3)],
                sin_veredicto: None,
                faltantes: Vec::new(),
            }],
            ..Veredicto::default()
        }
    }

    /// Los muertos de la base llegan al inyector de AssertBorrado: el memo se pasa solo a la base.
    #[test]
    fn calibra_pruebas_los_muertos_de_la_base_llegan_al_inyector() {
        use crate::oracle::mutacion::{ARCHIVO_OUTCOMES, construir_comando};
        use crate::oracle::perfiles::herramientas::SalidaHerramienta;
        const OUTCOMES: &str = r#"{"total_mutants":1,"outcomes":[{"scenario":"Baseline","summary":"Success"},{"scenario":{"Mutant":{"name":"src/protocolo.rs:2:1: replace leer_numero -> Result<u32, ParseIntError> with Ok(0)","file":"src/protocolo.rs","span":{"start":{"line":2,"column":1}}}},"summary":"CaughtMutant"}]}"#;
        let home = tempfile::tempdir().expect("home");
        let carpeta = home.path().join("oraculos").join("calibra-fixture");
        std::fs::create_dir_all(&carpeta).expect("carpeta");
        let repo = carpeta.join("repo-bueno");
        std::fs::create_dir_all(&repo).expect("repo");
        super::inyectar::pruebas::copiar(Path::new(FIXTURE), &repo);
        std::fs::write(repo.join("tests/protocolo.rs"), TEST_DE_LEER_NUMERO).expect("test");
        let _ = tarea_en(&carpeta, "\n[calibracion]\nrepo = \"repo-bueno\"\n");
        let con_memo = Mutex::new(Vec::<bool>::new());
        let evaluador = |tarea: &Tarea, _: &Path, o: &OpcionesOraculo| {
            con_memo.lock().expect("lock").push(o.memo.is_some());
            if let Some(memo) = o.memo.as_deref() {
                let mut salida = SalidaHerramienta {
                    codigo: 0,
                    stdout: "salida".into(),
                    ..Default::default()
                };
                salida
                    .archivos
                    .insert(ARCHIVO_OUTCOMES.into(), OUTCOMES.into());
                memo.anotar(&construir_comando(tarea), &salida);
            }
            Ok(veredicto_de_pruebas())
        };
        let calibrada = calibrar_tarea_con(
            home.path(),
            "calibra-fixture",
            "pruebas",
            2,
            42,
            None,
            None,
            &evaluador,
        )
        .expect("calibra");
        let vistos = con_memo.lock().expect("lock");
        assert!(vistos[0], "la base lleva el memo");
        assert!(
            vistos[1..].iter().all(|m| !m),
            "los casos no graban: {vistos:?}"
        );
        let ruta = calibrada.casos.expect("hay casos guardados");
        let texto = fs::read_to_string(&ruta).expect("lee los casos");
        let casos: Vec<CasoGuardado> = texto
            .lines()
            .map(|l| serde_json::from_str(l).expect("un caso por línea"))
            .collect();
        let con_defecto = casos
            .iter()
            .find(|c| c.defecto.as_deref() == Some("assert_borrado"))
            .expect("un caso con AssertBorrado aplicado");
        assert_eq!(con_defecto.archivo.as_deref(), Some("tests/protocolo.rs"));
        assert_eq!(con_defecto.linea, Some(5), "el primer assert del test");
    }

    /// Una llamada del evaluador de prueba: si era la base (lleva memo), su alcance de reuso y los comandos que corrió.
    struct Llamada {
        base: bool,
        /// Alcance del reuso de la llamada: `None` sin reuso (la base y los casos con defecto fuera de él).
        alcance: Option<Alcance>,
        cmds: Vec<String>,
        /// Cuántas veces corrió el juez y los tests en la llamada. Ni uno ni otro pasan por el ejecutor.
        juez: u32,
        tests: u32,
    }

    /// Ejecutor de prueba: anota cada comando. La mutación responde un mutante muerto, para `AssertBorrado`.
    struct Anota<'a> {
        cmds: &'a Mutex<Vec<String>>,
    }

    impl EjecutorHerramienta for Anota<'_> {
        fn ejecutar(&self, cmd: &str) -> Result<SalidaHerramienta, ErrorOraculo> {
            self.cmds.lock().expect("lock").push(cmd.to_string());
            let mut salida = SalidaHerramienta::default();
            if cmd.contains("cargo mutants") && cmd.contains("--list") {
                // Un solo mutante en el listado: la corrida lo filtra con `-F` y el fake la responde igual.
                salida.archivos.insert(
                    "listado.json".into(),
                    r#"[{"name":"src/protocolo.rs:2:1: replace leer_numero -> Result<u32, ParseIntError> with Ok(0)"}]"#
                        .into(),
                );
            } else if cmd.contains("cargo mutants") {
                salida
                    .archivos
                    .insert(ARCHIVO_OUTCOMES.into(), OUTCOMES_CON_MUERTO.into());
            }
            Ok(salida)
        }

        fn tiene_rustsec(&self) -> bool {
            false
        }
    }

    const OUTCOMES_CON_MUERTO: &str = r#"{"total_mutants":1,"outcomes":[{"scenario":"Baseline","summary":"Success"},{"scenario":{"Mutant":{"name":"src/protocolo.rs:2:1: replace leer_numero -> Result<u32, ParseIntError> with Ok(0)","file":"src/protocolo.rs","span":{"start":{"line":2,"column":1}}}},"summary":"CaughtMutant"}]}"#;

    /// Carpeta `calibra-fixture` de `home` con `repo-bueno` (copia del fixture) y la tarea que la usa.
    fn carpeta_con_repo_bueno(home: &Path) -> PathBuf {
        let carpeta = home.join("oraculos").join("calibra-fixture");
        std::fs::create_dir_all(&carpeta).expect("carpeta");
        let repo = carpeta.join("repo-bueno");
        std::fs::create_dir_all(&repo).expect("repo");
        super::inyectar::pruebas::copiar(Path::new(FIXTURE), &repo);
        tarea_en(&carpeta, "\n[calibracion]\nrepo = \"repo-bueno\"\n");
        carpeta
    }

    /// Evaluador de prueba que compone el ejecutor como `evaluar_con`: reuso, y el memo de la base. Corre una
    /// sonda estática y la mutación, y registra lo que corrió en cada llamada.
    fn evaluador_con_registro(
        llamadas: &Mutex<Vec<Llamada>>,
        veredicto: fn() -> Veredicto,
    ) -> impl Fn(&Tarea, &Path, &OpcionesOraculo) -> Result<Veredicto, ErrorOraculo> + '_ {
        move |tarea: &Tarea, _: &Path, o: &OpcionesOraculo| {
            let cmds = Mutex::new(Vec::<String>::new());
            let real = Anota { cmds: &cmds };
            let ejecutar = |cmd: &str| match o.reuso.as_ref() {
                Some(reuso) => {
                    crate::oracle::memo::EjecutorReusador { real: &real, reuso }.ejecutar(cmd)
                }
                None => real.ejecutar(cmd),
            };
            ejecutar("cargo clippy --version").expect("sonda estática");
            let mutacion = construir_comando(tarea);
            let salida = ejecutar(&mutacion).expect("mutación");
            if let Some(memo) = o.memo.as_deref() {
                memo.anotar(&mutacion, &salida);
            }
            llamadas.lock().expect("lock").push(Llamada {
                base: o.memo.is_some(),
                alcance: o.reuso.as_ref().map(|r| r.alcance),
                cmds: cmds.lock().expect("lock").clone(),
                juez: 0,
                tests: 0,
            });
            Ok(veredicto())
        }
    }

    /// Evaluador de prueba con sondas deterministas, juez y tests. Corre gitleaks, cargo audit, clippy,
    /// hadolint y la mutación. El juez y los tests los cuenta el fake: no pasan por el ejecutor. Con
    /// `usar_reuso` falso ignora `o.reuso` y corre todo: es la evaluación sin reuso.
    fn evaluador_con_juez_y_tests(
        llamadas: &Mutex<Vec<Llamada>>,
        veredicto: fn() -> Veredicto,
        usar_reuso: bool,
    ) -> impl Fn(&Tarea, &Path, &OpcionesOraculo) -> Result<Veredicto, ErrorOraculo> + '_ {
        move |tarea: &Tarea, _: &Path, o: &OpcionesOraculo| {
            let cmds = Mutex::new(Vec::<String>::new());
            let real = Anota { cmds: &cmds };
            let reuso = o.reuso.as_ref().filter(|_| usar_reuso);
            // Como `evaluar_con`: el reuso va dentro, y la base graba cada salida OK en el memo.
            let ejecutar = |cmd: &str| -> Result<SalidaHerramienta, ErrorOraculo> {
                let con_reuso = match reuso {
                    Some(reuso) => {
                        crate::oracle::memo::EjecutorReusador { real: &real, reuso }.ejecutar(cmd)
                    }
                    None => real.ejecutar(cmd),
                };
                match o.memo.as_deref() {
                    Some(memo) => {
                        let salida = con_reuso?;
                        memo.anotar(cmd, &salida);
                        Ok(salida)
                    }
                    None => con_reuso,
                }
            };
            let sondas = [
                COMANDO_GITLEAKS.to_string(),
                COMANDO_CARGO_AUDIT.to_string(),
                comando_clippy(),
                crate::oracle::configs::comando_hadolint("Dockerfile"),
            ];
            for cmd in &sondas {
                ejecutar(cmd).expect("sonda determinista");
            }
            ejecutar(&construir_comando(tarea)).expect("mutación");
            llamadas.lock().expect("lock").push(Llamada {
                base: o.memo.is_some(),
                alcance: reuso.map(|r| r.alcance),
                cmds: cmds.lock().expect("lock").clone(),
                juez: 1,
                tests: 1,
            });
            Ok(veredicto())
        }
    }

    /// Cuenta las corridas de mutación. El listado no cuenta: es otro comando de `cargo mutants`.
    fn cuenta_mutaciones(llamada: &Llamada) -> usize {
        llamada
            .cmds
            .iter()
            .filter(|c| c.contains("--output /salida/mutants"))
            .count()
    }

    /// Un caso de Diseño toma la mutación de la base y corre otra vez las sondas estáticas y el juez.
    #[test]
    fn calibra_un_caso_de_diseno_toma_la_mutacion_de_la_base() {
        let home = tempfile::tempdir().expect("home");
        carpeta_con_repo_bueno(home.path());
        let llamadas = Mutex::new(Vec::<Llamada>::new());
        let evaluador =
            evaluador_con_registro(&llamadas, || veredicto_de(Dimension::Diseno, 1.0, None));
        let calibrada = calibrar_tarea_con(
            home.path(),
            "calibra-fixture",
            "diseno",
            4,
            42,
            None,
            None,
            &evaluador,
        )
        .expect("calibra");
        let llamadas = llamadas.lock().expect("lock");
        let casos = calibrada.casos.expect("hay casos guardados");
        let medidos = fs::read_to_string(&casos)
            .expect("lee los casos")
            .lines()
            .count();
        assert_eq!(
            llamadas.len(),
            medidos + 1,
            "la base y cada caso llaman al evaluador"
        );
        assert!(llamadas[0].base, "la primera llamada es la base");
        assert_eq!(
            cuenta_mutaciones(&llamadas[0]),
            1,
            "la base corre la mutación"
        );
        assert!(
            llamadas[0]
                .cmds
                .iter()
                .any(|c| c == "cargo clippy --version"),
            "la base corre las sondas estáticas"
        );
        let con_defecto: Vec<&Llamada> = llamadas[1..]
            .iter()
            .filter(|l| l.alcance == Some(Alcance::SoloMutacion))
            .collect();
        assert!(!con_defecto.is_empty(), "hay un caso con defecto de Diseño");
        for l in con_defecto {
            assert_eq!(
                cuenta_mutaciones(l),
                0,
                "el defecto de Diseño reusa la mutación de la base"
            );
            assert!(
                l.cmds.iter().any(|c| c == "cargo clippy --version"),
                "el defecto de Diseño corre sus sondas: {:?}",
                l.cmds
            );
        }
        let limpios: Vec<&Llamada> = llamadas[1..]
            .iter()
            .filter(|l| l.alcance == Some(Alcance::Deterministas))
            .collect();
        assert!(!limpios.is_empty(), "hay un caso limpio");
        for l in limpios {
            assert_eq!(cuenta_mutaciones(l), 0, "un caso limpio toma la mutación");
        }
    }

    /// Un caso con defecto de Pruebas corre la mutación: Pruebas la mide, así que no la reusa. Un caso limpio
    /// sí la toma de la base: la copia es idéntica.
    #[test]
    fn calibra_un_caso_de_pruebas_corre_la_mutacion() {
        let home = tempfile::tempdir().expect("home");
        let carpeta = carpeta_con_repo_bueno(home.path());
        let repo = carpeta.join("repo-bueno");
        std::fs::write(repo.join("tests/protocolo.rs"), TEST_DE_LEER_NUMERO).expect("test");
        let llamadas = Mutex::new(Vec::<Llamada>::new());
        let evaluador = evaluador_con_registro(&llamadas, veredicto_de_pruebas);
        calibrar_tarea_con(
            home.path(),
            "calibra-fixture",
            "pruebas",
            2,
            42,
            None,
            None,
            &evaluador,
        )
        .expect("calibra");
        let llamadas = llamadas.lock().expect("lock");
        assert!(llamadas.len() >= 2, "la base y un caso con defecto");
        assert_eq!(
            cuenta_mutaciones(&llamadas[0]),
            1,
            "la base corre la mutación"
        );
        let con_defecto: Vec<&Llamada> = llamadas[1..]
            .iter()
            .filter(|l| l.alcance.is_none())
            .collect();
        assert!(
            !con_defecto.is_empty(),
            "hay un caso con defecto de Pruebas"
        );
        for l in con_defecto {
            assert_eq!(cuenta_mutaciones(l), 1, "Pruebas no reusa la mutación");
        }
        let limpios: Vec<&Llamada> = llamadas[1..]
            .iter()
            .filter(|l| l.alcance == Some(Alcance::Deterministas))
            .collect();
        assert!(!limpios.is_empty(), "hay un caso limpio");
        for l in limpios {
            assert_eq!(cuenta_mutaciones(l), 0, "un caso limpio toma la mutación");
        }
    }

    fn cuenta_comando(llamada: &Llamada, cmd: &str) -> usize {
        llamada.cmds.iter().filter(|c| c.as_str() == cmd).count()
    }

    /// Un caso limpio toma de la base las sondas deterministas y corre otra vez el juez y los tests. Un caso
    /// con defecto de Diseño solo toma la mutación: sus sondas deterministas corren otra vez.
    #[test]
    fn calibra_un_caso_limpio_toma_de_la_base_las_sondas_deterministas_y_corre_el_juez_y_los_tests()
    {
        let home = tempfile::tempdir().expect("home");
        let carpeta = carpeta_con_repo_bueno(home.path());
        std::fs::write(
            carpeta.join("repo-bueno").join("Dockerfile"),
            "FROM scratch\n",
        )
        .expect("Dockerfile");
        let llamadas = Mutex::new(Vec::<Llamada>::new());
        let evaluador = evaluador_con_juez_y_tests(
            &llamadas,
            || veredicto_de(Dimension::Diseno, 1.0, None),
            true,
        );
        let calibrada = calibrar_tarea_con(
            home.path(),
            "calibra-fixture",
            "diseno",
            4,
            42,
            None,
            None,
            &evaluador,
        )
        .expect("calibra");
        let llamadas = llamadas.lock().expect("lock");
        let casos = calibrada.casos.expect("hay casos guardados");
        let medidos = fs::read_to_string(&casos)
            .expect("lee los casos")
            .lines()
            .count();
        assert_eq!(
            llamadas.len(),
            medidos + 1,
            "la base y cada caso llaman al evaluador"
        );
        assert!(
            llamadas.iter().all(|l| l.juez == 1),
            "el juez corre en la base y en cada caso"
        );
        assert!(
            llamadas.iter().all(|l| l.tests == 1),
            "los tests corren en la base y en cada caso"
        );
        let deterministas = [
            COMANDO_GITLEAKS.to_string(),
            COMANDO_CARGO_AUDIT.to_string(),
            comando_clippy(),
            crate::oracle::configs::comando_hadolint("Dockerfile"),
        ];
        assert!(llamadas[0].base, "la primera llamada es la base");
        for cmd in &deterministas {
            assert_eq!(cuenta_comando(&llamadas[0], cmd), 1, "la base corre {cmd}");
        }
        assert_eq!(
            cuenta_mutaciones(&llamadas[0]),
            1,
            "la base corre la mutación"
        );
        let limpios: Vec<&Llamada> = llamadas[1..]
            .iter()
            .filter(|l| l.alcance == Some(Alcance::Deterministas))
            .collect();
        assert!(!limpios.is_empty(), "hay casos limpios");
        for l in limpios {
            assert!(
                l.cmds.is_empty(),
                "un caso limpio no corre sondas: {:?}",
                l.cmds
            );
        }
        for l in llamadas[1..]
            .iter()
            .filter(|l| l.alcance == Some(Alcance::SoloMutacion))
        {
            assert_eq!(
                cuenta_mutaciones(l),
                0,
                "el defecto de Diseño reusa la mutación"
            );
            for cmd in &deterministas {
                assert_eq!(cuenta_comando(l, cmd), 1, "el defecto corre {cmd}");
            }
        }
    }

    /// Un caso limpio da la misma especificidad con y sin reuso: el veredicto del fake es el mismo.
    #[test]
    fn calibra_un_caso_limpio_da_la_misma_especificidad_con_y_sin_reuso() {
        let calibrar = |usar_reuso: bool| {
            let home = tempfile::tempdir().expect("home");
            carpeta_con_repo_bueno(home.path());
            let llamadas = Mutex::new(Vec::<Llamada>::new());
            let evaluador = evaluador_con_juez_y_tests(
                &llamadas,
                || veredicto_de(Dimension::Diseno, 1.0, None),
                usar_reuso,
            );
            calibrar_tarea_con(
                home.path(),
                "calibra-fixture",
                "diseno",
                4,
                42,
                None,
                None,
                &evaluador,
            )
            .expect("calibra")
            .fila
        };
        let con = calibrar(true);
        let sin = calibrar(false);
        assert!(con.limpios > 0, "hay casos limpios");
        assert_eq!(con.limpios, sin.limpios);
        assert_eq!(con.defectos, sin.defectos);
        assert_eq!(con.especificidad, sin.especificidad);
    }

    /// Una calibración no escribe historial: cada caso es una versión distinta y se evalúa una vez.
    #[test]
    fn calibra_evalua_sin_historial() {
        let home = tempfile::tempdir().expect("home");
        let carpeta = home.path().join("oraculos").join("calibra-fixture");
        std::fs::create_dir_all(&carpeta).expect("carpeta");
        let _ = tarea_en(&carpeta, "");
        let historiales = Mutex::new(Vec::<bool>::new());
        let evaluador = |_: &Tarea, _: &Path, o: &OpcionesOraculo| {
            historiales.lock().expect("lock").push(o.historial);
            Ok(veredicto_base())
        };
        calibrar_tarea(
            home.path(),
            "calibra-fixture",
            "seguridad",
            2,
            42,
            Some(Path::new(FIXTURE)),
            &evaluador,
        )
        .expect("calibra");
        let vistos = historiales.lock().expect("lock");
        assert!(!vistos.is_empty(), "el evaluador no recibió llamadas");
        assert!(vistos.iter().all(|h| !h), "{vistos:?}");
    }

    #[test]
    fn calibra_sin_repo_ni_calibracion_es_uso() {
        let home = tempfile::tempdir().expect("home");
        let carpeta = home.path().join("oraculos").join("calibra-fixture");
        std::fs::create_dir_all(&carpeta).expect("carpeta");
        let _ = tarea_en(&carpeta, "");
        let evaluador = |_: &Tarea, _: &Path, _: &OpcionesOraculo| Ok(veredicto_base());
        let error = calibrar_tarea(
            home.path(),
            "calibra-fixture",
            "seguridad",
            2,
            42,
            None,
            &evaluador,
        )
        .expect_err("sin repo no hay calibración");
        assert_eq!(error.codigo_de_salida(), 2);
        assert!(!home.path().join(ARCHIVO).exists(), "no escribe nada");
    }

    fn fila_de_prueba(tarea: &str, ts: f64) -> Fila {
        fila(tarea, Dimension::Seguridad, ts, Some(0.95))
    }

    #[test]
    fn calibra_agregar_no_borra_las_filas_anteriores() {
        let home = tempfile::tempdir().expect("home");
        let anterior = fila_de_prueba("memora", 1.0);
        let inicial = Calibraciones {
            v: VERSION,
            filas: vec![anterior.clone()],
        };
        std::fs::write(
            home.path().join(ARCHIVO),
            serde_json::to_string(&inicial).expect("serializa"),
        )
        .expect("escribe");
        agregar(home.path(), &fila_de_prueba("memora", 2.0)).expect("agrega");
        let leido = Calibraciones::cargar(home.path()).expect("carga");
        assert_eq!(leido.filas.len(), 2);
        assert_eq!(leido.filas[0], anterior);
        assert_eq!(leido.filas[1].ts, 2.0);
    }

    #[test]
    fn calibra_agregar_crea_el_archivo_si_falta() {
        let home = tempfile::tempdir().expect("home");
        agregar(home.path(), &fila_de_prueba("memora", 1.0)).expect("agrega");
        let leido = Calibraciones::cargar(home.path()).expect("carga");
        assert_eq!(leido.filas.len(), 1);
    }

    #[test]
    fn calibra_tarea_escribe_una_fila_nueva_y_conserva_las_anteriores() {
        let home = tempfile::tempdir().expect("home");
        let carpeta = home.path().join("oraculos").join("calibra-fixture");
        std::fs::create_dir_all(&carpeta).expect("carpeta");
        let _ = tarea_en(&carpeta, "");
        let evaluador = |_: &Tarea, _: &Path, _: &OpcionesOraculo| Ok(veredicto_base());
        for _ in 0..2 {
            calibrar_tarea(
                home.path(),
                "calibra-fixture",
                "seguridad",
                2,
                42,
                Some(Path::new(FIXTURE)),
                &evaluador,
            )
            .expect("calibra");
        }
        let leido = Calibraciones::cargar(home.path()).expect("carga");
        assert_eq!(leido.filas.len(), 2, "una fila por corrida");
        assert_eq!(leido.filas[0].tarea, "calibra-fixture");
    }

    #[test]
    fn calibra_tarea_dimension_desconocida_es_uso() {
        let home = tempfile::tempdir().expect("home");
        let evaluador = |_: &Tarea, _: &Path, _: &OpcionesOraculo| Ok(veredicto_base());
        let error = calibrar_tarea(
            home.path(),
            "calibra-fixture",
            "magia",
            2,
            42,
            None,
            &evaluador,
        )
        .expect_err("dimensión inventada");
        assert_eq!(error.codigo_de_salida(), 2);
    }

    pub(super) fn puntaje(dimension: Dimension, valor: f32) -> Puntaje {
        Puntaje {
            dimension,
            valor,
            calibrada: false,
            exactitud_balanceada: None,
            items: Vec::new(),
            sin_veredicto: None,
            faltantes: Vec::new(),
        }
    }

    fn home_con(texto: &str) -> tempfile::TempDir {
        let home = tempfile::tempdir().expect("home");
        fs::write(home.path().join(ARCHIVO), texto).expect("escribe");
        home
    }

    #[test]
    fn calibra_sin_archivo_da_vacio() {
        let home = tempfile::tempdir().expect("home");
        let c = Calibraciones::cargar(home.path()).expect("sin archivo no es error");
        assert!(c.filas.is_empty());
        assert_eq!(c.v, VERSION);
    }

    #[test]
    fn calibra_json_roto_es_error_de_infraestructura() {
        let home = home_con("{\"v\": 1, \"filas\": [");
        let error = Calibraciones::cargar(home.path()).expect_err("JSON roto");
        assert!(matches!(error, ErrorOraculo::Infra(_)), "{error:?}");
        assert_eq!(error.codigo_de_salida(), 3);
    }

    #[test]
    fn calibra_version_desconocida_es_error_de_infraestructura() {
        let home = home_con("{\"v\": 9, \"filas\": []}");
        let error = Calibraciones::cargar(home.path()).expect_err("otra versión");
        assert_eq!(error.codigo_de_salida(), 3);
    }

    #[test]
    fn calibra_de_elige_la_fila_mas_reciente_por_ts() {
        let c = Calibraciones {
            v: VERSION,
            filas: vec![
                fila("memora", Dimension::Seguridad, 200.0, Some(0.5)),
                fila("memora", Dimension::Seguridad, 100.0, Some(0.95)),
                fila("brasa", Dimension::Seguridad, 300.0, Some(0.1)),
            ],
        };
        let f = c.de("memora", Dimension::Seguridad).expect("hay fila");
        assert_eq!(f.exactitud_balanceada, Some(0.5));
        assert!(c.de("memora", Dimension::Diseno).is_none());
    }

    #[test]
    fn calibra_umbral_es_0_9_inclusive() {
        let c = Calibraciones {
            v: VERSION,
            filas: vec![
                fila("memora", Dimension::Seguridad, 1.0, Some(0.9)),
                fila("memora", Dimension::Diseno, 1.0, Some(0.89)),
            ],
        };
        let mut puntajes = vec![
            puntaje(Dimension::Seguridad, 0.8),
            puntaje(Dimension::Diseno, 0.8),
        ];
        c.aplicar("memora", &mut puntajes);
        assert!(puntajes[0].calibrada);
        assert_eq!(puntajes[0].exactitud_balanceada, Some(0.9));
        assert!(!puntajes[1].calibrada);
        assert_eq!(puntajes[1].exactitud_balanceada, Some(0.89));
    }

    #[test]
    fn calibra_global_promedia_solo_calibradas_con_veredicto() {
        let c = Calibraciones {
            v: VERSION,
            filas: vec![
                fila("memora", Dimension::Seguridad, 1.0, Some(0.95)),
                fila("memora", Dimension::Diseno, 1.0, Some(0.7)),
                fila("memora", Dimension::Funciona, 1.0, Some(0.99)),
            ],
        };
        let mut puntajes = vec![
            puntaje(Dimension::Seguridad, 0.6),
            puntaje(Dimension::Diseno, 0.2),
            Puntaje {
                valor: 0.0,
                sin_veredicto: Some("sin sondas".into()),
                ..puntaje(Dimension::Funciona, 0.0)
            },
        ];
        c.aplicar("memora", &mut puntajes);
        // Diseño no está calibrada; Funciona está calibrada pero sin veredicto.
        let global = global_de(&puntajes).expect("hay una calibrada");
        assert!((global - 0.6).abs() < 1e-6, "{global}");
    }

    #[test]
    fn calibra_global_sin_calibradas_es_none() {
        let mut puntajes = vec![puntaje(Dimension::Seguridad, 0.9)];
        Calibraciones::default().aplicar("memora", &mut puntajes);
        assert_eq!(global_de(&puntajes), None);
    }

    #[test]
    fn calibra_n_dos_con_un_defecto_y_un_limpio_acertados_no_califica() {
        let base = veredicto_base();
        let fila = calibrar(
            Path::new(FIXTURE),
            &base,
            |limpia, _, base| if limpia { base.clone() } else { hundido(base) },
            2,
        );
        assert_eq!((fila.defectos, fila.limpios), (1, 1));
        assert_eq!(fila.exactitud_balanceada, Some(1.0));
        let tarea = fila.tarea.clone();
        let c = Calibraciones {
            v: VERSION,
            filas: vec![fila],
        };
        let mut puntajes = vec![puntaje(Dimension::Seguridad, 0.0)];
        c.aplicar(&tarea, &mut puntajes);
        assert!(
            !puntajes[0].calibrada,
            "1 defecto y 1 limpio no son muestra"
        );
        assert_eq!(puntajes[0].exactitud_balanceada, None);
    }

    #[test]
    fn calibra_cinco_defectos_y_cinco_limpios_acertados_si_califica() {
        let base = veredicto_base();
        let fila = calibrar(
            Path::new(FIXTURE),
            &base,
            |limpia, _, base| if limpia { base.clone() } else { hundido(base) },
            10,
        );
        assert_eq!((fila.defectos, fila.limpios), (5, 5));
        let tarea = fila.tarea.clone();
        let c = Calibraciones {
            v: VERSION,
            filas: vec![fila],
        };
        let mut puntajes = vec![puntaje(Dimension::Seguridad, 0.0)];
        c.aplicar(&tarea, &mut puntajes);
        assert!(puntajes[0].calibrada);
        assert_eq!(puntajes[0].exactitud_balanceada, Some(1.0));
    }

    #[test]
    fn calibra_cuatro_defectos_o_cuatro_limpios_no_alcanzan_el_minimo() {
        let mut pocos_defectos = fila("memora", Dimension::Seguridad, 1.0, Some(1.0));
        pocos_defectos.defectos = 4;
        let mut pocos_limpios = fila("memora", Dimension::Seguridad, 1.0, Some(1.0));
        pocos_limpios.limpios = 4;
        for f in [pocos_defectos, pocos_limpios] {
            let c = Calibraciones {
                v: VERSION,
                filas: vec![f],
            };
            let mut puntajes = vec![puntaje(Dimension::Seguridad, 0.8)];
            c.aplicar("memora", &mut puntajes);
            assert!(!puntajes[0].calibrada);
        }
    }

    #[test]
    fn calibra_ida_y_vuelta_del_archivo() {
        let c = Calibraciones {
            v: VERSION,
            filas: vec![fila("memora", Dimension::Seguridad, 1.5, Some(0.95))],
        };
        let texto = serde_json::to_string(&c).expect("serializa");
        let home = home_con(&texto);
        assert_eq!(Calibraciones::cargar(home.path()).expect("carga"), c);
    }

    #[test]
    fn calibra_esquema_lista_la_fila_con_sus_campos() {
        let esquema = crate::esquema::json("calibracion").expect("tipo conocido");
        assert!(esquema.contains("exactitud_balanceada"), "{esquema}");
        assert!(esquema.contains("items"), "{esquema}");
    }

    /// Elección de un juez por API con `modelo`.
    fn eleccion_juez(modelo: &str) -> Eleccion {
        Eleccion {
            via: crate::tipos::Via::Api,
            proveedor: Some(crate::tipos::Proveedor::Anthropic),
            modelo: Some(modelo.into()),
            razonamiento: true,
            motivo: "prueba".into(),
            rol: Some(crate::tipos::Rol::Juzga),
        }
    }

    const MODELO_JUEZ: &str = "claude-sonnet-5-5";

    #[test]
    fn calibra_peor_caso_sin_llamadas_es_cero() {
        let cfg = Config::default();
        let eleccion = eleccion_juez(MODELO_JUEZ);
        let jueces = JuecesCaso {
            diseno: Some(&eleccion),
            config: Some(&eleccion),
        };
        let partes = Partes {
            dimension: Some(Dimension::Seguridad),
            nativas: true,
            gitleaks: true,
            ..Partes::default()
        };
        assert_eq!(peor_caso_de_caso(&cfg, &jueces, &partes), 0.0);
    }

    #[test]
    fn calibra_peor_caso_de_juez_no_armado_es_cero() {
        let cfg = Config::default();
        let partes = Partes {
            dimension: Some(Dimension::Entrega),
            juez_config: true,
            juez_diseno: true,
            ..Partes::default()
        };
        let sin_jueces = JuecesCaso::default();
        assert_eq!(peor_caso_de_caso(&cfg, &sin_jueces, &partes), 0.0);
    }

    #[test]
    fn calibra_peor_caso_suma_el_juez_de_diseno_y_el_de_config_por_todos_los_intentos() {
        let cfg = Config::default();
        let eleccion = eleccion_juez(MODELO_JUEZ);
        let jueces = JuecesCaso {
            diseno: Some(&eleccion),
            config: Some(&eleccion),
        };
        let partes = Partes {
            dimension: Some(Dimension::Entrega),
            juez_config: true,
            juez_diseno: true,
            ..Partes::default()
        };
        // Sonnet cuesta 2 USD por millón de entrada y 10 por millón de salida. Cada juez: la entrada
        // estimada más todo `max_tokens`, por cada intento posible.
        let por_intento = (MAX_ENTRADA_JUEZ_BYTES as f64 * 2.0
            + f64::from(crate::proveedores::MAX_TOKENS_JUEZ) * 10.0)
            / 1_000_000.0;
        let esperado = 2.0 * f64::from(crate::proveedores::INTENTOS_MAX) * por_intento;
        let peor = peor_caso_de_caso(&cfg, &jueces, &partes);
        assert!((peor - esperado).abs() < 1e-9, "{peor} != {esperado}");
    }

    #[test]
    fn calibra_peor_caso_sin_precio_del_juez_es_infinito() {
        let mut cfg = Config::default();
        cfg.precios.remove(MODELO_JUEZ);
        let eleccion = eleccion_juez(MODELO_JUEZ);
        let jueces = JuecesCaso {
            diseno: Some(&eleccion),
            config: None,
        };
        let partes = Partes {
            dimension: Some(Dimension::Diseno),
            juez_diseno: true,
            ..Partes::default()
        };
        let peor = peor_caso_de_caso(&cfg, &jueces, &partes);
        assert!(peor.is_infinite() && peor > 0.0, "{peor}");
    }

    #[test]
    fn calibra_peor_caso_con_jueces_sin_precio_nombra_el_modelo_y_sale_con_2() {
        let mut cfg = Config::default();
        cfg.precios.remove(MODELO_JUEZ);
        let eleccion = eleccion_juez(MODELO_JUEZ);
        let jueces = JuecesCaso {
            diseno: Some(&eleccion),
            config: None,
        };
        let partes = Partes {
            dimension: Some(Dimension::Diseno),
            juez_diseno: true,
            ..Partes::default()
        };
        let error = peor_caso_con_jueces(&cfg, &jueces, &partes).expect_err("sin precio");
        assert_eq!(error.codigo_de_salida(), 2);
        let texto = error.to_string();
        assert!(texto.contains(MODELO_JUEZ), "{texto}");
        assert!(texto.contains("sin precio"), "{texto}");
    }

    #[test]
    fn calibra_modelos_sin_precio_solo_mira_los_jueces_que_las_partes_piden() {
        let mut cfg = Config::default();
        cfg.precios.remove(MODELO_JUEZ);
        let eleccion = eleccion_juez(MODELO_JUEZ);
        let jueces = JuecesCaso {
            diseno: Some(&eleccion),
            config: Some(&eleccion),
        };
        let partes = Partes {
            dimension: Some(Dimension::Seguridad),
            nativas: true,
            ..Partes::default()
        };
        assert!(modelos_sin_precio(&cfg, &jueces, &partes).is_empty());
    }

    #[test]
    fn calibra_aplicar_fila_incompleta_no_calibra_aunque_la_exactitud_llegue_a_0_9() {
        let mut incompleta = fila("memora", Dimension::Seguridad, 2.0, Some(0.95));
        incompleta.incompleta = true;
        let c = Calibraciones {
            v: VERSION,
            filas: vec![incompleta],
        };
        let mut puntajes = vec![puntaje(Dimension::Seguridad, 0.8)];
        c.aplicar("memora", &mut puntajes);
        assert!(!puntajes[0].calibrada);
        assert!(global_de(&puntajes).is_none());
    }

    #[test]
    fn calibra_fila_sin_el_campo_incompleta_en_el_json_lee_completa() {
        let texto = r#"{"v":1,"filas":[{"tarea":"memora","dimension":"seguridad","n":20,"semilla":42,"ts":1.0,"exactitud_balanceada":0.95}]}"#;
        let c: Calibraciones = serde_json::from_str(texto).expect("parsea");
        assert!(!c.filas[0].incompleta);
        assert!(
            c.de("memora", Dimension::Seguridad)
                .is_some_and(|f| !f.incompleta)
        );
    }

    #[test]
    fn calibra_casos_reserva_el_nombre_base_si_esta_libre() {
        let dir = tempfile::tempdir().expect("dir");
        let (ruta, _) = reservar_destino(dir.path(), "x").expect("reserva");
        assert_eq!(ruta.file_name().expect("nombre"), "x.casos.jsonl");
    }

    #[test]
    fn calibra_casos_reserva_el_sufijo_si_el_nombre_ya_existe() {
        let dir = tempfile::tempdir().expect("dir");
        fs::write(dir.path().join("x.casos.jsonl"), "").expect("escribe");
        fs::write(dir.path().join("x-2.casos.jsonl"), "").expect("escribe");
        let (ruta, _) = reservar_destino(dir.path(), "x").expect("reserva");
        assert_eq!(ruta.file_name().expect("nombre"), "x-3.casos.jsonl");
    }

    /// Motivo que emite la sonda de clippy cuando el código no compila.
    const NO_COMPILA: &str = "el código no compila: error[E0432]: unresolved import";

    /// Veredicto de `dimension` con `valor` y, si `sin` es `Some`, sin veredicto con ese motivo.
    fn veredicto_de(dimension: Dimension, valor: f32, sin: Option<&str>) -> Veredicto {
        Veredicto {
            puntajes: vec![Puntaje {
                sin_veredicto: sin.map(str::to_string),
                ..puntaje(dimension, valor)
            }],
            ..Veredicto::default()
        }
    }

    /// La copia de `base` con su dimensión sin veredicto por `motivo`.
    fn con_motivo(base: &Veredicto, motivo: &str) -> Veredicto {
        let mut v = base.clone();
        v.puntajes[0].sin_veredicto = Some(motivo.into());
        v
    }

    /// La copia de `base` con su dimensión en cero: un defecto que la dimensión detecta.
    fn bajada(base: &Veredicto) -> Veredicto {
        let mut v = base.clone();
        v.puntajes[0].valor = 0.0;
        v
    }

    fn faltante(motivo: &str) -> ItemFaltante {
        ItemFaltante {
            item: "pruebas.mutacion".into(),
            motivo: motivo.into(),
        }
    }

    #[test]
    fn rompe_la_compilacion_no_compila_nuevo_en_estructura_es_verdadero() {
        let base = veredicto_de(Dimension::Estructura, 1.0, None);
        let veredicto = con_motivo(&base, NO_COMPILA);
        assert!(rompe_la_compilacion(
            &base,
            &veredicto,
            Dimension::Estructura
        ));
    }

    #[test]
    fn rompe_la_compilacion_motivo_que_la_base_ya_tiene_es_falso() {
        let base = con_motivo(&veredicto_de(Dimension::Estructura, 1.0, None), NO_COMPILA);
        let veredicto = con_motivo(&base, NO_COMPILA);
        assert!(!rompe_la_compilacion(
            &base,
            &veredicto,
            Dimension::Estructura
        ));
    }

    #[test]
    fn rompe_la_compilacion_reconoce_fallan_sin_mutaciones_en_faltantes() {
        let base = veredicto_de(Dimension::Pruebas, 1.0, None);
        let mut veredicto = base.clone();
        veredicto.puntajes[0].faltantes =
            vec![faltante("los tests del proyecto fallan sin mutaciones")];
        assert!(rompe_la_compilacion(&base, &veredicto, Dimension::Pruebas));
    }

    #[test]
    fn rompe_la_compilacion_faltante_que_la_base_ya_tiene_es_falso() {
        let mut base = veredicto_de(Dimension::Pruebas, 1.0, None);
        base.puntajes[0].faltantes = vec![faltante("los tests del proyecto fallan sin mutaciones")];
        let veredicto = base.clone();
        assert!(!rompe_la_compilacion(&base, &veredicto, Dimension::Pruebas));
    }

    #[test]
    fn rompe_la_compilacion_completitud_y_funciona_son_falsas() {
        for dimension in [Dimension::Completitud, Dimension::Funciona] {
            let base = veredicto_de(dimension, 1.0, None);
            let veredicto = con_motivo(&base, NO_COMPILA);
            assert!(!rompe_la_compilacion(&base, &veredicto, dimension));
        }
    }

    #[test]
    fn rompe_la_compilacion_otro_motivo_es_falso() {
        let base = veredicto_de(Dimension::Estructura, 1.0, None);
        let veredicto = con_motivo(&base, "sin sonda");
        assert!(!rompe_la_compilacion(
            &base,
            &veredicto,
            Dimension::Estructura
        ));
    }

    #[test]
    fn calibra_inyeccion_invalida_se_reemplaza_por_el_siguiente_candidato() {
        let base = veredicto_de(Dimension::Estructura, 1.0, None);
        let primera = Mutex::new(true);
        let (_t, tarea) = tarea_prueba();
        let ev = falso(Path::new(FIXTURE), &base, |limpia, _, base| {
            if limpia {
                base.clone()
            } else if std::mem::replace(&mut *primera.lock().expect("lock"), false) {
                con_motivo(base, NO_COMPILA)
            } else {
                bajada(base)
            }
        });
        let fila = calibrar_con(
            &tarea,
            Dimension::Estructura,
            2,
            42,
            Path::new(FIXTURE),
            &ev,
        )
        .expect("calibra");
        assert_eq!(
            (
                fila.n,
                fila.defectos,
                fila.limpios,
                fila.no_aplicable,
                fila.reemplazos
            ),
            (2, 1, 1, 0, 1)
        );
        assert_eq!(fila.sensibilidad, Some(1.0));
        assert_eq!(fila.especificidad, Some(1.0));
    }

    #[test]
    fn calibra_inyeccion_invalida_agotada_es_no_aplicable_y_no_mide() {
        let base = veredicto_de(Dimension::Estructura, 1.0, None);
        let (_t, tarea) = tarea_prueba();
        let ev = falso(Path::new(FIXTURE), &base, |limpia, _, base| {
            if limpia {
                base.clone()
            } else {
                con_motivo(base, NO_COMPILA)
            }
        });
        let fila = calibrar_con(
            &tarea,
            Dimension::Estructura,
            2,
            42,
            Path::new(FIXTURE),
            &ev,
        )
        .expect("hay fila: queda el caso limpio");
        assert_eq!(
            (fila.n, fila.defectos, fila.limpios, fila.no_aplicable),
            (1, 0, 1, 1)
        );
        assert_eq!(fila.sensibilidad, None);
        // La inyección inicial más los MAX_REEMPLAZOS reemplazos, todos inválidos: cada intento suma 1.
        assert_eq!(fila.reemplazos, MAX_REEMPLAZOS + 1);
    }

    #[test]
    fn calibra_el_tope_se_revisa_antes_de_cada_reemplazo() {
        // La base no cuesta. La primera inyección cuesta 1 y es inválida. Con tope 1,5 y peor caso 1, el
        // reemplazo no entra: la calibración se detiene sin gastarlo.
        let base = veredicto_de(Dimension::Estructura, 1.0, None);
        let primera = Mutex::new(true);
        let ev = falso(Path::new(FIXTURE), &base, |limpia, _, base| {
            if limpia {
                base.clone()
            } else if std::mem::replace(&mut *primera.lock().expect("lock"), false) {
                con_motivo(base, NO_COMPILA)
            } else {
                bajada(base)
            }
        });
        let gastado = Mutex::new(0.0f64);
        let llamadas = AtomicUsize::new(0);
        let evaluar = |ruta: &Path, tipo: TipoCaso, _: Target| {
            llamadas.fetch_add(1, Ordering::SeqCst);
            if tipo != TipoCaso::Base {
                *gastado.lock().expect("gasto") += 1.0;
            }
            ev(ruta)
        };
        let gasto = |_: f64| -> Result<f64, ErrorOraculo> { Ok(*gastado.lock().expect("gasto")) };
        let tope = Tope {
            usd: 1.5,
            peor_caso: 1.0,
            inicio: 0.0,
            gasto_desde: &gasto,
            casos_con_reserva: &sin_reservas,
        };
        let (_t, tarea) = tarea_prueba();
        let medida = calibrar_medida_con_hilos(
            &tarea,
            Dimension::Estructura,
            1,
            42,
            Path::new(FIXTURE),
            &evaluar,
            Some(&tope),
            &Vec::new,
            &|| None,
            1,
        )
        .expect("calibra");
        assert!(
            medida.detencion.is_some(),
            "el tope detiene antes del reemplazo"
        );
        assert_eq!(
            llamadas.load(Ordering::SeqCst),
            2,
            "la base y la inyección inválida; el reemplazo no corre"
        );
        assert!(medida.casos.is_empty(), "ningún caso medido");
        assert!(medida.fila.is_none(), "sin casos no hay fila");
    }

    #[test]
    fn calibra_completitud_con_no_compila_mide_y_no_reemplaza() {
        let base = veredicto_de(Dimension::Completitud, 1.0, None);
        let (_t, tarea) = tarea_prueba();
        let ev = falso(Path::new(FIXTURE), &base, |limpia, _, base| {
            if limpia {
                base.clone()
            } else {
                con_motivo(base, NO_COMPILA)
            }
        });
        let fila = calibrar_con(
            &tarea,
            Dimension::Completitud,
            2,
            42,
            Path::new(FIXTURE),
            &ev,
        )
        .expect("calibra");
        assert_eq!((fila.defectos, fila.limpios, fila.reemplazos), (1, 1, 0));
        assert_eq!(
            fila.sensibilidad,
            Some(0.0),
            "sin veredicto cuenta como falso negativo"
        );
    }

    #[test]
    fn calibra_limpio_con_no_compila_no_se_reemplaza() {
        let base = veredicto_de(Dimension::Estructura, 1.0, None);
        let (_t, tarea) = tarea_prueba();
        let ev = falso(Path::new(FIXTURE), &base, |limpia, _, base| {
            if limpia {
                con_motivo(base, NO_COMPILA)
            } else {
                bajada(base)
            }
        });
        let fila = calibrar_con(
            &tarea,
            Dimension::Estructura,
            2,
            42,
            Path::new(FIXTURE),
            &ev,
        )
        .expect("calibra");
        assert_eq!((fila.defectos, fila.limpios, fila.reemplazos), (1, 1, 0));
        assert_eq!(
            fila.especificidad,
            Some(0.0),
            "un limpio sin veredicto es falso positivo"
        );
    }

    // ---- Tarea 48, parte 2, punto 3: el juez en paralelo, de a 4 ----

    /// Medidor de reservas para los tests que no las usan: ningún caso tiene reserva abierta.
    fn sin_reservas(_: f64) -> Result<HashSet<String>, ErrorOraculo> {
        Ok(HashSet::new())
    }

    /// Un caso que llama al juez: cuenta cuántos corren a la vez y espera 50 ms, como una llamada de red.
    fn llamada_de_juez(en_vuelo: &AtomicUsize, maximo: &AtomicUsize) {
        let dentro = en_vuelo.fetch_add(1, Ordering::SeqCst) + 1;
        maximo.fetch_max(dentro, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(50));
        en_vuelo.fetch_sub(1, Ordering::SeqCst);
    }

    #[test]
    fn calibra_con_ocho_casos_de_diseno_nunca_hay_mas_de_cuatro_jueces_a_la_vez() {
        let base = veredicto_de(Dimension::Diseno, 1.0, None);
        let (_t, tarea) = tarea_prueba();
        let falso_de_diseno = falso(Path::new(FIXTURE), &base, |limpia, _, base| {
            if limpia { base.clone() } else { bajada(base) }
        });
        let (en_vuelo, maximo) = (AtomicUsize::new(0), AtomicUsize::new(0));
        let juez = |ruta: &Path| {
            if ruta != Path::new(FIXTURE) {
                llamada_de_juez(&en_vuelo, &maximo);
            }
            falso_de_diseno(ruta)
        };
        calibrar_con(&tarea, Dimension::Diseno, 16, 42, Path::new(FIXTURE), &juez)
            .expect("calibra");
        let pico = maximo.load(Ordering::SeqCst);
        assert!(
            pico <= CASOS_EN_PARALELO,
            "más de 4 jueces a la vez: {pico}"
        );
        assert!(pico >= 2, "sin paralelismo real: {pico}");
    }

    #[test]
    fn calibra_el_tope_no_se_pasa_con_cuatro_llamadas_en_paralelo() {
        let base = veredicto_base();
        let (_t, tarea) = tarea_prueba();
        let falso_de_seguridad = falso(Path::new(FIXTURE), &base, |limpia, _, base| {
            if limpia { base.clone() } else { hundido(base) }
        });
        let peor = 1.0;
        let gastado = Mutex::new(0.0f64);
        let (en_vuelo, maximo) = (AtomicUsize::new(0), AtomicUsize::new(0));
        // Cada caso cobra un peor caso al terminar. La línea base no cobra.
        let evaluar = |ruta: &Path, tipo: TipoCaso, _: Target| {
            if tipo != TipoCaso::Base {
                llamada_de_juez(&en_vuelo, &maximo);
                *gastado.lock().expect("gasto") += peor;
            }
            falso_de_seguridad(ruta)
        };
        let gasto = |_: f64| -> Result<f64, ErrorOraculo> { Ok(*gastado.lock().expect("gasto")) };
        let tope = Tope {
            usd: 2.5 * peor,
            peor_caso: peor,
            inicio: 0.0,
            gasto_desde: &gasto,
            casos_con_reserva: &sin_reservas,
        };
        let medida = calibrar_medida_con_hilos(
            &tarea,
            Dimension::Seguridad,
            8,
            42,
            Path::new(FIXTURE),
            &evaluar,
            Some(&tope),
            &Vec::new,
            &|| None,
            CASOS_EN_PARALELO,
        )
        .expect("calibra");
        assert!(
            maximo.load(Ordering::SeqCst) <= 2,
            "más de 2 casos en vuelo"
        );
        let gastado = *gastado.lock().expect("gasto");
        assert!(gastado <= 2.5 * peor, "el gasto {gastado} pasó el tope");
        assert!(medida.detencion.is_some(), "el tope debe detener la medida");
        let fila = medida.fila.expect("hay casos terminados");
        assert!(fila.incompleta, "la fila queda incompleta");
        assert_eq!(fila.n as usize, medida.casos.len());
        assert_eq!(
            medida.casos.len(),
            2,
            "se detiene tras los 2 casos que entraron"
        );
    }

    #[test]
    fn calibra_gasto_comprometido_cuatro_reservas_de_cuatro_casos_cuentan_cuatro() {
        assert_eq!(gasto_comprometido(4.0, 1.0, 4, 4), 4.0);
    }

    #[test]
    fn calibra_gasto_comprometido_dos_reservas_de_cuatro_casos_suma_dos_peores() {
        assert_eq!(gasto_comprometido(2.0, 1.0, 4, 2), 2.0 + 2.0 * 1.0);
    }

    /// Cuatro casos en vuelo, cada uno con su reserva escrita: el coordinador no cuenta dos veces su
    /// peor caso. Cada reserva abierta cobra 1,0 y cada caso que termina cobra 1,0 como costo real.
    #[test]
    fn calibra_cuatro_hilos_los_casos_con_reserva_no_suman_dos_veces() {
        let base = veredicto_base();
        let falso_de_seguridad = falso(Path::new(FIXTURE), &base, |limpia, _, base| {
            if limpia { base.clone() } else { hundido(base) }
        });
        // Ids de los casos con reserva abierta. Un caso la abre al empezar y la cierra al terminar.
        let abiertas: Mutex<HashSet<String>> = Mutex::new(HashSet::new());
        let cerrados = AtomicUsize::new(0);
        let pico_gasto = Mutex::new(0.0f64);
        let (en_vuelo, maximo) = (AtomicUsize::new(0), AtomicUsize::new(0));
        let evaluar = |ruta: &Path, tipo: TipoCaso, _: Target| {
            if tipo == TipoCaso::Base {
                return falso_de_seguridad(ruta);
            }
            let id = crate::proveedores::caso_actual().expect("cada caso corre con su id");
            let dentro = en_vuelo.fetch_add(1, Ordering::SeqCst) + 1;
            maximo.fetch_max(dentro, Ordering::SeqCst);
            abiertas.lock().expect("lock").insert(id.clone());
            {
                let mut pico = pico_gasto.lock().expect("pico");
                let gasto = abiertas.lock().expect("lock").len() + cerrados.load(Ordering::SeqCst);
                *pico = pico.max(gasto as f64);
            }
            // Solo el primer lote espera a sus hermanos. Si el conteo viejo frena, espera el plazo.
            let llegada = Instant::now();
            while cerrados.load(Ordering::SeqCst) == 0
                && abiertas.lock().expect("lock").len() < CASOS_EN_PARALELO
                && llegada.elapsed() < Duration::from_secs(20)
            {
                std::thread::sleep(Duration::from_millis(1));
            }
            let resultado = falso_de_seguridad(ruta);
            abiertas.lock().expect("lock").remove(&id);
            cerrados.fetch_add(1, Ordering::SeqCst);
            en_vuelo.fetch_sub(1, Ordering::SeqCst);
            resultado
        };
        let gasto = |_: f64| -> Result<f64, ErrorOraculo> {
            // Da tiempo a que los casos recién lanzados escriban su reserva antes de la siguiente decisión.
            std::thread::sleep(Duration::from_millis(50));
            let abiertas = abiertas.lock().expect("lock").len();
            Ok((abiertas + cerrados.load(Ordering::SeqCst)) as f64)
        };
        let con_reserva = |_: f64| -> Result<HashSet<String>, ErrorOraculo> {
            Ok(abiertas.lock().expect("lock").clone())
        };
        let tope = Tope {
            usd: 5.5,
            peor_caso: 1.0,
            inicio: 0.0,
            gasto_desde: &gasto,
            casos_con_reserva: &con_reserva,
        };
        let (_t, tarea) = tarea_prueba();
        let medida = calibrar_medida_con_hilos(
            &tarea,
            Dimension::Seguridad,
            8,
            42,
            Path::new(FIXTURE),
            &evaluar,
            Some(&tope),
            &Vec::new,
            &|| None,
            CASOS_EN_PARALELO,
        )
        .expect("calibra");
        assert_eq!(
            maximo.load(Ordering::SeqCst),
            CASOS_EN_PARALELO,
            "con cuatro reservas escritas, llegan cuatro casos a la vez"
        );
        let pico = *pico_gasto.lock().expect("pico");
        assert!(pico <= 5.5, "el gasto {pico} pasó el tope");
        assert!(medida.detencion.is_some(), "el tope detiene la medida");
    }

    #[test]
    fn calibra_tope_nombra_el_archivo_de_casos_en_el_error() {
        // Base y casos cuestan 0,25. El peor caso es 0,5 y el tope 1,0: se miden dos casos y se detiene.
        let home = tempfile::tempdir().expect("home");
        let carpeta = home.path().join("oraculos").join("calibra-fixture");
        fs::create_dir_all(&carpeta).expect("carpeta");
        let repo = carpeta.join("repo-bueno");
        fs::create_dir_all(&repo).expect("repo");
        super::inyectar::pruebas::copiar(Path::new(FIXTURE), &repo);
        let _ = tarea_en(&carpeta, "\n[calibracion]\nrepo = \"repo-bueno\"\n");
        let gastado = Mutex::new(0.0f64);
        let peor = |_: &Partes| Ok::<f64, ErrorOraculo>(0.5);
        let gasto = |_: f64| -> Result<f64, ErrorOraculo> { Ok(*gastado.lock().expect("gasto")) };
        let presupuesto = Presupuesto {
            usd: 1.0,
            peor_caso: &peor,
            gasto_desde: &gasto,
        };
        let evaluador = |_: &Tarea, _: &Path, _: &OpcionesOraculo| {
            *gastado.lock().expect("gasto") += 0.25;
            Ok(veredicto_base())
        };
        let error = calibrar_tarea_con(
            home.path(),
            "calibra-fixture",
            "seguridad",
            8,
            42,
            None,
            Some(&presupuesto),
            &evaluador,
        )
        .expect_err("el tope detiene la calibración");
        let texto = error.to_string();
        let archivos: Vec<String> = fs::read_dir(home.path().join(CARPETA_CASOS))
            .expect("carpeta de casos")
            .map(|e| {
                e.expect("entrada")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .filter(|n| n.ends_with(".casos.jsonl"))
            .collect();
        assert_eq!(archivos.len(), 1, "un archivo de casos de la corrida");
        assert!(
            texto.contains(&archivos[0]),
            "el error nombra {}: {texto}",
            archivos[0]
        );
    }

    #[test]
    fn calibra_un_hilo_y_cuatro_dan_la_misma_fila_y_el_mismo_archivo_de_casos() {
        let base = veredicto_base();
        let falso_de_seguridad = falso(Path::new(FIXTURE), &base, |limpia, _, base| {
            if limpia { base.clone() } else { hundido(base) }
        });
        let (_t, tarea) = tarea_prueba();
        let medir = |hilos: usize| {
            calibrar_medida_con_hilos(
                &tarea,
                Dimension::Seguridad,
                20,
                42,
                Path::new(FIXTURE),
                &|ruta: &Path, _: TipoCaso, _: Target| falso_de_seguridad(ruta),
                None,
                &Vec::new,
                &|| None,
                hilos,
            )
            .expect("calibra")
        };
        let (uno, cuatro) = (medir(1), medir(CASOS_EN_PARALELO));
        let fila_uno = Fila {
            ts: 0.0,
            ..uno.fila.clone().expect("fila")
        };
        let fila_cuatro = Fila {
            ts: 0.0,
            ..cuatro.fila.clone().expect("fila")
        };
        assert_eq!(fila_uno, fila_cuatro);
        let guardados = |m: &Medida| m.casos.iter().map(CasoGuardado::de).collect::<Vec<_>>();
        assert_eq!(guardados(&uno), guardados(&cuatro));
        let home_uno = tempfile::tempdir().expect("home");
        let home_cuatro = tempfile::tempdir().expect("home");
        let ruta_uno = escribir_casos(
            home_uno.path(),
            &tarea.nombre,
            Dimension::Seguridad,
            &fila_uno,
            &uno.casos,
        )
        .expect("escribe")
        .expect("hay casos");
        let ruta_cuatro = escribir_casos(
            home_cuatro.path(),
            &tarea.nombre,
            Dimension::Seguridad,
            &fila_cuatro,
            &cuatro.casos,
        )
        .expect("escribe")
        .expect("hay casos");
        assert_eq!(
            fs::read_to_string(ruta_uno).expect("lee"),
            fs::read_to_string(ruta_cuatro).expect("lee")
        );
    }

    #[test]
    fn calibra_un_caso_con_error_corta_y_se_propaga_el_primero() {
        // Los primeros casos en vuelo fallan todos. Ninguno termina bien, así no se lanza nada nuevo.
        // El caso de índice menor es el más lento: el orden de llegada no decide el error.
        let base = veredicto_base();
        let falso_de_seguridad = falso(Path::new(FIXTURE), &base, |limpia, _, base| {
            if limpia { base.clone() } else { hundido(base) }
        });
        let llamadas = AtomicUsize::new(0);
        let evaluar = |ruta: &Path, tipo: TipoCaso, _: Target| {
            if tipo != TipoCaso::Base {
                llamadas.fetch_add(1, Ordering::SeqCst);
            }
            let indice = (0..CASOS_EN_PARALELO).find(|i| ruta.ends_with(format!("caso-{i}")));
            if let Some(i) = indice {
                std::thread::sleep(Duration::from_millis(40 * (CASOS_EN_PARALELO - i) as u64));
                return Err(ErrorOraculo::Infra(format!("falla del caso {i}")));
            }
            falso_de_seguridad(ruta)
        };
        let (_t, tarea) = tarea_prueba();
        let error = calibrar_medida_con_hilos(
            &tarea,
            Dimension::Seguridad,
            20,
            42,
            Path::new(FIXTURE),
            &evaluar,
            None,
            &Vec::new,
            &|| None,
            CASOS_EN_PARALELO,
        )
        .err()
        .expect("el error de un caso corta la calibración");
        assert!(
            matches!(&error, ErrorOraculo::Infra(m) if m == "falla del caso 0"),
            "{error:?}"
        );
        assert_eq!(
            llamadas.load(Ordering::SeqCst),
            CASOS_EN_PARALELO,
            "tras el error no se lanza ningún caso nuevo"
        );
    }

    /// Lo que el evaluador de `target_` recibe en cada llamada: el tipo de caso y el `target` que trae.
    type Registro = Mutex<Vec<(TipoCaso, Target)>>;

    /// Corre `calibrar_medida_con_hilos` con `n` casos sobre una copia del fixture y un hilo.
    fn medir_con_target(
        evaluador: &(dyn Fn(&Path, TipoCaso, Target) -> Result<Veredicto, ErrorOraculo> + Sync),
        leer_digest: &(dyn Fn() -> Option<String> + Sync),
        n: usize,
    ) -> Medida {
        let (_t, tarea) = tarea_prueba();
        let repo = copia_fixture();
        calibrar_medida_con_hilos(
            &tarea,
            Dimension::Seguridad,
            n,
            42,
            repo.path(),
            evaluador,
            None,
            &Vec::new,
            leer_digest,
            1,
        )
        .expect("calibra")
    }

    /// Target que el evaluador de prueba recibe para el primer caso limpio.
    fn target_del_limpio(registro: &Registro) -> Target {
        let registro = registro.lock().expect("lock");
        registro
            .iter()
            .find(|(tipo, _)| *tipo == TipoCaso::Limpio)
            .map(|(_, target)| target.clone())
            .expect("hay un caso limpio")
    }

    /// Evaluador de `target_`: la base deja `artefacto` en el `target` que recibe. Los casos devuelven `base`.
    fn evaluador_que_guarda_el_target<'a>(
        base: &'a Veredicto,
        registro: &'a Registro,
        aviso_en_limpio: Option<&'a str>,
    ) -> impl Fn(&Path, TipoCaso, Target) -> Result<Veredicto, ErrorOraculo> + Sync + 'a {
        move |_: &Path, tipo: TipoCaso, target: Target| {
            registro.lock().expect("lock").push((tipo, target.clone()));
            if let Target::Guardar(destino) = &target {
                fs::create_dir_all(destino).expect("carpeta del target de la base");
                fs::write(destino.join("artefacto"), "compilado").expect("artefacto");
            }
            // La carpeta de la base solo existe durante la medición: se mira aquí, dentro del caso.
            if let Target::Inicial(origen) = &target {
                assert!(
                    origen.join("artefacto").is_file(),
                    "el origen de Inicial tiene el artefacto de la base"
                );
            }
            let mut veredicto = base.clone();
            if let (TipoCaso::Limpio, Some(aviso)) = (tipo, aviso_en_limpio) {
                veredicto.avisos.push(aviso.to_string());
            }
            Ok(veredicto)
        }
    }

    #[test]
    fn target_la_base_guarda_y_el_limpio_recibe_el_target_y_reusa() {
        let base = veredicto_base();
        let registro = Registro::new(Vec::new());
        let ev = evaluador_que_guarda_el_target(&base, &registro, None);
        let medida = medir_con_target(&ev, &|| Some("sha:a".to_string()), 2);
        let primero = registro.lock().expect("lock")[0].1.clone();
        assert!(
            matches!(&primero, Target::Guardar(_)),
            "la base recibe Guardar: {primero:?}"
        );
        assert!(
            matches!(target_del_limpio(&registro), Target::Inicial(_)),
            "el limpio recibe Inicial"
        );
        let guardados: Vec<CasoGuardado> = medida.casos.iter().map(CasoGuardado::de).collect();
        let limpio = guardados.iter().find(|c| !c.es_defecto).expect("un limpio");
        assert_eq!(limpio.target_reusado, Some(true));
    }

    #[test]
    fn target_base_sin_target_hace_que_todos_los_casos_compilen_en_frio() {
        let base = veredicto_base();
        let registro = Registro::new(Vec::new());
        // La base no deja nada: el evaluador no crea la carpeta.
        let ev = |_: &Path, tipo: TipoCaso, target: Target| {
            registro.lock().expect("lock").push((tipo, target));
            Ok(base.clone())
        };
        let medida = medir_con_target(&ev, &|| None, 4);
        let primero = registro.lock().expect("lock")[0].1.clone();
        assert!(
            matches!(&primero, Target::Guardar(_)),
            "la base recibe Guardar: {primero:?}"
        );
        assert_eq!(target_del_limpio(&registro), Target::Ninguno);
        let guardados: Vec<CasoGuardado> = medida.casos.iter().map(CasoGuardado::de).collect();
        assert!(!guardados.is_empty());
        assert!(
            guardados.iter().all(|c| c.target_reusado == Some(false)),
            "{guardados:?}"
        );
    }

    #[test]
    fn target_digest_que_cambia_despues_de_la_base_hace_compilar_en_frio() {
        let base = veredicto_base();
        let registro = Registro::new(Vec::new());
        let ev = evaluador_que_guarda_el_target(&base, &registro, None);
        // La primera lectura es la de la base; cada caso lee otro digest.
        let lecturas = AtomicUsize::new(0);
        let leer = || {
            let n = lecturas.fetch_add(1, Ordering::SeqCst);
            Some(if n == 0 { "sha:a" } else { "sha:b" }.to_string())
        };
        let medida = medir_con_target(&ev, &leer, 2);
        let primero = registro.lock().expect("lock")[0].1.clone();
        assert!(
            matches!(&primero, Target::Guardar(_)),
            "la base recibe Guardar: {primero:?}"
        );
        assert_eq!(target_del_limpio(&registro), Target::Ninguno);
        let guardados: Vec<CasoGuardado> = medida.casos.iter().map(CasoGuardado::de).collect();
        assert!(
            guardados.iter().all(|c| c.target_reusado == Some(false)),
            "{guardados:?}"
        );
    }

    #[test]
    fn target_caso_que_cambia_cargo_lock_compila_en_frio() {
        let base = veredicto_base();
        let registro = Registro::new(Vec::new());
        let ev = evaluador_que_guarda_el_target(&base, &registro, None);
        let (_t, tarea) = tarea_prueba();
        let repo = copia_fixture();
        let lock = repo.path().join("Cargo.lock");
        fs::write(&lock, "version = 3\n").expect("lock de la base");
        // La copia del caso ya existe cuando se lee el digest: ahí cambia el lock del repo bueno, y la
        // copia queda con otro `Cargo.lock`. La primera lectura es la de la base y no toca nada.
        let lecturas = AtomicUsize::new(0);
        let leer = || {
            let n = lecturas.fetch_add(1, Ordering::SeqCst);
            if n > 0 {
                fs::write(&lock, format!("version = 3\n# cambio {n}\n")).expect("lock cambiado");
            }
            Some("sha:a".to_string())
        };
        let medida = calibrar_medida_con_hilos(
            &tarea,
            Dimension::Seguridad,
            2,
            42,
            repo.path(),
            &ev,
            None,
            &Vec::new,
            &leer,
            1,
        )
        .expect("calibra");
        assert_eq!(target_del_limpio(&registro), Target::Ninguno);
        let guardados: Vec<CasoGuardado> = medida.casos.iter().map(CasoGuardado::de).collect();
        let limpio = guardados.iter().find(|c| !c.es_defecto).expect("un limpio");
        assert_eq!(limpio.target_reusado, Some(false));
    }

    #[test]
    fn target_aviso_de_target_frio_en_el_caso_cuenta_como_frio() {
        let base = veredicto_base();
        let registro = Registro::new(Vec::new());
        let aviso = format!("{AVISO_TARGET_FRIO} no se pudo copiar");
        let ev = evaluador_que_guarda_el_target(&base, &registro, Some(&aviso));
        let medida = medir_con_target(&ev, &|| Some("sha:a".to_string()), 2);
        let primero = registro.lock().expect("lock")[0].1.clone();
        assert!(
            matches!(&primero, Target::Guardar(_)),
            "la base recibe Guardar: {primero:?}"
        );
        assert!(
            matches!(target_del_limpio(&registro), Target::Inicial(_)),
            "el limpio recibe Inicial aunque el aviso llegue después"
        );
        let guardados: Vec<CasoGuardado> = medida.casos.iter().map(CasoGuardado::de).collect();
        let limpio = guardados.iter().find(|c| !c.es_defecto).expect("un limpio");
        assert_eq!(limpio.target_reusado, Some(false));
    }

    #[test]
    fn target_renglon_sin_el_campo_se_lee_como_none_y_el_campo_hace_ida_y_vuelta() {
        let viejo = r#"{"caso":0,"es_defecto":false,"detectado":false}"#;
        let caso: CasoGuardado =
            serde_json::from_str(viejo).expect("renglón de antes de la Tarea 59");
        assert_eq!(caso.target_reusado, None);
        let nuevo = CasoGuardado {
            target_reusado: Some(true),
            ..caso
        };
        let texto = serde_json::to_string(&nuevo).expect("serializa");
        let ida: CasoGuardado = serde_json::from_str(&texto).expect("lee");
        assert_eq!(ida.target_reusado, Some(true));
    }
}
