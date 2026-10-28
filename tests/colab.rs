//! `r00t oraculo pendientes --colab` contra el binario real y una CLI `colab` falsa.
//! Ningún test toca Colab: el falso registra cada invocación en `llamadas.log`.

use std::fs::{self, File};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const JUNIT_COLAB: &str =
    r#"<testsuite><testcase classname="tests.test_o" name="o2"/></testsuite>"#;
const ENTORNO_O2: &str = "[[entorno]]\ntest = \"test_o::o2\"\nmotivo = \"sin GPU local\"\n";
const MENSAJE_LOGIN: &str = "Falta la sesión de Colab. Corré el login OAuth en tu terminal (por ejemplo `colab new -s prueba --gpu T4`, completá el navegador, y después `colab stop -s prueba`) y volvé a lanzar este comando.";

/// Entorno de un test: `ROOT_HOME`, repo y la carpeta del falso.
struct Escenario {
    home: tempfile::TempDir,
    repo: tempfile::TempDir,
    falso: tempfile::TempDir,
}

impl Escenario {
    fn nuevo(entorno: &str) -> Self {
        let e = Escenario {
            home: tempfile::tempdir().expect("home"),
            repo: tempfile::tempdir().expect("repo"),
            falso: tempfile::tempdir().expect("falso"),
        };
        let dir = e.home.path().join("oraculos/tarea-prueba");
        fs::create_dir_all(dir.join("oculto")).expect("dir");
        fs::create_dir_all(dir.join("docs")).expect("dir");
        fs::write(dir.join("oculto/test_o.py"), "def test_o2(): pass\n").expect("escribe");
        fs::write(dir.join("docs/SPEC.md"), "# spec\n").expect("escribe");
        let texto = format!(
            "nombre = \"tarea-prueba\"\nlenguaje = \"shell\"\nimagen = \"x\"\nspec = \"docs/SPEC.md\"\n\n[contrato]\ncmd = 'true'\n\n[oculto]\ncmd = 'true'\n\n{entorno}"
        );
        fs::write(dir.join("tarea.toml"), texto).expect("escribe tarea.toml");
        fs::write(e.repo.path().join("a.txt"), "original").expect("escribe");
        e
    }

    fn llamadas(&self) -> Vec<String> {
        fs::read_to_string(self.falso.path().join("llamadas.log"))
            .unwrap_or_default()
            .lines()
            .map(String::from)
            .collect()
    }

    fn eventos_runtime(&self) -> Vec<serde_json::Value> {
        let texto = fs::read_to_string(self.home.path().join("eventos.jsonl")).unwrap_or_default();
        texto
            .lines()
            .map(|l| serde_json::from_str::<serde_json::Value>(l).expect("línea JSON"))
            .filter(|e| e["tipo"] == "gate" && e["fase"] == "colab-runtime")
            .collect()
    }

    fn colab_json(&self) -> PathBuf {
        self.home.path().join("evidencia/tarea-prueba/colab.json")
    }

    fn entorno_del_falso(&self) -> String {
        fs::read_to_string(self.falso.path().join("entorno.txt")).unwrap_or_default()
    }
}

/// Script `colab` falso. Responde según `gpu`, `auth` (`ok` o `falta`) y si `corre.py` falla.
fn falso(dir: &Path, gpu: &str, auth: &str, correr_falla: bool) -> PathBuf {
    let guion = format!(
        r#"#!/bin/sh
DIR='{dir}'
GPU='{gpu}'
AUTH='{auth}'
CORRER_FALLA='{falla}'
echo "$*" >> "$DIR/llamadas.log"
env >> "$DIR/entorno.txt"
case "$1" in
new)
  if [ "$AUTH" = cuelga ]; then echo $$ > "$DIR/pid.txt"; sleep 300; exit 0; fi
  if [ "$AUTH" = falta ]; then echo "Not logged in: authenticate with OAuth first" >&2; exit 1; fi
  exit 0 ;;
exec)
  f=""; prev=""
  for a in "$@"; do if [ "$prev" = "-f" ]; then f="$a"; fi; prev="$a"; done
  case "$f" in
    */gpu.py) echo "ROOT_GPU=$GPU" ;;
    */prepara.py) echo "ROOT_PREPARADO=1" ;;
    */prepara-rust.py) echo "ROOT_RUST=rustup" ;;
    */corre.py)
      if [ "$CORRER_FALLA" = 1 ]; then echo "boom" >&2; exit 1; fi
      echo "ROOT_CORRIDA_CODIGO=0" ;;
  esac
  exit 0 ;;
upload) tar -tzf "$4" > "$DIR/subida.txt" || exit 1; exit 0 ;;
download) printf '%s' '{junit}' > "$5" || exit 1; exit 0 ;;
stop) exit 0 ;;
*) exit 2 ;;
esac
"#,
        dir = dir.display(),
        gpu = gpu,
        auth = auth,
        falla = if correr_falla { 1 } else { 0 },
        junit = JUNIT_COLAB,
    );
    let ruta = dir.join("colab-falso");
    fs::write(&ruta, guion).expect("escribe el falso");
    fs::set_permissions(&ruta, fs::Permissions::from_mode(0o755)).expect("permisos");
    ruta
}

/// Corre `oraculo pendientes` con el falso como `ROOT_COLAB_BIN`. La clave de prueba
/// queda en el entorno de `r00t`: el falso no debe verla.
fn correr(e: &Escenario, binario: &Path, extra: &[&str]) -> Output {
    correr_con(e, binario, extra, None)
}

/// Como `correr`. `plazo` pone `ROOT_COLAB_PLAZO_PRIMER_PASO_S`, variable solo para tests.
fn correr_con(e: &Escenario, binario: &Path, extra: &[&str], plazo: Option<&str>) -> Output {
    orden(e, binario, extra, plazo)
        .output()
        .expect("no se pudo ejecutar r00t")
}

/// Orden de `r00t oraculo pendientes` con la CLI falsa, sin ejecutarla. `extra` va después de la tarea y el repo.
fn orden(e: &Escenario, binario: &Path, extra: &[&str], plazo: Option<&str>) -> Command {
    let repo = e.repo.path().to_str().expect("ruta UTF-8").to_string();
    let mut args: Vec<&str> = vec![
        "oraculo",
        "pendientes",
        "--tarea",
        "tarea-prueba",
        "--repo",
        repo.as_str(),
    ];
    args.extend_from_slice(extra);
    let mut orden = Command::new(env!("CARGO_BIN_EXE_r00t"));
    orden
        .env("ROOT_HOME", e.home.path())
        .env("ROOT_COLAB_BIN", binario)
        .env("DEEPSEEK_API_KEY", "sk-test-SECRETO")
        .args(&args);
    if let Some(segundos) = plazo {
        orden.env("ROOT_COLAB_PLAZO_PRIMER_PASO_S", segundos);
    }
    orden
}

/// Vivo si `kill -0` lo acepta. Llama a la CLI `kill` con argumentos, sin shell.
fn proceso_vivo(pid: i32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Ningún argumento de ninguna llamada pide otra GPU, TPU ni RAM alta.
fn barrido(lineas: &[String]) {
    for linea in lineas {
        let tokens: Vec<&str> = linea.split_whitespace().collect();
        for (i, token) in tokens.iter().enumerate() {
            assert!(!token.contains("A100"), "GPU prohibida en {linea:?}");
            assert!(!token.contains("H100"), "GPU prohibida en {linea:?}");
            assert!(!matches!(*token, "L4" | "G4"), "GPU prohibida en {linea:?}");
            assert!(!token.starts_with("--tpu"), "TPU prohibida en {linea:?}");
            assert_ne!(*token, "--high-mem", "RAM alta prohibida en {linea:?}");
            if *token == "--gpu" {
                assert_eq!(tokens.get(i + 1), Some(&"T4"), "solo T4 en {linea:?}");
            }
        }
    }
}

/// Posición de la primera llamada que cumple `cuando`. Sin ella, el test falla.
fn posicion(lineas: &[String], nombre: &str, cuando: impl Fn(&str) -> bool) -> usize {
    lineas
        .iter()
        .position(|l| cuando(l))
        .unwrap_or_else(|| panic!("falta la llamada {nombre} en {lineas:?}"))
}

#[test]
fn colab_t4_importa_y_registra_pedir_y_liberar() {
    let e = Escenario::nuevo(ENTORNO_O2);
    let falso = falso(e.falso.path(), "Tesla T4", "ok", false);
    let salida = correr(&e, &falso, &["--colab"]);
    assert_eq!(
        salida.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&salida.stderr)
    );

    let colab: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(e.colab_json()).expect("colab.json"))
            .expect("JSON");
    assert_eq!(colab["test_o::o2"]["resultado"], "pasa");

    let lineas = e.llamadas();
    barrido(&lineas);
    let i_new = posicion(&lineas, "new", |l| {
        l.starts_with("new -s") && l.ends_with("--gpu T4")
    });
    let i_gpu = posicion(&lineas, "exec gpu", |l| {
        l.starts_with("exec") && l.contains("gpu.py")
    });
    let i_up = posicion(&lineas, "upload", |l| l.starts_with("upload"));
    let i_prep = posicion(&lineas, "exec prepara", |l| l.contains("prepara.py"));
    let i_corre = posicion(&lineas, "exec corre.sh", |l| l.contains("corre.py"));
    let i_down = posicion(&lineas, "download", |l| l.starts_with("download"));
    let i_stop = posicion(&lineas, "stop", |l| l.starts_with("stop -s"));
    assert!(
        i_new < i_gpu
            && i_gpu < i_up
            && i_up < i_prep
            && i_prep < i_corre
            && i_corre < i_down
            && i_down < i_stop,
        "orden incorrecto: {lineas:?}"
    );

    let gates = e.eventos_runtime();
    assert_eq!(gates.len(), 3, "pedir, preparar-rust y liberar: {gates:?}");
    assert_eq!(gates[0]["extra"]["accion"], "pedir");
    assert_eq!(gates[0]["extra"]["gpu_pedida"], "T4");
    assert_eq!(gates[1]["extra"]["accion"], "preparar-rust");
    assert_eq!(gates[2]["extra"]["accion"], "liberar");
    assert!(
        gates[2]["extra"]["gpu_observada"]
            .as_str()
            .is_some_and(|g| g.contains("T4")),
        "{gates:?}"
    );

    let subida = fs::read_to_string(e.falso.path().join("subida.txt")).expect("subida.txt");
    for esperado in ["correr.sh", "LEEME.txt", "tests/test_o.py", "repo/a.txt"] {
        assert!(subida.contains(esperado), "falta {esperado} en {subida}");
    }
    assert!(!subida.contains(".env"), "el paquete trae .env: {subida}");

    let eventos = fs::read_to_string(e.home.path().join("eventos.jsonl")).expect("eventos");
    assert!(
        eventos.contains("\"origen\":\"colab\""),
        "falta el Gate de importar"
    );
}

#[test]
fn colab_proceso_no_hereda_claves() {
    let e = Escenario::nuevo(ENTORNO_O2);
    let falso = falso(e.falso.path(), "Tesla T4", "ok", false);
    let salida = correr(&e, &falso, &["--colab"]);
    assert_eq!(salida.status.code(), Some(0));
    let entorno = e.entorno_del_falso();
    assert!(!entorno.contains("DEEPSEEK_API_KEY"), "hereda la clave");
    assert!(
        !entorno.contains("sk-test-SECRETO"),
        "hereda el valor de la clave"
    );
    assert!(entorno.contains("PATH="), "la lista blanca debe pasar PATH");
}

#[test]
fn colab_gpu_a100_sale_3_sin_subir_y_libera() {
    let e = Escenario::nuevo(ENTORNO_O2);
    let falso = falso(e.falso.path(), "NVIDIA A100-SXM4-40GB", "ok", false);
    let salida = correr(&e, &falso, &["--colab"]);
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert_eq!(salida.status.code(), Some(3), "stderr: {stderr}");
    assert!(
        stderr.contains(
            "la GPU del runtime no es una T4 (NVIDIA A100-SXM4-40GB); no se corrió ningún test"
        ),
        "{stderr}"
    );

    let lineas = e.llamadas();
    barrido(&lineas);
    assert!(
        !lineas.iter().any(|l| l.starts_with("upload")),
        "{lineas:?}"
    );
    assert!(!lineas.iter().any(|l| l.contains("corre.py")), "{lineas:?}");
    assert!(
        lineas.last().is_some_and(|l| l.starts_with("stop -s")),
        "{lineas:?}"
    );
    let gates = e.eventos_runtime();
    assert!(
        gates.iter().any(|g| g["extra"]["accion"] == "liberar"
            && g["extra"]["gpu_observada"] == "NVIDIA A100-SXM4-40GB"),
        "{gates:?}"
    );
    assert!(!e.colab_json().exists(), "no se debe importar nada");
}

#[test]
fn colab_sin_sesion_sale_3_con_mensaje_exacto() {
    let e = Escenario::nuevo(ENTORNO_O2);
    let falso = falso(e.falso.path(), "Tesla T4", "falta", false);
    let salida = correr(&e, &falso, &["--colab"]);
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert_eq!(salida.status.code(), Some(3), "stderr: {stderr}");
    assert!(stderr.contains(MENSAJE_LOGIN), "{stderr}");

    let lineas = e.llamadas();
    assert!(
        !lineas.iter().any(|l| l.starts_with("upload")),
        "{lineas:?}"
    );
    assert!(!lineas.iter().any(|l| l.starts_with("exec")), "{lineas:?}");
    assert!(e.eventos_runtime().is_empty(), "sin sesión no hay runtime");
    assert!(!e.colab_json().exists());
}

#[test]
fn colab_falla_a_mitad_libera_el_runtime() {
    let e = Escenario::nuevo(ENTORNO_O2);
    let falso = falso(e.falso.path(), "Tesla T4", "ok", true);
    let salida = correr(&e, &falso, &["--colab"]);
    assert_eq!(salida.status.code(), Some(3));

    let lineas = e.llamadas();
    assert!(
        lineas.last().is_some_and(|l| l.starts_with("stop -s")),
        "el runtime debe liberarse: {lineas:?}"
    );
    let gates = e.eventos_runtime();
    assert!(
        gates.iter().any(|g| g["extra"]["accion"] == "liberar"),
        "{gates:?}"
    );
    assert!(!e.colab_json().exists());
}

#[test]
fn colab_sin_pendientes_no_toca_colab() {
    let e = Escenario::nuevo("");
    let falso = falso(e.falso.path(), "Tesla T4", "ok", false);
    let salida = correr(&e, &falso, &["--colab"]);
    assert_eq!(salida.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&salida.stdout).contains("tests: []"));
    assert!(e.llamadas().is_empty(), "no debe invocar la CLI");
}

#[test]
fn colab_primer_paso_sin_respuesta_sale_3_en_menos_de_5_s() {
    let e = Escenario::nuevo(ENTORNO_O2);
    let falso = falso(e.falso.path(), "Tesla T4", "cuelga", false);
    let inicio = std::time::Instant::now();
    let salida = correr_con(&e, &falso, &["--colab"], Some("1"));
    assert!(
        inicio.elapsed() < std::time::Duration::from_secs(5),
        "tardó {:?}",
        inicio.elapsed()
    );
    let stderr = String::from_utf8_lossy(&salida.stderr).into_owned();
    assert_eq!(salida.status.code(), Some(3), "stderr: {stderr}");
    assert!(
        stderr.contains("60 segundos") && stderr.contains("login"),
        "{stderr}"
    );

    let lineas = e.llamadas();
    assert!(
        lineas.first().is_some_and(|l| l.starts_with("new -s")),
        "{lineas:?}"
    );
    assert!(
        lineas.last().is_some_and(|l| l.starts_with("stop -s")),
        "{lineas:?}"
    );
    assert!(
        !lineas
            .iter()
            .any(|l| l.starts_with("upload") || l.starts_with("exec")),
        "{lineas:?}"
    );
    let pid: i32 = fs::read_to_string(e.falso.path().join("pid.txt"))
        .expect("pid.txt")
        .trim()
        .parse()
        .expect("PID numérico");
    assert!(!proceso_vivo(pid), "la CLI falsa quedó colgada");
}

#[test]
fn colab_con_importar_es_uso_2() {
    let e = Escenario::nuevo(ENTORNO_O2);
    let falso = falso(e.falso.path(), "Tesla T4", "ok", false);
    let salida = correr(&e, &falso, &["--colab", "--importar", "x.xml"]);
    assert_eq!(salida.status.code(), Some(2));
    assert!(e.llamadas().is_empty());
}

#[test]
fn colab_paquete_con_env_no_se_sube() {
    let e = Escenario::nuevo(ENTORNO_O2);
    fs::write(
        e.repo.path().join(".env"),
        "DEEPSEEK_API_KEY=sk-test-SECRETO",
    )
    .expect("escribe");
    let falso = falso(e.falso.path(), "Tesla T4", "ok", false);
    let salida = correr(&e, &falso, &["--colab"]);
    assert_eq!(salida.status.code(), Some(3));
    assert!(
        String::from_utf8_lossy(&salida.stderr).contains("archivo .env"),
        "{}",
        String::from_utf8_lossy(&salida.stderr)
    );
    assert!(e.llamadas().is_empty(), "nada llega a Colab");
}

/// Script `colab` falso para señales, tope y liberación. `corre` es el cuerpo del caso `corre.py`
/// de `exec`; `stop_codigo` es el código de salida de `stop`. `pid.txt` guarda el PID del script.
fn falso_a_medida(dir: &Path, corre: &str, stop_codigo: i32) -> PathBuf {
    falso_a_medida_stop(dir, corre, &format!("exit {stop_codigo}"))
}

/// Como `falso_a_medida`, pero el caso `stop` es el cuerpo de shell `stop_cuerpo`.
fn falso_a_medida_stop(dir: &Path, corre: &str, stop_cuerpo: &str) -> PathBuf {
    let guion = format!(
        r#"#!/bin/sh
DIR='{dir}'
echo "$*" >> "$DIR/llamadas.log"
case "$1" in
new|upload|download) exit 0 ;;
stop) {stop_cuerpo} ;;
exec)
  f=""; prev=""
  for a in "$@"; do if [ "$prev" = "-f" ]; then f="$a"; fi; prev="$a"; done
  case "$f" in
    */gpu.py) echo "ROOT_GPU=Tesla T4" ;;
    */prepara.py) echo "ROOT_PREPARADO=1" ;;
    */prepara-rust.py) echo "ROOT_RUST=rustup" ;;
    */corre.py) echo $$ > "$DIR/pid.txt"; {corre} ;;
  esac
  exit 0 ;;
*) exit 2 ;;
esac
"#,
        dir = dir.display(),
        stop_cuerpo = stop_cuerpo,
        corre = corre,
    );
    let ruta = dir.join("colab-a-medida");
    fs::write(&ruta, guion).expect("escribe el falso");
    fs::set_permissions(&ruta, fs::Permissions::from_mode(0o755)).expect("permisos");
    ruta
}

/// Espera hasta que el log de la CLI falsa muestre `marca`. Falla si no aparece en `segundos`.
fn esperar_en_log(e: &Escenario, marca: &str, segundos: u64) {
    let limite = Instant::now() + Duration::from_secs(segundos);
    while Instant::now() < limite {
        if e.llamadas().iter().any(|l| l.contains(marca)) {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("no apareció {marca} en el log: {:?}", e.llamadas());
}

/// Espera a que `hijo` termine, hasta `segundos`. `None` si sigue vivo al vencer el plazo.
fn esperar_salida(hijo: &mut Child, segundos: u64) -> Option<ExitStatus> {
    let limite = Instant::now() + Duration::from_secs(segundos);
    while Instant::now() < limite {
        if let Ok(Some(estado)) = hijo.try_wait() {
            return Some(estado);
        }
        thread::sleep(Duration::from_millis(20));
    }
    None
}

/// Envía una señal con el comando `kill`, sin shell.
fn enviar_senal(pid: u32, senal: &str) {
    let estado = Command::new("kill")
        .arg(senal)
        .arg(pid.to_string())
        .status()
        .expect("no se pudo ejecutar kill");
    assert!(estado.success(), "kill {senal} {pid} falló");
}

/// Corrida `--colab` interrumpida por `senal` mientras `corre.py` duerme. Root debe liberar y salir con 3.
fn senal_durante_la_corrida(senal: &str, nombre_senal: &str) {
    let e = Escenario::nuevo(ENTORNO_O2);
    let falso = falso_a_medida(e.falso.path(), "sleep 60", 0);
    let salida_stdout = File::create(e.home.path().join("stdout.txt")).expect("stdout");
    let salida_stderr = File::create(e.home.path().join("stderr.txt")).expect("stderr");
    let mut hijo = orden(&e, &falso, &["--colab"], None)
        .stdout(Stdio::from(salida_stdout))
        .stderr(Stdio::from(salida_stderr))
        .spawn()
        .expect("no se pudo lanzar r00t");
    esperar_en_log(&e, "corre.py", 30);
    enviar_senal(hijo.id(), senal);
    let estado = esperar_salida(&mut hijo, 10).expect("Root debe salir en menos de 10 s");
    let stderr = fs::read_to_string(e.home.path().join("stderr.txt")).unwrap_or_default();
    assert_eq!(estado.code(), Some(3), "stderr: {stderr}");
    assert!(
        stderr.contains(&format!(
            "Interrumpido por {nombre_senal}: se liberó el runtime de Colab"
        )),
        "{stderr}"
    );

    let lineas = e.llamadas();
    assert!(
        lineas.iter().any(|l| l.starts_with("stop -s")),
        "falta el stop: {lineas:?}"
    );
    let gates = e.eventos_runtime();
    assert!(
        gates.iter().any(|g| g["extra"]["accion"] == "liberar"
            && g["extra"]["motivo"]
                .as_str()
                .is_some_and(|m| m.contains(nombre_senal))),
        "{gates:?}"
    );
    assert!(
        !e.home.path().join("colab").join("activo.json").exists(),
        "activo.json debe borrarse tras liberar"
    );
    let pid: i32 = fs::read_to_string(e.falso.path().join("pid.txt"))
        .expect("pid.txt")
        .trim()
        .parse()
        .expect("PID numérico");
    assert!(!proceso_vivo(pid), "la CLI falsa quedó viva");
}

#[test]
fn colab_sigint_durante_la_corrida_libera_y_sale_3() {
    senal_durante_la_corrida("-INT", "SIGINT");
}

#[test]
fn colab_sigterm_durante_la_corrida_libera_y_sale_3() {
    senal_durante_la_corrida("-TERM", "SIGTERM");
}

#[test]
fn colab_huerfano_se_libera_antes_de_new_y_sigue() {
    let e = Escenario::nuevo(ENTORNO_O2);
    let carpeta = e.home.path().join("colab");
    fs::create_dir_all(&carpeta).expect("carpeta colab");
    fs::write(carpeta.join("activo.json"), ACTIVO_VIEJO_TEST).expect("activo.json");
    let falso = falso(e.falso.path(), "Tesla T4", "ok", false);
    let salida = correr(&e, &falso, &["--colab"]);
    assert_eq!(
        salida.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let lineas = e.llamadas();
    let i_stop = posicion(&lineas, "stop de la sesión vieja", |l| {
        l == "stop -s vieja"
    });
    let i_new = posicion(&lineas, "new", |l| l.starts_with("new -s"));
    assert!(i_stop < i_new, "el huérfano va antes de new: {lineas:?}");
    let gates = e.eventos_runtime();
    assert!(
        gates
            .iter()
            .any(|g| g["extra"]["accion"] == "liberar-huerfano"),
        "{gates:?}"
    );
    assert!(!carpeta.join("activo.json").exists());
    assert!(e.colab_json().exists(), "la corrida nueva sigue y termina");
}

const ACTIVO_VIEJO_TEST: &str = r#"{"v":1,"sesion":"vieja","pedido_ts":1,"gpu_pedida":"T4"}"#;

/// `r00t oraculo pendientes --liberar`, sin tarea ni repo.
fn liberar(e: &Escenario, binario: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_r00t"))
        .env("ROOT_HOME", e.home.path())
        .env("ROOT_COLAB_BIN", binario)
        .args(["oraculo", "pendientes", "--liberar"])
        .output()
        .expect("no se pudo ejecutar r00t")
}

#[test]
fn colab_liberar_solo_hace_stop_y_sale_0() {
    let e = Escenario::nuevo("");
    let carpeta = e.home.path().join("colab");
    fs::create_dir_all(&carpeta).expect("carpeta colab");
    fs::write(carpeta.join("activo.json"), ACTIVO_VIEJO_TEST).expect("activo.json");
    let falso = falso_a_medida(e.falso.path(), "exit 0", 0);
    let salida = liberar(&e, &falso);
    assert_eq!(
        salida.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&salida.stderr)
    );
    assert_eq!(e.llamadas(), vec!["stop -s vieja".to_string()]);
    assert!(!carpeta.join("activo.json").exists());
}

#[test]
fn colab_liberar_sin_archivo_dice_no_hay_runtime() {
    let e = Escenario::nuevo("");
    let falso = falso_a_medida(e.falso.path(), "exit 0", 0);
    let salida = liberar(&e, &falso);
    assert_eq!(salida.status.code(), Some(0));
    assert!(
        String::from_utf8_lossy(&salida.stdout).contains("no hay runtime anotado"),
        "stdout: {}",
        String::from_utf8_lossy(&salida.stdout)
    );
    assert!(e.llamadas().is_empty(), "sin archivo no se llama a la CLI");
}

#[test]
fn colab_liberar_con_cli_que_falla_sale_3_y_conserva_archivo() {
    let e = Escenario::nuevo("");
    let carpeta = e.home.path().join("colab");
    fs::create_dir_all(&carpeta).expect("carpeta colab");
    fs::write(carpeta.join("activo.json"), ACTIVO_VIEJO_TEST).expect("activo.json");
    let falso = falso_a_medida(e.falso.path(), "exit 0", 1);
    let salida = liberar(&e, &falso);
    assert_eq!(salida.status.code(), Some(3));
    assert!(
        carpeta.join("activo.json").exists(),
        "sin parada confirmada el archivo sigue"
    );
}

#[test]
fn colab_liberar_con_sesion_invalida_renombra_a_ilegible() {
    let e = Escenario::nuevo("");
    let carpeta = e.home.path().join("colab");
    fs::create_dir_all(&carpeta).expect("carpeta colab");
    let texto = r#"{"v":1,"sesion":"a b;rm","pedido_ts":1,"gpu_pedida":"T4"}"#;
    fs::write(carpeta.join("activo.json"), texto).expect("activo.json");
    let falso = falso_a_medida(e.falso.path(), "exit 0", 0);
    let salida = liberar(&e, &falso);
    assert_eq!(salida.status.code(), Some(0));
    assert!(
        e.llamadas().is_empty(),
        "la sesión inválida no llega a la CLI"
    );
    assert!(!carpeta.join("activo.json").exists());
    assert_eq!(
        fs::read_to_string(carpeta.join("activo.json.ilegible")).expect("conservado"),
        texto
    );
}

#[test]
fn colab_activo_no_queda_tras_corrida_exitosa() {
    let e = Escenario::nuevo(ENTORNO_O2);
    let falso = falso(e.falso.path(), "Tesla T4", "ok", false);
    let salida = correr(&e, &falso, &["--colab"]);
    assert_eq!(salida.status.code(), Some(0));
    assert!(!e.home.path().join("colab").join("activo.json").exists());
}

/// Lanza `--colab` con `falso`, espera a que `corre.py` empiece, manda `senal` y espera la salida.
/// Devuelve el código de salida y el stderr de Root.
fn colab_interrumpido(e: &Escenario, falso: &Path, senal: &str) -> (Option<i32>, String) {
    let salida_stderr = File::create(e.home.path().join("stderr.txt")).expect("stderr");
    let mut hijo = orden(e, falso, &["--colab"], None)
        .stdout(Stdio::null())
        .stderr(Stdio::from(salida_stderr))
        .spawn()
        .expect("no se pudo lanzar r00t");
    esperar_en_log(e, "corre.py", 30);
    enviar_senal(hijo.id(), senal);
    let codigo = esperar_salida(&mut hijo, 10).and_then(|s| s.code());
    let stderr = fs::read_to_string(e.home.path().join("stderr.txt")).unwrap_or_default();
    (codigo, stderr)
}

#[test]
fn colab_stop_con_salida_desconocida_conserva_activo() {
    let e = Escenario::nuevo(ENTORNO_O2);
    let falso = falso_a_medida_stop(
        e.falso.path(),
        "exit 0",
        "echo 'respuesta rara 7f3' >&2; exit 1",
    );
    let salida = correr(&e, &falso, &["--colab"]);
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert_eq!(salida.status.code(), Some(3), "stderr: {stderr}");
    assert!(
        e.llamadas().iter().any(|l| l.starts_with("stop -s")),
        "falta el stop: {:?}",
        e.llamadas()
    );
    assert!(
        e.home.path().join("colab").join("activo.json").exists(),
        "una salida de stop no reconocida no confirma la parada: activo.json se conserva"
    );
    let gates = e.eventos_runtime();
    let ultimo = gates.last().expect("hay un Gate de colab-runtime");
    assert_eq!(ultimo["extra"]["accion"], "liberar", "{gates:?}");
    assert_eq!(ultimo["extra"]["confirmada"], false, "{gates:?}");
}

#[test]
fn colab_tope_fraccion_de_minuto_libera_en_menos_de_5_s() {
    let e = Escenario::nuevo(ENTORNO_O2);
    fs::write(
        e.home.path().join("root.toml"),
        "[colab]\nminutos_max = 0.01\n",
    )
    .expect("root.toml");
    let falso = falso_a_medida(e.falso.path(), "sleep 300", 0);
    let inicio = Instant::now();
    let salida = correr(&e, &falso, &["--colab"]);
    let duracion = inicio.elapsed();
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(
        duracion < Duration::from_secs(5),
        "tardó {duracion:?}: {stderr}"
    );
    assert_eq!(salida.status.code(), Some(3), "stderr: {stderr}");
    assert!(
        stderr.contains("Colab superó el tope de 600 ms: se liberó el runtime"),
        "{stderr}"
    );
    let lineas = e.llamadas();
    assert!(
        lineas.iter().any(|l| l.contains("corre.py")),
        "la corrida no llegó a corre.py: {lineas:?}"
    );
    let ultima = lineas.last().cloned().unwrap_or_default();
    assert!(ultima.starts_with("stop -s"), "última llamada: {ultima}");
}

#[test]
fn colab_sigint_con_sesion_inexistente_confirma_y_borra_activo() {
    let e = Escenario::nuevo(ENTORNO_O2);
    let falso = falso_a_medida_stop(
        e.falso.path(),
        "sleep 60",
        "echo 'session not found' >&2; exit 1",
    );
    let (codigo, stderr) = colab_interrumpido(&e, &falso, "-INT");
    assert_eq!(codigo, Some(3), "stderr: {stderr}");
    assert!(
        stderr.contains("Interrumpido por SIGINT: se liberó el runtime de Colab"),
        "{stderr}"
    );
    assert!(
        !e.home.path().join("colab").join("activo.json").exists(),
        "la sesión ya no existía: activo.json se borra"
    );
    let gates = e.eventos_runtime();
    assert!(
        gates.iter().any(|g| g["extra"]["accion"] == "liberar"
            && g["extra"]["confirmada"] == true
            && g["extra"]["nota"] == "la sesión ya no existía"),
        "{gates:?}"
    );
}

#[test]
fn colab_sigint_con_error_de_red_en_stop_conserva_activo() {
    let e = Escenario::nuevo(ENTORNO_O2);
    let falso = falso_a_medida_stop(
        e.falso.path(),
        "sleep 60",
        "echo 'error de red' >&2; exit 1",
    );
    let (codigo, stderr) = colab_interrumpido(&e, &falso, "-INT");
    assert_eq!(codigo, Some(3), "stderr: {stderr}");
    assert!(stderr.contains("no se confirmó la parada"), "{stderr}");
    assert!(
        e.home.path().join("colab").join("activo.json").exists(),
        "sin parada confirmada el archivo se conserva"
    );
    let gates = e.eventos_runtime();
    assert!(
        gates
            .iter()
            .any(|g| g["extra"]["accion"] == "liberar" && g["extra"]["confirmada"] == false),
        "{gates:?}"
    );
}
