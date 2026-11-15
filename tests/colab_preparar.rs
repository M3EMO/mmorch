//! `tools/colab/verificar-versiones.sh` contra binarios, archivos y `versiones.env` falsos.
//! Ningún test instala nada ni toca red. Los temporales van en `tempfile`, que usa el TMPDIR de test-seguro.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[path = "comun/ejecutable.rs"]
mod ejecutable;
use ejecutable::escribir_ejecutable;

use root::oracle::contenedor::CARPETAS_RUNTIME;

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

/// Carpetas de un test: falsos para el PATH, binarios, `ROOT_ETC_DIR`, el paquete y `runtime`.
/// La raíz `runtime` tiene las seis carpetas de `root::oracle::contenedor::CARPETAS_RUNTIME`.
struct Escenario {
    falsos: PathBuf,
    bin: PathBuf,
    etc: PathBuf,
    paquete: PathBuf,
    runtime: PathBuf,
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
        runtime: raiz.path().join("runtime"),
        _raiz: raiz,
    };
    for carpeta in [&e.falsos, &e.bin, &e.etc, &e.paquete] {
        fs::create_dir_all(carpeta).expect("crea carpeta");
    }
    for ruta in CARPETAS_RUNTIME.iter() {
        // Path::join con una ruta absoluta la reemplaza. Se quita la barra inicial.
        fs::create_dir_all(e.runtime.join(ruta.trim_start_matches('/'))).expect("crea carpeta");
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
        .env("ROOT_RAIZ_DIR", &e.runtime)
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
    assert_eq!(ls.len(), 15, "una línea por control: {ls:?}");
    for linea in &ls {
        assert!(linea.starts_with("ok "), "línea inesperada: {linea}");
    }
}

#[test]
fn seis_carpetas_dan_ok_cada_una() {
    if !bash_disponible() {
        eprintln!("sin bash: se saltea seis_carpetas_dan_ok_cada_una");
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
    for ruta in CARPETAS_RUNTIME.iter() {
        let esperado = format!("ok {ruta}");
        assert!(ls.contains(&esperado), "falta {esperado}: {ls:?}");
    }
}

#[test]
fn sin_repo_nombra_repo_y_sale_mal() {
    if !bash_disponible() {
        eprintln!("sin bash: se saltea sin_repo_nombra_repo_y_sale_mal");
        return;
    }
    let e = escenario();
    let shas = shas_de_bin(&e);
    escribir_versiones(&e, "1.90.0", &shas);
    fs::remove_dir(e.runtime.join("repo")).expect("quita repo");
    let salida = correr(&e);
    assert!(!salida.status.success(), "debe salir distinto de 0");
    let ls = lineas(&salida);
    assert!(
        ls.iter().any(|l| l.starts_with("mal /repo")),
        "falta la línea mal /repo: {ls:?}"
    );
}

/// Lee la lista `CARPETAS_RUNTIME=(...)` de un script. Debe caber en una sola línea.
/// Si falta la línea, el error nombra el script.
fn lista_de_carpetas(nombre_script: &str, texto: &str) -> Result<Vec<String>, String> {
    let linea = texto
        .lines()
        .find(|l| l.starts_with("CARPETAS_RUNTIME=("))
        .ok_or_else(|| format!("falta la línea CARPETAS_RUNTIME=(...) en {nombre_script}"))?;
    Ok(linea
        .trim_start_matches("CARPETAS_RUNTIME=(")
        .trim_end_matches(')')
        .split_whitespace()
        .map(String::from)
        .collect())
}

/// Compara la lista `CARPETAS_RUNTIME` de un script con la del crate. Si difiere o falta, el error nombra el script.
fn comparar_lista(nombre_script: &str, texto: &str) -> Result<(), String> {
    let esperado: Vec<String> = CARPETAS_RUNTIME.iter().map(|r| r.to_string()).collect();
    let lista = lista_de_carpetas(nombre_script, texto)?;
    if lista == esperado {
        return Ok(());
    }
    Err(format!(
        "la lista de {nombre_script} no es la del crate: {lista:?}, se esperaba {esperado:?}"
    ))
}

/// Los dos scripts deben tener la misma lista que la constante del crate.
#[test]
fn carpetas_del_runtime_son_los_montajes_de_bwrap() {
    for script in [PREPARAR, SCRIPT] {
        let texto = fs::read_to_string(script).expect("lee el script");
        comparar_lista(script, &texto).expect("lista de carpetas");
    }
}

#[test]
fn comparar_lista_nombra_el_script_si_la_lista_difiere() {
    let falso = "CARPETAS_RUNTIME=(/repo /otra)\n";
    let error = comparar_lista("tools/colab/falso.sh", falso).expect_err("debe dar error");
    assert!(
        error.contains("tools/colab/falso.sh"),
        "el error no nombra el script: {error}"
    );
    let igual = "CARPETAS_RUNTIME=(/repo /salida /oraculo /cache /rustsec /trabajo)\n";
    assert!(comparar_lista("tools/colab/falso.sh", igual).is_ok());
}

#[test]
fn lista_ausente_nombra_el_guion() {
    let raiz = tempfile::tempdir().expect("raíz temporal");
    let ruta = raiz.path().join("falso.sh");
    fs::write(&ruta, "#!/bin/bash\necho sin lista\n").expect("escribe falso.sh");
    let texto = fs::read_to_string(&ruta).expect("lee falso.sh");
    let nombre = ruta.display().to_string();
    let error = comparar_lista(&nombre, &texto).expect_err("debe dar error");
    assert!(
        error.contains(&nombre),
        "el error no nombra el guion: {error}"
    );
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

const PREPARAR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tools/colab/preparar-runtime.sh"
);

const CONTAINERFILE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/imagenes/oraculo-rust/Containerfile"
);

/// Bloque de nextest.toml dentro de `verificar-versiones.sh`: desde `[store]` hasta `path = "junit.xml"`.
fn bloque_nextest(texto: &str) -> String {
    let lineas: Vec<&str> = texto.lines().collect();
    // La primera línea del bloque lleva el prefijo `NEXTEST_TOML='`, que no es parte del texto.
    let inicio = lineas
        .iter()
        .position(|l| l.trim_start_matches("NEXTEST_TOML='") == "[store]")
        .expect("falta [store] en verificar-versiones.sh");
    let fin = lineas[inicio..]
        .iter()
        .position(|l| *l == "path = \"junit.xml\"")
        .expect("falta path = \"junit.xml\" en verificar-versiones.sh")
        + inicio;
    let mut bloque: Vec<&str> = lineas[inicio..=fin].to_vec();
    bloque[0] = bloque[0].trim_start_matches("NEXTEST_TOML='");
    bloque.join("\n")
}

#[test]
fn bash_n_de_preparar() {
    if !bash_disponible() {
        eprintln!("sin bash: se saltea bash_n_de_preparar");
        return;
    }
    let salida = Command::new("bash")
        .arg("-n")
        .arg(PREPARAR)
        .output()
        .expect("lanza bash");
    assert!(
        salida.status.success(),
        "bash -n falla: {}",
        String::from_utf8_lossy(&salida.stderr)
    );
}

#[test]
fn preparar_y_verificar_comparten_el_texto_de_nextest() {
    let preparar = fs::read_to_string(PREPARAR).expect("lee preparar-runtime.sh");
    let verificar = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tools/colab/verificar-versiones.sh"
    ))
    .expect("lee verificar-versiones.sh");
    let containerfile = fs::read_to_string(CONTAINERFILE).expect("lee Containerfile");

    let bloque = bloque_nextest(&verificar);
    assert!(
        preparar.contains(&bloque),
        "preparar-runtime.sh no contiene el bloque de nextest.toml:\n{bloque}"
    );
    for linea in bloque.lines().filter(|l| !l.is_empty()) {
        assert!(
            containerfile.contains(linea),
            "el Containerfile no contiene la línea de nextest.toml: {linea}"
        );
    }
}

#[test]
fn preparar_termina_llamando_a_verificar() {
    let preparar = fs::read_to_string(PREPARAR).expect("lee preparar-runtime.sh");
    for esperado in [
        "verificar-versiones.sh",
        "set -euo pipefail",
        "apt-get install -y bubblewrap",
        "/usr/local/rustup",
        "/usr/local/cargo",
        "--profile minimal",
        "rustup component add clippy",
        "0755",
    ] {
        assert!(
            preparar.contains(esperado),
            "preparar-runtime.sh no contiene {esperado:?}"
        );
    }
}

/// Corre el guion sin RUSTUP_HOME ni CARGO_HOME del entorno. Si `rustup_home` es `Some`, lo exporta.
fn correr_sin_rustup(e: &Escenario, rustup_home: Option<&str>) -> Output {
    let path = format!(
        "{}:{}",
        e.falsos.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut orden = Command::new("bash");
    orden
        .arg(SCRIPT)
        .arg(&e.paquete)
        .env("PATH", path)
        .env("ROOT_BIN_DIR", &e.bin)
        .env("ROOT_ETC_DIR", &e.etc)
        .env("ROOT_RAIZ_DIR", &e.runtime)
        .env_remove("RUSTUP_HOME")
        .env_remove("CARGO_HOME");
    if let Some(valor) = rustup_home {
        orden.env("RUSTUP_HOME", valor);
    }
    orden.output().expect("lanza bash")
}

/// Pone un rustc falso que escribe su RUSTUP_HOME y su CARGO_HOME en `e.falsos/entorno.txt`.
/// Devuelve la ruta de ese archivo.
fn rustc_que_registra_entorno(e: &Escenario) -> PathBuf {
    let registro = e.falsos.join("entorno.txt");
    let guion = format!(
        "#!/bin/sh\nprintf '%s\\n%s\\n' \"${{RUSTUP_HOME:-ausente}}\" \"${{CARGO_HOME:-ausente}}\" > '{}'\necho 'rustc 1.90.0 (abc 2026-01-01)'\n",
        registro.display()
    );
    escribir_ejecutable(&e.falsos.join("rustc"), &guion);
    registro
}

#[test]
fn sin_rustup_home_el_guion_fija_las_rutas_de_rust() {
    if !bash_disponible() {
        eprintln!("sin bash: se saltea sin_rustup_home_el_guion_fija_las_rutas_de_rust");
        return;
    }
    let e = escenario();
    let shas = shas_de_bin(&e);
    escribir_versiones(&e, "1.90.0", &shas);
    let registro = rustc_que_registra_entorno(&e);
    let salida = correr_sin_rustup(&e, None);
    assert!(
        salida.status.success(),
        "debe salir 0: {}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let vistos: Vec<String> = fs::read_to_string(&registro)
        .expect("lee el registro del rustc falso")
        .lines()
        .map(String::from)
        .collect();
    assert_eq!(
        vistos,
        vec!["/usr/local/rustup", "/usr/local/cargo"],
        "rustc debe ver RUSTUP_HOME y CARGO_HOME del guion"
    );
}

#[test]
fn rustup_home_del_entorno_se_conserva() {
    if !bash_disponible() {
        eprintln!("sin bash: se saltea rustup_home_del_entorno_se_conserva");
        return;
    }
    let e = escenario();
    let shas = shas_de_bin(&e);
    escribir_versiones(&e, "1.90.0", &shas);
    let registro = rustc_que_registra_entorno(&e);
    let salida = correr_sin_rustup(&e, Some("/otro"));
    assert!(
        salida.status.success(),
        "debe salir 0: {}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let visto = fs::read_to_string(&registro).expect("lee el registro del rustc falso");
    assert_eq!(
        visto.lines().next(),
        Some("/otro"),
        "el guion debe conservar RUSTUP_HOME del entorno"
    );
}
