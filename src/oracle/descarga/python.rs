//! Descarga de Python (Tarea 6c-B, decisión 21). Corre solo `pip download`, `curl` y `git`, con red.
//! Nunca corre `pip install`, `pip wheel` ni `setup.py`. Un sdist se baja y se guarda sin extraerlo.
//! Dentro de cada contenedor, `/cache` es la carpeta `<hash>.parcial` del requirements.

use std::fs;
use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::inventario::{RequisitosPip, redactar, requisitos_pip, separar_referencia};
use super::{
    ARCHIVO_REQUIREMENTS, CARPETA_CACHE, CARPETA_PYTHON, Diagnostico, Paso, Preparacion, Registro,
    borrar_en_home, corte, diagnosticar, entorno_descarga, error_de, hallazgo, hallazgo_no_instala,
    hash_corto, limpiar_parcial, parcial_de, temporal_de,
};
use crate::borrado;
use crate::oracle::contenedor::{self, Ejecucion};
use crate::oracle::tarea::Tarea;
use crate::oracle::{ErrorOraculo, infra};
use crate::tipos::Hallazgo;

/// Archivo con las líneas que `pip download` baja como wheel. Sin opciones ni URLs.
const ARCHIVO_INDICE: &str = "indice-pip.txt";
/// Lo que la evaluación instala sin red. Las URLs y los `git+` se cambian por rutas locales.
pub const ARCHIVO_OFFLINE: &str = "requirements-offline.txt";
const INDICE_PYPI: &str = "https://pypi.org/simple";
const JSON_PYPI: &str = "https://pypi.org/pypi";
const PREFIJO_SDIST_PYPI: &str = "https://files.pythonhosted.org/";
const ARCHIVO_JSON: &str = "pypi-meta.json";
/// Plazo de cada `curl` de un archivo de red.
const PLAZO_CURL_S: u32 = 60;

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

/// Comando que clona una fuente git sin hooks ni submódulos, y deja el commit obtenido en un archivo.
pub fn comando_clon(url: &str, dir: &str, commit: Option<&str>) -> String {
    let mut comando = format!(
        "mkdir -p /cache/src && git -c core.hooksPath=/dev/null -c protocol.allow=never -c protocol.https.allow=always clone --no-checkout {} /cache/src/{dir}",
        comilla(url)
    );
    if let Some(commit) = commit {
        comando.push_str(&format!(
            " && git -C /cache/src/{dir} -c core.hooksPath=/dev/null checkout --detach {commit}"
        ));
    }
    comando.push_str(&format!(
        " && git -C /cache/src/{dir} rev-parse HEAD > /cache/src/{dir}.commit"
    ));
    comando
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

/// Elige el sdist de una versión desde el JSON de `https://pypi.org/pypi/<n>/<v>/json`.
pub fn sdist_de_json(json: &str) -> Result<Sdist, String> {
    let v: Value = serde_json::from_str(json).map_err(|e| format!("JSON de PyPI ilegible: {e}"))?;
    let entradas = v["urls"]
        .as_array()
        .ok_or_else(|| "el JSON de PyPI no trae urls".to_string())?;
    let sdist = entradas
        .iter()
        .find(|e| e["packagetype"] == "sdist")
        .ok_or_else(|| "la versión no tiene sdist en PyPI".to_string())?;
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

/// Baja el sdist de `nombre==version` a `cache`. `pedir(url, archivo)` deja el archivo en `cache`.
/// Verifica el sha256 de PyPI. Un hash que no coincide borra el archivo y da error. Nunca lo extrae.
pub fn bajar_sdist_con<F>(
    nombre: &str,
    version: &str,
    cache: &Path,
    mut pedir: F,
) -> Result<Sdist, String>
where
    F: FnMut(&str, &str) -> Result<(), String>,
{
    if !caracteres_seguros(nombre) || !caracteres_seguros(version) {
        return Err(format!("paquete o versión no seguros: {nombre}=={version}"));
    }
    let url_json = format!("{JSON_PYPI}/{nombre}/{version}/json");
    pedir(&url_json, ARCHIVO_JSON)?;
    let ruta_json = cache.join(ARCHIVO_JSON);
    let json = fs::read_to_string(&ruta_json).map_err(|e| format!("leer el JSON de PyPI: {e}"))?;
    borrado::borrar_archivo_dentro(cache, &ruta_json)
        .map_err(|e| format!("borrar el JSON: {e}"))?;
    let sdist = sdist_de_json(&json)?;
    pedir(&sdist.url, &sdist.archivo)?;
    let ruta = cache.join(&sdist.archivo);
    let bytes = fs::read(&ruta).map_err(|e| format!("leer el sdist: {e}"))?;
    if sha256_hex(&bytes) != sdist.sha256 {
        let _ = borrado::borrar_archivo_dentro(cache, &ruta);
        return Err(format!(
            "el sha256 de {} no coincide: se descarta",
            sdist.archivo
        ));
    }
    Ok(sdist)
}

/// Un commit de 7 a 40 caracteres hexadecimales.
fn es_commit(texto: &str) -> bool {
    (7..=40).contains(&texto.len()) && texto.chars().all(|c| c.is_ascii_hexdigit())
}

/// Hallazgos de dependencias git sin commit fijo (la punta de la rama no es reproducible).
fn hallazgos_sin_commit(req: &RequisitosPip, texto: &str) -> Vec<Hallazgo> {
    req.directas
        .iter()
        .filter(|(_, raw)| raw.starts_with("git+"))
        .filter(|(_, raw)| {
            separar_referencia(
                raw.trim_start_matches("git+")
                    .split('#')
                    .next()
                    .unwrap_or(""),
            )
            .1
            .is_none()
        })
        .map(|(linea, raw)| {
            let cita = redactar(
                texto
                    .lines()
                    .nth((*linea as usize).saturating_sub(1))
                    .unwrap_or("")
                    .trim(),
            );
            hallazgo(
                ARCHIVO_REQUIREMENTS,
                *linea,
                &cita,
                2,
                format!(
                    "dependencia git sin commit fijo: {} : el build no es reproducible",
                    redactar(raw)
                ),
            )
        })
        .collect()
}

/// Resultado de correr un contenedor de descarga de Python: su código y su error.
struct Salida {
    codigo: i32,
    stderr: String,
}

/// Corre un comando de red con `parcial` montada en `/cache`, rw. Sin repo escribible.
fn correr(
    home: &Path,
    tarea: &Tarea,
    repo: &Path,
    parcial: &Path,
    cmd: String,
    timeout: Duration,
) -> Result<Salida, ErrorOraculo> {
    let salida = temporal_de(home)?;
    let ejecucion = Ejecucion {
        imagen: tarea.imagen.clone(),
        repo: Some(repo.to_path_buf()),
        oraculo: None,
        salida: salida.path().to_path_buf(),
        cmd,
        env: entorno_descarga(),
        timeout,
        red: true,
        cache: Some(parcial.to_path_buf()),
        cache_rw: true,
        rustsec: None,
    };
    let resultado = contenedor::ejecutar(&ejecucion)?;
    Ok(Salida {
        codigo: resultado.codigo,
        stderr: resultado.stderr,
    })
}

/// Sha256 de un archivo de la caché.
fn sha256_de(ruta: &Path) -> Result<String, ErrorOraculo> {
    let bytes = fs::read(ruta).map_err(|e| infra("leer un archivo de la caché de Python", e))?;
    Ok(sha256_hex(&bytes))
}

/// Resultado de la descarga: los archivos del Gate, o un corte con su hallazgo.
enum Fin {
    Lista(Vec<Value>),
    Corta(Hallazgo, &'static str),
}

/// Descarga de Python para `requirements.txt`. Cache por hash del archivo (`<hash>.parcial` y luego el nombre final).
pub(super) fn preparar_python(
    repo: &Path,
    tarea: &Tarea,
    home: &Path,
    timeout: Duration,
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
    let bytes = fs::read(&ruta).map_err(|e| infra("leer requirements.txt", e))?;
    let texto = String::from_utf8_lossy(&bytes).into_owned();
    let hash = hash_corto(&bytes);
    let req = requisitos_pip(&texto);
    let hallazgos = hallazgos_sin_commit(&req, &texto);
    let destino = home.join(CARPETA_CACHE).join(CARPETA_PYTHON).join(&hash);
    if destino.join(ARCHIVO_OFFLINE).is_file() {
        let registro = Registro {
            lenguaje: "python".into(),
            hash: Some(hash),
            cache_hit: true,
            paquetes: Vec::new(),
            rustsec: None,
            motivo: None,
            imagenes: None,
        };
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
    let fin = match descargar_en(home, tarea, repo, &parcial, &req, timeout) {
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
        Fin::Lista(paquetes) => {
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

/// Baja wheels, sdists, fuentes git y URLs dentro de `parcial`. Escribe el archivo offline.
fn descargar_en(
    home: &Path,
    tarea: &Tarea,
    repo: &Path,
    parcial: &Path,
    req: &RequisitosPip,
    timeout: Duration,
) -> Result<Fin, ErrorOraculo> {
    let mut paquetes: Vec<Value> = Vec::new();
    let mut pendientes: Vec<String> = req.especificadores.clone();
    while !pendientes.is_empty() {
        fs::write(
            parcial.join(ARCHIVO_INDICE),
            format!("{}\n", pendientes.join("\n")),
        )
        .map_err(|e| infra("escribir el índice de pip", e))?;
        let cmd = comando_wheels(&format!("/cache/{ARCHIVO_INDICE}"), req);
        let salida = correr(home, tarea, repo, parcial, cmd, timeout)?;
        if salida.codigo == 0 {
            break;
        }
        let Some(spec) = sin_wheel(&salida.stderr) else {
            return match diagnosticar(&salida.stderr) {
                Diagnostico::Producto(linea) => {
                    Ok(Fin::Corta(hallazgo_no_instala(&linea), "no-instala"))
                }
                otro => Err(error_de(otro)),
            };
        };
        let nombre = nombre_de_spec(&spec);
        let posicion = pendientes.iter().position(|s| nombre_de_spec(s) == nombre);
        let version = posicion.and_then(|i| version_fijada(&pendientes[i]));
        let (Some(pos), Some(version)) = (posicion, version) else {
            return Ok(Fin::Corta(
                hallazgo(
                    ARCHIVO_REQUIREMENTS,
                    0,
                    "",
                    2,
                    format!("dependencia sin wheel y sin versión fijada: {spec}"),
                ),
                "sin-version-fijada",
            ));
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
        let sdist = bajar_sdist_pypi(home, tarea, repo, parcial, timeout, &nombre, &version)?;
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
    let mut lineas: Vec<String> = req.especificadores.clone();
    for (linea, raw) in &req.directas {
        match fuente_directa(
            home,
            tarea,
            repo,
            parcial,
            timeout,
            *linea,
            raw,
            &mut paquetes,
        )? {
            Ok(ruta_local) => lineas.push(ruta_local),
            Err(corte) => return Ok(corte),
        }
    }
    fs::write(
        parcial.join(ARCHIVO_OFFLINE),
        format!("{}\n", lineas.join("\n")),
    )
    .map_err(|e| infra("escribir requirements-offline.txt", e))?;
    Ok(Fin::Lista(paquetes))
}

/// Baja el sdist de PyPI con `curl`, dentro del contenedor. Verifica el sha256 en el host.
fn bajar_sdist_pypi(
    home: &Path,
    tarea: &Tarea,
    repo: &Path,
    parcial: &Path,
    timeout: Duration,
    nombre: &str,
    version: &str,
) -> Result<Sdist, ErrorOraculo> {
    let pedir = |url: &str, archivo: &str| -> Result<(), String> {
        let cmd = comando_curl(url, archivo);
        match correr(home, tarea, repo, parcial, cmd, timeout) {
            Ok(salida) if salida.codigo == 0 => Ok(()),
            Ok(salida) => Err(salida.stderr.trim().chars().take(300).collect()),
            Err(error) => Err(error.to_string()),
        }
    };
    bajar_sdist_con(nombre, version, parcial, pedir).map_err(|motivo| {
        ErrorOraculo::Infra(format!("infraestructura: sdist de {nombre}: {motivo}"))
    })
}

/// Una fuente git o URL del requirements. `Ok(ruta)` es la línea local que va al archivo offline.
/// `Err(fin)` corta la evaluación con su hallazgo.
#[allow(clippy::too_many_arguments)]
fn fuente_directa(
    home: &Path,
    tarea: &Tarea,
    repo: &Path,
    parcial: &Path,
    timeout: Duration,
    linea: u32,
    raw: &str,
    paquetes: &mut Vec<Value>,
) -> Result<Result<String, Fin>, ErrorOraculo> {
    if let Some(sin_prefijo) = raw.strip_prefix("git+") {
        let sin_fragmento = sin_prefijo.split('#').next().unwrap_or(sin_prefijo);
        let (base, commit) = separar_referencia(sin_fragmento);
        if !caracteres_seguros(&base) || commit.as_deref().is_some_and(|c| !es_commit(c)) {
            return Ok(Err(Fin::Corta(
                hallazgo(
                    ARCHIVO_REQUIREMENTS,
                    linea,
                    "",
                    3,
                    "referencia git no segura".into(),
                ),
                "git-no-seguro",
            )));
        }
        let dir = hash_corto(base.as_bytes());
        let cmd = comando_clon(&base, &dir, commit.as_deref());
        let salida = correr(home, tarea, repo, parcial, cmd, timeout)?;
        if salida.codigo != 0 {
            return match diagnosticar(&salida.stderr) {
                Diagnostico::Producto(l) => {
                    Ok(Err(Fin::Corta(hallazgo_no_instala(&l), "no-instala")))
                }
                otro => Err(error_de(otro)),
            };
        }
        let commit_obtenido = fs::read_to_string(parcial.join("src").join(format!("{dir}.commit")))
            .map_err(|e| infra("leer el commit de un clon git", e))?
            .trim()
            .to_string();
        paquetes.push(json!({"nombre": base, "fuente": "git", "commit": commit_obtenido}));
        return Ok(Ok(format!("/cache/src/{dir}")));
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
    let salida = correr(
        home,
        tarea,
        repo,
        parcial,
        comando_curl(&url, &archivo),
        timeout,
    )?;
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
