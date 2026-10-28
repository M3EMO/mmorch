//! Entrega y escenarios de usuario (Tarea 9d). Corre el producto en una copia del repo, dentro de
//! un contenedor sin red: instala, arranca y pasa cada `[[escenario]]`. La verificación estática
//! del `Dockerfile` reusa la regla de "corre como root" de 9b. La construcción sin red
//! (`construir_dockerfile`, Tarea 9d2) no se conecta a `evaluar`: Tarea 13 la arma.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::oracle::ErrorOraculo;
use crate::oracle::configs::{self, instrucciones, palabra_clave};
use crate::oracle::contenedor::{SUFIJO_SELINUX, SalidaPodman, correr_podman};
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
/// `imagenes` son las imágenes base que vio la fase de descarga: un problema suyo da sin veredicto.
pub fn construir_dockerfile(
    repo: &Path,
    home: &Path,
    tarea: &Tarea,
    imagenes: &[ImagenBase],
) -> Result<ResultadoDockerfile, ErrorOraculo> {
    construir_con(repo, home, tarea, imagenes, PLAZO_BUILD).map(|c| c.resultado)
}

/// Lo que sale de construir. Si el Dockerfile construye y la imagen trae `sh`, `imagen` guarda la imagen:
/// los escenarios corren contra ella. Su `Drop` la borra con `podman rmi -f`.
pub struct Construccion {
    pub resultado: ResultadoDockerfile,
    imagen: Option<ImagenTemporal>,
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

/// Construye el Dockerfile elegido y verifica que corre sin root. Un solo `podman build`.
fn construir_con(
    repo: &Path,
    home: &Path,
    tarea: &Tarea,
    imagenes: &[ImagenBase],
    plazo: Duration,
) -> Result<Construccion, ErrorOraculo> {
    let reqs = tarea.requisito.as_slice();
    let mut resultado = ResultadoDockerfile {
        hallazgos: Vec::new(),
        pendientes: Vec::new(),
        corre_sin_root: None,
        construida: false,
        sin_veredicto: Vec::new(),
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
    let imagen = ImagenTemporal(etiqueta_unica());
    let df = contexto.join(&ruta).to_string_lossy().into_owned();
    let ctx = contexto.to_string_lossy().into_owned();
    let args = [
        "build",
        "--network",
        "none",
        "--pull=never",
        "-t",
        imagen.0.as_str(),
        "-f",
        df.as_str(),
        ctx.as_str(),
    ];
    let build = correr_podman(&args, plazo)?;
    if build.codigo != 0 {
        match clasificar_fallo(&build, &ruta, &texto, reqs)? {
            Desenlace::Pendiente(motivo) => resultado.pendientes.push(Pendiente {
                test: TEST_BUILD.into(),
                motivo,
                colab: None,
            }),
            Desenlace::Hallazgo(h) => resultado.hallazgos.push(h),
        }
        return Ok(Construccion {
            resultado,
            imagen: None,
            avisos,
        });
    }
    let con_sh = tiene_sh(&imagen.0)?;
    let sin_root = corre_sin_root(&imagen.0, con_sh)?;
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
struct ImagenTemporal(String);

impl Drop for ImagenTemporal {
    fn drop(&mut self) {
        // Solo borra su propia etiqueta. Un fallo no importa: la imagen quizá nunca se creó.
        let _ = correr_podman(&["rmi", "-f", self.0.as_str()], PLAZO_CORTO);
    }
}

/// Etiqueta única del proceso: `root-entrega-<pid>-<n>`.
fn etiqueta_unica() -> String {
    let n = CONTADOR_BUILD.fetch_add(1, Ordering::Relaxed);
    format!("root-entrega-{}-{n}", std::process::id())
}

/// Verdadero si la imagen corre sin root. Con `sh`, `id -u` lo confirma. Sin `sh`, vale solo el `USER`.
fn corre_sin_root(tag: &str, con_sh: bool) -> Result<bool, ErrorOraculo> {
    let inspeccion = correr_podman(
        &["image", "inspect", "--format", "{{.Config.User}}", tag],
        PLAZO_CORTO,
    )?;
    let usuario = inspeccion.stdout.trim().to_string();
    if !con_sh {
        return Ok(!es_root(&usuario));
    }
    let uid = correr_podman(
        &[
            "run",
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
fn tiene_sh(tag: &str) -> Result<bool, ErrorOraculo> {
    let prueba = correr_podman(
        &[
            "run",
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
    Pendiente(String),
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
    let (linea, cita, kw) = match instrucciones_df.get(paso.saturating_sub(1)) {
        Some(i) => (i.linea, i.cita.clone(), palabra_clave(&i.logico).0),
        None => match instrucciones_df
            .iter()
            .find(|i| palabra_clave(&i.logico).0 == "RUN")
        {
            Some(i) => (i.linea, i.cita.clone(), "RUN".to_string()),
            None => (0, String::new(), String::new()),
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
    if kw == "RUN" && pide_red(&salida) {
        return Ok(Desenlace::Pendiente(format!(
            "no se pudo construir sin red: línea {linea}, {cita}"
        )));
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

/// Rutas relativas (con `/`) de los Dockerfile del repo, en orden alfabético. Suma `*.Dockerfile` y
/// `*.Containerfile` a los nombres de `configs`, que no cambian: solo sirven para elegir qué construir.
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
        } else if tipo.is_file() && es_nombre_dockerfile(&nombre) {
            let rel = ruta.strip_prefix(repo).unwrap_or(&ruta);
            salida.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
}

fn es_nombre_dockerfile(nombre: &str) -> bool {
    nombre == "Dockerfile"
        || nombre == "Containerfile"
        || nombre.starts_with("Dockerfile.")
        || nombre.ends_with(".Dockerfile")
        || nombre.ends_with(".Containerfile")
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
    let reqs = tarea.requisito.as_slice();
    // Los escenarios son scripts de la carpeta de la tarea: sin ella no hay nada que montar.
    if !tarea.escenario.is_empty() && !tarea.dir.is_dir() {
        return Err(ErrorOraculo::Infra(format!(
            "no existe la carpeta de la tarea {}",
            tarea.dir.display()
        )));
    }
    let Construccion {
        resultado: construccion,
        imagen,
        avisos: avisos_construccion,
    } = construir_con(repo, home, tarea, imagenes, PLAZO_BUILD)?;
    let mut avisos = avisos_construccion;
    // El contenedor de escenario sale de la imagen construida. Sin él, van al de evaluación.
    let contenedor = match &imagen {
        Some(img) if !tarea.escenario.is_empty() => {
            match ContenedorEscenario::iniciar(&img.0, &tarea.dir) {
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
enum Prueba {
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
    fn iniciar(imagen: &str, oraculo: &Path) -> Result<Self, String> {
        // Los escenarios son scripts de la tarea: se montan en solo lectura, sin copiarlos a la imagen.
        let montaje = format!("{}:/oraculo:ro,{SUFIJO_SELINUX}", oraculo.display());
        let nombre = format!(
            "root-entrega-esc-{}-{}",
            std::process::id(),
            CONTADOR_ESCENARIO.fetch_add(1, Ordering::Relaxed)
        );
        for comando in [vec!["sleep", "600"], vec!["tail", "-f", "/dev/null"]] {
            let mut args = vec![
                "run",
                "-d",
                "--network",
                "none",
                "--pull=never",
                "--name",
                nombre.as_str(),
                "-v",
                montaje.as_str(),
                "--entrypoint",
                comando[0],
                imagen,
            ];
            args.extend_from_slice(&comando[1..]);
            let arranco = correr_podman(&args, PLAZO_CORTO).is_ok_and(|s| s.codigo == 0);
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
mod tests {
    use super::*;
    use crate::oracle::ErrorOraculo;
    use crate::oracle::perfiles::herramientas::SalidaHerramienta;
    use std::cell::{Cell, RefCell};
    use std::fs;

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
}
