//! Descarga de Python (Tarea 6c-B, decisión 21). Corre solo `pip download`, `curl` y `git`, con red.
//! Nunca corre `pip install`, `pip wheel` ni `setup.py`. Un sdist se baja y se guarda sin extraerlo.
//! Dentro de cada contenedor, `/cache` es la carpeta `<hash>.parcial` del requirements.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::credenciales::{Credencial, netrc_de_pip, redactar_secretos};
use super::inventario::{
    PartesGit, RequisitosPip, motivo_git, partes_git, redactar, requisitos_pip,
    textos_de_requisitos,
};
use super::{
    ARCHIVO_REQUIREMENTS, CARPETA_CACHE, CARPETA_PYTHON, Diagnostico, PLAZO_CORTO, Paso,
    Preparacion, Registro, borrar_en_home, corte, diagnosticar, entorno_descarga, error_de,
    hallazgo, hash_corto, limpiar_parcial, parcial_de, temporal_de,
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
/// `netrc` de pip dentro de la carpeta de trabajo de la descarga. Lleva las claves de los registros que
/// coinciden con un índice; se borra al terminar la descarga.
const ARCHIVO_NETRC: &str = "pip.netrc";
/// Ruta de `ARCHIVO_NETRC` dentro del contenedor, donde `trabajo` está montada en solo lectura.
const RUTA_NETRC_CONTENEDOR: &str = "/trabajo/pip.netrc";
/// Requisitos de build de cada sdist de la caché: nombre del archivo → `[build-system].requires` o `null`.
/// Lo escribe el contenedor de descarga; el host no abre ningún sdist.
pub const ARCHIVO_REQUISITOS_SDIST: &str = "requisitos-sdist.json";
/// Script del contenedor de descarga: lee `[build-system].requires` de cada sdist de `/cache` con
/// `tarfile` o `zipfile`, sin extraer nada, y deja el JSON en `/cache`. Sin `tomllib` (Python < 3.11) sale
/// con código 3: el host falla cerrado.
const GUION_REQUISITOS_SDIST: &str = r#"import glob, json, os, sys, tarfile, zipfile
try:
    import tomllib
except ImportError:
    sys.exit(3)

def primer_nivel(nombre):
    partes = nombre.split("/")
    return len(partes) == 2 and partes[1] == "pyproject.toml" and partes[0] not in ("", ".", "..") and not partes[0].startswith("-")

def pyproject_de(ruta):
    if ruta.endswith(".zip"):
        with zipfile.ZipFile(ruta) as z:
            nombres = [n for n in z.namelist() if primer_nivel(n)]
            return z.read(nombres[0]) if nombres else None
    with tarfile.open(ruta, "r:*") as t:
        for m in t.getmembers():
            if m.isfile() and primer_nivel(m.name):
                f = t.extractfile(m)
                return f.read() if f else None
    return None

def requires(ruta):
    try:
        texto = pyproject_de(ruta)
        if texto is None:
            return None
        lista = tomllib.loads(texto.decode("utf-8")).get("build-system", {}).get("requires")
        if not isinstance(lista, list) or not all(isinstance(s, str) for s in lista):
            return None
        return lista
    except Exception:
        return None

resultado = {}
for ruta in sorted(glob.glob("/cache/*.tar.gz") + glob.glob("/cache/*.zip")):
    resultado[os.path.basename(ruta)] = requires(ruta)
with open("/cache/requisitos-sdist.json", "w", encoding="utf-8") as f:
    json.dump(resultado, f)
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

/// Versión de un especificador con `==` exacto. `None` si no hay versión fijada.
fn version_fijada(spec: &str) -> Option<String> {
    let (_, resto) = spec.split_once("==")?;
    let version: String = resto
        .trim()
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
            "imagen={imagen}\n{digest}--requisitos--\n{requisitos}--restricciones--\n{restricciones}"
        )
        .as_bytes(),
    )
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
    let (texto, restricciones) = textos_de_requisitos(repo);
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
        };
        // Los hallazgos de build salen de los `pyproject` guardados en la caché: sin ellos, el segundo
        // corrido perdería el aviso del primero.
        let mut hallazgos = Vec::new();
        if let Some((_, de_build)) = indice_de_build_de(&destino, &rutas_clon(&req))? {
            hallazgos.extend(de_build);
        }
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
    let netrc = netrc_de_pip(&indices_de(&req), registros, deps.leer_env);
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
            Ok(corte(aviso, Registro::corte("python", Some(hash), motivo)))
        }
        Fin::Lista {
            paquetes,
            hallazgos: de_build,
        } => {
            let hallazgos = de_build;
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
            };
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
    let linea = redactar(linea);
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

/// `true` si el requisito se baja como wheel del índice: sin URL, sin `@` y sin opciones (`-e`, `-r`).
pub fn requisito_de_build_aceptable(spec: &str) -> bool {
    !(spec.contains("://") || spec.contains('@') || spec.starts_with('-'))
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
            hallazgos.push(hallazgo(
                ARCHIVO_PYPROJECT,
                0,
                "",
                1,
                format!(
                    "requisito de build con referencia directa: {}",
                    redactar(&spec)
                ),
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

/// `true` si una caché con sdists trae `requisitos-sdist.json`. Una caché anterior, sin ese archivo, no
/// sirve de acierto: se descarga de nuevo.
fn requisitos_sdist_listos(carpeta: &Path) -> Result<bool, ErrorOraculo> {
    Ok(nombres_sdist(carpeta)?.is_empty() || carpeta.join(ARCHIVO_REQUISITOS_SDIST).is_file())
}

/// Requisitos de build de cada sdist, en el orden de `nombres`, según el JSON del contenedor. Un sdist sin
/// lista (`null`) usa el respaldo de PEP 517. Sin JSON, es infraestructura.
fn requisitos_de_sdists(
    carpeta: &Path,
    nombres: &[String],
) -> Result<Vec<Vec<String>>, ErrorOraculo> {
    let ruta = carpeta.join(ARCHIVO_REQUISITOS_SDIST);
    let texto = fs::read_to_string(&ruta)
        .map_err(|e| infra("leer los requisitos de build de los sdists", e))?;
    let mapa: BTreeMap<String, Option<Vec<String>>> =
        serde_json::from_str(&texto).map_err(|e| {
            ErrorOraculo::Infra(format!("requisitos de build de los sdists ilegibles: {e}"))
        })?;
    Ok(nombres
        .iter()
        .map(|nombre| match mapa.get(nombre) {
            Some(Some(lista)) => lista
                .iter()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            _ => respaldo_de_build(),
        })
        .collect())
}

/// Rutas de los clones git de `req` bajo `src/`: `<clave>` o `<clave>/<subdirectory>`, sin repetidos.
fn rutas_clon(req: &RequisitosPip) -> Vec<String> {
    let rutas: BTreeSet<String> = req
        .directas
        .iter()
        .filter_map(|(_, raw)| partes_git(raw).ok())
        .map(|partes| {
            let dir = clave_clon(&partes);
            match partes.subdir {
                Some(subdir) => format!("{dir}/{subdir}"),
                None => dir,
            }
        })
        .collect();
    rutas.into_iter().collect()
}

/// Índice de build de las fuentes que necesitan build en `carpeta`: los sdists (con los requisitos que dejó
/// el contenedor) y los clones en `rutas_clon` (con el `pyproject` de cada uno o de su subdirectorio). `None`
/// si no hay ninguno, porque los wheels no necesitan build.
fn indice_de_build_de(
    carpeta: &Path,
    rutas_clon: &[String],
) -> Result<Option<IndiceBuild>, ErrorOraculo> {
    let sdists = nombres_sdist(carpeta)?;
    if sdists.is_empty() && rutas_clon.is_empty() {
        return Ok(None);
    }
    let mut listas = if sdists.is_empty() {
        Vec::new()
    } else {
        requisitos_de_sdists(carpeta, &sdists)?
    };
    listas.extend(rutas_clon.iter().map(|ruta| {
        requisitos_de_build(
            fs::read_to_string(carpeta.join("src").join(ruta).join(ARCHIVO_PYPROJECT))
                .ok()
                .as_deref(),
        )
    }));
    Ok(Some(indice_de_build(&listas)))
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
    fn descarga_requisitos_sdist_se_leen_en_el_contenedor_con_tarfile_y_zipfile() {
        let cmd = comando_requisitos_sdist();
        assert!(cmd.starts_with("python3 -I -c "), "{cmd}");
        for esperado in [
            "tarfile",
            "zipfile",
            "tomllib",
            "/cache/requisitos-sdist.json",
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

    /// Ejecutor de podman falso: responde un código y una salida fijos, y guarda los argumentos.
    struct PodmanDigest {
        codigo: i32,
        stdout: String,
        llamadas: std::cell::RefCell<Vec<Vec<String>>>,
    }

    impl PodmanDigest {
        fn que_responde(codigo: i32, stdout: &str) -> Self {
            PodmanDigest {
                codigo,
                stdout: stdout.into(),
                llamadas: std::cell::RefCell::new(Vec::new()),
            }
        }
    }

    impl Podman for PodmanDigest {
        fn correr(&self, args: &[&str], _plazo: Duration) -> Result<SalidaPodman, ErrorOraculo> {
            self.llamadas
                .borrow_mut()
                .push(args.iter().map(|a| a.to_string()).collect());
            Ok(SalidaPodman {
                codigo: self.codigo,
                stdout: self.stdout.clone(),
                stderr: String::new(),
            })
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
        let (texto, restricciones) = textos_de_requisitos(repo.path());
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
}
