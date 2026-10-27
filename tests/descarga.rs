//! Fase de descarga con red real: crates.io y github.com. Necesita podman y la imagen
//! `localhost/root-oraculo-rust:1`. Sin red, estos tests fallan con un mensaje explícito.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard, Once};
use std::time::{Duration, SystemTime};

use root::oracle::contenedor::{self, Ejecucion};
use root::oracle::descarga;
use root::oracle::tarea::Tarea;
use root::oracle::{OpcionesOraculo, evaluar};
use root::tipos::{Conteo, Dimension, Estado, Veredicto};
use serde_json::Value;

const IMAGEN: &str = "localhost/root-oraculo-rust:1";
/// Descargar y compilar puede tardar. El límite cubre una descarga lenta.
const LIMITE: Duration = Duration::from_secs(900);
const TAREA: &str = "tarea-prueba";

/// Los tests con podman corren de a uno, como en los demás tests de integración.
static SERIE: Mutex<()> = Mutex::new(());
static IMAGEN_LISTA: Once = Once::new();

fn preparar() -> MutexGuard<'static, ()> {
    let guarda = SERIE.lock().unwrap_or_else(|e| e.into_inner());
    IMAGEN_LISTA.call_once(|| {
        let existe = Command::new("podman")
            .args(["image", "exists", IMAGEN])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        assert!(
            existe,
            "falta la imagen {IMAGEN}: construirla con scripts/construir-imagen-oraculo.sh"
        );
    });
    guarda
}

/// Carpeta base dentro de `~/.cache/root-dbg/tmp`. Nunca `/tmp` ni el ROOT_HOME real.
fn base() -> PathBuf {
    let home = std::env::var_os("HOME").expect("HOME definido");
    let base = PathBuf::from(home).join(".cache/root-dbg/tmp");
    fs::create_dir_all(&base).expect("crea la carpeta de pruebas");
    base
}

/// `ROOT_HOME` temporal de un test. Se borra al salir del test.
fn home_de_prueba() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("descarga-home-")
        .tempdir_in(base())
        .expect("home temporal")
}

fn opciones(home: &Path) -> OpcionesOraculo {
    OpcionesOraculo {
        timeout: LIMITE,
        home: Some(home.to_path_buf()),
    }
}

const MAIN_HUMANTIME: &str = r#"fn main() {
    let d = humantime::parse_duration("1h").expect("parsea");
    println!("<testsuite><testcase classname=\"dep\" name=\"{}\"/></testsuite>", d.as_secs());
}
"#;

const CARGO_HUMANTIME: &str = r#"[package]
name = "dep-prueba"
version = "0.1.0"
edition = "2021"

[dependencies]
humantime = "2"
"#;

const JUNIT_OK: &str = "<testsuite><testcase classname=\"t\" name=\"ok\"/></testsuite>";

/// Repo con su `Cargo.toml`, un `main.rs` y, si se pide, un `Cargo.lock` dado.
fn repo_con(dir: &Path, cargo_toml: &str, lock: Option<&str>) {
    fs::create_dir_all(dir.join("src")).expect("crea src");
    fs::write(dir.join("Cargo.toml"), cargo_toml).expect("escribe Cargo.toml");
    fs::write(dir.join("src/main.rs"), MAIN_HUMANTIME).expect("escribe main.rs");
    if let Some(lock) = lock {
        fs::write(dir.join("Cargo.lock"), lock).expect("escribe Cargo.lock");
    }
}

/// Genera el `Cargo.lock` con cargo del host, con su `CARGO_HOME` dentro de la base de pruebas.
fn lock_generado(home: &Path) -> String {
    let repo = tempfile::Builder::new()
        .prefix("descarga-lock-")
        .tempdir_in(base())
        .expect("temporal");
    repo_con(repo.path(), CARGO_HUMANTIME, None);
    let cargo_home = home.join("cargo-host");
    let estado = Command::new("cargo")
        .arg("generate-lockfile")
        .current_dir(repo.path())
        .env("CARGO_HOME", &cargo_home)
        .status()
        .expect("no se pudo ejecutar cargo generate-lockfile");
    assert!(
        estado.success(),
        "sin red no se genera el Cargo.lock de prueba: revisar la conexión a crates.io"
    );
    fs::read_to_string(repo.path().join("Cargo.lock")).expect("lee el lock generado")
}

/// Escribe `tarea.toml` de prueba con sus dos suites y la carga.
fn tarea_con(home: &Path, contrato: &str, oculto: &str) -> Tarea {
    let dir = home.join("oraculos").join(TAREA);
    fs::create_dir_all(&dir).expect("crea la tarea");
    let texto = format!(
        "nombre = \"{TAREA}\"\nlenguaje = \"rust\"\nimagen = \"{IMAGEN}\"\nspec = \"docs/SPEC.md\"\n\n\
         [contrato]\ncmd = '''{contrato}'''\n\n[oculto]\ncmd = '''{oculto}'''\n"
    );
    fs::write(dir.join("tarea.toml"), texto).expect("escribe tarea.toml");
    Tarea::cargar_de(&dir).expect("carga la tarea")
}

/// Suite que compila el repo y corre el binario, que imprime su junit.
fn contrato_compila() -> String {
    "cd /repo && cargo build -q --target-dir /salida/target && /salida/target/debug/dep-prueba > /salida/contrato.xml"
        .to_string()
}

fn contrato_junit_ok() -> String {
    format!("echo '{JUNIT_OK}' > /salida/contrato.xml")
}

fn oculto_junit_ok() -> String {
    format!("echo '{JUNIT_OK}' > /salida/oculto.xml")
}

/// Eventos `Gate` de la corrida de descarga de la tarea de prueba.
fn gates(home: &Path) -> Vec<Value> {
    let Ok(texto) = fs::read_to_string(home.join("eventos.jsonl")) else {
        return Vec::new();
    };
    texto
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str::<Value>(l).expect("línea JSON"))
        .filter(|v| v["tipo"] == "gate" && v["corrida"] == format!("descarga-{TAREA}"))
        .collect()
}

/// Carpeta de caché de Rust que tiene `config.toml`. Debe haber una sola.
fn cache_rust(home: &Path) -> PathBuf {
    let raiz = home.join("cache/rust");
    let carpetas: Vec<PathBuf> = fs::read_dir(&raiz)
        .expect("existe cache/rust")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("config.toml").is_file())
        .collect();
    assert_eq!(carpetas.len(), 1, "una caché con config.toml: {carpetas:?}");
    carpetas.into_iter().next().expect("una carpeta")
}

fn evaluar_con(home: &Path, repo: &Path, tarea: &Tarea) -> Veredicto {
    evaluar(repo, tarea, &opciones(home)).unwrap_or_else(|e| {
        panic!("la evaluación debe terminar: {e}. Sin red, revisar crates.io y github.com")
    })
}

#[test]
fn descarga_dependencia_de_crates_io_compila_en_la_evaluacion_sin_red() {
    let _g = preparar();
    let home = home_de_prueba();
    let repo = base().join("repo-compila");
    let _ = fs::remove_dir_all(&repo);
    let lock = lock_generado(home.path());
    repo_con(&repo, CARGO_HUMANTIME, Some(&lock));
    let tarea = tarea_con(home.path(), &contrato_compila(), &oculto_junit_ok());

    let v = evaluar_con(home.path(), &repo, &tarea);

    assert_eq!(
        v.contrato,
        Conteo { pasa: 1, total: 1 },
        "compila y corre el binario con humantime; hallazgos: {:?}",
        v.hallazgos
    );
    assert_eq!(v.oculto, Conteo { pasa: 1, total: 1 });
    assert_eq!(v.estado, Estado::SinVeredicto);
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn descarga_lock_con_git_da_sin_veredicto_con_hallazgo_y_no_baja_nada() {
    let _g = preparar();
    let home = home_de_prueba();
    let repo = base().join("repo-git");
    let _ = fs::remove_dir_all(&repo);
    let lock = "# This file is automatically @generated by Cargo.\n\
version = 4\n\n[[package]]\nname = \"app\"\nversion = \"0.1.0\"\n\n\
[[package]]\nname = \"ext\"\nversion = \"1.0.0\"\n\
source = \"git+https://github.com/x/ext?rev=1#1\"\n";
    repo_con(&repo, CARGO_HUMANTIME, Some(lock));
    let tarea = tarea_con(home.path(), &contrato_junit_ok(), &oculto_junit_ok());

    let v = evaluar_con(home.path(), &repo, &tarea);

    assert_eq!(v.estado, Estado::SinVeredicto);
    assert_eq!(v.contrato, Conteo::default());
    assert_eq!(v.oculto, Conteo::default());
    let linea_source = lock
        .lines()
        .position(|l| l.starts_with("source = \"git+"))
        .expect("la línea del source")
        + 1;
    let hallazgo = v
        .hallazgos
        .iter()
        .find(|h| h.dimension == Dimension::Entrega)
        .expect("hallazgo de entrega");
    assert_eq!(hallazgo.archivo, "Cargo.lock");
    assert_eq!(hallazgo.linea as usize, linea_source);
    assert!(hallazgo.cita.contains("git+https://github.com/x/ext"));
    assert!(hallazgo.detalle.contains("P12b"));
    assert!(
        !home.path().join("cache/rust").exists(),
        "no se creó caché de Rust"
    );
    assert!(
        !home.path().join("cache/rustsec").exists(),
        "no se clonó RustSec"
    );
    assert!(
        gates(home.path())
            .iter()
            .all(|g| g["extra"]["motivo"] == "rechazo"),
        "ningún Gate de descarga sin motivo de rechazo"
    );
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn descarga_el_contenedor_de_descarga_no_ve_oraculo() {
    let _g = preparar();
    let home = home_de_prueba();
    let repo = base().join("repo-sin-oraculo");
    let _ = fs::remove_dir_all(&repo);
    repo_con(&repo, CARGO_HUMANTIME, None);
    let salida = home.path().join("salida");
    fs::create_dir_all(&salida).expect("crea salida");
    let ejecucion = Ejecucion {
        imagen: IMAGEN.into(),
        repo: Some(repo.clone()),
        oraculo: None,
        salida: salida.clone(),
        cmd: "ls /oraculo".into(),
        env: Vec::new(),
        timeout: LIMITE,
        red: true,
        cache: None,
        cache_rw: false,
        rustsec: None,
    };

    let r = contenedor::ejecutar(&ejecucion).expect("corre el contenedor");

    assert_ne!(r.codigo, 0, "/oraculo no existe en la descarga");
    let args = contenedor::argumentos(&ejecucion, "n");
    assert!(
        !args.iter().any(|a| a.contains(":/oraculo")),
        "no hay montaje /oraculo: {args:?}"
    );
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn descarga_segunda_evaluacion_usa_la_cache_y_no_toca_config() {
    let _g = preparar();
    let home = home_de_prueba();
    let repo = base().join("repo-cache");
    let _ = fs::remove_dir_all(&repo);
    let lock = lock_generado(home.path());
    repo_con(&repo, CARGO_HUMANTIME, Some(&lock));
    let tarea = tarea_con(home.path(), &contrato_compila(), &oculto_junit_ok());

    let primera = evaluar_con(home.path(), &repo, &tarea);
    assert_eq!(primera.contrato, Conteo { pasa: 1, total: 1 });
    let config = cache_rust(home.path()).join("config.toml");
    let mtime_antes: SystemTime = fs::metadata(&config)
        .expect("config")
        .modified()
        .expect("mtime");

    let segunda = evaluar_con(home.path(), &repo, &tarea);
    assert_eq!(segunda.contrato, Conteo { pasa: 1, total: 1 });

    let mtime_despues = fs::metadata(&config)
        .expect("config")
        .modified()
        .expect("mtime");
    assert_eq!(mtime_antes, mtime_despues, "config.toml no se reescribió");
    let gates = gates(home.path());
    assert_eq!(gates.len(), 2, "un Gate por descarga");
    assert_eq!(gates[1]["extra"]["cache_hit"], true);
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn descarga_dependencia_inexistente_sin_lock_da_no_instala() {
    let _g = preparar();
    let home = home_de_prueba();
    let repo = base().join("repo-inexistente");
    let _ = fs::remove_dir_all(&repo);
    let cargo = "[package]\nname = \"dep-prueba\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n\
[dependencies]\nroot-no-existe-xyz-987 = \"1\"\n";
    repo_con(&repo, cargo, None);
    let tarea = tarea_con(home.path(), &contrato_junit_ok(), &oculto_junit_ok());

    let v = evaluar_con(home.path(), &repo, &tarea);

    assert_eq!(v.estado, Estado::SinVeredicto);
    assert_eq!(v.contrato, Conteo::default());
    assert!(
        v.hallazgos
            .iter()
            .any(|h| h.dimension == Dimension::Entrega
                && h.detalle.contains("el producto no instala")),
        "hallazgo de producto roto: {:?}",
        v.hallazgos
    );
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn descarga_sin_lock_genera_el_lock_y_avisa_que_no_es_reproducible() {
    let _g = preparar();
    let home = home_de_prueba();
    let repo = base().join("repo-sin-lock");
    let _ = fs::remove_dir_all(&repo);
    repo_con(&repo, CARGO_HUMANTIME, None);
    let tarea = tarea_con(home.path(), &contrato_junit_ok(), &oculto_junit_ok());

    let v = evaluar_con(home.path(), &repo, &tarea);

    assert_eq!(
        v.contrato,
        Conteo { pasa: 1, total: 1 },
        "la evaluación sigue"
    );
    let aviso = v
        .hallazgos
        .iter()
        .find(|h| h.detalle.contains("el build no es reproducible"))
        .expect("hallazgo de lock generado");
    assert_eq!(aviso.severidad, 2);
    assert_eq!(aviso.archivo, "Cargo.lock");
    assert!(
        !repo.join("Cargo.lock").exists(),
        "el repo original no cambia"
    );
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn descarga_evento_gate_lista_paquetes_con_su_fuente() {
    let _g = preparar();
    let home = home_de_prueba();
    let repo = base().join("repo-gate");
    let _ = fs::remove_dir_all(&repo);
    let lock = lock_generado(home.path());
    repo_con(&repo, CARGO_HUMANTIME, Some(&lock));
    let tarea = tarea_con(home.path(), &contrato_junit_ok(), &oculto_junit_ok());

    let _ = descarga::preparar(&repo, &tarea, home.path(), LIMITE)
        .expect("la descarga debe terminar; sin red falla con infraestructura");

    let gates = gates(home.path());
    assert_eq!(gates.len(), 1, "un Gate por descarga");
    let extra = &gates[0]["extra"];
    assert_eq!(gates[0]["fase"], "descarga");
    assert_eq!(extra["lenguaje"], "rust");
    assert_eq!(extra["cache_hit"], false);
    assert_eq!(extra["hash"].as_str().map(str::len), Some(16));
    let paquetes = extra["paquetes"].as_array().expect("paquetes");
    let humantime = paquetes
        .iter()
        .find(|p| p["nombre"] == "humantime")
        .expect("humantime en paquetes");
    assert_eq!(humantime["fuente"], "crates.io");
    assert!(humantime["version"].is_string());
    let propio = paquetes
        .iter()
        .find(|p| p["nombre"] == "dep-prueba")
        .expect("el paquete del repo");
    assert_eq!(propio["fuente"], "workspace");
    assert_eq!(
        extra["rustsec"], "ok",
        "la base de RustSec se clona la primera vez"
    );
    let _ = fs::remove_dir_all(&repo);
}
