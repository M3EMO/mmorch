//! Mapa del repo: módulos, API pública, dependencias y tamaños. Lo lee el juez de diseño (Tarea 10).
//! Reusa los fuentes y el grafo de `perfiles::rust`: no calcula sus propias aristas.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::ErrorOraculo;
use super::perfiles::modulo_de;
use super::perfiles::rust::{Linea, analizar, archivos_rs, aristas_entre_modulos, tiene_token};

/// Nombre del módulo sintético de `src/main.rs` y `src/lib.rs`.
const RAIZ: &str = "(raiz)";
/// Marca que se agrega al final de un texto recortado.
const MARCA_RECORTE: &str = "…[recortado]";

/// Mapa completo del repo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mapa {
    pub modulos: Vec<ModuloMapa>,
    pub aristas: Vec<Arista>,
    pub total_lineas: u32,
}

/// Un módulo: sus archivos, sus líneas y su API pública.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModuloMapa {
    pub nombre: String,
    pub archivos: Vec<ArchivoMapa>,
    pub lineas: u32,
    /// Ítems `pub` de nivel superior o de `impl`, como `fn nombre` o `struct Nombre`.
    pub api_publica: Vec<String>,
}

/// Un fuente `.rs` con sus líneas. `ruta` va relativa al repo, con `/`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchivoMapa {
    pub ruta: String,
    pub lineas: u32,
}

/// Dependencia `desde -> hacia`, con el primer uso de `desde` que importa `hacia`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Arista {
    pub desde: String,
    pub hacia: String,
    pub archivo: String,
    pub linea: u32,
}

/// Construye el mapa de `repo` leyendo `src/`. Sin `src/` da un mapa vacío. Un error de lectura da error.
pub fn construir(repo: &Path) -> Result<Mapa, ErrorOraculo> {
    // El mapa lee solo `src/`: los fuentes de fuera (tests, crates) no entran a sus módulos.
    let archivos: Vec<_> = archivos_rs(repo)
        .map_err(ErrorOraculo::Infra)?
        .into_iter()
        .filter(|a| a.ruta.starts_with("src/"))
        .collect();
    let mut por_nombre: BTreeMap<String, ModuloMapa> = BTreeMap::new();
    for archivo in &archivos {
        let nombre = nombre_de_modulo(&archivo.ruta);
        let modulo = por_nombre
            .entry(nombre.clone())
            .or_insert_with(|| ModuloMapa {
                nombre,
                archivos: Vec::new(),
                lineas: 0,
                api_publica: Vec::new(),
            });
        let lineas = archivo.lineas.len() as u32;
        modulo.lineas += lineas;
        modulo.archivos.push(ArchivoMapa {
            ruta: archivo.ruta.clone(),
            lineas,
        });
        for item in api_publica(&analizar(&archivo.lineas)) {
            if !modulo.api_publica.contains(&item) {
                modulo.api_publica.push(item);
            }
        }
    }
    let modulos: Vec<ModuloMapa> = por_nombre.into_values().collect();
    let nombres: BTreeSet<String> = modulos
        .iter()
        .map(|m| m.nombre.clone())
        .filter(|n| n != RAIZ)
        .collect();
    let aristas = aristas_entre_modulos(&archivos, &nombres)
        .into_iter()
        .map(|((desde, hacia), uso)| Arista {
            desde,
            hacia,
            archivo: uso.archivo,
            linea: uso.linea,
        })
        .collect();
    let total_lineas = modulos.iter().map(|m| m.lineas).sum();
    Ok(Mapa {
        modulos,
        aristas,
        total_lineas,
    })
}

/// Módulo de un fuente: su primer componente bajo `src/`, o `(raiz)` para `main.rs` y `lib.rs`.
fn nombre_de_modulo(ruta: &str) -> String {
    modulo_de(ruta).unwrap_or_else(|| RAIZ.to_string())
}

/// Ítems `pub` de nivel superior o de `impl` (y de `mod`), sin los de test ni los de dentro de `fn`.
/// Un ítem se nombra `clase nombre`. Sin duplicados, en orden de aparición.
fn api_publica(lineas: &[Linea]) -> Vec<String> {
    let mut salida: Vec<String> = Vec::new();
    // Por cada bloque abierto: verdadero si es `impl` o `mod`, falso si es otro (fn, struct, etc.).
    let mut pila: Vec<bool> = Vec::new();
    for linea in lineas {
        let visible = pila.iter().all(|transparente| *transparente);
        let publica = if visible && !linea.de_test {
            item_publico(&linea.limpia)
        } else {
            None
        };
        if let Some(item) = publica.filter(|item| !salida.contains(item)) {
            salida.push(item);
        }
        let transparente = tiene_token(&linea.limpia, "impl") || tiene_token(&linea.limpia, "mod");
        for c in linea.limpia.chars() {
            match c {
                '{' => pila.push(transparente),
                '}' => {
                    pila.pop();
                }
                _ => {}
            }
        }
    }
    salida
}

/// `clase nombre` si la línea declara un ítem `pub` con clase de API. `pub(crate)` no cuenta.
fn item_publico(limpia: &str) -> Option<String> {
    let resto = limpia.trim_start().strip_prefix("pub ")?;
    let mut palabras = resto.split_whitespace().peekable();
    while matches!(palabras.peek(), Some(&"async" | &"unsafe")) {
        palabras.next();
    }
    let mut clase = palabras.next()?;
    if clase == "const" && palabras.peek() == Some(&"fn") {
        palabras.next();
        clase = "fn";
    }
    if !matches!(
        clase,
        "fn" | "struct" | "enum" | "trait" | "const" | "type" | "mod"
    ) {
        return None;
    }
    let nombre: String = palabras
        .next()?
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    (!nombre.is_empty()).then(|| format!("{clase} {nombre}"))
}

impl Mapa {
    /// Nombres de los módulos del repo, sin el módulo `(raiz)`.
    pub fn nombres_de_modulos(&self) -> BTreeSet<String> {
        self.modulos
            .iter()
            .filter(|m| m.nombre != RAIZ)
            .map(|m| m.nombre.clone())
            .collect()
    }

    /// Resumen en español para un prompt. Si supera `max_chars`, recorta primero la API de los
    /// módulos más grandes y al final corta el texto con `…[recortado]`. Es determinista.
    pub fn a_texto(&self, max_chars: usize) -> String {
        let mut orden: Vec<&ModuloMapa> = self.modulos.iter().collect();
        orden.sort_by(|a, b| {
            b.lineas
                .cmp(&a.lineas)
                .then_with(|| a.nombre.cmp(&b.nombre))
        });
        let mut recortar: BTreeSet<&str> = BTreeSet::new();
        let mut ultimo = self.render(&recortar);
        if ultimo.chars().count() <= max_chars {
            return ultimo;
        }
        for modulo in &orden {
            recortar.insert(modulo.nombre.as_str());
            ultimo = self.render(&recortar);
            if ultimo.chars().count() <= max_chars {
                return ultimo;
            }
        }
        let mut corto: String = ultimo.chars().take(max_chars).collect();
        corto.push_str(MARCA_RECORTE);
        corto
    }

    /// Rutas de los `n` archivos más grandes. Desempate alfabético.
    pub fn archivos_destacados(&self, n: usize) -> Vec<String> {
        let mut todos: Vec<&ArchivoMapa> = self
            .modulos
            .iter()
            .flat_map(|m| m.archivos.iter())
            .collect();
        todos.sort_by(|a, b| b.lineas.cmp(&a.lineas).then_with(|| a.ruta.cmp(&b.ruta)));
        todos.into_iter().take(n).map(|a| a.ruta.clone()).collect()
    }

    /// Texto del mapa. Los módulos en `recortar` muestran `api: recortada` en vez de su API.
    fn render(&self, recortar: &BTreeSet<&str>) -> String {
        let mut texto = String::from("Módulos:\n");
        for modulo in &self.modulos {
            let api = if recortar.contains(modulo.nombre.as_str()) {
                "recortada".to_string()
            } else if modulo.api_publica.is_empty() {
                "ninguna".to_string()
            } else {
                modulo.api_publica.join(", ")
            };
            texto.push_str(&format!(
                "{}: {} líneas, {} archivos, api: {api}\n",
                modulo.nombre,
                modulo.lineas,
                modulo.archivos.len()
            ));
        }
        texto.push_str("Dependencias:\n");
        if self.aristas.is_empty() {
            texto.push_str("ninguna\n");
        }
        for arista in &self.aristas {
            texto.push_str(&format!(
                "{} -> {} ({}:{})\n",
                arista.desde, arista.hacia, arista.archivo, arista.linea
            ));
        }
        texto
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn escribir(repo: &Path, ruta: &str, texto: &str) {
        let p = repo.join(ruta);
        fs::create_dir_all(p.parent().expect("con padre")).expect("crea carpeta");
        fs::write(p, texto).expect("escribe el fuente");
    }

    fn repo_de(archivos: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temp dir");
        for (ruta, texto) in archivos {
            escribir(dir.path(), ruta, texto);
        }
        dir
    }

    fn repo_base() -> tempfile::TempDir {
        repo_de(&[
            ("src/main.rs", "fn main() {}\n"),
            (
                "src/protocolo/mod.rs",
                "pub fn parsear() {}\npub struct Trama;\nfn privada() {}\npub(crate) fn interna() {}\n",
            ),
            (
                "src/almacen.rs",
                "use crate::protocolo::Trama;\npub trait Almacen {}\n",
            ),
        ])
    }

    fn modulo<'a>(mapa: &'a Mapa, nombre: &str) -> &'a ModuloMapa {
        mapa.modulos
            .iter()
            .find(|m| m.nombre == nombre)
            .unwrap_or_else(|| panic!("falta el módulo {nombre}"))
    }

    #[test]
    fn mapa_modulos_api_y_aristas_del_repo_de_prueba() {
        let dir = repo_base();
        let mapa = construir(dir.path()).expect("construye");
        let nombres: Vec<&str> = mapa.modulos.iter().map(|m| m.nombre.as_str()).collect();
        assert_eq!(nombres, ["(raiz)", "almacen", "protocolo"]);
        assert_eq!(
            modulo(&mapa, "protocolo").api_publica,
            ["fn parsear", "struct Trama"]
        );
        assert_eq!(modulo(&mapa, "almacen").api_publica, ["trait Almacen"]);
        assert_eq!(
            mapa.aristas,
            [Arista {
                desde: "almacen".into(),
                hacia: "protocolo".into(),
                archivo: "src/almacen.rs".into(),
                linea: 1,
            }]
        );
        assert_eq!(modulo(&mapa, "protocolo").lineas, 4);
        assert_eq!(
            modulo(&mapa, "protocolo").archivos,
            [ArchivoMapa {
                ruta: "src/protocolo/mod.rs".into(),
                lineas: 4,
            }]
        );
        assert_eq!(mapa.total_lineas, 1 + 4 + 2);
    }

    #[test]
    fn mapa_pub_en_comentario_o_cadena_no_cuenta() {
        let dir = repo_de(&[(
            "src/a.rs",
            "// pub fn oculta() {}\nlet s = \"pub struct Falsa\";\npub fn real() {}\n",
        )]);
        let mapa = construir(dir.path()).expect("construye");
        assert_eq!(modulo(&mapa, "a").api_publica, ["fn real"]);
    }

    #[test]
    fn mapa_pub_repetido_en_impls_no_duplica() {
        let dir = repo_de(&[(
            "src/a.rs",
            "pub struct A;\npub struct B;\nimpl A {\n    pub fn nuevo() {}\n}\nimpl B {\n    pub fn nuevo() {}\n}\n",
        )]);
        let mapa = construir(dir.path()).expect("construye");
        assert_eq!(
            modulo(&mapa, "a").api_publica,
            ["struct A", "struct B", "fn nuevo"]
        );
    }

    #[test]
    fn mapa_pub_dentro_de_fn_o_de_test_no_es_api() {
        let dir = repo_de(&[(
            "src/a.rs",
            "pub fn externa() {\n    pub fn anidada() {}\n}\n#[cfg(test)]\nmod tests {\n    pub fn ayuda() {}\n}\n",
        )]);
        let mapa = construir(dir.path()).expect("construye");
        assert_eq!(modulo(&mapa, "a").api_publica, ["fn externa"]);
        assert_eq!(modulo(&mapa, "a").lineas, 7, "el código de test cuenta");
    }

    #[test]
    fn mapa_sin_src_da_mapa_vacio() {
        let dir = repo_de(&[("Cargo.toml", "[package]\n")]);
        let mapa = construir(dir.path()).expect("construye");
        assert_eq!(
            mapa,
            Mapa {
                modulos: vec![],
                aristas: vec![],
                total_lineas: 0,
            }
        );
    }

    #[test]
    fn mapa_ignora_target_y_tests_fuera_de_src() {
        let dir = repo_de(&[
            ("src/a.rs", "fn a() {}\n"),
            ("target/debug/build/x.rs", "fn x() {}\n"),
            ("tests/t.rs", "pub fn t() {}\n"),
        ]);
        let mapa = construir(dir.path()).expect("construye");
        assert_eq!(mapa.total_lineas, 1);
        assert_eq!(mapa.modulos.len(), 1);
        assert_eq!(mapa.modulos[0].nombre, "a");
    }

    #[test]
    fn mapa_nombres_de_modulos_omite_raiz() {
        let dir = repo_base();
        let mapa = construir(dir.path()).expect("construye");
        assert_eq!(
            mapa.nombres_de_modulos(),
            BTreeSet::from(["almacen".to_string(), "protocolo".to_string()])
        );
    }

    #[test]
    fn mapa_a_texto_contiene_modulos_y_dependencias() {
        let dir = repo_base();
        let mapa = construir(dir.path()).expect("construye");
        let texto = mapa.a_texto(10_000);
        assert!(texto.contains("almacen:"), "{texto}");
        assert!(texto.contains("protocolo:"), "{texto}");
        assert!(texto.contains("api: fn parsear, struct Trama"), "{texto}");
        assert!(
            texto.contains("almacen -> protocolo (src/almacen.rs:1)"),
            "{texto}"
        );
        assert!(!texto.contains(MARCA_RECORTE));
        assert_eq!(texto, mapa.a_texto(10_000), "es determinista");
    }

    #[test]
    fn mapa_a_texto_con_max_chars_chico_recorta_al_final() {
        let api: String = (0..60).map(|i| format!("pub fn f{i}() {{}}\n")).collect();
        let dir = repo_de(&[
            ("src/grande/mod.rs", api.as_str()),
            ("src/chico.rs", "pub fn uno() {}\n"),
        ]);
        let mapa = construir(dir.path()).expect("construye");
        // Ni sin la API de ningún módulo entra en 50 caracteres: el texto se corta al final.
        let max = 50;
        let texto = mapa.a_texto(max);
        assert!(
            texto.chars().count() <= max + MARCA_RECORTE.chars().count(),
            "largo {}",
            texto.chars().count()
        );
        assert!(texto.ends_with(MARCA_RECORTE), "{texto}");
    }

    #[test]
    fn mapa_a_texto_recorta_api_de_los_modulos_grandes_primero() {
        let api: String = (0..60).map(|i| format!("pub fn f{i}() {{}}\n")).collect();
        let dir = repo_de(&[
            ("src/grande/mod.rs", api.as_str()),
            ("src/chico.rs", "pub fn uno() {}\n"),
        ]);
        let mapa = construir(dir.path()).expect("construye");
        let completo = mapa.a_texto(1_000_000);
        // Recortar solo la API de `grande` ahorra cientos de caracteres: basta con 100 menos.
        let recortado = mapa.a_texto(completo.chars().count() - 100);
        assert!(
            recortado.contains("grande: 60 líneas, 1 archivos, api: recortada"),
            "{recortado}"
        );
        assert!(
            recortado.contains("chico: 1 líneas, 1 archivos, api: fn uno"),
            "{recortado}"
        );
        assert!(!recortado.ends_with(MARCA_RECORTE), "{recortado}");
    }

    #[test]
    fn mapa_archivos_destacados_ordena_por_tamano_y_desempata_por_ruta() {
        let cinco: String = (0..5).map(|i| format!("let v{i} = {i};\n")).collect();
        let dir = repo_de(&[
            ("src/a.rs", "fn a() {}\nfn a2() {}\nfn a3() {}\n"),
            ("src/b.rs", cinco.as_str()),
            ("src/c.rs", cinco.as_str()),
            ("src/d.rs", "fn d() {}\n"),
        ]);
        let mapa = construir(dir.path()).expect("construye");
        assert_eq!(mapa.archivos_destacados(2), ["src/b.rs", "src/c.rs"]);
        assert_eq!(
            mapa.archivos_destacados(10),
            ["src/b.rs", "src/c.rs", "src/a.rs", "src/d.rs"]
        );
    }

    #[test]
    fn mapa_del_repo_propio_tiene_oracle_y_sus_aristas() {
        let mapa = construir(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("construye");
        let oracle = modulo(&mapa, "oracle");
        assert!(!oracle.api_publica.is_empty());
        assert!(
            mapa.aristas
                .iter()
                .any(|a| a.desde == "oracle" && a.hacia == "config"),
            "falta la arista oracle -> config"
        );
        let nombres = mapa.nombres_de_modulos();
        for esperado in ["oracle", "track", "observe"] {
            assert!(nombres.contains(esperado), "falta {esperado}");
        }
    }
}
