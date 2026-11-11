//! Inyectores de defectos de la calibración (Tarea 12). Cada inyector toma una copia de un repo Rust y una
//! semilla, y aplica UN defecto por construcción. La semilla elige el archivo, la función o la línea con un
//! PRNG propio (splitmix64). Con la misma copia y la misma semilla, el cambio es el mismo.

use std::collections::{BTreeMap, BTreeSet};
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
    ComportamientoRoto,
    CuerpoVaciado,
}

/// Todos los defectos, en el orden de `Defecto`.
pub const TODOS: [Defecto; 11] = [
    Defecto::SecretoEnCodigo,
    Defecto::PanicoEnEntrada,
    Defecto::BucleCuadratico,
    Defecto::CicloEntreCapas,
    Defecto::ArchivoDios,
    Defecto::ModuloBorrado,
    Defecto::ErrorTragado,
    Defecto::AssertBorrado,
    Defecto::DockerfileRoot,
    Defecto::ComportamientoRoto,
    Defecto::CuerpoVaciado,
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
            Defecto::ComportamientoRoto => Dimension::Funciona,
            Defecto::CuerpoVaciado => Dimension::Completitud,
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
            Defecto::ComportamientoRoto => "funciona.correccion",
            Defecto::CuerpoVaciado => "completitud.alcance",
        }
    }
}

/// Defectos que sirven para una dimensión, en el orden en que `calibrar` los rota. Vacío si no hay ninguno.
/// `ModuloBorrado` queda fuera: borrar un módulo que se usa siempre rompe el build. Es una detección segura.
pub fn de_dimension(dimension: Dimension) -> Vec<Defecto> {
    TODOS
        .into_iter()
        .filter(|d| d.dimension() == dimension && *d != Defecto::ModuloBorrado)
        .collect()
}

/// Un defecto aplicado: dónde quedó. `linea` es 1-based; 0 significa el archivo entero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inyeccion {
    pub defecto: Defecto,
    /// Ruta relativa a la raíz de la copia, con `/`.
    pub archivo: String,
    pub linea: u32,
    /// Archivo del mutante muerto que eligió `AssertBorrado`. `None` en los demás defectos.
    pub mutante_archivo: Option<String>,
    /// Nombre del mutante muerto que eligió `AssertBorrado`. `None` en los demás defectos.
    pub mutante: Option<String>,
    /// Nombres completos de los tests que `AssertBorrado` apagó con `#[ignore]`. Vacía en los demás defectos.
    pub tests_apagados: Vec<String>,
    /// `true` si el defecto cayó dentro del cambio de `bootstrap/`.
    /// En `ComportamientoRoto`, la línea invertida es una línea tocada.
    /// En `ModuloBorrado`, el módulo borrado es un archivo tocado.
    /// En `CuerpoVaciado`, la función vaciada cruza una línea tocada, desde su firma hasta su cierre.
    /// Sin estado inicial, la función se elige entre todas, así que puede caer fuera del cambio: `false`.
    /// Un archivo tocado tiene al menos una línea del cambio.
    pub en_cambio: bool,
    /// Operador que `ComportamientoRoto` cambió, con modo de estado inicial (p. ej. `"> -> >="`). `None` en los demás.
    pub operador: Option<String>,
}

/// Lo que `inyectar_con` necesita además de la copia y la semilla.
pub struct Contexto<'a> {
    /// Capas de `[arquitectura]` de la tarea.
    pub capas: &'a [Vec<String>],
    /// Funciones del camino caliente (Tarea 12). Vacía: el bucle cae en cualquier función.
    pub camino_caliente: &'a [String],
    /// Límites de la sonda de estructura. El defecto `ArchivoDios` los usa para pasar la sonda.
    pub limites: &'a LimitesEstructura,
    /// Mutantes muertos de la base: `AssertBorrado` elige uno y apaga con `#[ignore]` sus tests. Vacía: no aplica.
    pub muertos: &'a [MutanteMuerto],
    /// Rangos de líneas del cambio (1-based, inclusivos) por ruta relativa. `None`: la tarea no trae `bootstrap/`.
    pub cambios: Option<&'a Rangos>,
    /// Carpeta del estado inicial (`bootstrap/`). `Some`: `ComportamientoRoto` cambia también operadores y condiciones,
    /// `CuerpoVaciado` vacía solo funciones que ya existían, y `ModuloBorrado` no aplica.
    /// `None`: `CuerpoVaciado` elige cualquier función de producción.
    pub inicial: Option<&'a Path>,
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
        cambios: None,
        inicial: None,
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
    let mut apagado = None;
    let mut operador: Option<String> = None;
    let mut cuerpo_en_cambio = false;
    let lugar = match defecto {
        Defecto::SecretoEnCodigo => secreto(dir, &mut rng)?,
        Defecto::PanicoEnEntrada => panico(dir, &mut rng)?,
        Defecto::BucleCuadratico => bucle(dir, contexto.camino_caliente, &mut rng)?,
        Defecto::CicloEntreCapas => ciclo(dir, contexto.capas, &mut rng)?,
        Defecto::ArchivoDios => dios(dir, contexto.limites, &mut rng)?,
        Defecto::ModuloBorrado => {
            modulo_borrado(dir, &mut rng, contexto.cambios, contexto.inicial)?
        }
        Defecto::ErrorTragado => error_tragado(dir, &mut rng)?,
        Defecto::AssertBorrado => {
            apagado = assert_borrado(dir, contexto.muertos, &mut rng)?;
            apagado.as_ref().map(|a| (a.archivo.clone(), a.linea))
        }
        Defecto::DockerfileRoot => dockerfile(dir)?,
        Defecto::CuerpoVaciado => {
            let (lugar, en_cambio) =
                cuerpo_vaciado(dir, &mut rng, contexto.cambios, contexto.inicial)?;
            cuerpo_en_cambio = en_cambio;
            lugar
        }
        Defecto::ComportamientoRoto => {
            comportamiento(dir, &mut rng, contexto.cambios, contexto.inicial)?.map(
                |(archivo, linea, op)| {
                    operador = op;
                    (archivo, linea)
                },
            )
        }
    };
    // Con `cambios`, un defecto de línea o de módulo solo cae dentro del cambio: toda elección lo está.
    // Un cuerpo vaciado sin estado inicial puede caer fuera: su marca sale de `cuerpo_vaciado`.
    let en_cambio = lugar.is_some()
        && match defecto {
            Defecto::ComportamientoRoto | Defecto::ModuloBorrado => contexto.cambios.is_some(),
            Defecto::CuerpoVaciado => cuerpo_en_cambio,
            _ => false,
        };
    Ok(lugar.map(|(archivo, linea)| match apagado {
        Some(a) => Inyeccion {
            defecto,
            archivo,
            linea,
            mutante_archivo: Some(a.mutante_archivo),
            mutante: Some(a.mutante),
            tests_apagados: a.tests,
            en_cambio,
            operador,
        },
        None => Inyeccion {
            defecto,
            archivo,
            linea,
            mutante_archivo: None,
            mutante: None,
            tests_apagados: Vec::new(),
            en_cambio,
            operador,
        },
    }))
}

/// Archivo y línea (1-based) donde quedó el defecto, o `None` si no se pudo aplicar.
type Lugar = Option<(String, u32)>;

/// Rangos de líneas del cambio (1-based, inclusivos) por ruta relativa con `/`. Ver `Contexto.cambios`.
type Rangos = BTreeMap<String, Vec<(u32, u32)>>;

/// `true` si la línea `linea` (1-based) de `archivo` cae en un rango del cambio. Sin cambio (`None`), siempre.
fn en_el_cambio(cambios: Option<&Rangos>, archivo: &str, linea: u32) -> bool {
    cambios.is_none_or(|rangos| {
        rangos.get(archivo).is_some_and(|rs| {
            rs.iter()
                .any(|&(desde, hasta)| (desde..=hasta).contains(&linea))
        })
    })
}

/// `true` si `archivo` tiene al menos un rango del cambio. Sin cambio (`None`), siempre.
fn archivo_tocado(cambios: Option<&Rangos>, archivo: &str) -> bool {
    cambios.is_none_or(|rangos| rangos.get(archivo).is_some_and(|rs| !rs.is_empty()))
}

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
/// Acepta visibilidad (`pub`, `pub(crate)`, `pub(super)`) y los calificadores `const`, `async` y `unsafe`,
/// en cualquier orden.
fn nombre_de_funcion(linea: &str) -> Option<&str> {
    let mut t = linea.trim_start();
    if let Some(resto) = ["pub(crate) ", "pub(super) ", "pub "]
        .iter()
        .find_map(|v| t.strip_prefix(v))
    {
        t = resto;
    }
    while let Some(resto) = ["const ", "async ", "unsafe "]
        .iter()
        .find_map(|q| t.strip_prefix(q))
    {
        t = resto;
    }
    let resto = t.strip_prefix("fn ")?;
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
/// Con `cambios`, solo los módulos cuyo archivo tiene un rango del cambio.
/// Con `inicial`, no aplica: el estado inicial ya tiene esos módulos y borrarlos rompe el build.
fn modulo_borrado(
    dir: &Path,
    rng: &mut Prng,
    cambios: Option<&Rangos>,
    inicial: Option<&Path>,
) -> io::Result<Lugar> {
    if inicial.is_some() {
        return Ok(None);
    }
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
        .filter(|m| {
            let archivo = format!("src/{m}.rs");
            dir.join(&archivo).is_file() && archivo_tocado(cambios, &archivo)
        })
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

/// Un cuerpo de función en un archivo: la línea de la firma, la de la `{` de apertura y la de la `}` de cierre
/// (0-based), con la posición de cada llave en su línea. Las posiciones son bytes de la línea original.
#[derive(Debug, Clone, Copy)]
struct Cuerpo {
    firma: usize,
    abre: usize,
    abre_pos: usize,
    cierra: usize,
    cierra_pos: usize,
}

/// Cuerpo de la función que declara la línea `firma`. La firma puede ocupar varias líneas: el cuerpo empieza en la
/// primera `{` fuera de paréntesis y corchetes. `None` si la firma termina en `;` (declaración), si no cierra, o si
/// abre y cierra en la misma línea.
fn cuerpo_de(lineas: &[String], firma: usize) -> Option<Cuerpo> {
    let mut nivel = 0i32;
    let mut apertura = None;
    'firma: for (j, linea) in lineas.iter().enumerate().skip(firma) {
        for (p, c) in linea_limpia(linea).char_indices() {
            match c {
                '(' | '[' => nivel += 1,
                ')' | ']' => nivel -= 1,
                ';' if nivel == 0 => return None,
                '{' if nivel == 0 => {
                    apertura = Some((j, p));
                    break 'firma;
                }
                _ => {}
            }
        }
    }
    let (abre, abre_pos) = apertura?;
    let mut profundidad = 0i32;
    for (k, linea) in lineas.iter().enumerate().skip(abre) {
        let limpia = linea_limpia(linea);
        let desde = if k == abre { abre_pos } else { 0 };
        for (p, c) in limpia.char_indices().skip_while(|(p, _)| *p < desde) {
            match c {
                '{' => profundidad += 1,
                '}' => profundidad -= 1,
                _ => {}
            }
            if profundidad == 0 {
                return (k != abre).then_some(Cuerpo {
                    firma,
                    abre,
                    abre_pos,
                    cierra: k,
                    cierra_pos: p,
                });
            }
        }
    }
    None
}

/// `true` si el cuerpo, sin espacios, es solo `todo!()`.
fn cuerpo_es_todo(lineas: &[String], cuerpo: &Cuerpo) -> bool {
    let mut texto = lineas[cuerpo.abre][cuerpo.abre_pos + 1..].to_string();
    for linea in &lineas[cuerpo.abre + 1..cuerpo.cierra] {
        texto.push_str(linea);
    }
    texto.push_str(&lineas[cuerpo.cierra][..cuerpo.cierra_pos]);
    texto.retain(|c| !c.is_whitespace());
    texto == "todo!()"
}

/// `true` si `texto` declara la función `nombre`: `fn nombre(` o `fn nombre<`.
fn declara_fn(texto: &str, nombre: &str) -> bool {
    texto
        .lines()
        .any(|l| l.contains(&format!("fn {nombre}(")) || l.contains(&format!("fn {nombre}<")))
}

/// Vacía el cuerpo de una función de producción. El cuerpo queda en `todo!()`, con la firma intacta.
/// Con `inicial`, solo elige funciones que ya existían en el estado inicial y que el cambio tocó.
/// Sin `inicial`, elige cualquier función de producción con la semilla. Devuelve el lugar y si la elegida
/// cruza una línea del cambio. Ese `bool` es `false` sin `cambios`.
fn cuerpo_vaciado(
    dir: &Path,
    rng: &mut Prng,
    cambios: Option<&Rangos>,
    inicial: Option<&Path>,
) -> io::Result<(Lugar, bool)> {
    // (archivo, cuerpo, cruza el cambio)
    let mut candidatos: Vec<(String, Cuerpo, bool)> = Vec::new();
    for archivo in rust_en(dir, "src")? {
        let lineas = partir(&leer(dir, &archivo)?);
        let previo = match inicial {
            Some(inicial) if inicial.join(&archivo).is_file() => Some(leer(inicial, &archivo)?),
            Some(_) => Some(String::new()),
            None => None,
        };
        let limite = limite_de_produccion(&lineas);
        for firma in 0..limite {
            let Some(nombre) = nombre_de_funcion(&lineas[firma]) else {
                continue;
            };
            if previo.as_ref().is_some_and(|p| !declara_fn(p, nombre)) {
                continue;
            }
            let Some(cuerpo) = cuerpo_de(&lineas, firma).filter(|c| c.cierra < limite) else {
                continue;
            };
            let cruza =
                (firma..=cuerpo.cierra).any(|l| en_el_cambio(cambios, &archivo, l as u32 + 1));
            if (inicial.is_none() || cruza) && !cuerpo_es_todo(&lineas, &cuerpo) {
                candidatos.push((archivo.clone(), cuerpo, cambios.is_some() && cruza));
            }
        }
    }
    let Some((archivo, cuerpo, en_cambio)) = rng.elegir(&candidatos).cloned() else {
        return Ok((None, false));
    };
    let lineas = partir(&leer(dir, &archivo)?);
    let sangria =
        |linea: &str| -> String { linea.chars().take_while(|c| c.is_whitespace()).collect() };
    let mut nuevo: Vec<String> = lineas[..cuerpo.abre].to_vec();
    nuevo.push(lineas[cuerpo.abre][..=cuerpo.abre_pos].to_string());
    nuevo.push(format!("{}    todo!()", sangria(&lineas[cuerpo.firma])));
    nuevo.push(format!("{}}}", sangria(&lineas[cuerpo.cierra])));
    nuevo.extend_from_slice(&lineas[cuerpo.cierra + 1..]);
    escribir(dir, &archivo, &unir(&nuevo))?;
    Ok((Some((archivo, cuerpo.firma as u32 + 1)), en_cambio))
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

/// Lo que `AssertBorrado` apagó: el lugar del primer `#[ignore]` y el mutante elegido.
struct Apagado {
    archivo: String,
    linea: u32,
    mutante_archivo: String,
    mutante: String,
    tests: Vec<String>,
}

/// Apaga con `#[ignore]` los tests de un mutante muerto de `muertos`. La semilla elige el mutante por su
/// posición. Si ese mutante no sirve, prueba el siguiente, en orden. Sin muertos que sirvan, no aplica: no hay
/// apagado al azar.
fn assert_borrado(
    dir: &Path,
    muertos: &[MutanteMuerto],
    rng: &mut Prng,
) -> io::Result<Option<Apagado>> {
    if muertos.is_empty() {
        return Ok(None);
    }
    let inicio = rng.indice(muertos.len());
    for k in 0..muertos.len() {
        let mutante = &muertos[(inicio + k) % muertos.len()];
        if let Some(apagado) = apagar_tests(dir, mutante)? {
            return Ok(Some(apagado));
        }
    }
    Ok(None)
}

/// Un archivo `.rs` con sus líneas, y la línea desde la que empiezan sus tests (0 en `tests/`).
type Fuente = (String, Vec<String>, usize);

/// Agrega `#[ignore]` antes de la `fn` de cada test de `mutante.tests`. Busca todos antes de escribir: si uno no
/// aparece, `None` y el repo no cambia. El nombre se compara por su último segmento (`db::tests::zset` da `zset`).
fn apagar_tests(dir: &Path, mutante: &MutanteMuerto) -> io::Result<Option<Apagado>> {
    if mutante.tests.is_empty() {
        return Ok(None);
    }
    let mut fuentes: Vec<Fuente> = Vec::new();
    for archivo in rust_en(dir, "tests")? {
        let lineas = partir(&leer(dir, &archivo)?);
        fuentes.push((archivo, lineas, 0));
    }
    for archivo in rust_en(dir, "src")? {
        let lineas = partir(&leer(dir, &archivo)?);
        let desde = limite_de_produccion(&lineas);
        fuentes.push((archivo, lineas, desde));
    }
    // (índice de la fuente, línea de la `fn`) de cada test que coincide, y el primero de la lista.
    let mut marcas: BTreeSet<(usize, usize)> = BTreeSet::new();
    let mut primera: Option<(usize, usize)> = None;
    for nombre in &mutante.tests {
        let ultimo = nombre.rsplit("::").next().unwrap_or(nombre.as_str());
        let mut hallados = Vec::new();
        for (k, (_, lineas, desde)) in fuentes.iter().enumerate() {
            for (f, _) in cuerpos_de_tests(lineas, *desde) {
                if nombre_de_funcion(&lineas[f]) == Some(ultimo) {
                    hallados.push((k, f));
                }
            }
        }
        let Some(&primero) = hallados.first() else {
            return Ok(None);
        };
        primera.get_or_insert(primero);
        marcas.extend(hallados);
    }
    let Some((k_primera, f_primera)) = primera else {
        return Ok(None);
    };
    // Línea 1-based del `#[ignore]` de cada test apagado, en el archivo ya escrito.
    let mut linea_del_ignore: BTreeMap<(usize, usize), u32> = BTreeMap::new();
    for (k, (archivo, lineas, _)) in fuentes.iter().enumerate() {
        let filas: Vec<usize> = marcas
            .iter()
            .filter(|(kk, _)| *kk == k)
            .map(|(_, f)| *f)
            .collect();
        if filas.is_empty() {
            continue;
        }
        let mut nuevas: Vec<String> = Vec::with_capacity(lineas.len() + filas.len());
        for (i, linea) in lineas.iter().enumerate() {
            if filas.contains(&i) {
                let sangria: String = linea.chars().take_while(|c| c.is_whitespace()).collect();
                nuevas.push(format!("{sangria}#[ignore]"));
                linea_del_ignore.insert((k, i), nuevas.len() as u32);
            }
            nuevas.push(linea.clone());
        }
        escribir(dir, archivo, &unir(&nuevas))?;
    }
    Ok(Some(Apagado {
        archivo: fuentes[k_primera].0.clone(),
        linea: linea_del_ignore[&(k_primera, f_primera)],
        mutante_archivo: mutante.archivo.clone(),
        mutante: mutante.nombre.clone(),
        tests: mutante.tests.clone(),
    }))
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

/// Posición del primer ` == ` de `linea` si es una comparación de código real. Descarta comentarios, textos (un
/// `"` antes del `==`), líneas con `=>`, `>=` o `<=`, y los ` == ` sin expresión a un lado.
fn posicion_de_igualdad(linea: &str) -> Option<usize> {
    let t = linea.trim_start();
    if t.starts_with("//") || t.starts_with("/*") || t.starts_with("* ") {
        return None;
    }
    if ["=>", ">=", "<="]
        .iter()
        .any(|flecha| linea.contains(flecha))
    {
        return None;
    }
    let p = linea.find(" == ")?;
    let antes = &linea[..p];
    let despues = &linea[p + 4..];
    let real = !antes.contains('"')
        && !antes.contains("//")
        && !antes.contains("/*")
        && !antes.trim().is_empty()
        && !despues.trim().is_empty();
    real.then_some(p)
}

/// Cambia el primer ` == ` de una línea de producción de `src/` por ` != `: la comparación sale invertida.
/// Usa solo las líneas de antes de `#[cfg(test)]`. `Ok(None)` si no hay ninguna comparación de ese tipo.
/// Con `cambios`, solo las líneas que caen dentro de un rango del cambio.
fn igualdad_invertida(dir: &Path, rng: &mut Prng, cambios: Option<&Rangos>) -> io::Result<Lugar> {
    let mut candidatos: Vec<(String, usize)> = Vec::new();
    for archivo in rust_en(dir, "src")? {
        let lineas = partir(&leer(dir, &archivo)?);
        let limite = limite_de_produccion(&lineas);
        for (i, linea) in lineas[..limite].iter().enumerate() {
            if posicion_de_igualdad(linea).is_some()
                && en_el_cambio(cambios, &archivo, i as u32 + 1)
            {
                candidatos.push((archivo.clone(), i));
            }
        }
    }
    let Some((archivo, i)) = rng.elegir(&candidatos).cloned() else {
        return Ok(None);
    };
    let mut lineas = partir(&leer(dir, &archivo)?);
    let Some(p) = posicion_de_igualdad(&lineas[i]) else {
        return Ok(None);
    };
    lineas[i].replace_range(p..p + 4, " != ");
    escribir(dir, &archivo, &unir(&lineas))?;
    Ok(Some((archivo, i as u32 + 1)))
}

/// Pares de operadores que `ComportamientoRoto` cambia con estado inicial. Con espacios a ambos lados: no toca
/// genéricos (`<T>`), flechas (`->`, `=>`) ni desplazamientos (`<<`, `>>`).
const PARES_DE_OPERADOR: [(&str, &str); 8] = [
    (" < ", " <= "),
    (" <= ", " < "),
    (" > ", " >= "),
    (" >= ", " > "),
    (" == ", " != "),
    (" != ", " == "),
    (" && ", " || "),
    (" || ", " && "),
];

/// Constantes booleanas. Son palabras enteras: no van pegadas a letras, dígitos ni `_`.
const PARES_DE_PALABRA: [(&str, &str); 2] = [("true", "false"), ("false", "true")];

/// Sumas de uno. El `1` no sigue de dígito, letra, `_` ni `.`.
const PARES_DE_SUMA: [(&str, &str); 2] = [(" + 1", " - 1"), (" - 1", " + 1")];

/// Prefijos de línea que tienen una condición. `else if` y `} else if` van aparte: el prefijo es el texto exacto.
const PREFIJOS_DE_CONDICION: [&str; 4] = ["} else if ", "else if ", "if ", "while "];

/// Un cambio posible en una línea: reemplaza `inicio..fin` de la línea original por `nuevo`. `operador` lo nombra.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Edicion {
    inicio: usize,
    fin: usize,
    nuevo: String,
    operador: String,
}

impl Edicion {
    fn aplicar(&self, linea: &str) -> String {
        let mut nueva = linea.to_string();
        nueva.replace_range(self.inicio..self.fin, &self.nuevo);
        nueva
    }
}

fn cambio(inicio: usize, fin: usize, nuevo: &str, operador: String) -> Edicion {
    Edicion {
        inicio,
        fin,
        nuevo: nuevo.to_string(),
        operador,
    }
}

/// Nombre legible de un par: `> -> >=`.
fn nombre_de_par(de: &str, a: &str) -> String {
    format!("{} -> {}", de.trim(), a.trim())
}

/// `true` si `c` puede seguir o empezar un identificador: letra, dígito o `_`.
fn es_parte_de_palabra(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// La línea con el contenido de cadenas, de caracteres y de comentarios `//` en espacios. Mismo largo en bytes y
/// mismas posiciones: un operador que aparece aquí está en esa posición del código real.
fn linea_limpia(linea: &str) -> String {
    let cars: Vec<char> = linea.chars().collect();
    let mut oculto = vec![false; cars.len()];
    let mut en_cadena = false;
    let mut i = 0;
    while i < cars.len() {
        if en_cadena {
            oculto[i] = true;
            match cars[i] {
                '\\' => {
                    if let Some(o) = oculto.get_mut(i + 1) {
                        *o = true;
                    }
                    i += 2;
                    continue;
                }
                '"' => en_cadena = false,
                _ => {}
            }
        } else if cars[i] == '/' && cars.get(i + 1) == Some(&'/') {
            oculto[i..].fill(true);
            break;
        } else if cars[i] == '"' {
            en_cadena = true;
            oculto[i] = true;
        } else if cars[i] == '\''
            && let Some(fin) = fin_de_caracter(&cars, i)
        {
            oculto[i..=fin].fill(true);
            i = fin;
        }
        i += 1;
    }
    cars.iter()
        .zip(&oculto)
        .map(|(&c, &o)| {
            if o {
                " ".repeat(c.len_utf8())
            } else {
                c.to_string()
            }
        })
        .collect()
}

/// Índice del `'` que cierra el literal de carácter que abre en `i` (`'a'`, `'\n'`, `'\''`). `None` si es un lifetime.
fn fin_de_caracter(cars: &[char], i: usize) -> Option<usize> {
    match cars.get(i + 1)? {
        '\\' => (i + 3..cars.len()).find(|&j| cars[j] == '\''),
        _ => (cars.get(i + 2) == Some(&'\'')).then_some(i + 2),
    }
}

/// `true` si la línea empieza con `//`, `/*` o `* `: no tiene código que mutar.
fn es_comentario(linea: &str) -> bool {
    let t = linea.trim_start();
    t.starts_with("//") || t.starts_with("/*") || t.starts_with("* ")
}

/// Cambios de operador de comparación o lógico en la línea limpia.
fn ediciones_de_operador(limpia: &str) -> Vec<Edicion> {
    let mut salida = Vec::new();
    for &(de, a) in &PARES_DE_OPERADOR {
        for (p, _) in limpia.match_indices(de) {
            salida.push(cambio(p, p + de.len(), a, nombre_de_par(de, a)));
        }
    }
    salida
}

/// Cambios de `true`/`false` como palabra entera, y de `+ 1`/`- 1` donde el `1` no sigue de dígito, letra, `_` ni `.`.
fn ediciones_de_valor(limpia: &str) -> Vec<Edicion> {
    let mut salida = Vec::new();
    for &(de, a) in &PARES_DE_PALABRA {
        for (p, _) in limpia.match_indices(de) {
            let fin = p + de.len();
            let antes = limpia[..p].chars().next_back();
            let despues = limpia[fin..].chars().next();
            if !antes.is_some_and(es_parte_de_palabra) && !despues.is_some_and(es_parte_de_palabra)
            {
                salida.push(cambio(p, fin, a, nombre_de_par(de, a)));
            }
        }
    }
    for &(de, a) in &PARES_DE_SUMA {
        for (p, _) in limpia.match_indices(de) {
            let fin = p + de.len();
            let despues = limpia[fin..].chars().next();
            if !despues.is_some_and(|c| es_parte_de_palabra(c) || c == '.') {
                salida.push(cambio(p, fin, a, nombre_de_par(de, a)));
            }
        }
    }
    salida
}

/// Cambios de la condición de un `if`, `while` o `else if`: quita el `!` que tiene, o la envuelve en `!( ... )`.
/// La condición acaba en la primera `{` de la línea limpia, la que abre el bloque. Sin esa `{`, la condición
/// sigue en otra línea y no hay envoltorio. `if let` queda fuera.
fn ediciones_de_condicion(linea: &str, limpia: &str) -> Vec<Edicion> {
    let inicio_codigo = limpia.len() - limpia.trim_start().len();
    let resto = &limpia[inicio_codigo..];
    let Some(prefijo) = PREFIJOS_DE_CONDICION
        .into_iter()
        .find(|p| resto.starts_with(*p))
    else {
        return Vec::new();
    };
    let c = inicio_codigo + prefijo.len();
    let condicion = &limpia[c..];
    if condicion.starts_with("let ") {
        return Vec::new();
    }
    if condicion.starts_with('!') && !condicion.starts_with("!=") {
        return vec![cambio(c, c + 1, "", "if: quitar !".to_string())];
    }
    let Some(llave) = condicion.find('{') else {
        return Vec::new();
    };
    let fin = c + condicion[..llave].trim_end().len();
    if fin == c {
        return Vec::new();
    }
    // La línea limpia y la original tienen las mismas posiciones en bytes: `linea_limpia` no las mueve.
    let nuevo = format!("!({})", &linea[c..fin]);
    vec![cambio(c, fin, &nuevo, "if: agregar !".to_string())]
}

/// Todos los cambios posibles de `ComportamientoRoto` en una línea de producción. Vacío en comentarios.
fn candidatos_de_linea(linea: &str) -> Vec<Edicion> {
    if es_comentario(linea) {
        return Vec::new();
    }
    let limpia = linea_limpia(linea);
    let mut salida = ediciones_de_operador(&limpia);
    salida.extend(ediciones_de_valor(&limpia));
    salida.extend(ediciones_de_condicion(linea, &limpia));
    salida
}

/// Líneas del archivo en el estado inicial, sin espacios a los lados. Vacío si el archivo no existe allí.
fn lineas_del_inicial(inicial: &Path, archivo: &str) -> io::Result<BTreeSet<String>> {
    let ruta = inicial.join(archivo);
    if !ruta.is_file() {
        return Ok(BTreeSet::new());
    }
    Ok(partir(&fs::read_to_string(ruta)?)
        .iter()
        .map(|l| l.trim().to_string())
        .collect())
}

/// Modo operadores (`inicial` dado): cambia un operador, un valor o una condición dentro del cambio de `bootstrap/`.
/// Descarta lo que el estado inicial ya tenía, para no revertir el cambio. `Ok(None)` si no queda ningún candidato.
fn comportamiento_con_inicial(
    dir: &Path,
    rng: &mut Prng,
    cambios: Option<&Rangos>,
    inicial: &Path,
) -> io::Result<Option<(String, u32, Option<String>)>> {
    let mut candidatos: Vec<(String, usize, Edicion)> = Vec::new();
    for archivo in rust_en(dir, "src")? {
        let lineas = partir(&leer(dir, &archivo)?);
        let previas = lineas_del_inicial(inicial, &archivo)?;
        let limite = limite_de_produccion(&lineas);
        for (i, linea) in lineas[..limite].iter().enumerate() {
            if !en_el_cambio(cambios, &archivo, i as u32 + 1) {
                continue;
            }
            for edicion in candidatos_de_linea(linea) {
                if !previas.contains(edicion.aplicar(linea).trim()) {
                    candidatos.push((archivo.clone(), i, edicion));
                }
            }
        }
    }
    let Some((archivo, i, edicion)) = rng.elegir(&candidatos).cloned() else {
        return Ok(None);
    };
    let mut lineas = partir(&leer(dir, &archivo)?);
    lineas[i] = edicion.aplicar(&lineas[i]);
    escribir(dir, &archivo, &unir(&lineas))?;
    Ok(Some((archivo, i as u32 + 1, Some(edicion.operador))))
}

/// Cambia un defecto de comportamiento en `dir`. Sin `inicial`, cambia el primer ` == ` por ` != ` (la regla de
/// siempre). Con `inicial`, cambia un operador, un valor o una condición (ver `comportamiento_con_inicial`).
/// Devuelve archivo, línea (1-based) y el operador cambiado, que solo existe con `inicial`.
fn comportamiento(
    dir: &Path,
    rng: &mut Prng,
    cambios: Option<&Rangos>,
    inicial: Option<&Path>,
) -> io::Result<Option<(String, u32, Option<String>)>> {
    match inicial {
        Some(inicial) => comportamiento_con_inicial(dir, rng, cambios, inicial),
        None => {
            Ok(igualdad_invertida(dir, rng, cambios)?
                .map(|(archivo, linea)| (archivo, linea, None)))
        }
    }
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

    use super::pruebas::{arbol, capas_fixture, copia_fixture, copiar};
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
        assert_eq!(Defecto::ComportamientoRoto.dimension(), Dimension::Funciona);
        assert_eq!(Defecto::ComportamientoRoto.item(), "funciona.correccion");
        assert_eq!(Defecto::CuerpoVaciado.dimension(), Dimension::Completitud);
        assert_eq!(Defecto::CuerpoVaciado.item(), "completitud.alcance");
    }

    #[test]
    fn inyectar_de_dimension_lista_sus_defectos_y_funciona_tiene_el_suyo() {
        assert_eq!(
            de_dimension(Dimension::Seguridad),
            vec![Defecto::SecretoEnCodigo, Defecto::PanicoEnEntrada]
        );
        assert_eq!(
            de_dimension(Dimension::Completitud),
            vec![Defecto::CuerpoVaciado]
        );
        assert_eq!(
            de_dimension(Dimension::Funciona),
            vec![Defecto::ComportamientoRoto]
        );
    }

    #[test]
    fn de_dimension_completitud_no_trae_modulo_borrado() {
        assert_eq!(
            de_dimension(Dimension::Completitud),
            vec![Defecto::CuerpoVaciado]
        );
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
                inicial: None,
                cambios: None,
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
            tests: vec!["protocolo::leer_numero_lee_el_numero".into()],
        }
    }

    /// Mutante muerto de una función que ningún test llama.
    fn muerto_sin_test() -> MutanteMuerto {
        MutanteMuerto {
            archivo: "src/lib.rs".into(),
            linea: 1,
            nombre: "src/lib.rs:1:1: replace nadie_lo_llama -> u32 with 0".into(),
            tests: vec![],
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
            inicial: None,
            muertos,
            cambios: None,
        };
        inyectar_con(Defecto::AssertBorrado, dir, &contexto, semilla)
    }

    #[test]
    fn inyectar_assert_borrado_apaga_solo_el_test_que_cubre_el_mutante() {
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
        assert_eq!(inyeccion.linea, 4, "la línea del #[ignore] del test");
        assert_eq!(
            inyeccion.tests_apagados,
            vec!["protocolo::leer_numero_lee_el_numero".to_string()]
        );
        assert_eq!(inyeccion.mutante, Some(muerto_de_leer_numero().nombre));
        let nuevo = &despues["tests/protocolo.rs"];
        assert!(
            nuevo.contains("#[test]\n#[ignore]\nfn leer_numero_lee_el_numero()"),
            "{nuevo}"
        );
        assert!(
            nuevo.contains("fn otro_test_sin_relacion()") && !nuevo.contains("#[ignore]\nfn otro"),
            "{nuevo}"
        );
        assert_eq!(
            asserts_en(nuevo),
            asserts_en(&antes["tests/protocolo.rs"]),
            "no borra asserts"
        );
    }

    #[test]
    fn inyectar_assert_borrado_guarda_el_archivo_del_mutante_elegido() {
        let dir = copia_con_test_de_leer_numero();
        let inyeccion = borrar_con(dir.path(), &[muerto_de_leer_numero()], 42)
            .expect("E/S")
            .expect("hay un test que cubre leer_numero");
        assert_eq!(
            inyeccion.mutante_archivo.as_deref(),
            Some("src/protocolo.rs")
        );
    }

    #[test]
    fn inyectar_otros_defectos_no_guardan_mutante_archivo() {
        let dir = copia_fixture();
        let capas = capas_fixture();
        let limites = LimitesEstructura::default();
        let contexto = Contexto {
            capas: &capas,
            camino_caliente: &[],
            inicial: None,
            limites: &limites,
            muertos: &[muerto_de_leer_numero()],
            cambios: None,
        };
        let inyeccion = inyectar_con(Defecto::SecretoEnCodigo, dir.path(), &contexto, 42)
            .expect("E/S")
            .expect("aplica");
        assert_eq!(inyeccion.mutante_archivo, None);
        assert_eq!(inyeccion.mutante, None);
        assert!(inyeccion.tests_apagados.is_empty());
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
        assert_eq!(sin_test, None, "basico.rs no tiene el test del mutante");
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

    /// Lib con `parsear` y su único test, que la llama con `.unwrap()` y un assert que no depende del valor.
    const LIB_CON_UNWRAP: &str = "pub fn dividir(a: i32, b: i32) -> i32 {\n    a / b\n}\n\npub fn parsear(texto: &str) -> Result<i32, std::num::ParseIntError> {\n    texto.parse()\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn ese_test() {\n        let _n = parsear(\"7\").unwrap();\n        assert!(true);\n    }\n}\n";

    /// Test que llama a `leer_numero` por medio de `envoltorio`: el nombre de la función no aparece en su cuerpo.
    const TEST_POR_ENVOLTORIO: &str = "use calibra_fixture::protocolo::leer_numero;\n\nfn envoltorio(texto: &str) -> Result<u32, std::num::ParseIntError> {\n    leer_numero(texto)\n}\n\n#[test]\nfn lee_por_envoltorio() {\n    assert_eq!(envoltorio(\"7\").unwrap(), 7);\n}\n";

    /// `cargo test --offline` sobre `dir`, con el target en `objetivo`. `Err` con la salida si falla.
    fn cargo_test(dir: &Path, objetivo: &Path) -> Result<(), String> {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
        let salida = std::process::Command::new(cargo)
            .arg("test")
            .arg("--offline")
            .current_dir(dir)
            .env("CARGO_TARGET_DIR", objetivo)
            .output()
            .expect("lanza cargo test");
        if salida.status.success() {
            Ok(())
        } else {
            Err(format!(
                "{}{}",
                String::from_utf8_lossy(&salida.stdout),
                String::from_utf8_lossy(&salida.stderr)
            ))
        }
    }

    #[test]
    fn assert_borrado_apagado_el_test_del_unwrap_deja_vivo_al_mutante() {
        let dir = repo_de(&[
            (
                "Cargo.toml",
                "[package]\nname = \"minimo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
            ),
            ("src/lib.rs", LIB_CON_UNWRAP),
        ]);
        let muerto = MutanteMuerto {
            archivo: "src/lib.rs".into(),
            linea: 5,
            nombre: "src/lib.rs:5:5: replace parsear -> Result<i32, ParseIntError> with Err(..)"
                .into(),
            tests: vec!["tests::ese_test".into()],
        };
        let inyeccion = borrar_con(dir.path(), &[muerto], 42)
            .expect("E/S")
            .expect("el test del mutante aplica");
        let ruta = dir.path().join("src/lib.rs");
        let texto = std::fs::read_to_string(&ruta).expect("lee");
        assert!(
            texto.contains("    #[ignore]\n    fn ese_test()"),
            "{texto}"
        );
        assert_eq!(
            inyeccion.tests_apagados,
            vec!["tests::ese_test".to_string()]
        );
        // El mutante, a mano: `parsear` siempre devuelve un error.
        std::fs::write(
            &ruta,
            texto.replace("    texto.parse()\n", "    \"x\".parse()\n"),
        )
        .expect("escribe");
        let objetivo = tempfile::tempdir().expect("temporal");
        let resultado = cargo_test(dir.path(), objetivo.path());
        assert!(
            resultado.is_ok(),
            "con el test apagado el mutante vive: {resultado:?}"
        );
    }

    #[test]
    fn assert_borrado_apaga_el_test_que_llega_a_la_funcion_por_un_envoltorio() {
        let dir = copia_fixture();
        std::fs::write(dir.path().join("tests/envoltorio.rs"), TEST_POR_ENVOLTORIO)
            .expect("escribe");
        let muerto = MutanteMuerto {
            archivo: "src/protocolo.rs".into(),
            linea: 2,
            nombre:
                "src/protocolo.rs:2:1: replace leer_numero -> Result<u32, ParseIntError> with Ok(0)"
                    .into(),
            tests: vec!["envoltorio::lee_por_envoltorio".into()],
        };
        let inyeccion = borrar_con(dir.path(), &[muerto], 42)
            .expect("E/S")
            .expect("el test del mutante aplica aunque no nombre la función");
        assert_eq!(inyeccion.archivo, "tests/envoltorio.rs");
        assert_eq!(
            inyeccion.tests_apagados,
            vec!["envoltorio::lee_por_envoltorio".to_string()]
        );
        let texto = std::fs::read_to_string(dir.path().join("tests/envoltorio.rs")).expect("lee");
        assert_eq!(
            texto.lines().nth(inyeccion.linea as usize - 1),
            Some("#[ignore]"),
            "{texto}"
        );
        assert!(
            texto.contains("#[ignore]\nfn lee_por_envoltorio()"),
            "{texto}"
        );
    }

    #[test]
    fn assert_borrado_apaga_los_dos_tests_del_mutante_unitario_y_de_socket() {
        let dir = repo_de(&[
            ("src/lib.rs", "pub mod db;\n"),
            (
                "src/db.rs",
                "pub fn guardar() -> u32 {\n    1\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn zset() {\n        assert_eq!(guardar(), 1);\n    }\n}\n",
            ),
            (
                "tests/red.rs",
                "#[test]\nfn zset() {\n    assert_eq!(1 + 1, 2);\n}\n",
            ),
        ]);
        let muerto = MutanteMuerto {
            archivo: "src/db.rs".into(),
            linea: 2,
            nombre: "src/db.rs:2:5: replace guardar -> u32 with 0".into(),
            tests: vec!["db::tests::zset".into(), "red::zset".into()],
        };
        let inyeccion = borrar_con(dir.path(), &[muerto], 42)
            .expect("E/S")
            .expect("aplica");
        let db = std::fs::read_to_string(dir.path().join("src/db.rs")).expect("lee");
        let red = std::fs::read_to_string(dir.path().join("tests/red.rs")).expect("lee");
        assert!(db.contains("    #[ignore]\n    fn zset()"), "{db}");
        assert!(red.contains("#[ignore]\nfn zset()"), "{red}");
        assert_eq!(
            inyeccion.tests_apagados,
            vec!["db::tests::zset".to_string(), "red::zset".to_string()]
        );
        // Con el mismo último segmento, `tests/` se busca antes que `src/`: el lugar cae en `tests/red.rs`.
        assert_eq!(
            inyeccion.archivo, "tests/red.rs",
            "el lugar es el primer apagado, tests/ antes que src/"
        );
        assert_eq!(inyeccion.linea, 2);
    }

    #[test]
    fn assert_borrado_lugar_es_el_primer_test_de_la_lista_no_el_primero_del_archivo() {
        let dir = copia_con_test_de_leer_numero();
        let muerto = MutanteMuerto {
            archivo: "src/protocolo.rs".into(),
            linea: 2,
            nombre:
                "src/protocolo.rs:2:1: replace leer_numero -> Result<u32, ParseIntError> with Ok(0)"
                    .into(),
            tests: vec![
                "protocolo::otro_test_sin_relacion".into(),
                "protocolo::leer_numero_lee_el_numero".into(),
            ],
        };
        let inyeccion = borrar_con(dir.path(), &[muerto], 42)
            .expect("E/S")
            .expect("aplica");
        assert_eq!(
            inyeccion.linea, 11,
            "la línea del #[ignore] de otro_test_sin_relacion"
        );
        let texto = std::fs::read_to_string(dir.path().join("tests/protocolo.rs")).expect("lee");
        assert_eq!(texto.lines().nth(10), Some("#[ignore]"), "{texto}");
        assert_eq!(inyeccion.tests_apagados.len(), 2);
    }

    #[test]
    fn assert_borrado_un_test_que_no_aparece_descarta_el_mutante_sin_cambios_parciales() {
        let dir = copia_con_test_de_leer_numero();
        let antes = arbol(dir.path());
        let mixto = MutanteMuerto {
            archivo: "src/protocolo.rs".into(),
            linea: 2,
            nombre:
                "src/protocolo.rs:2:1: replace leer_numero -> Result<u32, ParseIntError> with Ok(0)"
                    .into(),
            tests: vec![
                "protocolo::leer_numero_lee_el_numero".into(),
                "protocolo::no_existe".into(),
            ],
        };
        let sin_aplicar = borrar_con(dir.path(), &[mixto], 42).expect("E/S");
        assert_eq!(sin_aplicar, None, "un test de la lista no existe");
        assert_eq!(arbol(dir.path()), antes, "no escribe nada a medias");
    }

    #[test]
    fn assert_borrado_mutante_sin_tests_y_mutante_con_test_ausente_no_aplican() {
        let dir = copia_con_test_de_leer_numero();
        let antes = arbol(dir.path());
        let mixto = MutanteMuerto {
            archivo: "src/protocolo.rs".into(),
            linea: 2,
            nombre:
                "src/protocolo.rs:2:1: replace leer_numero -> Result<u32, ParseIntError> with Ok(0)"
                    .into(),
            tests: vec![
                "protocolo::leer_numero_lee_el_numero".into(),
                "protocolo::no_existe".into(),
            ],
        };
        for semilla in 1..=5u64 {
            let muertos = [muerto_sin_test(), mixto.clone()];
            assert_eq!(
                borrar_con(dir.path(), &muertos, semilla).expect("E/S"),
                None,
                "semilla {semilla}"
            );
        }
        assert_eq!(arbol(dir.path()), antes, "no toca el repo");
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
            inicial: None,
            camino_caliente: camino,
            limites: &limites,
            muertos: &[],
            cambios: None,
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

    /// Copia del fixture con una comparación de producción: `Almacen::cuenta_es`, en `src/almacen.rs`.
    /// El fixture no tiene ningún ` == `, así que el defecto necesita esta línea.
    fn copia_con_comparacion() -> tempfile::TempDir {
        let dir = copia_fixture();
        let ruta = dir.path().join("src/almacen.rs");
        let mut texto = std::fs::read_to_string(&ruta).expect("almacen.rs");
        texto.push_str(
            "\nimpl Almacen {\n    pub fn cuenta_es(&self, n: usize) -> bool {\n        self.datos.len() == n\n    }\n}\n",
        );
        std::fs::write(&ruta, texto).expect("escribe almacen.rs");
        dir
    }

    /// Repo mínimo con `archivos` (ruta relativa y contenido). Sin `Cargo.toml`: solo lo que lee el inyector.
    fn repo_de(archivos: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temporal");
        for (ruta, texto) in archivos {
            let ruta = dir.path().join(ruta);
            std::fs::create_dir_all(ruta.parent().expect("carpeta")).expect("carpeta");
            std::fs::write(ruta, texto).expect("escribe");
        }
        dir
    }

    #[test]
    fn inyectar_comportamiento_roto_cambia_un_igual_por_distinto_en_una_linea_de_produccion() {
        let dir = copia_con_comparacion();
        let antes = arbol(dir.path());
        let inyeccion = inyectar(
            Defecto::ComportamientoRoto,
            dir.path(),
            &capas_fixture(),
            42,
        )
        .expect("no falla la E/S")
        .expect("aplica");
        let despues = arbol(dir.path());
        assert_eq!(inyeccion.defecto, Defecto::ComportamientoRoto);
        assert_eq!(inyeccion.archivo, "src/almacen.rs");
        assert_eq!(cambiados(&antes, &despues), vec![inyeccion.archivo.clone()]);
        let lineas_antes: Vec<&str> = antes[&inyeccion.archivo].lines().collect();
        let lineas_despues: Vec<&str> = despues[&inyeccion.archivo].lines().collect();
        assert_eq!(
            lineas_antes.len(),
            lineas_despues.len(),
            "no agrega ni quita líneas"
        );
        let cambiadas: Vec<usize> = (0..lineas_antes.len())
            .filter(|&i| lineas_antes[i] != lineas_despues[i])
            .collect();
        assert_eq!(
            cambiadas,
            vec![inyeccion.linea as usize - 1],
            "solo cambia la línea del defecto"
        );
        let i = cambiadas[0];
        assert!(lineas_antes[i].contains(" == "), "{}", lineas_antes[i]);
        assert_eq!(
            lineas_despues[i],
            lineas_antes[i].replacen(" == ", " != ", 1)
        );
    }

    #[test]
    fn inyectar_comportamiento_roto_misma_semilla_mismo_lugar() {
        let repo = repo_de(&[
            ("src/lib.rs", "pub mod a;\npub mod b;\n"),
            (
                "src/a.rs",
                "pub fn uno(x: u32) -> bool {\n    x == 1\n}\n\npub fn dos(x: u32) -> bool {\n    x == 2\n}\n",
            ),
            ("src/b.rs", "pub fn tres(x: u32) -> bool {\n    x == 3\n}\n"),
        ]);
        let mut lugares = BTreeSet::new();
        for semilla in 1..=20u64 {
            let a = tempfile::tempdir().expect("temporal");
            let b = tempfile::tempdir().expect("temporal");
            copiar(repo.path(), a.path());
            copiar(repo.path(), b.path());
            let ia = inyectar(Defecto::ComportamientoRoto, a.path(), &[], semilla).expect("E/S");
            let ib = inyectar(Defecto::ComportamientoRoto, b.path(), &[], semilla).expect("E/S");
            assert_eq!(ia, ib, "semilla {semilla}");
            assert_eq!(arbol(a.path()), arbol(b.path()), "semilla {semilla}");
            lugares.extend(ia.map(|i| (i.archivo, i.linea)));
        }
        assert!(
            lugares.len() >= 2,
            "la semilla elige entre las líneas candidatas: {lugares:?}"
        );
    }

    #[test]
    fn inyectar_comportamiento_roto_sin_igualdad_de_produccion_no_es_aplicable() {
        let repo = repo_de(&[
            ("src/lib.rs", "pub fn uno() -> u32 {\n    1\n}\n"),
            (
                "src/otra.rs",
                "pub fn dos(x: u32) -> bool {\n    x > 0\n}\n",
            ),
        ]);
        for semilla in 1..=5u64 {
            let inyeccion =
                inyectar(Defecto::ComportamientoRoto, repo.path(), &[], semilla).expect("E/S");
            assert_eq!(inyeccion, None, "semilla {semilla}");
        }
    }

    #[test]
    fn inyectar_comportamiento_roto_no_elige_igualdad_de_cfg_test() {
        let repo = repo_de(&[(
            "src/lib.rs",
            "pub fn uno() -> u32 {\n    1\n}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn igual() {\n        assert!(1 == 1);\n    }\n}\n",
        )]);
        for semilla in 1..=5u64 {
            let inyeccion =
                inyectar(Defecto::ComportamientoRoto, repo.path(), &[], semilla).expect("E/S");
            assert_eq!(inyeccion, None, "semilla {semilla}");
        }
    }

    #[test]
    fn inyectar_comportamiento_roto_descarta_comentarios_textos_y_flechas() {
        let codigo = r#"// a == b
/// c == d
pub fn uno(x: u32) -> &'static str {
    let texto = "x == y";
    match x {
        1 => "uno",
        _ if x >= 2 == true => "mayor",
        _ => texto,
    }
}
pub fn dos(x: u32) -> bool {
    let y = 1; // x == 2
    if x == 3 { y == 1 } else { false } // comentario
}
"#;
        let repo = repo_de(&[("src/lib.rs", codigo)]);
        for semilla in 1..=5u64 {
            let dir = tempfile::tempdir().expect("temporal");
            copiar(repo.path(), dir.path());
            let inyeccion = inyectar(Defecto::ComportamientoRoto, dir.path(), &[], semilla)
                .expect("E/S")
                .expect("aplica");
            assert_eq!(inyeccion.archivo, "src/lib.rs", "semilla {semilla}");
            let nuevo = std::fs::read_to_string(dir.path().join("src/lib.rs")).expect("lib.rs");
            let linea = nuevo
                .lines()
                .nth(inyeccion.linea as usize - 1)
                .expect("línea");
            assert_eq!(
                linea, "    if x != 3 { y == 1 } else { false } // comentario",
                "semilla {semilla}"
            );
        }
    }

    #[test]
    fn inyectar_comportamiento_roto_la_copia_del_fixture_compila() {
        let dir = copia_con_comparacion();
        inyectar(
            Defecto::ComportamientoRoto,
            dir.path(),
            &capas_fixture(),
            42,
        )
        .expect("E/S")
        .expect("aplica");
        let objetivo = tempfile::tempdir().expect("temporal");
        if let Err(stderr) = cargo_check(dir.path(), objetivo.path()) {
            panic!("la copia con el defecto no compila:\n{stderr}");
        }
    }

    /// Fuente de 30 líneas en `src/lib.rs`: las líneas 3, 12, 15 y 25 tienen ` == `, las demás no.
    fn fuente_con_igualdades() -> String {
        (1..=30)
            .map(|n| {
                if [3, 12, 15, 25].contains(&n) {
                    format!("    let c{n} = x == {n};")
                } else {
                    format!("    let c{n} = x;")
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    }

    /// Rangos con un solo archivo, para `Contexto.cambios`.
    fn rangos_de(archivo: &str, rangos: &[(u32, u32)]) -> BTreeMap<String, Vec<(u32, u32)>> {
        BTreeMap::from([(archivo.to_string(), rangos.to_vec())])
    }

    /// `inyectar_con` de `defecto` en `dir`, con `cambios` y las capas del fixture.
    fn con_cambios(
        defecto: Defecto,
        dir: &Path,
        cambios: Option<&BTreeMap<String, Vec<(u32, u32)>>>,
        semilla: u64,
    ) -> io::Result<Option<Inyeccion>> {
        let capas = capas_fixture();
        let limites = LimitesEstructura::default();
        let contexto = Contexto {
            inicial: None,
            capas: &capas,
            camino_caliente: &[],
            limites: &limites,
            muertos: &[],
            cambios,
        };
        inyectar_con(defecto, dir, &contexto, semilla)
    }

    #[test]
    fn comportamiento_con_cambio_elige_solo_igualdades_dentro_del_rango() {
        let fuente = fuente_con_igualdades();
        let cambios = rangos_de("src/lib.rs", &[(10, 20)]);
        let mut lineas = BTreeSet::new();
        for semilla in 1..=20u64 {
            let dir = repo_de(&[("src/lib.rs", fuente.as_str())]);
            let inyeccion = con_cambios(
                Defecto::ComportamientoRoto,
                dir.path(),
                Some(&cambios),
                semilla,
            )
            .expect("no falla la E/S")
            .expect("hay igualdades dentro del cambio");
            assert!(
                (10..=20).contains(&inyeccion.linea),
                "semilla {semilla}: la línea {} cae fuera del cambio",
                inyeccion.linea
            );
            lineas.insert(inyeccion.linea);
        }
        assert_eq!(
            lineas,
            BTreeSet::from([12, 15]),
            "la semilla elige entre las dos igualdades del rango"
        );
    }

    #[test]
    fn comportamiento_con_cambio_sin_igualdades_no_es_aplicable() {
        let fuente = fuente_con_igualdades();
        let cambios = rangos_de("src/lib.rs", &[(1, 2)]);
        for semilla in 1..=5u64 {
            let dir = repo_de(&[("src/lib.rs", fuente.as_str())]);
            let antes = arbol(dir.path());
            let inyeccion = con_cambios(
                Defecto::ComportamientoRoto,
                dir.path(),
                Some(&cambios),
                semilla,
            )
            .expect("no falla la E/S");
            assert_eq!(
                inyeccion, None,
                "semilla {semilla}: las líneas 1 y 2 no tienen =="
            );
            assert_eq!(
                arbol(dir.path()),
                antes,
                "semilla {semilla}: no toca el repo"
            );
        }
    }

    #[test]
    fn modulo_borrado_con_cambio_elige_un_modulo_tocado() {
        let cambios = rangos_de("src/a.rs", &[(1, 1)]);
        for semilla in 1..=10u64 {
            let dir = repo_de(&[
                ("src/lib.rs", "pub mod a;\npub mod b;\n"),
                ("src/a.rs", "pub fn a() -> u32 {\n    1\n}\n"),
                ("src/b.rs", "pub fn b() -> u32 {\n    2\n}\n"),
            ]);
            let inyeccion =
                con_cambios(Defecto::ModuloBorrado, dir.path(), Some(&cambios), semilla)
                    .expect("no falla la E/S")
                    .expect("el módulo a está en el cambio");
            assert_eq!(inyeccion.archivo, "src/a.rs", "semilla {semilla}");
        }
    }

    #[test]
    fn sin_cambio_los_dos_defectos_de_linea_eligen_como_la_api_sin_contexto() {
        let fuente = fuente_con_igualdades();
        for defecto in [Defecto::ComportamientoRoto, Defecto::ModuloBorrado] {
            for semilla in 1..=10u64 {
                let repo = repo_de(&[
                    ("src/lib.rs", "pub mod a;\npub mod b;\n"),
                    ("src/a.rs", fuente.as_str()),
                    ("src/b.rs", "pub fn b(x: u32) -> bool {\n    x == 2\n}\n"),
                ]);
                let a = tempfile::tempdir().expect("temporal");
                let b = tempfile::tempdir().expect("temporal");
                copiar(repo.path(), a.path());
                copiar(repo.path(), b.path());
                let ia = inyectar(defecto, a.path(), &capas_fixture(), semilla).expect("E/S");
                let ib = con_cambios(defecto, b.path(), None, semilla).expect("E/S");
                assert_eq!(ia, ib, "{defecto:?} semilla {semilla}");
                assert_eq!(
                    arbol(a.path()),
                    arbol(b.path()),
                    "{defecto:?} semilla {semilla}"
                );
            }
        }
    }

    #[test]
    fn inyeccion_marca_en_cambio_solo_con_rangos_y_defecto_de_linea() {
        let fuente = fuente_con_igualdades();
        let cambios = rangos_de("src/lib.rs", &[(10, 20)]);
        let dir = repo_de(&[("src/lib.rs", fuente.as_str())]);
        let dentro = con_cambios(Defecto::ComportamientoRoto, dir.path(), Some(&cambios), 42)
            .expect("E/S")
            .expect("aplica");
        assert!(dentro.en_cambio, "{dentro:?}");
        let sin_rangos = repo_de(&[("src/lib.rs", fuente.as_str())]);
        let fuera = con_cambios(Defecto::ComportamientoRoto, sin_rangos.path(), None, 42)
            .expect("E/S")
            .expect("aplica");
        assert!(!fuera.en_cambio, "sin rangos no hay cambio: {fuera:?}");
        let otro = repo_de(&[("src/lib.rs", fuente.as_str())]);
        let secreto = con_cambios(Defecto::SecretoEnCodigo, otro.path(), Some(&cambios), 42)
            .expect("E/S")
            .expect("aplica");
        assert!(!secreto.en_cambio, "el secreto no es de línea: {secreto:?}");
    }

    /// `inyectar_con` de `ComportamientoRoto` en `dir`, con `cambios` y el estado inicial `inicial`.
    fn con_inicial(
        dir: &Path,
        cambios: &BTreeMap<String, Vec<(u32, u32)>>,
        inicial: &Path,
        semilla: u64,
    ) -> io::Result<Option<Inyeccion>> {
        let capas = capas_fixture();
        let limites = LimitesEstructura::default();
        let contexto = Contexto {
            capas: &capas,
            camino_caliente: &[],
            limites: &limites,
            muertos: &[],
            cambios: Some(cambios),
            inicial: Some(inicial),
        };
        inyectar_con(Defecto::ComportamientoRoto, dir, &contexto, semilla)
    }

    #[test]
    fn comportamiento_con_inicial_cambia_mayor_por_mayor_o_igual_dentro_del_cambio() {
        let cambios = rangos_de("src/lib.rs", &[(1, 3)]);
        let dir = repo_de(&[(
            "src/lib.rs",
            "pub fn uno(a: u32, b: u32) -> bool {\n    a > b\n}\n\npub fn dos(a: u32, b: u32) -> bool {\n    a > b\n}\n",
        )]);
        let inicial = repo_de(&[(
            "src/lib.rs",
            "pub fn uno(a: u32, b: u32) -> bool {\n    a > b\n}\n",
        )]);
        let inyeccion = con_inicial(dir.path(), &cambios, inicial.path(), 42)
            .expect("E/S")
            .expect("aplica");
        assert_eq!(inyeccion.archivo, "src/lib.rs");
        assert_eq!(inyeccion.linea, 2);
        assert_eq!(inyeccion.operador.as_deref(), Some("> -> >="));
        let nuevo = std::fs::read_to_string(dir.path().join("src/lib.rs")).expect("lib.rs");
        assert_eq!(nuevo.lines().nth(1), Some("    a >= b"));
        assert_eq!(nuevo.lines().nth(5), Some("    a > b"), "fuera del cambio");
    }

    #[test]
    fn comportamiento_con_inicial_descarta_el_cambio_que_el_inicial_ya_tenia() {
        let cambios = rangos_de("src/lib.rs", &[(1, 3)]);
        let vacio = repo_de(&[]);
        let dir = repo_de(&[(
            "src/lib.rs",
            "pub fn uno(a: u32, b: u32) -> bool {\n    a > b\n}\n",
        )]);
        let sin_revertir = con_inicial(dir.path(), &cambios, vacio.path(), 42)
            .expect("E/S")
            .expect("sin estado inicial que lo contenga, aplica `a >= b`");
        assert_eq!(sin_revertir.linea, 2);
        let dir = repo_de(&[(
            "src/lib.rs",
            "pub fn uno(a: u32, b: u32) -> bool {\n    a > b\n}\n",
        )]);
        let inicial = repo_de(&[(
            "src/lib.rs",
            "pub fn uno(a: u32, b: u32) -> bool {\n    a >= b\n}\n",
        )]);
        let antes = arbol(dir.path());
        for semilla in 1..=5u64 {
            let inyeccion =
                con_inicial(dir.path(), &cambios, inicial.path(), semilla).expect("E/S");
            assert_eq!(
                inyeccion, None,
                "semilla {semilla}: `a >= b` ya estaba en el inicial"
            );
        }
        assert_eq!(arbol(dir.path()), antes, "no toca la copia");
    }

    #[test]
    fn comportamiento_con_inicial_sin_el_archivo_no_descarta_nada() {
        let cambios = rangos_de("src/lib.rs", &[(1, 3)]);
        let inicial = repo_de(&[("src/otra.rs", "    a >= b\n")]);
        let dir = repo_de(&[(
            "src/lib.rs",
            "pub fn uno(a: u32, b: u32) -> bool {\n    a > b\n}\n",
        )]);
        let inyeccion = con_inicial(dir.path(), &cambios, inicial.path(), 42)
            .expect("E/S")
            .expect("aplica");
        assert_eq!(inyeccion.linea, 2);
        let nuevo = std::fs::read_to_string(dir.path().join("src/lib.rs")).expect("lib.rs");
        assert_eq!(nuevo.lines().nth(1), Some("    a >= b"));
    }

    #[test]
    fn comportamiento_con_inicial_if_ofrece_or_y_negacion_de_la_condicion() {
        let cambios = rangos_de("src/lib.rs", &[(1, 3)]);
        let inicial = repo_de(&[]);
        let mut vistas = BTreeSet::new();
        for semilla in 1..=40u64 {
            let dir = repo_de(&[(
                "src/lib.rs",
                "pub fn uno(x: bool, y: bool) -> u32 {\n    if x && y { 1 } else { 0 }\n}\n",
            )]);
            let inyeccion = con_inicial(dir.path(), &cambios, inicial.path(), semilla)
                .expect("E/S")
                .expect("aplica");
            assert_eq!(inyeccion.linea, 2, "semilla {semilla}");
            assert!(inyeccion.operador.is_some(), "semilla {semilla}");
            let nuevo = std::fs::read_to_string(dir.path().join("src/lib.rs")).expect("lib.rs");
            vistas.insert(nuevo.lines().nth(1).expect("línea 2").to_string());
        }
        assert_eq!(
            vistas,
            BTreeSet::from([
                "    if x || y { 1 } else { 0 }".to_string(),
                "    if !(x && y) { 1 } else { 0 }".to_string(),
            ])
        );
    }

    #[test]
    fn nombre_de_funcion_reconoce_calificadores_y_visibilidad() {
        assert_eq!(
            nombre_de_funcion("pub(crate) const fn f() -> u32 {"),
            Some("f")
        );
        assert_eq!(nombre_de_funcion("async unsafe fn g() {"), Some("g"));
        assert_eq!(nombre_de_funcion("pub(super) async fn h() {"), Some("h"));
        assert_eq!(nombre_de_funcion("const fn k() -> u32 {"), Some("k"));
        assert_eq!(nombre_de_funcion("pub unsafe fn m() {"), Some("m"));
        assert_eq!(
            nombre_de_funcion("pub fn procesar(x: u32) {"),
            Some("procesar")
        );
        assert_eq!(nombre_de_funcion("const LIMITE: u32 = 3;"), None);
        assert_eq!(nombre_de_funcion("let fn_x = 1;"), None);
    }

    #[test]
    fn agregar_negacion_envuelve_la_condicion_con_parentesis() {
        let linea = "    if x == 1 {";
        let agregar = candidatos_de_linea(linea)
            .into_iter()
            .find(|e| e.operador == "if: agregar !")
            .expect("ofrece agregar !");
        assert_eq!(agregar.aplicar(linea), "    if !(x == 1) {");
    }

    #[test]
    fn comportamiento_con_inicial_cambia_true_y_mas_uno_por_su_par() {
        let cambios = rangos_de("src/lib.rs", &[(1, 5)]);
        let inicial = repo_de(&[]);
        let mut vistas = BTreeSet::new();
        for semilla in 1..=40u64 {
            let dir = repo_de(&[(
                "src/lib.rs",
                "pub fn uno(x: u32) -> u32 {\n    let n = x + 1;\n    let ok = true;\n    n\n}\n",
            )]);
            let inyeccion = con_inicial(dir.path(), &cambios, inicial.path(), semilla)
                .expect("E/S")
                .expect("aplica");
            let nuevo = std::fs::read_to_string(dir.path().join("src/lib.rs")).expect("lib.rs");
            vistas.insert(
                nuevo
                    .lines()
                    .nth(inyeccion.linea as usize - 1)
                    .expect("línea")
                    .to_string(),
            );
        }
        assert_eq!(
            vistas,
            BTreeSet::from([
                "    let n = x - 1;".to_string(),
                "    let ok = false;".to_string(),
            ])
        );
    }

    #[test]
    fn comportamiento_con_inicial_ignora_operadores_en_textos_y_comentarios() {
        let cambios = rangos_de("src/lib.rs", &[(1, 5)]);
        let inicial = repo_de(&[]);
        let dir = repo_de(&[(
            "src/lib.rs",
            "pub fn uno(x: u32) -> u32 {\n    // x > 1\n    let t = \"a > b && if x\"; // x == y || z\n    x\n}\n",
        )]);
        let antes = arbol(dir.path());
        for semilla in 1..=5u64 {
            let inyeccion =
                con_inicial(dir.path(), &cambios, inicial.path(), semilla).expect("E/S");
            assert_eq!(inyeccion, None, "semilla {semilla}");
        }
        assert_eq!(arbol(dir.path()), antes);
    }

    /// Guarda: sin `inicial` la elección sigue siendo la de siempre. Mismo PRNG sobre las líneas de igualdad, en orden.
    #[test]
    fn comportamiento_sin_inicial_elige_con_la_semilla_como_antes() {
        let fuente = fuente_con_igualdades();
        for semilla in 1..=20u64 {
            let esperada = Prng::nuevo(semilla).elegir(&[3u32, 12, 15, 25]).copied();
            let dir = repo_de(&[("src/lib.rs", fuente.as_str())]);
            let linea = con_cambios(Defecto::ComportamientoRoto, dir.path(), None, semilla)
                .expect("E/S")
                .map(|i| i.linea);
            assert_eq!(linea, esperada, "semilla {semilla}");
        }
    }

    /// `inyectar_con` de `defecto` en `dir`, con `cambios` y el estado inicial `inicial`.
    fn con_inicial_de(
        defecto: Defecto,
        dir: &Path,
        cambios: &BTreeMap<String, Vec<(u32, u32)>>,
        inicial: &Path,
        semilla: u64,
    ) -> io::Result<Option<Inyeccion>> {
        let capas = capas_fixture();
        let limites = LimitesEstructura::default();
        let contexto = Contexto {
            capas: &capas,
            camino_caliente: &[],
            limites: &limites,
            muertos: &[],
            cambios: Some(cambios),
            inicial: Some(inicial),
        };
        inyectar_con(defecto, dir, &contexto, semilla)
    }

    #[test]
    fn cuerpo_vaciado_cambia_el_cuerpo_de_una_funcion_que_existia_y_el_cambio_toco() {
        let cambios = rangos_de("src/lib.rs", &[(2, 2)]);
        let inicial = repo_de(&[("src/lib.rs", "pub fn uno(x: u32) -> u32 {\n    x + 1\n}\n")]);
        let dir = repo_de(&[("src/lib.rs", "pub fn uno(x: u32) -> u32 {\n    x + 2\n}\n")]);
        let inyeccion = con_inicial_de(
            Defecto::CuerpoVaciado,
            dir.path(),
            &cambios,
            inicial.path(),
            42,
        )
        .expect("E/S")
        .expect("aplica");
        assert_eq!(inyeccion.archivo, "src/lib.rs");
        assert_eq!(inyeccion.linea, 1, "la línea es la de la firma");
        assert!(inyeccion.en_cambio, "{inyeccion:?}");
        let nuevo = std::fs::read_to_string(dir.path().join("src/lib.rs")).expect("lib.rs");
        assert_eq!(nuevo, "pub fn uno(x: u32) -> u32 {\n    todo!()\n}\n");
    }

    #[test]
    fn cuerpo_vaciado_no_elige_una_funcion_nueva_del_cambio() {
        let cambios = rangos_de("src/lib.rs", &[(1, 3)]);
        let inicial = repo_de(&[("src/lib.rs", "pub fn otra() -> u32 {\n    1\n}\n")]);
        for semilla in 1..=5u64 {
            let dir = repo_de(&[(
                "src/lib.rs",
                "pub fn nueva(x: u32) -> u32 {\n    x + 2\n}\n",
            )]);
            let antes = arbol(dir.path());
            let inyeccion = con_inicial_de(
                Defecto::CuerpoVaciado,
                dir.path(),
                &cambios,
                inicial.path(),
                semilla,
            )
            .expect("E/S");
            assert_eq!(
                inyeccion, None,
                "semilla {semilla}: `nueva` no estaba en el inicial"
            );
            assert_eq!(
                arbol(dir.path()),
                antes,
                "semilla {semilla}: no toca la copia"
            );
        }
    }

    #[test]
    fn cuerpo_vaciado_salta_declaraciones_cuerpos_de_una_linea_y_todo() {
        let codigo = "pub trait T {\n    fn f(&self) -> u32;\n}\n\npub fn uno() -> u32 { 1 }\n\npub fn dos() -> u32 {\n    todo!()\n}\n";
        let cambios = rangos_de("src/lib.rs", &[(1, 20)]);
        let inicial = repo_de(&[("src/lib.rs", codigo)]);
        for semilla in 1..=5u64 {
            let dir = repo_de(&[("src/lib.rs", codigo)]);
            let antes = arbol(dir.path());
            let inyeccion = con_inicial_de(
                Defecto::CuerpoVaciado,
                dir.path(),
                &cambios,
                inicial.path(),
                semilla,
            )
            .expect("E/S");
            assert_eq!(inyeccion, None, "semilla {semilla}");
            assert_eq!(arbol(dir.path()), antes, "semilla {semilla}");
        }
    }

    #[test]
    fn cuerpo_vaciado_cuenta_llaves_fuera_de_cadenas_y_firma_multilinea() {
        let codigo = "pub fn suma(\n    a: u32,\n    b: u32,\n) -> u32 {\n    let s = \"}{\"; // cierra } aquí\n    a + b\n}\n";
        let cambios = rangos_de("src/lib.rs", &[(1, 8)]);
        let inicial = repo_de(&[("src/lib.rs", codigo)]);
        let dir = repo_de(&[("src/lib.rs", codigo)]);
        let inyeccion = con_inicial_de(
            Defecto::CuerpoVaciado,
            dir.path(),
            &cambios,
            inicial.path(),
            42,
        )
        .expect("E/S")
        .expect("aplica");
        assert_eq!(inyeccion.linea, 1, "la firma empieza en la línea 1");
        let nuevo = std::fs::read_to_string(dir.path().join("src/lib.rs")).expect("lib.rs");
        assert_eq!(
            nuevo,
            "pub fn suma(\n    a: u32,\n    b: u32,\n) -> u32 {\n    todo!()\n}\n"
        );
    }

    #[test]
    fn cuerpo_vaciado_sin_inicial_aplica_en_un_repo_de_fixture() {
        let dir = copia_fixture();
        let antes = arbol(dir.path());
        let inyeccion = inyectar(Defecto::CuerpoVaciado, dir.path(), &capas_fixture(), 42)
            .expect("no falla la E/S")
            .expect("sin bootstrap/ cualquier función de producción sirve");
        assert_eq!(inyeccion.defecto, Defecto::CuerpoVaciado);
        assert!(
            !inyeccion.en_cambio,
            "sin cambio no hay línea tocada: {inyeccion:?}"
        );
        let despues = arbol(dir.path());
        assert_eq!(cambiados(&antes, &despues), vec![inyeccion.archivo.clone()]);
        assert!(despues[&inyeccion.archivo].contains("todo!()"));
    }

    /// Sin `inicial` cualquier función sirve. `en_cambio` dice si la elegida cruza una línea tocada.
    #[test]
    fn cuerpo_vaciado_sin_inicial_aplica_y_marca_si_cruza_el_cambio() {
        let dentro = rangos_de("src/lib.rs", &[(1, 3)]);
        let fuera = rangos_de("src/lib.rs", &[(10, 12)]);
        let codigo = "pub fn uno(x: u32) -> u32 {\n    x + 2\n}\n";
        let dir = repo_de(&[("src/lib.rs", codigo)]);
        let con = con_cambios(Defecto::CuerpoVaciado, dir.path(), Some(&dentro), 42)
            .expect("E/S")
            .expect("aplica");
        assert!(con.en_cambio, "la función cruza el cambio: {con:?}");
        let dir = repo_de(&[("src/lib.rs", codigo)]);
        let lejos = con_cambios(Defecto::CuerpoVaciado, dir.path(), Some(&fuera), 42)
            .expect("E/S")
            .expect("aplica igual: sin inicial cualquier función sirve");
        assert!(!lejos.en_cambio, "la función no cruza el cambio: {lejos:?}");
        let dir = repo_de(&[("src/lib.rs", codigo)]);
        let sin_cambios = con_cambios(Defecto::CuerpoVaciado, dir.path(), None, 42)
            .expect("E/S")
            .expect("aplica");
        assert!(!sin_cambios.en_cambio, "sin cambios no hay línea tocada");
    }

    #[test]
    fn modulo_borrado_con_inicial_no_aplica_aunque_haya_modulos_tocados() {
        let cambios = rangos_de("src/a.rs", &[(1, 1)]);
        let inicial = repo_de(&[]);
        for semilla in 1..=10u64 {
            let dir = repo_de(&[
                ("src/lib.rs", "pub mod a;\npub mod b;\n"),
                ("src/a.rs", "pub fn a() -> u32 {\n    1\n}\n"),
                ("src/b.rs", "pub fn b() -> u32 {\n    2\n}\n"),
            ]);
            let antes = arbol(dir.path());
            let inyeccion = con_inicial_de(
                Defecto::ModuloBorrado,
                dir.path(),
                &cambios,
                inicial.path(),
                semilla,
            )
            .expect("E/S");
            assert_eq!(inyeccion, None, "semilla {semilla}");
            assert_eq!(
                arbol(dir.path()),
                antes,
                "semilla {semilla}: no toca el repo"
            );
        }
    }

    /// Sin `inicial`, `modulo_borrado` elige entre los módulos declarados, en orden, con la misma semilla que antes.
    #[test]
    fn modulo_borrado_sin_inicial_elige_con_la_semilla_como_antes() {
        let rutas = ["src/a.rs", "src/b.rs", "src/c.rs"];
        let cambios: BTreeMap<String, Vec<(u32, u32)>> = rutas
            .iter()
            .map(|r| (r.to_string(), vec![(1, 1)]))
            .collect();
        for semilla in 1..=10u64 {
            let texto = [
                ("src/lib.rs", "pub mod c;\npub mod a;\npub mod b;\n"),
                ("src/a.rs", "pub fn a() -> u32 {\n    1\n}\n"),
                ("src/b.rs", "pub fn b() -> u32 {\n    2\n}\n"),
                ("src/c.rs", "pub fn c() -> u32 {\n    3\n}\n"),
            ];
            let esperada = Prng::nuevo(semilla).elegir(&rutas).copied();
            let dir = repo_de(&texto);
            let con = con_cambios(Defecto::ModuloBorrado, dir.path(), Some(&cambios), semilla)
                .expect("E/S")
                .map(|i| i.archivo);
            assert_eq!(con.as_deref(), esperada, "con cambios, semilla {semilla}");
            let dir = repo_de(&texto);
            let sin = inyectar(
                Defecto::ModuloBorrado,
                dir.path(),
                &capas_fixture(),
                semilla,
            )
            .expect("E/S")
            .map(|i| i.archivo);
            assert_eq!(sin.as_deref(), esperada, "sin cambios, semilla {semilla}");
        }
    }

    /// Guarda: la copia con `cuerpo_vaciado` compila en el fixture, en cada semilla probada.
    #[test]
    fn cuerpo_vaciado_la_copia_del_fixture_compila() {
        let cambios: BTreeMap<String, Vec<(u32, u32)>> =
            ["src/almacen.rs", "src/comandos.rs", "src/protocolo.rs"]
                .iter()
                .map(|r| (r.to_string(), vec![(1, 100)]))
                .collect();
        let objetivo = tempfile::tempdir().expect("temporal");
        for semilla in 1..=8u64 {
            let dir = copia_fixture();
            let inicial = copia_fixture();
            let inyeccion = con_inicial_de(
                Defecto::CuerpoVaciado,
                dir.path(),
                &cambios,
                inicial.path(),
                semilla,
            )
            .expect("E/S")
            .expect("aplica");
            if let Err(stderr) = cargo_check(dir.path(), objetivo.path()) {
                panic!("semilla {semilla}: la copia con {inyeccion:?} no compila:\n{stderr}");
            }
        }
    }
}
