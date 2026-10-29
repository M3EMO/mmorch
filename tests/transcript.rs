//! Tests de la lectura del transcript de `stream-json` (Tarea 26, capa 2).
//!
//! Los transcripts son plantillas de `tests/fixtures/transcripts/`. `{HOME}`, `{ROOT_HOME}`, `{CWD}`
//! y `{HUELLA}` se reemplazan por rutas de un tmpdir y por una línea del oráculo oculto.

use std::fs;
use std::path::{Path, PathBuf};

use root::observe;
use root::observe::transcript::{self, HuellasOraculo};
use root::tipos::{Evento, TipoEvento};
use tempfile::TempDir;

const TAREA: &str = "x";
/// Línea larga y única del oráculo oculto. Aparece tal cual en `ORACULO_OCULTO`.
const HUELLA: &str = r#"assert sincronizar(tmp, "clave-secreta-42") == [1, 2, 3]"#;
const ORACULO_OCULTO: &str = "import pytest\nfrom memora import sincronizar\n\n\
def test_basico():\n    assert sincronizar([]) == 0\n\n\
def test_sincroniza_con_el_indice_invertido(tmp):\n    assert sincronizar(tmp, \"clave-secreta-42\") == [1, 2, 3]\n";
const ORACULO_CONTRATO: &str = "def test_basico():\n    assert True\n\n\
def test_contrato_propio():\n    assert True\n";

struct Entorno {
    _tmp: TempDir,
    /// Carpeta de trabajo de los transcripts: escribe aquí las plantillas ya llenas.
    trabajo: PathBuf,
    home: PathBuf,
    root: PathBuf,
    cwd: PathBuf,
}

/// Arma `HOME/.local/share/root/oraculos/x/{oculto,contrato}` y `HOME/runs/c1` como cwd.
fn preparar() -> Entorno {
    let tmp = TempDir::new().expect("temp dir");
    let base = tmp.path().canonicalize().expect("canónica");
    let home = base.join("home");
    let root = home.join(".local/share/root");
    let cwd = home.join("runs/c1");
    let trabajo = base.join("trabajo");
    fs::create_dir_all(&cwd).expect("cwd");
    fs::create_dir_all(&trabajo).expect("trabajo");
    fs::create_dir_all(root.join("oraculos/x/oculto")).expect("oculto");
    fs::create_dir_all(root.join("oraculos/x/contrato")).expect("contrato");
    fs::write(root.join("oraculos/x/oculto/t.py"), ORACULO_OCULTO).expect("oculto");
    fs::write(
        root.join("oraculos/x/contrato/test_contrato.py"),
        ORACULO_CONTRATO,
    )
    .expect("contrato");
    Entorno {
        _tmp: tmp,
        trabajo,
        home,
        root,
        cwd,
    }
}

/// Escapa un valor para ponerlo dentro de un string JSON.
fn escapar(texto: &str) -> String {
    let json = serde_json::to_string(texto).expect("serializa");
    json[1..json.len() - 1].to_string()
}

/// Llena la plantilla `nombre` y la escribe en la carpeta de trabajo.
fn plantilla(e: &Entorno, nombre: &str) -> PathBuf {
    let origen = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/transcripts")
        .join(nombre);
    let texto = fs::read_to_string(&origen).expect("lee la plantilla");
    let lleno = texto
        .replace("{HOME}", &escapar(&e.home.to_string_lossy()))
        .replace("{ROOT_HOME}", &escapar(&e.root.to_string_lossy()))
        .replace("{CWD}", &escapar(&e.cwd.to_string_lossy()))
        .replace("{HUELLA}", &escapar(HUELLA));
    let ruta = e.trabajo.join(nombre);
    fs::write(&ruta, lleno).expect("escribe el transcript");
    ruta
}

/// Escribe un transcript con las líneas dadas, sin plantilla.
fn escrito(e: &Entorno, lineas: &[String]) -> PathBuf {
    let ruta = e.trabajo.join("inline.jsonl");
    fs::write(&ruta, lineas.join("\n")).expect("escribe el transcript");
    ruta
}

fn init(e: &Entorno) -> String {
    format!(
        r#"{{"type":"system","subtype":"init","cwd":"{}"}}"#,
        escapar(&e.cwd.to_string_lossy())
    )
}

fn llamada_bash(id: &str, comando: &str) -> String {
    format!(
        r#"{{"type":"assistant","message":{{"content":[{{"type":"tool_use","id":"{id}","name":"Bash","input":{{"command":"{}"}}}}]}}}}"#,
        escapar(comando)
    )
}

fn huellas(e: &Entorno) -> HuellasOraculo {
    HuellasOraculo::armar(&e.root, Some(&e.home), TAREA).expect("arma las huellas")
}

fn revisar(e: &Entorno, transcript_path: &Path) -> Vec<Evento> {
    transcript::revisar_transcript(transcript_path, "c1", &huellas(e)).expect("revisa")
}

fn motivos(eventos: &[Evento]) -> Vec<String> {
    eventos
        .iter()
        .map(|ev| ev.extra["motivo"].as_str().expect("motivo").to_string())
        .collect()
}

#[test]
fn transcript_ruta_directa_a_oculto_emite_oraculo_leido() {
    let e = preparar();
    let ruta = plantilla(&e, "ruta-directa.jsonl");
    let eventos = revisar(&e, &ruta);

    assert!(!eventos.is_empty(), "debe haber hallazgos");
    assert!(eventos.iter().all(|ev| ev.tipo == TipoEvento::OraculoLeido));
    let ruta_ev = eventos
        .iter()
        .find(|ev| ev.extra["motivo"] == "ruta")
        .expect("un hallazgo por ruta");
    assert_eq!(ruta_ev.corrida, "c1");
    assert_eq!(ruta_ev.fase, "transcript");
    assert_eq!(ruta_ev.extra["herramienta"], "Bash");
    assert_eq!(ruta_ev.extra["linea"], 3);
    assert!(
        ruta_ev.extra["cita"]
            .as_str()
            .expect("cita")
            .contains("oraculos/x/oculto/t.py"),
        "{:?}",
        ruta_ev.extra
    );
}

#[test]
fn transcript_huella_en_salida_de_grep_sin_nombrar_ruta_emite() {
    let e = preparar();
    let ruta = plantilla(&e, "grep-huella.jsonl");
    let eventos = revisar(&e, &ruta);

    assert_eq!(motivos(&eventos), vec!["huella".to_string()]);
    assert_eq!(eventos[0].extra["herramienta"], "Grep");
    assert_eq!(eventos[0].extra["cita"], HUELLA);
}

#[test]
fn transcript_limpio_no_emite_nada() {
    let e = preparar();
    let ruta = plantilla(&e, "limpio.jsonl");
    let revision =
        transcript::revisar_transcript_con_avisos(&ruta, "c1", &huellas(&e)).expect("revisa");
    assert!(revision.eventos.is_empty(), "{:?}", revision.eventos);
    assert!(revision.avisos.is_empty());
}

#[test]
fn transcript_read_con_ruta_absoluta_en_oraculos_emite() {
    let e = preparar();
    let ruta = plantilla(&e, "read-absoluto.jsonl");
    let eventos = revisar(&e, &ruta);

    let ruta_ev = eventos
        .iter()
        .find(|ev| ev.extra["motivo"] == "ruta")
        .expect("un hallazgo por ruta");
    assert_eq!(ruta_ev.extra["herramienta"], "Read");
}

#[test]
fn transcript_enlace_en_cwd_hacia_oraculos_emite() {
    let e = preparar();
    std::os::unix::fs::symlink(e.root.join("oraculos"), e.cwd.join("enlace"))
        .expect("crea el enlace");
    let ruta = plantilla(&e, "enlace.jsonl");
    let eventos = revisar(&e, &ruta);

    // `enlace/t.py` no nombra la ruta: solo se llega a ella siguiendo el enlace del cwd.
    assert!(
        motivos(&eventos).iter().any(|m| m == "ruta"),
        "{:?}",
        eventos
    );
}

#[test]
fn transcript_linea_no_json_se_salta_con_aviso_y_no_corta() {
    let e = preparar();
    let ruta = plantilla(&e, "linea-no-json.jsonl");

    let leido = transcript::leer_transcript(&ruta).expect("lee");
    assert_eq!(leido.lineas_saltadas, vec![2]);
    assert_eq!(
        leido.llamadas.len(),
        1,
        "la revisión sigue después de la línea mala"
    );

    let revision =
        transcript::revisar_transcript_con_avisos(&ruta, "c1", &huellas(&e)).expect("revisa");
    assert_eq!(revision.avisos.len(), 1);
    assert!(revision.avisos[0].contains('2'), "{:?}", revision.avisos);
    assert!(motivos(&revision.eventos).iter().any(|m| m == "ruta"));
}

#[test]
fn transcript_nombre_oculto_solo_en_el_oculto_emite() {
    let e = preparar();
    let ruta = escrito(
        &e,
        &[
            init(&e),
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"g1","name":"Grep","input":{"pattern":"def test_sincroniza_con_el_indice_invertido"}}]}}"#.to_string(),
            r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"g1","content":"sin coincidencias"}]}}"#.to_string(),
        ],
    );
    let eventos = revisar(&e, &ruta);
    assert_eq!(motivos(&eventos), vec!["nombre".to_string()]);
    assert_eq!(
        eventos[0].extra["cita"],
        "test_sincroniza_con_el_indice_invertido"
    );
}

#[test]
fn transcript_nombre_compartido_con_contrato_no_emite() {
    let e = preparar();
    let ruta = escrito(
        &e,
        &[
            init(&e),
            llamada_bash("b1", "pytest -q -k test_basico test_contrato_propio"),
            r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"b1","content":"2 passed"}]}}"#.to_string(),
        ],
    );
    assert!(revisar(&e, &ruta).is_empty());
}

#[test]
fn transcript_cd_previo_en_el_mismo_comando_cuenta() {
    let e = preparar();
    let comando = format!(
        "cd {} && cat t.py",
        e.root.join("oraculos/x/oculto").to_string_lossy()
    );
    let ruta = escrito(&e, &[init(&e), llamada_bash("b1", &comando)]);
    let eventos = revisar(&e, &ruta);
    assert!(motivos(&eventos).iter().any(|m| m == "ruta"), "{eventos:?}");
}

#[test]
fn transcript_oraculos_enlazado_desde_root_home_cuenta_por_su_ruta_real() {
    let e = preparar();
    // ROOT_HOME/oraculos pasa a ser un enlace a otra carpeta. La ruta real es la de destino.
    let fuera = e.home.join("fuera");
    fs::create_dir_all(&fuera).expect("fuera");
    fs::rename(e.root.join("oraculos"), fuera.join("oraculos")).expect("mueve");
    std::os::unix::fs::symlink(fuera.join("oraculos"), e.root.join("oraculos")).expect("enlace");

    let real = fuera.join("oraculos/x/oculto/t.py");
    let ruta = escrito(
        &e,
        &[
            init(&e),
            format!(
                r#"{{"type":"assistant","message":{{"content":[{{"type":"tool_use","id":"r1","name":"Read","input":{{"file_path":"{}"}}}}]}}}}"#,
                escapar(&real.to_string_lossy())
            ),
        ],
    );
    let eventos = revisar(&e, &ruta);
    assert!(motivos(&eventos).iter().any(|m| m == "ruta"), "{eventos:?}");
}

#[test]
fn transcript_registrar_lecturas_escribe_y_marca_la_corrida_invalida() {
    let e = preparar();
    let eventos_jsonl = e.trabajo.join("eventos.jsonl");

    let sucia = plantilla(&e, "ruta-directa.jsonl");
    let revision = transcript::registrar_lecturas(&sucia, "c1", &huellas(&e), &eventos_jsonl)
        .expect("registra");
    assert!(!revision.eventos.is_empty());
    let resumen = observe::resumir_en(&eventos_jsonl, "c1").expect("resume");
    assert!(
        resumen.invalida,
        "una lectura del oráculo invalida la corrida"
    );

    let limpia = plantilla(&e, "limpio.jsonl");
    transcript::registrar_lecturas(&limpia, "c2", &huellas(&e), &eventos_jsonl).expect("registra");
    let resumen = observe::resumir_en(&eventos_jsonl, "c2").expect("resume");
    assert!(!resumen.invalida);
}

#[test]
fn transcript_archivo_inexistente_es_error() {
    let e = preparar();
    let ruta = e.trabajo.join("no-existe.jsonl");
    assert!(transcript::leer_transcript(&ruta).is_err());
    assert!(transcript::revisar_transcript(&ruta, "c1", &huellas(&e)).is_err());
}

#[test]
fn transcript_oculto_inexistente_falla_cerrado() {
    let e = preparar();
    assert!(HuellasOraculo::armar(&e.root, Some(&e.home), "no-existe").is_err());
}
