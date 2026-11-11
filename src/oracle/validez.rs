//! Validez del juicio de Root (Tarea 12b): ¿Root juzga como un PM competente? `validar` corre Root sobre
//! casos cuyo resultado conoce una fuente externa a Root: el orden conocido, los defectos reales y la
//! calificación a ciegas del usuario. Sin fuente, la métrica queda sin medir: nunca 0 ni 1.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::rubrica::{self, Rubrica};
use super::{ErrorOraculo, evaluar_por_nombre};
use crate::tipos::{Decision, Dimension, Estado, Fuente, TODAS_LAS_DIMENSIONES, Veredicto};

/// Versión del formato de `validez.json`.
pub const VERSION: u32 = 1;
/// Archivo que escribe `validar` dentro de la carpeta de casos.
pub const ARCHIVO: &str = "validez.json";
/// Recall desde el que los defectos reales cumplen (decisión 12).
pub const UMBRAL_RECALL: f32 = 0.8;
/// Kappa ponderado desde el que una dimensión cumple (decisión 13).
pub const UMBRAL_KAPPA: f32 = 0.6;
/// Exactitud de orden desde la que los pares cumplen: todos bien ordenados (desempate 44).
pub const UMBRAL_ORDEN: f32 = 1.0;
/// Tolerancia en líneas entre el defecto y el hallazgo (la del gate de evidencia).
pub const TOLERANCIA_LINEAS: u32 = 3;
/// Paquetes a ciegas del lote (decisión 13).
pub const PAQUETES_ESPERADOS: usize = 6;
/// Paquetes con la misma decisión que hacen cumplir el acuerdo (decisión 13: 5 de 6).
pub const ACUERDOS_MINIMOS: usize = 5;

const CARPETA_CIEGO: &str = "ciego";
const ARCHIVO_CLAVE: &str = "clave.toml";
const ARCHIVO_FORMULARIO: &str = "formulario.toml";
const CARPETA_REPO: &str = "repo";
const ARCHIVO_ORDEN: &str = "orden.toml";
const ARCHIVO_DEFECTOS: &str = "defectos.toml";
/// Niveles de la rúbrica: 0 a 3, cuatro categorías para el kappa.
const NIVEL_MAXIMO: u8 = 3;
const NIVELES: usize = 4;

/// Una medida de validez. Sin casos válidos, `valor` es `None` y `motivo` dice por qué.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Metrica {
    pub valor: Option<f32>,
    /// Casos válidos que entraron a la medida.
    pub casos: u32,
    /// Aciertos entre los casos. Solo en las fracciones; `None` en las demás.
    #[serde(default)]
    pub aciertos: Option<u32>,
    /// Umbral de la métrica. `None` si el plan no fija uno.
    #[serde(default)]
    pub umbral: Option<f32>,
    /// `true` si `valor` llega al umbral. `None` sin umbral o sin medir.
    #[serde(default)]
    pub cumple: Option<bool>,
    /// Por qué la métrica no se mide. Solo con `valor` en `None`.
    #[serde(default)]
    pub motivo: Option<String>,
}

/// Kappa de Cohen con pesos cuadráticos. `clave` es una dimensión (`seguridad`) o un ítem (`seguridad.secretos`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Kappa {
    pub clave: String,
    pub valor: Option<f32>,
    /// Pares de niveles (usuario, Root) que entraron al kappa.
    pub casos: u32,
    /// Umbral de la dimensión (0,6). `None` en los ítems, que solo son diagnóstico de anclas.
    #[serde(default)]
    pub umbral: Option<f32>,
    #[serde(default)]
    pub cumple: Option<bool>,
    #[serde(default)]
    pub motivo: Option<String>,
}

/// Resultado de `validar`. Cada métrica lleva su `valor`, su `cumple` y, si no se mide, su `motivo`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Validez {
    pub v: u32,
    /// Fracción de pares de `orden.toml` que Root ordena como el resultado medido. Umbral 1,0.
    pub exactitud_orden: Metrica,
    /// Un detalle por par de `orden.toml`, en su orden. Vacío sin `orden.toml` o sin pares.
    #[serde(default)]
    pub pares: Vec<DetallePar>,
    /// Fracción de defectos reales que Root encuentra: hallazgo con `archivo` y `linea` a ±3, o test rojo.
    /// Umbral 0,8.
    pub recall: Metrica,
    /// Fracción de defectos reales con hallazgo a `TOLERANCIA_LINEAS` líneas o menos. Sin umbral.
    #[serde(default = "metrica_sin_medir_vieja")]
    pub recall_linea: Metrica,
    /// Fracción de defectos reales con test rojo. Sin umbral.
    #[serde(default = "metrica_sin_medir_vieja")]
    pub recall_test: Metrica,
    /// Un detalle por defecto de `defectos.toml`, en su orden. Vacío sin `defectos.toml` o sin defectos.
    #[serde(default)]
    pub defectos: Vec<DetalleDefecto>,
    /// Paquetes con la misma `Decision` que el usuario. `cumple`: 5 de 6 (decisión 13).
    pub acuerdo_decision: Metrica,
    /// Una entrada por dimensión, en el orden de `TODAS_LAS_DIMENSIONES`. `cumple`: kappa >= 0,6.
    pub kappa_dimension: Vec<Kappa>,
    /// Una entrada por ítem de los formularios. Sin umbral: un kappa bajo marca un ancla ambigua.
    pub kappa_item: Vec<Kappa>,
    /// Spearman entre el `orden` de los formularios y el orden de Root. Sin umbral.
    pub spearman: Metrica,
    /// Casos que quedaron fuera o datos que faltan, con su motivo.
    pub avisos: Vec<String>,
}

/// Detalle de un par de `orden.toml`: qué decidió Root y con qué criterio.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DetallePar {
    /// Tarea del par.
    pub tarea: String,
    /// Caso que debe quedar por encima.
    pub mejor: String,
    /// Caso que debe quedar por debajo.
    pub peor: String,
    /// `true` si Root ordena `mejor` por encima de `peor`.
    pub acierto: bool,
    /// Decisión de `mejor`. `None` si Root no dio decisión.
    pub decision_mejor: Option<Decision>,
    /// Decisión de `peor`. `None` si Root no dio decisión.
    pub decision_peor: Option<Decision>,
    /// Global de `mejor`. `None` si el veredicto no lo trae.
    pub global_mejor: Option<f32>,
    /// Global de `peor`. `None` si el veredicto no lo trae.
    pub global_peor: Option<f32>,
    /// Completitud de `mejor`. `None` si el veredicto no la trae.
    pub completitud_mejor: Option<f32>,
    /// Completitud de `peor`. `None` si el veredicto no la trae.
    pub completitud_peor: Option<f32>,
    /// Criterio que decidió el par: `decision`, `global`, `completitud` o `sin_orden`.
    pub criterio: String,
}

/// Detalle de un defecto real de `defectos.toml`: si Root lo encontró y qué hallazgo quedó cerca.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DetalleDefecto {
    /// Tarea del defecto.
    pub tarea: String,
    /// Repo del defecto, dentro de la carpeta de casos.
    pub repo: String,
    /// Archivo del defecto.
    pub archivo: String,
    /// Línea del defecto.
    pub linea: u32,
    /// `true` si Root tiene un hallazgo a ±3 líneas. Solo este criterio: `recall` también cuenta el test rojo.
    pub encontrado: bool,
    /// `true` si la evidencia de algún ítem del veredicto cita este archivo a `TOLERANCIA_LINEAS` líneas o
    /// menos del defecto. No cuenta para `encontrado` ni para `recall`.
    #[serde(default)]
    pub en_evidencia: bool,
    /// `true` si algún hallazgo descartado del veredicto cae en el mismo archivo a `TOLERANCIA_LINEAS`
    /// líneas o menos del defecto. Solo informa: no cuenta para `encontrado`, `recall`, `recall_linea`
    /// ni `recall_test`.
    #[serde(default)]
    pub descartado_cerca: bool,
    /// Hallazgo del mismo archivo con menor distancia en línea. `None` si no hay ninguno en ese archivo.
    pub hallazgo_cercano: Option<HallazgoCercano>,
    /// Test que marca el defecto, copiado de `defectos.toml`. `None` si el defecto no lo trae.
    #[serde(default)]
    pub test: Option<String>,
    /// `Some(true)` si el test está en `ocultos_fallidos` o `contrato_fallidos` del veredicto. `None` sin `test`.
    /// Cuenta para `recall` y `recall_test`, no para `encontrado`.
    #[serde(default)]
    pub test_rojo: Option<bool>,
    /// `true` si el test está en `inestables` o `inestables_contrato`. No es rojo: no suma a `test_rojo`
    /// ni a `recall`. Un veredicto viejo lee `false`.
    #[serde(default)]
    pub test_inestable: bool,
    /// `true` si el veredicto no trae resultado de la sonda (`estado` sin veredicto). El defecto sigue en
    /// el denominador de `recall`. Un veredicto viejo lee `false`.
    #[serde(default)]
    pub sonda_sin_veredicto: bool,
}

/// Hallazgo de Root en el mismo archivo que un defecto. Es el más cercano en línea.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HallazgoCercano {
    /// Línea del hallazgo.
    pub linea: u32,
    /// Quién produjo el hallazgo.
    pub fuente: Fuente,
    /// Líneas entre el hallazgo y el defecto.
    pub distancia: u32,
}

/// Resultado de `preparar_ciego`: los ids de los paquetes armados. El origen queda en `clave.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preparado {
    pub paquetes: Vec<String>,
    /// `true` si no hay rúbricas embebidas: el formulario lleva solo la decisión y el orden.
    pub sin_rubricas: bool,
    /// Rastros de cada paquete. Van a `validez/rastros.json` (ver `ruta_rastros_en`), no a la salida
    /// (desempate 18): por eso no se serializan.
    #[serde(default, skip_serializing)]
    pub rastros: Vec<ResumenRastros>,
}

/// Carpeta de validez dentro de `home`: `<home>/validez/`. Aquí van `rastros.json` y las listas de rastros.
pub fn carpeta_validez_en(home: &Path) -> PathBuf {
    home.join("validez")
}

/// Dónde queda `rastros.json` dentro de `home`: `<home>/validez/rastros.json`.
pub fn ruta_rastros_en(home: &Path) -> PathBuf {
    carpeta_validez_en(home).join("rastros.json")
}

/// Resumen de los rastros de un paquete a ciegas. La lista completa está en `$ROOT_HOME/validez/<archivo>`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResumenRastros {
    pub id: String,
    /// Nombre de la lista, `rastros-<id>.toml`, dentro de `$ROOT_HOME/validez/`.
    pub archivo: String,
    /// Apariciones de `PALABRAS_DE_RASTRO` en los archivos de texto del paquete.
    pub apariciones: u32,
    /// Avisos del armado, por ejemplo un `Cargo.toml` que no parsea.
    pub avisos: u32,
}

/// Mide la validez de Root con el evaluador de producción (`evaluar_por_nombre`).
pub fn validar(casos: &Path) -> Result<Validez, ErrorOraculo> {
    validar_con(casos, &|ruta: &Path, tarea: &str| {
        evaluar_por_nombre(tarea, ruta)
    })
}

/// Mide la validez con un evaluador inyectado. Un error del evaluador aborta la medición: no hay veredicto.
pub fn validar_con(
    casos: &Path,
    evaluador: &dyn Fn(&Path, &str) -> Result<Veredicto, ErrorOraculo>,
) -> Result<Validez, ErrorOraculo> {
    if !casos.is_dir() {
        return Err(ErrorOraculo::Uso(format!(
            "no existe la carpeta de casos: {}",
            casos.display()
        )));
    }
    let mut ev = Evaluaciones {
        evaluador,
        cache: BTreeMap::new(),
        veredictos: casos.join("veredictos"),
    };
    let mut avisos = Vec::new();
    let pares = leer_orden(casos)?;
    let defectos = leer_defectos(casos)?;
    let (exactitud_orden, detalle_pares) = medir_orden(casos, pares.as_deref(), &mut ev)?;
    let (recall, recall_linea, recall_test, detalle_defectos) =
        medir_recall(casos, defectos.as_deref(), &mut ev)?;
    let ciego = leer_paquetes(casos, &mut ev, &mut avisos)?;
    let (acuerdo_decision, kappa_dimension, kappa_item, spearman) = match ciego {
        None => {
            let motivo = "falta casos/ciego/clave.toml: sin paquetes a ciegas";
            (
                Metrica::sin_medir(motivo),
                kappas_dimension_sin_medir(motivo),
                Vec::new(),
                Metrica::sin_medir(motivo),
            )
        }
        Some(paquetes) => {
            let (dimension, item) = medir_kappas(&paquetes, &mut avisos);
            (
                medir_decision(&paquetes, &mut avisos),
                dimension,
                item,
                medir_spearman(&paquetes, &mut avisos),
            )
        }
    };
    Ok(Validez {
        v: VERSION,
        exactitud_orden,
        pares: detalle_pares,
        recall,
        recall_linea,
        recall_test,
        defectos: detalle_defectos,
        acuerdo_decision,
        kappa_dimension,
        kappa_item,
        spearman,
        avisos,
    })
}

/// Arma los paquetes a ciegas de `fuentes` en `casos/ciego/`, con la semilla `semilla`. Las listas
/// `rastros-<id>.toml` van a `<home>/validez/`, nunca a `casos/ciego/`. Escribe los rastros de cada
/// paquete en `<home>/validez/rastros.json` (desempate 18).
pub fn preparar_ciego(
    casos: &Path,
    fuentes: &Path,
    semilla: u64,
    home: &Path,
) -> Result<Preparado, ErrorOraculo> {
    let rubricas = rubrica::cargar_embebidas()
        .map_err(|e| ErrorOraculo::Infra(format!("rúbricas embebidas: {e}")))?;
    let validez = carpeta_validez_en(home);
    let preparado = preparar_ciego_con(casos, fuentes, semilla, &rubricas, &validez)?;
    let ruta = ruta_rastros_en(home);
    if let Some(carpeta) = ruta.parent() {
        fs::create_dir_all(carpeta).map_err(|e| io_infra("crear la carpeta de validez", &e))?;
    }
    let texto = serde_json::to_string_pretty(&preparado.rastros)
        .map_err(|e| ErrorOraculo::Infra(format!("serializar los rastros: {e}")))?;
    fs::write(&ruta, format!("{texto}\n")).map_err(|e| io_infra("escribir rastros.json", &e))?;
    Ok(preparado)
}

/// Como `preparar_ciego`, con las rúbricas que recibe y la carpeta `validez` donde van las listas de
/// rastros. Los tests pasan rúbricas de fixture y una carpeta de validez bajo su temporal.
pub fn preparar_ciego_con(
    casos: &Path,
    fuentes: &Path,
    semilla: u64,
    rubricas: &[Rubrica],
    validez: &Path,
) -> Result<Preparado, ErrorOraculo> {
    let texto = fs::read_to_string(fuentes).map_err(|e| {
        ErrorOraculo::Uso(format!(
            "no se pudo leer el manifiesto {}: {e}",
            fuentes.display()
        ))
    })?;
    let manifiesto: ArchivoManifiesto = toml::from_str(&texto)
        .map_err(|e| ErrorOraculo::Uso(format!("manifiesto inválido: {e}")))?;
    if manifiesto.fuente.is_empty() {
        return Err(ErrorOraculo::Uso(
            "el manifiesto no tiene ninguna [[fuente]]".into(),
        ));
    }
    let base = fuentes.parent().unwrap_or_else(|| Path::new(""));
    let mut resueltas = Vec::new();
    for (i, fuente) in manifiesto.fuente.iter().enumerate() {
        let numero = i + 1;
        if fuente.tarea.trim().is_empty() || fuente.origen.trim().is_empty() {
            return Err(ErrorOraculo::Uso(format!(
                "fuente {numero}: falta `tarea` u `origen`"
            )));
        }
        let repo = base.join(&fuente.repo);
        if !repo.is_dir() {
            return Err(ErrorOraculo::Uso(format!(
                "fuente {numero}: no existe el repo {}",
                repo.display()
            )));
        }
        resueltas.push((fuente, repo));
    }

    let ciego = casos.join(CARPETA_CIEGO);
    if ciego.exists() && leer_entradas(&ciego)?.next().is_some() {
        return Err(ErrorOraculo::Uso(format!(
            "ya existe {} con contenido; bórrala o usa otro --casos",
            ciego.display()
        )));
    }

    // Los ids salen del PRNG y se repiten solo si chocan con otro id o contienen el origen o la tarea.
    let mut azar = Azar(semilla);
    let mut usados = BTreeSet::new();
    let mut ids = Vec::new();
    for (fuente, _) in &resueltas {
        let prohibidos = [fuente.origen.to_lowercase(), fuente.tarea.to_lowercase()];
        let mut id = None;
        for _ in 0..10_000 {
            let candidato = format!("{:08x}", (azar.siguiente() >> 32) as u32);
            let limpio = !usados.contains(&candidato)
                && !prohibidos.iter().any(|p| candidato.contains(p.as_str()));
            if limpio {
                id = Some(candidato);
                break;
            }
        }
        let id = id.ok_or_else(|| {
            ErrorOraculo::Infra("no hay un id libre para el paquete a ciegas".into())
        })?;
        usados.insert(id.clone());
        ids.push(id);
    }

    fs::create_dir_all(&ciego).map_err(|e| io_infra("crear casos/ciego", &e))?;
    fs::create_dir_all(validez).map_err(|e| io_infra("crear la carpeta de validez", &e))?;
    let mut clave = ArchivoClave::default();
    let mut resumen = Vec::new();
    let texto_rubricas = texto_formulario_items(rubricas);
    for (id, (fuente, repo)) in ids.iter().zip(&resueltas) {
        let paquete = ciego.join(id);
        fs::create_dir_all(&paquete).map_err(|e| io_infra("crear el paquete", &e))?;
        let copia = paquete.join(CARPETA_REPO);
        copiar_paquete(repo, &copia).map_err(|e| io_infra("copiar el repo al paquete", &e))?;
        let mut rastros = ArchivoRastros::default();
        revisar_copia(&copia, &copia, &mut rastros)
            .map_err(|e| io_infra("revisar los rastros del paquete", &e))?;
        let archivo = format!("rastros-{id}.toml");
        let texto_rastros = toml::to_string(&rastros)
            .map_err(|e| ErrorOraculo::Infra(format!("serializar los rastros: {e}")))?;
        fs::write(validez.join(&archivo), texto_rastros)
            .map_err(|e| io_infra("escribir la lista de rastros", &e))?;
        resumen.push(ResumenRastros {
            id: id.clone(),
            archivo,
            apariciones: rastros.aparicion.len() as u32,
            avisos: rastros.avisos.len() as u32,
        });
        let formulario = formulario_texto(id, &texto_rubricas);
        fs::write(paquete.join(ARCHIVO_FORMULARIO), formulario)
            .map_err(|e| io_infra("escribir el formulario", &e))?;
        clave.paquete.push(ClavePaquete {
            id: id.clone(),
            tarea: fuente.tarea.clone(),
            origen: fuente.origen.clone(),
            repo: repo.display().to_string(),
        });
    }
    let texto_clave = toml::to_string(&clave)
        .map_err(|e| ErrorOraculo::Infra(format!("serializar la clave: {e}")))?;
    fs::write(ciego.join(ARCHIVO_CLAVE), texto_clave)
        .map_err(|e| io_infra("escribir la clave", &e))?;
    Ok(Preparado {
        paquetes: ids,
        sin_rubricas: rubricas.is_empty(),
        rastros: resumen,
    })
}

/// Semilla del reloj para `preparar_ciego` cuando no hay `--semilla`.
pub fn semilla_del_reloj() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64)
}

/// Kappa ponderado (pesos cuadráticos sobre los niveles 0 a 3) de pares (usuario, Root).
/// `Err` con el motivo si no hay pares o si los niveles no varían (kappa indefinido).
fn kappa_ponderado(pares: &[(u8, u8)]) -> Result<f64, &'static str> {
    if pares.is_empty() {
        return Err("sin pares de niveles");
    }
    if pares
        .iter()
        .any(|&(u, r)| u > NIVEL_MAXIMO || r > NIVEL_MAXIMO)
    {
        return Err("hay un nivel fuera de 0 a 3");
    }
    let n = pares.len() as f64;
    let mut a = [0.0_f64; NIVELES];
    let mut b = [0.0_f64; NIVELES];
    let mut observado = 0.0;
    for &(usuario, root) in pares {
        a[usize::from(usuario)] += 1.0;
        b[usize::from(root)] += 1.0;
        observado += peso(usuario, root);
    }
    let mut esperado = 0.0;
    for (i, ai) in a.iter().enumerate() {
        for (j, bj) in b.iter().enumerate() {
            esperado += peso(i as u8, j as u8) * ai * bj;
        }
    }
    let desacuerdo_observado = observado / n;
    let desacuerdo_esperado = esperado / (n * n);
    if desacuerdo_esperado <= f64::EPSILON {
        return Err("sin variación de niveles: el kappa no está definido");
    }
    Ok(1.0 - desacuerdo_observado / desacuerdo_esperado)
}

/// Peso de desacuerdo cuadrático: (diferencia / 3)^2 con niveles de 0 a 3.
fn peso(usuario: u8, root: u8) -> f64 {
    let d = f64::from(usuario.abs_diff(root)) / f64::from(NIVEL_MAXIMO);
    d * d
}

/// Spearman de dos listas de valores: la correlación de Pearson de sus rangos promedio.
/// `None` si hay menos de dos valores o si una lista no varía.
pub fn spearman(a: &[f64], b: &[f64]) -> Option<f64> {
    if a.len() != b.len() || a.len() < 2 {
        return None;
    }
    let menos: fn(&f64) -> f64 = |x| -x;
    let ra = posiciones(&a.iter().map(menos).collect::<Vec<f64>>());
    let rb = posiciones(&b.iter().map(menos).collect::<Vec<f64>>());
    pearson(&ra, &rb)
}

/// Correlación de Pearson. `None` si una lista no varía.
fn pearson(x: &[f64], y: &[f64]) -> Option<f64> {
    let n = x.len() as f64;
    let mx = x.iter().sum::<f64>() / n;
    let my = y.iter().sum::<f64>() / n;
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for (a, b) in x.iter().zip(y) {
        let (dx, dy) = (a - mx, b - my);
        sxy += dx * dy;
        sxx += dx * dx;
        syy += dy * dy;
    }
    (sxx > f64::EPSILON && syy > f64::EPSILON).then(|| sxy / (sxx * syy).sqrt())
}

/// Posición de cada clave, con 1 para la mayor. Los empates comparten el promedio de sus posiciones.
fn posiciones<T: PartialOrd + Copy>(claves: &[T]) -> Vec<f64> {
    let mut orden: Vec<usize> = (0..claves.len()).collect();
    orden.sort_by(|&a, &b| claves[b].partial_cmp(&claves[a]).unwrap_or(Ordering::Equal));
    let mut salida = vec![0.0; claves.len()];
    let mut i = 0;
    while i < orden.len() {
        let mut j = i;
        while j + 1 < orden.len() && claves[orden[j + 1]] == claves[orden[i]] {
            j += 1;
        }
        let promedio = ((i + 1) + (j + 1)) as f64 / 2.0;
        for &k in &orden[i..=j] {
            salida[k] = promedio;
        }
        i = j + 1;
    }
    salida
}

/// Cuántas decisiones valen: listo 2, no listo 1, bloqueado y sin decisión 0. Sin decisión cuenta como
/// el peor: un caso sin veredicto cuenta en contra (desempate 41).
fn rango_decision(decision: Decision) -> u8 {
    match decision {
        Decision::Listo => 2,
        Decision::NoListo => 1,
        Decision::Bloqueado | Decision::SinDecision => 0,
    }
}

fn decision_de(v: &Veredicto) -> Option<Decision> {
    v.informe.as_ref().map(|i| i.decision)
}

fn completitud_de(v: &Veredicto) -> Option<f32> {
    v.puntajes
        .iter()
        .find(|p| p.dimension == Dimension::Completitud && p.sin_veredicto.is_none())
        .map(|p| p.valor)
}

fn nivel_de(v: &Veredicto, item: &str) -> Option<u8> {
    v.puntajes
        .iter()
        .flat_map(|p| p.items.iter())
        .find(|n| n.item == item)
        .map(|n| n.nivel)
}

/// Decide si Root pone `mejor` por encima de `peor`, y con qué criterio. Primero la decisión; a igual
/// decisión, el global si los dos lo tienen, y si no, la completitud. Sin decisión, o en empate, no
/// ordena: es un fallo.
fn ordena(mejor: &Veredicto, peor: &Veredicto) -> (bool, &'static str) {
    let (Some(dm), Some(dp)) = (decision_de(mejor), decision_de(peor)) else {
        return (false, "sin_orden");
    };
    if rango_decision(dm) != rango_decision(dp) {
        return (rango_decision(dm) > rango_decision(dp), "decision");
    }
    match (mejor.global, peor.global) {
        (Some(gm), Some(gp)) => (gm > gp, "global"),
        _ => match (completitud_de(mejor), completitud_de(peor)) {
            (Some(cm), Some(cp)) => (cm > cp, "completitud"),
            _ => (false, "sin_orden"),
        },
    }
}

/// `true` si el hallazgo cae en el mismo archivo y a no más de `TOLERANCIA_LINEAS` del defecto.
fn encuentra(v: &Veredicto, defecto: &DefectoReal) -> bool {
    let archivo = normaliza(&defecto.archivo);
    v.hallazgos.iter().any(|h| {
        normaliza(&h.archivo) == archivo && h.linea.abs_diff(defecto.linea) <= TOLERANCIA_LINEAS
    })
}

/// `true` si algún hallazgo descartado cae en el mismo archivo y a no más de `TOLERANCIA_LINEAS` del defecto.
/// Solo informa: no cuenta para `encuentra`.
fn descartado_cerca(v: &Veredicto, defecto: &DefectoReal) -> bool {
    let archivo = normaliza(&defecto.archivo);
    v.hallazgos_descartados.iter().any(|h| {
        normaliza(&h.archivo) == archivo && h.linea.abs_diff(defecto.linea) <= TOLERANCIA_LINEAS
    })
}

/// `Some(true)` si el `test` del defecto está en `ocultos_fallidos` o `contrato_fallidos`. `None` sin `test`.
/// Un test inestable no es rojo: ver `test_inestable`. Cuenta para `recall` y `recall_test`.
fn test_rojo(v: &Veredicto, defecto: &DefectoReal) -> Option<bool> {
    let test = defecto.test.as_deref()?;
    let en_lista = |lista: &[String]| lista.iter().any(|t| t == test);
    Some(en_lista(&v.ocultos_fallidos) || en_lista(&v.contrato_fallidos))
}

/// `true` si el `test` del defecto está en `inestables` o `inestables_contrato`. No cuenta como rojo.
fn test_inestable(v: &Veredicto, defecto: &DefectoReal) -> bool {
    let Some(test) = defecto.test.as_deref() else {
        return false;
    };
    let en_lista = |lista: &[String]| lista.iter().any(|t| t == test);
    en_lista(&v.inestables) || en_lista(&v.inestables_contrato)
}

/// `true` si la evidencia de algún ítem cita el archivo del defecto a `TOLERANCIA_LINEAS` líneas o menos.
/// Las citas que no tienen la forma `archivo:linea` se ignoran.
fn en_evidencia(v: &Veredicto, defecto: &DefectoReal) -> bool {
    let archivo = normaliza(&defecto.archivo);
    v.puntajes
        .iter()
        .flat_map(|p| p.items.iter())
        .flat_map(|n| n.evidencia.iter())
        .filter_map(|cita| cita.rsplit_once(':'))
        .any(|(ruta, linea)| {
            let Ok(linea) = linea.trim().parse::<u32>() else {
                return false;
            };
            normaliza(ruta) == archivo && linea.abs_diff(defecto.linea) <= TOLERANCIA_LINEAS
        })
}

/// Hallazgo del mismo archivo con menor distancia en línea. En empate, gana el primero de `v.hallazgos`.
fn hallazgo_mas_cercano(v: &Veredicto, defecto: &DefectoReal) -> Option<HallazgoCercano> {
    let archivo = normaliza(&defecto.archivo);
    v.hallazgos
        .iter()
        .filter(|h| normaliza(&h.archivo) == archivo)
        .min_by_key(|h| h.linea.abs_diff(defecto.linea))
        .map(|h| HallazgoCercano {
            linea: h.linea,
            fuente: h.fuente.clone(),
            distancia: h.linea.abs_diff(defecto.linea),
        })
}

fn normaliza(ruta: &str) -> &str {
    ruta.trim_start_matches("./")
}

/// Default de las métricas que no están en un `validez.json` viejo: sin medir, con motivo.
fn metrica_sin_medir_vieja() -> Metrica {
    Metrica::sin_medir("validez.json anterior: métrica no medida")
}

impl Metrica {
    /// Métrica sin medir: sin valor, sin casos y con el motivo.
    fn sin_medir(motivo: &str) -> Metrica {
        Metrica {
            valor: None,
            casos: 0,
            aciertos: None,
            umbral: None,
            cumple: None,
            motivo: Some(motivo.to_string()),
        }
    }

    /// Fracción de aciertos. Con `umbral`, cumple si la fracción llega a él.
    fn fraccion(aciertos: u32, casos: u32, umbral: Option<f32>) -> Metrica {
        let valor = aciertos as f32 / casos as f32;
        Metrica {
            valor: Some(valor),
            casos,
            aciertos: Some(aciertos),
            umbral,
            cumple: umbral.map(|u| valor >= u),
            motivo: None,
        }
    }
}

impl Kappa {
    fn sin_medir(clave: &str, umbral: Option<f32>, motivo: &str) -> Kappa {
        Kappa {
            clave: clave.to_string(),
            valor: None,
            casos: 0,
            umbral,
            cumple: None,
            motivo: Some(motivo.to_string()),
        }
    }

    fn de_pares(clave: &str, pares: &[(u8, u8)], umbral: Option<f32>) -> Kappa {
        match kappa_ponderado(pares) {
            Ok(k) => Kappa {
                clave: clave.to_string(),
                valor: Some(k as f32),
                casos: pares.len() as u32,
                umbral,
                cumple: umbral.map(|u| k as f32 >= u),
                motivo: None,
            },
            Err(motivo) => Kappa {
                casos: pares.len() as u32,
                ..Kappa::sin_medir(clave, umbral, motivo)
            },
        }
    }
}

fn kappas_dimension_sin_medir(motivo: &str) -> Vec<Kappa> {
    TODAS_LAS_DIMENSIONES
        .iter()
        .map(|d| Kappa::sin_medir(d.clave(), Some(UMBRAL_KAPPA), motivo))
        .collect()
}

/// Caché de veredictos por (tarea, ruta): cada repo se evalúa una sola vez.
/// Cada evaluación nueva se guarda en `veredictos`, la carpeta de veredictos de los casos.
struct Evaluaciones<'a> {
    evaluador: &'a dyn Fn(&Path, &str) -> Result<Veredicto, ErrorOraculo>,
    cache: BTreeMap<(String, PathBuf), Veredicto>,
    veredictos: PathBuf,
}

impl Evaluaciones<'_> {
    fn veredicto(&mut self, tarea: &str, ruta: &Path) -> Result<Veredicto, ErrorOraculo> {
        let clave = (tarea.to_string(), ruta.to_path_buf());
        if let Some(v) = self.cache.get(&clave) {
            return Ok(v.clone());
        }
        let v = (self.evaluador)(ruta, tarea)?;
        guardar_veredicto(&self.veredictos, ruta, &v)?;
        self.cache.insert(clave, v.clone());
        Ok(v)
    }
}

/// Escribe el veredicto de `ruta` en `<carpeta>/<nombre>.json`, de forma atómica.
fn guardar_veredicto(carpeta: &Path, ruta: &Path, v: &Veredicto) -> Result<(), ErrorOraculo> {
    fs::create_dir_all(carpeta).map_err(|e| io_infra("crear la carpeta de veredictos", &e))?;
    let texto = serde_json::to_string_pretty(v)
        .map_err(|e| ErrorOraculo::Infra(format!("serializar el veredicto: {e}")))?;
    let destino = carpeta.join(format!("{}.json", nombre_del_clon(ruta)));
    let mut temporal = tempfile::NamedTempFile::new_in(carpeta)
        .map_err(|e| io_infra("crear el temporal del veredicto", &e))?;
    temporal
        .write_all(format!("{texto}\n").as_bytes())
        .map_err(|e| io_infra("escribir el veredicto", &e))?;
    temporal
        .as_file()
        .sync_all()
        .map_err(|e| io_infra("escribir el veredicto", &e))?;
    temporal
        .persist(&destino)
        .map_err(|e| io_infra("guardar el veredicto", &e.error))?;
    Ok(())
}

/// Nombre del clon: el último componente de `ruta`. Un paquete ciego usa el id de su carpeta padre.
fn nombre_del_clon(ruta: &Path) -> String {
    let nombre = nombre_de(ruta);
    match ruta.parent() {
        Some(padre) if nombre == "repo" => nombre_de(padre),
        _ => nombre,
    }
}

fn nombre_de(ruta: &Path) -> String {
    ruta.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

/// Mide la exactitud de orden y devuelve un `DetallePar` por par, en el orden de `orden.toml`.
fn medir_orden(
    casos: &Path,
    pares: Option<&[ParOrden]>,
    ev: &mut Evaluaciones,
) -> Result<(Metrica, Vec<DetallePar>), ErrorOraculo> {
    let Some(pares) = pares else {
        return Ok((
            Metrica::sin_medir("falta orden.toml: sin pares de orden conocido"),
            Vec::new(),
        ));
    };
    if pares.is_empty() {
        return Ok((Metrica::sin_medir("orden.toml no tiene pares"), Vec::new()));
    }
    let mut aciertos = 0;
    let mut detalle = Vec::with_capacity(pares.len());
    for par in pares {
        let mejor = ev.veredicto(&par.tarea, &casos.join(&par.mejor))?;
        let peor = ev.veredicto(&par.tarea, &casos.join(&par.peor))?;
        let (acierto, criterio) = ordena(&mejor, &peor);
        if acierto {
            aciertos += 1;
        }
        detalle.push(DetallePar {
            tarea: par.tarea.clone(),
            mejor: par.mejor.clone(),
            peor: par.peor.clone(),
            acierto,
            decision_mejor: decision_de(&mejor),
            decision_peor: decision_de(&peor),
            global_mejor: mejor.global,
            global_peor: peor.global,
            completitud_mejor: completitud_de(&mejor),
            completitud_peor: completitud_de(&peor),
            criterio: criterio.to_string(),
        });
    }
    let metrica = Metrica::fraccion(aciertos, pares.len() as u32, Some(UMBRAL_ORDEN));
    Ok((metrica, detalle))
}

/// Mide el recall. Devuelve `recall` (hallazgo por línea o test rojo), `recall_linea`, `recall_test` y un
/// `DetalleDefecto` por defecto, en el orden de `defectos.toml`.
fn medir_recall(
    casos: &Path,
    defectos: Option<&[DefectoReal]>,
    ev: &mut Evaluaciones,
) -> Result<(Metrica, Metrica, Metrica, Vec<DetalleDefecto>), ErrorOraculo> {
    let Some(defectos) = defectos else {
        return Ok(recall_sin_medir("falta defectos.toml: sin defectos reales"));
    };
    if defectos.is_empty() {
        return Ok(recall_sin_medir("defectos.toml no tiene defectos"));
    }
    let mut encontrados = 0;
    let mut en_linea = 0;
    let mut en_test = 0;
    let mut detalle = Vec::with_capacity(defectos.len());
    for defecto in defectos {
        let v = ev.veredicto(&defecto.tarea, &casos.join(&defecto.repo))?;
        let encontrado = encuentra(&v, defecto);
        let rojo = test_rojo(&v, defecto);
        let en_rojo = rojo == Some(true);
        if encontrado || en_rojo {
            encontrados += 1;
        }
        en_linea += u32::from(encontrado);
        en_test += u32::from(en_rojo);
        detalle.push(DetalleDefecto {
            tarea: defecto.tarea.clone(),
            repo: defecto.repo.clone(),
            archivo: defecto.archivo.clone(),
            linea: defecto.linea,
            encontrado,
            en_evidencia: en_evidencia(&v, defecto),
            descartado_cerca: descartado_cerca(&v, defecto),
            hallazgo_cercano: hallazgo_mas_cercano(&v, defecto),
            test: defecto.test.clone(),
            test_rojo: rojo,
            test_inestable: test_inestable(&v, defecto),
            sonda_sin_veredicto: v.estado == Estado::SinVeredicto,
        });
    }
    let total = defectos.len() as u32;
    Ok((
        Metrica::fraccion(encontrados, total, Some(UMBRAL_RECALL)),
        Metrica::fraccion(en_linea, total, None),
        Metrica::fraccion(en_test, total, None),
        detalle,
    ))
}

/// Las tres medidas de recall sin medir, con el mismo motivo. Solo `recall` lleva umbral.
fn recall_sin_medir(motivo: &str) -> (Metrica, Metrica, Metrica, Vec<DetalleDefecto>) {
    (
        Metrica {
            umbral: Some(UMBRAL_RECALL),
            ..Metrica::sin_medir(motivo)
        },
        Metrica::sin_medir(motivo),
        Metrica::sin_medir(motivo),
        Vec::new(),
    )
}

/// Un paquete a ciegas ya evaluado por Root, con el formulario del usuario si lo hay.
struct Paquete {
    id: String,
    veredicto: Veredicto,
    formulario: Option<Formulario>,
}

impl Paquete {
    fn orden(&self) -> Option<u32> {
        self.formulario.as_ref().and_then(|f| f.orden)
    }
}

struct Formulario {
    decision: Option<Decision>,
    orden: Option<u32>,
    items: Vec<ItemFormulario>,
}

struct ItemFormulario {
    id: String,
    dimension: Dimension,
    nivel: Option<u8>,
}

/// Lee `casos/ciego/clave.toml` y evalúa cada paquete. `None` si no hay clave.
fn leer_paquetes(
    casos: &Path,
    ev: &mut Evaluaciones,
    avisos: &mut Vec<String>,
) -> Result<Option<Vec<Paquete>>, ErrorOraculo> {
    let carpeta = casos.join(CARPETA_CIEGO);
    let Some(texto) = leer_opcional(&carpeta.join(ARCHIVO_CLAVE))? else {
        return Ok(None);
    };
    let clave: ArchivoClave = toml::from_str(&texto)
        .map_err(|e| ErrorOraculo::Uso(format!("clave.toml inválida: {e}")))?;
    let mut ids = BTreeSet::new();
    let mut paquetes = Vec::new();
    for c in clave.paquete {
        let id_limpio = !c.id.is_empty() && c.id.chars().all(|ch| ch.is_ascii_alphanumeric());
        if !id_limpio || !ids.insert(c.id.clone()) {
            return Err(ErrorOraculo::Uso(format!(
                "clave.toml: id inválido o repetido '{}'",
                c.id
            )));
        }
        if c.tarea.trim().is_empty() || c.origen.trim().is_empty() || c.repo.trim().is_empty() {
            return Err(ErrorOraculo::Uso(format!(
                "clave.toml: el paquete {} necesita tarea, origen y repo",
                c.id
            )));
        }
        let dir = carpeta.join(&c.id);
        let veredicto = ev.veredicto(&c.tarea, &dir.join(CARPETA_REPO))?;
        let formulario = match leer_opcional(&dir.join(ARCHIVO_FORMULARIO))? {
            Some(texto) => Some(parsear_formulario(&texto, &c.id)?),
            None => {
                avisos.push(format!("falta el formulario del paquete {}", c.id));
                None
            }
        };
        paquetes.push(Paquete {
            id: c.id,
            veredicto,
            formulario,
        });
    }
    Ok(Some(paquetes))
}

/// Mide el acuerdo en la decisión: 5 de 6 cumple (decisión 13). Un paquete sin decisión del usuario no entra.
fn medir_decision(paquetes: &[Paquete], avisos: &mut Vec<String>) -> Metrica {
    let con: Vec<(&Paquete, Decision)> = paquetes
        .iter()
        .filter_map(|p| {
            p.formulario
                .as_ref()
                .and_then(|f| f.decision)
                .map(|d| (p, d))
        })
        .collect();
    if con.is_empty() {
        return Metrica::sin_medir("ningún formulario tiene decisión");
    }
    let total = con.len();
    let acuerdos = con
        .iter()
        .filter(|(p, d)| decision_de(&p.veredicto) == Some(*d))
        .count();
    if total != PAQUETES_ESPERADOS {
        avisos.push(format!(
            "acuerdo en la decisión: hay {total} formularios con decisión; el plan pide {PAQUETES_ESPERADOS}"
        ));
    }
    Metrica {
        valor: Some(acuerdos as f32 / total as f32),
        casos: total as u32,
        aciertos: Some(acuerdos as u32),
        umbral: None,
        cumple: Some(total == PAQUETES_ESPERADOS && acuerdos >= ACUERDOS_MINIMOS),
        motivo: None,
    }
}

/// Kappa por ítem y por dimensión. Un par entra solo si el usuario y Root dan nivel al ítem.
fn medir_kappas(paquetes: &[Paquete], avisos: &mut Vec<String>) -> (Vec<Kappa>, Vec<Kappa>) {
    let mut por_item: BTreeMap<String, (Dimension, Vec<(u8, u8)>)> = BTreeMap::new();
    for p in paquetes {
        let Some(f) = &p.formulario else {
            continue;
        };
        let (mut sin_usuario, mut sin_root) = (0, 0);
        for it in &f.items {
            let entrada = por_item
                .entry(it.id.clone())
                .or_insert_with(|| (it.dimension, Vec::new()));
            let Some(nivel_usuario) = it.nivel else {
                sin_usuario += 1;
                continue;
            };
            let Some(nivel_root) = nivel_de(&p.veredicto, &it.id) else {
                sin_root += 1;
                continue;
            };
            entrada.1.push((nivel_usuario, nivel_root));
        }
        if sin_usuario > 0 {
            avisos.push(format!(
                "paquete {}: {sin_usuario} ítems sin nivel; no entran al kappa",
                p.id
            ));
        }
        if sin_root > 0 {
            avisos.push(format!(
                "paquete {}: Root no dio nivel a {sin_root} ítems; no entran al kappa",
                p.id
            ));
        }
    }
    let items = por_item
        .iter()
        .map(|(id, (_, pares))| Kappa::de_pares(id, pares, None))
        .collect();
    let dimensiones = TODAS_LAS_DIMENSIONES
        .iter()
        .map(|&dim| {
            let hay_items = por_item.values().any(|(d, _)| *d == dim);
            if !hay_items {
                return Kappa::sin_medir(
                    dim.clave(),
                    Some(UMBRAL_KAPPA),
                    "ningún formulario tiene ítems de esta dimensión",
                );
            }
            let pares: Vec<(u8, u8)> = por_item
                .values()
                .filter(|(d, _)| *d == dim)
                .flat_map(|(_, p)| p.iter().copied())
                .collect();
            Kappa::de_pares(dim.clave(), &pares, Some(UMBRAL_KAPPA))
        })
        .collect();
    (dimensiones, items)
}

/// Spearman entre el `orden` del usuario (1 es el mejor) y el orden de Root. Root ordena por decisión y
/// luego por el global si todos los paquetes lo tienen, o por la completitud si no.
fn medir_spearman(paquetes: &[Paquete], avisos: &mut Vec<String>) -> Metrica {
    let con_orden: Vec<&Paquete> = paquetes
        .iter()
        .filter(|p| p.orden().is_some() && decision_de(&p.veredicto).is_some())
        .collect();
    if con_orden.len() < 2 {
        return Metrica::sin_medir("menos de dos formularios con orden y veredicto de Root");
    }
    let usa_global = con_orden.iter().all(|p| p.veredicto.global.is_some());
    let mut claves: Vec<(u8, f64)> = Vec::new();
    let mut usuario: Vec<f64> = Vec::new();
    for p in con_orden {
        let Some(decision) = decision_de(&p.veredicto) else {
            continue;
        };
        let score = if usa_global {
            p.veredicto.global.map(f64::from)
        } else {
            completitud_de(&p.veredicto).map(f64::from)
        };
        let Some(score) = score else {
            let que = if usa_global { "global" } else { "completitud" };
            avisos.push(format!(
                "paquete {}: sin {que} de Root; fuera del orden",
                p.id
            ));
            continue;
        };
        claves.push((rango_decision(decision), score));
        // Mayor valor, mejor: el orden del usuario se invierte para que 1 pese más.
        usuario.push(-f64::from(p.orden().unwrap_or(0)));
    }
    if claves.len() < 2 {
        return Metrica::sin_medir("menos de dos paquetes con orden y puntaje de Root");
    }
    let root = posiciones(&claves);
    let posicion_usuario = posiciones(&usuario);
    match spearman(&root, &posicion_usuario) {
        Some(r) => Metrica {
            valor: Some(r as f32),
            casos: claves.len() as u32,
            aciertos: None,
            umbral: None,
            cumple: None,
            motivo: None,
        },
        None => Metrica::sin_medir("el orden no varía: Spearman no está definido"),
    }
}

/// Un formulario ya leído: solo los campos que mide Root.
fn parsear_formulario(texto: &str, id_esperado: &str) -> Result<Formulario, ErrorOraculo> {
    let crudo: FormularioCrudo = toml::from_str(texto).map_err(|e| {
        ErrorOraculo::Uso(format!(
            "formulario del paquete {id_esperado} inválido: {e}"
        ))
    })?;
    if crudo.id != id_esperado {
        return Err(ErrorOraculo::Uso(format!(
            "el formulario de {id_esperado} dice ser de {}",
            crudo.id
        )));
    }
    let decision = match crudo.decision {
        None => None,
        Some(toml::Value::String(s)) if s.is_empty() => None,
        Some(toml::Value::String(s)) => Some(decision_desde(&s, id_esperado)?),
        Some(otro) => {
            return Err(ErrorOraculo::Uso(format!(
                "formulario de {id_esperado}: decision debe ser texto, no {otro}"
            )));
        }
    };
    let orden = match crudo.orden {
        None => None,
        Some(toml::Value::String(s)) if s.is_empty() => None,
        Some(toml::Value::Integer(n)) => {
            let posicion = u32::try_from(n).ok().filter(|p| *p >= 1).ok_or_else(|| {
                ErrorOraculo::Uso(format!(
                    "formulario de {id_esperado}: orden debe ser 1 o más"
                ))
            })?;
            Some(posicion)
        }
        Some(otro) => {
            return Err(ErrorOraculo::Uso(format!(
                "formulario de {id_esperado}: orden debe ser un entero o vacío, no {otro}"
            )));
        }
    };
    let mut items = Vec::new();
    for it in crudo.item {
        let dimension = Dimension::desde_clave(&it.dimension).ok_or_else(|| {
            ErrorOraculo::Uso(format!(
                "formulario de {id_esperado}: dimensión desconocida '{}'",
                it.dimension
            ))
        })?;
        let nivel = match it.nivel {
            None => None,
            Some(toml::Value::String(s)) if s.is_empty() => None,
            Some(toml::Value::Integer(n)) if (0..=i64::from(NIVEL_MAXIMO)).contains(&n) => {
                Some(n as u8)
            }
            Some(otro) => {
                return Err(ErrorOraculo::Uso(format!(
                    "formulario de {id_esperado}: el nivel de {} debe ser 0 a 3 o vacío, no {otro}",
                    it.id
                )));
            }
        };
        items.push(ItemFormulario {
            id: it.id,
            dimension,
            nivel,
        });
    }
    Ok(Formulario {
        decision,
        orden,
        items,
    })
}

fn decision_desde(texto: &str, id: &str) -> Result<Decision, ErrorOraculo> {
    match texto {
        "listo" => Ok(Decision::Listo),
        "no_listo" => Ok(Decision::NoListo),
        "bloqueado" => Ok(Decision::Bloqueado),
        otro => Err(ErrorOraculo::Uso(format!(
            "formulario de {id}: decision '{otro}' no existe; use listo, no_listo o bloqueado"
        ))),
    }
}

fn leer_orden(casos: &Path) -> Result<Option<Vec<ParOrden>>, ErrorOraculo> {
    let Some(texto) = leer_opcional(&casos.join(ARCHIVO_ORDEN))? else {
        return Ok(None);
    };
    let archivo: ArchivoOrden = toml::from_str(&texto)
        .map_err(|e| ErrorOraculo::Uso(format!("orden.toml inválido: {e}")))?;
    if archivo.par.iter().any(|p| p.fuente.trim().is_empty()) {
        return Err(ErrorOraculo::Uso(
            "orden.toml: cada [[par]] necesita `fuente`".into(),
        ));
    }
    Ok(Some(archivo.par))
}

fn leer_defectos(casos: &Path) -> Result<Option<Vec<DefectoReal>>, ErrorOraculo> {
    let Some(texto) = leer_opcional(&casos.join(ARCHIVO_DEFECTOS))? else {
        return Ok(None);
    };
    let archivo: ArchivoDefectos = toml::from_str(&texto)
        .map_err(|e| ErrorOraculo::Uso(format!("defectos.toml inválido: {e}")))?;
    if archivo
        .defecto
        .iter()
        .any(|d| d.fuente.trim().is_empty() || d.detalle.trim().is_empty())
    {
        return Err(ErrorOraculo::Uso(
            "defectos.toml: cada [[defecto]] necesita `fuente` y `detalle`".into(),
        ));
    }
    Ok(Some(archivo.defecto))
}

fn leer_opcional(ruta: &Path) -> Result<Option<String>, ErrorOraculo> {
    match fs::read_to_string(ruta) {
        Ok(texto) => Ok(Some(texto)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io_infra(&format!("leer {}", ruta.display()), &e)),
    }
}

fn leer_entradas(dir: &Path) -> Result<fs::ReadDir, ErrorOraculo> {
    fs::read_dir(dir).map_err(|e| io_infra(&format!("leer {}", dir.display()), &e))
}

fn io_infra(contexto: &str, e: &io::Error) -> ErrorOraculo {
    ErrorOraculo::Infra(format!("{contexto}: {e}"))
}

/// Texto de los ítems del formulario: anclas como comentario y un `[[item]]` con su nivel vacío.
fn texto_formulario_items(rubricas: &[Rubrica]) -> String {
    let mut texto = String::new();
    for rubrica in rubricas {
        for item in &rubrica.item {
            let bloquea = if item.bloquea { ", bloquea" } else { "" };
            texto += &format!("\n# {} (peso {}{bloquea})\n", item.id, item.peso);
            for ancla in &item.anclas {
                texto += &format!("# {}\n", ancla.replace('\n', " "));
            }
            texto += &format!(
                "[[item]]\nid = \"{}\"\ndimension = \"{}\"\nnivel = \"\"\n",
                item.id, rubrica.id
            );
        }
    }
    texto
}

fn formulario_texto(id: &str, items: &str) -> String {
    format!(
        "# Formulario a ciegas (Tarea 12b). Califica solo el paquete a la vista.\n\
         # decision: \"listo\", \"no_listo\" o \"bloqueado\".\n\
         # orden: posición de este paquete entre los del lote. 1 es el mejor.\n\
         # nivel: 0 a 3, según las anclas de cada ítem (comentarios).\n\
         id = \"{id}\"\n\
         decision = \"\"\n\
         orden = \"\"\n\
         {items}"
    )
}

/// Copia `origen` a `destino` sin los rastros de `EXCLUIDOS_PAQUETE` ni los enlaces simbólicos.
fn copiar_paquete(origen: &Path, destino: &Path) -> io::Result<()> {
    fs::create_dir_all(destino)?;
    for entrada in fs::read_dir(origen)? {
        let entrada = entrada?;
        let nombre = entrada.file_name();
        let tipo = entrada.file_type()?;
        if tipo.is_symlink() || excluido(&nombre.to_string_lossy()) {
            continue;
        }
        let dest = destino.join(&nombre);
        if tipo.is_dir() {
            copiar_paquete(&entrada.path(), &dest)?;
        } else if tipo.is_file() {
            fs::copy(entrada.path(), &dest)?;
        }
    }
    Ok(())
}

fn excluido(nombre: &str) -> bool {
    EXCLUIDOS_PAQUETE.contains(&nombre) || nombre.starts_with(".ecc") || nombre.ends_with(".log")
}

/// PRNG splitmix64 propio: los ids salen de la semilla, sin crates nuevos.
struct Azar(u64);

impl Azar {
    fn siguiente(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// Palabras que delatan el origen de un paquete: ECC, SDLC y los modelos (desempate 45). Se buscan como
/// subcadena y sin distinguir mayúsculas: `opus2` cuenta (desempate 19).
const PALABRAS_DE_RASTRO: [&str; 6] = ["ecc", "sdlc", "claude", "haiku", "sonnet", "opus"];

/// Claves de `[package]` y de `[workspace.package]` que nombran al autor o al repo (desempate 45).
const CLAVES_DE_AUTOR: [&str; 3] = ["authors", "repository", "homepage"];

/// Lista de rastros de un paquete: `$ROOT_HOME/validez/rastros-<id>.toml`. Fuera de `casos/ciego/`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct ArchivoRastros {
    /// Lo que el armado no pudo revisar del todo, por ejemplo un `Cargo.toml` sin parsear.
    #[serde(default)]
    avisos: Vec<String>,
    #[serde(default)]
    aparicion: Vec<Aparicion>,
}

/// Una aparición de una palabra de `PALABRAS_DE_RASTRO` en un archivo copiado.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Aparicion {
    /// Ruta relativa a la copia del repo, con `/`.
    archivo: String,
    /// Línea 1-based.
    linea: u32,
    /// La palabra tal como aparece en el archivo.
    palabra: String,
}

/// Revisa `actual` (dentro de la copia `raiz`): limpia los `Cargo.toml` y anota cada rastro de texto.
/// Los binarios (con bytes nulos o no UTF-8) no se revisan.
fn revisar_copia(raiz: &Path, actual: &Path, salida: &mut ArchivoRastros) -> io::Result<()> {
    let mut entradas: Vec<fs::DirEntry> = fs::read_dir(actual)?.collect::<io::Result<_>>()?;
    entradas.sort_by_key(fs::DirEntry::file_name);
    for entrada in entradas {
        let ruta = entrada.path();
        let tipo = entrada.file_type()?;
        if tipo.is_dir() {
            revisar_copia(raiz, &ruta, salida)?;
            continue;
        }
        if !tipo.is_file() {
            continue;
        }
        let relativa = ruta
            .strip_prefix(raiz)
            .unwrap_or(&ruta)
            .to_string_lossy()
            .replace('\\', "/");
        let mut bytes = fs::read(&ruta)?;
        if entrada.file_name() == "Cargo.toml" {
            match std::str::from_utf8(&bytes).ok().map(limpiar_cargo_toml) {
                Some(Ok(limpio)) => {
                    fs::write(&ruta, &limpio)?;
                    bytes = limpio.into_bytes();
                }
                _ => salida.avisos.push(format!(
                    "{relativa}: no parsea como TOML; no se borraron authors, repository ni homepage"
                )),
            }
        }
        if let Some(texto) = texto_de(&bytes) {
            for (linea, palabra) in buscar_palabras(texto) {
                salida.aparicion.push(Aparicion {
                    archivo: relativa.clone(),
                    linea,
                    palabra,
                });
            }
        }
    }
    Ok(())
}

/// El texto del archivo, o `None` si es binario: tiene bytes nulos o no es UTF-8.
fn texto_de(bytes: &[u8]) -> Option<&str> {
    if bytes.contains(&0) {
        return None;
    }
    std::str::from_utf8(bytes).ok()
}

/// Reescribe `texto` sin `authors`, `repository` y `homepage` de `[package]` y de `[workspace.package]`.
/// Siempre reescribe, aunque no quite nada: el formato no delata el origen (desempate 20). `Err` si no
/// parsea.
fn limpiar_cargo_toml(texto: &str) -> Result<String, String> {
    let mut tabla: toml::Table = toml::from_str(texto).map_err(|e| e.to_string())?;
    if let Some(paquete) = tabla.get_mut("package").and_then(toml::Value::as_table_mut) {
        quitar_claves_de_autor(paquete);
    }
    if let Some(paquete) = tabla
        .get_mut("workspace")
        .and_then(toml::Value::as_table_mut)
        .and_then(|ws| ws.get_mut("package"))
        .and_then(toml::Value::as_table_mut)
    {
        quitar_claves_de_autor(paquete);
    }
    toml::to_string(&tabla).map_err(|e| e.to_string())
}

fn quitar_claves_de_autor(tabla: &mut toml::Table) {
    for clave in CLAVES_DE_AUTOR {
        tabla.remove(clave);
    }
}

/// Apariciones de `PALABRAS_DE_RASTRO` en `texto`: `(línea 1-based, palabra)`, en orden de aparición. Cuenta
/// la subcadena, sin distinguir mayúsculas: `opus2` y `recclamo` cuentan (desempate 19).
fn buscar_palabras(texto: &str) -> Vec<(u32, String)> {
    let mut hallazgos = Vec::new();
    for (indice, linea) in texto.lines().enumerate() {
        for (pos, _) in linea.char_indices() {
            for palabra in PALABRAS_DE_RASTRO {
                let Some(hallada) = linea.get(pos..pos + palabra.len()) else {
                    continue;
                };
                if hallada.eq_ignore_ascii_case(palabra) {
                    hallazgos.push((indice as u32 + 1, hallada.to_string()));
                }
            }
        }
    }
    hallazgos
}

/// Ficheros de la copia a ciegas que no se copian: rastros de ECC y de SDLC, git, logs y artefactos de build.
/// Ver `docs/interfaces.md`, sección "Validez".
const EXCLUIDOS_PAQUETE: &[&str] = &[
    "AGENTS.md",
    "CLAUDE.md",
    "sdlc.toml",
    ".claude",
    ".git",
    "logs",
    "target",
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchivoOrden {
    #[serde(default)]
    par: Vec<ParOrden>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ParOrden {
    tarea: String,
    mejor: String,
    peor: String,
    fuente: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchivoDefectos {
    #[serde(default)]
    defecto: Vec<DefectoReal>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DefectoReal {
    tarea: String,
    repo: String,
    archivo: String,
    linea: u32,
    fuente: String,
    detalle: String,
    /// Test que marca el defecto. Opcional: los `defectos.toml` viejos no lo traen.
    #[serde(default)]
    test: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchivoManifiesto {
    #[serde(default)]
    fuente: Vec<FuenteManifiesto>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FuenteManifiesto {
    tarea: String,
    origen: String,
    repo: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FormularioCrudo {
    id: String,
    decision: Option<toml::Value>,
    orden: Option<toml::Value>,
    #[serde(default)]
    item: Vec<ItemCrudo>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ItemCrudo {
    id: String,
    dimension: String,
    nivel: Option<toml::Value>,
}

/// Clave de `casos/ciego/clave.toml`: cómo se mapea cada id a su origen. Solo la lee Root.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct ArchivoClave {
    #[serde(default)]
    paquete: Vec<ClavePaquete>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ClavePaquete {
    id: String,
    tarea: String,
    origen: String,
    repo: String,
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use tempfile::TempDir;

    use super::*;
    use crate::tipos::{Confianza, Fuente, Hallazgo, Informe, NivelItem, Puntaje};

    const SEGURIDAD: &str = r#"
id = "seguridad"
version = 1

[[item]]
id = "seguridad.secretos"
fuente = "fixture: OWASP ASVS 5.0 V13"
mide = "sonda"
peso = 3
bloquea = true
anclas = [
  "0: hay un secreto escrito en el código",
  "1: hay un secreto en un archivo de prueba",
  "2: hay una variable sensible sin valor fijo",
  "3: no hay secretos en el código",
]
"#;

    const DISENO: &str = r#"
id = "diseno"
version = 1

[[item]]
id = "diseno.modularidad"
fuente = "fixture: ISO/IEC 25010 mantenibilidad"
mide = "juez"
peso = 3
bloquea = false
anclas = [
  "0: un módulo concentra varios requisitos",
  "1: hay módulos con dependencias cruzadas",
  "2: cada requisito vive en un módulo con interfaz clara",
  "3: módulos con una responsabilidad y sin ciclos",
]
"#;

    fn rubricas() -> Vec<Rubrica> {
        vec![
            Rubrica::desde_toml(SEGURIDAD).expect("rúbrica de fixture"),
            Rubrica::desde_toml(DISENO).expect("rúbrica de fixture"),
        ]
    }

    fn escribir(ruta: &Path, texto: &str) {
        fs::create_dir_all(ruta.parent().expect("ruta con carpeta")).expect("crea carpeta");
        fs::write(ruta, texto).expect("escribe fixture");
    }

    fn informe(decision: Decision) -> Informe {
        Informe {
            avisos_previos: Vec::new(),
            decision,
            motivo: String::new(),
            requisitos: Vec::new(),
            bloqueantes: Vec::new(),
            riesgos: Vec::new(),
            proximos_pasos: Vec::new(),
            resumen: String::new(),
            confianza: Confianza::default(),
        }
    }

    fn puntaje(dimension: Dimension, valor: f32, items: Vec<NivelItem>) -> Puntaje {
        Puntaje {
            inestables: Vec::new(),
            motivo_sin_calibrar: None,
            dimension,
            valor,
            calibrada: true,
            exactitud_balanceada: None,
            items,
            sin_veredicto: None,
            faltantes: Vec::new(),
        }
    }

    /// Veredicto armado. `niveles` pone un ítem por par, con su nivel de Root.
    fn veredicto(
        decision: Option<Decision>,
        global: Option<f32>,
        completitud: Option<f32>,
        niveles: &[(&str, u8)],
    ) -> Veredicto {
        let mut v = Veredicto {
            informe: decision.map(informe),
            global,
            ..Veredicto::default()
        };
        if let Some(valor) = completitud {
            v.puntajes
                .push(puntaje(Dimension::Completitud, valor, Vec::new()));
        }
        for (item, nivel) in niveles {
            let clave = item.split('.').next().expect("ítem con prefijo");
            let dimension = Dimension::desde_clave(clave).expect("dimensión de fixture");
            let nivel_item = NivelItem {
                item: (*item).to_string(),
                nivel: *nivel,
                evidencia: Vec::new(),
                fuente: Fuente::Sonda("rubrica".into()),
            };
            v.puntajes.push(puntaje(dimension, 0.5, vec![nivel_item]));
        }
        v
    }

    fn hallazgo(archivo: &str, linea: u32) -> Hallazgo {
        Hallazgo {
            dimension: Dimension::Seguridad,
            severidad: 2,
            archivo: archivo.into(),
            linea,
            cita: String::new(),
            detalle: String::new(),
            fuente: Fuente::Sonda("fixture".into()),
            id: String::new(),
            impacto: Vec::new(),
            previo: false,
        }
    }

    /// Evaluador falso: devuelve el veredicto cuya clave es el final de la ruta. Sin clave, error.
    fn falso(
        mapa: Vec<(String, Veredicto)>,
    ) -> impl Fn(&Path, &str) -> Result<Veredicto, ErrorOraculo> {
        move |ruta: &Path, _tarea: &str| {
            mapa.iter()
                .find(|(clave, _)| ruta.ends_with(clave))
                .map(|(_, v)| v.clone())
                .ok_or_else(|| ErrorOraculo::Infra(format!("sin fixture para {}", ruta.display())))
        }
    }

    fn clave(texto: &str, v: Veredicto) -> (String, Veredicto) {
        (texto.to_string(), v)
    }

    /// Repo de fixture con rastros de ECC: AGENTS.md, sdlc.toml, .claude/, .git/, logs, target y más.
    fn repo_con_rastros(ruta: &Path) {
        escribir(&ruta.join("src/lib.rs"), "pub fn f() -> u8 { 1 }\n");
        escribir(&ruta.join("README.md"), "# fixture\n");
        escribir(&ruta.join("AGENTS.md"), "instrucciones de ECC\n");
        escribir(&ruta.join("sdlc.toml"), "[sdlc]\n");
        escribir(&ruta.join("CLAUDE.md"), "reglas\n");
        escribir(&ruta.join(".claude/settings.json"), "{}\n");
        escribir(&ruta.join(".git/HEAD"), "ref: refs/heads/main\n");
        escribir(&ruta.join("logs/run.txt"), "corrida\n");
        escribir(&ruta.join("build.log"), "log\n");
        escribir(&ruta.join("target/debug/bin"), "binario\n");
        escribir(&ruta.join(".ecc-cache/x"), "caché\n");
    }

    /// Prepara seis paquetes a ciegas de un mismo repo de fixture. Devuelve la carpeta de casos y los ids.
    fn preparar_seis(dir: &Path) -> (PathBuf, Vec<String>) {
        let casos = dir.join("casos");
        let repo = dir.join("repos").join("fuente");
        repo_con_rastros(&repo);
        let mut manifiesto = String::new();
        for i in 0..6 {
            manifiesto += &format!(
                "[[fuente]]\ntarea = \"memora\"\norigen = \"origen-{i}\"\nrepo = \"{}\"\n\n",
                repo.display()
            );
        }
        let ruta = dir.join("fuentes.toml");
        escribir(&ruta, &manifiesto);
        let validez = dir.join("validez");
        let p = preparar_ciego_con(&casos, &ruta, 7, &rubricas(), &validez)
            .expect("prepara seis paquetes");
        (casos, p.paquetes)
    }

    /// Llena un formulario como lo haría la persona. `orden` y `nivel` son TOML crudo: `""` o un número.
    fn llenar(
        casos: &Path,
        id: &str,
        decision: &str,
        orden: &str,
        nivel_secretos: &str,
        nivel_diseno: &str,
    ) {
        let texto = format!(
            "id = \"{id}\"\ndecision = \"{decision}\"\norden = {orden}\n\n\
             [[item]]\nid = \"seguridad.secretos\"\ndimension = \"seguridad\"\nnivel = {nivel_secretos}\n\n\
             [[item]]\nid = \"diseno.modularidad\"\ndimension = \"diseno\"\nnivel = {nivel_diseno}\n"
        );
        escribir(
            &casos.join("ciego").join(id).join("formulario.toml"),
            &texto,
        );
    }

    fn clave_de_ciego(id: &str) -> String {
        format!("ciego/{id}/repo")
    }

    fn kappa_de<'a>(lista: &'a [Kappa], clave: &str) -> &'a Kappa {
        lista
            .iter()
            .find(|k| k.clave == clave)
            .expect("kappa presente")
    }

    fn cerca(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    /// Lee un veredicto guardado en disco y lo deserializa. Falla si el archivo no existe.
    fn leer_veredicto(ruta: &Path) -> Veredicto {
        let texto = fs::read_to_string(ruta).expect("lee el veredicto guardado");
        serde_json::from_str::<Veredicto>(&texto).expect("deserializa el veredicto")
    }

    #[test]
    fn validez_guarda_el_veredicto_de_cada_repo() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path();
        escribir(
            &casos.join("orden.toml"),
            "[[par]]\ntarea = \"memora\"\nmejor = \"a\"\npeor = \"b\"\nfuente = \"fixture: resultado medido\"\n",
        );
        let va = veredicto(Some(Decision::Listo), None, Some(0.9), &[]);
        let vb = veredicto(Some(Decision::NoListo), None, Some(0.4), &[]);
        let mapa = falso(vec![clave("a", va.clone()), clave("b", vb.clone())]);
        validar_con(casos, &mapa).expect("mide");
        assert_eq!(leer_veredicto(&casos.join("veredictos/a.json")), va);
        assert_eq!(leer_veredicto(&casos.join("veredictos/b.json")), vb);
    }

    #[test]
    fn validez_veredicto_de_paquete_ciego_usa_el_id() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path();
        escribir(
            &casos.join("orden.toml"),
            "[[par]]\ntarea = \"memora\"\nmejor = \"ciego/p1/repo\"\npeor = \"ciego/p2/repo\"\nfuente = \"fixture: resultado medido\"\n",
        );
        let v1 = veredicto(Some(Decision::Listo), None, Some(0.9), &[]);
        let v2 = veredicto(Some(Decision::NoListo), None, Some(0.4), &[]);
        let mapa = falso(vec![
            clave("ciego/p1/repo", v1.clone()),
            clave("ciego/p2/repo", v2.clone()),
        ]);
        validar_con(casos, &mapa).expect("mide");
        assert_eq!(leer_veredicto(&casos.join("veredictos/p1.json")), v1);
        assert_eq!(leer_veredicto(&casos.join("veredictos/p2.json")), v2);
        assert!(!casos.join("veredictos/repo.json").exists());
    }

    #[test]
    fn validez_orden_acierta_2_de_3_pares() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path();
        escribir(
            &casos.join("orden.toml"),
            r#"
[[par]]
tarea = "memora"
mejor = "a"
peor = "b"
fuente = "fixture: resultado medido"

[[par]]
tarea = "memora"
mejor = "c"
peor = "d"
fuente = "fixture: resultado medido"

[[par]]
tarea = "memora"
mejor = "e"
peor = "f"
fuente = "fixture: resultado medido"
"#,
        );
        let mapa = falso(vec![
            clave("a", veredicto(Some(Decision::Listo), None, Some(0.9), &[])),
            clave(
                "b",
                veredicto(Some(Decision::NoListo), None, Some(0.4), &[]),
            ),
            clave(
                "c",
                veredicto(Some(Decision::NoListo), None, Some(0.7), &[]),
            ),
            clave(
                "d",
                veredicto(Some(Decision::NoListo), None, Some(0.3), &[]),
            ),
            clave(
                "e",
                veredicto(Some(Decision::NoListo), None, Some(0.5), &[]),
            ),
            clave(
                "f",
                veredicto(Some(Decision::NoListo), None, Some(0.8), &[]),
            ),
        ]);
        let v = validar_con(casos, &mapa).expect("mide");
        assert_eq!(v.exactitud_orden.aciertos, Some(2));
        assert_eq!(v.exactitud_orden.casos, 3);
        assert!(cerca(v.exactitud_orden.valor.expect("medida"), 2.0 / 3.0));
        assert_eq!(
            v.exactitud_orden.cumple,
            Some(false),
            "2 de 3 no llega al umbral 1,0 (desempate 44)"
        );
    }

    #[test]
    fn validez_orden_empate_o_sin_veredicto_cuenta_como_fallo() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path();
        escribir(
            &casos.join("orden.toml"),
            r#"
[[par]]
tarea = "memora"
mejor = "g"
peor = "h"
fuente = "fixture"

[[par]]
tarea = "memora"
mejor = "i"
peor = "j"
fuente = "fixture"

[[par]]
tarea = "memora"
mejor = "k"
peor = "l"
fuente = "fixture"
"#,
        );
        let mapa = falso(vec![
            clave("g", veredicto(Some(Decision::Listo), Some(0.5), None, &[])),
            clave("h", veredicto(Some(Decision::Listo), Some(0.5), None, &[])),
            clave("i", veredicto(Some(Decision::Listo), None, None, &[])),
            clave("j", veredicto(None, None, None, &[])),
            clave("k", veredicto(Some(Decision::Listo), Some(0.9), None, &[])),
            clave(
                "l",
                veredicto(Some(Decision::Bloqueado), Some(0.1), None, &[]),
            ),
        ]);
        let v = validar_con(casos, &mapa).expect("mide");
        assert_eq!(
            v.exactitud_orden.aciertos,
            Some(1),
            "empate y sin veredicto son fallos"
        );
        assert_eq!(v.exactitud_orden.casos, 3);
    }

    #[test]
    fn validez_orden_tres_pares_bien_ordenados_cumplen() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path();
        escribir(
            &casos.join("orden.toml"),
            "[[par]]\ntarea = \"memora\"\nmejor = \"a\"\npeor = \"b\"\nfuente = \"fixture\"\n\n\
             [[par]]\ntarea = \"memora\"\nmejor = \"c\"\npeor = \"d\"\nfuente = \"fixture\"\n\n\
             [[par]]\ntarea = \"memora\"\nmejor = \"e\"\npeor = \"f\"\nfuente = \"fixture\"\n",
        );
        let mapa = falso(vec![
            clave("a", veredicto(Some(Decision::Listo), None, Some(0.9), &[])),
            clave(
                "b",
                veredicto(Some(Decision::NoListo), None, Some(0.4), &[]),
            ),
            clave(
                "c",
                veredicto(Some(Decision::NoListo), None, Some(0.7), &[]),
            ),
            clave(
                "d",
                veredicto(Some(Decision::NoListo), None, Some(0.3), &[]),
            ),
            clave("e", veredicto(Some(Decision::Listo), None, Some(0.8), &[])),
            clave(
                "f",
                veredicto(Some(Decision::NoListo), None, Some(0.5), &[]),
            ),
        ]);
        let v = validar_con(casos, &mapa).expect("mide");
        assert_eq!(v.exactitud_orden.aciertos, Some(3));
        assert_eq!(v.exactitud_orden.umbral, Some(1.0));
        assert_eq!(v.exactitud_orden.cumple, Some(true));
    }

    #[test]
    fn validez_orden_sin_pares_no_cumple_ni_incumple() {
        let dir = TempDir::new().expect("temporal");
        let sin_archivo = validar_con(dir.path(), &falso(Vec::new())).expect("mide");
        assert_eq!(sin_archivo.exactitud_orden.valor, None);
        assert_eq!(sin_archivo.exactitud_orden.cumple, None);
        escribir(&dir.path().join("orden.toml"), "");
        let vacio = validar_con(dir.path(), &falso(Vec::new())).expect("mide");
        assert_eq!(vacio.exactitud_orden.valor, None);
        assert_eq!(
            vacio.exactitud_orden.cumple, None,
            "sin pares no hay veredicto"
        );
    }

    #[test]
    fn validez_recall_3_de_4_con_tolerancia_de_3_lineas() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path();
        let mut defectos = String::new();
        for linea in [10, 20, 30, 40] {
            defectos += &format!(
                "[[defecto]]\ntarea = \"memora\"\nrepo = \"r\"\narchivo = \"src/a.rs\"\nlinea = {linea}\n\
                 fuente = \"revisión fixture\"\ndetalle = \"defecto fixture\"\n\n"
            );
        }
        escribir(&casos.join("defectos.toml"), &defectos);
        // Hallazgos en 10 (exacto), 22 (+2), 27 (-3, en el borde) y 45 (+5, fuera).
        let v_root = Veredicto {
            hallazgos: vec![
                hallazgo("src/a.rs", 10),
                hallazgo("src/a.rs", 22),
                hallazgo("src/a.rs", 27),
                hallazgo("src/a.rs", 45),
            ],
            ..Veredicto::default()
        };
        let v = validar_con(casos, &falso(vec![clave("r", v_root)])).expect("mide");
        assert_eq!(v.recall.aciertos, Some(3));
        assert_eq!(v.recall.casos, 4);
        assert!(cerca(v.recall.valor.expect("medida"), 0.75));
        assert_eq!(v.recall.umbral, Some(UMBRAL_RECALL));
        assert_eq!(v.recall.cumple, Some(false));
    }

    #[test]
    fn validez_recall_en_0_8_cumple_el_umbral() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path();
        let mut defectos = String::new();
        for linea in [10, 20, 30, 40, 50] {
            defectos += &format!(
                "[[defecto]]\ntarea = \"memora\"\nrepo = \"r\"\narchivo = \"src/a.rs\"\nlinea = {linea}\n\
                 fuente = \"revisión fixture\"\ndetalle = \"defecto fixture\"\n\n"
            );
        }
        escribir(&casos.join("defectos.toml"), &defectos);
        let v_root = Veredicto {
            hallazgos: [10, 20, 30, 40].map(|l| hallazgo("src/a.rs", l)).to_vec(),
            ..Veredicto::default()
        };
        let v = validar_con(casos, &falso(vec![clave("r", v_root)])).expect("mide");
        assert_eq!(v.recall.aciertos, Some(4));
        assert_eq!(v.recall.cumple, Some(true), "0,8 cumple el umbral");
    }

    #[test]
    fn validez_kappa_niveles_iguales_dan_1() {
        let pares = [(0, 0), (1, 1), (2, 2), (3, 3), (2, 2), (1, 1)];
        let k = kappa_ponderado(&pares).expect("kappa definido");
        assert!((k - 1.0).abs() < 1e-9, "kappa {k}");
    }

    #[test]
    fn validez_kappa_caso_a_mano_da_0_9() {
        // Pares (usuario, Root): (0,0), (1,2), (2,2), (3,3). N = 4.
        // Pesos d(i,j) = (i-j)^2 / 9. Observado: solo (1,2) discrepa, D_o = (1/9) / 4 = 1/36.
        // Marginales: a = [1,1,1,1] (usuario), b = [1,0,2,1] (Root).
        // Esperado: sum d(i,j) a_i b_j = 40/9, D_e = (40/9) / 16 = 5/18.
        // kappa = 1 - D_o / D_e = 1 - (1/36) / (5/18) = 1 - 1/10 = 0,9.
        let k = kappa_ponderado(&[(0, 0), (1, 2), (2, 2), (3, 3)]).expect("kappa definido");
        assert!((k - 0.9).abs() < 1e-9, "kappa {k}");
    }

    #[test]
    fn validez_kappa_sin_variacion_queda_sin_medir() {
        assert!(kappa_ponderado(&[(2, 2), (2, 2), (2, 2)]).is_err());
        assert!(kappa_ponderado(&[]).is_err());
    }

    #[test]
    fn validez_decision_5_de_6_cumple() {
        let dir = TempDir::new().expect("temporal");
        let (casos, ids) = preparar_seis(dir.path());
        let mut mapa = Vec::new();
        for (i, id) in ids.iter().enumerate() {
            llenar(&casos, id, "listo", "\"\"", "\"\"", "\"\"");
            let decision = if i < 5 {
                Decision::Listo
            } else {
                Decision::NoListo
            };
            mapa.push(clave(
                &clave_de_ciego(id),
                veredicto(Some(decision), None, None, &[]),
            ));
        }
        let v = validar_con(&casos, &falso(mapa)).expect("mide");
        assert_eq!(v.acuerdo_decision.aciertos, Some(5));
        assert_eq!(v.acuerdo_decision.casos, 6);
        assert_eq!(v.acuerdo_decision.cumple, Some(true));
    }

    #[test]
    fn validez_decision_4_de_6_no_cumple() {
        let dir = TempDir::new().expect("temporal");
        let (casos, ids) = preparar_seis(dir.path());
        let mut mapa = Vec::new();
        for (i, id) in ids.iter().enumerate() {
            llenar(&casos, id, "listo", "\"\"", "\"\"", "\"\"");
            let decision = if i < 4 {
                Decision::Listo
            } else {
                Decision::Bloqueado
            };
            mapa.push(clave(
                &clave_de_ciego(id),
                veredicto(Some(decision), None, None, &[]),
            ));
        }
        let v = validar_con(&casos, &falso(mapa)).expect("mide");
        assert_eq!(v.acuerdo_decision.aciertos, Some(4));
        assert_eq!(v.acuerdo_decision.cumple, Some(false));
    }

    #[test]
    fn validez_spearman_orden_igual_da_1() {
        let s = spearman(
            &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
            &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
        )
        .expect("definido");
        assert!((s - 1.0).abs() < 1e-9, "spearman {s}");
    }

    #[test]
    fn validez_spearman_orden_invertido_da_menos_1() {
        let s = spearman(
            &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
            &[6.0, 5.0, 4.0, 3.0, 2.0, 1.0],
        )
        .expect("definido");
        assert!((s + 1.0).abs() < 1e-9, "spearman {s}");
    }

    #[test]
    fn validez_sin_fuentes_todo_queda_sin_medir() {
        let dir = TempDir::new().expect("temporal");
        let v = validar_con(dir.path(), &falso(Vec::new())).expect("mide");
        for m in [
            &v.exactitud_orden,
            &v.recall,
            &v.recall_linea,
            &v.recall_test,
            &v.acuerdo_decision,
            &v.spearman,
        ] {
            assert_eq!(m.valor, None, "sin fuente no hay valor");
            assert!(m.motivo.is_some(), "sin fuente hay motivo");
            assert_eq!(m.cumple, None);
        }
        assert_eq!(v.recall_linea.umbral, None, "recall_linea no tiene umbral");
        assert_eq!(v.recall_test.umbral, None, "recall_test no tiene umbral");
        assert_eq!(v.kappa_dimension.len(), TODAS_LAS_DIMENSIONES.len());
        assert!(
            v.kappa_dimension
                .iter()
                .all(|k| k.valor.is_none() && k.motivo.is_some())
        );
        assert!(v.kappa_item.is_empty());
    }

    #[test]
    fn validez_evaluador_con_error_aborta_la_medicion() {
        let dir = TempDir::new().expect("temporal");
        escribir(
            &dir.path().join("orden.toml"),
            "[[par]]\ntarea = \"memora\"\nmejor = \"x\"\npeor = \"y\"\nfuente = \"fixture\"\n",
        );
        // Sin fixture para `x`, el evaluador falla: no hay veredicto y la medición no sale.
        assert!(validar_con(dir.path(), &falso(Vec::new())).is_err());
    }

    #[test]
    fn validez_preparar_ciego_arma_paquetes_anonimos() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path().join("casos");
        let repo_ecc = dir.path().join("repos").join("memora-ecc");
        let repo_sdlc = dir.path().join("repos").join("brasa-sdlc");
        repo_con_rastros(&repo_ecc);
        repo_con_rastros(&repo_sdlc);
        let manifiesto = dir.path().join("fuentes.toml");
        escribir(
            &manifiesto,
            &format!(
                "[[fuente]]\ntarea = \"memora\"\norigen = \"ecc-sonnet\"\nrepo = \"{}\"\n\n\
                 [[fuente]]\ntarea = \"brasa\"\norigen = \"sdlc\"\nrepo = \"{}\"\n",
                repo_ecc.display(),
                repo_sdlc.display()
            ),
        );

        let validez = dir.path().join("validez");

        let p =
            preparar_ciego_con(&casos, &manifiesto, 42, &rubricas(), &validez).expect("prepara");
        assert_eq!(p.paquetes.len(), 2);
        assert_ne!(p.paquetes[0], p.paquetes[1], "los ids son distintos");
        assert!(!p.sin_rubricas, "con rúbricas de fixture no hay aviso");

        for id in &p.paquetes {
            assert_eq!(id.len(), 8, "id de 8 caracteres");
            assert!(id.chars().all(|c| c.is_ascii_hexdigit()), "id hexadecimal");
            let paquete = casos.join("ciego").join(id);
            let copia = paquete.join("repo");
            assert!(copia.join("src/lib.rs").is_file(), "el código sí se copia");
            assert!(copia.join("README.md").is_file(), "el README sí se copia");
            for rastro in [
                "AGENTS.md",
                "sdlc.toml",
                "CLAUDE.md",
                ".claude",
                ".git",
                "logs",
                "build.log",
                "target",
                ".ecc-cache",
            ] {
                assert!(!copia.join(rastro).exists(), "{rastro} no debe copiarse");
            }
            let texto = fs::read_to_string(paquete.join("formulario.toml")).expect("formulario");
            assert!(texto.contains("[[item]]"), "el formulario lista ítems");
            assert!(texto.contains("seguridad.secretos") && texto.contains("diseno.modularidad"));
            assert!(
                texto.contains("0: hay un secreto escrito en el código"),
                "lleva las anclas"
            );
            assert!(texto.contains("decision = \"\""), "decisión vacía");
            assert!(
                !texto.contains("ecc-sonnet")
                    && !texto.contains("sdlc")
                    && !texto.contains("memora")
            );
        }

        let clave_texto =
            fs::read_to_string(casos.join("ciego").join(ARCHIVO_CLAVE)).expect("clave");
        let clave: ArchivoClave = toml::from_str(&clave_texto).expect("clave válida");
        assert_eq!(clave.paquete.len(), 2);
        for c in &clave.paquete {
            assert!(!c.id.contains(&c.origen), "el id no contiene el origen");
            let esperado = if c.origen == "ecc-sonnet" {
                "memora"
            } else {
                "brasa"
            };
            assert_eq!(c.tarea, esperado, "la clave mapea id a tarea");
        }
        let ecc = clave
            .paquete
            .iter()
            .find(|c| c.origen == "ecc-sonnet")
            .expect("origen ecc");
        assert!(p.paquetes.contains(&ecc.id));
    }

    /// Repo con un `Cargo.toml` de autor (raíz con `[workspace.package]` y un miembro) y un README con ECC.
    fn repo_con_cargo_y_readme(ruta: &Path) {
        escribir(
            &ruta.join("Cargo.toml"),
            "[package]\nname = \"memora\"\nversion = \"0.1.0\"\nauthors = [\"Ana <ana@ecc.dev>\"]\n\
             repository = \"https://github.com/ecc/memora\"\nhomepage = \"https://ecc.dev\"\n\n\
             [workspace.package]\nauthors = [\"Ana\"]\nlicense = \"MIT\"\n\n[dependencies]\n",
        );
        escribir(
            &ruta.join("crates/uno/Cargo.toml"),
            "[package]\nname = \"uno\"\nversion = \"0.2.0\"\nauthors = [\"Ana\"]\n",
        );
        escribir(&ruta.join("src/lib.rs"), "pub fn f() -> u8 { 1 }\n");
        escribir(&ruta.join("README.md"), "# Memora\n\nHecho con ECC\n");
    }

    /// Manifiesto con una sola fuente, `repo` como repo bueno. Devuelve su ruta.
    fn manifiesto_de_una_fuente(dir: &Path, repo: &Path) -> PathBuf {
        let ruta = dir.join("fuentes.toml");
        escribir(
            &ruta,
            &format!(
                "[[fuente]]\ntarea = \"memora\"\norigen = \"ecc-sonnet\"\nrepo = \"{}\"\n",
                repo.display()
            ),
        );
        ruta
    }

    /// Lee `rastros-<id>.toml` del armado como tabla TOML. Las listas viven en la carpeta de validez.
    fn rastros_de(validez: &Path, id: &str) -> toml::Table {
        let ruta = validez.join(format!("rastros-{id}.toml"));
        let texto = fs::read_to_string(&ruta).expect("el paquete tiene su lista de rastros");
        toml::from_str(&texto).expect("la lista es TOML")
    }

    /// Apariciones de una lista de rastros como (archivo, línea, palabra).
    fn apariciones(tabla: &toml::Table) -> Vec<(String, i64, String)> {
        let Some(lista) = tabla.get("aparicion").and_then(toml::Value::as_array) else {
            return Vec::new();
        };
        lista
            .iter()
            .map(|a| {
                let t = a.as_table().expect("aparición con campos");
                (
                    t["archivo"].as_str().expect("archivo").to_string(),
                    t["linea"].as_integer().expect("linea"),
                    t["palabra"].as_str().expect("palabra").to_string(),
                )
            })
            .collect()
    }

    #[test]
    fn validez_rastros_cargo_pierde_authors_repository_y_homepage() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path().join("casos");
        let repo = dir.path().join("repos").join("memora");
        repo_con_cargo_y_readme(&repo);
        let manifiesto = manifiesto_de_una_fuente(dir.path(), &repo);
        let validez = dir.path().join("validez");
        let p =
            preparar_ciego_con(&casos, &manifiesto, 42, &rubricas(), &validez).expect("prepara");
        let copia = casos.join("ciego").join(&p.paquetes[0]).join("repo");

        let raiz: toml::Table =
            toml::from_str(&fs::read_to_string(copia.join("Cargo.toml")).expect("raíz"))
                .expect("TOML");
        let paquete = raiz["package"].as_table().expect("[package]");
        for clave in ["authors", "repository", "homepage"] {
            assert!(!paquete.contains_key(clave), "la raíz no trae {clave}");
        }
        assert_eq!(paquete["name"].as_str(), Some("memora"));
        assert_eq!(paquete["version"].as_str(), Some("0.1.0"));
        let workspace = raiz["workspace"]["package"]
            .as_table()
            .expect("[workspace.package]");
        assert!(
            !workspace.contains_key("authors"),
            "[workspace.package] tampoco"
        );
        assert_eq!(workspace["license"].as_str(), Some("MIT"), "lo demás queda");

        let miembro: toml::Table = toml::from_str(
            &fs::read_to_string(copia.join("crates/uno/Cargo.toml")).expect("miembro"),
        )
        .expect("TOML");
        let paquete = miembro["package"].as_table().expect("[package]");
        assert!(
            !paquete.contains_key("authors"),
            "el miembro tampoco trae authors"
        );
        assert_eq!(paquete["name"].as_str(), Some("uno"));
        assert_eq!(paquete["version"].as_str(), Some("0.2.0"));
    }

    #[test]
    fn validez_rastros_readme_con_ecc_sale_con_archivo_y_linea() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path().join("casos");
        let repo = dir.path().join("repos").join("memora");
        repo_con_cargo_y_readme(&repo);
        let manifiesto = manifiesto_de_una_fuente(dir.path(), &repo);
        let validez = dir.path().join("validez");
        let p =
            preparar_ciego_con(&casos, &manifiesto, 42, &rubricas(), &validez).expect("prepara");
        let apar = apariciones(&rastros_de(&validez, &p.paquetes[0]));
        assert!(
            apar.contains(&("README.md".to_string(), 3, "ECC".to_string())),
            "el README sale con su línea: {apar:?}"
        );
        assert!(
            apar.iter().all(|(archivo, _, _)| archivo != "src/lib.rs"),
            "un archivo sin palabras no sale: {apar:?}"
        );
        assert!(
            apar.iter().all(|(archivo, _, _)| archivo != "Cargo.toml"),
            "los autores ya se borraron antes de revisar: {apar:?}"
        );
    }

    #[test]
    fn validez_rastros_recclamo_cuenta_como_ecc() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path().join("casos");
        let repo = dir.path().join("repos").join("fuente");
        escribir(&repo.join("notas.md"), "recclamo, y ecc.\n");
        let manifiesto = manifiesto_de_una_fuente(dir.path(), &repo);
        let validez = dir.path().join("validez");
        let p =
            preparar_ciego_con(&casos, &manifiesto, 42, &rubricas(), &validez).expect("prepara");
        let apar = apariciones(&rastros_de(&validez, &p.paquetes[0]));
        assert_eq!(
            apar,
            vec![
                ("notas.md".to_string(), 1, "ecc".to_string()),
                ("notas.md".to_string(), 1, "ecc".to_string()),
            ],
            "la subcadena cuenta: recclamo y el ecc suelto"
        );
    }

    #[test]
    fn validez_rastros_cargo_sin_claves_de_autor_se_reescribe_sin_comentarios() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path().join("casos");
        let repo = dir.path().join("repos").join("fuente");
        escribir(
            &repo.join("Cargo.toml"),
            "# Comentario del autor\n[package]\nname = \"sinautor\"\nversion = \"0.3.0\"\n",
        );
        let manifiesto = manifiesto_de_una_fuente(dir.path(), &repo);
        let validez = dir.path().join("validez");
        let p =
            preparar_ciego_con(&casos, &manifiesto, 42, &rubricas(), &validez).expect("prepara");
        let copia = casos.join("ciego").join(&p.paquetes[0]).join("repo");
        let texto = fs::read_to_string(copia.join("Cargo.toml")).expect("copiado");
        assert!(
            !texto.contains("Comentario"),
            "sin authors también se reescribe y pierde el comentario: {texto}"
        );
        let tabla: toml::Table = toml::from_str(&texto).expect("sigue siendo TOML");
        assert_eq!(tabla["package"]["name"].as_str(), Some("sinautor"));
    }

    #[test]
    fn validez_rastros_cargo_sin_parsear_queda_como_esta_y_se_anota() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path().join("casos");
        let repo = dir.path().join("repos").join("fuente");
        let roto = "[package\nname = \"roto\"\n";
        escribir(&repo.join("Cargo.toml"), roto);
        let manifiesto = manifiesto_de_una_fuente(dir.path(), &repo);
        let validez = dir.path().join("validez");
        let p =
            preparar_ciego_con(&casos, &manifiesto, 42, &rubricas(), &validez).expect("prepara");
        let copia = casos.join("ciego").join(&p.paquetes[0]).join("repo");
        assert_eq!(
            fs::read_to_string(copia.join("Cargo.toml")).expect("copiado"),
            roto,
            "un Cargo.toml que no parsea no se toca"
        );
        let tabla = rastros_de(&validez, &p.paquetes[0]);
        let avisos = tabla
            .get("avisos")
            .and_then(toml::Value::as_array)
            .expect("hay avisos");
        assert!(
            avisos
                .iter()
                .any(|a| a.as_str().is_some_and(|t| t.contains("Cargo.toml"))),
            "el aviso nombra el archivo: {avisos:?}"
        );
    }

    #[test]
    fn validez_rastros_binario_no_se_revisa() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path().join("casos");
        let repo = dir.path().join("repos").join("fuente");
        fs::create_dir_all(&repo).expect("repo");
        fs::write(repo.join("datos.bin"), b"\x00ECC\x00").expect("binario con nulos");
        fs::write(repo.join("latin.txt"), [0xff, b'E', b'C', b'C', b'\n']).expect("no UTF-8");
        let manifiesto = manifiesto_de_una_fuente(dir.path(), &repo);
        let validez = dir.path().join("validez");
        let p =
            preparar_ciego_con(&casos, &manifiesto, 42, &rubricas(), &validez).expect("prepara");
        let apar = apariciones(&rastros_de(&validez, &p.paquetes[0]));
        assert!(apar.is_empty(), "los binarios no se revisan: {apar:?}");
    }

    #[test]
    fn validez_rastros_subcadena_sin_distinguir_mayusculas() {
        let texto = "Hecho con ECC; Claude, sonnet-4 y Opus.\nopus2, xopus y recclamo cuentan.\n";
        assert_eq!(
            buscar_palabras(texto),
            vec![
                (1, "ECC".to_string()),
                (1, "Claude".to_string()),
                (1, "sonnet".to_string()),
                (1, "Opus".to_string()),
                (2, "opus".to_string()),
                (2, "opus".to_string()),
                (2, "ecc".to_string()),
            ]
        );
    }

    #[test]
    fn validez_rastros_buscar_palabras_encuentra_opus_pegado_y_sdlc_con_guion_bajo() {
        let hallazgos = buscar_palabras("modelo opus2 y SDLC_v1");
        assert!(
            hallazgos.contains(&(1, "opus".to_string())),
            "opus pegado a un dígito cuenta: {hallazgos:?}"
        );
        assert!(
            hallazgos.contains(&(1, "SDLC".to_string())),
            "SDLC seguido de _ cuenta: {hallazgos:?}"
        );
    }

    #[test]
    fn validez_preparar_ciego_rechaza_carpeta_existente() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path().join("casos");
        escribir(&casos.join("ciego").join("previo.txt"), "de otra corrida\n");
        let repo = dir.path().join("repos").join("fuente");
        repo_con_rastros(&repo);
        let manifiesto = dir.path().join("fuentes.toml");
        escribir(
            &manifiesto,
            &format!(
                "[[fuente]]\ntarea = \"memora\"\norigen = \"ecc\"\nrepo = \"{}\"\n",
                repo.display()
            ),
        );
        let validez = dir.path().join("validez");
        assert!(preparar_ciego_con(&casos, &manifiesto, 1, &rubricas(), &validez).is_err());
    }

    #[test]
    fn validez_preparar_ciego_no_deja_listas_de_rastros_en_la_carpeta_de_los_paquetes() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path().join("casos");
        let home = dir.path().join("home");
        let repo = dir.path().join("repos").join("fuente");
        repo_con_rastros(&repo);
        let manifiesto = manifiesto_de_una_fuente(dir.path(), &repo);
        let p = preparar_ciego(&casos, &manifiesto, 42, &home).expect("prepara");

        let en_ciego: Vec<String> = fs::read_dir(casos.join("ciego"))
            .expect("carpeta de paquetes")
            .map(|e| {
                e.expect("entrada")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert!(
            en_ciego.iter().all(|n| !n.starts_with("rastros-")),
            "casos/ciego no tiene listas de rastros: {en_ciego:?}"
        );
        for id in &p.paquetes {
            let lista = home.join("validez").join(format!("rastros-{id}.toml"));
            assert!(
                lista.is_file(),
                "la lista va a validez/: {}",
                lista.display()
            );
        }
    }

    #[test]
    fn validez_rastros_json_nombra_listas_que_existen_en_validez() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path().join("casos");
        let home = dir.path().join("home");
        let repo = dir.path().join("repos").join("fuente");
        repo_con_rastros(&repo);
        let manifiesto = manifiesto_de_una_fuente(dir.path(), &repo);
        preparar_ciego(&casos, &manifiesto, 42, &home).expect("prepara");

        let texto = fs::read_to_string(ruta_rastros_en(&home)).expect("rastros.json");
        let lista: Vec<ResumenRastros> = serde_json::from_str(&texto).expect("rastros.json válido");
        assert_eq!(lista.len(), 1, "un paquete, una entrada: {texto}");
        for resumen in &lista {
            assert!(
                home.join("validez").join(&resumen.archivo).is_file(),
                "{} existe en validez/",
                resumen.archivo
            );
        }
    }

    #[test]
    fn validez_formulario_a_medio_llenar_no_cuenta_y_avisa() {
        let dir = TempDir::new().expect("temporal");
        let (casos, ids) = preparar_seis(dir.path());
        let mut mapa = Vec::new();
        for (i, id) in ids.iter().enumerate() {
            let nivel_root = u8::try_from(i % 4).expect("nivel");
            if i < 4 {
                // Cuatro formularios con nivel del usuario, distinto por paquete.
                llenar(&casos, id, "listo", "\"\"", &nivel_root.to_string(), "\"\"");
            } else {
                // Dos a medio llenar: el nivel del ítem queda vacío.
                llenar(&casos, id, "listo", "\"\"", "\"\"", "\"\"");
            }
            mapa.push(clave(
                &clave_de_ciego(id),
                veredicto(
                    Some(Decision::Listo),
                    None,
                    None,
                    &[
                        ("seguridad.secretos", nivel_root),
                        ("diseno.modularidad", 1),
                    ],
                ),
            ));
        }
        let v = validar_con(&casos, &falso(mapa)).expect("mide");
        let secretos = kappa_de(&v.kappa_item, "seguridad.secretos");
        assert_eq!(
            secretos.casos, 4,
            "solo los cuatro formularios llenos entran"
        );
        assert!(cerca(secretos.valor.expect("kappa de los llenos"), 1.0));
        assert!(
            v.avisos.iter().any(|a| a.contains("sin nivel")),
            "el aviso dice que faltan niveles: {:?}",
            v.avisos
        );
    }

    /// Busca el detalle del par cuyo `mejor` es `mejor`. Falla si no hay.
    fn par_de<'a>(pares: &'a [DetallePar], mejor: &str) -> &'a DetallePar {
        pares
            .iter()
            .find(|p| p.mejor == mejor)
            .expect("par en el detalle")
    }

    #[test]
    fn validez_pares_identifica_el_par_fallido() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path();
        escribir(
            &casos.join("orden.toml"),
            "[[par]]\ntarea = \"memora\"\nmejor = \"a\"\npeor = \"b\"\nfuente = \"fixture\"\n\n\
             [[par]]\ntarea = \"memora\"\nmejor = \"c\"\npeor = \"d\"\nfuente = \"fixture\"\n",
        );
        // El primer par falla: `a` tiene global menor que `b`. El segundo ordena bien.
        let mapa = falso(vec![
            clave(
                "a",
                veredicto(Some(Decision::Listo), Some(0.5), Some(0.7), &[]),
            ),
            clave(
                "b",
                veredicto(Some(Decision::Listo), Some(0.9), Some(0.6), &[]),
            ),
            clave(
                "c",
                veredicto(Some(Decision::Listo), Some(0.9), Some(0.8), &[]),
            ),
            clave(
                "d",
                veredicto(Some(Decision::Listo), Some(0.4), Some(0.2), &[]),
            ),
        ]);
        let v = validar_con(casos, &mapa).expect("mide");
        assert_eq!(v.pares.len(), 2, "un detalle por par");
        let fallido = par_de(&v.pares, "a");
        assert_eq!(fallido.tarea, "memora");
        assert_eq!(fallido.peor, "b");
        assert!(!fallido.acierto);
        assert_eq!(fallido.decision_mejor, Some(Decision::Listo));
        assert_eq!(fallido.decision_peor, Some(Decision::Listo));
        assert_eq!(fallido.global_mejor, Some(0.5));
        assert_eq!(fallido.global_peor, Some(0.9));
        assert_eq!(fallido.completitud_mejor, Some(0.7));
        assert_eq!(fallido.completitud_peor, Some(0.6));
        assert_eq!(fallido.criterio, "global");
        let bien = par_de(&v.pares, "c");
        assert!(bien.acierto, "el segundo par ordena bien");
        assert_eq!(bien.criterio, "global");
    }

    #[test]
    fn validez_pares_criterio_decision_completitud_y_sin_orden() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path();
        escribir(
            &casos.join("orden.toml"),
            "[[par]]\ntarea = \"memora\"\nmejor = \"d1\"\npeor = \"d2\"\nfuente = \"fixture\"\n\n\
             [[par]]\ntarea = \"memora\"\nmejor = \"g1\"\npeor = \"g2\"\nfuente = \"fixture\"\n\n\
             [[par]]\ntarea = \"memora\"\nmejor = \"c1\"\npeor = \"c2\"\nfuente = \"fixture\"\n\n\
             [[par]]\ntarea = \"memora\"\nmejor = \"s1\"\npeor = \"s2\"\nfuente = \"fixture\"\n",
        );
        let mapa = falso(vec![
            // Decisión distinta: `Listo` pesa más que `NoListo`, aunque el global sea menor.
            clave(
                "d1",
                veredicto(Some(Decision::Listo), Some(0.1), Some(0.1), &[]),
            ),
            clave(
                "d2",
                veredicto(Some(Decision::NoListo), Some(0.9), Some(0.9), &[]),
            ),
            // Misma decisión: manda el global.
            clave(
                "g1",
                veredicto(Some(Decision::Listo), Some(0.6), Some(0.2), &[]),
            ),
            clave(
                "g2",
                veredicto(Some(Decision::Listo), Some(0.4), Some(0.9), &[]),
            ),
            // Sin global en ninguno: manda la completitud.
            clave("c1", veredicto(Some(Decision::Listo), None, Some(0.8), &[])),
            clave("c2", veredicto(Some(Decision::Listo), None, Some(0.3), &[])),
            // Sin decisión del mejor: sin orden, y el par falla.
            clave("s1", veredicto(None, Some(0.9), Some(0.9), &[])),
            clave(
                "s2",
                veredicto(Some(Decision::Listo), Some(0.1), Some(0.1), &[]),
            ),
        ]);
        let v = validar_con(casos, &mapa).expect("mide");
        let d = par_de(&v.pares, "d1");
        assert_eq!(d.criterio, "decision");
        assert!(d.acierto);
        let g = par_de(&v.pares, "g1");
        assert_eq!(g.criterio, "global");
        assert!(g.acierto);
        let c = par_de(&v.pares, "c1");
        assert_eq!(c.criterio, "completitud");
        assert!(c.acierto);
        assert_eq!(c.global_mejor, None);
        let s = par_de(&v.pares, "s1");
        assert_eq!(s.criterio, "sin_orden");
        assert!(!s.acierto, "sin orden el par falla");
        assert_eq!(s.decision_mejor, None);
        assert_eq!(s.decision_peor, Some(Decision::Listo));
    }

    #[test]
    fn validez_pares_no_cambia_exactitud_orden() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path();
        escribir(
            &casos.join("orden.toml"),
            "[[par]]\ntarea = \"memora\"\nmejor = \"x1\"\npeor = \"x2\"\nfuente = \"fixture\"\n\n\
             [[par]]\ntarea = \"memora\"\nmejor = \"y1\"\npeor = \"y2\"\nfuente = \"fixture\"\n\n\
             [[par]]\ntarea = \"memora\"\nmejor = \"z1\"\npeor = \"z2\"\nfuente = \"fixture\"\n",
        );
        // Un acierto por decisión, un fallo por global y un empate por global (cuenta como fallo).
        let mapa = falso(vec![
            clave("x1", veredicto(Some(Decision::Listo), Some(0.9), None, &[])),
            clave(
                "x2",
                veredicto(Some(Decision::NoListo), Some(0.1), None, &[]),
            ),
            clave("y1", veredicto(Some(Decision::Listo), Some(0.3), None, &[])),
            clave("y2", veredicto(Some(Decision::Listo), Some(0.8), None, &[])),
            clave("z1", veredicto(Some(Decision::Listo), Some(0.5), None, &[])),
            clave("z2", veredicto(Some(Decision::Listo), Some(0.5), None, &[])),
        ]);
        let v = validar_con(casos, &mapa).expect("mide");
        assert_eq!(v.exactitud_orden.aciertos, Some(1));
        assert_eq!(v.exactitud_orden.casos, 3);
        assert!(cerca(v.exactitud_orden.valor.expect("medida"), 1.0 / 3.0));
        assert_eq!(v.exactitud_orden.cumple, Some(false));
        assert_eq!(v.pares.len(), 3);
        let aciertos_pares = v.pares.iter().filter(|p| p.acierto).count() as u32;
        assert_eq!(Some(aciertos_pares), v.exactitud_orden.aciertos);
    }

    #[test]
    fn validez_viejo_sin_pares_lee_vacio() {
        let dir = TempDir::new().expect("temporal");
        let v = validar_con(dir.path(), &falso(Vec::new())).expect("mide");
        let mut json = serde_json::to_value(&v).expect("serializa");
        let mapa = json.as_object_mut().expect("objeto");
        assert!(
            mapa.remove("pares").is_some(),
            "la clave pares se serializa"
        );
        let viejo: Validez = serde_json::from_value(json).expect("lee un validez sin pares");
        assert!(viejo.pares.is_empty());
        assert_eq!(viejo.exactitud_orden, v.exactitud_orden);
    }

    /// Escribe `defectos.toml` con un `[[defecto]]` por `(archivo, linea)`, todos de `memora` en `r`.
    fn escribir_defectos(casos: &Path, defectos: &[(&str, u32)]) {
        let mut texto = String::new();
        for (archivo, linea) in defectos {
            texto += &format!(
                "[[defecto]]\ntarea = \"memora\"\nrepo = \"r\"\narchivo = \"{archivo}\"\nlinea = {linea}\n\
                 fuente = \"revisión fixture\"\ndetalle = \"defecto fixture\"\n\n"
            );
        }
        escribir(&casos.join("defectos.toml"), &texto);
    }

    #[test]
    fn validez_defectos_identifica_el_encontrado_y_el_perdido() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path();
        escribir_defectos(casos, &[("src/a.rs", 20), ("src/a.rs", 40)]);
        // El hallazgo de 22 queda a 2 líneas del primer defecto. El de 50 queda a 10 del segundo.
        let mut juez = hallazgo("src/a.rs", 22);
        juez.fuente = Fuente::Juez {
            modelo: "modelo-fixture".into(),
        };
        let v_root = Veredicto {
            hallazgos: vec![juez, hallazgo("src/a.rs", 50)],
            ..Veredicto::default()
        };
        let v = validar_con(casos, &falso(vec![clave("r", v_root)])).expect("mide");
        assert_eq!(v.defectos.len(), 2, "un detalle por defecto");
        let encontrado = &v.defectos[0];
        assert!(
            encontrado.encontrado,
            "el hallazgo de 22 cubre el defecto de 20"
        );
        assert_eq!(encontrado.tarea, "memora");
        assert_eq!(encontrado.repo, "r");
        assert_eq!(encontrado.archivo, "src/a.rs");
        assert_eq!(encontrado.linea, 20);
        assert_eq!(
            encontrado.hallazgo_cercano,
            Some(HallazgoCercano {
                linea: 22,
                fuente: Fuente::Juez {
                    modelo: "modelo-fixture".into(),
                },
                distancia: 2,
            })
        );
        let perdido = &v.defectos[1];
        assert!(!perdido.encontrado, "el hallazgo de 50 queda fuera de ±3");
        assert_eq!(perdido.linea, 40);
        assert_eq!(
            perdido.hallazgo_cercano,
            Some(HallazgoCercano {
                linea: 50,
                fuente: Fuente::Sonda("fixture".into()),
                distancia: 10,
            })
        );
    }

    #[test]
    fn validez_defectos_sin_hallazgo_en_el_archivo_da_null() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path();
        escribir_defectos(casos, &[("src/a.rs", 20)]);
        // El hallazgo cae en otro archivo: no es cercano al defecto.
        let v_root = Veredicto {
            hallazgos: vec![hallazgo("src/b.rs", 20)],
            ..Veredicto::default()
        };
        let v = validar_con(casos, &falso(vec![clave("r", v_root)])).expect("mide");
        let detalle = &v.defectos[0];
        assert!(!detalle.encontrado);
        assert_eq!(detalle.hallazgo_cercano, None);
        let json = serde_json::to_value(detalle).expect("serializa");
        assert_eq!(
            json.get("hallazgo_cercano"),
            Some(&serde_json::Value::Null),
            "la clave existe con null"
        );
    }

    #[test]
    fn validez_defectos_elige_el_mas_cercano() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path();
        escribir_defectos(casos, &[("src/a.rs", 20)]);
        // El de 14 va primero y queda a 6. El de 19 queda a 1. El de otro archivo, a 0, no cuenta.
        let v_root = Veredicto {
            hallazgos: vec![
                hallazgo("src/a.rs", 14),
                hallazgo("src/otro.rs", 20),
                hallazgo("src/a.rs", 19),
            ],
            ..Veredicto::default()
        };
        let v = validar_con(casos, &falso(vec![clave("r", v_root)])).expect("mide");
        let cercano = v.defectos[0]
            .hallazgo_cercano
            .clone()
            .expect("hay hallazgo en el archivo");
        assert_eq!(cercano.linea, 19);
        assert_eq!(cercano.distancia, 1);
    }

    #[test]
    fn validez_defectos_no_cambia_recall() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path();
        escribir_defectos(
            casos,
            &[("src/a.rs", 20), ("src/a.rs", 40), ("src/b.rs", 20)],
        );
        // Caso mixto: uno encontrado, uno con hallazgo lejano y uno sin hallazgo en su archivo.
        let v_root = Veredicto {
            hallazgos: vec![
                hallazgo("src/a.rs", 22),
                hallazgo("src/a.rs", 50),
                hallazgo("src/c.rs", 20),
            ],
            ..Veredicto::default()
        };
        let v = validar_con(casos, &falso(vec![clave("r", v_root)])).expect("mide");
        let encontrados = v.defectos.iter().filter(|d| d.encontrado).count() as u32;
        assert_eq!(encontrados, 1);
        assert_eq!(v.recall.aciertos, Some(encontrados));
        assert_eq!(v.recall.casos, v.defectos.len() as u32);
        assert!(cerca(v.recall.valor.expect("medida"), 1.0 / 3.0));
    }

    #[test]
    fn validez_viejo_sin_defectos_lee_vacio() {
        let dir = TempDir::new().expect("temporal");
        let v = validar_con(dir.path(), &falso(Vec::new())).expect("mide");
        let mut json = serde_json::to_value(&v).expect("serializa");
        let mapa = json.as_object_mut().expect("objeto");
        assert!(
            mapa.remove("defectos").is_some(),
            "la clave defectos se serializa"
        );
        let viejo: Validez = serde_json::from_value(json).expect("lee un validez sin defectos");
        assert!(viejo.defectos.is_empty());
        assert_eq!(viejo.recall, v.recall);
    }

    #[test]
    fn validez_evidencia_cercana_sin_hallazgo_no_es_encontrado() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path();
        escribir_defectos(casos, &[("src/a.rs", 100)]);
        // Sin hallazgos. La evidencia cita una línea a 1 del defecto.
        let mut v_root = veredicto(None, None, None, &[("seguridad.secretos", 2)]);
        v_root.puntajes[0].items[0].evidencia = vec!["src/a.rs:101".into()];
        let v = validar_con(casos, &falso(vec![clave("r", v_root)])).expect("mide");
        let detalle = &v.defectos[0];
        assert!(!detalle.encontrado, "sin hallazgo no hay encontrado");
        assert!(detalle.en_evidencia, "la cita cae a 1 línea del defecto");
        assert_eq!(v.recall.aciertos, Some(0));
    }

    #[test]
    fn validez_evidencia_lejana_u_otro_archivo_no_cuenta() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path();
        escribir_defectos(casos, &[("src/a.rs", 100)]);
        // Ninguna cita cae a 3 líneas o menos del defecto en su archivo. La última no tiene forma archivo:linea.
        let mut v_root = veredicto(None, None, None, &[("seguridad.secretos", 2)]);
        v_root.puntajes[0].items[0].evidencia = vec![
            "src/a.rs:110".into(),
            "src/b.rs:100".into(),
            "test_x::t: pasa".into(),
        ];
        let v = validar_con(casos, &falso(vec![clave("r", v_root)])).expect("mide");
        assert!(!v.defectos[0].en_evidencia);
    }

    #[test]
    fn validez_evidencia_acepta_prefijo_y_borde() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path();
        escribir_defectos(casos, &[("src/a.rs", 100)]);
        let correr = |cita: &str| {
            let mut v_root = veredicto(None, None, None, &[("seguridad.secretos", 2)]);
            v_root.puntajes[0].items[0].evidencia = vec![cita.into()];
            validar_con(casos, &falso(vec![clave("r", v_root)])).expect("mide")
        };
        // El borde son 3 líneas: cuenta. Una más queda fuera.
        assert!(correr("./src/a.rs:103").defectos[0].en_evidencia);
        assert!(!correr("src/a.rs:104").defectos[0].en_evidencia);
    }

    #[test]
    fn validez_evidencia_no_cambia_recall() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path();
        escribir_defectos(casos, &[("src/a.rs", 100)]);
        // Hallazgo a 3 líneas y sin evidencia: sigue siendo encontrado.
        let v_root = Veredicto {
            hallazgos: vec![hallazgo("src/a.rs", 103)],
            ..Veredicto::default()
        };
        let v = validar_con(casos, &falso(vec![clave("r", v_root)])).expect("mide");
        assert!(v.defectos[0].encontrado);
        assert!(!v.defectos[0].en_evidencia, "sin evidencia no hay cita");
        assert_eq!(v.recall.valor, Some(1.0));
        assert_eq!(v.recall.aciertos, Some(1));
    }

    #[test]
    fn validez_viejo_defecto_sin_en_evidencia_lee_false() {
        let dir = TempDir::new().expect("temporal");
        escribir_defectos(dir.path(), &[("src/a.rs", 20)]);
        let v =
            validar_con(dir.path(), &falso(vec![clave("r", Veredicto::default())])).expect("mide");
        let mut json = serde_json::to_value(&v.defectos[0]).expect("serializa");
        assert!(
            json.as_object_mut()
                .expect("objeto")
                .remove("en_evidencia")
                .is_some(),
            "la clave en_evidencia se serializa"
        );
        let viejo: DetalleDefecto =
            serde_json::from_value(json).expect("lee un defecto sin en_evidencia");
        assert!(!viejo.en_evidencia);
    }

    /// Veredicto de Root con un hallazgo descartado en `archivo` a `linea`. Sin hallazgos.
    fn con_descartado(archivo: &str, linea: u32) -> Veredicto {
        Veredicto {
            hallazgos_descartados: vec![crate::tipos::HallazgoDescartado {
                archivo: archivo.into(),
                linea,
                item: "seguridad.secretos".into(),
                motivo: "sin requisito afectado".into(),
            }],
            ..Veredicto::default()
        }
    }

    #[test]
    fn validez_descartado_cerca_informa_sin_contar_recall() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path();
        escribir_defectos(casos, &[("src/a.rs", 100)]);
        // Un descartado a 2 líneas del defecto y ningún hallazgo.
        let v = validar_con(
            casos,
            &falso(vec![clave("r", con_descartado("src/a.rs", 102))]),
        )
        .expect("mide");
        let detalle = &v.defectos[0];
        assert!(!detalle.encontrado, "un descartado no es encontrado");
        assert!(detalle.descartado_cerca, "el descartado cae a 2 líneas");
        assert_eq!(
            v.recall.aciertos,
            Some(0),
            "un descartado no cuenta en recall"
        );
    }

    #[test]
    fn validez_descartado_a_10_lineas_no_es_cerca() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path();
        escribir_defectos(casos, &[("src/a.rs", 100)]);
        let v = validar_con(
            casos,
            &falso(vec![clave("r", con_descartado("src/a.rs", 110))]),
        )
        .expect("mide");
        assert!(!v.defectos[0].descartado_cerca, "a 10 líneas queda fuera");
    }

    #[test]
    fn validez_descartado_otro_archivo_no_es_cerca() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path();
        escribir_defectos(casos, &[("src/a.rs", 100)]);
        let v = validar_con(
            casos,
            &falso(vec![clave("r", con_descartado("src/b.rs", 100))]),
        )
        .expect("mide");
        assert!(!v.defectos[0].descartado_cerca, "otro archivo no cuenta");
    }

    #[test]
    fn validez_sin_descartados_da_descartado_cerca_false() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path();
        escribir_defectos(casos, &[("src/a.rs", 100)]);
        let v = validar_con(casos, &falso(vec![clave("r", Veredicto::default())])).expect("mide");
        assert!(!v.defectos[0].descartado_cerca);
    }

    #[test]
    fn validez_viejo_defecto_sin_descartado_cerca_lee_false() {
        let dir = TempDir::new().expect("temporal");
        escribir_defectos(dir.path(), &[("src/a.rs", 20)]);
        let v =
            validar_con(dir.path(), &falso(vec![clave("r", Veredicto::default())])).expect("mide");
        let mut json = serde_json::to_value(&v.defectos[0]).expect("serializa");
        assert!(
            json.as_object_mut()
                .expect("objeto")
                .remove("descartado_cerca")
                .is_some(),
            "la clave descartado_cerca se serializa"
        );
        let viejo: DetalleDefecto =
            serde_json::from_value(json).expect("lee un defecto sin descartado_cerca");
        assert!(!viejo.descartado_cerca);
    }

    /// Escribe `defectos.toml` con un `[[defecto]]` por `(repo, archivo, linea, test)`. Va siempre el campo `test`.
    fn escribir_defectos_con_test(casos: &Path, defectos: &[(&str, &str, u32, &str)]) {
        let mut texto = String::new();
        for (repo, archivo, linea, test) in defectos {
            texto += &format!(
                "[[defecto]]\ntarea = \"memora\"\nrepo = \"{repo}\"\narchivo = \"{archivo}\"\nlinea = {linea}\n\
                 fuente = \"revisión fixture\"\ndetalle = \"defecto fixture\"\ntest = \"{test}\"\n\n"
            );
        }
        escribir(&casos.join("defectos.toml"), &texto);
    }

    #[test]
    fn validez_defecto_acepta_el_campo_test() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path();
        escribir_defectos_con_test(casos, &[("r", "src/a.rs", 20, "t::mi_test")]);
        let v = validar_con(casos, &falso(vec![clave("r", Veredicto::default())]))
            .expect("acepta el campo test");
        assert_eq!(v.defectos[0].test, Some("t::mi_test".to_string()));
    }

    #[test]
    fn validez_test_rojo_en_cada_lista() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path();
        escribir_defectos_con_test(
            casos,
            &[
                ("uno", "src/a.rs", 20, "t::oculto"),
                ("dos", "src/a.rs", 40, "t::inestable"),
                ("tres", "src/a.rs", 60, "t::inestable_contrato"),
                ("cuatro", "src/a.rs", 80, "t::contrato"),
                ("cinco", "src/a.rs", 100, "t::sano"),
            ],
        );
        let v_uno = Veredicto {
            ocultos_fallidos: vec!["t::oculto".into()],
            ..Veredicto::default()
        };
        let v_dos = Veredicto {
            inestables: vec!["t::inestable".into()],
            ..Veredicto::default()
        };
        let v_tres = Veredicto {
            inestables_contrato: vec!["t::inestable_contrato".into()],
            ..Veredicto::default()
        };
        let v_cuatro = Veredicto {
            contrato_fallidos: vec!["t::contrato".into()],
            ..Veredicto::default()
        };
        // Lista de fallos con otros ids: `t::sano` no está en ninguna.
        let v_cinco = Veredicto {
            ocultos_fallidos: vec!["t::otro".into()],
            inestables: vec!["t::otro_inestable".into()],
            inestables_contrato: vec!["t::otro_contrato".into()],
            contrato_fallidos: vec!["t::otro_fallido".into()],
            ..Veredicto::default()
        };
        let mapa = vec![
            clave("uno", v_uno),
            clave("dos", v_dos),
            clave("tres", v_tres),
            clave("cuatro", v_cuatro),
            clave("cinco", v_cinco),
        ];
        let v = validar_con(casos, &falso(mapa)).expect("mide");
        // Los dos inestables (`dos`, `tres`) no son rojos: van en `test_inestable`.
        for (i, detalle) in v.defectos[..4].iter().enumerate() {
            let inestable = i == 1 || i == 2;
            assert_eq!(
                detalle.test_rojo,
                Some(!inestable),
                "test {:?}",
                detalle.test
            );
            assert_eq!(detalle.test_inestable, inestable, "test {:?}", detalle.test);
        }
        assert_eq!(v.defectos[4].test_rojo, Some(false));
        assert!(!v.defectos[4].test_inestable);
    }

    #[test]
    fn validez_test_rojo_sin_hallazgo_cuenta_en_recall() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path();
        escribir_defectos_con_test(casos, &[("r", "src/a.rs", 20, "t::rojo")]);
        // Test rojo y sin hallazgos en el archivo.
        let v_root = Veredicto {
            ocultos_fallidos: vec!["t::rojo".into()],
            ..Veredicto::default()
        };
        let v = validar_con(casos, &falso(vec![clave("r", v_root)])).expect("mide");
        let detalle = &v.defectos[0];
        assert!(
            !detalle.encontrado,
            "test rojo no cuenta como encontrado por línea"
        );
        assert_eq!(detalle.test_rojo, Some(true));
        assert_eq!(v.recall.aciertos, Some(1), "test rojo cuenta en recall");
    }

    #[test]
    fn validez_recall_test_rojo_sin_hallazgo_cuenta_en_recall_test() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path();
        escribir_defectos_con_test(casos, &[("r", "src/a.rs", 20, "t::rojo")]);
        let v_root = Veredicto {
            ocultos_fallidos: vec!["t::rojo".into()],
            ..Veredicto::default()
        };
        let v = validar_con(casos, &falso(vec![clave("r", v_root)])).expect("mide");
        assert_eq!(v.recall_test.aciertos, Some(1));
        assert_eq!(v.recall_test.casos, 1);
        assert_eq!(v.recall_test.umbral, None, "recall_test no tiene umbral");
        assert_eq!(v.recall_test.cumple, None);
        assert_eq!(
            v.recall_linea.aciertos,
            Some(0),
            "sin hallazgo no cuenta por línea"
        );
        assert_eq!(v.recall_linea.casos, 1);
    }

    #[test]
    fn validez_recall_sin_test_ni_hallazgo_no_cuenta() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path();
        escribir_defectos(casos, &[("src/a.rs", 20)]);
        let v = validar_con(casos, &falso(vec![clave("r", Veredicto::default())])).expect("mide");
        assert_eq!(v.recall.aciertos, Some(0));
        assert_eq!(v.recall_linea.aciertos, Some(0));
        assert_eq!(v.recall_test.aciertos, Some(0));
        assert_eq!(
            v.recall_test.casos, 1,
            "el defecto entra al total aunque no cuente"
        );
    }

    #[test]
    fn validez_recall_hallazgo_y_test_rojo_cuenta_una_vez() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path();
        escribir_defectos_con_test(casos, &[("r", "src/a.rs", 20, "t::rojo")]);
        // Hallazgo a 2 líneas y test rojo: el defecto cuenta una sola vez en recall.
        let v_root = Veredicto {
            hallazgos: vec![hallazgo("src/a.rs", 22)],
            ocultos_fallidos: vec!["t::rojo".into()],
            ..Veredicto::default()
        };
        let v = validar_con(casos, &falso(vec![clave("r", v_root)])).expect("mide");
        assert_eq!(v.recall.aciertos, Some(1), "un solo defecto, una sola vez");
        assert_eq!(v.recall.casos, 1);
        assert_eq!(v.recall_linea.aciertos, Some(1));
        assert_eq!(v.recall_test.aciertos, Some(1));
    }

    #[test]
    fn validez_recall_viejo_sin_metricas_lee_sin_medir() {
        let dir = TempDir::new().expect("temporal");
        let v = validar_con(dir.path(), &falso(Vec::new())).expect("mide");
        let mut json = serde_json::to_value(&v).expect("serializa");
        let mapa = json.as_object_mut().expect("objeto");
        assert!(
            mapa.remove("recall_linea").is_some(),
            "la clave recall_linea se serializa"
        );
        assert!(
            mapa.remove("recall_test").is_some(),
            "la clave recall_test se serializa"
        );
        let viejo: Validez =
            serde_json::from_value(json).expect("lee un validez sin recall_linea ni recall_test");
        assert_eq!(viejo.recall_linea.valor, None);
        assert!(viejo.recall_linea.motivo.is_some());
        assert_eq!(viejo.recall_test.valor, None);
        assert!(viejo.recall_test.motivo.is_some());
    }

    #[test]
    fn validez_defecto_sin_test_da_test_rojo_none() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path();
        escribir_defectos(casos, &[("src/a.rs", 20)]);
        let v = validar_con(casos, &falso(vec![clave("r", Veredicto::default())])).expect("mide");
        assert_eq!(v.defectos[0].test, None);
        assert_eq!(v.defectos[0].test_rojo, None);
    }

    #[test]
    fn validez_test_inestable_no_es_rojo_ni_suma_recall() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path();
        escribir_defectos_con_test(
            casos,
            &[
                ("uno", "src/a.rs", 20, "t::flaky"),
                ("dos", "src/a.rs", 40, "t::flaky_contrato"),
            ],
        );
        let v_uno = Veredicto {
            inestables: vec!["t::flaky".into()],
            estado: Estado::Verde,
            ..Veredicto::default()
        };
        let v_dos = Veredicto {
            inestables_contrato: vec!["t::flaky_contrato".into()],
            estado: Estado::Verde,
            ..Veredicto::default()
        };
        let mapa = vec![clave("uno", v_uno), clave("dos", v_dos)];
        let v = validar_con(casos, &falso(mapa)).expect("mide");
        for detalle in &v.defectos {
            assert_eq!(detalle.test_rojo, Some(false), "test {:?}", detalle.test);
            assert!(detalle.test_inestable, "test {:?}", detalle.test);
        }
        assert_eq!(v.recall.aciertos, Some(0), "inestable no cuenta en recall");
        assert_eq!(v.recall.casos, 2);
        assert_eq!(
            v.recall_test.aciertos,
            Some(0),
            "inestable no cuenta en recall_test"
        );
    }

    #[test]
    fn validez_sonda_sin_veredicto_marca_y_sigue_en_denominador() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path();
        escribir_defectos_con_test(
            casos,
            &[
                ("uno", "src/a.rs", 20, "t::a"),
                ("dos", "src/a.rs", 60, "t::b"),
            ],
        );
        // `uno` sin veredicto de sonda; `dos` con veredicto y sin hallazgos.
        let v_uno = Veredicto {
            estado: Estado::SinVeredicto,
            ..Veredicto::default()
        };
        let v_dos = Veredicto {
            estado: Estado::Verde,
            ..Veredicto::default()
        };
        let mapa = vec![clave("uno", v_uno), clave("dos", v_dos)];
        let v = validar_con(casos, &falso(mapa)).expect("mide");
        assert!(v.defectos[0].sonda_sin_veredicto, "sin veredicto se marca");
        assert!(
            !v.defectos[1].sonda_sin_veredicto,
            "con veredicto no se marca"
        );
        assert_eq!(
            v.recall.casos, 2,
            "el defecto sin veredicto sigue en el denominador"
        );
        assert_eq!(v.recall.aciertos, Some(0));
    }

    #[test]
    fn validez_guardas_rojo_ocultos_y_contrato_no_marcan_inestable() {
        let dir = TempDir::new().expect("temporal");
        let casos = dir.path();
        escribir_defectos_con_test(
            casos,
            &[
                ("uno", "src/a.rs", 20, "t::oculto"),
                ("dos", "src/a.rs", 40, "t::contrato"),
            ],
        );
        let v_uno = Veredicto {
            ocultos_fallidos: vec!["t::oculto".into()],
            estado: Estado::Rojo,
            ..Veredicto::default()
        };
        let v_dos = Veredicto {
            contrato_fallidos: vec!["t::contrato".into()],
            estado: Estado::Rojo,
            ..Veredicto::default()
        };
        let mapa = vec![clave("uno", v_uno), clave("dos", v_dos)];
        let v = validar_con(casos, &falso(mapa)).expect("mide");
        for detalle in &v.defectos {
            assert_eq!(detalle.test_rojo, Some(true), "test {:?}", detalle.test);
            assert!(!detalle.test_inestable, "test {:?}", detalle.test);
            assert!(!detalle.sonda_sin_veredicto, "test {:?}", detalle.test);
        }
        assert_eq!(v.recall.aciertos, Some(2));
    }

    #[test]
    fn validez_viejo_detalle_sin_marcas_nuevas_lee_false() {
        let dir = TempDir::new().expect("temporal");
        escribir_defectos_con_test(dir.path(), &[("r", "src/a.rs", 20, "t::x")]);
        let v =
            validar_con(dir.path(), &falso(vec![clave("r", Veredicto::default())])).expect("mide");
        let mut json = serde_json::to_value(&v.defectos[0]).expect("serializa");
        let mapa = json.as_object_mut().expect("objeto");
        assert!(mapa.remove("test_inestable").is_some(), "se serializa");
        assert!(mapa.remove("sonda_sin_veredicto").is_some(), "se serializa");
        let viejo: DetalleDefecto =
            serde_json::from_value(json).expect("lee un detalle sin marcas nuevas");
        assert!(!viejo.test_inestable);
        assert!(!viejo.sonda_sin_veredicto);
    }

    #[test]
    fn validez_viejo_detalle_sin_test_lee_none() {
        let dir = TempDir::new().expect("temporal");
        escribir_defectos(dir.path(), &[("src/a.rs", 20)]);
        let v =
            validar_con(dir.path(), &falso(vec![clave("r", Veredicto::default())])).expect("mide");
        let mut json = serde_json::to_value(&v.defectos[0]).expect("serializa");
        let mapa = json.as_object_mut().expect("objeto");
        assert!(mapa.remove("test").is_some(), "la clave test se serializa");
        assert!(
            mapa.remove("test_rojo").is_some(),
            "la clave test_rojo se serializa"
        );
        let viejo: DetalleDefecto =
            serde_json::from_value(json).expect("lee un detalle sin test ni test_rojo");
        assert_eq!(viejo.test, None);
        assert_eq!(viejo.test_rojo, None);
    }
}
