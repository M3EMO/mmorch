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
