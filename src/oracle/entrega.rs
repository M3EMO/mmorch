//! Entrega y escenarios de usuario (Tarea 9d). Corre el producto en una copia del repo, dentro de
//! un contenedor sin red: instala, arranca y pasa cada `[[escenario]]`. La verificación estática
//! del `Dockerfile` reusa la regla de "corre como root" de 9b. La construcción sin red
//! (`construir_dockerfile`, Tarea 9d2) no se conecta a `evaluar`: Tarea 13 la arma. Si un `RUN`
//! pide red, la construcción se repite una vez con red, sin credenciales ni montajes (Tarea 9d3).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::observe;
use crate::oracle::ErrorOraculo;
use crate::oracle::configs::{self, instrucciones, palabra_clave};
use crate::oracle::contenedor::{PROXY_FUERA, SUFIJO_SELINUX, SalidaPodman, correr_podman};
use crate::oracle::descarga::{self, ImagenBase, PRELUDIO, Problema};
use crate::oracle::perfiles::herramientas::EjecutorHerramienta;
use crate::oracle::perfiles::{ResultadoSonda, crear_hallazgo, puntuar_hallazgos};
use crate::oracle::tarea::{Escenario, Requisito, Tarea};
use crate::oracle::{CARPETA_TEMPORALES, copiar_repo, infra, temporal};
use crate::tipos::{Dimension, Hallazgo, Pendiente};

const SONDA: &str = "entrega";
const DIMENSION: Dimension = Dimension::Entrega;
/// Carpeta con escritura. Todos los `.rc` y `.log` de la entrega van aquí.
const SALIDA: &str = "/salida";
/// Líneas finales del log de `instalar` que entran al resultado.
const LINEAS_LOG: usize = 40;
/// Líneas finales de la salida de cada escenario que entran a `/salida`.
const LINEAS_ESCENARIO: usize = 20;
/// Log completo de `correr`. Vive en `/tmp` del contenedor: nunca llega a `/salida`.
const LOG_CORRER_COMPLETO: &str = "/tmp/entrega-correr.full.log";
/// Límite de cada escenario, en segundos.
const TIMEOUT_ESCENARIO: u32 = 120;
/// Plazo del host para cada escenario dentro de la imagen. El `timeout` interno corta antes.
const PLAZO_ESCENARIO: Duration = Duration::from_secs(TIMEOUT_ESCENARIO as u64);
/// Código que se registra cuando un escenario supera su plazo (el mismo que devuelve `timeout`).
const CODIGO_TIEMPO: i32 = 124;
/// Código con que podman sale ante un error propio (imagen, contenedor o uso).
const CODIGO_PODMAN: i32 = 125;

/// Plazo por defecto de cada `podman build`.
const PLAZO_BUILD: Duration = Duration::from_secs(20 * 60);
/// Plazo de las consultas rápidas de podman: inspeccionar, correr `id` y borrar.
const PLAZO_CORTO: Duration = Duration::from_secs(60);
/// Nombre de la sonda y etiqueta del cuerpo del archivo de pendientes (Tarea 9d2).
const TEST_BUILD: &str = "dockerfile::build";
/// Fragmentos (en minúscula) de la salida de un `RUN` que delatan falta de red. Solo cuentan
/// dentro de un `RUN`. "unable to locate package" no suma: sin "temporary failure" o "could not
/// resolve" ya cuenta por los patrones de abajo.
const PATRONES_RED: [&str; 13] = [
    "could not resolve host",
    "temporary failure in name resolution",
    "network is unreachable",
    "name or service not known",
    "connection refused",
    "connection timed out",
    "failed to connect",
    "could not connect",
    "unable to connect",
    "dns: transient error",
    "temporary error (try again later)",
    "failed to fetch",
    "bad address",
];
/// Fragmentos (en minúscula) de un `FROM` cuya imagen no está local (`--pull=never`).
const PATRONES_AUSENTE: [&str; 3] = ["image not known", "no such image", "unable to find"];
/// Contador de las etiquetas temporales de `podman build`.
static CONTADOR_BUILD: AtomicU64 = AtomicU64::new(0);
/// Detalle del hallazgo de entrega cuando la imagen solo se construye con red (Tarea 9d3).
const DETALLE_NECESITA_RED: &str = "la imagen necesita red para construirse";
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
/// Si un `RUN` pide red, lo repite una vez con red (Tarea 9d3). `imagenes` son las imágenes base
/// que vio la fase de descarga: un problema suyo da sin veredicto.
pub fn construir_dockerfile(
    repo: &Path,
    home: &Path,
    tarea: &Tarea,
    imagenes: &[ImagenBase],
) -> Result<ResultadoDockerfile, ErrorOraculo> {
    construir_con(repo, home, tarea, imagenes, PLAZO_BUILD, &PodmanReal).map(|c| c.resultado)
}

/// Corre `podman` con argumentos. El real llama a `correr_podman`; los tests usan un falso.
pub(crate) trait Podman {
    fn correr(&self, args: &[&str], plazo: Duration) -> Result<SalidaPodman, ErrorOraculo>;
}

/// Podman real: lanza el binario con `correr_podman`.
pub(crate) struct PodmanReal;

impl Podman for PodmanReal {
    fn correr(&self, args: &[&str], plazo: Duration) -> Result<SalidaPodman, ErrorOraculo> {
        correr_podman(args, plazo)
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
}

/// Corre un escenario dentro de su contenedor y dice cómo le fue.
pub(crate) trait CorredorDeEscenario {
    fn correr(&self, escenario: &Escenario) -> Result<Prueba, ErrorOraculo>;
}

/// Fábrica real: arranca el contenedor de escenario con `podman`.
pub(crate) struct FabricaReal;

impl FabricaEscenarios for FabricaReal {
    fn iniciar(
        &self,
        imagen: &str,
        escenarios: Option<&Path>,
    ) -> Result<Box<dyn CorredorDeEscenario + '_>, String> {
        ContenedorEscenario::iniciar(imagen, escenarios)
            .map(|c| Box::new(c) as Box<dyn CorredorDeEscenario + '_>)
    }
}

impl CorredorDeEscenario for ContenedorEscenario {
    fn correr(&self, escenario: &Escenario) -> Result<Prueba, ErrorOraculo> {
        ContenedorEscenario::correr(self, escenario)
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

/// Registra el `Gate` de la repetición con red (Tarea 9d3): la primera `RUN` que falló sin red y todas las
/// `RUN` del Dockerfile. No dice que todas necesitan red. Va antes del build con red. Si no se puede
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
        "primera_run_sin_red".into(),
        json!({"linea": linea, "cita": cita}),
    );
    extra.insert("runs_del_dockerfile".into(), json!(run));
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

/// Aviso de la repetición con red (Tarea 9d3) cuando el host tampoco tiene red.
const AVISO_RED_TAMPOCO: &str = "la construcción con red tampoco tuvo red";
/// Aviso cuando el build con red no llega a construir por un error de podman: queda el Pendiente.
const AVISO_RED_NO_ARRANCO: &str = "la construcción con red no arrancó";
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
    // La imagen temporal se borra al salir de la función, pase lo que pase.
    let imagen = ImagenTemporal {
        etiqueta: etiqueta_unica(),
        podman,
    };
    let df = contexto.join(&ruta).to_string_lossy().into_owned();
    let ctx = contexto.to_string_lossy().into_owned();
    let mut build = correr_build(podman, false, &imagen.etiqueta, &df, &ctx, plazo)?;
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
            _ => clasificar_fallo(&build, &ruta, &texto, reqs)?,
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
                let con_red_salida = correr_build(podman, true, &imagen.etiqueta, &df, &ctx, plazo)
                    .and_then(|salida| {
                        if salida.codigo == 0 {
                            return Ok((salida, None));
                        }
                        // Sin red configurable en el host: es error de podman, no un `RUN` que no construye.
                        if let Some(linea) = red_no_configurada(&salida) {
                            return Err(ErrorOraculo::Infra(linea));
                        }
                        clasificar_fallo(&salida, &ruta, &texto, reqs).map(|d| (salida, Some(d)))
                    });
                let desenlace_red = match con_red_salida {
                    Ok((salida, desenlace)) => {
                        build = salida;
                        desenlace
                    }
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
                if build.codigo == 0 {
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
                    match desenlace_red {
                        Some(Desenlace::PendienteRed { motivo, .. }) => {
                            avisos.push(AVISO_RED_TAMPOCO.into());
                            resultado.pendientes.push(pendiente_de_build(motivo));
                        }
                        Some(otro) => anotar_desenlace(&mut resultado, otro),
                        None => {}
                    }
                }
            }
            otro => anotar_desenlace(&mut resultado, otro),
        }
        if build.codigo != 0 {
            return Ok(Construccion {
                resultado,
                imagen: None,
                avisos,
            });
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

/// Separa un fallo de build en falta de red, imagen base ausente, o producto que no construye.
/// Sin ningún `STEP` en la salida, podman no empezó: es infraestructura.
fn clasificar_fallo(
    build: &SalidaPodman,
    archivo: &str,
    texto: &str,
    reqs: &[Requisito],
) -> Result<Desenlace, ErrorOraculo> {
    let salida = format!("{}\n{}", build.stdout, build.stderr);
    let Some(paso) = paso_fallido(&salida) else {
        let ultima = build.stderr.lines().rev().find(|l| !l.trim().is_empty());
        return Err(ErrorOraculo::Infra(format!(
            "podman no empezó la construcción: {}",
            ultima.unwrap_or("sin salida de error").trim()
        )));
    };
    let instrucciones_df = instrucciones(texto);
    let (linea, cita, kw, red_paso) = match instrucciones_df.get(paso.saturating_sub(1)) {
        Some(i) => (
            i.linea,
            i.cita.clone(),
            palabra_clave(&i.logico).0,
            paso_que_necesita_red(&i.logico),
        ),
        None => match instrucciones_df
            .iter()
            .find(|i| palabra_clave(&i.logico).0 == "RUN")
        {
            Some(i) => (i.linea, i.cita.clone(), "RUN".to_string(), true),
            None => (0, String::new(), String::new(), false),
        },
    };
    let minuscula = salida.to_lowercase();
    if kw == "FROM" {
        if PATRONES_AUSENTE.iter().any(|p| minuscula.contains(p)) {
            return Ok(Desenlace::Pendiente(format!(
                "imagen base no descargada (fase de descarga no corrió): línea {linea}, {cita}"
            )));
        }
        return Err(ErrorOraculo::Infra(format!(
            "podman no pudo arrancar el FROM de la línea {linea}"
        )));
    }
    if red_paso && pide_red(&salida) {
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

/// Número de paso (1-based) del último `STEP k/n` de la salida de `podman build`.
fn paso_fallido(salida: &str) -> Option<usize> {
    salida.lines().rev().find_map(|linea| {
        let resto = linea.split("STEP ").nth(1)?;
        resto.split('/').next()?.trim().parse::<usize>().ok()
    })
}

/// Primera línea de la salida que dice que podman no pudo armar la red del build (`slirp4netns`, `pasta`
/// o `netavark`). Un `RUN` del Dockerfile no produce estas líneas: son de podman.
fn red_no_configurada(build: &SalidaPodman) -> Option<String> {
    format!("{}\n{}", build.stdout, build.stderr)
        .lines()
        .find(|linea| es_linea_red_no_configurada(linea))
        .map(|linea| linea.trim().to_string())
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

/// Verdadero si la salida de un build trae evidencia de falta de red.
fn pide_red(salida: &str) -> bool {
    let minuscula = salida.to_lowercase();
    PATRONES_RED.iter().any(|p| minuscula.contains(p))
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
}

/// Construye los Dockerfile, corre la sonda de entrega y los escenarios. Una sola construcción.
/// Los escenarios corren contra la imagen si construye; si no, en el contenedor de evaluación.
pub fn entregar(
    ej: &dyn EjecutorHerramienta,
    repo: &Path,
    tarea: &Tarea,
    home: &Path,
    imagenes: &[ImagenBase],
) -> Result<ResultadoEntrega, ErrorOraculo> {
    entregar_con(ej, repo, tarea, home, imagenes, &PodmanReal, &FabricaReal)
}

/// Como `entregar`, con el podman y la fábrica de escenarios que se pasen. Producción pasa los reales.
pub(crate) fn entregar_con(
    ej: &dyn EjecutorHerramienta,
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
    let montaje = escenarios.is_dir().then_some(escenarios);
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
    let mut resultado = sonda_entrega_con(ej, repo, tarea, contenedor.is_none());
    if let Some(c) = &contenedor {
        for escenario in &tarea.escenario {
            let id = escenario.id.clone();
            match c.correr(escenario)? {
                Prueba::Paso => {
                    resultado.escenarios.insert(id.clone(), true);
                }
                Prueba::Fallo { codigo, cita } => {
                    resultado.escenarios.insert(id.clone(), false);
                    let detalle = format!("escenario falló (código {codigo})");
                    resultado
                        .sonda
                        .hallazgos
                        .push(hallazgo_de_escenario(escenario, &detalle, &cita, reqs));
                }
                Prueba::Pendiente(motivo) => resultado.pendientes.push(Pendiente {
                    test: format!("tarea.toml::escenario-{id}"),
                    motivo,
                    colab: None,
                }),
            }
            resultado
                .lugar_escenarios
                .insert(id, LugarEscenario::Imagen);
        }
    }
    resultado.pendientes.extend(construccion.pendientes);
    resultado.sonda.hallazgos.extend(construccion.hallazgos);
    resultado.sin_veredicto = construccion.sin_veredicto;
    resultado.avisos = avisos;
    // Sin construcción, la entrega no da verde: el resto de la evaluación sigue con su veredicto.
    if !resultado.sin_veredicto.is_empty() {
        let causas: Vec<&str> = resultado
            .sin_veredicto
            .iter()
            .map(|m| m.causa.as_str())
            .collect();
        let causas = causas.join("; ");
        resultado.sonda.valor = None;
        resultado.sonda.motivo = Some(match resultado.sonda.motivo.take() {
            Some(previo) => format!("{previo}; {causas}"),
            None => causas,
        });
    } else if resultado.sonda.motivo.is_none() {
        resultado.sonda.valor = Some(puntuar_hallazgos(&resultado.sonda.hallazgos, reqs));
    }
    Ok(resultado)
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

/// Contenedor vivo de la imagen construida, donde los escenarios corren con `podman exec`.
/// Su `Drop` lo borra con `podman rm -f -t 0`, pase lo que pase.
struct ContenedorEscenario {
    nombre: String,
    /// Verdadero si la imagen trae `timeout`. Si no, el plazo lo pone Root desde afuera.
    tiene_timeout: bool,
}

/// Contador de los contenedores de escenario de este proceso.
static CONTADOR_ESCENARIO: AtomicU64 = AtomicU64::new(0);

impl ContenedorEscenario {
    /// Arranca la imagen sin red, con `sleep` o, si no está, con `tail -f /dev/null`.
    /// Sin `sh` no sirve para escenarios: devuelve el motivo y la evaluación usa el contenedor de evaluación.
    fn iniciar(imagen: &str, escenarios: Option<&Path>) -> Result<Self, String> {
        let nombre = format!(
            "root-entrega-esc-{}-{}",
            std::process::id(),
            CONTADOR_ESCENARIO.fetch_add(1, Ordering::Relaxed)
        );
        for comando in [vec!["sleep", "600"], vec!["tail", "-f", "/dev/null"]] {
            let args = args_arranque(imagen, escenarios, &nombre, &comando);
            let refs: Vec<&str> = args.iter().map(String::as_str).collect();
            let arranco = correr_podman(&refs, PLAZO_CORTO).is_ok_and(|s| s.codigo == 0);
            if arranco && corriendo(&nombre) {
                let sh = correr_podman(&["exec", nombre.as_str(), "sh", "-c", "true"], PLAZO_CORTO);
                if sh.is_ok_and(|s| s.codigo == 0) {
                    let tiene_timeout = correr_podman(
                        &["exec", nombre.as_str(), "sh", "-c", "command -v timeout"],
                        PLAZO_CORTO,
                    )
                    .is_ok_and(|s| s.codigo == 0);
                    return Ok(ContenedorEscenario {
                        nombre,
                        tiene_timeout,
                    });
                }
                quitar(&nombre);
                return Err("la imagen no trae sh".into());
            }
            quitar(&nombre);
        }
        Err("la imagen no trae sleep ni tail: no arranca el contenedor".into())
    }

    /// Corre el `cmd` del escenario con el límite de `timeout` dentro del contenedor.
    fn correr(&self, escenario: &Escenario) -> Result<Prueba, ErrorOraculo> {
        let plazo_texto = TIMEOUT_ESCENARIO.to_string();
        let mut args = vec!["exec", self.nombre.as_str()];
        if self.tiene_timeout {
            args.extend(["timeout", plazo_texto.as_str()]);
        }
        args.extend(["sh", "-c", escenario.cmd.as_str()]);
        // El plazo de Root corta el `podman exec` a los 120 s, con o sin `timeout` en la imagen.
        let salida = match correr_podman(&args, PLAZO_ESCENARIO) {
            Ok(salida) => salida,
            Err(ErrorOraculo::Infra(motivo)) if motivo.contains("superó") => {
                return Ok(Prueba::Fallo {
                    codigo: CODIGO_TIEMPO,
                    cita: format!("el escenario superó {TIMEOUT_ESCENARIO} s"),
                });
            }
            Err(otro) => return Err(otro),
        };
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
        if pide_red(&texto) {
            let linea = ultima_linea_de(&texto).unwrap_or_default();
            return Ok(Prueba::Pendiente(format!(
                "necesita red: el contenedor de escenario corre con --network none: {linea}"
            )));
        }
        let cita = ultima_linea_de(&texto).unwrap_or_else(|| escenario.id.clone());
        Ok(Prueba::Fallo {
            codigo: salida.codigo,
            cita,
        })
    }
}

impl Drop for ContenedorEscenario {
    fn drop(&mut self) {
        quitar(&self.nombre);
    }
}

/// Borra un contenedor por nombre, sin esperar la parada. Un fallo no importa: ya hay un resultado.
fn quitar(nombre: &str) {
    let _ = correr_podman(&["rm", "-f", "-t", "0", nombre], PLAZO_CORTO);
}

/// Un escenario usa `/oraculo` si su `cmd` nombra esa ruta: sus scripts viven en `escenarios/`.
pub fn escenario_usa_oraculo(cmd: &str) -> bool {
    cmd.contains("/oraculo")
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
fn corriendo(nombre: &str) -> bool {
    correr_podman(
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

/// Script de `sh` que corre la entrega y los escenarios. Escribe `.rc` y `.log` en `/salida`.
/// Sin `[entrega]`, solo corre los escenarios, sobre `/repo`.
pub fn construir_script(tarea: &Tarea) -> String {
    construir_script_con(tarea, true)
}

/// Script del contenedor de evaluación. Con `escenarios` en falso no corre ninguno: van a la imagen.
fn construir_script_con(tarea: &Tarea, escenarios: bool) -> String {
    let eval: &[Escenario] = if escenarios { &tarea.escenario } else { &[] };
    let mut s = String::new();
    if tarea.lenguaje == "rust" {
        // El preludio termina en `&& `: su `true` aísla un fallo de caché del resto del script.
        s.push_str(PRELUDIO);
        s.push_str("true\n");
    }
    let work = format!("{SALIDA}/work");
    if let Some(entrega) = &tarea.entrega {
        s.push_str(&format!(
            "cp -a /repo {work} || exit 1\ncd {work} || exit 1\n"
        ));
        s.push_str(&format!(
            "{{ ( {} ) 2>&1; echo $? > {SALIDA}/entrega-instalar.rc; }} \
             | tail -n {LINEAS_LOG} > {SALIDA}/entrega-instalar.log\n",
            entrega.instalar
        ));
        s.push_str(&format!(
            "if [ \"$(cat {SALIDA}/entrega-instalar.rc)\" != 0 ]; then exit 0; fi\n"
        ));
        // `correr` va dentro de un grupo de procesos propio, para matarlo entero al final.
        // El archivo `.fin` marca que `correr` terminó: sin él, sigue vivo.
        let fin = format!("{SALIDA}/entrega-correr.fin");
        let interno = format!("( {} ); echo $? > {fin}", entrega.correr);
        s.push_str(&format!(
            "if command -v setsid >/dev/null 2>&1; then GRUPO=setsid; else GRUPO=; fi\n\
             $GRUPO sh -c {} > {LOG_CORRER_COMPLETO} 2>&1 &\n\
             PID=$!\n\
             sleep {}\n\
             if [ -f {fin} ]; then echo 1 > {SALIDA}/entrega-correr.rc; \
             else echo 0 > {SALIDA}/entrega-correr.rc; fi\n",
            comillas_simples(&interno),
            entrega.espera_segundos
        ));
    } else {
        s.push_str("cd /repo || exit 1\n");
    }
    for escenario in eval {
        s.push_str(&format!(
            "{{ timeout {TIMEOUT_ESCENARIO} sh -c {}; echo $? > {SALIDA}/entrega-escenario-{}.rc; }} \
             2>&1 | tail -n {LINEAS_ESCENARIO} > {SALIDA}/entrega-escenario-{}.log\n",
            comillas_simples(&escenario.cmd),
            escenario.id,
            escenario.id
        ));
    }
    if tarea.entrega.is_some() {
        s.push_str(
            "if [ -n \"$PID\" ]; then kill -KILL -- -\"$PID\" 2>/dev/null; \
             kill -KILL \"$PID\" 2>/dev/null; fi\n",
        );
        // Solo la cola del log de `correr` llega a `/salida`.
        s.push_str(&format!(
            "tail -n {LINEAS_LOG} {LOG_CORRER_COMPLETO} > {SALIDA}/entrega-correr.log\n"
        ));
    }
    s
}

/// Envuelve `texto` en comillas simples de `sh`. Las comillas simples dentro salen escapadas.
fn comillas_simples(texto: &str) -> String {
    format!("'{}'", texto.replace('\'', "'\\''"))
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
    }
}

/// Sonda de entrega con el contenedor de evaluación para todo. Un `Err` del ejecutor es infraestructura.
pub fn sonda_entrega(ej: &dyn EjecutorHerramienta, repo: &Path, tarea: &Tarea) -> ResultadoEntrega {
    sonda_entrega_con(ej, repo, tarea, true)
}

/// Sonda de entrega. `escenarios_eval` en falso deja los escenarios para la imagen construida.
fn sonda_entrega_con(
    ej: &dyn EjecutorHerramienta,
    repo: &Path,
    tarea: &Tarea,
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
    if tarea.entrega.is_some() || !eval.is_empty() {
        match ej.ejecutar(&construir_script_con(tarea, escenarios_eval)) {
            Ok(salida) => revisar_salida(
                &salida.archivos,
                tarea,
                eval,
                &mut hallazgos,
                &mut escenarios,
            ),
            Err(e) => motivos.push(e.to_string()),
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
        pendientes: Vec::new(),
        sin_veredicto: Vec::new(),
        avisos: Vec::new(),
    }
}

/// Convierte los `.rc` y `.log` de la salida en hallazgos y en el estado de cada escenario.
fn revisar_salida(
    archivos: &BTreeMap<String, String>,
    tarea: &Tarea,
    corridos: &[Escenario],
    hallazgos: &mut Vec<Hallazgo>,
    escenarios: &mut BTreeMap<String, bool>,
) {
    let reqs = tarea.requisito.as_slice();
    let instalo = match tarea.entrega {
        Some(_) => {
            let ok = codigo(archivos, "entrega-instalar.rc") == Some(0);
            if !ok {
                let cita = ultima_linea(archivos, "entrega-instalar.log")
                    .unwrap_or_else(|| "(sin salida)".to_string());
                let detalle = "no instala: el comando de instalación falla";
                hallazgos.push(crear_hallazgo(
                    DIMENSION, 3, "", 0, &cita, detalle, SONDA, reqs,
                ));
            }
            ok
        }
        None => true,
    };
    if tarea.entrega.is_some() && instalo && codigo(archivos, "entrega-correr.rc") == Some(1) {
        let cita = ultima_linea(archivos, "entrega-correr.log")
            .unwrap_or_else(|| "(sin salida)".to_string());
        let detalle = "no arranca: el comando de arranque muere antes de tiempo";
        hallazgos.push(crear_hallazgo(
            DIMENSION, 3, "", 0, &cita, detalle, SONDA, reqs,
        ));
    }
    for escenario in corridos {
        let rc = codigo(archivos, &format!("entrega-escenario-{}.rc", escenario.id));
        let ok = rc == Some(0);
        escenarios.insert(escenario.id.clone(), ok);
        if ok {
            continue;
        }
        let detalle = match rc {
            _ if !instalo => "no corrió: el producto no instala".to_string(),
            Some(c) => format!("escenario falló (código {c})"),
            None => "no corrió: el escenario no llegó a su código de salida".to_string(),
        };
        // La cita es la última línea de su salida. Sin salida, el id del escenario.
        let cita = ultima_linea(archivos, &format!("entrega-escenario-{}.log", escenario.id))
            .unwrap_or_else(|| escenario.id.clone());
        hallazgos.push(hallazgo_de_escenario(escenario, &detalle, &cita, reqs));
    }
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

/// Código de salida de un `.rc`. `None` si el archivo falta o no es un número.
fn codigo(archivos: &BTreeMap<String, String>, nombre: &str) -> Option<i32> {
    archivos.get(nombre).and_then(|t| t.trim().parse().ok())
}

/// Última línea no vacía de un `.log`. `None` si el archivo falta o está vacío.
fn ultima_linea(archivos: &BTreeMap<String, String>, nombre: &str) -> Option<String> {
    archivos
        .get(nombre)
        .and_then(|t| t.lines().map(str::trim).rev().find(|l| !l.is_empty()))
        .map(str::to_string)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::oracle::ErrorOraculo;
    use crate::oracle::perfiles::herramientas::SalidaHerramienta;
    use std::cell::{Cell, RefCell};
    use std::fs;
    use std::rc::Rc;

    /// Ejecutor de mentira: responde siempre lo mismo y cuenta las llamadas.
    struct Falso {
        salida: Result<SalidaHerramienta, String>,
        llamadas: Cell<u32>,
        cmds: RefCell<Vec<String>>,
    }

    impl Falso {
        fn con(archivos: &[(&str, &str)]) -> Self {
            let mut s = SalidaHerramienta::default();
            for (nombre, texto) in archivos {
                s.archivos.insert((*nombre).into(), (*texto).into());
            }
            Falso {
                salida: Ok(s),
                llamadas: Cell::new(0),
                cmds: RefCell::new(Vec::new()),
            }
        }

        /// Ejecutor sano: instalar y correr salen 0.
        fn sano() -> Self {
            Falso::con(&[("entrega-instalar.rc", "0\n"), ("entrega-correr.rc", "0\n")])
        }

        fn infra(msg: &str) -> Self {
            Falso {
                salida: Err(msg.into()),
                llamadas: Cell::new(0),
                cmds: RefCell::new(Vec::new()),
            }
        }
    }

    impl EjecutorHerramienta for Falso {
        fn ejecutar(&self, cmd: &str) -> Result<SalidaHerramienta, ErrorOraculo> {
            self.llamadas.set(self.llamadas.get() + 1);
            self.cmds.borrow_mut().push(cmd.to_string());
            match &self.salida {
                Ok(s) => Ok(s.clone()),
                Err(m) => Err(ErrorOraculo::Infra(m.clone())),
            }
        }
    }

    const ENTREGA: &str = "[entrega]\ninstalar = \"make\"\ncorrer = \"./run\"\n\
        archivos = [\"README.md\", \"Dockerfile\"]\n";

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

    #[test]
    fn entrega_script_incluye_preludio_solo_para_rust() {
        let rust = tarea_de("rust", ENTREGA);
        assert!(construir_script(&rust).contains(PRELUDIO));
        let shell = tarea_de("shell", ENTREGA);
        assert!(!construir_script(&shell).contains("CARGO_HOME"));
    }

    #[test]
    fn entrega_script_tiene_un_timeout_por_escenario() {
        let t = tarea_de("shell", &format!("{ENTREGA}\n{ESCENARIOS}"));
        let s = construir_script(&t);
        assert_eq!(s.matches("timeout 120").count(), 2);
    }

    #[test]
    fn entrega_script_sin_entrega_no_copia_ni_instala() {
        let t = tarea_de("shell", &format!("\n{ESCENARIOS}"));
        let s = construir_script(&t);
        assert!(!s.contains("cp -a"), "sin entrega no hay copia");
        assert!(!s.contains("entrega-instalar"), "sin entrega no instala");
        assert!(!s.contains("setsid"), "sin entrega no arranca");
        assert!(s.contains("cd /repo"), "los escenarios corren sobre /repo");
    }

    #[test]
    fn entrega_script_con_entrega_copia_instala_y_arranca_con_espera_5() {
        let s = construir_script(&tarea_de("shell", ENTREGA));
        assert!(s.contains("cp -a /repo /salida/work"));
        assert!(s.contains("/salida/entrega-instalar.rc"));
        assert!(s.contains("sleep 5"), "espera por defecto de 5 s");
        assert!(s.contains("/salida/entrega-correr.rc"));
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
        let r = sonda_entrega(&Falso::sano(), repo.path(), &tarea_de("shell", ENTREGA));
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
        let r = sonda_entrega(&Falso::sano(), repo.path(), &tarea_de("shell", ENTREGA));
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
        let r = sonda_entrega(&Falso::sano(), repo.path(), &tarea_de("shell", ENTREGA));
        assert_eq!(r.sonda.hallazgos.len(), 1);
        let h = &r.sonda.hallazgos[0];
        assert_eq!(h.dimension, Dimension::Entrega);
        assert_eq!(h.severidad, 3);
        assert_eq!(h.archivo, "Dockerfile");
        assert_eq!(h.fuente, crate::tipos::Fuente::Sonda("entrega".into()));
    }

    #[test]
    fn entrega_sin_entrega_ni_escenarios_no_corre_nada() {
        let repo = repo_completo();
        let ej = Falso::sano();
        let r = sonda_entrega(&ej, repo.path(), &tarea_de("shell", ""));
        assert_eq!(ej.llamadas.get(), 0);
        assert!(r.escenarios.is_empty());
        assert_eq!(r.sonda.valor, Some(1.0));
    }

    #[test]
    fn entrega_escenario_con_codigo_1_queda_false_y_con_hallazgo_de_su_requisito() {
        let repo = repo_completo();
        let ej = Falso::con(&[
            ("entrega-instalar.rc", "0\n"),
            ("entrega-correr.rc", "0\n"),
            ("entrega-escenario-a.rc", "0\n"),
            ("entrega-escenario-b.rc", "1\n"),
        ]);
        let t = tarea_de("shell", &format!("{ENTREGA}\n{ESCENARIOS}\n{REQUISITOS}"));
        let r = sonda_entrega(&ej, repo.path(), &t);
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
    fn entrega_escenario_sin_archivo_rc_queda_false() {
        let repo = repo_completo();
        let ej = Falso::con(&[("entrega-instalar.rc", "0\n"), ("entrega-correr.rc", "0\n")]);
        let t = tarea_de("shell", &format!("{ENTREGA}\n{ESCENARIOS}\n{REQUISITOS}"));
        let r = sonda_entrega(&ej, repo.path(), &t);
        assert_eq!(r.escenarios.get("a"), Some(&false));
        assert_eq!(r.escenarios.get("b"), Some(&false));
    }

    #[test]
    fn entrega_instalar_fallido_da_no_instala_y_escenarios_en_false() {
        let repo = repo_completo();
        let ej = Falso::con(&[
            ("entrega-instalar.rc", "2\n"),
            ("entrega-instalar.log", "compilando\nerror final\n"),
        ]);
        let t = tarea_de("shell", &format!("{ENTREGA}\n{ESCENARIOS}\n{REQUISITOS}"));
        let r = sonda_entrega(&ej, repo.path(), &t);
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
        let ej = Falso::con(&[
            ("entrega-instalar.rc", "0\n"),
            ("entrega-correr.rc", "1\n"),
            ("entrega-correr.log", "arrancando\npuerto ocupado\n"),
        ]);
        let r = sonda_entrega(&ej, repo.path(), &tarea_de("shell", ENTREGA));
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
        let t = tarea_de("shell", &format!("{ENTREGA}\n{ESCENARIOS}"));
        let r = sonda_entrega(&Falso::infra("podman caído"), repo.path(), &t);
        assert_eq!(r.sonda.valor, None);
        assert!(r.sonda.motivo.unwrap_or_default().contains("podman caído"));
        assert!(r.escenarios.is_empty(), "no inventa falsos");
    }

    #[test]
    fn entrega_valor_es_puntuar_de_los_hallazgos() {
        let repo = tempfile::tempdir().expect("temp dir");
        let r = sonda_entrega(&Falso::sano(), repo.path(), &tarea_de("shell", ENTREGA));
        let esperado = puntuar_hallazgos(&r.sonda.hallazgos, &[]);
        assert_eq!(r.sonda.valor, Some(esperado));
    }

    #[test]
    fn entrega_script_correr_escribe_el_log_completo_en_tmp_y_deja_la_cola() {
        let s = construir_script(&tarea_de("shell", ENTREGA));
        assert!(s.contains("> /tmp/entrega-correr.full.log 2>&1 &"));
        let cola = "tail -n 40 /tmp/entrega-correr.full.log > /salida/entrega-correr.log";
        let pos_cola = s.find(cola).expect("la cola de correr va a /salida");
        let pos_kill = s.find("kill -KILL").expect("mata correr");
        assert!(
            pos_kill < pos_cola,
            "la cola se escribe después de matar correr"
        );
        assert!(!s.contains("> /salida/entrega-correr.log 2>&1"));
    }

    #[test]
    fn entrega_script_instalar_y_escenarios_guardan_solo_la_cola() {
        let t = tarea_de("shell", &format!("{ENTREGA}\n{ESCENARIOS}"));
        let s = construir_script(&t);
        assert!(s.contains("| tail -n 40 > /salida/entrega-instalar.log"));
        assert!(s.contains("| tail -n 20 > /salida/entrega-escenario-a.log"));
        assert!(s.contains("| tail -n 20 > /salida/entrega-escenario-b.log"));
    }

    #[test]
    fn entrega_escenario_cita_sale_de_la_ultima_linea_de_su_log() {
        let repo = repo_completo();
        let ej = Falso::con(&[
            ("entrega-instalar.rc", "0\n"),
            ("entrega-correr.rc", "0\n"),
            ("entrega-escenario-a.rc", "0\n"),
            ("entrega-escenario-b.rc", "1\n"),
            ("entrega-escenario-b.log", "paso 1\nel login falló\n\n"),
        ]);
        let t = tarea_de("shell", &format!("{ENTREGA}\n{ESCENARIOS}\n{REQUISITOS}"));
        let r = sonda_entrega(&ej, repo.path(), &t);
        assert_eq!(r.sonda.hallazgos.len(), 1);
        let h = &r.sonda.hallazgos[0];
        assert_eq!(h.cita, "el login falló");
        assert_eq!(h.impacto, ["R1"]);
    }

    #[test]
    fn entrega_ultimo_paso_sale_del_ultimo_step_de_la_salida() {
        let salida = "STEP 1/3: FROM x\nSTEP 2/3: RUN a\nSTEP 3/3: RUN b\n\
            Error: building at STEP \"RUN b\": while running runtime: exit status 1";
        assert_eq!(paso_fallido(salida), Some(3));
        assert_eq!(
            paso_fallido("Error: creating build container: x: image not known"),
            None
        );
    }

    #[test]
    fn entrega_pide_red_reconoce_los_errores_de_apk_y_wget() {
        assert!(pide_red(
            "WARNING: fetching x: temporary error (try again later)"
        ));
        assert!(pide_red("wget: bad address 'example.com'"));
        assert!(pide_red("curl: (6) Could not resolve host: example.com"));
        assert!(!pide_red(
            "Error: building at STEP \"RUN false\": exit status 1"
        ));
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

    /// Salida de `podman build` sin red: el `RUN` de la línea 2 falla por el DNS.
    pub(crate) fn salida_sin_red() -> SalidaPodman {
        SalidaPodman {
            codigo: 1,
            stdout: "STEP 1/2: FROM debian:12\nSTEP 2/2: RUN apt-get install -y curl\n".into(),
            stderr: "Temporary failure in name resolution\n".into(),
        }
    }

    /// Salida de un comando que sale 0.
    pub(crate) fn salida_ok() -> SalidaPodman {
        SalidaPodman {
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

    /// Respuesta de mentira: el `build` sin red da `sin_red` y con red da `con_red`. El resto sale
    /// sano: `id -u` da 1000 (no es root) e `image inspect` da `app`.
    pub(crate) fn responde_build(
        sin_red: SalidaPodman,
        con_red: SalidaPodman,
    ) -> impl Fn(&[&str]) -> SalidaPodman {
        move |args| match args[0] {
            "build" if es_build_con_red(args) => con_red.clone(),
            "build" => sin_red.clone(),
            "run" if args.contains(&"id") => SalidaPodman {
                codigo: 0,
                stdout: "1000\n".into(),
                stderr: String::new(),
            },
            "image" => SalidaPodman {
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
        assert_eq!(g["extra"]["primera_run_sin_red"]["linea"], 2);
        assert_eq!(
            g["extra"]["primera_run_sin_red"]["cita"],
            "RUN apt-get install -y curl"
        );
        let run = g["extra"]["runs_del_dockerfile"]
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
            codigo: 1,
            stdout: "STEP 1/3: FROM debian:12\nSTEP 2/3: RUN echo listo\nSTEP 3/3: RUN apt-get install -y curl\n"
                .into(),
            stderr: "Temporary failure in name resolution\n".into(),
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
        assert_eq!(g["primera_run_sin_red"]["linea"], 3, "{g}");
        assert_eq!(
            g["primera_run_sin_red"]["cita"],
            "RUN apt-get install -y curl"
        );
        let runs = g["runs_del_dockerfile"].as_array().expect("lista de RUN");
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
            codigo: 1,
            stdout: "STEP 1/2: FROM debian:12\nSTEP 2/2: RUN apt-get install -y curl\n".into(),
            stderr: "E: Unable to locate package curl\n".into(),
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

    #[test]
    fn entrega_red_sin_red_en_el_host_queda_pendiente_con_aviso() {
        let repo = repo_con_archivos(&[("Dockerfile", DF_RED)]);
        let home = tempfile::tempdir().expect("home");
        let podman = PodmanFalso::nuevo(responde_build(salida_sin_red(), salida_sin_red()));
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
        assert!(
            r.pendientes[0].motivo.contains("línea 2"),
            "{:?}",
            r.pendientes
        );
        assert!(
            c.avisos.iter().any(|a| a == AVISO_RED_TAMPOCO),
            "{:?}",
            c.avisos
        );
        assert_eq!(podman.builds().len(), 2, "{:?}", podman.builds());
        assert_eq!(gates_de_repeticion(home.path()).len(), 1);
    }

    /// Salida de `podman build` con red cuando podman no puede armar la red del `RUN` (sin `slirp4netns`).
    fn salida_red_no_configurada() -> SalidaPodman {
        SalidaPodman {
            codigo: 1,
            stdout: "STEP 1/2: FROM debian:12\nSTEP 2/2: RUN apt-get install -y curl\n".into(),
            stderr: "Error: building at STEP \"RUN apt-get install -y curl\": setup network: could not find slirp4netns, the network namespace can't be configured: exec: \"slirp4netns\": executable file not found in $PATH\n".into(),
        }
    }

    #[test]
    fn entrega_red_setup_network_con_red_queda_pendiente_con_aviso() {
        let repo = repo_con_archivos(&[("Dockerfile", DF_RED)]);
        let home = tempfile::tempdir().expect("home");
        let podman = PodmanFalso::nuevo(responde_build(
            salida_sin_red(),
            salida_red_no_configurada(),
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
        assert!(
            r.pendientes[0].motivo.contains("línea 2"),
            "{:?}",
            r.pendientes
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
            codigo: 125,
            stdout: "STEP 1/2: FROM debian:12\nSTEP 2/2: RUN apt-get install -y curl\n".into(),
            stderr: "Error: creating network: pasta: executable file not found in $PATH\n".into(),
        };
        let podman = PodmanFalso::nuevo(responde_build(salida_sin_red(), pasta_falla));
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
        assert!(
            r.pendientes[0].motivo.contains("línea 2"),
            "{:?}",
            r.pendientes
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
            codigo: 1,
            stdout: "STEP 1/2: FROM debian:12\nSTEP 2/2: RUN apt-get install -y curl\n".into(),
            stderr: "ERROR: unable to select packages:\n  curl (no such package)\n".into(),
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
        let runs = gates[0]["extra"]["runs_del_dockerfile"]
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
            codigo: 1,
            stdout: SALIDA_ADD.into(),
            stderr: "dial tcp: lookup ejemplo.com: Temporary failure in name resolution\n".into(),
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
        let ej = Falso::sano();
        let podman = PodmanFalso::nuevo(responde_build(salida_ok(), salida_ok()));
        let fabrica = FabricaFalsa {
            imagenes: RefCell::new(Vec::new()),
        };
        let r = entregar_con(&ej, repo.path(), &t, home.path(), &[], &podman, &fabrica)
            .expect("entrega");
        assert_eq!(r.escenarios.get("a"), Some(&true), "{:?}", r.escenarios);
        assert_eq!(r.lugar_escenarios.get("a"), Some(&LugarEscenario::Imagen));
        assert_eq!(
            fabrica.imagenes.borrow().len(),
            1,
            "un contenedor de escenario"
        );
        assert_eq!(
            ej.llamadas.get(),
            0,
            "el escenario corre una sola vez, en la imagen"
        );
        assert!(r.sin_veredicto.is_empty(), "{:?}", r.sin_veredicto);
    }
}
