//! `r00t aprende` y `r00t esquema replay` contra el binario real, con `ROOT_HOME` temporal.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use root::aprende::NOMBRE_POLITICA;
use root::tipos::{Estado, Evento, Fila, Rol, TipoEvento, Via};
use root::{observe, track};
use serde_json::{Value, json};
use tempfile::TempDir;

fn root_con_home(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_r00t"))
        .env("ROOT_HOME", home)
        .args(args)
        .output()
        .expect("no se pudo ejecutar el binario root")
}

fn codigo(salida: &Output) -> i32 {
    salida
        .status
        .code()
        .expect("el proceso no terminó con código")
}

fn json_de(salida: &Output) -> Value {
    serde_json::from_slice(&salida.stdout).expect("stdout es JSON")
}

/// Fila de construye en la tarea "memora" con `modelo` y `usd` de cupo.
fn fila_construye(id: &str, modelo: &str, usd: f64) -> Fila {
    let mut f = Fila {
        id: id.into(),
        tarea: "memora".into(),
        veredicto: root::tipos::Veredicto {
            estado: Estado::Verde,
            ..Default::default()
        },
        resumen: root::tipos::Resumen {
            usd_cupo: usd,
            ..Default::default()
        },
        ..Fila::default()
    };
    f.modelos.insert(Rol::Construye, modelo.into());
    f
}

/// Diez filas de haiku (0.5 USD) y diez de sonnet (2 USD), todas con éxito.
fn sembrar_filas(home: &Path) {
    let ruta = home.join("runs.jsonl");
    for i in 0..10 {
        let f = fila_construye(&format!("h{i}"), "haiku", 0.5);
        track::agregar_en(&ruta, &f).expect("siembra una fila");
        let g = fila_construye(&format!("s{i}"), "sonnet", 2.0);
        track::agregar_en(&ruta, &g).expect("siembra una fila");
    }
}

/// Llamada de un rol de evento, con `extra.rol`, `extra.tipo` y `extra.exito`.
fn llamada(modelo: &str, familia: &str, costo: f64, exito: bool) -> Evento {
    let mut extra = serde_json::Map::new();
    extra.insert("rol".into(), json!("delega"));
    extra.insert("tipo".into(), json!("resumen"));
    extra.insert("exito".into(), json!(exito));
    Evento {
        ts: 1.0,
        corrida: "c".into(),
        tipo: TipoEvento::Llamada,
        via: Some(Via::Api),
        modelo: Some(modelo.into()),
        familia: Some(familia.into()),
        cost_usd: costo,
        extra,
        ..Evento::default()
    }
}

/// Doce llamadas por modelo de delega/resumen. `deepseek-lite` es el más barato y no es el default.
fn sembrar_eventos(home: &Path) {
    let ruta = home.join("eventos.jsonl");
    for i in 0..12 {
        observe::registrar_en(&ruta, &llamada("deepseek-chat", "deepseek", 0.001, i != 0))
            .expect("siembra");
        observe::registrar_en(&ruta, &llamada("claude-haiku", "anthropic", 0.01, true))
            .expect("siembra");
        observe::registrar_en(&ruta, &llamada("deepseek-lite", "deepseek", 0.0002, true))
            .expect("siembra");
    }
}

/// `r00t route` para delega/resumen en la API. Devuelve la elección como JSON.
fn route_delega_resumen(home: &Path) -> Value {
    let salida = root_con_home(
        home,
        &["route", "--rol", "delega", "--tipo", "resumen", "--json"],
    );
    assert_eq!(
        codigo(&salida),
        0,
        "{}",
        String::from_utf8_lossy(&salida.stderr)
    );
    json_de(&salida)
}

#[test]
fn aprende_json_escribe_politica_y_salida_coincide() {
    let home = TempDir::new().expect("temp dir");
    sembrar_filas(home.path());
    let salida = root_con_home(home.path(), &["aprende", "--json"]);
    assert_eq!(
        codigo(&salida),
        0,
        "{}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let escrita: Value = serde_json::from_str(
        &fs::read_to_string(home.path().join(NOMBRE_POLITICA)).expect("politica.json"),
    )
    .expect("JSON válido");
    assert_eq!(json_de(&salida), escrita);
    let candidatos = &escrita["estadisticas"]["construye:memora"];
    assert_eq!(candidatos[0]["modelo"], "haiku");
}

#[test]
fn aprende_replay_json_no_escribe_politica() {
    let home = TempDir::new().expect("temp dir");
    sembrar_filas(home.path());
    let salida = root_con_home(home.path(), &["aprende", "--replay", "--json"]);
    assert_eq!(codigo(&salida), 0);
    let r = json_de(&salida);
    assert!(r["aprendida"].is_object());
    assert!(r["root_toml"].is_object());
    assert!(!home.path().join(NOMBRE_POLITICA).exists());
}

#[test]
fn aprende_sin_datos_da_politica_vacia_y_sale_con_0() {
    let home = TempDir::new().expect("temp dir");
    let salida = root_con_home(home.path(), &["aprende", "--json"]);
    assert_eq!(codigo(&salida), 0);
    assert_eq!(json_de(&salida)["estadisticas"], json!({}));
    assert!(home.path().join(NOMBRE_POLITICA).exists());
}

#[test]
fn aprende_linea_invalida_en_eventos_sale_con_3() {
    let home = TempDir::new().expect("temp dir");
    fs::write(home.path().join("eventos.jsonl"), "no es json\n").expect("escribe");
    let salida = root_con_home(home.path(), &["aprende"]);
    assert_eq!(codigo(&salida), 3);
}

#[test]
fn aprende_esquema_replay_sale_con_0() {
    let home = TempDir::new().expect("temp dir");
    let salida = root_con_home(home.path(), &["esquema", "replay"]);
    assert_eq!(codigo(&salida), 0);
    let esquema: Value = json_de(&salida);
    assert!(esquema["properties"]["mejora_usd"].is_object());
}

#[test]
fn aprende_route_lee_la_politica_escrita_y_elige_el_barato() {
    let home = TempDir::new().expect("temp dir");
    sembrar_eventos(home.path());
    let aprende = root_con_home(home.path(), &["aprende"]);
    assert_eq!(
        codigo(&aprende),
        0,
        "{}",
        String::from_utf8_lossy(&aprende.stderr)
    );
    // Con la política escrita, Route elige el más barato con Wilson sobre el umbral.
    let con_politica = route_delega_resumen(home.path());
    assert_eq!(con_politica["modelo"], "deepseek-lite");
    assert_eq!(con_politica["via"], "api");
    // Sin `politica.json`, Route vuelve al default de `[ruteo]`.
    fs::remove_file(home.path().join(NOMBRE_POLITICA)).expect("borra la política");
    let sin_politica = route_delega_resumen(home.path());
    assert_eq!(sin_politica["modelo"], "deepseek-chat");
}
