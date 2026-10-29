//! Validez del juicio de Root (Tarea 12b): ¿Root juzga como un PM competente? `validar` corre Root sobre
//! casos cuyo resultado conoce una fuente externa a Root: el orden conocido, los defectos reales y la
//! calificación a ciegas del usuario. Sin fuente, la métrica queda sin medir: nunca 0 ni 1.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::rubrica::{self, Rubrica};
use super::{ErrorOraculo, evaluar_por_nombre};
use crate::tipos::{Decision, Dimension, TODAS_LAS_DIMENSIONES, Veredicto};

/// Versión del formato de `validez.json`.
pub const VERSION: u32 = 1;
/// Archivo que escribe `validar` dentro de la carpeta de casos.
pub const ARCHIVO: &str = "validez.json";
/// Recall desde el que los defectos reales cumplen (decisión 12).
pub const UMBRAL_RECALL: f32 = 0.8;
/// Kappa ponderado desde el que una dimensión cumple (decisión 13).
pub const UMBRAL_KAPPA: f32 = 0.6;
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
    /// Fracción de pares de `orden.toml` que Root ordena como el resultado medido. Sin umbral.
    pub exactitud_orden: Metrica,
    /// Fracción de defectos reales que Root encuentra con `archivo` y `linea` a ±3. Umbral 0,8.
    pub recall: Metrica,
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

/// Resultado de `preparar_ciego`: los ids de los paquetes armados. El origen queda en `clave.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preparado {
    pub paquetes: Vec<String>,
    /// `true` si no hay rúbricas embebidas: el formulario lleva solo la decisión y el orden.
    pub sin_rubricas: bool,
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
    };
    let mut avisos = Vec::new();
    let pares = leer_orden(casos)?;
    let defectos = leer_defectos(casos)?;
    let exactitud_orden = medir_orden(casos, pares.as_deref(), &mut ev)?;
    let recall = medir_recall(casos, defectos.as_deref(), &mut ev)?;
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
        recall,
        acuerdo_decision,
        kappa_dimension,
        kappa_item,
        spearman,
        avisos,
    })
}

/// Arma los paquetes a ciegas de `fuentes` en `casos/ciego/`, con la semilla `semilla`.
pub fn preparar_ciego(
    casos: &Path,
    fuentes: &Path,
    semilla: u64,
) -> Result<Preparado, ErrorOraculo> {
    let rubricas = rubrica::cargar_embebidas()
        .map_err(|e| ErrorOraculo::Infra(format!("rúbricas embebidas: {e}")))?;
    preparar_ciego_con(casos, fuentes, semilla, &rubricas)
}

/// Como `preparar_ciego`, con las rúbricas que recibe. Los tests pasan rúbricas de fixture.
pub fn preparar_ciego_con(
    casos: &Path,
    fuentes: &Path,
    semilla: u64,
    rubricas: &[Rubrica],
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
    let mut clave = ArchivoClave::default();
    let texto_rubricas = texto_formulario_items(rubricas);
    for (id, (fuente, repo)) in ids.iter().zip(&resueltas) {
        let paquete = ciego.join(id);
        fs::create_dir_all(&paquete).map_err(|e| io_infra("crear el paquete", &e))?;
        copiar_paquete(repo, &paquete.join(CARPETA_REPO))
            .map_err(|e| io_infra("copiar el repo al paquete", &e))?;
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

/// `true` si Root pone `mejor` por encima de `peor`. Primero la decisión; a igual decisión, el global si
/// los dos lo tienen, y si no, la completitud. Sin decisión, o en empate, no ordena: es un fallo.
fn ordena_antes(mejor: &Veredicto, peor: &Veredicto) -> bool {
    let (Some(dm), Some(dp)) = (decision_de(mejor), decision_de(peor)) else {
        return false;
    };
    if rango_decision(dm) != rango_decision(dp) {
        return rango_decision(dm) > rango_decision(dp);
    }
    match (mejor.global, peor.global) {
        (Some(gm), Some(gp)) => gm > gp,
        _ => match (completitud_de(mejor), completitud_de(peor)) {
            (Some(cm), Some(cp)) => cm > cp,
            _ => false,
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

fn normaliza(ruta: &str) -> &str {
    ruta.trim_start_matches("./")
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
struct Evaluaciones<'a> {
    evaluador: &'a dyn Fn(&Path, &str) -> Result<Veredicto, ErrorOraculo>,
    cache: BTreeMap<(String, PathBuf), Veredicto>,
}

impl Evaluaciones<'_> {
    fn veredicto(&mut self, tarea: &str, ruta: &Path) -> Result<Veredicto, ErrorOraculo> {
        let clave = (tarea.to_string(), ruta.to_path_buf());
        if let Some(v) = self.cache.get(&clave) {
            return Ok(v.clone());
        }
        let v = (self.evaluador)(ruta, tarea)?;
        self.cache.insert(clave, v.clone());
        Ok(v)
    }
}

fn medir_orden(
    casos: &Path,
    pares: Option<&[ParOrden]>,
    ev: &mut Evaluaciones,
) -> Result<Metrica, ErrorOraculo> {
    let Some(pares) = pares else {
        return Ok(Metrica::sin_medir(
            "falta orden.toml: sin pares de orden conocido",
        ));
    };
    if pares.is_empty() {
        return Ok(Metrica::sin_medir("orden.toml no tiene pares"));
    }
    let mut aciertos = 0;
    for par in pares {
        let mejor = ev.veredicto(&par.tarea, &casos.join(&par.mejor))?;
        let peor = ev.veredicto(&par.tarea, &casos.join(&par.peor))?;
        if ordena_antes(&mejor, &peor) {
            aciertos += 1;
        }
    }
    Ok(Metrica::fraccion(aciertos, pares.len() as u32, None))
}

fn medir_recall(
    casos: &Path,
    defectos: Option<&[DefectoReal]>,
    ev: &mut Evaluaciones,
) -> Result<Metrica, ErrorOraculo> {
    let umbral = Some(UMBRAL_RECALL);
    let Some(defectos) = defectos else {
        return Ok(Metrica {
            umbral,
            ..Metrica::sin_medir("falta defectos.toml: sin defectos reales")
        });
    };
    if defectos.is_empty() {
        return Ok(Metrica {
            umbral,
            ..Metrica::sin_medir("defectos.toml no tiene defectos")
        });
    }
    let mut encontrados = 0;
    for defecto in defectos {
        let v = ev.veredicto(&defecto.tarea, &casos.join(&defecto.repo))?;
        if encuentra(&v, defecto) {
            encontrados += 1;
        }
    }
    Ok(Metrica::fraccion(
        encontrados,
        defectos.len() as u32,
        umbral,
    ))
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
            dimension,
            valor,
            calibrada: true,
            exactitud_balanceada: None,
            items,
            sin_veredicto: None,
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
        let p = preparar_ciego_con(&casos, &ruta, 7, &rubricas()).expect("prepara seis paquetes");
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
        assert_eq!(v.exactitud_orden.cumple, None, "el orden no tiene umbral");
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
            &v.acuerdo_decision,
            &v.spearman,
        ] {
            assert_eq!(m.valor, None, "sin fuente no hay valor");
            assert!(m.motivo.is_some(), "sin fuente hay motivo");
            assert_eq!(m.cumple, None);
        }
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

        let p = preparar_ciego_con(&casos, &manifiesto, 42, &rubricas()).expect("prepara");
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
        assert!(preparar_ciego_con(&casos, &manifiesto, 1, &rubricas()).is_err());
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
}
