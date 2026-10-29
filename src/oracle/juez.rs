//! Juez de diseño y arquitectura (Tarea 10). Puntúa los ítems `juez` y `mixto` de las rúbricas.
//! Refuta por default: un nivel solo sube con evidencia que pasa el gate. Su JSON es estricto.
//! Las sondas fijan los ítems `sonda`. Una sonda puede poner un tope a un ítem `mixto`.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::ErrorOraculo;
use super::evidencia::{Verificacion, verificar_hallazgo};
use super::mapa::Mapa;
use super::perfiles::crear_hallazgo;
use super::perfiles::rust::REGLA_CICLO;
use super::rubrica::{ErrorRubrica, ItemRubrica, Mide, Puntuacion, Rubrica};
use super::tarea::Requisito;
use crate::proveedores::{self, Prompt, RespuestaJuez};
use crate::tipos::{Dimension, Eleccion, Fuente, Hallazgo, NivelItem, Via};

/// Ítem de la rúbrica de modularidad. Su tope lo pone un ciclo de módulos.
const MODULARIDAD: &str = "diseno.modularidad";
/// Id de la sonda de arquitectura. Su hallazgo de ciclo fija el tope de `diseno.modularidad`.
const SONDA_ARQUITECTURA: &str = "arquitectura-nativa";
/// Fase del flujo que va al `Evento` de la llamada.
const FASE: &str = "juez-diseno";
/// Marca que se agrega al final de un extracto recortado.
const MARCA_RECORTE: &str = "…[recortado]";
const NIVEL_MAX: i64 = 3;
const SEVERIDAD_MAX: i64 = 3;
/// Cuántos archivos grandes del mapa van en los extractos.
const ARCHIVOS_DESTACADOS: usize = 5;

/// Prompt fijo del juez. Refuta por default y pide solo un objeto JSON.
const PROMPT_SISTEMA: &str = "Sos un PM técnico que revisa el diseño y la arquitectura de un repo contra su spec.
REFUTA POR DEFAULT: el nivel de cada ítem parte en 0 y solo sube con evidencia citada textual.
No revisás estilo, nombres ni formato. Solo revisás si el diseño sirve a la spec.
Respondé estas tres preguntas con evidencia:
- ¿Qué requisito queda difícil de extender o de mantener con esta estructura?
- ¿Qué decisión de diseño es un riesgo para producción, como un estado global, un bloqueo o un error que se traga?
- ¿Qué falta para operar el producto: logs, configuración, manejo de errores?
Cada evidencia tiene archivo, linea y cita. La cita es texto textual del archivo, con los mismos caracteres.
Cada ítem trae evidencia que cumple el ancla de su nivel. Sin evidencia para un ítem, no lo incluyas en items.
Un hallazgo exige impacto con ids de requisitos reales. Sin requisito afectado, no reportes el hallazgo.
Respondé SOLO un objeto JSON con esta forma exacta:
{\"items\":[{\"item\":\"<id>\",\"nivel\":0-3,\"evidencia\":[{\"archivo\":\"...\",\"linea\":N,\"cita\":\"<texto textual del archivo>\"}]}],\"hallazgos\":[{\"item\":\"<id>\",\"severidad\":1-3,\"archivo\":\"...\",\"linea\":N,\"cita\":\"...\",\"detalle\":\"...\",\"impacto\":[\"R1\"]}]}
Sin hallazgos: \"hallazgos\":[]. Sin ítems con evidencia: \"items\":[].";

/// Un modelo que responde al prompt del juez.
pub trait ModeloJuez {
    fn consultar(&self, prompt: &Prompt) -> Result<RespuestaJuez, ErrorOraculo>;
}

/// Juez real: llama al proveedor de `eleccion`. Solo acepta una elección que razona y usa la API.
pub struct JuezProveedor {
    pub eleccion: Eleccion,
    pub corrida: String,
}

impl JuezProveedor {
    /// Rechaza una elección sin razonamiento, sin API o sin modelo: el verificador razona y refuta.
    /// Es un error de configuración (`Eleccion`), no de infraestructura.
    pub fn nuevo(eleccion: Eleccion, corrida: String) -> Result<Self, ErrorOraculo> {
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
        Ok(Self { eleccion, corrida })
    }
}

impl ModeloJuez for JuezProveedor {
    fn consultar(&self, prompt: &Prompt) -> Result<RespuestaJuez, ErrorOraculo> {
        proveedores::llamar(&self.eleccion, prompt, &self.corrida)
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
}

/// Arma el prompt del juez. Es determinista: la misma entrada da el mismo prompt.
pub fn construir_prompt(e: &EntradaJuez, limites: &LimitesPrompt) -> Prompt {
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
    usuario.push_str("\nÍTEMS A PUNTUAR:\n");
    for (item, _) in pedidos_de(e.rubricas) {
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
    usuario.push_str("\nEXTRACTOS:\n");
    for (archivo, texto) in extractos(e, limites) {
        usuario.push_str(&format!("--- {archivo} ---\n{texto}"));
    }
    Prompt {
        sistema: Some(PROMPT_SISTEMA.into()),
        usuario,
        max_tokens: limites.max_tokens,
        fase: FASE.into(),
    }
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

/// Extractos de los archivos grandes del mapa y de los que citan las sondas, en ese orden.
/// Respeta `extracto_max` por archivo y `extractos_total` en conjunto.
fn extractos(e: &EntradaJuez, limites: &LimitesPrompt) -> Vec<(String, String)> {
    let mut archivos = e.mapa.archivos_destacados(ARCHIVOS_DESTACADOS);
    for h in e.hallazgos_sondas {
        if !archivos.contains(&h.archivo) {
            archivos.push(h.archivo.clone());
        }
    }
    let mut restante = limites.extractos_total;
    let mut salida = Vec::new();
    for archivo in archivos {
        if restante == 0 {
            break;
        }
        if Path::new(&archivo).is_absolute() {
            continue;
        }
        let Ok(contenido) = fs::read_to_string(e.repo.join(&archivo)) else {
            continue;
        };
        let texto = numerar(&contenido, restante.min(limites.extracto_max));
        if texto.is_empty() {
            continue;
        }
        restante = restante.saturating_sub(texto.chars().count());
        salida.push((archivo, texto));
    }
    salida
}

/// Líneas con su número real (`NN: `). El texto, con la marca de recorte, no pasa `limite` caracteres.
/// Vacío si ni una línea cabe: así no sale un archivo con solo la marca.
fn numerar(contenido: &str, limite: usize) -> String {
    let marca = format!("{MARCA_RECORTE}\n");
    let tope = limite.saturating_sub(marca.chars().count());
    let mut salida = String::new();
    let mut usados = 0;
    for (i, linea) in contenido.lines().enumerate() {
        let fila = format!("{}: {linea}\n", i + 1);
        let largo = fila.chars().count();
        if usados + largo > tope {
            if salida.is_empty() {
                return String::new();
            }
            salida.push_str(&marca);
            break;
        }
        salida.push_str(&fila);
        usados += largo;
    }
    salida
}

/// Pide el juicio al modelo y aplica el parseo estricto y el gate.
pub fn juzgar(
    e: &EntradaJuez,
    modelo: &dyn ModeloJuez,
    nombre_modelo: &str,
    limites: &LimitesPrompt,
) -> ResultadoJuez {
    let pedidos: BTreeMap<String, Dimension> = pedidos_de(e.rubricas)
        .into_iter()
        .map(|(item, dimension)| (item.id.clone(), dimension))
        .collect();
    let catalogo: BTreeMap<String, Mide> = e
        .rubricas
        .iter()
        .flat_map(|r| r.item.iter())
        .map(|item| (item.id.clone(), item.mide))
        .collect();
    let prompt = construir_prompt(e, limites);
    let tope = prompt.max_tokens;
    let salida = match modelo.consultar(&prompt) {
        Ok(salida) => salida,
        Err(err) => return sin_veredicto(format!("el modelo falló: {err}"), &pedidos),
    };
    // Antes de parsear: una respuesta cortada por el tope no es JSON roto, es truncada.
    if salida.truncada {
        return sin_veredicto(proveedores::causa_tope_alcanzado(tope), &pedidos);
    }
    let respuesta = match parsear(&salida.texto) {
        Ok(respuesta) => respuesta,
        Err(causa) => return sin_veredicto(causa, &pedidos),
    };
    let cx = Contexto {
        repo: e.repo,
        requisitos: e.requisitos,
        pedidos: &pedidos,
        catalogo: &catalogo,
        modelo: nombre_modelo,
    };
    let mut niveles = BTreeMap::new();
    let mut items = Vec::new();
    let mut hallazgos = Vec::new();
    let mut descartes = Vec::new();
    for valor in respuesta.items {
        match procesar_item(valor, &cx) {
            Ok(nivel) if niveles.contains_key(&nivel.item) => descartes.push(descarte(
                format!("ítem {}", nivel.item),
                "ítem repetido; vale la primera respuesta",
            )),
            Ok(nivel) => {
                niveles.insert(nivel.item.clone(), nivel.nivel);
                items.push(nivel);
            }
            Err(d) => descartes.push(d),
        }
    }
    for valor in respuesta.hallazgos {
        match procesar_hallazgo(valor, &cx) {
            Ok(hallazgo) => hallazgos.push(hallazgo),
            Err(d) => descartes.push(d),
        }
    }
    let faltantes = pedidos
        .keys()
        .filter(|id| !niveles.contains_key(*id))
        .cloned()
        .collect();
    ResultadoJuez {
        estado: EstadoJuez::Ok,
        niveles,
        items,
        hallazgos,
        descartes,
        faltantes,
    }
}

/// Resultado sin veredicto: sin niveles ni hallazgos, y todo lo pedido queda como faltante.
fn sin_veredicto(causa: String, pedidos: &BTreeMap<String, Dimension>) -> ResultadoJuez {
    ResultadoJuez {
        estado: EstadoJuez::SinVeredicto(causa),
        niveles: BTreeMap::new(),
        items: Vec::new(),
        hallazgos: Vec::new(),
        descartes: Vec::new(),
        faltantes: pedidos.keys().cloned().collect(),
    }
}

/// Lo que los procesadores de cada elemento necesitan para validarlo.
struct Contexto<'a> {
    repo: &'a Path,
    requisitos: &'a [Requisito],
    pedidos: &'a BTreeMap<String, Dimension>,
    catalogo: &'a BTreeMap<String, Mide>,
    modelo: &'a str,
}

impl Contexto<'_> {
    /// Dimensión de un ítem que el juez puede puntuar. Un ítem de sonda o inexistente da el motivo.
    fn dimension(&self, id: &str) -> Result<Dimension, String> {
        match self.catalogo.get(id) {
            None => Err(format!("el ítem {id} no existe en las rúbricas pedidas")),
            Some(Mide::Sonda) => Err("el juez no puntúa ítems de sonda".into()),
            Some(_) => self
                .pedidos
                .get(id)
                .copied()
                .ok_or_else(|| format!("el ítem {id} no tiene dimensión conocida")),
        }
    }

    fn fuente(&self) -> Fuente {
        Fuente::Juez {
            modelo: self.modelo.into(),
        }
    }
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
    #[serde(default)]
    nivel: Option<i64>,
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
fn procesar_item(valor: Value, cx: &Contexto) -> Result<NivelItem, Descarte> {
    let cruda: ItemCrudo =
        serde_json::from_value(valor).map_err(|e| descarte("ítem", format!("mal formado: {e}")))?;
    let que = format!("ítem {}", cruda.item);
    let dimension = cx.dimension(&cruda.item).map_err(|m| descarte(&que, m))?;
    let Some(nivel) = cruda.nivel else {
        return Err(descarte(&que, "sin nivel"));
    };
    if !(0..=NIVEL_MAX).contains(&nivel) {
        return Err(descarte(&que, format!("nivel {nivel} fuera de 0..=3")));
    }
    let validas: Vec<String> = cruda
        .evidencia
        .into_iter()
        .filter_map(|v| serde_json::from_value::<EvidenciaCruda>(v).ok())
        .filter_map(|ev| evidencia_valida(ev, dimension, cx))
        .collect();
    if validas.is_empty() {
        return Err(descarte(&que, "sin evidencia que pase el gate"));
    }
    Ok(NivelItem {
        item: cruda.item,
        nivel: nivel as u8,
        evidencia: validas,
        fuente: cx.fuente(),
    })
}

/// Devuelve `archivo:linea` si la evidencia pasa el gate de evidencia.
fn evidencia_valida(ev: EvidenciaCruda, dimension: Dimension, cx: &Contexto) -> Option<String> {
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
    let valida = matches!(verificar_hallazgo(cx.repo, &h), Verificacion::Valido);
    valida.then(|| format!("{}:{}", h.archivo, h.linea))
}

/// Valida un hallazgo: requisito real, severidad 1 a 3, cita en el repo.
fn procesar_hallazgo(valor: Value, cx: &Contexto) -> Result<Hallazgo, Descarte> {
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
        return Err(descarte(&que, "sin requisito afectado"));
    }
    if let Some(id) = cruda
        .impacto
        .iter()
        .find(|id| !cx.requisitos.iter().any(|r| r.id == **id))
    {
        return Err(descarte(
            &que,
            format!("sin requisito afectado: {id} no existe"),
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
        Verificacion::Descartado(motivo) => Err(descarte(
            &que,
            format!("la cita no pasa el gate: {motivo:?}"),
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

/// Completa con nivel 0 los ítems de `rubrica` que el juez no respondió. Refuta por default: lo que
/// no tiene evidencia no sube el puntaje, pero tampoco lo deja fuera del promedio. Sin `Ok`, no cambia.
pub fn con_faltantes_en_cero(r: &ResultadoJuez, rubrica: &Rubrica, modelo: &str) -> ResultadoJuez {
    let mut completo = r.clone();
    if completo.estado != EstadoJuez::Ok {
        return completo;
    }
    for id in r.faltantes.iter().filter(|id| es_item_de(rubrica, id)) {
        completo.niveles.insert(id.clone(), 0);
        completo.items.push(NivelItem {
            item: id.clone(),
            nivel: 0,
            evidencia: Vec::new(),
            fuente: Fuente::Juez {
                modelo: modelo.into(),
            },
        });
    }
    completo
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
        let visto = modelo.visto.borrow();
        assert_eq!(visto.len(), 1);
        assert_eq!(visto[0], construir_prompt(&c.entrada(), &limites));
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
            faltantes: vec![],
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
            faltantes: vec![],
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
    fn juez_faltantes_en_cero_cuentan_en_el_promedio_sin_evidencia() {
        let c = caso();
        // El juez responde solo modularidad (nivel 2); el tope de la rúbrica queda sin respuesta.
        let texto = respuesta(
            json!([item_nivel(MODULARIDAD, 2, json!([real()]))]),
            json!([]),
        );
        let r = juzgar_con(&c, texto);
        assert_eq!(r.faltantes, vec![TOPE.to_string()]);
        let completo = con_faltantes_en_cero(&r, &c.rubricas[0], MODELO);
        let p =
            puntuar_con_juez(&c.rubricas[0], &completo, &BTreeMap::new(), MODELO).expect("puntúa");
        // (3·2 + 1·0) / (3·4) = 0.5: el ítem sin respuesta pesa cero, no sale del promedio.
        assert!((p.valor.expect("hay valor") - 0.5).abs() < 1e-6);
        let tope = p.items.iter().find(|i| i.item == TOPE).expect("está");
        assert_eq!(tope.nivel, 0);
        assert!(tope.evidencia.is_empty());
        assert_eq!(
            tope.fuente,
            Fuente::Juez {
                modelo: MODELO.into()
            }
        );
    }

    #[test]
    fn juez_faltantes_en_cero_no_cambia_un_juicio_sin_veredicto() {
        let c = caso();
        let r = ResultadoJuez {
            estado: EstadoJuez::SinVeredicto("JSON inválido".into()),
            niveles: BTreeMap::new(),
            items: Vec::new(),
            hallazgos: Vec::new(),
            descartes: Vec::new(),
            faltantes: vec![MODULARIDAD.into(), TOPE.into()],
        };
        assert_eq!(con_faltantes_en_cero(&r, &c.rubricas[0], MODELO), r);
    }
}
