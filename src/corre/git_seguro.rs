//! Repo limpio para el host. El builder escribe un árbol con su propio `.git`. El host no usa ese
//! `.git`: copia solo el árbol de trabajo a un repo nuevo y hace el commit ahí.

use std::fs;
use std::path::Path;

use super::{ErrorCorre, git, infra};

/// Copia `origen` a `destino` sin `.git` ni enlaces simbólicos. Luego hace un commit en `destino`.
/// `destino` no debe existir: la función lo crea.
pub(super) fn repo_limpio_desde(
    origen: &Path,
    destino: &Path,
    mensaje: &str,
) -> Result<(), ErrorCorre> {
    fs::create_dir(destino).map_err(infra(format!("crear {}", destino.display())))?;
    copiar_sin_git(origen, destino)?;
    git(destino, &["init", "-q"])?;
    git(destino, &["config", "user.name", "r00t corre"])?;
    git(destino, &["config", "user.email", "r00t@localhost"])?;
    git(destino, &["config", "commit.gpgsign", "false"])?;
    git(destino, &["add", "-A"])?;
    git(
        destino,
        &[
            "commit",
            "-q",
            "--no-verify",
            "--allow-empty",
            "-m",
            mensaje,
        ],
    )
}

/// Copia carpetas y archivos de `origen` a `destino`. Omite cada `.git` y cada enlace simbólico.
/// Otros tipos, como sockets y fifos, también se omiten.
fn copiar_sin_git(origen: &Path, destino: &Path) -> Result<(), ErrorCorre> {
    let contexto = format!("leer {}", origen.display());
    let entradas = fs::read_dir(origen).map_err(infra(contexto.clone()))?;
    for entrada in entradas {
        let entrada = entrada.map_err(infra(contexto.clone()))?;
        let nombre = entrada.file_name();
        if nombre == ".git" {
            continue;
        }
        let origen_hijo = entrada.path();
        let destino_hijo = destino.join(&nombre);
        // `file_type` no sigue enlaces: un enlace no es carpeta ni archivo regular.
        let tipo = entrada
            .file_type()
            .map_err(infra(format!("leer el tipo de {}", origen_hijo.display())))?;
        if tipo.is_dir() {
            fs::create_dir(&destino_hijo)
                .map_err(infra(format!("crear {}", destino_hijo.display())))?;
            copiar_sin_git(&origen_hijo, &destino_hijo)?;
        } else if tipo.is_file() {
            fs::copy(&origen_hijo, &destino_hijo)
                .map_err(infra(format!("copiar {}", origen_hijo.display())))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oracle::comando_git;
    use tempfile::TempDir;

    /// Salida de `git -C destino <args>` sin variables de repo. Falla el test si git falla.
    fn git_en(destino: &Path, args: &[&str]) -> String {
        let salida = comando_git(destino, args).output().expect("correr git");
        assert!(salida.status.success(), "git {args:?}: {salida:?}");
        String::from_utf8_lossy(&salida.stdout).trim().to_string()
    }

    /// Escribe `contenido` en `ruta`, creando las carpetas que falten.
    fn escribir(ruta: &Path, contenido: &str) {
        fs::create_dir_all(ruta.parent().expect("padre")).expect("crear carpetas");
        fs::write(ruta, contenido).expect("escribir archivo");
    }

    #[test]
    fn no_copia_el_git_del_origen_ni_ejecuta_su_fsmonitor() {
        let origen = TempDir::new().expect("origen");
        let trampa = TempDir::new().expect("trampa");
        let marca = trampa.path().join("marca");
        escribir(
            &origen.path().join(".git").join("config"),
            &format!("[core]\n\tfsmonitor = touch {}\n", marca.display()),
        );
        escribir(&origen.path().join("src").join("main.rs"), "fn main() {}\n");
        let base = TempDir::new().expect("base");
        let destino = base.path().join("repo");

        repo_limpio_desde(origen.path(), &destino, "limpio").expect("repo limpio");

        assert!(!marca.exists(), "el fsmonitor del origen se ejecutó");
        let config = fs::read_to_string(destino.join(".git").join("config")).expect("config");
        assert!(!config.contains("fsmonitor"), "config heredada: {config}");
    }

    #[cfg(unix)]
    #[test]
    fn no_copia_enlaces_simbolicos() {
        let origen = TempDir::new().expect("origen");
        let fuera = TempDir::new().expect("fuera");
        let secreto = fuera.path().join("secreto.txt");
        escribir(&secreto, "no copiar\n");
        escribir(&origen.path().join("normal.txt"), "copiar\n");
        std::os::unix::fs::symlink(&secreto, origen.path().join("enlace")).expect("crear enlace");
        let base = TempDir::new().expect("base");
        let destino = base.path().join("repo");

        repo_limpio_desde(origen.path(), &destino, "limpio").expect("repo limpio");

        assert!(
            fs::symlink_metadata(destino.join("enlace")).is_err(),
            "el enlace se copió"
        );
        assert!(
            destino.join("normal.txt").is_file(),
            "el archivo normal no se copió"
        );
    }

    #[test]
    fn copia_archivos_y_carpetas_anidadas_y_hace_un_commit() {
        let origen = TempDir::new().expect("origen");
        escribir(&origen.path().join("a.txt"), "hola\n");
        escribir(
            &origen.path().join("sub").join("deep").join("b.txt"),
            "mundo\n",
        );
        let base = TempDir::new().expect("base");
        let destino = base.path().join("repo");

        repo_limpio_desde(origen.path(), &destino, "bootstrap limpio").expect("repo limpio");

        assert_eq!(
            fs::read_to_string(destino.join("a.txt")).expect("a.txt"),
            "hola\n"
        );
        assert_eq!(
            fs::read_to_string(destino.join("sub").join("deep").join("b.txt")).expect("b.txt"),
            "mundo\n"
        );
        assert_eq!(git_en(&destino, &["rev-list", "--count", "HEAD"]), "1");
        assert_eq!(
            git_en(&destino, &["log", "-1", "--format=%s"]),
            "bootstrap limpio"
        );
    }

    #[test]
    fn no_copia_un_git_anidado_en_una_subcarpeta() {
        let origen = TempDir::new().expect("origen");
        escribir(
            &origen.path().join("sub").join(".git").join("HEAD"),
            "ref: x\n",
        );
        escribir(&origen.path().join("sub").join("x.txt"), "x\n");
        let base = TempDir::new().expect("base");
        let destino = base.path().join("repo");

        repo_limpio_desde(origen.path(), &destino, "limpio").expect("repo limpio");

        assert!(
            fs::symlink_metadata(destino.join("sub").join(".git")).is_err(),
            "el .git anidado se copió"
        );
        assert!(destino.join("sub").join("x.txt").is_file());
    }

    #[test]
    fn falla_si_el_destino_ya_existe() {
        let origen = TempDir::new().expect("origen");
        let base = TempDir::new().expect("base");
        let destino = base.path().join("repo");
        fs::create_dir(&destino).expect("destino previo");

        assert!(repo_limpio_desde(origen.path(), &destino, "limpio").is_err());
    }
}
