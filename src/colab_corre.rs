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
    EXCLUIDOS, ROOT_PODMAN_BIN, Resumen, es_elf_x86_64, revisar_root_toml, sha256_hex,
    version_de_rustc,
};
use crate::config::root_home;
use crate::corre::ErrorCorre;
use crate::corre::config::{ConfigCorrida, ruta_de};
use crate::observe::ruta_eventos_en;
use crate::oracle::ErrorOraculo;
use crate::oracle::pendientes::nombre_de_tarea_valido;

/// Imagen con `claude` y `rustc` que lee el paquete.
const IMAGEN_BUILDER: &str = "root-builder:1";
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
    empaquetar_corre(&root_home(), &raiz_root, &commit, &pedido, &PodmanBuilder)
}

/// Arma el paquete de `corre` con los datos dados. Los tests llaman a esta función.
pub fn empaquetar_corre(
    home: &Path,
    raiz_root: &Path,
    commit: &str,
    pedido: &PedidoEmpaquetarCorre,
    imagen: &dyn ImagenBuilder,
) -> Result<Resumen, ErrorOraculo> {
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
fn armar(a: &Armado, imagen: &dyn ImagenBuilder) -> Result<Resumen, ErrorOraculo> {
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
            }
        }

        fn empaqueta(&self, pedido: &PedidoEmpaquetarCorre) -> Result<Resumen, ErrorOraculo> {
            empaquetar_corre(self.home.path(), self.raiz.path(), "abc123", pedido, &Falso)
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

    /// Paquete falso, ROOT_HOME falso, token en un temporal y r00t falso.
    struct Corrida {
        raiz: tempfile::TempDir,
        paquete: PathBuf,
        home: PathBuf,
        token: PathBuf,
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
                r00t: raiz.path().join("r00t"),
                fuera: raiz.path().join("fuera"),
                raiz,
            };
            for dir in [&c.paquete, &c.home, &c.fuera] {
                fs::create_dir_all(dir).expect("crea carpeta");
            }
            let lista: Vec<String> = tareas.iter().map(|t| format!("\"{t}\"")).collect();
            let manifiesto = format!(
                "{{\"v\":1,\"tipo\":\"corre\",\"pasada\":1,\"config\":\"cfg\",\"tareas\":[{}],\"plugins\":{{}},\"archivos\":{{}}}}\n",
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
            Command::new("python3")
                .arg("-I")
                .arg(CORRE)
                .arg(&self.paquete)
                .current_dir(self.raiz.path())
                .env("ROOT_CORRE_R00T", &self.r00t)
                .env("ROOT_CORRE_HOME", &self.home)
                .env("ROOT_CORRE_TOKEN", &self.token)
                .env("ROOT_CORRE_PLAZO_S", plazo)
                .env("DEEPSEEK_API_KEY", CLAVE)
                .env("ROOT_TOKEN_CUPO", CUPO)
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
