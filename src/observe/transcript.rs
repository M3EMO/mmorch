//! Lee el transcript de `claude -p --output-format stream-json` y busca en él lecturas del oráculo.
//!
//! Por cada llamada a herramienta revisa la entrada (todos los strings de `input`) y la salida:
//! - `ruta`: una ruta real de `ROOT_HOME` o de `ROOT_HOME/oraculos/`. En la entrada, los tokens con
//!   forma de ruta se resuelven: `~`, `$HOME` y `$ROOT_HOME` se expanden; las relativas, contra el
//!   cwd del transcript o un `cd` previo del mismo comando; los enlaces se siguen. En la salida, el
//!   texto se mira tal cual.
//! - `nombre`: un archivo o una función `def test_...` del oráculo oculto que no aparece en
//!   `contrato/`. Los de contrato los ve el builder, así que no cuentan.
//! - `huella`: una línea larga y única de un archivo oculto, recortada, que aparece en la entrada o
//!   en la salida. Se descartan los imports, los comentarios y las líneas que también están en
//!   `contrato/`.
//!
//! Cada motivo da como mucho un `OraculoLeido` por llamada. La cita es el fragmento que coincidió,
//! recortado a 200 caracteres: nunca más que la línea que coincidió.
//!
//! Límite: la detección es un registro, no una defensa. Una lectura que no pasa por el texto de la
//! llamada (un dato que el builder recuerda, una copia que no nombra la ruta ni el nombre y que
//! cambia la huella) no se ve. La defensa es el aislamiento del builder (Tarea 26, capa 1).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::env;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::observe::{self, ErrorObserve};
use crate::tipos::{Evento, TipoEvento};

/// Largo máximo de la cita de un hallazgo, en caracteres.
const LARGO_CITA: usize = 200;
/// Largo mínimo de una línea del oráculo para servir de huella.
const LARGO_HUELLA: usize = 40;

/// Una llamada a herramienta del transcript, con la salida que le llegó.
#[derive(Debug, Clone, PartialEq)]
pub struct LlamadaHerramienta {
    pub id: String,
    pub nombre: String,
    pub entrada: Value,
    pub salida: String,
    /// Línea del transcript (desde 1) donde está la llamada, no su resultado.
    pub linea: usize,
}

/// Lo que dio el lector: el cwd inicial, las llamadas y las líneas que se saltaron.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Transcript {
    /// `cwd` del evento `system` `init`, si es absoluto.
    pub cwd: Option<PathBuf>,
    pub llamadas: Vec<LlamadaHerramienta>,
    /// Líneas no vacías que no son un objeto JSON (desde 1).
    pub lineas_saltadas: Vec<usize>,
}

/// Lo que el oráculo deja ver para buscarlo en un transcript. Se arma desde `ROOT_HOME` y la tarea.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HuellasOraculo {
    /// Rutas que cuentan como lectura: `ROOT_HOME` y `ROOT_HOME/oraculos`, tal cual y canonicalizadas.
    rutas: Vec<PathBuf>,
    /// `ROOT_HOME` tal como se configuró. Sirve para expandir `$ROOT_HOME`.
    root_home: PathBuf,
    /// `$HOME` para expandir `~`. Se pasa como parámetro, no se lee del entorno.
    home: Option<PathBuf>,
    /// Nombres de archivo y de funciones `def test_...` del oráculo oculto, sin los de contrato.
    nombres: Vec<String>,
    /// Líneas largas y únicas de los archivos ocultos, recortadas.
    lineas: Vec<String>,
}

/// Resultado de revisar un transcript: los eventos y los avisos de lo que se saltó.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RevisionTranscript {
    pub eventos: Vec<Evento>,
    pub avisos: Vec<String>,
}

/// Lee un transcript de `stream-json`. Una línea que no es un objeto JSON se salta y queda en
/// `lineas_saltadas`. Un archivo ilegible es error.
pub fn leer_transcript(ruta: &Path) -> Result<Transcript, ErrorObserve> {
    let texto = fs::read_to_string(ruta)?;
    let mut transcript = Transcript::default();
    let mut indice_por_id: HashMap<String, usize> = HashMap::new();
    for (indice, linea) in texto.lines().enumerate() {
        let numero = indice + 1;
        if linea.trim().is_empty() {
            continue;
        }
        let Ok(Value::Object(evento)) = serde_json::from_str::<Value>(linea) else {
            transcript.lineas_saltadas.push(numero);
            continue;
        };
        match evento.get("type").and_then(Value::as_str) {
            Some("system") => {
                let es_init = evento.get("subtype").and_then(Value::as_str) == Some("init");
                if es_init && transcript.cwd.is_none() {
                    transcript.cwd = evento
                        .get("cwd")
                        .and_then(Value::as_str)
                        .map(PathBuf::from)
                        .filter(|cwd| cwd.is_absolute());
                }
            }
            Some("assistant") => {
                for bloque in bloques(&evento).filter(|b| tipo_de(b) == "tool_use") {
                    let id = texto_de(bloque, "id");
                    indice_por_id.insert(id.clone(), transcript.llamadas.len());
                    transcript.llamadas.push(LlamadaHerramienta {
                        id,
                        nombre: texto_de(bloque, "name"),
                        entrada: bloque.get("input").cloned().unwrap_or(Value::Null),
                        salida: String::new(),
                        linea: numero,
                    });
                }
            }
            Some("user") => {
                for bloque in bloques(&evento).filter(|b| tipo_de(b) == "tool_result") {
                    let Some(&i) = indice_por_id.get(&texto_de(bloque, "tool_use_id")) else {
                        continue;
                    };
                    let salida = &mut transcript.llamadas[i].salida;
                    if !salida.is_empty() {
                        salida.push('\n');
                    }
                    salida.push_str(&texto_del_resultado(bloque.get("content")));
                }
            }
            _ => {}
        }
    }
    Ok(transcript)
}

/// Bloques de `message.content` de un evento. Si no hay, ninguno.
fn bloques(evento: &Map<String, Value>) -> impl Iterator<Item = &Value> {
    evento
        .get("message")
        .and_then(|mensaje| mensaje.get("content"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

fn tipo_de(bloque: &Value) -> &str {
    bloque.get("type").and_then(Value::as_str).unwrap_or("")
}

fn texto_de(valor: &Value, clave: &str) -> String {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// Texto de un `tool_result`: un string, o los bloques `text` de un arreglo.
fn texto_del_resultado(contenido: Option<&Value>) -> String {
    match contenido {
        Some(Value::String(texto)) => texto.clone(),
        Some(Value::Array(bloques)) => bloques
            .iter()
            .filter(|bloque| tipo_de(bloque) == "text")
            .filter_map(|bloque| bloque.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

impl HuellasOraculo {
    /// Arma las huellas de la tarea `tarea` dentro de `root_home`. `home` expande `~`.
    /// Falla si `oraculos/<tarea>/oculto/` no existe: sin ese archivo no hay nada que buscar.
    pub fn armar(
        root_home: &Path,
        home: Option<&Path>,
        tarea: &str,
    ) -> Result<HuellasOraculo, ErrorObserve> {
        if !root_home.is_absolute() {
            let error = io::Error::new(io::ErrorKind::InvalidInput, "ROOT_HOME debe ser absoluta");
            return Err(error.into());
        }
        let oraculos = root_home.join("oraculos");
        let carpeta = oraculos.join(tarea);
        let ocultos = archivos_bajo(&carpeta.join("oculto"))?;
        let carpeta_contrato = carpeta.join("contrato");
        let contratos = if carpeta_contrato.is_dir() {
            archivos_bajo(&carpeta_contrato)?
        } else {
            Vec::new()
        };

        // Nombres y texto de contrato: lo que el builder ya ve no cuenta como lectura.
        let mut texto_contrato = String::new();
        for archivo in &contratos {
            texto_contrato.push_str(&nombre_de(archivo));
            texto_contrato.push('\n');
            texto_contrato.push_str(&leer_lossy(archivo)?);
            texto_contrato.push('\n');
        }

        let mut nombres = BTreeSet::new();
        let mut conteo: BTreeMap<String, usize> = BTreeMap::new();
        for archivo in &ocultos {
            nombres.insert(nombre_de(archivo));
            for linea in leer_lossy(archivo)?.lines() {
                if let Some(nombre) = nombre_de_test(linea) {
                    nombres.insert(nombre);
                }
                let limpia = linea.trim();
                if !limpia.is_empty() {
                    *conteo.entry(limpia.to_string()).or_default() += 1;
                }
            }
        }
        let nombres = nombres
            .into_iter()
            .filter(|nombre| !aparece_bordeado(&texto_contrato, nombre))
            .collect();
        let lineas = conteo
            .into_iter()
            .filter(|(linea, veces)| {
                *veces == 1
                    && es_huella_candidata(linea)
                    && !texto_contrato.contains(linea.as_str())
            })
            .map(|(linea, _)| linea)
            .collect();

        let mut rutas = Vec::new();
        for ruta in [root_home.to_path_buf(), oraculos] {
            if let Ok(real) = fs::canonicalize(&ruta) {
                rutas.push(real);
            }
            rutas.push(ruta);
        }
        rutas.sort();
        rutas.dedup();

        Ok(HuellasOraculo {
            rutas,
            root_home: root_home.to_path_buf(),
            home: home.map(Path::to_path_buf),
            nombres,
            lineas,
        })
    }
}

/// Archivos bajo `carpeta`, recursivo, en orden. Un directorio inexistente es error.
fn archivos_bajo(carpeta: &Path) -> io::Result<Vec<PathBuf>> {
    let mut archivos = Vec::new();
    for entrada in fs::read_dir(carpeta)? {
        let entrada = entrada?;
        let ruta = entrada.path();
        if entrada.file_type()?.is_dir() {
            archivos.extend(archivos_bajo(&ruta)?);
        } else if ruta.is_file() {
            archivos.push(ruta);
        }
    }
    archivos.sort();
    Ok(archivos)
}

fn leer_lossy(ruta: &Path) -> io::Result<String> {
    Ok(String::from_utf8_lossy(&fs::read(ruta)?).into_owned())
}

fn nombre_de(ruta: &Path) -> String {
    ruta.file_name()
        .map(|nombre| nombre.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Revisa un transcript y devuelve un `OraculoLeido` por hallazgo.
pub fn revisar_transcript(
    transcript: &Path,
    corrida: &str,
    huellas: &HuellasOraculo,
) -> Result<Vec<Evento>, ErrorObserve> {
    revisar_transcript_con_avisos(transcript, corrida, huellas).map(|revision| revision.eventos)
}

/// Como `revisar_transcript`, pero también da los avisos de las líneas saltadas.
pub fn revisar_transcript_con_avisos(
    transcript: &Path,
    corrida: &str,
    huellas: &HuellasOraculo,
) -> Result<RevisionTranscript, ErrorObserve> {
    let leido = leer_transcript(transcript)?;
    let cwd = leido
        .cwd
        .clone()
        .or_else(|| env::current_dir().ok())
        .unwrap_or_default();
    let mut eventos = Vec::new();
    for llamada in &leido.llamadas {
        for hallazgo in hallazgos(llamada, &cwd, huellas) {
            eventos.push(evento(corrida, llamada, &hallazgo));
        }
    }
    let avisos = leido
        .lineas_saltadas
        .iter()
        .map(|linea| format!("línea {linea} no es un objeto JSON: se salta"))
        .collect();
    Ok(RevisionTranscript { eventos, avisos })
}

/// Revisa un transcript y agrega sus `OraculoLeido` a `eventos.jsonl` (`ruta_eventos`).
pub fn registrar_lecturas(
    transcript: &Path,
    corrida: &str,
    huellas: &HuellasOraculo,
    ruta_eventos: &Path,
) -> Result<RevisionTranscript, ErrorObserve> {
    let revision = revisar_transcript_con_avisos(transcript, corrida, huellas)?;
    for ev in &revision.eventos {
        observe::registrar_en(ruta_eventos, ev)?;
    }
    Ok(revision)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Motivo {
    Ruta,
    Nombre,
    Huella,
}

impl Motivo {
    fn nombre(self) -> &'static str {
        match self {
            Motivo::Ruta => "ruta",
            Motivo::Nombre => "nombre",
            Motivo::Huella => "huella",
        }
    }
}

struct Hallazgo {
    motivo: Motivo,
    cita: String,
}

/// Un hallazgo por motivo en la llamada, con la primera cita de cada uno.
fn hallazgos(llamada: &LlamadaHerramienta, cwd: &Path, huellas: &HuellasOraculo) -> Vec<Hallazgo> {
    let mut encontrados = Vec::new();
    for texto in cadenas(&llamada.entrada) {
        buscar_en(texto, true, cwd, huellas, &mut encontrados);
    }
    buscar_en(&llamada.salida, false, cwd, huellas, &mut encontrados);
    encontrados.sort_by_key(|hallazgo| hallazgo.motivo);
    encontrados.dedup_by_key(|hallazgo| hallazgo.motivo);
    encontrados
}

/// Busca en un texto. Solo la entrada (`es_entrada`) resuelve los tokens con forma de ruta.
fn buscar_en(
    texto: &str,
    es_entrada: bool,
    cwd: &Path,
    huellas: &HuellasOraculo,
    encontrados: &mut Vec<Hallazgo>,
) {
    if let Some(ruta) = huellas
        .rutas
        .iter()
        .find(|ruta| aparece_bordeado(texto, &ruta.to_string_lossy()))
    {
        encontrados.push(Hallazgo {
            motivo: Motivo::Ruta,
            cita: truncar(&ruta.to_string_lossy()),
        });
    }
    if es_entrada {
        let resueltas = rutas_nombradas(texto, cwd, huellas);
        if let Some((token, _)) = resueltas
            .iter()
            .find(|(_, ruta)| bajo_vigilados(ruta, huellas))
        {
            encontrados.push(Hallazgo {
                motivo: Motivo::Ruta,
                cita: truncar(token),
            });
        }
    }
    if let Some(nombre) = nombre_en_texto(texto, huellas) {
        encontrados.push(Hallazgo {
            motivo: Motivo::Nombre,
            cita: truncar(&nombre),
        });
    }
    if let Some(linea) = huella_en_texto(texto, huellas) {
        encontrados.push(Hallazgo {
            motivo: Motivo::Huella,
            cita: truncar(&linea),
        });
    }
}

/// Todos los strings de un valor JSON, recursivo.
fn cadenas(valor: &Value) -> Vec<&str> {
    match valor {
        Value::String(texto) => vec![texto.as_str()],
        Value::Array(items) => items.iter().flat_map(cadenas).collect(),
        Value::Object(campos) => campos.values().flat_map(cadenas).collect(),
        _ => Vec::new(),
    }
}

fn evento(corrida: &str, llamada: &LlamadaHerramienta, hallazgo: &Hallazgo) -> Evento {
    let mut ev = observe::evento_manual(corrida, TipoEvento::OraculoLeido, None);
    ev.fase = "transcript".into();
    ev.extra = Map::from_iter([
        ("herramienta".to_string(), json!(llamada.nombre)),
        ("linea".to_string(), json!(llamada.linea)),
        ("motivo".to_string(), json!(hallazgo.motivo.nombre())),
        ("cita".to_string(), json!(hallazgo.cita)),
    ]);
    ev
}

fn bajo_vigilados(ruta: &Path, huellas: &HuellasOraculo) -> bool {
    huellas
        .rutas
        .iter()
        .any(|vigilada| ruta.starts_with(vigilada))
}

/// Frontera de palabra: `buscado` aparece en `texto` sin letras, dígitos, `_` ni `-` pegados.
fn aparece_bordeado(texto: &str, buscado: &str) -> bool {
    if buscado.is_empty() {
        return false;
    }
    let es_palabra = |c: char| c.is_alphanumeric() || c == '_' || c == '-';
    texto.match_indices(buscado).any(|(i, _)| {
        let antes = texto[..i].chars().next_back();
        let despues = texto[i + buscado.len()..].chars().next();
        !antes.is_some_and(es_palabra) && !despues.is_some_and(es_palabra)
    })
}

/// Expande `~`, `$HOME`, `${HOME}`, `$ROOT_HOME` y `${ROOT_HOME}` al inicio de un token.
fn expandir(token: &str, huellas: &HuellasOraculo) -> PathBuf {
    let candidatos: [(&str, Option<&Path>); 5] = [
        ("~", huellas.home.as_deref()),
        ("$HOME", huellas.home.as_deref()),
        ("${HOME}", huellas.home.as_deref()),
        ("$ROOT_HOME", Some(huellas.root_home.as_path())),
        ("${ROOT_HOME}", Some(huellas.root_home.as_path())),
    ];
    for (prefijo, base) in candidatos {
        let Some(resto) = token.strip_prefix(prefijo) else {
            continue;
        };
        if !(resto.is_empty() || resto.starts_with('/')) {
            continue;
        }
        let Some(base) = base else {
            continue;
        };
        return base.join(resto.trim_start_matches('/'));
    }
    PathBuf::from(token)
}

/// Resuelve `ruta` recorriendo sus componentes: los enlaces existentes se siguen y `..` sube desde la
/// ruta física. Lo que no existe se deja en forma léxica.
fn resolver_fisico(ruta: &Path) -> PathBuf {
    let mut fisica = PathBuf::new();
    for componente in ruta.components() {
        match componente {
            Component::Prefix(prefijo) => fisica.push(prefijo.as_os_str()),
            Component::RootDir => fisica.push("/"),
            Component::CurDir => {}
            Component::ParentDir => {
                fisica.pop();
            }
            Component::Normal(nombre) => {
                fisica.push(nombre);
                if let Ok(real) = fs::canonicalize(&fisica) {
                    fisica = real;
                }
            }
        }
    }
    fisica
}

/// Resuelve un token contra `dir`: expansión, ruta absoluta o relativa, y ruta física.
fn resolver(token: &str, dir: &Path, huellas: &HuellasOraculo) -> PathBuf {
    let expandido = expandir(token, huellas);
    let completa = if expandido.is_absolute() {
        expandido
    } else {
        dir.join(expandido)
    };
    resolver_fisico(&completa)
}

/// Palabras de un segmento de comando, sin comillas ni separadores de shell.
fn palabras(segmento: &str) -> impl Iterator<Item = &str> {
    segmento
        .split(|c: char| c.is_whitespace() || "\"'`()<>=,".contains(c))
        .filter(|palabra| !palabra.is_empty())
}

/// Un token tiene forma de ruta si lleva `/` o empieza con `~`, `.` o `$`.
fn es_forma_de_ruta(token: &str) -> bool {
    token.contains('/') || token.starts_with(['~', '.', '$'])
}

/// Tokens con forma de ruta de un texto, cada uno con su ruta resuelta. Un `cd` simple del mismo
/// texto cambia el directorio para los segmentos que siguen.
fn rutas_nombradas(texto: &str, dir: &Path, huellas: &HuellasOraculo) -> Vec<(String, PathBuf)> {
    let mut salida = Vec::new();
    let mut actual = dir.to_path_buf();
    for segmento in texto.split(['\n', ';', '&', '|']) {
        let ps: Vec<&str> = palabras(segmento).collect();
        if ps.len() == 2 && ps[0] == "cd" && !ps[1].contains('$') && ps[1] != "-" {
            actual = resolver(ps[1], &actual, huellas);
            salida.push((ps[1].to_string(), actual.clone()));
            continue;
        }
        for palabra in ps.iter().filter(|palabra| es_forma_de_ruta(palabra)) {
            salida.push((palabra.to_string(), resolver(palabra, &actual, huellas)));
        }
    }
    salida
}

/// Primer nombre vigilado que aparece en `texto`, con frontera de palabra.
fn nombre_en_texto(texto: &str, huellas: &HuellasOraculo) -> Option<String> {
    huellas
        .nombres
        .iter()
        .find(|nombre| aparece_bordeado(texto, nombre))
        .cloned()
}

/// Primera huella que aparece en `texto`, como subcadena.
fn huella_en_texto(texto: &str, huellas: &HuellasOraculo) -> Option<String> {
    huellas
        .lineas
        .iter()
        .find(|linea| texto.contains(linea.as_str()))
        .cloned()
}

/// Nombre de la función si la línea es `def test_...`.
fn nombre_de_test(linea: &str) -> Option<String> {
    let resto = linea.trim_start().strip_prefix("def ")?;
    let nombre: String = resto
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    nombre.starts_with("test_").then_some(nombre)
}

/// Una línea del oráculo oculto sirve de huella si es larga y no es import ni comentario.
fn es_huella_candidata(linea: &str) -> bool {
    const PREFIJOS_COMENTARIO: [&str; 6] = ["#", "//", "/*", "*", "\"\"\"", "'''"];
    linea.chars().count() >= LARGO_HUELLA
        && !linea.starts_with("import ")
        && !(linea.starts_with("from ") && linea.contains(" import "))
        && !PREFIJOS_COMENTARIO
            .iter()
            .any(|prefijo| linea.starts_with(prefijo))
}

/// Recorta `texto` a `LARGO_CITA` caracteres.
fn truncar(texto: &str) -> String {
    texto.chars().take(LARGO_CITA).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn transcript_frontera_de_palabra_no_cuenta_prefijos_ni_pegados() {
        assert!(aparece_bordeado("cat /x/root/a", "/x/root"));
        assert!(!aparece_bordeado("cat /x/root-otro/a", "/x/root"));
        assert!(!aparece_bordeado("cat /y/x/root/a", "/x/root"));
        assert!(aparece_bordeado("cat t.py", "t.py"));
        assert!(!aparece_bordeado("cat st.py", "t.py"));
        assert!(!aparece_bordeado("cat t.pyc", "t.py"));
    }

    #[test]
    fn transcript_expande_tilde_y_variables_de_home_y_root_home() {
        let huellas = HuellasOraculo {
            rutas: vec!["/h/r".into()],
            root_home: "/h/r".into(),
            home: Some("/h".into()),
            ..HuellasOraculo::default()
        };
        assert_eq!(
            expandir("~/r/oraculos", &huellas),
            Path::new("/h/r/oraculos")
        );
        assert_eq!(expandir("$HOME/r", &huellas), Path::new("/h/r"));
        assert_eq!(
            expandir("${ROOT_HOME}/oraculos", &huellas),
            Path::new("/h/r/oraculos")
        );
        assert_eq!(expandir("$ROOT_HOME", &huellas), Path::new("/h/r"));
        assert_eq!(expandir("~otro/x", &huellas), Path::new("~otro/x"));
    }

    #[test]
    fn transcript_resolver_fisico_sigue_enlaces_antes_de_subir() {
        let tmp = tempfile::TempDir::new().expect("temp dir");
        let base = tmp.path().canonicalize().expect("canónica");
        fs::create_dir_all(base.join("b")).expect("crea b");
        fs::create_dir_all(base.join("a")).expect("crea a");
        std::os::unix::fs::symlink(base.join("b"), base.join("a/enlace")).expect("enlace");
        // Físicamente: a/enlace es b, y `..` sube a base. Léxicamente sería a/c/d.py.
        let resuelta = resolver_fisico(&base.join("a/enlace/../c/d.py"));
        assert_eq!(resuelta, base.join("c/d.py"));
    }

    #[test]
    fn transcript_rutas_nombradas_aplica_cd_previo_del_mismo_comando() {
        let huellas = HuellasOraculo::default();
        let resueltas = rutas_nombradas(
            "cd ../../r/oraculos && cat x/oculto/q.py",
            Path::new("/h/runs/c1"),
            &huellas,
        );
        assert!(
            resueltas
                .iter()
                .any(|(token, ruta)| token == "x/oculto/q.py"
                    && ruta == Path::new("/h/r/oraculos/x/oculto/q.py")),
            "{resueltas:?}"
        );
    }

    #[test]
    fn transcript_nombre_de_test_toma_solo_def_test() {
        assert_eq!(
            nombre_de_test("def test_sincroniza(tmp):").as_deref(),
            Some("test_sincroniza")
        );
        assert_eq!(
            nombre_de_test("    def test_x():").as_deref(),
            Some("test_x")
        );
        assert_eq!(nombre_de_test("def ayuda():"), None);
        assert_eq!(nombre_de_test("# def test_x():"), None);
    }

    #[test]
    fn transcript_huella_descarta_imports_comentarios_y_lineas_cortas() {
        assert!(!es_huella_candidata(
            "from memora.indice import sincronizar_todo_el_indice"
        ));
        assert!(!es_huella_candidata(
            "import pytest_asyncio_con_nombre_largo_para_probar_xyz"
        ));
        assert!(!es_huella_candidata(
            "# comentario largo que no debería contar como huella"
        ));
        assert!(!es_huella_candidata("assert x == 1"));
        assert!(es_huella_candidata(
            "assert sincronizar(tmp, \"clave-secreta-42\") == [1, 2, 3]"
        ));
    }

    #[test]
    fn transcript_cita_se_recorta_a_200_caracteres() {
        assert_eq!(truncar(&"a".repeat(300)).chars().count(), 200);
        assert_eq!(truncar("corta"), "corta");
    }
}
