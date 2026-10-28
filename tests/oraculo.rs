use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Mutex, MutexGuard, Once};
use std::time::{Duration, SystemTime};

use root::oracle::tarea::Tarea;
use root::oracle::{ErrorOraculo, OpcionesOraculo, evaluar};
use root::tipos::{Conteo, Estado, Veredicto};
use root::track::stats::wilson_inferior;

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

/// Comando `sh` que alterna: cuenta sus corridas en `/salida/<contador>`.
/// `t_flaky` falla en las impares, así que su muestra queda mezclada.
fn alterna(contador: &str, xml: &str) -> String {
    format!(
        "n=$(cat /salida/{contador} 2>/dev/null || echo 0); n=$((n+1)); echo $n > /salida/{contador}; \
         if [ $((n % 2)) -eq 1 ]; then F='<failure message=\"x\"/>'; else F=''; fi; \
         cat > /salida/{xml} <<FIN\n<testsuite><testcase classname=\"tests.test_f\" name=\"t_ok\"/><testcase classname=\"tests.test_f\" name=\"t_flaky\">$F</testcase></testsuite>\nFIN"
    )
}

/// Comando `sh` que cuenta sus corridas en `/salida/<contador>`. Falla `t_cuenta` desde la corrida 6.
/// Si el oráculo corriera más de 5 veces un test estable, ese test quedaría inestable.
fn cuenta(contador: &str, xml: &str) -> String {
    format!(
        "n=$(cat /salida/{contador} 2>/dev/null || echo 0); n=$((n+1)); echo $n > /salida/{contador}; \
         if [ $n -gt 5 ]; then F='<failure message=\"x\"/>'; else F=''; fi; \
         cat > /salida/{xml} <<FIN\n<testsuite><testcase classname=\"tests.test_c\" name=\"t_ok\"/><testcase classname=\"tests.test_c\" name=\"t_cuenta\">$F</testcase></testsuite>\nFIN"
    )
}

/// Bloque TOML de una suite: su `cmd` y, si hay, su `reintento`.
fn bloque(cmd: &str, reintento: Option<&str>) -> String {
    match reintento {
        Some(r) => format!("cmd = '''{cmd}'''\nreintento = '''{r}'''\n"),
        None => format!("cmd = '''{cmd}'''\n"),
    }
}

/// Escribe un `tarea.toml` con dos suites ya armadas. `raiz` son claves de la raíz (van tras `spec`);
/// `tablas` van al final del archivo.
fn escribir_tarea_extra(dir: &Path, contrato: &str, oculto: &str, raiz: &str, tablas: &str) {
    let texto = format!(
        r#"nombre = "tarea-prueba"
lenguaje = "shell"
imagen = "{IMAGEN}"
spec = "docs/SPEC.md"
{raiz}

[contrato]
{contrato}
[oculto]
{oculto}

{tablas}
"#
    );
    fs::write(dir.join("tarea.toml"), texto).expect("escribe tarea.toml");
    // `evaluar` exige una ficha válida en el spec de la tarea (Tarea 9f).
    fs::create_dir_all(dir.join("docs")).expect("dir docs");
    fs::write(dir.join("docs/SPEC.md"), SPEC_CON_FICHA).expect("escribe SPEC.md");
}

/// Spec con ficha válida: el mismo fixture que usan los tests de `ficha`.
const SPEC_CON_FICHA: &str = include_str!("fixtures/fichas/spec-completa.md");

/// Escribe un `tarea.toml` con dos comandos. Incluye secciones de etapas futuras.
fn escribir_tarea(dir: &Path, contrato: &str, oculto: &str) {
    escribir_tarea_extra(
        dir,
        &bloque(contrato, None),
        &bloque(oculto, None),
        "",
        "[[requisito]]\nid = \"R1\"\ntitulo = \"ignorado\"\n",
    );
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
    escribir_tarea_extra(
        dir.path(),
        &bloque(contrato, None),
        &bloque(oculto, None),
        raiz,
        tablas,
    );
    let tarea = Tarea::cargar_de(dir.path()).expect("carga la tarea");
    (dir, tarea)
}

/// Tarea con contrato que pasa y oculto alterno, con su `reintento` igual al comando.
fn tarea_alterna(raiz: &str, tablas: &str) -> (tempfile::TempDir, Tarea) {
    let dir = tempfile::tempdir().expect("temp dir");
    let oculto = alterna("n", "oculto.xml");
    escribir_tarea_extra(
        dir.path(),
        &bloque(&escribe_xml("contrato.xml", JUNIT_DOS_OK), None),
        &bloque(&oculto, Some(&oculto)),
        raiz,
        tablas,
    );
    let tarea = Tarea::cargar_de(dir.path()).expect("carga la tarea");
    (dir, tarea)
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

/// Opciones con `home` propio: el test nunca escribe en el ROOT_HOME real.
fn opciones(home: &Path) -> OpcionesOraculo {
    OpcionesOraculo {
        timeout: LIMITE,
        home: Some(home.to_path_buf()),
    }
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

/// Nombres de las carpetas `evaluar-*` que hay directamente en `dir`.
fn temporales_en(dir: &Path) -> Vec<String> {
    let Ok(entradas) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut nombres: Vec<String> = entradas
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("evaluar-"))
        .collect();
    nombres.sort();
    nombres
}

fn poner_mtime(ruta: &Path, cuando: SystemTime) {
    fs::File::open(ruta)
        .expect("abre la carpeta")
        .set_modified(cuando)
        .expect("fija la fecha");
}

fn root_cli(root_home: &Path, args: &[&str]) -> Output {
    root_cli_env(root_home, &[], args)
}

fn root_cli_env(root_home: &Path, env: &[(&str, &str)], args: &[&str]) -> Output {
    let mut orden = Command::new(env!("CARGO_BIN_EXE_r00t"));
    orden.env("ROOT_HOME", root_home).args(args);
    for (clave, valor) in env {
        orden.env(clave, valor);
    }
    orden.output().expect("no se pudo ejecutar el binario r00t")
}

#[test]
fn oraculo_todo_pasa_da_conteos_completos_y_sin_veredicto() {
    root::saltar_sin_podman!();
    let _g = preparar();
    let (_dir, tarea) = tarea_con(
        &escribe_xml("contrato.xml", JUNIT_DOS_OK),
        &escribe_xml("oculto.xml", JUNIT_DOS_OK),
    );
    let repo = repo_con_contenido();
    let home = tempfile::tempdir().expect("home");
    let v: Veredicto = evaluar(repo.path(), &tarea, &opciones(home.path())).expect("evalúa");
    assert_eq!(v.contrato, Conteo { pasa: 2, total: 2 });
    assert_eq!(v.oculto, Conteo { pasa: 2, total: 2 });
    assert_eq!(v.estado, Estado::SinVeredicto);
    assert_eq!(v.tarea, "tarea-prueba");
    assert_eq!(v.global, None);
}

#[test]
fn oraculo_una_falla_en_oculto_da_rojo() {
    root::saltar_sin_podman!();
    let _g = preparar();
    let (_dir, tarea) = tarea_con(
        &escribe_xml("contrato.xml", JUNIT_DOS_OK),
        &escribe_xml("oculto.xml", JUNIT_UNA_FALLA),
    );
    let repo = repo_con_contenido();
    let home = tempfile::tempdir().expect("home");
    let v = evaluar(repo.path(), &tarea, &opciones(home.path())).expect("evalúa");
    assert_eq!(v.oculto, Conteo { pasa: 1, total: 2 });
    assert_eq!(v.estado, Estado::Rojo);
}

#[test]
fn oraculo_comando_sin_xml_da_infra_y_no_hay_veredicto() {
    root::saltar_sin_podman!();
    let _g = preparar();
    let (_dir, tarea) = tarea_con(&escribe_xml("contrato.xml", JUNIT_DOS_OK), "true");
    let repo = repo_con_contenido();
    let home = tempfile::tempdir().expect("home");
    let res = evaluar(repo.path(), &tarea, &opciones(home.path()));
    assert!(
        matches!(res, Err(ErrorOraculo::Infra(_))),
        "se esperaba Infra sin veredicto, llegó {res:?}"
    );
}

#[test]
fn oraculo_repo_original_intacto_y_target_no_se_copia() {
    root::saltar_sin_podman!();
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
    let home = tempfile::tempdir().expect("home");

    let v = evaluar(repo.path(), &tarea, &opciones(home.path())).expect("evalúa");

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
    root::saltar_sin_podman!();
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
    root::saltar_sin_podman!();
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
    root::saltar_sin_podman!();
    let _g = preparar();
    let (_dir, tarea) = tarea_alterna("repeticiones = 3", "");
    let repo = repo_con_contenido();
    let home = tempfile::tempdir().expect("home");
    let v = evaluar(repo.path(), &tarea, &opciones(home.path())).expect("evalúa");
    assert_eq!(v.repeticiones, 3);
    assert_eq!(v.inestables, vec!["test_f::t_flaky".to_string()]);
    assert_eq!(v.oculto, Conteo { pasa: 1, total: 2 });
    assert_eq!(v.estado, Estado::Rojo, "un inestable no pasa: rojo");
}

#[test]
fn inestable_tasa_llega_a_10_corridas_y_su_wilson_es_consistente() {
    root::saltar_sin_podman!();
    let _g = preparar();
    let (_dir, tarea) = tarea_alterna("repeticiones = 5\nrepeticiones_inestable = 10", "");
    let repo = repo_con_contenido();
    let home = tempfile::tempdir().expect("home");
    let v = evaluar(repo.path(), &tarea, &opciones(home.path())).expect("evalúa");
    assert_eq!(v.repeticiones, 5, "las corridas completas");
    assert_eq!(v.inestables, vec!["test_f::t_flaky".to_string()]);
    let tasa = &v.tasas["oculto:test_f::t_flaky"];
    assert_eq!(tasa.total, 10, "5 completas más 5 extra");
    assert_eq!(tasa.pasa, 5, "pasa en las pares");
    assert!((tasa.wilson_inf - wilson_inferior(5, 10)).abs() < 1e-12);
    assert!(
        !v.tasas.contains_key("oculto:test_f::t_ok"),
        "los estables no tienen tasa"
    );
}

#[test]
fn inestable_exclusion_oraculo_dudoso_saca_al_test_de_los_inestables() {
    root::saltar_sin_podman!();
    let _g = preparar();
    let (_dir, tarea) = tarea_alterna(
        "repeticiones = 3",
        "[[oraculo_dudoso]]\ntest = \"test_f::t_flaky\"\nmotivo = \"la spec no fija el orden\"\n",
    );
    let repo = repo_con_contenido();
    let home = tempfile::tempdir().expect("home");
    let v = evaluar(repo.path(), &tarea, &opciones(home.path())).expect("evalúa");
    assert!(v.inestables.is_empty());
    assert!(v.tasas.is_empty());
    assert_eq!(v.oculto, Conteo { pasa: 1, total: 1 });
    assert_eq!(v.estado, Estado::SinVeredicto);
}

#[test]
fn inestable_contrato_figura_y_no_completa_el_contrato() {
    root::saltar_sin_podman!();
    let _g = preparar();
    let dir = tempfile::tempdir().expect("temp dir");
    escribir_tarea_extra(
        dir.path(),
        &bloque(
            &alterna("n-c", "contrato.xml"),
            Some(&alterna("n-c", "contrato.xml")),
        ),
        &bloque(&escribe_xml("oculto.xml", JUNIT_DOS_OK), None),
        "repeticiones = 3",
        "",
    );
    let tarea = Tarea::cargar_de(dir.path()).expect("carga");
    let repo = repo_con_contenido();
    let home = tempfile::tempdir().expect("home");
    let v = evaluar(repo.path(), &tarea, &opciones(home.path())).expect("evalúa");
    assert_eq!(v.inestables_contrato, vec!["test_f::t_flaky".to_string()]);
    assert_eq!(
        v.contrato,
        Conteo { pasa: 1, total: 2 },
        "el inestable no pasa"
    );
    assert!(v.inestables.is_empty(), "el oculto no tiene inestables");
    assert!(v.tasas.contains_key("contrato:test_f::t_flaky"));
    assert_eq!(v.estado, Estado::Rojo);
}

#[test]
fn repeticiones_estable_corre_5_veces_y_no_hay_extras() {
    root::saltar_sin_podman!();
    let _g = preparar();
    let (_dir, tarea) = tarea_con_extra(
        &escribe_xml("contrato.xml", JUNIT_DOS_OK),
        &cuenta("n-oculto", "oculto.xml"),
        "repeticiones = 5",
        "",
    );
    let repo = repo_con_contenido();
    let home = tempfile::tempdir().expect("home");
    let v = evaluar(repo.path(), &tarea, &opciones(home.path())).expect("evalúa");
    assert_eq!(v.repeticiones, 5);
    assert_eq!(v.oculto, Conteo { pasa: 2, total: 2 });
    assert!(
        v.inestables.is_empty(),
        "un extra habría vuelto inestable a t_cuenta"
    );
    assert!(v.tasas.is_empty());
}

#[test]
fn repeticiones_contrato_corre_5_veces_y_no_hay_extras() {
    root::saltar_sin_podman!();
    let _g = preparar();
    let (_dir, tarea) = tarea_con_extra(
        &cuenta("n-contrato", "contrato.xml"),
        &escribe_xml("oculto.xml", JUNIT_DOS_OK),
        "repeticiones = 5",
        "",
    );
    let repo = repo_con_contenido();
    let home = tempfile::tempdir().expect("home");
    let v = evaluar(repo.path(), &tarea, &opciones(home.path())).expect("evalúa");
    assert_eq!(v.repeticiones, 5);
    assert_eq!(v.contrato, Conteo { pasa: 2, total: 2 });
    assert!(
        v.inestables_contrato.is_empty(),
        "un extra habría vuelto inestable a t_cuenta"
    );
}

#[test]
fn repeticiones_sin_xml_en_una_repeticion_es_infra() {
    root::saltar_sin_podman!();
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
    let home = tempfile::tempdir().expect("home");
    let res = evaluar(repo.path(), &tarea, &opciones(home.path()));
    assert!(
        matches!(res, Err(ErrorOraculo::Infra(_))),
        "una repetición sin XML es Infra, llegó {res:?}"
    );
}

#[test]
fn repeticiones_cli_json_imprime_repeticiones_e_inestables() {
    root::saltar_sin_podman!();
    let _g = preparar();
    let home = tempfile::tempdir().expect("temp dir");
    let dir = home.path().join("oraculos").join("tarea-prueba");
    fs::create_dir_all(&dir).expect("dir");
    let oculto = alterna("n", "oculto.xml");
    escribir_tarea_extra(
        &dir,
        &bloque(&escribe_xml("contrato.xml", JUNIT_DOS_OK), None),
        &bloque(&oculto, Some(&oculto)),
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
    assert_eq!(
        json["tasas"]["oculto:test_f::t_flaky"]["total"], 10,
        "3 completas más 7 extra"
    );
}

#[test]
fn temporales_evaluar_con_tmpdir_inexistente_usa_root_home() {
    root::saltar_sin_podman!();
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
    let ruta = repo.path().to_str().expect("ruta UTF-8").to_string();
    let antes = temporales_en(Path::new("/tmp"));
    let salida = root_cli_env(
        home.path(),
        &[("TMPDIR", "/nada")],
        &[
            "--json",
            "oraculo",
            "evaluar",
            "--tarea",
            "tarea-prueba",
            "--repo",
            &ruta,
        ],
    );
    assert_eq!(salida.status.code(), Some(3), "sin veredicto sale con 3");
    let json: serde_json::Value =
        serde_json::from_slice(&salida.stdout).expect("stdout es un objeto JSON");
    assert_eq!(json["contrato"]["total"], 2);
    assert!(home.path().join("tmp").is_dir(), "usa ROOT_HOME/tmp");
    assert!(
        temporales_en(&home.path().join("tmp")).is_empty(),
        "no queda ningún temporal"
    );
    assert_eq!(
        temporales_en(Path::new("/tmp")),
        antes,
        "/tmp no recibe temporales"
    );
}

#[test]
fn temporales_evaluar_borra_las_carpetas_viejas_de_home_tmp() {
    root::saltar_sin_podman!();
    let _g = preparar();
    let home = tempfile::tempdir().expect("home");
    let tmp = home.path().join("tmp");
    let viejo = tmp.join("evaluar-viejo");
    let reciente = tmp.join("evaluar-reciente");
    let ajeno = tmp.join("ajeno.txt");
    fs::create_dir_all(&viejo).expect("dir");
    fs::write(viejo.join("resto"), "x").expect("escribe");
    fs::create_dir_all(&reciente).expect("dir");
    fs::write(&ajeno, "x").expect("escribe");
    poner_mtime(&viejo, SystemTime::now() - Duration::from_secs(25 * 3600));

    let (_dir, tarea) = tarea_con(
        &escribe_xml("contrato.xml", JUNIT_DOS_OK),
        &escribe_xml("oculto.xml", JUNIT_DOS_OK),
    );
    let repo = repo_con_contenido();
    evaluar(repo.path(), &tarea, &opciones(home.path())).expect("evalúa");

    assert!(
        !viejo.exists(),
        "la carpeta de más de 24 h se borra al arrancar"
    );
    assert!(reciente.exists(), "la carpeta reciente se conserva");
    assert!(ajeno.exists(), "un archivo ajeno se conserva");
}

#[test]
fn temporales_home_tmp_como_archivo_da_infra() {
    let home = tempfile::tempdir().expect("home");
    fs::write(home.path().join("tmp"), "no es carpeta").expect("escribe");
    let (_dir, tarea) = tarea_con(
        &escribe_xml("contrato.xml", JUNIT_DOS_OK),
        &escribe_xml("oculto.xml", JUNIT_DOS_OK),
    );
    let repo = repo_con_contenido();
    let res = evaluar(repo.path(), &tarea, &opciones(home.path()));
    assert!(
        matches!(res, Err(ErrorOraculo::Infra(_))),
        "un temporal que no se crea es Infra, llegó {res:?}"
    );
}

// ---- Tarea 7f: tests pendientes por entorno ----

const ENTORNO_O2: &str =
    "[[entorno]]\ntest = \"test_o::o2\"\nmotivo = \"lavapipe no emula la GPU\"\n";
const JUNIT_IMPORTAR: &str = r#"<testsuite><testcase classname="tests.test_o" name="o2"/><testcase classname="tests.test_ajeno" name="otro"/></testsuite>"#;

/// Tarea con un test en `[[entorno]]` y los archivos de su suite oculta ya escritos.
fn tarea_con_pendiente(home: &Path, entorno: &str) -> PathBuf {
    let dir = home.join("oraculos").join("tarea-prueba");
    fs::create_dir_all(dir.join("oculto")).expect("dir");
    escribir_tarea_extra(
        &dir,
        &bloque(&escribe_xml("contrato.xml", JUNIT_DOS_OK), None),
        &bloque(&escribe_xml("oculto.xml", JUNIT_UNA_FALLA), None),
        "",
        entorno,
    );
    dir
}

#[test]
fn pendiente_entorno_lista_el_test_y_no_cuenta() {
    root::saltar_sin_podman!();
    let _g = preparar();
    let (_dir, tarea) = tarea_con_extra(
        &escribe_xml("contrato.xml", JUNIT_DOS_OK),
        &escribe_xml("oculto.xml", JUNIT_UNA_FALLA),
        "",
        ENTORNO_O2,
    );
    let repo = repo_con_contenido();
    let home = tempfile::tempdir().expect("home");
    let v = evaluar(repo.path(), &tarea, &opciones(home.path())).expect("evalúa");
    assert_eq!(v.oculto, Conteo { pasa: 1, total: 1 }, "o2 sale del conteo");
    assert_eq!(v.pendientes.len(), 1);
    assert_eq!(v.pendientes[0].test, "test_o::o2");
    assert_eq!(v.pendientes[0].motivo, "lavapipe no emula la GPU");
    assert_eq!(v.pendientes[0].colab, None);
    assert_eq!(v.estado, Estado::SinVeredicto);
}

#[test]
fn pendiente_entorno_sin_test_en_el_junit_tambien_se_lista() {
    root::saltar_sin_podman!();
    let _g = preparar();
    let (_dir, tarea) = tarea_con_extra(
        &escribe_xml("contrato.xml", JUNIT_DOS_OK),
        &escribe_xml("oculto.xml", JUNIT_DOS_OK),
        "",
        "[[entorno]]\ntest = \"test_z::no_corre\"\nmotivo = \"no existe en este entorno\"\n",
    );
    let repo = repo_con_contenido();
    let home = tempfile::tempdir().expect("home");
    let v = evaluar(repo.path(), &tarea, &opciones(home.path())).expect("evalúa");
    assert_eq!(v.pendientes.len(), 1);
    assert_eq!(v.oculto, Conteo { pasa: 2, total: 2 });
}

#[test]
fn pendiente_paquete_crea_repo_tests_correr_y_leeme() {
    let home = tempfile::tempdir().expect("home");
    let entorno =
        "[[entorno]]\ntest = \"test_x::test_rendimiento[gpu-10000-res1]\"\nmotivo = \"lavapipe\"\n";
    let dir = tarea_con_pendiente(home.path(), entorno);
    fs::write(
        dir.join("oculto/test_x.py"),
        "def test_rendimiento(): pass\n",
    )
    .expect("escribe");
    fs::write(dir.join("oculto/conftest.py"), "# fixtures\n").expect("escribe");
    let repo = repo_con_contenido();
    let ruta = repo.path().to_str().expect("ruta UTF-8").to_string();

    let salida = root_cli(
        home.path(),
        &[
            "--json",
            "oraculo",
            "pendientes",
            "--tarea",
            "tarea-prueba",
            "--repo",
            &ruta,
        ],
    );
    assert_eq!(salida.status.code(), Some(0));
    let json: serde_json::Value =
        serde_json::from_slice(&salida.stdout).expect("stdout es un objeto JSON");
    assert_eq!(
        json["tests"],
        serde_json::json!(["test_x::test_rendimiento[gpu-10000-res1]"])
    );
    let paquete = PathBuf::from(json["dir"].as_str().expect("dir es texto"));
    assert!(paquete.starts_with(home.path().join("pendientes")));
    assert!(
        !paquete.starts_with(env!("CARGO_MANIFEST_DIR")),
        "el paquete no va dentro del worktree"
    );
    assert!(paquete.join("repo/a.txt").is_file());
    assert!(paquete.join("repo/src/lib.rs").is_file());
    assert!(!paquete.join("repo/target").exists(), "sin target/");
    assert!(!paquete.join("repo/.git").exists(), "sin .git/");
    assert!(paquete.join("tests/test_x.py").is_file());
    assert!(paquete.join("tests/conftest.py").is_file());
    let correr = fs::read_to_string(paquete.join("correr.sh")).expect("correr.sh");
    assert!(correr.contains("'tests/test_x.py::test_rendimiento[gpu-10000-res1]'"));
    let modo = fs::metadata(paquete.join("correr.sh"))
        .expect("meta")
        .permissions();
    assert!(
        std::os::unix::fs::PermissionsExt::mode(&modo) & 0o111 != 0,
        "correr.sh es ejecutable"
    );
    assert!(paquete.join("LEEME.txt").is_file());
    assert!(
        repo.path().join("target/x").exists(),
        "el repo original no cambia"
    );
}

#[test]
fn pendiente_sin_tests_pendientes_no_crea_nada() {
    let home = tempfile::tempdir().expect("home");
    let dir = home.path().join("oraculos").join("tarea-prueba");
    fs::create_dir_all(&dir).expect("dir");
    escribir_tarea(
        &dir,
        &escribe_xml("contrato.xml", JUNIT_DOS_OK),
        &escribe_xml("oculto.xml", JUNIT_DOS_OK),
    );
    let repo = repo_con_contenido();
    let ruta = repo.path().to_str().expect("ruta UTF-8").to_string();
    let salida = root_cli(
        home.path(),
        &[
            "oraculo",
            "pendientes",
            "--tarea",
            "tarea-prueba",
            "--repo",
            &ruta,
        ],
    );
    assert_eq!(salida.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&salida.stdout).trim_end(),
        "tests: []"
    );
    assert!(!home.path().join("pendientes").exists(), "no crea nada");
}

#[test]
fn pendiente_archivo_inexistente_es_error_de_tarea() {
    let home = tempfile::tempdir().expect("home");
    tarea_con_pendiente(
        home.path(),
        "[[entorno]]\ntest = \"test_falta::t\"\nmotivo = \"x\"\n",
    );
    let repo = repo_con_contenido();
    let ruta = repo.path().to_str().expect("ruta UTF-8").to_string();
    let salida = root_cli(
        home.path(),
        &[
            "oraculo",
            "pendientes",
            "--tarea",
            "tarea-prueba",
            "--repo",
            &ruta,
        ],
    );
    assert_eq!(
        salida.status.code(),
        Some(3),
        "falta el archivo: error de tarea"
    );
    assert!(
        !home.path().join("pendientes").exists(),
        "no deja un paquete a medias"
    );
}

#[test]
fn pendiente_importar_guarda_colab_y_evento_gate() {
    root::saltar_sin_podman!();
    let _g = preparar();
    let home = tempfile::tempdir().expect("home");
    let dir = tarea_con_pendiente(home.path(), ENTORNO_O2);
    fs::write(dir.join("oculto/conftest.py"), "").expect("escribe");
    let repo = repo_con_contenido();
    let ruta = repo.path().to_str().expect("ruta UTF-8").to_string();
    let junit = home.path().join("pendientes.xml");
    fs::write(&junit, JUNIT_IMPORTAR).expect("escribe el junit");
    let ruta_junit = junit.to_str().expect("ruta UTF-8").to_string();

    let salida = root_cli(
        home.path(),
        &[
            "--json",
            "oraculo",
            "pendientes",
            "--tarea",
            "tarea-prueba",
            "--repo",
            &ruta,
            "--importar",
            &ruta_junit,
        ],
    );
    assert_eq!(salida.status.code(), Some(0));
    let json: serde_json::Value =
        serde_json::from_slice(&salida.stdout).expect("stdout es un objeto JSON");
    assert_eq!(json["importados"], 1);
    assert_eq!(json["ignorados"], 1, "test_ajeno::otro no está en entorno");

    let colab: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(home.path().join("evidencia/tarea-prueba/colab.json"))
            .expect("colab.json"),
    )
    .expect("colab.json es JSON");
    assert_eq!(colab["test_o::o2"]["resultado"], "pasa");
    assert_eq!(
        colab["test_o::o2"]["junit_sha256"].as_str().map(str::len),
        Some(64)
    );
    assert!(colab.get("test_ajeno::otro").is_none());

    let eventos = fs::read_to_string(home.path().join("eventos.jsonl")).expect("eventos.jsonl");
    let gates: Vec<serde_json::Value> = eventos
        .lines()
        .map(|l| serde_json::from_str(l).expect("línea JSON"))
        .filter(|e: &serde_json::Value| e["tipo"] == "gate")
        .collect();
    assert_eq!(gates.len(), 1, "un solo evento gate");
    assert_eq!(gates[0]["extra"]["origen"], "colab");
    assert_eq!(gates[0]["corrida"], "pendientes-tarea-prueba");
    assert_eq!(gates[0]["fase"], "pendientes");

    let tarea = Tarea::cargar_de(&dir).expect("carga la tarea");
    let v = evaluar(repo.path(), &tarea, &opciones(home.path())).expect("evalúa");
    assert_eq!(v.pendientes[0].colab.as_deref(), Some("pasa"));
}

#[test]
fn pendiente_importar_junit_malformado_o_faltante_sale_3() {
    let home = tempfile::tempdir().expect("home");
    tarea_con_pendiente(home.path(), ENTORNO_O2);
    let repo = repo_con_contenido();
    let ruta = repo.path().to_str().expect("ruta UTF-8").to_string();
    let malo = home.path().join("malo.xml");
    fs::write(&malo, "<testsuite>").expect("escribe");
    for junit in [malo, home.path().join("no-existe.xml")] {
        let ruta_junit = junit.to_str().expect("ruta UTF-8").to_string();
        let salida = root_cli(
            home.path(),
            &[
                "oraculo",
                "pendientes",
                "--tarea",
                "tarea-prueba",
                "--repo",
                &ruta,
                "--importar",
                &ruta_junit,
            ],
        );
        assert_eq!(salida.status.code(), Some(3), "junit {ruta_junit} inválido");
    }
    assert!(!home.path().join("evidencia").exists(), "no guarda nada");
}

#[test]
fn pendiente_colab_malformado_no_tumba_evaluar_y_avisa() {
    root::saltar_sin_podman!();
    let _g = preparar();
    let (_dir, tarea) = tarea_con_extra(
        &escribe_xml("contrato.xml", JUNIT_DOS_OK),
        &escribe_xml("oculto.xml", JUNIT_UNA_FALLA),
        "",
        ENTORNO_O2,
    );
    let repo = repo_con_contenido();
    let home = tempfile::tempdir().expect("home");
    let carpeta = home.path().join("evidencia").join("tarea-prueba");
    fs::create_dir_all(&carpeta).expect("carpeta de evidencia");
    fs::write(carpeta.join("colab.json"), "{ esto no es JSON").expect("colab.json");
    let v = evaluar(repo.path(), &tarea, &opciones(home.path())).expect("evalúa sin fallar");
    assert_eq!(v.pendientes.len(), 1);
    assert_eq!(
        v.pendientes[0].colab, None,
        "queda pendiente, sin verde falso"
    );
    assert_eq!(v.avisos.len(), 1, "{:?}", v.avisos);
    assert!(
        v.avisos[0].starts_with("colab.json ilegible: "),
        "{:?}",
        v.avisos
    );
}

#[test]
fn pendiente_colab_valido_no_da_aviso() {
    root::saltar_sin_podman!();
    let _g = preparar();
    let (_dir, tarea) = tarea_con_extra(
        &escribe_xml("contrato.xml", JUNIT_DOS_OK),
        &escribe_xml("oculto.xml", JUNIT_UNA_FALLA),
        "",
        ENTORNO_O2,
    );
    let repo = repo_con_contenido();
    let home = tempfile::tempdir().expect("home");
    let carpeta = home.path().join("evidencia").join("tarea-prueba");
    fs::create_dir_all(&carpeta).expect("carpeta de evidencia");
    let valido = r#"{"test_o::o2": {"resultado": "pasa", "junit_sha256": "abc", "ts": 1.0}}"#;
    fs::write(carpeta.join("colab.json"), valido).expect("colab.json");
    let v = evaluar(repo.path(), &tarea, &opciones(home.path())).expect("evalúa");
    assert_eq!(v.pendientes[0].colab.as_deref(), Some("pasa"));
    assert!(v.avisos.is_empty(), "{:?}", v.avisos);
}
