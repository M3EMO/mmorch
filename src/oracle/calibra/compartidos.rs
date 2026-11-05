//! Veredictos de los casos limpios que comparten las dimensiones con juez (Tarea 63, punto 6). Un limpio es una
//! copia idéntica del repo bueno. Estructura, Arquitectura y Diseño la juzgan igual, así que cada una la
//! evaluaría de nuevo. Este caché guarda el veredicto una vez y las otras dimensiones lo reusan.
//!
//! La clave junta la tarea, la versión del repo bueno y las rúbricas. Cambiar cualquiera cambia la clave, y
//! entonces nada se reusa. Un archivo ilegible o corrupto cuenta como ausente. No se borra nada.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::goal::calcular_hash;
use crate::oracle::rubrica::cargar_embebidas;
use crate::oracle::tarea::Tarea;
use crate::oracle::{ErrorOraculo, infra};
use crate::tipos::{Dimension, Veredicto};

/// Carpeta dentro de `calibracion/` con los veredictos de los limpios.
const CARPETA: &str = "limpios";
/// Carpetas del repo que no entran en la versión: el `target` de Cargo y el `.git`.
const SALTADAS: [&str; 2] = ["target", ".git"];

/// Veredictos de los limpios de una tarea, con su clave. Cada caso va en `<clave>-<caso>.json`.
pub(super) struct Compartidos {
    carpeta: PathBuf,
    clave: String,
}

impl Compartidos {
    /// La clave de la tarea y de `repo_bueno` con las rúbricas embebidas. Lee todo el repo para la versión.
    pub(super) fn nuevo(
        home: &Path,
        tarea: &Tarea,
        repo_bueno: &Path,
    ) -> Result<Compartidos, ErrorOraculo> {
        let version = version_del_repo(repo_bueno)?;
        let rubricas = hash_de_rubricas()?;
        Ok(Compartidos {
            carpeta: home.join(super::CARPETA_CASOS).join(CARPETA),
            clave: format!("{}-{version}-{rubricas}", sanear(&tarea.nombre)),
        })
    }

    /// El veredicto guardado del caso `caso`. `Some` solo si parsea y trae un puntaje de `dimension`.
    pub(super) fn leer(&self, caso: u32, dimension: Dimension) -> Option<Veredicto> {
        let texto = fs::read_to_string(self.ruta(caso)).ok()?;
        let veredicto: Veredicto = serde_json::from_str(&texto).ok()?;
        super::puntaje_de(&veredicto, dimension)
            .is_some()
            .then_some(veredicto)
    }

    /// Guarda el veredicto del caso `caso`. Escribe a un temporal de la misma carpeta y lo renombra: un lector
    /// nunca ve un archivo a medio escribir.
    pub(super) fn guardar(&self, caso: u32, veredicto: &Veredicto) -> Result<(), ErrorOraculo> {
        fs::create_dir_all(&self.carpeta).map_err(|e| infra("crear la carpeta de limpios", e))?;
        let texto = serde_json::to_string(veredicto)
            .map_err(|e| ErrorOraculo::Infra(format!("serializar un limpio: {e}")))?;
        let mut temporal = tempfile::NamedTempFile::new_in(&self.carpeta)
            .map_err(|e| infra("temporal de un limpio", e))?;
        temporal
            .write_all(texto.as_bytes())
            .map_err(|e| infra("escribir un limpio", e))?;
        temporal
            .persist(self.ruta(caso))
            .map_err(|e| infra("guardar un limpio", e.error))?;
        Ok(())
    }

    fn ruta(&self, caso: u32) -> PathBuf {
        self.carpeta.join(format!("{}-{caso}.json", self.clave))
    }
}

/// Hash de todos los archivos del repo, en orden de ruta relativa. Cada archivo entra con su ruta y su tamaño.
fn version_del_repo(repo: &Path) -> Result<String, ErrorOraculo> {
    let mut archivos = Vec::new();
    recoger(repo, repo, &mut archivos)?;
    archivos.sort();
    let mut contenido = Vec::new();
    for (relativa, ruta) in &archivos {
        let bytes = fs::read(ruta).map_err(|e| infra("leer un archivo del repo", e))?;
        contenido.extend_from_slice(relativa.as_bytes());
        contenido.push(0);
        contenido.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        contenido.extend_from_slice(&bytes);
    }
    Ok(calcular_hash(&contenido))
}

/// Junta `(ruta relativa, ruta absoluta)` de los archivos de `dir`, sin `SALTADAS`. Los enlaces simbólicos no entran.
fn recoger(
    raiz: &Path,
    dir: &Path,
    salida: &mut Vec<(String, PathBuf)>,
) -> Result<(), ErrorOraculo> {
    let entradas = fs::read_dir(dir).map_err(|e| infra("leer el repo bueno", e))?;
    for entrada in entradas {
        let entrada = entrada.map_err(|e| infra("leer el repo bueno", e))?;
        let tipo = entrada
            .file_type()
            .map_err(|e| infra("leer el repo bueno", e))?;
        let ruta = entrada.path();
        if tipo.is_dir() {
            if SALTADAS.iter().any(|s| entrada.file_name() == *s) {
                continue;
            }
            recoger(raiz, &ruta, salida)?;
        } else if tipo.is_file() {
            let relativa = ruta
                .strip_prefix(raiz)
                .map_err(|_| ErrorOraculo::Infra("ruta fuera del repo bueno".into()))?
                .to_string_lossy()
                .replace('\\', "/");
            salida.push((relativa, ruta));
        }
    }
    Ok(())
}

/// Hash de las rúbricas embebidas, en su orden. Una rúbrica cambiada cambia la clave.
fn hash_de_rubricas() -> Result<String, ErrorOraculo> {
    let rubricas =
        cargar_embebidas().map_err(|e| ErrorOraculo::Infra(format!("rúbricas embebidas: {e}")))?;
    let juntas: String = rubricas.iter().map(|r| r.hash()).collect();
    Ok(calcular_hash(juntas.as_bytes()))
}

/// La tarea como nombre de archivo: letras, dígitos, `-` y `_`. Lo demás pasa a `_`.
fn sanear(nombre: &str) -> String {
    nombre
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_del_repo_ignora_target_y_git() {
        let repo = tempfile::tempdir().expect("repo");
        fs::write(repo.path().join("lib.rs"), "fn f() {}").expect("lib");
        let antes = version_del_repo(repo.path()).expect("versión");
        for carpeta in ["target", ".git"] {
            fs::create_dir_all(repo.path().join(carpeta)).expect("carpeta");
            fs::write(repo.path().join(carpeta).join("x"), "otro").expect("archivo");
        }
        assert_eq!(version_del_repo(repo.path()).expect("versión"), antes);
    }

    #[test]
    fn version_del_repo_cambia_con_el_contenido_y_con_la_ruta() {
        let repo = tempfile::tempdir().expect("repo");
        fs::write(repo.path().join("lib.rs"), "fn f() {}").expect("lib");
        let antes = version_del_repo(repo.path()).expect("versión");
        fs::write(repo.path().join("lib.rs"), "fn g() {}").expect("cambia");
        let contenido = version_del_repo(repo.path()).expect("versión");
        assert_ne!(contenido, antes, "cambiar el contenido cambia la versión");
        fs::rename(repo.path().join("lib.rs"), repo.path().join("otro.rs")).expect("renombra");
        assert_ne!(
            version_del_repo(repo.path()).expect("versión"),
            contenido,
            "cambiar la ruta cambia la versión"
        );
    }
}
