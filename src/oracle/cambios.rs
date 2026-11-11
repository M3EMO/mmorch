//! Rangos de líneas que cambió el repo frente a su estado inicial (`bootstrap/`). El juez los usa para
//! elegir qué código leer primero.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Hasta este tamaño de la parte distinta se calcula la LCS. Más grande, el rango distinto entero cuenta.
const LCS_MAX: usize = 2000;
/// Carpeta que no se recorre, en cualquier nivel.
const SALTEADAS: [&str; 2] = [".git", "target"];

/// Rangos de líneas (1-based, inclusivos) que cambió el repo, por ruta relativa con `/`.
/// Solo mira archivos del repo: un archivo borrado no aporta. Un archivo igual no aporta entrada.
pub(crate) fn rangos_cambiados(inicial: &Path, repo: &Path) -> BTreeMap<String, Vec<(u32, u32)>> {
    let mut relativas = Vec::new();
    recorrer(repo, repo, &mut relativas);
    let mut salida = BTreeMap::new();
    for relativa in relativas {
        let Some(nuevo) = leer_texto(&repo.join(&relativa)) else {
            continue;
        };
        let lineas_nuevo: Vec<&str> = nuevo.lines().collect();
        let ruta_inicial = inicial.join(&relativa);
        let rangos = match fs::symlink_metadata(&ruta_inicial) {
            // Archivo nuevo: cambia entero.
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                if lineas_nuevo.is_empty() {
                    Vec::new()
                } else {
                    vec![(1, lineas_nuevo.len() as u32)]
                }
            }
            Err(_) => continue,
            // Un enlace en el inicial no se sigue.
            Ok(meta) if meta.file_type().is_symlink() => continue,
            Ok(_) => {
                let Some(viejo) = leer_texto(&ruta_inicial) else {
                    continue;
                };
                let lineas_viejo: Vec<&str> = viejo.lines().collect();
                rangos_del_diff(&lineas_viejo, &lineas_nuevo)
            }
        };
        if !rangos.is_empty() {
            salida.insert(clave(&relativa), rangos);
        }
    }
    salida
}

/// Junta las rutas relativas de los archivos regulares de `carpeta`. No sigue enlaces ni baja a `.git/`
/// ni a `target/`.
fn recorrer(raiz: &Path, carpeta: &Path, salida: &mut Vec<PathBuf>) {
    let Ok(entradas) = fs::read_dir(carpeta) else {
        return;
    };
    for entrada in entradas.flatten() {
        let Ok(tipo) = entrada.file_type() else {
            continue;
        };
        if tipo.is_symlink() {
            continue;
        }
        let ruta = entrada.path();
        if tipo.is_dir() {
            let saltear = entrada
                .file_name()
                .to_str()
                .is_some_and(|nombre| SALTEADAS.contains(&nombre));
            if !saltear {
                recorrer(raiz, &ruta, salida);
            }
        } else if tipo.is_file()
            && let Ok(relativa) = ruta.strip_prefix(raiz)
        {
            salida.push(relativa.to_path_buf());
        }
    }
}

/// Ruta relativa con `/`, sin importar el sistema.
fn clave(relativa: &Path) -> String {
    relativa
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Texto del archivo. `None` si no es UTF-8 o no se puede leer: ese archivo se saltea.
fn leer_texto(ruta: &Path) -> Option<String> {
    fs::read_to_string(ruta).ok()
}

/// Rangos de `nuevo` que no están en `viejo`. Quita el prefijo y el sufijo comunes antes de comparar.
/// Una pura eliminación no aporta rango.
fn rangos_del_diff(viejo: &[&str], nuevo: &[&str]) -> Vec<(u32, u32)> {
    let prefijo = viejo.iter().zip(nuevo).take_while(|(a, b)| a == b).count();
    let maximo_sufijo = viejo.len().min(nuevo.len()) - prefijo;
    let sufijo = viejo
        .iter()
        .rev()
        .zip(nuevo.iter().rev())
        .take(maximo_sufijo)
        .take_while(|(a, b)| a == b)
        .count();
    let medio_viejo = &viejo[prefijo..viejo.len() - sufijo];
    let medio_nuevo = &nuevo[prefijo..nuevo.len() - sufijo];
    if medio_nuevo.is_empty() {
        return Vec::new();
    }
    let cambiadas = if medio_viejo.len() > LCS_MAX || medio_nuevo.len() > LCS_MAX {
        vec![true; medio_nuevo.len()]
    } else {
        fuera_de_lcs(medio_viejo, medio_nuevo)
    };
    let mut rangos: Vec<(u32, u32)> = Vec::new();
    for (i, cambiada) in cambiadas.into_iter().enumerate() {
        if !cambiada {
            continue;
        }
        let numero = (prefijo + i + 1) as u32;
        match rangos.last_mut() {
            Some(ultimo) if ultimo.1 + 1 == numero => ultimo.1 = numero,
            _ => rangos.push((numero, numero)),
        }
    }
    rangos
}

/// Por cada línea de `nuevo`: `true` si no está en la subsecuencia común más larga con `viejo`.
fn fuera_de_lcs(viejo: &[&str], nuevo: &[&str]) -> Vec<bool> {
    let (n, m) = (viejo.len(), nuevo.len());
    let ancho = m + 1;
    // `tabla[i * ancho + j]`: largo de la LCS de `viejo[i..]` y `nuevo[j..]`.
    let mut tabla = vec![0u32; (n + 1) * ancho];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            tabla[i * ancho + j] = if viejo[i] == nuevo[j] {
                tabla[(i + 1) * ancho + j + 1] + 1
            } else {
                tabla[(i + 1) * ancho + j].max(tabla[i * ancho + j + 1])
            };
        }
    }
    let mut en_lcs = vec![false; m];
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if viejo[i] == nuevo[j] {
            en_lcs[j] = true;
            i += 1;
            j += 1;
        } else if tabla[(i + 1) * ancho + j] >= tabla[i * ancho + j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    en_lcs.into_iter().map(|c| !c).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    /// Escribe `texto` en `base/ruta`, creando las carpetas que falten.
    fn escribir(base: &Path, ruta: &str, texto: &str) {
        let destino = base.join(ruta);
        fs::create_dir_all(destino.parent().expect("tiene carpeta")).expect("crea carpetas");
        fs::write(destino, texto).expect("escribe");
    }

    /// Par de carpetas: `(inicial, repo)`, vacías.
    fn par() -> (TempDir, TempDir) {
        (
            tempfile::tempdir().expect("tempdir"),
            tempfile::tempdir().expect("tempdir"),
        )
    }

    fn rango(desde: u32, hasta: u32) -> (u32, u32) {
        (desde, hasta)
    }

    #[test]
    fn cambio_de_una_linea_da_su_rango() {
        let (ini, repo) = par();
        escribir(ini.path(), "f.rs", "a\nb\nc\n");
        escribir(repo.path(), "f.rs", "a\nX\nc\n");
        let r = rangos_cambiados(ini.path(), repo.path());
        assert_eq!(r.get("f.rs"), Some(&vec![rango(2, 2)]), "{r:?}");
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn archivo_nuevo_cambia_entero() {
        let (ini, repo) = par();
        escribir(repo.path(), "nuevo.rs", "x\ny\nz\n");
        let r = rangos_cambiados(ini.path(), repo.path());
        assert_eq!(r.get("nuevo.rs"), Some(&vec![rango(1, 3)]), "{r:?}");
    }

    #[test]
    fn archivo_nuevo_vacio_no_aporta_rango() {
        let (ini, repo) = par();
        escribir(repo.path(), "vacio.rs", "");
        let r = rangos_cambiados(ini.path(), repo.path());
        assert!(!r.contains_key("vacio.rs"), "{r:?}");
    }

    #[test]
    fn archivo_borrado_no_aporta_rangos() {
        let (ini, repo) = par();
        escribir(ini.path(), "viejo.rs", "a\nb\n");
        let r = rangos_cambiados(ini.path(), repo.path());
        assert!(r.is_empty(), "{r:?}");
    }

    #[test]
    fn archivo_igual_no_aporta_entrada() {
        let (ini, repo) = par();
        escribir(ini.path(), "f.rs", "a\nb\n");
        escribir(repo.path(), "f.rs", "a\nb\n");
        let r = rangos_cambiados(ini.path(), repo.path());
        assert!(r.is_empty(), "{r:?}");
    }

    #[test]
    fn dos_cambios_separados_dan_dos_rangos() {
        let (ini, repo) = par();
        let base: String = (1..=10).map(|n| format!("l{n}\n")).collect();
        let cambiado = base.replace("l2\n", "X2\n").replace("l8\n", "X8\n");
        escribir(ini.path(), "f.rs", &base);
        escribir(repo.path(), "f.rs", &cambiado);
        let r = rangos_cambiados(ini.path(), repo.path());
        assert_eq!(
            r.get("f.rs"),
            Some(&vec![rango(2, 2), rango(8, 8)]),
            "{r:?}"
        );
    }

    #[test]
    fn salta_git_y_target_en_cualquier_nivel() {
        let (ini, repo) = par();
        escribir(repo.path(), ".git/nota.rs", "x\n");
        escribir(repo.path(), "target/salida.rs", "x\n");
        escribir(repo.path(), "sub/target/otro.rs", "x\n");
        escribir(repo.path(), "sub/.git/otro.rs", "x\n");
        escribir(repo.path(), "src/si.rs", "x\n");
        let r = rangos_cambiados(ini.path(), repo.path());
        let claves: Vec<&String> = r.keys().collect();
        assert_eq!(claves, vec!["src/si.rs"], "{r:?}");
    }

    #[test]
    fn parte_distinta_mayor_a_2000_da_un_solo_rango() {
        let (ini, repo) = par();
        // Inicio y fin iguales. En medio, una de cada 10 líneas del repo coincide con la inicial.
        // Con LCS saldrían muchos rangos. Por el tope sale uno solo, de la línea 2 a la 2501.
        let inicial: String = std::iter::once("inicio\n".to_string())
            .chain((0..2500).map(|_| "viejo\n".to_string()))
            .chain(std::iter::once("fin\n".to_string()))
            .collect();
        let nuevo: String = std::iter::once("inicio\n".to_string())
            .chain((0..2500).map(|n| {
                if n % 10 == 5 {
                    "viejo\n".to_string()
                } else {
                    format!("nuevo {n}\n")
                }
            }))
            .chain(std::iter::once("fin\n".to_string()))
            .collect();
        escribir(ini.path(), "f.rs", &inicial);
        escribir(repo.path(), "f.rs", &nuevo);
        let r = rangos_cambiados(ini.path(), repo.path());
        assert_eq!(r.get("f.rs"), Some(&vec![rango(2, 2501)]), "{r:?}");
    }

    #[test]
    fn archivo_no_utf8_se_saltea() {
        let (ini, repo) = par();
        fs::write(repo.path().join("binario.rs"), [0xff_u8, 0xfe, 0x00]).expect("escribe");
        let r = rangos_cambiados(ini.path(), repo.path());
        assert!(r.is_empty(), "{r:?}");
    }

    #[cfg(unix)]
    #[test]
    fn no_sigue_enlaces_del_repo() {
        let (ini, repo) = par();
        let afuera = tempfile::tempdir().expect("tempdir");
        fs::write(afuera.path().join("secreto.rs"), "x\n").expect("escribe");
        std::os::unix::fs::symlink(
            afuera.path().join("secreto.rs"),
            repo.path().join("enlace.rs"),
        )
        .expect("crea enlace");
        let r = rangos_cambiados(ini.path(), repo.path());
        assert!(r.is_empty(), "{r:?}");
    }
}
