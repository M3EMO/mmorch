//! Fase de descarga con red real: crates.io y github.com. Necesita podman y la imagen
//! `localhost/root-oraculo-rust:1`. Sin red, estos tests fallan con un mensaje explícito.

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard, Once};
use std::time::{Duration, SystemTime};

use root::oracle::contenedor::{self, Ejecucion};
use root::oracle::descarga;
use root::oracle::descarga::Preparacion;
use root::oracle::descarga::juez_config::ResultadoJuicio;
use root::oracle::juez::ModeloJuez;
use root::oracle::tarea::Tarea;
use root::oracle::{ErrorOraculo, OpcionesOraculo, evaluar};
use root::proveedores::{Prompt, RespuestaJuez};
use root::tipos::{Conteo, Dimension, Estado, Hallazgo, Veredicto};
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
    tarea_en_lenguaje(home, "rust", IMAGEN, contrato, oculto)
}

/// Como `tarea_con`, con el lenguaje dado.
fn tarea_en_lenguaje(
    home: &Path,
    lenguaje: &str,
    imagen: &str,
    contrato: &str,
    oculto: &str,
) -> Tarea {
    let dir = home.join("oraculos").join(TAREA);
    fs::create_dir_all(&dir).expect("crea la tarea");
    let texto = format!(
        "nombre = \"{TAREA}\"\nlenguaje = \"{lenguaje}\"\nimagen = \"{imagen}\"\nspec = \"docs/SPEC.md\"\n\n\
         [contrato]\ncmd = '''{contrato}'''\n\n[oculto]\ncmd = '''{oculto}'''\n"
    );
    fs::write(dir.join("tarea.toml"), texto).expect("escribe tarea.toml");
    // `evaluar` exige una ficha válida en el spec de la tarea (Tarea 9f).
    fs::create_dir_all(dir.join("docs")).expect("dir docs");
    fs::write(
        dir.join("docs/SPEC.md"),
        include_str!("fixtures/fichas/spec-completa.md"),
    )
    .expect("escribe SPEC.md");
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

/// Eventos `Gate` de una fase de la corrida de descarga de la tarea de prueba.
fn gates_de(home: &Path, fase: &str) -> Vec<Value> {
    let Ok(texto) = fs::read_to_string(home.join("eventos.jsonl")) else {
        return Vec::new();
    };
    texto
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str::<Value>(l).expect("línea JSON"))
        .filter(|v| {
            v["tipo"] == "gate" && v["corrida"] == format!("descarga-{TAREA}") && v["fase"] == fase
        })
        .collect()
}

/// Gates de la fase de descarga (después de la red).
fn gates(home: &Path) -> Vec<Value> {
    gates_de(home, "descarga")
}

/// Gates del inventario (antes de la red).
fn gates_inventario(home: &Path) -> Vec<Value> {
    gates_de(home, "inventario")
}

/// Lee `eventos.jsonl` completo, o cadena vacía si no existe.
fn eventos_crudos(home: &Path) -> String {
    fs::read_to_string(home.join("eventos.jsonl")).unwrap_or_default()
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
    root::saltar_sin_podman!();
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
fn descarga_lock_con_ssh_da_sin_veredicto_con_hallazgo_y_no_baja_nada() {
    root::saltar_sin_podman!();
    let _g = preparar();
    let home = home_de_prueba();
    let repo = base().join("repo-git");
    let _ = fs::remove_dir_all(&repo);
    let lock = "# This file is automatically @generated by Cargo.\n\
version = 4\n\n[[package]]\nname = \"app\"\nversion = \"0.1.0\"\n\n\
[[package]]\nname = \"ext\"\nversion = \"1.0.0\"\n\
source = \"git+ssh://git@github.com/x/ext?rev=1#1\"\n";
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
    assert!(hallazgo.cita.contains("ssh://"), "cita: {}", hallazgo.cita);
    assert!(
        !hallazgo.cita.contains("git@"),
        "cita redactada: {}",
        hallazgo.cita
    );
    assert!(
        hallazgo.detalle.contains("no aceptada"),
        "{}",
        hallazgo.detalle
    );
    assert!(
        !home.path().join("cache/rust").exists(),
        "no se creó caché de Rust"
    );
    assert!(
        !home.path().join("cache/rustsec").exists(),
        "no se clonó RustSec"
    );
    assert!(
        gates(home.path()).is_empty(),
        "sin rechazo no hay Gate de descarga"
    );
    assert_eq!(
        gates_inventario(home.path()).len(),
        1,
        "un Gate de inventario"
    );
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn descarga_el_contenedor_de_descarga_no_ve_oraculo() {
    root::saltar_sin_podman!();
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
        oraculo_destino: None,
        salida: salida.clone(),
        cmd: "ls /oraculo".into(),
        env: Vec::new(),
        timeout: LIMITE,
        red: true,
        cache: None,
        cache_rw: false,
        rustsec: None,
        trabajo: None,
        secretos: Vec::new(),
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
    root::saltar_sin_podman!();
    let _g = preparar();
    let home = home_de_prueba();
    // Carpeta propia del test: nunca se comparte con otro test (carrera de `repo-cache`).
    let repo_dir = repo_propio("repo-cache-");
    let repo = repo_dir.path().to_path_buf();
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
}

#[test]
fn descarga_dependencia_inexistente_sin_lock_da_no_instala() {
    root::saltar_sin_podman!();
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
    root::saltar_sin_podman!();
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
    root::saltar_sin_podman!();
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

/// Repo de prueba con un `main` dado y el manifiesto dado. Sin lock si `lock` es `None`.
fn repo_con_main(dir: &Path, cargo_toml: &str, main: &str, lock: Option<&str>) {
    fs::create_dir_all(dir.join("src")).expect("crea src");
    fs::write(dir.join("Cargo.toml"), cargo_toml).expect("escribe Cargo.toml");
    fs::write(dir.join("src/main.rs"), main).expect("escribe main.rs");
    if let Some(lock) = lock {
        fs::write(dir.join("Cargo.lock"), lock).expect("escribe Cargo.lock");
    }
}

/// Manifiesto del repo de prueba con las dependencias dadas.
fn manifiesto_con(deps: &str) -> String {
    format!(
        "[package]\nname = \"dep-prueba\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n{deps}"
    )
}

/// Crate público, sin dependencias de runtime, que se fija por commit en el test de git.
const ITOA_URL: &str = "https://github.com/dtolnay/itoa";
const ITOA_REV: &str = "1577ed901354d0d7448ac162328f9dbf5183124c";
const MAIN_ITOA: &str = r#"fn main() {
    let mut buf = itoa::Buffer::new();
    let s = buf.format(42u32).to_string();
    println!("<testsuite><testcase classname=\"dep\" name=\"{s}\"/></testsuite>");
}
"#;

#[test]
fn descarga_gate_de_inventario_se_registra_antes_de_cualquier_red() {
    root::saltar_sin_podman!();
    let _g = preparar();
    let home = home_de_prueba();
    let repo = base().join("repo-inalcanzable");
    let _ = fs::remove_dir_all(&repo);
    let cargo = manifiesto_con("fuera = { git = \"https://127.0.0.1:1/no-existe.git\" }\n");
    repo_con_main(&repo, &cargo, MAIN_HUMANTIME, None);
    let tarea = tarea_con(home.path(), &contrato_junit_ok(), &oculto_junit_ok());

    let err = descarga::preparar(&repo, &tarea, home.path(), LIMITE)
        .expect_err("una fuente inalcanzable es infraestructura");
    assert_eq!(err.codigo_de_salida(), 3, "{err}");

    let inventario = gates_inventario(home.path());
    assert_eq!(
        inventario.len(),
        1,
        "el Gate de inventario existe: {inventario:?}"
    );
    let fuentes = inventario[0]["extra"]["fuentes"]
        .as_array()
        .expect("fuentes");
    assert!(
        fuentes
            .iter()
            .any(|f| f["tipo"] == "git" && f["direccion"] == "https://127.0.0.1:1/no-existe.git"),
        "la fuente git está en el Gate antes de la red: {fuentes:?}"
    );
    assert!(
        gates(home.path()).is_empty(),
        "la red falló: no hay Gate de descarga"
    );
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn descarga_rechazos_no_usan_red_ni_contenedor_y_no_filtran_credenciales() {
    root::saltar_sin_podman!();
    let _g = preparar();
    let home = home_de_prueba();
    let repo = base().join("repo-rechazos");
    let _ = fs::remove_dir_all(&repo);
    let cargo = manifiesto_con(
        "uno = { git = \"ssh://git@github.com/x/y\" }\n\
dos = { path = \"../afuera\" }\n\
tres = { git = \"https://usuario:clave@host/x.git\" }\n",
    );
    repo_con_main(&repo, &cargo, MAIN_HUMANTIME, None);
    let tarea = tarea_con(home.path(), &contrato_junit_ok(), &oculto_junit_ok());

    let v = evaluar_con(home.path(), &repo, &tarea);

    assert_eq!(v.estado, Estado::SinVeredicto);
    assert_eq!(v.contrato, Conteo::default());
    assert_eq!(v.oculto, Conteo::default());
    let entrega: Vec<&Hallazgo> = v
        .hallazgos
        .iter()
        .filter(|h| h.dimension == Dimension::Entrega)
        .collect();
    assert_eq!(
        entrega.len(),
        3,
        "un hallazgo por rechazo: {:?}",
        v.hallazgos
    );
    assert!(
        entrega
            .iter()
            .all(|h| h.severidad == 3 && h.archivo == "Cargo.toml")
    );
    assert!(
        !home.path().join("cache").exists(),
        "ningún contenedor de descarga corrió"
    );
    assert!(
        gates(home.path()).is_empty(),
        "sin descarga no hay Gate de descarga"
    );
    let crudos = eventos_crudos(home.path());
    assert!(
        !crudos.contains("clave"),
        "la credencial no aparece en eventos"
    );
    assert!(
        !crudos.contains("usuario:"),
        "la credencial no aparece en eventos"
    );
    let veredicto = serde_json::to_string(&v).expect("serializa");
    assert!(!veredicto.contains("clave"), "ni en el veredicto");
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn descarga_dependencia_git_con_rev_compila_en_la_evaluacion_sin_red() {
    root::saltar_sin_podman!();
    let _g = preparar();
    let home = home_de_prueba();
    let repo = base().join("repo-git-rev");
    let _ = fs::remove_dir_all(&repo);
    let cargo = manifiesto_con(&format!(
        "itoa = {{ git = \"{ITOA_URL}\", rev = \"{ITOA_REV}\" }}\n"
    ));
    repo_con_main(&repo, &cargo, MAIN_ITOA, None);
    let tarea = tarea_con(home.path(), &contrato_compila(), &oculto_junit_ok());

    let v = evaluar_con(home.path(), &repo, &tarea);

    assert_eq!(
        v.contrato,
        Conteo { pasa: 1, total: 1 },
        "compila con itoa por git sin red; hallazgos: {:?}",
        v.hallazgos
    );
    assert!(
        !v.hallazgos
            .iter()
            .any(|h| h.detalle.contains("sin commit fijo")),
        "con rev no hay hallazgo de commit"
    );
    let descarga = gates(home.path());
    assert_eq!(descarga.len(), 1, "un Gate de descarga: {descarga:?}");
    let paquetes = descarga[0]["extra"]["paquetes"]
        .as_array()
        .expect("paquetes");
    let itoa = paquetes
        .iter()
        .find(|p| p["nombre"] == "itoa")
        .expect("itoa en el Gate");
    assert_eq!(itoa["fuente"], "git");
    assert_eq!(itoa["commit"], ITOA_REV);
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn descarga_dependencia_git_sin_rev_registra_el_commit_y_avisa_que_no_es_reproducible() {
    root::saltar_sin_podman!();
    let _g = preparar();
    let home = home_de_prueba();
    let repo = base().join("repo-git-sin-rev");
    let _ = fs::remove_dir_all(&repo);
    let cargo = manifiesto_con(&format!("itoa = {{ git = \"{ITOA_URL}\" }}\n"));
    repo_con_main(&repo, &cargo, MAIN_ITOA, None);
    let tarea = tarea_con(home.path(), &contrato_compila(), &oculto_junit_ok());

    let v = evaluar_con(home.path(), &repo, &tarea);

    assert_eq!(
        v.contrato,
        Conteo { pasa: 1, total: 1 },
        "la evaluación sigue"
    );
    let aviso = v
        .hallazgos
        .iter()
        .find(|h| h.dimension == Dimension::Entrega && h.detalle.contains("sin commit fijo"))
        .expect("hallazgo de dependencia git sin commit fijo");
    assert_eq!(aviso.severidad, 2);
    let descarga = gates(home.path());
    let paquetes = descarga[0]["extra"]["paquetes"]
        .as_array()
        .expect("paquetes");
    let itoa = paquetes
        .iter()
        .find(|p| p["nombre"] == "itoa")
        .expect("itoa en el Gate");
    let commit = itoa["commit"].as_str().expect("commit obtenido");
    assert_eq!(
        commit.len(),
        40,
        "el commit obtenido es un sha completo: {commit}"
    );
    assert!(commit.chars().all(|c| c.is_ascii_hexdigit()), "{commit}");
    let _ = fs::remove_dir_all(&repo);
}

/// Carpeta de repo propia de un test, dentro de la base de pruebas. Se borra al salir del test.
fn repo_propio(prefijo: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(prefijo)
        .tempdir_in(base())
        .expect("repo temporal")
}

/// Escribe el `.cargo/config.toml` del repo con el texto dado.
fn escribir_config(repo: &Path, texto: &str) {
    fs::create_dir_all(repo.join(".cargo")).expect("crea .cargo");
    fs::write(repo.join(".cargo/config.toml"), texto).expect("escribe config");
}

#[test]
fn descarga_respeta_el_proxy_del_cargo_config_del_repo() {
    root::saltar_sin_podman!();
    let _g = preparar();
    let home = home_de_prueba();
    let repo = repo_propio("repo-proxy-");
    let lock = lock_generado(home.path());
    repo_con(repo.path(), CARGO_HUMANTIME, Some(&lock));
    escribir_config(repo.path(), "[http]\nproxy = \"http://127.0.0.1:9\"\n");
    let tarea = tarea_con(home.path(), &contrato_compila(), &oculto_junit_ok());

    // `cargo vendor` corre en /repo y usa el proxy del config: con el proxy muerto no baja nada.
    let error = evaluar(repo.path(), &tarea, &opciones(home.path()))
        .expect_err("con el proxy del repo muerto, la descarga falla");
    assert!(
        matches!(error, root::oracle::ErrorOraculo::Infra(_)),
        "error de infraestructura, sin veredicto: {error}"
    );

    // El Gate del inventario lista el proxy antes de cualquier red.
    let inventario = gates_inventario(home.path());
    let config = inventario[0]["extra"]["config_cargo"]
        .as_array()
        .expect("config_cargo en el Gate del inventario");
    assert!(
        config.iter().any(|e| e["clave"] == "http.proxy"),
        "{config:?}"
    );
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn descarga_alias_y_rustc_wrapper_del_repo_no_afectan_la_descarga() {
    root::saltar_sin_podman!();
    let _g = preparar();
    let home = home_de_prueba();
    let repo = repo_propio("repo-wrapper-");
    let lock = lock_generado(home.path());
    repo_con(repo.path(), CARGO_HUMANTIME, Some(&lock));
    escribir_config(
        repo.path(),
        "[alias]\nx = \"build\"\n\n[build]\nrustc-wrapper = \"/bin/false\"\n",
    );
    let tarea = tarea_con(home.path(), &contrato_junit_ok(), &oculto_junit_ok());

    let v = evaluar_con(home.path(), repo.path(), &tarea);

    assert_eq!(v.contrato, Conteo { pasa: 1, total: 1 });
    assert!(
        cache_rust(home.path()).join("config.toml").is_file(),
        "la descarga dejó la caché"
    );
}

/// Un requirements con un `--extra-index-url` y una fuente que el inventario rechaza (`ssh`).
/// El rechazo corta antes de cualquier red, así que el test corre sin internet.
const REQUIREMENTS_EXTRA: &str =
    "--extra-index-url https://pypi.ejemplo.org/simple\nfoo @ ssh://x.ejemplo.org/foo.whl\n";

#[test]
fn descarga_extra_index_url_deja_hallazgo_severidad_1_en_gate_y_en_preparar() {
    let home = home_de_prueba();
    let repo = repo_propio("repo-extra-index-");
    fs::write(repo.path().join("requirements.txt"), REQUIREMENTS_EXTRA).expect("requirements");
    let tarea = tarea_en_lenguaje(
        home.path(),
        "python",
        IMAGEN,
        &contrato_junit_ok(),
        &oculto_junit_ok(),
    );

    let preparacion = descarga::preparar(repo.path(), &tarea, home.path(), LIMITE)
        .expect("el rechazo local no usa la red");

    let descarga::Preparacion::SinVeredicto(hallazgos) = preparacion else {
        panic!("el rechazo de ssh corta la evaluación: {preparacion:?}");
    };
    let mezcla = hallazgos
        .iter()
        .find(|h| h.severidad == 1)
        .unwrap_or_else(|| panic!("falta el hallazgo de --extra-index-url: {hallazgos:?}"));
    assert_eq!(mezcla.dimension, Dimension::Entrega);
    assert_eq!(mezcla.archivo, "requirements.txt");
    assert_eq!(mezcla.linea, 1, "la línea del --extra-index-url");
    assert!(
        mezcla.detalle.contains("mezclar índices"),
        "{}",
        mezcla.detalle
    );

    let inventario = gates_inventario(home.path());
    assert_eq!(inventario.len(), 1, "un Gate de inventario");
    let en_gate = inventario[0]["extra"]["hallazgos"]
        .as_array()
        .expect("el Gate de inventario lista los hallazgos");
    assert!(
        en_gate
            .iter()
            .any(|h| h["severidad"] == 1 && h["linea"] == 1),
        "el Gate trae el hallazgo severidad 1 en la línea 1: {en_gate:?}"
    );
}

#[test]
fn descarga_sin_extra_index_url_no_deja_hallazgo_de_mezcla() {
    let home = home_de_prueba();
    let repo = repo_propio("repo-sin-extra-");
    fs::write(
        repo.path().join("requirements.txt"),
        "foo @ ssh://x.ejemplo.org/foo.whl\n",
    )
    .expect("requirements");
    let tarea = tarea_en_lenguaje(
        home.path(),
        "python",
        IMAGEN,
        &contrato_junit_ok(),
        &oculto_junit_ok(),
    );

    let preparacion = descarga::preparar(repo.path(), &tarea, home.path(), LIMITE)
        .expect("el rechazo local no usa la red");

    let descarga::Preparacion::SinVeredicto(hallazgos) = preparacion else {
        panic!("el rechazo de ssh corta la evaluación: {preparacion:?}");
    };
    assert!(
        !hallazgos.iter().any(|h| h.severidad == 1),
        "sin --extra-index-url no hay hallazgo de severidad 1: {hallazgos:?}"
    );
}

// ---- Descarga de Python (Tarea 6c-B) ----

use root::oracle::descarga::PRELUDIO_PYTHON;
use root::oracle::descarga::inventario::{requisitos_pip, textos_de_requisitos};
use root::oracle::descarga::python::{
    FalloSdist, bajar_sdist_con, comando_clon, comando_curl, comando_wheels, sin_wheel,
};

/// Imagen con Python y pip (`:2`). La descarga y la evaluación de Python la usan.
const IMAGEN_PY: &str = "localhost/root-oraculo-rust:2";
/// Commit fijo de `six` (verificado con `git ls-remote`).
const SIX_COMMIT: &str = "c8e394065cd541a16c040515dc0afb85cf22a7c3";
const SIX_GIT: &str =
    "git+https://github.com/benjaminp/six@c8e394065cd541a16c040515dc0afb85cf22a7c3#egg=six";

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// JSON de PyPI para una versión con un wheel y un sdist.
fn json_pypi(url_sdist: &str, sha: &str) -> String {
    format!(
        r#"{{"urls":[{{"packagetype":"bdist_wheel","url":"https://files.pythonhosted.org/x.whl","filename":"x.whl","digests":{{"sha256":"{}"}}}},{{"packagetype":"sdist","url":"{url_sdist}","filename":"fix-1.0.tar.gz","digests":{{"sha256":"{sha}"}}}}]}}"#,
        "0".repeat(64)
    )
}

#[test]
fn descarga_python_comando_wheels_usa_only_binary_y_opciones_de_indice() {
    let req = requisitos_pip("--extra-index-url https://pypi.ejemplo.org/simple\nsix==1.16.0\n");
    let c = comando_wheels("/cache/indice.txt", &req);
    for esperado in [
        "pip download",
        "--only-binary=:all:",
        "--no-input",
        "--disable-pip-version-check",
        "-r /cache/indice.txt",
        "-d /cache",
        "--index-url 'https://pypi.org/simple'",
        "--extra-index-url 'https://pypi.ejemplo.org/simple'",
    ] {
        assert!(c.contains(esperado), "falta {esperado}: {c}");
    }
}

#[test]
fn descarga_python_requisitos_con_dos_indices_pasan_ambos_a_pip() {
    let req = requisitos_pip(
        "--index-url https://indice.ejemplo.org/simple\n--extra-index-url https://pypi.ejemplo.org/simple\nsix==1.16.0\n",
    );
    let c = comando_wheels("/cache/indice.txt", &req);
    assert!(
        c.contains("--index-url 'https://indice.ejemplo.org/simple'"),
        "{c}"
    );
    assert!(
        c.contains("--extra-index-url 'https://pypi.ejemplo.org/simple'"),
        "{c}"
    );
    assert!(!c.contains("--index-url 'https://pypi.org/simple'"), "{c}");
}

#[test]
fn descarga_python_comandos_de_red_no_corren_setup_ni_build() {
    let req = requisitos_pip(&format!("{SIX_GIT}\nsix==1.16.0\n"));
    let wheels = comando_wheels("/cache/indice.txt", &req);
    let clon = comando_clon("https://github.com/benjaminp/six", "abc", SIX_COMMIT);
    let curl = comando_curl(
        "https://files.pythonhosted.org/fix-1.0.tar.gz",
        "fix-1.0.tar.gz",
    );
    for c in [&wheels, &clon, &curl] {
        assert!(
            !c.contains("setup.py") && !c.contains("pip install") && !c.contains("pip wheel"),
            "ningún comando de red corre build: {c}"
        );
    }
    assert!(wheels.contains("pip download"), "{wheels}");
    assert!(
        clon.contains("--no-checkout") && clon.contains(SIX_COMMIT),
        "{clon}"
    );
    assert!(clon.contains("core.hooksPath=/dev/null"), "{clon}");
    assert!(clon.contains("protocol.https.allow=always"), "{clon}");
    assert!(clon.contains("protocol.allow=never"), "{clon}");
    assert!(
        curl.contains("--proto '=https'") && curl.contains("--tlsv1.2"),
        "{curl}"
    );
    assert!(curl.contains("curl "), "{curl}");
}

#[test]
fn descarga_python_sin_wheel_nombra_el_paquete_que_falta() {
    let salida = "ERROR: Could not find a version that satisfies the requirement six==1.16.0 (from versions: none)\n\
ERROR: No matching distribution found for six==1.16.0\n";
    assert_eq!(sin_wheel(salida), Some("six==1.16.0".to_string()));
    assert_eq!(sin_wheel("error: failed to download `x`"), None);
}

#[test]
fn descarga_python_paquete_que_pypi_no_tiene_es_no_existe_y_no_infra() {
    // B1: el JSON de PyPI da 404 para un nombre mal escrito. Es un veredicto del producto, no infraestructura.
    let cache = repo_propio("cache-404-");
    let r = bajar_sdist_con("reqeusts", "2.31.0", cache.path(), |u, _| {
        Err(format!(
            "curl: (22) The requested URL returned error: 404 ({u})"
        ))
    });
    assert_eq!(r.err(), Some(FalloSdist::NoExiste));
}

#[test]
fn descarga_python_json_de_pypi_con_otro_error_sigue_siendo_otro() {
    let cache = repo_propio("cache-timeout-");
    let r = bajar_sdist_con("six", "1.16.0", cache.path(), |_, _| {
        Err("curl: (28) Operation timed out".to_string())
    });
    assert!(matches!(r, Err(FalloSdist::Otro(_))), "{r:?}");
}

#[test]
fn descarga_python_preludio_tiene_respaldo_si_venv_falla_y_pone_pydeps_en_el_path() {
    // M12: `import ensurepip` puede funcionar y `venv` fallar (Debian sin python3-venv).
    assert!(
        PRELUDIO_PYTHON.contains("if python3 -m venv --system-site-packages /tmp/venv-eval"),
        "{PRELUDIO_PYTHON}"
    );
    assert!(
        !PRELUDIO_PYTHON.contains("import ensurepip"),
        "{PRELUDIO_PYTHON}"
    );
    assert!(
        PRELUDIO_PYTHON.contains("PATH=/tmp/pydeps/bin:$PATH"),
        "{PRELUDIO_PYTHON}"
    );
}

#[test]
fn descarga_python_preludio_copia_las_fuentes_git_a_una_carpeta_escribible() {
    // M10: setuptools escribe `*.egg-info` en el árbol fuente. `/cache` es de solo lectura en la evaluación.
    assert!(
        PRELUDIO_PYTHON.contains("cp -r /cache/src/. /tmp/fuentes/"),
        "{PRELUDIO_PYTHON}"
    );
    let copia = PRELUDIO_PYTHON.find("cp -r").expect("la copia");
    let instala = PRELUDIO_PYTHON.find("pip install").expect("la instalación");
    assert!(
        copia < instala,
        "la copia va antes de instalar: {PRELUDIO_PYTHON}"
    );
}

#[test]
fn descarga_python_requisitos_e_git_con_commit_llega_a_la_descarga() {
    // M6: `-e git+…@commit` es una fuente git, no una línea que se descarta.
    let req = requisitos_pip(
        "-e git+https://github.com/benjaminp/six@c8e394065cd541a16c040515dc0afb85cf22a7c3#egg=six\n",
    );
    assert_eq!(req.directas.len(), 1, "{req:?}");
    assert!(
        req.directas[0]
            .1
            .starts_with("git+https://github.com/benjaminp/six@")
    );
}

#[test]
fn descarga_python_r_del_repo_llega_aplanado_y_c_solo_como_restriccion() {
    // M6: `-r` se sigue hasta la descarga; `-c` no instala nada, solo limita versiones.
    let repo = repo_propio("repo-py-r-c-");
    fs::create_dir_all(repo.path().join("requirements")).expect("carpeta");
    fs::write(
        repo.path().join("requirements.txt"),
        "-r requirements/base.txt\n-c restricciones.txt\n",
    )
    .expect("requirements");
    fs::write(repo.path().join("requirements/base.txt"), "six==1.16.0\n").expect("base");
    fs::write(repo.path().join("restricciones.txt"), "urllib3<2\n").expect("restricciones");
    let (requisitos, restricciones) = textos_de_requisitos(repo.path());
    assert!(requisitos.contains("six==1.16.0"), "{requisitos}");
    assert!(!requisitos.contains("urllib3"), "{requisitos}");
    assert!(restricciones.contains("urllib3<2"), "{restricciones}");
}

#[test]
fn descarga_python_git_sin_commit_o_con_tag_es_rechazo_sin_red() {
    // B2 y M7: los dos casos dan el mismo rechazo, antes de cualquier contenedor. No necesita podman.
    for linea in [
        "git+https://github.com/benjaminp/six#egg=six",
        "git+https://github.com/benjaminp/six@v1.16.0#egg=six",
    ] {
        let home = home_de_prueba();
        let repo = repo_propio("repo-py-git-sin-ref-");
        fs::write(repo.path().join("requirements.txt"), format!("{linea}\n"))
            .expect("requirements");
        let tarea = tarea_en_lenguaje(
            home.path(),
            "python",
            IMAGEN,
            &contrato_junit_ok(),
            &oculto_junit_ok(),
        );
        let preparacion = descarga::preparar(repo.path(), &tarea, home.path(), LIMITE)
            .expect("el rechazo no usa la red");
        let descarga::Preparacion::SinVeredicto(hallazgos) = preparacion else {
            panic!("`{linea}` debe cortar la evaluación: {preparacion:?}");
        };
        assert!(
            hallazgos.iter().any(|h| h.severidad == 3 && h.linea == 1),
            "{hallazgos:?}"
        );
    }
}

#[test]
fn descarga_python_sdist_con_hash_correcto_se_guarda() {
    let cache = repo_propio("cache-sdist-");
    let bytes = b"contenido del sdist";
    let url = "https://files.pythonhosted.org/packages/fix-1.0.tar.gz";
    let json = json_pypi(url, &sha256_hex(bytes));
    let sdist = bajar_sdist_con("fix", "1.0", cache.path(), |u, archivo| {
        if u.ends_with("/json") {
            fs::write(cache.path().join(archivo), json.as_bytes()).expect("json");
            Ok(())
        } else if u == url {
            fs::write(cache.path().join(archivo), bytes).expect("sdist");
            Ok(())
        } else {
            Err(format!("url inesperada: {u}"))
        }
    })
    .expect("el hash coincide: se guarda");
    assert_eq!(
        fs::read(cache.path().join(&sdist.archivo)).expect("guardado"),
        bytes
    );
    assert_eq!(sdist.sha256, sha256_hex(bytes));
}

#[test]
fn descarga_python_sdist_con_hash_incorrecto_da_error_y_no_guarda() {
    let cache = repo_propio("cache-sdist-malo-");
    let url = "https://files.pythonhosted.org/packages/fix-1.0.tar.gz";
    let json = json_pypi(url, &sha256_hex(b"el esperado"));
    let r = bajar_sdist_con("fix", "1.0", cache.path(), |u, archivo| {
        if u.ends_with("/json") {
            fs::write(cache.path().join(archivo), json.as_bytes()).expect("json");
        } else {
            fs::write(cache.path().join(archivo), b"otro contenido").expect("sdist");
        }
        Ok(())
    });
    assert!(r.is_err(), "un hash que no coincide nunca se acepta");
    assert!(
        !cache.path().join("fix-1.0.tar.gz").exists(),
        "el archivo malo se borra"
    );
}

#[test]
fn descarga_python_sdist_de_fixture_se_baja_y_su_setup_no_se_ejecuta() {
    let base = repo_propio("sdist-fixture-");
    let marca = base.path().join("marca-setup");
    let fuente = base.path().join("fuente");
    fs::create_dir_all(fuente.join("fix-1.0")).expect("fuente");
    fs::write(
        fuente.join("fix-1.0/setup.py"),
        format!("open({:?}, 'w').close()\n", marca.display().to_string()),
    )
    .expect("setup.py");
    let tgz = base.path().join("fix-1.0.tar.gz");
    let estado = Command::new("tar")
        .arg("-czf")
        .arg(&tgz)
        .arg("-C")
        .arg(&fuente)
        .arg("fix-1.0")
        .status()
        .expect("tar");
    assert!(estado.success(), "arma el fixture");
    let bytes = fs::read(&tgz).expect("lee el fixture");
    let url = "https://files.pythonhosted.org/packages/fix-1.0.tar.gz";
    let json = json_pypi(url, &sha256_hex(&bytes));
    let cache = base.path().join("cache");
    fs::create_dir_all(&cache).expect("cache");

    let sdist = bajar_sdist_con("fix", "1.0", &cache, |u, archivo| {
        if u.ends_with("/json") {
            fs::write(cache.join(archivo), json.as_bytes()).expect("json");
            Ok(())
        } else {
            fs::write(cache.join(archivo), &bytes).expect("sdist");
            Ok(())
        }
    })
    .expect("el sdist se baja");

    assert!(
        cache.join(&sdist.archivo).is_file(),
        "el sdist queda guardado"
    );
    assert!(!marca.exists(), "el setup.py del sdist no se ejecuta");
}

#[test]
fn descarga_python_preludio_instala_sin_red_con_target_en_tmpfs() {
    assert!(
        PRELUDIO_PYTHON.contains("--no-index --find-links /cache"),
        "{PRELUDIO_PYTHON}"
    );
    assert!(
        PRELUDIO_PYTHON.contains("--target /tmp/pydeps"),
        "{PRELUDIO_PYTHON}"
    );
    assert!(
        PRELUDIO_PYTHON.contains("PYTHONPATH=/tmp/pydeps"),
        "{PRELUDIO_PYTHON}"
    );
    assert!(PRELUDIO_PYTHON.ends_with("&& "), "{PRELUDIO_PYTHON}");
}

#[test]
fn descarga_python_wheel_de_pypi_se_instala_sin_red() {
    root::saltar_sin_podman!();
    let _g = preparar_py();
    let home = home_de_prueba();
    let repo = repo_propio("repo-py-wheel-");
    fs::write(repo.path().join("requirements.txt"), "six==1.16.0\n").expect("requirements");
    let contrato = "python3 -c 'import six' && echo '<testsuite><testcase classname=\"dep\" name=\"six\"/></testsuite>' > /salida/contrato.xml";
    let tarea = tarea_en_lenguaje(
        home.path(),
        "python",
        IMAGEN_PY,
        contrato,
        &oculto_junit_ok(),
    );

    let v = evaluar_con(home.path(), repo.path(), &tarea);

    assert_eq!(
        v.contrato,
        Conteo { pasa: 1, total: 1 },
        "hallazgos: {:?}",
        v.hallazgos
    );
    let descarga = gates(home.path());
    assert_eq!(descarga.len(), 1, "un Gate de descarga");
    let paquetes = descarga[0]["extra"]["paquetes"]
        .as_array()
        .expect("paquetes");
    assert!(
        paquetes.iter().any(|p| {
            p["nombre"]
                .as_str()
                .is_some_and(|n| n.starts_with("six-1.16.0") && n.ends_with(".whl"))
                && p["sha256"].is_string()
        }),
        "el wheel de six con su sha256 en el Gate: {paquetes:?}"
    );
}

#[test]
fn descarga_python_segunda_llamada_usa_la_cache_sin_red() {
    root::saltar_sin_podman!();
    let _g = preparar_py();
    let home = home_de_prueba();
    let repo = repo_propio("repo-py-cache-");
    fs::write(repo.path().join("requirements.txt"), "six==1.16.0\n").expect("requirements");
    let tarea = tarea_en_lenguaje(
        home.path(),
        "python",
        IMAGEN_PY,
        &contrato_junit_ok(),
        &oculto_junit_ok(),
    );

    descarga::preparar(repo.path(), &tarea, home.path(), LIMITE).expect("primera descarga");
    descarga::preparar(repo.path(), &tarea, home.path(), LIMITE).expect("segunda descarga");

    let gates = gates(home.path());
    assert_eq!(gates.len(), 2, "un Gate por descarga");
    assert_eq!(gates[1]["extra"]["cache_hit"], true);
}

#[test]
fn descarga_python_git_se_clona_al_commit_sin_pip_con_red() {
    root::saltar_sin_podman!();
    let _g = preparar_py();
    let home = home_de_prueba();
    let repo = repo_propio("repo-py-git-");
    fs::write(repo.path().join("requirements.txt"), format!("{SIX_GIT}\n")).expect("requirements");
    let tarea = tarea_en_lenguaje(
        home.path(),
        "python",
        IMAGEN_PY,
        &contrato_junit_ok(),
        &oculto_junit_ok(),
    );

    let prep = descarga::preparar(repo.path(), &tarea, home.path(), LIMITE).expect("clona con git");

    let descarga::Preparacion::Lista { hallazgos, .. } = prep else {
        panic!("la descarga de git no corta la evaluación");
    };
    assert!(
        hallazgos.is_empty(),
        "con commit fijo no hay hallazgo: {hallazgos:?}"
    );
    let gates = gates(home.path());
    let paquetes = gates[0]["extra"]["paquetes"].as_array().expect("paquetes");
    let six = paquetes
        .iter()
        .find(|p| p["fuente"] == "git")
        .expect("la fuente git en el Gate");
    assert_eq!(six["commit"], SIX_COMMIT);
}

#[test]
#[ignore = "pip construye la fuente git con setuptools en aislamiento; la imagen :2 no trae setuptools y la evaluación no tiene red. Falta bajar los wheels de build en la descarga (decisión abierta)"]
fn descarga_python_git_la_evaluacion_instala_sin_red() {
    let _g = preparar_py();
    let home = home_de_prueba();
    let repo = repo_propio("repo-py-git-eval-");
    fs::write(repo.path().join("requirements.txt"), format!("{SIX_GIT}\n")).expect("requirements");
    let contrato = "python3 -c 'import six' && echo '<testsuite><testcase classname=\"dep\" name=\"six\"/></testsuite>' > /salida/contrato.xml";
    let tarea = tarea_en_lenguaje(
        home.path(),
        "python",
        IMAGEN_PY,
        contrato,
        &oculto_junit_ok(),
    );
    let v = evaluar_con(home.path(), repo.path(), &tarea);
    assert_eq!(v.contrato, Conteo { pasa: 1, total: 1 });
}

/// Serializa los tests con podman y comprueba la imagen de Python (`:2`).
fn preparar_py() -> MutexGuard<'static, ()> {
    let guarda = SERIE.lock().unwrap_or_else(|e| e.into_inner());
    let existe = Command::new("podman")
        .args(["image", "exists", IMAGEN_PY])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    assert!(
        existe,
        "falta la imagen {IMAGEN_PY}: la descarga de Python la usa"
    );
    guarda
}

/// Juez falso de la configuración: responde un texto fijo y cuenta las llamadas.
struct JuezFalso {
    respuesta: String,
    llamadas: Cell<u32>,
}

impl ModeloJuez for JuezFalso {
    fn consultar(&self, _prompt: &Prompt) -> Result<RespuestaJuez, ErrorOraculo> {
        self.llamadas.set(self.llamadas.get() + 1);
        Ok(RespuestaJuez {
            texto: self.respuesta.clone(),
            truncada: false,
            costo_usd: 0.0,
        })
    }
}

const CARGO_JUEZ: &str =
    "[package]\nname = \"dep-juez\"\nversion = \"0.1.0\"\nedition = \"2021\"\n";
const CONFIG_GIT_CLI: &str = "[net]\ngit-fetch-with-cli = true\n";
const CLAVE_GIT_CLI: &str = "net.git-fetch-with-cli";
const CITA_GIT_CLI: &str = "git-fetch-with-cli = true";

/// Repo con `Cargo.toml` mínimo y `.cargo/config.toml` con `git-fetch-with-cli`. El lenguaje `shell`
/// no tiene perfil de descarga: la fase no usa la red ni podman, pero el inventario de cargo sí corre.
fn repo_juez() -> tempfile::TempDir {
    let repo = repo_propio("repo-juez-config-");
    fs::write(repo.path().join("Cargo.toml"), CARGO_JUEZ).expect("escribe Cargo.toml");
    escribir_config(repo.path(), CONFIG_GIT_CLI);
    repo
}

/// Fases de los Gates de la corrida de descarga de la tarea, en el orden en que se escribieron.
fn fases_de_gates(home: &Path) -> Vec<String> {
    eventos_crudos(home)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str::<Value>(l).expect("línea JSON"))
        .filter(|v| v["tipo"] == "gate" && v["corrida"] == format!("descarga-{TAREA}"))
        .map(|v| v["fase"].as_str().unwrap_or_default().to_string())
        .collect()
}

#[test]
fn descarga_juez_config_innecesaria_deja_hallazgo_de_entrega_gate_y_sigue() {
    let home = home_de_prueba();
    let repo = repo_juez();
    let tarea = tarea_en_lenguaje(
        home.path(),
        "shell",
        IMAGEN,
        &contrato_junit_ok(),
        &oculto_junit_ok(),
    );
    let juez = JuezFalso {
        respuesta: format!(
            r#"[{{"clave":"{CLAVE_GIT_CLI}","necesidad":"innecesaria","motivo":"ningún crate usa git-fetch-with-cli","cita":"{CITA_GIT_CLI}"}}]"#
        ),
        llamadas: Cell::new(0),
    };

    let (preparacion, _, juicio) =
        descarga::preparar_con_juez(repo.path(), &tarea, home.path(), LIMITE, Some(&juez))
            .expect("el juez no corta la fase de descarga");

    // La descarga sigue: el juez solo informa.
    let Preparacion::Lista { hallazgos, .. } = &preparacion else {
        panic!("la descarga debe seguir: {preparacion:?}");
    };
    let entrega: Vec<&Hallazgo> = hallazgos
        .iter()
        .filter(|h| h.dimension == Dimension::Entrega && h.cita == CITA_GIT_CLI)
        .collect();
    assert_eq!(
        entrega.len(),
        1,
        "un hallazgo de entrega del juez: {hallazgos:?}"
    );
    assert_eq!(entrega[0].severidad, 1);
    assert_eq!(juez.llamadas.get(), 1);
    assert!(
        matches!(&juicio, Some(ResultadoJuicio::Juzgado { juicios, .. }) if juicios.len() == 1),
        "{juicio:?}"
    );

    // Gates en orden: el inventario (que lista la entrada de red) y después el juez.
    assert_eq!(
        fases_de_gates(home.path()),
        vec!["inventario".to_string(), "juez-config".to_string()]
    );
    let inventario = gates_inventario(home.path());
    let config = inventario[0]["extra"]["config_cargo"]
        .as_array()
        .expect("config_cargo en el Gate del inventario");
    assert!(
        config.iter().any(|e| e["clave"] == CLAVE_GIT_CLI),
        "{config:?}"
    );
    let juicio_gate = gates_de(home.path(), "juez-config");
    assert_eq!(juicio_gate.len(), 1, "{juicio_gate:?}");
    assert_eq!(
        juicio_gate[0]["extra"]["juicios"][0]["clave"],
        CLAVE_GIT_CLI
    );
}

#[test]
fn descarga_juez_config_json_roto_deja_gate_sin_veredicto_y_sigue() {
    let home = home_de_prueba();
    let repo = repo_juez();
    let tarea = tarea_en_lenguaje(
        home.path(),
        "shell",
        IMAGEN,
        &contrato_junit_ok(),
        &oculto_junit_ok(),
    );
    let juez = JuezFalso {
        respuesta: "no es json [".into(),
        llamadas: Cell::new(0),
    };

    let (preparacion, _, juicio) =
        descarga::preparar_con_juez(repo.path(), &tarea, home.path(), LIMITE, Some(&juez))
            .expect("el juez roto no corta la fase de descarga");

    assert!(
        matches!(&preparacion, Preparacion::Lista { hallazgos, .. } if hallazgos.iter().all(|h| h.dimension != Dimension::Entrega)),
        "la descarga sigue y el juez no deja hallazgos: {preparacion:?}"
    );
    assert_eq!(
        juicio,
        Some(ResultadoJuicio::SinVeredicto {
            causa: "JSON inválido del juez".into()
        })
    );
    let juicio_gate = gates_de(home.path(), "juez-config");
    assert_eq!(juicio_gate.len(), 1, "{juicio_gate:?}");
    assert_eq!(juicio_gate[0]["extra"]["causa"], "JSON inválido del juez");
}
