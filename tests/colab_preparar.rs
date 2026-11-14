//! `tools/colab/verificar-versiones.sh` contra binarios, archivos y `versiones.env` falsos.
//! Ningún test instala nada ni toca red. Los temporales van en `tempfile`, que usa el TMPDIR de test-seguro.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[path = "comun/ejecutable.rs"]
mod ejecutable;
use ejecutable::escribir_ejecutable;

const SCRIPT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tools/colab/verificar-versiones.sh"
);

const NEXTEST_TOML: &str = r#"[store]
dir = "/salida/target/nextest"

[profile.ci]
fail-fast = false
slow-timeout = { period = "60s", terminate-after = 5 }

[profile.ci.junit]
path = "junit.xml"
"#;

const HERRAMIENTAS: [&str; 5] = [
    "cargo-audit",
    "cargo-mutants",
    "cargo-nextest",
    "gitleaks",
    "hadolint",
];

/// Carpetas de un test: falsos para el PATH, binarios de `ROOT_BIN_DIR`, `ROOT_ETC_DIR` y el paquete.
struct Escenario {
    falsos: PathBuf,
    bin: PathBuf,
    etc: PathBuf,
    paquete: PathBuf,
    _raiz: tempfile::TempDir,
}

fn bash_disponible() -> bool {
    Command::new("bash")
        .arg("-c")
        .arg("exit 0")
        .status()
        .is_ok_and(|s| s.success())
}

fn escenario() -> Escenario {
    let raiz = tempfile::tempdir().expect("raíz temporal");
    let e = Escenario {
        falsos: raiz.path().join("falsos"),
        bin: raiz.path().join("bin"),
        etc: raiz.path().join("etc"),
        paquete: raiz.path().join("paquete"),
        _raiz: raiz,
    };
    for carpeta in [&e.falsos, &e.bin, &e.etc, &e.paquete] {
        fs::create_dir_all(carpeta).expect("crea carpeta");
    }
    escribir_ejecutable(
        &e.falsos.join("rustc"),
        "#!/bin/sh\necho 'rustc 1.90.0 (abc 2026-01-01)'\n",
    );
    escribir_ejecutable(
        &e.falsos.join("cargo"),
        "#!/bin/sh\nif [ \"$1 $2\" = 'clippy --version' ]; then echo 'clippy 0.1.90'; exit 0; fi\nexit 2\n",
    );
    escribir_ejecutable(&e.falsos.join("bwrap"), "#!/bin/sh\necho 'bwrap 0.9.0'\n");
    for nombre in HERRAMIENTAS {
        escribir_ejecutable(&e.bin.join(nombre), &format!("#!/bin/sh\necho {nombre}\n"));
    }
    fs::write(e.etc.join("nextest.toml"), NEXTEST_TOML).expect("escribe nextest.toml");
    e
}

/// SHA-256 de cada herramienta falsa, tal como queda en disco.
fn shas_de_bin(e: &Escenario) -> Vec<(&'static str, String)> {
    HERRAMIENTAS
        .iter()
        .map(|n| (*n, sha256(&e.bin.join(n))))
        .collect()
}

fn sha256(ruta: &Path) -> String {
    let salida = Command::new("sha256sum")
        .arg(ruta)
        .output()
        .expect("lanza sha256sum");
    assert!(
        salida.status.success(),
        "sha256sum falla sobre {}",
        ruta.display()
    );
    String::from_utf8(salida.stdout)
        .expect("salida UTF-8")
        .split_whitespace()
        .next()
        .expect("hash")
        .to_string()
}

fn escribir_versiones(e: &Escenario, rust: &str, shas: &[(&str, String)]) {
    let lista: Vec<String> = shas.iter().map(|(n, s)| format!("{n}:{s}")).collect();
    let texto = format!(
        "RUST_VERSION={rust}\nRUSTUP_SHA256={}\nHERRAMIENTAS_SHA256={}\n",
        "0".repeat(64),
        lista.join(",")
    );
    fs::write(e.paquete.join("versiones.env"), texto).expect("escribe versiones.env");
}

/// Corre el script con los falsos al inicio del PATH y las dos rutas de prueba.
fn correr(e: &Escenario) -> Output {
    let path = format!(
        "{}:{}",
        e.falsos.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    Command::new("bash")
        .arg(SCRIPT)
        .arg(&e.paquete)
        .env("PATH", path)
        .env("ROOT_BIN_DIR", &e.bin)
        .env("ROOT_ETC_DIR", &e.etc)
        .output()
        .expect("lanza bash")
}

fn lineas(salida: &Output) -> Vec<String> {
    String::from_utf8_lossy(&salida.stdout)
        .lines()
        .map(String::from)
        .collect()
}

#[test]
fn bash_n_de_verificar() {
    if !bash_disponible() {
        eprintln!("sin bash: se saltea bash_n_de_verificar");
        return;
    }
    let salida = Command::new("bash")
        .arg("-n")
        .arg(SCRIPT)
        .output()
        .expect("lanza bash");
    assert!(
        salida.status.success(),
        "bash -n falla: {}",
        String::from_utf8_lossy(&salida.stderr)
    );
}

#[test]
fn verificacion_coincide() {
    if !bash_disponible() {
        eprintln!("sin bash: se saltea verificacion_coincide");
        return;
    }
    let e = escenario();
    let shas = shas_de_bin(&e);
    escribir_versiones(&e, "1.90.0", &shas);
    let salida = correr(&e);
    assert!(
        salida.status.success(),
        "debe salir 0: {}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let ls = lineas(&salida);
    assert_eq!(ls.len(), 9, "una línea por control: {ls:?}");
    for linea in &ls {
        assert!(linea.starts_with("ok "), "línea inesperada: {linea}");
    }
}

#[test]
fn sha_distinto_nombra_la_herramienta() {
    if !bash_disponible() {
        eprintln!("sin bash: se saltea sha_distinto_nombra_la_herramienta");
        return;
    }
    let e = escenario();
    let mut shas = shas_de_bin(&e);
    for (nombre, sha) in shas.iter_mut() {
        if *nombre == "gitleaks" {
            *sha = "0".repeat(64);
        }
    }
    escribir_versiones(&e, "1.90.0", &shas);
    let salida = correr(&e);
    assert!(!salida.status.success(), "debe salir distinto de 0");
    let ls = lineas(&salida);
    assert!(
        ls.iter().any(|l| l.starts_with("mal gitleaks")),
        "falta la línea mal gitleaks: {ls:?}"
    );
    for linea in ls.iter().filter(|l| !l.starts_with("mal gitleaks")) {
        assert!(linea.starts_with("ok "), "línea inesperada: {linea}");
    }
}

#[test]
fn version_de_rust_distinta_nombra_rustc() {
    if !bash_disponible() {
        eprintln!("sin bash: se saltea version_de_rust_distinta_nombra_rustc");
        return;
    }
    let e = escenario();
    let shas = shas_de_bin(&e);
    escribir_versiones(&e, "1.91.0", &shas);
    let salida = correr(&e);
    assert!(!salida.status.success(), "debe salir distinto de 0");
    let ls = lineas(&salida);
    assert!(
        ls.iter().any(|l| l.starts_with("mal rustc")),
        "falta la línea mal rustc: {ls:?}"
    );
}

#[test]
fn toml_distinto_nombra_nextest_toml() {
    if !bash_disponible() {
        eprintln!("sin bash: se saltea toml_distinto_nombra_nextest_toml");
        return;
    }
    let e = escenario();
    let shas = shas_de_bin(&e);
    escribir_versiones(&e, "1.90.0", &shas);
    fs::write(
        e.etc.join("nextest.toml"),
        NEXTEST_TOML.replace("/salida/target/nextest", "/otro/destino"),
    )
    .expect("escribe nextest.toml cambiado");
    let salida = correr(&e);
    assert!(!salida.status.success(), "debe salir distinto de 0");
    let ls = lineas(&salida);
    assert!(
        ls.iter().any(|l| l.starts_with("mal nextest.toml")),
        "falta la línea mal nextest.toml: {ls:?}"
    );
}
