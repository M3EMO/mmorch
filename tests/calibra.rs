//! Tests de `r00t oraculo calibrar` (Tarea 12): la salida del binario y la escritura de `calibracion.json`.
//! El Oracle real corre con podman; estos tests no lo llaman: solo prueban los errores de uso y la escritura.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use root::oracle::calibra::{ARCHIVO, Calibraciones, calibrar_tarea};
use root::oracle::tarea::Tarea;
use root::oracle::{OpcionesOraculo, Partes};
use root::tipos::Dimension;
use root::tipos::{Puntaje, Veredicto};

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/calibra/repo-rust"
);

const TAREA_BASE: &str = r#"
nombre = "calibra-fixture"
lenguaje = "rust"
imagen = "root-oraculo-rust:1"
spec = "bootstrap/docs/SPEC.md"

[contrato]
cmd = "cargo test"

[oculto]
cmd = "cargo test"
"#;

fn r00t(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_r00t"))
        .env("ROOT_HOME", home)
        .args(args)
        .output()
        .expect("no se pudo ejecutar el binario r00t")
}

fn codigo(salida: &Output) -> i32 {
    salida.status.code().expect("el proceso terminó con código")
}

/// `ROOT_HOME` con `oraculos/calibra-fixture/tarea.toml`. `extra` se agrega al TOML.
fn home_con_tarea(extra: &str) -> tempfile::TempDir {
    let home = tempfile::tempdir().expect("home");
    let carpeta = home.path().join("oraculos").join("calibra-fixture");
    std::fs::create_dir_all(&carpeta).expect("carpeta");
    std::fs::write(carpeta.join("tarea.toml"), format!("{TAREA_BASE}{extra}")).expect("tarea");
    home
}

/// Veredicto de seguridad sin hallazgos: el evaluador falso de estos tests no detecta nada.
fn veredicto_plano() -> Veredicto {
    Veredicto {
        puntajes: vec![Puntaje {
            dimension: Dimension::Seguridad,
            valor: 1.0,
            calibrada: false,
            exactitud_balanceada: None,
            items: vec![],
            sin_veredicto: None,
        }],
        ..Veredicto::default()
    }
}

#[test]
fn calibra_cli_sin_repo_ni_calibracion_sale_con_2() {
    let home = home_con_tarea("");
    let salida = r00t(
        home.path(),
        &[
            "oraculo",
            "calibrar",
            "--tarea",
            "calibra-fixture",
            "--dimension",
            "seguridad",
        ],
    );
    assert_eq!(
        codigo(&salida),
        2,
        "{}",
        String::from_utf8_lossy(&salida.stderr)
    );
    assert!(String::from_utf8_lossy(&salida.stderr).contains("--repo"));
    assert!(
        !home.path().join(ARCHIVO).exists(),
        "sin repo no escribe nada"
    );
}

#[test]
fn calibra_cli_dimension_desconocida_sale_con_2() {
    let home = home_con_tarea("");
    let salida = r00t(
        home.path(),
        &[
            "oraculo",
            "calibrar",
            "--tarea",
            "calibra-fixture",
            "--dimension",
            "magia",
            "--repo",
            FIXTURE,
        ],
    );
    assert_eq!(codigo(&salida), 2);
}

#[test]
fn calibra_cli_repo_inexistente_sale_con_2() {
    let home = home_con_tarea("");
    let salida = r00t(
        home.path(),
        &[
            "oraculo",
            "calibrar",
            "--tarea",
            "calibra-fixture",
            "--dimension",
            "seguridad",
            "--repo",
            "/ruta/que/no/existe/calibra",
        ],
    );
    assert_eq!(
        codigo(&salida),
        2,
        "{}",
        String::from_utf8_lossy(&salida.stderr)
    );
}

#[test]
fn calibra_cli_tarea_inexistente_sale_con_3() {
    let home = tempfile::tempdir().expect("home");
    let salida = r00t(
        home.path(),
        &[
            "oraculo",
            "calibrar",
            "--tarea",
            "no-existe",
            "--dimension",
            "seguridad",
            "--repo",
            FIXTURE,
        ],
    );
    assert_eq!(
        codigo(&salida),
        3,
        "{}",
        String::from_utf8_lossy(&salida.stderr)
    );
}

#[test]
fn calibra_tarea_dos_corridas_agregan_filas_y_no_borran_las_anteriores() {
    let home = home_con_tarea("");
    let evaluador = |_: &Tarea, _: &Path, _: &OpcionesOraculo| Ok(veredicto_plano());
    let mut rutas = Vec::<PathBuf>::new();
    for semilla in [42, 43] {
        calibrar_tarea(
            home.path(),
            "calibra-fixture",
            "seguridad",
            4,
            semilla,
            Some(Path::new(FIXTURE)),
            &evaluador,
        )
        .expect("calibra");
        rutas.push(home.path().join(ARCHIVO));
    }
    let leido = Calibraciones::cargar(home.path()).expect("carga calibracion.json");
    assert_eq!(leido.filas.len(), 2, "una fila por corrida");
    assert_eq!(leido.filas[0].semilla, 42, "la primera fila sigue ahí");
    assert_eq!(leido.filas[1].semilla, 43);
    assert_eq!(leido.filas[0].dimension, Dimension::Seguridad);
}

#[test]
fn calibra_tarea_evalua_la_base_y_cada_caso_con_las_partes_de_su_dimension() {
    let home = home_con_tarea("");
    let vistas = std::cell::RefCell::new(Vec::<Option<Partes>>::new());
    let evaluador = |_: &Tarea, _: &Path, o: &OpcionesOraculo| {
        vistas.borrow_mut().push(o.partes);
        Ok(veredicto_plano())
    };
    calibrar_tarea(
        home.path(),
        "calibra-fixture",
        "seguridad",
        4,
        42,
        Some(Path::new(FIXTURE)),
        &evaluador,
    )
    .expect("calibra");
    let vistas = vistas.borrow();
    // Línea base más cuatro casos: todos con las partes de Seguridad.
    assert_eq!(vistas.len(), 5, "{vistas:?}");
    for partes in vistas.iter() {
        let partes = partes.expect("la calibración pasa partes");
        assert_eq!(partes.dimension, Some(Dimension::Seguridad));
        assert!(partes.mide(Dimension::Seguridad));
        assert!(!partes.mide(Dimension::Diseno));
        assert!(!partes.oculto, "Seguridad no corre la suite oculta");
        assert!(!partes.redactor, "la calibración no llama al redactor");
    }
}
