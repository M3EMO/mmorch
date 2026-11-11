//! Corre un comando en un contenedor de podman sin red y con un único directorio escribible.

use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde::Serialize;

use super::ErrorOraculo;

/// Caracteres de la cola de stdout y stderr que se guardan.
const COLA: usize = 4000;
/// Bytes que `leer_cola_en_hilo` conserva al final. Un carácter UTF-8 ocupa hasta 4 bytes; el margen de
/// 4 cubre el carácter cortado al inicio de la ventana. Así quedan siempre `COLA` caracteres completos.
const COLA_BYTES: usize = COLA * 4 + 4;
/// Bytes de la cola de stdout y stderr de `podman` que se conservan (build, inspect, rm). Es 1 MiB.
/// El build necesita la salida entera para clasificar el fallo. El recorte a `COLA` va después.
const TOPE_SALIDA_PODMAN: usize = 1024 * 1024;
/// Bytes de una línea de stdout que el lector guarda para reconocer un `STEP`. Lo que pasa de aquí se descarta.
const TOPE_LINEA_STEP: usize = 64 * 1024;
/// Bytes de stdout del builder que se guardan completos. El builder necesita su salida entera para
/// leer costos y sesión. Pasado el tope, el proceso corre igual, pero la corrida falla.
pub(crate) const TOPE_SALIDA_BUILDER: usize = 64 * 1024 * 1024;
/// Bytes de la cola de la salida de `claude` que se guardan en el oráculo colab. Es 16 MiB.
/// La cola basta para el diagnóstico. Lo que pasa del tope se descarta.
pub(crate) const TOPE_SALIDA_COLAB: usize = 16 * 1024 * 1024;
/// Tope de procesos e hilos del contenedor. Acota una fork bomb dentro de la imagen.
pub(crate) const PIDS_LIMITE: u32 = 512;
/// Memoria máxima del contenedor. Sube si una suite legítima llega al tope (ver el informe de medición).
pub(crate) const MEMORIA: &str = "2g";
/// Plazo de `podman rm -f` al limpiar un contenedor. Si podman no responde, la limpieza no bloquea a Root.
const PLAZO_LIMPIEZA: Duration = Duration::from_secs(30);
/// Códigos de salida que indican fallo del entorno, no del comando evaluado. 125: error propio de podman
/// (imagen inexistente, opción inválida). 126: comando no ejecutable. 127: comando no encontrado.
/// 137: proceso muerto por SIGKILL, por ejemplo por falta de memoria (OOM).
const CODIGOS_DE_INFRA: [i32; 4] = [125, 126, 127, 137];
/// Pausa entre consultas al proceso mientras se espera su fin.
const SONDEO: Duration = Duration::from_millis(20);

static CONTADOR: AtomicU64 = AtomicU64::new(0);

/// Tag de SELinux obligatorio en los montajes (Fedora).
pub const SUFIJO_SELINUX: &str = "z";
/// Opción de `podman run` y `podman build` que no pasa al contenedor las variables de proxy del host (M4).
pub(crate) const PROXY_FUERA: &str = "--http-proxy=false";

/// Qué correr, con qué montajes y con qué límite de tiempo. Su `Debug` oculta el valor de los secretos.
#[derive(Clone)]
pub struct Ejecucion {
    pub imagen: String,
    pub repo: Option<PathBuf>,
    pub oraculo: Option<PathBuf>,
    /// Destino de `oraculo` dentro del contenedor. Sin valor, `/oraculo`.
    pub oraculo_destino: Option<String>,
    pub salida: PathBuf,
    pub cmd: String,
    pub env: Vec<(String, String)>,
    pub timeout: Duration,
    /// Sin `red` el contenedor corre con `--network none`. Con `red` sale a internet.
    pub red: bool,
    /// Carpeta montada en `/cache`. Solo lectura, salvo que `cache_rw` sea verdadero.
    pub cache: Option<PathBuf>,
    pub cache_rw: bool,
    /// Base de RustSec montada en solo lectura en `/rustsec`.
    pub rustsec: Option<PathBuf>,
    /// Carpeta con copias de los `requirements*.txt`, montada en solo lectura en `/trabajo`.
    pub trabajo: Option<PathBuf>,
    /// Secretos (nombre, valor) que el contenedor hereda del entorno de Root con `-e NOMBRE`.
    /// El valor nunca va en los argumentos: `lanzar` lo pone en el entorno de `podman`.
    pub secretos: Vec<(String, String)>,
}

impl std::fmt::Debug for Ejecucion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let secretos: Vec<String> = self
            .secretos
            .iter()
            .map(|(nombre, _)| format!("{nombre}=***"))
            .collect();
        f.debug_struct("Ejecucion")
            .field("imagen", &self.imagen)
            .field("repo", &self.repo)
            .field("oraculo", &self.oraculo)
            .field("oraculo_destino", &self.oraculo_destino)
            .field("salida", &self.salida)
            .field("cmd", &self.cmd)
            .field("env", &self.env)
            .field("timeout", &self.timeout)
            .field("red", &self.red)
            .field("cache", &self.cache)
            .field("cache_rw", &self.cache_rw)
            .field("rustsec", &self.rustsec)
            .field("trabajo", &self.trabajo)
            .field("secretos", &secretos)
            .finish()
    }
}

/// Lo que dejó el comando dentro del contenedor. Solo la cola de la salida.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Resultado {
    pub codigo: i32,
    pub stdout: String,
    pub stderr: String,
    pub segundos: f64,
}

/// Argumentos de `podman run`, sin el binario `podman`.
pub fn argumentos(e: &Ejecucion, nombre: &str) -> Vec<String> {
    let mut args: Vec<String> = ["run", "--rm", "--name", nombre].map(String::from).into();
    args.extend(opciones(e));
    args.push(e.imagen.clone());
    args.extend(["sh", "-c"].map(String::from));
    args.push(e.cmd.clone());
    args
}

/// Argumentos de `podman run -d` para un contenedor que Root mantiene vivo. Corre `comando` como
/// entrypoint. No lleva `--rm`: Root quita el contenedor con `podman rm -f`.
/// Lleva `--userns=keep-id`: el proceso corre como el usuario del host, sin privilegios dentro. Así el
/// producto no reemplaza `/bin/sh` ni `/usr/bin/timeout` de la imagen. `argumentos` (run --rm) no lo lleva.
pub(crate) fn argumentos_separado(e: &Ejecucion, nombre: &str, comando: &[&str]) -> Vec<String> {
    let Some((entrada, resto)) = comando.split_first() else {
        return Vec::new();
    };
    let mut args: Vec<String> = ["run", "-d", "--name", nombre].map(String::from).into();
    args.push("--userns=keep-id".into());
    args.extend(opciones(e));
    args.extend(["--entrypoint", entrada].map(String::from));
    args.push(e.imagen.clone());
    args.extend(resto.iter().map(|a| a.to_string()));
    args
}

/// Opciones de `podman run` de la ejecución: endurecimiento, límites, montajes y variables. Van
/// antes de la imagen. No incluye la cabecera (`run`, nombre) ni la cola (imagen y comando).
fn opciones(e: &Ejecucion) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    // Sin capacidades de Linux ni ganancia de privilegios por setuid.
    args.extend(["--cap-drop=all", "--security-opt=no-new-privileges"].map(String::from));
    args.push(format!("--pids-limit={PIDS_LIMITE}"));
    args.push(format!("--memory={MEMORIA}"));
    // La imagen debe estar local. `run` no baja nada en silencio.
    args.push("--pull=never".into());
    args.push(PROXY_FUERA.into());
    if !e.red {
        args.extend(["--network", "none"].map(String::from));
    }
    args.extend(["--tmpfs", "/tmp:rw"].map(String::from));
    if let Some(repo) = &e.repo {
        montaje(&mut args, repo, "/repo", &format!("ro,{SUFIJO_SELINUX}"));
    }
    if let Some(trabajo) = &e.trabajo {
        montaje(
            &mut args,
            trabajo,
            "/trabajo",
            &format!("ro,{SUFIJO_SELINUX}"),
        );
    }
    if let Some(oraculo) = &e.oraculo {
        let destino = e.oraculo_destino.as_deref().unwrap_or("/oraculo");
        montaje(&mut args, oraculo, destino, &format!("ro,{SUFIJO_SELINUX}"));
    }
    if let Some(cache) = &e.cache {
        let opciones = if e.cache_rw {
            SUFIJO_SELINUX.to_string()
        } else {
            format!("ro,{SUFIJO_SELINUX}")
        };
        montaje(&mut args, cache, "/cache", &opciones);
    }
    if let Some(rustsec) = &e.rustsec {
        montaje(
            &mut args,
            rustsec,
            "/rustsec",
            &format!("ro,{SUFIJO_SELINUX}"),
        );
    }
    montaje(&mut args, &e.salida, "/salida", SUFIJO_SELINUX);
    for (clave, valor) in &e.env {
        args.push("-e".into());
        args.push(format!("{clave}={valor}"));
    }
    for (nombre, _) in &e.secretos {
        args.push("-e".into());
        args.push(nombre.clone());
    }
    args
}

fn montaje(args: &mut Vec<String>, origen: &Path, destino: &str, opciones: &str) {
    args.push("-v".into());
    args.push(format!("{}:{destino}:{opciones}", origen.display()));
}

/// Corre el comando en un contenedor nuevo y devuelve su resultado.
pub fn ejecutar(e: &Ejecucion) -> Result<Resultado, ErrorOraculo> {
    validar_rutas(e)?;
    let _turno = turno_de_contenedor();
    let nombre = nombre_unico();
    let inicio = Instant::now();
    let mut hijo = lanzar(e, &nombre)?;
    let salida = hijo.stdout.take().map(leer_cola_en_hilo);
    let error = hijo.stderr.take().map(leer_cola_en_hilo);
    let estado = match esperar(&mut hijo, e.timeout) {
        Ok(Some(estado)) => estado,
        Ok(None) => {
            terminar(&mut hijo, &nombre);
            return Err(ErrorOraculo::Infra(format!(
                "timeout tras {} s; contenedor {nombre} eliminado",
                e.timeout.as_secs_f64()
            )));
        }
        // Sin esto, un fallo al consultar el proceso dejaría el contenedor corriendo.
        Err(err) => {
            terminar(&mut hijo, &nombre);
            return Err(err);
        }
    };
    let stdout = recoger(salida);
    let stderr = recoger(error);
    let segundos = inicio.elapsed().as_secs_f64();
    let Some(codigo) = estado.code() else {
        return Err(ErrorOraculo::Infra("podman terminó por una señal".into()));
    };
    if let Some(error) = error_de_infra(codigo, &stderr) {
        return Err(error);
    }
    Ok(Resultado {
        codigo,
        stdout: cola(&stdout),
        stderr: cola(&stderr),
        segundos,
    })
}

/// Salida de un comando de podman que no es un contenedor de evaluación (pull, build, inspect).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SalidaPodman {
    pub(crate) codigo: i32,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    /// Última línea `STEP k/n:` del stdout, aunque la cola de 1 MiB ya la haya perdido.
    pub(crate) paso: Option<String>,
}

/// Turno de los contenedores (Tarea 48, parte 2): un solo `podman` corre trabajo a la vez. Lo toma cada
/// ejecución de podman. Una llamada a juez no lo toma: un hilo que espera a la red no lo retiene.
static TURNO: Mutex<()> = Mutex::new(());

/// Toma el turno de los contenedores. Mientras el guard vive, ningún otro hilo corre podman. El `Mutex` no es
/// reentrante: quien ya lo tiene no lo puede tomar otra vez, así que solo lo toman las funciones hoja.
pub(crate) fn turno_de_contenedor() -> MutexGuard<'static, ()> {
    TURNO.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Corre `podman` con `args` y espera hasta `plazo`. Si vence, mata el proceso y da `Infra`.
pub(crate) fn correr_podman(args: &[&str], plazo: Duration) -> Result<SalidaPodman, ErrorOraculo> {
    correr_programa("podman", args, plazo)
}

/// Igual que `correr_podman`, con el programa como parámetro. Los tests lo usan con `sh`.
pub(crate) fn correr_programa(
    programa: &str,
    args: &[&str],
    plazo: Duration,
) -> Result<SalidaPodman, ErrorOraculo> {
    let _turno = turno_de_contenedor();
    let mut comando = Command::new(programa);
    comando.args(args);
    correr_comando(comando, args.first().unwrap_or(&""), plazo)
}

/// Corre `comando` con salida capturada y espera hasta `plazo`. Si vence o falla la espera, mata el proceso.
/// No toma el turno: `terminar` la llama dentro de `ejecutar`, que ya lo tiene, y el `Mutex` no es reentrante.
fn correr_comando(
    mut comando: Command,
    etiqueta: &str,
    plazo: Duration,
) -> Result<SalidaPodman, ErrorOraculo> {
    let mut hijo = comando
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| ErrorOraculo::Infra(format!("no se pudo lanzar podman: {err}")))?;
    let salida = hijo
        .stdout
        .take()
        .map(|f| leer_salida_en_hilo(f, TOPE_SALIDA_PODMAN));
    let error = hijo
        .stderr
        .take()
        .map(|f| leer_cola_en_hilo_con(f, TOPE_SALIDA_PODMAN));
    let estado = match esperar(&mut hijo, plazo) {
        Ok(Some(estado)) => estado,
        Ok(None) => {
            matar(&mut hijo);
            return Err(ErrorOraculo::Infra(format!(
                "podman {etiqueta} superó {} s",
                plazo.as_secs()
            )));
        }
        Err(err) => {
            matar(&mut hijo);
            return Err(err);
        }
    };
    let Some(codigo) = estado.code() else {
        return Err(ErrorOraculo::Infra("podman terminó por una señal".into()));
    };
    let (stdout, paso) = recoger_salida(salida);
    Ok(SalidaPodman {
        codigo,
        stdout,
        stderr: recoger(error),
        paso,
    })
}

/// Mata el proceso y espera su fin. Los errores se ignoran: el llamador ya reporta el fallo.
fn matar(hijo: &mut Child) {
    let _ = hijo.kill();
    let _ = hijo.wait();
}

/// Plazo para recoger la salida de un hijo muerto. Un nieto con el pipe abierto no hace esperar más.
const PLAZO_RECOGIDA: Duration = Duration::from_secs(2);

/// Proceso de `podman` que sigue vivo mientras Root lo necesita, por ejemplo un `podman exec`.
/// Sus lectores corren en hilos. `terminar` lo mata y recoge lo que escribió.
pub(crate) struct HijoPodman {
    hijo: Child,
    salida: Option<JoinHandle<Vec<u8>>>,
    error: Option<JoinHandle<Vec<u8>>>,
}

/// Lanza `podman` con `args` y deja el proceso vivo. Un fallo al lanzar es `Infra`.
pub(crate) fn lanzar_hijo(args: &[&str]) -> Result<HijoPodman, ErrorOraculo> {
    lanzar_programa("podman", args)
}

/// Igual que `lanzar_hijo`, con el programa como parámetro. Los tests lo usan con `sh`.
fn lanzar_programa(programa: &str, args: &[&str]) -> Result<HijoPodman, ErrorOraculo> {
    // Un solo podman a la vez: el turno cubre el arranque. El proceso vivo no lo retiene.
    let _turno = turno_de_contenedor();
    let mut hijo = Command::new(programa)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| ErrorOraculo::Infra(format!("no se pudo lanzar podman: {err}")))?;
    let salida = hijo.stdout.take().map(leer_cola_en_hilo);
    let error = hijo.stderr.take().map(leer_cola_en_hilo);
    Ok(HijoPodman {
        hijo,
        salida,
        error,
    })
}

impl HijoPodman {
    /// Verdadero si el proceso ya salió. No espera.
    pub(crate) fn terminado(&mut self) -> Result<bool, ErrorOraculo> {
        self.hijo
            .try_wait()
            .map(|estado| estado.is_some())
            .map_err(|e| ErrorOraculo::Infra(format!("no se pudo consultar podman: {e}")))
    }

    /// Mata el proceso y da su stdout seguido de su stderr, con la cola de `COLA` caracteres.
    /// Mata antes de recoger: así el proceso no sigue escribiendo al pipe.
    pub(crate) fn terminar(&mut self) -> String {
        matar(&mut self.hijo);
        let limite = Instant::now() + PLAZO_RECOGIDA;
        let salida = recoger_antes(self.salida.take(), limite);
        let error = recoger_antes(self.error.take(), limite);
        cola(&format!("{salida}{error}"))
    }
}

/// Recoge la salida de un lector si termina antes de `limite`. Si no, da texto vacío.
/// El hilo sigue solo y termina cuando se cierra su pipe.
fn recoger_antes(hilo: Option<JoinHandle<Vec<u8>>>, limite: Instant) -> String {
    let Some(hilo) = hilo else {
        return String::new();
    };
    while !hilo.is_finished() {
        if Instant::now() >= limite {
            return String::new();
        }
        thread::sleep(SONDEO);
    }
    recoger(Some(hilo))
}

/// Falla antes de lanzar podman si falta alguna ruta montada.
pub(crate) fn validar_rutas(e: &Ejecucion) -> Result<(), ErrorOraculo> {
    let rutas = e
        .repo
        .iter()
        .chain(e.oraculo.iter())
        .chain(e.cache.iter())
        .chain(e.rustsec.iter())
        .chain(e.trabajo.iter())
        .chain(std::iter::once(&e.salida));
    for ruta in rutas {
        if !ruta.exists() {
            return Err(ErrorOraculo::Infra(format!("no existe {}", ruta.display())));
        }
    }
    Ok(())
}

fn nombre_unico() -> String {
    let n = CONTADOR.fetch_add(1, Ordering::Relaxed);
    format!("root-oraculo-{}-{n}", std::process::id())
}

fn lanzar(e: &Ejecucion, nombre: &str) -> Result<Child, ErrorOraculo> {
    Command::new("podman")
        .args(argumentos(e, nombre))
        .envs(e.secretos.iter().map(|(n, v)| (n.as_str(), v.as_str())))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| ErrorOraculo::Infra(format!("no se pudo lanzar podman: {err}")))
}

/// Resultado de leer un flujo: los bytes guardados y si la salida pasó el tope.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Lectura {
    pub bytes: Vec<u8>,
    pub recortada: bool,
}

/// Qué parte de la salida se guarda cuando pasa el tope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Modo {
    /// Guarda el inicio. Lo que pasa del tope se lee y se descarta.
    Inicio,
    /// Guarda la cola. Sirve para el diagnóstico de lo último que se imprimió.
    Cola,
}

/// Lee un flujo hasta EOF con un tope. Reintenta si la lectura se interrumpe. Otro error corta la lectura.
pub(crate) fn leer_con_tope<R: Read>(flujo: R, tope: usize, modo: Modo) -> Lectura {
    match modo {
        Modo::Inicio => leer_inicio(flujo, tope),
        Modo::Cola => leer_cola(flujo, tope, |_| {}),
    }
}

/// Guarda el inicio hasta `tope` bytes. Lo que pasa del tope se lee y se descarta.
fn leer_inicio<R: Read>(mut flujo: R, tope: usize) -> Lectura {
    let mut bytes = Vec::new();
    let mut total: usize = 0;
    let mut trozo = [0u8; 8192];
    loop {
        match flujo.read(&mut trozo) {
            Ok(0) => break,
            Ok(n) => {
                total += n;
                // Sigue leyendo al pasar el tope. Así el proceso no se bloquea al escribir.
                let libre = tope - bytes.len();
                bytes.extend_from_slice(&trozo[..n.min(libre)]);
            }
            Err(err) if err.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
    Lectura {
        bytes,
        recortada: total > tope,
    }
}

/// Lee un flujo hasta EOF y guarda su cola de `tope` bytes. `al_leer` ve cada trozo antes de recortar.
/// Si el total pasó el tope, la cola empieza en un borde de carácter UTF-8. Así no sale un carácter partido.
fn leer_cola<R: Read>(mut flujo: R, tope: usize, mut al_leer: impl FnMut(&[u8])) -> Lectura {
    let mut cola = Vec::new();
    let mut total: usize = 0;
    let mut trozo = [0u8; 8192];
    loop {
        match flujo.read(&mut trozo) {
            Ok(0) => break,
            Ok(n) => {
                total += n;
                al_leer(&trozo[..n]);
                cola.extend_from_slice(&trozo[..n]);
                // Se recorta al pasar el doble: el recorte cuesta lo mismo que la entrada leída.
                if cola.len() > 2 * tope {
                    cola.drain(..cola.len() - tope);
                }
            }
            Err(err) if err.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
    let recortada = total > tope;
    if cola.len() > tope {
        cola.drain(..cola.len() - tope);
    }
    // Solo si hubo recorte: una salida completa se guarda entera, aunque no sea UTF-8.
    if recortada {
        cola.drain(..saltos_hasta_caracter(&cola));
    }
    Lectura {
        bytes: cola,
        recortada,
    }
}

/// Cuántos bytes saltar al inicio para caer en un borde de carácter UTF-8.
/// Un carácter UTF-8 tiene como máximo 3 bytes de continuación. El tope de 3 evita saltar de más.
fn saltos_hasta_caracter(cola: &[u8]) -> usize {
    cola.iter()
        .take(3)
        .take_while(|b| (**b & 0xC0) == 0x80)
        .count()
}

/// Lee un flujo completo en otro hilo, con el tope del builder. Así el proceso no se bloquea al llenar el pipe.
/// Solo lo usan los tests. `correr_proceso` pasa su tope con `leer_en_hilo_con`.
#[cfg(test)]
pub(crate) fn leer_en_hilo<R: Read + Send + 'static>(flujo: R) -> JoinHandle<Lectura> {
    leer_en_hilo_con(flujo, TOPE_SALIDA_BUILDER)
}

/// Lee un flujo en otro hilo y guarda el inicio hasta `tope` bytes.
pub(crate) fn leer_en_hilo_con<R: Read + Send + 'static>(
    flujo: R,
    tope: usize,
) -> JoinHandle<Lectura> {
    thread::spawn(move || leer_con_tope(flujo, tope, Modo::Inicio))
}

/// Texto de un tamaño en bytes. Da `N MiB` si es múltiplo exacto de MiB. Si no, da `N bytes`.
pub(crate) fn tamano_legible(bytes: usize) -> String {
    const MIB: usize = 1024 * 1024;
    if bytes >= MIB && bytes.is_multiple_of(MIB) {
        format!("{} MiB", bytes / MIB)
    } else {
        format!("{bytes} bytes")
    }
}

/// Lee un flujo en otro hilo y conserva solo los últimos `COLA_BYTES` bytes. La memoria no crece con la salida.
pub(crate) fn leer_cola_en_hilo<R: Read + Send + 'static>(flujo: R) -> JoinHandle<Vec<u8>> {
    leer_cola_en_hilo_con(flujo, COLA_BYTES)
}

/// Lee un flujo en otro hilo y conserva solo los últimos `tope` bytes. El búfer nunca pasa de `2 * tope`.
pub(crate) fn leer_cola_en_hilo_con<R: Read + Send + 'static>(
    flujo: R,
    tope: usize,
) -> JoinHandle<Vec<u8>> {
    thread::spawn(move || leer_cola(flujo, tope, |_| {}).bytes)
}

/// Lee el stdout de `podman` en otro hilo. Da la cola y la última línea `STEP` vista, si hubo una.
fn leer_salida_en_hilo(
    flujo: impl Read + Send + 'static,
    tope: usize,
) -> JoinHandle<(Vec<u8>, Option<String>)> {
    thread::spawn(move || leer_salida_con_step(flujo, tope))
}

/// Lee un flujo hasta EOF. Da la cola de `tope` bytes y la última línea completa `STEP` que vio.
pub(crate) fn leer_salida_con_step<R: Read>(flujo: R, tope: usize) -> (Vec<u8>, Option<String>) {
    let mut rastreo = RastreoStep::default();
    let cola = leer_cola(flujo, tope, |trozo| rastreo.alimentar(trozo)).bytes;
    (cola, rastreo.cerrar())
}

/// Arma las líneas de un flujo entre trozos. Guarda solo la última línea `STEP` completa.
#[derive(Default)]
struct RastreoStep {
    /// Bytes de la línea en curso. No pasa de `TOPE_LINEA_STEP`.
    linea: Vec<u8>,
    ultimo: Option<String>,
}

impl RastreoStep {
    fn alimentar(&mut self, trozo: &[u8]) {
        for (k, parte) in trozo.split(|&b| b == b'\n').enumerate() {
            if k > 0 {
                self.cerrar_linea();
            }
            let libre = TOPE_LINEA_STEP - self.linea.len();
            self.linea
                .extend_from_slice(&parte[..parte.len().min(libre)]);
        }
    }

    fn cerrar_linea(&mut self) {
        let texto = String::from_utf8_lossy(&self.linea);
        if es_linea_step(&texto) {
            self.ultimo = Some(texto.trim_end_matches('\r').to_string());
        }
        self.linea.clear();
    }

    fn cerrar(mut self) -> Option<String> {
        self.cerrar_linea();
        self.ultimo
    }
}

pub(crate) fn recoger(hilo: Option<JoinHandle<Vec<u8>>>) -> String {
    let bytes = hilo.and_then(|h| h.join().ok()).unwrap_or_default();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Da la cola del stdout de `podman` y la última línea `STEP` que vio el lector, si hubo una.
///
/// La cola va cruda. Un build con más de 1 MiB después de su `STEP` deja la cola sin ese `STEP`.
/// Esa línea viaja aparte, en `paso`. Si la cola no trae ningún `STEP`, `clasificar_fallo` mira `paso`
/// para decidir entre sin veredicto e infraestructura.
fn recoger_salida(hilo: Option<JoinHandle<(Vec<u8>, Option<String>)>>) -> (String, Option<String>) {
    let (bytes, paso) = hilo.and_then(|h| h.join().ok()).unwrap_or_default();
    (String::from_utf8_lossy(&bytes).into_owned(), paso)
}

/// Parte de una línea `STEP k/n: texto`. Da el texto sin normalizar, o `None` si la línea no es un paso.
/// Acepta espacios iniciales y el prefijo `[k/m] ` de varias etapas.
fn texto_del_step(linea: &str) -> Option<&str> {
    let limpia = linea.trim_start();
    let limpia = match limpia.strip_prefix('[') {
        Some(resto) => resto.split_once("] ")?.1,
        None => limpia,
    };
    let resto = limpia.strip_prefix("STEP ")?;
    let (numeros, texto) = resto.split_once(": ")?;
    let (paso, total) = numeros.split_once('/')?;
    paso.trim().parse::<usize>().ok()?;
    total.trim().parse::<usize>().ok()?;
    Some(texto)
}

/// Verdadero si la línea es un paso `STEP k/n: texto` de `podman build`.
fn es_linea_step(linea: &str) -> bool {
    texto_del_step(linea).is_some()
}

/// Texto del paso de una línea `STEP k/n: texto`, con los espacios colapsados a uno. Da `None` si no es un paso.
pub(crate) fn texto_de_linea_step(linea: &str) -> Option<String> {
    texto_del_step(linea).map(|texto| texto.split_whitespace().collect::<Vec<_>>().join(" "))
}

/// Espera al proceso hasta `limite`. Da `None` si venció el plazo.
pub(crate) fn esperar(
    hijo: &mut Child,
    limite: Duration,
) -> Result<Option<ExitStatus>, ErrorOraculo> {
    let plazo = Instant::now() + limite;
    loop {
        if let Some(estado) = hijo
            .try_wait()
            .map_err(|e| ErrorOraculo::Infra(format!("no se pudo consultar podman: {e}")))?
        {
            return Ok(Some(estado));
        }
        if Instant::now() >= plazo {
            return Ok(None);
        }
        thread::sleep(SONDEO);
    }
}

/// Borra el contenedor por nombre sin esperar la parada, y mata el proceso de podman.
/// Los errores se ignoran: ya hay un fallo que reportar.
fn terminar(hijo: &mut Child, nombre: &str) {
    let mut rm = Command::new("podman");
    rm.args(["rm", "-f", "-t", "0", nombre]);
    terminar_con(rm, hijo, PLAZO_LIMPIEZA);
}

/// Corre el comando de limpieza con plazo y luego mata el hijo. Sin plazo, un podman colgado bloquearía a Root.
fn terminar_con(comando: Command, hijo: &mut Child, plazo: Duration) {
    let _ = correr_comando(comando, "rm", plazo);
    matar(hijo);
}

/// Últimos `COLA` caracteres de un texto.
fn cola(texto: &str) -> String {
    let total = texto.chars().count();
    texto.chars().skip(total.saturating_sub(COLA)).collect()
}

/// Da `Infra` si el código de salida indica un fallo del entorno y no del comando evaluado.
fn error_de_infra(codigo: i32, stderr: &str) -> Option<ErrorOraculo> {
    if CODIGOS_DE_INFRA.contains(&codigo) {
        return Some(ErrorOraculo::Infra(format!(
            "podman falló ({codigo}): {}",
            cola(stderr)
        )));
    }
    None
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::mpsc;

    /// Mensaje de `Infra` para un código, o `None` si el código no es de infraestructura.
    fn infra_de(codigo: i32) -> Option<String> {
        match error_de_infra(codigo, "detalle") {
            Some(ErrorOraculo::Infra(mensaje)) => Some(mensaje),
            _ => None,
        }
    }

    #[test]
    fn error_de_infra_marca_137_como_infra() {
        let mensaje = infra_de(137).expect("137 es infraestructura");
        assert!(mensaje.contains("137"), "{mensaje}");
        assert!(mensaje.contains("detalle"), "{mensaje}");
    }

    #[test]
    fn error_de_infra_marca_125_126_127_como_infra() {
        for codigo in [125, 126, 127] {
            let mensaje = infra_de(codigo).expect("código de infraestructura");
            assert!(mensaje.contains(&codigo.to_string()), "{mensaje}");
            assert!(mensaje.starts_with("podman falló ("), "{mensaje}");
        }
    }

    #[test]
    fn error_de_infra_deja_pasar_0_1_y_2() {
        for codigo in [0, 1, 2] {
            assert!(infra_de(codigo).is_none(), "código {codigo}");
        }
    }

    #[test]
    fn turno_de_contenedor_nunca_deja_dos_ejecuciones_a_la_vez() {
        let dentro = AtomicUsize::new(0);
        let maximo = AtomicUsize::new(0);
        std::thread::scope(|s| {
            for _ in 0..4 {
                s.spawn(|| {
                    for _ in 0..5 {
                        let _turno = turno_de_contenedor();
                        let ahora = dentro.fetch_add(1, Ordering::SeqCst) + 1;
                        maximo.fetch_max(ahora, Ordering::SeqCst);
                        thread::sleep(Duration::from_millis(5));
                        dentro.fetch_sub(1, Ordering::SeqCst);
                    }
                });
            }
        });
        assert_eq!(maximo.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn turno_de_contenedor_no_lo_retiene_quien_espera_al_juez() {
        // El turno es global del proceso: otros tests con podman real lo retienen durante su corrida. El plazo
        // cubre esa espera; no mide nada.
        let plazo = Duration::from_secs(300);
        let (listo_tx, listo_rx) = mpsc::channel::<()>();
        let (juez_tx, juez_rx) = mpsc::channel::<()>();
        let (turno_tx, turno_rx) = mpsc::channel::<()>();
        // Hilo del caso: corre su parte de podman, suelta el turno y espera la respuesta del juez.
        thread::spawn(move || {
            drop(turno_de_contenedor());
            listo_tx.send(()).expect("avisa");
            juez_rx.recv_timeout(plazo).expect("el juez responde");
        });
        listo_rx
            .recv_timeout(plazo)
            .expect("el caso suelta el turno");
        // Otro caso toma el turno mientras el primero espera al juez.
        thread::spawn(move || {
            let _turno = turno_de_contenedor();
            turno_tx.send(()).expect("avisa");
        });
        turno_rx
            .recv_timeout(plazo)
            .expect("el turno queda libre mientras un caso espera al juez");
        juez_tx.send(()).expect("responde");
    }

    fn ejecucion() -> Ejecucion {
        Ejecucion {
            imagen: "docker.io/library/alpine:3.20".into(),
            repo: Some(PathBuf::from("/tmp/r")),
            oraculo: Some(PathBuf::from("/tmp/o")),
            oraculo_destino: None,
            salida: PathBuf::from("/tmp/s"),
            cmd: "pytest -q".into(),
            env: vec![("PYTHONPATH".into(), "/repo".into())],
            timeout: Duration::from_secs(60),
            red: false,
            cache: None,
            cache_rw: false,
            rustsec: None,
            trabajo: None,
            secretos: Vec::new(),
        }
    }

    fn contiene_par(args: &[String], a: &str, b: &str) -> bool {
        args.windows(2).any(|p| p[0] == a && p[1] == b)
    }

    #[test]
    fn contenedor_argumentos_cortan_la_red_y_montan_con_selinux() {
        let args = argumentos(&ejecucion(), "root-oraculo-1-1");
        assert!(contiene_par(&args, "--network", "none"));
        assert!(args.contains(&"/tmp/r:/repo:ro,z".to_string()));
        assert!(args.contains(&"/tmp/o:/oraculo:ro,z".to_string()));
        assert!(args.contains(&"/tmp/s:/salida:z".to_string()));
        assert!(contiene_par(&args, "--tmpfs", "/tmp:rw"));
        assert!(contiene_par(&args, "--name", "root-oraculo-1-1"));
        assert_eq!(args.first().map(String::as_str), Some("run"));
    }

    #[test]
    fn contenedor_argumentos_terminan_con_imagen_sh_c_y_comando() {
        let args = argumentos(&ejecucion(), "n");
        let fin: Vec<&str> = args[args.len() - 4..].iter().map(String::as_str).collect();
        assert_eq!(
            fin,
            ["docker.io/library/alpine:3.20", "sh", "-c", "pytest -q"]
        );
    }

    #[test]
    fn contenedor_argumentos_pasan_cada_variable_de_entorno() {
        let args = argumentos(&ejecucion(), "n");
        assert!(contiene_par(&args, "-e", "PYTHONPATH=/repo"));
    }

    #[test]
    fn contenedor_sin_repo_ni_oraculo_solo_monta_salida() {
        let mut e = ejecucion();
        e.repo = None;
        e.oraculo = None;
        let args = argumentos(&e, "n");
        assert_eq!(args.iter().filter(|a| *a == "-v").count(), 1);
        assert!(args.contains(&"/tmp/s:/salida:z".to_string()));
    }

    #[test]
    fn contenedor_secreto_va_por_entorno_y_no_en_los_argumentos() {
        let mut e = ejecucion();
        e.secretos = vec![(
            "CARGO_REGISTRIES_PRIV_TOKEN".into(),
            "valor-secreto-9".into(),
        )];
        let args = argumentos(&e, "n");
        assert!(
            contiene_par(&args, "-e", "CARGO_REGISTRIES_PRIV_TOKEN"),
            "{args:?}"
        );
        assert!(
            !args.iter().any(|a| a.contains("valor-secreto-9")),
            "el valor no va en los argumentos: {args:?}"
        );
        let depuracion = format!("{e:?}");
        assert!(
            !depuracion.contains("valor-secreto-9"),
            "el Debug no muestra el valor: {depuracion}"
        );
        assert!(depuracion.contains("CARGO_REGISTRIES_PRIV_TOKEN"));
    }

    #[test]
    fn contenedor_argumentos_llevan_http_proxy_false_con_y_sin_red() {
        for red in [false, true] {
            let mut e = ejecucion();
            e.red = red;
            let args = argumentos(&e, "n");
            let posicion = args.iter().position(|a| a == "--http-proxy=false");
            let imagen = args
                .iter()
                .position(|a| a == "docker.io/library/alpine:3.20");
            assert!(
                posicion.is_some() && posicion < imagen,
                "red={red}: el proxy del host no entra al contenedor: {args:?}"
            );
        }
    }

    #[test]
    fn contenedor_trabajo_se_monta_solo_lectura_en_trabajo_y_sin_repo() {
        let mut e = ejecucion();
        e.repo = None;
        e.trabajo = Some(PathBuf::from("/tmp/t"));
        let args = argumentos(&e, "n");
        assert!(
            args.contains(&"/tmp/t:/trabajo:ro,z".to_string()),
            "{args:?}"
        );
        assert!(
            !args.iter().any(|a| a.contains(":/repo")),
            "sin repo no hay montaje de /repo: {args:?}"
        );
    }

    #[test]
    fn contenedor_oraculo_monta_en_su_destino_de_la_suite() {
        let mut e = ejecucion();
        e.oraculo = Some(PathBuf::from("/t/oraculos/x/contrato"));
        e.oraculo_destino = Some("/oraculo/contrato".into());
        let args = argumentos(&e, "n");
        assert!(
            args.contains(&"/t/oraculos/x/contrato:/oraculo/contrato:ro,z".to_string()),
            "{args:?}"
        );
        assert!(
            !args.iter().any(|a| a.ends_with(":/oraculo:ro,z")),
            "{args:?}"
        );
    }

    #[test]
    fn contenedor_sufijo_selinux_es_z() {
        assert_eq!(SUFIJO_SELINUX, "z");
    }

    #[test]
    fn contenedor_cola_guarda_los_ultimos_4000_caracteres() {
        let texto = format!("{}FIN", "a".repeat(5000));
        let c = cola(&texto);
        assert_eq!(c.chars().count(), 4000);
        assert!(c.ends_with("FIN"));
    }

    /// Flujo de `pendiente` bytes `a` seguidos de `fin`. Entrega por trozos y no guarda el total.
    struct Sintetico {
        pendiente: usize,
        fin: &'static [u8],
    }

    impl io::Read for Sintetico {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.pendiente > 0 {
                let n = buf.len().min(self.pendiente);
                buf[..n].fill(b'a');
                self.pendiente -= n;
                return Ok(n);
            }
            let n = buf.len().min(self.fin.len());
            buf[..n].copy_from_slice(&self.fin[..n]);
            self.fin = &self.fin[n..];
            Ok(n)
        }
    }

    #[test]
    fn contenedor_cola_de_100_mb_guarda_solo_los_ultimos_bytes() {
        let flujo = Sintetico {
            pendiente: 100 * 1024 * 1024,
            fin: b"FIN",
        };
        let bytes = leer_cola_en_hilo(flujo).join().expect("hilo");
        assert!(
            bytes.len() <= COLA_BYTES,
            "quedaron {} bytes de 100 MB",
            bytes.len()
        );
        assert!(bytes.ends_with(b"FIN"));
        let texto = recoger(Some(leer_cola_en_hilo(Sintetico {
            pendiente: 100 * 1024 * 1024,
            fin: b"FIN",
        })));
        assert_eq!(cola(&texto).chars().count(), COLA);
        assert!(cola(&texto).ends_with("FIN"));
    }

    /// Tope de la cola de salida de podman: 1 MiB.
    const MIB: usize = 1024 * 1024;

    #[test]
    fn contenedor_cola_con_tope_de_1_mib_guarda_exactamente_1_mib() {
        let flujo = Sintetico {
            pendiente: 3 * MIB,
            fin: b"FIN",
        };
        let bytes = leer_cola_en_hilo_con(flujo, MIB).join().expect("hilo");
        assert_eq!(bytes.len(), MIB, "quedaron {} bytes de 3 MiB", bytes.len());
        assert!(bytes.ends_with(b"FIN"));
    }

    #[test]
    fn contenedor_correr_comando_conserva_solo_la_cola_de_1_mib() {
        // 3 MiB por stdout y luego "FIN". La cola debe tener 1 MiB y terminar en "FIN".
        let mut comando = Command::new("sh");
        comando.args(["-c", "head -c 3145728 /dev/zero | tr '\\0' a; printf FIN"]);
        let salida = correr_comando(comando, "sh", Duration::from_secs(30)).expect("corre");
        assert_eq!(
            salida.stdout.len(),
            MIB,
            "quedaron {} bytes de 3 MiB",
            salida.stdout.len()
        );
        assert!(salida.stdout.ends_with("FIN"));
    }

    #[test]
    fn contenedor_cola_no_corta_utf8_a_medias() {
        let texto = format!("{}FIN", "€".repeat(20_000));
        let flujo = io::Cursor::new(texto.into_bytes());
        let leido = recoger(Some(leer_cola_en_hilo(flujo)));
        assert_eq!(cola(&leido), format!("{}FIN", "€".repeat(COLA - 3)));
    }

    #[test]
    fn contenedor_limpieza_con_plazo_no_cuelga_si_podman_no_responde() {
        let mut hijo = Command::new("sleep").arg("20").spawn().expect("sleep");
        let inicio = Instant::now();
        let mut podman_falso = Command::new("sh");
        podman_falso.args(["-c", "exec sleep 20"]);
        terminar_con(podman_falso, &mut hijo, Duration::from_millis(300));
        assert!(
            inicio.elapsed() < Duration::from_secs(10),
            "la limpieza tardó {:?}",
            inicio.elapsed()
        );
        assert!(
            hijo.try_wait().expect("try_wait").is_some(),
            "el hijo quedó vivo"
        );
    }

    #[test]
    fn contenedor_argumentos_limitan_privilegios_recursos_y_pull() {
        for red in [false, true] {
            let mut e = ejecucion();
            e.red = red;
            let args = argumentos(&e, "n");
            let imagen = args
                .iter()
                .position(|a| a == "docker.io/library/alpine:3.20")
                .expect("imagen");
            for opcion in [
                "--cap-drop=all",
                "--security-opt=no-new-privileges",
                "--pids-limit=512",
                "--memory=2g",
                "--pull=never",
            ] {
                let pos = args.iter().position(|a| a == opcion);
                assert!(
                    pos.is_some_and(|p| p < imagen),
                    "red={red}: falta {opcion} antes de la imagen: {args:?}"
                );
            }
        }
    }

    /// Flujo de `restante` bytes que no aloca el total. Cuenta cuántos bytes entregó.
    struct Contado {
        restante: usize,
        entregado: Arc<AtomicUsize>,
    }

    impl io::Read for Contado {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let n = buf.len().min(self.restante);
            buf[..n].fill(b'a');
            self.restante -= n;
            self.entregado.fetch_add(n, Ordering::SeqCst);
            Ok(n)
        }
    }

    /// Lector que pide reintentar una vez y después da sus datos.
    struct Interrumpido {
        interrumpio: bool,
        datos: &'static [u8],
    }

    impl io::Read for Interrumpido {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if !self.interrumpio {
                self.interrumpio = true;
                return Err(io::Error::from(io::ErrorKind::Interrupted));
            }
            let n = buf.len().min(self.datos.len());
            buf[..n].copy_from_slice(&self.datos[..n]);
            self.datos = &self.datos[n..];
            Ok(n)
        }
    }

    #[test]
    fn leer_en_hilo_guarda_64_mib_y_drena_los_70_mib() {
        let total = 70 * 1024 * 1024;
        let entregado = Arc::new(AtomicUsize::new(0));
        let flujo = Contado {
            restante: total,
            entregado: Arc::clone(&entregado),
        };
        let lectura = leer_en_hilo(flujo).join().expect("hilo");
        assert_eq!(lectura.bytes.len(), 64 * 1024 * 1024);
        assert!(lectura.recortada);
        assert_eq!(entregado.load(Ordering::SeqCst), total);
    }

    #[test]
    fn leer_en_hilo_salida_chica_se_lee_completa() {
        let lectura = leer_en_hilo(&b"hola\n"[..]).join().expect("hilo");
        assert_eq!(
            lectura,
            Lectura {
                bytes: b"hola\n".to_vec(),
                recortada: false
            }
        );
    }

    #[test]
    fn leer_en_hilo_con_tope_chico_guarda_el_inicio_y_marca_recorte() {
        let lectura = leer_en_hilo_con(&b"abcdefghij"[..], 4)
            .join()
            .expect("hilo");
        assert_eq!(
            lectura,
            Lectura {
                bytes: b"abcd".to_vec(),
                recortada: true
            }
        );
    }

    #[test]
    fn modo_cola_conserva_los_ultimos_bytes_y_marca_recorte() {
        let lectura = leer_con_tope(&b"abcdefghij"[..], 4, Modo::Cola);
        assert_eq!(
            lectura,
            Lectura {
                bytes: b"ghij".to_vec(),
                recortada: true
            }
        );
    }

    #[test]
    fn modo_cola_con_salida_del_tope_exacto_no_marca_recorte() {
        let lectura = leer_con_tope(&b"abcd"[..], 4, Modo::Cola);
        assert_eq!(
            lectura,
            Lectura {
                bytes: b"abcd".to_vec(),
                recortada: false
            }
        );
    }

    #[test]
    fn modo_cola_no_empieza_a_mitad_de_un_caracter() {
        // "€€€" ocupa 9 bytes. Con tope 4, el corte cae dentro del segundo "€".
        let lectura = leer_con_tope("€€€".as_bytes(), 4, Modo::Cola);
        assert_eq!(lectura.bytes, "€".as_bytes());
        assert!(lectura.recortada);
    }

    #[test]
    fn modo_cola_con_flujo_largo_no_empieza_a_mitad_de_un_caracter() {
        // Pasa el doble del tope. El recorte intermedio no debe cambiar el resultado final.
        let texto = "€".repeat(100);
        let lectura = leer_con_tope(texto.as_bytes(), 4, Modo::Cola);
        assert_eq!(lectura.bytes, "€".as_bytes());
    }

    #[test]
    fn cola_de_podman_no_empieza_a_mitad_de_un_caracter() {
        let flujo = io::Cursor::new("€€€".as_bytes().to_vec());
        let bytes = leer_cola_en_hilo_con(flujo, 4).join().expect("hilo");
        assert_eq!(bytes, "€".as_bytes());
    }

    #[test]
    fn leer_con_tope_reintenta_si_la_lectura_se_interrumpe() {
        let flujo = Interrumpido {
            interrumpio: false,
            datos: b"ok",
        };
        let lectura = leer_con_tope(flujo, 10, Modo::Inicio);
        assert_eq!(lectura.bytes, b"ok".to_vec());
        assert!(!lectura.recortada);
    }

    /// Flujo que entrega los datos en trozos fijos. Así se prueba una línea partida entre dos lecturas.
    struct PorTrozos {
        trozos: Vec<Vec<u8>>,
    }

    impl io::Read for PorTrozos {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.trozos.is_empty() {
                return Ok(0);
            }
            let trozo = self.trozos.remove(0);
            buf[..trozo.len()].copy_from_slice(&trozo);
            Ok(trozo.len())
        }
    }

    #[test]
    fn es_linea_step_reconoce_el_paso_con_y_sin_prefijo_de_etapa() {
        assert!(es_linea_step("STEP 1/3: FROM x"));
        assert!(es_linea_step("   STEP 2/2: RUN a"));
        assert!(es_linea_step("[2/2] STEP 3/3: RUN wget y"));
        assert!(!es_linea_step("Error: building at STEP \"RUN b\""));
        assert!(!es_linea_step("STEP x/3: RUN a"));
        assert!(!es_linea_step("STEP 1/3 RUN a"));
    }

    #[test]
    fn texto_de_linea_step_normaliza_los_espacios() {
        assert_eq!(
            texto_de_linea_step("STEP 3/3: RUN  b ").as_deref(),
            Some("RUN b")
        );
        assert_eq!(
            texto_de_linea_step("[2/2] STEP 3/3: WORKDIR /srv").as_deref(),
            Some("WORKDIR /srv")
        );
        assert_eq!(texto_de_linea_step("sin paso"), None);
    }

    #[test]
    fn lector_step_une_una_linea_partida_entre_dos_lecturas() {
        let flujo = PorTrozos {
            trozos: vec![b"a\nSTEP 3/3: R".to_vec(), b"UN x\nfin\n".to_vec()],
        };
        let (cola, step) = leer_salida_con_step(flujo, MIB);
        assert_eq!(cola, b"a\nSTEP 3/3: RUN x\nfin\n".to_vec());
        assert_eq!(step.as_deref(), Some("STEP 3/3: RUN x"));
    }

    #[test]
    fn lector_step_recuerda_la_ultima_linea_que_la_cola_ya_perdio() {
        let mut stream = b"STEP 1/2: FROM x\nSTEP 2/2: RUN a\n".to_vec();
        stream.extend(vec![b'y'; 3 * MIB]);
        stream.push(b'\n');
        let (cola, step) = leer_salida_con_step(&stream[..], MIB);
        assert_eq!(cola.len(), MIB);
        assert!(!String::from_utf8_lossy(&cola).contains("STEP"));
        assert_eq!(step.as_deref(), Some("STEP 2/2: RUN a"));
    }

    #[test]
    fn lector_step_con_linea_mas_larga_que_el_tope_la_guarda_recortada() {
        let mut stream = b"STEP 2/2: RUN ".to_vec();
        stream.extend(vec![b'z'; 100 * 1024]);
        stream.extend(b"\nfin\n");
        let (_, step) = leer_salida_con_step(&stream[..], MIB);
        let step = step.expect("guarda el STEP");
        assert!(step.starts_with("STEP 2/2: RUN "));
        assert_eq!(step.len(), TOPE_LINEA_STEP);
    }

    #[test]
    fn lector_step_sin_step_no_guarda_nada() {
        let (cola, step) = leer_salida_con_step(&b"hola\nmundo\n"[..], MIB);
        assert_eq!(cola, b"hola\nmundo\n".to_vec());
        assert_eq!(step, None);
    }

    #[test]
    fn contenedor_correr_comando_guarda_el_step_que_la_cola_perdio_en_paso() {
        let mut comando = Command::new("sh");
        comando.args([
            "-c",
            "printf 'STEP 3/5: RUN x\\n'; head -c 2500000 /dev/zero | tr '\\0' y; echo",
        ]);
        let salida = correr_comando(comando, "sh", Duration::from_secs(30)).expect("corre");
        assert_eq!(salida.paso.as_deref(), Some("STEP 3/5: RUN x"));
        assert!(!salida.stdout.starts_with("STEP"));
        assert_eq!(salida.stdout.len(), MIB);
    }

    #[test]
    fn contenedor_correr_comando_salida_chica_con_step_no_lo_duplica() {
        let mut comando = Command::new("sh");
        comando.args(["-c", "printf 'STEP 1/1: RUN x\\nerror\\n'"]);
        let salida = correr_comando(comando, "sh", Duration::from_secs(30)).expect("corre");
        assert_eq!(salida.stdout, "STEP 1/1: RUN x\nerror\n");
    }

    #[test]
    fn topes_de_salida_valen_64_y_16_mib() {
        assert_eq!(TOPE_SALIDA_BUILDER, 64 * 1024 * 1024);
        assert_eq!(TOPE_SALIDA_COLAB, 16 * 1024 * 1024);
    }

    #[test]
    fn tamano_legible_usa_mib_solo_si_es_exacto() {
        assert_eq!(tamano_legible(TOPE_SALIDA_BUILDER), "64 MiB");
        assert_eq!(tamano_legible(1000), "1000 bytes");
    }

    #[test]
    fn argumentos_separado_deja_el_contenedor_vivo_y_sin_rm() {
        let args = argumentos_separado(&ejecucion(), "root-oraculo-1-1", &["sleep", "600"]);
        let inicio: Vec<&str> = args[..4].iter().map(String::as_str).collect();
        assert_eq!(inicio, ["run", "-d", "--name", "root-oraculo-1-1"]);
        assert!(!args.iter().any(|a| a == "--rm"), "{args:?}");
        assert!(args.contains(&"--userns=keep-id".to_string()), "{args:?}");
        assert!(!args.iter().any(|a| a == "--user"), "{args:?}");
    }

    #[test]
    fn argumentos_separado_no_corre_como_root_del_contenedor() {
        // Con `--userns=keep-id` el proceso es el usuario del host, sin privilegios dentro. Así el producto
        // no puede reemplazar `/bin/sh` ni `/usr/bin/timeout` de la imagen.
        let args = argumentos_separado(&ejecucion(), "n", &["sleep", "600"]);
        let userns = args.iter().position(|a| a == "--userns=keep-id");
        let imagen = args
            .iter()
            .position(|a| a == "docker.io/library/alpine:3.20");
        assert!(userns.is_some() && userns < imagen, "{args:?}");
    }

    #[test]
    fn argumentos_con_rm_no_llevan_userns_keep_id() {
        // GUARDA: el camino de `suites`/`ejecutar` (`podman run --rm`) no cambia.
        let args = argumentos(&ejecucion(), "n");
        assert!(!args.iter().any(|a| a.contains("keep-id")), "{args:?}");
        assert!(!args.iter().any(|a| a.starts_with("--userns")), "{args:?}");
    }

    #[test]
    fn argumentos_separado_mantiene_tmp_escribible_como_tmpfs() {
        // GUARDA: `/tmp` sigue siendo tmpfs con escritura. Con keep-id el usuario de host lo escribe (1777 por defecto).
        let args = argumentos_separado(&ejecucion(), "n", &["sleep", "600"]);
        assert!(contiene_par(&args, "--tmpfs", "/tmp:rw"), "{args:?}");
    }

    #[test]
    fn argumentos_separado_lleva_el_mismo_endurecimiento_y_montajes() {
        let args = argumentos_separado(&ejecucion(), "n", &["sleep", "600"]);
        assert!(args.contains(&"--cap-drop=all".to_string()), "{args:?}");
        assert!(
            args.contains(&"--security-opt=no-new-privileges".to_string()),
            "{args:?}"
        );
        assert!(contiene_par(&args, "--network", "none"), "{args:?}");
        assert!(args.contains(&"/tmp/r:/repo:ro,z".to_string()), "{args:?}");
        assert!(args.contains(&"/tmp/s:/salida:z".to_string()), "{args:?}");
    }

    #[test]
    fn argumentos_separado_pone_el_comando_con_entrypoint_antes_de_la_imagen() {
        let args = argumentos_separado(&ejecucion(), "n", &["sleep", "600"]);
        assert!(contiene_par(&args, "--entrypoint", "sleep"), "{args:?}");
        let entrada = args.iter().position(|a| a == "--entrypoint");
        let imagen = args
            .iter()
            .position(|a| a == "docker.io/library/alpine:3.20");
        assert!(entrada < imagen, "{args:?}");
        let fin: Vec<&str> = args[args.len() - 2..].iter().map(String::as_str).collect();
        assert_eq!(fin, ["docker.io/library/alpine:3.20", "600"]);
    }

    #[test]
    fn hijo_podman_terminado_dice_si_ya_salio() {
        let mut hijo = lanzar_programa("sh", &["-c", "echo listo"]).expect("lanza");
        let inicio = Instant::now();
        while !hijo.terminado().expect("consulta") {
            assert!(
                inicio.elapsed() < Duration::from_secs(20),
                "el hijo no salió"
            );
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(hijo.terminar(), "listo\n");
    }

    #[test]
    fn hijo_podman_terminar_mata_y_da_stdout_seguido_de_stderr() {
        let mut hijo = lanzar_programa("sh", &["-c", "echo hola; echo err >&2; exec sleep 20"])
            .expect("lanza");
        // Da tiempo a que el hijo escriba antes de matarlo.
        thread::sleep(Duration::from_millis(500));
        assert!(!hijo.terminado().expect("consulta"), "el hijo vivo");
        assert_eq!(hijo.terminar(), "hola\nerr\n");
        assert!(hijo.terminado().expect("consulta"), "quedó muerto");
    }

    #[test]
    fn hijo_podman_terminar_no_cuelga_si_un_nieto_tiene_el_pipe() {
        // El `sleep` del nieto hereda el pipe. Matar `sh` no cierra el pipe: `terminar` no debe esperarlo.
        let mut hijo = lanzar_programa("sh", &["-c", "echo hola; sleep 20"]).expect("lanza");
        thread::sleep(Duration::from_millis(300));
        let inicio = Instant::now();
        hijo.terminar();
        assert!(
            inicio.elapsed() < Duration::from_secs(10),
            "terminar tardó {:?}",
            inicio.elapsed()
        );
    }
}
