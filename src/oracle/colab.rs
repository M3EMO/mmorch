//! Tests pendientes en Colab con una T4 (decisión 22). Root habla con la CLI `colab` solo por
//! vectores de argumentos, nunca por cadenas de shell. El único dato del usuario que llega a
//! Colab es el nombre de sesión, y se valida antes.
//!
//! La preparación (`prepara.py`) instala pytest en el runtime de Colab, en la cuenta de Google
//! del usuario. Rust lo prepara `prepara-rust.py`, con `rustup-init` verificado por sha256. Si
//! Rust no queda listo, Root compila el producto en su máquina y sube el binario (respaldo). Root
//! no instala nada en su propia máquina.
//!
//! Antes de pedir el runtime, Root anota la sesión en `$ROOT_HOME/colab/activo.json`. La borra
//! al liberar el runtime. `correr.sh` corre en segundo plano en el runtime, y Root lo sondea cada
//! minuto: libera el runtime en cuanto el junit cumple lo pedido, o cuando `correr.sh` termina.
//! `SIGINT`, `SIGTERM` y el tope de duración (2 horas por defecto) también liberan el runtime.
//! `oraculo pendientes --vigilar` es la red de seguridad: libera el runtime anotado si cumplió lo
//! pedido o si pasó su tope. `SIGKILL` no se puede manejar: la próxima corrida, o
//! `oraculo pendientes --liberar`, libera el runtime anotado.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use serde_json::{Value, json};
use signal_hook::consts::{SIGINT, SIGTERM};

use super::junit;
use super::pendientes::nombre_junit;
use super::tarea::Tarea;
use super::{ErrorOraculo, infra};
use crate::observe;

/// Variable que apunta al binario de la CLI. Los tests la usan con un falso.
const VARIABLE_BINARIO: &str = "ROOT_COLAB_BIN";
/// Solo para tests: baja el plazo del primer paso a los segundos que indique.
const VARIABLE_PLAZO_PRUEBA: &str = "ROOT_COLAB_PLAZO_PRIMER_PASO_S";
/// Tiempo máximo de cada llamada a la CLI, en segundos.
const TIMEOUT_LLAMADA: Duration = Duration::from_secs(3600);
/// Plazo del primer paso (`new`), que sirve de chequeo de sesión. La CLI responde la asignación
/// o el error de login rápido. Si Colab tarda más de 60 s, el usuario reintenta.
const PLAZO_PRIMER_PASO: Duration = Duration::from_secs(60);
/// Plazo de `colab stop` cuando el primer paso no responde, y de la parada tras una señal o un tope.
/// Su error se ignora en esos caminos.
const PLAZO_STOP_FALLIDO: Duration = Duration::from_secs(30);
/// Pausa entre sondeos del hijo. Cada sondeo mira también las señales.
const PASO_SONDEO: Duration = Duration::from_millis(50);
/// Tiempo de `exec` para leer la GPU.
const SEG_GPU: u64 = 300;
/// Tiempo de `exec` para preparar el runtime.
const SEG_PREPARAR: u64 = 1800;
/// Tiempo de `exec` para probar el binario de respaldo.
const SEG_PRUEBA_BINARIO: u64 = 120;
/// Tiempo de `exec` para lanzar `correr.sh`. El lanzamiento vuelve enseguida.
const SEG_CORRER: u64 = 300;
/// Tiempo de `exec` para leer el estado de `correr.sh`.
const SEG_ESTADO: u64 = 300;
/// Causa del `Gate` de liberación cuando el junit cumple lo que pedía la corrida.
const CAUSA_REQUISITO: &str = "requisito cumplido";
/// Causa del `Gate` de liberación cuando `correr.sh` terminó, con el junit completo o no.
const CAUSA_TERMINO: &str = "correr.sh terminó";
/// Causa del `Gate` de liberación al llegar al tope de duración. El valor real del tope va en `motivo`,
/// porque `minutos_max` se puede cambiar.
const CAUSA_TOPE: &str = "tope de duración";
/// Causa del `Gate` cuando la GPU del runtime no es una T4.
const CAUSA_GPU_NO_T4: &str = "gpu-no-t4";
/// Causa del `Gate` cuando `colab new` no asignó el runtime (un fallo que no es de login).
const CAUSA_RUNTIME_NO_ASIGNADO: &str = "runtime-no-asignado";
/// Causa del `Gate` cuando un paso de la corrida falla a mitad, sin señal ni tope.
const CAUSA_ERROR_CORRIDA: &str = "error-en-corrida";
/// La única GPU que Root pide.
const GPU_PEDIDA: &str = "T4";
/// Nombre del paquete dentro del runtime. Es el mismo en la subida y en la preparación.
const PAQUETE_REMOTO: &str = "root-paquete.tar.gz";
/// Carpeta donde `prepara.py` descomprime el paquete en el runtime.
const CARPETA_REMOTA: &str = "root-paquete";
/// Archivo de eventos de Root dentro de `ROOT_HOME`.
const ARCHIVO_EVENTOS: &str = "eventos.jsonl";
/// Fase de los eventos `Gate` del runtime.
const FASE_RUNTIME: &str = "colab-runtime";
/// Carpeta de Root que guarda el estado del runtime en curso.
const CARPETA_COLAB: &str = "colab";
/// Archivo que anota la sesión pedida y todavía no liberada.
const ARCHIVO_ACTIVO: &str = "activo.json";
/// Corrida que firma los `Gate` de un runtime anotado, sin tarea de por medio.
const CORRIDA_ANOTADA: &str = "colab-activo";
/// Mensaje exacto cuando la CLI no tiene sesión. Root no hace el login.
pub const MENSAJE_SIN_SESION: &str = "Falta la sesión de Colab. Corré el login OAuth en tu terminal (por ejemplo `colab new -s prueba --gpu T4`, completá el navegador, y después `colab stop -s prueba`) y volvé a lanzar este comando.";
/// Mensaje cuando el primer paso no responde en su plazo. Probablemente falta el login OAuth.
/// El texto dice 60 segundos aunque un test baje el plazo, porque el mensaje es para el usuario.
pub const MENSAJE_SIN_RESPUESTA: &str = "Colab no respondió en 60 segundos. Probablemente falta el login OAuth: corré `colab new -s prueba --gpu T4` en tu terminal, completá el navegador, `colab stop -s prueba`, y volvé a lanzar este comando.";
/// Textos de stdout o stderr que indican que la CLI no tiene login.
const MARCADORES_SIN_SESION: &[&str] = &[
    "login",
    "authenticate",
    "credentials",
    "oauth",
    "not logged",
    "unauthorized",
    "401",
];
/// Textos de stdout o stderr que indican que la sesión ya no existe: el runtime ya no está, y la
/// parada cuenta como confirmada. A confirmar con la CLI real (no se ejecutó). `not found` no está:
/// una salida desconocida no confirma la parada (desempate 4 de "Desempates técnicos 8").
const MARCADORES_SESION_INEXISTENTE: &[&str] = &[
    "no existe",
    "inexistente",
    "does not exist",
    "no such",
    "no encontr",
];
/// Variables que sí pasan a la CLI. Cualquier otra, sobre todo las claves, se descarta.
const VARIABLES_PERMITIDAS: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "LOGNAME",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "SSL_CERT_FILE",
    "REQUESTS_CA_BUNDLE",
    "GOOGLE_APPLICATION_CREDENTIALS",
    "CLOUDSDK_CONFIG",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "NO_PROXY",
    "http_proxy",
    "https_proxy",
    "no_proxy",
];
/// Prefijo de las variables XDG que sí pasan.
const PREFIJO_PERMITIDO: &str = "XDG_";

/// Script de lectura de GPU. Imprime una línea `ROOT_GPU=<nombre>`.
const GUION_GPU: &str = r#"import subprocess
r = subprocess.run(["nvidia-smi", "--query-gpu=name", "--format=csv,noheader"], capture_output=True, text=True)
lineas = [l.strip() for l in r.stdout.splitlines() if l.strip()]
print("ROOT_GPU=" + (lineas[0] if r.returncode == 0 and lineas else ""))
"#;

/// Script de preparación. Descomprime el paquete e instala pytest en el runtime. Rust no va acá:
/// lo prepara `guion_rust`, con `rustup-init` verificado.
const GUION_PREPARAR: &str = r#"import os, subprocess, sys
os.makedirs("root-paquete", exist_ok=True)
subprocess.run(["tar", "-xzf", "root-paquete.tar.gz", "-C", "root-paquete"], check=True)
subprocess.run([sys.executable, "-m", "pip", "install", "-q", "pytest"], check=True)
print("ROOT_PREPARADO=1")
"#;

/// Script de corrida. Lanza `correr.sh` en segundo plano y vuelve enseguida: Root lo sondea después
/// con `GUION_ESTADO`. La salida de `correr.sh` queda en `correr.log` y su código en `estado.rc`.
const GUION_CORRER: &str = r#"import os, subprocess
env = dict(os.environ)
env["PATH"] = os.path.expanduser("~/.cargo/bin") + os.pathsep + env.get("PATH", "")
# Sin DEVNULL en la entrada y en las salidas, el exec de Colab esperaría a que `correr.sh` termine.
subprocess.Popen(["bash", "-c", "bash correr.sh > correr.log 2>&1; echo $? > estado.rc"], cwd="root-paquete", env=env, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
print("ROOT_LANZADO=1")
"#;

/// Script de estado. `correr.sh` terminó cuando existe `root-paquete/estado.rc` con su código.
const GUION_ESTADO: &str = r#"import os
RC = os.path.join("root-paquete", "estado.rc")
codigo = ""
if os.path.exists(RC):
    with open(RC) as archivo:
        codigo = archivo.read().strip()
if codigo:
    print("ROOT_ESTADO=terminado:" + codigo)
else:
    print("ROOT_ESTADO=corriendo")
"#;

/// Base de las descargas de `rustup-init` (y de su `.sha256`) para x86_64.
pub const BASE_RUSTUP: &str = "https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu";

/// Script que deja Rust listo en el runtime. Baja `rustup-init` y su `.sha256` desde `base`,
/// compara el hash y solo si coincide lo corre. Imprime `ROOT_RUST=ya`, `ROOT_RUST=rustup` o
/// `ROOT_RUST=falla:<causa>` (`descarga`, `hash` o `instalacion`). Nunca usa `shell=True`,
/// `curl` ni una tubería a `sh`.
pub fn guion_rust(base: &str) -> String {
    let base = serde_json::to_string(base).expect("una cadena se serializa");
    format!(
        r#"import hashlib, os, shutil, subprocess, sys, urllib.request
BASE = {base}
CARPETA = "root-rust"
BINARIO = os.path.join(CARPETA, "rustup-init")
SUMA = BINARIO + ".sha256"

def salir(resultado):
    print("ROOT_RUST=" + resultado)
    sys.exit(0)

if shutil.which("cargo") or os.path.exists(os.path.expanduser("~/.cargo/bin/cargo")):
    salir("ya")

os.makedirs(CARPETA, exist_ok=True)
try:
    for origen, destino in [(BASE + "/rustup-init", BINARIO), (BASE + "/rustup-init.sha256", SUMA)]:
        with urllib.request.urlopen(origen, timeout=120) as respuesta, open(destino, "wb") as archivo:
            shutil.copyfileobj(respuesta, archivo)
except Exception:
    salir("falla:descarga")

with open(SUMA) as archivo:
    partes = archivo.read().split()
esperado = partes[0].lower() if partes else ""
with open(BINARIO, "rb") as archivo:
    obtenido = hashlib.sha256(archivo.read()).hexdigest()
if obtenido != esperado:
    os.remove(BINARIO)
    salir("falla:hash")

os.chmod(BINARIO, 0o755)
ruta = os.path.abspath(BINARIO)
try:
    codigo = subprocess.run([ruta, "-y", "--profile", "minimal", "--no-modify-path"], timeout=1500).returncode
except (subprocess.TimeoutExpired, OSError):
    salir("falla:instalacion")
salir("rustup" if codigo == 0 else "falla:instalacion")
"#
    )
}

/// Script que prueba el binario subido: `--version` y, si falla, `--help`. Imprime
/// `ROOT_BINARIO=ok` o `ROOT_BINARIO=falla:<código>`. Sin permiso de ejecución sale 126 y sin
/// archivo 127, como en la shell.
fn guion_probar_binario(nombre: &str) -> String {
    let nombre = serde_json::to_string(nombre).expect("una cadena se serializa");
    format!(
        r#"import os, subprocess
RUTA = os.path.abspath(os.path.join("root-paquete", "target", "debug", {nombre}))
CARGO = os.path.abspath(os.path.join("root-paquete", "bin", "cargo"))

def correr(opcion):
    return subprocess.run([RUTA, opcion], capture_output=True, timeout=60)

try:
    os.chmod(RUTA, 0o755)
    if os.path.exists(CARGO):
        os.chmod(CARGO, 0o755)
    resultado = correr("--version")
    if resultado.returncode != 0:
        resultado = correr("--help")
    codigo = resultado.returncode
except subprocess.TimeoutExpired:
    codigo = 124
except FileNotFoundError:
    codigo = 127
except OSError:
    codigo = 126
print("ROOT_BINARIO=ok" if codigo == 0 else "ROOT_BINARIO=falla:%d" % codigo)
"#
    )
}

/// `cargo` mínimo del respaldo (Tarea 7k). El runtime no tiene compilador: el binario ya viene en
/// `target/debug`. Solo acepta `build -q --bin <nombre>`, sale con 0 si el binario existe, y falla con un
/// mensaje ante cualquier otra orden.
fn guion_cargo(nombre: &str) -> String {
    format!(
        r#"#!/usr/bin/env bash
# cargo mínimo de Root: el binario de respaldo ya viene compilado en target/debug.
if [ "$*" != "build -q --bin {nombre}" ]; then
  echo "cargo de Root: solo acepta 'build -q --bin {nombre}'; el runtime no tiene compilador (orden: $*)" >&2
  exit 1
fi
destino="${{CARGO_TARGET_DIR:-$(dirname "$0")/../target}}/debug/{nombre}"
if [ ! -f "$destino" ]; then
  echo "cargo de Root: falta el binario $destino" >&2
  exit 1
fi
exit 0
"#
    )
}

/// Lo que `Cargo.toml` aporta al nombre del binario.
#[derive(Deserialize)]
struct CargoToml {
    package: Option<Nombrado>,
    #[serde(default)]
    bin: Vec<Nombrado>,
}

#[derive(Deserialize)]
struct Nombrado {
    name: Option<String>,
}

/// Nombre del binario que compila el repo: el del primer `[[bin]]` si tiene uno, o el de
/// `[package]`. `None` si no se puede leer o si el nombre no es seguro para armar rutas y scripts.
fn nombre_binario(repo: &Path) -> Option<String> {
    let texto = fs::read_to_string(repo.join("Cargo.toml")).ok()?;
    let cargo: CargoToml = toml::from_str(&texto).ok()?;
    let nombre = cargo
        .bin
        .first()
        .and_then(|bin| bin.name.clone())
        .or_else(|| cargo.package.and_then(|paquete| paquete.name))?;
    let seguro = !nombre.is_empty()
        && nombre
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    seguro.then_some(nombre)
}

/// Compila el producto en la máquina del usuario, con la imagen de la tarea, y devuelve la ruta
/// del binario. Es el respaldo cuando Rust no queda listo en Colab.
pub trait CompiladorLocal {
    fn compilar(&self, repo: &Path, tarea: &Tarea) -> Result<PathBuf, ErrorOraculo>;
}

/// Compilador real: por ahora no compila, y así el respaldo queda pendiente.
pub struct CompiladorPodman;

impl CompiladorLocal for CompiladorPodman {
    fn compilar(&self, _repo: &Path, _tarea: &Tarea) -> Result<PathBuf, ErrorOraculo> {
        // Parte laptop de la Tarea 7k: compilar con podman en la imagen de la tarea.
        Err(ErrorOraculo::Infra(
            "respaldo de compilación local sin implementar en la nube: lo arma la parte laptop de la Tarea 7k".into(),
        ))
    }
}

/// Salida de una llamada a la CLI `colab`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SalidaColab {
    pub codigo: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Cómo termina una espera de la CLI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Espera {
    /// La CLI terminó con su salida. Un código distinto de cero es un resultado, no un error.
    Termino(SalidaColab),
    /// La CLI no terminó en su plazo.
    Plazo,
    /// Llegó una señal (`SIGINT` o `SIGTERM`) mientras se esperaba.
    Senal(&'static str),
}

/// Banderas de señal. Una se activa al llegar `SIGINT`, y otra al llegar `SIGTERM`.
/// Un clon comparte las mismas banderas.
#[derive(Debug, Default, Clone)]
pub struct Senales {
    sigint: Arc<AtomicBool>,
    sigterm: Arc<AtomicBool>,
}

impl Senales {
    /// Registra los manejadores de `SIGINT` y `SIGTERM`. Se llama una vez por proceso.
    pub fn instalar() -> io::Result<Senales> {
        let senales = Senales::default();
        signal_hook::flag::register(SIGINT, Arc::clone(&senales.sigint))?;
        signal_hook::flag::register(SIGTERM, Arc::clone(&senales.sigterm))?;
        Ok(senales)
    }

    /// Nombre de la señal recibida, si llegó alguna.
    pub fn recibida(&self) -> Option<&'static str> {
        if self.sigint.load(Ordering::SeqCst) {
            Some("SIGINT")
        } else if self.sigterm.load(Ordering::SeqCst) {
            Some("SIGTERM")
        } else {
            None
        }
    }

    /// Activa una señal sin un proceso real. Solo para tests.
    #[cfg(test)]
    pub fn activar(&self, senal: &str) {
        let bandera = if senal == "SIGINT" {
            &self.sigint
        } else {
            &self.sigterm
        };
        bandera.store(true, Ordering::SeqCst);
    }
}

/// Lanza la CLI `colab` con argumentos y la espera hasta `plazo`, mirando `cancelar`.
pub trait ColabCli {
    fn ejecutar_con_plazo(
        &self,
        args: &[String],
        plazo: Duration,
        cancelar: &Senales,
    ) -> Result<Espera, ErrorOraculo>;

    /// Plazo del primer paso (`new`).
    fn plazo_primer_paso(&self) -> Duration {
        PLAZO_PRIMER_PASO
    }

    /// Pausa entre sondeos de `correr.sh` en el runtime. Solo los tests la bajan.
    fn intervalo_sondeo(&self) -> Duration {
        Duration::from_secs(60)
    }
}

/// CLI real: un proceso con entorno en lista blanca.
pub struct ColabProceso {
    binario: PathBuf,
    plazo_primer_paso: Duration,
}

impl ColabProceso {
    /// Binario de `ROOT_COLAB_BIN`, o `colab` del `PATH`, o `~/.local/bin/colab`.
    pub fn desde_entorno() -> Result<Self, ErrorOraculo> {
        let binario = match std::env::var_os(VARIABLE_BINARIO) {
            Some(valor) => PathBuf::from(valor),
            None => buscar_colab()?,
        };
        let plazo_primer_paso = std::env::var(VARIABLE_PLAZO_PRUEBA)
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .map_or(PLAZO_PRIMER_PASO, Duration::from_secs);
        Ok(ColabProceso {
            binario,
            plazo_primer_paso,
        })
    }
}

impl ColabCli for ColabProceso {
    fn plazo_primer_paso(&self) -> Duration {
        self.plazo_primer_paso
    }

    fn ejecutar_con_plazo(
        &self,
        args: &[String],
        plazo: Duration,
        cancelar: &Senales,
    ) -> Result<Espera, ErrorOraculo> {
        // Una señal que llegó antes de lanzar la CLI evita el lanzamiento: `new` no crea nada.
        if let Some(senal) = cancelar.recibida() {
            return Ok(Espera::Senal(senal));
        }
        let mut orden = Command::new(&self.binario);
        orden
            .args(args)
            .env_clear()
            .envs(entorno_permitido(std::env::vars_os()))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        propio_grupo(&mut orden);
        let mut hijo = orden.spawn().map_err(|e| {
            ErrorOraculo::Infra(format!("no se pudo lanzar {}: {e}", self.binario.display()))
        })?;
        let salida = hijo.stdout.take();
        let errores = hijo.stderr.take();
        let h_salida = thread::spawn(move || leer_todo(salida));
        let h_errores = thread::spawn(move || leer_todo(errores));
        let inicio = Instant::now();
        let estado = loop {
            match hijo.try_wait() {
                Ok(Some(estado)) => break estado,
                Ok(None) => {
                    if let Some(senal) = cancelar.recibida() {
                        matar_arbol(&mut hijo);
                        return Ok(Espera::Senal(senal));
                    }
                    if inicio.elapsed() >= plazo {
                        // Sin join: un nieto vivo podría retener los pipes. Los hilos terminan solos.
                        matar_arbol(&mut hijo);
                        return Ok(Espera::Plazo);
                    }
                    thread::sleep(PASO_SONDEO);
                }
                Err(e) => return Err(infra("esperar a colab", e)),
            }
        };
        Ok(Espera::Termino(SalidaColab {
            codigo: estado.code().unwrap_or(-1),
            stdout: h_salida.join().unwrap_or_default(),
            stderr: h_errores.join().unwrap_or_default(),
        }))
    }
}

/// Resultado de una corrida en Colab: el junit y el dato de GPU para el informe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultadoColab {
    /// Un junit por repetición, en orden. Puede faltar alguno si su repetición no terminó.
    pub junits: Vec<PathBuf>,
    pub gpu: String,
    pub sesion: String,
}

/// Contenido de `activo.json`: el runtime pedido y todavía no liberado.
#[derive(Debug, Deserialize)]
struct Activo {
    v: u64,
    sesion: String,
    pedido_ts: u64,
    gpu_pedida: String,
    /// Tope de la corrida en segundos. Ausente en un `activo.json` viejo: vale el de `root.toml`.
    #[serde(default)]
    tope_s: Option<f64>,
    /// Ids que el junit debe traer. Ausentes en un `activo.json` viejo: `vigilar` no mira el junit.
    #[serde(default)]
    ids: Vec<String>,
    /// Repeticiones que pide la tarea. Ausentes en un `activo.json` viejo.
    #[serde(default)]
    repeticiones: Option<u32>,
}

/// Qué hizo `vigilar` con el runtime anotado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Vigilancia {
    /// No había `activo.json` (o estaba ilegible y se apartó).
    SinRuntime,
    /// El runtime sigue dentro de su tope y sin el requisito cumplido.
    Vigente,
    /// Se liberó el runtime. `causa` es el texto del `Gate`.
    Liberado { causa: String },
}

/// Criterio de lo que pedía la corrida. Cumple si cada junit parsea, si `ids` no está vacío y si cada
/// id aparece con al menos `repeticiones` testcases, sumados entre todos los junit (uno por repetición).
/// Cada testcase cuenta como pasa, falla, error u omitido, así que un `<skipped/>` también cuenta.
pub fn cumple_lo_pedido(junits: &[String], ids: &[String], repeticiones: u32) -> bool {
    if ids.is_empty() {
        return false;
    }
    let mut veces: BTreeMap<String, u32> = BTreeMap::new();
    for texto in junits {
        let Ok(leido) = junit::parsear(texto) else {
            return false;
        };
        for (id, n) in leido.con_ids().testcases {
            *veces.entry(id).or_default() += n;
        }
    }
    ids.iter()
        .all(|id| veces.get(id).is_some_and(|total| *total >= repeticiones))
}

/// Red de seguridad para el runtime anotado: lee `activo.json` y lo libera si pasó su tope o si su
/// junit cumple lo pedido. Un runtime vigente no se toca. Sin `activo.json` no llama a la CLI.
/// `tope_por_defecto` vale si el archivo no trae `tope_s`.
pub fn vigilar(
    cli: &dyn ColabCli,
    home: &Path,
    tope_por_defecto: Duration,
) -> Result<Vigilancia, ErrorOraculo> {
    let texto = match fs::read_to_string(ruta_activo(home)) {
        Ok(texto) => texto,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vigilancia::SinRuntime),
        Err(e) => return Err(infra("leer activo.json", e)),
    };
    let Some(activo) = leer_activo(&texto) else {
        apartar_ilegible(home)?;
        return Ok(Vigilancia::SinRuntime);
    };
    let tope = activo
        .tope_s
        .and_then(|segundos| Duration::try_from_secs_f64(segundos).ok())
        .unwrap_or(tope_por_defecto);
    let transcurrido = epoch_segundos().saturating_sub(activo.pedido_ts);
    if Duration::from_secs(transcurrido) >= tope {
        let motivo = format!("tope de {}", describir_tope(tope));
        return parar_vigilado(cli, home, &activo, CAUSA_TOPE, &motivo);
    }
    let Some(repeticiones) = activo.repeticiones.filter(|_| !activo.ids.is_empty()) else {
        return Ok(Vigilancia::Vigente);
    };

    // Carpeta propia del vigía: la corrida que sigue viva es la que lo importa.
    let carpeta = home.join("tmp").join(format!("vigilar-{}", activo.sesion));
    fs::create_dir_all(&carpeta).map_err(|e| infra("carpeta del vigía", e))?;
    let sin_senal = Senales::default();
    let lim = Limites {
        cancelar: &sin_senal,
        inicio: Instant::now(),
        tope: TIMEOUT_LLAMADA,
    };
    let bajada = bajar_junits(
        cli,
        &lim,
        home,
        &activo.sesion,
        &carpeta,
        repeticiones,
        false,
    )?;
    let cumple = leer_junits(&bajada.rutas)
        .is_some_and(|textos| cumple_lo_pedido(&textos, &activo.ids, repeticiones));
    borrar_junits(home, &bajada.rutas);
    if cumple {
        return parar_vigilado(cli, home, &activo, CAUSA_REQUISITO, CAUSA_REQUISITO);
    }
    Ok(Vigilancia::Vigente)
}

/// Para el runtime anotado que decidió `vigilar` y registra su `Gate` con `accion` `vigilar`.
/// `activo.json` se borra solo si la parada se confirma. Si no, devuelve error y lo conserva.
fn parar_vigilado(
    cli: &dyn ColabCli,
    home: &Path,
    activo: &Activo,
    causa: &str,
    motivo: &str,
) -> Result<Vigilancia, ErrorOraculo> {
    let sin_senal = Senales::default();
    let parada = Parada {
        plazo: PLAZO_STOP_FALLIDO,
        cancelar: &sin_senal,
    };
    let confirmacion = confirmar_parada(cli, &activo.sesion, parada);
    let mut extra = json!({
        "accion": "vigilar",
        "sesion": activo.sesion,
        "pedido_ts": activo.pedido_ts,
        "gpu_pedida": activo.gpu_pedida,
        "causa": causa,
        "motivo": motivo,
        "confirmada": confirmacion.confirmada(),
    });
    if let Some(nota) = confirmacion.nota() {
        extra["nota"] = json!(nota);
    }
    registrar(home, CORRIDA_ANOTADA, extra)?;
    if !confirmacion.confirmada() {
        return Err(ErrorOraculo::Colab(format!(
            "no se pudo liberar el runtime anotado {}; revísalo en Colab y corré `r00t oraculo pendientes --liberar`",
            activo.sesion
        )));
    }
    borrar_activo(home)?;
    Ok(Vigilancia::Liberado {
        causa: causa.into(),
    })
}

/// Límites de una corrida: las señales y el tope de duración, contado desde su inicio.
struct Limites<'a> {
    cancelar: &'a Senales,
    inicio: Instant,
    tope: Duration,
}

impl Limites<'_> {
    fn restante(&self) -> Duration {
        self.tope.saturating_sub(self.inicio.elapsed())
    }

    fn vencido(&self) -> bool {
        self.inicio.elapsed() >= self.tope
    }
}

/// Nombre de sesión nuevo para la tarea: `root-<tarea>-<segundos>`, solo `[a-z0-9-]`.
pub fn nombre_sesion(tarea: &str) -> String {
    let segundos = epoch_segundos();
    let limpio: String = tarea
        .chars()
        .take(30)
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    format!("root-{limpio}-{segundos}")
}

/// Filtra el entorno: pasan solo las variables de la lista blanca y las `XDG_*`.
pub fn entorno_permitido<I>(vars: I) -> Vec<(OsString, OsString)>
where
    I: IntoIterator<Item = (OsString, OsString)>,
{
    vars.into_iter()
        .filter(|(clave, _)| es_permitida(clave))
        .collect()
}

/// Pide un runtime T4, verifica la GPU, sube el paquete, corre `correr.sh` y baja el junit.
/// Libera el runtime en todos los caminos. Registra dos `Gate` de runtime: pedir y liberar.
///
/// Antes de pedir, libera el runtime que quedó anotado en `activo.json`. `cancelar` y `tope`
/// cortan la corrida: al cortarla, Root libera el runtime y sale con error.
#[allow(clippy::too_many_arguments)]
pub fn correr_en_colab(
    cli: &dyn ColabCli,
    paquete: &Path,
    tarea: &Tarea,
    home: &Path,
    nombre_sesion: &str,
    cancelar: &Senales,
    tope: Duration,
    compilador: &dyn CompiladorLocal,
) -> Result<ResultadoColab, ErrorOraculo> {
    let inicio = Instant::now();
    validar_sesion(nombre_sesion)?;
    verificar_sin_claves(paquete)?;
    liberar_anotado(cli, home)?;
    let lim = Limites {
        cancelar,
        inicio,
        tope,
    };
    let tmp = home.join("tmp").join(format!("colab-{nombre_sesion}"));
    fs::create_dir_all(&tmp).map_err(|e| infra("carpeta temporal de Colab", e))?;
    let corrida = format!("colab-{}", tarea.nombre);
    let trabajo = Trabajo {
        paquete,
        tarea,
        corrida: &corrida,
        compilador,
    };
    escribir_activo(
        home,
        nombre_sesion,
        tope,
        &ids_de(tarea),
        tarea.repeticiones,
    )?;

    // La CLI no tiene un chequeo de login sin efectos: `new` hace de chequeo, con plazo corto.
    let nuevo = args(&["new", "-s", nombre_sesion, "--gpu", GPU_PEDIDA]);
    let plazo_nuevo = cli.plazo_primer_paso().min(lim.restante());
    match cli.ejecutar_con_plazo(&nuevo, plazo_nuevo, cancelar) {
        Ok(Espera::Termino(salida)) if salida.codigo == 0 => {}
        Ok(Espera::Termino(salida)) if parece_sin_sesion(&salida) => {
            borrar_activo(home)?;
            return Err(ErrorOraculo::Colab(MENSAJE_SIN_SESION.into()));
        }
        Ok(Espera::Plazo) if !lim.vencido() => {
            // Sin respuesta del primer paso: se pide la parada con plazo corto y se ignora su error.
            registrar_pedir(home, &corrida, nombre_sesion, "sin_respuesta")?;
            let sin_senal = Senales::default();
            let parada = Parada {
                plazo: PLAZO_STOP_FALLIDO,
                cancelar: &sin_senal,
            };
            let _ = liberar(
                cli,
                home,
                &corrida,
                nombre_sesion,
                "desconocida",
                Fin::Fallo(None),
                parada,
            );
            return Err(ErrorOraculo::Colab(MENSAJE_SIN_RESPUESTA.into()));
        }
        Err(e) => {
            // Ni siquiera se lanzó la CLI: no hay runtime que liberar.
            borrar_activo(home)?;
            return Err(e);
        }
        otro => {
            let causa = match otro {
                Ok(Espera::Termino(salida)) => format!(
                    "colab new salió con código {}: {}",
                    salida.codigo,
                    salida.stderr.trim()
                ),
                Ok(Espera::Senal(senal)) => format!("Interrumpido por {senal}"),
                _ => "colab no respondió a tiempo".to_string(),
            };
            // Un fallo que no es de login puede haber creado el runtime: se registra y se libera.
            let registro = registrar_pedir(home, &corrida, nombre_sesion, "fallo");
            let falla = ErrorOraculo::Infra(format!("no se pudo pedir el runtime: {causa}"));
            let error = cerrar_con_error(
                cli,
                &lim,
                home,
                &corrida,
                nombre_sesion,
                "desconocida",
                falla,
                Some(CAUSA_RUNTIME_NO_ASIGNADO),
            );
            return Err(sumar_error(error, registro));
        }
    }
    let registro = registrar_pedir(home, &corrida, nombre_sesion, "ok");

    let mut observado = Observado {
        gpu: String::from("desconocida"),
        causa: None,
    };
    let trabajo = match registro {
        Ok(()) => trabajar(
            cli,
            &lim,
            home,
            &tmp,
            nombre_sesion,
            &mut observado,
            &trabajo,
        ),
        Err(e) => Err(e),
    };
    match trabajo {
        Ok(corte) => {
            let parada = Parada {
                plazo: TIMEOUT_LLAMADA,
                cancelar: lim.cancelar,
            };
            match liberar(
                cli,
                home,
                &corrida,
                nombre_sesion,
                &observado.gpu,
                Fin::Bien(corte.causa),
                parada,
            ) {
                Ok(()) => Ok(ResultadoColab {
                    junits: corte.junits,
                    gpu: observado.gpu,
                    sesion: nombre_sesion.into(),
                }),
                // Una señal cortó la parada: se libera con su causa igual.
                Err(e) if lim.cancelar.recibida().is_some() => Err(cerrar_con_error(
                    cli,
                    &lim,
                    home,
                    &corrida,
                    nombre_sesion,
                    &observado.gpu,
                    e,
                    None,
                )),
                Err(e) => Err(e),
            }
        }
        Err(e) => {
            // Sin causa propia (GPU, señal o tope), el trabajo que falló es `error-en-corrida`.
            let causa = observado.causa.unwrap_or(CAUSA_ERROR_CORRIDA);
            Err(cerrar_con_error(
                cli,
                &lim,
                home,
                &corrida,
                nombre_sesion,
                &observado.gpu,
                e,
                Some(causa),
            ))
        }
    }
}

/// Libera el runtime anotado en `activo.json`, si hay uno, y devuelve su sesión.
/// Si la parada falla, conserva el archivo y devuelve error. Un `activo.json` ilegible se
/// renombra y se conserva; su sesión no llega a la CLI.
pub fn liberar_anotado(cli: &dyn ColabCli, home: &Path) -> Result<Option<String>, ErrorOraculo> {
    let ruta = ruta_activo(home);
    let texto = match fs::read_to_string(&ruta) {
        Ok(texto) => texto,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(infra("leer activo.json", e)),
    };
    let Some(activo) = leer_activo(&texto) else {
        apartar_ilegible(home)?;
        return Ok(None);
    };
    let sin_senal = Senales::default();
    let parada = Parada {
        plazo: PLAZO_STOP_FALLIDO,
        cancelar: &sin_senal,
    };
    let inexistente = match confirmar_parada(cli, &activo.sesion, parada) {
        Confirmacion::Confirmada => false,
        Confirmacion::SesionYaNoExistia => true,
        Confirmacion::Fallida => {
            return Err(ErrorOraculo::Colab(format!(
                "no se pudo liberar el runtime anotado {}; revísalo en Colab y corré `r00t oraculo pendientes --liberar`",
                activo.sesion
            )));
        }
    };
    let extra = json!({
        "accion": "liberar-huerfano",
        "sesion": activo.sesion,
        "pedido_ts": activo.pedido_ts,
        "gpu_pedida": activo.gpu_pedida,
        "parada": if inexistente { "sin_sesion" } else { "ok" },
    });
    registrar(home, CORRIDA_ANOTADA, extra)?;
    borrar_activo(home)?;
    Ok(Some(activo.sesion))
}

/// `true` si hay un `activo.json` que liberar.
pub fn hay_runtime_anotado(home: &Path) -> bool {
    ruta_activo(home).exists()
}

/// Texto del tope para el usuario: minutos si es un múltiplo exacto de minuto, si no segundos.
/// Un tope de menos de un segundo se dice en milisegundos (o en microsegundos si no llega a 1 ms).
pub fn describir_tope(tope: Duration) -> String {
    if tope < Duration::from_secs(1) {
        let milisegundos = tope.as_millis();
        return if milisegundos > 0 {
            format!("{milisegundos} ms")
        } else {
            format!("{} µs", tope.as_micros())
        };
    }
    let segundos = tope.as_secs();
    if segundos > 0 && segundos.is_multiple_of(60) {
        format!("{} minutos", segundos / 60)
    } else {
        format!("{segundos} segundos")
    }
}

fn ruta_activo(home: &Path) -> PathBuf {
    home.join(CARPETA_COLAB).join(ARCHIVO_ACTIVO)
}

fn epoch_segundos() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Anota la sesión pedida en `activo.json`, con su tope y lo que hay que cumplir para `vigilar`.
/// Escribe un temporal en la misma carpeta y lo renombra.
fn escribir_activo(
    home: &Path,
    sesion: &str,
    tope: Duration,
    ids: &[String],
    repeticiones: u32,
) -> Result<(), ErrorOraculo> {
    let carpeta = home.join(CARPETA_COLAB);
    fs::create_dir_all(&carpeta).map_err(|e| infra("carpeta colab", e))?;
    let texto = json!({
        "v": 1,
        "sesion": sesion,
        "pedido_ts": epoch_segundos(),
        "gpu_pedida": GPU_PEDIDA,
        "tope_s": tope.as_secs_f64(),
        "ids": ids,
        "repeticiones": repeticiones,
    })
    .to_string();
    let temporal = carpeta.join(format!("{ARCHIVO_ACTIVO}.tmp-{}", std::process::id()));
    fs::write(&temporal, texto).map_err(|e| infra("escribir activo.json", e))?;
    fs::rename(&temporal, ruta_activo(home)).map_err(|e| infra("anotar activo.json", e))
}

/// Borra `activo.json` con la guarda de `borrado`. Si no existe, no hace nada.
fn borrar_activo(home: &Path) -> Result<(), ErrorOraculo> {
    let ruta = ruta_activo(home);
    if !ruta.exists() {
        return Ok(());
    }
    crate::borrado::borrar_archivo_dentro(&home.join(CARPETA_COLAB), &ruta)
        .map_err(|e| infra("borrar activo.json", e))
}

/// Una sesión válida solo tiene letras ASCII, dígitos y guiones, y no pasa de 60 caracteres.
fn sesion_valida(nombre: &str) -> bool {
    !nombre.is_empty()
        && nombre.len() <= 60
        && nombre
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn validar_sesion(nombre: &str) -> Result<(), ErrorOraculo> {
    if sesion_valida(nombre) {
        Ok(())
    } else {
        Err(ErrorOraculo::Tarea(format!(
            "nombre de sesión inválido: {nombre:?}"
        )))
    }
}

/// Lee `activo.json`. `None` si el JSON no sirve, la versión no es 1 o la sesión no es válida.
fn leer_activo(texto: &str) -> Option<Activo> {
    let activo: Activo = serde_json::from_str(texto).ok()?;
    (activo.v == 1 && sesion_valida(&activo.sesion)).then_some(activo)
}

/// Renombra `activo.json` a `activo.json.ilegible` y registra un aviso. Nunca borra el archivo.
fn apartar_ilegible(home: &Path) -> Result<(), ErrorOraculo> {
    let carpeta = home.join(CARPETA_COLAB);
    let base = carpeta.join(format!("{ARCHIVO_ACTIVO}.ilegible"));
    let destino = if base.exists() {
        carpeta.join(format!("{ARCHIVO_ACTIVO}.ilegible.{}", epoch_segundos()))
    } else {
        base
    };
    fs::rename(ruta_activo(home), &destino).map_err(|e| infra("apartar activo.json", e))?;
    let extra = json!({
        "accion": "activo-ilegible",
        "archivo": destino.display().to_string(),
    });
    registrar(home, CORRIDA_ANOTADA, extra)
}

/// Cómo terminó el sondeo: los junit que se importan y la causa del `Gate` de liberación.
struct Corte {
    junits: Vec<PathBuf>,
    causa: &'static str,
}

/// Ids `archivo::nombre` que el junit debe traer: los tests de `[[entorno]]`, igual que el paquete.
fn ids_de(tarea: &Tarea) -> Vec<String> {
    tarea
        .entorno
        .iter()
        .map(|exclusion| exclusion.test.clone())
        .collect()
}

/// Lo que se ve del runtime mientras corre: la GPU que dijo tener y, si la corrida cortó por la GPU, la
/// causa del `Gate`.
struct Observado {
    gpu: String,
    causa: Option<&'static str>,
}

/// Lo que `trabajar` necesita del paquete, de la tarea y del compilador del respaldo de Rust.
struct Trabajo<'a> {
    paquete: &'a Path,
    tarea: &'a Tarea,
    corrida: &'a str,
    compilador: &'a dyn CompiladorLocal,
}

/// Lo que corre con el runtime ya pedido. Si falla, `correr_en_colab` libera el runtime.
fn trabajar(
    cli: &dyn ColabCli,
    lim: &Limites,
    home: &Path,
    tmp: &Path,
    sesion: &str,
    observado: &mut Observado,
    trabajo: &Trabajo,
) -> Result<Corte, ErrorOraculo> {
    let guion_gpu = escribir(tmp, "gpu.py", GUION_GPU)?;
    let lectura = exec(cli, lim, sesion, SEG_GPU, &guion_gpu)?;
    let leida = if lectura.codigo == 0 {
        marcador(&lectura.stdout, "ROOT_GPU=")
    } else {
        None
    };
    observado.gpu = leida.clone().unwrap_or_else(|| "desconocida".into());
    if !leida.as_deref().is_some_and(es_t4) {
        observado.causa = Some(CAUSA_GPU_NO_T4);
        return Err(ErrorOraculo::Colab(format!(
            "la GPU del runtime no es una T4 ({}); no se corrió ningún test",
            observado.gpu
        )));
    }

    let tar = empaquetar(trabajo.paquete, tmp)?;
    let subida = paso(
        cli,
        lim,
        &args(&["upload", "-s", sesion, &ruta(&tar), PAQUETE_REMOTO]),
        TIMEOUT_LLAMADA,
    );
    let _ = borrar_tar(home, &tar);
    let subida = subida?;
    if subida.codigo != 0 {
        return Err(ErrorOraculo::Infra(format!(
            "no se pudo subir el paquete: {}",
            subida.stderr.trim()
        )));
    }

    let guion_preparar = escribir(tmp, "prepara.py", GUION_PREPARAR)?;
    let preparado = exec(cli, lim, sesion, SEG_PREPARAR, &guion_preparar)?;
    if preparado.codigo != 0 || marcador(&preparado.stdout, "ROOT_PREPARADO=").is_none() {
        return Err(ErrorOraculo::Infra(format!(
            "la preparación del runtime falló: {}",
            preparado.stderr.trim()
        )));
    }

    preparar_rust(cli, lim, home, tmp, sesion, trabajo)?;

    let guion_correr = escribir(tmp, "corre.py", GUION_CORRER)?;
    let lanzada = exec(cli, lim, sesion, SEG_CORRER, &guion_correr)?;
    if lanzada.codigo != 0 || marcador(&lanzada.stdout, "ROOT_LANZADO=").is_none() {
        return Err(ErrorOraculo::Infra(format!(
            "correr.sh no arrancó en el runtime: {}",
            lanzada.stderr.trim()
        )));
    }

    // Sondeo: `correr.sh` sigue en el runtime. Corta en el corte 1 si los junit cumplen lo pedido, y
    // en el corte 2 si `correr.sh` terminó. El tope y las señales cortan en `esperar_sondeo`.
    // Sin verificar con la CLI real (nube sin colab): si `download` no puede leer durante la
    // corrida, el corte 1 se vuelve el corte 2.
    let guion_estado = escribir(tmp, "estado.py", GUION_ESTADO)?;
    let ids = ids_de(trabajo.tarea);
    let repeticiones = trabajo.tarea.repeticiones;
    loop {
        if lim.vencido() {
            return Err(ErrorOraculo::Infra("tope de la corrida".into()));
        }
        // a. Bajada de los junit: una repetición sin junit todavía no terminó.
        // b. Corte 1: los junit cumplen lo pedido. Se importan y se libera aunque `correr.sh` siga.
        let bajada = bajar_junits(cli, lim, home, sesion, tmp, repeticiones, false)?;
        if leer_junits(&bajada.rutas)
            .is_some_and(|textos| cumple_lo_pedido(&textos, &ids, repeticiones))
        {
            return Ok(Corte {
                junits: bajada.rutas,
                causa: CAUSA_REQUISITO,
            });
        }
        // c. Estado: `correr.sh` terminó. Corte 2 con los junit que haya; si no hay ninguno, error.
        if estado_terminado(cli, lim, sesion, &guion_estado)? {
            let bajada = bajar_junits(cli, lim, home, sesion, tmp, repeticiones, true)?;
            if bajada.rutas.is_empty() {
                let causa = bajada.causa.unwrap_or_else(|| "sin junit".into());
                return Err(ErrorOraculo::Infra(format!(
                    "no se pudo bajar {}: {causa}",
                    nombre_junit(1)
                )));
            }
            return Ok(Corte {
                junits: bajada.rutas,
                causa: CAUSA_TERMINO,
            });
        }
        // d. Sigue corriendo: espera el intervalo, mirando señales y el tope.
        esperar_sondeo(cli, lim)?;
    }
}

/// Lo que se bajó de los junit por repetición: las rutas de los que están completos, y la causa de la
/// última bajada que falló, si alguna falló.
struct Bajada {
    rutas: Vec<PathBuf>,
    causa: Option<String>,
}

/// Baja el junit de cada repetición a `carpeta`. Un junit ya bajado y completo no se vuelve a bajar:
/// una repetición que terminó no cambia. Con `todas` falso, la bajada para en la primera repetición sin
/// junit, porque las repeticiones corren en orden. `Err` solo si llegó una señal o se pasó el tope.
fn bajar_junits(
    cli: &dyn ColabCli,
    lim: &Limites,
    home: &Path,
    sesion: &str,
    carpeta: &Path,
    repeticiones: u32,
    todas: bool,
) -> Result<Bajada, ErrorOraculo> {
    let mut bajada = Bajada {
        rutas: Vec::new(),
        causa: None,
    };
    for n in 1..=repeticiones {
        let nombre = nombre_junit(n);
        let destino = carpeta.join(&nombre);
        if junit_completo(&destino) {
            bajada.rutas.push(destino);
            continue;
        }
        let fallo = match bajar_junit(cli, lim, home, sesion, &nombre, &destino)? {
            Ok(()) if junit_completo(&destino) => {
                bajada.rutas.push(destino);
                None
            }
            Ok(()) => {
                // Bajó a medias: se descarta y la próxima vuelta lo vuelve a bajar.
                let _ = crate::borrado::borrar_archivo_dentro(home, &destino);
                Some(format!("{nombre} no parsea"))
            }
            Err(causa) => Some(causa),
        };
        if let Some(causa) = fallo {
            bajada.causa = Some(causa);
            if !todas {
                break;
            }
        }
    }
    Ok(bajada)
}

/// `true` si `ruta` existe y su junit parsea: una repetición completa no cambia.
fn junit_completo(ruta: &Path) -> bool {
    fs::read_to_string(ruta).is_ok_and(|texto| junit::parsear(&texto).is_ok())
}

/// Lee los junit bajados. `None` si alguno no se puede leer.
fn leer_junits(rutas: &[PathBuf]) -> Option<Vec<String>> {
    rutas
        .iter()
        .map(|ruta| fs::read_to_string(ruta).ok())
        .collect()
}

/// Borra los junit bajados por la guarda de `borrado`.
fn borrar_junits(home: &Path, rutas: &[PathBuf]) {
    for ruta in rutas {
        let _ = crate::borrado::borrar_archivo_dentro(home, ruta);
    }
}

/// Baja el junit `nombre` de la carpeta remota a `destino`, sin dejar un junit viejo. `Ok(Ok(()))` si
/// bajó. `Ok(Err(causa))` si la bajada falló: es que todavía no hay junit. `Err` solo si llegó una señal
/// o se pasó el tope.
fn bajar_junit(
    cli: &dyn ColabCli,
    lim: &Limites,
    home: &Path,
    sesion: &str,
    nombre: &str,
    destino: &Path,
) -> Result<Result<(), String>, ErrorOraculo> {
    // Archivo propio de la corrida: un junit de un sondeo anterior no debe contar como nuevo.
    let _ = crate::borrado::borrar_archivo_dentro(home, destino);
    let remoto = format!("{CARPETA_REMOTA}/{nombre}");
    match paso(
        cli,
        lim,
        &args(&["download", "-s", sesion, &remoto, &ruta(destino)]),
        TIMEOUT_LLAMADA,
    ) {
        Ok(salida) if salida.codigo == 0 && destino.is_file() => Ok(Ok(())),
        Ok(salida) => Ok(Err(salida.stderr.trim().to_string())),
        Err(e) if lim.cancelar.recibida().is_some() || lim.vencido() => Err(e),
        Err(e) => Ok(Err(e.to_string())),
    }
}

/// Pregunta a `estado.py` si `correr.sh` terminó. `true` si el runtime ya tiene `estado.rc`.
fn estado_terminado(
    cli: &dyn ColabCli,
    lim: &Limites,
    sesion: &str,
    guion: &Path,
) -> Result<bool, ErrorOraculo> {
    let salida = exec(cli, lim, sesion, SEG_ESTADO, guion)?;
    if salida.codigo != 0 {
        return Err(ErrorOraculo::Infra(format!(
            "no se pudo leer el estado de correr.sh: {}",
            salida.stderr.trim()
        )));
    }
    match marcador(&salida.stdout, "ROOT_ESTADO=").as_deref() {
        Some("corriendo") => Ok(false),
        Some(otro) if otro.starts_with("terminado:") => Ok(true),
        _ => Err(ErrorOraculo::Infra("estado.py no dio resultado".into())),
    }
}

/// Espera el intervalo entre sondeos en pasos cortos. Una señal o el tope cortan la espera.
fn esperar_sondeo(cli: &dyn ColabCli, lim: &Limites) -> Result<(), ErrorOraculo> {
    let hasta = Instant::now() + cli.intervalo_sondeo();
    loop {
        if let Some(senal) = lim.cancelar.recibida() {
            return Err(ErrorOraculo::Colab(format!("Interrumpido por {senal}")));
        }
        if lim.vencido() {
            return Err(ErrorOraculo::Infra("tope de la corrida".into()));
        }
        let ahora = Instant::now();
        if ahora >= hasta {
            return Ok(());
        }
        thread::sleep(PASO_SONDEO.min(hasta - ahora));
    }
}

/// Prepara Rust en el runtime y registra el `Gate` `preparar-rust` con el camino que usó:
/// `rustup` o `ya-instalado` (el `rustup-init` verificado), `binario-local` (el respaldo que
/// corrió) o `pendiente`. Si Rust no queda listo, devuelve el error que deja los tests Pendiente.
fn preparar_rust(
    cli: &dyn ColabCli,
    lim: &Limites,
    home: &Path,
    tmp: &Path,
    sesion: &str,
    trabajo: &Trabajo,
) -> Result<(), ErrorOraculo> {
    let resultado = match rust_en_runtime(cli, lim, sesion, tmp) {
        Ok(camino) => Ok(camino),
        Err(causa_rust) => binario_local(cli, lim, sesion, tmp, trabajo)
            .map(|()| "binario-local")
            .map_err(|causa_respaldo| format!("{causa_rust}; respaldo: {causa_respaldo}")),
    };
    let (camino, causa) = match &resultado {
        Ok(camino) => (*camino, None),
        Err(causa) => ("pendiente", Some(causa.clone())),
    };
    registrar(
        home,
        trabajo.corrida,
        json!({"accion": "preparar-rust", "camino": camino, "causa": causa}),
    )?;
    resultado.map(|_| ()).map_err(|causa| {
        ErrorOraculo::Colab(format!(
            "Rust no quedó listo en Colab: {causa}; los tests siguen Pendiente"
        ))
    })
}

/// Corre `prepara-rust.py` en el runtime. Devuelve `rustup` o `ya-instalado`, o la causa de la falla.
fn rust_en_runtime(
    cli: &dyn ColabCli,
    lim: &Limites,
    sesion: &str,
    tmp: &Path,
) -> Result<&'static str, String> {
    let guion =
        escribir(tmp, "prepara-rust.py", &guion_rust(BASE_RUSTUP)).map_err(|e| e.to_string())?;
    let salida = exec(cli, lim, sesion, SEG_PREPARAR, &guion)
        .map_err(|e| format!("prepara-rust.py: {e}"))?;
    if salida.codigo != 0 {
        return Err(format!(
            "prepara-rust.py salió con código {}: {}",
            salida.codigo,
            salida.stderr.trim()
        ));
    }
    match marcador(&salida.stdout, "ROOT_RUST=").as_deref() {
        Some("ya") => Ok("ya-instalado"),
        Some("rustup") => Ok("rustup"),
        Some(otro) => Err(format!("prepara-rust.py: {otro}")),
        None => Err("prepara-rust.py no dio resultado".into()),
    }
}

/// Respaldo: compila el producto en la máquina local, sube el binario a `root-paquete/target/debug/` y el
/// `cargo` mínimo a `root-paquete/bin/`, y prueba el binario en el runtime. Si algo falla, devuelve la
/// causa y no se corre `corre.py`.
fn binario_local(
    cli: &dyn ColabCli,
    lim: &Limites,
    sesion: &str,
    tmp: &Path,
    trabajo: &Trabajo,
) -> Result<(), String> {
    // Una corrida ya cortada no compila ni sube nada: la parada la maneja `correr_en_colab`.
    if let Some(senal) = lim.cancelar.recibida() {
        return Err(format!("Interrumpido por {senal}"));
    }
    if lim.vencido() {
        return Err("tope de la corrida".into());
    }
    let repo = trabajo.paquete.join("repo");
    let nombre = nombre_binario(&repo).ok_or("sin nombre de binario")?;
    // El nombre entra en el `cargo` mínimo, que es un guion de shell: solo letras, números, - y _.
    let nombre_seguro = !nombre.is_empty()
        && nombre
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !nombre_seguro {
        return Err(format!("nombre de binario inválido: {nombre:?}"));
    }
    let binario = trabajo
        .compilador
        .compilar(&repo, trabajo.tarea)
        .map_err(|e| e.to_string())?;
    // El binario va donde el `cargo` mínimo lo busca: `target/debug` del paquete (CARGO_TARGET_DIR).
    let destino = format!("{CARPETA_REMOTA}/target/debug/{nombre}");
    let subida = paso(
        cli,
        lim,
        &args(&["upload", "-s", sesion, &ruta(&binario), &destino]),
        TIMEOUT_LLAMADA,
    )
    .map_err(|e| e.to_string())?;
    if subida.codigo != 0 {
        return Err(format!(
            "no se pudo subir el binario: {}",
            subida.stderr.trim()
        ));
    }
    let cargo = escribir(tmp, "cargo", &guion_cargo(&nombre)).map_err(|e| e.to_string())?;
    let destino_cargo = format!("{CARPETA_REMOTA}/bin/cargo");
    let subida_cargo = paso(
        cli,
        lim,
        &args(&["upload", "-s", sesion, &ruta(&cargo), &destino_cargo]),
        TIMEOUT_LLAMADA,
    )
    .map_err(|e| e.to_string())?;
    if subida_cargo.codigo != 0 {
        return Err(format!(
            "no se pudo subir el cargo mínimo: {}",
            subida_cargo.stderr.trim()
        ));
    }
    let guion = escribir(tmp, "prueba-binario.py", &guion_probar_binario(&nombre))
        .map_err(|e| e.to_string())?;
    let prueba = exec(cli, lim, sesion, SEG_PRUEBA_BINARIO, &guion).map_err(|e| e.to_string())?;
    match marcador(&prueba.stdout, "ROOT_BINARIO=").as_deref() {
        Some("ok") => Ok(()),
        Some(otro) => Err(format!("el binario no corre en Colab ({otro})")),
        None => Err(format!(
            "prueba-binario.py no dio resultado (código {})",
            prueba.codigo
        )),
    }
}

/// Llama a la CLI con el plazo dado, sin pasar de lo que queda del tope. Una señal o un plazo
/// vencido cortan la espera y devuelven error. El llamador decide la liberación.
fn paso(
    cli: &dyn ColabCli,
    lim: &Limites,
    args: &[String],
    plazo: Duration,
) -> Result<SalidaColab, ErrorOraculo> {
    let plazo = plazo.min(lim.restante());
    match cli.ejecutar_con_plazo(args, plazo, lim.cancelar)? {
        Espera::Termino(salida) => Ok(salida),
        Espera::Plazo => Err(ErrorOraculo::Infra(format!(
            "colab superó {} s",
            plazo.as_secs()
        ))),
        Espera::Senal(senal) => Err(ErrorOraculo::Colab(format!("Interrumpido por {senal}"))),
    }
}

/// Corre un script Python en el runtime con `exec -f`. El script es un archivo local de Root.
fn exec(
    cli: &dyn ColabCli,
    lim: &Limites,
    sesion: &str,
    segundos: u64,
    guion: &Path,
) -> Result<SalidaColab, ErrorOraculo> {
    paso(
        cli,
        lim,
        &args(&[
            "exec",
            "-s",
            sesion,
            "--timeout",
            &segundos.to_string(),
            "-f",
            &ruta(guion),
        ]),
        TIMEOUT_LLAMADA,
    )
}

/// Cierra una corrida que falló con el runtime pedido y devuelve el error a mostrar.
/// Una señal o el tope tienen su propio `Gate`, su motivo y su mensaje. Sin causa especial,
/// libera de forma normal: esa parada también la puede cortar una señal.
/// `causa` es la del `Gate` si el error no es una señal ni el tope (esos tienen su propia causa).
#[allow(clippy::too_many_arguments)]
fn cerrar_con_error(
    cli: &dyn ColabCli,
    lim: &Limites,
    home: &Path,
    corrida: &str,
    sesion: &str,
    gpu: &str,
    error: ErrorOraculo,
    causa: Option<&str>,
) -> ErrorOraculo {
    if let Some(causa_corte) = cerrar_por_causa(cli, lim, home, corrida, sesion, gpu) {
        return causa_corte;
    }
    let parada = Parada {
        plazo: TIMEOUT_LLAMADA,
        cancelar: lim.cancelar,
    };
    match liberar(cli, home, corrida, sesion, gpu, Fin::Fallo(causa), parada) {
        Ok(()) => error,
        Err(parada_fallida) => cerrar_por_causa(cli, lim, home, corrida, sesion, gpu)
            .unwrap_or_else(|| ErrorOraculo::Infra(format!("{error}; además, {parada_fallida}"))),
    }
}

/// Si llegó una señal o se pasó el tope, libera con esa causa y devuelve su error.
/// `None` si no hay causa especial.
fn cerrar_por_causa(
    cli: &dyn ColabCli,
    lim: &Limites,
    home: &Path,
    corrida: &str,
    sesion: &str,
    gpu: &str,
) -> Option<ErrorOraculo> {
    if let Some(senal) = lim.cancelar.recibida() {
        // La causa de una señal es su propio motivo.
        let motivo = format!("señal {senal}");
        let causa = Causa {
            causa: &motivo,
            motivo: &motivo,
        };
        return Some(cerrar_con_causa(
            cli,
            home,
            corrida,
            sesion,
            gpu,
            causa,
            format!("Interrumpido por {senal}: se liberó el runtime de Colab"),
        ));
    }
    if lim.vencido() {
        let texto = describir_tope(lim.tope);
        let motivo = format!("tope de {texto}");
        let causa = Causa {
            causa: CAUSA_TOPE,
            motivo: &motivo,
        };
        return Some(cerrar_con_causa(
            cli,
            home,
            corrida,
            sesion,
            gpu,
            causa,
            format!("Colab superó el tope de {texto}: se liberó el runtime"),
        ));
    }
    None
}

/// Por qué se libera tras una señal o un tope: la causa del `Gate` y el motivo con su detalle.
#[derive(Clone, Copy)]
struct Causa<'a> {
    causa: &'a str,
    motivo: &'a str,
}

/// Libera tras una señal o un tope. La parada no mira señales: ya llegó una, y debe terminar.
/// Su error no cambia el mensaje de la causa. Si no se confirma, conserva `activo.json` y lo dice.
fn cerrar_con_causa(
    cli: &dyn ColabCli,
    home: &Path,
    corrida: &str,
    sesion: &str,
    gpu: &str,
    causa: Causa,
    mensaje: String,
) -> ErrorOraculo {
    let sin_senal = Senales::default();
    let parada = Parada {
        plazo: PLAZO_STOP_FALLIDO,
        cancelar: &sin_senal,
    };
    let confirmacion = confirmar_parada(cli, sesion, parada);
    let mut extra = json!({
        "accion": "liberar",
        "motivo": causa.motivo,
        "causa": causa.causa,
        "sesion": sesion,
        "gpu_observada": gpu,
        "confirmada": confirmacion.confirmada(),
    });
    if let Some(nota) = confirmacion.nota() {
        extra["nota"] = json!(nota);
    }
    let mut texto = mensaje;
    if let Err(e) = registrar(home, corrida, extra) {
        texto = format!("{texto}; además, {e}");
    }
    if confirmacion.confirmada() {
        if let Err(e) = borrar_activo(home) {
            texto = format!("{texto}; además, {e}");
        }
    } else {
        texto = format!(
            "{texto}; no se confirmó la parada: revísalo en Colab y corré `r00t oraculo pendientes --liberar`"
        );
    }
    ErrorOraculo::Colab(texto)
}

/// Suma el error de registrar el `Gate` de pedir al error de la corrida.
fn sumar_error(error: ErrorOraculo, registro: Result<(), ErrorOraculo>) -> ErrorOraculo {
    match registro {
        Ok(()) => error,
        Err(e) => ErrorOraculo::Infra(format!("{error}; además, {e}")),
    }
}

/// Cómo terminó lo que se libera. `Bien` trae la causa del corte. `Fallo` trae la causa del error, si la hay.
#[derive(Clone, Copy)]
enum Fin<'a> {
    Bien(&'a str),
    Fallo(Option<&'a str>),
}

/// Para el runtime y registra el `Gate` de liberación. Con la parada confirmada, borra `activo.json`.
/// Devuelve error si la parada no se confirma: el archivo se conserva. El `Gate` lleva `causa` si
/// `fin` la trae.
fn liberar(
    cli: &dyn ColabCli,
    home: &Path,
    corrida: &str,
    sesion: &str,
    gpu: &str,
    fin: Fin,
    parada: Parada,
) -> Result<(), ErrorOraculo> {
    let confirmacion = confirmar_parada(cli, sesion, parada);
    let (trabajo, causa) = match fin {
        Fin::Bien(causa) => ("ok", Some(causa)),
        Fin::Fallo(causa) => ("fallo", causa),
    };
    let mut extra = json!({
        "accion": "liberar",
        "sesion": sesion,
        "gpu_observada": gpu,
        "trabajo": trabajo,
        "parada": if confirmacion.confirmada() { "ok" } else { "fallo" },
        "confirmada": confirmacion.confirmada(),
    });
    if let Some(causa) = causa {
        extra["causa"] = json!(causa);
    }
    if let Some(nota) = confirmacion.nota() {
        extra["nota"] = json!(nota);
    }
    registrar(home, corrida, extra)?;
    if !confirmacion.confirmada() {
        return Err(ErrorOraculo::Infra(format!(
            "no se pudo liberar el runtime {sesion}; revísalo en Colab"
        )));
    }
    borrar_activo(home)
}

/// Pide `colab stop` con una parada. Una señal de `parada.cancelar` la corta.
/// `None` si la CLI no terminó: por plazo o por señal.
fn parar(cli: &dyn ColabCli, sesion: &str, parada: Parada) -> Option<SalidaColab> {
    match cli.ejecutar_con_plazo(
        &args(&["stop", "-s", sesion]),
        parada.plazo,
        parada.cancelar,
    ) {
        Ok(Espera::Termino(salida)) => Some(salida),
        _ => None,
    }
}

/// Cómo quedó una parada de `colab stop`. Una sesión que ya no existe cuenta como confirmada:
/// el runtime ya no está.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Confirmacion {
    Confirmada,
    SesionYaNoExistia,
    Fallida,
}

impl Confirmacion {
    fn confirmada(self) -> bool {
        self != Confirmacion::Fallida
    }

    fn nota(self) -> Option<&'static str> {
        match self {
            Confirmacion::SesionYaNoExistia => Some("la sesión ya no existía"),
            _ => None,
        }
    }
}

/// Pide la parada y clasifica la respuesta. Cualquier otro fallo, o un plazo vencido, no confirma.
fn confirmar_parada(cli: &dyn ColabCli, sesion: &str, parada: Parada) -> Confirmacion {
    match parar(cli, sesion, parada) {
        Some(salida) if salida.codigo == 0 => Confirmacion::Confirmada,
        Some(salida) if parece_sesion_inexistente(&salida) => Confirmacion::SesionYaNoExistia,
        _ => Confirmacion::Fallida,
    }
}

/// Plazo y señales de una parada. Una parada normal mira las señales; tras una señal, no.
#[derive(Clone, Copy)]
struct Parada<'a> {
    plazo: Duration,
    cancelar: &'a Senales,
}

fn parece_sesion_inexistente(salida: &SalidaColab) -> bool {
    let texto = format!("{}\n{}", salida.stdout, salida.stderr).to_lowercase();
    MARCADORES_SESION_INEXISTENTE
        .iter()
        .any(|m| texto.contains(m))
}

/// El hijo lidera su propio grupo de procesos, para matar también a sus nietos.
#[cfg(unix)]
fn propio_grupo(orden: &mut Command) {
    use std::os::unix::process::CommandExt;
    orden.process_group(0);
}

#[cfg(not(unix))]
fn propio_grupo(_orden: &mut Command) {}

/// Mata al hijo y a su grupo de procesos, y lo recoge. Ningún nieto debe quedar colgado.
fn matar_arbol(hijo: &mut Child) {
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .arg("-KILL")
            .arg("--")
            .arg(format!("-{}", hijo.id()))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = hijo.kill();
    let _ = hijo.wait();
}

fn registrar_pedir(
    home: &Path,
    corrida: &str,
    sesion: &str,
    resultado: &str,
) -> Result<(), ErrorOraculo> {
    let extra = json!({
        "accion": "pedir",
        "sesion": sesion,
        "gpu_pedida": GPU_PEDIDA,
        "resultado": resultado,
    });
    registrar(home, corrida, extra)
}

fn registrar(home: &Path, corrida: &str, extra: Value) -> Result<(), ErrorOraculo> {
    let mapa = match extra {
        Value::Object(mapa) => mapa,
        _ => serde_json::Map::new(),
    };
    let evento = observe::evento_gate(corrida, FASE_RUNTIME, mapa);
    observe::registrar_en(&home.join(ARCHIVO_EVENTOS), &evento)
        .map_err(|e| infra("registrar el evento de runtime", e))
}

/// Empaqueta el paquete con el `tar` del sistema y argumentos en vector.
fn empaquetar(paquete: &Path, tmp: &Path) -> Result<PathBuf, ErrorOraculo> {
    let destino = tmp.join(PAQUETE_REMOTO);
    let estado = Command::new("tar")
        .arg("-czf")
        .arg(&destino)
        .arg("-C")
        .arg(paquete)
        .arg(".")
        .stdin(Stdio::null())
        .status()
        .map_err(|e| infra("lanzar tar", e))?;
    if !estado.success() {
        return Err(ErrorOraculo::Infra(
            "tar no pudo empaquetar el paquete".into(),
        ));
    }
    Ok(destino)
}

/// Borra el tar subido. Solo borra dentro de `<home>/tmp`, con la guarda de `borrado`.
fn borrar_tar(home: &Path, tar: &Path) -> io::Result<()> {
    crate::borrado::borrar_archivo_dentro(&home.join("tmp"), tar)
}

/// Antes de subir, el paquete no trae `.env` ni claves en los archivos que Root escribe.
fn verificar_sin_claves(paquete: &Path) -> Result<(), ErrorOraculo> {
    let mut pendientes = vec![paquete.to_path_buf()];
    while let Some(dir) = pendientes.pop() {
        let entradas = fs::read_dir(&dir).map_err(|e| infra("leer el paquete", e))?;
        for entrada in entradas {
            let ruta = entrada.map_err(|e| infra("leer el paquete", e))?.path();
            if ruta.is_dir() {
                pendientes.push(ruta);
                continue;
            }
            let nombre = ruta
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if nombre.starts_with(".env") {
                return Err(ErrorOraculo::Colab(
                    "el paquete trae un archivo .env; no se sube nada a Colab".into(),
                ));
            }
            if nombre == "correr.sh" || nombre == "LEEME.txt" {
                let texto = fs::read_to_string(&ruta).map_err(|e| infra("leer el paquete", e))?;
                if tiene_clave(&texto) {
                    return Err(ErrorOraculo::Colab(format!(
                        "{nombre} trae una clave; no se sube nada a Colab"
                    )));
                }
            }
        }
    }
    Ok(())
}

/// Una clave: el marcador `API_KEY`, o `sk-` seguido de 16 o más letras y dígitos.
fn tiene_clave(texto: &str) -> bool {
    if texto.contains("API_KEY") {
        return true;
    }
    texto.match_indices("sk-").any(|(i, _)| {
        texto[i + 3..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .count()
            >= 16
    })
}

fn buscar_colab() -> Result<PathBuf, ErrorOraculo> {
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let candidato = dir.join("colab");
            if candidato.is_file() {
                return Ok(candidato);
            }
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        let candidato = PathBuf::from(home).join(".local/bin/colab");
        if candidato.is_file() {
            return Ok(candidato);
        }
    }
    Err(ErrorOraculo::Infra(
        "no se encontró la CLI colab en PATH ni en ~/.local/bin; define ROOT_COLAB_BIN".into(),
    ))
}

fn es_permitida(clave: &OsString) -> bool {
    let Some(nombre) = clave.to_str() else {
        return false;
    };
    VARIABLES_PERMITIDAS.contains(&nombre) || nombre.starts_with(PREFIJO_PERMITIDO)
}

fn parece_sin_sesion(salida: &SalidaColab) -> bool {
    let texto = format!("{}\n{}", salida.stdout, salida.stderr).to_lowercase();
    MARCADORES_SIN_SESION.iter().any(|m| texto.contains(m))
}

/// Una GPU es T4 si una palabra del nombre es `T4`, sin distinguir mayúsculas.
fn es_t4(gpu: &str) -> bool {
    gpu.split(|c: char| !c.is_ascii_alphanumeric())
        .any(|palabra| palabra.eq_ignore_ascii_case("T4"))
}

/// Valor de la línea que empieza con `clave`. Vacío cuenta como ausente.
fn marcador(stdout: &str, clave: &str) -> Option<String> {
    stdout
        .lines()
        .find_map(|l| l.trim().strip_prefix(clave))
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn escribir(tmp: &Path, nombre: &str, texto: &str) -> Result<PathBuf, ErrorOraculo> {
    let ruta = tmp.join(nombre);
    fs::write(&ruta, texto).map_err(|e| infra(&format!("escribir {nombre}"), e))?;
    Ok(ruta)
}

fn leer_todo<R: Read>(lector: Option<R>) -> String {
    let mut bytes = Vec::new();
    if let Some(mut lector) = lector {
        let _ = lector.read_to_end(&mut bytes);
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

fn args(valores: &[&str]) -> Vec<String> {
    valores.iter().map(|v| (*v).to_string()).collect()
}

fn ruta(p: &Path) -> String {
    p.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oracle::tarea::{Comando, Exclusion};
    use std::cell::{Cell, RefCell};
    use std::os::unix::fs::PermissionsExt;
    use std::rc::Rc;

    use sha2::{Digest, Sha256};

    const JUNIT: &str = r#"<testsuite><testcase classname="tests.test_o" name="o2"/></testsuite>"#;

    type Respuesta = Box<dyn Fn(&[String]) -> SalidaColab>;

    /// Falso en memoria: registra cada llamada y responde con una función.
    struct Falso {
        llamadas: RefCell<Vec<Vec<String>>>,
        responde: Respuesta,
    }

    impl Falso {
        fn nuevo(responde: impl Fn(&[String]) -> SalidaColab + 'static) -> Self {
            Falso {
                llamadas: RefCell::new(Vec::new()),
                responde: Box::new(responde),
            }
        }

        fn lineas(&self) -> Vec<String> {
            self.llamadas.borrow().iter().map(|a| a.join(" ")).collect()
        }
    }

    impl ColabCli for Falso {
        fn ejecutar_con_plazo(
            &self,
            args: &[String],
            _plazo: Duration,
            _cancelar: &Senales,
        ) -> Result<Espera, ErrorOraculo> {
            self.llamadas.borrow_mut().push(args.to_vec());
            Ok(Espera::Termino((self.responde)(args)))
        }

        fn intervalo_sondeo(&self) -> Duration {
            Duration::from_millis(5)
        }
    }

    /// Tope holgado para los tests que no lo prueban.
    const TOPE_HOLGADO: Duration = Duration::from_secs(3600);

    /// `correr_en_colab` sin señal y con tope holgado.
    fn correr_prueba(
        cli: &dyn ColabCli,
        paquete: &Path,
        tarea: &Tarea,
        home: &Path,
        sesion: &str,
    ) -> Result<ResultadoColab, ErrorOraculo> {
        correr_en_colab(
            cli,
            paquete,
            tarea,
            home,
            sesion,
            &Senales::default(),
            TOPE_HOLGADO,
            &CompiladorPodman,
        )
    }

    fn salida(codigo: i32, stdout: &str, stderr: &str) -> SalidaColab {
        SalidaColab {
            codigo,
            stdout: stdout.into(),
            stderr: stderr.into(),
        }
    }

    /// Respuestas de una corrida buena. La GPU se elige con `gpu`.
    fn feliz(gpu: &'static str) -> impl Fn(&[String]) -> SalidaColab {
        move |args: &[String]| match args[0].as_str() {
            "new" | "stop" | "upload" => salida(0, "", ""),
            "download" => {
                fs::write(args.last().expect("destino"), JUNIT).expect("escribe");
                salida(0, "", "")
            }
            "exec" => {
                let archivo = args.iter().rev().find(|a| a.ends_with(".py")).cloned();
                match archivo.as_deref() {
                    Some(f) if f.ends_with("gpu.py") => salida(0, &format!("ROOT_GPU={gpu}"), ""),
                    Some(f) if f.ends_with("prepara.py") => salida(0, "ROOT_PREPARADO=1", ""),
                    Some(f) if f.ends_with("prepara-rust.py") => salida(0, "ROOT_RUST=rustup", ""),
                    Some(f) if f.ends_with("prueba-binario.py") => salida(0, "ROOT_BINARIO=ok", ""),
                    Some(f) if f.ends_with("corre.py") => salida(0, "ROOT_LANZADO=1", ""),
                    Some(f) if f.ends_with("estado.py") => salida(0, "ROOT_ESTADO=terminado:0", ""),
                    _ => salida(2, "", "archivo desconocido"),
                }
            }
            _ => salida(2, "", "comando desconocido"),
        }
    }

    fn tarea() -> Tarea {
        Tarea {
            nombre: "memora".into(),
            lenguaje: "rust".into(),
            imagen: "x".into(),
            containerfile: None,
            spec: "s".into(),
            repeticiones: 1,
            repeticiones_inestable: 10,
            contrato: Comando {
                cmd: "x".into(),
                reintento: None,
            },
            oculto: Comando {
                cmd: "x".into(),
                reintento: None,
            },
            entorno: vec![Exclusion {
                test: "test_o::o2".into(),
                motivo: "gpu".into(),
            }],
            oraculo_dudoso: Vec::new(),
            requisito: Vec::new(),
            arquitectura: None,
            eficiencia: None,
            pruebas: None,
            entrega: None,
            escenario: Vec::new(),
            calibracion: None,
            dir: PathBuf::new(),
        }
    }

    /// Paquete mínimo con los archivos que Root escribe.
    fn paquete(base: &Path) -> PathBuf {
        let dir = base.join("paquete");
        fs::create_dir_all(dir.join("repo")).expect("dir");
        fs::create_dir_all(dir.join("tests")).expect("dir");
        fs::write(dir.join("repo/a.txt"), "x").expect("escribe");
        fs::write(dir.join("tests/test_o.py"), "x").expect("escribe");
        fs::write(dir.join("correr.sh"), "#!/usr/bin/env bash\n").expect("escribe");
        fs::write(dir.join("LEEME.txt"), "pasos\n").expect("escribe");
        fs::write(
            dir.join("repo/Cargo.toml"),
            "[package]\nname = \"memora\"\n",
        )
        .expect("escribe");
        dir
    }

    fn eventos_runtime(home: &Path) -> Vec<Value> {
        fs::read_to_string(home.join(ARCHIVO_EVENTOS))
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str(l).expect("JSON"))
            .filter(|e: &Value| e["fase"] == FASE_RUNTIME)
            .collect()
    }

    #[test]
    fn colab_nombre_de_sesion_solo_usa_alfanumericos_y_guion() {
        let nombre = nombre_sesion("memora_x.y");
        assert!(nombre.starts_with("root-memora-x-y-"), "{nombre}");
        assert!(validar_sesion(&nombre).is_ok());
    }

    #[test]
    fn colab_sesion_invalida_no_llega_a_la_cli() {
        let base = tempfile::tempdir().expect("base");
        let home = tempfile::tempdir().expect("home");
        let falso = Falso::nuevo(feliz("Tesla T4"));
        let r = correr_prueba(
            &falso,
            &paquete(base.path()),
            &tarea(),
            home.path(),
            "x;rm -rf",
        );
        assert!(matches!(r, Err(ErrorOraculo::Tarea(_))));
        assert!(falso.lineas().is_empty());
    }

    #[test]
    fn colab_entorno_permitido_descarta_claves() {
        let vars = vec![
            (OsString::from("PATH"), OsString::from("/bin")),
            (OsString::from("DEEPSEEK_API_KEY"), OsString::from("sk-x")),
            (OsString::from("ANTHROPIC_API_KEY"), OsString::from("a")),
            (OsString::from("XDG_CONFIG_HOME"), OsString::from("/c")),
            (OsString::from("HOME"), OsString::from("/h")),
        ];
        let claves: Vec<String> = entorno_permitido(vars)
            .into_iter()
            .map(|(k, _)| k.to_string_lossy().into_owned())
            .collect();
        assert_eq!(claves, vec!["PATH", "XDG_CONFIG_HOME", "HOME"]);
    }

    #[test]
    fn colab_paquete_con_clave_en_leeme_no_se_sube() {
        let base = tempfile::tempdir().expect("base");
        let home = tempfile::tempdir().expect("home");
        let dir = paquete(base.path());
        fs::write(dir.join("LEEME.txt"), "OPENAI_API_KEY=abc\n").expect("escribe");
        let falso = Falso::nuevo(feliz("Tesla T4"));
        let r = correr_prueba(&falso, &dir, &tarea(), home.path(), "root-memora-1");
        assert!(matches!(r, Err(ErrorOraculo::Colab(_))));
        assert!(falso.lineas().is_empty(), "nada llega a Colab");
    }

    #[test]
    fn colab_borrar_tar_rechaza_ruta_fuera_de_home_tmp_sin_borrar() {
        let base = tempfile::tempdir().expect("temp dir");
        let home = base.path().join("home");
        let fuera = base.path().join("fuera");
        fs::create_dir_all(home.join("tmp")).expect("home/tmp");
        fs::create_dir_all(&fuera).expect("fuera");
        fs::write(fuera.join("paquete.tar"), "x").expect("escribe");
        assert!(borrar_tar(&home, &fuera.join("paquete.tar")).is_err());
        assert!(fuera.join("paquete.tar").exists());
    }

    #[test]
    fn colab_borrar_tar_dentro_de_home_tmp_lo_borra() {
        let home = tempfile::tempdir().expect("home");
        let tar = home.path().join("tmp").join("colab-x").join("paquete.tar");
        fs::create_dir_all(tar.parent().expect("padre")).expect("dir");
        fs::write(&tar, "x").expect("escribe");
        borrar_tar(home.path(), &tar).expect("borra");
        assert!(!tar.exists());
    }

    #[test]
    fn colab_clave_sk_solo_cuenta_con_forma_de_clave() {
        assert!(!tiene_clave("ruta /x/desk-chair-y/z"));
        assert!(tiene_clave("token sk-abcdefghijklmnop1234 fin"));
    }

    #[test]
    fn colab_sin_sesion_da_mensaje_exacto_y_no_sube_nada() {
        let base = tempfile::tempdir().expect("base");
        let home = tempfile::tempdir().expect("home");
        let falso = Falso::nuevo(|_: &[String]| salida(1, "", "Not logged in, run login"));
        let r = correr_prueba(
            &falso,
            &paquete(base.path()),
            &tarea(),
            home.path(),
            "root-memora-1",
        );
        match r {
            Err(ErrorOraculo::Colab(m)) => assert_eq!(m, MENSAJE_SIN_SESION),
            otro => panic!("esperaba el mensaje de sesión, llegó {otro:?}"),
        }
        assert_eq!(falso.lineas().len(), 1, "solo el intento de new");
        assert!(eventos_runtime(home.path()).is_empty());
    }

    #[test]
    fn colab_falla_a_mitad_libera_el_runtime() {
        let base = tempfile::tempdir().expect("base");
        let home = tempfile::tempdir().expect("home");
        let bueno = feliz("Tesla T4");
        let falso = Falso::nuevo(move |args: &[String]| {
            if args[0] == "exec" && args.iter().any(|a| a.ends_with("corre.py")) {
                return salida(1, "", "boom");
            }
            bueno(args)
        });
        let r = correr_prueba(
            &falso,
            &paquete(base.path()),
            &tarea(),
            home.path(),
            "root-memora-1",
        );
        assert!(matches!(r, Err(ErrorOraculo::Infra(_))));
        let lineas = falso.lineas();
        assert!(
            lineas.last().is_some_and(|l| l.starts_with("stop -s")),
            "{lineas:?}"
        );
        let gates = eventos_runtime(home.path());
        assert!(gates.iter().any(|g| g["extra"]["accion"] == "liberar"));
    }

    #[test]
    fn colab_gpu_no_t4_libera_y_no_sube() {
        let base = tempfile::tempdir().expect("base");
        let home = tempfile::tempdir().expect("home");
        let falso = Falso::nuevo(feliz("NVIDIA A100-SXM4-40GB"));
        let r = correr_prueba(
            &falso,
            &paquete(base.path()),
            &tarea(),
            home.path(),
            "root-memora-1",
        );
        match r {
            Err(ErrorOraculo::Colab(m)) => assert_eq!(
                m,
                "la GPU del runtime no es una T4 (NVIDIA A100-SXM4-40GB); no se corrió ningún test"
            ),
            otro => panic!("esperaba error de GPU, llegó {otro:?}"),
        }
        let lineas = falso.lineas();
        assert!(!lineas.iter().any(|l| l.starts_with("upload")));
        assert!(lineas.last().is_some_and(|l| l.starts_with("stop -s")));
    }

    #[test]
    fn colab_tope_de_30_minutos_dice_tope_de_duracion_y_motivo_30() {
        let home = tempfile::tempdir().expect("home");
        let falso = Falso::nuevo(|_: &[String]| salida(0, "", ""));
        let lim = Limites {
            cancelar: &Senales::default(),
            inicio: Instant::now()
                .checked_sub(Duration::from_secs(31 * 60))
                .expect("un instante 31 minutos atrás"),
            tope: Duration::from_secs(30 * 60),
        };
        let error = cerrar_por_causa(
            &falso,
            &lim,
            home.path(),
            "colab-memora",
            "root-memora-1",
            "T4",
        );
        assert!(matches!(error, Some(ErrorOraculo::Colab(_))), "{error:?}");
        let gates = eventos_runtime(home.path());
        assert!(
            gates.iter().any(|g| g["extra"]["accion"] == "liberar"
                && g["extra"]["causa"] == "tope de duración"
                && g["extra"]["motivo"] == "tope de 30 minutos"),
            "{gates:?}"
        );
    }

    #[test]
    fn colab_gpu_no_t4_gate_lleva_causa_gpu_no_t4() {
        let base = tempfile::tempdir().expect("base");
        let home = tempfile::tempdir().expect("home");
        let falso = Falso::nuevo(feliz("NVIDIA A100-SXM4-40GB"));
        let _ = correr_prueba(
            &falso,
            &paquete(base.path()),
            &tarea(),
            home.path(),
            "root-memora-1",
        );
        let gates = eventos_runtime(home.path());
        assert!(
            gates
                .iter()
                .any(|g| g["extra"]["accion"] == "liberar" && g["extra"]["causa"] == "gpu-no-t4"),
            "{gates:?}"
        );
    }

    #[test]
    fn colab_fallo_al_pedir_runtime_gate_lleva_causa_runtime_no_asignado() {
        let base = tempfile::tempdir().expect("base");
        let home = tempfile::tempdir().expect("home");
        let falso = Falso::nuevo(|args: &[String]| {
            if args[0] == "new" {
                salida(1, "", "no capacity for T4")
            } else {
                salida(0, "", "")
            }
        });
        let r = correr_prueba(
            &falso,
            &paquete(base.path()),
            &tarea(),
            home.path(),
            "root-memora-1",
        );
        assert!(r.is_err(), "{r:?}");
        let gates = eventos_runtime(home.path());
        assert!(
            gates.iter().any(|g| g["extra"]["accion"] == "liberar"
                && g["extra"]["causa"] == "runtime-no-asignado"),
            "{gates:?}"
        );
    }

    #[test]
    fn colab_error_a_mitad_de_corrida_gate_lleva_causa_error_en_corrida() {
        let base = tempfile::tempdir().expect("base");
        let home = tempfile::tempdir().expect("home");
        let bueno = feliz("Tesla T4");
        let falso = Falso::nuevo(move |args: &[String]| {
            if args[0] == "exec" && args.iter().any(|a| a.ends_with("corre.py")) {
                return salida(1, "", "boom");
            }
            bueno(args)
        });
        let r = correr_prueba(
            &falso,
            &paquete(base.path()),
            &tarea(),
            home.path(),
            "root-memora-1",
        );
        assert!(matches!(r, Err(ErrorOraculo::Infra(_))), "{r:?}");
        let gates = eventos_runtime(home.path());
        assert!(
            gates
                .iter()
                .any(|g| g["extra"]["accion"] == "liberar"
                    && g["extra"]["causa"] == "error-en-corrida"),
            "{gates:?}"
        );
    }

    /// Corre el `cargo` mínimo de `correr.sh` con `bash`, en `dir`, con `bin/cargo` escrito allí.
    fn correr_cargo_minimo(dir: &Path, orden: &[&str]) -> std::process::ExitStatus {
        let ruta = dir.join("bin").join("cargo");
        fs::create_dir_all(dir.join("bin")).expect("carpeta bin");
        fs::write(&ruta, guion_cargo("brasa")).expect("escribe cargo");
        // La salida se captura: el mensaje de error del `cargo` mínimo no llena la salida de los tests.
        Command::new("bash")
            .arg(&ruta)
            .args(orden)
            .current_dir(dir)
            .env_remove("CARGO_TARGET_DIR")
            .output()
            .expect("bash corre el cargo mínimo")
            .status
    }

    /// Crea `target/debug/brasa` dentro de `dir`, como deja el paquete del respaldo.
    fn poner_binario(dir: &Path) {
        let debug = dir.join("target").join("debug");
        fs::create_dir_all(&debug).expect("target/debug");
        fs::write(debug.join("brasa"), "binario").expect("binario");
    }

    #[test]
    fn colab_cargo_minimo_build_bin_con_binario_sale_0() {
        let base = tempfile::tempdir().expect("base");
        poner_binario(base.path());
        let estado = correr_cargo_minimo(base.path(), &["build", "-q", "--bin", "brasa"]);
        assert!(estado.success(), "{estado:?}");
    }

    #[test]
    fn colab_cargo_minimo_build_bin_sin_binario_falla() {
        let base = tempfile::tempdir().expect("base");
        let estado = correr_cargo_minimo(base.path(), &["build", "-q", "--bin", "brasa"]);
        assert!(!estado.success(), "{estado:?}");
    }

    #[test]
    fn colab_cargo_minimo_build_release_falla_aun_con_binario() {
        let base = tempfile::tempdir().expect("base");
        poner_binario(base.path());
        let estado = correr_cargo_minimo(base.path(), &["build", "--release"]);
        assert!(!estado.success(), "{estado:?}");
    }

    #[test]
    fn colab_gpu_ilegible_cuenta_como_desconocida() {
        let base = tempfile::tempdir().expect("base");
        let home = tempfile::tempdir().expect("home");
        let bueno = feliz("Tesla T4");
        let falso = Falso::nuevo(move |args: &[String]| {
            if args.iter().any(|a| a.ends_with("gpu.py")) {
                return salida(1, "", "sin nvidia-smi");
            }
            bueno(args)
        });
        let r = correr_prueba(
            &falso,
            &paquete(base.path()),
            &tarea(),
            home.path(),
            "root-memora-1",
        );
        match r {
            Err(ErrorOraculo::Colab(m)) => assert!(m.contains("(desconocida)"), "{m}"),
            otro => panic!("esperaba error de GPU, llegó {otro:?}"),
        }
    }

    #[test]
    fn colab_t4_registra_pedir_y_liberar_y_devuelve_el_junit() {
        let base = tempfile::tempdir().expect("base");
        let home = tempfile::tempdir().expect("home");
        let falso = Falso::nuevo(feliz("Tesla T4"));
        let r = correr_prueba(
            &falso,
            &paquete(base.path()),
            &tarea(),
            home.path(),
            "root-memora-1",
        )
        .expect("corrida buena");
        assert_eq!(r.gpu, "Tesla T4");
        assert_eq!(r.sesion, "root-memora-1");
        assert!(r.junits.iter().all(|j| j.is_file()));
        let gates = eventos_runtime(home.path());
        assert_eq!(gates.len(), 3, "pedir, preparar-rust y liberar: {gates:?}");
        assert_eq!(gates[0]["extra"]["gpu_pedida"], "T4");
        assert_eq!(gates[1]["extra"]["accion"], "preparar-rust");
        assert_eq!(gates[2]["extra"]["gpu_observada"], "Tesla T4");
    }

    #[test]
    fn colab_es_t4_exige_palabra_exacta() {
        assert!(es_t4("Tesla T4"));
        assert!(es_t4("tesla t4"));
        assert!(!es_t4("NVIDIA A100-SXM4-40GB"));
        assert!(!es_t4("T40"));
        assert!(!es_t4(""));
    }

    /// Script de una CLI que no responde a `new`. Anota su PID y duerme. `stop` sale al instante.
    fn falso_mudo(dir: &Path) -> PathBuf {
        let guion = format!(
            "#!/bin/sh\necho \"$*\" >> '{d}/llamadas.log'\ncase \"$1\" in\nnew) echo $$ > '{d}/pid.txt'; sleep 300 ;;\n*) exit 0 ;;\nesac\n",
            d = dir.display()
        );
        let ruta = dir.join("colab-mudo");
        fs::write(&ruta, guion).expect("escribe el falso");
        fs::set_permissions(&ruta, fs::Permissions::from_mode(0o755)).expect("permisos");
        ruta
    }

    /// Vivo si `kill -0` lo acepta. Llama a la CLI `kill` con argumentos, sin shell.
    fn proceso_vivo(pid: i32) -> bool {
        Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }

    #[test]
    fn colab_plazo_primer_paso_es_60_segundos() {
        assert_eq!(PLAZO_PRIMER_PASO, Duration::from_secs(60));
    }

    #[test]
    fn colab_primer_paso_sin_respuesta_sale_con_login_y_libera() {
        let base = tempfile::tempdir().expect("base");
        let home = tempfile::tempdir().expect("home");
        let binario = falso_mudo(base.path());
        let cli = ColabProceso {
            binario,
            plazo_primer_paso: Duration::from_secs(1),
        };
        let inicio = Instant::now();
        let r = correr_prueba(
            &cli,
            &paquete(base.path()),
            &tarea(),
            home.path(),
            "root-memora-1",
        );
        assert!(
            inicio.elapsed() < Duration::from_secs(5),
            "tardó {:?}",
            inicio.elapsed()
        );
        match r {
            Err(ErrorOraculo::Colab(m)) => {
                assert!(m.contains("60 segundos") && m.contains("login"), "{m}");
            }
            otro => panic!("esperaba error de Colab con login, llegó {otro:?}"),
        }
        let log = fs::read_to_string(base.path().join("llamadas.log")).expect("llamadas.log");
        let lineas: Vec<&str> = log.lines().collect();
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
        let pid: i32 = fs::read_to_string(base.path().join("pid.txt"))
            .expect("pid.txt")
            .trim()
            .parse()
            .expect("PID numérico");
        assert!(!proceso_vivo(pid), "la CLI falsa quedó colgada");
    }

    #[test]
    fn colab_primer_paso_cierra_stdin() {
        let base = tempfile::tempdir().expect("base");
        let binario = base.path().join("colab-stdin");
        let guion = format!(
            "#!/bin/sh\ncat > '{}/stdin.txt'\nexit 0\n",
            base.path().display()
        );
        fs::write(&binario, guion).expect("escribe");
        fs::set_permissions(&binario, fs::Permissions::from_mode(0o755)).expect("permisos");
        let cli = ColabProceso {
            binario,
            plazo_primer_paso: Duration::from_secs(2),
        };
        let espera = cli
            .ejecutar_con_plazo(
                &args(&["new", "-s", "root-memora-1"]),
                Duration::from_secs(1),
                &Senales::default(),
            )
            .expect("sin error");
        let Espera::Termino(salida) = espera else {
            panic!("la CLI debía terminar a tiempo: {espera:?}");
        };
        assert_eq!(salida.codigo, 0);
        let leido = fs::read_to_string(base.path().join("stdin.txt")).expect("stdin.txt");
        assert_eq!(leido, "", "la CLI leyó algo de stdin");
    }

    /// Script de una CLI cuyo `correr.sh` nunca termina: `corre.py` lanza y `estado.py` dice
    /// `corriendo`. Anota su PID en `pid.txt`. `stop_cuerpo` es el cuerpo del caso `stop`.
    fn falso_corre_lenta_stop(dir: &Path, stop_cuerpo: &str) -> PathBuf {
        let guion = format!(
            "#!/bin/sh\necho \"$*\" >> '{d}/llamadas.log'\ncase \"$1\" in\nexec)\n  case \"$*\" in\n    *gpu.py*) echo 'ROOT_GPU=Tesla T4' ;;\n    *prepara.py*) echo ROOT_PREPARADO=1 ;;\n    *prepara-rust.py*) echo ROOT_RUST=rustup ;;\n    *corre.py*) echo ROOT_LANZADO=1 ;;\n    *estado.py*) echo $$ > '{d}/pid.txt'; echo 'ROOT_ESTADO=corriendo' ;;\n  esac ;;\nstop) {stop_cuerpo} ;;\n*) exit 0 ;;\nesac\nexit 0\n",
            d = dir.display()
        );
        let ruta = dir.join("colab-lenta");
        fs::write(&ruta, guion).expect("escribe el falso");
        fs::set_permissions(&ruta, fs::Permissions::from_mode(0o755)).expect("permisos");
        ruta
    }

    /// Como `falso_corre_lenta_stop`, con `stop` que sale 0.
    fn falso_corre_lenta(dir: &Path) -> PathBuf {
        falso_corre_lenta_stop(dir, "exit 0")
    }

    /// Escribe un `activo.json` de prueba en `home/colab`.
    fn escribir_activo_de_prueba(home: &Path, texto: &str) -> PathBuf {
        let carpeta = home.join("colab");
        fs::create_dir_all(&carpeta).expect("carpeta colab");
        let ruta = carpeta.join("activo.json");
        fs::write(&ruta, texto).expect("escribe activo.json");
        ruta
    }

    const ACTIVO_VIEJO: &str = r#"{"v":1,"sesion":"vieja","pedido_ts":1,"gpu_pedida":"T4"}"#;

    #[test]
    fn colab_activo_se_escribe_antes_de_new() {
        let base = tempfile::tempdir().expect("base");
        let home = tempfile::tempdir().expect("home");
        let ruta = home.path().join("colab").join("activo.json");
        let visto: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
        let visto_en = Rc::clone(&visto);
        let ruta_en = ruta.clone();
        let bueno = feliz("Tesla T4");
        let falso = Falso::nuevo(move |args: &[String]| {
            if args[0] == "new" {
                *visto_en.borrow_mut() = fs::read_to_string(&ruta_en).ok();
            }
            bueno(args)
        });
        let r = correr_prueba(
            &falso,
            &paquete(base.path()),
            &tarea(),
            home.path(),
            "root-memora-1",
        );
        assert!(r.is_ok(), "{r:?}");
        let texto = visto
            .borrow()
            .clone()
            .expect("activo.json debe existir al pedir el runtime");
        let json: Value = serde_json::from_str(&texto).expect("JSON");
        assert_eq!(json["v"], 1);
        assert_eq!(json["sesion"], "root-memora-1");
        assert_eq!(json["gpu_pedida"], "T4");
        assert!(json["pedido_ts"].as_u64().is_some_and(|t| t > 0), "{json}");
        assert!(
            !ruta.exists(),
            "tras una corrida exitosa no queda activo.json"
        );
    }

    #[test]
    fn colab_huerfano_se_libera_antes_de_new() {
        let base = tempfile::tempdir().expect("base");
        let home = tempfile::tempdir().expect("home");
        let ruta = escribir_activo_de_prueba(home.path(), ACTIVO_VIEJO);
        let falso = Falso::nuevo(feliz("Tesla T4"));
        let r = correr_prueba(
            &falso,
            &paquete(base.path()),
            &tarea(),
            home.path(),
            "root-memora-1",
        );
        assert!(r.is_ok(), "{r:?}");
        let lineas = falso.lineas();
        let i_stop = lineas
            .iter()
            .position(|l| l == "stop -s vieja")
            .unwrap_or_else(|| panic!("falta el stop de la sesión vieja: {lineas:?}"));
        let i_new = lineas
            .iter()
            .position(|l| l.starts_with("new -s"))
            .expect("new");
        assert!(i_stop < i_new, "{lineas:?}");
        let gates = eventos_runtime(home.path());
        assert!(
            gates
                .iter()
                .any(|g| g["extra"]["accion"] == "liberar-huerfano"
                    && g["extra"]["sesion"] == "vieja"),
            "{gates:?}"
        );
        assert!(!ruta.exists());
    }

    #[test]
    fn colab_liberar_anotado_sin_archivo_no_llama_a_la_cli() {
        let home = tempfile::tempdir().expect("home");
        let falso = Falso::nuevo(feliz("Tesla T4"));
        let r = liberar_anotado(&falso, home.path());
        assert_eq!(r.expect("sin error"), None);
        assert!(falso.lineas().is_empty());
    }

    #[test]
    fn colab_liberar_anotado_cli_falla_conserva_archivo() {
        let home = tempfile::tempdir().expect("home");
        let ruta = escribir_activo_de_prueba(home.path(), ACTIVO_VIEJO);
        let falso = Falso::nuevo(|_: &[String]| salida(1, "", "boom"));
        let r = liberar_anotado(&falso, home.path());
        assert!(r.is_err(), "{r:?}");
        assert!(
            ruta.exists(),
            "sin parada confirmada el archivo se conserva"
        );
    }

    #[test]
    fn colab_liberar_anotado_sesion_inexistente_se_ignora_y_borra() {
        let home = tempfile::tempdir().expect("home");
        let ruta = escribir_activo_de_prueba(home.path(), ACTIVO_VIEJO);
        let falso = Falso::nuevo(|_: &[String]| salida(1, "", "session does not exist"));
        let r = liberar_anotado(&falso, home.path());
        assert_eq!(r.expect("sin error"), Some("vieja".to_string()));
        assert!(!ruta.exists());
    }

    #[test]
    fn colab_liberar_anotado_stop_con_not_found_conserva_activo() {
        // "not found" no prueba que la sesión ya no exista: la salida es desconocida y la parada no se confirma.
        let home = tempfile::tempdir().expect("home");
        let ruta = escribir_activo_de_prueba(home.path(), ACTIVO_VIEJO);
        let falso = Falso::nuevo(|_: &[String]| salida(1, "", "session not found"));
        let r = liberar_anotado(&falso, home.path());
        assert!(r.is_err(), "{r:?}");
        assert!(
            ruta.exists(),
            "una salida con 'not found' conserva activo.json"
        );
    }

    #[test]
    fn colab_activo_con_sesion_invalida_se_renombra_sin_llamar_a_la_cli() {
        let home = tempfile::tempdir().expect("home");
        let texto = r#"{"v":1,"sesion":"a b;rm","pedido_ts":1,"gpu_pedida":"T4"}"#;
        let ruta = escribir_activo_de_prueba(home.path(), texto);
        let falso = Falso::nuevo(feliz("Tesla T4"));
        let r = liberar_anotado(&falso, home.path());
        assert_eq!(r.expect("sin error"), None);
        assert!(
            falso.lineas().is_empty(),
            "la sesión inválida no llega a la CLI"
        );
        assert!(!ruta.exists());
        let ilegible = home.path().join("colab").join("activo.json.ilegible");
        assert_eq!(fs::read_to_string(ilegible).expect("conservado"), texto);
    }

    #[test]
    fn colab_activo_roto_se_renombra_y_se_conserva() {
        let home = tempfile::tempdir().expect("home");
        let ruta = escribir_activo_de_prueba(home.path(), "{no es json");
        let falso = Falso::nuevo(feliz("Tesla T4"));
        let r = liberar_anotado(&falso, home.path());
        assert_eq!(r.expect("sin error"), None);
        assert!(falso.lineas().is_empty());
        assert!(!ruta.exists());
        let ilegible = home.path().join("colab").join("activo.json.ilegible");
        assert_eq!(
            fs::read_to_string(ilegible).expect("conservado"),
            "{no es json"
        );
    }

    #[test]
    fn colab_senal_libera_con_gate_y_borra_activo() {
        let base = tempfile::tempdir().expect("base");
        let home = tempfile::tempdir().expect("home");
        let binario = falso_corre_lenta(base.path());
        let cli = ColabProceso {
            binario,
            plazo_primer_paso: Duration::from_secs(1),
        };
        let senales = Senales::default();
        let senal_en_hilo = senales.clone();
        let log = base.path().join("llamadas.log");
        thread::spawn(move || {
            // Espera a que la corrida esté sondeando (`estado.py`) y manda SIGINT simulado.
            loop {
                let texto = fs::read_to_string(&log).unwrap_or_default();
                if texto.contains("estado.py") {
                    break;
                }
                thread::sleep(Duration::from_millis(20));
            }
            senal_en_hilo.activar("SIGINT");
        });
        let inicio = Instant::now();
        let r = correr_en_colab(
            &cli,
            &paquete(base.path()),
            &tarea(),
            home.path(),
            "root-memora-1",
            &senales,
            TOPE_HOLGADO,
            &CompiladorPodman,
        );
        assert!(
            inicio.elapsed() < Duration::from_secs(10),
            "tardó {:?}",
            inicio.elapsed()
        );
        match r {
            Err(ErrorOraculo::Colab(m)) => {
                assert_eq!(m, "Interrumpido por SIGINT: se liberó el runtime de Colab");
            }
            otro => panic!("esperaba la interrupción, llegó {otro:?}"),
        }
        let lineas = fs::read_to_string(base.path().join("llamadas.log")).expect("log");
        assert!(lineas.lines().any(|l| l.starts_with("stop -s")), "{lineas}");
        let gates = eventos_runtime(home.path());
        assert!(
            gates.iter().any(|g| g["extra"]["accion"] == "liberar"
                && g["extra"]["motivo"]
                    .as_str()
                    .is_some_and(|m| m.contains("SIGINT"))
                && g["extra"]["causa"] == "señal SIGINT"),
            "{gates:?}"
        );
        assert!(!home.path().join("colab").join("activo.json").exists());
        let pid: i32 = fs::read_to_string(base.path().join("pid.txt"))
            .expect("pid.txt")
            .trim()
            .parse()
            .expect("PID");
        assert!(!proceso_vivo(pid), "la CLI falsa quedó viva");
    }

    #[test]
    fn colab_tope_libera_con_gate_y_mensaje() {
        let base = tempfile::tempdir().expect("base");
        let home = tempfile::tempdir().expect("home");
        let binario = falso_corre_lenta(base.path());
        let cli = ColabProceso {
            binario,
            plazo_primer_paso: Duration::from_secs(1),
        };
        let inicio = Instant::now();
        let r = correr_en_colab(
            &cli,
            &paquete(base.path()),
            &tarea(),
            home.path(),
            "root-memora-1",
            &Senales::default(),
            Duration::from_secs(2),
            &CompiladorPodman,
        );
        assert!(
            inicio.elapsed() < Duration::from_secs(10),
            "tardó {:?}",
            inicio.elapsed()
        );
        match r {
            Err(ErrorOraculo::Colab(m)) => {
                assert_eq!(
                    m,
                    "Colab superó el tope de 2 segundos: se liberó el runtime"
                );
            }
            otro => panic!("esperaba el tope, llegó {otro:?}"),
        }
        let lineas = fs::read_to_string(base.path().join("llamadas.log")).expect("log");
        assert!(lineas.lines().any(|l| l.starts_with("stop -s")), "{lineas}");
        let gates = eventos_runtime(home.path());
        assert!(
            gates.iter().any(|g| g["extra"]["accion"] == "liberar"
                && g["extra"]["motivo"] == "tope de 2 segundos"
                && g["extra"]["causa"] == "tope de duración"),
            "{gates:?}"
        );
        assert!(!home.path().join("colab").join("activo.json").exists());
        let pid: i32 = fs::read_to_string(base.path().join("pid.txt"))
            .expect("pid.txt")
            .trim()
            .parse()
            .expect("PID");
        assert!(!proceso_vivo(pid), "la CLI falsa quedó viva");
    }

    /// Compilador falso: cuenta sus llamadas y devuelve siempre el mismo binario.
    struct CompiladorFalso {
        llamadas: Cell<u32>,
        binario: PathBuf,
    }

    impl CompiladorLocal for CompiladorFalso {
        fn compilar(&self, _repo: &Path, _tarea: &Tarea) -> Result<PathBuf, ErrorOraculo> {
            self.llamadas.set(self.llamadas.get() + 1);
            Ok(self.binario.clone())
        }
    }

    /// Un archivo que hace de binario compilado.
    fn binario_falso(base: &Path) -> PathBuf {
        let ruta = base.join("memora-compilado");
        fs::write(&ruta, "binario").expect("escribe el binario");
        ruta
    }

    /// Responde `stdout` al `exec` del guion que termina en `guion`, y `base` al resto.
    fn cambiando(
        base: impl Fn(&[String]) -> SalidaColab + 'static,
        guion: &'static str,
        stdout: &'static str,
    ) -> impl Fn(&[String]) -> SalidaColab + 'static {
        move |args: &[String]| {
            if args[0] == "exec" && args.last().is_some_and(|a| a.ends_with(guion)) {
                salida(0, stdout, "")
            } else {
                base(args)
            }
        }
    }

    /// Fragmentos que ningún guion de Root puede tener: nada de descargar y ejecutar por tubería.
    const PROHIBIDOS: [&str; 4] = ["| sh", "|sh", "sh.rustup.rs", "shell=True"];

    /// Los fragmentos prohibidos que aparecen en `texto`.
    fn prohibidos_en(texto: &str) -> Vec<&'static str> {
        PROHIBIDOS
            .iter()
            .copied()
            .filter(|p| texto.contains(p))
            .collect()
    }

    /// Los `Gate` de `preparar-rust` de la corrida.
    fn gates_rust(home: &Path) -> Vec<Value> {
        eventos_runtime(home)
            .into_iter()
            .filter(|g| g["extra"]["accion"] == "preparar-rust")
            .collect()
    }

    /// `python3` según el PATH del test. Sin él, el test falla con un mensaje claro.
    fn python3_del_path() -> PathBuf {
        std::env::var_os("PATH")
            .and_then(|p| {
                std::env::split_paths(&p)
                    .map(|d| d.join("python3"))
                    .find(|c| c.is_file())
            })
            .expect("python3 debe estar en el PATH para correr el guion de Rust")
    }

    /// Corre `guion_rust` con `python3` y devuelve su salida. `base` se sirve con `file://`.
    /// El PATH es mínimo y HOME apunta a una carpeta vacía: no hay `cargo` que encontrar.
    fn correr_guion_rust(base: &Path) -> String {
        let guiones = tempfile::tempdir().expect("guiones");
        let archivo = guiones.path().join("prepara-rust.py");
        fs::write(&archivo, guion_rust(&format!("file://{}", base.display())))
            .expect("escribe el guion");
        let trabajo = tempfile::tempdir().expect("trabajo");
        let home = tempfile::tempdir().expect("home");
        let salida = Command::new(python3_del_path())
            .arg(&archivo)
            .current_dir(trabajo.path())
            .env_clear()
            .env("HOME", home.path())
            .env("PATH", "/usr/bin:/bin")
            .output()
            .expect("no se pudo correr python3");
        assert_eq!(
            salida.status.code(),
            Some(0),
            "stderr: {}",
            String::from_utf8_lossy(&salida.stderr)
        );
        String::from_utf8_lossy(&salida.stdout).into_owned()
    }

    #[test]
    fn colab_rust_hash_malo_no_corre_rustup_y_usa_el_binario() {
        let base = tempfile::tempdir().expect("base");
        let home = tempfile::tempdir().expect("home");
        let binario = binario_falso(base.path());
        let guiones: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        let guiones_en = Rc::clone(&guiones);
        let respuesta = cambiando(feliz("Tesla T4"), "prepara-rust.py", "ROOT_RUST=falla:hash");
        let falso = Falso::nuevo(move |args: &[String]| {
            if args[0] == "exec" {
                let guion = args.last().expect("ruta del guion");
                let texto = fs::read_to_string(guion).expect("el guion se puede leer");
                guiones_en.borrow_mut().push(texto);
            }
            respuesta(args)
        });
        let compilador = CompiladorFalso {
            llamadas: Cell::new(0),
            binario,
        };
        let r = correr_en_colab(
            &falso,
            &paquete(base.path()),
            &tarea(),
            home.path(),
            "root-memora-1",
            &Senales::default(),
            TOPE_HOLGADO,
            &compilador,
        );
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(compilador.llamadas.get(), 1, "compila una vez");

        let lineas = falso.lineas();
        let i_subida = lineas
            .iter()
            .position(|l| {
                l.starts_with("upload -s root-memora-1 ")
                    && l.ends_with(" root-paquete/target/debug/memora")
            })
            .unwrap_or_else(|| panic!("falta la subida del binario: {lineas:?}"));
        let i_cargo = lineas
            .iter()
            .position(|l| {
                l.starts_with("upload -s root-memora-1 ") && l.ends_with(" root-paquete/bin/cargo")
            })
            .unwrap_or_else(|| panic!("falta la subida del cargo mínimo: {lineas:?}"));
        let i_prueba = lineas
            .iter()
            .position(|l| l.contains("prueba-binario.py"))
            .unwrap_or_else(|| panic!("falta prueba-binario.py: {lineas:?}"));
        let i_corre = lineas
            .iter()
            .position(|l| l.contains("corre.py"))
            .unwrap_or_else(|| panic!("falta corre.py: {lineas:?}"));
        assert!(i_subida < i_prueba && i_prueba < i_corre, "{lineas:?}");
        assert!(
            i_cargo < i_prueba,
            "el cargo mínimo se sube antes de la prueba: {lineas:?}"
        );

        let gates = gates_rust(home.path());
        assert_eq!(gates.len(), 1, "{gates:?}");
        assert_eq!(gates[0]["extra"]["camino"], "binario-local", "{gates:?}");
        assert!(gates[0]["extra"]["causa"].is_null(), "{gates:?}");

        for guion in guiones.borrow().iter() {
            assert!(
                prohibidos_en(guion).is_empty(),
                "un guion trae texto prohibido {:?}:\n{guion}",
                prohibidos_en(guion)
            );
        }
    }

    #[test]
    fn colab_rust_hash_bueno_corre_rustup_y_no_compila() {
        let base = tempfile::tempdir().expect("base");
        let home = tempfile::tempdir().expect("home");
        let compilador = CompiladorFalso {
            llamadas: Cell::new(0),
            binario: binario_falso(base.path()),
        };
        let falso = Falso::nuevo(feliz("Tesla T4"));
        let r = correr_en_colab(
            &falso,
            &paquete(base.path()),
            &tarea(),
            home.path(),
            "root-memora-1",
            &Senales::default(),
            TOPE_HOLGADO,
            &compilador,
        );
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(
            compilador.llamadas.get(),
            0,
            "no compila en la máquina local"
        );
        assert!(falso.lineas().iter().any(|l| l.contains("corre.py")));
        let gates = gates_rust(home.path());
        assert_eq!(gates.len(), 1, "{gates:?}");
        assert_eq!(gates[0]["extra"]["camino"], "rustup", "{gates:?}");
    }

    #[test]
    fn colab_rust_binario_que_no_corre_queda_pendiente() {
        let base = tempfile::tempdir().expect("base");
        let home = tempfile::tempdir().expect("home");
        let respuesta = cambiando(
            cambiando(
                feliz("Tesla T4"),
                "prepara-rust.py",
                "ROOT_RUST=falla:descarga",
            ),
            "prueba-binario.py",
            "ROOT_BINARIO=falla:126",
        );
        let falso = Falso::nuevo(respuesta);
        let compilador = CompiladorFalso {
            llamadas: Cell::new(0),
            binario: binario_falso(base.path()),
        };
        let r = correr_en_colab(
            &falso,
            &paquete(base.path()),
            &tarea(),
            home.path(),
            "root-memora-1",
            &Senales::default(),
            TOPE_HOLGADO,
            &compilador,
        );
        match r {
            Err(ErrorOraculo::Colab(m)) => assert!(
                m.starts_with("Rust no quedó listo en Colab: ")
                    && m.contains("los tests siguen Pendiente"),
                "{m}"
            ),
            otro => panic!("esperaba el error de Rust pendiente, llegó {otro:?}"),
        }
        let lineas = falso.lineas();
        assert!(
            !lineas
                .iter()
                .any(|l| l.starts_with("exec") && l.contains("corre.py")),
            "no debe correr corre.py: {lineas:?}"
        );
        assert!(
            lineas.last().is_some_and(|l| l.starts_with("stop -s")),
            "el runtime se libera: {lineas:?}"
        );
        let gates = gates_rust(home.path());
        assert_eq!(gates.len(), 1, "{gates:?}");
        assert_eq!(gates[0]["extra"]["camino"], "pendiente", "{gates:?}");
        assert!(
            gates[0]["extra"]["causa"]
                .as_str()
                .is_some_and(|c| c.contains("falla:126")),
            "{gates:?}"
        );
    }

    #[test]
    fn colab_rust_compilador_real_de_la_nube_queda_pendiente() {
        let base = tempfile::tempdir().expect("base");
        let home = tempfile::tempdir().expect("home");
        let falso = Falso::nuevo(cambiando(
            feliz("Tesla T4"),
            "prepara-rust.py",
            "ROOT_RUST=falla:hash",
        ));
        let r = correr_en_colab(
            &falso,
            &paquete(base.path()),
            &tarea(),
            home.path(),
            "root-memora-1",
            &Senales::default(),
            TOPE_HOLGADO,
            &CompiladorPodman,
        );
        match r {
            Err(ErrorOraculo::Colab(m)) => assert!(m.contains("Pendiente"), "{m}"),
            otro => panic!("esperaba el error de Rust pendiente, llegó {otro:?}"),
        }
        let lineas = falso.lineas();
        assert!(
            !lineas
                .iter()
                .any(|l| l.starts_with("exec") && l.contains("corre.py")),
            "{lineas:?}"
        );
    }

    #[test]
    fn colab_ningun_guion_usa_curl_pipe_sh() {
        let rust = guion_rust(BASE_RUSTUP);
        let prueba = guion_probar_binario("memora");
        let guiones: [(&str, &str); 5] = [
            ("gpu", GUION_GPU),
            ("prepara", GUION_PREPARAR),
            ("corre", GUION_CORRER),
            ("rust", rust.as_str()),
            ("prueba-binario", prueba.as_str()),
        ];
        for (nombre, texto) in guiones {
            assert!(
                prohibidos_en(texto).is_empty(),
                "el guion {nombre} trae {:?}",
                prohibidos_en(texto)
            );
        }
    }

    #[test]
    fn colab_guion_rust_hash_malo_no_ejecuta_rustup_init() {
        let base = tempfile::tempdir().expect("base");
        let marca = base.path().join("marca");
        fs::write(
            base.path().join("rustup-init"),
            format!("#!/bin/sh\ntouch '{}'\n", marca.display()),
        )
        .expect("escribe rustup-init");
        fs::write(
            base.path().join("rustup-init.sha256"),
            format!("{}  rustup-init\n", "0".repeat(64)),
        )
        .expect("escribe el sha256");
        let salida = correr_guion_rust(base.path());
        assert!(salida.contains("ROOT_RUST=falla:hash"), "{salida}");
        assert!(
            !marca.exists(),
            "rustup-init corrió con un hash que no coincide"
        );
    }

    #[test]
    fn colab_guion_rust_hash_bueno_ejecuta_rustup_init() {
        let base = tempfile::tempdir().expect("base");
        let marca = base.path().join("marca");
        fs::write(
            base.path().join("rustup-init"),
            format!("#!/bin/sh\ntouch '{}'\n", marca.display()),
        )
        .expect("escribe rustup-init");
        let contenido = fs::read(base.path().join("rustup-init")).expect("lee rustup-init");
        let hash: String = Sha256::digest(&contenido)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        fs::write(
            base.path().join("rustup-init.sha256"),
            format!("{hash}  rustup-init\n"),
        )
        .expect("escribe el sha256");
        let salida = correr_guion_rust(base.path());
        assert!(salida.contains("ROOT_RUST=rustup"), "{salida}");
        assert!(marca.exists(), "rustup-init debía correr con el hash bueno");
    }

    #[test]
    fn colab_nombre_binario_sale_de_cargo_toml() {
        let base = tempfile::tempdir().expect("base");
        let con = |texto: &str| {
            fs::write(base.path().join("Cargo.toml"), texto).expect("escribe Cargo.toml");
            nombre_binario(base.path())
        };
        assert_eq!(
            con("[package]\nname = \"memora\"\n").as_deref(),
            Some("memora")
        );
        assert_eq!(
            con("[package]\nname = \"memora\"\n[[bin]]\nname = \"r\"\npath = \"src/main.rs\"\n")
                .as_deref(),
            Some("r")
        );
        assert_eq!(con("[workspace]\n"), None);
        assert_eq!(con("[package]\nname = \"mem ora\"\n"), None);
    }

    #[test]
    fn colab_describir_tope_minutos_o_segundos() {
        assert_eq!(describir_tope(Duration::from_secs(120)), "2 minutos");
        assert_eq!(describir_tope(Duration::from_secs(1800)), "30 minutos");
        assert_eq!(describir_tope(Duration::from_secs(2)), "2 segundos");
    }

    #[test]
    fn colab_describir_tope_menos_de_un_segundo_dice_milisegundos() {
        assert_eq!(describir_tope(Duration::from_millis(600)), "600 ms");
    }

    #[test]
    fn colab_senal_antes_de_lanzar_no_ejecuta_la_cli() {
        let base = tempfile::tempdir().expect("base");
        let marca = base.path().join("lanzado");
        let binario = base.path().join("colab-marca");
        fs::write(
            &binario,
            format!("#!/bin/sh\necho x > '{}'\nexit 0\n", marca.display()),
        )
        .expect("escribe");
        fs::set_permissions(&binario, fs::Permissions::from_mode(0o755)).expect("permisos");
        let cli = ColabProceso {
            binario,
            plazo_primer_paso: Duration::from_secs(1),
        };
        let senales = Senales::default();
        senales.activar("SIGTERM");
        let espera = cli
            .ejecutar_con_plazo(
                &args(&["new", "-s", "root-memora-1"]),
                Duration::from_secs(5),
                &senales,
            )
            .expect("sin error");
        assert_eq!(espera, Espera::Senal("SIGTERM"));
        assert!(
            !marca.exists(),
            "la CLI no debe lanzarse con una señal pendiente"
        );
    }

    #[test]
    fn colab_senal_con_sesion_inexistente_confirma_y_borra() {
        let base = tempfile::tempdir().expect("base");
        let home = tempfile::tempdir().expect("home");
        let binario =
            falso_corre_lenta_stop(base.path(), "echo 'session does not exist' >&2; exit 1");
        let cli = ColabProceso {
            binario,
            plazo_primer_paso: Duration::from_secs(1),
        };
        let senales = Senales::default();
        let senal_en_hilo = senales.clone();
        let log = base.path().join("llamadas.log");
        thread::spawn(move || {
            loop {
                if fs::read_to_string(&log)
                    .unwrap_or_default()
                    .contains("estado.py")
                {
                    break;
                }
                thread::sleep(Duration::from_millis(20));
            }
            senal_en_hilo.activar("SIGINT");
        });
        let r = correr_en_colab(
            &cli,
            &paquete(base.path()),
            &tarea(),
            home.path(),
            "root-memora-1",
            &senales,
            TOPE_HOLGADO,
            &CompiladorPodman,
        );
        match r {
            Err(ErrorOraculo::Colab(m)) => {
                assert_eq!(m, "Interrumpido por SIGINT: se liberó el runtime de Colab");
            }
            otro => panic!("esperaba la interrupción, llegó {otro:?}"),
        }
        let gates = eventos_runtime(home.path());
        assert!(
            gates.iter().any(|g| g["extra"]["accion"] == "liberar"
                && g["extra"]["confirmada"] == true
                && g["extra"]["nota"] == "la sesión ya no existía"),
            "{gates:?}"
        );
        assert!(!home.path().join("colab").join("activo.json").exists());
    }

    /// Primer argumento `.py` de un `exec`: el guion que corre el runtime.
    fn guion_de(args: &[String]) -> Option<String> {
        args.iter().rev().find(|a| a.ends_with(".py")).cloned()
    }

    /// Segundos desde la época, para fechar `activo.json` en los tests.
    fn ahora_s() -> u64 {
        epoch_segundos()
    }

    /// Un `activo.json` de la versión nueva, con el pedido en `pedido_ts`.
    fn activo_nuevo(sesion: &str, pedido_ts: u64, ids: &[&str]) -> String {
        json!({
            "v": 1,
            "sesion": sesion,
            "pedido_ts": pedido_ts,
            "gpu_pedida": "T4",
            "ids": ids,
            "repeticiones": 1,
        })
        .to_string()
    }

    #[test]
    fn colab_corte_temprano_junit_completo_con_correr_vivo() {
        let base = tempfile::tempdir().expect("base");
        let home = tempfile::tempdir().expect("home");
        let bueno = feliz("Tesla T4");
        // `estado.py` siempre dice `corriendo`. El tope de seguridad del falso evita colgar el test
        // si Root no corta: en ese caso el `causa` de abajo falla en vez de quedarse esperando.
        let vueltas = Rc::new(Cell::new(0u32));
        let vueltas_en = Rc::clone(&vueltas);
        let falso = Falso::nuevo(move |args: &[String]| {
            if guion_de(args).is_some_and(|g| g.ends_with("estado.py")) {
                vueltas_en.set(vueltas_en.get() + 1);
                return if vueltas_en.get() < 50 {
                    salida(0, "ROOT_ESTADO=corriendo", "")
                } else {
                    salida(0, "ROOT_ESTADO=terminado:0", "")
                };
            }
            bueno(args)
        });
        let resultado = correr_prueba(
            &falso,
            &paquete(base.path()),
            &tarea(),
            home.path(),
            "root-memora-1",
        )
        .expect("corta por requisito cumplido");
        assert_eq!(resultado.junits.len(), 1, "una repetición, un junit");
        assert!(
            crate::oracle::junit::leer(&resultado.junits[0]).is_ok(),
            "el junit debe ser importable"
        );
        let lineas = falso.lineas();
        assert!(
            lineas.last().is_some_and(|l| l.starts_with("stop -s")),
            "{lineas:?}"
        );
        assert!(
            vueltas.get() < 50,
            "estado.py no debía hacer falta: {lineas:?}"
        );
        let gates = eventos_runtime(home.path());
        assert!(
            gates.iter().any(|g| g["extra"]["accion"] == "liberar"
                && g["extra"]["causa"] == "requisito cumplido"),
            "{gates:?}"
        );
    }

    #[test]
    fn colab_junit_sin_todas_las_repeticiones_no_corta_antes() {
        let base = tempfile::tempdir().expect("base");
        let home = tempfile::tempdir().expect("home");
        let mut t = tarea();
        t.repeticiones = 2;
        // Un testcase por id, y `repeticiones = 2`: el junit no cumple lo pedido.
        let vueltas = Rc::new(Cell::new(0u32));
        let vueltas_en = Rc::clone(&vueltas);
        let falso = Falso::nuevo(move |args: &[String]| match args[0].as_str() {
            "new" | "stop" | "upload" => salida(0, "", ""),
            // Solo la repetición 1 tiene junit: la 2 todavía no terminó.
            "download" if args[3].ends_with("pendientes-1.xml") => {
                fs::write(args.last().expect("destino"), JUNIT).expect("escribe");
                salida(0, "", "")
            }
            "download" => salida(1, "", "todavía no hay junit"),
            "exec" => match guion_de(args).as_deref() {
                Some(g) if g.ends_with("gpu.py") => salida(0, "ROOT_GPU=Tesla T4", ""),
                Some(g) if g.ends_with("prepara.py") => salida(0, "ROOT_PREPARADO=1", ""),
                Some(g) if g.ends_with("prepara-rust.py") => salida(0, "ROOT_RUST=rustup", ""),
                Some(g) if g.ends_with("prueba-binario.py") => salida(0, "ROOT_BINARIO=ok", ""),
                Some(g) if g.ends_with("corre.py") => salida(0, "ROOT_LANZADO=1", ""),
                Some(g) if g.ends_with("estado.py") => {
                    let n = vueltas_en.get();
                    vueltas_en.set(n + 1);
                    if n < 3 {
                        salida(0, "ROOT_ESTADO=corriendo", "")
                    } else {
                        salida(0, "ROOT_ESTADO=terminado:0", "")
                    }
                }
                _ => salida(2, "", "archivo desconocido"),
            },
            _ => salida(2, "", "comando desconocido"),
        });
        let resultado = correr_prueba(
            &falso,
            &paquete(base.path()),
            &t,
            home.path(),
            "root-memora-1",
        )
        .expect("corta cuando correr.sh termina");
        assert!(resultado.junits.iter().all(|j| j.is_file()));
        let lineas = falso.lineas();
        let descargas = lineas.iter().filter(|l| l.starts_with("download")).count();
        assert!(descargas >= 3, "hubo {descargas} descargas: {lineas:?}");
        assert_eq!(vueltas.get(), 4, "{lineas:?}");
        let gates = eventos_runtime(home.path());
        assert!(
            gates
                .iter()
                .any(|g| g["extra"]["accion"] == "liberar"
                    && g["extra"]["causa"] == "correr.sh terminó"),
            "{gates:?}"
        );
    }

    #[test]
    fn colab_cumple_lo_pedido_casos() {
        let ids = vec!["test_o::o2".to_string()];
        let uno = r#"<testsuite><testcase classname="tests.test_o" name="o2"/></testsuite>"#;
        let omitido = r#"<testsuite><testcase classname="tests.test_o" name="o2"><skipped/></testcase></testsuite>"#;
        let falla = r#"<testsuite><testcase classname="tests.test_o" name="o2"><failure message="x"/></testcase></testsuite>"#;
        let otro = r#"<testsuite><testcase classname="tests.test_x" name="x1"/></testsuite>"#;
        let junits =
            |xmls: &[&str]| -> Vec<String> { xmls.iter().map(|x| x.to_string()).collect() };
        assert!(
            !cumple_lo_pedido(&junits(&["<testsuite><testcase"]), &ids, 1),
            "junit roto"
        );
        assert!(
            !cumple_lo_pedido(&junits(&[otro]), &ids, 1),
            "no aparece el id"
        );
        assert!(
            !cumple_lo_pedido(
                &junits(&[uno]),
                &["test_o::o2".to_string(), "test_o::o3".to_string()],
                1
            ),
            "falta un id"
        );
        assert!(
            !cumple_lo_pedido(&junits(&[uno, uno]), &ids, 3),
            "2 de 3 repeticiones"
        );
        assert!(
            cumple_lo_pedido(&junits(&[uno, uno, uno]), &ids, 3),
            "3 junit de 1 testcase cada uno, con 3 repeticiones"
        );
        assert!(
            !cumple_lo_pedido(&junits(&[uno, "<testsuite>"]), &ids, 2),
            "un junit roto no cumple"
        );
        assert!(
            !cumple_lo_pedido(&junits(&[uno]), &[], 1),
            "sin ids no hay nada cumplido"
        );
        assert!(
            cumple_lo_pedido(&junits(&[omitido]), &ids, 1),
            "un skipped cuenta como resultado"
        );
        assert!(
            cumple_lo_pedido(&junits(&[falla]), &ids, 1),
            "una falla cuenta como resultado"
        );
    }

    #[test]
    fn colab_vigilar_dos_repeticiones_completas_libera() {
        let home = tempfile::tempdir().expect("home");
        let ruta = escribir_activo_de_prueba(
            home.path(),
            &json!({
                "v": 1,
                "sesion": "vieja",
                "pedido_ts": ahora_s(),
                "gpu_pedida": "T4",
                "ids": ["test_o::o2"],
                "repeticiones": 2,
            })
            .to_string(),
        );
        let falso = Falso::nuevo(|args: &[String]| match args[0].as_str() {
            "download"
                if args[3].ends_with("pendientes-1.xml")
                    || args[3].ends_with("pendientes-2.xml") =>
            {
                fs::write(args.last().expect("destino"), JUNIT).expect("escribe");
                salida(0, "", "")
            }
            "download" => salida(1, "", "todavía no hay junit"),
            _ => salida(0, "", ""),
        });
        let r = vigilar(&falso, home.path(), Duration::from_secs(7200));
        match r {
            Ok(Vigilancia::Liberado { causa }) => assert_eq!(causa, "requisito cumplido"),
            otro => panic!("esperaba Liberado por requisito, llegó {otro:?}"),
        }
        assert!(!ruta.exists());
    }

    #[test]
    fn colab_activo_viejo_sin_campos_nuevos_se_lee() {
        let activo = leer_activo(ACTIVO_VIEJO).expect("un activo viejo sigue leyéndose");
        assert_eq!(activo.sesion, "vieja");
        assert!(activo.ids.is_empty());
        assert_eq!(activo.repeticiones, None);
        assert_eq!(activo.tope_s, None);
    }

    #[test]
    fn colab_vigilar_vencido_libera() {
        let home = tempfile::tempdir().expect("home");
        let pedido = ahora_s() - 3 * 3600;
        let ruta =
            escribir_activo_de_prueba(home.path(), &activo_nuevo("vieja", pedido, &["test_o::o2"]));
        let falso = Falso::nuevo(feliz("Tesla T4"));
        let r = vigilar(&falso, home.path(), Duration::from_secs(7200));
        match r {
            Ok(Vigilancia::Liberado { causa }) => assert_eq!(causa, "tope de duración"),
            otro => panic!("esperaba Liberado por el tope, llegó {otro:?}"),
        }
        assert_eq!(falso.lineas(), vec!["stop -s vieja".to_string()]);
        assert!(!ruta.exists(), "activo.json debe borrarse");
        let gates = eventos_runtime(home.path());
        assert!(
            gates
                .iter()
                .any(|g| g["extra"]["accion"] == "vigilar"
                    && g["extra"]["causa"] == "tope de duración"),
            "{gates:?}"
        );
    }

    #[test]
    fn colab_vigilar_vigente_sin_junit_no_hace_nada() {
        let home = tempfile::tempdir().expect("home");
        let ruta = escribir_activo_de_prueba(
            home.path(),
            &activo_nuevo("vieja", ahora_s(), &["test_o::o2"]),
        );
        let falso = Falso::nuevo(|args: &[String]| match args[0].as_str() {
            "download" => salida(1, "", "todavía no hay junit"),
            _ => salida(0, "", ""),
        });
        let r = vigilar(&falso, home.path(), Duration::from_secs(7200));
        assert!(matches!(r, Ok(Vigilancia::Vigente)), "{r:?}");
        let lineas = falso.lineas();
        assert!(
            lineas.iter().any(|l| l.starts_with("download")),
            "{lineas:?}"
        );
        assert!(!lineas.iter().any(|l| l.starts_with("stop")), "{lineas:?}");
        assert!(ruta.exists(), "un runtime vigente conserva activo.json");
        assert!(eventos_runtime(home.path()).is_empty());
    }

    #[test]
    fn colab_vigilar_vigente_con_junit_completo_libera() {
        let home = tempfile::tempdir().expect("home");
        let ruta = escribir_activo_de_prueba(
            home.path(),
            &activo_nuevo("vieja", ahora_s(), &["test_o::o2"]),
        );
        let falso = Falso::nuevo(feliz("Tesla T4"));
        let r = vigilar(&falso, home.path(), Duration::from_secs(7200));
        match r {
            Ok(Vigilancia::Liberado { causa }) => assert_eq!(causa, "requisito cumplido"),
            otro => panic!("esperaba Liberado por requisito, llegó {otro:?}"),
        }
        assert!(falso.lineas().iter().any(|l| l == "stop -s vieja"));
        assert!(!ruta.exists());
        let gates = eventos_runtime(home.path());
        assert!(
            gates.iter().any(|g| g["extra"]["accion"] == "vigilar"
                && g["extra"]["causa"] == "requisito cumplido"),
            "{gates:?}"
        );
    }

    #[test]
    fn colab_vigilar_sin_activo_no_llama_a_la_cli() {
        let home = tempfile::tempdir().expect("home");
        let falso = Falso::nuevo(feliz("Tesla T4"));
        let r = vigilar(&falso, home.path(), Duration::from_secs(7200));
        assert!(matches!(r, Ok(Vigilancia::SinRuntime)), "{r:?}");
        assert!(falso.lineas().is_empty());
    }
}
