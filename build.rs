//! Guarda en el binario el commit con el que se compiló `r00t`.
//!
//! Exporta `ROOT_COMMIT_BINARIO` con el `HEAD` completo. Lleva el sufijo `-sucio` si hay cambios
//! en archivos con seguimiento. Vale `desconocido` si no hay git o si git falla.
//!
//! Solo cuentan los archivos con seguimiento, modificados o en el índice. Un archivo sin
//! seguimiento no marca `-sucio`.
//!
//! Límite: el sufijo se calcula solo al compilar. Editar un archivo con seguimiento sin
//! `git add` no vuelve a correr este script. El sufijo queda viejo hasta el próximo cambio
//! en `HEAD` o en el índice.
//!
//! Las rutas de git van con `git rev-parse --git-path`. Así funciona también en un worktree,
//! donde `.git` es un archivo.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Variables que pueden apuntar git a otro repo. Espejo de `VARIABLES_GIT_DE_REPO` en src/oracle/mod.rs.
const VARIABLES_GIT_DE_REPO: &[&str] = &[
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_COMMON_DIR",
    "GIT_NAMESPACE",
    "GIT_PREFIX",
    "GIT_CEILING_DIRECTORIES",
    "GIT_DISCOVERY_ACROSS_FILESYSTEM",
];

fn main() {
    let dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap_or_else(|| ".".into()));
    for ruta in rutas_a_vigilar(&dir) {
        println!("cargo:rerun-if-changed={}", ruta.display());
    }
    let valor = commit_binario(&dir).unwrap_or_else(|| "desconocido".into());
    println!("cargo:rustc-env=ROOT_COMMIT_BINARIO={valor}");
}

/// Corre `git` en `dir` sin variables de repo heredadas. Sin éxito, `None`.
fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let mut cmd = Command::new("git");
    cmd.current_dir(dir).args(args);
    for nombre in VARIABLES_GIT_DE_REPO {
        cmd.env_remove(nombre);
    }
    let salida = cmd.output().ok()?;
    if !salida.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&salida.stdout).trim().to_string())
}

/// `HEAD` completo, con `-sucio` si hay cambios en archivos con seguimiento. Sin git, `None`.
fn commit_binario(dir: &Path) -> Option<String> {
    let commit = git(dir, &["rev-parse", "HEAD"]).filter(|c| !c.is_empty())?;
    let cambios = git(dir, &["status", "--porcelain", "--untracked-files=no"])?;
    if cambios.is_empty() {
        Some(commit)
    } else {
        Some(format!("{commit}-sucio"))
    }
}

/// Rutas que cambian al mover `HEAD`, al stagear y al hacer commit en la rama.
/// Son rutas de git, relativas al directorio del crate. Se devuelven absolutas.
fn rutas_a_vigilar(dir: &Path) -> Vec<PathBuf> {
    let mut nombres = vec!["HEAD".to_string(), "index".to_string()];
    if let Some(referencia) = git(dir, &["symbolic-ref", "-q", "HEAD"]) {
        nombres.push(referencia);
    }
    let mut rutas: Vec<PathBuf> = nombres
        .iter()
        .filter_map(|nombre| git(dir, &["rev-parse", "--git-path", nombre]))
        .map(|ruta| dir.join(ruta))
        .collect();
    // `packed-refs` solo existe si hay refs empaquetadas. Si no existe, no se vigila.
    if let Some(empaquetadas) = git(dir, &["rev-parse", "--git-path", "packed-refs"]) {
        let ruta = dir.join(empaquetadas);
        if ruta.exists() {
            rutas.push(ruta);
        }
    }
    rutas
}
