//! Descarga de Python (Tarea 6c-B, decisión 21). Corre solo `pip download`, `curl` y `git`, con red.
//! Nunca corre `pip install`, `pip wheel` ni `setup.py`. Un sdist se baja y se guarda sin extraerlo.
//! Dentro de cada contenedor, `/cache` es la carpeta `<hash>.parcial` del requirements.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Component, Path};
use std::time::Duration;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::credenciales::{Credencial, netrc_de_pip, redactar_secretos};
use super::inventario::{
    PartesGit, RequisitosPip, especificador_pep508_estricto, motivo_git, partes_git, redactar,
    requisitos_pip, textos_de_requisitos,
};
use super::{
    ARCHIVO_REQUIREMENTS, CARPETA_CACHE, CARPETA_PYTHON, Diagnostico, PLAZO_CORTO, Paso,
    Preparacion, Registro, borrar_en_home, corte, diagnosticar, entorno_descarga, error_de,
    hallazgo, hash_corto, limpiar_parcial, parcial_de, temporal_de, truncar_linea,
};
use crate::borrado;
use crate::config::RegistroConfig;
use crate::oracle::contenedor::{self, Ejecucion, Resultado};
use crate::oracle::entrega::{Podman, PodmanReal};
use crate::oracle::tarea::Tarea;
use crate::oracle::{ErrorOraculo, infra};
use crate::tipos::Hallazgo;

/// Archivo con las líneas que `pip download` baja como wheel. Sin opciones ni URLs.
const ARCHIVO_INDICE: &str = "indice-pip.txt";
/// Lo que la evaluación instala sin red. Las URLs y los `git+` se cambian por rutas locales.
pub const ARCHIVO_OFFLINE: &str = "requirements-offline.txt";
/// Restricciones (`-c`) del repo, sin fuentes: pip las usa para elegir versiones y no instala nada con ellas.
const ARCHIVO_RESTRICCIONES: &str = "restricciones.txt";
/// Mensaje de PyPI cuando una versión no trae sdist: la plataforma no tiene wheel y no hay de dónde construir.
const SIN_SDIST: &str = "la versión no tiene sdist en PyPI";
const INDICE_PYPI: &str = "https://pypi.org/simple";
const JSON_PYPI: &str = "https://pypi.org/pypi";
const PREFIJO_SDIST_PYPI: &str = "https://files.pythonhosted.org/";
const ARCHIVO_JSON: &str = "pypi-meta.json";
/// Plazo de cada `curl` de un archivo de red.
const PLAZO_CURL_S: u32 = 60;
/// Archivo con los requisitos de build que `pip download` baja como wheels en `parcial`.
const ARCHIVO_BUILD: &str = "build-requires.txt";
/// Requisitos de build sin `pyproject.toml`: el respaldo de PEP 517.
const REQUISITOS_BUILD_RESPALDO: [&str; 2] = ["setuptools", "wheel"];
const ARCHIVO_PYPROJECT: &str = "pyproject.toml";
/// Tope de tamaño del `pyproject.toml` de un clon. Un archivo más grande no se lee.
const TOPE_PYPROJECT_CLON: u64 = 256 * 1024;
/// `netrc` de pip dentro de la carpeta de trabajo de la descarga. Lleva las claves de los registros que
/// coinciden con un índice; se borra al terminar la descarga.
const ARCHIVO_NETRC: &str = "pip.netrc";
/// Ruta de `ARCHIVO_NETRC` dentro del contenedor, donde `trabajo` está montada en solo lectura.
const RUTA_NETRC_CONTENEDOR: &str = "/trabajo/pip.netrc";
/// Requisitos de build de cada sdist de la caché: nombre del archivo → `[build-system].requires` o `null`.
/// Lo escribe el contenedor de descarga; el host no abre ningún sdist.
pub const ARCHIVO_REQUISITOS_SDIST: &str = "requisitos-sdist.json";
/// Dependencias de runtime de cada sdist de la caché: nombre del archivo → `Requires-Dist` o `null`.
/// Lo escribe el mismo contenedor, junto a `ARCHIVO_REQUISITOS_SDIST`.
pub const ARCHIVO_DEPENDENCIAS_SDIST: &str = "dependencias-sdist.json";
/// Nombres de los sdists con metadatos anteriores a 2.1 (`Metadata-Version`). Esas versiones no traen
/// `Requires-Dist`, así que sus dependencias no se verifican. Lo escribe el mismo contenedor.
pub const ARCHIVO_METADATA_VIEJA_SDIST: &str = "metadata-vieja-sdist.json";
/// Script del contenedor de descarga: lee `[build-system].requires` de cada sdist de `/cache` con
/// `tarfile` o `zipfile`, sin extraer nada, y deja sus JSON en `/cache`: requisitos, dependencias y metadata
/// vieja. Sin `tomllib` (Python < 3.11) sale
/// con código 3: el host falla cerrado.
const GUION_REQUISITOS_SDIST: &str = r#"import glob, json, os, sys, tarfile, zipfile
try:
    import tomllib
except ImportError:
    sys.exit(3)

def primer_nivel(nombre, archivo):
    partes = nombre.split("/")
    return len(partes) == 2 and partes[1] == archivo and partes[0] not in ("", ".", "..") and not partes[0].startswith("-")

def archivo_de(ruta, archivo):
    if ruta.endswith(".zip"):
        with zipfile.ZipFile(ruta) as z:
            nombres = [n for n in z.namelist() if primer_nivel(n, archivo)]
            return z.read(nombres[0]) if nombres else None
    with tarfile.open(ruta, "r:*") as t:
        for m in t.getmembers():
            if m.isfile() and primer_nivel(m.name, archivo):
                f = t.extractfile(m)
                return f.read() if f else None
    return None

def requires(ruta):
    try:
        texto = archivo_de(ruta, "pyproject.toml")
        if texto is None:
            return None
        lista = tomllib.loads(texto.decode("utf-8")).get("build-system", {}).get("requires")
        if not isinstance(lista, list) or not all(isinstance(s, str) for s in lista):
            return None
        return lista
    except Exception:
        return None

def opcional(valor):
    marcador = valor.partition(";")[2]
    return "extra==" in marcador.replace(" ", "")

def dependencias(ruta):
    try:
        texto = archivo_de(ruta, "PKG-INFO")
        if texto is None:
            return None
        deps = []
        for linea in texto.decode("utf-8", "replace").split("\n"):
            linea = linea.rstrip("\r")
            if linea == "":
                break
            clave, _, valor = linea.partition(":")
            clave = clave.strip()
            valor = valor.strip()
            if clave == "Dynamic" and valor == "Requires-Dist":
                return None
            if clave == "Requires-Dist" and valor and not opcional(valor):
                deps.append(valor)
        return deps
    except Exception:
        return None

def metadata_vieja(ruta):
    try:
        texto = archivo_de(ruta, "PKG-INFO")
        if texto is None:
            return False
        for linea in texto.decode("utf-8", "replace").split("\n"):
            linea = linea.rstrip("\r")
            if linea == "":
                break
            clave, _, valor = linea.partition(":")
            if clave.strip() == "Metadata-Version":
                mayor, _, menor = valor.strip().partition(".")
                if not (mayor.isdecimal() and menor.isdecimal()):
                    return False
                return (int(mayor), int(menor)) < (2, 1)
        return False
    except Exception:
        return False

requisitos = {}
dependencias_de_cada = {}
viejas = []
for ruta in sorted(glob.glob("/cache/*.tar.gz") + glob.glob("/cache/*.zip")):
    nombre = os.path.basename(ruta)
    requisitos[nombre] = requires(ruta)
    dependencias_de_cada[nombre] = dependencias(ruta)
    if metadata_vieja(ruta):
        viejas.append(nombre)
with open("/cache/requisitos-sdist.json", "w", encoding="utf-8") as f:
    json.dump(requisitos, f)
with open("/cache/dependencias-sdist.json", "w", encoding="utf-8") as f:
    json.dump(dependencias_de_cada, f)
with open("/cache/metadata-vieja-sdist.json", "w", encoding="utf-8") as f:
    json.dump(viejas, f)
"#;

/// Sdist elegido desde PyPI: su URL, su nombre de archivo y su sha256 esperado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sdist {
    pub url: String,
    pub archivo: String,
    pub sha256: String,
}

/// Comilla simple para la shell del contenedor. El texto ya pasó las reglas del inventario.
fn comilla(texto: &str) -> String {
    format!("'{}'", texto.replace('\'', "'\\''"))
}

/// `true` si el texto no lleva espacios, control ni caracteres de shell.
fn caracteres_seguros(texto: &str) -> bool {
    !texto
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || "'\"$`;\\|&<>()".contains(c))
}

/// Nombre de archivo que nunca sale de `/cache`: letras, números, `.`, `_`, `+` y `-`.
fn archivo_seguro(nombre: &str) -> bool {
    !nombre.is_empty()
        && !nombre.starts_with('.')
        && !nombre.contains("..")
        && nombre
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-'))
}

/// Nombre normalizado de un especificador (PEP 503): minúsculas, `-` en lugar de `_` y `.`.
fn nombre_de_spec(spec: &str) -> String {
    spec.chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .collect::<String>()
        .to_lowercase()
        .replace(['_', '.'], "-")
}

/// Versión de un especificador con `==` exacto. `None` si no hay versión fijada. Un comodín como `==1.*`
/// no fija versión: no se usa su parte fija como versión.
fn version_fijada(spec: &str) -> Option<String> {
    let (_, resto) = spec.split_once("==")?;
    let resto = resto.trim();
    if resto.contains('*') {
        return None;
    }
    let version: String = resto
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '!' | '-'))
        .collect();
    (!version.is_empty()).then_some(version)
}

/// Comando de `pip download` de wheels. Solo wheels; el índice va explícito, sin config de pip.
pub fn comando_wheels(indice: &str, req: &RequisitosPip) -> String {
    let propio = req.indice_propio.as_deref().unwrap_or(INDICE_PYPI);
    let mut comando = format!(
        "cd /tmp && PIP_CONFIG_FILE=/dev/null python3 -m pip download --only-binary=:all: --no-input --disable-pip-version-check --index-url {} -d /cache -r {indice}",
        comilla(propio)
    );
    for extra in &req.indices_extra {
        comando.push_str(&format!(" --extra-index-url {}", comilla(extra)));
    }
    comando
}

/// Comando que clona una fuente git sin hooks ni submódulos, en el commit fijo, y deja el commit obtenido en un
/// archivo. El commit es obligatorio: un clon sin checkout deja un árbol vacío (M7).
pub fn comando_clon(url: &str, dir: &str, commit: &str) -> String {
    format!(
        "mkdir -p /cache/src && git -c core.hooksPath=/dev/null -c protocol.allow=never -c protocol.https.allow=always clone --no-checkout {} /cache/src/{dir} && git -C /cache/src/{dir} -c core.hooksPath=/dev/null checkout --detach {commit} && git -C /cache/src/{dir} rev-parse HEAD > /cache/src/{dir}.commit",
        comilla(url)
    )
}

/// Clave de un clon: repo y commit. Dos líneas del mismo repo y commit comparten el árbol (B3).
pub fn clave_clon(partes: &PartesGit) -> String {
    hash_corto(format!("{}@{}", partes.base, partes.commit).as_bytes())
}

/// Línea de `requirements-offline.txt` para un clon. Apunta a la copia escribible que el prelude deja en
/// `/tmp/fuentes`: pip construye en el árbol fuente, y `/cache` es de solo lectura en la evaluación (M10).
pub fn linea_offline_git(dir: &str, subdir: Option<&str>) -> String {
    match subdir {
        Some(subdir) => format!("/tmp/fuentes/{dir}/{subdir}"),
        None => format!("/tmp/fuentes/{dir}"),
    }
}

/// Versión del formato de la clave de Python. Subirla invalida las cachés hechas con la fórmula anterior.
const VERSION_CLAVE_PYTHON: u32 = 2;

/// Clave de la caché de Python: requisitos, restricciones e imagen (M8). La imagen decide qué wheels bajan
/// (`cp38` o `cp312`), así que va en la clave. Con el `digest` de podman (desempate 27), un tag reconstruido
/// no usa wheels de otra imagen. Sin digest, la clave es la de la imagen sola.
pub fn clave_cache_python(
    requisitos: &str,
    restricciones: &str,
    imagen: &str,
    digest: Option<&str>,
) -> String {
    let digest = digest.map_or(String::new(), |d| format!("digest={d}\n"));
    hash_corto(
        format!(
            "formato={VERSION_CLAVE_PYTHON}\nimagen={imagen}\n{digest}--requisitos--\n{requisitos}--restricciones--\n{restricciones}"
        )
        .as_bytes(),
    )
}

/// Argumentos de `podman image exists` para `imagen`. Son puros: no corren nada.
pub fn args_imagen_existe(imagen: &str) -> Vec<String> {
    ["image", "exists", imagen]
        .iter()
        .map(|a| a.to_string())
        .collect()
}

/// Confirma que `imagen` está local antes de calcular la clave (desempate 11). Sin ella, la evaluación no
/// puede correr: es infraestructura. Un código distinto de cero, o un error al lanzar `podman`, lo es.
pub(crate) fn comprobar_imagen_local(
    podman: &dyn Podman,
    imagen: &str,
) -> Result<(), ErrorOraculo> {
    let args = args_imagen_existe(imagen);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let salida = podman.correr(&refs, PLAZO_CORTO).map_err(|e| {
        ErrorOraculo::Infra(format!("no se pudo consultar la imagen {imagen}: {e}"))
    })?;
    if salida.codigo != 0 {
        return Err(ErrorOraculo::Infra(format!(
            "la imagen {imagen} no está local: `podman image exists` dio código {}",
            salida.codigo
        )));
    }
    Ok(())
}

/// Argumentos de `podman image inspect` que leen el digest de `imagen` (desempate 27). Son puros: no corren nada.
pub fn args_digest_imagen(imagen: &str) -> Vec<String> {
    ["image", "inspect", "--format", "{{.Digest}}", imagen]
        .iter()
        .map(|a| a.to_string())
        .collect()
}

/// Digest de `imagen` según `podman`. `None` si el comando falla o no da uno: la descarga sigue sin él.
pub(crate) fn digest_imagen_con(podman: &dyn Podman, imagen: &str) -> Option<String> {
    let args = args_digest_imagen(imagen);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let salida = podman.correr(&refs, PLAZO_CORTO).ok()?;
    if salida.codigo != 0 {
        return None;
    }
    let digest = salida.stdout.trim();
    (!digest.is_empty()).then(|| digest.to_string())
}

/// Comando que baja un archivo por HTTPS a `/cache/<archivo>`.
pub fn comando_curl(url: &str, archivo: &str) -> String {
    format!(
        "curl -fsSL --proto '=https' --tlsv1.2 --max-time {PLAZO_CURL_S} -o /cache/{archivo} {}",
        comilla(url)
    )
}

/// Nombre y versión de un paquete que pip no halló como wheel. `None` si el error es otro.
pub fn sin_wheel(stderr: &str) -> Option<String> {
    const MARCA: &str = "No matching distribution found for ";
    stderr
        .lines()
        .find_map(|linea| {
            linea
                .split_once(MARCA)
                .map(|(_, resto)| resto.trim().to_string())
        })
        .filter(|spec| !spec.is_empty())
}

/// Corte de un paquete que pip no halló como wheel (M9). `directo` dice si es requisito directo. Un requisito
/// directo sin versión fijada da `sin-version-fijada`. Una dependencia da `solo-sdist`: solo tiene sdist, y
/// la descarga no construye sdists (decisión 21). Es un corte sin veredicto, no un rojo del producto.
fn corte_sin_wheel(spec: &str, directo: bool) -> Fin {
    if directo {
        return Fin::Corta(
            hallazgo(
                ARCHIVO_REQUIREMENTS,
                0,
                "",
                2,
                format!("dependencia sin wheel y sin versión fijada: {spec}"),
            ),
            "sin-version-fijada",
        );
    }
    let nombre: String = spec
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .collect();
    Fin::Corta(
        hallazgo(
            ARCHIVO_REQUIREMENTS,
            0,
            "",
            2,
            format!("{nombre} solo tiene sdist y la descarga no construye sdists"),
        ),
        "solo-sdist",
    )
}

/// Elige el sdist de una versión desde el JSON de `https://pypi.org/pypi/<n>/<v>/json`.
pub fn sdist_de_json(json: &str) -> Result<Sdist, String> {
    let v: Value = serde_json::from_str(json).map_err(|e| format!("JSON de PyPI ilegible: {e}"))?;
    let entradas = v["urls"]
        .as_array()
        .ok_or_else(|| "el JSON de PyPI no trae urls".to_string())?;
    let sdist = entradas
        .iter()
        .find(|e| e["packagetype"] == "sdist")
        .ok_or_else(|| SIN_SDIST.to_string())?;
    let url = sdist["url"].as_str().unwrap_or_default();
    let archivo = sdist["filename"].as_str().unwrap_or_default();
    let sha = sdist["digests"]["sha256"].as_str().unwrap_or_default();
    if !url.starts_with(PREFIJO_SDIST_PYPI) || !caracteres_seguros(url) {
        return Err("URL de sdist fuera de files.pythonhosted.org".into());
    }
    if !archivo_seguro(archivo) {
        return Err(format!("nombre de sdist no seguro: {archivo}"));
    }
    if sha.len() != 64 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("PyPI no da un sha256 válido para el sdist".into());
    }
    Ok(Sdist {
        url: url.into(),
        archivo: archivo.into(),
        sha256: sha.to_lowercase(),
    })
}

/// sha256 en hexadecimal de un contenido.
fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Por qué no se baja un sdist: PyPI no tiene esa versión o no trae sdist (el producto no instala), o la
/// descarga falló por otra causa (infraestructura).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FalloSdist {
    NoExiste,
    Otro(String),
}

/// `true` si curl dijo que el recurso no existe. Con `-f`, curl da `returned error: 404`.
fn es_no_existe(motivo: &str) -> bool {
    motivo.contains("returned error: 404")
}

/// Baja el sdist de `nombre==version` a `cache`. `pedir(url, archivo)` deja el archivo en `cache`.
/// Verifica el sha256 de PyPI. Un hash que no coincide borra el archivo y da error. Nunca lo extrae.
pub fn bajar_sdist_con<F>(
    nombre: &str,
    version: &str,
    cache: &Path,
    mut pedir: F,
) -> Result<Sdist, FalloSdist>
where
    F: FnMut(&str, &str) -> Result<(), String>,
{
    if !caracteres_seguros(nombre) || !caracteres_seguros(version) {
        return Err(FalloSdist::Otro(format!(
            "paquete o versión no seguros: {nombre}=={version}"
        )));
    }
    let url_json = format!("{JSON_PYPI}/{nombre}/{version}/json");
    // Un 404 del JSON es un nombre o una versión que PyPI no tiene: un veredicto del producto (B1).
    pedir(&url_json, ARCHIVO_JSON).map_err(|motivo| {
        if es_no_existe(&motivo) {
            FalloSdist::NoExiste
        } else {
            FalloSdist::Otro(motivo)
        }
    })?;
    let ruta_json = cache.join(ARCHIVO_JSON);
    let json = fs::read_to_string(&ruta_json)
        .map_err(|e| FalloSdist::Otro(format!("leer el JSON de PyPI: {e}")))?;
    borrado::borrar_archivo_dentro(cache, &ruta_json)
        .map_err(|e| FalloSdist::Otro(format!("borrar el JSON: {e}")))?;
    let sdist = sdist_de_json(&json).map_err(|motivo| {
        if motivo == SIN_SDIST {
            FalloSdist::NoExiste
        } else {
            FalloSdist::Otro(motivo)
        }
    })?;
    pedir(&sdist.url, &sdist.archivo).map_err(FalloSdist::Otro)?;
    let ruta = cache.join(&sdist.archivo);
    let bytes = fs::read(&ruta).map_err(|e| FalloSdist::Otro(format!("leer el sdist: {e}")))?;
    if sha256_hex(&bytes) != sdist.sha256 {
        let _ = borrado::borrar_archivo_dentro(cache, &ruta);
        return Err(FalloSdist::Otro(format!(
            "el sha256 de {} no coincide: se descarta",
            sdist.archivo
        )));
    }
    Ok(sdist)
}

/// Resultado de correr un contenedor de descarga de Python: su código y su error.
struct Salida {
    codigo: i32,
    stderr: String,
}

/// Corre contenedores de la descarga de Python. Producción usa `contenedor::ejecutar`; los tests pasan uno
/// falso que guarda cada ejecución.
trait EjecutorDeDescarga {
    fn ejecutar(&self, ejecucion: &Ejecucion) -> Result<Resultado, ErrorOraculo>;
}

/// Ejecutor real: lanza el contenedor con podman.
struct ContenedorReal;

impl EjecutorDeDescarga for ContenedorReal {
    fn ejecutar(&self, ejecucion: &Ejecucion) -> Result<Resultado, ErrorOraculo> {
        contenedor::ejecutar(ejecucion)
    }
}

/// Lo que la descarga de Python toma de fuera: el podman del digest, el ejecutor de contenedores y las
/// variables de Root. Producción pasa las reales; los tests, dobles.
struct DependenciasPython<'a> {
    podman: &'a dyn Podman,
    ejecutor: &'a dyn EjecutorDeDescarga,
    leer_env: &'a dyn Fn(&str) -> Option<String>,
}

/// Cómo corre sus contenedores una descarga de Python: ejecutor, tope de tiempo y `netrc` de pip.
struct Corrida<'a> {
    home: &'a Path,
    tarea: &'a Tarea,
    timeout: Duration,
    ejecutor: &'a dyn EjecutorDeDescarga,
    /// Hay un `netrc` de pip en `trabajo`: los contenedores de pip lo leen con `NETRC`.
    netrc: bool,
    /// Credenciales del `netrc`. Solo sirven para redactar lo que sale de los contenedores.
    credenciales: &'a [Credencial],
}

/// Corre un comando de red con `parcial` montada en `/cache`, rw. El contenedor no monta `/repo`:
/// ve solo `trabajo`, la copia de los `requirements*.txt` (C1). No recibe el `netrc` de pip.
fn correr(
    corrida: &Corrida,
    trabajo: &Path,
    parcial: &Path,
    cmd: String,
) -> Result<Salida, ErrorOraculo> {
    ejecutar_en_contenedor(corrida, trabajo, parcial, cmd, false)
}

/// Como `correr`, para `pip download`: si hay `netrc` de pip, el contenedor lo lee con `NETRC`.
fn correr_pip(
    corrida: &Corrida,
    trabajo: &Path,
    parcial: &Path,
    cmd: String,
) -> Result<Salida, ErrorOraculo> {
    ejecutar_en_contenedor(corrida, trabajo, parcial, cmd, corrida.netrc)
}

fn ejecutar_en_contenedor(
    corrida: &Corrida,
    trabajo: &Path,
    parcial: &Path,
    cmd: String,
    con_netrc: bool,
) -> Result<Salida, ErrorOraculo> {
    let salida = temporal_de(corrida.home)?;
    let mut ejecucion = ejecucion_python(
        &corrida.tarea.imagen,
        trabajo,
        parcial,
        salida.path(),
        cmd,
        corrida.timeout,
    );
    if con_netrc {
        ejecucion
            .env
            .push(("NETRC".into(), RUTA_NETRC_CONTENEDOR.into()));
    }
    // Ningún texto que sale del contenedor lleva una clave del `netrc`, ni siquiera en un error de podman.
    let resultado = corrida
        .ejecutor
        .ejecutar(&ejecucion)
        .map_err(|error| match error {
            ErrorOraculo::Infra(texto) => {
                ErrorOraculo::Infra(redactar_secretos(&texto, corrida.credenciales))
            }
            otro => otro,
        })?;
    Ok(Salida {
        codigo: resultado.codigo,
        stderr: redactar_secretos(&resultado.stderr, corrida.credenciales),
    })
}

/// Escribe el `netrc` de pip en `trabajo` con permisos 0600. `create_new` no pisa ni sigue un archivo previo.
fn escribir_netrc(trabajo: &Path, contenido: &str) -> Result<(), ErrorOraculo> {
    let mut archivo = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(trabajo.join(ARCHIVO_NETRC))
        .map_err(|e| infra("escribir el netrc de pip", e))?;
    archivo
        .write_all(contenido.as_bytes())
        .map_err(|e| infra("escribir el netrc de pip", e))
}

/// Índices que usa `pip download`: el `--index-url`, o PyPI si no hay, y los `--extra-index-url`.
fn indices_de(req: &RequisitosPip) -> Vec<String> {
    let mut indices = vec![
        req.indice_propio
            .clone()
            .unwrap_or_else(|| INDICE_PYPI.to_string()),
    ];
    indices.extend(req.indices_extra.iter().cloned());
    indices
}

/// Sha256 de un archivo de la caché.
fn sha256_de(ruta: &Path) -> Result<String, ErrorOraculo> {
    let bytes = fs::read(ruta).map_err(|e| infra("leer un archivo de la caché de Python", e))?;
    Ok(sha256_hex(&bytes))
}

/// Resultado de la descarga: los archivos del Gate y sus hallazgos, o un corte con su hallazgo.
enum Fin {
    Lista {
        paquetes: Vec<Value>,
        hallazgos: Vec<Hallazgo>,
    },
    Corta(Hallazgo, &'static str),
}

/// Descarga de Python para `requirements.txt`. Cache por hash del archivo (`<hash>.parcial` y luego el nombre final).
/// `registros` son los `[[registro]]` de `root.toml`: los que coinciden con un índice del requirements
/// reciben su credencial en un `netrc` de pip (decisión 29).
pub(super) fn preparar_python(
    repo: &Path,
    tarea: &Tarea,
    home: &Path,
    timeout: Duration,
    registros: &[RegistroConfig],
) -> Result<Paso, ErrorOraculo> {
    let leer_env = |nombre: &str| std::env::var(nombre).ok();
    let deps = DependenciasPython {
        podman: &PodmanReal,
        ejecutor: &ContenedorReal,
        leer_env: &leer_env,
    };
    preparar_python_con(repo, tarea, home, timeout, registros, &deps)
}

/// Como `preparar_python`, con las dependencias que se le pasen. Los tests pasan dobles.
fn preparar_python_con(
    repo: &Path,
    tarea: &Tarea,
    home: &Path,
    timeout: Duration,
    registros: &[RegistroConfig],
    deps: &DependenciasPython,
) -> Result<Paso, ErrorOraculo> {
    let ruta = repo.join(ARCHIVO_REQUIREMENTS);
    if !ruta.is_file() {
        return Ok((
            Preparacion::Lista {
                cache: None,
                rustsec: None,
                hallazgos: Vec::new(),
            },
            None,
        ));
    }
    // El texto de `requirements.txt` con sus `-r` seguidos, y las `-c` aparte: pip no ve archivos del repo (C1, M6).
    let (texto, restricciones) = textos_de_requisitos(repo)?;
    comprobar_imagen_local(deps.podman, &tarea.imagen)?;
    let digest = digest_imagen_con(deps.podman, &tarea.imagen);
    let hash = clave_cache_python(&texto, &restricciones, &tarea.imagen, digest.as_deref());
    // Sin digest, la clave usa la imagen sola. El Gate lo registra; no es hallazgo del producto ni corta (desempate 27).
    let sin_digest = digest.is_none().then(|| tarea.imagen.clone());
    let req = requisitos_pip(&texto);
    let destino = home.join(CARPETA_CACHE).join(CARPETA_PYTHON).join(&hash);
    if destino.join(ARCHIVO_OFFLINE).is_file() && requisitos_sdist_listos(&destino)? {
        let registro = Registro {
            lenguaje: "python".into(),
            hash: Some(hash),
            cache_hit: true,
            paquetes: Vec::new(),
            rustsec: None,
            motivo: None,
            imagenes: None,
            sin_digest,
            avisos: Vec::new(),
            rechazos: Vec::new(),
        };
        // Los hallazgos de build salen de los `pyproject` guardados en la caché: sin ellos, el segundo
        // corrido perdería el aviso del primero.
        let mut hallazgos = Vec::new();
        if let Some((_, de_build)) = indice_de_build_de(&destino, &rutas_clon(&req))? {
            hallazgos.extend(de_build);
        }
        // El aviso de dependencias no verificadas también sale del acierto: lo leen los mismos archivos.
        let no_verificadas = hallazgos_de_dependencias_no_verificadas(&destino, &req)?;
        let registro = con_avisos_de(registro, &no_verificadas);
        hallazgos.extend(no_verificadas);
        let preparacion = Preparacion::Lista {
            cache: Some(destino),
            rustsec: None,
            hallazgos,
        };
        return Ok((preparacion, Some(registro)));
    }
    let parcial = parcial_de(&destino);
    limpiar_parcial(home, &parcial).map_err(|e| infra("borrar la descarga parcial", e))?;
    fs::create_dir_all(&parcial).map_err(|e| infra("crear la caché parcial de Python", e))?;
    // Copia de los requirements para el contenedor: el repo no entra (C1).
    let trabajo = temporal_de(home)?;
    copiar_requisitos(repo, trabajo.path()).map_err(|e| infra("copiar los requirements", e))?;
    // El `netrc` de pip solo lleva las claves de los registros que coinciden con un índice (decisión 29).
    let (netrc, avisos) = netrc_de_pip(&indices_de(&req), registros, deps.leer_env);
    if let Some(netrc) = &netrc {
        escribir_netrc(trabajo.path(), &netrc.contenido)?;
    }
    let credenciales: &[Credencial] = match &netrc {
        Some(netrc) => &netrc.credenciales,
        None => &[],
    };
    let corrida = Corrida {
        home,
        tarea,
        timeout,
        ejecutor: deps.ejecutor,
        netrc: netrc.is_some(),
        credenciales,
    };
    let fin = descargar_en(&corrida, trabajo.path(), &parcial, &req, &restricciones);
    // El `netrc` sale de la carpeta de trabajo al terminar, también si la descarga falló.
    if netrc.is_some() {
        borrado::borrar_archivo_dentro(trabajo.path(), &trabajo.path().join(ARCHIVO_NETRC))
            .map_err(|e| infra("borrar el netrc de pip", e))?;
    }
    let fin = match fin {
        Ok(fin) => fin,
        Err(error) => {
            limpiar_parcial(home, &parcial).map_err(|e| infra("borrar la descarga parcial", e))?;
            return Err(error);
        }
    };
    match fin {
        Fin::Corta(aviso, motivo) => {
            limpiar_parcial(home, &parcial).map_err(|e| infra("borrar la descarga parcial", e))?;
            let registro = Registro {
                avisos,
                ..Registro::corte("python", Some(hash), motivo)
            };
            Ok(corte(aviso, registro))
        }
        Fin::Lista {
            paquetes,
            hallazgos: de_build,
        } => {
            // Se calcula antes de mover `parcial` a su nombre final: lee los mismos archivos que el acierto.
            let no_verificadas = hallazgos_de_dependencias_no_verificadas(&parcial, &req)?;
            let mut hallazgos = de_build;
            hallazgos.extend(no_verificadas.iter().cloned());
            if destino.exists() {
                borrar_en_home(home, &destino)
                    .map_err(|e| infra("borrar la caché previa de Python", e))?;
            }
            fs::rename(&parcial, &destino).map_err(|e| infra("cerrar la caché de Python", e))?;
            let registro = Registro {
                lenguaje: "python".into(),
                hash: Some(hash),
                cache_hit: false,
                paquetes,
                rustsec: None,
                motivo: None,
                imagenes: None,
                sin_digest,
                avisos,
                rechazos: Vec::new(),
            };
            let registro = con_avisos_de(registro, &no_verificadas);
            let preparacion = Preparacion::Lista {
                cache: Some(destino),
                rustsec: None,
                hallazgos,
            };
            Ok((preparacion, Some(registro)))
        }
    }
}

/// Líneas del índice de pip: las restricciones (`-c`), si las hay, y los especificadores pendientes.
fn contenido_indice(pendientes: &[String], con_restricciones: bool) -> String {
    let mut lineas = Vec::new();
    if con_restricciones {
        lineas.push(format!("-c /cache/{ARCHIVO_RESTRICCIONES}"));
    }
    lineas.extend(pendientes.iter().cloned());
    format!("{}\n", lineas.join("\n"))
}

/// Hallazgo de un paquete que no instala desde PyPI o desde una fuente de Python. Su archivo es el requirements.
fn hallazgo_no_instala_python(linea: &str) -> Hallazgo {
    let linea = truncar_linea(&redactar(linea));
    hallazgo(
        ARCHIVO_REQUIREMENTS,
        0,
        &linea,
        3,
        format!("el producto no instala: {linea}"),
    )
}

/// Baja wheels, sdists, fuentes git y URLs dentro de `parcial`. Escribe el archivo offline.
fn descargar_en(
    corrida: &Corrida,
    trabajo: &Path,
    parcial: &Path,
    req: &RequisitosPip,
    restricciones: &str,
) -> Result<Fin, ErrorOraculo> {
    // Una referencia git sin commit fijo, o con otro formato, corta antes de cualquier red (B2, M7).
    for (linea, raw) in &req.directas {
        if let Some(motivo) = motivo_git(raw).filter(|_| raw.starts_with("git+")) {
            return Ok(Fin::Corta(
                hallazgo(
                    ARCHIVO_REQUIREMENTS,
                    *linea,
                    "",
                    3,
                    format!("{motivo}: {}", redactar(raw)),
                ),
                "git-no-seguro",
            ));
        }
    }
    if !restricciones.is_empty() {
        fs::write(parcial.join(ARCHIVO_RESTRICCIONES), restricciones)
            .map_err(|e| infra("escribir las restricciones de Python", e))?;
    }
    let mut paquetes: Vec<Value> = Vec::new();
    let mut pendientes: Vec<String> = req.especificadores.clone();
    while !pendientes.is_empty() {
        fs::write(
            parcial.join(ARCHIVO_INDICE),
            contenido_indice(&pendientes, !restricciones.is_empty()),
        )
        .map_err(|e| infra("escribir el índice de pip", e))?;
        let cmd = comando_wheels(&format!("/cache/{ARCHIVO_INDICE}"), req);
        let salida = correr_pip(corrida, trabajo, parcial, cmd)?;
        if salida.codigo == 0 {
            break;
        }
        let Some(spec) = sin_wheel(&salida.stderr) else {
            return match diagnosticar(&salida.stderr) {
                Diagnostico::Producto(linea) => {
                    Ok(Fin::Corta(hallazgo_no_instala_python(&linea), "no-instala"))
                }
                otro => Err(error_de(otro)),
            };
        };
        let nombre = nombre_de_spec(&spec);
        let posicion = pendientes.iter().position(|s| nombre_de_spec(s) == nombre);
        let version = posicion.and_then(|i| version_fijada(&pendientes[i]));
        let (Some(pos), Some(version)) = (posicion, version) else {
            return Ok(corte_sin_wheel(&spec, posicion.is_some()));
        };
        if req.indice_propio.is_some() {
            return Ok(Fin::Corta(
                hallazgo(
                    ARCHIVO_REQUIREMENTS,
                    0,
                    "",
                    2,
                    format!("sdist de un índice propio no se soporta: {nombre}"),
                ),
                "indice-propio-sdist",
            ));
        }
        let sdist = match bajar_sdist_pypi(corrida, trabajo, parcial, &nombre, &version)? {
            Ok(sdist) => sdist,
            Err(corte) => return Ok(corte),
        };
        paquetes
            .push(json!({"nombre": sdist.archivo, "fuente": "sdist-pypi", "sha256": sdist.sha256}));
        pendientes.remove(pos);
    }
    // Los wheels bajados: cada uno con su sha256 para el Gate.
    for entrada in fs::read_dir(parcial).map_err(|e| infra("leer la caché de Python", e))? {
        let entrada = entrada.map_err(|e| infra("leer la caché de Python", e))?;
        let nombre = entrada.file_name().to_string_lossy().into_owned();
        if nombre.ends_with(".whl") {
            let sha = sha256_de(&entrada.path())?;
            paquetes.push(json!({"nombre": nombre, "fuente": "pypi", "sha256": sha}));
        }
    }
    let mut lineas: Vec<String> = Vec::new();
    if !restricciones.is_empty() {
        lineas.push(format!("-c /cache/{ARCHIVO_RESTRICCIONES}"));
    }
    lineas.extend(req.especificadores.clone());
    for (linea, raw) in &req.directas {
        match fuente_directa(corrida, trabajo, parcial, *linea, raw, &mut paquetes)? {
            Ok(ruta_local) => lineas.push(ruta_local),
            Err(corte) => return Ok(corte),
        }
    }
    // Build de los sdists y clones git: sus requisitos se bajan como wheels, sin ejecutar nada del proyecto.
    // Los `pyproject` de los sdists los lee el contenedor; el host solo lee el JSON que deja en la caché.
    if !nombres_sdist(parcial)?.is_empty() {
        let salida = correr(corrida, trabajo, parcial, comando_requisitos_sdist())?;
        if salida.codigo != 0 {
            let motivo: String = salida.stderr.trim().chars().take(300).collect();
            return Err(ErrorOraculo::Infra(format!(
                "infraestructura: requisitos de build de los sdists: {motivo}"
            )));
        }
    }
    // Sin lista estática de dependencias de runtime no hay qué bajar: corte sin veredicto, antes de pip.
    if let Some(nombre) = fuentes_dinamicas(parcial, req)?.first() {
        return Ok(Fin::Corta(
            hallazgo(
                ARCHIVO_REQUIREMENTS,
                0,
                "",
                2,
                format!("dependencias de runtime no declaradas en forma estática: {nombre}"),
            ),
            "dependencias-dinamicas",
        ));
    }
    let (requisitos, hallazgos) =
        indice_de_build_de(parcial, &rutas_clon(req))?.unwrap_or_default();
    if !requisitos.is_empty() {
        let conocidos: BTreeSet<String> = paquetes
            .iter()
            .filter_map(|p| p["nombre"].as_str().map(String::from))
            .collect();
        fs::write(
            parcial.join(ARCHIVO_BUILD),
            format!("{}\n", requisitos.join("\n")),
        )
        .map_err(|e| infra("escribir los requisitos de build", e))?;
        let cmd = comando_wheels(&format!("/cache/{ARCHIVO_BUILD}"), req);
        let salida = correr_pip(corrida, trabajo, parcial, cmd)?;
        if salida.codigo != 0 {
            // Una dependencia de runtime sin wheel es un límite de Root, no un defecto del producto.
            let de_runtime = nombres_de_runtime(parcial, req)?;
            if let Some(spec) =
                sin_wheel(&salida.stderr).filter(|s| de_runtime.contains(&nombre_de_spec(s)))
            {
                return Ok(corte_sin_wheel(&spec, false));
            }
            return match diagnosticar(&salida.stderr) {
                Diagnostico::Producto(linea) => {
                    Ok(Fin::Corta(hallazgo_no_instala_python(&linea), "no-instala"))
                }
                otro => Err(error_de(otro)),
            };
        }
        for entrada in fs::read_dir(parcial).map_err(|e| infra("leer la caché de Python", e))? {
            let entrada = entrada.map_err(|e| infra("leer la caché de Python", e))?;
            let nombre = entrada.file_name().to_string_lossy().into_owned();
            if nombre.ends_with(".whl") && !conocidos.contains(&nombre) {
                let sha = sha256_de(&entrada.path())?;
                paquetes.push(json!({"nombre": nombre, "fuente": "pypi-build", "sha256": sha}));
            }
        }
    }
    fs::write(
        parcial.join(ARCHIVO_OFFLINE),
        format!("{}\n", lineas.join("\n")),
    )
    .map_err(|e| infra("escribir requirements-offline.txt", e))?;
    Ok(Fin::Lista {
        paquetes,
        hallazgos,
    })
}

/// Baja el sdist de PyPI con `curl`, dentro del contenedor. Verifica el sha256 en el host. Un nombre o una
/// versión que PyPI no tiene da un corte con su hallazgo de "no instala".
fn bajar_sdist_pypi(
    corrida: &Corrida,
    trabajo: &Path,
    parcial: &Path,
    nombre: &str,
    version: &str,
) -> Result<Result<Sdist, Fin>, ErrorOraculo> {
    let pedir = |url: &str, archivo: &str| -> Result<(), String> {
        let cmd = comando_curl(url, archivo);
        match correr(corrida, trabajo, parcial, cmd) {
            Ok(salida) if salida.codigo == 0 => Ok(()),
            Ok(salida) => Err(salida.stderr.trim().chars().take(300).collect()),
            Err(error) => Err(error.to_string()),
        }
    };
    match bajar_sdist_con(nombre, version, parcial, pedir) {
        Ok(sdist) => Ok(Ok(sdist)),
        Err(FalloSdist::NoExiste) => Ok(Err(Fin::Corta(
            hallazgo_no_instala_python(&format!("{nombre}=={version}")),
            "no-instala",
        ))),
        Err(FalloSdist::Otro(motivo)) => Err(ErrorOraculo::Infra(format!(
            "infraestructura: sdist de {nombre}: {motivo}"
        ))),
    }
}

/// Una fuente git o URL del requirements. `Ok(ruta)` es la línea local que va al archivo offline.
/// `Err(fin)` corta la evaluación con su hallazgo.
fn fuente_directa(
    corrida: &Corrida,
    trabajo: &Path,
    parcial: &Path,
    linea: u32,
    raw: &str,
    paquetes: &mut Vec<Value>,
) -> Result<Result<String, Fin>, ErrorOraculo> {
    if raw.starts_with("git+") {
        let partes = match partes_git(raw) {
            Ok(partes) => partes,
            Err(motivo) => {
                return Ok(Err(Fin::Corta(
                    hallazgo(ARCHIVO_REQUIREMENTS, linea, "", 3, motivo),
                    "git-no-seguro",
                )));
            }
        };
        if !caracteres_seguros(&partes.base) {
            return Ok(Err(Fin::Corta(
                hallazgo(
                    ARCHIVO_REQUIREMENTS,
                    linea,
                    "",
                    3,
                    "referencia git no segura: caracteres de shell".into(),
                ),
                "git-no-seguro",
            )));
        }
        let dir = clave_clon(&partes);
        // Un clon por repo y commit: dos líneas con distinto `#subdirectory=` comparten el árbol (B3).
        if !parcial.join("src").join(&dir).is_dir() {
            let cmd = comando_clon(&partes.base, &dir, &partes.commit);
            let salida = correr(corrida, trabajo, parcial, cmd)?;
            if salida.codigo != 0 {
                return match diagnosticar(&salida.stderr) {
                    Diagnostico::Producto(l) => Ok(Err(Fin::Corta(
                        hallazgo_no_instala_python(&l),
                        "no-instala",
                    ))),
                    otro => Err(error_de(otro)),
                };
            }
            let commit_obtenido =
                fs::read_to_string(parcial.join("src").join(format!("{dir}.commit")))
                    .map_err(|e| infra("leer el commit de un clon git", e))?
                    .trim()
                    .to_string();
            paquetes
                .push(json!({"nombre": partes.base, "fuente": "git", "commit": commit_obtenido}));
        }
        return Ok(Ok(linea_offline_git(&dir, partes.subdir.as_deref())));
    }
    // URL de archivo: `https://…/x.whl|.tar.gz|.zip`, con `#sha256=` opcional.
    let (url, sha_esperado) = match raw.split_once("#sha256=") {
        Some((url, sha)) => (url.to_string(), Some(sha.trim().to_lowercase())),
        None => (raw.split('#').next().unwrap_or(raw).to_string(), None),
    };
    let sin_consulta = url.split('?').next().unwrap_or(&url);
    let archivo = sin_consulta.rsplit('/').next().unwrap_or("").to_string();
    let formato_ok = [".whl", ".tar.gz", ".zip"]
        .iter()
        .any(|s| archivo.ends_with(s));
    if !formato_ok || !archivo_seguro(&archivo) || !caracteres_seguros(&url) {
        return Ok(Err(Fin::Corta(
            hallazgo(
                ARCHIVO_REQUIREMENTS,
                linea,
                "",
                3,
                format!("URL de archivo no soportada: {archivo}"),
            ),
            "url-no-soportada",
        )));
    }
    let salida = correr(corrida, trabajo, parcial, comando_curl(&url, &archivo))?;
    if salida.codigo != 0 {
        return Err(ErrorOraculo::Infra(format!(
            "infraestructura: descarga de {archivo}: {}",
            salida.stderr.trim().chars().take(300).collect::<String>()
        )));
    }
    let sha = sha256_de(&parcial.join(&archivo))?;
    if let Some(esperado) = sha_esperado
        && esperado != sha
    {
        let _ = borrado::borrar_archivo_dentro(parcial, &parcial.join(&archivo));
        return Err(ErrorOraculo::Infra(format!(
            "infraestructura: el sha256 de {archivo} no coincide"
        )));
    }
    paquetes.push(json!({"nombre": archivo, "fuente": "url", "sha256": sha}));
    Ok(Ok(format!("/cache/{archivo}")))
}

/// Requisitos de build de un `pyproject.toml`, sin ejecutar nada. Sin el texto, sin
/// `[build-system].requires`, o si no parsea, son `setuptools` y `wheel` (el respaldo de PEP 517).
pub fn requisitos_de_build(pyproject: Option<&str>) -> Vec<String> {
    let Some(texto) = pyproject else {
        return respaldo_de_build();
    };
    let Ok(tabla) = toml::from_str::<toml::Table>(texto) else {
        return respaldo_de_build();
    };
    let requires = tabla
        .get("build-system")
        .and_then(|b| b.as_table())
        .and_then(|b| b.get("requires"))
        .and_then(|r| r.as_array());
    let Some(requires) = requires else {
        return respaldo_de_build();
    };
    let mut salida = Vec::new();
    for item in requires {
        // Una lista con algo que no es texto no sirve: se usa el respaldo.
        let Some(spec) = item.as_str() else {
            return respaldo_de_build();
        };
        let spec = spec.trim();
        if !spec.is_empty() {
            salida.push(spec.to_string());
        }
    }
    salida
}

fn respaldo_de_build() -> Vec<String> {
    REQUISITOS_BUILD_RESPALDO
        .iter()
        .map(|s| s.to_string())
        .collect()
}

/// Dependencias de runtime de un `pyproject.toml` de clon, sin ejecutar nada: `[project].dependencies`.
/// Sin texto, o sin `[project]` (un `setup.py` legado), no declara ninguna: `Some(vec![])`. `None` si
/// `dependencies` es dinámico, si no es lista de strings o si el TOML no parsea. Sin `dependencies` ni
/// `dynamic`, tampoco declara ninguna.
fn dependencias_de_clon(pyproject: Option<&str>) -> Option<Vec<String>> {
    let Some(texto) = pyproject else {
        return Some(Vec::new());
    };
    let tabla = toml::from_str::<toml::Table>(texto).ok()?;
    let Some(proyecto) = tabla.get("project") else {
        return Some(Vec::new());
    };
    let proyecto = proyecto.as_table()?;
    if let Some(dinamico) = proyecto.get("dynamic") {
        let dinamicos = dinamico.as_array()?;
        if dinamicos.iter().any(|d| d.as_str() == Some("dependencies")) {
            return None;
        }
    }
    let Some(valor) = proyecto.get("dependencies") else {
        return Some(Vec::new());
    };
    let textos: Option<Vec<String>> = valor
        .as_array()?
        .iter()
        .map(|item| item.as_str().map(str::to_string))
        .collect();
    Some(limpios(&textos?))
}

/// Recorta cada texto de una lista y descarta los vacíos.
fn limpios(lista: &[String]) -> Vec<String> {
    lista
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// `true` si el requisito se baja como wheel del índice: pasa la gramática PEP 508 estricta. Sin URL,
/// sin `@` y sin opciones pegadas (`--no-binary`, `-e`).
pub fn requisito_de_build_aceptable(spec: &str) -> bool {
    especificador_pep508_estricto(spec)
}

/// Requisitos de build que se bajan como wheels, y los hallazgos de los que no se pueden bajar.
type IndiceBuild = (Vec<String>, Vec<Hallazgo>);

/// Índice de build de varias fuentes, cada una con su lista de requisitos: une sus requisitos sin repetir y
/// en orden, y separa los que no se bajan como wheel. Cada uno de estos da un hallazgo de entrega de
/// severidad 1.
pub fn indice_de_build(listas: &[Vec<String>]) -> IndiceBuild {
    let mut unicos: Vec<String> = Vec::new();
    for lista in listas {
        for spec in lista {
            if !unicos.contains(spec) {
                unicos.push(spec.clone());
            }
        }
    }
    let mut aceptables = Vec::new();
    let mut hallazgos = Vec::new();
    for spec in unicos {
        if requisito_de_build_aceptable(&spec) {
            aceptables.push(spec);
        } else {
            let motivo = if spec.contains("://") || spec.contains('@') {
                "requisito de build con referencia directa"
            } else {
                "requisito de build no soportado"
            };
            hallazgos.push(hallazgo(
                ARCHIVO_PYPROJECT,
                0,
                "",
                1,
                format!("{motivo}: {}", redactar(&spec)),
            ));
        }
    }
    (aceptables, hallazgos)
}

/// Comando del contenedor de descarga que lee los requisitos de build de los sdists de `/cache`.
/// El script corre con `python3 -I`: no toma módulos del directorio de trabajo.
pub fn comando_requisitos_sdist() -> String {
    format!("python3 -I -c {}", comilla(GUION_REQUISITOS_SDIST))
}

/// Nombres de los sdists (`.tar.gz` y `.zip`) de la caché `carpeta`, ordenados.
fn nombres_sdist(carpeta: &Path) -> Result<Vec<String>, ErrorOraculo> {
    let mut sdists = Vec::new();
    for entrada in fs::read_dir(carpeta).map_err(|e| infra("leer la caché de Python", e))? {
        let entrada = entrada.map_err(|e| infra("leer la caché de Python", e))?;
        let nombre = entrada.file_name().to_string_lossy().into_owned();
        if nombre.ends_with(".tar.gz") || nombre.ends_with(".zip") {
            sdists.push(nombre);
        }
    }
    sdists.sort();
    Ok(sdists)
}

/// `true` si una caché con sdists trae sus tres JSON: requisitos, dependencias y metadata vieja. Una caché anterior,
/// sin alguno, no sirve de acierto: se descarga de nuevo.
fn requisitos_sdist_listos(carpeta: &Path) -> Result<bool, ErrorOraculo> {
    Ok(nombres_sdist(carpeta)?.is_empty()
        || (carpeta.join(ARCHIVO_REQUISITOS_SDIST).is_file()
            && carpeta.join(ARCHIVO_DEPENDENCIAS_SDIST).is_file()
            && carpeta.join(ARCHIVO_METADATA_VIEJA_SDIST).is_file()))
}

/// Nombres de los sdists con metadata anterior a 2.1, según el JSON del contenedor. Sin el archivo, o si no
/// parsea, es infraestructura.
fn metadata_vieja_de_sdists(carpeta: &Path) -> Result<BTreeSet<String>, ErrorOraculo> {
    let texto = fs::read_to_string(carpeta.join(ARCHIVO_METADATA_VIEJA_SDIST))
        .map_err(|e| infra("leer la lista de metadata vieja de los sdists", e))?;
    let nombres: Vec<String> = serde_json::from_str(&texto).map_err(|e| {
        ErrorOraculo::Infra(format!(
            "JSON de metadata vieja ilegible ({ARCHIVO_METADATA_VIEJA_SDIST}): {e}"
        ))
    })?;
    Ok(nombres.into_iter().collect())
}

/// Lee el JSON `archivo` del contenedor de sdists: nombre del archivo → lista o `null`. Sin el archivo, o si no
/// parsea, es infraestructura.
fn leer_mapa_sdist(
    carpeta: &Path,
    archivo: &str,
) -> Result<BTreeMap<String, Option<Vec<String>>>, ErrorOraculo> {
    let texto = fs::read_to_string(carpeta.join(archivo))
        .map_err(|e| infra("leer el JSON de los sdists", e))?;
    serde_json::from_str(&texto)
        .map_err(|e| ErrorOraculo::Infra(format!("JSON de los sdists ilegible ({archivo}): {e}")))
}

/// Requisitos de build de cada sdist, en el orden de `nombres`, según el JSON del contenedor. Un sdist sin
/// lista (`null`) usa el respaldo de PEP 517. Sin JSON, es infraestructura.
fn requisitos_de_sdists(
    carpeta: &Path,
    nombres: &[String],
) -> Result<Vec<Vec<String>>, ErrorOraculo> {
    let mapa = leer_mapa_sdist(carpeta, ARCHIVO_REQUISITOS_SDIST)?;
    Ok(nombres
        .iter()
        .map(|nombre| match mapa.get(nombre) {
            Some(Some(lista)) => limpios(lista),
            _ => respaldo_de_build(),
        })
        .collect())
}

/// Dependencias de runtime de cada sdist, en el orden de `nombres`. `None` si el sdist no las declara en forma
/// estática, o si el JSON no lo trae.
fn dependencias_de_sdists(
    carpeta: &Path,
    nombres: &[String],
) -> Result<Vec<Option<Vec<String>>>, ErrorOraculo> {
    let mapa = leer_mapa_sdist(carpeta, ARCHIVO_DEPENDENCIAS_SDIST)?;
    Ok(nombres
        .iter()
        .map(|nombre| match mapa.get(nombre) {
            Some(Some(lista)) => Some(limpios(lista)),
            _ => None,
        })
        .collect())
}

/// Ruta de un clon bajo `src/`: `<clave>` o `<clave>/<subdirectory>`.
fn ruta_de_clon(partes: &PartesGit) -> String {
    let dir = clave_clon(partes);
    match &partes.subdir {
        Some(subdir) => format!("{dir}/{subdir}"),
        None => dir,
    }
}

/// Rutas de los clones git de `req` bajo `src/`, sin repetidos.
fn rutas_clon(req: &RequisitosPip) -> Vec<String> {
    let rutas: BTreeSet<String> = req
        .directas
        .iter()
        .filter_map(|(_, raw)| partes_git(raw).ok())
        .map(|partes| ruta_de_clon(&partes))
        .collect();
    rutas.into_iter().collect()
}

/// Nombres normalizados de las dependencias de runtime de los sdists y clones de `carpeta`. Las leen los
/// mismos JSON y `pyproject` que el índice de build. Sin dependencias estáticas, da un conjunto vacío.
fn nombres_de_runtime(
    carpeta: &Path,
    req: &RequisitosPip,
) -> Result<BTreeSet<String>, ErrorOraculo> {
    let mut specs: Vec<String> = Vec::new();
    let sdists = nombres_sdist(carpeta)?;
    if !sdists.is_empty() {
        for deps in dependencias_de_sdists(carpeta, &sdists)?
            .into_iter()
            .flatten()
        {
            specs.extend(deps);
        }
    }
    for ruta in rutas_clon(req) {
        let pyproject = pyproject_de_clon(carpeta, &ruta);
        specs.extend(dependencias_de_clon(pyproject.as_deref()).unwrap_or_default());
    }
    Ok(specs.iter().map(|spec| nombre_de_spec(spec)).collect())
}

/// Índice de build de las fuentes que necesitan build en `carpeta`: los sdists (con sus requisitos de build y sus
/// dependencias de runtime, que deja el contenedor) y los clones en `rutas_clon` (con el `pyproject` de cada uno
/// o de su subdirectorio). `None` si no hay ninguno, porque los wheels no necesitan build.
fn indice_de_build_de(
    carpeta: &Path,
    rutas_clon: &[String],
) -> Result<Option<IndiceBuild>, ErrorOraculo> {
    let sdists = nombres_sdist(carpeta)?;
    if sdists.is_empty() && rutas_clon.is_empty() {
        return Ok(None);
    }
    let mut listas = Vec::new();
    if !sdists.is_empty() {
        listas.extend(requisitos_de_sdists(carpeta, &sdists)?);
        // Un sdist sin lista estática ya cortó la descarga (`fuentes_dinamicas`): aquí solo suma sus listas.
        listas.extend(
            dependencias_de_sdists(carpeta, &sdists)?
                .into_iter()
                .map(Option::unwrap_or_default),
        );
    }
    for ruta in rutas_clon {
        let pyproject = pyproject_de_clon(carpeta, ruta);
        listas.push(requisitos_de_build(pyproject.as_deref()));
        listas.push(dependencias_de_clon(pyproject.as_deref()).unwrap_or_default());
    }
    Ok(Some(indice_de_build(&listas)))
}

/// Nombres de las fuentes de build cuyas dependencias de runtime no se declaran en forma estática: un sdist sin
/// lista en `dependencias-sdist.json`, o un clon con `[project].dependencies` dinámico o mal formado. Un clon sin
/// `pyproject.toml` legible no corta. Sin nombres, la descarga sigue.
fn fuentes_dinamicas(carpeta: &Path, req: &RequisitosPip) -> Result<Vec<String>, ErrorOraculo> {
    let sdists = nombres_sdist(carpeta)?;
    let mut dinamicas = Vec::new();
    if !sdists.is_empty() {
        let dependencias = dependencias_de_sdists(carpeta, &sdists)?;
        for (nombre, deps) in sdists.iter().zip(dependencias) {
            if deps.is_none() {
                dinamicas.push(nombre.clone());
            }
        }
    }
    for (_, raw) in &req.directas {
        let Ok(partes) = partes_git(raw) else {
            continue;
        };
        let pyproject = pyproject_de_clon(carpeta, &ruta_de_clon(&partes));
        if dependencias_de_clon(pyproject.as_deref()).is_none() {
            dinamicas.push(redactar(&partes.base));
        }
    }
    Ok(dinamicas)
}

/// Texto fijo de los avisos de dependencias no verificadas. No corta la descarga: es severidad 1.
const AVISO_NO_VERIFICADAS: &str = "dependencias de runtime no verificadas";

/// `true` si un clon no declara dependencias en un `pyproject.toml` con `[project]`. Sin el archivo, o con un
/// TOML sin `[project]` (un `setup.py` legado), es legado. Un TOML roto no cuenta: `dependencias_de_clon` lo corta.
fn clon_sin_metadatos_estaticos(pyproject: Option<&str>) -> bool {
    match pyproject {
        None => true,
        Some(texto) => {
            toml::from_str::<toml::Table>(texto).is_ok_and(|tabla| !tabla.contains_key("project"))
        }
    }
}

/// Hallazgos de dependencias no verificadas: sdists con metadata anterior a 2.1 y clones sin metadatos estáticos.
/// Leen los archivos de `carpeta`, así que sirven igual en una descarga nueva y en un acierto de caché.
fn hallazgos_de_dependencias_no_verificadas(
    carpeta: &Path,
    req: &RequisitosPip,
) -> Result<Vec<Hallazgo>, ErrorOraculo> {
    let mut hallazgos = Vec::new();
    let sdists = nombres_sdist(carpeta)?;
    if !sdists.is_empty() {
        let viejas = metadata_vieja_de_sdists(carpeta)?;
        for nombre in sdists.iter().filter(|n| viejas.contains(*n)) {
            hallazgos.push(hallazgo(
                ARCHIVO_REQUIREMENTS,
                0,
                nombre,
                1,
                format!("sdist con metadatos anteriores a 2.1: {AVISO_NO_VERIFICADAS}: {nombre}"),
            ));
        }
    }
    for (linea, raw) in &req.directas {
        let Ok(partes) = partes_git(raw) else {
            continue;
        };
        let pyproject = pyproject_de_clon(carpeta, &ruta_de_clon(&partes));
        if clon_sin_metadatos_estaticos(pyproject.as_deref()) {
            hallazgos.push(hallazgo(
                ARCHIVO_REQUIREMENTS,
                *linea,
                &redactar(raw),
                1,
                format!(
                    "clon sin metadatos estáticos: {AVISO_NO_VERIFICADAS}: {}",
                    redactar(&partes.base)
                ),
            ));
        }
    }
    Ok(hallazgos)
}

/// Suma al Gate el texto de cada hallazgo de dependencias no verificadas.
fn con_avisos_de(mut registro: Registro, hallazgos: &[Hallazgo]) -> Registro {
    registro
        .avisos
        .extend(hallazgos.iter().map(|h| h.detalle.clone()));
    registro
}

/// Texto del `pyproject.toml` de un clon bajo `carpeta/src/<ruta>`. No sigue enlaces: cada componente de la
/// ruta debe ser carpeta real, y el archivo debe ser regular. Pasa el tope de tamaño, no se lee. Si no cumple,
/// da `None`, como si no existiera.
fn pyproject_de_clon(carpeta: &Path, ruta: &str) -> Option<String> {
    let mut base = carpeta.to_path_buf();
    for componente in Path::new("src").join(ruta).components() {
        let Component::Normal(nombre) = componente else {
            return None;
        };
        base.push(nombre);
        if !fs::symlink_metadata(&base).ok()?.is_dir() {
            return None;
        }
    }
    let archivo = base.join(ARCHIVO_PYPROJECT);
    if !fs::symlink_metadata(&archivo).ok()?.is_file() {
        return None;
    }
    let mut texto = Vec::new();
    fs::File::open(&archivo)
        .ok()?
        .take(TOPE_PYPROJECT_CLON + 1)
        .read_to_end(&mut texto)
        .ok()?;
    if texto.len() as u64 > TOPE_PYPROJECT_CLON {
        return None;
    }
    String::from_utf8(texto).ok()
}

/// Ejecución del contenedor de descarga de Python: sin `/repo`, con las copias de los requisitos en `/trabajo`.
fn ejecucion_python(
    imagen: &str,
    trabajo: &Path,
    parcial: &Path,
    salida: &Path,
    cmd: String,
    timeout: Duration,
) -> Ejecucion {
    Ejecucion {
        imagen: imagen.into(),
        repo: None,
        oraculo: None,
        oraculo_destino: None,
        salida: salida.to_path_buf(),
        cmd,
        env: entorno_descarga(),
        timeout,
        red: true,
        cache: Some(parcial.to_path_buf()),
        cache_rw: true,
        rustsec: None,
        trabajo: Some(trabajo.to_path_buf()),
        secretos: Vec::new(),
    }
}

/// Copia los `requirements*.txt` de la raíz del repo a `destino`. Nada más del repo entra al contenedor.
fn copiar_requisitos(repo: &Path, destino: &Path) -> std::io::Result<()> {
    for entrada in fs::read_dir(repo)? {
        let entrada = entrada?;
        let nombre = entrada.file_name().to_string_lossy().into_owned();
        // Un enlace no cuenta como archivo: no se copia lo que apunta fuera del repo.
        let es_archivo = entrada.file_type()?.is_file();
        if es_archivo && nombre.starts_with("requirements") && nombre.ends_with(".txt") {
            fs::copy(entrada.path(), destino.join(&nombre))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::registrar;
    use super::*;
    use crate::config::RegistroConfig;
    use crate::oracle::contenedor::{Resultado, SalidaPodman};
    use std::cell::RefCell;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::process::Command;

    /// Lista de build que dan los respaldos de PEP 517.
    fn respaldo() -> Vec<String> {
        vec!["setuptools".into(), "wheel".into()]
    }

    #[test]
    fn descarga_requisitos_de_build_sin_pyproject_son_setuptools_y_wheel() {
        assert_eq!(requisitos_de_build(None), respaldo());
    }

    #[test]
    fn descarga_requisitos_de_build_lee_la_lista_de_build_system() {
        assert_eq!(
            requisitos_de_build(Some(
                "[build-system]\nrequires = [\"setuptools>=68\", \"wheel\"]"
            )),
            ["setuptools>=68", "wheel"]
        );
        assert_eq!(
            requisitos_de_build(Some("[build-system]\nrequires = [\"hatchling\"]")),
            ["hatchling"]
        );
    }

    #[test]
    fn descarga_requisitos_de_build_recorta_y_descarta_vacios() {
        assert_eq!(
            requisitos_de_build(Some(
                "[build-system]\nrequires = [\" setuptools>=68 \", \"  \", \"wheel\"]"
            )),
            ["setuptools>=68", "wheel"]
        );
    }

    #[test]
    fn descarga_requisitos_de_build_sin_build_system_o_roto_son_el_respaldo() {
        assert_eq!(
            requisitos_de_build(Some("[project]\nname = \"x\"")),
            respaldo()
        );
        assert_eq!(requisitos_de_build(Some("no es toml [[[")), respaldo());
    }

    #[test]
    fn descarga_requisito_de_build_aceptable_rechaza_referencias_directas() {
        assert!(requisito_de_build_aceptable("setuptools>=68"));
        assert!(!requisito_de_build_aceptable("foo @ https://x/y.tar.gz"));
        assert!(!requisito_de_build_aceptable("foo @ x"));
        assert!(!requisito_de_build_aceptable("git+https://x/y.git"));
        assert!(!requisito_de_build_aceptable("-e ."));
    }

    #[test]
    fn descarga_requisito_de_build_aceptable_rechaza_opciones_pegadas() {
        for spec in [
            "foo==1.0 --no-binary :all:",
            "foo==1.0\n--no-binary :all:",
            "foo==1.0\r-e .",
            "foo\t-r otro.txt",
        ] {
            assert!(!requisito_de_build_aceptable(spec), "{spec:?}");
        }
        assert!(requisito_de_build_aceptable("requests>=2"));
        assert!(requisito_de_build_aceptable("foo[extra]==1.0"));
        assert!(requisito_de_build_aceptable(
            "numpy>=1.20,<2; python_version>='3.8'"
        ));
    }

    #[test]
    fn descarga_indice_de_build_opcion_pegada_no_se_baja() {
        let listas = vec![vec![
            "foo==1.0 --no-binary :all:".to_string(),
            "wheel".to_string(),
        ]];
        let (requisitos, hallazgos) = indice_de_build(&listas);
        assert_eq!(requisitos, ["wheel"]);
        assert_eq!(hallazgos.len(), 1, "{hallazgos:?}");
    }

    #[test]
    fn descarga_requisitos_sdist_se_leen_en_el_contenedor_con_tarfile_y_zipfile() {
        let cmd = comando_requisitos_sdist();
        assert!(cmd.starts_with("python3 -I -c "), "{cmd}");
        for esperado in [
            "tarfile",
            "zipfile",
            "tomllib",
            "/cache/requisitos-sdist.json",
            "/cache/dependencias-sdist.json",
        ] {
            assert!(cmd.contains(esperado), "falta {esperado}: {cmd}");
        }
        // Nada del sdist se abre con `tar` en el host: el comando solo corre dentro del contenedor.
        assert!(!cmd.contains("tar -"), "{cmd}");
    }

    #[test]
    fn descarga_requisitos_de_sdists_leen_el_json_del_contenedor() {
        let dir = tempfile::tempdir().expect("temp dir");
        fs::write(
            dir.path().join(ARCHIVO_REQUISITOS_SDIST),
            r#"{"a-1.0.tar.gz": ["hatchling", " wheel "], "b-1.0.zip": null}"#,
        )
        .expect("json");
        let nombres = ["a-1.0.tar.gz".to_string(), "b-1.0.zip".to_string()];
        let listas = requisitos_de_sdists(dir.path(), &nombres).expect("lee el json");
        // El host recorta y descarta vacíos; un sdist sin lista usa el respaldo de PEP 517.
        assert_eq!(
            listas,
            vec![
                vec!["hatchling".to_string(), "wheel".to_string()],
                respaldo()
            ]
        );
    }

    #[test]
    fn descarga_requisitos_de_sdists_sin_json_es_infraestructura() {
        let dir = tempfile::tempdir().expect("temp dir");
        let r = requisitos_de_sdists(dir.path(), &["a-1.0.tar.gz".to_string()]);
        assert!(matches!(r, Err(ErrorOraculo::Infra(_))), "{r:?}");
    }

    #[test]
    fn descarga_cache_con_sdist_sin_requisitos_no_es_acierto() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert!(
            requisitos_sdist_listos(dir.path()).expect("lee"),
            "sin sdists"
        );
        fs::write(dir.path().join("a-1.0.tar.gz"), "x").expect("sdist");
        assert!(
            !requisitos_sdist_listos(dir.path()).expect("lee"),
            "un sdist sin json viene de una caché anterior"
        );
        fs::write(dir.path().join(ARCHIVO_REQUISITOS_SDIST), "{}").expect("json");
        assert!(
            !requisitos_sdist_listos(dir.path()).expect("lee"),
            "sin dependencias de runtime, la caché es anterior"
        );
        fs::write(dir.path().join(ARCHIVO_DEPENDENCIAS_SDIST), "{}").expect("json");
        assert!(
            !requisitos_sdist_listos(dir.path()).expect("lee"),
            "sin la metadata vieja, la caché es anterior: no daría el aviso"
        );
        fs::write(dir.path().join(ARCHIVO_METADATA_VIEJA_SDIST), "[]").expect("json");
        assert!(requisitos_sdist_listos(dir.path()).expect("lee"));
    }

    #[test]
    fn descarga_indice_de_build_une_sin_repetidos_exactos() {
        let listas = vec![
            vec!["setuptools>=68".to_string(), "wheel".to_string()],
            vec!["wheel".to_string()],
            respaldo(),
        ];
        let (requisitos, hallazgos) = indice_de_build(&listas);
        assert_eq!(requisitos, ["setuptools>=68", "wheel", "setuptools"]);
        assert!(hallazgos.is_empty());
    }

    #[test]
    fn descarga_indice_de_build_referencia_directa_da_hallazgo_y_no_se_baja() {
        let listas = vec![vec![
            "foo @ https://usuario:clave@x/y.tar.gz".to_string(),
            "wheel".to_string(),
        ]];
        let (requisitos, hallazgos) = indice_de_build(&listas);
        assert_eq!(requisitos, ["wheel"]);
        assert_eq!(hallazgos.len(), 1, "{hallazgos:?}");
        let h = &hallazgos[0];
        assert_eq!(h.severidad, 1);
        assert_eq!(h.archivo, "pyproject.toml");
        assert_eq!(h.linea, 0);
        assert!(
            h.detalle
                .starts_with("requisito de build con referencia directa: "),
            "{}",
            h.detalle
        );
        assert!(!h.detalle.contains("clave"), "{}", h.detalle);
    }

    /// Clon temporal con `src/clon/` vacío. Devuelve la carpeta y la carpeta de afuera del clon.
    fn clon_de_prueba() -> (tempfile::TempDir, tempfile::TempDir) {
        let carpeta = tempfile::tempdir().expect("carpeta");
        let afuera = tempfile::tempdir().expect("afuera");
        fs::create_dir_all(carpeta.path().join("src").join("clon")).expect("crea el clon");
        (carpeta, afuera)
    }

    #[test]
    fn descarga_pyproject_de_clon_symlink_fuera_del_clon_no_sale() {
        let (carpeta, afuera) = clon_de_prueba();
        fs::write(
            afuera.path().join("pyproject.toml"),
            "[build-system]\nrequires = [\"pkg-de-afuera\"]\n",
        )
        .expect("escribe el archivo de afuera");
        std::os::unix::fs::symlink(
            afuera.path().join("pyproject.toml"),
            carpeta.path().join("src/clon/pyproject.toml"),
        )
        .expect("enlace");
        let (requisitos, _) = indice_de_build_de(carpeta.path(), &["clon".to_string()])
            .expect("indice")
            .expect("hay build");
        assert_eq!(requisitos, respaldo());
    }

    #[test]
    fn descarga_pyproject_de_clon_componente_intermedio_symlink_no_se_sigue() {
        let (carpeta, afuera) = clon_de_prueba();
        fs::write(
            afuera.path().join("pyproject.toml"),
            "[build-system]\nrequires = [\"pkg-de-afuera\"]\n",
        )
        .expect("escribe el archivo de afuera");
        let clon = carpeta.path().join("src/clon");
        fs::remove_dir(&clon).expect("quita el clon");
        std::os::unix::fs::symlink(afuera.path(), &clon).expect("enlace al directorio");
        let (requisitos, _) = indice_de_build_de(carpeta.path(), &["clon".to_string()])
            .expect("indice")
            .expect("hay build");
        assert_eq!(requisitos, respaldo());
    }

    #[test]
    fn descarga_pyproject_de_clon_sobre_el_tope_no_se_lee() {
        let (carpeta, _afuera) = clon_de_prueba();
        let relleno = "#".repeat(300 * 1024);
        fs::write(
            carpeta.path().join("src/clon/pyproject.toml"),
            format!("{relleno}\n[build-system]\nrequires = [\"hatchling\"]\n"),
        )
        .expect("escribe el pyproject grande");
        let (requisitos, _) = indice_de_build_de(carpeta.path(), &["clon".to_string()])
            .expect("indice")
            .expect("hay build");
        assert_eq!(requisitos, respaldo());
    }

    #[test]
    fn descarga_pyproject_de_clon_regular_normal_se_lee() {
        let (carpeta, _afuera) = clon_de_prueba();
        fs::write(
            carpeta.path().join("src/clon/pyproject.toml"),
            "[build-system]\nrequires = [\"hatchling\"]\n",
        )
        .expect("escribe el pyproject");
        let (requisitos, _) = indice_de_build_de(carpeta.path(), &["clon".to_string()])
            .expect("indice")
            .expect("hay build");
        assert_eq!(requisitos, ["hatchling"]);
    }

    #[test]
    fn descarga_python_contenedor_no_monta_repo_y_recibe_la_copia_de_requisitos() {
        let e = ejecucion_python(
            "img:1",
            Path::new("/h/tmp/trabajo"),
            Path::new("/h/cache/parcial"),
            Path::new("/h/tmp/salida"),
            "pip download".into(),
            Duration::from_secs(60),
        );
        let args = contenedor::argumentos(&e, "n");
        assert!(!args.iter().any(|a| a.contains(":/repo")), "{args:?}");
        assert!(
            args.contains(&"/h/tmp/trabajo:/trabajo:ro,z".to_string()),
            "{args:?}"
        );
        assert!(
            args.contains(&"/h/cache/parcial:/cache:z".to_string()),
            "{args:?}"
        );
    }

    #[test]
    fn descarga_python_copia_solo_los_requirements_de_la_raiz() {
        let repo = tempfile::tempdir().expect("repo");
        let destino = tempfile::tempdir().expect("trabajo");
        for (ruta, texto) in [
            ("requirements.txt", "requests==2.31.0\n"),
            ("requirements-dev.txt", "pytest==8.0.0\n"),
            ("setup.py", "raise SystemExit(1)\n"),
            ("sub/requirements.txt", "otro==1.0\n"),
        ] {
            let p = repo.path().join(ruta);
            fs::create_dir_all(p.parent().expect("padre")).expect("carpeta");
            fs::write(p, texto).expect("escribe");
        }
        copiar_requisitos(repo.path(), destino.path()).expect("copia");
        let mut nombres: Vec<String> = fs::read_dir(destino.path())
            .expect("lee")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        nombres.sort();
        assert_eq!(nombres, ["requirements-dev.txt", "requirements.txt"]);
    }

    #[test]
    fn descarga_python_referencia_git_sin_commit_o_con_tag_se_rechaza_igual() {
        let sin = motivo_git("git+https://github.com/x/y#egg=y");
        let tag = motivo_git("git+https://github.com/x/y@v1.2.0#egg=y");
        assert!(sin.is_some() && tag.is_some(), "{sin:?} {tag:?}");
        assert_eq!(sin, tag, "la misma regla da el mismo motivo");
        assert!(
            motivo_git("git+https://github.com/x/y@c8e394065cd541a16c040515dc0afb85cf22a7c3#egg=y")
                .is_none()
        );
    }

    #[test]
    fn descarga_python_clave_de_cache_incluye_la_imagen_y_su_digest() {
        let a = clave_cache_python("six==1.16.0\n", "", "python:3.8", None);
        let b = clave_cache_python("six==1.16.0\n", "", "python:3.12", None);
        let c = clave_cache_python("six==1.16.0\n", "", "python:3.8", None);
        assert_ne!(a, b, "wheels cp38 y cp312 no comparten caché");
        assert_eq!(a, c, "misma imagen y mismo archivo: misma clave");
        let d = clave_cache_python("six==1.16.0\n", "", "python@sha256:aaaa", None);
        assert_ne!(a, d, "otro digest es otra clave");
        let e = clave_cache_python("six==1.16.0\n", "urllib3<2\n", "python:3.8", None);
        assert_ne!(a, e, "una restricción distinta es otra clave");
    }

    /// Fórmula de la clave antes de H5: sin versión de formato.
    fn clave_vieja(
        requisitos: &str,
        restricciones: &str,
        imagen: &str,
        digest: Option<&str>,
    ) -> String {
        let digest = digest.map_or(String::new(), |d| format!("digest={d}\n"));
        hash_corto(
            format!(
                "imagen={imagen}\n{digest}--requisitos--\n{requisitos}--restricciones--\n{restricciones}"
            )
            .as_bytes(),
        )
    }

    #[test]
    fn descarga_python_cache_con_clave_vieja_no_es_acierto() {
        // H5 cambió la clave. Una caché guardada con la fórmula vieja no debe usarse.
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        fs::write(repo.path().join("requirements.txt"), "six==1.16.0\n").expect("escribe");
        let (texto, restricciones) =
            textos_de_requisitos(repo.path()).expect("textos de requisitos");
        let vieja = clave_vieja(&texto, &restricciones, "python:3.8", None);
        let destino = home
            .path()
            .join(CARPETA_CACHE)
            .join(CARPETA_PYTHON)
            .join(vieja);
        fs::create_dir_all(&destino).expect("caché vieja");
        fs::write(destino.join(ARCHIVO_OFFLINE), "six==1.16.0\n").expect("offline");
        let tarea = tarea_python_de_prueba();
        let podman = PodmanDigest::que_responde(125, "");
        let deps = DependenciasPython {
            podman: &podman,
            ejecutor: &EjecutorFalso::que_responde(0, ""),
            leer_env: &|_: &str| None,
        };
        let (_, registro) = preparar_python_con(
            repo.path(),
            &tarea,
            home.path(),
            Duration::from_secs(60),
            &[],
            &deps,
        )
        .expect("prepara");
        let registro = registro.expect("hay registro de la descarga");
        assert!(
            !registro.cache_hit,
            "una caché con la clave vieja no es acierto"
        );
    }

    #[test]
    fn descarga_python_clave_de_cache_es_estable_y_distinta_de_la_vieja() {
        let req = "six==1.16.0\n";
        let nueva = clave_cache_python(req, "", "python:3.8", None);
        assert_eq!(
            nueva,
            clave_cache_python(req, "", "python:3.8", None),
            "mismas entradas: misma clave"
        );
        assert_ne!(
            nueva,
            clave_vieja(req, "", "python:3.8", None),
            "la versión de formato cambia la clave"
        );
    }

    /// Ejecutor de podman falso: responde un código y una salida fijos, y guarda los argumentos.
    struct PodmanDigest {
        codigo: i32,
        stdout: String,
        /// Código de `podman image exists`: cero si la imagen está local.
        existe: i32,
        llamadas: std::cell::RefCell<Vec<Vec<String>>>,
    }

    impl PodmanDigest {
        /// Responde `codigo` y `stdout` al digest, y da 0 a `image exists`: la imagen está local.
        fn que_responde(codigo: i32, stdout: &str) -> Self {
            PodmanDigest::con_imagen(0, codigo, stdout)
        }

        fn con_imagen(existe: i32, codigo: i32, stdout: &str) -> Self {
            PodmanDigest {
                codigo,
                stdout: stdout.into(),
                existe,
                llamadas: std::cell::RefCell::new(Vec::new()),
            }
        }
    }

    impl Podman for PodmanDigest {
        fn correr(&self, args: &[&str], _plazo: Duration) -> Result<SalidaPodman, ErrorOraculo> {
            self.llamadas
                .borrow_mut()
                .push(args.iter().map(|a| a.to_string()).collect());
            let (codigo, stdout) = if args.starts_with(&["image", "exists"]) {
                (self.existe, String::new())
            } else {
                (self.codigo, self.stdout.clone())
            };
            Ok(SalidaPodman {
                codigo,
                stdout,
                stderr: String::new(),
            })
        }
    }

    /// Podman que no arranca: cada llamada falla antes de correr nada.
    struct PodmanRoto;

    impl Podman for PodmanRoto {
        fn correr(&self, _args: &[&str], _plazo: Duration) -> Result<SalidaPodman, ErrorOraculo> {
            Err(ErrorOraculo::Infra("no arranca podman".into()))
        }
    }

    #[test]
    fn descarga_python_args_del_digest_son_los_del_image_inspect() {
        assert_eq!(
            args_digest_imagen("python:3.8"),
            vec!["image", "inspect", "--format", "{{.Digest}}", "python:3.8"]
        );
    }

    #[test]
    fn descarga_python_mismo_nombre_con_digest_a_o_b_da_claves_distintas() {
        let req = "six==1.16.0\n";
        let a = clave_cache_python(req, "", "python:3.8", Some("sha256:aaaa"));
        let b = clave_cache_python(req, "", "python:3.8", Some("sha256:bbbb"));
        let sin = clave_cache_python(req, "", "python:3.8", None);
        assert_ne!(
            a, b,
            "un tag reconstruido con otro digest no comparte wheels"
        );
        assert_ne!(a, sin, "con digest no es la clave de solo la imagen");
    }

    #[test]
    fn descarga_python_digest_del_ejecutor_entra_en_la_clave() {
        let podman = PodmanDigest::que_responde(0, "sha256:aaaa\n");
        let digest = digest_imagen_con(&podman, "python:3.8");
        assert_eq!(digest.as_deref(), Some("sha256:aaaa"));
        assert_eq!(
            podman.llamadas.borrow()[0],
            args_digest_imagen("python:3.8"),
            "el ejecutor recibe el image inspect"
        );
        assert_eq!(
            clave_cache_python("six==1.16.0\n", "", "python:3.8", digest.as_deref()),
            clave_cache_python("six==1.16.0\n", "", "python:3.8", Some("sha256:aaaa")),
            "el digest que devuelve podman es el que entra en la clave"
        );
    }

    #[test]
    fn descarga_python_digest_que_falla_es_none_sin_romper() {
        let sale_mal = PodmanDigest::que_responde(125, "");
        assert_eq!(digest_imagen_con(&sale_mal, "python:3.8"), None);
        let vacio = PodmanDigest::que_responde(0, "\n");
        assert_eq!(digest_imagen_con(&vacio, "python:3.8"), None);
    }

    #[test]
    fn descarga_python_dependencia_sin_wheel_da_pendiente_solo_sdist() {
        // M9 (desempate 31): pip no halla wheel de B, que es dependencia de A. Root no construye sdists.
        let stderr = "ERROR: No matching distribution found for B==1.0\n";
        let pendientes = ["A==2.0".to_string()];
        let spec = sin_wheel(stderr).expect("pip dice qué paquete falta");
        let directo = pendientes
            .iter()
            .any(|s| nombre_de_spec(s) == nombre_de_spec(&spec));
        assert!(!directo, "B no es requisito directo");
        let Fin::Corta(aviso, causa) = corte_sin_wheel(&spec, directo) else {
            panic!("una dependencia sin wheel corta");
        };
        assert_eq!(causa, "solo-sdist");
        assert_eq!(
            aviso.detalle,
            "B solo tiene sdist y la descarga no construye sdists"
        );
    }

    #[test]
    fn descarga_python_requisito_directo_sin_version_sigue_en_sin_version_fijada() {
        let Fin::Corta(_, causa) = corte_sin_wheel("A", true) else {
            panic!("un requisito directo sin versión corta");
        };
        assert_eq!(causa, "sin-version-fijada");
    }

    #[test]
    fn descarga_version_fijada_comodin_no_es_version_fijada() {
        assert_eq!(version_fijada("foo==1.*"), None);
        assert_eq!(version_fijada("foo==1.2.*"), None);
    }

    #[test]
    fn descarga_version_fijada_exacta_y_sin_fijar() {
        assert_eq!(version_fijada("foo==1.2.3"), Some("1.2.3".to_string()));
        assert_eq!(version_fijada("foo>=1"), None);
    }

    /// Tarea de Python mínima escrita en un directorio temporal.
    fn tarea_python_de_prueba() -> Tarea {
        let dir = tempfile::tempdir().expect("temp dir");
        let texto = "nombre = \"x\"\nlenguaje = \"python\"\nimagen = \"python:3.8\"\nspec = \"s\"\n\
                     [contrato]\ncmd = \"c\"\n[oculto]\ncmd = \"o\"\n";
        fs::write(dir.path().join("tarea.toml"), texto).expect("escribe tarea.toml");
        Tarea::cargar_de(dir.path()).expect("carga la tarea")
    }

    #[test]
    fn descarga_python_sin_digest_no_es_hallazgo_del_producto() {
        // Desempate del organizador: un digest que no se lee es falla del instrumento, no del producto.
        // La caché ya está (clave con digest None): la descarga sale por el camino de acierto, sin podman.
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        fs::write(repo.path().join("requirements.txt"), "six==1.16.0\n").expect("escribe");
        let tarea = tarea_python_de_prueba();
        let (texto, restricciones) =
            textos_de_requisitos(repo.path()).expect("textos de requisitos");
        let clave = clave_cache_python(&texto, &restricciones, "python:3.8", None);
        let destino = home
            .path()
            .join(CARPETA_CACHE)
            .join(CARPETA_PYTHON)
            .join(clave);
        fs::create_dir_all(&destino).expect("caché");
        fs::write(destino.join(ARCHIVO_OFFLINE), "six==1.16.0\n").expect("offline");
        let podman = PodmanDigest::que_responde(125, "");
        let deps = DependenciasPython {
            podman: &podman,
            ejecutor: &EjecutorFalso::que_responde(0, ""),
            leer_env: &|_: &str| None,
        };
        let (preparacion, registro) = preparar_python_con(
            repo.path(),
            &tarea,
            home.path(),
            Duration::from_secs(60),
            &[],
            &deps,
        )
        .expect("prepara");
        match preparacion {
            Preparacion::Lista { hallazgos, .. } => {
                assert!(
                    hallazgos.is_empty(),
                    "el digest no es hallazgo: {hallazgos:?}"
                );
            }
            otro => panic!("debía dar Lista, dio {otro:?}"),
        }
        let registro = registro.expect("hay registro de la descarga");
        assert_eq!(registro.sin_digest.as_deref(), Some("python:3.8"));
        registrar(home.path(), &tarea, &registro).expect("registra el Gate");
        let eventos = fs::read_to_string(home.path().join("eventos.jsonl")).expect("eventos");
        let evento: Value = eventos
            .lines()
            .map(|l| serde_json::from_str::<Value>(l).expect("línea JSON"))
            .find(|v| v["tipo"] == "gate" && v["fase"] == "descarga")
            .expect("evento Gate de la descarga");
        assert_eq!(
            evento["extra"]["sin_digest"],
            "no se leyó el digest de python:3.8: la caché de Python usa solo su nombre"
        );
    }

    #[test]
    fn descarga_python_aviso_sin_digest_nombra_la_imagen() {
        // El aviso va al Gate con el nombre de la imagen, no a los hallazgos del veredicto.
        let home = tempfile::tempdir().expect("home");
        let tarea = tarea_python_de_prueba();
        let mut registro = Registro::sin_paquetes("python");
        registro.sin_digest = Some("python:3.8".into());
        registrar(home.path(), &tarea, &registro).expect("registra el Gate");
        let eventos = fs::read_to_string(home.path().join("eventos.jsonl")).expect("eventos");
        assert!(
            eventos.contains(
                "no se leyó el digest de python:3.8: la caché de Python usa solo su nombre"
            ),
            "{eventos}"
        );
    }

    #[test]
    fn descarga_python_subdirectory_se_conserva_y_dos_lineas_del_mismo_repo_no_chocan() {
        let base = "git+https://github.com/x/mono@c8e394065cd541a16c040515dc0afb85cf22a7c3";
        let a = partes_git(&format!("{base}#subdirectory=pkg_a")).expect("parsea");
        let b = partes_git(&format!("{base}#egg=pkg_b&subdirectory=pkg_b")).expect("parsea");
        assert_eq!(a.subdir.as_deref(), Some("pkg_a"));
        assert_eq!(b.subdir.as_deref(), Some("pkg_b"));
        let clon = clave_clon(&a);
        assert_eq!(clon, clave_clon(&b), "mismo repo y commit: un solo clon");
        assert_eq!(
            linea_offline_git(&clon, a.subdir.as_deref()),
            format!("/tmp/fuentes/{clon}/pkg_a")
        );
        assert_ne!(
            linea_offline_git(&clon, a.subdir.as_deref()),
            linea_offline_git(&clon, b.subdir.as_deref())
        );
    }

    #[test]
    fn descarga_python_subdirectory_insegura_es_error() {
        for sub in ["../x", "/abs", "a/../../b", "a b", ""] {
            let raw = format!(
                "git+https://github.com/x/mono@c8e394065cd541a16c040515dc0afb85cf22a7c3#subdirectory={sub}"
            );
            assert!(
                partes_git(&raw).is_err(),
                "debe rechazar subdirectory `{sub}`"
            );
        }
    }

    #[test]
    fn descarga_python_linea_offline_de_git_apunta_a_la_copia_escribible() {
        assert_eq!(linea_offline_git("abc", None), "/tmp/fuentes/abc");
    }

    #[test]
    fn descarga_python_build_de_sdist_con_setuptools_mayor_se_suma_a_los_requisitos() {
        // Guarda (Tareas 6e y 27): el `[build-system].requires` del sdist llega a los wheels de build.
        let listas = vec![requisitos_de_build(Some(
            "[build-system]\nrequires = [\"setuptools>=68\"]",
        ))];
        let (requisitos, hallazgos) = indice_de_build(&listas);
        assert!(
            requisitos.contains(&"setuptools>=68".to_string()),
            "{requisitos:?}"
        );
        assert!(hallazgos.is_empty());
    }

    // --- Tarea 30, parte 3: netrc de pip para los registros de root.toml (decisión 29) ---

    /// Clave falsa del registro de prueba. Ningún texto de Root, ni argumento ni evento, debe mostrarla.
    const CLAVE_FALSA: &str = "clave-falsa-7";

    /// Ejecutor de contenedores falso: guarda cada ejecución, y lo que el primer contenedor ve en `/trabajo`
    /// (el `netrc` de pip, con su contenido y sus permisos). Responde `codigo` y `stderr` fijos.
    struct EjecutorFalso {
        codigo: i32,
        stderr: String,
        ejecuciones: RefCell<Vec<Ejecucion>>,
        netrc_visto: RefCell<Option<(PathBuf, String, u32)>>,
    }

    impl EjecutorFalso {
        fn que_responde(codigo: i32, stderr: &str) -> Self {
            EjecutorFalso {
                codigo,
                stderr: stderr.into(),
                ejecuciones: RefCell::new(Vec::new()),
                netrc_visto: RefCell::new(None),
            }
        }
    }

    impl EjecutorDeDescarga for EjecutorFalso {
        fn ejecutar(&self, ejecucion: &Ejecucion) -> Result<Resultado, ErrorOraculo> {
            self.ejecuciones.borrow_mut().push(ejecucion.clone());
            // Un curl "baja" el wheel de la URL directa del test, para que la descarga siga.
            if ejecucion.cmd.contains("curl")
                && let Some(cache) = &ejecucion.cache
            {
                fs::write(cache.join("x-1.0-py3-none-any.whl"), b"rueda").expect("wheel falso");
            }
            if let Some(trabajo) = &ejecucion.trabajo {
                let ruta = trabajo.join(ARCHIVO_NETRC);
                if self.netrc_visto.borrow().is_none() && ruta.is_file() {
                    let texto = fs::read_to_string(&ruta).expect("lee el netrc");
                    let modo = fs::metadata(&ruta).expect("metadatos").permissions().mode() & 0o777;
                    *self.netrc_visto.borrow_mut() = Some((ruta, texto, modo));
                }
            }
            Ok(Resultado {
                codigo: self.codigo,
                stdout: String::new(),
                stderr: self.stderr.clone(),
                segundos: 0.0,
            })
        }
    }

    /// Registro privado de prueba con su clave en `ROOT_TEST_CLAVE_PIP`.
    fn registro_pip(url: &str, usuario_env: Option<&str>) -> RegistroConfig {
        RegistroConfig {
            nombre: "priv".into(),
            url: url.into(),
            credencial_env: "ROOT_TEST_CLAVE_PIP".into(),
            usuario_env: usuario_env.map(String::from),
        }
    }

    /// Variables de Root de prueba: la clave del registro y su usuario.
    fn entorno_de_prueba(nombre: &str) -> Option<String> {
        match nombre {
            "ROOT_TEST_CLAVE_PIP" => Some(CLAVE_FALSA.into()),
            "ROOT_TEST_USR_PIP" => Some("usuario-falso".into()),
            _ => None,
        }
    }

    /// Corre la descarga de Python de un `requirements.txt` con `texto_req`, contra `ejecutor` y `leer`.
    /// Devuelve también el `home`, para leer sus eventos.
    fn descarga_python_con(
        texto_req: &str,
        registros: &[RegistroConfig],
        ejecutor: &EjecutorFalso,
        leer: &dyn Fn(&str) -> Option<String>,
    ) -> (tempfile::TempDir, Result<Paso, ErrorOraculo>) {
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        fs::write(repo.path().join("requirements.txt"), texto_req).expect("requirements");
        let tarea = tarea_python_de_prueba();
        let podman = PodmanDigest::que_responde(125, "");
        let deps = DependenciasPython {
            podman: &podman,
            ejecutor,
            leer_env: leer,
        };
        let resultado = preparar_python_con(
            repo.path(),
            &tarea,
            home.path(),
            Duration::from_secs(60),
            registros,
            &deps,
        );
        (home, resultado)
    }

    const REQ_CON_INDICE: &str = "--index-url https://reg.example:8443/simple/\nsix==1.16.0\n";

    #[test]
    fn descarga_python_netrc_lleva_el_usuario_env_y_la_clave_del_registro() {
        let ejecutor = EjecutorFalso::que_responde(0, "");
        let registros = [registro_pip(
            "https://reg.example:8443/simple/",
            Some("ROOT_TEST_USR_PIP"),
        )];
        let (_home, resultado) =
            descarga_python_con(REQ_CON_INDICE, &registros, &ejecutor, &entorno_de_prueba);
        resultado.expect("la descarga corre");
        let (ruta, texto, modo) = ejecutor
            .netrc_visto
            .borrow()
            .clone()
            .expect("el contenedor vio el netrc");
        assert_eq!(
            texto,
            format!("machine reg.example login usuario-falso password {CLAVE_FALSA}\n")
        );
        assert_eq!(modo, 0o600, "permisos del netrc");
        for e in ejecutor.ejecuciones.borrow().iter() {
            assert!(
                e.env
                    .contains(&("NETRC".to_string(), "/trabajo/pip.netrc".to_string())),
                "{:?}",
                e.env
            );
        }
        assert!(!ruta.exists(), "el netrc se borra al terminar");
    }

    #[test]
    fn descarga_python_netrc_sin_usuario_env_usa_token() {
        let ejecutor = EjecutorFalso::que_responde(0, "");
        let registros = [registro_pip("https://reg.example:8443/simple/", None)];
        let (_home, resultado) =
            descarga_python_con(REQ_CON_INDICE, &registros, &ejecutor, &entorno_de_prueba);
        resultado.expect("la descarga corre");
        let (_, texto, _) = ejecutor.netrc_visto.borrow().clone().expect("hay netrc");
        assert_eq!(
            texto,
            format!("machine reg.example login __token__ password {CLAVE_FALSA}\n")
        );
    }

    #[test]
    fn descarga_python_la_clave_no_llega_a_argumentos_ni_a_eventos() {
        let ejecutor = EjecutorFalso::que_responde(0, "");
        let registros = [registro_pip(
            "https://reg.example:8443/simple/",
            Some("ROOT_TEST_USR_PIP"),
        )];
        let (home, resultado) =
            descarga_python_con(REQ_CON_INDICE, &registros, &ejecutor, &entorno_de_prueba);
        let (preparacion, registro) = resultado.expect("la descarga corre");
        for e in ejecutor.ejecuciones.borrow().iter() {
            let args = contenedor::argumentos(e, "n").join(" ");
            assert!(!args.contains(CLAVE_FALSA), "en los argumentos: {args}");
            assert!(!format!("{e:?}").contains(CLAVE_FALSA), "en el Debug");
        }
        let registro = registro.expect("hay registro de la descarga");
        registrar(home.path(), &tarea_python_de_prueba(), &registro).expect("registra el Gate");
        let eventos = fs::read_to_string(home.path().join("eventos.jsonl")).expect("eventos");
        assert!(
            !eventos.contains(CLAVE_FALSA),
            "en eventos.jsonl: {eventos}"
        );
        assert!(
            !format!("{preparacion:?}").contains(CLAVE_FALSA),
            "en la preparación"
        );
    }

    #[test]
    fn descarga_python_stderr_con_la_clave_se_redacta_en_el_resultado() {
        // Pip falla y su texto trae la clave: lo que sale de la descarga no la muestra.
        let ejecutor = EjecutorFalso::que_responde(1, &format!("pip: 401 con {CLAVE_FALSA}"));
        let registros = [registro_pip(
            "https://reg.example:8443/simple/",
            Some("ROOT_TEST_USR_PIP"),
        )];
        let (_home, resultado) =
            descarga_python_con(REQ_CON_INDICE, &registros, &ejecutor, &entorno_de_prueba);
        let texto = match &resultado {
            Ok((preparacion, _)) => format!("{preparacion:?}"),
            Err(error) => error.to_string(),
        };
        assert!(!texto.contains(CLAVE_FALSA), "la clave no sale: {texto}");
    }

    #[test]
    fn descarga_python_netrc_solo_llega_a_los_contenedores_de_pip() {
        // Con una URL directa, un curl corre además de pip: ese contenedor no recibe el netrc.
        let ejecutor = EjecutorFalso::que_responde(0, "");
        let registros = [registro_pip(
            "https://reg.example:8443/simple/",
            Some("ROOT_TEST_USR_PIP"),
        )];
        let texto = format!("{REQ_CON_INDICE}https://files.example/x-1.0-py3-none-any.whl\n");
        let (_home, resultado) =
            descarga_python_con(&texto, &registros, &ejecutor, &entorno_de_prueba);
        resultado.expect("la descarga corre");
        let ejecuciones = ejecutor.ejecuciones.borrow();
        let con_netrc = |e: &Ejecucion| e.env.iter().any(|(n, _)| n == "NETRC");
        assert!(
            ejecuciones.iter().any(|e| e.cmd.contains("curl")),
            "hay un curl"
        );
        assert!(
            ejecuciones
                .iter()
                .any(|e| e.cmd.contains("pip download") && con_netrc(e))
        );
        for e in ejecuciones.iter().filter(|e| e.cmd.contains("curl")) {
            assert!(!con_netrc(e), "el curl no recibe el netrc: {}", e.cmd);
        }
    }

    #[test]
    fn descarga_python_registro_de_otro_host_no_recibe_el_netrc() {
        let ejecutor = EjecutorFalso::que_responde(0, "");
        let registros = [registro_pip(
            "https://otro.example/simple/",
            Some("ROOT_TEST_USR_PIP"),
        )];
        let (_home, resultado) =
            descarga_python_con(REQ_CON_INDICE, &registros, &ejecutor, &entorno_de_prueba);
        resultado.expect("la descarga corre");
        assert!(ejecutor.netrc_visto.borrow().is_none(), "no hay netrc");
        for e in ejecutor.ejecuciones.borrow().iter() {
            assert!(!e.env.iter().any(|(n, _)| n == "NETRC"), "{:?}", e.env);
        }
    }

    #[test]
    fn descarga_python_host_con_otro_puerto_en_el_requirements_no_recibe_el_netrc() {
        let ejecutor = EjecutorFalso::que_responde(0, "");
        let registros = [registro_pip(
            "https://reg.example:8443/simple/",
            Some("ROOT_TEST_USR_PIP"),
        )];
        let texto = "--index-url https://reg.example:8443/simple/\n\
                     --extra-index-url https://reg.example:9443/simple/\nsix==1.16.0\n";
        let (_home, resultado) =
            descarga_python_con(texto, &registros, &ejecutor, &entorno_de_prueba);
        resultado.expect("la descarga corre");
        assert!(
            ejecutor.netrc_visto.borrow().is_none(),
            "netrc no distingue puertos"
        );
    }

    #[test]
    fn descarga_python_valor_de_clave_con_salto_de_linea_no_arma_netrc() {
        let ejecutor = EjecutorFalso::que_responde(0, "");
        let registros = [registro_pip("https://reg.example:8443/simple/", None)];
        let malicioso = |nombre: &str| {
            (nombre == "ROOT_TEST_CLAVE_PIP")
                .then(|| "x\nmachine evil.example login a password b".to_string())
        };
        let (_home, resultado) =
            descarga_python_con(REQ_CON_INDICE, &registros, &ejecutor, &malicioso);
        resultado.expect("la descarga corre");
        assert!(
            ejecutor.netrc_visto.borrow().is_none(),
            "un valor inválido no entra"
        );
    }

    // --- Desempate 11: la imagen tiene que estar local antes de calcular la clave ---

    /// Corre `preparar_python_con` de un `requirements.txt` de un paquete, con `podman` y `ejecutor` dados.
    fn preparar_con_podman(
        podman: &dyn Podman,
        ejecutor: &EjecutorFalso,
    ) -> Result<Paso, ErrorOraculo> {
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        fs::write(repo.path().join("requirements.txt"), "six==1.16.0\n").expect("escribe");
        let tarea = tarea_python_de_prueba();
        let deps = DependenciasPython {
            podman,
            ejecutor,
            leer_env: &|_: &str| None,
        };
        preparar_python_con(
            repo.path(),
            &tarea,
            home.path(),
            Duration::from_secs(60),
            &[],
            &deps,
        )
    }

    #[test]
    fn descarga_python_imagen_no_local_es_infra_y_no_corre_contenedores() {
        let podman = PodmanDigest::con_imagen(1, 0, "sha256:aaaa\n");
        let ejecutor = EjecutorFalso::que_responde(0, "");
        match preparar_con_podman(&podman, &ejecutor) {
            Err(ErrorOraculo::Infra(mensaje)) => {
                assert!(mensaje.contains("python:3.8"), "{mensaje}");
            }
            otro => panic!("debía dar Infra, dio {:?}", otro.map(|_| ())),
        }
        let llamadas = podman.llamadas.borrow();
        assert_eq!(llamadas.len(), 1, "solo `image exists`: {llamadas:?}");
        assert_eq!(llamadas[0], vec!["image", "exists", "python:3.8"]);
        assert!(
            ejecutor.ejecuciones.borrow().is_empty(),
            "ningún contenedor"
        );
    }

    #[test]
    fn descarga_python_podman_que_no_arranca_es_infra() {
        let ejecutor = EjecutorFalso::que_responde(0, "");
        match preparar_con_podman(&PodmanRoto, &ejecutor) {
            Err(ErrorOraculo::Infra(mensaje)) => {
                assert!(mensaje.contains("python:3.8"), "{mensaje}");
            }
            otro => panic!("debía dar Infra, dio {:?}", otro.map(|_| ())),
        }
        assert!(ejecutor.ejecuciones.borrow().is_empty());
    }

    // --- Desempates 12 y 13 en el Gate: el aviso del netrc va en `extra.avisos` ---

    #[test]
    fn descarga_python_aviso_del_netrc_llega_al_evento_gate() {
        let ejecutor = EjecutorFalso::que_responde(0, "");
        let registros = [registro_pip("https://reg.example:8443/simple/", None)];
        let con_espacio = |nombre: &str| {
            (nombre == "ROOT_TEST_CLAVE_PIP").then(|| "clave con espacio".to_string())
        };
        let (home, resultado) =
            descarga_python_con(REQ_CON_INDICE, &registros, &ejecutor, &con_espacio);
        let (_, registro) = resultado.expect("la descarga corre");
        let registro = registro.expect("hay registro de la descarga");
        registrar(home.path(), &tarea_python_de_prueba(), &registro).expect("registra el Gate");
        let eventos = fs::read_to_string(home.path().join("eventos.jsonl")).expect("eventos");
        let evento: Value = eventos
            .lines()
            .map(|l| serde_json::from_str::<Value>(l).expect("línea JSON"))
            .find(|v| v["tipo"] == "gate" && v["fase"] == "descarga")
            .expect("evento Gate de la descarga");
        assert!(evento["extra"]["avisos"].is_array(), "{evento}");
        let avisos = evento["extra"]["avisos"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        assert!(
            avisos.iter().any(|a| a
                .as_str()
                .is_some_and(|t| t.contains("priv") && t.contains("ROOT_TEST_CLAVE_PIP"))),
            "{avisos:?}"
        );
        assert!(
            !eventos.contains("clave con espacio"),
            "el valor no sale: {eventos}"
        );
    }

    #[test]
    fn descarga_python_sin_avisos_no_escribe_la_clave_avisos_en_el_gate() {
        let home = tempfile::tempdir().expect("home");
        registrar(
            home.path(),
            &tarea_python_de_prueba(),
            &Registro::sin_paquetes("python"),
        )
        .expect("registra el Gate");
        let eventos = fs::read_to_string(home.path().join("eventos.jsonl")).expect("eventos");
        let evento: Value = eventos
            .lines()
            .map(|l| serde_json::from_str::<Value>(l).expect("línea JSON"))
            .find(|v| v["tipo"] == "gate" && v["fase"] == "descarga")
            .expect("evento Gate de la descarga");
        assert!(evento["extra"].get("avisos").is_none(), "{evento}");
    }

    // --- H5: dependencias de runtime de sdists y clones git ---

    /// Respuesta de un contenedor falso: código y error.
    fn respuesta(codigo: i32, stderr: &str) -> Resultado {
        Resultado {
            codigo,
            stdout: String::new(),
            stderr: stderr.into(),
            segundos: 0.0,
        }
    }

    /// Ejecutor que simula red, disco y el script del contenedor. `simular` responde cada ejecución y escribe
    /// en `cache`, la caché parcial. Guarda cada ejecución.
    struct EjecutorSimulado<'f> {
        simular: &'f dyn Fn(&Ejecucion, &Path) -> Resultado,
        ejecuciones: RefCell<Vec<Ejecucion>>,
    }

    impl EjecutorDeDescarga for EjecutorSimulado<'_> {
        fn ejecutar(&self, ejecucion: &Ejecucion) -> Result<Resultado, ErrorOraculo> {
            self.ejecuciones.borrow_mut().push(ejecucion.clone());
            let cache = ejecucion
                .cache
                .as_deref()
                .expect("la descarga monta la caché");
            Ok((self.simular)(ejecucion, cache))
        }
    }

    /// Corre la descarga de Python de `texto_req` contra `ejecutor`. Devuelve el `home`, para leer lo que deja.
    fn descarga_simulada(
        texto_req: &str,
        ejecutor: &dyn EjecutorDeDescarga,
    ) -> (tempfile::TempDir, Result<Paso, ErrorOraculo>) {
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        fs::write(repo.path().join("requirements.txt"), texto_req).expect("requirements");
        let tarea = tarea_python_de_prueba();
        let podman = PodmanDigest::que_responde(125, "");
        let deps = DependenciasPython {
            podman: &podman,
            ejecutor,
            leer_env: &|_: &str| None,
        };
        let resultado = preparar_python_con(
            repo.path(),
            &tarea,
            home.path(),
            Duration::from_secs(60),
            &[],
            &deps,
        );
        (home, resultado)
    }

    /// Bytes del sdist falso `foo-1.0.tar.gz`. El script simulado no los lee: solo cuenta su sha256.
    const SDIST_FALSO: &[u8] = b"sdist falso";

    /// Mundo con un solo sdist, `foo==1.0`. PyPI lo ofrece, pip no lo baja como wheel sin él, y el script
    /// del contenedor deja `dependencias` en `dependencias-sdist.json`. Guarda en `indice` el build de pip.
    fn mundo_de_sdist<'a>(
        dependencias: &'a str,
        indice: &'a RefCell<Option<String>>,
    ) -> impl Fn(&Ejecucion, &Path) -> Resultado + 'a {
        mundo_de_sdist_con(dependencias, "[]", indice)
    }

    /// Como `mundo_de_sdist`, con la lista de `viejas`: el JSON de sdists con metadatos anteriores a 2.1.
    fn mundo_de_sdist_con<'a>(
        dependencias: &'a str,
        viejas: &'a str,
        indice: &'a RefCell<Option<String>>,
    ) -> impl Fn(&Ejecucion, &Path) -> Resultado + 'a {
        let json_pypi = format!(
            r#"{{"urls": [{{"packagetype": "sdist", "url": "https://files.pythonhosted.org/packages/foo-1.0.tar.gz", "filename": "foo-1.0.tar.gz", "digests": {{"sha256": "{}"}}}}]}}"#,
            sha256_hex(SDIST_FALSO)
        );
        move |ejecucion: &Ejecucion, cache: &Path| {
            let cmd = ejecucion.cmd.as_str();
            if cmd.contains("-o /cache/pypi-meta.json") {
                fs::write(cache.join(ARCHIVO_JSON), &json_pypi).expect("json de PyPI");
            } else if cmd.contains("-o /cache/foo-1.0.tar.gz") {
                fs::write(cache.join("foo-1.0.tar.gz"), SDIST_FALSO).expect("sdist");
            } else if cmd.starts_with("python3 -I -c") {
                fs::write(
                    cache.join(ARCHIVO_REQUISITOS_SDIST),
                    r#"{"foo-1.0.tar.gz": ["hatchling"]}"#,
                )
                .expect("requisitos");
                fs::write(cache.join(ARCHIVO_DEPENDENCIAS_SDIST), dependencias)
                    .expect("dependencias");
                fs::write(cache.join(ARCHIVO_METADATA_VIEJA_SDIST), viejas)
                    .expect("metadata vieja");
            } else if cmd.contains("pip download") && cmd.contains(ARCHIVO_INDICE) {
                // Sin el sdist en la caché, pip no halla `foo==1.0` como wheel.
                if !cache.join("foo-1.0.tar.gz").is_file() {
                    return respuesta(1, "ERROR: No matching distribution found for foo==1.0");
                }
            } else if cmd.contains("pip download") && cmd.contains(ARCHIVO_BUILD) {
                *indice.borrow_mut() = fs::read_to_string(cache.join(ARCHIVO_BUILD)).ok();
            }
            respuesta(0, "")
        }
    }

    /// Mundo con un clon. `git clone` deja `pyproject` en `src/<dir>` y su commit. Guarda en `indice` el build.
    fn mundo_de_clon<'a>(
        dir: &'a str,
        pyproject: &'a str,
        indice: &'a RefCell<Option<String>>,
    ) -> impl Fn(&Ejecucion, &Path) -> Resultado + 'a {
        mundo_de_clon_con(dir, Some(pyproject), indice)
    }

    /// Como `mundo_de_clon`. Con `pyproject` en `None`, el clon trae solo un `setup.py` legado.
    fn mundo_de_clon_con<'a>(
        dir: &'a str,
        pyproject: Option<&'a str>,
        indice: &'a RefCell<Option<String>>,
    ) -> impl Fn(&Ejecucion, &Path) -> Resultado + 'a {
        move |ejecucion: &Ejecucion, cache: &Path| {
            let cmd = ejecucion.cmd.as_str();
            if cmd.contains(" clone ") {
                let src = cache.join("src");
                fs::create_dir_all(src.join(dir)).expect("clon");
                match pyproject {
                    Some(texto) => {
                        fs::write(src.join(dir).join(ARCHIVO_PYPROJECT), texto).expect("pyproject")
                    }
                    None => fs::write(
                        src.join(dir).join("setup.py"),
                        "from setuptools import setup\nsetup()\n",
                    )
                    .expect("setup.py"),
                }
                fs::write(
                    src.join(format!("{dir}.commit")),
                    "c8e394065cd541a16c040515dc0afb85cf22a7c3\n",
                )
                .expect("commit");
            } else if cmd.contains("pip download") && cmd.contains(ARCHIVO_BUILD) {
                *indice.borrow_mut() = fs::read_to_string(cache.join(ARCHIVO_BUILD)).ok();
            }
            respuesta(0, "")
        }
    }

    #[test]
    fn descarga_sdist_dependencia_de_runtime_llega_al_indice_de_wheels() {
        let indice = RefCell::new(None);
        let mundo = mundo_de_sdist(r#"{"foo-1.0.tar.gz": ["requests>=2"]}"#, &indice);
        let ejecutor = EjecutorSimulado {
            simular: &mundo,
            ejecuciones: RefCell::new(Vec::new()),
        };
        let (_home, resultado) = descarga_simulada("foo==1.0\n", &ejecutor);
        let (preparacion, _) = resultado.expect("la descarga corre");
        assert!(
            matches!(preparacion, Preparacion::Lista { .. }),
            "{preparacion:?}"
        );
        let build = indice
            .borrow()
            .clone()
            .expect("pip bajó los requisitos de build");
        assert!(build.lines().any(|l| l == "requests>=2"), "{build}");
    }

    #[test]
    fn descarga_sdist_sin_dependencias_de_runtime_no_agrega_ni_corta() {
        let indice = RefCell::new(None);
        let mundo = mundo_de_sdist(r#"{"foo-1.0.tar.gz": []}"#, &indice);
        let ejecutor = EjecutorSimulado {
            simular: &mundo,
            ejecuciones: RefCell::new(Vec::new()),
        };
        let (_home, resultado) = descarga_simulada("foo==1.0\n", &ejecutor);
        let (preparacion, _) = resultado.expect("la descarga corre");
        assert!(
            matches!(preparacion, Preparacion::Lista { .. }),
            "{preparacion:?}"
        );
        let build = indice.borrow().clone().expect("hay requisitos de build");
        assert_eq!(build.trim(), "hatchling");
    }

    #[test]
    fn descarga_sdist_con_dependencias_dinamicas_da_corte_sin_veredicto() {
        let indice = RefCell::new(None);
        let mundo = mundo_de_sdist(r#"{"foo-1.0.tar.gz": null}"#, &indice);
        let ejecutor = EjecutorSimulado {
            simular: &mundo,
            ejecuciones: RefCell::new(Vec::new()),
        };
        let (_home, resultado) = descarga_simulada("foo==1.0\n", &ejecutor);
        let (preparacion, registro) = resultado.expect("la descarga corre");
        let Preparacion::SinVeredicto(hallazgos) = &preparacion else {
            panic!("debía dar corte sin veredicto: {preparacion:?}");
        };
        let [hallazgo] = hallazgos.as_slice() else {
            panic!("un solo hallazgo: {hallazgos:?}");
        };
        assert_eq!(hallazgo.severidad, 2);
        assert_eq!(
            hallazgo.detalle,
            "dependencias de runtime no declaradas en forma estática: foo-1.0.tar.gz"
        );
        let registro = registro.expect("hay registro de la descarga");
        assert_eq!(registro.motivo, Some("dependencias-dinamicas"));
        assert!(indice.borrow().is_none(), "el corte va antes de pip");
    }

    #[test]
    fn descarga_clon_dependencias_de_runtime_llegan_al_indice_de_wheels() {
        let raw = "git+https://github.com/x/y@c8e394065cd541a16c040515dc0afb85cf22a7c3#egg=y";
        let dir = clave_clon(&partes_git(raw).expect("parsea"));
        let indice = RefCell::new(None);
        let pyproject =
            "[project]\nname = \"y\"\nversion = \"1.0\"\ndependencies = [\"requests>=2\"]\n";
        let mundo = mundo_de_clon(&dir, pyproject, &indice);
        let ejecutor = EjecutorSimulado {
            simular: &mundo,
            ejecuciones: RefCell::new(Vec::new()),
        };
        let (_home, resultado) = descarga_simulada(&format!("{raw}\n"), &ejecutor);
        let (preparacion, _) = resultado.expect("la descarga corre");
        assert!(
            matches!(preparacion, Preparacion::Lista { .. }),
            "{preparacion:?}"
        );
        let build = indice.borrow().clone().expect("hay requisitos de build");
        assert!(build.lines().any(|l| l == "requests>=2"), "{build}");
    }

    #[test]
    fn descarga_clon_dependencias_dinamicas_da_corte_sin_veredicto() {
        let raw = "git+https://github.com/x/y@c8e394065cd541a16c040515dc0afb85cf22a7c3#egg=y";
        let dir = clave_clon(&partes_git(raw).expect("parsea"));
        let indice = RefCell::new(None);
        let pyproject = "[project]\nname = \"y\"\ndynamic = [\"dependencies\"]\n";
        let mundo = mundo_de_clon(&dir, pyproject, &indice);
        let ejecutor = EjecutorSimulado {
            simular: &mundo,
            ejecuciones: RefCell::new(Vec::new()),
        };
        let (_home, resultado) = descarga_simulada(&format!("{raw}\n"), &ejecutor);
        let (preparacion, registro) = resultado.expect("la descarga corre");
        let Preparacion::SinVeredicto(hallazgos) = &preparacion else {
            panic!("debía dar corte sin veredicto: {preparacion:?}");
        };
        let [hallazgo] = hallazgos.as_slice() else {
            panic!("un solo hallazgo: {hallazgos:?}");
        };
        assert_eq!(hallazgo.severidad, 2);
        assert!(
            hallazgo
                .detalle
                .starts_with("dependencias de runtime no declaradas en forma estática: "),
            "{}",
            hallazgo.detalle
        );
        assert!(
            hallazgo.detalle.contains("github.com/x/y"),
            "{}",
            hallazgo.detalle
        );
        let registro = registro.expect("hay registro de la descarga");
        assert_eq!(registro.motivo, Some("dependencias-dinamicas"));
    }

    /// Responde `stderr` con código 1 al `pip download` de los requisitos de build. Lo demás lo responde `mundo`.
    fn build_que_falla<'a>(
        mundo: &'a impl Fn(&Ejecucion, &Path) -> Resultado,
        stderr: &'a str,
    ) -> impl Fn(&Ejecucion, &Path) -> Resultado + 'a {
        move |ejecucion: &Ejecucion, cache: &Path| {
            if ejecucion.cmd.contains("pip download") && ejecucion.cmd.contains(ARCHIVO_BUILD) {
                return respuesta(1, stderr);
            }
            mundo(ejecucion, cache)
        }
    }

    #[test]
    fn descarga_python_solo_sdist_de_runtime_en_el_build_da_corte_sin_veredicto() {
        // Una dependencia de runtime de un sdist sin wheel no es `no-instala`: es `solo-sdist`.
        let indice = RefCell::new(None);
        let mundo = mundo_de_sdist(r#"{"foo-1.0.tar.gz": ["dep-solo-sdist==1.0"]}"#, &indice);
        let falla = build_que_falla(
            &mundo,
            "ERROR: No matching distribution found for dep-solo-sdist==1.0\n",
        );
        let ejecutor = EjecutorSimulado {
            simular: &falla,
            ejecuciones: RefCell::new(Vec::new()),
        };
        let (_home, resultado) = descarga_simulada("foo==1.0\n", &ejecutor);
        let (preparacion, registro) = resultado.expect("la descarga corre");
        let Preparacion::SinVeredicto(hallazgos) = &preparacion else {
            panic!("debía dar corte sin veredicto: {preparacion:?}");
        };
        let [hallazgo] = hallazgos.as_slice() else {
            panic!("un solo hallazgo: {hallazgos:?}");
        };
        assert_eq!(hallazgo.severidad, 2);
        assert_eq!(
            hallazgo.detalle,
            "dep-solo-sdist solo tiene sdist y la descarga no construye sdists"
        );
        let registro = registro.expect("hay registro de la descarga");
        assert_eq!(registro.motivo, Some("solo-sdist"));
    }

    #[test]
    fn descarga_python_solo_sdist_de_runtime_de_clon_da_corte_sin_veredicto() {
        let raw = "git+https://github.com/x/y@c8e394065cd541a16c040515dc0afb85cf22a7c3#egg=y";
        let dir = clave_clon(&partes_git(raw).expect("parsea"));
        let indice = RefCell::new(None);
        let pyproject = "[project]\nname = \"y\"\nversion = \"1.0\"\ndependencies = [\"dep-solo-sdist==1.0\"]\n";
        let mundo = mundo_de_clon(&dir, pyproject, &indice);
        let falla = build_que_falla(
            &mundo,
            "ERROR: No matching distribution found for dep-solo-sdist==1.0\n",
        );
        let ejecutor = EjecutorSimulado {
            simular: &falla,
            ejecuciones: RefCell::new(Vec::new()),
        };
        let (_home, resultado) = descarga_simulada(&format!("{raw}\n"), &ejecutor);
        let (preparacion, registro) = resultado.expect("la descarga corre");
        let Preparacion::SinVeredicto(hallazgos) = &preparacion else {
            panic!("debía dar corte sin veredicto: {preparacion:?}");
        };
        let [hallazgo] = hallazgos.as_slice() else {
            panic!("un solo hallazgo: {hallazgos:?}");
        };
        assert_eq!(hallazgo.severidad, 2);
        assert_eq!(
            hallazgo.detalle,
            "dep-solo-sdist solo tiene sdist y la descarga no construye sdists"
        );
        let registro = registro.expect("hay registro de la descarga");
        assert_eq!(registro.motivo, Some("solo-sdist"));
    }

    #[test]
    fn descarga_python_requisito_de_build_sin_wheel_sigue_en_no_instala() {
        // Guarda: `hatchling` es requisito de build, no de runtime. Su falta sigue siendo `no-instala`.
        let indice = RefCell::new(None);
        let mundo = mundo_de_sdist(r#"{"foo-1.0.tar.gz": []}"#, &indice);
        let falla = build_que_falla(
            &mundo,
            "ERROR: No matching distribution found for hatchling==9.9\n",
        );
        let ejecutor = EjecutorSimulado {
            simular: &falla,
            ejecuciones: RefCell::new(Vec::new()),
        };
        let (_home, resultado) = descarga_simulada("foo==1.0\n", &ejecutor);
        let (_preparacion, registro) = resultado.expect("la descarga corre");
        let registro = registro.expect("hay registro de la descarga");
        assert_eq!(registro.motivo, Some("no-instala"));
    }

    #[test]
    fn descarga_python_error_de_build_sin_patron_no_es_solo_sdist() {
        // Guarda: un error de compilación del producto no cae en `solo-sdist`. Sin patrón conocido, es infra.
        let indice = RefCell::new(None);
        let mundo = mundo_de_sdist(r#"{"foo-1.0.tar.gz": ["dep-solo-sdist==1.0"]}"#, &indice);
        let falla = build_que_falla(&mundo, "error: command 'gcc' failed with exit status 1\n");
        let ejecutor = EjecutorSimulado {
            simular: &falla,
            ejecuciones: RefCell::new(Vec::new()),
        };
        let (_home, resultado) = descarga_simulada("foo==1.0\n", &ejecutor);
        assert!(
            matches!(resultado, Err(ErrorOraculo::Infra(_))),
            "un error sin patrón conocido debía ser infra"
        );
    }

    #[test]
    fn descarga_dependencias_de_clon_estaticas_se_leen() {
        assert_eq!(
            dependencias_de_clon(Some(
                "[project]\ndependencies = [\"requests>=2\", \" six \", \"  \"]\n"
            )),
            Some(vec!["requests>=2".to_string(), "six".to_string()])
        );
        assert_eq!(
            dependencias_de_clon(Some("[project]\nname = \"x\"\n")),
            Some(Vec::new()),
            "sin `dependencies` y sin `dynamic`: no declara ninguna"
        );
    }

    #[test]
    fn descarga_dependencias_de_clon_sin_pyproject_o_sin_project_son_vacias() {
        assert_eq!(dependencias_de_clon(None), Some(Vec::new()));
        assert_eq!(
            dependencias_de_clon(Some("[build-system]\nrequires = [\"setuptools\"]\n")),
            Some(Vec::new()),
            "setup.py legado: no declara ninguna"
        );
    }

    #[test]
    fn descarga_dependencias_de_clon_dinamicas_o_ilegibles_son_none() {
        for texto in [
            Some("[project]\nname = \"x\"\ndynamic = [\"dependencies\"]\n"),
            Some("[project]\ndependencies = \"requests\"\n"),
            Some("[project]\ndependencies = [1]\n"),
            Some("no es toml [[["),
        ] {
            assert_eq!(dependencias_de_clon(texto), None, "{texto:?}");
        }
    }

    /// Fuentes dinámicas de un clon sin o con ese `pyproject.toml` en `src/<clave>`.
    fn fuentes_de_clon(pyproject: Option<&str>) -> Vec<String> {
        let raw = "git+https://github.com/x/y@c8e394065cd541a16c040515dc0afb85cf22a7c3#egg=y";
        let dir = clave_clon(&partes_git(raw).expect("parsea"));
        let carpeta = tempfile::tempdir().expect("carpeta");
        let clon = carpeta.path().join("src").join(&dir);
        fs::create_dir_all(&clon).expect("clon");
        if let Some(texto) = pyproject {
            fs::write(clon.join(ARCHIVO_PYPROJECT), texto).expect("pyproject");
        }
        let req = RequisitosPip {
            directas: vec![(1, raw.to_string())],
            ..Default::default()
        };
        fuentes_dinamicas(carpeta.path(), &req).expect("fuentes")
    }

    #[test]
    fn descarga_clon_sin_pyproject_no_corta() {
        assert!(fuentes_de_clon(None).is_empty());
    }

    #[test]
    fn descarga_clon_pyproject_sin_project_no_corta() {
        assert!(fuentes_de_clon(Some("[build-system]\nrequires = [\"setuptools\"]\n")).is_empty());
    }

    #[test]
    fn descarga_indice_de_build_de_sdist_incluye_sus_dependencias_de_runtime() {
        // Camino de acierto de caché: el índice sale de los JSON guardados, sin correr el contenedor.
        let dir = tempfile::tempdir().expect("temp dir");
        fs::write(dir.path().join("a-1.0.tar.gz"), "x").expect("sdist");
        fs::write(
            dir.path().join(ARCHIVO_REQUISITOS_SDIST),
            r#"{"a-1.0.tar.gz": ["hatchling"]}"#,
        )
        .expect("json");
        fs::write(
            dir.path().join(ARCHIVO_DEPENDENCIAS_SDIST),
            r#"{"a-1.0.tar.gz": ["requests>=2"]}"#,
        )
        .expect("json");
        let (requisitos, hallazgos) = indice_de_build_de(dir.path(), &[])
            .expect("indice")
            .expect("hay build");
        assert_eq!(requisitos, ["hatchling", "requests>=2"]);
        assert!(hallazgos.is_empty());
    }

    #[test]
    fn descarga_script_de_sdists_escribe_dependencias_de_runtime() {
        // Corre el script real sobre sdists `.tar.gz` armados en temporales. Sin `python3` con `tomllib`, no corre.
        let corre = Command::new("python3")
            .args(["-I", "-c", "import tomllib"])
            .output()
            .is_ok_and(|s| s.status.success());
        if !corre {
            eprintln!("sin python3 con tomllib: el script no se prueba");
            return;
        }
        let cache = tempfile::tempdir().expect("cache");
        let obra = tempfile::tempdir().expect("obra");
        let fuentes: [(&str, Option<&str>); 4] = [
            (
                "pkg-1.0",
                Some(
                    "Metadata-Version: 2.1\nName: pkg\nVersion: 1.0\n\
                     Requires-Dist: requests>=2\n\
                     Requires-Dist: pytest; python_version >= \"3.8\"\n\
                     Requires-Dist: sphinx; extra == \"docs\"\n\
                     Requires-Dist: otro; extra==\"dev\"\n\
                     Provides-Extra: docs\n\n\
                     Requires-Dist: no-cuenta\n",
                ),
            ),
            (
                "dinamico-1.0",
                Some("Metadata-Version: 2.2\nName: dinamico\nDynamic: Requires-Dist\n"),
            ),
            (
                "vacio-1.0",
                Some("Metadata-Version: 2.1\nName: vacio\nVersion: 1.0\n"),
            ),
            ("sin-info-1.0", None),
        ];
        for (dir, info) in fuentes {
            let raiz = obra.path().join(dir);
            fs::create_dir_all(&raiz).expect("carpeta del sdist");
            if let Some(texto) = info {
                fs::write(raiz.join("PKG-INFO"), texto).expect("PKG-INFO");
            }
            fs::write(
                raiz.join(ARCHIVO_PYPROJECT),
                "[build-system]\nrequires = [\"hatchling\"]\n",
            )
            .expect("pyproject");
            let estado = Command::new("tar")
                .arg("-czf")
                .arg(cache.path().join(format!("{dir}.tar.gz")))
                .arg("-C")
                .arg(obra.path())
                .arg(dir)
                .status()
                .expect("tar");
            assert!(estado.success(), "tar de {dir}");
        }
        // El script escribe en `/cache`: la prueba cambia esa ruta por la carpeta temporal.
        let guion = GUION_REQUISITOS_SDIST.replace("/cache", &cache.path().to_string_lossy());
        let salida = Command::new("python3")
            .args(["-I", "-c", &guion])
            .output()
            .expect("corre python3");
        assert!(
            salida.status.success(),
            "el script falló: {}",
            String::from_utf8_lossy(&salida.stderr)
        );
        let texto = fs::read_to_string(cache.path().join(ARCHIVO_DEPENDENCIAS_SDIST))
            .expect("escribe las dependencias");
        let mapa: BTreeMap<String, Option<Vec<String>>> =
            serde_json::from_str(&texto).expect("JSON de dependencias");
        let esperado: BTreeMap<String, Option<Vec<String>>> = [
            (
                "pkg-1.0.tar.gz",
                Some(vec![
                    "requests>=2".to_string(),
                    "pytest; python_version >= \"3.8\"".to_string(),
                ]),
            ),
            ("dinamico-1.0.tar.gz", None),
            ("vacio-1.0.tar.gz", Some(Vec::new())),
            ("sin-info-1.0.tar.gz", None),
        ]
        .into_iter()
        .map(|(nombre, deps)| (nombre.to_string(), deps))
        .collect();
        assert_eq!(mapa, esperado);
    }

    // --- Dependencias de runtime no verificadas: sdists con metadata vieja y clones sin metadatos estáticos ---

    /// Texto fijo del aviso. Va a los hallazgos de la descarga y a los avisos del Gate.
    const AVISO_NO_VERIFICADAS: &str = "dependencias de runtime no verificadas";
    /// Clon de prueba: un repo y un commit fijos. Cada test decide qué archivos trae el clon.
    const RAW_CLON: &str =
        "git+https://github.com/x/y@c8e394065cd541a16c040515dc0afb85cf22a7c3#egg=y";

    /// Aviso esperado de un clon de `RAW_CLON` sin metadatos estáticos.
    fn aviso_de_clon() -> String {
        let base = partes_git(RAW_CLON).expect("parsea").base;
        format!(
            "clon sin metadatos estáticos: {AVISO_NO_VERIFICADAS}: {}",
            redactar(&base)
        )
    }

    /// Hallazgos y avisos del Gate que dicen `AVISO_NO_VERIFICADAS` en un paso de descarga que da Lista.
    fn no_verificadas_en(paso: &Paso) -> (Vec<&Hallazgo>, Vec<String>) {
        let (preparacion, registro) = paso;
        let Preparacion::Lista { hallazgos, .. } = preparacion else {
            panic!("debía dar Lista: {preparacion:?}");
        };
        let en_hallazgos: Vec<&Hallazgo> = hallazgos
            .iter()
            .filter(|h| h.detalle.contains(AVISO_NO_VERIFICADAS))
            .collect();
        let en_gate: Vec<String> = registro
            .as_ref()
            .map(|r| r.avisos.clone())
            .unwrap_or_default()
            .into_iter()
            .filter(|t| t.contains(AVISO_NO_VERIFICADAS))
            .collect();
        (en_hallazgos, en_gate)
    }

    /// Corre la descarga de `RAW_CLON` con `mundo`, que hace de `git clone`. Devuelve el paso.
    fn paso_de_clon_con_mundo(mundo: &dyn Fn(&Ejecucion, &Path) -> Resultado) -> Paso {
        let ejecutor = EjecutorSimulado {
            simular: mundo,
            ejecuciones: RefCell::new(Vec::new()),
        };
        let (_home, resultado) = descarga_simulada(&format!("{RAW_CLON}\n"), &ejecutor);
        resultado.expect("la descarga corre")
    }

    /// Paso de la descarga de `RAW_CLON` con su `pyproject` (o sin él, con un `setup.py` legado).
    fn paso_de_clon(pyproject: Option<&str>) -> Paso {
        let dir = clave_clon(&partes_git(RAW_CLON).expect("parsea"));
        let indice = RefCell::new(None);
        let mundo = mundo_de_clon_con(&dir, pyproject, &indice);
        paso_de_clon_con_mundo(&mundo)
    }

    #[test]
    fn descarga_python_clon_con_setup_py_sin_pyproject_avisa_y_lista() {
        let paso = paso_de_clon(None);
        assert!(matches!(paso.0, Preparacion::Lista { .. }), "{:?}", paso.0);
        let (en_hallazgos, en_gate) = no_verificadas_en(&paso);
        let esperado = aviso_de_clon();
        assert_eq!(en_hallazgos.len(), 1, "{en_hallazgos:?}");
        assert_eq!(en_hallazgos[0].severidad, 1);
        assert_eq!(en_hallazgos[0].detalle, esperado);
        assert_eq!(en_gate, [esperado]);
    }

    #[test]
    fn descarga_python_clon_con_pyproject_sin_project_avisa() {
        let paso = paso_de_clon(Some("[build-system]\nrequires = [\"setuptools\"]\n"));
        let (en_hallazgos, en_gate) = no_verificadas_en(&paso);
        assert_eq!(en_hallazgos.len(), 1, "{en_hallazgos:?}");
        assert_eq!(en_hallazgos[0].severidad, 1);
        assert_eq!(en_hallazgos[0].detalle, aviso_de_clon());
        assert_eq!(en_gate, [aviso_de_clon()]);
    }

    #[test]
    fn descarga_python_clon_con_pyproject_symlink_avisa() {
        // Un `pyproject` que es enlace no cuenta como archivo: el clon es legado.
        let afuera = tempfile::tempdir().expect("afuera");
        let real = afuera.path().join("pyproject.toml");
        fs::write(
            &real,
            "[project]\nname = \"y\"\ndependencies = [\"requests\"]\n",
        )
        .expect("afuera");
        let dir = clave_clon(&partes_git(RAW_CLON).expect("parsea"));
        let mundo = |ejecucion: &Ejecucion, cache: &Path| {
            if ejecucion.cmd.contains(" clone ") {
                let clon = cache.join("src").join(&dir);
                fs::create_dir_all(&clon).expect("clon");
                std::os::unix::fs::symlink(&real, clon.join(ARCHIVO_PYPROJECT)).expect("enlace");
                fs::write(
                    cache.join("src").join(format!("{dir}.commit")),
                    "c8e394065cd541a16c040515dc0afb85cf22a7c3\n",
                )
                .expect("commit");
            }
            respuesta(0, "")
        };
        let paso = paso_de_clon_con_mundo(&mundo);
        let (en_hallazgos, en_gate) = no_verificadas_en(&paso);
        assert_eq!(en_hallazgos.len(), 1, "{en_hallazgos:?}");
        assert_eq!(en_gate, [aviso_de_clon()]);
    }

    #[test]
    fn descarga_python_clon_con_pyproject_ilegible_avisa() {
        // Bytes que no son UTF-8: el `pyproject` no se lee, y el clon es legado.
        let dir = clave_clon(&partes_git(RAW_CLON).expect("parsea"));
        let mundo = |ejecucion: &Ejecucion, cache: &Path| {
            if ejecucion.cmd.contains(" clone ") {
                let clon = cache.join("src").join(&dir);
                fs::create_dir_all(&clon).expect("clon");
                fs::write(clon.join(ARCHIVO_PYPROJECT), [0xff_u8, 0xfe, 0x00]).expect("bytes");
                fs::write(
                    cache.join("src").join(format!("{dir}.commit")),
                    "c8e394065cd541a16c040515dc0afb85cf22a7c3\n",
                )
                .expect("commit");
            }
            respuesta(0, "")
        };
        let paso = paso_de_clon_con_mundo(&mundo);
        let (en_hallazgos, en_gate) = no_verificadas_en(&paso);
        assert_eq!(en_hallazgos.len(), 1, "{en_hallazgos:?}");
        assert_eq!(en_gate, [aviso_de_clon()]);
    }

    #[test]
    fn descarga_python_clon_con_project_sin_dependencies_no_avisa() {
        // `[project]` sin `dependencies` declara cero dependencias: no es legado, no hay aviso.
        let paso = paso_de_clon(Some("[project]\nname = \"y\"\nversion = \"1.0\"\n"));
        let (en_hallazgos, en_gate) = no_verificadas_en(&paso);
        assert!(en_hallazgos.is_empty(), "{en_hallazgos:?}");
        assert!(en_gate.is_empty(), "{en_gate:?}");
    }

    /// Dos corridas de la descarga de `texto_req` en el mismo `home`: la segunda es acierto de caché. Da cada
    /// paso y cuántos contenedores corrieron hasta él.
    fn dos_corridas(
        texto_req: &str,
        simular: &dyn Fn(&Ejecucion, &Path) -> Resultado,
    ) -> (Paso, usize, Paso, usize) {
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        fs::write(repo.path().join("requirements.txt"), texto_req).expect("requirements");
        let tarea = tarea_python_de_prueba();
        let podman = PodmanDigest::que_responde(125, "");
        let ejecutor = EjecutorSimulado {
            simular,
            ejecuciones: RefCell::new(Vec::new()),
        };
        let deps = DependenciasPython {
            podman: &podman,
            ejecutor: &ejecutor,
            leer_env: &|_: &str| None,
        };
        let correr = || {
            preparar_python_con(
                repo.path(),
                &tarea,
                home.path(),
                Duration::from_secs(60),
                &[],
                &deps,
            )
            .expect("prepara")
        };
        let primera = correr();
        let n_primera = ejecutor.ejecuciones.borrow().len();
        let segunda = correr();
        let n_segunda = ejecutor.ejecuciones.borrow().len();
        (primera, n_primera, segunda, n_segunda)
    }

    #[test]
    fn descarga_python_acierto_de_cache_repite_el_aviso_del_clon_legado() {
        let dir = clave_clon(&partes_git(RAW_CLON).expect("parsea"));
        let indice = RefCell::new(None);
        let mundo = mundo_de_clon_con(&dir, None, &indice);
        let (primera, n_primera, segunda, n_segunda) =
            dos_corridas(&format!("{RAW_CLON}\n"), &mundo);
        assert!(n_primera > 0, "la primera corrida baja el clon");
        assert_eq!(
            n_primera, n_segunda,
            "la segunda corrida es acierto: no corre nada"
        );
        let (_, gate_primera) = no_verificadas_en(&primera);
        assert_eq!(gate_primera, [aviso_de_clon()]);
        let (en_hallazgos, gate_segunda) = no_verificadas_en(&segunda);
        assert_eq!(en_hallazgos.len(), 1, "{en_hallazgos:?}");
        assert_eq!(en_hallazgos[0].detalle, aviso_de_clon());
        assert_eq!(gate_segunda, [aviso_de_clon()]);
    }

    #[test]
    fn descarga_python_sdist_de_metadata_vieja_avisa_con_su_nombre() {
        let indice = RefCell::new(None);
        let mundo = mundo_de_sdist_con(
            r#"{"foo-1.0.tar.gz": []}"#,
            r#"["foo-1.0.tar.gz"]"#,
            &indice,
        );
        let ejecutor = EjecutorSimulado {
            simular: &mundo,
            ejecuciones: RefCell::new(Vec::new()),
        };
        let (_home, resultado) = descarga_simulada("foo==1.0\n", &ejecutor);
        let paso = resultado.expect("la descarga corre");
        let esperado = "sdist con metadatos anteriores a 2.1: dependencias de runtime no verificadas: foo-1.0.tar.gz";
        let (en_hallazgos, en_gate) = no_verificadas_en(&paso);
        assert_eq!(en_hallazgos.len(), 1, "{en_hallazgos:?}");
        assert_eq!(en_hallazgos[0].severidad, 1);
        assert_eq!(en_hallazgos[0].detalle, esperado);
        assert_eq!(en_gate, [esperado]);
    }

    #[test]
    fn descarga_python_sdist_2_1_sin_requires_dist_no_avisa() {
        // Metadata 2.1 o superior sin `Requires-Dist` es cero dependencias, sin aviso.
        let indice = RefCell::new(None);
        let mundo = mundo_de_sdist(r#"{"foo-1.0.tar.gz": []}"#, &indice);
        let ejecutor = EjecutorSimulado {
            simular: &mundo,
            ejecuciones: RefCell::new(Vec::new()),
        };
        let (_home, resultado) = descarga_simulada("foo==1.0\n", &ejecutor);
        let paso = resultado.expect("la descarga corre");
        let (en_hallazgos, en_gate) = no_verificadas_en(&paso);
        assert!(en_hallazgos.is_empty(), "{en_hallazgos:?}");
        assert!(en_gate.is_empty(), "{en_gate:?}");
    }

    #[test]
    fn descarga_python_acierto_de_cache_repite_el_aviso_del_sdist_viejo() {
        let indice = RefCell::new(None);
        let mundo = mundo_de_sdist_con(
            r#"{"foo-1.0.tar.gz": []}"#,
            r#"["foo-1.0.tar.gz"]"#,
            &indice,
        );
        let (primera, n_primera, segunda, n_segunda) = dos_corridas("foo==1.0\n", &mundo);
        assert!(n_primera > 0, "la primera corrida baja el sdist");
        assert_eq!(
            n_primera, n_segunda,
            "la segunda corrida es acierto: no corre nada"
        );
        let esperado = "sdist con metadatos anteriores a 2.1: dependencias de runtime no verificadas: foo-1.0.tar.gz";
        let (_, gate_primera) = no_verificadas_en(&primera);
        assert_eq!(gate_primera, [esperado]);
        let (en_hallazgos, gate_segunda) = no_verificadas_en(&segunda);
        assert_eq!(en_hallazgos.len(), 1, "{en_hallazgos:?}");
        assert_eq!(gate_segunda, [esperado]);
    }

    /// `true` si hay `python3` con `tomllib`. Sin él, los tests del script real no corren.
    fn python_con_tomllib() -> bool {
        Command::new("python3")
            .args(["-I", "-c", "import tomllib"])
            .output()
            .is_ok_and(|s| s.status.success())
    }

    /// Arma un sdist `.tar.gz` por cada `(carpeta, PKG-INFO)`, sin `PKG-INFO` si es `None`. Corre el script real
    /// sobre ellos y devuelve la caché donde deja sus JSON.
    fn script_sobre_sdists(fuentes: &[(&str, Option<&str>)]) -> tempfile::TempDir {
        let cache = tempfile::tempdir().expect("cache");
        let obra = tempfile::tempdir().expect("obra");
        for &(dir, info) in fuentes {
            let raiz = obra.path().join(dir);
            fs::create_dir_all(&raiz).expect("carpeta del sdist");
            if let Some(texto) = info {
                fs::write(raiz.join("PKG-INFO"), texto).expect("PKG-INFO");
            }
            fs::write(
                raiz.join(ARCHIVO_PYPROJECT),
                "[build-system]\nrequires = [\"hatchling\"]\n",
            )
            .expect("pyproject");
            let estado = Command::new("tar")
                .arg("-czf")
                .arg(cache.path().join(format!("{dir}.tar.gz")))
                .arg("-C")
                .arg(obra.path())
                .arg(dir)
                .status()
                .expect("tar");
            assert!(estado.success(), "tar de {dir}");
        }
        // El script escribe en `/cache`: la prueba cambia esa ruta por la carpeta temporal.
        let guion = GUION_REQUISITOS_SDIST.replace("/cache", &cache.path().to_string_lossy());
        let salida = Command::new("python3")
            .args(["-I", "-c", &guion])
            .output()
            .expect("corre python3");
        assert!(
            salida.status.success(),
            "el script falló: {}",
            String::from_utf8_lossy(&salida.stderr)
        );
        cache
    }

    #[test]
    fn descarga_script_de_sdists_marca_metadata_anterior_a_2_1() {
        // Solo 1.0, 1.2 y 2.0 son anteriores a 2.1. Un 2.1 sin `Requires-Dist`, o sin cabecera, no cuenta.
        if !python_con_tomllib() {
            eprintln!("sin python3 con tomllib: el script no se prueba");
            return;
        }
        let cache = script_sobre_sdists(&[
            (
                "vieja10-1.0",
                Some("Metadata-Version: 1.0\nName: vieja10\nVersion: 1.0\n"),
            ),
            (
                "vieja12-1.0",
                Some("Metadata-Version: 1.2\nName: vieja12\nVersion: 1.0\n"),
            ),
            (
                "dos0-2.0",
                Some("Metadata-Version: 2.0\nName: dos0\nVersion: 1.0\n"),
            ),
            (
                "nueva-2.1",
                Some("Metadata-Version: 2.1\nName: nueva\nVersion: 1.0\n"),
            ),
            ("sincabecera-1.0", Some("Name: sincabecera\nVersion: 1.0\n")),
        ]);
        let texto = fs::read_to_string(cache.path().join(ARCHIVO_METADATA_VIEJA_SDIST))
            .expect("escribe la lista de metadata vieja");
        let mut viejas: Vec<String> = serde_json::from_str(&texto).expect("JSON de la lista");
        viejas.sort();
        assert_eq!(
            viejas,
            [
                "dos0-2.0.tar.gz",
                "vieja10-1.0.tar.gz",
                "vieja12-1.0.tar.gz"
            ]
        );
        let texto = fs::read_to_string(cache.path().join(ARCHIVO_DEPENDENCIAS_SDIST))
            .expect("escribe las dependencias");
        let mapa: BTreeMap<String, Option<Vec<String>>> =
            serde_json::from_str(&texto).expect("JSON de dependencias");
        assert_eq!(mapa["nueva-2.1.tar.gz"], Some(Vec::new()));
        assert_eq!(mapa["sincabecera-1.0.tar.gz"], Some(Vec::new()));
    }
}
