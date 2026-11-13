//! Prueba con podman real (Tarea 59): un caso que reusa el `target` de la base da la misma Completitud que un
//! caso en frío. Solo cambia el tiempo, y el tiempo sale por `eprintln!`. El oculto usa `CARGO_TARGET_DIR`
//! apuntando a `/salida/target`, como las suites de producción.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use std::time::{Duration, Instant};

use super::defectos::{IMAGEN_HERRAMIENTAS, JUNIT_PY, evaluar_copia_con, tarea_en};
use super::inyectar::pruebas::copia_fixture;
use crate::oracle::rubrica;
use crate::oracle::tarea::Tarea;
use crate::oracle::{AVISO_TARGET_FRIO, Partes, Target, partes_de};
use crate::tipos::{Conteo, Dimension, Puntaje, Veredicto};

/// Oculto que compila con el `target` de `/salida`. Sin esto, cargo compilaría en `/tmp/ws/target` y el reuso no
/// tendría efecto.
const COMANDO_OCULTO_CON_TARGET: &str = "cp -a /repo /tmp/ws && cd /tmp/ws && CARGO_TARGET_DIR=/salida/target cargo test --no-fail-fast --lib --tests 2>&1 | python3 /oraculo/oculto/junit.py > /salida/oculto.xml";

/// Cambio de fuente que mueve el mtime y no cambia el comportamiento del fixture.
const CAMBIO_SIN_EFECTO: &str = "\n// cambio de mtime sin efecto (Tarea 59)\n";

/// Partes de Completitud con la descarga apagada, como en `defectos`.
fn partes_completitud(tarea: &Tarea) -> Partes {
    let rubricas = rubrica::cargar_embebidas().expect("rúbricas embebidas");
    let mut partes = partes_de(Dimension::Completitud, &rubricas, tarea);
    partes.descarga = false;
    partes
}

/// Copia del fixture con el cambio de fuente de `CAMBIO_SIN_EFECTO`.
fn copia_con_cambio() -> tempfile::TempDir {
    let copia = copia_fixture();
    let ruta = copia.path().join("src/protocolo.rs");
    let mut texto = fs::read_to_string(&ruta).expect("lee src/protocolo.rs");
    texto.push_str(CAMBIO_SIN_EFECTO);
    fs::write(&ruta, texto).expect("escribe src/protocolo.rs");
    copia
}

/// Lo que mide Completitud: sus puntajes, el conteo del oculto y los tests malos del oculto.
fn medido_de_completitud(v: &Veredicto) -> (Vec<Puntaje>, Conteo, Vec<String>, Vec<String>) {
    let puntajes = v
        .puntajes
        .iter()
        .filter(|p| p.dimension == Dimension::Completitud)
        .cloned()
        .collect();
    (
        puntajes,
        v.oculto.clone(),
        v.ocultos_fallidos.clone(),
        v.inestables.clone(),
    )
}

/// El oculto con `target` copia el repo con `cp -a`. Así conserva los atributos y el `target` reusado.
#[test]
fn comando_oculto_con_target_no_usa_cp_r() {
    assert!(
        !COMANDO_OCULTO_CON_TARGET.contains("cp -r"),
        "{COMANDO_OCULTO_CON_TARGET}"
    );
}

#[test]
fn target_podman_reusado_da_la_misma_completitud_que_en_frio() {
    crate::saltar_sin_podman!();
    let dir = tempfile::tempdir().expect("carpeta de la tarea");
    let tarea = tarea_en(
        dir.path(),
        IMAGEN_HERRAMIENTAS,
        COMANDO_OCULTO_CON_TARGET,
        "",
    );
    // El conversor del oculto va en la carpeta de la tarea, como en `defectos`.
    let carpeta = dir.path().join("oculto");
    fs::create_dir_all(&carpeta).expect("carpeta del oculto");
    fs::write(carpeta.join("junit.py"), JUNIT_PY).expect("conversor del oculto");
    let home = tempfile::tempdir().expect("ROOT_HOME de prueba");
    let temporal = tempfile::tempdir().expect("carpeta temporal");
    let target_base = temporal.path().join("target-base");

    let inicio = Instant::now();
    let base = evaluar_copia_con(
        copia_fixture().path(),
        &tarea,
        home.path(),
        partes_completitud(&tarea),
        None,
        None,
        Target::Guardar(target_base.clone()),
    );
    eprintln!(
        "target_podman base (guarda el target): {:.2} s",
        inicio.elapsed().as_secs_f64()
    );
    assert!(
        target_base.is_dir(),
        "la base dejó el target en {}",
        target_base.display()
    );
    assert!(
        !base.avisos.iter().any(|a| a.starts_with(AVISO_TARGET_FRIO)),
        "la base guarda sin aviso: {:?}",
        base.avisos
    );

    let copia_reusa = copia_con_cambio();
    let inicio = Instant::now();
    let reusado = evaluar_copia_con(
        copia_reusa.path(),
        &tarea,
        home.path(),
        partes_completitud(&tarea),
        None,
        None,
        Target::Inicial(target_base.clone()),
    );
    eprintln!(
        "target_podman caso reusado (Inicial): {:.2} s",
        inicio.elapsed().as_secs_f64()
    );

    let copia_frio = copia_con_cambio();
    let inicio = Instant::now();
    let frio = evaluar_copia_con(
        copia_frio.path(),
        &tarea,
        home.path(),
        partes_completitud(&tarea),
        None,
        None,
        Target::Ninguno,
    );
    eprintln!(
        "target_podman caso en frío (Ninguno): {:.2} s",
        inicio.elapsed().as_secs_f64()
    );

    assert!(
        !reusado
            .avisos
            .iter()
            .any(|a| a.starts_with(AVISO_TARGET_FRIO)),
        "el caso reusado no avisa target frío: {:?}",
        reusado.avisos
    );
    assert_eq!(
        medido_de_completitud(&reusado),
        medido_de_completitud(&frio),
        "Completitud reusada y en frío deben coincidir"
    );
}

/// Corre `cargo test --offline` en `copia`, con `target` como `CARGO_TARGET_DIR`.
/// Quita las variables `CARGO_PROFILE_*` heredadas. Con `sin_debuginfo`, pone `debug=0` en dev y test.
fn correr_cargo_test(copia: &Path, target: &Path, sin_debuginfo: bool) -> (Output, Duration) {
    let mut orden = Command::new("cargo");
    orden
        .args(["test", "--offline"])
        .current_dir(copia)
        .env("CARGO_TARGET_DIR", target);
    for (clave, _) in std::env::vars() {
        if clave.starts_with("CARGO_PROFILE_") {
            orden.env_remove(&clave);
        }
    }
    if sin_debuginfo {
        orden
            .env("CARGO_PROFILE_DEV_DEBUG", "0")
            .env("CARGO_PROFILE_TEST_DEBUG", "0");
    }
    let inicio = Instant::now();
    let salida = orden.output().expect("lanza cargo test");
    (salida, inicio.elapsed())
}

/// Líneas `test <nombre> ... <estado>` de la salida de cargo, ordenadas.
fn tests_de_la_salida(stdout: &[u8]) -> Vec<String> {
    let texto = String::from_utf8_lossy(stdout);
    let mut lineas: Vec<String> = texto
        .lines()
        .filter_map(|linea| linea.strip_prefix("test "))
        .filter(|resto| resto.contains(" ... "))
        .map(str::to_string)
        .collect();
    lineas.sort();
    lineas
}

/// Suma los bytes de los archivos de `ruta`, de forma recursiva. Una carpeta inexistente suma cero.
fn tamano_en_bytes(ruta: &Path) -> u64 {
    if !ruta.is_dir() {
        return 0;
    }
    let mut total = 0;
    for entrada in fs::read_dir(ruta).expect("lee la carpeta") {
        let entrada = entrada.expect("entrada");
        if entrada.file_type().expect("tipo").is_dir() {
            total += tamano_en_bytes(&entrada.path());
        } else {
            total += entrada.metadata().expect("metadatos").len();
        }
    }
    total
}

/// Mide el perfil sin debuginfo: mismos tests, distinto tiempo y tamaño de `target`.
/// Corre a mano: compila en el host, sin podman.
#[test]
#[ignore = "mide compilación en el host; correr a mano"]
fn perfil_sin_debuginfo_da_los_mismos_tests_y_mide_la_ganancia() {
    // Lado A: perfil por defecto. Su `target` vive en otra carpeta temporal que la copia.
    let copia_a = copia_fixture();
    let temporal_a = tempfile::tempdir().expect("carpeta del target A");
    let target_a = temporal_a.path().join("target");
    let (salida_a, tiempo_a) = correr_cargo_test(copia_a.path(), &target_a, false);

    // Lado B: otra copia y otro `target`, con debuginfo apagada.
    let copia_b = copia_fixture();
    let temporal_b = tempfile::tempdir().expect("carpeta del target B");
    let target_b = temporal_b.path().join("target");
    let (salida_b, tiempo_b) = correr_cargo_test(copia_b.path(), &target_b, true);

    eprintln!(
        "perfil_sin_debuginfo tiempo A (perfil por defecto): {:.2} s",
        tiempo_a.as_secs_f64()
    );
    eprintln!(
        "perfil_sin_debuginfo tiempo B (debug=0): {:.2} s",
        tiempo_b.as_secs_f64()
    );
    eprintln!(
        "perfil_sin_debuginfo tamaño A: {} bytes",
        tamano_en_bytes(&target_a)
    );
    eprintln!(
        "perfil_sin_debuginfo tamaño B: {} bytes",
        tamano_en_bytes(&target_b)
    );

    assert!(
        salida_a.status.success(),
        "lado A falló:\n{}",
        String::from_utf8_lossy(&salida_a.stderr)
    );
    assert!(
        salida_b.status.success(),
        "lado B falló:\n{}",
        String::from_utf8_lossy(&salida_b.stderr)
    );
    let tests_a = tests_de_la_salida(&salida_a.stdout);
    let tests_b = tests_de_la_salida(&salida_b.stdout);
    assert!(!tests_a.is_empty(), "el lado A no lista tests");
    assert!(!tests_b.is_empty(), "el lado B no lista tests");
    assert_eq!(
        tests_a, tests_b,
        "el perfil sin debuginfo cambia los tests o sus estados"
    );
}
