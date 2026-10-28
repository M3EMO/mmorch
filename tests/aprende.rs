//! `r00t aprende` y `r00t esquema replay` contra el binario real, con `ROOT_HOME` temporal.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use root::aprende::NOMBRE_POLITICA;
use root::tipos::{Estado, Evento, Fila, Forma, Rol, TipoEvento, Via};
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

/// Fila de `rol` con `modelo` y `usd` de cupo. Éxito verde.
fn fila_de(rol: Rol, id: &str, modelo: &str, usd: f64) -> Fila {
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
    f.modelos.insert(rol, modelo.into());
    f
}

/// Diez de haiku (0.5 USD) y diez de sonnet (2 USD) para organiza, todas con éxito.
fn sembrar_organiza(home: &Path) {
    let ruta = home.join("runs.jsonl");
    for i in 0..10 {
        track::agregar_en(
            &ruta,
            &fila_de(Rol::Organiza, &format!("h{i}"), "haiku", 0.5),
        )
        .expect("siembra una fila");
        track::agregar_en(
            &ruta,
            &fila_de(Rol::Organiza, &format!("s{i}"), "sonnet", 2.0),
        )
        .expect("siembra una fila");
    }
}

/// Eventos `Gate` con `fase == "propuesta"` en `eventos.jsonl` de `home`.
fn gates_propuesta(home: &Path) -> Vec<Evento> {
    root::aprende::leer_eventos(&home.join("eventos.jsonl"))
        .expect("lee eventos")
        .into_iter()
        .filter(|ev| ev.tipo == TipoEvento::Gate && ev.fase == "propuesta")
        .collect()
}

#[test]
fn aprende_json_incluye_propuesta_y_registra_un_gate() {
    let home = TempDir::new().expect("temp dir");
    sembrar_organiza(home.path());
    let salida = root_con_home(home.path(), &["aprende", "--json"]);
    assert_eq!(
        codigo(&salida),
        0,
        "{}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let json = json_de(&salida);
    let propuestas = json["propuestas"].as_array().expect("hay propuestas");
    assert_eq!(propuestas.len(), 1);
    assert_eq!(propuestas[0]["modelo_propuesto"], "haiku");
    assert_eq!(propuestas[0]["rol"], "organiza");
    let gates = gates_propuesta(home.path());
    assert_eq!(gates.len(), 1);
    assert_eq!(gates[0].corrida, "aprende");
    assert_eq!(gates[0].extra["rol"], "organiza");
    assert_eq!(gates[0].extra["tarea"], "memora");
    assert_eq!(gates[0].extra["modelo_propuesto"], "haiku");
}

#[test]
fn aprende_texto_imprime_la_propuesta() {
    let home = TempDir::new().expect("temp dir");
    sembrar_organiza(home.path());
    let salida = root_con_home(home.path(), &["aprende"]);
    assert_eq!(codigo(&salida), 0);
    let texto = String::from_utf8_lossy(&salida.stdout);
    assert!(texto.contains("propuestas"), "{texto}");
    assert!(
        texto.contains("organiza:memora: sonnet -> haiku"),
        "{texto}"
    );
    // La sección de formas también dice «sin propuestas» cuando no hay: se acota a la de modelos.
    assert!(!texto.contains("propuestas: sin propuestas"), "{texto}");
    assert_eq!(gates_propuesta(home.path()).len(), 1);
}

#[test]
fn aprende_sin_propuestas_no_registra_gate_y_lo_dice() {
    let home = TempDir::new().expect("temp dir");
    let ruta = home.path().join("runs.jsonl");
    for i in 0..2 {
        track::agregar_en(
            &ruta,
            &fila_de(Rol::Organiza, &format!("h{i}"), "haiku", 0.5),
        )
        .expect("siembra una fila");
        track::agregar_en(
            &ruta,
            &fila_de(Rol::Organiza, &format!("s{i}"), "sonnet", 2.0),
        )
        .expect("siembra una fila");
    }
    let salida = root_con_home(home.path(), &["aprende"]);
    assert_eq!(codigo(&salida), 0);
    let texto = String::from_utf8_lossy(&salida.stdout);
    assert!(texto.contains("sin propuestas"), "{texto}");
    assert!(gates_propuesta(home.path()).is_empty());
}

#[test]
fn aprende_replay_no_registra_gate_ni_escribe_propuestas() {
    let home = TempDir::new().expect("temp dir");
    sembrar_organiza(home.path());
    let salida = root_con_home(home.path(), &["aprende", "--replay", "--json"]);
    assert_eq!(codigo(&salida), 0);
    assert!(gates_propuesta(home.path()).is_empty());
    assert!(!home.path().join(NOMBRE_POLITICA).exists());
}

#[test]
fn aprende_route_organiza_sigue_con_sonnet_aunque_haya_propuesta() {
    let home = TempDir::new().expect("temp dir");
    sembrar_organiza(home.path());
    let aprende = root_con_home(home.path(), &["aprende"]);
    assert_eq!(codigo(&aprende), 0);
    let salida = root_con_home(
        home.path(),
        &["route", "--rol", "organiza", "--tipo", "memora", "--json"],
    );
    assert_eq!(
        codigo(&salida),
        0,
        "{}",
        String::from_utf8_lossy(&salida.stderr)
    );
    assert_eq!(json_de(&salida)["modelo"], "sonnet");
}

#[test]
fn aprende_esquema_propuesta_sale_con_0() {
    let home = TempDir::new().expect("temp dir");
    let salida = root_con_home(home.path(), &["esquema", "propuesta"]);
    assert_eq!(codigo(&salida), 0);
    assert!(json_de(&salida)["properties"]["evidencia"].is_object());
}

/// Fila de construye en "memora" con `forma`, `ts` y `usd` de cupo. Éxito verde.
fn fila_con_forma(forma: &Forma, id: &str, ts: f64, usd: f64) -> Fila {
    let mut f = fila_construye(id, "haiku", usd);
    f.ts = ts;
    f.forma = forma.clone();
    f
}

/// Caso 2 de la Tarea 15d: cinco filas de la forma A (2 USD, las más recientes) y cinco de la B (1 USD).
fn sembrar_formas(home: &Path) {
    let ruta = home.join("runs.jsonl");
    let a = Forma {
        roles: vec![Rol::Planifica, Rol::Organiza, Rol::Construye, Rol::Verifica],
        verificador: true,
        config: "ecc".into(),
    };
    let b = Forma {
        roles: vec![Rol::Planifica, Rol::Organiza, Rol::Construye],
        verificador: false,
        config: "ecc".into(),
    };
    for i in 0..5 {
        track::agregar_en(
            &ruta,
            &fila_con_forma(&b, &format!("b{i}"), 1.0 + f64::from(i), 1.0),
        )
        .expect("siembra una fila");
        track::agregar_en(
            &ruta,
            &fila_con_forma(&a, &format!("a{i}"), 6.0 + f64::from(i), 2.0),
        )
        .expect("siembra una fila");
    }
}

/// Gates con `fase == "propuesta-forma"` en `eventos.jsonl` de `home`.
fn gates_propuesta_forma(home: &Path) -> Vec<Evento> {
    root::aprende::leer_eventos(&home.join("eventos.jsonl"))
        .expect("lee eventos")
        .into_iter()
        .filter(|ev| ev.tipo == TipoEvento::Gate && ev.fase == "propuesta-forma")
        .collect()
}

#[test]
fn aprende_imprime_y_registra_propuesta_de_forma() {
    let home = TempDir::new().expect("temp dir");
    sembrar_formas(home.path());
    fs::write(home.path().join("root.toml"), "umbral_wilson = 0.4\n").expect("escribe root.toml");
    let salida = root_con_home(home.path(), &["aprende"]);
    assert_eq!(
        codigo(&salida),
        0,
        "{}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let texto = String::from_utf8_lossy(&salida.stdout);
    assert!(texto.contains("propuestas de forma:"), "{texto}");
    assert!(
        !texto.contains("propuestas de forma: sin propuestas"),
        "{texto}"
    );
    let gates = gates_propuesta_forma(home.path());
    assert_eq!(gates.len(), 1);
    assert_eq!(gates[0].corrida, "aprende");
    assert_eq!(gates[0].extra["tarea"], "memora");

    let salida = root_con_home(home.path(), &["aprende", "--json"]);
    assert_eq!(
        codigo(&salida),
        0,
        "{}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let json = json_de(&salida);
    assert_eq!(
        json["propuestas_forma"]
            .as_array()
            .expect("hay propuestas_forma")
            .len(),
        1
    );
}
