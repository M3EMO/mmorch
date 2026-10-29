//! Tests de la documentación de interfaces (Tarea 19).
//!
//! `docs/interfaces.md` debe coincidir con el binario en cuatro puntos: los JSON Schema de
//! `r00t esquema`, los subcomandos de `r00t --help`, las herramientas de `tools/list` y los
//! ejemplos de `tarea.toml`, ficha y rúbrica, que deben cargar.
//! `docs/root.toml.example` debe cargar con `Config::desde_texto` y traer los precios por defecto.
//!
//! Para regenerar la sección `## Esquemas` tras cambiar un tipo:
//! `ROOT_ACTUALIZAR_SNAPSHOT=1 cargo test --test interfaces`.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use root::config::Config;
use root::oracle::rubrica::Rubrica;
use root::oracle::tarea::Tarea;
use serde_json::Value;

const VARIABLE_ACTUALIZAR: &str = "ROOT_ACTUALIZAR_SNAPSHOT";
const COMANDO_REGENERAR: &str = "ROOT_ACTUALIZAR_SNAPSHOT=1 cargo test --test interfaces";
/// Fuente de `Tarea`. Sus claves de nivel superior deben aparecer en la documentación.
const FUENTE_TAREA: &str = include_str!("../src/oracle/tarea.rs");
/// Tablas de `Config` que deben aparecer en la documentación.
const TABLAS_CONFIG: &[&str] = &[
    "balance",
    "ruteo",
    "proveedores",
    "precios",
    "modelos",
    "colab",
];

fn r00t(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_r00t"))
        .args(args)
        .output()
        .expect("no se pudo ejecutar el binario r00t")
}

fn ruta_doc(nombre: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("docs")
        .join(nombre)
}

fn leer(nombre: &str) -> String {
    fs::read_to_string(ruta_doc(nombre))
        .unwrap_or_else(|e| panic!("no se pudo leer docs/{nombre}: {e}"))
}

/// Tipos que acepta `r00t esquema`. Se leen del error de un tipo desconocido, que los lista.
fn tipos_publicos() -> Vec<String> {
    let salida = r00t(&["esquema", "tipo-que-no-existe"]);
    let stderr = String::from_utf8_lossy(&salida.stderr);
    let lista = stderr
        .split("Tipos: ")
        .nth(1)
        .expect("el error de esquema debe listar los tipos")
        .trim();
    let mut tipos: Vec<String> = lista.split(", ").map(str::to_string).collect();
    tipos.sort();
    tipos
}

/// Líneas de la sección `titulo` (una línea `## ...`), sin el encabezado, hasta la próxima `## `.
fn seccion<'a>(doc: &'a str, titulo: &str) -> Option<Vec<&'a str>> {
    let mut lineas = doc.lines();
    lineas.find(|l| l.trim_end() == titulo)?;
    Some(lineas.take_while(|l| !l.starts_with("## ")).collect())
}

/// Cuerpo del primer bloque ` ```lenguaje ` que sigue a `encabezado` y antes del próximo título.
fn bloque_bajo(lineas: &[&str], encabezado: &str, lenguaje: &str) -> Option<String> {
    let inicio = lineas.iter().position(|l| l.trim_end() == encabezado)?;
    let resto: Vec<&str> = lineas[inicio + 1..]
        .iter()
        .take_while(|l| !l.starts_with("### ") && !l.starts_with("## "))
        .copied()
        .collect();
    let abre = format!("```{lenguaje}");
    let pos = resto.iter().position(|l| l.trim_end() == abre.as_str())?;
    let cuerpo: Vec<&str> = resto[pos + 1..]
        .iter()
        .take_while(|l| l.trim_end() != "```")
        .copied()
        .collect();
    Some(cuerpo.join("\n"))
}

/// Reescribe la sección `## Esquemas` con la salida exacta de `r00t esquema` de cada tipo.
fn regenerar_esquemas(tipos: &[String]) {
    let mut bloques = String::from(
        "## Esquemas\n\nSalida exacta de `r00t esquema <tipo>`, en orden alfabético. \
         El test `tests/interfaces.rs` la compara con el binario.\n",
    );
    for tipo in tipos {
        let stdout = String::from_utf8(r00t(&["esquema", tipo.as_str()]).stdout)
            .expect("la salida de esquema es UTF-8");
        bloques.push_str(&format!(
            "\n### {tipo}\n\n```json\n{}\n```\n",
            stdout.trim_end()
        ));
    }
    let doc = leer("interfaces.md");
    let (mut antes, mut despues) = (String::new(), String::new());
    // 0: antes de la sección; 1: dentro; 2: después.
    let mut estado = 0;
    for linea in doc.split_inclusive('\n') {
        match estado {
            0 if linea.trim_end() == "## Esquemas" => estado = 1,
            0 => antes.push_str(linea),
            1 if linea.starts_with("## ") => {
                estado = 2;
                despues.push_str(linea);
            }
            1 => {}
            _ => despues.push_str(linea),
        }
    }
    let nuevo = if estado == 0 {
        format!("{}\n\n{bloques}", doc.trim_end())
    } else {
        let separador = if despues.is_empty() { "" } else { "\n" };
        format!("{antes}{bloques}{separador}{despues}")
    };
    let ruta = ruta_doc("interfaces.md");
    let temporal = ruta.with_extension("md.tmp");
    fs::write(&temporal, nuevo).expect("escribe el archivo temporal");
    fs::rename(&temporal, &ruta).expect("reemplaza docs/interfaces.md");
}

#[test]
fn esquema_de_cada_tipo_coincide_con_interfaces_md() {
    let tipos = tipos_publicos();
    assert!(!tipos.is_empty(), "r00t esquema no lista ningún tipo");
    if std::env::var_os(VARIABLE_ACTUALIZAR).is_some() {
        regenerar_esquemas(&tipos);
    }
    let doc = leer("interfaces.md");
    let lineas = seccion(&doc, "## Esquemas").unwrap_or_default();
    let mut fallos = Vec::new();
    for tipo in &tipos {
        let salida = r00t(&["esquema", tipo.as_str()]);
        let esperado: Value =
            serde_json::from_slice(&salida.stdout).expect("r00t esquema imprime JSON");
        match bloque_bajo(&lineas, &format!("### {tipo}"), "json") {
            None => fallos.push(format!("falta el bloque `json` de `{tipo}`")),
            Some(texto) => match serde_json::from_str::<Value>(&texto) {
                Ok(valor) if valor == esperado => {}
                Ok(_) => fallos.push(format!(
                    "el bloque de `{tipo}` no coincide con `r00t esquema {tipo}`"
                )),
                Err(e) => fallos.push(format!("el bloque de `{tipo}` no es JSON: {e}")),
            },
        }
    }
    assert!(
        fallos.is_empty(),
        "docs/interfaces.md, sección ## Esquemas:\n{}\nPara regenerar: {COMANDO_REGENERAR}",
        fallos.join("\n")
    );
}

/// Verdadero si la fila de la tabla empieza por `r00t <comando>` seguido de backtick o espacio.
fn fila_de(linea: &str, comando: &str) -> bool {
    let inicio = format!("| `r00t {comando}");
    linea.starts_with(&inicio) && linea[inicio.len()..].starts_with(['`', ' '])
}

#[test]
fn cli_tabla_cubre_cada_subcomando_de_help() {
    let ayuda = String::from_utf8(r00t(&["--help"]).stdout).expect("la ayuda es UTF-8");
    let mut comandos = Vec::new();
    let mut en_comandos = false;
    for linea in ayuda.lines() {
        if linea == "Commands:" {
            en_comandos = true;
            continue;
        }
        if !en_comandos {
            continue;
        }
        if linea.trim().is_empty() {
            break;
        }
        if let Some(nombre) = linea.split_whitespace().next()
            && nombre != "help"
        {
            comandos.push(nombre.to_string());
        }
    }
    assert!(!comandos.is_empty(), "r00t --help no lista comandos");
    let doc = leer("interfaces.md");
    let cli = seccion(&doc, "## CLI").expect("docs/interfaces.md debe tener ## CLI");
    let faltan: Vec<&String> = comandos
        .iter()
        .filter(|c| !cli.iter().any(|l| fila_de(l, c)))
        .collect();
    assert!(
        faltan.is_empty(),
        "subcomandos de r00t --help sin fila en ## CLI: {faltan:?}"
    );
}

#[test]
fn mcp_tabla_cubre_cada_herramienta_de_tools_list() {
    let mut hijo = Command::new(env!("CARGO_BIN_EXE_r00t"))
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("no se pudo iniciar r00t mcp");
    let mut entrada = hijo.stdin.take().expect("stdin del servidor");
    entrada
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}\n")
        .expect("escribe tools/list");
    drop(entrada);
    let salida = hijo.wait_with_output().expect("espera al servidor");
    let respuesta: Value = serde_json::from_slice(&salida.stdout).expect("respuesta JSON");
    let herramientas = respuesta["result"]["tools"]
        .as_array()
        .expect("tools/list devuelve una lista");
    assert!(!herramientas.is_empty());
    let doc = leer("interfaces.md");
    let mcp = seccion(&doc, "## MCP").expect("docs/interfaces.md debe tener ## MCP");
    let texto = mcp.join("\n");
    let faltan: Vec<&str> = herramientas
        .iter()
        .filter_map(|h| h["name"].as_str())
        .filter(|nombre| !texto.contains(nombre))
        .collect();
    assert!(
        faltan.is_empty(),
        "herramientas MCP sin documentar: {faltan:?}"
    );
}

/// Claves de nivel superior de `Tarea`, leídas de su struct en `src/oracle/tarea.rs`.
fn claves_de_tarea() -> Vec<String> {
    let inicio = FUENTE_TAREA
        .find("pub struct Tarea {")
        .expect("src/oracle/tarea.rs debe definir Tarea");
    let cuerpo = &FUENTE_TAREA[inicio..];
    let fin = cuerpo.find("\n}\n").expect("la struct Tarea debe cerrar");
    cuerpo[..fin]
        .lines()
        .skip(1)
        .filter_map(|l| {
            let resto = l.trim_start().strip_prefix("pub ")?;
            let nombre = resto.split(':').next()?.trim();
            Some(nombre.to_string())
        })
        .filter(|nombre| nombre != "dir")
        .collect()
}

/// Verdadero si el texto muestra la clave como tabla, arreglo de tablas, asignación o código.
fn menciona(texto: &str, clave: &str) -> bool {
    [
        format!("[{clave}]"),
        format!("[{clave}."),
        format!("{clave} ="),
        format!("`{clave}`"),
    ]
    .iter()
    .any(|patron| texto.contains(patron.as_str()))
}

#[test]
fn claves_de_tarea_y_tablas_de_config_se_mencionan() {
    let texto = format!("{}\n{}", leer("interfaces.md"), leer("root.toml.example"));
    let claves = claves_de_tarea();
    assert!(
        claves.len() >= 15,
        "Tarea debe tener sus claves de nivel superior: {claves:?}"
    );
    let tablas: Vec<String> = TABLAS_CONFIG.iter().map(|t| t.to_string()).collect();
    let faltan: Vec<&String> = claves
        .iter()
        .chain(tablas.iter())
        .filter(|c| !menciona(&texto, c))
        .collect();
    assert!(
        faltan.is_empty(),
        "claves de Tarea o tablas de Config sin mencionar en interfaces.md ni root.toml.example: {faltan:?}"
    );
}

#[test]
fn interfaces_y_evidencia_no_contienen_tbd() {
    for nombre in ["interfaces.md", "evidencia.md"] {
        assert!(
            !leer(nombre).contains("TBD"),
            "docs/{nombre} no debe contener TBD"
        );
    }
}

#[test]
fn interfaces_nombra_el_vault_solo_en_su_seccion() {
    let doc = leer("interfaces.md");
    assert!(
        seccion(&doc, "## Vault").is_some(),
        "docs/interfaces.md debe conservar ## Vault"
    );
    let mut en_vault = false;
    let mut fuera = Vec::new();
    for linea in doc.lines() {
        if linea.starts_with("## ") {
            en_vault = linea.trim_end() == "## Vault";
        }
        if !en_vault && linea.to_lowercase().contains("vault") {
            fuera.push(linea.to_string());
        }
    }
    assert!(fuera.is_empty(), "vault fuera de ## Vault: {fuera:?}");
}

#[test]
fn evidencia_declara_herencia_fuentes_y_cambios() {
    let doc = leer("evidencia.md");
    let primera = doc
        .lines()
        .find(|l| l.starts_with("## "))
        .unwrap_or_default();
    assert_eq!(
        primera, "## Evidencia heredada",
        "la primera sección debe declarar la evidencia heredada"
    );
    for marca in [
        "evidencia heredada",
        "decisión b1",
        "Tarea 12",
        "Tarea 12b",
        "medido-mmorch.md",
        "veredicto-ecc-f1.md",
        "mapa.md",
        "GOAL-mmorch.md",
        "bench-ecc-sdlc",
        "diferencias.md",
        "decisión 16",
        "Qué cambió desde el banco original",
    ] {
        assert!(
            doc.contains(marca),
            "docs/evidencia.md debe mencionar `{marca}`"
        );
    }
}

#[test]
fn root_toml_example_carga_con_los_defaults() {
    let texto = leer("root.toml.example");
    let cfg = Config::desde_texto(&texto).expect("docs/root.toml.example debe cargar");
    let por_defecto = Config::default();
    assert_eq!(cfg.precios, por_defecto.precios);
    assert_eq!(cfg.modelos, por_defecto.modelos);
    assert_eq!(cfg.balance, por_defecto.balance);
    assert_eq!(cfg.ruteo, por_defecto.ruteo);
    assert_eq!(cfg.umbral_verde, por_defecto.umbral_verde);
    assert_eq!(cfg.umbral_wilson, por_defecto.umbral_wilson);
    assert_eq!(cfg.proveedores, por_defecto.proveedores);
    assert!(
        cfg.avisos.is_empty(),
        "el ejemplo no usa nombres viejos: {:?}",
        cfg.avisos
    );
}

#[test]
fn root_toml_example_cita_el_origen_de_cada_precio() {
    let texto = leer("root.toml.example");
    let lineas: Vec<&str> = texto.lines().collect();
    let modelos: Vec<usize> = lineas
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with("[precios."))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(modelos.len(), 4, "un bloque por modelo por defecto");
    for i in modelos {
        let previa = i.checked_sub(1).map(|j| lineas[j]).unwrap_or_default();
        assert!(
            previa.starts_with('#') && previa.contains("origen"),
            "el bloque {} no cita su origen: {previa:?}",
            lineas[i]
        );
    }
}

#[test]
fn ejemplo_de_tarea_toml_de_interfaces_carga() {
    let doc = leer("interfaces.md");
    let lineas: Vec<&str> = doc.lines().collect();
    let texto = bloque_bajo(&lineas, "### Ejemplo de tarea.toml", "toml")
        .expect("falta el bloque toml bajo ### Ejemplo de tarea.toml");
    let dir = tempfile::tempdir().expect("directorio temporal");
    fs::write(dir.path().join("tarea.toml"), texto).expect("escribe tarea.toml");
    Tarea::cargar_de(dir.path()).expect("el ejemplo de tarea.toml debe cargar");
}

#[test]
fn ejemplo_de_ficha_de_interfaces_se_acepta() {
    let doc = leer("interfaces.md");
    let lineas: Vec<&str> = doc.lines().collect();
    let texto = bloque_bajo(&lineas, "### Ejemplo de ficha", "toml")
        .expect("falta el bloque toml bajo ### Ejemplo de ficha");
    let dir = tempfile::tempdir().expect("directorio temporal");
    let spec = dir.path().join("SPEC.md");
    fs::write(
        &spec,
        format!("# Spec\n\n## Ficha\n\n```toml\n{texto}\n```\n"),
    )
    .expect("escribe SPEC.md");
    let salida = r00t(&[
        "ficha",
        "tarea",
        "--spec",
        spec.to_str().expect("ruta UTF-8"),
        "--nombre",
        "ejemplo",
    ]);
    assert_eq!(
        salida.status.code(),
        Some(0),
        "la ficha de ejemplo debe aceptarse: {}",
        String::from_utf8_lossy(&salida.stderr)
    );
}

#[test]
fn ejemplo_de_rubrica_de_interfaces_carga() {
    let doc = leer("interfaces.md");
    let lineas: Vec<&str> = doc.lines().collect();
    let texto = bloque_bajo(&lineas, "### Ejemplo de rúbrica", "toml")
        .expect("falta el bloque toml bajo ### Ejemplo de rúbrica");
    Rubrica::desde_toml(&texto).expect("el ejemplo de rúbrica debe cargar");
}
