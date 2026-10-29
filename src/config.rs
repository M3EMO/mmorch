//! Configuración de Root: `ROOT_HOME` y `root.toml`.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::tipos::{Dimension, Proveedor};

/// Carpeta de datos de Root. Usa `ROOT_HOME` o, si no existe, `~/.local/share/root`.
pub fn root_home() -> PathBuf {
    resolver_root_home(
        std::env::var_os("ROOT_HOME").map(PathBuf::from),
        std::env::var_os("HOME").map(PathBuf::from),
    )
}

/// Valida la `ROOT_HOME` que resolvió `root_home()`. `ejecutar` la llama antes de cualquier subcomando (B6).
pub fn validar_root_home_actual() -> Result<(), Error> {
    let ruta = root_home();
    let home = std::env::var_os("HOME").map(PathBuf::from);
    validar_root_home(&ruta, home.as_deref()).map_err(Error::Uso)
}

/// `ROOT_HOME` debe ser absoluta, distinta de `/` y de `$HOME`. Sin `home`, solo rige la regla de `/`.
fn validar_root_home(ruta: &Path, home: Option<&Path>) -> Result<(), String> {
    if !ruta.is_absolute() {
        return Err(format!(
            "ROOT_HOME debe ser una ruta absoluta: {}",
            ruta.display()
        ));
    }
    if ruta.parent().is_none() {
        return Err("ROOT_HOME no puede ser /".into());
    }
    if home == Some(ruta) {
        return Err("ROOT_HOME no puede ser $HOME".into());
    }
    Ok(())
}

/// Función interna y pura: recibe las variables ya leídas.
fn resolver_root_home(root_home: Option<PathBuf>, home: Option<PathBuf>) -> PathBuf {
    root_home
        .filter(|ruta| !ruta.as_os_str().is_empty())
        .unwrap_or_else(|| home.unwrap_or_default().join(".local/share/root"))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Config {
    pub modelos: Modelos,
    /// Precio por modelo, en USD por millón de tokens.
    pub precios: BTreeMap<String, Precio>,
    pub balance: Balance,
    pub ruteo: Ruteo,
    pub umbral_verde: f64,
    /// Cota inferior de Wilson (90%, unilateral) que un candidato debe alcanzar para entrar a la política.
    pub umbral_wilson: f64,
    pub proveedores: ConfigProveedores,
    pub colab: ConfigColab,
    /// Sección `[jueces]`: modelo de los jueces que no pasan por `route`.
    pub jueces: Jueces,
    /// Registros privados de paquetes (`[[registro]]`). Solo los escribe el humano.
    #[serde(default, rename = "registro")]
    pub registros: Vec<RegistroConfig>,
    /// Sección `[decision]`: reglas deterministas del informe (Tarea 13b).
    #[serde(default)]
    pub decision: ReglasDecision,
    /// Avisos de la carga, como un nombre de modelo viejo. `Config::cargar` los imprime en stderr.
    #[serde(skip)]
    #[schemars(skip)]
    pub avisos: Vec<String>,
}

/// Una entrada `[[registro]]`: un registro privado y la variable de entorno que tiene su credencial.
/// Root pasa la credencial solo a un repo cuyo registro tiene el mismo host que `url`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct RegistroConfig {
    /// Nombre del registro. Cargo lo usa en `CARGO_REGISTRIES_<NOMBRE>_TOKEN`.
    pub nombre: String,
    /// Dirección del índice, con `https` y host.
    pub url: String,
    /// Variable de entorno de Root que tiene la credencial. Su valor nunca se escribe en un archivo.
    pub credencial_env: String,
    /// Variable de entorno de Root con el usuario para pip. Sin ella, el usuario es `__token__`.
    #[serde(default)]
    pub usuario_env: Option<String>,
}

/// Sección `[decision]`: las reglas que deciden `Listo`, `NoListo` o `Bloqueado` (Tarea 13b). Todo valor
/// va de 0 a 1 y es finito; si no, la carga falla con código 2.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ReglasDecision {
    /// Completitud mínima para `Listo`.
    pub completitud_minima: f64,
    /// Mínimo por dimensión, con la clave de la dimensión (`seguridad`, `diseno`...). Solo rige
    /// sobre dimensiones calibradas. Vacío por defecto.
    pub minimo: BTreeMap<String, f64>,
}

impl Default for ReglasDecision {
    fn default() -> Self {
        ReglasDecision {
            completitud_minima: 0.9,
            minimo: BTreeMap::new(),
        }
    }
}

impl ReglasDecision {
    /// Mínimo que `[decision.minimo]` fija para `dimension`, si lo fija.
    pub fn minimo_de(&self, dimension: Dimension) -> Option<f64> {
        self.minimo.get(dimension.clave()).copied()
    }
}

/// Sección `[colab]`: tope de duración de una corrida en Colab.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ConfigColab {
    /// Minutos máximos de una corrida en Colab. Al llegar al tope, Root libera el runtime.
    pub minutos_max: f64,
}

/// Sección `[jueces]`: el modelo del juez de la configuración de Cargo (decisión 47). Vía API.
/// El proveedor sale del nombre: `claude-*` es Anthropic y `deepseek-*` es DeepSeek.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Jueces {
    pub config_modelo: String,
}

/// Sección `[proveedores]`: URLs de las APIs, espera base de reintentos y timeout por intento.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ConfigProveedores {
    pub deepseek_url: String,
    pub anthropic_url: String,
    /// Espera antes del primer reintento, en milisegundos. Cada reintento la duplica.
    pub espera_base_ms: u64,
    /// Tiempo máximo de cada intento HTTP, en segundos.
    pub timeout_s: u64,
    /// Tiempo máximo de cada intento de los roles Verifica y Juzga, en segundos. Un juez puede
    /// escribir hasta `MAX_TOKENS_JUEZ` tokens.
    pub timeout_juez_s: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Modelos {
    pub planifica: String,
    pub organiza: String,
    pub construye: String,
}

/// Precio de un modelo, en USD por millón de tokens. Sus dos claves son obligatorias. El tramo alto
/// (`umbral_entrada`, `usd_entrada_alto`, `usd_salida_alto`) es opcional, pero va completo o no va.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Precio {
    /// USD por millón de tokens de entrada.
    pub usd_entrada: f64,
    /// USD por millón de tokens de salida.
    pub usd_salida: f64,
    /// Tokens de entrada por encima de los que el pedido entero cuesta al precio alto.
    #[serde(default)]
    pub umbral_entrada: Option<u64>,
    /// USD por millón de tokens de entrada, en el tramo alto.
    #[serde(default)]
    pub usd_entrada_alto: Option<f64>,
    /// USD por millón de tokens de salida, en el tramo alto.
    #[serde(default)]
    pub usd_salida_alto: Option<f64>,
}

impl Precio {
    /// Precio de entrada y de salida que rige un pedido con `in_tokens` de entrada. Si la entrada
    /// pasa `umbral_entrada` y el tramo alto está completo, es el alto; si no, el normal.
    pub fn tarifa_para(&self, in_tokens: u64) -> (f64, f64) {
        match (
            self.umbral_entrada,
            self.usd_entrada_alto,
            self.usd_salida_alto,
        ) {
            (Some(umbral), Some(entrada), Some(salida)) if in_tokens > umbral => (entrada, salida),
            _ => (self.usd_entrada, self.usd_salida),
        }
    }
}

/// Sección `[balance]`: topes duros de gasto. Route los lee; nunca los sube.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Balance {
    /// Fracción del tope de cupo (en (0, 1]) a partir de la cual se prefiere la API.
    pub prefiere_api_sobre: f64,
    pub cupo: TopeBloque,
    pub api: TopeMes,
    pub nube: TopeMes,
}

/// Tope de cupo por bloque de horas, imitando la ventana del plan.
/// La tabla tiene defaults: si falta una clave, vale el default de esta sección.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct TopeBloque {
    /// USD equivalentes de lista por bloque.
    pub tope_usd: f64,
    pub bloque_horas: u32,
}

/// Tope mensual. Sin defaults por campo: `api` y `nube` tienen topes distintos,
/// así que un `[balance.nube]` sin `tope_usd` debe fallar en vez de tomar el de la API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TopeMes {
    /// USD por mes calendario.
    pub tope_usd: f64,
}

/// Sección `[ruteo]`: modelo por rol cuando la vía es API, y equivalente de API de cada alias de cupo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Ruteo {
    pub delega: ModeloApi,
    pub verifica: ModeloApi,
    pub juzga: ModeloApi,
    /// Clave: alias de cupo ("opus", "sonnet", "haiku"). Valor: su modelo en la API.
    pub api_equivalente: BTreeMap<String, ModeloApi>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ModeloApi {
    pub proveedor: Proveedor,
    pub modelo: String,
    /// Modelo que Route usa si el proveedor principal no está disponible. Solo en Verifica y Juzga.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub respaldo: Option<Box<ModeloApi>>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            modelos: Modelos::default(),
            precios: precios_por_defecto(),
            balance: Balance::default(),
            ruteo: Ruteo::default(),
            umbral_verde: 0.8,
            umbral_wilson: 0.6,
            proveedores: ConfigProveedores::default(),
            colab: ConfigColab::default(),
            jueces: Jueces::default(),
            registros: Vec::new(),
            decision: ReglasDecision::default(),
            avisos: Vec::new(),
        }
    }
}

impl Default for Jueces {
    fn default() -> Self {
        Jueces {
            config_modelo: "claude-sonnet-5-5".into(),
        }
    }
}

impl Default for ConfigColab {
    fn default() -> Self {
        ConfigColab { minutos_max: 120.0 }
    }
}

impl Default for ConfigProveedores {
    fn default() -> Self {
        ConfigProveedores {
            deepseek_url: "https://api.deepseek.com".into(),
            anthropic_url: "https://api.anthropic.com".into(),
            espera_base_ms: 500,
            timeout_s: 120,
            timeout_juez_s: 1200,
        }
    }
}

impl Default for Modelos {
    fn default() -> Self {
        Modelos {
            planifica: "opus".into(),
            organiza: "sonnet".into(),
            construye: "haiku".into(),
        }
    }
}

impl Default for Balance {
    fn default() -> Self {
        Balance {
            prefiere_api_sobre: 0.9,
            cupo: TopeBloque::default(),
            api: TopeMes { tope_usd: 100.0 },
            nube: TopeMes { tope_usd: 250.0 },
        }
    }
}

impl Default for TopeBloque {
    fn default() -> Self {
        TopeBloque {
            tope_usd: 35.0,
            bloque_horas: 5,
        }
    }
}

/// Precios por defecto. Un `[precios.<modelo>]` de `root.toml` reemplaza el de ese modelo; los de
/// los demás modelos se conservan. Cada precio cita su origen.
fn precios_por_defecto() -> BTreeMap<String, Precio> {
    let plano = |usd_entrada, usd_salida| Precio {
        usd_entrada,
        usd_salida,
        umbral_entrada: None,
        usd_entrada_alto: None,
        usd_salida_alto: None,
    };
    BTreeMap::from([
        // Origen: config.py del proyecto anterior (decisión 41).
        ("deepseek-flash".to_string(), plano(0.14, 0.28)),
        // Origen: config.py del proyecto anterior (decisión 41).
        ("deepseek-v4-pro".to_string(), plano(1.74, 3.48)),
        // Origen: referencia de la API de Anthropic, tabla cacheada el 2026-10-06 (decisión 44).
        (
            "claude-haiku-5-5".to_string(),
            Precio {
                usd_entrada: 0.10,
                usd_salida: 0.50,
                umbral_entrada: Some(100_000),
                usd_entrada_alto: Some(0.50),
                usd_salida_alto: Some(2.50),
            },
        ),
        // Origen: referencia de la API de Anthropic, tabla cacheada el 2026-10-06 (decisión 47).
        ("claude-sonnet-5-5".to_string(), plano(2.00, 10.00)),
    ])
}

/// Modelos de DeepSeek que ya no se listan. Un rol de `[ruteo]` que los nombra se lee como
/// `deepseek-flash`.
const MODELOS_VIEJOS_DEEPSEEK: &[&str] = &["deepseek-chat", "deepseek-reasoner"];
const MODELO_DEEPSEEK_VIGENTE: &str = "deepseek-flash";

/// Cambia a `deepseek-flash` cada rol de `[ruteo]` que nombra un modelo viejo de DeepSeek. Devuelve
/// un aviso por cambio, con el modelo viejo y el nuevo. El razonamiento lo pone `route` por rol.
pub fn traducir_modelos_viejos(ruteo: &mut Ruteo) -> Vec<String> {
    let roles = [
        ("delega", &mut ruteo.delega),
        ("verifica", &mut ruteo.verifica),
        ("juzga", &mut ruteo.juzga),
    ];
    let mut avisos = Vec::new();
    for (rol, api) in roles {
        traducir_modelo(rol, api, &mut avisos);
        if let Some(respaldo) = api.respaldo.as_deref_mut() {
            traducir_modelo(&format!("{rol}.respaldo"), respaldo, &mut avisos);
        }
    }
    avisos
}

/// Cambia el modelo de un `ModeloApi` si es un nombre viejo de DeepSeek, y anota el aviso.
fn traducir_modelo(rol: &str, api: &mut ModeloApi, avisos: &mut Vec<String>) {
    let viejo = api.proveedor == Proveedor::Deepseek
        && MODELOS_VIEJOS_DEEPSEEK.contains(&api.modelo.as_str());
    if viejo {
        avisos.push(format!(
            "[ruteo.{rol}] usa \"{}\", un modelo viejo de DeepSeek: Root usa \"{MODELO_DEEPSEEK_VIGENTE}\"",
            api.modelo
        ));
        api.modelo = MODELO_DEEPSEEK_VIGENTE.into();
    }
}

/// Respaldo de Verifica y Juzga cuando `root.toml` no escribe uno: Haiku (decisión 41).
fn respaldo_haiku() -> Box<ModeloApi> {
    Box::new(ModeloApi {
        proveedor: Proveedor::Anthropic,
        modelo: "claude-haiku-5-5".into(),
        respaldo: None,
    })
}

impl Default for Ruteo {
    fn default() -> Self {
        let equivalente = |proveedor, modelo: &str| ModeloApi {
            proveedor,
            modelo: modelo.into(),
            respaldo: None,
        };
        // Verifica y Juzga llevan Haiku como respaldo (decisión 41).
        let con_respaldo = |proveedor, modelo: &str| ModeloApi {
            respaldo: Some(respaldo_haiku()),
            ..equivalente(proveedor, modelo)
        };
        Ruteo {
            delega: equivalente(Proveedor::Deepseek, "deepseek-flash"),
            verifica: con_respaldo(Proveedor::Deepseek, "deepseek-flash"),
            juzga: con_respaldo(Proveedor::Deepseek, "deepseek-flash"),
            api_equivalente: BTreeMap::from([
                (
                    "opus".to_string(),
                    equivalente(Proveedor::Anthropic, "claude-opus-5-5"),
                ),
                (
                    "sonnet".to_string(),
                    equivalente(Proveedor::Anthropic, "claude-sonnet-5-5"),
                ),
                (
                    "haiku".to_string(),
                    equivalente(Proveedor::Anthropic, "claude-haiku-5-5"),
                ),
            ]),
        }
    }
}

/// Tablas de `[balance]` con tope propio. Si una existe, debe declarar `tope_usd`.
const TABLAS_CON_TOPE: &[&str] = &["cupo", "api", "nube"];

/// Rechaza una tabla de tope sin `tope_usd`. Un TOML roto sigue siendo error de parseo (código 3).
fn exigir_topes_declarados(texto: &str) -> Result<(), Error> {
    let crudo: toml::Table = toml::from_str(texto)?;
    let Some(balance) = crudo.get("balance").and_then(toml::Value::as_table) else {
        return Ok(());
    };
    for nombre in TABLAS_CON_TOPE {
        let Some(tabla) = balance.get(*nombre) else {
            continue;
        };
        let declara_tope = tabla.as_table().is_some_and(|t| t.contains_key("tope_usd"));
        if !declara_tope {
            return Err(Error::ConfigValor(format!(
                "[balance.{nombre}] exige tope_usd"
            )));
        }
    }
    Ok(())
}

/// Claves obligatorias de `[precios.<modelo>]`: el precio normal.
const CLAVES_PRECIO: &[&str] = &["usd_entrada", "usd_salida"];
/// Claves opcionales del tramo alto. Van juntas: si una está, están las tres.
const CLAVES_TRAMO: &[&str] = &["umbral_entrada", "usd_entrada_alto", "usd_salida_alto"];

/// Revisa la tabla cruda `[precios.*]` antes de deserializar. Cada modelo declara las claves
/// obligatorias, sin claves ajenas y con números. El tramo alto va completo o no va. Así un error
/// de nombre o de tramo sale con código 2.
fn exigir_precios_declarados(texto: &str) -> Result<(), Error> {
    let crudo: toml::Table = toml::from_str(texto)?;
    let Some(precios) = crudo.get("precios").and_then(toml::Value::as_table) else {
        return Ok(());
    };
    for (modelo, valor) in precios {
        let Some(tabla) = valor.as_table() else {
            return Err(Error::ConfigValor(format!(
                "[precios.{modelo}] debe ser una tabla"
            )));
        };
        for (clave, numero) in tabla {
            let conocida =
                CLAVES_PRECIO.contains(&clave.as_str()) || CLAVES_TRAMO.contains(&clave.as_str());
            if !conocida {
                return Err(Error::ConfigValor(format!(
                    "[precios.{modelo}] no reconoce la clave \"{clave}\""
                )));
            }
            if clave == "umbral_entrada" {
                if !numero.as_integer().is_some_and(|n| n >= 0) {
                    return Err(Error::ConfigValor(format!(
                        "[precios.{modelo}] umbral_entrada debe ser un entero no negativo"
                    )));
                }
            } else if !(numero.is_float() || numero.is_integer()) {
                return Err(Error::ConfigValor(format!(
                    "[precios.{modelo}] {clave} debe ser un número"
                )));
            }
        }
        for clave in CLAVES_PRECIO {
            if !tabla.contains_key(*clave) {
                return Err(Error::ConfigValor(format!(
                    "[precios.{modelo}] exige {clave}"
                )));
            }
        }
        if CLAVES_TRAMO.iter().any(|c| tabla.contains_key(*c)) {
            for clave in CLAVES_TRAMO {
                if !tabla.contains_key(*clave) {
                    return Err(Error::ConfigValor(format!(
                        "[precios.{modelo}] exige {clave}: el tramo alto va con umbral_entrada, usd_entrada_alto y usd_salida_alto"
                    )));
                }
            }
        }
    }
    Ok(())
}

/// Reglas de cada `[[registro]]`: nombre, `url` con `https` y host sin credenciales, y variable
/// de entorno en mayúsculas. Un nombre no puede repetirse. Los mensajes no repiten la `url`.
/// Rango de `[decision]`: la completitud mínima y cada mínimo van de 0 a 1 y son finitos. Una clave de
/// `[decision.minimo]` que no es dimensión también falla.
fn validar_decision(reglas: &ReglasDecision) -> Result<(), Error> {
    let en_rango = |valor: f64| valor.is_finite() && (0.0..=1.0).contains(&valor);
    if !en_rango(reglas.completitud_minima) {
        return Err(Error::ConfigValor(format!(
            "decision.completitud_minima debe estar en [0, 1]: {}",
            reglas.completitud_minima
        )));
    }
    for (clave, valor) in &reglas.minimo {
        if Dimension::desde_clave(clave).is_none() {
            return Err(Error::ConfigValor(format!(
                "[decision.minimo] {clave} no es una dimensión"
            )));
        }
        if !en_rango(*valor) {
            return Err(Error::ConfigValor(format!(
                "[decision.minimo] {clave} debe estar en [0, 1]: {valor}"
            )));
        }
    }
    Ok(())
}

fn validar_registros(registros: &[RegistroConfig]) -> Result<(), Error> {
    let mut nombres: Vec<&str> = Vec::new();
    for registro in registros {
        let nombre = registro.nombre.as_str();
        let nombre_valido = !nombre.is_empty()
            && nombre
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
        if !nombre_valido {
            return Err(Error::ConfigValor(format!(
                "registro.nombre \"{nombre}\" debe tener solo letras, números, - o _"
            )));
        }
        if !url_de_registro_valida(&registro.url) {
            return Err(Error::ConfigValor(format!(
                "registro \"{nombre}\": url debe ser https, con host y sin usuario ni contraseña"
            )));
        }
        let env = registro.credencial_env.as_str();
        let env_valida = !env.is_empty()
            && env
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
        if !env_valida {
            return Err(Error::ConfigValor(format!(
                "registro \"{nombre}\": credencial_env debe ser un nombre de variable con A-Z, 0-9 o _"
            )));
        }
        if let Some(usuario) = registro.usuario_env.as_deref() {
            let usuario_valida = !usuario.is_empty()
                && usuario
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
            if !usuario_valida {
                return Err(Error::ConfigValor(format!(
                    "registro \"{nombre}\": usuario_env debe ser un nombre de variable con A-Z, 0-9 o _"
                )));
            }
        }
        if nombres.contains(&nombre) {
            return Err(Error::ConfigValor(format!(
                "registro \"{nombre}\" está repetido"
            )));
        }
        nombres.push(nombre);
    }
    Ok(())
}

/// `true` si la URL parsea, usa `https`, tiene host y no lleva usuario ni contraseña.
fn url_de_registro_valida(texto: &str) -> bool {
    let Ok(url) = url::Url::parse(texto) else {
        return false;
    };
    url.scheme() == "https"
        && url.host_str().is_some_and(|h| !h.is_empty())
        && url.username().is_empty()
        && url.password().is_none()
}

impl Config {
    /// Parsea el texto de un `root.toml`. Un texto vacío da los defaults.
    pub fn desde_texto(texto: &str) -> Result<Config, Error> {
        exigir_topes_declarados(texto)?;
        exigir_precios_declarados(texto)?;
        let mut config: Config = toml::from_str(texto)?;
        // Las entradas del usuario suman a los equivalentes de los defaults; no los reemplazan.
        for (alias, modelo) in Ruteo::default().api_equivalente {
            config.ruteo.api_equivalente.entry(alias).or_insert(modelo);
        }
        // Verifica y Juzga siempre tienen respaldo: si el usuario no escribió uno, vale Haiku.
        for api in [&mut config.ruteo.verifica, &mut config.ruteo.juzga] {
            api.respaldo.get_or_insert_with(respaldo_haiku);
        }
        // Un precio del usuario reemplaza el default de ese modelo; los demás defaults se conservan.
        for (modelo, precio) in precios_por_defecto() {
            config.precios.entry(modelo).or_insert(precio);
        }
        config.avisos = traducir_modelos_viejos(&mut config.ruteo);
        config.validar()?;
        Ok(config)
    }

    /// Rechaza topes en cero o negativos, `bloque_horas` en cero, `prefiere_api_sobre` fuera de (0, 1],
    /// un precio que no sea finito o sea negativo, y un alias de cupo en `[modelos]` sin
    /// `api_equivalente` en `[ruteo]`.
    fn validar(&self) -> Result<(), Error> {
        for alias in [
            &self.modelos.planifica,
            &self.modelos.organiza,
            &self.modelos.construye,
        ] {
            if !self.ruteo.api_equivalente.contains_key(alias) {
                return Err(Error::ConfigValor(format!(
                    "el alias \"{alias}\" de [modelos] no tiene api_equivalente en [ruteo]"
                )));
            }
        }
        let b = &self.balance;
        let topes = [
            ("balance.cupo.tope_usd", b.cupo.tope_usd),
            ("balance.api.tope_usd", b.api.tope_usd),
            ("balance.nube.tope_usd", b.nube.tope_usd),
        ];
        for (campo, valor) in topes {
            if valor.is_nan() || valor <= 0.0 {
                return Err(Error::ConfigValor(format!("{campo} debe ser mayor que 0")));
            }
        }
        let proveedores = &self.proveedores;
        for (campo, valor) in [
            ("proveedores.timeout_s", proveedores.timeout_s),
            ("proveedores.timeout_juez_s", proveedores.timeout_juez_s),
        ] {
            if valor == 0 {
                return Err(Error::ConfigValor(format!("{campo} debe ser mayor que 0")));
            }
        }
        // El respaldo solo vive en Verifica y Juzga, y no tiene respaldo propio.
        if self.ruteo.delega.respaldo.is_some() {
            return Err(Error::ConfigValor(
                "[ruteo.delega] no admite respaldo: solo verifica y juzga lo tienen".into(),
            ));
        }
        for (alias, equivalente) in &self.ruteo.api_equivalente {
            if equivalente.respaldo.is_some() {
                return Err(Error::ConfigValor(format!(
                    "[ruteo.api_equivalente.{alias}] no admite respaldo: solo verifica y juzga lo tienen"
                )));
            }
        }
        for api in [&self.ruteo.verifica, &self.ruteo.juzga] {
            if api
                .respaldo
                .as_deref()
                .is_some_and(|r| r.respaldo.is_some())
            {
                return Err(Error::ConfigValor(
                    "el respaldo no lleva otro respaldo".into(),
                ));
            }
        }
        if b.cupo.bloque_horas == 0 {
            return Err(Error::ConfigValor(
                "balance.cupo.bloque_horas debe ser al menos 1".into(),
            ));
        }
        if !(b.prefiere_api_sobre > 0.0 && b.prefiere_api_sobre <= 1.0) {
            return Err(Error::ConfigValor(
                "balance.prefiere_api_sobre debe estar en (0, 1]".into(),
            ));
        }
        let minutos = self.colab.minutos_max;
        if !(minutos.is_finite() && minutos > 0.0) {
            return Err(Error::ConfigValor(
                "colab.minutos_max debe ser mayor que 0".into(),
            ));
        }
        // Un valor finito enorme no entra en un `Duration`: `from_secs_f64` entraría en pánico.
        if std::time::Duration::try_from_secs_f64(minutos * 60.0).is_err() {
            return Err(Error::ConfigValor(
                "colab.minutos_max es demasiado grande".into(),
            ));
        }
        for (modelo, precio) in &self.precios {
            let campos = [
                ("usd_entrada", Some(precio.usd_entrada)),
                ("usd_salida", Some(precio.usd_salida)),
                ("usd_entrada_alto", precio.usd_entrada_alto),
                ("usd_salida_alto", precio.usd_salida_alto),
            ];
            for (campo, valor) in campos {
                let Some(valor) = valor else { continue };
                if !valor.is_finite() || valor < 0.0 {
                    return Err(Error::ConfigValor(format!(
                        "[precios.{modelo}] {campo} debe ser finito y no negativo"
                    )));
                }
            }
        }
        validar_decision(&self.decision)?;
        validar_registros(&self.registros)
    }

    /// Lee `root.toml`. Si el archivo no existe, devuelve el default.
    pub fn cargar(ruta: &Path) -> Result<Config, Error> {
        match std::fs::read_to_string(ruta) {
            Ok(texto) => {
                let config = Config::desde_texto(&texto)?;
                for aviso in &config.avisos {
                    eprintln!("aviso: {aviso}");
                }
                Ok(config)
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Config::default()),
            Err(fuente) => Err(Error::ConfigLeer {
                ruta: ruta.display().to_string(),
                fuente,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_root_toml_vacio_parsea_con_defaults() {
        let config = Config::desde_texto("").expect("vacío debe parsear");
        assert_eq!(config, Config::default());
        assert_eq!(config.modelos.construye, "haiku");
    }

    #[test]
    fn config_root_toml_con_valores_los_respeta() {
        let texto = r#"
umbral_verde = 0.75
umbral_wilson = 0.6

[modelos]
planifica = "opus"
organiza = "sonnet"
construye = "sonnet"

[precios.haiku]
usd_entrada = 1.0
usd_salida = 5.0

[balance]
prefiere_api_sobre = 0.8

[balance.cupo]
tope_usd = 50.0

[proveedores]
deepseek_url = "http://127.0.0.1:9/ds"
espera_base_ms = 1
"#;
        let config = Config::desde_texto(texto).expect("debe parsear");
        assert_eq!(config.modelos.construye, "sonnet");
        assert_eq!(config.modelos.planifica, "opus");
        assert_eq!(config.precios["haiku"].usd_salida, 5.0);
        assert_eq!(config.balance.cupo.tope_usd, 50.0);
        assert_eq!(config.balance.cupo.bloque_horas, 5);
        assert_eq!(config.balance.prefiere_api_sobre, 0.8);
        assert_eq!(config.umbral_verde, 0.75);
        assert_eq!(config.umbral_wilson, 0.6);
        assert_eq!(config.proveedores.deepseek_url, "http://127.0.0.1:9/ds");
        assert_eq!(config.proveedores.espera_base_ms, 1);
        assert_eq!(config.proveedores.timeout_s, 120);
    }

    #[test]
    fn config_root_toml_mal_formado_da_error() {
        assert!(Config::desde_texto("umbral_verde = \"x\"").is_err());
    }

    #[test]
    fn config_precio_con_clave_mal_escrita_da_codigo_2_y_nombra_la_clave() {
        let error = Config::desde_texto("[precios.x]\nusd_entrada = 1.0\nusd_salda = 1.1\n")
            .expect_err("una clave desconocida debe fallar");
        assert_eq!(error.codigo_de_salida(), 2);
        let mensaje = error.to_string();
        assert!(mensaje.contains("[precios.x]"), "{mensaje}");
        assert!(mensaje.contains("usd_salda"), "{mensaje}");
    }

    #[test]
    fn config_precio_sin_usd_salida_da_codigo_2() {
        let error = Config::desde_texto("[precios.x]\nusd_entrada = 1.0\n")
            .expect_err("un precio incompleto debe fallar");
        assert_eq!(error.codigo_de_salida(), 2);
        assert!(
            error.to_string().contains("[precios.x] exige usd_salida"),
            "{error}"
        );
    }

    #[test]
    fn config_precio_nan_inf_o_negativo_da_codigo_2() {
        for texto in [
            "[precios.x]\nusd_entrada = nan\nusd_salida = 1.0\n",
            "[precios.x]\nusd_entrada = 1.0\nusd_salida = inf\n",
            "[precios.x]\nusd_entrada = -1.0\nusd_salida = 1.0\n",
        ] {
            let error = Config::desde_texto(texto).expect_err("precio inválido debe fallar");
            assert_eq!(error.codigo_de_salida(), 2, "{texto}");
            assert!(
                error.to_string().contains("[precios.x]"),
                "{texto}: {error}"
            );
        }
    }

    #[test]
    fn config_precio_con_texto_en_vez_de_numero_da_codigo_2() {
        let error = Config::desde_texto("[precios.x]\nusd_entrada = \"1\"\nusd_salida = 1.0\n")
            .expect_err("un precio de texto debe fallar");
        assert_eq!(error.codigo_de_salida(), 2);
    }

    #[test]
    fn config_precio_completo_carga_con_sus_valores() {
        let c = Config::desde_texto("[precios.x]\nusd_entrada = 0.5\nusd_salida = 2\n")
            .expect("precio completo carga");
        assert_eq!(c.precios["x"].usd_entrada, 0.5);
        assert_eq!(c.precios["x"].usd_salida, 2.0);
    }

    #[test]
    fn config_precio_tramo_incompleto_da_codigo_2() {
        for texto in [
            "[precios.x]\nusd_entrada = 1.0\nusd_salida = 2.0\numbral_entrada = 100000\n",
            "[precios.x]\nusd_entrada = 1.0\nusd_salida = 2.0\numbral_entrada = 100000\nusd_entrada_alto = 0.5\n",
            "[precios.x]\nusd_entrada = 1.0\nusd_salida = 2.0\numbral_entrada = 100000\nusd_salida_alto = 2.5\n",
            "[precios.x]\nusd_entrada = 1.0\nusd_salida = 2.0\nusd_entrada_alto = 0.5\nusd_salida_alto = 2.5\n",
        ] {
            let error = Config::desde_texto(texto).expect_err(texto);
            assert_eq!(error.codigo_de_salida(), 2, "{texto}");
            assert!(error.to_string().contains("[precios.x]"), "{error}");
        }
    }

    #[test]
    fn config_precio_tramo_completo_carga_con_sus_valores() {
        let texto = "[precios.x]\nusd_entrada = 0.1\nusd_salida = 0.5\numbral_entrada = 100000\nusd_entrada_alto = 0.5\nusd_salida_alto = 2.5\n";
        let c = Config::desde_texto(texto).expect("tramo completo carga");
        let p = &c.precios["x"];
        assert_eq!(p.umbral_entrada, Some(100_000));
        assert_eq!(p.usd_entrada_alto, Some(0.5));
        assert_eq!(p.usd_salida_alto, Some(2.5));
    }

    #[test]
    fn config_precio_tramo_con_valor_invalido_da_codigo_2() {
        let base = "[precios.x]\nusd_entrada = 0.1\nusd_salida = 0.5\n";
        for extra in [
            "umbral_entrada = -5\nusd_entrada_alto = 0.5\nusd_salida_alto = 2.5\n",
            "umbral_entrada = 1.5\nusd_entrada_alto = 0.5\nusd_salida_alto = 2.5\n",
            "umbral_entrada = 100000\nusd_entrada_alto = nan\nusd_salida_alto = 2.5\n",
            "umbral_entrada = 100000\nusd_entrada_alto = 0.5\nusd_salida_alto = -1.0\n",
        ] {
            let texto = format!("{base}{extra}");
            let error = Config::desde_texto(&texto).expect_err(&texto);
            assert_eq!(error.codigo_de_salida(), 2, "{texto}");
        }
    }

    #[test]
    fn config_precios_por_defecto_son_los_de_las_decisiones_41_44_47() {
        let precios = Config::default().precios;
        let modelos: Vec<&str> = precios.keys().map(String::as_str).collect();
        assert_eq!(
            modelos,
            [
                "claude-haiku-5-5",
                "claude-sonnet-5-5",
                "deepseek-flash",
                "deepseek-v4-pro"
            ]
        );
        let flash = &precios["deepseek-flash"];
        assert_eq!((flash.usd_entrada, flash.usd_salida), (0.14, 0.28));
        let pro = &precios["deepseek-v4-pro"];
        assert_eq!((pro.usd_entrada, pro.usd_salida), (1.74, 3.48));
        let haiku = &precios["claude-haiku-5-5"];
        assert_eq!((haiku.usd_entrada, haiku.usd_salida), (0.10, 0.50));
        assert_eq!(haiku.umbral_entrada, Some(100_000));
        assert_eq!(haiku.usd_entrada_alto, Some(0.50));
        assert_eq!(haiku.usd_salida_alto, Some(2.50));
    }

    #[test]
    fn config_sin_root_toml_sonnet_cuesta_2_y_10_sin_tramos() {
        let ruta = Path::new("/ruta-que-no-existe-root-test/root.toml");
        let c = Config::cargar(ruta).expect("inexistente da default");
        let sonnet = c
            .precios
            .get("claude-sonnet-5-5")
            .expect("precio de Sonnet");
        assert_eq!((sonnet.usd_entrada, sonnet.usd_salida), (2.0, 10.0));
        assert_eq!(sonnet.umbral_entrada, None);
    }

    #[test]
    fn config_precio_de_root_toml_reemplaza_solo_ese_modelo() {
        let c = Config::desde_texto(
            "[precios.claude-haiku-5-5]\nusd_entrada = 1.0\nusd_salida = 5.0\n",
        )
        .expect("carga");
        let haiku = &c.precios["claude-haiku-5-5"];
        assert_eq!((haiku.usd_entrada, haiku.usd_salida), (1.0, 5.0));
        assert_eq!(
            haiku.umbral_entrada, None,
            "el reemplazo no hereda el tramo"
        );
        assert_eq!(c.precios["deepseek-flash"].usd_entrada, 0.14);
    }

    #[test]
    fn config_ruteo_viejo_de_deepseek_se_traduce_y_avisa() {
        let texto = "[ruteo.juzga]\nproveedor = \"deepseek\"\nmodelo = \"deepseek-reasoner\"\n";
        let c = Config::desde_texto(texto).expect("carga");
        assert_eq!(c.ruteo.juzga.modelo, "deepseek-flash");
        assert_eq!(c.avisos.len(), 1, "{:?}", c.avisos);
        assert!(
            c.avisos[0].contains("deepseek-reasoner") && c.avisos[0].contains("deepseek-flash"),
            "{:?}",
            c.avisos
        );
    }

    #[test]
    fn config_traducir_modelos_viejos_cambia_solo_deepseek_viejo() {
        let mut ruteo = Ruteo::default();
        ruteo.delega.modelo = "deepseek-chat".into();
        ruteo.verifica.modelo = "claude-sonnet-5-5".into();
        ruteo.verifica.proveedor = Proveedor::Anthropic;
        let avisos = traducir_modelos_viejos(&mut ruteo);
        assert_eq!(ruteo.delega.modelo, "deepseek-flash");
        assert_eq!(ruteo.verifica.modelo, "claude-sonnet-5-5");
        assert_eq!(avisos.len(), 1, "{avisos:?}");
        assert!(avisos[0].contains("[ruteo.delega]"), "{avisos:?}");
    }

    #[test]
    fn config_root_home_respeta_la_variable() {
        let ruta = resolver_root_home(
            Some(PathBuf::from("/datos/root")),
            Some(PathBuf::from("/h")),
        );
        assert_eq!(ruta, PathBuf::from("/datos/root"));
    }

    #[test]
    fn config_root_home_por_defecto_cuelga_de_home() {
        let ruta = resolver_root_home(None, Some(PathBuf::from("/h")));
        assert_eq!(ruta, PathBuf::from("/h/.local/share/root"));
    }

    #[test]
    fn config_root_home_valida_rechaza_raiz_relativa_y_home() {
        let home = Path::new("/h/usuario");
        assert!(validar_root_home(Path::new("/datos/root"), Some(home)).is_ok());
        assert!(validar_root_home(Path::new("/datos/root"), None).is_ok());
        assert!(validar_root_home(Path::new("/"), Some(home)).is_err());
        assert!(validar_root_home(Path::new("datos/root"), Some(home)).is_err());
        assert!(validar_root_home(Path::new("."), Some(home)).is_err());
        assert!(validar_root_home(Path::new("/h/usuario"), Some(home)).is_err());
        assert!(validar_root_home(Path::new("/h/usuario/"), Some(home)).is_err());
        assert!(validar_root_home(Path::new("/h/usuario/./"), Some(home)).is_err());
    }

    #[test]
    fn config_root_home_ignora_variable_vacia() {
        let ruta = resolver_root_home(Some(PathBuf::new()), Some(PathBuf::from("/h")));
        assert_eq!(ruta, PathBuf::from("/h/.local/share/root"));
    }

    #[test]
    fn route_balance_subtablas_parsean() {
        let texto = r#"
[balance]
prefiere_api_sobre = 0.8

[balance.cupo]
tope_usd = 40.0
bloque_horas = 4

[balance.api]
tope_usd = 90.0

[balance.nube]
tope_usd = 200.0
"#;
        let c = Config::desde_texto(texto).expect("debe parsear");
        assert_eq!(c.balance.prefiere_api_sobre, 0.8);
        assert_eq!(c.balance.cupo.tope_usd, 40.0);
        assert_eq!(c.balance.cupo.bloque_horas, 4);
        assert_eq!(c.balance.api.tope_usd, 90.0);
        assert_eq!(c.balance.nube.tope_usd, 200.0);
    }

    #[test]
    fn route_balance_defaults_35_5_100_250_09() {
        let c = Config::default();
        assert_eq!(c.balance.prefiere_api_sobre, 0.9);
        assert_eq!(c.balance.cupo.tope_usd, 35.0);
        assert_eq!(c.balance.cupo.bloque_horas, 5);
        assert_eq!(c.balance.api.tope_usd, 100.0);
        assert_eq!(c.balance.nube.tope_usd, 250.0);
    }

    #[test]
    fn route_root_toml_viejo_con_tope_cupo_usd_no_falla() {
        let c = Config::desde_texto("[balance]\ntope_cupo_usd = 50.0\n")
            .expect("una clave desconocida no rompe el parseo");
        assert_eq!(c.balance.cupo.tope_usd, 35.0);
    }

    #[test]
    fn route_ruteo_defaults() {
        let c = Config::default();
        assert_eq!(c.ruteo.delega.proveedor, Proveedor::Deepseek);
        assert_eq!(c.ruteo.delega.modelo, "deepseek-flash");
        assert_eq!(c.ruteo.verifica.proveedor, Proveedor::Deepseek);
        assert_eq!(c.ruteo.verifica.modelo, "deepseek-flash");
        assert_eq!(c.ruteo.juzga.modelo, "deepseek-flash");
        let eq = &c.ruteo.api_equivalente;
        assert_eq!(eq["opus"].proveedor, Proveedor::Anthropic);
        assert_eq!(eq["opus"].modelo, "claude-opus-5-5");
        assert_eq!(eq["sonnet"].modelo, "claude-sonnet-5-5");
        assert_eq!(eq["haiku"].modelo, "claude-haiku-5-5");
    }

    #[test]
    fn route_topes_invalidos_son_error() {
        for texto in [
            "[balance.cupo]\ntope_usd = 0.0",
            "[balance.cupo]\ntope_usd = -1.0",
            "[balance.api]\ntope_usd = 0.0",
            "[balance.nube]\ntope_usd = 0.0",
            "[balance.cupo]\ntope_usd = 35.0\nbloque_horas = 0",
            "[balance]\nprefiere_api_sobre = 0.0",
            "[balance]\nprefiere_api_sobre = 1.5",
            "[balance]\nprefiere_api_sobre = -0.1",
        ] {
            let error = Config::desde_texto(texto).expect_err("debe fallar");
            assert_eq!(error.codigo_de_salida(), 2, "{texto}");
        }
    }

    #[test]
    fn route_prefiere_api_sobre_uno_es_valido() {
        let c =
            Config::desde_texto("[balance]\nprefiere_api_sobre = 1.0").expect("1.0 está en (0, 1]");
        assert_eq!(c.balance.prefiere_api_sobre, 1.0);
    }

    #[test]
    fn route_tabla_de_tope_sin_tope_usd_da_codigo_2() {
        for (tabla, texto) in [
            ("balance.cupo", "[balance.cupo]\nbloque_horas = 4\n"),
            ("balance.api", "[balance.api]\n"),
            ("balance.nube", "[balance.nube]\n"),
        ] {
            let error = Config::desde_texto(texto).expect_err("sin tope_usd debe fallar");
            assert_eq!(error.codigo_de_salida(), 2, "{tabla}");
            let mensaje = error.to_string();
            assert!(
                mensaje.contains(&format!("[{tabla}] exige tope_usd")),
                "{tabla}: {mensaje}"
            );
        }
    }

    #[test]
    fn route_tope_negativo_da_codigo_2() {
        let error = Config::desde_texto("[balance.api]\ntope_usd = -1.0")
            .expect_err("tope negativo debe fallar");
        assert_eq!(error.codigo_de_salida(), 2);
    }

    #[test]
    fn route_cupo_con_tope_sin_bloque_usa_5_horas() {
        let c = Config::desde_texto("[balance.cupo]\ntope_usd = 20.0").expect("debe parsear");
        assert_eq!(c.balance.cupo.tope_usd, 20.0);
        assert_eq!(c.balance.cupo.bloque_horas, 5);
    }

    #[test]
    fn route_toml_malformado_sigue_siendo_codigo_3() {
        let error = Config::desde_texto("[balance\n").expect_err("TOML roto debe fallar");
        assert_eq!(error.codigo_de_salida(), 3);
    }

    #[test]
    fn route_alias_de_cupo_sin_api_equivalente_da_codigo_2() {
        let error = Config::desde_texto("[modelos]\norganiza = \"mistral\"\n")
            .expect_err("sin api_equivalente debe fallar");
        assert_eq!(error.codigo_de_salida(), 2);
        let mensaje = error.to_string();
        assert!(
            mensaje
                .contains("el alias \"mistral\" de [modelos] no tiene api_equivalente en [ruteo]"),
            "{mensaje}"
        );
    }

    #[test]
    fn route_alias_de_cupo_con_api_equivalente_carga() {
        let texto = "[modelos]\norganiza = \"mistral\"\n\n\
                     [ruteo.api_equivalente.mistral]\nproveedor = \"deepseek\"\nmodelo = \"mistral-large\"\n";
        let c = Config::desde_texto(texto).expect("con equivalente debe cargar");
        assert_eq!(c.ruteo.api_equivalente["mistral"].modelo, "mistral-large");
        // Una entrada del usuario suma a los defaults: opus y sonnet siguen definidos.
        assert_eq!(c.ruteo.api_equivalente["opus"].modelo, "claude-opus-5-5");
    }

    #[test]
    fn config_archivo_inexistente_da_default() {
        let ruta = Path::new("/ruta-que-no-existe-root-test/root.toml");
        assert_eq!(
            Config::cargar(ruta).expect("inexistente da default"),
            Config::default()
        );
    }

    #[test]
    fn config_umbrales_por_defecto_son_f64_exactos() {
        let c = Config::default();
        let wilson: f64 = c.umbral_wilson;
        let verde: f64 = c.umbral_verde;
        assert_eq!(wilson, 0.6_f64);
        assert_eq!(verde, 0.8_f64);
    }

    #[test]
    fn config_prefiere_api_sobre_0_9_ida_y_vuelta_exacta() {
        let c = Config::desde_texto("[balance]\nprefiere_api_sobre = 0.9").expect("debe parsear");
        let prefiere: f64 = c.balance.prefiere_api_sobre;
        assert_eq!(prefiere, 0.9_f64);
        let texto = toml::to_string(&c).expect("serializa");
        let vuelta = Config::desde_texto(&texto).expect("vuelve a parsear");
        assert_eq!(vuelta.balance.prefiere_api_sobre, 0.9_f64);
        assert_eq!(vuelta, c);
    }

    #[test]
    fn colab_config_minutos_max_por_defecto_es_120() {
        assert_eq!(Config::default().colab.minutos_max, 120.0);
        let c = Config::desde_texto("").expect("vacío debe parsear");
        assert_eq!(c.colab.minutos_max, 120.0);
    }

    #[test]
    fn colab_config_sin_minutos_max_vale_120() {
        for texto in ["", "[colab]\n"] {
            let c = Config::desde_texto(texto).expect("debe parsear");
            assert_eq!(c.colab.minutos_max, 120.0, "texto {texto:?}");
        }
    }

    #[test]
    fn colab_config_minutos_max_cero_da_codigo_2() {
        let error = Config::desde_texto("[colab]\nminutos_max = 0")
            .expect_err("minutos_max = 0 debe fallar");
        assert_eq!(error.codigo_de_salida(), 2);
        assert!(error.to_string().contains("colab.minutos_max"), "{error}");
    }

    #[test]
    fn colab_config_minutos_max_ida_y_vuelta() {
        let c = Config::desde_texto("[colab]\nminutos_max = 45").expect("debe parsear");
        assert_eq!(c.colab.minutos_max, 45.0);
        let texto = toml::to_string(&c).expect("serializa");
        let vuelta = Config::desde_texto(&texto).expect("vuelve a parsear");
        assert_eq!(vuelta, c);
    }

    #[test]
    fn colab_config_minutos_max_acepta_fraccion() {
        let c = Config::desde_texto("[colab]\nminutos_max = 0.01").expect("debe parsear");
        assert_eq!(c.colab.minutos_max, 0.01);
    }

    #[test]
    fn colab_config_minutos_max_entero_sigue_cargando() {
        let c = Config::desde_texto("[colab]\nminutos_max = 45").expect("debe parsear");
        assert_eq!(c.colab.minutos_max, 45.0);
    }

    #[test]
    fn colab_config_minutos_max_enorme_da_error_y_no_panica() {
        let error = Config::desde_texto("[colab]\nminutos_max = 1e300").expect_err("enorme");
        assert_eq!(error.codigo_de_salida(), 2);
        assert!(error.to_string().contains("colab.minutos_max"), "{error}");
    }

    fn registro_texto(nombre: &str, url: &str, env: &str) -> String {
        format!(
            "[[registro]]\nnombre = \"{nombre}\"\nurl = \"{url}\"\ncredencial_env = \"{env}\"\n"
        )
    }

    #[test]
    fn registro_sin_seccion_da_lista_vacia() {
        let c = Config::desde_texto("").expect("vacío debe parsear");
        assert!(c.registros.is_empty());
    }

    #[test]
    fn registro_valido_parsea_y_guarda_sus_tres_campos() {
        let texto = registro_texto("priv", "https://reg.example/index/", "ROOT_TOKEN_PRIV");
        let c = Config::desde_texto(&texto).expect("debe parsear");
        assert_eq!(c.registros.len(), 1);
        assert_eq!(c.registros[0].nombre, "priv");
        assert_eq!(c.registros[0].url, "https://reg.example/index/");
        assert_eq!(c.registros[0].credencial_env, "ROOT_TOKEN_PRIV");
    }

    #[test]
    fn registro_con_nombre_invalido_da_codigo_2() {
        for nombre in ["", "con espacio", "punto.no", "barra/no"] {
            let texto = registro_texto(nombre, "https://reg.example/", "ROOT_TOKEN_X");
            let error = Config::desde_texto(&texto).expect_err(nombre);
            assert_eq!(error.codigo_de_salida(), 2, "{nombre}");
            assert!(error.to_string().contains("registro"), "{error}");
        }
    }

    #[test]
    fn registro_con_url_que_no_es_https_o_sin_host_da_codigo_2() {
        for url in [
            "http://reg.example/index/",
            "ssh://reg.example/index/",
            "https://",
            "no es una url",
        ] {
            let texto = registro_texto("priv", url, "ROOT_TOKEN_X");
            let error = Config::desde_texto(&texto).expect_err(url);
            assert_eq!(error.codigo_de_salida(), 2, "{url}");
        }
    }

    #[test]
    fn registro_con_usuario_o_clave_en_la_url_da_codigo_2() {
        for url in ["https://u@reg.example/", "https://u:clave@reg.example/"] {
            let texto = registro_texto("priv", url, "ROOT_TOKEN_X");
            let error = Config::desde_texto(&texto).expect_err(url);
            assert_eq!(error.codigo_de_salida(), 2, "{url}");
            assert!(!error.to_string().contains("clave"), "{error}");
        }
    }

    #[test]
    fn registro_credencial_env_vacia_o_con_minusculas_da_codigo_2() {
        for env in ["", "minuscula", "CON-GUION"] {
            let texto = registro_texto("priv", "https://reg.example/", env);
            let error = Config::desde_texto(&texto).expect_err(env);
            assert_eq!(error.codigo_de_salida(), 2, "{env}");
        }
    }

    #[test]
    fn registro_usuario_env_es_opcional_y_se_valida_como_variable() {
        let base = registro_texto("priv", "https://reg.example/", "ROOT_TOKEN_PRIV");
        let sin = Config::desde_texto(&base).expect("sin usuario_env parsea");
        assert_eq!(sin.registros[0].usuario_env, None);
        let con = Config::desde_texto(&format!("{base}usuario_env = \"ROOT_USUARIO_PRIV\"\n"))
            .expect("con usuario_env parsea");
        assert_eq!(
            con.registros[0].usuario_env.as_deref(),
            Some("ROOT_USUARIO_PRIV")
        );
        for malo in ["", "minuscula", "CON-GUION"] {
            let texto = format!("{base}usuario_env = \"{malo}\"\n");
            let error = Config::desde_texto(&texto).expect_err(malo);
            assert_eq!(error.codigo_de_salida(), 2, "{malo}");
        }
    }

    #[test]
    fn registro_nombres_repetidos_dan_codigo_2() {
        let texto = format!(
            "{}{}",
            registro_texto("priv", "https://reg.example/", "ROOT_TOKEN_A"),
            registro_texto("priv", "https://otro.example/", "ROOT_TOKEN_B")
        );
        let error = Config::desde_texto(&texto).expect_err("repetido");
        assert_eq!(error.codigo_de_salida(), 2);
        assert!(error.to_string().contains("priv"), "{error}");
    }

    #[test]
    fn colab_config_minutos_max_negativo_o_infinito_da_error() {
        for texto in ["[colab]\nminutos_max = -1.0", "[colab]\nminutos_max = inf"] {
            let error = Config::desde_texto(texto).expect_err(texto);
            assert_eq!(error.codigo_de_salida(), 2);
            assert!(
                error
                    .to_string()
                    .contains("colab.minutos_max debe ser mayor que 0"),
                "{error}"
            );
        }
    }

    #[test]
    fn config_timeouts_por_defecto_son_120_y_1200() {
        let c = Config::desde_texto("").expect("vacío debe parsear");
        assert_eq!(c.proveedores.timeout_s, 120);
        assert_eq!(c.proveedores.timeout_juez_s, 1200);
    }

    #[test]
    fn config_timeout_en_cero_da_codigo_2() {
        for texto in [
            "[proveedores]\ntimeout_s = 0\n",
            "[proveedores]\ntimeout_juez_s = 0\n",
        ] {
            let error = Config::desde_texto(texto).expect_err(texto);
            assert_eq!(error.codigo_de_salida(), 2, "{texto}");
            assert!(error.to_string().contains("proveedores.timeout"), "{error}");
        }
    }

    #[test]
    fn config_jueces_config_modelo_por_defecto_es_sonnet() {
        let ruta = Path::new("/ruta-que-no-existe-root-test/root.toml");
        for c in [
            Config::desde_texto("").expect("vacío"),
            Config::cargar(ruta).expect("inexistente da default"),
        ] {
            assert_eq!(c.jueces.config_modelo, "claude-sonnet-5-5");
        }
    }

    #[test]
    fn config_jueces_config_modelo_se_lee_de_root_toml() {
        let c =
            Config::desde_texto("[jueces]\nconfig_modelo = \"deepseek-v4-pro\"\n").expect("carga");
        assert_eq!(c.jueces.config_modelo, "deepseek-v4-pro");
    }

    #[test]
    fn config_verifica_y_juzga_tienen_respaldo_haiku_por_defecto() {
        let c = Config::default();
        for api in [&c.ruteo.verifica, &c.ruteo.juzga] {
            let respaldo = api.respaldo.as_deref().expect("respaldo por defecto");
            assert_eq!(respaldo.proveedor, Proveedor::Anthropic);
            assert_eq!(respaldo.modelo, "claude-haiku-5-5");
        }
        assert!(c.ruteo.delega.respaldo.is_none());
    }

    #[test]
    fn config_respaldo_escrito_se_lee_y_si_falta_vale_haiku() {
        let texto = "[ruteo.verifica]\nproveedor = \"deepseek\"\nmodelo = \"deepseek-flash\"\n\
                     [ruteo.verifica.respaldo]\nproveedor = \"deepseek\"\nmodelo = \"deepseek-v4-pro\"\n\
                     [ruteo.juzga]\nproveedor = \"deepseek\"\nmodelo = \"deepseek-flash\"\n";
        let c = Config::desde_texto(texto).expect("carga");
        let escrito = c.ruteo.verifica.respaldo.as_deref().expect("escrito");
        assert_eq!(escrito.modelo, "deepseek-v4-pro");
        let juzga = c
            .ruteo
            .juzga
            .respaldo
            .as_deref()
            .expect("completa con Haiku");
        assert_eq!(juzga.modelo, "claude-haiku-5-5");
    }

    #[test]
    fn config_respaldo_en_delega_o_en_equivalente_da_codigo_2() {
        for texto in [
            "[ruteo.delega]\nproveedor = \"deepseek\"\nmodelo = \"deepseek-flash\"\n\
             [ruteo.delega.respaldo]\nproveedor = \"anthropic\"\nmodelo = \"claude-haiku-5-5\"\n",
            "[ruteo.api_equivalente.sonnet]\nproveedor = \"anthropic\"\nmodelo = \"claude-sonnet-5-5\"\n\
             [ruteo.api_equivalente.sonnet.respaldo]\nproveedor = \"anthropic\"\nmodelo = \"claude-haiku-5-5\"\n",
        ] {
            let error = Config::desde_texto(texto).expect_err(texto);
            assert_eq!(error.codigo_de_salida(), 2, "{texto}");
            assert!(error.to_string().contains("respaldo"), "{error}");
        }
    }

    #[test]
    fn config_decision_por_defecto_es_0_9_y_sin_minimos() {
        let cfg = Config::desde_texto("").expect("vacío carga");
        assert_eq!(cfg.decision.completitud_minima, 0.9);
        assert!(cfg.decision.minimo.is_empty());
        assert_eq!(cfg.decision, ReglasDecision::default());
    }

    #[test]
    fn config_decision_se_lee_de_root_toml() {
        let cfg = Config::desde_texto(
            "[decision]\ncompletitud_minima = 0.8\n[decision.minimo]\nseguridad = 0.7\ndiseno = 0.5\n",
        )
        .expect("carga");
        assert_eq!(cfg.decision.completitud_minima, 0.8);
        assert_eq!(cfg.decision.minimo_de(Dimension::Seguridad), Some(0.7));
        assert_eq!(cfg.decision.minimo_de(Dimension::Diseno), Some(0.5));
        assert_eq!(cfg.decision.minimo_de(Dimension::Pruebas), None);
    }

    #[test]
    fn config_decision_completitud_fuera_de_0_1_o_no_finita_da_codigo_2() {
        for texto in [
            "[decision]\ncompletitud_minima = 1.5\n",
            "[decision]\ncompletitud_minima = -0.1\n",
            "[decision]\ncompletitud_minima = nan\n",
            "[decision]\ncompletitud_minima = inf\n",
        ] {
            let error = Config::desde_texto(texto).expect_err(texto);
            assert_eq!(error.codigo_de_salida(), 2, "{texto}");
            assert!(error.to_string().contains("completitud_minima"), "{error}");
        }
    }

    #[test]
    fn config_decision_minimo_fuera_de_0_1_o_no_finito_da_codigo_2() {
        for texto in [
            "[decision.minimo]\nseguridad = 1.01\n",
            "[decision.minimo]\nseguridad = -0.2\n",
            "[decision.minimo]\nseguridad = nan\n",
        ] {
            let error = Config::desde_texto(texto).expect_err(texto);
            assert_eq!(error.codigo_de_salida(), 2, "{texto}");
            assert!(error.to_string().contains("seguridad"), "{error}");
        }
    }

    #[test]
    fn config_decision_minimo_con_dimension_desconocida_da_codigo_2() {
        let error = Config::desde_texto("[decision.minimo]\nseguridd = 0.8\n")
            .expect_err("dimensión inexistente");
        assert_eq!(error.codigo_de_salida(), 2);
        assert!(error.to_string().contains("seguridd"), "{error}");
    }
}
