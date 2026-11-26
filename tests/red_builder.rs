//! `tools/colab/red-builder.sh` contra `ip`, `unshare`, `nsenter`, `mknod`, `iptables`, `ip6tables`, `sysctl`, `slirp4netns`, `getent` y `sleep` falsos.
//! Ningún test toca red ni corre `ip netns`, `iptables` o `slirp4netns` reales.
//! Los temporales van en `tempfile`, que usa el TMPDIR de test-seguro.
//! Corré con `tools/test-seguro.sh --test red_builder` (el filtro sin `--test` no casa nombres de función).

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::thread;
use std::time::Duration;

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

/// `ip` falso. `netns attach NOMBRE` crea $ROOT_NETNS_DIR/NOMBRE; `netns delete NOMBRE` lo borra.
/// `netns exec` ejecuta lo que sigue. `netns add` solo se anota: el guion nuevo no lo debe usar.
/// `-6 route show default` y `-6 addr show scope global` salen vacíos, salvo que exista la marca
/// de IPv6 activo: entonces imprimen una ruta y una dirección global falsas.
const IP_FALSO: &str = r#"#!/bin/sh
echo "ip $*" >> 'REGISTRO'
if [ "$1" = netns ] && [ "$2" = add ]; then
  exit 0
fi
if [ "$1" = netns ] && [ "$2" = list ]; then
  exit 0
fi
if [ "$1" = netns ] && [ "$2" = attach ]; then
  : > "$ROOT_NETNS_DIR/$3"
  exit 0
fi
if [ "$1" = netns ] && [ "$2" = delete ]; then
  rm -f "$ROOT_NETNS_DIR/$3"
  exit 0
fi
if [ "$1" = netns ] && [ "$2" = exec ]; then
  shift 3
  exec "$@"
fi
if [ "$1" = -6 ] && [ "$2" = route ]; then
  if [ -f 'IPV6' ]; then echo "default via fe80::1 dev tap0 proto ra"; fi
  exit 0
fi
if [ "$1" = -6 ] && [ "$2" = addr ]; then
  if [ -f 'IPV6' ]; then echo "inet6 2001:db8::2/64 scope global"; fi
  exit 0
fi
exit 2
"#;

/// `unshare` falso. Anota la llamada y queda vivo como `sleep 30`, para que el guion guarde su pid.
const UNSHARE_FALSO: &str = r#"#!/bin/sh
echo "unshare $*" >> 'REGISTRO'
exec 'SLEEP_REAL' 30
"#;

/// `nsenter` falso. Anota la llamada, saltea `-t PID -U -n` (4 argumentos) y ejecuta lo que sigue,
/// así `iptables`, `ip6tables` e `ip` falsos anotan su propia línea.
const NSENTER_FALSO: &str = r#"#!/bin/sh
echo "nsenter $*" >> 'REGISTRO'
shift 4
exec "$@"
"#;

/// `mknod` falso. Anota la llamada, crea un archivo vacío en la ruta (`$3`) y sale 0.
/// Así una segunda corrida ve el nodo y no vuelve a llamar a mknod.
const MKNOD_FALSO: &str = r#"#!/bin/sh
echo "mknod $*" >> 'REGISTRO'
: > "$3"
exit 0
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

/// `sysctl` falso. Anota la llamada y sale 0.
const SYSCTL_FALSO: &str = r#"#!/bin/sh
echo "sysctl $*" >> 'REGISTRO'
exit 0
"#;

/// `ip6tables` falso. Anota la llamada y sale 0.
const IP6TABLES_FALSO: &str = r#"#!/bin/sh
echo "ip6tables $*" >> 'REGISTRO'
exit 0
"#;

/// `slirp4netns` falso. Anota la llamada, escribe una línea de log y queda vivo como `sleep 30`,
/// para que el guion guarde su pid.
const SLIRP_FALSO: &str = r#"#!/bin/sh
echo "slirp4netns $*" >> 'REGISTRO'
echo log-falso
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
    /// Enlaces a los programas reales que usa el guion (`cat`, `grep`...), para el PATH sin ip6tables.
    reales: PathBuf,
    registro: PathBuf,
    fallo: PathBuf,
    slirp_pid: PathBuf,
    sosten_pid: PathBuf,
    slirp_log: PathBuf,
    /// Carpeta de los archivos de nombre de red (`ROOT_NETNS_DIR`).
    netns_dir: PathBuf,
    /// Nodo de tun del test (`ROOT_DEV_NET_TUN`), dentro de la raíz temporal.
    tun: PathBuf,
    /// Carpeta `etc` del test (`ROOT_ETC_DIR`), dentro de la raíz temporal. Nunca es el /etc real.
    etc: PathBuf,
    /// Marca de IPv6 activo: si existe, el `ip` falso imprime ruta y dirección global.
    ipv6: PathBuf,
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

/// Programas reales que el guion llama y que no son falsos. Van enlazados en la carpeta `reales`.
const PROGRAMAS_REALES: [&str; 8] = [
    "cat", "cut", "grep", "dirname", "mkdir", "touch", "chmod", "rm",
];

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
    let reales = raiz.path().join("reales");
    fs::create_dir_all(&reales).expect("crea reales");
    for programa in PROGRAMAS_REALES {
        std::os::unix::fs::symlink(ruta_de(programa), reales.join(programa))
            .expect("enlaza el programa real");
    }
    let netns_dir = raiz.path().join("netns");
    fs::create_dir_all(&netns_dir).expect("crea la carpeta de nombres de red");
    let registro = raiz.path().join("registro.txt");
    let reglas = raiz.path().join("reglas.txt");
    let fallo = raiz.path().join("getent-falla");
    let sleep_real = ruta_de("sleep").display().to_string();
    let ipv6 = raiz.path().join("ipv6-activo");
    let completar = |guion: &str| {
        guion
            .replace("REGISTRO", &registro.display().to_string())
            .replace("REGLAS", &reglas.display().to_string())
            .replace("FALLO", &fallo.display().to_string())
            .replace("IPV6", &ipv6.display().to_string())
            .replace("SLEEP_REAL", &sleep_real)
    };
    escribir_ejecutable(&falsos.join("ip"), &completar(IP_FALSO));
    escribir_ejecutable(&falsos.join("unshare"), &completar(UNSHARE_FALSO));
    escribir_ejecutable(&falsos.join("nsenter"), &completar(NSENTER_FALSO));
    escribir_ejecutable(&falsos.join("mknod"), &completar(MKNOD_FALSO));
    escribir_ejecutable(&falsos.join("iptables"), &completar(IPTABLES_FALSO));
    escribir_ejecutable(&falsos.join("slirp4netns"), &completar(SLIRP_FALSO));
    escribir_ejecutable(&falsos.join("sleep"), &completar(SLEEP_FALSO));
    escribir_ejecutable(&falsos.join("getent"), &completar(GETENT_FALSO));
    escribir_ejecutable(&falsos.join("sysctl"), &completar(SYSCTL_FALSO));
    escribir_ejecutable(&falsos.join("ip6tables"), &completar(IP6TABLES_FALSO));
    Escenario {
        falsos,
        reales,
        registro,
        fallo,
        slirp_pid: raiz.path().join("slirp.pid"),
        sosten_pid: raiz.path().join("sosten.pid"),
        slirp_log: raiz.path().join("slirp.log"),
        netns_dir,
        tun: raiz.path().join("dev").join("net").join("tun"),
        etc: raiz.path().join("etc"),
        ipv6,
        _raiz: raiz,
    }
}

/// PATH del test: los falsos primero, después el PATH real. Con `aislado`, en vez del PATH
/// real va una carpeta propia con enlaces a los programas reales del guion: así el guion no encuentra
/// `ip6tables` ni `nsenter` aunque vivan en la misma carpeta que `bash`, `cat` o `grep` (en Fedora, `/usr/bin`).
fn path_del_test(e: &Escenario, aislado: bool) -> String {
    let resto = if aislado {
        e.reales.clone().into_os_string()
    } else {
        std::env::var_os("PATH").unwrap_or_default()
    };
    std::env::join_paths(std::iter::once(e.falsos.clone()).chain(std::env::split_paths(&resto)))
        .expect("PATH válido")
        .to_string_lossy()
        .into_owned()
}

/// Corre el guion con el PATH dado, el nombre de red, los pidfiles y el log de slirp del test.
/// `extra` agrega variables al entorno.
fn correr_con_path(e: &Escenario, path: &str, extra: &[(&str, &str)]) -> Output {
    // bash por ruta absoluta, resuelta con el PATH real de este proceso: el PATH del guion puede no tenerlo.
    let mut orden = Command::new(ruta_de("bash"));
    orden
        .arg(SCRIPT)
        .env("PATH", path)
        .env("ROOT_RED_BUILDER", RED)
        .env("ROOT_SLIRP_PID", &e.slirp_pid)
        .env("ROOT_SOSTEN_PID", &e.sosten_pid)
        .env("ROOT_SLIRP_LOG", &e.slirp_log)
        .env("ROOT_NETNS_DIR", &e.netns_dir)
        .env("ROOT_DEV_NET_TUN", &e.tun)
        .env("ROOT_ETC_DIR", &e.etc)
        .env("ROOT_RED_INTENTOS", "1");
    for (clave, valor) in extra {
        orden.env(clave, valor);
    }
    orden.output().expect("lanza bash")
}

/// Corre el guion con los falsos al inicio del PATH.
fn correr(e: &Escenario, extra: &[(&str, &str)]) -> Output {
    correr_con_path(e, &path_del_test(e, false), extra)
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
    let altas_de_red = ls
        .iter()
        .filter(|l| l.starts_with("ip netns attach"))
        .count();
    assert_eq!(altas_de_red, 1, "ip netns attach una sola vez: {ls:?}");
    let slirp = ls.iter().filter(|l| l.starts_with("slirp4netns ")).count();
    assert_eq!(slirp, 1, "slirp4netns una sola vez: {ls:?}");
    assert!(
        ls.iter().any(|l| l.starts_with(
            "slirp4netns --configure --disable-host-loopback --mtu=65520 --userns-path="
        ) && l.ends_with(" tap0")),
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

/// PID del sostén, tal como lo escribió el guion en su pidfile.
fn pid_del_sosten(e: &Escenario) -> String {
    fs::read_to_string(&e.sosten_pid)
        .expect("el guion escribe sosten.pid")
        .trim()
        .to_string()
}

#[test]
fn ninguna_llamada_a_sysctl() {
    if !bash_disponible() {
        eprintln!("sin bash: se saltea ninguna_llamada_a_sysctl");
        return;
    }
    let e = escenario();
    let salida = correr(&e, &[]);
    assert!(
        salida.status.success(),
        "debe salir 0: {}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let ls = llamadas(&e);
    assert!(
        !ls.iter().any(|l| l.starts_with("sysctl")),
        "no debe llamar a sysctl: {ls:?}"
    );
}

/// Toda llamada `iptables ...` o `ip6tables ...` debe ir justo detrás de `nsenter -t {pid} -U -n` con esa misma llamada.
fn revisar_adyacencia_nsenter(ls: &[String], pid: &str) -> Result<(), String> {
    // El falso de slirp4netns corre en segundo plano: su línea puede caer en cualquier parte.
    let sin_slirp: Vec<String> = ls
        .iter()
        .filter(|l| !l.starts_with("slirp4netns "))
        .cloned()
        .collect();
    for (i, llamada) in sin_slirp.iter().enumerate() {
        if !(llamada.starts_with("iptables ") || llamada.starts_with("ip6tables ")) {
            continue;
        }
        let previa = i.checked_sub(1).map(|j| sin_slirp[j].as_str());
        let esperada = format!("nsenter -t {pid} -U -n {llamada}");
        if previa != Some(esperada.as_str()) {
            return Err(format!(
                "`{llamada}` debe ir justo detrás de nsenter: {ls:?}"
            ));
        }
    }
    Ok(())
}

#[test]
fn adyacencia_ignora_slirp_en_segundo_plano() {
    let ls: Vec<String> = [
        "nsenter -t 42 -U -n iptables -C OUTPUT -d 10.0.2.3 -j ACCEPT",
        "slirp4netns --configure --disable-host-loopback --mtu=65520 --userns-path=/proc/42/ns/user tap0",
        "iptables -C OUTPUT -d 10.0.2.3 -j ACCEPT",
    ]
    .iter()
    .map(|l| l.to_string())
    .collect();
    assert!(
        revisar_adyacencia_nsenter(&ls, "42").is_ok(),
        "slirp en segundo plano no debe romper la adyacencia: {ls:?}"
    );
}

#[test]
fn adyacencia_exige_nsenter_justo_antes() {
    let ls: Vec<String> = ["iptables -C OUTPUT -d 10.0.2.3 -j ACCEPT"]
        .iter()
        .map(|l| l.to_string())
        .collect();
    assert!(
        revisar_adyacencia_nsenter(&ls, "42").is_err(),
        "sin nsenter antes debe fallar: {ls:?}"
    );
}

#[test]
fn toda_regla_va_detras_de_nsenter() {
    if !bash_disponible() {
        eprintln!("sin bash: se saltea toda_regla_va_detras_de_nsenter");
        return;
    }
    let e = escenario();
    let salida = correr(&e, &[]);
    assert!(
        salida.status.success(),
        "debe salir 0: {}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let pid = pid_del_sosten(&e);
    let ls = llamadas(&e);
    assert!(
        ls.iter()
            .any(|l| l.starts_with("iptables ") || l.starts_with("ip6tables ")),
        "debe haber llamadas a iptables o ip6tables: {ls:?}"
    );
    if let Err(motivo) = revisar_adyacencia_nsenter(&ls, &pid) {
        panic!("{motivo}");
    }
    for prohibida in [
        format!("ip netns exec {RED} iptables"),
        format!("ip netns exec {RED} ip6tables"),
    ] {
        assert!(
            !ls.iter().any(|l| l.starts_with(prohibida.as_str())),
            "no debe llamar a `{prohibida}`: {ls:?}"
        );
    }
}

#[test]
fn politicas_ip6tables_en_drop() {
    if !bash_disponible() {
        eprintln!("sin bash: se saltea politicas_ip6tables_en_drop");
        return;
    }
    let e = escenario();
    let salida = correr(&e, &[]);
    assert!(
        salida.status.success(),
        "debe salir 0: {}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let ls = llamadas(&e);
    for cadena in ["OUTPUT", "INPUT", "FORWARD"] {
        let esperada = format!("ip6tables -P {cadena} DROP");
        assert!(ls.contains(&esperada), "falta `{esperada}`: {ls:?}");
    }
}

#[test]
fn ruta_ipv6_frena_el_guion() {
    if !bash_disponible() {
        eprintln!("sin bash: se saltea ruta_ipv6_frena_el_guion");
        return;
    }
    let e = escenario();
    fs::write(&e.ipv6, "").expect("marca IPv6 activo");
    let salida = correr(&e, &[]);
    assert_eq!(salida.status.code(), Some(1), "debe salir con 1");
    let error = String::from_utf8_lossy(&salida.stderr);
    assert!(error.contains("IPv6"), "stderr debe nombrar IPv6: {error}");
    let ls = lineas(&salida.stdout);
    assert!(
        !ls.iter().any(|l| l == "red lista"),
        "no debe imprimir red lista: {ls:?}"
    );
}

#[test]
fn sin_ip6tables_frena_antes_de_crear_el_espacio() {
    if !bash_disponible() {
        eprintln!("sin bash: se saltea sin_ip6tables_frena_antes_de_crear_el_espacio");
        return;
    }
    let e = escenario();
    fs::remove_file(e.falsos.join("ip6tables")).expect("quita el ip6tables falso");
    let salida = correr_con_path(&e, &path_del_test(&e, true), &[]);
    assert_eq!(salida.status.code(), Some(1), "debe salir con 1");
    let error = String::from_utf8_lossy(&salida.stderr);
    assert!(
        error.contains("ip6tables"),
        "stderr debe nombrar ip6tables: {error}"
    );
    let ls = llamadas(&e);
    assert!(
        !ls.iter().any(|l| l.starts_with("ip netns")),
        "no debe crear el espacio: {ls:?}"
    );
    assert!(ls.is_empty(), "no debe llamar a ningún falso: {ls:?}");
    assert!(!e.slirp_log.exists(), "no debe crear el log de slirp");
}

#[test]
fn log_de_slirp_va_al_archivo_con_modo_0600() {
    if !bash_disponible() {
        eprintln!("sin bash: se saltea log_de_slirp_va_al_archivo_con_modo_0600");
        return;
    }
    let e = escenario();
    let salida = correr(&e, &[]);
    assert!(
        salida.status.success(),
        "debe salir 0: {}",
        String::from_utf8_lossy(&salida.stderr)
    );
    // slirp corre en segundo plano: se espera un poco a que escriba su línea.
    let mut contenido = String::new();
    for _ in 0..30 {
        contenido = fs::read_to_string(&e.slirp_log).unwrap_or_default();
        if contenido.lines().any(|l| l == "log-falso") {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    assert!(
        contenido.lines().any(|l| l == "log-falso"),
        "el log de slirp en {} debe tener la línea del falso: {contenido:?}",
        e.slirp_log.display()
    );
    let modo = fs::metadata(&e.slirp_log)
        .expect("metadatos del log de slirp")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(modo, 0o600, "el log de slirp debe ser 0600");
}

#[test]
fn unshare_y_attach() {
    if !bash_disponible() {
        eprintln!("sin bash: se saltea unshare_y_attach");
        return;
    }
    let e = escenario();
    let salida = correr(&e, &[]);
    assert!(
        salida.status.success(),
        "debe salir 0: {}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let ls = llamadas(&e);
    let pid = fs::read_to_string(&e.sosten_pid).expect("el guion escribe sosten.pid");
    let attach = format!("ip netns attach {RED} {}", pid.trim());
    let pos_unshare = ls
        .iter()
        .position(|l| l == "unshare -Urn sleep infinity")
        .unwrap_or_else(|| panic!("falta `unshare -Urn sleep infinity`: {ls:?}"));
    let pos_attach = ls
        .iter()
        .position(|l| *l == attach)
        .unwrap_or_else(|| panic!("falta `{attach}`: {ls:?}"));
    assert!(pos_unshare < pos_attach, "unshare antes de attach: {ls:?}");
    assert!(
        !ls.iter().any(|l| l.starts_with("ip netns add")),
        "no debe llamar a ip netns add: {ls:?}"
    );
}

#[test]
fn con_sosten_vivo_y_nombre_puesto_no_crea_nada() {
    if !bash_disponible() {
        eprintln!("sin bash: se saltea con_sosten_vivo_y_nombre_puesto_no_crea_nada");
        return;
    }
    let e = escenario();
    fs::write(e.netns_dir.join(RED), "").expect("pone el archivo de nombre");
    let mut vivo = Command::new(ruta_de("sleep"))
        .arg("30")
        .spawn()
        .expect("sostén vivo");
    fs::write(&e.sosten_pid, vivo.id().to_string()).expect("escribe sosten.pid");
    let salida = correr(&e, &[]);
    let _ = vivo.kill();
    let _ = vivo.wait();
    assert!(
        salida.status.success(),
        "debe salir 0: {}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let ls = llamadas(&e);
    for prohibida in [
        "unshare",
        "ip netns add",
        "ip netns attach",
        "ip netns delete",
    ] {
        assert!(
            !ls.iter().any(|l| l.starts_with(prohibida)),
            "no debe llamar a `{prohibida}`: {ls:?}"
        );
    }
}

#[test]
fn con_nombre_puesto_y_sosten_muerto_recrea() {
    if !bash_disponible() {
        eprintln!("sin bash: se saltea con_nombre_puesto_y_sosten_muerto_recrea");
        return;
    }
    let e = escenario();
    fs::write(e.netns_dir.join(RED), "").expect("pone el archivo de nombre");
    // Un proceso que ya terminó: su pid queda muerto.
    let mut muerto = Command::new(ruta_de("true"))
        .spawn()
        .expect("proceso muerto");
    muerto.wait().expect("espera al proceso muerto");
    fs::write(&e.sosten_pid, muerto.id().to_string()).expect("escribe sosten.pid");
    let salida = correr(&e, &[]);
    assert!(
        salida.status.success(),
        "debe salir 0: {}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let ls = llamadas(&e);
    let pos = |prefijo: &str| {
        ls.iter()
            .position(|l| l.starts_with(prefijo))
            .unwrap_or_else(|| panic!("falta `{prefijo}`: {ls:?}"))
    };
    let pos_borrar = pos("ip netns delete red-prueba");
    let pos_unshare = pos("unshare ");
    let pos_attach = pos("ip netns attach red-prueba ");
    assert!(
        pos_borrar < pos_unshare && pos_unshare < pos_attach,
        "orden: delete, unshare, attach: {ls:?}"
    );
}

#[test]
fn sin_tun_lo_crea_con_c_10_200() {
    if !bash_disponible() {
        eprintln!("sin bash: se saltea sin_tun_lo_crea_con_c_10_200");
        return;
    }
    let e = escenario();
    assert!(!e.tun.exists(), "el nodo tun no debe existir al empezar");
    let salida = correr(&e, &[]);
    assert!(
        salida.status.success(),
        "debe salir 0: {}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let ls = llamadas(&e);
    let esperada = format!("mknod -m 0666 {} c 10 200", e.tun.display());
    assert!(ls.contains(&esperada), "falta `{esperada}`: {ls:?}");
}

#[test]
fn con_tun_no_llama_a_mknod() {
    if !bash_disponible() {
        eprintln!("sin bash: se saltea con_tun_no_llama_a_mknod");
        return;
    }
    let e = escenario();
    fs::create_dir_all(e.tun.parent().expect("carpeta de tun")).expect("crea dev/net");
    fs::write(&e.tun, "").expect("pre-crea el nodo tun");
    let salida = correr(&e, &[]);
    assert!(
        salida.status.success(),
        "debe salir 0: {}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let ls = llamadas(&e);
    assert!(
        !ls.iter().any(|l| l.starts_with("mknod ")),
        "con tun existente no debe llamar a mknod: {ls:?}"
    );
}

#[test]
fn slirp_recibe_userns_path() {
    if !bash_disponible() {
        eprintln!("sin bash: se saltea slirp_recibe_userns_path");
        return;
    }
    let e = escenario();
    let salida = correr(&e, &[]);
    assert!(
        salida.status.success(),
        "debe salir 0: {}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let pid = fs::read_to_string(&e.sosten_pid).expect("el guion escribe sosten.pid");
    let pid = pid.trim();
    let userns = format!("--userns-path=/proc/{pid}/ns/user");
    let ls = llamadas(&e);
    assert!(
        ls.iter()
            .any(|l| l.starts_with("slirp4netns ") && l.contains(&userns)),
        "falta `{userns}` en la línea de slirp4netns: {ls:?}"
    );
}

#[test]
fn sin_nsenter_frena_y_lo_nombra() {
    if !bash_disponible() {
        eprintln!("sin bash: se saltea sin_nsenter_frena_y_lo_nombra");
        return;
    }
    let e = escenario();
    fs::remove_file(e.falsos.join("nsenter")).expect("quita el nsenter falso");
    let salida = correr_con_path(&e, &path_del_test(&e, true), &[]);
    assert_eq!(salida.status.code(), Some(1), "debe salir con 1");
    let error = String::from_utf8_lossy(&salida.stderr);
    assert!(
        error.contains("nsenter"),
        "stderr debe nombrar nsenter: {error}"
    );
    let ls = llamadas(&e);
    assert!(ls.is_empty(), "no debe llamar a ningún falso: {ls:?}");
}

#[test]
fn escribe_resolv_del_builder() {
    if !bash_disponible() {
        eprintln!("sin bash: se saltea escribe_resolv_del_builder");
        return;
    }
    let e = escenario();
    let salida = correr(&e, &[]);
    assert!(
        salida.status.success(),
        "debe salir 0: {}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let resolv = e.etc.join("netns").join(RED).join("resolv.conf");
    let contenido = fs::read_to_string(&resolv)
        .unwrap_or_else(|error| panic!("falta {}: {error}", resolv.display()));
    assert_eq!(
        contenido, "nameserver 10.0.2.3\n",
        "contenido de resolv.conf"
    );
    let modo = fs::metadata(&resolv)
        .expect("metadatos de resolv.conf")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(modo, 0o644, "resolv.conf del builder debe ser 0644");
}

#[test]
fn no_toca_el_resolv_del_runtime() {
    if !bash_disponible() {
        eprintln!("sin bash: se saltea no_toca_el_resolv_del_runtime");
        return;
    }
    let e = escenario();
    fs::create_dir_all(&e.etc).expect("crea la carpeta etc del test");
    let runtime = e.etc.join("resolv.conf");
    fs::write(&runtime, "nameserver 127.0.0.11\n").expect("pone el resolv.conf del runtime");
    let salida = correr(&e, &[]);
    assert!(
        salida.status.success(),
        "debe salir 0: {}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let contenido = fs::read_to_string(&runtime).expect("lee el resolv.conf del runtime");
    assert_eq!(
        contenido, "nameserver 127.0.0.11\n",
        "el resolv.conf del runtime no debe cambiar"
    );
}

#[test]
fn comprobacion_usa_ip_netns_exec_getent() {
    if !bash_disponible() {
        eprintln!("sin bash: se saltea comprobacion_usa_ip_netns_exec_getent");
        return;
    }
    let e = escenario();
    let salida = correr(&e, &[]);
    assert!(
        salida.status.success(),
        "debe salir 0: {}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let ls = llamadas(&e);
    let esperada = format!("ip netns exec {RED} getent hosts api.anthropic.com");
    assert!(ls.contains(&esperada), "falta `{esperada}`: {ls:?}");
}

#[test]
fn getent_reintenta_antes_de_rendirse() {
    if !bash_disponible() {
        eprintln!("sin bash: se saltea getent_reintenta_antes_de_rendirse");
        return;
    }
    let e = escenario();
    fs::write(&e.fallo, "").expect("marca el fallo de getent");
    let salida = correr(&e, &[("ROOT_RED_INTENTOS", "3")]);
    assert_eq!(salida.status.code(), Some(1), "debe salir con 1");
    let ls = llamadas(&e);
    let intentos = ls
        .iter()
        .filter(|l| l.as_str() == "getent hosts api.anthropic.com")
        .count();
    assert_eq!(intentos, 3, "3 intentos de getent: {ls:?}");
}

#[test]
fn la_comprobacion_corre_al_final() {
    if !bash_disponible() {
        eprintln!("sin bash: se saltea la_comprobacion_corre_al_final");
        return;
    }
    let e = escenario();
    let salida = correr(&e, &[]);
    assert!(
        salida.status.success(),
        "debe salir 0: {}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let ls = llamadas(&e);
    let getent = ls
        .iter()
        .rposition(|l| l == "getent hosts api.anthropic.com")
        .unwrap_or_else(|| panic!("falta la llamada a getent: {ls:?}"));
    let ultima_regla = ls
        .iter()
        .rposition(|l| l.starts_with("iptables -A "))
        .unwrap_or_else(|| panic!("falta iptables -A: {ls:?}"));
    assert!(
        getent > ultima_regla,
        "getent debe ir después de la última iptables -A: {ls:?}"
    );
    let ruta_v6 = ls
        .iter()
        .rposition(|l| l.starts_with("nsenter ") && l.ends_with(" ip -6 route show default"))
        .unwrap_or_else(|| panic!("falta nsenter ... ip -6 route show default: {ls:?}"));
    assert!(
        getent > ruta_v6,
        "getent debe ir después de nsenter ... ip -6 route show default: {ls:?}"
    );
}
