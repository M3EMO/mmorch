//! Tope de API lleno: `JuezProveedor` no envía el reintento del juez. Corre contra un servidor
//! falso en 127.0.0.1. Ninguna prueba sale a internet.
//!
//! Este archivo tiene un solo test. Así `set_var` no corre a la par con otro hilo que lea el entorno.

use std::fs;
use std::path::Path;

use root::oracle::MOTIVO_REINTENTO_TOPE;
use root::oracle::juez::{EntradaJuez, EstadoJuez, JuezProveedor, LimitesPrompt, juzgar};
use root::oracle::mapa;
use root::oracle::rubrica::Rubrica;
use root::oracle::tarea::Requisito;
use root::tipos::{Eleccion, Proveedor, Rol, Via};
use serde_json::{Value, json};

mod common;
use common::{Servidor, sse_deepseek};

/// Clave falsa. Nunca se lee la clave real del entorno.
const CLAVE_FALSA: &str = "clave-falsa-de-prueba";
const MODELO: &str = "deepseek-reasoner";
/// USD por millón de tokens de salida. Con 1000, cada token de salida cuesta 0.001 USD.
const PRECIO_SALIDA: f64 = 1000.0;
/// Tope de salida que el juez pide. El peor caso de cada llamada es `MAX_TOKENS` tokens de salida.
const MAX_TOKENS: u32 = 8000;
/// Tokens que el servidor falso cobra en la primera llamada.
const TOKENS_ENTRADA_1: u64 = 100;
const TOKENS_SALIDA_1: u64 = 50;

const SPEC: &str = "El almacén guarda pedidos.\nR1: guardar un pedido sin perder datos.";
const ALMACEN: &str = "pub struct Almacen;\n\nimpl Almacen {\n    pub fn guardar(&self) {}\n}\n";
const MODULARIDAD: &str = "diseno.modularidad";
const ITEM_OMITIDO: &str = "diseno.tope";

/// Peor caso de una llamada: todo `MAX_TOKENS` de salida, al precio. La entrada cuesta 0 USD.
fn peor_caso() -> f64 {
    f64::from(MAX_TOKENS) * PRECIO_SALIDA / 1e6
}

/// Costo real de la primera llamada: solo sus tokens de salida, porque la entrada cuesta 0 USD.
fn costo_real_primera() -> f64 {
    TOKENS_SALIDA_1 as f64 * PRECIO_SALIDA / 1e6
}

/// Tope que deja pasar la primera llamada y no la segunda. La primera cabe porque el tope supera
/// su peor caso. La segunda no cabe: el margen que queda es `tope - costo_real_primera`, y eso es
/// menor que el peor caso. Con la mitad del costo real de holgura, no hay casos cerca del borde.
fn tope_ajustado() -> f64 {
    peor_caso() + costo_real_primera() / 2.0
}

/// Escribe el `root.toml` del home: proveedor DeepSeek apuntando al servidor, precio y tope de API.
fn escribe_root_toml(home: &Path, url: &str, tope: f64) {
    let texto = format!(
        "[proveedores]\n\
         deepseek_url = \"{url}\"\n\
         espera_base_ms = 1\n\
         \n\
         [precios.\"{MODELO}\"]\n\
         usd_entrada = 0.0\n\
         usd_salida = {PRECIO_SALIDA:?}\n\
         \n\
         [balance.api]\n\
         tope_usd = {tope:?}\n"
    );
    fs::write(home.join("root.toml"), texto).expect("escribe root.toml");
}

/// Eventos de `eventos.jsonl` dentro de `home`, sin las reservas.
fn eventos(home: &Path) -> Vec<Value> {
    let texto = fs::read_to_string(home.join("eventos.jsonl")).unwrap_or_default();
    texto
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).expect("cada línea es JSON"))
        .filter(|ev| ev["tipo"] != "reserva")
        .collect()
}

/// Respuesta del juez: un nivel con evidencia real para `diseno.modularidad`, y un nivel para
/// `diseno.tope` con la misma evidencia.
fn respuesta_con(item: &str) -> String {
    let evidencia =
        json!([{"archivo": "src/almacen.rs", "linea": 4, "cita": "pub fn guardar(&self){}"}]);
    json!({
        "items": [{"item": item, "nivel": 2, "evidencia": evidencia}],
        "hallazgos": [],
    })
    .to_string()
}

#[test]
fn juez_con_tope_lleno_no_envia_el_reintento() {
    // SAFETY: el archivo tiene un solo test, así que nadie más lee ni escribe el entorno a la vez.
    unsafe { std::env::set_var("DEEPSEEK_API_KEY", CLAVE_FALSA) };

    // La primera respuesta omite `diseno.tope`: eso dispara el reintento. La segunda respondería
    // ese ítem, pero con tope lleno no debe llegar a enviarse.
    let srv = Servidor::arranca(vec![
        (
            200,
            sse_deepseek(
                &respuesta_con(MODULARIDAD),
                TOKENS_ENTRADA_1,
                TOKENS_SALIDA_1,
                "stop",
            ),
        ),
        (
            200,
            sse_deepseek(
                &respuesta_con(ITEM_OMITIDO),
                TOKENS_ENTRADA_1,
                TOKENS_SALIDA_1,
                "stop",
            ),
        ),
    ]);

    let home = tempfile::tempdir().expect("home");
    let tope = tope_ajustado();
    escribe_root_toml(home.path(), &srv.url, tope);

    let repo = tempfile::tempdir().expect("repo");
    fs::create_dir(repo.path().join("src")).expect("crea src");
    fs::write(repo.path().join("src/almacen.rs"), ALMACEN).expect("escribe almacen");
    let mapa = mapa::construir(repo.path()).expect("construye el mapa");
    let rubricas = vec![
        Rubrica::desde_toml(include_str!("fixtures/rubricas/diseno-valida.toml"))
            .expect("carga la rúbrica"),
    ];
    let requisitos = vec![Requisito {
        id: "R1".into(),
        titulo: "guardar un pedido sin perder datos".into(),
        seccion: String::new(),
        peso: 3,
        pruebas: vec![],
        bordes: vec![],
        escenarios: vec![],
        criterios: vec![],
        modulos: vec!["almacen".into()],
    }];

    let eleccion = Eleccion {
        via: Via::Api,
        proveedor: Some(Proveedor::Deepseek),
        modelo: Some(MODELO.into()),
        razonamiento: true,
        motivo: "prueba del tope".into(),
        rol: Some(Rol::Juzga),
    };
    let juez = JuezProveedor::nuevo_en(home.path(), eleccion, "c-tope".into())
        .expect("la elección del juez es válida");
    let entrada = EntradaJuez {
        spec: SPEC,
        requisitos: &requisitos,
        mapa: &mapa,
        hallazgos_sondas: &[],
        rubricas: &rubricas,
        repo: repo.path(),
        inicial: None,
    };
    let limites = LimitesPrompt {
        max_tokens: MAX_TOKENS,
        ..LimitesPrompt::default()
    };

    let r = juzgar(&entrada, &juez, MODELO, &limites);

    // (1) El servidor recibió una sola petición: el reintento no salió.
    assert_eq!(
        srv.peticiones(),
        1,
        "con el tope lleno no debe salir el reintento"
    );

    // (2) El ítem omitido queda como faltante, con su descarte y el motivo del tope.
    assert_eq!(r.estado, EstadoJuez::Ok);
    assert_eq!(
        r.niveles.get(MODULARIDAD),
        Some(&2),
        "la primera respuesta sí se usó"
    );
    assert_eq!(r.faltantes, vec![ITEM_OMITIDO.to_string()]);
    let descartes: Vec<(String, String)> = r
        .descartes
        .iter()
        .map(|d| (d.que.clone(), d.motivo.clone()))
        .collect();
    assert!(
        descartes.contains(&(
            format!("ítem {ITEM_OMITIDO}"),
            MOTIVO_REINTENTO_TOPE.to_string()
        )),
        "descartes: {descartes:?}"
    );

    // (3) El log tiene un `Gate` de tope y una sola `Llamada`, sin la marca de reintento.
    let evs = eventos(home.path());
    let llamadas: Vec<&Value> = evs.iter().filter(|ev| ev["tipo"] == "llamada").collect();
    let gates: Vec<&Value> = evs.iter().filter(|ev| ev["tipo"] == "gate").collect();
    assert_eq!(llamadas.len(), 1, "eventos: {evs:?}");
    assert!(
        llamadas[0]["extra"].get("reintento_juez").is_none(),
        "{}",
        llamadas[0]["extra"]
    );
    assert_eq!(gates.len(), 1, "eventos: {evs:?}");
    assert_eq!(gates[0]["fase"], "tope");
}
