//! Entrega y escenarios (Tarea 9d). Los de podman corren contra alpine sin red: la imagen debe
//! existir localmente. El resto es puro.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use root::oracle::ErrorOraculo;
use root::oracle::descarga::{self, Preparacion, Problema, traer_imagenes_base};
use root::oracle::entrega::{
    LugarEscenario, ResultadoDockerfile, construir_dockerfile, entregar, sonda_entrega,
};
use root::oracle::perfiles::deduplicar_hallazgos;
use root::oracle::tarea::Tarea;
use root::tipos::Dimension;

const ALPINE: &str = "docker.io/library/alpine:3.20";

/// Falla con un mensaje claro si la imagen no está. Nunca la baja: el test no tiene red.
fn asegurar_imagen(imagen: &str) {
    let existe = Command::new("podman")
        .args(["image", "exists", imagen])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    assert!(existe, "falta la imagen {imagen} en podman");
}

/// Tarea de lenguaje `shell`, con `extra` al final del `tarea.toml`.
fn tarea_con(extra: &str) -> Tarea {
    // La carpeta sobrevive al test: `entregar` exige que `tarea.dir` exista (Tarea 7i).
    let dir = tempfile::Builder::new()
        .prefix("tarea-")
        .tempdir_in(base_9d2())
        .expect("temp dir")
        .keep();
    let texto = format!(
        "nombre = \"x\"\nlenguaje = \"shell\"\nimagen = \"docker.io/library/alpine:3.20\"\nspec = \"s\"\n\
         [contrato]\ncmd = \"c\"\n[oculto]\ncmd = \"o\"\n{extra}"
    );
    fs::write(dir.join("tarea.toml"), texto).expect("escribe tarea.toml");
    Tarea::cargar_de(&dir).expect("carga la tarea")
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
    root::saltar_sin_podman!();
    asegurar_imagen(ALPINE);
    let repo = repo_con_run("sleep 60\n");
    let tarea = tarea_con(&entrega_con("echo instalado", "sh run.sh"));
    let inicio = Instant::now();
    let r = sonda_entrega(repo.path(), &tarea, &base_9d2());
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
    root::saltar_sin_podman!();
    asegurar_imagen(ALPINE);
    let repo = repo_con_run("");
    let tarea = tarea_con(&entrega_con("echo instalado", "echo adios; exit 1"));
    let r = sonda_entrega(repo.path(), &tarea, &base_9d2());
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
    root::saltar_sin_podman!();
    asegurar_imagen(ALPINE);
    let repo = repo_con_run("");
    let tarea = tarea_con(&entrega_con("echo roto; exit 2", "sleep 60"));
    let r = sonda_entrega(repo.path(), &tarea, &base_9d2());
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
    root::saltar_sin_podman!();
    asegurar_imagen(ALPINE);
    let repo = repo_con_run("");
    let tarea = tarea_con(
        "[[escenario]]\nid = \"ve\"\ncmd = \"test -f run.sh\"\n\n\
         [[escenario]]\nid = \"no\"\ncmd = \"test -f nope\"\n",
    );
    let r = sonda_entrega(repo.path(), &tarea, &base_9d2());
    assert_eq!(r.escenarios.get("ve"), Some(&true));
    assert_eq!(r.escenarios.get("no"), Some(&false));
    assert!(r.sonda.valor.is_some());
}

#[test]
fn entrega_podman_correr_que_muere_tras_mucha_salida_cita_su_ultima_linea() {
    root::saltar_sin_podman!();
    asegurar_imagen(ALPINE);
    let repo = repo_con_run("");
    let tarea = tarea_con(&entrega_con("echo instalado", "seq 1 20000; exit 1"));
    let r = sonda_entrega(repo.path(), &tarea, &base_9d2());
    let h = r
        .sonda
        .hallazgos
        .iter()
        .find(|h| h.detalle.contains("no arranca"))
        .expect("hallazgo de no arranca");
    assert_eq!(
        h.cita, "20000",
        "la cola del hijo llega con su última línea"
    );
}

#[test]
fn entrega_podman_escenario_fallido_cita_su_ultima_linea() {
    root::saltar_sin_podman!();
    asegurar_imagen(ALPINE);
    let repo = repo_con_run("");
    let tarea = tarea_con(
        "[[escenario]]\nid = \"login\"\ncmd = \"echo paso 1; echo credencial rechazada; exit 1\"\n",
    );
    let r = sonda_entrega(repo.path(), &tarea, &base_9d2());
    assert_eq!(r.escenarios.get("login"), Some(&false));
    assert_eq!(r.sonda.hallazgos[0].cita, "credencial rechazada");
}

/// Un proceso de fondo que escribe `0` en los `.rc` de `/salida` no hace pasar un escenario que falla.
/// Su salida va a `/dev/null`: así no retiene el pipe de `podman exec` que espera Root.
#[test]
fn entrega_podman_proceso_de_fondo_que_escribe_rc_no_pasa_un_escenario_que_falla() {
    root::saltar_sin_podman!();
    asegurar_imagen(ALPINE);
    let repo = repo_con_run("");
    let fondo = "(while true; do for f in /salida/entrega-*.rc; do echo 0 > $f; done; \
                  sleep 0.1; done) >/dev/null 2>&1 & echo instalado";
    let tarea = tarea_con(&entrega_con(fondo, "sleep 60"));
    let r = sonda_entrega(repo.path(), &tarea, &base_9d2());
    assert_eq!(r.escenarios.get("falla"), Some(&false), "{r:?}");
    assert_eq!(r.escenarios.get("ok"), Some(&true), "{r:?}");
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
        let destino = repo.path().join(ruta);
        fs::create_dir_all(destino.parent().expect("con carpeta")).expect("crea carpetas");
        fs::write(destino, texto).expect("escribe el archivo");
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
    root::saltar_sin_podman!();
    asegurar_imagen(ALPINE);
    let _serie = serie();
    let repo = dockerfile_9d2(&format!(
        "{FROM_ALPINE}COPY hola.txt /hola.txt\nUSER 1000\n"
    ));
    let home = home_9d2();
    let r = construir_dockerfile(repo.path(), home.path(), &tarea_con(""), &[]).expect("construye");
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
    root::saltar_sin_podman!();
    asegurar_imagen(ALPINE);
    let _serie = serie();
    let repo = dockerfile_9d2(&format!("{FROM_ALPINE}COPY hola.txt /hola.txt\n"));
    let home = home_9d2();
    let tarea = tarea_con("");
    let r = construir_dockerfile(repo.path(), home.path(), &tarea, &[]).expect("construye");
    assert!(r.construida, "{r:?}");
    assert_eq!(r.corre_sin_root, Some(false), "{r:?}");
    // La regla de root la emite la sonda estática. La construcción no la repite.
    let estatica = sonda_entrega(repo.path(), &tarea, &base_9d2())
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

/// Un `RUN` que pide red se repite con red (Tarea 9d3). La laptop puede tener red o no: los dos
/// desenlaces válidos son construir con red (hallazgo de severidad 1) o quedar pendiente con su línea.
fn afirma_repeticion_con_red(r: &ResultadoDockerfile, linea: u32, pista: &str) {
    assert!(
        !r.hallazgos
            .iter()
            .any(|h| h.detalle.contains("la imagen no construye")),
        "{:?}",
        r.hallazgos
    );
    if r.con_red {
        assert!(r.construida, "{r:?}");
        assert!(r.pendientes.is_empty(), "{:?}", r.pendientes);
        let de_red: Vec<_> = r
            .hallazgos
            .iter()
            .filter(|h| h.detalle == "la imagen necesita red para construirse")
            .collect();
        assert_eq!(de_red.len(), 1, "{:?}", r.hallazgos);
        assert_eq!((de_red[0].severidad, de_red[0].linea), (1, linea));
    } else {
        assert!(!r.construida, "{r:?}");
        assert!(r.hallazgos.is_empty(), "{:?}", r.hallazgos);
        assert_eq!(r.pendientes.len(), 1, "{:?}", r.pendientes);
        let motivo = &r.pendientes[0].motivo;
        assert!(
            motivo.contains(&format!("línea {linea}")) && motivo.contains(pista),
            "{motivo}"
        );
        assert_eq!(r.corre_sin_root, None);
    }
}

#[test]
fn entrega_build_run_apk_sin_red_se_repite_con_red_o_queda_pendiente() {
    root::saltar_sin_podman!();
    asegurar_imagen(ALPINE);
    let _serie = serie();
    let repo = dockerfile_9d2(&format!("{FROM_ALPINE}RUN apk add --no-cache curl\n"));
    let home = home_9d2();
    let r = construir_dockerfile(repo.path(), home.path(), &tarea_con(""), &[]).expect("construye");
    afirma_repeticion_con_red(&r, 2, "apk");
    assert!(
        imagenes_temporales().is_empty(),
        "queda una imagen temporal"
    );
}

/// Un `RUN` de red que falla en la 2ª etapa queda pendiente en su línea. La 1ª etapa tiene pasos
/// previos (`WORKDIR`, `RUN`) que antes se mapeaban por índice al paso 3/3 y daban un hallazgo.
#[test]
fn entrega_build_run_de_red_en_segunda_etapa_queda_pendiente_en_su_linea() {
    root::saltar_sin_podman!();
    asegurar_imagen(ALPINE);
    let _serie = serie();
    let repo = dockerfile_9d2(
        "FROM docker.io/library/alpine:3.20 AS base\nWORKDIR /app\nRUN echo listo > /app/x\n\
         FROM docker.io/library/alpine:3.20\nWORKDIR /srv\nRUN wget -q http://example.invalid\n",
    );
    let home = home_9d2();
    let r = construir_dockerfile(repo.path(), home.path(), &tarea_con(""), &[]).expect("construye");
    assert!(r.hallazgos.is_empty(), "{:?}", r.hallazgos);
    assert!(!r.construida, "{r:?}");
    assert_eq!(r.pendientes.len(), 1, "{:?}", r.pendientes);
    let motivo = &r.pendientes[0].motivo;
    assert!(
        motivo.contains("línea 6") && motivo.contains("wget"),
        "{motivo}"
    );
    assert!(
        imagenes_temporales().is_empty(),
        "queda una imagen temporal"
    );
}

/// Un `ARG` antes del primer `FROM` no corre el índice: buildah no lo cuenta como paso.
#[test]
fn entrega_build_arg_antes_del_from_ubica_el_run_de_red() {
    root::saltar_sin_podman!();
    asegurar_imagen(ALPINE);
    let _serie = serie();
    let repo = dockerfile_9d2(
        "ARG VERSION=3.20\nFROM docker.io/library/alpine:3.20\nRUN echo listo > /x\n\
         RUN wget -q http://example.invalid\n",
    );
    let home = home_9d2();
    let r = construir_dockerfile(repo.path(), home.path(), &tarea_con(""), &[]).expect("construye");
    assert!(r.hallazgos.is_empty(), "{:?}", r.hallazgos);
    assert!(!r.construida, "{r:?}");
    assert_eq!(r.pendientes.len(), 1, "{:?}", r.pendientes);
    let motivo = &r.pendientes[0].motivo;
    assert!(
        motivo.contains("línea 4") && motivo.contains("wget"),
        "{motivo}"
    );
    assert!(
        imagenes_temporales().is_empty(),
        "queda una imagen temporal"
    );
}

#[test]
fn entrega_build_run_false_da_hallazgo_de_imagen_que_no_construye() {
    root::saltar_sin_podman!();
    asegurar_imagen(ALPINE);
    let _serie = serie();
    let repo = dockerfile_9d2(&format!("{FROM_ALPINE}RUN false\n"));
    let home = home_9d2();
    let r = construir_dockerfile(repo.path(), home.path(), &tarea_con(""), &[]).expect("construye");
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
    root::saltar_sin_podman!();
    asegurar_imagen(ALPINE);
    let _serie = serie();
    let repo = dockerfile_9d2(&format!("{FROM_ALPINE}COPY no-existe /x\n"));
    let home = home_9d2();
    let r = construir_dockerfile(repo.path(), home.path(), &tarea_con(""), &[]).expect("construye");
    assert!(r.pendientes.is_empty(), "{:?}", r.pendientes);
    assert_eq!(r.hallazgos.len(), 1, "{:?}", r.hallazgos);
    assert_eq!(r.hallazgos[0].linea, 2);
}

#[test]
fn entrega_build_wget_sin_red_se_repite_con_red_o_queda_pendiente() {
    root::saltar_sin_podman!();
    asegurar_imagen(ALPINE);
    let _serie = serie();
    let repo = dockerfile_9d2(&format!(
        "{FROM_ALPINE}RUN wget -q http://example.com -O /x\n"
    ));
    let home = home_9d2();
    let r = construir_dockerfile(repo.path(), home.path(), &tarea_con(""), &[]).expect("construye");
    afirma_repeticion_con_red(&r, 2, "wget");
}

#[test]
fn entrega_build_imagen_base_ausente_localmente_es_pendiente_no_infra() {
    root::saltar_sin_podman!();
    let _serie = serie();
    let repo = dockerfile_9d2("FROM localhost/no-existe:9\nUSER 1000\n");
    let home = home_9d2();
    let r = construir_dockerfile(repo.path(), home.path(), &tarea_con(""), &[])
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
    root::saltar_sin_podman!();
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
fn entrega_pull_from_con_arg_se_resuelve_y_usa_la_imagen_local() {
    root::saltar_sin_podman!();
    let _serie = serie();
    let repo = repo_9d2(&[(
        "Dockerfile",
        "ARG BASE=docker.io/library/alpine:3.20\nFROM ${BASE}\n",
    )]);
    let imagenes = traer_imagenes_base(repo.path()).expect("la imagen es local: no hace falta red");
    assert_eq!(imagenes.len(), 1, "{imagenes:?}");
    assert_eq!(
        imagenes[0].imagen, "${BASE}",
        "el Gate lista el FROM escrito"
    );
    assert_eq!(imagenes[0].resuelta.as_deref(), Some(ALPINE));
    assert_eq!(imagenes[0].sin_resolver, None, "{:?}", imagenes[0]);
    assert!(imagenes[0].digest.is_some(), "{:?}", imagenes[0]);
}

/// Etiqueta temporal de podman, borrada al salir de la prueba aunque falle.
struct EtiquetaTemporal(String);

impl Drop for EtiquetaTemporal {
    fn drop(&mut self) {
        let _ = Command::new("podman").args(["rmi", "-f", &self.0]).output();
    }
}

#[test]
fn entrega_imagen_base_ya_local_no_se_descarga() {
    root::saltar_sin_podman!();
    let _serie = serie();
    // El registro `localhost:1` no responde. Si la fase intentara un pull, daría infraestructura.
    let tag = "localhost:1/root-entrega-local:1";
    let etiquetada = Command::new("podman")
        .args(["tag", ALPINE, tag])
        .status()
        .expect("corre podman")
        .success();
    assert!(etiquetada, "no se pudo etiquetar alpine");
    let _limpieza = EtiquetaTemporal(tag.into());
    let repo = repo_9d2(&[("Dockerfile", &format!("FROM {tag}\nUSER 1000\n"))]);
    let imagenes = traer_imagenes_base(repo.path()).expect("la fase no falla");
    assert_eq!(imagenes.len(), 1, "{imagenes:?}");
    assert!(!imagenes[0].descargada, "{:?}", imagenes[0]);
    assert_eq!(imagenes[0].problema, None, "{:?}", imagenes[0]);
    assert!(imagenes[0].digest.is_some(), "{:?}", imagenes[0]);
}

#[test]
fn entrega_pull_imagen_inexistente_se_marca_no_existe() {
    root::saltar_sin_podman!();
    let _serie = serie();
    let repo = repo_9d2(&[(
        "Dockerfile",
        "FROM docker.io/library/root-no-existe-xyz:1\n",
    )]);
    let imagenes = traer_imagenes_base(repo.path()).expect("la ausencia no es infraestructura");
    assert_eq!(imagenes.len(), 1, "{imagenes:?}");
    assert_eq!(
        imagenes[0].problema,
        Some(Problema::ImagenInaccesible),
        "{:?}",
        imagenes[0]
    );
    assert_eq!(imagenes[0].digest, None);
}

#[test]
fn entrega_pull_sin_red_no_corta_y_queda_como_problema_de_infraestructura() {
    root::saltar_sin_podman!();
    let _serie = serie();
    let repo = repo_9d2(&[("Dockerfile", "FROM localhost:1/alpine:3.20\n")]);
    let imagenes =
        traer_imagenes_base(repo.path()).expect("un pull fallido no es error de la fase");
    assert_eq!(imagenes.len(), 1, "{imagenes:?}");
    assert!(
        matches!(imagenes[0].problema, Some(Problema::Infraestructura(_))),
        "{:?}",
        imagenes[0]
    );
    assert!(!imagenes[0].descargada, "{:?}", imagenes[0]);
}

#[test]
fn entrega_pull_que_falla_no_corta_preparar() {
    root::saltar_sin_podman!();
    let _serie = serie();
    let repo = repo_9d2(&[("Dockerfile", "FROM localhost:1/x:1\nUSER 1000\n")]);
    let home = home_9d2();
    let tarea = tarea_con("");
    let (p, imagenes) =
        descarga::preparar_con_imagenes(repo.path(), &tarea, home.path(), LIMITE_9D2)
            .expect("preparar no corta por un pull fallido");
    assert!(matches!(p, Preparacion::Lista { .. }), "{p:?}");
    assert_eq!(imagenes.len(), 1, "{imagenes:?}");
    assert!(
        matches!(imagenes[0].problema, Some(Problema::Infraestructura(_))),
        "{:?}",
        imagenes[0]
    );
    let gates = gates_descarga(home.path(), &tarea.nombre);
    assert_eq!(gates.len(), 1, "{gates:?}");
    let problema = gates[0]["extra"]["imagenes"][0]["problema"]
        .as_str()
        .unwrap_or_default();
    assert!(
        problema.starts_with("infraestructura: "),
        "el Gate registra la falla: {problema}"
    );
}

#[test]
fn entrega_pull_que_falla_da_sin_veredicto_de_infraestructura_y_el_resto_da_veredicto() {
    root::saltar_sin_podman!();
    let _serie = serie();
    let repo = repo_9d2(&[
        ("README.md", "# x\n"),
        ("Dockerfile", "FROM localhost:1/x:1\nUSER 1000\n"),
        ("run.sh", ""),
    ]);
    let home = home_9d2();
    let tarea = tarea_con(&entrega_con("echo instalado", "sleep 60"));
    let (_, imagenes) =
        descarga::preparar_con_imagenes(repo.path(), &tarea, home.path(), LIMITE_9D2)
            .expect("prepara");
    let r = entregar(repo.path(), &tarea, home.path(), &imagenes).expect("la evaluación sigue");
    assert_eq!(r.sin_veredicto.len(), 1, "{:?}", r.sin_veredicto);
    assert_eq!(
        r.sin_veredicto[0].causa,
        "infraestructura: no se pudo bajar la imagen base localhost:1/x:1"
    );
    assert_eq!(r.sin_veredicto[0].linea, 1);
    assert_eq!(
        r.sonda.valor, None,
        "sin construcción, la entrega no da verde"
    );
    // El resto de la evaluación da su veredicto: los escenarios corren en el contenedor de evaluación.
    let esperado: BTreeMap<String, bool> =
        [("ok".to_string(), true), ("falla".to_string(), false)].into();
    assert_eq!(r.escenarios, esperado);
    assert!(
        r.lugar_escenarios
            .values()
            .all(|l| *l == LugarEscenario::ContenedorEvaluacion),
        "{:?}",
        r.lugar_escenarios
    );
}

#[test]
fn entrega_gate_descarga_incluye_imagenes_con_su_digest() {
    root::saltar_sin_podman!();
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
fn entrega_preparar_imagen_inexistente_no_da_hallazgo_al_producto() {
    root::saltar_sin_podman!();
    let _serie = serie();
    let repo = repo_9d2(&[(
        "Dockerfile",
        "FROM docker.io/library/root-no-existe-xyz:1\n",
    )]);
    let home = home_9d2();
    let p =
        descarga::preparar(repo.path(), &tarea_con(""), home.path(), LIMITE_9D2).expect("prepara");
    let Preparacion::Lista { hallazgos, .. } = p else {
        panic!("la evaluación sigue: la falla es de la construcción, no del producto: {p:?}");
    };
    assert!(hallazgos.is_empty(), "{hallazgos:?}");
}

#[test]
fn entrega_imagen_inexistente_o_privada_da_sin_veredicto_sin_hallazgo() {
    root::saltar_sin_podman!();
    let _serie = serie();
    let repo = repo_9d2(&[(
        "Dockerfile",
        "FROM docker.io/library/root-no-existe-xyz:1\nUSER 1000\n",
    )]);
    let home = home_9d2();
    let imagenes = traer_imagenes_base(repo.path()).expect("la ausencia no es infraestructura");
    let r = construir_dockerfile(repo.path(), home.path(), &tarea_con(""), &imagenes)
        .expect("construye");
    assert_eq!(r.sin_veredicto.len(), 1, "{:?}", r.sin_veredicto);
    let motivo = &r.sin_veredicto[0];
    assert_eq!(
        motivo.causa,
        "imagen base inaccesible: no existe o es privada"
    );
    assert_eq!(motivo.linea, 1);
    assert_eq!(
        motivo.imagen.as_deref(),
        Some("docker.io/library/root-no-existe-xyz:1")
    );
    assert!(r.hallazgos.is_empty(), "{:?}", r.hallazgos);
    assert!(r.pendientes.is_empty(), "{:?}", r.pendientes);
    assert!(!r.construida);
}

#[test]
fn entrega_arg_base_construye_con_el_from_resuelto() {
    root::saltar_sin_podman!();
    asegurar_imagen(ALPINE);
    let _serie = serie();
    let repo = repo_9d2(&[
        (
            "Dockerfile",
            "ARG BASE=docker.io/library/alpine:3.20\nFROM ${BASE}\nCOPY hola.txt /hola.txt\nUSER 1000\n",
        ),
        ("hola.txt", "hola\n"),
    ]);
    let home = home_9d2();
    let imagenes = traer_imagenes_base(repo.path()).expect("la imagen es local");
    let r = construir_dockerfile(repo.path(), home.path(), &tarea_con(""), &imagenes)
        .expect("construye");
    assert!(r.sin_veredicto.is_empty(), "{:?}", r.sin_veredicto);
    assert!(r.construida, "{r:?}");
    assert_eq!(r.corre_sin_root, Some(true), "{r:?}");
    assert!(r.hallazgos.is_empty(), "{:?}", r.hallazgos);
    assert!(
        imagenes_temporales().is_empty(),
        "queda una imagen temporal"
    );
}

#[test]
fn entrega_from_con_variable_sin_valor_da_sin_veredicto_con_su_linea() {
    let _serie = serie();
    let repo = repo_9d2(&[("Dockerfile", "ARG OTRO=x\nFROM ${SIN_VALOR}\nUSER 1000\n")]);
    let home = home_9d2();
    let imagenes = traer_imagenes_base(repo.path()).expect("no necesita red");
    assert_eq!(
        imagenes[0].imagen, "${SIN_VALOR}",
        "el Gate lista el FROM escrito"
    );
    let r = construir_dockerfile(repo.path(), home.path(), &tarea_con(""), &imagenes)
        .expect("construye");
    assert_eq!(r.sin_veredicto.len(), 1, "{:?}", r.sin_veredicto);
    assert_eq!(r.sin_veredicto[0].causa, "imagen base sin resolver");
    assert_eq!(r.sin_veredicto[0].linea, 2);
    assert_eq!(r.sin_veredicto[0].imagen.as_deref(), Some("${SIN_VALOR}"));
    assert!(r.hallazgos.is_empty(), "{:?}", r.hallazgos);
    assert!(r.pendientes.is_empty(), "{:?}", r.pendientes);
    assert!(!r.construida);
}

/// Contenedores de escenario que quedan en podman.
fn contenedores_escenario() -> Vec<String> {
    let salida = Command::new("podman")
        .args(["ps", "-a", "--format", "{{.Names}}"])
        .output()
        .expect("corre podman ps");
    String::from_utf8_lossy(&salida.stdout)
        .lines()
        .filter(|l| l.contains("root-entrega-esc"))
        .map(str::to_string)
        .collect()
}

#[test]
fn entrega_escenario_corre_contra_la_imagen_construida() {
    root::saltar_sin_podman!();
    asegurar_imagen(ALPINE);
    let _serie = serie();
    let repo = dockerfile_9d2(&format!(
        "{FROM_ALPINE}COPY hola.txt /hola.txt\nUSER 1000\n"
    ));
    let home = home_9d2();
    let tarea = tarea_con("[[escenario]]\nid = \"hola\"\ncmd = \"test -f /hola.txt\"\n");
    let imagenes = descarga::traer_imagenes_base(repo.path()).expect("la imagen es local");
    let r = entregar(repo.path(), &tarea, home.path(), &imagenes).expect("entrega");
    assert_eq!(r.escenarios.get("hola"), Some(&true), "{r:?}");
    assert_eq!(
        r.lugar_escenarios.get("hola"),
        Some(&LugarEscenario::Imagen),
        "{r:?}"
    );
    assert!(r.sonda.hallazgos.is_empty(), "{:?}", r.sonda.hallazgos);
    assert!(
        contenedores_escenario().is_empty(),
        "queda un contenedor de escenario"
    );
    assert!(
        imagenes_temporales().is_empty(),
        "queda una imagen temporal"
    );
}

#[test]
fn entrega_escenario_sin_construir_corre_en_contenedor_de_evaluacion() {
    root::saltar_sin_podman!();
    asegurar_imagen(ALPINE);
    let _serie = serie();
    let repo = dockerfile_9d2(&format!("{FROM_ALPINE}RUN false\n"));
    let home = home_9d2();
    let tarea = tarea_con("[[escenario]]\nid = \"hola\"\ncmd = \"true\"\n");
    let imagenes = descarga::traer_imagenes_base(repo.path()).expect("la imagen es local");
    let r = entregar(repo.path(), &tarea, home.path(), &imagenes).expect("entrega");
    assert!(
        r.sonda
            .hallazgos
            .iter()
            .any(|h| h.detalle.contains("no construye")),
        "{:?}",
        r.sonda.hallazgos
    );
    assert_eq!(r.escenarios.get("hola"), Some(&true), "{r:?}");
    assert_eq!(
        r.lugar_escenarios.get("hola"),
        Some(&LugarEscenario::ContenedorEvaluacion),
        "{r:?}"
    );
    assert!(
        imagenes_temporales().is_empty(),
        "queda una imagen temporal"
    );
}

#[test]
fn entrega_escenario_con_red_queda_pendiente_y_no_false() {
    root::saltar_sin_podman!();
    asegurar_imagen(ALPINE);
    let _serie = serie();
    let repo = dockerfile_9d2(&format!(
        "{FROM_ALPINE}COPY hola.txt /hola.txt\nUSER 1000\n"
    ));
    let home = home_9d2();
    let tarea =
        tarea_con("[[escenario]]\nid = \"descarga\"\ncmd = \"wget -q http://example.com -O /x\"\n");
    let imagenes = descarga::traer_imagenes_base(repo.path()).expect("la imagen es local");
    let r = entregar(repo.path(), &tarea, home.path(), &imagenes).expect("entrega");
    assert_eq!(r.escenarios.get("descarga"), None, "{r:?}");
    assert_eq!(r.pendientes.len(), 1, "{:?}", r.pendientes);
    assert!(r.pendientes[0].motivo.contains("red"), "{:?}", r.pendientes);
    assert!(r.sonda.hallazgos.is_empty(), "{:?}", r.sonda.hallazgos);
    assert!(
        contenedores_escenario().is_empty(),
        "queda un contenedor de escenario"
    );
    assert!(
        imagenes_temporales().is_empty(),
        "queda una imagen temporal"
    );
}

// ---- Tarea 7i-A (ajuste): los escenarios usan el `tarea.dir` montado en /oraculo ----

/// Tarea cuyo `tarea.toml` vive en `dir`, que el test conserva vivo.
fn tarea_en(dir: &Path, extra: &str) -> Tarea {
    let texto = format!(
        "nombre = \"x\"\nlenguaje = \"shell\"\nimagen = \"docker.io/library/alpine:3.20\"\nspec = \"s\"\n\
         [contrato]\ncmd = \"c\"\n[oculto]\ncmd = \"o\"\n{extra}"
    );
    fs::write(dir.join("tarea.toml"), texto).expect("escribe tarea.toml");
    Tarea::cargar_de(dir).expect("carga la tarea")
}

#[test]
fn entrega_escenario_de_oraculo_corre_contra_la_imagen() {
    root::saltar_sin_podman!();
    use std::os::unix::fs::PermissionsExt;
    asegurar_imagen(ALPINE);
    let _serie = serie();
    let oraculo = tempfile::Builder::new()
        .prefix("entrega-oraculo-")
        .tempdir_in(base_9d2())
        .expect("carpeta de la tarea");
    fs::create_dir_all(oraculo.path().join("escenarios")).expect("crea escenarios/");
    let script = oraculo.path().join("escenarios/ok.sh");
    fs::write(&script, "#!/bin/sh\ntest -f /hola.txt\n").expect("escribe el script");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("permisos");
    let tarea = tarea_en(
        oraculo.path(),
        "[[escenario]]\nid = \"ok\"\ncmd = \"/oraculo/escenarios/ok.sh\"\n",
    );
    let repo = dockerfile_9d2(&format!(
        "{FROM_ALPINE}COPY hola.txt /hola.txt\nUSER 1000\n"
    ));
    let home = home_9d2();
    let imagenes = descarga::traer_imagenes_base(repo.path()).expect("la imagen es local");
    let r = entregar(repo.path(), &tarea, home.path(), &imagenes).expect("entrega");
    assert_eq!(r.escenarios.get("ok"), Some(&true), "{r:?}");
    assert_eq!(
        r.lugar_escenarios.get("ok"),
        Some(&LugarEscenario::Imagen),
        "{r:?}"
    );
    assert!(
        contenedores_escenario().is_empty(),
        "queda un contenedor de escenario"
    );
}

#[test]
fn entrega_imagen_construida_no_contiene_oraculo() {
    root::saltar_sin_podman!();
    asegurar_imagen(ALPINE);
    let _serie = serie();
    // Si el build viera /oraculo, esta RUN fallaría y construida sería falso.
    let repo = dockerfile_9d2(&format!(
        "{FROM_ALPINE}COPY hola.txt /hola.txt\nRUN test ! -e /oraculo\nUSER 1000\n"
    ));
    let home = home_9d2();
    let imagenes = descarga::traer_imagenes_base(repo.path()).expect("la imagen es local");
    let r = construir_dockerfile(repo.path(), home.path(), &tarea_con(""), &imagenes)
        .expect("construye");
    assert!(r.construida, "{r:?}");
    assert!(r.hallazgos.is_empty(), "{:?}", r.hallazgos);
}

#[test]
fn entrega_escenario_sin_timeout_en_la_imagen_no_sale_127() {
    root::saltar_sin_podman!();
    asegurar_imagen(ALPINE);
    let _serie = serie();
    // Se quita `timeout` de la imagen: el escenario debe correr igual, con el plazo de Root.
    let repo = dockerfile_9d2(&format!(
        "{FROM_ALPINE}COPY hola.txt /hola.txt\nRUN rm -f /usr/bin/timeout /bin/timeout\nUSER 1000\n"
    ));
    let oraculo = tempfile::Builder::new()
        .prefix("entrega-oraculo-")
        .tempdir_in(base_9d2())
        .expect("carpeta de la tarea");
    let tarea = tarea_en(
        oraculo.path(),
        "[[escenario]]\nid = \"hola\"\ncmd = \"test -f /hola.txt\"\n",
    );
    let home = home_9d2();
    let imagenes = descarga::traer_imagenes_base(repo.path()).expect("la imagen es local");
    let r = entregar(repo.path(), &tarea, home.path(), &imagenes).expect("entrega");
    assert_eq!(r.escenarios.get("hola"), Some(&true), "{r:?}");
    assert_eq!(
        r.lugar_escenarios.get("hola"),
        Some(&LugarEscenario::Imagen),
        "{r:?}"
    );
}

#[test]
fn entrega_escenario_sin_carpeta_de_tarea_da_infraestructura() {
    let _serie = serie();
    let repo = dockerfile_9d2(&format!("{FROM_ALPINE}USER 1000\n"));
    let oraculo = tempfile::Builder::new()
        .prefix("entrega-oraculo-")
        .tempdir_in(base_9d2())
        .expect("carpeta de la tarea");
    let tarea = tarea_en(
        oraculo.path(),
        "[[escenario]]\nid = \"hola\"\ncmd = \"true\"\n",
    );
    drop(oraculo);
    let home = home_9d2();
    let r = entregar(repo.path(), &tarea, home.path(), &[]);
    assert!(matches!(r, Err(ErrorOraculo::Infra(_))), "{r:?}");
}

#[test]
fn entrega_escenario_en_linea_sin_carpeta_de_escenarios_no_monta_nada() {
    root::saltar_sin_podman!();
    asegurar_imagen(ALPINE);
    let _serie = serie();
    // Un `cmd` en línea no usa `/oraculo`: no hace falta `escenarios/` y no se monta nada (M15).
    let repo = dockerfile_9d2(&format!("{FROM_ALPINE}USER 1000\n"));
    let tarea = tarea_con("[[escenario]]\nid = \"ok\"\ncmd = \"exit 0\"\n");
    assert!(!tarea.dir.join("escenarios").exists());
    let home = home_9d2();
    let imagenes = descarga::traer_imagenes_base(repo.path()).expect("la imagen es local");
    let r = entregar(repo.path(), &tarea, home.path(), &imagenes).expect("entrega sin Infra");
    assert_eq!(r.escenarios.get("ok"), Some(&true), "{r:?}");
    assert_eq!(
        r.lugar_escenarios.get("ok"),
        Some(&LugarEscenario::Imagen),
        "{r:?}"
    );
    assert!(
        contenedores_escenario().is_empty(),
        "queda un contenedor de escenario"
    );
}

#[test]
fn entrega_escenario_que_usa_oraculo_sin_carpeta_da_infraestructura() {
    // Guarda: el `cmd` usa `/oraculo`, así que sin `escenarios/` no hay nada que montar. Infra antes de construir.
    let repo = dockerfile_9d2(&format!("{FROM_ALPINE}USER 1000\n"));
    let tarea = tarea_con("[[escenario]]\nid = \"ok\"\ncmd = \"/oraculo/escenarios/ok.sh\"\n");
    let home = home_9d2();
    let r = entregar(repo.path(), &tarea, home.path(), &[]);
    match r {
        Err(ErrorOraculo::Infra(motivo)) => {
            assert!(motivo.contains("carpeta de escenarios"), "{motivo}");
        }
        otro => panic!("se esperaba Infra, salió {otro:?}"),
    }
}

#[test]
fn entrega_escenario_con_carpeta_monta_solo_escenarios() {
    // Guarda: con `escenarios/`, el contenedor ve solo esa carpeta. `tarea.toml` no entra (M15).
    root::saltar_sin_podman!();
    use std::os::unix::fs::PermissionsExt;
    asegurar_imagen(ALPINE);
    let _serie = serie();
    let oraculo = tempfile::Builder::new()
        .prefix("entrega-oraculo-")
        .tempdir_in(base_9d2())
        .expect("carpeta de la tarea");
    fs::create_dir_all(oraculo.path().join("escenarios")).expect("crea escenarios/");
    let script = oraculo.path().join("escenarios/ok.sh");
    fs::write(
        &script,
        "#!/bin/sh\ntest -d /oraculo/escenarios && test ! -e /oraculo/tarea.toml\n",
    )
    .expect("escribe el script");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("permisos");
    let tarea = tarea_en(
        oraculo.path(),
        "[[escenario]]\nid = \"ok\"\ncmd = \"/oraculo/escenarios/ok.sh\"\n",
    );
    let repo = dockerfile_9d2(&format!("{FROM_ALPINE}USER 1000\n"));
    let home = home_9d2();
    let imagenes = descarga::traer_imagenes_base(repo.path()).expect("la imagen es local");
    let r = entregar(repo.path(), &tarea, home.path(), &imagenes).expect("entrega");
    assert_eq!(r.escenarios.get("ok"), Some(&true), "{r:?}");
}

// ---- Tarea 7j: un solo Dockerfile, imagen sin `sh` y causa "pide credenciales" ----

/// Tarea con `containerfile` en la raíz del `tarea.toml` y `extra` al final.
fn tarea_con_containerfile(containerfile: &str, extra: &str) -> Tarea {
    let dir = tempfile::Builder::new()
        .prefix("tarea-")
        .tempdir_in(base_9d2())
        .expect("temp dir")
        .keep();
    let texto = format!(
        "containerfile = \"{containerfile}\"\nnombre = \"x\"\nlenguaje = \"shell\"\nimagen = \"docker.io/library/alpine:3.20\"\n\
         spec = \"s\"\n[contrato]\ncmd = \"c\"\n[oculto]\ncmd = \"o\"\n{extra}"
    );
    fs::write(dir.join("tarea.toml"), texto).expect("escribe tarea.toml");
    Tarea::cargar_de(&dir).expect("carga la tarea")
}

const ESCENARIO_MARCA_RAIZ: &str = "[[escenario]]\nid = \"marca\"\ncmd = \"test -f /marca-raiz\"\n";

#[test]
fn entrega_dockerfile_de_la_raiz_gana_y_solo_se_construye_uno() {
    root::saltar_sin_podman!();
    asegurar_imagen(ALPINE);
    let _serie = serie();
    // La raíz copia `hola.txt`. El otro Dockerfile copia un archivo que no existe: si se construyera, fallaría.
    let repo = repo_9d2(&[
        (
            "Dockerfile",
            "FROM docker.io/library/alpine:3.20\nCOPY hola.txt /hola.txt\nUSER 1000\n",
        ),
        ("hola.txt", "hola\n"),
        (
            "docker/otro.Dockerfile",
            "FROM docker.io/library/alpine:3.20\nCOPY no-existe /x\nUSER 1000\n",
        ),
    ]);
    let home = home_9d2();
    let tarea = tarea_con("[[escenario]]\nid = \"hola\"\ncmd = \"test -f /hola.txt\"\n");
    let imagenes = descarga::traer_imagenes_base(repo.path()).expect("la imagen es local");
    let c = construir_dockerfile(repo.path(), home.path(), &tarea, &imagenes).expect("construye");
    assert!(c.construida, "la raíz debe construir: {c:?}");
    assert!(
        c.hallazgos.is_empty(),
        "solo se construye la raíz: {:?}",
        c.hallazgos
    );
    let r = entregar(repo.path(), &tarea, home.path(), &imagenes).expect("entrega");
    // El rc del escenario lo pone el ejecutor falso. Lo real: `test -f /hola.txt` corre en la imagen
    // de la raíz, y si falla deja un hallazgo "escenario falló".
    assert_eq!(
        r.lugar_escenarios.get("hola"),
        Some(&LugarEscenario::Imagen),
        "{r:?}"
    );
    assert!(
        r.sonda
            .hallazgos
            .iter()
            .all(|h| !h.detalle.contains("escenario falló")),
        "el escenario no corrió sobre la imagen de la raíz: {:?}",
        r.sonda.hallazgos
    );
    assert!(
        r.sonda
            .hallazgos
            .iter()
            .all(|h| !h.detalle.contains("no construye")),
        "{:?}",
        r.sonda.hallazgos
    );
    assert!(
        imagenes_temporales().is_empty(),
        "queda una imagen temporal"
    );
}

#[test]
fn entrega_containerfile_gana_sobre_el_dockerfile_de_la_raiz() {
    root::saltar_sin_podman!();
    asegurar_imagen(ALPINE);
    let _serie = serie();
    let repo = repo_9d2(&[
        (
            "Dockerfile",
            "FROM docker.io/library/alpine:3.20\nRUN false\nUSER 1000\n",
        ),
        (
            "docker/app.Dockerfile",
            "FROM docker.io/library/alpine:3.20\nRUN touch /marca-raiz\nUSER 1000\n",
        ),
    ]);
    let home = home_9d2();
    let tarea = tarea_con_containerfile("docker/app.Dockerfile", ESCENARIO_MARCA_RAIZ);
    let imagenes = descarga::traer_imagenes_base(repo.path()).expect("la imagen es local");
    let r = entregar(repo.path(), &tarea, home.path(), &imagenes).expect("entrega");
    assert!(
        r.sonda
            .hallazgos
            .iter()
            .all(|h| !h.detalle.contains("no construye")),
        "el Dockerfile de la raíz no se construye: {:?}",
        r.sonda.hallazgos
    );
    assert_eq!(
        r.lugar_escenarios.get("marca"),
        Some(&LugarEscenario::Imagen),
        "{r:?}"
    );
    assert_eq!(r.escenarios.get("marca"), Some(&true), "{r:?}");
}

#[test]
fn entrega_containerfile_inexistente_avisa_y_usa_el_criterio_siguiente() {
    root::saltar_sin_podman!();
    asegurar_imagen(ALPINE);
    let _serie = serie();
    let repo = repo_9d2(&[(
        "Dockerfile",
        "FROM docker.io/library/alpine:3.20\nRUN touch /marca-raiz\nUSER 1000\n",
    )]);
    let home = home_9d2();
    let tarea = tarea_con_containerfile("no/existe.Dockerfile", ESCENARIO_MARCA_RAIZ);
    let imagenes = descarga::traer_imagenes_base(repo.path()).expect("la imagen es local");
    let r = entregar(repo.path(), &tarea, home.path(), &imagenes).expect("entrega");
    assert!(
        r.avisos
            .iter()
            .any(|a| a.contains("no/existe.Dockerfile") && a.contains("no existe")),
        "{:?}",
        r.avisos
    );
    assert_eq!(
        r.lugar_escenarios.get("marca"),
        Some(&LugarEscenario::Imagen),
        "el Dockerfile de la raíz sí se construye: {r:?}"
    );
}

#[test]
fn entrega_varios_dockerfile_sin_eleccion_no_construye_y_avisa() {
    root::saltar_sin_podman!();
    let _serie = serie();
    let repo = repo_9d2(&[
        (
            "docker/a.Dockerfile",
            "FROM docker.io/library/alpine:3.20\nRUN false\nUSER 1000\n",
        ),
        (
            "docker/b.Dockerfile",
            "FROM docker.io/library/alpine:3.20\nUSER 1000\n",
        ),
    ]);
    let home = home_9d2();
    let tarea = tarea_con("[[escenario]]\nid = \"hola\"\ncmd = \"true\"\n");
    let imagenes = descarga::traer_imagenes_base(repo.path()).expect("la imagen es local");
    let r = entregar(repo.path(), &tarea, home.path(), &imagenes).expect("entrega");
    assert!(
        r.avisos
            .iter()
            .any(|a| a == "varios Dockerfile, ninguno elegido"),
        "{:?}",
        r.avisos
    );
    assert!(
        r.avisos
            .iter()
            .any(|a| a.contains("docker/a.Dockerfile") && a.contains("docker/b.Dockerfile")),
        "el aviso lista los archivos: {:?}",
        r.avisos
    );
    // Ninguno se construye: el `RUN false` no da hallazgo de construcción.
    assert!(
        r.sonda
            .hallazgos
            .iter()
            .all(|h| !h.detalle.contains("no construye")),
        "{:?}",
        r.sonda.hallazgos
    );
    assert_eq!(
        r.lugar_escenarios.get("hola"),
        Some(&LugarEscenario::ContenedorEvaluacion),
        "{r:?}"
    );
    assert!(
        imagenes_temporales().is_empty(),
        "queda una imagen temporal"
    );
}

#[test]
fn entrega_scratch_construye_sin_red_y_corre_sin_root_por_inspect() {
    root::saltar_sin_podman!();
    let _serie = serie();
    let repo = dockerfile_9d2("FROM scratch\nCOPY hola.txt /hola.txt\nUSER 1000\n");
    let home = home_9d2();
    let r = construir_dockerfile(repo.path(), home.path(), &tarea_con(""), &[]).expect("construye");
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
fn entrega_scratch_escenarios_corren_en_contenedor_de_evaluacion_con_aviso() {
    root::saltar_sin_podman!();
    let _serie = serie();
    let repo = dockerfile_9d2("FROM scratch\nCOPY hola.txt /hola.txt\nUSER 1000\n");
    let home = home_9d2();
    let tarea = tarea_con("[[escenario]]\nid = \"hola\"\ncmd = \"true\"\n");
    let imagenes = descarga::traer_imagenes_base(repo.path()).expect("scratch no se baja");
    let r = entregar(repo.path(), &tarea, home.path(), &imagenes).expect("entrega");
    assert!(
        r.avisos
            .iter()
            .any(|a| a
                == "la imagen no tiene sh: los escenarios corren en el contenedor de evaluación"),
        "{:?}",
        r.avisos
    );
    assert_eq!(
        r.lugar_escenarios.get("hola"),
        Some(&LugarEscenario::ContenedorEvaluacion),
        "{r:?}"
    );
    assert_eq!(r.escenarios.get("hola"), Some(&true), "{r:?}");
    assert!(
        imagenes_temporales().is_empty(),
        "queda una imagen temporal"
    );
}

#[test]
fn descarga_pull_no_hace_pull_de_scratch() {
    let _serie = serie();
    let repo = repo_9d2(&[(
        "Dockerfile",
        "FROM scratch\nCOPY hola.txt /hola.txt\nUSER 1000\n",
    )]);
    let imagenes = traer_imagenes_base(repo.path()).expect("scratch no es un pull");
    assert!(imagenes.is_empty(), "{imagenes:?}");
}

#[test]
fn entrega_pide_credenciales_da_causa_unificada_en_la_construccion() {
    let _serie = serie();
    let repo = repo_9d2(&[("Dockerfile", "FROM 127.0.0.1:1/x:1\nUSER 1000\n")]);
    let home = home_9d2();
    let imagenes = [descarga::ImagenBase {
        archivo: "Dockerfile".into(),
        linea: 1,
        cita: "FROM 127.0.0.1:1/x:1".into(),
        imagen: "127.0.0.1:1/x:1".into(),
        resuelta: Some("127.0.0.1:1/x:1".into()),
        digest: None,
        descargada: false,
        sin_resolver: None,
        problema: Some(Problema::PideCredenciales),
    }];
    let r = construir_dockerfile(repo.path(), home.path(), &tarea_con(""), &imagenes)
        .expect("construye");
    assert_eq!(r.sin_veredicto.len(), 1, "{:?}", r.sin_veredicto);
    assert_eq!(
        r.sin_veredicto[0].causa,
        "imagen base: pide credenciales (Root no las pasa)"
    );
}

/// Servidor HTTP local que responde 401 con `WWW-Authenticate` a toda petición. Hilo suelto.
fn servidor_401() -> u16 {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    let escucha = TcpListener::bind("127.0.0.1:0").expect("escucha en loopback");
    let puerto = escucha.local_addr().expect("puerto").port();
    std::thread::spawn(move || {
        for flujo in escucha.incoming() {
            let Ok(mut flujo) = flujo else { continue };
            let _ = flujo.set_read_timeout(Some(Duration::from_secs(5)));
            let mut buf = [0u8; 4096];
            let _ = flujo.read(&mut buf);
            let _ = flujo.write_all(
                b"HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"root\"\r\n\
                  Content-Length: 0\r\nConnection: close\r\n\r\n",
            );
        }
    });
    puerto
}

#[test]
fn descarga_pull_401_de_podman_pide_credenciales() {
    root::saltar_sin_podman!();
    let puerto = servidor_401();
    // Root no baja con `--tls-verify=false`: el test solo mira qué dice podman ante un 401 real.
    let salida = Command::new("podman")
        .args([
            "pull",
            "--tls-verify=false",
            &format!("127.0.0.1:{puerto}/x:1"),
        ])
        .output()
        .expect("corre podman");
    assert!(!salida.status.success(), "un 401 no puede bajar la imagen");
    let texto = format!(
        "{}\n{}",
        String::from_utf8_lossy(&salida.stdout),
        String::from_utf8_lossy(&salida.stderr)
    );
    assert_eq!(
        descarga::clasificar_error_pull(&texto),
        Problema::PideCredenciales,
        "{texto}"
    );
}

/// Guarda que pone `CONTAINERS_REGISTRIES_CONF` para tratar un registro local como inseguro (HTTP).
/// Podman lo lee al correr; sin esto, `podman pull` pide HTTPS y el 401 nunca llega.
struct RegistroInseguro;

impl RegistroInseguro {
    fn con(puerto: u16) -> Self {
        let conf = base_9d2().join(format!("registries-{puerto}.conf"));
        fs::write(
            &conf,
            format!("[[registry]]\nlocation = \"127.0.0.1:{puerto}\"\ninsecure = true\n"),
        )
        .expect("escribe el registries.conf de prueba");
        // SAFETY: los tests con podman van de a uno (`serie()`) y esta guarda la quita al salir.
        unsafe { std::env::set_var("CONTAINERS_REGISTRIES_CONF", &conf) };
        RegistroInseguro
    }
}

impl Drop for RegistroInseguro {
    fn drop(&mut self) {
        // SAFETY: igual que en `con`: el test sigue dentro de `serie()`.
        unsafe { std::env::remove_var("CONTAINERS_REGISTRIES_CONF") };
    }
}

#[test]
fn entrega_pide_credenciales_no_da_hallazgo_al_producto() {
    root::saltar_sin_podman!();
    let _serie = serie();
    let puerto = servidor_401();
    let _registro = RegistroInseguro::con(puerto);
    let repo = repo_9d2(&[(
        "Dockerfile",
        &format!("FROM 127.0.0.1:{puerto}/x:1\nUSER 1000\n"),
    )]);
    let home = home_9d2();
    let (p, imagenes) =
        descarga::preparar_con_imagenes(repo.path(), &tarea_con(""), home.path(), LIMITE_9D2)
            .expect("prepara");
    assert_eq!(imagenes.len(), 1, "{imagenes:?}");
    assert_eq!(
        imagenes[0].problema,
        Some(Problema::PideCredenciales),
        "{imagenes:?}"
    );
    let Preparacion::SinVeredicto(hallazgos) = p else {
        panic!("un 401 da sin veredicto de la fase: {p:?}");
    };
    assert!(
        hallazgos.is_empty(),
        "un 401 no cobra al producto: {hallazgos:?}"
    );
}
