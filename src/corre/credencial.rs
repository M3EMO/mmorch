//! Credencial: copia el login de Claude sin los tokens MCP y revisa que el login no venza.
//! Funciones puras sobre archivos. Ningún mensaje de error lleva el contenido ni los tokens.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::time::Duration;

use serde_json::{Map, Value};

use super::ErrorCorre;

/// Clave del login de Claude. Es la única que se copia.
const CLAVE_LOGIN: &str = "claudeAiOauth";

/// Mensaje cuando el login vence antes del plazo. No lleva tokens.
const MENSAJE_VENCE: &str = "el login de Claude vence antes del plazo de la corrida; renová el login en el host con `claude` y reintentá";

/// Copia `origen` a `destino` con solo la clave `claudeAiOauth`, sin `refreshToken`, con permisos 0600.
/// El destino previo se borra antes. Así el archivo nuevo nace con 0600.
pub fn copiar_sin_mcp(origen: &Path, destino: &Path) -> Result<(), ErrorCorre> {
    let json = leer_json(origen)?;
    let mut login = login_de(&json, origen)?.clone();
    // Sin refresco, un `claude` dentro del contenedor no puede rotar el login del host.
    if let Some(objeto) = login.as_object_mut() {
        objeto.remove("refreshToken");
    }
    let mut solo_login = Map::new();
    solo_login.insert(CLAVE_LOGIN.to_string(), login);
    let texto = serde_json::to_string_pretty(&Value::Object(solo_login)).map_err(|e| {
        ErrorCorre::Ejecutor(format!(
            "no se pudo armar la copia de {}: {e}",
            origen.display()
        ))
    })?;
    borrar_si_existe(destino)?;
    let mut archivo = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(destino)
        .map_err(|e| {
            ErrorCorre::Ejecutor(format!("no se pudo crear {}: {e}", destino.display()))
        })?;
    archivo
        .write_all(texto.as_bytes())
        .and_then(|()| archivo.sync_all())
        .map_err(|e| {
            ErrorCorre::Ejecutor(format!("no se pudo escribir {}: {e}", destino.display()))
        })
}

/// Lee `claudeAiOauth.expiresAt` en milisegundos desde la época.
pub fn vencimiento_ms(origen: &Path) -> Result<u64, ErrorCorre> {
    let json = leer_json(origen)?;
    let login = login_de(&json, origen)?;
    login
        .get("expiresAt")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            ErrorCorre::Ejecutor(format!(
                "falta expiresAt numérico en {CLAVE_LOGIN} de {}",
                origen.display()
            ))
        })
}

/// Falla si el login vence antes de `ahora_ms + plazo`, o justo en ese límite.
pub fn verificar_vigencia(origen: &Path, ahora_ms: u64, plazo: Duration) -> Result<(), ErrorCorre> {
    let vence = vencimiento_ms(origen)?;
    // u128 evita el desborde al sumar el plazo.
    let limite = u128::from(ahora_ms) + plazo.as_millis();
    if u128::from(vence) <= limite {
        return Err(ErrorCorre::Ejecutor(MENSAJE_VENCE.to_string()));
    }
    Ok(())
}

/// Lee y parsea el JSON de credenciales. El error nombra la ruta y nunca el contenido.
fn leer_json(origen: &Path) -> Result<Value, ErrorCorre> {
    let texto = fs::read_to_string(origen)
        .map_err(|e| ErrorCorre::Ejecutor(format!("no se pudo leer {}: {e}", origen.display())))?;
    serde_json::from_str(&texto)
        .map_err(|e| ErrorCorre::Ejecutor(format!("JSON inválido en {}: {e}", origen.display())))
}

/// Devuelve el objeto `claudeAiOauth`. Si falta, o no es objeto, es error.
fn login_de<'a>(json: &'a Value, origen: &Path) -> Result<&'a Value, ErrorCorre> {
    json.get(CLAVE_LOGIN)
        .filter(|login| login.is_object())
        .ok_or_else(|| ErrorCorre::Ejecutor(format!("falta {CLAVE_LOGIN} en {}", origen.display())))
}

/// Borra `ruta` si existe. Un enlace se borra como enlace, sin seguirlo.
fn borrar_si_existe(ruta: &Path) -> Result<(), ErrorCorre> {
    match fs::remove_file(ruta) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(ErrorCorre::Ejecutor(format!(
            "no se pudo reemplazar {}: {e}",
            ruta.display()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use tempfile::TempDir;

    const TOKEN_ACCESO: &str = "TOKEN_ACCESO_PRUEBA_1a2b";
    const TOKEN_REFRESCO: &str = "TOKEN_REFRESCO_PRUEBA_3c4d";
    const TOKEN_MCP: &str = "TOKEN_MCP_PRUEBA_5e6f";
    const VENCE_MS: u64 = 1_700_000_000_000;

    /// Credenciales de prueba con `claudeAiOauth` y `mcpOAuth`.
    fn credenciales_completas() -> String {
        format!(
            r#"{{"claudeAiOauth":{{"accessToken":"{TOKEN_ACCESO}","refreshToken":"{TOKEN_REFRESCO}","refreshTokenExpiresAt":{VENCE_MS},"expiresAt":{VENCE_MS},"scopes":["user:inference"]}},"mcpOAuth":{{"servidor":{{"accessToken":"{TOKEN_MCP}"}}}}}}"#
        )
    }

    /// Escribe `texto` en `nombre` dentro de `dir` y devuelve su ruta.
    fn escribir(dir: &TempDir, nombre: &str, texto: &str) -> PathBuf {
        let ruta = dir.path().join(nombre);
        fs::write(&ruta, texto).expect("escribe");
        ruta
    }

    /// Modo de permisos de `ruta`, sin los bits de tipo.
    fn modo(ruta: &Path) -> u32 {
        fs::metadata(ruta).expect("metadata").permissions().mode() & 0o777
    }

    /// Mensaje de un error `Ejecutor`. Cualquier otra variante hace fallar el test.
    fn mensaje_ejecutor(error: ErrorCorre) -> String {
        match error {
            ErrorCorre::Ejecutor(m) => m,
            otro => panic!("se esperaba Ejecutor, llegó {otro}"),
        }
    }

    #[test]
    fn corre_credencial_copia_sin_mcp_oauth() {
        let tmp = TempDir::new().expect("tmp");
        let origen = escribir(&tmp, "origen.json", &credenciales_completas());
        let destino = tmp.path().join("copia.json");
        copiar_sin_mcp(&origen, &destino).expect("copia");
        let copia = fs::read_to_string(&destino).expect("lee la copia");
        assert!(copia.contains("claudeAiOauth"), "{copia}");
        assert!(!copia.contains("mcpOAuth"), "{copia}");
        assert!(!copia.contains(TOKEN_MCP), "la copia trae el token MCP");
        let json: serde_json::Value = serde_json::from_str(&copia).expect("JSON válido");
        assert_eq!(json["claudeAiOauth"]["accessToken"], TOKEN_ACCESO);
    }

    #[test]
    fn corre_credencial_copia_sin_refresh_token() {
        let tmp = TempDir::new().expect("tmp");
        let origen = escribir(&tmp, "origen.json", &credenciales_completas());
        let destino = tmp.path().join("copia.json");
        copiar_sin_mcp(&origen, &destino).expect("copia");
        let copia = fs::read_to_string(&destino).expect("lee la copia");
        let json: serde_json::Value = serde_json::from_str(&copia).expect("JSON válido");
        // Se revisa la clave, no el texto: `refreshTokenExpiresAt` es una fecha y se conserva.
        let login = json["claudeAiOauth"].as_object().expect("objeto de login");
        assert!(!login.contains_key("refreshToken"), "{copia}");
        assert!(!copia.contains(TOKEN_REFRESCO), "la copia trae el refresco");
        assert_eq!(login["refreshTokenExpiresAt"], VENCE_MS);
        assert_eq!(json["claudeAiOauth"]["accessToken"], TOKEN_ACCESO);
        assert_eq!(json["claudeAiOauth"]["expiresAt"], VENCE_MS);
    }

    #[test]
    fn corre_credencial_copia_tiene_modo_0600() {
        let tmp = TempDir::new().expect("tmp");
        let origen = escribir(&tmp, "origen.json", &credenciales_completas());
        let destino = tmp.path().join("copia.json");
        copiar_sin_mcp(&origen, &destino).expect("copia");
        assert_eq!(modo(&destino), 0o600);
    }

    #[test]
    fn corre_credencial_reemplaza_destino_previo_con_otros_permisos() {
        let tmp = TempDir::new().expect("tmp");
        let origen = escribir(&tmp, "origen.json", &credenciales_completas());
        let destino = escribir(&tmp, "copia.json", "contenido viejo");
        fs::set_permissions(&destino, fs::Permissions::from_mode(0o644)).expect("permisos");
        copiar_sin_mcp(&origen, &destino).expect("copia");
        let copia = fs::read_to_string(&destino).expect("lee");
        assert!(!copia.contains("contenido viejo"), "{copia}");
        assert!(copia.contains("claudeAiOauth"), "{copia}");
        assert_eq!(modo(&destino), 0o600);
    }

    #[test]
    fn corre_credencial_sin_claude_ai_oauth_es_error() {
        let tmp = TempDir::new().expect("tmp");
        let texto = format!(r#"{{"mcpOAuth":{{"servidor":{{"accessToken":"{TOKEN_MCP}"}}}}}}"#);
        let origen = escribir(&tmp, "origen.json", &texto);
        let destino = tmp.path().join("copia.json");
        let error = copiar_sin_mcp(&origen, &destino).expect_err("debe fallar");
        let mensaje = mensaje_ejecutor(error);
        assert!(mensaje.contains("claudeAiOauth"), "{mensaje}");
        assert!(!mensaje.contains(TOKEN_MCP), "{mensaje}");
        assert!(!destino.exists(), "no debe crear el destino");
    }

    #[test]
    fn corre_credencial_json_invalido_es_error_sin_contenido() {
        let tmp = TempDir::new().expect("tmp");
        let texto = format!(r#"{{"claudeAiOauth":{{"accessToken":"{TOKEN_ACCESO}""#);
        let origen = escribir(&tmp, "origen.json", &texto);
        let destino = tmp.path().join("copia.json");
        let error = copiar_sin_mcp(&origen, &destino).expect_err("debe fallar");
        let mensaje = mensaje_ejecutor(error);
        assert!(!mensaje.contains(TOKEN_ACCESO), "{mensaje}");
        assert!(mensaje.contains("origen.json"), "{mensaje}");
    }

    #[test]
    fn corre_credencial_archivo_faltante_nombra_la_ruta_sin_tokens() {
        let tmp = TempDir::new().expect("tmp");
        let origen = tmp.path().join("no-existe.json");
        let destino = tmp.path().join("copia.json");
        let error = copiar_sin_mcp(&origen, &destino).expect_err("debe fallar");
        let mensaje = mensaje_ejecutor(error);
        assert!(mensaje.contains("no-existe.json"), "{mensaje}");
        assert!(!mensaje.contains(TOKEN_ACCESO), "{mensaje}");
    }

    #[test]
    fn corre_credencial_vencimiento_lee_expires_at() {
        let tmp = TempDir::new().expect("tmp");
        let origen = escribir(&tmp, "origen.json", &credenciales_completas());
        assert_eq!(vencimiento_ms(&origen).expect("lee"), VENCE_MS);
    }

    #[test]
    fn corre_credencial_vencimiento_sin_expires_at_es_error() {
        let tmp = TempDir::new().expect("tmp");
        let texto = format!(r#"{{"claudeAiOauth":{{"accessToken":"{TOKEN_ACCESO}"}}}}"#);
        let origen = escribir(&tmp, "origen.json", &texto);
        let mensaje = mensaje_ejecutor(vencimiento_ms(&origen).expect_err("debe fallar"));
        assert!(mensaje.contains("expiresAt"), "{mensaje}");
        assert!(!mensaje.contains(TOKEN_ACCESO), "{mensaje}");
    }

    /// Login que vence en `VENCE_MS`. Con `ahora = 1000` y plazo de 500 ms, el límite es 1500.
    fn origen_con_vencimiento(tmp: &TempDir, vence: u64) -> PathBuf {
        let texto = format!(
            r#"{{"claudeAiOauth":{{"accessToken":"{TOKEN_ACCESO}","expiresAt":{vence}}}}}"#
        );
        escribir(tmp, "origen.json", &texto)
    }

    #[test]
    fn corre_credencial_vigencia_falla_si_vence_antes_del_plazo() {
        let tmp = TempDir::new().expect("tmp");
        let origen = origen_con_vencimiento(&tmp, 1_400);
        let error = verificar_vigencia(&origen, 1_000, Duration::from_millis(500))
            .expect_err("debe fallar");
        assert!(matches!(error, ErrorCorre::Ejecutor(_)), "{error}");
    }

    #[test]
    fn corre_credencial_vigencia_falla_justo_en_el_limite() {
        let tmp = TempDir::new().expect("tmp");
        let origen = origen_con_vencimiento(&tmp, 1_500);
        let error = verificar_vigencia(&origen, 1_000, Duration::from_millis(500))
            .expect_err("el límite exacto debe fallar");
        assert!(matches!(error, ErrorCorre::Ejecutor(_)), "{error}");
    }

    #[test]
    fn corre_credencial_vigencia_pasa_si_vence_despues() {
        let tmp = TempDir::new().expect("tmp");
        let origen = origen_con_vencimiento(&tmp, 1_501);
        verificar_vigencia(&origen, 1_000, Duration::from_millis(500)).expect("vigente");
    }

    #[test]
    fn corre_credencial_mensaje_de_vigencia_no_trae_tokens() {
        let tmp = TempDir::new().expect("tmp");
        let origen = escribir(&tmp, "origen.json", &credenciales_completas());
        let mensaje = mensaje_ejecutor(
            verificar_vigencia(&origen, VENCE_MS, Duration::from_millis(1))
                .expect_err("vence en el límite"),
        );
        assert_eq!(
            mensaje,
            "el login de Claude vence antes del plazo de la corrida; renová el login en el host con `claude` y reintentá"
        );
        for token in [TOKEN_ACCESO, TOKEN_REFRESCO, TOKEN_MCP] {
            assert!(!mensaje.contains(token), "{mensaje}");
        }
    }
}
