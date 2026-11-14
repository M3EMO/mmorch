//! Limpieza de los contenedores de `oraculo evaluar`, `calibrar` y `auditar`.
//! Cada contenedor lleva en su nombre el pid del `r00t` que lo creó: `<prefijo><pid>-<n>`.
//! Al recibir SIGINT o SIGTERM, el proceso quita los suyos. Al arrancar un contenedor, quita los
//! de dueños muertos o reusados. SIGKILL no se atrapa: el barrido cubre ese caso.
//!
//! Este módulo no toma el turno de podman. La señal puede llegar mientras el hilo principal tiene
//! el turno en un contenedor largo. Tomarlo dejaría la limpieza colgada.

use std::fs;
use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::process::Command;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use signal_hook::consts::{SIGINT, SIGTERM};
use signal_hook::iterator::{Handle, Signals};

use crate::Error;
use crate::entorno::{VARIABLE_SIN_PODMAN, debe_saltear};
use crate::oracle::ErrorOraculo;
use crate::oracle::contenedor::{PLAZO_LIMPIEZA, SalidaPodman, correr_comando};

/// Prefijos de los contenedores de este código. Cada nombre es `<prefijo><pid>-<n>`.
const PREFIJOS: [&str; 3] = ["root-oraculo-", "root-entrega-esc-", "root-entrega-eval-"];
/// Plazo total para quitar los contenedores al recibir una señal. Igual a `PLAZO_AL_SENAL` de `corre`.
const PLAZO_AL_SENAL: Duration = Duration::from_secs(10);
/// Plazo total del barrido al arrancar. Es el de la limpieza de `oracle::contenedor`.
const PLAZO_BARRIDO: Duration = PLAZO_LIMPIEZA;
/// Carpeta donde Linux muestra los procesos vivos.
const PROCESOS: &str = "/proc";
/// Programa de podman en producción.
const PODMAN: &str = "podman";

/// Quita los contenedores de procesos muertos de la familia. Sin turno de podman: quien lo llama
/// debe tenerlo. Un fallo se avisa y no corta la corrida.
pub(crate) fn barrer_al_arrancar(programa: &str) {
    let sin_podman = std::env::var(VARIABLE_SIN_PODMAN).ok();
    barrer_al_arrancar_con(programa, sin_podman.as_deref());
}

/// Igual que `barrer_al_arrancar`, con el valor de `ROOT_SIN_PODMAN` pasado por quien llama.
/// Así los tests no cambian el entorno del proceso. Con `ROOT_SIN_PODMAN=1` no llama al programa.
fn barrer_al_arrancar_con(programa: &str, sin_podman: Option<&str>) {
    if debe_saltear(sin_podman) {
        return;
    }
    match barrer(programa, Path::new(PROCESOS), PLAZO_BARRIDO) {
        Ok(quitados) if quitados.is_empty() => {}
        Ok(quitados) => eprintln!(
            "r00t oraculo: barrido: se quitaron contenedores de procesos muertos: {}",
            quitados.join(" ")
        ),
        Err(error) => {
            eprintln!("r00t oraculo: barrido: no se pudieron quitar contenedores: {error}")
        }
    }
}

/// Quita los contenedores cuyo dueño no vive en `raiz_proc`. Devuelve sus nombres.
pub(crate) fn barrer(
    programa: &str,
    raiz_proc: &Path,
    plazo: Duration,
) -> Result<Vec<String>, ErrorOraculo> {
    let inicio = Instant::now();
    let nombres = listar(programa, plazo)?;
    let muertos = huerfanos(&nombres, raiz_proc);
    if muertos.is_empty() {
        return Ok(muertos);
    }
    quitar(programa, &muertos, plazo.saturating_sub(inicio.elapsed()))?;
    Ok(muertos)
}

/// Los nombres de `nombres` que llevan el pid `pid` en su prefijo.
pub(crate) fn nombres_de_pid(nombres: &[String], pid: u32) -> Vec<String> {
    nombres
        .iter()
        .filter(|nombre| pid_del_nombre(nombre) == Some(pid))
        .cloned()
        .collect()
}

/// Los nombres de la familia de este código cuyo dueño no vive en `raiz_proc`. Un nombre de la
/// familia sin pid legible cuenta como huérfano.
pub(crate) fn huerfanos(contenedores: &[Contenedor], raiz_proc: &Path) -> Vec<String> {
    contenedores
        .iter()
        .filter(|c| de_la_familia(&c.nombre))
        .filter(|c| match pid_del_nombre(&c.nombre) {
            None => true,
            Some(pid) => !dueno_vivo(pid, c.creado, raiz_proc),
        })
        .map(|c| c.nombre.clone())
        .collect()
}

/// Un contenedor de la familia, tal como lo lista podman.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Contenedor {
    nombre: String,
    /// Segundos desde la época en que se creó. `None` si podman no dio una hora legible.
    creado: Option<u64>,
}

/// Verdadero si el nombre empieza con un prefijo de este código.
fn de_la_familia(nombre: &str) -> bool {
    PREFIJOS.iter().any(|prefijo| nombre.starts_with(prefijo))
}

/// El pid de `<prefijo><pid>-<n>`. `None` si el nombre no es de la familia o el pid no se lee.
fn pid_del_nombre(nombre: &str) -> Option<u32> {
    let resto = PREFIJOS
        .iter()
        .find_map(|prefijo| nombre.strip_prefix(prefijo))?;
    resto.split('-').next()?.parse().ok()
}

/// Margen en segundos entre el arranque del dueño y la creación de su contenedor. La laptop se
/// suspende y NTP corrige el reloj, así que `btime` deriva. Un dueño vivo no debe verse como pid
/// reusado por ese corrimiento.
const MARGEN_DUENO_S: u64 = 300;
/// Ticks del reloj del kernel por segundo (`USER_HZ`). En Linux es 100.
const TICKS_POR_SEG: u64 = 100;

/// Qué se sabe del dueño de algo que un proceso creó. Lo usan `barrer` (referencia = hora de
/// creación del contenedor) y `corre::barrido` (referencia = segundos del id de la corrida).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Dueno {
    /// El pid existe y arrancó a tiempo para ser el dueño.
    Vivo,
    /// No hay carpeta del pid en `/proc`.
    NoExiste,
    /// El pid existe, pero arrancó más de `MARGEN_DUENO_S` después de la referencia: es otro proceso.
    Reusado,
}

/// Dice qué pasa con el dueño `pid`, creado en `referencia` (segundos desde la época). El nombre
/// del binario no cuenta: solo importan la existencia del pid y su hora de arranque. Sin hora de
/// referencia o de arranque, cuenta como vivo: el error va del lado de no borrar.
pub(crate) fn dueno_de(pid: u32, referencia: Option<u64>, raiz_proc: &Path) -> Dueno {
    if pid == std::process::id() {
        return Dueno::Vivo;
    }
    if !raiz_proc.join(pid.to_string()).exists() {
        return Dueno::NoExiste;
    }
    let (Some(referencia), Some(inicio)) = (referencia, inicio_de_proceso(pid, raiz_proc)) else {
        return Dueno::Vivo;
    };
    if inicio > referencia + MARGEN_DUENO_S {
        Dueno::Reusado
    } else {
        Dueno::Vivo
    }
}

/// Dice si el dueño `pid` de un contenedor creado en `referencia` vive. Ver `dueno_de`.
pub(crate) fn dueno_vivo(pid: u32, referencia: Option<u64>, raiz_proc: &Path) -> bool {
    dueno_de(pid, referencia, raiz_proc) == Dueno::Vivo
}

/// Segundos de inicio del proceso `pid`: `btime` más el `starttime` en ticks. `None` si `stat` o
/// `btime` no se pueden leer o parsear.
fn inicio_de_proceso(pid: u32, raiz_proc: &Path) -> Option<u64> {
    let stat = fs::read_to_string(raiz_proc.join(pid.to_string()).join("stat")).ok()?;
    // El comm va entre paréntesis y puede tener espacios o `)`: se parte después del último `)`.
    // Después del `)` el primer campo es el 3 (state). El campo 22 (starttime) es el índice 19.
    let despues_del_comm = stat.rsplit_once(')')?.1;
    let ticks: u64 = despues_del_comm.split_whitespace().nth(19)?.parse().ok()?;
    Some(btime_del_sistema(raiz_proc)? + ticks / TICKS_POR_SEG)
}

/// Segundos desde el arranque del sistema, de la línea `btime` de `<raiz_proc>/stat`.
fn btime_del_sistema(raiz_proc: &Path) -> Option<u64> {
    let stat = fs::read_to_string(raiz_proc.join("stat")).ok()?;
    stat.lines()
        .find_map(|linea| linea.strip_prefix("btime "))
        .and_then(|valor| valor.trim().parse().ok())
}

/// Lista los contenedores que coinciden con `root-` en `programa ps`. Sin filtrar por familia.
///
/// El formato pide el nombre y la hora de creación en segundos desde la época:
/// `{{.ListContainer.Created.Unix}}`. La plantilla `{{.Created}}` de `podman ps` da texto
/// `Y-M-D H:M:S`, no epoch, y por eso no sirve. Si la hora llega en un formato ilegible, `creado`
/// queda en `None`. Un `None` hace que el contenedor se conserve.
fn listar(programa: &str, plazo: Duration) -> Result<Vec<Contenedor>, ErrorOraculo> {
    let salida = correr(
        programa,
        &[
            "ps",
            "-a",
            "--format",
            "{{.Names}} {{.ListContainer.Created.Unix}}",
            "--filter",
            "name=root-",
        ],
        plazo,
    )?;
    if salida.codigo != 0 {
        return Err(ErrorOraculo::Infra(format!(
            "ps salió con código {}: {}",
            salida.codigo,
            salida.stderr.trim()
        )));
    }
    Ok(parsear_listado(&salida.stdout))
}

/// Parte la salida de `ps` en contenedores. Cada línea: nombre, espacio, epoch. Las líneas vacías
/// se saltan. Un epoch que no parsea, o que falta, deja `creado` en `None`.
fn parsear_listado(salida: &str) -> Vec<Contenedor> {
    salida
        .lines()
        .filter_map(|linea| {
            let mut partes = linea.split_whitespace();
            let nombre = partes.next()?;
            let creado = partes.next().and_then(|texto| texto.parse().ok());
            Some(Contenedor {
                nombre: nombre.to_string(),
                creado,
            })
        })
        .collect()
}

/// Corre `rm -f -t 0` con `nombres`. Un código distinto de cero es error.
fn quitar(programa: &str, nombres: &[String], plazo: Duration) -> Result<(), ErrorOraculo> {
    let mut args = vec!["rm", "-f", "-t", "0"];
    args.extend(nombres.iter().map(String::as_str));
    let salida = correr(programa, &args, plazo)?;
    if salida.codigo != 0 {
        return Err(ErrorOraculo::Infra(format!(
            "rm salió con código {}: {}",
            salida.codigo,
            salida.stderr.trim()
        )));
    }
    Ok(())
}

/// Corre `programa` con `args` y espera hasta `plazo`. No toma el turno de podman.
fn correr(programa: &str, args: &[&str], plazo: Duration) -> Result<SalidaPodman, ErrorOraculo> {
    let mut comando = Command::new(programa);
    comando.args(args);
    correr_comando(comando, args.first().copied().unwrap_or(""), plazo)
}

/// Quita los contenedores de este proceso. Devuelve sus nombres.
fn quitar_de_pid(programa: &str, pid: u32, plazo: Duration) -> Result<Vec<String>, ErrorOraculo> {
    let nombres: Vec<String> = listar(programa, plazo)?
        .into_iter()
        .map(|c| c.nombre)
        .collect();
    let nombres = nombres_de_pid(&nombres, pid);
    if nombres.is_empty() {
        return Ok(nombres);
    }
    quitar(programa, &nombres, plazo)?;
    Ok(nombres)
}

/// Código de salida de una señal: 128 más su número.
fn codigo_de_senal(senal: i32) -> i32 {
    128 + senal
}

/// Qué hace el proceso al recibir `senal`. Quita sus contenedores y luego llama a `salir` con
/// `128 + senal`. Un pánico en la limpieza se atrapa aquí: sale igual, una sola vez.
fn al_recibir_senal(programa: &str, senal: i32, salir: impl Fn(i32)) {
    let pid = std::process::id();
    let limpieza = catch_unwind(AssertUnwindSafe(|| {
        #[cfg(test)]
        panico_de_prueba();
        quitar_de_pid(programa, pid, PLAZO_AL_SENAL)
    }));
    match limpieza {
        Ok(Ok(_)) => {}
        Ok(Err(error)) => eprintln!(
            "r00t oraculo: al recibir la señal {senal} no se pudieron quitar los contenedores: {error}"
        ),
        Err(payload) => eprintln!(
            "r00t oraculo: pánico al limpiar tras la señal {senal}: {}",
            texto_del_panico(payload.as_ref())
        ),
    }
    salir(codigo_de_senal(senal));
}

/// Texto de un pánico, para el mensaje. Sin texto si el payload no es `&str` ni `String`.
fn texto_del_panico(payload: &(dyn std::any::Any + Send)) -> &str {
    if let Some(texto) = payload.downcast_ref::<&str>() {
        texto
    } else if let Some(texto) = payload.downcast_ref::<String>() {
        texto.as_str()
    } else {
        "sin texto"
    }
}

/// Vigila SIGINT y SIGTERM mientras vive. Un hilo espera las señales.
pub(crate) struct Vigia {
    manejador: Handle,
    hilo: Option<JoinHandle<()>>,
}

impl Vigia {
    /// Registra SIGINT y SIGTERM y arranca el hilo que espera las señales.
    pub(crate) fn iniciar(programa: &str) -> io::Result<Vigia> {
        let mut senales = Signals::new([SIGINT, SIGTERM])?;
        let manejador = senales.handle();
        let programa = programa.to_string();
        let hilo = thread::spawn(move || {
            for senal in senales.forever() {
                al_recibir_senal(&programa, senal, |codigo| std::process::exit(codigo));
            }
        });
        Ok(Vigia {
            manejador,
            hilo: Some(hilo),
        })
    }
}

impl Drop for Vigia {
    fn drop(&mut self) {
        // Cerrar el manejador termina el `forever` del hilo. Luego el hilo se une.
        self.manejador.close();
        if let Some(hilo) = self.hilo.take() {
            let _ = hilo.join();
        }
    }
}

/// Corre `trabajo` con la vigilia de señales instalada. Al terminar, la vigilia se suelta.
pub(crate) fn con_vigia<T>(trabajo: impl FnOnce() -> Result<T, Error>) -> Result<T, Error> {
    let _vigia = Vigia::iniciar(PODMAN).map_err(|fuente| {
        ErrorOraculo::Infra(format!(
            "no se pudieron registrar las señales SIGINT y SIGTERM: {fuente}"
        ))
    })?;
    trabajo()
}

/// Si vale `true`, la limpieza de `al_recibir_senal` entra en pánico. Solo existe en los tests.
#[cfg(test)]
static PANICO_DE_PRUEBA: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Pánico de prueba dentro de la limpieza. Solo existe en los tests.
#[cfg(test)]
fn panico_de_prueba() {
    if PANICO_DE_PRUEBA.load(std::sync::atomic::Ordering::Relaxed) {
        panic!("pánico de prueba en la limpieza");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::sync::atomic::Ordering;
    use tempfile::TempDir;

    /// Serie de los tests que llaman a `al_recibir_senal` o usan el `podman` falso.
    /// La marca de pánico es global: un test no debe verla activa en otro.
    static SERIE: Mutex<()> = Mutex::new(());

    fn serie() -> std::sync::MutexGuard<'static, ()> {
        SERIE.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Hora de arranque de la máquina de los tests, en segundos desde la época.
    const BTIME: u64 = 1_700_000_000;

    /// Un contenedor de prueba con la hora de creación `creado`.
    fn contenedor(nombre: &str, creado: Option<u64>) -> Contenedor {
        Contenedor {
            nombre: nombre.to_string(),
            creado,
        }
    }

    /// `podman` falso: anota cada llamada en su log. Responde `ps` con una línea por contenedor:
    /// nombre y epoch. Sin hora, solo el nombre.
    fn podman_falso(base: &Path, contenedores: &[Contenedor]) -> (PathBuf, PathBuf) {
        let log = base.join("podman.log");
        let lista: String = contenedores
            .iter()
            .map(|c| match c.creado {
                Some(segundos) => format!("echo {} {segundos}\n", c.nombre),
                None => format!("echo {}\n", c.nombre),
            })
            .collect();
        let cuerpo = format!(
            "#!/bin/sh\necho \"$*\" >> '{}'\nif [ \"$1\" = ps ]; then\n{lista}fi\n",
            log.display()
        );
        let ruta = base.join("podman-falso");
        fs::write(&ruta, cuerpo).expect("script");
        fs::set_permissions(&ruta, fs::Permissions::from_mode(0o755)).expect("permisos");
        (ruta, log)
    }

    /// Un `/proc` falso con un proceso `pid` cuyo cmdline es `cmdline`.
    fn proc_con(base: &Path, pid: u32, cmdline: &[u8]) -> PathBuf {
        let raiz = base.join("proc");
        let carpeta = raiz.join(pid.to_string());
        fs::create_dir_all(&carpeta).expect("proc");
        fs::write(carpeta.join("cmdline"), cmdline).expect("cmdline");
        raiz
    }

    /// Un `/proc` falso con un proceso `pid` que arrancó `ticks` después de `btime`. Escribe
    /// `stat` del proceso y la línea `btime` de `<raiz>/stat`.
    fn proc_con_inicio(base: &Path, pid: u32, cmdline: &[u8], btime: u64, ticks: u64) -> PathBuf {
        let raiz = proc_con(base, pid, cmdline);
        let carpeta = raiz.join(pid.to_string());
        let stat =
            format!("{pid} (r00t) S 1 {pid} {pid} 0 -1 4194560 0 0 0 0 0 0 0 0 20 0 1 0 {ticks}\n");
        fs::write(carpeta.join("stat"), stat).expect("stat");
        fs::write(raiz.join("stat"), format!("cpu  1 2 3\nbtime {btime}\n")).expect("btime");
        raiz
    }

    #[test]
    fn senal_quita_los_contenedores_de_este_pid_y_no_otros() {
        let _serie = serie();
        let tmp = TempDir::new().expect("tmp");
        let pid = std::process::id();
        let nombres = [
            format!("root-oraculo-{pid}-0"),
            format!("root-entrega-esc-{pid}-1"),
            format!("root-entrega-eval-{pid}-2"),
            "root-oraculo-1-0".to_string(),
            format!("root-oraculo-1{pid}-0"),
            format!("root-builder-{pid}-0"),
        ];
        let contenedores: Vec<Contenedor> = nombres.iter().map(|n| contenedor(n, None)).collect();
        let (podman, log) = podman_falso(tmp.path(), &contenedores);
        let codigo = Cell::new(None);
        al_recibir_senal(podman.to_str().expect("ruta"), SIGTERM, |c| {
            codigo.set(Some(c))
        });
        assert_eq!(codigo.get(), Some(143), "sale con 143 al recibir SIGTERM");
        let anotado = fs::read_to_string(&log).expect("log");
        assert!(
            anotado.contains(&format!(
                "rm -f -t 0 root-oraculo-{pid}-0 root-entrega-esc-{pid}-1 root-entrega-eval-{pid}-2"
            )),
            "{anotado}"
        );
        assert!(!anotado.contains("root-oraculo-1-0"), "{anotado}");
        assert!(
            !anotado.contains(&format!("root-oraculo-1{pid}-0")),
            "{anotado}"
        );
        assert!(!anotado.contains("root-builder"), "{anotado}");
    }

    #[test]
    fn senal_sigint_sale_con_130() {
        let _serie = serie();
        let tmp = TempDir::new().expect("tmp");
        let (podman, _log) = podman_falso(tmp.path(), &[]);
        let codigo = Cell::new(None);
        al_recibir_senal(podman.to_str().expect("ruta"), SIGINT, |c| {
            codigo.set(Some(c))
        });
        assert_eq!(codigo.get(), Some(130), "sale con 130 al recibir SIGINT");
    }

    #[test]
    fn senal_con_programa_inexistente_sale_igual() {
        let _serie = serie();
        let codigo = Cell::new(None);
        al_recibir_senal("/no/existe/podman", SIGINT, |c| codigo.set(Some(c)));
        assert_eq!(
            codigo.get(),
            Some(130),
            "sale con 130 aunque no haya podman"
        );
    }

    #[test]
    fn panico_en_la_limpieza_sale_una_vez_con_el_codigo_de_la_senal() {
        let _serie = serie();
        let tmp = TempDir::new().expect("tmp");
        let (podman, _log) = podman_falso(tmp.path(), &[]);
        let salidas = RefCell::new(Vec::new());
        PANICO_DE_PRUEBA.store(true, Ordering::Relaxed);
        let resultado = catch_unwind(AssertUnwindSafe(|| {
            al_recibir_senal(podman.to_str().expect("ruta"), SIGINT, |c| {
                salidas.borrow_mut().push(c);
            });
        }));
        PANICO_DE_PRUEBA.store(false, Ordering::Relaxed);
        assert!(
            resultado.is_ok(),
            "el pánico no debe salir de al_recibir_senal"
        );
        assert_eq!(*salidas.borrow(), vec![130], "sale una vez, con 130");
    }

    #[test]
    fn barrido_quita_contenedor_de_pid_muerto_y_conserva_dueno_vivo() {
        let _serie = serie();
        let tmp = TempDir::new().expect("tmp");
        let vivo = 4242;
        let muerto = 5151;
        let proc_falso = proc_con_inicio(
            tmp.path(),
            vivo,
            b"/usr/bin/r00t\0oraculo\0evaluar\0",
            BTIME,
            0,
        );
        let contenedores = vec![
            contenedor(&format!("root-oraculo-{vivo}-0"), Some(BTIME + 100)),
            contenedor(&format!("root-oraculo-{muerto}-1"), Some(BTIME + 100)),
            contenedor(&format!("root-entrega-esc-{muerto}-2"), Some(BTIME + 100)),
            contenedor(&format!("root-builder-{muerto}-3"), Some(BTIME + 100)),
        ];
        let (podman, log) = podman_falso(tmp.path(), &contenedores);
        let quitados =
            barrer(podman.to_str().expect("ruta"), &proc_falso, PLAZO_BARRIDO).expect("barrido");
        assert_eq!(
            quitados,
            vec![
                format!("root-oraculo-{muerto}-1"),
                format!("root-entrega-esc-{muerto}-2")
            ],
            "quita solo los de pid muerto de su familia"
        );
        let anotado = fs::read_to_string(&log).expect("log");
        assert!(
            anotado.contains(&format!(
                "rm -f -t 0 root-oraculo-{muerto}-1 root-entrega-esc-{muerto}-2"
            )),
            "{anotado}"
        );
        assert!(
            !anotado.contains(&format!("root-oraculo-{vivo}-0")),
            "{anotado}"
        );
        assert!(!anotado.contains("root-builder"), "{anotado}");
    }

    #[test]
    fn pid_propio_cuenta_como_vivo_aunque_no_tenga_carpeta() {
        let tmp = TempDir::new().expect("tmp");
        let raiz = tmp.path().join("proc");
        fs::create_dir_all(&raiz).expect("proc");
        let contenedores = vec![contenedor(
            &format!("root-oraculo-{}-0", std::process::id()),
            Some(BTIME),
        )];
        assert!(
            huerfanos(&contenedores, &raiz).is_empty(),
            "el pid de este proceso se conserva"
        );
    }

    #[test]
    fn dueno_de_dice_no_existe_reusado_o_vivo() {
        let tmp = TempDir::new().expect("tmp");
        let vacio = tmp.path().join("vacio");
        fs::create_dir_all(&vacio).expect("proc");
        assert_eq!(dueno_de(4242, Some(BTIME), &vacio), Dueno::NoExiste);

        let reusado = proc_con_inicio(
            &tmp.path().join("reusado"),
            4242,
            b"/x/r00t\0",
            BTIME,
            40_000,
        );
        assert_eq!(
            dueno_de(4242, Some(BTIME), &reusado),
            Dueno::Reusado,
            "arrancó 400 s después de la referencia"
        );

        let vivo = proc_con_inicio(&tmp.path().join("vivo"), 4242, b"/x/r00t\0", BTIME, 0);
        assert_eq!(
            dueno_de(4242, Some(BTIME), &vivo),
            Dueno::Vivo,
            "arrancó antes de la referencia"
        );
    }

    #[test]
    fn dueno_vivo_llamado_root_hash_que_arranco_antes_se_conserva() {
        let tmp = TempDir::new().expect("tmp");
        let proc_falso = proc_con_inicio(tmp.path(), 4242, b"/x/root-abc123\0", BTIME, 0);
        let contenedores = vec![contenedor("root-oraculo-4242-0", Some(BTIME + 100))];
        assert!(
            huerfanos(&contenedores, &proc_falso).is_empty(),
            "el nombre del binario no cuenta: el dueño arrancó antes de la creación"
        );
    }

    #[test]
    fn dueno_que_arranco_400_s_despues_de_la_creacion_se_quita() {
        let tmp = TempDir::new().expect("tmp");
        let proc_falso = proc_con_inicio(tmp.path(), 4242, b"/x/r00t\0", BTIME, 40_000);
        let contenedores = vec![contenedor("root-oraculo-4242-0", Some(BTIME))];
        assert_eq!(
            huerfanos(&contenedores, &proc_falso),
            vec!["root-oraculo-4242-0".to_string()],
            "un pid reusado, que arrancó 400 s después, se quita"
        );
    }

    #[test]
    fn dueno_que_arranco_300_s_despues_se_conserva() {
        let tmp = TempDir::new().expect("tmp");
        let proc_falso = proc_con_inicio(tmp.path(), 4242, b"/x/r00t\0", BTIME, 30_000);
        let contenedores = vec![contenedor("root-oraculo-4242-0", Some(BTIME))];
        assert!(
            huerfanos(&contenedores, &proc_falso).is_empty(),
            "en el borde de 300 s el dueño se conserva"
        );
    }

    #[test]
    fn dueno_sin_carpeta_en_proc_se_quita() {
        let tmp = TempDir::new().expect("tmp");
        let raiz = tmp.path().join("proc");
        fs::create_dir_all(&raiz).expect("proc");
        let contenedores = vec![contenedor("root-oraculo-4242-0", Some(BTIME))];
        assert_eq!(
            huerfanos(&contenedores, &raiz),
            vec!["root-oraculo-4242-0".to_string()],
            "sin carpeta el dueño no existe"
        );
    }

    #[test]
    fn contenedor_sin_hora_de_creacion_se_conserva() {
        let tmp = TempDir::new().expect("tmp");
        let proc_falso = proc_con_inicio(tmp.path(), 4242, b"/x/root-abc123\0", BTIME, 40_000);
        let contenedores = vec![contenedor("root-oraculo-4242-0", None)];
        assert!(
            huerfanos(&contenedores, &proc_falso).is_empty(),
            "sin hora de creación el contenedor se conserva"
        );
    }

    #[test]
    fn dueno_sin_hora_de_arranque_se_conserva() {
        let tmp = TempDir::new().expect("tmp");
        let proc_falso = proc_con(tmp.path(), 4242, b"/x/root-abc123\0");
        let contenedores = vec![contenedor("root-oraculo-4242-0", Some(BTIME))];
        assert!(
            huerfanos(&contenedores, &proc_falso).is_empty(),
            "sin stat el dueño se conserva"
        );
    }

    #[test]
    fn nombre_de_la_familia_sin_pid_legible_cuenta_como_huerfano() {
        let tmp = TempDir::new().expect("tmp");
        let raiz = tmp.path().join("proc");
        fs::create_dir_all(&raiz).expect("proc");
        let contenedores = vec![contenedor("root-oraculo-x-0", Some(BTIME))];
        assert_eq!(
            huerfanos(&contenedores, &raiz),
            vec!["root-oraculo-x-0".to_string()],
            "sin pid legible el nombre cuenta como huérfano"
        );
    }

    #[test]
    fn parsear_listado_lee_nombre_y_epoch() {
        assert_eq!(
            parsear_listado("root-oraculo-1-0 1700000100\nroot-entrega-esc-1-1 1700000200\n"),
            vec![
                contenedor("root-oraculo-1-0", Some(1_700_000_100)),
                contenedor("root-entrega-esc-1-1", Some(1_700_000_200)),
            ]
        );
    }

    #[test]
    fn parsear_listado_epoch_ilegible_da_none() {
        assert_eq!(
            parsear_listado("root-oraculo-1-0 2026-11-13 10:00:00\n"),
            vec![contenedor("root-oraculo-1-0", None)],
            "un texto Y-M-D H:M:S no parsea: sin hora"
        );
    }

    #[test]
    fn parsear_listado_linea_con_solo_nombre_da_none() {
        assert_eq!(
            parsear_listado("root-oraculo-1-0\n"),
            vec![contenedor("root-oraculo-1-0", None)]
        );
    }

    #[test]
    fn parsear_listado_salta_lineas_vacias() {
        assert_eq!(
            parsear_listado("\n   \nroot-oraculo-1-0 5\n\n"),
            vec![contenedor("root-oraculo-1-0", Some(5))]
        );
    }

    #[test]
    fn listar_lee_la_hora_de_creacion_de_podman_real() {
        crate::saltar_sin_podman!();
        // Copia del nombre de la imagen de `oracle::calibra::defectos::IMAGEN_HERRAMIENTAS`, que
        // es `pub(super)` y no se alcanza desde aquí.
        const IMAGEN: &str = "localhost/root-oraculo-rust:2";
        struct Quitar(String);
        impl Drop for Quitar {
            fn drop(&mut self) {
                let _ = Command::new("podman").args(["rm", "-f", &self.0]).output();
            }
        }
        let epoch = || {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("reloj")
                .as_secs()
        };
        let nombre = format!("root-oraculo-{}-9999", std::process::id());
        let _guarda = Quitar(nombre.clone());
        let antes = epoch();
        let creo = Command::new("podman")
            .args(["create", "--name", &nombre, IMAGEN, "true"])
            .output()
            .expect("podman create");
        assert!(
            creo.status.success(),
            "{}",
            String::from_utf8_lossy(&creo.stderr)
        );
        let despues = epoch();
        let contenedores = listar("podman", Duration::from_secs(30)).expect("listar");
        let hallado = contenedores
            .iter()
            .find(|c| c.nombre == nombre)
            .expect("el contenedor aparece en el listado");
        let creado = hallado.creado.expect("hora de creación legible");
        assert!(
            creado >= antes.saturating_sub(5) && creado <= despues + 5,
            "creado {creado} fuera de [{antes}, {despues}]"
        );
    }

    #[test]
    fn nombres_de_pid_no_confunde_pids_con_el_mismo_inicio() {
        let nombres = vec![
            "root-oraculo-12-0".to_string(),
            "root-oraculo-123-0".to_string(),
            "root-entrega-eval-12-1".to_string(),
            "root-entrega-esc-12-2".to_string(),
            "root-builder-12-3".to_string(),
        ];
        assert_eq!(
            nombres_de_pid(&nombres, 12),
            vec![
                "root-oraculo-12-0".to_string(),
                "root-entrega-eval-12-1".to_string(),
                "root-entrega-esc-12-2".to_string(),
            ]
        );
    }

    #[test]
    fn vigia_se_suelta_sin_colgarse() {
        let vigia = Vigia::iniciar(PODMAN).expect("registra");
        drop(vigia);
    }

    #[test]
    fn barrido_con_sin_podman_uno_no_llama_al_programa() {
        let _serie = serie();
        let tmp = TempDir::new().expect("tmp");
        let (podman, log) = podman_falso(tmp.path(), &[]);
        barrer_al_arrancar_con(podman.to_str().expect("ruta"), Some("1"));
        assert!(!log.exists(), "con ROOT_SIN_PODMAN=1 no se llama a podman");
    }

    #[test]
    fn barrido_sin_variable_llama_al_programa() {
        let _serie = serie();
        let tmp = TempDir::new().expect("tmp");
        let (podman, log) = podman_falso(tmp.path(), &[]);
        barrer_al_arrancar_con(podman.to_str().expect("ruta"), None);
        let texto = fs::read_to_string(&log).expect("log del podman falso");
        assert!(
            texto.contains("ps"),
            "sin la variable, el barrido lista con ps"
        );
    }

    #[test]
    fn barrido_con_otros_valores_de_sin_podman_llama_al_programa() {
        let _serie = serie();
        for valor in ["0", "true", ""] {
            let tmp = TempDir::new().expect("tmp");
            let (podman, log) = podman_falso(tmp.path(), &[]);
            barrer_al_arrancar_con(podman.to_str().expect("ruta"), Some(valor));
            assert!(log.exists(), "con {valor:?} el barrido llama a podman");
        }
    }
}
