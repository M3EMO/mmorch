//! Redacción: quita el `accessToken` de la copia de la credencial del stdout de `claude` antes de
//! guardar un transcript. Funciones puras. Ningún mensaje lleva el token ni el contenido de la copia.

use std::fs;
use std::path::Path;

use serde_json::Value;

/// Texto que reemplaza al token en el transcript.
pub const MARCA: &str = "[REDACTADO]";

/// Largo mínimo del token en bytes. Un token más corto no se reemplaza: su texto podría aparecer en otro lado.
const LARGO_MINIMO: usize = 8;

/// `accessToken` de la copia de la credencial. Da `None` si la copia no existe, no es JSON, o no hay un
/// `claudeAiOauth.accessToken` string no vacío. Nunca devuelve un mensaje con contenido.
pub fn token_de_copia(copia: &Path) -> Option<String> {
    let texto = fs::read_to_string(copia).ok()?;
    let valor: Value = serde_json::from_str(&texto).ok()?;
    valor
        .get("claudeAiOauth")?
        .get("accessToken")?
        .as_str()
        .filter(|token| !token.is_empty())
        .map(str::to_string)
}

/// Copia de `stdout` con cada aparición del `token` cambiada por `MARCA`. Busca el token crudo y sus formas
/// escapadas en JSON, porque puede aparecer dentro de un string. Un token más corto que `LARGO_MINIMO` no
/// se reemplaza.
pub fn redactar(stdout: &[u8], token: &str) -> Vec<u8> {
    if token.len() < LARGO_MINIMO {
        return stdout.to_vec();
    }
    let mut limpio = stdout.to_vec();
    for forma in formas_del_token(token) {
        limpio = reemplazar(&limpio, forma.as_bytes());
    }
    limpio
}

/// Formas en que el token puede aparecer en el stdout: la cruda, la escapada como `serde_json` y la
/// escapada con `\/`, que otros encoders usan. Sin repetidas.
fn formas_del_token(token: &str) -> Vec<String> {
    let mut formas = vec![token.to_string()];
    // `to_string` de un string da comillas: se quitan para quedar con el texto escapado.
    let escapado = serde_json::to_string(token).unwrap_or_default();
    let escapado = escapado
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or_default()
        .to_string();
    let con_barra = escapado.replace('/', "\\/");
    for forma in [escapado, con_barra] {
        if !formas.contains(&forma) {
            formas.push(forma);
        }
    }
    formas
}

/// Copia de `texto` con cada aparición de `patron` cambiada por `MARCA`. `patron` no debe ser vacío.
fn reemplazar(texto: &[u8], patron: &[u8]) -> Vec<u8> {
    let mut salida = Vec::with_capacity(texto.len());
    let mut i = 0;
    while i < texto.len() {
        if texto[i..].starts_with(patron) {
            salida.extend_from_slice(MARCA.as_bytes());
            i += patron.len();
        } else {
            salida.push(texto[i]);
            i += 1;
        }
    }
    salida
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    const TOKEN: &str = "TOKEN_FALSO_PRUEBA_ABC";

    #[test]
    fn redacta_reemplaza_todas_las_apariciones() {
        let stdout = format!("uno {TOKEN} dos {TOKEN}\nel tres {TOKEN}\n");
        let limpio = redactar(stdout.as_bytes(), TOKEN);
        let texto = String::from_utf8(limpio).expect("utf8");
        assert_eq!(texto, format!("uno {MARCA} dos {MARCA}\nel tres {MARCA}\n"));
    }

    #[test]
    fn redacta_reemplaza_el_token_dentro_de_un_string_json() {
        let stdout = format!(
            "{{\"type\":\"user\",\"message\":{{\"content\":[{{\"type\":\"tool_result\",\"content\":\"lee -> {TOKEN}\"}}]}}}}\n"
        );
        let texto = String::from_utf8(redactar(stdout.as_bytes(), TOKEN)).expect("utf8");
        assert!(!texto.contains(TOKEN), "{texto}");
        assert!(texto.contains(MARCA), "{texto}");
        let valor: serde_json::Value =
            serde_json::from_str(texto.trim()).expect("sigue siendo JSON");
        assert_eq!(
            valor["message"]["content"][0]["content"],
            format!("lee -> {MARCA}")
        );
    }

    #[test]
    fn redacta_cubre_la_forma_escapada_en_json() {
        // Con comilla y barra: serde_json la escapa como `\"`. Otro encoder escapa `/` como `\/`.
        let token = "TOKEN_FALSO/PRUEBA\"ABC";
        let escapado = "{\"texto\":\"TOKEN_FALSO/PRUEBA\\\"ABC\"}\n";
        let texto = String::from_utf8(redactar(escapado.as_bytes(), token)).expect("utf8");
        assert_eq!(texto, format!("{{\"texto\":\"{MARCA}\"}}\n"));

        let con_barra = "{\"texto\":\"TOKEN_FALSO\\/PRUEBA\"}\n";
        let token_barra = "TOKEN_FALSO/PRUEBA";
        let texto = String::from_utf8(redactar(con_barra.as_bytes(), token_barra)).expect("utf8");
        assert_eq!(texto, format!("{{\"texto\":\"{MARCA}\"}}\n"));
    }

    #[test]
    fn redacta_token_corto_no_reemplaza() {
        let stdout = b"abc corto abc\n";
        assert_eq!(redactar(stdout, "abc"), stdout.to_vec());
        assert_eq!(redactar(stdout, ""), stdout.to_vec());
    }

    #[test]
    fn redacta_sin_token_en_el_texto_deja_todo_igual() {
        let stdout = b"{\"type\":\"result\"}\n";
        assert_eq!(redactar(stdout, TOKEN), stdout.to_vec());
    }

    #[test]
    fn token_de_copia_lee_el_access_token() {
        let tmp = TempDir::new().expect("tmp");
        let copia = tmp.path().join(".credentials.json");
        fs::write(
            &copia,
            format!("{{\"claudeAiOauth\":{{\"accessToken\":\"{TOKEN}\",\"expiresAt\":1}}}}"),
        )
        .expect("copia");
        assert_eq!(token_de_copia(&copia), Some(TOKEN.to_string()));
    }

    #[test]
    fn token_de_copia_inexistente_es_none() {
        let tmp = TempDir::new().expect("tmp");
        assert_eq!(token_de_copia(&tmp.path().join("no-existe.json")), None);
    }

    #[test]
    fn token_de_copia_sin_token_legible_es_none() {
        let tmp = TempDir::new().expect("tmp");
        let casos = [
            "",
            "no es json",
            "{\"token\":\"otro\"}",
            "{\"claudeAiOauth\":{\"accessToken\":\"\"}}",
            "{\"claudeAiOauth\":{\"accessToken\":42}}",
        ];
        for (i, texto) in casos.iter().enumerate() {
            let copia = tmp.path().join(format!("copia-{i}.json"));
            fs::write(&copia, texto).expect("copia");
            assert_eq!(token_de_copia(&copia), None, "caso {i}");
        }
    }
}
