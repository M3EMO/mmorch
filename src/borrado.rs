//! Borrados con guarda: solo tocan rutas dentro de una raíz (`ROOT_HOME` o una de sus carpetas).

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

/// Borra el directorio `ruta` si está dentro de `raiz`. Un enlace simbólico se borra solo como enlace.
/// Un archivo suelto no es directorio y se rechaza. `NotFound` es éxito.
pub fn borrar_dir_dentro(raiz: &Path, ruta: &Path) -> io::Result<()> {
    let Some(destino) = dentro(raiz, ruta)? else {
        return Ok(());
    };
    let meta = match fs::symlink_metadata(&destino) {
        Ok(meta) => meta,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    if meta.is_symlink() {
        fs::remove_file(&destino)
    } else if meta.is_dir() {
        fs::remove_dir_all(&destino)
    } else {
        Err(invalida("no es un directorio", ruta))
    }
}

/// Borra el archivo o enlace `ruta` si está dentro de `raiz`. Nunca sigue el último componente.
/// Un directorio no se borra aquí. `NotFound` es éxito.
pub fn borrar_archivo_dentro(raiz: &Path, ruta: &Path) -> io::Result<()> {
    let Some(destino) = dentro(raiz, ruta)? else {
        return Ok(());
    };
    match fs::symlink_metadata(&destino) {
        Ok(meta) if meta.is_dir() => Err(invalida("es un directorio", ruta)),
        Ok(_) => fs::remove_file(&destino),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Dónde está `ruta` dentro de `raiz`, con el padre ya resuelto. `None` si no existe el padre.
/// Rechaza, sin tocar nada, una raíz inexistente o `/`, una ruta igual a la raíz y una ruta fuera de ella.
fn dentro(raiz: &Path, ruta: &Path) -> io::Result<Option<PathBuf>> {
    let raiz_fisica = fs::canonicalize(raiz).map_err(|_| invalida("la raíz no existe", raiz))?;
    if raiz_fisica.parent().is_none() {
        return Err(invalida("la raíz es /", raiz));
    }
    let raiz_logica =
        normalizar(&absoluta(raiz)?).ok_or_else(|| invalida("raíz inválida", raiz))?;
    let normal = normalizar(&absoluta(ruta)?).ok_or_else(|| invalida("ruta fuera de /", ruta))?;
    let dentro_logico = normal.starts_with(&raiz_logica) || normal.starts_with(&raiz_fisica);
    if !dentro_logico || normal == raiz_logica || normal == raiz_fisica {
        return Err(invalida("fuera de la raíz o igual a ella", ruta));
    }
    let Some(nombre) = normal.file_name() else {
        return Err(invalida("la ruta no tiene nombre", ruta));
    };
    let Some(padre) = normal.parent() else {
        return Err(invalida("la ruta no tiene padre", ruta));
    };
    // El padre se resuelve con enlaces; el último componente no, para borrar el enlace y no su destino.
    let padre_fisico = match fs::canonicalize(padre) {
        Ok(p) => p,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let destino = padre_fisico.join(nombre);
    if !destino.starts_with(&raiz_fisica) || destino == raiz_fisica {
        return Err(invalida("un enlace del camino sale de la raíz", ruta));
    }
    Ok(Some(destino))
}

fn absoluta(ruta: &Path) -> io::Result<PathBuf> {
    if ruta.is_absolute() {
        Ok(ruta.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(ruta))
    }
}

/// Resuelve `.` y `..` sin tocar el disco. `None` si un `..` sale de la raíz del sistema.
fn normalizar(ruta: &Path) -> Option<PathBuf> {
    let mut salida = PathBuf::new();
    for componente in ruta.components() {
        match componente {
            Component::Prefix(_) | Component::RootDir => salida.push(componente.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !salida.pop() {
                    return None;
                }
            }
            Component::Normal(nombre) => salida.push(nombre),
        }
    }
    Some(salida)
}

fn invalida(motivo: &str, ruta: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("borrado rechazado ({motivo}): {}", ruta.display()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;

    /// Carpeta base con `root/` (la raíz) y `fuera/` con un archivo `dato`.
    fn escenario() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let base = tempfile::tempdir().expect("temp dir");
        let raiz = base.path().join("root");
        let fuera = base.path().join("fuera");
        fs::create_dir_all(&raiz).expect("raiz");
        fs::create_dir_all(&fuera).expect("fuera");
        fs::write(fuera.join("dato"), "x").expect("dato");
        (base, raiz, fuera)
    }

    #[test]
    fn borrado_dir_dentro_de_la_raiz_se_borra_con_su_arbol() {
        let (_base, raiz, _fuera) = escenario();
        let sub = raiz.join("sub");
        fs::create_dir_all(sub.join("anidado")).expect("dir");
        fs::write(sub.join("anidado").join("a"), "x").expect("archivo");
        borrar_dir_dentro(&raiz, &sub).expect("dentro de la raíz se borra");
        assert!(!sub.exists());
        assert!(raiz.is_dir(), "la raíz no se toca");
    }

    #[test]
    fn borrado_dir_fuera_de_la_raiz_se_rechaza_sin_borrar() {
        let (_base, raiz, fuera) = escenario();
        let error = borrar_dir_dentro(&raiz, &fuera).expect_err("fuera de la raíz");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(fuera.join("dato").exists(), "el destino queda intacto");
    }

    #[test]
    fn borrado_dir_con_punto_punto_que_sale_se_rechaza() {
        let (_base, raiz, fuera) = escenario();
        let ruta = raiz.join("..").join("fuera");
        let error = borrar_dir_dentro(&raiz, &ruta).expect_err("sale con ..");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(fuera.join("dato").exists());
    }

    #[test]
    fn borrado_ruta_que_sale_por_un_enlace_del_camino_se_rechaza() {
        let (_base, raiz, fuera) = escenario();
        symlink(&fuera, raiz.join("salida")).expect("enlace de camino");
        let error = borrar_dir_dentro(&raiz, &raiz.join("salida").join("dato"))
            .expect_err("el camino pasa por un enlace que sale");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(fuera.join("dato").exists());
    }

    #[test]
    fn borrado_ruta_igual_a_raiz_y_raiz_raiz_se_rechazan() {
        let (base, raiz, _fuera) = escenario();
        let error = borrar_dir_dentro(&raiz, &raiz).expect_err("ruta == raíz");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(raiz.is_dir());
        let error = borrar_dir_dentro(Path::new("/"), base.path()).expect_err("raíz /");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        let error = borrar_dir_dentro(Path::new("/"), Path::new("/")).expect_err("raíz /");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(base.path().is_dir());
    }

    #[test]
    fn borrado_raiz_inexistente_se_rechaza() {
        let (base, _raiz, _fuera) = escenario();
        let inexistente = base.path().join("no-hay");
        let error =
            borrar_dir_dentro(&inexistente, &inexistente.join("x")).expect_err("la raíz no existe");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn borrado_enlace_dentro_a_directorio_fuera_borra_solo_el_enlace() {
        let (_base, raiz, fuera) = escenario();
        let enlace = raiz.join("enlace");
        symlink(&fuera, &enlace).expect("enlace");
        borrar_dir_dentro(&raiz, &enlace).expect("borra el enlace");
        assert!(fs::symlink_metadata(&enlace).is_err(), "el enlace se borra");
        assert_eq!(
            fs::read_to_string(fuera.join("dato")).expect("destino"),
            "x",
            "el directorio destino y su contenido quedan intactos"
        );
    }

    #[test]
    fn borrado_enlace_a_archivo_borra_solo_el_enlace() {
        let (_base, raiz, fuera) = escenario();
        let enlace = raiz.join("enlace-archivo");
        symlink(fuera.join("dato"), &enlace).expect("enlace");
        borrar_archivo_dentro(&raiz, &enlace).expect("borra el enlace");
        assert!(fs::symlink_metadata(&enlace).is_err());
        assert_eq!(
            fs::read_to_string(fuera.join("dato")).expect("destino"),
            "x"
        );
    }

    #[test]
    fn borrado_enlace_a_directorio_con_borrar_dir_solo_borra_el_enlace() {
        let (_base, raiz, fuera) = escenario();
        let enlace = raiz.join("enlace");
        symlink(&fuera, &enlace).expect("enlace");
        borrar_dir_dentro(&raiz, &enlace).expect("borra el enlace");
        assert!(fs::symlink_metadata(&enlace).is_err());
        assert!(fuera.join("dato").exists());
    }

    #[test]
    fn borrado_no_encontrado_dentro_es_exito_y_fuera_se_rechaza() {
        let (base, raiz, _fuera) = escenario();
        borrar_dir_dentro(&raiz, &raiz.join("no-existe")).expect("NotFound es éxito");
        borrar_archivo_dentro(&raiz, &raiz.join("no-existe.txt")).expect("NotFound es éxito");
        let error = borrar_dir_dentro(&raiz, &base.path().join("no-existe"))
            .expect_err("fuera se rechaza aunque no exista");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn borrado_archivo_dentro_borra_y_rechaza_fuera() {
        let (_base, raiz, fuera) = escenario();
        let dentro = raiz.join("a.txt");
        fs::write(&dentro, "x").expect("archivo");
        borrar_archivo_dentro(&raiz, &dentro).expect("dentro se borra");
        assert!(!dentro.exists());
        let error = borrar_archivo_dentro(&raiz, &fuera.join("dato")).expect_err("fuera");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(fuera.join("dato").exists());
    }
}
