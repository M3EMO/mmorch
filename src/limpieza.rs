//! Limpieza de los contenedores de `oraculo evaluar`, `calibrar` y `auditar`.
//! Cada contenedor lleva en su nombre el pid del `r00t` que lo creó: `<prefijo><pid>-<n>`.
//! Al recibir SIGINT o SIGTERM, el proceso quita los suyos. Al arrancar un contenedor, quita los
//! de procesos que ya no son `r00t`. SIGKILL no se atrapa: el barrido cubre ese caso.
//!
//! Este módulo no toma el turno de podman. La señal puede llegar mientras el hilo principal tiene
//! el turno en un contenedor largo. Tomarlo dejaría la limpieza colgada.

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::process::Command;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use signal_hook::consts::{SIGINT, SIGTERM};
use signal_hook::iterator::{Handle, Signals};

use crate::Error;
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
/// Nombre del programa `r00t`. Su proceso es el único que puede dejar un contenedor vivo.
const NOMBRE_R00T: &str = "r00t";
/// Programa de podman en producción.
const PODMAN: &str = "podman";

/// Quita los contenedores de procesos muertos de la familia. Sin turno de podman: quien lo llama
/// debe tenerlo. Un fallo se avisa y no corta la corrida.
pub(crate) fn barrer_al_arrancar(programa: &str) {
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

/// Quita los contenedores cuyo pid no es un `r00t` vivo en `raiz_proc`. Devuelve sus nombres.
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

/// Los nombres de la familia de este código cuyo pid no es un `r00t` vivo en `raiz_proc`.
pub(crate) fn huerfanos(nombres: &[String], raiz_proc: &Path) -> Vec<String> {
    nombres
        .iter()
        .filter(|nombre| de_la_familia(nombre))
        .filter(|nombre| !pid_del_nombre(nombre).is_some_and(|pid| r00t_vivo(pid, raiz_proc)))
        .cloned()
        .collect()
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

/// Dice si `pid` es un proceso `r00t` vivo. Sigue la regla de `corre::barrido`: sin carpeta, cmdline
/// ilegible o cmdline que no es `r00t` cuentan como muerto.
fn r00t_vivo(pid: u32, raiz_proc: &Path) -> bool {
    fs::read(raiz_proc.join(pid.to_string()).join("cmdline")).is_ok_and(|cmdline| es_r00t(&cmdline))
}

/// Dice si el cmdline (argumentos separados por NUL) empieza con un programa `r00t`. Mira solo el
/// nombre base del primer argumento.
fn es_r00t(cmdline: &[u8]) -> bool {
    let primero = cmdline.split(|byte| *byte == 0).next().unwrap_or_default();
    Path::new(OsStr::from_bytes(primero))
        .file_name()
        .is_some_and(|nombre| nombre == NOMBRE_R00T)
}

/// Lista los contenedores que coinciden con `root-` en `programa ps`. Sin filtrar por familia.
fn listar(programa: &str, plazo: Duration) -> Result<Vec<String>, ErrorOraculo> {
    let salida = correr(
        programa,
        &[
            "ps",
            "-a",
            "--format",
            "{{.Names}}",
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
    Ok(salida
        .stdout
        .lines()
        .map(str::trim)
        .filter(|linea| !linea.is_empty())
        .map(str::to_string)
        .collect())
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
    let nombres = nombres_de_pid(&listar(programa, plazo)?, pid);
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

    /// `podman` falso: anota cada llamada en su log. Responde `ps` con `nombres`.
    fn podman_falso(base: &Path, nombres: &[String]) -> (PathBuf, PathBuf) {
        let log = base.join("podman.log");
        let lista: String = nombres.iter().map(|n| format!("echo {n}\n")).collect();
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

    #[test]
    fn senal_quita_los_contenedores_de_este_pid_y_no_otros() {
        let _serie = serie();
        let tmp = TempDir::new().expect("tmp");
        let pid = std::process::id();
        let nombres = vec![
            format!("root-oraculo-{pid}-0"),
            format!("root-entrega-esc-{pid}-1"),
            format!("root-entrega-eval-{pid}-2"),
            "root-oraculo-1-0".to_string(),
            format!("root-oraculo-1{pid}-0"),
            format!("root-builder-{pid}-0"),
        ];
        let (podman, log) = podman_falso(tmp.path(), &nombres);
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
    fn barrido_quita_contenedor_de_pid_muerto_y_conserva_r00t_vivo() {
        let _serie = serie();
        let tmp = TempDir::new().expect("tmp");
        let vivo = 4242;
        let muerto = 5151;
        let proc_falso = proc_con(tmp.path(), vivo, b"/usr/bin/r00t\0oraculo\0evaluar\0");
        let nombres = vec![
            format!("root-oraculo-{vivo}-0"),
            format!("root-oraculo-{muerto}-1"),
            format!("root-entrega-esc-{muerto}-2"),
            format!("root-builder-{muerto}-3"),
        ];
        let (podman, log) = podman_falso(tmp.path(), &nombres);
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
    fn pid_reusado_por_otro_programa_cuenta_como_muerto() {
        let tmp = TempDir::new().expect("tmp");
        let proc_falso = proc_con(tmp.path(), 777, b"/usr/bin/bash\0-l\0");
        let nombres = vec!["root-oraculo-777-0".to_string()];
        assert_eq!(
            huerfanos(&nombres, &proc_falso),
            vec!["root-oraculo-777-0".to_string()],
            "un cmdline que no es r00t cuenta como muerto"
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
}
