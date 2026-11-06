//! Argumentos y entorno de `claude -p` por rol. Los argumentos niegan la lectura de `oraculos/`
//! (decisiones 39 y 43). El entorno no lleva claves de API ni `ROOT_HOME`.

use std::fs;
use std::path::{Path, PathBuf};

use super::config::ClaudeCorrida;
use super::contenedor::{ECC_DATOS_EN_CONTENEDOR, TIMEOUT_BUILDER};
use crate::tipos::Rol;

/// Única variable del proceso que pasa al builder, además de la config. Lo demás no pasa.
const VARIABLES_DEL_PROCESO: &[&str] = &["PATH", "LANG"];
/// Variable de ECC que la config puede fijar.
const VARIABLE_HOOKS: &str = "ECC_DISABLED_HOOKS";
/// Carpeta del plugin de ECC dentro del contenedor. `args_podman_builder` la monta en solo lectura.
pub(super) const PLUGIN_EN_CONTENEDOR: &str = "/opt/plugin";
/// Variable que fija cuánto espera `claude -p` a sus procesos en segundo plano. Igual a `TIMEOUT_BUILDER`.
const VARIABLE_ESPERA: &str = "CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS";
/// Variable que fija el modelo de los subagentes. Solo la trae el rol que la define.
const VARIABLE_SUBAGENTES: &str = "CLAUDE_CODE_SUBAGENT_MODEL";
/// Variable que dice a ECC dónde guarda sus datos. Apunta a la carpeta de estado montada.
const VARIABLE_DATOS_ECC: &str = "ECC_AGENT_DATA_HOME";

/// Argumentos de `claude -p` para un rol. Piden `stream-json`, el modelo, solo el proyecto como fuente de
/// configuración, ningún MCP, el modo de permisos de la config y la negación de `oraculos/` bajo `home`.
pub fn args_claude(
    prompt: &str,
    modelo: &str,
    home: &Path,
    claude: &ClaudeCorrida,
    rol: Rol,
) -> Vec<String> {
    let oraculos = ruta_oraculos(home);
    let mut rutas = vec![oraculos.clone()];
    let tal_cual = home.join("oraculos");
    if tal_cual != oraculos {
        rutas.push(tal_cual);
    }
    // Las reglas van al final: `--disallowedTools` acepta varios valores y no debe comerse nada más.
    let mut negadas: Vec<String> = Vec::new();
    for ruta in &rutas {
        let o = ruta.display();
        negadas.push(format!("Read({o}/**)"));
        negadas.push(format!("Edit({o}/**)"));
        negadas.push(format!("Grep({o}/**)"));
        negadas.push(format!("Glob({o}/**)"));
    }
    negadas.push("Bash(*oraculos*)".to_string());

    let mut args = vec![
        "-p".to_string(),
        prompt.to_string(),
        "--output-format".to_string(),
        "stream-json".to_string(),
        "--verbose".to_string(),
        "--model".to_string(),
        modelo.to_string(),
        "--setting-sources".to_string(),
        "project".to_string(),
        "--strict-mcp-config".to_string(),
    ];
    if claude.plugin_de(rol).is_some() {
        args.push("--plugin-dir".to_string());
        args.push(PLUGIN_EN_CONTENEDOR.to_string());
    }
    args.push("--permission-mode".to_string());
    args.push(claude.modo_permisos().to_string());
    args.push("--disallowedTools".to_string());
    args.extend(negadas);
    args
}

/// Inserta `--resume <sesion>` justo después de `-p <prompt>`, en el índice 2. Si la lista es más corta, lo agrega al final.
pub fn con_resume(mut args: Vec<String>, sesion: &str) -> Vec<String> {
    let indice = args.len().min(2);
    args.splice(indice..indice, ["--resume".to_string(), sesion.to_string()]);
    args
}

/// Entorno del lanzamiento del builder. De `entorno_del_proceso` solo pasan `PATH` y `LANG`. Siempre fija la
/// espera de `claude` igual al plazo del builder y la carpeta de datos de ECC. Si el rol fija `subagent_model`, agrega
/// `CLAUDE_CODE_SUBAGENT_MODEL`. Si la config trae `ecc_disabled_hooks`, agrega `ECC_DISABLED_HOOKS`.
/// Nunca pasa claves de API ni `ROOT_HOME`.
pub fn env_del_builder(
    claude: &ClaudeCorrida,
    rol: Rol,
    entorno_del_proceso: &[(String, String)],
) -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = entorno_del_proceso
        .iter()
        .filter(|(clave, _)| VARIABLES_DEL_PROCESO.contains(&clave.as_str()))
        .cloned()
        .collect();
    env.push((
        VARIABLE_ESPERA.to_string(),
        TIMEOUT_BUILDER.as_millis().to_string(),
    ));
    env.push((
        VARIABLE_DATOS_ECC.to_string(),
        ECC_DATOS_EN_CONTENEDOR.to_string(),
    ));
    if let Some(modelo) = claude.subagent_de(rol) {
        env.push((VARIABLE_SUBAGENTES.to_string(), modelo.to_string()));
    }
    if let Some(hooks) = &claude.ecc_disabled_hooks {
        env.push((VARIABLE_HOOKS.to_string(), hooks.clone()));
    }
    env
}

/// Ruta de `oraculos/` bajo `home`, canonicalizada si existe. Si no existe, la ruta tal cual.
fn ruta_oraculos(home: &Path) -> PathBuf {
    let base = home.join("oraculos");
    fs::canonicalize(&base).unwrap_or(base)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corre::config::ClaudeRol;
    use crate::corre::contenedor::TIMEOUT_BUILDER;
    use std::collections::BTreeMap;
    use tempfile::TempDir;

    fn par(clave: &str, valor: &str) -> (String, String) {
        (clave.to_string(), valor.to_string())
    }

    /// Valor que sigue a `bandera`. Vacío si la bandera no está.
    fn valor_de(args: &[String], bandera: &str) -> String {
        args.iter()
            .position(|a| a == bandera)
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or_default()
    }

    /// Config de `claude` con un solo rol: su plugin y su modelo de subagentes.
    fn con_rol(rol: Rol, plugin: Option<&str>, subagente: Option<&str>) -> ClaudeCorrida {
        let mut roles = BTreeMap::new();
        roles.insert(
            rol,
            ClaudeRol {
                plugin_dir: plugin.map(String::from),
                subagent_model: subagente.map(String::from),
            },
        );
        ClaudeCorrida {
            rol: roles,
            ..Default::default()
        }
    }

    #[test]
    fn corre_args_sin_modo_en_config_pasa_auto() {
        let tmp = TempDir::new().expect("tmp");
        let home = tmp.path().join("root");
        fs::create_dir_all(home.join("oraculos")).expect("oraculos");
        let args = args_claude("p", "m", &home, &ClaudeCorrida::default(), Rol::Construye);
        assert_eq!(valor_de(&args, "--permission-mode"), "auto", "{args:?}");
    }

    #[test]
    fn corre_args_con_modo_de_config_pasa_ese_modo() {
        let tmp = TempDir::new().expect("tmp");
        let home = tmp.path().join("root");
        fs::create_dir_all(home.join("oraculos")).expect("oraculos");
        let claude = ClaudeCorrida {
            permission_mode: Some("plan".into()),
            ..Default::default()
        };
        let args = args_claude("p", "m", &home, &claude, Rol::Construye);
        assert_eq!(valor_de(&args, "--permission-mode"), "plan", "{args:?}");
    }

    #[test]
    fn corre_args_modo_va_antes_de_disallowed_tools() {
        let tmp = TempDir::new().expect("tmp");
        let home = tmp.path().join("root");
        fs::create_dir_all(home.join("oraculos")).expect("oraculos");
        let args = args_claude("p", "m", &home, &ClaudeCorrida::default(), Rol::Construye);

        let modo = args.iter().position(|a| a == "--permission-mode");
        let negar = args.iter().position(|a| a == "--disallowedTools");
        assert!(modo.is_some(), "falta --permission-mode en {args:?}");
        assert!(negar.is_some(), "falta --disallowedTools en {args:?}");
        let (modo, negar) = (modo.unwrap(), negar.unwrap());
        assert!(modo < negar, "--permission-mode debe ir antes: {args:?}");
        assert!(
            args[negar + 1..].iter().all(|a| a.contains('(')),
            "solo reglas después de --disallowedTools: {args:?}"
        );
        assert!(args[negar + 1].starts_with("Read("), "{args:?}");
    }

    #[test]
    fn corre_args_traen_cada_pieza() {
        let tmp = TempDir::new().expect("tmp");
        let home = tmp.path().join("root");
        fs::create_dir_all(home.join("oraculos")).expect("oraculos");
        let args = args_claude(
            "haz el plan",
            "claude-haiku-5-5",
            &home,
            &ClaudeCorrida::default(),
            Rol::Construye,
        );

        let posicion = |bandera: &str| {
            args.iter()
                .position(|a| a == bandera)
                .unwrap_or_else(|| panic!("falta {bandera} en {args:?}"))
        };
        assert_eq!(args[posicion("-p") + 1], "haz el plan");
        assert_eq!(args[posicion("--model") + 1], "claude-haiku-5-5");
        assert_eq!(args[posicion("--output-format") + 1], "stream-json");
        assert_eq!(args[posicion("--setting-sources") + 1], "project");
        assert!(args.contains(&"--verbose".to_string()));
        assert!(args.contains(&"--strict-mcp-config".to_string()));

        let oraculos = fs::canonicalize(home.join("oraculos")).expect("canonica");
        let o = oraculos.display();
        for regla in [
            format!("Read({o}/**)"),
            format!("Edit({o}/**)"),
            format!("Grep({o}/**)"),
            format!("Glob({o}/**)"),
            "Bash(*oraculos*)".to_string(),
        ] {
            assert!(args.contains(&regla), "falta {regla} en {args:?}");
        }
        assert!(posicion("--disallowedTools") < args.len() - 1);
    }

    #[test]
    fn corre_args_niegan_la_ruta_real_si_home_tiene_enlace() {
        let tmp = TempDir::new().expect("tmp");
        let real = tmp.path().join("real");
        fs::create_dir_all(real.join("oraculos")).expect("oraculos");
        let enlace = tmp.path().join("enlace");
        std::os::unix::fs::symlink(&real, &enlace).expect("enlace");
        let args = args_claude("p", "m", &enlace, &ClaudeCorrida::default(), Rol::Construye);
        let canonica = fs::canonicalize(real.join("oraculos")).expect("canonica");
        assert!(
            args.contains(&format!("Read({}/**)", canonica.display())),
            "{args:?}"
        );
    }

    #[test]
    fn corre_args_sin_plugin_no_traen_plugin_dir() {
        let tmp = TempDir::new().expect("tmp");
        let home = tmp.path().join("root");
        fs::create_dir_all(home.join("oraculos")).expect("oraculos");
        let args = args_claude("p", "m", &home, &ClaudeCorrida::default(), Rol::Planifica);
        assert!(!args.contains(&"--plugin-dir".to_string()), "{args:?}");
    }

    #[test]
    fn corre_args_con_plugin_traen_plugin_en_contenedor_antes_de_permisos() {
        let tmp = TempDir::new().expect("tmp");
        let home = tmp.path().join("root");
        fs::create_dir_all(home.join("oraculos")).expect("oraculos");
        let claude = con_rol(Rol::Planifica, Some("/home/mpera/ecc-plan"), None);
        let args = args_claude("p", "m", &home, &claude, Rol::Planifica);

        assert_eq!(valor_de(&args, "--plugin-dir"), "/opt/plugin", "{args:?}");
        let plugin = args.iter().position(|a| a == "--plugin-dir");
        let modo = args.iter().position(|a| a == "--permission-mode");
        let negar = args.iter().position(|a| a == "--disallowedTools");
        let (plugin, modo, negar) = (plugin.unwrap(), modo.unwrap(), negar.unwrap());
        assert!(
            plugin < modo,
            "--plugin-dir debe ir antes de --permission-mode: {args:?}"
        );
        assert!(modo < negar, "--disallowedTools va al final: {args:?}");
        assert!(
            args[negar + 1..].iter().all(|a| a.contains('(')),
            "solo reglas después de --disallowedTools: {args:?}"
        );
        assert!(
            args.iter().all(|a| !a.contains("ecc-plan")),
            "la ruta del host no va a claude: {args:?}"
        );
    }

    #[test]
    fn corre_args_plugin_de_otro_rol_no_se_pasa() {
        let tmp = TempDir::new().expect("tmp");
        let home = tmp.path().join("root");
        fs::create_dir_all(home.join("oraculos")).expect("oraculos");
        let claude = con_rol(Rol::Planifica, Some("/home/mpera/ecc-plan"), None);
        let args = args_claude("p", "m", &home, &claude, Rol::Organiza);
        assert!(!args.contains(&"--plugin-dir".to_string()), "{args:?}");
    }

    #[test]
    fn corre_args_traen_cada_pieza_con_plugin() {
        let tmp = TempDir::new().expect("tmp");
        let home = tmp.path().join("root");
        fs::create_dir_all(home.join("oraculos")).expect("oraculos");
        let claude = con_rol(Rol::Organiza, Some("/home/mpera/ecc-haiku"), Some("haiku"));
        let args = args_claude("haz", "m", &home, &claude, Rol::Organiza);
        assert!(
            args.contains(&"--strict-mcp-config".to_string()),
            "{args:?}"
        );
        assert!(args.contains(&"--verbose".to_string()), "{args:?}");
        assert_eq!(valor_de(&args, "--setting-sources"), "project", "{args:?}");
        assert!(args.contains(&"Bash(*oraculos*)".to_string()), "{args:?}");
    }

    #[test]
    fn corre_env_no_trae_claves_ni_root_home() {
        let entorno = vec![
            par("PATH", "/usr/bin"),
            par("DEEPSEEK_API_KEY", "clave-1"),
            par("ANTHROPIC_API_KEY", "clave-2"),
            par("ROOT_HOME", "/datos/root"),
            par("HOME", "/casa"),
            par("ECC_DISABLED_HOOKS", "del-proceso"),
        ];
        let claude = ClaudeCorrida {
            ecc_disabled_hooks: Some("a,b".into()),
            ..Default::default()
        };
        let env = env_del_builder(&claude, Rol::Construye, &entorno);
        for prohibida in ["DEEPSEEK_API_KEY", "ANTHROPIC_API_KEY", "ROOT_HOME", "HOME"] {
            assert!(
                env.iter().all(|(k, _)| k != prohibida),
                "{prohibida} no debe pasar: {env:?}"
            );
        }
        assert!(env.contains(&par("PATH", "/usr/bin")), "{env:?}");
        assert!(env.contains(&par("ECC_DISABLED_HOOKS", "a,b")), "{env:?}");
        assert_eq!(
            env.iter()
                .filter(|(k, _)| k == "ECC_DISABLED_HOOKS")
                .count(),
            1,
            "{env:?}"
        );
    }

    #[test]
    fn corre_env_sin_config_no_pone_hooks() {
        let entorno = vec![
            par("PATH", "/usr/bin"),
            par("ECC_DISABLED_HOOKS", "del-proceso"),
        ];
        let env = env_del_builder(&ClaudeCorrida::default(), Rol::Construye, &entorno);
        assert!(
            env.iter().all(|(k, _)| k != "ECC_DISABLED_HOOKS"),
            "{env:?}"
        );
    }

    #[test]
    fn corre_env_siempre_trae_espera_del_plazo_y_datos_de_ecc() {
        let entorno = vec![par("PATH", "/usr/bin")];
        let env = env_del_builder(&ClaudeCorrida::default(), Rol::Planifica, &entorno);
        let plazo = TIMEOUT_BUILDER.as_millis().to_string();
        assert_eq!(plazo, "7200000");
        assert!(
            env.contains(&par("CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS", &plazo)),
            "{env:?}"
        );
        assert!(
            env.iter()
                .all(|(k, v)| k != "CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS" || v != "0"),
            "la espera no debe ser 0: {env:?}"
        );
        assert!(
            env.contains(&par(
                "ECC_AGENT_DATA_HOME",
                "/home/builder/.claude/ecc-datos"
            )),
            "{env:?}"
        );
    }

    #[test]
    fn corre_env_subagent_model_solo_si_el_rol_lo_define_y_cambia_por_rol() {
        let mut roles = BTreeMap::new();
        roles.insert(
            Rol::Organiza,
            ClaudeRol {
                plugin_dir: None,
                subagent_model: Some("haiku".into()),
            },
        );
        roles.insert(
            Rol::Planifica,
            ClaudeRol {
                plugin_dir: None,
                subagent_model: Some("sonnet".into()),
            },
        );
        roles.insert(Rol::Construye, ClaudeRol::default());
        let claude = ClaudeCorrida {
            rol: roles,
            ..Default::default()
        };
        let entorno = vec![par("PATH", "/usr/bin")];
        let subagente = |rol: Rol| {
            env_del_builder(&claude, rol, &entorno)
                .into_iter()
                .find(|(k, _)| k == "CLAUDE_CODE_SUBAGENT_MODEL")
                .map(|(_, v)| v)
        };
        assert_eq!(subagente(Rol::Organiza), Some("haiku".to_string()));
        assert_eq!(subagente(Rol::Planifica), Some("sonnet".to_string()));
        assert_eq!(subagente(Rol::Construye), None);
        assert_eq!(subagente(Rol::Verifica), None);
    }

    #[test]
    fn corre_con_resume_va_justo_despues_del_prompt() {
        let args: Vec<String> = ["-p", "haz", "--output-format", "stream-json"]
            .map(String::from)
            .to_vec();
        let con = con_resume(args, "sesion-1");
        let esperado: Vec<String> = [
            "-p",
            "haz",
            "--resume",
            "sesion-1",
            "--output-format",
            "stream-json",
        ]
        .map(String::from)
        .to_vec();
        assert_eq!(con, esperado);
    }

    #[test]
    fn corre_con_resume_sobre_args_de_claude_deja_el_prompt_en_su_lugar() {
        let tmp = TempDir::new().expect("tmp");
        let home = tmp.path().join("root");
        fs::create_dir_all(home.join("oraculos")).expect("oraculos");
        let args = args_claude(
            "el prompt",
            "m",
            &home,
            &ClaudeCorrida::default(),
            Rol::Construye,
        );
        let con = con_resume(args, "s9");
        assert_eq!(con[0], "-p");
        assert_eq!(con[1], "el prompt");
        assert_eq!(valor_de(&con, "--resume"), "s9", "{con:?}");
        assert_eq!(con[2], "--resume", "{con:?}");
        assert_eq!(valor_de(&con, "--model"), "m", "{con:?}");
    }
}
