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
/// Tope de procesos e hilos del contenedor. Acota una fork bomb dentro de la imagen.
const PIDS_LIMITE: u32 = 512;
/// Memoria máxima del contenedor. Sube si una suite legítima llega al tope (ver el informe de medición).
const MEMORIA: &str = "2g";
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
    args.push(e.imagen.clone());
    args.extend(["sh", "-c"].map(String::from));
    args.push(e.cmd.clone());
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
        .map(|f| leer_cola_en_hilo_con(f, TOPE_SALIDA_PODMAN));
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
    Ok(SalidaPodman {
        codigo,
        stdout: recoger(salida),
        stderr: recoger(error),
    })
}

/// Mata el proceso y espera su fin. Los errores se ignoran: el llamador ya reporta el fallo.
fn matar(hijo: &mut Child) {
    let _ = hijo.kill();
    let _ = hijo.wait();
}

/// Falla antes de lanzar podman si falta alguna ruta montada.
fn validar_rutas(e: &Ejecucion) -> Result<(), ErrorOraculo> {
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

/// Lee un flujo completo en otro hilo para que el proceso no se bloquee al llenar el pipe.
pub(crate) fn leer_en_hilo<R: Read + Send + 'static>(mut flujo: R) -> JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = flujo.read_to_end(&mut bytes);
        bytes
    })
}

/// Lee un flujo en otro hilo y conserva solo los últimos `COLA_BYTES` bytes. La memoria no crece con la salida.
pub(crate) fn leer_cola_en_hilo<R: Read + Send + 'static>(flujo: R) -> JoinHandle<Vec<u8>> {
    leer_cola_en_hilo_con(flujo, COLA_BYTES)
}

/// Lee un flujo en otro hilo y conserva solo los últimos `tope` bytes. El búfer nunca pasa de `2 * tope`.
pub(crate) fn leer_cola_en_hilo_con<R: Read + Send + 'static>(
    mut flujo: R,
    tope: usize,
) -> JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut cola = Vec::new();
        let mut trozo = [0u8; 8192];
        loop {
            match flujo.read(&mut trozo) {
                Ok(0) => break,
                Ok(n) => {
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
        if cola.len() > tope {
            cola.drain(..cola.len() - tope);
        }
        cola
    })
}

pub(crate) fn recoger(hilo: Option<JoinHandle<Vec<u8>>>) -> String {
    let bytes = hilo.and_then(|h| h.join().ok()).unwrap_or_default();
    String::from_utf8_lossy(&bytes).into_owned()
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
}
