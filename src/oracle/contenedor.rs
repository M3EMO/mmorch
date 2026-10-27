//! Corre un comando en un contenedor de podman sin red y con un único directorio escribible.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde::Serialize;

use super::ErrorOraculo;

/// Caracteres de la cola de stdout y stderr que se guardan.
const COLA: usize = 4000;
/// Código con que podman sale ante un error propio (imagen inexistente, opción inválida).
const CODIGO_ERROR_PODMAN: i32 = 125;
/// Pausa entre consultas al proceso mientras se espera su fin.
const SONDEO: Duration = Duration::from_millis(20);

static CONTADOR: AtomicU64 = AtomicU64::new(0);

/// Tag de SELinux obligatorio en los montajes (Fedora).
pub const SUFIJO_SELINUX: &str = "z";

/// Qué correr, con qué montajes y con qué límite de tiempo.
#[derive(Debug, Clone)]
pub struct Ejecucion {
    pub imagen: String,
    pub repo: Option<PathBuf>,
    pub oraculo: Option<PathBuf>,
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
    if !e.red {
        args.extend(["--network", "none"].map(String::from));
    }
    args.extend(["--tmpfs", "/tmp:rw"].map(String::from));
    if let Some(repo) = &e.repo {
        montaje(&mut args, repo, "/repo", &format!("ro,{SUFIJO_SELINUX}"));
    }
    if let Some(oraculo) = &e.oraculo {
        montaje(
            &mut args,
            oraculo,
            "/oraculo",
            &format!("ro,{SUFIJO_SELINUX}"),
        );
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
    let nombre = nombre_unico();
    let inicio = Instant::now();
    let mut hijo = lanzar(e, &nombre)?;
    let salida = hijo.stdout.take().map(leer_en_hilo);
    let error = hijo.stderr.take().map(leer_en_hilo);
    let Some(estado) = esperar(&mut hijo, e.timeout)? else {
        terminar(&mut hijo, &nombre);
        return Err(ErrorOraculo::Infra(format!(
            "timeout tras {} s; contenedor {nombre} eliminado",
            e.timeout.as_secs_f64()
        )));
    };
    let stdout = recoger(salida);
    let stderr = recoger(error);
    let segundos = inicio.elapsed().as_secs_f64();
    let Some(codigo) = estado.code() else {
        return Err(ErrorOraculo::Infra("podman terminó por una señal".into()));
    };
    if codigo == CODIGO_ERROR_PODMAN {
        return Err(ErrorOraculo::Infra(format!(
            "podman falló (125): {}",
            cola(&stderr)
        )));
    }
    Ok(Resultado {
        codigo,
        stdout: cola(&stdout),
        stderr: cola(&stderr),
        segundos,
    })
}

/// Falla antes de lanzar podman si falta alguna ruta montada.
fn validar_rutas(e: &Ejecucion) -> Result<(), ErrorOraculo> {
    let rutas = e
        .repo
        .iter()
        .chain(e.oraculo.iter())
        .chain(e.cache.iter())
        .chain(e.rustsec.iter())
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
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| ErrorOraculo::Infra(format!("no se pudo lanzar podman: {err}")))
}

/// Lee un flujo completo en otro hilo para que el proceso no se bloquee al llenar el pipe.
fn leer_en_hilo<R: Read + Send + 'static>(mut flujo: R) -> JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = flujo.read_to_end(&mut bytes);
        bytes
    })
}

fn recoger(hilo: Option<JoinHandle<Vec<u8>>>) -> String {
    let bytes = hilo.and_then(|h| h.join().ok()).unwrap_or_default();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Espera al proceso hasta `limite`. Da `None` si venció el plazo.
fn esperar(hijo: &mut Child, limite: Duration) -> Result<Option<ExitStatus>, ErrorOraculo> {
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
    let _ = Command::new("podman")
        .args(["rm", "-f", "-t", "0", nombre])
        .output();
    let _ = hijo.kill();
    let _ = hijo.wait();
}

/// Últimos `COLA` caracteres de un texto.
fn cola(texto: &str) -> String {
    let total = texto.chars().count();
    texto.chars().skip(total.saturating_sub(COLA)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ejecucion() -> Ejecucion {
        Ejecucion {
            imagen: "docker.io/library/alpine:3.20".into(),
            repo: Some(PathBuf::from("/tmp/r")),
            oraculo: Some(PathBuf::from("/tmp/o")),
            salida: PathBuf::from("/tmp/s"),
            cmd: "pytest -q".into(),
            env: vec![("PYTHONPATH".into(), "/repo".into())],
            timeout: Duration::from_secs(60),
            red: false,
            cache: None,
            cache_rw: false,
            rustsec: None,
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
}
