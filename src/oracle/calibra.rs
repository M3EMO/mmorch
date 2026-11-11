//! Calibración del Oracle. La Tarea 12 escribe `$ROOT_HOME/calibracion.json` con la sensibilidad, la
//! especificidad y la exactitud balanceada de cada dimensión, medidas con defectos conocidos. La Tarea 13
//! lo lee: una dimensión queda `calibrada` si su exactitud balanceada llega a 0,9, y solo esas entran a
//! `global`. Además exige muestra mínima: 5 defectos aplicados y 5 casos limpios.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::{SystemTime, UNIX_EPOCH};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use serde_json::{Map, json};

use super::juez::{ExtractoGuardado, huella_del_prompt};
use super::memo::{Alcance, MemoSondas, ReusoSondas};
use super::mutacion::MutanteMuerto;
use super::rubrica::{Mide, Rubrica, cargar_embebidas};
use super::tarea::Tarea;
use super::{AVISO_TARGET_FRIO, ErrorOraculo, OpcionesOraculo, Partes, Target, partes_de};
use crate::borrado;
use crate::config::Config;
use crate::goal::calcular_hash;
use crate::observe;
use crate::proveedores::{LimiteCalibracion, con_caso, con_limite, rechazo_en_este_hilo};
use crate::tipos::{Dimension, Eleccion, Evento, Fuente, Hallazgo, Puntaje, Veredicto};

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
/// Porcentaje máximo de casos limpios sin veredicto para calibrar una dimensión. Más que esto, la medida no calibra.
const MAX_LIMPIOS_SIN_VEREDICTO_PCT: u32 = 20;

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
    /// Copias limpias sin veredicto de la dimensión. No entran a `especificidad`: no hay veredicto que medir.
    /// No son falsas alarmas. Una fila sin el campo vale 0.
    #[serde(default)]
    pub limpios_sin_veredicto: u32,
    /// Casos que no se pudieron aplicar al repo, por ejemplo sin Dockerfile. No entran a las métricas.
    #[serde(default)]
    pub no_aplicable: u32,
    /// Defectos cuyo ítem ya estaba en piso en la base. Una fila sin el campo vale 0.
    #[serde(default)]
    pub defectos_en_piso: u32,
    /// Omisiones de todos los casos: ítems estables de la base que el juez no puntuó. Sin el campo vale 0.
    #[serde(default)]
    pub omisiones: u32,
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
    /// Tests ruidosos de la calibración, en orden: los que cambiaron de estado entre la base y los limpios. Sus
    /// hallazgos no detectan (Tarea 63). Una fila sin el campo vale vacía.
    #[serde(default)]
    pub ruidosos: Vec<String>,
    /// `true` si el tope de gasto detuvo la calibración antes de medir todos sus casos (decisión 48). Una
    /// fila incompleta no calibra la dimensión, aunque su exactitud llegue a `UMBRAL`.
    #[serde(default)]
    pub incompleta: bool,
    /// Motivo por el que la dimensión no se midió: la tarea no la aplica. Una fila así no tiene casos ni
    /// métricas, y no calibra la dimensión. `None` en una medición real. Una dimensión inestable va a `inestable`.
    #[serde(default)]
    pub no_aplica: Option<String>,
    /// Motivo por el que la dimensión es inestable en la tarea: sus ítems del juez variaron entre juicios.
    /// Una fila sin el campo vale `None`.
    #[serde(default)]
    pub inestable: Option<String>,
    /// Inyecciones inválidas reemplazadas en la calibración, incluidas las del caso agotado.
    #[serde(default)]
    pub reemplazos: u32,
    /// Huella del juez con que se midió la fila: el prompt y las rúbricas de la dimensión (`huella_de`).
    /// Una fila sin el campo vale `None`. Con ítems del juez, `None` cuenta como vieja y no calibra.
    #[serde(default)]
    pub huella: Option<String>,
    /// Hash del `tarea.toml` con que se midió la fila. Otro `tarea.toml` deja la fila sin valor.
    /// Una fila sin el campo vale `None`, y sigue valiendo.
    #[serde(default)]
    pub tarea_hash: Option<String>,
    /// Ítems del juez que variaron entre los tres juicios de la base, en orden. No entran a la medición de la
    /// dimensión. Una fila sin el campo vale vacía.
    #[serde(default)]
    pub inestables: Vec<String>,
    /// Extractos que leyó el juez en cada juicio de la base, en orden. Una sola lista si la dimensión no repite
    /// la base. Una fila sin el campo vale vacía.
    #[serde(default)]
    pub extractos_base: Vec<Vec<ExtractoGuardado>>,
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
        let mut calibracion: Calibraciones = serde_json::from_str(&texto).map_err(|e| {
            ErrorOraculo::Infra(format!("calibración inválida en {}: {e}", ruta.display()))
        })?;
        if calibracion.v != VERSION {
            return Err(ErrorOraculo::Infra(format!(
                "calibración: la versión {} de {} no se entiende",
                calibracion.v,
                ruta.display()
            )));
        }
        for fila in &mut calibracion.filas {
            fila.normalizar_inestable();
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
    /// Sin fila, o con muestra insuficiente, el puntaje queda sin calibrar. `tarea_hash` es el hash actual
    /// del `tarea.toml` de `tarea` (`hash_de_tarea`).
    pub fn aplicar(&self, tarea: &str, tarea_hash: &str, puntajes: &mut [Puntaje]) {
        for puntaje in puntajes {
            // Una fila con muestra corta cuenta como si no existiera.
            let fila = self
                .de(tarea, puntaje.dimension)
                .filter(|f| muestra_suficiente(f));
            puntaje.exactitud_balanceada = fila.and_then(|f| f.exactitud_balanceada);
            // Una fila incompleta no calibra: el tope detuvo la medición antes de su último caso. Tampoco una con
            // más limpios sin veredicto que `MAX_LIMPIOS_SIN_VEREDICTO_PCT`: la dimensión no mide bien esos casos.
            let calibrable = fila.is_some_and(|f| {
                !f.incompleta
                    && f.limpios_sin_veredicto * 100 <= f.limpios * MAX_LIMPIOS_SIN_VEREDICTO_PCT
                    && tarea_vigente(f, tarea_hash)
                    && huella_vigente(f, puntaje.dimension)
            });
            puntaje.calibrada =
                calibrable && puntaje.exactitud_balanceada.is_some_and(|e| e >= UMBRAL);
        }
    }

    /// Ítems inestables de la fila más reciente de `dimension`. Vacía si la tarea no tiene fila, o si la fila
    /// tiene otra huella: sus ítems salen de otro juez (Tarea 83, punto 2).
    pub fn inestables_de(&self, tarea: &str, dimension: Dimension) -> &[String] {
        self.de(tarea, dimension)
            .filter(|fila| huella_vigente(fila, dimension))
            .map_or(&[], |fila| fila.inestables.as_slice())
    }

    /// Motivo por el que la fila más reciente de `dimension` no calibra: es de otro `tarea.toml` o de otro juez.
    /// Con las dos causas, gana la del `tarea.toml`. Da `None` si no hay fila con muestra suficiente, o si la fila vale.
    pub fn motivo_sin_calibrar(
        &self,
        tarea: &str,
        tarea_hash: &str,
        dimension: Dimension,
    ) -> Option<&'static str> {
        let fila = self
            .de(tarea, dimension)
            .filter(|f| muestra_suficiente(f))?;
        if !tarea_vigente(fila, tarea_hash) {
            return Some(MOTIVO_OTRA_TAREA);
        }
        (!huella_vigente(fila, dimension)).then_some(MOTIVO_OTRO_JUEZ)
    }
}

/// Motivo de un puntaje cuya fila es de otro `tarea.toml`: la calibración sale de otra versión de la tarea.
const MOTIVO_OTRA_TAREA: &str = "calibración con otro tarea.toml";

/// Motivo de un puntaje cuya fila tiene otra huella: la calibración sale de otro juez.
const MOTIVO_OTRO_JUEZ: &str = "calibración de otro juez";

/// Prefijo del motivo de una dimensión inestable. Las filas viejas lo guardaban en `no_aplica`.
const PREFIJO_INESTABLE: &str = "inestable en esta tarea";

impl Fila {
    /// Pasa el motivo de inestable de una fila vieja de `no_aplica` a `inestable`. Una fila nueva no cambia.
    fn normalizar_inestable(&mut self) {
        let es_viejo = self
            .no_aplica
            .as_deref()
            .is_some_and(|motivo| motivo.starts_with(PREFIJO_INESTABLE));
        if es_viejo && self.inestable.is_none() {
            self.inestable = self.no_aplica.take();
        }
    }
}

/// `true` si los ítems `inestables` de `dimension` dejan menos de la mitad de su peso estable (Tarea 83, punto 2).
pub fn dimension_inestable(
    rubricas: &[Rubrica],
    dimension: Dimension,
    inestables: &[String],
) -> bool {
    let de_la_dimension: Vec<Rubrica> = rubricas
        .iter()
        .filter(|r| Dimension::desde_clave(&r.id) == Some(dimension))
        .cloned()
        .collect();
    let excluidos: BTreeSet<String> = inestables.iter().cloned().collect();
    inestables::dimension_inestable(&inestables::pesos_de(&de_la_dimension), &excluidos)
}

/// Motivo de una dimensión inestable que nombra sus ítems.
pub fn motivo_inestable(inestables: &[String]) -> String {
    inestables::motivo(&inestables.iter().cloned().collect())
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

/// Calcula el hash del `tarea.toml` de `tarea`. Una fila lo guarda y solo vale para esa versión.
pub fn hash_de_tarea(tarea: &Tarea) -> Result<String, ErrorOraculo> {
    compartidos::hash_del_tarea_toml(&tarea.dir)
}

/// Huella de la calibración de `dimension`. Con ítems del juez, cuenta el prompt del juez y las rúbricas.
/// Sin ellos, cuentan solo las rúbricas. Cuenta también la versión de sus sondas (`version_de_sondas`).
/// Las filas con otra huella no calibran.
pub fn huella_de(dimension: Dimension) -> String {
    huella_con_version(
        dimension,
        &huella_del_prompt(),
        version_de_sondas(dimension),
    )
}

/// Versión base de las sondas. Una dimensión en esta versión conserva su huella de antes de existir la versión.
const VERSION_SONDAS_BASE: u32 = 1;

/// Versión de las sondas de `dimension`. Subirla cambia la huella de la dimensión: sus filas viejas no calibran.
fn version_de_sondas(dimension: Dimension) -> u32 {
    match dimension {
        // Arquitectura sube a 2 por el cambio de imports agrupados de sus sondas.
        Dimension::Arquitectura => 2,
        Dimension::Funciona
        | Dimension::Seguridad
        | Dimension::Eficiencia
        | Dimension::Estructura
        | Dimension::Diseno
        | Dimension::Completitud
        | Dimension::Entrega
        | Dimension::Pruebas => VERSION_SONDAS_BASE,
    }
}

/// Calcula la huella de `dimension` con `huella_prompt` y con la versión `version` de sus sondas.
/// Sin ítems del juez, ignora `huella_prompt`. La versión base no entra al texto: su huella no cambia.
fn huella_con_version(dimension: Dimension, huella_prompt: &str, version: u32) -> String {
    let juntas: String = rubricas_de(dimension).iter().map(Rubrica::hash).collect();
    let texto = if tiene_items_de_juez(dimension) {
        format!("{huella_prompt}\n{juntas}")
    } else {
        juntas
    };
    let texto = if version == VERSION_SONDAS_BASE {
        texto
    } else {
        format!("{texto}\nsondas v{version}")
    };
    calcular_hash(texto.as_bytes())
}

/// La fila sirve para la tarea según su `tarea_hash`. Sin hash, la fila es vieja y sirve. Con hash, debe ser el actual.
fn tarea_vigente(fila: &Fila, tarea_hash: &str) -> bool {
    fila.tarea_hash
        .as_deref()
        .is_none_or(|hash| hash == tarea_hash)
}

/// Rúbricas embebidas de `dimension`. Su id es la clave de la dimensión.
fn rubricas_de(dimension: Dimension) -> Vec<Rubrica> {
    cargar_embebidas()
        .expect("las rúbricas embebidas cargan")
        .into_iter()
        .filter(|r| Dimension::desde_clave(&r.id) == Some(dimension))
        .collect()
}

/// `dimension` tiene ítems que mide el juez (`juez` o `mixto`). Sin ellos, solo cuentan sus sondas.
fn tiene_items_de_juez(dimension: Dimension) -> bool {
    rubricas_de(dimension)
        .iter()
        .flat_map(|r| r.item.iter())
        .any(|i| i.mide != Mide::Sonda)
}

/// `true` si `veredicto` trae ítems del juez en `dimension`. Sin modelo de juez, el juez no corre y no los trae.
fn trae_juicio_del_juez(veredicto: &Veredicto, dimension: Dimension) -> bool {
    veredicto
        .puntajes
        .iter()
        .filter(|p| p.dimension == dimension)
        .flat_map(|p| p.items.iter())
        .any(|i| matches!(i.fuente, Fuente::Juez { .. }))
}

/// La fila sirve para `dimension` según su huella. Con huella, debe ser la actual. Sin huella, sirve solo si la
/// dimensión no tiene ítems del juez.
fn huella_vigente(fila: &Fila, dimension: Dimension) -> bool {
    match &fila.huella {
        Some(huella) => *huella == huella_de(dimension),
        None => !tiene_items_de_juez(dimension),
    }
}

pub mod inyectar;

use inyectar::{Defecto, Inyeccion};

mod compartidos;
mod estables;
mod inestables;
mod regla;
mod sonda;
mod target;

use compartidos::Compartidos;
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
        None,
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
    /// `true`: el limpio salió del caché de las otras dimensiones con juez, sin evaluarse. `false` en los demás.
    #[serde(default)]
    pub limpio_reusado: bool,
    /// Por qué `detectado` es true. Estructura, Arquitectura y Diseño: `hallazgo_sonda`, `item_sonda:<item>`,
    /// `cita_juez` o `item_baja_2:<item>`. Las demás: `hallazgo_nuevo:<cita>`, `test_malo_nuevo:<clave>`,
    /// `valor_menor` o `item_bajo:<item>`. `<cita>` es la del hallazgo nuevo que disparó. `<clave>` es
    /// `oculto:<id>` o `contrato:<id>` del test malo nuevo, y solo aparece en Completitud y Funciona.
    /// `None` si no detectó. Un caso sin veredicto de la dimensión no detecta: su motivo está en `sin_veredicto`.
    #[serde(default)]
    pub motivo: Option<String>,
    /// Nivel de cada ítem de la dimensión calibrada en la base. Vacío si la base no tiene puntaje.
    #[serde(default)]
    pub niveles_base: BTreeMap<String, u8>,
    /// Nivel de cada ítem de la dimensión calibrada en este caso. Vacío si no hay puntaje.
    #[serde(default)]
    pub niveles: BTreeMap<String, u8>,
    /// `true`: el ítem del defecto ya estaba en piso en la base. Un caso limpio o sin nivel base vale `false`.
    #[serde(default)]
    pub piso: bool,
    /// Ítems estables de la base que el juez no puntuó en este caso. Vacío sin veredicto de la dimensión.
    #[serde(default)]
    pub omitidos: Vec<String>,
    /// Valor de la dimensión calibrada en la base. `None` si la base no tiene puntaje de la dimensión.
    #[serde(default)]
    pub valor_base: Option<f32>,
    /// Valor de la dimensión calibrada en este caso. `None` si el caso no tiene puntaje de la dimensión.
    #[serde(default)]
    pub valor: Option<f32>,
    /// Los extractos que leyó el juez en este caso (archivo, primera línea, última línea y motivo).
    #[serde(default)]
    pub extractos: Vec<ExtractoGuardado>,
    /// Motivo por el que el caso no aplica. Solo en el renglón de un caso no aplicable.
    #[serde(default)]
    pub no_aplicable: Option<String>,
    /// Nombre del mutante muerto que eligió `AssertBorrado`. `None` en los demás defectos.
    #[serde(default)]
    pub mutante: Option<String>,
    /// Nombres completos de los tests que `AssertBorrado` apagó. Vacío en los demás defectos.
    #[serde(default)]
    pub tests_apagados: Vec<String>,
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
    /// Los casos con defecto que no se pudieron aplicar, en orden. No entran en `casos`.
    no_aplicables: Vec<NoAplicable>,
}

/// Un caso con defecto que no se pudo aplicar. No mide nada: su renglón en `.casos.jsonl` solo dice por qué.
#[derive(Debug, Clone, PartialEq)]
struct NoAplicable {
    indice: u32,
    defecto: Defecto,
    motivo: String,
}

/// Por qué el tope detuvo una calibración: el tope, el gasto medido, la cota del intento que no entró y los
/// casos medidos (decisión 48).
#[derive(Clone)]
struct Detencion {
    tope_usd: f64,
    gasto_usd: f64,
    peor_caso_usd: f64,
    casos: u32,
}

/// Tope de una medición: su límite de gasto. Los casos lo activan en su hilo, y el coordinador lo mira antes
/// de lanzar cada caso. El límite es `Sync`: sus casos corren en hilos distintos.
struct Tope {
    limite: Arc<LimiteCalibracion>,
}

/// Detención del límite de la medición, si ya recibió un rechazo. Sin tope o sin rechazo, `None`.
fn detencion_de_rechazo(tope: Option<&Tope>) -> Option<Detencion> {
    let limite = &tope?.limite;
    let rechazo = limite.rechazo()?;
    Some(Detencion {
        tope_usd: limite.usd,
        gasto_usd: rechazo.gasto_usd,
        peor_caso_usd: rechazo.cota_usd,
        casos: 0,
    })
}

/// Detención si el juez de este hilo recibió un rechazo del límite. Su caso no es un caso medido.
fn juez_rechazado(tope: Option<&Tope>) -> Option<Detencion> {
    if rechazo_en_este_hilo() {
        detencion_de_rechazo(tope)
    } else {
        None
    }
}

/// Corre `f` con el límite del tope activo en este hilo. Sin tope, corre `f` tal cual.
fn en_limite<R>(tope: Option<&Tope>, f: impl FnOnce() -> R) -> R {
    match tope {
        Some(tope) => con_limite(&tope.limite, f),
        None => f(),
    }
}

/// Medida que la detención corta: sin fila y sin casos.
fn medida_detenida(detencion: Detencion) -> Medida {
    Medida {
        fila: None,
        detencion: Some(detencion),
        casos: Vec::new(),
        no_aplicables: Vec::new(),
    }
}

/// Qué caso evalúa el evaluador de `calibrar_medida`. Un caso con defecto lleva la dimensión calibrada: de
/// ella sale qué sondas reusa de la base.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TipoCaso {
    Base,
    /// Juicio extra de la base (Tarea 83, punto 2). Reusa las sondas deterministas de la base y no guarda `target`.
    BaseRepetida,
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
        TipoCaso::BaseRepetida | TipoCaso::Limpio => Alcance::Deterministas,
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

/// La medición de `calibrar_con` y de `calibrar_tarea_con`. Con tope, cada intento de juez lo revisa en
/// `reservar`. Si el límite rechaza uno, para: la fila queda con los casos hechos y marcada `incompleta`.
/// Sin casos hechos, no hay fila. `muertos_de_la_base` se llama una vez, después de la
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
    compartidos: Option<&Compartidos>,
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
        compartidos,
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
    evaluador: &'a (dyn Fn(&Path, TipoCaso, Target) -> Result<Veredicto, ErrorOraculo> + Sync),
    /// Tope de la medición. Cada reemplazo lo revisa antes de correr.
    tope: Option<&'a Tope>,
    /// `target` de la base (Tarea 59): `Some` solo si la base dejó una carpeta. Los casos solo la leen.
    target_base: Option<PathBuf>,
    /// Digest de la imagen tras la base. Un caso que lo cambia compila en frío.
    digest_base: Option<String>,
    /// Digest de la imagen antes de cada evaluación de un caso.
    leer_digest: &'a (dyn Fn() -> Option<String> + Sync),
    /// Prefijo del id de cada caso: `calibra-{pid}-{inicio_ms}`. El caso `i` usa `{prefijo}-{i}`.
    prefijo: String,
    /// Veredictos de los limpios compartidos con las otras dimensiones con juez. `None` si la dimensión no comparte.
    compartidos: Option<&'a Compartidos>,
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
    /// Lo que impide medir el caso: `Some` solo si no hay caso y el tope no lo detuvo.
    no_aplicable: Option<NoAplicable>,
}

impl Medido {
    /// Un caso que el límite detuvo: no mide nada y la detención lo dice.
    fn detenido(reemplazos: u32, detencion: Detencion) -> Medido {
        Medido {
            reemplazos,
            caso: None,
            detencion: Some(detencion),
            no_aplicable: None,
        }
    }
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
    let veredicto = con_caso(&id, || {
        en_limite(r.tope, || (r.evaluador)(copia, tipo, target))
    })?;
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
        // Un limpio es la copia del repo bueno: la misma para Estructura, Arquitectura y Diseño. Si otra
        // dimensión con juez ya lo juzgó, se reusa su veredicto y no se copia ni se evalúa (Tarea 63, punto 6).
        let compartido = r.compartidos.filter(|_| regla::es_con_juez(r.dimension));
        if let Some(veredicto) = compartido.and_then(|c| c.leer(i as u32, r.dimension)) {
            let caso = Caso {
                limpio_reusado: true,
                ..Caso::nuevo(i as u32, None, veredicto)
            };
            return Ok(Medido {
                reemplazos: 0,
                caso: Some(caso),
                detencion: None,
                no_aplicable: None,
            });
        }
        let copia = r.tmp.join(format!("caso-{i}"));
        super::copiar_repo(r.repo_bueno, &copia).map_err(|e| super::infra("copia del repo", e))?;
        let (veredicto, reusado) = evaluar_copia(r, &copia, TipoCaso::Limpio, i)?;
        borrado::borrar_dir_dentro(r.tmp, &copia)
            .map_err(|e| super::infra("borrar una copia", e))?;
        // Un juez rechazado por el límite no mide: su veredicto no se guarda ni cuenta.
        if let Some(detencion) = juez_rechazado(r.tope) {
            return Ok(Medido::detenido(0, detencion));
        }
        if let Some(c) = compartido {
            c.guardar(i as u32, &veredicto)?;
        }
        let caso = Caso {
            target_reusado: reusado,
            ..Caso::nuevo(i as u32, None, veredicto)
        };
        return Ok(Medido {
            reemplazos: 0,
            caso: Some(caso),
            detencion: None,
            no_aplicable: None,
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
    // Un defecto que rompe la construcción, o que la suite no ejecuta, se reemplaza por el siguiente
    // candidato, hasta `MAX_REEMPLAZOS` veces. Cada intento inválido suma a `invalidos`, también el del caso agotado.
    let mut invalidos = 0u32;
    let mut medido = None;
    // Si el bucle termina sin medido, el motivo es el del último descarte: sin candidato, agotadas o sin ejecutar.
    let mut motivo_sin_caso = MOTIVO_AGOTADAS;
    let mut ultimo_descarte_sonda = false;
    for k in 0..=MAX_REEMPLAZOS {
        // Antes de cada reemplazo (no antes de la primera inyección) se mira el límite. Si un juez ya lo
        // rechazó, no se gasta más.
        if k > 0
            && let Some(detencion) = detencion_de_rechazo(r.tope)
        {
            return Ok(Medido::detenido(invalidos, detencion));
        }
        let semilla_k = semilla_caso.wrapping_add(u64::from(k) * 1_000_003);
        let copia = r.tmp.join(format!("caso-{i}"));
        super::copiar_repo(r.repo_bueno, &copia).map_err(|e| super::infra("copia del repo", e))?;
        let aplicado = inyectar::inyectar_con(defecto, &copia, &contexto, semilla_k)
            .map_err(|e| super::infra("inyectar un defecto", e))?;
        let Some(inyeccion) = aplicado else {
            borrado::borrar_dir_dentro(r.tmp, &copia)
                .map_err(|e| super::infra("borrar una copia", e))?;
            motivo_sin_caso = if ultimo_descarte_sonda {
                MOTIVO_SIN_EJECUTAR
            } else {
                motivo_sin_inyeccion(defecto, r.muertos.is_empty())
            };
            break;
        };
        // `AssertBorrado` acota la mutación al archivo de su mutante: solo ese archivo se vuelve a medir.
        let acotado = inyeccion.mutante_archivo.as_deref();
        let (veredicto, reusado) = super::mutacion::con_archivo_acotado(acotado, || {
            evaluar_copia(r, &copia, TipoCaso::Defecto(r.dimension), i)
        })?;
        borrado::borrar_dir_dentro(r.tmp, &copia)
            .map_err(|e| super::infra("borrar una copia", e))?;
        if let Some(detencion) = juez_rechazado(r.tope) {
            return Ok(Medido::detenido(invalidos, detencion));
        }
        if rompe_la_compilacion(r.base, &veredicto, r.dimension) {
            invalidos += 1;
            motivo_sin_caso = MOTIVO_AGOTADAS;
            ultimo_descarte_sonda = false;
            continue;
        }
        // Un comportamiento roto que no cambia nada frente a la base puede caer fuera de lo que la suite ejecuta.
        let sin_cambio = defecto == Defecto::ComportamientoRoto
            && motivo_de_deteccion(r.base, &veredicto, r.dimension, &BTreeSet::new()).is_none();
        if sin_cambio {
            let sin_efecto = la_suite_no_la_ejecuta(r, i, &inyeccion)?;
            if let Some(detencion) = juez_rechazado(r.tope) {
                return Ok(Medido::detenido(invalidos, detencion));
            }
            if sin_efecto {
                invalidos += 1;
                motivo_sin_caso = MOTIVO_SIN_EJECUTAR;
                ultimo_descarte_sonda = true;
                continue;
            }
        }
        medido = Some((inyeccion, veredicto, reusado));
        break;
    }
    let Some((inyeccion, veredicto, reusado)) = medido else {
        return Ok(Medido {
            reemplazos: invalidos,
            caso: None,
            detencion: None,
            no_aplicable: Some(NoAplicable {
                indice: i as u32,
                defecto,
                motivo: motivo_sin_caso.to_string(),
            }),
        });
    };
    let caso = Caso {
        reemplazos: invalidos,
        target_reusado: reusado,
        ..Caso::nuevo(i as u32, Some(inyeccion), veredicto)
    };
    Ok(Medido {
        reemplazos: invalidos,
        caso: Some(caso),
        detencion: None,
        no_aplicable: None,
    })
}

/// Si la suite no ejecuta la línea de `inyeccion` (Tarea 66, punto 4). Copia el repo bueno, inserta la sonda al
/// inicio de la fn que contiene la línea y evalúa esa copia. `true` si la sonda no cambia nada frente a la base.
/// `false` si la línea no está en una fn (no se puede probar) o si la sonda sí cambia el veredicto.
fn la_suite_no_la_ejecuta(
    r: &Ronda,
    i: usize,
    inyeccion: &Inyeccion,
) -> Result<bool, ErrorOraculo> {
    let copia = r.tmp.join(format!("caso-{i}-sonda"));
    super::copiar_repo(r.repo_bueno, &copia).map_err(|e| super::infra("copia del repo", e))?;
    let ruta = copia.join(&inyeccion.archivo);
    let fuente = fs::read_to_string(&ruta).map_err(|e| super::infra("leer una copia", e))?;
    let Some(sondada) = sonda::con_panic_en_funcion(&fuente, inyeccion.linea as usize) else {
        borrado::borrar_dir_dentro(r.tmp, &copia)
            .map_err(|e| super::infra("borrar una copia", e))?;
        return Ok(false);
    };
    fs::write(&ruta, sondada).map_err(|e| super::infra("escribir la sonda", e))?;
    let (veredicto, _) = evaluar_copia(r, &copia, TipoCaso::Defecto(r.dimension), i)?;
    borrado::borrar_dir_dentro(r.tmp, &copia).map_err(|e| super::infra("borrar una copia", e))?;
    Ok(motivo_de_deteccion(r.base, &veredicto, r.dimension, &BTreeSet::new()).is_none())
}

/// Como `calibrar_medida`, con `hilos` casos a la vez (1 corre en serie). Un coordinador lanza los casos en
/// orden. Antes de cada lanzamiento mira el límite: si un juez ya lo rechazó, para y los casos en vuelo terminan.
/// Antes de cada reemplazo de una inyección inválida, un caso hace lo mismo. Un juez rechazado no mide su caso.
/// Un error de caso deja de lanzar y espera a los que están en vuelo; devuelve el error del índice más bajo.
/// Los casos se ordenan por índice antes de armar la fila, así el resultado no depende del orden de término.
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
    compartidos: Option<&Compartidos>,
) -> Result<Medida, ErrorOraculo> {
    let tarea_hash = hash_de_tarea(tarea)?;
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
    // La carpeta temporal va antes de la base: la base deja ahí su `target` (Tarea 59).
    let tmp = tempfile::tempdir().map_err(|e| super::infra("carpeta temporal", e))?;
    let destino_target = tmp.path().join(NOMBRE_TARGET_BASE);
    // La base corre con el límite activo: su juez también cuenta. Una base rechazada no mide nada.
    let base = en_limite(tope, || {
        evaluador(
            repo_bueno,
            TipoCaso::Base,
            Target::Guardar(destino_target.clone()),
        )
    });
    let base = match (base, detencion_de_rechazo(tope)) {
        (_, Some(detencion)) => return Ok(medida_detenida(detencion)),
        (Err(error), None) => return Err(error),
        (Ok(veredicto), None) => veredicto,
    };
    // Una línea base sin veredicto no mide nada: los defectos no tienen contra qué compararse.
    let motivo_de_la_base = match puntaje_de(&base, dimension) {
        Some(puntaje) => puntaje.sin_veredicto.clone(),
        None => Some("la dimensión no figura en el veredicto de la base".into()),
    };
    if let Some(motivo) = motivo_de_la_base {
        return Err(ErrorOraculo::Infra(format!(
            "la línea base no tiene veredicto de {}: {motivo}",
            dimension.clave()
        )));
    }
    // Base repetida (Tarea 83, punto 2): las dimensiones con ítems del juez juzgan la base dos veces más. Las
    // sondas no se repiten: `BaseRepetida` reusa las deterministas de la base. Sin juez (sin modelo), la base
    // no trae ítems del juez y no hay juicios que repetir.
    let pesos = inestables::pesos_de(&rubricas_de(dimension));
    let juez_corre = tiene_items_de_juez(dimension) && trae_juicio_del_juez(&base, dimension);
    let mut juicios = vec![base];
    if juez_corre {
        for _ in 0..inestables::JUICIOS_EXTRA {
            let repetida = en_limite(tope, || {
                evaluador(repo_bueno, TipoCaso::BaseRepetida, Target::Ninguno)
            });
            match (repetida, detencion_de_rechazo(tope)) {
                (_, Some(detencion)) => return Ok(medida_detenida(detencion)),
                (Err(error), None) => return Err(error),
                (Ok(veredicto), None) => juicios.push(veredicto),
            }
        }
    }
    let extractos_base: Vec<Vec<ExtractoGuardado>> =
        juicios.iter().map(|j| j.extractos.clone()).collect();
    let refs: Vec<&Veredicto> = juicios.iter().collect();
    let inestables = inestables::inestables_de(&refs, dimension, &pesos.del_juez);
    // Una dimensión cuyos ítems estables pesan menos de la mitad no mide casos: ahorra el gasto del juez.
    if inestables::dimension_inestable(&pesos, &inestables) {
        let motivo = inestables::motivo(&inestables);
        let fila = Fila {
            inestable: Some(motivo.clone()),
            no_aplica: None,
            inestables: inestables.iter().cloned().collect(),
            extractos_base,
            ..fila_no_aplica(tarea, &tarea_hash, dimension, semilla, &motivo)
        };
        return Ok(Medida {
            fila: Some(fila),
            detencion: None,
            casos: Vec::new(),
            no_aplicables: Vec::new(),
        });
    }
    let base = inestables::base_efectiva(&juicios[0], &refs, dimension, &inestables, &pesos.todos);
    // Sin carpeta, la base no dejó `target`: ningún caso reusa nada.
    let target_base = destino_target.is_dir().then_some(destino_target);
    let digest_base = leer_digest();
    let muertos = if defectos.contains(&Defecto::AssertBorrado) {
        muertos_de_la_base()
    } else {
        Vec::new()
    };
    let items: BTreeSet<&'static str> = defectos.iter().map(|d| d.item()).collect();
    // Los ítems inestables no entran a las métricas por ítem.
    let items: Vec<&'static str> = items
        .into_iter()
        .filter(|i| !inestables.contains(*i))
        .collect();
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
        compartidos,
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
    std::thread::scope(|s| -> Result<(), ErrorOraculo> {
        let ronda = &ronda;
        loop {
            while error.is_none() && siguiente < n && en_vuelo < hilos && detencion.is_none() {
                // Con el límite rechazado, no se lanza otro caso. Los que están en vuelo terminan.
                if let Some(d) = detencion_de_rechazo(tope) {
                    detencion = Some(d);
                    break;
                }
                let i = siguiente;
                siguiente += 1;
                en_vuelo += 1;
                let envio = tx_abierto
                    .clone()
                    .expect("hay canal mientras se lanzan casos");
                s.spawn(move || {
                    // Un error de un caso cuyo juez fue rechazado no es un error: el caso no se mide.
                    let resultado =
                        medir_caso(ronda, i).or_else(|error| match juez_rechazado(ronda.tope) {
                            Some(detencion) => Ok(Medido::detenido(0, detencion)),
                            None => Err(error),
                        });
                    let _ = envio.send((i, resultado));
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
            // Un rechazo de cualquier caso detiene la medición, también si este caso terminó bien.
            if detencion.is_none() {
                detencion = detencion_de_rechazo(tope);
            }
        }
        Ok(())
    })?;
    if let Some((_, e)) = error {
        return Err(e);
    }
    let mut reemplazos = 0u32;
    let mut no_aplicables = Vec::new();
    let mut casos = Vec::with_capacity(resultados.len());
    for medido in resultados.into_values() {
        reemplazos += medido.reemplazos;
        match (medido.caso, medido.no_aplicable) {
            (Some(caso), _) => casos.push(caso),
            (None, Some(no)) => no_aplicables.push(no),
            // Un caso que el tope detuvo no es no aplicable: no se midió.
            (None, None) => {}
        }
    }
    // Los ítems inestables salen de los veredictos de los casos antes de detectar, puntuar y cubrir.
    for caso in &mut casos {
        inestables::sin_inestables(&mut caso.veredicto, dimension, &inestables, &pesos.todos);
    }
    // La detección se decide después de medir todos los casos, en orden de índice (Tarea 63, puntos 2 a 4).
    let ruidosos = finalizar_casos(&base, dimension, &items, &muertos, &mut casos);
    if let Some(d) = detencion.as_mut() {
        d.casos = casos.len() as u32;
    }
    let incompleta = detencion.is_some();
    let fila = if incompleta && casos.is_empty() {
        None
    } else {
        Some(Fila {
            reemplazos,
            ruidosos: ruidosos.into_iter().collect(),
            inestables: inestables.iter().cloned().collect(),
            extractos_base,
            tarea_hash: Some(tarea_hash.clone()),
            ..fila_de(
                tarea,
                dimension,
                semilla,
                &casos,
                &items,
                no_aplicables.len() as u32,
                incompleta,
            )
        })
    };
    Ok(Medida {
        fila,
        detencion,
        casos,
        no_aplicables,
    })
}

/// La fila de los casos medidos: sensibilidad, especificidad y exactitud por dimensión y por ítem.
/// Deja `tarea_hash` en `None`: quien la llama lo pone con el hash del `tarea.toml`.
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
    let limpios_sin_veredicto = casos
        .iter()
        .filter(|c| !c.es_defecto && c.sin_veredicto.is_some())
        .count() as u32;
    // La especificidad solo mira los limpios con veredicto. Un limpio sin veredicto no es un no detectado.
    let limpios_con_veredicto = casos
        .iter()
        .filter(|c| !c.es_defecto && c.sin_veredicto.is_none());
    let defectos_en_piso = casos.iter().filter(|c| c.es_defecto && c.piso).count() as u32;
    let omisiones: u32 = casos.iter().map(|c| c.omitidos.len() as u32).sum();
    let sensibilidad = proporcion(
        casos.iter().filter(|c| c.es_defecto && c.detectado).count() as u32,
        defectos_medidos,
    );
    let especificidad = proporcion(
        limpios_con_veredicto
            .clone()
            .filter(|c| !c.detectado)
            .count() as u32,
        limpios_con_veredicto.count() as u32,
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
            .filter(|c| !c.es_defecto && c.sin_veredicto.is_none())
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
        tarea_hash: None,
        tarea: tarea.nombre.clone(),
        dimension,
        n: casos.len() as u32,
        defectos: defectos_medidos,
        limpios,
        limpios_sin_veredicto,
        no_aplicable: no_aplicables,
        defectos_en_piso,
        omisiones,
        semilla,
        ts: ahora(),
        sensibilidad,
        especificidad,
        exactitud_balanceada: exactitud_de(sensibilidad, especificidad),
        items: items_medidos,
        incompleta,
        no_aplica: None,
        inestable: None,
        reemplazos: 0,
        ruidosos: Vec::new(),
        inestables: Vec::new(),
        huella: Some(huella_de(dimension)),
        extractos_base: Vec::new(),
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

/// Tope de gasto de una calibración (decisión 48). Cada intento de juez se reserva con su cota: si el gasto
/// desde el inicio más la cota pasa `usd`, el intento no se envía y la calibración se detiene.
pub struct Presupuesto {
    /// Tope en USD. Debe ser finito y mayor que 0.
    pub usd: f64,
}

/// Elecciones de los jueces que un caso puede llamar. `None`: el juez no se arma y no llama.
#[derive(Debug, Clone, Copy, Default)]
pub struct JuecesCaso<'a> {
    /// Juez de diseño (Tarea 10): llama si las partes lo piden.
    pub diseno: Option<&'a Eleccion>,
    /// Juez de la configuración de Cargo (decisión 30): llama si las partes lo piden.
    pub config: Option<&'a Eleccion>,
}

/// Gasto de la calibración con tope: el `Gate` que registra su detención.
const CORRIDA_CALIBRACION: &str = "oraculo-calibracion";
/// Fase del `Gate` de tope de una calibración.
const FASE_TOPE_CALIBRACION: &str = "calibracion-tope";

/// Elecciones de juez que un caso llama: las que `partes` pide y `jueces` arma.
fn elecciones_de_jueces<'a>(jueces: &JuecesCaso<'a>, partes: &Partes) -> Vec<&'a Eleccion> {
    let mut elecciones = Vec::new();
    if partes.juez_diseno
        && let Some(eleccion) = jueces.diseno
    {
        elecciones.push(eleccion);
    }
    if partes.juez_config
        && let Some(eleccion) = jueces.config
    {
        elecciones.push(eleccion);
    }
    elecciones
}

/// Modelos de los jueces que `partes` pide y que no tienen precio en `cfg`.
pub fn modelos_sin_precio(cfg: &Config, jueces: &JuecesCaso, partes: &Partes) -> Vec<String> {
    elecciones_de_jueces(jueces, partes)
        .into_iter()
        .filter(|eleccion| {
            eleccion
                .modelo
                .as_deref()
                .is_none_or(|modelo| !cfg.precios.contains_key(modelo))
        })
        .map(|eleccion| {
            eleccion
                .modelo
                .clone()
                .unwrap_or_else(|| "(sin modelo)".into())
        })
        .collect()
}

/// Un modelo de juez sin precio es error de configuración: sin precio no hay cota de cada intento, y el
/// límite no puede contar el gasto. La calibración con tope no arranca. Sale con 2.
fn validar_precios_de_jueces(
    cfg: &Config,
    jueces: &JuecesCaso,
    partes: &Partes,
) -> Result<(), ErrorOraculo> {
    let sin_precio = modelos_sin_precio(cfg, jueces, partes);
    if sin_precio.is_empty() {
        return Ok(());
    }
    Err(ErrorOraculo::Eleccion(format!(
        "sin precio en root.toml para {}: no se puede estimar el costo de un intento y la calibración con --tope-usd no arranca. Agregá [precios.<modelo>]",
        sin_precio.join(", ")
    )))
}

/// Valida los precios de los jueces que `partes` pide, con el `root.toml` de `home`. Sin jueces, no lee nada.
fn validar_precios_de_home(home: &Path, partes: &Partes) -> Result<(), ErrorOraculo> {
    if !partes.juez_diseno && !partes.juez_config {
        return Ok(());
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
    validar_precios_de_jueces(&cfg, &jueces, partes)
}

/// `oraculo calibrar` con tope opcional (decisión 48): como `calibrar_tarea`, con las dependencias
/// inyectables. Con `presupuesto`, cada intento de juez se reserva con su cota: si el gasto más la cota no
/// entra en el tope, el intento no se envía. Si no entra, registra un `Gate` `calibracion-tope`, agrega la fila de los casos hechos
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
    let tarea_hash = hash_de_tarea(&tarea)?;
    // Eficiencia sin `[eficiencia]`: la tarea no la aplica. Ni la base ni los casos corren, y la fila lleva
    // solo el motivo. Es el mismo criterio que la sonda usa para no medir.
    if dim == Dimension::Eficiencia && tarea.eficiencia.is_none() {
        let fila = fila_no_aplica(
            &tarea,
            &tarea_hash,
            dim,
            semilla,
            MOTIVO_NO_APLICA_EFICIENCIA,
        );
        agregar(home, &fila)?;
        return Ok(Calibrada { fila, casos: None });
    }
    let repo = repo_de(&tarea, repo_cli)?;
    // Entrega sin `[entrega]` y sin Dockerfile que elegir: no hay nada que instalar ni levantar. Mismo
    // `containerfile` que usa la evaluación real.
    let sin_entrega = dim == Dimension::Entrega && tarea.entrega.is_none();
    if sin_entrega
        && matches!(
            super::entrega::elegir_dockerfile(&repo, tarea.containerfile.as_deref()),
            super::entrega::Eleccion::SinDockerfile
        )
    {
        let fila = fila_no_aplica(&tarea, &tarea_hash, dim, semilla, MOTIVO_NO_APLICA_ENTREGA);
        agregar(home, &fila)?;
        return Ok(Calibrada { fila, casos: None });
    }
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
    // El límite se fija antes de la base: su gasto se cuenta desde `inicio`. Un modelo sin precio no deja
    // arrancar: sin precio no hay cota, y el límite no puede contar el gasto.
    let inicio = ahora();
    let tope = match presupuesto {
        Some(p) => {
            if !p.usd.is_finite() || p.usd <= 0.0 {
                return Err(ErrorOraculo::Uso(format!(
                    "el tope debe ser un número finito mayor que 0, no {}",
                    p.usd
                )));
            }
            validar_precios_de_home(home, &partes)?;
            Some(Tope {
                limite: LimiteCalibracion::nuevo(p.usd, inicio),
            })
        }
        None => None,
    };
    // Los limpios de Estructura, Arquitectura y Diseño son la misma copia: sus veredictos se comparten entre las tres.
    let compartidos = regla::es_con_juez(dim)
        .then(|| {
            let modelo = compartidos::modelo_del_juez_diseno(home);
            Compartidos::nuevo(home, &tarea, &repo, &modelo, semilla)
        })
        .transpose()?;
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
        compartidos.as_ref(),
    )?;
    // Los casos se escriben antes de `agregar`, también si el tope detuvo la calibración.
    let casos = match &medida.fila {
        Some(fila) => escribir_casos(
            home,
            &tarea.nombre,
            dim,
            fila,
            &medida.casos,
            &medida.no_aplicables,
        )?,
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
            "calibración de {}/{} detenida por el tope de {:.2} USD: gasto de API {:.2} USD y el intento siguiente cuesta como máximo {:.2} USD (cota). Casos medidos: {}. Archivo de casos: {}.",
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

/// Motivo de la fila de Entrega en una tarea sin `[entrega]` y con un repo sin Dockerfile.
const MOTIVO_NO_APLICA_ENTREGA: &str =
    "no aplica: la tarea no define [entrega] y el repo no tiene Dockerfile";

/// Fila de una dimensión que la tarea no aplica: sin casos ni métricas, con `motivo` en `no_aplica`.
fn fila_no_aplica(
    tarea: &Tarea,
    tarea_hash: &str,
    dimension: Dimension,
    semilla: u64,
    motivo: &str,
) -> Fila {
    Fila {
        tarea_hash: Some(tarea_hash.into()),
        tarea: tarea.nombre.clone(),
        dimension,
        n: 0,
        defectos: 0,
        limpios: 0,
        limpios_sin_veredicto: 0,
        no_aplicable: 0,
        defectos_en_piso: 0,
        omisiones: 0,
        semilla,
        ts: ahora(),
        sensibilidad: None,
        especificidad: None,
        exactitud_balanceada: None,
        items: BTreeMap::new(),
        incompleta: false,
        no_aplica: Some(motivo.into()),
        inestable: None,
        reemplazos: 0,
        ruidosos: Vec::new(),
        inestables: Vec::new(),
        huella: Some(huella_de(dimension)),
        extractos_base: Vec::new(),
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
    /// El veredicto de la copia medida. `finalizar_casos` lo compara con la base.
    veredicto: Veredicto,
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
    /// Si el limpio salió del caché `Compartidos`, sin evaluarse. Siempre `false` en un caso con defecto.
    limpio_reusado: bool,
    /// Por qué `detectado` es true. `None` si no detectó.
    motivo: Option<String>,
    /// Nivel de cada ítem de la dimensión calibrada en la base.
    niveles_base: BTreeMap<String, u8>,
    /// Nivel de cada ítem de la dimensión calibrada en este caso.
    niveles: BTreeMap<String, u8>,
    /// El ítem del defecto ya estaba en piso en la base. Ver `en_piso`.
    piso: bool,
    /// Ítems estables de la base que el juez dejó sin nivel en este caso. Ver `omitidos_de`.
    omitidos: Vec<String>,
    /// Valor de la dimensión calibrada en la base. `None` si la base no tiene puntaje de la dimensión.
    valor_base: Option<f32>,
    /// Valor de la dimensión calibrada en este caso. `None` si el caso no tiene puntaje de la dimensión.
    valor: Option<f32>,
    /// Extractos que leyó el juez en este caso. Vacío mientras el veredicto no los lleve.
    extractos: Vec<ExtractoGuardado>,
}

impl Caso {
    /// Un caso medido con su veredicto. Lo derivado (`detectado`, `motivo`, niveles, `items`) lo calcula
    /// `finalizar_casos` sobre todos los casos. Aquí queda en su valor neutro.
    fn nuevo(indice: u32, inyeccion: Option<Inyeccion>, veredicto: Veredicto) -> Caso {
        let extractos = veredicto.extractos.clone();
        Caso {
            indice,
            es_defecto: inyeccion.is_some(),
            item_defecto: inyeccion.as_ref().map(|i| i.defecto.item()),
            inyeccion,
            veredicto,
            detectado: false,
            motivo: None,
            sin_veredicto: None,
            items: BTreeMap::new(),
            niveles_base: BTreeMap::new(),
            niveles: BTreeMap::new(),
            valor_base: None,
            valor: None,
            extractos,
            reemplazos: 0,
            target_reusado: false,
            limpio_reusado: false,
            piso: false,
            omitidos: Vec::new(),
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
            limpio_reusado: caso.limpio_reusado,
            motivo: caso.motivo.clone(),
            niveles_base: caso.niveles_base.clone(),
            niveles: caso.niveles.clone(),
            valor_base: caso.valor_base,
            valor: caso.valor,
            extractos: caso.extractos.clone(),
            no_aplicable: None,
            piso: caso.piso,
            omitidos: caso.omitidos.clone(),
            mutante: caso.inyeccion.as_ref().and_then(|i| i.mutante.clone()),
            tests_apagados: caso
                .inyeccion
                .as_ref()
                .map(|i| i.tests_apagados.clone())
                .unwrap_or_default(),
        }
    }

    /// Renglón de un caso con defecto que no se pudo aplicar: no mide, solo dice por qué.
    fn de_no_aplicable(no: &NoAplicable) -> CasoGuardado {
        CasoGuardado {
            caso: no.indice,
            defecto: Some(nombre_defecto(no.defecto)),
            item: Some(no.defecto.item().to_string()),
            archivo: None,
            linea: None,
            es_defecto: true,
            detectado: false,
            sin_veredicto: None,
            reemplazos: 0,
            repetido_por_entorno: false,
            target_reusado: None,
            limpio_reusado: false,
            motivo: None,
            niveles_base: BTreeMap::new(),
            niveles: BTreeMap::new(),
            valor_base: None,
            valor: None,
            extractos: Vec::new(),
            no_aplicable: Some(no.motivo.clone()),
            piso: false,
            omitidos: Vec::new(),
            mutante: None,
            tests_apagados: Vec::new(),
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

/// Motivo de un caso no aplicable: la inyección no dio ningún candidato.
const MOTIVO_SIN_CANDIDATO: &str = "sin candidato para inyectar";

/// Motivo de un `AssertBorrado` no aplicable: la base no dejó mutantes muertos.
const MOTIVO_SIN_MUERTOS: &str = "la base no dio mutantes muertos";

/// Motivo de un `AssertBorrado` no aplicable: hay mutantes muertos, pero ninguno tiene un test fallido ubicable.
const MOTIVO_SIN_TEST_DE_MUERTO: &str = "ningún mutante muerto tiene tests fallidos ubicables";

/// Motivo de un caso sin inyección. `AssertBorrado` separa si la base no dio muertos de si ninguno tiene test.
/// Los demás defectos usan el motivo común.
fn motivo_sin_inyeccion(defecto: Defecto, sin_muertos: bool) -> &'static str {
    match (defecto, sin_muertos) {
        (Defecto::AssertBorrado, true) => MOTIVO_SIN_MUERTOS,
        (Defecto::AssertBorrado, false) => MOTIVO_SIN_TEST_DE_MUERTO,
        _ => MOTIVO_SIN_CANDIDATO,
    }
}

/// Motivo de un caso no aplicable: todos los reemplazos rompieron la construcción.
const MOTIVO_AGOTADAS: &str = "inyecciones inválidas agotadas";

/// Motivo de un caso no aplicable: la sonda vio que ningún candidato cae en una función que la suite ejecuta.
const MOTIVO_SIN_EJECUTAR: &str =
    "ninguna línea candidata está en una función que la suite ejecute";

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

/// Escribe los casos medidos y los no aplicables en `home/calibracion/<tarea>-<dimension>-<ts>.casos.jsonl`: un
/// renglón JSON por caso, en orden de índice. Escritura atómica: un temporal junto al destino y un `rename`. Sin
/// renglones, no escribe y da `None`.
fn escribir_casos(
    home: &Path,
    tarea: &str,
    dimension: Dimension,
    fila: &Fila,
    casos: &[Caso],
    no_aplicables: &[NoAplicable],
) -> Result<Option<PathBuf>, ErrorOraculo> {
    let mut renglones: Vec<(u32, CasoGuardado)> = casos
        .iter()
        .map(|c| (c.indice, CasoGuardado::de(c)))
        .chain(
            no_aplicables
                .iter()
                .map(|n| (n.indice, CasoGuardado::de_no_aplicable(n))),
        )
        .collect();
    if renglones.is_empty() {
        return Ok(None);
    }
    renglones.sort_by_key(|(indice, _)| *indice);
    let carpeta = home.join(CARPETA_CASOS);
    fs::create_dir_all(&carpeta).map_err(|e| super::infra("crear la carpeta de casos", e))?;
    let base = format!("{tarea}-{}-{}", dimension.clave(), fila.ts as u64);
    let (destino, _) = reservar_destino(&carpeta, &base)
        .map_err(|e| super::infra("reservar el archivo de casos", e))?;
    let mut texto = String::new();
    for (_, guardado) in &renglones {
        let renglon = serde_json::to_string(guardado)
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

/// Ítem y lugar con que `regla::detecta_con_juez` evalúa cada caso, en el orden de `casos`. Un defecto usa los
/// suyos. Un limpio `i` usa los del defecto `i-1`, que es el que le tocaba medir: el ítem sale de la lista de
/// defectos de la dimensión, así vale aunque ese defecto no esté en `casos`. El lugar sale de `casos`, y es
/// `None` si el defecto no se midió.
fn item_y_lugar_de(
    dimension: Dimension,
    casos: &[Caso],
) -> Vec<(Option<&'static str>, Option<regla::Lugar>)> {
    let defectos = inyectar::de_dimension(dimension);
    let lugar_de = |i: &Inyeccion| regla::Lugar {
        archivo: i.archivo.clone(),
        linea: i.linea,
    };
    let por_indice: BTreeMap<u32, &Caso> = casos.iter().map(|c| (c.indice, c)).collect();
    casos
        .iter()
        .map(|caso| {
            if caso.es_defecto {
                return (caso.item_defecto, caso.inyeccion.as_ref().map(lugar_de));
            }
            // Los limpios son impares: el defecto `i-1` es el de `defectos[((i-1)/2) % len]`.
            let anterior = caso.indice.saturating_sub(1);
            let item = (!defectos.is_empty())
                .then(|| defectos[((anterior / 2) as usize) % defectos.len()].item());
            let lugar = por_indice
                .get(&anterior)
                .and_then(|c| c.inyeccion.as_ref())
                .map(lugar_de);
            (item, lugar)
        })
        .collect()
}

/// Calcula la detección de cada caso, en orden de índice, contra la base. Estructura, Arquitectura y Diseño usan
/// `regla::detecta_con_juez`, con el ítem y el lugar de `item_y_lugar_de`. Pruebas con `AssertBorrado` usa
/// `motivo_de_mutante_vivo`, con `muertos` de la base. Las demás dimensiones usan `motivo_de_deteccion`.
/// También calcula los niveles, el `sin_veredicto` y los `items` de cada caso. Devuelve los tests ruidosos,
/// para que la fila los guarde.
fn finalizar_casos(
    base: &Veredicto,
    dimension: Dimension,
    items: &[&'static str],
    muertos: &[MutanteMuerto],
    casos: &mut [Caso],
) -> BTreeSet<String> {
    let contexto = item_y_lugar_de(dimension, casos);
    // Tests que variaron entre la base y los limpios: no cuentan para detectar (Tarea 63, punto 4).
    let ruidosos = {
        let limpios: Vec<&Veredicto> = casos
            .iter()
            .filter(|c| !c.es_defecto)
            .map(|c| &c.veredicto)
            .collect();
        estables::ruidosos(base, &limpios)
    };
    for (caso, (item, lugar)) in casos.iter_mut().zip(contexto) {
        let es_assert_borrado = caso
            .inyeccion
            .as_ref()
            .is_some_and(|i| i.defecto == Defecto::AssertBorrado);
        let motivo = if regla::es_con_juez(dimension) {
            motivo_con_juez(base, caso, dimension, item, lugar.as_ref())
        } else if dimension == Dimension::Pruebas && es_assert_borrado {
            motivo_de_mutante_vivo(base, &caso.veredicto, dimension, muertos, &ruidosos)
        } else {
            motivo_de_deteccion(base, &caso.veredicto, dimension, &ruidosos)
        };
        caso.detectado = motivo.is_some();
        caso.motivo = motivo;
        caso.sin_veredicto = motivo_sin_veredicto(&caso.veredicto, dimension);
        caso.niveles_base = niveles_de(puntaje_de(base, dimension));
        caso.niveles = niveles_de(puntaje_de(&caso.veredicto, dimension));
        caso.valor_base = puntaje_de(base, dimension).map(|p| p.valor);
        caso.valor = puntaje_de(&caso.veredicto, dimension).map(|p| p.valor);
        caso.piso = en_piso(&caso.niveles_base, caso.item_defecto, dimension);
        caso.omitidos = omitidos_de(caso);
        caso.items = items
            .iter()
            .map(|item| {
                (
                    *item,
                    detecta_item(
                        base,
                        &caso.veredicto,
                        dimension,
                        item,
                        caso.es_defecto,
                        &ruidosos,
                    ),
                )
            })
            .collect();
    }
    ruidosos
}

/// Motivo de un caso en Estructura, Arquitectura o Diseño. Sin veredicto de la dimensión, no detecta: el defecto
/// es un falso negativo y el limpio no es falsa alarma. `item` y `lugar` son los del caso, según
/// `item_y_lugar_de`.
fn motivo_con_juez(
    base: &Veredicto,
    caso: &Caso,
    dimension: Dimension,
    item: Option<&str>,
    lugar: Option<&regla::Lugar>,
) -> Option<String> {
    if puntaje_de(&caso.veredicto, dimension).is_none_or(|p| p.sin_veredicto.is_some()) {
        return None;
    }
    regla::detecta_con_juez(base, &caso.veredicto, dimension, item, lugar)
}

/// Por qué la dimensión detecta el caso, o `None` si no lo detecta. Sin veredicto de la dimensión, no detecta.
/// Si dos reglas disparan, gana la primera en este orden: `hallazgo_nuevo:<cita>`, `test_malo_nuevo:<clave>`,
/// `valor_menor`, `item_bajo:<item>` (el primer ítem de la base que baja). Las reglas de hallazgo, test malo,
/// valor e ítem pasan por `estables::permite`. `test_malo_nuevo` solo aplica en Completitud y Funciona.
/// Solo para las dimensiones sin juez: Estructura, Arquitectura y Diseño usan `regla::detecta_con_juez`.
fn motivo_de_deteccion(
    base: &Veredicto,
    veredicto: &Veredicto,
    dimension: Dimension,
    ruidosos: &BTreeSet<String>,
) -> Option<String> {
    let pv = puntaje_de(veredicto, dimension)?;
    if pv.sin_veredicto.is_some() {
        return None;
    }
    let pb = puntaje_de(base, dimension)?;
    let motivo = if let Some(hallazgo) =
        estables::hallazgo_nuevo(base, veredicto, dimension, ruidosos)
    {
        format!("hallazgo_nuevo:{}", hallazgo.cita)
    } else if let Some(clave) = estables::test_malo_nuevo(base, veredicto, dimension, ruidosos) {
        format!("test_malo_nuevo:{clave}")
    } else if pv.valor < pb.valor {
        "valor_menor".to_string()
    } else {
        let bajo = pb
            .items
            .iter()
            .find(|ib| nivel_de(pv, &ib.item).is_some_and(|nivel| nivel < ib.nivel))?;
        format!("item_bajo:{}", bajo.item)
    };
    estables::permite(base, veredicto, dimension, ruidosos).then_some(motivo)
}

/// Motivo de un caso con `AssertBorrado` en Pruebas: `mutante_vivo:<nombre>` si un mutante que la base mató
/// vive en el caso. Un hallazgo de Pruebas coincide con un muerto si tienen el mismo archivo y su `cita` es el
/// `nombre` del mutante. No compara `valor`, ids ni ítems. Gana el primer muerto de la base en su orden.
fn motivo_de_mutante_vivo(
    base: &Veredicto,
    veredicto: &Veredicto,
    dimension: Dimension,
    muertos: &[MutanteMuerto],
    ruidosos: &BTreeSet<String>,
) -> Option<String> {
    let pv = puntaje_de(veredicto, dimension)?;
    if pv.sin_veredicto.is_some() {
        return None;
    }
    let vivos: Vec<&Hallazgo> = veredicto
        .hallazgos
        .iter()
        .filter(|h| h.dimension == dimension)
        .collect();
    let muerto = muertos.iter().find(|m| {
        vivos
            .iter()
            .any(|h| ruta_sin_punto(&h.archivo) == ruta_sin_punto(&m.archivo) && h.cita == m.nombre)
    })?;
    estables::permite(base, veredicto, dimension, ruidosos)
        .then(|| format!("mutante_vivo:{}", muerto.nombre))
}

/// La ruta sin `./` inicial. Sirve para comparar el archivo de un hallazgo con el de un mutante.
fn ruta_sin_punto(ruta: &str) -> &str {
    let mut ruta = ruta;
    while let Some(resto) = ruta.strip_prefix("./") {
        ruta = resto;
    }
    ruta
}

/// Niveles de los ítems de un puntaje, por nombre. Vacío sin puntaje.
fn niveles_de(puntaje: Option<&Puntaje>) -> BTreeMap<String, u8> {
    puntaje.map_or_else(BTreeMap::new, |p| {
        p.items.iter().map(|i| (i.item.clone(), i.nivel)).collect()
    })
}

/// Niveles que un ítem debe bajar para contar en Estructura, Arquitectura y Diseño. Copia de
/// `regla::BAJA_ITEM_FUERTE`, que es privada allí: si cambia uno, cambia el otro.
const BAJA_ITEM_CON_JUEZ: u8 = 2;

/// Si el ítem `item` de la dimensión baja frente a la base. `None` si la base no mide el ítem: no entra a
/// la métrica. Sin veredicto de la dimensión, no baja (`Some(false)`), sea limpio o defecto.
/// En Estructura, Arquitectura y Diseño, baja solo si pierde `BAJA_ITEM_CON_JUEZ` niveles o más. Sin nivel
/// del ítem, no cuenta: ni como detección ni como falsa alarma. En las demás, baja con cualquier nivel menos.
/// Sin nivel del ítem, cuenta como falso positivo si el caso es limpio, y como no detectado si es defecto.
fn detecta_item(
    base: &Veredicto,
    veredicto: &Veredicto,
    dimension: Dimension,
    item: &str,
    es_defecto: bool,
    ruidosos: &BTreeSet<String>,
) -> Option<bool> {
    let nivel_base = nivel_de(puntaje_de(base, dimension)?, item)?;
    let Some(pv) = puntaje_de(veredicto, dimension) else {
        return Some(false);
    };
    if pv.sin_veredicto.is_some() {
        return Some(false);
    }
    let con_juez = regla::es_con_juez(dimension);
    Some(match nivel_de(pv, item) {
        Some(nivel) if con_juez => {
            nivel_base.saturating_sub(nivel) >= BAJA_ITEM_CON_JUEZ
                && estables::permite(base, veredicto, dimension, ruidosos)
        }
        Some(nivel) => {
            nivel < nivel_base && estables::permite(base, veredicto, dimension, ruidosos)
        }
        None => !con_juez && !es_defecto,
    })
}

/// Si el nivel base del ítem es menor que la baja que pide la dimensión.
/// Con juez la baja es `BAJA_ITEM_CON_JUEZ`; sin juez, 1. Sin nivel base del ítem, no está en piso.
fn en_piso(niveles_base: &BTreeMap<String, u8>, item: Option<&str>, dimension: Dimension) -> bool {
    let baja = if regla::es_con_juez(dimension) {
        BAJA_ITEM_CON_JUEZ
    } else {
        1
    };
    item.and_then(|i| niveles_base.get(i))
        .is_some_and(|nivel| *nivel < baja)
}

/// Ítems estables de la base que el caso no puntuó. Vacío si el caso no tiene veredicto de la dimensión.
fn omitidos_de(caso: &Caso) -> Vec<String> {
    if caso.sin_veredicto.is_some() {
        return Vec::new();
    }
    caso.niveles_base
        .keys()
        .filter(|item| !caso.niveles.contains_key(*item))
        .cloned()
        .collect()
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

/// La especificidad de la fila y cuántos limpios tienen veredicto. Sin limpios con veredicto dice "sin medir".
pub fn texto_especificidad(fila: &Fila) -> String {
    let con_veredicto = fila.limpios.saturating_sub(fila.limpios_sin_veredicto);
    let medida = fila
        .especificidad
        .map_or_else(|| "sin medir".to_string(), |v| format!("{v:.2}"));
    format!(
        "especificidad {medida} (limpios con veredicto {con_veredicto} de {})",
        fila.limpios
    )
}

/// Texto de los defectos en piso y de las omisiones de la fila.
/// Un defecto en piso no puede bajar su ítem, así que no mide la dimensión.
pub fn texto_piso(fila: &Fila) -> String {
    format!(
        "defectos en piso {}, omisiones {}",
        fila.defectos_en_piso, fila.omisiones
    )
}

/// `aciertos / total`. `None` sin casos: una métrica sin clase no es 0 ni 1.
fn proporcion(aciertos: u32, total: u32) -> Option<f32> {
    (total > 0).then(|| aciertos as f32 / total as f32)
}

/// Una fila calibra solo con muestra mínima de defectos y de limpios CON veredicto.
/// Los limpios sin veredicto no cuentan para el mínimo de limpios.
fn muestra_suficiente(fila: &Fila) -> bool {
    fila.defectos >= MIN_DEFECTOS_APLICABLES
        && fila.limpios.saturating_sub(fila.limpios_sin_veredicto) >= MIN_LIMPIOS
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
    use crate::config::Precio;
    use crate::oracle::mutacion::{ARCHIVO_OUTCOMES, construir_comando};
    use crate::oracle::perfiles::herramientas::{
        COMANDO_CARGO_AUDIT, COMANDO_GITLEAKS, EjecutorHerramienta, SalidaHerramienta,
        comando_clippy,
    };
    use crate::proveedores::RechazoLimite;
    use crate::tipos::{Fuente, Hallazgo, ItemFaltante, NivelItem};
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
    use std::time::{Duration, Instant};
    use tempfile::TempDir;

    fn fila(tarea: &str, dimension: Dimension, ts: f64, exactitud: Option<f32>) -> Fila {
        Fila {
            tarea_hash: None,
            tarea: tarea.into(),
            dimension,
            n: 20,
            defectos: 10,
            limpios: 10,
            limpios_sin_veredicto: 0,
            no_aplicable: 0,
            defectos_en_piso: 0,
            omisiones: 0,
            semilla: 42,
            ts,
            sensibilidad: Some(1.0),
            especificidad: Some(1.0),
            exactitud_balanceada: exactitud,
            items: BTreeMap::new(),
            incompleta: false,
            no_aplica: None,
            inestable: None,
            reemplazos: 0,
            ruidosos: Vec::new(),
            inestables: Vec::new(),
            huella: None,
            extractos_base: Vec::new(),
        }
    }

    /// Veredicto de Seguridad sano: valor 1, dos ítems en nivel 3 y sin hallazgos.
    fn veredicto_base() -> Veredicto {
        Veredicto {
            puntajes: vec![Puntaje {
                inestables: Vec::new(),
                motivo_sin_calibrar: None,
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
    fn calibra_limpio_sin_veredicto_no_es_falso_positivo() {
        let base = veredicto_base();
        let fila = calibrar(
            Path::new(FIXTURE),
            &base,
            |limpia, _, base| if limpia { sin(base) } else { hundido(base) },
            20,
        );
        assert_eq!(fila.sensibilidad, Some(1.0));
        assert_eq!(
            fila.especificidad, None,
            "sin limpio con veredicto no hay especificidad"
        );
        assert_eq!(fila.limpios_sin_veredicto, fila.limpios);
        assert_eq!(fila.exactitud_balanceada, None);
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
    fn calibra_limpio_sin_veredicto_no_cuenta_como_detectado() {
        let (_t, tarea) = tarea_prueba();
        let base = veredicto_base();
        let medida = medir(
            &tarea,
            Dimension::Seguridad,
            Path::new(FIXTURE),
            &base,
            |limpia, _, base| {
                if limpia {
                    con_motivo(base, "sin sonda")
                } else {
                    hundido(base)
                }
            },
            20,
        );
        let limpios: Vec<&Caso> = medida.casos.iter().filter(|c| !c.es_defecto).collect();
        assert!(!limpios.is_empty(), "hay limpios medidos");
        for caso in &limpios {
            assert!(
                !caso.detectado,
                "el limpio {} sin veredicto detecta",
                caso.indice
            );
            assert_eq!(
                caso.motivo, None,
                "el limpio {} sin veredicto tiene motivo",
                caso.indice
            );
        }
        let fila = medida.fila.expect("fila");
        assert_eq!(fila.limpios_sin_veredicto, fila.limpios);
        assert_eq!(fila.especificidad, None);
    }

    #[test]
    fn calibra_defecto_sin_veredicto_no_suma_a_limpios_sin_veredicto() {
        let (_t, tarea) = tarea_prueba();
        let base = veredicto_base();
        let medida = medir(
            &tarea,
            Dimension::Seguridad,
            Path::new(FIXTURE),
            &base,
            |limpia, _, base| {
                if limpia {
                    base.clone()
                } else {
                    con_motivo(base, "sin sonda")
                }
            },
            20,
        );
        let defectos: Vec<&Caso> = medida.casos.iter().filter(|c| c.es_defecto).collect();
        assert!(!defectos.is_empty(), "hay defectos medidos");
        assert!(
            defectos.iter().all(|c| !c.detectado),
            "un defecto sin veredicto es FN"
        );
        let fila = medida.fila.expect("fila");
        assert_eq!(fila.sensibilidad, Some(0.0));
        assert_eq!(fila.limpios_sin_veredicto, 0);
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
    fn calibra_un_limpio_sin_veredicto_deja_especificidad_none() {
        // Sin limpio con veredicto no hay especificidad. El texto lo dice como "sin medir".
        let base = veredicto_base();
        let fila = calibrar(
            Path::new(FIXTURE),
            &base,
            |limpia, _, base| if limpia { sin(base) } else { hundido(base) },
            2,
        );
        assert_eq!(
            (fila.defectos, fila.limpios, fila.limpios_sin_veredicto),
            (1, 1, 1)
        );
        assert_eq!(
            fila.especificidad, None,
            "un limpio sin veredicto no mide especificidad"
        );
        assert_eq!(fila.exactitud_balanceada, None);
        let texto = texto_especificidad(&fila);
        assert!(texto.contains("sin medir"), "texto: {texto}");
        assert!(texto.contains("0 de 1"), "texto: {texto}");
    }

    #[test]
    fn calibra_limpio_sin_veredicto_mas_limpio_que_no_detecta_da_especificidad_1() {
        // Dos limpios: uno sin veredicto y otro con veredicto que no detecta. Solo el segundo cuenta.
        let (_t, tarea) = tarea_prueba();
        let base = veredicto_base();
        let llamadas = std::sync::atomic::AtomicUsize::new(0);
        let medida = medir(
            &tarea,
            Dimension::Seguridad,
            Path::new(FIXTURE),
            &base,
            |limpia, _, base| {
                if !limpia {
                    return hundido(base);
                }
                // Con un hilo, la primera copia limpia es la de índice menor.
                if llamadas.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                    sin(base)
                } else {
                    base.clone()
                }
            },
            4,
        );
        let fila = medida.fila.expect("fila");
        assert_eq!(
            (fila.defectos, fila.limpios, fila.limpios_sin_veredicto),
            (2, 2, 1)
        );
        assert_eq!(fila.especificidad, Some(1.0), "1 de 1 con veredicto");
        assert_eq!(fila.exactitud_balanceada, Some(1.0));
        let texto = texto_especificidad(&fila);
        assert!(texto.contains("1 de 2"), "texto: {texto}");
    }

    #[test]
    fn calibra_limpio_sin_veredicto_mas_limpio_que_detecta_da_especificidad_0() {
        // Dos limpios: uno sin veredicto y otro con veredicto que detecta. El detectado es el único con veredicto.
        let (_t, tarea) = tarea_prueba();
        let base = veredicto_base();
        let llamadas = std::sync::atomic::AtomicUsize::new(0);
        let medida = medir(
            &tarea,
            Dimension::Seguridad,
            Path::new(FIXTURE),
            &base,
            |limpia, _, base| {
                if !limpia {
                    return hundido(base);
                }
                if llamadas.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                    sin(base)
                } else {
                    hundido(base)
                }
            },
            4,
        );
        let fila = medida.fila.expect("fila");
        assert_eq!(
            (fila.defectos, fila.limpios, fila.limpios_sin_veredicto),
            (2, 2, 1)
        );
        assert_eq!(fila.especificidad, Some(0.0), "0 de 1 con veredicto");
        assert_eq!(fila.exactitud_balanceada, Some(0.5));
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
                inestables: Vec::new(),
                motivo_sin_calibrar: None,
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
                inestables: Vec::new(),
                motivo_sin_calibrar: None,
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
                inestables: Vec::new(),
                motivo_sin_calibrar: None,
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

    /// Punto 3: el error de la base sin veredicto dice el motivo del puntaje. Sin puntaje, dice que no figura.
    #[test]
    fn calibra_base_sin_veredicto_dice_el_motivo() {
        let (_t, tarea) = tarea_prueba();
        let con_motivo_base = con_motivo(&veredicto_base(), "falta la sonda de seguridad");
        let ev = falso(Path::new(FIXTURE), &con_motivo_base, |_, _, base| {
            base.clone()
        });
        let error = calibrar_con(
            &tarea,
            Dimension::Seguridad,
            20,
            42,
            Path::new(FIXTURE),
            &ev,
        )
        .expect_err("sin base no hay medición");
        let texto = match error {
            ErrorOraculo::Infra(texto) => texto,
            otro => panic!("{otro:?}"),
        };
        assert!(texto.contains(": falta la sonda de seguridad"), "{texto}");

        let sin_puntaje = Veredicto::default();
        let ev = falso(Path::new(FIXTURE), &sin_puntaje, |_, _, base| base.clone());
        let error = calibrar_con(
            &tarea,
            Dimension::Seguridad,
            20,
            42,
            Path::new(FIXTURE),
            &ev,
        )
        .expect_err("sin base no hay medición");
        let texto = match error {
            ErrorOraculo::Infra(texto) => texto,
            otro => panic!("{otro:?}"),
        };
        assert!(
            texto.contains(": la dimensión no figura en el veredicto de la base"),
            "{texto}"
        );
    }

    #[test]
    fn calibra_funciona_tiene_defecto_y_n_cero_es_uso() {
        let base = veredicto_base();
        let (_t, tarea) = tarea_prueba();
        let ev = falso(Path::new(FIXTURE), &base, |_, _, base| base.clone());
        // Desde la Tarea 63 toda dimensión tiene al menos un defecto: Funciona tiene `ComportamientoRoto`.
        let funciona = calibrar_con(&tarea, Dimension::Funciona, 2, 42, Path::new(FIXTURE), &ev);
        assert!(
            !matches!(&funciona, Err(e) if e.codigo_de_salida() == 2),
            "Funciona ya no sale con 2 por falta de defectos"
        );
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

    /// Entrega sin `[entrega]` y con un repo sin Dockerfile: la fila es no aplicable. Ni la base ni los casos corren.
    #[test]
    fn calibra_entrega_sin_entrega_ni_dockerfile_es_no_aplica() {
        let home = tempfile::tempdir().expect("home");
        // El fixture trae un Dockerfile en la raíz: se quita de la copia del repo bueno.
        let carpeta = carpeta_con_repo_bueno(home.path());
        std::fs::remove_file(carpeta.join("repo-bueno").join("Dockerfile"))
            .expect("sin Dockerfile");
        let llamadas = AtomicUsize::new(0);
        let evaluador = |_: &Tarea, _: &Path, _: &OpcionesOraculo| {
            llamadas.fetch_add(1, Ordering::SeqCst);
            Ok(Veredicto::default())
        };
        let calibrada = calibrar_tarea_con(
            home.path(),
            "calibra-fixture",
            "entrega",
            2,
            42,
            None,
            None,
            &evaluador,
        )
        .expect("una entrega no aplicable sale con Ok");
        assert_eq!(llamadas.load(Ordering::SeqCst), 0, "el evaluador no corre");
        assert_eq!(
            calibrada.fila.no_aplica.as_deref(),
            Some("no aplica: la tarea no define [entrega] y el repo no tiene Dockerfile")
        );
        assert!(calibrada.casos.is_none(), "sin casos");
        let guardadas = Calibraciones::cargar(home.path()).expect("carga");
        assert_eq!(guardadas.filas.len(), 1, "la fila se guarda");
        assert_eq!(guardadas.filas[0], calibrada.fila);
    }

    /// Entrega sin `[entrega]` pero con Dockerfile en la raíz del repo: no es no aplicable. El evaluador corre.
    #[test]
    fn calibra_entrega_sin_entrega_con_dockerfile_mide_como_siempre() {
        let home = tempfile::tempdir().expect("home");
        let carpeta = carpeta_con_repo_bueno(home.path());
        std::fs::write(
            carpeta.join("repo-bueno").join("Dockerfile"),
            "FROM scratch\n",
        )
        .expect("dockerfile");
        let llamadas = AtomicUsize::new(0);
        let evaluador = |_: &Tarea, _: &Path, _: &OpcionesOraculo| {
            llamadas.fetch_add(1, Ordering::SeqCst);
            Ok(Veredicto::default())
        };
        let resultado = calibrar_tarea_con(
            home.path(),
            "calibra-fixture",
            "entrega",
            2,
            42,
            None,
            None,
            &evaluador,
        );
        assert!(
            llamadas.load(Ordering::SeqCst) > 0,
            "con Dockerfile el evaluador corre"
        );
        if let Ok(calibrada) = &resultado {
            assert_eq!(
                calibrada.fila.no_aplica, None,
                "con Dockerfile no hay no aplica"
            );
        }
    }

    /// `tests/protocolo.rs` de la calibración de Pruebas: un test de `leer_numero` con dos asserts.
    const TEST_DE_LEER_NUMERO: &str = "use calibra_fixture::protocolo::leer_numero;\n\n#[test]\nfn leer_numero_lee_el_numero() {\n    assert!(leer_numero(\"42\").is_ok());\n    assert_eq!(leer_numero(\"7\").unwrap(), 7);\n}\n";
    /// Log del mutante de `leer_numero` en las salidas de prueba, y el `tests-fallidos.txt` que lo cita.
    const LOG_DE_PROTOCOLO: &str = "log/src__protocolo.rs_line_2_col_1.log";
    const TESTS_FALLIDOS_DE_PROTOCOLO: &str =
        "log/src__protocolo.rs_line_2_col_1.log:test leer_numero_lee_el_numero ... FAILED\n";

    /// Veredicto de Pruebas sano: valor 1 y el ítem de mutación en nivel 3.
    fn veredicto_de_pruebas() -> Veredicto {
        Veredicto {
            puntajes: vec![Puntaje {
                inestables: Vec::new(),
                motivo_sin_calibrar: None,
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
        const OUTCOMES: &str = r#"{"total_mutants":1,"outcomes":[{"scenario":"Baseline","summary":"Success"},{"scenario":{"Mutant":{"name":"src/protocolo.rs:2:1: replace leer_numero -> Result<u32, ParseIntError> with Ok(0)","file":"src/protocolo.rs","span":{"start":{"line":2,"column":1}}}},"summary":"CaughtMutant","log_path":"log/src__protocolo.rs_line_2_col_1.log"}]}"#;
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
                salida.archivos.insert(
                    crate::oracle::mutacion::ARCHIVO_TESTS_FALLIDOS.into(),
                    TESTS_FALLIDOS_DE_PROTOCOLO.into(),
                );
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
        assert_eq!(
            con_defecto.linea,
            Some(4),
            "la línea del #[ignore] del test"
        );
    }

    /// Calibra Pruebas de `calibra-fixture` con `outcomes` como salida grabada de la base. Devuelve los casos guardados.
    fn casos_de_pruebas_con(outcomes: &'static str) -> Vec<CasoGuardado> {
        use crate::oracle::mutacion::{ARCHIVO_OUTCOMES, construir_comando};
        use crate::oracle::perfiles::herramientas::SalidaHerramienta;
        let home = tempfile::tempdir().expect("home");
        let carpeta = home.path().join("oraculos").join("calibra-fixture");
        std::fs::create_dir_all(&carpeta).expect("carpeta");
        let repo = carpeta.join("repo-bueno");
        std::fs::create_dir_all(&repo).expect("repo");
        super::inyectar::pruebas::copiar(Path::new(FIXTURE), &repo);
        std::fs::write(repo.join("tests/protocolo.rs"), TEST_DE_LEER_NUMERO).expect("test");
        let _ = tarea_en(&carpeta, "\n[calibracion]\nrepo = \"repo-bueno\"\n");
        let evaluador = |tarea: &Tarea, _: &Path, o: &OpcionesOraculo| {
            if let Some(memo) = o.memo.as_deref() {
                let mut salida = SalidaHerramienta::default();
                salida
                    .archivos
                    .insert(ARCHIVO_OUTCOMES.into(), outcomes.into());
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
        let ruta = calibrada.casos.expect("hay casos guardados");
        let texto = fs::read_to_string(&ruta).expect("lee los casos");
        texto
            .lines()
            .map(|l| serde_json::from_str(l).expect("un caso por línea"))
            .collect()
    }

    /// Sin mutantes muertos en la base, el AssertBorrado no aplicable dice que la base no dio muertos.
    #[test]
    fn calibra_pruebas_assert_borrado_sin_muertos_dice_que_la_base_no_dio_muertos() {
        const SIN_MUERTOS: &str = r#"{"total_mutants":1,"outcomes":[{"scenario":"Baseline","summary":"Success"},{"scenario":{"Mutant":{"name":"src/protocolo.rs:2:1: replace leer_numero -> Result<u32, ParseIntError> with Ok(0)","file":"src/protocolo.rs","span":{"start":{"line":2,"column":1}}}},"summary":"MissedMutant"}]}"#;
        let casos = casos_de_pruebas_con(SIN_MUERTOS);
        let motivos: Vec<&str> = casos
            .iter()
            .filter(|c| c.defecto.as_deref() == Some("assert_borrado"))
            .filter_map(|c| c.no_aplicable.as_deref())
            .collect();
        assert!(
            !motivos.is_empty(),
            "hay un AssertBorrado no aplicable: {casos:?}"
        );
        assert!(
            motivos
                .iter()
                .all(|m| *m == "la base no dio mutantes muertos"),
            "motivo de la base sin muertos: {motivos:?}"
        );
    }

    /// Con un mutante muerto que ningún test llama, el AssertBorrado no aplicable dice que ningún mutante muerto tiene tests fallidos ubicables.
    #[test]
    fn calibra_pruebas_assert_borrado_sin_tests_ubicables_de_muerto_dice_que_ninguno_tiene_tests_ubicables()
     {
        const MUERTO_SIN_TEST: &str = r#"{"total_mutants":1,"outcomes":[{"scenario":"Baseline","summary":"Success"},{"scenario":{"Mutant":{"name":"src/protocolo.rs:9:1: replace escribir_numero -> Result<u32, ParseIntError> with Ok(0)","file":"src/protocolo.rs","span":{"start":{"line":9,"column":1}}}},"summary":"CaughtMutant"}]}"#;
        let casos = casos_de_pruebas_con(MUERTO_SIN_TEST);
        let motivos: Vec<&str> = casos
            .iter()
            .filter(|c| c.defecto.as_deref() == Some("assert_borrado"))
            .filter_map(|c| c.no_aplicable.as_deref())
            .collect();
        assert!(
            !motivos.is_empty(),
            "hay un AssertBorrado no aplicable: {casos:?}"
        );
        assert!(
            motivos
                .iter()
                .all(|m| *m == "ningún mutante muerto tiene tests fallidos ubicables"),
            "motivo con muertos sin test: {motivos:?}"
        );
    }

    /// Un caso con AssertBorrado corre el evaluador acotado al archivo de su mutante. La base y el caso limpio no.
    #[test]
    fn calibra_pruebas_el_caso_con_assert_borrado_corre_acotado_al_mutante() {
        use crate::oracle::mutacion::{ARCHIVO_OUTCOMES, archivo_acotado, construir_comando};
        use crate::oracle::perfiles::herramientas::SalidaHerramienta;
        const OUTCOMES: &str = r#"{"total_mutants":1,"outcomes":[{"scenario":"Baseline","summary":"Success"},{"scenario":{"Mutant":{"name":"src/protocolo.rs:2:1: replace leer_numero -> Result<u32, ParseIntError> with Ok(0)","file":"src/protocolo.rs","span":{"start":{"line":2,"column":1}}}},"summary":"CaughtMutant","log_path":"log/src__protocolo.rs_line_2_col_1.log"}]}"#;
        let home = tempfile::tempdir().expect("home");
        let carpeta = home.path().join("oraculos").join("calibra-fixture");
        std::fs::create_dir_all(&carpeta).expect("carpeta");
        let repo = carpeta.join("repo-bueno");
        std::fs::create_dir_all(&repo).expect("repo");
        super::inyectar::pruebas::copiar(Path::new(FIXTURE), &repo);
        std::fs::write(repo.join("tests/protocolo.rs"), TEST_DE_LEER_NUMERO).expect("test");
        let _ = tarea_en(&carpeta, "\n[calibracion]\nrepo = \"repo-bueno\"\n");
        let acotados = Mutex::new(Vec::<Option<String>>::new());
        let evaluador = |tarea: &Tarea, _: &Path, o: &OpcionesOraculo| {
            acotados.lock().expect("lock").push(archivo_acotado());
            if let Some(memo) = o.memo.as_deref() {
                let mut salida = SalidaHerramienta::default();
                salida
                    .archivos
                    .insert(ARCHIVO_OUTCOMES.into(), OUTCOMES.into());
                salida.archivos.insert(
                    crate::oracle::mutacion::ARCHIVO_TESTS_FALLIDOS.into(),
                    TESTS_FALLIDOS_DE_PROTOCOLO.into(),
                );
                memo.anotar(&construir_comando(tarea), &salida);
            }
            Ok(veredicto_de_pruebas())
        };
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
        let vistos = acotados.lock().expect("lock");
        let con_archivo: Vec<&String> = vistos.iter().flatten().collect();
        assert!(
            !con_archivo.is_empty(),
            "el caso con AssertBorrado va acotado: {vistos:?}"
        );
        assert!(
            con_archivo.iter().all(|a| a.as_str() == "src/protocolo.rs"),
            "solo el mutante elegido: {vistos:?}"
        );
        let sin_archivo = vistos.iter().filter(|a| a.is_none()).count();
        assert!(
            sin_archivo >= 2,
            "la base y el caso limpio no van acotados: {vistos:?}"
        );
    }

    /// Punto 3: el renglón del caso AssertBorrado en `.casos.jsonl` guarda el mutante y los tests que apagó.
    #[test]
    fn calibra_pruebas_el_renglon_de_assert_borrado_guarda_mutante_y_tests() {
        use crate::oracle::mutacion::{ARCHIVO_OUTCOMES, construir_comando};
        use crate::oracle::perfiles::herramientas::SalidaHerramienta;
        const OUTCOMES: &str = r#"{"total_mutants":1,"outcomes":[{"scenario":"Baseline","summary":"Success"},{"scenario":{"Mutant":{"name":"src/protocolo.rs:2:1: replace leer_numero -> Result<u32, ParseIntError> with Ok(0)","file":"src/protocolo.rs","span":{"start":{"line":2,"column":1}}}},"summary":"CaughtMutant","log_path":"log/src__protocolo.rs_line_2_col_1.log"}]}"#;
        let home = tempfile::tempdir().expect("home");
        let carpeta = home.path().join("oraculos").join("calibra-fixture");
        std::fs::create_dir_all(&carpeta).expect("carpeta");
        let repo = carpeta.join("repo-bueno");
        std::fs::create_dir_all(&repo).expect("repo");
        super::inyectar::pruebas::copiar(Path::new(FIXTURE), &repo);
        std::fs::write(repo.join("tests/protocolo.rs"), TEST_DE_LEER_NUMERO).expect("test");
        let _ = tarea_en(&carpeta, "\n[calibracion]\nrepo = \"repo-bueno\"\n");
        let evaluador = |tarea: &Tarea, _: &Path, o: &OpcionesOraculo| {
            if let Some(memo) = o.memo.as_deref() {
                let mut salida = SalidaHerramienta::default();
                salida
                    .archivos
                    .insert(ARCHIVO_OUTCOMES.into(), OUTCOMES.into());
                salida.archivos.insert(
                    crate::oracle::mutacion::ARCHIVO_TESTS_FALLIDOS.into(),
                    TESTS_FALLIDOS_DE_PROTOCOLO.into(),
                );
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
        let ruta = calibrada.casos.expect("hay casos guardados");
        let texto = fs::read_to_string(&ruta).expect("lee los casos");
        let renglones: Vec<serde_json::Value> = texto
            .lines()
            .map(|l| serde_json::from_str(l).expect("un caso por línea"))
            .collect();
        let con_mutante: Vec<&serde_json::Value> = renglones
            .iter()
            .filter(|r| r["defecto"] == "assert_borrado" && r["mutante"].is_string())
            .collect();
        assert!(
            !con_mutante.is_empty(),
            "un renglón de AssertBorrado trae mutante: {renglones:?}"
        );
        let renglon = con_mutante[0];
        assert!(
            renglon["mutante"]
                .as_str()
                .is_some_and(|m| m.contains("leer_numero")),
            "el mutante elegido: {renglon}"
        );
        let apagados: Vec<&str> = renglon["tests_apagados"]
            .as_array()
            .expect("tests_apagados es una lista")
            .iter()
            .filter_map(|t| t.as_str())
            .collect();
        assert!(
            apagados
                .iter()
                .any(|t| t.contains("leer_numero_lee_el_numero")),
            "los tests apagados: {apagados:?}"
        );
        let limpios: Vec<&serde_json::Value> = renglones
            .iter()
            .filter(|r| r["es_defecto"] == false)
            .collect();
        assert!(
            limpios.iter().all(|r| r["mutante"].is_null()),
            "un limpio no trae mutante: {limpios:?}"
        );
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
                salida.archivos.insert(
                    crate::oracle::mutacion::ARCHIVO_TESTS_FALLIDOS.into(),
                    TESTS_FALLIDOS_DE_PROTOCOLO.into(),
                );
            }
            Ok(salida)
        }

        fn tiene_rustsec(&self) -> bool {
            false
        }
    }

    const OUTCOMES_CON_MUERTO: &str = r#"{"total_mutants":1,"outcomes":[{"scenario":"Baseline","summary":"Success"},{"scenario":{"Mutant":{"name":"src/protocolo.rs:2:1: replace leer_numero -> Result<u32, ParseIntError> with Ok(0)","file":"src/protocolo.rs","span":{"start":{"line":2,"column":1}}}},"summary":"CaughtMutant","log_path":"log/src__protocolo.rs_line_2_col_1.log"}]}"#;

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
        let evaluador = evaluador_con_registro(&llamadas, || diseno_con(&[(ITEM_MODULARIDAD, 2)]));
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
            medidos + 1 + inestables::JUICIOS_EXTRA,
            "la base (tres juicios) y cada caso llaman al evaluador"
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
        let limpios: Vec<&Llamada> = llamadas[1 + inestables::JUICIOS_EXTRA..]
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
        let evaluador =
            evaluador_con_juez_y_tests(&llamadas, || diseno_con(&[(ITEM_MODULARIDAD, 2)]), true);
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
            medidos + 1 + inestables::JUICIOS_EXTRA,
            "la base (tres juicios) y cada caso llaman al evaluador"
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
        let limpios: Vec<&Llamada> = llamadas[1 + inestables::JUICIOS_EXTRA..]
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

    /// Veredicto con Arquitectura y Diseño, las dos con juez, con el mismo `valor`.
    fn veredicto_arq_dis(valor: f32) -> Veredicto {
        Veredicto {
            puntajes: vec![
                puntaje(Dimension::Arquitectura, valor),
                puntaje(Dimension::Diseno, valor),
            ],
            ..Veredicto::default()
        }
    }

    /// Evaluador de prueba. Cuenta las llamadas de los casos limpios (reuso `Deterministas`) y devuelve
    /// `veredicto(es_limpio)`.
    fn evaluador_con_limpios<'a>(
        limpios: &'a AtomicUsize,
        veredicto: fn(bool) -> Veredicto,
    ) -> impl Fn(&Tarea, &Path, &OpcionesOraculo) -> Result<Veredicto, ErrorOraculo> + 'a {
        move |_: &Tarea, ruta: &Path, o: &OpcionesOraculo| {
            let es_limpio = o
                .reuso
                .as_ref()
                .is_some_and(|r| r.alcance == Alcance::Deterministas)
                && es_copia_de_caso(ruta);
            if es_limpio {
                limpios.fetch_add(1, Ordering::SeqCst);
            }
            Ok(veredicto(es_limpio))
        }
    }

    /// Si `ruta` es una copia de un caso. Las repeticiones de la base evalúan el repo bueno, no una copia.
    fn es_copia_de_caso(ruta: &Path) -> bool {
        ruta.file_name()
            .is_some_and(|nombre| nombre.to_string_lossy().starts_with("caso-"))
    }

    /// Archivos de `home/calibracion/limpios/`. Falla si la carpeta no existe.
    fn archivos_limpios(home: &Path) -> Vec<PathBuf> {
        let carpeta = home.join(CARPETA_CASOS).join("limpios");
        let mut archivos: Vec<PathBuf> = fs::read_dir(&carpeta)
            .expect("hay carpeta de limpios")
            .map(|e| e.expect("entrada").path())
            .collect();
        archivos.sort();
        assert!(!archivos.is_empty(), "hay limpios guardados");
        archivos
    }

    /// Veredictos guardados de los limpios. Falla si alguno no parsea.
    fn veredictos_limpios(home: &Path) -> Vec<Veredicto> {
        archivos_limpios(home)
            .iter()
            .map(|ruta| {
                let texto = fs::read_to_string(ruta).expect("lee un limpio");
                serde_json::from_str(&texto).expect("veredicto de un limpio")
            })
            .collect()
    }

    /// Por caso limpio de la calibración: índice, `detectado` y `motivo`, en orden.
    fn limpios_medidos(calibrada: &Calibrada) -> Vec<(u32, bool, Option<String>)> {
        let ruta = calibrada.casos.as_ref().expect("hay casos guardados");
        fs::read_to_string(ruta)
            .expect("lee los casos")
            .lines()
            .map(|l| serde_json::from_str::<CasoGuardado>(l).expect("caso"))
            .filter(|c| !c.es_defecto)
            .map(|c| (c.caso, c.detectado, c.motivo))
            .collect()
    }

    /// Un limpio de Diseño sin veredicto del juez. Con esto, el limpio no detecta: su motivo queda en `sin_veredicto`.
    fn veredicto_limpio_sin_juez(es_limpio: bool) -> Veredicto {
        let mut v = veredicto_arq_dis(1.0);
        if es_limpio {
            v.puntajes[1].sin_veredicto = Some("juez caído".into());
        }
        v
    }

    #[test]
    fn calibra_diseno_reusa_los_limpios_que_arquitectura_ya_juzgo() {
        let home = tempfile::tempdir().expect("home");
        carpeta_con_repo_bueno(home.path());
        let arquitectura = AtomicUsize::new(0);
        calibrar_tarea_con(
            home.path(),
            "calibra-fixture",
            "arquitectura",
            4,
            42,
            None,
            None,
            &evaluador_con_limpios(&arquitectura, |_| veredicto_arq_dis(1.0)),
        )
        .expect("calibra arquitectura");
        assert_eq!(
            arquitectura.load(Ordering::SeqCst),
            2,
            "dos limpios con n 4"
        );
        let diseno = AtomicUsize::new(0);
        calibrar_tarea_con(
            home.path(),
            "calibra-fixture",
            "diseno",
            4,
            42,
            None,
            None,
            &evaluador_con_limpios(&diseno, |_| veredicto_arq_dis(1.0)),
        )
        .expect("calibra diseño");
        assert_eq!(
            diseno.load(Ordering::SeqCst),
            0,
            "los limpios de Diseño salen del cache de Arquitectura"
        );
    }

    /// Renglones de `.casos.jsonl` de una calibración. Falla si no hay casos guardados.
    fn renglones_guardados(calibrada: &Calibrada) -> Vec<CasoGuardado> {
        let ruta = calibrada.casos.as_ref().expect("hay casos guardados");
        fs::read_to_string(ruta)
            .expect("lee los casos")
            .lines()
            .map(|l| serde_json::from_str::<CasoGuardado>(l).expect("caso"))
            .collect()
    }

    /// Punto 2: cada caso medido guarda el `valor` de la dimensión calibrada y el de la base, como en el veredicto.
    #[test]
    fn calibra_el_caso_guardado_dice_su_valor_y_el_de_la_base() {
        let home = tempfile::tempdir().expect("home");
        let carpeta = carpeta_con_repo_bueno(home.path());
        let repo = carpeta.join("repo-bueno");
        let base = veredicto_de(Dimension::Seguridad, 0.9, None);
        let ev = falso(&repo, &base, |limpia, _, _| {
            veredicto_de(Dimension::Seguridad, if limpia { 0.8 } else { 0.2 }, None)
        });
        let evaluador = |_: &Tarea, ruta: &Path, _: &OpcionesOraculo| ev(ruta);
        let calibrada = calibrar_tarea_con(
            home.path(),
            "calibra-fixture",
            "seguridad",
            4,
            42,
            None,
            None,
            &evaluador,
        )
        .expect("calibra seguridad");
        let guardados = renglones_guardados(&calibrada);
        assert!(guardados.iter().any(|c| c.es_defecto), "hay defectos");
        assert!(guardados.iter().any(|c| !c.es_defecto), "hay limpios");
        for caso in &guardados {
            let esperado = if caso.es_defecto { 0.2 } else { 0.8 };
            assert_eq!(caso.valor, Some(esperado), "caso {}: valor", caso.caso);
            assert_eq!(
                caso.valor_base,
                Some(0.9),
                "caso {}: valor de la base",
                caso.caso
            );
        }
    }

    #[test]
    fn calibra_diseno_marca_limpio_reusado_solo_en_los_limpios_del_cache() {
        // Punto 5: los limpios que Diseño reusa de Arquitectura dicen `limpio_reusado`. Los de Arquitectura no.
        // Los casos con defecto siempre dicen `false`.
        let home = tempfile::tempdir().expect("home");
        carpeta_con_repo_bueno(home.path());
        let arquitectura = AtomicUsize::new(0);
        let primera = calibrar_tarea_con(
            home.path(),
            "calibra-fixture",
            "arquitectura",
            4,
            42,
            None,
            None,
            &evaluador_con_limpios(&arquitectura, |_| veredicto_arq_dis(1.0)),
        )
        .expect("calibra arquitectura");
        let diseno = AtomicUsize::new(0);
        let segunda = calibrar_tarea_con(
            home.path(),
            "calibra-fixture",
            "diseno",
            4,
            42,
            None,
            None,
            &evaluador_con_limpios(&diseno, |_| veredicto_arq_dis(1.0)),
        )
        .expect("calibra diseño");
        let de_arquitectura = renglones_guardados(&primera);
        let de_diseno = renglones_guardados(&segunda);
        assert!(
            de_diseno.iter().any(|c| !c.es_defecto),
            "hay limpios de Diseño"
        );
        for caso in de_diseno.iter().filter(|c| !c.es_defecto) {
            assert!(caso.limpio_reusado, "caso {}: sale del caché", caso.caso);
        }
        for caso in de_arquitectura.iter().filter(|c| !c.es_defecto) {
            assert!(
                !caso.limpio_reusado,
                "caso {}: Arquitectura evalúa",
                caso.caso
            );
        }
        for caso in de_arquitectura
            .iter()
            .chain(&de_diseno)
            .filter(|c| c.es_defecto)
        {
            assert!(!caso.limpio_reusado, "caso {}: con defecto", caso.caso);
        }
    }

    #[test]
    fn calibra_renglon_sin_limpio_reusado_se_lee_como_false() {
        // Guarda: un renglón de antes del caché carga sin el campo y queda en `false`.
        let viejo = r#"{"caso":1,"es_defecto":false,"detectado":false}"#;
        let caso: CasoGuardado = serde_json::from_str(viejo).expect("renglón sin el campo");
        assert!(!caso.limpio_reusado);
    }

    /// Veredicto de Arquitectura medida y de Diseño no medida (`NO_MEDIDA`), como el evaluador real cuando
    /// se calibró otra dimensión.
    fn veredicto_arq_con_diseno_no_medido(_es_limpio: bool) -> Veredicto {
        let mut v = veredicto_arq_dis(1.0);
        v.puntajes[1].sin_veredicto = Some(crate::oracle::NO_MEDIDA.into());
        v
    }

    #[test]
    fn calibra_diseno_no_reusa_un_limpio_que_arquitectura_no_midio_en_diseno() {
        let home = tempfile::tempdir().expect("home");
        carpeta_con_repo_bueno(home.path());
        let arquitectura = AtomicUsize::new(0);
        calibrar_tarea_con(
            home.path(),
            "calibra-fixture",
            "arquitectura",
            4,
            42,
            None,
            None,
            &evaluador_con_limpios(&arquitectura, veredicto_arq_con_diseno_no_medido),
        )
        .expect("calibra arquitectura");
        let diseno = AtomicUsize::new(0);
        calibrar_tarea_con(
            home.path(),
            "calibra-fixture",
            "diseno",
            4,
            42,
            None,
            None,
            &evaluador_con_limpios(&diseno, |_| veredicto_arq_dis(1.0)),
        )
        .expect("calibra diseño");
        assert_eq!(
            diseno.load(Ordering::SeqCst),
            2,
            "Diseño no reusa un limpio que no midió: evalúa los dos"
        );
    }

    #[test]
    fn calibra_diseno_no_reusa_un_cache_sin_su_puntaje_y_lo_sobrescribe() {
        let home = tempfile::tempdir().expect("home");
        carpeta_con_repo_bueno(home.path());
        let arquitectura = AtomicUsize::new(0);
        calibrar_tarea_con(
            home.path(),
            "calibra-fixture",
            "arquitectura",
            4,
            42,
            None,
            None,
            &evaluador_con_limpios(&arquitectura, |_| {
                veredicto_de(Dimension::Arquitectura, 1.0, None)
            }),
        )
        .expect("calibra arquitectura");
        let sin_diseno = veredictos_limpios(home.path());
        assert!(
            sin_diseno
                .iter()
                .all(|v| puntaje_de(v, Dimension::Diseno).is_none()),
            "el cache de Arquitectura no trae Diseño"
        );
        let diseno = AtomicUsize::new(0);
        calibrar_tarea_con(
            home.path(),
            "calibra-fixture",
            "diseno",
            4,
            42,
            None,
            None,
            &evaluador_con_limpios(&diseno, |_| veredicto_arq_dis(1.0)),
        )
        .expect("calibra diseño");
        assert_eq!(
            diseno.load(Ordering::SeqCst),
            2,
            "sin el puntaje de Diseño, los limpios se evalúan de nuevo"
        );
        assert!(
            veredictos_limpios(home.path())
                .iter()
                .all(|v| puntaje_de(v, Dimension::Diseno).is_some()),
            "el cache queda con el puntaje de Diseño"
        );
    }

    #[test]
    fn calibra_diseno_con_otra_version_del_repo_no_reusa_nada() {
        let home = tempfile::tempdir().expect("home");
        let carpeta = carpeta_con_repo_bueno(home.path());
        let primera = AtomicUsize::new(0);
        calibrar_tarea_con(
            home.path(),
            "calibra-fixture",
            "diseno",
            4,
            42,
            None,
            None,
            &evaluador_con_limpios(&primera, |_| veredicto_arq_dis(1.0)),
        )
        .expect("calibra");
        std::fs::write(carpeta.join("repo-bueno").join("NOTA.txt"), "otra versión")
            .expect("cambia el repo");
        let segunda = AtomicUsize::new(0);
        calibrar_tarea_con(
            home.path(),
            "calibra-fixture",
            "diseno",
            4,
            42,
            None,
            None,
            &evaluador_con_limpios(&segunda, |_| veredicto_arq_dis(1.0)),
        )
        .expect("calibra con el repo cambiado");
        assert_eq!(
            segunda.load(Ordering::SeqCst),
            2,
            "otra versión del repo no reusa el cache"
        );
    }

    #[test]
    fn calibra_diseno_con_cache_corrupto_reevalua() {
        let home = tempfile::tempdir().expect("home");
        carpeta_con_repo_bueno(home.path());
        let primera = AtomicUsize::new(0);
        calibrar_tarea_con(
            home.path(),
            "calibra-fixture",
            "diseno",
            4,
            42,
            None,
            None,
            &evaluador_con_limpios(&primera, |_| veredicto_arq_dis(1.0)),
        )
        .expect("calibra");
        for ruta in archivos_limpios(home.path()) {
            std::fs::write(&ruta, "{ no es json").expect("corrompe el cache");
        }
        let segunda = AtomicUsize::new(0);
        calibrar_tarea_con(
            home.path(),
            "calibra-fixture",
            "diseno",
            4,
            42,
            None,
            None,
            &evaluador_con_limpios(&segunda, |_| veredicto_arq_dis(1.0)),
        )
        .expect("calibra con cache corrupto");
        assert_eq!(
            segunda.load(Ordering::SeqCst),
            2,
            "un cache corrupto no cuenta"
        );
        assert!(
            veredictos_limpios(home.path())
                .iter()
                .all(|v| puntaje_de(v, Dimension::Diseno).is_some()),
            "la reevaluación sobrescribe el cache corrupto"
        );
    }

    #[test]
    fn calibra_seguridad_no_escribe_ni_lee_la_carpeta_de_limpios() {
        let home = tempfile::tempdir().expect("home");
        carpeta_con_repo_bueno(home.path());
        let seguridad = AtomicUsize::new(0);
        calibrar_tarea_con(
            home.path(),
            "calibra-fixture",
            "seguridad",
            4,
            42,
            None,
            None,
            &evaluador_con_limpios(&seguridad, |_| {
                veredicto_de(Dimension::Seguridad, 1.0, None)
            }),
        )
        .expect("calibra seguridad");
        assert_eq!(
            seguridad.load(Ordering::SeqCst),
            2,
            "los limpios se evalúan"
        );
        assert!(
            !home.path().join(CARPETA_CASOS).join("limpios").exists(),
            "Seguridad no escribe cache de limpios"
        );
    }

    #[test]
    fn calibra_limpios_reusados_dan_los_mismos_casos_que_sin_cache() {
        let sin_cache = {
            let home = tempfile::tempdir().expect("home");
            carpeta_con_repo_bueno(home.path());
            let llamadas = AtomicUsize::new(0);
            limpios_medidos(
                &calibrar_tarea_con(
                    home.path(),
                    "calibra-fixture",
                    "diseno",
                    4,
                    42,
                    None,
                    None,
                    &evaluador_con_limpios(&llamadas, veredicto_limpio_sin_juez),
                )
                .expect("calibra sin cache"),
            )
        };
        let con_cache = {
            let home = tempfile::tempdir().expect("home");
            carpeta_con_repo_bueno(home.path());
            let llamadas = AtomicUsize::new(0);
            calibrar_tarea_con(
                home.path(),
                "calibra-fixture",
                "arquitectura",
                4,
                42,
                None,
                None,
                &evaluador_con_limpios(&llamadas, veredicto_limpio_sin_juez),
            )
            .expect("calibra arquitectura");
            let segunda = AtomicUsize::new(0);
            let calibrada = calibrar_tarea_con(
                home.path(),
                "calibra-fixture",
                "diseno",
                4,
                42,
                None,
                None,
                &evaluador_con_limpios(&segunda, veredicto_limpio_sin_juez),
            )
            .expect("calibra diseño");
            assert_eq!(segunda.load(Ordering::SeqCst), 0, "usa el cache");
            limpios_medidos(&calibrada)
        };
        assert!(!sin_cache.is_empty(), "hay limpios medidos");
        assert!(
            sin_cache
                .iter()
                .all(|(_, detectado, motivo)| !*detectado && motivo.is_none()),
            "el limpio sin juez no detecta: {sin_cache:?}"
        );
        assert_eq!(con_cache, sin_cache);
    }

    fn fila_de_prueba(tarea: &str, ts: f64) -> Fila {
        fila(tarea, Dimension::Seguridad, ts, Some(0.95))
    }

    #[test]
    fn muestra_suficiente_no_cuenta_limpios_sin_veredicto() {
        let f = Fila {
            defectos: MIN_DEFECTOS_APLICABLES,
            limpios: 5,
            limpios_sin_veredicto: 1,
            ..fila("memora", Dimension::Seguridad, 1.0, Some(1.0))
        };
        assert!(!muestra_suficiente(&f));
    }

    #[test]
    fn muestra_suficiente_con_limpios_con_veredicto_suficientes() {
        let f = Fila {
            defectos: MIN_DEFECTOS_APLICABLES,
            limpios: 6,
            limpios_sin_veredicto: 1,
            ..fila("memora", Dimension::Seguridad, 1.0, Some(1.0))
        };
        assert!(muestra_suficiente(&f));
    }

    #[test]
    fn calibra_limpios_sin_veredicto_hasta_el_20_por_ciento_calibra() {
        let dentro = Fila {
            limpios_sin_veredicto: 2,
            ..fila("memora", Dimension::Seguridad, 1.0, Some(1.0))
        };
        let fuera = Fila {
            limpios_sin_veredicto: 3,
            ..dentro.clone()
        };
        for (fila, calibrada) in [(dentro, true), (fuera, false)] {
            let c = Calibraciones {
                v: VERSION,
                filas: vec![fila],
            };
            let mut puntajes = vec![puntaje(Dimension::Seguridad, 0.8)];
            c.aplicar("memora", "", &mut puntajes);
            assert_eq!(puntajes[0].calibrada, calibrada);
        }
    }

    #[test]
    fn fila_sin_limpios_sin_veredicto_carga_con_cero() {
        let fila = fila("memora", Dimension::Seguridad, 1.0, Some(0.95));
        let mut vieja = serde_json::to_value(&fila).expect("serializa");
        vieja
            .as_object_mut()
            .expect("objeto")
            .remove("limpios_sin_veredicto");
        let leida: Fila = serde_json::from_value(vieja).expect("un JSON viejo parsea");
        assert_eq!(leida.limpios_sin_veredicto, 0);
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
            inestables: Vec::new(),
            motivo_sin_calibrar: None,
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
        c.aplicar("memora", "", &mut puntajes);
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
        c.aplicar("memora", "", &mut puntajes);
        // Diseño no está calibrada; Funciona está calibrada pero sin veredicto.
        let global = global_de(&puntajes).expect("hay una calibrada");
        assert!((global - 0.6).abs() < 1e-6, "{global}");
    }

    #[test]
    fn calibra_global_sin_calibradas_es_none() {
        let mut puntajes = vec![puntaje(Dimension::Seguridad, 0.9)];
        Calibraciones::default().aplicar("memora", "", &mut puntajes);
        assert_eq!(global_de(&puntajes), None);
    }

    /// Guarda: una fila "no aplica" tiene 0 defectos y 0 limpios. Aunque su exactitud valga 1.0, no calibra.
    #[test]
    fn calibra_fila_no_aplica_no_calibra_aunque_tenga_exactitud() {
        let mut no_aplica = fila("memora", Dimension::Seguridad, 1.0, Some(1.0));
        no_aplica.defectos = 0;
        no_aplica.limpios = 0;
        no_aplica.no_aplica = Some("no aplica: la tarea no define [entrega]".into());
        let c = Calibraciones {
            v: VERSION,
            filas: vec![no_aplica],
        };
        let mut puntajes = vec![puntaje(Dimension::Seguridad, 0.9)];
        c.aplicar("memora", "", &mut puntajes);
        assert!(!puntajes[0].calibrada, "una fila no aplica no calibra");
        assert_eq!(puntajes[0].exactitud_balanceada, None);
    }

    /// Fila de Diseño con exactitud 1.0 y la huella dada. Sus conteos pasan la muestra mínima.
    fn fila_de_diseno_con_huella(huella: Option<String>) -> Fila {
        Fila {
            huella,
            ..fila("memora", Dimension::Diseno, 1.0, Some(1.0))
        }
    }

    /// Aplica una sola fila a un puntaje de `dimension` y dice si la calibra.
    fn calibra_con_una_fila(fila: Fila, dimension: Dimension) -> bool {
        let c = Calibraciones {
            v: VERSION,
            filas: vec![fila],
        };
        let mut puntajes = vec![puntaje(dimension, 0.8)];
        c.aplicar("memora", "", &mut puntajes);
        puntajes[0].calibrada
    }

    #[test]
    fn huella_vieja_no_calibra_diseno_aunque_la_exactitud_llegue_al_umbral() {
        let vieja = fila_de_diseno_con_huella(Some("vieja".into()));
        assert_eq!(vieja.exactitud_balanceada, Some(1.0));
        assert!(!calibra_con_una_fila(vieja, Dimension::Diseno));
    }

    #[test]
    fn fila_sin_huella_no_calibra_diseno() {
        let sin = fila_de_diseno_con_huella(None);
        assert!(!calibra_con_una_fila(sin, Dimension::Diseno));
    }

    #[test]
    fn huella_actual_calibra_diseno() {
        let actual = fila_de_diseno_con_huella(Some(huella_de(Dimension::Diseno)));
        assert!(calibra_con_una_fila(actual, Dimension::Diseno));
    }

    #[test]
    fn dimension_solo_de_sondas_sin_huella_sigue_calibrando() {
        let sin = fila("memora", Dimension::Seguridad, 1.0, Some(1.0));
        assert_eq!(sin.huella, None);
        assert!(calibra_con_una_fila(sin, Dimension::Seguridad));
    }

    /// Crea una fila de Seguridad con exactitud 1.0 y el `tarea_hash` dado. Sus conteos pasan la muestra mínima.
    fn fila_con_tarea_hash(tarea_hash: Option<String>) -> Fila {
        Fila {
            tarea_hash,
            ..fila("memora", Dimension::Seguridad, 1.0, Some(1.0))
        }
    }

    #[test]
    fn fila_con_otro_tarea_toml_no_calibra_y_da_su_motivo() {
        let c = Calibraciones {
            v: VERSION,
            filas: vec![fila_con_tarea_hash(Some("tarea-vieja".into()))],
        };
        let mut puntajes = vec![puntaje(Dimension::Seguridad, 0.8)];
        c.aplicar("memora", "tarea-actual", &mut puntajes);
        assert!(!puntajes[0].calibrada);
        assert_eq!(
            c.motivo_sin_calibrar("memora", "tarea-actual", Dimension::Seguridad),
            Some("calibración con otro tarea.toml")
        );
    }

    #[test]
    fn fila_sin_tarea_hash_sigue_calibrando() {
        let c = Calibraciones {
            v: VERSION,
            filas: vec![fila_con_tarea_hash(None)],
        };
        let mut puntajes = vec![puntaje(Dimension::Seguridad, 0.8)];
        c.aplicar("memora", "tarea-actual", &mut puntajes);
        assert!(puntajes[0].calibrada);
        assert_eq!(
            c.motivo_sin_calibrar("memora", "tarea-actual", Dimension::Seguridad),
            None
        );
    }

    #[test]
    fn version_de_sondas_de_arquitectura_cambia_su_huella_y_no_la_de_seguridad() {
        let prompt = huella_del_prompt();
        assert_eq!(version_de_sondas(Dimension::Arquitectura), 2);
        assert_ne!(
            huella_con_version(Dimension::Arquitectura, &prompt, 1),
            huella_con_version(Dimension::Arquitectura, &prompt, 2)
        );
        assert_eq!(
            huella_de(Dimension::Arquitectura),
            huella_con_version(Dimension::Arquitectura, &prompt, 2)
        );
        assert_eq!(
            huella_de(Dimension::Seguridad),
            huella_con_version(Dimension::Seguridad, &prompt, 1)
        );
    }

    #[test]
    fn inestables_de_da_los_de_la_fila_mas_reciente() {
        let vieja = Fila {
            inestables: vec!["diseno.errores".into()],
            huella: Some(huella_de(Dimension::Diseno)),
            ..fila("memora", Dimension::Diseno, 1.0, Some(1.0))
        };
        let nueva = Fila {
            inestables: vec!["diseno.ciclos".into()],
            huella: Some(huella_de(Dimension::Diseno)),
            ..fila("memora", Dimension::Diseno, 2.0, Some(1.0))
        };
        let c = Calibraciones {
            v: VERSION,
            filas: vec![nueva, vieja],
        };
        assert_eq!(
            c.inestables_de("memora", Dimension::Diseno),
            ["diseno.ciclos".to_string()]
        );
        assert!(c.inestables_de("otra", Dimension::Diseno).is_empty());
    }

    #[test]
    fn inestables_de_con_huella_vieja_no_los_da() {
        let vieja = Fila {
            inestables: vec!["diseno.errores".into()],
            huella: Some("vieja".into()),
            ..fila("memora", Dimension::Diseno, 1.0, Some(1.0))
        };
        let c = Calibraciones {
            v: VERSION,
            filas: vec![vieja],
        };
        assert!(c.inestables_de("memora", Dimension::Diseno).is_empty());
    }

    #[test]
    fn calibra_fila_vieja_con_motivo_inestable_en_no_aplica_se_lee_como_inestable() {
        let texto = r#"{"v":1,"filas":[
            {"tarea":"memora","dimension":"diseno","n":0,"semilla":42,"ts":1.0,
             "no_aplica":"inestable en esta tarea: diseno.modularidad"},
            {"tarea":"memora","dimension":"seguridad","n":0,"semilla":42,"ts":1.0,
             "no_aplica":"no aplica: la tarea no define [entrega]"}
        ]}"#;
        let home = home_con(texto);
        let leida = Calibraciones::cargar(home.path()).expect("carga");
        let inestable = &leida.filas[0];
        assert_eq!(
            inestable.inestable.as_deref(),
            Some("inestable en esta tarea: diseno.modularidad")
        );
        assert_eq!(inestable.no_aplica, None);
        let no_aplica = &leida.filas[1];
        assert_eq!(no_aplica.inestable, None);
        assert_eq!(
            no_aplica.no_aplica.as_deref(),
            Some("no aplica: la tarea no define [entrega]")
        );
    }

    #[test]
    fn dimension_solo_de_sondas_con_huella_distinta_no_calibra() {
        let otra = Fila {
            inestables: Vec::new(),
            huella: Some("vieja".into()),
            ..fila("memora", Dimension::Seguridad, 1.0, Some(1.0))
        };
        assert!(!calibra_con_una_fila(otra, Dimension::Seguridad));
    }

    #[test]
    fn motivo_de_otro_juez_aparece_con_huella_vieja() {
        let c = Calibraciones {
            v: VERSION,
            filas: vec![fila_de_diseno_con_huella(Some("vieja".into()))],
        };
        assert_eq!(
            c.motivo_sin_calibrar("memora", "", Dimension::Diseno),
            Some("calibración de otro juez")
        );
        let vigente = Calibraciones {
            v: VERSION,
            filas: vec![fila_de_diseno_con_huella(Some(huella_de(
                Dimension::Diseno,
            )))],
        };
        assert_eq!(
            vigente.motivo_sin_calibrar("memora", "", Dimension::Diseno),
            None
        );
        assert_eq!(c.motivo_sin_calibrar("otra", "", Dimension::Diseno), None);
    }

    #[test]
    fn fila_vieja_sin_huella_en_json_se_lee_none() {
        let fila = fila("memora", Dimension::Diseno, 1.0, Some(1.0));
        let mut vieja = serde_json::to_value(&fila).expect("serializa");
        vieja.as_object_mut().expect("objeto").remove("huella");
        let leida: Fila = serde_json::from_value(vieja).expect("un JSON viejo parsea");
        assert_eq!(leida.huella, None);
    }

    #[test]
    fn calibra_huella_de_seguridad_no_depende_del_prompt() {
        assert_eq!(
            huella_con_version(
                Dimension::Seguridad,
                "a",
                version_de_sondas(Dimension::Seguridad)
            ),
            huella_con_version(
                Dimension::Seguridad,
                "b",
                version_de_sondas(Dimension::Seguridad)
            )
        );
    }

    #[test]
    fn calibra_huella_de_diseno_depende_del_prompt() {
        assert_ne!(
            huella_con_version(Dimension::Diseno, "a", version_de_sondas(Dimension::Diseno)),
            huella_con_version(Dimension::Diseno, "b", version_de_sondas(Dimension::Diseno))
        );
    }

    #[test]
    fn calibra_huella_de_diseno_no_cambia_de_valor() {
        let juntas: String = rubricas_de(Dimension::Diseno)
            .iter()
            .map(Rubrica::hash)
            .collect();
        let esperada = calcular_hash(format!("{}\n{juntas}", huella_del_prompt()).as_bytes());
        assert_eq!(huella_de(Dimension::Diseno), esperada);
    }

    #[test]
    fn calibrar_con_escribe_la_huella_de_su_dimension() {
        let base = veredicto_base();
        let fila = calibrar(
            Path::new(FIXTURE),
            &base,
            |limpia, _, base| if limpia { base.clone() } else { hundido(base) },
            20,
        );
        assert_eq!(fila.huella, Some(huella_de(Dimension::Seguridad)));
        assert_ne!(
            huella_de(Dimension::Seguridad),
            huella_de(Dimension::Diseno)
        );
    }

    #[test]
    fn calibrar_con_escribe_el_hash_del_tarea_toml() {
        let base = veredicto_base();
        let (_t, tarea) = tarea_prueba();
        let ev = falso(Path::new(FIXTURE), &base, |limpia, _, base| {
            if limpia { base.clone() } else { hundido(base) }
        });
        let fila = calibrar_con(
            &tarea,
            Dimension::Seguridad,
            20,
            42,
            Path::new(FIXTURE),
            &ev,
        )
        .expect("calibra");
        let esperado = calcular_hash(&fs::read(tarea.dir.join("tarea.toml")).expect("lee"));
        assert_eq!(fila.tarea_hash, Some(esperado));
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
        let hash = fila
            .tarea_hash
            .clone()
            .expect("calibrar deja el hash del tarea.toml");
        let c = Calibraciones {
            v: VERSION,
            filas: vec![fila],
        };
        let mut puntajes = vec![puntaje(Dimension::Seguridad, 0.0)];
        c.aplicar(&tarea, &hash, &mut puntajes);
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
        let hash = fila
            .tarea_hash
            .clone()
            .expect("calibrar deja el hash del tarea.toml");
        let c = Calibraciones {
            v: VERSION,
            filas: vec![fila],
        };
        let mut puntajes = vec![puntaje(Dimension::Seguridad, 0.0)];
        c.aplicar(&tarea, &hash, &mut puntajes);
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
            c.aplicar("memora", "", &mut puntajes);
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
    fn calibra_validar_precios_sin_precio_nombra_el_modelo_y_sale_con_2() {
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
        let error = validar_precios_de_jueces(&cfg, &jueces, &partes).expect_err("sin precio");
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
        c.aplicar("memora", "", &mut puntajes);
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

    /// La copia de `base` con un hallazgo de sonda nuevo en `dimension`: un defecto que la regla con juez detecta.
    fn con_hallazgo_de_sonda(base: &Veredicto, dimension: Dimension) -> Veredicto {
        let mut v = base.clone();
        let mut nuevo = hallazgo("nuevo-sonda");
        nuevo.dimension = dimension;
        v.hallazgos.push(nuevo);
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
                // Estructura detecta por sondas o por el juez (Tarea 63): una baja de `valor` ya no cuenta.
                con_hallazgo_de_sonda(base, Dimension::Estructura)
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

    const MUERTA: &str = "pub fn sin_uso(a: i32) -> bool {\n    a == 1\n}\n";
    /// La firma ocupa varias líneas: la sonda tiene que caer en el cuerpo (Tarea 66, punto 4).
    const VIVA: &str = "pub fn usada(\n    a: i32,\n) -> bool {\n    a == 2\n}\n";
    const FUERA_DE_FN: &str = "pub const EN_CONSTANTE: bool = 1 == 1;\n";
    const MOTIVO_EXACTO: &str = "ninguna línea candidata está en una función que la suite ejecute";

    /// Copia del fixture con los archivos `archivos` en `src/`. Cada uno tiene un `==` candidato.
    fn copia_con_sondas(archivos: &[(&str, &str)]) -> TempDir {
        let dir = copia_fixture();
        // La tarea va antes que cualquier `arbol` de la base: la base y las copias la tienen igual.
        tarea_en(dir.path(), "");
        for (ruta, texto) in archivos {
            fs::write(dir.path().join(ruta), texto).expect("escribe el archivo de prueba");
        }
        dir
    }

    /// Evaluador de sondas (Tarea 66, punto 4). Una copia con `root-sonda` cambia el veredicto solo si la
    /// sonda cae en `ejecutada`. Una inyección sin sonda detecta solo si `detecta`. Cuenta las copias con sonda.
    fn evaluador_de_sondas<'a>(
        base_dir: &'a Path,
        base: &'a Veredicto,
        ejecutada: &'a str,
        detecta: bool,
        sondas: &'a AtomicUsize,
    ) -> impl Fn(&Path, TipoCaso, Target) -> Result<Veredicto, ErrorOraculo> + Sync + 'a {
        let arbol_base = arbol(base_dir);
        move |ruta: &Path, _: TipoCaso, _: Target| {
            let copia = arbol(ruta);
            if copia == arbol_base {
                return Ok(base.clone());
            }
            let archivos_con_sonda: Vec<&String> = copia
                .iter()
                .filter(|(_, texto)| texto.contains("root-sonda"))
                .map(|(archivo, _)| archivo)
                .collect();
            if archivos_con_sonda.is_empty() {
                let detectada = detecta.then(|| con_hallazgo_de_sonda(base, Dimension::Funciona));
                return Ok(detectada.unwrap_or_else(|| base.clone()));
            }
            sondas.fetch_add(1, Ordering::SeqCst);
            let cambia = archivos_con_sonda.iter().any(|a| a.as_str() == ejecutada);
            Ok(if cambia {
                con_hallazgo_de_sonda(base, Dimension::Funciona)
            } else {
                base.clone()
            })
        }
    }

    /// Mide `n` casos de Funciona en `dir` con `ev`. La semilla fija la línea que se elige primero.
    fn medir_con_sondas(
        dir: &Path,
        n: usize,
        semilla: u64,
        ev: &(dyn Fn(&Path, TipoCaso, Target) -> Result<Veredicto, ErrorOraculo> + Sync),
    ) -> Medida {
        let tarea = tarea_en(dir, "");
        calibrar_medida_con_hilos(
            &tarea,
            Dimension::Funciona,
            n,
            semilla,
            dir,
            ev,
            None,
            &Vec::new,
            &|| None,
            1,
            None,
        )
        .expect("mide")
    }

    /// El caso con defecto de una medición de Funciona.
    fn caso_con_defecto(medida: &Medida) -> &Caso {
        medida
            .casos
            .iter()
            .find(|c| c.inyeccion.is_some())
            .expect("un caso con defecto")
    }

    #[test]
    fn sonda_linea_no_ejecutada_se_reemplaza_y_mide_la_siguiente() {
        let dir = copia_con_sondas(&[("src/muerta.rs", MUERTA), ("src/viva.rs", VIVA)]);
        let base = veredicto_de(Dimension::Funciona, 1.0, None);
        let sondas = AtomicUsize::new(0);
        let ev = evaluador_de_sondas(dir.path(), &base, "src/viva.rs", false, &sondas);
        // Con la semilla 2 la primera elección es muerta.rs: se descarta una vez y se mide viva.rs.
        let medida = medir_con_sondas(dir.path(), 2, 2, &ev);
        let fila = medida.fila.as_ref().expect("fila");
        assert!(
            fila.reemplazos >= 1,
            "la línea de muerta.rs se descarta: {}",
            fila.reemplazos
        );
        let defecto = caso_con_defecto(&medida);
        assert_eq!(
            defecto.inyeccion.as_ref().map(|i| i.archivo.as_str()),
            Some("src/viva.rs"),
            "el caso mide la línea que sí ejecuta la suite"
        );
        assert!(!defecto.detectado);
    }

    #[test]
    fn sonda_linea_ejecutada_que_no_cambia_nada_es_falso_negativo_medido() {
        let dir = copia_con_sondas(&[("src/viva.rs", VIVA)]);
        let base = veredicto_de(Dimension::Funciona, 1.0, None);
        let sondas = AtomicUsize::new(0);
        let ev = evaluador_de_sondas(dir.path(), &base, "src/viva.rs", false, &sondas);
        let medida = medir_con_sondas(dir.path(), 2, 42, &ev);
        assert_eq!(sondas.load(Ordering::SeqCst), 1, "la sonda corre una vez");
        let defecto = caso_con_defecto(&medida);
        assert!(
            !defecto.detectado,
            "la inyección no detecta y la sonda sí cambia: medido"
        );
        assert_eq!(medida.fila.as_ref().expect("fila").reemplazos, 0);
    }

    #[test]
    fn sonda_caso_detectado_no_corre_la_sonda() {
        let dir = copia_con_sondas(&[("src/viva.rs", VIVA)]);
        let base = veredicto_de(Dimension::Funciona, 1.0, None);
        let sondas = AtomicUsize::new(0);
        let ev = evaluador_de_sondas(dir.path(), &base, "src/viva.rs", true, &sondas);
        let medida = medir_con_sondas(dir.path(), 2, 42, &ev);
        assert_eq!(
            sondas.load(Ordering::SeqCst),
            0,
            "un caso detectado no corre la sonda"
        );
        assert!(caso_con_defecto(&medida).detectado);
    }

    #[test]
    fn sonda_sin_fn_que_contenga_la_linea_mide_sin_sonda() {
        let dir = copia_con_sondas(&[("src/fuera.rs", FUERA_DE_FN)]);
        let base = veredicto_de(Dimension::Funciona, 1.0, None);
        let sondas = AtomicUsize::new(0);
        let ev = evaluador_de_sondas(dir.path(), &base, "src/fuera.rs", false, &sondas);
        let medida = medir_con_sondas(dir.path(), 2, 42, &ev);
        assert_eq!(
            sondas.load(Ordering::SeqCst),
            0,
            "sin fn no hay sonda: se mide igual"
        );
        let defecto = caso_con_defecto(&medida);
        assert_eq!(
            defecto.inyeccion.as_ref().map(|i| i.archivo.as_str()),
            Some("src/fuera.rs")
        );
        assert!(!defecto.detectado);
    }

    #[test]
    fn sonda_sin_candidatos_ejecutados_es_no_aplicable_con_motivo_de_sonda() {
        let dir = copia_con_sondas(&[("src/muerta.rs", MUERTA)]);
        let base = veredicto_de(Dimension::Funciona, 1.0, None);
        let sondas = AtomicUsize::new(0);
        let ev = evaluador_de_sondas(dir.path(), &base, "src/viva.rs", false, &sondas);
        let medida = medir_con_sondas(dir.path(), 2, 42, &ev);
        let fila = medida
            .fila
            .as_ref()
            .expect("hay fila: queda el caso limpio");
        assert_eq!(fila.no_aplicable, 1);
        assert_eq!(fila.reemplazos, MAX_REEMPLAZOS + 1);
        assert_eq!(sondas.load(Ordering::SeqCst), (MAX_REEMPLAZOS + 1) as usize);
        assert_eq!(medida.no_aplicables.len(), 1);
        assert_eq!(medida.no_aplicables[0].motivo, MOTIVO_EXACTO);
    }

    /// Mide `n` casos de `dimension` con el evaluador falso `regla` (la base es `base_dir`, como en `falso`).
    /// Un hilo: los casos salen en orden de índice.
    fn medir(
        tarea: &Tarea,
        dimension: Dimension,
        base_dir: &Path,
        base: &Veredicto,
        regla: impl Fn(bool, &BTreeMap<String, String>, &Veredicto) -> Veredicto + Sync,
        n: usize,
    ) -> Medida {
        let ev = falso(base_dir, base, regla);
        let evaluador = |ruta: &Path, _: TipoCaso, _: Target| ev(ruta);
        calibrar_medida_con_hilos(
            tarea,
            dimension,
            n,
            42,
            base_dir,
            &evaluador,
            None,
            &Vec::new,
            &|| None,
            1,
            None,
        )
        .expect("mide")
    }

    #[test]
    fn calibra_caso_limpio_detectado_guarda_motivo_y_niveles() {
        let (_t, tarea) = tarea_prueba();
        let base = veredicto_base();
        // Las copias limpias ganan un hallazgo con id nuevo: el limpio cuenta como detectado por ese motivo.
        let medida = medir(
            &tarea,
            Dimension::Seguridad,
            Path::new(FIXTURE),
            &base,
            |limpia, _, base| {
                let mut v = base.clone();
                if limpia {
                    v.hallazgos.push(hallazgo("nuevo"));
                }
                v
            },
            2,
        );
        let guardados: Vec<CasoGuardado> = medida.casos.iter().map(CasoGuardado::de).collect();
        let limpio = guardados.iter().find(|c| !c.es_defecto).expect("un limpio");
        assert!(limpio.detectado);
        assert_eq!(limpio.motivo.as_deref(), Some("hallazgo_nuevo:x"));
        let niveles = BTreeMap::from([
            ("seguridad.panico_en_entrada".to_string(), 3u8),
            ("seguridad.secretos".to_string(), 3u8),
        ]);
        assert_eq!(limpio.niveles_base, niveles);
        assert_eq!(limpio.niveles, niveles);
    }

    #[test]
    fn calibra_defecto_detectado_por_item_guarda_el_item_que_bajo() {
        let (_t, tarea) = tarea_prueba();
        let base = veredicto_base();
        // Solo el secreto baja su ítem: el valor de la dimensión no cambia.
        let medida = medir(
            &tarea,
            Dimension::Seguridad,
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
        let guardados: Vec<CasoGuardado> = medida.casos.iter().map(CasoGuardado::de).collect();
        let secretos: Vec<&CasoGuardado> = guardados
            .iter()
            .filter(|c| c.es_defecto && c.item.as_deref() == Some("seguridad.secretos"))
            .collect();
        assert!(!secretos.is_empty(), "hay defectos de secretos");
        for caso in &secretos {
            assert_eq!(
                caso.motivo.as_deref(),
                Some("item_bajo:seguridad.secretos"),
                "caso {}",
                caso.caso
            );
            assert_eq!(caso.niveles["seguridad.secretos"], 0);
            assert_eq!(caso.niveles_base["seguridad.secretos"], 3);
        }
        let panicos: Vec<&CasoGuardado> = guardados
            .iter()
            .filter(|c| c.es_defecto && c.item.as_deref() == Some("seguridad.panico_en_entrada"))
            .collect();
        assert!(!panicos.is_empty(), "hay defectos de pánico");
        for caso in &panicos {
            assert_eq!(caso.motivo, None, "caso {}: no bajó nada", caso.caso);
            assert!(!caso.detectado);
        }
    }

    /// Hallazgo de `dimension` que emite el juez, con `id`, en `archivo:linea`.
    fn hallazgo_del_juez(dimension: Dimension, id: &str, archivo: &str, linea: u32) -> Hallazgo {
        Hallazgo {
            dimension,
            severidad: 2,
            archivo: archivo.into(),
            linea,
            cita: "x".into(),
            detalle: String::new(),
            fuente: Fuente::Juez {
                modelo: "deepseek-reasoner".into(),
            },
            id: id.into(),
            impacto: vec![],
        }
    }

    #[test]
    fn calibra_seguridad_item_que_baja_uno_sigue_detectando() {
        // Guarda: Seguridad no entra en la regla con juez. Un ítem de la sonda que baja 1 nivel detecta.
        let (_t, tarea) = tarea_prueba();
        let base = veredicto_base();
        let medida = medir(
            &tarea,
            Dimension::Seguridad,
            Path::new(FIXTURE),
            &base,
            |_, copia, base| {
                let mut v = base.clone();
                if copia.values().any(|t| t.contains("AKIA")) {
                    for item in &mut v.puntajes[0].items {
                        if item.item == "seguridad.secretos" {
                            item.nivel = 2;
                        }
                    }
                }
                v
            },
            20,
        );
        let guardados: Vec<CasoGuardado> = medida.casos.iter().map(CasoGuardado::de).collect();
        let secretos: Vec<&CasoGuardado> = guardados
            .iter()
            .filter(|c| c.es_defecto && c.item.as_deref() == Some("seguridad.secretos"))
            .collect();
        assert!(!secretos.is_empty(), "hay defectos de secretos");
        for caso in &secretos {
            assert!(caso.detectado, "caso {}: bajó 1 nivel", caso.caso);
            assert_eq!(
                caso.motivo.as_deref(),
                Some("item_bajo:seguridad.secretos"),
                "caso {}",
                caso.caso
            );
        }
    }

    #[test]
    fn calibra_arquitectura_hallazgo_del_juez_nuevo_en_un_limpio_no_detecta() {
        // Punto 2: un hallazgo del juez con id nuevo, en un caso limpio, ya no detecta en Arquitectura.
        let (_t, tarea) = tarea_prueba();
        let base = veredicto_de(Dimension::Arquitectura, 1.0, None);
        let medida = medir(
            &tarea,
            Dimension::Arquitectura,
            Path::new(FIXTURE),
            &base,
            |limpia, _, base| {
                let mut v = base.clone();
                if limpia {
                    v.hallazgos.push(hallazgo_del_juez(
                        Dimension::Arquitectura,
                        "nuevo",
                        "src/lib.rs",
                        1,
                    ));
                }
                v
            },
            4,
        );
        let limpios: Vec<CasoGuardado> = medida
            .casos
            .iter()
            .filter(|c| !c.es_defecto)
            .map(CasoGuardado::de)
            .collect();
        assert!(!limpios.is_empty(), "hay casos limpios");
        for caso in &limpios {
            assert!(!caso.detectado, "caso {}: no detecta", caso.caso);
            assert_eq!(caso.motivo, None, "caso {}", caso.caso);
        }
    }

    const ITEM_ARQUITECTURA: &str = "arquitectura.dependencias";

    /// Arquitectura sana con su ítem del juez en `nivel`.
    fn arquitectura_con_item(nivel: u8) -> Veredicto {
        Veredicto {
            puntajes: vec![Puntaje {
                items: vec![NivelItem {
                    item: ITEM_ARQUITECTURA.into(),
                    nivel,
                    evidencia: vec![],
                    fuente: Fuente::Juez {
                        modelo: "deepseek-reasoner".into(),
                    },
                }],
                ..puntaje(Dimension::Arquitectura, 1.0)
            }],
            ..Veredicto::default()
        }
    }

    /// Archivo y línea 1-based del `use` que `CicloEntreCapas` agrega en la copia: la línea que termina en `as _;`.
    fn lugar_del_use(copia: &BTreeMap<String, String>) -> Option<(String, u32)> {
        copia.iter().find_map(|(archivo, texto)| {
            texto
                .lines()
                .position(|l| l.ends_with("as _;"))
                .map(|i| (archivo.clone(), i as u32 + 1))
        })
    }

    /// Mide Arquitectura con `n` casos y un evaluador que recuerda el `use` del último defecto. Cada copia limpia
    /// recibe `de_limpia(lugar, base)` con ese lugar. Un hilo: el defecto `i-1` se mide antes que el limpio `i`.
    fn medir_arquitectura(
        base: &Veredicto,
        de_limpia: impl Fn(&(String, u32), &Veredicto) -> Veredicto + Sync,
        n: usize,
    ) -> Medida {
        let (_t, tarea) = tarea_prueba();
        let ultimo: Mutex<Option<(String, u32)>> = Mutex::new(None);
        medir(
            &tarea,
            Dimension::Arquitectura,
            Path::new(FIXTURE),
            base,
            |limpia, copia, base| {
                if !limpia {
                    if let Some(lugar) = lugar_del_use(copia) {
                        *ultimo.lock().expect("lock") = Some(lugar);
                    }
                    return base.clone();
                }
                let previo = ultimo.lock().expect("lock").clone();
                match previo {
                    Some(lugar) => de_limpia(&lugar, base),
                    None => base.clone(),
                }
            },
            n,
        )
    }

    fn limpios_guardados(medida: &Medida) -> Vec<CasoGuardado> {
        medida
            .casos
            .iter()
            .filter(|c| !c.es_defecto)
            .map(CasoGuardado::de)
            .collect()
    }

    #[test]
    fn calibra_arquitectura_limpio_cita_del_juez_en_el_defecto_anterior_cuenta_falsa_alarma() {
        // Punto 3: el juez cita el `use` del defecto anterior en el limpio. Ese limpio cuenta como detectado.
        let medida = medir_arquitectura(
            &arquitectura_con_item(3),
            |(archivo, linea), base| {
                let mut v = base.clone();
                v.hallazgos.push(hallazgo_del_juez(
                    Dimension::Arquitectura,
                    "cita",
                    archivo,
                    *linea,
                ));
                v
            },
            4,
        );
        let limpios = limpios_guardados(&medida);
        assert!(!limpios.is_empty(), "hay casos limpios");
        for caso in &limpios {
            assert!(
                caso.detectado,
                "caso {}: el juez cita el defecto",
                caso.caso
            );
            assert_eq!(
                caso.motivo.as_deref(),
                Some("cita_juez"),
                "caso {}",
                caso.caso
            );
        }
    }

    #[test]
    fn calibra_arquitectura_limpio_cita_del_juez_en_otro_archivo_no_detecta() {
        // Punto 3: la misma cita, en otro archivo que el defecto anterior, no cuenta.
        let medida = medir_arquitectura(
            &arquitectura_con_item(3),
            |_, base| {
                let mut v = base.clone();
                v.hallazgos.push(hallazgo_del_juez(
                    Dimension::Arquitectura,
                    "cita",
                    "src/fuera.rs",
                    1,
                ));
                v
            },
            4,
        );
        let limpios = limpios_guardados(&medida);
        assert!(!limpios.is_empty(), "hay casos limpios");
        for caso in &limpios {
            assert!(!caso.detectado, "caso {}: otro archivo", caso.caso);
            assert_eq!(caso.motivo, None, "caso {}", caso.caso);
        }
    }

    #[test]
    fn calibra_arquitectura_limpio_con_item_del_defecto_que_baja_dos_detecta() {
        // Punto 3: el ítem del defecto anterior baja 2 niveles en el limpio: detecta.
        let medida = medir_arquitectura(
            &arquitectura_con_item(3),
            |_, _| arquitectura_con_item(1),
            4,
        );
        let limpios = limpios_guardados(&medida);
        assert!(!limpios.is_empty(), "hay casos limpios");
        for caso in &limpios {
            assert!(caso.detectado, "caso {}: bajó 2 niveles", caso.caso);
            assert_eq!(
                caso.motivo.as_deref(),
                Some("item_baja_2:arquitectura.dependencias"),
                "caso {}",
                caso.caso
            );
        }
    }

    #[test]
    fn calibra_arquitectura_limpio_con_item_del_defecto_que_baja_uno_no_detecta() {
        // Punto 3: una baja de 1 nivel del ítem del defecto anterior no cuenta.
        let medida = medir_arquitectura(
            &arquitectura_con_item(3),
            |_, _| arquitectura_con_item(2),
            4,
        );
        let limpios = limpios_guardados(&medida);
        assert!(!limpios.is_empty(), "hay casos limpios");
        for caso in &limpios {
            assert!(!caso.detectado, "caso {}: bajó 1 nivel", caso.caso);
            assert_eq!(caso.motivo, None, "caso {}", caso.caso);
        }
    }

    #[test]
    fn detecta_item_con_juez_cuenta_solo_la_baja_de_dos_niveles() {
        // Punto 7: en Arquitectura, una baja de 1 nivel no cuenta. Una baja de 2 sí. Sin baja, no.
        let base = arquitectura_con_item(3);
        let item_de = |veredicto: Veredicto| -> Option<bool> {
            let mut casos = vec![Caso::nuevo(1, None, veredicto)];
            finalizar_casos(
                &base,
                Dimension::Arquitectura,
                &[ITEM_ARQUITECTURA],
                &[],
                &mut casos,
            );
            casos[0].detecta_en(ITEM_ARQUITECTURA)
        };
        assert_eq!(item_de(arquitectura_con_item(2)), Some(false), "baja 1");
        assert_eq!(item_de(arquitectura_con_item(1)), Some(true), "baja 2");
        assert_eq!(item_de(arquitectura_con_item(3)), Some(false), "sin baja");
    }

    #[test]
    fn detecta_item_con_juez_limpio_sin_nivel_del_item_no_es_falsa_alarma() {
        // Punto 7: un limpio sin nivel del ítem en Arquitectura no cuenta como falsa alarma.
        let base = arquitectura_con_item(3);
        let sin_item = Veredicto {
            puntajes: vec![puntaje(Dimension::Arquitectura, 1.0)],
            ..Veredicto::default()
        };
        let mut casos = vec![Caso::nuevo(1, None, sin_item)];
        finalizar_casos(
            &base,
            Dimension::Arquitectura,
            &[ITEM_ARQUITECTURA],
            &[],
            &mut casos,
        );
        assert_eq!(casos[0].detecta_en(ITEM_ARQUITECTURA), Some(false));
    }

    #[test]
    fn detecta_item_sin_juez_baja_de_un_nivel_sigue_contando() {
        // Control: Seguridad no usa la regla con juez. Una baja de 1 nivel cuenta.
        let base = veredicto_base();
        let mut baja = base.clone();
        baja.puntajes[0].items[0].nivel = 2;
        let mut casos = vec![Caso::nuevo(1, None, baja)];
        finalizar_casos(
            &base,
            Dimension::Seguridad,
            &["seguridad.secretos"],
            &[],
            &mut casos,
        );
        assert_eq!(casos[0].detecta_en("seguridad.secretos"), Some(true));
    }

    /// Caso con defecto en `item`. Sin inyección: solo sirve a `finalizar_casos` y a `fila_de`.
    fn caso_defecto_en(item: &'static str, veredicto: Veredicto) -> Caso {
        Caso {
            es_defecto: true,
            item_defecto: Some(item),
            ..Caso::nuevo(1, None, veredicto)
        }
    }

    #[test]
    fn piso_defecto_en_diseno_con_nivel_base_0_guarda_true() {
        // Diseño usa juez: el ítem en 0 no puede bajar 2 niveles. Está en piso.
        let base = diseno_con(&[(ITEM_MODULARIDAD, 0), (ITEM_ROBUSTEZ, 3)]);
        let mut casos = vec![caso_defecto_en(ITEM_MODULARIDAD, base.clone())];
        finalizar_casos(
            &base,
            Dimension::Diseno,
            &[ITEM_MODULARIDAD, ITEM_ROBUSTEZ],
            &[],
            &mut casos,
        );
        assert!(CasoGuardado::de(&casos[0]).piso);
    }

    #[test]
    fn piso_defecto_en_diseno_con_nivel_base_3_guarda_false() {
        // Con nivel 3, un defecto puede bajarlo 2 niveles. No está en piso.
        let base = diseno_con(&[(ITEM_MODULARIDAD, 3), (ITEM_ROBUSTEZ, 3)]);
        let mut casos = vec![caso_defecto_en(ITEM_MODULARIDAD, base.clone())];
        finalizar_casos(
            &base,
            Dimension::Diseno,
            &[ITEM_MODULARIDAD, ITEM_ROBUSTEZ],
            &[],
            &mut casos,
        );
        assert!(!CasoGuardado::de(&casos[0]).piso);
    }

    #[test]
    fn piso_sin_juez_usa_la_baja_de_un_nivel() {
        // Seguridad no usa juez: la baja es de 1 nivel. Con nivel base 0 está en piso; con nivel base 1, no.
        let piso_de_base = |nivel: u8| -> bool {
            let mut base = veredicto_base();
            base.puntajes[0].items[0].nivel = nivel;
            let mut casos = vec![caso_defecto_en("seguridad.secretos", base.clone())];
            finalizar_casos(
                &base,
                Dimension::Seguridad,
                &["seguridad.secretos"],
                &[],
                &mut casos,
            );
            CasoGuardado::de(&casos[0]).piso
        };
        assert!(piso_de_base(0), "nivel base 0");
        assert!(!piso_de_base(1), "nivel base 1");
    }

    #[test]
    fn piso_sin_nivel_base_del_item_guarda_false() {
        // El ítem del defecto no está en la base: su nivel base no se conoce.
        let base = veredicto_base();
        let mut casos = vec![caso_defecto_en("seguridad.inexistente", base.clone())];
        finalizar_casos(
            &base,
            Dimension::Seguridad,
            &["seguridad.inexistente"],
            &[],
            &mut casos,
        );
        assert!(!CasoGuardado::de(&casos[0]).piso);
    }

    #[test]
    fn omitidos_lista_el_item_estable_que_el_juez_dejo_sin_nivel() {
        // El juez no dio nivel a `secretos` en el caso 1. En el caso 2 sí lo dio: no hay omisión.
        let base = veredicto_base();
        let mut sin_secretos = base.clone();
        sin_secretos.puntajes[0]
            .items
            .retain(|i| i.item != "seguridad.secretos");
        let mut casos = vec![
            Caso::nuevo(1, None, sin_secretos),
            Caso::nuevo(2, None, base.clone()),
        ];
        finalizar_casos(
            &base,
            Dimension::Seguridad,
            &["seguridad.secretos", "seguridad.panico_en_entrada"],
            &[],
            &mut casos,
        );
        assert_eq!(
            CasoGuardado::de(&casos[0]).omitidos,
            vec!["seguridad.secretos".to_string()]
        );
        assert!(CasoGuardado::de(&casos[1]).omitidos.is_empty());
    }

    #[test]
    fn fila_cuenta_defectos_en_piso_y_omisiones_y_el_texto_los_muestra() {
        let (_t, tarea) = tarea_prueba();
        let mut en_piso = Caso::nuevo(1, None, Veredicto::default());
        en_piso.es_defecto = true;
        en_piso.piso = true;
        en_piso.omitidos = vec!["a".into(), "b".into()];
        let mut fuera_de_piso = Caso::nuevo(2, None, Veredicto::default());
        fuera_de_piso.es_defecto = true;
        let mut limpio = Caso::nuevo(3, None, Veredicto::default());
        limpio.omitidos = vec!["a".into()];
        let fila = fila_de(
            &tarea,
            Dimension::Seguridad,
            1,
            &[en_piso, fuera_de_piso, limpio],
            &[],
            0,
            false,
        );
        assert_eq!((fila.defectos_en_piso, fila.omisiones), (1, 3));
        let texto = texto_piso(&fila);
        assert!(texto.contains("defectos en piso 1"), "texto: {texto}");
        assert!(texto.contains("omisiones 3"), "texto: {texto}");
    }

    #[test]
    fn piso_no_cambia_la_sensibilidad_de_un_defecto_detectado_por_cita() {
        // Guarda: un defecto en piso que el juez cita cuenta como detectado. La sensibilidad no cambia.
        let (_t, tarea) = tarea_prueba();
        let mut defecto = Caso::nuevo(1, None, Veredicto::default());
        defecto.es_defecto = true;
        defecto.piso = true;
        defecto.detectado = true;
        defecto.motivo = Some("cita_juez".into());
        let limpio = Caso::nuevo(2, None, Veredicto::default());
        let fila = fila_de(
            &tarea,
            Dimension::Seguridad,
            1,
            &[defecto, limpio],
            &[],
            0,
            false,
        );
        assert_eq!(fila.sensibilidad, Some(1.0));
        assert_eq!(fila.especificidad, Some(1.0));
    }

    #[test]
    fn caso_viejo_sin_piso_ni_omitidos_se_lee_con_cero() {
        // Guarda: un renglón sin los campos nuevos se lee con piso false y omitidos vacío.
        let base = diseno_con(&[(ITEM_MODULARIDAD, 0), (ITEM_ROBUSTEZ, 3)]);
        let mut casos = vec![caso_defecto_en(ITEM_MODULARIDAD, base.clone())];
        finalizar_casos(
            &base,
            Dimension::Diseno,
            &[ITEM_MODULARIDAD, ITEM_ROBUSTEZ],
            &[],
            &mut casos,
        );
        let guardado = CasoGuardado::de(&casos[0]);
        assert!(guardado.piso, "antes de quitar el campo, piso es true");
        let mut vieja = serde_json::to_value(&guardado).expect("serializa");
        let objeto = vieja.as_object_mut().expect("objeto");
        objeto.remove("piso");
        objeto.remove("omitidos");
        let leido: CasoGuardado = serde_json::from_value(vieja).expect("un JSON viejo parsea");
        assert!(!leido.piso);
        assert!(leido.omitidos.is_empty());
    }

    #[test]
    fn fila_vieja_sin_conteos_de_piso_se_lee_con_cero() {
        let fila = fila("memora", Dimension::Seguridad, 1.0, Some(0.95));
        let mut vieja = serde_json::to_value(&fila).expect("serializa");
        let objeto = vieja.as_object_mut().expect("objeto");
        objeto.remove("defectos_en_piso");
        objeto.remove("omisiones");
        let leida: Fila = serde_json::from_value(vieja).expect("un JSON viejo parsea");
        assert_eq!((leida.defectos_en_piso, leida.omisiones), (0, 0));
    }

    #[test]
    fn calibra_limpio_tras_defecto_no_aplicable_usa_el_item_sin_lugar() {
        // Punto 3: el defecto 0 no está en `casos` (no era aplicable). El limpio 1 toma su ítem y no tiene lugar:
        // solo cuentan las sondas y la baja de 2 niveles. No panica.
        let base = arquitectura_con_item(3);
        let evaluar = |veredicto: Veredicto| -> CasoGuardado {
            let mut casos = vec![Caso::nuevo(1, None, veredicto)];
            finalizar_casos(
                &base,
                Dimension::Arquitectura,
                &[ITEM_ARQUITECTURA],
                &[],
                &mut casos,
            );
            CasoGuardado::de(&casos[0])
        };
        let baja_2 = evaluar(arquitectura_con_item(1));
        assert_eq!(
            baja_2.motivo.as_deref(),
            Some("item_baja_2:arquitectura.dependencias")
        );
        assert_eq!(evaluar(arquitectura_con_item(2)).motivo, None);
        assert_eq!(
            evaluar(con_hallazgo_de_sonda(&base, Dimension::Arquitectura))
                .motivo
                .as_deref(),
            Some("hallazgo_sonda")
        );
        let mut con_cita = arquitectura_con_item(3);
        con_cita.hallazgos.push(hallazgo_del_juez(
            Dimension::Arquitectura,
            "cita",
            "src/comandos.rs",
            10,
        ));
        assert_eq!(evaluar(con_cita).motivo, None);
    }

    #[test]
    fn calibra_seguridad_limpio_con_cita_del_juez_en_el_lugar_del_defecto_no_detecta() {
        // Guarda: Seguridad no usa la regla con juez. Una cita del juez con id de la base, en el lugar del
        // defecto anterior, no cuenta.
        let previo = hallazgo_del_juez(Dimension::Seguridad, "previo", "src/lib.rs", 5);
        let base = Veredicto {
            hallazgos: vec![previo.clone()],
            ..veredicto_base()
        };
        let inyeccion = Inyeccion {
            defecto: Defecto::SecretoEnCodigo,
            archivo: "src/lib.rs".into(),
            linea: 5,
            mutante_archivo: None,
            mutante: None,
            tests_apagados: vec![],
        };
        let mut casos = vec![
            Caso::nuevo(0, Some(inyeccion), base.clone()),
            Caso::nuevo(1, None, base.clone()),
        ];
        finalizar_casos(
            &base,
            Dimension::Seguridad,
            &["seguridad.secretos"],
            &[],
            &mut casos,
        );
        assert_eq!(casos[1].motivo, None);
        assert!(!casos[1].detectado);
    }

    /// Base de Completitud en 1, sin tests malos.
    fn base_completitud() -> Veredicto {
        Veredicto {
            puntajes: vec![puntaje(Dimension::Completitud, 1.0)],
            ..Veredicto::default()
        }
    }

    /// Inyección de `ModuloBorrado` (Completitud) en `src/lib.rs:5`.
    fn inyeccion_modulo() -> Inyeccion {
        Inyeccion {
            defecto: Defecto::ModuloBorrado,
            archivo: "src/lib.rs".into(),
            linea: 5,
            mutante_archivo: None,
            mutante: None,
            tests_apagados: vec![],
        }
    }

    #[test]
    fn calibra_completitud_test_estable_que_se_mezcla_en_un_limpio_no_detecta() {
        let base = base_completitud();
        let limpio = Veredicto {
            puntajes: vec![puntaje(Dimension::Completitud, 0.5)],
            inestables: vec!["t_mix".into()],
            ..Veredicto::default()
        };
        let mut casos = vec![Caso::nuevo(1, None, limpio)];
        finalizar_casos(&base, Dimension::Completitud, &[], &[], &mut casos);
        assert_eq!(
            casos[0].motivo, None,
            "un test ruidoso no detecta en un limpio"
        );
    }

    #[test]
    fn calibra_completitud_test_estable_que_falla_solo_en_el_defecto_detecta() {
        let base = base_completitud();
        let defecto = Veredicto {
            puntajes: vec![puntaje(Dimension::Completitud, 0.5)],
            ocultos_fallidos: vec!["t_real".into()],
            ..Veredicto::default()
        };
        let mut casos = vec![
            Caso::nuevo(0, Some(inyeccion_modulo()), defecto),
            Caso::nuevo(1, None, base.clone()),
        ];
        finalizar_casos(&base, Dimension::Completitud, &[], &[], &mut casos);
        assert!(
            casos[0].detectado,
            "un test estable nuevo que falla sí detecta"
        );
        assert!(!casos[1].detectado, "el limpio sigue sin detectar");
    }

    #[test]
    fn calibra_completitud_test_ruidoso_que_falla_en_el_defecto_no_detecta_por_ese_test() {
        let base = base_completitud();
        let limpio = Veredicto {
            puntajes: vec![puntaje(Dimension::Completitud, 1.0)],
            inestables: vec!["t_mix".into()],
            ..Veredicto::default()
        };
        let defecto = Veredicto {
            puntajes: vec![puntaje(Dimension::Completitud, 0.5)],
            ocultos_fallidos: vec!["t_mix".into()],
            ..Veredicto::default()
        };
        let mut casos = vec![
            Caso::nuevo(0, Some(inyeccion_modulo()), defecto),
            Caso::nuevo(1, None, limpio),
        ];
        finalizar_casos(&base, Dimension::Completitud, &[], &[], &mut casos);
        assert!(
            !casos[0].detectado,
            "el test ruidoso no cuenta en el defecto"
        );
    }

    #[test]
    fn calibra_completitud_contrato_ruidoso_que_falla_en_el_defecto_no_detecta_por_ese_test() {
        let base = base_completitud();
        let limpio = Veredicto {
            puntajes: vec![puntaje(Dimension::Completitud, 1.0)],
            inestables_contrato: vec!["t_mix".into()],
            ..Veredicto::default()
        };
        let defecto = Veredicto {
            puntajes: vec![puntaje(Dimension::Completitud, 0.5)],
            contrato_fallidos: vec!["t_mix".into()],
            ..Veredicto::default()
        };
        let mut casos = vec![
            Caso::nuevo(0, Some(inyeccion_modulo()), defecto),
            Caso::nuevo(1, None, limpio),
        ];
        finalizar_casos(&base, Dimension::Completitud, &[], &[], &mut casos);
        assert!(
            !casos[0].detectado,
            "el contrato ruidoso no cuenta en el defecto"
        );
    }

    #[test]
    fn calibra_seguridad_test_ruidoso_sigue_contando() {
        let base = Veredicto {
            puntajes: vec![puntaje(Dimension::Seguridad, 1.0)],
            ..Veredicto::default()
        };
        let limpio = Veredicto {
            puntajes: vec![puntaje(Dimension::Seguridad, 1.0)],
            inestables: vec!["t_mix".into()],
            ..Veredicto::default()
        };
        let defecto = Veredicto {
            puntajes: vec![puntaje(Dimension::Seguridad, 0.5)],
            ocultos_fallidos: vec!["t_mix".into()],
            ..Veredicto::default()
        };
        let inyeccion = Inyeccion {
            defecto: Defecto::SecretoEnCodigo,
            archivo: "src/lib.rs".into(),
            linea: 5,
            mutante_archivo: None,
            mutante: None,
            tests_apagados: vec![],
        };
        let mut casos = vec![
            Caso::nuevo(0, Some(inyeccion), defecto),
            Caso::nuevo(1, None, limpio),
        ];
        finalizar_casos(&base, Dimension::Seguridad, &[], &[], &mut casos);
        assert!(
            casos[0].detectado,
            "Seguridad no aplica la regla de ruidosos"
        );
    }

    /// Mutante que la base mata: `archivo` y `nombre` del mutante.
    fn muerto_de_base(archivo: &str, nombre: &str) -> MutanteMuerto {
        MutanteMuerto {
            archivo: archivo.into(),
            linea: 10,
            nombre: nombre.into(),
            tests: vec![],
        }
    }

    /// Hallazgo de Pruebas de un mutante vivo en el caso: `archivo` del mutante y `cita` con su nombre.
    fn mutante_vivo(archivo: &str, nombre: &str) -> Hallazgo {
        Hallazgo {
            dimension: Dimension::Pruebas,
            severidad: 2,
            archivo: archivo.into(),
            linea: 10,
            cita: nombre.into(),
            detalle: String::new(),
            fuente: Fuente::Sonda("mutacion".into()),
            id: format!("vivo-{archivo}-{nombre}"),
            impacto: vec![],
        }
    }

    /// Veredicto de Pruebas con `valor` y los hallazgos de mutantes vivos `vivos`.
    fn pruebas_con(valor: f32, vivos: Vec<Hallazgo>) -> Veredicto {
        Veredicto {
            puntajes: vec![puntaje(Dimension::Pruebas, valor)],
            hallazgos: vivos,
            ..Veredicto::default()
        }
    }

    /// Caso con `AssertBorrado` aplicado a `src/lib.rs:10`, con el veredicto `veredicto`.
    fn caso_assert_borrado(indice: u32, veredicto: Veredicto) -> Caso {
        let inyeccion = Inyeccion {
            defecto: Defecto::AssertBorrado,
            archivo: "src/lib.rs".into(),
            linea: 10,
            mutante_archivo: Some("src/lib.rs".into()),
            mutante: None,
            tests_apagados: vec![],
        };
        Caso::nuevo(indice, Some(inyeccion), veredicto)
    }

    #[test]
    fn assert_borrado_valor_menor_sin_mutante_vivo_de_la_base_no_detecta() {
        let base = pruebas_con(1.0, vec![]);
        let muertos = vec![muerto_de_base("src/lib.rs", "reemplaza suma -> 0")];
        let mut casos = vec![caso_assert_borrado(0, pruebas_con(0.5, vec![]))];
        finalizar_casos(&base, Dimension::Pruebas, &[], &muertos, &mut casos);
        assert!(
            !casos[0].detectado,
            "valor menor no cuenta sin mutante vivo"
        );
        assert_eq!(casos[0].motivo, None);
    }

    #[test]
    fn assert_borrado_mutante_vivo_fuera_de_los_muertos_de_la_base_no_detecta() {
        let base = pruebas_con(1.0, vec![]);
        let muertos = vec![muerto_de_base("src/lib.rs", "reemplaza suma -> 0")];
        let vivo = mutante_vivo("src/otro.rs", "reemplaza resta -> 0");
        let mut casos = vec![caso_assert_borrado(0, pruebas_con(1.0, vec![vivo]))];
        finalizar_casos(&base, Dimension::Pruebas, &[], &muertos, &mut casos);
        assert!(
            !casos[0].detectado,
            "el mutante vivo no era muerto en la base"
        );
        assert_eq!(casos[0].motivo, None);
    }

    #[test]
    fn assert_borrado_mutante_muerto_en_la_base_vivo_detecta_con_su_nombre() {
        let base = pruebas_con(0.5, vec![]);
        let muertos = vec![muerto_de_base("src/lib.rs", "reemplaza suma -> 0")];
        let vivo = mutante_vivo("src/lib.rs", "reemplaza suma -> 0");
        let mut casos = vec![caso_assert_borrado(0, pruebas_con(0.9, vec![vivo]))];
        finalizar_casos(&base, Dimension::Pruebas, &[], &muertos, &mut casos);
        assert!(
            casos[0].detectado,
            "el mutante muerto en la base vive en el caso"
        );
        assert_eq!(
            casos[0].motivo.as_deref(),
            Some("mutante_vivo:reemplaza suma -> 0")
        );
    }

    /// Guarda: el mutante tiene un test unitario y uno de socket, y el caso no lo cita vivo. Sigue muerto, no detecta.
    #[test]
    fn assert_borrado_mutante_que_sigue_muerto_con_dos_tests_no_detecta() {
        let base = pruebas_con(1.0, vec![]);
        let muertos = vec![MutanteMuerto {
            archivo: "src/db.rs".into(),
            linea: 10,
            nombre: "reemplaza zset -> 0".into(),
            tests: vec!["db::tests::zset".into(), "red::zset".into()],
        }];
        let mut casos = vec![caso_assert_borrado(0, pruebas_con(0.5, vec![]))];
        finalizar_casos(&base, Dimension::Pruebas, &[], &muertos, &mut casos);
        assert!(!casos[0].detectado, "el mutante sigue muerto: no cuenta");
        assert_eq!(casos[0].motivo, None);
    }

    #[test]
    fn assert_borrado_normaliza_el_punto_inicial_del_archivo() {
        let base = pruebas_con(1.0, vec![]);
        let muertos = vec![muerto_de_base("src/lib.rs", "reemplaza suma -> 0")];
        let vivo = mutante_vivo("./src/lib.rs", "reemplaza suma -> 0");
        let mut casos = vec![caso_assert_borrado(0, pruebas_con(1.0, vec![vivo]))];
        finalizar_casos(&base, Dimension::Pruebas, &[], &muertos, &mut casos);
        assert_eq!(
            casos[0].motivo.as_deref(),
            Some("mutante_vivo:reemplaza suma -> 0")
        );
    }

    #[test]
    fn calibra_completitud_test_que_se_mezcla_en_un_limpio_no_detecta_limpios() {
        let (_t, tarea) = tarea_prueba();
        let base = base_completitud();
        // Un hilo: los limpios salen en orden de índice. El primero se mezcla; los demás quedan como la base.
        let limpios_vistos = AtomicUsize::new(0);
        let medida = medir(
            &tarea,
            Dimension::Completitud,
            Path::new(FIXTURE),
            &base,
            |limpia, _, base| {
                if !limpia {
                    return Veredicto {
                        puntajes: vec![puntaje(Dimension::Completitud, 0.5)],
                        ocultos_fallidos: vec!["t_real".into()],
                        ..Veredicto::default()
                    };
                }
                if limpios_vistos.fetch_add(1, Ordering::SeqCst) == 0 {
                    return Veredicto {
                        puntajes: vec![puntaje(Dimension::Completitud, 0.5)],
                        inestables: vec!["t_mix".into()],
                        ..Veredicto::default()
                    };
                }
                base.clone()
            },
            4,
        );
        let limpios = limpios_guardados(&medida);
        assert!(!limpios.is_empty(), "hay casos limpios");
        for caso in &limpios {
            assert!(
                !caso.detectado,
                "caso {}: un test ruidoso no detecta",
                caso.caso
            );
        }
        assert!(
            medida.casos.iter().any(|c| c.es_defecto && c.detectado),
            "un defecto que falla un test distinto sí detecta"
        );
    }

    #[test]
    fn calibra_inyeccion_invalida_agotada_guarda_el_motivo_del_no_aplicable() {
        let (_t, tarea) = tarea_prueba();
        let base = veredicto_de(Dimension::Estructura, 1.0, None);
        let medida = medir(
            &tarea,
            Dimension::Estructura,
            Path::new(FIXTURE),
            &base,
            |limpia, _, base| {
                if limpia {
                    base.clone()
                } else {
                    con_motivo(base, NO_COMPILA)
                }
            },
            2,
        );
        assert_eq!(medida.no_aplicables.len(), 1);
        assert_eq!(medida.no_aplicables[0].indice, 0);
        assert_eq!(
            medida.no_aplicables[0].motivo,
            "inyecciones inválidas agotadas"
        );
    }

    #[test]
    fn calibra_el_tope_se_revisa_antes_de_cada_reemplazo() {
        // La base no cuesta. La primera inyección es inválida, y su juez deja el límite rechazado: el
        // reemplazo no corre y la calibración se detiene.
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
        let limite = LimiteCalibracion::nuevo(1.5, 0.0);
        let llamadas = AtomicUsize::new(0);
        let evaluar = |ruta: &Path, _: TipoCaso, _: Target| {
            // La llamada 1 es la base; la 2, la primera inyección.
            if llamadas.fetch_add(1, Ordering::SeqCst) + 1 == 2 {
                limite.anota(RechazoLimite {
                    gasto_usd: 1.0,
                    cota_usd: 1.0,
                });
            }
            ev(ruta)
        };
        let tope = Tope {
            limite: limite.clone(),
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
            None,
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
            fila.especificidad, None,
            "un limpio sin veredicto no mide especificidad"
        );
        assert_eq!(fila.limpios_sin_veredicto, 1);
    }

    // ---- Tarea 48, parte 2, punto 3: el juez en paralelo, de a 4 ----

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

    /// Modelo de juez de prueba, con precio propio. Su URL es un puerto cerrado: si el límite dejara
    /// pasar un intento, la llamada fallaría por red y no saldría a internet.
    const MODELO_JUEZ_LIMITE: &str = "modelo-juez-limite";

    /// Pide un juicio al juez de prueba con el límite activo del hilo. Si el límite rechaza el intento, no se
    /// envía y la llamada da error. Sin límite, el intento iría a la red y fallaría al conectar.
    fn pedir_juicio_limitado(home: &Path) -> Result<(), crate::proveedores::ErrorProveedor> {
        use crate::proveedores::{Breaker, Clave, Claves, Contexto, Plazos, Prompt, llamar_con};
        use crate::tipos::{Proveedor, Rol, Via};
        let mut cfg = Config::default();
        cfg.proveedores.deepseek_url = "http://127.0.0.1:9".into();
        cfg.precios.insert(
            MODELO_JUEZ_LIMITE.into(),
            Precio {
                usd_entrada: 1.74,
                usd_salida: 3.48,
                umbral_entrada: None,
                usd_entrada_alto: None,
                usd_salida_alto: None,
            },
        );
        let eleccion = Eleccion {
            via: Via::Api,
            proveedor: Some(Proveedor::Deepseek),
            modelo: Some(MODELO_JUEZ_LIMITE.into()),
            razonamiento: true,
            motivo: "prueba".into(),
            rol: Some(Rol::Juzga),
        };
        // Cota: 4 bytes de entrada y 32 000 de salida, unos 0,11 USD.
        let prompt = Prompt {
            sistema: None,
            usuario: "hola".into(),
            max_tokens: 32_000,
            fase: "fase-prueba".into(),
            reintento_juez: false,
        };
        let claves = Claves {
            deepseek: Some(Clave::nueva("clave-falsa-de-prueba")),
            anthropic: None,
        };
        let breaker = Breaker::new();
        let reloj = Instant::now;
        let ctx = Contexto {
            cfg: &cfg,
            home,
            breaker: &breaker,
            claves: &claves,
            reloj: &reloj,
            plazos: Plazos::default(),
        };
        llamar_con(&ctx, &eleccion, &prompt, "c-calibra-limite").map(|_| ())
    }

    #[test]
    fn calibra_juez_rechazado_por_el_limite_deja_la_detencion_y_el_caso_fuera() {
        let home = tempfile::tempdir().expect("home");
        let base = veredicto_base();
        let (_t, tarea) = tarea_prueba();
        let falso_de_seguridad = falso(Path::new(FIXTURE), &base, |limpia, _, base| {
            if limpia { base.clone() } else { hundido(base) }
        });
        // Cada copia pide un juicio con el límite activo. Si el límite lo rechaza, el veredicto queda sin
        // juicio, como lo deja `juzgar`. La base no pide juicio.
        let evaluar = |ruta: &Path, tipo: TipoCaso, _: Target| -> Result<Veredicto, ErrorOraculo> {
            let veredicto = falso_de_seguridad(ruta)?;
            if tipo == TipoCaso::Base {
                return Ok(veredicto);
            }
            Ok(if pedir_juicio_limitado(home.path()).is_err() {
                sin(&veredicto)
            } else {
                veredicto
            })
        };
        let limite = LimiteCalibracion::nuevo(0.05, 0.0);
        let tope = Tope {
            limite: limite.clone(),
        };
        let medida = calibrar_medida_con_hilos(
            &tarea,
            Dimension::Seguridad,
            4,
            42,
            Path::new(FIXTURE),
            &evaluar,
            Some(&tope),
            &Vec::new,
            &|| None,
            CASOS_EN_PARALELO,
            None,
        )
        .expect("calibra");
        assert!(medida.detencion.is_some(), "el límite detiene la medida");
        assert!(
            medida.casos.is_empty(),
            "el caso del juez rechazado no es un caso medido"
        );
        assert!(medida.fila.is_none(), "sin casos medidos no hay fila");
        assert!(limite.rechazo().is_some(), "el límite anota el rechazo");
    }

    #[test]
    fn calibra_tras_un_rechazo_no_se_lanza_otro_caso() {
        let home = tempfile::tempdir().expect("home");
        let base = veredicto_base();
        let (_t, tarea) = tarea_prueba();
        let falso_de_seguridad = falso(Path::new(FIXTURE), &base, |limpia, _, base| {
            if limpia { base.clone() } else { hundido(base) }
        });
        let copias = AtomicUsize::new(0);
        let evaluar = |ruta: &Path, tipo: TipoCaso, _: Target| -> Result<Veredicto, ErrorOraculo> {
            let veredicto = falso_de_seguridad(ruta)?;
            if tipo == TipoCaso::Base {
                return Ok(veredicto);
            }
            copias.fetch_add(1, Ordering::SeqCst);
            Ok(if pedir_juicio_limitado(home.path()).is_err() {
                sin(&veredicto)
            } else {
                veredicto
            })
        };
        let limite = LimiteCalibracion::nuevo(0.05, 0.0);
        let tope = Tope { limite };
        // Un caso a la vez: tras el primer rechazo, el coordinador no lanza el siguiente.
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
            1,
            None,
        )
        .expect("calibra");
        assert_eq!(copias.load(Ordering::SeqCst), 1, "un solo caso corre");
        assert!(medida.detencion.is_some(), "el rechazo detiene la medida");
        assert!(medida.casos.is_empty(), "el caso rechazado no se mide");
    }

    #[test]
    fn calibra_tope_sin_casos_medidos_nombra_ninguno_en_el_error() {
        // La base pide un juicio y el límite de 0,05 USD lo rechaza: no hay casos, ni archivo de casos.
        let home = tempfile::tempdir().expect("home");
        let carpeta = home.path().join("oraculos").join("calibra-fixture");
        fs::create_dir_all(&carpeta).expect("carpeta");
        let repo = carpeta.join("repo-bueno");
        fs::create_dir_all(&repo).expect("repo");
        super::inyectar::pruebas::copiar(Path::new(FIXTURE), &repo);
        let _ = tarea_en(&carpeta, "\n[calibracion]\nrepo = \"repo-bueno\"\n");
        let presupuesto = Presupuesto { usd: 0.05 };
        let evaluador = |_: &Tarea, _: &Path, _: &OpcionesOraculo| {
            let _ = pedir_juicio_limitado(home.path());
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
        assert!(texto.contains("Casos medidos: 0"), "{texto}");
        assert!(texto.contains("Archivo de casos: ninguno"), "{texto}");
        assert!(texto.contains("cuesta como máximo"), "{texto}");
        assert!(
            !home.path().join(CARPETA_CASOS).exists(),
            "sin casos no hay carpeta de casos"
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
                None,
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
            &uno.no_aplicables,
        )
        .expect("escribe")
        .expect("hay casos");
        let ruta_cuatro = escribir_casos(
            home_cuatro.path(),
            &tarea.nombre,
            Dimension::Seguridad,
            &fila_cuatro,
            &cuatro.casos,
            &cuatro.no_aplicables,
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
            None,
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
            None,
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
            None,
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

    /// Punto 3: un renglón de antes de `mutante` y `tests_apagados` se lee con los dos campos vacíos.
    #[test]
    fn calibra_renglon_viejo_sin_mutante_se_lee_con_campos_vacios() {
        let viejo = r#"{"caso":0,"es_defecto":true,"detectado":false,"defecto":"assert_borrado"}"#;
        let caso: CasoGuardado = serde_json::from_str(viejo).expect("renglón de antes del mutante");
        assert_eq!(caso.mutante, None);
        assert!(caso.tests_apagados.is_empty());
    }

    /// Punto 2: un renglón de antes de `valor` y `valor_base` se lee con los dos en `None`.
    #[test]
    fn calibra_renglon_viejo_sin_valor_se_lee_con_none() {
        let viejo = r#"{"caso":0,"es_defecto":true,"detectado":false,"defecto":"assert_borrado"}"#;
        let caso: CasoGuardado = serde_json::from_str(viejo).expect("renglón sin valor");
        assert_eq!(caso.valor, None);
        assert_eq!(caso.valor_base, None);
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

    /// Un caso con defecto guarda los extractos que leyó el juez. El rango de uno cubre la línea del defecto.
    #[test]
    fn caso_con_defecto_guarda_sus_extractos_y_el_rango_cubre_la_linea() {
        let base = veredicto_de(Dimension::Seguridad, 0.9, None);
        let mut caso = Caso::nuevo(1, Some(inyeccion_modulo()), base);
        caso.extractos = vec![ExtractoGuardado {
            archivo: "src/lib.rs".into(),
            desde: 1,
            hasta: 10,
            motivo: "cambio".into(),
        }];
        let guardado = CasoGuardado::de(&caso);
        assert_eq!(guardado.extractos.len(), 1, "{:?}", guardado.extractos);
        let linea = guardado.linea.expect("el defecto tiene línea");
        let extracto = &guardado.extractos[0];
        assert_eq!(extracto.archivo, "src/lib.rs");
        assert!((extracto.desde..=extracto.hasta).contains(&linea));
    }

    /// Los extractos llegan al archivo `.casos.jsonl` y vuelven a leerse igual.
    #[test]
    fn escribir_casos_guarda_los_extractos_en_el_renglon() {
        let base = veredicto_de(Dimension::Seguridad, 0.9, None);
        let mut caso = Caso::nuevo(1, Some(inyeccion_modulo()), base);
        let esperados = vec![ExtractoGuardado {
            archivo: "src/lib.rs".into(),
            desde: 1,
            hasta: 10,
            motivo: "cambio".into(),
        }];
        caso.extractos = esperados.clone();
        let home = tempfile::tempdir().expect("home");
        let fila = fila("t", Dimension::Seguridad, 1.0, None);
        let ruta = escribir_casos(home.path(), "t", Dimension::Seguridad, &fila, &[caso], &[])
            .expect("escribe")
            .expect("hay casos");
        let texto = fs::read_to_string(ruta).expect("lee");
        let leido: CasoGuardado = serde_json::from_str(texto.trim()).expect("caso");
        assert_eq!(leido.extractos, esperados);
    }

    /// Un renglón de `.casos.jsonl` sin el campo `extractos` se lee con lista vacía.
    #[test]
    fn caso_guardado_viejo_sin_extractos_lee_lista_vacia() {
        let viejo = r#"{"caso":3,"es_defecto":true,"detectado":true}"#;
        let caso: CasoGuardado = serde_json::from_str(viejo).expect("renglón de antes");
        assert!(caso.extractos.is_empty(), "{:?}", caso.extractos);
    }

    /// Un caso con defecto toma los extractos de su veredicto. El rango del archivo del defecto cubre su línea.
    #[test]
    fn caso_con_defecto_toma_los_extractos_de_su_veredicto() {
        let base = veredicto_de(Dimension::Estructura, 1.0, None);
        let evaluador = |ruta: &Path, _: TipoCaso, _: Target| {
            let extractos = arbol(ruta)
                .into_keys()
                .map(|archivo| ExtractoGuardado {
                    archivo,
                    desde: 1,
                    hasta: u32::MAX,
                    motivo: "cambio".into(),
                })
                .collect();
            Ok(Veredicto {
                extractos,
                ..base.clone()
            })
        };
        let (_t, tarea) = tarea_prueba();
        let medida = calibrar_medida_con_hilos(
            &tarea,
            Dimension::Estructura,
            2,
            42,
            Path::new(FIXTURE),
            &evaluador,
            None,
            &Vec::new,
            &|| None,
            1,
            None,
        )
        .expect("calibra");
        let caso = medida
            .casos
            .iter()
            .find(|c| c.es_defecto)
            .expect("hay un caso con defecto");
        let guardado = CasoGuardado::de(caso);
        assert!(!guardado.extractos.is_empty(), "{:?}", guardado.extractos);
        let linea = guardado.linea.expect("el defecto tiene línea");
        let archivo = guardado.archivo.clone().expect("el defecto tiene archivo");
        let extracto = guardado
            .extractos
            .iter()
            .find(|e| e.archivo == archivo)
            .expect("hay un extracto del archivo del defecto");
        assert!((extracto.desde..=extracto.hasta).contains(&linea));
    }

    /// La base de una dimensión con juez guarda un extracto por cada uno de sus tres juicios.
    #[test]
    fn base_con_juez_guarda_una_lista_de_extractos_por_juicio() {
        let mut base = veredicto_de(Dimension::Estructura, 1.0, None);
        base.puntajes[0].items = vec![NivelItem {
            item: "estructura.legibilidad".into(),
            nivel: 2,
            evidencia: vec![],
            fuente: Fuente::Juez {
                modelo: "deepseek-reasoner".into(),
            },
        }];
        base.extractos = vec![ExtractoGuardado {
            archivo: "src/lib.rs".into(),
            desde: 1,
            hasta: 10,
            motivo: "grande".into(),
        }];
        let evaluador = |_: &Path, _: TipoCaso, _: Target| Ok(base.clone());
        let (_t, tarea) = tarea_prueba();
        let medida = calibrar_medida_con_hilos(
            &tarea,
            Dimension::Estructura,
            2,
            42,
            Path::new(FIXTURE),
            &evaluador,
            None,
            &Vec::new,
            &|| None,
            1,
            None,
        )
        .expect("calibra");
        let fila = medida.fila.expect("hay fila");
        assert_eq!(fila.extractos_base.len(), 3, "{:?}", fila.extractos_base);
        assert!(fila.extractos_base.iter().all(|e| e.len() == 1));
    }

    /// Una fila de antes de `extractos_base` se lee con lista vacía.
    #[test]
    fn fila_vieja_sin_extractos_base_lee_lista_vacia() {
        let mut texto =
            serde_json::to_value(fila("t", Dimension::Estructura, 1.0, None)).expect("json");
        texto
            .as_object_mut()
            .expect("objeto")
            .remove("extractos_base");
        let leida: Fila = serde_json::from_value(texto).expect("fila vieja");
        assert!(
            leida.extractos_base.is_empty(),
            "{:?}",
            leida.extractos_base
        );
    }

    // Punto 3: calibración de punta a punta con el evaluador real (`evaluar_con`), sin podman ni contenedores.

    /// Juez falso `Sync`: responde `texto` y cuenta sus llamadas.
    struct JuezSync {
        texto: String,
        llamadas: AtomicU32,
    }

    impl crate::oracle::juez::ModeloJuez for JuezSync {
        fn consultar(
            &self,
            _prompt: &crate::proveedores::Prompt,
        ) -> Result<crate::proveedores::RespuestaJuez, ErrorOraculo> {
            self.llamadas.fetch_add(1, Ordering::SeqCst);
            Ok(crate::proveedores::RespuestaJuez {
                texto: self.texto.clone(),
                truncada: false,
                costo_usd: 0.0,
            })
        }
    }

    /// Respuesta del juez para todos sus ítems en `rubricas`, con evidencia que cita `src/lib.rs`.
    fn texto_juez(rubricas: &[crate::oracle::rubrica::Rubrica]) -> String {
        let items: Vec<serde_json::Value> = rubricas
            .iter()
            .flat_map(|r| r.item.iter())
            .filter(|i| {
                i.mide != crate::oracle::rubrica::Mide::Sonda
                    && crate::oracle::juez::dimension_de(&i.id).is_some()
            })
            .map(|i| {
                json!({
                    "item": i.id,
                    "nivel": 2,
                    "evidencia": [{"archivo": "src/lib.rs", "linea": 2, "cita": "pub mod protocolo;"}],
                })
            })
            .collect();
        json!({"items": items, "hallazgos": []}).to_string()
    }

    /// Ejecutor de herramientas falso de una imagen sana. Responde las sondas y la mutación que muere.
    struct HerramientasSanas;

    impl EjecutorHerramienta for HerramientasSanas {
        fn ejecutar(&self, cmd: &str) -> Result<SalidaHerramienta, ErrorOraculo> {
            let mut salida = SalidaHerramienta::default();
            if cmd.contains("gitleaks detect") {
                salida.archivos.insert("gitleaks.json".into(), "[]".into());
                salida.stderr = "no leaks found".into();
            } else if cmd.contains("cargo audit") {
                salida.archivos.insert(
                    "audit.json".into(),
                    r#"{"vulnerabilities":{"found":false,"list":[]}}"#.into(),
                );
            } else if cmd.contains("--message-format=json") {
                salida.archivos.insert(
                    "clippy.json".into(),
                    r#"{"reason":"build-finished","success":true}"#.into(),
                );
            } else if cmd.contains("cargo mutants") && cmd.contains("--list") {
                salida
                    .archivos
                    .insert("listado.json".into(), r#"[{"name":"m"}]"#.into());
            } else if cmd.contains("cargo mutants") {
                salida
                    .archivos
                    .insert(ARCHIVO_OUTCOMES.into(), OUTCOMES_CON_MUERTO.into());
                salida.archivos.insert(
                    crate::oracle::mutacion::ARCHIVO_TESTS_FALLIDOS.into(),
                    TESTS_FALLIDOS_DE_PROTOCOLO.into(),
                );
            }
            Ok(salida)
        }

        fn tiene_rustsec(&self) -> bool {
            true
        }
    }

    /// Podman que no debe usarse: el repo de la prueba no tiene Dockerfile, así que no se construye imagen.
    struct PodmanSinUso;

    impl crate::oracle::entrega::Podman for PodmanSinUso {
        fn correr(
            &self,
            args: &[&str],
            _plazo: Duration,
        ) -> Result<crate::oracle::contenedor::SalidaPodman, ErrorOraculo> {
            panic!("la calibración no debía llamar a podman: {args:?}");
        }
    }

    /// Fábrica de escenarios que no arranca nada.
    struct FabricaSinContenedoresCal;

    impl crate::oracle::entrega::FabricaEscenarios for FabricaSinContenedoresCal {
        fn iniciar(
            &self,
            _imagen: &str,
            _escenarios: Option<&Path>,
        ) -> Result<Box<dyn crate::oracle::entrega::CorredorDeEscenario + '_>, String> {
            Err("sin contenedor de escenario en esta prueba".into())
        }
    }

    /// Suites: esta prueba no las pide (Arquitectura y Diseño no corren contrato ni oculta). Si corren, falla.
    fn correr_sin_contenedores(
        e: &crate::oracle::contenedor::Ejecucion,
    ) -> Result<crate::oracle::contenedor::Resultado, ErrorOraculo> {
        Err(ErrorOraculo::Infra(format!(
            "la calibración no debía correr suites: {}",
            e.cmd
        )))
    }

    /// Cuentas de una evaluación por tipo de caso: evaluaciones del evaluador y llamadas al juez.
    #[derive(Default)]
    struct Cuenta {
        evaluaciones: AtomicU32,
        juez: AtomicU32,
    }

    /// Cuentas de la calibración: la base, los limpios (alcance `Deterministas`) y los defectos (`SoloMutacion`).
    #[derive(Default)]
    struct Cuentas {
        base: Cuenta,
        limpios: Cuenta,
        defectos: Cuenta,
    }

    /// Evaluador real: `evaluar_con` con juez falso y dependencias sin podman. Cuenta por tipo de caso.
    fn evaluador_real<'a>(
        home: &'a Path,
        cuentas: &'a Cuentas,
    ) -> impl Fn(&Tarea, &Path, &OpcionesOraculo) -> Result<Veredicto, ErrorOraculo> + Sync + 'a
    {
        move |tarea: &Tarea, repo: &Path, o: &OpcionesOraculo| -> Result<Veredicto, ErrorOraculo> {
            let rubricas = crate::oracle::rubrica::cargar_embebidas()
                .map_err(|e| ErrorOraculo::Infra(format!("rúbricas embebidas: {e}")))?;
            let calibracion = crate::oracle::calibra::Calibraciones::cargar(home)?;
            let juez = JuezSync {
                texto: texto_juez(&rubricas),
                llamadas: AtomicU32::new(0),
            };
            let herramientas = |_: &crate::oracle::Preparada| -> Box<dyn EjecutorHerramienta> {
                Box::new(HerramientasSanas)
            };
            let deps = crate::oracle::Dependencias {
                correr: &correr_sin_contenedores,
                herramientas: &herramientas,
                juez: Some(&juez),
                modelo_juez: "juez-falso",
                juez_config: None,
                calibracion: &calibracion,
                rubricas: &rubricas,
                reglas: crate::config::ReglasDecision::default(),
                redactor: None,
                avisos: Vec::new(),
                podman: &PodmanSinUso,
                escenarios: &FabricaSinContenedoresCal,
            };
            let con_home = OpcionesOraculo {
                home: Some(home.to_path_buf()),
                ..o.clone()
            };
            let veredicto = crate::oracle::evaluar_con(repo, tarea, &con_home, deps)?;
            let deterministas = o
                .reuso
                .as_ref()
                .is_some_and(|r| r.alcance == Alcance::Deterministas);
            // Una repetición de la base no cuenta: no es un caso ni la base única.
            let cuenta = if deterministas && es_copia_de_caso(repo) {
                Some(&cuentas.limpios)
            } else if o.memo.is_some() {
                Some(&cuentas.base)
            } else if deterministas {
                None
            } else {
                Some(&cuentas.defectos)
            };
            if let Some(cuenta) = cuenta {
                cuenta.evaluaciones.fetch_add(1, Ordering::SeqCst);
                cuenta
                    .juez
                    .fetch_add(juez.llamadas.load(Ordering::SeqCst), Ordering::SeqCst);
            }
            Ok(veredicto)
        }
    }

    /// Lock de la fixture: su paquete es `calibra-fixture`, como el `Cargo.toml` de la copia.
    const LOCK_CALIBRA: &str =
        "version = 4\n\n[[package]]\nname = \"calibra-fixture\"\nversion = \"0.1.0\"\n";

    /// Carpeta de la calibración sin Dockerfile y con `Cargo.lock`, y la caché de la descarga en `home`.
    /// Así la descarga no baja nada y Entrega no construye imagen.
    fn carpeta_sin_red(home: &Path) -> PathBuf {
        let carpeta = carpeta_con_repo_bueno(home);
        let repo = carpeta.join("repo-bueno");
        fs::remove_file(repo.join("Dockerfile")).expect("quita el Dockerfile");
        fs::write(repo.join("Cargo.lock"), LOCK_CALIBRA).expect("Cargo.lock");
        let cache = home
            .join("cache/rust")
            .join(crate::oracle::descarga::hash_corto(LOCK_CALIBRA.as_bytes()));
        fs::create_dir_all(&cache).expect("caché de Rust");
        fs::write(cache.join("config.toml"), "").expect("config.toml");
        fs::create_dir_all(home.join("cache/rustsec")).expect("base de RustSec");
        // La tarea de la fixture declara `bootstrap/docs/SPEC.md`: la evaluación real lo lee.
        let spec = carpeta.join("bootstrap/docs");
        fs::create_dir_all(&spec).expect("carpeta del spec");
        fs::write(
            spec.join("SPEC.md"),
            include_str!("../../tests/fixtures/fichas/spec-completa.md"),
        )
        .expect("SPEC.md");
        carpeta
    }

    #[test]
    fn calibra_diseno_tras_arquitectura_con_evaluador_real_no_llama_al_juez_en_limpios() {
        // Punto 3, de punta a punta: Arquitectura y luego Diseño, con la misma semilla y n, y `evaluar_con` real.
        let home = tempfile::tempdir().expect("home");
        carpeta_sin_red(home.path());
        let de_arquitectura = Cuentas::default();
        calibrar_tarea_con(
            home.path(),
            "calibra-fixture",
            "arquitectura",
            4,
            42,
            None,
            None,
            &evaluador_real(home.path(), &de_arquitectura),
        )
        .expect("calibra arquitectura");
        // Guarda: Arquitectura sí llama al juez en sus limpios. Sin esto, el cero de Diseño no probaría nada.
        assert!(
            de_arquitectura.limpios.evaluaciones.load(Ordering::SeqCst) > 0,
            "Arquitectura evalúa limpios"
        );
        assert!(
            de_arquitectura.limpios.juez.load(Ordering::SeqCst) > 0,
            "Arquitectura llama al juez en sus limpios"
        );
        let de_diseno = Cuentas::default();
        let segunda = calibrar_tarea_con(
            home.path(),
            "calibra-fixture",
            "diseno",
            4,
            42,
            None,
            None,
            &evaluador_real(home.path(), &de_diseno),
        )
        .expect("calibra diseño");
        let renglones = renglones_guardados(&segunda);
        let con_defecto = renglones.iter().filter(|c| c.es_defecto).count();
        assert!(
            renglones.iter().any(|c| !c.es_defecto),
            "hay limpios de Diseño"
        );
        // (i) Los limpios de Diseño salen del caché: ni se evalúan ni llaman al juez.
        assert_eq!(
            de_diseno.limpios.evaluaciones.load(Ordering::SeqCst),
            0,
            "los limpios no pasan por el evaluador"
        );
        assert_eq!(
            de_diseno.limpios.juez.load(Ordering::SeqCst),
            0,
            "los limpios no llaman al juez"
        );
        // La base corre una vez y cada caso con defecto corre una vez. Solo esos llaman al juez.
        assert_eq!(
            de_diseno.base.evaluaciones.load(Ordering::SeqCst),
            1,
            "una base"
        );
        assert_eq!(
            de_diseno.defectos.evaluaciones.load(Ordering::SeqCst) as usize,
            con_defecto,
            "un caso con defecto por renglón"
        );
        assert!(
            de_diseno.defectos.juez.load(Ordering::SeqCst) as usize >= con_defecto,
            "cada caso con defecto llama al juez"
        );
        // (ii) La fila de Diseño no cuenta limpios sin veredicto.
        assert_eq!(segunda.fila.limpios_sin_veredicto, 0, "fila de Diseño");
        // (iii) Cada limpio de Diseño tiene veredicto.
        for caso in renglones.iter().filter(|c| !c.es_defecto) {
            assert!(
                caso.sin_veredicto.is_none(),
                "caso {} sin veredicto: {:?}",
                caso.caso,
                caso.sin_veredicto
            );
        }
    }

    // Punto 8: la mutación acotada llega a los comandos cuando la evaluación real corre dentro del ámbito.

    /// Ejecutor de herramientas que anota cada comando y responde como `HerramientasSanas`.
    struct Anotador {
        cmds: std::sync::Arc<Mutex<Vec<String>>>,
    }

    /// Listado de mutantes con un mutante de `src/protocolo.rs`, para que la muestra acotada no quede vacía.
    const LISTADO_DE_PROTOCOLO: &str = r#"[{"name":"src/protocolo.rs:2:1: replace leer_numero -> Result<u32, ParseIntError> with Ok(0)"}]"#;

    impl EjecutorHerramienta for Anotador {
        fn ejecutar(&self, cmd: &str) -> Result<SalidaHerramienta, ErrorOraculo> {
            self.cmds.lock().expect("lock").push(cmd.to_string());
            if cmd.contains("cargo mutants") && cmd.contains("--list") {
                let mut salida = SalidaHerramienta::default();
                salida
                    .archivos
                    .insert("listado.json".into(), LISTADO_DE_PROTOCOLO.into());
                return Ok(salida);
            }
            HerramientasSanas.ejecutar(cmd)
        }

        fn tiene_rustsec(&self) -> bool {
            true
        }
    }

    /// Corre `evaluar_con` real con la parte de Pruebas (mutación) y devuelve todos los comandos que corrieron.
    fn comandos_de_pruebas(home: &Path) -> Vec<String> {
        let carpeta = carpeta_sin_red(home);
        let tarea = Tarea::cargar_de(&carpeta).expect("tarea");
        let repo = carpeta.join("repo-bueno");
        let rubricas = crate::oracle::rubrica::cargar_embebidas().expect("rúbricas");
        let partes = partes_de(Dimension::Pruebas, &rubricas, &tarea);
        let cmds = std::sync::Arc::new(Mutex::new(Vec::<String>::new()));
        let del_hilo = cmds.clone();
        let fabrica = move |_: &crate::oracle::Preparada| -> Box<dyn EjecutorHerramienta> {
            Box::new(Anotador {
                cmds: del_hilo.clone(),
            })
        };
        let calibracion = Calibraciones::cargar(home).expect("calibración");
        let deps = crate::oracle::Dependencias {
            correr: &correr_sin_contenedores,
            herramientas: &fabrica,
            juez: None,
            modelo_juez: "",
            juez_config: None,
            calibracion: &calibracion,
            rubricas: &rubricas,
            reglas: crate::config::ReglasDecision::default(),
            redactor: None,
            avisos: Vec::new(),
            podman: &PodmanSinUso,
            escenarios: &FabricaSinContenedoresCal,
        };
        let o = OpcionesOraculo {
            partes: Some(partes),
            home: Some(home.to_path_buf()),
            historial: false,
            ..OpcionesOraculo::default()
        };
        crate::oracle::evaluar_con(&repo, &tarea, &o, deps).expect("evalúa");
        cmds.lock().expect("lock").clone()
    }

    #[test]
    fn mutacion_acotada_filtra_con_f_en_evaluar_con_real() {
        // Guarda: con el archivo acotado, la corrida lleva `-F` con su mutante y ningún comando lleva `--file`.
        let home = tempfile::tempdir().expect("home");
        let cmds = crate::oracle::mutacion::con_archivo_acotado(Some("src/protocolo.rs"), || {
            comandos_de_pruebas(home.path())
        });
        let de_mutacion: Vec<&String> = cmds
            .iter()
            .filter(|c| c.contains("cargo mutants"))
            .collect();
        assert!(de_mutacion.len() >= 2, "listado y corrida: {cmds:?}");
        for cmd in &cmds {
            assert!(!cmd.contains("--file"), "{cmd}");
        }
        let corrida = de_mutacion
            .iter()
            .find(|c| c.contains("--output /salida/mutants"))
            .expect("hay una corrida");
        assert!(corrida.contains(" -F "), "{corrida}");
        assert!(corrida.contains("src/protocolo\\.rs:2:1:"), "{corrida}");
    }

    #[test]
    fn mutacion_sin_archivo_acotado_no_lleva_file() {
        // Control: sin el ámbito, los mismos comandos salen sin `--file`.
        let home = tempfile::tempdir().expect("home");
        let cmds = comandos_de_pruebas(home.path());
        let de_mutacion: Vec<&String> = cmds
            .iter()
            .filter(|c| c.contains("cargo mutants"))
            .collect();
        assert!(de_mutacion.len() >= 2, "listado y corrida: {cmds:?}");
        for cmd in &de_mutacion {
            assert!(!cmd.contains("--file"), "{cmd}");
        }
    }

    // Punto 4: Pruebas calibra con `evaluar_con` real. La muestra repartida llega a mutantes con test.

    /// Listado de la prueba: 60 mutantes de `src/otro.rs` que ningún test llama, y luego 20 de `leer_numero`.
    fn listado_de_prueba() -> Vec<String> {
        let otros =
            (1..=60).map(|i| format!("src/otro.rs:{i}:1: replace funcion_{i} -> u32 with 0"));
        let protocolo = (0..20).map(|k| {
            format!("src/protocolo.rs:2:1: replace leer_numero -> Result<u32, ParseIntError> with Ok({k})")
        });
        otros.chain(protocolo).collect()
    }

    /// Escapa como `escapar_regex` de `mutacion.rs`. El `-F` de la sonda usa esta misma regla.
    fn escapar_como_f(texto: &str) -> String {
        let mut escapado = String::new();
        for c in texto.chars() {
            if "\\.+*?()|[]{}^$".contains(c) {
                escapado.push('\\');
            }
            escapado.push(c);
        }
        escapado
    }

    /// Nombres que pide una corrida: los de su `-F` `^(?:a|b)$`, comparados escapados. Sin `-F`, todos.
    fn nombres_pedidos(cmd: &str, listado: &[String]) -> Vec<String> {
        let Some(inicio) = cmd.find(" -F '") else {
            return listado.to_vec();
        };
        let resto = &cmd[inicio + " -F '".len()..];
        let regex = &resto[..resto.find('\'').expect("cierra el -F")];
        let cuerpo = regex
            .strip_prefix("^(?:")
            .and_then(|r| r.strip_suffix(")$"))
            .expect("forma ^(?:...)$");
        // Ningún nombre del listado tiene `|`. Partir por `|` da las alternativas.
        let alternativas: Vec<&str> = cuerpo.split('|').collect();
        listado
            .iter()
            .filter(|n| alternativas.contains(&escapar_como_f(n).as_str()))
            .cloned()
            .collect()
    }

    /// Verdadero si un test que llama a `leer_numero` aún tiene asserts en la copia.
    fn test_de_leer_numero_con_asserts(repo: &Path) -> bool {
        fs::read_dir(repo.join("tests"))
            .expect("carpeta tests de la copia")
            .filter_map(Result::ok)
            .any(|e| {
                let texto = fs::read_to_string(e.path()).unwrap_or_default();
                texto.contains("leer_numero")
                    && texto.contains("assert")
                    && !texto.contains("#[ignore]")
            })
    }

    /// Ejecutor falso de `cargo mutants` que responde según el `-F` que recibe, como la herramienta real.
    /// Un mutante de `src/protocolo.rs` muere solo si su test aún tiene asserts y no está apagado con `#[ignore]`.
    struct MutantesSegunFiltro {
        repo: PathBuf,
        pedidos: std::sync::Arc<Mutex<Vec<Vec<String>>>>,
    }

    impl EjecutorHerramienta for MutantesSegunFiltro {
        fn ejecutar(&self, cmd: &str) -> Result<SalidaHerramienta, ErrorOraculo> {
            let mut salida = SalidaHerramienta::default();
            let listado = listado_de_prueba();
            if crate::oracle::mutacion::es_comando_de_listado(cmd) {
                let items: Vec<serde_json::Value> =
                    listado.iter().map(|n| json!({"name": n})).collect();
                let texto = serde_json::Value::Array(items).to_string();
                salida.archivos.insert("listado.json".into(), texto);
                return Ok(salida);
            }
            if crate::oracle::mutacion::es_comando_de_mutacion(cmd) {
                let pedidos = nombres_pedidos(cmd, &listado);
                self.pedidos.lock().expect("lock").push(pedidos.clone());
                let con_asserts = test_de_leer_numero_con_asserts(&self.repo);
                let mut outcomes = vec![json!({"scenario": "Baseline", "summary": "Success"})];
                for nombre in &pedidos {
                    let archivo = nombre.split(':').next().unwrap_or_default();
                    let linea: u32 = nombre
                        .split(':')
                        .nth(1)
                        .and_then(|l| l.parse().ok())
                        .expect("línea del nombre");
                    let vivo = archivo == "src/protocolo.rs" && !con_asserts;
                    let estado = if vivo { "MissedMutant" } else { "CaughtMutant" };
                    let mutante = json!({
                        "name": nombre,
                        "file": archivo,
                        "span": {"start": {"line": linea, "column": 1}},
                    });
                    let mut resultado = json!({"scenario": {"Mutant": mutante}, "summary": estado});
                    if archivo == "src/protocolo.rs" {
                        resultado["log_path"] = json!(LOG_DE_PROTOCOLO);
                    }
                    outcomes.push(resultado);
                }
                let reporte = json!({"total_mutants": pedidos.len(), "outcomes": outcomes});
                salida.archivos.insert(
                    crate::oracle::mutacion::ARCHIVO_OUTCOMES.into(),
                    reporte.to_string(),
                );
                salida.archivos.insert(
                    crate::oracle::mutacion::ARCHIVO_TESTS_FALLIDOS.into(),
                    TESTS_FALLIDOS_DE_PROTOCOLO.into(),
                );
                return Ok(salida);
            }
            HerramientasSanas.ejecutar(cmd)
        }
    }

    /// Punto 4: Pruebas con `evaluar_con` real. La base muestra un mutante de `protocolo.rs` y su AssertBorrado aplica.
    #[test]
    fn calibra_pruebas_con_evaluar_con_real_aplica_el_defecto_en_la_muestra_repartida() {
        let home = tempfile::tempdir().expect("home");
        let carpeta = carpeta_sin_red(home.path());
        fs::write(
            carpeta.join("repo-bueno/tests/protocolo.rs"),
            TEST_DE_LEER_NUMERO,
        )
        .expect("test de leer_numero");
        // Cuatro mutantes de muestra, repartidos sobre los 80 del listado.
        tarea_en(
            &carpeta,
            "\n[calibracion]\nrepo = \"repo-bueno\"\n\n[pruebas]\nmutantes_max = 4\nminutos_max = 20\n",
        );
        let log_pedidos = std::sync::Arc::new(Mutex::new(Vec::<Vec<String>>::new()));
        let fabrica = |p: &crate::oracle::Preparada| -> Box<dyn EjecutorHerramienta> {
            Box::new(MutantesSegunFiltro {
                repo: p.repo.clone(),
                pedidos: log_pedidos.clone(),
            })
        };
        let rubricas = crate::oracle::rubrica::cargar_embebidas().expect("rúbricas");
        let calibracion = Calibraciones::cargar(home.path()).expect("calibración");
        let evaluador = |tarea: &Tarea, repo: &Path, o: &OpcionesOraculo| {
            let deps = crate::oracle::Dependencias {
                correr: &correr_sin_contenedores,
                herramientas: &fabrica,
                juez: None,
                modelo_juez: "",
                juez_config: None,
                calibracion: &calibracion,
                rubricas: &rubricas,
                reglas: crate::config::ReglasDecision::default(),
                redactor: None,
                avisos: Vec::new(),
                podman: &PodmanSinUso,
                escenarios: &FabricaSinContenedoresCal,
            };
            let con_home = OpcionesOraculo {
                home: Some(home.path().to_path_buf()),
                ..o.clone()
            };
            crate::oracle::evaluar_con(repo, tarea, &con_home, deps)
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
        .expect("calibra Pruebas");
        let casos = renglones_guardados(&calibrada);
        let con_defecto: Vec<&CasoGuardado> = casos
            .iter()
            .filter(|c| c.defecto.as_deref() == Some("assert_borrado"))
            .collect();
        assert!(!con_defecto.is_empty(), "hay un AssertBorrado: {casos:?}");
        // El defecto aplica: ningún caso queda no aplicable.
        let no_aplicables: Vec<&str> = con_defecto
            .iter()
            .filter_map(|c| c.no_aplicable.as_deref())
            .collect();
        assert!(
            no_aplicables.is_empty(),
            "el AssertBorrado no debe quedar no aplicable: {no_aplicables:?}"
        );
        // Cada caso aplicado detecta el mutante que vive sin asserts.
        for caso in &con_defecto {
            assert!(
                caso.detectado,
                "caso {} sin detectar: {:?}",
                caso.caso, caso.motivo
            );
            assert!(
                caso.motivo
                    .as_deref()
                    .is_some_and(|m| m.starts_with("mutante_vivo:src/protocolo.rs:")),
                "motivo del caso {}: {:?}",
                caso.caso,
                caso.motivo
            );
        }
        // La muestra repartida de la base incluye un mutante de `protocolo.rs`.
        let pedidos = log_pedidos.lock().expect("lock");
        assert!(
            pedidos
                .first()
                .is_some_and(|p| p.iter().any(|n| n.starts_with("src/protocolo.rs:"))),
            "la muestra de la base: {pedidos:?}"
        );
    }

    /// Requisito con `prueba` como única prueba.
    fn requisito_con_prueba(id: &str, prueba: &str) -> crate::oracle::tarea::Requisito {
        crate::oracle::tarea::Requisito {
            id: id.into(),
            titulo: format!("Título de {id}"),
            seccion: String::new(),
            peso: 1,
            pruebas: vec![prueba.into()],
            bordes: Vec::new(),
            escenarios: Vec::new(),
            criterios: Vec::new(),
            modulos: Vec::new(),
        }
    }

    /// Resultados de las suites: `pasa` pasa y `inestables` varía entre corridas.
    fn resultados_con_inestables(
        pasa: &[&str],
        inestables: &[&str],
    ) -> crate::oracle::requisitos::ResultadosTests {
        crate::oracle::requisitos::ResultadosTests {
            tests: pasa
                .iter()
                .map(|t| (t.to_string(), crate::oracle::junit::ResultadoTest::Pasa))
                .collect(),
            inestables: inestables.iter().map(|t| t.to_string()).collect(),
            tasas: BTreeMap::new(),
        }
    }

    #[test]
    fn hallazgo_de_test_ruidoso_no_detecta_en_completitud() {
        // `t::a` pasa en la base y es inestable en el limpio: es ruidoso. Su hallazgo no cuenta.
        let reqs = [requisito_con_prueba("R1", "t::a")];
        let r = resultados_con_inestables(&[], &["t::a"]);
        let base = Veredicto {
            puntajes: vec![puntaje(Dimension::Completitud, 1.0)],
            ..Veredicto::default()
        };
        let limpio = Veredicto {
            puntajes: vec![puntaje(Dimension::Completitud, 1.0)],
            inestables: vec!["t::a".into()],
            hallazgos: crate::oracle::requisitos::hallazgos_inestables(&reqs, &r),
            ..Veredicto::default()
        };
        let mut casos = vec![Caso::nuevo(1, None, limpio)];
        finalizar_casos(&base, Dimension::Completitud, &[], &[], &mut casos);
        assert!(
            !casos[0].detectado,
            "el hallazgo de un test ruidoso no cuenta: {:?}",
            casos[0].motivo
        );
    }

    #[test]
    fn hallazgo_de_test_que_no_es_ruidoso_detecta_en_completitud() {
        // `t::c` no cambia de estado entre la base y el limpio: no es ruidoso. Su hallazgo nuevo cuenta.
        let base = Veredicto {
            puntajes: vec![puntaje(Dimension::Completitud, 1.0)],
            ..Veredicto::default()
        };
        let limpio = Veredicto {
            puntajes: vec![puntaje(Dimension::Completitud, 1.0)],
            hallazgos: vec![hallazgo_de_completitud("h-t-c", "t::c")],
            ..Veredicto::default()
        };
        let mut casos = vec![Caso::nuevo(1, None, limpio)];
        finalizar_casos(&base, Dimension::Completitud, &[], &[], &mut casos);
        assert!(casos[0].detectado, "un hallazgo nuevo sin ruido sí cuenta");
    }

    #[test]
    fn calibra_fila_guarda_la_lista_de_ruidosos() {
        // `t_var` falla solo en los limpios: varía entre la base y los limpios, así que es ruidoso.
        let base = veredicto_base();
        let fila = calibrar(
            Path::new(FIXTURE),
            &base,
            |limpia, _, base| {
                let mut v = if limpia { base.clone() } else { hundido(base) };
                if limpia {
                    v.inestables = vec!["t_var".into()];
                }
                v
            },
            20,
        );
        assert!(
            fila.ruidosos.contains(&"oculto:t_var".to_string()),
            "{:?}",
            fila.ruidosos
        );
    }

    #[test]
    fn fila_sin_ruidosos_carga_con_lista_vacia() {
        let fila = fila("memora", Dimension::Seguridad, 1.0, Some(0.95));
        let mut vieja = serde_json::to_value(&fila).expect("serializa");
        vieja.as_object_mut().expect("objeto").remove("ruidosos");
        let leida: Fila = serde_json::from_value(vieja).expect("un JSON viejo parsea");
        assert!(leida.ruidosos.is_empty());
    }

    /// Hallazgo de Completitud con `id` y `cita`.
    fn hallazgo_de_completitud(id: &str, cita: &str) -> Hallazgo {
        Hallazgo {
            dimension: Dimension::Completitud,
            severidad: 2,
            archivo: String::new(),
            linea: 0,
            cita: cita.into(),
            detalle: String::new(),
            fuente: Fuente::Sonda("test".into()),
            id: id.into(),
            impacto: vec![],
        }
    }

    #[test]
    fn motivo_hallazgo_nuevo_nombra_la_cita_que_disparo() {
        let base = Veredicto {
            puntajes: vec![puntaje(Dimension::Completitud, 1.0)],
            ..Veredicto::default()
        };
        let mut veredicto = base.clone();
        veredicto
            .hallazgos
            .push(hallazgo_de_completitud("h-nuevo", "t::nuevo"));
        assert_eq!(
            motivo_de_deteccion(&base, &veredicto, Dimension::Completitud, &BTreeSet::new())
                .as_deref(),
            Some("hallazgo_nuevo:t::nuevo"),
        );
    }

    const ITEM_MODULARIDAD: &str = "diseno.modularidad";
    const ITEM_ROBUSTEZ: &str = "diseno.robustez";
    const ITEM_ERRORES: &str = "diseno.errores";

    /// Veredicto de Diseño con `items` del juez y valor 1.
    fn diseno_con(items: &[(&str, u8)]) -> Veredicto {
        Veredicto {
            puntajes: vec![Puntaje {
                items: items
                    .iter()
                    .map(|(item, nivel)| NivelItem {
                        item: (*item).into(),
                        nivel: *nivel,
                        evidencia: vec![],
                        fuente: Fuente::Juez {
                            modelo: "deepseek-reasoner".into(),
                        },
                    })
                    .collect(),
                ..puntaje(Dimension::Diseno, 1.0)
            }],
            ..Veredicto::default()
        }
    }

    /// Nivel alterno de la base: el juicio `k` pone `bajo` si `k` es par y `alto` si es impar.
    fn alterno(k: usize, bajo: u8, alto: u8) -> u8 {
        if k.is_multiple_of(2) { bajo } else { alto }
    }

    #[test]
    fn calibra_item_del_juez_que_alterna_en_la_base_queda_inestable_y_fuera_de_la_medicion() {
        let (_t, tarea) = tarea_prueba();
        let juicios = AtomicUsize::new(0);
        let evaluador = |_: &Path, tipo: TipoCaso, _: Target| -> Result<Veredicto, ErrorOraculo> {
            if matches!(tipo, TipoCaso::Base | TipoCaso::BaseRepetida) {
                let k = juicios.fetch_add(1, Ordering::SeqCst);
                let nivel = alterno(k, 0, 2);
                return Ok(diseno_con(&[(ITEM_MODULARIDAD, nivel), (ITEM_ROBUSTEZ, 3)]));
            }
            Ok(diseno_con(&[(ITEM_MODULARIDAD, 0), (ITEM_ROBUSTEZ, 3)]))
        };
        let medida = calibrar_medida_con_hilos(
            &tarea,
            Dimension::Diseno,
            6,
            42,
            Path::new(FIXTURE),
            &evaluador,
            None,
            &Vec::new,
            &|| None,
            1,
            None,
        )
        .expect("mide");
        let fila = medida.fila.expect("hay fila");
        assert_eq!(fila.inestables, vec![ITEM_MODULARIDAD.to_string()]);
        assert!(!medida.casos.is_empty(), "hay casos medidos");
        for caso in &medida.casos {
            assert!(
                !caso.niveles_base.contains_key(ITEM_MODULARIDAD),
                "caso {}",
                caso.indice
            );
        }
    }

    #[test]
    fn calibra_dimension_inestable_no_mide_casos_y_no_califica() {
        let (_t, tarea) = tarea_prueba();
        let llamadas = AtomicUsize::new(0);
        let evaluador = |_: &Path, _: TipoCaso, _: Target| -> Result<Veredicto, ErrorOraculo> {
            let k = llamadas.fetch_add(1, Ordering::SeqCst);
            let nivel = alterno(k, 0, 2);
            Ok(diseno_con(&[
                (ITEM_MODULARIDAD, nivel),
                (ITEM_ROBUSTEZ, nivel),
                (ITEM_ERRORES, nivel),
            ]))
        };
        let medida = calibrar_medida_con_hilos(
            &tarea,
            Dimension::Diseno,
            6,
            42,
            Path::new(FIXTURE),
            &evaluador,
            None,
            &Vec::new,
            &|| None,
            1,
            None,
        )
        .expect("mide");
        // Tres juicios de la base y ningún caso: los ítems inestables pesan 8 de 13.
        assert_eq!(llamadas.load(Ordering::SeqCst), 3);
        assert!(medida.casos.is_empty());
        let fila = medida.fila.expect("hay fila");
        assert_eq!(fila.n, 0);
        assert_eq!(
            fila.inestables,
            vec![
                ITEM_ERRORES.to_string(),
                ITEM_MODULARIDAD.to_string(),
                ITEM_ROBUSTEZ.to_string()
            ]
        );
        let motivo = fila.inestable.clone().expect("motivo");
        assert_eq!(fila.no_aplica, None, "una inestable no va a no_aplica");
        assert!(motivo.starts_with("inestable en esta tarea"), "{motivo}");
        assert!(motivo.contains(ITEM_MODULARIDAD), "{motivo}");
        assert!(!muestra_suficiente(&fila), "no califica");
    }

    #[test]
    fn calibra_dimension_inestable_va_a_inestable_y_no_a_no_aplica() {
        let (_t, tarea) = tarea_prueba();
        let llamadas = AtomicUsize::new(0);
        let evaluador = |_: &Path, _: TipoCaso, _: Target| -> Result<Veredicto, ErrorOraculo> {
            let k = llamadas.fetch_add(1, Ordering::SeqCst);
            let nivel = alterno(k, 0, 2);
            Ok(diseno_con(&[
                (ITEM_MODULARIDAD, nivel),
                (ITEM_ROBUSTEZ, nivel),
                (ITEM_ERRORES, nivel),
            ]))
        };
        let medida = calibrar_medida_con_hilos(
            &tarea,
            Dimension::Diseno,
            6,
            42,
            Path::new(FIXTURE),
            &evaluador,
            None,
            &Vec::new,
            &|| None,
            1,
            None,
        )
        .expect("mide");
        let fila = medida.fila.expect("hay fila");
        let motivo = fila.inestable.as_deref().expect("la fila es inestable");
        assert!(motivo.contains(ITEM_MODULARIDAD), "{motivo}");
        assert_eq!(fila.no_aplica, None, "{fila:?}");
    }

    #[test]
    fn calibra_base_sin_items_del_juez_se_juzga_una_sola_vez() {
        // Sin modelo de juez, la base no trae ítems del juez: no hay juicios que repetir.
        let (_t, tarea) = tarea_prueba();
        let bases = AtomicUsize::new(0);
        let repetidas = AtomicUsize::new(0);
        let evaluador = |_: &Path, tipo: TipoCaso, _: Target| -> Result<Veredicto, ErrorOraculo> {
            match tipo {
                TipoCaso::Base => {
                    bases.fetch_add(1, Ordering::SeqCst);
                }
                TipoCaso::BaseRepetida => {
                    repetidas.fetch_add(1, Ordering::SeqCst);
                }
                _ => {}
            }
            Ok(veredicto_de(Dimension::Diseno, 1.0, None))
        };
        calibrar_medida_con_hilos(
            &tarea,
            Dimension::Diseno,
            2,
            42,
            Path::new(FIXTURE),
            &evaluador,
            None,
            &Vec::new,
            &|| None,
            1,
            None,
        )
        .expect("mide");
        assert_eq!(bases.load(Ordering::SeqCst), 1, "la base se juzga una vez");
        assert_eq!(
            repetidas.load(Ordering::SeqCst),
            0,
            "sin juez no hay repeticiones"
        );
    }

    #[test]
    fn calibra_limpio_que_baja_dos_niveles_en_item_inestable_no_es_falsa_alarma() {
        let (_t, tarea) = tarea_prueba();
        let juicios = AtomicUsize::new(0);
        let evaluador = |_: &Path, tipo: TipoCaso, _: Target| -> Result<Veredicto, ErrorOraculo> {
            if matches!(tipo, TipoCaso::Base | TipoCaso::BaseRepetida) {
                let k = juicios.fetch_add(1, Ordering::SeqCst);
                let nivel = alterno(k, 2, 0);
                return Ok(diseno_con(&[(ITEM_MODULARIDAD, nivel), (ITEM_ROBUSTEZ, 3)]));
            }
            // Las copias bajan el ítem inestable de 2 a 0, respecto del primer juicio.
            Ok(diseno_con(&[(ITEM_MODULARIDAD, 0), (ITEM_ROBUSTEZ, 3)]))
        };
        let medida = calibrar_medida_con_hilos(
            &tarea,
            Dimension::Diseno,
            6,
            42,
            Path::new(FIXTURE),
            &evaluador,
            None,
            &Vec::new,
            &|| None,
            1,
            None,
        )
        .expect("mide");
        let limpios: Vec<&Caso> = medida.casos.iter().filter(|c| !c.es_defecto).collect();
        assert!(!limpios.is_empty(), "hay limpios medidos");
        for caso in limpios {
            assert!(!caso.detectado, "caso {}: no es falsa alarma", caso.indice);
            assert!(
                !caso.niveles_base.contains_key(ITEM_MODULARIDAD),
                "caso {}",
                caso.indice
            );
        }
    }

    #[test]
    fn calibra_dimension_solo_de_sondas_mide_la_base_una_vez() {
        let (_t, tarea) = tarea_prueba();
        let base_llamadas = AtomicUsize::new(0);
        let evaluador = |_: &Path, tipo: TipoCaso, _: Target| -> Result<Veredicto, ErrorOraculo> {
            if matches!(tipo, TipoCaso::Base | TipoCaso::BaseRepetida) {
                base_llamadas.fetch_add(1, Ordering::SeqCst);
            }
            Ok(veredicto_base())
        };
        calibrar_medida_con_hilos(
            &tarea,
            Dimension::Seguridad,
            2,
            42,
            Path::new(FIXTURE),
            &evaluador,
            None,
            &Vec::new,
            &|| None,
            1,
            None,
        )
        .expect("mide");
        assert_eq!(base_llamadas.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn fila_sin_inestables_en_el_json_se_lee_vacia() {
        let mut json = serde_json::to_value(fila_de_prueba("t", 1.0)).expect("serializa");
        json.as_object_mut().expect("objeto").remove("inestables");
        let fila: Fila = serde_json::from_value(json).expect("lee");
        assert!(fila.inestables.is_empty());
    }
}
