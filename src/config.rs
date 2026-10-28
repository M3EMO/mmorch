//! Configuración de Root: `ROOT_HOME` y `root.toml`.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::tipos::Proveedor;

/// Carpeta de datos de Root. Usa `ROOT_HOME` o, si no existe, `~/.local/share/root`.
pub fn root_home() -> PathBuf {
    resolver_root_home(
        std::env::var_os("ROOT_HOME").map(PathBuf::from),
        std::env::var_os("HOME").map(PathBuf::from),
    )
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
}

/// Sección `[colab]`: tope de duración de una corrida en Colab.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ConfigColab {
    /// Minutos máximos de una corrida en Colab. Al llegar al tope, Root libera el runtime.
    pub minutos_max: u32,
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Modelos {
    pub planifica: String,
    pub organiza: String,
    pub construye: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Precio {
    pub usd_entrada: f64,
    pub usd_salida: f64,
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
}

impl Default for Config {
    fn default() -> Self {
        Config {
            modelos: Modelos::default(),
            precios: BTreeMap::new(),
            balance: Balance::default(),
            ruteo: Ruteo::default(),
            umbral_verde: 0.8,
            umbral_wilson: 0.6,
            proveedores: ConfigProveedores::default(),
            colab: ConfigColab::default(),
        }
    }
}

impl Default for ConfigColab {
    fn default() -> Self {
        ConfigColab { minutos_max: 30 }
    }
}

impl Default for ConfigProveedores {
    fn default() -> Self {
        ConfigProveedores {
            deepseek_url: "https://api.deepseek.com".into(),
            anthropic_url: "https://api.anthropic.com".into(),
            espera_base_ms: 500,
            timeout_s: 120,
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

impl Default for Ruteo {
    fn default() -> Self {
        let equivalente = |proveedor, modelo: &str| ModeloApi {
            proveedor,
            modelo: modelo.into(),
        };
        Ruteo {
            delega: equivalente(Proveedor::Deepseek, "deepseek-chat"),
            verifica: equivalente(Proveedor::Deepseek, "deepseek-reasoner"),
            juzga: equivalente(Proveedor::Deepseek, "deepseek-reasoner"),
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

impl Config {
    /// Parsea el texto de un `root.toml`. Un texto vacío da los defaults.
    pub fn desde_texto(texto: &str) -> Result<Config, Error> {
        exigir_topes_declarados(texto)?;
        let mut config: Config = toml::from_str(texto)?;
        // Las entradas del usuario suman a los equivalentes de los defaults; no los reemplazan.
        for (alias, modelo) in Ruteo::default().api_equivalente {
            config.ruteo.api_equivalente.entry(alias).or_insert(modelo);
        }
        config.validar()?;
        Ok(config)
    }

    /// Rechaza topes en cero o negativos, `bloque_horas` en cero, `prefiere_api_sobre` fuera de (0, 1]
    /// y un alias de cupo en `[modelos]` sin `api_equivalente` en `[ruteo]`.
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
        if self.colab.minutos_max == 0 {
            return Err(Error::ConfigValor(
                "colab.minutos_max debe ser al menos 1".into(),
            ));
        }
        Ok(())
    }

    /// Lee `root.toml`. Si el archivo no existe, devuelve el default.
    pub fn cargar(ruta: &Path) -> Result<Config, Error> {
        match std::fs::read_to_string(ruta) {
            Ok(texto) => Config::desde_texto(&texto),
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
        assert_eq!(c.ruteo.delega.modelo, "deepseek-chat");
        assert_eq!(c.ruteo.verifica.proveedor, Proveedor::Deepseek);
        assert_eq!(c.ruteo.verifica.modelo, "deepseek-reasoner");
        assert_eq!(c.ruteo.juzga.modelo, "deepseek-reasoner");
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
    fn colab_config_minutos_max_por_defecto_es_30() {
        assert_eq!(Config::default().colab.minutos_max, 30);
        let c = Config::desde_texto("").expect("vacío debe parsear");
        assert_eq!(c.colab.minutos_max, 30);
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
        assert_eq!(c.colab.minutos_max, 45);
        let texto = toml::to_string(&c).expect("serializa");
        let vuelta = Config::desde_texto(&texto).expect("vuelve a parsear");
        assert_eq!(vuelta, c);
    }
}
