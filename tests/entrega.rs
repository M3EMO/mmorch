//! Entrega y escenarios (Tarea 9d). Los de podman corren contra alpine sin red: la imagen debe
//! existir localmente. El resto es puro.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use root::oracle::ErrorOraculo;
use root::oracle::configs::sondas_configs;
use root::oracle::descarga::{self, Preparacion, Problema, traer_imagenes_base};
use root::oracle::entrega::{construir_dockerfile, sonda_entrega};
use root::oracle::junit::ResultadoTest;
use root::oracle::perfiles::deduplicar_hallazgos;
use root::oracle::perfiles::herramientas::{
    EjecutorContenedor, EjecutorHerramienta, SalidaHerramienta,
};
use root::oracle::requisitos::{ResultadosTests, evaluar_requisitos};
use root::oracle::tarea::Tarea;
use root::tipos::{Cumplimiento, Dimension, Fuente, Hallazgo};

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

/// Ejecutor sin hadolint: `command -v` falla y la entrega sale bien.
struct SinHadolint;

impl EjecutorHerramienta for SinHadolint {
    fn ejecutar(&self, cmd: &str) -> Result<SalidaHerramienta, ErrorOraculo> {
        if cmd.starts_with("command -v") {
            return Ok(SalidaHerramienta {
                codigo: 1,
                ..Default::default()
            });
        }
        Ok(salida_con(&[
            ("entrega-instalar.rc", "0\n"),
            ("entrega-correr.rc", "0\n"),
        ]))
    }
}

#[test]
fn entrega_dockerfile_sin_user_da_un_solo_hallazgo_entre_configs_y_entrega() {
    let repo = tempfile::tempdir().expect("temp dir");
    fs::write(repo.path().join("README.md"), "# x\n").expect("escribe README");
    fs::write(
        repo.path().join("Dockerfile"),
        "FROM debian:bookworm-slim\n",
    )
    .expect("escribe Dockerfile");
    let tarea = tarea_con(
        "[entrega]\ninstalar = \"make\"\ncorrer = \"./run\"\n\
         archivos = [\"README.md\", \"Dockerfile\"]\n",
    );
    let ej = SinHadolint;
    let configs: Vec<Hallazgo> = sondas_configs(repo.path(), &ej, &tarea)
        .into_iter()
        .flat_map(|r| r.hallazgos)
        .collect();
    let entrega = sonda_entrega(&ej, repo.path(), &tarea).sonda.hallazgos;
    let todos: Vec<Hallazgo> = configs.into_iter().chain(entrega).collect();
    // La unión ya trae el hallazgo una sola vez: solo la sonda de entrega emite la regla de USER.
    assert_eq!(todos.len(), 1, "{todos:?}");
    // Red de seguridad: la sonda dueña es la que nombra la tabla de rúbricas.
    let hallazgos = deduplicar_hallazgos(todos);
    assert_eq!(hallazgos.len(), 1, "{hallazgos:?}");
    assert_eq!(hallazgos[0].dimension, Dimension::Entrega);
    assert!(hallazgos[0].detalle.contains("USER"));
}

// ---- Tarea 9d2: construir el Dockerfile sin red, y el paso con red (solo podman pull) ----

/// Los tests con podman corren de a uno: cuentan las imágenes `root-entrega-*` sin interferencias.
static SERIE_9D2: Mutex<()> = Mutex::new(());
const FROM_ALPINE: &str = "FROM docker.io/library/alpine:3.20\n";
const LIMITE_9D2: Duration = Duration::from_secs(900);

fn serie() -> MutexGuard<'static, ()> {
    SERIE_9D2.lock().unwrap_or_else(|e| e.into_inner())
}

/// Carpeta base dentro de `~/.cache/root-dbg/tmp`. Nunca `/tmp` ni el ROOT_HOME real.
fn base_9d2() -> PathBuf {
    let home = std::env::var_os("HOME").expect("HOME definido");
    let base = PathBuf::from(home).join(".cache/root-dbg/tmp");
    fs::create_dir_all(&base).expect("crea la carpeta de pruebas");
    base
}

/// `ROOT_HOME` temporal de un test.
fn home_9d2() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("entrega-home-")
        .tempdir_in(base_9d2())
        .expect("home temporal")
}

/// Repo temporal con los archivos dados, como pares (ruta, texto).
fn repo_9d2(archivos: &[(&str, &str)]) -> tempfile::TempDir {
    let repo = tempfile::Builder::new()
        .prefix("entrega-repo-")
        .tempdir_in(base_9d2())
        .expect("repo temporal");
    for (ruta, texto) in archivos {
        fs::write(repo.path().join(ruta), texto).expect("escribe el archivo");
    }
    repo
}

/// Repo con un Dockerfile de cuerpo dado y el `hola.txt` que usan los `COPY`.
fn dockerfile_9d2(cuerpo: &str) -> tempfile::TempDir {
    repo_9d2(&[("Dockerfile", cuerpo), ("hola.txt", "hola\n")])
}

/// Imágenes `root-entrega-*` que quedan en podman.
fn imagenes_temporales() -> Vec<String> {
    let salida = Command::new("podman")
        .args(["images", "--format", "{{.Repository}}:{{.Tag}}"])
        .output()
        .expect("corre podman images");
    String::from_utf8_lossy(&salida.stdout)
        .lines()
        .filter(|l| l.contains("root-entrega-"))
        .map(str::to_string)
        .collect()
}

/// Eventos `Gate` de la fase `descarga` de la tarea `nombre`.
fn gates_descarga(home: &Path, nombre: &str) -> Vec<serde_json::Value> {
    let Ok(texto) = fs::read_to_string(home.join("eventos.jsonl")) else {
        return Vec::new();
    };
    texto
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str::<serde_json::Value>(l).expect("línea JSON"))
        .filter(|v| {
            v["tipo"] == "gate"
                && v["corrida"] == format!("descarga-{nombre}")
                && v["fase"] == "descarga"
        })
        .collect()
}

#[test]
fn entrega_build_sin_run_construye_arranca_sin_root_y_borra_su_imagen() {
    asegurar_imagen(ALPINE);
    let _serie = serie();
    let repo = dockerfile_9d2(&format!(
        "{FROM_ALPINE}COPY hola.txt /hola.txt\nUSER 1000\n"
    ));
    let home = home_9d2();
    let r = construir_dockerfile(repo.path(), home.path(), &tarea_con("")).expect("construye");
    assert!(r.construida, "{r:?}");
    assert_eq!(r.corre_sin_root, Some(true), "{r:?}");
    assert!(r.hallazgos.is_empty(), "{:?}", r.hallazgos);
    assert!(r.pendientes.is_empty(), "{:?}", r.pendientes);
    assert!(
        imagenes_temporales().is_empty(),
        "queda una imagen temporal"
    );
}

#[test]
fn entrega_build_sin_user_corre_como_root_y_la_regla_no_se_duplica() {
    asegurar_imagen(ALPINE);
    let _serie = serie();
    let repo = dockerfile_9d2(&format!("{FROM_ALPINE}COPY hola.txt /hola.txt\n"));
    let home = home_9d2();
    let tarea = tarea_con("");
    let r = construir_dockerfile(repo.path(), home.path(), &tarea).expect("construye");
    assert!(r.construida, "{r:?}");
    assert_eq!(r.corre_sin_root, Some(false), "{r:?}");
    // La regla de root la emite la sonda estática. La construcción no la repite.
    let estatica = sonda_entrega(&ejecutor(repo.path()), repo.path(), &tarea)
        .sonda
        .hallazgos;
    assert_eq!(estatica.len(), 1, "{estatica:?}");
    let union = deduplicar_hallazgos(estatica.into_iter().chain(r.hallazgos).collect());
    assert_eq!(union.len(), 1, "{union:?}");
    assert!(
        imagenes_temporales().is_empty(),
        "queda una imagen temporal"
    );
}

#[test]
fn entrega_build_run_apk_sin_red_queda_pendiente_con_su_linea_y_sin_hallazgo() {
    asegurar_imagen(ALPINE);
    let _serie = serie();
    let repo = dockerfile_9d2(&format!("{FROM_ALPINE}RUN apk add --no-cache curl\n"));
    let home = home_9d2();
    let r = construir_dockerfile(repo.path(), home.path(), &tarea_con("")).expect("construye");
    assert!(
        r.hallazgos.is_empty(),
        "sin red no se cobra al producto: {:?}",
        r.hallazgos
    );
    assert_eq!(r.pendientes.len(), 1, "{:?}", r.pendientes);
    assert_eq!(r.pendientes[0].test, "dockerfile::build");
    let motivo = &r.pendientes[0].motivo;
    assert!(
        motivo.contains("línea 2") && motivo.contains("apk"),
        "{motivo}"
    );
    assert!(!r.construida);
    assert_eq!(r.corre_sin_root, None);
    assert!(
        imagenes_temporales().is_empty(),
        "queda una imagen temporal"
    );
}

#[test]
fn entrega_build_run_false_da_hallazgo_de_imagen_que_no_construye() {
    asegurar_imagen(ALPINE);
    let _serie = serie();
    let repo = dockerfile_9d2(&format!("{FROM_ALPINE}RUN false\n"));
    let home = home_9d2();
    let r = construir_dockerfile(repo.path(), home.path(), &tarea_con("")).expect("construye");
    assert!(r.pendientes.is_empty(), "{:?}", r.pendientes);
    assert_eq!(r.hallazgos.len(), 1, "{:?}", r.hallazgos);
    let h = &r.hallazgos[0];
    assert_eq!(h.dimension, Dimension::Entrega);
    assert_eq!(
        (h.severidad, h.linea, h.archivo.as_str()),
        (3, 2, "Dockerfile")
    );
    assert!(
        h.detalle.contains("la imagen no construye"),
        "{}",
        h.detalle
    );
}

#[test]
fn entrega_build_copy_inexistente_da_hallazgo_y_no_pendiente() {
    asegurar_imagen(ALPINE);
    let _serie = serie();
    let repo = dockerfile_9d2(&format!("{FROM_ALPINE}COPY no-existe /x\n"));
    let home = home_9d2();
    let r = construir_dockerfile(repo.path(), home.path(), &tarea_con("")).expect("construye");
    assert!(r.pendientes.is_empty(), "{:?}", r.pendientes);
    assert_eq!(r.hallazgos.len(), 1, "{:?}", r.hallazgos);
    assert_eq!(r.hallazgos[0].linea, 2);
}

#[test]
fn entrega_build_wget_sin_red_queda_pendiente() {
    asegurar_imagen(ALPINE);
    let _serie = serie();
    let repo = dockerfile_9d2(&format!(
        "{FROM_ALPINE}RUN wget -q http://example.com -O /x\n"
    ));
    let home = home_9d2();
    let r = construir_dockerfile(repo.path(), home.path(), &tarea_con("")).expect("construye");
    assert!(r.hallazgos.is_empty(), "{:?}", r.hallazgos);
    assert_eq!(r.pendientes.len(), 1, "{:?}", r.pendientes);
    assert!(
        r.pendientes[0].motivo.contains("línea 2"),
        "{:?}",
        r.pendientes
    );
}

#[test]
fn entrega_build_imagen_base_ausente_localmente_es_pendiente_no_infra() {
    let _serie = serie();
    let repo = dockerfile_9d2("FROM localhost/no-existe:9\nUSER 1000\n");
    let home = home_9d2();
    let r = construir_dockerfile(repo.path(), home.path(), &tarea_con(""))
        .expect("la ausencia local no es infraestructura");
    assert!(r.hallazgos.is_empty(), "{:?}", r.hallazgos);
    assert_eq!(r.pendientes.len(), 1, "{:?}", r.pendientes);
    assert!(
        r.pendientes[0].motivo.contains("imagen base no descargada"),
        "{:?}",
        r.pendientes
    );
    assert!(!r.construida);
}

#[test]
fn entrega_pull_baja_alpine_ignora_el_alias_y_no_ejecuta_run() {
    let _serie = serie();
    let repo = repo_9d2(&[(
        "Dockerfile",
        "FROM docker.io/library/alpine:3.20 AS base\nRUN touch /marca\nFROM base\n",
    )]);
    let imagenes = traer_imagenes_base(repo.path()).expect("baja con red");
    assert_eq!(imagenes.len(), 1, "{imagenes:?}");
    assert_eq!(imagenes[0].imagen, ALPINE);
    assert_eq!(imagenes[0].linea, 1);
    let digest = imagenes[0].digest.as_deref().unwrap_or_default();
    assert!(digest.starts_with("sha256:"), "{:?}", imagenes[0]);
    // El paso con red no corre ningún RUN: la marca no está en la imagen base bajada.
    let marca = Command::new("podman")
        .args([
            "run",
            "--rm",
            "--network",
            "none",
            "--pull=never",
            "--entrypoint",
            "ls",
            ALPINE,
            "/marca",
        ])
        .status()
        .expect("corre podman");
    assert!(
        !marca.success(),
        "la imagen base trae /marca: se ejecutó un RUN"
    );
}

#[test]
fn entrega_pull_from_con_variable_queda_sin_resolver_y_no_baja_nada() {
    let repo = repo_9d2(&[(
        "Dockerfile",
        "ARG BASE=docker.io/library/alpine:3.20\nFROM ${BASE}\n",
    )]);
    let imagenes = traer_imagenes_base(repo.path()).expect("no necesita red");
    assert_eq!(imagenes.len(), 1, "{imagenes:?}");
    assert_eq!(imagenes[0].imagen, "${BASE}");
    assert!(imagenes[0].sin_resolver.is_some(), "{:?}", imagenes[0]);
    assert_eq!(imagenes[0].digest, None);
}

#[test]
fn entrega_pull_imagen_inexistente_se_marca_no_existe() {
    let _serie = serie();
    let repo = repo_9d2(&[(
        "Dockerfile",
        "FROM docker.io/library/root-no-existe-xyz:1\n",
    )]);
    let imagenes = traer_imagenes_base(repo.path()).expect("la ausencia no es infraestructura");
    assert_eq!(imagenes.len(), 1, "{imagenes:?}");
    assert_eq!(
        imagenes[0].problema,
        Some(Problema::NoExiste),
        "{:?}",
        imagenes[0]
    );
    assert_eq!(imagenes[0].digest, None);
}

#[test]
fn entrega_pull_sin_red_falla_claro_con_infraestructura() {
    let _serie = serie();
    let repo = repo_9d2(&[("Dockerfile", "FROM localhost:1/alpine:3.20\n")]);
    let r = traer_imagenes_base(repo.path());
    assert!(matches!(r, Err(ErrorOraculo::Infra(_))), "{r:?}");
}

#[test]
fn entrega_gate_descarga_incluye_imagenes_con_su_digest() {
    let _serie = serie();
    let repo = repo_9d2(&[(
        "Dockerfile",
        "FROM docker.io/library/alpine:3.20\nUSER 1000\n",
    )]);
    let home = home_9d2();
    let tarea = tarea_con("");
    let p = descarga::preparar(repo.path(), &tarea, home.path(), LIMITE_9D2).expect("prepara");
    assert!(matches!(p, Preparacion::Lista { .. }), "{p:?}");
    let gates = gates_descarga(home.path(), &tarea.nombre);
    assert_eq!(gates.len(), 1, "{gates:?}");
    let imagenes = gates[0]["extra"]["imagenes"]
        .as_array()
        .expect("imagenes en el Gate");
    assert_eq!(imagenes[0]["imagen"], ALPINE, "{imagenes:?}");
    assert!(
        imagenes[0]["digest"]
            .as_str()
            .is_some_and(|d| d.starts_with("sha256:")),
        "{imagenes:?}"
    );
}

#[test]
fn entrega_preparar_imagen_inexistente_da_hallazgo_de_imagen_base() {
    let _serie = serie();
    let repo = repo_9d2(&[(
        "Dockerfile",
        "FROM docker.io/library/root-no-existe-xyz:1\n",
    )]);
    let home = home_9d2();
    let p =
        descarga::preparar(repo.path(), &tarea_con(""), home.path(), LIMITE_9D2).expect("prepara");
    let Preparacion::Lista { hallazgos, .. } = p else {
        panic!("la evaluación sigue con el hallazgo: {p:?}");
    };
    assert_eq!(hallazgos.len(), 1, "{hallazgos:?}");
    let h = &hallazgos[0];
    assert_eq!(
        (h.dimension, h.severidad, h.linea),
        (Dimension::Entrega, 3, 1)
    );
    assert!(
        h.detalle
            .contains("la imagen base no existe: docker.io/library/root-no-existe-xyz:1"),
        "{}",
        h.detalle
    );
}
