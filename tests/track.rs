use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use root::tipos::{Estado, Fila, Veredicto};
use serde_json::Value;

fn root_con_home(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_root"))
        .env("ROOT_HOME", home)
        .args(args)
        .output()
        .expect("no se pudo ejecutar el binario root")
}

fn fila_json(id: &str, tarea: &str, config: &str, estado: Estado) -> String {
    let fila = Fila {
        id: id.into(),
        tarea: tarea.into(),
        config: config.into(),
        veredicto: Veredicto {
            estado,
            ..Veredicto::default()
        },
        ..Fila::default()
    };
    serde_json::to_string(&fila).expect("serializa la fila")
}

#[test]
fn track_cli_agregar_y_listar_devuelve_la_fila() {
    let home = tempfile::tempdir().expect("temp dir");
    let archivo = home.path().join("fila.json");
    fs::write(
        &archivo,
        fila_json("f-001", "memora", "haiku", Estado::Verde),
    )
    .expect("escribe");
    let archivo_str = archivo.to_str().expect("ruta utf-8");

    let agrega = root_con_home(home.path(), &["track", "agregar", "--fila", archivo_str]);
    assert_eq!(agrega.status.code(), Some(0), "{agrega:?}");

    let lista = root_con_home(home.path(), &["track", "listar", "--tarea", "memora"]);
    assert_eq!(lista.status.code(), Some(0));
    let filas: Value = serde_json::from_slice(&lista.stdout).expect("stdout es JSON");
    let arreglo = filas.as_array().expect("arreglo de filas");
    assert_eq!(arreglo.len(), 1);
    assert_eq!(arreglo[0]["id"], "f-001");
}

#[test]
fn track_cli_comparar_imprime_comparacion_json() {
    let home = tempfile::tempdir().expect("temp dir");
    for (id, config, estado) in [
        ("f-1", "fuerte", Estado::Verde),
        ("f-2", "debil", Estado::Rojo),
    ] {
        let archivo = home.path().join(format!("{id}.json"));
        fs::write(&archivo, fila_json(id, "memora", config, estado)).expect("escribe");
        let archivo_str = archivo.to_str().expect("ruta utf-8");
        let agrega = root_con_home(home.path(), &["track", "agregar", "--fila", archivo_str]);
        assert_eq!(agrega.status.code(), Some(0));
    }

    let salida = root_con_home(
        home.path(),
        &[
            "track", "comparar", "--a", "fuerte", "--b", "debil", "--tarea", "memora",
        ],
    );
    assert_eq!(salida.status.code(), Some(0));
    let json: Value = serde_json::from_slice(&salida.stdout).expect("stdout es JSON");
    assert_eq!(json["mejor"], "fuerte");
    assert_eq!(json["a"]["n"], 1);
    assert_eq!(json["b"]["exitos"], 0);
}

#[test]
fn track_cli_listar_sin_runs_da_arreglo_vacio() {
    let home = tempfile::tempdir().expect("temp dir");
    let salida = root_con_home(home.path(), &["track", "listar"]);
    assert_eq!(salida.status.code(), Some(0));
    let json: Value = serde_json::from_slice(&salida.stdout).expect("stdout es JSON");
    assert_eq!(json, Value::Array(vec![]));
}
