use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Mutex, MutexGuard, Once};
use std::time::Duration;

use root::oracle::tarea::Tarea;
use root::oracle::{ErrorOraculo, OpcionesOraculo, evaluar};
use root::tipos::{Conteo, Estado, Veredicto};

const IMAGEN: &str = "docker.io/library/alpine:3.20";
const LIMITE: Duration = Duration::from_secs(120);

/// Los tests con podman corren de a uno, como en `tests/contenedor.rs`.
static SERIE: Mutex<()> = Mutex::new(());
static IMAGEN_LISTA: Once = Once::new();

fn preparar() -> MutexGuard<'static, ()> {
    let guarda = SERIE.lock().unwrap_or_else(|e| e.into_inner());
    IMAGEN_LISTA.call_once(asegurar_imagen);
    guarda
}

fn asegurar_imagen() {
    let existe = Command::new("podman")
        .args(["image", "exists", IMAGEN])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if existe {
        return;
    }
    let pull = Command::new("podman")
        .args(["pull", IMAGEN])
        .output()
        .expect("no se pudo ejecutar podman pull");
    assert!(
        pull.status.success(),
        "no se pudo bajar la imagen {IMAGEN}: {}",
        String::from_utf8_lossy(&pull.stderr)
    );
}

const JUNIT_DOS_OK: &str = r#"<testsuite><testcase classname="tests.test_a" name="t1"/><testcase classname="tests.test_a" name="t2"/></testsuite>"#;
const JUNIT_UNA_FALLA: &str = r#"<testsuite><testcase classname="tests.test_o" name="o1"/><testcase classname="tests.test_o" name="o2"><failure message="falla"/></testcase></testsuite>"#;

/// Comando `sh` que escribe un junit a mano en `/salida/<xml>`.
fn escribe_xml(xml: &str, junit: &str) -> String {
    format!("cat > /salida/{xml} <<FIN\n{junit}\nFIN")
}

/// Escribe un `tarea.toml` con dos comandos. Incluye secciones de etapas futuras.
fn escribir_tarea(dir: &Path, contrato: &str, oculto: &str) {
    escribir_tarea_extra(
        dir,
        contrato,
        oculto,
        "",
        "[[requisito]]\nid = \"R1\"\ntitulo = \"ignorado\"\n",
    );
}

/// `raiz` son claves de la raíz (van tras `spec`); `tablas` van al final del archivo.
fn escribir_tarea_extra(dir: &Path, contrato: &str, oculto: &str, raiz: &str, tablas: &str) {
    let texto = format!(
        r#"nombre = "tarea-prueba"
lenguaje = "rust"
imagen = "{IMAGEN}"
spec = "docs/SPEC.md"
{raiz}

[contrato]
cmd = '''{contrato}'''

[oculto]
cmd = '''{oculto}'''

{tablas}
"#
    );
    fs::write(dir.join("tarea.toml"), texto).expect("escribe tarea.toml");
}

fn tarea_con(contrato: &str, oculto: &str) -> (tempfile::TempDir, Tarea) {
    tarea_con_extra(contrato, oculto, "", "")
}

fn tarea_con_extra(
    contrato: &str,
    oculto: &str,
    raiz: &str,
    tablas: &str,
) -> (tempfile::TempDir, Tarea) {
    let dir = tempfile::tempdir().expect("temp dir");
    escribir_tarea_extra(dir.path(), contrato, oculto, raiz, tablas);
    let tarea = Tarea::cargar_de(dir.path()).expect("carga la tarea");
    (dir, tarea)
}

/// Oculto que alterna: cuenta ejecuciones en `/salida/n`. `t_flaky` falla en las impares.
fn oculto_alterna() -> String {
    "n=$(cat /salida/n 2>/dev/null || echo 0); n=$((n+1)); echo $n > /salida/n; \
     if [ $((n % 2)) -eq 1 ]; then F='<failure message=\"x\"/>'; else F=''; fi; \
     cat > /salida/oculto.xml <<FIN
<testsuite><testcase classname=\"tests.test_f\" name=\"t_ok\"/><testcase classname=\"tests.test_f\" name=\"t_flaky\">$F</testcase></testsuite>
FIN"
        .to_string()
}

fn repo_con_contenido() -> tempfile::TempDir {
    let repo = tempfile::tempdir().expect("temp dir");
    fs::write(repo.path().join("a.txt"), "original").expect("escribe");
    fs::create_dir_all(repo.path().join("src")).expect("dir");
    fs::write(repo.path().join("src/lib.rs"), "fn x() {}").expect("escribe");
    fs::create_dir_all(repo.path().join("target")).expect("dir");
    fs::write(repo.path().join("target/x"), "build").expect("escribe");
    fs::create_dir_all(repo.path().join(".git")).expect("dir");
    fs::write(repo.path().join(".git/HEAD"), "ref").expect("escribe");
    repo
}

fn opciones() -> OpcionesOraculo {
    OpcionesOraculo { timeout: LIMITE }
}

fn listar(dir: &Path) -> Vec<PathBuf> {
    let mut salida = Vec::new();
    for entrada in fs::read_dir(dir).expect("lee dir") {
        let ruta = entrada.expect("entrada").path();
        if ruta.is_dir() {
            salida.extend(listar(&ruta));
        }
        salida.push(ruta);
    }
    salida.sort();
    salida
}

fn root_cli(root_home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_r00t"))
        .env("ROOT_HOME", root_home)
        .args(args)
        .output()
        .expect("no se pudo ejecutar el binario r00t")
}

#[test]
fn oraculo_todo_pasa_da_conteos_completos_y_sin_veredicto() {
    let _g = preparar();
    let (_dir, tarea) = tarea_con(
        &escribe_xml("contrato.xml", JUNIT_DOS_OK),
        &escribe_xml("oculto.xml", JUNIT_DOS_OK),
    );
    let repo = repo_con_contenido();
    let v: Veredicto = evaluar(repo.path(), &tarea, &opciones()).expect("evalúa");
    assert_eq!(v.contrato, Conteo { pasa: 2, total: 2 });
    assert_eq!(v.oculto, Conteo { pasa: 2, total: 2 });
    assert_eq!(v.estado, Estado::SinVeredicto);
    assert_eq!(v.tarea, "tarea-prueba");
    assert_eq!(v.global, None);
}

#[test]
fn oraculo_una_falla_en_oculto_da_rojo() {
    let _g = preparar();
    let (_dir, tarea) = tarea_con(
        &escribe_xml("contrato.xml", JUNIT_DOS_OK),
        &escribe_xml("oculto.xml", JUNIT_UNA_FALLA),
    );
    let repo = repo_con_contenido();
    let v = evaluar(repo.path(), &tarea, &opciones()).expect("evalúa");
    assert_eq!(v.oculto, Conteo { pasa: 1, total: 2 });
    assert_eq!(v.estado, Estado::Rojo);
}

#[test]
fn oraculo_comando_sin_xml_da_infra_y_no_hay_veredicto() {
    let _g = preparar();
    let (_dir, tarea) = tarea_con(&escribe_xml("contrato.xml", JUNIT_DOS_OK), "true");
    let repo = repo_con_contenido();
    let res = evaluar(repo.path(), &tarea, &opciones());
    assert!(
        matches!(res, Err(ErrorOraculo::Infra(_))),
        "se esperaba Infra sin veredicto, llegó {res:?}"
    );
}

#[test]
fn oraculo_repo_original_intacto_y_target_no_se_copia() {
    let _g = preparar();
    // El contrato escribe un junit con una entrada por cada nombre que ve en /repo.
    // El repo tiene a.txt, src, target y .git: la copia debe tener solo a.txt y src.
    let listado = "{ echo '<testsuite>'; for f in $(ls -A /repo); do \
echo \"<testcase classname=\\\"repo\\\" name=\\\"$f\\\"/>\"; done; \
echo '</testsuite>'; } > /salida/contrato.xml";
    let (_dir, tarea) = tarea_con(
        listado,
        &format!(
            "touch /repo/escrito-por-la-tarea 2>/dev/null; {}",
            escribe_xml("oculto.xml", JUNIT_DOS_OK)
        ),
    );
    let repo = repo_con_contenido();
    let antes = listar(repo.path());

    let v = evaluar(repo.path(), &tarea, &opciones()).expect("evalúa");

    assert_eq!(
        v.contrato,
        Conteo { pasa: 2, total: 2 },
        "a.txt y src, sin target ni .git"
    );
    assert_eq!(listar(repo.path()), antes, "el repo original no cambia");
    assert_eq!(
        fs::read_to_string(repo.path().join("a.txt")).expect("lee"),
        "original"
    );
}

#[test]
fn oraculo_cli_evaluar_json_imprime_veredicto() {
    let _g = preparar();
    let home = tempfile::tempdir().expect("temp dir");
    let dir = home.path().join("oraculos").join("tarea-prueba");
    fs::create_dir_all(&dir).expect("dir");
    escribir_tarea(
        &dir,
        &escribe_xml("contrato.xml", JUNIT_DOS_OK),
        &escribe_xml("oculto.xml", JUNIT_DOS_OK),
    );
    let repo = repo_con_contenido();
    let salida = root_cli(
        home.path(),
        &[
            "--json",
            "oraculo",
            "evaluar",
            "--tarea",
            "tarea-prueba",
            "--repo",
            repo.path().to_str().expect("ruta UTF-8"),
        ],
    );
    assert_eq!(salida.status.code(), Some(3), "sin veredicto sale con 3");
    let json: serde_json::Value =
        serde_json::from_slice(&salida.stdout).expect("stdout es un objeto JSON");
    assert_eq!(json["contrato"]["pasa"], 2);
    assert_eq!(json["contrato"]["total"], 2);
    assert_eq!(json["oculto"]["total"], 2);
    assert_eq!(json["estado"], "sin_veredicto");
}

#[test]
fn oraculo_cli_evaluar_sin_json_imprime_una_linea_y_sale_con_1_si_hay_rojo() {
    let _g = preparar();
    let home = tempfile::tempdir().expect("temp dir");
    let dir = home.path().join("oraculos").join("tarea-prueba");
    fs::create_dir_all(&dir).expect("dir");
    escribir_tarea(
        &dir,
        &escribe_xml("contrato.xml", JUNIT_DOS_OK),
        &escribe_xml("oculto.xml", JUNIT_UNA_FALLA),
    );
    let repo = repo_con_contenido();
    let ruta = repo.path().to_str().expect("ruta UTF-8").to_string();
    let salida = root_cli(
        home.path(),
        &[
            "oraculo",
            "evaluar",
            "--tarea",
            "tarea-prueba",
            "--repo",
            &ruta,
        ],
    );
    assert_eq!(salida.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&salida.stdout);
    assert_eq!(
        stdout.trim_end(),
        format!("{ruta}: contrato 2/2, oculto 1/2, estado rojo, repeticiones 1, inestables 0")
    );
}

#[test]
fn inestable_tres_repeticiones_marca_el_test_que_alterna() {
    let _g = preparar();
    let (_dir, tarea) = tarea_con_extra(
        &escribe_xml("contrato.xml", JUNIT_DOS_OK),
        &oculto_alterna(),
        "repeticiones = 3",
        "",
    );
    let repo = repo_con_contenido();
    let v = evaluar(repo.path(), &tarea, &opciones()).expect("evalúa");
    assert_eq!(v.repeticiones, 3);
    assert_eq!(v.inestables, vec!["test_f::t_flaky".to_string()]);
    assert_eq!(v.oculto, Conteo { pasa: 1, total: 2 });
    assert_eq!(v.estado, Estado::Rojo, "un inestable no pasa: rojo");
}

#[test]
fn exclusion_oraculo_dudoso_saca_al_test_de_los_inestables() {
    let _g = preparar();
    let (_dir, tarea) = tarea_con_extra(
        &escribe_xml("contrato.xml", JUNIT_DOS_OK),
        &oculto_alterna(),
        "repeticiones = 3",
        "[[oraculo_dudoso]]\ntest = \"test_f::t_flaky\"\nmotivo = \"la spec no fija el orden\"\n",
    );
    let repo = repo_con_contenido();
    let v = evaluar(repo.path(), &tarea, &opciones()).expect("evalúa");
    assert!(v.inestables.is_empty());
    assert_eq!(v.oculto, Conteo { pasa: 1, total: 1 });
    assert_eq!(v.estado, Estado::SinVeredicto);
}

#[test]
fn repeticiones_sin_xml_en_una_repeticion_es_infra() {
    let _g = preparar();
    // La segunda ejecución no escribe XML. Si el archivo viejo sobreviviera, pasaría sin error.
    let oculto = format!(
        "n=$(cat /salida/n 2>/dev/null || echo 0); n=$((n+1)); echo $n > /salida/n; \
         if [ $n -ne 2 ]; then {}; fi",
        escribe_xml("oculto.xml", JUNIT_DOS_OK)
    );
    let (_dir, tarea) = tarea_con_extra(
        &escribe_xml("contrato.xml", JUNIT_DOS_OK),
        &oculto,
        "repeticiones = 3",
        "",
    );
    let repo = repo_con_contenido();
    let res = evaluar(repo.path(), &tarea, &opciones());
    assert!(
        matches!(res, Err(ErrorOraculo::Infra(_))),
        "una repetición sin XML es Infra, llegó {res:?}"
    );
}

#[test]
fn repeticiones_cli_json_imprime_repeticiones_e_inestables() {
    let _g = preparar();
    let home = tempfile::tempdir().expect("temp dir");
    let dir = home.path().join("oraculos").join("tarea-prueba");
    fs::create_dir_all(&dir).expect("dir");
    escribir_tarea_extra(
        &dir,
        &escribe_xml("contrato.xml", JUNIT_DOS_OK),
        &oculto_alterna(),
        "repeticiones = 3",
        "",
    );
    let repo = repo_con_contenido();
    let salida = root_cli(
        home.path(),
        &[
            "--json",
            "oraculo",
            "evaluar",
            "--tarea",
            "tarea-prueba",
            "--repo",
            repo.path().to_str().expect("ruta UTF-8"),
        ],
    );
    assert_eq!(salida.status.code(), Some(1), "inestable deja rojo");
    let json: serde_json::Value =
        serde_json::from_slice(&salida.stdout).expect("stdout es un objeto JSON");
    assert_eq!(json["repeticiones"], 3);
    assert_eq!(json["inestables"], serde_json::json!(["test_f::t_flaky"]));
}
