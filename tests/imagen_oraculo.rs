//! Chequeos de texto para la imagen del oráculo Rust.
//! Leen el Containerfile y los scripts como texto. No construyen ni corren la imagen.

use std::fs;
use std::path::PathBuf;

const CONTAINERFILE: &str = "imagenes/oraculo-rust/Containerfile";
const CONSTRUIR: &str = "scripts/construir-imagen-oraculo.sh";
const PROBAR: &str = "scripts/probar-imagen-oraculo.sh";

const PREFIJO_INSTALL: &str = "cargo install --locked --version ";
const FUENTE_NEXTEST: &str = "https://crates.io/crates/cargo-nextest";
const RUTA_NEXTEST_TOML: &str = "/etc/root-oraculo/nextest.toml";

/// Líneas exactas que deben quedar en nextest.toml, tal como van entre comillas simples en el RUN.
const LINEAS_NEXTEST_TOML: &[&str] = &[
    "[store]",
    "dir = \"/salida/target/nextest\"",
    "[profile.ci.junit]",
    "path = \"junit.xml\"",
];

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
    let posteriores: Vec<(usize, &str)> = lineas
        .iter()
        .enumerate()
        .skip(idx + 1)
        .filter_map(|(i, l)| {
            let primera = l.split_whitespace().next()?;
            INSTRUCCIONES.contains(&primera).then_some((i, *l))
        })
        .collect();
    assert_eq!(
        posteriores.len(),
        1,
        "debe haber exactamente una instrucción después de la capa de cargo-nextest, hay {}: {posteriores:?}",
        posteriores.len()
    );
    let (_, linea_final) = posteriores[0];
    assert!(
        linea_final.contains(RUTA_NEXTEST_TOML),
        "la única instrucción posterior debe escribir {RUTA_NEXTEST_TOML}, es: {linea_final:?}"
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

#[test]
fn f_nextest_toml_va_despues_de_nextest_con_el_contenido() {
    let texto = leer(CONTAINERFILE);
    let lineas: Vec<&str> = texto.lines().collect();
    let (idx_nextest, _) = linea_nextest(&texto).expect("falta la línea de cargo-nextest");
    let (idx_toml, linea_toml) = lineas
        .iter()
        .enumerate()
        .find(|(_, l)| l.trim_start().starts_with("RUN") && l.contains(RUTA_NEXTEST_TOML))
        .expect("falta una instrucción RUN que escriba /etc/root-oraculo/nextest.toml");
    assert!(
        idx_toml > idx_nextest,
        "la instrucción que escribe nextest.toml debe ir después de la capa de cargo-nextest"
    );
    for esperada in LINEAS_NEXTEST_TOML {
        let entre_comillas = format!("'{esperada}'");
        assert!(
            linea_toml.contains(&entre_comillas),
            "nextest.toml no trae la línea {entre_comillas} en el RUN: {linea_toml:?}"
        );
    }
}
