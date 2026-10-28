//! Root no depende del vault de mmorch (decisión b4). El nombre `mmorch` solo etiqueta
//! el origen de los datos importados, y eso vive en `src/importar.rs` y `src/cli.rs` (decisión b3).

use std::fs;
use std::path::{Path, PathBuf};

/// Únicos archivos de `src/` que pueden nombrar mmorch, relativos a `src/`.
const ARCHIVOS_CON_MMORCH: &[&str] = &["importar.rs", "cli.rs"];

fn archivos_src(dir: &Path, salida: &mut Vec<PathBuf>) {
    for entrada in fs::read_dir(dir).expect("lee el directorio de src") {
        let ruta = entrada.expect("entrada válida").path();
        if ruta.is_dir() {
            archivos_src(&ruta, salida);
        } else {
            salida.push(ruta);
        }
    }
}

/// Devuelve `archivo:línea: texto` por cada línea que contiene `palabra`, sin distinguir mayúsculas.
fn apariciones(palabra: &str, excepto: impl Fn(&str) -> bool) -> Vec<String> {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut archivos = Vec::new();
    archivos_src(&raiz, &mut archivos);
    archivos.sort();

    let mut hallazgos = Vec::new();
    for ruta in archivos {
        let relativa = ruta
            .strip_prefix(&raiz)
            .expect("ruta bajo src")
            .to_string_lossy()
            .replace('\\', "/");
        if excepto(&relativa) {
            continue;
        }
        let bytes = fs::read(&ruta).expect("lee el archivo");
        let texto = String::from_utf8_lossy(&bytes);
        for (n, linea) in texto.lines().enumerate() {
            if linea.to_lowercase().contains(palabra) {
                hallazgos.push(format!("src/{relativa}:{}: {}", n + 1, linea.trim()));
            }
        }
    }
    hallazgos
}

#[test]
fn vault_fuera_de_root_src_no_nombra_vault() {
    let hallazgos = apariciones("vault", |_| false);
    assert!(
        hallazgos.is_empty(),
        "src/ no debe nombrar vault (decisión b4). Violaciones:\n{}",
        hallazgos.join("\n")
    );
}

#[test]
fn vault_fuera_de_root_mmorch_solo_en_origen_de_importacion() {
    let hallazgos = apariciones("mmorch", |relativa| ARCHIVOS_CON_MMORCH.contains(&relativa));
    assert!(
        hallazgos.is_empty(),
        "mmorch solo puede aparecer en src/importar.rs y src/cli.rs (decisión b3). Violaciones:\n{}",
        hallazgos.join("\n")
    );
}
