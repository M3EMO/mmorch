//! Tests pendientes en Colab con una T4 (decisión 22). Root habla con la CLI `colab` solo por
//! vectores de argumentos, nunca por cadenas de shell. El único dato del usuario que llega a
//! Colab es el nombre de sesión, y se valida antes.
//!
//! La preparación (`prepara.py`) instala Rust y pytest en el runtime de Colab, en la cuenta
//! de Google del usuario. No instala nada en la máquina donde corre Root.

use std::ffi::OsString;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::Value;

use super::tarea::Tarea;
use super::{ErrorOraculo, infra};
use crate::observe;

/// Variable que apunta al binario de la CLI. Los tests la usan con un falso.
const VARIABLE_BINARIO: &str = "ROOT_COLAB_BIN";
/// Tiempo máximo de cada llamada a la CLI, en segundos.
const TIMEOUT_LLAMADA: Duration = Duration::from_secs(3600);
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
/// Mensaje exacto cuando la CLI no tiene sesión. Root no hace el login.
pub const MENSAJE_SIN_SESION: &str = "Falta la sesión de Colab. Corré el login OAuth en tu terminal (por ejemplo `colab new -s prueba --gpu T4`, completá el navegador, y después `colab stop -s prueba`) y volvé a lanzar este comando.";
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

/// Lanza la CLI `colab` con argumentos. Un código distinto de cero es un resultado, no un error.
pub trait ColabCli {
    fn ejecutar(&self, args: &[String]) -> Result<SalidaColab, ErrorOraculo>;
}

/// CLI real: un proceso con entorno en lista blanca.
pub struct ColabProceso {
    binario: PathBuf,
    timeout: Duration,
}

impl ColabProceso {
    /// Binario de `ROOT_COLAB_BIN`, o `colab` del `PATH`, o `~/.local/bin/colab`.
    pub fn desde_entorno() -> Result<Self, ErrorOraculo> {
        let binario = match std::env::var_os(VARIABLE_BINARIO) {
            Some(valor) => PathBuf::from(valor),
            None => buscar_colab()?,
        };
        Ok(ColabProceso {
            binario,
            timeout: TIMEOUT_LLAMADA,
        })
    }
}

impl ColabCli for ColabProceso {
    fn ejecutar(&self, args: &[String]) -> Result<SalidaColab, ErrorOraculo> {
        let mut hijo = Command::new(&self.binario)
            .args(args)
            .env_clear()
            .envs(entorno_permitido(std::env::vars_os()))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
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
                    if inicio.elapsed() >= self.timeout {
                        let _ = hijo.kill();
                        let _ = hijo.wait();
                        return Err(ErrorOraculo::Infra(format!(
                            "colab superó {} s",
                            self.timeout.as_secs()
                        )));
                    }
                    thread::sleep(Duration::from_millis(50));
                }
                Err(e) => return Err(infra("esperar a colab", e)),
            }
        };
        Ok(SalidaColab {
            codigo: estado.code().unwrap_or(-1),
            stdout: h_salida.join().unwrap_or_default(),
            stderr: h_errores.join().unwrap_or_default(),
        })
    }
}

/// Resultado de una corrida en Colab: el junit y el dato de GPU para el informe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultadoColab {
    pub junit: PathBuf,
    pub gpu: String,
    pub sesion: String,
}

/// Nombre de sesión nuevo para la tarea: `root-<tarea>-<segundos>`, solo `[a-z0-9-]`.
pub fn nombre_sesion(tarea: &str) -> String {
    let segundos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
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
pub fn correr_en_colab(
    cli: &dyn ColabCli,
    paquete: &Path,
    tarea: &Tarea,
    home: &Path,
    nombre_sesion: &str,
) -> Result<ResultadoColab, ErrorOraculo> {
    validar_sesion(nombre_sesion)?;
    verificar_sin_claves(paquete)?;
    let tmp = home.join("tmp").join(format!("colab-{nombre_sesion}"));
    fs::create_dir_all(&tmp).map_err(|e| infra("carpeta temporal de Colab", e))?;
    let corrida = format!("colab-{}", tarea.nombre);

    // La CLI no tiene un chequeo de login sin efectos: `new` hace de chequeo.
    match cli.ejecutar(&args(&["new", "-s", nombre_sesion, "--gpu", GPU_PEDIDA])) {
        Ok(salida) if salida.codigo == 0 => {}
        Ok(salida) if parece_sin_sesion(&salida) => {
            return Err(ErrorOraculo::Colab(MENSAJE_SIN_SESION.into()));
        }
        otro => {
            // Un fallo que no es de login puede haber creado el runtime: se registra y se libera.
            let causa = match otro {
                Ok(salida) => format!(
                    "colab new salió con código {}: {}",
                    salida.codigo,
                    salida.stderr.trim()
                ),
                Err(e) => e.to_string(),
            };
            registrar_pedir(home, &corrida, nombre_sesion, "fallo")?;
            let _ = liberar(cli, home, &corrida, nombre_sesion, "desconocida", false);
            return Err(ErrorOraculo::Infra(format!(
                "no se pudo pedir el runtime: {causa}"
            )));
        }
    }
    registrar_pedir(home, &corrida, nombre_sesion, "ok")?;

    let mut gpu = String::from("desconocida");
    let trabajo = trabajar(cli, paquete, home, &tmp, nombre_sesion, &mut gpu);
    let parada = liberar(cli, home, &corrida, nombre_sesion, &gpu, trabajo.is_ok());
    match (trabajo, parada) {
        (Ok(junit), Ok(())) => Ok(ResultadoColab {
            junit,
            gpu,
            sesion: nombre_sesion.into(),
        }),
        (Err(e), Ok(())) => Err(e),
        (Ok(_), Err(p)) => Err(p),
        (Err(e), Err(p)) => Err(ErrorOraculo::Infra(format!("{e}; además, {p}"))),
    }
}

/// Borra el tar subido. Solo borra dentro de `<home>/tmp`, con la guarda de `borrado`.
fn borrar_tar(home: &Path, tar: &Path) -> io::Result<()> {
    crate::borrado::borrar_archivo_dentro(&home.join("tmp"), tar)
}

/// Lo que corre con el runtime ya pedido. Si falla, `correr_en_colab` libera el runtime.
fn trabajar(
    cli: &dyn ColabCli,
    paquete: &Path,
    home: &Path,
    tmp: &Path,
    sesion: &str,
    gpu: &mut String,
) -> Result<PathBuf, ErrorOraculo> {
    let guion_gpu = escribir(tmp, "gpu.py", GUION_GPU)?;
    let lectura = exec(cli, sesion, SEG_GPU, &guion_gpu)?;
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
    let subida = cli.ejecutar(&args(&[
        "upload",
        "-s",
        sesion,
        &ruta(&tar),
        PAQUETE_REMOTO,
    ]))?;
    let _ = borrar_tar(home, &tar);
    if subida.codigo != 0 {
        return Err(ErrorOraculo::Infra(format!(
            "no se pudo subir el paquete: {}",
            subida.stderr.trim()
        )));
    }

    let guion_preparar = escribir(tmp, "prepara.py", GUION_PREPARAR)?;
    let preparado = exec(cli, sesion, SEG_PREPARAR, &guion_preparar)?;
    if preparado.codigo != 0 || marcador(&preparado.stdout, "ROOT_PREPARADO=").is_none() {
        return Err(ErrorOraculo::Infra(format!(
            "la preparación del runtime falló: {}",
            preparado.stderr.trim()
        )));
    }

    let guion_correr = escribir(tmp, "corre.py", GUION_CORRER)?;
    let corrida = exec(cli, sesion, SEG_CORRER, &guion_correr)?;
    if corrida.codigo != 0 {
        return Err(ErrorOraculo::Infra(format!(
            "correr.sh no terminó en el runtime: {}",
            corrida.stderr.trim()
        )));
    }

    let junit = tmp.join(XML_PENDIENTES);
    let remoto = format!("{CARPETA_REMOTA}/{XML_PENDIENTES}");
    let bajada = cli.ejecutar(&args(&["download", "-s", sesion, &remoto, &ruta(&junit)]))?;
    if bajada.codigo != 0 || !junit.is_file() {
        return Err(ErrorOraculo::Infra(format!(
            "no se pudo bajar {XML_PENDIENTES}: {}",
            bajada.stderr.trim()
        )));
    }
    Ok(junit)
}

/// Para el runtime y registra el `Gate` de liberación. Devuelve error si la parada falla.
fn liberar(
    cli: &dyn ColabCli,
    home: &Path,
    corrida: &str,
    sesion: &str,
    gpu: &str,
    trabajo_ok: bool,
) -> Result<(), ErrorOraculo> {
    let parada = cli.ejecutar(&args(&["stop", "-s", sesion]));
    let parada_ok = matches!(&parada, Ok(s) if s.codigo == 0);
    let extra = serde_json::json!({
        "accion": "liberar",
        "sesion": sesion,
        "gpu_observada": gpu,
        "trabajo": if trabajo_ok { "ok" } else { "fallo" },
        "parada": if parada_ok { "ok" } else { "fallo" },
    });
    registrar(home, corrida, extra)?;
    if parada_ok {
        return Ok(());
    }
    Err(ErrorOraculo::Infra(format!(
        "no se pudo liberar el runtime {sesion}; revísalo en Colab"
    )))
}

fn registrar_pedir(
    home: &Path,
    corrida: &str,
    sesion: &str,
    resultado: &str,
) -> Result<(), ErrorOraculo> {
    let extra = serde_json::json!({
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

/// Corre un script Python en el runtime con `exec -f`. El script es un archivo local de Root.
fn exec(
    cli: &dyn ColabCli,
    sesion: &str,
    segundos: u64,
    guion: &Path,
) -> Result<SalidaColab, ErrorOraculo> {
    cli.ejecutar(&args(&[
        "exec",
        "-s",
        sesion,
        "--timeout",
        &segundos.to_string(),
        "-f",
        &ruta(guion),
    ]))
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

fn validar_sesion(nombre: &str) -> Result<(), ErrorOraculo> {
    let valido = !nombre.is_empty()
        && nombre.len() <= 60
        && nombre
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-');
    if valido {
        Ok(())
    } else {
        Err(ErrorOraculo::Tarea(format!(
            "nombre de sesión inválido: {nombre:?}"
        )))
    }
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
        fn ejecutar(&self, args: &[String]) -> Result<SalidaColab, ErrorOraculo> {
            self.llamadas.borrow_mut().push(args.to_vec());
            Ok((self.responde)(args))
        }
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
        let r = correr_en_colab(
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
        let r = correr_en_colab(&falso, &dir, &tarea(), home.path(), "root-memora-1");
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
        let r = correr_en_colab(
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
        let r = correr_en_colab(
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
        let r = correr_en_colab(
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
        let r = correr_en_colab(
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
        let r = correr_en_colab(
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
}
