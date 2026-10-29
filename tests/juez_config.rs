//! Juez de la configuración de Cargo con `[jueces].config_modelo`, contra el servidor HTTP falso.
//! Un solo test: `ROOT_HOME` y la clave de Anthropic viven en el entorno del proceso, y así no hay
//! otro test en paralelo que los lea.

mod common;

use std::fs;

use root::config::Config;
use root::oracle::descarga::inventario::inventariar;
use root::oracle::descarga::juez_config::{ResultadoJuicio, juez_config_real, juzgar_config};
use serde_json::Value;

const ANTHROPIC_OK: &str =
    r#"{"content":[{"type":"text","text":"[]"}],"usage":{"input_tokens":10,"output_tokens":5}}"#;
const CARGO_TOML: &str = "[package]\nname = \"dep-prueba\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nhumantime = \"2\"\n";
const CONFIG_PROXY: &str = "[http]\nproxy = \"http://127.0.0.1:9\"\n";

#[test]
fn juez_config_usa_jueces_config_modelo_y_sin_precio_no_envia() {
    let srv = common::Servidor::arranca(vec![(200, ANTHROPIC_OK)]);
    let home = tempfile::tempdir().expect("home");
    let repo = tempfile::tempdir().expect("repo");
    fs::create_dir_all(repo.path().join(".cargo")).expect("crea .cargo");
    fs::write(repo.path().join("Cargo.toml"), CARGO_TOML).expect("Cargo.toml");
    fs::write(repo.path().join(".cargo/config.toml"), CONFIG_PROXY).expect("config");
    let inv = inventariar(repo.path()).expect("inventario");
    assert!(
        !inv.config_cargo.is_empty(),
        "el fixture debe tener una entrada de config"
    );
    let textos = vec![
        (".cargo/config.toml".to_string(), CONFIG_PROXY.to_string()),
        ("Cargo.toml".to_string(), CARGO_TOML.to_string()),
    ];

    let ruta = home.path().join("root.toml");
    fs::write(
        &ruta,
        format!(
            "[proveedores]\nanthropic_url = \"{}\"\nespera_base_ms = 1\n",
            srv.url
        ),
    )
    .expect("root.toml");
    // SAFETY: único test de este binario; nadie más lee el entorno en paralelo.
    unsafe {
        std::env::set_var("ROOT_HOME", home.path());
        std::env::set_var("ANTHROPIC_API_KEY", "sk-test-SECRETO-JUEZ");
    }

    // Por defecto, el juez llama a Sonnet con razonamiento adaptativo.
    let cfg = Config::cargar(&ruta).expect("carga");
    let juez = juez_config_real(&cfg).expect("juez");
    let _ = juzgar_config(&juez, &inv.config_cargo, &textos, CARGO_TOML, &inv, None);
    assert_eq!(srv.peticiones(), 1);
    let cuerpo: Value = serde_json::from_str(&srv.cuerpo(0)).expect("JSON");
    assert_eq!(cuerpo["model"], "claude-sonnet-5-5");
    assert_eq!(cuerpo["thinking"], serde_json::json!({"type": "adaptive"}));

    // Un modelo sin precio: el juez da sin veredicto y no envía nada.
    fs::write(
        &ruta,
        format!(
            "[proveedores]\nanthropic_url = \"{}\"\nespera_base_ms = 1\n\n[jueces]\nconfig_modelo = \"claude-opus-5-5\"\n",
            srv.url
        ),
    )
    .expect("root.toml");
    let cfg = Config::cargar(&ruta).expect("carga");
    let juez = juez_config_real(&cfg).expect("juez");
    match juzgar_config(&juez, &inv.config_cargo, &textos, CARGO_TOML, &inv, None) {
        ResultadoJuicio::SinVeredicto { causa } => {
            assert!(causa.contains("sin precio"), "{causa}");
        }
        otro => panic!("debía dar sin veredicto: {otro:?}"),
    }
    assert_eq!(srv.peticiones(), 1, "sin precio no se envía");
}
