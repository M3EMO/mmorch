//! Argumentos y entorno de `claude -p` por rol. Los argumentos niegan la lectura de `oraculos/`
//! (decisiones 39 y 43). El entorno no lleva claves de API ni `ROOT_HOME`.

use std::fs;
use std::path::{Path, PathBuf};

use super::config::ClaudeCorrida;

/// Única variable del proceso que pasa al builder, además de la config. Lo demás no pasa.
const VARIABLES_DEL_PROCESO: &[&str] = &["PATH", "LANG"];
/// Variable de ECC que la config puede fijar.
const VARIABLE_HOOKS: &str = "ECC_DISABLED_HOOKS";

/// Argumentos de `claude -p` para un rol. Piden `stream-json`, el modelo, solo el proyecto como fuente de
/// configuración, ningún MCP y la negación de `oraculos/` bajo `home`.
pub fn args_claude(prompt: &str, modelo: &str, home: &Path) -> Vec<String> {
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
    args.push("--disallowedTools".to_string());
    args.extend(negadas);
    args
}

/// Entorno del lanzamiento del builder. De `entorno_del_proceso` solo pasan `PATH` y `LANG`. Si la config
/// trae `ecc_disabled_hooks`, agrega `ECC_DISABLED_HOOKS`. Nunca pasa claves de API ni `ROOT_HOME`.
pub fn env_del_builder(
    claude: &ClaudeCorrida,
    entorno_del_proceso: &[(String, String)],
) -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = entorno_del_proceso
        .iter()
        .filter(|(clave, _)| VARIABLES_DEL_PROCESO.contains(&clave.as_str()))
        .cloned()
        .collect();
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
    use tempfile::TempDir;

    fn par(clave: &str, valor: &str) -> (String, String) {
        (clave.to_string(), valor.to_string())
    }

    #[test]
    fn corre_args_traen_cada_pieza() {
        let tmp = TempDir::new().expect("tmp");
        let home = tmp.path().join("root");
        fs::create_dir_all(home.join("oraculos")).expect("oraculos");
        let args = args_claude("haz el plan", "claude-haiku-5-5", &home);

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
        let args = args_claude("p", "m", &enlace);
        let canonica = fs::canonicalize(real.join("oraculos")).expect("canonica");
        assert!(
            args.contains(&format!("Read({}/**)", canonica.display())),
            "{args:?}"
        );
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
        };
        let env = env_del_builder(&claude, &entorno);
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
        let env = env_del_builder(&ClaudeCorrida::default(), &entorno);
        assert!(
            env.iter().all(|(k, _)| k != "ECC_DISABLED_HOOKS"),
            "{env:?}"
        );
    }
}
