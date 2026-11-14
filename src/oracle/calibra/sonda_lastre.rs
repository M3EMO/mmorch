//! Sonda de lastre: marca las funciones de producción de una copia de `bootstrap/` y anota cuáles corren.
//! Funciones puras sobre texto. Solo `armar_copia_marcada` toca archivos.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::Path;

use super::inyectar::{
    Cuerpo, cuerpo_de, es_fn_normal, limite_de_produccion, limpias_de, nombre_de_funcion, partir,
    unir,
};
use crate::oracle::descarga::PRELUDIO;

/// Función de producción marcada. `linea` es 1-based y apunta a la firma en el archivo original.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct FuncionMarcada {
    pub archivo: String,
    pub linea: u32,
    pub nombre: String,
}

/// Marca de inicio de la salida de la sonda.
const INICIO: &str = "ROOT_SONDA_INICIO";
/// Marca de fin de la salida de la sonda.
const FIN: &str = "ROOT_SONDA_FIN";

/// Devuelve el texto marcado y el mapa de `<id>` a función. Cada marca va justo después de la `{`.
pub(crate) fn marcar_archivo(
    ruta_relativa: &str,
    texto: &str,
) -> (String, BTreeMap<String, FuncionMarcada>) {
    let lineas = partir(texto);
    let limpias = limpias_de(&lineas);
    let limite = limite_de_produccion(&lineas);
    // Funciones marcables, en orden de archivo: (línea de firma, cuerpo, nombre).
    let mut hallazgos: Vec<(usize, Cuerpo, &str)> = Vec::new();
    for firma in 0..limite {
        let Some(nombre) = nombre_de_funcion(&lineas[firma]) else {
            continue;
        };
        if nombre == "main" || !es_fn_normal(&lineas, firma) {
            continue;
        }
        let Some(cuerpo) = cuerpo_de(&limpias, firma).filter(|c| c.cierra < limite) else {
            continue;
        };
        if lineas[cuerpo.abre].trim_end().ends_with('{') {
            hallazgos.push((firma, cuerpo, nombre));
        }
    }
    let mut mapa = BTreeMap::new();
    let mut inserciones = Vec::new();
    for (n, (firma, cuerpo, nombre)) in hallazgos.iter().enumerate() {
        let linea = *firma as u32 + 1;
        let id = id_de(ruta_relativa, linea, nombre);
        inserciones.push((cuerpo.abre, linea_marca(n, &id)));
        mapa.insert(
            id,
            FuncionMarcada {
                archivo: ruta_relativa.to_string(),
                linea,
                nombre: nombre.to_string(),
            },
        );
    }
    // De abajo hacia arriba: las inserciones no mueven las posiciones pendientes.
    let mut salida = lineas;
    for (abre, marca) in inserciones.into_iter().rev() {
        salida.insert(abre + 1, marca);
    }
    (unir(&salida), mapa)
}

/// Identificador de archivo: `archivo__linea__nombre` con todo carácter fuera de `[A-Za-z0-9_]` en `_`.
fn id_de(archivo: &str, linea: u32, nombre: &str) -> String {
    format!("{archivo}__{linea}__{nombre}")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Línea que la sonda inserta al inicio de una función. `n` es único por archivo.
fn linea_marca(n: usize, id: &str) -> String {
    format!(
        "{{ static ROOT_SONDA_{n}: ::std::sync::atomic::AtomicBool = ::std::sync::atomic::AtomicBool::new(false); if !ROOT_SONDA_{n}.swap(true, ::std::sync::atomic::Ordering::Relaxed) {{ let _ = ::std::fs::write(\"/tmp/root-sonda/{id}\", b\"\"); }} }}"
    )
}

/// Copia `bootstrap` a `destino` y marca cada `.rs` bajo `src/`. Omite `target/` y `.git/` de la raíz.
pub(crate) fn armar_copia_marcada(
    bootstrap: &Path,
    destino: &Path,
) -> io::Result<BTreeMap<String, FuncionMarcada>> {
    let mut mapa = BTreeMap::new();
    copiar_marcando(bootstrap, destino, "", &mut mapa)?;
    Ok(mapa)
}

/// Copia `origen` a `destino` con `relativa` como ruta del directorio actual. Vacía en la raíz.
fn copiar_marcando(
    origen: &Path,
    destino: &Path,
    relativa: &str,
    mapa: &mut BTreeMap<String, FuncionMarcada>,
) -> io::Result<()> {
    fs::create_dir_all(destino)?;
    for entrada in fs::read_dir(origen)? {
        let entrada = entrada?;
        let nombre = entrada.file_name();
        let nombre_texto = nombre.to_string_lossy().into_owned();
        let ruta = entrada.path();
        let rel = if relativa.is_empty() {
            nombre_texto.clone()
        } else {
            format!("{relativa}/{nombre_texto}")
        };
        let dest = destino.join(&nombre);
        if ruta.is_dir() {
            if relativa.is_empty() && (nombre_texto == "target" || nombre_texto == ".git") {
                continue;
            }
            copiar_marcando(&ruta, &dest, &rel, mapa)?;
        } else if rel.starts_with("src/") && rel.ends_with(".rs") {
            match fs::read_to_string(&ruta) {
                Ok(texto) => {
                    let (marcado, marcas) = marcar_archivo(&rel, &texto);
                    fs::write(&dest, marcado)?;
                    mapa.extend(marcas);
                }
                // Un `.rs` que no es UTF-8 se copia tal cual.
                Err(e) if e.kind() == io::ErrorKind::InvalidData => {
                    fs::copy(&ruta, &dest)?;
                }
                Err(e) => return Err(e),
            }
        } else {
            fs::copy(&ruta, &dest)?;
        }
    }
    Ok(())
}

/// Comando de shell que corre la sonda. Nextest no corta en el primer fallo; `ls` corre aunque falle.
pub(crate) fn cmd_sonda() -> String {
    format!(
        "{PRELUDIO}rm -rf /tmp/root-sonda; mkdir -p /tmp/root-sonda; cd /repo && export CARGO_TARGET_DIR=/salida/target && cargo nextest run --offline --no-fail-fast --profile ci --tool-config-file root:/etc/root-oraculo/nextest.toml; echo ROOT_SONDA_INICIO; ls /tmp/root-sonda; echo ROOT_SONDA_FIN"
    )
}

/// Líneas entre las dos marcas, si ambas están y en orden. `None` si no.
fn tramo(stdout: &str) -> Option<Vec<&str>> {
    let lineas: Vec<&str> = stdout.lines().map(str::trim).collect();
    let ini = lineas.iter().position(|l| *l == INICIO)?;
    let fin = lineas[ini + 1..].iter().position(|l| *l == FIN)? + ini + 1;
    Some(lineas[ini + 1..fin].to_vec())
}

/// Funciones que corrieron según la salida del comando. Los ids fuera de `mapa` se ignoran.
pub(crate) fn leer_salida(
    stdout: &str,
    mapa: &BTreeMap<String, FuncionMarcada>,
) -> BTreeSet<FuncionMarcada> {
    tramo(stdout)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|id| mapa.get(id).cloned())
        .collect()
}

/// `true` si la salida trae ambas marcas, en orden. Sin ellas, la sonda no corrió.
pub(crate) fn salida_valida(stdout: &str) -> bool {
    tramo(stdout).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cargo del entorno, o `cargo` del PATH.
    fn cargo() -> String {
        std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())
    }

    #[test]
    fn sonda_lastre_marca_dos_funciones_con_numeros_distintos() {
        let texto = "pub fn uno() -> u32 {\n    1\n}\n\nfn dos() -> u32 {\n    2\n}\n";
        let (marcado, mapa) = marcar_archivo("src/a.rs", texto);
        assert!(marcado.contains("static ROOT_SONDA_0:"));
        assert!(marcado.contains("static ROOT_SONDA_1:"));
        assert_eq!(mapa.len(), 2);
        let uno = mapa.get("src_a_rs__1__uno").expect("uno marcada");
        assert_eq!((uno.linea, uno.nombre.as_str()), (1, "uno"));
        assert_eq!(uno.archivo, "src/a.rs");
        let dos = mapa.get("src_a_rs__5__dos").expect("dos marcada");
        assert_eq!((dos.linea, dos.nombre.as_str()), (5, "dos"));
        // La marca va justo después de la `{` de cada función, en una línea sola.
        let lineas: Vec<&str> = marcado.lines().collect();
        let marca = |n: usize, id: &str| {
            format!(
                "{{ static ROOT_SONDA_{n}: ::std::sync::atomic::AtomicBool = ::std::sync::atomic::AtomicBool::new(false); if !ROOT_SONDA_{n}.swap(true, ::std::sync::atomic::Ordering::Relaxed) {{ let _ = ::std::fs::write(\"/tmp/root-sonda/{id}\", b\"\"); }} }}"
            )
        };
        assert_eq!(lineas[1], marca(0, "src_a_rs__1__uno"));
        assert_eq!(lineas[6], marca(1, "src_a_rs__5__dos"));
    }

    #[test]
    fn sonda_lastre_archivo_marcado_compila() {
        let fuente = "pub fn uno(a: u32) -> u32 {\n    a + 1\n}\n\npub fn generica<T: Clone>(t: &T) -> T {\n    t.clone()\n}\n\npub fn con_retorno() -> Option<u32> {\n    Some(uno(1))\n}\n";
        let (marcado, mapa) = marcar_archivo("src/lib.rs", fuente);
        assert_eq!(mapa.len(), 3);
        let dir = tempfile::tempdir().expect("temporal");
        fs::create_dir_all(dir.path().join("src")).expect("carpeta src");
        fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"sonda_lastre_fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
        )
        .expect("Cargo.toml");
        fs::write(dir.path().join("src/lib.rs"), marcado).expect("lib.rs");
        let salida = std::process::Command::new(cargo())
            .arg("check")
            .arg("--offline")
            .current_dir(dir.path())
            .env("CARGO_TARGET_DIR", dir.path().join("target"))
            .output()
            .expect("lanza cargo check");
        assert!(
            salida.status.success(),
            "cargo check: {}",
            String::from_utf8_lossy(&salida.stderr)
        );
    }

    #[test]
    fn sonda_lastre_const_unsafe_main_y_tests_no_se_marcan() {
        let texto = "pub const fn constante() -> u32 {\n    1\n}\n\npub unsafe fn insegura() -> u32 {\n    1\n}\n\nfn main() {\n    println!(\"hola\");\n}\n\nfn normal() -> u32 {\n    2\n}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn de_test() {\n        assert_eq!(1, 1);\n    }\n\n    fn ayuda() -> u32 {\n        3\n    }\n}\n";
        let (marcado, mapa) = marcar_archivo("src/a.rs", texto);
        let nombres: Vec<&str> = mapa.values().map(|f| f.nombre.as_str()).collect();
        assert_eq!(nombres, vec!["normal"]);
        assert!(!marcado.contains("ROOT_SONDA_1"));
    }

    #[test]
    fn sonda_lastre_leer_salida_da_las_funciones_que_corrieron() {
        let texto = "fn uno() -> u32 {\n    1\n}\n\nfn dos() -> u32 {\n    2\n}\n";
        let (_, mapa) = marcar_archivo("src/a.rs", texto);
        let salida = "salida de tests\nROOT_SONDA_INICIO\nsrc_a_rs__1__uno\nsrc_fuera__9__x\n  src_a_rs__5__dos  \nROOT_SONDA_FIN\n";
        let nombres: Vec<String> = leer_salida(salida, &mapa)
            .into_iter()
            .map(|f| f.nombre)
            .collect();
        assert_eq!(nombres, vec!["uno".to_string(), "dos".to_string()]);
    }

    #[test]
    fn sonda_lastre_leer_salida_ignora_lo_que_esta_fuera_de_las_marcas() {
        let texto = "fn uno() -> u32 {\n    1\n}\n";
        let (_, mapa) = marcar_archivo("src/a.rs", texto);
        let antes = "src_a_rs__1__uno\nROOT_SONDA_INICIO\nROOT_SONDA_FIN\n";
        assert!(leer_salida(antes, &mapa).is_empty());
        let sin_marcas = "src_a_rs__1__uno\n";
        assert!(leer_salida(sin_marcas, &mapa).is_empty());
        assert!(!salida_valida(sin_marcas));
        let sin_fin = "ROOT_SONDA_INICIO\nsrc_a_rs__1__uno\n";
        assert!(leer_salida(sin_fin, &mapa).is_empty());
        assert!(!salida_valida(sin_fin));
    }

    #[test]
    fn sonda_lastre_comando_corre_nextest_sin_cortar_y_lista_marcas() {
        let cmd = cmd_sonda();
        assert!(cmd.contains("--no-fail-fast"));
        assert!(cmd.contains("mkdir -p /tmp/root-sonda"));
        assert!(cmd.contains("cargo nextest run --offline"));
        assert!(cmd.contains("ls /tmp/root-sonda"));
        assert!(cmd.starts_with(PRELUDIO));
        let nextest = cmd.find("cargo nextest").expect("nextest");
        let ls = cmd.find("ls /tmp/root-sonda").expect("ls");
        assert!(nextest < ls, "el ls va después de nextest");
    }

    #[test]
    fn sonda_lastre_armar_copia_marca_src_y_omite_target() {
        let origen = tempfile::tempdir().expect("origen");
        fs::create_dir_all(origen.path().join("src")).expect("src");
        fs::create_dir_all(origen.path().join("target")).expect("target");
        fs::write(
            origen.path().join("src/a.rs"),
            "fn uno() -> u32 {\n    1\n}\n",
        )
        .expect("a.rs");
        fs::write(origen.path().join("target/x"), "x").expect("target/x");
        fs::write(origen.path().join("Cargo.toml"), "[package]\n").expect("Cargo.toml");
        let base = tempfile::tempdir().expect("destino");
        let destino = base.path().join("copia");
        let mapa = armar_copia_marcada(origen.path(), &destino).expect("copia");
        assert_eq!(mapa.len(), 1);
        let a = fs::read_to_string(destino.join("src/a.rs")).expect("a.rs copiado");
        assert!(a.contains("ROOT_SONDA_0"));
        assert!(!destino.join("target").exists());
        assert!(destino.join("Cargo.toml").exists());
    }
}
