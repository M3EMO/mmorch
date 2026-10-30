use std::fs;
use std::path::{Path, PathBuf};

use root::importar::{ErrorImportar, importar};
use root::tipos::{Evento, TipoEvento, Via};
use sha2::{Digest, Sha256};

/// Log real de mmorch. Solo se lee; el test nunca escribe sobre él.
const METRICS_REALES: &str = "/home/mpera/.claude/orchestration/logs/metrics.jsonl";

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mmorch-metrics-muestra.jsonl")
}

fn sha256(ruta: &Path) -> Vec<u8> {
    Sha256::digest(fs::read(ruta).expect("lee el archivo")).to_vec()
}

fn eventos_de(home: &Path) -> Vec<Evento> {
    fs::read_to_string(home.join("importado/mmorch-eventos.jsonl"))
        .expect("existe el archivo de eventos")
        .lines()
        .map(|linea| serde_json::from_str(linea).expect("cada línea es un Evento"))
        .collect()
}

#[test]
fn importar_fixture_da_tres_eventos_con_origen_mmorch() {
    let home = tempfile::tempdir().expect("temp dir");
    let resumen = importar(&fixture(), home.path()).expect("importa");
    assert_eq!(resumen.eventos, 3);

    let eventos = eventos_de(home.path());
    assert_eq!(eventos.len(), 3);
    for ev in &eventos {
        assert_eq!(ev.extra["origen"], "mmorch");
        assert_eq!(ev.corrida, "mmorch-importado");
        assert_eq!(ev.tipo, TipoEvento::Llamada);
        assert_eq!(ev.via, Some(Via::Api));
    }
    // La primera línea del fixture es un error de codificación con su `extra` original.
    assert_eq!(eventos[0].extra["error"], "UnicodeEncodeError");
    assert_eq!(eventos[0].extra["pattern"], "raw");
    assert_eq!(eventos[0].modelo.as_deref(), Some("deepseek-chat"));
    // La tercera línea del fixture es de la fase `memory`, con `node` y `pattern`.
    assert_eq!(eventos[2].fase, "memory");
    assert_eq!(eventos[2].extra["node"], "distiller");
    assert_eq!(eventos[2].extra["pattern"], "distill");
}

#[test]
fn importar_deja_el_original_intacto() {
    let home = tempfile::tempdir().expect("temp dir");
    let desde = fixture();
    let antes = sha256(&desde);
    importar(&desde, home.path()).expect("importa");
    assert_eq!(sha256(&desde), antes);
}

#[test]
fn importar_copia_el_original_en_importado() {
    let home = tempfile::tempdir().expect("temp dir");
    importar(&fixture(), home.path()).expect("importa");
    assert_eq!(
        sha256(&home.path().join("importado/mmorch-metrics.jsonl")),
        sha256(&fixture())
    );
}

#[test]
fn importar_dos_veces_no_duplica_eventos() {
    let home = tempfile::tempdir().expect("temp dir");
    importar(&fixture(), home.path()).expect("primera corrida");
    let segunda = importar(&fixture(), home.path()).expect("segunda corrida");
    assert_eq!(segunda.eventos, 3);
    assert_eq!(eventos_de(home.path()).len(), 3);
}

#[test]
fn importar_linea_invalida_da_error_con_numero() {
    let dir = tempfile::tempdir().expect("temp dir");
    let desde = dir.path().join("metrics.jsonl");
    let valida = fs::read_to_string(fixture()).expect("lee el fixture");
    fs::write(&desde, format!("{valida}no es json\n")).expect("escribe");
    let home = tempfile::tempdir().expect("temp dir");
    match importar(&desde, home.path()) {
        Err(ErrorImportar::Linea { linea, .. }) => assert_eq!(linea, 4),
        otro => panic!("se esperaba error de línea 4, llegó {otro:?}"),
    }
}

#[test]
fn importar_no_escribe_sobre_su_propia_salida() {
    let home = tempfile::tempdir().expect("temp dir");
    importar(&fixture(), home.path()).expect("primera corrida");
    let salida = home.path().join("importado/mmorch-eventos.jsonl");
    let antes = sha256(&salida);
    match importar(&salida, home.path()) {
        Err(ErrorImportar::MismoArchivo(_)) => {}
        otro => panic!("se esperaba MismoArchivo, llegó {otro:?}"),
    }
    assert_eq!(sha256(&salida), antes);
}

#[test]
#[ignore = "lee el log vivo de metrics del usuario, que crece: 575 era el conteo del 2026-10 temprano y hoy da 592; fijar el conteo o usar un snapshot es decisión del planificador"]
fn importar_metrics_reales_da_575_eventos_sin_tocar_el_original() {
    let real = Path::new(METRICS_REALES);
    if !real.exists() {
        eprintln!("aviso: no existe {METRICS_REALES}; se omite el test del log real");
        return;
    }
    let home = tempfile::tempdir().expect("temp dir");
    let antes = sha256(real);
    let resumen = importar(real, home.path()).expect("importa el log real");
    assert_eq!(resumen.eventos, 575);
    assert_eq!(sha256(real), antes);
}
