//! Inventario de fuentes de la fase de descarga (decisión 21). Lee los manifiestos del repo y
//! lista de dónde sale cada dependencia. Es puro salvo leer archivos: no usa la red ni ejecuta nada.
//! Las reglas que rechazan una fuente también viven aquí.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io;
use std::path::{Component, Path};

use serde::{Deserialize, Serialize};
use toml::{Table, Value};
use url::Host;

use crate::oracle::ErrorOraculo;
use crate::tipos::Dimension;

const ARCHIVO_CARGO: &str = "Cargo.toml";
const ARCHIVO_LOCK: &str = "Cargo.lock";
const ARCHIVO_REQUIREMENTS: &str = "requirements.txt";
/// Archivos de configuración de Cargo que pueden declarar registros y fuentes.
const ARCHIVOS_CONFIG: [&str; 2] = [".cargo/config.toml", ".cargo/config"];
const TABLAS_DEPENDENCIAS: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];
/// Fuentes de crates.io aceptadas en el `Cargo.lock`.
const ORIGENES_CRATES_IO: [&str; 2] = [
    "registry+https://github.com/rust-lang/crates.io-index",
    "sparse+https://index.crates.io/",
];
const INDICE_CRATES_IO: &str = "https://index.crates.io/";
const INDICE_PYPI: &str = "https://pypi.org/simple";
const HOST_CRATES_IO: &str = "index.crates.io";
const HOST_PYPI: &str = "pypi.org";

/// Tipo de una fuente declarada en el repo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoFuente {
    Registro,
    Git,
    Url,
}

/// Una fuente que Root bajaría. `direccion` nunca lleva credenciales.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fuente {
    pub tipo: TipoFuente,
    pub direccion: String,
    /// Versión, commit, rama o tag pedidos. Vacío si el registro no pide ninguno.
    pub version: Option<String>,
    /// `Cargo.toml`, `Cargo.lock` o `requirements.txt`, relativo al repo.
    pub archivo: String,
    pub linea: u32,
    /// Host real de la dirección, sin credenciales. Vacío si la fuente no es una URL.
    #[serde(default)]
    pub host: String,
}

/// Una fuente que no se acepta. La cita va redactada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rechazo {
    pub motivo: String,
    pub archivo: String,
    pub linea: u32,
    pub cita: String,
}

/// Un aviso que no corta la evaluación. La cita va redactada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Aviso {
    pub archivo: String,
    pub linea: u32,
    pub cita: String,
    pub detalle: String,
}

/// Un hallazgo que no corta la evaluación: de entrega, o de seguridad si es un secreto escrito en el repo.
/// La cita va redactada y nunca lleva el valor del secreto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HallazgoInventario {
    pub severidad: u8,
    pub archivo: String,
    pub linea: u32,
    pub cita: String,
    pub detalle: String,
    /// Entrega si no se indica otra cosa (los hallazgos viejos no la traen).
    #[serde(default = "dimension_entrega")]
    pub dimension: Dimension,
}

fn dimension_entrega() -> Dimension {
    Dimension::Entrega
}

/// Una entrada de `.cargo/config.toml` o `config`. `clave` es la ruta con puntos (`http.proxy`).
/// `red` marca lo que toca la red: el `Gate` del inventario la lista antes de usarla. `cita` no lleva secretos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntradaConfig {
    pub archivo: String,
    pub linea: u32,
    pub seccion: String,
    pub clave: String,
    pub red: bool,
    pub cita: String,
}

/// Lo que el repo declara. Se arma sin red.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inventario {
    pub fuentes: Vec<Fuente>,
    pub rechazos: Vec<Rechazo>,
    pub avisos: Vec<Aviso>,
    /// Dependencias git sin `rev` y sin commit en el lock: el build no es reproducible.
    pub sin_commit: Vec<Aviso>,
    /// Hallazgos de entrega sin corte: un `--extra-index-url` deja uno de severidad 1.
    pub hallazgos: Vec<HallazgoInventario>,
    /// Registros que una dependencia usa por nombre (`registry = "x"`). Sin repetir por nombre.
    #[serde(default)]
    pub registros_nombrados: Vec<RegistroNombrado>,
    /// Entradas de `.cargo/config.toml` y `config` del repo, todas. Las de `red = true` van al `Gate`.
    #[serde(default)]
    pub config_cargo: Vec<EntradaConfig>,
}

/// Un registro que el repo usa por nombre. `direccion` es la de su índice (`[registries.<nombre>]`),
/// sin credenciales. Sin `index` queda `registry:<nombre>`, que no tiene host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistroNombrado {
    pub nombre: String,
    pub direccion: String,
}

/// Dirección leída con el parser de URL (RFC 3986). Sin credenciales: el parser ya las rechazó.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UrlAnalizada {
    pub esquema: String,
    pub host: String,
    pub puerto: Option<u16>,
    pub ruta: String,
}

/// Motivo por el que `analizar_url` no acepta una dirección.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MotivoRechazoUrl {
    SinProtocolo,
    Protocolo(String),
    Credenciales,
    Invalida,
}

impl MotivoRechazoUrl {
    /// Texto que va al rechazo. Nunca incluye la dirección.
    pub fn texto(&self) -> String {
        match self {
            MotivoRechazoUrl::SinProtocolo => {
                "forma sin protocolo (scp o ruta): solo se acepta https".into()
            }
            MotivoRechazoUrl::Protocolo(esquema) => {
                format!("protocolo no aceptado: {esquema}; solo se acepta https")
            }
            MotivoRechazoUrl::Credenciales => "dirección con credenciales: se rechaza".into(),
            MotivoRechazoUrl::Invalida => "dirección inválida".into(),
        }
    }
}

/// Prefijos de herramienta que pip, Cargo y los registros ponen delante de una dirección.
const PREFIJOS_HERRAMIENTA: [&str; 5] = ["git+", "hg+", "svn+", "sparse+", "registry+"];

/// Analiza una dirección con el parser de URL. Sin red.
/// Orden: espacios, forma sin `://`, parseo, protocolo `https`, credenciales, host.
pub fn analizar_url(texto: &str) -> Result<UrlAnalizada, MotivoRechazoUrl> {
    let sin_prefijo = PREFIJOS_HERRAMIENTA
        .iter()
        .find_map(|p| texto.strip_prefix(p))
        .unwrap_or(texto);
    // RFC 3986: una dirección no lleva espacios ni caracteres de control.
    if sin_prefijo
        .chars()
        .any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(MotivoRechazoUrl::Invalida);
    }
    // La forma scp (`git@host:ruta`) y las rutas no llevan `://`: no son URL.
    if !sin_prefijo.contains("://") {
        return Err(MotivoRechazoUrl::SinProtocolo);
    }
    let url = url::Url::parse(sin_prefijo).map_err(|_| MotivoRechazoUrl::Invalida)?;
    let esquema = url.scheme().to_string();
    if esquema != "https" {
        return Err(MotivoRechazoUrl::Protocolo(esquema));
    }
    let con_credencial =
        !url.username().is_empty() || url.password().is_some_and(|clave| !clave.is_empty());
    if con_credencial {
        return Err(MotivoRechazoUrl::Credenciales);
    }
    let host = match url.host() {
        Some(Host::Domain(nombre)) => nombre.to_string(),
        Some(Host::Ipv4(direccion)) => direccion.to_string(),
        Some(Host::Ipv6(direccion)) => direccion.to_string(),
        None => return Err(MotivoRechazoUrl::Invalida),
    };
    Ok(UrlAnalizada {
        esquema,
        host,
        puerto: url.port(),
        ruta: url.path().to_string(),
    })
}

impl UrlAnalizada {
    /// `esquema://host[:puerto]ruta`. Sin credenciales, sin consulta y sin fragmento.
    fn base(&self) -> String {
        let host = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        let puerto = self.puerto.map(|p| format!(":{p}")).unwrap_or_default();
        format!("{}://{host}{puerto}{}", self.esquema, self.ruta)
    }
}

/// Lee los manifiestos y el lock del repo y arma el inventario. Sin red.
pub fn inventariar(repo: &Path) -> Result<Inventario, ErrorOraculo> {
    let mut inv = Inventario::default();
    inventariar_cargo(repo, &mut inv)?;
    inventariar_requirements(repo, &mut inv)?;
    inventariar_config_cargo(repo, &mut inv)?;
    Ok(inv)
}

/// Clasifica `.cargo/config.toml` y `config` del repo (decisión 30). No depende del manifiesto: un
/// secreto en el config es un secreto aunque falte `Cargo.toml`. Un TOML inválido es rechazo que cita
/// solo el archivo y la línea.
fn inventariar_config_cargo(repo: &Path, inv: &mut Inventario) -> Result<(), ErrorOraculo> {
    for ruta in ARCHIVOS_CONFIG {
        let Some(texto) = leer_opcional(&repo.join(ruta))? else {
            continue;
        };
        match toml::from_str::<Table>(&texto) {
            Ok(tabla) => {
                let config = Manifiesto {
                    dir: String::new(),
                    texto: Texto::nuevo(ruta, &texto),
                    tabla,
                };
                clasificar_config(&config, inv);
            }
            Err(e) if !inv.rechazos.iter().any(|r| r.archivo == ruta) => {
                let linea = linea_de_error(&texto, &e);
                inv.rechazos.push(Rechazo {
                    motivo: format!("{ruta} inválido en la línea {linea}"),
                    archivo: ruta.into(),
                    linea,
                    cita: String::new(),
                });
            }
            Err(_) => {}
        }
    }
    Ok(())
}

/// Oculta lo que va antes del último `@` de cada dirección del texto. Es conservadora: sirve
/// para citas aunque la URL no parsee. Las decisiones no usan esta función, usan `analizar_url`.
pub fn redactar(texto: &str) -> String {
    let mut salida = String::with_capacity(texto.len());
    let mut resto = texto;
    while let Some(pos) = resto.find("://") {
        let (antes, despues) = resto.split_at(pos + 3);
        salida.push_str(antes);
        let fin = despues
            .find(|c: char| c == '"' || c == '\'' || c.is_whitespace())
            .unwrap_or(despues.len());
        let (token, cola) = despues.split_at(fin);
        match token.rfind('@') {
            Some(i) => {
                salida.push_str("***");
                salida.push_str(&token[i..]);
            }
            None => salida.push_str(token),
        }
        resto = cola;
    }
    salida.push_str(resto);
    salida
}

/// Origen de un paquete del `Cargo.lock`.
#[derive(Debug, Clone, PartialEq)]
pub enum Origen {
    /// Miembro del workspace o path: no tiene `source`.
    Workspace,
    /// crates.io, por registro o por sparse.
    CratesIo,
    /// Repositorio git. `url` no lleva el commit ni la consulta.
    Git { url: String, commit: Option<String> },
    /// Otro registro. Guarda la dirección del índice.
    Registro { url: String },
    /// Fuente que no se reconoce. Guarda el valor del `source`.
    Otro(String),
}

/// Un `[[package]]` del `Cargo.lock`.
#[derive(Debug, Clone, PartialEq)]
pub struct PaqueteLock {
    pub nombre: String,
    pub version: String,
    pub origen: Origen,
    /// Línea 1-indexed del `source`. Cero si no tiene `source`.
    pub linea: u32,
    /// Texto de la línea del `source`, sin espacios de los extremos.
    pub cita: String,
}

#[derive(Deserialize)]
struct LockArchivo {
    #[serde(default)]
    package: Vec<LockPaquete>,
}

#[derive(Deserialize)]
struct LockPaquete {
    name: String,
    version: String,
    #[serde(default)]
    source: Option<String>,
}

/// Lee un `Cargo.lock` y da sus paquetes con origen y línea del `source`.
pub fn analizar_lock(texto: &str) -> Result<Vec<PaqueteLock>, String> {
    let lock: LockArchivo = toml::from_str(texto).map_err(|e| mensaje_toml(texto, &e))?;
    let fuentes = lineas_de_source(texto);
    if fuentes.len() != lock.package.len() {
        return Err(format!(
            "{} bloques [[package]] en el texto, {} al parsear",
            fuentes.len(),
            lock.package.len()
        ));
    }
    Ok(lock
        .package
        .into_iter()
        .zip(fuentes)
        .map(|(paquete, fuente)| {
            let (linea, cita) = fuente.unwrap_or((0, String::new()));
            PaqueteLock {
                origen: origen_de(paquete.source.as_deref()),
                nombre: paquete.name,
                version: paquete.version,
                linea,
                cita,
            }
        })
        .collect())
}

/// Una entrada por bloque `[[package]]`: línea y texto de su `source`, si lo tiene.
fn lineas_de_source(texto: &str) -> Vec<Option<(u32, String)>> {
    let mut bloques: Vec<Option<(u32, String)>> = Vec::new();
    for (indice, linea) in texto.lines().enumerate() {
        let limpia = linea.trim();
        if limpia == "[[package]]" {
            bloques.push(None);
        } else if limpia.starts_with("source = ")
            && let Some(ultimo) = bloques.last_mut()
            && ultimo.is_none()
        {
            let numero = u32::try_from(indice + 1).unwrap_or(u32::MAX);
            *ultimo = Some((numero, limpia.to_string()));
        }
    }
    bloques
}

/// Clasifica el `source` de un paquete del lock.
fn origen_de(valor: Option<&str>) -> Origen {
    let Some(valor) = valor else {
        return Origen::Workspace;
    };
    if ORIGENES_CRATES_IO.contains(&valor) {
        return Origen::CratesIo;
    }
    if let Some(git) = valor.strip_prefix("git+") {
        // El commit va después del último `#`. El resto se analiza tal cual, sin redactar.
        let (resto, commit) = match git.rsplit_once('#') {
            Some((resto, commit)) => (resto.to_string(), Some(commit.to_string())),
            None => (git.to_string(), None),
        };
        return Origen::Git {
            url: sin_consulta(&resto).to_string(),
            commit,
        };
    }
    if let Some(url) = valor
        .strip_prefix("registry+")
        .or_else(|| valor.strip_prefix("sparse+"))
    {
        return Origen::Registro {
            url: url.to_string(),
        };
    }
    Origen::Otro(valor.to_string())
}

/// Quita la consulta. La autoridad termina antes de `?`, así que el parser da la misma decisión.
fn sin_consulta(url: &str) -> &str {
    url.split('?').next().unwrap_or(url)
}

/// Motivo por el que una dirección no se acepta, o `None` si se acepta. Usa `analizar_url`.
fn motivo_de_url(url: &str) -> Option<String> {
    analizar_url(url).err().map(|m| m.texto())
}

/// Separa la referencia (`@rama` o `@commit`) de la ruta de una URL git.
pub fn separar_referencia(url: &str) -> (String, Option<String>) {
    if let Some((esquema, resto)) = url.split_once("://")
        && let Some(i) = resto.find('/')
    {
        let (autoridad, ruta) = resto.split_at(i);
        if let Some((ruta, referencia)) = ruta.rsplit_once('@') {
            return (
                format!("{esquema}://{autoridad}{ruta}"),
                Some(referencia.to_string()),
            );
        }
    }
    (url.to_string(), None)
}

/// `true` si `ruta`, resuelta desde `dir` (relativo al repo), queda dentro del repo.
fn dentro_del_repo(dir: &str, ruta: &str) -> bool {
    ruta_en_repo(dir, ruta).is_some()
}

/// Carpeta de `ruta` resuelta desde `dir` (relativa al repo), sin `..` que salga. `None` si sale o es absoluta.
fn ruta_en_repo(dir: &str, ruta: &str) -> Option<String> {
    if Path::new(ruta).is_absolute() {
        return None;
    }
    let mut pila: Vec<String> = Vec::new();
    for componente in Path::new(dir).join(ruta).components() {
        match componente {
            Component::Normal(nombre) => pila.push(nombre.to_string_lossy().into_owned()),
            Component::ParentDir => {
                pila.pop()?;
            }
            Component::CurDir => {}
            _ => return None,
        }
    }
    Some(pila.join("/"))
}

/// `true` si la carpeta `relativa` del repo existe y, con sus enlaces resueltos, queda dentro del repo.
/// Lo que no existe no se lee, así que cuenta como dentro.
fn dentro_fisico(repo: &Path, relativa: &str) -> bool {
    match (
        fs::canonicalize(repo),
        fs::canonicalize(repo.join(relativa)),
    ) {
        (Ok(raiz), Ok(real)) => real.starts_with(raiz),
        _ => true,
    }
}

/// Mensaje de un TOML que no parsea: la línea del error, sin el texto de esa línea (B4).
fn mensaje_toml(texto: &str, error: &toml::de::Error) -> String {
    match linea_de_error(texto, error) {
        0 => "TOML inválido".to_string(),
        linea => format!("TOML inválido en la línea {linea}"),
    }
}

/// Número de línea (1-indexed) donde está el error de TOML. Cero si el error no trae posición.
fn linea_de_error(texto: &str, error: &toml::de::Error) -> u32 {
    let Some(rango) = error.span() else {
        return 0;
    };
    let previo = texto.get(..rango.start).unwrap_or(texto);
    u32::try_from(previo.matches('\n').count() + 1).unwrap_or(u32::MAX)
}

/// Quita comillas y espacios de un encabezado o de una clave, para comparar.
fn norma(texto: &str) -> String {
    texto
        .chars()
        .filter(|c| !matches!(c, '\'' | '"' | ' '))
        .collect()
}

/// Línea de cada clave y cada encabezado de tabla de un TOML, por (sección, clave).
fn ubicaciones(texto: &str) -> HashMap<(String, String), u32> {
    let mut mapa = HashMap::new();
    let mut seccion = String::new();
    for (indice, linea) in texto.lines().enumerate() {
        let numero = u32::try_from(indice + 1).unwrap_or(u32::MAX);
        let limpia = linea.trim();
        if limpia.starts_with('[') {
            let encabezado = norma(limpia.trim_start_matches('[').trim_end_matches(']'));
            if let Some((padre, hoja)) = encabezado.rsplit_once('.') {
                mapa.entry((padre.to_string(), hoja.to_string()))
                    .or_insert(numero);
            }
            seccion = encabezado;
        } else if let Some((clave, _)) = limpia.split_once('=') {
            let clave = clave.trim().trim_matches('"');
            let nombre = clave.split('.').next().unwrap_or(clave);
            mapa.entry((seccion.clone(), norma(nombre)))
                .or_insert(numero);
        }
    }
    mapa
}

/// Texto de un archivo, con la línea de cada clave para citarla.
struct Texto {
    ruta: String,
    lineas: Vec<String>,
    ubicaciones: HashMap<(String, String), u32>,
}

impl Texto {
    fn nuevo(ruta: &str, texto: &str) -> Texto {
        Texto {
            ruta: ruta.into(),
            lineas: texto.lines().map(str::to_string).collect(),
            ubicaciones: ubicaciones(texto),
        }
    }

    /// Línea de la clave `clave` dentro de la sección `seccion`. Cero si no aparece.
    fn linea(&self, seccion: &str, clave: &str) -> u32 {
        self.ubicaciones
            .get(&(norma(seccion), norma(clave)))
            .copied()
            .unwrap_or(0)
    }

    /// Texto de la línea `linea`, redactado. Vacío si no hay línea.
    fn cita(&self, linea: u32) -> String {
        match linea
            .checked_sub(1)
            .and_then(|i| self.lineas.get(i as usize))
        {
            Some(l) => redactar(l.trim()),
            None => String::new(),
        }
    }
}

/// Un manifiesto de Cargo: su carpeta relativa al repo, su texto y su tabla.
struct Manifiesto {
    dir: String,
    texto: Texto,
    tabla: Table,
}

/// Una dependencia declarada: su sección, su nombre y sus valores.
struct Dep {
    seccion: String,
    nombre: String,
    valores: Table,
}

fn rechazo(texto: &Texto, linea: u32, motivo: String) -> Rechazo {
    Rechazo {
        motivo: redactar(&motivo),
        archivo: texto.ruta.clone(),
        linea,
        cita: texto.cita(linea),
    }
}

fn rechazo_de_lock(paquete: &PaqueteLock, motivo: String) -> Rechazo {
    Rechazo {
        motivo: redactar(&motivo),
        archivo: ARCHIVO_LOCK.into(),
        linea: paquete.linea,
        cita: redactar(&paquete.cita),
    }
}

fn aviso(texto: &Texto, linea: u32, detalle: String) -> Aviso {
    Aviso {
        archivo: texto.ruta.clone(),
        linea,
        cita: texto.cita(linea),
        detalle: redactar(&detalle),
    }
}

/// Agrega una fuente sin repetirla: dos fuentes iguales en tipo, dirección y versión son una.
fn push_unico(fuentes: &mut Vec<Fuente>, nueva: Fuente) {
    let repetida = fuentes.iter().any(|f| {
        f.tipo == nueva.tipo && f.direccion == nueva.direccion && f.version == nueva.version
    });
    if !repetida {
        fuentes.push(nueva);
    }
}

fn crates_io_fuente(archivo: &str, linea: u32) -> Fuente {
    Fuente {
        tipo: TipoFuente::Registro,
        direccion: INDICE_CRATES_IO.into(),
        host: HOST_CRATES_IO.into(),
        version: None,
        archivo: archivo.into(),
        linea,
    }
}

/// Lee un archivo si existe. Un archivo que no se puede leer es infraestructura.
fn leer_opcional(ruta: &Path) -> Result<Option<String>, ErrorOraculo> {
    match fs::read_to_string(ruta) {
        Ok(texto) => Ok(Some(texto)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(ErrorOraculo::Infra(format!("leer {}: {e}", ruta.display()))),
    }
}

/// Parsea un manifiesto. Un TOML inválido es rechazo: Cargo no instalaría el producto.
fn manifiesto(ruta: &str, dir: &str, texto: &str, inv: &mut Inventario) -> Option<Manifiesto> {
    match toml::from_str::<Table>(texto) {
        Ok(tabla) => Some(Manifiesto {
            dir: dir.into(),
            texto: Texto::nuevo(ruta, texto),
            tabla,
        }),
        Err(e) => {
            let linea = linea_de_error(texto, &e);
            inv.rechazos.push(Rechazo {
                motivo: format!("{ruta} inválido en la línea {linea}"),
                archivo: ruta.into(),
                linea,
                cita: String::new(),
            });
            None
        }
    }
}

/// Carpetas de los miembros del workspace. Solo `dir` y `dir/*`; `exclude` no se lee.
fn miembros(repo: &Path, raiz: &Table, inv: &mut Inventario) -> Vec<String> {
    let mut dirs: Vec<String> = Vec::new();
    let lista = raiz
        .get("workspace")
        .and_then(Value::as_table)
        .and_then(|w| w.get("members"))
        .and_then(Value::as_array);
    let Some(lista) = lista else {
        return dirs;
    };
    for patron in lista.iter().filter_map(Value::as_str) {
        let patron = patron.trim_start_matches("./").trim_end_matches('/');
        let base = if patron == "*" {
            Some("")
        } else {
            patron.strip_suffix("/*")
        };
        match base {
            Some(base) => {
                let Ok(entradas) = fs::read_dir(repo.join(base)) else {
                    continue;
                };
                let mut nombres: Vec<String> = entradas
                    .flatten()
                    .filter(|e| e.path().join(ARCHIVO_CARGO).is_file())
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect();
                nombres.sort();
                for nombre in nombres {
                    let dir = if base.is_empty() {
                        nombre
                    } else {
                        format!("{base}/{nombre}")
                    };
                    if !dirs.contains(&dir) {
                        dirs.push(dir);
                    }
                }
            }
            None if patron.contains('*') => inv.avisos.push(Aviso {
                archivo: ARCHIVO_CARGO.into(),
                linea: 0,
                cita: String::new(),
                detalle: redactar(&format!("glob no soportado en members: {patron}")),
            }),
            None if patron.is_empty() || patron == "." => {}
            None => {
                if !dirs.iter().any(|d| d == patron) {
                    dirs.push(patron.to_string());
                }
            }
        }
    }
    dirs
}

/// Dependencias de un manifiesto: las cuatro tablas, las de `target`, las de `workspace` y las de `patch`.
fn dependencias(tabla: &Table) -> Vec<Dep> {
    let mut salida = Vec::new();
    for tabla_dep in TABLAS_DEPENDENCIAS {
        agregar_dependencias(&mut salida, tabla.get(tabla_dep), tabla_dep);
    }
    if let Some(ws) = tabla.get("workspace").and_then(Value::as_table) {
        agregar_dependencias(
            &mut salida,
            ws.get("dependencies"),
            "workspace.dependencies",
        );
    }
    if let Some(destinos) = tabla.get("target").and_then(Value::as_table) {
        for (cfg, valor) in destinos {
            let Some(t) = valor.as_table() else {
                continue;
            };
            for tabla_dep in TABLAS_DEPENDENCIAS {
                let seccion = format!("target.{cfg}.{tabla_dep}");
                agregar_dependencias(&mut salida, t.get(tabla_dep), &seccion);
            }
        }
    }
    if let Some(parches) = tabla.get("patch").and_then(Value::as_table) {
        for (fuente, valor) in parches {
            agregar_dependencias(&mut salida, Some(valor), &format!("patch.{fuente}"));
        }
    }
    salida
}

/// Agrega las entradas de una tabla de dependencias. Un texto suelto es una versión de crates.io.
fn agregar_dependencias(salida: &mut Vec<Dep>, valor: Option<&Value>, seccion: &str) {
    let Some(tabla) = valor.and_then(Value::as_table) else {
        return;
    };
    for (nombre, dep) in tabla {
        let valores = match dep {
            Value::Table(t) => t.clone(),
            Value::String(version) => {
                let mut t = Table::new();
                t.insert("version".into(), Value::String(version.clone()));
                t
            }
            _ => continue,
        };
        salida.push(Dep {
            seccion: norma(seccion),
            nombre: nombre.clone(),
            valores,
        });
    }
}

/// Lee `.cargo/config.toml` y `.cargo/config` del repo: los dos que existan, en ese orden. Cargo los lee
/// en `/repo`. Un TOML inválido es rechazo.
fn leer_configs(repo: &Path, inv: &mut Inventario) -> Result<Vec<Manifiesto>, ErrorOraculo> {
    let mut salida = Vec::new();
    for ruta in ARCHIVOS_CONFIG {
        if let Some(texto) = leer_opcional(&repo.join(ruta))?
            && let Some(m) = manifiesto(ruta, "", &texto, inv)
        {
            salida.push(m);
        }
    }
    Ok(salida)
}

/// Tabla `[registries.<nombre>]` de la configuración del repo, en el primer archivo que la declara.
fn registro_de<'a>(configs: &'a [Manifiesto], nombre: &str) -> Option<&'a Table> {
    configs.iter().find_map(|c| {
        c.tabla
            .get("registries")?
            .as_table()?
            .get(nombre)?
            .as_table()
    })
}

/// Fuentes de reemplazo de `[source.*]`: su `registry` y su `git` pasan por las mismas reglas.
fn revisar_fuentes_config(config: &Manifiesto, inv: &mut Inventario) {
    let Some(fuentes) = config.tabla.get("source").and_then(Value::as_table) else {
        return;
    };
    for (nombre, valor) in fuentes {
        let Some(tabla) = valor.as_table() else {
            continue;
        };
        for clave in ["registry", "git"] {
            let Some(url) = tabla.get(clave).and_then(Value::as_str) else {
                continue;
            };
            if let Some(motivo) = motivo_de_url(url) {
                let linea = config.texto.linea(&format!("source.{nombre}"), clave);
                inv.rechazos.push(rechazo(&config.texto, linea, motivo));
            }
        }
    }
}

/// Clasifica las entradas de un config de Cargo (decisión 30). Una por tabla de `registries`, `source` y
/// `target`, y una por clave de `http`, `net`, `env`, `build` y `alias`. Las de la raíz van como `otra`.
/// Tocan la red `registries.*`, `source.*`, `http.proxy` y `net.git-fetch-with-cli`. Un secreto escrito en
/// el repo (un token de registro, o usuario y clave en el proxy) da un hallazgo de seguridad severidad 3.
fn clasificar_config(config: &Manifiesto, inv: &mut Inventario) {
    let texto = &config.texto;
    for (seccion, valor) in &config.tabla {
        match (seccion.as_str(), valor) {
            ("registries" | "source" | "target", Value::Table(tablas)) => {
                for (nombre, tabla) in tablas.iter().filter(|(_, t)| t.is_table()) {
                    inv.config_cargo.push(EntradaConfig {
                        archivo: texto.ruta.clone(),
                        linea: texto.linea(seccion, nombre),
                        seccion: seccion.clone(),
                        clave: format!("{seccion}.{nombre}"),
                        red: matches!(seccion.as_str(), "registries" | "source"),
                        cita: format!("[{seccion}.{nombre}]"),
                    });
                    let Some(t) = tabla.as_table() else {
                        continue;
                    };
                    if seccion == "registries" {
                        for clave in ["token", "credential-provider"] {
                            if t.contains_key(clave) {
                                inv.hallazgos.push(hallazgo_de_seguridad(
                                    &texto.ruta,
                                    texto.linea(&format!("registries.{nombre}"), clave),
                                    format!("{clave} = \"***\""),
                                    format!(
                                        "registro con {clave} escrito en el repo: registries.{nombre}"
                                    ),
                                ));
                            }
                        }
                    }
                }
            }
            ("http" | "net" | "env" | "build" | "alias", Value::Table(claves)) => {
                for (clave, valor_clave) in claves {
                    let ruta = format!("{seccion}.{clave}");
                    let red = matches!(ruta.as_str(), "http.proxy" | "net.git-fetch-with-cli");
                    let cita = cita_de_valor(clave, valor_clave);
                    if ruta == "http.proxy" && valor_clave.as_str().is_some_and(|u| u.contains('@'))
                    {
                        inv.hallazgos.push(hallazgo_de_seguridad(
                            &texto.ruta,
                            texto.linea(seccion, clave),
                            cita.clone(),
                            "proxy con credenciales escritas en el repo: http.proxy".into(),
                        ));
                    }
                    inv.config_cargo.push(EntradaConfig {
                        archivo: texto.ruta.clone(),
                        linea: texto.linea(seccion, clave),
                        seccion: seccion.clone(),
                        clave: ruta,
                        red,
                        cita,
                    });
                }
            }
            _ => inv.config_cargo.push(EntradaConfig {
                archivo: texto.ruta.clone(),
                linea: texto.linea("", seccion),
                seccion: "otra".into(),
                clave: seccion.clone(),
                red: false,
                cita: cita_de_valor(seccion, valor),
            }),
        }
    }
}

/// Cita de una clave del config sin su secreto: en un texto, lo que va antes del último `@` se oculta.
fn cita_de_valor(clave: &str, valor: &Value) -> String {
    match valor {
        Value::String(texto) => {
            let visible = match texto.rfind('@') {
                Some(i) => format!("***{}", &texto[i..]),
                None => texto.clone(),
            };
            format!("{clave} = \"{visible}\"")
        }
        Value::Table(_) => format!("[{clave}]"),
        Value::Array(_) => format!("{clave} = [...]"),
        otro => format!("{clave} = {otro}"),
    }
}

/// Hallazgo de seguridad severidad 3: un secreto escrito en el repo. Cita y detalle no llevan su valor.
fn hallazgo_de_seguridad(
    archivo: &str,
    linea: u32,
    cita: String,
    detalle: String,
) -> HallazgoInventario {
    HallazgoInventario {
        severidad: 3,
        archivo: archivo.into(),
        linea,
        cita: redactar(&cita),
        detalle: redactar(&detalle),
        dimension: Dimension::Seguridad,
    }
}

/// Inventario de los manifiestos de Cargo y del `Cargo.lock`.
fn inventariar_cargo(repo: &Path, inv: &mut Inventario) -> Result<(), ErrorOraculo> {
    let Some(texto_raiz) = leer_opcional(&repo.join(ARCHIVO_CARGO))? else {
        return Ok(());
    };
    let Some(raiz) = manifiesto(ARCHIVO_CARGO, "", &texto_raiz, inv) else {
        return Ok(());
    };
    let paquetes = match leer_opcional(&repo.join(ARCHIVO_LOCK))? {
        Some(texto) => match analizar_lock(&texto) {
            Ok(paquetes) => paquetes,
            Err(e) => {
                inv.rechazos.push(Rechazo {
                    motivo: redactar(&format!("Cargo.lock inválido: {e}")),
                    archivo: ARCHIVO_LOCK.into(),
                    linea: 0,
                    cita: String::new(),
                });
                Vec::new()
            }
        },
        None => Vec::new(),
    };
    // El lock fija el commit de cada fuente git: ese commit reemplaza a la rama o al tag.
    let commits: BTreeMap<String, String> = paquetes
        .iter()
        .filter_map(|p| match &p.origen {
            Origen::Git {
                url,
                commit: Some(c),
            } => analizar_url(url).ok().map(|u| (u.base(), c.clone())),
            _ => None,
        })
        .collect();

    let mut manifiestos = vec![raiz];
    for dir in miembros(repo, &manifiestos[0].tabla, inv) {
        let ruta = format!("{dir}/{ARCHIVO_CARGO}");
        if let Some(texto) = leer_opcional(&repo.join(&ruta))?
            && let Some(m) = manifiesto(&ruta, &dir, &texto, inv)
        {
            manifiestos.push(m);
        }
    }

    let configs = leer_configs(repo, inv)?;
    let mut crates_io: Option<(String, u32)> = None;
    // Las dependencias `path` dentro del repo se siguen: su `Cargo.toml` también declara fuentes (M5).
    let mut vistos: BTreeSet<String> = manifiestos.iter().map(|m| m.dir.clone()).collect();
    let mut i = 0;
    while i < manifiestos.len() {
        let mut nuevos = Vec::new();
        for dep in dependencias(&manifiestos[i].tabla) {
            revisar_dep(
                &dep,
                &manifiestos[i],
                &configs,
                &commits,
                inv,
                &mut crates_io,
            );
            if let Some(destino) = seguir_path(repo, &manifiestos[i], &dep, inv)
                && vistos.insert(destino.clone())
                && let Some(m) = leer_manifiesto_en(repo, &destino, inv)?
            {
                nuevos.push(m);
            }
        }
        manifiestos.extend(nuevos);
        i += 1;
    }
    for config in &configs {
        revisar_fuentes_config(config, inv);
    }

    let crates_lock = fuentes_de_lock(&paquetes, inv);
    // crates.io es la fuente por defecto: una sola entrada, la del lock si existe.
    match (crates_lock, crates_io) {
        (Some(linea), _) => push_unico(&mut inv.fuentes, crates_io_fuente(ARCHIVO_LOCK, linea)),
        (None, Some((archivo, linea))) => {
            push_unico(&mut inv.fuentes, crates_io_fuente(&archivo, linea))
        }
        (None, None) => {}
    }
    Ok(())
}

/// Fuentes de los paquetes de un lock, con las mismas reglas que las de los manifiestos (6c). Las aceptadas
/// van a `inv.fuentes` y las demás a `inv.rechazos`. Da la línea de la primera fuente de crates.io del lock.
fn fuentes_de_lock(paquetes: &[PaqueteLock], inv: &mut Inventario) -> Option<u32> {
    let mut crates_lock: Option<u32> = None;
    for p in paquetes {
        match &p.origen {
            Origen::Workspace => {}
            Origen::CratesIo => {
                crates_lock.get_or_insert(p.linea);
            }
            Origen::Git { url, commit } => match analizar_url(url) {
                Err(motivo) => inv.rechazos.push(rechazo_de_lock(p, motivo.texto())),
                Ok(u) => push_unico(
                    &mut inv.fuentes,
                    Fuente {
                        tipo: TipoFuente::Git,
                        direccion: u.base(),
                        host: u.host,
                        version: commit.clone(),
                        archivo: ARCHIVO_LOCK.into(),
                        linea: p.linea,
                    },
                ),
            },
            Origen::Registro { url } => match analizar_url(url) {
                Err(motivo) => inv.rechazos.push(rechazo_de_lock(p, motivo.texto())),
                Ok(u) => push_unico(
                    &mut inv.fuentes,
                    Fuente {
                        tipo: TipoFuente::Registro,
                        direccion: u.base(),
                        host: u.host,
                        version: None,
                        archivo: ARCHIVO_LOCK.into(),
                        linea: p.linea,
                    },
                ),
            },
            Origen::Otro(valor) => inv.rechazos.push(rechazo_de_lock(
                p,
                format!("fuente no reconocida en Cargo.lock: {valor}"),
            )),
        }
    }
    crates_lock
}

/// Revisa las fuentes de un `Cargo.lock` con las reglas del inventario. Sin red. Un lock que no parsea es error.
pub fn revisar_lock(texto: &str) -> Result<Vec<Rechazo>, String> {
    let paquetes = analizar_lock(texto)?;
    let mut inv = Inventario::default();
    fuentes_de_lock(&paquetes, &mut inv);
    Ok(inv.rechazos)
}

/// Carpeta de un `path` que se sigue: dentro del repo, con sus enlaces resueltos. Un `path` que sale del
/// repo da rechazo y no se sigue. `None` si la dependencia no tiene `path` o no se sigue.
fn seguir_path(repo: &Path, m: &Manifiesto, dep: &Dep, inv: &mut Inventario) -> Option<String> {
    let ruta = dep.valores.get("path")?.as_str()?;
    // Lo que sale del repo por la forma de la ruta lo rechaza `revisar_dep`.
    let destino = ruta_en_repo(&m.dir, ruta)?;
    if dentro_fisico(repo, &destino) {
        return Some(destino);
    }
    let linea = m.texto.linea(&dep.seccion, &dep.nombre);
    inv.rechazos.push(rechazo(
        &m.texto,
        linea,
        format!("path fuera del repo: {ruta}"),
    ));
    None
}

/// Manifiesto de la carpeta `dir` del repo, si tiene `Cargo.toml`. Un TOML inválido es rechazo.
fn leer_manifiesto_en(
    repo: &Path,
    dir: &str,
    inv: &mut Inventario,
) -> Result<Option<Manifiesto>, ErrorOraculo> {
    let ruta = format!("{dir}/{ARCHIVO_CARGO}");
    let Some(texto) = leer_opcional(&repo.join(&ruta))? else {
        return Ok(None);
    };
    Ok(manifiesto(&ruta, dir, &texto, inv))
}

/// Aplica las reglas a una dependencia de un manifiesto. Una fuente aceptada va a `inv.fuentes`.
fn revisar_dep(
    dep: &Dep,
    m: &Manifiesto,
    configs: &[Manifiesto],
    commits: &BTreeMap<String, String>,
    inv: &mut Inventario,
    crates_io: &mut Option<(String, u32)>,
) {
    let linea = m.texto.linea(&dep.seccion, &dep.nombre);
    let texto_de = |clave: &str| {
        dep.valores
            .get(clave)
            .and_then(Value::as_str)
            .map(str::to_string)
    };
    let git = texto_de("git");
    let path = texto_de("path");
    let registro = texto_de("registry");
    if let Some(ruta) = &path
        && !dentro_del_repo(&m.dir, ruta)
    {
        inv.rechazos.push(rechazo(
            &m.texto,
            linea,
            format!("path fuera del repo: {ruta}"),
        ));
        return;
    }
    if let Some(url) = &git {
        let u = match analizar_url(url) {
            Ok(u) => u,
            Err(motivo) => {
                inv.rechazos.push(rechazo(&m.texto, linea, motivo.texto()));
                return;
            }
        };
        let base = u.base();
        let commit_lock = commits.get(&base).cloned();
        let rev = texto_de("rev");
        let fijo = rev.is_some() || commit_lock.is_some();
        let version = commit_lock
            .or(rev)
            .or_else(|| texto_de("tag"))
            .or_else(|| texto_de("branch"));
        push_unico(
            &mut inv.fuentes,
            Fuente {
                tipo: TipoFuente::Git,
                direccion: base.clone(),
                host: u.host,
                version,
                archivo: m.texto.ruta.clone(),
                linea,
            },
        );
        if !fijo {
            inv.sin_commit.push(Aviso {
                archivo: m.texto.ruta.clone(),
                linea,
                cita: m.texto.cita(linea),
                detalle: redactar(&format!(
                    "dependencia git sin commit fijo: {} ({base}): el build no es reproducible",
                    dep.nombre
                )),
            });
        }
        return;
    }
    if let Some(nombre) = &registro {
        // Un token o un proveedor de credenciales escritos en el repo son un secreto: hallazgo de seguridad
        // (`clasificar_config`). La fuente se acepta, y Root pasa su credencial solo si el host coincide.
        let tabla = registro_de(configs, nombre);
        let (direccion, host) = match tabla.and_then(|t| t.get("index")).and_then(Value::as_str) {
            Some(url) => match analizar_url(url) {
                Ok(u) => (u.base(), u.host),
                Err(motivo) => {
                    inv.rechazos.push(rechazo(&m.texto, linea, motivo.texto()));
                    return;
                }
            },
            None => (format!("registry:{nombre}"), String::new()),
        };
        if !inv.registros_nombrados.iter().any(|r| r.nombre == *nombre) {
            inv.registros_nombrados.push(RegistroNombrado {
                nombre: nombre.clone(),
                direccion: direccion.clone(),
            });
        }
        push_unico(
            &mut inv.fuentes,
            Fuente {
                tipo: TipoFuente::Registro,
                direccion,
                host,
                version: None,
                archivo: m.texto.ruta.clone(),
                linea,
            },
        );
        return;
    }
    // Se cita la primera línea de crates.io del archivo, no la primera en orden alfabético.
    if path.is_none() && crates_io.as_ref().is_none_or(|(_, previa)| linea < *previa) {
        *crates_io = Some((m.texto.ruta.clone(), linea));
    }
}

/// Inventario de `requirements.txt`. Sin red: solo lee el texto.
fn inventariar_requirements(repo: &Path, inv: &mut Inventario) -> Result<(), ErrorOraculo> {
    let Some(texto) = leer_opcional(&repo.join(ARCHIVO_REQUIREMENTS))? else {
        return Ok(());
    };
    let t = Texto::nuevo(ARCHIVO_REQUIREMENTS, &texto);
    let logicas = lineas_logicas(&texto);
    // Pasada 1: cada `--index-url` va primero. Reemplaza a PyPI, así que PyPI no se lista.
    // Varios `--index-url`: se listan todos, por conservador. pip usa el último.
    let mut indice_propio = false;
    for (linea, logica) in &logicas {
        if let Some(valor) = opcion(contenido(logica), &["-i", "--index-url"]) {
            indice_propio = true;
            fuente_de_indice(&t, *linea, valor, inv);
        }
    }
    // Sin `--index-url`, pip usa PyPI como base. Va antes de los `--extra-index-url`.
    if !indice_propio {
        push_unico(
            &mut inv.fuentes,
            Fuente {
                tipo: TipoFuente::Registro,
                direccion: INDICE_PYPI.into(),
                host: HOST_PYPI.into(),
                version: None,
                archivo: ARCHIVO_REQUIREMENTS.into(),
                linea: 0,
            },
        );
    }
    // Pasada 2: el resto de las líneas, en el orden del archivo.
    for (linea, logica) in &logicas {
        let linea = *linea;
        let resto = contenido(logica);
        if resto.is_empty() || resto.starts_with('#') {
            continue;
        }
        if opcion(resto, &["-i", "--index-url"]).is_some() {
            continue;
        }
        if let Some(valor) = opcion(resto, &["--extra-index-url"]) {
            fuente_de_indice(&t, linea, valor, inv);
            inv.hallazgos.push(HallazgoInventario {
                severidad: 1,
                archivo: ARCHIVO_REQUIREMENTS.into(),
                linea,
                cita: t.cita(linea),
                detalle: "mezclar índices (--extra-index-url) permite la confusión de dependencias"
                    .into(),
                dimension: Dimension::Entrega,
            });
        } else if let Some(valor) = opcion(resto, &["-f", "--find-links"]) {
            fuente_de_indice(&t, linea, valor, inv);
        } else if opcion(resto, &["-r", "--requirement", "-c", "--constraint"]).is_some() {
            inv.avisos
                .push(aviso(&t, linea, format!("no se sigue: {resto}")));
        } else if let Some(valor) = opcion(resto, &["-e", "--editable"]) {
            if es_directa(valor) {
                directa(valor, linea, &t, inv);
            } else if es_ruta_local(repo, valor) {
                inv.rechazos.push(rechazo(
                    &t,
                    linea,
                    format!("ruta local no soportada: {valor}"),
                ));
            } else {
                inv.avisos.push(aviso(
                    &t,
                    linea,
                    format!("editable local, no se sigue: {valor}"),
                ));
            }
        } else if resto.starts_with('-') {
            inv.avisos
                .push(aviso(&t, linea, format!("opción no soportada: {resto}")));
        } else if let Some((_, url)) = resto.split_once(" @ ") {
            directa(url.trim(), linea, &t, inv);
        } else if es_directa(resto) {
            directa(resto, linea, &t, inv);
        } else if es_ruta_local(repo, resto) {
            // Una ruta local haría que pip compile el proyecto: el contenedor no la ve y Root la rechaza (C1).
            inv.rechazos.push(rechazo(
                &t,
                linea,
                format!("ruta local no soportada: {resto}"),
            ));
        }
        // Un nombre de paquete no agrega fuente: lo sirve el índice ya listado.
    }
    Ok(())
}

/// Contenido de una línea lógica: sin comentario, sin `--hash` y sin espacios de los extremos.
fn contenido(logica: &str) -> &str {
    let mut resto = logica.trim();
    if let Some(i) = resto.find(" --hash") {
        resto = resto[..i].trim();
    }
    if let Some(i) = resto.find(" #") {
        resto = resto[..i].trim();
    }
    resto
}

/// Une las líneas que terminan en `\` con la siguiente. Da la línea de inicio de cada una.
fn lineas_logicas(texto: &str) -> Vec<(u32, String)> {
    let mut salida = Vec::new();
    let mut acumulado: Option<(u32, String)> = None;
    for (indice, linea) in texto.lines().enumerate() {
        let numero = u32::try_from(indice + 1).unwrap_or(u32::MAX);
        let (inicio, mut contenido) = acumulado.take().unwrap_or((numero, String::new()));
        let limpia = linea.trim_end();
        if let Some(continuada) = limpia.strip_suffix('\\') {
            contenido.push_str(continuada);
            contenido.push(' ');
            acumulado = Some((inicio, contenido));
        } else {
            contenido.push_str(limpia);
            salida.push((inicio, contenido));
        }
    }
    if let Some(pendiente) = acumulado {
        salida.push(pendiente);
    }
    salida
}

/// Valor de una opción de pip (`-i x`, `--index-url=x`). `None` si la línea no es esa opción.
fn opcion<'a>(linea: &'a str, nombres: &[&str]) -> Option<&'a str> {
    nombres.iter().find_map(|nombre| {
        let resto = linea.strip_prefix(nombre)?;
        (resto.starts_with(' ') || resto.starts_with('='))
            .then(|| resto.trim_start_matches([' ', '=']).trim())
    })
}

/// `true` si el texto es una referencia directa: `git+...` o una URL con `://`.
fn es_directa(texto: &str) -> bool {
    texto.starts_with("git+") || texto.contains("://")
}

/// `true` si el texto de un requisito es una ruta local: absoluta, `./`, `../`, `file:`, un archivo de paquete
/// suelto (`.whl`, `.tar.gz`, `.zip`) o un nombre que es una carpeta del repo. Pip compila esas rutas (C1).
fn es_ruta_local(repo: &Path, texto: &str) -> bool {
    let texto = texto.trim();
    let minuscula = texto.to_ascii_lowercase();
    if minuscula.starts_with("file:")
        || texto.starts_with(['/', '~'])
        || texto.starts_with("./")
        || texto.starts_with("../")
        || texto == "."
        || texto == ".."
    {
        return true;
    }
    if texto.contains("://") {
        return false;
    }
    if texto.contains(['/', '\\']) {
        return true;
    }
    let nombre = texto
        .split(|c: char| c.is_whitespace() || "[;=<>!~@,".contains(c))
        .next()
        .unwrap_or("");
    if [".whl", ".tar.gz", ".zip"]
        .iter()
        .any(|f| nombre.ends_with(f))
    {
        return true;
    }
    !nombre.is_empty() && repo.join(nombre).is_dir()
}

/// Fuente de un índice (`--index-url`, `--extra-index-url`, `--find-links`).
fn fuente_de_indice(t: &Texto, linea: u32, url: &str, inv: &mut Inventario) {
    let u = match analizar_url(url) {
        Ok(u) => u,
        Err(motivo) => {
            inv.rechazos.push(rechazo(t, linea, motivo.texto()));
            return;
        }
    };
    push_unico(
        &mut inv.fuentes,
        Fuente {
            tipo: TipoFuente::Registro,
            direccion: u.base(),
            host: u.host,
            version: None,
            archivo: t.ruta.clone(),
            linea,
        },
    );
}

/// Referencia directa de un requisito: `git+https://...@commit` o una URL de archivo.
fn directa(url: &str, linea: u32, t: &Texto, inv: &mut Inventario) {
    let u = match analizar_url(url) {
        Ok(u) => u,
        Err(motivo) => {
            inv.rechazos.push(rechazo(t, linea, motivo.texto()));
            return;
        }
    };
    if url.starts_with("git+") {
        let (base, version) = separar_referencia(&u.base());
        push_unico(
            &mut inv.fuentes,
            Fuente {
                tipo: TipoFuente::Git,
                direccion: base,
                host: u.host,
                version,
                archivo: t.ruta.clone(),
                linea,
            },
        );
        return;
    }
    push_unico(
        &mut inv.fuentes,
        Fuente {
            tipo: TipoFuente::Url,
            direccion: u.base(),
            host: u.host,
            version: None,
            archivo: t.ruta.clone(),
            linea,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_con(archivos: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temp dir");
        for (ruta, texto) in archivos {
            let destino = dir.path().join(ruta);
            if let Some(padre) = destino.parent() {
                std::fs::create_dir_all(padre).expect("crea carpeta");
            }
            std::fs::write(destino, texto).expect("escribe");
        }
        dir
    }

    fn inventario_de(archivos: &[(&str, &str)]) -> Inventario {
        let dir = repo_con(archivos);
        inventariar(dir.path()).expect("inventaria sin red")
    }

    /// Manifiesto cuya primera dependencia queda en la línea 6.
    fn manifiesto(deps: &str) -> String {
        format!("[package]\nname = \"x\"\nversion = \"0.1.0\"\n\n[dependencies]\n{deps}")
    }

    fn inventario_cargo(deps: &str) -> Inventario {
        inventario_de(&[("Cargo.toml", &manifiesto(deps))])
    }

    #[test]
    fn descarga_inventario_git_con_rev_da_fuente_git_con_el_rev_como_version() {
        let inv =
            inventario_cargo("foo = { git = \"https://github.com/x/y\", rev = \"abc123\" }\n");
        assert!(inv.rechazos.is_empty(), "{:?}", inv.rechazos);
        assert_eq!(
            inv.fuentes,
            vec![Fuente {
                tipo: TipoFuente::Git,
                direccion: "https://github.com/x/y".into(),
                host: "github.com".into(),
                version: Some("abc123".into()),
                archivo: "Cargo.toml".into(),
                linea: 6,
            }]
        );
        assert!(inv.sin_commit.is_empty());
    }

    #[test]
    fn descarga_inventario_git_con_branch_sin_rev_marca_sin_commit_fijo() {
        let inv =
            inventario_cargo("foo = { git = \"https://github.com/x/y\", branch = \"main\" }\n");
        assert_eq!(inv.fuentes.len(), 1);
        assert_eq!(inv.fuentes[0].version.as_deref(), Some("main"));
        assert_eq!(inv.sin_commit.len(), 1, "{inv:?}");
        assert!(inv.sin_commit[0].detalle.contains("sin commit fijo"));
    }

    #[test]
    fn descarga_inventario_lock_aporta_el_commit_de_la_fuente_git() {
        let lock = "version = 4\n\n[[package]]\nname = \"app\"\nversion = \"0.1.0\"\n\n\
[[package]]\nname = \"y\"\nversion = \"0.1.0\"\n\
source = \"git+https://github.com/x/y?branch=main#0123456789abcdef0123456789abcdef01234567\"\n";
        let inv = inventario_de(&[
            (
                "Cargo.toml",
                &manifiesto("foo = { git = \"https://github.com/x/y\", branch = \"main\" }\n"),
            ),
            ("Cargo.lock", lock),
        ]);
        assert!(inv.rechazos.is_empty(), "{:?}", inv.rechazos);
        let git: Vec<&Fuente> = inv
            .fuentes
            .iter()
            .filter(|f| f.tipo == TipoFuente::Git)
            .collect();
        assert_eq!(
            git.len(),
            1,
            "la fuente del manifiesto y la del lock son una: {inv:?}"
        );
        assert_eq!(
            git[0].version.as_deref(),
            Some("0123456789abcdef0123456789abcdef01234567")
        );
        assert!(inv.sin_commit.is_empty(), "el lock fija el commit");
    }

    #[test]
    fn descarga_inventario_lock_solo_da_fuente_git_con_su_commit() {
        let lock = "version = 4\n\n[[package]]\nname = \"t\"\nversion = \"0.1.0\"\n\
source = \"git+https://github.com/x/t?rev=r1#aaaa\"\n";
        let inv = inventario_de(&[("Cargo.toml", &manifiesto("")), ("Cargo.lock", lock)]);
        let git = inv
            .fuentes
            .iter()
            .find(|f| f.tipo == TipoFuente::Git)
            .expect("git");
        assert_eq!(git.direccion, "https://github.com/x/t");
        assert_eq!(git.version.as_deref(), Some("aaaa"));
        assert_eq!(git.archivo, "Cargo.lock");
    }

    #[test]
    fn descarga_inventario_workspace_con_members_glob_encuentra_deps_de_los_miembros() {
        let inv = inventario_de(&[
            ("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n"),
            (
                "crates/a/Cargo.toml",
                &manifiesto("z = { git = \"https://github.com/x/z\", rev = \"r1\" }\n"),
            ),
        ]);
        assert!(inv.rechazos.is_empty(), "{:?}", inv.rechazos);
        assert_eq!(
            inv.fuentes,
            vec![Fuente {
                tipo: TipoFuente::Git,
                direccion: "https://github.com/x/z".into(),
                host: "github.com".into(),
                version: Some("r1".into()),
                archivo: "crates/a/Cargo.toml".into(),
                linea: 6,
            }]
        );
    }

    #[test]
    fn descarga_inventario_dependencias_por_version_dan_un_solo_registro_crates_io() {
        let inv = inventario_cargo("serde = \"1\"\nrand = { version = \"0.8\" }\n");
        assert_eq!(
            inv.fuentes,
            vec![Fuente {
                tipo: TipoFuente::Registro,
                direccion: "https://index.crates.io/".into(),
                host: "index.crates.io".into(),
                version: None,
                archivo: "Cargo.toml".into(),
                linea: 6,
            }]
        );
    }

    #[test]
    fn descarga_inventario_requirements_lista_indice_git_url_y_avisos() {
        let texto = "# comentario\nrequests==2.31.0\n--index-url https://pypi.example/simple\n\
git+https://github.com/x/y@abc123#egg=y\nhttps://host/p.tar.gz\n-r otro.txt\n";
        let inv = inventario_de(&[("requirements.txt", texto)]);
        assert!(inv.rechazos.is_empty(), "{:?}", inv.rechazos);
        assert_eq!(
            inv.fuentes,
            vec![
                Fuente {
                    tipo: TipoFuente::Registro,
                    direccion: "https://pypi.example/simple".into(),
                    host: "pypi.example".into(),
                    version: None,
                    archivo: "requirements.txt".into(),
                    linea: 3,
                },
                Fuente {
                    tipo: TipoFuente::Git,
                    direccion: "https://github.com/x/y".into(),
                    host: "github.com".into(),
                    version: Some("abc123".into()),
                    archivo: "requirements.txt".into(),
                    linea: 4,
                },
                Fuente {
                    tipo: TipoFuente::Url,
                    direccion: "https://host/p.tar.gz".into(),
                    host: "host".into(),
                    version: None,
                    archivo: "requirements.txt".into(),
                    linea: 5,
                },
            ]
        );
        assert_eq!(inv.avisos.len(), 1, "{:?}", inv.avisos);
        assert_eq!(inv.avisos[0].linea, 6);
        assert!(inv.avisos[0].detalle.contains("otro.txt"));
    }

    #[test]
    fn descarga_inventario_requirements_con_nombres_sin_indice_usa_pypi() {
        let inv = inventario_de(&[("requirements.txt", "requests==2.31.0\n")]);
        assert_eq!(inv.fuentes.len(), 1);
        assert_eq!(inv.fuentes[0].direccion, "https://pypi.org/simple");
        assert_eq!(inv.fuentes[0].tipo, TipoFuente::Registro);
    }

    #[test]
    fn descarga_inventario_protocolos_sin_https_son_rechazo() {
        for url in [
            "ssh://git@github.com/x/y",
            "git://github.com/x/y",
            "http://github.com/x/y",
            "file:///tmp/x",
            "git@github.com:x/y.git",
        ] {
            let inv = inventario_cargo(&format!("foo = {{ git = \"{url}\" }}\n"));
            assert_eq!(inv.rechazos.len(), 1, "debe rechazar {url}: {inv:?}");
            assert!(inv.fuentes.is_empty(), "no lista {url}: {inv:?}");
            assert_eq!(inv.rechazos[0].archivo, "Cargo.toml");
            assert_eq!(inv.rechazos[0].linea, 6);
        }
    }

    #[test]
    fn descarga_inventario_path_fuera_del_repo_es_rechazo() {
        let inv = inventario_cargo("afuera = { path = \"../afuera\" }\n");
        assert_eq!(inv.rechazos.len(), 1, "{inv:?}");
        assert!(inv.rechazos[0].motivo.contains("fuera del repo"));
    }

    #[test]
    fn descarga_inventario_path_dentro_del_repo_no_es_rechazo() {
        let inv = inventario_cargo("local = { path = \"crates/local\" }\n");
        assert!(inv.rechazos.is_empty(), "{:?}", inv.rechazos);
        assert!(inv.fuentes.is_empty(), "un path local no baja nada");
    }

    #[test]
    fn descarga_inventario_path_de_miembro_que_escapa_es_rechazo() {
        let inv = inventario_de(&[
            ("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n"),
            (
                "crates/a/Cargo.toml",
                &manifiesto("x = { path = \"../../../afuera\" }\n"),
            ),
        ]);
        assert_eq!(inv.rechazos.len(), 1, "{inv:?}");
    }

    #[test]
    fn descarga_inventario_path_dentro_del_repo_sigue_sus_dependencias() {
        // Sin `Cargo.lock` el lock se genera con las fuentes de `sub`: `sub` no es miembro del workspace.
        let inv = inventario_de(&[
            ("Cargo.toml", &manifiesto("sub = { path = \"sub\" }\n")),
            (
                "sub/Cargo.toml",
                &manifiesto("x = { git = \"http://host/x\" }\n"),
            ),
        ]);
        assert_eq!(inv.rechazos.len(), 1, "{inv:?}");
        assert_eq!(inv.rechazos[0].archivo, "sub/Cargo.toml");
        assert_eq!(inv.rechazos[0].linea, 6);
    }

    #[cfg(unix)]
    #[test]
    fn descarga_inventario_path_que_sale_por_enlace_es_rechazo() {
        let fuera = tempfile::tempdir().expect("fuera del repo");
        fs::write(
            fuera.path().join("Cargo.toml"),
            manifiesto("x = { git = \"https://github.com/x/y\" }\n"),
        )
        .expect("escribe");
        let repo = repo_con(&[("Cargo.toml", &manifiesto("sub = { path = \"sub\" }\n"))]);
        std::os::unix::fs::symlink(fuera.path(), repo.path().join("sub")).expect("enlace");
        let inv = inventariar(repo.path()).expect("inventaria sin red");
        assert_eq!(inv.rechazos.len(), 1, "{inv:?}");
        assert!(inv.rechazos[0].motivo.contains("fuera del repo"), "{inv:?}");
        assert!(
            inv.fuentes.is_empty(),
            "no se lee nada fuera del repo: {inv:?}"
        );
    }

    #[test]
    fn descarga_inventario_requirements_ruta_local_es_rechazo() {
        for linea in [
            "/repo",
            "/repo/sub",
            "./",
            "./x",
            "../afuera",
            "file:///tmp/x",
            "file:x",
            "-e ./x",
            "-e /repo",
            "pkg-1.0.whl",
            "pkg-1.0.tar.gz",
            "sub/pkg",
            "sub",
        ] {
            let texto = format!("requests==2.31.0\n{linea}\n");
            let inv = inventario_de(&[("requirements.txt", &texto), ("sub/x.txt", "")]);
            assert!(
                inv.rechazos
                    .iter()
                    .any(|r| r.archivo == "requirements.txt" && r.linea == 2),
                "debe rechazar `{linea}`: {inv:?}"
            );
        }
    }

    #[test]
    fn descarga_inventario_requirements_nombres_y_urls_no_son_rechazo() {
        for linea in [
            "requests==2.31.0",
            "pkg @ https://h/x.tar.gz",
            "https://h/x-1.0.whl",
            "git+https://github.com/x/y@abc1234",
        ] {
            let inv = inventario_de(&[("requirements.txt", &format!("{linea}\n"))]);
            assert!(
                inv.rechazos.is_empty(),
                "no debe rechazar `{linea}`: {inv:?}"
            );
        }
    }

    #[test]
    fn descarga_inventario_toml_roto_no_copia_el_token_ni_la_linea() {
        let secreto = "cio_SECRETO_9f3a";
        let config = format!(
            "[registries.privado]\nindex = \"https://x.example/index\"\ntoken = \"{secreto}\" basura\n"
        );
        // Sin `Cargo.toml`: la configuración rota también es rechazo, sin su contenido.
        let inv = inventario_de(&[(".cargo/config.toml", &config)]);
        let gate = serde_json::to_string(&inv).expect("json del inventario");
        assert!(!gate.contains(secreto), "{gate}");
        assert_eq!(inv.rechazos.len(), 1, "{inv:?}");
        assert_eq!(inv.rechazos[0].archivo, ".cargo/config.toml");
        assert_eq!(inv.rechazos[0].linea, 3);
        assert_eq!(inv.rechazos[0].cita, "");
        assert!(!inv.rechazos[0].motivo.contains(secreto));
    }

    #[test]
    fn descarga_inventario_toml_roto_con_cargo_toml_no_copia_el_token() {
        let secreto = "cio_SECRETO_9f3a";
        let config = format!("[registries.privado]\ntoken = \"{secreto}\" basura\n");
        let inv = inventario_de(&[
            ("Cargo.toml", &manifiesto("")),
            (".cargo/config.toml", &config),
        ]);
        let gate = serde_json::to_string(&inv).expect("json del inventario");
        assert!(!gate.contains(secreto), "{gate}");
        assert_eq!(inv.rechazos.len(), 1, "{inv:?}");
    }

    #[test]
    fn descarga_inventario_credencial_en_url_es_rechazo_y_no_se_filtra() {
        let inv = inventario_cargo("foo = { git = \"https://usuario:clave@host/x.git\" }\n");
        assert_eq!(inv.rechazos.len(), 1, "{inv:?}");
        let json = serde_json::to_string(&inv).expect("serializa");
        assert!(!json.contains("clave"), "{json}");
        assert!(!json.contains("usuario:"), "{json}");
        assert!(json.contains("https://***@host/x.git"), "{json}");
        assert!(!format!("{inv:?}").contains("clave"));
    }

    #[test]
    fn descarga_inventario_registro_privado_con_token_se_acepta_con_hallazgo_de_seguridad() {
        let inv = inventario_de(&[
            (
                "Cargo.toml",
                &manifiesto("secreto = { version = \"1\", registry = \"privado\" }\n"),
            ),
            (
                ".cargo/config.toml",
                "[registries.privado]\nindex = \"sparse+https://reg.example/index/\"\ntoken = \"TOKEN-SECRETO-123\"\n",
            ),
        ]);
        assert!(inv.rechazos.is_empty(), "la fuente se acepta: {inv:?}");
        assert_eq!(direcciones(&inv), ["https://reg.example/index/"], "{inv:?}");
        let seguridad: Vec<&HallazgoInventario> = inv
            .hallazgos
            .iter()
            .filter(|h| h.dimension == Dimension::Seguridad)
            .collect();
        assert_eq!(seguridad.len(), 1, "{inv:?}");
        assert_eq!(seguridad[0].severidad, 3);
        assert_eq!(seguridad[0].archivo, ".cargo/config.toml");
        assert_eq!(seguridad[0].linea, 3);
        let json = serde_json::to_string(&inv).expect("serializa");
        assert!(!json.contains("TOKEN-SECRETO-123"), "{json}");
        assert!(!format!("{inv:?}").contains("TOKEN-SECRETO-123"));
    }

    #[test]
    fn inventario_registro_con_credential_provider_es_hallazgo_de_seguridad() {
        let inv = inventario_de(&[(
            ".cargo/config.toml",
            "[registries.privado]\nindex = \"sparse+https://reg.example/index/\"\ncredential-provider = \"proveedor-secreto-7\"\n",
        )]);
        let seguridad: Vec<&HallazgoInventario> = inv
            .hallazgos
            .iter()
            .filter(|h| h.dimension == Dimension::Seguridad)
            .collect();
        assert_eq!(seguridad.len(), 1, "{inv:?}");
        assert_eq!(seguridad[0].severidad, 3);
        assert_eq!(seguridad[0].linea, 3);
        let json = serde_json::to_string(&inv).expect("serializa");
        assert!(!json.contains("proveedor-secreto-7"), "{json}");
    }

    #[test]
    fn descarga_inventario_registro_privado_sin_credenciales_se_lista() {
        let inv = inventario_de(&[
            (
                "Cargo.toml",
                &manifiesto("secreto = { version = \"1\", registry = \"publico\" }\n"),
            ),
            (
                ".cargo/config.toml",
                "[registries.publico]\nindex = \"sparse+https://reg.example/index/\"\n",
            ),
        ]);
        assert!(inv.rechazos.is_empty(), "{:?}", inv.rechazos);
        assert_eq!(inv.fuentes[0].direccion, "https://reg.example/index/");
        assert_eq!(inv.fuentes[0].tipo, TipoFuente::Registro);
    }

    #[test]
    fn descarga_inventario_index_url_con_credenciales_es_rechazo() {
        let inv = inventario_de(&[(
            "requirements.txt",
            "--index-url https://u:p@host/simple\nrequests==1\n",
        )]);
        assert_eq!(inv.rechazos.len(), 1, "{inv:?}");
        let json = serde_json::to_string(&inv).expect("serializa");
        assert!(!json.contains("u:p@"), "{json}");
        assert!(!json.contains(":p@"), "{json}");
    }

    #[test]
    fn descarga_inventario_dependencia_en_tabla_con_ssh_es_rechazo() {
        let texto = "[package]\nname = \"x\"\nversion = \"0.1.0\"\n\n[dependencies.foo]\ngit = \"ssh://git@github.com/x/y\"\n";
        let inv = inventario_de(&[("Cargo.toml", texto)]);
        assert_eq!(inv.rechazos.len(), 1, "{inv:?}");
        assert_eq!(
            inv.rechazos[0].linea, 5,
            "la línea del encabezado de la tabla"
        );
    }

    #[test]
    fn descarga_inventario_patch_con_ssh_es_rechazo() {
        let texto = "[package]\nname = \"x\"\nversion = \"0.1.0\"\n\n[patch.crates-io]\nfoo = { git = \"ssh://host/x\" }\n";
        let inv = inventario_de(&[("Cargo.toml", texto)]);
        assert_eq!(inv.rechazos.len(), 1, "{inv:?}");
    }

    #[test]
    fn descarga_inventario_lock_con_credenciales_es_rechazo() {
        let lock = "version = 4\n\n[[package]]\nname = \"t\"\nversion = \"0.1.0\"\n\
source = \"git+https://u:p@host/x?rev=1#aaaa\"\n";
        let inv = inventario_de(&[("Cargo.toml", &manifiesto("")), ("Cargo.lock", lock)]);
        assert_eq!(inv.rechazos.len(), 1, "{inv:?}");
        assert_eq!(inv.rechazos[0].archivo, "Cargo.lock");
        assert_eq!(inv.rechazos[0].linea, 6);
        let json = serde_json::to_string(&inv).expect("serializa");
        assert!(!json.contains("u:p@"), "{json}");
    }

    #[test]
    fn descarga_inventario_cargo_toml_invalido_es_rechazo() {
        let inv = inventario_de(&[("Cargo.toml", "esto no es toml = = =")]);
        assert_eq!(inv.rechazos.len(), 1, "{inv:?}");
        assert!(inv.rechazos[0].motivo.starts_with("Cargo.toml inválido"));
    }

    #[test]
    fn descarga_redactar_oculta_la_credencial_y_respeta_el_resto() {
        assert_eq!(redactar("https://u:p@h/x"), "https://***@h/x");
        assert_eq!(
            redactar("git = \"https://t@h/x\" y https://a/b"),
            "git = \"https://***@h/x\" y https://a/b"
        );
        assert_eq!(redactar("sin url aquí"), "sin url aquí");
    }

    fn direcciones(inv: &Inventario) -> Vec<&str> {
        inv.fuentes.iter().map(|f| f.direccion.as_str()).collect()
    }

    #[test]
    fn descarga_inventario_requirements_sin_indice_lista_pypi_como_unica_fuente() {
        let inv = inventario_de(&[("requirements.txt", "requests==2.31.0\n")]);
        assert_eq!(direcciones(&inv), ["https://pypi.org/simple"], "{inv:?}");
        assert_eq!(inv.fuentes[0].tipo, TipoFuente::Registro);
        assert!(inv.hallazgos.is_empty(), "{inv:?}");
    }

    #[test]
    fn descarga_inventario_index_url_reemplaza_a_pypi() {
        let texto = "--index-url https://pypi.ejemplo.org/simple\nrequests==2.31.0\n";
        let inv = inventario_de(&[("requirements.txt", texto)]);
        assert_eq!(
            direcciones(&inv),
            ["https://pypi.ejemplo.org/simple"],
            "{inv:?}"
        );
        assert!(!direcciones(&inv).iter().any(|d| d.contains("pypi.org")));
    }

    #[test]
    fn descarga_inventario_extra_index_url_suma_pypi_y_deja_hallazgo() {
        let texto = "requests==2.31.0\n--extra-index-url https://pypi.ejemplo.org/simple\n";
        let inv = inventario_de(&[("requirements.txt", texto)]);
        assert_eq!(
            direcciones(&inv),
            ["https://pypi.org/simple", "https://pypi.ejemplo.org/simple"],
            "{inv:?}"
        );
        assert_eq!(inv.hallazgos.len(), 1, "{inv:?}");
        let h = &inv.hallazgos[0];
        assert_eq!(h.severidad, 1);
        assert_eq!(h.archivo, "requirements.txt");
        assert_eq!(h.linea, 2);
        assert_eq!(
            h.detalle,
            "mezclar índices (--extra-index-url) permite la confusión de dependencias"
        );
        assert!(h.cita.contains("--extra-index-url"), "{}", h.cita);
    }

    #[test]
    fn descarga_inventario_dos_extra_index_url_dan_tres_fuentes_y_dos_hallazgos() {
        let texto = "requests\n--extra-index-url https://a.example/simple\n\
--extra-index-url https://b.example/simple\n";
        let inv = inventario_de(&[("requirements.txt", texto)]);
        assert_eq!(
            direcciones(&inv),
            [
                "https://pypi.org/simple",
                "https://a.example/simple",
                "https://b.example/simple"
            ],
            "{inv:?}"
        );
        let lineas: Vec<u32> = inv.hallazgos.iter().map(|h| h.linea).collect();
        assert_eq!(lineas, [2, 3], "{inv:?}");
    }

    #[test]
    fn descarga_inventario_index_url_mas_extra_lista_base_primero_sin_pypi() {
        let texto = "requests\n--extra-index-url https://extra.example/simple\n\
--index-url https://base.example/simple\n";
        let inv = inventario_de(&[("requirements.txt", texto)]);
        assert_eq!(
            direcciones(&inv),
            [
                "https://base.example/simple",
                "https://extra.example/simple"
            ],
            "{inv:?}"
        );
        assert_eq!(inv.hallazgos.len(), 1, "{inv:?}");
        assert_eq!(inv.hallazgos[0].linea, 2);
    }

    #[test]
    fn descarga_inventario_formas_de_opcion_son_equivalentes() {
        for linea in [
            "-i https://base.example/simple",
            "--index-url=https://base.example/simple",
            "--index-url https://base.example/simple",
            "   -i    https://base.example/simple   ",
        ] {
            let texto = format!("{linea}\nrequests\n");
            let inv = inventario_de(&[("requirements.txt", &texto)]);
            assert_eq!(
                direcciones(&inv),
                ["https://base.example/simple"],
                "{texto}"
            );
        }
        let texto = "--index-url=https://base.example/simple\n\
--extra-index-url=https://extra.example/simple\nrequests\n";
        let inv = inventario_de(&[("requirements.txt", texto)]);
        assert_eq!(
            direcciones(&inv),
            [
                "https://base.example/simple",
                "https://extra.example/simple"
            ],
            "{inv:?}"
        );
        assert_eq!(inv.hallazgos.len(), 1, "{inv:?}");
    }

    #[test]
    fn descarga_inventario_extra_index_url_http_es_rechazo_y_deja_hallazgo() {
        let texto = "--extra-index-url http://x/simple\nrequests\n";
        let inv = inventario_de(&[("requirements.txt", texto)]);
        assert_eq!(inv.rechazos.len(), 1, "{inv:?}");
        assert_eq!(inv.rechazos[0].linea, 1);
        assert_eq!(direcciones(&inv), ["https://pypi.org/simple"], "{inv:?}");
        assert_eq!(inv.hallazgos.len(), 1, "{inv:?}");
    }

    #[test]
    fn descarga_inventario_index_url_con_credencial_es_rechazo_sin_filtrar() {
        let texto = "--index-url https://u:clave@host/simple\nrequests\n";
        let inv = inventario_de(&[("requirements.txt", texto)]);
        assert_eq!(inv.rechazos.len(), 1, "{inv:?}");
        assert_eq!(inv.rechazos[0].cita, "--index-url https://***@host/simple");
        assert!(inv.fuentes.is_empty(), "{inv:?}");
        let json = serde_json::to_string(&inv).expect("serializa");
        assert!(!json.contains("clave"), "{json}");
        assert!(!json.contains("u:clave@"), "{json}");
    }

    #[test]
    fn descarga_inventario_extra_index_url_con_token_es_rechazo_sin_filtrar() {
        let texto = "requests\n--extra-index-url https://tok@host/simple\n";
        let inv = inventario_de(&[("requirements.txt", texto)]);
        assert_eq!(inv.rechazos.len(), 1, "{inv:?}");
        assert_eq!(direcciones(&inv), ["https://pypi.org/simple"], "{inv:?}");
        assert_eq!(inv.hallazgos.len(), 1, "{inv:?}");
        assert!(inv.hallazgos[0].cita.contains("***@host"), "{inv:?}");
        let json = serde_json::to_string(&inv).expect("serializa");
        assert!(!json.contains("tok@"), "{json}");
    }

    #[test]
    fn descarga_inventario_orden_de_fuentes_es_estable() {
        let texto = "--index-url https://base.example/simple\n\
--extra-index-url https://b.example/simple\n\
--extra-index-url https://a.example/simple\n\
requests\ngit+https://github.com/x/y@abc#egg=y\n";
        let dir = repo_con(&[("requirements.txt", texto)]);
        let primera = inventariar(dir.path()).expect("inventaria sin red");
        let segunda = inventariar(dir.path()).expect("inventaria sin red");
        assert_eq!(primera, segunda);
        assert_eq!(
            direcciones(&primera),
            [
                "https://base.example/simple",
                "https://b.example/simple",
                "https://a.example/simple",
                "https://github.com/x/y"
            ],
            "{primera:?}"
        );
        assert_eq!(primera.hallazgos.len(), 2, "{primera:?}");
    }

    #[test]
    fn descarga_inventario_git_con_contrasena_con_barra_es_rechazo_sin_filtrar() {
        let inv = inventario_cargo("foo = { git = \"https://u:cla/ve@host/x.git\" }\n");
        assert_eq!(inv.rechazos.len(), 1, "{inv:?}");
        assert!(inv.fuentes.is_empty(), "{inv:?}");
        let json = serde_json::to_string(&inv).expect("serializa");
        assert!(!json.contains("cla/ve"), "{json}");
        assert!(!json.contains("u:"), "{json}");
    }

    #[test]
    fn descarga_inventario_requirements_git_con_contrasena_con_barra_es_rechazo_sin_filtrar() {
        let inv = inventario_de(&[("requirements.txt", "git+https://u:cla/ve@host/x.git\n")]);
        assert_eq!(inv.rechazos.len(), 1, "{inv:?}");
        // Sin `--index-url` PyPI sigue listado; la fuente git no entra.
        assert_eq!(direcciones(&inv), ["https://pypi.org/simple"], "{inv:?}");
        let json = serde_json::to_string(&inv).expect("serializa");
        assert!(!json.contains("cla/ve"), "{json}");
        assert!(!json.contains("u:"), "{json}");
    }

    #[test]
    fn descarga_inventario_index_url_con_contrasena_con_interrogante_es_rechazo_sin_filtrar() {
        let inv = inventario_de(&[(
            "requirements.txt",
            "--index-url https://u:p?x#y@host/z\nrequests\n",
        )]);
        assert_eq!(inv.rechazos.len(), 1, "{inv:?}");
        assert!(inv.fuentes.is_empty(), "{inv:?}");
        let json = serde_json::to_string(&inv).expect("serializa");
        assert!(!json.contains("p?x"), "{json}");
        assert!(!json.contains("u:"), "{json}");
    }

    #[test]
    fn descarga_inventario_url_directa_con_contrasena_con_numeral_es_rechazo() {
        let inv = inventario_de(&[("requirements.txt", "https://u:p?x#y@host/z\n")]);
        assert_eq!(inv.rechazos.len(), 1, "{inv:?}");
        assert_eq!(direcciones(&inv), ["https://pypi.org/simple"], "{inv:?}");
        let json = serde_json::to_string(&inv).expect("serializa");
        assert!(!json.contains("p?x"), "{json}");
        assert!(!json.contains("u:"), "{json}");
    }

    #[test]
    fn descarga_inventario_host_con_arroba_en_la_consulta_no_es_credencial() {
        let inv = inventario_de(&[(
            "requirements.txt",
            "--index-url https://host/p?e=a@b.c\nrequests\n",
        )]);
        assert!(inv.rechazos.is_empty(), "{inv:?}");
        assert_eq!(direcciones(&inv), ["https://host/p"], "{inv:?}");
    }

    #[test]
    fn descarga_inventario_contrasena_con_porcentaje_arroba_es_rechazo_sin_filtrar() {
        let inv = inventario_de(&[(
            "requirements.txt",
            "--extra-index-url https://u:p%40ss@host/simple\nrequests\n",
        )]);
        assert_eq!(inv.rechazos.len(), 1, "{inv:?}");
        let json = serde_json::to_string(&inv).expect("serializa");
        assert!(!json.contains("p%40ss"), "{json}");
        assert!(!json.contains("u:"), "{json}");
    }

    #[test]
    fn descarga_inventario_redactar_corta_en_el_ultimo_arroba() {
        assert_eq!(
            redactar("https://u:cla/ve@host/x.git"),
            "https://***@host/x.git"
        );
    }

    #[test]
    fn descarga_inventario_analizar_url_token_en_la_ruta_da_host_tok() {
        let u = analizar_url("https://tok/en@host").expect("acepta");
        assert_eq!(u.host, "tok");
        assert_eq!(u.ruta, "/en@host");
        assert_eq!(u.esquema, "https");
        assert_eq!(u.puerto, None);
    }

    #[test]
    fn descarga_inventario_analizar_url_credenciales_son_rechazo() {
        for url in [
            "https://usuario:clave@host/x",
            "https://token@host",
            "https://u:p%40ss@host",
        ] {
            assert_eq!(
                analizar_url(url),
                Err(MotivoRechazoUrl::Credenciales),
                "{url}"
            );
        }
    }

    #[test]
    fn descarga_inventario_analizar_url_parseo_invalido_es_direccion_invalida() {
        for url in [
            "https://u:cla/ve@host/x.git",
            "https://host:99999/x",
            "https://ho st/x",
            "https://host/a b",
        ] {
            assert_eq!(analizar_url(url), Err(MotivoRechazoUrl::Invalida), "{url}");
        }
        assert_eq!(MotivoRechazoUrl::Invalida.texto(), "dirección inválida");
    }

    #[test]
    fn descarga_inventario_analizar_url_protocolos_sin_https_son_rechazo() {
        assert_eq!(
            analizar_url("ssh://git@github.com/x/y"),
            Err(MotivoRechazoUrl::Protocolo("ssh".into()))
        );
        assert_eq!(
            analizar_url("git://github.com/x/y"),
            Err(MotivoRechazoUrl::Protocolo("git".into()))
        );
        assert_eq!(
            analizar_url("http://github.com/x/y"),
            Err(MotivoRechazoUrl::Protocolo("http".into()))
        );
        assert_eq!(
            analizar_url("file:///tmp/x"),
            Err(MotivoRechazoUrl::Protocolo("file".into()))
        );
        assert_eq!(
            analizar_url("git@github.com:x/y.git"),
            Err(MotivoRechazoUrl::SinProtocolo)
        );
    }

    #[test]
    fn descarga_inventario_analizar_url_ipv6_da_host_sin_corchetes_y_puerto() {
        let u = analizar_url("https://[::1]:8443/x").expect("acepta");
        assert_eq!(u.host, "::1");
        assert_eq!(u.puerto, Some(8443));
    }

    #[test]
    fn descarga_inventario_analizar_url_minusculas_prefijo_y_fragmento() {
        let u = analizar_url("HTTPS://Host.COM/x").expect("acepta");
        assert_eq!(u.host, "host.com");
        assert_eq!(u.esquema, "https");
        let g = analizar_url("git+https://github.com/x/y@abc123#egg=y").expect("acepta");
        assert_eq!(g.host, "github.com");
        assert_eq!(g.ruta, "/x/y@abc123");
        let h = analizar_url("hg+https://host/x").expect("acepta");
        assert_eq!(h.esquema, "https");
    }

    #[test]
    fn descarga_inventario_analizar_url_arroba_en_la_consulta_no_es_credencial() {
        let u = analizar_url("https://host/p?e=a@b.c").expect("acepta");
        assert_eq!(u.host, "host");
        assert_eq!(u.ruta, "/p");
    }

    #[test]
    fn descarga_inventario_git_con_token_en_la_ruta_se_acepta_y_registra_el_host_real() {
        let inv = inventario_cargo("foo = { git = \"https://tok/en@host/x.git\", rev = \"r1\" }\n");
        assert!(inv.rechazos.is_empty(), "{inv:?}");
        assert_eq!(inv.fuentes.len(), 1, "{inv:?}");
        assert_eq!(inv.fuentes[0].host, "tok");
    }

    #[test]
    fn descarga_inventario_usuario_con_barra_en_la_clave_motivo_es_direccion_invalida() {
        let inv = inventario_cargo("foo = { git = \"https://u:cla/ve@host/x.git\" }\n");
        assert_eq!(inv.rechazos.len(), 1, "{inv:?}");
        assert_eq!(inv.rechazos[0].motivo, "dirección inválida");
        let json = serde_json::to_string(&inv).expect("serializa");
        assert!(!json.contains("cla/ve"), "{json}");
    }

    #[test]
    fn descarga_inventario_requirements_host_con_arroba_en_consulta_registra_host() {
        let inv = inventario_de(&[(
            "requirements.txt",
            "--index-url https://host/p?e=a@b.c\nrequests\n",
        )]);
        assert_eq!(inv.fuentes[0].host, "host", "{inv:?}");
    }

    #[test]
    fn descarga_inventario_pypi_por_defecto_registra_su_host() {
        let inv = inventario_de(&[("requirements.txt", "requests==2.31.0\n")]);
        assert_eq!(inv.fuentes[0].host, "pypi.org", "{inv:?}");
    }

    #[test]
    fn descarga_redactar_es_conservador_con_el_arroba_en_la_ruta() {
        assert_eq!(redactar("https://tok/en@host/x"), "https://***@host/x");
    }

    #[test]
    fn descarga_inventario_cita_no_muestra_credencial_si_la_url_no_parsea() {
        let inv = inventario_cargo("foo = { git = \"https://usuario:clave@ho st/x\" }\n");
        assert_eq!(inv.rechazos.len(), 1, "{inv:?}");
        let json = serde_json::to_string(&inv).expect("serializa");
        assert!(!json.contains("clave"), "{json}");
        assert!(!json.contains("usuario:"), "{json}");
    }

    fn config_de(texto: &str) -> Inventario {
        inventario_de(&[(".cargo/config.toml", texto)])
    }

    fn entrada<'a>(inv: &'a Inventario, clave: &str) -> &'a EntradaConfig {
        inv.config_cargo
            .iter()
            .find(|e| e.clave == clave)
            .unwrap_or_else(|| panic!("falta la entrada {clave}: {:?}", inv.config_cargo))
    }

    #[test]
    fn inventario_http_proxy_suma_entrada_de_red_y_no_da_hallazgo() {
        let inv = config_de("[http]\nproxy = \"http://127.0.0.1:9\"\n");
        let e = entrada(&inv, "http.proxy");
        assert!(e.red, "{e:?}");
        assert_eq!(e.seccion, "http");
        assert_eq!(e.archivo, ".cargo/config.toml");
        assert_eq!(e.linea, 2);
        assert!(inv.hallazgos.is_empty(), "{inv:?}");
    }

    #[test]
    fn inventario_proxy_con_usuario_y_clave_es_hallazgo_de_seguridad_sin_filtrar() {
        let inv = config_de("[http]\nproxy = \"http://u:clave@proxy:3128\"\n");
        let h = inv
            .hallazgos
            .iter()
            .find(|h| h.dimension == Dimension::Seguridad)
            .expect("hallazgo de seguridad");
        assert_eq!(h.severidad, 3);
        assert_eq!(h.archivo, ".cargo/config.toml");
        assert_eq!(h.linea, 2);
        assert!(entrada(&inv, "http.proxy").red);
        // El campo `clave` del JSON se llama igual que la contraseña: se busca el secreto con su contexto.
        assert!(
            !h.cita.contains("clave") && !h.detalle.contains("clave"),
            "{h:?}"
        );
        let json = serde_json::to_string(&inv).expect("serializa");
        for texto in [json, format!("{inv:?}")] {
            assert!(!texto.contains("u:clave"), "{texto}");
            assert!(!texto.contains("clave@"), "{texto}");
            assert!(!texto.contains("u:"), "{texto}");
        }
    }

    #[test]
    fn inventario_net_git_fetch_with_cli_es_entrada_de_red() {
        let inv = config_de("[net]\ngit-fetch-with-cli = true\n");
        let e = entrada(&inv, "net.git-fetch-with-cli");
        assert!(e.red, "{e:?}");
        assert_eq!(e.seccion, "net");
        assert_eq!(e.linea, 2);
    }

    #[test]
    fn inventario_alias_y_build_son_entradas_sin_red() {
        let inv = config_de("[alias]\nb = \"build\"\n\n[build]\nrustc-wrapper = \"x\"\n");
        assert!(!entrada(&inv, "alias.b").red, "{inv:?}");
        assert!(!entrada(&inv, "build.rustc-wrapper").red, "{inv:?}");
        assert!(inv.config_cargo.iter().all(|e| !e.red), "{inv:?}");
        assert!(inv.hallazgos.is_empty(), "{inv:?}");
    }

    #[test]
    fn inventario_source_y_registries_son_entradas_de_red_y_target_no() {
        let texto = "[registries.privado]\nindex = \"sparse+https://reg.example/index/\"\n\n\
[source.privado]\nregistry = \"sparse+https://reg.example/index/\"\n\n\
[target.x86_64-unknown-linux-gnu]\nrunner = \"x\"\n";
        let inv = config_de(texto);
        let registro = entrada(&inv, "registries.privado");
        assert!(registro.red);
        assert_eq!(registro.linea, 1);
        assert_eq!(registro.cita, "[registries.privado]");
        assert!(entrada(&inv, "source.privado").red);
        assert!(!entrada(&inv, "target.x86_64-unknown-linux-gnu").red);
    }

    #[test]
    fn inventario_lee_config_sin_extension_ademas_del_toml() {
        let inv = inventario_de(&[(".cargo/config", "[http]\nproxy = \"http://127.0.0.1:9\"\n")]);
        assert_eq!(entrada(&inv, "http.proxy").archivo, ".cargo/config");
    }
}

/// Lo que pip necesita del `requirements.txt`, leído sin red. Las fuentes ya pasaron las reglas del inventario.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RequisitosPip {
    /// Índice que reemplaza a PyPI (`--index-url`). Varios: pip usa el último.
    pub indice_propio: Option<String>,
    /// Índices que se suman a los demás (`--extra-index-url`), en orden.
    pub indices_extra: Vec<String>,
    /// Especificadores de paquete, sin opciones ni comentarios, en el orden del archivo.
    pub especificadores: Vec<String>,
    /// Referencias directas (`git+https://...` o URL de archivo): línea de inicio y texto.
    pub directas: Vec<(u32, String)>,
}

/// Lee un `requirements.txt` y separa índices, especificadores y referencias directas. Sin red.
pub fn requisitos_pip(texto: &str) -> RequisitosPip {
    let mut salida = RequisitosPip::default();
    for (linea, logica) in lineas_logicas(texto) {
        let resto = contenido(&logica);
        if resto.is_empty() || resto.starts_with('#') {
            continue;
        }
        if let Some(valor) = opcion(resto, &["-i", "--index-url"]) {
            salida.indice_propio = Some(valor.to_string());
        } else if let Some(valor) = opcion(resto, &["--extra-index-url"]) {
            salida.indices_extra.push(valor.to_string());
        } else if resto.starts_with('-') {
            // Otras opciones no llegan a `pip download`: el inventario ya dejó su aviso.
        } else if let Some((_, url)) = resto.split_once(" @ ") {
            salida.directas.push((linea, url.trim().to_string()));
        } else if es_directa(resto) {
            salida.directas.push((linea, resto.to_string()));
        } else {
            salida.especificadores.push(resto.to_string());
        }
    }
    salida
}
