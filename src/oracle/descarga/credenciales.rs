//! Credenciales de los registros privados (decisión 29). Root lee cada credencial de la variable que
//! nombra su `[[registro]]` y la pasa solo a los contenedores de descarga de Rust, y solo si el host
//! del registro del repo es exactamente el host de la `url` del registro. Ningún texto que sale de
//! Root la muestra: `Credencial` no tiene `Debug` con el valor, y `redactar_secretos` lo oculta.

use std::fmt;

use serde_json::json;
use url::Url;

use crate::config::RegistroConfig;
use crate::oracle::descarga::inventario::Inventario;

/// Prefijos de herramienta que quita `host_de` antes de parsear la dirección.
const PREFIJOS_REGISTRO: [&str; 2] = ["sparse+", "registry+"];

/// Host en minúscula de una dirección de registro. `None` si no parsea, no es `https` o no tiene host.
pub fn host_de(url: &str) -> Option<String> {
    let sin_prefijo = PREFIJOS_REGISTRO
        .iter()
        .find_map(|p| url.strip_prefix(p))
        .unwrap_or(url);
    let parseada = Url::parse(sin_prefijo).ok()?;
    if parseada.scheme() != "https" {
        return None;
    }
    parseada
        .host_str()
        .filter(|h| !h.is_empty())
        .map(str::to_lowercase)
}

/// El registro de `[[registro]]` cuyo host y puerto son exactamente los de `url_repo`. Sin sufijos ni
/// esquemas distintos de `https`: un host parecido, u otro puerto, no alcanza.
pub fn registro_para<'a>(
    registros: &'a [RegistroConfig],
    url_repo: &str,
) -> Option<&'a RegistroConfig> {
    let origen = origen_de(url_repo)?;
    registros
        .iter()
        .find(|r| origen_de(&r.url).as_ref() == Some(&origen))
}

/// Host y puerto efectivo de una URL `https`. Otro puerto del mismo host es otro servidor.
fn origen_de(url: &str) -> Option<(String, u16)> {
    let host = host_de(url)?;
    let sin_prefijo = PREFIJOS_REGISTRO
        .iter()
        .find_map(|p| url.strip_prefix(p))
        .unwrap_or(url);
    let puerto = Url::parse(sin_prefijo).ok()?.port_or_known_default()?;
    Some((host, puerto))
}

/// Variable que Cargo lee para el token de un registro: `CARGO_REGISTRIES_<NOMBRE>_TOKEN`, con el
/// nombre en mayúsculas y `-` cambiado a `_`.
pub fn variable_cargo(nombre_registro: &str) -> String {
    let nombre: String = nombre_registro
        .chars()
        .map(|c| {
            if c == '-' {
                '_'
            } else {
                c.to_ascii_uppercase()
            }
        })
        .collect();
    format!("CARGO_REGISTRIES_{nombre}_TOKEN")
}

/// Credencial de un registro: la variable de Cargo y su valor. El valor no sale de Root: `Debug`
/// lo oculta y `redactar_secretos` lo quita de cualquier texto.
#[derive(Clone)]
pub struct Credencial {
    pub variable: String,
    valor: String,
}

impl Credencial {
    /// El valor, solo para pasarlo al contenedor de descarga.
    pub fn valor(&self) -> &str {
        &self.valor
    }
}

impl fmt::Debug for Credencial {
    fn fmt(&self, formato: &mut fmt::Formatter<'_>) -> fmt::Result {
        formato
            .debug_struct("Credencial")
            .field("variable", &self.variable)
            .field("valor", &"***")
            .finish()
    }
}

/// Credenciales para los registros que el repo usa por nombre. Un registro entra si su host es el de
/// un `[[registro]]`, y si la variable de entorno de ese registro tiene un valor. Sin eso no hay
/// credencial, y no es error.
pub fn credenciales_cargo(
    inv: &Inventario,
    registros: &[RegistroConfig],
    leer_env: impl Fn(&str) -> Option<String>,
) -> Vec<Credencial> {
    let mut salida: Vec<Credencial> = Vec::new();
    for usado in &inv.registros_nombrados {
        let Some(registro) = registro_para(registros, &usado.direccion) else {
            continue;
        };
        let Some(valor) = leer_env(&registro.credencial_env).filter(|v| !v.is_empty()) else {
            continue;
        };
        let variable = variable_cargo(&usado.nombre);
        if salida.iter().any(|c| c.variable == variable) {
            continue;
        }
        salida.push(Credencial { variable, valor });
    }
    salida
}

/// Usuario del `netrc` de pip de un registro: el valor de `usuario_env`, o `__token__` si el registro no la
/// nombra o la variable no tiene valor. `__token__` es la convención de los índices compatibles con PyPI.
pub fn usuario_de_pip(
    registro: &RegistroConfig,
    leer_env: impl Fn(&str) -> Option<String>,
) -> String {
    registro
        .usuario_env
        .as_deref()
        .and_then(leer_env)
        .filter(|valor| !valor.is_empty())
        .unwrap_or_else(|| "__token__".to_string())
}

/// Archivo `netrc` de una credencial, para pip. Sin conectar todavía: la descarga de Python no arma
/// credenciales de pip (`credenciales_cargo` es solo de Cargo). Queda lista para cuando lo haga.
pub fn netrc_para(host: &str, usuario: &str, clave: &str) -> String {
    format!("machine {host} login {usuario} password {clave}\n")
}

/// Archivo `auth.json` de `podman pull --authfile`. `valor` ya es el token codificado que pone el
/// humano en la variable. Sin conectar todavía: el pull de imágenes privadas no la usa aún.
pub fn authfile_para(host: &str, valor: &str) -> String {
    json!({"auths": {host: {"auth": valor}}}).to_string()
}

/// Reemplaza cada valor de credencial por `***`. Se usa en todo texto que sale del contenedor de
/// descarga: diagnósticos, errores e hallazgos. Un valor vacío no se reemplaza.
pub fn redactar_secretos(texto: &str, credenciales: &[Credencial]) -> String {
    credenciales
        .iter()
        .filter(|c| !c.valor.is_empty())
        .fold(texto.to_string(), |acumulado, c| {
            acumulado.replace(&c.valor, "***")
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RegistroConfig;
    use crate::oracle::descarga::inventario::inventariar;

    fn registro(nombre: &str, url: &str, env: &str) -> RegistroConfig {
        RegistroConfig {
            nombre: nombre.into(),
            url: url.into(),
            credencial_env: env.into(),
            usuario_env: None,
        }
    }

    fn credencial_de_prueba() -> Credencial {
        Credencial {
            variable: "CARGO_REGISTRIES_PRIV_TOKEN".into(),
            valor: "valor-secreto-9".into(),
        }
    }

    /// Repo con una dependencia del registro `priv`, y su `.cargo/config.toml` con `index` dado.
    fn inventario_con_registro(index: &str) -> Inventario {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"x\"\nversion = \"0.1.0\"\n\n[dependencies]\n\
             secreto = { version = \"1\", registry = \"priv\" }\n",
        )
        .expect("escribe Cargo.toml");
        std::fs::create_dir_all(dir.path().join(".cargo")).expect("crea .cargo");
        std::fs::write(
            dir.path().join(".cargo/config.toml"),
            format!("[registries.priv]\nindex = \"{index}\"\n"),
        )
        .expect("escribe config");
        inventariar(dir.path()).expect("inventaria sin red")
    }

    #[test]
    fn credenciales_host_de_toma_el_host_en_minuscula_y_quita_prefijos() {
        assert_eq!(
            host_de("sparse+https://Reg.Example/index/"),
            Some("reg.example".into())
        );
        assert_eq!(
            host_de("registry+https://reg.example/x"),
            Some("reg.example".into())
        );
        assert_eq!(
            host_de("https://reg.example:8443/x"),
            Some("reg.example".into())
        );
    }

    #[test]
    fn credenciales_host_de_es_none_si_no_es_https_o_no_parsea() {
        assert_eq!(host_de("http://reg.example/"), None);
        assert_eq!(host_de("ssh://reg.example/"), None);
        assert_eq!(host_de("no es una url"), None);
    }

    #[test]
    fn credenciales_registro_para_exige_host_exacto_y_https() {
        let regs = vec![registro("priv", "https://reg.example/index/", "ROOT_T")];
        assert!(registro_para(&regs, "sparse+https://reg.example/index/").is_some());
        assert!(registro_para(&regs, "https://mal.reg.example/").is_none());
        assert!(registro_para(&regs, "https://reg.example.mal.com/").is_none());
        assert!(registro_para(&regs, "https://xreg.example/").is_none());
        assert!(registro_para(&regs, "http://reg.example/").is_none());
        // El puerto también cuenta: otro puerto del mismo host es otro servidor.
        assert!(registro_para(&regs, "https://reg.example:8443/index/").is_none());
        assert!(registro_para(&regs, "https://reg.example:443/index/").is_some());
    }

    #[test]
    fn credenciales_variable_cargo_va_en_mayusculas_y_con_guion_bajo() {
        assert_eq!(variable_cargo("priv"), "CARGO_REGISTRIES_PRIV_TOKEN");
        assert_eq!(variable_cargo("mi-reg"), "CARGO_REGISTRIES_MI_REG_TOKEN");
    }

    #[test]
    fn credenciales_debug_de_la_credencial_no_muestra_el_valor() {
        let texto = format!("{:?}", credencial_de_prueba());
        assert!(!texto.contains("valor-secreto-9"), "{texto}");
        assert!(texto.contains("***"), "{texto}");
        assert!(texto.contains("CARGO_REGISTRIES_PRIV_TOKEN"), "{texto}");
    }

    #[test]
    fn credenciales_redactar_secretos_oculta_el_valor_en_los_textos() {
        let cred = credencial_de_prueba();
        let texto = redactar_secretos(
            "error: 401 con valor-secreto-9 y otra vez valor-secreto-9",
            std::slice::from_ref(&cred),
        );
        assert!(!texto.contains("valor-secreto-9"), "{texto}");
        assert_eq!(texto, "error: 401 con *** y otra vez ***");
    }

    #[test]
    fn credenciales_repo_con_registro_en_otro_host_no_recibe_la_credencial() {
        let regs = vec![registro(
            "priv",
            "https://reg.example/index/",
            "ROOT_TEST_TOKEN",
        )];
        let leer = |nombre: &str| (nombre == "ROOT_TEST_TOKEN").then(|| "valor-9".to_string());
        let inv = inventario_con_registro("sparse+https://mal.example/index/");
        assert!(credenciales_cargo(&inv, &regs, leer).is_empty());

        let inv = inventario_con_registro("sparse+https://reg.example/index/");
        let creds = credenciales_cargo(&inv, &regs, leer);
        assert_eq!(creds.len(), 1, "{creds:?}");
        assert_eq!(creds[0].variable, "CARGO_REGISTRIES_PRIV_TOKEN");
    }

    #[test]
    fn credenciales_variable_ausente_o_vacia_no_da_credencial_y_no_es_error() {
        let regs = vec![registro(
            "priv",
            "https://reg.example/index/",
            "ROOT_TEST_TOKEN",
        )];
        let inv = inventario_con_registro("sparse+https://reg.example/index/");
        assert!(credenciales_cargo(&inv, &regs, |_| None).is_empty());
        assert!(credenciales_cargo(&inv, &regs, |_| Some(String::new())).is_empty());
    }

    #[test]
    fn credenciales_netrc_para_arma_machine_login_y_password() {
        assert_eq!(
            netrc_para("reg.example", "usuario", "clave-1"),
            "machine reg.example login usuario password clave-1\n"
        );
    }

    #[test]
    fn credenciales_netrc_sin_usuario_env_usa_token() {
        let sin_usuario = registro("priv", "https://reg.example/", "ROOT_TOKEN_PRIV");
        let usuario = usuario_de_pip(&sin_usuario, |_| Some("ignorada".into()));
        assert_eq!(usuario, "__token__");
        assert_eq!(
            netrc_para("reg.example", &usuario, "clave-1"),
            "machine reg.example login __token__ password clave-1\n"
        );
    }

    #[test]
    fn credenciales_netrc_con_usuario_env_usa_el_valor_de_esa_variable() {
        let mut con_usuario = registro("priv", "https://reg.example/", "ROOT_TOKEN_PRIV");
        con_usuario.usuario_env = Some("ROOT_USUARIO_PRIV".into());
        let usuario = usuario_de_pip(&con_usuario, |v| {
            (v == "ROOT_USUARIO_PRIV").then(|| "alice".to_string())
        });
        assert_eq!(usuario, "alice");
        // Si la variable no tiene valor, el usuario vuelve a `__token__`.
        assert_eq!(usuario_de_pip(&con_usuario, |_| None), "__token__");
    }

    #[test]
    fn credenciales_authfile_para_arma_el_json_de_auths_con_el_valor_tal_cual() {
        let texto = authfile_para("reg.example", "dG9rZW4=");
        assert_eq!(texto, r#"{"auths":{"reg.example":{"auth":"dG9rZW4="}}}"#);
        let v: serde_json::Value = serde_json::from_str(&texto).expect("JSON válido");
        assert_eq!(v["auths"]["reg.example"]["auth"], "dG9rZW4=");
    }
}
