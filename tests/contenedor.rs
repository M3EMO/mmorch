use std::process::Command;
use std::sync::{Mutex, MutexGuard, Once};
use std::time::{Duration, Instant};

use root::oracle::ErrorOraculo;
use root::oracle::contenedor::{Ejecucion, Resultado, ejecutar};

const IMAGEN: &str = "docker.io/library/alpine:3.20";
const LIMITE: Duration = Duration::from_secs(120);

/// Los tests con podman corren de a uno: el de timeout revisa contenedores por prefijo de PID.
static SERIE: Mutex<()> = Mutex::new(());
static IMAGEN_LISTA: Once = Once::new();

fn preparar() -> MutexGuard<'static, ()> {
    let guarda = SERIE.lock().unwrap_or_else(|e| e.into_inner());
    IMAGEN_LISTA.call_once(asegurar_imagen);
    guarda
}

fn asegurar_imagen() {
    let existe = Command::new("podman")
        .args(["image", "exists", IMAGEN])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if existe {
        return;
    }
    let pull = Command::new("podman")
        .args(["pull", IMAGEN])
        .output()
        .expect("no se pudo ejecutar podman pull");
    assert!(
        pull.status.success(),
        "no se pudo bajar la imagen {IMAGEN}: {}",
        String::from_utf8_lossy(&pull.stderr)
    );
}

fn ejecucion(repo: &std::path::Path, salida: &std::path::Path, cmd: &str) -> Ejecucion {
    Ejecucion {
        imagen: IMAGEN.into(),
        repo: Some(repo.into()),
        oraculo: None,
        oraculo_destino: None,
        salida: salida.into(),
        cmd: cmd.into(),
        env: vec![],
        timeout: LIMITE,
        red: false,
        cache: None,
        cache_rw: false,
        rustsec: None,
        trabajo: None,
        secretos: Vec::new(),
    }
}

fn correr(e: &Ejecucion) -> Resultado {
    ejecutar(e).expect("el comando debe terminar sin error de infraestructura")
}

#[test]
fn contenedor_escritura_en_repo_falla_porque_el_montaje_es_ro() {
    root::saltar_sin_podman!();
    let _guarda = preparar();
    let repo = tempfile::tempdir().expect("temp dir");
    let salida = tempfile::tempdir().expect("temp dir");
    let r = correr(&ejecucion(repo.path(), salida.path(), "echo x > /repo/x"));
    assert_ne!(r.codigo, 0);
    assert!(!repo.path().join("x").exists());
}

#[test]
fn contenedor_escritura_en_salida_funciona_y_llega_al_host() {
    root::saltar_sin_podman!();
    let _guarda = preparar();
    let repo = tempfile::tempdir().expect("temp dir");
    let salida = tempfile::tempdir().expect("temp dir");
    let r = correr(&ejecucion(
        repo.path(),
        salida.path(),
        "echo ok > /salida/ok.txt",
    ));
    assert_eq!(r.codigo, 0);
    assert!(salida.path().join("ok.txt").exists());
}

#[test]
fn contenedor_red_cortada_impide_salir_a_internet() {
    root::saltar_sin_podman!();
    let _guarda = preparar();
    let repo = tempfile::tempdir().expect("temp dir");
    let salida = tempfile::tempdir().expect("temp dir");
    let r = correr(&ejecucion(
        repo.path(),
        salida.path(),
        "wget -T 3 -q -O- http://1.1.1.1",
    ));
    assert_ne!(r.codigo, 0);
}

#[test]
fn contenedor_timeout_da_error_de_infraestructura_y_borra_el_contenedor() {
    root::saltar_sin_podman!();
    let _guarda = preparar();
    let repo = tempfile::tempdir().expect("temp dir");
    let salida = tempfile::tempdir().expect("temp dir");
    let mut e = ejecucion(repo.path(), salida.path(), "sleep 30");
    e.timeout = Duration::from_secs(2);
    let inicio = Instant::now();
    let res = ejecutar(&e);
    assert!(
        matches!(res, Err(ErrorOraculo::Infra(_))),
        "se esperaba Infra, llegó {res:?}"
    );
    assert!(inicio.elapsed() < Duration::from_secs(10));
    let prefijo = format!("name=root-oraculo-{}-", std::process::id());
    let ps = Command::new("podman")
        .args(["ps", "-a", "--filter", &prefijo, "--format", "{{.Names}}"])
        .output()
        .expect("podman ps");
    assert!(
        ps.stdout.is_empty(),
        "quedó un contenedor: {}",
        String::from_utf8_lossy(&ps.stdout)
    );
}

#[test]
fn contenedor_imagen_inexistente_da_error_de_infraestructura() {
    root::saltar_sin_podman!();
    let _guarda = preparar();
    let repo = tempfile::tempdir().expect("temp dir");
    let salida = tempfile::tempdir().expect("temp dir");
    let mut e = ejecucion(repo.path(), salida.path(), "true");
    e.imagen = "localhost/no-existe:0".into();
    let res = ejecutar(&e);
    assert!(
        matches!(res, Err(ErrorOraculo::Infra(_))),
        "se esperaba Infra, llegó {res:?}"
    );
}

#[test]
fn contenedor_codigo_de_salida_del_comando_llega_intacto() {
    root::saltar_sin_podman!();
    let _guarda = preparar();
    let repo = tempfile::tempdir().expect("temp dir");
    let salida = tempfile::tempdir().expect("temp dir");
    let r = correr(&ejecucion(repo.path(), salida.path(), "exit 7"));
    assert_eq!(r.codigo, 7);
}
