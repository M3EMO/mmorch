//! Sondas de configuración del proyecto evaluado (Tarea 9b): TOML, YAML y Dockerfile.
//! `yaml-rust2` se usa solo en este archivo. Root nunca lee su propia configuración en YAML.

use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::oracle::perfiles::herramientas::{
    EjecutorHerramienta, con_veredicto, falta_herramienta, sin_veredicto,
};
use crate::oracle::perfiles::{ResultadoSonda, crear_hallazgo};
use crate::oracle::tarea::{Requisito, Tarea};
use crate::tipos::{Dimension, Hallazgo};

const SONDA_ESTRUCTURA: &str = "configs";
const SONDA_SECRETOS: &str = "configs-secretos";
const SONDA_DOCKERFILE: &str = "dockerfile";
const SONDA_HADOLINT: &str = "hadolint";
/// Detalle de la regla (b): el último stage corre como root. La sonda de entrega lo reusa.
const DETALLE_RAIZ: &str =
    "la última etapa no define un USER no-root: el contenedor corre como root";
/// Carpetas que no se recorren, en cualquier nivel del repo.
const CARPETAS_IGNORADAS: &[&str] = &["target", ".git", "node_modules", "vendor"];
/// Palabras que, dentro de una clave, marcan un secreto.
const PALABRAS_SECRETO: &[&str] = &["password", "token", "api_key", "apikey", "secret"];
/// Valores de relleno que no son secretos reales.
const PLACEHOLDERS: &[&str] = &["changeme", "xxx", "redacted", "example"];

/// Tipo de archivo de configuración.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoConfig {
    Toml,
    Yaml,
    Dockerfile,
}

/// Archivo de configuración hallado en el repo. `ruta_relativa` usa `/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchivoConfig {
    pub ruta_relativa: String,
    pub tipo: TipoConfig,
}

/// Busca los archivos de configuración del repo, en orden alfabético.
pub fn buscar_archivos(repo: &Path) -> Vec<ArchivoConfig> {
    let mut salida = Vec::new();
    recorrer(repo, repo, &mut salida);
    salida.sort_by(|a, b| a.ruta_relativa.cmp(&b.ruta_relativa));
    salida
}

fn recorrer(repo: &Path, dir: &Path, salida: &mut Vec<ArchivoConfig>) {
    let Ok(entradas) = fs::read_dir(dir) else {
        return;
    };
    for entrada in entradas.flatten() {
        // `file_type` no sigue enlaces simbólicos: un enlace no entra ni causa bucles.
        let Ok(tipo_entrada) = entrada.file_type() else {
            continue;
        };
        let nombre = entrada.file_name().to_string_lossy().into_owned();
        let ruta = entrada.path();
        if tipo_entrada.is_dir() {
            if !CARPETAS_IGNORADAS.contains(&nombre.as_str()) {
                recorrer(repo, &ruta, salida);
            }
        } else if let Some(tipo) = tipo_de(&nombre).filter(|_| tipo_entrada.is_file()) {
            let rel = ruta.strip_prefix(repo).unwrap_or(&ruta);
            salida.push(ArchivoConfig {
                ruta_relativa: rel.to_string_lossy().replace('\\', "/"),
                tipo,
            });
        }
    }
}

fn tipo_de(nombre: &str) -> Option<TipoConfig> {
    if nombre.ends_with(".toml") {
        Some(TipoConfig::Toml)
    } else if nombre.ends_with(".yaml") || nombre.ends_with(".yml") {
        Some(TipoConfig::Yaml)
    } else if nombre == "Dockerfile"
        || nombre == "Containerfile"
        || nombre.starts_with("Dockerfile.")
    {
        Some(TipoConfig::Dockerfile)
    } else {
        None
    }
}

/// Las cuatro sondas de configuración. Valen para cualquier lenguaje.
pub fn sondas_configs(
    repo: &Path,
    ej: &dyn EjecutorHerramienta,
    tarea: &Tarea,
) -> Vec<ResultadoSonda> {
    let reqs = &tarea.requisito;
    vec![
        sonda_estructura_configs(repo, reqs),
        sonda_secretos_configs(repo, reqs),
        sonda_dockerfile_reglas(repo, reqs),
        sonda_hadolint(ej, repo, reqs),
    ]
}

/// Parsea la salida JSON de hadolint y arma los hallazgos del archivo `archivo`.
/// `error` da severidad 2 y `warning` severidad 1. `info` y `style` se ignoran.
pub fn parsear_hadolint(
    json: &str,
    archivo: &str,
    requisitos: &[Requisito],
) -> Result<Vec<Hallazgo>, String> {
    let defectos: Vec<DefectoHadolint> = serde_json::from_str(json).map_err(|e| e.to_string())?;
    Ok(defectos
        .into_iter()
        .filter_map(|d| {
            let severidad = match d.level.as_str() {
                "error" => 2,
                "warning" => 1,
                _ => return None,
            };
            Some(crear_hallazgo(
                Dimension::Entrega,
                severidad,
                archivo,
                d.line,
                &d.code,
                &d.message,
                SONDA_HADOLINT,
                requisitos,
            ))
        })
        .collect())
}

/// Defecto de la salida JSON de hadolint. `file` no hace falta: el archivo lo pone quien llama.
#[derive(Deserialize)]
struct DefectoHadolint {
    code: String,
    level: String,
    line: u32,
    #[serde(default)]
    message: String,
}

fn leer(repo: &Path, ruta: &str) -> Result<String, String> {
    fs::read_to_string(repo.join(ruta)).map_err(|e| format!("no se pudo leer {ruta}: {e}"))
}

/// Línea (1-based) y cita de un punto del texto. Si la línea está en blanco, sube a la anterior
/// con texto. Así la cita nunca sale vacía.
fn ubicar(texto: &str, linea: u32) -> (u32, String) {
    let lineas: Vec<&str> = texto.lines().collect();
    let mut indice = (linea as usize).clamp(1, lineas.len().max(1)) - 1;
    loop {
        let limpia = lineas.get(indice).map_or("", |l| l.trim());
        if !limpia.is_empty() || indice == 0 {
            return (indice as u32 + 1, limpia.to_string());
        }
        indice -= 1;
    }
}

/// Línea (1-based) de un byte del texto.
fn linea_de_byte(texto: &str, byte: usize) -> u32 {
    let hasta = byte.min(texto.len());
    let saltos = texto.as_bytes()[..hasta]
        .iter()
        .filter(|&&b| b == b'\n')
        .count();
    saltos as u32 + 1
}

/// Error de parseo de TOML: línea del `span`, o la 1 si no hay `span`.
fn error_toml(texto: &str) -> Option<(u32, String)> {
    let error = toml::from_str::<toml::Table>(texto).err()?;
    let linea = error.span().map_or(1, |r| linea_de_byte(texto, r.start));
    Some((linea, format!("no parsea como TOML: {}", error.message())))
}

/// Error de parseo de YAML. Si hay una tabulación de indentación en una línea anterior o igual
/// a la que informa `yaml-rust2`, esa tabulación es la línea del hallazgo.
fn error_yaml(texto: &str) -> Option<(u32, String)> {
    let error = yaml_rust2::YamlLoader::load_from_str(texto).err()?;
    let reportada = error.marker().line() as u32;
    let linea = match primera_tabulacion(texto) {
        Some(tab) if tab <= reportada => tab,
        _ => reportada,
    };
    Some((linea, format!("no parsea como YAML: {}", error.info())))
}

/// Primera línea cuyo prefijo de espacios en blanco contiene una tabulación.
fn primera_tabulacion(texto: &str) -> Option<u32> {
    texto
        .lines()
        .position(|l| {
            l.chars()
                .take_while(|c| c.is_whitespace())
                .any(|c| c == '\t')
        })
        .map(|i| i as u32 + 1)
}

fn sonda_estructura_configs(repo: &Path, requisitos: &[Requisito]) -> ResultadoSonda {
    let d = Dimension::Estructura;
    let mut hallazgos = Vec::new();
    for archivo in buscar_archivos(repo) {
        let error = match archivo.tipo {
            TipoConfig::Dockerfile => continue,
            TipoConfig::Toml | TipoConfig::Yaml => {
                let texto = match leer(repo, &archivo.ruta_relativa) {
                    Ok(t) => t,
                    Err(motivo) => return sin_veredicto(d, motivo),
                };
                let error = if archivo.tipo == TipoConfig::Toml {
                    error_toml(&texto)
                } else {
                    error_yaml(&texto)
                };
                error.map(|(linea, detalle)| (texto, linea, detalle))
            }
        };
        if let Some((texto, linea, detalle)) = error {
            let (linea, cita) = ubicar(&texto, linea);
            hallazgos.push(crear_hallazgo(
                d,
                2,
                &archivo.ruta_relativa,
                linea,
                &cita,
                &detalle,
                SONDA_ESTRUCTURA,
                requisitos,
            ));
        }
    }
    con_veredicto(d, hallazgos, requisitos)
}

fn sonda_secretos_configs(repo: &Path, requisitos: &[Requisito]) -> ResultadoSonda {
    let d = Dimension::Seguridad;
    let mut hallazgos = Vec::new();
    for archivo in buscar_archivos(repo) {
        let yaml = match archivo.tipo {
            TipoConfig::Toml => false,
            TipoConfig::Yaml => true,
            TipoConfig::Dockerfile => continue,
        };
        let texto = match leer(repo, &archivo.ruta_relativa) {
            Ok(t) => t,
            Err(motivo) => return sin_veredicto(d, motivo),
        };
        for (i, linea) in texto.lines().enumerate() {
            if let Some(clave) = clave_con_secreto(linea, yaml) {
                hallazgos.push(crear_hallazgo(
                    d,
                    3,
                    &archivo.ruta_relativa,
                    i as u32 + 1,
                    &clave,
                    "clave con forma de secreto y valor literal",
                    SONDA_SECRETOS,
                    requisitos,
                ));
            }
        }
    }
    con_veredicto(d, hallazgos, requisitos)
}

/// Clave de una línea `clave = valor` (TOML) o `clave: valor` (YAML) con secreto literal.
fn clave_con_secreto(linea: &str, yaml: bool) -> Option<String> {
    let mut texto = linea.trim_start();
    if texto.starts_with('#') {
        return None;
    }
    let (clave_cruda, valor_crudo) = if yaml {
        texto = texto.strip_prefix("- ").map_or(texto, str::trim_start);
        separar_yaml(texto)?
    } else {
        texto.split_once('=')?
    };
    let clave = clave_cruda.trim().trim_matches(['"', '\'']).trim();
    if clave.is_empty() || !es_clave_secreta(clave) || !es_literal_secreto(valor_crudo) {
        return None;
    }
    Some(clave.to_string())
}

/// Separa en el primer `:` seguido de espacio o del fin de la línea.
fn separar_yaml(linea: &str) -> Option<(&str, &str)> {
    let indice = linea
        .char_indices()
        .find(|&(i, c)| {
            c == ':'
                && linea[i + 1..]
                    .chars()
                    .next()
                    .is_none_or(char::is_whitespace)
        })?
        .0;
    Some((&linea[..indice], &linea[indice + 1..]))
}

fn es_clave_secreta(clave: &str) -> bool {
    let minuscula = clave.to_lowercase();
    PALABRAS_SECRETO.iter().copied().any(|palabra| {
        minuscula.match_indices(palabra).any(|(i, _)| {
            // `token` seguido de `s` (max_tokens, num_tokens) es un conteo, no un secreto.
            palabra != "token" || !minuscula[i + palabra.len()..].starts_with('s')
        })
    })
}

/// Verdadero si el valor, sin comillas, es un número o un booleano.
fn es_numero_o_booleano(valor: &str) -> bool {
    matches!(valor, "true" | "false")
        || (valor.starts_with(|c: char| c.is_ascii_digit() || matches!(c, '-' | '+' | '.'))
            && valor.parse::<f64>().is_ok())
}

/// Contenido de un valor: sin comillas y sin comentario final. Vacío si no hay contenido.
fn contenido_de(crudo: &str) -> String {
    let valor = crudo.trim();
    if valor.starts_with('#') {
        return String::new();
    }
    if let Some(comilla) = valor.chars().next().filter(|c| *c == '"' || *c == '\'') {
        let resto = &valor[1..];
        let fin = resto.find(comilla).unwrap_or(resto.len());
        return resto[..fin].trim().to_string();
    }
    let fin = valor.find(" #").unwrap_or(valor.len());
    valor[..fin].trim().to_string()
}

/// Verdadero si el valor es un literal que parece un secreto. Vacío, referencia, plantilla,
/// placeholder, `null` de YAML, o número y booleano sin comillas dan falso.
fn es_literal_secreto(crudo: &str) -> bool {
    let valor = contenido_de(crudo);
    let minuscula = valor.to_lowercase();
    if valor.is_empty() || matches!(minuscula.as_str(), "~" | "null") {
        return false;
    }
    // Sin comillas, un número o un booleano no es secreto. Entre comillas, sí cuenta.
    let entrecomillado = crudo.trim().starts_with(['"', '\'']);
    if !entrecomillado && es_numero_o_booleano(&minuscula) {
        return false;
    }
    if valor.starts_with(['{', '[', '|', '>', '&', '*']) {
        return false;
    }
    if valor.contains("${") || valor.contains("{{") || valor.starts_with('$') {
        return false;
    }
    if minuscula.starts_with("env:") || (valor.contains('<') && valor.contains('>')) {
        return false;
    }
    !PLACEHOLDERS.iter().any(|p| minuscula.contains(p))
}

/// Hallazgos de la regla (b) "corre como root" de los Dockerfile del repo. Un error de lectura
/// da `Err`. Lo usa la sonda de entrega (Tarea 9d) para repetir solo ese caso.
pub fn dockerfile_corre_como_root(
    repo: &Path,
    requisitos: &[Requisito],
) -> Result<Vec<Hallazgo>, String> {
    let mut hallazgos = Vec::new();
    for archivo in buscar_archivos(repo) {
        if archivo.tipo != TipoConfig::Dockerfile {
            continue;
        }
        let texto = leer(repo, &archivo.ruta_relativa)?;
        hallazgos.extend(
            reglas_dockerfile(&archivo.ruta_relativa, &texto, requisitos)
                .into_iter()
                .filter(|h| h.detalle == DETALLE_RAIZ),
        );
    }
    Ok(hallazgos)
}

fn sonda_dockerfile_reglas(repo: &Path, requisitos: &[Requisito]) -> ResultadoSonda {
    let d = Dimension::Seguridad;
    let mut hallazgos = Vec::new();
    for archivo in buscar_archivos(repo) {
        if archivo.tipo != TipoConfig::Dockerfile {
            continue;
        }
        let texto = match leer(repo, &archivo.ruta_relativa) {
            Ok(t) => t,
            Err(motivo) => return sin_veredicto(d, motivo),
        };
        hallazgos.extend(reglas_dockerfile(
            &archivo.ruta_relativa,
            &texto,
            requisitos,
        ));
    }
    con_veredicto(d, hallazgos, requisitos)
}

/// Una instrucción del Dockerfile, con las líneas continuadas unidas.
struct Instruccion {
    linea: u32,
    /// Primera línea física, recortada.
    cita: String,
    /// Texto lógico, sin `\` de continuación.
    logico: String,
}

fn instrucciones(texto: &str) -> Vec<Instruccion> {
    let mut salida = Vec::new();
    let mut actual: Option<Instruccion> = None;
    for (i, linea) in texto.lines().enumerate() {
        let limpia = linea.trim();
        if limpia.is_empty() || limpia.starts_with('#') {
            continue;
        }
        let inst = actual.get_or_insert_with(|| Instruccion {
            linea: i as u32 + 1,
            cita: limpia.to_string(),
            logico: String::new(),
        });
        let (parte, sigue) = match limpia.strip_suffix('\\') {
            Some(p) => (p.trim_end(), true),
            None => (limpia, false),
        };
        if !inst.logico.is_empty() {
            inst.logico.push(' ');
        }
        inst.logico.push_str(parte);
        if !sigue {
            salida.extend(actual.take());
        }
    }
    salida.extend(actual.take());
    salida
}

/// Palabra clave en mayúsculas, y el resto de la instrucción recortado.
fn palabra_clave(logico: &str) -> (String, &str) {
    let texto = logico.trim_start();
    let fin = texto.find(char::is_whitespace).unwrap_or(texto.len());
    (texto[..fin].to_uppercase(), texto[fin..].trim())
}

/// Imagen y nombre de etapa (`AS x`) de un `FROM`. Las banderas `--x=y` se ignoran.
fn parsear_from(resto: &str) -> (Option<String>, Option<String>) {
    let mut tokens = resto.split_whitespace().filter(|t| !t.starts_with("--"));
    let imagen = tokens.next().map(str::to_string);
    let alias = match (tokens.next(), tokens.next()) {
        (Some(as_), Some(nombre)) if as_.eq_ignore_ascii_case("as") => Some(nombre.to_string()),
        _ => None,
    };
    (imagen, alias)
}

/// Verdadero si la imagen no fija versión: sin tag, o con `:latest`.
/// `scratch`, alias de etapas previas, digests y referencias `$VAR` no cuentan.
fn imagen_sin_version(imagen: &str, alias_previos: &[String]) -> bool {
    if imagen.eq_ignore_ascii_case("scratch")
        || imagen.contains('@')
        || imagen.contains('$')
        || alias_previos.iter().any(|a| a.eq_ignore_ascii_case(imagen))
    {
        return false;
    }
    let ultimo = imagen.rsplit('/').next().unwrap_or(imagen);
    match ultimo.split_once(':') {
        None => true,
        Some((_, tag)) => tag.is_empty() || tag == "latest",
    }
}

/// Pares `clave, valor` de un `ENV` o `ARG`. `ENV clave valor` (forma antigua) da un par.
fn pares(kw: &str, resto: &str) -> Vec<(String, String)> {
    let tokens = tokens_con_comillas(resto);
    let Some(primero) = tokens.first() else {
        return Vec::new();
    };
    if kw == "ENV" && !primero.contains('=') {
        let tras = resto.get(primero.len()..).unwrap_or("").trim();
        return vec![(primero.clone(), tras.to_string())];
    }
    tokens
        .iter()
        .filter_map(|t| {
            t.split_once('=')
                .map(|(k, v)| (k.to_string(), v.to_string()))
        })
        .collect()
}

/// Separa por espacios sin partir lo que va entre comillas.
fn tokens_con_comillas(texto: &str) -> Vec<String> {
    let mut salida = Vec::new();
    let mut actual = String::new();
    let mut comilla: Option<char> = None;
    for c in texto.chars() {
        match comilla {
            Some(q) => {
                actual.push(c);
                if c == q {
                    comilla = None;
                }
            }
            None if c == '"' || c == '\'' => {
                comilla = Some(c);
                actual.push(c);
            }
            None if c.is_whitespace() => {
                if !actual.is_empty() {
                    salida.push(std::mem::take(&mut actual));
                }
            }
            None => actual.push(c),
        }
    }
    if !actual.is_empty() {
        salida.push(actual);
    }
    salida
}

/// Reglas propias del Dockerfile: (a) imagen sin versión, (b) último stage sin `USER` no-root,
/// (c) secreto en `ENV` o `ARG`.
fn reglas_dockerfile(archivo: &str, texto: &str, requisitos: &[Requisito]) -> Vec<Hallazgo> {
    let mut hallazgos = Vec::new();
    let mut alias_previos: Vec<String> = Vec::new();
    let mut ultimo_from: Option<Instruccion> = None;
    let mut ultimo_user: Option<String> = None;
    for inst in instrucciones(texto) {
        let (kw, resto) = palabra_clave(&inst.logico);
        match kw.as_str() {
            "FROM" => {
                let (imagen, alias) = parsear_from(resto);
                if imagen.is_some_and(|i| imagen_sin_version(&i, &alias_previos)) {
                    hallazgos.push(crear_hallazgo(
                        Dimension::Seguridad,
                        2,
                        archivo,
                        inst.linea,
                        &inst.cita,
                        "imagen base sin versión fija: usar un tag concreto o un digest",
                        SONDA_DOCKERFILE,
                        requisitos,
                    ));
                }
                alias_previos.extend(alias.map(|a| a.to_lowercase()));
                ultimo_user = None;
                ultimo_from = Some(inst);
            }
            "USER" if !resto.is_empty() => ultimo_user = Some(resto.to_string()),
            "ENV" | "ARG" => {
                for (clave, valor) in pares(&kw, resto) {
                    if es_clave_secreta(&clave) && es_literal_secreto(&valor) {
                        hallazgos.push(crear_hallazgo(
                            Dimension::Seguridad,
                            3,
                            archivo,
                            inst.linea,
                            &clave,
                            "secreto literal en ENV o ARG: va a la imagen",
                            SONDA_DOCKERFILE,
                            requisitos,
                        ));
                    }
                }
            }
            _ => {}
        }
    }
    if let Some(from) = ultimo_from {
        let raiz = match ultimo_user.as_deref() {
            None => true,
            Some(usuario) => {
                let nombre = usuario.split(':').next().unwrap_or("").to_lowercase();
                nombre == "root" || nombre == "0"
            }
        };
        if raiz {
            hallazgos.push(crear_hallazgo(
                Dimension::Seguridad,
                3,
                archivo,
                from.linea,
                &from.cita,
                DETALLE_RAIZ,
                SONDA_DOCKERFILE,
                requisitos,
            ));
        }
    }
    hallazgos
}

fn sonda_hadolint(
    ej: &dyn EjecutorHerramienta,
    repo: &Path,
    requisitos: &[Requisito],
) -> ResultadoSonda {
    let d = Dimension::Entrega;
    let dockerfiles: Vec<ArchivoConfig> = buscar_archivos(repo)
        .into_iter()
        .filter(|a| a.tipo == TipoConfig::Dockerfile)
        .collect();
    if dockerfiles.is_empty() {
        return ResultadoSonda {
            dimension: d,
            valor: Some(1.0),
            motivo: None,
            hallazgos: Vec::new(),
        };
    }
    if let Some(sin) = falta_herramienta(ej, d, "hadolint") {
        return sin;
    }
    let mut hallazgos = Vec::new();
    for archivo in &dockerfiles {
        let cmd = format!(
            "hadolint --format json {}",
            ruta_en_contenedor(&archivo.ruta_relativa)
        );
        let salida = match ej.ejecutar(&cmd) {
            Ok(s) => s,
            Err(e) => return sin_veredicto(d, e.to_string()),
        };
        match parsear_hadolint(&salida.stdout, &archivo.ruta_relativa, requisitos) {
            Ok(mut hs) => hallazgos.append(&mut hs),
            Err(e) => {
                return sin_veredicto(
                    d,
                    format!(
                        "salida de hadolint ilegible en {}: {e}",
                        archivo.ruta_relativa
                    ),
                );
            }
        }
    }
    con_veredicto(d, hallazgos, requisitos)
}

/// Ruta del archivo dentro del contenedor, con comillas si tiene caracteres raros.
fn ruta_en_contenedor(ruta: &str) -> String {
    let destino = format!("/repo/{ruta}");
    let simple = destino
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "/._-".contains(c));
    if simple {
        destino
    } else {
        format!("'{}'", destino.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn escribir(repo: &Path, ruta: &str, texto: &str) {
        let archivo = repo.join(ruta);
        fs::create_dir_all(archivo.parent().expect("con padre")).expect("crea carpeta");
        fs::write(archivo, texto).expect("escribe el archivo");
    }

    fn repo_con(ruta: &str, texto: &str) -> tempfile::TempDir {
        let repo = tempfile::tempdir().expect("temp dir");
        escribir(repo.path(), ruta, texto);
        repo
    }

    fn rutas(repo: &Path) -> Vec<String> {
        buscar_archivos(repo)
            .into_iter()
            .map(|a| a.ruta_relativa)
            .collect()
    }

    fn serializado(r: &ResultadoSonda) -> String {
        serde_json::to_string(&r.hallazgos).expect("serializa")
    }

    #[test]
    fn configs_buscar_toma_toml_yaml_yml_y_dockerfiles() {
        let repo = tempfile::tempdir().expect("temp dir");
        for r in [
            "a.toml",
            "b.yaml",
            "c.yml",
            "Dockerfile",
            "Containerfile",
            "Dockerfile.dev",
            "Cargo.lock",
            "notas.txt",
        ] {
            escribir(repo.path(), r, "x");
        }
        assert_eq!(
            rutas(repo.path()),
            [
                "Containerfile",
                "Dockerfile",
                "Dockerfile.dev",
                "a.toml",
                "b.yaml",
                "c.yml"
            ]
        );
        let tipos: Vec<TipoConfig> = buscar_archivos(repo.path())
            .iter()
            .map(|a| a.tipo)
            .collect();
        assert_eq!(tipos[1], TipoConfig::Dockerfile);
        assert_eq!(tipos[3], TipoConfig::Toml);
        assert_eq!(tipos[4], TipoConfig::Yaml);
    }

    #[test]
    fn configs_buscar_ignora_target_git_node_modules_y_vendor() {
        let repo = tempfile::tempdir().expect("temp dir");
        for r in [
            "target/x.toml",
            ".git/y.toml",
            "node_modules/a/z.yaml",
            "vendor/w/Dockerfile",
            "sub/target/q.toml",
            "src/ok.toml",
        ] {
            escribir(repo.path(), r, "x");
        }
        assert_eq!(rutas(repo.path()), ["src/ok.toml"]);
    }

    #[test]
    fn configs_buscar_repo_vacio_da_lista_vacia() {
        let repo = tempfile::tempdir().expect("temp dir");
        assert!(buscar_archivos(repo.path()).is_empty());
    }

    #[test]
    fn configs_toml_roto_da_hallazgo_con_su_linea() {
        let repo = repo_con("x.toml", "x = 1\ny = \"abc\nz = 2\n");
        let r = sonda_estructura_configs(repo.path(), &[]);
        assert_eq!(r.hallazgos.len(), 1);
        let h = &r.hallazgos[0];
        assert_eq!((h.archivo.as_str(), h.linea, h.severidad), ("x.toml", 2, 2));
        assert_eq!(h.cita, "y = \"abc");
    }

    #[test]
    fn configs_toml_valido_vacio_y_solo_comentarios_dan_cero() {
        let repo = tempfile::tempdir().expect("temp dir");
        escribir(repo.path(), "a.toml", "x = 1\n[t]\nk = [1, 2]\n");
        escribir(repo.path(), "b.toml", "");
        escribir(repo.path(), "c.toml", "# solo comentario\n# otro\n");
        let r = sonda_estructura_configs(repo.path(), &[]);
        assert!(r.hallazgos.is_empty());
        assert_eq!(r.valor, Some(1.0));
    }

    #[test]
    fn configs_yaml_tabulacion_invalida_da_la_linea_exacta() {
        let repo = repo_con("c.yaml", "a:\n  b: 1\nc:\n\td: 2\n");
        let r = sonda_estructura_configs(repo.path(), &[]);
        assert_eq!(r.hallazgos.len(), 1, "un hallazgo: {:?}", r.hallazgos);
        assert_eq!(r.hallazgos[0].linea, 4);
        assert_eq!(r.hallazgos[0].cita, "d: 2");
    }

    #[test]
    fn configs_yaml_roto_sin_tabulacion_da_hallazgo() {
        let repo = repo_con("c.yml", "a: [1, 2\nb: 3\n");
        let r = sonda_estructura_configs(repo.path(), &[]);
        assert_eq!(r.hallazgos.len(), 1);
        assert_eq!(r.hallazgos[0].severidad, 2);
        assert!(r.hallazgos[0].linea >= 1);
    }

    #[test]
    fn configs_yaml_valido_vacio_y_comentarios_dan_cero() {
        let repo = tempfile::tempdir().expect("temp dir");
        escribir(repo.path(), "a.yaml", "a:\n  - 1\n  - two\n");
        escribir(repo.path(), "b.yaml", "");
        escribir(repo.path(), "c.yml", "# comentario\n");
        let r = sonda_estructura_configs(repo.path(), &[]);
        assert!(r.hallazgos.is_empty());
        assert_eq!(r.valor, Some(1.0));
    }

    #[test]
    fn configs_secretos_toml_password_literal_da_sev_3() {
        let repo = repo_con("a.toml", "[db]\nhost = \"x\"\npassword = \"hunter2\"\n");
        let r = sonda_secretos_configs(repo.path(), &[]);
        assert_eq!(r.hallazgos.len(), 1);
        assert_eq!(r.hallazgos[0].severidad, 3);
        assert_eq!(r.hallazgos[0].linea, 3);
        assert_eq!(r.hallazgos[0].cita, "password");
    }

    #[test]
    fn configs_secretos_yaml_api_key_literal_da_sev_3() {
        let repo = repo_con("a.yaml", "api_key: abc123\n");
        let r = sonda_secretos_configs(repo.path(), &[]);
        assert_eq!(r.hallazgos.len(), 1);
        assert_eq!(r.hallazgos[0].cita, "api_key");
    }

    #[test]
    fn configs_secretos_clave_con_forma_de_secreto_cuenta() {
        let repo = repo_con("a.toml", "auth_token = \"zz9\"\nDB_PASSWORD = 'pw'\n");
        let r = sonda_secretos_configs(repo.path(), &[]);
        assert_eq!(r.hallazgos.len(), 2);
    }

    #[test]
    fn configs_secretos_max_tokens_numerico_no_cuenta() {
        let repo = repo_con("a.toml", "max_tokens = 4096\nnum_tokens = 8\n");
        let r = sonda_secretos_configs(repo.path(), &[]);
        assert!(r.hallazgos.is_empty(), "sobran: {:?}", r.hallazgos);
    }

    #[test]
    fn configs_secretos_booleano_sin_comillas_no_cuenta() {
        let repo = repo_con("a.toml", "api_key_enabled = true\ntimeout = true\n");
        let r = sonda_secretos_configs(repo.path(), &[]);
        assert!(r.hallazgos.is_empty(), "sobran: {:?}", r.hallazgos);
    }

    #[test]
    fn configs_secretos_numerico_entre_comillas_si_cuenta() {
        let repo = repo_con("a.toml", "password = \"12345\"\n");
        let r = sonda_secretos_configs(repo.path(), &[]);
        assert_eq!(r.hallazgos.len(), 1);
    }

    #[test]
    fn configs_secretos_auth_token_literal_cuenta() {
        let repo = repo_con("a.toml", "auth_token = \"abc\"\n");
        let r = sonda_secretos_configs(repo.path(), &[]);
        assert_eq!(r.hallazgos.len(), 1);
    }

    #[test]
    fn configs_dockerfile_env_max_tokens_numerico_no_cuenta() {
        let texto = "FROM python:3.12-slim\nENV MAX_TOKENS=4096\nUSER app\n";
        let repo = repo_con("Dockerfile", texto);
        let r = sonda_dockerfile_reglas(repo.path(), &[]);
        assert!(r.hallazgos.is_empty(), "sobran: {:?}", r.hallazgos);
    }

    #[test]
    fn configs_secretos_ignora_vacios_referencias_placeholders_y_comentarios() {
        let texto = "db_password: ${DB_PASS}\n\
                     token: \"\"\n\
                     secret_a: ''\n\
                     token_b:\n\
                     api_key: $API\n\
                     api_key_c: \"{{ vault }}\"\n\
                     token_d: env:TOKEN\n\
                     token_e: <valor>\n\
                     token_f: changeme\n\
                     token_g: xxx-123\n\
                     token_h: REDACTED\n\
                     token_i: example\n\
                     # password: x\n";
        let repo = repo_con("c.yaml", texto);
        let r = sonda_secretos_configs(repo.path(), &[]);
        assert!(r.hallazgos.is_empty(), "sobran: {:?}", r.hallazgos);
    }

    #[test]
    fn configs_secretos_nunca_copia_el_valor_al_hallazgo() {
        let repo = repo_con("a.toml", "password = \"hunter2-valor-secreto\"\n");
        let r = sonda_secretos_configs(repo.path(), &[]);
        assert_eq!(r.hallazgos.len(), 1);
        assert!(!serializado(&r).contains("hunter2-valor-secreto"));
        assert!(!r.hallazgos[0].detalle.contains("hunter2"));
    }

    #[test]
    fn configs_dockerfile_python_latest_sin_user_da_dos_hallazgos() {
        let repo = repo_con("Dockerfile", "FROM python:latest\n");
        let r = sonda_dockerfile_reglas(repo.path(), &[]);
        assert_eq!(r.hallazgos.len(), 2, "{:?}", r.hallazgos);
        assert!(
            r.hallazgos
                .iter()
                .all(|h| h.cita == "FROM python:latest" && h.linea == 1)
        );
        assert_eq!(
            r.hallazgos.iter().map(|h| h.severidad).collect::<Vec<_>>(),
            [2, 3]
        );
    }

    #[test]
    fn configs_dockerfile_sin_tag_da_regla_a() {
        let repo = repo_con("Dockerfile", "FROM python\nUSER app\n");
        let r = sonda_dockerfile_reglas(repo.path(), &[]);
        assert_eq!(r.hallazgos.len(), 1);
        assert_eq!(r.hallazgos[0].severidad, 2);
        assert_eq!(r.hallazgos[0].cita, "FROM python");
    }

    #[test]
    fn configs_dockerfile_puerto_del_registro_no_es_tag() {
        let repo = repo_con("Dockerfile", "FROM localhost:5000/app\nUSER app\n");
        let r = sonda_dockerfile_reglas(repo.path(), &[]);
        assert_eq!(r.hallazgos.len(), 1, "sin tag: regla a");
    }

    #[test]
    fn configs_dockerfile_con_digest_no_dispara() {
        let texto = "FROM python:3.12-slim@sha256:0123456789abcdef\nUSER app\n";
        let repo = repo_con("Dockerfile", texto);
        assert!(
            sonda_dockerfile_reglas(repo.path(), &[])
                .hallazgos
                .is_empty()
        );
    }

    #[test]
    fn configs_dockerfile_scratch_no_dispara_regla_a() {
        let repo = repo_con("Dockerfile", "FROM scratch\nUSER app\n");
        assert!(
            sonda_dockerfile_reglas(repo.path(), &[])
                .hallazgos
                .is_empty()
        );
    }

    #[test]
    fn configs_dockerfile_multietapa_con_alias_y_user_da_cero() {
        let texto = "FROM rust:1 AS builder\nRUN cargo build\n\
                     FROM builder\nUSER app\n";
        let repo = repo_con("Dockerfile", texto);
        assert!(
            sonda_dockerfile_reglas(repo.path(), &[])
                .hallazgos
                .is_empty()
        );
    }

    #[test]
    fn configs_dockerfile_user_root_al_final_da_regla_b() {
        let repo = repo_con(
            "Dockerfile",
            "FROM debian:bookworm-slim\nUSER app\nUSER root\n",
        );
        let r = sonda_dockerfile_reglas(repo.path(), &[]);
        assert_eq!(r.hallazgos.len(), 1);
        assert_eq!(r.hallazgos[0].severidad, 3);
        assert_eq!(r.hallazgos[0].cita, "FROM debian:bookworm-slim");
    }

    #[test]
    fn configs_dockerfile_user_cero_al_final_da_regla_b() {
        let repo = repo_con("Dockerfile", "FROM debian:bookworm-slim\nUSER 0:0\n");
        assert_eq!(sonda_dockerfile_reglas(repo.path(), &[]).hallazgos.len(), 1);
    }

    #[test]
    fn configs_dockerfile_user_de_la_etapa_anterior_no_cuenta() {
        let texto = "FROM rust:1 AS builder\nUSER app\nFROM debian:bookworm-slim\n";
        let repo = repo_con("Dockerfile", texto);
        let r = sonda_dockerfile_reglas(repo.path(), &[]);
        assert_eq!(r.hallazgos.len(), 1, "la última etapa no tiene USER");
        assert_eq!(r.hallazgos[0].linea, 3);
    }

    #[test]
    fn configs_dockerfile_secreto_en_env_y_arg_da_dos_sin_el_valor() {
        let texto =
            "FROM python:3.12-slim\nENV API_TOKEN=s3cr3tValor\nARG PASSWORD=hunter2x\nUSER app\n";
        let repo = repo_con("Dockerfile", texto);
        let r = sonda_dockerfile_reglas(repo.path(), &[]);
        assert_eq!(r.hallazgos.len(), 2, "{:?}", r.hallazgos);
        assert_eq!(r.hallazgos[0].cita, "API_TOKEN");
        assert_eq!(r.hallazgos[0].linea, 2);
        assert_eq!(r.hallazgos[1].cita, "PASSWORD");
        let json = serializado(&r);
        assert!(!json.contains("s3cr3tValor"));
        assert!(!json.contains("hunter2x"));
    }

    #[test]
    fn configs_dockerfile_env_forma_legacy_con_secreto() {
        let repo = repo_con(
            "Dockerfile",
            "FROM python:3.12-slim\nENV API_TOKEN abc123\nUSER app\n",
        );
        let r = sonda_dockerfile_reglas(repo.path(), &[]);
        assert_eq!(r.hallazgos.len(), 1);
        assert_eq!(r.hallazgos[0].cita, "API_TOKEN");
    }

    #[test]
    fn configs_dockerfile_arg_sin_valor_da_cero() {
        let texto = "ARG PASSWORD\nFROM python:3.12-slim\nUSER app\n";
        let repo = repo_con("Dockerfile", texto);
        assert!(
            sonda_dockerfile_reglas(repo.path(), &[])
                .hallazgos
                .is_empty()
        );
    }

    #[test]
    fn configs_dockerfile_linea_continuada_con_secreto_cuenta_en_la_primera() {
        let texto = "FROM python:3.12-slim\nENV A=1 \\\n  API_TOKEN=abc123\nUSER app\n";
        let repo = repo_con("Dockerfile", texto);
        let r = sonda_dockerfile_reglas(repo.path(), &[]);
        assert_eq!(r.hallazgos.len(), 1, "{:?}", r.hallazgos);
        assert_eq!(r.hallazgos[0].cita, "API_TOKEN");
        assert_eq!(r.hallazgos[0].linea, 2);
    }

    #[test]
    fn configs_dockerfile_platform_y_as_se_entienden() {
        let texto = "FROM --platform=linux/amd64 python:3.12-slim AS base\nUSER app\nFROM base AS fin\nUSER app\n";
        let repo = repo_con("Dockerfile", texto);
        assert!(
            sonda_dockerfile_reglas(repo.path(), &[])
                .hallazgos
                .is_empty()
        );
    }

    #[test]
    fn configs_dockerfile_platform_con_latest_dispara_a() {
        let repo = repo_con(
            "Dockerfile",
            "FROM --platform=linux/amd64 python AS base\nUSER app\n",
        );
        let r = sonda_dockerfile_reglas(repo.path(), &[]);
        assert_eq!(r.hallazgos.len(), 1);
        assert_eq!(
            r.hallazgos[0].cita,
            "FROM --platform=linux/amd64 python AS base"
        );
    }

    const HADOLINT_JSON: &str = r#"[
        {"code":"DL3006","level":"error","line":1,"message":"tag","file":"/repo/Dockerfile"},
        {"code":"DL3008","level":"warning","line":2,"message":"pin","file":"/repo/Dockerfile"},
        {"code":"SC2086","level":"info","line":3,"message":"i","file":"/repo/Dockerfile"},
        {"code":"DL3002","level":"style","line":4,"message":"s","file":"/repo/Dockerfile"}
    ]"#;

    #[test]
    fn configs_parsear_hadolint_error_warning_y_se_ignora_info_style() {
        let hs = parsear_hadolint(HADOLINT_JSON, "Dockerfile", &[]).expect("parsea");
        assert_eq!(hs.len(), 2);
        assert_eq!((hs[0].severidad, hs[0].cita.as_str()), (2, "DL3006"));
        assert_eq!((hs[1].severidad, hs[1].cita.as_str()), (1, "DL3008"));
        assert_eq!(hs[0].archivo, "Dockerfile");
        assert_eq!(hs[1].linea, 2);
    }

    #[test]
    fn configs_parsear_hadolint_json_roto_es_error() {
        assert!(parsear_hadolint("no es json", "Dockerfile", &[]).is_err());
        assert!(parsear_hadolint("", "Dockerfile", &[]).is_err());
    }

    #[test]
    fn configs_yaml_libreria_solo_aparece_en_configs_rs() {
        fn recorrer(dir: &Path, salida: &mut Vec<PathBuf>) {
            for entrada in fs::read_dir(dir).expect("lee carpeta").flatten() {
                let ruta = entrada.path();
                if ruta.is_dir() {
                    recorrer(&ruta, salida);
                } else if ruta.extension().is_some_and(|e| e == "rs") {
                    salida.push(ruta);
                }
            }
        }
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut archivos = Vec::new();
        recorrer(&src, &mut archivos);
        assert!(archivos.len() > 10);
        // El patrón se arma en partes: el propio test no debe contar como uso del crate.
        let crate_yaml = ["yaml", "rust2"].join("_");
        let culpables: Vec<&PathBuf> = archivos
            .iter()
            .filter(|p| {
                fs::read_to_string(p)
                    .unwrap_or_default()
                    .contains(&crate_yaml)
            })
            .collect();
        assert_eq!(culpables.len(), 1, "solo configs.rs: {culpables:?}");
        assert!(culpables[0].ends_with("oracle/configs.rs"));
    }
}
