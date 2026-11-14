//! Paquete de calibración para Colab: `r00t colab empaquetar`.
//!
//! El paquete lleva el binario, las herramientas de la imagen, las tareas, sus cachés y un
//! manifiesto con el orden de cada trabajo. Nada del ROOT_HOME sale sin pasar por las reglas de
//! exclusión y de secretos.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::borrado;
use crate::config::root_home;
use crate::oracle::ErrorOraculo;
use crate::oracle::calibra::{
    self, Calibraciones, CasoGuardado, Fila, hash_de_tarea, huella_de, tiene_items_de_juez,
};
use crate::oracle::contenedor::correr_programa;
use crate::oracle::descarga::cache_de;
use crate::oracle::tarea::{Tarea, cargar_en};
use crate::tipos::Dimension;

/// Variable que el paquete fija para elegir el modo de contenedor del trabajo.
/// Mismo nombre y valor que define la Tarea 139 en contenedor.rs. Al encadenar, el orquestador deja una sola definición.
pub const ROOT_CONTENEDOR_MODO: &str = "ROOT_CONTENEDOR_MODO";
/// Valor de `ROOT_CONTENEDOR_MODO` para correr con bwrap.
/// Mismo nombre y valor que define la Tarea 139 en contenedor.rs. Al encadenar, el orquestador deja una sola definición.
pub const MODO_BWRAP: &str = "bwrap";
/// Variable que apunta al archivo de entorno del paquete.
/// Mismo nombre y valor que define la Tarea 139 en contenedor.rs. Al encadenar, el orquestador deja una sola definición.
pub const ROOT_BWRAP_ENTORNO: &str = "ROOT_BWRAP_ENTORNO";
/// Variable que lleva el id de la imagen del trabajo.
/// Mismo nombre y valor que define la Tarea 139 en contenedor.rs. Al encadenar, el orquestador deja una sola definición.
pub const ROOT_BWRAP_IMAGEN_ID: &str = "ROOT_BWRAP_IMAGEN_ID";

/// Variable que cambia el binario de podman. Sin ella, se usa `podman`.
pub const ROOT_PODMAN_BIN: &str = "ROOT_PODMAN_BIN";

/// Herramientas que el paquete copia desde `/usr/local/bin` de la imagen.
const HERRAMIENTAS: [&str; 5] = [
    "cargo-audit",
    "cargo-mutants",
    "cargo-nextest",
    "gitleaks",
    "hadolint",
];

/// Dimensiones que Colab calibra. Las demás se rechazan.
const DIMENSIONES_COLAB: [Dimension; 4] = [
    Dimension::Seguridad,
    Dimension::Funciona,
    Dimension::Completitud,
    Dimension::Pruebas,
];

/// Segmentos que marcan un nombre como secreto, en minúsculas.
const SEGMENTOS_SECRETOS: [&str; 9] = [
    "key",
    "apikey",
    "token",
    "secret",
    "password",
    "passwd",
    "clave",
    "credencial",
    "credential",
];

/// Nombres que nunca entran al paquete, con el motivo.
pub const EXCLUIDOS: &[(&str, &str)] = &[
    ("eventos.jsonl", "registro de eventos local"),
    ("runs.jsonl", "registro de corridas local"),
    ("transcripts", "transcripciones de sesiones"),
    ("evidencia", "evidencia de corridas locales"),
    ("colab", "estado de runtimes de Colab (activo.json)"),
];

/// Tiempo máximo de cada llamada a podman.
const PLAZO_PODMAN: Duration = Duration::from_secs(300);

/// Acceso a la imagen de la tarea. Los tests usan un doble. La implementación real llama a podman.
pub trait Imagen {
    /// Copia `/usr/local/bin/<nombre>` de la imagen a `destino`.
    fn extraer_herramienta(
        &self,
        imagen: &str,
        nombre: &str,
        destino: &Path,
    ) -> Result<(), ErrorOraculo>;
    /// Salida de `rustc --version` dentro de la imagen.
    fn version_rustc(&self, imagen: &str) -> Result<String, ErrorOraculo>;
    /// Variables de entorno de la imagen, como líneas `CLAVE=valor`.
    fn entorno(&self, imagen: &str) -> Result<Vec<String>, ErrorOraculo>;
    /// Id de la imagen.
    fn id(&self, imagen: &str) -> Result<String, ErrorOraculo>;
}

/// Imagen leída con podman real, por vectores de argumentos.
pub struct PodmanReal {
    pub binario: PathBuf,
}

impl PodmanReal {
    /// Corre podman con `args` y devuelve su salida. Un código distinto de cero es error de infraestructura.
    fn correr(&self, args: &[&str]) -> Result<String, ErrorOraculo> {
        let programa = self.binario.to_str().ok_or_else(|| {
            ErrorOraculo::Infra(format!(
                "ruta de podman no UTF-8: {}",
                self.binario.display()
            ))
        })?;
        let salida = correr_programa(programa, args, PLAZO_PODMAN)?;
        if salida.codigo != 0 {
            return Err(ErrorOraculo::Infra(format!(
                "podman {} falló: {}",
                args.join(" "),
                salida.stderr.trim()
            )));
        }
        Ok(salida.stdout)
    }
}

impl Imagen for PodmanReal {
    fn extraer_herramienta(
        &self,
        imagen: &str,
        nombre: &str,
        destino: &Path,
    ) -> Result<(), ErrorOraculo> {
        let creado = self.correr(&["create", imagen])?;
        let contenedor = creado.trim();
        let origen = format!("{contenedor}:/usr/local/bin/{nombre}");
        let copia = match destino.to_str() {
            Some(texto) => self.correr(&["cp", origen.as_str(), texto]),
            None => Err(ErrorOraculo::Infra(format!(
                "ruta no UTF-8: {}",
                destino.display()
            ))),
        };
        // Borra el contenedor temporal aunque la copia falle.
        let _ = self.correr(&["rm", "-f", contenedor]);
        copia.map(|_| ())
    }

    fn version_rustc(&self, imagen: &str) -> Result<String, ErrorOraculo> {
        let salida = self.correr(&["run", "--rm", imagen, "rustc", "--version"])?;
        Ok(salida.trim().to_string())
    }

    fn entorno(&self, imagen: &str) -> Result<Vec<String>, ErrorOraculo> {
        let json = self.correr(&[
            "image",
            "inspect",
            "--format",
            "{{json .Config.Env}}",
            imagen,
        ])?;
        serde_json::from_str(json.trim())
            .map_err(|e| ErrorOraculo::Infra(format!("entorno de la imagen ilegible: {e}")))
    }

    fn id(&self, imagen: &str) -> Result<String, ErrorOraculo> {
        let salida = self.correr(&["image", "inspect", "--format", "{{.Id}}", imagen])?;
        Ok(salida.trim().to_string())
    }
}

/// Un trabajo de calibración: una tarea, una dimensión y sus comandos de prueba.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trabajo {
    pub tarea: String,
    pub dimension: Dimension,
    pub comandos: Vec<String>,
}

/// Datos ya leídos que arman el paquete. Los tests los llenan a mano.
#[derive(Debug, Clone)]
pub struct Paquete {
    pub home: PathBuf,
    pub raiz_root: PathBuf,
    pub binario: PathBuf,
    pub trabajos: Vec<(String, Dimension)>,
    pub repos: BTreeMap<String, PathBuf>,
    pub rustup_sha256: String,
    pub salida: PathBuf,
    pub commit_root: String,
}

/// Pedido tal como llega de la línea de comandos.
#[derive(Debug, Clone)]
pub struct PedidoEmpaquetar {
    pub binario: PathBuf,
    pub trabajos: Vec<String>,
    pub repos: Vec<String>,
    pub rustup_sha256: String,
    pub salida: PathBuf,
    pub raiz_root: Option<PathBuf>,
}

/// Lo que devuelve el comando: dónde quedó el paquete y qué trae.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resumen {
    pub salida: PathBuf,
    pub archivos: usize,
    pub trabajos: usize,
}

/// Manifiesto del paquete. Lo lee quien corre los trabajos en Colab.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifiesto {
    pub v: u32,
    pub commit_root: String,
    pub binario_sha256: String,
    pub archivos: BTreeMap<String, String>,
    pub trabajos: Vec<TrabajoManifiesto>,
    pub imagenes: BTreeMap<String, String>,
}

/// Un trabajo dentro del manifiesto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrabajoManifiesto {
    pub tarea: String,
    pub dimension: String,
    pub huella: String,
    pub tarea_hash: String,
    pub imagen_id: String,
    pub comando: String,
    pub args: Vec<String>,
    pub entorno: BTreeMap<String, String>,
}

/// Datos de un trabajo que el manifiesto necesita.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatoTrabajo {
    pub tarea: String,
    pub dimension: Dimension,
    pub tarea_hash: String,
    pub imagen_id: String,
}

/// Datos que arman el manifiesto, sin tocar el disco.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatosManifiesto {
    pub commit_root: String,
    pub binario_sha256: String,
    pub trabajos: Vec<DatoTrabajo>,
    pub imagenes: BTreeMap<String, String>,
}

/// Orden de un trabajo: el comando, sus argumentos y su entorno.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Orden {
    pub comando: String,
    pub args: Vec<String>,
    pub entorno: BTreeMap<String, String>,
}

/// Punto de entrada del comando. Lee ROOT_HOME, git y podman, y arma el paquete.
pub fn empaquetar_cli(pedido: PedidoEmpaquetar) -> Result<Resumen, ErrorOraculo> {
    let trabajos = parsear_trabajos(&pedido.trabajos)?;
    let repos = parsear_repos(&pedido.repos)?;
    let raiz_root = match pedido.raiz_root {
        Some(raiz) => raiz,
        None => std::env::current_dir()
            .map_err(|e| ErrorOraculo::Infra(format!("directorio actual: {e}")))?,
    };
    let commit_root = commit_de(&raiz_root)?;
    let paquete = Paquete {
        home: root_home(),
        raiz_root,
        binario: pedido.binario,
        trabajos,
        repos,
        rustup_sha256: pedido.rustup_sha256,
        salida: pedido.salida,
        commit_root,
    };
    let imagen = PodmanReal {
        binario: binario_podman(),
    };
    empaquetar(&paquete, &imagen)
}

/// Arma el paquete con los datos dados y la imagen dada.
pub fn empaquetar(paquete: &Paquete, imagen: &dyn Imagen) -> Result<Resumen, ErrorOraculo> {
    if !es_sha256_hex(&paquete.rustup_sha256) {
        return Err(ErrorOraculo::Uso(
            "--rustup-sha256 debe tener 64 caracteres hexadecimales".into(),
        ));
    }
    let bytes = fs::read(&paquete.binario)
        .map_err(|e| ErrorOraculo::Infra(format!("leer {}: {e}", paquete.binario.display())))?;
    if !es_elf_x86_64(&bytes) {
        return Err(ErrorOraculo::Uso(format!(
            "{} no es un ELF x86-64",
            paquete.binario.display()
        )));
    }
    if paquete.trabajos.is_empty() {
        return Err(ErrorOraculo::Uso(
            "hace falta al menos un trabajo en --trabajos".into(),
        ));
    }

    let mut tareas: BTreeMap<String, Tarea> = BTreeMap::new();
    let mut trabajos = Vec::new();
    for (nombre, dimension) in &paquete.trabajos {
        if !tareas.contains_key(nombre) {
            let tarea = cargar_en(&paquete.home, nombre)?;
            tareas.insert(nombre.clone(), tarea);
        }
        let tarea = &tareas[nombre];
        trabajos.push(Trabajo {
            tarea: nombre.clone(),
            dimension: *dimension,
            comandos: vec![tarea.contrato.cmd.clone(), tarea.oculto.cmd.clone()],
        });
    }
    validar_trabajos(&trabajos)?;

    for nombre in tareas.keys() {
        if !paquete.repos.contains_key(nombre) {
            return Err(ErrorOraculo::Uso(format!(
                "falta --repo para la tarea {nombre}"
            )));
        }
    }
    let imagenes: BTreeSet<&str> = tareas.values().map(|t| t.imagen.as_str()).collect();
    if imagenes.len() > 1 {
        return Err(ErrorOraculo::Uso(
            "las tareas usan imágenes distintas; el paquete lleva una sola imagen".into(),
        ));
    }
    let Some(nombre_imagen) = imagenes.iter().next() else {
        return Err(ErrorOraculo::Uso("no hay imagen de tarea".into()));
    };

    if !paquete.raiz_root.join("tools/colab").is_dir() {
        return Err(ErrorOraculo::Uso(format!(
            "falta tools/colab/ en {} (lo trae la Tarea 140)",
            paquete.raiz_root.display()
        )));
    }
    let root_toml = paquete.home.join("root.toml");
    if root_toml.is_file() {
        let texto = fs::read_to_string(&root_toml)
            .map_err(|e| ErrorOraculo::Infra(format!("leer root.toml: {e}")))?;
        revisar_root_toml(&texto).map_err(ErrorOraculo::Uso)?;
    }

    // La carpeta de trabajo vive en ROOT_HOME/tmp y se borra al salir, también ante error.
    let tmp = paquete.home.join("tmp");
    fs::create_dir_all(&tmp)
        .map_err(|e| ErrorOraculo::Infra(format!("crear {}: {e}", tmp.display())))?;
    let staging = tmp.join(format!("colab-empaquetar-{}", std::process::id()));
    borrado::borrar_dir_dentro(&tmp, &staging)
        .map_err(|e| ErrorOraculo::Infra(format!("limpiar {}: {e}", staging.display())))?;
    fs::create_dir_all(&staging)
        .map_err(|e| ErrorOraculo::Infra(format!("crear {}: {e}", staging.display())))?;
    let resultado = armar_y_empaquetar(paquete, &staging, &bytes, &tareas, imagen, nombre_imagen);
    let limpieza = borrado::borrar_dir_dentro(&tmp, &staging);
    let resumen = resultado?;
    limpieza.map_err(|e| ErrorOraculo::Infra(format!("borrar la carpeta de trabajo: {e}")))?;
    Ok(resumen)
}

/// Llena la carpeta de trabajo y la empaqueta con `tar`.
fn armar_y_empaquetar(
    paquete: &Paquete,
    staging: &Path,
    bytes: &[u8],
    tareas: &BTreeMap<String, Tarea>,
    imagen: &dyn Imagen,
    nombre_imagen: &str,
) -> Result<Resumen, ErrorOraculo> {
    let id = imagen.id(nombre_imagen)?;

    escribir(&staging.join("r00t"), bytes)?;
    fijar_ejecutable(&staging.join("r00t"))?;
    let binario_sha256 = sha256_hex(bytes);

    let carpeta = staging.join("herramientas");
    fs::create_dir_all(&carpeta)
        .map_err(|e| ErrorOraculo::Infra(format!("crear {}: {e}", carpeta.display())))?;
    let mut hashes = Vec::new();
    for nombre in HERRAMIENTAS {
        let destino = carpeta.join(nombre);
        imagen.extraer_herramienta(nombre_imagen, nombre, &destino)?;
        fijar_ejecutable(&destino)?;
        let contenido = fs::read(&destino)
            .map_err(|e| ErrorOraculo::Infra(format!("leer {}: {e}", destino.display())))?;
        hashes.push((nombre.to_string(), sha256_hex(&contenido)));
    }

    let salida_rustc = imagen.version_rustc(nombre_imagen)?;
    let rust = version_de_rustc(&salida_rustc).ok_or_else(|| {
        ErrorOraculo::Infra(format!(
            "salida de rustc no reconocida: {}",
            salida_rustc.trim()
        ))
    })?;
    escribir(
        &staging.join("versiones.env"),
        versiones_env(&rust, &paquete.rustup_sha256, &hashes).as_bytes(),
    )?;
    let entorno = filtrar_entorno(&imagen.entorno(nombre_imagen)?);
    escribir(
        &staging.join("entorno.env"),
        lineas_texto(&entorno).as_bytes(),
    )?;

    copiar_dir(
        &paquete.raiz_root.join("tools/colab"),
        &staging.join("tools/colab"),
        &[],
    )?;
    for nombre in tareas.keys() {
        copiar_dir(
            &paquete.home.join("oraculos").join(nombre),
            &staging.join("oraculos").join(nombre),
            &[],
        )?;
    }
    for (nombre, tarea) in tareas {
        let repo = &paquete.repos[nombre];
        if let Some(cache) = cache_de(&paquete.home, &tarea.lenguaje, repo, &tarea.imagen) {
            let relativa = cache.strip_prefix(&paquete.home).map_err(|_| {
                ErrorOraculo::Infra(format!("la caché {} no está en ROOT_HOME", cache.display()))
            })?;
            copiar_dir(&cache, &staging.join(relativa), &[])?;
        }
    }
    let rustsec = paquete.home.join("cache/rustsec");
    if rustsec.is_dir() {
        copiar_dir(&rustsec, &staging.join("cache/rustsec"), &[])?;
    }
    copiar_archivo_si_existe(
        &paquete.home.join("calibracion.json"),
        &staging.join("calibracion.json"),
    )?;
    let configs = paquete.home.join("configs");
    if configs.is_dir() {
        copiar_dir(&configs, &staging.join("configs"), &[])?;
    }
    copiar_archivo_si_existe(&paquete.home.join("root.toml"), &staging.join("root.toml"))?;
    for nombre in tareas.keys() {
        copiar_dir(
            &paquete.repos[nombre],
            &staging.join("repos").join(nombre),
            &[".git", "target"],
        )?;
    }

    // El manifiesto se escribe al final: lista todos los archivos anteriores.
    let archivos =
        archivos_de(staging).map_err(|e| ErrorOraculo::Infra(format!("leer el paquete: {e}")))?;
    // Cuenta también `manifiesto.json`, que todavía no existe.
    let contados = archivos.len() + 1;
    let mut trabajos = Vec::new();
    for (nombre, dimension) in &paquete.trabajos {
        let tarea = &tareas[nombre];
        trabajos.push(DatoTrabajo {
            tarea: nombre.clone(),
            dimension: *dimension,
            tarea_hash: hash_de_tarea(tarea)?,
            imagen_id: id.clone(),
        });
    }
    let imagenes: BTreeMap<String, String> =
        tareas.keys().map(|n| (n.clone(), id.clone())).collect();
    let datos = DatosManifiesto {
        commit_root: paquete.commit_root.clone(),
        binario_sha256,
        trabajos,
        imagenes,
    };
    let manifiesto = armar_manifiesto(&datos, archivos);
    let json = serde_json::to_string_pretty(&manifiesto)
        .map_err(|e| ErrorOraculo::Infra(format!("serializar el manifiesto: {e}")))?;
    escribir(
        &staging.join("manifiesto.json"),
        format!("{json}\n").as_bytes(),
    )?;

    // `tar` recibe la salida absoluta: con `-C`, una ruta relativa se resolvería mal.
    let salida = std::path::absolute(&paquete.salida)
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
    Ok(Resumen {
        salida,
        archivos: contados,
        trabajos: paquete.trabajos.len(),
    })
}

/// Revisa los trabajos: dimensión calibrable y comandos sin `pytest`.
pub fn validar_trabajos(trabajos: &[Trabajo]) -> Result<(), ErrorOraculo> {
    for trabajo in trabajos {
        let dimension = trabajo.dimension;
        let quien = format!("tarea {}, dimensión {}", trabajo.tarea, dimension.clave());
        if dimension == Dimension::Eficiencia {
            return Err(ErrorOraculo::Uso(format!(
                "{quien}: Eficiencia necesita el cgroup del contenedor"
            )));
        }
        if tiene_items_de_juez(dimension) {
            return Err(ErrorOraculo::Uso(format!(
                "{quien}: el juez necesita Colab Secrets, que `exec` no lee"
            )));
        }
        if !DIMENSIONES_COLAB.contains(&dimension) {
            return Err(ErrorOraculo::Uso(format!(
                "{quien}: dimensión no calibrable en Colab"
            )));
        }
        if trabajo.comandos.iter().any(|c| c.contains("pytest")) {
            return Err(ErrorOraculo::Uso(format!(
                "tarea {}: Colab usa otro Python, no pytest",
                trabajo.tarea
            )));
        }
    }
    Ok(())
}

/// Dice si un nombre de clave o variable es secreto.
pub fn es_nombre_secreto(nombre: &str) -> bool {
    let minusculas = nombre.to_lowercase();
    // Un nombre `*_env` guarda el nombre de una variable, no su valor.
    if minusculas.ends_with("_env") {
        return false;
    }
    minusculas
        .split(['_', '-', '.'])
        .any(|segmento| SEGMENTOS_SECRETOS.contains(&segmento))
}

/// Deja las líneas `CLAVE=valor` cuyo nombre no es secreto.
pub fn filtrar_entorno(lineas: &[String]) -> Vec<String> {
    lineas
        .iter()
        .filter(|linea| {
            linea
                .split_once('=')
                .is_some_and(|(clave, _)| !es_nombre_secreto(clave))
        })
        .cloned()
        .collect()
}

/// Revisa el texto de `root.toml`. Falla si una clave secreta trae un valor.
/// El error nombra la clave y nunca muestra el valor.
pub fn revisar_root_toml(texto: &str) -> Result<(), String> {
    // El error del parser se descarta: puede mostrar un trozo del texto con secretos.
    let tabla: toml::Table = texto
        .parse()
        .map_err(|_| "root.toml no es TOML válido".to_string())?;
    match buscar_secreto(&tabla, "") {
        Some(ruta) => Err(format!(
            "root.toml: {ruta} trae un valor secreto; quítalo antes de empaquetar"
        )),
        None => Ok(()),
    }
}

/// Busca una clave secreta con valor. Devuelve su ubicación, como `[tabla] clave`.
fn buscar_secreto(tabla: &toml::Table, camino: &str) -> Option<String> {
    for (clave, valor) in tabla.iter() {
        match valor {
            toml::Value::Table(sub) => {
                let nuevo = if camino.is_empty() {
                    clave.clone()
                } else {
                    format!("{camino}.{clave}")
                };
                if let Some(ruta) = buscar_secreto(sub, &nuevo) {
                    return Some(ruta);
                }
            }
            toml::Value::String(texto) if es_nombre_secreto(clave) && !texto.is_empty() => {
                return Some(if camino.is_empty() {
                    clave.clone()
                } else {
                    format!("[{camino}] {clave}")
                });
            }
            _ => {}
        }
    }
    None
}

/// Arma el manifiesto a partir de los datos y los hashes de archivos.
pub fn armar_manifiesto(datos: &DatosManifiesto, archivos: BTreeMap<String, String>) -> Manifiesto {
    let trabajos = datos
        .trabajos
        .iter()
        .map(|t| {
            let orden = comando_de_trabajo(&t.tarea, t.dimension, &t.imagen_id);
            TrabajoManifiesto {
                tarea: t.tarea.clone(),
                dimension: t.dimension.clave().to_string(),
                huella: huella_de(t.dimension),
                tarea_hash: t.tarea_hash.clone(),
                imagen_id: t.imagen_id.clone(),
                comando: orden.comando,
                args: orden.args,
                entorno: orden.entorno,
            }
        })
        .collect();
    Manifiesto {
        v: 1,
        commit_root: datos.commit_root.clone(),
        binario_sha256: datos.binario_sha256.clone(),
        archivos,
        trabajos,
        imagenes: datos.imagenes.clone(),
    }
}

/// SHA-256 en hexadecimal.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Dice si los bytes son un ELF de 64 bits, little endian, para x86-64.
pub fn es_elf_x86_64(bytes: &[u8]) -> bool {
    bytes.len() >= 20
        && bytes.starts_with(b"\x7fELF")
        && bytes[4] == 2
        && bytes[5] == 1
        && u16::from_le_bytes([bytes[18], bytes[19]]) == 0x3e
}

/// Texto de `versiones.env` con sus tres líneas.
pub fn versiones_env(rust: &str, rustup_sha256: &str, herramientas: &[(String, String)]) -> String {
    let lista = herramientas
        .iter()
        .map(|(nombre, hash)| format!("{nombre}:{hash}"))
        .collect::<Vec<_>>()
        .join(",");
    format!("RUST_VERSION={rust}\nRUSTUP_SHA256={rustup_sha256}\nHERRAMIENTAS_SHA256={lista}\n")
}

/// Versión de `rustc --version`: el segundo campo de la salida.
pub fn version_de_rustc(salida: &str) -> Option<String> {
    let mut campos = salida.split_whitespace();
    if campos.next()? != "rustc" {
        return None;
    }
    campos.next().map(String::from)
}

/// Orden de un trabajo: comando, argumentos y entorno.
pub fn comando_de_trabajo(tarea: &str, dimension: Dimension, imagen_id: &str) -> Orden {
    let repo = format!("repos/{tarea}");
    let args: Vec<String> = [
        "oraculo",
        "calibrar",
        "--tarea",
        tarea,
        "--dimension",
        dimension.clave(),
        "--repo",
        repo.as_str(),
        "--json",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let comando = format!("r00t {}", args.join(" "));
    let entorno = BTreeMap::from([
        (ROOT_CONTENEDOR_MODO.to_string(), MODO_BWRAP.to_string()),
        (ROOT_BWRAP_ENTORNO.to_string(), "entorno.env".to_string()),
        (ROOT_BWRAP_IMAGEN_ID.to_string(), imagen_id.to_string()),
    ]);
    Orden {
        comando,
        args,
        entorno,
    }
}

/// Lee `tarea:dimension` de cada trabajo.
pub fn parsear_trabajos(entradas: &[String]) -> Result<Vec<(String, Dimension)>, ErrorOraculo> {
    entradas
        .iter()
        .map(|entrada| {
            let (tarea, clave) = entrada.split_once(':').ok_or_else(|| {
                ErrorOraculo::Uso(format!(
                    "trabajo mal formado: {entrada} (se espera tarea:dimension)"
                ))
            })?;
            let dimension = Dimension::desde_clave(clave)
                .ok_or_else(|| ErrorOraculo::Uso(format!("dimensión desconocida: {clave}")))?;
            Ok((tarea.to_string(), dimension))
        })
        .collect()
}

/// Lee `tarea=ruta` de cada repo.
pub fn parsear_repos(entradas: &[String]) -> Result<BTreeMap<String, PathBuf>, ErrorOraculo> {
    let mut repos = BTreeMap::new();
    for entrada in entradas {
        let (tarea, ruta) = entrada.split_once('=').ok_or_else(|| {
            ErrorOraculo::Uso(format!(
                "repo mal formado: {entrada} (se espera tarea=ruta)"
            ))
        })?;
        if repos
            .insert(tarea.to_string(), PathBuf::from(ruta))
            .is_some()
        {
            return Err(ErrorOraculo::Uso(format!(
                "repo repetido para la tarea {tarea}"
            )));
        }
    }
    Ok(repos)
}

/// Dice si el texto son 64 caracteres hexadecimales.
pub fn es_sha256_hex(texto: &str) -> bool {
    texto.len() == 64 && texto.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Ruta del binario de podman: la variable `ROOT_PODMAN_BIN`, o `podman`.
fn binario_podman() -> PathBuf {
    std::env::var_os(ROOT_PODMAN_BIN)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("podman"))
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

/// Copia el contenido de `origen` a `destino`. Omite los nombres de `EXCLUIDOS` y los de `excluir`.
/// En archivos, omite los `.env*` y los nombres secretos. Los enlaces simbólicos no se copian.
fn copiar_dir(origen: &Path, destino: &Path, excluir: &[&str]) -> Result<(), ErrorOraculo> {
    let error = |e: io::Error| ErrorOraculo::Infra(format!("copiar {}: {e}", origen.display()));
    fs::create_dir_all(destino)
        .map_err(|e| ErrorOraculo::Infra(format!("crear {}: {e}", destino.display())))?;
    for entrada in fs::read_dir(origen).map_err(error)? {
        let entrada = entrada.map_err(error)?;
        let nombre_os = entrada.file_name();
        let nombre = nombre_os.to_string_lossy();
        if EXCLUIDOS.iter().any(|(n, _)| *n == &*nombre) || excluir.contains(&&*nombre) {
            continue;
        }
        let tipo = entrada.file_type().map_err(error)?;
        let ruta = entrada.path();
        let destino_entrada = destino.join(&*nombre);
        if tipo.is_dir() {
            copiar_dir(&ruta, &destino_entrada, excluir)?;
        } else if tipo.is_file() && !(nombre.starts_with(".env") || es_nombre_secreto(&nombre)) {
            fs::copy(&ruta, &destino_entrada)
                .map_err(|e| ErrorOraculo::Infra(format!("copiar {}: {e}", ruta.display())))?;
        }
    }
    Ok(())
}

/// Copia un archivo si existe. Si no existe, no hace nada.
fn copiar_archivo_si_existe(origen: &Path, destino: &Path) -> Result<(), ErrorOraculo> {
    if !origen.is_file() {
        return Ok(());
    }
    fs::copy(origen, destino)
        .map(|_| ())
        .map_err(|e| ErrorOraculo::Infra(format!("copiar {}: {e}", origen.display())))
}

/// Escribe `bytes` en `ruta`.
fn escribir(ruta: &Path, bytes: &[u8]) -> Result<(), ErrorOraculo> {
    fs::write(ruta, bytes)
        .map_err(|e| ErrorOraculo::Infra(format!("escribir {}: {e}", ruta.display())))
}

/// Pone permisos 0755 al archivo.
fn fijar_ejecutable(ruta: &Path) -> Result<(), ErrorOraculo> {
    fs::set_permissions(ruta, fs::Permissions::from_mode(0o755))
        .map_err(|e| ErrorOraculo::Infra(format!("permisos de {}: {e}", ruta.display())))
}

/// Une las líneas con salto de línea final en cada una.
fn lineas_texto(lineas: &[String]) -> String {
    lineas.iter().map(|l| format!("{l}\n")).collect()
}

/// SHA-256 de cada archivo bajo `raiz`, con la ruta relativa y `/` como separador.
fn archivos_de(raiz: &Path) -> io::Result<BTreeMap<String, String>> {
    let mut mapa = BTreeMap::new();
    recorrer(raiz, raiz, &mut mapa)?;
    Ok(mapa)
}

fn recorrer(raiz: &Path, dir: &Path, mapa: &mut BTreeMap<String, String>) -> io::Result<()> {
    for entrada in fs::read_dir(dir)? {
        let ruta = entrada?.path();
        if ruta.is_dir() {
            recorrer(raiz, &ruta, mapa)?;
        } else if ruta.is_file() {
            let relativa = ruta.strip_prefix(raiz).map_err(io::Error::other)?;
            let clave = relativa
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            mapa.insert(clave, sha256_hex(&fs::read(&ruta)?));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Importar la vuelta de Colab: `r00t colab importar`.
// ---------------------------------------------------------------------------

/// Tolerancia al comparar dos `valor_base`.
const TOLERANCIA_VALOR: f32 = 1e-6;
/// Carpeta de los casos dentro del paquete de vuelta y dentro de ROOT_HOME.
const CARPETA_CALIBRACION: &str = "calibracion";
/// Nombre del manifiesto dentro del paquete de vuelta.
const MANIFIESTO_VUELTA: &str = "manifiesto.json";
/// Aviso fijo: la comparación no mira el estado de los mutantes de la base.
const AVISO_MUTANTES: &str = "Pruebas: solo se comparó el valor de la base; el estado de los mutantes de la base no está guardado.";

/// Manifiesto del paquete de vuelta. Lo escribe Colab al terminar la calibración.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifiestoVuelta {
    pub v: u32,
    pub commit_root: String,
    /// SHA-256 de cada archivo del paquete, salvo `manifiesto.json`. Clave: ruta relativa con `/`.
    pub archivos: BTreeMap<String, String>,
    pub filas: Vec<FilaVuelta>,
    pub donde: DondeSeMidio,
}

/// Una fila del paquete de vuelta y los nombres de sus `.casos.jsonl`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilaVuelta {
    pub tarea: String,
    /// Clave de la dimensión, en minúsculas.
    pub dimension: String,
    pub ts: f64,
    /// Nombres de archivo dentro de `calibracion/` que son de esta fila.
    #[serde(default)]
    pub casos: Vec<String>,
}

/// Dónde se midió la calibración. No cambia la fila.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DondeSeMidio {
    pub lugar: String,
    pub runtime: String,
}

/// Pedido de `colab importar` tal como llega de la línea de comandos.
#[derive(Debug, Clone)]
pub struct PedidoImportar {
    pub archivo: PathBuf,
    pub raiz_root: Option<PathBuf>,
}

/// Una fila que no entró, con su motivo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Rechazada {
    pub tarea: String,
    pub dimension: String,
    pub motivo: String,
}

/// Lo que devuelve `colab importar`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResumenImportar {
    pub entraron: usize,
    pub rechazadas: Vec<Rechazada>,
    pub casos_copiados: usize,
    /// Carpeta del respaldo. `None` si no se escribió nada.
    pub respaldo: Option<PathBuf>,
    pub avisos: Vec<String>,
}

/// Decisión sobre una fila del paquete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Entra,
    NoEntra(String),
}

/// Valores distintos de `valor_base` de cada lado de la comparación.
#[derive(Debug, Clone, PartialEq)]
pub struct Valores {
    pub colab: Vec<f32>,
    pub laptop: Vec<f32>,
}

/// Datos que la decisión necesita de una fila. Todo ya leído: la decisión no toca el disco.
pub struct Entrada<'a> {
    pub fila: &'a Fila,
    /// Huella que la laptop tiene para la dimensión de la fila.
    pub huella_laptop: &'a str,
    /// Hash del `tarea.toml` de la laptop. `None` si la tarea no carga en la laptop.
    pub tarea_hash_laptop: Option<&'a str>,
    /// Si la laptop ya tiene una fila con la misma tarea, dimensión y `ts`.
    pub ya_estaba: bool,
    /// `None`: la laptop no tiene fila para comparar. `Err`: faltan casos de esos lados.
    pub base: Option<Result<Valores, Vec<&'static str>>>,
}

/// Decide si una fila entra. Un motivo por cada caso que no entra.
pub fn decidir_fila(e: &Entrada) -> Decision {
    let no = |motivo: &str| Decision::NoEntra(motivo.to_string());
    if e.fila.huella.as_deref() != Some(e.huella_laptop) {
        return no("la huella no coincide con la de la laptop");
    }
    let hash_coincide =
        e.tarea_hash_laptop.is_some() && e.fila.tarea_hash.as_deref() == e.tarea_hash_laptop;
    if !hash_coincide {
        return no("el tarea_hash no coincide con el de la laptop");
    }
    if e.ya_estaba {
        return no("ya estaba en la laptop");
    }
    match &e.base {
        None => no("sin base de la laptop para comparar"),
        Some(Err(lados)) => no(&format!(
            "sin casos de la base para comparar: faltan los de {}",
            lados.join(" y de ")
        )),
        Some(Ok(v)) if v.colab.is_empty() && v.laptop.is_empty() => {
            no("sin valores de base para comparar")
        }
        Some(Ok(v)) if !iguales(&v.colab, &v.laptop) => Decision::NoEntra(format!(
            "la base de Colab difiere de la base de la laptop: colab={:?} laptop={:?}",
            v.colab, v.laptop
        )),
        Some(Ok(_)) => Decision::Entra,
    }
}

/// Dos listas de valores ordenados y sin repetir son iguales si cada par está a `TOLERANCIA_VALOR`.
fn iguales(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| (x - y).abs() <= TOLERANCIA_VALOR)
}

/// Revisa el paquete antes de escribir nada. Devuelve un problema por línea; vacío si todo pasa.
pub fn revisar_paquete(
    m: &ManifiestoVuelta,
    hallados: &BTreeMap<String, String>,
    verificacion: Option<&str>,
    commit_laptop: &str,
    filas: &[Fila],
) -> Vec<String> {
    let mut problemas = Vec::new();
    if m.v != 1 {
        problemas.push(format!("manifiesto: la versión {} no se entiende", m.v));
    }
    if m.commit_root != commit_laptop {
        problemas.push(format!(
            "el commit del paquete es {}; la laptop está en {}",
            m.commit_root, commit_laptop
        ));
    }
    for (ruta, sha) in &m.archivos {
        if !ruta_segura(ruta) {
            problemas.push(format!("ruta no permitida: {ruta}"));
            continue;
        }
        match hallados.get(ruta) {
            None => problemas.push(format!("falta {ruta}, listado en el manifiesto")),
            Some(real) if real != sha => {
                problemas.push(format!(
                    "el SHA-256 de {ruta} no coincide con el manifiesto"
                ));
            }
            Some(_) => {}
        }
    }
    for ruta in hallados.keys() {
        if !m.archivos.contains_key(ruta) {
            problemas.push(format!("{ruta} no está listado en el manifiesto"));
        }
    }
    for fila in &m.filas {
        for nombre in &fila.casos {
            if !nombre_de_casos_valido(nombre) {
                problemas.push(format!("nombre de casos no permitido: {nombre}"));
                continue;
            }
            if !hallados.contains_key(&format!("{CARPETA_CALIBRACION}/{nombre}")) {
                problemas.push(format!("falta {CARPETA_CALIBRACION}/{nombre}"));
            }
        }
    }
    match verificacion {
        None => problemas.push("falta verificacion.txt".into()),
        Some(texto) => {
            let lineas: Vec<&str> = texto
                .lines()
                .map(str::trim_end)
                .filter(|l| !l.is_empty())
                .collect();
            if lineas.is_empty() {
                problemas.push("verificacion.txt está vacío".into());
            }
            for linea in lineas {
                if !linea.starts_with("ok ") {
                    problemas.push(format!("verificación: {linea}"));
                }
            }
        }
    }
    for fila in &m.filas {
        if !filas.iter().any(|f| coincide_vuelta(fila, f)) {
            problemas.push(format!(
                "el manifiesto lista {} {} y filas.json no la trae",
                fila.tarea, fila.dimension
            ));
        }
    }
    for fila in filas {
        if !m.filas.iter().any(|v| coincide_vuelta(v, fila)) {
            problemas.push(format!(
                "filas.json trae {} {} y el manifiesto no la lista",
                fila.tarea,
                fila.dimension.clave()
            ));
        }
    }
    problemas
}

/// Dice si una ruta del paquete es relativa y no sale de su carpeta.
fn ruta_segura(ruta: &str) -> bool {
    !ruta.is_empty() && !ruta.starts_with('/') && !ruta.split('/').any(|c| c == "..")
}

/// Dice si un nombre de casos es un nombre simple `.casos.jsonl`, sin carpetas ni `..`.
fn nombre_de_casos_valido(nombre: &str) -> bool {
    !nombre.is_empty()
        && !nombre.contains(['/', '\\'])
        && !nombre.contains("..")
        && nombre.ends_with(".casos.jsonl")
}

/// Dice si una fila del manifiesto es la misma medición que una fila de `filas.json`.
fn coincide_vuelta(vuelta: &FilaVuelta, fila: &Fila) -> bool {
    vuelta.tarea == fila.tarea && vuelta.dimension == fila.dimension.clave() && vuelta.ts == fila.ts
}

/// Valores distintos de una lista, ordenados. Dos valores a menos de `TOLERANCIA_VALOR` cuentan como uno.
pub fn valores_distintos(valores: impl IntoIterator<Item = f32>) -> Vec<f32> {
    let mut ordenados: Vec<f32> = valores.into_iter().collect();
    ordenados.sort_by(f32::total_cmp);
    let mut distintos: Vec<f32> = Vec::new();
    for valor in ordenados {
        match distintos.last() {
            Some(ultimo) if (valor - ultimo).abs() <= TOLERANCIA_VALOR => {}
            _ => distintos.push(valor),
        }
    }
    distintos
}

/// Punto de entrada de `colab importar`. Toma el commit de la laptop de `raiz_root` y el ROOT_HOME real.
pub fn importar_cli(pedido: PedidoImportar) -> Result<ResumenImportar, ErrorOraculo> {
    let raiz_root = match pedido.raiz_root {
        Some(raiz) => raiz,
        None => std::env::current_dir()
            .map_err(|e| ErrorOraculo::Infra(format!("directorio actual: {e}")))?,
    };
    let commit = commit_de(&raiz_root)?;
    importar(&root_home(), &pedido.archivo, &commit)
}

/// Importa el paquete de vuelta en `home`. `commit_laptop` es el commit de la laptop.
/// Revisa todo antes de escribir. Si una fila no pasa, las demás siguen.
pub fn importar(
    home: &Path,
    archivo: &Path,
    commit_laptop: &str,
) -> Result<ResumenImportar, ErrorOraculo> {
    if !archivo.is_file() {
        return Err(ErrorOraculo::Uso(format!(
            "no existe el paquete {}",
            archivo.display()
        )));
    }
    // La carpeta de trabajo vive en ROOT_HOME/tmp y se borra al salir, también ante error.
    let tmp = home.join("tmp");
    fs::create_dir_all(&tmp)
        .map_err(|e| ErrorOraculo::Infra(format!("crear {}: {e}", tmp.display())))?;
    let dir = tmp.join(format!("colab-importar-{}", std::process::id()));
    borrado::borrar_dir_dentro(&tmp, &dir)
        .map_err(|e| ErrorOraculo::Infra(format!("limpiar {}: {e}", dir.display())))?;
    fs::create_dir_all(&dir)
        .map_err(|e| ErrorOraculo::Infra(format!("crear {}: {e}", dir.display())))?;
    let resultado = importar_desde(home, archivo, &dir, commit_laptop);
    let limpieza = borrado::borrar_dir_dentro(&tmp, &dir);
    let resumen = resultado?;
    limpieza.map_err(|e| ErrorOraculo::Infra(format!("borrar la carpeta de trabajo: {e}")))?;
    Ok(resumen)
}

/// Cuerpo de `importar`: extrae, revisa, decide por fila y escribe. `dir` es la carpeta de trabajo vacía.
fn importar_desde(
    home: &Path,
    archivo: &Path,
    dir: &Path,
    commit_laptop: &str,
) -> Result<ResumenImportar, ErrorOraculo> {
    extraer_paquete(archivo, dir)?;
    let enlaces = enlaces_simbolicos(dir)
        .map_err(|e| ErrorOraculo::Infra(format!("revisar el paquete: {e}")))?;
    if !enlaces.is_empty() {
        return Err(ErrorOraculo::Uso(format!(
            "el paquete trae enlaces simbólicos: {}",
            enlaces.join(", ")
        )));
    }
    let manifiesto: ManifiestoVuelta = leer_json(&dir.join(MANIFIESTO_VUELTA))?;
    let filas: Vec<Fila> = leer_json(&dir.join("filas.json"))?;
    let verificacion = leer_texto_si_existe(&dir.join("verificacion.txt"))?;
    let mut hallados =
        archivos_de(dir).map_err(|e| ErrorOraculo::Infra(format!("leer el paquete: {e}")))?;
    hallados.remove(MANIFIESTO_VUELTA);

    let problemas = revisar_paquete(
        &manifiesto,
        &hallados,
        verificacion.as_deref(),
        commit_laptop,
        &filas,
    );
    if !problemas.is_empty() {
        let lista: Vec<String> = problemas.iter().map(|p| format!("- {p}")).collect();
        return Err(ErrorOraculo::Uso(format!(
            "el paquete de vuelta no pasa la revisión:\n{}",
            lista.join("\n")
        )));
    }

    let calibracion = Calibraciones::cargar(home)?;
    let mut entran: Vec<&Fila> = Vec::new();
    let mut nombres_a_copiar: BTreeSet<String> = BTreeSet::new();
    let mut rechazadas = Vec::new();
    for fila in &filas {
        let vuelta = manifiesto.filas.iter().find(|v| coincide_vuelta(v, fila));
        let casos_colab: &[String] = match vuelta {
            Some(v) => &v.casos,
            None => &[],
        };
        let huella = huella_de(fila.dimension);
        let ya_estaba = calibracion
            .filas
            .iter()
            .any(|f| misma_medicion(f, fila) && f.ts == fila.ts);
        let base = mejor_base(&calibracion, fila, &huella);
        let valores = match base {
            Some(b) => Some(valores_de_base(home, dir, casos_colab, b)?),
            None => None,
        };
        let decision = decidir_fila(&Entrada {
            fila,
            huella_laptop: &huella,
            tarea_hash_laptop: tarea_hash_de(home, &fila.tarea).as_deref(),
            ya_estaba,
            base: valores,
        });
        match decision {
            Decision::Entra => {
                entran.push(fila);
                nombres_a_copiar.extend(casos_colab.iter().cloned());
            }
            Decision::NoEntra(motivo) => rechazadas.push(Rechazada {
                tarea: fila.tarea.clone(),
                dimension: fila.dimension.clave().to_string(),
                motivo,
            }),
        }
    }

    let mut avisos = Vec::new();
    if !filas.is_empty() {
        avisos.push(AVISO_MUTANTES.to_string());
    }
    if entran.is_empty() {
        return Ok(ResumenImportar {
            entraron: 0,
            rechazadas,
            casos_copiados: 0,
            respaldo: None,
            avisos,
        });
    }

    let respaldo = respaldar(home)?;
    for fila in &entran {
        calibra::agregar(home, fila)?;
    }
    let destino_casos = home.join(CARPETA_CALIBRACION);
    fs::create_dir_all(&destino_casos)
        .map_err(|e| ErrorOraculo::Infra(format!("crear {}: {e}", destino_casos.display())))?;
    let mut casos_copiados = 0;
    for nombre in &nombres_a_copiar {
        let origen = dir.join(CARPETA_CALIBRACION).join(nombre);
        if copiar_caso(&origen, &destino_casos.join(nombre))? {
            casos_copiados += 1;
        } else {
            avisos.push(format!("no se copió {nombre}: ya existe en la laptop"));
        }
    }
    Ok(ResumenImportar {
        entraron: entran.len(),
        rechazadas,
        casos_copiados,
        respaldo: Some(respaldo),
        avisos,
    })
}

/// Dice si dos filas son de la misma tarea y dimensión.
fn misma_medicion(a: &Fila, b: &Fila) -> bool {
    a.tarea == b.tarea && a.dimension == b.dimension
}

/// La fila de la laptop con más `ts` de la misma tarea, dimensión y huella.
fn mejor_base<'a>(calibracion: &'a Calibraciones, fila: &Fila, huella: &str) -> Option<&'a Fila> {
    calibracion
        .filas
        .iter()
        .filter(|f| misma_medicion(f, fila) && f.huella.as_deref() == Some(huella))
        .max_by(|a, b| a.ts.total_cmp(&b.ts))
}

/// Hash del `tarea.toml` de la tarea en la laptop. `None` si la tarea no carga.
fn tarea_hash_de(home: &Path, tarea: &str) -> Option<String> {
    let cargada = cargar_en(home, tarea).ok()?;
    hash_de_tarea(&cargada).ok()
}

/// Valores de `valor_base` de la base de Colab y de la laptop. `Err` con los lados que no tienen casos.
fn valores_de_base(
    home: &Path,
    dir: &Path,
    casos_colab: &[String],
    base: &Fila,
) -> Result<Result<Valores, Vec<&'static str>>, ErrorOraculo> {
    let rutas_colab: Vec<PathBuf> = casos_colab
        .iter()
        .map(|n| dir.join(CARPETA_CALIBRACION).join(n))
        .collect();
    let rutas_laptop = casos_de_laptop(home, base)?;
    let mut faltan = Vec::new();
    if rutas_colab.is_empty() || !rutas_colab.iter().all(|r| r.is_file()) {
        faltan.push("Colab");
    }
    if rutas_laptop.is_empty() {
        faltan.push("la laptop");
    }
    if !faltan.is_empty() {
        return Ok(Err(faltan));
    }
    let colab = valores_de_casos(&rutas_colab)?;
    let laptop = valores_de_casos(&rutas_laptop)?;
    Ok(Ok(Valores {
        colab: valores_distintos(colab),
        laptop: valores_distintos(laptop),
    }))
}

/// Archivos `.casos.jsonl` de la laptop para la fila `base`: `<tarea>-<dimension>-<ts>` con o sin `-N`.
fn casos_de_laptop(home: &Path, base: &Fila) -> Result<Vec<PathBuf>, ErrorOraculo> {
    let carpeta = home.join(CARPETA_CALIBRACION);
    let entradas = match fs::read_dir(&carpeta) {
        Ok(entradas) => entradas,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            return Err(ErrorOraculo::Infra(format!(
                "leer {}: {e}",
                carpeta.display()
            )));
        }
    };
    let prefijo = format!(
        "{}-{}-{}",
        base.tarea,
        base.dimension.clave(),
        base.ts as u64
    );
    let mut rutas = Vec::new();
    for entrada in entradas {
        let entrada =
            entrada.map_err(|e| ErrorOraculo::Infra(format!("leer {}: {e}", carpeta.display())))?;
        let nombre = entrada.file_name().to_string_lossy().into_owned();
        if es_caso_de(&nombre, &prefijo) {
            rutas.push(entrada.path());
        }
    }
    rutas.sort();
    Ok(rutas)
}

/// Dice si `nombre` es `<prefijo>.casos.jsonl` o `<prefijo>-N.casos.jsonl`.
fn es_caso_de(nombre: &str, prefijo: &str) -> bool {
    let Some(resto) = nombre.strip_prefix(prefijo) else {
        return false;
    };
    if resto == ".casos.jsonl" {
        return true;
    }
    resto
        .strip_prefix('-')
        .and_then(|r| r.strip_suffix(".casos.jsonl"))
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// Lee los `valor_base` de los renglones de cada archivo de casos. Un renglón ilegible frena la importación.
fn valores_de_casos(rutas: &[PathBuf]) -> Result<Vec<f32>, ErrorOraculo> {
    let mut valores = Vec::new();
    for ruta in rutas {
        let texto = fs::read_to_string(ruta)
            .map_err(|e| ErrorOraculo::Infra(format!("leer {}: {e}", ruta.display())))?;
        for linea in texto.lines().filter(|l| !l.trim().is_empty()) {
            let caso: CasoGuardado = serde_json::from_str(linea).map_err(|e| {
                ErrorOraculo::Infra(format!("caso ilegible en {}: {e}", ruta.display()))
            })?;
            valores.extend(caso.valor_base);
        }
    }
    Ok(valores)
}

/// Extrae el tar.gz del paquete en `dir` con el `tar` del sistema.
fn extraer_paquete(archivo: &Path, dir: &Path) -> Result<(), ErrorOraculo> {
    let absoluto = std::path::absolute(archivo)
        .map_err(|e| ErrorOraculo::Infra(format!("ruta del paquete: {e}")))?;
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

/// Rutas relativas de los enlaces simbólicos bajo `raiz`. No sigue los enlaces.
fn enlaces_simbolicos(raiz: &Path) -> io::Result<Vec<String>> {
    let mut hallados = Vec::new();
    buscar_enlaces(raiz, raiz, &mut hallados)?;
    Ok(hallados)
}

fn buscar_enlaces(raiz: &Path, dir: &Path, hallados: &mut Vec<String>) -> io::Result<()> {
    for entrada in fs::read_dir(dir)? {
        let entrada = entrada?;
        let tipo = entrada.file_type()?;
        let ruta = entrada.path();
        if tipo.is_symlink() {
            hallados.push(
                ruta.strip_prefix(raiz)
                    .unwrap_or(&ruta)
                    .display()
                    .to_string(),
            );
        } else if tipo.is_dir() {
            buscar_enlaces(raiz, &ruta, hallados)?;
        }
    }
    Ok(())
}

/// Lee un JSON del paquete. Un archivo que falta o no parsea es error de uso.
fn leer_json<T: serde::de::DeserializeOwned>(ruta: &Path) -> Result<T, ErrorOraculo> {
    let nombre = ruta
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let texto = match fs::read_to_string(ruta) {
        Ok(texto) => texto,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Err(ErrorOraculo::Uso(format!("falta {nombre} en el paquete")));
        }
        Err(e) => return Err(ErrorOraculo::Infra(format!("leer {nombre}: {e}"))),
    };
    serde_json::from_str(&texto).map_err(|e| ErrorOraculo::Uso(format!("{nombre} ilegible: {e}")))
}

/// Lee un texto del paquete. Sin archivo, devuelve `None`.
fn leer_texto_si_existe(ruta: &Path) -> Result<Option<String>, ErrorOraculo> {
    match fs::read_to_string(ruta) {
        Ok(texto) => Ok(Some(texto)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(ErrorOraculo::Infra(format!("leer {}: {e}", ruta.display()))),
    }
}

/// Copia un caso a `destino` sin pisar. Devuelve `false` si el nombre ya existe.
fn copiar_caso(origen: &Path, destino: &Path) -> Result<bool, ErrorOraculo> {
    let mut entrada = fs::File::open(origen)
        .map_err(|e| ErrorOraculo::Infra(format!("abrir {}: {e}", origen.display())))?;
    let mut salida = match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destino)
    {
        Ok(salida) => salida,
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => return Ok(false),
        Err(e) => {
            return Err(ErrorOraculo::Infra(format!(
                "crear {}: {e}",
                destino.display()
            )));
        }
    };
    io::copy(&mut entrada, &mut salida)
        .map_err(|e| ErrorOraculo::Infra(format!("copiar {}: {e}", origen.display())))?;
    Ok(true)
}

/// Respaldo de `calibracion.json` y `calibracion/` en `colab/respaldos/<fecha>/`, con `cp`.
fn respaldar(home: &Path) -> Result<PathBuf, ErrorOraculo> {
    let padre = home.join("colab").join("respaldos");
    fs::create_dir_all(&padre)
        .map_err(|e| ErrorOraculo::Infra(format!("crear {}: {e}", padre.display())))?;
    let formato =
        time::format_description::parse_borrowed::<2>("[year][month][day]-[hour][minute][second]")
            .map_err(|e| ErrorOraculo::Infra(format!("formato de fecha: {e}")))?;
    let fecha = time::OffsetDateTime::now_utc()
        .format(&formato)
        .map_err(|e| ErrorOraculo::Infra(format!("fecha del respaldo: {e}")))?;
    let destino = reservar_carpeta(&padre, &fecha)
        .map_err(|e| ErrorOraculo::Infra(format!("crear el respaldo: {e}")))?;
    let fuentes: Vec<PathBuf> = [
        home.join("calibracion.json"),
        home.join(CARPETA_CALIBRACION),
    ]
    .into_iter()
    .filter(|p| p.exists())
    .collect();
    if !fuentes.is_empty() {
        let salida = Command::new("cp")
            .arg("--reflink=auto")
            .arg("-r")
            .args(&fuentes)
            .arg(&destino)
            .output()
            .map_err(|e| ErrorOraculo::Infra(format!("correr cp: {e}")))?;
        if !salida.status.success() {
            return Err(ErrorOraculo::Infra(format!(
                "cp falló: {}",
                String::from_utf8_lossy(&salida.stderr).trim()
            )));
        }
    }
    Ok(destino)
}

/// Crea la primera carpeta libre `<base>`, `<base>-2`, `<base>-3`... en `padre`.
fn reservar_carpeta(padre: &Path, base: &str) -> io::Result<PathBuf> {
    let mut n = 1u32;
    loop {
        let nombre = if n == 1 {
            base.to_string()
        } else {
            format!("{base}-{n}")
        };
        let ruta = padre.join(nombre);
        match fs::create_dir(&ruta) {
            Ok(()) => return Ok(ruta),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => n += 1,
            Err(e) => return Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::fs;
    use std::process::Command;

    /// Doble de la imagen. Devuelve datos fijos y no llama a podman.
    struct Doble;

    impl Imagen for Doble {
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

    /// Bytes de un ELF de 64 bits, little endian, x86-64 (e_machine 0x3E).
    const ELF_X86_64: &[u8] = &[
        0x7f, b'E', b'L', b'F', 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0x3e, 0,
    ];

    const TAREA_TOML: &str = "nombre = \"tarea-prueba\"\nlenguaje = \"rust\"\nimagen = \"imagen-x\"\nspec = \"docs/SPEC.md\"\n\n[contrato]\ncmd = '{CMD}'\n\n[oculto]\ncmd = 'cargo test'\n";

    /// Un ROOT_HOME falso, un repo bueno, una raíz de Root y un binario ELF.
    pub(super) struct Escenario {
        pub(super) home: tempfile::TempDir,
        raiz: tempfile::TempDir,
        repo: tempfile::TempDir,
        salida: tempfile::TempDir,
        binario: PathBuf,
    }

    impl Escenario {
        pub(super) fn con_contrato(cmd: &str) -> Self {
            let e = Escenario {
                home: tempfile::tempdir().expect("home"),
                raiz: tempfile::tempdir().expect("raiz"),
                repo: tempfile::tempdir().expect("repo"),
                salida: tempfile::tempdir().expect("salida"),
                binario: PathBuf::new(),
            };
            let e = Escenario {
                binario: e.raiz.path().join("r00t-bin"),
                ..e
            };
            let home = e.home.path();
            let dir = home.join("oraculos/tarea-prueba");
            fs::create_dir_all(dir.join("oculto")).expect("dir");
            fs::create_dir_all(dir.join("docs")).expect("dir");
            fs::write(dir.join("oculto/test_o.rs"), "// oculto\n").expect("escribe");
            fs::write(dir.join("docs/SPEC.md"), "# spec\n").expect("escribe");
            fs::write(dir.join("tarea.toml"), TAREA_TOML.replace("{CMD}", cmd)).expect("escribe");

            // Repo bueno, con `.git` y `target` que no deben entrar.
            fs::write(e.repo.path().join("Cargo.lock"), "lock\n").expect("escribe");
            fs::write(e.repo.path().join("a.txt"), "original").expect("escribe");
            fs::create_dir_all(e.repo.path().join("target")).expect("dir");
            fs::write(e.repo.path().join("target/basura"), "x").expect("escribe");
            fs::create_dir_all(e.repo.path().join(".git")).expect("dir");
            fs::write(e.repo.path().join(".git/HEAD"), "ref\n").expect("escribe");

            // Caché de dependencias, con el hash que calcula la descarga.
            let cache = home
                .join("cache/rust")
                .join(crate::oracle::descarga::hash_corto(b"lock\n"));
            fs::create_dir_all(&cache).expect("dir");
            fs::write(cache.join("config.toml"), "[x]\n").expect("escribe");
            fs::create_dir_all(home.join("cache/rustsec")).expect("dir");
            fs::write(home.join("cache/rustsec/aviso.txt"), "aviso\n").expect("escribe");
            fs::write(home.join("calibracion.json"), "{}\n").expect("escribe");
            fs::create_dir_all(home.join("configs")).expect("dir");
            fs::write(home.join("configs/hola.txt"), "hola\n").expect("escribe");

            // Registros que nunca deben salir.
            fs::write(home.join("eventos.jsonl"), "{}\n").expect("escribe");
            fs::write(home.join("runs.jsonl"), "{}\n").expect("escribe");
            fs::create_dir_all(home.join("transcripts")).expect("dir");
            fs::write(home.join("transcripts/t.jsonl"), "{}\n").expect("escribe");
            fs::create_dir_all(home.join("colab")).expect("dir");
            fs::write(home.join("colab/activo.json"), "{}\n").expect("escribe");
            fs::create_dir_all(home.join("evidencia")).expect("dir");
            fs::write(home.join("evidencia/x.json"), "{}\n").expect("escribe");

            // Raíz de Root y binario.
            fs::create_dir_all(e.raiz.path().join("tools/colab")).expect("dir");
            fs::write(e.raiz.path().join("tools/colab/prepara.py"), "print()\n").expect("escribe");
            fs::write(&e.binario, ELF_X86_64).expect("escribe");
            e
        }

        fn paquete(&self, trabajos: &[(&str, Dimension)]) -> Paquete {
            let mut repos = BTreeMap::new();
            repos.insert("tarea-prueba".to_string(), self.repo.path().to_path_buf());
            Paquete {
                home: self.home.path().to_path_buf(),
                raiz_root: self.raiz.path().to_path_buf(),
                binario: self.binario.clone(),
                trabajos: trabajos.iter().map(|(t, d)| (t.to_string(), *d)).collect(),
                repos,
                rustup_sha256: "a".repeat(64),
                salida: self.salida.path().join("paquete.tar.gz"),
                commit_root: "abc123".into(),
            }
        }
    }

    /// Lista las entradas de archivo del paquete, sin `./` ni carpetas.
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

    /// Extrae el paquete en una carpeta temporal nueva.
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

    #[test]
    fn paquete_trae_oraculos_cache_rustsec_calibracion_binario_y_herramientas() {
        let e = Escenario::con_contrato("cargo test");
        let resumen = empaquetar(&e.paquete(&[("tarea-prueba", Dimension::Funciona)]), &Doble)
            .expect("empaqueta");
        let nombres = listar(&resumen.salida);
        assert!(nombres.contains("r00t"));
        for herramienta in HERRAMIENTAS {
            assert!(
                nombres.contains(&format!("herramientas/{herramienta}")),
                "falta {herramienta}"
            );
        }
        assert!(nombres.contains("oraculos/tarea-prueba/tarea.toml"));
        assert!(nombres.contains("versiones.env"));
        assert!(nombres.contains("entorno.env"));
        assert!(nombres.contains("tools/colab/prepara.py"));
        assert!(nombres.contains("repos/tarea-prueba/a.txt"));
        assert!(nombres.contains("calibracion.json"));
        assert!(nombres.contains("cache/rustsec/aviso.txt"));
        assert!(nombres.contains("manifiesto.json"));
        assert!(
            nombres
                .iter()
                .any(|n| n.starts_with("cache/rust/") && n.ends_with("config.toml"))
        );
        // El repo no lleva `.git` ni `target`.
        assert!(
            !nombres
                .iter()
                .any(|n| n.starts_with("repos/tarea-prueba/target"))
        );
        assert!(
            !nombres
                .iter()
                .any(|n| n.starts_with("repos/tarea-prueba/.git"))
        );
        // El binario sale con permisos 0755.
        let extraido = extraer(&resumen.salida);
        let modo = fs::metadata(extraido.path().join("r00t"))
            .expect("r00t")
            .permissions();
        assert_eq!(
            std::os::unix::fs::PermissionsExt::mode(&modo) & 0o777,
            0o755
        );
    }

    #[test]
    fn paquete_no_trae_registros_ni_colab_ni_evidencia() {
        let e = Escenario::con_contrato("cargo test");
        let resumen = empaquetar(&e.paquete(&[("tarea-prueba", Dimension::Funciona)]), &Doble)
            .expect("empaqueta");
        let nombres = listar(&resumen.salida);
        let prohibidos = ["eventos.jsonl", "runs.jsonl", "transcripts", "evidencia"];
        for nombre in &nombres {
            assert!(
                !nombre.split('/').any(|c| prohibidos.contains(&c)),
                "entra {nombre}"
            );
            assert!(!nombre.starts_with("colab/"), "entra {nombre}");
        }
    }

    #[test]
    fn rechaza_eficiencia_y_diseno_con_su_motivo() {
        let e = Escenario::con_contrato("cargo test");
        let texto = empaquetar(
            &e.paquete(&[("tarea-prueba", Dimension::Eficiencia)]),
            &Doble,
        )
        .expect_err("rechaza eficiencia")
        .to_string();
        assert!(
            texto.contains("Eficiencia necesita el cgroup del contenedor"),
            "{texto}"
        );
        assert!(
            texto.contains("tarea-prueba") && texto.contains("eficiencia"),
            "{texto}"
        );

        let texto = empaquetar(&e.paquete(&[("tarea-prueba", Dimension::Diseno)]), &Doble)
            .expect_err("rechaza diseño")
            .to_string();
        assert!(
            texto.contains("el juez necesita Colab Secrets, que `exec` no lee"),
            "{texto}"
        );
        assert!(
            texto.contains("tarea-prueba") && texto.contains("diseno"),
            "{texto}"
        );
    }

    #[test]
    fn root_toml_con_secreto_falla_sin_mostrar_el_valor() {
        let e = Escenario::con_contrato("cargo test");
        fs::write(
            e.home.path().join("root.toml"),
            "[proveedores.deepseek]\napi_key = \"valor-secreto-123\"\n",
        )
        .expect("escribe");
        let texto = empaquetar(&e.paquete(&[("tarea-prueba", Dimension::Funciona)]), &Doble)
            .expect_err("rechaza")
            .to_string();
        assert!(!texto.contains("valor-secreto-123"), "{texto}");
        assert!(
            texto.contains("proveedores.deepseek") && texto.contains("api_key"),
            "{texto}"
        );
        assert!(revisar_root_toml("[calibracion]\nmax_tokens = 4096\n").is_ok());
    }

    #[test]
    fn variable_con_token_no_entra_a_entorno_env() {
        let e = Escenario::con_contrato("cargo test");
        let resumen = empaquetar(&e.paquete(&[("tarea-prueba", Dimension::Funciona)]), &Doble)
            .expect("empaqueta");
        let extraido = extraer(&resumen.salida);
        let entorno = fs::read_to_string(extraido.path().join("entorno.env")).expect("entorno");
        assert!(entorno.contains("PATH=/usr/bin"), "{entorno}");
        assert!(!entorno.contains("GITHUB_TOKEN"), "{entorno}");
        assert_eq!(
            filtrar_entorno(&["PATH=/usr/bin".into(), "GITHUB_TOKEN=abc".into()]),
            vec!["PATH=/usr/bin".to_string()]
        );
    }

    #[test]
    fn manifiesto_trae_sha256_del_archivo_conocido_y_el_orden_del_trabajo() {
        let e = Escenario::con_contrato("cargo test");
        let resumen = empaquetar(
            &e.paquete(&[("tarea-prueba", Dimension::Seguridad)]),
            &Doble,
        )
        .expect("empaqueta");
        let extraido = extraer(&resumen.salida);
        let texto =
            fs::read_to_string(extraido.path().join("manifiesto.json")).expect("manifiesto");
        let m: Manifiesto = serde_json::from_str(&texto).expect("JSON del manifiesto");
        assert_eq!(m.v, 1);
        assert_eq!(m.commit_root, "abc123");
        // sha256 de "hola\n", calculado con `sha256sum`.
        assert_eq!(
            m.archivos.get("configs/hola.txt").map(String::as_str),
            Some("133ee989293f92736301280c6f14c89d521200c17dcdcecca30cd20705332d44")
        );
        let tarea = crate::oracle::tarea::cargar_en(e.home.path(), "tarea-prueba").expect("tarea");
        let trabajo = &m.trabajos[0];
        assert_eq!(
            trabajo.huella,
            crate::oracle::calibra::huella_de(Dimension::Seguridad)
        );
        assert_eq!(
            trabajo.tarea_hash,
            crate::oracle::calibra::hash_de_tarea(&tarea).expect("hash")
        );
        assert_eq!(
            trabajo.comando,
            "r00t oraculo calibrar --tarea tarea-prueba --dimension seguridad --repo repos/tarea-prueba --json"
        );
        assert_eq!(
            trabajo
                .entorno
                .get(ROOT_BWRAP_IMAGEN_ID)
                .map(String::as_str),
            Some("sha256:imagen-doble")
        );
        assert_eq!(
            trabajo
                .entorno
                .get(ROOT_CONTENEDOR_MODO)
                .map(String::as_str),
            Some("bwrap")
        );
        assert_eq!(
            m.imagenes.get("tarea-prueba").map(String::as_str),
            Some("sha256:imagen-doble")
        );
    }

    #[test]
    fn es_nombre_secreto_distingue_tokens_de_claves() {
        assert!(!es_nombre_secreto("max_tokens"));
        assert!(!es_nombre_secreto("tokens"));
        assert!(!es_nombre_secreto("API_KEY_ENV"));
        assert!(es_nombre_secreto("api_key"));
        assert!(es_nombre_secreto("GITHUB_TOKEN"));
        assert!(es_nombre_secreto("clave.txt"));
    }

    #[test]
    fn tarea_con_pytest_se_rechaza() {
        let e = Escenario::con_contrato("pytest -q");
        let texto = empaquetar(&e.paquete(&[("tarea-prueba", Dimension::Funciona)]), &Doble)
            .expect_err("rechaza")
            .to_string();
        assert!(texto.contains("Colab usa otro Python"), "{texto}");
    }

    #[test]
    fn binario_que_no_es_elf_x86_64_se_rechaza() {
        assert!(es_elf_x86_64(ELF_X86_64));
        assert!(!es_elf_x86_64(b"#!/bin/sh\n"));
        let mut arm = ELF_X86_64.to_vec();
        arm[18] = 0xb7;
        assert!(!es_elf_x86_64(&arm));
        let mut bits32 = ELF_X86_64.to_vec();
        bits32[4] = 1;
        assert!(!es_elf_x86_64(&bits32));

        let e = Escenario::con_contrato("cargo test");
        fs::write(&e.binario, "no es ELF\n").expect("escribe");
        let texto = empaquetar(&e.paquete(&[("tarea-prueba", Dimension::Funciona)]), &Doble)
            .expect_err("rechaza")
            .to_string();
        assert!(texto.contains("ELF"), "{texto}");
    }

    #[test]
    fn versiones_env_da_tres_lineas_con_formato() {
        let hashes: Vec<(String, String)> = HERRAMIENTAS
            .iter()
            .map(|h| (h.to_string(), format!("{h}-sha")))
            .collect();
        let texto = versiones_env("1.91.0", &"b".repeat(64), &hashes);
        let esperado = format!(
            "RUST_VERSION=1.91.0\nRUSTUP_SHA256={}\nHERRAMIENTAS_SHA256=cargo-audit:cargo-audit-sha,cargo-mutants:cargo-mutants-sha,cargo-nextest:cargo-nextest-sha,gitleaks:gitleaks-sha,hadolint:hadolint-sha\n",
            "b".repeat(64)
        );
        assert_eq!(texto, esperado);
        assert_eq!(
            version_de_rustc("rustc 1.91.0 (abc 2026-01-01)").as_deref(),
            Some("1.91.0")
        );
        assert_eq!(version_de_rustc("gcc 14"), None);
    }

    #[test]
    fn podman_real_lee_id_entorno_y_version_con_script_falso() {
        const SCRIPT: &str = "#!/bin/sh\ncase \"$*\" in\n  \"image inspect --format {{.Id}} imagen-x\") echo sha256:abc ;;\n  \"image inspect --format {{json .Config.Env}} imagen-x\") echo '[\"PATH=/usr/bin\",\"GITHUB_TOKEN=abc\"]' ;;\n  \"run --rm imagen-x rustc --version\") echo \"rustc 1.91.0 (f00 2026)\" ;;\n  *) echo \"argumento inesperado: $*\" >&2; exit 1 ;;\nesac\n";
        let dir = tempfile::tempdir().expect("dir");
        let script =
            crate::prueba_ejecutable::escribir_ejecutable(&dir.path().join("podman"), SCRIPT);
        let podman = PodmanReal { binario: script };
        assert_eq!(podman.id("imagen-x").expect("id"), "sha256:abc");
        assert_eq!(
            podman.entorno("imagen-x").expect("entorno"),
            vec!["PATH=/usr/bin".to_string(), "GITHUB_TOKEN=abc".to_string()]
        );
        assert_eq!(
            podman.version_rustc("imagen-x").expect("rustc"),
            "rustc 1.91.0 (f00 2026)"
        );
    }

    #[test]
    fn parseo_de_trabajos_repos_y_sha_de_entrada() {
        let trabajos = parsear_trabajos(&["tarea-a:funciona".into(), "tarea-b:pruebas".into()])
            .expect("trabajos");
        assert_eq!(
            trabajos,
            vec![
                ("tarea-a".to_string(), Dimension::Funciona),
                ("tarea-b".to_string(), Dimension::Pruebas)
            ]
        );
        assert!(parsear_trabajos(&["tarea-a".into()]).is_err());
        assert!(parsear_trabajos(&["tarea-a:nada".into()]).is_err());
        let repos = parsear_repos(&["tarea-a=ruta/a".into()]).expect("repos");
        assert_eq!(repos.get("tarea-a"), Some(&PathBuf::from("ruta/a")));
        assert!(parsear_repos(&["sin-igual".into()]).is_err());
        assert!(es_sha256_hex(&"a".repeat(64)));
        assert!(!es_sha256_hex("abc"));
    }
}

#[cfg(test)]
mod tests_importar {
    use super::tests::Escenario;
    use super::*;
    use crate::oracle::tarea::cargar_en;

    const TS_LAPTOP: f64 = 1_700_000_000.0;
    const TS_COLAB: f64 = 1_700_100_000.0;
    const TS_OTRO: f64 = 1_700_200_000.0;
    const VERIFICACION_OK: &str = "ok tests\nok clippy\n";

    fn huella() -> String {
        huella_de(Dimension::Seguridad)
    }

    fn tarea_hash(home: &Path) -> String {
        hash_de_tarea(&cargar_en(home, "tarea-prueba").expect("tarea")).expect("hash")
    }

    /// Fila de Seguridad de la tarea de prueba. Los campos que no van aquí valen 0 o vacío.
    fn fila_de(ts: f64, huella: &str, tarea_hash: &str) -> Fila {
        serde_json::from_value(serde_json::json!({
            "tarea": "tarea-prueba",
            "dimension": "seguridad",
            "n": 4,
            "semilla": 7,
            "ts": ts,
            "huella": huella,
            "tarea_hash": tarea_hash,
        }))
        .expect("fila de prueba")
    }

    fn nombre_casos(ts: f64) -> String {
        format!("tarea-prueba-seguridad-{}.casos.jsonl", ts as u64)
    }

    /// Un renglón de `.casos.jsonl` con el `valor_base` dado.
    fn linea_caso(valor_base: f32) -> String {
        format!(
            "{{\"caso\":0,\"es_defecto\":false,\"detectado\":false,\"valor_base\":{valor_base}}}\n"
        )
    }

    fn escribir_en_calibracion(home: &Path, nombre: &str, contenido: &str) {
        let carpeta = home.join(CARPETA_CALIBRACION);
        fs::create_dir_all(&carpeta).expect("carpeta de casos");
        fs::write(carpeta.join(nombre), contenido).expect("escribe los casos");
    }

    /// ROOT_HOME sin `calibracion.json` de prueba: la laptop no tiene filas.
    fn laptop_sin_base() -> Escenario {
        let e = Escenario::con_contrato("cargo test");
        fs::remove_file(e.home.path().join("calibracion.json"))
            .expect("quita el calibracion.json de prueba");
        e
    }

    /// ROOT_HOME con una fila de la laptop y sus casos. Los casos traen `valor` como `valor_base`.
    fn laptop_con_base(valor: f32) -> Escenario {
        let e = laptop_sin_base();
        let home = e.home.path();
        let fila = fila_de(TS_LAPTOP, &huella(), &tarea_hash(home));
        calibra::agregar(home, &fila).expect("agrega la fila de la laptop");
        escribir_en_calibracion(home, &nombre_casos(TS_LAPTOP), &linea_caso(valor));
        e
    }

    /// Una fila del paquete con sus casos `(nombre, contenido)`. `listados_sin_archivo` solo va al manifiesto.
    struct FilaPrueba {
        fila: Fila,
        casos: Vec<(String, String)>,
        listados_sin_archivo: Vec<String>,
    }

    fn fp(fila: Fila, nombre: &str, contenido: &str) -> FilaPrueba {
        FilaPrueba {
            fila,
            casos: vec![(nombre.to_string(), contenido.to_string())],
            listados_sin_archivo: Vec::new(),
        }
    }

    struct VueltaPrueba {
        commit: String,
        verificacion: Option<String>,
        filas: Vec<FilaPrueba>,
    }

    /// Vuelta con commit `abc123` y verificación en verde.
    fn vuelta(filas: Vec<FilaPrueba>) -> VueltaPrueba {
        VueltaPrueba {
            commit: "abc123".into(),
            verificacion: Some(VERIFICACION_OK.into()),
            filas,
        }
    }

    /// Escribe la carpeta del paquete de vuelta con sus SHA-256 y la empaqueta en un tar.gz.
    /// `alterar` cambia un archivo después de hashearlo, para simular un paquete roto.
    /// Devuelve la carpeta que guarda el tar.gz, que hay que conservar mientras se usa.
    fn armar_paquete(
        v: &VueltaPrueba,
        alterar: Option<(&str, &str)>,
    ) -> (tempfile::TempDir, PathBuf) {
        let staging = tempfile::tempdir().expect("carpeta del paquete");
        let dir = staging.path();
        let mut archivos = BTreeMap::new();
        let mut vueltas = Vec::new();
        let mut filas = Vec::new();
        let mut escribir = |ruta: &str, contenido: &str| {
            let destino = dir.join(ruta);
            fs::create_dir_all(destino.parent().expect("padre")).expect("carpeta");
            fs::write(&destino, contenido).expect("escribe");
            archivos.insert(ruta.to_string(), sha256_hex(contenido.as_bytes()));
        };
        for fp in &v.filas {
            let mut nombres = Vec::new();
            for (nombre, contenido) in &fp.casos {
                escribir(&format!("{CARPETA_CALIBRACION}/{nombre}"), contenido);
                nombres.push(nombre.clone());
            }
            nombres.extend(fp.listados_sin_archivo.iter().cloned());
            vueltas.push(FilaVuelta {
                tarea: fp.fila.tarea.clone(),
                dimension: fp.fila.dimension.clave().to_string(),
                ts: fp.fila.ts,
                casos: nombres,
            });
            filas.push(fp.fila.clone());
        }
        let json_filas = serde_json::to_string(&filas).expect("filas");
        escribir("filas.json", &json_filas);
        if let Some(texto) = &v.verificacion {
            escribir("verificacion.txt", texto);
        }
        let manifiesto = ManifiestoVuelta {
            v: 1,
            commit_root: v.commit.clone(),
            archivos,
            filas: vueltas,
            donde: DondeSeMidio {
                lugar: "colab".into(),
                runtime: "cpu".into(),
            },
        };
        let json = serde_json::to_string_pretty(&manifiesto).expect("manifiesto");
        fs::write(dir.join(MANIFIESTO_VUELTA), format!("{json}\n")).expect("escribe manifiesto");
        if let Some((ruta, nuevo)) = alterar {
            fs::write(dir.join(ruta), nuevo).expect("altera el archivo");
        }
        let caja = tempfile::tempdir().expect("caja del tar.gz");
        let tgz = caja.path().join("vuelta.tar.gz");
        let estado = Command::new("tar")
            .arg("-czf")
            .arg(&tgz)
            .arg("-C")
            .arg(dir)
            .arg(".")
            .status()
            .expect("tar");
        assert!(estado.success(), "tar falló");
        (caja, tgz)
    }

    /// Hash de cada archivo de ROOT_HOME. Sirve para ver si un intento escribió algo.
    fn instantanea(home: &Path) -> BTreeMap<String, String> {
        archivos_de(home).expect("instantánea de ROOT_HOME")
    }

    #[test]
    fn fila_valida_entra_copia_sus_casos_y_deja_respaldo() {
        let e = laptop_con_base(0.5);
        let home = e.home.path();
        let fila = fila_de(TS_COLAB, &huella(), &tarea_hash(home));
        let v = vuelta(vec![fp(fila, &nombre_casos(TS_COLAB), &linea_caso(0.5))]);
        let (_caja, tgz) = armar_paquete(&v, None);

        let resumen = importar(home, &tgz, "abc123").expect("importa");

        assert_eq!(resumen.entraron, 1, "{resumen:?}");
        assert!(resumen.rechazadas.is_empty(), "{resumen:?}");
        assert_eq!(resumen.casos_copiados, 1);
        let cal = calibra::Calibraciones::cargar(home).expect("calibracion");
        assert_eq!(cal.filas.len(), 2);
        assert!(cal.filas.iter().any(|f| f.ts == TS_COLAB));
        let copiado =
            fs::read_to_string(home.join(CARPETA_CALIBRACION).join(nombre_casos(TS_COLAB)))
                .expect("casos copiados");
        assert_eq!(copiado, linea_caso(0.5));
        let respaldo = resumen.respaldo.expect("hay respaldo");
        assert!(respaldo.starts_with(home.join("colab/respaldos")));
        let previa =
            calibra::Calibraciones::cargar(&respaldo).expect("respaldo de calibracion.json");
        assert_eq!(previa.filas.len(), 1);
        assert!(
            respaldo
                .join(CARPETA_CALIBRACION)
                .join(nombre_casos(TS_LAPTOP))
                .is_file()
        );
    }

    #[test]
    fn valor_base_distinto_no_entra_y_nombra_los_dos_valores() {
        let e = laptop_con_base(0.75);
        let home = e.home.path();
        let fila = fila_de(TS_COLAB, &huella(), &tarea_hash(home));
        let v = vuelta(vec![fp(fila, &nombre_casos(TS_COLAB), &linea_caso(0.5))]);
        let (_caja, tgz) = armar_paquete(&v, None);

        let resumen = importar(home, &tgz, "abc123").expect("importa");

        assert_eq!(resumen.entraron, 0, "{resumen:?}");
        assert_eq!(resumen.rechazadas.len(), 1, "{resumen:?}");
        assert_eq!(
            resumen.rechazadas[0].motivo,
            "la base de Colab difiere de la base de la laptop: colab=[0.5] laptop=[0.75]"
        );
        assert!(resumen.respaldo.is_none());
    }

    #[test]
    fn fila_sin_base_en_la_laptop_no_entra() {
        let e = laptop_sin_base();
        let home = e.home.path();
        let fila = fila_de(TS_COLAB, &huella(), &tarea_hash(home));
        let v = vuelta(vec![fp(fila, &nombre_casos(TS_COLAB), &linea_caso(0.5))]);
        let (_caja, tgz) = armar_paquete(&v, None);

        let resumen = importar(home, &tgz, "abc123").expect("importa");

        assert_eq!(resumen.entraron, 0, "{resumen:?}");
        assert_eq!(
            resumen.rechazadas[0].motivo,
            "sin base de la laptop para comparar"
        );
    }

    #[test]
    fn verificacion_con_linea_mal_frena_sin_escribir_nada() {
        let e = laptop_con_base(0.5);
        let home = e.home.path();
        let fila = fila_de(TS_COLAB, &huella(), &tarea_hash(home));
        let mut v = vuelta(vec![fp(fila, &nombre_casos(TS_COLAB), &linea_caso(0.5))]);
        v.verificacion = Some("ok tests\nmal clippy: 0 contra 2\n".into());
        let (_caja, tgz) = armar_paquete(&v, None);
        let antes = instantanea(home);

        let texto = importar(home, &tgz, "abc123")
            .expect_err("frena la importación")
            .to_string();

        assert!(texto.contains("mal clippy: 0 contra 2"), "{texto}");
        assert_eq!(instantanea(home), antes);
        assert!(!home.join("colab/respaldos").exists());
    }

    #[test]
    fn sha_distinto_frena_sin_escribir_nada() {
        let e = laptop_con_base(0.5);
        let home = e.home.path();
        let fila = fila_de(TS_COLAB, &huella(), &tarea_hash(home));
        let nombre = nombre_casos(TS_COLAB);
        let v = vuelta(vec![fp(fila, &nombre, &linea_caso(0.5))]);
        let ruta = format!("{CARPETA_CALIBRACION}/{nombre}");
        let (_caja, tgz) = armar_paquete(&v, Some((ruta.as_str(), "alterado\n")));
        let antes = instantanea(home);

        let texto = importar(home, &tgz, "abc123")
            .expect_err("frena la importación")
            .to_string();

        assert!(texto.contains(&ruta), "{texto}");
        assert_eq!(instantanea(home), antes);
        assert!(!home.join("colab/respaldos").exists());
    }

    #[test]
    fn importar_dos_veces_no_duplica_filas_ni_casos() {
        let e = laptop_con_base(0.5);
        let home = e.home.path();
        let fila = fila_de(TS_COLAB, &huella(), &tarea_hash(home));
        let v = vuelta(vec![fp(fila, &nombre_casos(TS_COLAB), &linea_caso(0.5))]);
        let (_caja, tgz) = armar_paquete(&v, None);
        let primera = importar(home, &tgz, "abc123").expect("primera importación");
        assert_eq!(primera.entraron, 1);
        let despues_de_la_primera = instantanea(home);

        let segunda = importar(home, &tgz, "abc123").expect("segunda importación");

        assert_eq!(segunda.entraron, 0, "{segunda:?}");
        assert_eq!(segunda.rechazadas[0].motivo, "ya estaba en la laptop");
        assert_eq!(segunda.casos_copiados, 0);
        assert!(segunda.respaldo.is_none());
        assert_eq!(instantanea(home), despues_de_la_primera);
        let cal = calibra::Calibraciones::cargar(home).expect("calibracion");
        assert_eq!(cal.filas.len(), 2);
    }

    #[test]
    fn commit_distinto_frena_todo() {
        let e = laptop_con_base(0.5);
        let home = e.home.path();
        let fila = fila_de(TS_COLAB, &huella(), &tarea_hash(home));
        let mut v = vuelta(vec![fp(fila, &nombre_casos(TS_COLAB), &linea_caso(0.5))]);
        v.commit = "otro456".into();
        let (_caja, tgz) = armar_paquete(&v, None);
        let antes = instantanea(home);

        let texto = importar(home, &tgz, "abc123")
            .expect_err("frena la importación")
            .to_string();

        assert!(
            texto.contains("otro456") && texto.contains("abc123"),
            "{texto}"
        );
        assert_eq!(instantanea(home), antes);
    }

    #[test]
    fn tarea_hash_distinto_no_entra_y_la_otra_fila_si() {
        let e = laptop_con_base(0.5);
        let home = e.home.path();
        let mala = fila_de(TS_COLAB, &huella(), "hash-falso");
        let buena = fila_de(TS_OTRO, &huella(), &tarea_hash(home));
        let v = vuelta(vec![
            fp(mala, &nombre_casos(TS_COLAB), &linea_caso(0.5)),
            fp(buena, &nombre_casos(TS_OTRO), &linea_caso(0.5)),
        ]);
        let (_caja, tgz) = armar_paquete(&v, None);

        let resumen = importar(home, &tgz, "abc123").expect("importa");

        assert_eq!(resumen.entraron, 1, "{resumen:?}");
        assert_eq!(resumen.rechazadas.len(), 1, "{resumen:?}");
        assert_eq!(
            resumen.rechazadas[0].motivo,
            "el tarea_hash no coincide con el de la laptop"
        );
        assert_eq!(resumen.casos_copiados, 1);
        let cal = calibra::Calibraciones::cargar(home).expect("calibracion");
        assert!(cal.filas.iter().any(|f| f.ts == TS_OTRO));
        assert!(!cal.filas.iter().any(|f| f.ts == TS_COLAB));
        assert!(
            !home
                .join(CARPETA_CALIBRACION)
                .join(nombre_casos(TS_COLAB))
                .exists()
        );
    }

    #[test]
    fn nombre_de_casos_con_puntopunto_frena_todo() {
        let e = laptop_con_base(0.5);
        let home = e.home.path();
        let fila = fila_de(TS_COLAB, &huella(), &tarea_hash(home));
        let v = vuelta(vec![FilaPrueba {
            fila,
            casos: Vec::new(),
            listados_sin_archivo: vec!["../fuera.casos.jsonl".into()],
        }]);
        let (_caja, tgz) = armar_paquete(&v, None);
        let antes = instantanea(home);

        let texto = importar(home, &tgz, "abc123")
            .expect_err("frena la importación")
            .to_string();

        assert!(texto.contains("../fuera.casos.jsonl"), "{texto}");
        assert_eq!(instantanea(home), antes);
        assert!(!home.join("colab/respaldos").exists());
    }

    #[test]
    fn valores_distintos_agrupa_con_tolerancia() {
        assert_eq!(valores_distintos([0.75, 0.5, 0.500_000_1]), vec![0.5, 0.75]);
    }

    #[test]
    fn sin_valores_de_base_en_ningun_lado_no_entra() {
        let h = huella();
        let fila = fila_de(TS_COLAB, &h, "hash");
        let e = Entrada {
            fila: &fila,
            huella_laptop: &h,
            tarea_hash_laptop: Some("hash"),
            ya_estaba: false,
            base: Some(Ok(Valores {
                colab: Vec::new(),
                laptop: Vec::new(),
            })),
        };
        assert_eq!(
            decidir_fila(&e),
            Decision::NoEntra("sin valores de base para comparar".into())
        );
    }
}
