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
    /// Modo de permisos de `claude -p`. Ausente si la config no lo trae.
    #[serde(default)]
    pub permission_mode: Option<String>,
    /// ECC por rol: `[claude.rol.<rol>]`. Ausente si la config no la trae.
    #[serde(default)]
    pub rol: BTreeMap<Rol, ClaudeRol>,
}

/// Config de ECC para un rol: su plugin y el modelo de sus subagentes.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaudeRol {
    /// Carpeta del plugin de ECC del rol. Debe ser ruta absoluta.
    #[serde(default)]
    pub plugin_dir: Option<String>,
    /// Modelo de los subagentes del rol (`CLAUDE_CODE_SUBAGENT_MODEL`). Ausente si el rol no lo fija.
    #[serde(default)]
    pub subagent_model: Option<String>,
}

/// Modo de permisos de `claude -p` si la config no fija otro.
const MODO_PREDETERMINADO: &str = "auto";

impl ClaudeCorrida {
    /// Modo de permisos de `claude -p`. Vale `auto` si la config no lo fija.
    pub fn modo_permisos(&self) -> &str {
        self.permission_mode
            .as_deref()
            .unwrap_or(MODO_PREDETERMINADO)
    }

    /// Carpeta del plugin de ECC del rol, si la config la fija.
    pub fn plugin_de(&self, rol: Rol) -> Option<&Path> {
        self.rol
            .get(&rol)
            .and_then(|config| config.plugin_dir.as_deref())
            .map(Path::new)
    }

    /// Modelo de los subagentes del rol, si la config lo fija.
    pub fn subagent_de(&self, rol: Rol) -> Option<&str> {
        self.rol
            .get(&rol)
            .and_then(|config| config.subagent_model.as_deref())
    }
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
        for (rol, de_rol) in &config.claude.rol {
            if !config.roles.contains(rol) {
                return Err(format!(
                    "[claude.rol.{}] no está en roles",
                    nombre_rol(*rol)
                ));
            }
            let relativo = de_rol
                .plugin_dir
                .as_deref()
                .filter(|plugin| !Path::new(plugin).is_absolute());
            if let Some(plugin) = relativo {
                return Err(format!(
                    "el plugin de {} debe ser una ruta absoluta: {plugin}",
                    nombre_rol(*rol)
                ));
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
    fn corre_config_carga_modo_de_permisos() {
        let texto = format!("{VALIDA}permission_mode = \"acceptEdits\"\n");
        let config = ConfigCorrida::desde_texto(&texto).expect("valida");
        assert_eq!(
            config.claude.permission_mode.as_deref(),
            Some("acceptEdits")
        );
        assert_eq!(config.claude.modo_permisos(), "acceptEdits");
    }

    #[test]
    fn corre_config_sin_modo_de_permisos_es_auto() {
        let config = ConfigCorrida::desde_texto(VALIDA).expect("valida");
        assert_eq!(config.claude.modo_permisos(), "auto");
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

    const CONFIG_ECC_HAIKU: &str = include_str!("../../docs/builds/config-ecc-haiku.toml");

    #[test]
    fn corre_config_carga_claude_rol() {
        let texto = format!(
            "{VALIDA}\n[claude.rol.planifica]\nplugin_dir = \"/opt/ecc-plan\"\n\n[claude.rol.construye]\nplugin_dir = \"/opt/ecc-build\"\nsubagent_model = \"haiku\"\n"
        );
        let config = ConfigCorrida::desde_texto(&texto).expect("valida");
        assert_eq!(
            config.claude.plugin_de(Rol::Planifica),
            Some(Path::new("/opt/ecc-plan"))
        );
        assert_eq!(
            config.claude.plugin_de(Rol::Construye),
            Some(Path::new("/opt/ecc-build"))
        );
        assert_eq!(config.claude.subagent_de(Rol::Construye), Some("haiku"));
        assert_eq!(config.claude.subagent_de(Rol::Planifica), None);
        assert_eq!(config.claude.ecc_disabled_hooks.as_deref(), Some("a,b"));
    }

    #[test]
    fn corre_config_sin_claude_rol_no_tiene_plugin_ni_subagentes() {
        let config = ConfigCorrida::desde_texto(VALIDA).expect("valida");
        assert_eq!(config.claude.plugin_de(Rol::Planifica), None);
        assert_eq!(config.claude.subagent_de(Rol::Construye), None);
    }

    #[test]
    fn corre_config_docs_ecc_haiku_carga_y_fija_plugins_y_subagentes() {
        let config = ConfigCorrida::desde_texto(CONFIG_ECC_HAIKU).expect("carga la config");
        assert_eq!(
            config.claude.plugin_de(Rol::Planifica),
            Some(Path::new(
                "/home/mpera/Proyectos/bench-ecc-sdlc/.ecc-plugin"
            ))
        );
        assert_eq!(
            config.claude.plugin_de(Rol::Organiza),
            Some(Path::new(
                "/home/mpera/Proyectos/bench-ecc-sdlc/.ecc-plugin-haiku"
            ))
        );
        assert_eq!(config.claude.subagent_de(Rol::Organiza), Some("haiku"));
        assert_eq!(config.claude.subagent_de(Rol::Planifica), None);
    }

    #[test]
    fn corre_config_rol_fuera_de_roles_es_error() {
        let texto = r#"
roles = ["planifica"]
[prompt]
planifica = "x"
[claude.rol.organiza]
plugin_dir = "/opt/ecc"
"#;
        let error = ConfigCorrida::desde_texto(texto).expect_err("organiza no está en roles");
        assert!(error.contains("organiza"), "{error}");
    }

    #[test]
    fn corre_config_plugin_relativo_es_error() {
        let texto = r#"
roles = ["planifica"]
[prompt]
planifica = "x"
[claude.rol.planifica]
plugin_dir = "relativo/ecc"
"#;
        let error = ConfigCorrida::desde_texto(texto).expect_err("plugin relativo");
        assert!(error.contains("relativo/ecc"), "{error}");
    }

    #[test]
    fn corre_config_campo_desconocido_en_rol_es_error() {
        let texto = r#"
roles = ["planifica"]
[prompt]
planifica = "x"
[claude.rol.planifica]
otra = "x"
"#;
        assert!(ConfigCorrida::desde_texto(texto).is_err());
    }
}
