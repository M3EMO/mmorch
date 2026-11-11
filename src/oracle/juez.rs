//! Juez de diseño y arquitectura (Tarea 10). Puntúa los ítems `juez` y `mixto` de las rúbricas.
//! Refuta por default: un nivel solo sube con evidencia que pasa el gate. Su JSON es estricto.
//! Las sondas fijan los ítems `sonda`. Una sonda puede poner un tope a un ítem `mixto`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::cambios;
use super::evidencia::{Verificacion, ruta_dentro, verificar_hallazgo};
use super::mapa::Mapa;
use super::perfiles::crear_hallazgo;
use super::perfiles::rust::REGLA_CICLO;
use super::rubrica::{ErrorRubrica, ItemRubrica, Mide, Puntuacion, Rubrica};
use super::tarea::Requisito;
use super::{
    ErrorOraculo, MOTIVO_GATE_REINTENTO, MOTIVO_JUEZ_NIVEL_INVALIDO, MOTIVO_JUEZ_SIN_REINTENTO,
    MOTIVO_REINTENTO_TOPE,
};
use crate::config::root_home;
use crate::proveedores::{self, Prompt, RespuestaJuez};
use crate::tipos::{Dimension, Eleccion, Fuente, Hallazgo, HallazgoDescartado, NivelItem, Via};

/// Ítem de la rúbrica de modularidad. Su tope lo pone un ciclo de módulos.
const MODULARIDAD: &str = "diseno.modularidad";
/// Id de la sonda de arquitectura. Su hallazgo de ciclo fija el tope de `diseno.modularidad`.
const SONDA_ARQUITECTURA: &str = "arquitectura-nativa";
/// Fase del flujo que va al `Evento` de la llamada.
const FASE: &str = "juez-diseno";
/// Marca que se agrega al final de un extracto recortado.
const MARCA_RECORTE: &str = "…[recortado]";
/// Sonda que reporta secretos. Su línea es un secreto: no sale al prompt.
const SONDA_GITLEAKS: &str = "gitleaks";
/// Texto que reemplaza la línea de un hallazgo de `gitleaks` en un extracto.
const LINEA_OMITIDA: &str = "[línea con un secreto omitida]";
const NIVEL_MAX: i64 = 3;
const SEVERIDAD_MAX: i64 = 3;
/// Cuántos archivos grandes del mapa van en los extractos.
const ARCHIVOS_DESTACADOS: usize = 5;
/// Líneas de contexto a cada lado de un cambio en su extracto.
const CONTEXTO: usize = 40;
/// Límite del prompt que se envía al juez, en bytes UTF-8 del sistema y el usuario. Un prompt más grande no se envía
/// y su juicio da sin veredicto. Ninguna cota de calibración usa este valor.
pub const MAX_ENTRADA_JUEZ_BYTES: u64 = 256 * 1024;
/// Prompt fijo del juez. Refuta por default y pide solo un objeto JSON.
const PROMPT_SISTEMA: &str = "Sos un PM técnico que revisa el diseño y la arquitectura de un repo contra su spec.
REFUTA POR DEFAULT: el nivel de cada ítem parte en 0 y solo sube con evidencia citada textual.
No juzgás estilo, nombres ni formato, salvo lo que pide el ancla de un ítem. Solo revisás si el diseño sirve a la spec.
Respondé estas tres preguntas con evidencia:
- ¿Qué requisito queda difícil de extender o de mantener con esta estructura?
- ¿Qué decisión de diseño es un riesgo para producción, como un estado global, un bloqueo o un error que se traga?
- ¿Qué falta para operar el producto: logs, configuración, manejo de errores?
Cada evidencia tiene archivo, linea y cita. La cita es texto textual del archivo, con los mismos caracteres.
El juez responde CADA id de la lista IDS OBLIGATORIOS una sola vez, en ese orden.
Cada ítem trae evidencia que cumple el ancla de su nivel.
Sin evidencia para un ítem, responde \"nivel\":null, \"motivo\":\"sin_evidencia\" y \"evidencia\":[].
Un id sin respuesta, repetido, o sin nivel ni sin_evidencia rompe el contrato.
Un hallazgo exige impacto con ids de requisitos reales. Sin requisito afectado, no reportes el hallazgo.
Respondé SOLO un objeto JSON con esta forma exacta:
{\"items\":[{\"item\":\"<id>\",\"nivel\":0-3,\"evidencia\":[{\"archivo\":\"...\",\"linea\":N,\"cita\":\"<texto textual del archivo>\"}]},{\"item\":\"<id>\",\"nivel\":null,\"motivo\":\"sin_evidencia\",\"evidencia\":[]}],\"hallazgos\":[{\"item\":\"<id>\",\"severidad\":1-3,\"archivo\":\"...\",\"linea\":N,\"cita\":\"...\",\"detalle\":\"...\",\"impacto\":[\"R1\"]}]}
Sin hallazgos: \"hallazgos\":[].";

/// Versión de la forma de elegir los extractos. Cambiarla cambia la huella: los limpios con la lectura vieja no se reusan.
const VERSION_LECTURA: &str = "lectura-guiada-1";

/// Huella de los textos fijos del prompt del juez. Cambiar uno cambia la huella: los limpios guardados con el
/// prompt viejo ya no se reusan.
pub fn huella_del_prompt() -> String {
    crate::goal::calcular_hash(
        format!("{PROMPT_SISTEMA}\n{MARCA_RECORTE}\n{VERSION_LECTURA}").as_bytes(),
    )
}

/// Un modelo que responde al prompt del juez.
pub trait ModeloJuez {
    fn consultar(&self, prompt: &Prompt) -> Result<RespuestaJuez, ErrorOraculo>;
}

/// Juez real: llama al proveedor de `eleccion`. Solo acepta una elección que razona y usa la API.
pub struct JuezProveedor {
    pub eleccion: Eleccion,
    pub corrida: String,
    /// Carpeta de `root.toml` y `eventos.jsonl`: el `Gate` y el evento de cada llamada van aquí.
    pub home: PathBuf,
}

impl JuezProveedor {
    /// Rechaza una elección sin razonamiento, sin API o sin modelo: el verificador razona y refuta.
    /// Es un error de configuración (`Eleccion`), no de infraestructura.
    pub fn nuevo(eleccion: Eleccion, corrida: String) -> Result<Self, ErrorOraculo> {
        Self::nuevo_en(&root_home(), eleccion, corrida)
    }

    /// Como `nuevo`, con `home` para `root.toml` y `eventos.jsonl`.
    pub fn nuevo_en(
        home: &Path,
        eleccion: Eleccion,
        corrida: String,
    ) -> Result<Self, ErrorOraculo> {
        if !eleccion.razonamiento {
            return Err(ErrorOraculo::Eleccion(
                "el juez exige razonamiento prendido; la elección lo tiene apagado".into(),
            ));
        }
        if eleccion.via != Via::Api {
            return Err(ErrorOraculo::Eleccion(
                "el juez exige la vía api; la elección usa otra vía".into(),
            ));
        }
        if eleccion.modelo.is_none() {
            return Err(ErrorOraculo::Eleccion(
                "la elección del juez no dice qué modelo usar".into(),
            ));
        }
        Ok(Self {
            eleccion,
            corrida,
            home: home.to_path_buf(),
        })
    }
}

impl ModeloJuez for JuezProveedor {
    fn consultar(&self, prompt: &Prompt) -> Result<RespuestaJuez, ErrorOraculo> {
        proveedores::llamar_en(&self.home, &self.eleccion, prompt, &self.corrida)
            .map(|respuesta| RespuestaJuez {
                texto: respuesta.texto,
                truncada: respuesta.truncada,
                costo_usd: respuesta.cost_usd,
            })
            .map_err(|e| ErrorOraculo::Infra(e.to_string()))
    }
}

/// Lo que el juez recibe: la spec, los requisitos, el mapa, las sondas y las rúbricas.
pub struct EntradaJuez<'a> {
    pub spec: &'a str,
    pub requisitos: &'a [Requisito],
    pub mapa: &'a Mapa,
    pub hallazgos_sondas: &'a [Hallazgo],
    pub rubricas: &'a [Rubrica],
    pub repo: &'a Path,
    /// Estado inicial del repo (`bootstrap/`). Con él, el juez lee primero lo que cambió.
    pub inicial: Option<&'a Path>,
}

/// Límites del prompt. El `Default` es el de producción.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LimitesPrompt {
    pub mapa_chars: usize,
    pub extractos_total: usize,
    pub extracto_max: usize,
    pub hallazgos_max: usize,
    /// Tope de tokens de salida del juez.
    pub max_tokens: u32,
}

impl Default for LimitesPrompt {
    fn default() -> Self {
        Self {
            mapa_chars: 8000,
            extractos_total: 40_000,
            extracto_max: 6000,
            hallazgos_max: 40,
            max_tokens: proveedores::MAX_TOKENS_JUEZ,
        }
    }
}

/// Estado del juicio. `SinVeredicto` nunca da verde ni "sin problemas".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EstadoJuez {
    Ok,
    SinVeredicto(String),
}

/// Un elemento de la respuesta que se descarta, con su motivo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Descarte {
    pub que: String,
    pub motivo: String,
}

/// Resultado del juez para una evaluación.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultadoJuez {
    pub estado: EstadoJuez,
    pub niveles: BTreeMap<String, u8>,
    pub items: Vec<NivelItem>,
    pub hallazgos: Vec<Hallazgo>,
    pub descartes: Vec<Descarte>,
    pub faltantes: Vec<String>,
    /// Hallazgos rechazados por no tener requisito afectado o por el gate de citas. Un resultado viejo lee vacío.
    #[serde(default)]
    pub hallazgos_descartados: Vec<HallazgoDescartado>,
    /// Ventanas que vieron en el prompt, sin su texto. Un resultado viejo lee vacío.
    #[serde(default)]
    pub extractos: Vec<ExtractoGuardado>,
}

/// Una ventana del prompt del juez, sin su texto. `motivo` es `cambio`, `grande` o `sonda`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ExtractoGuardado {
    pub archivo: String,
    pub desde: u32,
    pub hasta: u32,
    pub motivo: String,
}

/// Arma el prompt del juez. Es determinista: la misma entrada da el mismo prompt.
pub fn construir_prompt(e: &EntradaJuez, limites: &LimitesPrompt) -> Prompt {
    construir_prompt_con(e, limites, None, None, &extractos(e, limites))
}

/// Como `construir_prompt`. Con `ids`, solo pide esos ítems. Con `reintento`, agrega su sección `REINTENTO:`.
/// `leidos` son los extractos de `e`: el llamador los lee una vez y los pasa.
fn construir_prompt_con(
    e: &EntradaJuez,
    limites: &LimitesPrompt,
    ids: Option<&BTreeSet<String>>,
    reintento: Option<&str>,
    leidos: &[Extracto],
) -> Prompt {
    let pedidos: Vec<&ItemRubrica> = pedidos_de(e.rubricas)
        .into_iter()
        .map(|(item, _)| item)
        .filter(|item| ids.is_none_or(|s| s.contains(&item.id)))
        .collect();
    let mut usuario = String::new();
    usuario.push_str("SPEC:\n");
    usuario.push_str(e.spec);
    usuario.push_str("\n\nREQUISITOS:\n");
    for r in e.requisitos {
        usuario.push_str(&format!("- {} (peso {}): {}\n", r.id, r.peso, r.titulo));
    }
    usuario.push_str("\nMAPA:\n");
    usuario.push_str(&e.mapa.a_texto(limites.mapa_chars));
    usuario.push_str("\nHALLAZGOS DE LAS SONDAS:\n");
    if e.hallazgos_sondas.is_empty() {
        usuario.push_str("ninguno\n");
    }
    for h in e.hallazgos_sondas.iter().take(limites.hallazgos_max) {
        usuario.push_str(&format!(
            "- {} | {:?} | {}:{}: {}\n",
            h.id, h.dimension, h.archivo, h.linea, h.detalle
        ));
    }
    // Ids que el juez debe responder, uno por línea y en orden. Sin ellos no hay contrato.
    usuario.push_str("\nIDS OBLIGATORIOS:\n");
    for item in &pedidos {
        usuario.push_str(&format!("- {}\n", item.id));
    }
    usuario.push_str("\nÍTEMS A PUNTUAR:\n");
    for item in &pedidos {
        usuario.push_str(&format!(
            "- {} | fuente: {} | mide: {}\n",
            item.id,
            item.fuente,
            mide_texto(item.mide)
        ));
        for ancla in &item.anclas {
            usuario.push_str(&format!("  {ancla}\n"));
        }
    }
    if let Some(texto) = reintento {
        usuario.push_str("\nREINTENTO:\n");
        usuario.push_str(texto);
    }
    usuario.push_str("\nEXTRACTOS:\n");
    let mut anterior: Option<String> = None;
    for extracto in leidos {
        // Una ventana que sigue a otra del mismo archivo va con la marca de recorte antes.
        if anterior.as_deref() == Some(extracto.archivo.as_str()) {
            usuario.push_str(&format!("{MARCA_RECORTE}\n"));
        } else {
            usuario.push_str(&format!("--- {} ---\n", extracto.archivo));
        }
        usuario.push_str(&extracto.texto);
        anterior = Some(extracto.archivo.clone());
    }
    Prompt {
        sistema: Some(PROMPT_SISTEMA.into()),
        usuario,
        max_tokens: limites.max_tokens,
        fase: FASE.into(),
        reintento_juez: reintento.is_some(),
    }
}

/// Ítems pedidos con su dimensión, y todos los ítems de las rúbricas con su modo de medida.
fn mapas_de(rubricas: &[Rubrica]) -> (BTreeMap<String, Dimension>, BTreeMap<String, Mide>) {
    let pedidos = pedidos_de(rubricas)
        .into_iter()
        .map(|(item, dimension)| (item.id.clone(), dimension))
        .collect();
    let catalogo = rubricas
        .iter()
        .flat_map(|r| r.item.iter())
        .map(|item| (item.id.clone(), item.mide))
        .collect();
    (pedidos, catalogo)
}

/// Hay ítems que el juez puntúa. Sin ellos no hace falta llamarlo.
pub fn hay_pedidos(rubricas: &[Rubrica]) -> bool {
    !pedidos_de(rubricas).is_empty()
}

/// Ítems que el juez puntúa: `juez` o `mixto`, con una dimensión conocida por su prefijo.
fn pedidos_de(rubricas: &[Rubrica]) -> Vec<(&ItemRubrica, Dimension)> {
    rubricas
        .iter()
        .flat_map(|r| r.item.iter())
        .filter(|i| i.mide != Mide::Sonda)
        .filter_map(|i| dimension_de(&i.id).map(|d| (i, d)))
        .collect()
}

pub(crate) fn dimension_de(id: &str) -> Option<Dimension> {
    match id.split('.').next()? {
        "diseno" => Some(Dimension::Diseno),
        "arquitectura" => Some(Dimension::Arquitectura),
        "estructura" => Some(Dimension::Estructura),
        _ => None,
    }
}

fn mide_texto(mide: Mide) -> &'static str {
    match mide {
        Mide::Sonda => "sonda",
        Mide::Juez => "juez",
        Mide::Mixto => "mixto",
    }
}

/// Por qué un extracto va en el prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Motivo {
    /// Tiene líneas cambiadas frente a `bootstrap/`.
    Cambio,
    /// Archivo grande del mapa.
    Grande,
    /// Archivo que citan las sondas.
    Sonda,
}

impl Motivo {
    /// Nombre que se guarda en el resultado: `cambio`, `grande` o `sonda`.
    fn nombre(self) -> &'static str {
        match self {
            Motivo::Cambio => "cambio",
            Motivo::Grande => "grande",
            Motivo::Sonda => "sonda",
        }
    }
}

/// Una ventana de un archivo en el prompt: las líneas `desde..=hasta`, con sus números reales.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Extracto {
    pub archivo: String,
    pub desde: u32,
    pub hasta: u32,
    pub motivo: Motivo,
    pub texto: String,
}

/// Extractos del prompt. Primero van las ventanas de lo que cambió, si hay `inicial`. Después, los archivos
/// grandes del mapa y los citados por sondas, desde su comienzo. Respeta `extracto_max` por ventana y
/// `extractos_total` en conjunto.
pub(crate) fn extractos(e: &EntradaJuez, limites: &LimitesPrompt) -> Vec<Extracto> {
    let secretos = lineas_con_secreto(e.hallazgos_sondas);
    let mut restante = limites.extractos_total;
    let mut salida = Vec::new();
    let mut por_cambio = BTreeSet::new();
    if let Some(inicial) = e.inicial {
        let cambios = cambios::rangos_cambiados(inicial, e.repo);
        por_cambio =
            extractos_de_cambios(e, limites, &cambios, &secretos, &mut restante, &mut salida);
    }
    let destacados = e.mapa.archivos_destacados(ARCHIVOS_DESTACADOS);
    let mut archivos = destacados.clone();
    for h in e.hallazgos_sondas {
        if !archivos.contains(&h.archivo) {
            archivos.push(h.archivo.clone());
        }
    }
    for archivo in archivos {
        if restante == 0 {
            break;
        }
        if por_cambio.contains(&archivo) {
            continue;
        }
        // Solo se lee lo que queda dentro del repo. Se lee la ruta canónica que devuelve `ruta_dentro`.
        let Ok(ruta) = ruta_dentro(e.repo, &archivo) else {
            continue;
        };
        let Ok(contenido) = fs::read_to_string(ruta) else {
            continue;
        };
        let omitidas = secretos.get(archivo.as_str()).cloned().unwrap_or_default();
        let (texto, hasta) = numerar_rango(
            &contenido,
            1,
            usize::MAX,
            restante.min(limites.extracto_max),
            &omitidas,
            limites.extracto_max,
        );
        if texto.is_empty() {
            continue;
        }
        restante = restante.saturating_sub(texto.chars().count());
        let motivo = if destacados.contains(&archivo) {
            Motivo::Grande
        } else {
            Motivo::Sonda
        };
        salida.push(Extracto {
            archivo,
            desde: 1,
            hasta: hasta as u32,
            motivo,
            texto,
        });
    }
    salida
}

/// Ventanas de los archivos con cambios, de más líneas cambiadas a menos (desempate por ruta). Agrega sus
/// extractos a `salida` y gasta `restante`. Devuelve los archivos que tuvieron al menos una ventana.
fn extractos_de_cambios(
    e: &EntradaJuez,
    limites: &LimitesPrompt,
    cambios: &BTreeMap<String, Vec<(u32, u32)>>,
    secretos: &BTreeMap<&str, BTreeSet<u32>>,
    restante: &mut usize,
    salida: &mut Vec<Extracto>,
) -> BTreeSet<String> {
    let mut orden: Vec<(&String, &Vec<(u32, u32)>)> = cambios.iter().collect();
    orden.sort_by(|a, b| {
        lineas_cambiadas(b.1)
            .cmp(&lineas_cambiadas(a.1))
            .then_with(|| a.0.cmp(b.0))
    });
    let mut entraron = BTreeSet::new();
    for (archivo, rangos) in orden {
        if *restante == 0 {
            break;
        }
        let Ok(ruta) = ruta_dentro(e.repo, archivo) else {
            continue;
        };
        let Ok(contenido) = fs::read_to_string(ruta) else {
            continue;
        };
        let lineas: Vec<&str> = contenido.lines().collect();
        let omitidas = secretos.get(archivo.as_str()).cloned().unwrap_or_default();
        let mut emitidas = 0;
        for (desde, hasta) in ventanas(rangos, &lineas, &omitidas, limites.extracto_max) {
            // Entre dos ventanas del mismo archivo va la marca de recorte, en su propia línea.
            let separador = if emitidas > 0 { largo_separador() } else { 0 };
            if *restante <= separador {
                break;
            }
            let presupuesto = *restante - separador;
            let filas = largo_filas(&lineas, desde, hasta, &omitidas, limites.extracto_max);
            let limite = limite_de_emision(filas, presupuesto);
            let (texto, ultima) = numerar_rango(
                &contenido,
                desde,
                hasta,
                limite,
                &omitidas,
                limites.extracto_max,
            );
            if texto.is_empty() {
                continue;
            }
            *restante -= separador + texto.chars().count();
            salida.push(Extracto {
                archivo: archivo.clone(),
                desde: desde as u32,
                hasta: ultima as u32,
                motivo: Motivo::Cambio,
                texto,
            });
            emitidas += 1;
        }
        if emitidas > 0 {
            entraron.insert(archivo.clone());
        }
    }
    entraron
}

/// Líneas cambiadas de un archivo: la suma de sus rangos.
fn lineas_cambiadas(rangos: &[(u32, u32)]) -> usize {
    rangos
        .iter()
        .map(|&(desde, hasta)| (hasta - desde + 1) as usize)
        .sum()
}

/// Ventanas de un archivo de `lineas`: cada rango con `CONTEXTO` líneas a cada lado. Las que se tocan o se
/// solapan se unen. Una ventana que pasa `limite` caracteres baja su contexto, por igual de cada lado, hasta
/// que entra. Si ni con contexto 0 entra, se parte en ventanas consecutivas que entran.
fn ventanas(
    rangos: &[(u32, u32)],
    lineas: &[&str],
    omitidas: &BTreeSet<u32>,
    limite: usize,
) -> Vec<(usize, usize)> {
    let total = lineas.len();
    let mut salida = Vec::new();
    for (desde, hasta) in expandir_y_unir(rangos, total, CONTEXTO) {
        // Los rangos del bloque: los que caen dentro de la ventana con contexto completo.
        let del_bloque: Vec<(u32, u32)> = rangos
            .iter()
            .copied()
            .filter(|&(a, b)| (a as usize) <= hasta && (b as usize) >= desde)
            .collect();
        let mut contexto = CONTEXTO;
        let mut partes = expandir_y_unir(&del_bloque, total, contexto);
        while contexto > 0
            && !partes
                .iter()
                .all(|&(d, h)| largo_filas(lineas, d, h, omitidas, limite) <= limite)
        {
            contexto -= 1;
            partes = expandir_y_unir(&del_bloque, total, contexto);
        }
        for (d, h) in partes {
            salida.extend(partir(d, h, lineas, omitidas, limite));
        }
    }
    salida
}

/// Expande cada rango con `contexto` líneas a cada lado, dentro de `1..=total`. Las que se tocan o se
/// solapan se unen.
fn expandir_y_unir(rangos: &[(u32, u32)], total: usize, contexto: usize) -> Vec<(usize, usize)> {
    let mut expandidas: Vec<(usize, usize)> = rangos
        .iter()
        .map(|&(desde, hasta)| {
            let desde = (desde as usize).saturating_sub(contexto).max(1);
            let hasta = (hasta as usize).saturating_add(contexto).min(total);
            (desde, hasta)
        })
        .filter(|(desde, hasta)| desde <= hasta)
        .collect();
    expandidas.sort();
    let mut unidas: Vec<(usize, usize)> = Vec::new();
    for (desde, hasta) in expandidas {
        match unidas.last_mut() {
            Some(ultima) if desde <= ultima.1 + 1 => ultima.1 = ultima.1.max(hasta),
            _ => unidas.push((desde, hasta)),
        }
    }
    unidas
}

/// Parte `desde..=hasta` en ventanas consecutivas que entran en `limite`. Una línea que sola pasa `limite`
/// entra como su marca de largo. Solo si ni la marca entra, la línea se deja fuera.
fn partir(
    desde: usize,
    hasta: usize,
    lineas: &[&str],
    omitidas: &BTreeSet<u32>,
    limite: usize,
) -> Vec<(usize, usize)> {
    let mut salida = Vec::new();
    let mut inicio = desde;
    while inicio <= hasta {
        let mut fin = inicio - 1;
        let mut usados = 0;
        while fin < hasta {
            let largo = largo_filas(lineas, fin + 1, fin + 1, omitidas, limite);
            if usados + largo > limite {
                break;
            }
            usados += largo;
            fin += 1;
        }
        if fin < inicio {
            inicio += 1;
        } else {
            salida.push((inicio, fin));
            inicio = fin + 1;
        }
    }
    salida
}

/// Fila de la línea `numero` (1-based): `NN: texto\n`, como la escribe `numerar_rango`. Una línea de
/// `omitidas` sale con `LINEA_OMITIDA`. Una línea que sola pasa `max` caracteres sale con su marca de largo.
fn fila_de(numero: usize, lineas: &[&str], omitidas: &BTreeSet<u32>, max: usize) -> String {
    if omitidas.contains(&(numero as u32)) {
        return format!("{numero}: {LINEA_OMITIDA}\n");
    }
    let fila = format!("{numero}: {}\n", lineas[numero - 1]);
    if fila.chars().count() <= max {
        fila
    } else {
        format!("{numero}: [línea {numero} omitida por largo]\n")
    }
}

/// Caracteres de las filas `desde..=hasta` como las escribe `numerar_rango`, sin marca de recorte. Acotadas
/// al archivo.
fn largo_filas(
    lineas: &[&str],
    desde: usize,
    hasta: usize,
    omitidas: &BTreeSet<u32>,
    max: usize,
) -> usize {
    (desde..=hasta.min(lineas.len()))
        .map(|numero| fila_de(numero, lineas, omitidas, max).chars().count())
        .sum()
}

/// Límite para `numerar_rango` de una ventana de `filas` caracteres con `presupuesto` libre. Si la ventana
/// entra, el límite le deja sitio exacto: no corta. Si no entra, el límite es el presupuesto y corta con la
/// marca.
fn limite_de_emision(filas: usize, presupuesto: usize) -> usize {
    if filas <= presupuesto {
        filas + largo_separador()
    } else {
        presupuesto
    }
}

/// Largo de la línea de marca entre dos ventanas: la marca y su salto de línea.
fn largo_separador() -> usize {
    MARCA_RECORTE.chars().count() + 1
}

/// Líneas de cada archivo que cita un hallazgo de `gitleaks`. Todas van omitidas en el extracto.
fn lineas_con_secreto(hallazgos: &[Hallazgo]) -> BTreeMap<&str, BTreeSet<u32>> {
    let mut secretos: BTreeMap<&str, BTreeSet<u32>> = BTreeMap::new();
    for h in hallazgos {
        if matches!(&h.fuente, Fuente::Sonda(s) if s == SONDA_GITLEAKS) {
            secretos
                .entry(h.archivo.as_str())
                .or_default()
                .insert(h.linea);
        }
    }
    secretos
}

/// Líneas `desde..=hasta` (1-based, acotadas al archivo) con su número real (`NN: `), como `fila_de`. El
/// texto, con la marca de recorte, no pasa `limite` caracteres. Vacío si ni una fila cabe: así no sale una
/// ventana con solo la marca. Devuelve también la última línea escrita.
fn numerar_rango(
    contenido: &str,
    desde: usize,
    hasta: usize,
    limite: usize,
    omitidas: &BTreeSet<u32>,
    max: usize,
) -> (String, usize) {
    let lineas: Vec<&str> = contenido.lines().collect();
    let marca = format!("{MARCA_RECORTE}\n");
    let tope = limite.saturating_sub(marca.chars().count());
    let mut salida = String::new();
    let mut usados = 0;
    let mut ultima = 0;
    for numero in desde..=hasta.min(lineas.len()) {
        let fila = fila_de(numero, &lineas, omitidas, max);
        let largo = fila.chars().count();
        if usados + largo > tope {
            if salida.is_empty() {
                return (String::new(), 0);
            }
            salida.push_str(&marca);
            break;
        }
        salida.push_str(&fila);
        usados += largo;
        ultima = numero;
    }
    (salida, ultima)
}

/// Bytes UTF-8 que el modelo recibe de un prompt: el sistema y el usuario.
pub fn bytes_de_prompt(prompt: &Prompt) -> u64 {
    let sistema = prompt.sistema.as_deref().map_or(0, str::len);
    (sistema + prompt.usuario.len()) as u64
}

/// Motivo del juicio que no envía un prompt de más de `MAX_ENTRADA_JUEZ_BYTES`. `None` si cabe.
pub fn exceso_de_entrada(prompt: &Prompt) -> Option<String> {
    let bytes = bytes_de_prompt(prompt);
    (bytes > MAX_ENTRADA_JUEZ_BYTES).then(|| {
        format!("el prompt tiene {bytes} bytes y supera el límite de {MAX_ENTRADA_JUEZ_BYTES} bytes del juez")
    })
}

/// Pide el juicio al modelo y aplica el parseo estricto. Un fallo da su causa en el `Err`.
fn pedir(modelo: &dyn ModeloJuez, prompt: &Prompt) -> Result<Respuesta, String> {
    let salida = modelo
        .consultar(prompt)
        .map_err(|err| format!("el modelo falló: {err}"))?;
    // Antes de parsear: una respuesta cortada por el tope no es JSON roto, es truncada.
    if salida.truncada {
        return Err(proveedores::causa_tope_alcanzado(prompt.max_tokens));
    }
    parsear(&salida.texto)
}

/// Pide el juicio al modelo y aplica el parseo estricto y el gate. Hace como mucho un reintento.
pub fn juzgar(
    e: &EntradaJuez,
    modelo: &dyn ModeloJuez,
    nombre_modelo: &str,
    limites: &LimitesPrompt,
) -> ResultadoJuez {
    let (pedidos, catalogo) = mapas_de(e.rubricas);
    // Se leen una vez. El prompt y el reintento usan las mismas ventanas.
    let leidos = extractos(e, limites);
    let guardados = guardar_extractos(&leidos);
    let prompt = construir_prompt_con(e, limites, None, None, &leidos);
    if let Some(motivo) = exceso_de_entrada(&prompt) {
        return sin_veredicto(motivo, &pedidos, guardados);
    }
    let respuesta = match pedir(modelo, &prompt) {
        Ok(respuesta) => respuesta,
        Err(causa) => return sin_veredicto(causa, &pedidos, guardados),
    };
    let cx = Contexto {
        repo: e.repo,
        requisitos: e.requisitos,
        pedidos: &pedidos,
        catalogo: &catalogo,
        modelo: nombre_modelo,
        leidos: &guardados,
    };
    let mut lectura = leer_items(respuesta.items, &cx);
    let mut hallazgos = Vec::new();
    let mut hallazgos_descartados = Vec::new();
    for valor in respuesta.hallazgos {
        match procesar_hallazgo(valor, &cx, &mut hallazgos_descartados) {
            Ok(hallazgo) => hallazgos.push(hallazgo),
            Err(d) => lectura.descartes.push(d),
        }
    }
    // Un solo reintento: los ids que rompieron el contrato o no pasaron el gate.
    let reintento: BTreeSet<String> = lectura
        .rotos
        .iter()
        .chain(lectura.gate.keys())
        .cloned()
        .collect();
    if !reintento.is_empty() {
        let otra = reintentar(e, modelo, limites, &cx, &reintento, &lectura, &leidos);
        for nivel in otra.items {
            lectura.niveles.insert(nivel.item.clone(), nivel.nivel);
            lectura.items.push(nivel);
        }
        lectura.descartes.extend(otra.descartes);
        // Sin nivel tras el reintento, y sin `sin_evidencia` en él: el juez no respondió.
        // Si la evidencia del reintento no pasó el gate, el motivo es el gate, no la falta de respuesta.
        let sin_nivel: Vec<String> = reintento
            .iter()
            .filter(|id| !lectura.niveles.contains_key(*id) && !otra.sin_evidencia.contains(*id))
            .cloned()
            .collect();
        for id in sin_nivel {
            let motivo = if otra.frenado_por_tope {
                MOTIVO_REINTENTO_TOPE
            } else if otra.gate.contains_key(&id) {
                MOTIVO_GATE_REINTENTO
            } else if otra.invalidos.contains(&id) {
                MOTIVO_JUEZ_NIVEL_INVALIDO
            } else {
                MOTIVO_JUEZ_SIN_REINTENTO
            };
            lectura
                .descartes
                .push(descarte(format!("ítem {id}"), motivo));
        }
    }
    let faltantes = pedidos
        .keys()
        .filter(|id| !lectura.niveles.contains_key(*id))
        .cloned()
        .collect();
    ResultadoJuez {
        estado: EstadoJuez::Ok,
        niveles: lectura.niveles,
        items: lectura.items,
        hallazgos,
        descartes: lectura.descartes,
        faltantes,
        hallazgos_descartados,
        extractos: guardados,
    }
}

/// Reintento de `ids`: el mismo prompt, con solo esos ítems y su sección `REINTENTO:`. De su respuesta
/// toma niveles, `sin_evidencia` y descartes. Sin respuesta usable, la lectura queda vacía.
fn reintentar(
    e: &EntradaJuez,
    modelo: &dyn ModeloJuez,
    limites: &LimitesPrompt,
    cx: &Contexto,
    ids: &BTreeSet<String>,
    lectura: &Lectura,
    leidos: &[Extracto],
) -> Lectura {
    let texto = texto_reintento(&lectura.gate, &lectura.rotos, &lectura.invalidos);
    let prompt = construir_prompt_con(e, limites, Some(ids), Some(&texto), leidos);
    if exceso_de_entrada(&prompt).is_some() {
        return Lectura::default();
    }
    let respuesta = match pedir(modelo, &prompt) {
        Ok(respuesta) => respuesta,
        Err(causa) => {
            return Lectura {
                frenado_por_tope: es_tope_de_api(&causa),
                ..Lectura::default()
            };
        }
    };
    // Un id fuera del reintento no se pidió de nuevo: `leer_items` lo descarta con aviso.
    let pedidos: BTreeMap<String, Dimension> = cx
        .pedidos
        .iter()
        .filter(|(id, _)| ids.contains(*id))
        .map(|(id, dimension)| (id.clone(), *dimension))
        .collect();
    let cx_reintento = Contexto {
        repo: cx.repo,
        requisitos: cx.requisitos,
        pedidos: &pedidos,
        catalogo: cx.catalogo,
        modelo: cx.modelo,
        leidos: cx.leidos,
    };
    leer_items(respuesta.items, &cx_reintento)
}

/// Texto de la sección `REINTENTO:`: una línea por cita que no pasó el gate, y una por id roto.
/// Un id con nivel inválido recibe una línea que dice qué nivel se espera.
fn texto_reintento(
    gate: &BTreeMap<String, Vec<CitaFallida>>,
    rotos: &BTreeSet<String>,
    invalidos: &BTreeSet<String>,
) -> String {
    let mut texto = String::new();
    for (id, citas) in gate {
        if citas.is_empty() {
            texto.push_str(&format!(
                "- {id}: ninguna evidencia trae archivo, línea y cita válidos\n"
            ));
        }
        for c in citas {
            texto.push_str(&format!(
                "- {id}: la cita \"{}\" no se encontró en {}:{}\n",
                c.cita, c.archivo, c.linea
            ));
        }
    }
    for id in rotos {
        if invalidos.contains(id) {
            texto.push_str(&format!(
                "- {id}: el nivel debe ser un entero de 0 a 3 o null con sin_evidencia\n"
            ));
        } else {
            texto.push_str(&format!("- {id}: no respondió o rompió el contrato\n"));
        }
    }
    texto
}

/// Resultado sin veredicto: sin niveles ni hallazgos, y todo lo pedido queda como faltante.
/// Los extractos guardados sí van: el prompt ya se armó cuando el juez falla.
fn sin_veredicto(
    causa: String,
    pedidos: &BTreeMap<String, Dimension>,
    extractos: Vec<ExtractoGuardado>,
) -> ResultadoJuez {
    ResultadoJuez {
        estado: EstadoJuez::SinVeredicto(causa),
        niveles: BTreeMap::new(),
        items: Vec::new(),
        hallazgos: Vec::new(),
        descartes: Vec::new(),
        faltantes: pedidos.keys().cloned().collect(),
        hallazgos_descartados: Vec::new(),
        extractos,
    }
}

/// Copia las ventanas sin su texto, para guardarlas en el resultado.
fn guardar_extractos(leidos: &[Extracto]) -> Vec<ExtractoGuardado> {
    leidos
        .iter()
        .map(|e| ExtractoGuardado {
            archivo: e.archivo.clone(),
            desde: e.desde,
            hasta: e.hasta,
            motivo: e.motivo.nombre().into(),
        })
        .collect()
}

/// Lo que los procesadores de cada elemento necesitan para validarlo.
struct Contexto<'a> {
    repo: &'a Path,
    requisitos: &'a [Requisito],
    pedidos: &'a BTreeMap<String, Dimension>,
    catalogo: &'a BTreeMap<String, Mide>,
    modelo: &'a str,
    /// Extractos que el juez leyó. Los hallazgos deben caer dentro de alguno.
    leidos: &'a [ExtractoGuardado],
}

impl Contexto<'_> {
    /// Dimensión de un ítem pedido. Un id que no se pidió se ignora, y el motivo lo dice.
    fn dimension(&self, id: &str) -> Result<Dimension, String> {
        match self.catalogo.get(id) {
            None => Err(format!(
                "no se pidió; se ignora: el ítem {id} no existe en las rúbricas pedidas"
            )),
            Some(Mide::Sonda) => {
                Err("no se pidió; se ignora: el juez no puntúa ítems de sonda".into())
            }
            Some(_) => self.pedidos.get(id).copied().ok_or_else(|| {
                format!("no se pidió; se ignora: el ítem {id} no está entre los pedidos")
            }),
        }
    }

    fn fuente(&self) -> Fuente {
        Fuente::Juez {
            modelo: self.modelo.into(),
        }
    }
}

/// Valor de `motivo` con el que el juez dice que no halló evidencia para un ítem.
const SIN_EVIDENCIA_DEL_JUEZ: &str = "sin_evidencia";

/// Lo que dice el juez de un ítem pedido, ya validado.
enum Dicho {
    Nivel(NivelItem),
    /// El juez no halló evidencia. No rompe el contrato.
    SinEvidencia(String),
    /// Hay nivel, pero ninguna evidencia pasa el gate. Lleva las citas que fallaron.
    SinGate {
        id: String,
        citas: Vec<CitaFallida>,
    },
}

/// Lectura de los ítems de la respuesta: niveles, descartes y los ids que rompen el contrato.
#[derive(Debug, Default)]
struct Lectura {
    niveles: BTreeMap<String, u8>,
    items: Vec<NivelItem>,
    descartes: Vec<Descarte>,
    /// Ids pedidos que rompen el contrato: omitidos, repetidos, o sin nivel ni `sin_evidencia`.
    rotos: BTreeSet<String>,
    /// Ids con un nivel presente que no es un entero de 0 a 3. Subconjunto de `rotos`.
    invalidos: BTreeSet<String>,
    /// Ids con nivel que no pasaron el gate, con las citas que fallaron.
    gate: BTreeMap<String, Vec<CitaFallida>>,
    /// Ids que el juez dio por `sin_evidencia`.
    sin_evidencia: BTreeSet<String>,
    /// El reintento no se envió porque el tope de API frenó la llamada.
    frenado_por_tope: bool,
}

/// Texto que trae el error del tope de API cuando frena una llamada antes de enviarla.
const TEXTO_TOPE_API: &str = "tope de API";
/// Texto del error de la llamada que no se envía: el tope no pudo leer o reservar el gasto.
const TEXTO_NO_ENVIADA: &str = "la llamada no se envía";

/// Un error del modelo que frenó la llamada por el tope de API. Mira el texto: "tope" a secas no cuenta.
fn es_tope_de_api(causa: &str) -> bool {
    causa.contains(TEXTO_TOPE_API) || causa.contains(TEXTO_NO_ENVIADA)
}

/// Evidencia que no pasó el gate: archivo, línea y cita tal como la dijo el juez.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CitaFallida {
    archivo: String,
    linea: u32,
    cita: String,
}

/// Lee los ítems de la respuesta. Un id pedido repetido no cuenta: ninguna de sus copias sirve.
fn leer_items(valores: Vec<Value>, cx: &Contexto) -> Lectura {
    let mut apariciones: BTreeMap<String, usize> = BTreeMap::new();
    for id in valores.iter().filter_map(id_de) {
        *apariciones.entry(id).or_default() += 1;
    }
    let mut lectura = Lectura::default();
    for valor in valores {
        if let Some(id) = id_de(&valor).filter(|id| cx.pedidos.contains_key(id)) {
            if apariciones[&id] > 1 {
                lectura.descartes.push(descarte(
                    format!("ítem {id}"),
                    "ítem repetido; rompe el contrato",
                ));
                lectura.rotos.insert(id);
                continue;
            }
            if nivel_invalido(&valor) {
                lectura.rotos.insert(id.clone());
                lectura.invalidos.insert(id);
            } else if sin_nivel_ni_evidencia(&valor) {
                lectura.rotos.insert(id);
            }
        }
        match procesar_item(valor, cx) {
            Ok(Dicho::Nivel(nivel)) => {
                lectura.niveles.insert(nivel.item.clone(), nivel.nivel);
                lectura.items.push(nivel);
            }
            Ok(Dicho::SinEvidencia(id)) => {
                lectura.descartes.push(descarte(
                    format!("ítem {id}"),
                    "sin_evidencia: el juez no encontró evidencia",
                ));
                lectura.sin_evidencia.insert(id);
            }
            Ok(Dicho::SinGate { id, citas }) => {
                lectura.descartes.push(descarte(
                    format!("ítem {id}"),
                    "sin evidencia que pase el gate",
                ));
                lectura.gate.insert(id, citas);
            }
            Err(d) => lectura.descartes.push(d),
        }
    }
    for id in cx.pedidos.keys() {
        if !apariciones.contains_key(id) {
            lectura.rotos.insert(id.clone());
        }
    }
    lectura
}

/// Id de un ítem crudo, si trae uno de texto.
fn id_de(valor: &Value) -> Option<String> {
    valor.get("item")?.as_str().map(str::to_string)
}

/// Un nivel presente, no null, que no es un entero de 0 a 3. Rompe el contrato.
fn nivel_invalido(valor: &Value) -> bool {
    match valor.get("nivel") {
        None | Some(Value::Null) => false,
        Some(nivel) => !nivel_entero(nivel).is_some_and(|n| (0..=NIVEL_MAX).contains(&n)),
    }
}

/// Nivel entero de un valor JSON: un entero, o un flotante finito sin parte fraccionaria (2.0 vale 2).
fn nivel_entero(valor: &Value) -> Option<i64> {
    let numero = valor.as_number()?;
    numero.as_i64().or_else(|| {
        numero
            .as_f64()
            .filter(|f| f.is_finite() && f.fract() == 0.0)
            .map(|f| f as i64)
    })
}

/// Un ítem sin nivel y sin `sin_evidencia`. Rompe el contrato.
fn sin_nivel_ni_evidencia(valor: &Value) -> bool {
    let sin_nivel = valor.get("nivel").is_none_or(Value::is_null);
    let sin_evidencia = valor.get("motivo").and_then(Value::as_str) == Some(SIN_EVIDENCIA_DEL_JUEZ);
    sin_nivel && !sin_evidencia
}

fn descarte(que: impl Into<String>, motivo: impl Into<String>) -> Descarte {
    Descarte {
        que: que.into(),
        motivo: motivo.into(),
    }
}

/// Respuesta del juez: dos arreglos, `items` y `hallazgos`.
struct Respuesta {
    items: Vec<Value>,
    hallazgos: Vec<Value>,
}

/// Parseo estricto: un objeto JSON con las claves `items` y `hallazgos`, con o sin un bloque ```json.
fn parsear(texto: &str) -> Result<Respuesta, String> {
    let valor: Value =
        serde_json::from_str(desenvuelto(texto)).map_err(|e| format!("JSON inválido: {e}"))?;
    let Value::Object(mut campos) = valor else {
        return Err("se esperaba un objeto JSON".into());
    };
    let items = arreglo(campos.remove("items"), "items")?;
    let hallazgos = arreglo(campos.remove("hallazgos"), "hallazgos")?;
    Ok(Respuesta { items, hallazgos })
}

fn arreglo(valor: Option<Value>, clave: &str) -> Result<Vec<Value>, String> {
    match valor {
        Some(Value::Array(lista)) => Ok(lista),
        Some(_) => Err(format!("{clave} debe ser un arreglo")),
        None => Err(format!("falta la clave {clave}")),
    }
}

/// Quita un bloque ```json ... ``` que envuelve todo el texto. Cualquier otro texto queda igual.
pub(crate) fn desenvuelto(texto: &str) -> &str {
    let t = texto.trim();
    let Some(resto) = t.strip_prefix("```") else {
        return t;
    };
    let Some((info, cuerpo)) = resto.split_once('\n') else {
        return t;
    };
    if !matches!(info.trim(), "" | "json") {
        return t;
    }
    match cuerpo.trim_end().strip_suffix("```") {
        Some(interior) => interior.trim(),
        None => t,
    }
}

#[derive(Debug, Deserialize)]
struct ItemCrudo {
    #[serde(default)]
    item: String,
    /// Valor crudo: `nivel_entero` lo normaliza, así 2.0 vale como 2.
    #[serde(default)]
    nivel: Option<Value>,
    #[serde(default)]
    motivo: Option<String>,
    #[serde(default)]
    evidencia: Vec<Value>,
}

#[derive(Debug, Deserialize)]
struct EvidenciaCruda {
    #[serde(default)]
    archivo: String,
    #[serde(default)]
    linea: u32,
    #[serde(default)]
    cita: String,
}

#[derive(Debug, Deserialize)]
struct HallazgoCrudo {
    #[serde(default)]
    item: String,
    #[serde(default)]
    severidad: Option<i64>,
    #[serde(default)]
    archivo: String,
    #[serde(default)]
    linea: u32,
    #[serde(default)]
    cita: String,
    #[serde(default)]
    detalle: String,
    #[serde(default)]
    impacto: Vec<String>,
}

/// Valida un ítem. El nivel se conserva solo si una evidencia pasa el gate.
fn procesar_item(valor: Value, cx: &Contexto) -> Result<Dicho, Descarte> {
    let cruda: ItemCrudo =
        serde_json::from_value(valor).map_err(|e| descarte("ítem", format!("mal formado: {e}")))?;
    let que = format!("ítem {}", cruda.item);
    let dimension = cx.dimension(&cruda.item).map_err(|m| descarte(&que, m))?;
    let Some(valor_nivel) = cruda.nivel.filter(|v| !v.is_null()) else {
        if cruda.motivo.as_deref() == Some(SIN_EVIDENCIA_DEL_JUEZ) {
            return Ok(Dicho::SinEvidencia(cruda.item));
        }
        return Err(descarte(&que, "sin nivel"));
    };
    let Some(nivel) = nivel_entero(&valor_nivel).filter(|n| (0..=NIVEL_MAX).contains(n)) else {
        return Err(descarte(
            &que,
            format!("nivel {valor_nivel} fuera de 0..=3"),
        ));
    };
    let mut validas = Vec::new();
    let mut fallidas = Vec::new();
    for ev in cruda
        .evidencia
        .into_iter()
        .filter_map(|v| serde_json::from_value::<EvidenciaCruda>(v).ok())
    {
        match evidencia_valida(ev, dimension, cx) {
            Ok(valida) => validas.push(valida),
            Err(fallida) => fallidas.push(fallida),
        }
    }
    if validas.is_empty() {
        return Ok(Dicho::SinGate {
            id: cruda.item,
            citas: fallidas,
        });
    }
    Ok(Dicho::Nivel(NivelItem {
        item: cruda.item,
        nivel: nivel as u8,
        evidencia: validas,
        fuente: cx.fuente(),
    }))
}

/// Devuelve `archivo:linea` si la evidencia pasa el gate de evidencia. Si no, la cita que falló.
fn evidencia_valida(
    ev: EvidenciaCruda,
    dimension: Dimension,
    cx: &Contexto,
) -> Result<String, CitaFallida> {
    let h = Hallazgo {
        dimension,
        severidad: 1,
        archivo: ev.archivo,
        linea: ev.linea,
        cita: ev.cita,
        detalle: String::new(),
        fuente: cx.fuente(),
        id: String::new(),
        impacto: Vec::new(),
    };
    if matches!(verificar_hallazgo(cx.repo, &h), Verificacion::Valido) {
        Ok(format!("{}:{}", h.archivo, h.linea))
    } else {
        Err(CitaFallida {
            archivo: h.archivo,
            linea: h.linea,
            cita: h.cita,
        })
    }
}

/// Anota un hallazgo rechazado por requisito o por cita, con su archivo, línea e ítem. Devuelve el descarte
/// que va a `descartes`.
fn descartar_hallazgo(
    archivo: &str,
    linea: u32,
    item: &str,
    motivo: String,
    rechazados: &mut Vec<HallazgoDescartado>,
) -> Descarte {
    rechazados.push(HallazgoDescartado {
        archivo: archivo.into(),
        linea,
        item: item.into(),
        motivo: motivo.clone(),
    });
    descarte(format!("hallazgo sobre {item}"), motivo)
}

/// Motivo de un hallazgo cuya línea no cae en ningún extracto leído de su archivo.
const MOTIVO_LINEA_FUERA: &str = "línea fuera de lo leído";

/// Vale si la línea cae en un extracto leído del mismo archivo.
/// Sin extractos no hay contra qué validar. Ese caso no descarta nada.
fn linea_leida(cx: &Contexto, archivo: &str, linea: u32) -> bool {
    cx.leidos.is_empty()
        || cx
            .leidos
            .iter()
            .any(|e| e.archivo == archivo && e.desde <= linea && linea <= e.hasta)
}

/// Valida un hallazgo: requisito real, severidad 1 a 3, línea leída, cita en el repo. Los rechazos por
/// requisito, línea o cita también van a `rechazados`.
fn procesar_hallazgo(
    valor: Value,
    cx: &Contexto,
    rechazados: &mut Vec<HallazgoDescartado>,
) -> Result<Hallazgo, Descarte> {
    let cruda: HallazgoCrudo = serde_json::from_value(valor)
        .map_err(|e| descarte("hallazgo", format!("mal formado: {e}")))?;
    let que = format!("hallazgo sobre {}", cruda.item);
    let dimension = cx.dimension(&cruda.item).map_err(|m| descarte(&que, m))?;
    let severidad = match cruda.severidad {
        Some(s) if (1..=SEVERIDAD_MAX).contains(&s) => s as u8,
        _ => return Err(descarte(&que, "severidad fuera de 1..=3")),
    };
    if cruda.archivo.trim().is_empty() || cruda.linea == 0 || cruda.cita.trim().is_empty() {
        return Err(descarte(&que, "falta archivo, línea o cita"));
    }
    if cruda.impacto.is_empty() {
        return Err(descartar_hallazgo(
            &cruda.archivo,
            cruda.linea,
            &cruda.item,
            "sin requisito afectado".into(),
            rechazados,
        ));
    }
    if let Some(id) = cruda
        .impacto
        .iter()
        .find(|id| !cx.requisitos.iter().any(|r| r.id == **id))
    {
        return Err(descartar_hallazgo(
            &cruda.archivo,
            cruda.linea,
            &cruda.item,
            format!("sin requisito afectado: {id} no existe"),
            rechazados,
        ));
    }
    if !linea_leida(cx, &cruda.archivo, cruda.linea) {
        return Err(descartar_hallazgo(
            &cruda.archivo,
            cruda.linea,
            &cruda.item,
            MOTIVO_LINEA_FUERA.into(),
            rechazados,
        ));
    }
    let mut h = crear_hallazgo(
        dimension,
        severidad,
        &cruda.archivo,
        cruda.linea,
        &cruda.cita,
        &cruda.detalle,
        "juez",
        cx.requisitos,
    );
    h.fuente = cx.fuente();
    h.impacto = cruda.impacto;
    match verificar_hallazgo(cx.repo, &h) {
        Verificacion::Valido => Ok(h),
        Verificacion::Descartado(motivo) => Err(descartar_hallazgo(
            &cruda.archivo,
            cruda.linea,
            &cruda.item,
            format!("la cita no pasa el gate: {motivo:?}"),
            rechazados,
        )),
    }
}

/// Tope de los ítems `mixto` según las sondas. Con un ciclo de módulos, `diseno.modularidad` no supera 1.
pub fn topes_desde_sondas(hallazgos: &[Hallazgo], rubricas: &[Rubrica]) -> BTreeMap<String, u8> {
    let hay_ciclo = hallazgos.iter().any(|h| {
        h.severidad == 3
            && h.detalle.contains(REGLA_CICLO)
            && matches!(&h.fuente, Fuente::Sonda(s) if s == SONDA_ARQUITECTURA)
    });
    let modularidad_mixta = rubricas
        .iter()
        .flat_map(|r| r.item.iter())
        .any(|i| i.id == MODULARIDAD && i.mide == Mide::Mixto);
    if hay_ciclo && modularidad_mixta {
        BTreeMap::from([(MODULARIDAD.to_string(), 1)])
    } else {
        BTreeMap::new()
    }
}

/// Puntúa una rúbrica con el resultado del juez. Con `SinVeredicto` el valor es `None`.
pub fn puntuar_con_juez(
    rubrica: &Rubrica,
    r: &ResultadoJuez,
    topes: &BTreeMap<String, u8>,
    nombre_modelo: &str,
) -> Result<Puntuacion, ErrorRubrica> {
    if r.estado != EstadoJuez::Ok {
        return Ok(Puntuacion {
            valor: None,
            items: Vec::new(),
            faltantes: rubrica.item.iter().map(|i| i.id.clone()).collect(),
        });
    }
    let niveles: BTreeMap<String, u8> = r
        .niveles
        .iter()
        .filter(|(id, _)| es_item_de(rubrica, id))
        .map(|(id, nivel)| (id.clone(), *nivel))
        .collect();
    let topes_propios: BTreeMap<String, u8> = topes
        .iter()
        .filter(|(id, _)| es_item_de(rubrica, id))
        .map(|(id, tope)| (id.clone(), *tope))
        .collect();
    let mut puntuacion = rubrica.puntuar(&niveles, &topes_propios)?;
    for item in &mut puntuacion.items {
        if let Some(origen) = r.items.iter().find(|n| n.item == item.item) {
            item.evidencia = origen.evidencia.clone();
            item.fuente = Fuente::Juez {
                modelo: nombre_modelo.into(),
            };
        }
    }
    Ok(puntuacion)
}

fn es_item_de(rubrica: &Rubrica, id: &str) -> bool {
    rubrica.item.iter().any(|i| i.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oracle::mapa::construir as construir_mapa;
    use crate::tipos::Rol;
    use serde_json::json;
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::fs;
    use tempfile::TempDir;

    const SPEC: &str = "El almacén guarda pedidos.\nR1: guardar un pedido sin perder datos.";
    const ALMACEN: &str =
        "pub struct Almacen;\n\nimpl Almacen {\n    pub fn guardar(&self) {}\n}\n";
    const CITA_REAL: &str = "pub fn guardar(&self) {}";
    const DETALLE: &str = "el almacén guarda sin validar";
    const MODELO: &str = "deepseek-reasoner";
    const TOPE: &str = "diseno.tope";
    const CICLOS: &str = "diseno.ciclos";

    fn fixture(nombre: &str) -> String {
        let ruta = format!(
            "{}/tests/fixtures/rubricas/{nombre}",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::read_to_string(ruta).expect("lee la fixture")
    }

    fn rubrica_valida() -> Rubrica {
        Rubrica::desde_toml(&fixture("diseno-valida.toml")).expect("carga")
    }

    /// La fixture con `diseno.modularidad` como `mixto`.
    fn rubrica_mixta() -> Rubrica {
        let texto = fixture("diseno-valida.toml").replace("mide = \"juez\"", "mide = \"mixto\"");
        Rubrica::desde_toml(&texto).expect("carga")
    }

    fn requisito(id: &str, peso: u8) -> Requisito {
        Requisito {
            id: id.into(),
            titulo: format!("título de {id}"),
            seccion: String::new(),
            peso,
            pruebas: vec![],
            bordes: vec![],
            escenarios: vec![],
            criterios: vec![],
            modulos: vec!["almacen".into()],
        }
    }

    fn eleccion(via: Via, razonamiento: bool, modelo: Option<&str>) -> Eleccion {
        Eleccion {
            via,
            proveedor: None,
            modelo: modelo.map(String::from),
            razonamiento,
            motivo: "prueba".into(),
            rol: Some(Rol::Juzga),
        }
    }

    struct Caso {
        repo: TempDir,
        mapa: Mapa,
        requisitos: Vec<Requisito>,
        rubricas: Vec<Rubrica>,
        sondas: Vec<Hallazgo>,
    }

    fn caso_con(rubricas: Vec<Rubrica>, sondas: Vec<Hallazgo>) -> Caso {
        let repo = tempfile::tempdir().expect("tempdir");
        fs::create_dir(repo.path().join("src")).expect("crea src");
        fs::write(repo.path().join("src/almacen.rs"), ALMACEN).expect("escribe");
        let mapa = construir_mapa(repo.path()).expect("mapa");
        Caso {
            repo,
            mapa,
            requisitos: vec![requisito("R1", 3)],
            rubricas,
            sondas,
        }
    }

    fn caso() -> Caso {
        caso_con(vec![rubrica_valida()], vec![])
    }

    impl Caso {
        fn entrada(&self) -> EntradaJuez<'_> {
            EntradaJuez {
                spec: SPEC,
                requisitos: &self.requisitos,
                mapa: &self.mapa,
                hallazgos_sondas: &self.sondas,
                rubricas: &self.rubricas,
                repo: self.repo.path(),
                inicial: None,
            }
        }
    }

    /// Modelo falso: devuelve una respuesta fija y guarda cada prompt recibido.
    struct Falso {
        respuesta: Result<RespuestaJuez, String>,
        visto: RefCell<Vec<Prompt>>,
    }

    impl Falso {
        fn con(texto: String) -> Self {
            Self {
                respuesta: Ok(RespuestaJuez {
                    texto,
                    truncada: false,
                    costo_usd: 0.0,
                }),
                visto: RefCell::new(Vec::new()),
            }
        }

        /// Respuesta truncada por el tope de salida, con el texto que se le pase.
        fn truncada_con(texto: String) -> Self {
            Self {
                respuesta: Ok(RespuestaJuez {
                    texto,
                    truncada: true,
                    costo_usd: 0.0,
                }),
                visto: RefCell::new(Vec::new()),
            }
        }

        fn caido() -> Self {
            Self {
                respuesta: Err("tiempo agotado".into()),
                visto: RefCell::new(Vec::new()),
            }
        }
    }

    impl ModeloJuez for Falso {
        fn consultar(&self, prompt: &Prompt) -> Result<RespuestaJuez, ErrorOraculo> {
            self.visto.borrow_mut().push(prompt.clone());
            self.respuesta.clone().map_err(ErrorOraculo::Infra)
        }
    }

    fn juzgar_con(caso: &Caso, texto: String) -> ResultadoJuez {
        juzgar(
            &caso.entrada(),
            &Falso::con(texto),
            MODELO,
            &LimitesPrompt::default(),
        )
    }

    /// Lectura de `items` con el caso `c`, con los mapas de pedidos y catálogo que arma `juzgar`.
    fn leer(c: &Caso, items: Value) -> Lectura {
        let Value::Array(lista) = items else {
            panic!("se esperaba un arreglo");
        };
        let (pedidos, catalogo) = mapas_de(&c.rubricas);
        let cx = Contexto {
            repo: c.repo.path(),
            requisitos: &c.requisitos,
            pedidos: &pedidos,
            catalogo: &catalogo,
            modelo: MODELO,
            leidos: &[],
        };
        leer_items(lista, &cx)
    }

    fn sin_evidencia_json(id: &str) -> Value {
        json!({"item": id, "nivel": null, "motivo": "sin_evidencia", "evidencia": []})
    }

    fn respuesta(items: Value, hallazgos: Value) -> String {
        json!({"items": items, "hallazgos": hallazgos}).to_string()
    }

    fn item_nivel(id: &str, nivel: i64, evidencia: Value) -> Value {
        json!({"item": id, "nivel": nivel, "evidencia": evidencia})
    }

    fn evidencia(archivo: &str, linea: u32, cita: &str) -> Value {
        json!({"archivo": archivo, "linea": linea, "cita": cita})
    }

    /// Evidencia real con otra indentación y espaciado: el gate la acepta.
    fn real() -> Value {
        evidencia("src/almacen.rs", 4, "pub fn guardar(&self){}")
    }

    fn hallazgo_json(impacto: Value, severidad: i64, cita: &str) -> Value {
        json!({
            "item": MODULARIDAD,
            "severidad": severidad,
            "archivo": "src/almacen.rs",
            "linea": 4,
            "cita": cita,
            "detalle": DETALLE,
            "impacto": impacto,
        })
    }

    fn assert_sin_veredicto(r: &ResultadoJuez) {
        assert!(matches!(r.estado, EstadoJuez::SinVeredicto(_)), "{r:?}");
        assert!(r.niveles.is_empty());
        assert!(r.hallazgos.is_empty());
    }

    fn ciclo() -> Hallazgo {
        Hallazgo {
            dimension: Dimension::Diseno,
            severidad: 3,
            archivo: "src/almacen.rs".into(),
            linea: 4,
            cita: CITA_REAL.into(),
            detalle: "ciclo de módulos: almacen -> almacen".into(),
            fuente: Fuente::Sonda(SONDA_ARQUITECTURA.into()),
            id: String::new(),
            impacto: vec!["R1".into()],
        }
    }

    // Respuesta truncada por el tope de salida: se mira antes de parsear.

    #[test]
    fn juez_truncado_da_sin_veredicto_con_la_causa_del_tope() {
        let c = caso();
        let texto = respuesta(json!([]), json!([]));
        let r = juzgar(
            &c.entrada(),
            &Falso::truncada_con(texto),
            MODELO,
            &LimitesPrompt::default(),
        );
        assert_eq!(
            r.estado,
            EstadoJuez::SinVeredicto("tope de salida alcanzado (32000 tokens)".into())
        );
        assert!(r.niveles.is_empty());
        assert!(r.items.is_empty());
        assert!(r.hallazgos.is_empty());
    }

    #[test]
    fn juez_truncado_con_texto_vacio_no_dice_json_invalido() {
        let c = caso();
        let r = juzgar(
            &c.entrada(),
            &Falso::truncada_con(String::new()),
            MODELO,
            &LimitesPrompt::default(),
        );
        match &r.estado {
            EstadoJuez::SinVeredicto(causa) => {
                assert!(causa.starts_with("tope de salida alcanzado"), "{causa}");
                assert!(!causa.contains("JSON"), "{causa}");
            }
            otro => panic!("esperaba SinVeredicto, llegó {otro:?}"),
        }
    }

    #[test]
    fn juez_no_truncado_json_roto_sigue_con_su_causa() {
        let r = juzgar_con(&caso(), "{\"items\": [".into());
        match &r.estado {
            EstadoJuez::SinVeredicto(causa) => {
                assert!(causa.starts_with("JSON inválido"), "{causa}");
            }
            otro => panic!("esperaba SinVeredicto, llegó {otro:?}"),
        }
    }

    // Respuesta que no es JSON válido o no tiene la forma pedida.

    #[test]
    fn juez_json_roto_da_sin_veredicto() {
        let r = juzgar_con(&caso(), "{\"items\": [".into());
        assert_sin_veredicto(&r);
    }

    #[test]
    fn juez_arreglo_en_vez_de_objeto_da_sin_veredicto() {
        let r = juzgar_con(&caso(), "[]".into());
        assert_sin_veredicto(&r);
    }

    #[test]
    fn juez_falta_clave_hallazgos_da_sin_veredicto() {
        let r = juzgar_con(&caso(), json!({"items": []}).to_string());
        assert_sin_veredicto(&r);
    }

    #[test]
    fn juez_error_del_modelo_da_sin_veredicto_y_puntaje_none() {
        let c = caso();
        let r = juzgar(
            &c.entrada(),
            &Falso::caido(),
            MODELO,
            &LimitesPrompt::default(),
        );
        assert_sin_veredicto(&r);
        let p = puntuar_con_juez(&c.rubricas[0], &r, &BTreeMap::new(), MODELO).expect("puntúa");
        assert_eq!(p.valor, None);
    }

    #[test]
    fn juez_bloque_json_se_acepta() {
        let cuerpo = respuesta(
            json!([item_nivel(MODULARIDAD, 1, json!([real()]))]),
            json!([]),
        );
        let r = juzgar_con(&caso(), format!("```json\n{cuerpo}\n```"));
        assert_eq!(r.estado, EstadoJuez::Ok);
        assert_eq!(r.niveles.get(MODULARIDAD), Some(&1));
    }

    #[test]
    fn juez_texto_antes_o_despues_da_sin_veredicto() {
        let cuerpo = respuesta(json!([]), json!([]));
        assert_sin_veredicto(&juzgar_con(&caso(), format!("Aquí va: {cuerpo}")));
        assert_sin_veredicto(&juzgar_con(&caso(), format!("{cuerpo}\nEso es todo.")));
    }

    #[test]
    fn juez_item_mal_formado_no_tumba_el_resto() {
        let c = caso();
        let texto = respuesta(
            json!([
                {"item": MODULARIDAD, "nivel": "alto"},
                item_nivel(TOPE, 1, json!([real()])),
            ]),
            json!([]),
        );
        let r = juzgar_con(&c, texto);
        assert_eq!(r.estado, EstadoJuez::Ok);
        assert_eq!(r.niveles.get(TOPE), Some(&1));
        assert!(!r.niveles.contains_key(MODULARIDAD));
        assert!(!r.descartes.is_empty());
    }

    // Ítems: evidencia, gate, sondas y faltantes.

    #[test]
    fn juez_respuesta_valida_conserva_nivel_con_evidencia_real() {
        let c = caso();
        let texto = respuesta(
            json!([item_nivel(MODULARIDAD, 2, json!([real()]))]),
            json!([]),
        );
        let r = juzgar_con(&c, texto);
        assert_eq!(r.estado, EstadoJuez::Ok);
        assert_eq!(r.niveles.get(MODULARIDAD), Some(&2));
        let item = r
            .items
            .iter()
            .find(|i| i.item == MODULARIDAD)
            .expect("está");
        assert_eq!(item.evidencia, vec!["src/almacen.rs:4".to_string()]);
        assert_eq!(
            item.fuente,
            Fuente::Juez {
                modelo: MODELO.into()
            }
        );
        assert_eq!(r.faltantes, vec![TOPE.to_string()]);
    }

    #[test]
    fn juez_evidencia_inventada_manda_item_a_faltantes() {
        let c = caso();
        let inventada = evidencia("src/almacen.rs", 4, "pub fn borrar(&self) {}");
        let texto = respuesta(
            json!([item_nivel(MODULARIDAD, 3, json!([inventada]))]),
            json!([]),
        );
        let r = juzgar_con(&c, texto);
        assert_eq!(r.estado, EstadoJuez::Ok);
        assert!(!r.niveles.contains_key(MODULARIDAD));
        assert!(r.faltantes.contains(&MODULARIDAD.to_string()));
        assert!(r.descartes.iter().any(|d| d.motivo.contains("gate")));
    }

    #[test]
    fn juez_evidencia_mixta_conserva_solo_la_real() {
        let c = caso();
        let inventada = evidencia("src/almacen.rs", 4, "pub fn borrar(&self) {}");
        let texto = respuesta(
            json!([item_nivel(MODULARIDAD, 3, json!([inventada, real()]))]),
            json!([]),
        );
        let r = juzgar_con(&c, texto);
        assert_eq!(r.niveles.get(MODULARIDAD), Some(&3));
        let item = r
            .items
            .iter()
            .find(|i| i.item == MODULARIDAD)
            .expect("está");
        assert_eq!(item.evidencia, vec!["src/almacen.rs:4".to_string()]);
    }

    #[test]
    fn juez_item_de_sonda_respondido_se_descarta() {
        let c = caso();
        let texto = respuesta(json!([item_nivel(CICLOS, 2, json!([real()]))]), json!([]));
        let r = juzgar_con(&c, texto);
        assert!(!r.niveles.contains_key(CICLOS));
        assert!(r.descartes.iter().any(|d| d.motivo.contains("sonda")));
    }

    #[test]
    fn juez_item_inexistente_se_descarta() {
        let c = caso();
        let texto = respuesta(
            json!([item_nivel("diseno.no_existe", 2, json!([real()]))]),
            json!([]),
        );
        let r = juzgar_con(&c, texto);
        assert!(!r.niveles.contains_key("diseno.no_existe"));
        assert!(r.descartes.iter().any(|d| d.motivo.contains("no existe")));
    }

    #[test]
    fn juez_nivel_cinco_se_descarta() {
        let c = caso();
        let texto = respuesta(
            json!([item_nivel(MODULARIDAD, 5, json!([real()]))]),
            json!([]),
        );
        let r = juzgar_con(&c, texto);
        assert!(!r.niveles.contains_key(MODULARIDAD));
        assert!(
            r.descartes
                .iter()
                .any(|d| d.motivo.contains("fuera de 0..=3"))
        );
        assert!(r.faltantes.contains(&MODULARIDAD.to_string()));
    }

    #[test]
    fn juez_item_pedido_no_respondido_va_a_faltantes() {
        let c = caso();
        let r = juzgar_con(&c, respuesta(json!([]), json!([])));
        assert_eq!(r.estado, EstadoJuez::Ok);
        assert_eq!(r.faltantes, vec![MODULARIDAD.to_string(), TOPE.to_string()]);
    }

    #[test]
    fn juez_sin_evidencia_va_a_faltantes_con_su_descarte() {
        let c = caso();
        let texto = respuesta(
            json!([
                item_nivel(MODULARIDAD, 2, json!([real()])),
                sin_evidencia_json(TOPE),
            ]),
            json!([]),
        );
        let r = juzgar_con(&c, texto);
        assert_eq!(r.estado, EstadoJuez::Ok);
        assert!(!r.niveles.contains_key(TOPE));
        assert_eq!(r.faltantes, vec![TOPE.to_string()]);
        assert!(
            r.descartes
                .iter()
                .any(|d| d.que == "ítem diseno.tope" && d.motivo.contains("sin_evidencia")),
            "{:?}",
            r.descartes
        );
    }

    #[test]
    fn juez_id_repetido_no_cuenta_ninguna_copia() {
        let c = caso();
        let items = json!([
            item_nivel(MODULARIDAD, 2, json!([real()])),
            item_nivel(MODULARIDAD, 3, json!([real()])),
            item_nivel(TOPE, 1, json!([real()])),
        ]);
        // El reintento de MODULARIDAD no responde: así los descartes "repetido" son solo los del primer envío.
        let modelo = FalsoSecuencia::nueva(vec![
            Ok(respuesta(items.clone(), json!([]))),
            Ok(respuesta(json!([]), json!([]))),
        ]);
        let r = juzgar_secuencia(&c, &modelo);
        assert!(!r.niveles.contains_key(MODULARIDAD), "{:?}", r.niveles);
        assert!(r.faltantes.contains(&MODULARIDAD.to_string()));
        let repetidos = r
            .descartes
            .iter()
            .filter(|d| d.que == "ítem diseno.modularidad" && d.motivo.contains("repetido"))
            .count();
        assert_eq!(repetidos, 2, "{:?}", r.descartes);
        assert!(leer(&c, items).rotos.contains(MODULARIDAD));
    }

    #[test]
    fn juez_id_omitido_queda_en_rotos() {
        let c = caso();
        let l = leer(&c, json!([item_nivel(MODULARIDAD, 2, json!([real()]))]));
        assert_eq!(l.rotos, BTreeSet::from([TOPE.to_string()]));
    }

    #[test]
    fn juez_item_sin_nivel_ni_motivo_queda_en_rotos() {
        let c = caso();
        let l = leer(
            &c,
            json!([{"item": MODULARIDAD}, item_nivel(TOPE, 1, json!([real()]))]),
        );
        assert_eq!(l.rotos, BTreeSet::from([MODULARIDAD.to_string()]));
        assert!(!l.niveles.contains_key(MODULARIDAD));
    }

    #[test]
    fn juez_sin_evidencia_no_esta_en_rotos() {
        let c = caso();
        let l = leer(
            &c,
            json!([
                sin_evidencia_json(MODULARIDAD),
                item_nivel(TOPE, 1, json!([real()])),
            ]),
        );
        assert!(l.rotos.is_empty(), "{:?}", l.rotos);
    }

    #[test]
    fn juez_id_no_pedido_se_ignora_con_aviso() {
        let c = caso();
        let items = json!([
            item_nivel("diseno.inventado", 2, json!([real()])),
            item_nivel(CICLOS, 2, json!([real()])),
            item_nivel(MODULARIDAD, 2, json!([real()])),
            item_nivel(TOPE, 1, json!([real()])),
        ]);
        let r = juzgar_con(&c, respuesta(items.clone(), json!([])));
        for id in ["diseno.inventado", CICLOS] {
            assert!(!r.niveles.contains_key(id), "{id}");
            assert!(!r.faltantes.contains(&id.to_string()), "{id}");
            assert!(
                r.descartes.iter().any(|d| d.que == format!("ítem {id}")
                    && d.motivo.contains("no se pidió")
                    && d.motivo.contains("se ignora")),
                "{id}: {:?}",
                r.descartes
            );
        }
        let l = leer(&c, items);
        assert!(l.rotos.is_empty(), "{:?}", l.rotos);
    }

    // Hallazgos: impacto, cita, severidad y gate.

    #[test]
    fn juez_hallazgo_sin_impacto_se_descarta() {
        let c = caso();
        let texto = respuesta(json!([]), json!([hallazgo_json(json!([]), 2, CITA_REAL)]));
        let r = juzgar_con(&c, texto);
        assert!(r.hallazgos.is_empty());
        assert!(
            r.descartes
                .iter()
                .any(|d| d.motivo.contains("sin requisito"))
        );
    }

    #[test]
    fn juez_hallazgo_con_requisito_inexistente_se_descarta() {
        let c = caso();
        let texto = respuesta(
            json!([]),
            json!([hallazgo_json(json!(["R9"]), 2, CITA_REAL)]),
        );
        let r = juzgar_con(&c, texto);
        assert!(r.hallazgos.is_empty());
        assert!(r.descartes.iter().any(|d| d.motivo.contains("R9")));
    }

    #[test]
    fn juez_hallazgo_con_requisito_valido_se_conserva() {
        let c = caso();
        let texto = respuesta(
            json!([]),
            json!([hallazgo_json(json!(["R1"]), 2, CITA_REAL)]),
        );
        let r = juzgar_con(&c, texto);
        assert_eq!(r.hallazgos.len(), 1);
        let h = &r.hallazgos[0];
        assert_eq!(
            h.fuente,
            Fuente::Juez {
                modelo: MODELO.into()
            }
        );
        assert_eq!(h.dimension, Dimension::Diseno);
        assert_eq!(h.impacto, vec!["R1".to_string()]);
        let esperado = crear_hallazgo(
            Dimension::Diseno,
            2,
            "src/almacen.rs",
            4,
            CITA_REAL,
            DETALLE,
            "juez",
            &c.requisitos,
        );
        assert_eq!(h.id, esperado.id);
        assert!(!h.id.is_empty());
    }

    #[test]
    fn juez_hallazgo_con_cita_inventada_se_descarta() {
        let c = caso();
        let texto = respuesta(
            json!([]),
            json!([hallazgo_json(json!(["R1"]), 2, "pub fn borrar(&self) {}")]),
        );
        let r = juzgar_con(&c, texto);
        assert!(r.hallazgos.is_empty());
        assert!(r.descartes.iter().any(|d| d.motivo.contains("gate")));
    }

    #[test]
    fn juez_hallazgo_severidad_cuatro_se_descarta() {
        let c = caso();
        let texto = respuesta(
            json!([]),
            json!([hallazgo_json(json!(["R1"]), 4, CITA_REAL)]),
        );
        let r = juzgar_con(&c, texto);
        assert!(r.hallazgos.is_empty());
        assert!(r.descartes.iter().any(|d| d.motivo.contains("severidad")));
    }

    // Hallazgos descartados: el juez conserva archivo, línea, ítem y motivo de los que rechaza por requisito o cita.

    #[test]
    fn juez_hallazgo_sin_impacto_queda_en_descartados() {
        let c = caso();
        let texto = respuesta(json!([]), json!([hallazgo_json(json!([]), 2, CITA_REAL)]));
        let r = juzgar_con(&c, texto);
        assert!(r.hallazgos.is_empty());
        assert_eq!(
            r.hallazgos_descartados.len(),
            1,
            "{:?}",
            r.hallazgos_descartados
        );
        let d = &r.hallazgos_descartados[0];
        assert_eq!(d.archivo, "src/almacen.rs");
        assert_eq!(d.linea, 4);
        assert_eq!(d.item, MODULARIDAD);
        assert!(d.motivo.contains("sin requisito afectado"), "{}", d.motivo);
    }

    #[test]
    fn juez_hallazgo_con_requisito_inexistente_queda_en_descartados() {
        let c = caso();
        let texto = respuesta(
            json!([]),
            json!([hallazgo_json(json!(["R9"]), 2, CITA_REAL)]),
        );
        let r = juzgar_con(&c, texto);
        assert!(r.hallazgos.is_empty());
        assert_eq!(
            r.hallazgos_descartados.len(),
            1,
            "{:?}",
            r.hallazgos_descartados
        );
        let d = &r.hallazgos_descartados[0];
        assert_eq!((d.archivo.as_str(), d.linea), ("src/almacen.rs", 4));
        assert_eq!(d.item, MODULARIDAD);
        assert!(d.motivo.contains("sin requisito afectado"), "{}", d.motivo);
    }

    #[test]
    fn juez_hallazgo_con_cita_inventada_queda_en_descartados() {
        let c = caso();
        let texto = respuesta(
            json!([]),
            json!([hallazgo_json(json!(["R1"]), 2, "pub fn borrar(&self) {}")]),
        );
        let r = juzgar_con(&c, texto);
        assert!(r.hallazgos.is_empty());
        assert_eq!(
            r.hallazgos_descartados.len(),
            1,
            "{:?}",
            r.hallazgos_descartados
        );
        let d = &r.hallazgos_descartados[0];
        assert_eq!((d.archivo.as_str(), d.linea), ("src/almacen.rs", 4));
        assert_eq!(d.item, MODULARIDAD);
        assert!(d.motivo.contains("gate"), "{}", d.motivo);
    }

    #[test]
    fn juez_hallazgo_valido_no_entra_a_descartados() {
        let c = caso();
        let texto = respuesta(
            json!([]),
            json!([hallazgo_json(json!(["R1"]), 2, CITA_REAL)]),
        );
        let r = juzgar_con(&c, texto);
        assert_eq!(r.hallazgos.len(), 1);
        assert!(
            r.hallazgos_descartados.is_empty(),
            "{:?}",
            r.hallazgos_descartados
        );
    }

    #[test]
    fn juez_hallazgo_severidad_invalida_no_entra_a_descartados() {
        let c = caso();
        let texto = respuesta(
            json!([]),
            json!([hallazgo_json(json!(["R1"]), 4, CITA_REAL)]),
        );
        let r = juzgar_con(&c, texto);
        assert!(r.hallazgos.is_empty());
        assert!(r.descartes.iter().any(|d| d.motivo.contains("severidad")));
        assert!(
            r.hallazgos_descartados.is_empty(),
            "{:?}",
            r.hallazgos_descartados
        );
    }

    // Validación de línea: el hallazgo debe caer dentro de un extracto leído del mismo archivo.

    /// Caso con `src/almacen.rs` de 150 líneas. La línea 100 tiene la cita real.
    fn caso_de_150_lineas() -> Caso {
        let repo = tempfile::tempdir().expect("tempdir");
        fs::create_dir(repo.path().join("src")).expect("crea src");
        let texto: String = (1..=150)
            .map(|n| {
                if n == 100 {
                    format!("{CITA_REAL}\n")
                } else {
                    format!("fn f{n}() {{}}\n")
                }
            })
            .collect();
        fs::write(repo.path().join("src/almacen.rs"), texto).expect("escribe");
        let mapa = construir_mapa(repo.path()).expect("mapa");
        Caso {
            repo,
            mapa,
            requisitos: vec![requisito("R1", 3)],
            rubricas: vec![rubrica_valida()],
            sondas: vec![],
        }
    }

    /// Hallazgo del juez sobre `archivo:linea`, con requisito R1 y severidad 2.
    fn hallazgo_en(archivo: &str, linea: u32, cita: &str) -> Value {
        json!({
            "item": MODULARIDAD,
            "severidad": 2,
            "archivo": archivo,
            "linea": linea,
            "cita": cita,
            "detalle": DETALLE,
            "impacto": ["R1"],
        })
    }

    #[test]
    fn juez_hallazgo_fuera_de_lo_leido_queda_en_descartados() {
        let c = caso_de_150_lineas();
        let texto = respuesta(
            json!([]),
            json!([hallazgo_en("src/almacen.rs", 500, CITA_REAL)]),
        );
        let r = juzgar_con(&c, texto);
        // Premisa: el extracto cubre 1..=150, así que la línea 500 queda fuera.
        assert_eq!(r.extractos.len(), 1, "{:?}", r.extractos);
        assert_eq!((r.extractos[0].desde, r.extractos[0].hasta), (1, 150));
        assert!(r.hallazgos.is_empty(), "{:?}", r.hallazgos);
        assert_eq!(
            r.hallazgos_descartados.len(),
            1,
            "{:?}",
            r.hallazgos_descartados
        );
        let d = &r.hallazgos_descartados[0];
        assert_eq!((d.archivo.as_str(), d.linea), ("src/almacen.rs", 500));
        assert_eq!(d.item, MODULARIDAD);
        assert_eq!(d.motivo, "línea fuera de lo leído");
    }

    #[test]
    fn juez_hallazgo_dentro_de_lo_leido_sigue_valido() {
        let c = caso_de_150_lineas();
        let texto = respuesta(
            json!([]),
            json!([hallazgo_en("src/almacen.rs", 100, CITA_REAL)]),
        );
        let r = juzgar_con(&c, texto);
        assert_eq!(r.hallazgos.len(), 1, "{:?}", r.descartes);
        assert!(
            r.hallazgos_descartados.is_empty(),
            "{:?}",
            r.hallazgos_descartados
        );
    }

    #[test]
    fn juez_hallazgo_de_archivo_sin_extracto_queda_en_descartados() {
        let c = caso_de_150_lineas();
        let texto = respuesta(
            json!([]),
            json!([hallazgo_en("src/otro.rs", 100, CITA_REAL)]),
        );
        let r = juzgar_con(&c, texto);
        assert!(r.hallazgos.is_empty());
        assert_eq!(
            r.hallazgos_descartados.len(),
            1,
            "{:?}",
            r.hallazgos_descartados
        );
        assert_eq!(r.hallazgos_descartados[0].motivo, "línea fuera de lo leído");
    }

    #[test]
    fn juez_sin_extractos_no_descarta_por_linea() {
        // Repo sin archivos legibles: no hay extractos. La línea no se valida; cae en el gate.
        let repo = tempfile::tempdir().expect("tempdir");
        let mapa = construir_mapa(repo.path()).expect("mapa");
        let c = Caso {
            repo,
            mapa,
            requisitos: vec![requisito("R1", 3)],
            rubricas: vec![rubrica_valida()],
            sondas: vec![],
        };
        let texto = respuesta(
            json!([]),
            json!([hallazgo_en("src/almacen.rs", 500, CITA_REAL)]),
        );
        let r = juzgar_con(&c, texto);
        assert!(r.extractos.is_empty());
        assert_eq!(r.hallazgos_descartados.len(), 1, "{:?}", r.descartes);
        assert!(
            !r.hallazgos_descartados[0]
                .motivo
                .contains("línea fuera de lo leído"),
            "{:?}",
            r.hallazgos_descartados
        );
    }

    // Proveedor: el juez razona y usa la API.

    /// Una elección inválida es un error de configuración: `Eleccion`, con código de salida 2.
    fn assert_eleccion(error: ErrorOraculo, motivo: &str) {
        assert!(matches!(error, ErrorOraculo::Eleccion(_)), "{error}");
        assert!(error.to_string().contains(motivo), "{error}");
        assert_eq!(crate::error::Error::from(error).codigo_de_salida(), 2);
    }

    #[test]
    fn juez_proveedor_rechaza_razonamiento_apagado() {
        let error = JuezProveedor::nuevo(eleccion(Via::Api, false, Some(MODELO)), "c".into())
            .err()
            .expect("rechaza");
        assert_eleccion(error, "razonamiento");
    }

    #[test]
    fn juez_proveedor_rechaza_via_cupo() {
        let error = JuezProveedor::nuevo(eleccion(Via::Cupo, true, Some(MODELO)), "c".into())
            .err()
            .expect("rechaza");
        assert_eleccion(error, "api");
    }

    #[test]
    fn juez_proveedor_rechaza_via_oraculo() {
        let error = JuezProveedor::nuevo(eleccion(Via::Oraculo, true, Some(MODELO)), "c".into())
            .err()
            .expect("rechaza");
        assert_eleccion(error, "api");
    }

    #[test]
    fn juez_proveedor_rechaza_sin_modelo() {
        let error = JuezProveedor::nuevo(eleccion(Via::Api, true, None), "c".into())
            .err()
            .expect("rechaza");
        assert_eleccion(error, "modelo");
    }

    #[test]
    fn juez_proveedor_con_razonamiento_y_api_se_acepta() {
        let ok = JuezProveedor::nuevo(eleccion(Via::Api, true, Some(MODELO)), "c".into());
        assert!(ok.is_ok());
    }

    // Prompt.

    #[test]
    fn juez_prompt_de_mas_de_256_kib_no_se_envia_y_da_sin_veredicto() {
        let c = caso();
        let spec = "x".repeat(300 * 1024);
        let modelo = Falso::con(respuesta(json!([]), json!([])));
        let entrada = EntradaJuez {
            spec: &spec,
            ..c.entrada()
        };
        let r = juzgar(&entrada, &modelo, MODELO, &LimitesPrompt::default());
        assert_sin_veredicto(&r);
        assert!(modelo.visto.borrow().is_empty(), "el prompt no se envía");
        let EstadoJuez::SinVeredicto(causa) = &r.estado else {
            panic!("{:?}", r.estado);
        };
        assert!(causa.contains("262144"), "{causa}");
    }

    #[test]
    fn juez_prompt_con_spec_chica_se_envia() {
        let c = caso();
        let modelo = Falso::con(respuesta(json!([]), json!([])));
        let _ = juzgar(&c.entrada(), &modelo, MODELO, &LimitesPrompt::default());
        assert!(
            !modelo.visto.borrow().is_empty(),
            "un prompt chico sí se envía"
        );
    }

    #[test]
    fn juez_prompt_tiene_reglas_spec_requisitos_mapa_y_anclas() {
        let c = caso();
        let p = construir_prompt(&c.entrada(), &LimitesPrompt::default());
        let sistema = p.sistema.expect("tiene sistema");
        assert!(sistema.contains("REFUTA"));
        assert!(sistema.contains("¿Qué falta para operar"));
        assert!(p.usuario.contains("El almacén guarda pedidos."));
        assert!(p.usuario.contains("R1"));
        assert!(p.usuario.contains("Módulos:"));
        assert!(
            p.usuario
                .contains("0: un módulo concentra varios requisitos")
        );
        assert!(p.usuario.contains("3: módulos con una responsabilidad"));
        assert_eq!(p.fase, FASE);
        assert_eq!(p.max_tokens, 32_000);
    }

    #[test]
    fn juez_limites_max_tokens_llega_al_prompt() {
        let c = caso();
        let limites = LimitesPrompt {
            max_tokens: 8000,
            ..LimitesPrompt::default()
        };
        assert_eq!(construir_prompt(&c.entrada(), &limites).max_tokens, 8000);
    }

    #[test]
    fn juez_prompt_omite_items_de_sonda() {
        let c = caso();
        let p = construir_prompt(&c.entrada(), &LimitesPrompt::default());
        assert!(p.usuario.contains(MODULARIDAD));
        assert!(!p.usuario.contains(CICLOS));
    }

    #[test]
    fn juez_prompt_nombra_ids_obligatorios_en_orden() {
        let c = caso();
        let p = construir_prompt(&c.entrada(), &LimitesPrompt::default());
        let esperado = format!("IDS OBLIGATORIOS:\n- {MODULARIDAD}\n- {TOPE}\n");
        let inicio = p
            .usuario
            .find(&esperado)
            .expect("bloque de ids obligatorios");
        let fin = p.usuario.find("ÍTEMS A PUNTUAR:").expect("bloque de ítems");
        assert!(inicio < fin, "los ids van antes de ÍTEMS A PUNTUAR");
    }

    #[test]
    fn juez_prompt_sistema_exige_cada_id_y_acepta_sin_evidencia() {
        let c = caso();
        let p = construir_prompt(&c.entrada(), &LimitesPrompt::default());
        let sistema = p.sistema.expect("tiene sistema");
        assert!(sistema.contains("sin_evidencia"), "{sistema}");
        assert!(!sistema.contains("no lo incluyas en items"), "{sistema}");
    }

    #[test]
    fn juez_prompt_no_veta_nombres_si_el_ancla_los_pide() {
        let c = caso();
        let p = construir_prompt(&c.entrada(), &LimitesPrompt::default());
        let sistema = p.sistema.expect("tiene sistema");
        assert!(
            sistema.contains("salvo lo que pide el ancla de un ítem"),
            "{sistema}"
        );
        assert!(
            !sistema.contains("No revisás estilo, nombres ni formato. Solo revisás"),
            "{sistema}"
        );
    }

    #[test]
    fn juez_prompt_extractos_numerados_y_con_limite() {
        let repo = tempfile::tempdir().expect("tempdir");
        fs::create_dir(repo.path().join("src")).expect("crea src");
        let grande: String = (1..=200).map(|n| format!("fn f{n}() {{}}\n")).collect();
        fs::write(repo.path().join("src/almacen.rs"), grande).expect("escribe");
        let mapa = construir_mapa(repo.path()).expect("mapa");
        let c = caso();
        let entrada = EntradaJuez {
            repo: repo.path(),
            mapa: &mapa,
            ..c.entrada()
        };
        let limites = LimitesPrompt {
            extracto_max: 120,
            ..LimitesPrompt::default()
        };
        let p = construir_prompt(&entrada, &limites);
        let extractos = p
            .usuario
            .split("EXTRACTOS:\n")
            .nth(1)
            .expect("hay extractos");
        assert!(extractos.contains("--- src/almacen.rs ---\n1: fn f1() {}\n"));
        assert!(extractos.contains(MARCA_RECORTE));
        assert!(!extractos.contains("150: "));
        let cuerpo = extractos
            .split("--- src/almacen.rs ---\n")
            .nth(1)
            .expect("cuerpo");
        assert!(cuerpo.chars().count() <= 120, "{}", cuerpo.chars().count());
    }

    #[test]
    fn juez_prompt_extractos_respetan_el_total() {
        let repo = tempfile::tempdir().expect("tempdir");
        fs::create_dir(repo.path().join("src")).expect("crea src");
        let grande: String = (1..=100).map(|n| format!("fn f{n}() {{}}\n")).collect();
        fs::write(repo.path().join("src/a.rs"), &grande).expect("escribe");
        fs::write(repo.path().join("src/b.rs"), &grande[..200]).expect("escribe");
        let mapa = construir_mapa(repo.path()).expect("mapa");
        let c = caso();
        let entrada = EntradaJuez {
            repo: repo.path(),
            mapa: &mapa,
            ..c.entrada()
        };
        let limites = LimitesPrompt {
            extractos_total: 100,
            extracto_max: 1000,
            ..LimitesPrompt::default()
        };
        let p = construir_prompt(&entrada, &limites);
        let extractos = p
            .usuario
            .split("EXTRACTOS:\n")
            .nth(1)
            .expect("hay extractos");
        assert!(extractos.contains("--- src/a.rs ---"));
        assert!(!extractos.contains("--- src/b.rs ---"), "{extractos}");
    }

    // Lectura guiada por cambios: extractos desde `bootstrap/`.

    /// Texto de `n` líneas `fn fK() {}`. Las de `cambiadas` dicen `fn cambio_K() {}`.
    fn texto_de(n: usize, cambiadas: &[usize]) -> String {
        (1..=n)
            .map(|k| {
                if cambiadas.contains(&k) {
                    format!("fn cambio_{k}() {{}}\n")
                } else {
                    format!("fn f{k}() {{}}\n")
                }
            })
            .collect()
    }

    /// Par `(repo, inicial)`. Cada `(ruta, n, cambiadas)` escribe `n` líneas en el repo. La copia inicial
    /// tiene las mismas líneas, sin los cambios.
    fn par_de_carpetas(archivos: &[(&str, usize, &[usize])]) -> (TempDir, TempDir) {
        let repo = tempfile::tempdir().expect("tempdir");
        let inicial = tempfile::tempdir().expect("tempdir");
        for (ruta, n, cambiadas) in archivos {
            for (base, texto) in [
                (repo.path(), texto_de(*n, cambiadas)),
                (inicial.path(), texto_de(*n, &[])),
            ] {
                let destino = base.join(ruta);
                fs::create_dir_all(destino.parent().expect("tiene carpeta")).expect("crea");
                fs::write(destino, texto).expect("escribe");
            }
        }
        (repo, inicial)
    }

    /// Extractos del repo con `inicial` como copia de partida y límites de producción.
    fn extractos_con_inicial(repo: &Path, inicial: &Path) -> Vec<Extracto> {
        let mapa = construir_mapa(repo).expect("mapa");
        let c = caso();
        let entrada = EntradaJuez {
            repo,
            mapa: &mapa,
            inicial: Some(inicial),
            ..c.entrada()
        };
        extractos(&entrada, &LimitesPrompt::default())
    }

    /// Sección de extractos del prompt, desde `EXTRACTOS:`.
    fn seccion_extractos_de(usuario: &str) -> &str {
        usuario.split("EXTRACTOS:\n").nth(1).expect("hay extractos")
    }

    /// Caracteres de la sección que cuentan para `extractos_total`: sin los encabezados `--- archivo ---`.
    fn chars_sin_encabezados(seccion: &str) -> usize {
        seccion
            .lines()
            .filter(|l| !l.starts_with("--- "))
            .map(|l| l.chars().count() + 1)
            .sum()
    }

    #[test]
    fn juez_resultado_guarda_los_extractos_del_prompt_sin_texto() {
        let (repo, ini) =
            par_de_carpetas(&[("src/cambia.rs", 2000, &[900]), ("src/grande.rs", 300, &[])]);
        let mapa = construir_mapa(repo.path()).expect("mapa");
        let c = caso();
        let entrada = EntradaJuez {
            repo: repo.path(),
            mapa: &mapa,
            inicial: Some(ini.path()),
            ..c.entrada()
        };
        let modelo = Falso::con(respuesta(json!([]), json!([])));
        let r = juzgar(&entrada, &modelo, MODELO, &LimitesPrompt::default());
        assert_eq!(r.estado, EstadoJuez::Ok, "{r:?}");
        let guardados: Vec<(&str, u32, u32, &str)> = r
            .extractos
            .iter()
            .map(|e| (e.archivo.as_str(), e.desde, e.hasta, e.motivo.as_str()))
            .collect();
        assert_eq!(
            guardados,
            vec![
                ("src/cambia.rs", 860, 940, "cambio"),
                ("src/grande.rs", 1, 300, "grande"),
            ],
            "{r:?}"
        );
    }

    #[test]
    fn juez_resultado_viejo_sin_extractos_lee_lista_vacia() {
        let r = ResultadoJuez {
            estado: EstadoJuez::Ok,
            niveles: BTreeMap::new(),
            items: vec![],
            hallazgos: vec![],
            descartes: vec![],
            hallazgos_descartados: vec![],
            faltantes: vec![],
            extractos: vec![ExtractoGuardado {
                archivo: "src/x.rs".into(),
                desde: 1,
                hasta: 2,
                motivo: "sonda".into(),
            }],
        };
        let mut valor = serde_json::to_value(&r).expect("serializa");
        valor.as_object_mut().expect("objeto").remove("extractos");
        let viejo: ResultadoJuez = serde_json::from_value(valor).expect("lee el resultado viejo");
        assert!(viejo.extractos.is_empty(), "{viejo:?}");
    }

    #[test]
    fn juez_extractos_cambio_en_linea_900_da_la_ventana_860_a_940() {
        let (repo, ini) = par_de_carpetas(&[("src/grande.rs", 2000, &[900])]);
        let salida = extractos_con_inicial(repo.path(), ini.path());
        let grande: Vec<&Extracto> = salida
            .iter()
            .filter(|e| e.archivo == "src/grande.rs")
            .collect();
        assert_eq!(grande.len(), 1, "{salida:?}");
        assert_eq!((grande[0].desde, grande[0].hasta), (860, 940));
        assert_eq!(grande[0].motivo, Motivo::Cambio);
        let texto = &grande[0].texto;
        assert!(texto.starts_with("860: fn f860() {}\n"), "{texto}");
        assert!(texto.contains("900: fn cambio_900() {}\n"), "{texto}");
        assert!(texto.ends_with("940: fn f940() {}\n"), "{texto}");
        assert!(!texto.contains("\n859: "), "{texto}");
        assert!(!texto.contains("\n941: "), "{texto}");
    }

    #[test]
    fn juez_archivo_nuevo_entra_desde_su_comienzo_antes_que_los_grandes() {
        let repo = tempfile::tempdir().expect("tempdir");
        let ini = tempfile::tempdir().expect("tempdir");
        fs::create_dir(repo.path().join("src")).expect("crea src");
        fs::write(
            repo.path().join("src/nuevo.rs"),
            "fn a() {}\nfn b() {}\nfn c() {}\n",
        )
        .expect("escribe");
        fs::write(repo.path().join("src/grande.rs"), texto_de(300, &[])).expect("escribe");
        fs::create_dir(ini.path().join("src")).expect("crea src");
        fs::write(ini.path().join("src/grande.rs"), texto_de(300, &[])).expect("escribe");
        let salida = extractos_con_inicial(repo.path(), ini.path());
        assert_eq!(salida[0].archivo, "src/nuevo.rs", "{salida:?}");
        assert_eq!((salida[0].desde, salida[0].hasta), (1, 3));
        assert_eq!(salida[0].motivo, Motivo::Cambio);
        assert!(salida[0].texto.starts_with("1: fn a() {}\n"), "{salida:?}");
        let grande = salida
            .iter()
            .position(|e| e.archivo == "src/grande.rs")
            .expect("el grande sin cambio también entra");
        assert!(grande > 0, "{salida:?}");
        assert_eq!(salida[grande].motivo, Motivo::Grande);
    }

    #[test]
    fn juez_dos_cambios_a_50_lineas_dan_una_sola_ventana() {
        let (repo, ini) = par_de_carpetas(&[("src/grande.rs", 2000, &[500, 550])]);
        let salida = extractos_con_inicial(repo.path(), ini.path());
        let grande: Vec<&Extracto> = salida
            .iter()
            .filter(|e| e.archivo == "src/grande.rs")
            .collect();
        assert_eq!(grande.len(), 1, "{salida:?}");
        assert_eq!((grande[0].desde, grande[0].hasta), (460, 590));
    }

    #[test]
    fn juez_dos_cambios_muy_separados_dan_dos_ventanas_y_un_solo_encabezado() {
        let (repo, ini) = par_de_carpetas(&[("src/grande.rs", 2000, &[100, 1500])]);
        let mapa = construir_mapa(repo.path()).expect("mapa");
        let c = caso();
        let entrada = EntradaJuez {
            repo: repo.path(),
            mapa: &mapa,
            inicial: Some(ini.path()),
            ..c.entrada()
        };
        let p = construir_prompt(&entrada, &LimitesPrompt::default());
        let seccion = seccion_extractos_de(&p.usuario);
        assert_eq!(
            seccion.matches("--- src/grande.rs ---").count(),
            1,
            "{seccion}"
        );
        assert!(
            seccion.contains("140: fn f140() {}\n…[recortado]\n1460: "),
            "{seccion}"
        );
        let salida = extractos(&entrada, &LimitesPrompt::default());
        let ventanas: Vec<(u32, u32)> = salida
            .iter()
            .filter(|e| e.archivo == "src/grande.rs")
            .map(|e| (e.desde, e.hasta))
            .collect();
        assert_eq!(ventanas, vec![(60, 140), (1460, 1540)]);
    }

    #[test]
    fn juez_extractos_con_cambios_no_pasan_el_total() {
        let archivos: Vec<(String, usize, Vec<usize>)> = (0..10)
            .map(|i| (format!("src/f{i}.rs"), 2000, vec![100, 1000, 1900]))
            .collect();
        let refs: Vec<(&str, usize, &[usize])> = archivos
            .iter()
            .map(|(r, n, c)| (r.as_str(), *n, c.as_slice()))
            .collect();
        let (repo, ini) = par_de_carpetas(&refs);
        let mapa = construir_mapa(repo.path()).expect("mapa");
        let c = caso();
        let entrada = EntradaJuez {
            repo: repo.path(),
            mapa: &mapa,
            inicial: Some(ini.path()),
            ..c.entrada()
        };
        let p = construir_prompt(&entrada, &LimitesPrompt::default());
        let usados = chars_sin_encabezados(seccion_extractos_de(&p.usuario));
        assert!(usados <= 40_000, "{usados}");
        assert!(usados >= 39_000, "el total se gasta casi entero: {usados}");
    }

    #[test]
    fn juez_extractos_linea_de_secreto_dentro_de_ventana_sale_omitida() {
        let repo = tempfile::tempdir().expect("tempdir");
        let ini = tempfile::tempdir().expect("tempdir");
        fs::create_dir(repo.path().join("src")).expect("crea src");
        fs::create_dir(ini.path().join("src")).expect("crea src");
        fs::write(
            repo.path().join("src/secreto.rs"),
            "fn inicio() {}\nconst TOKEN: &str = \"sk-SECRETO-123\";\n",
        )
        .expect("escribe");
        fs::write(
            ini.path().join("src/secreto.rs"),
            "fn inicio() {}\nconst TOKEN: &str = \"\";\n",
        )
        .expect("escribe");
        let mapa = construir_mapa(repo.path()).expect("mapa");
        let sondas = vec![hallazgo_de("gitleaks", "src/secreto.rs", 2)];
        let c = caso();
        let entrada = EntradaJuez {
            repo: repo.path(),
            mapa: &mapa,
            hallazgos_sondas: &sondas,
            inicial: Some(ini.path()),
            ..c.entrada()
        };
        let p = construir_prompt(&entrada, &LimitesPrompt::default());
        assert!(!p.usuario.contains("SECRETO-123"), "{}", p.usuario);
        assert!(
            p.usuario.contains(
                "--- src/secreto.rs ---\n1: fn inicio() {}\n2: [línea con un secreto omitida]\n"
            ),
            "{}",
            p.usuario
        );
    }

    #[test]
    fn juez_archivo_con_cambio_no_entra_otra_vez_desde_el_comienzo() {
        let (repo, ini) = par_de_carpetas(&[("src/grande.rs", 2000, &[900])]);
        let salida = extractos_con_inicial(repo.path(), ini.path());
        let veces = salida
            .iter()
            .filter(|e| e.archivo == "src/grande.rs")
            .count();
        assert_eq!(veces, 1, "{salida:?}");
        assert!(
            salida
                .iter()
                .all(|e| e.archivo != "src/grande.rs" || e.motivo == Motivo::Cambio),
            "{salida:?}"
        );
    }

    #[test]
    fn juez_veinte_lineas_cambiadas_entran_enteras_si_el_contexto_baja() {
        let cambio = |k: usize| format!("fn cambio_{k}() {{ \"{}\" }}\n", "c".repeat(100));
        let base = |k: usize| format!("fn f{k}() {{ \"{}\" }}\n", "x".repeat(50));
        let nuevo: String = (1..=200)
            .map(|k| {
                if (90..=109).contains(&k) {
                    cambio(k)
                } else {
                    base(k)
                }
            })
            .collect();
        let viejo: String = (1..=200).map(base).collect();
        let repo = tempfile::tempdir().expect("tempdir");
        let ini = tempfile::tempdir().expect("tempdir");
        for (dir, texto) in [(repo.path(), &nuevo), (ini.path(), &viejo)] {
            fs::create_dir(dir.join("src")).expect("crea src");
            fs::write(dir.join("src/largo.rs"), texto).expect("escribe");
        }
        let mapa = construir_mapa(repo.path()).expect("mapa");
        let c = caso();
        let entrada = EntradaJuez {
            repo: repo.path(),
            mapa: &mapa,
            inicial: Some(ini.path()),
            ..c.entrada()
        };
        let limites = LimitesPrompt {
            extracto_max: 4000,
            ..LimitesPrompt::default()
        };
        let salida = extractos(&entrada, &limites);
        let cambios: Vec<&Extracto> = salida
            .iter()
            .filter(|e| e.archivo == "src/largo.rs" && e.motivo == Motivo::Cambio)
            .collect();
        assert_eq!(cambios.len(), 1, "{salida:?}");
        let v = cambios[0];
        for k in 90..=109 {
            let fila = format!("{k}: {}", cambio(k));
            assert!(v.texto.contains(&fila), "falta la línea {k}: {}", v.texto);
        }
        assert!(
            v.texto.chars().count() <= 4000,
            "{}",
            v.texto.chars().count()
        );
        // El contexto baja por igual de cada lado.
        assert_eq!(90 - v.desde, v.hasta - 109, "{salida:?}");
    }

    #[test]
    fn juez_cambio_que_solo_pasa_el_maximo_se_parte_en_ventanas() {
        let cambio = |k: usize| format!("fn cambio_{k}() {{ \"{}\" }}\n", "c".repeat(190));
        let nuevo: String = (1..=300)
            .map(|k| {
                if (100..=159).contains(&k) {
                    cambio(k)
                } else {
                    format!("fn f{k}() {{}}\n")
                }
            })
            .collect();
        let viejo: String = (1..=300).map(|k| format!("fn f{k}() {{}}\n")).collect();
        let repo = tempfile::tempdir().expect("tempdir");
        let ini = tempfile::tempdir().expect("tempdir");
        for (dir, texto) in [(repo.path(), &nuevo), (ini.path(), &viejo)] {
            fs::create_dir(dir.join("src")).expect("crea src");
            fs::write(dir.join("src/cambio.rs"), texto).expect("escribe");
        }
        let mapa = construir_mapa(repo.path()).expect("mapa");
        let c = caso();
        let entrada = EntradaJuez {
            repo: repo.path(),
            mapa: &mapa,
            inicial: Some(ini.path()),
            ..c.entrada()
        };
        let limites = LimitesPrompt::default();
        let salida = extractos(&entrada, &limites);
        let cambios: Vec<&Extracto> = salida
            .iter()
            .filter(|e| e.archivo == "src/cambio.rs" && e.motivo == Motivo::Cambio)
            .collect();
        assert!(cambios.len() >= 2, "{salida:?}");
        for e in &cambios {
            assert!(
                e.texto.chars().count() <= limites.extracto_max,
                "{}",
                e.texto.chars().count()
            );
        }
        for k in 100..=159 {
            let fila = format!("{k}: {}", cambio(k));
            assert!(
                cambios.iter().any(|e| e.texto.contains(&fila)),
                "falta la línea {k}"
            );
        }
        let p = construir_prompt(&entrada, &limites);
        let seccion = seccion_extractos_de(&p.usuario);
        assert!(
            chars_sin_encabezados(seccion) <= limites.extractos_total,
            "{seccion}"
        );
        assert_eq!(
            seccion.matches("--- src/cambio.rs ---").count(),
            1,
            "{seccion}"
        );
        assert!(seccion.contains(&format!("{MARCA_RECORTE}\n")), "{seccion}");
    }

    #[test]
    fn juez_linea_mas_larga_que_el_maximo_deja_marca_con_su_numero() {
        let repo = tempfile::tempdir().expect("tempdir");
        fs::create_dir(repo.path().join("src")).expect("crea src");
        let texto = format!(
            "fn f1() {{}}\nfn f2() {{}}\n{}\nfn f4() {{}}\nfn f5() {{}}\n",
            "l".repeat(7000)
        );
        fs::write(repo.path().join("src/larga.rs"), texto).expect("escribe");
        let mapa = construir_mapa(repo.path()).expect("mapa");
        let c = caso();
        let entrada = EntradaJuez {
            repo: repo.path(),
            mapa: &mapa,
            ..c.entrada()
        };
        let limites = LimitesPrompt {
            extracto_max: 6000,
            ..LimitesPrompt::default()
        };
        let salida = extractos(&entrada, &limites);
        let e = salida
            .iter()
            .find(|e| e.archivo == "src/larga.rs")
            .expect("hay extracto");
        assert!(e.texto.contains("1: fn f1() {}\n"), "{}", e.texto);
        assert!(e.texto.contains("2: fn f2() {}\n"), "{}", e.texto);
        assert!(
            e.texto.contains("3: [línea 3 omitida por largo]\n"),
            "{}",
            e.texto
        );
        assert!(e.texto.contains("4: fn f4() {}\n"), "{}", e.texto);
        assert!(e.texto.contains("5: fn f5() {}\n"), "{}", e.texto);
        assert!(!e.texto.contains("lll"), "{}", e.texto);
        assert!(e.texto.chars().count() <= 6000);
        assert_eq!(e.hasta, 5);
    }

    #[test]
    fn juez_marca_de_largo_cuenta_en_el_maximo_y_en_el_total() {
        let repo = tempfile::tempdir().expect("tempdir");
        fs::create_dir(repo.path().join("src")).expect("crea src");
        let texto: String = (1..=60)
            .map(|k| {
                if k == 2 {
                    format!("2{}\n", "y".repeat(500))
                } else {
                    format!("fn f{k}() {{}}\n")
                }
            })
            .collect();
        fs::write(repo.path().join("src/a.rs"), &texto).expect("escribe");
        fs::write(repo.path().join("src/b.rs"), &texto).expect("escribe");
        let mapa = construir_mapa(repo.path()).expect("mapa");
        let c = caso();
        let entrada = EntradaJuez {
            repo: repo.path(),
            mapa: &mapa,
            ..c.entrada()
        };
        let limites = LimitesPrompt {
            extractos_total: 700,
            extracto_max: 300,
            ..LimitesPrompt::default()
        };
        let salida = extractos(&entrada, &limites);
        assert!(salida.len() >= 2, "{salida:?}");
        for e in &salida {
            assert!(
                e.texto.chars().count() <= limites.extracto_max,
                "{}",
                e.texto.chars().count()
            );
        }
        let suma: usize = salida.iter().map(|e| e.texto.chars().count()).sum();
        assert!(suma <= limites.extractos_total, "{suma}");
        let a = salida
            .iter()
            .find(|e| e.archivo == "src/a.rs")
            .expect("hay extracto de a.rs");
        assert!(
            a.texto.contains("2: [línea 2 omitida por largo]\n"),
            "{}",
            a.texto
        );
    }

    #[test]
    fn juez_linea_larga_cambiada_deja_marca_en_la_ventana_de_cambio() {
        let nuevo: String = (1..=40)
            .map(|k| {
                if k == 20 {
                    format!("fn cambio_20() {{ \"{}\" }}\n", "c".repeat(7000))
                } else {
                    format!("fn f{k}() {{}}\n")
                }
            })
            .collect();
        let viejo: String = (1..=40).map(|k| format!("fn f{k}() {{}}\n")).collect();
        let repo = tempfile::tempdir().expect("tempdir");
        let ini = tempfile::tempdir().expect("tempdir");
        for (dir, texto) in [(repo.path(), &nuevo), (ini.path(), &viejo)] {
            fs::create_dir(dir.join("src")).expect("crea src");
            fs::write(dir.join("src/cambio_larga.rs"), texto).expect("escribe");
        }
        let mapa = construir_mapa(repo.path()).expect("mapa");
        let c = caso();
        let entrada = EntradaJuez {
            repo: repo.path(),
            mapa: &mapa,
            inicial: Some(ini.path()),
            ..c.entrada()
        };
        let limites = LimitesPrompt::default();
        let salida = extractos(&entrada, &limites);
        let cambios: Vec<&Extracto> = salida
            .iter()
            .filter(|e| e.archivo == "src/cambio_larga.rs" && e.motivo == Motivo::Cambio)
            .collect();
        assert!(
            cambios
                .iter()
                .any(|e| e.texto.contains("20: [línea 20 omitida por largo]\n")),
            "{salida:?}"
        );
        for e in &cambios {
            assert!(e.texto.chars().count() <= limites.extracto_max);
        }
        let p = construir_prompt(&entrada, &limites);
        let seccion = seccion_extractos_de(&p.usuario);
        assert!(
            chars_sin_encabezados(seccion) <= limites.extractos_total,
            "{seccion}"
        );
    }

    #[test]
    fn juez_archivo_que_empieza_con_linea_larguisima_no_se_salta() {
        let repo = tempfile::tempdir().expect("tempdir");
        fs::create_dir(repo.path().join("src")).expect("crea src");
        let resto: String = (2..=300).map(|k| format!("fn f{k}() {{}}\n")).collect();
        let texto = format!("{}\n{resto}", "z".repeat(9000));
        fs::write(repo.path().join("src/primera.rs"), texto).expect("escribe");
        let mapa = construir_mapa(repo.path()).expect("mapa");
        let c = caso();
        let entrada = EntradaJuez {
            repo: repo.path(),
            mapa: &mapa,
            ..c.entrada()
        };
        let salida = extractos(&entrada, &LimitesPrompt::default());
        let e = salida
            .iter()
            .find(|e| e.archivo == "src/primera.rs")
            .expect("el archivo no se salta");
        assert!(
            e.texto.starts_with("1: [línea 1 omitida por largo]\n"),
            "{}",
            e.texto
        );
        assert!(e.texto.contains("2: fn f2() {}\n"), "{}", e.texto);
    }

    #[test]
    fn juez_largo_de_filas_coincide_con_lo_que_escribe_numerar_rango() {
        let contenido = format!("corta\n{}\n{}\nfin\n", "a".repeat(80), "b".repeat(10));
        let lineas: Vec<&str> = contenido.lines().collect();
        let omitidas: BTreeSet<u32> = [4].into_iter().collect();
        for (desde, hasta) in [(1, 4), (2, 3), (3, 4)] {
            let (texto, _) = numerar_rango(&contenido, desde, hasta, usize::MAX, &omitidas, 40);
            assert_eq!(
                largo_filas(&lineas, desde, hasta, &omitidas, 40),
                texto.chars().count(),
                "{texto}"
            );
        }
        let (texto, _) = numerar_rango(&contenido, 1, 4, usize::MAX, &omitidas, 40);
        assert!(
            texto.contains("2: [línea 2 omitida por largo]\n"),
            "{texto}"
        );
        assert!(
            texto.contains("4: [línea con un secreto omitida]\n"),
            "{texto}"
        );
    }

    #[test]
    fn juez_extractos_inicial_sin_diferencias_es_identico_a_sin_inicial() {
        let (repo, ini) = par_de_carpetas(&[("src/grande.rs", 300, &[]), ("src/corto.rs", 5, &[])]);
        let mapa = construir_mapa(repo.path()).expect("mapa");
        let c = caso();
        let sin = EntradaJuez {
            repo: repo.path(),
            mapa: &mapa,
            ..c.entrada()
        };
        let con = EntradaJuez {
            repo: repo.path(),
            mapa: &mapa,
            inicial: Some(ini.path()),
            ..c.entrada()
        };
        let limites = LimitesPrompt::default();
        assert_eq!(extractos(&sin, &limites), extractos(&con, &limites));
        assert_eq!(
            construir_prompt(&sin, &limites),
            construir_prompt(&con, &limites)
        );
    }

    #[test]
    fn juez_huella_cambia_con_la_lectura_guiada() {
        let vieja =
            crate::goal::calcular_hash(format!("{PROMPT_SISTEMA}\n{MARCA_RECORTE}").as_bytes());
        assert_ne!(huella_del_prompt(), vieja);
    }

    /// Texto que solo existe fuera de la raíz del repo.
    const AFUERA: &str = "secreto fuera de la raiz";

    /// Repo en `base/repo/` con `src/a.rs`. Un `afuera.txt` hermano de `repo/` tiene `AFUERA`.
    fn repo_hermano_de_afuera() -> (TempDir, PathBuf) {
        let base = tempfile::tempdir().expect("tempdir");
        let repo = base.path().join("repo");
        fs::create_dir_all(repo.join("src")).expect("crea src");
        fs::write(repo.join("src/a.rs"), ALMACEN).expect("escribe");
        fs::write(base.path().join("afuera.txt"), AFUERA).expect("escribe");
        (base, repo)
    }

    /// Prompt del juez con un hallazgo de sonda que cita `archivo`.
    fn prompt_con_sonda_en(repo: &Path, archivo: &str) -> String {
        let mapa = construir_mapa(repo).expect("mapa");
        let sondas = vec![hallazgo_de(SONDA_ARQUITECTURA, archivo, 1)];
        let c = caso();
        let entrada = EntradaJuez {
            repo,
            mapa: &mapa,
            hallazgos_sondas: &sondas,
            ..c.entrada()
        };
        construir_prompt(&entrada, &LimitesPrompt::default()).usuario
    }

    #[test]
    fn juez_extractos_no_leen_con_punto_punto_fuera_de_la_raiz() {
        let (_base, repo) = repo_hermano_de_afuera();
        let usuario = prompt_con_sonda_en(&repo, "../afuera.txt");
        assert!(!usuario.contains(AFUERA), "{usuario}");
    }

    #[test]
    fn juez_extractos_leen_punto_punto_que_queda_dentro() {
        let (_base, repo) = repo_hermano_de_afuera();
        let usuario = prompt_con_sonda_en(&repo, "src/../src/a.rs");
        assert!(
            usuario.contains("--- src/../src/a.rs ---\n1: pub struct Almacen;"),
            "{usuario}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn juez_extractos_no_siguen_enlace_que_sale_de_la_raiz() {
        let (base, repo) = repo_hermano_de_afuera();
        std::os::unix::fs::symlink(base.path().join("afuera.txt"), repo.join("enlace.txt"))
            .expect("crea enlace");
        let usuario = prompt_con_sonda_en(&repo, "enlace.txt");
        assert!(!usuario.contains(AFUERA), "{usuario}");
    }

    #[test]
    fn juez_extractos_omiten_ruta_absoluta() {
        let (base, repo) = repo_hermano_de_afuera();
        let absoluta = base.path().join("afuera.txt");
        let usuario = prompt_con_sonda_en(&repo, &absoluta.to_string_lossy());
        assert!(!usuario.contains(AFUERA), "{usuario}");
    }

    /// Hallazgo de la sonda `sonda` en `archivo`, línea `linea`.
    fn hallazgo_de(sonda: &str, archivo: &str, linea: u32) -> Hallazgo {
        Hallazgo {
            dimension: Dimension::Seguridad,
            severidad: 3,
            archivo: archivo.into(),
            linea,
            cita: "x".into(),
            detalle: "detalle".into(),
            fuente: Fuente::Sonda(sonda.into()),
            id: String::new(),
            impacto: Vec::new(),
        }
    }

    /// Repo con `src/secreto.rs` de dos líneas: la línea 2 tiene un secreto.
    fn repo_con_secreto() -> (TempDir, Mapa) {
        let repo = tempfile::tempdir().expect("tempdir");
        fs::create_dir(repo.path().join("src")).expect("crea src");
        fs::write(
            repo.path().join("src/secreto.rs"),
            "fn inicio() {}\nconst TOKEN: &str = \"sk-SECRETO-123\";\n",
        )
        .expect("escribe");
        let mapa = construir_mapa(repo.path()).expect("mapa");
        (repo, mapa)
    }

    #[test]
    fn juez_prompt_no_envia_la_linea_de_un_hallazgo_gitleaks() {
        let (repo, mapa) = repo_con_secreto();
        let sondas = vec![hallazgo_de("gitleaks", "src/secreto.rs", 2)];
        let c = caso();
        let entrada = EntradaJuez {
            repo: repo.path(),
            mapa: &mapa,
            hallazgos_sondas: &sondas,
            ..c.entrada()
        };
        let p = construir_prompt(&entrada, &LimitesPrompt::default());
        assert!(!p.usuario.contains("SECRETO-123"), "{}", p.usuario);
        assert!(
            p.usuario.contains(
                "--- src/secreto.rs ---\n1: fn inicio() {}\n2: [línea con un secreto omitida]\n"
            ),
            "{}",
            p.usuario
        );
    }

    #[test]
    fn juez_prompt_omite_gitleaks_aunque_otro_hallazgo_cite_el_archivo() {
        let (repo, mapa) = repo_con_secreto();
        let sondas = vec![
            hallazgo_de("arquitectura-nativa", "src/secreto.rs", 1),
            hallazgo_de("gitleaks", "src/secreto.rs", 2),
        ];
        let c = caso();
        let entrada = EntradaJuez {
            repo: repo.path(),
            mapa: &mapa,
            hallazgos_sondas: &sondas,
            ..c.entrada()
        };
        let p = construir_prompt(&entrada, &LimitesPrompt::default());
        assert!(!p.usuario.contains("SECRETO-123"), "{}", p.usuario);
        assert!(p.usuario.contains("2: [línea con un secreto omitida]\n"));
    }

    #[test]
    fn juez_prompt_sin_gitleaks_es_identico_al_de_antes() {
        // Texto fijo del extracto de `caso()` sin hallazgos de gitleaks: el prompt no cambia.
        let c = caso();
        let p = construir_prompt(&c.entrada(), &LimitesPrompt::default());
        let esperado = "\nEXTRACTOS:\n--- src/almacen.rs ---\n1: pub struct Almacen;\n2: \n3: impl Almacen {\n4:     pub fn guardar(&self) {}\n5: }\n";
        assert!(p.usuario.ends_with(esperado), "{}", p.usuario);
    }

    #[test]
    fn juez_prompt_es_determinista() {
        let c = caso();
        let a = construir_prompt(&c.entrada(), &LimitesPrompt::default());
        let b = construir_prompt(&c.entrada(), &LimitesPrompt::default());
        assert_eq!(a, b);
    }

    #[test]
    fn juez_modelo_recibe_el_prompt_construido() {
        let c = caso();
        let limites = LimitesPrompt::default();
        let modelo = Falso::con(respuesta(json!([]), json!([])));
        juzgar(&c.entrada(), &modelo, MODELO, &limites);
        // La respuesta sin ítems pide un reintento: el primer prompt es el construido.
        let visto = modelo.visto.borrow();
        assert_eq!(visto.len(), 2);
        assert_eq!(visto[0], construir_prompt(&c.entrada(), &limites));
    }

    #[test]
    fn juez_marca_solo_el_prompt_del_reintento() {
        let c = caso();
        let vacia = respuesta(json!([]), json!([]));
        let completa = respuesta(
            json!([
                item_nivel(MODULARIDAD, 2, json!([real()])),
                item_nivel(TOPE, 1, json!([real()])),
            ]),
            json!([]),
        );
        let modelo = FalsoSecuencia::nueva(vec![Ok(vacia), Ok(completa)]);
        juzgar_secuencia(&c, &modelo);
        let visto = modelo.visto.borrow();
        assert_eq!(visto.len(), 2);
        assert!(!visto[0].reintento_juez, "la 1ª llamada no es reintento");
        assert!(visto[1].reintento_juez, "la 2ª llamada es el reintento");
    }

    // Reintento: un solo por juicio, solo para los ids rotos o sin gate.

    /// Modelo falso con una respuesta por llamada, en orden. Una respuesta puede ser error.
    struct FalsoSecuencia {
        respuestas: RefCell<VecDeque<Result<RespuestaJuez, String>>>,
        visto: RefCell<Vec<Prompt>>,
    }

    impl FalsoSecuencia {
        fn nueva(respuestas: Vec<Result<String, String>>) -> Self {
            let respuestas = respuestas
                .into_iter()
                .map(|r| {
                    r.map(|texto| RespuestaJuez {
                        texto,
                        truncada: false,
                        costo_usd: 0.0,
                    })
                })
                .collect();
            Self {
                respuestas: RefCell::new(respuestas),
                visto: RefCell::new(Vec::new()),
            }
        }
    }

    impl ModeloJuez for FalsoSecuencia {
        fn consultar(&self, prompt: &Prompt) -> Result<RespuestaJuez, ErrorOraculo> {
            self.visto.borrow_mut().push(prompt.clone());
            let siguiente = self
                .respuestas
                .borrow_mut()
                .pop_front()
                .expect("el modelo recibió más llamadas de las programadas");
            siguiente.map_err(ErrorOraculo::Infra)
        }
    }

    fn juzgar_secuencia(c: &Caso, modelo: &FalsoSecuencia) -> ResultadoJuez {
        juzgar(&c.entrada(), modelo, MODELO, &LimitesPrompt::default())
    }

    /// Sección de ids e ítems del prompt: de `IDS OBLIGATORIOS:` a `EXTRACTOS:`.
    fn seccion_ids(usuario: &str) -> &str {
        let inicio = usuario
            .find("IDS OBLIGATORIOS:")
            .expect("hay ids obligatorios");
        let fin = usuario.find("EXTRACTOS:").expect("hay extractos");
        &usuario[inicio..fin]
    }

    /// Sección `REINTENTO:` del prompt, de su rótulo a `EXTRACTOS:`.
    fn seccion_reintento(usuario: &str) -> &str {
        let inicio = usuario
            .find("REINTENTO:")
            .expect("hay sección de reintento");
        let fin = usuario.find("EXTRACTOS:").expect("hay extractos");
        &usuario[inicio..fin]
    }

    fn descarte_de(r: &ResultadoJuez, id: &str, motivo: &str) -> bool {
        r.descartes
            .iter()
            .any(|d| d.que == format!("ítem {id}") && d.motivo.contains(motivo))
    }

    #[test]
    fn juez_sin_evidencia_no_hace_reintento() {
        let c = caso();
        let texto = respuesta(
            json!([
                item_nivel(MODULARIDAD, 1, json!([real()])),
                sin_evidencia_json(TOPE),
            ]),
            json!([]),
        );
        let modelo = FalsoSecuencia::nueva(vec![Ok(texto)]);
        let r = juzgar_secuencia(&c, &modelo);
        assert_eq!(
            modelo.visto.borrow().len(),
            1,
            "sin_evidencia no pide reintento"
        );
        assert_eq!(r.faltantes, vec![TOPE.to_string()]);
    }

    #[test]
    fn juez_id_omitido_hace_un_reintento_solo_con_ese_id() {
        let c = caso();
        let primera = respuesta(
            json!([item_nivel(MODULARIDAD, 2, json!([real()]))]),
            json!([]),
        );
        let segunda = respuesta(json!([item_nivel(TOPE, 1, json!([real()]))]), json!([]));
        let modelo = FalsoSecuencia::nueva(vec![Ok(primera), Ok(segunda)]);
        let r = juzgar_secuencia(&c, &modelo);
        let visto = modelo.visto.borrow();
        assert_eq!(visto.len(), 2, "exactamente un reintento");
        let ids = seccion_ids(&visto[1].usuario);
        assert!(ids.contains(TOPE), "{ids}");
        assert!(!ids.contains(MODULARIDAD), "{ids}");
        assert_eq!(r.niveles.get(TOPE), Some(&1));
        assert_eq!(r.niveles.get(MODULARIDAD), Some(&2));
        assert!(r.faltantes.is_empty(), "{:?}", r.faltantes);
    }

    #[test]
    fn juez_evidencia_que_no_pasa_el_gate_reintenta_con_la_cita_y_su_linea() {
        let c = caso();
        let inventada = evidencia("src/almacen.rs", 4, "pub fn borrar(&self) {}");
        let primera = respuesta(
            json!([
                item_nivel(MODULARIDAD, 2, json!([real()])),
                item_nivel(TOPE, 3, json!([inventada])),
            ]),
            json!([]),
        );
        let segunda = respuesta(json!([item_nivel(TOPE, 3, json!([real()]))]), json!([]));
        let modelo = FalsoSecuencia::nueva(vec![Ok(primera), Ok(segunda)]);
        let r = juzgar_secuencia(&c, &modelo);
        let visto = modelo.visto.borrow();
        assert_eq!(visto.len(), 2, "exactamente un reintento");
        let reintento = seccion_reintento(&visto[1].usuario);
        assert!(reintento.contains("pub fn borrar(&self) {}"), "{reintento}");
        assert!(reintento.contains("src/almacen.rs:4"), "{reintento}");
        assert_eq!(r.niveles.get(TOPE), Some(&3));
        assert!(r.faltantes.is_empty(), "{:?}", r.faltantes);
    }

    #[test]
    fn juez_reintento_sin_respuesta_deja_faltante_con_su_motivo() {
        let c = caso();
        let primera = respuesta(
            json!([item_nivel(MODULARIDAD, 2, json!([real()]))]),
            json!([]),
        );
        let segunda = respuesta(json!([]), json!([]));
        let modelo = FalsoSecuencia::nueva(vec![Ok(primera), Ok(segunda)]);
        let r = juzgar_secuencia(&c, &modelo);
        assert_eq!(r.estado, EstadoJuez::Ok);
        assert_eq!(r.faltantes, vec![TOPE.to_string()]);
        assert!(
            descarte_de(&r, TOPE, "tras un reintento"),
            "{:?}",
            r.descartes
        );
    }

    #[test]
    fn juez_gate_tambien_en_el_reintento_deja_motivo_de_gate() {
        let c = caso();
        let inventada = evidencia("src/almacen.rs", 4, "pub fn borrar(&self) {}");
        let primera = respuesta(
            json!([
                item_nivel(MODULARIDAD, 2, json!([real()])),
                item_nivel(TOPE, 3, json!([inventada.clone()])),
            ]),
            json!([]),
        );
        let segunda = respuesta(json!([item_nivel(TOPE, 3, json!([inventada]))]), json!([]));
        let modelo = FalsoSecuencia::nueva(vec![Ok(primera), Ok(segunda)]);
        let r = juzgar_secuencia(&c, &modelo);
        assert_eq!(modelo.visto.borrow().len(), 2, "exactamente un reintento");
        assert_eq!(r.faltantes, vec![TOPE.to_string()]);
        assert!(
            descarte_de(&r, TOPE, MOTIVO_GATE_REINTENTO),
            "{:?}",
            r.descartes
        );
        assert!(
            !descarte_de(&r, TOPE, MOTIVO_JUEZ_SIN_REINTENTO),
            "{:?}",
            r.descartes
        );
    }

    #[test]
    fn juez_nivel_texto_dispara_reintento_y_el_reintento_valido_gana() {
        let c = caso();
        let primera = respuesta(
            json!([
                item_nivel(MODULARIDAD, 2, json!([real()])),
                {"item": TOPE, "nivel": "alto", "evidencia": [real()]},
            ]),
            json!([]),
        );
        let segunda = respuesta(json!([item_nivel(TOPE, 1, json!([real()]))]), json!([]));
        let modelo = FalsoSecuencia::nueva(vec![Ok(primera), Ok(segunda)]);
        let r = juzgar_secuencia(&c, &modelo);
        assert_eq!(modelo.visto.borrow().len(), 2, "exactamente un reintento");
        assert_eq!(r.niveles.get(TOPE), Some(&1));
        assert!(r.faltantes.is_empty(), "{:?}", r.faltantes);
    }

    #[test]
    fn juez_nivel_flotante_entero_vale_como_entero_sin_reintento() {
        let c = caso();
        let primera = respuesta(
            json!([
                {"item": MODULARIDAD, "nivel": 2.0, "evidencia": [real()]},
                item_nivel(TOPE, 3, json!([real()])),
            ]),
            json!([]),
        );
        let modelo = FalsoSecuencia::nueva(vec![Ok(primera)]);
        let r = juzgar_secuencia(&c, &modelo);
        assert_eq!(modelo.visto.borrow().len(), 1, "sin reintento");
        assert_eq!(r.niveles.get(MODULARIDAD), Some(&2));
        assert_eq!(r.niveles.get(TOPE), Some(&3));
    }

    #[test]
    fn juez_nivel_flotante_no_entero_o_fuera_de_rango_va_al_reintento() {
        for nivel in [2.5, -1.0, 4.0] {
            let c = caso();
            let primera = respuesta(
                json!([
                    {"item": MODULARIDAD, "nivel": nivel, "evidencia": [real()]},
                    item_nivel(TOPE, 1, json!([real()])),
                ]),
                json!([]),
            );
            let segunda = respuesta(
                json!([item_nivel(MODULARIDAD, 1, json!([real()]))]),
                json!([]),
            );
            let modelo = FalsoSecuencia::nueva(vec![Ok(primera), Ok(segunda)]);
            let r = juzgar_secuencia(&c, &modelo);
            assert_eq!(
                modelo.visto.borrow().len(),
                2,
                "nivel {nivel}: un reintento"
            );
            assert_eq!(r.niveles.get(MODULARIDAD), Some(&1), "nivel {nivel}");
        }
    }

    #[test]
    fn juez_nivel_cinco_en_reintento_deja_motivo_de_nivel_invalido() {
        let c = caso();
        let primera = respuesta(
            json!([
                item_nivel(MODULARIDAD, 2, json!([real()])),
                item_nivel(TOPE, 5, json!([real()])),
            ]),
            json!([]),
        );
        let segunda = respuesta(json!([item_nivel(TOPE, 5, json!([real()]))]), json!([]));
        let modelo = FalsoSecuencia::nueva(vec![Ok(primera), Ok(segunda)]);
        let r = juzgar_secuencia(&c, &modelo);
        assert_eq!(modelo.visto.borrow().len(), 2, "exactamente un reintento");
        assert_eq!(r.faltantes, vec![TOPE.to_string()]);
        assert!(
            descarte_de(&r, TOPE, MOTIVO_JUEZ_NIVEL_INVALIDO),
            "{:?}",
            r.descartes
        );
        assert!(
            !descarte_de(&r, TOPE, MOTIVO_JUEZ_SIN_REINTENTO),
            "{:?}",
            r.descartes
        );
    }

    #[test]
    fn juez_nivel_texto_en_reintento_deja_motivo_de_nivel_invalido() {
        let c = caso();
        let primera = respuesta(
            json!([item_nivel(MODULARIDAD, 2, json!([real()]))]),
            json!([]),
        );
        let segunda = respuesta(
            json!([{"item": TOPE, "nivel": "alto", "evidencia": [real()]}]),
            json!([]),
        );
        let modelo = FalsoSecuencia::nueva(vec![Ok(primera), Ok(segunda)]);
        let r = juzgar_secuencia(&c, &modelo);
        assert_eq!(modelo.visto.borrow().len(), 2, "exactamente un reintento");
        assert_eq!(r.faltantes, vec![TOPE.to_string()]);
        assert!(
            descarte_de(&r, TOPE, MOTIVO_JUEZ_NIVEL_INVALIDO),
            "{:?}",
            r.descartes
        );
    }

    #[test]
    fn juez_nunca_hay_mas_de_un_reintento() {
        let c = caso();
        let vacia = respuesta(json!([]), json!([]));
        let completa = respuesta(
            json!([
                item_nivel(MODULARIDAD, 2, json!([real()])),
                item_nivel(TOPE, 1, json!([real()])),
            ]),
            json!([]),
        );
        let modelo = FalsoSecuencia::nueva(vec![Ok(vacia.clone()), Ok(vacia), Ok(completa)]);
        let r = juzgar_secuencia(&c, &modelo);
        assert_eq!(modelo.visto.borrow().len(), 2, "el tercer envío no ocurre");
        assert_eq!(r.faltantes, vec![MODULARIDAD.to_string(), TOPE.to_string()]);
    }

    #[test]
    fn juez_reintento_frenado_por_tope_deja_motivo_de_tope() {
        let c = caso();
        let primera = respuesta(
            json!([item_nivel(MODULARIDAD, 2, json!([real()]))]),
            json!([]),
        );
        let tope = "llamada rechazada antes de enviarla: costo estimado 0.100000 USD, margen 0.000000 USD del tope de API de 5.00 USD";
        let modelo = FalsoSecuencia::nueva(vec![Ok(primera), Err(tope.into())]);
        let r = juzgar_secuencia(&c, &modelo);
        assert_eq!(modelo.visto.borrow().len(), 2);
        assert_eq!(r.faltantes, vec![TOPE.to_string()]);
        assert!(
            descarte_de(&r, TOPE, MOTIVO_REINTENTO_TOPE),
            "{:?}",
            r.descartes
        );
        assert!(
            !descarte_de(&r, TOPE, MOTIVO_JUEZ_SIN_REINTENTO),
            "{:?}",
            r.descartes
        );
    }

    #[test]
    fn juez_reintento_con_error_que_no_es_tope_deja_motivo_sin_reintento() {
        let c = caso();
        let primera = respuesta(
            json!([item_nivel(MODULARIDAD, 2, json!([real()]))]),
            json!([]),
        );
        let modelo = FalsoSecuencia::nueva(vec![Ok(primera), Err("tope".into())]);
        let r = juzgar_secuencia(&c, &modelo);
        assert!(
            descarte_de(&r, TOPE, MOTIVO_JUEZ_SIN_REINTENTO),
            "{:?}",
            r.descartes
        );
        assert!(
            !descarte_de(&r, TOPE, MOTIVO_REINTENTO_TOPE),
            "{:?}",
            r.descartes
        );
    }

    #[test]
    fn juez_reintento_con_error_del_modelo_deja_faltante() {
        let c = caso();
        let primera = respuesta(
            json!([item_nivel(MODULARIDAD, 2, json!([real()]))]),
            json!([]),
        );
        let modelo = FalsoSecuencia::nueva(vec![Ok(primera), Err("tope".into())]);
        let r = juzgar_secuencia(&c, &modelo);
        assert_eq!(modelo.visto.borrow().len(), 2);
        assert_eq!(r.estado, EstadoJuez::Ok);
        assert_eq!(
            r.niveles.get(MODULARIDAD),
            Some(&2),
            "la 1ª respuesta se conserva"
        );
        assert_eq!(r.faltantes, vec![TOPE.to_string()]);
        assert!(
            descarte_de(&r, TOPE, "tras un reintento"),
            "{:?}",
            r.descartes
        );
    }

    #[test]
    fn juez_reintento_usa_el_mismo_max_tokens() {
        let c = caso();
        let limites = LimitesPrompt {
            max_tokens: 8000,
            ..LimitesPrompt::default()
        };
        let primera = respuesta(
            json!([item_nivel(MODULARIDAD, 2, json!([real()]))]),
            json!([]),
        );
        let segunda = respuesta(json!([item_nivel(TOPE, 1, json!([real()]))]), json!([]));
        let modelo = FalsoSecuencia::nueva(vec![Ok(primera), Ok(segunda)]);
        juzgar(&c.entrada(), &modelo, MODELO, &limites);
        let visto = modelo.visto.borrow();
        assert_eq!(visto.len(), 2);
        assert_eq!(visto[0].max_tokens, 8000);
        assert_eq!(visto[1].max_tokens, 8000);
        assert_eq!(visto[1].sistema, visto[0].sistema);
        assert_eq!(visto[1].fase, visto[0].fase);
    }

    #[test]
    fn juez_reintento_ignora_id_no_pedido_con_aviso() {
        let c = caso();
        let primera = respuesta(
            json!([item_nivel(MODULARIDAD, 2, json!([real()]))]),
            json!([]),
        );
        let segunda = respuesta(
            json!([
                item_nivel("diseno.inventado", 2, json!([real()])),
                item_nivel(MODULARIDAD, 3, json!([real()])),
                item_nivel(TOPE, 1, json!([real()])),
            ]),
            json!([]),
        );
        let modelo = FalsoSecuencia::nueva(vec![Ok(primera), Ok(segunda)]);
        let r = juzgar_secuencia(&c, &modelo);
        assert!(!r.niveles.contains_key("diseno.inventado"));
        assert_eq!(
            r.niveles.get(MODULARIDAD),
            Some(&2),
            "el reintento no reescribe un ítem ya puntuado"
        );
        assert_eq!(r.niveles.get(TOPE), Some(&1));
        assert!(
            descarte_de(&r, "diseno.inventado", "no se pidió"),
            "{:?}",
            r.descartes
        );
        assert!(r.faltantes.is_empty(), "{:?}", r.faltantes);
    }

    #[test]
    fn juez_reintento_sin_evidencia_no_queda_como_sin_reintento() {
        let c = caso();
        let inventada = evidencia("src/almacen.rs", 4, "pub fn borrar(&self) {}");
        let primera = respuesta(
            json!([
                item_nivel(MODULARIDAD, 2, json!([real()])),
                item_nivel(TOPE, 3, json!([inventada])),
            ]),
            json!([]),
        );
        let segunda = respuesta(json!([sin_evidencia_json(TOPE)]), json!([]));
        let modelo = FalsoSecuencia::nueva(vec![Ok(primera), Ok(segunda)]);
        let r = juzgar_secuencia(&c, &modelo);
        assert_eq!(r.faltantes, vec![TOPE.to_string()]);
        assert!(descarte_de(&r, TOPE, "sin_evidencia"), "{:?}", r.descartes);
        assert!(
            !descarte_de(&r, TOPE, "tras un reintento"),
            "{:?}",
            r.descartes
        );
    }

    #[test]
    fn juez_gate_guarda_las_citas_que_fallaron() {
        let c = caso();
        let inventada = evidencia("src/almacen.rs", 4, "pub fn borrar(&self) {}");
        let l = leer(
            &c,
            json!([
                item_nivel(TOPE, 3, json!([inventada])),
                item_nivel(MODULARIDAD, 2, json!([inventada, real()])),
            ]),
        );
        assert_eq!(
            l.gate.get(TOPE),
            Some(&vec![CitaFallida {
                archivo: "src/almacen.rs".into(),
                linea: 4,
                cita: "pub fn borrar(&self) {}".into(),
            }])
        );
        assert!(!l.gate.contains_key(MODULARIDAD));
    }

    // Topes de las sondas y puntaje.

    #[test]
    fn juez_tope_con_ciclo_y_mixto_baja_a_uno() {
        let topes = topes_desde_sondas(&[ciclo()], &[rubrica_mixta()]);
        assert_eq!(topes, BTreeMap::from([(MODULARIDAD.to_string(), 1)]));
        let r = ResultadoJuez {
            estado: EstadoJuez::Ok,
            niveles: BTreeMap::from([(MODULARIDAD.to_string(), 3), (TOPE.to_string(), 3)]),
            items: vec![],
            hallazgos: vec![],
            descartes: vec![],
            hallazgos_descartados: vec![],
            faltantes: vec![],
            extractos: vec![],
        };
        let p = puntuar_con_juez(&rubrica_mixta(), &r, &topes, MODELO).expect("puntúa");
        let nivel = p
            .items
            .iter()
            .find(|i| i.item == MODULARIDAD)
            .expect("está");
        assert_eq!(nivel.nivel, 1);
        // (3·1 + 1·3) / (3·4) = 0.5
        assert!((p.valor.expect("hay valor") - 0.5).abs() < 1e-6);
    }

    #[test]
    fn juez_tope_sin_mixto_no_aplica() {
        let topes = topes_desde_sondas(&[ciclo()], &[rubrica_valida()]);
        assert!(topes.is_empty());
    }

    #[test]
    fn juez_tope_sin_ciclos_vacio() {
        assert!(topes_desde_sondas(&[], &[rubrica_mixta()]).is_empty());
        let sin_ciclo = Hallazgo {
            severidad: 2,
            ..ciclo()
        };
        assert!(topes_desde_sondas(&[sin_ciclo], &[rubrica_mixta()]).is_empty());
    }

    #[test]
    fn juez_puntuar_completa_evidencia_y_fuente() {
        let c = caso();
        let texto = respuesta(
            json!([item_nivel(MODULARIDAD, 2, json!([real()]))]),
            json!([]),
        );
        let r = juzgar_con(&c, texto);
        let p = puntuar_con_juez(&c.rubricas[0], &r, &BTreeMap::new(), MODELO).expect("puntúa");
        let item = p
            .items
            .iter()
            .find(|i| i.item == MODULARIDAD)
            .expect("está");
        assert_eq!(item.evidencia, vec!["src/almacen.rs:4".to_string()]);
        assert_eq!(
            item.fuente,
            Fuente::Juez {
                modelo: MODELO.into()
            }
        );
        // Modularidad (peso 3, nivel 2) y tope sin nivel: (3·2) / (3·3) = 2/3.
        assert!((p.valor.expect("hay valor") - 2.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn juez_ciclo_de_la_sonda_real_topa_modularidad_en_uno() {
        let repo = tempfile::tempdir().expect("tempdir");
        fs::create_dir(repo.path().join("src")).expect("crea src");
        fs::write(
            repo.path().join("src/a.rs"),
            "use crate::b::algo;\npub fn f() {}\n",
        )
        .expect("escribe a");
        fs::write(
            repo.path().join("src/b.rs"),
            "pub fn algo() {}\nuse crate::a::f;\n",
        )
        .expect("escribe b");
        let sonda = crate::oracle::perfiles::rust::arquitectura_nativa(repo.path(), &[], &[]);
        assert!(
            sonda.hallazgos.iter().any(|h| h
                .detalle
                .starts_with(crate::oracle::perfiles::rust::REGLA_CICLO)),
            "la sonda nombra el ciclo con REGLA_CICLO: {:?}",
            sonda.hallazgos
        );
        let rubricas = [rubrica_mixta()];
        let topes = topes_desde_sondas(&sonda.hallazgos, &rubricas);
        assert_eq!(topes, BTreeMap::from([(MODULARIDAD.to_string(), 1)]));
        let r = ResultadoJuez {
            estado: EstadoJuez::Ok,
            niveles: BTreeMap::from([(MODULARIDAD.to_string(), 3)]),
            items: vec![],
            hallazgos: vec![],
            descartes: vec![],
            hallazgos_descartados: vec![],
            faltantes: vec![],
            extractos: vec![],
        };
        let p = puntuar_con_juez(&rubricas[0], &r, &topes, MODELO).expect("puntúa");
        let nivel = p
            .items
            .iter()
            .find(|i| i.item == MODULARIDAD)
            .expect("está");
        assert_eq!(nivel.nivel, 1);
    }

    #[test]
    fn juez_hay_pedidos_con_rubrica_solo_de_sondas_es_falso() {
        let solo_sondas = fixture("diseno-valida.toml")
            .replace("mide = \"juez\"", "mide = \"sonda\"")
            .replace("mide = \"mixto\"", "mide = \"sonda\"");
        let rubrica = Rubrica::desde_toml(&solo_sondas).expect("carga");
        assert!(!hay_pedidos(&[rubrica]));
        assert!(hay_pedidos(&[rubrica_valida()]));
        assert!(!hay_pedidos(&[]));
    }

    #[test]
    fn juez_puntuar_item_sin_respuesta_queda_en_faltantes_y_no_cuenta_como_cero() {
        let c = caso();
        // El juez responde solo modularidad (nivel 2); el tope de la rúbrica queda sin respuesta.
        let texto = respuesta(
            json!([item_nivel(MODULARIDAD, 2, json!([real()]))]),
            json!([]),
        );
        let r = juzgar_con(&c, texto);
        assert_eq!(r.faltantes, vec![TOPE.to_string()]);
        let p = puntuar_con_juez(&c.rubricas[0], &r, &BTreeMap::new(), MODELO).expect("puntúa");
        // Solo modularidad (peso 3, nivel 2): (3·2) / (3·3) = 2/3. El tope no entra ni como 0.
        assert!((p.valor.expect("hay valor") - 2.0 / 3.0).abs() < 1e-6);
        // `diseno.ciclos` es de sonda: el juez no lo responde, y `puntuar_con_juez` lo lista igual.
        assert_eq!(p.faltantes, vec![CICLOS.to_string(), TOPE.to_string()]);
        assert!(p.items.iter().all(|i| i.item != TOPE), "{:?}", p.items);
    }
}
