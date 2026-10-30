use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use root::ficha::{
    self, ContextoRevision, ErrorFicha, JuezAmbiguedad, revisar_con_juez, revisar_con_tope,
};
use root::observe;
use root::oracle::tarea::Tarea;
use root::oracle::{ErrorOraculo, OpcionesOraculo, evaluar};
use root::proveedores::{Prompt, RespuestaJuez};
use root::tipos::{Evento, TipoEvento, Via};

mod common;
use common::{Servidor, sse_deepseek};

/// Juez falso: devuelve una respuesta fija y guarda cada prompt que recibe.
struct JuezFalso {
    respuesta: Result<RespuestaJuez, String>,
    prompts: RefCell<Vec<Prompt>>,
}

impl JuezFalso {
    /// El proveedor del juez falla: no hay veredicto.
    fn con_error(causa: &str) -> JuezFalso {
        JuezFalso {
            respuesta: Err(causa.to_string()),
            prompts: RefCell::new(Vec::new()),
        }
    }

    fn ok(texto: &str) -> JuezFalso {
        JuezFalso {
            respuesta: Ok(RespuestaJuez {
                texto: texto.to_string(),
                truncada: false,
                costo_usd: 0.0,
            }),
            prompts: RefCell::new(Vec::new()),
        }
    }

    /// El modelo se quedó sin tope de salida: texto vacío y `truncada`.
    fn truncada() -> JuezFalso {
        JuezFalso::truncada_con("")
    }

    /// Respuesta truncada con un texto que, por sí solo, sí parsearía.
    fn truncada_con(texto: &str) -> JuezFalso {
        JuezFalso {
            respuesta: Ok(RespuestaJuez {
                texto: texto.to_string(),
                truncada: true,
                costo_usd: 0.0,
            }),
            prompts: RefCell::new(Vec::new()),
        }
    }

    fn falla(causa: &str) -> JuezFalso {
        JuezFalso {
            respuesta: Err(causa.to_string()),
            prompts: RefCell::new(Vec::new()),
        }
    }
}

impl JuezAmbiguedad for JuezFalso {
    fn consultar(&self, prompt: &Prompt) -> Result<RespuestaJuez, ErrorFicha> {
        self.prompts.borrow_mut().push(prompt.clone());
        self.respuesta.clone().map_err(ErrorFicha::Juez)
    }
}

/// Frase real de `spec-completa.md`: `concurrencia` de `[arquitectura]`.
const FRASE_REAL: &str = "un lock por fragmento del almacén";

/// Arreglo JSON con una ambigüedad de la frase real.
fn ambiguedad_real() -> String {
    format!(
        r#"[{{"frase": "{FRASE_REAL}", "lectura_a": "lock por fragmento de clave", "lectura_b": "lock por fragmento de nodo"}}]"#
    )
}

fn fixture(nombre: &str) -> String {
    let ruta = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/fichas")
        .join(nombre);
    fs::read_to_string(&ruta).unwrap_or_else(|e| panic!("no se pudo leer {nombre}: {e}"))
}

fn ctx_vacio() -> ContextoRevision {
    ContextoRevision::default()
}

fn root(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_r00t"))
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

fn ruta_fixture(nombre: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/fichas")
        .join(nombre)
        .display()
        .to_string()
}

fn linea_con(texto: &str, buscado: &str) -> usize {
    texto
        .lines()
        .position(|l| l.contains(buscado))
        .map(|i| i + 1)
        .unwrap_or_else(|| panic!("no aparece {buscado:?}"))
}

#[test]
fn ficha_completa_da_cero_hallazgos() {
    let texto = fixture("spec-completa.md");
    let revision = ficha::revisar(&texto, &ctx_vacio()).expect("la ficha parsea");
    assert!(revision.hallazgos.is_empty(), "{:?}", revision.hallazgos);
    assert!(revision.ambiguedades.is_empty());
    assert_eq!(revision.juez, "sin_veredicto: juez no corrido");
}

#[test]
fn ficha_sin_juez_sin_hallazgos_sale_3_y_no_0() {
    let texto = fixture("spec-completa.md");
    let revision = ficha::revisar(&texto, &ctx_vacio()).expect("la ficha parsea");
    assert!(revision.hallazgos.is_empty(), "{:?}", revision.hallazgos);
    assert_eq!(
        revision.codigo_de_salida(),
        3,
        "sin juez no hay veredicto limpio: {}",
        revision.juez
    );
}

#[test]
fn ficha_responde_rapido_sin_numero_da_su_linea_exacta() {
    let texto = fixture("spec-vaga.md");
    let revision = ficha::revisar(&texto, &ctx_vacio()).expect("la ficha parsea");
    let rapido: Vec<_> = revision
        .hallazgos
        .iter()
        .filter(|h| h.regla == "vaga" && h.cita == "rápido")
        .collect();
    assert_eq!(rapido.len(), 1, "{:?}", revision.hallazgos);
    assert_eq!(rapido[0].linea, linea_con(&texto, "responde rápido"));
}

#[test]
fn ficha_tbd_da_un_hallazgo_vago() {
    let texto = fixture("spec-vaga.md");
    let revision = ficha::revisar(&texto, &ctx_vacio()).expect("la ficha parsea");
    let tbd: Vec<_> = revision
        .hallazgos
        .iter()
        .filter(|h| h.regla == "vaga" && h.cita == "TBD")
        .collect();
    assert_eq!(tbd.len(), 1, "{:?}", revision.hallazgos);
    assert_eq!(tbd[0].linea, linea_con(&texto, "TBD"));
}

#[test]
fn ficha_requisito_sin_aceptacion_da_uno() {
    let texto = fixture("spec-sin-aceptacion.md");
    let revision = ficha::revisar(&texto, &ctx_vacio()).expect("la ficha parsea");
    let aceptacion: Vec<_> = revision
        .hallazgos
        .iter()
        .filter(|h| h.regla == "aceptacion")
        .collect();
    assert_eq!(aceptacion.len(), 1, "{:?}", revision.hallazgos);
    assert_eq!(aceptacion[0].cita, "R2");
}

#[test]
fn ficha_campo_pruebas_da_uno() {
    let texto = fixture("spec-con-pruebas.md");
    let revision = ficha::revisar(&texto, &ctx_vacio()).expect("la ficha parsea");
    let pruebas: Vec<_> = revision
        .hallazgos
        .iter()
        .filter(|h| h.regla == "sin_pruebas")
        .collect();
    assert_eq!(pruebas.len(), 1, "{:?}", revision.hallazgos);
    assert_eq!(pruebas[0].cita, "R1");
    assert_eq!(pruebas[0].linea, linea_con(&texto, "pruebas = "));
}

#[test]
fn ficha_sin_seccion_da_sin_ficha() {
    let texto = fixture("spec-sin-ficha.md");
    let error = ficha::extraer(&texto).expect_err("debe fallar");
    assert!(matches!(error, ErrorFicha::SinFicha), "{error:?}");
    assert_eq!(error.codigo_de_salida(), 2);
}

#[test]
fn ficha_bloque_roto_da_toml() {
    let texto = fixture("spec-toml-roto.md");
    let error = ficha::extraer(&texto).expect_err("debe fallar");
    assert!(matches!(error, ErrorFicha::Toml(_)), "{error:?}");
    assert_eq!(error.codigo_de_salida(), 2);
}

#[test]
fn ficha_dos_bloques_da_varios_bloques() {
    let texto = fixture("spec-dos-bloques.md");
    let error = ficha::extraer(&texto).expect_err("debe fallar");
    assert!(matches!(error, ErrorFicha::VariosBloques), "{error:?}");
}

#[test]
fn ficha_sin_bloque_toml_da_sin_bloque() {
    let texto = "# Memora\n\n## Ficha\n\nSolo prosa.\n";
    let error = ficha::extraer(texto).expect_err("debe fallar");
    assert!(matches!(error, ErrorFicha::SinBloque), "{error:?}");
}

#[test]
fn ficha_falta_una_parte_da_estructura() {
    let completa = fixture("spec-completa.md");
    let sin_diseno = completa
        .split("[diseno]")
        .next()
        .expect("tiene diseño")
        .to_string()
        + "```\n\nFin del spec.\n";
    let error = ficha::extraer(&sin_diseno).expect_err("debe fallar");
    assert!(matches!(error, ErrorFicha::Estructura(_)), "{error:?}");
    assert_eq!(error.codigo_de_salida(), 2);
}

#[test]
fn ficha_extraer_da_linea_de_inicio_del_bloque() {
    let texto = fixture("spec-completa.md");
    let (ficha_leida, bloque) = ficha::extraer(&texto).expect("parsea");
    assert_eq!(ficha_leida.producto.requisito.len(), 2);
    let cerca = linea_con(&texto, "```toml");
    assert_eq!(bloque.linea_inicio, cerca + 1);
    assert!(bloque.texto.starts_with("[producto]"));
}

#[test]
fn ficha_limite_sin_unidad_da_uno() {
    let texto = fixture("spec-completa.md");
    let sin_unidad = texto.replace("unidad = \"operaciones por segundo\"", "unidad = \"\"");
    assert_ne!(sin_unidad, texto);
    let revision = ficha::revisar(&sin_unidad, &ctx_vacio()).expect("parsea");
    let unidades: Vec<_> = revision
        .hallazgos
        .iter()
        .filter(|h| h.regla == "limite_unidad")
        .collect();
    assert_eq!(unidades.len(), 1, "{:?}", revision.hallazgos);
    assert_eq!(unidades[0].cita, "T1");
}

#[test]
fn ficha_capas_con_modulos_que_faltan_da_uno() {
    let texto = fixture("spec-completa.md");
    let modulos: BTreeSet<String> = ["protocolo", "comandos"].map(String::from).into();
    let ctx = ContextoRevision {
        modulos: Some(modulos),
        items_rubrica: None,
    };
    let revision = ficha::revisar(&texto, &ctx).expect("parsea");
    let capas: Vec<_> = revision
        .hallazgos
        .iter()
        .filter(|h| h.regla == "capas")
        .collect();
    assert_eq!(capas.len(), 1, "{:?}", revision.hallazgos);
    assert_eq!(capas[0].cita, "almacen");
}

#[test]
fn ficha_capas_sin_mapa_no_corre() {
    let texto = fixture("spec-completa.md");
    let revision = ficha::revisar(&texto, &ctx_vacio()).expect("parsea");
    assert!(revision.hallazgos.iter().all(|h| h.regla != "capas"));
}

#[test]
fn ficha_regla_item_fuera_de_rubrica_da_uno() {
    let texto = fixture("spec-completa.md");
    let items: BTreeSet<String> = ["diseno.errores"].map(String::from).into();
    let ctx = ContextoRevision {
        modulos: None,
        items_rubrica: Some(items),
    };
    let revision = ficha::revisar(&texto, &ctx).expect("parsea");
    let reglas: Vec<_> = revision
        .hallazgos
        .iter()
        .filter(|h| h.regla == "regla_item")
        .collect();
    assert_eq!(reglas.len(), 1, "{:?}", revision.hallazgos);
    assert_eq!(reglas[0].cita, "diseno.robustez");
}

#[test]
fn ficha_regla_item_con_rubrica_completa_no_da_hallazgos() {
    let texto = fixture("spec-completa.md");
    let items: BTreeSet<String> = ["diseno.robustez"].map(String::from).into();
    let ctx = ContextoRevision {
        modulos: None,
        items_rubrica: Some(items),
    };
    let revision = ficha::revisar(&texto, &ctx).expect("parsea");
    assert!(revision.hallazgos.is_empty(), "{:?}", revision.hallazgos);
}

#[test]
fn ficha_borrador_parsea_y_tarea_lo_acepta() {
    let texto = fixture("spec-completa.md");
    let (ficha_leida, _) = ficha::extraer(&texto).expect("parsea");
    let borrador = ficha::borrador_tarea(&ficha_leida, "memora").expect("arma el borrador");
    toml::from_str::<toml::Value>(&borrador).expect("TOML válido");
    assert_eq!(borrador.matches("[[requisito]]").count(), 2);

    let dir = tempfile::tempdir().expect("temp dir");
    fs::write(dir.path().join("tarea.toml"), &borrador).expect("escribe");
    let tarea = Tarea::cargar_de(dir.path()).expect("Tarea acepta el borrador");
    assert_eq!(tarea.nombre, "memora");
    assert_eq!(tarea.imagen, "localhost/root-oraculo-rust:1");
    assert_eq!(tarea.repeticiones, 5);
    assert_eq!(tarea.repeticiones_inestable, 10);

    let valor: toml::Value = toml::from_str(&borrador).expect("TOML válido");
    let requisitos = valor["requisito"]
        .as_array()
        .expect("arreglo de requisitos");
    assert_eq!(requisitos.len(), 2);
    for req in requisitos {
        assert_eq!(req["pruebas"].as_array().map(Vec::len), Some(0));
    }
    assert_eq!(valor["eficiencia"]["piso"].as_float(), Some(5000.0));
}

#[test]
fn ficha_cli_revisar_completa_sin_juez_sale_3_y_dice_juez_no_corrido() {
    let salida = root(&[
        "ficha",
        "revisar",
        "--spec",
        &ruta_fixture("spec-completa.md"),
    ]);
    assert_eq!(
        codigo(&salida),
        3,
        "{}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let texto = String::from_utf8_lossy(&salida.stdout);
    assert!(texto.contains("juez no corrido"), "{texto}");
}

#[test]
fn ficha_cli_revisar_vaga_sale_1_y_json_trae_linea() {
    let ruta = ruta_fixture("spec-vaga.md");
    let salida = root(&["--json", "ficha", "revisar", "--spec", &ruta]);
    assert_eq!(codigo(&salida), 1);
    let json: serde_json::Value = serde_json::from_slice(&salida.stdout).expect("stdout es JSON");
    let texto = fixture("spec-vaga.md");
    let linea = linea_con(&texto, "responde rápido");
    let trae = json["hallazgos"]
        .as_array()
        .expect("arreglo de hallazgos")
        .iter()
        .any(|h| h["linea"] == linea && h["cita"] == "rápido");
    assert!(trae, "{json}");
}

#[test]
fn ficha_cli_revisar_sin_ficha_sale_2() {
    let salida = root(&[
        "ficha",
        "revisar",
        "--spec",
        &ruta_fixture("spec-sin-ficha.md"),
    ]);
    assert_eq!(codigo(&salida), 2);
}

#[test]
fn ficha_cli_tarea_sale_0_con_requisito() {
    let salida = root(&[
        "ficha",
        "tarea",
        "--spec",
        &ruta_fixture("spec-completa.md"),
        "--nombre",
        "memora",
    ]);
    assert_eq!(
        codigo(&salida),
        0,
        "{}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let stdout = String::from_utf8_lossy(&salida.stdout);
    assert!(stdout.contains("[[requisito]]"), "{stdout}");
}

#[test]
fn ficha_esquema_revision_sale_0() {
    let salida = root(&["esquema", "revision"]);
    assert_eq!(codigo(&salida), 0);
    let schema: serde_json::Value = serde_json::from_slice(&salida.stdout).expect("stdout es JSON");
    assert_eq!(schema["title"], "Revision");
}

#[test]
fn ficha_esquema_revision_lista_descartadas() {
    let salida = root(&["esquema", "revision"]);
    let texto = String::from_utf8_lossy(&salida.stdout);
    assert!(texto.contains("descartadas"), "{texto}");
}

#[test]
fn ficha_juez_ambiguedad_real_entra_con_su_linea_y_juez_ok() {
    let texto = fixture("spec-completa.md");
    let juez = JuezFalso::ok(&ambiguedad_real());
    let revision = revisar_con_juez(&texto, &ctx_vacio(), &juez).expect("revisa");
    assert_eq!(revision.juez, "ok");
    assert_eq!(
        revision.ambiguedades.len(),
        1,
        "{:?}",
        revision.ambiguedades
    );
    assert_eq!(revision.ambiguedades[0].frase, FRASE_REAL);
    assert_eq!(
        revision.ambiguedades[0].linea,
        linea_con(&texto, "concurrencia =")
    );
    assert!(revision.descartadas.is_empty());
}

#[test]
fn ficha_juez_frase_inventada_se_descarta() {
    let texto = fixture("spec-completa.md");
    let json = r#"[{"frase": "el nodo replica en tres zonas", "lectura_a": "sincrónica", "lectura_b": "asincrónica"}]"#;
    let revision = revisar_con_juez(&texto, &ctx_vacio(), &JuezFalso::ok(json)).expect("revisa");
    assert_eq!(revision.juez, "ok");
    assert!(
        revision.ambiguedades.is_empty(),
        "{:?}",
        revision.ambiguedades
    );
    assert_eq!(revision.descartadas, vec!["el nodo replica en tres zonas"]);
}

#[test]
fn ficha_juez_frase_con_otro_espaciado_se_acepta() {
    let texto = fixture("spec-completa.md");
    let json = r#"[{"frase": "un   lock  por fragmento del almacén", "lectura_a": "por clave", "lectura_b": "por nodo"}]"#;
    let revision = revisar_con_juez(&texto, &ctx_vacio(), &JuezFalso::ok(json)).expect("revisa");
    assert_eq!(revision.ambiguedades.len(), 1, "{:?}", revision);
    assert_eq!(
        revision.ambiguedades[0].linea,
        linea_con(&texto, "concurrencia =")
    );
}

#[test]
fn ficha_juez_lecturas_iguales_o_vacias_se_descartan() {
    let texto = fixture("spec-completa.md");
    let json = format!(
        r#"[{{"frase": "{FRASE_REAL}", "lectura_a": "igual", "lectura_b": "igual"}},
            {{"frase": "{FRASE_REAL}", "lectura_a": "algo", "lectura_b": ""}}]"#
    );
    let revision = revisar_con_juez(&texto, &ctx_vacio(), &JuezFalso::ok(&json)).expect("revisa");
    assert!(
        revision.ambiguedades.is_empty(),
        "{:?}",
        revision.ambiguedades
    );
    assert_eq!(revision.descartadas.len(), 2, "{:?}", revision.descartadas);
}

#[test]
fn ficha_juez_json_roto_es_sin_veredicto_y_nunca_ok() {
    let texto = fixture("spec-completa.md");
    let revision =
        revisar_con_juez(&texto, &ctx_vacio(), &JuezFalso::ok("no es json")).expect("revisa");
    assert!(
        revision.juez.starts_with("sin_veredicto"),
        "{}",
        revision.juez
    );
    assert!(revision.ambiguedades.is_empty());
}

#[test]
fn ficha_juez_objeto_en_vez_de_arreglo_es_sin_veredicto() {
    let texto = fixture("spec-completa.md");
    let json = format!(r#"{{"frase": "{FRASE_REAL}", "lectura_a": "a", "lectura_b": "b"}}"#);
    let revision = revisar_con_juez(&texto, &ctx_vacio(), &JuezFalso::ok(&json)).expect("revisa");
    assert!(
        revision.juez.starts_with("sin_veredicto"),
        "{}",
        revision.juez
    );
    assert!(revision.ambiguedades.is_empty());
}

#[test]
fn ficha_juez_campos_faltantes_es_sin_veredicto() {
    let texto = fixture("spec-completa.md");
    let json = format!(r#"[{{"frase": "{FRASE_REAL}", "lectura_a": "a"}}]"#);
    let revision = revisar_con_juez(&texto, &ctx_vacio(), &JuezFalso::ok(&json)).expect("revisa");
    assert!(
        revision.juez.starts_with("sin_veredicto"),
        "{}",
        revision.juez
    );
    assert!(revision.ambiguedades.is_empty());
}

#[test]
fn ficha_juez_arreglo_envuelto_en_bloque_json_se_acepta() {
    let texto = fixture("spec-completa.md");
    let envuelto = format!("```json\n{}\n```", ambiguedad_real());
    let revision =
        revisar_con_juez(&texto, &ctx_vacio(), &JuezFalso::ok(&envuelto)).expect("revisa");
    assert_eq!(revision.juez, "ok");
    assert_eq!(revision.ambiguedades.len(), 1);
}

#[test]
fn ficha_juez_texto_fuera_del_bloque_es_sin_veredicto() {
    let texto = fixture("spec-completa.md");
    let json = format!("Revisé el spec:\n{}", ambiguedad_real());
    let revision = revisar_con_juez(&texto, &ctx_vacio(), &JuezFalso::ok(&json)).expect("revisa");
    assert!(
        revision.juez.starts_with("sin_veredicto"),
        "{}",
        revision.juez
    );
}

#[test]
fn ficha_juez_arreglo_vacio_es_ok_sin_ambiguedades() {
    let texto = fixture("spec-completa.md");
    let revision = revisar_con_juez(&texto, &ctx_vacio(), &JuezFalso::ok("[]")).expect("revisa");
    assert_eq!(revision.juez, "ok");
    assert!(revision.ambiguedades.is_empty());
    assert!(revision.descartadas.is_empty());
}

#[test]
fn ficha_juez_error_del_proveedor_es_sin_veredicto_y_deterministas_siguen() {
    let texto = fixture("spec-vaga.md");
    let revision =
        revisar_con_juez(&texto, &ctx_vacio(), &JuezFalso::falla("HTTP 500")).expect("revisa");
    assert!(
        revision.juez.starts_with("sin_veredicto"),
        "{}",
        revision.juez
    );
    assert!(revision.juez.contains("HTTP 500"), "{}", revision.juez);
    assert!(revision.ambiguedades.is_empty());
    assert!(!revision.hallazgos.is_empty());
}

#[test]
fn ficha_juez_prompt_fijo_refuta_pide_json_y_lleva_el_spec() {
    let texto = fixture("spec-completa.md");
    let juez = JuezFalso::ok("[]");
    revisar_con_juez(&texto, &ctx_vacio(), &juez).expect("revisa");
    let prompts = juez.prompts.borrow();
    assert_eq!(prompts.len(), 1);
    let prompt = &prompts[0];
    let sistema = prompt.sistema.as_deref().expect("hay prompt de sistema");
    assert!(sistema.contains("REFUTA"), "{sistema}");
    assert!(sistema.contains("JSON"), "{sistema}");
    assert_eq!(prompt.usuario, texto);
    assert_eq!(prompt.max_tokens, 32_000);
    assert_eq!(prompt.fase, "ficha");
}

#[test]
fn ficha_juez_sin_ficha_es_error_de_ficha() {
    let texto = fixture("spec-sin-ficha.md");
    let error =
        revisar_con_juez(&texto, &ctx_vacio(), &JuezFalso::ok("[]")).expect_err("debe fallar");
    assert!(matches!(error, ErrorFicha::SinFicha), "{error:?}");
}

#[test]
fn ficha_juez_error_codigo_de_salida_es_3() {
    let error = ErrorFicha::Juez("x".into());
    assert_eq!(error.codigo_de_salida(), 3);
}

#[test]
fn ficha_cli_juez_proveedor_sin_modelo_sale_2() {
    let salida = root(&[
        "ficha",
        "revisar",
        "--spec",
        &ruta_fixture("spec-completa.md"),
        "--juez-proveedor",
        "deepseek",
    ]);
    assert_eq!(codigo(&salida), 2);
}

#[test]
fn ficha_cli_juez_modelo_fuera_de_la_lista_sale_2_y_lo_nombra() {
    let home = tempfile::tempdir().expect("temp dir");
    // Sin clave: si la regla no corta antes, la salida sería 3 y no 2.
    let salida = Command::new(env!("CARGO_BIN_EXE_r00t"))
        .env("ROOT_HOME", home.path())
        .env_remove("DEEPSEEK_API_KEY")
        .args([
            "ficha",
            "revisar",
            "--spec",
            &ruta_fixture("spec-completa.md"),
            "--juez-proveedor",
            "deepseek",
            "--juez-modelo",
            "deepseek-chat",
        ])
        .output()
        .expect("ejecuta r00t");
    assert_eq!(codigo(&salida), 2);
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(stderr.contains("deepseek-chat"), "stderr: {stderr}");
    assert!(stderr.contains("deepseek-v4-pro"), "stderr: {stderr}");
}

#[test]
fn ficha_cli_juez_modelo_de_la_lista_no_sale_con_2_por_la_regla() {
    let home = tempfile::tempdir().expect("temp dir");
    let salida = Command::new(env!("CARGO_BIN_EXE_r00t"))
        .env("ROOT_HOME", home.path())
        .env_remove("DEEPSEEK_API_KEY")
        .args([
            "ficha",
            "revisar",
            "--spec",
            &ruta_fixture("spec-completa.md"),
            "--juez-proveedor",
            "deepseek",
            "--juez-modelo",
            "deepseek-v4-pro",
        ])
        .output()
        .expect("ejecuta r00t");
    // Sin clave, el juez no llama a la red: sale con 3 por falta de clave, no con el mensaje de la lista.
    assert_eq!(codigo(&salida), 3);
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(
        !stderr.contains("no admite razonamiento"),
        "stderr: {stderr}"
    );
}

#[test]
fn ficha_cli_juez_sin_clave_da_sin_veredicto_y_sale_3() {
    let home = tempfile::tempdir().expect("temp dir");
    let salida = Command::new(env!("CARGO_BIN_EXE_r00t"))
        .env("ROOT_HOME", home.path())
        .env_remove("DEEPSEEK_API_KEY")
        .args([
            "--json",
            "ficha",
            "revisar",
            "--spec",
            &ruta_fixture("spec-completa.md"),
            "--juez-proveedor",
            "deepseek",
            "--juez-modelo",
            "deepseek-v4-pro",
        ])
        .output()
        .expect("ejecuta r00t");
    assert_eq!(
        codigo(&salida),
        3,
        "{}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&salida.stdout).expect("stdout es JSON");
    assert!(
        json["juez"]
            .as_str()
            .is_some_and(|j| j.starts_with("sin_veredicto")),
        "{json}"
    );
}

#[test]
fn ficha_evaluar_sin_ficha_da_error_de_ficha_antes_de_contenedores() {
    let home = tempfile::tempdir().expect("home");
    let repo = tempfile::tempdir().expect("repo");
    let dir = home.path().join("oraculos").join("tarea-prueba");
    fs::create_dir_all(dir.join("docs")).expect("dir");
    fs::write(dir.join("docs/SPEC.md"), fixture("spec-sin-ficha.md")).expect("spec");
    fs::write(
        dir.join("tarea.toml"),
        "nombre = \"tarea-prueba\"\nlenguaje = \"rust\"\nimagen = \"x\"\nspec = \"docs/SPEC.md\"\n\n[contrato]\ncmd = 'true'\n\n[oculto]\ncmd = 'true'\n",
    )
    .expect("tarea");
    let tarea = Tarea::cargar_de(&dir).expect("carga");
    let opciones = OpcionesOraculo {
        timeout: std::time::Duration::from_secs(5),
        home: Some(home.path().to_path_buf()),
    };
    let res = evaluar(repo.path(), &tarea, &opciones);
    assert!(
        matches!(res, Err(ErrorOraculo::Ficha(ErrorFicha::SinFicha))),
        "{res:?}"
    );
}

#[test]
fn ficha_cli_evaluar_sin_ficha_sale_2() {
    let home = tempfile::tempdir().expect("home");
    let repo = tempfile::tempdir().expect("repo");
    let dir = home.path().join("oraculos").join("tarea-prueba");
    fs::create_dir_all(dir.join("docs")).expect("dir");
    fs::write(dir.join("docs/SPEC.md"), fixture("spec-sin-ficha.md")).expect("spec");
    fs::write(
        dir.join("tarea.toml"),
        "nombre = \"tarea-prueba\"\nlenguaje = \"rust\"\nimagen = \"x\"\nspec = \"docs/SPEC.md\"\n\n[contrato]\ncmd = 'true'\n\n[oculto]\ncmd = 'true'\n",
    )
    .expect("tarea");
    let salida = Command::new(env!("CARGO_BIN_EXE_r00t"))
        .env("ROOT_HOME", home.path())
        .args([
            "oraculo",
            "evaluar",
            "--tarea",
            "tarea-prueba",
            "--repo",
            repo.path().to_str().expect("ruta UTF-8"),
        ])
        .output()
        .expect("ejecuta r00t");
    assert_eq!(codigo(&salida), 2);
    assert!(String::from_utf8_lossy(&salida.stderr).contains("## Ficha"));
}

#[test]
fn ficha_tarea_borrador_de_fixture_acepta_tarea_y_spec_resuelve() {
    let texto = fixture("spec-completa.md");
    let (ficha_leida, _) = ficha::extraer(&texto).expect("parsea");
    let borrador = ficha::borrador_tarea(&ficha_leida, "memora").expect("borrador");
    let dir = tempfile::tempdir().expect("temp dir");
    fs::create_dir_all(dir.path().join("bootstrap/docs")).expect("dir");
    fs::write(dir.path().join("bootstrap/docs/SPEC.md"), &texto).expect("spec");
    fs::write(dir.path().join("tarea.toml"), &borrador).expect("tarea");
    let tarea = Tarea::cargar_de(dir.path()).expect("Tarea acepta el borrador");
    let spec = fs::read_to_string(dir.path().join(&tarea.spec)).expect("lee el spec");
    ficha::extraer(&spec).expect("la ficha resuelve");
}

#[test]
fn ficha_codigo_hallazgo_determinista_mas_juez_roto_sale_1() {
    let texto = fixture("spec-vaga.md");
    let revision =
        revisar_con_juez(&texto, &ctx_vacio(), &JuezFalso::ok("esto no es JSON")).expect("revisa");
    assert!(
        revision.juez.starts_with("sin_veredicto"),
        "{}",
        revision.juez
    );
    assert!(!revision.hallazgos.is_empty());
    assert_eq!(revision.codigo_de_salida(), 1);
}

#[test]
fn ficha_codigo_ficha_limpia_mas_juez_roto_sale_3() {
    let texto = fixture("spec-completa.md");
    let revision =
        revisar_con_juez(&texto, &ctx_vacio(), &JuezFalso::ok("esto no es JSON")).expect("revisa");
    assert!(
        revision.juez.starts_with("sin_veredicto"),
        "{}",
        revision.juez
    );
    assert!(revision.hallazgos.is_empty());
    assert!(revision.ambiguedades.is_empty());
    assert_eq!(revision.codigo_de_salida(), 3);
}

#[test]
fn ficha_codigo_ficha_limpia_mas_juez_ok_sin_ambiguedades_sale_0() {
    let texto = fixture("spec-completa.md");
    let revision = revisar_con_juez(&texto, &ctx_vacio(), &JuezFalso::ok("[]")).expect("revisa");
    assert_eq!(revision.juez, "ok");
    assert_eq!(revision.codigo_de_salida(), 0);
}

#[test]
fn ficha_codigo_sin_juez_y_ficha_limpia_sale_3() {
    let revision = ficha::revisar(&fixture("spec-completa.md"), &ctx_vacio()).expect("revisa");
    assert_eq!(revision.codigo_de_salida(), 3);
}

#[test]
fn ficha_cli_tarea_json_da_objeto_con_toml_que_parsea_y_tarea_lo_acepta() {
    let salida = root(&[
        "--json",
        "ficha",
        "tarea",
        "--spec",
        &ruta_fixture("spec-completa.md"),
        "--nombre",
        "memora",
    ]);
    assert_eq!(
        codigo(&salida),
        0,
        "{}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&salida.stdout).expect("stdout es un objeto JSON");
    let texto = json["toml"].as_str().expect("clave `toml` con texto");
    assert!(texto.contains("[[requisito]]"), "{texto}");
    toml::from_str::<toml::Value>(texto).expect("el TOML de la clave parsea");

    let dir = tempfile::tempdir().expect("temp dir");
    fs::create_dir_all(dir.path().join("bootstrap/docs")).expect("dir");
    fs::write(
        dir.path().join("bootstrap/docs/SPEC.md"),
        fixture("spec-completa.md"),
    )
    .expect("spec");
    fs::write(dir.path().join("tarea.toml"), texto).expect("tarea");
    Tarea::cargar_de(dir.path()).expect("Tarea acepta el TOML del JSON");
}

#[test]
fn ficha_cli_tarea_sin_json_sigue_siendo_toml_puro() {
    let salida = root(&[
        "ficha",
        "tarea",
        "--spec",
        &ruta_fixture("spec-completa.md"),
        "--nombre",
        "memora",
    ]);
    assert_eq!(codigo(&salida), 0);
    let texto = String::from_utf8_lossy(&salida.stdout);
    assert!(!texto.trim_start().starts_with('{'), "{texto}");
    toml::from_str::<toml::Value>(&texto).expect("stdout es TOML puro");
}

#[test]
fn ficha_cli_evaluar_sin_ficha_escribe_el_mensaje_una_sola_vez() {
    let home = tempfile::tempdir().expect("home");
    let repo = tempfile::tempdir().expect("repo");
    let dir = home.path().join("oraculos").join("tarea-prueba");
    fs::create_dir_all(dir.join("docs")).expect("dir");
    fs::write(dir.join("docs/SPEC.md"), fixture("spec-sin-ficha.md")).expect("spec");
    fs::write(
        dir.join("tarea.toml"),
        "nombre = \"tarea-prueba\"\nlenguaje = \"rust\"\nimagen = \"x\"\nspec = \"docs/SPEC.md\"\n\n[contrato]\ncmd = 'true'\n\n[oculto]\ncmd = 'true'\n",
    )
    .expect("tarea");
    let salida = Command::new(env!("CARGO_BIN_EXE_r00t"))
        .env("ROOT_HOME", home.path())
        .args([
            "oraculo",
            "evaluar",
            "--tarea",
            "tarea-prueba",
            "--repo",
            repo.path().to_str().expect("ruta UTF-8"),
        ])
        .output()
        .expect("ejecuta r00t");
    assert_eq!(codigo(&salida), 2);
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert_eq!(stderr.matches("sin ficha").count(), 1, "{stderr}");
    assert!(
        stderr.contains("sin ficha: el spec no tiene la sección `## Ficha`"),
        "{stderr}"
    );
}

#[test]
fn ficha_cli_spec_inexistente_nombra_la_causa_una_sola_vez() {
    let ruta = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/fichas/no-existe.md")
        .display()
        .to_string();
    let salida = root(&["ficha", "revisar", "--spec", &ruta]);
    assert_eq!(codigo(&salida), 2);
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert_eq!(stderr.matches("no se pudo leer").count(), 1, "{stderr}");
    assert_eq!(stderr.matches("No such file").count(), 1, "{stderr}");
}

// Tope de salida del juez de la ficha (Tarea 10c). Ningún test sale a internet: los pedidos van al servidor falso.

const CLAVE_FALSA: &str = "sk-test-FICHA-TOPE";
/// El juez responde un arreglo vacío y termina con `stop`.
fn juez_lista_vacia() -> String {
    sse_deepseek("[]", 1, 1, "stop")
}
/// El juez gastó todo el tope en razonamiento: contenido vacío y `finish_reason` = `length`.
fn juez_truncado() -> String {
    sse_deepseek("", 100, 32000, "length")
}

/// `ficha revisar` con el juez de DeepSeek apuntado al servidor falso. `ROOT_HOME` es `home`.
fn revisar_contra(srv: &Servidor, home: &Path, extra: &[&str]) -> Output {
    let texto = format!(
        "[proveedores]\ndeepseek_url = \"{}\"\nespera_base_ms = 1\n\n[precios.\"deepseek-flash\"]\nusd_entrada = 0.14\nusd_salida = 0.28\n",
        srv.url
    );
    fs::write(home.join("root.toml"), texto).expect("root.toml");
    Command::new(env!("CARGO_BIN_EXE_r00t"))
        .env("ROOT_HOME", home)
        .env("DEEPSEEK_API_KEY", CLAVE_FALSA)
        .env_remove("ANTHROPIC_API_KEY")
        .args([
            "--json",
            "ficha",
            "revisar",
            "--spec",
            &ruta_fixture("spec-completa.md"),
            "--juez-proveedor",
            "deepseek",
            "--juez-modelo",
            "deepseek-flash",
        ])
        .args(extra)
        .output()
        .expect("ejecuta r00t")
}

fn max_tokens_del_pedido(srv: &Servidor) -> serde_json::Value {
    let cuerpo: serde_json::Value =
        serde_json::from_str(&srv.cuerpo(0)).expect("el cuerpo es JSON");
    cuerpo["max_tokens"].clone()
}

#[test]
fn ficha_revisar_con_tope_pasa_el_tope_al_prompt() {
    let texto = fixture("spec-completa.md");
    let juez = JuezFalso::ok("[]");
    revisar_con_tope(&texto, &ctx_vacio(), &juez, 8000).expect("revisa");
    assert_eq!(juez.prompts.borrow()[0].max_tokens, 8000);
}

#[test]
fn ficha_juez_truncado_da_sin_veredicto_con_la_causa_del_tope() {
    let texto = fixture("spec-vaga.md");
    let revision = revisar_con_juez(&texto, &ctx_vacio(), &JuezFalso::truncada()).expect("revisa");
    assert_eq!(
        revision.juez,
        "sin_veredicto: tope de salida alcanzado (32000 tokens)"
    );
    assert!(
        !revision.juez.contains("JSON inválido"),
        "{}",
        revision.juez
    );
    // Las reglas deterministas siguen presentes aunque el juez se haya truncado.
    assert!(!revision.hallazgos.is_empty(), "{:?}", revision.hallazgos);
    assert!(revision.ambiguedades.is_empty());
    assert_eq!(revision.codigo_de_salida(), 1);
}

#[test]
fn ficha_juez_truncado_mira_la_bandera_antes_de_parsear() {
    // El texto parsearía bien; la bandera manda igual.
    let texto = fixture("spec-completa.md");
    let juez = JuezFalso::truncada_con(&ambiguedad_real());
    let revision = revisar_con_juez(&texto, &ctx_vacio(), &juez).expect("revisa");
    assert_eq!(
        revision.juez,
        "sin_veredicto: tope de salida alcanzado (32000 tokens)"
    );
    assert!(revision.ambiguedades.is_empty());
}

#[test]
fn ficha_codigo_truncado_en_ficha_limpia_sale_3() {
    let texto = fixture("spec-completa.md");
    let revision = revisar_con_juez(&texto, &ctx_vacio(), &JuezFalso::truncada()).expect("revisa");
    assert!(revision.hallazgos.is_empty());
    assert!(revision.ambiguedades.is_empty());
    assert_eq!(revision.codigo_de_salida(), 3);
}

#[test]
fn ficha_juez_no_truncado_json_roto_sigue_siendo_json_invalido() {
    let texto = fixture("spec-completa.md");
    let revision =
        revisar_con_juez(&texto, &ctx_vacio(), &JuezFalso::ok("no es json")).expect("revisa");
    assert!(revision.juez.contains("JSON inválido"), "{}", revision.juez);
}

#[test]
fn ficha_cli_juez_pide_32000_de_tope_por_defecto() {
    let srv = Servidor::arranca(vec![(200, juez_lista_vacia())]);
    let home = tempfile::tempdir().expect("home");
    let salida = revisar_contra(&srv, home.path(), &[]);
    assert_eq!(
        codigo(&salida),
        0,
        "{}",
        String::from_utf8_lossy(&salida.stderr)
    );
    assert_eq!(max_tokens_del_pedido(&srv), 32_000);
}

#[test]
fn ficha_cli_juez_max_tokens_llega_al_cuerpo_del_pedido() {
    let srv = Servidor::arranca(vec![(200, juez_lista_vacia())]);
    let home = tempfile::tempdir().expect("home");
    let salida = revisar_contra(&srv, home.path(), &["--juez-max-tokens", "8000"]);
    assert_eq!(
        codigo(&salida),
        0,
        "{}",
        String::from_utf8_lossy(&salida.stderr)
    );
    assert_eq!(max_tokens_del_pedido(&srv), 8000);
}

#[test]
fn ficha_cli_juez_max_tokens_cero_sale_2() {
    // Sin clave: si el valor 0 se aceptara, el juez daría sin_veredicto y no saldría ningún pedido.
    let home = tempfile::tempdir().expect("home");
    let salida = Command::new(env!("CARGO_BIN_EXE_r00t"))
        .env("ROOT_HOME", home.path())
        .env_remove("DEEPSEEK_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .args([
            "ficha",
            "revisar",
            "--spec",
            &ruta_fixture("spec-completa.md"),
            "--juez-proveedor",
            "deepseek",
            "--juez-modelo",
            "deepseek-flash",
            "--juez-max-tokens",
            "0",
        ])
        .output()
        .expect("ejecuta r00t");
    assert_eq!(codigo(&salida), 2);
}

#[test]
fn ficha_cli_juez_max_tokens_sin_proveedor_sale_2() {
    let salida = root(&[
        "ficha",
        "revisar",
        "--spec",
        &ruta_fixture("spec-completa.md"),
        "--juez-max-tokens",
        "8000",
    ]);
    assert_eq!(codigo(&salida), 2);
}

#[test]
fn ficha_cli_juez_max_tokens_sin_modelo_sale_2() {
    let salida = root(&[
        "ficha",
        "revisar",
        "--spec",
        &ruta_fixture("spec-completa.md"),
        "--juez-proveedor",
        "deepseek",
        "--juez-max-tokens",
        "8000",
    ]);
    assert_eq!(codigo(&salida), 2);
}

#[test]
fn ficha_cli_respuesta_truncada_da_la_causa_del_tope_y_registra_costo() {
    let srv = Servidor::arranca(vec![(200, juez_truncado())]);
    let home = tempfile::tempdir().expect("home");
    let salida = revisar_contra(&srv, home.path(), &[]);
    assert_eq!(
        codigo(&salida),
        3,
        "{}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&salida.stdout).expect("stdout es JSON");
    assert_eq!(
        json["juez"],
        "sin_veredicto: tope de salida alcanzado (32000 tokens)"
    );
    let eventos = fs::read_to_string(home.path().join("eventos.jsonl")).expect("eventos");
    let ultimo = eventos.lines().last().expect("hay un evento");
    let evento: serde_json::Value = serde_json::from_str(ultimo).expect("JSON");
    assert_eq!(evento["extra"]["truncada"], true);
    assert!(evento["cost_usd"].as_f64().expect("costo") > 0.0);
}

fn deepseek_juez_vacio() -> String {
    sse_deepseek("[]", 10, 5, "stop")
}

/// `root.toml` con el servidor falso como proveedor y un precio para `deepseek-v4-pro`.
fn home_juez(url: &str, precio: &str) -> tempfile::TempDir {
    let home = tempfile::tempdir().expect("home");
    let texto = format!(
        "[proveedores]\ndeepseek_url = \"{url}\"\nespera_base_ms = 1\n\n[precios.deepseek-v4-pro]\n{precio}\n"
    );
    fs::write(home.path().join("root.toml"), texto).expect("root.toml");
    home
}

/// `r00t ficha revisar` con el juez de DeepSeek y `ROOT_HOME` de `home`.
fn revisar_con_juez_cli(home: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_r00t"))
        .env("ROOT_HOME", home)
        .env("DEEPSEEK_API_KEY", "sk-test-SECRETO-FICHA")
        .args([
            "ficha",
            "revisar",
            "--spec",
            &ruta_fixture("spec-completa.md"),
            "--juez-proveedor",
            "deepseek",
            "--juez-modelo",
            "deepseek-v4-pro",
        ])
        .output()
        .expect("no se pudo ejecutar el binario root")
}

#[test]
fn ficha_cli_juez_con_tope_lleno_no_envia_y_sale_3() {
    let srv = Servidor::arranca(vec![(200, deepseek_juez_vacio())]);
    let home = home_juez(&srv.url, "usd_entrada = 1.0\nusd_salida = 2.0");
    // 100 USD de API este mes: el tope por defecto de 100 USD ya está lleno.
    let gasto = Evento {
        ts: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("reloj")
            .as_secs_f64(),
        corrida: "previa".into(),
        tipo: TipoEvento::Llamada,
        via: Some(Via::Api),
        cost_usd: 100.0,
        ..Evento::default()
    };
    observe::registrar_en(&home.path().join("eventos.jsonl"), &gasto).expect("siembra");

    let salida = revisar_con_juez_cli(home.path());

    assert_eq!(
        codigo(&salida),
        3,
        "{}",
        String::from_utf8_lossy(&salida.stderr)
    );
    assert_eq!(
        srv.peticiones(),
        0,
        "el juez no debe enviar con el tope lleno"
    );
    let stdout = String::from_utf8_lossy(&salida.stdout);
    assert!(stdout.contains("sin_veredicto"), "{stdout}");
    assert!(stdout.contains("margen"), "{stdout}");
}

#[test]
fn ficha_cli_juez_dentro_del_margen_envia_una_vez() {
    let srv = Servidor::arranca(vec![(200, deepseek_juez_vacio())]);
    let home = home_juez(&srv.url, "usd_entrada = 1.0\nusd_salida = 2.0");

    let salida = revisar_con_juez_cli(home.path());

    assert_eq!(
        codigo(&salida),
        0,
        "{}",
        String::from_utf8_lossy(&salida.stderr)
    );
    assert_eq!(srv.peticiones(), 1);
}

#[test]
fn ficha_cli_juez_con_root_toml_invalido_sale_2_sin_pedidos() {
    let srv = Servidor::arranca(vec![(200, deepseek_juez_vacio())]);
    let home = home_juez(&srv.url, "usd_entrada = 1.0\nusd_salda = 2.0");

    let salida = revisar_con_juez_cli(home.path());

    assert_eq!(
        codigo(&salida),
        2,
        "{}",
        String::from_utf8_lossy(&salida.stderr)
    );
    assert_eq!(srv.peticiones(), 0);
}

// --- Tarea 30, parte 1: la revisión del juez se guarda en ROOT_HOME/fichas ---

/// Sha256 en hexadecimal de un texto, calculado aparte de `ficha` para comparar.
fn sha_aparte(texto: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(texto.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Revisión con una ambigüedad de la frase real (línea 38 de `spec-completa.md`), como la deja el juez.
fn revision_con_ambiguedad() -> ficha::Revision {
    ficha::Revision {
        hallazgos: Vec::new(),
        ambiguedades: vec![ficha::Ambiguedad {
            frase: FRASE_REAL.into(),
            linea: 38,
            lectura_a: "lock por fragmento de clave".into(),
            lectura_b: "lock por fragmento de nodo".into(),
        }],
        juez: "ok".into(),
        descartadas: Vec::new(),
    }
}

#[test]
fn ficha_guardar_revision_escribe_el_json_en_home_fichas_con_el_sha_del_spec() {
    let home = tempfile::tempdir().expect("home");
    let spec = fixture("spec-completa.md");
    let revision = revision_con_ambiguedad();

    let ruta = ficha::guardar_revision(home.path(), &spec, &revision).expect("guarda");

    let esperada = home
        .path()
        .join("fichas")
        .join(format!("{}.json", sha_aparte(&spec)));
    assert_eq!(ruta, esperada);
    let leida: ficha::Revision =
        serde_json::from_str(&fs::read_to_string(&ruta).expect("lee")).expect("JSON de Revision");
    assert_eq!(leida, revision);
    let entradas = fs::read_dir(home.path().join("fichas"))
        .expect("carpeta fichas")
        .count();
    assert_eq!(entradas, 1, "solo el JSON, sin temporales");
}

#[test]
fn ficha_guardar_revision_no_escribe_en_la_carpeta_del_spec() {
    let home = tempfile::tempdir().expect("home");
    let repo = tempfile::tempdir().expect("repo");
    let spec = fixture("spec-completa.md");
    fs::write(repo.path().join("spec.md"), &spec).expect("spec en el repo");

    ficha::guardar_revision(home.path(), &spec, &revision_con_ambiguedad()).expect("guarda");

    let en_repo: Vec<String> = fs::read_dir(repo.path())
        .expect("repo")
        .map(|e| {
            e.expect("entrada")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(
        en_repo,
        vec!["spec.md".to_string()],
        "nada nuevo en el repo"
    );
}

#[test]
fn ficha_guardar_si_hay_juicio_sin_juez_no_guarda_nada() {
    let home = tempfile::tempdir().expect("home");
    let spec = fixture("spec-completa.md");
    let revision = ficha::revisar(&spec, &ctx_vacio()).expect("revisa sin juez");

    let guardada = ficha::guardar_si_hay_juicio(home.path(), &spec, &revision).expect("no falla");

    assert_eq!(guardada, None);
    assert!(
        !home.path().join("fichas").exists(),
        "no se crea la carpeta"
    );
}

#[test]
fn ficha_guardar_si_hay_juicio_con_juez_sin_veredicto_no_guarda_nada() {
    let home = tempfile::tempdir().expect("home");
    let spec = fixture("spec-completa.md");
    let juez = JuezFalso::con_error("el proveedor no responde");
    let revision = revisar_con_juez(&spec, &ctx_vacio(), &juez).expect("revisa");
    assert!(
        revision.juez.starts_with("sin_veredicto"),
        "{}",
        revision.juez
    );

    let guardada = ficha::guardar_si_hay_juicio(home.path(), &spec, &revision).expect("no falla");

    assert_eq!(guardada, None);
    assert!(!home.path().join("fichas").exists());
}

#[test]
fn ficha_guardar_si_hay_juicio_con_veredicto_guarda_la_revision() {
    let home = tempfile::tempdir().expect("home");
    let spec = fixture("spec-completa.md");
    let juez = JuezFalso::ok(&ambiguedad_real());
    let revision = revisar_con_juez(&spec, &ctx_vacio(), &juez).expect("revisa");

    let guardada = ficha::guardar_si_hay_juicio(home.path(), &spec, &revision).expect("guarda");

    let ruta = guardada.expect("con veredicto se guarda");
    assert!(ruta.is_file(), "{}", ruta.display());
}

#[test]
fn ficha_ambiguedades_guardadas_sin_archivo_es_none() {
    let home = tempfile::tempdir().expect("home");
    let spec = fixture("spec-completa.md");
    assert_eq!(
        ficha::ambiguedades_guardadas(home.path(), &spec).expect("no es error"),
        None
    );
}

#[test]
fn ficha_ambiguedades_guardadas_devuelve_las_de_la_revision_del_mismo_spec() {
    let home = tempfile::tempdir().expect("home");
    let spec = fixture("spec-completa.md");
    ficha::guardar_revision(home.path(), &spec, &revision_con_ambiguedad()).expect("guarda");

    let leidas = ficha::ambiguedades_guardadas(home.path(), &spec)
        .expect("lee")
        .expect("hay revisión");

    assert_eq!(leidas.len(), 1);
    assert_eq!(leidas[0].frase, FRASE_REAL);
    assert_eq!(leidas[0].linea, 38);
}

#[test]
fn ficha_ambiguedades_guardadas_con_spec_cambiado_es_none() {
    let home = tempfile::tempdir().expect("home");
    let spec = fixture("spec-completa.md");
    ficha::guardar_revision(home.path(), &spec, &revision_con_ambiguedad()).expect("guarda");
    let cambiado = format!("{spec}\n");

    let leidas = ficha::ambiguedades_guardadas(home.path(), &cambiado).expect("no es error");

    assert_eq!(leidas, None, "otro texto, otro hash: no hay revisión");
}

#[test]
fn ficha_ambiguedades_guardadas_json_roto_es_error_con_causa() {
    let home = tempfile::tempdir().expect("home");
    let spec = fixture("spec-completa.md");
    let carpeta = home.path().join("fichas");
    fs::create_dir_all(&carpeta).expect("carpeta");
    fs::write(
        carpeta.join(format!("{}.json", sha_aparte(&spec))),
        "{no es json",
    )
    .expect("escribe");

    let causa = ficha::ambiguedades_guardadas(home.path(), &spec).expect_err("JSON roto");

    assert!(!causa.is_empty());
}

#[test]
fn ficha_cli_juez_con_veredicto_guarda_la_revision_en_root_home() {
    let srv = Servidor::arranca(vec![(200, deepseek_juez_vacio())]);
    let home = home_juez(&srv.url, "usd_entrada = 1.0\nusd_salida = 2.0");

    let salida = revisar_con_juez_cli(home.path());

    assert_eq!(
        codigo(&salida),
        0,
        "{}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let ruta = home
        .path()
        .join("fichas")
        .join(format!("{}.json", sha_aparte(&fixture("spec-completa.md"))));
    assert!(ruta.is_file(), "{}", ruta.display());
}

#[test]
fn ficha_cli_sin_juez_no_guarda_revision() {
    let home = tempfile::tempdir().expect("home");

    let salida = Command::new(env!("CARGO_BIN_EXE_r00t"))
        .env("ROOT_HOME", home.path())
        .args([
            "ficha",
            "revisar",
            "--spec",
            &ruta_fixture("spec-completa.md"),
        ])
        .output()
        .expect("no se pudo ejecutar el binario root");

    assert_eq!(
        codigo(&salida),
        3,
        "{}",
        String::from_utf8_lossy(&salida.stderr)
    );
    assert!(!home.path().join("fichas").exists());
}
