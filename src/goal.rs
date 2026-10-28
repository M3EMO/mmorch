//! Hash del GOAL: `GOAL.md` autorizado por un humano con `GOAL.hash`.

use std::io::{self, BufRead, IsTerminal, Write};
use std::path::Path;

use sha2::{Digest, Sha256};
use thiserror::Error as ErrorDerive;

/// Errores del GOAL. Cada variante tiene su código de salida.
#[derive(Debug, ErrorDerive)]
pub enum ErrorGoal {
    /// Falta `GOAL.md` o `GOAL.hash`. Sale con código 3.
    #[error("GOAL no autorizado: falta {0}")]
    NoAutorizado(String),
    /// El hash de `GOAL.md` no coincide con `GOAL.hash`. Sale con código 1.
    #[error("GOAL alterado: el hash {obtenido} no coincide con {esperado}")]
    Alterado { esperado: String, obtenido: String },
    /// `autorizar` necesita una terminal. Sale con código 2.
    #[error("uso: autorizar exige una terminal interactiva")]
    SinTerminal,
    /// La confirmación tecleada no coincide con el hash. Sale con código 1.
    #[error("confirmación incorrecta: no se escribió GOAL.hash (esperado {esperado})")]
    ConfirmacionIncorrecta { esperado: String },
    #[error("error de E/S")]
    Io(#[from] io::Error),
}

impl ErrorGoal {
    /// Código de salida del proceso para este error.
    pub fn codigo_de_salida(&self) -> u8 {
        match self {
            ErrorGoal::SinTerminal => 2,
            ErrorGoal::Alterado { .. } | ErrorGoal::ConfirmacionIncorrecta { .. } => 1,
            ErrorGoal::NoAutorizado(_) | ErrorGoal::Io(_) => 3,
        }
    }
}

/// Hash de un contenido: sha256 en hex, primeros 16 caracteres.
pub fn calcular_hash(contenido: &[u8]) -> String {
    Sha256::digest(contenido)
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Hash de `GOAL.md` del repo.
pub fn hash_del_repo(repo: &Path) -> Result<String, ErrorGoal> {
    let goal = leer_archivo(&repo.join("GOAL.md"), "GOAL.md")?;
    Ok(calcular_hash(&goal))
}

/// Verifica que `GOAL.md` coincida con `GOAL.hash`.
pub fn verificar_goal(repo: &Path) -> Result<(), ErrorGoal> {
    let goal = leer_archivo(&repo.join("GOAL.md"), "GOAL.md")?;
    let esperado = leer_hash_guardado(repo)?;
    let obtenido = calcular_hash(&goal);
    if obtenido == esperado {
        Ok(())
    } else {
        Err(ErrorGoal::Alterado { esperado, obtenido })
    }
}

/// Pide confirmación tecleada y escribe `GOAL.hash`. Sin TTY no hace nada.
pub fn autorizar_goal(
    repo: &Path,
    es_tty: bool,
    entrada: &mut dyn BufRead,
    salida: &mut dyn Write,
) -> Result<(), ErrorGoal> {
    if !es_tty {
        return Err(ErrorGoal::SinTerminal);
    }
    let hash = hash_del_repo(repo)?;
    writeln!(salida, "Hash de GOAL.md: {hash}")?;
    write!(salida, "Teclea ese hash para autorizar: ")?;
    salida.flush()?;

    let mut linea = String::new();
    entrada.read_line(&mut linea)?;
    if linea.trim_end_matches(['\r', '\n']) != hash {
        return Err(ErrorGoal::ConfirmacionIncorrecta { esperado: hash });
    }

    std::fs::write(repo.join("GOAL.hash"), format!("{hash}\n"))?;
    writeln!(salida, "GOAL autorizado: {hash}")?;
    Ok(())
}

/// Versión para la CLI: usa la terminal real.
pub fn autorizar(repo: &Path) -> Result<(), ErrorGoal> {
    let es_tty = io::stdin().is_terminal() && io::stdout().is_terminal();
    let stdin = io::stdin();
    let mut entrada = stdin.lock();
    let stdout = io::stdout();
    let mut salida = stdout.lock();
    autorizar_goal(repo, es_tty, &mut entrada, &mut salida)
}

/// Lee un archivo del repo. Si falta, el GOAL no está autorizado (código 3).
fn leer_archivo(ruta: &Path, nombre: &str) -> Result<Vec<u8>, ErrorGoal> {
    match std::fs::read(ruta) {
        Ok(bytes) => Ok(bytes),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            Err(ErrorGoal::NoAutorizado(nombre.to_string()))
        }
        Err(e) => Err(ErrorGoal::Io(e)),
    }
}

/// Lee `GOAL.hash` y recorta los espacios y saltos de línea.
fn leer_hash_guardado(repo: &Path) -> Result<String, ErrorGoal> {
    let bytes = leer_archivo(&repo.join("GOAL.hash"), "GOAL.hash")?;
    Ok(String::from_utf8_lossy(&bytes).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Cursor;
    use tempfile::TempDir;

    const GOAL_FIXTURE: &str = "# GOAL de prueba\n\nContenido de fixture.\n";

    fn repo_con_goal() -> TempDir {
        let dir = tempfile::tempdir().expect("no se pudo crear el directorio temporal");
        fs::write(dir.path().join("GOAL.md"), GOAL_FIXTURE).expect("no se pudo escribir GOAL.md");
        dir
    }

    fn hash_fixture() -> String {
        calcular_hash(GOAL_FIXTURE.as_bytes())
    }

    #[test]
    fn goal_calcular_hash_de_hola_coincide_con_sha256sum() {
        assert_eq!(calcular_hash(b"hola"), "b221d9dbb083a7f3");
    }

    #[test]
    fn goal_calcular_hash_tiene_16_hex_minusculas() {
        let hash = calcular_hash(b"cualquier contenido");
        assert_eq!(hash.len(), 16);
        assert!(
            hash.chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
        );
    }

    #[test]
    fn goal_verificar_con_hash_valido_es_ok() {
        let dir = repo_con_goal();
        fs::write(
            dir.path().join("GOAL.hash"),
            format!("{}\n", hash_fixture()),
        )
        .expect("no se pudo escribir GOAL.hash");
        assert!(verificar_goal(dir.path()).is_ok());
    }

    #[test]
    fn goal_verificar_con_goal_alterado_da_codigo_1() {
        let dir = repo_con_goal();
        fs::write(
            dir.path().join("GOAL.hash"),
            format!("{}\n", hash_fixture()),
        )
        .expect("no se pudo escribir GOAL.hash");
        fs::write(dir.path().join("GOAL.md"), "GOAL editado a mano\n")
            .expect("no se pudo alterar GOAL.md");
        let error = verificar_goal(dir.path()).expect_err("un GOAL alterado debe fallar");
        assert_eq!(error.codigo_de_salida(), 1);
    }

    #[test]
    fn goal_verificar_sin_goal_md_da_codigo_3() {
        let dir = tempfile::tempdir().expect("temp dir");
        fs::write(
            dir.path().join("GOAL.hash"),
            format!("{}\n", hash_fixture()),
        )
        .expect("no se pudo escribir GOAL.hash");
        let error = verificar_goal(dir.path()).expect_err("sin GOAL.md debe fallar");
        assert_eq!(error.codigo_de_salida(), 3);
    }

    #[test]
    fn goal_verificar_sin_goal_hash_da_codigo_3() {
        let dir = repo_con_goal();
        let error = verificar_goal(dir.path()).expect_err("sin GOAL.hash debe fallar");
        assert_eq!(error.codigo_de_salida(), 3);
    }

    #[test]
    fn goal_autorizar_sin_tty_da_codigo_2_y_no_escribe() {
        let dir = repo_con_goal();
        let mut entrada = Cursor::new(format!("{}\n", hash_fixture()).into_bytes());
        let mut salida: Vec<u8> = Vec::new();
        let error = autorizar_goal(dir.path(), false, &mut entrada, &mut salida)
            .expect_err("sin TTY debe fallar");
        assert_eq!(error.codigo_de_salida(), 2);
        assert!(!dir.path().join("GOAL.hash").exists());
        assert!(salida.is_empty());
    }

    #[test]
    fn goal_autorizar_con_tty_y_entrada_correcta_escribe_hash() {
        let dir = repo_con_goal();
        let hash = hash_fixture();
        let mut entrada = Cursor::new(format!("{hash}\n").into_bytes());
        let mut salida: Vec<u8> = Vec::new();
        autorizar_goal(dir.path(), true, &mut entrada, &mut salida)
            .expect("la confirmación correcta debe autorizar");
        let escrito =
            fs::read_to_string(dir.path().join("GOAL.hash")).expect("GOAL.hash debe existir");
        assert_eq!(escrito, format!("{hash}\n"));
        assert!(String::from_utf8_lossy(&salida).contains(&hash));
        assert!(verificar_goal(dir.path()).is_ok());
    }

    #[test]
    fn goal_autorizar_con_tty_y_entrada_incorrecta_no_escribe() {
        let dir = repo_con_goal();
        let mut entrada = Cursor::new(b"0000000000000000\n".to_vec());
        let mut salida: Vec<u8> = Vec::new();
        let error = autorizar_goal(dir.path(), true, &mut entrada, &mut salida)
            .expect_err("una confirmación incorrecta debe fallar");
        assert_eq!(error.codigo_de_salida(), 1);
        assert!(!dir.path().join("GOAL.hash").exists());
    }
}
