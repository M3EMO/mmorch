//! Tests pendientes en Colab con una T4 (decisión 22). Root habla con la CLI `colab` solo por
//! vectores de argumentos, nunca por cadenas de shell. El único dato del usuario que llega a
//! Colab es el nombre de sesión, y se valida antes.
//!
//! La preparación (`prepara.py`) instala Rust y pytest en el runtime de Colab, en la cuenta
//! de Google del usuario. No instala nada en la máquina donde corre Root.
//!
//! Antes de pedir el runtime, Root anota la sesión en `$ROOT_HOME/colab/activo.json`. La borra
//! al liberar el runtime. `SIGINT`, `SIGTERM` y el tope de duración también liberan el runtime.
//! `SIGKILL` no se puede manejar: la próxima corrida, o `oraculo pendientes --liberar`, libera
//! el runtime anotado.

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
/// Tiempo de `exec` para correr `correr.sh`.
const SEG_CORRER: u64 = 2400;
/// La única GPU que Root pide.
const GPU_PEDIDA: &str = "T4";
/// Nombre del paquete dentro del runtime. Es el mismo en la subida y en la preparación.
const PAQUETE_REMOTO: &str = "root-paquete.tar.gz";
/// Carpeta donde `prepara.py` descomprime el paquete en el runtime.
const CARPETA_REMOTA: &str = "root-paquete";
/// Junit que deja `correr.sh`, dentro de la carpeta remota.
const XML_PENDIENTES: &str = "pendientes.xml";
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
/// parada cuenta como confirmada. A confirmar con la CLI real (no se ejecutó).
const MARCADORES_SESION_INEXISTENTE: &[&str] = &[
    "not found",
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

/// Script de preparación. Descomprime el paquete y deja Rust y pytest listos en el runtime.
/// Instala el toolchain en el runtime de Colab (cuenta del usuario), no en la máquina de Root.
const GUION_PREPARAR: &str = r#"import os, shutil, subprocess, sys
os.makedirs("root-paquete", exist_ok=True)
subprocess.run(["tar", "-xzf", "root-paquete.tar.gz", "-C", "root-paquete"], check=True)
if shutil.which("cargo") is None and not os.path.exists(os.path.expanduser("~/.cargo/bin/cargo")):
    subprocess.run("curl https://sh.rustup.rs -sSf | sh -s -- -y", shell=True, check=True)
subprocess.run([sys.executable, "-m", "pip", "install", "-q", "pytest"], check=True)
print("ROOT_PREPARADO=1")
"#;

/// Script de corrida. Corre `correr.sh` y deja `pendientes.xml`, aunque pytest falle.
const GUION_CORRER: &str = r#"import os, subprocess
env = dict(os.environ)
env["PATH"] = os.path.expanduser("~/.cargo/bin") + os.pathsep + env.get("PATH", "")
r = subprocess.run(["bash", "correr.sh"], cwd="root-paquete", env=env, timeout=2100)
print("ROOT_CORRIDA_CODIGO=%d" % r.returncode)
"#;

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
    pub junit: PathBuf,
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
pub fn correr_en_colab(
    cli: &dyn ColabCli,
    paquete: &Path,
    tarea: &Tarea,
    home: &Path,
    nombre_sesion: &str,
    cancelar: &Senales,
    tope: Duration,
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
    escribir_activo(home, nombre_sesion)?;

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
                false,
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
            );
            return Err(sumar_error(error, registro));
        }
    }
    let registro = registrar_pedir(home, &corrida, nombre_sesion, "ok");

    let mut gpu = String::from("desconocida");
    let trabajo = match registro {
        Ok(()) => trabajar(cli, &lim, paquete, home, &tmp, nombre_sesion, &mut gpu),
        Err(e) => Err(e),
    };
    match trabajo {
        Ok(junit) => {
            let parada = Parada {
                plazo: TIMEOUT_LLAMADA,
                cancelar: lim.cancelar,
            };
            match liberar(cli, home, &corrida, nombre_sesion, &gpu, true, parada) {
                Ok(()) => Ok(ResultadoColab {
                    junit,
                    gpu,
                    sesion: nombre_sesion.into(),
                }),
                // Una señal cortó la parada: se libera con su causa igual.
                Err(e) if lim.cancelar.recibida().is_some() => Err(cerrar_con_error(
                    cli,
                    &lim,
                    home,
                    &corrida,
                    nombre_sesion,
                    &gpu,
                    e,
                )),
                Err(e) => Err(e),
            }
        }
        Err(e) => Err(cerrar_con_error(
            cli,
            &lim,
            home,
            &corrida,
            nombre_sesion,
            &gpu,
            e,
        )),
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
pub fn describir_tope(tope: Duration) -> String {
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

/// Anota la sesión pedida en `activo.json`. Escribe un temporal en la misma carpeta y lo renombra.
fn escribir_activo(home: &Path, sesion: &str) -> Result<(), ErrorOraculo> {
    let carpeta = home.join(CARPETA_COLAB);
    fs::create_dir_all(&carpeta).map_err(|e| infra("carpeta colab", e))?;
    let texto = json!({
        "v": 1,
        "sesion": sesion,
        "pedido_ts": epoch_segundos(),
        "gpu_pedida": GPU_PEDIDA,
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

/// Lo que corre con el runtime ya pedido. Si falla, `correr_en_colab` libera el runtime.
fn trabajar(
    cli: &dyn ColabCli,
    lim: &Limites,
    paquete: &Path,
    home: &Path,
    tmp: &Path,
    sesion: &str,
    gpu: &mut String,
) -> Result<PathBuf, ErrorOraculo> {
    let guion_gpu = escribir(tmp, "gpu.py", GUION_GPU)?;
    let lectura = exec(cli, lim, sesion, SEG_GPU, &guion_gpu)?;
    let leida = if lectura.codigo == 0 {
        marcador(&lectura.stdout, "ROOT_GPU=")
    } else {
        None
    };
    *gpu = leida.clone().unwrap_or_else(|| "desconocida".into());
    if !leida.as_deref().is_some_and(es_t4) {
        return Err(ErrorOraculo::Colab(format!(
            "la GPU del runtime no es una T4 ({gpu}); no se corrió ningún test"
        )));
    }

    let tar = empaquetar(paquete, tmp)?;
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

    let guion_correr = escribir(tmp, "corre.py", GUION_CORRER)?;
    let corrida = exec(cli, lim, sesion, SEG_CORRER, &guion_correr)?;
    if corrida.codigo != 0 {
        return Err(ErrorOraculo::Infra(format!(
            "correr.sh no terminó en el runtime: {}",
            corrida.stderr.trim()
        )));
    }

    let junit = tmp.join(XML_PENDIENTES);
    let remoto = format!("{CARPETA_REMOTA}/{XML_PENDIENTES}");
    let bajada = paso(
        cli,
        lim,
        &args(&["download", "-s", sesion, &remoto, &ruta(&junit)]),
        TIMEOUT_LLAMADA,
    )?;
    if bajada.codigo != 0 || !junit.is_file() {
        return Err(ErrorOraculo::Infra(format!(
            "no se pudo bajar {XML_PENDIENTES}: {}",
            bajada.stderr.trim()
        )));
    }
    Ok(junit)
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
fn cerrar_con_error(
    cli: &dyn ColabCli,
    lim: &Limites,
    home: &Path,
    corrida: &str,
    sesion: &str,
    gpu: &str,
    error: ErrorOraculo,
) -> ErrorOraculo {
    if let Some(causa) = cerrar_por_causa(cli, lim, home, corrida, sesion, gpu) {
        return causa;
    }
    let parada = Parada {
        plazo: TIMEOUT_LLAMADA,
        cancelar: lim.cancelar,
    };
    match liberar(cli, home, corrida, sesion, gpu, false, parada) {
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
        return Some(cerrar_con_causa(
            cli,
            home,
            corrida,
            sesion,
            gpu,
            &format!("señal {senal}"),
            format!("Interrumpido por {senal}: se liberó el runtime de Colab"),
        ));
    }
    if lim.vencido() {
        let texto = describir_tope(lim.tope);
        return Some(cerrar_con_causa(
            cli,
            home,
            corrida,
            sesion,
            gpu,
            &format!("tope de {texto}"),
            format!("Colab superó el tope de {texto}: se liberó el runtime"),
        ));
    }
    None
}

/// Libera tras una señal o un tope. La parada no mira señales: ya llegó una, y debe terminar.
/// Su error no cambia el mensaje de la causa. Si no se confirma, conserva `activo.json` y lo dice.
fn cerrar_con_causa(
    cli: &dyn ColabCli,
    home: &Path,
    corrida: &str,
    sesion: &str,
    gpu: &str,
    motivo: &str,
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
        "motivo": motivo,
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

/// Para el runtime y registra el `Gate` de liberación. Con la parada confirmada, borra `activo.json`.
/// Devuelve error si la parada no se confirma: el archivo se conserva.
fn liberar(
    cli: &dyn ColabCli,
    home: &Path,
    corrida: &str,
    sesion: &str,
    gpu: &str,
    trabajo_ok: bool,
    parada: Parada,
) -> Result<(), ErrorOraculo> {
    let confirmacion = confirmar_parada(cli, sesion, parada);
    let mut extra = json!({
        "accion": "liberar",
        "sesion": sesion,
        "gpu_observada": gpu,
        "trabajo": if trabajo_ok { "ok" } else { "fallo" },
        "parada": if confirmacion.confirmada() { "ok" } else { "fallo" },
        "confirmada": confirmacion.confirmada(),
    });
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
    use std::cell::RefCell;
    use std::os::unix::fs::PermissionsExt;
    use std::rc::Rc;

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
                    Some(f) if f.ends_with("corre.py") => salida(0, "ROOT_CORRIDA_CODIGO=0", ""),
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
        assert!(r.junit.is_file());
        let gates = eventos_runtime(home.path());
        assert_eq!(gates.len(), 2);
        assert_eq!(gates[0]["extra"]["gpu_pedida"], "T4");
        assert_eq!(gates[1]["extra"]["gpu_observada"], "Tesla T4");
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

    /// Script de una CLI que deja correr `corre.py` durante `segundos`. Anota su PID en `pid.txt`.
    /// `stop_cuerpo` es el cuerpo del caso `stop`: el texto y el código con que responde.
    fn falso_corre_lenta_stop(dir: &Path, segundos: u32, stop_cuerpo: &str) -> PathBuf {
        let guion = format!(
            "#!/bin/sh\necho \"$*\" >> '{d}/llamadas.log'\ncase \"$1\" in\nexec)\n  case \"$*\" in\n    *gpu.py*) echo 'ROOT_GPU=Tesla T4' ;;\n    *prepara.py*) echo ROOT_PREPARADO=1 ;;\n    *corre.py*) echo $$ > '{d}/pid.txt'; sleep {segundos} ;;\n  esac ;;\nstop) {stop_cuerpo} ;;\n*) exit 0 ;;\nesac\nexit 0\n",
            d = dir.display()
        );
        let ruta = dir.join("colab-lenta");
        fs::write(&ruta, guion).expect("escribe el falso");
        fs::set_permissions(&ruta, fs::Permissions::from_mode(0o755)).expect("permisos");
        ruta
    }

    /// Como `falso_corre_lenta_stop`, con `stop` que sale 0.
    fn falso_corre_lenta(dir: &Path, segundos: u32) -> PathBuf {
        falso_corre_lenta_stop(dir, segundos, "exit 0")
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
        let falso = Falso::nuevo(|_: &[String]| salida(1, "", "session not found"));
        let r = liberar_anotado(&falso, home.path());
        assert_eq!(r.expect("sin error"), Some("vieja".to_string()));
        assert!(!ruta.exists());
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
        let binario = falso_corre_lenta(base.path(), 20);
        let cli = ColabProceso {
            binario,
            plazo_primer_paso: Duration::from_secs(1),
        };
        let senales = Senales::default();
        let senal_en_hilo = senales.clone();
        let log = base.path().join("llamadas.log");
        thread::spawn(move || {
            // Espera a que la corrida esté dentro de `corre.py` y manda SIGINT simulado.
            loop {
                let texto = fs::read_to_string(&log).unwrap_or_default();
                if texto.contains("corre.py") {
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
                    .is_some_and(|m| m.contains("SIGINT"))),
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
        let binario = falso_corre_lenta(base.path(), 20);
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
                && g["extra"]["motivo"] == "tope de 2 segundos"),
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
    fn colab_describir_tope_minutos_o_segundos() {
        assert_eq!(describir_tope(Duration::from_secs(120)), "2 minutos");
        assert_eq!(describir_tope(Duration::from_secs(1800)), "30 minutos");
        assert_eq!(describir_tope(Duration::from_secs(2)), "2 segundos");
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
            falso_corre_lenta_stop(base.path(), 20, "echo 'session not found' >&2; exit 1");
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
                    .contains("corre.py")
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
}
