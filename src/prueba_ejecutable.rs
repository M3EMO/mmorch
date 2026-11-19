//! Helper de tests: escribe un ejecutable y espera a que se pueda lanzar.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use tempfile::TempDir;

use crate::corre::ejecutor;

/// Marca que el script lee para salir antes de hacer nada, al probarse.
const SENTINELA: &[u8] = b"[ \"$ROOT_SONDA_EJECUTABLE\" = 1 ] && exit 0\n";

/// Escribe un script o un binario copiado. Ver `escribir_ejecutable_bytes`.
pub(crate) fn escribir_ejecutable(ruta: &Path, contenido: &str) -> PathBuf {
    escribir_ejecutable_bytes(ruta, contenido.as_bytes())
}

/// Escribe un ejecutable con permisos 0o755 y lo prueba antes de devolverlo.
///
/// Si el contenido empieza con `#!/bin/sh`, inserta la sentinela tras la primera línea.
/// La sonda sale con 0 y no ejecuta el resto del script. Un binario copiado no lleva sentinela.
pub(crate) fn escribir_ejecutable_bytes(ruta: &Path, contenido: &[u8]) -> PathBuf {
    let script = es_script_sh(contenido);
    let cuerpo = if script {
        con_sentinela(contenido)
    } else {
        contenido.to_vec()
    };
    fs::write(ruta, &cuerpo).expect("escribe el ejecutable");
    fs::set_permissions(ruta, fs::Permissions::from_mode(0o755)).expect("permisos");
    sondear(ruta, script);
    ruta.to_path_buf()
}

/// Dice si la primera línea es exactamente `#!/bin/sh`.
fn es_script_sh(contenido: &[u8]) -> bool {
    let primera = contenido.split(|&b| b == b'\n').next().unwrap_or(&[]);
    primera == b"#!/bin/sh"
}

/// Copia el contenido y pone la sentinela justo después de la primera línea.
fn con_sentinela(contenido: &[u8]) -> Vec<u8> {
    let mut salida = Vec::with_capacity(contenido.len() + SENTINELA.len() + 1);
    match contenido.iter().position(|&b| b == b'\n') {
        Some(fin) => {
            salida.extend_from_slice(&contenido[..=fin]);
            salida.extend_from_slice(SENTINELA);
            salida.extend_from_slice(&contenido[fin + 1..]);
        }
        None => {
            // Solo hay cabecera, sin salto de línea. Se agrega uno antes de la sentinela.
            salida.extend_from_slice(contenido);
            salida.push(b'\n');
            salida.extend_from_slice(SENTINELA);
        }
    }
    salida
}

/// Lanza el ejecutable con `ROOT_SONDA_EJECUTABLE=1` y sin argumentos.
///
/// Linux da ETXTBSY (errno 26) si un archivo ejecutable tiene un descriptor de escritura
/// abierto. Un fork concurrente hereda ese descriptor hasta su `exec`. Por eso la sonda
/// espera 10 ms y reintenta, hasta 50 intentos. Un script debe salir con 0. Un binario
/// copiado solo necesita que el `exec` no dé 26. Su código de salida no importa.
fn sondear(ruta: &Path, script: bool) {
    const ETXTBSY: i32 = 26;
    for _ in 0..50 {
        match Command::new(ruta)
            .env("ROOT_SONDA_EJECUTABLE", "1")
            .status()
        {
            Err(e) if e.raw_os_error() == Some(ETXTBSY) => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(e) => panic!("no se pudo lanzar la sonda de {}: {e}", ruta.display()),
            Ok(estado) => {
                if script {
                    assert!(
                        estado.success(),
                        "la sonda de {} debe salir con 0",
                        ruta.display()
                    );
                }
                return;
            }
        }
    }
    panic!(
        "el ejecutable {} sigue con ETXTBSY (errno 26) tras 50 intentos",
        ruta.display()
    );
}

#[test]
fn script_con_sonda_sale_con_cero_y_no_deja_marcas() {
    let _serie = ejecutor::serie_de_pruebas();
    let tmp = TempDir::new().expect("tmp");
    let marca = tmp.path().join("marca");
    let cuerpo = format!("#!/bin/sh\necho usado > '{}'\n", marca.display());
    let ruta = escribir_ejecutable(&tmp.path().join("script"), &cuerpo);
    assert!(!marca.exists(), "la sonda no deja marca");
    // Sin la sonda el script sí escribe la marca. Así el test no pasa en vacío.
    let estado = Command::new(&ruta).status().expect("lanza el script");
    assert!(estado.success(), "el script real sale con 0");
    assert!(marca.exists(), "el script real escribe la marca");
}

#[test]
fn doscientas_escrituras_con_fork_concurrente_no_fallan() {
    let _serie = ejecutor::serie_de_pruebas();
    // Al salir, con pánico incluido, pone la bandera. Así los hilos del scope siempre terminan.
    struct Parar<'a>(&'a AtomicBool);
    impl Drop for Parar<'_> {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Relaxed);
        }
    }
    let parar = AtomicBool::new(false);
    thread::scope(|s| {
        // Hilos que hacen fork en bucle: su hijo hereda el descriptor de escritura del script.
        for _ in 0..4 {
            s.spawn(|| {
                while !parar.load(Ordering::Relaxed) {
                    let _ = Command::new("true").status();
                }
            });
        }
        let _al_salir = Parar(&parar);
        for i in 0..200 {
            let tmp = TempDir::new().expect("tmp");
            let ruta = tmp.path().join(format!("script-{i}"));
            let cuerpo = "#!/bin/sh\ntrue\n".to_string();
            let escrito = escribir_ejecutable(&ruta, &cuerpo);
            assert_eq!(escrito, ruta);
            let estado = Command::new(&escrito).status().expect("lanza el script");
            assert!(estado.success(), "el script {i} sale con 0");
        }
    });
}

#[test]
fn binario_copiado_pasa_sin_panico() {
    let _serie = ejecutor::serie_de_pruebas();
    let tmp = TempDir::new().expect("tmp");
    let bytes = fs::read("/bin/true").expect("lee /bin/true");
    let ruta = escribir_ejecutable_bytes(&tmp.path().join("true-copia"), &bytes);
    let estado = Command::new(&ruta).status().expect("lanza la copia");
    assert!(estado.success(), "la copia de /bin/true sale con 0");
}
