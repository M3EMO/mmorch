//! Reuso del `target` de Cargo entre la línea base y los casos de la calibración (Tarea 59). Un caso reusa el
//! `target` de la base solo si su copia tiene los mismos `Cargo.toml` y `Cargo.lock` y la misma imagen. Si no,
//! compila en frío. Las funciones de este archivo no cambian nada: solo miran archivos y la imagen.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Carpetas de la raíz que no cuentan: su contenido no es código fuente de la copia.
const EXCLUIDOS_RAIZ: &[&str] = &["target", ".git"];
/// Nombres de archivo que cuentan como manifiesto. Cualquier carpeta del árbol puede tenerlos.
const MANIFIESTOS: &[&str] = &["Cargo.toml", "Cargo.lock"];

/// Si `a` y `b` tienen los mismos `Cargo.toml` y `Cargo.lock`, con las mismas rutas relativas y contenido.
/// Un manifiesto nuevo o borrado cuenta como cambio.
pub fn manifiestos_iguales(a: &Path, b: &Path) -> io::Result<bool> {
    Ok(manifiestos(a)? == manifiestos(b)?)
}

/// Contenido de cada manifiesto de `raiz`, con su ruta relativa como clave. Omite `target/` y `.git/` de la raíz.
fn manifiestos(raiz: &Path) -> io::Result<BTreeMap<PathBuf, Vec<u8>>> {
    let mut mapa = BTreeMap::new();
    recoger(raiz, raiz, &mut mapa)?;
    Ok(mapa)
}

fn recoger(raiz: &Path, dir: &Path, mapa: &mut BTreeMap<PathBuf, Vec<u8>>) -> io::Result<()> {
    for entrada in fs::read_dir(dir)? {
        let entrada = entrada?;
        let tipo = entrada.file_type()?;
        let nombre = entrada.file_name();
        // Un enlace simbólico no se sigue: no es parte del árbol que se copia.
        if tipo.is_dir() {
            if dir == raiz && EXCLUIDOS_RAIZ.iter().any(|excluido| nombre == *excluido) {
                continue;
            }
            recoger(raiz, &entrada.path(), mapa)?;
        } else if tipo.is_file() && MANIFIESTOS.iter().any(|m| nombre == *m) {
            let ruta = entrada.path();
            let relativa = ruta.strip_prefix(raiz).unwrap_or(&ruta).to_path_buf();
            mapa.insert(relativa, fs::read(&ruta)?);
        }
    }
    Ok(())
}

/// Id de la imagen `imagen` con `podman`. `None` si `podman` falla o no hay imagen. Con imágenes locales el
/// `.Digest` sale vacío, así que se usa el `.Id`.
pub fn digest_de_imagen(imagen: &str) -> Option<String> {
    let salida = Command::new("podman")
        .args(["image", "inspect", "--format", "{{.Id}}", imagen])
        .output()
        .ok()?;
    if !salida.status.success() {
        return None;
    }
    let id = String::from_utf8(salida.stdout).ok()?.trim().to_string();
    (!id.is_empty()).then_some(id)
}

/// Si el caso puede reusar el `target` de la base: hay `target_base`, los manifiestos son iguales y los digests
/// de la imagen también. Dos digests `None` cuentan como iguales. Un error al leer la copia cuenta como cambio:
/// el caso compila en frío.
pub fn puede_reusar(
    base: &Path,
    copia: &Path,
    target_base: Option<&Path>,
    digest_base: Option<&str>,
    digest_ahora: Option<&str>,
) -> bool {
    target_base.is_some()
        && matches!(manifiestos_iguales(base, copia), Ok(true))
        && digest_base == digest_ahora
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Árbol con un workspace: `Cargo.toml` y `Cargo.lock` en la raíz, un miembro con su `Cargo.toml` y `.rs`.
    fn arbol(dir: &Path) {
        fs::create_dir_all(dir.join("src")).expect("src");
        fs::create_dir_all(dir.join("crates/miembro/src")).expect("miembro");
        fs::write(
            dir.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/miembro\"]\n",
        )
        .expect("Cargo.toml");
        fs::write(dir.join("Cargo.lock"), "version = 4\n").expect("Cargo.lock");
        fs::write(dir.join("src/lib.rs"), "pub fn f() {}\n").expect("lib");
        fs::write(
            dir.join("crates/miembro/Cargo.toml"),
            "[package]\nname = \"miembro\"\n",
        )
        .expect("Cargo.toml del miembro");
        fs::write(dir.join("crates/miembro/src/lib.rs"), "pub fn g() {}\n").expect("lib miembro");
    }

    /// Dos árboles iguales: `a` y `b`.
    fn par() -> (tempfile::TempDir, tempfile::TempDir) {
        let a = tempfile::tempdir().expect("a");
        let b = tempfile::tempdir().expect("b");
        arbol(a.path());
        arbol(b.path());
        (a, b)
    }

    #[test]
    fn target_manifiestos_iguales_da_true() {
        let (a, b) = par();
        assert!(manifiestos_iguales(a.path(), b.path()).expect("compara"));
    }

    #[test]
    fn target_cargo_lock_cambiado_cuenta_como_cambio() {
        let (a, b) = par();
        fs::write(b.path().join("Cargo.lock"), "version = 3\n").expect("cambia el lock");
        assert!(!manifiestos_iguales(a.path(), b.path()).expect("compara"));
    }

    #[test]
    fn target_cargo_toml_de_miembro_cambiado_cuenta_como_cambio() {
        let (a, b) = par();
        fs::write(
            b.path().join("crates/miembro/Cargo.toml"),
            "[package]\nname = \"miembro\"\nversion = \"0.2.0\"\n",
        )
        .expect("cambia el miembro");
        assert!(!manifiestos_iguales(a.path(), b.path()).expect("compara"));
    }

    #[test]
    fn target_rs_cambiado_no_cuenta_como_cambio() {
        let (a, b) = par();
        fs::write(b.path().join("src/lib.rs"), "pub fn f() -> u8 { 1 }\n").expect("cambia el rs");
        assert!(manifiestos_iguales(a.path(), b.path()).expect("compara"));
    }

    #[test]
    fn target_target_de_la_raiz_se_ignora() {
        let (a, b) = par();
        fs::create_dir_all(b.path().join("target/debug")).expect("target");
        fs::write(b.path().join("target/Cargo.lock"), "ruido\n").expect("lock en target");
        fs::write(b.path().join("target/debug/x"), "binario\n").expect("binario");
        assert!(manifiestos_iguales(a.path(), b.path()).expect("compara"));
    }

    #[test]
    fn target_manifiesto_nuevo_o_borrado_cuenta_como_cambio() {
        let (a, b) = par();
        fs::create_dir_all(b.path().join("crates/otro")).expect("otro");
        fs::write(b.path().join("crates/otro/Cargo.toml"), "[package]\n").expect("nuevo");
        assert!(!manifiestos_iguales(a.path(), b.path()).expect("compara nuevo"));

        let (a2, b2) = par();
        fs::remove_file(b2.path().join("crates/miembro/Cargo.toml")).expect("borra");
        assert!(!manifiestos_iguales(a2.path(), b2.path()).expect("compara borrado"));
    }

    #[test]
    fn target_sin_target_base_no_reusa() {
        let (a, b) = par();
        assert!(!puede_reusar(a.path(), b.path(), None, None, None));
    }

    #[test]
    fn target_digests_distintos_no_reusan() {
        let (a, b) = par();
        let t = a.path().join("target-base");
        assert!(!puede_reusar(
            a.path(),
            b.path(),
            Some(&t),
            Some("sha:a"),
            Some("sha:b")
        ));
    }

    #[test]
    fn target_digest_que_aparece_o_desaparece_no_reusa() {
        let (a, b) = par();
        let t = a.path().join("target-base");
        assert!(!puede_reusar(
            a.path(),
            b.path(),
            Some(&t),
            Some("sha:a"),
            None
        ));
        assert!(!puede_reusar(
            a.path(),
            b.path(),
            Some(&t),
            None,
            Some("sha:a")
        ));
    }

    #[test]
    fn target_digests_none_y_none_cuentan_como_iguales() {
        let (a, b) = par();
        let t = a.path().join("target-base");
        assert!(puede_reusar(a.path(), b.path(), Some(&t), None, None));
    }

    #[test]
    fn target_todo_igual_reusa() {
        let (a, b) = par();
        let t = a.path().join("target-base");
        assert!(puede_reusar(
            a.path(),
            b.path(),
            Some(&t),
            Some("sha:a"),
            Some("sha:a")
        ));
    }

    #[test]
    fn target_manifiesto_distinto_no_reusa_aunque_el_digest_sea_igual() {
        let (a, b) = par();
        fs::write(b.path().join("Cargo.lock"), "version = 3\n").expect("cambia el lock");
        let t = a.path().join("target-base");
        assert!(!puede_reusar(
            a.path(),
            b.path(),
            Some(&t),
            Some("sha:a"),
            Some("sha:a")
        ));
    }
}
