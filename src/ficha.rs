//! Ficha del spec: el bloque TOML de `## Ficha` que Root mide, y su revisión.
//! Las reglas deterministas no usan LLM. El juez de ambigüedades vive en `juez`.

pub mod juez;

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error as ErrorDerive;

pub use juez::{JuezAmbiguedad, JuezProveedor, revisar_con_juez};

/// Errores de la ficha. Salen con código 2: sin ficha válida no hay evaluación.
/// La excepción es `Juez`, un fallo de infraestructura: sale con 3.
#[derive(Debug, ErrorDerive)]
pub enum ErrorFicha {
    #[error("el spec no tiene sección `## Ficha`")]
    SinFicha,
    #[error("la sección `## Ficha` no tiene bloque ```toml")]
    SinBloque,
    #[error("la sección `## Ficha` tiene más de un bloque ```toml")]
    VariosBloques,
    #[error("la ficha no parsea: {0}")]
    Toml(String),
    #[error("la ficha no tiene la estructura esperada: {0}")]
    Estructura(String),
    #[error("no se pudo leer {ruta}: {fuente}")]
    Leer {
        ruta: String,
        #[source]
        fuente: std::io::Error,
    },
    #[error("el juez falló: {0}")]
    Juez(String),
}

impl ErrorFicha {
    /// Código de salida del proceso: 2 por error de uso, 3 por fallo del juez.
    pub fn codigo_de_salida(&self) -> u8 {
        match self {
            ErrorFicha::Juez(_) => 3,
            _ => 2,
        }
    }
}

/// Bloque ```toml de la sección `## Ficha`. `linea_inicio` es la línea 1-based del primer contenido.
#[derive(Debug, Clone, PartialEq)]
pub struct BloqueFicha {
    pub linea_inicio: usize,
    pub texto: String,
}

/// Ficha completa. Las cuatro partes son obligatorias.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Ficha {
    pub producto: ProductoFicha,
    pub tecnica: TecnicaFicha,
    pub arquitectura: ArquitecturaFicha,
    pub diseno: DisenoFicha,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ProductoFicha {
    pub usuarios: Vec<String>,
    pub problema: String,
    pub fuera_de_alcance: Vec<String>,
    pub requisito: Vec<ReqFicha>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ReqFicha {
    pub id: String,
    pub titulo: String,
    pub peso: u32,
    pub aceptacion: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct TecnicaFicha {
    pub lenguaje: String,
    pub version_minima: String,
    pub dependencias_permitidas: Vec<String>,
    pub plataformas: Vec<String>,
    pub limite: Vec<LimiteFicha>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct LimiteFicha {
    pub id: String,
    pub metrica: String,
    pub minimo: Option<f64>,
    pub maximo: Option<f64>,
    pub unidad: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ArquitecturaFicha {
    pub capas: Vec<Vec<String>>,
    pub persistencia: String,
    pub concurrencia: String,
    pub interfaz: Vec<InterfazFicha>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct InterfazFicha {
    pub id: String,
    pub modulo: String,
    pub publica: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct DisenoFicha {
    pub errores: String,
    pub logs: String,
    pub configuracion: String,
    pub regla: Vec<ReglaFicha>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ReglaFicha {
    pub id: String,
    pub texto: String,
    pub item: String,
}

/// Un hallazgo de las reglas deterministas. `linea` es la línea del SPEC.md.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HallazgoFicha {
    pub regla: String,
    pub linea: usize,
    pub cita: String,
    pub detalle: String,
}

/// Una frase con dos lecturas posibles. La produce el juez y la acepta el gate de evidencia.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Ambiguedad {
    pub frase: String,
    pub linea: usize,
    pub lectura_a: String,
    pub lectura_b: String,
}

/// Resultado de revisar un spec. `ambiguedades` queda vacía hasta que corra el juez.
/// `juez` es `"no corrido"`, `"ok"` (el JSON parseó) o `"sin_veredicto: <causa>"`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Revision {
    pub hallazgos: Vec<HallazgoFicha>,
    pub ambiguedades: Vec<Ambiguedad>,
    pub juez: String,
    /// Frases que el juez citó y el gate de evidencia descartó.
    #[serde(default)]
    pub descartadas: Vec<String>,
}

/// Datos del repo que algunas reglas comparan. `None` apaga la regla.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ContextoRevision {
    /// Módulos del mapa del repo. Si es `None`, la regla `capas` no corre.
    pub modulos: Option<BTreeSet<String>>,
    /// Ítems de la rúbrica. Si es `None`, `regla_item` solo exige que `item` no esté vacío.
    pub items_rubrica: Option<BTreeSet<String>>,
}

const ENCABEZADO: &str = "## Ficha";
const JUEZ_NO_CORRIDO: &str = "no corrido";
const MARCAS_VAGAS: &[&str] = &["TBD", "etc.", "..."];
const PALABRAS_VAGAS: &[&str] = &[
    "rápido",
    "rápida",
    "adecuado",
    "adecuada",
    "robusto",
    "robusta",
    "escalable",
    "fácil",
    "suficiente",
];
/// Claves que nombrarían tests ocultos. Van solo en `tarea.toml`.
const CLAVES_OCULTAS: &[&str] = &["pruebas", "bordes", "escenarios"];

/// Lee el bloque TOML de `## Ficha`, con su línea de inicio.
pub fn extraer(spec: &str) -> Result<(Ficha, BloqueFicha), ErrorFicha> {
    let bloque = bloque_de_ficha(spec)?;
    valor_crudo(&bloque)?;
    let ficha: Ficha =
        toml::from_str(&bloque.texto).map_err(|e| ErrorFicha::Estructura(e.message().into()))?;
    Ok((ficha, bloque))
}

/// Revisa el spec con las reglas deterministas. La capa del juez no corre aquí.
pub fn revisar(spec: &str, ctx: &ContextoRevision) -> Result<Revision, ErrorFicha> {
    let (ficha, bloque) = extraer(spec)?;
    Ok(Revision {
        hallazgos: revisar_reglas(&ficha, &bloque, ctx),
        ambiguedades: Vec::new(),
        juez: JUEZ_NO_CORRIDO.into(),
        descartadas: Vec::new(),
    })
}

/// Aplica las reglas deterministas y devuelve los hallazgos ordenados por línea.
pub fn revisar_reglas(
    ficha: &Ficha,
    bloque: &BloqueFicha,
    ctx: &ContextoRevision,
) -> Vec<HallazgoFicha> {
    let mut r = Revisor {
        bloque,
        hallazgos: Vec::new(),
    };
    regla_partes(ficha, &mut r);
    regla_aceptacion(ficha, &mut r);
    regla_vaga(ficha, &mut r);
    regla_capas(ficha, ctx, &mut r);
    regla_item(ficha, ctx, &mut r);
    regla_limite_unidad(ficha, &mut r);
    regla_sin_pruebas(bloque, &mut r);
    let mut hallazgos = r.hallazgos;
    hallazgos.sort_by_key(|h| h.linea);
    hallazgos
}

/// Arma el texto de un `tarea.toml` borrador desde la ficha.
pub fn borrador_tarea(ficha: &Ficha, nombre: &str) -> Result<String, ErrorFicha> {
    let lenguaje = ficha.tecnica.lenguaje.trim();
    if lenguaje.is_empty() {
        return Err(ErrorFicha::Estructura("tecnica.lenguaje está vacío".into()));
    }
    let borrador = Borrador {
        nombre: nombre.into(),
        lenguaje: lenguaje.into(),
        imagen: format!("localhost/root-oraculo-{lenguaje}:1"),
        spec: "bootstrap/docs/SPEC.md".into(),
        repeticiones: 5,
        repeticiones_inestable: 10,
        contrato: Cmd {
            cmd: CMD_CONTRATO.into(),
        },
        oculto: Cmd {
            cmd: CMD_OCULTO.into(),
        },
        requisito: ficha.producto.requisito.iter().map(req_borrador).collect(),
        eficiencia: ficha.tecnica.limite.iter().find_map(|l| {
            l.minimo.map(|piso| Eficiencia {
                cmd: String::new(),
                piso,
            })
        }),
        arquitectura: Arq {
            capas: ficha.arquitectura.capas.clone(),
        },
    };
    toml::to_string(&borrador).map_err(|e| ErrorFicha::Toml(e.to_string()))
}

// Mismos comandos que scripts/instalar-oraculos.sh.
const CMD_CONTRATO: &str = "ORACULO_REPO=/repo CARGO_TARGET_DIR=/salida/target python3 -m pytest -q -p no:cacheprovider --junitxml=/salida/contrato.xml /oraculo/contrato";
const CMD_OCULTO: &str = "ORACULO_REPO=/repo CARGO_TARGET_DIR=/salida/target python3 -m pytest -q -p no:cacheprovider --junitxml=/salida/oculto.xml /oraculo/oculto";

#[derive(Serialize)]
struct Borrador {
    nombre: String,
    lenguaje: String,
    imagen: String,
    spec: String,
    repeticiones: u32,
    repeticiones_inestable: u32,
    contrato: Cmd,
    oculto: Cmd,
    requisito: Vec<ReqBorrador>,
    #[serde(skip_serializing_if = "Option::is_none")]
    eficiencia: Option<Eficiencia>,
    arquitectura: Arq,
}

#[derive(Serialize)]
struct Cmd {
    cmd: String,
}

#[derive(Serialize)]
struct ReqBorrador {
    id: String,
    titulo: String,
    seccion: String,
    peso: u32,
    pruebas: Vec<String>,
    bordes: Vec<String>,
    escenarios: Vec<String>,
    criterios: Vec<String>,
}

#[derive(Serialize)]
struct Eficiencia {
    cmd: String,
    piso: f64,
}

#[derive(Serialize)]
struct Arq {
    capas: Vec<Vec<String>>,
}

fn req_borrador(req: &ReqFicha) -> ReqBorrador {
    ReqBorrador {
        id: req.id.clone(),
        titulo: req.titulo.clone(),
        seccion: String::new(),
        peso: req.peso,
        pruebas: Vec::new(),
        bordes: Vec::new(),
        escenarios: Vec::new(),
        criterios: Vec::new(),
    }
}

/// Busca la sección `## Ficha` y su único bloque ```toml. Las líneas de código no cortan la sección.
fn bloque_de_ficha(spec: &str) -> Result<BloqueFicha, ErrorFicha> {
    let lineas: Vec<&str> = spec.lines().collect();
    let encabezado = lineas
        .iter()
        .position(|l| l.starts_with(ENCABEZADO))
        .ok_or(ErrorFicha::SinFicha)?;
    let base = encabezado + 1;
    let fin = fin_de_seccion(&lineas, base);
    let mut bloques = bloques_toml(&lineas[base..fin], base);
    if bloques.len() > 1 {
        return Err(ErrorFicha::VariosBloques);
    }
    bloques.pop().ok_or(ErrorFicha::SinBloque)
}

/// Índice de la línea que cierra la sección: el próximo `# ` o `## ` fuera de un bloque de código.
fn fin_de_seccion(lineas: &[&str], desde: usize) -> usize {
    let mut i = desde;
    while i < lineas.len() {
        if es_cerca(lineas[i]) {
            i = indice_de_cierre(lineas, i + 1) + 1;
            continue;
        }
        if lineas[i].starts_with("# ") || lineas[i].starts_with("## ") {
            return i;
        }
        i += 1;
    }
    lineas.len()
}

/// Bloques ```toml de la sección. `base` es el índice 0-based de su primera línea.
fn bloques_toml(seccion: &[&str], base: usize) -> Vec<BloqueFicha> {
    let mut bloques = Vec::new();
    let mut i = 0;
    while i < seccion.len() {
        if !es_cerca(seccion[i]) {
            i += 1;
            continue;
        }
        let cierre = indice_de_cierre(seccion, i + 1);
        if info_de_cerca(seccion[i]) == "toml" {
            bloques.push(BloqueFicha {
                linea_inicio: base + i + 2,
                texto: seccion[i + 1..cierre]
                    .iter()
                    .map(|l| format!("{l}\n"))
                    .collect(),
            });
        }
        i = cierre + 1;
    }
    bloques
}

fn es_cerca(linea: &str) -> bool {
    linea.trim_start().starts_with("```")
}

fn info_de_cerca(linea: &str) -> &str {
    linea.trim_start().trim_start_matches('`').trim()
}

/// Índice de la cerca de cierre desde `desde`, o el largo si no hay.
fn indice_de_cierre(lineas: &[&str], desde: usize) -> usize {
    lineas[desde.min(lineas.len())..]
        .iter()
        .position(|l| l.trim() == "```")
        .map_or(lineas.len(), |p| desde + p)
}

/// Parsea el bloque como TOML crudo. Un error da la línea relativa dentro del bloque.
fn valor_crudo(bloque: &BloqueFicha) -> Result<toml::Value, ErrorFicha> {
    toml::from_str(&bloque.texto).map_err(|e| {
        let linea = e
            .span()
            .and_then(|s| bloque.texto.get(..s.start))
            .map_or(1, |previo| previo.matches('\n').count() + 1);
        ErrorFicha::Toml(format!("línea {linea} del bloque: {}", e.message()))
    })
}

/// Acumula hallazgos y calcula las líneas del SPEC.md.
struct Revisor<'a> {
    bloque: &'a BloqueFicha,
    hallazgos: Vec<HallazgoFicha>,
}

impl Revisor<'_> {
    fn push(&mut self, regla: &str, cita: &str, linea: usize, detalle: String) {
        self.hallazgos.push(HallazgoFicha {
            regla: regla.into(),
            linea,
            cita: cita.into(),
            detalle,
        });
    }

    /// Línea del SPEC.md de la primera búsqueda que aparece. Sin coincidencia, la del bloque.
    fn linea_de(&self, buscados: &[&str]) -> usize {
        for buscado in buscados.iter().filter(|b| !b.is_empty()) {
            if let Some(i) = self.bloque.texto.lines().position(|l| l.contains(buscado)) {
                return self.bloque.linea_inicio + i;
            }
        }
        self.bloque.linea_inicio
    }

    /// Línea de la clave `clave` dentro del requisito `id`, a partir de su línea `id = "..."`.
    fn linea_de_clave(&self, id: &str, clave: &str) -> usize {
        let lineas: Vec<&str> = self.bloque.texto.lines().collect();
        let quoted = comillas(id);
        let Some(inicio) = lineas.iter().position(|l| l.contains(&quoted)) else {
            return self.bloque.linea_inicio;
        };
        let rel = lineas[inicio..]
            .iter()
            .position(|l| l.trim_start().starts_with(clave))
            .unwrap_or(0);
        self.bloque.linea_inicio + inicio + rel
    }

    fn linea_de_id(&self, id: &str) -> usize {
        self.linea_de(&[comillas(id).as_str()])
    }
}

fn comillas(texto: &str) -> String {
    format!("\"{texto}\"")
}

/// Regla `partes`: un producto sin requisitos no sirve para medir.
fn regla_partes(ficha: &Ficha, r: &mut Revisor) {
    if ficha.producto.requisito.is_empty() {
        let linea = r.linea_de(&["[producto]"]);
        r.push(
            "partes",
            "producto.requisito",
            linea,
            "el producto no tiene requisitos".into(),
        );
    }
}

/// Regla `aceptacion`: cada requisito trae una aceptación medible.
fn regla_aceptacion(ficha: &Ficha, r: &mut Revisor) {
    for req in &ficha.producto.requisito {
        let acept = req.aceptacion.trim();
        if acept.is_empty() {
            let linea = r.linea_de_id(&req.id);
            r.push(
                "aceptacion",
                &req.id,
                linea,
                "el requisito no tiene aceptación".into(),
            );
        } else if !aceptacion_medible(acept) {
            let linea = r.linea_de(&[acept]);
            r.push(
                "aceptacion",
                acept,
                linea,
                "la aceptación no tiene número, comando, salida exacta ni id de test".into(),
            );
        }
    }
}

/// Verdadero si la aceptación trae un número, un comando, una salida exacta o un id de test.
fn aceptacion_medible(texto: &str) -> bool {
    texto.chars().any(|c| c.is_ascii_digit())
        || entre_marcas(texto, '`')
        || entre_marcas(texto, '"')
        || texto.contains("test_")
        || texto.contains("::")
}

/// Verdadero si hay un par de `marca` con texto no vacío entre ellos.
fn entre_marcas(texto: &str, marca: char) -> bool {
    let partes: Vec<&str> = texto.split(marca).collect();
    partes
        .iter()
        .enumerate()
        .any(|(i, p)| i % 2 == 1 && i + 1 < partes.len() && !p.trim().is_empty())
}

/// Regla `vaga`: ninguna cadena descriptiva trae TBD, etc., ... ni palabras vagas sin número.
fn regla_vaga(ficha: &Ficha, r: &mut Revisor) {
    for cadena in cadenas_descriptivas(ficha) {
        for cita in vaguedades(cadena) {
            let linea = r.linea_de(&[cadena, cita]);
            r.push("vaga", cita, linea, format!("la frase es vaga: {cita}"));
        }
    }
}

/// Textos libres de la ficha que la regla `vaga` revisa.
fn cadenas_descriptivas(ficha: &Ficha) -> Vec<&str> {
    let producto = &ficha.producto;
    let diseno = &ficha.diseno;
    let arq = &ficha.arquitectura;
    let mut cadenas: Vec<&str> = vec![
        producto.problema.as_str(),
        arq.persistencia.as_str(),
        arq.concurrencia.as_str(),
        diseno.errores.as_str(),
        diseno.logs.as_str(),
        diseno.configuracion.as_str(),
    ];
    cadenas.extend(producto.usuarios.iter().map(String::as_str));
    for req in &producto.requisito {
        cadenas.push(req.titulo.as_str());
        cadenas.push(req.aceptacion.as_str());
    }
    cadenas.extend(diseno.regla.iter().map(|r| r.texto.as_str()));
    cadenas.retain(|c| !c.is_empty());
    cadenas
}

/// Marcas y palabras vagas de una cadena. Las palabras no cuentan si la cadena trae un dígito.
fn vaguedades(cadena: &str) -> Vec<&'static str> {
    let mut encontradas: Vec<&'static str> = MARCAS_VAGAS
        .iter()
        .copied()
        .filter(|m| cadena.contains(m))
        .collect();
    if cadena.chars().any(|c| c.is_ascii_digit()) {
        return encontradas;
    }
    let palabras: BTreeSet<String> = cadena
        .split(|c: char| !c.is_alphanumeric())
        .map(str::to_lowercase)
        .collect();
    encontradas.extend(
        PALABRAS_VAGAS
            .iter()
            .copied()
            .filter(|p| palabras.contains(*p)),
    );
    encontradas
}

/// Regla `capas`: cada capa existe como módulo, si el contexto trae el mapa.
fn regla_capas(ficha: &Ficha, ctx: &ContextoRevision, r: &mut Revisor) {
    let Some(modulos) = &ctx.modulos else {
        return;
    };
    for capa in ficha.arquitectura.capas.iter().flatten() {
        if !modulos.contains(capa) {
            let linea = r.linea_de(&[comillas(capa).as_str()]);
            r.push(
                "capas",
                capa,
                linea,
                format!("la capa {capa} no existe como módulo"),
            );
        }
    }
}

/// Regla `regla_item`: cada regla apunta a un ítem. Con rúbrica, el ítem debe existir en ella.
fn regla_item(ficha: &Ficha, ctx: &ContextoRevision, r: &mut Revisor) {
    for regla in &ficha.diseno.regla {
        let item = regla.item.trim();
        if item.is_empty() {
            let linea = r.linea_de(&[regla.texto.as_str()]);
            r.push(
                "regla_item",
                &regla.id,
                linea,
                "la regla no apunta a ningún ítem".into(),
            );
        } else if ctx
            .items_rubrica
            .as_ref()
            .is_some_and(|i| !i.contains(item))
        {
            let linea = r.linea_de(&[regla.texto.as_str()]);
            r.push(
                "regla_item",
                item,
                linea,
                format!("el ítem {item} no existe en la rúbrica"),
            );
        }
    }
}

/// Regla `limite_unidad`: cada límite de la técnica trae su unidad.
fn regla_limite_unidad(ficha: &Ficha, r: &mut Revisor) {
    for limite in &ficha.tecnica.limite {
        if limite.unidad.trim().is_empty() {
            let linea = r.linea_de_id(&limite.id);
            r.push(
                "limite_unidad",
                &limite.id,
                linea,
                "el límite no tiene unidad".into(),
            );
        }
    }
}

/// Regla `sin_pruebas`: la ficha no nombra tests ocultos. Lee el TOML crudo.
fn regla_sin_pruebas(bloque: &BloqueFicha, r: &mut Revisor) {
    let Ok(valor) = valor_crudo(bloque) else {
        return;
    };
    let requisitos = valor
        .get("producto")
        .and_then(|p| p.get("requisito"))
        .and_then(toml::Value::as_array);
    for req in requisitos.into_iter().flatten() {
        let Some(tabla) = req.as_table() else {
            continue;
        };
        let id = tabla
            .get("id")
            .and_then(toml::Value::as_str)
            .unwrap_or_default();
        if let Some(clave) = CLAVES_OCULTAS.iter().find(|c| tabla.contains_key(**c)) {
            let linea = r.linea_de_clave(id, clave);
            r.push(
                "sin_pruebas",
                id,
                linea,
                format!("la ficha no nombra tests ocultos: `{clave}` en {id}"),
            );
        }
    }
}
