//! Ejecutor del builder. Un `Lanzamiento` describe un `claude -p` de un rol y el ejecutor lo corre.
//! `EjecutorContenedorBuilder` lo corre en un contenedor de podman (decisión 43, Tarea 26, capa 1). Los
//! tests usan `pruebas::EjecutorFalso`, que corre `tests/fixtures/claude-falso.sh` y nunca llama a `claude`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::contenedor::{args_podman_builder, estado_de, fisica, validar_montajes};
use super::{ErrorCorre, copiar_sin_mcp, verificar_vigencia};
use crate::oracle::ErrorOraculo;
use crate::oracle::contenedor::{esperar, leer_en_hilo};
use crate::tipos::Rol;

/// Nombre de la copia de la credencial dentro del estado de `claude`. Es el que ve el builder.
const CREDENCIAL_COPIA: &str = ".credentials.json";

/// Un `claude -p` de un rol: sus argumentos, su entorno y el repo donde corre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lanzamiento {
    pub rol: Rol,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub repo: PathBuf,
}

/// Salida de un lanzamiento. Un código distinto de 0 no es error del ejecutor: lo decide el llamador.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SalidaClaude {
    pub codigo: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Corre el builder de un rol. Es el único punto por donde se lanza `claude`.
pub trait EjecutorBuilder {
    /// Corre el builder con `l` y devuelve su salida. Sale por `Err` solo si no pudo lanzarlo.
    fn lanzar(&self, l: &Lanzamiento) -> Result<SalidaClaude, ErrorCorre>;

    /// Revisa antes de armar nada que el login alcance para `plazo`. Por defecto no revisa nada.
    fn verificar_antes(&self, _plazo: Duration) -> Result<(), ErrorCorre> {
        Ok(())
    }
}

/// Ejecutor del contenedor del builder (decisión 43). Corre `claude` en un contenedor de podman. Nunca lo
/// corre en el host.
pub struct EjecutorContenedorBuilder {
    /// Imagen del builder: `IMAGEN_BUILDER` en la CLI.
    pub imagen: String,
    /// Credencial de Claude del host. Entra al contenedor en solo lectura.
    pub credencial: PathBuf,
    /// `ROOT_HOME`. Ningún montaje lo toca.
    pub home: PathBuf,
    /// `$HOME` del host. Ningún montaje es igual a él ni lo contiene.
    pub home_usuario: PathBuf,
    /// Tope de cada lanzamiento. Si vence, el contenedor se borra.
    pub timeout: Duration,
}

/// Código con que podman sale ante un error propio (imagen inexistente, opción inválida).
const CODIGO_ERROR_PODMAN: i32 = 125;

/// Contador de nombres de contenedor dentro del proceso. Con el pid, el nombre no choca con otra corrida.
static CONTADOR: AtomicU64 = AtomicU64::new(0);

impl EjecutorContenedorBuilder {
    /// Copia la credencial del host a `estado/.credentials.json` sin `mcpOAuth`, con 0600.
    /// Devuelve la ruta de la copia. Es lo único que el contenedor ve como credencial.
    pub fn preparar_credencial(&self, estado: &Path) -> Result<PathBuf, ErrorCorre> {
        let copia = estado.join(CREDENCIAL_COPIA);
        copiar_sin_mcp(&self.credencial, &copia)?;
        Ok(copia)
    }
}

impl EjecutorBuilder for EjecutorContenedorBuilder {
    fn lanzar(&self, l: &Lanzamiento) -> Result<SalidaClaude, ErrorCorre> {
        // La credencial va primero: sin ella no se valida, no se crea nada y no se busca podman.
        revisar_credencial(&self.credencial)?;

        // El estado de claude va al lado del repo. Todo montaje se valida antes de crear nada.
        let estado = estado_de(&l.repo).ok_or_else(|| {
            ErrorCorre::Ejecutor(format!(
                "el repo de la corrida no tiene carpeta padre: {}",
                l.repo.display()
            ))
        })?;
        validar_montajes(
            &self.home,
            &self.home_usuario,
            &[
                ("repo", &l.repo),
                ("estado", &estado),
                ("credencial", &self.credencial),
            ],
        )?;
        fs::create_dir_all(&estado).map_err(|fuente| ErrorCorre::Io {
            contexto: format!("crear el estado de claude en {}", estado.display()),
            fuente,
        })?;
        // El builder no ve la credencial del usuario: monta una copia sin `mcpOAuth`.
        let copia = self.preparar_credencial(&estado)?;

        // Los montajes van por ruta física: lo que se validó es lo que se monta.
        let mut real = l.clone();
        real.repo = fisica(&l.repo, "el repo")?;
        let estado = fisica(&estado, "el estado")?;
        let credencial = fisica(&copia, "la copia de la credencial")?;
        let nombre = nombre_unico();
        let args = args_podman_builder(&real, &self.imagen, &credencial, &estado, &nombre);

        match correr_proceso("podman", &args, self.timeout)? {
            Some(salida) if salida.codigo == CODIGO_ERROR_PODMAN => {
                Err(ErrorCorre::Ejecutor(format!(
                    "podman falló (125): {}",
                    String::from_utf8_lossy(&salida.stderr).trim()
                )))
            }
            Some(salida) => Ok(salida),
            None => {
                // Venció: se pide borrar el contenedor por nombre. Un fallo al borrar no tapa el aviso.
                let _ = Command::new("podman")
                    .args(["rm", "-f", "-t", "0", &nombre])
                    .output();
                Err(ErrorCorre::Ejecutor(format!(
                    "timeout tras {} s; se pidió borrar el contenedor {nombre}",
                    self.timeout.as_secs()
                )))
            }
        }
    }

    fn verificar_antes(&self, plazo: Duration) -> Result<(), ErrorCorre> {
        // Mismo chequeo de existencia que `lanzar`: la falta de credencial tiene un solo mensaje.
        revisar_credencial(&self.credencial)?;
        verificar_vigencia(&self.credencial, ahora_ms(), plazo)
    }
}

/// Milisegundos desde la época. Reloj anterior a 1970 cuenta como 0.
fn ahora_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// Revisa que la credencial de Claude sea un archivo. El mensaje nombra solo la ruta, nunca el contenido.
fn revisar_credencial(credencial: &Path) -> Result<(), ErrorCorre> {
    match fs::metadata(credencial) {
        Ok(meta) if meta.is_file() => Ok(()),
        Ok(_) => Err(ErrorCorre::Ejecutor(format!(
            "la credencial de Claude no es un archivo: {}",
            credencial.display()
        ))),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Err(ErrorCorre::Ejecutor(format!(
            "no existe la credencial de Claude en {}",
            credencial.display()
        ))),
        Err(_) => Err(ErrorCorre::Ejecutor(format!(
            "no se pudo leer la credencial de Claude en {}",
            credencial.display()
        ))),
    }
}

/// Nombre único de un contenedor del builder: `root-builder-<pid>-<contador>`.
fn nombre_unico() -> String {
    let n = CONTADOR.fetch_add(1, Ordering::Relaxed);
    format!("root-builder-{}-{n}", std::process::id())
}

/// Corre `programa` con `args` y espera hasta `plazo`. Da `None` si venció el plazo: el proceso ya murió.
/// Un fallo al lanzar o al esperar es `Ejecutor`. El código de salida lo decide el llamador.
fn correr_proceso(
    programa: &str,
    args: &[String],
    plazo: Duration,
) -> Result<Option<SalidaClaude>, ErrorCorre> {
    let mut hijo = Command::new(programa)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            ErrorCorre::Ejecutor(format!(
                "no se pudo lanzar {programa} (¿está instalado y en PATH?): {e}"
            ))
        })?;
    let salida = hijo.stdout.take().map(leer_en_hilo);
    let error = hijo.stderr.take().map(leer_en_hilo);
    let estado = esperar(&mut hijo, plazo).map_err(|e| {
        ErrorCorre::Ejecutor(match e {
            ErrorOraculo::Infra(m) => m,
            otro => otro.to_string(),
        })
    })?;
    let Some(estado) = estado else {
        let _ = hijo.kill();
        let _ = hijo.wait();
        return Ok(None);
    };
    let Some(codigo) = estado.code() else {
        return Err(ErrorCorre::Ejecutor(format!(
            "{programa} terminó por una señal"
        )));
    };
    Ok(Some(SalidaClaude {
        codigo,
        stdout: bytes_de(salida),
        stderr: bytes_de(error),
    }))
}

/// Bytes de un flujo leído en otro hilo. Sin flujo o con un hilo caído, vacío.
fn bytes_de(hilo: Option<JoinHandle<Vec<u8>>>) -> Vec<u8> {
    hilo.and_then(|h| h.join().ok()).unwrap_or_default()
}

#[cfg(test)]
pub(crate) mod pruebas {
    use std::path::PathBuf;
    use std::process::Command;

    use super::*;

    /// Script que hace de `claude`: emite `stream-json` y crea `hecho.txt` en su cwd.
    const SCRIPT: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/claude-falso.sh"
    );

    /// Ejecutor falso: corre el script con `sh`, con el entorno exacto de `l.env` (y `PATH`).
    pub(crate) struct EjecutorFalso;

    impl EjecutorBuilder for EjecutorFalso {
        fn lanzar(&self, l: &Lanzamiento) -> Result<SalidaClaude, ErrorCorre> {
            let mut cmd = Command::new("sh");
            cmd.arg(PathBuf::from(SCRIPT))
                .args(&l.args)
                .env_clear()
                .envs(l.env.iter().map(|(k, v)| (k, v)))
                .current_dir(&l.repo);
            // El builder no hereda el entorno del proceso de test: solo `PATH` y lo que pasa el llamador.
            if let Some(path) = std::env::var_os("PATH") {
                cmd.env("PATH", path);
            }
            let salida = cmd
                .output()
                .map_err(|e| ErrorCorre::Ejecutor(format!("builder falso: {e}")))?;
            Ok(SalidaClaude {
                codigo: salida.status.code().unwrap_or(-1),
                stdout: salida.stdout,
                stderr: salida.stderr,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::pruebas::EjecutorFalso;
    use super::*;
    use crate::observe::transcript::leer_transcript;
    use std::fs;
    use tempfile::TempDir;

    fn lanzamiento(repo: PathBuf, env: Vec<(String, String)>) -> Lanzamiento {
        Lanzamiento {
            rol: Rol::Construye,
            args: ["-p", "haz hecho", "--model", "claude-haiku-5-5"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            env,
            repo,
        }
    }

    #[test]
    fn corre_ejecutor_falso_emite_stream_json_y_crea_hecho() {
        let tmp = TempDir::new().expect("tmp");
        let repo = tmp.path().join("repo");
        fs::create_dir_all(&repo).expect("repo");
        let salida = EjecutorFalso
            .lanzar(&lanzamiento(repo.clone(), Vec::new()))
            .expect("lanza");
        assert_eq!(
            salida.codigo,
            0,
            "{}",
            String::from_utf8_lossy(&salida.stderr)
        );
        assert!(
            repo.join("hecho.txt").is_file(),
            "el builder no creó hecho.txt"
        );

        let texto = String::from_utf8(salida.stdout.clone()).expect("utf8");
        let lineas: Vec<serde_json::Value> = texto
            .lines()
            .map(|l| serde_json::from_str(l).expect("cada línea es JSON"))
            .collect();
        assert_eq!(lineas.first().expect("init")["type"], "system");
        assert_eq!(lineas.first().expect("init")["model"], "claude-haiku-5-5");
        let fin = lineas.last().expect("result");
        assert_eq!(fin["type"], "result");
        assert_eq!(fin["subtype"], "success");
        assert_eq!(fin["is_error"], false);

        // El transcript del builder lo lee el lector de observe.
        let ruta = tmp.path().join("salida.jsonl");
        fs::write(&ruta, &salida.stdout).expect("escribe");
        let transcript = leer_transcript(&ruta).expect("lee");
        let cwd = fs::canonicalize(&repo).expect("canonica");
        assert_eq!(transcript.cwd.as_deref(), Some(cwd.as_path()));
        assert_eq!(transcript.llamadas.len(), 1);
        assert_eq!(transcript.llamadas[0].nombre, "Write");
        assert_eq!(transcript.llamadas[0].salida, "ok");
    }

    /// Ejecutor real con `$HOME` y `ROOT_HOME` bajo `tmp`. La credencial se crea solo si `con_credencial`.
    fn ejecutor_real(tmp: &TempDir, con_credencial: bool) -> (EjecutorContenedorBuilder, PathBuf) {
        let usuario = tmp.path().join("usuario");
        let credencial = usuario.join(".claude").join(".credentials.json");
        if con_credencial {
            fs::create_dir_all(credencial.parent().expect("padre")).expect("claude");
            fs::write(&credencial, "{\"token\":\"secreto-de-prueba\"}").expect("credencial");
        }
        let ejecutor = EjecutorContenedorBuilder {
            imagen: "root-builder:1".into(),
            credencial: credencial.clone(),
            home: tmp.path().join("root"),
            home_usuario: usuario,
            timeout: Duration::from_secs(60),
        };
        (ejecutor, credencial)
    }

    #[test]
    fn corre_contenedor_sin_credencial_no_lanza_y_nombra_la_ruta() {
        // Sin credencial en el host, el error sale antes de buscar o correr podman.
        let tmp = TempDir::new().expect("tmp");
        let (ejecutor, credencial) = ejecutor_real(&tmp, false);
        let repo = tmp.path().join("usuario/.cache/root/corridas/corre-1/repo");
        let error = ejecutor
            .lanzar(&lanzamiento(repo, Vec::new()))
            .expect_err("sin credencial no hay lanzamiento");
        let esperado = format!(
            "no existe la credencial de Claude en {}",
            credencial.display()
        );
        assert!(
            matches!(error, ErrorCorre::Ejecutor(ref m) if *m == esperado),
            "{error}"
        );
    }

    #[test]
    fn corre_contenedor_repo_dentro_de_root_home_no_lanza() {
        let tmp = TempDir::new().expect("tmp");
        let (ejecutor, _) = ejecutor_real(&tmp, true);
        let repo = tmp.path().join("root").join("corrida").join("repo");
        let error = ejecutor
            .lanzar(&lanzamiento(repo, Vec::new()))
            .expect_err("un repo dentro de ROOT_HOME no se lanza");
        assert!(
            matches!(error, ErrorCorre::Ejecutor(ref m) if m.contains("toca ROOT_HOME")),
            "{error}"
        );
        assert!(
            !tmp.path().join("root").join("corrida").exists(),
            "no debe crear la carpeta del repo dentro de ROOT_HOME"
        );
    }

    #[test]
    fn corre_proceso_devuelve_codigo_y_salida() {
        let args = vec![
            "-c".to_string(),
            "echo hola; echo mal >&2; exit 4".to_string(),
        ];
        let salida = correr_proceso("sh", &args, Duration::from_secs(20))
            .expect("corre")
            .expect("no venció");
        assert_eq!(salida.codigo, 4);
        assert_eq!(salida.stdout, b"hola\n".to_vec());
        assert_eq!(salida.stderr, b"mal\n".to_vec());
    }

    #[test]
    fn corre_proceso_vence_el_plazo_y_da_none() {
        let args = vec!["-c".to_string(), "sleep 5".to_string()];
        let inicio = std::time::Instant::now();
        let salida = correr_proceso("sh", &args, Duration::from_millis(100)).expect("corre");
        assert!(salida.is_none(), "debe vencer");
        assert!(
            inicio.elapsed() < Duration::from_secs(4),
            "no espera el fin del proceso"
        );
    }

    #[test]
    fn corre_proceso_sin_programa_es_error_de_infraestructura() {
        let error = correr_proceso(
            "root-programa-inexistente-7c1e",
            &[],
            Duration::from_secs(20),
        )
        .expect_err("el programa no existe");
        assert!(
            matches!(error, ErrorCorre::Ejecutor(ref m) if m.contains("no se pudo lanzar")),
            "{error}"
        );
    }

    /// Credencial de prueba con `claudeAiOauth` y `mcpOAuth`. Ningún token es real.
    fn credencial_con_mcp(vence_ms: u64) -> String {
        format!(
            r#"{{"claudeAiOauth":{{"accessToken":"TOKEN_ACCESO_EJ","expiresAt":{vence_ms}}},"mcpOAuth":{{"servidor":{{"accessToken":"TOKEN_MCP_EJ"}}}}}}"#
        )
    }

    /// Vencimiento lejano del login de prueba: año 2100, en milisegundos.
    const VENCE_LEJOS_MS: u64 = 4_102_444_800_000;

    /// Escribe la credencial del usuario con `texto`, creando su carpeta.
    fn escribir_credencial(credencial: &Path, texto: &str) {
        fs::create_dir_all(credencial.parent().expect("padre")).expect("claude");
        fs::write(credencial, texto).expect("credencial");
    }

    #[test]
    fn corre_contenedor_prepara_copia_sin_mcp_oauth_con_0600() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = TempDir::new().expect("tmp");
        let (ejecutor, credencial) = ejecutor_real(&tmp, false);
        escribir_credencial(&credencial, &credencial_con_mcp(VENCE_LEJOS_MS));
        let estado = tmp.path().join("corrida").join("claude-estado");
        fs::create_dir_all(&estado).expect("estado");
        let copia = ejecutor
            .preparar_credencial(&estado)
            .expect("prepara la copia");
        assert!(copia.is_file(), "no existe la copia: {}", copia.display());
        assert_eq!(copia, estado.join(".credentials.json"));
        let texto = fs::read_to_string(&copia).expect("lee la copia");
        assert!(texto.contains("claudeAiOauth"), "{texto}");
        assert!(
            !texto.contains("mcpOAuth"),
            "la copia trae mcpOAuth: {texto}"
        );
        assert!(
            !texto.contains("TOKEN_MCP_EJ"),
            "la copia trae el token MCP"
        );
        let modo = fs::metadata(&copia).expect("meta").permissions().mode() & 0o777;
        assert_eq!(modo, 0o600);
    }

    #[test]
    fn corre_contenedor_verificar_antes_con_login_vencido_es_error_de_ejecutor() {
        let tmp = TempDir::new().expect("tmp");
        let (ejecutor, credencial) = ejecutor_real(&tmp, false);
        escribir_credencial(&credencial, &credencial_con_mcp(1_000));
        let error = ejecutor
            .verificar_antes(Duration::from_secs(60))
            .expect_err("el login vence antes del plazo");
        assert!(matches!(error, ErrorCorre::Ejecutor(_)), "{error}");
        assert!(
            !error.to_string().contains("TOKEN_ACCESO_EJ"),
            "el mensaje no lleva tokens: {error}"
        );
    }

    #[test]
    fn corre_contenedor_verificar_antes_con_login_lejano_da_ok() {
        let tmp = TempDir::new().expect("tmp");
        let (ejecutor, credencial) = ejecutor_real(&tmp, false);
        escribir_credencial(&credencial, &credencial_con_mcp(VENCE_LEJOS_MS));
        ejecutor
            .verificar_antes(Duration::from_secs(60))
            .expect("el login alcanza");
    }

    #[test]
    fn corre_ejecutor_falso_sale_9_con_clave_o_root_home_en_el_entorno() {
        let tmp = TempDir::new().expect("tmp");
        let repo = tmp.path().join("repo");
        fs::create_dir_all(&repo).expect("repo");
        for (clave, valor) in [
            ("ROOT_HOME", "/datos/root"),
            ("DEEPSEEK_API_KEY", "clave"),
            ("ANTHROPIC_API_KEY", "clave"),
        ] {
            let env = vec![(clave.to_string(), valor.to_string())];
            let salida = EjecutorFalso
                .lanzar(&lanzamiento(repo.clone(), env))
                .expect("lanza");
            assert_eq!(salida.codigo, 9, "{clave} debe cortar el builder");
        }
    }
}
