//! Corre: arma y corre la corrida de un builder (Tarea 16). Arma la carpeta fuera de `ROOT_HOME`, el
//! repo del bootstrap, los argumentos de `claude -p` por rol y el transcript. `correr` rutea, lanza
//! cada rol, registra las sesiones, evalúa y agrega la fila. El builder se lanza por `EjecutorBuilder`,
//! nunca directo.

pub mod args;
pub mod config;
pub mod contenedor;
pub mod corrida;
pub mod ejecutor;

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error as ErrorDerive;

use crate::borrado;
use crate::error::Error;
use crate::goal::ErrorGoal;
use crate::oracle::pendientes::nombre_de_tarea_valido;
use crate::oracle::{comando_git, commit_de};
use crate::route::politica::nombre_rol;
use crate::tipos::Rol;

pub use args::{args_claude, env_del_builder};
pub use config::{ClaudeCorrida, ConfigCorrida};
pub use corrida::{DepsCorre, correr};
pub use ejecutor::{EjecutorBuilder, EjecutorContenedorBuilder, Lanzamiento, SalidaClaude};

/// Errores de una corrida. `Uso` sale con código 2: lo arregla el humano en la config o en la tarea.
/// `Goal` y `Ruteo` llevan el código de su error. Los demás salen con código 3.
#[derive(Debug, ErrorDerive)]
pub enum ErrorCorre {
    #[error("uso: {0}")]
    Uso(String),
    #[error("{contexto}")]
    Io {
        contexto: String,
        #[source]
        fuente: io::Error,
    },
    #[error("git: {0}")]
    Git(String),
    #[error("no se pudo lanzar el builder: {0}")]
    Ejecutor(String),
    /// El GOAL no está autorizado o fue alterado. Sale con el código de `goal verificar`.
    #[error(transparent)]
    Goal(#[from] ErrorGoal),
    /// `route` no eligió la vía de un rol. Sale con el código de ese error.
    #[error(transparent)]
    Ruteo(Box<Error>),
    /// Leer o registrar eventos, o armar las huellas del oráculo. Sale con código 3.
    #[error("observe: {0}")]
    Observa(String),
}

impl ErrorCorre {
    /// Código de salida: 2 para `Uso`, el de su error para `Goal` y `Ruteo`, 3 para el resto.
    pub fn codigo_de_salida(&self) -> u8 {
        match self {
            ErrorCorre::Uso(_) => 2,
            ErrorCorre::Goal(e) => e.codigo_de_salida(),
            ErrorCorre::Ruteo(e) => e.codigo_de_salida(),
            ErrorCorre::Io { .. }
            | ErrorCorre::Git(_)
            | ErrorCorre::Ejecutor(_)
            | ErrorCorre::Observa(_) => 3,
        }
    }
}

/// Corrida armada: su carpeta, su repo y el commit del bootstrap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Corrida {
    /// Id `corre-<tarea>-<config>-<segundos>-<pid>-<contador>`.
    pub id: String,
    /// Carpeta de la corrida: `<dir_corridas>/<id>`. Fuera de `ROOT_HOME` (desempate 39).
    pub dir: PathBuf,
    /// Repo del builder: `<dir>/repo`.
    pub repo: PathBuf,
    pub tarea: String,
    pub config: String,
    /// Commit corto del bootstrap. Vacío si git no lo da.
    pub commit: String,
}

/// Contador de ids dentro del proceso. Con el pid, el id no choca con otra corrida.
static CONTADOR: AtomicU64 = AtomicU64::new(0);

/// Carpeta de las corridas: `<home_usuario>/.cache/root/corridas/`. Sale de `$HOME` en producción.
pub fn dir_corridas(home_usuario: &Path) -> PathBuf {
    home_usuario.join(".cache").join("root").join("corridas")
}

/// Id de una corrida. Lleva la tarea, la config, los segundos de inicio, el pid y un contador.
pub fn id_corrida(tarea: &str, config: &str, ahora_s: u64) -> String {
    let contador = CONTADOR.fetch_add(1, Ordering::Relaxed);
    format!(
        "corre-{tarea}-{config}-{ahora_s}-{}-{contador}",
        std::process::id()
    )
}

/// Arma la corrida: `<dir_corridas>/<id>/repo/` con el bootstrap, el contrato en `tests/` y un commit.
/// La carpeta de corridas no puede quedar dentro de `home` (la de `ROOT_HOME`). Si el armado falla a
/// medias, borra la carpeta de la corrida.
pub fn preparar_corrida(
    home: &Path,
    home_usuario: &Path,
    tarea: &str,
    config: &str,
) -> Result<Corrida, ErrorCorre> {
    nombre_de_tarea_valido(tarea).map_err(|m| ErrorCorre::Uso(format!("tarea: {m}")))?;
    nombre_de_tarea_valido(config).map_err(|m| ErrorCorre::Uso(format!("config: {m}")))?;
    let raiz = dir_corridas(home_usuario);
    let raiz_root = ruta_fisica(home).map_err(infra("resolver ROOT_HOME"))?;
    let raiz_fisica = ruta_fisica(&raiz).map_err(infra("resolver la carpeta de corridas"))?;
    if raiz_fisica.starts_with(&raiz_root) {
        return Err(ErrorCorre::Uso(format!(
            "la carpeta de corridas {} queda dentro de ROOT_HOME {}",
            raiz_fisica.display(),
            raiz_root.display()
        )));
    }
    let id = id_corrida(tarea, config, ahora_s());
    let dir = raiz.join(&id);
    fs::create_dir_all(&raiz).map_err(infra("crear la carpeta de corridas"))?;
    fs::create_dir(&dir).map_err(infra("crear la carpeta de la corrida"))?;
    match armar(home, &dir, tarea) {
        Ok(commit) => Ok(Corrida {
            id,
            repo: dir.join("repo"),
            dir,
            tarea: tarea.to_string(),
            config: config.to_string(),
            commit,
        }),
        Err(error) => {
            // El armado falló a medias: la carpeta se borra. Un fallo al borrar no tapa el error original.
            let _ = borrado::borrar_dir_dentro(&raiz, &dir);
            Err(error)
        }
    }
}

/// Escribe el stdout de `claude` en `<home>/transcripts/<id>/<n>-<rol>.jsonl`, de forma atómica.
/// Va fuera de la carpeta de la corrida, así el builder no lo ve (Tarea 26, capa 2).
pub fn guardar_transcript(
    home: &Path,
    id: &str,
    n: u32,
    rol: Rol,
    stdout: &[u8],
) -> Result<PathBuf, ErrorCorre> {
    nombre_de_tarea_valido(id).map_err(|m| ErrorCorre::Uso(format!("id: {m}")))?;
    let carpeta = home.join("transcripts").join(id);
    fs::create_dir_all(&carpeta).map_err(infra("crear la carpeta de transcripts"))?;
    let destino = carpeta.join(format!("{n}-{}.jsonl", nombre_rol(rol)));
    let mut temporal = tempfile::NamedTempFile::new_in(&carpeta)
        .map_err(infra("crear el temporal del transcript"))?;
    temporal
        .write_all(stdout)
        .map_err(infra("escribir el transcript"))?;
    temporal
        .as_file()
        .sync_all()
        .map_err(infra("escribir el transcript"))?;
    temporal
        .persist(&destino)
        .map_err(|e| infra("guardar el transcript")(e.error))?;
    Ok(destino)
}

/// Arma el repo de la corrida dentro de `dir`: bootstrap, contrato en `tests/` y el commit inicial.
/// Devuelve el commit corto.
fn armar(home: &Path, dir: &Path, tarea: &str) -> Result<String, ErrorCorre> {
    let oraculo = home.join("oraculos").join(tarea);
    let bootstrap = oraculo.join("bootstrap");
    let contrato = oraculo.join("contrato");
    carpeta_sin_enlaces(&bootstrap, "bootstrap")?;
    carpeta_sin_enlaces(&contrato, "contrato")?;
    if bootstrap.join(".git").symlink_metadata().is_ok() {
        return Err(ErrorCorre::Uso(format!(
            "el bootstrap trae .git: la corrida crea su propio historial ({})",
            bootstrap.display()
        )));
    }
    let repo = dir.join("repo");
    fs::create_dir(&repo).map_err(infra("crear el repo de la corrida"))?;
    copiar_arbol(&bootstrap, &repo)?;
    let tests = repo.join("tests");
    match fs::symlink_metadata(&tests) {
        Ok(meta) if meta.is_dir() => {}
        Ok(_) => {
            return Err(ErrorCorre::Uso(format!(
                "ya existe y no es carpeta: {}",
                tests.display()
            )));
        }
        Err(_) => fs::create_dir(&tests).map_err(infra("crear tests/ de la corrida"))?,
    }
    copiar_arbol(&contrato, &tests)?;
    iniciar_git(&repo)?;
    Ok(commit_de(&repo))
}

/// Revisa que `ruta` sea una carpeta real: sin enlace simbólico y sin faltar.
fn carpeta_sin_enlaces(ruta: &Path, nombre: &str) -> Result<(), ErrorCorre> {
    let meta = match fs::symlink_metadata(ruta) {
        Ok(meta) => meta,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Err(ErrorCorre::Uso(format!(
                "falta la carpeta {nombre} de la tarea: {}",
                ruta.display()
            )));
        }
        Err(e) => return Err(infra(format!("leer {}", ruta.display()))(e)),
    };
    if meta.file_type().is_symlink() {
        return Err(ErrorCorre::Uso(format!(
            "{nombre} es un enlace simbólico: {}",
            ruta.display()
        )));
    }
    if !meta.is_dir() {
        return Err(ErrorCorre::Uso(format!(
            "{nombre} no es una carpeta: {}",
            ruta.display()
        )));
    }
    Ok(())
}

/// Copia `origen` dentro de `destino` sin seguir enlaces. Un enlace simbólico es error. Un archivo que ya
/// existe en `destino` es error. Una carpeta que ya existe se combina.
fn copiar_arbol(origen: &Path, destino: &Path) -> Result<(), ErrorCorre> {
    let entradas = fs::read_dir(origen).map_err(infra(format!("leer {}", origen.display())))?;
    for entrada in entradas {
        let entrada = entrada.map_err(infra(format!("leer {}", origen.display())))?;
        let tipo = entrada.file_type().map_err(infra(format!(
            "leer el tipo de {}",
            entrada.path().display()
        )))?;
        let origen_hijo = entrada.path();
        let destino_hijo = destino.join(entrada.file_name());
        if tipo.is_symlink() {
            return Err(ErrorCorre::Uso(format!(
                "enlace simbólico en {}: no se copia",
                origen_hijo.display()
            )));
        } else if tipo.is_dir() {
            match fs::symlink_metadata(&destino_hijo) {
                Ok(meta) if meta.is_dir() => {}
                Ok(_) => {
                    return Err(ErrorCorre::Uso(format!(
                        "ya existe y no es carpeta: {}",
                        destino_hijo.display()
                    )));
                }
                Err(_) => fs::create_dir(&destino_hijo)
                    .map_err(infra(format!("crear {}", destino_hijo.display())))?,
            }
            copiar_arbol(&origen_hijo, &destino_hijo)?;
        } else if tipo.is_file() {
            if fs::symlink_metadata(&destino_hijo).is_ok() {
                return Err(ErrorCorre::Uso(format!(
                    "ya existe en el repo: {}",
                    destino_hijo.display()
                )));
            }
            fs::copy(&origen_hijo, &destino_hijo)
                .map_err(infra(format!("copiar {}", origen_hijo.display())))?;
        } else {
            return Err(ErrorCorre::Uso(format!(
                "tipo de archivo no soportado: {}",
                origen_hijo.display()
            )));
        }
    }
    Ok(())
}

/// Inicia el repo de la corrida con un commit "bootstrap". Usuario y correo son locales: la corrida no
/// usa la identidad del humano. Sin hooks: el repo es nuevo y no debe ejecutar nada ajeno.
fn iniciar_git(repo: &Path) -> Result<(), ErrorCorre> {
    git(repo, &["init", "-q"])?;
    git(repo, &["config", "user.name", "r00t corre"])?;
    git(repo, &["config", "user.email", "r00t@localhost"])?;
    git(repo, &["config", "commit.gpgsign", "false"])?;
    git(repo, &["add", "-A"])?;
    git(
        repo,
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "commit",
            "-q",
            "--no-verify",
            "--allow-empty",
            "-m",
            "bootstrap",
        ],
    )
}

/// Corre `git -C repo <args>` sin las variables `GIT_*` heredadas. Un fallo sale con la salida de git.
fn git(repo: &Path, args: &[&str]) -> Result<(), ErrorCorre> {
    let salida = comando_git(repo, args)
        .output()
        .map_err(|e| ErrorCorre::Git(format!("no se pudo correr git {}: {e}", args.join(" "))))?;
    if salida.status.success() {
        return Ok(());
    }
    Err(ErrorCorre::Git(format!(
        "git {}: {}",
        args.join(" "),
        String::from_utf8_lossy(&salida.stderr).trim()
    )))
}

/// Ruta física de `ruta`, aunque no exista: resuelve el ancestro que existe y le agrega el resto.
fn ruta_fisica(ruta: &Path) -> io::Result<PathBuf> {
    let mut pendientes: Vec<&std::ffi::OsStr> = Vec::new();
    let mut actual = ruta;
    loop {
        match fs::canonicalize(actual) {
            Ok(fisica) => {
                return Ok(pendientes
                    .iter()
                    .rev()
                    .fold(fisica, |acumulada, nombre| acumulada.join(nombre)));
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let (Some(padre), Some(nombre)) = (actual.parent(), actual.file_name()) else {
                    return Err(e);
                };
                pendientes.push(nombre);
                actual = padre;
            }
            Err(e) => return Err(e),
        }
    }
}

/// Segundos desde la época. Reloj anterior a 1970 cuenta como 0.
fn ahora_s() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Convierte un `io::Error` en `ErrorCorre::Io` con su contexto.
fn infra(contexto: impl Into<String>) -> impl FnOnce(io::Error) -> ErrorCorre {
    let contexto = contexto.into();
    move |fuente| ErrorCorre::Io { contexto, fuente }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;
    use std::process::Command;
    use tempfile::TempDir;

    /// Texto único del test oculto de la fixture. No debe aparecer en ningún archivo del repo.
    const SECRETO: &str = "SECRETO_ORACULO_7f3a_linea_unica_del_oculto";

    /// Arma una tarea `memo` dentro de `home`: bootstrap, contrato, oculto y `tarea.toml`.
    fn armar_home(home: &Path) {
        let tarea = home.join("oraculos").join("memo");
        fs::create_dir_all(tarea.join("bootstrap").join("docs")).expect("bootstrap");
        fs::write(
            tarea.join("bootstrap").join("docs").join("SPEC.md"),
            "# Spec\n",
        )
        .expect("spec");
        fs::write(tarea.join("bootstrap").join("Cargo.toml"), "[package]\n").expect("cargo");
        fs::create_dir_all(tarea.join("contrato")).expect("contrato");
        fs::write(
            tarea.join("contrato").join("t_contrato.py"),
            "def test_contrato(): pass\n",
        )
        .expect("contrato");
        fs::create_dir_all(tarea.join("oculto")).expect("oculto");
        fs::write(
            tarea.join("oculto").join("t_oculto.py"),
            format!("{SECRETO}\n"),
        )
        .expect("oculto");
        fs::write(tarea.join("tarea.toml"), "nombre = \"memo\"\n").expect("tarea.toml");
    }

    /// `ROOT_HOME` en `<tmp>/root` con la tarea, y `$HOME` en `<tmp>/usuario`.
    fn entorno(tmp: &Path) -> (PathBuf, PathBuf) {
        let home = tmp.join("root");
        armar_home(&home);
        let usuario = tmp.join("usuario");
        fs::create_dir_all(&usuario).expect("home de usuario");
        (home, usuario)
    }

    /// Todos los archivos bajo `dir`, sin entrar a `.git`.
    fn archivos(dir: &Path) -> Vec<PathBuf> {
        let mut salida = Vec::new();
        for entrada in fs::read_dir(dir).expect("lee") {
            let ruta = entrada.expect("entrada").path();
            if ruta.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            if ruta.is_dir() {
                salida.extend(archivos(&ruta));
            } else {
                salida.push(ruta);
            }
        }
        salida
    }

    #[test]
    fn corre_dir_corridas_cuelga_de_home_de_usuario() {
        assert_eq!(
            dir_corridas(Path::new("/h/u")),
            PathBuf::from("/h/u/.cache/root/corridas")
        );
    }

    #[test]
    fn corre_id_tiene_la_forma_pedida_y_no_se_repite() {
        let a = id_corrida("memora", "ecc-haiku", 1_700_000_000);
        let b = id_corrida("memora", "ecc-haiku", 1_700_000_000);
        assert!(
            a.starts_with("corre-memora-ecc-haiku-1700000000-"),
            "id {a}"
        );
        assert_ne!(a, b);
    }

    #[test]
    fn corre_preparar_rechaza_corridas_dentro_de_root_home() {
        // ROOT_HOME = <tmp>/.cache/root y HOME = <tmp>: la carpeta de corridas cae dentro de ROOT_HOME.
        let tmp = TempDir::new().expect("tmp");
        let home = tmp.path().join(".cache").join("root");
        armar_home(&home);
        let error =
            preparar_corrida(&home, tmp.path(), "memo", "ecc-haiku").expect_err("debe fallar");
        assert!(
            matches!(error, ErrorCorre::Uso(ref m) if m.contains("ROOT_HOME")),
            "{error}"
        );
        assert!(
            !dir_corridas(tmp.path()).exists(),
            "no debe crear la carpeta"
        );
    }

    #[test]
    fn corre_preparar_copia_bootstrap_y_contrato() {
        let tmp = TempDir::new().expect("tmp");
        let (home, usuario) = entorno(tmp.path());
        let corrida = preparar_corrida(&home, &usuario, "memo", "ecc-haiku").expect("prepara");
        assert_eq!(corrida.repo, corrida.dir.join("repo"));
        assert!(corrida.dir.starts_with(dir_corridas(&usuario)));
        assert!(corrida.repo.join("docs").join("SPEC.md").is_file());
        assert!(corrida.repo.join("Cargo.toml").is_file());
        assert!(corrida.repo.join("tests").join("t_contrato.py").is_file());
    }

    #[test]
    fn corre_preparar_no_copia_nada_de_oculto() {
        let tmp = TempDir::new().expect("tmp");
        let (home, usuario) = entorno(tmp.path());
        let corrida = preparar_corrida(&home, &usuario, "memo", "ecc-haiku").expect("prepara");
        let copiados = archivos(&corrida.repo);
        assert!(
            copiados
                .iter()
                .all(|r| !r.ends_with("t_oculto.py") && !r.ends_with("tarea.toml")),
            "{copiados:?}"
        );
        for ruta in &copiados {
            let texto = fs::read_to_string(ruta).unwrap_or_default();
            assert!(
                !texto.contains(SECRETO),
                "el repo trae el oculto en {}",
                ruta.display()
            );
        }
    }

    #[test]
    fn corre_preparar_hace_commit_bootstrap() {
        let tmp = TempDir::new().expect("tmp");
        let (home, usuario) = entorno(tmp.path());
        let corrida = preparar_corrida(&home, &usuario, "memo", "ecc-haiku").expect("prepara");
        let log = Command::new("git")
            .arg("-C")
            .arg(&corrida.repo)
            .args(["log", "-1", "--format=%s"])
            .output()
            .expect("git log");
        assert!(log.status.success());
        assert_eq!(String::from_utf8_lossy(&log.stdout).trim(), "bootstrap");
        assert!(!corrida.commit.is_empty());
    }

    #[test]
    fn corre_preparar_rechaza_enlace_simbolico_en_bootstrap() {
        let tmp = TempDir::new().expect("tmp");
        let (home, usuario) = entorno(tmp.path());
        let oculto = home
            .join("oraculos")
            .join("memo")
            .join("oculto")
            .join("t_oculto.py");
        let enlace = home
            .join("oraculos")
            .join("memo")
            .join("bootstrap")
            .join("enlace.py");
        std::os::unix::fs::symlink(&oculto, &enlace).expect("enlace");
        let error = preparar_corrida(&home, &usuario, "memo", "ecc-haiku")
            .expect_err("un enlace debe fallar");
        assert!(matches!(error, ErrorCorre::Uso(_)), "{error}");
        let restos = fs::read_dir(dir_corridas(&usuario))
            .map(|d| d.count())
            .unwrap_or(0);
        assert_eq!(restos, 0, "el armado fallido no deja carpeta");
    }

    #[test]
    fn corre_preparar_rechaza_archivo_repetido_en_tests() {
        let tmp = TempDir::new().expect("tmp");
        let (home, usuario) = entorno(tmp.path());
        let tests = home
            .join("oraculos")
            .join("memo")
            .join("bootstrap")
            .join("tests");
        fs::create_dir_all(&tests).expect("tests");
        fs::write(tests.join("t_contrato.py"), "# otro\n").expect("repetido");
        let error = preparar_corrida(&home, &usuario, "memo", "ecc-haiku")
            .expect_err("un archivo repetido debe fallar");
        assert!(
            matches!(error, ErrorCorre::Uso(ref m) if m.contains("t_contrato.py")),
            "{error}"
        );
    }

    #[test]
    fn corre_preparar_rechaza_git_en_bootstrap() {
        let tmp = TempDir::new().expect("tmp");
        let (home, usuario) = entorno(tmp.path());
        fs::create_dir_all(
            home.join("oraculos")
                .join("memo")
                .join("bootstrap")
                .join(".git"),
        )
        .expect("git");
        let error = preparar_corrida(&home, &usuario, "memo", "ecc-haiku")
            .expect_err("un .git en el bootstrap debe fallar");
        assert!(matches!(error, ErrorCorre::Uso(_)), "{error}");
    }

    #[test]
    fn corre_preparar_rechaza_nombre_de_tarea_con_ruta() {
        let tmp = TempDir::new().expect("tmp");
        let (home, usuario) = entorno(tmp.path());
        for tarea in ["../memo", "", "a/b"] {
            let error = preparar_corrida(&home, &usuario, tarea, "ecc-haiku")
                .expect_err("un nombre con ruta debe fallar");
            assert!(matches!(error, ErrorCorre::Uso(_)), "{tarea}: {error}");
        }
    }

    #[test]
    fn corre_guardar_transcript_queda_bajo_root_home_y_fuera_de_la_corrida() {
        let tmp = TempDir::new().expect("tmp");
        let (home, usuario) = entorno(tmp.path());
        let corrida = preparar_corrida(&home, &usuario, "memo", "ecc-haiku").expect("prepara");
        let ruta = guardar_transcript(
            &home,
            &corrida.id,
            1,
            Rol::Construye,
            b"{\"type\":\"result\"}\n",
        )
        .expect("guarda");
        assert_eq!(
            ruta,
            home.join("transcripts")
                .join(&corrida.id)
                .join("1-construye.jsonl")
        );
        assert!(ruta.starts_with(&home));
        assert!(!ruta.starts_with(&corrida.dir));
        assert_eq!(
            fs::read_to_string(&ruta).expect("lee"),
            "{\"type\":\"result\"}\n"
        );
    }

    #[test]
    fn corre_error_de_uso_sale_con_2_y_el_resto_con_3() {
        assert_eq!(ErrorCorre::Uso("x".into()).codigo_de_salida(), 2);
        assert_eq!(ErrorCorre::Git("x".into()).codigo_de_salida(), 3);
        assert_eq!(ErrorCorre::Ejecutor("x".into()).codigo_de_salida(), 3);
        let error = Error::from(ErrorCorre::Uso("x".into()));
        assert_eq!(error.codigo_de_salida(), 2);
    }
}
