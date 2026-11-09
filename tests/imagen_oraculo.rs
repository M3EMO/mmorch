//! Chequeos de texto para la imagen del oráculo Rust.
//! Leen el Containerfile y los scripts como texto. No construyen ni corren la imagen.

use std::fs;
use std::path::PathBuf;

const CONTAINERFILE: &str = "imagenes/oraculo-rust/Containerfile";
const CONSTRUIR: &str = "scripts/construir-imagen-oraculo.sh";
const PROBAR: &str = "scripts/probar-imagen-oraculo.sh";

const PREFIJO_INSTALL: &str = "cargo install --locked --version ";
const FUENTE_NEXTEST: &str = "https://crates.io/crates/cargo-nextest";

/// Palabras que abren una instrucción de Dockerfile.
const INSTRUCCIONES: &[&str] = &[
    "FROM",
    "RUN",
    "COPY",
    "ADD",
    "ENV",
    "ARG",
    "WORKDIR",
    "USER",
    "CMD",
    "ENTRYPOINT",
    "LABEL",
    "EXPOSE",
    "VOLUME",
    "SHELL",
    "HEALTHCHECK",
    "ONBUILD",
    "STOPSIGNAL",
];

fn leer(relativa: &str) -> String {
    let ruta = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relativa);
    fs::read_to_string(&ruta).unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", ruta.display()))
}

/// Dice si el texto tiene la forma X.Y.Z con números.
fn es_semver(version: &str) -> bool {
    let partes: Vec<&str> = version.split('.').collect();
    partes.len() == 3
        && partes
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

/// Busca la línea `cargo install --locked --version X cargo-nextest`.
/// Devuelve el número de línea (desde 0) y la versión X.
fn linea_nextest(texto: &str) -> Option<(usize, &str)> {
    texto.lines().enumerate().find_map(|(i, linea)| {
        let resto = linea.trim().strip_prefix(PREFIJO_INSTALL)?;
        let mut tokens = resto.split_whitespace();
        let version = tokens.next()?;
        if tokens.any(|t| t == "cargo-nextest") {
            Some((i, version))
        } else {
            None
        }
    })
}

#[test]
fn a_nextest_tiene_version_fija_y_fuente() {
    let texto = leer(CONTAINERFILE);
    let lineas: Vec<&str> = texto.lines().collect();
    let (idx, version) = linea_nextest(&texto)
        .expect("falta la línea `cargo install --locked --version X.Y.Z cargo-nextest`");
    assert!(
        es_semver(version),
        "la versión de cargo-nextest no es X.Y.Z: {version:?}"
    );
    let hay_fuente = lineas[..idx]
        .iter()
        .any(|l| l.trim().starts_with('#') && l.contains(FUENTE_NEXTEST));
    assert!(
        hay_fuente,
        "falta un comentario # con {FUENTE_NEXTEST} antes de la instalación"
    );
}

#[test]
fn b_nextest_va_despues_de_hadolint_y_al_final() {
    let texto = leer(CONTAINERFILE);
    let lineas: Vec<&str> = texto.lines().collect();
    let (idx, _) = linea_nextest(&texto).expect("falta la línea de cargo-nextest");
    let idx_hadolint = lineas
        .iter()
        .position(|l| l.contains("hadolint-linux-x86_64"))
        .expect("falta la descarga de hadolint");
    assert!(
        idx > idx_hadolint,
        "la capa de cargo-nextest debe ir después de la de hadolint"
    );
    let instruccion_despues = lineas[idx + 1..].iter().find_map(|l| {
        let primera = l.split_whitespace().next()?;
        INSTRUCCIONES.contains(&primera).then_some(primera)
    });
    assert!(
        instruccion_despues.is_none(),
        "hay una instrucción {instruccion_despues:?} después de la capa de cargo-nextest"
    );
}

#[test]
fn c_construir_tiene_default_3() {
    let texto = leer(CONSTRUIR);
    assert!(
        texto.contains("IMAGEN=${IMAGEN:-root-oraculo-rust:3}"),
        "construir-imagen-oraculo.sh debe tener por defecto root-oraculo-rust:3"
    );
}

#[test]
fn d_probar_tiene_default_3_y_chequeo_nextest() {
    let texto = leer(PROBAR);
    assert!(
        texto.contains("IMAGEN=${IMAGEN:-localhost/root-oraculo-rust:3}"),
        "probar-imagen-oraculo.sh debe tener por defecto localhost/root-oraculo-rust:3"
    );
    let hay_chequeo = texto.lines().any(|l| {
        let t = l.trim();
        t.starts_with("chequeo ")
            && t.contains("(g) cargo nextest --version")
            && t.contains("correr \"$IMAGEN\" cargo nextest --version")
    });
    assert!(
        hay_chequeo,
        "falta el chequeo (g) que corre `cargo nextest --version`"
    );
}

#[test]
fn e_herramientas_anteriores_sin_cambio_de_version() {
    let texto = leer(CONTAINERFILE);
    for exigido in [
        "--version 0.22.2 cargo-audit",
        "--version 27.1.0 cargo-mutants",
        "releases/download/v8.30.1/gitleaks_8.30.1_linux_x64.tar.gz",
        "releases/download/v2.15.1/hadolint-linux-x86_64",
    ] {
        assert!(
            texto.contains(exigido),
            "el Containerfile perdió o cambió: {exigido}"
        );
    }
}
