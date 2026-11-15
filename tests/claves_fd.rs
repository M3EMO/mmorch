//! `ROOT_CLAVES_FD`: la clave entra por un descriptor y no llega a los hijos de r00t.
//! Ningún test sale a la red: los errores salen antes de tocar nada, y el hijo es un `git` falso.

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

const CLAVE_FALSA: &str = "sk-falsa-123";
/// Descriptor que usan los tests. No es 0, 1 ni 2.
const FD_CLAVES: &str = "7";

/// Lanza r00t con `args`. `contenido` va a una tubería que llega como descriptor 7.
/// El shell hace `exec` del binario, así que el descriptor hereda sin pasar por otro proceso.
fn r00t_con_tuberia(contenido: &[u8], home: &Path, path: &str, args: &[&str]) -> Output {
    let (lector, mut escritor) = std::io::pipe().expect("tubería");
    escritor.write_all(contenido).expect("escribe la tubería");
    drop(escritor);
    Command::new("/bin/sh")
        .arg("-c")
        .arg(r#"exec "$0" "$@" 7<&0"#)
        .arg(env!("CARGO_BIN_EXE_r00t"))
        .args(args)
        .stdin(Stdio::from(lector))
        .env("ROOT_CLAVES_FD", FD_CLAVES)
        .env("ROOT_HOME", home)
        .env("ROOT_SIN_PODMAN", "1")
        .env("PATH", path)
        .env_remove("DEEPSEEK_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .output()
        .expect("lanza r00t")
}

/// Argumentos de `colab empaquetar` con un trabajo y un repo. Llama a `git` antes de podman.
fn args_empaquetar(tmp: &Path) -> Vec<String> {
    let raiz = tmp.join("raiz");
    fs::create_dir_all(&raiz).expect("raiz");
    vec![
        "colab".into(),
        "empaquetar".into(),
        "--binario".into(),
        tmp.join("r00t").display().to_string(),
        "--trabajos".into(),
        "t:eficiencia".into(),
        "--repo".into(),
        format!("t={}", tmp.join("repo").display()),
        "--rustup-sha256".into(),
        "0".repeat(64),
        "--salida".into(),
        tmp.join("paquete.tar.gz").display().to_string(),
        "--raiz-root".into(),
        raiz.display().to_string(),
    ]
}

#[test]
fn claves_fd_no_llegan_al_hijo_ni_en_entorno_ni_en_descriptores() {
    let tmp = TempDir::new().expect("tmp");
    let bin = tmp.path().join("bin");
    fs::create_dir(&bin).expect("bin");
    let registro = tmp.path().join("registro.txt");
    // El git falso vuelca su entorno y sus descriptores, y sale con error para no seguir.
    let guion = format!(
        "#!/bin/bash\n{{\n  echo '--- env'\n  env\n  echo '--- fd'\n  ls -l /proc/self/fd\n}} > '{}'\nexit 1\n",
        registro.display()
    );
    let git = bin.join("git");
    fs::write(&git, guion).expect("git falso");
    fs::set_permissions(&git, fs::Permissions::from_mode(0o755)).expect("permisos");
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let home = tmp.path().join("home");
    fs::create_dir(&home).expect("home");
    let argumentos = args_empaquetar(tmp.path());
    let argumentos: Vec<&str> = argumentos.iter().map(String::as_str).collect();

    let contenido = format!("DEEPSEEK_API_KEY={CLAVE_FALSA}\n");
    let salida = r00t_con_tuberia(contenido.as_bytes(), &home, &path, &argumentos);

    let texto_registro =
        fs::read_to_string(&registro).expect("el git falso no corrió: no hay registro");
    assert!(!texto_registro.contains(CLAVE_FALSA), "{texto_registro}");
    assert!(
        !texto_registro.contains("DEEPSEEK_API_KEY"),
        "{texto_registro}"
    );
    assert!(
        !texto_registro.contains("ROOT_CLAVES_FD"),
        "{texto_registro}"
    );
    // El fd 7 es la tubería de la clave. Los fd 0, 1 y 2 son los del arnés y no cuentan.
    assert!(!texto_registro.contains(" 7 -> "), "{texto_registro}");

    let todo = format!(
        "{}{}",
        String::from_utf8_lossy(&salida.stdout),
        String::from_utf8_lossy(&salida.stderr)
    );
    assert!(!todo.contains(CLAVE_FALSA), "{todo}");
    assert!(!todo.contains("ROOT_CLAVES_FD"), "{todo}");
}

#[test]
fn claves_fd_que_no_es_numero_sale_con_2_sin_mostrar_el_valor() {
    let tmp = TempDir::new().expect("tmp");
    let salida = Command::new(env!("CARGO_BIN_EXE_r00t"))
        .args(["esquema", "veredicto"])
        .env("ROOT_CLAVES_FD", "fd-x9")
        .env("ROOT_HOME", tmp.path())
        .output()
        .expect("lanza r00t");
    assert_eq!(salida.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(!stderr.contains("fd-x9"), "{stderr}");
}

#[test]
fn claves_fd_descriptor_cerrado_sale_con_2() {
    let tmp = TempDir::new().expect("tmp");
    let salida = Command::new(env!("CARGO_BIN_EXE_r00t"))
        .args(["esquema", "veredicto"])
        .env("ROOT_CLAVES_FD", "999")
        .env("ROOT_HOME", tmp.path())
        .output()
        .expect("lanza r00t");
    assert_eq!(salida.status.code(), Some(2));
}

#[test]
fn claves_fd_linea_de_otra_clave_sale_con_2_sin_mostrar_el_valor() {
    let tmp = TempDir::new().expect("tmp");
    let salida = r00t_con_tuberia(
        b"OTRA=valorsecreto99\n",
        tmp.path(),
        &std::env::var("PATH").unwrap_or_default(),
        &["esquema", "veredicto"],
    );
    assert_eq!(salida.status.code(), Some(2));
    let todo = format!(
        "{}{}",
        String::from_utf8_lossy(&salida.stdout),
        String::from_utf8_lossy(&salida.stderr)
    );
    assert!(!todo.contains("valorsecreto99"), "{todo}");
}
