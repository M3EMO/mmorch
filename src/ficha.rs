//! Ficha del spec: el bloque TOML de `## Ficha` que Root mide, y su revisión.
//! Las reglas deterministas no usan LLM. El juez de ambigüedades vive en `juez`.

pub mod juez;

use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error as ErrorDerive;

use juez::{JUEZ_OK, SIN_VEREDICTO};
pub use juez::{JuezAmbiguedad, JuezProveedor, revisar_con_juez, revisar_con_tope};

/// Errores de la ficha. Salen con código 2: sin ficha válida no hay evaluación.
/// La excepción es `Juez`, un fallo de infraestructura: sale con 3.
#[derive(Debug, ErrorDerive)]
pub enum ErrorFicha {
    #[error("sin ficha: el spec no tiene la sección `## Ficha`")]
    SinFicha,
    #[error("la sección `## Ficha` no tiene bloque ```toml")]
    SinBloque,
    #[error("la sección `## Ficha` tiene más de un bloque ```toml")]
    VariosBloques,
    #[error("la ficha no parsea: {0}")]
    Toml(String),
    #[error("la ficha no tiene la estructura esperada: {0}")]
    Estructura(String),
    #[error("no se pudo leer {ruta}")]
    Leer {
        ruta: String,
        #[source]
        fuente: std::io::Error,
    },
    #[error("el juez falló: {0}")]
    Juez(String),
    #[error("no se pudo guardar la revisión en {ruta}")]
    Guardar {
        ruta: String,
        #[source]
        fuente: std::io::Error,
    },
}

impl ErrorFicha {
    /// Código de salida del proceso: 2 por error de uso, 3 por fallo del juez o de escritura.
    pub fn codigo_de_salida(&self) -> u8 {
        match self {
            ErrorFicha::Juez(_) | ErrorFicha::Guardar { .. } => 3,
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
    /// Runner de tests. Vacío equivale a `pytest`. Valores: `pytest`, `nextest`.
    pub runner: String,
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
/// `juez` es `"ok"` (el JSON parseó), `"sin_veredicto: <causa>"` o `"no pedido"` (con `--solo-deterministas`).
/// Sin juez, la causa es `juez no corrido`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Revision {
    pub hallazgos: Vec<HallazgoFicha>,
    pub ambiguedades: Vec<Ambiguedad>,
    pub juez: String,
    /// Frases que el juez citó y el gate de evidencia descartó.
    #[serde(default)]
    pub descartadas: Vec<String>,
}

impl Revision {
    /// Código de `ficha revisar`. Un hallazgo o una ambigüedad da 1, aunque el juez haya fallado.
    /// Sale 3 solo si el juez no dio veredicto y no hay hallazgos ni ambigüedades. 0 exige las dos capas limpias.
    pub fn codigo_de_salida(&self) -> u8 {
        if !self.hallazgos.is_empty() || !self.ambiguedades.is_empty() {
            1
        } else if self.juez.starts_with(SIN_VEREDICTO) {
            3
        } else {
            0
        }
    }
}

/// Carpeta de las revisiones guardadas del juez, dentro de `ROOT_HOME`. Nunca va al repo.
const CARPETA_FICHAS: &str = "fichas";

/// Sha256 en hexadecimal del texto del spec. Su revisión guardada se llama `<sha>.json`.
pub fn sha_del_spec(spec: &str) -> String {
    Sha256::digest(spec.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Ruta de la revisión guardada de `spec` dentro de `home`.
fn ruta_revision(home: &Path, spec: &str) -> PathBuf {
    home.join(CARPETA_FICHAS)
        .join(format!("{}.json", sha_del_spec(spec)))
}

/// Guarda la revisión del juez de `spec` en `<home>/fichas/<sha>.json`. La escritura es atómica:
/// un temporal en la misma carpeta y un `rename`. `home` va explícito; nunca se usa el repo.
pub fn guardar_revision(
    home: &Path,
    spec: &str,
    revision: &Revision,
) -> Result<PathBuf, ErrorFicha> {
    let ruta = ruta_revision(home, spec);
    let error = |fuente: io::Error| ErrorFicha::Guardar {
        ruta: ruta.display().to_string(),
        fuente,
    };
    let carpeta = ruta.parent().unwrap_or(home);
    fs::create_dir_all(carpeta).map_err(error)?;
    let texto = serde_json::to_string_pretty(revision).map_err(|e| error(io::Error::other(e)))?;
    let mut temporal = tempfile::NamedTempFile::new_in(carpeta).map_err(error)?;
    temporal.write_all(texto.as_bytes()).map_err(error)?;
    temporal.as_file().sync_all().map_err(error)?;
    temporal.persist(&ruta).map_err(|e| error(e.error))?;
    Ok(ruta)
}

/// Guarda la revisión solo si el juez dio veredicto. Sin juez, o con un juez sin veredicto, no escribe
/// nada y da `None`.
pub fn guardar_si_hay_juicio(
    home: &Path,
    spec: &str,
    revision: &Revision,
) -> Result<Option<PathBuf>, ErrorFicha> {
    if revision.juez != JUEZ_OK {
        return Ok(None);
    }
    guardar_revision(home, spec, revision).map(Some)
}

/// Ambigüedades de la revisión guardada de `spec`. `Ok(None)` si no hay revisión para ese texto, así que
/// un spec cambiado no hereda las de otro. `Err` con la causa si el archivo no se lee o no parsea: la
/// evaluación lo convierte en aviso.
pub fn ambiguedades_guardadas(home: &Path, spec: &str) -> Result<Option<Vec<Ambiguedad>>, String> {
    let ruta = ruta_revision(home, spec);
    let texto = match fs::read_to_string(&ruta) {
        Ok(texto) => texto,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    serde_json::from_str::<Revision>(&texto)
        .map(|revision| Some(revision.ambiguedades))
        .map_err(|e| e.to_string())
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
/// Causa del juez cuando no corrió. Sin juez no hay veredicto, así que `codigo_de_salida` da 3 sin hallazgos.
const JUEZ_NO_CORRIDO: &str = "sin_veredicto: juez no corrido";
/// Valor de `Revision.juez` cuando la revisión se pide con `--solo-deterministas`: el juez no se pidió,
/// así que no hay veredicto que guardar y el código sale de los hallazgos.
pub const JUEZ_NO_PEDIDO: &str = "no pedido";
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

/// Como `revisar`, pero el juez no se pidió (`--solo-deterministas`): `juez` vale `JUEZ_NO_PEDIDO`.
pub fn revisar_solo_deterministas(
    spec: &str,
    ctx: &ContextoRevision,
) -> Result<Revision, ErrorFicha> {
    let mut revision = revisar(spec, ctx)?;
    revision.juez = JUEZ_NO_PEDIDO.into();
    Ok(revision)
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
    // Vacío y "pytest" dan la salida de siempre. Solo `:3` trae cargo-nextest.
    let (version_imagen, contrato, oculto) = match ficha.tecnica.runner.trim() {
        "" | "pytest" => (1, CMD_CONTRATO.into(), CMD_OCULTO.into()),
        "nextest" => (3, cmd_nextest("contrato"), cmd_nextest("oculto")),
        otro => {
            return Err(ErrorFicha::Estructura(format!(
                "tecnica.runner desconocido: {otro}; usá \"pytest\" o \"nextest\""
            )));
        }
    };
    let borrador = Borrador {
        nombre: nombre.into(),
        lenguaje: lenguaje.into(),
        imagen: format!("localhost/root-oraculo-{lenguaje}:{version_imagen}"),
        spec: "bootstrap/docs/SPEC.md".into(),
        repeticiones: 5,
        repeticiones_inestable: 10,
        contrato: Cmd { cmd: contrato },
        oculto: Cmd { cmd: oculto },
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

/// Comando de nextest que copia el junit a `/salida/<xml>.xml`.
/// El código de nextest llega entero aunque el `cp` falle.
/// El `rm -f` evita copiar un junit viejo de otra corrida.
///
/// Usa el tool-config `/etc/root-oraculo/nextest.toml` que trae la imagen: ahí vive el perfil `ci`
/// y su store. El repo no pide el perfil `ci`; lo define la imagen.
fn cmd_nextest(xml: &str) -> String {
    format!(
        "rm -f /salida/target/nextest/ci/junit.xml; CARGO_TARGET_DIR=/salida/target cargo nextest run --offline --profile ci --tool-config-file root:/etc/root-oraculo/nextest.toml; s=$?; cp /salida/target/nextest/ci/junit.xml /salida/{xml}.xml 2>/dev/null; exit $s"
    )
}

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

#[cfg(test)]
mod tests {
    use super::*;

    const LIMPIA: &str = include_str!("../tests/fixtures/fichas/spec-completa.md");
    const VAGA: &str = include_str!("../tests/fixtures/fichas/spec-vaga.md");

    #[test]
    fn revision_juez_no_pedido_sin_hallazgos_sale_0() {
        let revision =
            revisar_solo_deterministas(LIMPIA, &ContextoRevision::default()).expect("revisa");
        assert_eq!(revision.juez, JUEZ_NO_PEDIDO);
        assert!(revision.hallazgos.is_empty());
        assert!(revision.ambiguedades.is_empty());
        assert_eq!(revision.codigo_de_salida(), 0);
    }

    #[test]
    fn revision_juez_no_pedido_con_hallazgos_sale_1() {
        let revision =
            revisar_solo_deterministas(VAGA, &ContextoRevision::default()).expect("revisa");
        assert_eq!(revision.juez, JUEZ_NO_PEDIDO);
        assert!(!revision.hallazgos.is_empty());
        assert_eq!(revision.codigo_de_salida(), 1);
    }

    #[test]
    fn nextest_borra_el_junit_viejo_antes_de_correr_cargo() {
        let cmd = cmd_nextest("contrato");
        let rm = "rm -f /salida/target/nextest/ci/junit.xml";
        let cargo = "cargo nextest run";
        let pos_rm = cmd.find(rm).expect("el comando trae el rm -f");
        let pos_cargo = cmd.find(cargo).expect("el comando corre cargo nextest");
        assert!(pos_rm < pos_cargo, "el rm -f va antes de cargo: {cmd}");
    }

    #[test]
    fn nextest_no_corta_con_el_cp_y_conserva_el_codigo() {
        let cmd = cmd_nextest("contrato");
        assert!(
            !cmd.contains("|| exit 1"),
            "el cp no corta el comando: {cmd}"
        );
        assert!(
            cmd.ends_with("exit $s"),
            "el comando termina con exit $s: {cmd}"
        );
    }

    /// Corre `cmd_nextest` con `/salida` dentro de un tempdir y un `cargo` falso delante en el PATH.
    fn correr_nextest(cargo_falso: &str) -> (tempfile::TempDir, std::process::ExitStatus) {
        let dir = tempfile::tempdir().expect("temp dir");
        let bin = dir.path().join("bin");
        fs::create_dir(&bin).expect("carpeta bin");
        let cargo = bin.join("cargo");
        fs::write(&cargo, cargo_falso).expect("cargo falso");
        fs::set_permissions(&cargo, std::os::unix::fs::PermissionsExt::from_mode(0o755))
            .expect("permisos del cargo falso");
        let salida = dir.path().join("salida");
        fs::create_dir(&salida).expect("carpeta salida");
        let comando = cmd_nextest("contrato").replace("/salida", &salida.display().to_string());
        let path = format!(
            "{}:{}",
            bin.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let status = std::process::Command::new("sh")
            .arg("-c")
            .arg(&comando)
            .env("PATH", path)
            .status()
            .expect("sh del host");
        (dir, status)
    }

    #[test]
    fn nextest_cargo_que_falla_sin_junit_sale_con_su_codigo() {
        let (_dir, status) = correr_nextest("#!/bin/sh\nexit 101\n");
        assert_eq!(status.code(), Some(101));
    }

    #[test]
    fn nextest_copia_el_junit_y_sale_0_si_cargo_pasa() {
        let cargo_ok = "#!/bin/sh\n\
            mkdir -p \"$CARGO_TARGET_DIR/nextest/ci\"\n\
            printf '<testsuites/>' > \"$CARGO_TARGET_DIR/nextest/ci/junit.xml\"\n\
            exit 0\n";
        let (dir, status) = correr_nextest(cargo_ok);
        assert_eq!(status.code(), Some(0));
        let copiado = dir.path().join("salida").join("contrato.xml");
        assert_eq!(
            fs::read_to_string(copiado).expect("junit copiado"),
            "<testsuites/>"
        );
    }
}
