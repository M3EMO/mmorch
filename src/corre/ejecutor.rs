//! Ejecutor del builder. Un `Lanzamiento` describe un `claude -p` de un rol y el ejecutor lo corre.
//! `EjecutorContenedorBuilder` lo corre en un contenedor de podman (decisión 43, Tarea 26, capa 1). Los
//! tests usan `pruebas::EjecutorFalso`, que corre `tests/fixtures/claude-falso.sh` y nunca llama a `claude`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::contenedor::{args_podman_builder, estado_de, fisica, validar_montajes};
use super::{ErrorCorre, copiar_sin_mcp, verificar_vigencia, vida_restante};
use crate::oracle::ErrorOraculo;
use crate::oracle::contenedor::{
    Lectura, TOPE_SALIDA_BUILDER, esperar, leer_en_hilo_con, tamano_legible,
};
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
        // El cerrojo entra aquí y llega hasta el registro del hijo. Una señal espera en él: no borra antes.
        let cerrojo = estado_hijo();
        // Con el ejecutor terminando no se crea copia ni se lanza nada.
        if cerrojo.terminando {
            return Err(ErrorCorre::Ejecutor(
                "no se lanza el builder: el ejecutor está terminando".into(),
            ));
        }
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
        #[cfg(test)]
        llamar_costura(&COSTURA_ENTRE_TERMINANDO_Y_COPIA);
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

        // El plazo de este rol no supera el tope del ejecutor. El spawn y el registro van bajo el cerrojo.
        let plazo = l.plazo.min(self.timeout);
        match correr_registrado(cerrojo, "podman", &args, plazo, TOPE_SALIDA_BUILDER)? {
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

/// Estado del hijo en curso. Va bajo un solo cerrojo: el spawn, el registro y la espera miran el mismo
/// estado frente a `terminar_por_senal`.
struct EstadoHijo {
    /// El `Child` del `podman run` en curso. Sin valor si no hay hijo. Quien lo recoge lo saca de aquí.
    hijo: Option<Child>,
    /// Lo pone `terminar_por_senal`. Un lanzamiento pedido después no lanza nada.
    terminando: bool,
}

static ESTADO_HIJO: Mutex<EstadoHijo> = Mutex::new(EstadoHijo {
    hijo: None,
    terminando: false,
});

/// Toma el cerrojo del estado del hijo. Un pánico previo no lo deja inservible.
fn estado_hijo() -> MutexGuard<'static, EstadoHijo> {
    ESTADO_HIJO.lock().unwrap_or_else(|e| e.into_inner())
}

/// Pausa entre dos consultas del hijo registrado. Igual que el sondeo de `oracle::contenedor`.
const SONDEO_HIJO: Duration = Duration::from_millis(20);

/// Termina el ejecutor por una señal. Toma el cerrojo, marca `terminando` y mata el hijo en curso, si
/// lo hay. Con el cerrojo tomado, corre `limpieza` con el resultado de matar. Un hijo ya esperado no
/// está en el estado: nunca se manda `kill` a un pid ajeno. Un lanzamiento que espera el cerrojo no
/// empieza. Si `limpieza` sale del proceso, no vuelve.
pub fn terminar_por_senal(limpieza: impl FnOnce(Result<(), ErrorCorre>)) {
    let mut estado = estado_hijo();
    estado.terminando = true;
    let resultado = match estado.hijo.take() {
        Some(mut hijo) => matar_y_recoger(&mut hijo).map_err(|e| {
            ErrorCorre::Ejecutor(format!("no se pudo terminar el hijo {}: {e}", hijo.id()))
        }),
        None => Ok(()),
    };
    limpieza(resultado);
}

/// Mata con SIGKILL un hijo que aún no se esperó y lo recoge. Devuelve el resultado del `kill`.
fn matar_y_recoger(hijo: &mut Child) -> io::Result<()> {
    #[cfg(test)]
    KILLS_DE_PRUEBA.fetch_add(1, Ordering::Relaxed);
    let muerto = hijo.kill();
    let _ = hijo.wait();
    muerto
}

/// Pid del hijo en curso, para los tests.
#[cfg(test)]
pub(crate) fn pid_en_curso() -> Option<u32> {
    estado_hijo().hijo.as_ref().map(Child::id)
}

/// Cuántos `kill` mandó el ejecutor en el test. Un pid ya esperado no debe sumar.
#[cfg(test)]
pub(crate) static KILLS_DE_PRUEBA: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// Si vale `true`, la siguiente consulta de la espera del hijo da error.
#[cfg(test)]
pub(crate) static FALLO_DE_ESPERA: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Costura entre el chequeo de `terminando` y la copia de la credencial. Corre con el cerrojo tomado.
#[cfg(test)]
pub(crate) static COSTURA_ENTRE_TERMINANDO_Y_COPIA: Mutex<Option<Costura>> = Mutex::new(None);

/// Función que corre en un punto de `correr_proceso_con_tope`, solo en tests.
#[cfg(test)]
type Costura = Box<dyn Fn() + Send + Sync>;

/// Costura entre el spawn y el registro del pid. Corre tras el spawn y antes de registrar.
#[cfg(test)]
static COSTURA_ENTRE_SPAWN_Y_REGISTRO: Mutex<Option<Costura>> = Mutex::new(None);

/// Costura tras la espera del hijo registrado. Corre bajo el cerrojo, con el hijo ya esperado y antes de
/// sacarlo del registro.
#[cfg(test)]
static COSTURA_TRAS_ESPERA: Mutex<Option<Costura>> = Mutex::new(None);

/// Corre la costura guardada en `costura`, si la hay.
#[cfg(test)]
fn llamar_costura(costura: &Mutex<Option<Costura>>) {
    let guardia = costura.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(f) = guardia.as_ref() {
        f();
    }
}

/// Cerrojo de los tests que corren hijos o señales. Así una señal de un test no mata el hijo de otro.
/// Al tomarlo y al soltarlo, borra la marca `terminando` y las costuras. Ningún test hereda una señal.
#[cfg(test)]
pub(crate) fn serie_de_pruebas() -> SeriePruebas {
    static SERIE: Mutex<()> = Mutex::new(());
    let guardia = SERIE.lock().unwrap_or_else(|e| e.into_inner());
    reiniciar_estado_de_pruebas();
    SeriePruebas { _guardia: guardia }
}

/// Guardia de `serie_de_pruebas`. Al soltarlo, deja el estado del hijo limpio para el siguiente test.
#[cfg(test)]
pub(crate) struct SeriePruebas {
    _guardia: std::sync::MutexGuard<'static, ()>,
}

#[cfg(test)]
impl Drop for SeriePruebas {
    fn drop(&mut self) {
        reiniciar_estado_de_pruebas();
    }
}

/// Quita la marca `terminando` y las costuras. No toca el pid registrado.
#[cfg(test)]
fn reiniciar_estado_de_pruebas() {
    estado_hijo().terminando = false;
    for costura in [
        &COSTURA_ENTRE_SPAWN_Y_REGISTRO,
        &COSTURA_TRAS_ESPERA,
        &COSTURA_ENTRE_TERMINANDO_Y_COPIA,
    ] {
        *costura.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
    KILLS_DE_PRUEBA.store(0, Ordering::Relaxed);
    FALLO_DE_ESPERA.store(false, Ordering::Relaxed);
}

/// Quita los contenedores del builder que lanzó este proceso: los que llevan `root-builder-<pid>-` en
/// el nombre. Corre `<programa> ps` y luego `<programa> rm -f -t 0 <nombres>`, con `plazo` en total.
/// Devuelve el primer fallo: no se pudo correr el programa, un paso venció el plazo o salió con error.
pub fn quitar_builders_de_este_proceso(programa: &str, plazo: Duration) -> Result<(), ErrorCorre> {
    let inicio = Instant::now();
    let prefijo = format!("root-builder-{}-", std::process::id());
    let filtro = format!("name={prefijo}");
    let args_ps = [
        "ps",
        "-a",
        "--format",
        "{{.Names}}",
        "--filter",
        filtro.as_str(),
    ];
    let args_ps: Vec<String> = args_ps.iter().map(|a| a.to_string()).collect();
    let salida = correr_paso_de_quitar(programa, &args_ps, plazo, "ps")?;
    let nombres: Vec<String> = String::from_utf8_lossy(&salida.stdout)
        .lines()
        .map(str::trim)
        .filter(|nombre| nombre.starts_with(&prefijo))
        .map(str::to_string)
        .collect();
    if nombres.is_empty() {
        return Ok(());
    }
    let mut args_rm: Vec<String> = ["rm", "-f", "-t", "0"].map(String::from).into();
    args_rm.extend(nombres);
    let restante = plazo.saturating_sub(inicio.elapsed());
    correr_paso_de_quitar(programa, &args_rm, restante, "rm")?;
    Ok(())
}

/// Corre un paso de `quitar_builders_de_este_proceso`. Solo sale bien si termina con código 0 dentro del plazo.
fn correr_paso_de_quitar(
    programa: &str,
    args: &[String],
    plazo: Duration,
    paso: &str,
) -> Result<SalidaClaude, ErrorCorre> {
    match correr_hijo(programa, args, plazo, TOPE_SALIDA_BUILDER, false)? {
        Fin::Termino(salida) if salida.codigo == 0 => Ok(salida),
        Fin::Termino(salida) => Err(ErrorCorre::Ejecutor(format!(
            "{paso} de los contenedores salió con código {}: {}",
            salida.codigo,
            String::from_utf8_lossy(&salida.stderr).trim()
        ))),
        Fin::Vencio { .. } => Err(ErrorCorre::Ejecutor(format!(
            "{paso} de los contenedores venció el plazo"
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
pub(crate) enum Fin {
    Termino(SalidaClaude),
    /// Lo leído de `stdout` y `stderr` hasta el corte. Puede faltar si un nieto retiene la pipe.
    Vencio {
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
}

/// Cuánto espera la lectura de una pipe después de matar el proceso. Pasado ese plazo, se deja de leer.
const GRACIA_LECTURA: Duration = Duration::from_secs(2);

/// Corre `programa` con `args` y espera hasta `plazo`, con el tope del builder. Ver `correr_proceso_con_tope`.
/// Solo los tests lo usan: el builder llama a `correr_registrado` con el cerrojo ya tomado.
#[cfg(test)]
pub(crate) fn correr_proceso(
    programa: &str,
    args: &[String],
    plazo: Duration,
) -> Result<Fin, ErrorCorre> {
    correr_proceso_con_tope(programa, args, plazo, TOPE_SALIDA_BUILDER)
}

/// Corre `programa` con `args` y espera hasta `plazo`. Si vence, mata el proceso y recoge lo leído hasta
/// ahí. Un fallo al lanzar o al esperar es `Ejecutor`. El código de salida lo decide el llamador.
/// Si el proceso termina y su stdout pasa `tope`, falla: el builder necesita la salida entera.
/// Un stderr recortado no falla. Solo se trunca.
#[cfg(test)]
fn correr_proceso_con_tope(
    programa: &str,
    args: &[String],
    plazo: Duration,
    tope: usize,
) -> Result<Fin, ErrorCorre> {
    correr_hijo(programa, args, plazo, tope, true)
}

/// Corre `programa` como `correr_proceso_con_tope`. Con `registrar`, el hijo queda en el registro de
/// `terminar_por_senal` y no lanza nada si el ejecutor ya termina. Sin `registrar`, no toca el registro
/// ni lo frena `terminando`: lo usan los comandos que corre la señal.
fn correr_hijo(
    programa: &str,
    args: &[String],
    plazo: Duration,
    tope: usize,
    registrar: bool,
) -> Result<Fin, ErrorCorre> {
    if registrar {
        return correr_registrado(estado_hijo(), programa, args, plazo, tope);
    }
    let mut hijo = spawn_hijo(programa, args)?;
    let pipes = tomar_pipes(&mut hijo, tope);
    let espera = esperar_o_matar(&mut hijo, plazo);
    fin_de_espera(programa, espera, pipes, tope)
}

/// Flujos de salida de un hijo, leídos cada uno en su hilo. Sin flujo, `None`.
type Pipes = (Option<JoinHandle<Lectura>>, Option<JoinHandle<Lectura>>);

/// Toma las pipes de `hijo` y arranca un hilo que lee cada una. Va antes de guardar el hijo en el registro.
fn tomar_pipes(hijo: &mut Child, tope: usize) -> Pipes {
    let salida = hijo.stdout.take().map(|f| leer_en_hilo_con(f, tope));
    let error = hijo.stderr.take().map(|f| leer_en_hilo_con(f, tope));
    (salida, error)
}

/// Lanza `programa` y lo registra como hijo en curso. El cerrojo entra tomado: el spawn y el registro
/// van bajo él, y lo suelta antes de esperar. Con `terminando` o con otro hijo registrado, no lanza nada.
fn correr_registrado(
    mut estado: MutexGuard<'static, EstadoHijo>,
    programa: &str,
    args: &[String],
    plazo: Duration,
    tope: usize,
) -> Result<Fin, ErrorCorre> {
    if estado.terminando {
        return Err(ErrorCorre::Ejecutor(format!(
            "no se lanza {programa}: el ejecutor está terminando"
        )));
    }
    if estado.hijo.is_some() {
        return Err(ErrorCorre::Ejecutor(format!(
            "no se lanza {programa}: ya hay un hijo en curso"
        )));
    }
    let mut hijo = spawn_hijo(programa, args)?;
    let pipes = tomar_pipes(&mut hijo, tope);
    #[cfg(test)]
    llamar_costura(&COSTURA_ENTRE_SPAWN_Y_REGISTRO);
    estado.hijo = Some(hijo);
    drop(estado);
    let espera = esperar_registrado(plazo);
    fin_de_espera(programa, espera, pipes, tope)
}

/// Arma el `Fin` de un hijo esperado o vencido. Un error de la espera pasa sin cambios.
fn fin_de_espera(
    programa: &str,
    espera: Result<Option<ExitStatus>, ErrorCorre>,
    (salida, error): Pipes,
    tope: usize,
) -> Result<Fin, ErrorCorre> {
    let Some(estado) = espera? else {
        return Ok(Fin::Vencio {
            stdout: lectura_tras_corte(salida).bytes,
            stderr: lectura_tras_corte(error).bytes,
        });
    };
    let Some(codigo) = estado.code() else {
        return Err(ErrorCorre::Ejecutor(format!(
            "{programa} terminó por una señal"
        )));
    };
    let stdout = lectura_de(salida);
    if stdout.recortada {
        return Err(ErrorCorre::Ejecutor(format!(
            "salida del builder mayor que {}",
            tamano_legible(tope)
        )));
    }
    Ok(Fin::Termino(SalidaClaude {
        codigo,
        stdout: stdout.bytes,
        stderr: lectura_de(error).bytes,
    }))
}

/// Espera al hijo registrado hasta `plazo`. En cada vuelta toma el cerrojo, mira el registro y lo suelta
/// durante la pausa. Si el hijo ya no está en el registro, lo recogió la señal.
/// Si la espera da error o vence el plazo, mata el hijo y lo recoge antes de devolver.
fn esperar_registrado(plazo: Duration) -> Result<Option<ExitStatus>, ErrorCorre> {
    let limite = Instant::now() + plazo;
    loop {
        let mut estado = estado_hijo();
        let consulta = match estado.hijo.as_mut() {
            Some(hijo) => consultar(hijo),
            None => {
                return Err(ErrorCorre::Ejecutor(
                    "el hijo fue terminado por una señal".into(),
                ));
            }
        };
        match consulta {
            Ok(Some(status)) => {
                // Ya esperado: sale del registro en el mismo cerrojo. Nunca quedará un `kill` para él.
                #[cfg(test)]
                llamar_costura(&COSTURA_TRAS_ESPERA);
                estado.hijo = None;
                return Ok(Some(status));
            }
            Ok(None) if Instant::now() < limite => {}
            Ok(None) => {
                if let Some(mut hijo) = estado.hijo.take() {
                    let _ = matar_y_recoger(&mut hijo);
                }
                return Ok(None);
            }
            Err(e) => {
                if let Some(mut hijo) = estado.hijo.take() {
                    let _ = matar_y_recoger(&mut hijo);
                }
                return Err(ErrorCorre::Ejecutor(format!(
                    "no se pudo consultar el hijo: {e}"
                )));
            }
        }
        drop(estado);
        std::thread::sleep(SONDEO_HIJO);
    }
}

/// Consulta si el hijo ya salió, sin esperarlo. En tests, puede forzar un error.
fn consultar(hijo: &mut Child) -> io::Result<Option<ExitStatus>> {
    #[cfg(test)]
    if FALLO_DE_ESPERA.load(Ordering::Relaxed) {
        return Err(io::Error::other("fallo forzado de la espera"));
    }
    hijo.try_wait()
}

/// Lanza `programa` con sus flujos de salida en pipe. Sin registro ni cerrojo.
fn spawn_hijo(programa: &str, args: &[String]) -> Result<Child, ErrorCorre> {
    Command::new(programa)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            ErrorCorre::Ejecutor(format!(
                "no se pudo lanzar {programa} (¿está instalado y en PATH?): {e}"
            ))
        })
}

/// Espera al hijo sin registro hasta `plazo`. Si vence o la espera falla, lo mata y lo recoge. Da `None` si venció.
fn esperar_o_matar(hijo: &mut Child, plazo: Duration) -> Result<Option<ExitStatus>, ErrorCorre> {
    let estado = match esperar(hijo, plazo) {
        Ok(estado) => estado,
        Err(e) => {
            let _ = matar_y_recoger(hijo);
            return Err(ErrorCorre::Ejecutor(match e {
                ErrorOraculo::Infra(m) => m,
                otro => otro.to_string(),
            }));
        }
    };
    if estado.is_none() {
        let _ = matar_y_recoger(hijo);
    }
    Ok(estado)
}

/// Lectura de un flujo leído en otro hilo. Sin flujo o con un hilo caído, vacía.
fn lectura_de(hilo: Option<JoinHandle<Lectura>>) -> Lectura {
    hilo.and_then(|h| h.join().ok()).unwrap_or_default()
}

/// Lectura de un flujo después de matar el proceso. Espera la lectura hasta `GRACIA_LECTURA` y no más: un
/// nieto que retiene la pipe no debe colgar la corrida. Si vence la espera, el hilo se suelta y da vacía.
fn lectura_tras_corte(hilo: Option<JoinHandle<Lectura>>) -> Lectura {
    let Some(hilo) = hilo else {
        return Lectura::default();
    };
    let limite = std::time::Instant::now() + GRACIA_LECTURA;
    while !hilo.is_finished() {
        if std::time::Instant::now() >= limite {
            return Lectura::default();
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

    /// Resultado de una señal en otro hilo, guardado por una costura para que el test lo espere.
    type SenalGuardada = std::sync::Arc<Mutex<Option<JoinHandle<Option<Result<(), ErrorCorre>>>>>>;

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
        // Toma la serie: un test de señal no debe dejar puesta la marca `terminando` aquí.
        let _serie = serie_de_pruebas();
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
        // Toma la serie: un test de señal no debe dejar puesta la marca `terminando` aquí.
        let _serie = serie_de_pruebas();
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
        // Toma la serie: un test de señal no debe dejar puesta la marca `terminando` aquí.
        let _serie = serie_de_pruebas();
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
        // Toma la serie: un test de señal no debe dejar puesta la marca `terminando` aquí.
        let _serie = serie_de_pruebas();
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
        // Toma la serie: un test de señal no debe dejar puesta la marca `terminando` aquí.
        let _serie = serie_de_pruebas();
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
        let _serie = serie_de_pruebas();
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
        let _serie = serie_de_pruebas();
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
        let _serie = serie_de_pruebas();
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
        let _serie = serie_de_pruebas();
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

    #[test]
    fn corre_proceso_con_tope_falla_si_stdout_pasa_el_tope() {
        let _serie = serie_de_pruebas();
        let args = vec![
            "-c".to_string(),
            "head -c 5000 /dev/zero | tr '\\0' x".to_string(),
        ];
        let error = correr_proceso_con_tope("sh", &args, Duration::from_secs(20), 1000)
            .expect_err("stdout pasa el tope");
        assert!(
            matches!(error, ErrorCorre::Ejecutor(ref m) if m.contains("salida del builder mayor que")),
            "{error}"
        );
    }

    #[test]
    fn corre_proceso_con_tope_stderr_recortado_no_falla() {
        let _serie = serie_de_pruebas();
        let args = vec![
            "-c".to_string(),
            "head -c 5000 /dev/zero | tr '\\0' x >&2; echo ok".to_string(),
        ];
        let Fin::Termino(salida) =
            correr_proceso_con_tope("sh", &args, Duration::from_secs(20), 1000).expect("corre")
        else {
            panic!("no debe vencer");
        };
        assert_eq!(salida.stdout, b"ok\n".to_vec());
        assert_eq!(salida.stderr.len(), 1000);
    }

    #[test]
    fn corre_proceso_con_tope_amplio_y_salida_chica_igual_que_hoy() {
        let _serie = serie_de_pruebas();
        let args = vec!["-c".to_string(), "echo hola".to_string()];
        let Fin::Termino(salida) =
            correr_proceso_con_tope("sh", &args, Duration::from_secs(20), 1000).expect("corre")
        else {
            panic!("no debe vencer");
        };
        assert_eq!(salida.codigo, 0);
        assert_eq!(salida.stdout, b"hola\n".to_vec());
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

    /// Script de prueba ejecutable con `cuerpo`, en `dir`. Devuelve su ruta.
    fn programa_falso(dir: &Path, cuerpo: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let ruta = dir.join("podman-falso");
        fs::write(&ruta, format!("#!/bin/sh\n{cuerpo}\n")).expect("script");
        fs::set_permissions(&ruta, fs::Permissions::from_mode(0o755)).expect("permisos");
        ruta
    }

    #[test]
    fn quitar_builders_pide_por_nombre_solo_los_de_este_proceso() {
        let _serie = serie_de_pruebas();
        let tmp = TempDir::new().expect("tmp");
        let log = tmp.path().join("podman.log");
        let pid = std::process::id();
        let cuerpo = format!(
            "echo \"$*\" >> '{}'\nif [ \"$1\" = ps ]; then\necho root-builder-{pid}-0\necho root-builder-{pid}-1\necho root-builder-1-0\necho otro\nfi",
            log.display()
        );
        let programa = programa_falso(tmp.path(), &cuerpo);
        quitar_builders_de_este_proceso(programa.to_str().expect("ruta"), Duration::from_secs(10))
            .expect("quita los del proceso");
        let anotado = fs::read_to_string(&log).expect("log");
        assert!(
            anotado.contains(&format!(
                "ps -a --format {{{{.Names}}}} --filter name=root-builder-{pid}-"
            )),
            "{anotado}"
        );
        assert!(
            anotado.contains(&format!(
                "rm -f -t 0 root-builder-{pid}-0 root-builder-{pid}-1"
            )),
            "{anotado}"
        );
        assert!(!anotado.contains("root-builder-1-0"), "{anotado}");
        assert!(!anotado.contains("otro"), "{anotado}");
    }

    #[test]
    fn quitar_builders_sin_nombres_no_llama_a_rm() {
        let _serie = serie_de_pruebas();
        let tmp = TempDir::new().expect("tmp");
        let log = tmp.path().join("podman.log");
        let cuerpo = format!("echo \"$*\" >> '{}'", log.display());
        let programa = programa_falso(tmp.path(), &cuerpo);
        quitar_builders_de_este_proceso(programa.to_str().expect("ruta"), Duration::from_secs(10))
            .expect("sin nombres no hay error");
        let anotado = fs::read_to_string(&log).expect("log");
        assert!(!anotado.contains(" rm "), "{anotado}");
    }

    #[test]
    fn quitar_builders_respeta_el_plazo() {
        let _serie = serie_de_pruebas();
        let tmp = TempDir::new().expect("tmp");
        // `ps` vence: el plazo se corta y se avisa el fallo.
        let programa = programa_falso(tmp.path(), "exec sleep 30");
        let inicio = std::time::Instant::now();
        let resultado = quitar_builders_de_este_proceso(
            programa.to_str().expect("ruta"),
            Duration::from_millis(300),
        );
        assert!(
            inicio.elapsed() < Duration::from_secs(10),
            "no espera el fin"
        );
        assert!(resultado.is_err(), "vencer el plazo es un fallo");
    }

    #[test]
    fn quitar_builders_con_programa_inexistente_avisa_el_error() {
        let _serie = serie_de_pruebas();
        let resultado =
            quitar_builders_de_este_proceso("/no/existe/podman", Duration::from_secs(1));
        assert!(resultado.is_err(), "sin programa no se quita nada");
    }

    #[test]
    fn quitar_builders_con_rm_fallido_avisa_el_error() {
        let _serie = serie_de_pruebas();
        let tmp = TempDir::new().expect("tmp");
        let pid = std::process::id();
        // `ps` da un nombre del proceso y `rm` sale con error.
        let cuerpo =
            format!("if [ \"$1\" = ps ]; then echo root-builder-{pid}-0; exit 0; fi\nexit 1");
        let programa = programa_falso(tmp.path(), &cuerpo);
        let resultado = quitar_builders_de_este_proceso(
            programa.to_str().expect("ruta"),
            Duration::from_secs(10),
        );
        assert!(resultado.is_err(), "un rm fallido es un error");
    }

    /// Hijo en curso falso: un `sh` que se reemplaza por `sleep`. Corre en otro hilo, como `podman run`.
    fn lanzar_hijo_en_curso() -> std::thread::JoinHandle<bool> {
        let args = vec!["-c".to_string(), "exec sleep 30".to_string()];
        std::thread::spawn(move || correr_proceso("sh", &args, Duration::from_secs(10)).is_err())
    }

    /// Espera hasta que haya un hijo registrado. Si no lo hay en 5 s, falla.
    fn esperar_hijo_registrado() {
        let limite = Instant::now() + Duration::from_secs(5);
        while pid_en_curso().is_none() {
            assert!(Instant::now() < limite, "el hijo no quedó registrado");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn terminar_hijo_sin_hijo_en_curso_no_hace_nada() {
        let _serie = serie_de_pruebas();
        let mut resultado = None;
        terminar_por_senal(|r| resultado = Some(r));
        assert!(matches!(resultado, Some(Ok(()))), "{resultado:?}");
        assert_eq!(pid_en_curso(), None);
    }

    #[test]
    fn terminar_hijo_mata_el_hijo_en_curso_y_libera_el_registro() {
        let _serie = serie_de_pruebas();
        let hilo = lanzar_hijo_en_curso();
        esperar_hijo_registrado();
        let inicio = Instant::now();
        terminar_por_senal(|_| {});
        let murio_con_error = hilo.join().expect("hilo del hijo");
        assert!(
            murio_con_error,
            "el hijo muere por la señal, no por su plazo"
        );
        assert!(
            inicio.elapsed() < Duration::from_secs(5),
            "no espera el plazo"
        );
        assert_eq!(pid_en_curso(), None, "el registro se limpia al terminar");
    }

    #[test]
    fn senal_en_la_ventana_entre_spawn_y_registro_termina_el_hijo() {
        let _serie = serie_de_pruebas();
        // La costura avisa que el spawn ocurrió y espera un instante, antes de registrar el pid.
        let (aviso, avisado) = std::sync::mpsc::channel::<()>();
        *COSTURA_ENTRE_SPAWN_Y_REGISTRO.lock().expect("costura") = Some(Box::new(move || {
            let _ = aviso.send(());
            std::thread::sleep(Duration::from_millis(100));
        }));
        let senal = std::thread::spawn(move || {
            avisado
                .recv_timeout(Duration::from_secs(5))
                .expect("la costura avisa el spawn");
            let mut recibido = None;
            terminar_por_senal(|r| recibido = Some(r));
            recibido
        });
        let args = vec!["-c".to_string(), "exec sleep 30".to_string()];
        let inicio = Instant::now();
        let resultado = correr_proceso("sh", &args, Duration::from_secs(10));
        assert!(
            matches!(senal.join().expect("hilo de la señal"), Some(Ok(()))),
            "la señal termina sin error"
        );
        assert!(
            resultado.is_err(),
            "el hijo debe morir por la señal, no vencer por su plazo"
        );
        assert!(
            inicio.elapsed() < Duration::from_secs(5),
            "no espera el plazo"
        );
    }

    #[test]
    fn spawn_pedido_despues_de_terminando_no_lanza_nada() {
        let _serie = serie_de_pruebas();
        let tmp = TempDir::new().expect("tmp");
        let marcador = tmp.path().join("lanzado.txt");
        terminar_por_senal(|_| {});
        let args = vec![
            "-c".to_string(),
            format!("echo x > '{}'", marcador.display()),
        ];
        let resultado = correr_proceso("sh", &args, Duration::from_secs(10));
        assert!(
            matches!(resultado, Err(ErrorCorre::Ejecutor(_))),
            "con terminando no se lanza el proceso"
        );
        assert!(!marcador.exists(), "el proceso no debe correr");
    }

    #[test]
    fn lanzar_con_terminando_no_crea_copia_ni_lanza() {
        let _serie = serie_de_pruebas();
        let tmp = TempDir::new().expect("tmp");
        let (ejecutor, credencial) = ejecutor_real(&tmp, false);
        escribir_credencial(&credencial, &credencial_con_mcp(VENCE_LEJOS_MS));
        // Imagen vacía: si el código viejo llega a podman, podman falla sin bajar nada.
        let ejecutor = EjecutorContenedorBuilder {
            imagen: String::new(),
            ..ejecutor
        };
        terminar_por_senal(|_| {});
        let repo = tmp.path().join("usuario/.cache/root/corridas/corre-1/repo");
        let error = ejecutor
            .lanzar(&lanzamiento(repo, Vec::new()))
            .expect_err("con terminando no hay lanzamiento");
        assert!(matches!(error, ErrorCorre::Ejecutor(_)), "{error}");
        let copia = tmp
            .path()
            .join("usuario/.cache/root/corridas/corre-1/claude-estado/.credentials.json");
        assert!(!copia.exists(), "no debe crear la copia de la credencial");
    }

    #[test]
    fn quitar_builders_no_registra_su_pid_ni_pisa_el_del_hijo() {
        let _serie = serie_de_pruebas();
        let tmp = TempDir::new().expect("tmp");
        let hilo = lanzar_hijo_en_curso();
        esperar_hijo_registrado();
        let pid_hijo = pid_en_curso();
        let programa = programa_falso(tmp.path(), "exit 0");
        let resultado = quitar_builders_de_este_proceso(
            programa.to_str().expect("ruta"),
            Duration::from_secs(10),
        );
        let despues = pid_en_curso();
        // Limpieza antes de los asserts: el hijo de la prueba no debe quedar vivo si uno falla.
        terminar_por_senal(|_| {});
        let _ = hilo.join();
        assert!(resultado.is_ok(), "quitar sin nombres no falla");
        assert_eq!(
            despues, pid_hijo,
            "quitar no debe cambiar el pid registrado del hijo"
        );
    }

    /// La costura corre bajo el cerrojo, así que no puede llamar a `pid_en_curso`. El test mira el registro
    /// después de que `correr_proceso` vuelve. Un hijo que sale solo y otro que vence el plazo: los dos
    /// caminos del wait.
    #[test]
    fn registro_del_hijo_se_borra_al_recoger_el_wait() {
        let _serie = serie_de_pruebas();
        for (script, plazo) in [
            ("exit 0", Duration::from_secs(20)),
            ("exec sleep 5", Duration::from_millis(100)),
        ] {
            let args = vec!["-c".to_string(), script.to_string()];
            correr_proceso("sh", &args, plazo).expect("corre");
            assert_eq!(
                pid_en_curso(),
                None,
                "tras el wait no debe quedar pid registrado: {script}"
            );
        }
    }

    /// Señal en otro hilo: corre `terminar_por_senal` y devuelve lo que recibió su limpieza.
    fn senal_en_hilo() -> JoinHandle<Option<Result<(), ErrorCorre>>> {
        std::thread::spawn(|| {
            let mut recibido = None;
            terminar_por_senal(|r| recibido = Some(r));
            recibido
        })
    }

    #[test]
    fn senal_tras_el_try_wait_no_mata_el_pid_ya_esperado() {
        let _serie = serie_de_pruebas();
        // La costura cae entre el `try_wait` que vio la salida y el borrado del registro.
        let guardada: SenalGuardada = std::sync::Arc::new(Mutex::new(None));
        let para_costura = std::sync::Arc::clone(&guardada);
        *COSTURA_TRAS_ESPERA.lock().expect("costura") = Some(Box::new(move || {
            *para_costura.lock().expect("guardada") = Some(senal_en_hilo());
            std::thread::sleep(Duration::from_millis(100));
        }));
        let args = vec!["-c".to_string(), "exit 0".to_string()];
        let resultado = correr_proceso("sh", &args, Duration::from_secs(20));
        let hilo = guardada
            .lock()
            .expect("guardada")
            .take()
            .expect("la costura lanzó la señal");
        let senal = hilo.join().expect("hilo de la señal");
        assert!(
            matches!(resultado, Ok(Fin::Termino(ref s)) if s.codigo == 0),
            "el hijo sale solo: {resultado:?}"
        );
        assert!(
            matches!(senal, Some(Ok(()))),
            "la señal no falla: {senal:?}"
        );
        assert_eq!(
            KILLS_DE_PRUEBA.load(Ordering::Relaxed),
            0,
            "no se manda kill a un pid ya esperado"
        );
    }

    #[test]
    fn error_de_la_espera_termina_al_hijo_antes_de_devolver_el_error() {
        let _serie = serie_de_pruebas();
        let tmp = TempDir::new().expect("tmp");
        let pidfile = tmp.path().join("hijo.pid");
        let args = vec![
            "-c".to_string(),
            format!("echo $$ > '{}'; exec sleep 30", pidfile.display()),
        ];
        let hilo = std::thread::spawn(move || correr_proceso("sh", &args, Duration::from_secs(3)));
        let limite = Instant::now() + Duration::from_secs(5);
        while !pidfile.exists() {
            assert!(Instant::now() < limite, "el hijo no escribió su pid");
            std::thread::sleep(Duration::from_millis(10));
        }
        FALLO_DE_ESPERA.store(true, Ordering::Relaxed);
        let resultado = hilo.join().expect("hilo del hijo");
        let pid = fs::read_to_string(&pidfile).expect("pid");
        let pid = pid.trim().to_string();
        let vive = Path::new(&format!("/proc/{pid}")).exists();
        // Limpieza antes de los asserts: el hijo de la prueba no debe quedar vivo si uno falla.
        if vive {
            let _ = Command::new("kill").args(["-KILL", &pid]).status();
        }
        assert!(
            matches!(resultado, Err(ErrorCorre::Ejecutor(_))),
            "un error de la espera sale por Err: {resultado:?}"
        );
        assert!(
            !vive,
            "tras el error de la espera el hijo debe estar terminado"
        );
    }

    #[test]
    fn segundo_registro_con_hijo_en_curso_da_error_y_no_lanza() {
        let _serie = serie_de_pruebas();
        let tmp = TempDir::new().expect("tmp");
        let marcador = tmp.path().join("lanzado.txt");
        let hilo = lanzar_hijo_en_curso();
        esperar_hijo_registrado();
        let args = vec![
            "-c".to_string(),
            format!("echo x > '{}'", marcador.display()),
        ];
        let resultado = correr_proceso("sh", &args, Duration::from_secs(10));
        // Limpieza antes de los asserts: el hijo en curso no debe quedar vivo si uno falla.
        terminar_por_senal(|_| {});
        let _ = hilo.join();
        assert!(
            matches!(resultado, Err(ErrorCorre::Ejecutor(_))),
            "un segundo registro da error: {resultado:?}"
        );
        assert!(!marcador.exists(), "el segundo hijo no debe correr");
    }

    #[test]
    fn senal_en_la_ventana_entre_terminando_y_la_copia_no_deja_copia() {
        let _serie = serie_de_pruebas();
        let tmp = TempDir::new().expect("tmp");
        let (ejecutor, credencial) = ejecutor_real(&tmp, false);
        escribir_credencial(&credencial, &credencial_con_mcp(VENCE_LEJOS_MS));
        // Imagen vacía: podman falla sin bajar nada.
        let ejecutor = EjecutorContenedorBuilder {
            imagen: String::new(),
            ..ejecutor
        };
        let repo = tmp.path().join("usuario/.cache/root/corridas/corre-1/repo");
        let estado = tmp
            .path()
            .join("usuario/.cache/root/corridas/corre-1/claude-estado");
        let copia = estado.join(".credentials.json");
        let guardada: SenalGuardada = std::sync::Arc::new(Mutex::new(None));
        let para_costura = std::sync::Arc::clone(&guardada);
        let (estado_senal, copia_senal) = (estado.clone(), copia.clone());
        *COSTURA_ENTRE_TERMINANDO_Y_COPIA.lock().expect("costura") = Some(Box::new(move || {
            let (estado, copia) = (estado_senal.clone(), copia_senal.clone());
            // La señal corre en otro hilo: con el cerrojo tomado, debe esperar a `lanzar`.
            let hilo = std::thread::spawn(move || {
                let mut recibido = None;
                terminar_por_senal(|r| {
                    recibido = Some(r);
                    let _ = crate::borrado::borrar_archivo_dentro(&estado, &copia);
                });
                recibido
            });
            *para_costura.lock().expect("guardada") = Some(hilo);
            std::thread::sleep(Duration::from_millis(100));
        }));
        let _ = ejecutor.lanzar(&lanzamiento(repo, Vec::new()));
        let hilo = guardada
            .lock()
            .expect("guardada")
            .take()
            .expect("la costura lanzó la señal");
        let senal = hilo.join().expect("hilo de la señal");
        assert!(
            matches!(senal, Some(Ok(()))),
            "la señal no falla: {senal:?}"
        );
        assert!(
            fs::symlink_metadata(&copia).is_err(),
            "la señal no debe dejar copia de la credencial"
        );
    }

    #[test]
    fn limpieza_de_la_senal_corre_sin_tomar_el_cerrojo() {
        let _serie = serie_de_pruebas();
        let tmp = TempDir::new().expect("tmp");
        let pid = std::process::id();
        let cuerpo = format!("if [ \"$1\" = ps ]; then echo root-builder-{pid}-0; fi\nexit 0");
        let programa = programa_falso(tmp.path(), &cuerpo);
        let programa = programa.to_str().expect("ruta").to_string();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut quitado = None;
            terminar_por_senal(|_| {
                quitado = Some(quitar_builders_de_este_proceso(
                    &programa,
                    Duration::from_secs(10),
                ));
            });
            let _ = tx.send(quitado);
        });
        // Si la limpieza tomara el cerrojo ya tomado, este recv vencería.
        let quitado = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("la limpieza no debe trabarse con el cerrojo");
        assert!(matches!(quitado, Some(Ok(()))), "{quitado:?}");
        assert_eq!(pid_en_curso(), None, "la limpieza no registra ningún hijo");
    }
}
