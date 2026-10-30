//! Configuración de una corrida: `<home>/configs/<config>.toml`. Dice el orden de los roles, su prompt
//! y las variables de `claude` que la corrida necesita.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::ErrorCorre;
use crate::oracle::pendientes::nombre_de_tarea_valido;
use crate::route::politica::nombre_rol;
use crate::tipos::{Forma, Rol};

/// Carpeta de las configuraciones dentro de `ROOT_HOME`.
const CARPETA_CONFIGS: &str = "configs";

/// Configuración de una corrida. `roles` define el orden; `prompt` trae el texto de cada rol.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigCorrida {
    pub roles: Vec<Rol>,
    pub prompt: BTreeMap<Rol, String>,
    /// Opcional: variables que la corrida pasa a `claude`.
    #[serde(default)]
    pub claude: ClaudeCorrida,
}

/// Variables de `claude` que la config fija.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaudeCorrida {
    /// Valor de `ECC_DISABLED_HOOKS` para el builder. Ausente si la config no lo trae.
    #[serde(default)]
    pub ecc_disabled_hooks: Option<String>,
}

impl ConfigCorrida {
    /// Lee `<home>/configs/<nombre>.toml` y lo valida. Un nombre con ruta, un archivo que no existe o una
    /// config inválida son error de uso.
    pub fn cargar(home: &Path, nombre: &str) -> Result<Self, ErrorCorre> {
        nombre_de_tarea_valido(nombre).map_err(|m| ErrorCorre::Uso(format!("config: {m}")))?;
        let ruta = ruta_de(home, nombre);
        let texto = fs::read_to_string(&ruta).map_err(|e| {
            ErrorCorre::Uso(format!("no se pudo leer la config {}: {e}", ruta.display()))
        })?;
        Self::desde_texto(&texto)
            .map_err(|m| ErrorCorre::Uso(format!("config {}: {m}", ruta.display())))
    }

    /// Lee la config de un texto TOML y la valida: `roles` no vacío, sin repetidos y con prompt por rol.
    pub fn desde_texto(texto: &str) -> Result<Self, String> {
        let config: ConfigCorrida = toml::from_str(texto).map_err(|e| e.to_string())?;
        if config.roles.is_empty() {
            return Err("roles está vacío".into());
        }
        let mut vistos = BTreeSet::new();
        for rol in &config.roles {
            if !vistos.insert(*rol) {
                return Err(format!("el rol {} está repetido", nombre_rol(*rol)));
            }
            if !config.prompt.contains_key(rol) {
                return Err(format!("falta el prompt de {}", nombre_rol(*rol)));
            }
        }
        Ok(config)
    }

    /// Forma de la corrida (decisión 28): roles en orden, si hubo verificador y el nombre de la config.
    pub fn forma(&self, nombre: &str) -> Forma {
        Forma {
            roles: self.roles.clone(),
            verificador: self.roles.contains(&Rol::Verifica),
            config: nombre.to_string(),
        }
    }
}

/// Ruta de la config: `<home>/configs/<nombre>.toml`.
pub fn ruta_de(home: &Path, nombre: &str) -> PathBuf {
    home.join(CARPETA_CONFIGS).join(format!("{nombre}.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const VALIDA: &str = r#"
roles = ["planifica", "construye"]

[prompt]
planifica = "texto de planifica"
construye = "texto de construye"

[claude]
ecc_disabled_hooks = "a,b"
"#;

    fn escribir(home: &Path, nombre: &str, texto: &str) {
        let carpeta = home.join(CARPETA_CONFIGS);
        fs::create_dir_all(&carpeta).expect("configs");
        fs::write(carpeta.join(format!("{nombre}.toml")), texto).expect("escribe");
    }

    #[test]
    fn corre_config_carga_valida() {
        let tmp = TempDir::new().expect("tmp");
        escribir(tmp.path(), "ecc-haiku", VALIDA);
        let config = ConfigCorrida::cargar(tmp.path(), "ecc-haiku").expect("carga");
        assert_eq!(config.roles, vec![Rol::Planifica, Rol::Construye]);
        assert_eq!(config.prompt[&Rol::Planifica], "texto de planifica");
        assert_eq!(config.prompt[&Rol::Construye], "texto de construye");
        assert_eq!(config.claude.ecc_disabled_hooks.as_deref(), Some("a,b"));
    }

    #[test]
    fn corre_config_sin_seccion_claude_es_valida() {
        let texto = VALIDA.split("[claude]").next().expect("corta");
        let config = ConfigCorrida::desde_texto(texto).expect("valida");
        assert_eq!(config.claude, ClaudeCorrida::default());
    }

    #[test]
    fn corre_config_rol_sin_prompt_es_error() {
        let texto = r#"
roles = ["planifica", "verifica"]
[prompt]
planifica = "x"
"#;
        let error = ConfigCorrida::desde_texto(texto).expect_err("falta el prompt de verifica");
        assert!(error.contains("verifica"), "{error}");
    }

    #[test]
    fn corre_config_rol_repetido_es_error() {
        let texto = r#"
roles = ["planifica", "planifica"]
[prompt]
planifica = "x"
"#;
        let error = ConfigCorrida::desde_texto(texto).expect_err("rol repetido");
        assert!(error.contains("planifica"), "{error}");
    }

    #[test]
    fn corre_config_roles_vacios_es_error() {
        let texto = "roles = []\n[prompt]\n";
        assert!(ConfigCorrida::desde_texto(texto).is_err());
    }

    #[test]
    fn corre_config_campo_desconocido_es_error() {
        let texto = format!("{VALIDA}\notra = 1\n");
        assert!(ConfigCorrida::desde_texto(&texto).is_err());
    }

    #[test]
    fn corre_config_inexistente_es_error_con_la_ruta() {
        let tmp = TempDir::new().expect("tmp");
        let error = ConfigCorrida::cargar(tmp.path(), "no-existe").expect_err("no existe");
        let ruta = ruta_de(tmp.path(), "no-existe");
        match error {
            ErrorCorre::Uso(mensaje) => {
                assert!(mensaje.contains(&ruta.display().to_string()), "{mensaje}");
            }
            otro => panic!("debe ser de uso: {otro}"),
        }
    }

    #[test]
    fn corre_config_nombre_con_ruta_es_uso() {
        let tmp = TempDir::new().expect("tmp");
        for nombre in ["a/b", "..", ""] {
            let error = ConfigCorrida::cargar(tmp.path(), nombre).expect_err("nombre inválido");
            assert!(matches!(error, ErrorCorre::Uso(_)), "{nombre}: {error}");
        }
    }

    #[test]
    fn corre_config_forma_correcta() {
        let config = ConfigCorrida::desde_texto(VALIDA).expect("valida");
        let forma = config.forma("ecc-haiku");
        assert_eq!(
            forma,
            Forma {
                roles: vec![Rol::Planifica, Rol::Construye],
                verificador: false,
                config: "ecc-haiku".into(),
            }
        );
        let con_verificador = ConfigCorrida::desde_texto(
            r#"
roles = ["construye", "verifica"]
[prompt]
construye = "x"
verifica = "y"
"#,
        )
        .expect("valida");
        let forma = con_verificador.forma("ecc-deepseek");
        assert!(forma.verificador);
        assert_eq!(forma.roles, vec![Rol::Construye, Rol::Verifica]);
    }
}
