//! `tools/colab/red-builder.sh` contra `ip`, `iptables`, `slirp4netns`, `getent` y `sleep` falsos.
//! Ningún test toca red ni corre `ip netns`, `iptables` o `slirp4netns` reales.
//! Los temporales van en `tempfile`, que usa el TMPDIR de test-seguro.
//! Corré con `tools/test-seguro.sh --test red_builder` (el filtro sin `--test` no casa nombres de función).

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

#[path = "comun/ejecutable.rs"]
mod ejecutable;
use ejecutable::escribir_ejecutable;

const SCRIPT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tools/colab/red-builder.sh");

const RED: &str = "red-prueba";

/// Reglas que el guion debe añadir, en orden: dos ACCEPT de DNS y luego seis DROP.
const REGLAS_ESPERADAS: [&str; 8] = [
    "-A OUTPUT -d 10.0.2.3 -p udp --dport 53 -j ACCEPT",
    "-A OUTPUT -d 10.0.2.3 -p tcp --dport 53 -j ACCEPT",
    "-A OUTPUT ! -o lo -d 10.0.0.0/8 -j DROP",
    "-A OUTPUT ! -o lo -d 172.16.0.0/12 -j DROP",
    "-A OUTPUT ! -o lo -d 192.168.0.0/16 -j DROP",
    "-A OUTPUT ! -o lo -d 100.64.0.0/10 -j DROP",
    "-A OUTPUT ! -o lo -d 169.254.0.0/16 -j DROP",
    "-A OUTPUT ! -o lo -d 127.0.0.0/8 -j DROP",
];

/// `ip` falso. Guarda los espacios de nombres en ESTADO. Ejecuta lo que sigue a `netns exec`.
const IP_FALSO: &str = r#"#!/bin/sh
echo "ip $*" >> 'REGISTRO'
if [ "$1" = netns ] && [ "$2" = add ]; then
  echo "$3" >> 'ESTADO'
  exit 0
fi
if [ "$1" = netns ] && [ "$2" = list ]; then
  if [ -f 'ESTADO' ]; then cat 'ESTADO'; fi
  exit 0
fi
if [ "$1" = netns ] && [ "$2" = exec ]; then
  shift 3
  exec "$@"
fi
exit 2
"#;

/// `iptables` falso. `-A` guarda la regla en REGLAS. `-C` sale 0 si la regla está, 1 si no.
const IPTABLES_FALSO: &str = r#"#!/bin/sh
echo "iptables $*" >> 'REGISTRO'
op="$1"
shift
if [ "$op" = -C ]; then
  grep -qxF -- "$*" 'REGLAS' 2>/dev/null && exit 0
  exit 1
fi
if [ "$op" = -A ]; then
  echo "$*" >> 'REGLAS'
  exit 0
fi
exit 2
"#;

/// `slirp4netns` falso. Anota la llamada y queda vivo como `sleep 30`, para que el guion guarde su pid.
const SLIRP_FALSO: &str = r#"#!/bin/sh
echo "slirp4netns $*" >> 'REGISTRO'
exec 'SLEEP_REAL' 30
"#;

/// `sleep` falso. Anota la llamada y después corre el `sleep` real.
const SLEEP_FALSO: &str = r#"#!/bin/sh
echo "sleep $*" >> 'REGISTRO'
exec 'SLEEP_REAL' "$@"
"#;

/// `getent` falso. Sale con error si existe FALLO. Si no, responde una línea.
const GETENT_FALSO: &str = r#"#!/bin/sh
echo "getent $*" >> 'REGISTRO'
if [ -f 'FALLO' ]; then exit 2; fi
echo '160.79.104.10 api.anthropic.com'
exit 0
"#;

/// Carpetas y archivos de un test. Los pidfiles están en la raíz temporal.
struct Escenario {
    falsos: PathBuf,
    registro: PathBuf,
    fallo: PathBuf,
    slirp_pid: PathBuf,
    sosten_pid: PathBuf,
    _raiz: tempfile::TempDir,
}

impl Drop for Escenario {
    /// Mata lo que el guion dejó en segundo plano, leyendo los pidfiles del test.
    fn drop(&mut self) {
        for pidfile in [&self.slirp_pid, &self.sosten_pid] {
            if let Ok(pid) = fs::read_to_string(pidfile) {
                let _ = Command::new("kill").arg(pid.trim()).status();
            }
        }
    }
}

fn bash_disponible() -> bool {
    Command::new("bash")
        .arg("-c")
        .arg("exit 0")
        .status()
        .is_ok_and(|s| s.success())
}

/// Ruta absoluta de un programa, buscada en el PATH de este proceso de test.
fn ruta_de(nombre: &str) -> PathBuf {
    std::env::var_os("PATH")
        .and_then(|path| {
            std::env::split_paths(&path)
                .map(|carpeta| carpeta.join(nombre))
                .find(|ruta| ruta.is_file())
        })
        .unwrap_or_else(|| panic!("falta {nombre} en el PATH"))
}

fn escenario() -> Escenario {
    let raiz = tempfile::tempdir().expect("raíz temporal");
    let falsos = raiz.path().join("falsos");
    fs::create_dir_all(&falsos).expect("crea falsos");
    let registro = raiz.path().join("registro.txt");
    let estado = raiz.path().join("netns.txt");
    let reglas = raiz.path().join("reglas.txt");
    let fallo = raiz.path().join("getent-falla");
    let sleep_real = ruta_de("sleep").display().to_string();
    let completar = |guion: &str| {
        guion
            .replace("REGISTRO", &registro.display().to_string())
            .replace("ESTADO", &estado.display().to_string())
            .replace("REGLAS", &reglas.display().to_string())
            .replace("FALLO", &fallo.display().to_string())
            .replace("SLEEP_REAL", &sleep_real)
    };
    escribir_ejecutable(&falsos.join("ip"), &completar(IP_FALSO));
    escribir_ejecutable(&falsos.join("iptables"), &completar(IPTABLES_FALSO));
    escribir_ejecutable(&falsos.join("slirp4netns"), &completar(SLIRP_FALSO));
    escribir_ejecutable(&falsos.join("sleep"), &completar(SLEEP_FALSO));
    escribir_ejecutable(&falsos.join("getent"), &completar(GETENT_FALSO));
    Escenario {
        falsos,
        registro,
        fallo,
        slirp_pid: raiz.path().join("slirp.pid"),
        sosten_pid: raiz.path().join("sosten.pid"),
        _raiz: raiz,
    }
}

/// Corre el guion con los falsos al inicio del PATH, el nombre de red y los pidfiles del test.
/// `extra` agrega variables al entorno.
fn correr(e: &Escenario, extra: &[(&str, &str)]) -> Output {
    let path = format!(
        "{}:{}",
        e.falsos.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut orden = Command::new("bash");
    orden
        .arg(SCRIPT)
        .env("PATH", path)
        .env("ROOT_RED_BUILDER", RED)
        .env("ROOT_SLIRP_PID", &e.slirp_pid)
        .env("ROOT_SOSTEN_PID", &e.sosten_pid)
        .env("ROOT_RED_INTENTOS", "1");
    for (clave, valor) in extra {
        orden.env(clave, valor);
    }
    orden.output().expect("lanza bash")
}

/// Una entrada por llamada de los falsos, en orden. Vacía si nunca se llamó a ninguno.
fn llamadas(e: &Escenario) -> Vec<String> {
    fs::read_to_string(&e.registro)
        .map(|texto| texto.lines().map(String::from).collect())
        .unwrap_or_default()
}

/// Reglas añadidas con `iptables -A`, sin el prefijo `iptables `, en el orden de la llamada.
fn reglas_añadidas(llamadas: &[String]) -> Vec<String> {
    llamadas
        .iter()
        .filter_map(|l| l.strip_prefix("iptables "))
        .filter(|r| r.starts_with("-A "))
        .map(String::from)
        .collect()
}

fn esperadas() -> Vec<String> {
    REGLAS_ESPERADAS.iter().map(|r| r.to_string()).collect()
}

fn lineas(salida: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(salida)
        .lines()
        .map(String::from)
        .collect()
}

#[test]
fn reglas_de_dns_van_antes_que_los_cortes() {
    if !bash_disponible() {
        eprintln!("sin bash: se saltea reglas_de_dns_van_antes_que_los_cortes");
        return;
    }
    let e = escenario();
    let salida = correr(&e, &[]);
    assert!(
        salida.status.success(),
        "debe salir 0: {}",
        String::from_utf8_lossy(&salida.stderr)
    );
    assert_eq!(
        reglas_añadidas(&llamadas(&e)),
        esperadas(),
        "reglas en orden"
    );
}

#[test]
fn segunda_corrida_no_repite_nada() {
    if !bash_disponible() {
        eprintln!("sin bash: se saltea segunda_corrida_no_repite_nada");
        return;
    }
    let e = escenario();
    let primera = correr(&e, &[]);
    assert!(
        primera.status.success(),
        "primera corrida debe salir 0: {}",
        String::from_utf8_lossy(&primera.stderr)
    );
    let segunda = correr(&e, &[]);
    assert!(
        segunda.status.success(),
        "segunda corrida debe salir 0: {}",
        String::from_utf8_lossy(&segunda.stderr)
    );
    let ls = llamadas(&e);
    let altas_de_red = ls.iter().filter(|l| l.starts_with("ip netns add")).count();
    assert_eq!(altas_de_red, 1, "ip netns add una sola vez: {ls:?}");
    let slirp = ls.iter().filter(|l| l.starts_with("slirp4netns ")).count();
    assert_eq!(slirp, 1, "slirp4netns una sola vez: {ls:?}");
    assert!(
        ls.iter().any(|l| l
            .starts_with("slirp4netns --configure --disable-host-loopback --mtu=65520 ")
            && l.ends_with(" tap0")),
        "faltan los argumentos de slirp4netns: {ls:?}"
    );
    assert_eq!(
        reglas_añadidas(&ls),
        esperadas(),
        "cada regla con -A una sola vez"
    );
}

#[test]
fn getent_responde_imprime_red_lista() {
    if !bash_disponible() {
        eprintln!("sin bash: se saltea getent_responde_imprime_red_lista");
        return;
    }
    let e = escenario();
    let salida = correr(&e, &[]);
    assert!(
        salida.status.success(),
        "debe salir 0: {}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let ls = lineas(&salida.stdout);
    assert_eq!(
        ls.last().map(String::as_str),
        Some("red lista"),
        "stdout: {ls:?}"
    );
}

#[test]
fn getent_falla_sale_con_1_sin_red_lista() {
    if !bash_disponible() {
        eprintln!("sin bash: se saltea getent_falla_sale_con_1_sin_red_lista");
        return;
    }
    let e = escenario();
    fs::write(&e.fallo, "").expect("marca el fallo de getent");
    let salida = correr(&e, &[]);
    assert_eq!(salida.status.code(), Some(1), "debe salir con 1");
    let ls = lineas(&salida.stdout);
    assert!(
        !ls.iter().any(|l| l == "red lista"),
        "no debe imprimir red lista: {ls:?}"
    );
    assert!(!salida.stderr.is_empty(), "debe avisar por stderr");
}

#[test]
fn ningun_secreto_llega_a_la_salida_ni_al_registro() {
    if !bash_disponible() {
        eprintln!("sin bash: se saltea ningun_secreto_llega_a_la_salida_ni_al_registro");
        return;
    }
    let e = escenario();
    let salida = correr(
        &e,
        &[
            ("ANTHROPIC_API_KEY", "sk-falso"),
            ("DEEPSEEK_API_KEY", "sk-falso"),
        ],
    );
    assert!(
        salida.status.success(),
        "debe salir 0: {}",
        String::from_utf8_lossy(&salida.stderr)
    );
    assert!(
        !String::from_utf8_lossy(&salida.stdout).contains("sk-falso"),
        "stdout con secreto"
    );
    assert!(
        !String::from_utf8_lossy(&salida.stderr).contains("sk-falso"),
        "stderr con secreto"
    );
    let registro = fs::read_to_string(&e.registro).unwrap_or_default();
    assert!(!registro.contains("sk-falso"), "el registro tiene secreto");
}
