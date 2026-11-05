//! Un test por defecto de `Defecto` (Tarea 48, punto 6). Cada test evalúa la copia del fixture
//! `tests/fixtures/calibra/repo-rust` sin defecto (base), inyecta el defecto con `inyectar_con` y evalúa
//! la copia con el defecto. Con las mismas partes (`partes_de` de la dimensión del defecto) afirma que la
//! dimensión baja y que el ítem de rúbrica baja. La inyección tiene que ser válida: no rompe la compilación.
//!
//! Qué es real y qué es falso en cada test: ver el informe de la tarea. En corto:
//! - Sin podman (sondas nativas o de archivos): Panico, Ciclo, Dios y ErrorTragado (el juez es falso).
//! - Con podman y herramienta real: Secreto (gitleaks), Bucle (el comando de `[eficiencia]`), Modulo
//!   (el oculto, con un junit que escribe el propio test), Assert (cargo-mutants), Dockerfile (sonda de Entrega).
//!
//! Todas las partes que no usa el test se apagan después de `partes_de`: la descarga siempre (baja
//! dependencias y la base de RustSec, por red) y los contenedores que el test no mide.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use super::inyectar::pruebas::{TAREA_BASE, copia_fixture};
use super::inyectar::{Contexto, Defecto, Inyeccion, inyectar_con};
use super::*;
use crate::config::ReglasDecision;
use crate::oracle::descarga;
use crate::oracle::entrega::{FabricaReal, PodmanReal};
use crate::oracle::juez::ModeloJuez;
use crate::oracle::memo::MemoSondas;
use crate::oracle::perfiles::herramientas::{EjecutorContenedor, EjecutorHerramienta};
use crate::oracle::perfiles::rust::LimitesEstructura;
use crate::oracle::rubrica;
use crate::oracle::{
    Dependencias, OpcionesOraculo, Preparada, Target, correr_en_podman, evaluar_con,
};
use crate::proveedores::{Prompt, RespuestaJuez};

/// Semilla de todos los tests. Con ella, cada defecto cae en un lugar fijo de la copia.
const SEMILLA: u64 = 42;
/// Límite de cada corrida de contenedor en estos tests.
const TIMEOUT: Duration = Duration::from_secs(900);
/// Imagen con las herramientas de las sondas: gitleaks, hadolint, cargo-audit, clippy y cargo-mutants.
pub(super) const IMAGEN_HERRAMIENTAS: &str = "localhost/root-oraculo-rust:2";
/// Nombre del juez falso de Diseño. Va a la fuente de cada nivel y de cada hallazgo.
const NOMBRE_JUEZ: &str = "juez-falso";
/// Ficha de la tarea de prueba: la de `tests/fixtures/fichas/spec-con-pruebas.md`. Solo debe parsear.
const FICHA: &str = include_str!("../../../tests/fixtures/fichas/spec-con-pruebas.md");
/// Requisito de la tarea: su módulos y su nombre aparecen en el impacto de los hallazgos.
const REQUISITO: &str = "[[requisito]]\nid = \"R1\"\ntitulo = \"Lee y guarda valores\"\npeso = 3\nmodulos = [\"almacen\", \"comandos\", \"protocolo\"]\n";

/// Escribe `tarea.toml` en `dir`: `TAREA_BASE` con la imagen, el comando del oculto y `extra`. Escribe la
/// ficha de prueba y carga la tarea.
pub(super) fn tarea_en(dir: &Path, imagen: &str, oculto: &str, extra: &str) -> Tarea {
    let base = TAREA_BASE
        .replace(
            "imagen = \"root-oraculo-rust:1\"",
            &format!("imagen = \"{imagen}\""),
        )
        .replace(
            "[oculto]\ncmd = \"cargo test\"",
            &format!("[oculto]\ncmd = \"{oculto}\""),
        );
    fs::write(dir.join("tarea.toml"), format!("{base}{extra}")).expect("escribe tarea.toml");
    fs::create_dir_all(dir.join("bootstrap/docs")).expect("carpeta de la ficha");
    fs::write(dir.join("bootstrap/docs/SPEC.md"), FICHA).expect("escribe la ficha");
    Tarea::cargar_de(dir).expect("carga la tarea de prueba")
}

/// Partes de `partes_de` para `dimension`, con la descarga apagada y lo que `ajustar` pida apagado.
fn partes_para(dimension: Dimension, tarea: &Tarea, ajustar: impl FnOnce(&mut Partes)) -> Partes {
    let rubricas = rubrica::cargar_embebidas().expect("rúbricas embebidas");
    let mut partes = partes_de(dimension, &rubricas, tarea);
    partes.descarga = false;
    ajustar(&mut partes);
    partes
}

/// Evalúa `repo` con la tarea y las partes, como `evaluar`, con las dependencias de producción y un juez
/// que se pase. Solo el juez cambia: el contenedor, la descarga y las sondas son los de producción.
fn evaluar_copia(
    repo: &Path,
    tarea: &Tarea,
    home: &Path,
    partes: Partes,
    juez: Option<&dyn ModeloJuez>,
    memo: Option<Arc<MemoSondas>>,
) -> Veredicto {
    evaluar_copia_con(repo, tarea, home, partes, juez, memo, Target::Ninguno)
}

/// Como `evaluar_copia`, con el `target` que pide la calibración (Tarea 59). Lo usa `target_podman`.
pub(super) fn evaluar_copia_con(
    repo: &Path,
    tarea: &Tarea,
    home: &Path,
    partes: Partes,
    juez: Option<&dyn ModeloJuez>,
    memo: Option<Arc<MemoSondas>>,
    target: Target,
) -> Veredicto {
    let rubricas = rubrica::cargar_embebidas().expect("rúbricas embebidas");
    let calibracion = Calibraciones::cargar(home).expect("calibración vacía");
    let herramientas = |p: &Preparada| -> Box<dyn EjecutorHerramienta> {
        Box::new(EjecutorContenedor {
            imagen: tarea.imagen.clone(),
            repo: p.repo.clone(),
            oraculo: None,
            rustsec: p.rustsec.clone(),
            cache: p.cache.clone(),
            timeout: TIMEOUT,
            home: home.to_path_buf(),
        })
    };
    let deps = Dependencias {
        correr: &correr_en_podman,
        herramientas: &herramientas,
        juez,
        modelo_juez: if juez.is_some() { NOMBRE_JUEZ } else { "" },
        juez_config: None,
        calibracion: &calibracion,
        rubricas: &rubricas,
        reglas: ReglasDecision::default(),
        redactor: None,
        avisos: Vec::new(),
        podman: &PodmanReal,
        escenarios: &FabricaReal,
    };
    let opciones = OpcionesOraculo {
        timeout: TIMEOUT,
        home: Some(home.to_path_buf()),
        partes: Some(partes),
        historial: false,
        memo,
        reuso: None,
        target,
    };
    evaluar_con(repo, tarea, &opciones, deps).expect("la evaluación da veredicto")
}

/// Base y copia con el defecto, con la misma evaluación.
struct Ensayo {
    base: Veredicto,
    con_defecto: Veredicto,
    inyeccion: Inyeccion,
}

/// Prepara la copia del fixture (con `preparar`), evalúa la base, inyecta `defecto` con la semilla fija y
/// evalúa la copia con el defecto. La base graba sus sondas en un memo; `muertos` sale de ese memo.
fn ensayar(
    defecto: Defecto,
    tarea: &Tarea,
    partes: Partes,
    juez: Option<&dyn ModeloJuez>,
    preparar: &dyn Fn(&Path),
) -> Ensayo {
    ensayar_con_home(defecto, tarea, partes, juez, preparar, &|_| {})
}

/// Como `ensayar`, con `sembrar` antes de la base: prepara el `ROOT_HOME` que las sondas van a leer.
fn ensayar_con_home(
    defecto: Defecto,
    tarea: &Tarea,
    partes: Partes,
    juez: Option<&dyn ModeloJuez>,
    preparar: &dyn Fn(&Path),
    sembrar: &dyn Fn(&Path),
) -> Ensayo {
    let home = tempfile::tempdir().expect("ROOT_HOME de prueba");
    sembrar(home.path());
    let copia = copia_fixture();
    preparar(copia.path());
    let memo = Arc::new(MemoSondas::nuevo());
    let base = evaluar_copia(
        copia.path(),
        tarea,
        home.path(),
        partes,
        juez,
        Some(memo.clone()),
    );
    let muertos = memo.mutantes_muertos(tarea);
    let limites = LimitesEstructura::default();
    let capas: &[Vec<String>] = tarea
        .arquitectura
        .as_ref()
        .map_or(&[], |a| a.capas.as_slice());
    let camino: &[String] = tarea
        .calibracion
        .as_ref()
        .map_or(&[], |c| c.camino_caliente.as_slice());
    let contexto = Contexto {
        capas,
        camino_caliente: camino,
        limites: &limites,
        muertos: &muertos,
    };
    let inyeccion = inyectar_con(defecto, copia.path(), &contexto, SEMILLA)
        .expect("inyecta el defecto")
        .unwrap_or_else(|| panic!("{defecto:?} no es aplicable a la copia"));
    let con_defecto = evaluar_copia(copia.path(), tarea, home.path(), partes, juez, None);
    Ensayo {
        base,
        con_defecto,
        inyeccion,
    }
}

/// Si el defecto de `e` baja su dimensión, por el mismo camino de la calibración: `finalizar_casos` sobre el caso.
fn detecta_defecto(e: &Ensayo) -> bool {
    let mut casos = vec![Caso::nuevo(
        0,
        Some(e.inyeccion.clone()),
        e.con_defecto.clone(),
    )];
    finalizar_casos(&e.base, e.inyeccion.defecto.dimension(), &[], &mut casos);
    casos[0].detectado
}

/// Afirma lo que pide el test. La inyección es válida y la dimensión baja. Si `en_linea`, un hallazgo nuevo
/// de la dimensión cae en la línea que el inyector tocó. Si la base mide el ítem de rúbrica como ítem (juez o
/// mixto), ese ítem baja. Las sondas no llenan `items`: para ellas, el hallazgo en la línea es la prueba.
fn afirma_detectado(e: &Ensayo, defecto: Defecto, en_linea: bool) {
    let dimension = defecto.dimension();
    assert_eq!(e.inyeccion.defecto, defecto, "la inyección es otro defecto");
    assert!(
        !rompe_la_compilacion(&e.base, &e.con_defecto, dimension),
        "la inyección rompe la compilación: {:?}",
        e.inyeccion
    );
    assert!(
        detecta_defecto(e),
        "{dimension:?} no baja con {:?}",
        e.inyeccion
    );
    if en_linea {
        let ids_base: BTreeSet<&str> = e.base.hallazgos.iter().map(|h| h.id.as_str()).collect();
        let nuevo = e.con_defecto.hallazgos.iter().any(|h| {
            h.dimension == dimension
                && !ids_base.contains(h.id.as_str())
                && h.archivo == e.inyeccion.archivo
                && h.linea == e.inyeccion.linea
        });
        assert!(
            nuevo,
            "ningún hallazgo nuevo de {dimension:?} cae en {}:{}: {:?}",
            e.inyeccion.archivo, e.inyeccion.linea, e.con_defecto.hallazgos
        );
    }
    if let Some(bajo) = detecta_item(&e.base, &e.con_defecto, dimension, defecto.item(), true) {
        assert!(
            bajo,
            "el ítem {} no baja con {:?}",
            defecto.item(),
            e.inyeccion
        );
    }
}

/// Sin cambios en la copia.
fn sin_cambios(_: &Path) {}

#[test]
fn secreto_en_codigo_lo_detecta_gitleaks() {
    crate::saltar_sin_podman!();
    let dir = tempfile::tempdir().expect("tarea");
    let tarea = tarea_en(dir.path(), IMAGEN_HERRAMIENTAS, "cargo test", REQUISITO);
    let partes = partes_para(Dimension::Seguridad, &tarea, |p| {
        p.nativas = false;
        p.cargo_audit = false;
        p.configs = false;
    });
    let e = ensayar(Defecto::SecretoEnCodigo, &tarea, partes, None, &sin_cambios);
    afirma_detectado(&e, Defecto::SecretoEnCodigo, true);
}

#[test]
fn panico_en_entrada_lo_detecta_la_sonda_nativa() {
    let dir = tempfile::tempdir().expect("tarea");
    let tarea = tarea_en(dir.path(), IMAGEN_HERRAMIENTAS, "cargo test", REQUISITO);
    let partes = partes_para(Dimension::Seguridad, &tarea, |p| {
        p.gitleaks = false;
        p.cargo_audit = false;
        p.configs = false;
    });
    let e = ensayar(Defecto::PanicoEnEntrada, &tarea, partes, None, &sin_cambios);
    afirma_detectado(&e, Defecto::PanicoEnEntrada, true);
}

#[test]
fn bucle_cuadratico_lo_detecta_el_comando_de_eficiencia() {
    crate::saltar_sin_podman!();
    // El comando compila y corre `examples/medir.rs` de la copia, que llama a las funciones del fixture.
    // Un defecto en cualquiera de ellas baja `ops_s` bajo el piso.
    let dir = tempfile::tempdir().expect("tarea");
    let extra =
        format!("{REQUISITO}\n[eficiencia]\ncmd = \"{COMANDO_EFICIENCIA}\"\npiso = 1000.0\n");
    let tarea = tarea_en(dir.path(), IMAGEN_HERRAMIENTAS, "cargo test", &extra);
    let partes = partes_para(Dimension::Eficiencia, &tarea, |_| {});
    let e = ensayar(Defecto::BucleCuadratico, &tarea, partes, None, &|repo| {
        let ejemplo = repo.join("examples");
        fs::create_dir_all(&ejemplo).expect("carpeta de ejemplos");
        fs::write(ejemplo.join("medir.rs"), MEDIR_RS).expect("escribe el ejemplo");
    });
    afirma_detectado(&e, Defecto::BucleCuadratico, false);
}

/// Comando de `[eficiencia]`: copia el repo a `/tmp` (rw) y corre el ejemplo en debug. El ejemplo imprime
/// `ops_s` en su última línea. Debug, para que el bucle no lo borre el optimizador.
const COMANDO_EFICIENCIA: &str = "cp -r /repo /tmp/ws && cd /tmp/ws && CARGO_TARGET_DIR=/tmp/ws/target cargo run -q --example medir";

/// Ejemplo de la copia: llama a cada función de producción del fixture, unas 20 veces.
const MEDIR_RS: &str = r#"use std::time::Instant;

use calibra_fixture::almacen::Almacen;
use calibra_fixture::comandos::ejecutar;
use calibra_fixture::protocolo::leer_numero;

fn main() {
    const VUELTAS: u32 = 20;
    let inicio = Instant::now();
    let mut suma: u64 = 0;
    for i in 0..VUELTAS {
        let mut almacen = Almacen::default();
        almacen.set("a", i);
        suma += almacen.get("a").unwrap_or(0) as u64;
        suma += leer_numero("42").unwrap_or(0) as u64;
        suma += ejecutar("7").unwrap_or(0) as u64;
    }
    let segundos = inicio.elapsed().as_secs_f64();
    println!("{suma}");
    println!("{{\"ops_s\": {}}}", VUELTAS as f64 / segundos);
}
"#;

#[test]
fn ciclo_entre_capas_lo_detecta_la_sonda_nativa() {
    let dir = tempfile::tempdir().expect("tarea");
    let tarea = tarea_en(dir.path(), IMAGEN_HERRAMIENTAS, "cargo test", REQUISITO);
    let partes = partes_para(Dimension::Arquitectura, &tarea, |p| {
        p.entrega = false;
        p.gitleaks = false;
        p.cargo_audit = false;
        p.clippy = false;
        p.eficiencia = false;
        p.mutacion = false;
        p.configs = false;
        p.hadolint = false;
    });
    let e = ensayar(
        Defecto::CicloEntreCapas,
        &tarea,
        partes,
        Some(&JuezFalso),
        &sin_cambios,
    );
    afirma_detectado(&e, Defecto::CicloEntreCapas, true);
}

#[test]
fn archivo_dios_lo_detecta_la_sonda_nativa() {
    let dir = tempfile::tempdir().expect("tarea");
    let tarea = tarea_en(dir.path(), IMAGEN_HERRAMIENTAS, "cargo test", REQUISITO);
    let partes = partes_para(Dimension::Estructura, &tarea, |p| {
        p.entrega = false;
        p.gitleaks = false;
        p.cargo_audit = false;
        p.clippy = false;
        p.eficiencia = false;
        p.mutacion = false;
        p.configs = false;
        p.hadolint = false;
    });
    let e = ensayar(
        Defecto::ArchivoDios,
        &tarea,
        partes,
        Some(&JuezFalso),
        &sin_cambios,
    );
    afirma_detectado(&e, Defecto::ArchivoDios, true);
}

#[test]
fn modulo_borrado_lo_detecta_el_oculto_real() {
    crate::saltar_sin_podman!();
    let dir = tempfile::tempdir().expect("tarea");
    let tarea = tarea_en(
        dir.path(),
        IMAGEN_HERRAMIENTAS,
        COMANDO_OCULTO,
        REQUISITO_MODULO,
    );
    // El junit lo escribe `oculto/junit.py` de la carpeta de la tarea: `cargo test` no lo produce.
    let carpeta = dir.path().join("oculto");
    fs::create_dir_all(&carpeta).expect("carpeta del oculto");
    fs::write(carpeta.join("junit.py"), JUNIT_PY).expect("escribe el conversor");
    let partes = partes_para(Dimension::Completitud, &tarea, |_| {});
    let e = ensayar(Defecto::ModuloBorrado, &tarea, partes, None, &|repo| {
        fs::write(repo.join("tests/comandos.rs"), TEST_COMANDOS).expect("test de comandos");
    });
    afirma_detectado(&e, Defecto::ModuloBorrado, false);
}

/// Requisito de comandos: su test vive en `tests/comandos.rs` de la copia. Sin `comandos`, el test no existe.
const REQUISITO_MODULO: &str = "[[requisito]]\nid = \"R1\"\ntitulo = \"Ejecuta comandos\"\npeso = 3\npruebas = [\"test_comandos::ejecuta_numero_valido\"]\nmodulos = [\"comandos\"]\n";

/// Test de la copia: usa `comandos`. Sin el módulo, este archivo no compila.
const TEST_COMANDOS: &str = "use calibra_fixture::comandos::ejecutar;\n\n#[test]\nfn ejecuta_numero_valido() {\n    assert_eq!(ejecutar(\"7\"), Ok(7));\n}\n";

/// Oculto: `cargo test` en una copia de escritura, con su salida convertida a junit por `junit.py`.
const COMANDO_OCULTO: &str = "cp -r /repo /tmp/ws && cd /tmp/ws && cargo test --no-fail-fast --lib --tests 2>&1 | python3 /oraculo/oculto/junit.py > /salida/oculto.xml";

/// Conversor de la salida de `cargo test` a junit: un `testcase` por línea `test <nombre> ... <estado>`.
/// La clase es `tests.test_<binario>`, con el nombre del archivo de la corrida (`tests/comandos.rs` da `comandos`).
pub(super) const JUNIT_PY: &str = r#"import re, sys
from xml.sax.saxutils import quoteattr

binario = "lib"
casos = []
compila = True
for linea in sys.stdin:
    if re.match(r"error(\[E\d+\])?:", linea):
        compila = False
    m = re.match(r"\s*Running (\S+)", linea)
    if m:
        ruta = m.group(1)
        binario = ruta.rsplit("/", 1)[-1].removesuffix(".rs") if ruta.startswith("tests/") else "lib"
        continue
    m = re.match(r"test (\S+) \.\.\. (ok|FAILED|ignored)", linea)
    if m:
        casos.append((binario, m.group(1), m.group(2)))

# Un error de compilación no deja ningún `test ...`: la suite lo dice con un test fallido, no con un junit vacío.
if not casos and not compila:
    casos.append(("build", "compila", "FAILED"))

print("<testsuite>")
for binario, nombre, estado in casos:
    clase = quoteattr("tests.test_" + binario)
    nombre = quoteattr(nombre)
    if estado == "FAILED":
        print(f'<testcase classname={clase} name={nombre}><failure message="x"/></testcase>')
    elif estado == "ignored":
        print(f'<testcase classname={clase} name={nombre}><skipped/></testcase>')
    else:
        print(f'<testcase classname={clase} name={nombre}/>')
print("</testsuite>")
"#;

/// Lock del fixture de mutación: sin dependencias. Con su caché sembrada, la descarga no baja nada.
const LOCK_MUTACION: &str =
    "version = 4\n\n[[package]]\nname = \"calibra-fixture\"\nversion = \"0.1.0\"\n";

/// Caché de Rust y base de RustSec del `ROOT_HOME`, con el hash de `LOCK_MUTACION`. Así la descarga no
/// levanta contenedores ni baja nada. Sin `config.toml` en la caché, el comando de `cargo mutants` no corre.
fn sembrar_cache_mutacion(home: &Path) {
    let hash = descarga::hash_corto(LOCK_MUTACION.as_bytes());
    let cache = home.join("cache/rust").join(hash);
    fs::create_dir_all(&cache).expect("caché de Rust");
    fs::write(cache.join("config.toml"), "").expect("config.toml");
    fs::create_dir_all(home.join("cache/rustsec")).expect("base de RustSec");
}

#[test]
fn assert_borrado_lo_detecta_cargo_mutants() {
    crate::saltar_sin_podman!();
    let dir = tempfile::tempdir().expect("tarea");
    let tarea = tarea_en(dir.path(), IMAGEN_HERRAMIENTAS, "cargo test", REQUISITO);
    // La sonda de mutación lee `/cache/config.toml`: sin descarga, el comando falla antes de `cargo mutants`.
    // La descarga corre, pero con la caché sembrada. El fixture no tiene dependencias y su `Dockerfile` pediría
    // traer la imagen base por red: la copia de este test no lo lleva.
    let partes = partes_para(Dimension::Pruebas, &tarea, |p| p.descarga = true);
    let preparar = |repo: &Path| {
        fs::remove_file(repo.join("Dockerfile")).expect("sin Dockerfile: no pide la imagen base");
        fs::write(repo.join("Cargo.lock"), LOCK_MUTACION).expect("Cargo.lock");
    };
    let e = ensayar_con_home(
        Defecto::AssertBorrado,
        &tarea,
        partes,
        None,
        &preparar,
        &sembrar_cache_mutacion,
    );
    afirma_detectado(&e, Defecto::AssertBorrado, false);
}

#[test]
fn error_tragado_lo_detecta_el_juez_falso() {
    let dir = tempfile::tempdir().expect("tarea");
    let tarea = tarea_en(dir.path(), IMAGEN_HERRAMIENTAS, "cargo test", REQUISITO);
    let partes = partes_para(Dimension::Diseno, &tarea, |p| {
        p.gitleaks = false;
        p.cargo_audit = false;
        p.clippy = false;
        p.eficiencia = false;
        p.mutacion = false;
        p.configs = false;
        p.hadolint = false;
        p.entrega = false;
    });
    let juez = JuezFalso;
    let e = ensayar(
        Defecto::ErrorTragado,
        &tarea,
        partes,
        Some(&juez),
        &sin_cambios,
    );
    afirma_detectado(&e, Defecto::ErrorTragado, true);
}

#[test]
fn dockerfile_root_lo_detecta_la_sonda_de_entrega() {
    crate::saltar_sin_podman!();
    let dir = tempfile::tempdir().expect("tarea");
    let tarea = tarea_en(dir.path(), IMAGEN_HERRAMIENTAS, "cargo test", REQUISITO);
    // hadolint no marca un Dockerfile sin `USER` (solo marca `USER root` explícito). La regla de root es de la
    // sonda de Entrega: por eso la Entrega va entera. Solo se apaga el juez de configs.
    let partes = partes_para(Dimension::Entrega, &tarea, |p| {
        p.juez_config = false;
    });
    let e = ensayar(Defecto::DockerfileRoot, &tarea, partes, None, &sin_cambios);
    // La regla cita el `FROM` de la última etapa (línea 1 del fixture), no la línea del `USER` que se quitó.
    afirma_detectado(&e, Defecto::DockerfileRoot, false);
    let cita_el_from =
        e.con_defecto.hallazgos.iter().any(|h| {
            h.dimension == Dimension::Entrega && h.archivo == "Dockerfile" && h.linea == 1
        });
    assert!(
        cita_el_from,
        "ningún hallazgo de Entrega cita el FROM del Dockerfile: {:?}",
        e.con_defecto.hallazgos
    );
}

/// Juez falso (FALSO: no llama a ningún modelo). Responde cada ítem pedido en `IDS OBLIGATORIOS:` con nivel 3
/// y una cita de `src/lib.rs`. Sin `let _ =` en el prompt, todos quedan en 3: solo da cobertura de juez a
/// Arquitectura y Estructura, que sin juez no tienen veredicto (`sin_veredicto` por cobertura). Con un
/// `let _ =` en el prompt (lo que deja `ErrorTragado`), baja `diseno.errores` a nivel 1 y cita esa línea en
/// un hallazgo. No mira nada más del repo: solo el texto que recibe.
struct JuezFalso;

impl ModeloJuez for JuezFalso {
    fn consultar(&self, prompt: &Prompt) -> Result<RespuestaJuez, crate::oracle::ErrorOraculo> {
        Ok(RespuestaJuez {
            texto: respuesta_falsa(&prompt.usuario),
            truncada: false,
            costo_usd: 0.0,
        })
    }
}

/// Texto de respuesta del juez falso para `usuario`.
fn respuesta_falsa(usuario: &str) -> String {
    let ids = ids_pedidos(usuario);
    let con_let = donde_let_guion(usuario);
    let items: Vec<Value> = ids
        .iter()
        .map(|id| {
            let nivel = match (id.as_str(), &con_let) {
                ("diseno.errores", Some(_)) => 1,
                _ => 3,
            };
            json!({
                "item": id,
                "nivel": nivel,
                "evidencia": [{"archivo": "src/lib.rs", "linea": 1, "cita": CITA_LIB}],
            })
        })
        .collect();
    let hallazgos: Vec<Value> = con_let
        .into_iter()
        .map(|(archivo, linea, cita)| {
            json!({
                "item": "diseno.errores",
                "severidad": 2,
                "archivo": archivo,
                "linea": linea,
                "cita": cita,
                "detalle": "el error se traga con let _ =",
                "impacto": ["R1"],
            })
        })
        .collect();
    json!({"items": items, "hallazgos": hallazgos}).to_string()
}

/// Línea 1 de `src/lib.rs` del fixture: la cita que el juez falso usa para los ítems sin hallazgo.
const CITA_LIB: &str = "//! Repo de prueba de la calibración: tres módulos por capa y un test.";

/// Ids de la lista `IDS OBLIGATORIOS:` del prompt, en orden.
fn ids_pedidos(usuario: &str) -> Vec<String> {
    usuario
        .split("IDS OBLIGATORIOS:\n")
        .nth(1)
        .unwrap_or_default()
        .lines()
        .take_while(|l| l.starts_with("- "))
        .map(|l| l[2..].to_string())
        .collect()
}

/// Archivo, línea y cita de la primera línea de los extractos que tiene `let _ =`. Los extractos vienen
/// como `--- <archivo> ---` seguido de `N: <texto>` por línea.
fn donde_let_guion(usuario: &str) -> Option<(String, u32, String)> {
    let mut archivo = String::new();
    for linea in usuario.lines() {
        if let Some(nombre) = linea
            .strip_prefix("--- ")
            .and_then(|r| r.strip_suffix(" ---"))
        {
            archivo = nombre.to_string();
            continue;
        }
        if let Some((numero, texto)) = linea.split_once(": ")
            && let Ok(n) = numero.parse::<u32>()
            && texto.contains("let _ =")
        {
            return Some((archivo, n, texto.trim().to_string()));
        }
    }
    None
}
