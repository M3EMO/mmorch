//! Prueba con podman real (Tarea 59): un caso que reusa el `target` de la base da la misma Completitud que un
//! caso en frío. Solo cambia el tiempo, y el tiempo sale por `eprintln!`. El oculto usa `CARGO_TARGET_DIR`
//! apuntando a `/salida/target`, como las suites de producción.

use std::fs;
use std::time::Instant;

use super::defectos::{IMAGEN_HERRAMIENTAS, JUNIT_PY, evaluar_copia_con, tarea_en};
use super::inyectar::pruebas::copia_fixture;
use crate::oracle::rubrica;
use crate::oracle::tarea::Tarea;
use crate::oracle::{AVISO_TARGET_FRIO, Partes, Target, partes_de};
use crate::tipos::{Conteo, Dimension, Puntaje, Veredicto};

/// Oculto que compila con el `target` de `/salida`. Sin esto, cargo compilaría en `/tmp/ws/target` y el reuso no
/// tendría efecto.
const COMANDO_OCULTO_CON_TARGET: &str = "cp -r /repo /tmp/ws && cd /tmp/ws && CARGO_TARGET_DIR=/salida/target cargo test --no-fail-fast --lib --tests 2>&1 | python3 /oraculo/oculto/junit.py > /salida/oculto.xml";

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
