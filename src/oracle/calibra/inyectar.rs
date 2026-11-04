//! Inyectores de defectos de la calibración (Tarea 12). Cada inyector toma una copia de un repo Rust y una
//! semilla, y aplica UN defecto por construcción. La semilla elige el archivo, la función o la línea con un
//! PRNG propio (splitmix64). Con la misma copia y la misma semilla, el cambio es el mismo.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;

use crate::borrado;
use crate::oracle::mutacion::MutanteMuerto;
use crate::oracle::perfiles::rust::LimitesEstructura;
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

/// Lo que `inyectar_con` necesita además de la copia y la semilla.
pub struct Contexto<'a> {
    /// Capas de `[arquitectura]` de la tarea.
    pub capas: &'a [Vec<String>],
    /// Funciones del camino caliente (Tarea 12). Vacía: el bucle cae en cualquier función.
    pub camino_caliente: &'a [String],
    /// Límites de la sonda de estructura. El defecto `ArchivoDios` los usa para pasar la sonda.
    pub limites: &'a LimitesEstructura,
    /// Mutantes muertos de la base: `AssertBorrado` elige uno y borra los asserts de sus tests. Vacía: no aplica.
    pub muertos: &'a [MutanteMuerto],
}

/// Aplica `defecto` a la copia de repo Rust en `dir`. `capas` son las de `[arquitectura]` de la tarea.
/// `Ok(None)` si el defecto no se puede aplicar a ese repo: no hay Dockerfile, no hay tests, etc.
/// Usa los límites por defecto de la sonda de estructura.
pub fn inyectar(
    defecto: Defecto,
    dir: &Path,
    capas: &[Vec<String>],
    semilla: u64,
) -> io::Result<Option<Inyeccion>> {
    let limites = LimitesEstructura::default();
    let contexto = Contexto {
        capas,
        camino_caliente: &[],
        limites: &limites,
        muertos: &[],
    };
    inyectar_con(defecto, dir, &contexto, semilla)
}

/// Como `inyectar`, con el `Contexto` completo. Con `camino_caliente` no vacía (Tarea 12, desempate 42), el
/// bucle O(n²) cae solo al inicio de una de esas funciones, elegida con la semilla entre las que existen en
/// la copia. Si ninguna existe, `Ok(None)`. Los demás defectos no usan la lista.
pub fn inyectar_con(
    defecto: Defecto,
    dir: &Path,
    contexto: &Contexto,
    semilla: u64,
) -> io::Result<Option<Inyeccion>> {
    let mut rng = Prng::nuevo(semilla);
    let lugar = match defecto {
        Defecto::SecretoEnCodigo => secreto(dir, &mut rng)?,
        Defecto::PanicoEnEntrada => panico(dir, &mut rng)?,
        Defecto::BucleCuadratico => bucle(dir, contexto.camino_caliente, &mut rng)?,
        Defecto::CicloEntreCapas => ciclo(dir, contexto.capas, &mut rng)?,
        Defecto::ArchivoDios => dios(dir, contexto.limites, &mut rng)?,
        Defecto::ModuloBorrado => modulo_borrado(dir, &mut rng)?,
        Defecto::ErrorTragado => error_tragado(dir, &mut rng)?,
        Defecto::AssertBorrado => assert_borrado(dir, contexto.muertos, &mut rng)?,
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

/// Clave con forma de AWS (`AKIA` y 16 caracteres de A-Z y 2-7, como las ids reales) sacada de la semilla.
/// No es una credencial.
fn clave_sintetica(rng: &mut Prng) -> String {
    const ALFABETO: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
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

/// Agrega funciones de relleno a un módulo de primer nivel de `src/`, hasta que el archivo pasa el límite de
/// archivo y una función pasa el de función. Los demás archivos no cambian. Cada función de relleno es válida,
/// no tiene dependencias y no tiene uso. Devuelve el archivo y la línea de su primera función.
fn dios(dir: &Path, limites: &LimitesEstructura, rng: &mut Prng) -> io::Result<Lugar> {
    let mut destinos: Vec<String> = Vec::new();
    for archivo in rust_en(dir, "src")? {
        let nombre = archivo.trim_start_matches("src/");
        if nombre.contains('/') || nombre == "lib.rs" || nombre == "main.rs" {
            continue;
        }
        let stem = nombre.trim_end_matches(".rs");
        if es_identificador(stem) {
            destinos.push(archivo);
        }
    }
    let Some(destino) = rng.elegir(&destinos).cloned() else {
        return Ok(None);
    };
    let mut texto = leer(dir, &destino)?;
    // Una función de `funcion_lineas + 1` líneas de cuerpo pasa el límite. Cada una agrega líneas al archivo.
    let largo = limites.funcion_lineas.saturating_add(1);
    let mut numero = 1;
    let mut primera = None;
    while primera.is_none() || texto.lines().count() <= limites.archivo_lineas {
        while texto.contains(&format!("fn relleno_calibracion_{numero}(")) {
            numero += 1;
        }
        texto = insertar_antes_de_tests(&texto, &relleno(numero, largo));
        if primera.is_none() {
            primera = Some(numero);
        }
        numero += 1;
    }
    let primera = primera.unwrap_or(1);
    escribir(dir, &destino, &texto)?;
    Ok(Some((
        destino,
        linea_de(&texto, &format!("pub fn relleno_calibracion_{primera}(")),
    )))
}

/// Función de relleno `relleno_calibracion_<numero>` con `largo` líneas de cuerpo. Cada línea del cuerpo
/// tiene constantes propias, así que ningún bloque de líneas se repite en el archivo.
fn relleno(numero: usize, largo: usize) -> Vec<String> {
    let mut bloque = vec![
        "#[allow(dead_code)]".to_string(),
        format!("pub fn relleno_calibracion_{numero}(entrada: u32) -> u32 {{"),
        "    let mut acumulado = entrada;".to_string(),
    ];
    for i in 0..largo {
        let k = numero * 100_000 + i;
        bloque.push(format!(
            "    acumulado = acumulado.wrapping_mul({}).wrapping_add({});",
            2 * k + 1,
            7 * k + 3
        ));
    }
    bloque.push("    acumulado".to_string());
    bloque.push("}".to_string());
    bloque
}

/// Inserta `bloque` antes de `#[cfg(test)]`, o al final si no hay tests. Va con una línea en blanco antes.
fn insertar_antes_de_tests(texto: &str, bloque: &[String]) -> String {
    let mut lineas = partir(texto);
    let limite = limite_de_produccion(&lineas);
    let mut entrada = vec![String::new()];
    entrada.extend(bloque.iter().cloned());
    if limite < lineas.len() {
        entrada.push(String::new());
        lineas.splice(limite..limite, entrada);
    } else {
        // Sin tests: el último elemento es lo que sigue al salto final, si el texto termina en `\n`.
        let fin = if lineas.last().is_some_and(String::is_empty) {
            lineas.len() - 1
        } else {
            lineas.len()
        };
        lineas.splice(fin..fin, entrada);
    }
    unir(&lineas)
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
/// La línea debe tener paréntesis balanceados. Así `)?;` de una llamada multilínea no cuenta.
fn es_sentencia_con_try(linea: &str) -> bool {
    let t = linea.trim();
    t.ends_with("?;")
        && !t.starts_with("let ")
        && !t.starts_with("return")
        && !t.starts_with("//")
        && !t.starts_with('}')
        && !t.contains('=')
        && equilibrada(t)
}

/// `true` si cada `(`, `[` y `{` se cierra con su par, en orden.
fn equilibrada(texto: &str) -> bool {
    let mut pila = Vec::new();
    for c in texto.chars() {
        match c {
            '(' | '[' | '{' => pila.push(c),
            ')' | ']' | '}' => {
                let abre = match c {
                    ')' => '(',
                    ']' => '[',
                    _ => '{',
                };
                if pila.pop() != Some(abre) {
                    return false;
                }
            }
            _ => {}
        }
    }
    pila.is_empty()
}

/// `true` si la línea anterior no vacía cierra una sentencia o un bloque. Sin anterior, también.
/// Así `b()?;` tras `let total = a +` no cuenta: continúa el `let`.
fn empieza_sentencia(lineas: &[String], i: usize) -> bool {
    let anterior = lineas[..i]
        .iter()
        .rev()
        .map(|l| l.trim())
        .find(|t| !t.is_empty() && !t.starts_with("//"));
    match anterior {
        None => true,
        Some(t) => t.ends_with(';') || t.ends_with('{') || t.ends_with('}'),
    }
}

/// Cambia una sentencia `expr?;` por `let _ = expr;`: el error se traga.
fn error_tragado(dir: &Path, rng: &mut Prng) -> io::Result<Lugar> {
    let mut candidatos: Vec<(String, usize)> = Vec::new();
    for archivo in rust_en(dir, "src")? {
        let lineas = partir(&leer(dir, &archivo)?);
        let limite = limite_de_produccion(&lineas);
        for (i, linea) in lineas[..limite].iter().enumerate() {
            if es_sentencia_con_try(linea) && empieza_sentencia(&lineas, i) {
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

/// Borra los asserts de los tests que cubren un mutante muerto de `muertos`. La semilla elige el mutante por
/// su posición. Si su función no tiene tests con asserts, prueba el siguiente de la lista, en orden. Sin
/// muertos, o sin ninguno con test, no aplica: no hay borrado al azar.
fn assert_borrado(dir: &Path, muertos: &[MutanteMuerto], rng: &mut Prng) -> io::Result<Lugar> {
    if muertos.is_empty() {
        return Ok(None);
    }
    let inicio = rng.indice(muertos.len());
    for k in 0..muertos.len() {
        let mutante = &muertos[(inicio + k) % muertos.len()];
        let Some(funcion) = funcion_de_mutante(dir, mutante)? else {
            continue;
        };
        let cubiertos = asserts_de_tests_que_llaman(dir, &funcion)?;
        let Some((archivo, indices)) = cubiertos.first() else {
            continue;
        };
        let lugar = (archivo.clone(), indices[0] as u32 + 1);
        for (archivo, indices) in &cubiertos {
            quitar_lineas(dir, archivo, indices)?;
        }
        return Ok(Some(lugar));
    }
    Ok(None)
}

/// Función afectada por un mutante: la del nombre (`... replace <fn> -> <Tipo> with <valor>`) o, si el nombre
/// no la tiene, la que contiene su línea. `None` si no hay ninguna.
fn funcion_de_mutante(dir: &Path, mutante: &MutanteMuerto) -> io::Result<Option<String>> {
    if let Some(funcion) = funcion_del_nombre(&mutante.nombre) {
        return Ok(Some(funcion));
    }
    if !dir.join(&mutante.archivo).is_file() {
        return Ok(None);
    }
    let lineas = partir(&leer(dir, &mutante.archivo)?);
    let hasta = (mutante.linea as usize).min(lineas.len());
    Ok(lineas[..hasta]
        .iter()
        .rev()
        .find_map(|linea| nombre_de_funcion(linea).map(str::to_string)))
}

/// Función de un nombre de mutante `... replace <fn> -> <Tipo> with <valor>`. `None` si la parte antes de
/// `->` o de `with` no es una ruta de identificadores, como en `replace a + b with a - b`.
fn funcion_del_nombre(nombre: &str) -> Option<String> {
    let (_, resto) = nombre.split_once("replace ")?;
    let cabeza = resto.split(" -> ").next()?.split(" with ").next()?.trim();
    let ultimo = cabeza.rsplit("::").next()?;
    es_identificador(ultimo).then(|| ultimo.to_string())
}

/// Por archivo de tests con asserts que cubren `funcion`, en orden (primero `tests/`, luego `src/`): los índices
/// de sus líneas de assert. Solo entran los archivos con al menos una línea que borrar.
fn asserts_de_tests_que_llaman(dir: &Path, funcion: &str) -> io::Result<Vec<(String, Vec<usize>)>> {
    let mut origenes: Vec<(String, bool)> = Vec::new();
    for archivo in rust_en(dir, "tests")? {
        origenes.push((archivo, false));
    }
    for archivo in rust_en(dir, "src")? {
        origenes.push((archivo, true));
    }
    let mut salida = Vec::new();
    for (archivo, solo_tests_de_src) in origenes {
        let lineas = partir(&leer(dir, &archivo)?);
        let desde = if solo_tests_de_src {
            limite_de_produccion(&lineas)
        } else {
            0
        };
        let mut indices = Vec::new();
        for (fn_linea, cierre) in cuerpos_de_tests(&lineas, desde) {
            let cuerpo = fn_linea + 1..=cierre;
            if cuerpo.clone().any(|j| llama_a(&lineas[j], funcion)) {
                indices.extend(cuerpo.filter(|&j| es_assert(&lineas[j])));
            }
        }
        if !indices.is_empty() {
            salida.push((archivo, indices));
        }
    }
    Ok(salida)
}

/// Rangos `(línea de fn, línea de cierre)` de los tests de `lineas` desde `desde`. Un test empieza con
/// `#[test]`, y su `fn` es la primera línea que no es atributo después.
fn cuerpos_de_tests(lineas: &[String], desde: usize) -> Vec<(usize, usize)> {
    let mut salida = Vec::new();
    let mut i = desde;
    while i < lineas.len() {
        if lineas[i].trim() == "#[test]" {
            let fn_linea =
                (i + 1..lineas.len()).find(|&j| !lineas[j].trim_start().starts_with("#["));
            if let Some(f) = fn_linea.filter(|&f| lineas[f].trim_start().starts_with("fn "))
                && let Some(cierre) = fin_de_bloque(lineas, f)
            {
                salida.push((f, cierre));
                i = cierre + 1;
                continue;
            }
        }
        i += 1;
    }
    salida
}

/// Línea donde cierra el bloque que abre `inicio`: la que vuelve la profundidad de `{` a cero. `None` si no cierra.
fn fin_de_bloque(lineas: &[String], inicio: usize) -> Option<usize> {
    let mut profundidad = 0i32;
    let mut abrio = false;
    for (j, linea) in lineas.iter().enumerate().skip(inicio) {
        for c in linea.chars() {
            match c {
                '{' => {
                    profundidad += 1;
                    abrio = true;
                }
                '}' => profundidad -= 1,
                _ => {}
            }
        }
        if abrio && profundidad == 0 {
            return Some(j);
        }
    }
    None
}

/// `true` si `linea` llama a `funcion`: `funcion(` precedido de algo que no es identificador (`.`, `::`, espacio).
fn llama_a(linea: &str, funcion: &str) -> bool {
    let patron = format!("{funcion}(");
    linea.match_indices(&patron).any(|(p, _)| {
        linea[..p]
            .chars()
            .next_back()
            .is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_'))
    })
}

/// Quita de `archivo` las líneas de `indices` (0-based, en orden). El resto del texto no cambia.
fn quitar_lineas(dir: &Path, archivo: &str, indices: &[usize]) -> io::Result<()> {
    let lineas = partir(&leer(dir, archivo)?);
    let nuevas: Vec<String> = lineas
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !indices.contains(i))
        .map(|(_, linea)| linea)
        .collect();
    escribir(dir, archivo, &unir(&nuevas))
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

    /// Sonda real de estructura sobre `dir`, con los límites por defecto y sin requisitos.
    fn sonda_estructura(dir: &Path) -> crate::oracle::perfiles::ResultadoSonda {
        crate::oracle::perfiles::rust::estructura_nativa(dir, &[], &LimitesEstructura::default())
    }

    #[test]
    fn inyectar_dios_agrega_funciones_largas_y_no_toca_los_otros_modulos() {
        let (inyeccion, antes, despues) = aplicar(Defecto::ArchivoDios, 42);
        let inyeccion = inyeccion.expect("hay módulos de primer nivel");
        let archivo = inyeccion.archivo.clone();
        assert_ne!(archivo, "src/lib.rs");
        assert_eq!(cambiados(&antes, &despues), vec![archivo.clone()]);
        let nuevo = &despues[&archivo];
        assert!(
            nuevo.starts_with(&antes[&archivo]),
            "el contenido original queda como prefijo"
        );
        let limites = LimitesEstructura::default();
        assert!(
            nuevo.lines().count() > limites.archivo_lineas,
            "el archivo pasa el límite de archivo: {}",
            nuevo.lines().count()
        );
        let primera = linea_de(nuevo, "pub fn relleno_calibracion_");
        assert_eq!(
            inyeccion.linea, primera,
            "la línea es la de la primera función"
        );
        assert!(
            nuevo.contains("fn relleno_calibracion_"),
            "agrega funciones de relleno"
        );
        assert!(!nuevo.contains("concat_"), "ya no copia otros módulos");
    }

    #[test]
    fn inyectar_dios_la_sonda_real_ve_archivo_grande_y_funcion_larga_sin_bloques_duplicados() {
        let dir = copia_fixture();
        let inyeccion = inyectar(Defecto::ArchivoDios, dir.path(), &capas_fixture(), 42)
            .expect("E/S")
            .expect("aplica");
        let resultado = sonda_estructura(dir.path());
        let de_archivo: Vec<_> = resultado
            .hallazgos
            .iter()
            .filter(|h| h.archivo == inyeccion.archivo)
            .collect();
        assert!(
            de_archivo
                .iter()
                .any(|h| h.severidad == 2 && h.detalle.starts_with("archivo grande")),
            "hallazgo de archivo grande: {:?}",
            resultado.hallazgos
        );
        assert!(
            de_archivo
                .iter()
                .any(|h| h.severidad == 2 && h.detalle.starts_with("función de")),
            "hallazgo de función larga: {:?}",
            resultado.hallazgos
        );
        assert!(
            !resultado
                .hallazgos
                .iter()
                .any(|h| h.detalle.contains("repetido en")),
            "sin bloques duplicados: {:?}",
            resultado.hallazgos
        );
    }

    /// `cargo check --offline` sobre `dir`, con el target en `objetivo`. Devuelve el stderr si falla.
    fn cargo_check(dir: &Path, objetivo: &Path) -> Result<(), String> {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
        let salida = std::process::Command::new(cargo)
            .arg("check")
            .arg("--offline")
            .current_dir(dir)
            .env("CARGO_TARGET_DIR", objetivo)
            .output()
            .expect("lanza cargo check");
        if salida.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&salida.stderr).into_owned())
        }
    }

    /// Repo donde `dos` nombra `super::uno`: con `dos` concatenado dentro de `uno`, esa ruta ya no existe.
    fn repo_con_super() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temporal");
        std::fs::create_dir_all(dir.path().join("src")).expect("src");
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"super-repo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .expect("Cargo.toml");
        std::fs::write(
            dir.path().join("src/lib.rs"),
            "pub mod uno;\npub mod dos;\n",
        )
        .expect("lib.rs");
        std::fs::write(
            dir.path().join("src/uno.rs"),
            "pub fn base() -> u32 {\n    1\n}\n",
        )
        .expect("uno.rs");
        std::fs::write(
            dir.path().join("src/dos.rs"),
            "use super::uno::base;\n\npub fn usa() -> u32 {\n    base() + 1\n}\n",
        )
        .expect("dos.rs");
        dir
    }

    #[test]
    fn inyectar_dios_la_copia_compila_con_rutas_super() {
        let objetivo = tempfile::tempdir().expect("temporal");
        for semilla in 1..=6u64 {
            let dir = repo_con_super();
            inyectar(Defecto::ArchivoDios, dir.path(), &[], semilla)
                .expect("E/S")
                .expect("aplica");
            if let Err(stderr) = cargo_check(dir.path(), objetivo.path()) {
                panic!("semilla {semilla}: la copia no compila:\n{stderr}");
            }
        }
    }

    #[test]
    fn inyectar_dios_la_copia_del_fixture_compila() {
        let dir = copia_fixture();
        inyectar(Defecto::ArchivoDios, dir.path(), &capas_fixture(), 42)
            .expect("E/S")
            .expect("aplica");
        let objetivo = tempfile::tempdir().expect("temporal");
        if let Err(stderr) = cargo_check(dir.path(), objetivo.path()) {
            panic!("la copia del fixture no compila:\n{stderr}");
        }
    }

    #[test]
    fn inyectar_dios_con_limites_chicos_agrega_menos_relleno() {
        let lineas_con = |archivo_lineas: usize| {
            let dir = copia_fixture();
            let capas = capas_fixture();
            let limites = LimitesEstructura {
                archivo_lineas,
                funcion_lineas: 80,
                bloque_lineas: 10,
            };
            let contexto = Contexto {
                capas: &capas,
                camino_caliente: &[],
                limites: &limites,
                muertos: &[],
            };
            let inyeccion = inyectar_con(Defecto::ArchivoDios, dir.path(), &contexto, 42)
                .expect("E/S")
                .expect("aplica");
            arbol(dir.path())[&inyeccion.archivo].lines().count()
        };
        assert!(
            lineas_con(60) < lineas_con(500),
            "con archivo_lineas 60 hay menos relleno que con 500"
        );
    }

    #[test]
    fn inyectar_dios_sin_modulos_de_primer_nivel_no_aplica() {
        let dir = tempfile::tempdir().expect("temporal");
        std::fs::create_dir_all(dir.path().join("src")).expect("src");
        std::fs::write(dir.path().join("src/lib.rs"), "pub fn uno() -> u32 { 1 }\n")
            .expect("lib.rs");
        let antes = arbol(dir.path());
        let inyeccion =
            inyectar(Defecto::ArchivoDios, dir.path(), &capas_fixture(), 42).expect("E/S");
        assert_eq!(inyeccion, None);
        assert_eq!(arbol(dir.path()), antes, "no toca el repo");
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

    /// Repo mínimo con `src/lib.rs` = `texto`. Sin `tarea.toml`: `error_tragado` solo lee `src/`.
    fn repo_con_lib(texto: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temporal");
        std::fs::create_dir_all(dir.path().join("src")).expect("src");
        escribir(dir.path(), "src/lib.rs", texto).expect("lib.rs");
        dir
    }

    #[test]
    fn inyectar_error_tragado_no_elige_linea_de_continuacion() {
        // La única línea `?;` es el cierre de una llamada multilínea. Reescribirla rompe el código.
        let original = "pub fn guardar(a: u32) -> Result<(), String> {\n    registrar(\n        a,\n        a,\n    )?;\n    Ok(())\n}\n\nfn registrar(a: u32, b: u32) -> Result<(), String> {\n    if a == b { Ok(()) } else { Err(String::new()) }\n}\n";
        let dir = repo_con_lib(original);
        for semilla in 1..=20u64 {
            let inyeccion = inyectar(Defecto::ErrorTragado, dir.path(), &capas_fixture(), semilla)
                .expect("no falla la E/S");
            assert!(
                inyeccion.is_none(),
                "semilla {semilla}: no hay sentencia completa: {inyeccion:?}"
            );
            assert_eq!(
                leer(dir.path(), "src/lib.rs").expect("lee"),
                original,
                "semilla {semilla}"
            );
        }
    }

    #[test]
    fn inyectar_error_tragado_no_elige_continuacion_de_let() {
        // `registrar(a)?;` continúa el `let` de arriba: no es una sentencia propia.
        let original = "pub fn total(a: u32) -> Result<u32, String> {\n    let total = a +\n        registrar(a)?;\n    Ok(total)\n}\n\nfn registrar(a: u32) -> Result<u32, String> {\n    Ok(a)\n}\n";
        let dir = repo_con_lib(original);
        for semilla in 1..=20u64 {
            let inyeccion = inyectar(Defecto::ErrorTragado, dir.path(), &capas_fixture(), semilla)
                .expect("no falla la E/S");
            assert!(
                inyeccion.is_none(),
                "semilla {semilla}: es continuación de let: {inyeccion:?}"
            );
            assert_eq!(
                leer(dir.path(), "src/lib.rs").expect("lee"),
                original,
                "semilla {semilla}"
            );
        }
    }

    /// Test que cubre `leer_numero` (`src/protocolo.rs`, con dos asserts), y otro test sin relación con un assert.
    const TEST_QUE_CUBRE: &str = "use calibra_fixture::protocolo::leer_numero;\n\n#[test]\nfn leer_numero_lee_el_numero() {\n    assert!(leer_numero(\"42\").is_ok());\n    assert_eq!(leer_numero(\"7\").unwrap(), 7);\n}\n\n#[test]\nfn otro_test_sin_relacion() {\n    assert_eq!(1 + 1, 2);\n}\n";

    /// Cuántas líneas de `t` son asserts.
    fn asserts_en(t: &str) -> usize {
        t.lines()
            .filter(|l| l.trim_start().starts_with("assert"))
            .count()
    }

    /// Copia del fixture con `tests/protocolo.rs`, que cubre `leer_numero`.
    fn copia_con_test_de_leer_numero() -> tempfile::TempDir {
        let dir = copia_fixture();
        std::fs::write(dir.path().join("tests/protocolo.rs"), TEST_QUE_CUBRE).expect("escribe");
        dir
    }

    /// Mutante muerto de `leer_numero`, con el nombre que da cargo-mutants.
    fn muerto_de_leer_numero() -> MutanteMuerto {
        MutanteMuerto {
            archivo: "src/protocolo.rs".into(),
            linea: 2,
            nombre:
                "src/protocolo.rs:2:1: replace leer_numero -> Result<u32, ParseIntError> with Ok(0)"
                    .into(),
        }
    }

    /// Mutante muerto de una función que ningún test llama.
    fn muerto_sin_test() -> MutanteMuerto {
        MutanteMuerto {
            archivo: "src/lib.rs".into(),
            linea: 1,
            nombre: "src/lib.rs:1:1: replace nadie_lo_llama -> u32 with 0".into(),
        }
    }

    /// `inyectar_con` de `AssertBorrado` en `dir`, con `muertos` y la semilla dada.
    fn borrar_con(
        dir: &Path,
        muertos: &[MutanteMuerto],
        semilla: u64,
    ) -> io::Result<Option<Inyeccion>> {
        let capas = capas_fixture();
        let limites = LimitesEstructura::default();
        let contexto = Contexto {
            capas: &capas,
            camino_caliente: &[],
            limites: &limites,
            muertos,
        };
        inyectar_con(Defecto::AssertBorrado, dir, &contexto, semilla)
    }

    #[test]
    fn inyectar_assert_borrado_quita_solo_los_asserts_del_test_que_cubre_el_mutante() {
        let dir = copia_con_test_de_leer_numero();
        let antes = arbol(dir.path());
        let inyeccion = borrar_con(dir.path(), &[muerto_de_leer_numero()], 42)
            .expect("E/S")
            .expect("hay un test que cubre leer_numero");
        let despues = arbol(dir.path());
        assert_eq!(
            cambiados(&antes, &despues),
            vec!["tests/protocolo.rs".to_string()]
        );
        assert_eq!(inyeccion.defecto, Defecto::AssertBorrado);
        assert_eq!(inyeccion.archivo, "tests/protocolo.rs");
        assert_eq!(inyeccion.linea, 5, "la línea del primer assert borrado");
        let nuevo = &despues["tests/protocolo.rs"];
        assert_eq!(asserts_en(&antes["tests/protocolo.rs"]), 3);
        assert_eq!(
            asserts_en(nuevo),
            1,
            "borra los dos asserts del test que cubre"
        );
        assert!(nuevo.contains("assert_eq!(1 + 1, 2);"), "{nuevo}");
        assert!(nuevo.contains("fn leer_numero_lee_el_numero()"), "{nuevo}");
    }

    #[test]
    fn inyectar_assert_borrado_sin_nombre_de_funcion_usa_la_funcion_de_la_linea() {
        let dir = copia_con_test_de_leer_numero();
        let muerto = MutanteMuerto {
            archivo: "src/protocolo.rs".into(),
            linea: 3,
            nombre: "src/protocolo.rs:3:5: replace a + b with a - b".into(),
        };
        let inyeccion = borrar_con(dir.path(), &[muerto], 42)
            .expect("E/S")
            .expect("la línea 3 está dentro de leer_numero");
        assert_eq!(inyeccion.archivo, "tests/protocolo.rs");
        assert_eq!(inyeccion.linea, 5);
        assert_eq!(asserts_en(&arbol(dir.path())["tests/protocolo.rs"]), 1);
    }

    #[test]
    fn inyectar_assert_borrado_sin_muertos_no_es_aplicable() {
        let dir = copia_con_test_de_leer_numero();
        let antes = arbol(dir.path());
        let sin_muertos = borrar_con(dir.path(), &[], 42).expect("E/S");
        assert_eq!(sin_muertos, None, "sin muertos no hay al azar");
        let capas = capas_fixture();
        let por_la_api = inyectar(Defecto::AssertBorrado, dir.path(), &capas, 42).expect("E/S");
        assert_eq!(por_la_api, None, "inyectar pasa una lista vacía");
        assert_eq!(arbol(dir.path()), antes, "no toca el repo");
    }

    #[test]
    fn inyectar_assert_borrado_muerto_sin_test_que_lo_cubra_no_es_aplicable() {
        let dir = copia_fixture();
        let antes = arbol(dir.path());
        let sin_test = borrar_con(dir.path(), &[muerto_de_leer_numero()], 42).expect("E/S");
        assert_eq!(sin_test, None, "basico.rs no llama a leer_numero");
        assert_eq!(arbol(dir.path()), antes, "no toca el repo");
    }

    #[test]
    fn inyectar_assert_borrado_salta_el_muerto_sin_test_y_usa_el_que_tiene() {
        let muertos = [muerto_sin_test(), muerto_de_leer_numero()];
        for semilla in 1..=10u64 {
            let dir = copia_con_test_de_leer_numero();
            let antes = arbol(dir.path());
            let inyeccion = borrar_con(dir.path(), &muertos, semilla)
                .expect("E/S")
                .expect("siempre hay un muerto con test");
            assert_eq!(inyeccion.archivo, "tests/protocolo.rs", "semilla {semilla}");
            assert_eq!(
                cambiados(&antes, &arbol(dir.path())),
                vec!["tests/protocolo.rs".to_string()],
                "semilla {semilla}"
            );
        }
    }

    #[test]
    fn inyectar_assert_borrado_misma_semilla_mismo_resultado() {
        let muertos = [muerto_sin_test(), muerto_de_leer_numero()];
        for semilla in 1..=5u64 {
            let a = copia_con_test_de_leer_numero();
            let b = copia_con_test_de_leer_numero();
            let ia = borrar_con(a.path(), &muertos, semilla).expect("E/S");
            let ib = borrar_con(b.path(), &muertos, semilla).expect("E/S");
            assert_eq!(ia, ib, "semilla {semilla}");
            assert_eq!(arbol(a.path()), arbol(b.path()), "semilla {semilla}");
        }
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

    /// `inyectar_con` con las capas del fixture, los límites por defecto y el camino caliente `camino`.
    fn con_camino(
        defecto: Defecto,
        dir: &Path,
        camino: &[String],
        semilla: u64,
    ) -> io::Result<Option<Inyeccion>> {
        let capas = capas_fixture();
        let limites = LimitesEstructura::default();
        let contexto = Contexto {
            capas: &capas,
            camino_caliente: camino,
            limites: &limites,
            muertos: &[],
        };
        inyectar_con(defecto, dir, &contexto, semilla)
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
            let inyeccion = con_camino(Defecto::BucleCuadratico, dir.path(), &lista, semilla)
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
            let ia = con_camino(Defecto::BucleCuadratico, a.path(), &lista, semilla).expect("E/S");
            let ib = con_camino(Defecto::BucleCuadratico, b.path(), &lista, semilla).expect("E/S");
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
        let sin = con_camino(Defecto::BucleCuadratico, dir.path(), &lista, 42).expect("E/S");
        assert_eq!(sin, None, "procesar no es procesar_todo");
    }

    #[test]
    fn inyectar_camino_caliente_sin_funciones_existentes_no_inyecta() {
        let dir = repo_con_dos_funciones();
        let antes = arbol(dir.path());
        let lista = vec!["no_existe".to_string()];
        let inyeccion = con_camino(Defecto::BucleCuadratico, dir.path(), &lista, 42).expect("E/S");
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
            let ib = con_camino(Defecto::BucleCuadratico, b.path(), &[], semilla).expect("E/S");
            assert_eq!(ia, ib, "semilla {semilla}");
            assert_eq!(arbol(a.path()), arbol(b.path()), "semilla {semilla}");
        }
    }

    #[test]
    fn inyectar_secreto_clave_tiene_la_forma_que_gitleaks_reconoce() {
        // La regla `aws-access-token` de gitleaks solo casa `AKIA` seguido de 16 caracteres de A-Z y 2-7.
        // Con dígitos 0, 1, 8 o 9 la clave pasa sin hallazgo.
        for semilla in 1..=30u64 {
            let (inyeccion, _, despues) = aplicar(Defecto::SecretoEnCodigo, semilla);
            let inyeccion = inyeccion.expect("el secreto siempre se aplica al fixture");
            let nuevo = &despues[&inyeccion.archivo];
            let clave = nuevo
                .split("\"AKIA")
                .nth(1)
                .and_then(|resto| resto.split('"').next())
                .expect("hay una clave AKIA");
            assert!(
                clave
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || ('2'..='7').contains(&c)),
                "semilla {semilla}: la clave sale de A-Z y 2-7: {clave}"
            );
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
