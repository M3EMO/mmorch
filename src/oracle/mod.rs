//! Oracle: corre los tests en un contenedor sin red y cuenta el resultado desde junit.

pub mod calibra;
pub mod colab;
pub mod configs;
pub mod contenedor;
pub mod descarga;
pub mod entrega;
pub mod evidencia;
pub mod informe;
pub mod juez;
pub mod junit;
pub mod mapa;
pub mod mutacion;
pub mod pendientes;
pub mod perfiles;
pub mod requisitos;
pub mod rubrica;
pub mod tarea;
pub mod validez;

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

use thiserror::Error as ErrorDerive;

use crate::borrado;
use crate::config::{Config, ReglasDecision, root_home};
use crate::ficha::{self, ErrorFicha};
use crate::proveedores::{Prompt, RespuestaJuez};
use crate::tipos::{
    Conteo, Dimension, Estado, EstadoRequisito, Hallazgo, Pendiente, Puntaje, Rol, TasaTest,
    VERSION, Veredicto,
};
use crate::track::stats::wilson_inferior;
use calibra::Calibraciones;
use contenedor::{Ejecucion, Resultado};
use entrega::{FabricaEscenarios, FabricaReal, Podman, PodmanReal, ResultadoEntrega};
use informe::{Redactor, RedactorMedido, RedactorProveedor};
use juez::{EntradaJuez, EstadoJuez, JuezProveedor, ModeloJuez, ResultadoJuez};
use junit::{Junit, ResultadoTest};
use perfiles::herramientas::{EjecutorContenedor, EjecutorHerramienta};
use perfiles::{ResultadoSonda, combinar, deduplicar_hallazgos};
use rubrica::{Mide, Rubrica};
use tarea::{Comando, Tarea};

/// Carpetas de la raíz del repo que no se copian: artefactos del builder e historial de git.
const EXCLUIDOS: &[&str] = &["target", ".git"];
/// Nombre del junit del contrato dentro de `/salida`.
const XML_CONTRATO: &str = "contrato.xml";
/// Nombre del junit del oculto dentro de `/salida`.
const XML_OCULTO: &str = "oculto.xml";
/// Carpeta de temporales dentro de `ROOT_HOME`.
const CARPETA_TEMPORALES: &str = "tmp";
/// Prefijo de las carpetas temporales de `evaluar`.
const PREFIJO_TEMPORAL: &str = "evaluar-";
/// Prefijo de las carpetas de Colab dentro de `tmp`: el junit bajado de cada corrida (B18).
const PREFIJO_COLAB_TEMPORAL: &str = "colab-";
/// Lista de tests que pytest recolecta, dentro de `/salida`. Se compara con el junit (A5).
const ARCHIVO_COLECCION: &str = "coleccion.txt";
/// Carpeta de los registros de stderr de cada tarea, dentro de `oraculos/<tarea>/` (A2).
const CARPETA_REGISTROS: &str = "registros";
/// Carpeta de las tareas dentro de `ROOT_HOME`. El builder no puede leerla.
const CARPETA_ORACULOS: &str = "oraculos";
/// Antigüedad a partir de la cual se borra un temporal.
const CADUCIDAD: Duration = Duration::from_secs(24 * 3600);
/// Carpeta de cada suite dentro del contenedor. Es el último argumento de su `cmd`. Cada suite monta
/// solo su carpeta, nunca la tarea entera (A2).
const DIR_CONTRATO: &str = "/oraculo/contrato";
const DIR_OCULTO: &str = "/oraculo/oculto";

/// Ejecutor de un contenedor. En producción es `contenedor::ejecutar`; los tests le dan uno falso.
type Correr<'a> = &'a dyn Fn(&Ejecucion) -> Result<Resultado, ErrorOraculo>;

fn correr_en_podman(e: &Ejecucion) -> Result<Resultado, ErrorOraculo> {
    contenedor::ejecutar(e)
}

/// Errores del Oracle. Salen con código 3, salvo `Ficha` (2). Ante uno de ellos no hay veredicto.
#[derive(Debug, ErrorDerive)]
pub enum ErrorOraculo {
    /// Infraestructura: podman ausente, timeout, error de podman o E/S.
    #[error("error de infraestructura del oráculo: {0}")]
    Infra(String),
    /// El junit falta o está mal formado.
    #[error("junit inválido: {0}")]
    Junit(String),
    /// La tarea no existe, no se lee o su `tarea.toml` es inválido.
    #[error("tarea inválida: {0}")]
    Tarea(String),
    /// Elección de juez inválida: sin razonamiento, sin vía API o sin modelo. Es configuración.
    #[error("elección inválida: {0}")]
    Eleccion(String),
    /// El spec de la tarea no tiene una ficha válida. Sin ficha no se evalúa.
    #[error(transparent)]
    Ficha(#[from] ErrorFicha),
    /// Colab: sin sesión de la CLI, GPU que no es T4 o un runtime que falló. Sale con código 3.
    #[error("{0}")]
    Colab(String),
    /// `root.toml` no carga. Lleva el código de salida que dio `Config`: 2 si un valor está fuera de rango.
    #[error("root.toml inválido: {1}")]
    Configuracion(u8, String),
    /// Uso de `oraculo calibrar`: dimensión sin defectos, `n` cero o sin repo bueno. Es error de uso: sale con 2.
    #[error("{0}")]
    Uso(String),
    /// Otra corrida de `oraculo pendientes` tiene la tarea. No espera: sale con código 3 y nombra al dueño.
    #[error("{0}")]
    Ocupado(String),
}

impl ErrorOraculo {
    /// Código de salida: 2 si la ficha o la elección del juez son inválidas, 3 en los demás casos.
    pub fn codigo_de_salida(&self) -> u8 {
        match self {
            ErrorOraculo::Ficha(e) => e.codigo_de_salida(),
            ErrorOraculo::Eleccion(_) | ErrorOraculo::Uso(_) => 2,
            ErrorOraculo::Configuracion(codigo, _) => *codigo,
            _ => 3,
        }
    }
}

/// Opciones de una evaluación.
#[derive(Debug, Clone)]
pub struct OpcionesOraculo {
    /// Límite de cada corrida de tests dentro del contenedor.
    pub timeout: Duration,
    /// `ROOT_HOME` donde viven los temporales. Sin valor, usa `root_home()`.
    pub home: Option<PathBuf>,
}

impl Default for OpcionesOraculo {
    fn default() -> Self {
        OpcionesOraculo {
            timeout: Duration::from_secs(3600),
            home: None,
        }
    }
}

/// Lo que cada suite necesita para correr: la copia del repo, la salida, las opciones
/// y las cachés que la fase de descarga dejó listas.
struct Corrida<'a> {
    repo: &'a Path,
    tarea: &'a Tarea,
    salida: &'a Path,
    o: &'a OpcionesOraculo,
    cache: Option<PathBuf>,
    rustsec: Option<PathBuf>,
    /// `ROOT_HOME`: donde van los registros de stderr, dentro de `oraculos/<tarea>/`.
    home: &'a Path,
}

/// Resultado de una suite: su clasificación, o el motivo por el que no tiene veredicto (A5).
#[derive(Debug)]
enum Suite {
    Clasificada(Clasificacion),
    SinVeredicto(String),
}

/// Evalúa `repo` con la tarea: corre el contrato y el oculto sobre una copia del repo, y las sondas.
pub fn evaluar(repo: &Path, tarea: &Tarea, o: &OpcionesOraculo) -> Result<Veredicto, ErrorOraculo> {
    // Todo lo que escribe la evaluación va a `home`: `root.toml`, `eventos.jsonl` y temporales.
    let home = o.home.clone().unwrap_or_else(root_home);
    // Las sondas no montan el oráculo: los tests ocultos no entran a sus contenedores (A2).
    let herramientas = |p: &Preparada| -> Box<dyn EjecutorHerramienta> {
        Box::new(EjecutorContenedor {
            imagen: tarea.imagen.clone(),
            repo: p.repo.clone(),
            oraculo: None,
            rustsec: p.rustsec.clone(),
            cache: p.cache.clone(),
            timeout: o.timeout,
            home: home.clone(),
        })
    };
    // Un root.toml que no carga no evalúa: las reglas de decisión tienen que ser las del usuario.
    let cfg = Config::cargar(&home.join("root.toml"))
        .map_err(|e| ErrorOraculo::Configuracion(e.codigo_de_salida(), e.to_string()))?;
    let rubricas = rubrica::cargar_embebidas()
        .map_err(|e| ErrorOraculo::Infra(format!("rúbricas embebidas: {e}")))?;
    let calibracion = Calibraciones::cargar(&home)?;
    let mut avisos = Vec::new();
    let juez_config = juez_config_de_produccion(&home, &cfg, &mut avisos);
    // Sin ítems del juez en las rúbricas, no se arma: no hay nada que llamar ni gastar.
    let juez = juez::hay_pedidos(&rubricas)
        .then(|| juez_diseno_de_produccion(&home, &mut avisos))
        .flatten();
    let nombre = juez
        .as_ref()
        .and_then(|j| j.eleccion.modelo.clone())
        .unwrap_or_default();
    let redactor = redactor_de_produccion(&home, &mut avisos);
    let deps = Dependencias {
        correr: &correr_en_podman,
        herramientas: &herramientas,
        juez: juez.as_ref().map(|j| j as &dyn ModeloJuez),
        modelo_juez: &nombre,
        juez_config: juez_config.as_ref().map(|j| j as &dyn ModeloJuez),
        calibracion: &calibracion,
        rubricas: &rubricas,
        reglas: cfg.decision.clone(),
        redactor: redactor.as_ref().map(|r| r as &dyn Redactor),
        avisos,
        podman: &PodmanReal,
        escenarios: &FabricaReal,
    };
    evaluar_con(repo, tarea, o, deps)
}

/// Corrida con la que el juez de diseño de producción aparece en el `Gate` y en el registro.
const CORRIDA_JUEZ_DISENO: &str = "oraculo-juez-diseno";

/// Juez de la configuración de Cargo de producción. Sin modelo válido, no hay juez y el aviso lo dice.
/// La descarga sigue y el juicio queda sin veredicto.
fn juez_config_de_produccion(
    home: &Path,
    cfg: &Config,
    avisos: &mut Vec<String>,
) -> Option<JuezProveedor> {
    match descarga::juez_config::juez_config_real_en(home, cfg) {
        Ok(juez) => Some(juez),
        Err(e) => {
            avisos.push(format!("juez de la configuración sin armar: {e}"));
            None
        }
    }
}

/// Juez de diseño de producción: la elección de Route para `juzga`, con su respaldo. Sin presupuesto o
/// sin elección válida, no hay juez y el aviso lo dice.
fn juez_diseno_de_produccion(home: &Path, avisos: &mut Vec<String>) -> Option<JuezProveedor> {
    let tokens = juez::LimitesPrompt::default().max_tokens;
    let eleccion = match crate::route::comando::rutear_en(home, Rol::Juzga, "diseno", false, tokens)
    {
        Ok(eleccion) => eleccion,
        Err(e) => {
            avisos.push(format!("juez de diseño sin armar: {e}"));
            return None;
        }
    };
    match JuezProveedor::nuevo_en(home, eleccion, CORRIDA_JUEZ_DISENO.into()) {
        Ok(juez) => Some(juez),
        Err(e) => {
            avisos.push(format!("juez de diseño sin armar: {e}"));
            None
        }
    }
}

/// Redactor de producción del informe: la elección de Route para `delega`. Sin elección válida, no hay
/// redactor y el resumen queda determinista; el aviso lo dice.
fn redactor_de_produccion(home: &Path, avisos: &mut Vec<String>) -> Option<RedactorProveedor> {
    match crate::route::comando::rutear_en(
        home,
        Rol::Delega,
        "informe",
        false,
        informe::MAX_TOKENS_REDACTOR,
    ) {
        Ok(eleccion) => Some(RedactorProveedor {
            eleccion,
            corrida: CORRIDA_INFORME.into(),
            home: home.to_path_buf(),
        }),
        Err(e) => {
            avisos.push(format!("redactor del informe sin armar: {e}"));
            None
        }
    }
}

/// Corrida con la que el redactor del informe aparece en el `Gate` y en el registro.
const CORRIDA_INFORME: &str = "oraculo-informe";

/// Dependencias de una evaluación. `evaluar` arma las de producción; los tests dan ejecutores falsos.
struct Dependencias<'a> {
    /// Corre una suite de tests en un contenedor.
    correr: Correr<'a>,
    /// Arma el ejecutor de las sondas. Va después de la descarga: necesita la copia y las cachés.
    herramientas: FabricaHerramientas<'a>,
    /// Juez de diseño (Tarea 10). `None`: no se armó o no corre, y Diseño queda sin veredicto.
    juez: Option<&'a dyn ModeloJuez>,
    /// Nombre del modelo del juez de diseño. Va a la fuente de cada nivel y de cada hallazgo.
    modelo_juez: &'a str,
    /// Juez de la configuración de Cargo (decisión 30). `None`: la configuración no se juzga.
    juez_config: Option<&'a dyn ModeloJuez>,
    /// Calibración de Root (Tarea 12). Vacía: nada está calibrado y `global` es `None`.
    calibracion: &'a Calibraciones,
    /// Rúbricas de la evaluación. Producción pasa las embebidas.
    rubricas: &'a [Rubrica],
    /// Reglas de la decisión (`[decision]` de `root.toml`).
    reglas: ReglasDecision,
    /// Redactor del informe. `None`: el resumen queda determinista.
    redactor: Option<&'a dyn Redactor>,
    /// Avisos que se conocen antes de evaluar, como un juez que no se pudo armar.
    avisos: Vec<String>,
    /// Podman de la construcción sin red (Tarea 9d2). Producción pasa el real.
    podman: &'a dyn Podman,
    /// Arranca los contenedores de escenario de la imagen construida. Producción pasa la real.
    escenarios: &'a dyn FabricaEscenarios,
}

/// Lo que la descarga deja listo para las sondas: la copia del repo y las cachés que monta cada contenedor.
struct Preparada {
    repo: PathBuf,
    cache: Option<PathBuf>,
    rustsec: Option<PathBuf>,
}

/// Arma el ejecutor de herramientas de una evaluación a partir de lo que dejó la descarga.
type FabricaHerramientas<'a> = &'a dyn Fn(&Preparada) -> Box<dyn EjecutorHerramienta>;

/// Las nueve dimensiones de `Veredicto.puntajes`, en el orden de `Dimension`.
const DIMENSIONES: [Dimension; 9] = [
    Dimension::Funciona,
    Dimension::Seguridad,
    Dimension::Eficiencia,
    Dimension::Estructura,
    Dimension::Arquitectura,
    Dimension::Diseno,
    Dimension::Completitud,
    Dimension::Entrega,
    Dimension::Pruebas,
];

/// Como `evaluar`, con las dependencias que se le pasen.
fn evaluar_con(
    repo: &Path,
    tarea: &Tarea,
    o: &OpcionesOraculo,
    deps: Dependencias,
) -> Result<Veredicto, ErrorOraculo> {
    let texto_spec = verificar_ficha(tarea)?;
    let rubricas = rubrica::etiquetas(deps.rubricas);
    let repo = fs::canonicalize(repo)
        .map_err(|e| ErrorOraculo::Infra(format!("no existe el repo {}: {e}", repo.display())))?;
    // Los temporales van a ROOT_HOME, nunca a TMPDIR ni a /tmp (desempate 6).
    let home = o.home.clone().unwrap_or_else(root_home);
    let (colab, avisos_colab) = pendientes::colab_para_veredicto(&home, &tarea.nombre)?;
    let commit = commit_de(&repo);
    let (colab, aviso_commit) = filtrar_colab_por_commit(colab, &commit);
    let mut avisos = deps.avisos;
    avisos.extend(avisos_colab);
    avisos.extend(aviso_commit);
    avisos.extend(pendientes::aviso_de_red(&colab));
    // Las ambigüedades de la revisión guardada del juez para este spec (Tarea 30, parte 1).
    let (ambiguedades, aviso_ficha) = ambiguedades_de_la_ficha(&texto_spec, &home);
    avisos.extend(aviso_ficha);
    let tmp = home.join(CARPETA_TEMPORALES);
    fs::create_dir_all(&tmp).map_err(|e| infra("directorio temporal de ROOT_HOME", e))?;
    limpiar_temporales(&tmp, SystemTime::now());
    // La copia aísla el repo: un artefacto del builder no altera el original.
    let copia = temporal(&tmp).map_err(|e| infra("directorio temporal", e))?;
    let repo_copia = copia.path().join("repo");
    copiar_repo(&repo, &repo_copia).map_err(|e| infra("copia del repo", e))?;
    // Cada juez suma el costo de sus llamadas. El de la configuración va antes que las suites.
    let juez_config = deps.juez_config.map(Medido::nuevo);
    let juez_config_ref = juez_config.as_ref().map(|m| m as &dyn ModeloJuez);
    // La descarga va antes de cualquier suite: es la única fase con red (decisión 17).
    let (preparacion, imagenes, _) =
        descarga::preparar_con_juez(&repo_copia, tarea, &home, o.timeout, juez_config_ref)?;
    let (cache, rustsec, hallazgos) = match preparacion {
        descarga::Preparacion::Lista {
            cache,
            rustsec,
            hallazgos,
        } => (cache, rustsec, hallazgos),
        descarga::Preparacion::SinVeredicto(hallazgos) => {
            let motivo = motivo_de_la_descarga(&hallazgos);
            let mut v = sin_veredicto(&repo, tarea, &colab, hallazgos, avisos, rubricas, &motivo);
            v.ambiguedades = ambiguedades.clone();
            return Ok(con_costo(v, &juez_config));
        }
    };
    let salida = temporal(&tmp).map_err(|e| infra("directorio de salida", e))?;
    // Podman rootless: si el contenedor corre con otro uid, la salida debe ser escribible por todos.
    fs::set_permissions(salida.path(), fs::Permissions::from_mode(0o777))
        .map_err(|e| infra("permisos de la salida", e))?;

    let corrida = Corrida {
        repo: &repo_copia,
        tarea,
        salida: salida.path(),
        o,
        cache: cache.clone(),
        rustsec: rustsec.clone(),
        home: &home,
    };
    // El contrato no tiene exclusiones: solo el oculto las usa.
    let contrato = match evaluar_suite(
        &corrida,
        &tarea.contrato,
        DIR_CONTRATO,
        XML_CONTRATO,
        &BTreeSet::new(),
        deps.correr,
    )? {
        Suite::Clasificada(c) => c,
        Suite::SinVeredicto(motivo) => {
            avisos.push(format!("contrato: {motivo}"));
            let motivo = format!("suite del contrato sin veredicto: {motivo}");
            let mut v = sin_veredicto(&repo, tarea, &colab, hallazgos, avisos, rubricas, &motivo);
            v.ambiguedades = ambiguedades.clone();
            return Ok(con_costo(v, &juez_config));
        }
    };
    let excluidos: BTreeSet<String> = tarea
        .entorno
        .iter()
        .chain(&tarea.oraculo_dudoso)
        .map(|e| e.test.clone())
        .collect();
    let oculto = match evaluar_suite(
        &corrida,
        &tarea.oculto,
        DIR_OCULTO,
        XML_OCULTO,
        &excluidos,
        deps.correr,
    )? {
        Suite::Clasificada(c) => c,
        Suite::SinVeredicto(motivo) => {
            avisos.push(format!("oculto: {motivo}"));
            let motivo = format!("suite oculta sin veredicto: {motivo}");
            let mut v = sin_veredicto(&repo, tarea, &colab, hallazgos, avisos, rubricas, &motivo);
            v.ambiguedades = ambiguedades.clone();
            return Ok(con_costo(v, &juez_config));
        }
    };
    let preparada = Preparada {
        repo: repo_copia.clone(),
        cache,
        rustsec,
    };
    let ej = (deps.herramientas)(&preparada);
    // La Entrega sale de `entregar_con`: construcción, sonda y escenarios en una sola pasada (desempate 7).
    // Un error de infraestructura deja Entrega sin veredicto con su motivo, como un error del ejecutor.
    let entrega = entrega::entregar_con(
        ej.as_ref(),
        &repo_copia,
        tarea,
        &home,
        &imagenes,
        deps.podman,
        deps.escenarios,
    )
    .unwrap_or_else(|error| entrega::entrega_sin_veredicto(error.to_string()));
    let juez_diseno = deps.juez.map(Medido::nuevo);
    let juez = JuezDiseno {
        modelo: juez_diseno.as_ref().map(|m| m as &dyn ModeloJuez),
        nombre: deps.modelo_juez,
        rubricas: deps.rubricas,
    };
    let medicion = medir(
        tarea,
        &repo_copia,
        ej.as_ref(),
        &contrato,
        &oculto,
        &hallazgos,
        &juez,
        entrega,
    )?;
    let mut tasas = BTreeMap::new();
    for (id, tasa) in &contrato.tasas {
        tasas.insert(format!("contrato:{id}"), tasa.clone());
    }
    for (id, tasa) in &oculto.tasas {
        tasas.insert(format!("oculto:{id}"), tasa.clone());
    }
    let mut todos = hallazgos;
    todos.extend(medicion.hallazgos);
    avisos.extend(medicion.avisos);
    let mut pendientes_del_veredicto = pendientes::desde(tarea, &colab);
    pendientes_del_veredicto.extend(medicion.pendientes);
    let mut puntajes = medicion.puntajes;
    deps.calibracion.aplicar(&tarea.nombre, &mut puntajes);
    let sin_calibrar = sin_calibrar(&puntajes);
    let global = calibra::global_de(&puntajes);
    let mut veredicto = Veredicto {
        v: VERSION,
        tarea: tarea.nombre.clone(),
        repo: nombre_de(&repo),
        commit,
        contrato: contrato.conteo.clone(),
        oculto: oculto.conteo.clone(),
        puntajes,
        hallazgos: deduplicar_hallazgos(todos),
        global,
        estado: Estado::SinVeredicto,
        costo_usd: costo_de(&juez_config) + costo_de(&juez_diseno),
        informe: None,
        rubricas,
        repeticiones: tarea.repeticiones,
        inestables: oculto.inestables.clone(),
        inestables_contrato: contrato.inestables.clone(),
        tasas,
        pendientes: pendientes_del_veredicto,
        avisos,
        requisitos: medicion.requisitos,
        sin_calibrar,
        ocultos_fallidos: fallidos_de(&oculto),
        ambiguedades,
    };
    // La decisión sale de reglas deterministas; el estado sale de la decisión (Tarea 13b).
    let redactor_medido = deps.redactor.map(RedactorMedido::nuevo);
    let informe = informe::informar_con(
        &veredicto,
        tarea,
        &deps.reglas,
        deps.rubricas,
        redactor_medido.as_ref().map(|r| r as &dyn Redactor),
    )?;
    veredicto.costo_usd += redactor_medido.as_ref().map_or(0.0, RedactorMedido::costo);
    veredicto.estado = informe::estado_de(Some(&informe));
    veredicto.informe = Some(informe);
    Ok(veredicto)
}

/// Ids ordenados de los tests de la suite oculta que fallan de forma estable. Los inestables van aparte.
fn fallidos_de(c: &Clasificacion) -> Vec<String> {
    c.estables
        .iter()
        .filter(|(_, resultado)| **resultado == ResultadoTest::Falla)
        .map(|(id, _)| id.clone())
        .collect()
}

/// Envoltorio que suma el costo de las llamadas de un juez. `evaluar_con` lo lee para `costo_usd`.
struct Medido<'a> {
    juez: &'a dyn ModeloJuez,
    costo: Cell<f64>,
}

impl<'a> Medido<'a> {
    fn nuevo(juez: &'a dyn ModeloJuez) -> Self {
        Medido {
            juez,
            costo: Cell::new(0.0),
        }
    }

    fn costo(&self) -> f64 {
        self.costo.get()
    }
}

impl ModeloJuez for Medido<'_> {
    fn consultar(&self, prompt: &Prompt) -> Result<RespuestaJuez, ErrorOraculo> {
        let respuesta = self.juez.consultar(prompt)?;
        self.costo.set(self.costo.get() + respuesta.costo_usd);
        Ok(respuesta)
    }
}

/// Costo de un juez medido, o cero si no se armó.
fn costo_de(juez: &Option<Medido>) -> f64 {
    juez.as_ref().map_or(0.0, Medido::costo)
}

/// Veredicto de una evaluación que no corrió, con el costo de los jueces que sí llamó.
fn con_costo(mut v: Veredicto, juez_config: &Option<Medido>) -> Veredicto {
    v.costo_usd = costo_de(juez_config);
    v
}

/// Lo que las sondas y el juez aportan al veredicto: los nueve puntajes, los hallazgos, el estado de cada
/// requisito y los avisos del juez (los hallazgos que el gate de evidencia descartó).
struct Medicion {
    puntajes: Vec<Puntaje>,
    hallazgos: Vec<Hallazgo>,
    requisitos: Vec<EstadoRequisito>,
    avisos: Vec<String>,
    /// Lo que la construcción de la Entrega no pudo medir (Tarea 9d2). Va al `Veredicto`.
    pendientes: Vec<Pendiente>,
}

/// El juez de diseño que `medir` usa: el modelo si se armó, su nombre y las rúbricas de la evaluación.
struct JuezDiseno<'a> {
    modelo: Option<&'a dyn ModeloJuez>,
    nombre: &'a str,
    rubricas: &'a [Rubrica],
}

/// Lo que el juez aporta a `medir`: su resultado si corrió, sus hallazgos que pasan el gate y los topes
/// que las sondas ponen a los ítems `mixto`.
struct Juicio {
    resultado: Option<ResultadoJuez>,
    hallazgos: Vec<Hallazgo>,
    avisos: Vec<String>,
    topes: BTreeMap<String, u8>,
}

/// Mide las nueve dimensiones con las sondas, los requisitos y el juez de diseño. `descarga` son los
/// hallazgos de la descarga: los que tocan una dimensión con sondas entran a su puntaje.
#[allow(clippy::too_many_arguments)]
fn medir(
    tarea: &Tarea,
    repo: &Path,
    ej: &dyn EjecutorHerramienta,
    contrato: &Clasificacion,
    oculto: &Clasificacion,
    descarga: &[Hallazgo],
    juez: &JuezDiseno,
    entrega: ResultadoEntrega,
) -> Result<Medicion, ErrorOraculo> {
    let resultados = requisitos::desde_clasificacion(oculto);
    let estados = requisitos::evaluar_requisitos(
        &tarea.requisito,
        &resultados,
        &entrega.escenarios,
        &tarea.entorno,
        &tarea.oraculo_dudoso,
        &BTreeMap::new(),
    );
    let mutacion = mutacion::sonda_mutacion(ej, tarea);
    let mut sondas = vec![entrega.sonda.clone(), mutacion.sonda.clone()];
    sondas.extend(perfiles::sondas_nativas(repo, tarea));
    sondas.extend(perfiles::herramientas::sondas_herramientas(ej, tarea, repo));
    let mut hallazgos: Vec<Hallazgo> = sondas.iter().flat_map(|s| s.hallazgos.clone()).collect();
    // El juez ve los hallazgos de las sondas (el ciclo de módulos topa `diseno.modularidad`).
    let juicio = juzgar_diseno(tarea, juez, repo, &hallazgos)?;
    hallazgos.extend(requisitos::hallazgos_inestables(
        &tarea.requisito,
        &resultados,
    ));
    let Juicio {
        resultado,
        hallazgos: de_juez,
        avisos,
        topes,
    } = juicio;
    hallazgos.extend(de_juez);
    let puntajes = DIMENSIONES
        .iter()
        .map(|&dimension| match dimension {
            Dimension::Funciona => funciona(tarea, contrato, oculto, &entrega.escenarios),
            Dimension::Diseno => puntaje_diseno(juez, resultado.as_ref(), &topes),
            Dimension::Completitud => {
                requisitos::puntaje_completitud(&estados).unwrap_or_else(|| {
                    sin_veredicto_puntaje(dimension, "ningún requisito tiene nivel medido".into())
                })
            }
            _ => {
                let de_la_dimension = sondas
                    .iter()
                    .filter(|s| s.dimension == dimension)
                    .cloned()
                    .collect();
                let extra = descarga
                    .iter()
                    .filter(|h| h.dimension == dimension)
                    .cloned()
                    .collect();
                let de_sondas = puntaje_de(dimension, de_la_dimension, extra, tarea);
                match parte_juez(dimension, juez, resultado.as_ref(), &topes) {
                    Some(de_juez) => juntar_partes(dimension, de_sondas, de_juez),
                    None => de_sondas,
                }
            }
        })
        .collect();
    let mut avisos = avisos;
    avisos.extend(entrega.avisos);
    Ok(Medicion {
        puntajes,
        hallazgos,
        requisitos: estados,
        avisos,
        pendientes: entrega.pendientes,
    })
}

/// Corre el juez de diseño sobre la copia del repo y filtra sus hallazgos con el gate de evidencia.
/// Sin modelo, o sin ítems de juez en las rúbricas, no lo llama: el juicio queda vacío.
fn juzgar_diseno(
    tarea: &Tarea,
    juez: &JuezDiseno,
    repo: &Path,
    sondas: &[Hallazgo],
) -> Result<Juicio, ErrorOraculo> {
    let topes = juez::topes_desde_sondas(sondas, juez.rubricas);
    let Some(modelo) = juez.modelo.filter(|_| juez::hay_pedidos(juez.rubricas)) else {
        return Ok(Juicio {
            resultado: None,
            hallazgos: Vec::new(),
            avisos: Vec::new(),
            topes,
        });
    };
    let spec_ruta = tarea.dir.join(&tarea.spec);
    let spec = fs::read_to_string(&spec_ruta).map_err(|e| infra("leer la spec del juez", e))?;
    let mapa = mapa::construir(repo)?;
    let entrada = EntradaJuez {
        spec: &spec,
        requisitos: &tarea.requisito,
        mapa: &mapa,
        hallazgos_sondas: sondas,
        rubricas: juez.rubricas,
        repo,
    };
    let mut resultado = juez::juzgar(
        &entrada,
        modelo,
        juez.nombre,
        &juez::LimitesPrompt::default(),
    );
    let filtrado = evidencia::filtrar_hallazgos(repo, std::mem::take(&mut resultado.hallazgos));
    let mut avisos = Vec::new();
    if !filtrado.descartados.is_empty() {
        avisos.push(format!(
            "{} hallazgos del juez de diseño no pasaron el gate de evidencia",
            filtrado.descartados.len()
        ));
    }
    Ok(Juicio {
        resultado: Some(resultado),
        hallazgos: filtrado.validos,
        avisos,
        topes,
    })
}

/// Rúbrica de los ítems `juez` y `mixto` de una dimensión, juntados de las rúbricas que la nombran. Los
/// ítems de sonda no pasan: sus niveles los fija la sonda, no el juez. `None` si no hay ítems del juez.
fn rubrica_del_juez(rubricas: &[Rubrica], dimension: Dimension) -> Option<Rubrica> {
    let mut id = None;
    let mut version = 0;
    let mut items = Vec::new();
    for r in rubricas
        .iter()
        .filter(|r| juez::dimension_de(&r.id) == Some(dimension))
    {
        id.get_or_insert_with(|| r.id.clone());
        version = r.version;
        items.extend(r.item.iter().filter(|i| i.mide != Mide::Sonda).cloned());
    }
    (!items.is_empty()).then(|| Rubrica {
        id: id.unwrap_or_default(),
        version,
        item: items,
    })
}

/// Parte del juez de una dimensión. `None` si sus rúbricas no tienen ítems del juez. Sin juez, sin juicio
/// o con `SinVeredicto`, la parte no tiene veredicto y lleva la causa.
fn parte_juez(
    dimension: Dimension,
    juez: &JuezDiseno,
    resultado: Option<&ResultadoJuez>,
    topes: &BTreeMap<String, u8>,
) -> Option<Puntaje> {
    let rubrica = rubrica_del_juez(juez.rubricas, dimension)?;
    let Some(resultado) = resultado else {
        return Some(sin_veredicto_puntaje(dimension, "juez no corrido".into()));
    };
    if let EstadoJuez::SinVeredicto(causa) = &resultado.estado {
        return Some(sin_veredicto_puntaje(dimension, causa.clone()));
    }
    // Un ítem sin respuesta queda en `faltantes` y no entra al promedio: no vale 0 (desempate 3).
    let puntuacion = match juez::puntuar_con_juez(&rubrica, resultado, topes, juez.nombre) {
        Ok(puntuacion) => puntuacion,
        Err(e) => {
            return Some(sin_veredicto_puntaje(dimension, format!("rúbrica: {e}")));
        }
    };
    Some(match puntuacion.valor {
        Some(valor) => Puntaje {
            dimension,
            valor,
            calibrada: false,
            exactitud_balanceada: None,
            items: puntuacion.items,
            sin_veredicto: None,
        },
        None => sin_veredicto_puntaje(dimension, "ningún ítem tiene nivel".into()),
    })
}

/// Diseño toma su valor solo del juez. Sin ítems de diseño en las rúbricas, o sin juez, no hay veredicto.
fn puntaje_diseno(
    juez: &JuezDiseno,
    resultado: Option<&ResultadoJuez>,
    topes: &BTreeMap<String, u8>,
) -> Puntaje {
    parte_juez(Dimension::Diseno, juez, resultado, topes).unwrap_or_else(|| {
        let motivo = if juez.rubricas.is_empty() {
            "sin rúbricas embebidas"
        } else {
            "sin ítems de diseño en las rúbricas"
        };
        sin_veredicto_puntaje(Dimension::Diseno, motivo.into())
    })
}

/// Junta las partes de una dimensión: sondas y juez. Si una parte no tiene veredicto, la dimensión lo
/// lleva con los motivos. Si tienen, promedio simple de los valores, con los ítems del juez.
fn juntar_partes(dimension: Dimension, sondas: Puntaje, juez: Puntaje) -> Puntaje {
    let motivos: Vec<String> = [sondas.sin_veredicto.clone(), juez.sin_veredicto.clone()]
        .into_iter()
        .flatten()
        .collect();
    if !motivos.is_empty() {
        return sin_veredicto_puntaje(dimension, motivos.join("; "));
    }
    Puntaje {
        dimension,
        valor: (sondas.valor + juez.valor) / 2.0,
        calibrada: false,
        exactitud_balanceada: None,
        items: juez.items,
        sin_veredicto: None,
    }
}

/// Puntaje de una dimensión con sus sondas. Sin sondas no hay veredicto. Una sola sonda sin hallazgos
/// de la descarga conserva su valor, porque eficiencia y mutación no se miden con `puntuar_hallazgos`.
/// Con varias partes, o con hallazgos de la descarga, se juntan con `combinar`.
fn puntaje_de(
    dimension: Dimension,
    mut sondas: Vec<ResultadoSonda>,
    extra: Vec<Hallazgo>,
    tarea: &Tarea,
) -> Puntaje {
    if sondas.is_empty() {
        let motivo = format!(
            "no hay sondas de {dimension:?} para el lenguaje {}",
            tarea.lenguaje
        );
        return sin_veredicto_puntaje(dimension, motivo);
    }
    let juntada = if sondas.len() == 1 && extra.is_empty() {
        sondas.pop()
    } else {
        if !extra.is_empty() {
            // Parte sin valor propio: solo lleva hallazgos, que `combinar` vuelve a puntuar.
            sondas.push(ResultadoSonda {
                dimension,
                valor: Some(1.0),
                motivo: None,
                hallazgos: extra,
            });
        }
        combinar(dimension, sondas, &tarea.requisito)
    };
    match juntada {
        Some(sonda) => sonda.a_puntaje().unwrap_or_else(|| {
            sin_veredicto_puntaje(
                dimension,
                sonda.motivo.unwrap_or_else(|| "sin motivo".into()),
            )
        }),
        None => sin_veredicto_puntaje(dimension, "sin sondas".into()),
    }
}

/// Funciona: la fracción de tests que pasan (contrato y oculto, sin los de `[[entorno]]`) y, si la tarea
/// tiene escenarios, la de escenarios verdes. Una parte sin veredicto deja la dimensión sin veredicto.
fn funciona(
    tarea: &Tarea,
    contrato: &Clasificacion,
    oculto: &Clasificacion,
    escenarios: &BTreeMap<String, bool>,
) -> Puntaje {
    const DIMENSION: Dimension = Dimension::Funciona;
    let entorno: BTreeSet<&str> = tarea.entorno.iter().map(|e| e.test.as_str()).collect();
    let (pasa_contrato, total_contrato) = fraccion_pasa(contrato, &entorno);
    let (pasa_oculto, total_oculto) = fraccion_pasa(oculto, &entorno);
    let total = total_contrato + total_oculto;
    if total == 0 {
        return sin_veredicto_puntaje(DIMENSION, "no hay tests que medir".into());
    }
    let mut partes = vec![(pasa_contrato + pasa_oculto) as f32 / total as f32];
    if !tarea.escenario.is_empty() {
        let faltan: Vec<&str> = tarea
            .escenario
            .iter()
            .map(|e| e.id.as_str())
            .filter(|id| !escenarios.contains_key(*id))
            .collect();
        if !faltan.is_empty() {
            let motivo = format!("escenarios sin medir: {}", faltan.join(", "));
            return sin_veredicto_puntaje(DIMENSION, motivo);
        }
        let verdes = tarea
            .escenario
            .iter()
            .filter(|e| escenarios.get(&e.id) == Some(&true))
            .count();
        partes.push(verdes as f32 / tarea.escenario.len() as f32);
    }
    let valor = partes.iter().sum::<f32>() / partes.len() as f32;
    Puntaje {
        dimension: DIMENSION,
        valor,
        calibrada: false,
        exactitud_balanceada: None,
        items: Vec::new(),
        sin_veredicto: None,
    }
}

/// Tests que pasan y tests medidos de una suite, sin los de `[[entorno]]`. Los omitidos no cuentan.
fn fraccion_pasa(c: &Clasificacion, entorno: &BTreeSet<&str>) -> (u32, u32) {
    let mut pasa: u32 = 0;
    let mut total: u32 = 0;
    for (id, resultado) in &c.estables {
        if entorno.contains(id.as_str()) {
            continue;
        }
        total += 1;
        if *resultado == ResultadoTest::Pasa {
            pasa += 1;
        }
    }
    total += c
        .inestables
        .iter()
        .filter(|id| !entorno.contains(id.as_str()))
        .count() as u32;
    (pasa, total)
}

/// Puntaje de una dimensión sin veredicto: `valor` 0 y el motivo.
fn sin_veredicto_puntaje(dimension: Dimension, motivo: String) -> Puntaje {
    Puntaje {
        dimension,
        valor: 0.0,
        calibrada: false,
        exactitud_balanceada: None,
        items: Vec::new(),
        sin_veredicto: Some(motivo),
    }
}

/// Las nueve dimensiones sin veredicto, con el mismo motivo.
fn puntajes_sin_veredicto(motivo: &str) -> Vec<Puntaje> {
    DIMENSIONES
        .iter()
        .map(|&d| sin_veredicto_puntaje(d, motivo.to_string()))
        .collect()
}

/// Dimensiones con veredicto que no están calibradas. Sin calibración todavía, son todas las que tienen
/// veredicto.
fn sin_calibrar(puntajes: &[Puntaje]) -> Vec<Dimension> {
    puntajes
        .iter()
        .filter(|p| p.sin_veredicto.is_none() && !p.calibrada)
        .map(|p| p.dimension)
        .collect()
}

/// Motivo de una descarga sin veredicto: el detalle de su primer hallazgo, si lo hay.
fn motivo_de_la_descarga(hallazgos: &[Hallazgo]) -> String {
    match hallazgos.first() {
        Some(h) => format!("descarga sin veredicto: {}", h.detalle),
        None => "descarga sin veredicto".into(),
    }
}

/// Carga la tarea `nombre` de `ROOT_HOME` y la evalúa con las opciones por defecto.
/// La usan `r00t oraculo evaluar` y la herramienta MCP `root_evaluar`.
pub fn evaluar_por_nombre(nombre: &str, repo: &Path) -> Result<Veredicto, ErrorOraculo> {
    let tarea = Tarea::cargar(nombre)?;
    evaluar(repo, &tarea, &OpcionesOraculo::default())
}

/// Resultados de Colab del commit del repo. Los de otro commit no entran al veredicto (B16). El aviso
/// cuenta cuántos quedaron afuera.
fn filtrar_colab_por_commit(
    colab: BTreeMap<String, pendientes::ColabEntrada>,
    commit: &str,
) -> (BTreeMap<String, pendientes::ColabEntrada>, Option<String>) {
    let total = colab.len();
    let del_commit: BTreeMap<String, pendientes::ColabEntrada> = colab
        .into_iter()
        .filter(|(_, c)| c.commit == commit)
        .collect();
    let aviso = (del_commit.len() < total).then(|| {
        format!(
            "{} resultados de Colab de otro commit no entran al veredicto",
            total - del_commit.len()
        )
    });
    (del_commit, aviso)
}

/// Lee el spec de la tarea y exige una ficha válida. Va antes de copiar o descargar nada.
/// Aviso del veredicto cuando no hay revisión del juez para el spec. Sin ella, las ambigüedades no se miden.
const AVISO_SIN_REVISION_JUEZ: &str = "ficha sin revisión del juez: ambigüedades sin medir";

/// Lee el spec y exige una ficha válida. Devuelve el texto, que la revisión guardada usa como clave.
fn verificar_ficha(tarea: &Tarea) -> Result<String, ErrorOraculo> {
    let ruta = tarea.dir.join(&tarea.spec);
    let texto = fs::read_to_string(&ruta).map_err(|fuente| ErrorFicha::Leer {
        ruta: ruta.display().to_string(),
        fuente,
    })?;
    ficha::extraer(&texto)?;
    Ok(texto)
}

/// Ambigüedades de la revisión guardada del juez para `spec`, como `línea <n>: <frase>`, y el aviso que
/// corresponde. Sin revisión, el aviso dice que no se midieron. Una revisión ilegible da aviso, no error.
fn ambiguedades_de_la_ficha(spec: &str, home: &Path) -> (Vec<String>, Option<String>) {
    match ficha::ambiguedades_guardadas(home, spec) {
        Ok(Some(ambiguedades)) => (
            ambiguedades
                .iter()
                .map(|a| format!("línea {}: {}", a.linea, a.frase))
                .collect(),
            None,
        ),
        Ok(None) => (Vec::new(), Some(AVISO_SIN_REVISION_JUEZ.into())),
        Err(causa) => (
            Vec::new(),
            Some(format!("revisión de la ficha ilegible: {causa}")),
        ),
    }
}

/// Veredicto de una evaluación que no corrió: sin conteos, con sus hallazgos y pendientes. Las nueve
/// dimensiones van sin veredicto, con `motivo`.
fn sin_veredicto(
    repo: &Path,
    tarea: &Tarea,
    colab: &BTreeMap<String, pendientes::ColabEntrada>,
    hallazgos: Vec<Hallazgo>,
    avisos: Vec<String>,
    rubricas: Vec<String>,
    motivo: &str,
) -> Veredicto {
    Veredicto {
        tarea: tarea.nombre.clone(),
        repo: nombre_de(repo),
        commit: commit_de(repo),
        repeticiones: tarea.repeticiones,
        hallazgos,
        pendientes: pendientes::desde(tarea, colab),
        avisos,
        puntajes: puntajes_sin_veredicto(motivo),
        rubricas,
        ..Veredicto::default()
    }
}

/// Carpeta temporal dentro de `tmp`, con el prefijo de `evaluar`.
fn temporal(tmp: &Path) -> io::Result<tempfile::TempDir> {
    tempfile::Builder::new()
        .prefix(PREFIJO_TEMPORAL)
        .tempdir_in(tmp)
}

/// Borra las carpetas `evaluar-*` y `colab-*` de `tmp` con más de 24 horas (B18). Los fallos se
/// ignoran: un temporal viejo que no se borra no impide evaluar.
fn limpiar_temporales(tmp: &Path, ahora: SystemTime) {
    let Ok(entradas) = fs::read_dir(tmp) else {
        return;
    };
    for entrada in entradas.flatten() {
        let nombre = entrada.file_name().to_string_lossy().into_owned();
        if !nombre.starts_with(PREFIJO_TEMPORAL) && !nombre.starts_with(PREFIJO_COLAB_TEMPORAL) {
            continue;
        }
        let ruta = entrada.path();
        let Ok(meta) = fs::symlink_metadata(&ruta) else {
            continue;
        };
        let Ok(modificado) = meta.modified() else {
            continue;
        };
        if !ahora
            .duration_since(modificado)
            .is_ok_and(|edad| edad > CADUCIDAD)
        {
            continue;
        }
        // Un enlace se borra como enlace: su destino puede estar fuera de `tmp` y no se toca.
        let _ = if meta.is_symlink() {
            borrado::borrar_archivo_dentro(tmp, &ruta)
        } else if meta.is_dir() {
            borrado::borrar_dir_dentro(tmp, &ruta)
        } else {
            continue;
        };
    }
}

/// Corre una suite `repeticiones` veces. Después corre solo sus tests inestables,
/// hasta `repeticiones_inestable` corridas en total. Una suite de pytest lista antes lo que pytest
/// recolecta: su junit vale solo si trae exactamente eso y su código cuadra con las fallas (A5).
/// Las demás suites no tienen esa lista: su junit lo escribe el propio comando.
fn evaluar_suite(
    c: &Corrida,
    comando: &Comando,
    dir: &str,
    xml: &str,
    excluidos: &BTreeSet<String>,
    correr: Correr,
) -> Result<Suite, ErrorOraculo> {
    let pytest = es_pytest(&comando.cmd);
    let esperados = if pytest {
        match coleccion_de(c, &comando.cmd, dir, correr)? {
            Ok(nodos) => Some(claves_de_nodos(&nodos)),
            Err(motivo) => return Ok(Suite::SinVeredicto(motivo)),
        }
    } else {
        None
    };
    let mut completas = Vec::new();
    for _ in 0..c.tarea.repeticiones {
        match correr_una(
            c,
            &comando.cmd,
            dir,
            xml,
            pytest,
            esperados.as_ref(),
            correr,
        )? {
            Ok(junit) => completas.push(junit),
            Err(motivo) => return Ok(Suite::SinVeredicto(motivo)),
        }
    }
    let previa = clasificar(&completas, &[], excluidos);
    let mut extras = Vec::new();
    if !previa.inestables.is_empty() && c.tarea.repeticiones_inestable > c.tarea.repeticiones {
        let reintento = reintento_de(comando, dir, &previa.inestables)?;
        for _ in c.tarea.repeticiones..c.tarea.repeticiones_inestable {
            // El reintento corre solo los inestables: su junit no trae la lista completa.
            match correr_una(c, &reintento, dir, xml, pytest, None, correr)? {
                Ok(junit) => extras.push(junit),
                Err(motivo) => return Ok(Suite::SinVeredicto(motivo)),
            }
        }
    }
    Ok(Suite::Clasificada(clasificar(
        &completas, &extras, excluidos,
    )))
}

/// Si el comando corre pytest. Solo entonces hay lista de recolección y código que cotejar (A5).
fn es_pytest(cmd: &str) -> bool {
    cmd.contains("pytest")
}

/// Comando de las corridas extra: corre solo los tests `inestables` de la suite `dir`.
/// Usa `reintento` si existe; si no, reemplaza `dir` al final de `cmd`.
fn reintento_de(
    comando: &Comando,
    dir: &str,
    inestables: &[String],
) -> Result<String, ErrorOraculo> {
    let lista = inestables
        .iter()
        .map(|id| comillas(&node_id(dir, id)))
        .collect::<Vec<_>>()
        .join(" ");
    if let Some(plantilla) = &comando.reintento {
        return Ok(plantilla.replace("{tests}", &lista));
    }
    match comando.cmd.trim_end().strip_suffix(dir) {
        Some(prefijo) => Ok(format!("{prefijo}{lista}")),
        None => Err(ErrorOraculo::Tarea(format!(
            "el comando no termina en {dir} y no tiene reintento: agregalo en tarea.toml"
        ))),
    }
}

/// Node id de pytest para un test `archivo::nombre` de la suite `dir`.
fn node_id(dir: &str, id: &str) -> String {
    match id.split_once("::") {
        Some((archivo, nombre)) => format!("{dir}/{archivo}.py::{nombre}"),
        None => format!("{dir}/{id}"),
    }
}

/// Entrecomilla para `sh`.
fn comillas(texto: &str) -> String {
    format!("'{}'", texto.replace('\'', "'\\''"))
}

/// Resultado de clasificar los tests de una suite a través de sus corridas.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Clasificacion {
    /// Tests estables con su resultado (`Pasa` o `Falla`). Los omitidos no aparecen.
    estables: BTreeMap<String, ResultadoTest>,
    /// Tests que se omitieron en todas sus corridas. No cuentan en el conteo, pero sí en el universo de
    /// su requisito, como no verdes (A10).
    omitidos: BTreeSet<String>,
    /// Ids ordenados de los tests inestables.
    inestables: Vec<String>,
    /// Tasa de cada test inestable, con la clave `archivo::nombre` (sin prefijo de suite).
    tasas: BTreeMap<String, TasaTest>,
    /// Conteo de la suite. Un inestable no pasa, pero sí cuenta en el total.
    conteo: Conteo,
}

/// Resultado de un test a través de sus muestras: estable, con su resultado único, o inestable, con
/// su tasa de aprobación. Lo usan la evaluación en la laptop y la importación de Colab (desempate 28).
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Muestreo {
    Estable(ResultadoTest),
    Inestable(TasaTest),
}

/// Clasifica las muestras de un test. Cada muestra es el resultado en una corrida, o `None` si el
/// test no aparece en esa corrida. Es inestable si falta en alguna muestra o si los resultados se
/// mezclan (Pasa con Falla, o Omite con otro). La tasa cuenta solo las muestras presentes.
/// Sin ninguna muestra presente no hay nada que clasificar y da `None`.
pub(super) fn muestreo_de(muestras: &[Option<ResultadoTest>]) -> Option<Muestreo> {
    let presentes: Vec<ResultadoTest> = muestras.iter().flatten().copied().collect();
    let primera = *presentes.first()?;
    let ausente = muestras.iter().any(Option::is_none);
    let mezcla = presentes.iter().any(|m| *m != primera);
    if !ausente && !mezcla {
        return Some(Muestreo::Estable(primera));
    }
    let pasa = presentes
        .iter()
        .filter(|m| **m == ResultadoTest::Pasa)
        .count() as u32;
    let total = presentes.len() as u32;
    Some(Muestreo::Inestable(TasaTest {
        pasa,
        total,
        wilson_inf: wilson_inferior(pasa, total),
    }))
}

/// Clasifica cada test con sus muestras. Cada corrida completa aporta una muestra por test presente.
/// Un test ausente en una corrida completa es inestable. Las corridas extra solo suman muestras.
/// Un test es inestable si tiene resultados mezclados (Pasa con Falla, o Omite con otro).
/// Los excluidos no entran al cálculo.
fn clasificar(
    completas: &[Junit],
    extras: &[Junit],
    excluidos: &BTreeSet<String>,
) -> Clasificacion {
    let ids: BTreeSet<&str> = completas
        .iter()
        .flat_map(|j| j.tests.keys().map(String::as_str))
        .collect();
    let mut c = Clasificacion::default();
    for id in ids {
        if excluidos.contains(id) {
            continue;
        }
        // Las corridas extra solo suman muestras: una que no trae el test no cuenta como ausencia.
        let mut muestras: Vec<Option<ResultadoTest>> =
            completas.iter().map(|j| j.tests.get(id).copied()).collect();
        muestras.extend(
            extras
                .iter()
                .filter_map(|j| j.tests.get(id).copied())
                .map(Some),
        );
        match muestreo_de(&muestras) {
            Some(Muestreo::Inestable(tasa)) => {
                c.inestables.push(id.to_string());
                c.tasas.insert(id.to_string(), tasa);
            }
            Some(Muestreo::Estable(ResultadoTest::Omite)) => {
                c.omitidos.insert(id.to_string());
            }
            Some(Muestreo::Estable(r)) => {
                c.estables.insert(id.to_string(), r);
            }
            None => {}
        }
    }
    let pasa = c
        .estables
        .values()
        .filter(|r| **r == ResultadoTest::Pasa)
        .count();
    c.conteo = Conteo {
        pasa: pasa as u32,
        total: (c.estables.len() + c.inestables.len()) as u32,
    };
    c
}

fn infra(contexto: &str, e: io::Error) -> ErrorOraculo {
    ErrorOraculo::Infra(format!("{contexto}: {e}"))
}

/// Borra el junit de la corrida anterior. Sin borrarlo, una corrida sin XML pasaría con datos viejos.
fn borrar_xml(salida: &Path, xml: &str) -> Result<(), ErrorOraculo> {
    borrado::borrar_archivo_dentro(salida, &salida.join(xml))
        .map_err(|e| infra("borrar el junit de la corrida anterior", e))
}

/// Una corrida de la suite en un contenedor nuevo. `Ok(Err(motivo))` si el junit no vale (A5).
/// `Err` solo si la infraestructura falla o si no hay junit (A2: ahí el stderr va a `registros/`).
/// Con `pytest`, el código de salida debe cuadrar con las fallas, y con `esperados` (si hay) el junit
/// trae exactamente esas claves.
fn correr_una(
    c: &Corrida,
    cmd: &str,
    dir: &str,
    xml: &str,
    pytest: bool,
    esperados: Option<&BTreeSet<String>>,
    correr: Correr,
) -> Result<Result<Junit, String>, ErrorOraculo> {
    borrar_xml(c.salida, xml)?;
    let resultado = correr(&ejecucion_de_suite(c, &cmd_con_cache(c, cmd), dir))?;
    let ruta = c.salida.join(xml);
    if !ruta.is_file() {
        return Err(sin_junit(c, xml, &resultado));
    }
    let Ok(crudo) = junit::leer(&ruta) else {
        return Ok(Err("el junit no se pudo leer".into()));
    };
    if pytest {
        let claves: BTreeSet<String> = crudo.tests.keys().cloned().collect();
        if let Some(esperados) = esperados
            && claves != *esperados
        {
            return Ok(Err(format!(
                "el junit trae {} tests y pytest recolectó {}",
                claves.len(),
                esperados.len()
            )));
        }
        let hay_falla = crudo.tests.values().any(|r| *r == ResultadoTest::Falla);
        let cuadra = match resultado.codigo {
            0 => !hay_falla,
            1 => hay_falla,
            _ => false,
        };
        if !cuadra {
            return Ok(Err(format!(
                "el código de pytest ({}) no coincide con las fallas del junit",
                resultado.codigo
            )));
        }
    }
    Ok(Ok(crudo.con_ids()))
}

/// Lista de node ids que pytest recolecta para la suite. `Ok(Err(motivo))` si pytest no recolectó
/// tests o salió con error. Sin lista en `/salida`, o con código 0 y sin nodos, es error de
/// infraestructura (A2): no es una corrida de pytest.
fn coleccion_de(
    c: &Corrida,
    cmd: &str,
    dir: &str,
    correr: Correr,
) -> Result<Result<Vec<String>, String>, ErrorOraculo> {
    let ruta = c.salida.join(ARCHIVO_COLECCION);
    borrado::borrar_archivo_dentro(c.salida, &ruta)
        .map_err(|e| infra("borrar la lista de tests de la corrida anterior", e))?;
    let orden = comando_de_coleccion(&cmd_con_cache(c, cmd));
    let resultado = correr(&ejecucion_de_suite(c, &orden, dir))?;
    if !ruta.is_file() {
        return Err(sin_junit(c, ARCHIVO_COLECCION, &resultado));
    }
    // Código 5 de pytest: ningún test recolectado. Otro código distinto de 0 no da veredicto.
    match resultado.codigo {
        0 => {}
        5 => return Ok(Err("pytest no recolectó tests".into())),
        otro => {
            return Ok(Err(format!(
                "pytest --collect-only salió con código {otro}"
            )));
        }
    }
    let texto = fs::read_to_string(&ruta).map_err(|e| infra("leer la lista de tests", e))?;
    let mut nodos: Vec<String> = Vec::new();
    for linea in texto.lines().map(str::trim) {
        if linea.contains(".py::") && !nodos.iter().any(|n| n == linea) {
            nodos.push(linea.to_string());
        }
    }
    if nodos.is_empty() {
        return Err(sin_junit(c, ARCHIVO_COLECCION, &resultado));
    }
    if nodos.iter().any(|n| junit::clave_de_nodo(n).is_none()) {
        return Ok(Err("la lista de tests recolectados no se pudo leer".into()));
    }
    Ok(Ok(nodos))
}

/// Claves de junit de los node ids recolectados.
fn claves_de_nodos(nodos: &[String]) -> BTreeSet<String> {
    nodos
        .iter()
        .filter_map(|n| junit::clave_de_nodo(n))
        .collect()
}

/// Comando que lista lo que pytest recolecta: el de la suite sin su junit, con `--collect-only -q`.
/// La lista va a `/salida/coleccion.txt`.
fn comando_de_coleccion(cmd: &str) -> String {
    let mut sin_junit = cmd.to_string();
    while let Some(inicio) = sin_junit.find("--junitxml=") {
        let fin = sin_junit[inicio..]
            .find(char::is_whitespace)
            .map_or(sin_junit.len(), |n| inicio + n);
        sin_junit.replace_range(inicio..fin, "");
    }
    format!(
        "{} --collect-only -q > /salida/{ARCHIVO_COLECCION}",
        sin_junit.trim_end()
    )
}

/// El comando con el preludio de la caché cuando la hay (Tarea 6c-B).
fn cmd_con_cache(c: &Corrida, cmd: &str) -> String {
    // Con caché, la suite arranca desde la caché sin red: cargo para Rust, pip para Python (Tarea 6c-B).
    match (c.cache.is_some(), c.tarea.lenguaje.as_str()) {
        (true, "rust") => format!("{}{cmd}", descarga::PRELUDIO),
        (true, "python") => format!("{}{cmd}", descarga::PRELUDIO_PYTHON),
        _ => cmd.to_string(),
    }
}

/// Lo que un contenedor de suite necesita: su carpeta (solo esa, no la tarea) y la salida.
/// `dir` es la carpeta dentro del contenedor, y es el último argumento del comando.
fn ejecucion_de_suite(c: &Corrida, cmd: &str, dir: &str) -> Ejecucion {
    let nombre = dir.rsplit('/').next().unwrap_or_default();
    Ejecucion {
        imagen: c.tarea.imagen.clone(),
        repo: Some(c.repo.to_path_buf()),
        // Sin carpeta de suite, nada se monta: el comando falla con su propio error (A2).
        oraculo: Some(c.tarea.dir.join(nombre)).filter(|ruta| ruta.is_dir()),
        oraculo_destino: Some(dir.to_string()),
        salida: c.salida.to_path_buf(),
        cmd: cmd.to_string(),
        env: Vec::new(),
        timeout: c.o.timeout,
        red: false,
        cache: c.cache.clone(),
        cache_rw: false,
        rustsec: c.rustsec.clone(),
        trabajo: None,
        secretos: Vec::new(),
    }
}

/// Error de una corrida sin junit. El stderr del contenedor va a `registros/` y el mensaje solo
/// nombra el archivo: el texto del stderr puede traer tests ocultos (A2).
fn sin_junit(c: &Corrida, xml: &str, resultado: &Resultado) -> ErrorOraculo {
    let donde = match registrar_stderr(c, xml, &resultado.stderr) {
        Some(ruta) => format!("el stderr del contenedor quedó en {}", ruta.display()),
        None => "no se pudo guardar el stderr del contenedor".into(),
    };
    ErrorOraculo::Infra(format!(
        "el comando no produjo /salida/{xml} (código {}); {donde}",
        resultado.codigo
    ))
}

/// Guarda el stderr de una corrida en `oraculos/<tarea>/registros/`. `None` si no se pudo escribir.
fn registrar_stderr(c: &Corrida, xml: &str, stderr: &str) -> Option<PathBuf> {
    let carpeta = c
        .home
        .join(CARPETA_ORACULOS)
        .join(&c.tarea.nombre)
        .join(CARPETA_REGISTROS);
    fs::create_dir_all(&carpeta).ok()?;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let base = xml.trim_end_matches(".xml");
    let ruta = carpeta.join(format!("{base}-{nanos}.stderr"));
    fs::write(&ruta, stderr).ok()?;
    Some(ruta)
}

/// Veredicto para la herramienta MCP: solo conteos y niveles, sin ids de tests ocultos (A2). La CLI
/// para el humano conserva todo. Campo por campo: `inestables`, `tasas` (`oculto:`), `pendientes` y las
/// evidencias de requisitos y de ítems de puntaje se vacían; un hallazgo que cita un test oculto se
/// quita; el resto (conteos, estado, `avisos`, hallazgos sin ids) se conserva.
pub fn redactar_para_mcp(mut v: Veredicto) -> Veredicto {
    let ocultos: BTreeSet<String> = informe::ids_ocultos(&v);
    v.inestables.clear();
    v.ocultos_fallidos.clear();
    v.tasas.retain(|k, _| !k.starts_with("oculto:"));
    v.pendientes.clear();
    v.hallazgos.retain(|h| {
        !ocultos
            .iter()
            .any(|id| h.cita.contains(id.as_str()) || h.detalle.contains(id.as_str()))
    });
    for puntaje in &mut v.puntajes {
        puntaje.items.clear();
        let cita_oculto = puntaje
            .sin_veredicto
            .as_deref()
            .is_some_and(|m| ocultos.iter().any(|id| m.contains(id.as_str())));
        if cita_oculto {
            puntaje.sin_veredicto = Some("motivo con ids de tests ocultos: ver la CLI".into());
        }
    }
    for requisito in &mut v.requisitos {
        requisito.evidencia.clear();
    }
    if let Some(informe) = &mut v.informe {
        for requisito in &mut informe.requisitos {
            requisito.evidencia.clear();
        }
        informe::ocultar_tests(informe, &ocultos);
    }
    v
}

/// Copia `origen` a `destino` sin `target/` ni `.git/` de la raíz y sin enlaces simbólicos.
fn copiar_repo(origen: &Path, destino: &Path) -> io::Result<()> {
    fs::create_dir_all(destino)?;
    copiar_dir(origen, destino, true)
}

fn copiar_dir(origen: &Path, destino: &Path, raiz: bool) -> io::Result<()> {
    for entrada in fs::read_dir(origen)? {
        let entrada = entrada?;
        let tipo = entrada.file_type()?;
        let nombre = entrada.file_name();
        if tipo.is_symlink() {
            continue;
        }
        if raiz && EXCLUIDOS.iter().any(|excluido| nombre == *excluido) {
            continue;
        }
        let dest = destino.join(&nombre);
        if tipo.is_dir() {
            fs::create_dir(&dest)?;
            copiar_dir(&entrada.path(), &dest, false)?;
        } else if tipo.is_file() {
            fs::copy(entrada.path(), &dest)?;
        }
    }
    Ok(())
}

fn nombre_de(repo: &Path) -> String {
    repo.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Variables de git que redirigen qué repo se lee. Un hook de git las exporta: el subproceso no debe heredarlas.
const VARIABLES_GIT_DE_REPO: &[&str] = &[
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_COMMON_DIR",
    "GIT_NAMESPACE",
    "GIT_PREFIX",
    "GIT_CEILING_DIRECTORIES",
    "GIT_DISCOVERY_ACROSS_FILESYSTEM",
];

/// Comando `git -C <repo>` sin las variables `GIT_*` de `VARIABLES_GIT_DE_REPO` heredadas del entorno.
fn comando_git(repo: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(repo).args(args);
    for nombre in VARIABLES_GIT_DE_REPO {
        cmd.env_remove(nombre);
    }
    cmd
}

/// Commit corto del repo original. Vacío si no es un repo git.
pub(crate) fn commit_de(repo: &Path) -> String {
    comando_git(repo, &["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|salida| salida.status.success())
        .map(|salida| String::from_utf8_lossy(&salida.stdout).trim().to_string())
        .unwrap_or_default()
}

/// Código de salida de un estado: 0 verde, 1 rojo, 3 sin veredicto.
pub fn codigo_de_estado(estado: Estado) -> u8 {
    match estado {
        Estado::Verde => 0,
        Estado::Rojo => 1,
        Estado::SinVeredicto => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oracle::contenedor::SalidaPodman;
    use crate::oracle::entrega::tests::{PodmanFalso, responde_build, salida_ok, salida_sin_red};
    use crate::oracle::entrega::{CorredorDeEscenario, FabricaEscenarios, Podman};

    #[test]
    fn comando_git_quita_las_variables_que_redirigen_el_repo() {
        // Un hook de git exporta GIT_DIR: el subproceso no debe leer el repo de Root en vez del evaluado.
        let cmd = comando_git(Path::new("/x"), &["rev-parse", "HEAD"]);
        for nombre in VARIABLES_GIT_DE_REPO {
            assert!(
                cmd.get_envs()
                    .any(|(k, v)| k == std::ffi::OsStr::new(nombre) && v.is_none()),
                "{nombre} no se quita del entorno del subproceso"
            );
        }
    }
    use crate::oracle::perfiles::herramientas::{EjecutorHerramienta, SalidaHerramienta};
    use crate::oracle::tarea::{Escenario, Exclusion};
    use crate::tipos::{
        Cumplimiento, Decision, Dimension, EstadoRequisito, Fuente, NivelItem, Puntaje,
    };
    use filetime::{FileTime, set_symlink_file_times};
    use serde_json::{Value, json};
    use std::fs;
    use std::path::PathBuf;

    fn conteo(pasa: u32, total: u32) -> Conteo {
        Conteo { pasa, total }
    }

    fn listar(dir: &Path) -> Vec<PathBuf> {
        let mut salida = Vec::new();
        for entrada in fs::read_dir(dir).expect("lee dir") {
            let ruta = entrada.expect("entrada").path();
            if ruta.is_dir() {
                salida.extend(listar(&ruta));
            }
            salida.push(ruta);
        }
        salida.sort();
        salida
    }

    #[test]
    fn oraculo_copia_excluye_target_y_git_de_la_raiz() {
        let origen = tempfile::tempdir().expect("temp dir");
        let o = origen.path();
        fs::write(o.join("a.txt"), "uno").expect("escribe");
        fs::create_dir_all(o.join("target")).expect("dir");
        fs::write(o.join("target/x"), "build").expect("escribe");
        fs::create_dir_all(o.join(".git")).expect("dir");
        fs::write(o.join(".git/HEAD"), "ref").expect("escribe");
        fs::create_dir_all(o.join("sub/target")).expect("dir");
        fs::write(o.join("sub/target/y"), "anidado").expect("escribe");

        let destino = tempfile::tempdir().expect("temp dir");
        let d = destino.path().join("repo");
        copiar_repo(o, &d).expect("copia");

        assert_eq!(fs::read_to_string(d.join("a.txt")).expect("lee"), "uno");
        assert!(!d.join("target").exists(), "no copia target de la raíz");
        assert!(!d.join(".git").exists(), "no copia .git");
        assert!(d.join("sub/target/y").exists(), "solo excluye la raíz");
    }

    #[test]
    fn oraculo_copia_ignora_enlaces_simbolicos() {
        let afuera = tempfile::tempdir().expect("temp dir");
        fs::write(afuera.path().join("secreto"), "no copiar").expect("escribe");
        let origen = tempfile::tempdir().expect("temp dir");
        std::os::unix::fs::symlink(afuera.path().join("secreto"), origen.path().join("enlace"))
            .expect("symlink");
        fs::write(origen.path().join("real.txt"), "ok").expect("escribe");

        let destino = tempfile::tempdir().expect("temp dir");
        let d = destino.path().join("repo");
        copiar_repo(origen.path(), &d).expect("copia");

        assert!(!d.join("enlace").exists());
        assert!(d.join("real.txt").exists());
    }

    #[test]
    fn oraculo_copia_deja_el_origen_igual() {
        let origen = tempfile::tempdir().expect("temp dir");
        fs::create_dir_all(origen.path().join("src")).expect("dir");
        fs::write(origen.path().join("src/lib.rs"), "fn x() {}").expect("escribe");
        let antes = listar(origen.path());

        let destino = tempfile::tempdir().expect("temp dir");
        copiar_repo(origen.path(), &destino.path().join("repo")).expect("copia");

        assert_eq!(listar(origen.path()), antes);
    }

    #[test]
    fn oraculo_el_estado_sale_de_la_decision_y_no_del_conteo() {
        let base = tempfile::tempdir().expect("base");
        let tarea = tarea_rust(base.path());
        let repo = repo_rust(base.path(), LOCK_SANO);
        let home = home_con_cache(base.path(), LOCK_SANO);
        let correr = correr_suites(JUNIT_CONTRATO);
        // Con el mínimo por defecto, la completitud 0,75 no llega a 0,9: no listo, rojo.
        let v = evaluar_falso(&repo, &tarea, &home, &correr, &herramientas(&[])).expect("evalúa");
        assert_eq!(v.oculto, conteo(1, 2), "t4 falla en el oculto");
        assert_eq!(
            v.informe.as_ref().map(|i| i.decision),
            Some(Decision::NoListo)
        );
        assert_eq!(v.estado, Estado::Rojo);
        // Con el mínimo en 0,75, el mismo conteo da listo y verde: el conteo no decide.
        let reglas = ReglasDecision {
            completitud_minima: 0.75,
            ..ReglasDecision::default()
        };
        let herramientas_falsas = herramientas(&[]);
        let deps = Dependencias {
            reglas,
            ..deps_de(&correr, &herramientas_falsas)
        };
        let v = evaluar_con(&repo, &tarea, &opciones(&home), deps).expect("evalúa");
        assert_eq!(
            v.informe.as_ref().map(|i| i.decision),
            Some(Decision::Listo)
        );
        assert_eq!(v.estado, Estado::Verde);
    }

    #[test]
    fn oraculo_con_informe_el_estado_es_el_que_da_la_decision() {
        let base = tempfile::tempdir().expect("base");
        let v = veredicto_sano(base.path());
        let informe = v
            .informe
            .as_ref()
            .expect("con suites completas hay informe");
        assert_eq!(v.estado, informe::estado_de(Some(informe)));
    }

    #[test]
    fn muestreo_todas_iguales_es_estable() {
        let m = muestreo_de(&[Some(ResultadoTest::Pasa), Some(ResultadoTest::Pasa)]);
        assert_eq!(m, Some(Muestreo::Estable(ResultadoTest::Pasa)));
    }

    #[test]
    fn muestreo_pasa_y_falla_mezclados_es_inestable_con_tasa() {
        let m = muestreo_de(&[Some(ResultadoTest::Pasa), Some(ResultadoTest::Falla)]);
        let Some(Muestreo::Inestable(tasa)) = m else {
            panic!("debía ser inestable: {m:?}");
        };
        assert_eq!((tasa.pasa, tasa.total), (1, 2));
        assert!((tasa.wilson_inf - wilson_inferior(1, 2)).abs() < 1e-12);
    }

    #[test]
    fn muestreo_omite_mezclado_con_pasa_es_inestable() {
        let m = muestreo_de(&[Some(ResultadoTest::Omite), Some(ResultadoTest::Pasa)]);
        assert!(matches!(m, Some(Muestreo::Inestable(t)) if (t.pasa, t.total) == (1, 2)));
    }

    #[test]
    fn muestreo_ausente_en_una_muestra_es_inestable() {
        let m = muestreo_de(&[Some(ResultadoTest::Pasa), None]);
        assert!(matches!(m, Some(Muestreo::Inestable(t)) if (t.pasa, t.total) == (1, 1)));
    }

    #[test]
    fn muestreo_sin_ninguna_muestra_no_da_nada() {
        assert_eq!(muestreo_de(&[None, None]), None);
        assert_eq!(muestreo_de(&[]), None);
    }

    fn junit_de(filas: &[(&str, ResultadoTest)]) -> Junit {
        let mut j = Junit::default();
        for (clave, resultado) in filas {
            j.tests.insert((*clave).to_string(), *resultado);
        }
        j
    }

    fn sin_exclusiones() -> BTreeSet<String> {
        BTreeSet::new()
    }

    #[test]
    fn repeticiones_todo_pasa_en_todas_cuenta_como_estable() {
        let j = junit_de(&[("a::t1", ResultadoTest::Pasa)]);
        let c = clasificar(&[j.clone(), j.clone(), j], &[], &sin_exclusiones());
        assert_eq!(c.conteo, Conteo { pasa: 1, total: 1 });
        assert!(c.inestables.is_empty());
        assert!(c.tasas.is_empty());
    }

    #[test]
    fn repeticiones_falla_en_todas_es_estable_que_falla() {
        let j = junit_de(&[("a::t1", ResultadoTest::Falla)]);
        let c = clasificar(&[j.clone(), j.clone(), j], &[], &sin_exclusiones());
        assert_eq!(c.conteo, Conteo { pasa: 0, total: 1 });
        assert_eq!(c.estables["a::t1"], ResultadoTest::Falla);
    }

    #[test]
    fn inestable_resultados_mezclados_no_pasan() {
        let r1 = junit_de(&[
            ("a::t_flaky", ResultadoTest::Falla),
            ("a::t_ok", ResultadoTest::Pasa),
        ]);
        let r2 = junit_de(&[
            ("a::t_flaky", ResultadoTest::Pasa),
            ("a::t_ok", ResultadoTest::Pasa),
        ]);
        let r3 = junit_de(&[
            ("a::t_flaky", ResultadoTest::Falla),
            ("a::t_ok", ResultadoTest::Pasa),
        ]);
        let c = clasificar(&[r1, r2, r3], &[], &sin_exclusiones());
        assert_eq!(c.inestables, vec!["a::t_flaky".to_string()]);
        assert_eq!(c.conteo, Conteo { pasa: 1, total: 2 });
    }

    #[test]
    fn repeticiones_omitido_en_todas_no_cuenta() {
        let j = junit_de(&[("a::t1", ResultadoTest::Omite)]);
        let c = clasificar(&[j.clone(), j], &[], &sin_exclusiones());
        assert_eq!(c.conteo, Conteo { pasa: 0, total: 0 });
        assert!(c.inestables.is_empty());
    }

    #[test]
    fn repeticiones_test_ausente_en_una_repeticion_es_inestable() {
        let con = junit_de(&[("a::t1", ResultadoTest::Pasa)]);
        let sin = junit_de(&[]);
        let c = clasificar(&[con, sin], &[], &sin_exclusiones());
        assert_eq!(c.inestables, vec!["a::t1".to_string()]);
    }

    #[test]
    fn inestable_ausente_en_corrida_extra_no_suma_muestra() {
        let con = junit_de(&[("a::t1", ResultadoTest::Pasa)]);
        let sin = junit_de(&[]);
        let c = clasificar(&[con.clone(), con], &[sin], &sin_exclusiones());
        assert!(c.inestables.is_empty());
        assert_eq!(c.conteo, Conteo { pasa: 1, total: 1 });
        assert!(c.tasas.is_empty());
    }

    #[test]
    fn inestable_corridas_extra_suman_muestras_y_tasa_wilson() {
        let f = junit_de(&[("a::t_flaky", ResultadoTest::Falla)]);
        let p = junit_de(&[("a::t_flaky", ResultadoTest::Pasa)]);
        let c = clasificar(
            &[f, p.clone()],
            &[p.clone(), junit_de(&[("a::t_flaky", ResultadoTest::Falla)])],
            &sin_exclusiones(),
        );
        assert_eq!(c.inestables, vec!["a::t_flaky".to_string()]);
        let tasa = &c.tasas["a::t_flaky"];
        assert_eq!((tasa.pasa, tasa.total), (2, 4));
        assert!((tasa.wilson_inf - wilson_inferior(2, 4)).abs() < 1e-12);
    }

    #[test]
    fn inestable_omite_mezclado_con_pasa_es_inestable() {
        let p = junit_de(&[("a::t1", ResultadoTest::Pasa)]);
        let o = junit_de(&[("a::t1", ResultadoTest::Omite)]);
        let c = clasificar(&[p, o], &[], &sin_exclusiones());
        assert_eq!(c.inestables, vec!["a::t1".to_string()]);
        assert_eq!(c.tasas["a::t1"].total, 2);
    }

    #[test]
    fn inestable_test_estable_no_tiene_tasa() {
        let p = junit_de(&[("a::t1", ResultadoTest::Pasa)]);
        let c = clasificar(&[p.clone(), p], &[], &sin_exclusiones());
        assert!(c.tasas.is_empty());
    }

    #[test]
    fn inestable_reintento_derivado_reemplaza_la_cola() {
        let c = Comando {
            cmd: "x --junitxml=/salida/oculto.xml /oraculo/oculto".into(),
            reintento: None,
        };
        let r =
            reintento_de(&c, "/oraculo/oculto", &["test_f::t_flaky".to_string()]).expect("deriva");
        assert_eq!(
            r,
            "x --junitxml=/salida/oculto.xml '/oraculo/oculto/test_f.py::t_flaky'"
        );
    }

    #[test]
    fn inestable_reintento_con_plantilla_reemplaza_tests() {
        let c = Comando {
            cmd: "x".into(),
            reintento: Some("pytest -q {tests}".into()),
        };
        let ids = ["a::t1".to_string(), "b::t2".to_string()];
        let r = reintento_de(&c, "/oraculo/contrato", &ids).expect("plantilla");
        assert_eq!(
            r,
            "pytest -q '/oraculo/contrato/a.py::t1' '/oraculo/contrato/b.py::t2'"
        );
    }

    #[test]
    fn inestable_reintento_sin_cola_conocida_es_error_de_tarea() {
        let c = Comando {
            cmd: "pytest -q".into(),
            reintento: None,
        };
        assert!(matches!(
            reintento_de(&c, "/oraculo/oculto", &["a::t".to_string()]),
            Err(ErrorOraculo::Tarea(_))
        ));
    }

    #[test]
    fn inestable_reintento_escapa_comillas_simples() {
        let c = Comando {
            cmd: "x /oraculo/oculto".into(),
            reintento: None,
        };
        let r = reintento_de(&c, "/oraculo/oculto", &["a::it's".to_string()]).expect("deriva");
        assert_eq!(r, "x '/oraculo/oculto/a.py::it'\\''s'");
    }

    fn poner_mtime(ruta: &Path, cuando: SystemTime) {
        fs::File::open(ruta)
            .expect("abre la carpeta")
            .set_modified(cuando)
            .expect("fija la fecha");
    }

    #[test]
    fn temporales_limpieza_borra_viejas_y_conserva_recientes_y_ajenos() {
        let home = tempfile::tempdir().expect("temp dir");
        let tmp = home.path().join("tmp");
        let viejo = tmp.join("evaluar-viejo");
        let reciente = tmp.join("evaluar-reciente");
        let otra = tmp.join("otra-carpeta");
        let ajeno = tmp.join("ajeno.txt");
        fs::create_dir_all(&viejo).expect("dir");
        fs::write(viejo.join("resto"), "x").expect("escribe");
        fs::create_dir_all(&reciente).expect("dir");
        fs::create_dir_all(&otra).expect("dir");
        fs::write(&ajeno, "x").expect("escribe");
        let ahora = SystemTime::now();
        poner_mtime(&viejo, ahora - Duration::from_secs(25 * 3600));
        poner_mtime(&otra, ahora - Duration::from_secs(48 * 3600));

        limpiar_temporales(&tmp, ahora);

        assert!(!viejo.exists(), "la carpeta de más de 24 h se borra");
        assert!(reciente.exists(), "la carpeta reciente se conserva");
        assert!(otra.exists(), "sin el prefijo no se toca");
        assert!(ajeno.exists(), "un archivo ajeno no se toca");
    }

    #[test]
    fn temporales_enlace_viejo_se_borra_como_enlace_y_su_destino_queda() {
        let home = tempfile::tempdir().expect("temp dir");
        let tmp = home.path().join("tmp");
        fs::create_dir_all(&tmp).expect("dir");
        let fuera = tempfile::tempdir().expect("destino fuera de home");
        fs::write(fuera.path().join("dato"), "x").expect("escribe");
        let enlace = tmp.join("evaluar-viejo");
        std::os::unix::fs::symlink(fuera.path(), &enlace).expect("enlace");
        let ahora = SystemTime::now();
        let viejo = FileTime::from_system_time(ahora - Duration::from_secs(25 * 3600));
        set_symlink_file_times(&enlace, viejo, viejo).expect("fija el mtime del enlace");

        limpiar_temporales(&tmp, ahora);

        assert!(
            fs::symlink_metadata(&enlace).is_err(),
            "el enlace de más de 24 h se borra"
        );
        assert_eq!(
            fs::read_to_string(fuera.path().join("dato")).expect("destino"),
            "x",
            "el destino queda intacto"
        );
    }

    #[test]
    fn temporales_enlace_reciente_se_conserva() {
        let home = tempfile::tempdir().expect("temp dir");
        let tmp = home.path().join("tmp");
        fs::create_dir_all(&tmp).expect("dir");
        let fuera = tempfile::tempdir().expect("destino fuera de home");
        let enlace = tmp.join("evaluar-reciente");
        std::os::unix::fs::symlink(fuera.path(), &enlace).expect("enlace");

        limpiar_temporales(&tmp, SystemTime::now());

        assert!(
            fs::symlink_metadata(&enlace).is_ok(),
            "el enlace reciente se conserva"
        );
    }

    #[test]
    fn oraculo_borrar_xml_rechaza_ruta_fuera_de_la_salida() {
        let base = tempfile::tempdir().expect("temp dir");
        let salida = base.path().join("salida");
        fs::create_dir_all(&salida).expect("salida");
        fs::write(base.path().join("fuera.xml"), "x").expect("escribe");
        assert!(borrar_xml(&salida, "../fuera.xml").is_err());
        assert!(base.path().join("fuera.xml").exists());
    }

    #[test]
    fn temporales_limpieza_sin_carpeta_no_falla() {
        let home = tempfile::tempdir().expect("temp dir");
        limpiar_temporales(&home.path().join("no-existe"), SystemTime::now());
    }

    #[test]
    fn exclusion_entorno_no_cuenta_en_el_calculo() {
        let r1 = junit_de(&[
            ("a::t_flaky", ResultadoTest::Falla),
            ("a::t_ok", ResultadoTest::Pasa),
        ]);
        let r2 = junit_de(&[
            ("a::t_flaky", ResultadoTest::Pasa),
            ("a::t_ok", ResultadoTest::Pasa),
        ]);
        let excluidos = BTreeSet::from(["a::t_flaky".to_string()]);
        let c = clasificar(&[r1, r2], &[], &excluidos);
        assert!(c.inestables.is_empty());
        assert_eq!(c.conteo, Conteo { pasa: 1, total: 1 });
    }

    #[test]
    fn exclusion_de_test_inexistente_se_ignora() {
        let j = junit_de(&[("a::t1", ResultadoTest::Pasa)]);
        let excluidos = BTreeSet::from(["a::no_existe".to_string()]);
        let c = clasificar(&[j.clone(), j], &[], &excluidos);
        assert_eq!(c.conteo, Conteo { pasa: 1, total: 1 });
    }

    #[test]
    fn oraculo_codigo_de_estado_verde_rojo_y_sin_veredicto() {
        assert_eq!(codigo_de_estado(Estado::Verde), 0);
        assert_eq!(codigo_de_estado(Estado::Rojo), 1);
        assert_eq!(codigo_de_estado(Estado::SinVeredicto), 3);
    }

    // --- Tarea 23: suites con ejecutor falso (sin podman) ---

    /// Tarea mínima con `dir` real (con `contrato/` y `oculto/`) bajo `base`.
    fn tarea_de_prueba(base: &Path) -> Tarea {
        fs::create_dir_all(base.join("contrato")).expect("contrato");
        fs::create_dir_all(base.join("oculto")).expect("oculto");
        Tarea {
            nombre: "tarea-prueba".into(),
            lenguaje: "python".into(),
            imagen: "x".into(),
            containerfile: None,
            spec: "s".into(),
            repeticiones: 1,
            repeticiones_inestable: 1,
            contrato: Comando {
                cmd: "python3 -m pytest -q --junitxml=/salida/contrato.xml /oraculo/contrato"
                    .into(),
                reintento: None,
            },
            oculto: Comando {
                cmd: "python3 -m pytest -q --junitxml=/salida/oculto.xml /oraculo/oculto".into(),
                reintento: None,
            },
            entorno: Vec::new(),
            oraculo_dudoso: Vec::new(),
            requisito: Vec::new(),
            arquitectura: None,
            eficiencia: None,
            pruebas: None,
            entrega: None,
            escenario: Vec::new(),
            calibracion: None,
            dir: base.to_path_buf(),
        }
    }

    fn resultado_falso(codigo: i32, stderr: &str) -> contenedor::Resultado {
        contenedor::Resultado {
            codigo,
            stdout: String::new(),
            stderr: stderr.into(),
            segundos: 0.0,
        }
    }

    /// Un ejecutor que responde la recolección con `coleccion` y, si no es recolección, deja `xml`
    /// como junit y devuelve `codigo`.
    fn ejecutor_con(
        coleccion: &'static str,
        xml: Option<&'static str>,
        codigo: i32,
    ) -> impl Fn(&contenedor::Ejecucion) -> Result<contenedor::Resultado, ErrorOraculo> {
        move |e: &contenedor::Ejecucion| {
            if e.cmd.contains("--collect-only") {
                fs::write(e.salida.join(ARCHIVO_COLECCION), coleccion).expect("coleccion");
                return Ok(resultado_falso(0, ""));
            }
            if let Some(texto) = xml {
                fs::write(e.salida.join(XML_OCULTO), texto).expect("junit");
            }
            Ok(resultado_falso(codigo, ""))
        }
    }

    fn correr_suite_de_prueba(
        base: &Path,
        tarea: &Tarea,
        correr: &dyn Fn(&contenedor::Ejecucion) -> Result<contenedor::Resultado, ErrorOraculo>,
    ) -> Result<Suite, ErrorOraculo> {
        let repo = base.join("repo");
        let salida = base.join("salida");
        let home = base.join("home");
        fs::create_dir_all(&repo).expect("repo");
        fs::create_dir_all(&salida).expect("salida");
        fs::create_dir_all(&home).expect("home");
        let o = OpcionesOraculo::default();
        let c = Corrida {
            repo: &repo,
            tarea,
            salida: &salida,
            o: &o,
            cache: None,
            rustsec: None,
            home: &home,
        };
        evaluar_suite(
            &c,
            &tarea.oculto,
            DIR_OCULTO,
            XML_OCULTO,
            &BTreeSet::new(),
            correr,
        )
    }

    fn coleccion_de(n: usize) -> &'static str {
        Box::leak(
            (1..=n)
                .map(|i| format!("tests/test_a.py::t{i}\n"))
                .chain(std::iter::once(format!("{n} tests collected in 0.01s\n")))
                .collect::<String>()
                .into_boxed_str(),
        )
    }

    const JUNIT_TRES: &str = r#"<testsuite>
  <testcase classname="tests.test_a" name="t1"/>
  <testcase classname="tests.test_a" name="t2"/>
  <testcase classname="tests.test_a" name="t3"/>
</testsuite>"#;

    #[test]
    fn oraculo_junit_de_3_con_40_recolectados_es_sin_veredicto() {
        let base = tempfile::tempdir().expect("base");
        let tarea = tarea_de_prueba(base.path());
        let correr = ejecutor_con(coleccion_de(40), Some(JUNIT_TRES), 0);
        let s = correr_suite_de_prueba(base.path(), &tarea, &correr).expect("sin error de infra");
        assert!(
            matches!(s, Suite::SinVeredicto(_)),
            "3 tests de 40 no es veredicto: {s:?}"
        );
    }

    #[test]
    fn oraculo_codigo_de_pytest_que_no_cuadra_con_las_fallas_es_sin_veredicto() {
        let con_falla = r#"<testsuite><testcase classname="tests.test_a" name="t1"><failure message="x"/></testcase></testsuite>"#;
        let sin_falla = r#"<testsuite><testcase classname="tests.test_a" name="t1"/></testsuite>"#;
        let casos = [
            // Código 0 con una falla en el junit: el junit no es de esta corrida.
            (
                ejecutor_con(coleccion_de(1), Some(con_falla), 0),
                "0 con falla",
            ),
            // Código 1 sin fallas en el junit.
            (
                ejecutor_con(coleccion_de(1), Some(sin_falla), 1),
                "1 sin falla",
            ),
            // Código 5: ningún test recolectado. Código 2: interrumpida.
            (ejecutor_con(coleccion_de(1), Some(sin_falla), 5), "5"),
            (ejecutor_con(coleccion_de(1), Some(sin_falla), 2), "2"),
        ];
        for (correr, nombre) in casos {
            let base = tempfile::tempdir().expect("base");
            let tarea = tarea_de_prueba(base.path());
            let s = correr_suite_de_prueba(base.path(), &tarea, &correr).expect("sin infra");
            assert!(matches!(s, Suite::SinVeredicto(_)), "{nombre}: {s:?}");
        }
    }

    #[test]
    fn oraculo_junit_falsificado_con_codigo_de_pytest_valido_no_cuenta_si_no_son_los_recolectados()
    {
        // El producto escribe un junit con otro test; pytest recolectó `test_a.py::t1`.
        let base = tempfile::tempdir().expect("base");
        let tarea = tarea_de_prueba(base.path());
        let falso = r#"<testsuite><testcase classname="tests.otro" name="z"/></testsuite>"#;
        let correr = ejecutor_con(coleccion_de(1), Some(falso), 0);
        let s = correr_suite_de_prueba(base.path(), &tarea, &correr).expect("sin infra");
        assert!(matches!(s, Suite::SinVeredicto(_)), "{s:?}");
    }

    #[test]
    fn oraculo_stderr_con_texto_oculto_no_sale_en_el_error_y_queda_en_registros() {
        const SECRETO: &str = "/oraculo/oculto/test_a.py::t_secreto contenido-filtrado";
        let base = tempfile::tempdir().expect("base");
        let tarea = tarea_de_prueba(base.path());
        // Sin junit: el producto escribió el texto en stderr y mató a pytest.
        let correr = |e: &contenedor::Ejecucion| {
            if e.cmd.contains("--collect-only") {
                fs::write(e.salida.join(ARCHIVO_COLECCION), coleccion_de(1)).expect("coleccion");
                return Ok(resultado_falso(0, ""));
            }
            Ok(resultado_falso(-9, SECRETO))
        };
        let error = correr_suite_de_prueba(base.path(), &tarea, &correr)
            .expect_err("sin junit es error de infraestructura");
        let texto = error.to_string();
        assert!(!texto.contains("contenido-filtrado"), "{texto}");
        assert!(!texto.contains("t_secreto"), "{texto}");
        assert_eq!(error.codigo_de_salida(), 3);
        let registros = base.path().join("home/oraculos/tarea-prueba/registros");
        let archivos: Vec<PathBuf> = fs::read_dir(&registros)
            .expect("registros existe")
            .flatten()
            .map(|e| e.path())
            .collect();
        assert_eq!(archivos.len(), 1, "un archivo de stderr: {archivos:?}");
        let contenido = fs::read_to_string(&archivos[0]).expect("lee el registro");
        assert!(contenido.contains("contenido-filtrado"), "{contenido}");
        let nombre = archivos[0]
            .file_name()
            .expect("nombre")
            .to_string_lossy()
            .into_owned();
        assert!(
            texto.contains(&nombre),
            "el mensaje nombra el archivo: {texto}"
        );
    }

    #[test]
    fn oraculo_test_set_y_test_get_con_el_mismo_nombre_cuentan_dos() {
        let coleccion: &'static str = "tests/test_set.py::TestBasico::test_vacio\n\
             tests/test_get.py::TestBasico::test_vacio\n2 tests collected in 0.01s\n";
        let xml = r#"<testsuite>
  <testcase classname="tests.test_set.TestBasico" name="test_vacio"><failure message="x"/></testcase>
  <testcase classname="tests.test_get.TestBasico" name="test_vacio"/>
</testsuite>"#;
        let base = tempfile::tempdir().expect("base");
        let tarea = tarea_de_prueba(base.path());
        let correr = ejecutor_con(coleccion, Some(xml), 1);
        let s = correr_suite_de_prueba(base.path(), &tarea, &correr).expect("sin infra");
        let Suite::Clasificada(c) = s else {
            panic!("debía clasificar: {s:?}");
        };
        assert_eq!(c.conteo, Conteo { pasa: 1, total: 2 });
        assert!(c.estables.contains_key("test_set::TestBasico::test_vacio"));
        assert!(c.estables.contains_key("test_get::TestBasico::test_vacio"));
    }

    #[test]
    fn oraculo_contrato_no_monta_la_tarea_entera_ni_oculto() {
        let base = tempfile::tempdir().expect("base");
        let tarea = tarea_de_prueba(base.path());
        let repo = base.path().join("repo");
        let salida = base.path().join("salida");
        fs::create_dir_all(&repo).expect("repo");
        fs::create_dir_all(&salida).expect("salida");
        let o = OpcionesOraculo::default();
        let c = Corrida {
            repo: &repo,
            tarea: &tarea,
            salida: &salida,
            o: &o,
            cache: None,
            rustsec: None,
            home: base.path(),
        };
        let e = ejecucion_de_suite(&c, &tarea.contrato.cmd, DIR_CONTRATO);
        let args = contenedor::argumentos(&e, "n");
        assert!(
            !args
                .iter()
                .any(|a| a.starts_with(&format!("{}:/oraculo:", base.path().display()))),
            "no monta la tarea en /oraculo: {args:?}"
        );
        let montajes: Vec<&String> = args
            .windows(2)
            .filter(|p| p[0] == "-v")
            .map(|p| &p[1])
            .collect();
        assert!(
            montajes
                .iter()
                .any(|m| m.ends_with(":/oraculo/contrato:ro,z")),
            "monta solo contrato: {montajes:?}"
        );
        assert!(
            !montajes.iter().any(|m| m.contains("oculto")),
            "{montajes:?}"
        );
    }

    #[test]
    fn oraculo_redaccion_mcp_quita_ids_de_tests_ocultos() {
        let mut v = Veredicto {
            inestables: vec!["test_a::t1".into()],
            inestables_contrato: vec!["test_c::t9".into()],
            pendientes: vec![crate::tipos::Pendiente {
                test: "test_b::x".into(),
                motivo: "gpu".into(),
                colab: None,
                tasa: None,
            }],
            avisos: vec!["aviso sin ids".into()],
            ..Veredicto::default()
        };
        v.tasas.insert(
            "oculto:test_a::t1".into(),
            crate::tipos::TasaTest {
                pasa: 1,
                total: 2,
                wilson_inf: 0.1,
            },
        );
        v.hallazgos.push(Hallazgo {
            dimension: crate::tipos::Dimension::Seguridad,
            severidad: 2,
            archivo: "src/lib.rs".into(),
            linea: 1,
            cita: "test_a::t1".into(),
            detalle: "falla test_a::t1".into(),
            fuente: crate::tipos::Fuente::Sonda("x".into()),
            id: String::new(),
            impacto: Vec::new(),
        });
        v.hallazgos.push(Hallazgo {
            dimension: crate::tipos::Dimension::Seguridad,
            severidad: 1,
            archivo: "src/lib.rs".into(),
            linea: 2,
            cita: "unwrap()".into(),
            detalle: "unwrap sin control".into(),
            fuente: crate::tipos::Fuente::Sonda("x".into()),
            id: String::new(),
            impacto: Vec::new(),
        });
        let r = redactar_para_mcp(v);
        let texto = serde_json::to_string(&r).expect("json");
        assert!(!texto.contains("test_a::t1"), "{texto}");
        assert!(!texto.contains("test_b::x"), "{texto}");
        assert!(
            texto.contains("test_c::t9"),
            "el contrato no está oculto: {texto}"
        );
        assert!(
            texto.contains("unwrap sin control"),
            "un hallazgo sin ids queda: {texto}"
        );
        assert_eq!(r.hallazgos.len(), 1);
    }

    #[test]
    fn temporales_limpieza_borra_colab_viejas_y_conserva_recientes() {
        let home = tempfile::tempdir().expect("temp dir");
        let tmp = home.path().join("tmp");
        let viejo = tmp.join("colab-viejo");
        let reciente = tmp.join("colab-reciente");
        fs::create_dir_all(&viejo).expect("dir");
        fs::write(viejo.join("pendientes.xml"), "x").expect("escribe");
        fs::create_dir_all(&reciente).expect("dir");
        let ahora = SystemTime::now();
        poner_mtime(&viejo, ahora - Duration::from_secs(25 * 3600));
        limpiar_temporales(&tmp, ahora);
        assert!(!viejo.exists(), "la carpeta colab de más de 24 h se borra");
        assert!(reciente.exists(), "la reciente se conserva");
    }

    #[test]
    fn oraculo_colab_de_otro_commit_no_entra_al_veredicto() {
        let entrada = |commit: &str| pendientes::ColabEntrada {
            resultado: "pasa".into(),
            junit_sha256: "x".into(),
            ts: 1.0,
            commit: commit.into(),
            tasa: None,
            confianza: None,
        };
        let colab = BTreeMap::from([
            ("test_a::uno".to_string(), entrada("abc")),
            ("test_a::dos".to_string(), entrada("vieja")),
        ]);
        let (del_commit, aviso) = filtrar_colab_por_commit(colab, "abc");
        assert_eq!(del_commit.keys().collect::<Vec<_>>(), vec!["test_a::uno"]);
        assert_eq!(
            aviso.as_deref(),
            Some("1 resultados de Colab de otro commit no entran al veredicto")
        );
        let (todos, sin_aviso) = filtrar_colab_por_commit(
            BTreeMap::from([("test_a::uno".to_string(), entrada("abc"))]),
            "abc",
        );
        assert_eq!(todos.len(), 1);
        assert_eq!(sin_aviso, None);
    }

    // --- Tarea 13: evaluación completa con dependencias falsas (sin podman ni red) ---

    /// Spec con ficha válida para las tareas de prueba.
    const SPEC: &str = include_str!("../../tests/fixtures/fichas/spec-completa.md");
    /// Cargo.lock de un repo sin dependencias externas: solo su paquete.
    const LOCK_SANO: &str = "version = 4\n\n[[package]]\nname = \"x\"\nversion = \"0.1.0\"\n";
    /// Cargo.lock que no parsea: el inventario lo rechaza antes de cualquier red o contenedor.
    const LOCK_ROTO: &str = "version = 4\n[[package\n";
    const COLECCION_CONTRATO: &str =
        "tests/test_a.py::t1\ntests/test_a.py::t2\n2 tests collected\n";
    const JUNIT_CONTRATO: &str = r#"<testsuite><testcase classname="tests.test_a" name="t1"/><testcase classname="tests.test_a" name="t2"/></testsuite>"#;
    /// Contrato con la primera prueba fallando: el memora-sdlc falso de la prueba de calibración.
    const JUNIT_CONTRATO_CON_FALLA: &str = r#"<testsuite><testcase classname="tests.test_a" name="t1"><failure message="x"/></testcase><testcase classname="tests.test_a" name="t2"/></testsuite>"#;
    const MODELO_JUEZ: &str = "juez-falso";
    const MODULARIDAD_ID: &str = "diseno.modularidad";
    const TOPE_ID: &str = "diseno.tope";
    const JUNIT_CONTRATO_INCOMPLETO: &str =
        r#"<testsuite><testcase classname="tests.test_a" name="t1"/></testsuite>"#;
    const COLECCION_OCULTO: &str = "tests/test_b.py::t3\ntests/test_b.py::t4\n2 tests collected\n";
    const JUNIT_OCULTO: &str = r#"<testsuite><testcase classname="tests.test_b" name="t3"/><testcase classname="tests.test_b" name="t4"><failure message="x"/></testcase></testsuite>"#;
    /// Salida de `cargo mutants`: un mutante cazado de dos viables, con la línea de fin del Baseline.
    const OUTCOMES: &str = r#"{"outcomes":[{"scenario":"Baseline","summary":"Success"},{"scenario":{"Mutant":{"name":"m","file":"src/lib.rs","span":{"start":{"line":1}}}},"summary":"CaughtMutant"}],"total_mutants":1}"#;

    /// Tarea de Rust con dos requisitos, eficiencia y su spec. Los tests de la oculta son `test_b::t3` y `t4`.
    fn tarea_rust(base: &Path) -> Tarea {
        let dir = base.join("tarea");
        fs::create_dir_all(&dir).expect("tarea");
        fs::write(dir.join("spec.md"), SPEC).expect("spec");
        let texto = "nombre = \"tarea-rust\"\nlenguaje = \"rust\"\nimagen = \"imagen-falsa\"\n\
            spec = \"spec.md\"\nrepeticiones_inestable = 1\n\
            [contrato]\ncmd = \"pytest -q --junitxml=/salida/contrato.xml /oraculo/contrato\"\n\
            [oculto]\ncmd = \"pytest -q --junitxml=/salida/oculto.xml /oraculo/oculto\"\n\
            [eficiencia]\ncmd = \"bench-eficiencia\"\npiso = 5000.0\n\
            [[requisito]]\nid = \"R1\"\ntitulo = \"Uno\"\npeso = 3\npruebas = [\"test_b::t3\"]\n\
            [[requisito]]\nid = \"R2\"\ntitulo = \"Dos\"\npeso = 1\npruebas = [\"test_b::t4\"]\n";
        fs::write(dir.join("tarea.toml"), texto).expect("tarea.toml");
        Tarea::cargar_de(&dir).expect("carga la tarea")
    }

    /// Repo de Rust con el `Cargo.lock` dado.
    fn repo_rust(base: &Path, lock: &str) -> PathBuf {
        let repo = base.join("repo");
        fs::create_dir_all(repo.join("src")).expect("src");
        fs::write(repo.join("src/lib.rs"), "pub fn f() -> u8 { 1 }\n").expect("lib.rs");
        fs::write(
            repo.join("Cargo.toml"),
            "[package]\nname = \"x\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .expect("Cargo.toml");
        fs::write(repo.join("Cargo.lock"), lock).expect("Cargo.lock");
        repo
    }

    /// `ROOT_HOME` con la caché de dependencias del lock y la base de RustSec ya listas. Así la
    /// descarga no levanta contenedores ni baja nada.
    fn home_con_cache(base: &Path, lock: &str) -> PathBuf {
        let home = base.join("home");
        let hash = descarga::hash_corto(lock.as_bytes());
        let cache = home.join("cache/rust").join(&hash);
        fs::create_dir_all(&cache).expect("cache de Rust");
        fs::write(cache.join("config.toml"), "").expect("config.toml");
        fs::create_dir_all(home.join("cache/rustsec")).expect("base de RustSec");
        home
    }

    /// Ejecutor de contenedores de las suites: responde la recolección y deja el junit de cada una.
    fn correr_suites(
        junit_contrato: &'static str,
    ) -> impl Fn(&contenedor::Ejecucion) -> Result<contenedor::Resultado, ErrorOraculo> {
        move |e: &contenedor::Ejecucion| {
            let contrato = e.oraculo_destino.as_deref() == Some(DIR_CONTRATO);
            let (coleccion, junit, xml) = if contrato {
                (COLECCION_CONTRATO, junit_contrato, XML_CONTRATO)
            } else {
                (COLECCION_OCULTO, JUNIT_OCULTO, XML_OCULTO)
            };
            if e.cmd.contains("--collect-only") {
                fs::write(e.salida.join(ARCHIVO_COLECCION), coleccion).expect("coleccion");
                return Ok(resultado_falso(0, ""));
            }
            fs::write(e.salida.join(xml), junit).expect("junit");
            let codigo = if junit.contains("<failure") { 1 } else { 0 };
            Ok(resultado_falso(codigo, ""))
        }
    }

    /// Ejecutor de suites que no debe usarse: si la evaluación corre una, el test falla.
    fn sin_suites(_: &contenedor::Ejecucion) -> Result<contenedor::Resultado, ErrorOraculo> {
        panic!("la descarga no debía dejar correr ninguna suite");
    }

    /// Ejecutor de herramientas falso. Responde como una imagen sana, salvo las de `faltan`.
    struct HerramientasSanas {
        faltan: Vec<&'static str>,
        /// Si `true`, gitleaks reporta una filtración de secreto.
        fuga: bool,
    }

    impl EjecutorHerramienta for HerramientasSanas {
        fn ejecutar(&self, cmd: &str) -> Result<SalidaHerramienta, ErrorOraculo> {
            let mut salida = SalidaHerramienta::default();
            if let Some(nombre) = cmd.strip_prefix("command -v ") {
                salida.codigo = i32::from(self.faltan.contains(&nombre));
            } else if cmd == "cargo clippy --version" {
                salida.codigo = i32::from(self.faltan.contains(&"clippy"));
            } else if cmd.contains("gitleaks detect") {
                let reporte = if self.fuga {
                    r#"[{"RuleID":"generic-api-key","File":"src/lib.rs","StartLine":1,"Description":"clave en el código"}]"#
                } else {
                    "[]"
                };
                salida
                    .archivos
                    .insert("gitleaks.json".into(), reporte.into());
                salida.stderr = "no leaks found".into();
            } else if cmd.contains("cargo audit") {
                salida.archivos.insert(
                    "audit.json".into(),
                    r#"{"vulnerabilities":{"found":false,"list":[]}}"#.into(),
                );
            } else if cmd.contains("--message-format=json") {
                salida.archivos.insert(
                    "clippy.json".into(),
                    r#"{"reason":"build-finished","success":true}"#.into(),
                );
            } else if cmd == "bench-eficiencia" {
                salida.stdout = r#"{"ops_s": 6000.0}"#.into();
            } else if cmd.contains("cargo mutants") {
                salida
                    .archivos
                    .insert("outcomes.json".into(), OUTCOMES.into());
            }
            Ok(salida)
        }

        fn tiene_rustsec(&self) -> bool {
            true
        }
    }

    /// Fábrica de herramientas falsas: no necesita la copia del repo.
    fn herramientas(
        faltan: &'static [&'static str],
    ) -> impl Fn(&Preparada) -> Box<dyn EjecutorHerramienta> {
        move |_: &Preparada| {
            Box::new(HerramientasSanas {
                faltan: faltan.to_vec(),
                fuga: false,
            })
        }
    }

    /// Fábrica de herramientas con una filtración de secreto en gitleaks.
    fn herramientas_con_fuga() -> impl Fn(&Preparada) -> Box<dyn EjecutorHerramienta> {
        |_: &Preparada| {
            Box::new(HerramientasSanas {
                faltan: Vec::new(),
                fuga: true,
            })
        }
    }

    fn opciones(home: &Path) -> OpcionesOraculo {
        OpcionesOraculo {
            timeout: Duration::from_secs(60),
            home: Some(home.to_path_buf()),
        }
    }

    /// Evaluación completa con las dependencias falsas: sin juez de configuración.
    fn evaluar_falso(
        repo: &Path,
        tarea: &Tarea,
        home: &Path,
        correr: &dyn Fn(&contenedor::Ejecucion) -> Result<contenedor::Resultado, ErrorOraculo>,
        herramientas: &dyn Fn(&Preparada) -> Box<dyn EjecutorHerramienta>,
    ) -> Result<Veredicto, ErrorOraculo> {
        evaluar_con(repo, tarea, &opciones(home), deps_de(correr, herramientas))
    }

    /// Veredicto de la tarea de Rust con todas las sondas sanas y la suite del contrato completa.
    fn veredicto_sano(base: &Path) -> Veredicto {
        let tarea = tarea_rust(base);
        let repo = repo_rust(base, LOCK_SANO);
        let home = home_con_cache(base, LOCK_SANO);
        let correr = correr_suites(JUNIT_CONTRATO);
        evaluar_falso(&repo, &tarea, &home, &correr, &herramientas(&[])).expect("evalúa")
    }

    fn puntaje_de_prueba(v: &Veredicto, dimension: Dimension) -> &Puntaje {
        v.puntajes
            .iter()
            .find(|p| p.dimension == dimension)
            .expect("la dimensión está")
    }

    #[test]
    fn oraculo_veredicto_lista_nueve_dimensiones_en_orden_de_dimension() {
        let base = tempfile::tempdir().expect("base");
        let v = veredicto_sano(base.path());
        let dims: Vec<Dimension> = v.puntajes.iter().map(|p| p.dimension).collect();
        assert_eq!(
            dims,
            vec![
                Dimension::Funciona,
                Dimension::Seguridad,
                Dimension::Eficiencia,
                Dimension::Estructura,
                Dimension::Arquitectura,
                Dimension::Diseno,
                Dimension::Completitud,
                Dimension::Entrega,
                Dimension::Pruebas,
            ]
        );
        assert_eq!(DIMENSIONES.to_vec(), dims);
    }

    #[test]
    fn oraculo_con_sondas_sanas_solo_diseno_y_arquitectura_quedan_sin_veredicto() {
        // El repo de la prueba tiene un solo `src/lib.rs`: sin módulos, Arquitectura no tiene análisis (Tarea 28).
        let base = tempfile::tempdir().expect("base");
        let v = veredicto_sano(base.path());
        let sin: Vec<Dimension> = v
            .puntajes
            .iter()
            .filter(|p| p.sin_veredicto.is_some())
            .map(|p| p.dimension)
            .collect();
        assert_eq!(
            sin,
            vec![Dimension::Arquitectura, Dimension::Diseno],
            "{:?}",
            v.puntajes
        );
        assert_eq!(v.global, None);
    }

    #[test]
    fn oraculo_diseno_sin_rubricas_embebidas_queda_sin_veredicto() {
        let base = tempfile::tempdir().expect("base");
        let v = veredicto_sano(base.path());
        let diseno = puntaje_de_prueba(&v, Dimension::Diseno);
        assert_eq!(
            diseno.sin_veredicto.as_deref(),
            Some("sin rúbricas embebidas")
        );
        assert_eq!(diseno.valor, 0.0);
    }

    #[test]
    fn oraculo_diseno_sin_juez_con_rubricas_dice_juez_no_corrido() {
        let base = tempfile::tempdir().expect("base");
        let tarea = tarea_rust(base.path());
        let repo = repo_rust(base.path(), LOCK_SANO);
        let home = home_con_cache(base.path(), LOCK_SANO);
        let rubricas = [rubrica_diseno()];
        let correr = correr_suites(JUNIT_CONTRATO);
        let v = evaluar_con(
            &repo,
            &tarea,
            &opciones(&home),
            Dependencias {
                rubricas: &rubricas,
                ..deps_de(&correr, &herramientas(&[]))
            },
        )
        .expect("evalúa");
        let diseno = puntaje_de_prueba(&v, Dimension::Diseno);
        assert_eq!(diseno.sin_veredicto.as_deref(), Some("juez no corrido"));
    }

    #[test]
    fn oraculo_sonda_sin_veredicto_deja_su_dimension_sin_veredicto_y_valor_cero() {
        let base = tempfile::tempdir().expect("base");
        let tarea = tarea_rust(base.path());
        let repo = repo_rust(base.path(), LOCK_SANO);
        let home = home_con_cache(base.path(), LOCK_SANO);
        let correr = correr_suites(JUNIT_CONTRATO);
        let v = evaluar_falso(
            &repo,
            &tarea,
            &home,
            &correr,
            &herramientas(&["cargo-mutants"]),
        )
        .expect("evalúa");
        let pruebas = puntaje_de_prueba(&v, Dimension::Pruebas);
        let motivo = pruebas.sin_veredicto.as_deref().expect("sin veredicto");
        assert!(motivo.contains("cargo-mutants"), "{motivo}");
        assert_eq!(pruebas.valor, 0.0);
        assert_eq!(v.puntajes.len(), 9, "las nueve siguen listadas");
    }

    #[test]
    fn oraculo_requisitos_traen_un_estado_por_requisito_de_la_tarea() {
        let base = tempfile::tempdir().expect("base");
        let v = veredicto_sano(base.path());
        let ids: Vec<&str> = v.requisitos.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["R1", "R2"]);
        assert_eq!(v.requisitos[0].cumplimiento, Cumplimiento::Cumplido);
        assert_eq!(v.requisitos[1].cumplimiento, Cumplimiento::Ausente);
    }

    #[test]
    fn oraculo_funciona_con_3_de_4_pasando_y_sin_escenarios_da_075() {
        let base = tempfile::tempdir().expect("base");
        let v = veredicto_sano(base.path());
        let funciona = puntaje_de_prueba(&v, Dimension::Funciona);
        assert_eq!(funciona.sin_veredicto, None);
        assert!(
            (funciona.valor - 0.75).abs() < 1e-6,
            "valor {}",
            funciona.valor
        );
    }

    #[test]
    fn oraculo_funciona_no_cuenta_los_tests_de_entorno() {
        let base = tempfile::tempdir().expect("base");
        let mut tarea = tarea_de_prueba(base.path());
        tarea.entorno = vec![Exclusion {
            test: "test_a::t2".into(),
            motivo: "gpu".into(),
        }];
        let contrato = Clasificacion {
            estables: BTreeMap::from([
                ("test_a::t1".to_string(), ResultadoTest::Pasa),
                ("test_a::t2".to_string(), ResultadoTest::Falla),
            ]),
            ..Clasificacion::default()
        };
        let oculto = Clasificacion {
            estables: BTreeMap::from([("test_b::t3".to_string(), ResultadoTest::Pasa)]),
            ..Clasificacion::default()
        };
        let p = funciona(&tarea, &contrato, &oculto, &BTreeMap::new());
        assert!(
            (p.valor - 1.0).abs() < 1e-6,
            "sin el de entorno quedan 2 de 2: {}",
            p.valor
        );
    }

    #[test]
    fn oraculo_funciona_escenario_sin_medir_deja_la_dimension_sin_veredicto() {
        let base = tempfile::tempdir().expect("base");
        let mut tarea = tarea_de_prueba(base.path());
        tarea.escenario = vec![Escenario {
            id: "e2e".into(),
            cmd: "exit 0".into(),
        }];
        let contrato = Clasificacion {
            estables: BTreeMap::from([("test_a::t1".to_string(), ResultadoTest::Pasa)]),
            ..Clasificacion::default()
        };
        let p = funciona(
            &tarea,
            &contrato,
            &Clasificacion::default(),
            &BTreeMap::new(),
        );
        let motivo = p.sin_veredicto.expect("sin veredicto");
        assert!(motivo.contains("e2e"), "{motivo}");
    }

    #[test]
    fn oraculo_funciona_con_escenarios_promedia_con_correccion() {
        let base = tempfile::tempdir().expect("base");
        let mut tarea = tarea_de_prueba(base.path());
        tarea.escenario = vec![Escenario {
            id: "e2e".into(),
            cmd: "exit 0".into(),
        }];
        let contrato = Clasificacion {
            estables: BTreeMap::from([("test_a::t1".to_string(), ResultadoTest::Pasa)]),
            ..Clasificacion::default()
        };
        let escenarios = BTreeMap::from([("e2e".to_string(), false)]);
        let p = funciona(&tarea, &contrato, &Clasificacion::default(), &escenarios);
        assert!((p.valor - 0.5).abs() < 1e-6, "(1 + 0) / 2: {}", p.valor);
    }

    #[test]
    fn oraculo_descarga_sin_veredicto_deja_las_nueve_sin_veredicto() {
        let base = tempfile::tempdir().expect("base");
        let tarea = tarea_rust(base.path());
        let repo = repo_rust(base.path(), LOCK_ROTO);
        let home = base.path().join("home");
        let v = evaluar_falso(&repo, &tarea, &home, &sin_suites, &herramientas(&[]))
            .expect("la descarga sin veredicto no es error de infraestructura");
        assert_eq!(v.puntajes.len(), 9);
        for p in &v.puntajes {
            assert_eq!(p.valor, 0.0, "{:?}", p.dimension);
            let motivo = p.sin_veredicto.as_deref().expect("sin veredicto");
            assert!(motivo.contains("descarga"), "{motivo}");
        }
        assert!(
            v.hallazgos
                .iter()
                .any(|h| h.dimension == Dimension::Entrega)
        );
    }

    #[test]
    fn oraculo_suite_sin_veredicto_deja_las_nueve_sin_veredicto() {
        let base = tempfile::tempdir().expect("base");
        let tarea = tarea_rust(base.path());
        let repo = repo_rust(base.path(), LOCK_SANO);
        let home = home_con_cache(base.path(), LOCK_SANO);
        // El junit trae 1 test y pytest recolectó 2: el contrato no da veredicto.
        let correr = correr_suites(JUNIT_CONTRATO_INCOMPLETO);
        let v = evaluar_falso(&repo, &tarea, &home, &correr, &herramientas(&[])).expect("evalúa");
        assert_eq!(v.puntajes.len(), 9);
        assert!(v.puntajes.iter().all(|p| p.sin_veredicto.is_some()));
        let motivo = v.puntajes[0].sin_veredicto.clone().unwrap_or_default();
        assert!(motivo.contains("contrato"), "{motivo}");
    }

    #[test]
    fn oraculo_veredicto_lleva_las_nueve_etiquetas_de_rubrica() {
        let base = tempfile::tempdir().expect("base");
        let tarea = tarea_rust(base.path());
        let repo = repo_rust(base.path(), LOCK_SANO);
        let home = home_con_cache(base.path(), LOCK_SANO);
        let correr = correr_suites(JUNIT_CONTRATO);
        let embebidas = rubrica::cargar_embebidas().expect("rúbricas");
        let fabrica = herramientas(&[]);
        let deps = Dependencias {
            rubricas: &embebidas,
            ..deps_de(&correr, &fabrica)
        };
        let v = evaluar_con(&repo, &tarea, &opciones(&home), deps).expect("evalúa");
        assert_eq!(v.rubricas.len(), 9, "{:?}", v.rubricas);
        assert_eq!(v.rubricas, rubrica::etiquetas(&embebidas));
    }

    #[test]
    fn oraculo_redaccion_mcp_quita_los_ids_de_ocultos_fallidos() {
        let informe = crate::tipos::Informe {
            decision: crate::tipos::Decision::NoListo,
            motivo: "test_oculto_fuera_de_requisitos: test_b::t9".into(),
            requisitos: Vec::new(),
            riesgos: Vec::new(),
            proximos_pasos: Vec::new(),
            bloqueantes: Vec::new(),
            resumen: "Resumen sin ids de tests.".into(),
            confianza: crate::tipos::Confianza::default(),
        };
        let v = Veredicto {
            ocultos_fallidos: vec!["test_b::t9".into()],
            informe: Some(informe),
            ..Veredicto::default()
        };
        let r = redactar_para_mcp(v);
        let texto = serde_json::to_string(&r).expect("json");
        assert!(!texto.contains("test_b::t9"), "{texto}");
        assert!(r.ocultos_fallidos.is_empty());
    }

    #[test]
    fn oraculo_redaccion_mcp_quita_evidencia_de_requisitos_y_motivos_con_ids_ocultos() {
        let v = Veredicto {
            inestables: vec!["test_b::t4".into()],
            requisitos: vec![EstadoRequisito {
                id: "R1".into(),
                titulo: "Uno".into(),
                peso: 3,
                cumplimiento: Cumplimiento::Parcial,
                evidencia: vec!["test_b::t3: pasa".into()],
                nivel: Some(1),
            }],
            puntajes: vec![
                sin_veredicto_puntaje(Dimension::Pruebas, "falló test_b::t4".into()),
                sin_veredicto_puntaje(Dimension::Entrega, "no instala".into()),
            ],
            ..Veredicto::default()
        };
        let r = redactar_para_mcp(v);
        let texto = serde_json::to_string(&r).expect("json");
        assert!(!texto.contains("test_b::t3"), "{texto}");
        assert!(!texto.contains("test_b::t4"), "{texto}");
        assert!(
            texto.contains("no instala"),
            "un motivo sin ids queda: {texto}"
        );
    }

    /// Tabla de calibración con la misma exactitud en todas las dimensiones de `tarea-rust`.
    static SIN_CALIBRAR: Calibraciones = Calibraciones {
        v: calibra::VERSION,
        filas: Vec::new(),
    };

    /// Dependencias sin juez, sin calibración y sin rúbricas: las de una evaluación sin nada extra.
    fn deps_de<'a>(correr: Correr<'a>, herramientas: FabricaHerramientas<'a>) -> Dependencias<'a> {
        Dependencias {
            correr,
            herramientas,
            juez: None,
            modelo_juez: "",
            juez_config: None,
            calibracion: &SIN_CALIBRAR,
            rubricas: &[],
            reglas: ReglasDecision::default(),
            redactor: None,
            avisos: Vec::new(),
            podman: &PodmanNoUsado,
            escenarios: &FabricaSinContenedores,
        }
    }

    /// Juez falso: responde un texto fijo con su costo y cuenta las llamadas.
    struct JuezFalso {
        texto: String,
        costo: f64,
        llamadas: Cell<u32>,
    }

    impl JuezFalso {
        fn con(texto: &str, costo: f64) -> Self {
            Self {
                texto: texto.into(),
                costo,
                llamadas: Cell::new(0),
            }
        }

        fn llamadas(&self) -> u32 {
            self.llamadas.get()
        }
    }

    impl ModeloJuez for JuezFalso {
        fn consultar(&self, _prompt: &Prompt) -> Result<RespuestaJuez, ErrorOraculo> {
            self.llamadas.set(self.llamadas.get() + 1);
            Ok(RespuestaJuez {
                texto: self.texto.clone(),
                truncada: false,
                costo_usd: self.costo,
            })
        }
    }

    /// Rúbrica de diseño de la fixture: `diseno.modularidad` (juez), `diseno.ciclos` (sonda) y `diseno.tope` (mixto).
    fn rubrica_diseno() -> Rubrica {
        Rubrica::desde_toml(&fixture_rubrica("diseno-valida.toml")).expect("carga")
    }

    /// La fixture con `diseno.modularidad` como `mixto`, para probar el tope de los ciclos.
    fn rubrica_diseno_mixta() -> Rubrica {
        let texto =
            fixture_rubrica("diseno-valida.toml").replace("mide = \"juez\"", "mide = \"mixto\"");
        Rubrica::desde_toml(&texto).expect("carga")
    }

    fn fixture_rubrica(nombre: &str) -> String {
        let ruta = format!(
            "{}/tests/fixtures/rubricas/{nombre}",
            env!("CARGO_MANIFEST_DIR")
        );
        fs::read_to_string(ruta).expect("lee la fixture")
    }

    /// Repo de Rust en `dir` con el `Cargo.lock` dado y los archivos `(ruta, texto)`.
    fn repo_con(dir: PathBuf, lock: &str, archivos: &[(&str, &str)]) -> PathBuf {
        fs::create_dir_all(dir.join("src")).expect("src");
        fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"x\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .expect("Cargo.toml");
        fs::write(dir.join("Cargo.lock"), lock).expect("Cargo.lock");
        for (ruta, texto) in archivos {
            fs::write(dir.join(ruta), texto).expect("archivo");
        }
        dir
    }

    /// Repo con `.cargo/config.toml` de una entrada que no toca la red (`[build] jobs`).
    fn repo_con_config(base: &Path) -> PathBuf {
        let repo = repo_rust(base, LOCK_SANO);
        fs::create_dir_all(repo.join(".cargo")).expect(".cargo");
        fs::write(repo.join(".cargo/config.toml"), "[build]\njobs = 2\n").expect("config");
        repo
    }

    /// Evidencia que cita la línea 1 de `src/lib.rs` del repo de `repo_rust`.
    fn ev_lib() -> Value {
        json!({"archivo": "src/lib.rs", "linea": 1, "cita": "pub fn f() -> u8 { 1 }"})
    }

    fn item_juez(id: &str, nivel: i64, evidencia: Value) -> Value {
        json!({"item": id, "nivel": nivel, "evidencia": evidencia})
    }

    fn hallazgo_juez(item: &str, archivo: &str, linea: u32, cita: &str, detalle: &str) -> Value {
        json!({
            "item": item,
            "severidad": 2,
            "archivo": archivo,
            "linea": linea,
            "cita": cita,
            "detalle": detalle,
            "impacto": ["R1"],
        })
    }

    fn respuesta_juez(items: Value, hallazgos: Value) -> String {
        json!({"items": items, "hallazgos": hallazgos}).to_string()
    }

    /// Evaluación de la tarea de Rust con `deps` sobre un repo y una caché de `base`.
    fn evaluar_con_deps(base: &Path, repo: &Path, deps: Dependencias) -> Veredicto {
        let tarea = tarea_rust(base);
        let home = home_con_cache(base, LOCK_SANO);
        evaluar_con(repo, &tarea, &opciones(&home), deps).expect("evalúa")
    }

    #[test]
    fn oraculo_diseno_con_juez_que_cita_tiene_valor_y_no_queda_sin_veredicto() {
        let base = tempfile::tempdir().expect("base");
        let repo = repo_rust(base.path(), LOCK_SANO);
        let juez = JuezFalso::con(
            &respuesta_juez(
                json!([
                    item_juez(MODULARIDAD_ID, 2, json!([ev_lib()])),
                    item_juez(TOPE_ID, 2, json!([ev_lib()])),
                ]),
                json!([]),
            ),
            0.25,
        );
        let rubricas = [rubrica_diseno()];
        let correr = correr_suites(JUNIT_CONTRATO);
        let v = evaluar_con_deps(
            base.path(),
            &repo,
            Dependencias {
                juez: Some(&juez),
                modelo_juez: MODELO_JUEZ,
                rubricas: &rubricas,
                ..deps_de(&correr, &herramientas(&[]))
            },
        );
        let diseno = puntaje_de_prueba(&v, Dimension::Diseno);
        assert_eq!(diseno.sin_veredicto, None, "{diseno:?}");
        // (3·2 + 1·2) / (3·4) = 2/3
        assert!((diseno.valor - 2.0 / 3.0).abs() < 1e-6, "{}", diseno.valor);
        assert_eq!(juez.llamadas(), 1);
        assert!((v.costo_usd - 0.25).abs() < 1e-9, "{}", v.costo_usd);
    }

    #[test]
    fn oraculo_diseno_item_del_juez_sin_respuesta_no_baja_la_dimension() {
        // El juez responde solo `diseno.modularidad` (peso 3, nivel 2). `diseno.tope` (mixto, peso 1)
        // queda en `faltantes`: no vale 0. Diseño es 2/3, el valor del ítem respondido.
        let r = ResultadoJuez {
            estado: EstadoJuez::Ok,
            niveles: BTreeMap::from([(MODULARIDAD_ID.to_string(), 2)]),
            items: Vec::new(),
            hallazgos: Vec::new(),
            descartes: Vec::new(),
            faltantes: vec![TOPE_ID.to_string()],
        };
        let rubricas = [rubrica_diseno()];
        let juez = JuezDiseno {
            modelo: None,
            nombre: MODELO_JUEZ,
            rubricas: &rubricas,
        };
        let p = puntaje_diseno(&juez, Some(&r), &BTreeMap::new());
        assert_eq!(p.sin_veredicto, None, "{p:?}");
        let valor = p.valor;
        assert!((valor - 2.0 / 3.0).abs() < 1e-6, "{valor}");
    }

    #[test]
    fn oraculo_diseno_con_juez_de_json_roto_queda_sin_veredicto_y_nunca_verde() {
        let base = tempfile::tempdir().expect("base");
        let repo = repo_rust(base.path(), LOCK_SANO);
        let juez = JuezFalso::con("esto no es json", 0.0);
        let rubricas = [rubrica_diseno()];
        let correr = correr_suites(JUNIT_CONTRATO);
        let v = evaluar_con_deps(
            base.path(),
            &repo,
            Dependencias {
                juez: Some(&juez),
                modelo_juez: MODELO_JUEZ,
                rubricas: &rubricas,
                ..deps_de(&correr, &herramientas(&[]))
            },
        );
        let diseno = puntaje_de_prueba(&v, Dimension::Diseno);
        let causa = diseno.sin_veredicto.as_deref().expect("sin veredicto");
        assert!(causa.contains("JSON inválido"), "{causa}");
        assert_eq!(diseno.valor, 0.0);
        assert_ne!(v.estado, Estado::Verde);
        assert_eq!(v.global, None);
    }

    #[test]
    fn oraculo_hallazgo_del_juez_con_cita_inventada_no_entra_al_veredicto() {
        let base = tempfile::tempdir().expect("base");
        let repo = repo_rust(base.path(), LOCK_SANO);
        let texto = respuesta_juez(
            json!([item_juez(MODULARIDAD_ID, 2, json!([ev_lib()]))]),
            json!([
                hallazgo_juez(
                    MODULARIDAD_ID,
                    "src/lib.rs",
                    1,
                    "pub fn f() -> u8 { 1 }",
                    "hallazgo válido"
                ),
                hallazgo_juez(
                    MODULARIDAD_ID,
                    "src/lib.rs",
                    1,
                    "fn inventada() -> bool { false }",
                    "hallazgo inventado"
                ),
            ]),
        );
        let juez = JuezFalso::con(&texto, 0.0);
        let rubricas = [rubrica_diseno()];
        let correr = correr_suites(JUNIT_CONTRATO);
        let v = evaluar_con_deps(
            base.path(),
            &repo,
            Dependencias {
                juez: Some(&juez),
                modelo_juez: MODELO_JUEZ,
                rubricas: &rubricas,
                ..deps_de(&correr, &herramientas(&[]))
            },
        );
        assert!(
            v.hallazgos
                .iter()
                .any(|h| h.detalle == "hallazgo válido" && h.dimension == Dimension::Diseno),
            "{:?}",
            v.hallazgos
        );
        assert!(
            !v.hallazgos
                .iter()
                .any(|h| h.detalle == "hallazgo inventado"),
            "{:?}",
            v.hallazgos
        );
    }

    #[test]
    fn oraculo_mixto_de_diseno_con_ciclos_no_supera_el_tope() {
        let base = tempfile::tempdir().expect("base");
        let repo = repo_con(
            base.path().join("ciclo"),
            LOCK_SANO,
            &[
                ("src/lib.rs", "pub fn f() -> u8 { 1 }\n"),
                ("src/a.rs", "use crate::b::algo;\npub fn f() {}\n"),
                ("src/b.rs", "pub fn algo() {}\nuse crate::a::f;\n"),
            ],
        );
        let texto = respuesta_juez(
            json!([
                item_juez(
                    MODULARIDAD_ID,
                    3,
                    json!([{"archivo": "src/a.rs", "linea": 1, "cita": "use crate::b::algo;"}])
                ),
                item_juez(
                    TOPE_ID,
                    3,
                    json!([{"archivo": "src/b.rs", "linea": 1, "cita": "pub fn algo() {}"}])
                ),
            ]),
            json!([]),
        );
        let juez = JuezFalso::con(&texto, 0.0);
        let rubricas = [rubrica_diseno_mixta()];
        let correr = correr_suites(JUNIT_CONTRATO);
        let v = evaluar_con_deps(
            base.path(),
            &repo,
            Dependencias {
                juez: Some(&juez),
                modelo_juez: MODELO_JUEZ,
                rubricas: &rubricas,
                ..deps_de(&correr, &herramientas(&[]))
            },
        );
        assert!(
            v.hallazgos
                .iter()
                .any(|h| h.detalle.starts_with(perfiles::rust::REGLA_CICLO)),
            "la sonda de arquitectura ve el ciclo: {:?}",
            v.hallazgos
        );
        let diseno = puntaje_de_prueba(&v, Dimension::Diseno);
        let modularidad = diseno
            .items
            .iter()
            .find(|i| i.item == MODULARIDAD_ID)
            .expect("modularidad tiene nivel");
        assert_eq!(modularidad.nivel, 1, "el ciclo topa el mixto en 1");
        // (3·1 + 1·3) / (3·4) = 1/2
        assert!((diseno.valor - 0.5).abs() < 1e-6, "{}", diseno.valor);
    }

    #[test]
    fn oraculo_calibracion_marca_calibradas_y_global_es_la_de_seguridad() {
        let base = tempfile::tempdir().expect("base");
        let repo = repo_rust(base.path(), LOCK_SANO);
        let home = home_con_cache(base.path(), LOCK_SANO);
        fs::write(
            home.join(calibra::ARCHIVO),
            json!({"v": 1, "filas": [
                {"tarea": "tarea-rust", "dimension": "seguridad", "n": 20, "semilla": 42,
                 "ts": 1.0, "exactitud_balanceada": 0.95},
                {"tarea": "tarea-rust", "dimension": "diseno", "n": 20, "semilla": 42,
                 "ts": 1.0, "exactitud_balanceada": 0.7},
            ]})
            .to_string(),
        )
        .expect("calibración");
        let calibracion = Calibraciones::cargar(&home).expect("carga");
        let juez = JuezFalso::con(
            &respuesta_juez(
                json!([item_juez(MODULARIDAD_ID, 2, json!([ev_lib()]))]),
                json!([]),
            ),
            0.0,
        );
        let rubricas = [rubrica_diseno()];
        let correr = correr_suites(JUNIT_CONTRATO);
        let v = evaluar_con_deps(
            base.path(),
            &repo,
            Dependencias {
                juez: Some(&juez),
                modelo_juez: MODELO_JUEZ,
                calibracion: &calibracion,
                rubricas: &rubricas,
                ..deps_de(&correr, &herramientas(&[]))
            },
        );
        let seguridad = puntaje_de_prueba(&v, Dimension::Seguridad);
        assert!(seguridad.calibrada);
        assert_eq!(seguridad.exactitud_balanceada, Some(0.95));
        let diseno = puntaje_de_prueba(&v, Dimension::Diseno);
        assert!(!diseno.calibrada);
        assert_eq!(diseno.exactitud_balanceada, Some(0.7));
        let global = v.global.expect("hay una calibrada con veredicto");
        assert!((global - seguridad.valor).abs() < 1e-6, "{global}");
        assert!(v.sin_calibrar.contains(&Dimension::Diseno));
        assert!(!v.sin_calibrar.contains(&Dimension::Seguridad));
        let con_veredicto: Vec<Dimension> = v
            .puntajes
            .iter()
            .filter(|p| p.sin_veredicto.is_none() && p.dimension != Dimension::Seguridad)
            .map(|p| p.dimension)
            .collect();
        assert_eq!(v.sin_calibrar, con_veredicto);
    }

    #[test]
    fn oraculo_sin_calibracion_global_es_none() {
        let base = tempfile::tempdir().expect("base");
        let v = veredicto_sano(base.path());
        assert_eq!(v.global, None);
        assert!(v.puntajes.iter().all(|p| !p.calibrada));
    }

    #[test]
    fn oraculo_juez_de_config_innecesaria_llega_al_veredicto() {
        let base = tempfile::tempdir().expect("base");
        let repo = repo_con_config(base.path());
        let juez_config = JuezFalso::con(
            &json!([{
                "clave": "build.jobs",
                "necesidad": "innecesaria",
                "motivo": "el proyecto no lo usa",
                "cita": "jobs = 2",
            }])
            .to_string(),
            0.0,
        );
        let correr = correr_suites(JUNIT_CONTRATO);
        let v = evaluar_con_deps(
            base.path(),
            &repo,
            Dependencias {
                juez_config: Some(&juez_config),
                ..deps_de(&correr, &herramientas(&[]))
            },
        );
        assert_eq!(juez_config.llamadas(), 1);
        assert!(
            v.hallazgos.iter().any(|h| h.dimension == Dimension::Entrega
                && h.fuente
                    == Fuente::Juez {
                        modelo: "juez-config".into()
                    }
                && h.detalle.contains("innecesaria")),
            "{:?}",
            v.hallazgos
        );
    }

    #[test]
    fn oraculo_costo_usd_suma_las_llamadas_de_los_dos_jueces() {
        let base = tempfile::tempdir().expect("base");
        let repo = repo_con_config(base.path());
        let juez_config = JuezFalso::con(
            &json!([{
                "clave": "build.jobs",
                "necesidad": "necesaria",
                "motivo": "se usa",
                "cita": "jobs = 2",
            }])
            .to_string(),
            0.1,
        );
        let juez = JuezFalso::con(
            &respuesta_juez(
                json!([item_juez(MODULARIDAD_ID, 2, json!([ev_lib()]))]),
                json!([]),
            ),
            0.25,
        );
        let rubricas = [rubrica_diseno()];
        let correr = correr_suites(JUNIT_CONTRATO);
        let v = evaluar_con_deps(
            base.path(),
            &repo,
            Dependencias {
                juez: Some(&juez),
                modelo_juez: MODELO_JUEZ,
                juez_config: Some(&juez_config),
                rubricas: &rubricas,
                ..deps_de(&correr, &herramientas(&[]))
            },
        );
        assert!((v.costo_usd - 0.35).abs() < 1e-9, "{}", v.costo_usd);
    }

    #[test]
    fn oraculo_diseno_sin_rubricas_no_llama_al_juez() {
        let base = tempfile::tempdir().expect("base");
        let repo = repo_rust(base.path(), LOCK_SANO);
        let juez = JuezFalso::con("no se usa", 0.0);
        let correr = correr_suites(JUNIT_CONTRATO);
        let v = evaluar_con_deps(
            base.path(),
            &repo,
            Dependencias {
                juez: Some(&juez),
                modelo_juez: MODELO_JUEZ,
                ..deps_de(&correr, &herramientas(&[]))
            },
        );
        assert_eq!(juez.llamadas(), 0);
        let diseno = puntaje_de_prueba(&v, Dimension::Diseno);
        assert_eq!(
            diseno.sin_veredicto.as_deref(),
            Some("sin rúbricas embebidas")
        );
    }

    #[test]
    fn oraculo_memora_sdlc_con_mas_fallas_da_global_menor_que_ecc() {
        let base = tempfile::tempdir().expect("base");
        let lib = [("src/lib.rs", "pub fn f() -> u8 { 1 }\n")];
        let ecc = repo_con(base.path().join("memora-ecc"), LOCK_SANO, &lib);
        let sdlc = repo_con(base.path().join("memora-sdlc"), LOCK_SANO, &lib);
        let filas = [
            Dimension::Funciona,
            Dimension::Seguridad,
            Dimension::Eficiencia,
            Dimension::Estructura,
            Dimension::Arquitectura,
            Dimension::Completitud,
            Dimension::Entrega,
            Dimension::Pruebas,
        ]
        .into_iter()
        .map(|dimension| calibra::Fila {
            tarea: "tarea-rust".into(),
            dimension,
            n: 20,
            defectos: 10,
            limpios: 10,
            no_aplicable: 0,
            semilla: 42,
            ts: 1.0,
            sensibilidad: Some(0.95),
            especificidad: Some(0.95),
            exactitud_balanceada: Some(0.95),
            items: BTreeMap::new(),
        })
        .collect();
        let calibracion = Calibraciones {
            v: calibra::VERSION,
            filas,
        };
        let correr_ecc = correr_suites(JUNIT_CONTRATO);
        let correr_sdlc = correr_suites(JUNIT_CONTRATO_CON_FALLA);
        let fabrica_sdlc = herramientas_con_fuga();
        let ecc_v = evaluar_con_deps(
            base.path(),
            &ecc,
            Dependencias {
                calibracion: &calibracion,
                ..deps_de(&correr_ecc, &herramientas(&[]))
            },
        );
        let sdlc_v = evaluar_con_deps(
            base.path(),
            &sdlc,
            Dependencias {
                calibracion: &calibracion,
                ..deps_de(&correr_sdlc, &fabrica_sdlc)
            },
        );
        let global_ecc = ecc_v.global.expect("ecc tiene global");
        let global_sdlc = sdlc_v.global.expect("sdlc tiene global");
        assert!(
            global_sdlc < global_ecc,
            "sdlc {global_sdlc} debe quedar bajo ecc {global_ecc}"
        );
    }

    #[test]
    fn oraculo_juntar_partes_promedia_y_lleva_el_motivo() {
        let sondas = Puntaje {
            dimension: Dimension::Estructura,
            valor: 1.0,
            calibrada: false,
            exactitud_balanceada: None,
            items: Vec::new(),
            sin_veredicto: None,
        };
        let juez = Puntaje {
            valor: 0.5,
            items: vec![NivelItem {
                item: "estructura.capas".into(),
                nivel: 1,
                evidencia: Vec::new(),
                fuente: Fuente::Juez {
                    modelo: MODELO_JUEZ.into(),
                },
            }],
            ..sondas.clone()
        };
        let junta = juntar_partes(Dimension::Estructura, sondas.clone(), juez);
        assert!((junta.valor - 0.75).abs() < 1e-6, "{}", junta.valor);
        assert_eq!(junta.items.len(), 1);
        let sin = sin_veredicto_puntaje(Dimension::Estructura, "juez no corrido".into());
        let con_motivo = juntar_partes(Dimension::Estructura, sondas, sin);
        assert_eq!(con_motivo.sin_veredicto.as_deref(), Some("juez no corrido"));
        assert_eq!(con_motivo.valor, 0.0);
    }

    // ---- Tarea 28, parte 1: la Entrega del veredicto sale de `entregar_con` (desempate 7) ----

    /// Dockerfile de la prueba: `FROM scratch` no baja ninguna imagen, así que la descarga no llama a podman.
    const DF_RUN_FALSO: &str = "FROM scratch\nRUN false\nUSER app\n";

    /// Podman que no debe usarse. Las pruebas que no construyen lo dejan por defecto.
    struct PodmanNoUsado;

    impl Podman for PodmanNoUsado {
        fn correr(&self, args: &[&str], _plazo: Duration) -> Result<SalidaPodman, ErrorOraculo> {
            Err(ErrorOraculo::Infra(format!(
                "la prueba no debía llamar a podman: {args:?}"
            )))
        }
    }

    /// Fábrica de escenarios que no arranca nada: el escenario cae al contenedor de evaluación.
    struct FabricaSinContenedores;

    impl FabricaEscenarios for FabricaSinContenedores {
        fn iniciar(
            &self,
            _imagen: &str,
            _escenarios: Option<&Path>,
        ) -> Result<Box<dyn CorredorDeEscenario + '_>, String> {
            Err("sin contenedor de escenario en esta prueba".into())
        }
    }

    /// Salida de un `RUN false` que falla dentro de la imagen, con el paso 2 de 2.
    fn salida_run_falso() -> SalidaPodman {
        SalidaPodman {
            codigo: 1,
            stdout: "STEP 1/2: FROM scratch\nSTEP 2/2: RUN false\n".into(),
            stderr: "Error: building at STEP \"RUN false\": exit status 1\n".into(),
        }
    }

    /// Dependencias de la tarea de Rust con un podman falso y la fábrica de escenarios sin contenedores.
    fn deps_con_podman<'a>(
        correr: Correr<'a>,
        herramientas: FabricaHerramientas<'a>,
        podman: &'a dyn Podman,
    ) -> Dependencias<'a> {
        Dependencias {
            podman,
            ..deps_de(correr, herramientas)
        }
    }

    #[test]
    fn oraculo_hallazgo_de_la_construccion_llega_al_veredicto_en_entrega() {
        let base = tempfile::tempdir().expect("base");
        let repo = repo_rust(base.path(), LOCK_SANO);
        fs::write(repo.join("Dockerfile"), DF_RUN_FALSO).expect("Dockerfile");
        let correr = correr_suites(JUNIT_CONTRATO);
        let fabrica = herramientas(&[]);
        let podman = PodmanFalso::nuevo(responde_build(salida_run_falso(), salida_ok()));
        let deps = deps_con_podman(&correr, &fabrica, &podman);
        let v = evaluar_con_deps(base.path(), &repo, deps);
        let hallazgo = v.hallazgos.iter().find(|h| {
            h.dimension == Dimension::Entrega && h.detalle == "la imagen no construye: línea 2"
        });
        assert!(hallazgo.is_some(), "{:?}", v.hallazgos);
        let entrega = puntaje_de_prueba(&v, Dimension::Entrega);
        assert_eq!(entrega.sin_veredicto, None, "{entrega:?}");
    }

    #[test]
    fn oraculo_construccion_sin_veredicto_deja_entrega_sin_veredicto_con_su_motivo() {
        let base = tempfile::tempdir().expect("base");
        let repo = repo_rust(base.path(), LOCK_SANO);
        // Una variable sin valor: la imagen base no se resuelve y la construcción no da veredicto.
        fs::write(repo.join("Dockerfile"), "FROM ${NO_DEFINIDA}\nUSER app\n").expect("Dockerfile");
        let correr = correr_suites(JUNIT_CONTRATO);
        let fabrica = herramientas(&[]);
        let podman = PodmanFalso::nuevo(responde_build(salida_ok(), salida_ok()));
        let deps = deps_con_podman(&correr, &fabrica, &podman);
        let v = evaluar_con_deps(base.path(), &repo, deps);
        let entrega = puntaje_de_prueba(&v, Dimension::Entrega);
        let motivo = entrega
            .sin_veredicto
            .as_deref()
            .expect("entrega sin veredicto");
        assert!(motivo.contains("imagen base sin resolver"), "{motivo}");
        assert!(podman.builds().is_empty(), "{:?}", podman.builds());
    }

    #[test]
    fn oraculo_sin_dockerfile_no_llama_a_podman_y_entrega_da_veredicto() {
        let base = tempfile::tempdir().expect("base");
        let repo = repo_rust(base.path(), LOCK_SANO);
        let correr = correr_suites(JUNIT_CONTRATO);
        let fabrica = herramientas(&[]);
        let podman = PodmanFalso::nuevo(responde_build(salida_sin_red(), salida_sin_red()));
        let deps = deps_con_podman(&correr, &fabrica, &podman);
        let v = evaluar_con_deps(base.path(), &repo, deps);
        assert_eq!(podman.total(), 0, "{:?}", podman.builds());
        let entrega = puntaje_de_prueba(&v, Dimension::Entrega);
        assert_eq!(entrega.sin_veredicto, None, "{entrega:?}");
    }

    #[test]
    fn oraculo_pendiente_y_aviso_de_la_construccion_llegan_al_veredicto() {
        let base = tempfile::tempdir().expect("base");
        let repo = repo_rust(base.path(), LOCK_SANO);
        fs::write(
            repo.join("Dockerfile"),
            "FROM scratch\nRUN apt-get install -y curl\nUSER app\n",
        )
        .expect("Dockerfile");
        let correr = correr_suites(JUNIT_CONTRATO);
        let fabrica = herramientas(&[]);
        // Sin red, el `RUN` falla por la resolución de nombres, y con red también: queda pendiente.
        let podman = PodmanFalso::nuevo(responde_build(salida_sin_red(), salida_sin_red()));
        let deps = deps_con_podman(&correr, &fabrica, &podman);
        let v = evaluar_con_deps(base.path(), &repo, deps);
        assert!(
            v.pendientes
                .iter()
                .any(|p| p.test == "dockerfile::build" && p.motivo.contains("línea 2")),
            "{:?}",
            v.pendientes
        );
        assert!(
            v.avisos
                .iter()
                .any(|a| a.contains("la construcción con red tampoco tuvo red")),
            "{:?}",
            v.avisos
        );
    }

    // --- Tarea 30, parte 1: las ambigüedades de la ficha llegan al informe ---

    /// Frase de `SPEC` que la revisión guardada cita (línea 38).
    const FRASE_SPEC: &str = "un lock por fragmento del almacén";
    const AVISO_SIN_REVISION: &str = "ficha sin revisión del juez: ambigüedades sin medir";

    /// Guarda en `home` la revisión del juez de `spec`, con una ambigüedad de `FRASE_SPEC`.
    fn revision_del_juez_para(home: &Path, spec: &str) {
        let revision = ficha::Revision {
            hallazgos: Vec::new(),
            ambiguedades: vec![ficha::Ambiguedad {
                frase: FRASE_SPEC.into(),
                linea: 38,
                lectura_a: "lock por fragmento de clave".into(),
                lectura_b: "lock por fragmento de nodo".into(),
            }],
            juez: "ok".into(),
            descartadas: Vec::new(),
        };
        ficha::guardar_revision(home, spec, &revision).expect("guarda la revisión");
    }

    #[test]
    fn oraculo_ambiguedad_guardada_llega_a_la_confianza_del_informe() {
        let base = tempfile::tempdir().expect("base");
        let repo = repo_rust(base.path(), LOCK_SANO);
        let home = base.path().join("home");
        revision_del_juez_para(&home, SPEC);
        let correr = correr_suites(JUNIT_CONTRATO);
        let v = evaluar_con_deps(base.path(), &repo, deps_de(&correr, &herramientas(&[])));
        let esperada = format!("línea 38: {FRASE_SPEC}");
        assert_eq!(v.ambiguedades, vec![esperada.clone()]);
        let informe = v.informe.as_ref().expect("hay informe");
        assert_eq!(informe.confianza.ambiguedades, vec![esperada]);
        assert!(
            !v.avisos.iter().any(|a| a == AVISO_SIN_REVISION),
            "{:?}",
            v.avisos
        );
    }

    #[test]
    fn oraculo_ambiguedad_de_otro_spec_no_se_muestra_y_avisa_sin_medir() {
        let base = tempfile::tempdir().expect("base");
        let repo = repo_rust(base.path(), LOCK_SANO);
        // La caché de Rust evita la descarga; la revisión es del spec original.
        let home = home_con_cache(base.path(), LOCK_SANO);
        revision_del_juez_para(&home, SPEC);
        // La tarea trae el spec con un byte más: otro hash, otra revisión.
        let tarea = tarea_rust(base.path());
        fs::write(base.path().join("tarea/spec.md"), format!("{SPEC}\n")).expect("spec cambiado");
        let correr = correr_suites(JUNIT_CONTRATO);
        let fabrica = herramientas(&[]);
        let deps = deps_de(&correr, &fabrica);
        let v = evaluar_con(&repo, &tarea, &opciones(&home), deps).expect("evalúa");
        assert!(v.ambiguedades.is_empty(), "{:?}", v.ambiguedades);
        let informe = v.informe.as_ref().expect("hay informe");
        assert!(informe.confianza.ambiguedades.is_empty());
        assert!(
            v.avisos.iter().any(|a| a == AVISO_SIN_REVISION),
            "{:?}",
            v.avisos
        );
    }

    #[test]
    fn oraculo_revision_de_ficha_ilegible_da_aviso_y_no_falla() {
        let base = tempfile::tempdir().expect("base");
        let repo = repo_rust(base.path(), LOCK_SANO);
        let home = base.path().join("home");
        let carpeta = home.join("fichas");
        fs::create_dir_all(&carpeta).expect("carpeta");
        fs::write(
            carpeta.join(format!("{}.json", ficha::sha_del_spec(SPEC))),
            "{no es json",
        )
        .expect("escribe la revisión rota");
        let correr = correr_suites(JUNIT_CONTRATO);
        let v = evaluar_con_deps(base.path(), &repo, deps_de(&correr, &herramientas(&[])));
        assert!(v.ambiguedades.is_empty());
        assert!(
            v.avisos
                .iter()
                .any(|a| a.starts_with("revisión de la ficha ilegible")),
            "{:?}",
            v.avisos
        );
    }

    // --- Tarea 30, parte 2: aviso de corrida de Colab con red en el veredicto ---

    /// Escribe el `colab.json` de la tarea de Rust en `home`. Cada entrada: (test, commit, con marca de red).
    fn colab_json(home: &Path, entradas: &[(&str, &str, bool)]) {
        let piezas: Vec<String> = entradas
            .iter()
            .map(|(test, commit, con_marca)| {
                let marca = if *con_marca {
                    format!(r#","confianza":"{}""#, pendientes::CONFIANZA_CON_RED)
                } else {
                    String::new()
                };
                format!(
                    r#""{test}":{{"resultado":"pasa","junit_sha256":"x","ts":1.0,"commit":"{commit}"{marca}}}"#
                )
            })
            .collect();
        let dir = home.join("evidencia").join("tarea-rust");
        fs::create_dir_all(&dir).expect("carpeta de evidencia");
        fs::write(dir.join("colab.json"), format!("{{{}}}", piezas.join(","))).expect("colab.json");
    }

    fn veredicto_con_colab(base: &Path, entradas: &[(&str, &str, bool)]) -> Veredicto {
        let repo = repo_rust(base, LOCK_SANO);
        let home = base.join("home");
        colab_json(&home, entradas);
        let correr = correr_suites(JUNIT_CONTRATO);
        evaluar_con_deps(base, &repo, deps_de(&correr, &herramientas(&[])))
    }

    #[test]
    fn oraculo_colab_con_red_deja_el_aviso_en_el_veredicto() {
        let base = tempfile::tempdir().expect("base");
        let commit = commit_de(&repo_rust(base.path(), LOCK_SANO));
        let v = veredicto_con_colab(base.path(), &[("test_b::t3", &commit, true)]);
        assert!(
            v.avisos.iter().any(|a| a == pendientes::CONFIANZA_CON_RED),
            "{:?}",
            v.avisos
        );
    }

    #[test]
    fn oraculo_colab_con_red_da_el_aviso_una_sola_vez_aunque_haya_varias_entradas() {
        let base = tempfile::tempdir().expect("base");
        let commit = commit_de(&repo_rust(base.path(), LOCK_SANO));
        let v = veredicto_con_colab(
            base.path(),
            &[("test_b::t3", &commit, true), ("test_b::t4", &commit, true)],
        );
        let veces = v
            .avisos
            .iter()
            .filter(|a| a.as_str() == pendientes::CONFIANZA_CON_RED)
            .count();
        assert_eq!(veces, 1, "{:?}", v.avisos);
    }

    #[test]
    fn oraculo_colab_sin_marca_de_red_no_deja_aviso() {
        let base = tempfile::tempdir().expect("base");
        let commit = commit_de(&repo_rust(base.path(), LOCK_SANO));
        let v = veredicto_con_colab(base.path(), &[("test_b::t3", &commit, false)]);
        assert!(
            !v.avisos.iter().any(|a| a == pendientes::CONFIANZA_CON_RED),
            "{:?}",
            v.avisos
        );
    }

    #[test]
    fn oraculo_colab_de_otro_commit_con_red_no_deja_aviso() {
        let base = tempfile::tempdir().expect("base");
        let v = veredicto_con_colab(base.path(), &[("test_b::t3", "otro-commit", true)]);
        assert!(
            v.avisos
                .iter()
                .any(|a| a.contains("no entran al veredicto")),
            "el filtro por commit corrió: {:?}",
            v.avisos
        );
        assert!(
            !v.avisos.iter().any(|a| a == pendientes::CONFIANZA_CON_RED),
            "{:?}",
            v.avisos
        );
    }
}
