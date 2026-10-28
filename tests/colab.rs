//! `r00t oraculo pendientes --colab` contra el binario real y una CLI `colab` falsa.
//! Ningún test toca Colab: el falso registra cada invocación en `llamadas.log`.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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
  if [ "$AUTH" = falta ]; then echo "Not logged in: authenticate with OAuth first" >&2; exit 1; fi
  exit 0 ;;
exec)
  f=""; prev=""
  for a in "$@"; do if [ "$prev" = "-f" ]; then f="$a"; fi; prev="$a"; done
  case "$f" in
    */gpu.py) echo "ROOT_GPU=$GPU" ;;
    */prepara.py) echo "ROOT_PREPARADO=1" ;;
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
    Command::new(env!("CARGO_BIN_EXE_r00t"))
        .env("ROOT_HOME", e.home.path())
        .env("ROOT_COLAB_BIN", binario)
        .env("DEEPSEEK_API_KEY", "sk-test-SECRETO")
        .args(&args)
        .output()
        .expect("no se pudo ejecutar r00t")
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
    assert_eq!(gates.len(), 2, "pedir y liberar: {gates:?}");
    assert_eq!(gates[0]["extra"]["accion"], "pedir");
    assert_eq!(gates[0]["extra"]["gpu_pedida"], "T4");
    assert_eq!(gates[1]["extra"]["accion"], "liberar");
    assert!(
        gates[1]["extra"]["gpu_observada"]
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
