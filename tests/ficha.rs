use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

use root::ficha::{self, ContextoRevision, ErrorFicha};
use root::oracle::tarea::Tarea;

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
    assert_eq!(revision.juez, "no corrido");
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
fn ficha_cli_revisar_completa_sale_0() {
    let salida = root(&[
        "ficha",
        "revisar",
        "--spec",
        &ruta_fixture("spec-completa.md"),
    ]);
    assert_eq!(
        codigo(&salida),
        0,
        "{}",
        String::from_utf8_lossy(&salida.stderr)
    );
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
