//! Fase de descarga (decisiones 17 y 21): baja las dependencias con red antes de evaluar.
//! Antes de la red, `inventario` lista las fuentes del repo y aplica sus reglas. Corre en un
//! contenedor aparte. Ese contenedor nunca monta `/oraculo` y no ejecuta código del proyecto.
//! La evaluación usa la caché sin red.

pub mod inventario;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::configs::{self, TipoConfig, instrucciones, palabra_clave, parsear_from};
use super::contenedor::{self, Ejecucion, correr_podman};
use super::tarea::Tarea;
use super::{CARPETA_TEMPORALES, ErrorOraculo, infra, temporal};
use crate::observe;
use crate::tipos::{Dimension, Fuente, Hallazgo, TipoEvento};
use inventario::{Inventario, Origen, PaqueteLock, Rechazo, analizar_lock};

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
const COMANDO_VENDOR: &str =
    "cd /repo && cargo vendor --locked --versioned-dirs /cache/vendor > /cache/config.toml";
const COMANDO_LOCK: &str = "mkdir -p /tmp/w && cp -r /repo/. /tmp/w/ && cd /tmp/w && cargo generate-lockfile && cp Cargo.lock /salida/Cargo.lock";
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
/// Fragmentos (en minúscula) de un `pull` de una imagen que no existe. Van antes que los de credenciales.
const PATRONES_NO_EXISTE: [&str; 3] = [
    "manifest unknown",
    "not found",
    "requested access to the resource is denied",
];
/// Fragmentos (en minúscula) de un registro que pide credenciales. No se pasan (zona roja, P21).
const PATRONES_CREDENCIALES: [&str; 2] = ["denied", "unauthorized"];

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

/// Borra la carpeta parcial de una descarga anterior que no terminó.
pub fn limpiar_parcial(dir: &Path) -> io::Result<()> {
    match fs::remove_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Hallazgo de entrega. El `id` sale de la dimensión, el archivo y la cita.
fn hallazgo(archivo: &str, linea: u32, cita: &str, severidad: u8, detalle: String) -> Hallazgo {
    let id = hash_corto(format!("entrega|{archivo}|{cita}").as_bytes());
    Hallazgo {
        dimension: Dimension::Entrega,
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
    let inv = inventario::inventariar(repo)?;
    registrar_inventario(home, tarea, &inv)?;
    if !inv.rechazos.is_empty() {
        return Ok(Preparacion::SinVeredicto(hallazgos_de_rechazos(
            &inv.rechazos,
        )));
    }
    // Las imágenes base van antes de las dependencias: sin ellas el Dockerfile no construye (Tarea 9d2).
    let imagenes = if hay_dockerfile(repo) {
        Some(traer_imagenes_base(repo)?)
    } else {
        None
    };
    let hallazgos_imagen = imagenes
        .as_deref()
        .map(hallazgos_de_imagenes)
        .unwrap_or_default();
    if imagenes
        .iter()
        .flatten()
        .any(|i| i.problema == Some(Problema::Credenciales))
    {
        // Sin veredicto: no se pasan credenciales, así que no se sigue con la descarga.
        let mut registro = Registro::sin_paquetes(&tarea.lenguaje);
        registro.motivo = Some("credenciales");
        registro.imagenes = imagenes;
        registrar(home, tarea, &registro)?;
        return Ok(Preparacion::SinVeredicto(hallazgos_imagen));
    }
    let (preparacion, registro) = match tarea.lenguaje.as_str() {
        "rust" => preparar_rust(repo, tarea, home, timeout, &inv)?,
        "python" => preparar_python(repo, tarea, home, timeout)?,
        _ => (
            Preparacion::Lista {
                cache: None,
                rustsec: None,
                hallazgos: Vec::new(),
            },
            None,
        ),
    };
    let preparacion = con_hallazgos(preparacion, hallazgos_imagen);
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
    Ok(preparacion)
}

/// Agrega hallazgos a una preparación, sea cual sea su variante.
fn con_hallazgos(preparacion: Preparacion, extra: Vec<Hallazgo>) -> Preparacion {
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

/// Gate de la fase `inventario`: las fuentes que el repo declara, antes de usar la red.
fn registrar_inventario(home: &Path, tarea: &Tarea, inv: &Inventario) -> Result<(), ErrorOraculo> {
    let mut extra = serde_json::Map::new();
    extra.insert("lenguaje".into(), json!(tarea.lenguaje));
    extra.insert("fuentes".into(), json!(inv.fuentes));
    extra.insert("rechazos".into(), json!(inv.rechazos));
    extra.insert("avisos".into(), json!(inv.avisos));
    let evento = observe::evento_gate(&format!("descarga-{}", tarea.nombre), "inventario", extra);
    observe::registrar_en(&home.join(ARCHIVO_EVENTOS), &evento)
        .map_err(|e| infra("registrar el inventario de fuentes", e))
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
) -> Result<Paso, ErrorOraculo> {
    let mut hallazgos = Vec::new();
    let lock = repo.join(ARCHIVO_LOCK);
    if !lock.is_file() {
        if let Some(linea) = generar_lock(repo, tarea, home, timeout)? {
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
    if !cache_hit && let Some(linea) = descargar_vendor(repo, tarea, home, &hash, timeout)? {
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

fn preparar_python(
    repo: &Path,
    tarea: &Tarea,
    home: &Path,
    timeout: Duration,
) -> Result<Paso, ErrorOraculo> {
    let sin_nada = Preparacion::Lista {
        cache: None,
        rustsec: None,
        hallazgos: Vec::new(),
    };
    // TODO 6c-B: wheels, sdist con curl, y fuentes git o URL de Python. Hoy solo corre `pip download`.
    let requirements = repo.join(ARCHIVO_REQUIREMENTS);
    if !requirements.is_file() {
        return Ok((sin_nada, None));
    }
    let contenido = fs::read(&requirements).map_err(|e| infra("leer requirements.txt", e))?;
    let hash = hash_corto(&contenido);
    let destino = home.join(CARPETA_CACHE).join(CARPETA_PYTHON).join(&hash);
    fs::create_dir_all(&destino).map_err(|e| infra("crear la caché de Python", e))?;
    let salida = temporal_de(home)?;
    let ejecucion = Ejecucion {
        imagen: tarea.imagen.clone(),
        repo: Some(repo.to_path_buf()),
        oraculo: None,
        salida: salida.path().to_path_buf(),
        cmd: comando_pip("/repo/requirements.txt"),
        env: Vec::new(),
        timeout,
        red: true,
        cache: Some(destino),
        cache_rw: true,
        rustsec: None,
    };
    match ejecutar_descarga(&ejecucion) {
        Ok(()) => {
            let registro = Registro {
                lenguaje: "python".into(),
                hash: Some(hash),
                cache_hit: false,
                paquetes: Vec::new(),
                rustsec: None,
                motivo: None,
                imagenes: None,
            };
            Ok((sin_nada, Some(registro)))
        }
        Err(Diagnostico::Producto(linea)) => Ok(corte(
            hallazgo_no_instala(&linea),
            Registro::corte("python", Some(hash), "no-instala"),
        )),
        Err(diagnostico) => Err(error_de(diagnostico)),
    }
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

/// Corre un contenedor de descarga. Un fallo llega como `Diagnostico`.
fn ejecutar_descarga(ejecucion: &Ejecucion) -> Result<(), Diagnostico> {
    match contenedor::ejecutar(ejecucion) {
        Err(ErrorOraculo::Infra(m)) => Err(Diagnostico::Infra(m)),
        Err(otro) => Err(Diagnostico::Desconocido(otro.to_string())),
        Ok(resultado) if resultado.codigo == 0 => Ok(()),
        Ok(resultado) => Err(diagnosticar(&resultado.stderr)),
    }
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
/// Da `Some(linea)` si el producto no instala.
fn generar_lock(
    repo: &Path,
    tarea: &Tarea,
    home: &Path,
    timeout: Duration,
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
    };
    match ejecutar_descarga(&ejecucion) {
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
fn descargar_vendor(
    repo: &Path,
    tarea: &Tarea,
    home: &Path,
    hash: &str,
    timeout: Duration,
) -> Result<Option<String>, ErrorOraculo> {
    let destino = home.join(CARPETA_CACHE).join(CARPETA_RUST).join(hash);
    let parcial = parcial_de(&destino);
    limpiar_parcial(&parcial).map_err(|e| infra("borrar la descarga parcial", e))?;
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
    };
    match ejecutar_descarga(&ejecucion) {
        Ok(()) if parcial.join(ARCHIVO_CONFIG).is_file() => {
            if destino.exists() {
                fs::remove_dir_all(&destino).map_err(|e| infra("borrar la caché previa", e))?;
            }
            fs::rename(&parcial, &destino).map_err(|e| infra("cerrar la caché", e))?;
            Ok(None)
        }
        Ok(()) => {
            limpiar_parcial(&parcial).map_err(|e| infra("borrar la descarga parcial", e))?;
            Err(ErrorOraculo::Infra(
                "infraestructura: cargo vendor no dejó config.toml".into(),
            ))
        }
        Err(diagnostico) => {
            limpiar_parcial(&parcial).map_err(|e| infra("borrar la descarga parcial", e))?;
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
    };
    match ejecutar_descarga(&ejecucion) {
        Ok(()) => Ok((Some(dir), "ok".into())),
        Err(diagnostico) => {
            let motivo = match diagnostico {
                Diagnostico::Infra(m) | Diagnostico::Producto(m) | Diagnostico::Desconocido(m) => m,
            };
            if accion == Accion::Clonar {
                let _ = fs::remove_dir_all(&dir);
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

/// Imagen base de un `FROM` de un Dockerfile del repo (Tarea 9d2). Sin `digest` no hubo descarga.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ImagenBase {
    pub archivo: String,
    pub linea: u32,
    pub cita: String,
    pub imagen: String,
    pub digest: Option<String>,
    /// Motivo por el que no se resolvió la imagen (una variable `$VAR` sin resolver).
    pub sin_resolver: Option<String>,
    /// Por qué el registro no entregó la imagen.
    pub problema: Option<Problema>,
}

/// Motivo por el que una imagen base no se bajó.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Problema {
    NoExiste,
    Credenciales,
}

/// Causa de un `podman pull` fallido.
#[derive(Debug, Clone, PartialEq)]
enum Causa {
    NoExiste,
    Credenciales,
    Otra(String),
}

/// Clasifica la salida de un `podman pull` fallido. Lo que no es inexistente ni credenciales es red.
fn causa_de_pull(salida: &str) -> Causa {
    let minuscula = salida.to_lowercase();
    if PATRONES_NO_EXISTE.iter().any(|p| minuscula.contains(p)) {
        return Causa::NoExiste;
    }
    if PATRONES_CREDENCIALES.iter().any(|p| minuscula.contains(p)) {
        return Causa::Credenciales;
    }
    Causa::Otra(linea_util(salida))
}

/// Verdadero si el repo trae al menos un Dockerfile o Containerfile.
fn hay_dockerfile(repo: &Path) -> bool {
    configs::buscar_archivos(repo)
        .iter()
        .any(|a| a.tipo == TipoConfig::Dockerfile)
}

/// Resultado de bajar una imagen base: el digest, o el motivo por el que el registro no la dio.
enum Bajada {
    Lista(Option<String>),
    Problema(Problema),
}

/// Corre `podman pull` de la imagen. Un fallo de red o de registro caído es `Infra`.
fn bajar_imagen(imagen: &str) -> Result<Bajada, ErrorOraculo> {
    let pull = correr_podman(&["pull", imagen], PLAZO_PULL)?;
    if pull.codigo == 0 {
        return Ok(Bajada::Lista(digest_de(imagen)?));
    }
    let texto = format!("{}\n{}", pull.stdout, pull.stderr);
    match causa_de_pull(&texto) {
        Causa::NoExiste => Ok(Bajada::Problema(Problema::NoExiste)),
        Causa::Credenciales => Ok(Bajada::Problema(Problema::Credenciales)),
        Causa::Otra(linea) => Err(ErrorOraculo::Infra(format!(
            "no se pudo bajar {imagen}: {linea}"
        ))),
    }
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

/// Hallazgos de las imágenes base que el registro no dio. Credenciales también es hallazgo, con sin veredicto.
fn hallazgos_de_imagenes(imagenes: &[ImagenBase]) -> Vec<Hallazgo> {
    imagenes
        .iter()
        .filter_map(|i| {
            let detalle = match i.problema? {
                Problema::NoExiste => format!("la imagen base no existe: {}", i.imagen),
                Problema::Credenciales => format!(
                    "la imagen base pide credenciales: no se pasan (zona roja, P21): {}",
                    i.imagen
                ),
            };
            Some(hallazgo(&i.archivo, i.linea, &i.cita, 3, detalle))
        })
        .collect()
}

/// Baja con `podman pull` la imagen base de cada `FROM` de los Dockerfile del repo.
/// Solo baja: no corre ningún `RUN`. Ignora `scratch` y los alias de etapas previas del mismo archivo.
pub fn traer_imagenes_base(repo: &Path) -> Result<Vec<ImagenBase>, ErrorOraculo> {
    let mut salida = Vec::new();
    for archivo in configs::buscar_archivos(repo) {
        if archivo.tipo != TipoConfig::Dockerfile {
            continue;
        }
        let texto = fs::read_to_string(repo.join(&archivo.ruta_relativa))
            .map_err(|e| infra("leer el Dockerfile", e))?;
        let mut alias_previos: Vec<String> = Vec::new();
        for inst in instrucciones(&texto) {
            let (kw, resto) = palabra_clave(&inst.logico);
            if kw != "FROM" {
                continue;
            }
            let (imagen, alias) = parsear_from(resto);
            let es_etapa = imagen.as_deref().is_some_and(|i| {
                i.eq_ignore_ascii_case("scratch")
                    || alias_previos.iter().any(|a| a.eq_ignore_ascii_case(i))
            });
            alias_previos.extend(alias.map(|a| a.to_lowercase()));
            let Some(imagen) = imagen.filter(|_| !es_etapa) else {
                continue;
            };
            let mut base = ImagenBase {
                archivo: archivo.ruta_relativa.clone(),
                linea: inst.linea,
                cita: inst.cita.clone(),
                imagen: imagen.clone(),
                digest: None,
                sin_resolver: None,
                problema: None,
            };
            if imagen.contains('$') {
                base.sin_resolver =
                    Some("la imagen usa una variable: no se puede resolver sin los ARG".into());
            } else {
                match bajar_imagen(&imagen)? {
                    Bajada::Lista(digest) => base.digest = digest,
                    Bajada::Problema(problema) => base.problema = Some(problema),
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
    use crate::oracle::contenedor::{self, Ejecucion};

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
        limpiar_parcial(&parcial).expect("limpia");
        assert!(!parcial.exists());
        limpiar_parcial(&parcial).expect("sin carpeta no es error");
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
        assert_eq!(causa_de_pull(inexistente), Causa::NoExiste);
        let denegado = "Error: reading manifest 1 in docker.io/library/x: requested access to the resource is denied";
        assert_eq!(causa_de_pull(denegado), Causa::NoExiste);
        assert_eq!(
            causa_de_pull("Error: unauthorized: authentication required"),
            Causa::Credenciales
        );
        let red = "Error: dial tcp: lookup registro.ejemplo: no such host";
        assert!(matches!(causa_de_pull(red), Causa::Otra(_)));
    }
}
