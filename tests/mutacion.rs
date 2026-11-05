//! Sonda de mutación (Tarea 9e). Los fixtures validan el puntaje; la prueba de contenedor corre
//! `sonda_mutacion` contra podman real, sin red. La imagen debe existir localmente.

use std::cell::RefCell;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use root::oracle::ErrorOraculo;
use root::oracle::descarga::PRELUDIO;
use root::oracle::mutacion::{con_archivo_acotado, parsear_outcomes, sonda_mutacion};
use root::oracle::perfiles::herramientas::{
    EjecutorContenedor, EjecutorHerramienta, SalidaHerramienta,
};
use root::oracle::tarea::Tarea;

/// Imagen de evaluación de Rust. Hoy no trae `cargo-mutants`.
const RUST: &str = "localhost/root-oraculo-rust:1";
const LIMITE: Duration = Duration::from_secs(120);

fn fixture(nombre: &str) -> String {
    let ruta = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/mutacion")
        .join(nombre);
    fs::read_to_string(&ruta).unwrap_or_else(|e| panic!("lee {}: {e}", ruta.display()))
}

/// Falla con un mensaje claro si la imagen no está. Nunca la baja: el test no tiene red.
fn asegurar_imagen(imagen: &str) {
    let existe = Command::new("podman")
        .args(["image", "exists", imagen])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    assert!(existe, "falta la imagen {imagen} en podman");
}

/// Tarea de Rust mínima, con su `tarea.toml` en una carpeta temporal.
fn tarea_rust(dir: &Path) -> Tarea {
    fs::write(
        dir.join("tarea.toml"),
        "nombre = \"x\"\nlenguaje = \"rust\"\nimagen = \"i\"\nspec = \"s\"\n\
         [contrato]\ncmd = \"c\"\n[oculto]\ncmd = \"o\"\n",
    )
    .expect("escribe tarea.toml");
    Tarea::cargar_de(dir).expect("carga la tarea")
}

#[test]
fn mutacion_fixture_sin_asserts_puntaje_menor_que_con_asserts() {
    let con = parsear_outcomes(&fixture("outcomes-con-asserts.json"), 60, &[]).expect("parsea");
    let sin = parsear_outcomes(&fixture("outcomes-sin-asserts.json"), 60, &[]).expect("parsea");
    let valor_con = con.sonda.valor.expect("valor con asserts");
    let valor_sin = sin.sonda.valor.expect("valor sin asserts");
    assert_eq!(valor_con, 1.0);
    assert!(
        valor_sin < valor_con,
        "{valor_sin} debe ser menor que {valor_con}"
    );
    assert!((valor_sin - 2.0 / 9.0).abs() < 1e-6, "valor {valor_sin}");
    assert_eq!(sin.sonda.hallazgos.len(), 7);
    assert!(con.sonda.hallazgos.is_empty());
}

#[test]
fn mutacion_contenedor_sin_cargo_mutants_da_sin_veredicto() {
    root::saltar_sin_podman!();
    asegurar_imagen(RUST);
    let repo = tempfile::tempdir().expect("temp dir");
    fs::create_dir_all(repo.path().join("src")).expect("crea src");
    fs::write(repo.path().join("src/lib.rs"), "pub fn f() -> u8 { 1 }\n").expect("escribe");
    fs::write(
        repo.path().join("Cargo.toml"),
        "[package]\nname = \"x\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("escribe Cargo.toml");
    let carpeta = tempfile::tempdir().expect("temp dir");
    let tarea = tarea_rust(carpeta.path());
    let ej = EjecutorContenedor {
        imagen: RUST.into(),
        repo: repo.path().into(),
        oraculo: None,
        rustsec: None,
        cache: None,
        timeout: LIMITE,
        home: carpeta.path().into(),
    };
    let r = sonda_mutacion(&ej, &tarea);
    // Hoy la imagen no trae cargo-mutants. Si algún día la trae, este test debe cambiar:
    // el motivo ya no dirá "no está en la imagen de evaluación".
    assert_eq!(r.sonda.valor, None);
    assert_eq!(
        r.sonda.motivo.as_deref(),
        Some("cargo-mutants no está en la imagen de evaluación")
    );
}

/// Imagen con `cargo-mutants` (la `:2`). Solo para la prueba real de mutación sin mutantes.
const RUST_CON_MUTANTS: &str = "localhost/root-oraculo-rust:2";

#[test]
fn mutacion_real_sin_mutantes_da_sin_mutantes_para_medir() {
    root::saltar_sin_podman!();
    asegurar_imagen(RUST_CON_MUTANTS);
    // Un repo sin funciones ni expresiones que mutar: el listado llega vacío.
    let repo = tempfile::tempdir().expect("temp dir");
    fs::create_dir_all(repo.path().join("src")).expect("crea src");
    fs::write(repo.path().join("src/lib.rs"), "pub struct S;\n").expect("escribe");
    fs::write(
        repo.path().join("Cargo.toml"),
        "[package]\nname = \"x\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("escribe Cargo.toml");
    // El preludio copia /cache/config.toml: sin él, el comando no corre y el test mentiría.
    let cache = tempfile::tempdir().expect("temp dir");
    fs::write(cache.path().join("config.toml"), "").expect("escribe config.toml");
    let carpeta = tempfile::tempdir().expect("temp dir");
    let tarea = tarea_rust(carpeta.path());
    let ej = EjecutorContenedor {
        imagen: RUST_CON_MUTANTS.into(),
        repo: repo.path().into(),
        oraculo: None,
        rustsec: None,
        cache: Some(cache.path().into()),
        timeout: LIMITE,
        home: carpeta.path().into(),
    };
    let r = sonda_mutacion(&ej, &tarea);
    assert_eq!(r.sonda.valor, None, "{:?}", r.sonda);
    let motivo = r.sonda.motivo.unwrap_or_default();
    assert!(motivo.starts_with("sin mutantes para medir"), "{motivo}");
}

/// Repo con más mutantes que `mutantes_max` en los casos de prueba (16 o más). Algunos cazados.
const LIB_CON_TESTS: &str = "pub fn suma(a: u32, b: u32) -> u32 { a + b }\n\
pub fn doble(a: u32) -> u32 { a * 2 }\n\
pub fn es_par(a: u32) -> bool { a % 2 == 0 }\n\
pub fn cero() -> u32 { 0 }\n\
pub fn uno(a: u32) -> u32 { a / 1 }\n\
pub fn tres(a: u32) -> u32 { a - 0 }\n\
#[cfg(test)]\n\
mod tests {\n    #[test]\n    fn t() { assert_eq!(super::suma(1, 2), 3); assert!(super::es_par(2)); }\n}\n";

fn repo_de_mutantes() -> tempfile::TempDir {
    let repo = tempfile::tempdir().expect("temp dir");
    fs::create_dir_all(repo.path().join("src")).expect("crea src");
    fs::write(repo.path().join("src/lib.rs"), LIB_CON_TESTS).expect("escribe lib.rs");
    fs::write(
        repo.path().join("Cargo.toml"),
        "[package]\nname = \"x\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("escribe Cargo.toml");
    repo
}

/// Tarea de Rust con `[pruebas]`: `mutantes_max` y 20 minutos.
fn tarea_con_pruebas(dir: &Path, mutantes_max: u32) -> Tarea {
    fs::write(
        dir.join("tarea.toml"),
        format!(
            "nombre = \"x\"\nlenguaje = \"rust\"\nimagen = \"i\"\nspec = \"s\"\n\
             [contrato]\ncmd = \"c\"\n[oculto]\ncmd = \"o\"\n\
             [pruebas]\nmutantes_max = {mutantes_max}\nminutos_max = 20\n"
        ),
    )
    .expect("escribe tarea.toml");
    Tarea::cargar_de(dir).expect("carga la tarea")
}

/// Ejecutor real que anota cada comando y cuántos mutantes trae el reporte de cada corrida.
struct Espia {
    dentro: EjecutorContenedor,
    cmds: RefCell<Vec<String>>,
    mutantes_del_reporte: RefCell<Vec<usize>>,
}

impl EjecutorHerramienta for Espia {
    fn ejecutar(&self, cmd: &str) -> Result<SalidaHerramienta, ErrorOraculo> {
        self.cmds.borrow_mut().push(cmd.to_string());
        let salida = self.dentro.ejecutar(cmd)?;
        if let Some(json) = salida.archivos.get("outcomes.json") {
            let valor: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
            let n = valor["outcomes"].as_array().map_or(0, |outs| {
                outs.iter()
                    .filter(|o| o["scenario"]["Mutant"].is_object())
                    .count()
            });
            self.mutantes_del_reporte.borrow_mut().push(n);
        }
        Ok(salida)
    }

    fn tiene_rustsec(&self) -> bool {
        self.dentro.tiene_rustsec()
    }
}

/// Ejecutor real sobre `repo`, con el `config.toml` de `cache` y la salida en `home`.
fn ejecutor_real(repo: &Path, cache: &Path, home: &Path) -> EjecutorContenedor {
    EjecutorContenedor {
        imagen: RUST_CON_MUTANTS.into(),
        repo: repo.into(),
        oraculo: None,
        rustsec: None,
        cache: Some(cache.into()),
        timeout: LIMITE,
        home: home.into(),
    }
}

/// Carpeta de caché con un `config.toml` vacío, que el preludio copia.
fn cache_vacia() -> tempfile::TempDir {
    let cache = tempfile::tempdir().expect("temp dir");
    fs::write(cache.path().join("config.toml"), "").expect("escribe config.toml");
    cache
}

#[test]
fn mutacion_real_corre_solo_la_muestra_de_mutantes_max() {
    root::saltar_sin_podman!();
    asegurar_imagen(RUST_CON_MUTANTS);
    let repo = repo_de_mutantes();
    let cache = cache_vacia();
    let carpeta = tempfile::tempdir().expect("temp dir");
    let tarea = tarea_con_pruebas(carpeta.path(), 3);
    let ej = Espia {
        dentro: ejecutor_real(repo.path(), cache.path(), carpeta.path()),
        cmds: RefCell::new(Vec::new()),
        mutantes_del_reporte: RefCell::new(Vec::new()),
    };
    let r = sonda_mutacion(&ej, &tarea);
    assert!(r.sonda.valor.is_some(), "{:?}", r.sonda);
    assert!(r.muestra >= 1 && r.muestra <= 3, "muestra {}", r.muestra);
    assert!(r.truncado, "hay más mutantes que la muestra");
    assert!(r.total_conocido.unwrap_or(0) > 3, "{:?}", r.total_conocido);
    let cmds = ej.cmds.borrow();
    let corridas = cmds
        .iter()
        .filter(|c| c.contains("--output /salida/mutants"))
        .count();
    let listados = cmds.iter().filter(|c| c.contains("--list")).count();
    assert_eq!((listados, corridas), (1, 1), "{cmds:?}");
    drop(cmds);
    let reporte = ej.mutantes_del_reporte.borrow();
    assert_eq!(reporte.len(), 1, "un solo reporte: {reporte:?}");
    assert!(
        (1..=3).contains(&reporte[0]),
        "cargo-mutants recibió más que la muestra: {reporte:?}"
    );
}

#[test]
fn mutacion_real_muestra_igual_que_el_comando_completo_viejo() {
    root::saltar_sin_podman!();
    asegurar_imagen(RUST_CON_MUTANTS);
    // Con menos mutantes que `mutantes_max`, la muestra nueva es la corrida completa de antes.
    let repo = repo_de_mutantes();
    let cache = cache_vacia();
    let carpeta = tempfile::tempdir().expect("temp dir");
    let tarea = tarea_con_pruebas(carpeta.path(), 60);
    let ej = ejecutor_real(repo.path(), cache.path(), carpeta.path());
    // Comando de la corrida completa antes del cambio: `git show HEAD:src/oracle/mutacion.rs`.
    let viejo = format!(
        "{PRELUDIO}cp -a /repo /salida/work && cd /salida/work && \
         CARGO_TARGET_DIR=/salida/target timeout 1200 cargo mutants --no-shuffle --jobs 1 \
         --output /salida/mutants -- --offline; \
         cp /salida/mutants/mutants.out/outcomes.json /salida/outcomes.json 2>/dev/null"
    );
    let salida = ej.ejecutar(&viejo).expect("corre el comando viejo");
    let json = salida.archivos.get("outcomes.json").expect("reporte viejo");
    let antes = parsear_outcomes(json, 60, &[]).expect("parsea el reporte viejo");
    let despues = sonda_mutacion(&ej, &tarea);
    assert!(despues.muestra > 0, "{:?}", despues.sonda);
    assert_eq!(despues.sonda.valor, antes.sonda.valor);
    assert_eq!(despues.muertos, antes.muertos);
    assert_eq!(despues.vivos, antes.vivos);
    assert_eq!(despues.muestra, antes.muestra);
    assert_eq!(despues.truncado, antes.truncado);
    assert_eq!(despues.sonda.hallazgos.len(), antes.sonda.hallazgos.len());
}

/// Ejecutor real que anota el archivo de cada mutante del reporte de la corrida.
struct Vigia {
    dentro: EjecutorContenedor,
    archivos_mutados: RefCell<Vec<String>>,
}

impl EjecutorHerramienta for Vigia {
    fn ejecutar(&self, cmd: &str) -> Result<SalidaHerramienta, ErrorOraculo> {
        let salida = self.dentro.ejecutar(cmd)?;
        if let Some(json) = salida.archivos.get("outcomes.json") {
            let valor: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
            for outcome in valor["outcomes"].as_array().into_iter().flatten() {
                if let Some(archivo) = outcome["scenario"]["Mutant"]["file"].as_str() {
                    self.archivos_mutados.borrow_mut().push(archivo.to_string());
                }
            }
        }
        Ok(salida)
    }

    fn tiene_rustsec(&self) -> bool {
        self.dentro.tiene_rustsec()
    }
}

/// GUARDA: la corre el orquestador con podman, no el builder.
/// Acotada a `src/uno.rs`, la corrida no mide ningún mutante de `src/dos.rs`.
#[test]
fn mutacion_acotada_a_un_archivo_no_mide_el_otro() {
    root::saltar_sin_podman!();
    asegurar_imagen(RUST_CON_MUTANTS);
    // El fixture se monta en solo lectura. Su copia de trabajo la hace el comando.
    let repo =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mutacion/repo-dos-archivos");
    let cache = cache_vacia();
    let carpeta = tempfile::tempdir().expect("temp dir");
    // Con 60 de máximo, la muestra cubre todos los mutantes del fixture.
    let tarea = tarea_con_pruebas(carpeta.path(), 60);
    let vigia = Vigia {
        dentro: ejecutor_real(&repo, cache.path(), carpeta.path()),
        archivos_mutados: RefCell::new(Vec::new()),
    };
    let r = con_archivo_acotado(Some("src/uno.rs"), || sonda_mutacion(&vigia, &tarea));
    assert!(r.sonda.valor.is_some(), "{:?}", r.sonda);
    assert!(r.muestra > 0, "{:?}", r.sonda);
    let mutados = vigia.archivos_mutados.borrow();
    assert!(!mutados.is_empty(), "el reporte no trae mutantes");
    assert!(mutados.iter().all(|a| a == "src/uno.rs"), "{mutados:?}");
    // `doble` no tiene prueba: al menos un hallazgo vivo sale de `src/uno.rs`.
    assert!(!r.sonda.hallazgos.is_empty(), "{:?}", r.sonda);
    for h in &r.sonda.hallazgos {
        assert_eq!(h.archivo, "src/uno.rs", "{h:?}");
    }
}
