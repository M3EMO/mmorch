//! Inyectores de defectos de la calibración (Tarea 12). Cada inyector toma una copia de un repo Rust y una
//! semilla, y aplica UN defecto por construcción. La semilla elige el archivo, la función o la línea con un
//! PRNG propio (splitmix64). Con la misma copia y la misma semilla, el cambio es el mismo.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;

use crate::borrado;
use crate::tipos::Dimension;

/// Defecto que se inyecta en una copia. Cada uno apunta a una dimensión y a un ítem de rúbrica.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Defecto {
    SecretoEnCodigo,
    PanicoEnEntrada,
    BucleCuadratico,
    CicloEntreCapas,
    ArchivoDios,
    ModuloBorrado,
    ErrorTragado,
    AssertBorrado,
    DockerfileRoot,
}

/// Todos los defectos, en el orden de `Defecto`.
pub const TODOS: [Defecto; 9] = [
    Defecto::SecretoEnCodigo,
    Defecto::PanicoEnEntrada,
    Defecto::BucleCuadratico,
    Defecto::CicloEntreCapas,
    Defecto::ArchivoDios,
    Defecto::ModuloBorrado,
    Defecto::ErrorTragado,
    Defecto::AssertBorrado,
    Defecto::DockerfileRoot,
];

impl Defecto {
    /// Dimensión que el defecto debe hacer caer.
    pub fn dimension(self) -> Dimension {
        match self {
            Defecto::SecretoEnCodigo | Defecto::PanicoEnEntrada => Dimension::Seguridad,
            Defecto::BucleCuadratico => Dimension::Eficiencia,
            Defecto::CicloEntreCapas => Dimension::Arquitectura,
            Defecto::ArchivoDios => Dimension::Estructura,
            Defecto::ModuloBorrado => Dimension::Completitud,
            Defecto::ErrorTragado => Dimension::Diseno,
            Defecto::AssertBorrado => Dimension::Pruebas,
            Defecto::DockerfileRoot => Dimension::Entrega,
        }
    }

    /// Ítem de rúbrica que el defecto debe hacer caer, por ejemplo `seguridad.secretos`.
    pub fn item(self) -> &'static str {
        match self {
            Defecto::SecretoEnCodigo => "seguridad.secretos",
            Defecto::PanicoEnEntrada => "seguridad.panico_en_entrada",
            Defecto::BucleCuadratico => "eficiencia.tiempo",
            Defecto::CicloEntreCapas => "arquitectura.dependencias",
            Defecto::ArchivoDios => "estructura.tamano",
            Defecto::ModuloBorrado => "completitud.alcance",
            Defecto::ErrorTragado => "diseno.errores",
            Defecto::AssertBorrado => "pruebas.mutacion",
            Defecto::DockerfileRoot => "entrega.contenedor",
        }
    }
}

/// Defectos que sirven para una dimensión, en el orden en que `calibrar` los rota. Vacío si no hay ninguno.
pub fn de_dimension(dimension: Dimension) -> Vec<Defecto> {
    TODOS
        .into_iter()
        .filter(|d| d.dimension() == dimension)
        .collect()
}

/// Un defecto aplicado: dónde quedó. `linea` es 1-based; 0 significa el archivo entero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inyeccion {
    pub defecto: Defecto,
    /// Ruta relativa a la raíz de la copia, con `/`.
    pub archivo: String,
    pub linea: u32,
}

/// Aplica `defecto` a la copia de repo Rust en `dir`. `capas` son las de `[arquitectura]` de la tarea.
/// `Ok(None)` si el defecto no se puede aplicar a ese repo: no hay Dockerfile, no hay tests, etc.
pub fn inyectar(
    defecto: Defecto,
    dir: &Path,
    capas: &[Vec<String>],
    semilla: u64,
) -> io::Result<Option<Inyeccion>> {
    inyectar_con(defecto, dir, capas, &[], semilla)
}

/// Como `inyectar`, con `camino_caliente` (Tarea 12, desempate 42). Con la lista no vacía, el bucle O(n²)
/// cae solo al inicio de una de esas funciones, elegida con la semilla entre las que existen en la copia.
/// Si ninguna existe, `Ok(None)`. Los demás defectos no usan la lista.
pub fn inyectar_con(
    defecto: Defecto,
    dir: &Path,
    capas: &[Vec<String>],
    camino_caliente: &[String],
    semilla: u64,
) -> io::Result<Option<Inyeccion>> {
    let mut rng = Prng::nuevo(semilla);
    let lugar = match defecto {
        Defecto::SecretoEnCodigo => secreto(dir, &mut rng)?,
        Defecto::PanicoEnEntrada => panico(dir, &mut rng)?,
        Defecto::BucleCuadratico => bucle(dir, camino_caliente, &mut rng)?,
        Defecto::CicloEntreCapas => ciclo(dir, capas, &mut rng)?,
        Defecto::ArchivoDios => dios(dir, &mut rng)?,
        Defecto::ModuloBorrado => modulo_borrado(dir, &mut rng)?,
        Defecto::ErrorTragado => error_tragado(dir, &mut rng)?,
        Defecto::AssertBorrado => assert_borrado(dir, &mut rng)?,
        Defecto::DockerfileRoot => dockerfile(dir)?,
    };
    Ok(lugar.map(|(archivo, linea)| Inyeccion {
        defecto,
        archivo,
        linea,
    }))
}

/// Archivo y línea (1-based) donde quedó el defecto, o `None` si no se pudo aplicar.
type Lugar = Option<(String, u32)>;

/// PRNG determinista (splitmix64). Sin crates: la misma semilla da la misma secuencia.
struct Prng(u64);

impl Prng {
    fn nuevo(semilla: u64) -> Prng {
        Prng(semilla)
    }

    fn siguiente(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Un índice de `0..n`. `n` debe ser mayor que cero.
    fn indice(&mut self, n: usize) -> usize {
        (self.siguiente() % n as u64) as usize
    }

    /// Un elemento de `lista` elegido por la semilla. `None` si la lista está vacía.
    fn elegir<'a, T>(&mut self, lista: &'a [T]) -> Option<&'a T> {
        if lista.is_empty() {
            None
        } else {
            lista.get(self.indice(lista.len()))
        }
    }
}

/// Partes de un texto por `\n`. Unirlas de nuevo con `unir` da el texto original, con su último salto.
fn partir(texto: &str) -> Vec<String> {
    texto.split('\n').map(str::to_string).collect()
}

fn unir(lineas: &[String]) -> String {
    lineas.join("\n")
}

fn leer(dir: &Path, relativa: &str) -> io::Result<String> {
    fs::read_to_string(dir.join(relativa))
}

fn escribir(dir: &Path, relativa: &str, texto: &str) -> io::Result<()> {
    fs::write(dir.join(relativa), texto)
}

/// Número de línea (1-based) de la primera línea que contiene `patron`. 0 si no aparece.
fn linea_de(texto: &str, patron: &str) -> u32 {
    texto
        .lines()
        .position(|l| l.contains(patron))
        .map_or(0, |i| i as u32 + 1)
}

/// Líneas de producción: las de antes del primer `#[cfg(test)]`. El resto es de tests.
fn limite_de_produccion(lineas: &[String]) -> usize {
    lineas
        .iter()
        .position(|l| l.trim() == "#[cfg(test)]")
        .unwrap_or(lineas.len())
}

/// Archivos `.rs` bajo `carpeta` (relativa a `dir`), con `/`, en orden de ruta. Vacío si no existe.
fn rust_en(dir: &Path, carpeta: &str) -> io::Result<Vec<String>> {
    let mut salida = Vec::new();
    recorrer(dir, &dir.join(carpeta), &mut salida)?;
    salida.sort();
    Ok(salida)
}

fn recorrer(raiz: &Path, actual: &Path, salida: &mut Vec<String>) -> io::Result<()> {
    if !actual.is_dir() {
        return Ok(());
    }
    for entrada in fs::read_dir(actual)? {
        let ruta = entrada?.path();
        if ruta.is_dir() {
            recorrer(raiz, &ruta, salida)?;
        } else if ruta.extension().is_some_and(|e| e == "rs") {
            let relativa = ruta.strip_prefix(raiz).unwrap_or(&ruta);
            salida.push(relativa.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

/// Un identificador de Rust sin ruta: letras, dígitos y `_`, sin empezar por dígito.
fn es_identificador(texto: &str) -> bool {
    texto
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && texto.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Clave con forma de AWS (`AKIA` y 16 caracteres) sacada de la semilla. No es una credencial.
fn clave_sintetica(rng: &mut Prng) -> String {
    const ALFABETO: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let resto: String = (0..16)
        .map(|_| ALFABETO[rng.indice(ALFABETO.len())] as char)
        .collect();
    format!("AKIA{resto}")
}

/// Agrega una constante con una clave sintética al final de un archivo de `src/`.
fn secreto(dir: &Path, rng: &mut Prng) -> io::Result<Lugar> {
    let archivos = rust_en(dir, "src")?;
    let Some(archivo) = rng.elegir(&archivos).cloned() else {
        return Ok(None);
    };
    let clave = clave_sintetica(rng);
    let mut texto = leer(dir, &archivo)?;
    texto.push_str(&format!(
        "\n/// Clave sintética de la calibración.\npub const CLAVE_CALIBRACION: &str = \"{clave}\";\n"
    ));
    escribir(dir, &archivo, &texto)?;
    Ok(Some((
        archivo,
        linea_de(&texto, "pub const CLAVE_CALIBRACION"),
    )))
}

/// Tokens que buscan una llamada de parseo de entrada.
const TOKENS_DE_PARSEO: [&str; 3] = [".parse", "from_utf8", "from_str"];

/// Posición del primer token de parseo de `linea`, si tiene alguno.
fn posicion_de_parseo(linea: &str) -> Option<usize> {
    TOKENS_DE_PARSEO
        .iter()
        .filter_map(|token| linea.find(token))
        .min()
}

/// Cambia el `?` de una línea que parsea entrada por `.unwrap()`. El código sigue compilando.
fn panico(dir: &Path, rng: &mut Prng) -> io::Result<Lugar> {
    let mut candidatos: Vec<(String, usize)> = Vec::new();
    for archivo in rust_en(dir, "src")? {
        let lineas = partir(&leer(dir, &archivo)?);
        let limite = limite_de_produccion(&lineas);
        for (i, linea) in lineas[..limite].iter().enumerate() {
            if posicion_de_parseo(linea).is_some_and(|p| linea[p..].contains('?')) {
                candidatos.push((archivo.clone(), i));
            }
        }
    }
    let Some((archivo, i)) = rng.elegir(&candidatos).cloned() else {
        return Ok(None);
    };
    let mut lineas = partir(&leer(dir, &archivo)?);
    let Some(p) = posicion_de_parseo(&lineas[i]) else {
        return Ok(None);
    };
    let Some(q) = lineas[i][p..].find('?').map(|q| p + q) else {
        return Ok(None);
    };
    lineas[i].replace_range(q..q + 1, ".unwrap()");
    escribir(dir, &archivo, &unir(&lineas))?;
    Ok(Some((archivo, i as u32 + 1)))
}

/// Sentencia de un bucle O(n²) que se inserta al inicio de una función.
const BUCLE: &str =
    "let _ = (0..2_000u32).map(|i| (0..2_000u32).filter(|j| *j == i).count()).sum::<usize>();";

/// Nombre de la función que declara `linea` (`pub fn procesar(` da `procesar`). `None` si no declara una.
fn nombre_de_funcion(linea: &str) -> Option<&str> {
    let t = linea.trim_start();
    let resto = ["pub(crate) fn ", "pub fn ", "fn "]
        .iter()
        .find_map(|p| t.strip_prefix(p))?;
    let fin = resto
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(resto.len());
    let nombre = &resto[..fin];
    es_identificador(nombre).then_some(nombre)
}

/// `true` si `linea` puede recibir el bucle: con `camino` vacío, cualquiera; si no, una función de la lista.
fn en_camino(linea: &str, camino: &[String]) -> bool {
    camino.is_empty() || nombre_de_funcion(linea).is_some_and(|n| camino.iter().any(|c| c == n))
}

/// Inserta un bucle cuadrático al inicio de una función de producción. Con `camino`, solo de las listadas.
fn bucle(dir: &Path, camino: &[String], rng: &mut Prng) -> io::Result<Lugar> {
    let mut candidatos: Vec<(String, usize)> = Vec::new();
    for archivo in rust_en(dir, "src")? {
        let lineas = partir(&leer(dir, &archivo)?);
        let limite = limite_de_produccion(&lineas);
        for (i, linea) in lineas[..limite].iter().enumerate() {
            let t = linea.trim_start();
            let es_funcion = ["pub fn ", "fn ", "pub(crate) fn "]
                .iter()
                .any(|p| t.starts_with(p));
            if es_funcion
                && linea.trim_end().ends_with('{')
                && !t.contains("const fn")
                && en_camino(linea, camino)
            {
                candidatos.push((archivo.clone(), i));
            }
        }
    }
    let Some((archivo, i)) = rng.elegir(&candidatos).cloned() else {
        return Ok(None);
    };
    let mut lineas = partir(&leer(dir, &archivo)?);
    let sangria: String = lineas[i]
        .chars()
        .take_while(|c| c.is_whitespace())
        .collect();
    lineas.insert(i + 1, format!("{sangria}    {BUCLE}"));
    escribir(dir, &archivo, &unir(&lineas))?;
    Ok(Some((archivo, i as u32 + 2)))
}

/// Archivo de un módulo de primer nivel: `src/<m>.rs` o `src/<m>/mod.rs`. `None` si no existe.
fn archivo_de_modulo(dir: &Path, modulo: &str) -> Option<String> {
    if !es_identificador(modulo) {
        return None;
    }
    [format!("src/{modulo}.rs"), format!("src/{modulo}/mod.rs")]
        .into_iter()
        .find(|relativa| dir.join(relativa).is_file())
}

/// Un `use` de una capa anterior en un módulo de una capa posterior: el ciclo entre capas.
fn ciclo(dir: &Path, capas: &[Vec<String>], rng: &mut Prng) -> io::Result<Lugar> {
    if capas.len() < 2 {
        return Ok(None);
    }
    // (archivo de la capa posterior, módulo de una capa anterior que se importa)
    let mut candidatos: Vec<(String, String)> = Vec::new();
    for (indice, capa) in capas.iter().enumerate().skip(1) {
        for modulo in capa {
            let Some(archivo) = archivo_de_modulo(dir, modulo) else {
                continue;
            };
            for superior in capas[..indice].iter().flatten() {
                if archivo_de_modulo(dir, superior).is_some() {
                    candidatos.push((archivo.clone(), superior.clone()));
                }
            }
        }
    }
    let Some((archivo, superior)) = rng.elegir(&candidatos).cloned() else {
        return Ok(None);
    };
    let mut texto = leer(dir, &archivo)?;
    let uso = format!("use crate::{superior} as _;");
    texto.push_str(&format!("\n#[allow(unused_imports)]\n{uso}\n"));
    escribir(dir, &archivo, &texto)?;
    Ok(Some((archivo, linea_de(&texto, &uso))))
}

/// Concatena los otros módulos de primer nivel dentro de un archivo de `src/`, como módulos en línea.
/// Los archivos originales quedan en la copia: lo que crece es el archivo destino.
fn dios(dir: &Path, rng: &mut Prng) -> io::Result<Lugar> {
    // (ruta, nombre del módulo, contenido) de cada archivo de primer nivel que puede entrar o recibir.
    let mut modulos: Vec<(String, String, String)> = Vec::new();
    for archivo in rust_en(dir, "src")? {
        let nombre = archivo.trim_start_matches("src/");
        if nombre.contains('/') || nombre == "lib.rs" || nombre == "main.rs" {
            continue;
        }
        let stem = nombre.trim_end_matches(".rs");
        if !es_identificador(stem) {
            continue;
        }
        let texto = leer(dir, &archivo)?;
        modulos.push((archivo.clone(), stem.to_string(), texto));
    }
    let Some(destino) = rng.elegir(&modulos).map(|m| m.0.clone()) else {
        return Ok(None);
    };
    // Un módulo con `mod x;` apunta a archivos hijos: no se pasa a línea.
    let fuentes: Vec<&(String, String, String)> = modulos
        .iter()
        .filter(|m| m.0 != destino)
        .filter(|m| !m.2.lines().any(|l| nombre_de_declaracion(l).is_some()))
        .collect();
    if fuentes.is_empty() {
        return Ok(None);
    }
    let mut texto = leer(dir, &destino)?;
    for (_, stem, contenido) in fuentes {
        texto.push_str(&format!("\npub mod concat_{stem} {{\n{contenido}\n}}\n"));
    }
    escribir(dir, &destino, &texto)?;
    Ok(Some((destino, linea_de(&texto, "pub mod concat_"))))
}

/// Nombre de `mod x;` o `pub mod x;` (con `pub(crate)` también). `None` si la línea no declara un archivo.
fn nombre_de_declaracion(linea: &str) -> Option<String> {
    let t = linea.trim();
    let t = ["pub(crate) ", "pub "]
        .iter()
        .find_map(|p| t.strip_prefix(p))
        .unwrap_or(t);
    let nombre = t.strip_prefix("mod ")?.strip_suffix(';')?.trim();
    es_identificador(nombre).then(|| nombre.to_string())
}

/// Borra un módulo declarado en `lib.rs` o `main.rs`, con su archivo y su declaración.
fn modulo_borrado(dir: &Path, rng: &mut Prng) -> io::Result<Lugar> {
    let raices = ["src/lib.rs", "src/main.rs"];
    let mut declarados: BTreeSet<String> = BTreeSet::new();
    for raiz in raices {
        if dir.join(raiz).is_file() {
            for linea in leer(dir, raiz)?.lines() {
                declarados.extend(nombre_de_declaracion(linea));
            }
        }
    }
    let candidatos: Vec<String> = declarados
        .into_iter()
        .filter(|m| dir.join(format!("src/{m}.rs")).is_file())
        .collect();
    let Some(modulo) = rng.elegir(&candidatos).cloned() else {
        return Ok(None);
    };
    let archivo = format!("src/{modulo}.rs");
    borrado::borrar_archivo_dentro(dir, &dir.join(&archivo))?;
    for raiz in raices {
        if !dir.join(raiz).is_file() {
            continue;
        }
        let texto = leer(dir, raiz)?;
        let antes = texto.lines().count();
        let nuevo: Vec<&str> = texto
            .lines()
            .filter(|l| nombre_de_declaracion(l).as_deref() != Some(modulo.as_str()))
            .collect();
        if nuevo.len() != antes {
            escribir(dir, raiz, &format!("{}\n", nuevo.join("\n")))?;
        }
    }
    // Línea 0: el archivo entero desaparece.
    Ok(Some((archivo, 0)))
}

/// Sentencia `expr?;` sin `let`, `return` ni asignación: el `?` pasa a `let _ = expr;`.
fn es_sentencia_con_try(linea: &str) -> bool {
    let t = linea.trim();
    t.ends_with("?;")
        && !t.starts_with("let ")
        && !t.starts_with("return")
        && !t.starts_with("//")
        && !t.starts_with('}')
        && !t.contains('=')
}

/// Cambia una sentencia `expr?;` por `let _ = expr;`: el error se traga.
fn error_tragado(dir: &Path, rng: &mut Prng) -> io::Result<Lugar> {
    let mut candidatos: Vec<(String, usize)> = Vec::new();
    for archivo in rust_en(dir, "src")? {
        let lineas = partir(&leer(dir, &archivo)?);
        let limite = limite_de_produccion(&lineas);
        for (i, linea) in lineas[..limite].iter().enumerate() {
            if es_sentencia_con_try(linea) {
                candidatos.push((archivo.clone(), i));
            }
        }
    }
    let Some((archivo, i)) = rng.elegir(&candidatos).cloned() else {
        return Ok(None);
    };
    let mut lineas = partir(&leer(dir, &archivo)?);
    let original = lineas[i].clone();
    let t = original.trim();
    let Some(expr) = t.strip_suffix("?;") else {
        return Ok(None);
    };
    let sangria: String = original.chars().take_while(|c| c.is_whitespace()).collect();
    lineas[i] = format!("{sangria}let _ = {expr};");
    escribir(dir, &archivo, &unir(&lineas))?;
    Ok(Some((archivo, i as u32 + 1)))
}

/// Una línea de `assert!`, `assert_eq!` o `assert_ne!` completa en una línea.
fn es_assert(linea: &str) -> bool {
    let t = linea.trim();
    ["assert!(", "assert_eq!(", "assert_ne!("]
        .iter()
        .any(|p| t.starts_with(p))
        && t.ends_with(");")
}

/// Borra una línea de assert de un test: en `tests/` o en la parte de tests de `src/`.
fn assert_borrado(dir: &Path, rng: &mut Prng) -> io::Result<Lugar> {
    let mut candidatos: Vec<(String, usize)> = Vec::new();
    for archivo in rust_en(dir, "tests")? {
        for (i, linea) in partir(&leer(dir, &archivo)?).iter().enumerate() {
            if es_assert(linea) {
                candidatos.push((archivo.clone(), i));
            }
        }
    }
    for archivo in rust_en(dir, "src")? {
        let lineas = partir(&leer(dir, &archivo)?);
        let limite = limite_de_produccion(&lineas);
        for (i, linea) in lineas.iter().enumerate().skip(limite) {
            if es_assert(linea) {
                candidatos.push((archivo.clone(), i));
            }
        }
    }
    let Some((archivo, i)) = rng.elegir(&candidatos).cloned() else {
        return Ok(None);
    };
    let mut lineas = partir(&leer(dir, &archivo)?);
    lineas.remove(i);
    escribir(dir, &archivo, &unir(&lineas))?;
    Ok(Some((archivo, i as u32 + 1)))
}

/// `USER` de un usuario que no es root.
fn es_user_no_root(linea: &str) -> bool {
    linea
        .strip_prefix("USER ")
        .is_some_and(|usuario| !matches!(usuario.trim(), "root" | "0"))
}

/// Quita el `USER` no root del Dockerfile: la imagen corre como root.
fn dockerfile(dir: &Path) -> io::Result<Lugar> {
    if !dir.join("Dockerfile").is_file() {
        return Ok(None);
    }
    let lineas = partir(&leer(dir, "Dockerfile")?);
    let mut primera: Option<usize> = None;
    let mut restantes = Vec::new();
    for (i, linea) in lineas.iter().enumerate() {
        if es_user_no_root(linea) {
            if primera.is_none() {
                primera = Some(i + 1);
            }
        } else {
            restantes.push(linea.clone());
        }
    }
    let Some(linea) = primera else {
        return Ok(None);
    };
    escribir(dir, "Dockerfile", &unir(&restantes))?;
    Ok(Some(("Dockerfile".to_string(), linea as u32)))
}

#[cfg(test)]
pub(crate) mod pruebas {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};

    use tempfile::TempDir;

    use crate::oracle::tarea::Tarea;

    /// Repo Rust de prueba, versionado en el repo.
    pub const FIXTURE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/calibra/repo-rust"
    );

    /// Capas del fixture: `[arquitectura]` de `TAREA_BASE`.
    pub fn capas_fixture() -> Vec<Vec<String>> {
        vec![
            vec!["protocolo".to_string()],
            vec!["comandos".to_string()],
            vec!["almacen".to_string()],
        ]
    }

    /// Copia el fixture a un directorio temporal.
    pub fn copia_fixture() -> TempDir {
        let temporal = tempfile::tempdir().expect("temporal");
        copiar(Path::new(FIXTURE), temporal.path());
        temporal
    }

    /// Copia recursiva de `origen` a `destino`, que ya existe.
    pub fn copiar(origen: &Path, destino: &Path) {
        for entrada in fs::read_dir(origen).expect("lee el origen") {
            let entrada = entrada.expect("entrada");
            let destino_hijo = destino.join(entrada.file_name());
            if entrada.file_type().expect("tipo").is_dir() {
                fs::create_dir_all(&destino_hijo).expect("crea carpeta");
                copiar(&entrada.path(), &destino_hijo);
            } else {
                fs::copy(entrada.path(), &destino_hijo).expect("copia archivo");
            }
        }
    }

    /// Contenido de cada archivo de `dir`, con la ruta relativa y `/` como clave.
    pub fn arbol(dir: &Path) -> BTreeMap<String, String> {
        let mut mapa = BTreeMap::new();
        recoger(dir, dir, &mut mapa);
        mapa
    }

    fn recoger(raiz: &Path, dir: &Path, mapa: &mut BTreeMap<String, String>) {
        for entrada in fs::read_dir(dir).expect("lee carpeta") {
            let ruta: PathBuf = entrada.expect("entrada").path();
            if ruta.is_dir() {
                recoger(raiz, &ruta, mapa);
            } else {
                let rel = ruta
                    .strip_prefix(raiz)
                    .expect("dentro de la raíz")
                    .to_string_lossy()
                    .replace('\\', "/");
                mapa.insert(rel, fs::read_to_string(&ruta).expect("texto"));
            }
        }
    }

    /// Tarea mínima con `[arquitectura]` del fixture. `extra` se agrega al final del TOML.
    pub const TAREA_BASE: &str = r#"
nombre = "calibra-fixture"
lenguaje = "rust"
imagen = "root-oraculo-rust:1"
spec = "bootstrap/docs/SPEC.md"

[contrato]
cmd = "cargo test"

[oculto]
cmd = "cargo test"

[arquitectura]
capas = [["protocolo"], ["comandos"], ["almacen"]]
"#;

    /// Escribe `tarea.toml` en `dir` (`TAREA_BASE` más `extra`) y lo carga.
    pub fn tarea_en(dir: &Path, extra: &str) -> Tarea {
        fs::write(dir.join("tarea.toml"), format!("{TAREA_BASE}{extra}")).expect("escribe tarea");
        Tarea::cargar_de(dir).expect("carga la tarea de prueba")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::Path;

    use super::pruebas::{arbol, capas_fixture, copia_fixture};
    use super::*;

    /// Aplica `defecto` a una copia nueva del fixture. Devuelve el resultado y los árboles antes y después.
    fn aplicar(
        defecto: Defecto,
        semilla: u64,
    ) -> (
        Option<Inyeccion>,
        BTreeMap<String, String>,
        BTreeMap<String, String>,
    ) {
        let dir = copia_fixture();
        let antes = arbol(dir.path());
        let inyeccion =
            inyectar(defecto, dir.path(), &capas_fixture(), semilla).expect("no falla la E/S");
        let despues = arbol(dir.path());
        (inyeccion, antes, despues)
    }

    /// Rutas cuyo contenido cambió, se agregó o se borró.
    fn cambiados(
        antes: &BTreeMap<String, String>,
        despues: &BTreeMap<String, String>,
    ) -> Vec<String> {
        let mut rutas: BTreeSet<&String> = antes.keys().collect();
        rutas.extend(despues.keys());
        rutas
            .into_iter()
            .filter(|r| antes.get(*r) != despues.get(*r))
            .cloned()
            .collect()
    }

    #[test]
    fn inyectar_cada_defecto_apunta_a_su_dimension_e_item() {
        assert_eq!(Defecto::SecretoEnCodigo.dimension(), Dimension::Seguridad);
        assert_eq!(Defecto::SecretoEnCodigo.item(), "seguridad.secretos");
        assert_eq!(
            Defecto::PanicoEnEntrada.item(),
            "seguridad.panico_en_entrada"
        );
        assert_eq!(Defecto::BucleCuadratico.item(), "eficiencia.tiempo");
        assert_eq!(
            Defecto::CicloEntreCapas.dimension(),
            Dimension::Arquitectura
        );
        assert_eq!(Defecto::ArchivoDios.item(), "estructura.tamano");
        assert_eq!(Defecto::ModuloBorrado.item(), "completitud.alcance");
        assert_eq!(Defecto::ErrorTragado.item(), "diseno.errores");
        assert_eq!(Defecto::AssertBorrado.item(), "pruebas.mutacion");
        assert_eq!(Defecto::DockerfileRoot.item(), "entrega.contenedor");
    }

    #[test]
    fn inyectar_de_dimension_lista_sus_defectos_y_funciona_no_tiene() {
        assert_eq!(
            de_dimension(Dimension::Seguridad),
            vec![Defecto::SecretoEnCodigo, Defecto::PanicoEnEntrada]
        );
        assert!(de_dimension(Dimension::Funciona).is_empty());
    }

    #[test]
    fn inyectar_secreto_agrega_una_clave_y_nada_mas() {
        let (inyeccion, antes, despues) = aplicar(Defecto::SecretoEnCodigo, 42);
        let inyeccion = inyeccion.expect("el secreto siempre se aplica al fixture");
        assert_eq!(cambiados(&antes, &despues), vec![inyeccion.archivo.clone()]);
        let nuevo = &despues[&inyeccion.archivo];
        assert!(
            nuevo.starts_with(&antes[&inyeccion.archivo]),
            "solo agrega al final"
        );
        let clave = nuevo
            .split("\"AKIA")
            .nth(1)
            .and_then(|resto| resto.split('"').next())
            .expect("hay una clave AKIA");
        assert_eq!(clave.len(), 16, "AKIA más 16 caracteres: {clave}");
        assert!(
            clave
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        );
        let linea = nuevo
            .lines()
            .nth(inyeccion.linea as usize - 1)
            .expect("línea");
        assert!(linea.contains("CLAVE_CALIBRACION"), "{linea}");
    }

    #[test]
    fn inyectar_panico_cambia_el_signo_de_interrogacion_por_unwrap() {
        let (inyeccion, antes, despues) = aplicar(Defecto::PanicoEnEntrada, 42);
        assert_eq!(
            cambiados(&antes, &despues),
            vec!["src/protocolo.rs".to_string()]
        );
        let nuevo = &despues["src/protocolo.rs"];
        assert!(
            nuevo.contains("let numero = linea.trim().parse::<u32>().unwrap();"),
            "{nuevo}"
        );
        let antes_signos = antes["src/protocolo.rs"].matches('?').count();
        assert_eq!(
            nuevo.matches('?').count(),
            antes_signos - 1,
            "solo cambia el signo"
        );
        assert_eq!(inyeccion.expect("aplica").linea, 3);
    }

    #[test]
    fn inyectar_bucle_agrega_una_sola_linea_al_inicio_de_una_funcion() {
        let (inyeccion, antes, despues) = aplicar(Defecto::BucleCuadratico, 42);
        let inyeccion = inyeccion.expect("hay funciones en el fixture");
        assert_eq!(cambiados(&antes, &despues), vec![inyeccion.archivo.clone()]);
        let (viejo, nuevo) = (&antes[&inyeccion.archivo], &despues[&inyeccion.archivo]);
        assert_eq!(nuevo.lines().count(), viejo.lines().count() + 1);
        assert!(
            nuevo.contains("(0..2_000u32).filter(|j| *j == i).count()"),
            "{nuevo}"
        );
        let linea = nuevo
            .lines()
            .nth(inyeccion.linea as usize - 1)
            .expect("línea");
        assert!(linea.contains("let _ = (0..2_000u32)"), "{linea}");
    }

    #[test]
    fn inyectar_ciclo_agrega_un_use_de_capa_inferior_a_superior() {
        let (inyeccion, antes, despues) = aplicar(Defecto::CicloEntreCapas, 42);
        let inyeccion = inyeccion.expect("hay módulos en capas posteriores");
        let archivo = inyeccion.archivo.clone();
        assert_eq!(cambiados(&antes, &despues), vec![archivo.clone()]);
        let modulo = Path::new(&archivo)
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("nombre del módulo")
            .to_string();
        let capas = capas_fixture();
        let indice_de = |nombre: &str| capas.iter().position(|c| c.iter().any(|m| m == nombre));
        let indice_modulo = indice_de(&modulo).expect("el módulo está en una capa");
        assert!(
            indice_modulo >= 1,
            "el archivo debe ser de una capa posterior"
        );
        let uso = despues[&archivo]
            .lines()
            .find(|l| l.starts_with("use crate::") && l.ends_with(" as _;"))
            .expect("hay un use agregado");
        let superior = uso
            .trim_start_matches("use crate::")
            .trim_end_matches(" as _;");
        let indice_superior = indice_de(superior).expect("el use apunta a una capa del fixture");
        assert!(
            indice_superior < indice_modulo,
            "{uso}: debe importar una capa anterior"
        );
    }

    #[test]
    fn inyectar_ciclo_sin_capas_no_es_aplicable() {
        let dir = copia_fixture();
        let sin_capas = inyectar(Defecto::CicloEntreCapas, dir.path(), &[], 42).expect("E/S");
        assert_eq!(sin_capas, None);
        let una_capa = vec![vec!["protocolo".to_string()]];
        let sola = inyectar(Defecto::CicloEntreCapas, dir.path(), &una_capa, 42).expect("E/S");
        assert_eq!(sola, None, "con una sola capa no hay capa posterior");
    }

    #[test]
    fn inyectar_dios_concatena_los_otros_modulos_en_un_archivo() {
        let (inyeccion, antes, despues) = aplicar(Defecto::ArchivoDios, 42);
        let archivo = inyeccion.expect("hay módulos top-level").archivo;
        assert_ne!(archivo, "src/lib.rs");
        assert_eq!(cambiados(&antes, &despues), vec![archivo.clone()]);
        let nuevo = &despues[&archivo];
        assert_eq!(
            nuevo.matches("pub mod concat_").count(),
            2,
            "entran los otros dos módulos"
        );
        assert!(nuevo.len() > antes[&archivo].len() * 2, "el archivo crece");
    }

    #[test]
    fn inyectar_modulo_borrado_quita_archivo_y_declaracion() {
        let (inyeccion, antes, despues) = aplicar(Defecto::ModuloBorrado, 42);
        let archivo = inyeccion.expect("hay módulos declarados").archivo;
        assert!(!despues.contains_key(&archivo), "el archivo se borra");
        let modulo = Path::new(&archivo)
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("nombre")
            .to_string();
        let lib = &despues["src/lib.rs"];
        assert!(!lib.contains(&format!("pub mod {modulo};")), "{lib}");
        let cambiadas = cambiados(&antes, &despues);
        assert_eq!(
            cambiadas.len(),
            2,
            "solo el archivo y lib.rs: {cambiadas:?}"
        );
        assert!(cambiadas.contains(&"src/lib.rs".to_string()));
    }

    #[test]
    fn inyectar_error_tragado_cambia_el_statement_por_let_guion_bajo() {
        let (inyeccion, antes, despues) = aplicar(Defecto::ErrorTragado, 42);
        assert_eq!(
            cambiados(&antes, &despues),
            vec!["src/comandos.rs".to_string()]
        );
        let nuevo = &despues["src/comandos.rs"];
        assert!(nuevo.contains("    let _ = registrar(valor);"), "{nuevo}");
        assert!(!nuevo.contains("registrar(valor)?;"));
        assert_eq!(inyeccion.expect("aplica").linea, 6);
    }

    #[test]
    fn inyectar_assert_borrado_quita_una_linea_de_test() {
        let (_, antes, despues) = aplicar(Defecto::AssertBorrado, 42);
        assert_eq!(
            cambiados(&antes, &despues),
            vec!["tests/basico.rs".to_string()]
        );
        let asserts = |t: &str| {
            t.lines()
                .filter(|l| l.trim_start().starts_with("assert"))
                .count()
        };
        assert_eq!(
            asserts(&antes["tests/basico.rs"]) - 1,
            asserts(&despues["tests/basico.rs"])
        );
        assert_eq!(
            despues["tests/basico.rs"].lines().count() + 1,
            antes["tests/basico.rs"].lines().count()
        );
    }

    #[test]
    fn inyectar_dockerfile_quita_el_user_no_root() {
        let (inyeccion, antes, despues) = aplicar(Defecto::DockerfileRoot, 42);
        assert_eq!(cambiados(&antes, &despues), vec!["Dockerfile".to_string()]);
        let esperado: String = antes["Dockerfile"]
            .lines()
            .filter(|l| !l.starts_with("USER "))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        assert_eq!(despues["Dockerfile"], esperado);
        assert_eq!(inyeccion.expect("aplica").linea, 6);
    }

    #[test]
    fn inyectar_sin_dockerfile_o_sin_user_no_es_aplicable() {
        let dir = copia_fixture();
        crate::borrado::borrar_archivo_dentro(dir.path(), &dir.path().join("Dockerfile"))
            .expect("borra el Dockerfile");
        let sin_archivo =
            inyectar(Defecto::DockerfileRoot, dir.path(), &capas_fixture(), 42).expect("E/S");
        assert_eq!(sin_archivo, None);
        std::fs::write(dir.path().join("Dockerfile"), "FROM rust:1.85-slim\n").expect("escribe");
        let sin_user =
            inyectar(Defecto::DockerfileRoot, dir.path(), &capas_fixture(), 42).expect("E/S");
        assert_eq!(sin_user, None, "sin USER ya corre como root");
    }

    #[test]
    fn inyectar_misma_semilla_aplica_el_mismo_defecto_en_el_mismo_lugar() {
        for defecto in TODOS {
            let (a, _, despues_a) = aplicar(defecto, 7);
            let (b, _, despues_b) = aplicar(defecto, 7);
            assert_eq!(a, b, "{defecto:?}");
            assert_eq!(despues_a, despues_b, "{defecto:?}");
        }
    }

    /// Repo de prueba con dos funciones de producción: `procesar` y `otra`.
    fn repo_con_dos_funciones() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temporal");
        std::fs::create_dir_all(dir.path().join("src")).expect("src");
        std::fs::write(
            dir.path().join("src/lib.rs"),
            "pub fn procesar(x: u32) -> u32 {\n    x + 1\n}\n\nfn otra(x: u32) -> u32 {\n    x * 2\n}\n",
        )
        .expect("lib.rs");
        dir
    }

    #[test]
    fn inyectar_camino_caliente_pone_el_bucle_al_inicio_de_la_funcion_listada() {
        for semilla in 1..=20u64 {
            let dir = repo_con_dos_funciones();
            let lista = vec!["procesar".to_string()];
            let inyeccion = inyectar_con(
                Defecto::BucleCuadratico,
                dir.path(),
                &capas_fixture(),
                &lista,
                semilla,
            )
            .expect("no falla la E/S")
            .expect("procesar existe: el bucle aplica");
            assert_eq!(inyeccion.archivo, "src/lib.rs");
            let texto = std::fs::read_to_string(dir.path().join("src/lib.rs")).expect("lib.rs");
            let lineas: Vec<&str> = texto.lines().collect();
            let linea = inyeccion.linea as usize;
            assert!(
                lineas[linea - 2].contains("fn procesar"),
                "semilla {semilla}: la línea anterior es el encabezado: {texto}"
            );
            assert!(
                lineas[linea - 1].contains("let _ = (0..2_000u32)"),
                "semilla {semilla}: el bucle va en la línea {linea}: {texto}"
            );
        }
    }

    #[test]
    fn inyectar_camino_caliente_elige_entre_las_listadas_y_repite_con_la_semilla() {
        let lista = vec!["procesar".to_string(), "otra".to_string()];
        let mut elegidas = BTreeSet::new();
        for semilla in 1..=20u64 {
            let a = repo_con_dos_funciones();
            let b = repo_con_dos_funciones();
            let ia = inyectar_con(
                Defecto::BucleCuadratico,
                a.path(),
                &capas_fixture(),
                &lista,
                semilla,
            )
            .expect("E/S");
            let ib = inyectar_con(
                Defecto::BucleCuadratico,
                b.path(),
                &capas_fixture(),
                &lista,
                semilla,
            )
            .expect("E/S");
            assert_eq!(ia, ib, "semilla {semilla}: el mismo lugar");
            assert_eq!(
                arbol(a.path()),
                arbol(b.path()),
                "semilla {semilla}: el mismo cambio"
            );
            let texto = &arbol(a.path())["src/lib.rs"];
            let encabezado = texto
                .lines()
                .nth(ia.expect("aplica").linea as usize - 2)
                .expect("encabezado");
            elegidas.insert(encabezado.contains("fn otra"));
        }
        assert_eq!(
            elegidas.len(),
            2,
            "con la semilla cae en procesar y en otra"
        );
    }

    #[test]
    fn inyectar_camino_caliente_compara_el_nombre_completo() {
        let dir = tempfile::tempdir().expect("temporal");
        std::fs::create_dir_all(dir.path().join("src")).expect("src");
        std::fs::write(
            dir.path().join("src/lib.rs"),
            "pub fn procesar_todo(x: u32) -> u32 {\n    x\n}\n",
        )
        .expect("lib.rs");
        let lista = vec!["procesar".to_string()];
        let sin = inyectar_con(
            Defecto::BucleCuadratico,
            dir.path(),
            &capas_fixture(),
            &lista,
            42,
        )
        .expect("E/S");
        assert_eq!(sin, None, "procesar no es procesar_todo");
    }

    #[test]
    fn inyectar_camino_caliente_sin_funciones_existentes_no_inyecta() {
        let dir = repo_con_dos_funciones();
        let antes = arbol(dir.path());
        let lista = vec!["no_existe".to_string()];
        let inyeccion = inyectar_con(
            Defecto::BucleCuadratico,
            dir.path(),
            &capas_fixture(),
            &lista,
            42,
        )
        .expect("E/S");
        assert_eq!(inyeccion, None, "ninguna función de la lista existe");
        assert_eq!(arbol(dir.path()), antes, "no toca el repo");
    }

    #[test]
    fn inyectar_camino_caliente_vacio_da_lo_mismo_que_inyectar() {
        for semilla in 1..=10u64 {
            let a = repo_con_dos_funciones();
            let b = repo_con_dos_funciones();
            let ia = inyectar(
                Defecto::BucleCuadratico,
                a.path(),
                &capas_fixture(),
                semilla,
            )
            .expect("E/S");
            let ib = inyectar_con(
                Defecto::BucleCuadratico,
                b.path(),
                &capas_fixture(),
                &[],
                semilla,
            )
            .expect("E/S");
            assert_eq!(ia, ib, "semilla {semilla}");
            assert_eq!(arbol(a.path()), arbol(b.path()), "semilla {semilla}");
        }
    }

    #[test]
    fn inyectar_otra_semilla_puede_elegir_otro_lugar() {
        let lugares: BTreeSet<(String, u32)> = (1..=30u64)
            .filter_map(|semilla| aplicar(Defecto::SecretoEnCodigo, semilla).0)
            .map(|i| (i.archivo, i.linea))
            .collect();
        assert!(
            lugares.len() >= 2,
            "las semillas deben elegir lugares distintos: {lugares:?}"
        );
    }
}
