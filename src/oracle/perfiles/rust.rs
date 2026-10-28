//! Sondas nativas del perfil `rust`: leen los fuentes, sin herramientas externas.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::Path;

use super::{ResultadoSonda, crear_hallazgo, modulo_de, puntuar_hallazgos};
use crate::goal::calcular_hash;
use crate::oracle::tarea::Requisito;
use crate::tipos::{Dimension, Hallazgo};

const SONDA_SEGURIDAD: &str = "seguridad-nativa";
const SONDA_ESTRUCTURA: &str = "estructura-nativa";
const SONDA_ARQUITECTURA: &str = "arquitectura-nativa";

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
struct Archivo {
    ruta: String,
    lineas: Vec<String>,
}

/// Una línea con su número, su texto crudo y su texto sin comentarios ni cadenas.
struct Linea {
    numero: u32,
    cruda: String,
    limpia: String,
    /// Verdadero dentro de un bloque `#[cfg(test)]`.
    de_test: bool,
}

/// Una arista entre módulos: el primer uso del módulo origen hacia el destino.
struct Uso {
    archivo: String,
    linea: u32,
    cita: String,
}

/// Seguridad: cada `unsafe` (severidad 2) y cada `unwrap()`/`expect(` (severidad 1) fuera de tests.
pub fn seguridad_nativa(repo: &Path, requisitos: &[Requisito]) -> ResultadoSonda {
    let mut hallazgos = Vec::new();
    for archivo in archivos_rs(repo) {
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
    let archivos = archivos_rs(repo);
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

/// Arquitectura: ciclos entre módulos (severidad 3) y usos de una capa anterior (severidad 3).
pub fn arquitectura_nativa(
    repo: &Path,
    capas: &[Vec<String>],
    requisitos: &[Requisito],
) -> ResultadoSonda {
    let archivos = archivos_rs(repo);
    let modulos: BTreeSet<String> = archivos.iter().filter_map(|a| modulo_de(&a.ruta)).collect();
    let aristas = aristas_entre_modulos(&archivos, &modulos);
    let mut adj: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (origen, destino) in aristas.keys() {
        adj.entry(origen.clone()).or_default().push(destino.clone());
    }
    let mut hallazgos = Vec::new();
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
                &format!("ciclo de módulos: {}", camino.join(" -> ")),
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

fn archivos_rs(repo: &Path) -> Vec<Archivo> {
    let mut salida = Vec::new();
    recorrer(repo, &repo.join("src"), &mut salida);
    salida.sort_by(|a, b| a.ruta.cmp(&b.ruta));
    salida
}

fn recorrer(repo: &Path, dir: &Path, salida: &mut Vec<Archivo>) {
    let Ok(entradas) = fs::read_dir(dir) else {
        return;
    };
    for entrada in entradas.flatten() {
        let ruta = entrada.path();
        if ruta.is_dir() {
            if entrada.file_name().to_str() != Some("target") {
                recorrer(repo, &ruta, salida);
            }
        } else if ruta.extension().is_some_and(|e| e == "rs") {
            let Ok(texto) = fs::read_to_string(&ruta) else {
                continue;
            };
            let rel = ruta.strip_prefix(repo).unwrap_or(&ruta);
            salida.push(Archivo {
                ruta: rel.to_string_lossy().replace('\\', "/"),
                lineas: texto.lines().map(String::from).collect(),
            });
        }
    }
}

/// Test por ruta: carpeta `tests`, o nombre `*_test.rs` / `*_tests.rs`.
fn es_archivo_de_test(ruta: &str) -> bool {
    ruta.split('/').any(|c| c == "tests")
        || ruta.ends_with("_test.rs")
        || ruta.ends_with("_tests.rs")
}

/// Quita comentarios `//` y cambia el contenido de las cadenas por espacios, por línea.
fn limpiar_linea(linea: &str) -> String {
    let mut salida = String::with_capacity(linea.len());
    let mut en_cadena = false;
    let mut chars = linea.chars().peekable();
    while let Some(c) = chars.next() {
        if en_cadena {
            match c {
                '\\' => {
                    chars.next();
                    salida.push(' ');
                }
                '"' => {
                    en_cadena = false;
                    salida.push('"');
                }
                _ => salida.push(' '),
            }
        } else if c == '/' && chars.peek() == Some(&'/') {
            break;
        } else if c == '"' {
            en_cadena = true;
            salida.push('"');
        } else {
            salida.push(c);
        }
    }
    salida
}

/// Marca cada línea como de test o no. Un bloque `#[cfg(test)]` cuenta con sus llaves.
fn analizar(texto: &[String]) -> Vec<Linea> {
    let mut profundidad: i32 = 0;
    let mut bloque_test: Option<i32> = None;
    let mut pendiente = false;
    let mut salida = Vec::with_capacity(texto.len());
    for (indice, cruda) in texto.iter().enumerate() {
        let limpia = limpiar_linea(cruda);
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
fn tiene_token(linea: &str, palabra: &str) -> bool {
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
fn aristas_entre_modulos(
    archivos: &[Archivo],
    modulos: &BTreeSet<String>,
) -> BTreeMap<(String, String), Uso> {
    let mut aristas = BTreeMap::new();
    for archivo in archivos {
        let Some(origen) = modulo_de(&archivo.ruta) else {
            continue;
        };
        for linea in analizar(&archivo.lineas).iter().filter(|l| !l.de_test) {
            for destino in usos_de_crate(&linea.limpia) {
                if destino != origen && modulos.contains(&destino) {
                    aristas
                        .entry((origen.clone(), destino))
                        .or_insert_with(|| Uso {
                            archivo: archivo.ruta.clone(),
                            linea: linea.numero,
                            cita: linea.cruda.clone(),
                        });
                }
            }
        }
    }
    aristas
}

/// Nombres X de cada `crate::X` en la línea.
fn usos_de_crate(limpia: &str) -> Vec<String> {
    limpia
        .match_indices("crate::")
        .filter_map(|(i, _)| {
            let nombre: String = limpia[i + "crate::".len()..]
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            (!nombre.is_empty()).then_some(nombre)
        })
        .collect()
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

/// Módulos que se alcanzan desde `desde` siguiendo al menos una arista.
fn alcanzables(desde: &str, adj: &BTreeMap<String, Vec<String>>) -> BTreeSet<String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oracle::perfiles::requisito_de;
    use std::fs;
    use std::path::Path;

    fn escribir(repo: &Path, ruta: &str, texto: &str) {
        let p = repo.join(ruta);
        fs::create_dir_all(p.parent().expect("con padre")).expect("crea carpeta");
        fs::write(p, texto).expect("escribe el fuente");
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
        let r = arquitectura_nativa(dir.path(), &capas(), &[]);
        assert!(r.hallazgos.is_empty());
    }
}
