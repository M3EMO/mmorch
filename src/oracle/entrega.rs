//! Entrega y escenarios de usuario (Tarea 9d). Corre el producto en una copia del repo, dentro de
//! un contenedor sin red: instala, arranca y pasa cada `[[escenario]]`. La verificación estática
//! del `Dockerfile` reusa la regla de "corre como root" de 9b. La construcción sin red
//! (`construir_dockerfile`, Tarea 9d2) no se conecta a `evaluar`: Tarea 13 la arma. Si un `RUN`
//! pide red, la construcción se repite una vez con red, sin credenciales ni montajes (Tarea 9d3).

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::observe;
use crate::oracle::ErrorOraculo;
use crate::oracle::configs::{self, Instruccion, instrucciones, palabra_clave};
use crate::oracle::contenedor::{
    Ejecucion, HijoPodman, MEMORIA, PIDS_LIMITE, PROXY_FUERA, SUFIJO_SELINUX, SalidaPodman,
    correr_podman as correr_podman_crudo, correr_programa, lanzar_hijo, texto_de_linea_step,
};
use crate::oracle::descarga::{self, ImagenBase, PRELUDIO, Problema};
use crate::oracle::perfiles::{ResultadoSonda, crear_hallazgo, puntuar_hallazgos};
use crate::oracle::tarea::{Escenario, Requisito, Tarea};
use crate::oracle::{CARPETA_TEMPORALES, copiar_repo, infra, temporal};
use crate::tipos::{Dimension, Hallazgo, ItemFaltante, Pendiente};

const SONDA: &str = "entrega";
const DIMENSION: Dimension = Dimension::Entrega;
/// Carpeta con escritura. Todos los `.rc` y `.log` de la entrega van aquí.
const SALIDA: &str = "/salida";
/// Límite de cada escenario, en segundos.
const TIMEOUT_ESCENARIO: u32 = 120;
/// Plazo del host para cada escenario dentro de la imagen. El `timeout` interno corta antes.
const PLAZO_ESCENARIO: Duration = Duration::from_secs(TIMEOUT_ESCENARIO as u64);
/// Código que se registra cuando un escenario supera su plazo (el mismo que devuelve `timeout`).
const CODIGO_TIEMPO: i32 = 124;
/// Código con que podman sale ante un error propio (imagen, contenedor o uso). Es infraestructura.
/// El envoltorio de cada paso convierte el 125 del producto en 1, así que este código siempre es de podman.
const CODIGO_PODMAN: i32 = 125;
/// Plazo de cada paso de la evaluación en el contenedor: preludio, copia, instalar y escenarios de evaluación.
const PLAZO_PASO_ENTREGA: Duration = Duration::from_secs(15 * 60);
/// Contador de los contenedores de evaluación de este proceso.
static CONTADOR_EVALUACION: AtomicU64 = AtomicU64::new(0);

/// Plazo por defecto de cada `podman build`.
const PLAZO_BUILD: Duration = Duration::from_secs(20 * 60);
/// Plazo de las consultas rápidas de podman: inspeccionar, correr `id` y borrar.
const PLAZO_CORTO: Duration = Duration::from_secs(60);
/// Nombre de la sonda y etiqueta del cuerpo del archivo de pendientes (Tarea 9d2).
const TEST_BUILD: &str = "dockerfile::build";
/// Ítem de la rúbrica que mide la construcción. Su ancla 3 pide que construya sin red.
const ITEM_CONSTRUCCION: &str = "entrega.contenedor";
/// Fragmentos (en minúscula) de un `FROM` cuya imagen no está local (`--pull=never`).
const PATRONES_AUSENTE: [&str; 3] = ["image not known", "no such image", "unable to find"];
/// Contador de las etiquetas temporales de `podman build`.
static CONTADOR_BUILD: AtomicU64 = AtomicU64::new(0);
/// Detalle del hallazgo de entrega cuando la imagen solo se construye con red (Tarea 9d3).
const DETALLE_NECESITA_RED: &str = "la imagen necesita red para construirse";
/// Motivo del hallazgo rojo cuando un `RUN` trae `--network` (Tarea 9d4). Sale antes del build.
const DETALLE_RED_EN_RUN: &str = "el Dockerfile pide red en RUN";
/// Fase del `Gate` que registra la repetición del build con red (Tarea 9d3).
const FASE_CONSTRUCCION_RED: &str = "construccion-red";
/// Archivo de eventos de `ROOT_HOME`, el mismo que usan los demás módulos.
const ARCHIVO_EVENTOS: &str = "eventos.jsonl";

/// Lo que sale de construir los Dockerfile del repo sin red (Tarea 9d2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ResultadoDockerfile {
    pub hallazgos: Vec<Hallazgo>,
    pub pendientes: Vec<Pendiente>,
    /// `Some(true)` si la imagen corre sin root. `None` si no hubo imagen que arrancar.
    pub corre_sin_root: Option<bool>,
    /// Verdadero si todos los Dockerfile del repo construyeron.
    pub construida: bool,
    /// Motivos por los que la construcción no da veredicto (Tarea 7i). Si hay alguno, no se construye.
    pub sin_veredicto: Vec<MotivoSinVeredicto>,
    /// Verdadero si la imagen salió del build con red (Tarea 9d3).
    #[serde(default)]
    pub con_red: bool,
}

/// Una causa por la que la construcción no da veredicto, con la línea del `FROM`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MotivoSinVeredicto {
    pub causa: String,
    pub linea: u32,
    /// La imagen tal como está escrita en el `FROM`.
    pub imagen: Option<String>,
}

/// Construye el Dockerfile elegido con `--network none --pull=never` y verifica que corre sin root.
/// Si un paso puede pedir red, lo repite una vez con red (Tarea 9d3). Si la repetición falla, la
/// sonda de red del host decide: fallo del producto, o sin veredicto (Tarea 91). `imagenes` son las
/// imágenes base que vio la fase de descarga: un problema suyo da sin veredicto.
pub fn construir_dockerfile(
    repo: &Path,
    home: &Path,
    tarea: &Tarea,
    imagenes: &[ImagenBase],
) -> Result<ResultadoDockerfile, ErrorOraculo> {
    construir_con(repo, home, tarea, imagenes, PLAZO_BUILD, &PodmanReal).map(|c| c.resultado)
}

/// Resultado de la sonda de red del host (Tarea 91).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Red {
    /// El control corre y la prueba con `pasta` sale 0.
    Hay,
    /// El control corre y la prueba con `pasta` falla: el host no arma la red.
    NoHay,
    /// El control no corre: la sonda no dice nada de la red. Trae el motivo.
    NoCorrio(String),
}

/// Corre `podman` con argumentos. El real llama a `correr_podman`; los tests usan un falso.
pub(crate) trait Podman {
    fn correr(&self, args: &[&str], plazo: Duration) -> Result<SalidaPodman, ErrorOraculo>;

    /// Sonda de red del host (Tarea 91). Solo se pregunta cuando la repetición con red falló.
    fn red_disponible(&self, imagen: &str) -> Red {
        sonda_de_red(|args| self.correr(args, PLAZO_CORTO), imagen)
    }
}

/// Sonda de red de `imagen`. Primero el control sin red: si no corre, `NoCorrio`. Luego la prueba
/// con `pasta`: sale 0 es `Hay`; si no, `NoHay`.
fn sonda_de_red(
    correr: impl Fn(&[&str]) -> Result<SalidaPodman, ErrorOraculo>,
    imagen: &str,
) -> Red {
    let control = correr(&[
        "run",
        PROXY_FUERA,
        "--rm",
        "--network",
        "none",
        "--pull=never",
        "--entrypoint",
        "sh",
        imagen,
        "-c",
        "true",
    ]);
    match control {
        Err(error) => return Red::NoCorrio(error.to_string()),
        Ok(salida) if salida.codigo != 0 => return Red::NoCorrio(motivo_de_control(&salida)),
        Ok(_) => {}
    }
    let prueba = correr(&[
        "run",
        PROXY_FUERA,
        "--rm",
        "--network",
        "pasta:--no-map-gw",
        "--pull=never",
        "--entrypoint",
        "sh",
        imagen,
        "-c",
        "true",
    ]);
    if matches!(prueba, Ok(salida) if salida.codigo == 0) {
        Red::Hay
    } else {
        Red::NoHay
    }
}

/// Motivo de un control que sale con código distinto de 0: la última línea del stderr, o el código.
fn motivo_de_control(salida: &SalidaPodman) -> String {
    ultima_linea_de(&salida.stderr)
        .unwrap_or_else(|| format!("sin salida de error, código {}", salida.codigo))
}

/// Sonda con memoria por imagen. Una imagen con red `Hay` ya no corre la sonda. Lo demás no se guarda.
fn sonda_con_memoria(
    memoria: &Mutex<HashSet<String>>,
    correr: impl Fn(&[&str]) -> Result<SalidaPodman, ErrorOraculo>,
    imagen: &str,
) -> Red {
    let guardada = memoria
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .contains(imagen);
    if guardada {
        return Red::Hay;
    }
    let red = sonda_de_red(correr, imagen);
    if red == Red::Hay {
        memoria
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(imagen.to_string());
    }
    red
}

/// Podman real: lanza el binario con `correr_podman`.
pub(crate) struct PodmanReal;

impl Podman for PodmanReal {
    fn correr(&self, args: &[&str], plazo: Duration) -> Result<SalidaPodman, ErrorOraculo> {
        correr_podman_build("podman", args, plazo)
    }

    /// Las imágenes con red `Hay` quedan por proceso. Una imagen no vale por otra.
    fn red_disponible(&self, imagen: &str) -> Red {
        static MEMORIA_RED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
        let memoria = MEMORIA_RED.get_or_init(|| Mutex::new(HashSet::new()));
        sonda_con_memoria(memoria, |args| self.correr(args, PLAZO_CORTO), imagen)
    }
}

/// Proceso de `podman` que sigue vivo. Root lo consulta con `terminado` y lo cierra con `terminar`.
pub(crate) trait ProcesoVivo {
    fn terminado(&mut self) -> Result<bool, ErrorOraculo>;
    fn terminar(&mut self) -> String;
}

/// Puerta de la sesión de escenarios hacia `podman`. El real lanza el binario. Los tests dan respuestas.
pub(crate) trait Exec {
    fn exec(&self, args: &[&str], plazo: Duration) -> Result<SalidaPodman, ErrorOraculo>;
    /// Lanza `podman` y deja el proceso vivo.
    fn lanzar(&self, args: &[&str]) -> Result<Box<dyn ProcesoVivo>, ErrorOraculo>;
}

/// Ejecutor real: corre `podman` de verdad.
pub(crate) struct ExecReal;

impl Exec for ExecReal {
    fn exec(&self, args: &[&str], plazo: Duration) -> Result<SalidaPodman, ErrorOraculo> {
        correr_podman_crudo(args, plazo).map(recortar_salida)
    }

    fn lanzar(&self, args: &[&str]) -> Result<Box<dyn ProcesoVivo>, ErrorOraculo> {
        lanzar_hijo(args).map(|hijo| Box::new(hijo) as Box<dyn ProcesoVivo>)
    }
}

impl ProcesoVivo for HijoPodman {
    fn terminado(&mut self) -> Result<bool, ErrorOraculo> {
        HijoPodman::terminado(self)
    }

    fn terminar(&mut self) -> String {
        HijoPodman::terminar(self)
    }
}

/// Arranca el contenedor de escenario de una imagen construida (Tarea 9d2). El real usa `podman`.
pub(crate) trait FabricaEscenarios {
    /// Arranca `imagen` y da el corredor de sus escenarios. Un `Err` deja el motivo.
    fn iniciar(
        &self,
        imagen: &str,
        escenarios: Option<&Path>,
    ) -> Result<Box<dyn CorredorDeEscenario + '_>, String>;

    /// Arranca el contenedor de evaluación (parte 2b). Un `Err` deja el motivo. Por defecto no arranca nada.
    fn iniciar_evaluacion(&self, _ejecucion: &Ejecucion) -> Result<Sesion<'_>, String> {
        Err("sin contenedor de evaluación".into())
    }
}

/// Corre un escenario dentro de su contenedor y dice cómo le fue.
pub(crate) trait CorredorDeEscenario {
    fn correr(&self, escenario: &Escenario) -> Result<Prueba, ErrorOraculo>;
}

/// Fábrica real: arranca el contenedor de escenario con `podman`.
pub(crate) struct FabricaReal;

/// Ejecutor real que usa la fábrica. Sin estado, así que basta una instancia estática.
static EXEC_REAL: ExecReal = ExecReal;

impl FabricaEscenarios for FabricaReal {
    fn iniciar(
        &self,
        imagen: &str,
        escenarios: Option<&Path>,
    ) -> Result<Box<dyn CorredorDeEscenario + '_>, String> {
        Sesion::de_imagen(&EXEC_REAL, imagen, escenarios)
            .map(|s| Box::new(s) as Box<dyn CorredorDeEscenario + '_>)
    }

    fn iniciar_evaluacion(&self, ejecucion: &Ejecucion) -> Result<Sesion<'_>, String> {
        crate::oracle::contenedor::validar_rutas(ejecucion).map_err(|e| e.to_string())?;
        let nombre = format!(
            "root-entrega-eval-{}-{}",
            std::process::id(),
            CONTADOR_EVALUACION.fetch_add(1, Ordering::Relaxed)
        );
        let arranque = |comando: &[&str]| {
            crate::oracle::contenedor::argumentos_separado(ejecucion, &nombre, comando)
        };
        Sesion::iniciar(&EXEC_REAL, &nombre, &arranque)
    }
}

impl CorredorDeEscenario for Sesion<'_> {
    fn correr(&self, escenario: &Escenario) -> Result<Prueba, ErrorOraculo> {
        self.correr_escenario(&Entorno::default(), escenario)
    }
}

/// Argumentos de `podman build` (Tarea 9d3). Sin red, `--network none`. Con red, `pasta` (el default de
/// Podman 5) con `--no-map-gw`: el contenedor no alcanza la dirección del host. Sin montajes, secretos,
/// `--build-arg`, `--authfile` ni `--env`. Que el host no sea alcanzable se prueba en la laptop.
fn args_build(red: bool, etiqueta: &str, dockerfile: &str, contexto: &str) -> Vec<String> {
    let red = if red { "pasta:--no-map-gw" } else { "none" };
    vec![
        "build".into(),
        PROXY_FUERA.into(),
        // Se quitan todas las capacidades y se devuelven cinco (Tarea 9d4).
        // Sin ganancia de privilegios y con tope de memoria.
        // `podman build` no acepta `--pids-limit`: el tope de procesos no va aquí.
        "--cap-drop=all".into(),
        // CHOWN, DAC_OVERRIDE y FOWNER: un RUN toca archivos del rootfs con otro dueño.
        // SETUID y SETGID: los gestores de paquetes cambian de usuario y de grupo.
        "--cap-add=CHOWN,DAC_OVERRIDE,FOWNER,SETUID,SETGID".into(),
        "--security-opt=no-new-privileges".into(),
        format!("--memory={MEMORIA}"),
        "--network".into(),
        red.into(),
        "--pull=never".into(),
        "-t".into(),
        etiqueta.into(),
        "-f".into(),
        dockerfile.into(),
        contexto.into(),
    ]
}

/// Corre un `podman build` con los argumentos de `args_build`.
fn correr_build(
    podman: &dyn Podman,
    red: bool,
    etiqueta: &str,
    dockerfile: &str,
    contexto: &str,
    plazo: Duration,
) -> Result<SalidaPodman, ErrorOraculo> {
    let args = args_build(red, etiqueta, dockerfile, contexto);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    podman.correr(&refs, plazo)
}

/// Pendiente del build, con su test y su motivo.
fn pendiente_de_build(motivo: String) -> Pendiente {
    Pendiente {
        test: TEST_BUILD.into(),
        motivo,
        colab: None,
        tasa: None,
    }
}

/// Anota un desenlace de build fallido: el pendiente o el hallazgo del producto.
fn anotar_desenlace(resultado: &mut ResultadoDockerfile, desenlace: Desenlace) {
    match desenlace {
        Desenlace::Pendiente(motivo) | Desenlace::PendienteRed { motivo, .. } => {
            resultado.pendientes.push(pendiente_de_build(motivo));
        }
        Desenlace::Hallazgo(h) => resultado.hallazgos.push(h),
    }
}

/// Registra el `Gate` de la repetición con red (Tarea 9d3): el primer paso que falló sin red y todos los pasos
/// del Dockerfile que piden red (`RUN`, y `ADD` o `COPY` con URL). No dice que todos necesitan red. Va antes del build con red. Si no se puede
/// registrar, no se construye con red.
fn registrar_repeticion(
    home: &Path,
    tarea: &Tarea,
    ruta: &str,
    texto: &str,
    linea: u32,
    cita: &str,
) -> Result<(), ErrorOraculo> {
    let run: Vec<Value> = instrucciones(texto)
        .into_iter()
        .filter(|i| paso_que_necesita_red(&i.logico))
        .map(|i| json!({"linea": i.linea, "cita": i.cita}))
        .collect();
    let mut extra = serde_json::Map::new();
    extra.insert("accion".into(), json!("repetir-con-red"));
    extra.insert("dockerfile".into(), json!(ruta));
    extra.insert(
        "primer_paso_sin_red".into(),
        json!({"linea": linea, "cita": cita}),
    );
    extra.insert("pasos_del_dockerfile".into(), json!(run));
    let evento = observe::evento_gate(
        &format!("entrega-{}", tarea.nombre),
        FASE_CONSTRUCCION_RED,
        extra,
    );
    observe::registrar_en(&home.join(ARCHIVO_EVENTOS), &evento)
        .map_err(|e| infra("registrar la repetición con red", e))
}

/// Lo que sale de construir. Si el Dockerfile construye y la imagen trae `sh`, `imagen` guarda la imagen:
/// los escenarios corren contra ella. Su `Drop` la borra con `podman rmi -f`.
pub struct Construccion<'a> {
    pub resultado: ResultadoDockerfile,
    imagen: Option<ImagenTemporal<'a>>,
    /// Avisos de la elección y de la construcción (Tarea 7j).
    avisos: Vec<String>,
}

/// Aviso cuando hay varios Dockerfile y ninguno se puede elegir.
const AVISO_VARIOS: &str = "varios Dockerfile, ninguno elegido";
/// Aviso cuando la imagen construida no trae `sh`: los escenarios no corren dentro de ella.
const AVISO_SIN_SH: &str =
    "la imagen no tiene sh: los escenarios corren en el contenedor de evaluación";
/// Código con que podman sale cuando el ejecutable existe pero no se puede correr.
const CODIGO_NO_EJECUTABLE: i32 = 126;
/// Código con que podman sale cuando el ejecutable no está en la imagen.
const CODIGO_NO_HALLADO: i32 = 127;

/// Motivos por los que la construcción del Dockerfile elegido no da veredicto, antes de tocar podman:
/// un `FROM` sin valor, o una imagen base que la fase de descarga no pudo traer.
fn motivos_sin_veredicto(
    repo: &Path,
    ruta: &str,
    imagenes: &[ImagenBase],
) -> Result<Vec<MotivoSinVeredicto>, ErrorOraculo> {
    let texto = fs::read_to_string(repo.join(ruta))
        .map_err(|e| infra("leer el Dockerfile para construir", e))?;
    let mut motivos = Vec::new();
    for from in descarga::resolver_froms(&texto) {
        if from.resuelto.is_err() {
            motivos.push(MotivoSinVeredicto {
                causa: "imagen base sin resolver".into(),
                linea: from.linea,
                imagen: Some(from.escrito),
            });
        }
    }
    for imagen in imagenes.iter().filter(|i| i.archivo == ruta) {
        let causa = match &imagen.problema {
            None => continue,
            Some(Problema::ImagenInaccesible) => {
                "imagen base inaccesible: no existe o es privada".to_string()
            }
            Some(Problema::PideCredenciales) => {
                "imagen base: pide credenciales (Root no las pasa)".to_string()
            }
            Some(Problema::Infraestructura(_)) => format!(
                "infraestructura: no se pudo bajar la imagen base {}",
                imagen.resuelta.as_deref().unwrap_or(&imagen.imagen)
            ),
        };
        motivos.push(MotivoSinVeredicto {
            causa,
            linea: imagen.linea,
            imagen: Some(imagen.imagen.clone()),
        });
    }
    Ok(motivos)
}

/// Aviso cuando el build con red no llega a construir por un error de podman: queda el Pendiente.
const AVISO_RED_NO_ARRANCO: &str = "la construcción con red no arrancó";
/// Motivo del Pendiente cuando el host no arma la red de podman (la sonda sale mal).
const MOTIVO_RED_EN_HOST: &str = "la red no se pudo configurar en este host";
/// Marcadores (en minúsculas) de un build con red donde podman no armó la red del `RUN`.
const MARCADORES_RED_NO_CONFIGURADA: [&str; 4] = [
    "setup network",
    "could not find slirp4netns",
    "network namespace can't be configured",
    "did not get container start message from parent",
];

/// Construye el Dockerfile elegido y verifica que corre sin root. Un solo `podman build`.
fn construir_con<'a>(
    repo: &Path,
    home: &Path,
    tarea: &Tarea,
    imagenes: &[ImagenBase],
    plazo: Duration,
    podman: &'a dyn Podman,
) -> Result<Construccion<'a>, ErrorOraculo> {
    let reqs = tarea.requisito.as_slice();
    let mut resultado = ResultadoDockerfile {
        hallazgos: Vec::new(),
        pendientes: Vec::new(),
        corre_sin_root: None,
        construida: false,
        sin_veredicto: Vec::new(),
        con_red: false,
    };
    let mut avisos = Vec::new();
    if let Some(c) = tarea.containerfile.as_deref()
        && ruta_del_repo(repo, c).is_none()
    {
        avisos.push(format!(
            "containerfile {c} no existe: se elige el Dockerfile por el criterio siguiente"
        ));
    }
    let ruta = match elegir_dockerfile(repo, tarea.containerfile.as_deref()) {
        Eleccion::SinDockerfile => {
            return Ok(Construccion {
                resultado,
                imagen: None,
                avisos,
            });
        }
        Eleccion::NingunoElegido(lista) => {
            avisos.push(AVISO_VARIOS.into());
            avisos.push(format!("Dockerfile del repo: {}", lista.join(", ")));
            return Ok(Construccion {
                resultado,
                imagen: None,
                avisos,
            });
        }
        Eleccion::Elegido(ruta) => ruta,
    };
    let motivos = motivos_sin_veredicto(repo, &ruta, imagenes)?;
    if !motivos.is_empty() {
        resultado.sin_veredicto = motivos;
        return Ok(Construccion {
            resultado,
            imagen: None,
            avisos,
        });
    }
    // La copia aísla el repo. El contexto de build sale de ella, nunca del repo original.
    let tmp = home.join(CARPETA_TEMPORALES);
    fs::create_dir_all(&tmp).map_err(|e| infra("directorio temporal de ROOT_HOME", e))?;
    let copia = temporal(&tmp).map_err(|e| infra("directorio temporal de la construcción", e))?;
    let contexto = copia.path().join("repo");
    copiar_repo(repo, &contexto).map_err(|e| infra("copia del repo para construir", e))?;
    // La regla de root la emite la sonda estática: la construcción no la repite.
    let estatica = configs::dockerfile_corre_como_root(repo, reqs).map_err(ErrorOraculo::Infra)?;
    let texto = fs::read_to_string(contexto.join(&ruta))
        .map_err(|e| infra("leer el Dockerfile para construir", e))?;
    // Un RUN con `--network` frena antes de podman (Tarea 9d4): hallazgo rojo, sin build.
    if let Some(i) = instrucciones(&texto)
        .into_iter()
        .find(|i| run_pide_network(&i.logico))
    {
        resultado.hallazgos.push(crear_hallazgo(
            DIMENSION,
            3,
            &ruta,
            i.linea,
            &i.cita,
            DETALLE_RED_EN_RUN,
            SONDA,
            reqs,
        ));
        return Ok(Construccion {
            resultado,
            imagen: None,
            avisos,
        });
    }
    // La imagen temporal se borra al salir de la función, pase lo que pase.
    let imagen = ImagenTemporal {
        etiqueta: etiqueta_unica(),
        podman,
    };
    let df = contexto.join(&ruta).to_string_lossy().into_owned();
    let ctx = contexto.to_string_lossy().into_owned();
    let build = correr_build(podman, false, &imagen.etiqueta, &df, &ctx, plazo)?;
    let mut con_red = false;
    // Un `ADD` con URL baja desde el host aunque el build vaya sin red: que salga 0 no prueba que sea
    // sin red. Ese paso manda la repetición con red, igual que un `RUN` que falla sin red (A11).
    let paso_url = instrucciones(&texto)
        .into_iter()
        .find(|i| paso_con_url(&i.logico))
        .map(|i| (i.linea, i.cita));
    if build.codigo != 0 || paso_url.is_some() {
        let desenlace = match (build.codigo, paso_url) {
            (0, Some((linea, cita))) => Desenlace::PendienteRed {
                motivo: format!("paso con URL que baja desde el host: línea {linea}, {cita}"),
                linea,
                cita,
            },
            _ => clasificar_fallo(&build, &ruta, &texto, reqs, false)?,
        };
        match desenlace {
            Desenlace::PendienteRed {
                linea,
                cita,
                motivo,
            } => {
                // Regla de repetir (Tarea 9d3): una sola vez, con red y el mismo contexto. El Gate va antes.
                registrar_repeticion(home, tarea, &ruta, &texto, linea, &cita)?;
                // Un error de podman en el respaldo (opción de red rechazada, timeout) no borra el
                // Pendiente del primer build: queda con un aviso y nunca da verde.
                let salida = match correr_build(podman, true, &imagen.etiqueta, &df, &ctx, plazo) {
                    Ok(salida) => salida,
                    Err(error) => {
                        avisos.push(format!("{AVISO_RED_NO_ARRANCO}: {error}"));
                        resultado.pendientes.push(pendiente_de_build(motivo));
                        return Ok(Construccion {
                            resultado,
                            imagen: None,
                            avisos,
                        });
                    }
                };
                if salida.codigo == 0 {
                    con_red = true;
                    resultado.hallazgos.push(crear_hallazgo(
                        DIMENSION,
                        1,
                        &ruta,
                        linea,
                        &cita,
                        DETALLE_NECESITA_RED,
                        SONDA,
                        reqs,
                    ));
                } else {
                    match podman.red_disponible(&tarea.imagen) {
                        Red::NoCorrio(motivo_sonda) => {
                            // La sonda no corrió: no dice si el host tiene red. Es infraestructura, no pendiente.
                            resultado.sin_veredicto.push(MotivoSinVeredicto {
                                causa: format!(
                                    "infraestructura: la sonda de red no pudo correr: {motivo_sonda}"
                                ),
                                linea,
                                imagen: None,
                            });
                            avisos.push(format!("{AVISO_RED_NO_ARRANCO}: {motivo_sonda}"));
                            return Ok(Construccion {
                                resultado,
                                imagen: None,
                                avisos,
                            });
                        }
                        Red::NoHay => {
                            // El host no arma la red: no hay veredicto. La última línea del stderr solo es un dato.
                            let dato = red_no_configurada(&salida)
                                .or_else(|| ultima_linea_de(&salida.stderr))
                                .unwrap_or_else(|| "sin salida de error".into());
                            avisos.push(format!("{AVISO_RED_NO_ARRANCO}: {dato}"));
                            resultado
                                .pendientes
                                .push(pendiente_de_build(MOTIVO_RED_EN_HOST.into()));
                            return Ok(Construccion {
                                resultado,
                                imagen: None,
                                avisos,
                            });
                        }
                        Red::Hay => {
                            // Con la red del host, el fallo es del producto: hallazgo. Si podman no arrancó, aviso.
                            match clasificar_fallo(&salida, &ruta, &texto, reqs, true) {
                                Ok(desenlace) => anotar_desenlace(&mut resultado, desenlace),
                                Err(error) => {
                                    avisos.push(format!("{AVISO_RED_NO_ARRANCO}: {error}"));
                                    resultado.pendientes.push(pendiente_de_build(motivo));
                                }
                            }
                            return Ok(Construccion {
                                resultado,
                                imagen: None,
                                avisos,
                            });
                        }
                    }
                }
            }
            otro => {
                anotar_desenlace(&mut resultado, otro);
                return Ok(Construccion {
                    resultado,
                    imagen: None,
                    avisos,
                });
            }
        }
    }
    let con_sh = tiene_sh(podman, &imagen.etiqueta)?;
    let sin_root = corre_sin_root(podman, &imagen.etiqueta, con_sh)?;
    if !sin_root
        && !estatica.iter().any(|h| h.archivo == ruta)
        && let Some((linea, cita)) = ultimo_from(&texto)
    {
        resultado.hallazgos.push(crear_hallazgo(
            DIMENSION,
            3,
            &ruta,
            linea,
            &cita,
            configs::DETALLE_RAIZ,
            SONDA,
            reqs,
        ));
    }
    resultado.construida = true;
    resultado.corre_sin_root = Some(sin_root);
    resultado.con_red = con_red;
    if !con_sh {
        avisos.push(AVISO_SIN_SH.into());
    }
    Ok(Construccion {
        resultado,
        imagen: con_sh.then_some(imagen),
        avisos,
    })
}

/// Imagen temporal de una construcción. Su `Drop` la borra con `podman rmi -f`, pase lo que pase.
struct ImagenTemporal<'a> {
    etiqueta: String,
    podman: &'a dyn Podman,
}

impl Drop for ImagenTemporal<'_> {
    fn drop(&mut self) {
        // Solo borra su propia etiqueta. Un fallo no importa: la imagen quizá nunca se creó.
        let _ = self
            .podman
            .correr(&["rmi", "-f", self.etiqueta.as_str()], PLAZO_CORTO);
    }
}

/// Etiqueta única del proceso: `root-entrega-<pid>-<n>`.
fn etiqueta_unica() -> String {
    let n = CONTADOR_BUILD.fetch_add(1, Ordering::Relaxed);
    format!("root-entrega-{}-{n}", std::process::id())
}

/// Verdadero si la imagen corre sin root. Con `sh`, `id -u` lo confirma. Sin `sh`, vale solo el `USER`.
fn corre_sin_root(podman: &dyn Podman, tag: &str, con_sh: bool) -> Result<bool, ErrorOraculo> {
    let inspeccion = podman.correr(
        &["image", "inspect", "--format", "{{.Config.User}}", tag],
        PLAZO_CORTO,
    )?;
    let usuario = inspeccion.stdout.trim().to_string();
    if !con_sh {
        return Ok(!es_root(&usuario));
    }
    let uid = podman.correr(
        &[
            "run",
            PROXY_FUERA,
            "--rm",
            "--network",
            "none",
            "--pull=never",
            "--entrypoint",
            "id",
            tag,
            "-u",
        ],
        PLAZO_CORTO,
    )?;
    if uid.codigo == 0
        && let Ok(numero) = uid.stdout.trim().parse::<u32>()
    {
        return Ok(numero != 0);
    }
    Ok(!es_root(&usuario))
}

/// Verdadero si la imagen trae `sh`. Sin `sh`, podman sale con 126 o 127 al intentar correrlo.
fn tiene_sh(podman: &dyn Podman, tag: &str) -> Result<bool, ErrorOraculo> {
    let prueba = podman.correr(
        &[
            "run",
            PROXY_FUERA,
            "--rm",
            "--network",
            "none",
            "--pull=never",
            "--entrypoint",
            "sh",
            tag,
            "-c",
            "true",
        ],
        PLAZO_CORTO,
    )?;
    match prueba.codigo {
        CODIGO_NO_EJECUTABLE | CODIGO_NO_HALLADO => Ok(false),
        CODIGO_PODMAN => Err(ErrorOraculo::Infra(
            "podman no pudo probar sh en la imagen construida".into(),
        )),
        _ => Ok(true),
    }
}

/// Verdadero si el `USER` de la imagen es root o 0, o si no define ninguno.
fn es_root(usuario: &str) -> bool {
    let nombre = usuario.split(':').next().unwrap_or("").trim();
    usuario.is_empty() || nombre == "root" || nombre == "0"
}

/// Línea y cita de la última instrucción `FROM` del Dockerfile.
fn ultimo_from(texto: &str) -> Option<(u32, String)> {
    instrucciones(texto)
        .into_iter()
        .rev()
        .find(|i| palabra_clave(&i.logico).0 == "FROM")
        .map(|i| (i.linea, i.cita))
}

/// Desenlace de un build que falló: un pendiente sin cobro, o un hallazgo del producto.
enum Desenlace {
    /// Imagen base del `FROM` que no está local: no se repite.
    Pendiente(String),
    /// Un `RUN` que pide red: la regla de repetir con red (Tarea 9d3) decide qué sigue.
    PendienteRed {
        motivo: String,
        linea: u32,
        cita: String,
    },
    Hallazgo(Hallazgo),
}

/// Separa un fallo de build en imagen base ausente, paso que puede necesitar red, o producto que no
/// construye. Solo cuenta la última línea no vacía del stderr: la escribe podman después de que
/// terminó el proceso del repo. Lo que imprime un `RUN` (stdout, o un `STEP` falso) no decide el paso.
/// Un paso que puede necesitar red (`RUN`, o `ADD`/`COPY` con URL) da `PendienteRed` sin red, sin
/// mirar el texto. Con `con_red`, un fallo de paso es hallazgo: la repetición no da `PendienteRed`.
/// Sin esa línea de paso, ver `sin_paso_en_la_linea_final`.
fn clasificar_fallo(
    build: &SalidaPodman,
    archivo: &str,
    texto: &str,
    reqs: &[Requisito],
    con_red: bool,
) -> Result<Desenlace, ErrorOraculo> {
    let ultima = ultima_linea_de(&build.stderr);
    let Some(linea_paso) = ultima
        .as_deref()
        .filter(|l| l.starts_with(PREFIJO_PASO_FALLIDO))
    else {
        return sin_paso_en_la_linea_final(build, ultima.as_deref(), texto);
    };
    let instrucciones_df = instrucciones(texto);
    let cortes = particiones_del_paso(linea_paso);
    let ubicado = cortes
        .iter()
        .find(|(paso, _)| indice_del_paso(&instrucciones_df, paso).is_some())
        .or(cortes.first());
    let Some((paso, causa)) = ubicado else {
        return sin_paso_en_la_linea_final(build, ultima.as_deref(), texto);
    };
    let Some(indice) = indice_del_paso(&instrucciones_df, paso) else {
        return Ok(Desenlace::Pendiente(format!(
            "sin veredicto: no se ubica en el Dockerfile el paso que falló: {paso}"
        )));
    };
    let i = &instrucciones_df[indice];
    let (linea, cita, kw, red_paso) = (
        i.linea,
        i.cita.clone(),
        palabra_clave(&i.logico).0,
        paso_que_necesita_red(&i.logico),
    );
    if kw == "FROM" {
        // Solo la causa de esta línea: el texto del repo no cuenta como imagen ausente.
        let minuscula = causa.to_lowercase();
        if PATRONES_AUSENTE.iter().any(|p| minuscula.contains(p)) {
            return Ok(Desenlace::Pendiente(format!(
                "imagen base no descargada (fase de descarga no corrió): línea {linea}, {cita}"
            )));
        }
        return Err(ErrorOraculo::Infra(format!(
            "podman no pudo arrancar el FROM de la línea {linea}"
        )));
    }
    if !con_red && red_paso {
        let motivo = format!("no se pudo construir sin red: línea {linea}, {cita}");
        return Ok(Desenlace::PendienteRed {
            motivo,
            linea,
            cita,
        });
    }
    Ok(Desenlace::Hallazgo(crear_hallazgo(
        DIMENSION,
        3,
        archivo,
        linea,
        &cita,
        &format!("la imagen no construye: línea {linea}"),
        SONDA,
        reqs,
    )))
}

/// Prefijo de la línea final de podman cuando un paso del Dockerfile falla.
const PREFIJO_PASO_FALLIDO: &str = "Error: building at STEP \"";

/// Pares (instrucción, causa) que puede traer `Error: building at STEP "<instrucción>": <causa>`.
/// La instrucción puede tener `": ` dentro, así que da cada corte posible, del primero al último.
/// La instrucción sale normalizada. Si la línea no tiene la forma, no da ninguno.
fn particiones_del_paso(linea: &str) -> Vec<(String, String)> {
    let Some(resto) = linea.strip_prefix(PREFIJO_PASO_FALLIDO) else {
        return Vec::new();
    };
    resto
        .match_indices("\": ")
        .map(|(p, _)| (normalizar_texto(&resto[..p]), resto[p + 3..].to_string()))
        .collect()
}

/// Un fallo cuya última línea de podman no trae el paso. Con la imagen base ausente en esa línea,
/// da pendiente. Con algún `STEP` visible, no hay veredicto. Sin ningún `STEP`, podman no empezó.
fn sin_paso_en_la_linea_final(
    build: &SalidaPodman,
    ultima: Option<&str>,
    texto: &str,
) -> Result<Desenlace, ErrorOraculo> {
    if let Some(linea) = ultima {
        let minuscula = linea.to_lowercase();
        if PATRONES_AUSENTE.iter().any(|p| minuscula.contains(p)) {
            let donde = from_de_la_linea(texto, linea)
                .map(|(n, cita)| format!(": línea {n}, {cita}"))
                .unwrap_or_default();
            return Ok(Desenlace::Pendiente(format!(
                "imagen base no descargada (fase de descarga no corrió){donde}"
            )));
        }
    }
    let salida = format!("{}\n{}", build.stdout, build.stderr);
    let hubo_paso =
        build.paso.is_some() || salida.lines().any(|l| texto_de_linea_step(l).is_some());
    if hubo_paso {
        return Ok(Desenlace::Pendiente(
            "sin veredicto: podman no informó el paso que falló".into(),
        ));
    }
    let detalle = ultima.unwrap_or("sin salida de error").to_string();
    Err(ErrorOraculo::Infra(format!(
        "podman no empezó la construcción: {}",
        recortar_cola(detalle)
    )))
}

/// Línea y cita del único `FROM` cuya imagen escrita aparece en `linea`. Si hay varios, o ninguno, da `None`.
fn from_de_la_linea(texto: &str, linea: &str) -> Option<(u32, String)> {
    let mut hallados = descarga::resolver_froms(texto)
        .into_iter()
        .filter(|f| !f.escrito.is_empty() && linea.contains(f.escrito.as_str()))
        .map(|f| f.linea);
    let primero = hallados.next()?;
    if hallados.next().is_some() {
        return None;
    }
    instrucciones(texto)
        .into_iter()
        .find(|i| i.linea == primero)
        .map(|i| (primero, i.cita))
}

/// Texto con los espacios en blanco colapsados a uno. Así se compara lo que imprime buildah con el Dockerfile.
fn normalizar_texto(texto: &str) -> String {
    texto.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Índice de la única instrucción cuyo texto lógico normalizado es `paso`. Si ninguna o varias coinciden, `None`.
fn indice_del_paso(instrucciones_df: &[Instruccion], paso: &str) -> Option<usize> {
    let mut hallados = instrucciones_df
        .iter()
        .enumerate()
        .filter(|(_, i)| normalizar_texto(&i.logico) == paso)
        .map(|(k, _)| k);
    let primero = hallados.next()?;
    hallados.next().is_none().then_some(primero)
}

/// Dato para el aviso: la última línea del stderr, si dice que podman no pudo armar la red del build
/// (`slirp4netns`, `pasta` o `netavark`). No decide nada: la sonda de red decide (Tarea 91).
fn red_no_configurada(build: &SalidaPodman) -> Option<String> {
    ultima_linea_de(&build.stderr)
        .filter(|linea| es_linea_red_no_configurada(linea))
        .map(recortar_cola)
}

fn es_linea_red_no_configurada(linea: &str) -> bool {
    let minuscula = linea.to_lowercase();
    if MARCADORES_RED_NO_CONFIGURADA
        .iter()
        .any(|m| minuscula.contains(m))
    {
        return true;
    }
    let falla = minuscula.contains("not found") || minuscula.contains("failed");
    let red_sin_binario =
        (minuscula.contains("slirp4netns") || minuscula.contains("pasta")) && falla;
    red_sin_binario || (minuscula.contains("netavark") && minuscula.contains("error"))
}

/// Verdadero si el paso `ADD` o `COPY` trae una fuente remota (A11). Buildah la baja desde el host, así
/// que cuenta como paso que necesita red, igual que un `RUN` (decisión 27).
fn paso_con_url(logico: &str) -> bool {
    !configs::fuentes_remotas(logico).is_empty()
}

/// Verdadero si el paso necesita red al construirse: un `RUN`, o un `ADD`/`COPY` con una URL (A11).
fn paso_que_necesita_red(logico: &str) -> bool {
    palabra_clave(logico).0 == "RUN" || paso_con_url(logico)
}

/// Verdadero si el `RUN` trae `--network` entre sus flags (Tarea 9d4). Acepta `--network=x` y
/// `--network x`, con la palabra clave en cualquier caja. Solo mira las flags de antes del comando.
fn run_pide_network(logico: &str) -> bool {
    let (kw, resto) = palabra_clave(logico);
    if kw != "RUN" {
        return false;
    }
    for token in resto.split_whitespace() {
        let minuscula = token.to_lowercase();
        if minuscula == "--network" || minuscula.starts_with("--network=") {
            return true;
        }
        // Del primer token que no es flag en adelante, son el comando: no cuentan.
        if !token.starts_with("--") {
            break;
        }
    }
    false
}

/// Dónde corrió un escenario: en la imagen construida, o en el contenedor de evaluación.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LugarEscenario {
    Imagen,
    ContenedorEvaluacion,
}

/// Dockerfile que se construye (Tarea 7j). Stub de la fase roja: todavía no elige nada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Eleccion {
    Elegido(String),
    NingunoElegido(Vec<String>),
    SinDockerfile,
}

/// Elige el Dockerfile a construir (Tarea 7j), por este orden: el `containerfile` de `tarea.toml` si
/// existe; `Dockerfile` o `Containerfile` en la raíz; el único Dockerfile del repo. Si hay varios, no elige.
pub fn elegir_dockerfile(repo: &Path, containerfile: Option<&str>) -> Eleccion {
    if let Some(ruta) = containerfile.and_then(|c| ruta_del_repo(repo, c)) {
        return Eleccion::Elegido(ruta);
    }
    let todos = dockerfiles_del_repo(repo);
    if todos.is_empty() {
        return Eleccion::SinDockerfile;
    }
    for raiz in ["Dockerfile", "Containerfile"] {
        if todos.iter().any(|t| t == raiz) {
            return Eleccion::Elegido(raiz.into());
        }
    }
    if todos.len() == 1 {
        return Eleccion::Elegido(todos[0].clone());
    }
    Eleccion::NingunoElegido(todos)
}

/// Carpetas que no entran al buscar Dockerfile. Las mismas que la sonda de `configs`.
const CARPETAS_SIN_DOCKERFILE: [&str; 4] = ["target", ".git", "node_modules", "vendor"];

/// Rutas relativas (con `/`) de los Dockerfile del repo, en orden alfabético. Usa
/// `configs::es_nombre_dockerfile`, el mismo reconocedor que la sonda de `configs`.
pub fn dockerfiles_del_repo(repo: &Path) -> Vec<String> {
    let mut salida = Vec::new();
    recorrer_dockerfiles(repo, repo, &mut salida);
    salida.sort();
    salida
}

fn recorrer_dockerfiles(repo: &Path, dir: &Path, salida: &mut Vec<String>) {
    let Ok(entradas) = fs::read_dir(dir) else {
        return;
    };
    for entrada in entradas.flatten() {
        // `file_type` no sigue enlaces simbólicos: un enlace no entra ni causa bucles.
        let Ok(tipo) = entrada.file_type() else {
            continue;
        };
        let nombre = entrada.file_name().to_string_lossy().into_owned();
        let ruta = entrada.path();
        if tipo.is_dir() {
            if !CARPETAS_SIN_DOCKERFILE.contains(&nombre.as_str()) {
                recorrer_dockerfiles(repo, &ruta, salida);
            }
        } else if tipo.is_file() && configs::es_nombre_dockerfile(&nombre) {
            let rel = ruta.strip_prefix(repo).unwrap_or(&ruta);
            salida.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
}

/// Ruta normalizada si `ruta` es un archivo regular del repo. No sale del repo ni sigue enlaces.
pub(crate) fn ruta_del_repo(repo: &Path, ruta: &str) -> Option<String> {
    let mut partes = Vec::new();
    for componente in Path::new(ruta).components() {
        match componente {
            Component::Normal(parte) => partes.push(parte.to_string_lossy().into_owned()),
            Component::CurDir => {}
            // Raíz, prefijo o `..`: la ruta sale del repo.
            _ => return None,
        }
    }
    if partes.is_empty() {
        return None;
    }
    let relativa = partes.join("/");
    let meta = fs::symlink_metadata(repo.join(&relativa)).ok()?;
    meta.file_type().is_file().then_some(relativa)
}

/// Resultado de la sonda de entrega: el puntaje de la dimensión y el estado de cada escenario.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultadoEntrega {
    pub sonda: ResultadoSonda,
    /// Id del escenario -> `true` si su `cmd` salió 0. Un escenario pendiente no está en el mapa.
    pub escenarios: BTreeMap<String, bool>,
    /// Id del escenario -> el lugar donde corrió.
    pub lugar_escenarios: BTreeMap<String, LugarEscenario>,
    /// Escenarios que no se pudieron medir (p. ej. piden red con `--network none`).
    pub pendientes: Vec<Pendiente>,
    /// Motivos por los que la construcción no dio veredicto. Vacío si la construcción sí dio.
    pub sin_veredicto: Vec<MotivoSinVeredicto>,
    /// Avisos de ejecución, p. ej. una imagen sin `sh` que obligó a usar el contenedor de evaluación.
    pub avisos: Vec<String>,
    /// Ítems de Entrega que no se midieron, con su motivo. Un build pendiente va aquí como `entrega.contenedor`.
    pub faltantes: Vec<ItemFaltante>,
}

/// Construye los Dockerfile, corre la sonda de entrega y los escenarios. Una sola construcción.
/// Los escenarios corren contra la imagen si construye; si no, en el contenedor de evaluación.
pub fn entregar(
    repo: &Path,
    tarea: &Tarea,
    home: &Path,
    imagenes: &[ImagenBase],
) -> Result<ResultadoEntrega, ErrorOraculo> {
    entregar_con(repo, tarea, home, imagenes, &PodmanReal, &FabricaReal)
}

/// Como `entregar`, con el podman y la fábrica de escenarios que se pasen. Producción pasa los reales.
pub(crate) fn entregar_con(
    repo: &Path,
    tarea: &Tarea,
    home: &Path,
    imagenes: &[ImagenBase],
    podman: &dyn Podman,
    fabrica: &dyn FabricaEscenarios,
) -> Result<ResultadoEntrega, ErrorOraculo> {
    let reqs = tarea.requisito.as_slice();
    if !tarea.escenario.is_empty() && !tarea.dir.is_dir() {
        return Err(ErrorOraculo::Infra(format!(
            "no existe la carpeta de la tarea {}",
            tarea.dir.display()
        )));
    }
    // Un escenario que nombra `/oraculo` necesita sus scripts en `escenarios/` (M15).
    // Los que son comandos en línea no montan nada: la carpeta es opcional para ellos.
    let escenarios = tarea.dir.join("escenarios");
    let usa_oraculo = tarea
        .escenario
        .iter()
        .any(|e| escenario_usa_oraculo(&e.cmd));
    if usa_oraculo && !escenarios.is_dir() {
        return Err(ErrorOraculo::Infra(format!(
            "no existe la carpeta de escenarios {}",
            escenarios.display()
        )));
    }
    let montaje = montaje_de_escenarios(tarea);
    let Construccion {
        resultado: construccion,
        imagen,
        avisos: avisos_construccion,
    } = construir_con(repo, home, tarea, imagenes, PLAZO_BUILD, podman)?;
    let mut avisos = avisos_construccion;
    // El contenedor de escenario sale de la imagen construida. Sin él, van al de evaluación.
    let contenedor = match &imagen {
        Some(img) if !tarea.escenario.is_empty() => {
            match fabrica.iniciar(&img.etiqueta, montaje.as_deref()) {
                Ok(c) => Some(c),
                Err(motivo) => {
                    avisos.push(format!(
                        "escenarios en el contenedor de evaluación: {motivo}"
                    ));
                    None
                }
            }
        }
        _ => None,
    };
    let mut resultado = sonda_entrega_con(fabrica, repo, tarea, home, contenedor.is_none());
    if let Some(c) = &contenedor {
        for escenario in &tarea.escenario {
            let prueba = c.correr(escenario)?;
            aplicar_prueba(
                &mut resultado.escenarios,
                &mut resultado.sonda.hallazgos,
                &mut resultado.pendientes,
                escenario,
                prueba,
                reqs,
            );
            resultado
                .lugar_escenarios
                .insert(escenario.id.clone(), LugarEscenario::Imagen);
        }
    }
    let construida = construccion.construida;
    let causas_build: Vec<String> = construccion
        .pendientes
        .iter()
        .map(|p| p.motivo.clone())
        .collect();
    resultado.faltantes = construccion
        .pendientes
        .iter()
        .map(|p| ItemFaltante {
            item: ITEM_CONSTRUCCION.into(),
            motivo: p.motivo.clone(),
        })
        .collect();
    resultado.pendientes.extend(construccion.pendientes);
    resultado.sonda.hallazgos.extend(construccion.hallazgos);
    resultado.sin_veredicto = construccion.sin_veredicto;
    resultado.avisos = avisos;
    // Sin construcción, la entrega no da verde: el resto de la evaluación sigue con su veredicto.
    // Un build pendiente sin hallazgos deja la sonda sin veredicto. Con hallazgos medidos, puntúan.
    let causas: Vec<String> = resultado
        .sin_veredicto
        .iter()
        .map(|m| m.causa.clone())
        .collect();
    if !causas.is_empty() {
        sin_veredicto_sonda(&mut resultado.sonda, &causas.join("; "));
    } else if !causas_build.is_empty() && resultado.sonda.hallazgos.is_empty() {
        sin_veredicto_sonda(&mut resultado.sonda, &causas_build.join("; "));
    } else if resultado.sonda.motivo.is_none() {
        if medido_algo(tarea, &resultado, construida) {
            resultado.sonda.valor = Some(puntuar_hallazgos(&resultado.sonda.hallazgos, reqs));
        } else {
            sin_veredicto_sonda(&mut resultado.sonda, MOTIVO_SIN_ITEMS);
        }
    }
    Ok(resultado)
}

/// Motivo de la sonda cuando la entrega no midió ningún ítem.
const MOTIVO_SIN_ITEMS: &str = "entrega sin ítems medidos";

/// Deja la sonda sin veredicto. El motivo nuevo va después del previo, si lo hay.
fn sin_veredicto_sonda(sonda: &mut ResultadoSonda, causas: &str) {
    sonda.valor = None;
    sonda.motivo = Some(match sonda.motivo.take() {
        Some(previo) => format!("{previo}; {causas}"),
        None => causas.to_string(),
    });
}

/// Verdadero si la entrega midió algo propio. Cuenta: archivos de `[entrega]`, escenarios con salida,
/// un Dockerfile construido o evaluado (con hallazgos). Los requisitos son de Completitud: no cuentan.
fn medido_algo(tarea: &Tarea, resultado: &ResultadoEntrega, construida: bool) -> bool {
    let hay_archivos = tarea
        .entrega
        .as_ref()
        .is_some_and(|e| !e.archivos.is_empty());
    hay_archivos
        || !resultado.escenarios.is_empty()
        || construida
        || !resultado.sonda.hallazgos.is_empty()
}

/// Resultado de un escenario corrido dentro de la imagen.
pub(crate) enum Prueba {
    Paso,
    Fallo {
        codigo: i32,
        cita: String,
    },
    /// No se puede medir sin red: el contenedor corre con `--network none`.
    Pendiente(String),
}

/// Bytes de salida de un proceso que conserva la entrega: solo la cola. Mismo valor que `descarga`.
const COLA: usize = 4000;

/// Corre `podman` para un build y deja la cola de 1 MiB de cada flujo, sin recortar a `COLA`.
/// `clasificar_fallo` necesita la línea final de podman aunque el error de antes sea largo. Lo que se muestra se recorta
/// en su punto de salida. `programa` es `podman` en producción; los tests pasan `sh`.
fn correr_podman_build(
    programa: &str,
    args: &[&str],
    plazo: Duration,
) -> Result<SalidaPodman, ErrorOraculo> {
    correr_programa(programa, args, plazo)
}

/// Deja los últimos `COLA` bytes de `texto`. Si el corte cae dentro de un carácter, avanza al siguiente.
fn recortar_cola(mut texto: String) -> String {
    if texto.len() > COLA {
        let mut inicio = texto.len() - COLA;
        while !texto.is_char_boundary(inicio) {
            inicio += 1;
        }
        texto.drain(..inicio);
    }
    texto
}

/// Recorta stdout y stderr de una salida de podman a su cola.
fn recortar_salida(salida: SalidaPodman) -> SalidaPodman {
    SalidaPodman {
        codigo: salida.codigo,
        stdout: recortar_cola(salida.stdout),
        stderr: recortar_cola(salida.stderr),
        paso: salida.paso,
    }
}

/// Datos de entorno que una sesión aplica a sus `exec`. Vacío para la imagen: sin directorio ni activación.
#[derive(Default, Clone)]
pub(crate) struct Entorno {
    /// Directorio de trabajo dentro del contenedor (`-w`). Sin valor, el de la imagen.
    dir: Option<String>,
    /// Línea de `sh` que corre antes del comando, por ejemplo `export A=1;`. Vacía no hace nada.
    activar: String,
}

/// Activación de Rust para cada paso de la evaluación. `podman exec` no conserva el entorno entre pasos.
const ACTIVAR_RUST: &str = "export CARGO_HOME=/tmp/cargo-home;";
/// Activación de Python: el venv del preludio, o el `--target` si el venv no funcionó.
const ACTIVAR_PYTHON: &str = "if [ -f /tmp/venv-eval/bin/activate ]; then . /tmp/venv-eval/bin/activate; else export PYTHONPATH=/tmp/pydeps PATH=/tmp/pydeps/bin:$PATH; fi;";

/// Entorno de los pasos de la evaluación: `/salida/work` con entrega, `/repo` sin ella, y la activación del lenguaje.
fn entorno_de_evaluacion(tarea: &Tarea) -> Entorno {
    let dir = if tarea.entrega.is_some() {
        format!("{SALIDA}/work")
    } else {
        "/repo".to_string()
    };
    let activar = match tarea.lenguaje.as_str() {
        "rust" => ACTIVAR_RUST,
        "python" => ACTIVAR_PYTHON,
        _ => "",
    };
    Entorno {
        dir: Some(dir),
        activar: activar.to_string(),
    }
}

/// PATH del sistema. Va ANTES de la activación de Root en cada paso. Así los dirs de Root (venv, pydeps)
/// quedan primero, luego el sistema, luego el resto. Un archivo del producto en `bin/` no tapa una
/// herramienta del sistema, y el venv de Root sí tapa al `python` del sistema.
const PATH_DEL_SISTEMA: &str = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";

/// Línea de `sh` que envuelve cada paso: el PATH del sistema, la activación (si hay), el `cmd` como `$1`
/// y el código de salida. No usa `exec`: el `sh` de afuera vive para leer `$?`.
/// `ruta_sh` y `ruta_timeout` son rutas absolutas dentro de la imagen. Así la activación de Root no
/// tapa el `sh` ni el `timeout`. Con `ruta_timeout`, el `timeout 120` va DENTRO, antes del `sh` del `cmd`.
/// Así el `sh` de afuera no se muere con el plazo y siempre llega a leer `$?`.
/// El 125 del producto pasa a 1. Así un 125 que sale de `podman exec` siempre es de podman.
fn envoltorio_de(activar: &str, ruta_sh: &str, ruta_timeout: Option<&str>) -> String {
    let previa = if activar.is_empty() {
        String::new()
    } else {
        format!("{activar} ")
    };
    let plazo = ruta_timeout
        .map(|ruta| format!("{ruta} {TIMEOUT_ESCENARIO} "))
        .unwrap_or_default();
    format!(
        "export PATH={PATH_DEL_SISTEMA}:$PATH; {previa}{plazo}{ruta_sh} -c \"$1\"; s=$?; [ \"$s\" -eq {CODIGO_PODMAN} ] && s=1; exit \"$s\""
    )
}

/// Verdadero si `ruta` es absoluta y no vive bajo `/tmp`, `/salida` ni `/repo`. Compara por componente:
/// `/tmpx` no está bajo `/tmp`. Un `..` en la ruta la vuelve inválida.
fn ruta_de_sistema_valida(ruta: &str) -> bool {
    let ruta = Path::new(ruta);
    let absoluta = ruta.is_absolute();
    let sin_saltos = !ruta
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir));
    let propias = [Path::new("/tmp"), Path::new(SALIDA), Path::new("/repo")];
    absoluta && sin_saltos && !propias.iter().any(|raiz| ruta.starts_with(raiz))
}

/// Rutas de `sh` y `timeout` dentro del contenedor, con una sola llamada. Sin `sh` válido da error.
/// Un `timeout` vacío o inválido da `None`: el plazo lo pone Root desde afuera.
fn rutas_de_la_imagen(exec: &dyn Exec, nombre: &str) -> Result<(String, Option<String>), String> {
    // Con el PATH del sistema y nada activado: el `PATH` de la imagen no decide qué `sh` se guarda.
    let sonda = format!(
        r#"PATH={PATH_DEL_SISTEMA}; printf "sh=%s\ntimeout=%s\n" "$(command -v sh)" "$(command -v timeout)""#
    );
    let texto = match exec.exec(&["exec", nombre, "/bin/sh", "-c", &sonda], PLAZO_CORTO) {
        Ok(s) if s.codigo == 0 => s.stdout,
        _ => return Err("la imagen no trae sh".into()),
    };
    let campo = |clave: &str| {
        texto
            .lines()
            .find_map(|l| l.strip_prefix(clave))
            .unwrap_or("")
            .trim()
            .to_string()
    };
    let sh = campo("sh=");
    if sh.is_empty() {
        return Err("la imagen no trae sh".into());
    }
    if !ruta_de_sistema_valida(&sh) {
        return Err(format!("la ruta de sh no es válida: {sh}"));
    }
    let timeout = campo("timeout=");
    let ruta_timeout = (!timeout.is_empty() && ruta_de_sistema_valida(&timeout)).then_some(timeout);
    Ok((sh, ruta_timeout))
}

/// Contenedor vivo, donde los pasos corren con `podman exec`. Cada paso lleva su `Entorno`.
/// Su `Drop` lo borra con `podman rm -f -t 0`, pase lo que pase.
pub(crate) struct Sesion<'a> {
    nombre: String,
    exec: &'a dyn Exec,
    /// Ruta absoluta de `sh` en la imagen, de `command -v sh` dentro del contenedor.
    ruta_sh: String,
    /// Ruta absoluta de `timeout`, o `None` si la imagen no lo trae o su ruta no es válida.
    /// Si es `None`, el plazo lo pone Root desde afuera.
    ruta_timeout: Option<String>,
}

/// Contador de los contenedores de escenario de este proceso.
static CONTADOR_ESCENARIO: AtomicU64 = AtomicU64::new(0);

impl<'a> Sesion<'a> {
    /// Arranca un contenedor de la imagen construida. Sus argumentos salen de `args_arranque`.
    pub(crate) fn de_imagen(
        exec: &'a dyn Exec,
        imagen: &str,
        escenarios: Option<&Path>,
    ) -> Result<Self, String> {
        let nombre = format!(
            "root-entrega-esc-{}-{}",
            std::process::id(),
            CONTADOR_ESCENARIO.fetch_add(1, Ordering::Relaxed)
        );
        let arranque = |comando: &[&str]| args_arranque(imagen, escenarios, &nombre, comando);
        Self::iniciar(exec, &nombre, &arranque)
    }

    /// Arranca el contenedor con `sleep` o, si no está, con `tail -f /dev/null`. `arranque` da los
    /// argumentos de `run` para cada comando. Sin `sh` no sirve para escenarios: devuelve el motivo.
    pub(crate) fn iniciar(
        exec: &'a dyn Exec,
        nombre: &str,
        arranque: &dyn Fn(&[&str]) -> Vec<String>,
    ) -> Result<Self, String> {
        let mut motivos = Vec::new();
        for comando in [vec!["sleep", "600"], vec!["tail", "-f", "/dev/null"]] {
            let args = arranque(comando.as_slice());
            let refs: Vec<&str> = args.iter().map(String::as_str).collect();
            let salida = exec.exec(&refs, PLAZO_CORTO);
            let arranco = salida.as_ref().is_ok_and(|s| s.codigo == 0);
            if arranco && corriendo(exec, nombre) {
                return match rutas_de_la_imagen(exec, nombre) {
                    Ok((ruta_sh, ruta_timeout)) => Ok(Sesion {
                        nombre: nombre.to_string(),
                        exec,
                        ruta_sh,
                        ruta_timeout,
                    }),
                    Err(motivo) => {
                        quitar(exec, nombre);
                        Err(motivo)
                    }
                };
            }
            motivos.push(format!("{}: {}", comando.join(" "), motivo_de_run(&salida)));
            quitar(exec, nombre);
        }
        Err(format!(
            "la imagen no trae sleep ni tail: no arranca el contenedor ({})",
            motivos.join("; ")
        ))
    }

    /// Argumentos de `podman exec` para `cmd`. Todo paso corre dentro de un `sh` envoltorio: primero el
    /// PATH del sistema, después la activación (si hay) y el `cmd` como `$1`. El `sh` y el `timeout` del
    /// envoltorio van por ruta absoluta. Con `con_timeout` y si la imagen trae `timeout`, el `timeout` va
    /// dentro del envoltorio. El envoltorio convierte el 125 del producto en 1 (ver `CODIGO_PODMAN`).
    pub(crate) fn paso_args(&self, entorno: &Entorno, cmd: &str, con_timeout: bool) -> Vec<String> {
        let mut args: Vec<String> = vec!["exec".into()];
        if let Some(dir) = &entorno.dir {
            args.extend(["-w".to_string(), dir.clone()]);
        }
        args.push(self.nombre.clone());
        let ruta_timeout = if con_timeout {
            self.ruta_timeout.as_deref()
        } else {
            None
        };
        args.extend([
            "sh".to_string(),
            "-c".to_string(),
            envoltorio_de(&entorno.activar, &self.ruta_sh, ruta_timeout),
            "_".to_string(),
            cmd.to_string(),
        ]);
        args
    }

    /// Corre `cmd` con `podman exec` y da la salida que dio el ejecutor.
    pub(crate) fn paso(
        &self,
        entorno: &Entorno,
        cmd: &str,
        plazo: Duration,
        con_timeout: bool,
    ) -> Result<SalidaPodman, ErrorOraculo> {
        let args = self.paso_args(entorno, cmd, con_timeout);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        self.exec.exec(&refs, plazo)
    }

    /// Lanza `cmd` dentro del contenedor y lo deja vivo.
    pub(crate) fn lanzar(
        &self,
        entorno: &Entorno,
        cmd: &str,
    ) -> Result<Box<dyn ProcesoVivo>, ErrorOraculo> {
        let args = self.paso_args(entorno, cmd, false);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        self.exec.lanzar(&refs)
    }

    /// Corre el `cmd` del escenario con el límite de `timeout` dentro del contenedor y dice cómo le fue.
    pub(crate) fn correr_escenario(
        &self,
        entorno: &Entorno,
        escenario: &Escenario,
    ) -> Result<Prueba, ErrorOraculo> {
        // Root no corre los escenarios que piden red: quedan pendientes sin ejecutarse.
        if escenario.red {
            return Ok(Prueba::Pendiente("el escenario pide red".into()));
        }
        // El plazo de Root corta el `podman exec` a los 120 s, con o sin `timeout` en la imagen.
        let salida = match self.paso(entorno, &escenario.cmd, PLAZO_ESCENARIO, true) {
            Ok(salida) => salida,
            Err(ErrorOraculo::Infra(motivo)) if motivo.contains("superó") => {
                return Ok(Prueba::Fallo {
                    codigo: CODIGO_TIEMPO,
                    cita: format!("el escenario superó {TIMEOUT_ESCENARIO} s"),
                });
            }
            Err(otro) => return Err(otro),
        };
        // Un 125 aquí es de podman: el envoltorio ya convirtió el 125 del producto en 1.
        if salida.codigo == CODIGO_PODMAN {
            return Err(ErrorOraculo::Infra(format!(
                "podman no pudo correr el escenario {}",
                escenario.id
            )));
        }
        if salida.codigo == 0 {
            return Ok(Prueba::Paso);
        }
        let texto = format!("{}\n{}", salida.stdout, salida.stderr);
        let cita = ultima_linea_de(&texto).unwrap_or_else(|| escenario.id.clone());
        Ok(Prueba::Fallo {
            codigo: salida.codigo,
            cita,
        })
    }
}

impl Drop for Sesion<'_> {
    fn drop(&mut self) {
        quitar(self.exec, &self.nombre);
    }
}

/// Motivo corto de un `run` que no dejó el contenedor corriendo. Toma la última línea no vacía del
/// stderr, o del stdout si el stderr está vacío. Recorta a 300 caracteres.
fn motivo_de_run(salida: &Result<SalidaPodman, ErrorOraculo>) -> String {
    let texto = match salida {
        Ok(s) if s.codigo == 0 => return "no quedó corriendo".into(),
        Ok(s) if s.stderr.trim().is_empty() => s.stdout.clone(),
        Ok(s) => s.stderr.clone(),
        Err(ErrorOraculo::Infra(motivo)) => motivo.clone(),
        Err(otro) => otro.to_string(),
    };
    let linea = texto
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("sin salida");
    linea.trim().chars().take(300).collect()
}

/// Borra un contenedor por nombre, sin esperar la parada. Un fallo no importa: ya hay un resultado.
fn quitar(exec: &dyn Exec, nombre: &str) {
    let _ = exec.exec(&["rm", "-f", "-t", "0", nombre], PLAZO_CORTO);
}

/// Un escenario usa `/oraculo` si su `cmd` nombra esa ruta: sus scripts viven en `escenarios/`.
pub fn escenario_usa_oraculo(cmd: &str) -> bool {
    cmd.contains("/oraculo")
}

/// Carpeta `escenarios/` que el contenedor de escenario monta, o `None` si no monta nada. Solo monta si
/// algún `cmd` usa `/oraculo` (M15): los comandos en línea no la necesitan, aunque exista.
fn montaje_de_escenarios(tarea: &Tarea) -> Option<PathBuf> {
    let escenarios = tarea.dir.join("escenarios");
    let usa_oraculo = tarea
        .escenario
        .iter()
        .any(|e| escenario_usa_oraculo(&e.cmd));
    (usa_oraculo && escenarios.is_dir()).then_some(escenarios)
}

/// Argumentos de `podman run -d` de un escenario: sin red, sin el proxy del host (M4) y con `comando` como
/// entrypoint. Los escenarios son scripts de la tarea: `oraculo` entra en solo lectura, sin copiarse a la imagen.
/// Sin `escenarios` no monta nada: los comandos en línea no lo necesitan.
fn args_arranque(
    imagen: &str,
    escenarios: Option<&Path>,
    nombre: &str,
    comando: &[&str],
) -> Vec<String> {
    let Some((entrada, resto)) = comando.split_first() else {
        return Vec::new();
    };
    let mut args: Vec<String> = vec![
        "run".into(),
        "-d".into(),
        PROXY_FUERA.into(),
        "--network".into(),
        "none".into(),
        "--pull=never".into(),
        "--name".into(),
        nombre.into(),
    ];
    // Sin capacidades, sin ganancia de privilegios y con topes de procesos y memoria (Tarea 9d4).
    args.extend(["--cap-drop=all", "--security-opt=no-new-privileges"].map(String::from));
    args.push(format!("--pids-limit={PIDS_LIMITE}"));
    args.push(format!("--memory={MEMORIA}"));
    // Solo `escenarios/` entra al contenedor: `oculto/` y `tarea.toml` quedan afuera (M15).
    if let Some(dir) = escenarios {
        args.push("-v".into());
        args.push(format!(
            "{}:/oraculo/escenarios:ro,{SUFIJO_SELINUX}",
            dir.display()
        ));
    }
    args.extend(["--entrypoint".into(), (*entrada).into(), imagen.into()]);
    args.extend(resto.iter().map(|a| a.to_string()));
    args
}

/// Verdadero si el contenedor sigue corriendo.
fn corriendo(exec: &dyn Exec, nombre: &str) -> bool {
    exec.exec(
        &[
            "container",
            "inspect",
            "--format",
            "{{.State.Running}}",
            nombre,
        ],
        PLAZO_CORTO,
    )
    .is_ok_and(|s| s.stdout.trim() == "true")
}

/// Última línea no vacía de un texto.
fn ultima_linea_de(texto: &str) -> Option<String> {
    texto
        .lines()
        .map(str::trim)
        .rev()
        .find(|l| !l.is_empty())
        .map(str::to_string)
}

/// Texto descriptivo de los pasos de la evaluación. `memo.rs` lo usa como clave de texto: no es un script.
#[cfg(test)]
pub(crate) fn construir_script_con(tarea: &Tarea, escenarios: bool) -> String {
    let mut pasos: Vec<String> = preludio_de(&tarea.lenguaje).into_iter().collect();
    if let Some(entrega) = &tarea.entrega {
        pasos.push(format!("cp -a /repo {SALIDA}/work"));
        pasos.push(entrega.instalar.clone());
        pasos.push(entrega.correr.clone());
    }
    if escenarios {
        pasos.extend(tarea.escenario.iter().map(|e| e.cmd.clone()));
    }
    pasos.push(format!(
        "evaluación de escenarios: {}",
        if escenarios { "sí" } else { "no" }
    ));
    pasos.join("\n")
}

/// Archivos de `archivos` que no existen en `repo`.
pub fn archivos_faltantes(repo: &Path, archivos: &[String]) -> Vec<String> {
    archivos
        .iter()
        .filter(|a| !repo.join(a).exists())
        .cloned()
        .collect()
}

/// Entrega sin veredicto por un error de infraestructura de la construcción: el motivo va a la sonda.
pub(crate) fn entrega_sin_veredicto(motivo: String) -> ResultadoEntrega {
    ResultadoEntrega {
        sonda: ResultadoSonda {
            dimension: DIMENSION,
            valor: None,
            motivo: Some(motivo),
            hallazgos: Vec::new(),
        },
        escenarios: BTreeMap::new(),
        lugar_escenarios: BTreeMap::new(),
        pendientes: Vec::new(),
        sin_veredicto: Vec::new(),
        avisos: Vec::new(),
        faltantes: Vec::new(),
    }
}

/// Sonda de entrega con el contenedor de evaluación para todo. Un `Err` de podman es infraestructura.
pub fn sonda_entrega(repo: &Path, tarea: &Tarea, home: &Path) -> ResultadoEntrega {
    sonda_entrega_con(&FabricaReal, repo, tarea, home, true)
}

/// Sonda de entrega. `escenarios_eval` en falso deja los escenarios para la imagen construida.
fn sonda_entrega_con(
    fabrica: &dyn FabricaEscenarios,
    repo: &Path,
    tarea: &Tarea,
    home: &Path,
    escenarios_eval: bool,
) -> ResultadoEntrega {
    let reqs = tarea.requisito.as_slice();
    let eval: &[Escenario] = if escenarios_eval {
        &tarea.escenario
    } else {
        &[]
    };
    let mut hallazgos = Vec::new();
    let mut motivos: Vec<String> = Vec::new();
    let archivos = tarea
        .entrega
        .as_ref()
        .map_or(&[][..], |e| e.archivos.as_slice());
    for faltante in archivos_faltantes(repo, archivos) {
        let detalle = format!("falta {faltante}");
        hallazgos.push(crear_hallazgo(
            DIMENSION, 2, &faltante, 0, &faltante, &detalle, SONDA, reqs,
        ));
    }
    match configs::dockerfile_corre_como_root(repo, reqs) {
        Ok(hs) => hallazgos.extend(hs.iter().map(|h| {
            crear_hallazgo(
                DIMENSION, 3, &h.archivo, h.linea, &h.cita, &h.detalle, SONDA, reqs,
            )
        })),
        Err(motivo) => motivos.push(motivo),
    }
    let mut escenarios = BTreeMap::new();
    let mut pendientes = Vec::new();
    if tarea.entrega.is_some() || !eval.is_empty() {
        match evaluar_en_contenedor(fabrica, repo, home, tarea, eval) {
            Ok(ev) => {
                hallazgos.extend(ev.hallazgos);
                escenarios = ev.escenarios;
                pendientes = ev.pendientes;
            }
            Err(motivo) => motivos.push(motivo),
        }
    }
    let lugar_escenarios = escenarios
        .keys()
        .map(|id| (id.clone(), LugarEscenario::ContenedorEvaluacion))
        .collect();
    // Sin veredicto: el motivo va a la sonda y los escenarios no medidos no se inventan.
    let (valor, motivo) = if motivos.is_empty() {
        (Some(puntuar_hallazgos(&hallazgos, reqs)), None)
    } else {
        (None, Some(motivos.join("; ")))
    };
    ResultadoEntrega {
        sonda: ResultadoSonda {
            dimension: DIMENSION,
            valor,
            motivo,
            hallazgos,
        },
        escenarios,
        lugar_escenarios,
        pendientes,
        sin_veredicto: Vec::new(),
        avisos: Vec::new(),
        faltantes: Vec::new(),
    }
}

/// Estado de un escenario que dejó la evaluación: su paso o su fallo, y los pendientes que salieron.
/// Lo llena `correr_en_evaluacion`. Sin veredicto, no sale nada de aquí.
#[derive(Default)]
struct Evaluada {
    hallazgos: Vec<Hallazgo>,
    escenarios: BTreeMap<String, bool>,
    pendientes: Vec<Pendiente>,
}

/// Aplica el resultado de un escenario al estado de la entrega. Lo usan la imagen y la evaluación.
/// Un pendiente no entra al mapa de escenarios: solo queda como `Pendiente`.
fn aplicar_prueba(
    escenarios: &mut BTreeMap<String, bool>,
    hallazgos: &mut Vec<Hallazgo>,
    pendientes: &mut Vec<Pendiente>,
    escenario: &Escenario,
    prueba: Prueba,
    reqs: &[Requisito],
) {
    let id = escenario.id.clone();
    match prueba {
        Prueba::Paso => {
            escenarios.insert(id, true);
        }
        Prueba::Fallo { codigo, cita } => {
            escenarios.insert(id, false);
            let detalle = format!("escenario falló (código {codigo})");
            hallazgos.push(hallazgo_de_escenario(escenario, &detalle, &cita, reqs));
        }
        Prueba::Pendiente(motivo) => pendientes.push(Pendiente {
            test: format!("tarea.toml::escenario-{id}"),
            motivo,
            colab: None,
            tasa: None,
        }),
    }
}

/// Preludio que corre una vez antes de copiar, en su propio `podman exec`. `true` cierra la línea.
fn preludio_de(lenguaje: &str) -> Option<String> {
    match lenguaje {
        "rust" => Some(format!("{PRELUDIO}true")),
        "python" => Some(format!("{}true", descarga::PRELUDIO_PYTHON)),
        _ => None,
    }
}

/// Ejecución del contenedor de evaluación: la imagen de la tarea, el repo en `/repo` y `escenarios/` en
/// `/oraculo/escenarios` si un `cmd` lo usa. Sin red y sin caché en escritura. `salida` es la carpeta temporal.
fn ejecucion_de_evaluacion(repo: &Path, home: &Path, tarea: &Tarea, salida: &Path) -> Ejecucion {
    Ejecucion {
        imagen: tarea.imagen.clone(),
        repo: Some(repo.to_path_buf()),
        oraculo: montaje_de_escenarios(tarea),
        oraculo_destino: Some("/oraculo/escenarios".into()),
        salida: salida.to_path_buf(),
        cmd: String::new(),
        env: Vec::new(),
        timeout: PLAZO_PASO_ENTREGA,
        red: false,
        cache: descarga::cache_de(home, &tarea.lenguaje, repo, &tarea.imagen),
        cache_rw: false,
        rustsec: None,
        trabajo: None,
        secretos: Vec::new(),
    }
}

/// Corre la evaluación en su contenedor propio: arranque, pasos y cierre. Un `Err` es infraestructura.
/// La carpeta de salida vive hasta después de quitar el contenedor.
fn evaluar_en_contenedor(
    fabrica: &dyn FabricaEscenarios,
    repo: &Path,
    home: &Path,
    tarea: &Tarea,
    eval: &[Escenario],
) -> Result<Evaluada, String> {
    let tmp = home.join(CARPETA_TEMPORALES);
    fs::create_dir_all(&tmp).map_err(|e| format!("directorio temporal de la evaluación: {e}"))?;
    let salida = temporal(&tmp).map_err(|e| format!("carpeta de salida de la evaluación: {e}"))?;
    let ejecucion = ejecucion_de_evaluacion(repo, home, tarea, salida.path());
    // La sesión se declara después de la salida: se quita antes que la carpeta.
    let sesion = fabrica
        .iniciar_evaluacion(&ejecucion)
        .map_err(|motivo| format!("el contenedor no arrancó el script de entrega: {motivo}"))?;
    correr_en_evaluacion(&sesion, tarea, eval)
}

/// Corre la evaluación en `sesion`. Cada paso es su propio `podman exec`. Un `Err` es infraestructura.
/// El hijo de `correr` se termina pase lo que pase, antes de que la sesión quite el contenedor.
fn correr_en_evaluacion(
    sesion: &Sesion<'_>,
    tarea: &Tarea,
    eval: &[Escenario],
) -> Result<Evaluada, String> {
    let mut lanzado: Option<Box<dyn ProcesoVivo>> = None;
    let resultado = correr_pasos(sesion, tarea, eval, &mut lanzado);
    if let Some(mut hijo) = lanzado {
        hijo.terminar();
    }
    resultado
}

/// Los pasos en orden: preludio, copia, instalar, correr y escenarios. `lanzado` guarda el hijo de `correr`.
fn correr_pasos(
    sesion: &Sesion<'_>,
    tarea: &Tarea,
    eval: &[Escenario],
    lanzado: &mut Option<Box<dyn ProcesoVivo>>,
) -> Result<Evaluada, String> {
    let reqs = tarea.requisito.as_slice();
    let entorno = entorno_de_evaluacion(tarea);
    let sin_entorno = Entorno::default();
    let mut out = Evaluada::default();
    let a_texto = |e: ErrorOraculo| e.to_string();

    // Preludio: sin activación y sin timeout. En Python cualquier código distinto de 0 es infraestructura.
    // En Rust solo cuenta el 125: el envoltorio ya dejó el 125 del producto en 1, así que sale de podman.
    if let Some(preludio) = preludio_de(&tarea.lenguaje) {
        let salida = sesion
            .paso(&sin_entorno, &preludio, PLAZO_PASO_ENTREGA, false)
            .map_err(a_texto)?;
        if tarea.lenguaje == "python" && salida.codigo != 0 {
            return Err("el preludio de Python no pudo aplicarse".into());
        }
        if tarea.lenguaje == "rust" && salida.codigo == CODIGO_PODMAN {
            return Err("el preludio de Rust no pudo aplicarse".into());
        }
    }

    if let Some(entrega) = &tarea.entrega {
        // La copia corre en `/` y sin activación: el directorio `/salida/work` todavía no existe.
        let en_raiz = Entorno {
            dir: Some("/".into()),
            activar: String::new(),
        };
        let copia = format!("cp -a /repo {SALIDA}/work");
        let copiado = sesion
            .paso(&en_raiz, &copia, PLAZO_PASO_ENTREGA, false)
            .map_err(a_texto)?;
        if copiado.codigo != 0 {
            return Err(
                "instalar nunca corrió: falló la copia de /repo o el script se cortó".into(),
            );
        }
        let instalado = sesion
            .paso(&entorno, &entrega.instalar, PLAZO_PASO_ENTREGA, false)
            .map_err(a_texto)?;
        // Un 125 aquí es de podman, no del producto: el envoltorio dejó el del producto en 1.
        if instalado.codigo == CODIGO_PODMAN {
            return Err(
                "instalar no corrió: podman no pudo correr el comando de instalación".into(),
            );
        }
        if instalado.codigo != 0 {
            // La cita es la última línea de la salida. Los escenarios no corren: el producto no instala.
            let texto = format!("{}\n{}", instalado.stdout, instalado.stderr);
            let cita = ultima_linea_de(&texto).unwrap_or_else(|| "(sin salida)".into());
            let detalle = "no instala: el comando de instalación falla";
            out.hallazgos.push(crear_hallazgo(
                DIMENSION, 3, "", 0, &cita, detalle, SONDA, reqs,
            ));
            for escenario in eval {
                out.escenarios.insert(escenario.id.clone(), false);
                let no_corrio = "no corrió: el producto no instala";
                out.hallazgos.push(hallazgo_de_escenario(
                    escenario,
                    no_corrio,
                    &escenario.id,
                    reqs,
                ));
            }
            return Ok(out);
        }
        // `correr` sigue vivo mientras corren los escenarios. Si muere antes de la espera, es un hallazgo.
        *lanzado = Some(sesion.lanzar(&entorno, &entrega.correr).map_err(a_texto)?);
        std::thread::sleep(Duration::from_secs(entrega.espera_segundos));
        if let Some(hijo) = lanzado.as_mut()
            && hijo.terminado().map_err(a_texto)?
        {
            let log = hijo.terminar();
            let cita = ultima_linea_de(&log).unwrap_or_else(|| "(sin salida)".into());
            let detalle = "no arranca: el comando de arranque muere antes de tiempo";
            out.hallazgos.push(crear_hallazgo(
                DIMENSION, 3, "", 0, &cita, detalle, SONDA, reqs,
            ));
        }
    }

    for escenario in eval {
        let prueba = sesion
            .correr_escenario(&entorno, escenario)
            .map_err(a_texto)?;
        aplicar_prueba(
            &mut out.escenarios,
            &mut out.hallazgos,
            &mut out.pendientes,
            escenario,
            prueba,
            reqs,
        );
    }
    Ok(out)
}

/// Hallazgo de entrega de un escenario que no pasó. Lleva los requisitos que lo citan.
fn hallazgo_de_escenario(
    escenario: &Escenario,
    detalle: &str,
    cita: &str,
    reqs: &[Requisito],
) -> Hallazgo {
    let impacto: Vec<String> = reqs
        .iter()
        .filter(|r| r.escenarios.contains(&escenario.id))
        .map(|r| r.id.clone())
        .collect();
    let mut h = crear_hallazgo(DIMENSION, 2, "", 0, cita, detalle, SONDA, reqs);
    h.impacto = impacto;
    h
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::oracle::ErrorOraculo;
    use crate::oracle::perfiles::herramientas::{EjecutorHerramienta, SalidaHerramienta};
    use std::cell::{Cell, RefCell};
    use std::collections::VecDeque;
    use std::fs;
    use std::rc::Rc;

    /// Respuesta de `podman` de mentira: un código con su salida, o un fallo de infraestructura.
    #[derive(Clone)]
    enum Resp {
        Codigo(i32, &'static str),
        Error(&'static str),
    }

    /// Ejecutor de mentira de la sesión. Guarda cada llamada en orden: `exec`, `lanzar` y `terminar`.
    /// Responde con la cola si queda algo. Si no, con la primera regla cuyo texto está en el último
    /// argumento. Si no, con el valor por defecto: arranque e `inspect` sanos, lo demás código 0.
    struct ExecFalso {
        llamadas: Rc<RefCell<Vec<Vec<String>>>>,
        cola: RefCell<VecDeque<Result<SalidaPodman, ErrorOraculo>>>,
        reglas: Vec<(&'static str, Resp)>,
        proceso_vivo: bool,
        proceso_log: &'static str,
    }

    impl ExecFalso {
        fn vacio() -> Self {
            ExecFalso {
                llamadas: Rc::new(RefCell::new(Vec::new())),
                cola: RefCell::new(VecDeque::new()),
                reglas: Vec::new(),
                proceso_vivo: true,
                proceso_log: "",
            }
        }

        /// Ejecutor que responde `respuestas` en orden, antes que cualquier regla.
        fn con(respuestas: Vec<Result<SalidaPodman, ErrorOraculo>>) -> Self {
            let ejecutor = Self::vacio();
            *ejecutor.cola.borrow_mut() = respuestas.into();
            ejecutor
        }

        /// Un código de salida por llamada, sin stdout.
        fn con_codigos(codigos: &[i32]) -> Self {
            Self::con(codigos.iter().map(|&c| Ok(salida(c, ""))).collect())
        }

        /// Regla: si el último argumento contiene `texto`, responde `resp`.
        fn responde(mut self, texto: &'static str, resp: Resp) -> Self {
            self.reglas.push((texto, resp));
            self
        }

        /// Proceso que `lanzar` da: `vivo` dice si sigue corriendo; `log` es lo que da al terminar.
        fn proceso(mut self, vivo: bool, log: &'static str) -> Self {
            self.proceso_vivo = vivo;
            self.proceso_log = log;
            self
        }

        fn llamadas(&self) -> Vec<Vec<String>> {
            self.llamadas.borrow().clone()
        }

        /// Índice de la primera llamada cuyos argumentos unidos contienen `texto`. Sin ella, falla.
        fn posicion(&self, texto: &str) -> usize {
            self.llamadas()
                .iter()
                .position(|l| l.join(" ").contains(texto))
                .unwrap_or_else(|| panic!("no hubo llamada con {texto:?}: {:?}", self.llamadas()))
        }

        /// Verdadero si alguna llamada contiene `texto`.
        fn hubo(&self, texto: &str) -> bool {
            self.llamadas().iter().any(|l| l.join(" ").contains(texto))
        }
    }

    impl Exec for ExecFalso {
        fn exec(&self, args: &[&str], _plazo: Duration) -> Result<SalidaPodman, ErrorOraculo> {
            self.llamadas
                .borrow_mut()
                .push(args.iter().map(|a| a.to_string()).collect());
            if let Some(respuesta) = self.cola.borrow_mut().pop_front() {
                return respuesta;
            }
            let ultimo = args.last().copied().unwrap_or_default();
            if let Some((_, resp)) = self
                .reglas
                .iter()
                .find(|(texto, _)| ultimo.contains(*texto))
            {
                return match resp {
                    Resp::Codigo(codigo, stdout) => Ok(salida(*codigo, stdout)),
                    Resp::Error(motivo) => Err(ErrorOraculo::Infra((*motivo).into())),
                };
            }
            let stdout = if args.first() == Some(&"container") {
                "true\n"
            } else if ultimo.contains("command -v sh") {
                "sh=/usr/bin/sh\ntimeout=/usr/bin/timeout\n"
            } else {
                ""
            };
            Ok(salida(0, stdout))
        }

        fn lanzar(&self, args: &[&str]) -> Result<Box<dyn ProcesoVivo>, ErrorOraculo> {
            let mut anotada = vec!["lanzar".to_string()];
            anotada.extend(args.iter().map(|a| a.to_string()));
            self.llamadas.borrow_mut().push(anotada);
            Ok(Box::new(ProcesoFalso {
                llamadas: Rc::clone(&self.llamadas),
                vivo: self.proceso_vivo,
                log: self.proceso_log.into(),
            }))
        }
    }

    /// Proceso de mentira que `lanzar` da. `terminado` dice si ya murió. `terminar` anota y da su log.
    struct ProcesoFalso {
        llamadas: Rc<RefCell<Vec<Vec<String>>>>,
        vivo: bool,
        log: String,
    }

    impl ProcesoVivo for ProcesoFalso {
        fn terminado(&mut self) -> Result<bool, ErrorOraculo> {
            Ok(!self.vivo)
        }

        fn terminar(&mut self) -> String {
            self.llamadas.borrow_mut().push(vec!["terminar".into()]);
            self.log.clone()
        }
    }

    /// Fábrica de evaluación de mentira. El arranque y cada paso salen por su `ExecFalso`. Con `arranque`
    /// no arranca nada y da ese motivo. `archivos` se escriben en la salida al arrancar: un producto que
    /// escribe `.rc` no debe cambiar el resultado.
    struct FabricaEvalFalsa {
        exec: ExecFalso,
        arranque: Option<&'static str>,
        archivos: Vec<(&'static str, &'static str)>,
        aperturas: Cell<u32>,
    }

    impl FabricaEvalFalsa {
        fn con(exec: ExecFalso) -> Self {
            FabricaEvalFalsa {
                exec,
                arranque: None,
                archivos: Vec::new(),
                aperturas: Cell::new(0),
            }
        }
    }

    impl FabricaEscenarios for FabricaEvalFalsa {
        fn iniciar(
            &self,
            _imagen: &str,
            _escenarios: Option<&Path>,
        ) -> Result<Box<dyn CorredorDeEscenario + '_>, String> {
            Err("sin contenedor de escenario en esta prueba".into())
        }

        fn iniciar_evaluacion(&self, ejecucion: &Ejecucion) -> Result<Sesion<'_>, String> {
            self.aperturas.set(self.aperturas.get() + 1);
            if let Some(motivo) = self.arranque {
                return Err(motivo.into());
            }
            for (nombre, texto) in &self.archivos {
                fs::write(ejecucion.salida.join(nombre), texto).expect("escribe el .rc");
            }
            let nombre = "eval-falsa";
            let arranque = |comando: &[&str]| {
                crate::oracle::contenedor::argumentos_separado(ejecucion, nombre, comando)
            };
            Sesion::iniciar(&self.exec, nombre, &arranque)
        }
    }

    /// Fábrica sana: el arranque sale bien y cada paso da código 0.
    fn sana() -> FabricaEvalFalsa {
        FabricaEvalFalsa::con(ExecFalso::vacio())
    }

    /// Sonda de entrega con una fábrica de mentira. La carpeta `home` vive hasta que termina la función.
    fn sonda_con(fabrica: &dyn FabricaEscenarios, repo: &Path, tarea: &Tarea) -> ResultadoEntrega {
        let home = tempfile::tempdir().expect("home");
        sonda_entrega_con(fabrica, repo, tarea, home.path(), true)
    }

    /// Salida de podman con `codigo` y `stdout`, sin stderr ni paso.
    fn salida(codigo: i32, stdout: &str) -> SalidaPodman {
        SalidaPodman {
            codigo,
            stdout: stdout.into(),
            stderr: String::new(),
            paso: None,
        }
    }

    const ENTREGA: &str = "[entrega]\ninstalar = \"make\"\ncorrer = \"./run\"\n\
        espera_segundos = 0\narchivos = [\"README.md\", \"Dockerfile\"]\n";

    const REQUISITOS: &str = "[[requisito]]\nid = \"R1\"\ntitulo = \"uno\"\n\
        escenarios = [\"b\"]\nmodulos = [\"protocolo\"]\n\n\
        [[requisito]]\nid = \"R2\"\ntitulo = \"dos\"\nescenarios = [\"a\"]\n";

    const ESCENARIOS: &str = "[[escenario]]\nid = \"a\"\ncmd = \"exit 0\"\n\n\
        [[escenario]]\nid = \"b\"\ncmd = \"exit 1\"\n";

    /// Tarea con `extra` al final del `tarea.toml`.
    fn tarea_de(lenguaje: &str, extra: &str) -> Tarea {
        let dir = tempfile::tempdir().expect("temp dir");
        let texto = format!(
            "nombre = \"x\"\nlenguaje = \"{lenguaje}\"\nimagen = \"i\"\nspec = \"s\"\n\
             [contrato]\ncmd = \"c\"\n[oculto]\ncmd = \"o\"\n{extra}"
        );
        fs::write(dir.path().join("tarea.toml"), texto).expect("escribe tarea.toml");
        Tarea::cargar_de(dir.path()).expect("carga la tarea")
    }

    /// Repo con README y Dockerfile que sí define `USER`.
    fn repo_completo() -> tempfile::TempDir {
        let repo = tempfile::tempdir().expect("temp dir");
        fs::write(repo.path().join("README.md"), "# x\n").expect("escribe README");
        fs::write(
            repo.path().join("Dockerfile"),
            "FROM debian:bookworm-slim\nUSER app\n",
        )
        .expect("escribe Dockerfile");
        repo
    }

    fn sin_hallazgos_de_entrega(r: &ResultadoEntrega) -> bool {
        r.sonda.hallazgos.is_empty()
    }

    // Migrados a la sesión de evaluación (parte 2b). Los que miraban texto del script o los `.rc`
    // se borraron: su intención la cubren los tests de pasos de más abajo.

    #[test]
    fn entrega_eval_preludio_solo_para_rust() {
        let repo = repo_completo();
        for (lenguaje, esperado) in [("rust", true), ("shell", false)] {
            let fabrica = sana();
            sonda_con(&fabrica, repo.path(), &tarea_de(lenguaje, ENTREGA));
            assert_eq!(
                fabrica.exec.hubo("export CARGO_HOME"),
                esperado,
                "{lenguaje}"
            );
        }
    }

    #[test]
    fn entrega_eval_python_aplica_el_preludio_antes_de_la_copia() {
        let repo = repo_completo();
        let fabrica = sana();
        sonda_con(&fabrica, repo.path(), &tarea_de("python", ENTREGA));
        let pre = fabrica.exec.posicion("mkdir -p /tmp/fuentes");
        let cp = fabrica.exec.posicion("cp -a /repo");
        assert!(pre < cp, "el preludio corre antes de la copia");
        assert!(
            !fabrica.exec.hubo("export CARGO_HOME"),
            "no aplica el de Rust"
        );
        let shell = sana();
        sonda_con(&shell, repo.path(), &tarea_de("shell", ENTREGA));
        assert!(!shell.exec.hubo("mkdir -p /tmp/fuentes"));
    }

    #[test]
    fn entrega_eval_sin_entrega_copia_nada_y_corre_sobre_repo() {
        let repo = repo_completo();
        let fabrica = sana();
        let t = tarea_de("shell", &format!("\n{ESCENARIOS}"));
        sonda_con(&fabrica, repo.path(), &t);
        assert!(!fabrica.exec.hubo("cp -a"), "sin entrega no hay copia");
        assert!(!fabrica.exec.hubo("make"), "sin entrega no instala");
        assert!(!fabrica.exec.hubo("./run"), "sin entrega no arranca");
        assert!(
            fabrica.exec.hubo("-w /repo"),
            "los escenarios corren sobre /repo"
        );
    }

    #[test]
    fn entrega_eval_python_preludio_con_codigo_1_da_sin_veredicto_sin_hallazgos() {
        let repo = repo_completo();
        let fabrica = FabricaEvalFalsa::con(
            ExecFalso::vacio().responde("mkdir -p /tmp/fuentes", Resp::Codigo(1, "")),
        );
        let r = sonda_con(&fabrica, repo.path(), &tarea_de("python", ENTREGA));
        assert_eq!(r.sonda.valor, None);
        assert!(
            r.sonda
                .motivo
                .as_deref()
                .unwrap_or_default()
                .contains("preludio de Python"),
            "el motivo nombra el preludio"
        );
        assert!(sin_hallazgos_de_entrega(&r), "no es un rojo falso");
        assert!(r.escenarios.is_empty());
        assert!(
            !fabrica.exec.hubo("make"),
            "nada se instala tras el preludio roto"
        );
    }

    #[test]
    fn entrega_eval_python_con_preludio_ok_sigue_el_flujo_normal() {
        let repo = repo_completo();
        let r = sonda_con(&sana(), repo.path(), &tarea_de("python", ENTREGA));
        assert_eq!(r.sonda.valor, Some(1.0));
        assert_eq!(r.sonda.motivo, None);
    }

    #[test]
    fn entrega_archivos_faltantes_lista_los_que_no_existen() {
        let repo = tempfile::tempdir().expect("temp dir");
        fs::write(repo.path().join("README.md"), "x").expect("escribe");
        let archivos = ["README.md".to_string(), "Dockerfile".to_string()];
        assert_eq!(archivos_faltantes(repo.path(), &archivos), ["Dockerfile"]);
    }

    #[test]
    fn entrega_sin_readme_da_un_hallazgo_de_archivo_faltante() {
        let repo = tempfile::tempdir().expect("temp dir");
        fs::write(repo.path().join("Dockerfile"), "FROM x\nUSER app\n").expect("escribe");
        let r = sonda_con(&sana(), repo.path(), &tarea_de("shell", ENTREGA));
        assert_eq!(r.sonda.hallazgos.len(), 1);
        let h = &r.sonda.hallazgos[0];
        assert_eq!(h.dimension, Dimension::Entrega);
        assert_eq!(
            (h.severidad, h.archivo.as_str(), h.linea),
            (2, "README.md", 0)
        );
        assert_eq!(
            (h.cita.as_str(), h.detalle.as_str()),
            ("README.md", "falta README.md")
        );
    }

    #[test]
    fn entrega_con_readme_y_dockerfile_con_user_da_cero() {
        let repo = repo_completo();
        let r = sonda_con(&sana(), repo.path(), &tarea_de("shell", ENTREGA));
        assert!(sin_hallazgos_de_entrega(&r));
        assert_eq!(r.sonda.valor, Some(1.0));
        assert_eq!(r.sonda.motivo, None);
    }

    #[test]
    fn entrega_dockerfile_que_corre_como_root_da_hallazgo_entrega_sev_3() {
        let repo = tempfile::tempdir().expect("temp dir");
        fs::write(repo.path().join("README.md"), "x").expect("escribe");
        fs::write(
            repo.path().join("Dockerfile"),
            "FROM debian:bookworm-slim\n",
        )
        .expect("escribe");
        let r = sonda_con(&sana(), repo.path(), &tarea_de("shell", ENTREGA));
        assert_eq!(r.sonda.hallazgos.len(), 1);
        let h = &r.sonda.hallazgos[0];
        assert_eq!(h.dimension, Dimension::Entrega);
        assert_eq!(h.severidad, 3);
        assert_eq!(h.archivo, "Dockerfile");
        assert_eq!(h.fuente, crate::tipos::Fuente::Sonda("entrega".into()));
    }

    #[test]
    fn entrega_sin_entrega_ni_escenarios_no_abre_contenedor() {
        let repo = repo_completo();
        let fabrica = sana();
        let r = sonda_con(&fabrica, repo.path(), &tarea_de("shell", ""));
        assert_eq!(fabrica.aperturas.get(), 0, "no abre contenedor");
        assert!(fabrica.exec.llamadas().is_empty());
        assert!(r.escenarios.is_empty());
        assert_eq!(r.sonda.valor, Some(1.0));
    }

    #[test]
    fn entrega_escenario_con_codigo_1_queda_false_y_con_hallazgo_de_su_requisito() {
        let repo = repo_completo();
        let fabrica =
            FabricaEvalFalsa::con(ExecFalso::vacio().responde("exit 1", Resp::Codigo(1, "")));
        let t = tarea_de("shell", &format!("{ENTREGA}\n{ESCENARIOS}\n{REQUISITOS}"));
        let r = sonda_con(&fabrica, repo.path(), &t);
        let esperado: BTreeMap<String, bool> =
            [("a".to_string(), true), ("b".to_string(), false)].into();
        assert_eq!(r.escenarios, esperado);
        assert_eq!(r.sonda.hallazgos.len(), 1);
        let h = &r.sonda.hallazgos[0];
        assert_eq!(h.cita, "b");
        assert_eq!(
            h.impacto,
            ["R1"],
            "solo el requisito que cita el escenario b"
        );
    }

    #[test]
    fn entrega_instalar_fallido_da_no_instala_y_escenarios_en_false() {
        let repo = repo_completo();
        let fabrica = FabricaEvalFalsa::con(
            ExecFalso::vacio().responde("make", Resp::Codigo(2, "compilando\nerror final\n")),
        );
        let t = tarea_de("shell", &format!("{ENTREGA}\n{ESCENARIOS}\n{REQUISITOS}"));
        let r = sonda_con(&fabrica, repo.path(), &t);
        let no_instala = r
            .sonda
            .hallazgos
            .iter()
            .find(|h| h.detalle.contains("no instala"))
            .expect("hallazgo de no instala");
        assert_eq!(
            (no_instala.severidad, no_instala.cita.as_str()),
            (3, "error final")
        );
        assert_eq!(r.escenarios.values().filter(|v| **v).count(), 0);
        let no_corrio = r
            .sonda
            .hallazgos
            .iter()
            .filter(|h| h.detalle.contains("el producto no instala"))
            .count();
        assert_eq!(no_corrio, 2, "cada escenario que no corrió lo dice");
    }

    #[test]
    fn entrega_correr_que_muere_da_no_arranca_con_la_ultima_linea() {
        let repo = repo_completo();
        let fabrica = FabricaEvalFalsa::con(
            ExecFalso::vacio().proceso(false, "arrancando\npuerto ocupado\n"),
        );
        let r = sonda_con(&fabrica, repo.path(), &tarea_de("shell", ENTREGA));
        let h = r
            .sonda
            .hallazgos
            .iter()
            .find(|h| h.detalle.contains("no arranca"))
            .expect("hallazgo de no arranca");
        assert_eq!((h.severidad, h.cita.as_str()), (3, "puerto ocupado"));
    }

    #[test]
    fn entrega_error_de_infraestructura_da_sin_veredicto_y_escenarios_vacios() {
        let repo = repo_completo();
        let fabrica =
            FabricaEvalFalsa::con(ExecFalso::vacio().responde("make", Resp::Error("podman caído")));
        let t = tarea_de("shell", &format!("{ENTREGA}\n{ESCENARIOS}"));
        let r = sonda_con(&fabrica, repo.path(), &t);
        assert_eq!(r.sonda.valor, None);
        assert!(r.sonda.motivo.unwrap_or_default().contains("podman caído"));
        assert!(r.escenarios.is_empty(), "no inventa falsos");
    }

    #[test]
    fn entrega_valor_es_puntuar_de_los_hallazgos() {
        let repo = tempfile::tempdir().expect("temp dir");
        let r = sonda_con(&sana(), repo.path(), &tarea_de("shell", ENTREGA));
        let esperado = puntuar_hallazgos(&r.sonda.hallazgos, &[]);
        assert_eq!(r.sonda.valor, Some(esperado));
    }

    #[test]
    fn entrega_escenario_cita_sale_de_la_ultima_linea_de_su_salida() {
        let repo = repo_completo();
        let fabrica = FabricaEvalFalsa::con(
            ExecFalso::vacio().responde("exit 1", Resp::Codigo(1, "paso 1\nel login falló\n\n")),
        );
        let t = tarea_de("shell", &format!("{ENTREGA}\n{ESCENARIOS}\n{REQUISITOS}"));
        let r = sonda_con(&fabrica, repo.path(), &t);
        assert_eq!(r.sonda.hallazgos.len(), 1);
        let h = &r.sonda.hallazgos[0];
        assert_eq!(h.cita, "el login falló");
        assert_eq!(h.impacto, ["R1"]);
    }

    #[test]
    fn entrega_paso_de_la_linea_final_sale_de_error_building() {
        // Reemplaza al test del último `STEP`: ahora solo cuenta `Error: building at STEP` de la línea final.
        let cortes = particiones_del_paso(
            "Error: building at STEP \"RUN  b\": while running runtime: exit status 1",
        );
        assert_eq!(
            cortes.first(),
            Some(&(
                "RUN b".to_string(),
                "while running runtime: exit status 1".to_string()
            ))
        );
        assert!(particiones_del_paso("STEP 3/3: RUN b").is_empty());
        assert!(particiones_del_paso("Error: creating build container: x").is_empty());
    }

    #[test]
    fn entrega_paso_con_comillas_y_separador_da_todos_los_cortes() {
        // La instrucción tiene `": ` dentro. Cada corte posible sale, del primero al último.
        let cortes = particiones_del_paso(
            "Error: building at STEP \"RUN echo \"x\": y\": while running runtime: exit status 1",
        );
        assert_eq!(cortes.len(), 2, "{cortes:?}");
        assert_eq!(cortes[1].0, "RUN echo \"x\": y");
        assert_eq!(cortes[1].1, "while running runtime: exit status 1");
    }

    /// Salida sintética de `podman build` (formato real de buildah): un `STEP k/n: <texto>` por paso,
    /// y el error que nombra el último paso. `causa` es la línea de error de la herramienta del paso.
    fn build_fallido(pasos: &[&str], causa: &str) -> SalidaPodman {
        let n = pasos.len();
        let stdout: String = pasos
            .iter()
            .enumerate()
            .map(|(k, paso)| format!("STEP {}/{n}: {paso}\n", k + 1))
            .collect();
        let fallido = pasos.last().copied().unwrap_or("");
        SalidaPodman {
            paso: None,
            codigo: 1,
            stdout,
            stderr: format!(
                "{causa}\nError: building at STEP \"{fallido}\": while running runtime: exit status 1\n"
            ),
        }
    }

    fn clasifica(salida: &SalidaPodman, texto: &str) -> Desenlace {
        clasificar_fallo(salida, "Dockerfile", texto, &[], false).expect("clasifica el fallo")
    }

    /// Como `clasifica`, con la red del host ya probada: un fallo de paso da hallazgo.
    fn clasifica_con_red(salida: &SalidaPodman, texto: &str) -> Desenlace {
        clasificar_fallo(salida, "Dockerfile", texto, &[], true).expect("clasifica el fallo")
    }

    #[test]
    fn entrega_paso_cuyo_texto_no_esta_en_el_dockerfile_da_sin_veredicto() {
        let texto = "FROM alpine:3.20\nRUN echo hola\n";
        let salida = build_fallido(
            &["FROM alpine:3.20", "RUN wget -q http://example.invalid"],
            "wget: bad address 'example.invalid'",
        );
        match clasifica(&salida, texto) {
            Desenlace::Pendiente(motivo) => {
                assert!(motivo.starts_with("sin veredicto"), "{motivo}")
            }
            otro => panic!("debía quedar sin veredicto, no {}", nombre(&otro)),
        }
    }

    #[test]
    fn entrega_multietapa_ubica_el_paso_de_la_segunda_etapa_por_su_texto() {
        // Buildah numera cada etapa desde 1: el `RUN` de la 2ª etapa es el paso 3/3, no el 3º del archivo.
        let texto = "FROM alpine:3.20 AS base\nWORKDIR /app\nRUN echo listo > /app/x\n\
            FROM alpine:3.20\nWORKDIR /srv\nRUN false\n";
        let salida = build_fallido(&["FROM alpine:3.20", "WORKDIR /srv", "RUN false"], "");
        match clasifica_con_red(&salida, texto) {
            Desenlace::Hallazgo(h) => {
                assert_eq!((h.severidad, h.linea), (3, 6), "{h:?}");
                assert_eq!(h.cita, "RUN false");
            }
            otro => panic!("debía ser hallazgo, no {}", nombre(&otro)),
        }
    }

    #[test]
    fn entrega_multietapa_run_de_red_en_la_segunda_etapa_se_ubica_por_texto() {
        let texto = "FROM alpine:3.20 AS base\nWORKDIR /app\nRUN echo listo > /app/x\n\
            FROM alpine:3.20\nWORKDIR /srv\nRUN wget -q http://example.invalid\n";
        let salida = build_fallido(
            &[
                "FROM alpine:3.20",
                "WORKDIR /srv",
                "RUN wget -q http://example.invalid",
            ],
            "wget: bad address 'example.invalid'",
        );
        match clasifica(&salida, texto) {
            Desenlace::PendienteRed { linea, cita, .. } => {
                assert_eq!(linea, 6);
                assert_eq!(cita, "RUN wget -q http://example.invalid");
            }
            otro => panic!("debía pedir red en la línea 6, no {}", nombre(&otro)),
        }
    }

    #[test]
    fn entrega_arg_antes_del_from_no_corre_el_indice() {
        // Buildah no cuenta el `ARG` previo al primer `FROM`: el paso 3/3 es el `RUN` de red, línea 4.
        let texto = "ARG VERSION=3.20\nFROM alpine:3.20\nRUN echo hola\n\
            RUN wget -q http://example.invalid\n";
        let salida = build_fallido(
            &[
                "FROM alpine:3.20",
                "RUN echo hola",
                "RUN wget -q http://example.invalid",
            ],
            "wget: bad address 'example.invalid'",
        );
        match clasifica(&salida, texto) {
            Desenlace::PendienteRed { linea, .. } => assert_eq!(linea, 4),
            otro => panic!("debía pedir red en la línea 4, no {}", nombre(&otro)),
        }
    }

    #[test]
    fn entrega_texto_repetido_en_dos_etapas_da_sin_veredicto() {
        let texto = "FROM alpine:3.20 AS base\nRUN apt-get update\n\
            FROM alpine:3.20\nRUN apt-get update\n";
        let salida = build_fallido(
            &["FROM alpine:3.20", "RUN apt-get update"],
            "bad address 'deb.example.invalid'",
        );
        match clasifica(&salida, texto) {
            Desenlace::Pendiente(motivo) => {
                assert!(motivo.starts_with("sin veredicto"), "{motivo}")
            }
            otro => panic!("debía quedar sin veredicto, no {}", nombre(&otro)),
        }
    }

    #[test]
    fn entrega_texto_con_continuacion_se_compara_con_espacios_normalizados() {
        // Guarda: buildah quita la `\` y deja los espacios de la línea siguiente. Debe seguir ubicándose.
        let texto = "FROM alpine:3.20\nRUN echo hola && \\\n    wget -q http://example.invalid\n";
        let salida = build_fallido(
            &[
                "FROM alpine:3.20",
                "RUN echo hola &&     wget -q http://example.invalid",
            ],
            "wget: bad address 'example.invalid'",
        );
        match clasifica(&salida, texto) {
            Desenlace::PendienteRed { linea, .. } => assert_eq!(linea, 2),
            otro => panic!("debía pedir red en la línea 2, no {}", nombre(&otro)),
        }
    }

    #[test]
    fn entrega_sin_step_en_la_salida_es_infraestructura() {
        // Sin ningún STEP y sin imagen ausente en la línea final, podman no empezó: es infraestructura.
        // La imagen ausente tiene su propio caso (pendiente), en `entrega_imagen_base_ausente_*`.
        let salida = SalidaPodman {
            paso: None,
            codigo: 125,
            stdout: String::new(),
            stderr: "Error: no se pudo iniciar el motor\n".into(),
        };
        assert!(clasificar_fallo(&salida, "Dockerfile", "FROM x\n", &[], false).is_err());
    }

    #[test]
    fn entrega_paso_guardado_clasifica_si_la_cola_perdio_el_step() {
        // La cola de 1 MiB no trae ningún STEP. La línea final de podman sigue ahí y da el paso.
        // `paso` ya no decide: el texto de la línea final manda.
        let salida = SalidaPodman {
            codigo: 1,
            stdout: "yyy\n".into(),
            stderr:
                "causa\nError: building at STEP \"RUN a\": while running runtime: exit status 1\n"
                    .into(),
            paso: Some("STEP 2/2: RUN a".into()),
        };
        match clasifica_con_red(&salida, "FROM x\nRUN a\n") {
            Desenlace::Hallazgo(h) => assert!(
                h.detalle.contains("la imagen no construye: línea 2"),
                "{}",
                h.detalle
            ),
            otro => panic!("debía dar hallazgo, no {}", nombre(&otro)),
        }
    }

    #[test]
    fn entrega_si_la_cola_trae_un_step_manda_la_cola_y_no_paso() {
        // El `paso` guardado apunta a `RUN a`, pero la línea final de podman nombra `RUN b`: manda la final.
        let salida = SalidaPodman {
            codigo: 1,
            stdout: "STEP 3/3: RUN b\n".into(),
            stderr:
                "causa\nError: building at STEP \"RUN b\": while running runtime: exit status 1\n"
                    .into(),
            paso: Some("STEP 2/3: RUN a".into()),
        };
        match clasifica_con_red(&salida, "FROM x\nRUN a\nRUN b\n") {
            Desenlace::Hallazgo(h) => assert!(
                h.detalle.contains("la imagen no construye: línea 3"),
                "{}",
                h.detalle
            ),
            otro => panic!("debía dar hallazgo, no {}", nombre(&otro)),
        }
    }

    #[test]
    fn entrega_step_impreso_por_el_repo_no_decide_el_paso() {
        // El `RUN` imprime `STEP 9/9: FROM falsa` en stdout. Solo cuenta la línea final de podman.
        let salida = SalidaPodman {
            paso: None,
            codigo: 1,
            stdout: "STEP 1/2: FROM debian:12\nSTEP 2/2: RUN ./falla\nSTEP 9/9: FROM falsa\n"
                .into(),
            stderr:
                "Error: building at STEP \"RUN ./falla\": while running runtime: exit status 1\n"
                    .into(),
        };
        match clasifica_con_red(&salida, "FROM debian:12\nRUN ./falla\n") {
            Desenlace::Hallazgo(h) => {
                assert_eq!((h.severidad, h.linea), (3, 2), "{h:?}");
                assert_eq!(h.cita, "RUN ./falla");
            }
            otro => panic!("debía ser hallazgo de la línea 2, no {}", nombre(&otro)),
        }
    }

    #[test]
    fn entrega_red_con_red_que_falla_igual_en_el_mismo_paso_es_hallazgo_sin_aviso() {
        // Sin red, el `RUN` imprime "could not resolve host": PendienteRed, se repite. Con red falla igual.
        let repo = repo_con_archivos(&[("Dockerfile", DF_RED)]);
        let home = tempfile::tempdir().expect("home");
        let falla = SalidaPodman {
            paso: None,
            codigo: 1,
            stdout: "STEP 1/2: FROM debian:12\nSTEP 2/2: RUN apt-get install -y curl\n".into(),
            stderr: "E: could not resolve host deb.debian.org\n\
                Error: building at STEP \"RUN apt-get install -y curl\": while running runtime: exit status 1\n"
                .into(),
        };
        let podman = PodmanFalso::nuevo(responde_build(falla.clone(), falla));
        let c = construir_con(
            repo.path(),
            home.path(),
            &tarea_de("shell", ""),
            &[],
            PLAZO_BUILD,
            &podman,
        )
        .expect("construye");
        let r = &c.resultado;
        assert!(!r.construida, "{r:?}");
        assert!(r.pendientes.is_empty(), "{:?}", r.pendientes);
        assert_eq!(r.hallazgos.len(), 1, "{:?}", r.hallazgos);
        assert!(
            r.hallazgos[0]
                .detalle
                .contains("la imagen no construye: línea 2"),
            "{:?}",
            r.hallazgos
        );
        assert!(c.avisos.is_empty(), "{:?}", c.avisos);
        assert_eq!(podman.builds().len(), 2, "{:?}", podman.builds());
        assert_eq!(gates_de_repeticion(home.path()).len(), 1);
    }

    #[test]
    fn entrega_sin_linea_final_de_podman_da_sin_veredicto_con_pasos_visibles() {
        let salida = SalidaPodman {
            paso: None,
            codigo: 1,
            stdout: "STEP 1/2: FROM x\nSTEP 2/2: RUN a\n".into(),
            stderr: "causa\n".into(),
        };
        match clasifica(&salida, "FROM x\nRUN a\n") {
            Desenlace::Pendiente(motivo) => {
                assert!(motivo.starts_with("sin veredicto"), "{motivo}");
                assert!(
                    motivo.contains("podman no informó el paso que falló"),
                    "{motivo}"
                );
            }
            otro => panic!("debía quedar sin veredicto, no {}", nombre(&otro)),
        }
    }

    #[test]
    fn entrega_imagen_base_ausente_en_la_linea_final_da_pendiente_con_su_from() {
        let salida = SalidaPodman {
            paso: None,
            codigo: 125,
            stdout: "STEP 1/2: FROM localhost/no-existe:9\n".into(),
            stderr: "Error: creating build container: localhost/no-existe:9: image not known\n"
                .into(),
        };
        match clasifica(&salida, "FROM localhost/no-existe:9\nRUN x\n") {
            Desenlace::Pendiente(motivo) => {
                assert!(motivo.contains("imagen base no descargada"), "{motivo}");
                assert!(motivo.contains("línea 1"), "{motivo}");
            }
            otro => panic!("debía ser pendiente de imagen base, no {}", nombre(&otro)),
        }
    }

    #[test]
    fn entrega_from_que_falla_con_imagen_ausente_en_su_paso_da_pendiente() {
        let salida = SalidaPodman {
            paso: None,
            codigo: 125,
            stdout: "STEP 1/2: FROM localhost/no-existe:9\n".into(),
            stderr: "Error: building at STEP \"FROM localhost/no-existe:9\": image not known\n"
                .into(),
        };
        match clasifica(&salida, "FROM localhost/no-existe:9\nRUN x\n") {
            Desenlace::Pendiente(motivo) => {
                assert!(motivo.contains("imagen base no descargada"), "{motivo}")
            }
            otro => panic!("debía ser pendiente de imagen base, no {}", nombre(&otro)),
        }
    }

    #[test]
    fn entrega_image_not_known_solo_en_stdout_no_da_imagen_base_ausente() {
        // El repo imprime "image not known" en stdout. La línea final es un fallo normal de `RUN`.
        let salida = SalidaPodman {
            paso: None,
            codigo: 1,
            stdout: "STEP 1/3: FROM debian:12\nSTEP 2/3: RUN echo hola\nSTEP 3/3: RUN ./falla\n\
                image not known\n"
                .into(),
            stderr:
                "Error: building at STEP \"RUN ./falla\": while running runtime: exit status 1\n"
                    .into(),
        };
        match clasifica_con_red(&salida, "FROM debian:12\nRUN echo hola\nRUN ./falla\n") {
            Desenlace::Hallazgo(h) => assert_eq!(h.linea, 3, "{h:?}"),
            otro => panic!("debía ser hallazgo de la línea 3, no {}", nombre(&otro)),
        }
    }

    #[test]
    fn entrega_red_no_configurada_solo_mira_la_linea_final_del_stderr() {
        // El repo imprime el texto de podman en stdout. La línea final es un fallo normal: no cuenta.
        let salida = SalidaPodman {
            paso: None,
            codigo: 1,
            stdout: "STEP 1/2: FROM debian:12\nSTEP 2/2: RUN ./falla\ncould not find slirp4netns\n"
                .into(),
            stderr:
                "Error: building at STEP \"RUN ./falla\": while running runtime: exit status 1\n"
                    .into(),
        };
        assert_eq!(red_no_configurada(&salida), None);
    }

    #[test]
    fn entrega_con_red_slirp_impreso_por_el_repo_no_es_error_de_podman() {
        let repo = repo_con_archivos(&[("Dockerfile", DF_RED)]);
        let home = tempfile::tempdir().expect("home");
        let sin_red = SalidaPodman {
            paso: None,
            codigo: 1,
            stdout: "STEP 1/2: FROM debian:12\nSTEP 2/2: RUN apt-get install -y curl\n".into(),
            stderr: "Temporary failure in name resolution\n\
                Error: building at STEP \"RUN apt-get install -y curl\": while running runtime: exit status 1\n"
                .into(),
        };
        let con_red = SalidaPodman {
            paso: None,
            codigo: 1,
            stdout: "STEP 1/2: FROM debian:12\nSTEP 2/2: RUN apt-get install -y curl\n\
                could not find slirp4netns\n"
                .into(),
            stderr: "Error: building at STEP \"RUN apt-get install -y curl\": while running runtime: exit status 1\n"
                .into(),
        };
        let podman = PodmanFalso::nuevo(responde_build(sin_red, con_red));
        let c = construir_con(
            repo.path(),
            home.path(),
            &tarea_de("shell", ""),
            &[],
            PLAZO_BUILD,
            &podman,
        )
        .expect("construye");
        let r = &c.resultado;
        assert!(r.pendientes.is_empty(), "{:?}", r.pendientes);
        assert_eq!(r.hallazgos.len(), 1, "{:?}", r.hallazgos);
        assert!(
            r.hallazgos[0].detalle.contains("la imagen no construye"),
            "{:?}",
            r.hallazgos
        );
        assert!(
            !c.avisos.iter().any(|a| a.starts_with(AVISO_RED_NO_ARRANCO)),
            "{:?}",
            c.avisos
        );
    }

    #[test]
    fn entrega_run_con_podman_real_que_imprime_step_falso_da_la_linea_del_run() {
        crate::saltar_sin_podman!();
        // GUARDA con podman real: el `RUN` imprime un STEP falso y sale con 1. Cuenta la línea 2.
        let texto =
            "FROM docker.io/library/alpine:3.20\nRUN echo \"STEP 9/9: FROM falsa\"; exit 1\n";
        let repo = repo_con_archivos(&[("Dockerfile", texto)]);
        let home = tempfile::tempdir().expect("home");
        // La sonda de red corre en la imagen de la tarea: con podman real, una que existe local.
        let mut tarea = tarea_de("shell", "");
        tarea.imagen = "docker.io/library/alpine:3.20".into();
        let c = construir_con(
            repo.path(),
            home.path(),
            &tarea,
            &[],
            PLAZO_BUILD,
            &PodmanReal,
        )
        .expect("construye");
        let r = &c.resultado;
        assert!(!r.construida, "{r:?}");
        assert!(r.pendientes.is_empty(), "{:?}", r.pendientes);
        assert_eq!(r.hallazgos.len(), 1, "{:?}", r.hallazgos);
        assert_eq!(r.hallazgos[0].linea, 2, "{:?}", r.hallazgos);
    }

    fn nombre(d: &Desenlace) -> &'static str {
        match d {
            Desenlace::Pendiente(_) => "Pendiente",
            Desenlace::PendienteRed { .. } => "PendienteRed",
            Desenlace::Hallazgo(_) => "Hallazgo",
        }
    }

    /// Repo temporal con los archivos dados (ruta, texto). Crea las carpetas que haga falta.
    fn repo_con_archivos(archivos: &[(&str, &str)]) -> tempfile::TempDir {
        let repo = tempfile::tempdir().expect("temp dir");
        for (ruta, texto) in archivos {
            let destino = repo.path().join(ruta);
            fs::create_dir_all(destino.parent().expect("con carpeta")).expect("crea carpetas");
            fs::write(destino, texto).expect("escribe el archivo");
        }
        repo
    }

    #[test]
    fn entrega_elegir_containerfile_valido_gana_sobre_la_raiz() {
        let repo = repo_con_archivos(&[
            ("Dockerfile", "FROM x\n"),
            ("bootstrap/Dockerfile", "FROM y\n"),
        ]);
        assert_eq!(
            elegir_dockerfile(repo.path(), Some("bootstrap/Dockerfile")),
            Eleccion::Elegido("bootstrap/Dockerfile".into())
        );
    }

    #[test]
    fn entrega_elegir_dockerfile_de_la_raiz_gana_a_otro_fuera() {
        let repo = repo_con_archivos(&[
            ("Dockerfile", "FROM x\n"),
            ("docker/otro.Dockerfile", "FROM y\n"),
        ]);
        assert_eq!(
            elegir_dockerfile(repo.path(), None),
            Eleccion::Elegido("Dockerfile".into())
        );
    }

    #[test]
    fn entrega_elegir_containerfile_de_la_raiz_sin_dockerfile() {
        let repo = repo_con_archivos(&[("Containerfile", "FROM x\n")]);
        assert_eq!(
            elegir_dockerfile(repo.path(), None),
            Eleccion::Elegido("Containerfile".into())
        );
    }

    #[test]
    fn entrega_elegir_unico_dockerfile_fuera_de_la_raiz() {
        let repo = repo_con_archivos(&[("docker/app.Dockerfile", "FROM x\n")]);
        assert_eq!(
            elegir_dockerfile(repo.path(), None),
            Eleccion::Elegido("docker/app.Dockerfile".into())
        );
    }

    #[test]
    fn entrega_elegir_dos_fuera_de_la_raiz_sin_containerfile_es_ninguno() {
        let repo = repo_con_archivos(&[
            ("docker/a.Dockerfile", "FROM x\n"),
            ("docker/b.Dockerfile", "FROM y\n"),
        ]);
        assert_eq!(
            elegir_dockerfile(repo.path(), None),
            Eleccion::NingunoElegido(vec![
                "docker/a.Dockerfile".into(),
                "docker/b.Dockerfile".into(),
            ])
        );
    }

    #[test]
    fn entrega_elegir_containerfile_inexistente_cae_al_criterio_siguiente() {
        let repo = repo_con_archivos(&[("Dockerfile", "FROM x\n")]);
        assert_eq!(
            elegir_dockerfile(repo.path(), Some("no/existe.Dockerfile")),
            Eleccion::Elegido("Dockerfile".into())
        );
    }

    #[test]
    fn entrega_elegir_containerfile_fuera_del_repo_no_vale() {
        let repo = repo_con_archivos(&[("Dockerfile", "FROM x\n")]);
        for ruta in ["../afuera/Dockerfile", "/etc/passwd"] {
            assert_eq!(
                elegir_dockerfile(repo.path(), Some(ruta)),
                Eleccion::Elegido("Dockerfile".into()),
                "containerfile {ruta} debe ignorarse"
            );
        }
    }

    #[test]
    fn entrega_elegir_sin_dockerfile_da_sin_dockerfile() {
        let repo = repo_con_archivos(&[("README.md", "x\n")]);
        assert_eq!(
            elegir_dockerfile(repo.path(), None),
            Eleccion::SinDockerfile
        );
    }

    #[test]
    fn entrega_elegir_ignora_dockerfiles_de_target_y_vendor() {
        let repo = repo_con_archivos(&[
            ("target/x/Dockerfile", "FROM x\n"),
            ("vendor/y/Dockerfile", "FROM y\n"),
            ("docker/a.Dockerfile", "FROM z\n"),
        ]);
        assert_eq!(
            elegir_dockerfile(repo.path(), None),
            Eleccion::Elegido("docker/a.Dockerfile".into())
        );
    }

    #[test]
    fn entrega_imagen_que_pide_credenciales_da_causa_unificada() {
        let repo = repo_con_archivos(&[("Dockerfile", "FROM 127.0.0.1:1/x:1\nUSER 1000\n")]);
        let home = tempfile::tempdir().expect("home");
        let imagenes = [ImagenBase {
            archivo: "Dockerfile".into(),
            linea: 1,
            cita: "FROM 127.0.0.1:1/x:1".into(),
            imagen: "127.0.0.1:1/x:1".into(),
            resuelta: Some("127.0.0.1:1/x:1".into()),
            digest: None,
            descargada: false,
            sin_resolver: None,
            problema: Some(Problema::PideCredenciales),
        }];
        let r = construir_dockerfile(repo.path(), home.path(), &tarea_de("shell", ""), &imagenes)
            .expect("construye");
        assert_eq!(r.sin_veredicto.len(), 1, "{:?}", r.sin_veredicto);
        assert_eq!(
            r.sin_veredicto[0].causa,
            "imagen base: pide credenciales (Root no las pasa)"
        );
        assert!(!r.construida);
    }

    // ---- Tarea 9d3: repetir el build con red cuando un RUN pide red ----

    /// Dockerfile de la 9d3: la línea 2 es un `RUN` que pide red.
    pub(crate) const DF_RED: &str = "FROM debian:12\nRUN apt-get install -y curl\nUSER app\n";

    /// Salida de `podman build` sin red: el `RUN` de la línea 2 falla por el DNS. La última línea de
    /// error es la de podman, con el paso que falló.
    pub(crate) fn salida_sin_red() -> SalidaPodman {
        SalidaPodman {
            paso: None,
            codigo: 1,
            stdout: "STEP 1/2: FROM debian:12\nSTEP 2/2: RUN apt-get install -y curl\n".into(),
            stderr: "Temporary failure in name resolution\n\
                Error: building at STEP \"RUN apt-get install -y curl\": while running runtime: exit status 1\n"
                .into(),
        }
    }

    /// Salida de un comando que sale 0.
    pub(crate) fn salida_ok() -> SalidaPodman {
        SalidaPodman {
            paso: None,
            codigo: 0,
            stdout: String::new(),
            stderr: String::new(),
        }
    }

    /// Lo que responde el podman falso a cada llamada.
    type RespuestaFalsa = dyn Fn(&[&str]) -> SalidaPodman;

    /// Podman falso: responde con `responde` y guarda cada llamada, en orden.
    pub(crate) struct PodmanFalso {
        llamadas: RefCell<Vec<Vec<String>>>,
        responde: Box<RespuestaFalsa>,
    }

    impl PodmanFalso {
        pub(crate) fn nuevo(responde: impl Fn(&[&str]) -> SalidaPodman + 'static) -> Self {
            PodmanFalso {
                llamadas: RefCell::new(Vec::new()),
                responde: Box::new(responde),
            }
        }

        /// Cuántas llamadas recibió, de cualquier tipo.
        pub(crate) fn total(&self) -> usize {
            self.llamadas.borrow().len()
        }

        /// Los argumentos de cada `build`, en orden.
        pub(crate) fn builds(&self) -> Vec<Vec<String>> {
            self.llamadas
                .borrow()
                .iter()
                .filter(|a| a[0] == "build")
                .cloned()
                .collect()
        }
    }

    impl Podman for PodmanFalso {
        fn correr(&self, args: &[&str], _plazo: Duration) -> Result<SalidaPodman, ErrorOraculo> {
            self.llamadas
                .borrow_mut()
                .push(args.iter().map(|a| a.to_string()).collect());
            Ok((self.responde)(args))
        }
    }

    /// Verdadero si el `build` usa la red de usuario de podman (la de la 9d3).
    fn es_build_con_red(args: &[&str]) -> bool {
        args.iter().any(|a| a.starts_with("pasta"))
    }

    /// Verdadero si la llamada es la prueba de la sonda de red del host (Tarea 91): un `run` con `pasta` y `sh`.
    fn es_sonda_de_red(args: &[&str]) -> bool {
        args[0] == "run" && args.contains(&"sh") && args.iter().any(|a| a.starts_with("pasta"))
    }

    /// Verdadero si la llamada es el control de la sonda: un `run` sin red con `sh -c true` sobre la
    /// imagen base. `tiene_sh` usa la misma forma, pero con la etiqueta de build (`root-entrega-*`).
    fn es_control_de_red(args: &[&str]) -> bool {
        args[0] == "run"
            && args.contains(&"none")
            && args.ends_with(&["-c", "true"])
            && !args[args.len() - 3].starts_with("root-entrega-")
    }

    /// Salida de la sonda de red cuando el host no arma la red `pasta`.
    pub(crate) fn salida_sonda_roja() -> SalidaPodman {
        SalidaPodman {
            paso: None,
            codigo: 125,
            stdout: String::new(),
            stderr: "Error: creating network: pasta: executable file not found in $PATH\n".into(),
        }
    }

    /// Respuesta de mentira: el `build` sin red da `sin_red` y con red da `con_red`. La sonda de red
    /// sale en verde. El resto sale sano: `id -u` da 1000 (no es root) e `image inspect` da `app`.
    pub(crate) fn responde_build(
        sin_red: SalidaPodman,
        con_red: SalidaPodman,
    ) -> impl Fn(&[&str]) -> SalidaPodman {
        responde_build_con_sonda(sin_red, con_red, true)
    }

    /// Como `responde_build`. `sonda_ok` dice si la prueba de la sonda de red del host sale bien.
    /// El control sale verde.
    pub(crate) fn responde_build_con_sonda(
        sin_red: SalidaPodman,
        con_red: SalidaPodman,
        sonda_ok: bool,
    ) -> impl Fn(&[&str]) -> SalidaPodman {
        responde_build_con_control(sin_red, con_red, true, sonda_ok)
    }

    /// Como `responde_build_con_sonda`. `control_ok` dice si el control sin red de la sonda corre.
    /// `prueba_ok` dice si la prueba con `pasta` sale bien.
    pub(crate) fn responde_build_con_control(
        sin_red: SalidaPodman,
        con_red: SalidaPodman,
        control_ok: bool,
        prueba_ok: bool,
    ) -> impl Fn(&[&str]) -> SalidaPodman {
        move |args| match args[0] {
            "build" if es_build_con_red(args) => con_red.clone(),
            "build" => sin_red.clone(),
            "run" if es_control_de_red(args) => {
                if control_ok {
                    salida_ok()
                } else {
                    salida_control_roja()
                }
            }
            "run" if es_sonda_de_red(args) => {
                if prueba_ok {
                    salida_ok()
                } else {
                    salida_sonda_roja()
                }
            }
            "run" if args.contains(&"id") => SalidaPodman {
                paso: None,
                codigo: 0,
                stdout: "1000\n".into(),
                stderr: String::new(),
            },
            "image" => SalidaPodman {
                paso: None,
                codigo: 0,
                stdout: "app\n".into(),
                stderr: String::new(),
            },
            _ => salida_ok(),
        }
    }

    /// Los `Gate` de la fase `construccion-red` que hay en `eventos.jsonl` de `home`.
    fn gates_de_repeticion(home: &Path) -> Vec<serde_json::Value> {
        let Ok(texto) = fs::read_to_string(home.join("eventos.jsonl")) else {
            return Vec::new();
        };
        texto
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str::<serde_json::Value>(l).expect("línea JSON"))
            .filter(|v| v["tipo"] == "gate" && v["fase"] == "construccion-red")
            .collect()
    }

    /// Afirma que `args` no trae montajes ni credenciales, y que la ruta de `home` aparece solo en
    /// el `-f` y en el contexto (la copia en `home/tmp`, que es contexto y no un montaje).
    fn assert_sin_credenciales_ni_montajes(args: &[String], home: &str) {
        const PROHIBIDOS: [&str; 6] = [
            "-v",
            "--volume",
            "--secret",
            "--build-arg",
            "--authfile",
            "--env",
        ];
        for a in args {
            assert!(!PROHIBIDOS.contains(&a.as_str()), "{a} en {args:?}");
            assert!(
                !PROHIBIDOS.iter().any(|p| a.starts_with(&format!("{p}="))),
                "{a} en {args:?}"
            );
        }
        let ultimo = args.len() - 1;
        for (i, a) in args.iter().enumerate() {
            if a.contains(home) {
                let es_df = i > 0 && args[i - 1] == "-f";
                assert!(
                    es_df || i == ultimo,
                    "la ruta de ROOT_HOME va en {a}: {args:?}"
                );
            }
        }
    }

    #[test]
    fn entrega_args_build_llevan_http_proxy_false_con_y_sin_red() {
        for red in [false, true] {
            let args = args_build(red, "etiqueta", "Dockerfile", ".");
            assert!(
                args.contains(&"--http-proxy=false".to_string()),
                "red={red}: {args:?}"
            );
            assert_eq!(args.first().map(String::as_str), Some("build"));
        }
    }

    // ---- Tarea 9d4: capacidades, pids y memoria; RUN con --network frena el build ----

    /// Dockerfile sin `--network` en ningún `RUN`: construye como hoy.
    const DF_SIN_NETWORK: &str = "FROM debian:12\nRUN apt-get install -y curl\nUSER app\n";

    #[test]
    fn entrega_args_build_llevan_capacidades_y_memoria_sin_pids_limit() {
        use crate::oracle::contenedor::{MEMORIA, PIDS_LIMITE};
        // `podman build` no acepta `--pids-limit`: solo `--memory` de los límites.
        for red in [false, true] {
            let args = args_build(red, "etiqueta", "Dockerfile", ".");
            for esperado in [
                "--cap-drop=all".to_string(),
                "--cap-add=CHOWN,DAC_OVERRIDE,FOWNER,SETUID,SETGID".to_string(),
                "--security-opt=no-new-privileges".to_string(),
                format!("--memory={MEMORIA}"),
            ] {
                assert!(
                    args.contains(&esperado),
                    "red={red}: falta {esperado} en {args:?}"
                );
            }
            assert!(
                !args.iter().any(|a| a.starts_with("--pids-limit")),
                "red={red}: sobra --pids-limit={PIDS_LIMITE} en {args:?}"
            );
        }
    }

    #[test]
    fn entrega_arranque_de_escenario_lleva_capacidades_pids_y_memoria() {
        use crate::oracle::contenedor::{MEMORIA, PIDS_LIMITE};
        let args = args_arranque("img:1", None, "nombre", &["sleep", "600"]);
        for esperado in [
            "--cap-drop=all".to_string(),
            "--security-opt=no-new-privileges".to_string(),
            format!("--pids-limit={PIDS_LIMITE}"),
            format!("--memory={MEMORIA}"),
        ] {
            assert!(args.contains(&esperado), "falta {esperado} en {args:?}");
        }
        // Las opciones van antes de la imagen: si no, podman las toma como argumentos del comando.
        let imagen = args.iter().position(|a| a == "img:1");
        let memoria = args.iter().position(|a| a.starts_with("--memory="));
        assert!(memoria < imagen, "{args:?}");
    }

    #[test]
    fn entrega_dockerfile_con_run_network_frena_antes_del_build() {
        // RUN con `--network=host`: el build no corre. Sale rojo y no llama a podman.
        let repo = repo_con_archivos(&[(
            "Dockerfile",
            "FROM debian:12\nRUN --network=host apt-get update\nUSER app\n",
        )]);
        let home = tempfile::tempdir().expect("home");
        let podman = PodmanFalso::nuevo(|_| salida_ok());
        let c = construir_con(
            repo.path(),
            home.path(),
            &tarea_de("shell", ""),
            &[],
            PLAZO_BUILD,
            &podman,
        )
        .expect("construye");
        let r = &c.resultado;
        assert_eq!(
            podman.total(),
            0,
            "llamó a podman: {:?}",
            podman.llamadas.borrow()
        );
        assert!(!r.construida, "{r:?}");
        assert_eq!(r.hallazgos.len(), 1, "{:?}", r.hallazgos);
        assert_eq!(r.hallazgos[0].detalle, "el Dockerfile pide red en RUN");
        assert_eq!(r.hallazgos[0].linea, 2, "{:?}", r.hallazgos);
        assert_eq!(r.hallazgos[0].severidad, 3, "{:?}", r.hallazgos);
    }

    #[test]
    fn entrega_run_network_en_minusculas_y_con_espacio_frena() {
        // Palabra clave en minúsculas, la forma `--network host` con espacio, y un `--mount` antes.
        for texto in [
            "FROM debian:12\nrun --network host apt-get update\n",
            "FROM debian:12\nRUN --mount=type=cache,target=/c --network=host apt-get update\n",
        ] {
            let repo = repo_con_archivos(&[("Dockerfile", texto)]);
            let home = tempfile::tempdir().expect("home");
            let podman = PodmanFalso::nuevo(|_| salida_ok());
            let c = construir_con(
                repo.path(),
                home.path(),
                &tarea_de("shell", ""),
                &[],
                PLAZO_BUILD,
                &podman,
            )
            .expect("construye");
            let r = &c.resultado;
            assert_eq!(podman.total(), 0, "{texto:?} llamó a podman");
            assert!(!r.construida, "{texto:?}: {r:?}");
            assert_eq!(r.hallazgos.len(), 1, "{texto:?}: {:?}", r.hallazgos);
            assert_eq!(
                r.hallazgos[0].detalle, "el Dockerfile pide red en RUN",
                "{texto:?}"
            );
        }
    }

    #[test]
    fn entrega_dockerfile_sin_network_construye_como_hoy() {
        // GUARDA: sin `--network` en ningún RUN, el build sigue sin red y construye.
        let repo = repo_con_archivos(&[("Dockerfile", DF_SIN_NETWORK)]);
        let home = tempfile::tempdir().expect("home");
        let podman = PodmanFalso::nuevo(responde_build(salida_ok(), salida_ok()));
        let c = construir_con(
            repo.path(),
            home.path(),
            &tarea_de("shell", ""),
            &[],
            PLAZO_BUILD,
            &podman,
        )
        .expect("construye");
        assert!(c.resultado.construida, "{:?}", c.resultado);
        let builds = podman.builds();
        assert_eq!(builds.len(), 1, "{builds:?}");
        assert!(
            builds[0]
                .windows(2)
                .any(|p| p[0] == "--network" && p[1] == "none"),
            "{builds:?}"
        );
    }

    #[test]
    fn guarda_escenario_sin_capacidades_ni_procesos_de_mas() {
        crate::saltar_sin_podman!();
        // GUARDA: `ping` con grupo 0 sale 0 por ping_group_range, por eso se miden capacidades.
        // Sin capacidades, `chown` a otro usuario falla. Con el tope de procesos, un bucle que crea
        // más procesos que el límite falla también.
        let corredor = FabricaReal
            .iniciar("docker.io/library/alpine:3.20", None)
            .expect("arranca el contenedor de escenario");
        let sin_capacidades = Escenario {
            id: "capacidades".into(),
            cmd: "grep -q '^CapEff:[[:space:]]*0000000000000000$' /proc/self/status".into(),
            red: false,
        };
        let r = corredor
            .correr(&sin_capacidades)
            .expect("corre el escenario");
        assert!(
            matches!(r, Prueba::Paso),
            "el contenedor tiene capacidades efectivas"
        );
        let chown = Escenario {
            id: "chown".into(),
            cmd: "chown nobody /tmp".into(),
            red: false,
        };
        let r = corredor.correr(&chown).expect("corre el escenario");
        assert!(
            !matches!(r, Prueba::Paso),
            "chown salió 0 dentro del contenedor"
        );
        let bomba = Escenario {
            id: "procesos".into(),
            cmd: "i=0; while [ $i -lt 700 ]; do sleep 5 & i=$((i+1)); done; wait".into(),
            red: false,
        };
        let r = corredor.correr(&bomba).expect("corre el escenario");
        assert!(!matches!(r, Prueba::Paso), "700 procesos salieron bien");
    }

    #[test]
    fn escenario_usa_oraculo_detecta_la_ruta_de_la_tarea() {
        assert!(escenario_usa_oraculo("/oraculo/escenarios/set_get.sh"));
        assert!(escenario_usa_oraculo("sh /oraculo/x.sh && true"));
    }

    #[test]
    fn escenario_usa_oraculo_no_mira_comandos_en_linea() {
        assert!(!escenario_usa_oraculo("exit 0"));
        assert!(!escenario_usa_oraculo("test -f /hola.txt"));
        assert!(!escenario_usa_oraculo("wget -q http://example.com -O /x"));
    }

    /// Tarea cargada desde `dir`, que el test conserva. `tarea_de` borra su carpeta al salir.
    fn tarea_en(dir: &Path, extra: &str) -> Tarea {
        let texto = format!(
            "nombre = \"x\"\nlenguaje = \"rust\"\nimagen = \"i\"\nspec = \"s\"\n\
             [contrato]\ncmd = \"c\"\n[oculto]\ncmd = \"o\"\n{extra}"
        );
        fs::write(dir.join("tarea.toml"), texto).expect("escribe tarea.toml");
        Tarea::cargar_de(dir).expect("carga la tarea")
    }

    #[test]
    fn escenario_montaje_sin_oraculo_es_none_aunque_exista_la_carpeta() {
        // M15: un `cmd` en línea no monta `escenarios/`, aunque la carpeta exista.
        let dir = tempfile::tempdir().expect("temp dir");
        fs::create_dir_all(dir.path().join("escenarios")).expect("crea escenarios/");
        let tarea = tarea_en(dir.path(), "[[escenario]]\nid = \"ok\"\ncmd = \"exit 0\"\n");
        assert_eq!(montaje_de_escenarios(&tarea), None);
    }

    #[test]
    fn escenario_montaje_con_oraculo_es_la_carpeta_escenarios() {
        let dir = tempfile::tempdir().expect("temp dir");
        fs::create_dir_all(dir.path().join("escenarios")).expect("crea escenarios/");
        let tarea = tarea_en(
            dir.path(),
            "[[escenario]]\nid = \"ok\"\ncmd = \"/oraculo/escenarios/ok.sh\"\n",
        );
        assert_eq!(
            montaje_de_escenarios(&tarea),
            Some(dir.path().join("escenarios"))
        );
    }

    #[test]
    fn escenario_montaje_con_oraculo_sin_carpeta_es_none() {
        // `entregar_con` corta antes con error de infraestructura; el montaje no inventa la carpeta.
        let dir = tempfile::tempdir().expect("temp dir");
        let tarea = tarea_en(
            dir.path(),
            "[[escenario]]\nid = \"ok\"\ncmd = \"/oraculo/escenarios/ok.sh\"\n",
        );
        assert_eq!(montaje_de_escenarios(&tarea), None);
    }

    #[test]
    fn escenario_arranque_monta_solo_la_carpeta_escenarios() {
        // M15: el contenedor de escenario ve `escenarios/`, nunca la tarea entera (oculto/ y tarea.toml).
        let args = args_arranque(
            "img:1",
            Some(Path::new("/t/oraculos/x/escenarios")),
            "nombre",
            &["sleep", "600"],
        );
        let montajes: Vec<&String> = args
            .windows(2)
            .filter(|p| p[0] == "-v")
            .map(|p| &p[1])
            .collect();
        assert_eq!(
            montajes,
            vec!["/t/oraculos/x/escenarios:/oraculo/escenarios:ro,z"],
            "{args:?}"
        );
        assert!(
            !args
                .iter()
                .any(|a| a.contains(":/oraculo:") || a.ends_with(":/oraculo")),
            "{args:?}"
        );
    }

    #[test]
    fn escenario_arranque_sin_carpeta_no_monta_nada() {
        let args = args_arranque("img:1", None, "nombre", &["sleep", "600"]);
        assert!(!args.iter().any(|a| a == "-v"), "{args:?}");
        assert!(!args.iter().any(|a| a.contains("/oraculo")), "{args:?}");
        assert!(args.iter().any(|a| a == "img:1"), "{args:?}");
    }

    #[test]
    fn entrega_arranque_de_escenario_lleva_http_proxy_false() {
        let args = args_arranque("img:1", Some(Path::new("/o")), "nombre", &["sleep", "600"]);
        let proxy = args.iter().position(|a| a == "--http-proxy=false");
        let imagen = args.iter().position(|a| a == "img:1");
        assert!(proxy.is_some() && proxy < imagen, "{args:?}");
        assert_eq!(args.first().map(String::as_str), Some("run"));
    }

    #[test]
    fn entrega_run_de_la_comprobacion_lleva_http_proxy_false() {
        let podman = PodmanFalso::nuevo(|_| salida_ok());
        let _ = corre_sin_root(&podman, "etiqueta", true);
        let _ = tiene_sh(&podman, "etiqueta");
        let runs: Vec<Vec<String>> = podman
            .llamadas
            .borrow()
            .iter()
            .filter(|a| a[0] == "run")
            .cloned()
            .collect();
        assert_eq!(runs.len(), 2, "{runs:?}");
        for args in runs {
            assert!(args.contains(&"--http-proxy=false".to_string()), "{args:?}");
        }
    }

    #[test]
    fn entrega_red_pendiente_por_red_se_repite_y_deja_hallazgo_sev_1() {
        let repo = repo_con_archivos(&[("Dockerfile", DF_RED)]);
        let home = tempfile::tempdir().expect("home");
        let podman = PodmanFalso::nuevo(responde_build(salida_sin_red(), salida_ok()));
        let c = construir_con(
            repo.path(),
            home.path(),
            &tarea_de("shell", ""),
            &[],
            PLAZO_BUILD,
            &podman,
        )
        .expect("construye");
        let r = &c.resultado;
        assert!(r.construida, "{r:?}");
        assert!(r.con_red, "{r:?}");
        assert!(r.pendientes.is_empty(), "{:?}", r.pendientes);
        assert_eq!(r.hallazgos.len(), 1, "{:?}", r.hallazgos);
        let h = &r.hallazgos[0];
        assert_eq!(
            (
                h.severidad,
                h.archivo.as_str(),
                h.linea,
                h.cita.as_str(),
                h.detalle.as_str()
            ),
            (
                1,
                "Dockerfile",
                2,
                "RUN apt-get install -y curl",
                "la imagen necesita red para construirse"
            )
        );
        // El trait no recibe variables de entorno: lo que el build recibe son solo sus argumentos.
        let builds = podman.builds();
        assert_eq!(builds.len(), 2, "{builds:?}");
        assert!(builds[0].contains(&"none".to_string()), "{:?}", builds[0]);
        assert!(
            builds[1].contains(&"pasta:--no-map-gw".to_string()),
            "{:?}",
            builds[1]
        );
        assert_sin_credenciales_ni_montajes(&builds[1], &home.path().to_string_lossy());
    }

    #[test]
    fn entrega_red_construye_sin_red_no_se_repite() {
        let repo = repo_con_archivos(&[("Dockerfile", "FROM debian:12\nUSER app\n")]);
        let home = tempfile::tempdir().expect("home");
        let podman = PodmanFalso::nuevo(responde_build(salida_ok(), salida_ok()));
        let c = construir_con(
            repo.path(),
            home.path(),
            &tarea_de("shell", ""),
            &[],
            PLAZO_BUILD,
            &podman,
        )
        .expect("construye");
        assert!(c.resultado.construida, "{:?}", c.resultado);
        assert!(!c.resultado.con_red, "{:?}", c.resultado);
        assert_eq!(podman.builds().len(), 1, "{:?}", podman.builds());
        assert!(
            gates_de_repeticion(home.path()).is_empty(),
            "no debe haber Gate de la repetición"
        );
    }

    #[test]
    fn entrega_red_args_sin_credenciales_ni_montajes() {
        let (home, df, ctx) = ("/h", "/h/tmp/c/repo/Dockerfile", "/h/tmp/c/repo");
        let args = args_build(true, "etiqueta", df, ctx);
        assert_eq!(
            args,
            [
                "build",
                "--http-proxy=false",
                "--cap-drop=all",
                "--cap-add=CHOWN,DAC_OVERRIDE,FOWNER,SETUID,SETGID",
                "--security-opt=no-new-privileges",
                "--memory=2g",
                "--network",
                "pasta:--no-map-gw",
                "--pull=never",
                "-t",
                "etiqueta",
                "-f",
                df,
                ctx
            ]
        );
        assert_sin_credenciales_ni_montajes(&args, home);
        // Sin red, los argumentos son los de siempre.
        assert_eq!(
            args_build(false, "etiqueta", df, ctx),
            [
                "build",
                "--http-proxy=false",
                "--cap-drop=all",
                "--cap-add=CHOWN,DAC_OVERRIDE,FOWNER,SETUID,SETGID",
                "--security-opt=no-new-privileges",
                "--memory=2g",
                "--network",
                "none",
                "--pull=never",
                "-t",
                "etiqueta",
                "-f",
                df,
                ctx
            ]
        );
    }

    #[test]
    fn entrega_red_gate_previo_lista_los_run_y_va_antes_del_build() {
        let repo = repo_con_archivos(&[("Dockerfile", DF_RED)]);
        let home = tempfile::tempdir().expect("home");
        // Al pedir el build con red, el falso anota si el Gate ya estaba en eventos.jsonl.
        let estaba: Rc<Cell<Option<bool>>> = Rc::new(Cell::new(None));
        let (anota, eventos) = (Rc::clone(&estaba), home.path().join("eventos.jsonl"));
        let base = responde_build(salida_sin_red(), salida_ok());
        let podman = PodmanFalso::nuevo(move |args| {
            if args[0] == "build" && es_build_con_red(args) {
                let texto = fs::read_to_string(&eventos).unwrap_or_default();
                anota.set(Some(texto.contains("construccion-red")));
            }
            base(args)
        });
        let c = construir_con(
            repo.path(),
            home.path(),
            &tarea_de("shell", ""),
            &[],
            PLAZO_BUILD,
            &podman,
        )
        .expect("construye");
        assert!(c.resultado.con_red, "{:?}", c.resultado);
        assert_eq!(
            estaba.get(),
            Some(true),
            "el Gate debe estar antes del build con red"
        );
        let gates = gates_de_repeticion(home.path());
        assert_eq!(gates.len(), 1, "{gates:?}");
        let g = &gates[0];
        assert_eq!(g["corrida"], "entrega-x");
        assert_eq!(g["extra"]["accion"], "repetir-con-red");
        assert_eq!(g["extra"]["dockerfile"], "Dockerfile");
        assert_eq!(g["extra"]["primer_paso_sin_red"]["linea"], 2);
        assert_eq!(
            g["extra"]["primer_paso_sin_red"]["cita"],
            "RUN apt-get install -y curl"
        );
        let run = g["extra"]["pasos_del_dockerfile"]
            .as_array()
            .expect("lista de RUN");
        assert_eq!(run.len(), 1, "{run:?}");
        assert_eq!(run[0]["linea"], 2);
    }

    #[test]
    fn entrega_red_gate_separa_la_run_que_fallo_de_todas_las_run() {
        // La línea 2 no pide red; solo la 3 falla sin red. El Gate no dice que todas las RUN la necesitan.
        const DF: &str = "FROM debian:12\nRUN echo listo\nRUN apt-get install -y curl\nUSER app\n";
        let repo = repo_con_archivos(&[("Dockerfile", DF)]);
        let home = tempfile::tempdir().expect("home");
        let sin_red = SalidaPodman {
            paso: None,
            codigo: 1,
            stdout: "STEP 1/3: FROM debian:12\nSTEP 2/3: RUN echo listo\nSTEP 3/3: RUN apt-get install -y curl\n"
                .into(),
            stderr: "Temporary failure in name resolution\n\
                Error: building at STEP \"RUN apt-get install -y curl\": exit status 1\n"
                .into(),
        };
        let podman = PodmanFalso::nuevo(responde_build(sin_red, salida_ok()));
        let c = construir_con(
            repo.path(),
            home.path(),
            &tarea_de("shell", ""),
            &[],
            PLAZO_BUILD,
            &podman,
        )
        .expect("construye");
        assert!(c.resultado.con_red, "{:?}", c.resultado);
        let gates = gates_de_repeticion(home.path());
        let g = &gates[0]["extra"];
        assert_eq!(g["primer_paso_sin_red"]["linea"], 3, "{g}");
        assert_eq!(
            g["primer_paso_sin_red"]["cita"],
            "RUN apt-get install -y curl"
        );
        let runs = g["pasos_del_dockerfile"].as_array().expect("lista de RUN");
        let lineas: Vec<u64> = runs.iter().filter_map(|r| r["linea"].as_u64()).collect();
        assert_eq!(lineas, vec![2, 3], "{runs:?}");
        assert!(g.get("fallo_sin_red").is_none(), "{g}");
        assert!(g.get("run").is_none(), "{g}");
    }

    #[test]
    fn entrega_red_segundo_build_falla_da_la_imagen_no_construye() {
        let repo = repo_con_archivos(&[("Dockerfile", DF_RED)]);
        let home = tempfile::tempdir().expect("home");
        // El build con red falla por un error que no es de red: el `RUN` no construye.
        let con_red_falla = SalidaPodman {
            paso: None,
            codigo: 1,
            stdout: "STEP 1/2: FROM debian:12\nSTEP 2/2: RUN apt-get install -y curl\n".into(),
            stderr: "E: Unable to locate package curl\n\
                Error: building at STEP \"RUN apt-get install -y curl\": exit status 100\n"
                .into(),
        };
        let podman = PodmanFalso::nuevo(responde_build(salida_sin_red(), con_red_falla));
        let c = construir_con(
            repo.path(),
            home.path(),
            &tarea_de("shell", ""),
            &[],
            PLAZO_BUILD,
            &podman,
        )
        .expect("construye");
        let r = &c.resultado;
        assert!(!r.construida, "{r:?}");
        assert!(r.pendientes.is_empty(), "{:?}", r.pendientes);
        assert_eq!(r.hallazgos.len(), 1, "{:?}", r.hallazgos);
        assert!(
            r.hallazgos[0].detalle.contains("la imagen no construye"),
            "{}",
            r.hallazgos[0].detalle
        );
        assert_eq!(podman.builds().len(), 2, "{:?}", podman.builds());
    }

    #[test]
    fn entrega_red_pendiente_de_from_no_se_repite() {
        let repo = repo_con_archivos(&[(
            "Dockerfile",
            "FROM localhost/no-existe:9\nRUN apt-get install -y curl\nUSER app\n",
        )]);
        let home = tempfile::tempdir().expect("home");
        let sin_imagen = SalidaPodman {
            paso: None,
            codigo: 1,
            stdout: "STEP 1/2: FROM localhost/no-existe:9\n".into(),
            stderr: "Error: creating build container: localhost/no-existe:9: image not known\n"
                .into(),
        };
        let podman = PodmanFalso::nuevo(responde_build(sin_imagen, salida_ok()));
        let c = construir_con(
            repo.path(),
            home.path(),
            &tarea_de("shell", ""),
            &[],
            PLAZO_BUILD,
            &podman,
        )
        .expect("construye");
        let r = &c.resultado;
        assert!(!r.construida, "{r:?}");
        assert_eq!(r.pendientes.len(), 1, "{:?}", r.pendientes);
        assert!(
            r.pendientes[0].motivo.contains("imagen base no descargada"),
            "{:?}",
            r.pendientes
        );
        assert_eq!(podman.builds().len(), 1, "{:?}", podman.builds());
        assert!(gates_de_repeticion(home.path()).is_empty());
    }

    /// Escena de un test de la sonda: repo, casa y tarea en un temporal dentro del proyecto, no en /tmp.
    struct EscenaSonda {
        _base: tempfile::TempDir,
        repo: PathBuf,
        home: PathBuf,
        tarea: Tarea,
    }

    fn escena_de_sonda(dockerfile: &str) -> EscenaSonda {
        let base = tempfile::Builder::new()
            .prefix("sonda-")
            .tempdir_in(env!("CARGO_MANIFEST_DIR"))
            .expect("temporal del proyecto");
        let repo = base.path().join("repo");
        fs::create_dir_all(&repo).expect("crea el repo");
        fs::write(repo.join("Dockerfile"), dockerfile).expect("escribe el Dockerfile");
        let home = base.path().join("home");
        fs::create_dir_all(&home).expect("crea la casa");
        let carpeta_tarea = base.path().join("tarea");
        fs::create_dir_all(&carpeta_tarea).expect("crea la carpeta de la tarea");
        fs::write(
            carpeta_tarea.join("tarea.toml"),
            "nombre = \"x\"\nlenguaje = \"shell\"\nimagen = \"i\"\nspec = \"s\"\n\
             [contrato]\ncmd = \"c\"\n[oculto]\ncmd = \"o\"\n",
        )
        .expect("escribe tarea.toml");
        let tarea = Tarea::cargar_de(&carpeta_tarea).expect("carga la tarea");
        EscenaSonda {
            _base: base,
            repo,
            home,
            tarea,
        }
    }

    #[test]
    fn build_con_control_que_falla_da_infraestructura_no_pendiente() {
        let escena = escena_de_sonda(DF_RED);
        let podman = PodmanFalso::nuevo(responde_build_con_control(
            salida_sin_red(),
            salida_red_no_configurada(),
            false,
            true,
        ));
        let c = construir_con(
            &escena.repo,
            &escena.home,
            &escena.tarea,
            &[],
            PLAZO_BUILD,
            &podman,
        )
        .expect("un control de red roto no tumba la construcción");
        let r = &c.resultado;
        assert_eq!(r.sin_veredicto.len(), 1, "{:?}", r.sin_veredicto);
        let causa = &r.sin_veredicto[0].causa;
        assert!(causa.contains("infraestructura"), "{causa}");
        assert!(causa.contains("image not found"), "{causa}");
        assert!(r.pendientes.is_empty(), "{:?}", r.pendientes);
    }

    #[test]
    fn build_con_control_verde_y_prueba_roja_queda_pendiente_con_su_motivo() {
        let escena = escena_de_sonda(DF_RED);
        let podman = PodmanFalso::nuevo(responde_build_con_control(
            salida_sin_red(),
            salida_red_no_configurada(),
            true,
            false,
        ));
        let c = construir_con(
            &escena.repo,
            &escena.home,
            &escena.tarea,
            &[],
            PLAZO_BUILD,
            &podman,
        )
        .expect("construye");
        let r = &c.resultado;
        assert!(r.sin_veredicto.is_empty(), "{:?}", r.sin_veredicto);
        assert_eq!(r.pendientes.len(), 1, "{:?}", r.pendientes);
        assert_eq!(r.pendientes[0].motivo, MOTIVO_RED_EN_HOST);
    }

    /// Sonda de mentira que cuenta sus llamadas. El control sale verde. La prueba con `pasta` sale
    /// según `prueba_ok`.
    fn sonda_que_cuenta(
        llamadas: &Cell<u32>,
        prueba_ok: bool,
    ) -> impl Fn(&[&str]) -> Result<SalidaPodman, ErrorOraculo> + '_ {
        move |args: &[&str]| {
            llamadas.set(llamadas.get() + 1);
            let es_prueba = args.iter().any(|a| a.starts_with("pasta"));
            Ok(if es_prueba && !prueba_ok {
                salida_sonda_roja()
            } else {
                salida_ok()
            })
        }
    }

    #[test]
    fn sonda_que_fallo_una_vez_y_pasa_la_segunda_da_hay_red() {
        let memo = Mutex::new(HashSet::new());
        let llamadas = Cell::new(0);
        let correr = |_: &[&str]| -> Result<SalidaPodman, ErrorOraculo> {
            let k = llamadas.get();
            llamadas.set(k + 1);
            Ok(if k == 0 {
                salida_control_roja()
            } else {
                salida_ok()
            })
        };
        assert_eq!(
            sonda_con_memoria(&memo, correr, "imagen"),
            Red::NoCorrio("Error: image not found".into())
        );
        assert_eq!(sonda_con_memoria(&memo, correr, "imagen"), Red::Hay);
    }

    #[test]
    fn hay_red_de_una_imagen_no_vale_para_otra() {
        let memo = Mutex::new(HashSet::new());
        let llamadas = Cell::new(0);
        assert_eq!(
            sonda_con_memoria(&memo, sonda_que_cuenta(&llamadas, true), "imagen-a"),
            Red::Hay
        );
        let tras_a = llamadas.get();
        assert!(tras_a > 0, "la primera vez la sonda corre");
        assert_eq!(
            sonda_con_memoria(&memo, sonda_que_cuenta(&llamadas, true), "imagen-a"),
            Red::Hay
        );
        assert_eq!(
            llamadas.get(),
            tras_a,
            "una imagen recordada no corre la sonda"
        );
        assert_eq!(
            sonda_con_memoria(&memo, sonda_que_cuenta(&llamadas, false), "imagen-b"),
            Red::NoHay
        );
        assert!(llamadas.get() > tras_a, "la imagen B corre su propia sonda");
        let guardadas = memo.lock().expect("memoria");
        assert!(guardadas.contains("imagen-a"));
        assert!(!guardadas.contains("imagen-b"));
    }

    #[test]
    fn sonda_de_red_nohay_y_nocorrio_no_se_guardan() {
        let memo = Mutex::new(HashSet::new());
        let llamadas = Cell::new(0);
        assert_eq!(
            sonda_con_memoria(&memo, sonda_que_cuenta(&llamadas, false), "imagen"),
            Red::NoHay
        );
        assert!(memo.lock().expect("memoria").is_empty());
        let roja = |_: &[&str]| -> Result<SalidaPodman, ErrorOraculo> { Ok(salida_control_roja()) };
        assert!(matches!(
            sonda_con_memoria(&memo, roja, "imagen"),
            Red::NoCorrio(_)
        ));
        assert!(memo.lock().expect("memoria").is_empty());
    }

    /// Salida del control de la sonda cuando podman no encuentra la imagen base.
    pub(crate) fn salida_control_roja() -> SalidaPodman {
        SalidaPodman {
            paso: None,
            codigo: 125,
            stdout: String::new(),
            stderr: "Error: image not found\n".into(),
        }
    }

    /// Salida de `podman build` con red cuando podman no puede armar la red del `RUN` (sin `slirp4netns`).
    pub(crate) fn salida_red_no_configurada() -> SalidaPodman {
        SalidaPodman {
            paso: None,
            codigo: 1,
            stdout: "STEP 1/2: FROM debian:12\nSTEP 2/2: RUN apt-get install -y curl\n".into(),
            stderr: "Error: building at STEP \"RUN apt-get install -y curl\": setup network: could not find slirp4netns, the network namespace can't be configured: exec: \"slirp4netns\": executable file not found in $PATH\n".into(),
        }
    }

    #[test]
    fn entrega_red_setup_network_con_red_queda_pendiente_con_aviso() {
        let repo = repo_con_archivos(&[("Dockerfile", DF_RED)]);
        let home = tempfile::tempdir().expect("home");
        let podman = PodmanFalso::nuevo(responde_build_con_sonda(
            salida_sin_red(),
            salida_red_no_configurada(),
            false,
        ));
        let c = construir_con(
            repo.path(),
            home.path(),
            &tarea_de("shell", ""),
            &[],
            PLAZO_BUILD,
            &podman,
        )
        .expect("un build con red sin red configurable no tumba la construcción");
        let r = &c.resultado;
        assert!(!r.construida, "{r:?}");
        assert!(!r.con_red, "{r:?}");
        assert!(r.hallazgos.is_empty(), "{:?}", r.hallazgos);
        assert_eq!(r.pendientes.len(), 1, "{:?}", r.pendientes);
        assert_eq!(
            r.pendientes[0].motivo,
            "la red no se pudo configurar en este host"
        );
        assert!(
            c.avisos
                .iter()
                .any(|a| a.starts_with(AVISO_RED_NO_ARRANCO)
                    && a.contains("could not find slirp4netns")),
            "{:?}",
            c.avisos
        );
    }

    #[test]
    fn entrega_red_pasta_fallida_con_red_queda_pendiente_con_aviso() {
        let repo = repo_con_archivos(&[("Dockerfile", DF_RED)]);
        let home = tempfile::tempdir().expect("home");
        let pasta_falla = SalidaPodman {
            paso: None,
            codigo: 125,
            stdout: "STEP 1/2: FROM debian:12\nSTEP 2/2: RUN apt-get install -y curl\n".into(),
            stderr: "Error: creating network: pasta: executable file not found in $PATH\n".into(),
        };
        let podman = PodmanFalso::nuevo(responde_build_con_sonda(
            salida_sin_red(),
            pasta_falla,
            false,
        ));
        let c = construir_con(
            repo.path(),
            home.path(),
            &tarea_de("shell", ""),
            &[],
            PLAZO_BUILD,
            &podman,
        )
        .expect("un build con red con pasta fallida no tumba la construcción");
        let r = &c.resultado;
        assert!(!r.construida, "{r:?}");
        assert!(r.hallazgos.is_empty(), "{:?}", r.hallazgos);
        assert_eq!(r.pendientes.len(), 1, "{:?}", r.pendientes);
        assert_eq!(
            r.pendientes[0].motivo,
            "la red no se pudo configurar en este host"
        );
        assert!(
            c.avisos
                .iter()
                .any(|a| a.starts_with(AVISO_RED_NO_ARRANCO) && a.contains("pasta")),
            "{:?}",
            c.avisos
        );
    }

    #[test]
    fn entrega_red_error_de_paquetes_con_red_sigue_como_hallazgo() {
        // GUARDA: un error de `RUN` sin marcadores de red es un producto que no construye.
        let repo = repo_con_archivos(&[("Dockerfile", DF_RED)]);
        let home = tempfile::tempdir().expect("home");
        let error_paquetes = SalidaPodman {
            paso: None,
            codigo: 1,
            stdout: "STEP 1/2: FROM debian:12\nSTEP 2/2: RUN apt-get install -y curl\n".into(),
            stderr: "ERROR: unable to select packages:\n  curl (no such package)\n\
                Error: building at STEP \"RUN apt-get install -y curl\": exit status 1\n"
                .into(),
        };
        let podman = PodmanFalso::nuevo(responde_build(salida_sin_red(), error_paquetes));
        let c = construir_con(
            repo.path(),
            home.path(),
            &tarea_de("shell", ""),
            &[],
            PLAZO_BUILD,
            &podman,
        )
        .expect("construye");
        let r = &c.resultado;
        assert!(!r.construida, "{r:?}");
        assert!(r.pendientes.is_empty(), "{:?}", r.pendientes);
        assert_eq!(r.hallazgos.len(), 1, "{:?}", r.hallazgos);
        assert!(
            r.hallazgos[0].detalle.contains("la imagen no construye"),
            "{}",
            r.hallazgos[0].detalle
        );
        assert!(c.avisos.is_empty(), "{:?}", c.avisos);
    }

    #[test]
    fn entrega_red_con_red_ok_da_hallazgo_severidad_1() {
        // GUARDA: con red el build sale bien y el hallazgo es de severidad 1.
        let repo = repo_con_archivos(&[("Dockerfile", DF_RED)]);
        let home = tempfile::tempdir().expect("home");
        let podman = PodmanFalso::nuevo(responde_build(salida_sin_red(), salida_ok()));
        let c = construir_con(
            repo.path(),
            home.path(),
            &tarea_de("shell", ""),
            &[],
            PLAZO_BUILD,
            &podman,
        )
        .expect("construye");
        let r = &c.resultado;
        assert!(r.con_red, "{r:?}");
        assert_eq!(r.hallazgos.len(), 1, "{:?}", r.hallazgos);
        assert_eq!(r.hallazgos[0].severidad, 1, "{:?}", r.hallazgos);
    }

    #[test]
    fn entrega_red_podman_rechaza_la_red_conserva_el_pendiente_con_aviso() {
        let repo = repo_con_archivos(&[("Dockerfile", DF_RED)]);
        let home = tempfile::tempdir().expect("home");
        // Podman rechaza la opción de red: sale 125 sin ningún STEP.
        let rechazo = SalidaPodman {
            paso: None,
            codigo: 125,
            stdout: String::new(),
            stderr: "Error: invalid network option\n".into(),
        };
        let podman = PodmanFalso::nuevo(responde_build(salida_sin_red(), rechazo));
        let c = construir_con(
            repo.path(),
            home.path(),
            &tarea_de("shell", ""),
            &[],
            PLAZO_BUILD,
            &podman,
        )
        .expect("un fallo del respaldo con red no tumba la construcción");
        let r = &c.resultado;
        assert!(!r.construida, "{r:?}");
        assert!(r.hallazgos.is_empty(), "{:?}", r.hallazgos);
        assert_eq!(r.pendientes.len(), 1, "{:?}", r.pendientes);
        assert!(
            r.pendientes[0].motivo.contains("línea 2"),
            "{:?}",
            r.pendientes
        );
        assert!(
            c.avisos
                .iter()
                .any(|a| a.starts_with(AVISO_RED_NO_ARRANCO) && a.contains("invalid network")),
            "{:?}",
            c.avisos
        );
    }

    /// Dockerfile con un `ADD` de una URL: buildah la baja desde el host aunque el build vaya sin red (A11).
    const DF_ADD_URL: &str = "FROM debian:12\nADD https://ejemplo.com/x.tar.gz /x\nUSER app\n";

    #[test]
    fn entrega_red_add_con_url_que_construye_sin_red_se_repite_con_red() {
        // El build sin red sale 0, pero el ADD bajó la URL desde el host: cuenta como paso con red.
        let repo = repo_con_archivos(&[("Dockerfile", DF_ADD_URL)]);
        let home = tempfile::tempdir().expect("home");
        let podman = PodmanFalso::nuevo(responde_build(salida_ok(), salida_ok()));
        let c = construir_con(
            repo.path(),
            home.path(),
            &tarea_de("shell", ""),
            &[],
            PLAZO_BUILD,
            &podman,
        )
        .expect("construye");
        assert!(c.resultado.con_red, "{:?}", c.resultado);
        assert_eq!(podman.builds().len(), 2, "{:?}", podman.builds());
        assert!(es_build_con_red(
            &podman.builds()[1]
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
        ));
        let h = c.resultado.hallazgos.first().expect("hay hallazgo");
        assert_eq!(h.severidad, 1);
        assert_eq!(h.detalle, DETALLE_NECESITA_RED);
        let gates = gates_de_repeticion(home.path());
        let runs = gates[0]["extra"]["pasos_del_dockerfile"]
            .as_array()
            .expect("lista de pasos");
        assert_eq!(runs.len(), 1, "{runs:?}");
        assert_eq!(runs[0]["cita"], "ADD https://ejemplo.com/x.tar.gz /x");
    }

    #[test]
    fn entrega_red_add_con_url_que_falla_sin_red_queda_pendiente_y_se_repite() {
        const SALIDA_ADD: &str =
            "STEP 1/3: FROM debian:12\nSTEP 2/3: ADD https://ejemplo.com/x.tar.gz /x\n";
        let sin_red = SalidaPodman {
            paso: None,
            codigo: 1,
            stdout: SALIDA_ADD.into(),
            stderr: "dial tcp: lookup ejemplo.com: Temporary failure in name resolution\n\
                Error: building at STEP \"ADD https://ejemplo.com/x.tar.gz /x\": exit status 1\n"
                .into(),
        };
        let repo = repo_con_archivos(&[("Dockerfile", DF_ADD_URL)]);
        let home = tempfile::tempdir().expect("home");
        let podman = PodmanFalso::nuevo(responde_build(sin_red, salida_ok()));
        let c = construir_con(
            repo.path(),
            home.path(),
            &tarea_de("shell", ""),
            &[],
            PLAZO_BUILD,
            &podman,
        )
        .expect("construye");
        assert!(c.resultado.con_red, "{:?}", c.resultado);
        assert!(
            c.resultado
                .hallazgos
                .iter()
                .all(|h| h.detalle != "la imagen no construye: línea 2"),
            "{:?}",
            c.resultado.hallazgos
        );
        assert_eq!(gates_de_repeticion(home.path()).len(), 1);
    }

    /// Dockerfile con un `RUN wget`: falla sin red, y la salida no trae ningún texto de red.
    const DF_WGET: &str = "FROM debian:12\nRUN wget -q http://example.com/x\nUSER app\n";

    /// Salida de `podman build` de un `RUN wget` que falla, sin ningún texto de red.
    fn salida_wget_que_falla() -> SalidaPodman {
        SalidaPodman {
            paso: None,
            codigo: 1,
            stdout: "STEP 1/2: FROM debian:12\nSTEP 2/2: RUN wget -q http://example.com/x\nboom\n"
                .into(),
            stderr: "boom\n\
                Error: building at STEP \"RUN wget -q http://example.com/x\": exit status 4\n"
                .into(),
        }
    }

    #[test]
    fn entrega_red_run_que_falla_sin_texto_de_red_se_repite_con_red() {
        // Sin mirar el texto: un RUN que falla sin red se repite con red igual.
        let repo = repo_con_archivos(&[("Dockerfile", DF_WGET)]);
        let home = tempfile::tempdir().expect("home");
        let podman = PodmanFalso::nuevo(responde_build(
            salida_wget_que_falla(),
            salida_wget_que_falla(),
        ));
        let c = construir_con(
            repo.path(),
            home.path(),
            &tarea_de("shell", ""),
            &[],
            PLAZO_BUILD,
            &podman,
        )
        .expect("construye");
        assert_eq!(podman.builds().len(), 2, "{:?}", podman.builds());
        assert!(
            podman.builds()[1].contains(&"pasta:--no-map-gw".to_string()),
            "{:?}",
            podman.builds()[1]
        );
        assert_eq!(gates_de_repeticion(home.path()).len(), 1);
        assert!(
            c.resultado.pendientes.is_empty(),
            "{:?}",
            c.resultado.pendientes
        );
    }

    #[test]
    fn entrega_red_con_sonda_verde_y_build_con_red_que_falla_da_rojo() {
        let repo = repo_con_archivos(&[("Dockerfile", DF_RED)]);
        let home = tempfile::tempdir().expect("home");
        // Con red, el `apt-get` falla sin texto de red: el paso de la línea 2 no construye.
        let con_red_falla = SalidaPodman {
            paso: None,
            codigo: 100,
            stdout: "STEP 1/2: FROM debian:12\nSTEP 2/2: RUN apt-get install -y curl\n".into(),
            stderr: "E: Unable to locate package curl\n\
                Error: building at STEP \"RUN apt-get install -y curl\": exit status 100\n"
                .into(),
        };
        let podman = PodmanFalso::nuevo(responde_build(salida_sin_red(), con_red_falla));
        let c = construir_con(
            repo.path(),
            home.path(),
            &tarea_de("shell", ""),
            &[],
            PLAZO_BUILD,
            &podman,
        )
        .expect("construye");
        let r = &c.resultado;
        assert!(!r.construida, "{r:?}");
        assert!(r.pendientes.is_empty(), "{:?}", r.pendientes);
        assert_eq!(r.hallazgos.len(), 1, "{:?}", r.hallazgos);
        assert_eq!(r.hallazgos[0].severidad, 3, "{:?}", r.hallazgos);
        assert!(
            r.hallazgos[0]
                .detalle
                .contains("la imagen no construye: línea 2"),
            "{:?}",
            r.hallazgos
        );
        assert_eq!(podman.builds().len(), 2, "{:?}", podman.builds());
    }

    #[test]
    fn entrega_red_con_sonda_roja_queda_pendiente_con_motivo_exacto_y_aviso() {
        let repo = repo_con_archivos(&[("Dockerfile", DF_RED)]);
        let home = tempfile::tempdir().expect("home");
        let podman = PodmanFalso::nuevo(responde_build_con_sonda(
            salida_sin_red(),
            salida_red_no_configurada(),
            false,
        ));
        let c = construir_con(
            repo.path(),
            home.path(),
            &tarea_de("shell", ""),
            &[],
            PLAZO_BUILD,
            &podman,
        )
        .expect("construye");
        let r = &c.resultado;
        assert!(!r.construida, "{r:?}");
        assert!(r.hallazgos.is_empty(), "{:?}", r.hallazgos);
        assert_eq!(r.pendientes.len(), 1, "{:?}", r.pendientes);
        assert_eq!(
            r.pendientes[0].motivo,
            "la red no se pudo configurar en este host"
        );
        assert!(
            c.avisos
                .iter()
                .any(|a| a.starts_with(AVISO_RED_NO_ARRANCO)
                    && a.contains("could not find slirp4netns")),
            "{:?}",
            c.avisos
        );
    }

    #[test]
    fn entrega_red_run_con_texto_setup_network_y_false_con_sonda_verde_da_rojo() {
        // El RUN imprime "setup network" y sale con 1. El texto del repo no decide: con sonda verde, rojo.
        const DF: &str = "FROM debian:12\nRUN echo \"setup network\"; false\nUSER app\n";
        let fallo = |stderr_previo: &str| SalidaPodman {
            paso: None,
            codigo: 1,
            stdout: "STEP 1/2: FROM debian:12\nSTEP 2/2: RUN echo \"setup network\"; false\n"
                .into(),
            stderr: format!(
                "{stderr_previo}Error: building at STEP \"RUN echo \"setup network\"; false\": exit status 1\n"
            ),
        };
        let repo = repo_con_archivos(&[("Dockerfile", DF)]);
        let home = tempfile::tempdir().expect("home");
        // Sin red, el texto del RUN trae falta de DNS: hay repetición. Con red, el texto del RUN trae "setup network".
        let podman = PodmanFalso::nuevo(responde_build(
            fallo("Temporary failure in name resolution\n"),
            fallo(""),
        ));
        let c = construir_con(
            repo.path(),
            home.path(),
            &tarea_de("shell", ""),
            &[],
            PLAZO_BUILD,
            &podman,
        )
        .expect("construye");
        let r = &c.resultado;
        assert!(r.pendientes.is_empty(), "{:?}", r.pendientes);
        assert_eq!(r.hallazgos.len(), 1, "{:?}", r.hallazgos);
        assert_eq!(r.hallazgos[0].linea, 2, "{:?}", r.hallazgos);
        assert!(
            !c.avisos.iter().any(|a| a.starts_with(AVISO_RED_NO_ARRANCO)),
            "{:?}",
            c.avisos
        );
    }

    #[test]
    fn entrega_copy_local_que_falla_da_rojo_sin_repetir() {
        const DF: &str = "FROM debian:12\nCOPY no-existe /x\nUSER app\n";
        let fallo = SalidaPodman {
            paso: None,
            codigo: 1,
            stdout: "STEP 1/2: FROM debian:12\nSTEP 2/2: COPY no-existe /x\n".into(),
            stderr: "Error: building at STEP \"COPY no-existe /x\": checking on sources under \"/ctx\": no such file or directory\n"
                .into(),
        };
        let repo = repo_con_archivos(&[("Dockerfile", DF)]);
        let home = tempfile::tempdir().expect("home");
        let podman = PodmanFalso::nuevo(responde_build(fallo, salida_ok()));
        let c = construir_con(
            repo.path(),
            home.path(),
            &tarea_de("shell", ""),
            &[],
            PLAZO_BUILD,
            &podman,
        )
        .expect("construye");
        let r = &c.resultado;
        assert!(r.pendientes.is_empty(), "{:?}", r.pendientes);
        assert_eq!(r.hallazgos.len(), 1, "{:?}", r.hallazgos);
        assert_eq!(r.hallazgos[0].linea, 2, "{:?}", r.hallazgos);
        assert_eq!(podman.builds().len(), 1, "{:?}", podman.builds());
        assert!(gates_de_repeticion(home.path()).is_empty());
    }

    #[test]
    fn entrega_paso_con_url_reconoce_add_y_copy_con_fuente_remota() {
        assert!(paso_con_url("ADD https://x.org/a.tar.gz /a"));
        assert!(paso_con_url("ADD --chown=app https://x.org/a /a"));
        assert!(paso_con_url("COPY git@github.com:x/y.git /y"));
        assert!(paso_con_url(r#"ADD ["https://x.org/a", "/a"]"#));
        assert!(paso_con_url("ADD git://x.org/y /y"));
        assert!(!paso_con_url("ADD ./local.tar.gz /x"));
        assert!(!paso_con_url("COPY ./src /app"));
        assert!(!paso_con_url("RUN wget https://x.org/a"));
        assert!(!paso_con_url("ADD ./https-notas.txt /x"));
    }

    // ---- Tarea 28, parte 1: `entregar_con` con podman y fábrica de escenarios inyectables ----

    /// Fábrica de escenarios falsa: cada `iniciar` cuenta la imagen que recibe y da un corredor que
    /// pasa cada escenario sin tocar ningún contenedor.
    struct FabricaFalsa {
        imagenes: RefCell<Vec<String>>,
    }

    impl FabricaEscenarios for FabricaFalsa {
        fn iniciar(
            &self,
            imagen: &str,
            _escenarios: Option<&Path>,
        ) -> Result<Box<dyn CorredorDeEscenario + '_>, String> {
            self.imagenes.borrow_mut().push(imagen.to_string());
            Ok(Box::new(CorredorFalso))
        }
    }

    /// Corredor de mentira: cada escenario pasa.
    struct CorredorFalso;

    impl CorredorDeEscenario for CorredorFalso {
        fn correr(&self, _escenario: &Escenario) -> Result<Prueba, ErrorOraculo> {
            Ok(Prueba::Paso)
        }
    }

    #[test]
    fn entrega_entregar_con_corre_el_escenario_en_la_imagen_y_lo_marca_verde() {
        let repo = repo_con_archivos(&[("Dockerfile", "FROM scratch\nUSER app\n")]);
        let home = tempfile::tempdir().expect("home");
        // La carpeta de la tarea tiene que seguir viva durante la prueba: `entregar` la exige.
        let dir = tempfile::tempdir().expect("temp dir");
        fs::write(
            dir.path().join("tarea.toml"),
            "nombre = \"x\"\nlenguaje = \"shell\"\nimagen = \"i\"\nspec = \"s\"\n\
             [contrato]\ncmd = \"c\"\n[oculto]\ncmd = \"o\"\n\
             [[escenario]]\nid = \"a\"\ncmd = \"exit 0\"\n",
        )
        .expect("escribe tarea.toml");
        let t = Tarea::cargar_de(dir.path()).expect("carga la tarea");
        let podman = PodmanFalso::nuevo(responde_build(salida_ok(), salida_ok()));
        let fabrica = FabricaFalsa {
            imagenes: RefCell::new(Vec::new()),
        };
        let r =
            entregar_con(repo.path(), &t, home.path(), &[], &podman, &fabrica).expect("entrega");
        assert_eq!(r.escenarios.get("a"), Some(&true), "{:?}", r.escenarios);
        assert_eq!(r.lugar_escenarios.get("a"), Some(&LugarEscenario::Imagen));
        assert_eq!(
            fabrica.imagenes.borrow().len(),
            1,
            "un contenedor de escenario"
        );
        assert!(r.sin_veredicto.is_empty(), "{:?}", r.sin_veredicto);
    }

    /// Fábrica que cuenta los contenedores de escenario que se abren.
    fn fabrica_vacia() -> FabricaFalsa {
        FabricaFalsa {
            imagenes: RefCell::new(Vec::new()),
        }
    }

    #[test]
    fn entrega_dockerfile_con_paso_fallido_por_red_sin_otros_hallazgos_no_da_valor() {
        let repo = repo_con_archivos(&[("Dockerfile", DF_RED)]);
        let home = tempfile::tempdir().expect("home");
        let podman = PodmanFalso::nuevo(responde_build_con_sonda(
            salida_sin_red(),
            salida_red_no_configurada(),
            false,
        ));
        let fabrica = fabrica_vacia();
        let r = entregar_con(
            repo.path(),
            &tarea_de("shell", ""),
            home.path(),
            &[],
            &podman,
            &fabrica,
        )
        .expect("entrega");
        assert!(r.sin_veredicto.is_empty(), "{:?}", r.sin_veredicto);
        assert!(
            r.pendientes.iter().any(|p| p.test == "dockerfile::build"),
            "{:?}",
            r.pendientes
        );
        assert_eq!(r.sonda.valor, None, "{:?}", r.sonda);
        assert!(
            r.sonda.motivo.as_deref().is_some_and(|m| !m.is_empty()),
            "{:?}",
            r.sonda
        );
    }

    #[test]
    fn entrega_sin_ningun_item_medido_da_sin_veredicto() {
        let repo = tempfile::tempdir().expect("repo");
        let home = tempfile::tempdir().expect("home");
        let podman = PodmanFalso::nuevo(responde_build(salida_ok(), salida_ok()));
        let fabrica = fabrica_vacia();
        let r = entregar_con(
            repo.path(),
            &tarea_de("shell", ""),
            home.path(),
            &[],
            &podman,
            &fabrica,
        )
        .expect("entrega");
        assert_eq!(r.sonda.valor, None, "{:?}", r.sonda);
        assert_eq!(
            r.sonda.motivo.as_deref(),
            Some("entrega sin ítems medidos"),
            "{:?}",
            r.sonda
        );
    }

    #[test]
    fn entrega_imagen_base_sin_descargar_da_sin_veredicto_en_la_sonda() {
        let repo = repo_con_archivos(&[("Dockerfile", "FROM 127.0.0.1:1/x:1\nUSER 1000\n")]);
        let home = tempfile::tempdir().expect("home");
        let imagenes = [ImagenBase {
            archivo: "Dockerfile".into(),
            linea: 1,
            cita: "FROM 127.0.0.1:1/x:1".into(),
            imagen: "127.0.0.1:1/x:1".into(),
            resuelta: Some("127.0.0.1:1/x:1".into()),
            digest: None,
            descargada: false,
            sin_resolver: None,
            problema: Some(Problema::PideCredenciales),
        }];
        let podman = PodmanFalso::nuevo(responde_build(salida_ok(), salida_ok()));
        let fabrica = fabrica_vacia();
        let r = entregar_con(
            repo.path(),
            &tarea_de("shell", ""),
            home.path(),
            &imagenes,
            &podman,
            &fabrica,
        )
        .expect("entrega");
        assert_eq!(podman.total(), 0, "{:?}", podman.builds());
        assert_eq!(r.sonda.valor, None, "{:?}", r.sonda);
        assert!(
            r.sonda
                .motivo
                .as_deref()
                .is_some_and(|m| m.contains("pide credenciales")),
            "{:?}",
            r.sonda
        );
    }

    #[test]
    fn entrega_dockerfile_que_construye_sin_hallazgos_sigue_con_valor() {
        let repo = repo_con_archivos(&[("Dockerfile", "FROM scratch\nUSER app\n")]);
        let home = tempfile::tempdir().expect("home");
        let podman = PodmanFalso::nuevo(responde_build(salida_ok(), salida_ok()));
        let fabrica = fabrica_vacia();
        let r = entregar_con(
            repo.path(),
            &tarea_de("shell", ""),
            home.path(),
            &[],
            &podman,
            &fabrica,
        )
        .expect("entrega");
        assert!(r.pendientes.is_empty(), "{:?}", r.pendientes);
        assert!(r.sonda.hallazgos.is_empty(), "{:?}", r.sonda.hallazgos);
        assert_eq!(r.sonda.valor, Some(1.0), "{:?}", r.sonda);
        assert_eq!(r.sonda.motivo, None);
    }

    #[test]
    fn entrega_build_pendiente_lleva_su_item_a_faltantes_con_su_motivo() {
        let repo =
            repo_con_archivos(&[("Dockerfile", "FROM scratch\nRUN apt-get install -y curl\n")]);
        let home = tempfile::tempdir().expect("home");
        let podman = PodmanFalso::nuevo(responde_build_con_sonda(
            salida_sin_red(),
            salida_red_no_configurada(),
            false,
        ));
        let fabrica = fabrica_vacia();
        let r = entregar_con(
            repo.path(),
            &tarea_de("shell", ""),
            home.path(),
            &[],
            &podman,
            &fabrica,
        )
        .expect("entrega");
        assert_eq!(r.faltantes.len(), 1, "{:?}", r.faltantes);
        assert_eq!(r.faltantes[0].item, "entrega.contenedor");
        assert_eq!(
            r.faltantes[0].motivo, "la red no se pudo configurar en este host",
            "{:?}",
            r.faltantes
        );
    }

    #[test]
    fn entrega_build_pendiente_sin_hallazgos_sigue_sin_veredicto_con_su_item() {
        let repo = repo_con_archivos(&[(
            "Dockerfile",
            "FROM scratch\nUSER app\nRUN apt-get install -y curl\n",
        )]);
        let home = tempfile::tempdir().expect("home");
        let podman = PodmanFalso::nuevo(responde_build_con_sonda(
            salida_sin_red(),
            salida_red_no_configurada(),
            false,
        ));
        let fabrica = fabrica_vacia();
        let r = entregar_con(
            repo.path(),
            &tarea_de("shell", ""),
            home.path(),
            &[],
            &podman,
            &fabrica,
        )
        .expect("entrega");
        assert!(r.sonda.hallazgos.is_empty(), "{:?}", r.sonda.hallazgos);
        assert_eq!(r.sonda.valor, None, "{:?}", r.sonda);
        assert_eq!(r.faltantes.len(), 1, "{:?}", r.faltantes);
        assert_eq!(r.faltantes[0].item, "entrega.contenedor");
    }

    #[test]
    fn entrega_build_pendiente_con_hallazgo_de_root_baja_el_valor() {
        // Sin `USER`, el Dockerfile corre como root: la sonda estática lo mide aunque el build quede pendiente.
        let repo = repo_con_archivos(&[(
            "Dockerfile",
            "FROM debian:12\nRUN apt-get install -y curl\n",
        )]);
        let home = tempfile::tempdir().expect("home");
        let podman = PodmanFalso::nuevo(responde_build_con_sonda(
            salida_sin_red(),
            salida_red_no_configurada(),
            false,
        ));
        let fabrica = fabrica_vacia();
        let r = entregar_con(
            repo.path(),
            &tarea_de("shell", ""),
            home.path(),
            &[],
            &podman,
            &fabrica,
        )
        .expect("entrega");
        assert!(r.sin_veredicto.is_empty(), "{:?}", r.sin_veredicto);
        assert!(
            r.pendientes.iter().any(|p| p.test == "dockerfile::build"),
            "{:?}",
            r.pendientes
        );
        assert!(
            r.sonda
                .hallazgos
                .iter()
                .any(|h| h.cita == "FROM debian:12" && h.archivo == "Dockerfile"),
            "{:?}",
            r.sonda.hallazgos
        );
        let valor = r.sonda.valor.expect("valor");
        assert!(valor < 1.0, "{:?}", r.sonda);
        assert_eq!(r.sonda.motivo, None);
    }

    #[test]
    fn entrega_salida_mayor_que_la_cola_conserva_solo_los_ultimos_bytes() {
        let cola = recortar_cola(format!("{}FIN", "a".repeat(3 * COLA)));
        assert_eq!(cola.len(), COLA, "{}", cola.len());
        assert!(cola.ends_with("FIN"));
    }

    #[test]
    fn entrega_cola_no_parte_un_caracter_utf8() {
        // Cada 'é' ocupa dos bytes: el corte natural cae dentro de uno.
        let cola = recortar_cola(format!("{}FIN", "é".repeat(3 * COLA)));
        assert!(cola.len() <= COLA, "{}", cola.len());
        assert!(cola.ends_with("FIN"));
        assert!(
            cola.chars()
                .all(|c| c == 'é' || c == 'F' || c == 'I' || c == 'N')
        );
    }

    #[test]
    fn entrega_salida_de_podman_recorta_stdout_y_stderr() {
        let salida = recortar_salida(SalidaPodman {
            paso: None,
            codigo: 1,
            stdout: format!("{}FIN", "o".repeat(2 * COLA)),
            stderr: format!("{}ERR", "e".repeat(2 * COLA)),
        });
        assert_eq!(salida.codigo, 1);
        assert_eq!(salida.stdout.len(), COLA);
        assert!(salida.stdout.ends_with("FIN"));
        assert_eq!(salida.stderr.len(), COLA);
        assert!(salida.stderr.ends_with("ERR"));
    }

    #[test]
    fn entrega_build_con_step_seguido_de_mas_de_1_mib_da_la_imagen_no_construye() {
        // Los STEP van por stdout y luego siguen más de 2 MiB de salida. La cola de 1 MiB pierde el STEP.
        // Al final, la línea de error de podman en stderr: es la que cuenta.
        let script = "printf 'STEP 1/3: FROM x\\nSTEP 2/3: RUN a\\nSTEP 3/5: RUN x\\n'; \
                      head -c 2500000 /dev/zero | tr '\\0' y; echo; \
                      echo 'Error: building at STEP \"RUN x\": while running runtime: exit status 1' >&2; \
                      exit 1";
        let salida = correr_podman_build("sh", &["-c", script], PLAZO_BUILD).expect("corre");
        let texto = "FROM x\nRUN a\nRUN x\n";
        match clasifica_con_red(&salida, texto) {
            Desenlace::Hallazgo(h) => assert!(
                h.detalle.contains("la imagen no construye: línea 3"),
                "{}",
                h.detalle
            ),
            otro => panic!("debía dar hallazgo, no {}", nombre(&otro)),
        }
    }

    #[test]
    fn entrega_build_con_step_y_8_kb_de_error_da_la_imagen_no_construye() {
        // El STEP va al principio del stderr, luego siguen 8 KB de error sin saltos de línea,
        // y al final la línea de error de podman.
        let script = "printf 'STEP 1/2: FROM debian:12\\nSTEP 2/2: RUN false\\n' >&2; \
                      head -c 8192 /dev/zero | tr '\\0' e >&2; echo >&2; \
                      echo 'Error: building at STEP \"RUN false\": exit status 1' >&2; exit 1";
        let salida = correr_podman_build("sh", &["-c", script], PLAZO_BUILD).expect("corre");
        let texto = "FROM debian:12\nRUN false\n";
        match clasifica_con_red(&salida, texto) {
            Desenlace::Hallazgo(h) => assert!(
                h.detalle.contains("la imagen no construye"),
                "{}",
                h.detalle
            ),
            otro => panic!("debía dar hallazgo, no {}", nombre(&otro)),
        }
    }

    // ---- Tarea 91, parte 2a: `Sesion` sobre un `Exec` inyectable ----

    fn escenario_de(cmd: &str) -> Escenario {
        Escenario {
            id: "e".into(),
            cmd: cmd.into(),
            red: false,
        }
    }

    /// Sesión de prueba con nombre `n` sobre `exec`. `sh` es `/usr/bin/sh`; `timeout` es `/usr/bin/timeout` si `tiene_timeout`.
    fn sesion_de_prueba(exec: &dyn Exec, tiene_timeout: bool) -> Sesion<'_> {
        Sesion {
            nombre: "n".into(),
            exec,
            ruta_sh: "/usr/bin/sh".into(),
            ruta_timeout: tiene_timeout.then(|| "/usr/bin/timeout".into()),
        }
    }

    /// Envoltorio de cada paso sin activación: PATH del sistema primero, `sh` por ruta, `125` pasa a `1`.
    const ENVOLTORIO_SIN_ACTIVAR: &str = "export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:$PATH; /usr/bin/sh -c \"$1\"; s=$?; [ \"$s\" -eq 125 ] && s=1; exit \"$s\"";

    /// Envoltorio con plazo: el `timeout 120` va dentro, antes del `sh` que corre el `cmd`. Ambos por ruta.
    const ENVOLTORIO_CON_TIMEOUT: &str = "export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:$PATH; /usr/bin/timeout 120 /usr/bin/sh -c \"$1\"; s=$?; [ \"$s\" -eq 125 ] && s=1; exit \"$s\"";

    /// Corre el envoltorio de `paso_args` de verdad, en el `sh` local, y da su código de salida.
    /// Sirve para probar el 125 del producto sin `podman`: el `sh` local recibe los mismos argumentos.
    fn codigo_del_envoltorio(cmd: &str) -> i32 {
        let exec = ExecFalso::con_codigos(&[]);
        let s = Sesion {
            nombre: "n".into(),
            exec: &exec,
            ruta_sh: "/bin/sh".into(),
            ruta_timeout: None,
        };
        let args = s.paso_args(&Entorno::default(), cmd, false);
        // Quita `exec` y el nombre del contenedor: queda `sh -c <envoltorio> _ <cmd>`.
        let (programa, resto) = args[2..].split_first().expect("hay argumentos");
        let salida = std::process::Command::new(programa)
            .args(resto)
            .output()
            .expect("corre el envoltorio con sh");
        salida.status.code().expect("sin código de salida")
    }

    /// Ejecutor de mentira que responde con el código real del envoltorio para `cmd`.
    fn exec_con_envoltorio(cmd: &str) -> ExecFalso {
        ExecFalso::con_codigos(&[codigo_del_envoltorio(cmd)])
    }

    #[test]
    fn sesion_paso_args_de_la_imagen_con_timeout() {
        let exec = ExecFalso::con_codigos(&[]);
        let s = sesion_de_prueba(&exec, true);
        assert_eq!(
            s.paso_args(&Entorno::default(), "exit 0", true),
            [
                "exec",
                "n",
                "sh",
                "-c",
                ENVOLTORIO_CON_TIMEOUT,
                "_",
                "exit 0",
            ]
        );
    }

    #[test]
    fn sesion_paso_args_con_timeout_lo_pone_dentro_del_envoltorio() {
        // El `timeout` va DENTRO del envoltorio. Así el `sh` de afuera sigue vivo para leer `$?`.
        let exec = ExecFalso::con_codigos(&[]);
        let s = sesion_de_prueba(&exec, true);
        let args = s.paso_args(&Entorno::default(), "exit 0", true);
        assert_eq!(args[2], "sh", "{args:?}");
        assert!(!args.iter().any(|a| a == "timeout"), "{args:?}");
        assert!(
            args[4].contains("/usr/bin/timeout 120 /usr/bin/sh -c \"$1\""),
            "{args:?}"
        );
    }

    #[test]
    fn sesion_paso_args_de_la_imagen_sin_timeout() {
        let exec = ExecFalso::con_codigos(&[]);
        let sin_pedirlo = sesion_de_prueba(&exec, true);
        assert_eq!(
            sin_pedirlo.paso_args(&Entorno::default(), "exit 0", false),
            [
                "exec",
                "n",
                "sh",
                "-c",
                ENVOLTORIO_SIN_ACTIVAR,
                "_",
                "exit 0"
            ]
        );
        // La imagen sin `timeout` no lo lleva, aunque se pida.
        let sin_imagen = sesion_de_prueba(&exec, false);
        assert_eq!(
            sin_imagen.paso_args(&Entorno::default(), "exit 0", true),
            [
                "exec",
                "n",
                "sh",
                "-c",
                ENVOLTORIO_SIN_ACTIVAR,
                "_",
                "exit 0"
            ]
        );
    }

    #[test]
    fn sesion_paso_args_con_entorno_lleva_directorio_y_activacion() {
        let exec = ExecFalso::con_codigos(&[]);
        let entorno = Entorno {
            dir: Some("/salida/work".into()),
            activar: "export A=1;".into(),
        };
        let s = sesion_de_prueba(&exec, true);
        assert_eq!(
            s.paso_args(&entorno, "pytest -q", true),
            [
                "exec",
                "-w",
                "/salida/work",
                "n",
                "sh",
                "-c",
                "export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:$PATH; export A=1; /usr/bin/timeout 120 /usr/bin/sh -c \"$1\"; s=$?; [ \"$s\" -eq 125 ] && s=1; exit \"$s\"",
                "_",
                "pytest -q",
            ]
        );
    }

    #[test]
    fn sesion_paso_args_pone_el_path_del_sistema_antes_de_la_activacion() {
        // El PATH del sistema va antes de la activación. La activación de Root (venv, pydeps) queda primero.
        let exec = ExecFalso::con_codigos(&[]);
        let s = sesion_de_prueba(&exec, false);
        let entorno = Entorno {
            dir: None,
            activar: "export PATH=/tmp/pydeps/bin:$PATH;".into(),
        };
        let args = s.paso_args(&entorno, "curl --version", false);
        let envoltorio = &args[4];
        let activacion = envoltorio
            .find("export PATH=/tmp/pydeps/bin:$PATH;")
            .expect("la activación va en el envoltorio");
        let sistema = envoltorio
            .find("export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:$PATH")
            .expect("el PATH del sistema va en el envoltorio");
        assert!(sistema < activacion, "{envoltorio}");
    }

    #[test]
    fn sesion_correr_escenario_con_125_del_producto_es_fallo() {
        // El 125 del producto sale como 1 por el envoltorio: es rojo del producto, no de podman.
        let exec = exec_con_envoltorio("exit 125");
        let s = sesion_de_prueba(&exec, false);
        match s
            .correr_escenario(&Entorno::default(), &escenario_de("exit 125"))
            .expect("corre")
        {
            Prueba::Fallo { codigo, .. } => assert_eq!(codigo, 1),
            _ => panic!("el 125 del producto debe ser un fallo"),
        }
    }

    #[test]
    fn envoltorio_deja_pasar_126_y_127_como_codigos_del_producto() {
        assert_eq!(codigo_del_envoltorio("exit 0"), 0);
        assert_eq!(codigo_del_envoltorio("exit 1"), 1);
        assert_eq!(codigo_del_envoltorio("exit 126"), 126);
        assert_eq!(codigo_del_envoltorio("exit 127"), 127);
    }

    #[test]
    fn sesion_correr_escenario_con_codigo_0_pasa() {
        let exec = ExecFalso::con_codigos(&[0]);
        let s = sesion_de_prueba(&exec, true);
        let r = s
            .correr_escenario(&Entorno::default(), &escenario_de("exit 0"))
            .expect("corre");
        assert!(matches!(r, Prueba::Paso));
    }

    #[test]
    fn sesion_correr_escenario_con_fallo_cita_la_ultima_linea() {
        let exec = ExecFalso::con(vec![Ok(salida(1, "boom\n"))]);
        let s = sesion_de_prueba(&exec, true);
        match s
            .correr_escenario(&Entorno::default(), &escenario_de("false"))
            .expect("corre")
        {
            Prueba::Fallo { codigo, cita } => {
                assert_eq!(codigo, 1);
                assert_eq!(cita, "boom");
            }
            _ => panic!("debía fallar"),
        }
    }

    #[test]
    fn sesion_correr_escenario_con_codigo_125_es_infraestructura() {
        let exec = ExecFalso::con_codigos(&[125]);
        let s = sesion_de_prueba(&exec, true);
        let r = s.correr_escenario(&Entorno::default(), &escenario_de("exit 0"));
        assert!(matches!(r, Err(ErrorOraculo::Infra(_))));
    }

    #[test]
    fn sesion_correr_escenario_con_texto_de_red_y_sin_red_es_fallo() {
        // La salida no decide la clasificación: sin `red`, salir 1 es fallo aunque diga "could not resolve host".
        let exec = ExecFalso::con(vec![Ok(salida(1, "Could not resolve host: x\n"))]);
        let s = sesion_de_prueba(&exec, true);
        match s
            .correr_escenario(&Entorno::default(), &escenario_de("wget x"))
            .expect("corre")
        {
            Prueba::Fallo { codigo, cita } => {
                assert_eq!(codigo, 1);
                assert!(cita.contains("Could not resolve host"), "{cita}");
            }
            _ => panic!("debía fallar"),
        }
    }

    #[test]
    fn sesion_correr_escenario_con_red_queda_pendiente_sin_correrlo() {
        let exec = ExecFalso::vacio();
        let s = sesion_de_prueba(&exec, true);
        let escenario = Escenario {
            id: "e".into(),
            cmd: "wget x".into(),
            red: true,
        };
        match s
            .correr_escenario(&Entorno::default(), &escenario)
            .expect("corre")
        {
            Prueba::Pendiente(motivo) => {
                assert!(motivo.contains("el escenario pide red"), "{motivo}")
            }
            _ => panic!("debía quedar pendiente"),
        }
        assert!(exec.llamadas().is_empty(), "{:?}", exec.llamadas());
    }

    #[test]
    fn sesion_correr_escenario_que_supera_el_plazo_da_124() {
        let exec = ExecFalso::con(vec![Err(ErrorOraculo::Infra(
            "podman exec superó 120 s".into(),
        ))]);
        let s = sesion_de_prueba(&exec, true);
        match s
            .correr_escenario(&Entorno::default(), &escenario_de("sleep 999"))
            .expect("corre")
        {
            Prueba::Fallo { codigo, cita } => {
                assert_eq!(codigo, CODIGO_TIEMPO);
                assert!(cita.contains("120"), "{cita}");
            }
            _ => panic!("debía fallar por tiempo"),
        }
    }

    #[test]
    fn sesion_correr_escenario_de_la_imagen_manda_los_mismos_argumentos() {
        // GUARDA: el camino de la imagen manda el envoltorio por `Exec`, con el `timeout` dentro.
        let exec = ExecFalso::con(vec![Ok(salida(0, ""))]);
        let s = sesion_de_prueba(&exec, true);
        s.correr_escenario(&Entorno::default(), &escenario_de("exit 0"))
            .expect("corre");
        assert_eq!(
            exec.llamadas()[0],
            [
                "exec",
                "n",
                "sh",
                "-c",
                ENVOLTORIO_CON_TIMEOUT,
                "_",
                "exit 0"
            ]
        );
    }

    #[test]
    fn sesion_iniciar_detecta_sh_y_timeout_por_el_ejecutor() {
        let exec = ExecFalso::con(vec![
            Ok(salida(0, "")),
            Ok(salida(0, "true\n")),
            Ok(salida(0, "sh=/usr/bin/sh\ntimeout=/usr/bin/timeout\n")),
        ]);
        let arranque = |c: &[&str]| vec!["run".to_string(), c[0].to_string()];
        let s = Sesion::iniciar(&exec, "n", &arranque).expect("arranca");
        assert_eq!(s.ruta_sh, "/usr/bin/sh");
        assert_eq!(s.ruta_timeout.as_deref(), Some("/usr/bin/timeout"));
        assert_eq!(exec.llamadas()[0], ["run", "sleep"]);
    }

    #[test]
    fn sesion_iniciar_rechaza_sh_bajo_tmp() {
        let exec = ExecFalso::con(vec![
            Ok(salida(0, "")),
            Ok(salida(0, "true\n")),
            Ok(salida(
                0,
                "sh=/tmp/venv-eval/bin/sh\ntimeout=/usr/bin/timeout\n",
            )),
            Ok(salida(0, "")),
        ]);
        let arranque = |c: &[&str]| vec!["run".to_string(), c[0].to_string()];
        match Sesion::iniciar(&exec, "n", &arranque) {
            Err(motivo) => assert!(motivo.contains("la ruta de sh no es válida"), "{motivo}"),
            Ok(_) => panic!("un sh bajo /tmp no debe arrancar"),
        }
        let ultima = exec.llamadas().pop().expect("hay llamadas");
        assert_eq!(ultima, ["rm", "-f", "-t", "0", "n"]);
    }

    #[test]
    fn sesion_iniciar_timeout_bajo_tmp_se_ignora() {
        let exec = ExecFalso::con(vec![
            Ok(salida(0, "")),
            Ok(salida(0, "true\n")),
            Ok(salida(
                0,
                "sh=/usr/bin/sh\ntimeout=/tmp/pydeps/bin/timeout\n",
            )),
        ]);
        let arranque = |c: &[&str]| vec!["run".to_string(), c[0].to_string()];
        let s = Sesion::iniciar(&exec, "n", &arranque).expect("arranca");
        assert_eq!(s.ruta_sh, "/usr/bin/sh");
        assert_eq!(s.ruta_timeout, None);
    }

    #[test]
    fn envoltorio_con_activacion_usa_rutas_absolutas_de_sh_y_timeout() {
        let exec = ExecFalso::con_codigos(&[]);
        let s = Sesion {
            nombre: "n".into(),
            exec: &exec,
            ruta_sh: "/usr/bin/sh".into(),
            ruta_timeout: Some("/usr/bin/timeout".into()),
        };
        let entorno = Entorno {
            dir: None,
            activar: "export VIRTUAL_ENV=/tmp/venv-eval;".into(),
        };
        let args = s.paso_args(&entorno, "exit 0", true);
        let envoltorio = &args[4];
        assert!(
            envoltorio.contains("/usr/bin/timeout 120 /usr/bin/sh -c \"$1\""),
            "{envoltorio}"
        );
        assert!(!envoltorio.contains(" sh -c"), "{envoltorio}");
        assert!(!envoltorio.contains(" timeout 120 sh"), "{envoltorio}");
    }

    #[test]
    fn ruta_de_sistema_valida_tabla() {
        let casos = [
            ("/usr/bin/sh", true),
            ("sh", false),
            ("", false),
            ("/tmp/x/sh", false),
            ("/tmp", false),
            ("/salida/work/bin/sh", false),
            ("/repo/sh", false),
            ("/tmpx/sh", true),
            ("/usr/bin/../tmp/sh", false),
        ];
        for (ruta, esperado) in casos {
            assert_eq!(ruta_de_sistema_valida(ruta), esperado, "{ruta:?}");
        }
    }

    #[test]
    fn sesion_iniciar_sin_sh_da_el_motivo_y_quita_el_contenedor() {
        let exec = ExecFalso::con(vec![
            Ok(salida(0, "")),
            Ok(salida(0, "true\n")),
            Ok(salida(127, "")),
            Ok(salida(0, "")),
        ]);
        let arranque = |c: &[&str]| vec!["run".to_string(), c[0].to_string()];
        match Sesion::iniciar(&exec, "n", &arranque) {
            Err(motivo) => assert_eq!(motivo, "la imagen no trae sh"),
            Ok(_) => panic!("sin sh no debe arrancar"),
        }
        let ultima = exec.llamadas().pop().expect("hay llamadas");
        assert_eq!(ultima, ["rm", "-f", "-t", "0", "n"]);
    }

    #[test]
    fn sesion_iniciar_incluye_el_stderr_de_cada_run_fallido() {
        let error = |stderr: &str| -> Result<SalidaPodman, ErrorOraculo> {
            Ok(SalidaPodman {
                codigo: 125,
                stdout: String::new(),
                stderr: stderr.into(),
                paso: None,
            })
        };
        // Cada `run` fallido va seguido de su `rm -f` de `quitar`.
        let exec = ExecFalso::con(vec![
            error("Error: no such file or directory"),
            Ok(salida(0, "")),
            error("Error: no such file or directory"),
            Ok(salida(0, "")),
        ]);
        let arranque = |c: &[&str]| vec!["run".to_string(), c[0].to_string()];
        match Sesion::iniciar(&exec, "n", &arranque) {
            Err(motivo) => {
                assert!(motivo.contains("no such file or directory"), "{motivo}");
                assert!(motivo.contains("sleep 600"), "{motivo}");
                assert!(motivo.contains("tail -f /dev/null"), "{motivo}");
                assert!(
                    motivo.contains("la imagen no trae sleep ni tail"),
                    "{motivo}"
                );
            }
            Ok(_) => panic!("sin arranque no debe haber sesión"),
        }
    }

    #[test]
    fn iniciar_evaluacion_valida_rutas_antes_de_podman() {
        let trabajo = tempfile::tempdir().expect("temp dir");
        let faltante = trabajo.path().join("no-existe");
        let ejecucion = Ejecucion {
            imagen: "docker.io/library/alpine:3.20".into(),
            repo: None,
            oraculo: None,
            oraculo_destino: None,
            salida: trabajo.path().to_path_buf(),
            cmd: String::new(),
            env: Vec::new(),
            timeout: Duration::from_secs(60),
            red: false,
            cache: Some(faltante.clone()),
            cache_rw: false,
            rustsec: None,
            trabajo: None,
            secretos: Vec::new(),
        };
        match FabricaReal.iniciar_evaluacion(&ejecucion) {
            Err(motivo) => assert!(motivo.contains(&faltante.display().to_string()), "{motivo}"),
            Ok(_) => panic!("una ruta que no existe no debe arrancar"),
        }
    }

    // ---- Tarea 91, parte 2b: la evaluación corre por pasos, cada uno con su `podman exec` ----

    #[test]
    fn eval_entorno_usa_work_con_entrega_y_repo_sin_ella() {
        let con = entorno_de_evaluacion(&tarea_de("python", ENTREGA));
        assert_eq!(con.dir.as_deref(), Some("/salida/work"));
        assert!(con.activar.contains("venv-eval/bin/activate"));
        let sin = entorno_de_evaluacion(&tarea_de("rust", ESCENARIOS));
        assert_eq!(sin.dir.as_deref(), Some("/repo"));
        assert_eq!(sin.activar, ACTIVAR_RUST);
        assert_eq!(
            entorno_de_evaluacion(&tarea_de("shell", ENTREGA)).activar,
            ""
        );
    }

    #[test]
    fn eval_orden_de_los_pasos_con_entrega_y_escenarios() {
        let repo = repo_completo();
        let fabrica =
            FabricaEvalFalsa::con(ExecFalso::vacio().responde("exit 1", Resp::Codigo(1, "")));
        let t = tarea_de("python", &format!("{ENTREGA}\n{ESCENARIOS}"));
        let r = sonda_con(&fabrica, repo.path(), &t);
        let e = &fabrica.exec;
        let orden = [
            e.posicion("mkdir -p /tmp/fuentes"),
            e.posicion("cp -a /repo /salida/work"),
            e.posicion("make"),
            e.posicion("./run"),
            e.posicion("exit 0"),
            e.posicion("exit 1"),
            e.posicion("terminar"),
            e.posicion("rm -f"),
        ];
        assert!(
            orden.windows(2).all(|p| p[0] < p[1]),
            "orden {orden:?}: {:?}",
            e.llamadas()
        );
        assert_eq!(r.escenarios.get("a"), Some(&true));
        assert_eq!(r.escenarios.get("b"), Some(&false));
    }

    #[test]
    fn eval_cada_escenario_repite_la_activacion_de_python() {
        let repo = repo_completo();
        let fabrica = sana();
        sonda_con(
            &fabrica,
            repo.path(),
            &tarea_de("python", &format!("{ENTREGA}\n{ESCENARIOS}")),
        );
        let escenario_a = fabrica
            .exec
            .llamadas()
            .into_iter()
            .find(|l| l.join(" ").contains("exit 0"))
            .expect("corre el escenario a");
        assert!(
            escenario_a.join(" ").contains("venv-eval"),
            "{escenario_a:?}"
        );
    }

    #[test]
    fn eval_arranque_usa_el_contenedor_separado_sin_red_y_sin_rm() {
        let repo = repo_completo();
        let fabrica = sana();
        sonda_con(&fabrica, repo.path(), &tarea_de("shell", ENTREGA));
        let arranque = &fabrica.exec.llamadas()[0];
        assert_eq!(arranque[0], "run");
        assert!(arranque.contains(&"-d".to_string()), "{arranque:?}");
        assert!(arranque.contains(&"sleep".to_string()), "{arranque:?}");
        assert!(
            arranque.contains(&"none".to_string()),
            "sin red: {arranque:?}"
        );
        assert!(!arranque.contains(&"--rm".to_string()), "lo quita Root");
    }

    #[test]
    fn eval_producto_que_escribe_rc_no_cambia_el_resultado() {
        // El producto escribe 0 en el `.rc` del escenario `a`, pero su código de salida es 1.
        let repo = repo_completo();
        let fabrica = FabricaEvalFalsa {
            exec: ExecFalso::vacio().responde("exit 0", Resp::Codigo(1, "fallo\n")),
            arranque: None,
            archivos: vec![
                ("entrega-escenario-a.rc", "0\n"),
                ("entrega-escenario-b.rc", "0\n"),
            ],
            aperturas: Cell::new(0),
        };
        let t = tarea_de("shell", &format!("{ENTREGA}\n{ESCENARIOS}"));
        let r = sonda_con(&fabrica, repo.path(), &t);
        assert_eq!(
            r.escenarios.get("a"),
            Some(&false),
            "manda el código del exec"
        );
        assert!(fabrica.exec.hubo("exit 0"));
    }

    #[test]
    fn eval_correr_que_termina_antes_da_no_arranca_y_los_escenarios_siguen() {
        let repo = repo_completo();
        let fabrica =
            FabricaEvalFalsa::con(ExecFalso::vacio().proceso(false, "arrancando\nadios\n"));
        let t = tarea_de("shell", &format!("{ENTREGA}\n{ESCENARIOS}"));
        let r = sonda_con(&fabrica, repo.path(), &t);
        let h = r
            .sonda
            .hallazgos
            .iter()
            .find(|h| h.detalle.contains("no arranca"))
            .expect("hallazgo de no arranca");
        assert_eq!((h.severidad, h.cita.as_str()), (3, "adios"));
        assert!(r.escenarios.contains_key("a") && r.escenarios.contains_key("b"));
    }

    #[test]
    fn eval_correr_vivo_no_da_hallazgo_y_se_termina_al_final() {
        let repo = repo_completo();
        let fabrica = sana();
        let r = sonda_con(&fabrica, repo.path(), &tarea_de("shell", ENTREGA));
        assert!(
            !r.sonda
                .hallazgos
                .iter()
                .any(|h| h.detalle.contains("no arranca"))
        );
        assert!(fabrica.exec.hubo("terminar"), "el hijo se termina");
        assert_eq!(r.sonda.valor, Some(1.0));
    }

    #[test]
    fn eval_instalar_que_falla_corta_escenarios_y_correr() {
        let repo = repo_completo();
        let fabrica =
            FabricaEvalFalsa::con(ExecFalso::vacio().responde("make", Resp::Codigo(1, "roto\n")));
        let t = tarea_de("shell", &format!("{ENTREGA}\n{ESCENARIOS}"));
        let r = sonda_con(&fabrica, repo.path(), &t);
        assert!(!fabrica.exec.hubo("./run"), "correr no se lanza");
        assert!(!fabrica.exec.hubo("exit 0"), "ningún escenario corre");
        assert!(fabrica.exec.hubo("rm -f"), "el contenedor se quita igual");
        assert_eq!(r.sonda.motivo, None);
        assert!(r.escenarios.values().all(|v| !v));
        assert!(
            r.sonda
                .hallazgos
                .iter()
                .any(|h| h.detalle.contains("no instala") && h.cita == "roto")
        );
        let no_corrio = r
            .sonda
            .hallazgos
            .iter()
            .filter(|h| h.detalle.contains("el producto no instala"))
            .count();
        assert_eq!(no_corrio, 2);
    }

    #[test]
    fn eval_instalar_con_codigo_125_es_infraestructura() {
        // GUARDA: un 125 que llega sin pasar por el envoltorio sale de podman, así que es infraestructura.
        let repo = repo_completo();
        let fabrica =
            FabricaEvalFalsa::con(ExecFalso::vacio().responde("make", Resp::Codigo(125, "")));
        let r = sonda_con(&fabrica, repo.path(), &tarea_de("shell", ENTREGA));
        assert_eq!(r.sonda.valor, None);
        assert!(
            r.sonda
                .motivo
                .as_deref()
                .unwrap_or_default()
                .contains("instalar")
        );
        assert!(
            !r.sonda
                .hallazgos
                .iter()
                .any(|h| h.detalle.contains("no instala"))
        );
        assert!(!fabrica.exec.hubo("./run"));
    }

    #[test]
    fn eval_instalar_con_125_del_producto_no_instala() {
        // El `exit 125` del producto sale como 1 por el envoltorio: es un hallazgo de "no instala".
        let repo = repo_completo();
        let entrega = "[entrega]\ninstalar = \"exit 125\"\ncorrer = \"./run\"\n\
            espera_segundos = 0\narchivos = [\"README.md\", \"Dockerfile\"]\n";
        let fabrica = FabricaEvalFalsa::con(ExecFalso::vacio().responde(
            "exit 125",
            Resp::Codigo(codigo_del_envoltorio("exit 125"), ""),
        ));
        let r = sonda_con(&fabrica, repo.path(), &tarea_de("shell", entrega));
        assert!(
            r.sonda
                .hallazgos
                .iter()
                .any(|h| h.detalle.contains("no instala")),
            "{:?}",
            r.sonda.hallazgos
        );
        assert!(!fabrica.exec.hubo("./run"));
    }

    #[test]
    fn eval_preludio_de_rust_con_codigo_125_es_infraestructura() {
        // GUARDA: el 125 del preludio sale de podman. El envoltorio ya convierte el del producto en 1.
        let repo = repo_completo();
        let fabrica = FabricaEvalFalsa::con(
            ExecFalso::vacio().responde("config.toml", Resp::Codigo(125, "")),
        );
        let r = sonda_con(&fabrica, repo.path(), &tarea_de("rust", ESCENARIOS));
        assert_eq!(r.sonda.valor, None);
        assert!(
            r.sonda
                .motivo
                .as_deref()
                .unwrap_or_default()
                .contains("preludio de Rust"),
            "{:?}",
            r.sonda.motivo
        );
    }

    // ---- Tarea 91, parte 2c: el envoltorio de cada paso aísla el 125 del producto ----

    /// Imagen de los tests con podman real. Debe estar construida o descargada localmente.
    const ALPINE: &str = "docker.io/library/alpine:3.20";
    /// Imagen con `python` en `/usr/local/bin`. Debe estar descargada localmente (`--pull=never`).
    const PYTHON_SLIM: &str = "docker.io/library/python:3.12-slim";

    #[test]
    fn guarda_podman_envoltorio_el_wget_del_sistema_gana_al_wget_falso_del_producto() {
        crate::saltar_sin_podman!();
        // GUARDA con podman real: el producto deja un `wget` falso (exit 0) en /tmp/producto/bin.
        // La activación lo agrega al final del PATH, detrás del sistema. Alpine sí trae `wget` en el
        // sistema (busybox). Con el PATH del sistema primero, el escenario usa el wget real, que falla
        // al conectar a 127.0.0.1:1. El falso solo tapa herramientas que el sistema NO tiene; no tapa
        // una que el sistema sí tiene.
        let sesion = Sesion::de_imagen(&EXEC_REAL, ALPINE, None).expect("arranca el contenedor");
        let entorno = Entorno {
            dir: None,
            activar: "export PATH=$PATH:/tmp/producto/bin;".into(),
        };
        let instalar = "mkdir -p /tmp/producto/bin && printf '#!/bin/sh\\nexit 0\\n' > /tmp/producto/bin/wget && chmod +x /tmp/producto/bin/wget";
        let paso = sesion
            .paso(&entorno, instalar, PLAZO_CORTO, false)
            .expect("corre el paso del producto");
        assert_eq!(paso.codigo, 0, "{paso:?}");
        let r = sesion
            .correr_escenario(
                &entorno,
                &Escenario {
                    id: "wget".into(),
                    cmd: "wget -q -T 1 http://127.0.0.1:1/x".into(),
                    red: false,
                },
            )
            .expect("corre el escenario");
        assert!(matches!(r, Prueba::Fallo { .. }), "el wget falso pasó");
    }

    #[test]
    fn guarda_podman_envoltorio_el_venv_de_root_gana_al_python_del_sistema() {
        crate::saltar_sin_podman!();
        // GUARDA con podman real: el venv de Root tapa al `python` de /usr/local/bin. El PATH del sistema
        // va antes de la activación, así el venv queda primero.
        let sesion =
            Sesion::de_imagen(&EXEC_REAL, PYTHON_SLIM, None).expect("arranca el contenedor");
        let preparar = sesion
            .paso(
                &Entorno::default(),
                "python -m venv /tmp/venv-eval",
                PLAZO_CORTO,
                false,
            )
            .expect("corre el paso de preparación");
        assert_eq!(preparar.codigo, 0, "{preparar:?}");
        let entorno = Entorno {
            dir: None,
            activar: ACTIVAR_PYTHON.into(),
        };
        let prefijo = sesion
            .paso(
                &entorno,
                "python -c \"import sys; print(sys.prefix)\"",
                PLAZO_CORTO,
                false,
            )
            .expect("corre python -c");
        assert_eq!(prefijo.stdout.trim(), "/tmp/venv-eval", "{prefijo:?}");
        let ruta = sesion
            .paso(&entorno, "command -v python", PLAZO_CORTO, false)
            .expect("corre command -v");
        assert_eq!(ruta.stdout.trim(), "/tmp/venv-eval/bin/python", "{ruta:?}");
    }

    #[test]
    fn eval_copia_que_falla_es_infraestructura_sin_instalar() {
        let repo = repo_completo();
        let fabrica =
            FabricaEvalFalsa::con(ExecFalso::vacio().responde("cp -a", Resp::Codigo(1, "")));
        let r = sonda_con(&fabrica, repo.path(), &tarea_de("shell", ENTREGA));
        assert_eq!(r.sonda.valor, None);
        assert!(
            r.sonda
                .motivo
                .as_deref()
                .unwrap_or_default()
                .contains("instalar nunca corrió")
        );
        assert!(!fabrica.exec.hubo("make"));
    }

    #[test]
    fn eval_arranque_fallido_da_sin_veredicto_con_el_motivo() {
        let repo = repo_completo();
        let fabrica = FabricaEvalFalsa {
            arranque: Some("la imagen no trae sh"),
            ..FabricaEvalFalsa::con(ExecFalso::vacio())
        };
        let r = sonda_con(&fabrica, repo.path(), &tarea_de("shell", ENTREGA));
        assert_eq!(r.sonda.valor, None);
        assert!(
            r.sonda
                .motivo
                .as_deref()
                .unwrap_or_default()
                .contains("el contenedor no arrancó el script de entrega: la imagen no trae sh")
        );
        assert!(sin_hallazgos_de_entrega(&r));
        assert_eq!(fabrica.aperturas.get(), 1);
    }

    #[test]
    fn eval_escenario_con_red_queda_pendiente_sin_correrlo() {
        let repo = repo_completo();
        let fabrica = FabricaEvalFalsa::con(ExecFalso::vacio());
        let t = tarea_de(
            "shell",
            &format!("{ENTREGA}\n[[escenario]]\nid = \"b\"\ncmd = \"exit 1\"\nred = true\n"),
        );
        let r = sonda_con(&fabrica, repo.path(), &t);
        assert!(
            !r.escenarios.contains_key("b"),
            "pendiente no entra al mapa"
        );
        assert_eq!(r.pendientes.len(), 1, "{:?}", r.pendientes);
        assert_eq!(r.pendientes[0].test, "tarea.toml::escenario-b");
        assert!(
            r.pendientes[0].motivo.contains("el escenario pide red"),
            "{:?}",
            r.pendientes
        );
        let corrio = fabrica
            .exec
            .llamadas()
            .iter()
            .any(|args| args.iter().any(|a| a.contains("exit 1")));
        assert!(!corrio, "el escenario con red no debe correr");
    }

    #[test]
    fn eval_escenario_con_error_de_podman_da_sin_veredicto() {
        let repo = repo_completo();
        let fabrica = FabricaEvalFalsa::con(
            ExecFalso::vacio().responde("exit 0", Resp::Error("podman caído")),
        );
        let t = tarea_de("shell", &format!("{ENTREGA}\n{ESCENARIOS}"));
        let r = sonda_con(&fabrica, repo.path(), &t);
        assert_eq!(r.sonda.valor, None);
        assert!(r.sonda.motivo.unwrap_or_default().contains("podman caído"));
    }

    #[test]
    fn entrega_escenario_fallido_deja_requisito_en_parcial() {
        let repo = tempfile::tempdir().expect("temp dir");
        fs::write(repo.path().join("README.md"), "x").expect("escribe");
        fs::write(repo.path().join("Dockerfile"), "FROM x\nUSER app\n").expect("escribe");
        let tarea = tarea_de(
            "shell",
            "[entrega]\ninstalar = \"make\"\ncorrer = \"./run\"\nespera_segundos = 0\n\
             archivos = [\"README.md\", \"Dockerfile\"]\n\n\
             [[escenario]]\nid = \"set-get\"\ncmd = \"false\"\n\n\
             [[requisito]]\nid = \"R1\"\ntitulo = \"Guardar y leer\"\npeso = 3\n\
             pruebas = [\"tests/entrega.rs::test_a\"]\nescenarios = [\"set-get\"]\n",
        );
        let fabrica =
            FabricaEvalFalsa::con(ExecFalso::vacio().responde("false", Resp::Codigo(1, "")));
        let r = sonda_con(&fabrica, repo.path(), &tarea);
        assert_eq!(r.sonda.hallazgos.len(), 1, "solo el escenario falla");

        let mut tests = BTreeMap::new();
        tests.insert(
            "tests/entrega.rs::test_a".to_string(),
            crate::oracle::junit::ResultadoTest::Pasa,
        );
        let resultados = crate::oracle::requisitos::ResultadosTests {
            tests,
            inestables: Default::default(),
            tasas: Default::default(),
        };
        let estados = crate::oracle::requisitos::evaluar_requisitos(
            &tarea.requisito,
            &resultados,
            &r.escenarios,
            &[],
            &[],
            &BTreeMap::new(),
        );
        assert_eq!(estados[0].nivel, Some(2));
        assert_eq!(estados[0].cumplimiento, crate::tipos::Cumplimiento::Parcial);
        assert_eq!(r.sonda.hallazgos[0].dimension, Dimension::Entrega);
        assert_eq!(
            r.sonda.hallazgos[0].fuente,
            crate::tipos::Fuente::Sonda("entrega".into())
        );
    }

    /// Ejecutor sin hadolint: `command -v` falla y la entrega sale bien.
    struct SinHadolint;

    impl EjecutorHerramienta for SinHadolint {
        fn ejecutar(&self, cmd: &str) -> Result<SalidaHerramienta, ErrorOraculo> {
            Ok(SalidaHerramienta {
                codigo: if cmd.starts_with("command -v") { 1 } else { 0 },
                ..Default::default()
            })
        }
    }

    #[test]
    fn entrega_dockerfile_sin_user_da_un_solo_hallazgo_entre_configs_y_entrega() {
        let repo = tempfile::tempdir().expect("temp dir");
        fs::write(repo.path().join("README.md"), "# x\n").expect("escribe README");
        fs::write(
            repo.path().join("Dockerfile"),
            "FROM debian:bookworm-slim\n",
        )
        .expect("escribe Dockerfile");
        let tarea = tarea_de(
            "shell",
            "[entrega]\ninstalar = \"make\"\ncorrer = \"./run\"\nespera_segundos = 0\n\
             archivos = [\"README.md\", \"Dockerfile\"]\n",
        );
        let ej = SinHadolint;
        let configs: Vec<Hallazgo> =
            crate::oracle::configs::sondas_configs(repo.path(), &ej, &tarea)
                .into_iter()
                .flat_map(|r| r.hallazgos)
                .collect();
        let entrega = sonda_con(&sana(), repo.path(), &tarea).sonda.hallazgos;
        let todos: Vec<Hallazgo> = configs.into_iter().chain(entrega).collect();
        // La unión ya trae el hallazgo una sola vez: solo la sonda de entrega emite la regla de USER.
        assert_eq!(todos.len(), 1, "{todos:?}");
        // Red de seguridad: la sonda dueña es la que nombra la tabla de rúbricas.
        let hallazgos = crate::oracle::perfiles::deduplicar_hallazgos(todos);
        assert_eq!(hallazgos.len(), 1, "{hallazgos:?}");
        assert_eq!(hallazgos[0].dimension, Dimension::Entrega);
        assert!(hallazgos[0].detalle.contains("USER"));
    }

    #[test]
    fn entrega_script_de_la_evaluacion_de_memo_son_textos_distintos() {
        let tarea = tarea_de("shell", &format!("{ENTREGA}\n{ESCENARIOS}"));
        assert_ne!(
            construir_script_con(&tarea, true),
            construir_script_con(&tarea, false)
        );
    }
}
