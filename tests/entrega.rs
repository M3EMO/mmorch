//! Entrega y escenarios (Tarea 9d). Los de podman corren contra alpine sin red: la imagen debe
//! existir localmente. El resto es puro.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use root::oracle::ErrorOraculo;
use root::oracle::entrega::sonda_entrega;
use root::oracle::junit::ResultadoTest;
use root::oracle::perfiles::herramientas::{
    EjecutorContenedor, EjecutorHerramienta, SalidaHerramienta,
};
use root::oracle::requisitos::{ResultadosTests, evaluar_requisitos};
use root::oracle::tarea::Tarea;
use root::tipos::{Cumplimiento, Dimension, Fuente};

const ALPINE: &str = "docker.io/library/alpine:3.20";
const LIMITE: Duration = Duration::from_secs(120);

/// Falla con un mensaje claro si la imagen no está. Nunca la baja: el test no tiene red.
fn asegurar_imagen(imagen: &str) {
    let existe = Command::new("podman")
        .args(["image", "exists", imagen])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    assert!(existe, "falta la imagen {imagen} en podman");
}

fn ejecutor(repo: &Path) -> EjecutorContenedor {
    EjecutorContenedor {
        imagen: ALPINE.into(),
        repo: repo.into(),
        oraculo: None,
        rustsec: None,
        cache: None,
        timeout: LIMITE,
    }
}

/// Tarea de lenguaje `shell`, con `extra` al final del `tarea.toml`.
fn tarea_con(extra: &str) -> Tarea {
    let dir = tempfile::tempdir().expect("temp dir");
    let texto = format!(
        "nombre = \"x\"\nlenguaje = \"shell\"\nimagen = \"i\"\nspec = \"s\"\n\
         [contrato]\ncmd = \"c\"\n[oculto]\ncmd = \"o\"\n{extra}"
    );
    fs::write(dir.path().join("tarea.toml"), texto).expect("escribe tarea.toml");
    Tarea::cargar_de(dir.path()).expect("carga la tarea")
}

/// Repo con README, Dockerfile que define `USER` y el `run.sh` dado.
fn repo_con_run(run_sh: &str) -> tempfile::TempDir {
    let repo = tempfile::tempdir().expect("temp dir");
    fs::write(repo.path().join("README.md"), "# x\n").expect("escribe README");
    fs::write(
        repo.path().join("Dockerfile"),
        "FROM debian:bookworm-slim\nUSER app\n",
    )
    .expect("escribe Dockerfile");
    fs::write(repo.path().join("run.sh"), run_sh).expect("escribe run.sh");
    repo
}

/// Entrega con `instalar`, `correr` y espera de 1 s, más dos escenarios: `ok` y `falla`.
fn entrega_con(instalar: &str, correr: &str) -> String {
    format!(
        "[entrega]\ninstalar = \"{instalar}\"\ncorrer = \"{correr}\"\n\
         espera_segundos = 1\narchivos = [\"README.md\", \"Dockerfile\"]\n\n\
         [[escenario]]\nid = \"ok\"\ncmd = \"exit 0\"\n\n\
         [[escenario]]\nid = \"falla\"\ncmd = \"exit 1\"\n"
    )
}

#[test]
fn entrega_podman_escenarios_ok_y_falla_con_el_producto_vivo() {
    asegurar_imagen(ALPINE);
    let repo = repo_con_run("sleep 60\n");
    let tarea = tarea_con(&entrega_con("echo instalado", "sh run.sh"));
    let inicio = Instant::now();
    let r = sonda_entrega(&ejecutor(repo.path()), repo.path(), &tarea);
    let tardo = inicio.elapsed();
    let esperado: BTreeMap<String, bool> =
        [("ok".to_string(), true), ("falla".to_string(), false)].into();
    assert_eq!(r.escenarios, esperado);
    assert_eq!(r.sonda.motivo, None);
    // El script mata `correr`: el contenedor sale antes de que el `sleep 60` termine.
    assert!(
        tardo < Duration::from_secs(50),
        "el contenedor no salió solo: tardó {tardo:?}"
    );
    assert!(r.sonda.hallazgos.iter().any(|h| h.cita == "falla"));
}

#[test]
fn entrega_podman_correr_que_termina_enseguida_da_no_arranca() {
    asegurar_imagen(ALPINE);
    let repo = repo_con_run("");
    let tarea = tarea_con(&entrega_con("echo instalado", "echo adios; exit 1"));
    let r = sonda_entrega(&ejecutor(repo.path()), repo.path(), &tarea);
    let h = r
        .sonda
        .hallazgos
        .iter()
        .find(|h| h.detalle.contains("no arranca"))
        .expect("hallazgo de no arranca");
    assert_eq!((h.severidad, h.cita.as_str()), (3, "adios"));
}

#[test]
fn entrega_podman_instalar_que_falla_da_no_instala() {
    asegurar_imagen(ALPINE);
    let repo = repo_con_run("");
    let tarea = tarea_con(&entrega_con("echo roto; exit 2", "sleep 60"));
    let r = sonda_entrega(&ejecutor(repo.path()), repo.path(), &tarea);
    let h = r
        .sonda
        .hallazgos
        .iter()
        .find(|h| h.detalle.contains("no instala"))
        .expect("hallazgo de no instala");
    assert_eq!((h.severidad, h.cita.as_str()), (3, "roto"));
    assert!(r.escenarios.values().all(|v| !v), "ningún escenario corrió");
}

#[test]
fn entrega_podman_sin_entrega_corre_escenarios_sobre_repo() {
    asegurar_imagen(ALPINE);
    let repo = repo_con_run("");
    let tarea = tarea_con(
        "[[escenario]]\nid = \"ve\"\ncmd = \"test -f run.sh\"\n\n\
         [[escenario]]\nid = \"no\"\ncmd = \"test -f nope\"\n",
    );
    let r = sonda_entrega(&ejecutor(repo.path()), repo.path(), &tarea);
    assert_eq!(r.escenarios.get("ve"), Some(&true));
    assert_eq!(r.escenarios.get("no"), Some(&false));
    assert!(r.sonda.valor.is_some());
}

/// Ejecutor de mentira con los `.rc` dados. Sirve para la prueba pura de completitud.
struct Falso(SalidaHerramienta);

impl EjecutorHerramienta for Falso {
    fn ejecutar(&self, _cmd: &str) -> Result<SalidaHerramienta, ErrorOraculo> {
        Ok(self.0.clone())
    }
}

fn salida_con(archivos: &[(&str, &str)]) -> SalidaHerramienta {
    let mut s = SalidaHerramienta::default();
    for (nombre, texto) in archivos {
        s.archivos.insert((*nombre).into(), (*texto).into());
    }
    s
}

#[test]
fn entrega_escenario_fallido_deja_requisito_en_parcial() {
    let repo = tempfile::tempdir().expect("temp dir");
    fs::write(repo.path().join("README.md"), "x").expect("escribe");
    fs::write(repo.path().join("Dockerfile"), "FROM x\nUSER app\n").expect("escribe");
    let tarea = tarea_con(
        "[entrega]\ninstalar = \"make\"\ncorrer = \"./run\"\n\
         archivos = [\"README.md\", \"Dockerfile\"]\n\n\
         [[escenario]]\nid = \"set-get\"\ncmd = \"false\"\n\n\
         [[requisito]]\nid = \"R1\"\ntitulo = \"Guardar y leer\"\npeso = 3\n\
         pruebas = [\"tests/entrega.rs::test_a\"]\nescenarios = [\"set-get\"]\n",
    );
    let ej = Falso(salida_con(&[
        ("entrega-instalar.rc", "0\n"),
        ("entrega-correr.rc", "0\n"),
        ("entrega-escenario-set-get.rc", "1\n"),
    ]));
    let r = sonda_entrega(&ej, repo.path(), &tarea);
    assert_eq!(r.sonda.hallazgos.len(), 1, "solo el escenario falla");

    let mut tests = BTreeMap::new();
    tests.insert("tests/entrega.rs::test_a".to_string(), ResultadoTest::Pasa);
    let resultados = ResultadosTests {
        tests,
        inestables: Default::default(),
    };
    let estados = evaluar_requisitos(
        &tarea.requisito,
        &resultados,
        &r.escenarios,
        &[],
        &[],
        &BTreeMap::new(),
    );
    assert_eq!(estados[0].nivel, Some(2));
    assert_eq!(estados[0].cumplimiento, Cumplimiento::Parcial);
    assert_eq!(r.sonda.hallazgos[0].dimension, Dimension::Entrega);
    assert_eq!(r.sonda.hallazgos[0].fuente, Fuente::Sonda("entrega".into()));
}

#[test]
fn entrega_podman_log_de_correr_queda_en_la_cola_de_40_lineas() {
    asegurar_imagen(ALPINE);
    let repo = repo_con_run("");
    let tarea = tarea_con(&entrega_con("echo instalado", "seq 1 20000; sleep 60"));
    let ej = ejecutor(repo.path());
    let salida = ej
        .ejecutar(&root::oracle::entrega::construir_script(&tarea))
        .expect("corre");
    let log = salida
        .archivos
        .get("entrega-correr.log")
        .expect("hay log de correr");
    assert_eq!(log.lines().count(), 40, "solo la cola llega a /salida");
    assert!(log.contains("20000"));
}

#[test]
fn entrega_podman_escenario_fallido_cita_su_ultima_linea() {
    asegurar_imagen(ALPINE);
    let repo = repo_con_run("");
    let tarea = tarea_con(
        "[[escenario]]\nid = \"login\"\ncmd = \"echo paso 1; echo credencial rechazada; exit 1\"\n",
    );
    let r = sonda_entrega(&ejecutor(repo.path()), repo.path(), &tarea);
    assert_eq!(r.escenarios.get("login"), Some(&false));
    assert_eq!(r.sonda.hallazgos[0].cita, "credencial rechazada");
}
