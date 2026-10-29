//! Fase de descarga (decisiones 17 y 21): baja las dependencias con red antes de evaluar.
//! Antes de la red, `inventario` lista las fuentes del repo y aplica sus reglas. Corre en un
//! contenedor aparte. Ese contenedor nunca monta `/oraculo` y no ejecuta código del proyecto.
//! La evaluación usa la caché sin red.

pub mod credenciales;
pub mod inventario;
pub mod juez_config;
pub mod python;

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::configs::{instrucciones, palabra_clave, parsear_from};
use super::contenedor::{self, Ejecucion, correr_podman};
use super::entrega::{Eleccion, dockerfiles_del_repo, elegir_dockerfile};
use super::juez::ModeloJuez;
use super::tarea::Tarea;
use super::{CARPETA_TEMPORALES, ErrorOraculo, infra, temporal};
use crate::borrado;
use crate::observe;
use crate::tipos::{Dimension, Fuente, Hallazgo, TipoEvento};
use credenciales::{Credencial, credenciales_cargo, redactar_secretos};
use inventario::{
    EntradaConfig, HallazgoInventario, Inventario, Origen, PaqueteLock, Rechazo, analizar_lock,
};
use juez_config::ResultadoJuicio;

/// Lo que la evaluación necesita de la fase de descarga.
#[derive(Debug, Clone, PartialEq)]
pub enum Preparacion {
    /// La evaluación sigue. `cache` y `rustsec` son las carpetas que monta cada suite.
    Lista {
        cache: Option<PathBuf>,
        rustsec: Option<PathBuf>,
        hallazgos: Vec<Hallazgo>,
    },
    /// La evaluación no corre: no hay veredicto, solo hallazgos.
    SinVeredicto(Vec<Hallazgo>),
}

/// Lectura de la salida de error de un contenedor de descarga.
#[derive(Debug, Clone, PartialEq)]
pub enum Diagnostico {
    /// Registro caído o sin red: no hay veredicto.
    Infra(String),
    /// El producto no instala (paquete inexistente o retirado). Guarda la línea útil.
    Producto(String),
    /// Ningún patrón conocido. Se trata como infraestructura.
    Desconocido(String),
}

/// Qué hacer con la base de RustSec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accion {
    Clonar,
    Actualizar,
    Nada,
}

/// Prefijo de la suite de Python: instala desde `/cache` sin red, en `/tmp/pydeps` (la imagen no trae `ensurepip`).
pub const PRELUDIO_PYTHON: &str = "if python3 -c 'import ensurepip' 2>/dev/null; then python3 -m venv --system-site-packages /tmp/venv-eval && . /tmp/venv-eval/bin/activate && pip install --no-index --find-links /cache --no-input --disable-pip-version-check -r /cache/requirements-offline.txt; else mkdir -p /tmp/pydeps && python3 -m pip install --no-index --find-links /cache --no-input --disable-pip-version-check --target /tmp/pydeps -r /cache/requirements-offline.txt && export PYTHONPATH=/tmp/pydeps; fi && ";
/// Prefijo de cada suite: deja la caché de dependencias visible para cargo, sin escribir en ella.
pub const PRELUDIO: &str = "export CARGO_HOME=/tmp/cargo-home && mkdir -p \"$CARGO_HOME\" && cp /cache/config.toml \"$CARGO_HOME/config.toml\" && ";

const CARPETA_CACHE: &str = "cache";
const CARPETA_RUST: &str = "rust";
const CARPETA_PYTHON: &str = "python";
const CARPETA_RUSTSEC: &str = "rustsec";
const SUFIJO_PARCIAL: &str = ".parcial";
const ARCHIVO_CONFIG: &str = "config.toml";
const ARCHIVO_LOCK: &str = "Cargo.lock";
const ARCHIVO_REQUIREMENTS: &str = "requirements.txt";
const ARCHIVO_EVENTOS: &str = "eventos.jsonl";
const CARGO_HOME_DESCARGA: &str = "/tmp/cargo-dl";
// Cargo lee `.cargo/config.toml` desde el cwd y sus padres. La descarga corre desde `/repo`, como un build
// del proyecto: respeta su config, incluido un proxy o un registro (decisión 30). Los padres de `/repo`
// en el contenedor no tienen config.
const COMANDO_VENDOR: &str =
    "cd /repo && cargo vendor --locked --versioned-dirs /cache/vendor > /cache/config.toml";
// La copia `/tmp/w` trae `.cargo/` del repo: el lock sale con el mismo config que la descarga.
const COMANDO_LOCK: &str = "mkdir -p /tmp/w && cp -r /repo/. /tmp/w/ && cd /tmp/w && cargo generate-lockfile && cp /tmp/w/Cargo.lock /salida/Cargo.lock";
const URL_RUSTSEC: &str = "https://github.com/RustSec/advisory-db";
const CADUCIDAD_RUSTSEC: Duration = Duration::from_secs(24 * 3600);
/// Fragmentos (en minúscula) de errores de red o de registro caído.
const PATRONES_INFRA: [&str; 5] = [
    "failed to download",
    "could not resolve host",
    "network is unreachable",
    "spurious network error",
    "timed out",
];
/// Fragmentos (en minúscula) de errores de un paquete que no existe o fue retirado.
const PATRONES_PRODUCTO: [&str; 4] = [
    "no matching package",
    "failed to select a version",
    "yanked",
    "no matching distribution found",
];
/// Largo máximo de la línea útil de un error.
const LARGO_LINEA_UTIL: usize = 300;
/// Plazo de cada `podman pull` de una imagen base.
const PLAZO_PULL: Duration = Duration::from_secs(15 * 60);
/// Plazo de las consultas rápidas de podman: inspeccionar una imagen.
const PLAZO_CORTO: Duration = Duration::from_secs(60);
/// Fragmentos (en minúscula) de un pedido explícito de autenticación (401). Root no pasa credenciales (P21).
/// Van antes que los textos ambiguos: un 401 con "not found" en el mismo texto sigue siendo un 401.
const PATRONES_PIDE_CREDENCIALES: [&str; 3] = [
    "authentication required",
    "401 unauthorized",
    "basic auth credentials",
];
/// Fragmentos (en minúscula) de un texto ambiguo: la imagen no existe o es privada.
const PATRONES_IMAGEN_INACCESIBLE: [&str; 6] = [
    "requested access to the resource is denied",
    "pull access denied",
    "manifest unknown",
    "name unknown",
    "not found",
    "unauthorized",
];

/// Hash corto de un contenido: primeros 16 caracteres hex de su sha256.
pub fn hash_corto(contenido: &[u8]) -> String {
    Sha256::digest(contenido)
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Clasifica el stderr de un contenedor de descarga. La infraestructura gana a un producto roto.
pub fn diagnosticar(stderr: &str) -> Diagnostico {
    let minuscula = stderr.to_lowercase();
    if PATRONES_INFRA.iter().any(|p| minuscula.contains(p)) {
        return Diagnostico::Infra(linea_util(stderr));
    }
    if PATRONES_PRODUCTO.iter().any(|p| minuscula.contains(p)) {
        return Diagnostico::Producto(linea_util(stderr));
    }
    Diagnostico::Desconocido(linea_util(stderr))
}

/// Primera línea de error del texto. Sin una, la primera línea no vacía.
fn linea_util(texto: &str) -> String {
    let lineas: Vec<&str> = texto
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    lineas
        .iter()
        .find(|l| l.to_lowercase().contains("error"))
        .or(lineas.first())
        .map_or("sin salida de error".to_string(), |l| {
            l.chars().take(LARGO_LINEA_UTIL).collect()
        })
}

/// Comando de `pip download` contra PyPI. El destino es `/cache`.
pub fn comando_pip(requirements: &str) -> String {
    format!(
        "python3 -m pip download -r {requirements} --index-url https://pypi.org/simple -d /cache"
    )
}

/// Qué hacer con la base de RustSec según si existe y su edad.
pub fn accion_rustsec(existe: bool, edad: Option<Duration>) -> Accion {
    if !existe {
        return Accion::Clonar;
    }
    match edad {
        Some(edad) if edad <= CADUCIDAD_RUSTSEC => Accion::Nada,
        _ => Accion::Actualizar,
    }
}

/// Borra la carpeta parcial de una descarga anterior que no terminó. Solo borra dentro de `home`.
pub fn limpiar_parcial(home: &Path, dir: &Path) -> io::Result<()> {
    borrar_en_home(home, dir)
}

/// Borra una carpeta de la caché de `home` con la guarda de `borrado`. Un destino fuera de `home` se rechaza.
fn borrar_en_home(home: &Path, ruta: &Path) -> io::Result<()> {
    borrado::borrar_dir_dentro(home, ruta)
}

/// Hallazgo de entrega. El `id` sale de la dimensión, el archivo y la cita.
fn hallazgo(archivo: &str, linea: u32, cita: &str, severidad: u8, detalle: String) -> Hallazgo {
    hallazgo_en(Dimension::Entrega, archivo, linea, cita, severidad, detalle)
}

/// Hallazgo de una dimensión dada. El `id` sale de la dimensión, el archivo y la cita.
fn hallazgo_en(
    dimension: Dimension,
    archivo: &str,
    linea: u32,
    cita: &str,
    severidad: u8,
    detalle: String,
) -> Hallazgo {
    let etiqueta = serde_json::to_value(dimension)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default();
    let id = hash_corto(format!("{etiqueta}|{archivo}|{cita}").as_bytes());
    Hallazgo {
        dimension,
        severidad,
        archivo: archivo.into(),
        linea,
        cita: cita.into(),
        detalle,
        fuente: Fuente::Sonda("descarga".into()),
        id,
        impacto: Vec::new(),
    }
}

fn hallazgo_no_instala(linea: &str) -> Hallazgo {
    hallazgo(
        "Cargo.toml",
        0,
        linea,
        3,
        format!("el producto no instala: {linea}"),
    )
}

fn hallazgo_sin_lock() -> Hallazgo {
    hallazgo(
        ARCHIVO_LOCK,
        0,
        "",
        2,
        "el repo no trae Cargo.lock: el build no es reproducible".into(),
    )
}

fn hallazgo_lock_invalido(error: &str) -> Hallazgo {
    hallazgo(
        ARCHIVO_LOCK,
        0,
        "",
        3,
        format!("Cargo.lock inválido: {error}"),
    )
}

/// Lo que se registra en el evento `Gate` de una descarga.
struct Registro {
    lenguaje: String,
    hash: Option<String>,
    cache_hit: bool,
    paquetes: Vec<Value>,
    rustsec: Option<String>,
    motivo: Option<&'static str>,
    /// Imágenes base de los Dockerfile del repo. `None` si el repo no trae Dockerfile.
    imagenes: Option<Vec<ImagenBase>>,
}

impl Registro {
    /// Registro de una descarga que cortó la evaluación.
    fn corte(lenguaje: &str, hash: Option<String>, motivo: &'static str) -> Self {
        Registro {
            motivo: Some(motivo),
            hash,
            ..Registro::sin_paquetes(lenguaje)
        }
    }

    /// Registro sin paquetes ni caché: lo que deja una fase que no descarga dependencias.
    fn sin_paquetes(lenguaje: &str) -> Self {
        Registro {
            lenguaje: lenguaje.into(),
            hash: None,
            cache_hit: false,
            paquetes: Vec::new(),
            rustsec: None,
            motivo: None,
            imagenes: None,
        }
    }
}

/// Corre la fase de descarga para la tarea. Ver las decisiones 17 y 21 en `root.plan.md`.
/// Orden fijo: inventario sin red, Gate del inventario, rechazos sin red, y recién después la red.
pub fn preparar(
    repo: &Path,
    tarea: &Tarea,
    home: &Path,
    timeout: Duration,
) -> Result<Preparacion, ErrorOraculo> {
    preparar_con_imagenes(repo, tarea, home, timeout).map(|(preparacion, _)| preparacion)
}

/// Como `preparar`, y además devuelve las imágenes base que la fase vio (con su problema, si lo hubo).
pub fn preparar_con_imagenes(
    repo: &Path,
    tarea: &Tarea,
    home: &Path,
    timeout: Duration,
) -> Result<(Preparacion, Vec<ImagenBase>), ErrorOraculo> {
    preparar_con_juez(repo, tarea, home, timeout, None)
        .map(|(preparacion, imagenes, _)| (preparacion, imagenes))
}

/// Como `preparar_con_imagenes`, con el juez de la configuración de Cargo (decisión 30): corre después
/// del Gate del inventario y antes de la red, y solo informa. Nadie de producción pasa un juez todavía.
pub fn preparar_con_juez(
    repo: &Path,
    tarea: &Tarea,
    home: &Path,
    timeout: Duration,
    juez: Option<&dyn ModeloJuez>,
) -> Result<(Preparacion, Vec<ImagenBase>, Option<ResultadoJuicio>), ErrorOraculo> {
    let inv = inventario::inventariar(repo)?;
    registrar_inventario(home, tarea, &inv)?;
    let juicio = match juez {
        Some(juez) => {
            let resultado = juez_config::juzgar_repo(juez, repo, tarea, &inv);
            if !inv.config_cargo.is_empty() {
                registrar_juicio(home, tarea, &resultado)?;
            }
            Some(resultado)
        }
        None => None,
    };
    // Los hallazgos del inventario (severidad 1, `--extra-index-url`) llegan a cada veredicto (Tarea 6c-B).
    // Los del juicio de la configuración van con ellos.
    let mut extra = hallazgos_de_inventario(&inv.hallazgos);
    if let Some(resultado) = &juicio {
        extra.extend(juez_config::hallazgos_de_juicio(
            resultado,
            &inv.config_cargo,
        ));
    }
    if !inv.rechazos.is_empty() {
        let mut hallazgos = hallazgos_de_rechazos(&inv.rechazos);
        hallazgos.extend(extra);
        return Ok((Preparacion::SinVeredicto(hallazgos), Vec::new(), juicio));
    }
    // Las credenciales de los registros privados salen de `root.toml` y del entorno de Root. Solo van a
    // los contenedores de descarga de Rust: nunca a la evaluación, a Python ni a la construcción (decisión 29).
    let config = crate::config::Config::cargar(&home.join("root.toml"))
        .map_err(|e| ErrorOraculo::Infra(format!("configuración de registros: {e}")))?;
    let credenciales =
        credenciales_cargo(&inv, &config.registros, |nombre| std::env::var(nombre).ok());
    // Las imágenes base van antes de las dependencias: sin ellas el Dockerfile no construye (Tarea 9d2).
    // Un pull que falla no corta la fase: queda en el Gate y la construcción da sin veredicto (Tarea 7i).
    let imagenes = if hay_dockerfile(repo) {
        Some(traer_imagenes_base_de(repo, &elegido(repo, tarea))?)
    } else {
        None
    };
    let devueltas = imagenes.clone().unwrap_or_default();
    if imagenes
        .iter()
        .flatten()
        .any(|i| i.problema == Some(Problema::PideCredenciales))
    {
        // Sin veredicto: no se pasan credenciales, así que no se sigue con la descarga.
        let mut registro = Registro::sin_paquetes(&tarea.lenguaje);
        registro.motivo = Some("imagen base: pide credenciales");
        registro.imagenes = imagenes;
        registrar(home, tarea, &registro)?;
        return Ok((Preparacion::SinVeredicto(extra), devueltas, juicio));
    }
    let (preparacion, registro) = match tarea.lenguaje.as_str() {
        "rust" => preparar_rust(repo, tarea, home, timeout, &inv, &credenciales)?,
        "python" => python::preparar_python(repo, tarea, home, timeout)?,
        _ => (
            Preparacion::Lista {
                cache: None,
                rustsec: None,
                hallazgos: Vec::new(),
            },
            None,
        ),
    };
    let registro = match (registro, imagenes) {
        (Some(mut registro), imagenes) => {
            registro.imagenes = imagenes;
            Some(registro)
        }
        (None, Some(imagenes)) => {
            let mut registro = Registro::sin_paquetes(&tarea.lenguaje);
            registro.imagenes = Some(imagenes);
            Some(registro)
        }
        (None, None) => None,
    };
    if let Some(registro) = registro {
        registrar(home, tarea, &registro)?;
    }
    Ok((sumar_hallazgos(preparacion, extra), devueltas, juicio))
}

/// Gate de la fase `juez-config`: el juicio de la configuración, o la causa de su sin veredicto.
fn registrar_juicio(
    home: &Path,
    tarea: &Tarea,
    resultado: &ResultadoJuicio,
) -> Result<(), ErrorOraculo> {
    let mut extra = serde_json::Map::new();
    match resultado {
        ResultadoJuicio::Juzgado {
            juicios,
            descartados,
        } => {
            extra.insert("estado".into(), json!("juzgado"));
            extra.insert("juicios".into(), json!(juicios));
            extra.insert("descartados".into(), json!(descartados));
        }
        ResultadoJuicio::SinVeredicto { causa } => {
            extra.insert("estado".into(), json!("sin_veredicto"));
            extra.insert("causa".into(), json!(causa));
        }
    }
    let evento = observe::evento_gate(&format!("descarga-{}", tarea.nombre), "juez-config", extra);
    observe::registrar_en(&home.join(ARCHIVO_EVENTOS), &evento)
        .map_err(|e| infra("registrar el juicio de la configuración", e))
}

/// Gate de la fase `inventario`: las fuentes que el repo declara, antes de usar la red.
fn registrar_inventario(home: &Path, tarea: &Tarea, inv: &Inventario) -> Result<(), ErrorOraculo> {
    let mut extra = serde_json::Map::new();
    extra.insert("lenguaje".into(), json!(tarea.lenguaje));
    extra.insert("fuentes".into(), json!(inv.fuentes));
    extra.insert("rechazos".into(), json!(inv.rechazos));
    extra.insert("avisos".into(), json!(inv.avisos));
    extra.insert("hallazgos".into(), json!(inv.hallazgos));
    // Solo lo que toca la red: el Gate lo lista antes de que la descarga la use (decisión 30).
    let config_cargo: Vec<Value> = inv
        .config_cargo
        .iter()
        .filter(|e| e.red)
        .map(entrada_config_gate)
        .collect();
    extra.insert("config_cargo".into(), Value::Array(config_cargo));
    let evento = observe::evento_gate(&format!("descarga-{}", tarea.nombre), "inventario", extra);
    observe::registrar_en(&home.join(ARCHIVO_EVENTOS), &evento)
        .map_err(|e| infra("registrar el inventario de fuentes", e))
}

/// Lo que el `Gate` muestra de una entrada de config que toca la red.
fn entrada_config_gate(entrada: &EntradaConfig) -> Value {
    json!({
        "archivo": entrada.archivo,
        "linea": entrada.linea,
        "clave": entrada.clave,
        "cita": entrada.cita,
    })
}

/// Un hallazgo por cada hallazgo del inventario, con su dimensión. Las citas ya vienen redactadas.
fn hallazgos_de_inventario(hallazgos: &[HallazgoInventario]) -> Vec<Hallazgo> {
    hallazgos
        .iter()
        .map(|h| {
            hallazgo_en(
                h.dimension,
                &h.archivo,
                h.linea,
                &h.cita,
                h.severidad,
                h.detalle.clone(),
            )
        })
        .collect()
}

/// Suma hallazgos a una preparación. No cambia si la evaluación sigue o corta.
fn sumar_hallazgos(preparacion: Preparacion, extra: Vec<Hallazgo>) -> Preparacion {
    match preparacion {
        Preparacion::Lista {
            cache,
            rustsec,
            mut hallazgos,
        } => {
            hallazgos.extend(extra);
            Preparacion::Lista {
                cache,
                rustsec,
                hallazgos,
            }
        }
        Preparacion::SinVeredicto(mut hallazgos) => {
            hallazgos.extend(extra);
            Preparacion::SinVeredicto(hallazgos)
        }
    }
}

/// Un hallazgo de entrega por cada rechazo del inventario. La cita ya viene redactada.
fn hallazgos_de_rechazos(rechazos: &[Rechazo]) -> Vec<Hallazgo> {
    rechazos
        .iter()
        .map(|r| {
            hallazgo(
                &r.archivo,
                r.linea,
                &r.cita,
                3,
                format!("fuente no aceptada: {}", r.motivo),
            )
        })
        .collect()
}

type Paso = (Preparacion, Option<Registro>);

fn preparar_rust(
    repo: &Path,
    tarea: &Tarea,
    home: &Path,
    timeout: Duration,
    inv: &Inventario,
    credenciales: &[Credencial],
) -> Result<Paso, ErrorOraculo> {
    let mut hallazgos = Vec::new();
    let lock = repo.join(ARCHIVO_LOCK);
    if !lock.is_file() {
        if let Some(linea) = generar_lock(repo, tarea, home, timeout, credenciales)? {
            return Ok(corte(
                hallazgo_no_instala(&linea),
                Registro::corte("rust", None, "no-instala"),
            ));
        }
        hallazgos.push(hallazgo_sin_lock());
    }
    let texto = fs::read_to_string(&lock).map_err(|e| infra("leer Cargo.lock", e))?;
    let hash = hash_corto(texto.as_bytes());
    let paquetes = match analizar_lock(&texto) {
        Ok(paquetes) => paquetes,
        Err(error) => {
            return Ok(corte(
                hallazgo_lock_invalido(&error),
                Registro::corte("rust", Some(hash), "lock-invalido"),
            ));
        }
    };
    let destino = home.join(CARPETA_CACHE).join(CARPETA_RUST).join(&hash);
    let cache_hit = destino.join(ARCHIVO_CONFIG).is_file();
    if !cache_hit
        && let Some(linea) = descargar_vendor(repo, tarea, home, &hash, timeout, credenciales)?
    {
        return Ok(corte(
            hallazgo_no_instala(&linea),
            Registro::corte("rust", Some(hash), "no-instala"),
        ));
    }
    // Una dependencia git sin commit fijo: el build no es reproducible (decisión 21).
    for aviso in &inv.sin_commit {
        hallazgos.push(hallazgo(
            &aviso.archivo,
            aviso.linea,
            &aviso.cita,
            2,
            aviso.detalle.clone(),
        ));
    }
    let (rustsec, estado) = actualizar_rustsec(home, tarea, timeout)?;
    let registro = Registro {
        lenguaje: "rust".into(),
        hash: Some(hash),
        cache_hit,
        paquetes: paquetes_json(&paquetes),
        rustsec: Some(estado),
        motivo: None,
        imagenes: None,
    };
    let preparacion = Preparacion::Lista {
        cache: Some(destino),
        rustsec,
        hallazgos,
    };
    Ok((preparacion, Some(registro)))
}

fn corte(hallazgo: Hallazgo, registro: Registro) -> Paso {
    (Preparacion::SinVeredicto(vec![hallazgo]), Some(registro))
}

/// Carpeta temporal de la descarga, dentro de `ROOT_HOME/tmp`.
fn temporal_de(home: &Path) -> Result<tempfile::TempDir, ErrorOraculo> {
    let tmp = home.join(CARPETA_TEMPORALES);
    fs::create_dir_all(&tmp).map_err(|e| infra("directorio temporal de ROOT_HOME", e))?;
    temporal(&tmp).map_err(|e| infra("directorio temporal de la descarga", e))
}

/// Variables de la descarga: red abierta y `CARGO_HOME` en tmpfs.
fn entorno_descarga() -> Vec<(String, String)> {
    vec![
        ("CARGO_NET_OFFLINE".into(), "false".into()),
        ("CARGO_HOME".into(), CARGO_HOME_DESCARGA.into()),
    ]
}

/// Corre un contenedor de descarga. Un fallo llega como `Diagnostico`, sin ningún valor de `credenciales`.
fn ejecutar_descarga(
    ejecucion: &Ejecucion,
    credenciales: &[Credencial],
) -> Result<(), Diagnostico> {
    let resultado = match contenedor::ejecutar(ejecucion) {
        Err(ErrorOraculo::Infra(m)) => Err(Diagnostico::Infra(m)),
        Err(otro) => Err(Diagnostico::Desconocido(otro.to_string())),
        Ok(resultado) if resultado.codigo == 0 => Ok(()),
        Ok(resultado) => Err(diagnosticar(&resultado.stderr)),
    };
    resultado.map_err(|diagnostico| redactar_diagnostico(diagnostico, credenciales))
}

/// Quita de un diagnóstico cualquier valor de credencial (decisión 29).
fn redactar_diagnostico(diagnostico: Diagnostico, credenciales: &[Credencial]) -> Diagnostico {
    let quitar = |texto: String| redactar_secretos(&texto, credenciales);
    match diagnostico {
        Diagnostico::Infra(m) => Diagnostico::Infra(quitar(m)),
        Diagnostico::Producto(m) => Diagnostico::Producto(quitar(m)),
        Diagnostico::Desconocido(m) => Diagnostico::Desconocido(quitar(m)),
    }
}

/// Credenciales como pares (variable, valor) para el entorno del contenedor de descarga.
fn secretos_de(credenciales: &[Credencial]) -> Vec<(String, String)> {
    credenciales
        .iter()
        .map(|c| (c.variable.clone(), c.valor().to_string()))
        .collect()
}

/// Error de la evaluación para un fallo de descarga que no es del producto.
fn error_de(diagnostico: Diagnostico) -> ErrorOraculo {
    match diagnostico {
        Diagnostico::Infra(m) => ErrorOraculo::Infra(format!("infraestructura: {m}")),
        Diagnostico::Producto(m) | Diagnostico::Desconocido(m) => {
            ErrorOraculo::Infra(format!("la descarga falló: {m}"))
        }
    }
}

/// Corre `cargo generate-lockfile` en una copia descartable y copia el lock al repo.
/// Da `Some(linea)` si el producto no instala. Las `credenciales` van solo a este contenedor de descarga.
fn generar_lock(
    repo: &Path,
    tarea: &Tarea,
    home: &Path,
    timeout: Duration,
    credenciales: &[Credencial],
) -> Result<Option<String>, ErrorOraculo> {
    let salida = temporal_de(home)?;
    let ejecucion = Ejecucion {
        imagen: tarea.imagen.clone(),
        repo: Some(repo.to_path_buf()),
        oraculo: None,
        salida: salida.path().to_path_buf(),
        cmd: COMANDO_LOCK.into(),
        env: entorno_descarga(),
        timeout,
        red: true,
        cache: None,
        cache_rw: false,
        rustsec: None,
        secretos: secretos_de(credenciales),
    };
    match ejecutar_descarga(&ejecucion, credenciales) {
        Ok(()) => {
            fs::copy(salida.path().join(ARCHIVO_LOCK), repo.join(ARCHIVO_LOCK))
                .map_err(|e| infra("copiar el Cargo.lock generado", e))?;
            Ok(None)
        }
        Err(Diagnostico::Producto(linea)) => Ok(Some(linea)),
        Err(diagnostico) => Err(error_de(diagnostico)),
    }
}

/// Carpeta parcial de una caché: `<hash>.parcial`, al lado de la carpeta final.
fn parcial_de(dir: &Path) -> PathBuf {
    let nombre = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    dir.with_file_name(format!("{nombre}{SUFIJO_PARCIAL}"))
}

/// Corre `cargo vendor` en la carpeta parcial y la renombra al final.
/// Da `Some(linea)` si el producto no instala. Un fallo borra la carpeta parcial.
/// Las `credenciales` van solo a este contenedor de descarga.
fn descargar_vendor(
    repo: &Path,
    tarea: &Tarea,
    home: &Path,
    hash: &str,
    timeout: Duration,
    credenciales: &[Credencial],
) -> Result<Option<String>, ErrorOraculo> {
    let destino = home.join(CARPETA_CACHE).join(CARPETA_RUST).join(hash);
    let parcial = parcial_de(&destino);
    limpiar_parcial(home, &parcial).map_err(|e| infra("borrar la descarga parcial", e))?;
    fs::create_dir_all(&parcial).map_err(|e| infra("crear la caché parcial", e))?;
    let salida = temporal_de(home)?;
    let ejecucion = Ejecucion {
        imagen: tarea.imagen.clone(),
        repo: Some(repo.to_path_buf()),
        oraculo: None,
        salida: salida.path().to_path_buf(),
        cmd: COMANDO_VENDOR.into(),
        env: entorno_descarga(),
        timeout,
        red: true,
        cache: Some(parcial.clone()),
        cache_rw: true,
        rustsec: None,
        secretos: secretos_de(credenciales),
    };
    match ejecutar_descarga(&ejecucion, credenciales) {
        Ok(()) if parcial.join(ARCHIVO_CONFIG).is_file() => {
            if destino.exists() {
                borrar_en_home(home, &destino).map_err(|e| infra("borrar la caché previa", e))?;
            }
            fs::rename(&parcial, &destino).map_err(|e| infra("cerrar la caché", e))?;
            Ok(None)
        }
        Ok(()) => {
            limpiar_parcial(home, &parcial).map_err(|e| infra("borrar la descarga parcial", e))?;
            Err(ErrorOraculo::Infra(
                "infraestructura: cargo vendor no dejó config.toml".into(),
            ))
        }
        Err(diagnostico) => {
            limpiar_parcial(home, &parcial).map_err(|e| infra("borrar la descarga parcial", e))?;
            match diagnostico {
                Diagnostico::Producto(linea) => Ok(Some(linea)),
                otro => Err(error_de(otro)),
            }
        }
    }
}

/// Antigüedad de la base de RustSec: la de `FETCH_HEAD` si existe, si no la de la carpeta.
fn edad_de(dir: &Path) -> Option<Duration> {
    let marca = dir.join(".git").join("FETCH_HEAD");
    let ruta = if marca.is_file() {
        marca
    } else {
        dir.to_path_buf()
    };
    fs::metadata(ruta).ok()?.modified().ok()?.elapsed().ok()
}

/// Deja la base de RustSec al día. Un fallo no bloquea: vuelve como estado `error: ...`.
fn actualizar_rustsec(
    home: &Path,
    tarea: &Tarea,
    timeout: Duration,
) -> Result<(Option<PathBuf>, String), ErrorOraculo> {
    let dir = home.join(CARPETA_CACHE).join(CARPETA_RUSTSEC);
    let existe = dir.is_dir();
    let accion = accion_rustsec(existe, if existe { edad_de(&dir) } else { None });
    if accion == Accion::Nada {
        return Ok((Some(dir), "cache".into()));
    }
    let cmd = match accion {
        Accion::Clonar => {
            fs::create_dir_all(&dir).map_err(|e| infra("crear la base de RustSec", e))?;
            format!("git clone --depth 1 {URL_RUSTSEC} /cache")
        }
        _ => "git -C /cache pull --ff-only".to_string(),
    };
    let salida = temporal_de(home)?;
    let ejecucion = Ejecucion {
        imagen: tarea.imagen.clone(),
        repo: None,
        oraculo: None,
        salida: salida.path().to_path_buf(),
        cmd,
        env: Vec::new(),
        timeout,
        red: true,
        cache: Some(dir.clone()),
        cache_rw: true,
        rustsec: None,
        secretos: Vec::new(),
    };
    match ejecutar_descarga(&ejecucion, &[]) {
        Ok(()) => Ok((Some(dir), "ok".into())),
        Err(diagnostico) => {
            let motivo = match diagnostico {
                Diagnostico::Infra(m) | Diagnostico::Producto(m) | Diagnostico::Desconocido(m) => m,
            };
            if accion == Accion::Clonar {
                let _ = borrar_en_home(home, &dir);
                return Ok((None, format!("error: {motivo}")));
            }
            Ok((Some(dir), format!("error: {motivo}")))
        }
    }
}

/// Paquetes del lock para el Gate de la descarga. Un paquete git lleva el commit obtenido.
fn paquetes_json(paquetes: &[PaqueteLock]) -> Vec<Value> {
    paquetes
        .iter()
        .map(|p| {
            let (fuente, commit) = match &p.origen {
                Origen::Workspace => ("workspace", None),
                Origen::CratesIo => ("crates.io", None),
                Origen::Git { commit, .. } => ("git", commit.clone()),
                Origen::Registro { .. } => ("registro", None),
                Origen::Otro(_) => ("otro", None),
            };
            let mut paquete = json!({"nombre": p.nombre, "version": p.version, "fuente": fuente});
            if let Some(commit) = commit {
                paquete["commit"] = json!(commit);
            }
            paquete
        })
        .collect()
}

/// Registra el evento `Gate` de la descarga en `eventos.jsonl` de `home`.
fn registrar(home: &Path, tarea: &Tarea, registro: &Registro) -> Result<(), ErrorOraculo> {
    let mut evento = observe::evento_manual(
        &format!("descarga-{}", tarea.nombre),
        TipoEvento::Gate,
        None,
    );
    evento.fase = "descarga".into();
    evento
        .extra
        .insert("lenguaje".into(), json!(registro.lenguaje));
    evento.extra.insert("hash".into(), json!(registro.hash));
    evento
        .extra
        .insert("cache_hit".into(), json!(registro.cache_hit));
    evento
        .extra
        .insert("paquetes".into(), Value::Array(registro.paquetes.clone()));
    if let Some(rustsec) = &registro.rustsec {
        evento.extra.insert("rustsec".into(), json!(rustsec));
    }
    if let Some(motivo) = registro.motivo {
        evento.extra.insert("motivo".into(), json!(motivo));
    }
    if let Some(imagenes) = &registro.imagenes {
        evento.extra.insert("imagenes".into(), json!(imagenes));
    }
    observe::registrar_en(&home.join(ARCHIVO_EVENTOS), &evento)
        .map_err(|e| infra("registrar el evento de descarga", e))
}

/// Imagen base de un `FROM` de un Dockerfile del repo (Tarea 9d2). Sin `digest` no hubo imagen local.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ImagenBase {
    pub archivo: String,
    pub linea: u32,
    pub cita: String,
    /// La imagen tal como está escrita en el `FROM`. El `Gate` la lista así.
    pub imagen: String,
    /// La imagen con los `ARG` resueltos. `None` si la variable quedó sin valor.
    pub resuelta: Option<String>,
    pub digest: Option<String>,
    /// Verdadero solo si `podman pull` bajó la imagen en esta fase.
    pub descargada: bool,
    /// Motivo por el que no se resolvió el `FROM` (una variable sin valor).
    pub sin_resolver: Option<String>,
    /// Por qué el registro no entregó la imagen.
    pub problema: Option<Problema>,
}

/// Motivo por el que una imagen base no se bajó. En el `Gate` se escribe como texto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problema {
    /// Inexistente o privada: el registro responde lo mismo para los dos casos.
    ImagenInaccesible,
    /// Un pedido explícito de autenticación (401). Root no pasa credenciales.
    PideCredenciales,
    /// Red o registro caído. Guarda la causa, sin el prefijo.
    Infraestructura(String),
}

impl Serialize for Problema {
    fn serialize<S: serde::Serializer>(&self, serializador: S) -> Result<S::Ok, S::Error> {
        let texto = match self {
            Problema::ImagenInaccesible => "imagen_inaccesible".to_string(),
            Problema::PideCredenciales => "pide_credenciales".to_string(),
            Problema::Infraestructura(causa) => format!("infraestructura: {causa}"),
        };
        serializador.serialize_str(&texto)
    }
}

/// Un `FROM` con su imagen resuelta. `resuelto` es `Err` si una variable quedó sin valor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FromResuelto {
    pub linea: u32,
    /// La imagen tal como está escrita en el `FROM`.
    pub escrito: String,
    pub resuelto: Result<String, String>,
}

/// Resuelve los `FROM` de un Dockerfile con los `ARG` que van antes del primer `FROM` (Tarea 7i).
/// Un `ARG` después del primer `FROM` es de su etapa: no entra en los `FROM`, como en `docker build`.
pub fn resolver_froms(texto: &str) -> Vec<FromResuelto> {
    froms_del_texto(texto).into_iter().map(|f| f.from).collect()
}

/// Un `FROM` del texto, con su alias de etapa y su cita (para el `Gate` y la construcción).
struct FromEnTexto {
    from: FromResuelto,
    alias: Option<String>,
    cita: String,
}

fn froms_del_texto(texto: &str) -> Vec<FromEnTexto> {
    let mut globales: BTreeMap<String, Option<String>> = BTreeMap::new();
    let mut vio_from = false;
    let mut salida = Vec::new();
    for inst in instrucciones(texto) {
        let (kw, resto) = palabra_clave(&inst.logico);
        match kw.as_str() {
            "ARG" if !vio_from => globales.extend(args_declarados(resto)),
            "FROM" => {
                vio_from = true;
                let (imagen, alias) = parsear_from(resto);
                let Some(escrito) = imagen else {
                    continue;
                };
                let resuelto = expandir(&escrito, &globales);
                salida.push(FromEnTexto {
                    from: FromResuelto {
                        linea: inst.linea,
                        escrito,
                        resuelto,
                    },
                    alias,
                    cita: inst.cita.clone(),
                });
            }
            _ => {}
        }
    }
    salida
}

/// Nombres y valores por defecto de un `ARG`. `ARG NOMBRE` no tiene valor.
fn args_declarados(resto: &str) -> Vec<(String, Option<String>)> {
    resto
        .split_whitespace()
        .map(|token| match token.split_once('=') {
            Some((nombre, valor)) => (nombre.to_string(), Some(quitar_comillas(valor))),
            None => (token.to_string(), None),
        })
        .collect()
}

fn quitar_comillas(texto: &str) -> String {
    texto.trim_matches(|c| c == '"' || c == '\'').to_string()
}

/// Sustituye `${NOMBRE}`, `${NOMBRE:-defecto}` y `$NOMBRE` con los `ARG` globales.
/// Una variable sin valor, o un resultado vacío, es `Err` con el motivo.
fn expandir(texto: &str, globales: &BTreeMap<String, Option<String>>) -> Result<String, String> {
    let mut salida = String::new();
    let mut resto = texto;
    while let Some(pos) = resto.find('$') {
        salida.push_str(&resto[..pos]);
        let tras = &resto[pos + 1..];
        if let Some(cuerpo) = tras.strip_prefix('{') {
            let fin = cuerpo
                .find('}')
                .ok_or_else(|| format!("llave sin cerrar en {texto}"))?;
            salida.push_str(&valor_de_expresion(&cuerpo[..fin], globales)?);
            resto = &cuerpo[fin + 1..];
        } else {
            let nombre: String = tras
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if nombre.is_empty() {
                salida.push('$');
                resto = tras;
                continue;
            }
            salida.push_str(&valor_de_nombre(&nombre, globales)?);
            resto = &tras[nombre.len()..];
        }
    }
    salida.push_str(resto);
    if salida.is_empty() {
        return Err("la imagen quedó vacía".into());
    }
    Ok(salida)
}

/// Valor de lo que va entre llaves: `NOMBRE` o `NOMBRE:-defecto`. Otras formas no se soportan.
fn valor_de_expresion(
    expresion: &str,
    globales: &BTreeMap<String, Option<String>>,
) -> Result<String, String> {
    match expresion.split_once(":-") {
        Some((nombre, defecto)) => {
            Ok(valor_de(nombre, globales).unwrap_or_else(|| defecto.to_string()))
        }
        None if expresion.contains(':') => Err(format!("expansión no soportada: ${{{expresion}}}")),
        None => valor_de_nombre(expresion, globales),
    }
}

/// Valor de un `ARG` global. Sin valor, o vacío, es `None`.
fn valor_de(nombre: &str, globales: &BTreeMap<String, Option<String>>) -> Option<String> {
    globales
        .get(nombre)
        .cloned()
        .flatten()
        .filter(|valor| !valor.is_empty())
}

fn valor_de_nombre(
    nombre: &str,
    globales: &BTreeMap<String, Option<String>>,
) -> Result<String, String> {
    valor_de(nombre, globales).ok_or_else(|| format!("variable sin valor: {nombre}"))
}

/// Verdadero si hay que correr `podman pull`. Criterio de la Tarea 7i:
/// - Falta en el almacén local: se baja.
/// - Está local y el `FROM` no fija digest: se acepta la local, sin red. Un tag móvil como `:latest`
///   no se verifica, así que no se intenta ver si hay una versión nueva.
/// - Está local y el `FROM` fija un digest (`@sha256:`): se baja solo si el digest local no es ese.
pub fn necesita_pull(
    existe_local: bool,
    digest_fijado: Option<&str>,
    digest_local: Option<&str>,
) -> bool {
    if !existe_local {
        return true;
    }
    digest_fijado.is_some_and(|fijado| digest_local != Some(fijado))
}

/// Clasifica la salida de un `podman pull` fallido (Tarea 7j). Un pedido explícito de autenticación va
/// primero. Un texto ambiguo de Docker Hub es "no existe o es privada". Lo demás es infraestructura.
pub fn clasificar_error_pull(salida: &str) -> Problema {
    let minuscula = salida.to_lowercase();
    if PATRONES_PIDE_CREDENCIALES
        .iter()
        .any(|p| minuscula.contains(p))
    {
        return Problema::PideCredenciales;
    }
    if PATRONES_IMAGEN_INACCESIBLE
        .iter()
        .any(|p| minuscula.contains(p))
    {
        return Problema::ImagenInaccesible;
    }
    Problema::Infraestructura(linea_util(salida))
}

/// Verdadero si el repo trae al menos un Dockerfile o Containerfile.
fn hay_dockerfile(repo: &Path) -> bool {
    !dockerfiles_del_repo(repo).is_empty()
}

/// Resultado de mirar una imagen base: local o bajada, o el motivo por el que el registro no la dio.
enum Bajada {
    Lista {
        digest: Option<String>,
        descargada: bool,
    },
    Problema(Problema),
}

/// Corre `podman pull` de la imagen. Un fallo de red o de registro caído es `Problema::Infraestructura`.
fn bajar_imagen(imagen: &str) -> Result<Bajada, ErrorOraculo> {
    let pull = match correr_podman(&["pull", imagen], PLAZO_PULL) {
        Ok(pull) => pull,
        Err(ErrorOraculo::Infra(motivo)) => {
            return Ok(Bajada::Problema(Problema::Infraestructura(format!(
                "no se pudo bajar {imagen}: {motivo}"
            ))));
        }
        Err(otro) => return Err(otro),
    };
    if pull.codigo == 0 {
        return Ok(Bajada::Lista {
            digest: digest_de(imagen)?,
            descargada: true,
        });
    }
    let texto = format!("{}\n{}", pull.stdout, pull.stderr);
    Ok(Bajada::Problema(match clasificar_error_pull(&texto) {
        Problema::Infraestructura(linea) => {
            Problema::Infraestructura(format!("no se pudo bajar {imagen}: {linea}"))
        }
        otro => otro,
    }))
}

/// Mira una imagen base: la local si basta, o la baja. El criterio está en `necesita_pull`.
fn traer(imagen: &str) -> Result<Bajada, ErrorOraculo> {
    let (nombre, fijado) = match imagen.split_once('@') {
        Some((nombre, digest)) => (nombre, Some(digest)),
        None => (imagen, None),
    };
    let existe = correr_podman(&["image", "exists", nombre], PLAZO_CORTO)?.codigo == 0;
    let digest_local = if existe { digest_de(nombre)? } else { None };
    if !necesita_pull(existe, fijado, digest_local.as_deref()) {
        return Ok(Bajada::Lista {
            digest: digest_local,
            descargada: false,
        });
    }
    bajar_imagen(imagen)
}

/// Digest de una imagen ya bajada. `None` si podman no lo informa.
fn digest_de(imagen: &str) -> Result<Option<String>, ErrorOraculo> {
    let salida = correr_podman(
        &["image", "inspect", "--format", "{{.Digest}}", imagen],
        PLAZO_CORTO,
    )?;
    if salida.codigo != 0 {
        return Ok(None);
    }
    let digest = salida.stdout.trim();
    Ok((!digest.is_empty()).then(|| digest.to_string()))
}

/// El Dockerfile que se construye, como lista de una ruta. Sin elección posible, la lista está vacía.
fn elegido(repo: &Path, tarea: &Tarea) -> Vec<String> {
    match elegir_dockerfile(repo, tarea.containerfile.as_deref()) {
        Eleccion::Elegido(ruta) => vec![ruta],
        Eleccion::NingunoElegido(_) | Eleccion::SinDockerfile => Vec::new(),
    }
}

/// Baja con `podman pull` la imagen base de cada `FROM` de todos los Dockerfile del repo.
pub fn traer_imagenes_base(repo: &Path) -> Result<Vec<ImagenBase>, ErrorOraculo> {
    traer_imagenes_base_de(repo, &dockerfiles_del_repo(repo))
}

/// Baja con `podman pull` la imagen base de cada `FROM` de los Dockerfile `rutas` (relativas al repo).
/// Solo baja: no corre ningún `RUN`. Ignora `scratch` y los alias de etapas previas del mismo archivo.
pub fn traer_imagenes_base_de(
    repo: &Path,
    rutas: &[String],
) -> Result<Vec<ImagenBase>, ErrorOraculo> {
    let mut salida = Vec::new();
    for ruta in rutas {
        let texto =
            fs::read_to_string(repo.join(ruta)).map_err(|e| infra("leer el Dockerfile", e))?;
        let mut alias_previos: Vec<String> = Vec::new();
        for f in froms_del_texto(&texto) {
            let FromEnTexto { from, alias, cita } = f;
            let nombre = from
                .resuelto
                .as_ref()
                .map_or(from.escrito.as_str(), String::as_str);
            let es_etapa = nombre.eq_ignore_ascii_case("scratch")
                || alias_previos.iter().any(|a| a.eq_ignore_ascii_case(nombre));
            alias_previos.extend(alias.map(|a| a.to_lowercase()));
            if es_etapa {
                continue;
            }
            let mut base = ImagenBase {
                archivo: ruta.clone(),
                linea: from.linea,
                cita,
                imagen: from.escrito.clone(),
                resuelta: None,
                digest: None,
                descargada: false,
                sin_resolver: None,
                problema: None,
            };
            match &from.resuelto {
                Err(motivo) => base.sin_resolver = Some(motivo.clone()),
                Ok(imagen) => {
                    base.resuelta = Some(imagen.clone());
                    match traer(imagen)? {
                        Bajada::Lista { digest, descargada } => {
                            base.digest = digest;
                            base.descargada = descargada;
                        }
                        Bajada::Problema(problema) => base.problema = Some(problema),
                    }
                }
            }
            salida.push(base);
        }
    }
    Ok(salida)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RegistroConfig;
    use crate::oracle::contenedor::{self, Ejecucion};
    use inventario::RegistroNombrado;

    const LOCK: &str = "# This file is automatically @generated by Cargo.\n\
version = 4\n\
\n\
[[package]]\n\
name = \"app\"\n\
version = \"0.1.0\"\n\
\n\
[[package]]\n\
name = \"humantime\"\n\
version = \"2.1.0\"\n\
source = \"registry+https://github.com/rust-lang/crates.io-index\"\n\
checksum = \"abc\"\n\
\n\
[[package]]\n\
name = \"ext\"\n\
version = \"1.0.0\"\n\
source = \"git+https://github.com/x/ext?rev=1#1\"\n";

    fn ejecucion() -> Ejecucion {
        Ejecucion {
            imagen: "localhost/root-oraculo-rust:1".into(),
            repo: Some(PathBuf::from("/x/repo")),
            oraculo: None,
            salida: PathBuf::from("/x/salida"),
            cmd: "cargo vendor".into(),
            env: vec![],
            timeout: Duration::from_secs(60),
            red: false,
            cache: None,
            cache_rw: false,
            rustsec: None,
            secretos: Vec::new(),
        }
    }

    fn montajes(args: &[String]) -> Vec<String> {
        args.windows(2)
            .filter(|p| p[0] == "-v")
            .map(|p| p[1].clone())
            .collect()
    }

    #[test]
    fn descarga_hash_corto_es_estable_y_de_16_hex() {
        let h = hash_corto(b"");
        assert_eq!(h, "e3b0c44298fc1c14");
        assert_eq!(hash_corto(LOCK.as_bytes()), hash_corto(LOCK.as_bytes()));
        assert_ne!(hash_corto(b"a"), hash_corto(b"b"));
    }

    #[test]
    fn descarga_lock_acepta_workspace_y_crates_io_registry_y_sparse() {
        let texto = "version = 4\n\n[[package]]\nname = \"app\"\nversion = \"0.1.0\"\n\n\
[[package]]\nname = \"a\"\nversion = \"1\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n\n\
[[package]]\nname = \"b\"\nversion = \"2\"\nsource = \"sparse+https://index.crates.io/\"\n";
        let paquetes = analizar_lock(texto).expect("parsea");
        let origenes: Vec<&Origen> = paquetes.iter().map(|p| &p.origen).collect();
        assert_eq!(
            origenes,
            vec![&Origen::Workspace, &Origen::CratesIo, &Origen::CratesIo]
        );
    }

    #[test]
    fn descarga_lock_da_origen_git_con_commit_y_registro_con_su_linea() {
        let paquetes = analizar_lock(LOCK).expect("parsea");
        let ext = paquetes.iter().find(|p| p.nombre == "ext").expect("ext");
        assert_eq!(
            ext.origen,
            Origen::Git {
                url: "https://github.com/x/ext".into(),
                commit: Some("1".into()),
            }
        );
        assert_eq!(ext.linea, 17, "la línea del source de ext");
        assert_eq!(
            ext.cita,
            "source = \"git+https://github.com/x/ext?rev=1#1\""
        );

        let privado = "version = 4\n[[package]]\nname = \"p\"\nversion = \"1\"\n\
source = \"registry+https://registro.ejemplo.com/index\"\n";
        let p = analizar_lock(privado).expect("parsea");
        assert_eq!(
            p[0].origen,
            Origen::Registro {
                url: "https://registro.ejemplo.com/index".into()
            }
        );
        assert_eq!(p[0].linea, 5);
    }

    #[test]
    fn descarga_lock_sin_source_da_linea_cero() {
        let paquetes = analizar_lock(LOCK).expect("parsea");
        assert_eq!(paquetes[0].linea, 0);
        assert_eq!(paquetes[0].origen, Origen::Workspace);
    }

    #[test]
    fn descarga_lock_invalido_da_error() {
        assert!(analizar_lock("esto no es toml = = =").is_err());
    }

    #[test]
    fn descarga_hallazgos_de_rechazo_son_entrega_severidad_3_con_cita_e_id_estable() {
        let rechazo = Rechazo {
            motivo: "protocolo no aceptado: ssh; solo se acepta https".into(),
            archivo: "Cargo.lock".into(),
            linea: 17,
            cita: "source = \"git+ssh://***@github.com/x/ext\"".into(),
        };
        let hallazgos = hallazgos_de_rechazos(std::slice::from_ref(&rechazo));
        assert_eq!(hallazgos.len(), 1);
        let h = &hallazgos[0];
        assert_eq!(h.dimension, Dimension::Entrega);
        assert_eq!(h.severidad, 3);
        assert_eq!(h.archivo, "Cargo.lock");
        assert_eq!(h.linea, 17);
        assert_eq!(h.cita, rechazo.cita);
        assert!(h.detalle.contains("no aceptada"), "{}", h.detalle);
        assert!(h.detalle.contains("ssh"), "{}", h.detalle);
        assert_eq!(h.fuente, Fuente::Sonda("descarga".into()));
        let esperado = hash_corto(format!("entrega|Cargo.lock|{}", rechazo.cita).as_bytes());
        assert_eq!(h.id, esperado);
    }

    #[test]
    fn descarga_clasifica_fallas_de_red_como_infraestructura() {
        for stderr in [
            "error: failed to download `humantime v2.1.0`",
            "Could not resolve host: index.crates.io",
            "connect: Network is unreachable",
            "spurious network error (3 tries remaining)",
            "operation timed out",
        ] {
            assert!(
                matches!(diagnosticar(stderr), Diagnostico::Infra(_)),
                "debe ser infra: {stderr}"
            );
        }
    }

    #[test]
    fn descarga_clasifica_paquete_inexistente_o_retirado_como_producto() {
        let inexistente = "error: no matching package named `root-no-existe-xyz-987` found\n\
location searched: crates.io index";
        let d = diagnosticar(inexistente);
        assert_eq!(
            d,
            Diagnostico::Producto(
                "error: no matching package named `root-no-existe-xyz-987` found".into()
            )
        );
        assert!(matches!(
            diagnosticar("error: failed to select a version for the requirement `x = \"=1.0.0\"`"),
            Diagnostico::Producto(_)
        ));
        assert!(matches!(
            diagnosticar("error: version 1.0.0 of x is yanked"),
            Diagnostico::Producto(_)
        ));
    }

    #[test]
    fn descarga_infraestructura_gana_a_producto_en_el_mismo_stderr() {
        let mezclado =
            "error: no matching package named `x` found\nCould not resolve host: index.crates.io";
        assert!(matches!(diagnosticar(mezclado), Diagnostico::Infra(_)));
    }

    #[test]
    fn descarga_clasifica_error_sin_patron_como_desconocido() {
        assert!(matches!(
            diagnosticar("error: algo raro"),
            Diagnostico::Desconocido(_)
        ));
    }

    #[test]
    fn descarga_comando_pip_usa_pypi_y_el_destino_cache() {
        let c = comando_pip("/repo/requirements.txt");
        assert!(c.contains("pip download -r /repo/requirements.txt"), "{c}");
        assert!(c.contains("--index-url https://pypi.org/simple"), "{c}");
        assert!(c.ends_with("-d /cache"), "{c}");
    }

    #[test]
    fn descarga_accion_rustsec_clona_si_no_existe_y_actualiza_si_es_vieja() {
        let dia = Duration::from_secs(24 * 3600);
        assert_eq!(accion_rustsec(false, None), Accion::Clonar);
        assert_eq!(
            accion_rustsec(true, Some(Duration::from_secs(3600))),
            Accion::Nada
        );
        assert_eq!(
            accion_rustsec(true, Some(dia + Duration::from_secs(1))),
            Accion::Actualizar
        );
        assert_eq!(accion_rustsec(true, None), Accion::Actualizar);
    }

    #[test]
    fn descarga_limpiar_parcial_borra_la_carpeta_y_tolera_que_no_exista() {
        let home = tempfile::tempdir().expect("temp dir");
        let parcial = home.path().join("abc.parcial");
        std::fs::create_dir_all(parcial.join("vendor")).expect("dir");
        std::fs::write(parcial.join("config.toml"), "x").expect("escribe");
        limpiar_parcial(home.path(), &parcial).expect("limpia");
        assert!(!parcial.exists());
        limpiar_parcial(home.path(), &parcial).expect("sin carpeta no es error");
    }

    #[test]
    fn descarga_limpiar_parcial_rechaza_ruta_fuera_de_home_sin_borrar() {
        let base = tempfile::tempdir().expect("temp dir");
        let home = base.path().join("home");
        let fuera = base.path().join("fuera");
        std::fs::create_dir_all(&home).expect("home");
        std::fs::create_dir_all(&fuera).expect("fuera");
        std::fs::write(fuera.join("dato"), "x").expect("escribe");
        assert!(limpiar_parcial(&home, &fuera).is_err());
        assert!(fuera.join("dato").exists());
    }

    #[test]
    fn descarga_borrar_en_home_rechaza_ruta_fuera_de_home_sin_borrar() {
        let base = tempfile::tempdir().expect("temp dir");
        let home = base.path().join("home");
        let rustsec_fuera = base.path().join("rustsec");
        std::fs::create_dir_all(&home).expect("home");
        std::fs::create_dir_all(&rustsec_fuera).expect("dir");
        std::fs::write(rustsec_fuera.join("dato"), "x").expect("escribe");
        assert!(borrar_en_home(&home, &rustsec_fuera).is_err());
        assert!(rustsec_fuera.join("dato").exists());
    }

    #[test]
    fn descarga_argumentos_sin_red_mantienen_network_none() {
        let args = contenedor::argumentos(&ejecucion(), "n");
        let pos = args.iter().position(|a| a == "--network").expect("network");
        assert_eq!(args[pos + 1], "none");
    }

    #[test]
    fn descarga_argumentos_con_red_no_cortan_la_red() {
        let mut e = ejecucion();
        e.red = true;
        let args = contenedor::argumentos(&e, "n");
        assert!(!args.iter().any(|a| a == "--network"), "{args:?}");
    }

    #[test]
    fn descarga_argumentos_cache_ro_y_rw_con_selinux() {
        let mut e = ejecucion();
        e.cache = Some(PathBuf::from("/x/cache"));
        let ro = montajes(&contenedor::argumentos(&e, "n"));
        assert!(ro.contains(&"/x/cache:/cache:ro,z".to_string()), "{ro:?}");

        e.cache_rw = true;
        let rw = montajes(&contenedor::argumentos(&e, "n"));
        assert!(rw.contains(&"/x/cache:/cache:z".to_string()), "{rw:?}");
    }

    #[test]
    fn descarga_argumentos_rustsec_se_monta_solo_lectura() {
        let mut e = ejecucion();
        e.rustsec = Some(PathBuf::from("/x/rustsec"));
        let m = montajes(&contenedor::argumentos(&e, "n"));
        assert!(m.contains(&"/x/rustsec:/rustsec:ro,z".to_string()), "{m:?}");
    }

    #[test]
    fn descarga_argumentos_sin_oraculo_no_montan_oraculo() {
        let mut e = ejecucion();
        e.red = true;
        e.cache = Some(PathBuf::from("/x/cache"));
        e.cache_rw = true;
        let args = contenedor::argumentos(&e, "n");
        assert!(
            !montajes(&args).iter().any(|m| m.contains(":/oraculo")),
            "no monta /oraculo: {args:?}"
        );
    }

    #[test]
    fn descarga_argumentos_todos_los_montajes_llevan_z() {
        let mut e = ejecucion();
        e.oraculo = Some(PathBuf::from("/x/oraculo"));
        e.cache = Some(PathBuf::from("/x/cache"));
        e.rustsec = Some(PathBuf::from("/x/rustsec"));
        for m in montajes(&contenedor::argumentos(&e, "n")) {
            assert!(m.ends_with(",z") || m.ends_with(":z"), "sin z: {m}");
        }
    }

    #[test]
    fn descarga_preludio_arma_cargo_home_y_copia_la_config() {
        assert!(PRELUDIO.contains("export CARGO_HOME=/tmp/cargo-home"));
        assert!(PRELUDIO.contains("cp /cache/config.toml \"$CARGO_HOME/config.toml\""));
        assert!(PRELUDIO.ends_with("&& "));
    }

    #[test]
    fn descarga_causa_de_pull_separa_inexistente_de_credenciales_y_red() {
        let inexistente = "Error: reading manifest 1 in docker.io/library/x: manifest unknown";
        assert_eq!(
            clasificar_error_pull(inexistente),
            Problema::ImagenInaccesible
        );
        let denegado = "Error: reading manifest 1 in docker.io/library/x: requested access to the resource is denied";
        assert_eq!(clasificar_error_pull(denegado), Problema::ImagenInaccesible);
        assert_eq!(
            clasificar_error_pull("Error: unauthorized: authentication required"),
            Problema::PideCredenciales
        );
        let red = "Error: dial tcp: lookup registro.ejemplo: no such host";
        assert!(matches!(
            clasificar_error_pull(red),
            Problema::Infraestructura(_)
        ));
    }

    fn resuelto(texto: &str, i: usize) -> Result<String, String> {
        resolver_froms(texto)[i].resuelto.clone()
    }

    #[test]
    fn descarga_resolver_froms_arg_antes_del_from_sustituye_llaves_y_dolar() {
        let texto = "ARG BASE=debian:12\nFROM ${BASE}\n";
        assert_eq!(resuelto(texto, 0), Ok("debian:12".to_string()));
        let sin_llaves = "ARG BASE=debian:12\nFROM $BASE AS x\n";
        assert_eq!(resuelto(sin_llaves, 0), Ok("debian:12".to_string()));
        let fijo = resolver_froms("FROM docker.io/library/alpine:3.20\n");
        assert_eq!(
            fijo,
            [FromResuelto {
                linea: 1,
                escrito: "docker.io/library/alpine:3.20".into(),
                resuelto: Ok("docker.io/library/alpine:3.20".into()),
            }]
        );
    }

    #[test]
    fn descarga_resolver_froms_arg_sin_valor_y_variable_ausente_quedan_sin_resolver() {
        let sin_valor = resolver_froms("ARG SIN_VALOR\nFROM ${SIN_VALOR}\n");
        assert_eq!(sin_valor[0].linea, 2);
        assert_eq!(sin_valor[0].escrito, "${SIN_VALOR}");
        assert!(sin_valor[0].resuelto.is_err(), "{sin_valor:?}");
        assert!(resuelto("FROM ${NO_DECLARADA}\n", 0).is_err());
    }

    #[test]
    fn descarga_resolver_froms_default_con_dos_puntos_guion_usa_el_defecto() {
        let texto = "ARG X\nFROM ${X:-alpine:3.20}\n";
        assert_eq!(resuelto(texto, 0), Ok("alpine:3.20".to_string()));
        let con_valor = "ARG X=debian:12\nFROM ${X:-alpine:3.20}\n";
        assert_eq!(resuelto(con_valor, 0), Ok("debian:12".to_string()));
    }

    #[test]
    fn descarga_resolver_froms_arg_en_etapa_no_cambia_el_from_anterior() {
        // Un ARG declarado después del primer FROM es de su etapa: no entra en FROM posteriores.
        let texto = "ARG BASE=alpine:3.20\nFROM ${BASE}\nARG BASE=debian:12\nFROM ${BASE}\n";
        let froms = resolver_froms(texto);
        assert_eq!(froms.len(), 2, "{froms:?}");
        assert_eq!(froms[0].resuelto, Ok("alpine:3.20".to_string()));
        assert_eq!(froms[1].resuelto, Ok("alpine:3.20".to_string()));
    }

    #[test]
    fn descarga_necesita_pull_solo_si_falta_o_el_digest_fijado_no_coincide() {
        // Falta en el almacén: hay que bajarla.
        assert!(necesita_pull(false, None, None));
        // Local y sin digest fijado (tag móvil como `:latest`): se acepta la local, sin red.
        assert!(!necesita_pull(true, None, Some("sha256:a")));
        // Local con el digest fijado igual: no se baja.
        assert!(!necesita_pull(true, Some("sha256:a"), Some("sha256:a")));
        // Local con un digest fijado distinto, o sin digest local que comparar: se baja.
        assert!(necesita_pull(true, Some("sha256:a"), Some("sha256:b")));
        assert!(necesita_pull(true, Some("sha256:a"), None));
    }

    #[test]
    fn descarga_problema_infraestructura_se_escribe_como_texto_con_prefijo() {
        let json =
            serde_json::to_value(Problema::Infraestructura("dial tcp".into())).expect("serializa");
        assert_eq!(json, serde_json::json!("infraestructura: dial tcp"));
        assert_eq!(
            serde_json::to_value(Problema::ImagenInaccesible).expect("serializa"),
            serde_json::json!("imagen_inaccesible")
        );
    }

    #[test]
    fn descarga_clasifica_error_pull_pide_credenciales_con_auth_explicita() {
        for salida in [
            "Error: unauthorized: authentication required",
            "Error: initializing source: 401 Unauthorized",
            "Error: no basic auth credentials",
            "Error: basic auth credentials missing",
        ] {
            assert_eq!(
                clasificar_error_pull(salida),
                Problema::PideCredenciales,
                "{salida}"
            );
        }
    }

    #[test]
    fn descarga_clasifica_error_pull_auth_explicita_gana_a_no_existe() {
        let mezclado = "Error: requested access to the resource is denied\n\
            unauthorized: authentication required";
        assert_eq!(clasificar_error_pull(mezclado), Problema::PideCredenciales);
    }

    #[test]
    fn descarga_clasifica_error_pull_ambiguo_es_imagen_inaccesible() {
        for salida in [
            "Error: requested access to the resource is denied",
            "Error: pull access denied for x, repository does not exist or may require 'docker login'",
            "Error: manifest unknown: manifest unknown",
            "Error: name unknown: repository x not found",
        ] {
            assert_eq!(
                clasificar_error_pull(salida),
                Problema::ImagenInaccesible,
                "{salida}"
            );
        }
    }

    #[test]
    fn descarga_clasifica_error_pull_red_es_infraestructura() {
        for salida in [
            "Error: dial tcp: lookup registro.ejemplo: no such host",
            "Error: dial tcp 127.0.0.1:1: connect: connection refused",
        ] {
            assert!(
                matches!(clasificar_error_pull(salida), Problema::Infraestructura(_)),
                "{salida}"
            );
        }
    }

    #[test]
    fn descarga_unauthorized_sin_authentication_required_es_imagen_inaccesible() {
        assert_eq!(
            clasificar_error_pull(
                "Error: initializing source docker://x/y:1: unauthorized: access to the requested resource is not authorized"
            ),
            Problema::ImagenInaccesible
        );
        assert_eq!(
            clasificar_error_pull("Error: unauthorized: authentication required"),
            Problema::PideCredenciales
        );
    }

    #[test]
    fn descarga_clasifica_error_pull_401_de_podman_pide_credenciales() {
        let salida = "Error: unable to copy from source docker://127.0.0.1:5099/x:1: initializing source docker://127.0.0.1:5099/x:1: reading manifest 1 in 127.0.0.1:5099/x: authentication required";
        assert_eq!(clasificar_error_pull(salida), Problema::PideCredenciales);
    }

    #[test]
    fn descarga_comando_vendor_corre_desde_el_repo_para_leer_su_config() {
        assert!(
            COMANDO_VENDOR.starts_with("cd /repo && cargo vendor --locked"),
            "{COMANDO_VENDOR}"
        );
        assert!(
            COMANDO_VENDOR.ends_with("--versioned-dirs /cache/vendor > /cache/config.toml"),
            "{COMANDO_VENDOR}"
        );
    }

    #[test]
    fn descarga_comando_lock_genera_en_la_copia_del_repo_con_su_config() {
        assert!(
            COMANDO_LOCK.contains("cp -r /repo/. /tmp/w/"),
            "{COMANDO_LOCK}"
        );
        assert!(
            COMANDO_LOCK.contains("cd /tmp/w && cargo generate-lockfile"),
            "{COMANDO_LOCK}"
        );
        assert!(
            COMANDO_LOCK.ends_with("cp /tmp/w/Cargo.lock /salida/Cargo.lock"),
            "{COMANDO_LOCK}"
        );
    }

    /// Credencial de prueba: `registros_nombrados` y `[[registro]]` con host igual, valor `valor-secreto-9`.
    fn credencial_de_prueba() -> Vec<Credencial> {
        let inv = Inventario {
            registros_nombrados: vec![RegistroNombrado {
                nombre: "priv".into(),
                direccion: "https://reg.example/index/".into(),
            }],
            ..Inventario::default()
        };
        let registros = vec![RegistroConfig {
            nombre: "priv".into(),
            url: "https://reg.example/index/".into(),
            credencial_env: "ROOT_TOKEN_PRIV".into(),
        }];
        credenciales_cargo(&inv, &registros, |_| Some("valor-secreto-9".into()))
    }

    #[test]
    fn descarga_redactar_diagnostico_quita_el_secreto_en_todas_sus_formas() {
        let creds = credencial_de_prueba();
        assert_eq!(creds.len(), 1, "{creds:?}");
        for diagnostico in [
            Diagnostico::Infra("401 con valor-secreto-9".into()),
            Diagnostico::Producto("paquete valor-secreto-9".into()),
            Diagnostico::Desconocido("algo valor-secreto-9".into()),
        ] {
            let texto = format!("{:?}", redactar_diagnostico(diagnostico, &creds));
            assert!(!texto.contains("valor-secreto-9"), "{texto}");
            assert!(texto.contains("***"), "{texto}");
        }
    }

    /// Tarea de Rust mínima, cargada desde un `tarea.toml` temporal.
    fn tarea_rust_minima() -> Tarea {
        let dir = tempfile::tempdir().expect("temp dir");
        let texto = "nombre = \"x\"\nlenguaje = \"rust\"\nimagen = \"i\"\nspec = \"s\"\n\
                     [contrato]\ncmd = \"c\"\n[oculto]\ncmd = \"o\"\n";
        fs::write(dir.path().join("tarea.toml"), texto).expect("escribe tarea.toml");
        Tarea::cargar_de(dir.path()).expect("carga la tarea")
    }

    #[test]
    fn descarga_gate_del_inventario_lista_solo_las_entradas_de_red_del_config() {
        let home = tempfile::tempdir().expect("home temporal");
        let tarea = tarea_rust_minima();
        let inv = Inventario {
            config_cargo: vec![
                EntradaConfig {
                    archivo: ".cargo/config.toml".into(),
                    linea: 2,
                    seccion: "http".into(),
                    clave: "http.proxy".into(),
                    red: true,
                    cita: "proxy = \"http://***@proxy:3128\"".into(),
                },
                EntradaConfig {
                    archivo: ".cargo/config.toml".into(),
                    linea: 5,
                    seccion: "alias".into(),
                    clave: "alias.b".into(),
                    red: false,
                    cita: "b = \"build\"".into(),
                },
            ],
            ..Inventario::default()
        };
        registrar_inventario(home.path(), &tarea, &inv).expect("registra el inventario");

        let texto = fs::read_to_string(home.path().join("eventos.jsonl")).expect("eventos");
        let evento: Value = texto
            .lines()
            .map(|l| serde_json::from_str::<Value>(l).expect("línea JSON"))
            .find(|v| v["fase"] == "inventario")
            .expect("Gate del inventario");
        let config = evento["extra"]["config_cargo"]
            .as_array()
            .expect("config_cargo en el Gate");
        assert_eq!(config.len(), 1, "{config:?}");
        assert_eq!(config[0]["clave"], "http.proxy");
        assert_eq!(config[0]["linea"], 2);
        assert_eq!(config[0]["archivo"], ".cargo/config.toml");
        assert_eq!(config[0]["cita"], "proxy = \"http://***@proxy:3128\"");
        assert!(
            !texto.contains("alias.b"),
            "una entrada sin red no entra al Gate: {texto}"
        );
    }
}
