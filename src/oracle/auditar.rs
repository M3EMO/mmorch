//! Pasada de búsqueda de defectos (Tarea 74). Un modelo propone candidatos; el gate de cita y la medición
//! determinista los cuentan. Un LLM nunca decide si un defecto se encontró. No puntúa y no toca al juez.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::ErrorOraculo;
use super::evidencia::cita_en_archivo;
use super::juez::{LimitesPrompt, MAX_ENTRADA_JUEZ_BYTES, ModeloJuez, bytes_de_prompt};
use super::perfiles::herramientas::parsear_gitleaks;
use crate::config::Config;
use crate::observe;
use crate::proveedores::{INTENTOS_MAX, Prompt, costo_peor_caso};
use crate::tipos::Hallazgo;

/// Prompt de sistema de la pasada. Cambiar un byte cambia `hash_prompt`.
pub const PROMPT_AUDITORIA: &str = r#"Sos un revisor senior. Leé este código y reportá defectos reales: conducta incorrecta, seguridad, fiabilidad o uso de recursos. Cada defecto trae archivo, línea, la cita textual de esa línea y una frase que dice qué falla. No reportes estilo. Si no hallás defectos, respondé una lista vacía. Respondé SOLO un objeto JSON: {"defectos":[{"archivo":"...","linea":N,"cita":"...","detalle":"..."}]}"#;

/// Texto que reemplaza la línea de un hallazgo de `gitleaks` en el prompt.
const LINEA_OMITIDA: &str = "[línea con un secreto omitida]";
/// Desde esta línea, el código es de tests y no entra al prompt.
const MARCA_TEST: &str = "#[cfg(test)]";
/// Marca que se agrega al final de una línea recortada.
const MARCA_RECORTE: &str = "…[recortado]";
/// Distancia máxima en líneas entre un hallazgo y un defecto para contarlo como encontrado.
const VENTANA_MEDICION: u32 = 3;
/// Fase del flujo que va al `Evento` de cada llamada.
const FASE: &str = "auditar";

/// Opciones de `oraculo auditar`.
#[derive(Debug, Clone)]
pub struct OpcionesAuditar {
    pub repo: PathBuf,
    pub salida: PathBuf,
    pub tope_usd: Option<f64>,
    pub gitleaks: Option<PathBuf>,
    pub incluir: Vec<PathBuf>,
    pub defectos: Option<PathBuf>,
    pub por_archivo: bool,
    pub solo_medir: Option<PathBuf>,
}

/// Un defecto que el modelo propone y que el gate de cita confirma.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Candidato {
    pub archivo: String,
    pub linea: u32,
    pub cita: String,
    pub detalle: String,
}

/// Un candidato que no entra a `hallazgos`, con el motivo.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Descartado {
    pub archivo: String,
    pub linea: u32,
    pub cita: String,
    pub detalle: String,
    pub motivo: String,
}

/// Origen de un defecto: lo encontró un revisor real o se sembró a propósito.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Origen {
    Real,
    Sembrado,
}

impl Origen {
    /// Clave del conteo por origen. Coincide con el texto de `defectos.toml`.
    fn etiqueta(self) -> &'static str {
        match self {
            Origen::Real => "real",
            Origen::Sembrado => "sembrado",
        }
    }
}

/// Suma un defecto medido al conteo de su grupo.
fn sumar(mapa: &mut BTreeMap<String, Conteo>, clave: &str, encontrado: bool) {
    let c = mapa.entry(clave.to_string()).or_default();
    c.casos_leidos += 1;
    if encontrado {
        c.encontrados += 1;
    }
}

/// Conteo de un grupo de defectos medidos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
pub struct Conteo {
    pub casos_leidos: u32,
    pub encontrados: u32,
}

/// Un defecto de `defectos.toml` y si algún hallazgo lo cubre.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DetalleDefecto {
    pub archivo: String,
    pub linea: u32,
    /// Falso si el archivo no se leyó. Un defecto sin medir no cuenta.
    pub medido: bool,
    pub encontrado: bool,
    /// Solo con `encontrado`: el hallazgo más cercano dentro de la ventana.
    pub hallazgo_cercano: Option<Candidato>,
    pub clase: Option<String>,
    pub origen: Option<Origen>,
}

/// Cuántos defectos del repo encontró la pasada.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Medicion {
    pub encontrados: u32,
    pub casos: u32,
    /// Defectos cuyo archivo se leyó. Solo estos entran en `encontrados`.
    pub casos_leidos: u32,
    pub detalle: Vec<DetalleDefecto>,
    /// Solo defectos medidos. Suma de `casos_leidos` y `encontrados` igual a los totales.
    pub por_clase: BTreeMap<String, Conteo>,
    /// Igual que `por_clase`, agrupado por origen.
    pub por_origen: BTreeMap<String, Conteo>,
}

/// Contenido de `--salida`.
#[derive(Debug, Clone, Serialize)]
pub struct Auditoria {
    pub hash_prompt: String,
    pub modelo: String,
    /// Archivos que salieron en alguna llamada.
    pub archivos: Vec<String>,
    /// Llamadas enviadas al modelo, con o sin respuesta usable.
    pub llamadas: u32,
    pub costo_usd: f64,
    pub hallazgos: Vec<Candidato>,
    pub descartados: Vec<Descartado>,
    pub incompleta: bool,
    /// Archivos que no se leyeron con éxito.
    pub sin_leer: Vec<String>,
    pub avisos: Vec<String>,
    pub medicion: Option<Medicion>,
    /// Líneas del texto que vio el modelo, de los archivos leídos. Sin código de tests.
    pub lineas_leidas: u32,
    /// Hallazgos por cada mil líneas leídas. `None` si no se leyó ninguna línea.
    pub candidatos_por_1000_lineas: Option<f64>,
}

/// Huella del prompt de sistema. Cambia si cambia `PROMPT_AUDITORIA`.
pub fn hash_prompt() -> String {
    hash_de(PROMPT_AUDITORIA)
}

fn hash_de(texto: &str) -> String {
    crate::goal::calcular_hash(texto.as_bytes())
}

/// Mide los defectos de `texto_defectos` que son de `repo` contra los `hallazgos`.
pub fn medir(
    texto_defectos: &str,
    repo: &Path,
    hallazgos: &[Candidato],
    leidos: &BTreeSet<String>,
) -> Result<Medicion, ErrorOraculo> {
    let defectos = defectos_del_repo(texto_defectos, repo)?;
    let mut encontrados = 0;
    let mut casos_leidos = 0;
    let mut detalle = Vec::with_capacity(defectos.len());
    let mut por_clase = BTreeMap::new();
    let mut por_origen = BTreeMap::new();
    for d in &defectos {
        // Sin lectura no hay medición: el defecto no cuenta como encontrado ni como caso medido.
        let medido = leidos.contains(&d.archivo);
        let cercano = if medido {
            hallazgos
                .iter()
                .filter(|h| h.archivo == d.archivo && h.linea.abs_diff(d.linea) <= VENTANA_MEDICION)
                .min_by_key(|h| h.linea.abs_diff(d.linea))
                .cloned()
        } else {
            None
        };
        if medido {
            casos_leidos += 1;
            let encontrado = cercano.is_some();
            let clase = d.clase.as_deref().unwrap_or("sin_clase");
            let origen = d.origen.map_or("sin_origen", Origen::etiqueta);
            sumar(&mut por_clase, clase, encontrado);
            sumar(&mut por_origen, origen, encontrado);
        }
        if cercano.is_some() {
            encontrados += 1;
        }
        detalle.push(DetalleDefecto {
            archivo: d.archivo.clone(),
            linea: d.linea,
            medido,
            encontrado: cercano.is_some(),
            hallazgo_cercano: cercano,
            clase: d.clase.clone(),
            origen: d.origen,
        });
    }
    Ok(Medicion {
        encontrados,
        casos: u32::try_from(defectos.len()).unwrap_or(u32::MAX),
        casos_leidos,
        detalle,
        por_clase,
        por_origen,
    })
}

/// Medición de la pasada. Sin archivos leídos no hay medición: devuelve `None`.
fn medir_auditoria(
    texto: &str,
    repo: &Path,
    a: &Auditoria,
) -> Result<Option<Medicion>, ErrorOraculo> {
    let leidos: BTreeSet<String> = a
        .archivos
        .iter()
        .filter(|f| !a.sin_leer.contains(f))
        .cloned()
        .collect();
    if leidos.is_empty() {
        return Ok(None);
    }
    Ok(Some(medir(texto, repo, &a.hallazgos, &leidos)?))
}

/// Mide y asigna `a.medicion`. Sin archivos leídos no hay medición: deja el aviso que lo dice.
fn medir_con_aviso(texto: &str, repo: &Path, a: &mut Auditoria) -> Result<(), ErrorOraculo> {
    a.medicion = medir_auditoria(texto, repo, a)?;
    if a.medicion.is_none() {
        a.avisos
            .push("ningún archivo leído: no hay medición".into());
    }
    Ok(())
}

/// Defecto de `defectos.toml` en la forma que mide la pasada. Sin `deny_unknown_fields`: otros campos se ignoran.
#[derive(Deserialize)]
struct DefectoRepo {
    repo: String,
    archivo: String,
    linea: u32,
    /// Libre. Sin clase, el conteo cae en `sin_clase`.
    #[serde(default)]
    clase: Option<String>,
    /// Solo `real` o `sembrado`. Otro valor es error de uso.
    #[serde(default)]
    origen: Option<Origen>,
}

#[derive(Deserialize)]
struct ArchivoDefectos {
    #[serde(default)]
    defecto: Vec<DefectoRepo>,
}

/// Defectos de `texto` que son de `repo`. El repo se compara por su último componente, no por texto.
fn defectos_del_repo(texto: &str, repo: &Path) -> Result<Vec<DefectoRepo>, ErrorOraculo> {
    let archivo: ArchivoDefectos = toml::from_str(texto)
        .map_err(|e| ErrorOraculo::Uso(format!("defectos.toml inválido: {e}")))?;
    let canonico = repo
        .canonicalize()
        .map_err(|e| ErrorOraculo::Uso(format!("repo {}: {e}", repo.display())))?;
    let nombre = canonico
        .file_name()
        .ok_or_else(|| ErrorOraculo::Uso(format!("repo sin nombre: {}", repo.display())))?;
    Ok(archivo
        .defecto
        .into_iter()
        .filter(|d| Path::new(&d.repo).file_name() == Some(nombre))
        .collect())
}

/// Corre la pasada con un modelo cualquiera. Con `tope_usd`, corta antes de la llamada que lo pasaría.
#[allow(clippy::too_many_arguments)]
pub fn auditar_con(
    repo: &Path,
    incluir: &[PathBuf],
    gitleaks: &[Hallazgo],
    modelo: &dyn ModeloJuez,
    nombre_modelo: &str,
    tope_usd: f64,
    peor_caso: &dyn Fn(&Prompt) -> f64,
    gasto: &dyn Fn() -> Result<f64, ErrorOraculo>,
) -> Result<Auditoria, ErrorOraculo> {
    auditar_con_limite(
        repo,
        incluir,
        gitleaks,
        modelo,
        nombre_modelo,
        tope_usd,
        peor_caso,
        gasto,
        MAX_ENTRADA_JUEZ_BYTES,
        false,
    )
}

/// Como `auditar_con`, con el límite de entrada como parámetro. Los tests lo bajan para partir archivos chicos.
#[allow(clippy::too_many_arguments)]
fn auditar_con_limite(
    repo: &Path,
    incluir: &[PathBuf],
    gitleaks: &[Hallazgo],
    modelo: &dyn ModeloJuez,
    nombre_modelo: &str,
    tope_usd: f64,
    peor_caso: &dyn Fn(&Prompt) -> f64,
    gasto: &dyn Fn() -> Result<f64, ErrorOraculo>,
    limite: u64,
    por_archivo: bool,
) -> Result<Auditoria, ErrorOraculo> {
    let archivos = leer_archivos(repo, incluir)?;
    let presupuesto = (limite as usize).saturating_sub(PROMPT_AUDITORIA.len());
    let omitidas = lineas_omitidas(gitleaks);
    let bloques = bloques(&archivos, &omitidas, presupuesto);
    if let Some(grande) = bloques.iter().find(|b| b.texto.len() > presupuesto) {
        return Err(ErrorOraculo::Uso(format!(
            "la ruta {} no cabe en un prompt del juez",
            grande.archivo
        )));
    }
    let grupos = if por_archivo {
        // Cada archivo empaqueta solo consigo: sus partes pueden compartir llamada, otros archivos no.
        tramos_por_archivo(bloques)
            .into_iter()
            .flat_map(|tramo| agrupar(tramo, presupuesto))
            .collect()
    } else {
        agrupar(bloques, presupuesto)
    };
    // El gate lee lo que el modelo vio: sin código de tests y con las líneas de secretos omitidas.
    let contenidos = contenidos_visibles(&archivos, &omitidas);

    let mut auditoria = Auditoria {
        hash_prompt: hash_prompt(),
        modelo: nombre_modelo.into(),
        archivos: Vec::new(),
        llamadas: 0,
        costo_usd: 0.0,
        hallazgos: Vec::new(),
        descartados: Vec::new(),
        incompleta: false,
        sin_leer: Vec::new(),
        avisos: Vec::new(),
        medicion: None,
        lineas_leidas: 0,
        candidatos_por_1000_lineas: None,
    };
    let mut enviados = BTreeSet::new();
    let mut sin_leer = BTreeSet::new();

    for (i, grupo) in grupos.iter().enumerate() {
        let prompt = prompt_de(grupo);
        let necesario = gasto()? + peor_caso(&prompt);
        if necesario > tope_usd {
            // Sin la primera llamada no hay nada que leer: es un error de uso, no una medición parcial.
            if i == 0 {
                return Err(ErrorOraculo::Uso(format!(
                    "tope insuficiente: la primera llamada necesita {necesario:.4} USD en el peor caso y --tope-usd es {tope_usd:.4}"
                )));
            }
            auditoria.incompleta = true;
            sin_leer.extend(archivos_de(&grupos[i..]));
            break;
        }
        auditoria.llamadas += 1;
        let de_grupo = archivos_de(std::slice::from_ref(grupo));
        enviados.extend(de_grupo.iter().cloned());
        let n = auditoria.llamadas;
        let salida = match modelo.consultar(&prompt) {
            Ok(salida) => salida,
            Err(e) => {
                auditoria
                    .avisos
                    .push(format!("llamada {n}: el modelo falló: {e}"));
                auditoria.incompleta = true;
                sin_leer.extend(archivos_de(&grupos[i..]));
                break;
            }
        };
        auditoria.costo_usd += salida.costo_usd;
        if salida.truncada {
            auditoria.avisos.push(format!(
                "llamada {n}: respuesta truncada por el tope de salida"
            ));
            auditoria.incompleta = true;
            sin_leer.extend(de_grupo);
            continue;
        }
        match parsear_respuesta(&salida.texto) {
            Ok(propuestas) => {
                aplicar_gate(
                    propuestas,
                    &|archivo: &str| de_grupo.contains(archivo),
                    &contenidos,
                    &mut auditoria,
                );
            }
            Err(motivo) => {
                auditoria.avisos.push(format!("llamada {n}: {motivo}"));
                auditoria.incompleta = true;
                sin_leer.extend(de_grupo);
            }
        }
    }
    auditoria.archivos = enviados.into_iter().collect();
    auditoria.sin_leer = sin_leer.into_iter().collect();
    let (lineas, por_mil) = densidad(
        &contenidos,
        &auditoria.archivos,
        &auditoria.sin_leer,
        auditoria.hallazgos.len(),
    );
    auditoria.lineas_leidas = lineas;
    auditoria.candidatos_por_1000_lineas = por_mil;
    Ok(auditoria)
}

/// Decide por candidato: hallazgo si el archivo se envió y la cita está en el texto visible.
/// Lo que no pasa va a `descartados` con el motivo.
fn aplicar_gate(
    propuestas: Vec<Result<Candidato, Descartado>>,
    enviado: &dyn Fn(&str) -> bool,
    contenidos: &BTreeMap<&str, String>,
    a: &mut Auditoria,
) {
    for propuesta in propuestas {
        match propuesta {
            Ok(c) if !enviado(&c.archivo) => a
                .descartados
                .push(descartar(c, "el archivo no se envió en esta llamada")),
            Ok(c) => {
                let citada = contenidos
                    .get(c.archivo.as_str())
                    .is_some_and(|texto| cita_en_archivo(texto.as_str(), c.linea, &c.cita));
                if citada {
                    a.hallazgos.push(c);
                } else {
                    a.descartados
                        .push(descartar(c, "cita no confirmada en el archivo"));
                }
            }
            Err(d) => a.descartados.push(d),
        }
    }
}

/// Texto visible de cada archivo, con sus claves relativas. Sin código de tests y con secretos omitidos.
fn contenidos_visibles<'a>(
    archivos: &'a [Archivo],
    omitidas: &BTreeMap<&str, BTreeSet<u32>>,
) -> BTreeMap<&'a str, String> {
    let vacia = BTreeSet::new();
    archivos
        .iter()
        .map(|a| {
            let omitidas = omitidas.get(a.ruta.as_str()).unwrap_or(&vacia);
            (a.ruta.as_str(), texto_visible(a, omitidas))
        })
        .collect()
}

/// Líneas leídas y hallazgos por mil líneas. Sin líneas leídas, la densidad es `None`.
/// Cuenta el texto visible de los archivos leídos, no los archivos enviados.
fn densidad(
    contenidos: &BTreeMap<&str, String>,
    archivos: &[String],
    sin_leer: &[String],
    n_hallazgos: usize,
) -> (u32, Option<f64>) {
    let lineas: usize = archivos
        .iter()
        .filter(|f| !sin_leer.contains(f))
        .filter_map(|f| contenidos.get(f.as_str()))
        .map(|texto| texto.lines().count())
        .sum();
    let lineas = u32::try_from(lineas).unwrap_or(u32::MAX);
    if lineas == 0 {
        return (0, None);
    }
    let por_mil = n_hallazgos as f64 * 1000.0 / f64::from(lineas);
    (lineas, Some(por_mil))
}

/// Un archivo `.rs` del repo con su contenido completo.
struct Archivo {
    ruta: String,
    contenido: String,
}

/// Un trozo de archivo con su encabezado. Cabe en un prompt por sí solo.
struct Bloque {
    archivo: String,
    texto: String,
}

fn descartar(c: Candidato, motivo: &str) -> Descartado {
    Descartado {
        archivo: c.archivo,
        linea: c.linea,
        cita: c.cita,
        detalle: c.detalle,
        motivo: motivo.into(),
    }
}

/// Archivos `.rs` a auditar, ordenados por ruta relativa. Sin `incluir`, todo `src` del repo.
fn leer_archivos(repo: &Path, incluir: &[PathBuf]) -> Result<Vec<Archivo>, ErrorOraculo> {
    let raices: Vec<PathBuf> = if incluir.is_empty() {
        vec![repo.join("src")]
    } else {
        incluir
            .iter()
            .map(|e| relativa(e).map(|r| repo.join(r)))
            .collect::<Result<Vec<PathBuf>, ErrorOraculo>>()?
    };
    let mut rutas = BTreeMap::new();
    for raiz in &raices {
        recolectar(repo, raiz, &mut rutas)?;
    }
    if rutas.is_empty() {
        return Err(ErrorOraculo::Uso(format!(
            "no hay archivos .rs que auditar en {}",
            repo.display()
        )));
    }
    rutas
        .into_iter()
        .map(|(ruta, absoluta)| {
            let bytes = fs::read(&absoluta)
                .map_err(|e| ErrorOraculo::Infra(format!("leer {}: {e}", absoluta.display())))?;
            Ok(Archivo {
                ruta,
                contenido: String::from_utf8_lossy(&bytes).into_owned(),
            })
        })
        .collect()
}

/// Ruta de `entrada` sin raíz ni `..`. Una ruta que sale del repo es error de uso.
fn relativa(entrada: &Path) -> Result<PathBuf, ErrorOraculo> {
    let mut rel = PathBuf::new();
    for componente in entrada.components() {
        match componente {
            Component::Normal(p) => rel.push(p),
            Component::CurDir => {}
            _ => {
                return Err(ErrorOraculo::Uso(format!(
                    "{} sale del repo: usá una ruta relativa",
                    entrada.display()
                )));
            }
        }
    }
    Ok(rel)
}

/// Junta los `.rs` de `ruta` en `rutas`. No sigue symlinks: un symlink como raíz es error de uso.
fn recolectar(
    repo: &Path,
    ruta: &Path,
    rutas: &mut BTreeMap<String, PathBuf>,
) -> Result<(), ErrorOraculo> {
    let meta = fs::symlink_metadata(ruta)
        .map_err(|e| ErrorOraculo::Uso(format!("{}: {e}", ruta.display())))?;
    if meta.file_type().is_symlink() {
        return Err(ErrorOraculo::Uso(format!(
            "{} es un symlink: no se sigue",
            ruta.display()
        )));
    }
    if meta.is_dir() {
        let entradas = fs::read_dir(ruta)
            .map_err(|e| ErrorOraculo::Infra(format!("leer {}: {e}", ruta.display())))?;
        for entrada in entradas {
            let entrada = entrada
                .map_err(|e| ErrorOraculo::Infra(format!("leer {}: {e}", ruta.display())))?;
            let tipo = entrada
                .file_type()
                .map_err(|e| ErrorOraculo::Infra(format!("tipo de {}: {e}", ruta.display())))?;
            if tipo.is_symlink() {
                continue;
            }
            recolectar(repo, &entrada.path(), rutas)?;
        }
    } else if meta.is_file() && ruta.extension().is_some_and(|x| x == "rs") {
        let rel = ruta.strip_prefix(repo).unwrap_or(ruta);
        rutas.insert(clave(rel), ruta.to_path_buf());
    }
    Ok(())
}

/// Ruta relativa con `/` como separador. Es la clave de `Archivo` y de los candidatos.
fn clave(rel: &Path) -> String {
    rel.components()
        .filter_map(|c| match c {
            Component::Normal(p) => Some(p.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<String>>()
        .join("/")
}

/// Líneas de cada archivo que cita un hallazgo de `gitleaks`. Todas salen omitidas en el prompt.
fn lineas_omitidas(gitleaks: &[Hallazgo]) -> BTreeMap<&str, BTreeSet<u32>> {
    let mut mapa: BTreeMap<&str, BTreeSet<u32>> = BTreeMap::new();
    for h in gitleaks {
        mapa.entry(h.archivo.as_str()).or_default().insert(h.linea);
    }
    mapa
}

/// Texto de `a` que ve el modelo, con los números de línea reales: sin tests y sin líneas de secretos.
fn texto_visible(a: &Archivo, omitidas: &BTreeSet<u32>) -> String {
    let mut salida = String::new();
    for (i, linea) in a.contenido.lines().enumerate() {
        if linea.trim_start().starts_with(MARCA_TEST) {
            break;
        }
        let omitida = omitidas.contains(&((i + 1) as u32));
        salida.push_str(if omitida { LINEA_OMITIDA } else { linea });
        salida.push('\n');
    }
    salida
}

/// Texto de `a` solo con sus líneas de secretos. Las demás quedan vacías, con los mismos números.
fn texto_solo_secretos(a: &Archivo, omitidas: &BTreeSet<u32>) -> String {
    let mut salida = String::new();
    for (i, linea) in a.contenido.lines().enumerate() {
        if linea.trim_start().starts_with(MARCA_TEST) {
            break;
        }
        if omitidas.contains(&((i + 1) as u32)) {
            salida.push_str(linea);
        }
        salida.push('\n');
    }
    salida
}

/// Cierto si la cita de `c` cae en una línea de secreto: la línea misma o una vecina que la cita.
fn cae_en_secreto(
    c: &Candidato,
    omitidas: &BTreeMap<&str, BTreeSet<u32>>,
    solo_secretos: &BTreeMap<&str, String>,
) -> bool {
    let en_omitida = omitidas
        .get(c.archivo.as_str())
        .is_some_and(|lineas| lineas.contains(&c.linea));
    en_omitida
        || solo_secretos
            .get(c.archivo.as_str())
            .is_some_and(|texto| cita_en_archivo(texto, c.linea, &c.cita))
}

/// Pasa a `descartados` sin texto los candidatos que caen en una línea de secreto.
/// Solo mira candidatos válidos. Los demás pasan tal cual.
fn ocultar_secretos(
    propuestas: Vec<Result<Candidato, Descartado>>,
    archivos: &[Archivo],
    omitidas: &BTreeMap<&str, BTreeSet<u32>>,
) -> Vec<Result<Candidato, Descartado>> {
    let vacia = BTreeSet::new();
    let solo_secretos: BTreeMap<&str, String> = archivos
        .iter()
        .map(|a| {
            let omitidas = omitidas.get(a.ruta.as_str()).unwrap_or(&vacia);
            (a.ruta.as_str(), texto_solo_secretos(a, omitidas))
        })
        .collect();
    propuestas
        .into_iter()
        .map(|propuesta| match propuesta {
            Ok(c) if cae_en_secreto(&c, omitidas, &solo_secretos) => Err(Descartado {
                archivo: c.archivo,
                linea: c.linea,
                cita: String::new(),
                detalle: String::new(),
                motivo: "la cita cae en una línea de secreto".into(),
            }),
            otra => otra,
        })
        .collect()
}

/// Líneas de producción de `a` como `NN: texto`. Corta en la primera marca de tests.
fn lineas_numeradas(a: &Archivo, omitidas: &BTreeSet<u32>) -> Vec<String> {
    let mut salida = Vec::new();
    for (i, linea) in a.contenido.lines().enumerate() {
        if linea.trim_start().starts_with(MARCA_TEST) {
            break;
        }
        let numero = (i + 1) as u32;
        let texto = if omitidas.contains(&numero) {
            LINEA_OMITIDA
        } else {
            linea
        };
        salida.push(format!("{numero}: {texto}\n"));
    }
    salida
}

fn encabezado(ruta: &str, parte: Option<usize>) -> String {
    match parte {
        None => format!("=== ARCHIVO {ruta} ===\n"),
        Some(n) => format!("=== ARCHIVO {ruta} (parte {n}) ===\n"),
    }
}

/// Recorta `fila` a `max` bytes, con la marca de recorte al final. Vacío si no cabe ni la marca.
fn recortar(fila: &str, max: usize) -> String {
    if fila.len() <= max {
        return fila.to_string();
    }
    let marca = format!("{MARCA_RECORTE}\n");
    if max < marca.len() {
        return String::new();
    }
    let mut corte = (max - marca.len()).min(fila.len());
    while !fila.is_char_boundary(corte) {
        corte -= 1;
    }
    format!("{}{marca}", &fila[..corte])
}

/// Bloques de todos los archivos. Un archivo que no cabe entero se parte por líneas.
fn bloques(
    archivos: &[Archivo],
    omitidas: &BTreeMap<&str, BTreeSet<u32>>,
    presupuesto: usize,
) -> Vec<Bloque> {
    let vacio = BTreeSet::new();
    let mut salida = Vec::new();
    for a in archivos {
        let filas = lineas_numeradas(a, omitidas.get(a.ruta.as_str()).unwrap_or(&vacio));
        if filas.is_empty() {
            continue;
        }
        let entero = format!("{}{}", encabezado(&a.ruta, None), filas.concat());
        if entero.len() <= presupuesto {
            salida.push(Bloque {
                archivo: a.ruta.clone(),
                texto: entero,
            });
            continue;
        }
        let mut parte = 1;
        let mut actual = encabezado(&a.ruta, Some(parte));
        let mut con_filas = false;
        for fila in filas {
            if con_filas && actual.len() + fila.len() > presupuesto {
                salida.push(Bloque {
                    archivo: a.ruta.clone(),
                    texto: std::mem::take(&mut actual),
                });
                parte += 1;
                actual = encabezado(&a.ruta, Some(parte));
            }
            let fila = if actual.len() + fila.len() > presupuesto {
                recortar(&fila, presupuesto.saturating_sub(actual.len()))
            } else {
                fila
            };
            actual.push_str(&fila);
            con_filas = true;
        }
        salida.push(Bloque {
            archivo: a.ruta.clone(),
            texto: actual,
        });
    }
    salida
}

/// Parte los bloques en tramos de bloques consecutivos del mismo archivo, en orden.
fn tramos_por_archivo(bloques: Vec<Bloque>) -> Vec<Vec<Bloque>> {
    let mut tramos: Vec<Vec<Bloque>> = Vec::new();
    for b in bloques {
        let mismo = tramos
            .last()
            .and_then(|t| t.last())
            .is_some_and(|ult| ult.archivo == b.archivo);
        if !mismo {
            tramos.push(Vec::new());
        }
        if let Some(tramo) = tramos.last_mut() {
            tramo.push(b);
        }
    }
    tramos
}

/// Empaqueta los bloques en orden, sin pasar `presupuesto` bytes por grupo.
fn agrupar(bloques: Vec<Bloque>, presupuesto: usize) -> Vec<Vec<Bloque>> {
    let mut grupos = Vec::new();
    let mut actual: Vec<Bloque> = Vec::new();
    let mut bytes = 0;
    for b in bloques {
        if !actual.is_empty() && bytes + b.texto.len() > presupuesto {
            grupos.push(std::mem::take(&mut actual));
            bytes = 0;
        }
        bytes += b.texto.len();
        actual.push(b);
    }
    if !actual.is_empty() {
        grupos.push(actual);
    }
    grupos
}

fn archivos_de(grupos: &[Vec<Bloque>]) -> BTreeSet<String> {
    grupos.iter().flatten().map(|b| b.archivo.clone()).collect()
}

fn prompt_de(grupo: &[Bloque]) -> Prompt {
    Prompt {
        sistema: Some(PROMPT_AUDITORIA.into()),
        usuario: grupo.iter().map(|b| b.texto.as_str()).collect(),
        max_tokens: LimitesPrompt::default().max_tokens,
        fase: FASE.into(),
        reintento_juez: false,
    }
}

#[derive(Deserialize)]
struct RespuestaAuditor {
    defectos: Vec<Value>,
}

/// Candidato tal como lo manda el modelo. Cada campo puede faltar.
#[derive(Deserialize)]
struct CandidatoCrudo {
    archivo: Option<String>,
    linea: Option<u32>,
    cita: Option<String>,
    detalle: Option<String>,
}

/// Parsea la respuesta. Un candidato incompleto o mal formado va a descartados, no rompe la llamada.
fn parsear_respuesta(texto: &str) -> Result<Vec<Result<Candidato, Descartado>>, String> {
    let respuesta: RespuestaAuditor =
        serde_json::from_str(quitar_cerca(texto)).map_err(|e| format!("JSON inválido: {e}"))?;
    Ok(respuesta
        .defectos
        .into_iter()
        .map(|valor| {
            let Ok(crudo) = serde_json::from_value::<CandidatoCrudo>(valor) else {
                return Err(Descartado {
                    archivo: String::new(),
                    linea: 0,
                    cita: String::new(),
                    detalle: String::new(),
                    motivo: "candidato mal formado".into(),
                });
            };
            match (crudo.archivo, crudo.linea, crudo.cita, crudo.detalle) {
                (Some(archivo), Some(linea), Some(cita), Some(detalle)) => Ok(Candidato {
                    archivo,
                    linea,
                    cita,
                    detalle,
                }),
                (archivo, linea, cita, detalle) => Err(Descartado {
                    archivo: archivo.unwrap_or_default(),
                    linea: linea.unwrap_or(0),
                    cita: cita.unwrap_or_default(),
                    detalle: detalle.unwrap_or_default(),
                    motivo: "campos incompletos".into(),
                }),
            }
        })
        .collect())
}

/// Quita una cerca de Markdown alrededor del JSON, con o sin etiqueta de lenguaje.
fn quitar_cerca(texto: &str) -> &str {
    let limpio = texto.trim();
    let Some(resto) = limpio.strip_prefix("```") else {
        return limpio;
    };
    let resto = match resto.split_once('\n') {
        Some((etiqueta, cuerpo)) if etiqueta.trim().chars().all(char::is_alphanumeric) => cuerpo,
        _ => resto,
    };
    resto
        .trim()
        .strip_suffix("```")
        .unwrap_or(resto.trim())
        .trim()
}

/// Lee el reporte de `gitleaks`. Ausente o ilegible es error de uso.
fn leer_gitleaks(ruta: &Path) -> Result<Vec<Hallazgo>, ErrorOraculo> {
    let texto = fs::read_to_string(ruta)
        .map_err(|e| ErrorOraculo::Uso(format!("reporte de gitleaks {}: {e}", ruta.display())))?;
    parsear_gitleaks(&texto, &[])
        .map_err(|e| ErrorOraculo::Uso(format!("reporte de gitleaks ilegible: {e}")))
}

/// Escribe `a` en `ruta` de forma atómica, con JSON y salto de línea final.
fn escribir_salida(ruta: &Path, a: &Auditoria) -> Result<(), ErrorOraculo> {
    let dir = match ruta.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    fs::create_dir_all(dir)
        .map_err(|e| ErrorOraculo::Infra(format!("crear {}: {e}", dir.display())))?;
    let texto = serde_json::to_string_pretty(a)
        .map_err(|e| ErrorOraculo::Infra(format!("serializar la auditoría: {e}")))?;
    let mut temporal = tempfile::NamedTempFile::new_in(dir)
        .map_err(|e| ErrorOraculo::Infra(format!("temporal en {}: {e}", dir.display())))?;
    temporal
        .write_all(format!("{texto}\n").as_bytes())
        .map_err(|e| ErrorOraculo::Infra(format!("escribir la salida: {e}")))?;
    temporal
        .as_file()
        .sync_all()
        .map_err(|e| ErrorOraculo::Infra(format!("sincronizar la salida: {e}")))?;
    temporal
        .persist(ruta)
        .map_err(|e| ErrorOraculo::Infra(format!("reemplazar {}: {}", ruta.display(), e.error)))?;
    Ok(())
}

/// Mide candidatos de otro modelo con el gate de cita. No llama a ningún modelo ni cuesta nada.
/// `texto_candidatos` tiene la forma de `{"defectos":[...]}`, con o sin cerca de Markdown.
pub fn medir_candidatos(
    repo: &Path,
    incluir: &[PathBuf],
    gitleaks: &[Hallazgo],
    texto_candidatos: &str,
) -> Result<Auditoria, ErrorOraculo> {
    let archivos = leer_archivos(repo, incluir)?;
    let omitidas = lineas_omitidas(gitleaks);
    let contenidos = contenidos_visibles(&archivos, &omitidas);
    let propuestas = parsear_respuesta(texto_candidatos)
        .map_err(|motivo| ErrorOraculo::Uso(format!("candidatos ilegibles: {motivo}")))?;
    let propuestas = ocultar_secretos(propuestas, &archivos, &omitidas);
    let enviados: BTreeSet<String> = contenidos
        .iter()
        .filter(|(_, texto)| !texto.is_empty())
        .map(|(ruta, _)| (*ruta).to_string())
        .collect();
    let mut a = Auditoria {
        hash_prompt: hash_prompt(),
        modelo: "externo".into(),
        archivos: enviados.iter().cloned().collect(),
        llamadas: 0,
        costo_usd: 0.0,
        hallazgos: Vec::new(),
        descartados: Vec::new(),
        incompleta: false,
        sin_leer: Vec::new(),
        avisos: Vec::new(),
        medicion: None,
        lineas_leidas: 0,
        candidatos_por_1000_lineas: None,
    };
    aplicar_gate(
        propuestas,
        &|archivo: &str| enviados.contains(archivo),
        &contenidos,
        &mut a,
    );
    let (lineas, por_mil) = densidad(&contenidos, &a.archivos, &a.sin_leer, a.hallazgos.len());
    a.lineas_leidas = lineas;
    a.candidatos_por_1000_lineas = por_mil;
    Ok(a)
}

/// Lee `--defectos` y valida que sus defectos se puedan medir. Sin ruta, no hay medición.
fn leer_defectos(ruta: Option<&Path>, repo: &Path) -> Result<Option<String>, ErrorOraculo> {
    let Some(ruta) = ruta else {
        return Ok(None);
    };
    let texto = fs::read_to_string(ruta)
        .map_err(|e| ErrorOraculo::Uso(format!("defectos {}: {e}", ruta.display())))?;
    defectos_del_repo(&texto, repo)?;
    Ok(Some(texto))
}

/// Mide candidatos ya escritos en `candidatos`. Exige `o.gitleaks`: sin él es error de uso.
/// Sin hallazgos de gitleaks en el reporte, no se omite ninguna línea.
fn auditar_solo_medir(o: &OpcionesAuditar, candidatos: &Path) -> Result<Auditoria, ErrorOraculo> {
    let Some(ruta_gitleaks) = &o.gitleaks else {
        return Err(ErrorOraculo::Uso(
            "--solo-medir también exige --gitleaks".into(),
        ));
    };
    let texto = fs::read_to_string(candidatos)
        .map_err(|e| ErrorOraculo::Uso(format!("candidatos {}: {e}", candidatos.display())))?;
    let gitleaks = leer_gitleaks(ruta_gitleaks)?;
    let texto_defectos = leer_defectos(o.defectos.as_deref(), &o.repo)?;
    let mut auditoria = medir_candidatos(&o.repo, &o.incluir, &gitleaks, &texto)?;
    if let Some(texto) = &texto_defectos {
        medir_con_aviso(texto, &o.repo, &mut auditoria)?;
    }
    escribir_salida(&o.salida, &auditoria)?;
    Ok(auditoria)
}

/// Corre la pasada con el juez de producción y escribe `o.salida`.
/// Con `solo_medir`, mide candidatos externos sin juez, sin `Config` y sin tope.
/// El reporte de gitleaks se lee antes de armar el juez: si falla, no se envía nada.
pub fn auditar(home: &Path, o: &OpcionesAuditar) -> Result<Auditoria, ErrorOraculo> {
    if let Some(candidatos) = &o.solo_medir {
        return auditar_solo_medir(o, candidatos);
    }
    let (Some(tope_usd), Some(ruta_gitleaks)) = (o.tope_usd, o.gitleaks.as_deref()) else {
        return Err(ErrorOraculo::Uso(
            "sin --solo-medir hacen falta --tope-usd y --gitleaks".into(),
        ));
    };
    if !(tope_usd.is_finite() && tope_usd > 0.0) {
        return Err(ErrorOraculo::Uso(format!(
            "el tope de gasto debe ser finito y mayor que cero: {tope_usd}"
        )));
    }
    let inicio = observe::ahora().0;
    let gitleaks = leer_gitleaks(ruta_gitleaks)?;
    let texto_defectos = leer_defectos(o.defectos.as_deref(), &o.repo)?;

    let mut avisos = Vec::new();
    let juez = super::juez_diseno_de_produccion(home, &mut avisos)
        .ok_or_else(|| ErrorOraculo::Eleccion(avisos.join("; ")))?;
    let modelo = juez.eleccion.modelo.clone().ok_or_else(|| {
        ErrorOraculo::Eleccion("la elección del juez no dice qué modelo usar".into())
    })?;
    let cfg = Config::cargar(&home.join("root.toml"))
        .map_err(|e| ErrorOraculo::Configuracion(e.codigo_de_salida(), e.to_string()))?;
    let precio = cfg.precios.get(&modelo).ok_or_else(|| {
        ErrorOraculo::Eleccion(format!("root.toml no tiene precio para el modelo {modelo}"))
    })?;
    let peor = |p: &Prompt| {
        f64::from(INTENTOS_MAX) * costo_peor_caso(precio, bytes_de_prompt(p), p.max_tokens)
    };
    let eventos = observe::ruta_eventos_en(home);
    let gasto = || {
        observe::uso::gasto_api_desde_en(&eventos, inicio)
            .map_err(|e| ErrorOraculo::Infra(e.to_string()))
    };

    let mut auditoria = auditar_con_limite(
        &o.repo,
        &o.incluir,
        &gitleaks,
        &juez,
        &modelo,
        tope_usd,
        &peor,
        &gasto,
        MAX_ENTRADA_JUEZ_BYTES,
        o.por_archivo,
    )?;
    if let Some(texto) = &texto_defectos {
        medir_con_aviso(texto, &o.repo, &mut auditoria)?;
    }
    escribir_salida(&o.salida, &auditoria)?;
    Ok(auditoria)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use tempfile::TempDir;

    use super::*;
    use crate::proveedores::RespuestaJuez;

    const JSON_VACIO: &str = r#"{"defectos":[]}"#;

    /// Modelo falso. Guarda cada prompt que recibe y responde con `responder(n)`, con `n` las llamadas previas.
    struct ModeloFalso {
        enviados: RefCell<Vec<Prompt>>,
        responder: Box<dyn Fn(usize) -> Result<RespuestaJuez, ErrorOraculo>>,
    }

    impl ModeloFalso {
        fn nuevo(
            responder: impl Fn(usize) -> Result<RespuestaJuez, ErrorOraculo> + 'static,
        ) -> Self {
            Self {
                enviados: RefCell::new(Vec::new()),
                responder: Box::new(responder),
            }
        }

        fn que_responde(texto: &'static str) -> Self {
            Self::nuevo(move |_| respuesta(texto, 0.0))
        }

        fn llamadas(&self) -> usize {
            self.enviados.borrow().len()
        }

        fn prompt(&self, n: usize) -> Prompt {
            self.enviados.borrow()[n].clone()
        }

        fn usuario(&self, n: usize) -> String {
            self.prompt(n).usuario
        }
    }

    impl ModeloJuez for ModeloFalso {
        fn consultar(&self, prompt: &Prompt) -> Result<RespuestaJuez, ErrorOraculo> {
            let n = self.llamadas();
            self.enviados.borrow_mut().push(prompt.clone());
            (self.responder)(n)
        }
    }

    fn respuesta(texto: &str, costo_usd: f64) -> Result<RespuestaJuez, ErrorOraculo> {
        Ok(RespuestaJuez {
            texto: texto.into(),
            truncada: false,
            costo_usd,
        })
    }

    fn peor_cero(_: &Prompt) -> f64 {
        0.0
    }

    fn peor_uno(_: &Prompt) -> f64 {
        1.0
    }

    fn gasto_cero() -> Result<f64, ErrorOraculo> {
        Ok(0.0)
    }

    /// Repo temporal con los archivos dados. La carpeta se llama `root`.
    fn repo_con(archivos: &[(&str, &str)]) -> (TempDir, PathBuf) {
        let tmp = TempDir::new().expect("temporal");
        let repo = tmp.path().join("root");
        fs::create_dir_all(&repo).expect("repo");
        for (ruta, texto) in archivos {
            let destino = repo.join(ruta);
            fs::create_dir_all(destino.parent().expect("padre")).expect("carpetas");
            fs::write(&destino, texto).expect("escribe");
        }
        (tmp, repo)
    }

    fn opciones(repo: PathBuf, tmp: &Path) -> OpcionesAuditar {
        OpcionesAuditar {
            repo,
            salida: tmp.join("auditoria.json"),
            tope_usd: Some(1.0),
            gitleaks: Some(tmp.join("no-existe.json")),
            incluir: Vec::new(),
            defectos: None,
            por_archivo: false,
            solo_medir: None,
        }
    }

    /// Repo con `src/a.rs` de tres líneas de producción y una de tests.
    fn repo_tres_lineas() -> (TempDir, PathBuf) {
        repo_con(&[(
            "src/a.rs",
            "fn uno() {}\nfn calcular(x: u32) -> u32 { x * 2 }\nfn tres() {}\n#[cfg(test)]\nmod t {}\n",
        )])
    }

    #[test]
    fn solo_medir_aplica_el_gate_y_no_llama_a_ningun_modelo() {
        let (_tmp, repo) = repo_tres_lineas();
        let json = r#"{"defectos":[
 {"archivo":"src/a.rs","linea":2,"cita":"fn calcular(x: u32) -> u32 { x * 2 }","detalle":"real"},
 {"archivo":"src/a.rs","linea":3,"cita":"fn no_existe(y: u8) {}","detalle":"falso"}
]}"#;
        let a = medir_candidatos(&repo, &[], &[], json).expect("medición");
        assert_eq!(a.hallazgos.len(), 1, "{:?}", a.hallazgos);
        assert_eq!(a.hallazgos[0].linea, 2);
        assert_eq!(a.descartados.len(), 1, "{:?}", a.descartados);
        assert!(
            a.descartados[0].motivo.contains("cita"),
            "{:?}",
            a.descartados
        );
        assert_eq!(a.llamadas, 0);
        assert_eq!(a.costo_usd, 0.0);
        assert_eq!(a.modelo, "externo");
        assert_eq!(a.archivos, vec!["src/a.rs".to_string()]);
    }

    #[test]
    fn solo_medir_con_candidato_de_archivo_inexistente_va_a_descartados() {
        let (_tmp, repo) = repo_tres_lineas();
        let json = r#"{"defectos":[
 {"archivo":"src/fantasma.rs","linea":1,"cita":"fn uno() {}","detalle":"x"}
]}"#;
        let a = medir_candidatos(&repo, &[], &[], json).expect("medición");
        assert!(a.hallazgos.is_empty(), "{:?}", a.hallazgos);
        assert_eq!(a.descartados.len(), 1, "{:?}", a.descartados);
        assert!(
            a.descartados[0].motivo.contains("no se envió"),
            "{:?}",
            a.descartados
        );
    }

    #[test]
    fn solo_medir_con_json_roto_es_uso() {
        let (_tmp, repo) = repo_tres_lineas();
        let err = medir_candidatos(&repo, &[], &[], "no es json").expect_err("debe fallar");
        assert!(matches!(err, ErrorOraculo::Uso(_)), "{err}");
    }

    /// Repo con un secreto falso en la línea 3 de `src/a.rs`, rodeado de código normal.
    fn repo_con_secreto_en_linea_tres() -> (TempDir, PathBuf) {
        repo_con(&[(
            "src/a.rs",
            "fn calcular_total(x: u32) -> u32 { x * 2 }\nfn uno_de_muchos(x: u32) -> u32 { x + 1 }\nconst CLAVE: &str = \"AKIAFAKEFAKEFAKE1234\";\nfn cuatro_normal(x: u32) -> u32 { x - 1 }\nfn cinco_normal(x: u32) -> u32 { x * 3 }\nfn seis_normal(x: u32) -> u32 { x * 4 }\nfn siete_normal(x: u32) -> u32 { x * 5 }\n",
        )])
    }

    /// Reporte de `gitleaks` con un hallazgo en la línea 3 de `src/a.rs`.
    fn gitleaks_linea_tres() -> Vec<Hallazgo> {
        parsear_gitleaks(
            r#"[{"RuleID":"x","File":"src/a.rs","StartLine":3,"Description":"d"}]"#,
            &[],
        )
        .expect("reporte")
    }

    const SECRETO_FALSO: &str = "AKIAFAKEFAKEFAKE1234";

    #[test]
    fn un_candidato_sobre_el_secreto_va_a_descartados_sin_copiar_su_texto() {
        let (_tmp, repo) = repo_con_secreto_en_linea_tres();
        let json = r#"{"defectos":[{"archivo":"src/a.rs","linea":3,"cita":"const CLAVE: &str = \"AKIAFAKEFAKEFAKE1234\";","detalle":"secreto"}]}"#;
        let a = medir_candidatos(&repo, &[], &gitleaks_linea_tres(), json).expect("medición");
        assert!(a.hallazgos.is_empty(), "{:?}", a.hallazgos);
        assert_eq!(a.descartados.len(), 1, "{:?}", a.descartados);
        assert_eq!(a.descartados[0].cita, "");
        assert_eq!(a.descartados[0].detalle, "");
        assert!(
            a.descartados[0].motivo.contains("secreto"),
            "{:?}",
            a.descartados
        );
        let salida = serde_json::to_string(&a).expect("serializa");
        assert!(!salida.contains(SECRETO_FALSO), "{salida}");
    }

    #[test]
    fn un_candidato_en_la_linea_vecina_al_secreto_va_a_descartados_sin_texto() {
        let (_tmp, repo) = repo_con_secreto_en_linea_tres();
        let json = r#"{"defectos":[{"archivo":"src/a.rs","linea":4,"cita":"const CLAVE: &str = \"AKIAFAKEFAKEFAKE1234\";","detalle":"secreto"}]}"#;
        let a = medir_candidatos(&repo, &[], &gitleaks_linea_tres(), json).expect("medición");
        assert!(a.hallazgos.is_empty(), "{:?}", a.hallazgos);
        assert_eq!(a.descartados.len(), 1, "{:?}", a.descartados);
        assert_eq!(a.descartados[0].cita, "");
        assert_eq!(a.descartados[0].detalle, "");
        assert!(
            a.descartados[0].motivo.contains("secreto"),
            "{:?}",
            a.descartados
        );
        let salida = serde_json::to_string(&a).expect("serializa");
        assert!(!salida.contains(SECRETO_FALSO), "{salida}");
    }

    #[test]
    fn un_candidato_legitimo_cerca_del_secreto_sigue_en_hallazgos() {
        let (_tmp, repo) = repo_con_secreto_en_linea_tres();
        let json = r#"{"defectos":[
 {"archivo":"src/a.rs","linea":1,"cita":"fn calcular_total(x: u32) -> u32 { x * 2 }","detalle":"real"},
 {"archivo":"src/a.rs","linea":7,"cita":"fn siete_normal(x: u32) -> u32 { x * 5 }","detalle":"real"}
]}"#;
        let a = medir_candidatos(&repo, &[], &gitleaks_linea_tres(), json).expect("medición");
        assert_eq!(a.hallazgos.len(), 2, "{:?}", a.hallazgos);
        assert!(a.descartados.is_empty(), "{:?}", a.descartados);
    }

    #[test]
    fn auditar_solo_medir_mide_sin_juez_ni_tope() {
        let (tmp, repo) = repo_tres_lineas();
        let home = TempDir::new().expect("home vacío");
        let candidatos = tmp.path().join("candidatos.json");
        fs::write(
            &candidatos,
            r#"{"defectos":[{"archivo":"src/a.rs","linea":2,"cita":"fn calcular(x: u32) -> u32 { x * 2 }","detalle":"real"}]}"#,
        )
        .expect("escribe");
        let defectos = tmp.path().join("defectos.toml");
        fs::write(
            &defectos,
            "[[defecto]]\nrepo = \"root\"\narchivo = \"src/a.rs\"\nlinea = 2\n",
        )
        .expect("escribe");
        let gitleaks = tmp.path().join("gitleaks.json");
        fs::write(&gitleaks, "[]").expect("escribe");
        let o = OpcionesAuditar {
            tope_usd: None,
            gitleaks: Some(gitleaks),
            defectos: Some(defectos),
            solo_medir: Some(candidatos),
            ..opciones(repo, tmp.path())
        };
        let a = auditar(home.path(), &o).expect("solo medir");
        let medicion = a.medicion.expect("hay medición");
        assert_eq!(medicion.encontrados, 1);
        assert_eq!(medicion.casos_leidos, 1);
        let texto = fs::read_to_string(&o.salida).expect("hay salida");
        let _: Value = serde_json::from_str(&texto).expect("salida JSON");
    }

    #[test]
    fn solo_medir_sin_gitleaks_es_uso() {
        let (tmp, repo) = repo_tres_lineas();
        let home = TempDir::new().expect("home vacío");
        let candidatos = tmp.path().join("candidatos.json");
        fs::write(&candidatos, JSON_VACIO).expect("escribe");
        let o = OpcionesAuditar {
            tope_usd: None,
            gitleaks: None,
            solo_medir: Some(candidatos),
            ..opciones(repo, tmp.path())
        };
        let err = auditar(home.path(), &o).expect_err("solo-medir exige gitleaks");
        assert!(matches!(err, ErrorOraculo::Uso(_)), "{err}");
        assert!(!o.salida.exists());
    }

    #[test]
    fn solo_medir_con_reporte_de_gitleaks_vacio_sigue_midiendo() {
        let (tmp, repo) = repo_tres_lineas();
        let home = TempDir::new().expect("home vacío");
        let candidatos = tmp.path().join("candidatos.json");
        fs::write(
            &candidatos,
            r#"{"defectos":[{"archivo":"src/a.rs","linea":2,"cita":"fn calcular(x: u32) -> u32 { x * 2 }","detalle":"real"}]}"#,
        )
        .expect("escribe");
        let gitleaks = tmp.path().join("gitleaks.json");
        fs::write(&gitleaks, "[]").expect("escribe");
        let o = OpcionesAuditar {
            tope_usd: None,
            gitleaks: Some(gitleaks),
            solo_medir: Some(candidatos),
            ..opciones(repo, tmp.path())
        };
        let a = auditar(home.path(), &o).expect("solo medir con gitleaks vacío");
        assert_eq!(a.hallazgos.len(), 1, "{:?}", a.hallazgos);
        assert_eq!(a.llamadas, 0);
        assert!(o.salida.exists());
    }

    #[test]
    fn auditar_sin_solo_medir_y_sin_tope_es_uso() {
        let (tmp, repo) = repo_tres_lineas();
        let home = TempDir::new().expect("home vacío");
        let o = OpcionesAuditar {
            tope_usd: None,
            ..opciones(repo, tmp.path())
        };
        let err = auditar(home.path(), &o).expect_err("debe fallar");
        assert!(matches!(err, ErrorOraculo::Uso(_)), "{err}");
        assert!(!o.salida.exists());
    }

    #[test]
    fn el_codigo_de_test_no_sale_y_las_lineas_son_reales() {
        let codigo = "fn uno() {}\nfn dos() {}\n  #[cfg(test)]\nmod pruebas { fn en_test() {} }\n";
        let (_tmp, repo) = repo_con(&[("src/lib.rs", codigo)]);
        let modelo = ModeloFalso::que_responde(JSON_VACIO);
        let aud = auditar_con(&repo, &[], &[], &modelo, "m", 10.0, &peor_cero, &gasto_cero)
            .expect("pasada");
        assert_eq!(modelo.llamadas(), 1);
        let usuario = modelo.usuario(0);
        assert!(usuario.contains("2: fn dos() {}\n"), "{usuario}");
        assert!(!usuario.contains("en_test"), "{usuario}");
        assert!(!usuario.contains("cfg(test)"), "{usuario}");
        assert_eq!(aud.archivos, vec!["src/lib.rs".to_string()]);
    }

    #[test]
    fn la_linea_de_un_secreto_sale_omitida() {
        let codigo = "fn uno() {}\nlet clave = \"AKIAEXAMPLE123\";\n";
        let (_tmp, repo) = repo_con(&[("src/lib.rs", codigo)]);
        let json = r#"[{"RuleID":"aws-access-token","File":"src/lib.rs","StartLine":2,"Description":"clave"}]"#;
        let gitleaks = parsear_gitleaks(json, &[]).expect("reporte");
        let modelo = ModeloFalso::que_responde(JSON_VACIO);
        auditar_con(
            &repo,
            &[],
            &gitleaks,
            &modelo,
            "m",
            10.0,
            &peor_cero,
            &gasto_cero,
        )
        .expect("pasada");
        let usuario = modelo.usuario(0);
        assert!(
            usuario.contains("2: [línea con un secreto omitida]\n"),
            "{usuario}"
        );
        assert!(!usuario.contains("AKIA"), "{usuario}");
    }

    #[test]
    fn el_gate_solo_confirma_citas_de_lo_que_el_modelo_vio() {
        let codigo = "fn uno() {}\nlet clave = \"AKIAEXAMPLE123\";\n#[cfg(test)]\nfn solo_en_tests_secreta() {}\n";
        let (_tmp, repo) = repo_con(&[("src/lib.rs", codigo)]);
        let json = r#"[{"RuleID":"aws-access-token","File":"src/lib.rs","StartLine":2,"Description":"clave"}]"#;
        let gitleaks = parsear_gitleaks(json, &[]).expect("reporte");
        let modelo = ModeloFalso::que_responde(
            r#"{"defectos":[
 {"archivo":"src/lib.rs","linea":2,"cita":"let clave = \"AKIAEXAMPLE123\";","detalle":"secreto"},
 {"archivo":"src/lib.rs","linea":4,"cita":"fn solo_en_tests_secreta() {}","detalle":"test"}
]}"#,
        );
        let a = auditar_con(
            &repo,
            &[],
            &gitleaks,
            &modelo,
            "m",
            10.0,
            &peor_cero,
            &gasto_cero,
        )
        .expect("pasada");
        assert!(a.hallazgos.is_empty(), "{:?}", a.hallazgos);
        assert_eq!(a.descartados.len(), 2);
    }

    #[test]
    fn el_tope_corta_antes_de_la_segunda_llamada() {
        // Cada archivo ocupa unos 180 KB con números: juntos no caben en un prompt.
        let grande = "let x = 1;\n".repeat(10_000);
        let (_tmp, repo) =
            repo_con(&[("src/a.rs", grande.as_str()), ("src/b.rs", grande.as_str())]);
        let modelo = ModeloFalso::nuevo(|_| respuesta(JSON_VACIO, 1.0));
        let gasto = || -> Result<f64, ErrorOraculo> { Ok(modelo.llamadas() as f64) };
        let aud =
            auditar_con(&repo, &[], &[], &modelo, "m", 1.5, &peor_uno, &gasto).expect("pasada");
        assert_eq!(modelo.llamadas(), 1);
        assert_eq!(aud.llamadas, 1);
        assert!(aud.incompleta);
        assert_eq!(aud.sin_leer, vec!["src/b.rs".to_string()]);
        assert_eq!(aud.archivos, vec!["src/a.rs".to_string()]);
        assert_eq!(aud.costo_usd, 1.0);
    }

    #[test]
    fn un_tope_que_no_alcanza_para_la_primera_llamada_es_uso_y_no_envia_nada() {
        let (_tmp, repo) = repo_con(&[("src/lib.rs", "fn uno() {}\n")]);
        let modelo = ModeloFalso::que_responde(JSON_VACIO);
        let err = auditar_con(&repo, &[], &[], &modelo, "m", 0.5, &peor_uno, &gasto_cero)
            .expect_err("debe fallar");
        assert!(matches!(err, ErrorOraculo::Uso(_)), "{err}");
        assert!(err.to_string().contains("1.0000"), "{err}");
        assert_eq!(modelo.llamadas(), 0);
    }

    #[test]
    fn un_tope_igual_al_peor_caso_si_alcanza() {
        let (_tmp, repo) = repo_con(&[("src/lib.rs", "fn uno() {}\n")]);
        let modelo = ModeloFalso::que_responde(JSON_VACIO);
        auditar_con(&repo, &[], &[], &modelo, "m", 1.0, &peor_uno, &gasto_cero).expect("pasada");
        assert_eq!(modelo.llamadas(), 1);
    }

    #[test]
    fn un_archivo_que_no_cabe_se_parte_con_numeros_reales() {
        let grande = "let x = 1;\n".repeat(40_000);
        let (_tmp, repo) = repo_con(&[("src/grande.rs", grande.as_str())]);
        let modelo = ModeloFalso::que_responde(JSON_VACIO);
        let aud = auditar_con(&repo, &[], &[], &modelo, "m", 10.0, &peor_cero, &gasto_cero)
            .expect("pasada");
        assert!(modelo.llamadas() >= 3, "llamadas: {}", modelo.llamadas());
        for n in 0..modelo.llamadas() {
            let prompt = modelo.prompt(n);
            assert!(bytes_de_prompt(&prompt) <= MAX_ENTRADA_JUEZ_BYTES);
        }
        let todo: String = (0..modelo.llamadas()).map(|n| modelo.usuario(n)).collect();
        assert!(todo.contains("40000: let x = 1;\n"));
        assert_eq!(aud.archivos, vec!["src/grande.rs".to_string()]);
        assert!(!aud.incompleta);
    }

    #[test]
    fn por_archivo_envia_cada_archivo_en_su_llamada() {
        let (_tmp, repo) = repo_con(&[("src/a.rs", "fn a() {}\n"), ("src/b.rs", "fn b() {}\n")]);
        let modelo = ModeloFalso::que_responde(JSON_VACIO);
        let aud = auditar_con_limite(
            &repo,
            &[],
            &[],
            &modelo,
            "m",
            10.0,
            &peor_cero,
            &gasto_cero,
            MAX_ENTRADA_JUEZ_BYTES,
            true,
        )
        .expect("pasada");
        assert_eq!(modelo.llamadas(), 2);
        assert!(modelo.usuario(0).contains("src/a.rs"));
        assert!(!modelo.usuario(0).contains("src/b.rs"));
        assert!(modelo.usuario(1).contains("src/b.rs"));
        assert!(!modelo.usuario(1).contains("src/a.rs"));
        assert_eq!(modelo.prompt(0).sistema.as_deref(), Some(PROMPT_AUDITORIA));
        assert_eq!(modelo.prompt(1).sistema.as_deref(), Some(PROMPT_AUDITORIA));
        assert_eq!(aud.llamadas, 2);
        assert_eq!(
            aud.archivos,
            vec!["src/a.rs".to_string(), "src/b.rs".to_string()]
        );
    }

    #[test]
    fn sin_por_archivo_los_dos_chicos_van_juntos() {
        let (_tmp, repo) = repo_con(&[("src/a.rs", "fn a() {}\n"), ("src/b.rs", "fn b() {}\n")]);
        let modelo = ModeloFalso::que_responde(JSON_VACIO);
        auditar_con_limite(
            &repo,
            &[],
            &[],
            &modelo,
            "m",
            10.0,
            &peor_cero,
            &gasto_cero,
            MAX_ENTRADA_JUEZ_BYTES,
            false,
        )
        .expect("pasada");
        assert_eq!(modelo.llamadas(), 1);
        assert!(modelo.usuario(0).contains("src/a.rs"));
        assert!(modelo.usuario(0).contains("src/b.rs"));
    }

    #[test]
    fn por_archivo_parte_un_archivo_que_no_cabe() {
        let grande = "let x = 1;\n".repeat(40_000);
        let (_tmp, repo) = repo_con(&[("src/grande.rs", grande.as_str())]);
        let modelo = ModeloFalso::que_responde(JSON_VACIO);
        let aud = auditar_con_limite(
            &repo,
            &[],
            &[],
            &modelo,
            "m",
            10.0,
            &peor_cero,
            &gasto_cero,
            MAX_ENTRADA_JUEZ_BYTES,
            true,
        )
        .expect("pasada");
        assert!(modelo.llamadas() >= 3, "llamadas: {}", modelo.llamadas());
        for n in 0..modelo.llamadas() {
            assert!(bytes_de_prompt(&modelo.prompt(n)) <= MAX_ENTRADA_JUEZ_BYTES);
        }
        assert_eq!(aud.archivos, vec!["src/grande.rs".to_string()]);
        assert!(!aud.incompleta);
    }

    #[test]
    fn una_cita_falsa_va_a_descartados_y_una_real_entra() {
        let (_tmp, repo) =
            repo_con(&[("src/lib.rs", "fn calcular_total(x: u32) -> u32 { x * 2 }\n")]);
        let texto = r#"```json
{"defectos":[
 {"archivo":"src/lib.rs","linea":1,"cita":"fn no_existe_en_el_archivo(y: u8) {}","detalle":"falso"},
 {"archivo":"src/lib.rs","linea":1,"cita":"fn calcular_total(x: u32) -> u32 { x * 2 }","detalle":"real"}
]}
```"#;
        let modelo = ModeloFalso::que_responde(texto);
        let aud = auditar_con(&repo, &[], &[], &modelo, "m", 10.0, &peor_cero, &gasto_cero)
            .expect("pasada");
        assert_eq!(aud.hallazgos.len(), 1, "{:?}", aud.hallazgos);
        assert_eq!(aud.hallazgos[0].detalle, "real");
        assert_eq!(aud.descartados.len(), 1, "{:?}", aud.descartados);
        assert_eq!(
            aud.descartados[0].cita,
            "fn no_existe_en_el_archivo(y: u8) {}"
        );
        assert!(
            aud.descartados[0].motivo.contains("cita"),
            "{}",
            aud.descartados[0].motivo
        );
    }

    #[test]
    fn medir_cuenta_solo_los_defectos_del_repo() {
        let (_tmp, repo) = repo_con(&[]);
        let texto = r#"
[[defecto]]
tarea = "t1"
repo = "root"
archivo = "src/a.rs"
linea = 10
fuente = "x"
detalle = "uno"

[[defecto]]
tarea = "t2"
repo = "/otra/ruta/root"
archivo = "src/a.rs"
linea = 50
fuente = "x"
detalle = "dos"

[[defecto]]
tarea = "t3"
repo = "/x/broot"
archivo = "src/a.rs"
linea = 10
fuente = "x"
detalle = "de otro repo"
"#;
        let hallazgo = Candidato {
            archivo: "src/a.rs".into(),
            linea: 12,
            cita: "cita".into(),
            detalle: "d".into(),
        };
        let leidos: BTreeSet<String> = ["src/a.rs".to_string()].into();
        let m = medir(texto, &repo, &[hallazgo], &leidos).expect("medición");
        assert_eq!(m.casos, 2);
        assert_eq!(m.casos_leidos, 2);
        assert_eq!(m.encontrados, 1);
        assert!(m.detalle[0].encontrado);
        assert_eq!(
            m.detalle[0].hallazgo_cercano.as_ref().map(|h| h.linea),
            Some(12)
        );
        assert!(!m.detalle[1].encontrado);
        assert!(m.detalle[1].hallazgo_cercano.is_none());
    }

    #[test]
    fn un_defecto_en_un_archivo_no_leido_queda_sin_medir() {
        let (_tmp, repo) = repo_con(&[]);
        let texto = r#"
[[defecto]]
repo = "root"
archivo = "src/a.rs"
linea = 10

[[defecto]]
repo = "root"
archivo = "src/b.rs"
linea = 5
"#;
        let hallazgo = Candidato {
            archivo: "src/a.rs".into(),
            linea: 12,
            cita: "cita".into(),
            detalle: "d".into(),
        };
        let leidos: BTreeSet<String> = ["src/a.rs".to_string()].into();
        let m = medir(texto, &repo, &[hallazgo], &leidos).expect("medición");
        assert_eq!(m.casos, 2);
        assert_eq!(m.casos_leidos, 1);
        assert_eq!(m.encontrados, 1);
        assert!(m.detalle[0].medido);
        let b = &m.detalle[1];
        assert!(!b.medido);
        assert!(!b.encontrado);
        assert!(b.hallazgo_cercano.is_none());
    }

    #[test]
    fn medir_con_toml_roto_es_error_de_uso() {
        let (_tmp, repo) = repo_con(&[]);
        let err = medir("[[defecto\n", &repo, &[], &BTreeSet::new()).expect_err("debe fallar");
        assert!(matches!(err, ErrorOraculo::Uso(_)), "{err}");
    }

    /// Conteo corto para armar los mapas esperados.
    fn conteo(casos_leidos: u32, encontrados: u32) -> Conteo {
        Conteo {
            casos_leidos,
            encontrados,
        }
    }

    #[test]
    fn medir_cuenta_por_clase_y_por_origen() {
        let (_tmp, repo) = repo_con(&[]);
        let texto = r#"
[[defecto]]
repo = "root"
archivo = "src/a.rs"
linea = 10
clase = "seguridad"
origen = "real"

[[defecto]]
repo = "root"
archivo = "src/a.rs"
linea = 20
clase = "seguridad"
origen = "sembrado"

[[defecto]]
repo = "root"
archivo = "src/a.rs"
linea = 30
clase = "logica"
origen = "sembrado"

[[defecto]]
repo = "root"
archivo = "src/a.rs"
linea = 40
"#;
        let hallazgo = |linea| Candidato {
            archivo: "src/a.rs".into(),
            linea,
            cita: "cita".into(),
            detalle: "d".into(),
        };
        let hallazgos = vec![hallazgo(11), hallazgo(31)];
        let leidos: BTreeSet<String> = ["src/a.rs".to_string()].into();
        let m = medir(texto, &repo, &hallazgos, &leidos).expect("medición");

        assert_eq!(
            m.por_origen,
            BTreeMap::from([
                ("real".to_string(), conteo(1, 1)),
                ("sembrado".to_string(), conteo(2, 1)),
                ("sin_origen".to_string(), conteo(1, 0)),
            ])
        );
        assert_eq!(
            m.por_clase,
            BTreeMap::from([
                ("seguridad".to_string(), conteo(2, 1)),
                ("logica".to_string(), conteo(1, 1)),
                ("sin_clase".to_string(), conteo(1, 0)),
            ])
        );
        let leidos_por_origen: u32 = m.por_origen.values().map(|c| c.casos_leidos).sum();
        let leidos_por_clase: u32 = m.por_clase.values().map(|c| c.casos_leidos).sum();
        let encontrados_por_origen: u32 = m.por_origen.values().map(|c| c.encontrados).sum();
        let encontrados_por_clase: u32 = m.por_clase.values().map(|c| c.encontrados).sum();
        assert_eq!(leidos_por_origen, m.casos_leidos);
        assert_eq!(leidos_por_clase, m.casos_leidos);
        assert_eq!(encontrados_por_origen, m.encontrados);
        assert_eq!(encontrados_por_clase, m.encontrados);

        assert_eq!(m.detalle[0].clase.as_deref(), Some("seguridad"));
        assert_eq!(m.detalle[0].origen, Some(Origen::Real));
        assert_eq!(m.detalle[3].clase, None);
        assert_eq!(m.detalle[3].origen, None);
    }

    #[test]
    fn un_defecto_no_leido_no_entra_en_los_conteos_por_clase() {
        let (_tmp, repo) = repo_con(&[]);
        let texto = r#"
[[defecto]]
repo = "root"
archivo = "src/b.rs"
linea = 5
clase = "seguridad"
origen = "real"
"#;
        let leidos: BTreeSet<String> = ["src/a.rs".to_string()].into();
        let m = medir(texto, &repo, &[], &leidos).expect("medición");
        assert!(m.por_clase.is_empty());
        assert!(m.por_origen.is_empty());
        assert!(!m.detalle[0].medido);
    }

    #[test]
    fn un_origen_desconocido_es_error_de_uso() {
        let (_tmp, repo) = repo_con(&[]);
        let texto =
            "[[defecto]]\nrepo = \"root\"\narchivo = \"src/a.rs\"\nlinea = 10\norigen = \"otro\"\n";
        let err = medir(texto, &repo, &[], &BTreeSet::new()).expect_err("debe fallar");
        assert!(
            matches!(&err, ErrorOraculo::Uso(m) if m.contains("defectos.toml inválido")),
            "{err}"
        );
    }

    #[test]
    fn defectos_sin_clase_ni_origen_siguen_parseando() {
        let (_tmp, repo) = repo_con(&[]);
        let m = medir(DEFECTOS_DOS_ARCHIVOS, &repo, &[], &BTreeSet::new()).expect("medición");
        assert_eq!(m.casos, 2);
    }

    #[test]
    fn la_medicion_serializa_los_conteos() {
        let (_tmp, repo) = repo_con(&[]);
        let m = medir(DEFECTOS_DOS_ARCHIVOS, &repo, &[], &BTreeSet::new()).expect("medición");
        let json = serde_json::to_value(&m).expect("json");
        assert!(json["por_clase"].is_object(), "{json}");
        assert!(json["por_origen"].is_object(), "{json}");
    }

    /// Auditoría armada a mano. Solo importan `archivos` y `sin_leer`.
    fn auditoria_con(archivos: &[&str], sin_leer: &[&str]) -> Auditoria {
        Auditoria {
            hash_prompt: "h".into(),
            modelo: "m".into(),
            archivos: archivos.iter().map(|s| s.to_string()).collect(),
            llamadas: 1,
            costo_usd: 0.0,
            hallazgos: Vec::new(),
            descartados: Vec::new(),
            incompleta: false,
            sin_leer: sin_leer.iter().map(|s| s.to_string()).collect(),
            avisos: Vec::new(),
            medicion: None,
            lineas_leidas: 0,
            candidatos_por_1000_lineas: None,
        }
    }

    const DEFECTOS_DOS_ARCHIVOS: &str = r#"
[[defecto]]
repo = "root"
archivo = "src/a.rs"
linea = 10

[[defecto]]
repo = "root"
archivo = "src/b.rs"
linea = 5
"#;

    #[test]
    fn sin_ningun_archivo_leido_la_medicion_es_null() {
        let (_tmp, repo) = repo_con(&[]);
        let sin_archivos = auditoria_con(&[], &[]);
        let m = medir_auditoria(DEFECTOS_DOS_ARCHIVOS, &repo, &sin_archivos).expect("medición");
        assert!(m.is_none());
        let todo_sin_leer = auditoria_con(&["src/a.rs", "src/b.rs"], &["src/a.rs", "src/b.rs"]);
        let m = medir_auditoria(DEFECTOS_DOS_ARCHIVOS, &repo, &todo_sin_leer).expect("medición");
        assert!(m.is_none());
    }

    #[test]
    fn con_archivos_leidos_la_medicion_existe() {
        let (_tmp, repo) = repo_con(&[]);
        let a = auditoria_con(&["src/a.rs", "src/b.rs"], &["src/b.rs"]);
        let m = medir_auditoria(DEFECTOS_DOS_ARCHIVOS, &repo, &a)
            .expect("medición")
            .expect("hay medición");
        assert_eq!(m.casos, 2);
        assert_eq!(m.casos_leidos, 1);
    }

    /// Repo cuyo único `.rs` es código de tests: ningún archivo llega a la medición.
    fn repo_solo_tests() -> (TempDir, PathBuf) {
        repo_con(&[("src/a.rs", "#[cfg(test)]\nmod t {}\n")])
    }

    /// Corre `auditar` con `--solo-medir` y gitleaks vacío. Con `defectos`, usa un defecto en `src/a.rs`.
    fn solo_medir_sobre(
        repo: PathBuf,
        tmp: &Path,
        candidatos: &str,
        defectos: bool,
    ) -> Result<Auditoria, ErrorOraculo> {
        let home = TempDir::new().expect("home vacío");
        let ruta_candidatos = tmp.join("candidatos.json");
        fs::write(&ruta_candidatos, candidatos).expect("escribe");
        let gitleaks = tmp.join("gitleaks.json");
        fs::write(&gitleaks, "[]").expect("escribe");
        let ruta_defectos = tmp.join("defectos.toml");
        fs::write(
            &ruta_defectos,
            "[[defecto]]\nrepo = \"root\"\narchivo = \"src/a.rs\"\nlinea = 2\n",
        )
        .expect("escribe");
        let o = OpcionesAuditar {
            tope_usd: None,
            gitleaks: Some(gitleaks),
            defectos: defectos.then_some(ruta_defectos),
            solo_medir: Some(ruta_candidatos),
            ..opciones(repo, tmp)
        };
        auditar(home.path(), &o)
    }

    #[test]
    fn solo_medir_sin_archivos_leidos_avisa_que_no_hay_medicion() {
        let (_repo_tmp, repo) = repo_solo_tests();
        let tmp = TempDir::new().expect("temporal");
        let a = solo_medir_sobre(repo, tmp.path(), JSON_VACIO, true).expect("solo medir");
        assert!(a.medicion.is_none());
        assert!(
            a.avisos
                .contains(&"ningún archivo leído: no hay medición".to_string()),
            "{:?}",
            a.avisos
        );
    }

    #[test]
    fn solo_medir_con_archivo_leido_mide_y_no_avisa() {
        let (_repo_tmp, repo) = repo_tres_lineas();
        let tmp = TempDir::new().expect("temporal");
        let a = solo_medir_sobre(repo, tmp.path(), JSON_VACIO, true).expect("solo medir");
        assert!(a.medicion.is_some());
        assert!(
            !a.avisos
                .contains(&"ningún archivo leído: no hay medición".to_string()),
            "{:?}",
            a.avisos
        );
    }

    #[test]
    fn solo_medir_sin_defectos_no_avisa_aunque_no_se_lea_nada() {
        let (_repo_tmp, repo) = repo_solo_tests();
        let tmp = TempDir::new().expect("temporal");
        let a = solo_medir_sobre(repo, tmp.path(), JSON_VACIO, false).expect("solo medir");
        assert!(a.medicion.is_none());
        assert!(
            !a.avisos
                .contains(&"ningún archivo leído: no hay medición".to_string()),
            "{:?}",
            a.avisos
        );
    }

    #[test]
    fn medir_con_aviso_sin_archivos_leidos_empuja_el_aviso() {
        let (_tmp, repo) = repo_con(&[]);
        let mut a = auditoria_con(&[], &[]);
        medir_con_aviso(DEFECTOS_DOS_ARCHIVOS, &repo, &mut a).expect("medición");
        assert!(a.medicion.is_none());
        assert_eq!(
            a.avisos,
            vec!["ningún archivo leído: no hay medición".to_string()]
        );
    }

    #[test]
    fn medir_con_aviso_con_archivos_leidos_no_empuja_nada() {
        let (_tmp, repo) = repo_con(&[]);
        let mut a = auditoria_con(&["src/a.rs"], &[]);
        medir_con_aviso(DEFECTOS_DOS_ARCHIVOS, &repo, &mut a).expect("medición");
        assert!(a.medicion.is_some());
        assert!(a.avisos.is_empty(), "{:?}", a.avisos);
    }

    #[test]
    fn el_hash_del_prompt_es_estable_y_cambia_con_el_texto() {
        assert_eq!(hash_prompt(), hash_prompt());
        assert_eq!(hash_prompt(), hash_de(PROMPT_AUDITORIA));
        assert_ne!(hash_prompt(), hash_de(&format!("{PROMPT_AUDITORIA} ")));
        assert_eq!(hash_prompt().len(), 16);
    }

    #[test]
    fn un_reporte_de_gitleaks_ilegible_o_ausente_es_uso() {
        let tmp = TempDir::new().expect("temporal");
        let ruta = tmp.path().join("gitleaks.json");
        fs::write(&ruta, "{ no es json").expect("escribe");
        let err = leer_gitleaks(&ruta).expect_err("ilegible");
        assert!(matches!(err, ErrorOraculo::Uso(_)), "{err}");
        let err = leer_gitleaks(&tmp.path().join("no-existe.json")).expect_err("ausente");
        assert!(matches!(err, ErrorOraculo::Uso(_)), "{err}");
    }

    #[test]
    fn auditar_sin_reporte_de_gitleaks_falla_antes_de_armar_el_juez() {
        let (tmp, repo) = repo_con(&[("src/lib.rs", "fn uno() {}\n")]);
        let o = opciones(repo, tmp.path());
        let err = auditar(tmp.path(), &o).expect_err("debe fallar");
        assert!(matches!(err, ErrorOraculo::Uso(_)), "{err}");
        assert!(!o.salida.exists());
    }

    #[test]
    fn la_salida_trae_los_campos_pedidos() {
        let tmp = TempDir::new().expect("temporal");
        let ruta = tmp.path().join("anidado").join("auditoria.json");
        let a = Auditoria {
            hash_prompt: "h".into(),
            modelo: "m".into(),
            archivos: vec!["src/a.rs".into()],
            llamadas: 1,
            costo_usd: 0.5,
            hallazgos: Vec::new(),
            descartados: Vec::new(),
            incompleta: true,
            sin_leer: vec!["src/b.rs".into()],
            avisos: vec!["aviso".into()],
            medicion: None,
            lineas_leidas: 0,
            candidatos_por_1000_lineas: None,
        };
        escribir_salida(&ruta, &a).expect("escribe");
        let texto = fs::read_to_string(&ruta).expect("lee la salida");
        assert!(texto.ends_with("}\n"));
        let v: Value = serde_json::from_str(&texto).expect("json");
        for campo in [
            "hash_prompt",
            "modelo",
            "archivos",
            "llamadas",
            "costo_usd",
            "hallazgos",
            "descartados",
            "incompleta",
            "sin_leer",
            "avisos",
            "medicion",
            "lineas_leidas",
            "candidatos_por_1000_lineas",
        ] {
            assert!(v.get(campo).is_some(), "falta {campo}");
        }
        assert_eq!(v["incompleta"], true);
        assert_eq!(v["sin_leer"][0], "src/b.rs");
        let entradas = fs::read_dir(ruta.parent().expect("padre"))
            .expect("lista")
            .count();
        assert_eq!(entradas, 1, "no queda temporal");
    }

    #[test]
    fn tope_que_no_es_positivo_y_finito_es_uso() {
        let (tmp, repo) = repo_con(&[("src/lib.rs", "fn uno() {}\n")]);
        for tope in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let mut o = opciones(repo.clone(), tmp.path());
            o.tope_usd = Some(tope);
            let err = auditar(tmp.path(), &o).expect_err("debe fallar");
            assert!(matches!(err, ErrorOraculo::Uso(_)), "tope {tope}: {err}");
        }
    }

    #[test]
    fn incluir_fuera_del_repo_es_uso_y_no_envia_nada() {
        let (_tmp, repo) = repo_con(&[("src/lib.rs", "fn uno() {}\n")]);
        let modelo = ModeloFalso::que_responde(JSON_VACIO);
        for ruta in ["/etc", "../otra"] {
            let err = auditar_con(
                &repo,
                &[PathBuf::from(ruta)],
                &[],
                &modelo,
                "m",
                10.0,
                &peor_cero,
                &gasto_cero,
            )
            .expect_err("debe fallar");
            assert!(matches!(err, ErrorOraculo::Uso(_)), "{ruta}: {err}");
        }
        assert_eq!(modelo.llamadas(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn los_symlinks_no_se_siguen() {
        let (tmp, repo) = repo_con(&[("src/lib.rs", "fn uno() {}\n")]);
        let afuera = tmp.path().join("afuera");
        fs::create_dir_all(&afuera).expect("carpeta");
        fs::write(afuera.join("fuera.rs"), "fn fuera_del_repo() {}\n").expect("escribe");
        std::os::unix::fs::symlink(&afuera, repo.join("src").join("enlace")).expect("enlace");
        let modelo = ModeloFalso::que_responde(JSON_VACIO);
        auditar_con(&repo, &[], &[], &modelo, "m", 10.0, &peor_cero, &gasto_cero).expect("pasada");
        assert_eq!(modelo.llamadas(), 1);
        assert!(!modelo.usuario(0).contains("fuera_del_repo"));
    }

    #[test]
    fn un_error_del_modelo_deja_el_resto_sin_leer() {
        let grande = "let x = 1;\n".repeat(10_000);
        let (_tmp, repo) =
            repo_con(&[("src/a.rs", grande.as_str()), ("src/b.rs", grande.as_str())]);
        let modelo = ModeloFalso::nuevo(|_| Err(ErrorOraculo::Infra("caída".into())));
        let aud = auditar_con(&repo, &[], &[], &modelo, "m", 10.0, &peor_cero, &gasto_cero)
            .expect("pasada");
        assert_eq!(modelo.llamadas(), 1);
        assert!(aud.incompleta);
        assert_eq!(
            aud.sin_leer,
            vec!["src/a.rs".to_string(), "src/b.rs".to_string()]
        );
        assert_eq!(aud.avisos.len(), 1, "{:?}", aud.avisos);
    }

    #[test]
    fn una_respuesta_de_formato_roto_deja_solo_su_grupo_sin_leer() {
        let grande = "let x = 1;\n".repeat(10_000);
        let (_tmp, repo) =
            repo_con(&[("src/a.rs", grande.as_str()), ("src/b.rs", grande.as_str())]);
        let modelo = ModeloFalso::nuevo(|n| {
            if n == 0 {
                respuesta("no es json", 0.0)
            } else {
                respuesta(JSON_VACIO, 0.0)
            }
        });
        let aud = auditar_con(&repo, &[], &[], &modelo, "m", 10.0, &peor_cero, &gasto_cero)
            .expect("pasada");
        assert_eq!(aud.llamadas, 2);
        assert_eq!(aud.sin_leer, vec!["src/a.rs".to_string()]);
        assert!(aud.incompleta);
        assert_eq!(aud.avisos.len(), 1, "{:?}", aud.avisos);
    }

    #[test]
    fn la_auditoria_cuenta_lineas_leidas_y_densidad() {
        // Diez líneas de producción y cinco de tests. Solo cuentan las de producción.
        let produccion = "fn uno() {}\nfn dos() {}\nfn calcular_total(x: u32) -> u32 { x * 2 }\nfn cuatro() {}\nfn cinco() {}\nfn seis() {}\nfn siete() {}\nfn ocho() {}\nfn nueve() {}\nfn diez() {}\n";
        let pruebas = "#[cfg(test)]\nmod pruebas {\n    fn uno() {}\n    fn dos() {}\n}\n";
        let codigo = format!("{produccion}{pruebas}");
        let (_tmp, repo) = repo_con(&[("src/a.rs", codigo.as_str())]);
        let modelo = ModeloFalso::que_responde(
            r#"{"defectos":[
 {"archivo":"src/a.rs","linea":3,"cita":"fn calcular_total(x: u32) -> u32 { x * 2 }","detalle":"real"}
]}"#,
        );
        let aud = auditar_con(&repo, &[], &[], &modelo, "m", 10.0, &peor_cero, &gasto_cero)
            .expect("pasada");
        assert_eq!(aud.hallazgos.len(), 1, "{:?}", aud.descartados);
        assert_eq!(aud.lineas_leidas, 10);
        assert_eq!(aud.candidatos_por_1000_lineas, Some(100.0));
    }

    #[test]
    fn un_archivo_sin_leer_no_suma_lineas() {
        let grande = "let x = 1;\n".repeat(10_000);
        let (_tmp, repo) =
            repo_con(&[("src/a.rs", grande.as_str()), ("src/b.rs", grande.as_str())]);
        let modelo = ModeloFalso::nuevo(|_| Err(ErrorOraculo::Infra("caída".into())));
        let aud = auditar_con(&repo, &[], &[], &modelo, "m", 10.0, &peor_cero, &gasto_cero)
            .expect("pasada");
        assert!(
            aud.archivos.contains(&"src/a.rs".to_string()),
            "{:?}",
            aud.archivos
        );
        assert_eq!(aud.lineas_leidas, 0);
        assert_eq!(aud.candidatos_por_1000_lineas, None);
    }

    #[test]
    fn densidad_sin_lineas_es_none() {
        let contenidos: BTreeMap<&str, String> = BTreeMap::from([
            ("src/a.rs", "fn uno() {}\n".to_string()),
            ("src/vacio.rs", String::new()),
        ]);
        let a = vec!["src/a.rs".to_string()];
        let vacio = vec!["src/vacio.rs".to_string()];
        let nada: Vec<String> = Vec::new();
        // Sin archivos leídos: no hay densidad.
        assert_eq!(densidad(&contenidos, &a, &a, 0), (0, None));
        assert_eq!(densidad(&contenidos, &[], &[], 0), (0, None));
        // Archivo leído sin líneas: tampoco hay densidad.
        assert_eq!(densidad(&contenidos, &vacio, &nada, 0), (0, None));
        // Archivo leído con una línea y sin hallazgos: densidad cero.
        assert_eq!(densidad(&contenidos, &a, &nada, 0), (1, Some(0.0)));
    }
}
