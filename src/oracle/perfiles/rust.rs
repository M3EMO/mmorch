//! Sondas nativas del perfil `rust`: leen los fuentes, sin herramientas externas.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::Path;

use super::herramientas::sin_veredicto;
use super::{ResultadoSonda, crear_hallazgo, modulo_de, modulo_de_capa, puntuar_hallazgos};
use crate::goal::calcular_hash;
use crate::oracle::tarea::Requisito;
use crate::tipos::{Dimension, Hallazgo};

const SONDA_SEGURIDAD: &str = "seguridad-nativa";
const SONDA_ESTRUCTURA: &str = "estructura-nativa";
const SONDA_ARQUITECTURA: &str = "arquitectura-nativa";
/// Texto con que la sonda de arquitectura nombra un ciclo de módulos. El juez lo usa para el tope.
pub const REGLA_CICLO: &str = "ciclo de módulos";
/// Inicio del texto de una capa sin módulo en el repo. `capas.rs` lo usa para reconocer esa falta.
pub(crate) const FALTA_CAPA: &str = "falta la capa";

/// Límites de la sonda de estructura. Por defecto: 500 líneas, 80 por función, 10 de bloque.
pub struct LimitesEstructura {
    pub archivo_lineas: usize,
    pub funcion_lineas: usize,
    pub bloque_lineas: usize,
}

impl Default for LimitesEstructura {
    fn default() -> Self {
        LimitesEstructura {
            archivo_lineas: 500,
            funcion_lineas: 80,
            bloque_lineas: 10,
        }
    }
}

/// Un fuente `.rs` del repo: ruta relativa con `/` y sus líneas.
pub(crate) struct Archivo {
    pub(crate) ruta: String,
    pub(crate) lineas: Vec<String>,
}

/// Una línea con su número, su texto crudo y su texto sin comentarios ni contenido de cadenas.
/// `limpia` tiene el mismo largo en bytes que la línea cruda sin recortar: sus partes en blanco son espacios.
pub(crate) struct Linea {
    pub(crate) numero: u32,
    pub(crate) cruda: String,
    pub(crate) limpia: String,
    /// Verdadero dentro de un bloque `#[cfg(test)]`.
    pub(crate) de_test: bool,
}

/// Una arista entre módulos: el primer uso del módulo origen hacia el destino.
pub(crate) struct Uso {
    pub(crate) archivo: String,
    pub(crate) linea: u32,
    pub(crate) cita: String,
}

/// Seguridad: cada `unsafe` (severidad 2) y cada `unwrap()`/`expect(` (severidad 1) fuera de tests.
pub fn seguridad_nativa(repo: &Path, requisitos: &[Requisito]) -> ResultadoSonda {
    let archivos = match fuentes_o_sin_veredicto(repo, Dimension::Seguridad) {
        Ok(archivos) => archivos,
        Err(sin) => return sin,
    };
    let mut hallazgos = Vec::new();
    for archivo in &archivos {
        if es_archivo_de_test(&archivo.ruta) {
            continue;
        }
        for linea in analizar(&archivo.lineas).iter().filter(|l| !l.de_test) {
            if tiene_token(&linea.limpia, "unsafe") {
                hallazgos.push(crear_hallazgo(
                    Dimension::Seguridad,
                    2,
                    &archivo.ruta,
                    linea.numero,
                    &linea.cruda,
                    "unsafe fuera de tests",
                    SONDA_SEGURIDAD,
                    requisitos,
                ));
            }
            if linea.limpia.contains(".unwrap()") || linea.limpia.contains(".expect(") {
                hallazgos.push(crear_hallazgo(
                    Dimension::Seguridad,
                    1,
                    &archivo.ruta,
                    linea.numero,
                    &linea.cruda,
                    "unwrap() o expect() fuera de tests",
                    SONDA_SEGURIDAD,
                    requisitos,
                ));
            }
        }
    }
    resultado(Dimension::Seguridad, hallazgos, requisitos)
}

/// Estructura: archivos y funciones largos (severidad 2) y bloques duplicados (severidad 1).
pub fn estructura_nativa(
    repo: &Path,
    requisitos: &[Requisito],
    limites: &LimitesEstructura,
) -> ResultadoSonda {
    let archivos = match fuentes_o_sin_veredicto(repo, Dimension::Estructura) {
        Ok(archivos) => archivos,
        Err(sin) => return sin,
    };
    let mut hallazgos = Vec::new();
    for archivo in &archivos {
        if archivo.lineas.len() > limites.archivo_lineas {
            hallazgos.push(archivo_grande(archivo, limites, requisitos));
        }
        hallazgos.extend(funciones_grandes(archivo, limites, requisitos));
    }
    hallazgos.extend(bloques_duplicados(&archivos, limites, requisitos));
    resultado(Dimension::Estructura, hallazgos, requisitos)
}

/// Motivo de la sonda de arquitectura sin capas declaradas y sin módulos: no hay nada que medir. La
/// parte de sondas de Arquitectura lo suma a `faltantes` (desempate 6).
pub const MOTIVO_SIN_MODULOS: &str = "sin módulos";

/// Arquitectura: ciclos entre módulos (severidad 3), usos de una capa anterior (severidad 3) y capas
/// declaradas enteras que faltan en el repo (severidad 3). Una capa es un grupo de nombres de
/// implementaciones distintas: está presente si existe al menos uno de sus módulos. La cita es el mapa de
/// módulos del repo y la capa.
pub fn arquitectura_nativa(
    repo: &Path,
    capas: &[Vec<String>],
    requisitos: &[Requisito],
) -> ResultadoSonda {
    let archivos = match fuentes_o_sin_veredicto(repo, Dimension::Arquitectura) {
        Ok(archivos) => archivos,
        Err(sin) => return sin,
    };
    let modulos: BTreeSet<String> = archivos.iter().filter_map(|a| modulo_de(&a.ruta)).collect();
    // Sin capas declaradas y sin módulos (un workspace de crates, por ejemplo), no hay nada que medir.
    if modulos.is_empty() && capas.is_empty() {
        return sin_veredicto(Dimension::Arquitectura, MOTIVO_SIN_MODULOS);
    }
    // Una capa prometida que no existe es evidencia. Sin módulos, las demás reglas no encuentran nada.
    let presentes = modulos_presentes(&archivos);
    let mut hallazgos = capas_sin_modulo(capas, &modulos, &presentes, requisitos);
    let aristas = aristas_de_capas(&archivos, &modulos);
    let adj = adyacencias(&aristas, &modulos);
    for modulo in &modulos {
        let Some(camino) = ciclo_de(modulo, &adj) else {
            continue;
        };
        if let Some(uso) = aristas.get(&(modulo.clone(), camino[1].clone())) {
            hallazgos.push(crear_hallazgo(
                Dimension::Arquitectura,
                3,
                &uso.archivo,
                uso.linea,
                &uso.cita,
                &format!("{REGLA_CICLO}: {}", camino.join(" -> ")),
                SONDA_ARQUITECTURA,
                requisitos,
            ));
        }
    }
    for ((origen, destino), uso) in &aristas {
        let (Some(i), Some(j)) = (capa_de(capas, origen), capa_de(capas, destino)) else {
            continue;
        };
        if j < i {
            hallazgos.push(crear_hallazgo(
                Dimension::Arquitectura,
                3,
                &uso.archivo,
                uso.linea,
                &uso.cita,
                &format!("{origen} (capa {i}) importa {destino} (capa {j})"),
                SONDA_ARQUITECTURA,
                requisitos,
            ));
        }
    }
    resultado(Dimension::Arquitectura, hallazgos, requisitos)
}

fn resultado(dimension: Dimension, hallazgos: Vec<Hallazgo>, reqs: &[Requisito]) -> ResultadoSonda {
    ResultadoSonda {
        dimension,
        valor: Some(puntuar_hallazgos(&hallazgos, reqs)),
        motivo: None,
        hallazgos,
    }
}

/// Todos los `*.rs` del repo, salvo los de `target/` y las carpetas ocultas (`.git`, por ejemplo).
/// Un error de lectura falla entero: con un archivo sin leer, la sonda no sabe qué falta (A9).
pub(crate) fn archivos_rs(repo: &Path) -> Result<Vec<Archivo>, String> {
    let mut salida = Vec::new();
    recorrer(repo, repo, &mut salida)?;
    salida.sort_by(|a, b| a.ruta.cmp(&b.ruta));
    Ok(salida)
}

/// Módulos presentes en el repo: los de `modulo_de` más `main` y `lib` si existe su archivo. La regla de
/// capas de Arquitectura y el informe (qué falta de un requisito) usan este mismo conjunto.
pub(crate) fn modulos_presentes(archivos: &[Archivo]) -> BTreeSet<String> {
    archivos
        .iter()
        .filter_map(|a| modulo_de_capa(&a.ruta))
        .collect()
}

/// Los fuentes que una sonda necesita. Un error de lectura o un repo sin `.rs` no da veredicto (A9).
pub(crate) fn fuentes_o_sin_veredicto(
    repo: &Path,
    dimension: Dimension,
) -> Result<Vec<Archivo>, ResultadoSonda> {
    let archivos = archivos_rs(repo).map_err(|motivo| sin_veredicto(dimension, motivo))?;
    if archivos.is_empty() {
        return Err(sin_veredicto(dimension, "no hay archivos .rs en el repo"));
    }
    Ok(archivos)
}

fn recorrer(repo: &Path, dir: &Path, salida: &mut Vec<Archivo>) -> Result<(), String> {
    let rel = |p: &Path| {
        p.strip_prefix(repo)
            .unwrap_or(p)
            .to_string_lossy()
            .replace('\\', "/")
    };
    let entradas =
        fs::read_dir(dir).map_err(|e| format!("no se pudo leer la carpeta {}: {e}", rel(dir)))?;
    for entrada in entradas {
        let entrada = entrada.map_err(|e| format!("no se pudo leer {}: {e}", rel(dir)))?;
        let ruta = entrada.path();
        if ruta.is_dir() {
            let nombre = entrada.file_name().to_string_lossy().into_owned();
            if nombre != "target" && !nombre.starts_with('.') {
                recorrer(repo, &ruta, salida)?;
            }
        } else if ruta.extension().is_some_and(|e| e == "rs") {
            let texto = fs::read_to_string(&ruta)
                .map_err(|e| format!("no se pudo leer {}: {e}", rel(&ruta)))?;
            salida.push(Archivo {
                ruta: rel(&ruta),
                lineas: texto.lines().map(String::from).collect(),
            });
        }
    }
    Ok(())
}

/// Test por ruta: carpeta `tests`, o nombre `*_test.rs` / `*_tests.rs`.
fn es_archivo_de_test(ruta: &str) -> bool {
    ruta.split('/').any(|c| c == "tests")
        || ruta.ends_with("_test.rs")
        || ruta.ends_with("_tests.rs")
}

/// Estado del limpiador al empezar una línea. Un comentario o una cadena pueden seguir en la línea siguiente.
#[derive(Clone, Copy, Default)]
enum Estado {
    #[default]
    Codigo,
    /// Dentro de `/* ... */`. Cuenta los `/*` sin cerrar, porque los comentarios de bloque se anidan.
    Bloque(u32),
    /// Dentro de una cadena `"..."` que sigue desde una línea anterior.
    Cadena,
    /// Dentro de una cadena cruda `r#"..."#`. Guarda cuántas `#` cierran la cadena.
    CadenaCruda(usize),
}

/// Limpia las líneas de un fuente en orden. Recuerda el estado entre una línea y la siguiente.
#[derive(Default)]
struct Limpiador {
    estado: Estado,
}

impl Limpiador {
    /// Cambia cada comentario y cada contenido de cadena por espacios, un espacio por byte.
    /// El largo en bytes no cambia. Los delimitadores `"`, `r#"` y `"#` se conservan.
    fn limpiar(&mut self, linea: &str) -> String {
        let cs: Vec<char> = linea.chars().collect();
        let mut salida = String::with_capacity(linea.len());
        let mut i = 0;
        while i < cs.len() {
            let estado = self.estado;
            i += match estado {
                Estado::Codigo => self.en_codigo(&cs, i, &mut salida),
                Estado::Bloque(n) => self.en_bloque(n, &cs, i, &mut salida),
                Estado::Cadena => self.en_cadena(&cs, i, &mut salida),
                Estado::CadenaCruda(n) => self.en_cadena_cruda(n, &cs, i, &mut salida),
            };
        }
        salida
    }

    /// Procesa código desde `cs[i]`. Devuelve cuántos caracteres consumió.
    fn en_codigo(&mut self, cs: &[char], i: usize, salida: &mut String) -> usize {
        match (cs[i], cs.get(i + 1).copied()) {
            ('/', Some('/')) => {
                // El resto de la línea es comentario.
                tramo_en_blanco(salida, &cs[i..]);
                cs.len() - i
            }
            ('/', Some('*')) => {
                self.estado = Estado::Bloque(1);
                tramo_en_blanco(salida, &cs[i..i + 2]);
                2
            }
            ('"', _) => {
                self.estado = Estado::Cadena;
                salida.push('"');
                1
            }
            ('r', _) => match inicio_de_cadena_cruda(cs, i) {
                Some(n) => {
                    self.estado = Estado::CadenaCruda(n);
                    salida.extend(&cs[i..i + n + 2]);
                    n + 2
                }
                None => {
                    salida.push('r');
                    1
                }
            },
            ('\'', _) => match largo_de_caracter(cs, i) {
                // Un literal de carácter se copia tal cual: su comilla no abre cadena.
                Some(largo) => {
                    salida.extend(&cs[i..i + largo]);
                    largo
                }
                // Sin cierre en esa forma es un lifetime: solo se copia la comilla.
                None => {
                    salida.push('\'');
                    1
                }
            },
            (c, _) => {
                salida.push(c);
                1
            }
        }
    }

    /// Procesa un comentario de bloque desde `cs[i]`, con `n` niveles abiertos.
    fn en_bloque(&mut self, n: u32, cs: &[char], i: usize, salida: &mut String) -> usize {
        match (cs[i], cs.get(i + 1).copied()) {
            ('/', Some('*')) => {
                self.estado = Estado::Bloque(n + 1);
                tramo_en_blanco(salida, &cs[i..i + 2]);
                2
            }
            ('*', Some('/')) => {
                self.estado = if n <= 1 {
                    Estado::Codigo
                } else {
                    Estado::Bloque(n - 1)
                };
                tramo_en_blanco(salida, &cs[i..i + 2]);
                2
            }
            (c, _) => {
                tramo_en_blanco(salida, &[c]);
                1
            }
        }
    }

    /// Procesa el contenido de una cadena `"..."` desde `cs[i]`.
    fn en_cadena(&mut self, cs: &[char], i: usize, salida: &mut String) -> usize {
        match cs[i] {
            '\\' => {
                // La barra escapa el carácter que sigue. Si es el último de la línea, la cadena sigue.
                let largo = if i + 1 < cs.len() { 2 } else { 1 };
                tramo_en_blanco(salida, &cs[i..i + largo]);
                largo
            }
            '"' => {
                self.estado = Estado::Codigo;
                salida.push('"');
                1
            }
            c => {
                tramo_en_blanco(salida, &[c]);
                1
            }
        }
    }

    /// Procesa el contenido de una cadena cruda desde `cs[i]`. No tiene escapes.
    fn en_cadena_cruda(&mut self, n: usize, cs: &[char], i: usize, salida: &mut String) -> usize {
        let cierra =
            cs[i] == '"' && cs.len() >= i + 1 + n && cs[i + 1..i + 1 + n].iter().all(|&c| c == '#');
        if cierra {
            self.estado = Estado::Codigo;
            salida.extend(&cs[i..i + 1 + n]);
            1 + n
        } else {
            tramo_en_blanco(salida, &[cs[i]]);
            1
        }
    }
}

/// Escribe un espacio por cada byte del carácter `c`.
fn tramo_en_blanco(salida: &mut String, tramo: &[char]) {
    for c in tramo {
        for _ in 0..c.len_utf8() {
            salida.push(' ');
        }
    }
}

/// Si `cs[i]` es una `r` que abre una cadena cruda (`r"`, `r#"`, `br"`), da cuántas `#` tiene.
fn inicio_de_cadena_cruda(cs: &[char], i: usize) -> Option<usize> {
    let previo_ok = match i {
        0 => true,
        _ if cs[i - 1] == 'b' => i < 2 || !es_ident(cs[i - 2]),
        _ => !es_ident(cs[i - 1]),
    };
    if !previo_ok {
        return None;
    }
    let n = cs[i + 1..].iter().take_while(|&&c| c == '#').count();
    (cs.get(i + 1 + n) == Some(&'"')).then_some(n)
}

/// Largo del literal de carácter que empieza en `cs[i]`: `'x'`, `'\n'` o `'\u{..}'`. `None` si es un lifetime.
fn largo_de_caracter(cs: &[char], i: usize) -> Option<usize> {
    if cs.get(i + 1) == Some(&'\\') {
        // La barra escapa el carácter que sigue, que puede ser una comilla.
        cs.iter()
            .skip(i + 3)
            .position(|&c| c == '\'')
            .map(|p| p + 4)
    } else {
        (cs.get(i + 2) == Some(&'\'')).then_some(3)
    }
}

/// Marca cada línea como de test o no. Un bloque `#[cfg(test)]` cuenta con sus llaves.
pub(crate) fn analizar(texto: &[String]) -> Vec<Linea> {
    let mut profundidad: i32 = 0;
    let mut bloque_test: Option<i32> = None;
    let mut pendiente = false;
    let mut limpiador = Limpiador::default();
    let mut salida = Vec::with_capacity(texto.len());
    for (indice, cruda) in texto.iter().enumerate() {
        let limpia = limpiador.limpiar(cruda);
        let mut de_test = bloque_test.is_some();
        if limpia.contains("#[cfg(test)]") {
            pendiente = true;
        } else if pendiente && limpia.contains('{') {
            bloque_test = Some(profundidad);
            pendiente = false;
            de_test = true;
        } else if pendiente && limpia.contains(';') {
            pendiente = false;
        }
        for c in limpia.chars() {
            match c {
                '{' => profundidad += 1,
                '}' => profundidad -= 1,
                _ => {}
            }
        }
        if bloque_test.is_some_and(|p| profundidad <= p) {
            bloque_test = None;
        }
        salida.push(Linea {
            numero: indice as u32 + 1,
            cruda: cruda.trim().to_string(),
            limpia,
            de_test,
        });
    }
    salida
}

/// Verdadero si `palabra` aparece como token completo (no dentro de `unsafe_code`).
pub(crate) fn tiene_token(linea: &str, palabra: &str) -> bool {
    linea
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .any(|t| t == palabra)
}

fn archivo_grande(archivo: &Archivo, limites: &LimitesEstructura, reqs: &[Requisito]) -> Hallazgo {
    let cita = archivo
        .lineas
        .iter()
        .map(|l| l.trim())
        .find(|l| !l.is_empty())
        .unwrap_or(archivo.ruta.as_str());
    crear_hallazgo(
        Dimension::Estructura,
        2,
        &archivo.ruta,
        1,
        cita,
        &format!(
            "archivo grande: {} líneas (límite {})",
            archivo.lineas.len(),
            limites.archivo_lineas
        ),
        SONDA_ESTRUCTURA,
        reqs,
    )
}

fn funciones_grandes(
    archivo: &Archivo,
    limites: &LimitesEstructura,
    reqs: &[Requisito],
) -> Vec<Hallazgo> {
    let lineas = analizar(&archivo.lineas);
    let limpias: Vec<&str> = lineas.iter().map(|l| l.limpia.as_str()).collect();
    let mut salida = Vec::new();
    for (i, linea) in lineas.iter().enumerate() {
        if !tiene_token(&linea.limpia, "fn") {
            continue;
        }
        let Some(fin) = fin_del_cuerpo(&limpias, i) else {
            continue;
        };
        let largo = fin - i + 1;
        if largo > limites.funcion_lineas {
            salida.push(crear_hallazgo(
                Dimension::Estructura,
                2,
                &archivo.ruta,
                linea.numero,
                &linea.cruda,
                &format!(
                    "función de {largo} líneas (límite {})",
                    limites.funcion_lineas
                ),
                SONDA_ESTRUCTURA,
                reqs,
            ));
        }
    }
    salida
}

/// Línea donde cierra el cuerpo de la función que empieza en `inicio`. `None` si no tiene cuerpo.
fn fin_del_cuerpo(limpias: &[&str], inicio: usize) -> Option<usize> {
    let mut profundidad = 0;
    let mut abierto = false;
    for (j, texto) in limpias.iter().enumerate().skip(inicio) {
        for c in texto.chars() {
            match c {
                '{' => {
                    profundidad += 1;
                    abierto = true;
                }
                '}' => {
                    profundidad -= 1;
                    if abierto && profundidad == 0 {
                        return Some(j);
                    }
                }
                ';' if !abierto => return None,
                _ => {}
            }
        }
    }
    None
}

/// Bloques de `bloque_lineas` líneas no triviales repetidos en dos o más lugares.
/// Se reporta una vez por bloque, en su primer lugar.
fn bloques_duplicados(
    archivos: &[Archivo],
    limites: &LimitesEstructura,
    reqs: &[Requisito],
) -> Vec<Hallazgo> {
    let ventana = limites.bloque_lineas.max(1);
    let sig: Vec<Vec<(u32, String, String)>> = archivos.iter().map(lineas_significativas).collect();
    // Hash de cada ventana y sus lugares: (archivo, posición).
    let mut lugares: BTreeMap<String, Vec<(usize, usize)>> = BTreeMap::new();
    let mut hashes: Vec<Vec<String>> = Vec::new();
    for (f, lineas) in sig.iter().enumerate() {
        let mut hs = Vec::new();
        for (p, bloque) in lineas.windows(ventana).enumerate() {
            let texto: Vec<&str> = bloque.iter().map(|l| l.1.as_str()).collect();
            let h = calcular_hash(texto.join("\n").as_bytes());
            lugares.entry(h.clone()).or_default().push((f, p));
            hs.push(h);
        }
        hashes.push(hs);
    }
    // Tramos de ventanas repetidas, agrupados por su primera ventana y su largo.
    let mut tramos: BTreeMap<String, Vec<(usize, usize, usize)>> = BTreeMap::new();
    for (f, hs) in hashes.iter().enumerate() {
        let repetida = |p: usize| lugares.get(&hs[p]).is_some_and(|v| v.len() >= 2);
        let mut p = 0;
        while p < hs.len() {
            if !repetida(p) {
                p += 1;
                continue;
            }
            let inicio = p;
            while p < hs.len() && repetida(p) {
                p += 1;
            }
            let clave = format!("{}:{}", hs[inicio], p - inicio);
            tramos
                .entry(clave)
                .or_default()
                .push((f, inicio, p - inicio));
        }
    }
    let mut salida = Vec::new();
    for grupo in tramos.values().filter(|g| g.len() >= 2) {
        let (f, inicio, largo) = grupo[0];
        let (linea, _, cita) = &sig[f][inicio];
        let otros: Vec<String> = grupo[1..]
            .iter()
            .map(|(of, op, _)| format!("{}:{}", archivos[*of].ruta, sig[*of][*op].0))
            .collect();
        salida.push(crear_hallazgo(
            Dimension::Estructura,
            1,
            &archivos[f].ruta,
            *linea,
            cita,
            &format!(
                "bloque de {} líneas repetido en {}",
                largo + ventana - 1,
                otros.join(", ")
            ),
            SONDA_ESTRUCTURA,
            reqs,
        ));
    }
    salida
}

/// Líneas que cuentan para duplicación: (número, texto sin espacios, cita). Sin vacías ni `{}();,`.
fn lineas_significativas(archivo: &Archivo) -> Vec<(u32, String, String)> {
    analizar(&archivo.lineas)
        .into_iter()
        .filter_map(|l| {
            let sin_comentario = l.cruda.split("//").next().unwrap_or_default();
            let norm: String = sin_comentario
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            if norm.chars().all(|c| "{}();,".contains(c)) {
                return None;
            }
            Some((l.numero, norm, l.cruda))
        })
        .collect()
}

/// Aristas `origen -> destino` por `crate::X` fuera de tests. Guarda el primer uso de cada una.
pub(crate) fn aristas_entre_modulos(
    archivos: &[Archivo],
    modulos: &BTreeSet<String>,
) -> BTreeMap<(String, String), Uso> {
    aristas_de(archivos, modulos, false)
}

/// Aristas de la regla de capas: como `aristas_entre_modulos`, pero la raíz (`main`, `lib`) también es origen,
/// y un `crate::X` con X que no es módulo es una arista hacia la raíz que declara el módulo de origen.
pub(crate) fn aristas_de_capas(
    archivos: &[Archivo],
    modulos: &BTreeSet<String>,
) -> BTreeMap<(String, String), Uso> {
    aristas_de(archivos, modulos, true)
}

fn aristas_de(
    archivos: &[Archivo],
    modulos: &BTreeSet<String>,
    con_raiz: bool,
) -> BTreeMap<(String, String), Uso> {
    let mut aristas = BTreeMap::new();
    for archivo in archivos {
        let origen = if con_raiz {
            modulo_de_capa(&archivo.ruta)
        } else {
            modulo_de(&archivo.ruta)
        };
        let Some(origen) = origen else {
            continue;
        };
        // Un archivo de la raíz no tiene raíz que lo declare: sus `crate::X` solo cuentan si X es módulo.
        let es_modulo = modulo_de(&archivo.ruta).is_some();
        for uso in usos_de_crate_en(&analizar(&archivo.lineas)) {
            let destino = if modulos.contains(&uso.nombre) {
                uso.nombre
            } else if con_raiz && es_modulo {
                match raiz_de_modulo(&origen, archivos) {
                    Some(raiz) => raiz.to_string(),
                    None => continue,
                }
            } else {
                continue;
            };
            if destino != origen {
                aristas
                    .entry((origen.clone(), destino))
                    .or_insert_with(|| Uso {
                        archivo: archivo.ruta.clone(),
                        linea: uso.linea,
                        cita: uso.cita,
                    });
            }
        }
    }
    aristas
}

/// Raíz del crate que declara `modulo`: `lib` si `src/lib.rs` lo declara con `mod`; si no, `main` si existe
/// `src/main.rs`; si no, `lib` si existe `src/lib.rs`. Sin raíz, `None`.
fn raiz_de_modulo(modulo: &str, archivos: &[Archivo]) -> Option<&'static str> {
    let lib = archivos.iter().find(|a| a.ruta == "src/lib.rs");
    if lib.is_some_and(|a| declara_modulo(a, modulo)) {
        return Some("lib");
    }
    if archivos.iter().any(|a| a.ruta == "src/main.rs") {
        return Some("main");
    }
    lib.map(|_| "lib")
}

/// Verdadero si el archivo declara `mod <modulo>;`, con `pub` o `pub(crate)`, fuera de tests.
fn declara_modulo(archivo: &Archivo, modulo: &str) -> bool {
    let formas = [
        format!("mod {modulo};"),
        format!("pub mod {modulo};"),
        format!("pub(crate) mod {modulo};"),
    ];
    analizar(&archivo.lineas)
        .iter()
        .any(|l| !l.de_test && formas.iter().any(|f| l.limpia.trim() == f))
}

/// Un `crate::X` de un fuente: X, la línea donde aparece X y la línea cruda como cita.
struct UsoDeCrate {
    nombre: String,
    linea: u32,
    cita: String,
}

/// Usos `crate::X` fuera de tests. Una sentencia `use` se lee entera, hasta su `;`, aunque ocupe varias
/// líneas. El resto de líneas se lee una a una, como antes.
fn usos_de_crate_en(lineas: &[Linea]) -> Vec<UsoDeCrate> {
    let mut salida = Vec::new();
    let mut i = 0;
    while i < lineas.len() {
        let linea = &lineas[i];
        if linea.de_test {
            i += 1;
        } else if es_inicio_de_use(&linea.limpia) {
            let fin = (i..lineas.len())
                .find(|&k| lineas[k].limpia.contains(';'))
                .unwrap_or(lineas.len() - 1);
            let mut texto = String::new();
            let mut inicios = Vec::new();
            for (k, l) in lineas[i..=fin].iter().enumerate() {
                inicios.push(texto.len());
                // Solo la primera línea puede llevar `pub(in ...)`. Su camino no es destino.
                if k == 0 {
                    texto.push_str(&sin_visibilidad(&l.limpia));
                } else {
                    texto.push_str(&l.limpia);
                }
                texto.push('\n');
            }
            for (nombre, byte) in nombres_en(&texto) {
                // La línea de X es la última que empieza antes de su byte.
                let k = inicios.partition_point(|&s| s <= byte) - 1;
                let l = &lineas[i + k];
                salida.push(UsoDeCrate {
                    nombre,
                    linea: l.numero,
                    cita: l.cruda.clone(),
                });
            }
            i = fin + 1;
        } else {
            for (nombre, _) in nombres_en(&linea.limpia) {
                salida.push(UsoDeCrate {
                    nombre,
                    linea: linea.numero,
                    cita: linea.cruda.clone(),
                });
            }
            i += 1;
        }
    }
    salida
}

/// Verdadero si la línea empieza una sentencia `use`, con o sin `pub`, `pub(crate)` o `pub(in ...)`.
fn es_inicio_de_use(limpia: &str) -> bool {
    let mut resto = limpia.trim_start();
    if let Some(tras_pub) = resto.strip_prefix("pub") {
        let tras_pub = tras_pub.trim_start();
        resto = match tras_pub.strip_prefix('(') {
            Some(dentro) => match dentro.find(')') {
                Some(fin) => dentro[fin + 1..].trim_start(),
                None => return false,
            },
            None => tras_pub,
        };
    }
    resto
        .strip_prefix("use")
        .is_some_and(|r| r.is_empty() || r.starts_with(|c: char| c.is_whitespace() || c == '{'))
}

/// Cambia el `pub(...)` del inicio de la línea por espacios, del mismo largo. Así no queda ningún `crate::`
/// de la visibilidad, y los bytes de las demás partes no se mueven.
fn sin_visibilidad(limpia: &str) -> String {
    let sangria = limpia.len() - limpia.trim_start().len();
    let Some(tras_pub) = limpia[sangria..].strip_prefix("pub") else {
        return limpia.to_string();
    };
    let espacios = tras_pub.len() - tras_pub.trim_start().len();
    let Some(dentro) = tras_pub[espacios..].strip_prefix('(') else {
        return limpia.to_string();
    };
    let Some(fin) = dentro.find(')') else {
        return limpia.to_string();
    };
    let largo = "pub".len() + espacios + 1 + fin + 1;
    let mut salida = limpia.to_string();
    salida.replace_range(sangria..sangria + largo, &" ".repeat(largo));
    salida
}

/// Nombres X de cada `crate::X` del texto, con el byte donde empieza X. Con `crate::{...}` da el primer
/// segmento de cada elemento de nivel superior del grupo, sin `self`.
fn nombres_en(texto: &str) -> Vec<(String, usize)> {
    let cs: Vec<(usize, char)> = texto.char_indices().collect();
    let mut salida = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        if !empieza_crate(&cs, i) {
            i += 1;
            continue;
        }
        let j = i + "crate::".len();
        if cs.get(j).is_some_and(|&(_, c)| c == '{') {
            let (grupo, fin) = elementos_de_grupo(&cs, j + 1);
            salida.extend(grupo);
            i = fin;
        } else {
            let nombre = ident_desde(&cs, j);
            if !nombre.is_empty() {
                salida.push((nombre, cs[j].0));
            }
            i = j;
        }
    }
    salida
}

/// Identificador que empieza en `cs[desde]`. Vacío si no hay.
fn ident_desde(cs: &[(usize, char)], desde: usize) -> String {
    cs.get(desde..)
        .unwrap_or_default()
        .iter()
        .take_while(|(_, c)| es_ident(*c))
        .map(|(_, c)| *c)
        .collect()
}

/// Verdadero si `cs[i..]` empieza con `crate::` como palabra: no precedido por letra, cifra o `_`.
fn empieza_crate(cs: &[(usize, char)], i: usize) -> bool {
    let previo_ok = i == 0 || !es_ident(cs[i - 1].1);
    previo_ok
        && "crate::"
            .chars()
            .enumerate()
            .all(|(k, c)| cs.get(i + k).is_some_and(|&(_, x)| x == c))
}

fn es_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Elementos de un grupo `crate::{...}` que empieza en `desde`, justo después de `{`. Da cada primer nombre y
/// el índice justo después de la `}` que cierra el grupo.
fn elementos_de_grupo(cs: &[(usize, char)], desde: usize) -> (Vec<(String, usize)>, usize) {
    let mut salida = Vec::new();
    let mut inicio = desde;
    let mut profundidad = 0usize;
    for k in desde..cs.len() {
        match cs[k].1 {
            '{' => profundidad += 1,
            '}' if profundidad > 0 => profundidad -= 1,
            ',' if profundidad == 0 => {
                salida.extend(primer_nombre(cs, inicio, k));
                inicio = k + 1;
            }
            '}' => {
                salida.extend(primer_nombre(cs, inicio, k));
                return (salida, k + 1);
            }
            _ => {}
        }
    }
    salida.extend(primer_nombre(cs, inicio, cs.len()));
    (salida, cs.len())
}

/// Primer identificador de `cs[desde..hasta]`, con su byte. Nada si está vacío o es `self`.
fn primer_nombre(cs: &[(usize, char)], desde: usize, hasta: usize) -> Option<(String, usize)> {
    let p = (desde..hasta).find(|&k| !cs[k].1.is_whitespace())?;
    let nombre = ident_desde(cs, p);
    (!nombre.is_empty() && nombre != "self").then_some((nombre, cs[p].0))
}

/// Camino `[m, ..., m]` de un ciclo que pasa por `modulo`. Solo lo da el módulo menor de su
/// componente fuertemente conexa, para reportar cada componente una vez.
fn ciclo_de(modulo: &str, adj: &BTreeMap<String, Vec<String>>) -> Option<Vec<String>> {
    let mut miembros: BTreeSet<String> = BTreeSet::from([modulo.to_string()]);
    for otro in alcanzables(modulo, adj) {
        if alcanzables(&otro, adj).contains(modulo) {
            miembros.insert(otro);
        }
    }
    if miembros.len() < 2 || miembros.first().map(String::as_str) != Some(modulo) {
        return None;
    }
    camino_de_vuelta(modulo, adj, &miembros)
}

/// Grafo de módulos: `origen -> destinos`, solo con destinos que son módulos. Las aristas hacia la raíz no
/// entran: usar un ítem de la raíz que declara el módulo es Rust normal.
pub(crate) fn adyacencias(
    aristas: &BTreeMap<(String, String), Uso>,
    modulos: &BTreeSet<String>,
) -> BTreeMap<String, Vec<String>> {
    let mut adj: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (origen, destino) in aristas.keys().filter(|(_, d)| modulos.contains(d)) {
        adj.entry(origen.clone()).or_default().push(destino.clone());
    }
    adj
}

/// Módulos que se alcanzan desde `desde` siguiendo al menos una arista.
pub(crate) fn alcanzables(desde: &str, adj: &BTreeMap<String, Vec<String>>) -> BTreeSet<String> {
    let mut visto = BTreeSet::new();
    let mut cola = VecDeque::from([desde.to_string()]);
    while let Some(x) = cola.pop_front() {
        for y in adj.get(&x).map(Vec::as_slice).unwrap_or_default() {
            if visto.insert(y.clone()) {
                cola.push_back(y.clone());
            }
        }
    }
    visto
}

/// Camino `[inicio, ..., inicio]` dentro de `miembros`.
fn camino_de_vuelta(
    inicio: &str,
    adj: &BTreeMap<String, Vec<String>>,
    miembros: &BTreeSet<String>,
) -> Option<Vec<String>> {
    let mut previo: BTreeMap<String, String> = BTreeMap::new();
    let mut cola = VecDeque::from([inicio.to_string()]);
    while let Some(x) = cola.pop_front() {
        for y in adj.get(&x).map(Vec::as_slice).unwrap_or_default() {
            if !miembros.contains(y) {
                continue;
            }
            if y == inicio {
                let mut camino = vec![x.clone()];
                let mut cur = x;
                while let Some(p) = previo.get(&cur) {
                    camino.push(p.clone());
                    cur = p.clone();
                }
                camino.reverse();
                camino.push(inicio.to_string());
                return Some(camino);
            }
            if !previo.contains_key(y) {
                previo.insert(y.clone(), x.clone());
                cola.push_back(y.clone());
            }
        }
    }
    None
}

fn capa_de(capas: &[Vec<String>], modulo: &str) -> Option<usize> {
    capas.iter().position(|c| c.iter().any(|m| m == modulo))
}

/// Un hallazgo por capa de `capas` que no tiene ningún módulo en el repo. `presentes` son los módulos más la
/// raíz (`main`, `lib`) si existe su archivo. Una capa con al menos uno presente no da hallazgo. La cita es el
/// mapa de módulos del repo (sin la raíz) y la capa entera, así cada capa ausente tiene su propio id. Una capa
/// vacía no da hallazgo, como antes.
fn capas_sin_modulo(
    capas: &[Vec<String>],
    modulos: &BTreeSet<String>,
    presentes: &BTreeSet<String>,
    requisitos: &[Requisito],
) -> Vec<Hallazgo> {
    let mapa = if modulos.is_empty() {
        "ninguno".to_string()
    } else {
        modulos
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut hallazgos = Vec::new();
    for capa in capas {
        if capa.is_empty() || capa.iter().any(|m| presentes.contains(m)) {
            continue;
        }
        let nombres = capa.join(", ");
        let cita = format!("módulos: {mapa}; {FALTA_CAPA} [{nombres}]");
        let detalle = format!("{FALTA_CAPA} [{nombres}]: ninguno de sus módulos está en el repo");
        hallazgos.push(crear_hallazgo(
            Dimension::Arquitectura,
            3,
            "src",
            0,
            &cita,
            &detalle,
            SONDA_ARQUITECTURA,
            requisitos,
        ));
    }
    hallazgos
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oracle::perfiles::requisito_de;
    use crate::tipos::Fuente;
    use std::fs;
    use std::path::Path;

    fn escribir(repo: &Path, ruta: &str, texto: &str) {
        let p = repo.join(ruta);
        fs::create_dir_all(p.parent().expect("con padre")).expect("crea carpeta");
        fs::write(p, texto).expect("escribe el fuente");
    }

    #[test]
    fn modulos_presentes_junta_los_modulos_y_la_raiz_si_existe_su_archivo() {
        let archivos = [
            "src/main.rs",
            "src/comandos/mod.rs",
            "src/cmd.rs",
            "src/lib.rs",
        ]
        .map(|ruta| Archivo {
            ruta: ruta.into(),
            lineas: Vec::new(),
        });
        let presentes = modulos_presentes(&archivos);
        let esperados: BTreeSet<String> = ["cmd", "comandos", "lib", "main"]
            .map(String::from)
            .into_iter()
            .collect();
        assert_eq!(presentes, esperados);
    }

    #[test]
    fn sonda_200_avisos_de_estilo_y_cero_defectos_da_puntaje_maximo() {
        let dir = tempfile::tempdir().expect("temp dir");
        let texto: String = (0..200)
            .map(|i| format!("let v{i} = Some({i}).unwrap();\n"))
            .collect();
        escribir(dir.path(), "src/util.rs", &texto);
        let r = seguridad_nativa(dir.path(), &[]);
        assert_eq!(r.valor, Some(1.0));
        assert_eq!(r.hallazgos.len(), 200, "los avisos se listan");
        assert!(r.hallazgos.iter().all(|h| h.impacto.is_empty()));
    }

    #[test]
    fn sonda_unsafe_en_modulo_con_peso_3_baja_a_7_9() {
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(dir.path(), "src/protocolo/mod.rs", "pub unsafe fn f() {}\n");
        let reqs = [requisito_de("R1", 3, &["protocolo"])];
        let r = seguridad_nativa(dir.path(), &reqs);
        assert_eq!(r.hallazgos.len(), 1);
        assert_eq!(r.hallazgos[0].impacto, ["R1"]);
        assert_eq!(r.hallazgos[0].severidad, 2);
        // penal = (2/3) * (3/3) = 2/3; valor = 1 - (2/3)/3 = 7/9
        let valor = r.valor.expect("hay veredicto");
        assert!((valor - 7.0 / 9.0).abs() < 1e-6, "valor {valor}");
    }

    #[test]
    fn sonda_unsafe_sin_requisito_baja_menos_y_no_tiene_impacto() {
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(dir.path(), "src/otro.rs", "pub unsafe fn f() {}\n");
        let reqs = [requisito_de("R1", 3, &["protocolo"])];
        let r = seguridad_nativa(dir.path(), &reqs);
        assert!(r.hallazgos[0].impacto.is_empty());
        // penal = (2/3) * (1/3) = 2/9; valor = 1 - (2/9)/3 = 25/27
        let valor = r.valor.expect("hay veredicto");
        assert!((valor - 25.0 / 27.0).abs() < 1e-6, "valor {valor}");
        assert!(valor > 7.0 / 9.0);
    }

    #[test]
    fn sonda_unwrap_en_cfg_test_y_en_tests_no_cuenta() {
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(
            dir.path(),
            "src/protocolo/mod.rs",
            "pub fn f() -> u8 { 1 }\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() { Some(1).unwrap(); }\n}\n",
        );
        escribir(dir.path(), "tests/x.rs", "fn t() { Some(1).unwrap(); }\n");
        escribir(
            dir.path(),
            "src/x_test.rs",
            "fn t() { Some(1).unwrap(); }\n",
        );
        let r = seguridad_nativa(dir.path(), &[]);
        assert!(r.hallazgos.is_empty());
        assert_eq!(r.valor, Some(1.0));
    }

    #[test]
    fn sonda_unwrap_en_comentario_o_cadena_no_cuenta() {
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(
            dir.path(),
            "src/protocolo/mod.rs",
            "// Some(1).unwrap()\nlet s = \"x.unwrap()\";\n",
        );
        let r = seguridad_nativa(dir.path(), &[]);
        assert!(r.hallazgos.is_empty());
    }

    #[test]
    fn limpia_bloque_de_comentario_de_tres_lineas_queda_en_blanco() {
        let texto = ["    /*", "    unsafe a > b", "    */"].map(String::from);
        let lineas = analizar(&texto);
        for (i, linea) in lineas.iter().enumerate() {
            assert!(
                linea.limpia.trim().is_empty(),
                "línea {i}: {:?}",
                linea.limpia
            );
            assert_eq!(linea.limpia.len(), texto[i].len(), "línea {i}");
        }
    }

    #[test]
    fn limpia_cadena_cruda_de_varias_lineas_deja_el_medio_en_blanco() {
        let texto = ["let s = r#\"", "a > b", "\"#;"].map(String::from);
        let lineas = analizar(&texto);
        assert!(lineas[1].limpia.trim().is_empty(), "{:?}", lineas[1].limpia);
        assert_eq!(lineas[1].limpia.len(), texto[1].len());
    }

    #[test]
    fn limpia_bloque_anidado_se_cierra_en_el_segundo_cierre() {
        let una = analizar(&["/* /* */ */ x".to_string()]);
        assert_eq!(una[0].limpia.trim(), "x");
        let varias = ["/* a /*", "b */", "c */ x"].map(String::from);
        let lineas = analizar(&varias);
        assert!(lineas[1].limpia.trim().is_empty(), "{:?}", lineas[1].limpia);
        assert_eq!(lineas[2].limpia.trim(), "x");
    }

    #[test]
    fn sonda_unsafe_dentro_de_comentario_de_bloque_no_cuenta() {
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(
            dir.path(),
            "src/protocolo/mod.rs",
            "/*\nunsafe fn f() {}\n*/\npub fn g() -> u8 { 1 }\n",
        );
        let r = seguridad_nativa(dir.path(), &[]);
        assert!(r.hallazgos.is_empty(), "{:?}", r.hallazgos);
    }

    #[test]
    fn aristas_use_dentro_de_comentario_de_bloque_no_cuenta() {
        let texto = "/*\nuse crate::app::run;\n*/\npub fn f() {}\n";
        let archivos = [archivo_de("src/x.rs", texto)];
        let mods = modulos_de(&["x", "app"]);
        assert!(aristas_entre_modulos(&archivos, &mods).is_empty());
    }

    #[test]
    fn limpia_cadena_comun_multilinea_deja_el_medio_en_blanco() {
        let texto = ["let s = \"a", "unsafe", "b\";"].map(String::from);
        let lineas = analizar(&texto);
        assert!(lineas[1].limpia.trim().is_empty(), "{:?}", lineas[1].limpia);
        assert_eq!(lineas[1].limpia.len(), texto[1].len());
    }

    #[test]
    fn limpia_conserva_el_largo_en_bytes_con_caracteres_multibyte() {
        let texto = ["let s = \"ñu\"; // año"].map(String::from);
        let lineas = analizar(&texto);
        assert_eq!(lineas[0].limpia.len(), texto[0].len());
    }

    #[test]
    fn limpia_comilla_de_caracter_no_abre_cadena() {
        // Guarda: la comilla de un literal de carácter no abre cadena. Los lifetimes tampoco.
        let texto = [
            "let c = '\"'; unsafe { f() }",
            "let d = '\\''; unsafe { g() }",
            "fn h<'a>(x: &'a u8) -> &'a u8 { unsafe { x } }",
        ]
        .map(String::from);
        let lineas = analizar(&texto);
        for linea in &lineas {
            assert!(tiene_token(&linea.limpia, "unsafe"), "{:?}", linea.limpia);
        }
    }

    #[test]
    fn sonda_estructura_archivo_grande_da_hallazgo() {
        let dir = tempfile::tempdir().expect("temp dir");
        let texto: String = (0..600).map(|i| format!("let v{i} = {i};\n")).collect();
        escribir(dir.path(), "src/grande.rs", &texto);
        let r = estructura_nativa(dir.path(), &[], &LimitesEstructura::default());
        let grandes: Vec<&Hallazgo> = r
            .hallazgos
            .iter()
            .filter(|h| h.detalle.contains("archivo grande"))
            .collect();
        assert_eq!(grandes.len(), 1);
        assert_eq!(grandes[0].archivo, "src/grande.rs");
        assert_eq!(grandes[0].linea, 1);
        assert_eq!(grandes[0].severidad, 2);
    }

    #[test]
    fn sonda_estructura_funcion_grande_da_hallazgo_en_la_linea_del_fn() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut texto = String::from("pub fn grande() {\n");
        for i in 0..98 {
            texto.push_str(&format!("    let v{i} = {i};\n"));
        }
        texto.push_str("}\n");
        escribir(dir.path(), "src/f.rs", &texto);
        let r = estructura_nativa(dir.path(), &[], &LimitesEstructura::default());
        let funciones: Vec<&Hallazgo> = r
            .hallazgos
            .iter()
            .filter(|h| h.detalle.contains("función"))
            .collect();
        assert_eq!(funciones.len(), 1);
        assert_eq!(funciones[0].linea, 1);
        assert_eq!(funciones[0].severidad, 2);
    }

    #[test]
    fn sonda_estructura_limites_son_configurables() {
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(dir.path(), "src/f.rs", "fn a() {\n    let x = 1;\n}\n");
        let chicos = LimitesEstructura {
            archivo_lineas: 2,
            funcion_lineas: 2,
            bloque_lineas: 10,
        };
        let r = estructura_nativa(dir.path(), &[], &chicos);
        assert_eq!(r.hallazgos.len(), 2, "archivo y función pasan el límite");
    }

    #[test]
    fn sonda_estructura_bloque_copiado_en_dos_archivos_da_un_hallazgo() {
        let dir = tempfile::tempdir().expect("temp dir");
        let bloque: String = (0..10).map(|i| format!("let b{i} = {i} + 1;\n")).collect();
        escribir(
            dir.path(),
            "src/a.rs",
            &format!("let x1 = 1;\n{bloque}let x2 = 2;\n"),
        );
        escribir(
            dir.path(),
            "src/b.rs",
            &format!("let y1 = 1;\n{bloque}let y2 = 2;\n"),
        );
        let r = estructura_nativa(dir.path(), &[], &LimitesEstructura::default());
        let dups: Vec<&Hallazgo> = r
            .hallazgos
            .iter()
            .filter(|h| h.detalle.contains("repetido"))
            .collect();
        assert_eq!(dups.len(), 1);
        assert_eq!(dups[0].archivo, "src/a.rs");
        assert_eq!(dups[0].linea, 2);
        assert_eq!(dups[0].severidad, 1);
    }

    #[test]
    fn sonda_arquitectura_ciclo_da_un_hallazgo_con_archivo_y_linea() {
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(
            dir.path(),
            "src/a.rs",
            "use crate::b::algo;\npub fn f() {}\n",
        );
        escribir(
            dir.path(),
            "src/b.rs",
            "pub fn algo() {}\nuse crate::a::f;\n",
        );
        let r = arquitectura_nativa(dir.path(), &[], &[]);
        assert_eq!(r.hallazgos.len(), 1);
        let h = &r.hallazgos[0];
        assert_eq!(h.severidad, 3);
        assert_eq!((h.archivo.as_str(), h.linea), ("src/a.rs", 1));
        assert!(h.detalle.contains("a -> b -> a"), "{}", h.detalle);
        assert!(h.detalle.starts_with(REGLA_CICLO), "{}", h.detalle);
        assert_eq!(h.dimension, Dimension::Arquitectura);
    }

    #[test]
    fn sonda_arquitectura_sin_ciclo_da_cero() {
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(dir.path(), "src/a.rs", "use crate::b::algo;\n");
        escribir(dir.path(), "src/b.rs", "pub fn algo() {}\n");
        let r = arquitectura_nativa(dir.path(), &[], &[]);
        assert!(r.hallazgos.is_empty());
        assert_eq!(r.valor, Some(1.0));
    }

    #[test]
    fn sonda_arquitectura_workspace_sin_modulos_bajo_src_da_sin_veredicto() {
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(
            dir.path(),
            "Cargo.toml",
            "[workspace]\nmembers = [\"crates/a\"]\n",
        );
        escribir(
            dir.path(),
            "crates/a/src/lib.rs",
            "pub fn f() -> u8 { 1 }\n",
        );
        let r = arquitectura_nativa(dir.path(), &[], &[]);
        assert_eq!(r.valor, None, "{:?}", r.hallazgos);
        assert!(r.hallazgos.is_empty(), "{:?}", r.hallazgos);
        assert_eq!(r.motivo.as_deref(), Some("sin módulos"));
    }

    #[test]
    fn sonda_arquitectura_capas_declaradas_en_workspace_dan_hallazgo_por_capa() {
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(
            dir.path(),
            "Cargo.toml",
            "[workspace]\nmembers = [\"crates/a\"]\n",
        );
        escribir(
            dir.path(),
            "crates/a/src/lib.rs",
            "pub fn f() -> u8 { 1 }\n",
        );
        let r = arquitectura_nativa(dir.path(), &capas(), &[]);
        assert!(r.valor.is_some(), "{:?}", r.motivo);
        assert_eq!(r.hallazgos.len(), 3, "{:?}", r.hallazgos);
        assert!(
            r.hallazgos
                .iter()
                .all(|h| h.cita.starts_with("módulos: ninguno; falta la capa ["))
        );
    }

    #[test]
    fn sonda_arquitectura_capa_sin_modulo_en_el_repo_da_hallazgo_con_la_cita_del_mapa() {
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(dir.path(), "src/main.rs", "fn main() {}\n");
        let capas = [vec!["protocolo".to_string()]];
        let r = arquitectura_nativa(dir.path(), &capas, &[]);
        assert!(r.valor.is_some_and(|v| v < 1.0), "{:?}", r.motivo);
        assert_eq!(r.hallazgos.len(), 1, "{:?}", r.hallazgos);
        let h = &r.hallazgos[0];
        assert_eq!(h.dimension, Dimension::Arquitectura);
        assert_eq!(h.severidad, 3);
        assert_eq!(h.archivo, "src");
        assert_eq!(h.linea, 0);
        assert_eq!(h.cita, "módulos: ninguno; falta la capa [protocolo]");
        assert!(h.detalle.contains("capa") && h.detalle.contains("protocolo"));
        assert_eq!(h.fuente, Fuente::Sonda(SONDA_ARQUITECTURA.into()));
    }

    #[test]
    fn sonda_arquitectura_capa_sin_su_modulo_nombra_los_modulos_del_mapa() {
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(dir.path(), "src/comandos/mod.rs", "pub fn c() {}\n");
        let r = arquitectura_nativa(dir.path(), &capas(), &[]);
        assert_eq!(r.hallazgos.len(), 2, "{:?}", r.hallazgos);
        assert!(
            r.hallazgos
                .iter()
                .all(|h| h.cita.starts_with("módulos: comandos"))
        );
        assert!(r.hallazgos.iter().any(|h| h.detalle.contains("protocolo")));
        assert!(r.hallazgos.iter().any(|h| h.detalle.contains("almacen")));
        assert!(
            r.hallazgos
                .iter()
                .any(|h| h.cita == "módulos: comandos; falta la capa [protocolo]")
        );
        assert!(
            r.hallazgos
                .iter()
                .any(|h| h.cita == "módulos: comandos; falta la capa [almacen]")
        );
    }

    #[test]
    fn sonda_arquitectura_capa_con_un_modulo_presente_no_da_hallazgo() {
        // Una capa junta nombres de dos implementaciones: basta con que exista uno de sus módulos.
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(dir.path(), "src/main.rs", "fn main() {}\n");
        escribir(dir.path(), "src/cmd.rs", "pub fn c() {}\n");
        let capas = [vec![
            "cmd".to_string(),
            "cmd_keys".to_string(),
            "cmd_strings".to_string(),
        ]];
        let r = arquitectura_nativa(dir.path(), &capas, &[]);
        assert!(r.hallazgos.is_empty(), "{:?}", r.hallazgos);
        assert_eq!(r.valor, Some(1.0));
    }

    #[test]
    fn sonda_arquitectura_capa_entera_ausente_da_un_hallazgo_que_nombra_sus_modulos() {
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(dir.path(), "src/main.rs", "fn main() {}\n");
        let capas = [vec![
            "cmd".to_string(),
            "cmd_keys".to_string(),
            "cmd_strings".to_string(),
        ]];
        let r = arquitectura_nativa(dir.path(), &capas, &[]);
        assert_eq!(r.hallazgos.len(), 1, "{:?}", r.hallazgos);
        let h = &r.hallazgos[0];
        assert_eq!(h.dimension, Dimension::Arquitectura);
        assert_eq!(h.severidad, 3);
        assert_eq!(h.archivo, "src");
        assert_eq!(h.linea, 0);
        assert_eq!(h.fuente, Fuente::Sonda(SONDA_ARQUITECTURA.into()));
        assert!(
            h.cita
                .contains("falta la capa [cmd, cmd_keys, cmd_strings]"),
            "{}",
            h.cita
        );
        assert!(
            h.detalle.contains("cmd")
                && h.detalle.contains("cmd_keys")
                && h.detalle.contains("cmd_strings"),
            "{}",
            h.detalle
        );
        assert!(!h.detalle.contains("capa 0"), "{}", h.detalle);
    }

    #[test]
    fn sonda_arquitectura_dos_capas_faltantes_sobreviven_a_la_deduplicacion() {
        // Corrección del organizador: cada capa faltante es un hallazgo propio en el veredicto.
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(dir.path(), "src/comandos/mod.rs", "pub fn c() {}\n");
        let r = arquitectura_nativa(dir.path(), &capas(), &[]);
        assert_eq!(r.hallazgos.len(), 2, "{:?}", r.hallazgos);
        assert_ne!(r.hallazgos[0].id, r.hallazgos[1].id, "{:?}", r.hallazgos);
        let quedan = crate::oracle::perfiles::deduplicar_hallazgos(r.hallazgos);
        assert_eq!(quedan.len(), 2, "{quedan:?}");
    }

    #[test]
    fn sonda_arquitectura_sin_capas_ni_modulos_da_sin_modulos_y_sin_hallazgos() {
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(dir.path(), "src/lib.rs", "pub fn f() -> u8 { 1 }\n");
        let r = arquitectura_nativa(dir.path(), &[], &[]);
        assert_eq!(r.valor, None, "{:?}", r.hallazgos);
        assert!(r.hallazgos.is_empty(), "{:?}", r.hallazgos);
        assert_eq!(r.motivo.as_deref(), Some("sin módulos"));
    }

    #[test]
    fn sonda_arquitectura_con_src_a_y_b_sigue_con_veredicto() {
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(dir.path(), "src/a.rs", "pub fn a() -> u8 { 1 }\n");
        escribir(dir.path(), "src/b.rs", "pub fn b() -> u8 { 2 }\n");
        let r = arquitectura_nativa(dir.path(), &[], &[]);
        assert_eq!(r.valor, Some(1.0), "{:?}", r.motivo);
        assert_eq!(r.motivo, None);
    }

    fn capas() -> Vec<Vec<String>> {
        vec![
            vec!["protocolo".into()],
            vec!["comandos".into()],
            vec!["almacen".into()],
        ]
    }

    #[test]
    fn sonda_arquitectura_capa_posterior_que_importa_una_anterior_viola() {
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(dir.path(), "src/protocolo/mod.rs", "pub fn p() {}\n");
        escribir(dir.path(), "src/comandos/mod.rs", "pub fn c() {}\n");
        escribir(
            dir.path(),
            "src/almacen/mod.rs",
            "fn a() {}\nuse crate::protocolo::p;\n",
        );
        let r = arquitectura_nativa(dir.path(), &capas(), &[]);
        assert_eq!(r.hallazgos.len(), 1);
        assert_eq!(r.hallazgos[0].archivo, "src/almacen/mod.rs");
        assert_eq!(r.hallazgos[0].linea, 2);
        assert_eq!(r.hallazgos[0].severidad, 3);
    }

    #[test]
    fn sonda_arquitectura_capa_anterior_que_importa_una_posterior_no_viola() {
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(
            dir.path(),
            "src/protocolo/mod.rs",
            "use crate::almacen::x;\n",
        );
        escribir(dir.path(), "src/almacen/mod.rs", "pub fn x() {}\n");
        escribir(dir.path(), "src/comandos/mod.rs", "pub fn c() {}\n");
        let r = arquitectura_nativa(dir.path(), &capas(), &[]);
        assert!(r.hallazgos.is_empty());
    }

    #[test]
    fn rust_sonda_recorre_crates_fuera_de_src() {
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(dir.path(), "crates/a/src/lib.rs", "pub unsafe fn f() {}\n");
        let r = seguridad_nativa(dir.path(), &[]);
        assert_eq!(r.hallazgos.len(), 1, "{:?}", r.hallazgos);
        assert_eq!(r.hallazgos[0].archivo, "crates/a/src/lib.rs");
        assert!(r.valor.is_some_and(|v| v < 1.0), "{:?}", r.valor);
    }

    #[test]
    fn rust_sonda_no_recorre_target_ni_carpetas_ocultas() {
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(dir.path(), "src/lib.rs", "pub fn f() {}\n");
        escribir(
            dir.path(),
            "target/debug/build/x.rs",
            "pub unsafe fn g() {}\n",
        );
        escribir(dir.path(), ".git/hooks/x.rs", "pub unsafe fn h() {}\n");
        let r = seguridad_nativa(dir.path(), &[]);
        assert!(r.hallazgos.is_empty(), "{:?}", r.hallazgos);
        assert_eq!(r.valor, Some(1.0));
    }

    #[test]
    fn rust_repo_sin_archivos_rs_da_sin_veredicto_en_tres_dimensiones() {
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(dir.path(), "README.md", "sin fuentes\n");
        let capas = capas();
        let sondas = [
            seguridad_nativa(dir.path(), &[]),
            estructura_nativa(dir.path(), &[], &LimitesEstructura::default()),
            arquitectura_nativa(dir.path(), &capas, &[]),
        ];
        for r in &sondas {
            assert_eq!(r.valor, None, "{:?}", r.dimension);
            assert!(
                r.motivo
                    .as_deref()
                    .is_some_and(|m| m.contains("no hay archivos .rs")),
                "{:?}",
                r.motivo
            );
        }
    }

    #[test]
    fn rust_repo_inexistente_da_sin_veredicto() {
        let dir = tempfile::tempdir().expect("temp dir");
        let ausente = dir.path().join("no-existe");
        let r = seguridad_nativa(&ausente, &[]);
        assert_eq!(r.valor, None);
        assert!(r.motivo.is_some());
    }

    #[test]
    fn rust_archivo_ilegible_da_sin_veredicto_y_no_puntaje() {
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(dir.path(), "src/lib.rs", "pub fn f() {}\n");
        // Bytes que no son UTF-8: `read_to_string` falla y la sonda no puede leer el fuente.
        fs::write(dir.path().join("src/roto.rs"), [0xff, 0xfe, 0x00, 0x9f]).expect("escribe");
        let r = seguridad_nativa(dir.path(), &[]);
        assert_eq!(r.valor, None, "{:?}", r.motivo);
        assert!(r.motivo.is_some());
        let e = estructura_nativa(dir.path(), &[], &LimitesEstructura::default());
        assert_eq!(e.valor, None);
        let a = arquitectura_nativa(dir.path(), &capas(), &[]);
        assert_eq!(a.valor, None);
    }

    #[test]
    fn sonda_arquitectura_capa_main_con_src_main_rs_no_da_capa_faltante() {
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(dir.path(), "src/main.rs", "mod server;\nfn main() {}\n");
        escribir(dir.path(), "src/server.rs", "pub fn run() {}\n");
        let capas = [vec!["main".to_string()], vec!["server".to_string()]];
        let r = arquitectura_nativa(dir.path(), &capas, &[]);
        assert!(
            r.hallazgos
                .iter()
                .all(|h| !h.detalle.contains("falta la capa")),
            "{:?}",
            r.hallazgos
        );
    }

    #[test]
    fn sonda_arquitectura_capa_main_lib_con_src_lib_rs_no_da_capa_faltante() {
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(dir.path(), "src/lib.rs", "pub mod app;\n");
        escribir(dir.path(), "src/app.rs", "pub fn a() {}\n");
        let capas = [
            vec!["main".to_string(), "lib".to_string()],
            vec!["app".to_string()],
        ];
        let r = arquitectura_nativa(dir.path(), &capas, &[]);
        assert!(
            r.hallazgos
                .iter()
                .all(|h| !h.detalle.contains("falta la capa")),
            "{:?}",
            r.hallazgos
        );
    }

    #[test]
    fn sonda_arquitectura_modulo_que_importa_de_main_da_hallazgo_de_orden() {
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(
            dir.path(),
            "src/main.rs",
            "mod server;\npub struct Config;\nfn main() {}\n",
        );
        escribir(
            dir.path(),
            "src/server.rs",
            "use crate::Config;\npub fn run() {}\n",
        );
        let capas = [vec!["main".to_string()], vec!["server".to_string()]];
        let r = arquitectura_nativa(dir.path(), &capas, &[]);
        assert_eq!(r.hallazgos.len(), 1, "{:?}", r.hallazgos);
        let h = &r.hallazgos[0];
        assert_eq!(h.detalle, "server (capa 1) importa main (capa 0)");
        assert_eq!((h.archivo.as_str(), h.linea), ("src/server.rs", 1));
        assert!(!h.detalle.starts_with(REGLA_CICLO), "{}", h.detalle);
    }

    #[test]
    fn sonda_arquitectura_main_que_importa_un_modulo_no_da_hallazgo() {
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(
            dir.path(),
            "src/main.rs",
            "mod server;\nuse crate::server::run;\nfn main() { run(); }\n",
        );
        escribir(dir.path(), "src/server.rs", "pub fn run() {}\n");
        let capas = [vec!["main".to_string()], vec!["server".to_string()]];
        let r = arquitectura_nativa(dir.path(), &capas, &[]);
        assert!(r.hallazgos.is_empty(), "{:?}", r.hallazgos);
        assert_eq!(r.valor, Some(1.0));
    }

    #[test]
    fn sonda_arquitectura_modulo_de_lib_que_importa_un_item_de_lib_da_hallazgo_de_orden() {
        // `app` la declara `lib.rs`: la raíz es `lib`, aunque también exista `main.rs`.
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(
            dir.path(),
            "src/lib.rs",
            "pub mod app;\npub struct Config;\n",
        );
        escribir(dir.path(), "src/main.rs", "fn main() {}\n");
        escribir(
            dir.path(),
            "src/app.rs",
            "use crate::Config;\npub fn a() {}\n",
        );
        let capas = [vec!["lib".to_string()], vec!["app".to_string()]];
        let r = arquitectura_nativa(dir.path(), &capas, &[]);
        assert_eq!(r.hallazgos.len(), 1, "{:?}", r.hallazgos);
        assert_eq!(r.hallazgos[0].detalle, "app (capa 1) importa lib (capa 0)");
    }

    #[test]
    fn sonda_arquitectura_uso_de_un_item_de_la_raiz_no_forma_ciclo() {
        // Guarda: `server` usa `Config` de `main.rs` y `main.rs` usa `server`. Eso es Rust normal, no ciclo.
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(
            dir.path(),
            "src/main.rs",
            "mod server;\nuse crate::server::run;\npub struct Config;\nfn main() { run(); }\n",
        );
        escribir(
            dir.path(),
            "src/server.rs",
            "use crate::Config;\npub fn run() {}\n",
        );
        let r = arquitectura_nativa(dir.path(), &[], &[]);
        assert!(
            r.hallazgos.iter().all(|h| !h.detalle.contains(REGLA_CICLO)),
            "{:?}",
            r.hallazgos
        );
        assert_eq!(r.valor, Some(1.0));
    }

    #[test]
    fn sonda_arquitectura_solo_main_rs_sin_capas_da_sin_modulos() {
        // Guarda: `main.rs` no es módulo para el criterio "sin módulos".
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(dir.path(), "src/main.rs", "fn main() {}\n");
        let r = arquitectura_nativa(dir.path(), &[], &[]);
        assert_eq!(r.valor, None, "{:?}", r.hallazgos);
        assert!(r.hallazgos.is_empty(), "{:?}", r.hallazgos);
        assert_eq!(r.motivo.as_deref(), Some(MOTIVO_SIN_MODULOS));
    }

    fn archivo_de(ruta: &str, texto: &str) -> Archivo {
        Archivo {
            ruta: ruta.into(),
            lineas: texto.lines().map(String::from).collect(),
        }
    }

    fn modulos_de(nombres: &[&str]) -> BTreeSet<String> {
        nombres.iter().map(|m| m.to_string()).collect()
    }

    #[test]
    fn aristas_bloque_agrupado_llega_a_app_y_display() {
        let texto = "use {\n    crate::{\n        app::*,\n        display::Screen,\n    },\n    std::io,\n};\n";
        let archivos = [archivo_de("src/x.rs", texto)];
        let mods = modulos_de(&["x", "app", "display"]);
        let aristas = aristas_entre_modulos(&archivos, &mods);
        let claves: Vec<String> = aristas.keys().map(|(o, d)| format!("{o}->{d}")).collect();
        assert_eq!(claves, ["x->app", "x->display"]);
        let uso = &aristas[&("x".to_string(), "display".to_string())];
        assert_eq!((uso.linea, uso.cita.as_str()), (4, "display::Screen,"));
    }

    #[test]
    fn aristas_grupo_en_una_linea_da_el_primer_segmento_de_cada_camino() {
        let archivos = [archivo_de("src/x.rs", "use crate::{a::X, b::{c, d}};\n")];
        let mods = modulos_de(&["x", "a", "b", "c", "d"]);
        let claves: Vec<String> = aristas_entre_modulos(&archivos, &mods)
            .keys()
            .map(|(o, d)| format!("{o}->{d}"))
            .collect();
        assert_eq!(claves, ["x->a", "x->b"]);
    }

    #[test]
    fn aristas_pub_crate_use_agrupado_en_varias_lineas() {
        let archivos = [archivo_de(
            "src/x.rs",
            "pub(crate) use crate::{\n    a,\n};\n",
        )];
        let mods = modulos_de(&["x", "a"]);
        let aristas = aristas_entre_modulos(&archivos, &mods);
        let claves: Vec<String> = aristas.keys().map(|(o, d)| format!("{o}->{d}")).collect();
        assert_eq!(claves, ["x->a"]);
        let uso = &aristas[&("x".to_string(), "a".to_string())];
        assert_eq!((uso.linea, uso.cita.as_str()), (2, "a,"));
    }

    #[test]
    fn aristas_ignoran_self_en_grupos() {
        let texto = "use crate::{self};\nuse crate::a::{self, b};\n";
        let archivos = [archivo_de("src/x.rs", texto)];
        let mods = modulos_de(&["x", "a", "b"]);
        let claves: Vec<String> = aristas_entre_modulos(&archivos, &mods)
            .keys()
            .map(|(o, d)| format!("{o}->{d}"))
            .collect();
        assert_eq!(claves, ["x->a"]);
    }

    #[test]
    fn aristas_agrupado_dentro_de_test_no_cuenta() {
        let texto = "pub fn f() {}\n#[cfg(test)]\nmod tests {\n    use crate::{\n        app::X,\n    };\n}\n";
        let archivos = [archivo_de("src/x.rs", texto)];
        let mods = modulos_de(&["x", "app"]);
        assert!(aristas_entre_modulos(&archivos, &mods).is_empty());
    }

    #[test]
    fn aristas_expresion_con_crate_sigue_contando() {
        let archivos = [archivo_de("src/x.rs", "fn f() { crate::app::run(); }\n")];
        let mods = modulos_de(&["x", "app"]);
        let claves: Vec<String> = aristas_entre_modulos(&archivos, &mods)
            .keys()
            .map(|(o, d)| format!("{o}->{d}"))
            .collect();
        assert_eq!(claves, ["x->app"]);
    }

    #[test]
    fn aristas_pub_in_no_cuenta_el_camino_de_visibilidad() {
        // El `crate::a` de `pub(in ...)` no es destino: solo cuenta `b`.
        let archivos = [archivo_de(
            "src/x.rs",
            "pub(in crate::a) use crate::b::X;\n",
        )];
        let mods = modulos_de(&["x", "a", "b"]);
        let claves: Vec<String> = aristas_entre_modulos(&archivos, &mods)
            .keys()
            .map(|(o, d)| format!("{o}->{d}"))
            .collect();
        assert_eq!(claves, ["x->b"]);
    }

    #[test]
    fn aristas_pub_in_agrupado_no_cuenta_la_visibilidad() {
        let texto = "pub(in crate::a) use crate::{\n    b::X,\n};\n";
        let archivos = [archivo_de("src/x.rs", texto)];
        let mods = modulos_de(&["x", "a", "b"]);
        let claves: Vec<String> = aristas_entre_modulos(&archivos, &mods)
            .keys()
            .map(|(o, d)| format!("{o}->{d}"))
            .collect();
        assert_eq!(claves, ["x->b"]);
    }

    #[test]
    fn aristas_otras_visibilidades_y_use_simple_siguen_igual() {
        // Guarda: sin `in crate::`, la visibilidad no tiene `crate::` y el destino sigue igual.
        let mods = modulos_de(&["x", "b"]);
        for texto in [
            "pub(crate) use crate::b::X;\n",
            "pub(super) use crate::b::X;\n",
            "use crate::b::X;\n",
        ] {
            let archivos = [archivo_de("src/x.rs", texto)];
            let claves: Vec<String> = aristas_entre_modulos(&archivos, &mods)
                .keys()
                .map(|(o, d)| format!("{o}->{d}"))
                .collect();
            assert_eq!(claves, ["x->b"], "texto: {texto}");
        }
    }

    #[test]
    fn sonda_arquitectura_pub_in_no_inventa_ciclo() {
        // `b` usa `pub(in crate::a)`: ese `a` no es una arista b -> a. Sin la arista falsa no hay ciclo.
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(
            dir.path(),
            "src/a.rs",
            "use crate::b::algo;\npub fn f() {}\n",
        );
        escribir(
            dir.path(),
            "src/b.rs",
            "pub(in crate::a) use crate::c::K;\npub fn algo() {}\n",
        );
        escribir(dir.path(), "src/c.rs", "pub struct K;\n");
        let r = arquitectura_nativa(dir.path(), &[], &[]);
        assert!(r.hallazgos.is_empty(), "{:?}", r.hallazgos);
        assert_eq!(r.valor, Some(1.0));
    }

    #[test]
    fn sonda_arquitectura_ciclo_escondido_en_import_agrupado_da_hallazgo() {
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(
            dir.path(),
            "src/a.rs",
            "use crate::{\n    b::algo,\n};\npub fn f() {}\n",
        );
        escribir(
            dir.path(),
            "src/b.rs",
            "pub fn algo() {}\nuse crate::a::f;\n",
        );
        let r = arquitectura_nativa(dir.path(), &[], &[]);
        assert_eq!(r.hallazgos.len(), 1, "{:?}", r.hallazgos);
        let h = &r.hallazgos[0];
        assert_eq!(h.severidad, 3);
        assert_eq!((h.archivo.as_str(), h.linea), ("src/a.rs", 2));
        assert!(h.detalle.contains("a -> b -> a"), "{}", h.detalle);
        assert!(h.detalle.starts_with(REGLA_CICLO), "{}", h.detalle);
    }

    #[test]
    fn sonda_arquitectura_uso_hacia_capa_anterior_en_import_agrupado_da_hallazgo() {
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(dir.path(), "src/protocolo/mod.rs", "pub fn p() {}\n");
        escribir(dir.path(), "src/comandos/mod.rs", "pub fn c() {}\n");
        escribir(
            dir.path(),
            "src/almacen/mod.rs",
            "fn a() {}\nuse crate::{\n    protocolo::p,\n};\n",
        );
        let r = arquitectura_nativa(dir.path(), &capas(), &[]);
        assert_eq!(r.hallazgos.len(), 1, "{:?}", r.hallazgos);
        assert_eq!(r.hallazgos[0].archivo, "src/almacen/mod.rs");
        assert_eq!(r.hallazgos[0].linea, 3);
        assert_eq!(r.hallazgos[0].severidad, 3);
    }
}
