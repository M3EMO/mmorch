//! Paquete de `corre` para Colab: `r00t colab empaquetar --corre`.
//!
//! El paquete lleva el binario, `GOAL.md`, `tools/colab/`, la config de la corrida, los plugins de
//! ECC que la config fija, los oráculos de las tareas pedidas y un manifiesto. Del ROOT_HOME solo
//! sale lo que la lista dice. Una revisión final frena claves, `.env` y `.credentials.json`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::borrado;
use crate::colab_calibra::{
    EXCLUIDOS, Imagen, PodmanReal, ROOT_PODMAN_BIN, Resumen, armar_oraculo_runtime, binario_podman,
    es_elf_x86_64, es_sha256_hex, revisar_root_toml, sha256_hex, version_de_rustc,
};
use crate::config::root_home;
use crate::corre::ErrorCorre;
use crate::corre::config::{ConfigCorrida, ruta_de};
use crate::observe::ruta_eventos_en;
use crate::oracle::ErrorOraculo;
use crate::oracle::pendientes::nombre_de_tarea_valido;
use crate::tipos::{Estado, Evento, Fila, TipoEvento};
use crate::track::{agregar_en, es_sin_veredicto};
use serde_json::Value;

/// Imagen con `claude` y `rustc` que lee el paquete.
const IMAGEN_BUILDER: &str = "root-builder:1";
/// Imagen del oráculo: de ella salen `herramientas/`, `versiones.env` y `entorno.env`.
const IMAGEN_ORACULO: &str = "localhost/root-oraculo-rust:4";
/// Nombre del binario dentro del paquete.
const BINARIO: &str = "r00t";
/// Archivo de versiones del builder, en la raíz del paquete.
const VERSIONES: &str = "versiones-builder.env";
/// Manifiesto, en la raíz del paquete. Se escribe al final.
const MANIFIESTO: &str = "manifiesto.json";
/// Carpeta del ROOT_HOME que el paquete trae, con lo que el builder lee.
const RAIZ_HOME: &str = "root-home";
/// Carpeta de trabajo, dentro de ROOT_HOME. Se borra al salir.
const CARPETA_TMP: &str = "tmp";

/// Pedido de `colab empaquetar --corre`, tal como llega de la línea de comandos.
#[derive(Debug, Clone)]
pub struct PedidoEmpaquetarCorre {
    pub tareas: Vec<String>,
    pub config: String,
    pub pasada: u32,
    pub binario: PathBuf,
    pub raiz_root: Option<PathBuf>,
    pub salida: PathBuf,
    /// SHA-256 del instalador de rustup, en hexadecimal (64 caracteres).
    pub rustup_sha256: String,
}

/// Lee las versiones de `claude` y de `rustc` dentro de la imagen del builder.
pub trait ImagenBuilder {
    /// Salida de `claude --version`, por ejemplo `2.1.5 (Claude Code)`.
    fn version_claude(&self) -> Result<String, ErrorOraculo>;
    /// Salida de `rustc --version`.
    fn version_rustc(&self) -> Result<String, ErrorOraculo>;
}

/// Imagen real: corre `claude` y `rustc` dentro de `root-builder:1` con podman.
/// Los tests no la usan.
pub struct PodmanBuilder;

impl PodmanBuilder {
    /// Corre `args` dentro de la imagen del builder y devuelve su salida, sin espacios al borde.
    fn correr(&self, args: &[&str]) -> Result<String, ErrorOraculo> {
        let binario = std::env::var_os(ROOT_PODMAN_BIN)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("podman"));
        let salida = Command::new(&binario)
            .args(["run", "--rm", IMAGEN_BUILDER])
            .args(args)
            .output()
            .map_err(|e| ErrorOraculo::Infra(format!("correr podman: {e}")))?;
        if !salida.status.success() {
            return Err(ErrorOraculo::Infra(format!(
                "{} falló en {IMAGEN_BUILDER}: {}",
                args.join(" "),
                String::from_utf8_lossy(&salida.stderr).trim()
            )));
        }
        Ok(String::from_utf8_lossy(&salida.stdout).trim().to_string())
    }
}

impl ImagenBuilder for PodmanBuilder {
    fn version_claude(&self) -> Result<String, ErrorOraculo> {
        self.correr(&["claude", "--version"])
    }
    fn version_rustc(&self) -> Result<String, ErrorOraculo> {
        self.correr(&["rustc", "--version"])
    }
}

/// Manifiesto del paquete de `corre`. Lo lee quien corre el builder en Colab.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ManifiestoCorre {
    pub v: u32,
    pub tipo: String,
    pub pasada: u32,
    pub commit_root: String,
    pub config: String,
    pub binario_sha256: String,
    /// Id de la imagen del oráculo, que el runtime pasa al r00t.
    #[serde(default)]
    pub imagen_id: String,
    /// Tareas en el orden pedido.
    pub tareas: Vec<String>,
    /// Ruta absoluta original del plugin -> ruta relativa dentro del paquete.
    pub plugins: BTreeMap<String, String>,
    /// Ruta relativa -> SHA-256 de cada archivo del paquete, sin el manifiesto.
    pub archivos: BTreeMap<String, String>,
}

/// Punto de entrada de `colab empaquetar --corre`. Usa ROOT_HOME, git de la raíz y el builder real.
pub fn empaquetar_corre_cli(pedido: PedidoEmpaquetarCorre) -> Result<Resumen, ErrorOraculo> {
    let raiz_root = match &pedido.raiz_root {
        Some(raiz) => raiz.clone(),
        None => std::env::current_dir()
            .map_err(|e| ErrorOraculo::Infra(format!("directorio actual: {e}")))?,
    };
    let commit = commit_de(&raiz_root)?;
    let oraculo = PodmanReal {
        binario: binario_podman(),
    };
    empaquetar_corre(
        &root_home(),
        &raiz_root,
        &commit,
        &pedido,
        &PodmanBuilder,
        &oraculo,
    )
}

/// Arma el paquete de `corre` con los datos dados. Los tests llaman a esta función.
pub fn empaquetar_corre(
    home: &Path,
    raiz_root: &Path,
    commit: &str,
    pedido: &PedidoEmpaquetarCorre,
    imagen: &dyn ImagenBuilder,
    oraculo: &dyn Imagen,
) -> Result<Resumen, ErrorOraculo> {
    if !es_sha256_hex(&pedido.rustup_sha256) {
        return Err(ErrorOraculo::Uso(
            "--rustup-sha256 debe tener 64 caracteres hexadecimales".into(),
        ));
    }
    validar_tareas(home, &pedido.tareas)?;
    let config = ConfigCorrida::cargar(home, &pedido.config).map_err(error_de_corre)?;
    let bytes = fs::read(&pedido.binario)
        .map_err(|e| ErrorOraculo::Infra(format!("leer {}: {e}", pedido.binario.display())))?;
    if !es_elf_x86_64(&bytes) {
        return Err(ErrorOraculo::Uso(format!(
            "{} no es un ELF x86-64",
            pedido.binario.display()
        )));
    }
    if !raiz_root.join("tools/colab").is_dir() {
        return Err(ErrorOraculo::Uso(format!(
            "falta tools/colab/ en {}",
            raiz_root.display()
        )));
    }
    if !raiz_root.join("GOAL.md").is_file() {
        return Err(ErrorOraculo::Uso(format!(
            "falta GOAL.md en {}",
            raiz_root.display()
        )));
    }
    let root_toml = home.join("root.toml");
    if root_toml.is_file() {
        let texto = fs::read_to_string(&root_toml)
            .map_err(|e| ErrorOraculo::Infra(format!("leer root.toml: {e}")))?;
        revisar_root_toml(&texto).map_err(ErrorOraculo::Uso)?;
    }
    let plugins = plugins_de(&config)?;

    // La carpeta de trabajo vive en ROOT_HOME/tmp y se borra al salir, también ante error.
    let tmp = home.join(CARPETA_TMP);
    fs::create_dir_all(&tmp)
        .map_err(|e| ErrorOraculo::Infra(format!("crear {}: {e}", tmp.display())))?;
    let staging = tmp.join(format!("colab-empaquetar-corre-{}", std::process::id()));
    borrado::borrar_dir_dentro(&tmp, &staging)
        .map_err(|e| ErrorOraculo::Infra(format!("limpiar {}: {e}", staging.display())))?;
    fs::create_dir_all(&staging)
        .map_err(|e| ErrorOraculo::Infra(format!("crear {}: {e}", staging.display())))?;
    let resultado = armar(
        &Armado {
            home,
            raiz_root,
            commit,
            pedido,
            bytes: &bytes,
            plugins: &plugins,
            staging: &staging,
        },
        imagen,
        oraculo,
    );
    let limpieza = borrado::borrar_dir_dentro(&tmp, &staging);
    let resumen = resultado?;
    limpieza.map_err(|e| ErrorOraculo::Infra(format!("borrar la carpeta de trabajo: {e}")))?;
    Ok(resumen)
}

/// Datos fijos de un armado. Viajan juntos para no alargar las firmas.
struct Armado<'a> {
    home: &'a Path,
    raiz_root: &'a Path,
    commit: &'a str,
    pedido: &'a PedidoEmpaquetarCorre,
    bytes: &'a [u8],
    /// Ruta absoluta original del plugin -> ruta relativa dentro del paquete.
    plugins: &'a BTreeMap<PathBuf, String>,
    staging: &'a Path,
}

/// Revisa las tareas pedidas: no vacías, sin repetir, con nombre válido y con oráculo en ROOT_HOME.
fn validar_tareas(home: &Path, tareas: &[String]) -> Result<(), ErrorOraculo> {
    if tareas.is_empty() {
        return Err(ErrorOraculo::Uso(
            "hace falta al menos una tarea en --tareas".into(),
        ));
    }
    let mut vistas = BTreeSet::new();
    for tarea in tareas {
        nombre_de_tarea_valido(tarea).map_err(ErrorOraculo::Uso)?;
        if !vistas.insert(tarea) {
            return Err(ErrorOraculo::Uso(format!(
                "la tarea {tarea} está repetida en --tareas"
            )));
        }
        if !home.join("oraculos").join(tarea).is_dir() {
            return Err(ErrorOraculo::Uso(format!(
                "la tarea {tarea} no existe en ROOT_HOME"
            )));
        }
    }
    Ok(())
}

/// Convierte el error de leer la config: un error de uso sigue siendo de uso.
fn error_de_corre(error: ErrorCorre) -> ErrorOraculo {
    match error {
        ErrorCorre::Uso(mensaje) => ErrorOraculo::Uso(mensaje),
        otro => ErrorOraculo::Infra(otro.to_string()),
    }
}

/// Carpetas de plugin que la config fija, sin repetir rutas. Cada una lleva su nombre relativo.
/// Un plugin que no existe es error de uso.
fn plugins_de(config: &ConfigCorrida) -> Result<BTreeMap<PathBuf, String>, ErrorOraculo> {
    let mut plugins: BTreeMap<PathBuf, String> = BTreeMap::new();
    for rol in config.claude.rol.keys() {
        let Some(origen) = config.claude.plugin_de(*rol) else {
            continue;
        };
        if plugins.contains_key(origen) {
            continue;
        }
        if !origen.is_dir() {
            return Err(ErrorOraculo::Uso(format!(
                "el plugin {} no existe",
                origen.display()
            )));
        }
        let nombre = origen
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "plugin".into());
        let relativa = format!("plugins/{}-{nombre}", plugins.len());
        plugins.insert(origen.to_path_buf(), relativa);
    }
    Ok(plugins)
}

/// Llena la carpeta de trabajo, la revisa, escribe el manifiesto y empaqueta con `tar`.
fn armar(
    a: &Armado,
    imagen: &dyn ImagenBuilder,
    oraculo: &dyn Imagen,
) -> Result<Resumen, ErrorOraculo> {
    let staging = a.staging;
    let claude = imagen.version_claude()?;
    let version_claude = claude
        .split_whitespace()
        .next()
        .ok_or_else(|| ErrorOraculo::Infra(format!("salida de claude vacía: {claude}")))?;
    let salida_rustc = imagen.version_rustc()?;
    let version_rust = version_de_rustc(&salida_rustc).ok_or_else(|| {
        ErrorOraculo::Infra(format!(
            "salida de rustc no reconocida: {}",
            salida_rustc.trim()
        ))
    })?;

    escribir(&staging.join(BINARIO), a.bytes)?;
    fijar_ejecutable(&staging.join(BINARIO))?;
    let binario_sha256 = sha256_hex(a.bytes);
    let imagen_id =
        armar_oraculo_runtime(staging, &a.pedido.rustup_sha256, oraculo, IMAGEN_ORACULO)?;
    escribir(
        &staging.join(VERSIONES),
        format!("CLAUDE_VERSION={version_claude}\nRUST_VERSION={version_rust}\n").as_bytes(),
    )?;

    copiar_arbol(
        &a.raiz_root.join("tools/colab"),
        &staging.join("tools/colab"),
    )?;
    copiar_archivo(&a.raiz_root.join("GOAL.md"), &staging.join("GOAL.md"))?;

    let raiz_home = staging.join(RAIZ_HOME);
    let configs = raiz_home.join("configs");
    fs::create_dir_all(&configs)
        .map_err(|e| ErrorOraculo::Infra(format!("crear {}: {e}", configs.display())))?;
    copiar_archivo(
        &ruta_de(a.home, &a.pedido.config),
        &configs.join(format!("{}.toml", a.pedido.config)),
    )?;
    copiar_si_existe(&a.home.join("root.toml"), &raiz_home.join("root.toml"))?;
    copiar_si_existe(
        &a.home.join("calibracion.json"),
        &raiz_home.join("calibracion.json"),
    )?;
    let calibracion = a.home.join("calibracion");
    if calibracion.is_dir() {
        copiar_arbol(&calibracion, &raiz_home.join("calibracion"))?;
    }
    copiar_si_existe(&ruta_eventos_en(a.home), &raiz_home.join("eventos.jsonl"))?;
    for tarea in &a.pedido.tareas {
        copiar_arbol(
            &a.home.join("oraculos").join(tarea),
            &raiz_home.join("oraculos").join(tarea),
        )?;
    }
    for (origen, relativa) in a.plugins {
        copiar_arbol(origen, &staging.join(relativa))?;
    }

    // Antes del manifiesto y del tar: nada con secretos sale del paquete.
    revisar_paquete(staging)?;

    let archivos = archivos_de(staging)?;
    // Cuenta también `manifiesto.json`, que todavía no existe.
    let contados = archivos.len() + 1;
    let manifiesto = ManifiestoCorre {
        v: 1,
        tipo: "corre".into(),
        pasada: a.pedido.pasada,
        commit_root: a.commit.into(),
        config: a.pedido.config.clone(),
        binario_sha256,
        imagen_id,
        tareas: a.pedido.tareas.clone(),
        plugins: a
            .plugins
            .iter()
            .map(|(origen, relativa)| (origen.display().to_string(), relativa.clone()))
            .collect(),
        archivos,
    };
    let json = serde_json::to_string_pretty(&manifiesto)
        .map_err(|e| ErrorOraculo::Infra(format!("serializar el manifiesto: {e}")))?;
    escribir(&staging.join(MANIFIESTO), format!("{json}\n").as_bytes())?;

    // `tar` recibe la salida absoluta: con `-C`, una ruta relativa se resolvería mal.
    let salida = std::path::absolute(&a.pedido.salida)
        .map_err(|e| ErrorOraculo::Infra(format!("ruta de salida: {e}")))?;
    let estado = Command::new("tar")
        .arg("-czf")
        .arg(&salida)
        .arg("-C")
        .arg(staging)
        .arg(".")
        .output()
        .map_err(|e| ErrorOraculo::Infra(format!("correr tar: {e}")))?;
    if !estado.status.success() {
        return Err(ErrorOraculo::Infra(format!(
            "tar falló: {}",
            String::from_utf8_lossy(&estado.stderr).trim()
        )));
    }
    let tar = fs::read(&salida)
        .map_err(|e| ErrorOraculo::Infra(format!("leer {}: {e}", salida.display())))?;
    Ok(Resumen {
        salida,
        archivos: contados,
        trabajos: a.pedido.tareas.len(),
        sha256: sha256_hex(&tar),
    })
}

/// Revisión final del paquete armado. Falla con `Uso` que nombra la ruta, nunca el valor.
/// Frena `.env*`, `.credentials.json` y texto con forma de clave `sk-`. Salta `r00t`.
fn revisar_paquete(raiz: &Path) -> Result<(), ErrorOraculo> {
    let mut rutas = Vec::new();
    archivos_relativos(raiz, raiz, &mut rutas)?;
    for relativa in rutas {
        if relativa == BINARIO {
            continue;
        }
        let nombre = relativa.rsplit('/').next().unwrap_or(&relativa);
        if nombre.starts_with(".env") || nombre == ".credentials.json" {
            return Err(ErrorOraculo::Uso(format!(
                "{relativa} parece un secreto (.env o .credentials.json); quítalo antes de empaquetar"
            )));
        }
        let bytes = fs::read(raiz.join(&relativa))
            .map_err(|e| ErrorOraculo::Infra(format!("leer {relativa}: {e}")))?;
        // Los archivos que no son UTF-8 no son texto con claves: se saltan.
        match std::str::from_utf8(&bytes) {
            Ok(texto) if tiene_forma_de_clave(texto) => {
                return Err(ErrorOraculo::Uso(format!(
                    "{relativa} parece traer una clave sk-; quítala antes de empaquetar"
                )));
            }
            _ => {}
        }
    }
    Ok(())
}

/// Dice si el texto trae `sk-` precedido de un no alfanumérico (o al inicio) y seguido de
/// al menos 16 caracteres de `[A-Za-z0-9_-]`.
fn tiene_forma_de_clave(texto: &str) -> bool {
    let bytes = texto.as_bytes();
    (0..bytes.len().saturating_sub(2)).any(|i| {
        bytes[i..].starts_with(b"sk-")
            && (i == 0 || !bytes[i - 1].is_ascii_alphanumeric())
            && bytes[i + 3..]
                .iter()
                .take_while(|b| b.is_ascii_alphanumeric() || **b == b'_' || **b == b'-')
                .count()
                >= 16
    })
}

/// Commit de `HEAD` en el repo de Root. Falla si git no responde.
fn commit_de(raiz: &Path) -> Result<String, ErrorOraculo> {
    let salida = Command::new("git")
        .arg("-C")
        .arg(raiz)
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|e| ErrorOraculo::Infra(format!("correr git: {e}")))?;
    if !salida.status.success() {
        return Err(ErrorOraculo::Infra(format!(
            "git rev-parse HEAD falló en {}",
            raiz.display()
        )));
    }
    Ok(String::from_utf8_lossy(&salida.stdout).trim().to_string())
}

/// Copia `origen` a `destino` en cualquier nivel. Omite los nombres de `EXCLUIDOS`.
/// Un enlace simbólico falla con `Uso` que nombra su ruta.
fn copiar_arbol(origen: &Path, destino: &Path) -> Result<(), ErrorOraculo> {
    let error = |e: io::Error| ErrorOraculo::Infra(format!("copiar {}: {e}", origen.display()));
    fs::create_dir_all(destino)
        .map_err(|e| ErrorOraculo::Infra(format!("crear {}: {e}", destino.display())))?;
    for entrada in fs::read_dir(origen).map_err(error)? {
        let entrada = entrada.map_err(error)?;
        let nombre_os = entrada.file_name();
        let nombre = nombre_os.to_string_lossy();
        if EXCLUIDOS.iter().any(|(excluido, _)| *excluido == &*nombre) {
            continue;
        }
        let ruta = entrada.path();
        let tipo = entrada.file_type().map_err(error)?;
        if tipo.is_symlink() {
            return Err(ErrorOraculo::Uso(format!(
                "{} es un enlace simbólico; no se empaqueta, quítalo",
                ruta.display()
            )));
        }
        let destino_entrada = destino.join(&*nombre);
        if tipo.is_dir() {
            copiar_arbol(&ruta, &destino_entrada)?;
        } else if tipo.is_file() {
            fs::copy(&ruta, &destino_entrada)
                .map_err(|e| ErrorOraculo::Infra(format!("copiar {}: {e}", ruta.display())))?;
        }
    }
    Ok(())
}

/// Copia un archivo. Si no existe, falla con `Infra`.
fn copiar_archivo(origen: &Path, destino: &Path) -> Result<(), ErrorOraculo> {
    fs::copy(origen, destino)
        .map(|_| ())
        .map_err(|e| ErrorOraculo::Infra(format!("copiar {}: {e}", origen.display())))
}

/// Copia un archivo si existe. Si no existe, no hace nada.
fn copiar_si_existe(origen: &Path, destino: &Path) -> Result<(), ErrorOraculo> {
    if !origen.is_file() {
        return Ok(());
    }
    copiar_archivo(origen, destino)
}

/// Escribe `bytes` en `ruta`.
fn escribir(ruta: &Path, bytes: &[u8]) -> Result<(), ErrorOraculo> {
    fs::write(ruta, bytes)
        .map_err(|e| ErrorOraculo::Infra(format!("escribir {}: {e}", ruta.display())))
}

/// Pone permisos 0755 al archivo.
fn fijar_ejecutable(ruta: &Path) -> Result<(), ErrorOraculo> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(ruta, fs::Permissions::from_mode(0o755))
        .map_err(|e| ErrorOraculo::Infra(format!("permisos de {}: {e}", ruta.display())))
}

/// Rutas relativas de los archivos bajo `dir`, con `/` como separador.
fn archivos_relativos(
    raiz: &Path,
    dir: &Path,
    salida: &mut Vec<String>,
) -> Result<(), ErrorOraculo> {
    let error = |e: io::Error| ErrorOraculo::Infra(format!("leer {}: {e}", dir.display()));
    for entrada in fs::read_dir(dir).map_err(error)? {
        let ruta = entrada.map_err(error)?.path();
        if ruta.is_dir() {
            archivos_relativos(raiz, &ruta, salida)?;
        } else if ruta.is_file() {
            let relativa = ruta
                .strip_prefix(raiz)
                .map_err(|e| ErrorOraculo::Infra(format!("ruta del paquete: {e}")))?;
            let clave = relativa
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            salida.push(clave);
        }
    }
    Ok(())
}

/// SHA-256 de cada archivo bajo `raiz`, con la ruta relativa como clave.
fn archivos_de(raiz: &Path) -> Result<BTreeMap<String, String>, ErrorOraculo> {
    let mut rutas = Vec::new();
    archivos_relativos(raiz, raiz, &mut rutas)?;
    rutas
        .into_iter()
        .map(|relativa| {
            let bytes = fs::read(raiz.join(&relativa))
                .map_err(|e| ErrorOraculo::Infra(format!("leer {relativa}: {e}")))?;
            Ok((relativa, sha256_hex(&bytes)))
        })
        .collect()
}

/// Pedido de `colab importar --corre`, tal como llega de la línea de comandos.
#[derive(Debug, Clone)]
pub struct PedidoImportarCorre {
    /// Vuelta de una tarea: `vuelta-<tarea>.tar.gz`.
    pub vuelta: PathBuf,
    /// SHA-256 en hexadecimal de la vuelta, tal como lo dio quien la trajo.
    pub sha: String,
    /// Paquete de ida de `colab empaquetar --corre`. De ahí salen la pasada y el commit.
    pub ida: PathBuf,
    /// Repo de Root: de ahí sale el commit de la laptop. Por defecto, el directorio actual.
    pub raiz_root: Option<PathBuf>,
}

/// Clase de una corrida de la vuelta. `Completa` tiene fila y entra a `runs.jsonl`.
/// `Cortada` no tiene fila: solo suma su gasto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClaseCorrida {
    Completa,
    Cortada,
}

/// Lo que devuelve `colab importar --corre`. No trae nombres de tests ni nada de `oculto/`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResumenImportarCorre {
    pub tarea: String,
    pub pasada: u32,
    /// Si la vuelta entró.
    pub entro: bool,
    /// Por qué no entró. `None` si entró.
    pub motivo: Option<String>,
    pub clase: ClaseCorrida,
    /// Estado de la fila: `verde`, `rojo` o `sin_veredicto`. `None` si la vuelta no trae fila.
    pub veredicto: Option<String>,
    /// Pasan y total del contrato.
    pub contrato: Option<(u32, u32)>,
    /// Pasan y total del oculto.
    pub oculto: Option<(u32, u32)>,
    pub minutos: Option<f64>,
    pub gasto_usd: f64,
}

/// Archivo de registro en ROOT_HOME: qué vueltas entraron, por pasada y tarea.
const REGISTRO_CORRE: &str = "colab-corre.json";
/// Prefijo del motivo de un `Gate` de cupo agotado. Lo escribe `corre::corrida`.
const MOTIVO_CUPO: &str = "cupo agotado antes del rol";
/// Origen que se marca en los eventos importados desde la vuelta.
const ORIGEN_COLAB: &str = "colab";

/// Una entrada del registro: la vuelta por su SHA, su clase y el id de su fila (`""` sin fila).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct EntradaRegistro {
    sha: String,
    clase: ClaseCorrida,
    id: String,
}

/// Registro de `colab-corre.json`: `pasada` -> tarea -> entradas.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct Registro {
    #[serde(default)]
    pasadas: BTreeMap<String, BTreeMap<String, Vec<EntradaRegistro>>>,
}

/// Punto de entrada de `colab importar --corre`. Toma el commit de la laptop de `raiz_root` y el ROOT_HOME real.
pub fn importar_corre_cli(
    pedido: PedidoImportarCorre,
) -> Result<ResumenImportarCorre, ErrorOraculo> {
    let raiz_root = match &pedido.raiz_root {
        Some(raiz) => raiz.clone(),
        None => std::env::current_dir()
            .map_err(|e| ErrorOraculo::Infra(format!("directorio actual: {e}")))?,
    };
    let commit = commit_de(&raiz_root)?;
    importar_corre(&root_home(), &pedido, &commit)
}

/// Importa una vuelta de `corre` en `home`. Revisa todo antes de escribir. `commit_laptop` llega como argumento.
/// Una vuelta que no entra no escribe nada en `home`; el resumen dice por qué.
pub fn importar_corre(
    home: &Path,
    pedido: &PedidoImportarCorre,
    commit_laptop: &str,
) -> Result<ResumenImportarCorre, ErrorOraculo> {
    if !es_sha256_hex(&pedido.sha) {
        return Err(ErrorOraculo::Uso(format!(
            "--sha quiere 64 caracteres hexadecimales, llegó {}",
            pedido.sha
        )));
    }
    verificar_sha_de_la_vuelta(&pedido.sha, &pedido.vuelta)?;

    // Las carpetas de trabajo viven en ROOT_HOME/tmp y se borran al salir, también ante error.
    let tmp = home.join(CARPETA_TMP);
    fs::create_dir_all(&tmp)
        .map_err(|e| ErrorOraculo::Infra(format!("crear {}: {e}", tmp.display())))?;
    let id = std::process::id();
    let ida_dir = tmp.join(format!("colab-importar-corre-{id}"));
    let vuelta_dir = tmp.join(format!("colab-importar-corre-{id}-vuelta"));
    for dir in [&ida_dir, &vuelta_dir] {
        borrado::borrar_dir_dentro(&tmp, dir)
            .map_err(|e| ErrorOraculo::Infra(format!("limpiar {}: {e}", dir.display())))?;
        fs::create_dir_all(dir)
            .map_err(|e| ErrorOraculo::Infra(format!("crear {}: {e}", dir.display())))?;
    }
    let resultado = importar_corre_desde(home, pedido, &ida_dir, &vuelta_dir, commit_laptop);
    let limpieza_ida = borrado::borrar_dir_dentro(&tmp, &ida_dir);
    let limpieza_vuelta = borrado::borrar_dir_dentro(&tmp, &vuelta_dir);
    let resumen = resultado?;
    limpieza_ida
        .and(limpieza_vuelta)
        .map_err(|e| ErrorOraculo::Infra(format!("borrar la carpeta de trabajo: {e}")))?;
    Ok(resumen)
}

/// Cuerpo de `importar_corre`: extrae ida y vuelta, decide y escribe. Las carpetas de trabajo ya existen y están vacías.
fn importar_corre_desde(
    home: &Path,
    pedido: &PedidoImportarCorre,
    ida_dir: &Path,
    vuelta_dir: &Path,
    commit_laptop: &str,
) -> Result<ResumenImportarCorre, ErrorOraculo> {
    extraer_seguro(&pedido.ida, ida_dir)?;
    let manifiesto = leer_manifiesto_de_ida(ida_dir)?;

    extraer_seguro(&pedido.vuelta, vuelta_dir)?;
    let meta: VueltaMeta = leer_json_de_vuelta(&vuelta_dir.join(VUELTA_META))?;
    if !manifiesto.tareas.contains(&meta.tarea) {
        return Err(ErrorOraculo::Uso(format!(
            "la tarea {} no está en el paquete de ida: {}",
            meta.tarea,
            manifiesto.tareas.join(", ")
        )));
    }
    if manifiesto.commit_root != commit_laptop {
        return Err(ErrorOraculo::Uso(format!(
            "el commit del paquete es {}; la laptop está en {}",
            manifiesto.commit_root, commit_laptop
        )));
    }

    let filas: Vec<Fila> = lineas_de_vuelta(&vuelta_dir.join(VUELTA_RUNS))?;
    let de_la_tarea: Vec<&Fila> = filas.iter().filter(|f| f.tarea == meta.tarea).collect();
    if de_la_tarea.len() > 1 {
        return Err(ErrorOraculo::Uso(format!(
            "la vuelta trae {} filas de {}; se espera una",
            de_la_tarea.len(),
            meta.tarea
        )));
    }
    let fila = de_la_tarea.first().map(|f| (*f).clone());
    let eventos: Vec<Evento> = lineas_de_vuelta(&vuelta_dir.join(VUELTA_EVENTOS))?;

    let completa = fila.as_ref().is_some_and(|f| !es_sin_veredicto(f))
        && meta.cortada.is_none()
        && !cupo_agotado(&eventos);
    let clase = if completa {
        ClaseCorrida::Completa
    } else {
        ClaseCorrida::Cortada
    };
    let gasto_usd = match &fila {
        Some(f) => f.resumen.usd_api,
        None => eventos
            .iter()
            .filter(|ev| es_gasto(ev.tipo))
            .fold(0.0, |total, ev| total + ev.cost_usd),
    };
    let mut resumen = ResumenImportarCorre {
        tarea: meta.tarea.clone(),
        pasada: manifiesto.pasada,
        entro: false,
        motivo: None,
        clase,
        veredicto: fila
            .as_ref()
            .map(|f| etiqueta_estado(f.veredicto.estado).to_string()),
        contrato: fila
            .as_ref()
            .map(|f| (f.veredicto.contrato.pasa, f.veredicto.contrato.total)),
        oculto: fila
            .as_ref()
            .map(|f| (f.veredicto.oculto.pasa, f.veredicto.oculto.total)),
        minutos: fila.as_ref().map(|f| f.resumen.minutos),
        gasto_usd,
    };

    let sha = pedido.sha.to_ascii_lowercase();
    let mut registro = leer_registro(&home.join(REGISTRO_CORRE))?;
    let previas: Vec<EntradaRegistro> = registro
        .pasadas
        .get(&manifiesto.pasada.to_string())
        .and_then(|tareas| tareas.get(&meta.tarea))
        .cloned()
        .unwrap_or_default();
    if previas.iter().any(|e| e.sha == sha) {
        resumen.motivo = Some("ya estaba".into());
        return Ok(resumen);
    }
    if clase == ClaseCorrida::Completa && previas.iter().any(|e| e.clase == ClaseCorrida::Completa)
    {
        resumen.motivo = Some("ya hay una corrida completa".into());
        return Ok(resumen);
    }

    // Revisado todo: a partir de aquí se escribe.
    let id = fila.as_ref().map(|f| f.id.clone()).unwrap_or_default();
    if clase == ClaseCorrida::Completa {
        if !id_de_carpeta_valido(&id) {
            return Err(ErrorOraculo::Uso(format!(
                "el id de la fila no sirve como nombre de carpeta: {id}"
            )));
        }
        let origen = vuelta_dir.join(CARPETA_TRANSCRIPTS).join(&id);
        if origen.is_dir() && home.join(CARPETA_TRANSCRIPTS).join(&id).exists() {
            return Err(ErrorOraculo::Infra(format!(
                "ya existe transcripts/{id} en la laptop; no se importó nada"
            )));
        }
    }
    let a_importar: Vec<Evento> = eventos
        .into_iter()
        .filter(|ev| clase == ClaseCorrida::Completa || es_gasto(ev.tipo))
        .map(|mut ev| {
            ev.extra
                .insert("origen".into(), Value::String(ORIGEN_COLAB.into()));
            ev
        })
        .collect();
    agregar_eventos(home, &a_importar)?;
    if let (ClaseCorrida::Completa, Some(fila)) = (clase, &fila) {
        agregar_en(&home.join("runs.jsonl"), fila)
            .map_err(|e| ErrorOraculo::Infra(format!("escribir runs.jsonl: {e}")))?;
        copiar_transcripts(vuelta_dir, home, &id)?;
    }
    registro
        .pasadas
        .entry(manifiesto.pasada.to_string())
        .or_default()
        .entry(meta.tarea.clone())
        .or_default()
        .push(EntradaRegistro { sha, clase, id });
    escribir_registro(home, &registro)?;
    resumen.entro = true;
    Ok(resumen)
}

/// Nombre del `vuelta.json` dentro de la vuelta.
const VUELTA_META: &str = "vuelta.json";
/// Filas nuevas de la vuelta.
const VUELTA_RUNS: &str = "runs.jsonl";
/// Eventos nuevos de la vuelta.
const VUELTA_EVENTOS: &str = "eventos.jsonl";
/// Carpeta de transcripts dentro de la vuelta y dentro de ROOT_HOME.
const CARPETA_TRANSCRIPTS: &str = "transcripts";

/// Lo que la importación lee de `vuelta.json`. Lo demás del archivo no hace falta.
#[derive(Debug, Clone, Deserialize)]
struct VueltaMeta {
    tarea: String,
    /// Motivo del corte: `plazo`, o `null` si la corrida terminó.
    cortada: Option<String>,
}

/// Revisa el SHA-256 de la vuelta contra `--sha`. Sin coincidencia, la vuelta no se abre.
fn verificar_sha_de_la_vuelta(esperado: &str, vuelta: &Path) -> Result<(), ErrorOraculo> {
    if !vuelta.is_file() {
        return Err(ErrorOraculo::Uso(format!(
            "no existe la vuelta {}",
            vuelta.display()
        )));
    }
    let bytes = fs::read(vuelta)
        .map_err(|e| ErrorOraculo::Infra(format!("leer {}: {e}", vuelta.display())))?;
    let real = sha256_hex(&bytes);
    if real == esperado.to_ascii_lowercase() {
        return Ok(());
    }
    Err(ErrorOraculo::Infra(format!(
        "el SHA-256 de la vuelta es {real}, --sha dio {esperado}; no se importó nada"
    )))
}

/// Extrae un tar.gz en `dir` con el `tar` del sistema. Antes revisa la lista: rechaza enlaces
/// y rutas absolutas o con `..`. El error nombra la entrada, nunca su contenido.
fn extraer_seguro(archivo: &Path, dir: &Path) -> Result<(), ErrorOraculo> {
    if !archivo.is_file() {
        return Err(ErrorOraculo::Uso(format!(
            "no existe el paquete {}",
            archivo.display()
        )));
    }
    let absoluto = std::path::absolute(archivo)
        .map_err(|e| ErrorOraculo::Infra(format!("ruta del paquete: {e}")))?;
    let nombres = tar_listado(&absoluto, &["-tzf"])?;
    for nombre in &nombres {
        if nombre.starts_with('/') || nombre.split('/').any(|c| c == "..") {
            return Err(ErrorOraculo::Uso(format!(
                "el paquete trae una ruta no permitida: {nombre}"
            )));
        }
    }
    // `-tvzf` da el tipo de cada entrada en el mismo orden que `-tzf`: `l` enlace, `h` enlace duro.
    let detalle = tar_listado(&absoluto, &["-tvzf"])?;
    for (nombre, linea) in nombres.iter().zip(&detalle) {
        if linea.starts_with('l') || linea.starts_with('h') {
            return Err(ErrorOraculo::Uso(format!(
                "el paquete trae un enlace: {nombre}; no se importa"
            )));
        }
    }
    let salida = Command::new("tar")
        .arg("-xzf")
        .arg(&absoluto)
        .arg("-C")
        .arg(dir)
        .output()
        .map_err(|e| ErrorOraculo::Infra(format!("correr tar: {e}")))?;
    if !salida.status.success() {
        return Err(ErrorOraculo::Uso(format!(
            "no se pudo leer el paquete: {}",
            String::from_utf8_lossy(&salida.stderr).trim()
        )));
    }
    Ok(())
}

/// Líneas de `tar` con `modo` sobre `archivo`, sin líneas vacías.
fn tar_listado(archivo: &Path, modo: &[&str]) -> Result<Vec<String>, ErrorOraculo> {
    let salida = Command::new("tar")
        .args(modo)
        .arg(archivo)
        .output()
        .map_err(|e| ErrorOraculo::Infra(format!("correr tar: {e}")))?;
    if !salida.status.success() {
        return Err(ErrorOraculo::Uso(format!(
            "no se pudo leer el paquete: {}",
            String::from_utf8_lossy(&salida.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&salida.stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(str::to_string)
        .collect())
}

/// Lee `manifiesto.json` de la ida. Debe ser un manifiesto de `corre`.
fn leer_manifiesto_de_ida(ida_dir: &Path) -> Result<ManifiestoCorre, ErrorOraculo> {
    let ruta = ida_dir.join(MANIFIESTO);
    let texto = fs::read_to_string(&ruta)
        .map_err(|_| ErrorOraculo::Uso("falta manifiesto.json en el paquete de ida".into()))?;
    let valor: Value = serde_json::from_str(&texto)
        .map_err(|e| ErrorOraculo::Uso(format!("manifiesto.json ilegible: {e}")))?;
    if valor.get("tipo").and_then(Value::as_str) != Some("corre") {
        return Err(ErrorOraculo::Uso(
            "el paquete de ida no es de corre (tipo distinto de \"corre\")".into(),
        ));
    }
    serde_json::from_value(valor)
        .map_err(|e| ErrorOraculo::Uso(format!("manifiesto.json ilegible: {e}")))
}

/// Lee un JSON de la vuelta. Un archivo que falta o no parsea es error de uso.
fn leer_json_de_vuelta<T: serde::de::DeserializeOwned>(ruta: &Path) -> Result<T, ErrorOraculo> {
    let nombre = ruta
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let texto = fs::read_to_string(ruta)
        .map_err(|_| ErrorOraculo::Uso(format!("falta {nombre} en la vuelta")))?;
    serde_json::from_str(&texto).map_err(|e| ErrorOraculo::Uso(format!("{nombre} ilegible: {e}")))
}

/// Lee un `.jsonl` de la vuelta. Sin archivo, lista vacía. Una línea que no parsea es error de uso.
fn lineas_de_vuelta<T: serde::de::DeserializeOwned>(ruta: &Path) -> Result<Vec<T>, ErrorOraculo> {
    let texto = match fs::read_to_string(ruta) {
        Ok(texto) => texto,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(ErrorOraculo::Infra(format!("leer {}: {e}", ruta.display()))),
    };
    let nombre = ruta
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    texto
        .lines()
        .filter(|l| !l.trim().is_empty())
        .enumerate()
        .map(|(indice, linea)| {
            serde_json::from_str(linea)
                .map_err(|_| ErrorOraculo::Uso(format!("{nombre} línea {} ilegible", indice + 1)))
        })
        .collect()
}

/// Dice si hay un `Gate` de cupo agotado antes de un rol. Lo escribe `corre::corrida` en `extra.motivo`.
fn cupo_agotado(eventos: &[Evento]) -> bool {
    eventos.iter().any(|ev| {
        ev.tipo == TipoEvento::Gate
            && ev
                .extra
                .get("motivo")
                .and_then(Value::as_str)
                .is_some_and(|motivo| motivo.starts_with(MOTIVO_CUPO))
    })
}

/// Dice si un tipo de evento es gasto de API de la vuelta: solo llamadas y reservas.
fn es_gasto(tipo: TipoEvento) -> bool {
    matches!(tipo, TipoEvento::Llamada | TipoEvento::Reserva)
}

/// Etiqueta de un estado de la fila, como se muestra en el resumen.
fn etiqueta_estado(estado: Estado) -> &'static str {
    match estado {
        Estado::Verde => "verde",
        Estado::Rojo => "rojo",
        Estado::SinVeredicto => "sin_veredicto",
    }
}

/// Dice si un id de fila sirve como nombre de carpeta: no vacío y sin separadores ni `..`.
fn id_de_carpeta_valido(id: &str) -> bool {
    !id.is_empty() && !id.contains(['/', '\\']) && id != "." && id != ".."
}

/// Clave de un evento para no duplicarlo: corrida, `extra.reserva` (o ausente) y `ts`.
type ClaveEvento = (String, Option<String>, u64);

fn clave_de_evento(ev: &Evento) -> ClaveEvento {
    (
        ev.corrida.clone(),
        ev.extra.get("reserva").map(Value::to_string),
        ev.ts.to_bits(),
    )
}

/// Eventos de `nuevos` que `existentes` no tiene. Una clave repetida cuenta una vez.
fn pendientes_de(existentes: &[Evento], nuevos: &[Evento]) -> Vec<Evento> {
    let mut vistos: BTreeSet<ClaveEvento> = existentes.iter().map(clave_de_evento).collect();
    nuevos
        .iter()
        .filter(|ev| vistos.insert(clave_de_evento(ev)))
        .cloned()
        .collect()
}

/// Agrega a `eventos.jsonl` de `home` los eventos que faltan. Devuelve cuántos agregó.
/// Lee y escribe bajo el mismo lock, como `agregar_gasto`.
fn agregar_eventos(home: &Path, eventos: &[Evento]) -> Result<usize, ErrorOraculo> {
    let ruta = ruta_eventos_en(home);
    crate::observe::uso::con_eventos_bloqueados(
        &ruta,
        |existentes, log| -> Result<usize, crate::observe::ErrorObserve> {
            let pendientes = pendientes_de(existentes, eventos);
            for ev in &pendientes {
                let linea = serde_json::to_string(ev)?;
                log.agregar(&linea, crate::observe::jsonl::UMBRAL_POR_DEFECTO)?;
            }
            Ok(pendientes.len())
        },
    )
    .map_err(|e| ErrorOraculo::Infra(format!("escribir eventos en eventos.jsonl: {e}")))
}

/// Copia `transcripts/<id>` de la vuelta a `home/transcripts/<id>`. Sin carpeta en la vuelta, no hace nada.
/// La carpeta de destino no debe existir: `create_dir` la crea sola o falla.
fn copiar_transcripts(vuelta_dir: &Path, home: &Path, id: &str) -> Result<(), ErrorOraculo> {
    let origen = vuelta_dir.join(CARPETA_TRANSCRIPTS).join(id);
    if !origen.is_dir() {
        return Ok(());
    }
    let padre = home.join(CARPETA_TRANSCRIPTS);
    fs::create_dir_all(&padre)
        .map_err(|e| ErrorOraculo::Infra(format!("crear {}: {e}", padre.display())))?;
    let destino = padre.join(id);
    fs::create_dir(&destino)
        .map_err(|e| ErrorOraculo::Infra(format!("crear {}: {e}", destino.display())))?;
    copiar_carpeta(&origen, &destino)
}

/// Copia el contenido de `origen` a `destino`, en cualquier nivel. Un enlace no se copia.
fn copiar_carpeta(origen: &Path, destino: &Path) -> Result<(), ErrorOraculo> {
    let error = |e: io::Error| ErrorOraculo::Infra(format!("copiar {}: {e}", origen.display()));
    fs::create_dir_all(destino)
        .map_err(|e| ErrorOraculo::Infra(format!("crear {}: {e}", destino.display())))?;
    for entrada in fs::read_dir(origen).map_err(error)? {
        let entrada = entrada.map_err(error)?;
        let ruta = entrada.path();
        let tipo = entrada.file_type().map_err(error)?;
        let destino_entrada = destino.join(entrada.file_name());
        if tipo.is_dir() {
            copiar_carpeta(&ruta, &destino_entrada)?;
        } else if tipo.is_file() {
            fs::copy(&ruta, &destino_entrada)
                .map_err(|e| ErrorOraculo::Infra(format!("copiar {}: {e}", ruta.display())))?;
        }
    }
    Ok(())
}

/// Lee `colab-corre.json`. Sin archivo, el registro está vacío. Un archivo ilegible es error de infraestructura.
fn leer_registro(ruta: &Path) -> Result<Registro, ErrorOraculo> {
    match fs::read_to_string(ruta) {
        Ok(texto) => serde_json::from_str(&texto)
            .map_err(|e| ErrorOraculo::Infra(format!("{REGISTRO_CORRE} ilegible: {e}"))),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Registro::default()),
        Err(e) => Err(ErrorOraculo::Infra(format!("leer {}: {e}", ruta.display()))),
    }
}

/// Escribe `colab-corre.json` de forma atómica: primero un temporal en el mismo directorio, luego rename.
fn escribir_registro(home: &Path, registro: &Registro) -> Result<(), ErrorOraculo> {
    let ruta = home.join(REGISTRO_CORRE);
    let temporal = home.join(format!("{REGISTRO_CORRE}.{}.tmp", std::process::id()));
    let json = serde_json::to_string_pretty(registro)
        .map_err(|e| ErrorOraculo::Infra(format!("serializar {REGISTRO_CORRE}: {e}")))?;
    fs::write(&temporal, format!("{json}\n"))
        .map_err(|e| ErrorOraculo::Infra(format!("escribir {}: {e}", temporal.display())))?;
    fs::rename(&temporal, &ruta).map_err(|e| {
        let _ = fs::remove_file(&temporal);
        ErrorOraculo::Infra(format!("cerrar {}: {e}", ruta.display()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::os::unix::fs::symlink;

    /// Doble del builder. Devuelve versiones fijas y no llama a podman.
    struct Falso;

    impl ImagenBuilder for Falso {
        fn version_claude(&self) -> Result<String, ErrorOraculo> {
            Ok("2.1.5 (Claude Code)\n".into())
        }
        fn version_rustc(&self) -> Result<String, ErrorOraculo> {
            Ok("rustc 1.91.0 (abc 2026-01-01)\n".into())
        }
    }

    /// Bytes de un ELF de 64 bits, little endian, x86-64 (e_machine 0x3E).
    const ELF_X86_64: &[u8] = &[
        0x7f, b'E', b'L', b'F', 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0x3e, 0,
    ];

    /// Config válida de una sola fase.
    const CONFIG: &str = "roles = [\"construye\"]\n\n[prompt]\nconstruye = \"haz el trabajo\"\n";

    /// Un ROOT_HOME falso con tres oráculos, una config y una raíz de Root con su binario.
    struct Escenario {
        home: tempfile::TempDir,
        raiz: tempfile::TempDir,
        salida: tempfile::TempDir,
        binario: PathBuf,
    }

    impl Escenario {
        fn nuevo() -> Self {
            let raiz = tempfile::tempdir().expect("raiz");
            let binario = raiz.path().join("r00t-bin");
            fs::write(&binario, ELF_X86_64).expect("escribe");
            fs::create_dir_all(raiz.path().join("tools/colab")).expect("dir");
            fs::write(raiz.path().join("tools/colab/prepara.py"), "print()\n").expect("escribe");
            fs::write(raiz.path().join("GOAL.md"), "# meta\n").expect("escribe");
            let e = Escenario {
                home: tempfile::tempdir().expect("home"),
                raiz,
                salida: tempfile::tempdir().expect("salida"),
                binario,
            };
            for tarea in ["tarea-a", "tarea-b", "tarea-c"] {
                e.oraculo(tarea, "docs/SPEC.md", "# spec\n");
            }
            e.escribe("configs/corrida.toml", CONFIG);
            e
        }

        fn escribe(&self, relativa: &str, texto: &str) {
            let ruta = self.home.path().join(relativa);
            fs::create_dir_all(ruta.parent().expect("padre")).expect("dir");
            fs::write(ruta, texto).expect("escribe");
        }

        fn oraculo(&self, tarea: &str, relativa: &str, texto: &str) {
            self.escribe(&format!("oraculos/{tarea}/{relativa}"), texto);
        }

        fn pedido(&self, tareas: &[&str]) -> PedidoEmpaquetarCorre {
            PedidoEmpaquetarCorre {
                tareas: tareas.iter().map(|t| t.to_string()).collect(),
                config: "corrida".into(),
                pasada: 2,
                binario: self.binario.clone(),
                raiz_root: Some(self.raiz.path().to_path_buf()),
                salida: self.salida.path().join("paquete-corre.tar.gz"),
                rustup_sha256: "a".repeat(64),
            }
        }

        fn empaqueta(&self, pedido: &PedidoEmpaquetarCorre) -> Result<Resumen, ErrorOraculo> {
            empaquetar_corre(
                self.home.path(),
                self.raiz.path(),
                "abc123",
                pedido,
                &Falso,
                &DobleOraculo,
            )
        }
    }

    /// Doble de la imagen del oráculo. Devuelve datos fijos y no llama a podman.
    struct DobleOraculo;

    impl Imagen for DobleOraculo {
        fn extraer_herramienta(
            &self,
            _imagen: &str,
            nombre: &str,
            destino: &Path,
        ) -> Result<(), ErrorOraculo> {
            fs::write(destino, format!("herramienta {nombre}\n"))
                .map_err(|e| ErrorOraculo::Infra(e.to_string()))
        }
        fn version_rustc(&self, _imagen: &str) -> Result<String, ErrorOraculo> {
            Ok("rustc 1.91.0 (abc 2026-01-01)".into())
        }
        fn entorno(&self, _imagen: &str) -> Result<Vec<String>, ErrorOraculo> {
            Ok(vec![
                "PATH=/usr/bin".into(),
                "GITHUB_TOKEN=abc".into(),
                "LANG=C.UTF-8".into(),
            ])
        }
        fn id(&self, _imagen: &str) -> Result<String, ErrorOraculo> {
            Ok("sha256:imagen-doble".into())
        }
    }

    /// Mensaje de un error de uso. Otro tipo de error hace fallar el test.
    fn uso(r: Result<Resumen, ErrorOraculo>) -> String {
        match r {
            Err(ErrorOraculo::Uso(mensaje)) => mensaje,
            otro => panic!("se esperaba Uso, llegó {otro:?}"),
        }
    }

    /// Lista las entradas de archivo del tar, sin `./` ni carpetas.
    fn listar(tgz: &Path) -> BTreeSet<String> {
        let salida = Command::new("tar")
            .arg("-tzf")
            .arg(tgz)
            .output()
            .expect("tar");
        String::from_utf8_lossy(&salida.stdout)
            .lines()
            .filter(|l| !l.ends_with('/'))
            .map(|l| l.trim_start_matches("./").to_string())
            .collect()
    }

    /// Extrae el tar en una carpeta temporal nueva.
    fn extraer(tgz: &Path) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("extraido");
        let estado = Command::new("tar")
            .arg("-xzf")
            .arg(tgz)
            .arg("-C")
            .arg(dir.path())
            .status()
            .expect("tar");
        assert!(estado.success(), "tar falló");
        dir
    }

    /// Rutas relativas de todos los archivos bajo `dir`, con `/`.
    fn archivos_bajo(dir: &Path) -> Vec<String> {
        fn recorre(raiz: &Path, dir: &Path, salida: &mut Vec<String>) {
            for entrada in fs::read_dir(dir).expect("lee dir") {
                let ruta = entrada.expect("entrada").path();
                if ruta.is_dir() {
                    recorre(raiz, &ruta, salida);
                } else {
                    let relativa = ruta.strip_prefix(raiz).expect("prefijo");
                    salida.push(relativa.to_string_lossy().into_owned());
                }
            }
        }
        let mut salida = Vec::new();
        recorre(dir, dir, &mut salida);
        salida
    }

    /// Todo el texto del paquete extraído, concatenado. Sirve para buscar valores.
    fn texto_del_paquete(dir: &Path) -> String {
        archivos_bajo(dir)
            .iter()
            .map(|rel| String::from_utf8_lossy(&fs::read(dir.join(rel)).expect("lee")).into_owned())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Lee el manifiesto de la carpeta extraída.
    fn manifiesto_de(extraido: &Path) -> ManifiestoCorre {
        let texto = fs::read_to_string(extraido.join("manifiesto.json")).expect("lee manifiesto");
        serde_json::from_str(&texto).expect("manifiesto válido")
    }

    #[test]
    fn corre_paquete_trae_herramientas_versiones_y_entorno_del_oraculo() {
        let e = Escenario::nuevo();
        let resumen = e.empaqueta(&e.pedido(&["tarea-a"])).expect("empaqueta");
        let nombres = listar(&resumen.salida);
        for herramienta in [
            "cargo-audit",
            "cargo-mutants",
            "cargo-nextest",
            "gitleaks",
            "hadolint",
        ] {
            assert!(
                nombres.contains(&format!("herramientas/{herramienta}")),
                "falta herramientas/{herramienta}"
            );
        }
        assert!(nombres.contains("versiones.env"), "falta versiones.env");
        assert!(nombres.contains("entorno.env"), "falta entorno.env");
        // El builder sigue con su propio archivo de versiones.
        assert!(nombres.contains("versiones-builder.env"));

        let dir = extraer(&resumen.salida);
        let versiones = fs::read_to_string(dir.path().join("versiones.env")).expect("lee");
        assert!(versiones.contains("RUST_VERSION=1.91.0\n"), "{versiones}");
        assert!(versiones.contains(&format!("RUSTUP_SHA256={}\n", "a".repeat(64))));
        let entorno = fs::read_to_string(dir.path().join("entorno.env")).expect("lee");
        assert!(entorno.contains("PATH=/usr/bin\n"), "{entorno}");
        assert!(!entorno.contains("GITHUB_TOKEN"), "entorno trae un secreto");
    }

    #[test]
    fn corre_manifiesto_trae_el_id_de_la_imagen_del_oraculo() {
        let e = Escenario::nuevo();
        let resumen = e.empaqueta(&e.pedido(&["tarea-a"])).expect("empaqueta");
        let dir = extraer(&resumen.salida);
        assert_eq!(manifiesto_de(dir.path()).imagen_id, "sha256:imagen-doble");
    }

    #[test]
    fn empaquetar_corre_con_sha_invalido_da_uso_antes_de_armar() {
        let e = Escenario::nuevo();
        let mut pedido = e.pedido(&["tarea-a"]);
        pedido.rustup_sha256 = "corto".into();
        let mensaje = uso(e.empaqueta(&pedido));
        assert!(
            mensaje.contains("--rustup-sha256 debe tener 64 caracteres hexadecimales"),
            "{mensaje}"
        );
        assert!(!pedido.salida.exists(), "no debe escribir el paquete");
    }

    #[test]
    fn corre_paquete_trae_oraculos_de_las_tareas_pedidas_y_de_ninguna_otra() {
        let e = Escenario::nuevo();
        let resumen = e
            .empaqueta(&e.pedido(&["tarea-a", "tarea-b"]))
            .expect("empaqueta");
        assert_eq!(resumen.trabajos, 2);
        let extraido = extraer(&resumen.salida);
        assert!(
            extraido
                .path()
                .join("root-home/oraculos/tarea-a/docs/SPEC.md")
                .is_file()
        );
        assert!(
            extraido
                .path()
                .join("root-home/oraculos/tarea-b/docs/SPEC.md")
                .is_file()
        );
        assert!(!extraido.path().join("root-home/oraculos/tarea-c").exists());
        assert!(
            listar(&resumen.salida)
                .iter()
                .all(|r| !r.contains("tarea-c"))
        );
    }

    #[test]
    fn corre_paquete_no_mete_env_ni_credenciales_del_root_home() {
        let e = Escenario::nuevo();
        e.escribe(".env", "DEEPSEEK_API_KEY=falsa-no-usar\n");
        e.escribe(".credentials.json", "{\"token\": \"falso-no-usar\"}\n");
        let resumen = e.empaqueta(&e.pedido(&["tarea-a"])).expect("empaqueta");
        let extraido = extraer(&resumen.salida);
        let texto = texto_del_paquete(extraido.path());
        assert!(!texto.contains("falsa-no-usar"));
        assert!(!texto.contains("falso-no-usar"));
        assert!(archivos_bajo(extraido.path()).iter().all(|r| {
            let nombre = r.rsplit('/').next().unwrap_or(r);
            !nombre.starts_with(".env") && nombre != ".credentials.json"
        }));
    }

    #[test]
    fn corre_paquete_no_trae_nombres_de_claves_de_entorno_como_valor() {
        let e = Escenario::nuevo();
        e.escribe(".env", "ANTHROPIC_API_KEY=falsa-no-usar\n");
        let resumen = e.empaqueta(&e.pedido(&["tarea-a"])).expect("empaqueta");
        let texto = texto_del_paquete(extraer(&resumen.salida).path());
        assert!(!texto.contains("ANTHROPIC_API_KEY"));
        assert!(!texto.contains("DEEPSEEK_API_KEY"));
    }

    #[test]
    fn corre_paquete_frena_con_clave_sk_en_una_tarea() {
        let e = Escenario::nuevo();
        e.oraculo(
            "tarea-a",
            "docs/notas.md",
            "clave sk-abcdefghijklmnopqrstuvwx\n",
        );
        let mensaje = uso(e.empaqueta(&e.pedido(&["tarea-a"])));
        assert!(
            mensaje.contains("oraculos/tarea-a/docs/notas.md"),
            "{mensaje}"
        );
        assert!(
            !mensaje.contains("sk-abcdefghijklmnopqrstuvwx"),
            "{mensaje}"
        );
    }

    #[test]
    fn corre_paquete_frena_con_env_dentro_de_una_tarea() {
        let e = Escenario::nuevo();
        e.oraculo("tarea-a", ".env", "CLAVE=valor-secreto-x\n");
        let mensaje = uso(e.empaqueta(&e.pedido(&["tarea-a"])));
        assert!(mensaje.contains("oraculos/tarea-a/.env"), "{mensaje}");
        assert!(!mensaje.contains("valor-secreto-x"), "{mensaje}");
    }

    #[test]
    fn corre_manifiesto_trae_pasada_commit_y_tareas_en_orden() {
        let e = Escenario::nuevo();
        let resumen = e
            .empaqueta(&e.pedido(&["tarea-b", "tarea-a"]))
            .expect("empaqueta");
        let extraido = extraer(&resumen.salida);
        let m = manifiesto_de(extraido.path());
        assert_eq!(m.v, 1);
        assert_eq!(m.tipo, "corre");
        assert_eq!(m.pasada, 2);
        assert_eq!(m.commit_root, "abc123");
        assert_eq!(m.config, "corrida");
        assert_eq!(m.tareas, vec!["tarea-b".to_string(), "tarea-a".to_string()]);
        assert_eq!(m.binario_sha256, sha256_hex(ELF_X86_64));
        assert!(m.archivos.contains_key("r00t"));
        assert!(!m.archivos.contains_key("manifiesto.json"));
        assert_eq!(m.archivos["r00t"], sha256_hex(ELF_X86_64));
    }

    #[test]
    fn corre_tarea_inexistente_da_uso_con_su_id() {
        let e = Escenario::nuevo();
        let mensaje = uso(e.empaqueta(&e.pedido(&["tarea-a", "no-existe"])));
        assert!(
            mensaje.contains("la tarea no-existe no existe en ROOT_HOME"),
            "{mensaje}"
        );
    }

    #[test]
    fn corre_lista_vacia_da_uso() {
        let e = Escenario::nuevo();
        let mensaje = uso(e.empaqueta(&e.pedido(&[])));
        assert!(mensaje.contains("tareas"), "{mensaje}");
    }

    #[test]
    fn corre_tarea_repetida_da_uso_con_su_id() {
        let e = Escenario::nuevo();
        let mensaje = uso(e.empaqueta(&e.pedido(&["tarea-a", "tarea-a"])));
        assert!(mensaje.contains("tarea-a"), "{mensaje}");
    }

    #[test]
    fn corre_id_de_tarea_invalido_da_uso() {
        let e = Escenario::nuevo();
        let mensaje = uso(e.empaqueta(&e.pedido(&["../tarea-a"])));
        assert!(mensaje.contains("nombre inválido"), "{mensaje}");
    }

    #[test]
    fn corre_config_con_plugin_entra_y_el_manifiesto_lo_mapea() {
        let e = Escenario::nuevo();
        let plugins = tempfile::tempdir().expect("plugins");
        let plugin = plugins.path().join("ecc-plugin");
        fs::create_dir_all(plugin.join("skills")).expect("dir");
        fs::write(plugin.join("plugin.json"), "{\"name\": \"ecc\"}\n").expect("escribe");
        fs::write(plugin.join("skills/x.md"), "# skill\n").expect("escribe");
        e.escribe(
            "configs/con-plugin.toml",
            &format!(
                "roles = [\"construye\"]\n\n[prompt]\nconstruye = \"haz\"\n\n[claude.rol.construye]\nplugin_dir = \"{}\"\n",
                plugin.display()
            ),
        );
        let mut pedido = e.pedido(&["tarea-a"]);
        pedido.config = "con-plugin".into();
        let resumen = e.empaqueta(&pedido).expect("empaqueta");
        let extraido = extraer(&resumen.salida);
        assert!(
            extraido
                .path()
                .join("plugins/0-ecc-plugin/plugin.json")
                .is_file()
        );
        assert!(
            extraido
                .path()
                .join("plugins/0-ecc-plugin/skills/x.md")
                .is_file()
        );
        let m = manifiesto_de(extraido.path());
        assert_eq!(
            m.plugins.get(&plugin.display().to_string()),
            Some(&"plugins/0-ecc-plugin".to_string())
        );
    }

    #[test]
    fn corre_plugin_inexistente_da_uso_con_la_ruta() {
        let e = Escenario::nuevo();
        e.escribe(
            "configs/falta-plugin.toml",
            "roles = [\"construye\"]\n\n[prompt]\nconstruye = \"haz\"\n\n[claude.rol.construye]\nplugin_dir = \"/no/existe/plugin-x\"\n",
        );
        let mut pedido = e.pedido(&["tarea-a"]);
        pedido.config = "falta-plugin".into();
        let mensaje = uso(e.empaqueta(&pedido));
        assert!(mensaje.contains("/no/existe/plugin-x"), "{mensaje}");
    }

    #[test]
    fn corre_versiones_builder_env_trae_las_dos_claves() {
        let e = Escenario::nuevo();
        let resumen = e.empaqueta(&e.pedido(&["tarea-a"])).expect("empaqueta");
        let extraido = extraer(&resumen.salida);
        let texto = fs::read_to_string(extraido.path().join("versiones-builder.env"))
            .expect("lee versiones");
        assert_eq!(texto, "CLAUDE_VERSION=2.1.5\nRUST_VERSION=1.91.0\n");
    }

    #[test]
    fn corre_config_inexistente_da_uso() {
        let e = Escenario::nuevo();
        let mut pedido = e.pedido(&["tarea-a"]);
        pedido.config = "no-hay".into();
        let mensaje = uso(e.empaqueta(&pedido));
        assert!(
            mensaje.contains("no-hay") || mensaje.contains("config"),
            "{mensaje}"
        );
    }

    #[test]
    fn corre_root_toml_con_secreto_frena_sin_mostrar_el_valor() {
        let e = Escenario::nuevo();
        e.escribe("root.toml", "[x]\napi_key = \"clave-falsa-123\"\n");
        let mensaje = uso(e.empaqueta(&e.pedido(&["tarea-a"])));
        assert!(mensaje.contains("root.toml"), "{mensaje}");
        assert!(!mensaje.contains("clave-falsa-123"), "{mensaje}");
    }

    #[test]
    fn corre_enlace_simbolico_en_tools_colab_da_uso_que_nombra_la_ruta() {
        let e = Escenario::nuevo();
        symlink(
            e.raiz.path().join("tools/colab/prepara.py"),
            e.raiz.path().join("tools/colab/enlace.py"),
        )
        .expect("enlace");
        let mensaje = uso(e.empaqueta(&e.pedido(&["tarea-a"])));
        assert!(mensaje.contains("enlace.py"), "{mensaje}");
    }

    #[test]
    fn corre_binario_que_no_es_elf_da_uso() {
        let e = Escenario::nuevo();
        fs::write(&e.binario, b"texto plano, no es un binario").expect("escribe");
        let mensaje = uso(e.empaqueta(&e.pedido(&["tarea-a"])));
        assert!(mensaje.contains("ELF"), "{mensaje}");
    }

    // ----- colab importar --corre -----

    /// Commit de la laptop en los tests de importación. Es el que trae el `Escenario`.
    const COMMIT_LAPTOP: &str = "abc123";

    /// Un momento posterior al corte de S2: los eventos de gasto de los tests cuentan para `gasto_s2`.
    fn ts_de_prueba() -> f64 {
        crate::colab_calibra::corte_s2_epoch(crate::colab_calibra::CORTE_S2).expect("corte")
            + 3600.0
    }

    /// Fila de la vuelta, con contrato 3/4, oculto 5/6, 12.5 minutos y 0.25 USD de API.
    fn fila(id: &str, tarea: &str, estado: crate::tipos::Estado) -> Fila {
        Fila {
            id: id.into(),
            tarea: tarea.into(),
            ts: ts_de_prueba(),
            veredicto: crate::tipos::Veredicto {
                estado,
                contrato: crate::tipos::Conteo { pasa: 3, total: 4 },
                oculto: crate::tipos::Conteo { pasa: 5, total: 6 },
                ..crate::tipos::Veredicto::default()
            },
            resumen: crate::tipos::Resumen {
                usd_api: 0.25,
                minutos: 12.5,
                ..Default::default()
            },
            ..Fila::default()
        }
    }

    /// Llamada de API con costo `costo`, en la corrida `corrida`, `desfase` segundos después del corte.
    fn gasto(corrida: &str, desfase: f64, costo: f64) -> Evento {
        Evento {
            ts: ts_de_prueba() + desfase,
            corrida: corrida.into(),
            tipo: crate::tipos::TipoEvento::Llamada,
            via: Some(crate::tipos::Via::Api),
            cost_usd: costo,
            ..Evento::default()
        }
    }

    /// Evento que no es gasto.
    fn sesion(corrida: &str, desfase: f64) -> Evento {
        Evento {
            ts: ts_de_prueba() + desfase,
            corrida: corrida.into(),
            tipo: crate::tipos::TipoEvento::Sesion,
            ..Evento::default()
        }
    }

    /// `Gate` de cupo agotado antes de un rol, con el motivo que escribe `corre::corrida`.
    fn gate_de_cupo(corrida: &str) -> Evento {
        let mut extra = serde_json::Map::new();
        extra.insert(
            "motivo".into(),
            serde_json::json!("cupo agotado antes del rol construye, vuelta 1: va por api"),
        );
        Evento {
            ts: ts_de_prueba() + 5.0,
            corrida: corrida.into(),
            fase: "rol".into(),
            tipo: crate::tipos::TipoEvento::Gate,
            extra,
            ..Evento::default()
        }
    }

    /// Una línea de JSON por elemento.
    fn lineas<T: Serialize>(items: &[T]) -> String {
        items
            .iter()
            .map(|i| format!("{}\n", serde_json::to_string(i).expect("serializa")))
            .collect()
    }

    /// Arma una vuelta falsa en `dir` con `tar`: `vuelta.json`, `runs.jsonl`, `eventos.jsonl`,
    /// `salida.txt`, los transcripts y los enlaces simbólicos pedidos (nombre, destino).
    fn armar_vuelta(
        dir: &Path,
        nombre: &str,
        meta: &str,
        runs: &str,
        eventos: &str,
        transcripts: &[(&str, &str)],
        enlaces: &[(&str, &str)],
    ) -> PathBuf {
        let raiz = dir.join(format!("stage-{nombre}"));
        fs::create_dir_all(&raiz).expect("dir");
        fs::write(raiz.join("vuelta.json"), meta).expect("escribe");
        fs::write(raiz.join("runs.jsonl"), runs).expect("escribe");
        fs::write(raiz.join("eventos.jsonl"), eventos).expect("escribe");
        fs::write(raiz.join("salida.txt"), "salida\n").expect("escribe");
        for (relativa, texto) in transcripts {
            let ruta = raiz.join("transcripts").join(relativa);
            fs::create_dir_all(ruta.parent().expect("padre")).expect("dir");
            fs::write(ruta, texto).expect("escribe");
        }
        for (nombre_enlace, destino) in enlaces {
            symlink(destino, raiz.join(nombre_enlace)).expect("enlace");
        }
        let salida = dir.join(nombre);
        let estado = Command::new("tar")
            .arg("-czf")
            .arg(&salida)
            .arg("-C")
            .arg(&raiz)
            .arg(".")
            .status()
            .expect("tar");
        assert!(estado.success(), "tar falló");
        salida
    }

    /// Un ROOT_HOME de importación, el paquete de ida de `corre` (tareas a y b, pasada 2) y
    /// una carpeta para las vueltas.
    struct Importa {
        /// Dueño de las carpetas del paquete de ida: vive mientras viva `Importa`.
        _origen: Escenario,
        ida: PathBuf,
        destino: tempfile::TempDir,
        vueltas: tempfile::TempDir,
    }

    impl Importa {
        fn nuevo() -> Self {
            let origen = Escenario::nuevo();
            let ida = origen
                .empaqueta(&origen.pedido(&["tarea-a", "tarea-b"]))
                .expect("empaqueta la ida")
                .salida;
            Importa {
                _origen: origen,
                ida,
                destino: tempfile::tempdir().expect("destino"),
                vueltas: tempfile::tempdir().expect("vueltas"),
            }
        }

        fn home(&self) -> &Path {
            self.destino.path()
        }

        /// Vuelta de `tarea` con las filas, eventos y transcripts dados. `cortada` es el motivo del corte.
        fn vuelta(
            &self,
            nombre: &str,
            tarea: &str,
            cortada: Option<&str>,
            filas: &[Fila],
            eventos: &[Evento],
            transcripts: &[(&str, &str)],
        ) -> PathBuf {
            let meta = serde_json::json!({
                "tarea": tarea,
                "codigo": 0,
                "cortada": cortada,
                "inicio": 1.0,
                "fin": 2.0,
            })
            .to_string();
            armar_vuelta(
                self.vueltas.path(),
                nombre,
                &meta,
                &lineas(filas),
                &lineas(eventos),
                transcripts,
                &[],
            )
        }

        /// Pedido con el SHA real de la vuelta.
        fn pedido_de(&self, vuelta: &Path) -> PedidoImportarCorre {
            PedidoImportarCorre {
                vuelta: vuelta.to_path_buf(),
                sha: sha_de(vuelta),
                ida: self.ida.clone(),
                raiz_root: None,
            }
        }

        fn importa(
            &self,
            pedido: &PedidoImportarCorre,
        ) -> Result<ResumenImportarCorre, ErrorOraculo> {
            importar_corre(self.home(), pedido, COMMIT_LAPTOP)
        }
    }

    /// SHA-256 real del archivo.
    fn sha_de(ruta: &Path) -> String {
        crate::colab_calibra::sha256_hex(&fs::read(ruta).expect("lee la vuelta"))
    }

    /// Todos los archivos bajo `home`, con su contenido. Las carpetas vacías no cuentan.
    fn instantanea(home: &Path) -> BTreeMap<String, Vec<u8>> {
        fn recorre(raiz: &Path, dir: &Path, salida: &mut BTreeMap<String, Vec<u8>>) {
            let Ok(entradas) = fs::read_dir(dir) else {
                return;
            };
            for entrada in entradas {
                let ruta = entrada.expect("entrada").path();
                if ruta.is_dir() {
                    recorre(raiz, &ruta, salida);
                } else {
                    let relativa = ruta.strip_prefix(raiz).expect("prefijo");
                    let clave = relativa.to_string_lossy().into_owned();
                    salida.insert(clave, fs::read(&ruta).expect("lee"));
                }
            }
        }
        let mut salida = BTreeMap::new();
        recorre(home, home, &mut salida);
        salida
    }

    /// Ids de las filas de `runs.jsonl` de `home`. Sin archivo, lista vacía.
    fn ids_de_runs(home: &Path) -> Vec<String> {
        fs::read_to_string(home.join("runs.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str::<Fila>(l).expect("fila").id)
            .collect()
    }

    /// Eventos de `eventos.jsonl` de `home`.
    fn eventos_de(home: &Path) -> Vec<Evento> {
        fs::read_to_string(home.join("eventos.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str::<Evento>(l).expect("evento"))
            .collect()
    }

    /// Registro de `colab-corre.json` de `home`.
    fn registro_de(home: &Path) -> Registro {
        let texto = fs::read_to_string(home.join(REGISTRO_CORRE)).expect("hay registro");
        serde_json::from_str(&texto).expect("registro válido")
    }

    #[test]
    fn importar_corre_vuelta_completa_entra_y_el_registro_la_marca() {
        let i = Importa::nuevo();
        let vuelta = i.vuelta(
            "v1.tar.gz",
            "tarea-a",
            None,
            &[fila("f1", "tarea-a", crate::tipos::Estado::Verde)],
            &[gasto("c1", 10.0, 0.5)],
            &[("f1/1-construye.jsonl", "{}\n")],
        );
        let r = i.importa(&i.pedido_de(&vuelta)).expect("importa");
        assert!(r.entro, "{r:?}");
        assert_eq!(r.motivo, None);
        assert_eq!(r.clase, ClaseCorrida::Completa);
        assert_eq!(r.veredicto.as_deref(), Some("verde"));
        assert_eq!(r.contrato, Some((3, 4)));
        assert_eq!(r.oculto, Some((5, 6)));
        assert_eq!(r.minutos, Some(12.5));
        assert_eq!(ids_de_runs(i.home()), vec!["f1".to_string()]);
        assert!(i.home().join("transcripts/f1/1-construye.jsonl").is_file());
        let registro = registro_de(i.home());
        let entradas = &registro.pasadas["2"]["tarea-a"];
        assert_eq!(entradas.len(), 1, "{entradas:?}");
        assert_eq!(entradas[0].clase, ClaseCorrida::Completa);
        assert_eq!(entradas[0].id, "f1");
        assert_eq!(entradas[0].sha, sha_de(&vuelta));
        let eventos = eventos_de(i.home());
        assert_eq!(eventos.len(), 1, "{eventos:?}");
        assert_eq!(
            eventos[0].extra.get("origen"),
            Some(&serde_json::json!("colab"))
        );
    }

    #[test]
    fn importar_corre_segunda_completa_misma_tarea_y_pasada_no_entra() {
        let i = Importa::nuevo();
        let v1 = i.vuelta(
            "v1.tar.gz",
            "tarea-a",
            None,
            &[fila("f1", "tarea-a", crate::tipos::Estado::Verde)],
            &[],
            &[],
        );
        i.importa(&i.pedido_de(&v1)).expect("la primera entra");
        let antes = instantanea(i.home());
        let v2 = i.vuelta(
            "v2.tar.gz",
            "tarea-a",
            None,
            &[fila("f2", "tarea-a", crate::tipos::Estado::Rojo)],
            &[gasto("c2", 20.0, 0.1)],
            &[],
        );
        let r = i.importa(&i.pedido_de(&v2)).expect("la segunda no falla");
        assert!(!r.entro, "{r:?}");
        assert_eq!(r.motivo.as_deref(), Some("ya hay una corrida completa"));
        assert_eq!(r.clase, ClaseCorrida::Completa);
        assert_eq!(ids_de_runs(i.home()), vec!["f1".to_string()]);
        assert_eq!(instantanea(i.home()), antes, "ROOT_HOME no cambia");
    }

    #[test]
    fn importar_corre_cortada_y_despues_completa_entran_las_dos_y_solo_cuenta_la_completa() {
        let i = Importa::nuevo();
        let cortada = i.vuelta(
            "c.tar.gz",
            "tarea-a",
            Some("plazo"),
            &[],
            &[gasto("c1", 10.0, 0.5), sesion("c1", 11.0)],
            &[("c1/1-construye.jsonl", "{}\n")],
        );
        let r1 = i.importa(&i.pedido_de(&cortada)).expect("la cortada");
        assert!(r1.entro, "{r1:?}");
        assert_eq!(r1.clase, ClaseCorrida::Cortada);
        assert_eq!(r1.veredicto, None);
        assert!(
            !i.home().join("transcripts").exists(),
            "una cortada no copia transcripts"
        );
        let completa = i.vuelta(
            "f.tar.gz",
            "tarea-a",
            None,
            &[fila("f1", "tarea-a", crate::tipos::Estado::Verde)],
            &[gasto("c2", 20.0, 0.25), sesion("c2", 21.0)],
            &[("f1/1-construye.jsonl", "{}\n")],
        );
        let r2 = i.importa(&i.pedido_de(&completa)).expect("la completa");
        assert!(r2.entro, "{r2:?}");
        assert_eq!(r2.clase, ClaseCorrida::Completa);
        assert_eq!(ids_de_runs(i.home()), vec!["f1".to_string()]);
        let registro = registro_de(i.home());
        let clases: Vec<ClaseCorrida> = registro.pasadas["2"]["tarea-a"]
            .iter()
            .map(|e| e.clase)
            .collect();
        assert_eq!(clases, vec![ClaseCorrida::Cortada, ClaseCorrida::Completa]);
        // La cortada trae solo su gasto; la completa trae sus dos eventos.
        assert_eq!(eventos_de(i.home()).len(), 3);
    }

    #[test]
    fn importar_corre_gasto_de_una_cortada_entra_y_sube_el_gasto_s2() {
        let i = Importa::nuevo();
        let antes = crate::colab_calibra::gasto_s2(i.home()).expect("gasto antes");
        let cortada = i.vuelta(
            "c.tar.gz",
            "tarea-a",
            Some("plazo"),
            &[],
            &[gasto("c1", 10.0, 1.5)],
            &[],
        );
        let r = i.importa(&i.pedido_de(&cortada)).expect("importa");
        assert!(r.entro, "{r:?}");
        assert_eq!(r.clase, ClaseCorrida::Cortada);
        assert!((r.gasto_usd - 1.5).abs() < 1e-9, "{}", r.gasto_usd);
        let despues = crate::colab_calibra::gasto_s2(i.home()).expect("gasto después");
        assert!(
            (despues.desde_corte_usd - antes.desde_corte_usd - 1.5).abs() < 1e-9,
            "{despues:?}"
        );
        assert!((despues.total_usd - antes.total_usd - 1.5).abs() < 1e-9);
    }

    #[test]
    fn importar_corre_sha_distinto_no_entra_y_no_escribe_nada() {
        let i = Importa::nuevo();
        let vuelta = i.vuelta(
            "v.tar.gz",
            "tarea-a",
            None,
            &[fila("f1", "tarea-a", crate::tipos::Estado::Verde)],
            &[gasto("c1", 10.0, 0.5)],
            &[("f1/1-construye.jsonl", "{}\n")],
        );
        let antes = instantanea(i.home());
        let otro = "0".repeat(64);
        let pedido = PedidoImportarCorre {
            sha: otro.clone(),
            ..i.pedido_de(&vuelta)
        };
        match i.importa(&pedido) {
            Err(ErrorOraculo::Infra(mensaje)) => {
                assert!(mensaje.contains("no se importó nada"), "{mensaje}");
                assert!(mensaje.contains(&otro), "{mensaje}");
            }
            otro => panic!("se esperaba Infra, llegó {otro:?}"),
        }
        assert_eq!(instantanea(i.home()), antes);
        let pedido = PedidoImportarCorre {
            sha: "no-hex".into(),
            ..i.pedido_de(&vuelta)
        };
        assert!(matches!(i.importa(&pedido), Err(ErrorOraculo::Uso(_))));
        assert_eq!(instantanea(i.home()), antes);
    }

    #[test]
    fn importar_corre_tarea_fuera_del_manifiesto_no_entra() {
        let i = Importa::nuevo();
        let vuelta = i.vuelta(
            "v.tar.gz",
            "tarea-c",
            None,
            &[fila("f1", "tarea-c", crate::tipos::Estado::Verde)],
            &[],
            &[],
        );
        let antes = instantanea(i.home());
        match i.importa(&i.pedido_de(&vuelta)) {
            Err(ErrorOraculo::Uso(mensaje)) => assert!(mensaje.contains("tarea-c"), "{mensaje}"),
            otro => panic!("se esperaba Uso, llegó {otro:?}"),
        }
        assert_eq!(instantanea(i.home()), antes);
    }

    #[test]
    fn importar_corre_la_misma_vuelta_dos_veces_ya_estaba() {
        let i = Importa::nuevo();
        let vuelta = i.vuelta(
            "v.tar.gz",
            "tarea-a",
            None,
            &[fila("f1", "tarea-a", crate::tipos::Estado::Verde)],
            &[gasto("c1", 10.0, 0.5)],
            &[],
        );
        let pedido = i.pedido_de(&vuelta);
        assert!(i.importa(&pedido).expect("la primera").entro);
        let antes = instantanea(i.home());
        let r = i.importa(&pedido).expect("la segunda no falla");
        assert!(!r.entro, "{r:?}");
        assert_eq!(r.motivo.as_deref(), Some("ya estaba"));
        assert_eq!(instantanea(i.home()), antes);
    }

    #[test]
    fn importar_corre_commit_distinto_no_entra() {
        let i = Importa::nuevo();
        let vuelta = i.vuelta(
            "v.tar.gz",
            "tarea-a",
            None,
            &[fila("f1", "tarea-a", crate::tipos::Estado::Verde)],
            &[],
            &[],
        );
        let antes = instantanea(i.home());
        match importar_corre(i.home(), &i.pedido_de(&vuelta), "otro999") {
            Err(ErrorOraculo::Uso(mensaje)) => {
                assert!(mensaje.contains(COMMIT_LAPTOP), "{mensaje}");
                assert!(mensaje.contains("otro999"), "{mensaje}");
            }
            otro => panic!("se esperaba Uso, llegó {otro:?}"),
        }
        assert_eq!(instantanea(i.home()), antes);
    }

    #[test]
    fn importar_corre_no_imprime_nombres_de_tests() {
        let i = Importa::nuevo();
        let mut f = fila("f1", "tarea-a", crate::tipos::Estado::Rojo);
        f.veredicto.inestables = vec!["oculto:test_secreto_zeta_42".into()];
        let vuelta = i.vuelta("v.tar.gz", "tarea-a", None, &[f], &[], &[]);
        let r = i.importa(&i.pedido_de(&vuelta)).expect("importa");
        let texto = serde_json::to_string(&r).expect("json");
        assert!(!texto.contains("test_secreto_zeta_42"), "{texto}");
        assert!(!texto.contains("oculto:"), "{texto}");
        assert!(!texto.contains("oculto/"), "{texto}");
    }

    #[test]
    fn importar_corre_gate_de_cupo_hace_cortada_aunque_haya_fila() {
        let i = Importa::nuevo();
        let vuelta = i.vuelta(
            "g.tar.gz",
            "tarea-a",
            None,
            &[fila("f1", "tarea-a", crate::tipos::Estado::Verde)],
            &[gate_de_cupo("c1")],
            &[("f1/1-construye.jsonl", "{}\n")],
        );
        let r = i.importa(&i.pedido_de(&vuelta)).expect("importa");
        assert!(r.entro, "{r:?}");
        assert_eq!(r.clase, ClaseCorrida::Cortada);
        assert_eq!(r.veredicto.as_deref(), Some("verde"));
        assert!(ids_de_runs(i.home()).is_empty());
        assert!(!i.home().join("transcripts").exists());
    }

    #[test]
    fn importar_corre_vuelta_con_enlace_simbolico_da_uso() {
        let i = Importa::nuevo();
        let meta =
            serde_json::json!({"tarea": "tarea-a", "codigo": 0, "cortada": null}).to_string();
        let vuelta = armar_vuelta(
            i.vueltas.path(),
            "e.tar.gz",
            &meta,
            "",
            "",
            &[],
            &[("liga", "/etc/hosts")],
        );
        match i.importa(&i.pedido_de(&vuelta)) {
            Err(ErrorOraculo::Uso(mensaje)) => assert!(mensaje.contains("liga"), "{mensaje}"),
            otro => panic!("se esperaba Uso, llegó {otro:?}"),
        }
        assert!(instantanea(i.home()).is_empty());
    }
}

/// Tests de los guiones de Colab: `corre-runtime.py` y `preparar-corre.sh`.
/// Usan Python y bash reales, con programas falsos. Ningún test toca /run, /usr ni /root del host.
#[cfg(test)]
mod runtime {
    use crate::prueba_ejecutable::escribir_ejecutable;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};

    const CORRE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tools/colab/corre-runtime.py");
    const PREPARAR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tools/colab/preparar-corre.sh");

    /// Valores falsos. Solo llegan al hijo por `Command::env`, nunca al shell del test.
    const CLAVE: &str = "sk-falso-clave-0001";
    const CUPO: &str = "tok-falso-cupo-0002";

    /// r00t falso. `REGISTRO` y `COMPORTAMIENTO` se cambian antes de escribirlo.
    /// Anota la tarea (`$3`, después de `corre --tarea`) y deja una fila en runs y eventos.
    const R00T_BASE: &str = r#"#!/bin/sh
tarea="$3"
echo "$tarea" >> 'REGISTRO'
mkdir -p "$ROOT_HOME/transcripts/run-$tarea"
echo "turno de $tarea" > "$ROOT_HOME/transcripts/run-$tarea/turno.txt"
echo "{\"tarea\":\"$tarea\"}" >> "$ROOT_HOME/runs.jsonl"
echo "{\"evento\":\"$tarea\"}" >> "$ROOT_HOME/eventos.jsonl"
COMPORTAMIENTO
"#;

    const VERSIONES_OK: &str = "CLAUDE_VERSION=2.1.5\nRUST_VERSION=1.91.0\n";

    /// Id de imagen falso que trae el manifiesto de prueba.
    const IMAGEN_ID: &str = "sha256:imagen-prueba";

    fn bash_disponible() -> bool {
        Command::new("bash")
            .arg("-c")
            .arg("exit 0")
            .status()
            .is_ok_and(|s| s.success())
    }

    fn python_disponible() -> bool {
        Command::new("python3")
            .arg("-c")
            .arg("pass")
            .status()
            .is_ok_and(|s| s.success())
    }

    /// Falla con la causa clara si el guion no existe.
    fn falta_guion(ruta: &str) {
        assert!(Path::new(ruta).is_file(), "falta el guion {ruta}");
    }

    fn texto(salida: &Output) -> String {
        format!(
            "{}{}",
            String::from_utf8_lossy(&salida.stdout),
            String::from_utf8_lossy(&salida.stderr)
        )
    }

    /// Lee todos los archivos de un árbol y los une en un texto.
    fn texto_de_arbol(dir: &Path) -> String {
        let mut salida = String::new();
        for entrada in fs::read_dir(dir).expect("lee carpeta") {
            let ruta = entrada.expect("entrada").path();
            if ruta.is_dir() {
                salida.push_str(&texto_de_arbol(&ruta));
            } else {
                salida.push_str(&fs::read_to_string(&ruta).unwrap_or_default());
            }
        }
        salida
    }

    /// Paquete falso, ROOT_HOME falso, token en un temporal, archivo del token original (con
    /// `CUPO`) y r00t falso.
    struct Corrida {
        raiz: tempfile::TempDir,
        paquete: PathBuf,
        home: PathBuf,
        token: PathBuf,
        archivo: PathBuf,
        r00t: PathBuf,
        fuera: PathBuf,
    }

    impl Corrida {
        fn nueva(tareas: &[&str]) -> Self {
            let raiz = tempfile::tempdir().expect("raíz temporal");
            let c = Corrida {
                paquete: raiz.path().join("paquete"),
                home: raiz.path().join("home"),
                token: raiz.path().join("token"),
                archivo: raiz.path().join("content-root-token"),
                r00t: raiz.path().join("r00t"),
                fuera: raiz.path().join("fuera"),
                raiz,
            };
            for dir in [&c.paquete, &c.home, &c.fuera] {
                fs::create_dir_all(dir).expect("crea carpeta");
            }
            fs::write(&c.archivo, CUPO).expect("escribe el archivo del token");
            let lista: Vec<String> = tareas.iter().map(|t| format!("\"{t}\"")).collect();
            let manifiesto = format!(
                "{{\"v\":1,\"tipo\":\"corre\",\"pasada\":1,\"config\":\"cfg\",\"imagen_id\":\"{IMAGEN_ID}\",\"tareas\":[{}],\"plugins\":{{}},\"archivos\":{{}}}}\n",
                lista.join(",")
            );
            fs::write(c.paquete.join("manifiesto.json"), manifiesto).expect("escribe manifiesto");
            c
        }

        /// Escribe el r00t falso. `comportamiento` es código de shell que corre tras anotar la tarea.
        fn r00t_falso(&self, comportamiento: &str) {
            let texto = R00T_BASE
                .replace(
                    "REGISTRO",
                    &self.fuera.join("registro").display().to_string(),
                )
                .replace("COMPORTAMIENTO", comportamiento);
            escribir_ejecutable(&self.r00t, &texto);
        }

        /// Tareas que el r00t falso recibió, en orden.
        fn registro(&self) -> Vec<String> {
            fs::read_to_string(self.fuera.join("registro"))
                .unwrap_or_default()
                .lines()
                .map(String::from)
                .collect()
        }

        fn correr(&self, plazo: &str) -> Output {
            self.correr_con(plazo, &[])
        }

        /// Corre el guion. `extra` se aplica al final y puede pisar cualquier variable.
        fn correr_con(&self, plazo: &str, extra: &[(&str, &str)]) -> Output {
            Command::new("python3")
                .arg("-I")
                .arg(CORRE)
                .arg(&self.paquete)
                .current_dir(self.raiz.path())
                .env("ROOT_CORRE_R00T", &self.r00t)
                .env("ROOT_CORRE_HOME", &self.home)
                .env("ROOT_CORRE_TOKEN", &self.token)
                .env("ROOT_CORRE_PLAZO_S", plazo)
                .env("ROOT_TOKEN_ARCHIVO", &self.archivo)
                .env("DEEPSEEK_API_KEY", CLAVE)
                .env_remove("ROOT_TOKEN_CUPO")
                .envs(extra.iter().copied())
                .output()
                .expect("lanza python3")
        }

        fn escribir_hecho(&self, tarea: &str, codigo: i64) {
            let dir = self.paquete.join("hechos");
            fs::create_dir_all(&dir).expect("crea hechos");
            let texto = format!(
                "{{\"codigo\":{codigo},\"sha\":\"previo\",\"inicio\":\"previo\",\"fin\":\"previo\"}}"
            );
            fs::write(dir.join(format!("{tarea}.json")), texto).expect("escribe hecho");
        }

        fn estado(&self) -> serde_json::Value {
            let texto = fs::read_to_string(self.paquete.join("estado.json")).expect("estado.json");
            serde_json::from_str(&texto).expect("estado.json es JSON")
        }

        fn hecho(&self, tarea: &str) -> serde_json::Value {
            let texto = fs::read_to_string(self.paquete.join(format!("hechos/{tarea}.json")))
                .expect("hecho");
            serde_json::from_str(&texto).expect("hecho es JSON")
        }

        fn vuelta(&self, tarea: &str) -> PathBuf {
            self.paquete.join(format!("vuelta-{tarea}.tar.gz"))
        }

        /// Extrae la vuelta de una tarea en una carpeta del temporal y devuelve su ruta.
        fn extraer(&self, tarea: &str) -> PathBuf {
            let destino = self.raiz.path().join(format!("extraida-{tarea}"));
            fs::create_dir_all(&destino).expect("crea destino");
            let estado = Command::new("tar")
                .arg("-xzf")
                .arg(self.vuelta(tarea))
                .arg("-C")
                .arg(&destino)
                .status()
                .expect("lanza tar");
            assert!(estado.success(), "tar no abre la vuelta de {tarea}");
            destino
        }
    }

    #[test]
    fn corre_runtime_pasa_al_r00t_el_entorno_y_el_id_de_la_imagen() {
        if !python_disponible() {
            eprintln!(
                "sin python3: se saltea corre_runtime_pasa_al_r00t_el_entorno_y_el_id_de_la_imagen"
            );
            return;
        }
        falta_guion(CORRE);
        let c = Corrida::nueva(&["t1"]);
        c.r00t_falso(&format!(
            "printf '%s\\n%s\\n' \"$ROOT_BWRAP_ENTORNO\" \"$ROOT_BWRAP_IMAGEN_ID\" > '{}'",
            c.fuera.join("vars").display()
        ));
        let salida = c.correr("7200");
        assert!(salida.status.success(), "{}", texto(&salida));
        let vars = fs::read_to_string(c.fuera.join("vars")).expect("vars");
        let esperado = format!("{}\n{IMAGEN_ID}\n", c.paquete.join("entorno.env").display());
        assert_eq!(vars, esperado, "el r00t debe recibir la ruta y el id");
    }

    #[test]
    fn corre_runtime_sin_imagen_id_en_el_manifiesto_falla_sin_correr() {
        if !python_disponible() {
            eprintln!(
                "sin python3: se saltea corre_runtime_sin_imagen_id_en_el_manifiesto_falla_sin_correr"
            );
            return;
        }
        falta_guion(CORRE);
        let c = Corrida::nueva(&["t1"]);
        fs::write(
            c.paquete.join("manifiesto.json"),
            "{\"v\":1,\"tipo\":\"corre\",\"pasada\":1,\"config\":\"cfg\",\"tareas\":[\"t1\"],\"plugins\":{},\"archivos\":{}}\n",
        )
        .expect("escribe manifiesto");
        c.r00t_falso("");
        let salida = c.correr("7200");
        assert!(!salida.status.success(), "debe fallar sin imagen_id");
        let estado = c.estado();
        let error = estado["error"].as_str().unwrap_or_default();
        assert!(error.contains("imagen_id"), "{error}");
        assert!(c.registro().is_empty(), "no debe correr ningún r00t");
    }

    #[test]
    fn corre_runtime_salta_tareas_ya_hechas() {
        if !python_disponible() {
            eprintln!("sin python3: se saltea corre_runtime_salta_tareas_ya_hechas");
            return;
        }
        falta_guion(CORRE);
        let c = Corrida::nueva(&["t1", "t2", "t3"]);
        c.r00t_falso("");
        c.escribir_hecho("t2", 0);
        let salida = c.correr("7200");
        assert!(salida.status.success(), "{}", texto(&salida));
        assert_eq!(c.registro(), vec!["t1", "t3"]);
    }

    #[test]
    fn corre_runtime_sigue_si_r00t_sale_con_1() {
        if !python_disponible() {
            eprintln!("sin python3: se saltea corre_runtime_sigue_si_r00t_sale_con_1");
            return;
        }
        falta_guion(CORRE);
        let c = Corrida::nueva(&["t1", "t2"]);
        c.r00t_falso("if [ \"$tarea\" = t1 ]; then exit 1; fi");
        let salida = c.correr("7200");
        assert!(salida.status.success(), "{}", texto(&salida));
        assert!(c.vuelta("t1").is_file(), "falta vuelta-t1.tar.gz");
        assert_eq!(c.hecho("t1")["codigo"], 1);
        assert_eq!(c.registro(), vec!["t1", "t2"], "debe seguir con t2");
    }

    #[test]
    fn corre_runtime_no_filtra_clave_ni_token() {
        if !python_disponible() {
            eprintln!("sin python3: se saltea corre_runtime_no_filtra_clave_ni_token");
            return;
        }
        falta_guion(CORRE);
        let c = Corrida::nueva(&["t1", "t2"]);
        // El r00t falso imprime su entorno y el token a propósito. La salida debe ocultarlos.
        c.r00t_falso(&format!(
            "printf '%s' \"$DEEPSEEK_API_KEY\" > '{clave}'\n\
             env > '{entorno}'\n\
             cat \"$ROOT_TOKEN_BUILDER\"\n\
             env",
            clave = c.fuera.join("clave").display(),
            entorno = c.fuera.join("entorno").display(),
        ));
        let salida = c.correr("7200");
        assert!(salida.status.success(), "{}", texto(&salida));

        let dicho = texto(&salida);
        assert!(!dicho.contains(CLAVE), "la salida trae la clave");
        assert!(!dicho.contains(CUPO), "la salida trae el token");

        let estado = fs::read_to_string(c.paquete.join("estado.json")).expect("estado.json");
        assert!(
            !estado.contains(CLAVE) && !estado.contains(CUPO),
            "estado.json con secretos"
        );

        for tarea in ["t1", "t2"] {
            let vuelta = texto_de_arbol(&c.extraer(tarea));
            assert!(
                !vuelta.contains(CLAVE),
                "la vuelta de {tarea} trae la clave"
            );
            assert!(!vuelta.contains(CUPO), "la vuelta de {tarea} trae el token");
            assert!(
                vuelta.contains("[oculto]"),
                "la vuelta de {tarea} debe marcar lo oculto"
            );
        }

        // La clave sí llegó al hijo. Lo anotó el r00t falso fuera de la vuelta.
        let recibida = fs::read_to_string(c.fuera.join("clave")).expect("clave recibida");
        assert_eq!(recibida, CLAVE, "el hijo debe recibir la clave");
        let entorno = fs::read_to_string(c.fuera.join("entorno")).expect("entorno");
        assert!(
            !entorno.contains("ROOT_TOKEN_CUPO="),
            "el hijo no debe recibir ROOT_TOKEN_CUPO"
        );
        assert!(
            entorno.contains("ROOT_TOKEN_BUILDER="),
            "el hijo debe recibir la ruta del token"
        );
        assert!(
            !entorno.contains(CUPO),
            "el hijo no debe recibir el valor del token"
        );
    }

    #[test]
    fn corre_runtime_borra_el_token_al_terminar() {
        if !python_disponible() {
            eprintln!("sin python3: se saltea corre_runtime_borra_el_token_al_terminar");
            return;
        }
        falta_guion(CORRE);
        // Caso 1: r00t sale con 0. El token existe con modo 0600 durante la corrida.
        let c = Corrida::nueva(&["t1"]);
        c.r00t_falso(&format!(
            "stat -c %a \"$ROOT_TOKEN_BUILDER\" > '{}'",
            c.fuera.join("modo").display()
        ));
        let salida = c.correr("7200");
        assert!(salida.status.success(), "{}", texto(&salida));
        let modo = fs::read_to_string(c.fuera.join("modo")).expect("modo");
        assert_eq!(
            modo.trim(),
            "600",
            "el token debe ser 0600 durante la corrida"
        );
        assert!(!c.token.exists(), "el token sigue después de la corrida");

        // Caso 2: r00t sale con 1. El token también se borra.
        let c = Corrida::nueva(&["t1"]);
        c.r00t_falso(&format!(
            "stat -c %a \"$ROOT_TOKEN_BUILDER\" > '{}'; exit 1",
            c.fuera.join("modo").display()
        ));
        let salida = c.correr("7200");
        assert!(salida.status.success(), "{}", texto(&salida));
        assert!(!c.token.exists(), "el token sigue tras un r00t que falló");
    }

    #[test]
    fn corre_runtime_corta_por_plazo() {
        if !python_disponible() {
            eprintln!("sin python3: se saltea corre_runtime_corta_por_plazo");
            return;
        }
        falta_guion(CORRE);
        let c = Corrida::nueva(&["t1"]);
        c.r00t_falso("sleep 30");
        let salida = c.correr("1");
        assert!(salida.status.success(), "{}", texto(&salida));
        assert!(c.vuelta("t1").is_file(), "falta vuelta-t1.tar.gz");
        let dir = c.extraer("t1");
        let meta: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(dir.join("vuelta.json")).expect("vuelta.json"),
        )
        .expect("vuelta.json es JSON");
        assert_eq!(meta["cortada"], "plazo");
        assert_eq!(meta["codigo"], 124);
    }

    #[test]
    fn corre_runtime_estado_final() {
        if !python_disponible() {
            eprintln!("sin python3: se saltea corre_runtime_estado_final");
            return;
        }
        falta_guion(CORRE);
        let c = Corrida::nueva(&["t1", "t2"]);
        fs::write(
            c.home.join("runs.jsonl"),
            "{\"previo\":1}\n{\"previo\":2}\n",
        )
        .expect("runs previo");
        c.r00t_falso("");
        let salida = c.correr("7200");
        assert!(salida.status.success(), "{}", texto(&salida));

        let estado = c.estado();
        assert_eq!(estado["fin"], true);
        assert_eq!(estado["hechas"], serde_json::json!(["t1", "t2"]));
        assert_eq!(estado["faltan"], serde_json::json!([]));

        for tarea in ["t1", "t2"] {
            let dir = c.extraer(tarea);
            let runs = fs::read_to_string(dir.join("runs.jsonl")).expect("runs.jsonl");
            let filas: Vec<&str> = runs.lines().collect();
            assert_eq!(
                filas,
                vec![format!("{{\"tarea\":\"{tarea}\"}}")],
                "solo la fila nueva"
            );
        }
    }

    #[test]
    fn corre_runtime_mueve_el_token_del_archivo_y_borra_el_original() {
        if !python_disponible() {
            eprintln!(
                "sin python3: se saltea corre_runtime_mueve_el_token_del_archivo_y_borra_el_original"
            );
            return;
        }
        falta_guion(CORRE);
        let c = Corrida::nueva(&["t1"]);
        c.r00t_falso("");
        let salida = c.correr("7200");
        assert!(salida.status.success(), "{}", texto(&salida));
        assert_eq!(c.registro(), vec!["t1"], "debe correr la tarea");
        assert!(
            !c.archivo.exists(),
            "el archivo original debe borrarse al moverlo"
        );
    }

    #[test]
    fn corre_runtime_token_nuevo_es_0600_con_el_contenido_del_archivo() {
        if !python_disponible() {
            eprintln!(
                "sin python3: se saltea corre_runtime_token_nuevo_es_0600_con_el_contenido_del_archivo"
            );
            return;
        }
        falta_guion(CORRE);
        let c = Corrida::nueva(&["t1"]);
        c.r00t_falso(&format!(
            "stat -c %a \"$ROOT_TOKEN_BUILDER\" > '{modo}'\ncat \"$ROOT_TOKEN_BUILDER\" > '{contenido}'",
            modo = c.fuera.join("modo").display(),
            contenido = c.fuera.join("contenido").display(),
        ));
        let salida = c.correr("7200");
        assert!(salida.status.success(), "{}", texto(&salida));
        let modo = fs::read_to_string(c.fuera.join("modo")).expect("modo");
        assert_eq!(modo.trim(), "600", "el token nuevo debe ser 0600");
        let contenido = fs::read_to_string(c.fuera.join("contenido")).expect("contenido");
        assert_eq!(
            contenido, CUPO,
            "el token nuevo debe tener el contenido del archivo"
        );
    }

    #[test]
    fn corre_runtime_rechaza_root_token_cupo_sin_correr_ni_mostrar_el_valor() {
        if !python_disponible() {
            eprintln!(
                "sin python3: se saltea corre_runtime_rechaza_root_token_cupo_sin_correr_ni_mostrar_el_valor"
            );
            return;
        }
        falta_guion(CORRE);
        // Con valor y vacía: basta que la variable esté definida.
        for valor in [CUPO, ""] {
            let c = Corrida::nueva(&["t1"]);
            c.r00t_falso("");
            let salida = c.correr_con("7200", &[("ROOT_TOKEN_CUPO", valor)]);
            let dicho = texto(&salida);
            assert_eq!(salida.status.code(), Some(2), "debe salir con 2: {dicho}");
            assert!(
                dicho.contains("ROOT_TOKEN_CUPO"),
                "el error debe nombrar la variable: {dicho}"
            );
            assert!(
                !dicho.contains(CUPO),
                "la salida trae el valor del token: {dicho}"
            );
            assert!(c.registro().is_empty(), "no debe correr ningún r00t");
        }
    }

    #[test]
    fn corre_runtime_sin_archivo_o_vacio_no_corre_nada() {
        if !python_disponible() {
            eprintln!("sin python3: se saltea corre_runtime_sin_archivo_o_vacio_no_corre_nada");
            return;
        }
        falta_guion(CORRE);
        for vacio in [false, true] {
            let c = Corrida::nueva(&["t1"]);
            if vacio {
                fs::write(&c.archivo, "").expect("vacía el archivo");
            } else {
                fs::remove_file(&c.archivo).expect("quita el archivo");
            }
            c.r00t_falso("");
            let salida = c.correr("7200");
            let dicho = texto(&salida);
            assert_eq!(salida.status.code(), Some(2), "vacio={vacio}: {dicho}");
            assert!(
                dicho.contains(&c.archivo.display().to_string()),
                "el error debe nombrar la ruta del archivo: {dicho}"
            );
            assert!(
                c.registro().is_empty(),
                "vacio={vacio}: no debe correr ningún r00t"
            );
        }
    }

    #[test]
    fn corre_runtime_si_el_destino_falla_conserva_el_original() {
        if !python_disponible() {
            eprintln!(
                "sin python3: se saltea corre_runtime_si_el_destino_falla_conserva_el_original"
            );
            return;
        }
        falta_guion(CORRE);
        let c = Corrida::nueva(&["t1"]);
        c.r00t_falso("");
        // La carpeta del destino no existe: crear el token debe fallar.
        let destino = c.raiz.path().join("no-existe").join("token");
        let destino_txt = destino.display().to_string();
        let salida = c.correr_con("7200", &[("ROOT_CORRE_TOKEN", &destino_txt)]);
        let dicho = texto(&salida);
        assert!(!salida.status.success(), "debe fallar: {dicho}");
        assert!(
            dicho.contains(&destino_txt),
            "el error debe nombrar el destino: {dicho}"
        );
        assert!(
            c.archivo.is_file(),
            "no debe borrar el original si el destino falla"
        );
        assert!(c.registro().is_empty(), "no debe correr ningún r00t");
    }

    #[test]
    fn corre_runtime_rechaza_el_mismo_archivo_como_origen_y_destino() {
        if !python_disponible() {
            eprintln!(
                "sin python3: se saltea corre_runtime_rechaza_el_mismo_archivo_como_origen_y_destino"
            );
            return;
        }
        falta_guion(CORRE);
        let c = Corrida::nueva(&["t1"]);
        c.r00t_falso("");
        let mismo = c.archivo.display().to_string();
        let salida = c.correr_con("7200", &[("ROOT_CORRE_TOKEN", &mismo)]);
        assert_eq!(salida.status.code(), Some(2), "{}", texto(&salida));
        assert!(c.archivo.is_file(), "no debe borrar el token");
        assert!(c.registro().is_empty(), "no debe correr ningún r00t");
    }

    /// Paquete de `preparar-corre.sh` con programas falsos en PATH y rutas dentro de un temporal.
    struct Prep {
        raiz: tempfile::TempDir,
        paquete: PathBuf,
        falsos: PathBuf,
        bin: PathBuf,
        destino: PathBuf,
        root: PathBuf,
        prefijo: PathBuf,
        red: PathBuf,
        runtime: PathBuf,
    }

    impl Prep {
        /// `claude_dice` es la salida falsa de `claude --version`. `versiones` es versiones-builder.env.
        fn nuevo(claude_dice: &str, versiones: &str) -> Self {
            let raiz = tempfile::tempdir().expect("raíz temporal");
            let p = Prep {
                paquete: raiz.path().join("paquete"),
                falsos: raiz.path().join("falsos"),
                bin: raiz.path().join("bin"),
                destino: raiz.path().join("destino/root-home"),
                root: raiz.path().join("root"),
                prefijo: raiz.path().join("prefijo"),
                red: raiz.path().join("red-builder.sh"),
                runtime: raiz.path().join("preparar-runtime.sh"),
                raiz,
            };
            for dir in [&p.paquete, &p.falsos, &p.root, &p.prefijo] {
                fs::create_dir_all(dir).expect("crea carpeta");
            }
            fs::write(p.paquete.join("versiones-builder.env"), versiones).expect("versiones");
            fs::write(
                p.paquete.join("manifiesto.json"),
                "{\"v\":1,\"tipo\":\"corre\",\"pasada\":1,\"config\":\"cfg\",\"tareas\":[\"tarea-a\"],\
                 \"plugins\":{\"/opt/ecc-falso\":\"plugins/ecc\"},\"archivos\":{}}\n",
            )
            .expect("manifiesto");
            fs::create_dir_all(p.paquete.join("root-home/oraculos/tarea-a/docs"))
                .expect("root-home");
            fs::write(
                p.paquete.join("root-home/oraculos/tarea-a/docs/SPEC.md"),
                "spec\n",
            )
            .expect("spec");
            fs::create_dir_all(p.paquete.join("plugins/ecc")).expect("plugin");
            fs::write(p.paquete.join("plugins/ecc/x.md"), "plugin de prueba\n").expect("plugin");
            fs::write(p.paquete.join("r00t"), "binario falso\n").expect("r00t");

            escribir_ejecutable(&p.runtime, "#!/bin/sh\necho \"runtime $1\"\n");
            escribir_ejecutable(
                &p.falsos.join("apt-get"),
                "#!/bin/sh\necho \"apt-get $*\"\n",
            );
            escribir_ejecutable(&p.falsos.join("npm"), "#!/bin/sh\necho \"npm $*\"\n");
            escribir_ejecutable(
                &p.falsos.join("useradd"),
                "#!/bin/sh\necho \"useradd $*\"\n",
            );
            escribir_ejecutable(&p.falsos.join("chown"), "#!/bin/sh\nexit 0\n");
            escribir_ejecutable(&p.falsos.join("id"), "#!/bin/sh\nexit 1\n");
            escribir_ejecutable(
                &p.falsos.join("claude"),
                &format!("#!/bin/sh\necho '{claude_dice}'\n"),
            );
            escribir_ejecutable(
                &p.falsos.join("rustc"),
                "#!/bin/sh\necho 'rustc 1.91.0 (abc 2026-01-01)'\n",
            );
            p
        }

        fn correr(&self) -> Output {
            let path = format!(
                "{}:{}",
                self.falsos.display(),
                std::env::var("PATH").unwrap_or_default()
            );
            Command::new("bash")
                .arg(PREPARAR)
                .arg(&self.paquete)
                .current_dir(self.raiz.path())
                .env("PATH", path)
                .env("ROOT_PREPARAR_RUNTIME", &self.runtime)
                .env("ROOT_RED_BUILDER_SH", &self.red)
                .env("ROOT_BIN_DIR", &self.bin)
                .env("ROOT_HOME_DESTINO", &self.destino)
                .env("ROOT_DIR_RAIZ", &self.root)
                .env("ROOT_RAIZ_DIR", &self.prefijo)
                .env("ROOT_APT_ESPERA", "0")
                .output()
                .expect("lanza bash")
        }
    }

    fn modo(ruta: &Path) -> u32 {
        fs::metadata(ruta).expect("metadatos").permissions().mode() & 0o777
    }

    #[test]
    fn preparar_corre_frena_si_claude_no_coincide() {
        if !bash_disponible() {
            eprintln!("sin bash: se saltea preparar_corre_frena_si_claude_no_coincide");
            return;
        }
        falta_guion(PREPARAR);
        let p = Prep::nuevo("9.9.9 (Claude Code)", VERSIONES_OK);
        let salida = p.correr();
        assert!(!salida.status.success(), "debe salir distinto de 0");
        let dicho = texto(&salida);
        assert!(dicho.contains("mal versiones"), "{dicho}");
        assert!(
            dicho.contains("9.9.9"),
            "el mensaje nombra la versión real: {dicho}"
        );
        assert!(
            dicho.contains("2.1.5"),
            "el mensaje nombra la versión fijada: {dicho}"
        );
    }

    #[test]
    fn preparar_corre_sin_red_builder_falla() {
        if !bash_disponible() {
            eprintln!("sin bash: se saltea preparar_corre_sin_red_builder_falla");
            return;
        }
        falta_guion(PREPARAR);
        let p = Prep::nuevo("2.1.5 (Claude Code)", VERSIONES_OK);
        let salida = p.correr();
        assert!(!salida.status.success(), "debe salir distinto de 0");
        let dicho = texto(&salida);
        assert!(dicho.contains("mal red-builder.sh"), "{dicho}");
        assert!(dicho.contains("Tarea 171"), "{dicho}");
    }

    #[test]
    fn preparar_corre_camino_feliz() {
        if !bash_disponible() || !python_disponible() {
            eprintln!("sin bash o python3: se saltea preparar_corre_camino_feliz");
            return;
        }
        falta_guion(PREPARAR);
        let p = Prep::nuevo("2.1.5 (Claude Code)", VERSIONES_OK);
        escribir_ejecutable(&p.red, "#!/bin/sh\necho red-ok\n");
        let salida = p.correr();
        assert!(salida.status.success(), "{}", texto(&salida));

        // El root-home se mueve: no queda copia en el paquete.
        assert!(
            !p.paquete.join("root-home").exists(),
            "queda root-home en el paquete"
        );
        assert!(p.destino.join("oraculos/tarea-a/docs/SPEC.md").is_file());
        assert_eq!(modo(&p.destino), 0o700);
        assert_eq!(modo(&p.root), 0o700);
        assert_eq!(modo(&p.paquete), 0o700);

        // El r00t queda instalado y ejecutable.
        assert!(p.bin.join("r00t").is_file());
        assert_eq!(modo(&p.bin.join("r00t")), 0o755);

        // El plugin vuelve a su ruta original, dentro del prefijo de prueba.
        let original = fs::read(p.paquete.join("plugins/ecc/x.md")).expect("plugin en paquete");
        let copia = fs::read(p.prefijo.join("opt/ecc-falso/x.md")).expect("plugin copiado");
        assert_eq!(copia, original);
    }
}
