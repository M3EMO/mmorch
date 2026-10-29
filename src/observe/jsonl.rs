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

/// Acceso de lectura a un log, con el lock compartido tomado. Lo entrega `con_lock_compartido`.
pub struct Lectura<'a> {
    ruta: &'a Path,
}

/// Acceso de escritura a un log, con el lock exclusivo tomado. Lo entrega `con_lock_exclusivo`.
pub struct Escritura<'a> {
    ruta: &'a Path,
}

/// Corre `f` con el lock compartido de `ruta` tomado una sola vez. `f` lista y lee todos los
/// archivos del log (rotados y vigente) sin volver a tomar el lock. Sin carpeta no hay log que
/// bloquear: `f` corre igual, con un log vacío.
pub fn con_lock_compartido<T, E: From<io::Error>>(
    ruta: &Path,
    f: impl FnOnce(&Lectura<'_>) -> Result<T, E>,
) -> Result<T, E> {
    let candado = match abrir_candado(ruta) {
        Ok(candado) => Some(candado),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.into()),
    };
    if let Some(candado) = &candado {
        FileExt::lock_shared(candado)?;
    }
    f(&Lectura { ruta })
}

/// Corre `f` con el lock exclusivo de `ruta` tomado una sola vez. Dentro, `f` puede leer y agregar
/// sin volver a tomar el lock: un segundo `flock` del mismo proceso se bloquearía a sí mismo.
pub fn con_lock_exclusivo<T, E: From<io::Error>>(
    ruta: &Path,
    f: impl FnOnce(&Escritura<'_>) -> Result<T, E>,
) -> Result<T, E> {
    if let Some(padre) = ruta.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(padre)?;
    }
    let candado = abrir_candado(ruta)?;
    FileExt::lock(&candado)?;
    f(&Escritura { ruta })
}

impl Lectura<'_> {
    /// Líneas no vacías de todos los archivos del log, en orden de escritura.
    pub fn lineas(&self) -> io::Result<Vec<String>> {
        lineas_de_todos(self.ruta)
    }
}

impl Escritura<'_> {
    /// Líneas no vacías de todos los archivos del log, en orden de escritura.
    pub fn lineas(&self) -> io::Result<Vec<String>> {
        lineas_de_todos(self.ruta)
    }

    /// Agrega `linea` y un `\n` al vigente. Si supera `umbral` bytes, lo rota antes de escribir.
    pub fn agregar(&self, linea: &str, umbral: u64) -> io::Result<()> {
        rotar_si_supera(self.ruta, umbral)?;
        let mut archivo = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.ruta)?;
        archivo.write_all(format!("{linea}\n").as_bytes())
    }
}

/// Líneas no vacías de los archivos de `ruta`, rotados y vigente. Debe llamarse con el lock tomado.
fn lineas_de_todos(ruta: &Path) -> io::Result<Vec<String>> {
    let mut lineas = Vec::new();
    for archivo in archivos_en_orden(ruta)? {
        lineas.extend(leer_lineas_sin_lock(&archivo)?);
    }
    Ok(lineas)
}

/// Líneas no vacías de un archivo. No toma el lock: lo debe tomar quien llama.
fn leer_lineas_sin_lock(ruta: &Path) -> io::Result<Vec<String>> {
    Ok(fs::read_to_string(ruta)?
        .lines()
        .filter(|linea| !linea.trim().is_empty())
        .map(str::to_string)
        .collect())
}

/// Agrega `linea` y un `\n` al archivo, con lock exclusivo.
/// Si el archivo supera `umbral` bytes, lo renombra a `<nombre>.<n>` antes de escribir.
pub fn agregar(ruta: &Path, linea: &str, umbral: u64) -> io::Result<()> {
    con_lock_exclusivo(ruta, |log| log.agregar(linea, umbral))
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

/// Archivos de un log en orden de escritura: los rotados `<nombre>.N` por N ascendente (el más
/// antiguo primero) y al final el vigente, si existe. Un directorio inexistente da una lista vacía.
pub fn archivos_en_orden(ruta: &Path) -> io::Result<Vec<PathBuf>> {
    let padre = ruta
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let entradas = match fs::read_dir(padre) {
        Ok(entradas) => entradas,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let nombre = ruta
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let prefijo = format!("{nombre}.");
    let mut rotados: Vec<(u32, PathBuf)> = Vec::new();
    for entrada in entradas {
        let entrada = entrada?;
        let otro = entrada.file_name().to_string_lossy().into_owned();
        let Some(sufijo) = otro.strip_prefix(&prefijo) else {
            continue;
        };
        if !sufijo.is_empty()
            && sufijo.bytes().all(|b| b.is_ascii_digit())
            && let Ok(numero) = sufijo.parse::<u32>()
        {
            rotados.push((numero, entrada.path()));
        }
    }
    rotados.sort_by_key(|(numero, _)| *numero);
    let mut archivos: Vec<PathBuf> = rotados.into_iter().map(|(_, ruta)| ruta).collect();
    if ruta.exists() {
        archivos.push(ruta.to_path_buf());
    }
    Ok(archivos)
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
    fn observe_archivos_en_orden_ordena_rotados_por_numero_y_pone_el_vigente_al_final() {
        let dir = TempDir::new().expect("temp dir");
        let ruta = dir.path().join("eventos.jsonl");
        for sufijo in ["10", "2", "1"] {
            fs::write(dir.path().join(format!("eventos.jsonl.{sufijo}")), "x\n").expect("rotado");
        }
        fs::write(dir.path().join("eventos.jsonl.lock"), "").expect("candado");
        fs::write(&ruta, "vigente\n").expect("vigente");
        let nombres: Vec<String> = archivos_en_orden(&ruta)
            .expect("lista")
            .iter()
            .map(|p| {
                p.file_name()
                    .expect("nombre")
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(
            nombres,
            vec![
                "eventos.jsonl.1",
                "eventos.jsonl.2",
                "eventos.jsonl.10",
                "eventos.jsonl"
            ]
        );
    }

    #[test]
    fn observe_archivos_en_orden_de_ruta_inexistente_da_lista_vacia() {
        let dir = TempDir::new().expect("temp dir");
        let lista = archivos_en_orden(&dir.path().join("no-existe.jsonl")).expect("no es error");
        assert!(lista.is_empty());
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

    #[test]
    fn observe_lectura_mantiene_el_candado_mientras_lee_todos_los_archivos() {
        let dir = TempDir::new().expect("temp dir");
        let ruta = dir.path().join("eventos.jsonl");
        agregar(&ruta, "aaaaaaaaaa", 10).expect("a");
        agregar(&ruta, "bbbbbbbbbb", 10).expect("b rota a .1");
        con_lock_compartido(&ruta, |log| -> io::Result<()> {
            assert_eq!(log.lineas()?, vec!["aaaaaaaaaa", "bbbbbbbbbb"]);
            // Otro descriptor no puede tomar el lock exclusivo: la lectura lo mantiene tomado.
            let otro = abrir_candado(&ruta)?;
            assert!(otro.try_lock().is_err(), "el lock debía seguir tomado");
            Ok(())
        })
        .expect("lectura");
    }
}
