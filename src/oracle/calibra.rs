//! Calibración del Oracle. La Tarea 12 escribe `$ROOT_HOME/calibracion.json` con la sensibilidad, la
//! especificidad y la exactitud balanceada de cada dimensión, medidas con defectos conocidos. La Tarea 13
//! lo lee: una dimensión queda `calibrada` si su exactitud balanceada llega a 0,9, y solo esas entran a
//! `global`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use serde_json::{Map, json};

use super::juez::LimitesPrompt;
use super::tarea::Tarea;
use super::{ErrorOraculo, OpcionesOraculo, Partes, partes_de};
use crate::borrado;
use crate::config::{Config, Precio};
use crate::observe;
use crate::proveedores::{INTENTOS_MAX, MAX_TOKENS_JUEZ, costo_peor_caso};
use crate::tipos::{Dimension, Eleccion, Evento, Puntaje, Veredicto};

/// Nombre del archivo de calibración dentro de `ROOT_HOME`.
pub const ARCHIVO: &str = "calibracion.json";
/// Versión del formato. Otra versión es un archivo que esta lectura no entiende: error de infraestructura.
pub const VERSION: u32 = 1;
/// Exactitud balanceada desde la que una dimensión queda calibrada.
pub const UMBRAL: f32 = 0.9;

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
    /// Sin fila, el puntaje queda sin calibrar.
    pub fn aplicar(&self, tarea: &str, puntajes: &mut [Puntaje]) {
        for puntaje in puntajes {
            let fila = self.de(tarea, puntaje.dimension);
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

use inyectar::Defecto;

/// Archivo de candado de `calibracion.json`. Una escritura lo toma en exclusiva.
const CANDADO: &str = "calibracion.lock";

/// Calibra `dimension` de `tarea` con `n` casos y la semilla `semilla`, sobre el repo bueno `repo_bueno`.
/// `evaluador` corre el Oracle sobre una ruta. Sin defectos para la dimensión, `n` cero o una base sin
/// veredicto, no hay fila.
pub fn calibrar_con(
    tarea: &Tarea,
    dimension: Dimension,
    n: usize,
    semilla: u64,
    repo_bueno: &Path,
    evaluador: &dyn Fn(&Path) -> Result<Veredicto, ErrorOraculo>,
) -> Result<Fila, ErrorOraculo> {
    calibrar_medida(tarea, dimension, n, semilla, repo_bueno, evaluador, None)?
        .fila
        .ok_or_else(|| ErrorOraculo::Infra("la calibración no dejó ninguna fila".into()))
}

/// Resultado de una medición: la fila (completa, o la de los casos hechos si el tope la detuvo) y, si el
/// tope la detuvo, su motivo. Sin casos medidos, la fila no existe.
struct Medida {
    fila: Option<Fila>,
    detencion: Option<Detencion>,
}

/// Por qué el tope detuvo una calibración: el tope, el gasto medido, el peor caso que no entró y los
/// casos medidos (decisión 48).
struct Detencion {
    tope_usd: f64,
    gasto_usd: f64,
    peor_caso_usd: f64,
    casos: u32,
}

/// Tope ya resuelto para una medición: el peor caso de un caso, el inicio de la calibración y el medidor.
struct Tope<'a> {
    usd: f64,
    peor_caso: f64,
    inicio: f64,
    gasto_desde: &'a dyn Fn(f64) -> Result<f64, ErrorOraculo>,
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

/// La medición de `calibrar_con` y de `calibrar_tarea_con`. Antes de la línea base y de cada caso, si hay
/// tope, compara el gasto más el peor caso con él. Si no entra, para: la fila queda con los casos hechos y
/// marcada `incompleta`. Sin casos hechos, no hay fila.
fn calibrar_medida(
    tarea: &Tarea,
    dimension: Dimension,
    n: usize,
    semilla: u64,
    repo_bueno: &Path,
    evaluador: &dyn Fn(&Path) -> Result<Veredicto, ErrorOraculo>,
    tope: Option<&Tope>,
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
        });
    }
    // Una línea base sin veredicto no mide nada: los defectos no tienen contra qué compararse.
    let base = evaluador(repo_bueno)?;
    if puntaje_de(&base, dimension).is_none_or(|p| p.sin_veredicto.is_some()) {
        return Err(ErrorOraculo::Infra(format!(
            "la línea base no tiene veredicto de {}",
            dimension.clave()
        )));
    }
    let items: BTreeSet<&'static str> = defectos.iter().map(|d| d.item()).collect();
    let items: Vec<&'static str> = items.into_iter().collect();
    let tmp = tempfile::tempdir().map_err(|e| super::infra("carpeta temporal", e))?;
    let capas: &[Vec<String>] = tarea
        .arquitectura
        .as_ref()
        .map_or(&[], |a| a.capas.as_slice());

    let camino: &[String] = tarea
        .calibracion
        .as_ref()
        .map_or(&[], |c| c.camino_caliente.as_slice());
    let mut casos: Vec<Caso> = Vec::with_capacity(n);
    let mut no_aplicables = 0u32;
    let mut rotacion = 0usize;
    let mut detencion = None;
    for i in 0..n {
        if let Some(d) = detencion_si_no_entra(tope, casos.len())? {
            detencion = Some(d);
            break;
        }
        // Casos pares con defecto; impares limpios. Con n impar sobra un defecto.
        let es_defecto = i % 2 == 0;
        let copia = tmp.path().join(format!("caso-{i}"));
        super::copiar_repo(repo_bueno, &copia).map_err(|e| super::infra("copia del repo", e))?;
        let defecto = if es_defecto {
            let defecto = defectos[rotacion % defectos.len()];
            rotacion += 1;
            let semilla_caso = semilla.wrapping_add(i as u64);
            let aplicado = inyectar::inyectar_con(defecto, &copia, capas, camino, semilla_caso)
                .map_err(|e| super::infra("inyectar un defecto", e))?;
            if aplicado.is_none() {
                no_aplicables += 1;
                borrado::borrar_dir_dentro(tmp.path(), &copia)
                    .map_err(|e| super::infra("borrar una copia", e))?;
                continue;
            }
            Some(defecto)
        } else {
            None
        };
        let veredicto = evaluador(&copia)?;
        borrado::borrar_dir_dentro(tmp.path(), &copia)
            .map_err(|e| super::infra("borrar una copia", e))?;
        casos.push(Caso::nuevo(defecto, &base, &veredicto, dimension, &items));
    }

    let incompleta = detencion.is_some();
    let fila = if incompleta && casos.is_empty() {
        None
    } else {
        Some(fila_de(
            tarea,
            dimension,
            semilla,
            &casos,
            &items,
            no_aplicables,
            incompleta,
        ))
    };
    Ok(Medida { fila, detencion })
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
    evaluador: &dyn Fn(&Tarea, &Path, &OpcionesOraculo) -> Result<Veredicto, ErrorOraculo>,
) -> Result<Fila, ErrorOraculo> {
    calibrar_tarea_con(
        home, nombre, dimension, n, semilla, repo_cli, None, evaluador,
    )
}

/// Tope de gasto de una calibración (decisión 48). Antes de la línea base y de cada caso, se detiene si el
/// gasto de API desde el inicio más el peor caso de un caso pasa `usd`.
pub struct Presupuesto<'a> {
    /// Tope en USD. Debe ser finito y mayor que 0.
    pub usd: f64,
    /// Peor caso en USD de un caso con estas partes. Un modelo sin precio da un error.
    pub peor_caso: &'a dyn Fn(&Partes) -> Result<f64, ErrorOraculo>,
    /// Gasto de API desde un instante (epoch en segundos): llamadas y reservas abiertas.
    pub gasto_desde: &'a dyn Fn(f64) -> Result<f64, ErrorOraculo>,
}

/// Elecciones de los jueces que un caso puede llamar. `None`: el juez no se arma y no llama.
#[derive(Debug, Clone, Copy, Default)]
pub struct JuecesCaso<'a> {
    /// Juez de diseño (Tarea 10): llama si las partes lo piden.
    pub diseno: Option<&'a Eleccion>,
    /// Juez de la configuración de Cargo (decisión 30): llama si las partes lo piden.
    pub config: Option<&'a Eleccion>,
}

/// Entrada estimada de cada llamada de juez, en bytes UTF-8. Es una cota de diseño, no un límite: el
/// prompt del juez de diseño tiene límites de mapa y extractos (4 bytes por carácter como máximo), pero
/// la spec y el `Cargo.toml` no tienen. 256 KiB cubre una spec y un `Cargo.toml` de tamaño normal.
pub const ENTRADA_JUEZ_BYTES: u64 = 256 * 1024;

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
                        * costo_peor_caso(precio, ENTRADA_JUEZ_BYTES, max_tokens)
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
/// marcada `incompleta` (si hubo alguno) y sale con código 3.
#[allow(clippy::too_many_arguments)]
pub fn calibrar_tarea_con(
    home: &Path,
    nombre: &str,
    dimension: &str,
    n: u32,
    semilla: u64,
    repo_cli: Option<&Path>,
    presupuesto: Option<&Presupuesto>,
    evaluador: &dyn Fn(&Tarea, &Path, &OpcionesOraculo) -> Result<Veredicto, ErrorOraculo>,
) -> Result<Fila, ErrorOraculo> {
    let dim = Dimension::desde_clave(dimension)
        .ok_or_else(|| ErrorOraculo::Uso(format!("dimensión desconocida: {dimension}")))?;
    let tarea = super::tarea::cargar_en(home, nombre)?;
    let repo = repo_de(&tarea, repo_cli)?;
    // La base y cada caso corren las mismas partes: solo la dimensión calibrada se mide.
    let rubricas = super::rubrica::cargar_embebidas()
        .map_err(|e| ErrorOraculo::Infra(format!("rúbricas embebidas: {e}")))?;
    let partes = partes_de(dim, &rubricas);
    let opciones = OpcionesOraculo {
        partes: Some(partes),
        ..OpcionesOraculo::default()
    };
    // El tope se resuelve antes de la base: un peor caso sin precio no deja arrancar.
    let inicio = ahora();
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
        &|ruta| evaluador(&tarea, ruta, &opciones),
        tope.as_ref(),
    )?;
    if let Some(detencion) = medida.detencion {
        let gate = gate_de_tope(&tarea.nombre, dim, &detencion);
        observe::registrar_en(&observe::ruta_eventos_en(home), &gate)
            .map_err(|e| ErrorOraculo::Infra(format!("registrar el Gate de calibración: {e}")))?;
        if let Some(fila) = &medida.fila {
            agregar(home, fila)?;
        }
        return Err(ErrorOraculo::Tope(format!(
            "calibración de {}/{} detenida por el tope de {:.2} USD: gasto de API {:.2} USD y el peor caso de un caso es {:.2} USD. Casos medidos: {}.",
            tarea.nombre,
            dim.clave(),
            detencion.tope_usd,
            detencion.gasto_usd,
            detencion.peor_caso_usd,
            detencion.casos
        )));
    }
    let fila = medida
        .fila
        .ok_or_else(|| ErrorOraculo::Infra("la calibración no dejó ninguna fila".into()))?;
    agregar(home, &fila)?;
    Ok(fila)
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

/// Un caso medido: si era un defecto (y cuál ítem apuntaba), si la dimensión lo detectó y, por ítem,
/// si cada ítem bajó (`None` si la base no mide ese ítem).
struct Caso {
    es_defecto: bool,
    item_defecto: Option<&'static str>,
    detectado: bool,
    items: BTreeMap<&'static str, Option<bool>>,
}

impl Caso {
    fn nuevo(
        defecto: Option<Defecto>,
        base: &Veredicto,
        veredicto: &Veredicto,
        dimension: Dimension,
        items: &[&'static str],
    ) -> Caso {
        let es_defecto = defecto.is_some();
        Caso {
            es_defecto,
            item_defecto: defecto.map(Defecto::item),
            detectado: detecta_dimension(base, veredicto, dimension, es_defecto),
            items: items
                .iter()
                .map(|item| {
                    (
                        *item,
                        detecta_item(base, veredicto, dimension, item, es_defecto),
                    )
                })
                .collect(),
        }
    }

    fn detecta_en(&self, item: &str) -> Option<bool> {
        self.items.get(item).copied().flatten()
    }
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
    hallazgo_nuevo || pv.valor < pb.valor || item_bajo
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
        Some(nivel) => nivel < nivel_base,
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
mod tests {
    use super::inyectar::pruebas::{FIXTURE, arbol, copia_fixture, tarea_en};
    use super::*;
    use crate::tipos::{Fuente, Hallazgo, NivelItem};
    use std::cell::RefCell;
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
            }],
            ..Veredicto::default()
        }
    }

    fn nivel_item(item: &str, nivel: u8) -> NivelItem {
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
        regla: impl Fn(bool, &BTreeMap<String, String>, &Veredicto) -> Veredicto + 'a,
    ) -> impl Fn(&Path) -> Result<Veredicto, ErrorOraculo> + 'a {
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
        regla: impl Fn(bool, &BTreeMap<String, String>, &Veredicto) -> Veredicto,
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
        let vistos = RefCell::new(Vec::<PathBuf>::new());
        let evaluador = |_: &Tarea, ruta: &Path, _: &OpcionesOraculo| {
            vistos.borrow_mut().push(ruta.to_path_buf());
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
        assert_eq!(vistos.borrow()[0], repo);
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

    fn puntaje(dimension: Dimension, valor: f32) -> Puntaje {
        Puntaje {
            dimension,
            valor,
            calibrada: false,
            exactitud_balanceada: None,
            items: Vec::new(),
            sin_veredicto: None,
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
        let por_intento = (ENTRADA_JUEZ_BYTES as f64 * 2.0
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
}
