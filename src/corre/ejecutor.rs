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
use super::{ErrorCorre, copiar_sin_mcp, verificar_vigencia, vida_restante};
use crate::oracle::ErrorOraculo;
use crate::oracle::contenedor::{esperar, leer_en_hilo};
use crate::tipos::Rol;

/// Nombre de la copia de la credencial dentro del estado de `claude`. Es el que ve el builder.
const CREDENCIAL_COPIA: &str = ".credentials.json";

/// Un `claude -p` de un rol: sus argumentos, su entorno, el repo donde corre y el plugin de ECC del rol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lanzamiento {
    pub rol: Rol,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub repo: PathBuf,
    /// Plugin de ECC del rol, en el host. Ausente si el rol no lo trae.
    pub plugin: Option<PathBuf>,
    /// Plazo de este rol. Lo fija `correr` según la vida del login. El ejecutor no lo supera: aplica
    /// el menor entre este plazo y su propio tope.
    pub plazo: Duration,
}

/// Salida de un lanzamiento. Un código distinto de 0 no es error del ejecutor: lo decide el llamador.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SalidaClaude {
    pub codigo: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Cómo espera `correr` a que el login dure lo mínimo para arrancar un rol. Revisa cada `intervalo`, hasta `tope`.
#[derive(Debug, Clone, Copy)]
pub struct EsperaVigencia {
    pub intervalo: Duration,
    pub tope: Duration,
}

impl Default for EsperaVigencia {
    fn default() -> Self {
        Self {
            intervalo: Duration::from_secs(60),
            tope: Duration::from_secs(30 * 60),
        }
    }
}

impl EsperaVigencia {
    /// Reintentos máximos: `ceil(tope / intervalo)`. Con intervalo cero no hay reintentos.
    pub fn reintentos(&self) -> u32 {
        if self.intervalo.is_zero() {
            return 0;
        }
        let veces = self.tope.as_nanos().div_ceil(self.intervalo.as_nanos());
        u32::try_from(veces).unwrap_or(u32::MAX)
    }
}

/// Corre el builder de un rol. Es el único punto por donde se lanza `claude`.
pub trait EjecutorBuilder {
    /// Corre el builder con `l` y devuelve su salida. Sale por `Err` solo si no pudo lanzarlo.
    fn lanzar(&self, l: &Lanzamiento) -> Result<SalidaClaude, ErrorCorre>;

    /// Revisa que el login dure al menos `plazo`. Por defecto no revisa nada.
    fn verificar_antes(&self, _plazo: Duration) -> Result<(), ErrorCorre> {
        Ok(())
    }

    /// Cuánto espera `correr` a que el login dure lo mínimo para arrancar un rol. Por defecto, `EsperaVigencia::default()`.
    fn espera_vigencia(&self) -> EsperaVigencia {
        EsperaVigencia::default()
    }

    /// Vida que le queda al login. `None` si el ejecutor no conoce ningún límite.
    fn vida_restante(&self) -> Result<Option<Duration>, ErrorCorre> {
        Ok(None)
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
    /// Tope de cada lanzamiento. Si vence, el contenedor se borra. Un rol corre con el menor entre
    /// su `plazo` y este tope.
    pub timeout: Duration,
}

/// Código con que podman avisa que mató el contenedor con SIGKILL. El stdout del builder sigue válido.
const CODIGO_SIGKILL: i32 = 137;

/// Motivo de un contenedor matado con SIGKILL. Lo lee quien llama junto con el stdout conservado.
const MENSAJE_SIGKILL: &str = "el contenedor fue matado por SIGKILL, a menudo por falta de memoria";

/// Códigos con que podman sale por un fallo de infraestructura. Ninguno es un resultado del builder.
/// El 137 no va aquí: un SIGKILL conserva el stdout (ver `clasificar_salida`).
const CODIGOS_PODMAN_INFRA: [(i32, &str); 3] = [
    (
        125,
        "error propio de podman: imagen inexistente u opción inválida",
    ),
    (126, "el comando no es ejecutable"),
    (127, "el comando no se encontró"),
];

/// Significado del código de podman si es fallo de infraestructura. `None` si no lo es.
fn significado_codigo_podman(codigo: i32) -> Option<&'static str> {
    CODIGOS_PODMAN_INFRA
        .iter()
        .find(|(c, _)| *c == codigo)
        .map(|(_, significado)| *significado)
}

/// Error del ejecutor si podman falló por infraestructura. `None` si la salida es de claude.
/// Nombra el código y su significado. El `stderr` de podman va al final.
fn fallo_de_podman(salida: &SalidaClaude) -> Option<ErrorCorre> {
    let significado = significado_codigo_podman(salida.codigo)?;
    Some(ErrorCorre::Ejecutor(format!(
        "podman falló ({}, {significado}): {}",
        salida.codigo,
        String::from_utf8_lossy(&salida.stderr).trim()
    )))
}

/// Decide qué hacer con la salida de podman. Función pura: no lanza nada.
/// Un 137 devuelve `Cortado` con el stdout intacto, para que quien llama guarde el transcript.
/// Los códigos 125, 126 y 127 son fallos de infraestructura: dan `Ejecutor`.
/// Cualquier otro código es de claude y pasa como `Ok`.
fn clasificar_salida(salida: SalidaClaude) -> Result<SalidaClaude, ErrorCorre> {
    if salida.codigo == CODIGO_SIGKILL {
        let detalle = String::from_utf8_lossy(&salida.stderr).trim().to_string();
        let mensaje = if detalle.is_empty() {
            MENSAJE_SIGKILL.to_string()
        } else {
            format!("{MENSAJE_SIGKILL}: {detalle}")
        };
        return Err(ErrorCorre::Cortado {
            mensaje,
            stdout: salida.stdout,
        });
    }
    match fallo_de_podman(&salida) {
        Some(error) => Err(error),
        None => Ok(salida),
    }
}

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
        if let Some(plugin) = &l.plugin {
            revisar_plugin(plugin)?;
        }

        // El estado de claude va al lado del repo. Todo montaje se valida antes de crear nada.
        let estado = estado_de(&l.repo).ok_or_else(|| {
            ErrorCorre::Ejecutor(format!(
                "el repo de la corrida no tiene carpeta padre: {}",
                l.repo.display()
            ))
        })?;
        let mut montajes: Vec<(&str, &Path)> = vec![
            ("repo", l.repo.as_path()),
            ("estado", estado.as_path()),
            ("credencial", self.credencial.as_path()),
        ];
        if let Some(plugin) = &l.plugin {
            montajes.push(("plugin", plugin.as_path()));
        }
        validar_montajes(&self.home, &self.home_usuario, &montajes)?;
        fs::create_dir_all(&estado).map_err(|fuente| ErrorCorre::Io {
            contexto: format!("crear el estado de claude en {}", estado.display()),
            fuente,
        })?;
        // El builder no ve la credencial del usuario: monta una copia sin `mcpOAuth`.
        let copia = self.preparar_credencial(&estado)?;

        // Los montajes van por ruta física: lo que se validó es lo que se monta.
        let mut real = l.clone();
        real.repo = fisica(&l.repo, "el repo")?;
        real.plugin = l
            .plugin
            .as_deref()
            .map(|plugin| fisica(plugin, "el plugin"))
            .transpose()?;
        let estado = fisica(&estado, "el estado")?;
        let credencial = fisica(&copia, "la copia de la credencial")?;
        let nombre = nombre_unico();
        let args = args_podman_builder(&real, &self.imagen, &credencial, &estado, &nombre);

        // El plazo de este rol no supera el tope del ejecutor.
        let plazo = l.plazo.min(self.timeout);
        match correr_proceso("podman", &args, plazo)? {
            Fin::Termino(salida) => clasificar_salida(salida),
            Fin::Vencio { stdout, .. } => {
                // Venció: se pide borrar el contenedor por nombre. Un fallo al borrar no tapa el aviso.
                let _ = Command::new("podman")
                    .args(["rm", "-f", "-t", "0", &nombre])
                    .output();
                // Lo que alcanzó a dar el builder sigue en `stdout`: quien llama lo guarda y lo cuenta.
                Err(ErrorCorre::Cortado {
                    mensaje: format!(
                        "timeout tras {} s; se pidió borrar el contenedor {nombre}",
                        plazo.as_secs()
                    ),
                    stdout,
                })
            }
        }
    }

    fn verificar_antes(&self, plazo: Duration) -> Result<(), ErrorCorre> {
        // Mismo chequeo de existencia que `lanzar`: la falta de credencial tiene un solo mensaje.
        revisar_credencial(&self.credencial)?;
        verificar_vigencia(&self.credencial, ahora_ms(), plazo)
    }

    /// Revisa la credencial igual que `verificar_antes` y devuelve la vida que le queda al login.
    fn vida_restante(&self) -> Result<Option<Duration>, ErrorCorre> {
        revisar_credencial(&self.credencial)?;
        vida_restante(&self.credencial, ahora_ms()).map(Some)
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

/// Revisa que el plugin de ECC del rol sea una carpeta. El mensaje nombra solo la ruta.
fn revisar_plugin(plugin: &Path) -> Result<(), ErrorCorre> {
    match fs::metadata(plugin) {
        Ok(meta) if meta.is_dir() => Ok(()),
        Ok(_) => Err(ErrorCorre::Ejecutor(format!(
            "el plugin de ECC no es una carpeta: {}",
            plugin.display()
        ))),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Err(ErrorCorre::Ejecutor(format!(
            "no existe el plugin de ECC en {}",
            plugin.display()
        ))),
        Err(_) => Err(ErrorCorre::Ejecutor(format!(
            "no se pudo leer el plugin de ECC en {}",
            plugin.display()
        ))),
    }
}

/// Nombre único de un contenedor del builder: `root-builder-<pid>-<contador>`.
fn nombre_unico() -> String {
    let n = CONTADOR.fetch_add(1, Ordering::Relaxed);
    format!("root-builder-{}-{n}", std::process::id())
}

/// Fin de un proceso: terminó solo, o venció el plazo y se mató.
#[derive(Debug, PartialEq, Eq)]
enum Fin {
    Termino(SalidaClaude),
    /// Lo leído de `stdout` y `stderr` hasta el corte. Puede faltar si un nieto retiene la pipe.
    Vencio {
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
}

/// Cuánto espera la lectura de una pipe después de matar el proceso. Pasado ese plazo, se deja de leer.
const GRACIA_LECTURA: Duration = Duration::from_secs(2);

/// Corre `programa` con `args` y espera hasta `plazo`. Si vence, mata el proceso y recoge lo leído hasta
/// ahí. Un fallo al lanzar o al esperar es `Ejecutor`. El código de salida lo decide el llamador.
fn correr_proceso(programa: &str, args: &[String], plazo: Duration) -> Result<Fin, ErrorCorre> {
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
        return Ok(Fin::Vencio {
            stdout: bytes_tras_corte(salida),
            stderr: bytes_tras_corte(error),
        });
    };
    let Some(codigo) = estado.code() else {
        return Err(ErrorCorre::Ejecutor(format!(
            "{programa} terminó por una señal"
        )));
    };
    Ok(Fin::Termino(SalidaClaude {
        codigo,
        stdout: bytes_de(salida),
        stderr: bytes_de(error),
    }))
}

/// Bytes de un flujo leído en otro hilo. Sin flujo o con un hilo caído, vacío.
fn bytes_de(hilo: Option<JoinHandle<Vec<u8>>>) -> Vec<u8> {
    hilo.and_then(|h| h.join().ok()).unwrap_or_default()
}

/// Bytes de un flujo después de matar el proceso. Espera la lectura hasta `GRACIA_LECTURA` y no más: un
/// nieto que retiene la pipe no debe colgar la corrida. Si vence la espera, el hilo se suelta y da vacío.
fn bytes_tras_corte(hilo: Option<JoinHandle<Vec<u8>>>) -> Vec<u8> {
    let Some(hilo) = hilo else {
        return Vec::new();
    };
    let limite = std::time::Instant::now() + GRACIA_LECTURA;
    while !hilo.is_finished() {
        if std::time::Instant::now() >= limite {
            return Vec::new();
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    hilo.join().unwrap_or_default()
}

#[cfg(test)]
mod codigos_podman {
    use super::*;

    #[test]
    fn codigos_de_infra_de_podman_tienen_significado() {
        for codigo in [125, 126, 127] {
            assert!(
                significado_codigo_podman(codigo).is_some(),
                "falta el significado de {codigo}"
            );
        }
    }

    #[test]
    fn codigos_de_claude_no_son_fallo_de_podman() {
        for codigo in [0, 1, 2, 9, 124, 128, 130] {
            assert!(
                significado_codigo_podman(codigo).is_none(),
                "{codigo} no es de podman"
            );
        }
    }

    #[test]
    fn fallo_de_podman_nombra_el_codigo_y_su_significado() {
        for (codigo, esperado) in [(126, "no es ejecutable"), (127, "no se encontró")] {
            let salida = SalidaClaude {
                codigo,
                stdout: Vec::new(),
                stderr: b"detalle\n".to_vec(),
            };
            let error = fallo_de_podman(&salida).expect("es fallo de podman");
            let texto = error.to_string();
            assert!(matches!(error, ErrorCorre::Ejecutor(_)), "{texto}");
            assert!(
                texto.contains(&codigo.to_string()) && texto.contains(esperado),
                "{texto}"
            );
        }
    }

    #[test]
    fn sigkill_con_stdout_da_cortado_con_ese_stdout() {
        let stdout = b"{\"tipo\":\"parcial\"}\n".to_vec();
        let salida = SalidaClaude {
            codigo: 137,
            stdout: stdout.clone(),
            stderr: b"detalle\n".to_vec(),
        };
        match clasificar_salida(salida) {
            Err(ErrorCorre::Cortado {
                mensaje,
                stdout: conservado,
            }) => {
                assert_eq!(conservado, stdout);
                assert!(
                    mensaje.contains("el contenedor fue matado por SIGKILL"),
                    "{mensaje}"
                );
            }
            otro => panic!("137 debe dar Cortado, dio {otro:?}"),
        }
    }

    #[test]
    fn codigos_de_infra_de_podman_siguen_siendo_ejecutor() {
        for codigo in [125, 126, 127] {
            let salida = SalidaClaude {
                codigo,
                stdout: b"algo\n".to_vec(),
                stderr: b"detalle\n".to_vec(),
            };
            match clasificar_salida(salida) {
                Err(error @ ErrorCorre::Ejecutor(_)) => {
                    assert!(error.to_string().contains(&codigo.to_string()), "{error}");
                }
                otro => panic!("{codigo} debe dar Ejecutor, dio {otro:?}"),
            }
        }
    }

    #[test]
    fn codigos_de_claude_0_y_1_pasan_como_ok() {
        for codigo in [0, 1] {
            let salida = SalidaClaude {
                codigo,
                stdout: b"resultado\n".to_vec(),
                stderr: Vec::new(),
            };
            assert_eq!(clasificar_salida(salida.clone()).ok(), Some(salida));
        }
    }

    #[test]
    fn fallo_de_podman_con_codigo_de_claude_es_none() {
        let salida = SalidaClaude {
            codigo: 1,
            stdout: Vec::new(),
            stderr: Vec::new(),
        };
        assert!(fallo_de_podman(&salida).is_none());
    }
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
            plugin: None,
            plazo: crate::corre::contenedor::TIMEOUT_BUILDER,
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

    /// Lanzamiento del rol con `plugin`. El repo va bajo `tmp`, en una corrida.
    fn con_plugin(tmp: &TempDir, plugin: PathBuf) -> Lanzamiento {
        let repo = tmp.path().join("usuario/.cache/root/corridas/corre-1/repo");
        let mut l = lanzamiento(repo, Vec::new());
        l.plugin = Some(plugin);
        l
    }

    #[test]
    fn corre_contenedor_plugin_inexistente_es_error_que_nombra_la_ruta() {
        let tmp = TempDir::new().expect("tmp");
        let (ejecutor, _) = ejecutor_real(&tmp, true);
        let plugin = tmp.path().join("no-existe-ecc");
        let error = ejecutor
            .lanzar(&con_plugin(&tmp, plugin.clone()))
            .expect_err("sin plugin no hay lanzamiento");
        assert!(
            matches!(error, ErrorCorre::Ejecutor(ref m) if m.contains(&plugin.display().to_string())),
            "{error}"
        );
        let estado = tmp
            .path()
            .join("usuario/.cache/root/corridas/corre-1/claude-estado");
        assert!(
            !estado.exists(),
            "no debe crear el estado antes de revisar el plugin"
        );
    }

    #[test]
    fn corre_contenedor_plugin_que_es_archivo_es_error() {
        let tmp = TempDir::new().expect("tmp");
        let (ejecutor, _) = ejecutor_real(&tmp, true);
        let plugin = tmp.path().join("plugin-archivo");
        fs::write(&plugin, "no es carpeta").expect("archivo");
        let error = ejecutor
            .lanzar(&con_plugin(&tmp, plugin.clone()))
            .expect_err("un plugin debe ser carpeta");
        assert!(
            matches!(error, ErrorCorre::Ejecutor(ref m) if m.contains(&plugin.display().to_string())),
            "{error}"
        );
    }

    #[test]
    fn corre_contenedor_plugin_dentro_de_root_home_no_lanza() {
        let tmp = TempDir::new().expect("tmp");
        let (ejecutor, _) = ejecutor_real(&tmp, true);
        let plugin = tmp.path().join("root").join("ecc-plugin");
        fs::create_dir_all(&plugin).expect("plugin");
        let error = ejecutor
            .lanzar(&con_plugin(&tmp, plugin))
            .expect_err("un plugin dentro de ROOT_HOME no se lanza");
        assert!(
            matches!(error, ErrorCorre::Ejecutor(ref m) if m.contains("plugin") && m.contains("toca ROOT_HOME")),
            "{error}"
        );
    }

    #[test]
    fn corre_proceso_devuelve_codigo_y_salida() {
        let args = vec![
            "-c".to_string(),
            "echo hola; echo mal >&2; exit 4".to_string(),
        ];
        let Fin::Termino(salida) =
            correr_proceso("sh", &args, Duration::from_secs(20)).expect("corre")
        else {
            panic!("no debe vencer");
        };
        assert_eq!(salida.codigo, 4);
        assert_eq!(salida.stdout, b"hola\n".to_vec());
        assert_eq!(salida.stderr, b"mal\n".to_vec());
    }

    #[test]
    fn corre_proceso_vence_el_plazo_y_no_espera_el_fin() {
        let args = vec!["-c".to_string(), "sleep 5".to_string()];
        let inicio = std::time::Instant::now();
        let fin = correr_proceso("sh", &args, Duration::from_millis(100)).expect("corre");
        assert!(matches!(fin, Fin::Vencio { .. }), "debe vencer: {fin:?}");
        assert!(
            inicio.elapsed() < Duration::from_secs(4),
            "no espera el fin del proceso"
        );
    }

    #[test]
    fn corre_proceso_vencido_conserva_lo_que_alcanzo_a_dar() {
        // `exec` hace que el `sleep` sea el proceso que se mata: nadie más retiene la pipe.
        let args = vec!["-c".to_string(), "echo parcial; exec sleep 5".to_string()];
        let fin = correr_proceso("sh", &args, Duration::from_secs(1)).expect("corre");
        match fin {
            Fin::Vencio { stdout, .. } => assert_eq!(stdout, b"parcial\n".to_vec()),
            otro => panic!("debe venir vencido: {otro:?}"),
        }
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
    fn corre_contenedor_vida_restante_sin_credencial_es_error_de_ejecutor() {
        let tmp = TempDir::new().expect("tmp");
        let (ejecutor, credencial) = ejecutor_real(&tmp, false);
        let error = ejecutor
            .vida_restante()
            .expect_err("sin credencial no hay vida que medir");
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
    fn corre_contenedor_vida_restante_da_lo_que_resta_del_login() {
        const HORA_MS: u64 = 3_600_000;
        let tmp = TempDir::new().expect("tmp");
        let (ejecutor, credencial) = ejecutor_real(&tmp, false);
        escribir_credencial(&credencial, &credencial_con_mcp(ahora_ms() + 3 * HORA_MS));
        let vida = ejecutor
            .vida_restante()
            .expect("lee la vida")
            .expect("hay límite conocido");
        assert!(
            vida > Duration::from_secs(3 * 3600 - 60) && vida <= Duration::from_secs(3 * 3600),
            "{vida:?}"
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

    /// Guarda de vigencia por rol: un login de 3 horas alcanza para el plazo de un rol de 2 horas.
    /// Un login de 1 hora no alcanza. Este test pasa desde el inicio: cubre la regla, no un cambio.
    #[test]
    fn corre_ejecutor_token_de_3_horas_deja_arrancar_dos_roles() {
        const HORA_MS: u64 = 3_600_000;
        let tmp = TempDir::new().expect("tmp");
        let (ejecutor, credencial) = ejecutor_real(&tmp, false);
        let tres_horas = ahora_ms() + 3 * HORA_MS;
        escribir_credencial(&credencial, &credencial_con_mcp(tres_horas));
        ejecutor
            .verificar_antes(crate::corre::contenedor::TIMEOUT_BUILDER)
            .expect("3 horas alcanzan para un rol");
        let una_hora = ahora_ms() + HORA_MS;
        escribir_credencial(&credencial, &credencial_con_mcp(una_hora));
        let error = ejecutor
            .verificar_antes(crate::corre::contenedor::TIMEOUT_BUILDER)
            .expect_err("1 hora no alcanza para un rol de 2 horas");
        assert!(matches!(error, ErrorCorre::Ejecutor(_)), "{error}");
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
