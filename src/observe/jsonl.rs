//! Append de una línea JSON con lock exclusivo y rotación por tamaño.
//!
//! El lock vive en un archivo hermano `<nombre>.lock`. Así la rotación no cambia
//! el archivo que otro proceso espera bloquear.

use std::borrow::Cow;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use fs4::FileExt;

/// Umbral por defecto de rotación: 50 MB.
pub const UMBRAL_POR_DEFECTO: u64 = 50 * 1024 * 1024;

/// Agrega `linea` y un `\n` al archivo, con lock exclusivo.
/// Si el archivo supera `umbral` bytes, lo renombra a `<nombre>.<n>` antes de escribir.
pub fn agregar(ruta: &Path, linea: &str, umbral: u64) -> io::Result<()> {
    if let Some(padre) = ruta.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(padre)?;
    }
    let candado = abrir_candado(ruta)?;
    FileExt::lock(&candado)?;
    rotar_si_supera(ruta, umbral)?;
    let mut archivo = OpenOptions::new().create(true).append(true).open(ruta)?;
    archivo.write_all(format!("{linea}\n").as_bytes())
}

/// Abre el archivo de lock hermano. Se libera al cerrarse el `File`.
fn abrir_candado(ruta: &Path) -> io::Result<File> {
    OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(ruta_hermana(ruta, ".lock"))
}

/// Renombra el archivo a `<nombre>.<n>` si supera el umbral. Debe llamarse con el lock tomado.
fn rotar_si_supera(ruta: &Path, umbral: u64) -> io::Result<()> {
    let tamano = match fs::metadata(ruta) {
        Ok(metadatos) => metadatos.len(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    if tamano > umbral {
        fs::rename(ruta, primera_rotada_libre(ruta))?;
    }
    Ok(())
}

fn primera_rotada_libre(ruta: &Path) -> PathBuf {
    (1u32..)
        .map(|n| ruta_hermana(ruta, &format!(".{n}")))
        .find(|candidata| !candidata.exists())
        .expect("siempre hay un número libre")
}

/// Ruta vecina de `ruta`: su nombre más `sufijo`.
fn ruta_hermana(ruta: &Path, sufijo: &str) -> PathBuf {
    let nombre: Cow<str> = ruta
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_default();
    ruta.with_file_name(format!("{nombre}{sufijo}"))
}

/// Lee las líneas no vacías del archivo, con lock compartido. Si no existe, da una lista vacía.
pub fn leer_lineas(ruta: &Path) -> io::Result<Vec<String>> {
    Ok(leer_texto(ruta)?
        .lines()
        .filter(|linea| !linea.trim().is_empty())
        .map(str::to_string)
        .collect())
}

/// Texto completo del archivo, con lock compartido. Si no existe, da un texto vacío.
pub fn leer_texto(ruta: &Path) -> io::Result<String> {
    if !ruta.exists() {
        return Ok(String::new());
    }
    let candado = abrir_candado(ruta)?;
    FileExt::lock_shared(&candado)?;
    fs::read_to_string(ruta)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn observe_agregar_crea_directorio_y_escribe_lineas_en_orden() {
        let dir = TempDir::new().expect("temp dir");
        let ruta = dir.path().join("anidado/eventos.jsonl");
        agregar(&ruta, r#"{"a":1}"#, UMBRAL_POR_DEFECTO).expect("primera escritura");
        agregar(&ruta, r#"{"a":2}"#, UMBRAL_POR_DEFECTO).expect("segunda escritura");
        let texto = fs::read_to_string(&ruta).expect("lee el archivo");
        assert_eq!(texto, "{\"a\":1}\n{\"a\":2}\n");
    }

    #[test]
    fn observe_rotacion_con_umbral_chico_crea_archivo_rotado() {
        let dir = TempDir::new().expect("temp dir");
        let ruta = dir.path().join("eventos.jsonl");
        agregar(&ruta, "aaaaaaaaaa", 10).expect("escribe la primera");
        agregar(&ruta, "bbbbbbbbbb", 10).expect("rota y escribe la segunda");
        let rotado = dir.path().join("eventos.jsonl.1");
        assert_eq!(
            fs::read_to_string(&rotado).expect("existe el rotado"),
            "aaaaaaaaaa\n"
        );
        assert_eq!(
            fs::read_to_string(&ruta).expect("existe el activo"),
            "bbbbbbbbbb\n"
        );
    }

    #[test]
    fn observe_rotacion_numera_sin_pisar_rotados_previos() {
        let dir = TempDir::new().expect("temp dir");
        let ruta = dir.path().join("eventos.jsonl");
        agregar(&ruta, "aaaaaaaaaa", 10).expect("a");
        agregar(&ruta, "bbbbbbbbbb", 10).expect("b rota a .1");
        agregar(&ruta, "cccccccccc", 10).expect("c rota a .2");
        assert_eq!(
            fs::read_to_string(dir.path().join("eventos.jsonl.1")).expect("1"),
            "aaaaaaaaaa\n"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("eventos.jsonl.2")).expect("2"),
            "bbbbbbbbbb\n"
        );
        assert_eq!(fs::read_to_string(&ruta).expect("activo"), "cccccccccc\n");
    }

    #[test]
    fn observe_leer_lineas_de_archivo_inexistente_da_lista_vacia() {
        let dir = TempDir::new().expect("temp dir");
        let lineas = leer_lineas(&dir.path().join("no-existe.jsonl")).expect("no es error");
        assert!(lineas.is_empty());
    }

    #[test]
    fn observe_leer_lineas_omite_lineas_vacias() {
        let dir = TempDir::new().expect("temp dir");
        let ruta = dir.path().join("eventos.jsonl");
        agregar(&ruta, "uno", UMBRAL_POR_DEFECTO).expect("escribe");
        agregar(&ruta, "dos", UMBRAL_POR_DEFECTO).expect("escribe");
        assert_eq!(leer_lineas(&ruta).expect("lee"), vec!["uno", "dos"]);
    }
}
