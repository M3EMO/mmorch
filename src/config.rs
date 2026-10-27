//! Configuración de Root: `ROOT_HOME` y `root.toml`.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::Error;

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
    pub umbral_verde: f32,
    pub umbral_wilson: f32,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Balance {
    pub tope_cupo_usd: f64,
    /// Fracción del tope (0 a 1) a partir de la cual se prefiere la API.
    pub prefiere_api_sobre: f32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            modelos: Modelos::default(),
            precios: BTreeMap::new(),
            balance: Balance::default(),
            umbral_verde: 0.8,
            umbral_wilson: 0.5,
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
            tope_cupo_usd: 200.0,
            prefiere_api_sobre: 0.9,
        }
    }
}

impl Config {
    /// Parsea el texto de un `root.toml`. Un texto vacío da los defaults.
    pub fn desde_texto(texto: &str) -> Result<Config, Error> {
        Ok(toml::from_str(texto)?)
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
tope_cupo_usd = 50.0
prefiere_api_sobre = 0.8
"#;
        let config = Config::desde_texto(texto).expect("debe parsear");
        assert_eq!(config.modelos.construye, "sonnet");
        assert_eq!(config.modelos.planifica, "opus");
        assert_eq!(config.precios["haiku"].usd_salida, 5.0);
        assert_eq!(config.balance.tope_cupo_usd, 50.0);
        assert_eq!(config.balance.prefiere_api_sobre, 0.8);
        assert_eq!(config.umbral_verde, 0.75);
        assert_eq!(config.umbral_wilson, 0.6);
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
    fn config_archivo_inexistente_da_default() {
        let ruta = Path::new("/ruta-que-no-existe-root-test/root.toml");
        assert_eq!(
            Config::cargar(ruta).expect("inexistente da default"),
            Config::default()
        );
    }
}
