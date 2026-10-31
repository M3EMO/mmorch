//! `tools/test-seguro.sh`: la suite corre sin claves, con un `ROOT_HOME` de prueba nuevo y con los
//! temporales y el target en una carpeta por worktree, `ROOT_TESTS_BASE/<hash>/`. Un lock de podman
//! (`ROOT_TESTS_BASE/podman.lock`) hace esperar a las corridas que no tienen `ROOT_SIN_PODMAN=1`.
//! Un `cargo` falso, que vuelca el entorno y los argumentos, reemplaza a cargo por
//! `ROOT_TEST_SEGURO_CARGO`. Así no se paga una compilación anidada.
//! Cada test arma el entorno del hijo con `Command::env`: nada cambia el entorno del proceso.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

const SCRIPT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tools/test-seguro.sh");
const PRE_COMMIT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/.githooks/pre-commit");

/// Contador para que cada temporal de `escribir_ejecutable` tenga un nombre propio.
static CONTADOR_TEMPORAL: AtomicU64 = AtomicU64::new(0);

/// Escribe `texto` en `ruta` como ejecutable. Escribe un temporal único en la misma carpeta, le pone
/// los permisos, lo cierra y recién después lo renombra a `ruta`. Así el archivo no queda abierto
/// para escritura cuando se lo lanza (evita `ETXTBSY`).
fn escribir_ejecutable(ruta: &Path, texto: &str) {
    let nombre = ruta
        .file_name()
        .expect("la ruta tiene nombre")
        .to_string_lossy();
    let n = CONTADOR_TEMPORAL.fetch_add(1, Ordering::Relaxed);
    let temporal = ruta.with_file_name(format!(".{nombre}.tmp-{}-{n}", std::process::id()));
    {
        let mut archivo = fs::File::create(&temporal).expect("crea el temporal del ejecutable");
        archivo
            .write_all(texto.as_bytes())
            .expect("escribe el ejecutable");
        archivo
            .set_permissions(fs::Permissions::from_mode(0o755))
            .expect("permisos del ejecutable");
    }
    fs::rename(&temporal, ruta).expect("renombra el ejecutable a su nombre final");
}

/// Un `cargo` falso: vuelca su entorno en `env.txt` y sus argumentos en `args.txt`, y sale con `codigo`.
fn falso(dir: &Path, codigo: u8) -> PathBuf {
    let ruta = dir.join(format!("cargo-falso-{codigo}.sh"));
    let texto = format!(
        "#!/usr/bin/env bash\nenv > '{d}/env.txt'\nprintf '%s\\n' \"$@\" > '{d}/args.txt'\nexit {codigo}\n",
        d = dir.display()
    );
    escribir_ejecutable(&ruta, &texto);
    ruta
}

/// Arma la orden que corre `script` con `args` y el `cargo` falso `cargo`. La base es `dir/base`.
/// `extra` se agrega al entorno del hijo y `quitar` se saca del entorno heredado.
fn orden(
    script: &Path,
    dir: &Path,
    cargo: &Path,
    extra: &[(&str, String)],
    quitar: &[&str],
    args: &[&str],
) -> Command {
    let mut orden = Command::new(script);
    orden
        .args(args)
        .env("DEEPSEEK_API_KEY", "x")
        .env("ANTHROPIC_API_KEY", "y")
        .env("HOME", dir)
        .env("ROOT_HOME", dir.join("real"))
        .env("ROOT_TESTS_BASE", dir.join("base"))
        .env("ROOT_TEST_SEGURO_CARGO", cargo);
    // Un target fijado en la suite no debe llegar al hijo, salvo que el test lo pida en `extra`.
    for clave in ["CARGO_TARGET_DIR_SEGURO", "CARGO_TARGET_DIR"] {
        if !extra.iter().any(|(k, _)| *k == clave) {
            orden.env_remove(clave);
        }
    }
    for clave in quitar {
        orden.env_remove(clave);
    }
    for (clave, valor) in extra {
        orden.env(clave, valor);
    }
    orden
}

/// Corre `script` con `args`, el `cargo` falso de `dir` y `extra` en el entorno del hijo, sin `quitar`.
/// Devuelve el estado y el entorno que vio el falso.
fn correr(
    dir: &Path,
    codigo: u8,
    extra: &[(&str, String)],
    args: &[&str],
) -> (ExitStatus, HashMap<String, String>, Vec<String>) {
    correr_con(Path::new(SCRIPT), dir, codigo, extra, &[], args)
}

/// Como `correr`, pero con otro `script` y con variables que el hijo no hereda (`quitar`).
fn correr_con(
    script: &Path,
    dir: &Path,
    codigo: u8,
    extra: &[(&str, String)],
    quitar: &[&str],
    args: &[&str],
) -> (ExitStatus, HashMap<String, String>, Vec<String>) {
    let cargo = falso(dir, codigo);
    let estado = orden(script, dir, &cargo, extra, quitar, args)
        .status()
        .expect("corre tools/test-seguro.sh");
    let env_texto = fs::read_to_string(dir.join("env.txt")).expect("el falso dejó su entorno");
    let entorno = env_texto
        .lines()
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let args_texto =
        fs::read_to_string(dir.join("args.txt")).expect("el falso dejó sus argumentos");
    let argumentos = args_texto.lines().map(str::to_string).collect();
    (estado, entorno, argumentos)
}

fn base(dir: &Path) -> PathBuf {
    dir.join("base")
}

/// Carpeta temporal de un test, bajo `TMPDIR` si está fijado fuera de `/tmp`. Si no, bajo
/// `~/.cache/root-dbg/tmp`. La regla 2 de la laptop vale también para los tests.
fn dir_de_prueba() -> tempfile::TempDir {
    let base = std::env::var_os("TMPDIR")
        .map(PathBuf::from)
        .filter(|tmpdir| !tmpdir.starts_with("/tmp"))
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").expect("HOME definido"))
                .join(".cache/root-dbg/tmp")
        });
    fs::create_dir_all(&base).expect("crea la carpeta de pruebas");
    tempfile::Builder::new()
        .prefix("test-seguro-")
        .tempdir_in(&base)
        .expect("carpeta temporal")
}

#[test]
fn dir_de_prueba_no_empieza_con_tmp() {
    let dir = dir_de_prueba();
    assert!(
        !dir.path().starts_with("/tmp"),
        "la carpeta de prueba cae en /tmp: {}",
        dir.path().display()
    );
}

#[test]
fn test_seguro_quita_las_claves_del_entorno() {
    let dir = dir_de_prueba();
    let (_, entorno, _) = correr(dir.path(), 0, &[], &["algo"]);
    assert!(!entorno.contains_key("DEEPSEEK_API_KEY"), "{entorno:?}");
    assert!(!entorno.contains_key("ANTHROPIC_API_KEY"), "{entorno:?}");
}

#[test]
fn test_seguro_root_home_es_nuevo_bajo_base_y_se_borra_al_salir() {
    let dir = dir_de_prueba();
    let (_, entorno, _) = correr(dir.path(), 0, &[], &["algo"]);
    let root_home = PathBuf::from(&entorno["ROOT_HOME"]);
    assert!(root_home.starts_with(base(dir.path())), "{root_home:?}");
    assert_ne!(root_home, dir.path().join("real"));
    assert!(
        !root_home.exists(),
        "el ROOT_HOME de prueba se borra al terminar"
    );
}

#[test]
fn test_seguro_temporales_y_target_bajo_base() {
    let dir = dir_de_prueba();
    // Un CARGO_TARGET_DIR fijado por el usuario no gana: el script usa el de base.
    let fijado = dir.path().join("fijado");
    let extra = [("CARGO_TARGET_DIR", fijado.display().to_string())];
    let (_, entorno, _) = correr(dir.path(), 0, &extra, &["algo"]);
    assert!(PathBuf::from(&entorno["TMPDIR"]).starts_with(base(dir.path())));
    let target = PathBuf::from(&entorno["CARGO_TARGET_DIR"]);
    assert!(target.starts_with(base(dir.path())), "{target:?}");
    assert_ne!(target, fijado);
    assert!(target.is_dir(), "el target se crea");
}

#[test]
fn test_seguro_cargo_target_dir_seguro_gana_sobre_base() {
    let dir = dir_de_prueba();
    let propio = dir.path().join("propio-target");
    let extra = [("CARGO_TARGET_DIR_SEGURO", propio.display().to_string())];
    let (_, entorno, _) = correr(dir.path(), 0, &extra, &["algo"]);
    assert_eq!(PathBuf::from(&entorno["CARGO_TARGET_DIR"]), propio);
}

#[test]
fn test_seguro_pasa_los_argumentos_a_cargo_test() {
    let dir = dir_de_prueba();
    let (estado, _, argumentos) = correr(dir.path(), 0, &[], &["algo"]);
    assert!(estado.success());
    assert_eq!(argumentos, ["test", "algo"]);
}

#[test]
fn test_seguro_sale_con_el_codigo_de_cargo() {
    let dir = dir_de_prueba();
    let (estado, _, _) = correr(dir.path(), 7, &[], &["algo"]);
    assert_eq!(estado.code(), Some(7));
}

/// Las variables `GIT_*` que apuntan a un repo. Espejo de `VARIABLES_GIT_DE_REPO` en `src/oracle/mod.rs`.
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

#[test]
fn test_seguro_quita_las_variables_git_de_repo_del_entorno() {
    // Un `git commit` exporta GIT_INDEX_FILE y GIT_DIR al hook. Ninguna debe llegar a cargo.
    let dir = dir_de_prueba();
    let extra: Vec<(&str, String)> = VARIABLES_GIT_DE_REPO
        .iter()
        .map(|nombre| (*nombre, format!("/x/{nombre}")))
        .collect();
    let (_, entorno, _) = correr(dir.path(), 0, &extra, &["algo"]);
    for nombre in VARIABLES_GIT_DE_REPO {
        assert!(
            !entorno.contains_key(*nombre),
            "{nombre} llegó a cargo: {entorno:?}"
        );
    }
}

#[test]
fn test_seguro_root_sin_podman_llega_al_comando() {
    let dir = dir_de_prueba();
    let extra = [("ROOT_SIN_PODMAN", "1".to_string())];
    let (_, entorno, _) = correr(dir.path(), 0, &extra, &["algo"]);
    assert_eq!(
        entorno.get("ROOT_SIN_PODMAN").map(String::as_str),
        Some("1")
    );
}

#[test]
fn test_seguro_pre_commit_usa_el_script_sin_cargo_test_suelto() {
    let texto = fs::read_to_string(PRE_COMMIT).expect("lee .githooks/pre-commit");
    assert!(texto.contains("tools/test-seguro.sh"), "{texto}");
    assert!(
        !texto.lines().any(|l| l.trim() == "cargo test"),
        "el hook no debe correr `cargo test` suelto: {texto}"
    );
    assert!(texto.contains("cargo fmt --check"), "{texto}");
    assert!(
        texto.contains("cargo clippy --all-targets -- -D warnings"),
        "{texto}"
    );
}

/// Copia el script a `dir/<repo>/tools/test-seguro.sh`, como si fuera el script de otro worktree.
fn copia_del_script(dir: &Path, repo: &str) -> PathBuf {
    let tools = dir.join(repo).join("tools");
    fs::create_dir_all(&tools).expect("crea tools/ del worktree");
    let ruta = tools.join("test-seguro.sh");
    let texto = fs::read_to_string(SCRIPT).expect("lee el script");
    escribir_ejecutable(&ruta, &texto);
    ruta
}

/// Devuelve el `<hash>` de una ruta `base/<hash>/...`. Falla si no está bajo `base` o si el hash
/// no tiene 12 caracteres hexadecimales.
fn hash_bajo_base(ruta: &Path, base: &Path) -> String {
    let resto = ruta
        .strip_prefix(base)
        .unwrap_or_else(|_| panic!("{ruta:?} no está bajo {base:?}"));
    let primero = resto.components().next().expect("hay un componente");
    let hash = primero.as_os_str().to_string_lossy().into_owned();
    assert!(
        hash.len() == 12 && hash.chars().all(|c| c.is_ascii_hexdigit()),
        "el hash no tiene 12 hex: {hash}"
    );
    hash
}

#[test]
fn test_seguro_misma_ruta_da_la_misma_carpeta() {
    let dir = dir_de_prueba();
    let copia = copia_del_script(dir.path(), "repo-a");
    let (_, primera, _) = correr_con(&copia, dir.path(), 0, &[], &[], &["algo"]);
    let (_, segunda, _) = correr_con(&copia, dir.path(), 0, &[], &[], &["algo"]);
    assert_eq!(
        primera["CARGO_TARGET_DIR"], segunda["CARGO_TARGET_DIR"],
        "la misma ruta de repo debe dar la misma carpeta"
    );
}

#[test]
fn test_seguro_dos_rutas_dan_dos_carpetas() {
    let dir = dir_de_prueba();
    let a = copia_del_script(dir.path(), "repo-a");
    let b = copia_del_script(dir.path(), "repo-b");
    let (_, entorno_a, _) = correr_con(&a, dir.path(), 0, &[], &[], &["algo"]);
    let (_, entorno_b, _) = correr_con(&b, dir.path(), 0, &[], &[], &["algo"]);
    assert_ne!(
        entorno_a["CARGO_TARGET_DIR"], entorno_b["CARGO_TARGET_DIR"],
        "dos worktrees no deben compartir target"
    );
}

#[test]
fn test_seguro_target_tmpdir_y_root_home_van_bajo_la_carpeta_del_worktree() {
    let dir = dir_de_prueba();
    let (_, entorno, _) = correr(dir.path(), 0, &[], &["algo"]);
    let target = PathBuf::from(&entorno["CARGO_TARGET_DIR"]);
    let hash = hash_bajo_base(&target, &base(dir.path()));
    let carpeta = base(dir.path()).join(&hash);
    assert_eq!(target, carpeta.join("target"));
    assert_eq!(PathBuf::from(&entorno["TMPDIR"]), carpeta.join("tmp"));
    let root_home = PathBuf::from(&entorno["ROOT_HOME"]);
    assert_eq!(root_home.parent(), Some(carpeta.as_path()), "{root_home:?}");
}

/// Un `cargo` falso para las pruebas del lock: anota `inicio` y `fin` en `log.txt` y duerme 2 segundos.
fn falso_de_lock(dir: &Path) -> PathBuf {
    let ruta = dir.join("cargo-lock.sh");
    let texto = format!(
        "#!/usr/bin/env bash\necho inicio >> '{d}/log.txt'\nsleep 2\necho fin >> '{d}/log.txt'\n",
        d = dir.display()
    );
    escribir_ejecutable(&ruta, &texto);
    ruta
}

/// Lanza dos corridas del script casi a la vez, con la misma base, y espera las dos.
/// Devuelve el log del falso y el stderr de cada corrida.
fn dos_corridas(
    dir: &Path,
    extra: &[(&str, String)],
    quitar: &[&str],
) -> (Vec<String>, Vec<String>) {
    let cargo = falso_de_lock(dir);
    let hijos: Vec<_> = (0..2)
        .map(|_| {
            orden(Path::new(SCRIPT), dir, &cargo, extra, quitar, &["algo"])
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .expect("lanza tools/test-seguro.sh")
        })
        .collect();
    let salidas: Vec<_> = hijos
        .into_iter()
        .map(|h| h.wait_with_output().expect("espera la corrida"))
        .collect();
    for salida in &salidas {
        let errores = String::from_utf8_lossy(&salida.stderr);
        assert!(salida.status.success(), "{errores}");
    }
    let log = fs::read_to_string(dir.join("log.txt")).expect("el falso dejó su log");
    let errores = salidas
        .iter()
        .map(|s| String::from_utf8_lossy(&s.stderr).into_owned())
        .collect();
    (log.lines().map(str::to_string).collect(), errores)
}

#[test]
fn test_seguro_sin_root_sin_podman_dos_corridas_no_se_pisan() {
    let dir = dir_de_prueba();
    let (log, _) = dos_corridas(dir.path(), &[], &["ROOT_SIN_PODMAN"]);
    assert_eq!(log, ["inicio", "fin", "inicio", "fin"]);
}

#[test]
fn test_seguro_con_root_sin_podman_dos_corridas_no_esperan() {
    let dir = dir_de_prueba();
    let extra = [("ROOT_SIN_PODMAN", "1".to_string())];
    let (log, _) = dos_corridas(dir.path(), &extra, &[]);
    assert_eq!(log, ["inicio", "inicio", "fin", "fin"]);
}

#[test]
fn test_seguro_avisa_en_stderr_que_espera_el_lock() {
    let dir = dir_de_prueba();
    let (_, errores) = dos_corridas(dir.path(), &[], &["ROOT_SIN_PODMAN"]);
    let avisos: Vec<&String> = errores
        .iter()
        .filter(|e| e.contains("test-seguro: espera el lock de podman"))
        .collect();
    assert_eq!(avisos.len(), 1, "{errores:?}");
    assert!(avisos[0].contains("podman.lock"), "{}", avisos[0]);
}

#[test]
fn test_seguro_lock_vive_en_base() {
    let con_lock = dir_de_prueba();
    correr_con(
        Path::new(SCRIPT),
        con_lock.path(),
        0,
        &[],
        &["ROOT_SIN_PODMAN"],
        &["algo"],
    );
    assert!(
        base(con_lock.path()).join("podman.lock").is_file(),
        "sin ROOT_SIN_PODMAN, el lock queda en la base"
    );

    let sin_lock = dir_de_prueba();
    let extra = [("ROOT_SIN_PODMAN", "1".to_string())];
    correr(sin_lock.path(), 0, &extra, &["algo"]);
    assert!(
        !base(sin_lock.path()).join("podman.lock").exists(),
        "con ROOT_SIN_PODMAN=1 no se crea el lock"
    );
}

#[test]
fn escribir_ejecutable_deja_solo_el_final_y_ejecutable() {
    // Guarda: el temporal se renombra al nombre final y no queda ningún otro archivo en la carpeta.
    let dir = dir_de_prueba();
    let ruta = dir.path().join("ejecutable.sh");
    escribir_ejecutable(&ruta, "#!/usr/bin/env bash\necho hola\n");
    let entradas: Vec<String> = fs::read_dir(dir.path())
        .expect("lee la carpeta")
        .map(|e| {
            e.expect("entrada")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(entradas, ["ejecutable.sh"], "{entradas:?}");
    let modo = fs::metadata(&ruta).expect("metadatos").permissions().mode();
    assert_eq!(modo & 0o777, 0o755);
    assert_eq!(
        fs::read_to_string(&ruta).expect("lee el ejecutable"),
        "#!/usr/bin/env bash\necho hola\n"
    );
}
