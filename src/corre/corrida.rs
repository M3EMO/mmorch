//! Corrida completa de `r00t corre` (Tarea 16, parte 2): GOAL, ruteo previo de todos los roles, armado,
//! lanzamiento por rol, sesiones, lecturas del oráculo, evaluación y la fila de `runs.jsonl`.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Map, Value, json};

use crate::borrado;
use crate::config::Config;
use crate::corre::args::{args_claude, con_resume, env_del_builder};
use crate::corre::config::ConfigCorrida;
use crate::corre::contenedor::{TIMEOUT_BUILDER, estado_de};
use crate::corre::costo_cortado::{
    costo_de_uso, evento_de_corte, precio_de_rol, uso_de_transcript,
};
use crate::corre::git_seguro;
use crate::corre::{
    Corrida, EjecutorBuilder, ErrorCorre, Lanzamiento, barrido, dir_corridas, guardar_transcript,
    preparar_corrida, reabrir_corrida,
};
use crate::goal::verificar_goal;
use crate::observe::claude::registrar_sesion_claude;
use crate::observe::transcript::{HuellasOraculo, registrar_lecturas};
use crate::observe::{self, ErrorObserve};
use crate::oracle::tarea::{Tarea, cargar_en};
use crate::oracle::{ErrorOraculo, OpcionesOraculo, comando_git};
use crate::route::comando::{etiqueta_via, rutear_en};
use crate::route::politica::nombre_rol;
use crate::tipos::{Eleccion, Evento, Fila, Rol, TipoEvento, Veredicto, Via};
use crate::track;

/// Tokens que `r00t corre` pasa a `route` por cada rol: 0, como `r00t route` sin `--tokens`.
const TOKENS_RUTEO: u32 = 0;
/// Motivo del evento `PlanAprobado` que escribe `r00t corre`. La aprobación es automática.
const MOTIVO_PLAN: &str = "aprobación automática de r00t corre";
/// Fases de los `Gate` de una corrida.
const FASE_ROL: &str = "rol";
const FASE_LANZAMIENTO: &str = "lanzamiento";
const FASE_SESION: &str = "sesion";
const FASE_EVALUACION: &str = "evaluacion";
const FASE_TRANSCRIPT: &str = "transcript";
/// Fase del `Gate` que queda cuando el login no alcanza para un rol que va después del primero.
const FASE_VIGENCIA: &str = "vigencia";
/// Mensaje de commit del estado final del repo de la corrida.
const MENSAJE_ESTADO_FINAL: &str = "r00t corre: estado final";
/// Margen que se resta a la vida del login antes de fijar el plazo de un rol.
const MARGEN_LOGIN: Duration = Duration::from_secs(5 * 60);
/// Vida mínima que debe tener el login para arrancar un rol.
const VIGENCIA_MINIMA: Duration = Duration::from_secs(20 * 60);
/// Lanzamientos máximos de un rol cuando sus vueltas se cortan por plazo.
const MAX_VUELTAS: u32 = 4;
/// Prefijo del prompt de cada vuelta que sigue a un corte por plazo.
const PROMPT_CONTINUACION: &str = "La vuelta anterior se cortó por tiempo. Revisá `git status` y `git log`. Terminá o descartá lo que quedó sin commit. Seguí esta instrucción:";

/// Lo que `correr` necesita del mundo. Producción arma las de la CLI; los tests dan las suyas.
pub struct DepsCorre<'a> {
    /// `ROOT_HOME`: `root.toml`, `configs/`, `oraculos/`, `eventos.jsonl`, `runs.jsonl` y transcripts.
    pub home: &'a Path,
    /// `$HOME`: la carpeta de corridas va bajo `<home_usuario>/.cache/root/corridas/`.
    pub home_usuario: &'a Path,
    /// Repo de Root: su `GOAL.md` y su `GOAL.hash` autorizan la corrida.
    pub repo_root: &'a Path,
    /// Lanza el builder de cada rol. Es el único punto por donde sale `claude`.
    pub ejecutor: &'a dyn EjecutorBuilder,
    /// Evalúa el repo que dejó el builder con la tarea.
    pub evaluar: &'a dyn Fn(&Path, &Tarea, &OpcionesOraculo) -> Result<Veredicto, ErrorOraculo>,
    /// Entorno del proceso. Solo `PATH` y `LANG` llegan al builder, y eso lo decide `env_del_builder`.
    pub entorno: &'a [(String, String)],
}

/// Una vuelta que terminó bien: su número de lanzamiento, su código de salida y su línea `result`.
struct Lanzado {
    n: u32,
    rol: Rol,
    codigo: i32,
    linea: String,
}

/// Corre la tarea con la configuración y devuelve la fila que se agregó a `runs.jsonl`.
///
/// Cada rol registra su sesión al terminar, antes de lanzar el siguiente.
/// Así su costo queda aunque un rol posterior aborte.
///
/// El login debe durar al menos `VIGENCIA_MINIMA` para arrancar. Cada lanzamiento corre con un plazo
/// que sale de la vida del login en ese momento (ver `plazo_de_lanzamiento`).
///
/// Un corte por plazo no aborta: el mismo rol vuelve a lanzarse, hasta `MAX_VUELTAS` lanzamientos.
/// Cada vuelta retoma la sesión de la anterior con `--resume`, si la hay, y pide continuar.
/// Si la última vuelta también se corta, el rol se agota y no corren más roles.
/// La corrida igual llega a las lecturas del oráculo, a la evaluación y a la fila.
///
/// Un código distinto de 0 de un rol corta la corrida: los roles siguientes no corren,
/// pero la evaluación sí, y el informe dice lo que falta.
/// Un fallo al lanzar que no es un corte aborta sin evaluar: sin builder no hay nada que evaluar,
/// y la evaluación puede gastar API.
/// Un rol sin `result` exitoso también aborta sin evaluar ni escribir fila: deja un `Gate` con el motivo.
/// Cada vuelta cortada guarda su transcript parcial y registra su costo estimado
/// (ver `registrar_corte`).
/// Desde el segundo lanzamiento, la vigencia del login se espera antes de cada lanzamiento
/// (ver `esperar_vigencia_del_rol`).
pub fn correr(deps: &DepsCorre, tarea: &str, config: &str) -> Result<Fila, ErrorCorre> {
    correr_interno(deps, tarea, config, None)
}

/// Retoma la corrida `id`, cortada antes de terminar, y devuelve su fila.
///
/// `id` debe ser de esta tarea y esta config, y su carpeta debe existir con `repo/`. Si no, es error de
/// uso y no se lanza nada. No se arma un repo nuevo: sigue el de la corrida cortada.
///
/// Los roles que terminaron con `result` exitoso no se lanzan otra vez. Su sesión se registra si falta.
/// El primer rol sin `result` exitoso retoma: si tiene transcripts parciales, la primera vuelta nueva
/// lleva `--resume` con la última sesión que traigan, y pide continuar. Tiene sus `MAX_VUELTAS` vueltas.
/// Los roles que siguen se lanzan como en una corrida nueva.
/// El GOAL, la config, el ruteo, la vigencia, las lecturas, la evaluación y la fila son como en `correr`.
pub fn correr_reanudando(
    deps: &DepsCorre,
    tarea: &str,
    config: &str,
    id: &str,
) -> Result<Fila, ErrorCorre> {
    correr_interno(deps, tarea, config, Some(id))
}

/// Cuerpo común de `correr` y `correr_reanudando`. Con `reanuda`, abre la carpeta de esa corrida en vez de
/// armar una nueva, y retoma lo que ya tiene guardado.
fn correr_interno(
    deps: &DepsCorre,
    tarea: &str,
    config: &str,
    reanuda: Option<&str>,
) -> Result<Fila, ErrorCorre> {
    // a) GOAL: sin autorización no arranca y no se crea nada.
    verificar_goal(deps.repo_root)?;

    // b) Tarea, config y huellas del oráculo. Las huellas van antes de lanzar: sin ellas no hay lecturas que revisar.
    let tarea_cargada = cargar_en(deps.home, tarea).map_err(|e| ErrorCorre::Uso(e.to_string()))?;
    let config_cargada = ConfigCorrida::cargar(deps.home, config)?;
    // Al reanudar, el id se revisa antes de lanzar o crear nada.
    let reabierta = match reanuda {
        Some(id) => Some(reabrir_corrida(
            deps.home,
            deps.home_usuario,
            tarea,
            config,
            id,
        )?),
        None => None,
    };
    let huellas = HuellasOraculo::armar(deps.home, Some(deps.home_usuario), tarea)
        .map_err(|e| de_observe("armar las huellas del oráculo", e))?;
    let ruta_eventos = observe::ruta_eventos_en(deps.home);

    // Copias de credencial de corridas muertas: se barren antes de rutear o lanzar nada.
    barrido::barrer(&dir_corridas(deps.home_usuario), &ruta_eventos)?;

    // c) Ruteo previo de todos los roles: si uno no va por cupo, no se lanza nada.
    let mut elecciones: Vec<(Rol, Eleccion)> = Vec::new();
    for rol in &config_cargada.roles {
        let eleccion = rutear_en(deps.home, *rol, tarea, false, TOKENS_RUTEO)
            .map_err(|e| ErrorCorre::Ruteo(Box::new(e)))?;
        exigir_cupo(*rol, &eleccion)?;
        elecciones.push((*rol, eleccion));
    }
    // Vigencia del login: debe durar al menos `VIGENCIA_MINIMA`. Si no, no se arma ni se lanza nada.
    deps.ejecutor.verificar_antes(VIGENCIA_MINIMA)?;

    // d) Armado de la carpeta de la corrida, o su reapertura. Reanudar no arma nada.
    let corrida = match reabierta {
        Some(corrida) => corrida,
        None => preparar_corrida(deps.home, deps.home_usuario, tarea, config)?,
    };
    // Desde acá, todo camino de salida quita la copia de la credencial (ver `GuardiaCredencial`).
    let guardia = GuardiaCredencial {
        corrida: &corrida,
        hecho: false,
    };
    let id = corrida.id.clone();
    // Lo que dejó una corrida anterior: sus transcripts y el mayor número de lanzamiento.
    let (previos, mayor_previo) = previos_de(deps.home, &id)?;

    // e) Lanzamiento por rol, en orden. Cada salida se guarda como transcript. Un corte por plazo
    // vuelve a lanzar el mismo rol, hasta `MAX_VUELTAS` veces.
    let mut lanzados: Vec<Lanzado> = Vec::new();
    // Transcript de cada lanzamiento, buenos y cortados, también los de corridas anteriores. Lo revisa
    // el oráculo en el paso g.
    let mut transcripts: Vec<(Rol, PathBuf)> =
        previos.iter().map(|p| (p.rol, p.ruta.clone())).collect();
    let mut lanzamientos: u32 = mayor_previo;
    // Al reanudar, cada rol se mira en orden hasta el primero sin `result` exitoso. Después, todo se lanza normal.
    let mut reanudando = reanuda.is_some();
    // Desde el primer lanzamiento de esta invocación, la vigencia se espera antes de cada lanzamiento.
    let mut ya_lanzo = false;
    for (rol, eleccion) in &elecciones {
        let prompt_rol = config_cargada.prompt.get(rol).cloned().unwrap_or_default();
        let modelo = eleccion.modelo.clone().unwrap_or_default();
        let mut sesion: Option<String> = None;
        // Hay transcripts parciales de este rol: la primera vuelta sigue con la instrucción de continuar.
        let mut retoma = false;
        if reanudando {
            match estado_previo(&previos, *rol) {
                EstadoRol::Terminado(lanzado) => {
                    registrar_sesion(deps.home, &id, &lanzado, &lanzado.linea, &ruta_eventos)?;
                    lanzados.push(lanzado);
                    continue;
                }
                EstadoRol::Parcial { sesion: previa } => {
                    sesion = previa;
                    retoma = true;
                }
                EstadoRol::Nuevo => {}
            }
            reanudando = false;
        }
        let mut vuelta: u32 = 0;
        // `Some(código)` si el rol terminó. `None` si agotó sus vueltas cortadas.
        let codigo = loop {
            // Desde el primer lanzamiento de esta invocación, cada uno vuelve a leer la credencial: se revisa su vigencia antes.
            if ya_lanzo {
                esperar_vigencia_del_rol(deps.ejecutor, &ruta_eventos, &id, *rol)?;
            }
            vuelta += 1;
            lanzamientos += 1;
            ya_lanzo = true;
            let n = lanzamientos;
            let prompt = if retoma || vuelta > 1 {
                format!("{PROMPT_CONTINUACION} {prompt_rol}")
            } else {
                prompt_rol.clone()
            };
            let args = args_claude(&prompt, &modelo, deps.home, &config_cargada.claude, *rol);
            let args = match &sesion {
                Some(sesion) => con_resume(args, sesion),
                None => args,
            };
            // El plazo sale de la vida del login en este momento, no de la del inicio de la corrida.
            let plazo = plazo_de_lanzamiento(deps.ejecutor.vida_restante()?);
            let lanzamiento = Lanzamiento {
                rol: *rol,
                args,
                env: env_del_builder(&config_cargada.claude, *rol, deps.entorno),
                repo: corrida.repo.clone(),
                plugin: config_cargada.claude.plugin_de(*rol).map(Path::to_path_buf),
                plazo,
            };
            match deps.ejecutor.lanzar(&lanzamiento) {
                Ok(salida) => {
                    let ruta = guardar_transcript(deps.home, &id, n, *rol, &salida.stdout)?;
                    transcripts.push((*rol, ruta.clone()));
                    // Sin `result` exitoso el rol no evalúa: no hay resultado que evaluar.
                    let linea = match resultado_exitoso(&salida.stdout) {
                        Ok(linea) => linea,
                        Err(causa) => {
                            let motivo =
                                format!("rol {} sin resultado exitoso: {causa}", nombre_rol(*rol));
                            gate(&ruta_eventos, &id, FASE_ROL, motivo.clone(), Some(*rol))?;
                            return Err(ErrorCorre::Ejecutor(motivo));
                        }
                    };
                    let codigo = salida.codigo;
                    let lanzado = Lanzado {
                        n,
                        rol: *rol,
                        codigo,
                        linea,
                    };
                    // La sesión se registra al terminar el rol: si un rol siguiente aborta, su costo ya queda.
                    registrar_sesion(deps.home, &id, &lanzado, &lanzado.linea, &ruta_eventos)?;
                    lanzados.push(lanzado);
                    break Some(codigo);
                }
                Err(ErrorCorre::Cortado { mensaje, stdout }) => {
                    // Un corte no aborta: deja su transcript parcial, su costo y la sesión para la siguiente vuelta.
                    let ruta =
                        registrar_corte(deps.home, &ruta_eventos, &id, *rol, n, &modelo, &stdout)?;
                    transcripts.push((*rol, ruta));
                    let motivo = format!("rol {} cortado por plazo: {mensaje}", nombre_rol(*rol));
                    gate(&ruta_eventos, &id, FASE_LANZAMIENTO, motivo, Some(*rol))?;
                    if let Some(id_sesion) = sesion_de_transcript(&stdout) {
                        sesion = Some(id_sesion);
                    }
                    if vuelta == MAX_VUELTAS {
                        break None;
                    }
                }
                Err(error) => {
                    let motivo = format!("rol {} no arrancó: {error}", nombre_rol(*rol));
                    gate(&ruta_eventos, &id, FASE_LANZAMIENTO, motivo, Some(*rol))?;
                    return Err(error);
                }
            }
        };
        match codigo {
            None => {
                let motivo = format!(
                    "rol {} agotó {MAX_VUELTAS} vueltas cortadas por plazo",
                    nombre_rol(*rol)
                );
                gate(&ruta_eventos, &id, FASE_ROL, motivo, Some(*rol))?;
                break;
            }
            Some(codigo) if codigo != 0 => {
                let motivo = format!("rol {} salió con {codigo}", nombre_rol(*rol));
                gate(&ruta_eventos, &id, FASE_ROL, motivo, Some(*rol))?;
                break;
            }
            Some(_) => {}
        }
    }

    // f) Después de planifica, la aprobación automática del plan. Solo si planifica salió bien.
    // Reanudar no la vuelve a escribir si ya está en el log.
    let plan_aprobado = lanzados
        .iter()
        .any(|l| l.rol == Rol::Planifica && l.codigo == 0);
    if plan_aprobado && !plan_ya_aprobado(&ruta_eventos, &id)? {
        let mut ev = observe::evento_manual(&id, TipoEvento::PlanAprobado, Some(MOTIVO_PLAN));
        // Una aprobación humana y una automática no se confunden.
        ev.extra.insert("automatica".into(), json!(true));
        registrar(&ruta_eventos, &ev)?;
    }

    // g) Lecturas del oráculo en cada transcript, también los de vueltas cortadas.
    // Las líneas que no son JSON también se anotan.
    for (rol, ruta) in &transcripts {
        let revision = registrar_lecturas(ruta, &id, &huellas, &ruta_eventos)
            .map_err(|e| de_observe("revisar el transcript", e))?;
        if !revision.avisos.is_empty() {
            let motivo = format!(
                "transcript con líneas que no son JSON: {}",
                revision.avisos.join("; ")
            );
            gate(&ruta_eventos, &id, FASE_TRANSCRIPT, motivo, Some(*rol))?;
        }
    }

    // h) Se quita la copia de la credencial. La guardia lo hace y su error llega acá.
    // Después se arma el estado final en un repo nuevo.
    guardia.cerrar()?;
    // El host no corre git sobre el `.git` del builder: copia el árbol a un repo nuevo y lo evalúa ahí.
    let repo_final = corrida.dir.join("final");
    git_seguro::repo_limpio_desde(&corrida.repo, &repo_final, MENSAJE_ESTADO_FINAL)?;

    // i) Evaluación. Un error no pierde la corrida: la fila lleva el veredicto por defecto.
    let opciones = OpcionesOraculo {
        home: Some(deps.home.to_path_buf()),
        ..OpcionesOraculo::default()
    };
    let veredicto = match (deps.evaluar)(&repo_final, &tarea_cargada, &opciones) {
        Ok(veredicto) => veredicto,
        Err(error) => {
            let motivo = format!("la evaluación falló: {error}");
            gate(&ruta_eventos, &id, FASE_EVALUACION, motivo, None)?;
            Veredicto {
                tarea: tarea_cargada.nombre.clone(),
                ..Veredicto::default()
            }
        }
    };

    // j) La fila. El resumen sale de los eventos de esta corrida.
    let resumen =
        observe::resumir_en(&ruta_eventos, &id).map_err(|e| de_observe("resumir la corrida", e))?;
    let modelos = elecciones
        .iter()
        .filter_map(|(rol, eleccion)| eleccion.modelo.clone().map(|m| (*rol, m)))
        .collect();
    let fila = Fila {
        id,
        ts: observe::ahora().0,
        tarea: tarea.to_string(),
        config: config.to_string(),
        commit_root: commit_completo(deps.repo_root),
        veredicto,
        resumen,
        modelos,
        forma: config_cargada.forma(config),
        vueltas: lanzamientos,
        reanudada: reanuda.is_some(),
        ..Fila::default()
    };
    track::agregar_en(&deps.home.join("runs.jsonl"), &fila).map_err(|fuente| ErrorCorre::Io {
        contexto: "escribir runs.jsonl".into(),
        fuente,
    })?;
    Ok(fila)
}

/// Un transcript guardado de una corrida anterior: `<n>-<rol>.jsonl` en `transcripts/<id>/`.
struct Previo {
    n: u32,
    rol: Rol,
    ruta: PathBuf,
    stdout: Vec<u8>,
}

/// Qué queda de un rol en la corrida que se reanuda.
enum EstadoRol {
    /// Un transcript trae `result` exitoso: el rol no se lanza otra vez.
    Terminado(Lanzado),
    /// Hay transcripts y ninguno terminó. Lleva la sesión del último que la traiga.
    Parcial { sesion: Option<String> },
    /// No hay transcripts del rol: se lanza como en una corrida nueva.
    Nuevo,
}

/// Transcripts guardados de la corrida `id`, en orden de `n`, y el mayor `n` de sus archivos, también
/// los `resultado.json`. Sin carpeta, no hay ninguno.
fn previos_de(home: &Path, id: &str) -> Result<(Vec<Previo>, u32), ErrorCorre> {
    let carpeta = home.join("transcripts").join(id);
    let entradas = match fs::read_dir(&carpeta) {
        Ok(entradas) => entradas,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok((Vec::new(), 0));
        }
        Err(error) => {
            return Err(super::infra(format!("leer {}", carpeta.display()))(error));
        }
    };
    let mut previos = Vec::new();
    let mut mayor = 0;
    for entrada in entradas {
        let entrada = entrada.map_err(super::infra(format!("leer {}", carpeta.display())))?;
        let nombre = entrada.file_name().to_string_lossy().into_owned();
        let Some((n, rol, es_resultado)) = partes_de_archivo(&nombre) else {
            continue;
        };
        mayor = mayor.max(n);
        if es_resultado {
            continue;
        }
        let ruta = entrada.path();
        let stdout = fs::read(&ruta).map_err(super::infra(format!("leer {}", ruta.display())))?;
        previos.push(Previo {
            n,
            rol,
            ruta,
            stdout,
        });
    }
    previos.sort_by_key(|p| p.n);
    Ok((previos, mayor))
}

/// Partes de un archivo de transcripts: `<n>-<rol>.jsonl` o `<n>-<rol>.resultado.json`. Da `n`, el rol
/// y si es el resultado. Un nombre que no tiene esa forma da `None`.
fn partes_de_archivo(nombre: &str) -> Option<(u32, Rol, bool)> {
    let (base, es_resultado) = match nombre.strip_suffix(".resultado.json") {
        Some(base) => (base, true),
        None => (nombre.strip_suffix(".jsonl")?, false),
    };
    let (n, rol) = base.split_once('-')?;
    let rol = serde_json::from_value(Value::String(rol.to_string())).ok()?;
    Some((n.parse().ok()?, rol, es_resultado))
}

/// Estado de `rol` según lo guardado. Terminado si alguno de sus transcripts trae `result` exitoso.
fn estado_previo(previos: &[Previo], rol: Rol) -> EstadoRol {
    let del_rol: Vec<&Previo> = previos.iter().filter(|p| p.rol == rol).collect();
    let terminado = del_rol
        .iter()
        .rev()
        .find_map(|p| resultado_exitoso(&p.stdout).ok().map(|linea| (p.n, linea)));
    if let Some((n, linea)) = terminado {
        // El código de salida no se guardó. Un rol que terminó bien lleva 0: si hubiera salido distinto,
        // la corrida original ya habría cortado antes de los roles siguientes.
        return EstadoRol::Terminado(Lanzado {
            n,
            rol,
            codigo: 0,
            linea,
        });
    }
    if del_rol.is_empty() {
        return EstadoRol::Nuevo;
    }
    let sesion = del_rol
        .iter()
        .rev()
        .find_map(|p| sesion_de_transcript(&p.stdout));
    EstadoRol::Parcial { sesion }
}

/// Si `eventos.jsonl` ya trae la aprobación automática de la corrida `id`.
fn plan_ya_aprobado(ruta_eventos: &Path, id: &str) -> Result<bool, ErrorCorre> {
    let eventos =
        observe::uso::leer_eventos(ruta_eventos).map_err(|e| de_observe("leer eventos", e))?;
    Ok(eventos
        .iter()
        .any(|ev| ev.tipo == TipoEvento::PlanAprobado && ev.corrida == id))
}

/// Plazo de un rol según la vida del login. Sin vida conocida, rige el tope de dos horas. Con vida,
/// el plazo es la vida menos `MARGEN_LOGIN`, sin pasar del tope.
fn plazo_de_lanzamiento(vida: Option<Duration>) -> Duration {
    match vida {
        None => TIMEOUT_BUILDER,
        Some(vida) => TIMEOUT_BUILDER.min(vida.saturating_sub(MARGEN_LOGIN)),
    }
}

/// Espera a que el login dure al menos `VIGENCIA_MINIMA`. Si no dura, deja un `Gate` y revisa cada
/// `intervalo`, como mucho `reintentos()` veces. Si nunca dura, devuelve el último error.
fn esperar_vigencia_del_rol(
    ejecutor: &dyn EjecutorBuilder,
    ruta_eventos: &Path,
    id: &str,
    rol: Rol,
) -> Result<(), ErrorCorre> {
    let Err(mut ultimo) = ejecutor.verificar_antes(VIGENCIA_MINIMA) else {
        return Ok(());
    };
    let motivo = format!(
        "el login no alcanza para el rol {}: r00t espera su renovación",
        nombre_rol(rol)
    );
    gate(ruta_eventos, id, FASE_VIGENCIA, motivo, Some(rol))?;
    let espera = ejecutor.espera_vigencia();
    for _ in 0..espera.reintentos() {
        std::thread::sleep(espera.intervalo);
        match ejecutor.verificar_antes(VIGENCIA_MINIMA) {
            Ok(()) => return Ok(()),
            Err(error) => ultimo = error,
        }
    }
    Err(ultimo)
}

/// Una vuelta cortada por tiempo: guarda su transcript parcial como `<n>-<rol>.jsonl` y registra el costo
/// estimado de lo que gastó. Devuelve la ruta del transcript. Su `Gate` lo pone quien llama.
/// El costo sale de `usage` y del precio del modelo.
fn registrar_corte(
    home: &Path,
    ruta_eventos: &Path,
    id: &str,
    rol: Rol,
    n: u32,
    modelo: &str,
    stdout: &[u8],
) -> Result<PathBuf, ErrorCorre> {
    let ruta = guardar_transcript(home, id, n, rol, stdout)?;
    let config =
        Config::cargar(&home.join("root.toml")).map_err(|e| ErrorCorre::Uso(e.to_string()))?;
    let uso = uso_de_transcript(stdout);
    let costo = costo_de_uso(&uso, precio_de_rol(&config, modelo));
    registrar(ruta_eventos, &evento_de_corte(id, modelo, &uso, &costo))?;
    Ok(ruta)
}

/// `session_id` de la primera línea JSON del `stdout` que lo trae. Las líneas que no son JSON se saltan.
fn sesion_de_transcript(stdout: &[u8]) -> Option<String> {
    String::from_utf8_lossy(stdout).lines().find_map(|linea| {
        let valor: Value = serde_json::from_str(linea).ok()?;
        valor.get("session_id")?.as_str().map(str::to_string)
    })
}

/// Un rol va por cupo con modelo, o `r00t corre` no lo lanza.
fn exigir_cupo(rol: Rol, eleccion: &Eleccion) -> Result<(), ErrorCorre> {
    let nombre = nombre_rol(rol);
    match (eleccion.via, &eleccion.modelo) {
        (Via::Cupo, Some(_)) => Ok(()),
        (Via::Cupo, None) => Err(ErrorCorre::Uso(format!(
            "el rol {nombre} va por cupo sin modelo: r00t corre solo lanza claude -p por cupo"
        ))),
        (via, _) => Err(ErrorCorre::Uso(format!(
            "el rol {nombre} va por {}: r00t corre solo lanza claude -p por cupo",
            etiqueta_via(via)
        ))),
    }
}

/// Textos de `result` que dicen que el login venció o no es válido. Se comparan en minúsculas.
const TEXTOS_LOGIN_VENCIDO: [&str; 4] = [
    "authentication",
    "authenticate",
    "oauth token has expired",
    "invalid api key",
];

/// Línea `result` del rol si terminó bien. Si no, el motivo que va al `Gate`.
fn resultado_exitoso(stdout: &[u8]) -> Result<String, String> {
    let linea = linea_result(stdout).ok_or("sin línea result: el builder no dejó su resultado")?;
    let valor: Value = serde_json::from_str(&linea).unwrap_or(Value::Null);
    if valor.get("is_error").and_then(Value::as_bool) == Some(true) {
        return Err(motivo_de_error(&valor));
    }
    match valor.get("subtype").and_then(Value::as_str) {
        Some("success") => Ok(linea),
        Some(otro) => Err(format!("terminó con subtype {otro}")),
        None => Err("la línea result no trae subtype".into()),
    }
}

/// Motivo de un `result` con `is_error` true. Si el token venció, lo dice.
/// No copia el texto del `result`: puede traer datos de la cuenta.
fn motivo_de_error(valor: &Value) -> String {
    let estado = valor.get("api_error_status").and_then(Value::as_u64);
    let texto = valor
        .get("result")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_lowercase();
    let login = TEXTOS_LOGIN_VENCIDO.iter().any(|t| texto.contains(t));
    if estado == Some(401) || login {
        "el login de Claude venció o no es válido".into()
    } else {
        "terminó con is_error".into()
    }
}

/// Línea `{"type":"result",...}` de un stream-json, si hay una.
fn linea_result(stdout: &[u8]) -> Option<String> {
    let texto = String::from_utf8_lossy(stdout);
    texto
        .lines()
        .find(|linea| {
            serde_json::from_str::<Value>(linea)
                .ok()
                .and_then(|valor| {
                    valor
                        .get("type")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .as_deref()
                == Some("result")
        })
        .map(str::to_string)
}

/// Registra la sesión de un rol. Guarda su línea `result` como `<n>-<rol>.resultado.json` en transcripts.
/// `registrar_sesion_claude` la registra una sola vez por `session_id`.
/// Un error de `ErrorObserve` deja un `Gate` y no corta la corrida.
/// Si el log no se puede escribir, el `Gate` también falla y corta la corrida.
fn registrar_sesion(
    home: &Path,
    id: &str,
    lanzado: &Lanzado,
    linea: &str,
    ruta_eventos: &Path,
) -> Result<(), ErrorCorre> {
    let ruta = home.join("transcripts").join(id).join(format!(
        "{}-{}.resultado.json",
        lanzado.n,
        nombre_rol(lanzado.rol)
    ));
    fs::write(&ruta, format!("{linea}\n"))
        .map_err(super::infra("guardar el resultado de la sesión"))?;
    if let Err(error) = registrar_sesion_claude(&ruta, id, None, ruta_eventos) {
        let motivo = format!("sesión no registrada: {error}");
        return gate(ruta_eventos, id, FASE_SESION, motivo, Some(lanzado.rol));
    }
    Ok(())
}

/// Guardia de la copia de la credencial de una corrida. La copia es de Root y se borra siempre al
/// terminar. `cerrar` la quita en el camino normal y devuelve su error. Si `correr` sale antes por otro
/// error, el `Drop` la quita igual, con el error descartado.
struct GuardiaCredencial<'a> {
    corrida: &'a Corrida,
    hecho: bool,
}

impl GuardiaCredencial<'_> {
    /// Camino normal: quita la copia de la credencial y devuelve el error de borrarla, si lo hubo.
    fn cerrar(mut self) -> Result<(), ErrorCorre> {
        self.hecho = true;
        quitar_credencial(self.corrida)
    }
}

impl Drop for GuardiaCredencial<'_> {
    fn drop(&mut self) {
        if !self.hecho {
            // En un camino de error ya hay otro error que devolver: el de borrar se descarta.
            let _ = quitar_credencial(self.corrida);
        }
    }
}

/// Borra la copia `.credentials.json` del estado de la corrida, si existe. La copia la arma Root sin
/// `mcpOAuth`. Un enlace se borra como enlace, sin seguir su destino. Lo llama `GuardiaCredencial`, en todo
/// camino de salida de `correr` desde que se prepara la corrida.
fn quitar_credencial(corrida: &Corrida) -> Result<(), ErrorCorre> {
    let Some(estado) = estado_de(&corrida.repo) else {
        return Ok(());
    };
    let ruta = estado.join(".credentials.json");
    borrado::borrar_archivo_dentro(&corrida.dir, &ruta).map_err(|fuente| ErrorCorre::Io {
        contexto: format!("borrar la copia de la credencial {}", ruta.display()),
        fuente,
    })
}

/// Commit completo de `HEAD` de `repo`. Vacío si git no lo da.
fn commit_completo(repo: &Path) -> String {
    comando_git(repo, &["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|salida| salida.status.success())
        .map(|salida| String::from_utf8_lossy(&salida.stdout).trim().to_string())
        .unwrap_or_default()
}

/// Escribe un `Gate` de la corrida con su motivo y, si lo hay, el rol.
fn gate(
    ruta_eventos: &Path,
    corrida: &str,
    fase: &str,
    motivo: String,
    rol: Option<Rol>,
) -> Result<(), ErrorCorre> {
    let mut extra = Map::from_iter([("motivo".to_string(), json!(motivo))]);
    if let Some(rol) = rol {
        extra.insert("rol".into(), json!(nombre_rol(rol)));
    }
    registrar(ruta_eventos, &observe::evento_gate(corrida, fase, extra))
}

/// Agrega un evento a `eventos.jsonl` de `home`.
fn registrar(ruta_eventos: &Path, ev: &Evento) -> Result<(), ErrorCorre> {
    observe::registrar_en(ruta_eventos, ev).map_err(|fuente| ErrorCorre::Io {
        contexto: format!("registrar en {}", ruta_eventos.display()),
        fuente,
    })
}

/// Convierte un error de Observe en el de la corrida. Un error de E/S conserva su causa.
fn de_observe(contexto: &str, error: ErrorObserve) -> ErrorCorre {
    match error {
        ErrorObserve::Io(fuente) => ErrorCorre::Io {
            contexto: contexto.into(),
            fuente,
        },
        otro => ErrorCorre::Observa(format!("{contexto}: {otro}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::fs;
    use std::path::PathBuf;

    use std::time::Duration;

    use serde_json::Value;
    use tempfile::TempDir;

    use crate::corre::ejecutor::pruebas::EjecutorFalso;
    use crate::corre::{EsperaVigencia, Lanzamiento, SalidaClaude, dir_corridas};
    use crate::goal::{ErrorGoal, calcular_hash};
    use crate::oracle::ErrorOraculo;
    use crate::tipos::{Confianza, Decision, Estado, Evento, Forma, Informe, Rol, TipoEvento, Via};
    use crate::track::{self, Filtro};

    const GOAL: &str = "# GOAL de prueba\n\nContenido de fixture.\n";
    const TAREA: &str = "memo";
    const CONFIG: &str = "ecc-test";
    const CONFIG_PLAN: &str = "solo-plan";
    const CONFIG_CONSTRUYE: &str = "solo-construye";
    const CONFIG_PLAN_ORGANIZA: &str = "plan-organiza";
    const CONFIG_API: &str = "por-api";
    const PLAN_APROBADO: &str = "aprobación automática de r00t corre";

    /// Un ROOT_HOME de prueba con la tarea y sus configs, un `$HOME` y un repo de Root con GOAL válido.
    struct Entorno {
        _tmp: TempDir,
        home: PathBuf,
        usuario: PathBuf,
        repo: PathBuf,
    }

    fn entorno() -> Entorno {
        let tmp = TempDir::new().expect("tmp");
        let base = tmp.path().to_path_buf();
        let home = base.join("root");
        let usuario = base.join("usuario");
        let repo = base.join("repo");
        fs::create_dir_all(&home).expect("home");
        fs::create_dir_all(&usuario).expect("usuario");
        fs::create_dir_all(&repo).expect("repo");

        fs::write(
            home.join("root.toml"),
            "[modelos]\nplanifica = \"opus\"\norganiza = \"sonnet\"\nconstruye = \"haiku\"\n",
        )
        .expect("root.toml");
        let configs = home.join("configs");
        fs::create_dir_all(&configs).expect("configs");
        let prompt = "[prompt]\nplanifica = \"haz el plan\"\nconstruye = \"haz hecho\"\n";
        fs::write(
            configs.join(format!("{CONFIG}.toml")),
            format!("roles = [\"planifica\", \"construye\"]\n{prompt}"),
        )
        .expect("config");
        fs::write(
            configs.join(format!("{CONFIG_PLAN}.toml")),
            format!("roles = [\"planifica\"]\n{prompt}"),
        )
        .expect("config plan");
        fs::write(
            configs.join(format!("{CONFIG_CONSTRUYE}.toml")),
            format!("roles = [\"construye\"]\n{prompt}"),
        )
        .expect("config construye");
        fs::write(
            configs.join(format!("{CONFIG_PLAN_ORGANIZA}.toml")),
            "roles = [\"planifica\", \"organiza\"]\n[prompt]\nplanifica = \"haz el plan\"\norganiza = \"organiza\"\n",
        )
        .expect("config plan organiza");
        fs::write(
            configs.join(format!("{CONFIG_API}.toml")),
            format!("roles = [\"planifica\", \"delega\"]\n{prompt}delega = \"delega\"\n"),
        )
        .expect("config api");

        let oraculo = home.join("oraculos").join(TAREA);
        fs::create_dir_all(oraculo.join("bootstrap").join("docs")).expect("bootstrap");
        fs::write(
            oraculo.join("bootstrap").join("docs").join("SPEC.md"),
            "# Spec\n",
        )
        .expect("spec");
        fs::create_dir_all(oraculo.join("contrato")).expect("contrato");
        fs::write(
            oraculo.join("contrato").join("test_x.py"),
            "def test_x(): pass\n",
        )
        .expect("contrato");
        fs::create_dir_all(oraculo.join("oculto")).expect("oculto");
        fs::write(
            oraculo.join("oculto").join("test_y.py"),
            "def test_y():\n    assert sincronizar(tmp, \"clave-secreta-42\") == [1, 2, 3]\n",
        )
        .expect("oculto");
        fs::write(
            oraculo.join("tarea.toml"),
            "nombre = \"memo\"\nlenguaje = \"rust\"\nimagen = \"root-oraculo-rust:1\"\nspec = \"bootstrap/docs/SPEC.md\"\n\n[contrato]\ncmd = \"pytest -q /oraculo/contrato\"\n\n[oculto]\ncmd = \"pytest -q /oraculo/oculto\"\n",
        )
        .expect("tarea.toml");

        fs::write(repo.join("GOAL.md"), GOAL).expect("GOAL.md");
        fs::write(
            repo.join("GOAL.hash"),
            format!("{}\n", calcular_hash(GOAL.as_bytes())),
        )
        .expect("GOAL.hash");

        Entorno {
            _tmp: tmp,
            home,
            usuario,
            repo,
        }
    }

    /// Builder de prueba: el falso de `tests/fixtures`, con las fallas que cada test pide.
    #[derive(Default)]
    struct Builder {
        /// Rol cuyo builder sale con código 1.
        falla: Option<Rol>,
        /// Variables que se suman al entorno del lanzamiento. Solo las pone el test.
        extra: Vec<(String, String)>,
        /// Quita la línea `result` de la salida.
        sin_resultado: bool,
        /// Quita la línea `result` solo al lanzamiento de este rol.
        sin_resultado_en: Option<Rol>,
        /// Cambia el `result` por uno sin costo.
        costo_desconocido: bool,
        /// Cambia el `result` por uno con `is_error` true, como un login vencido.
        token_vencido: bool,
        /// El lanzamiento no arranca.
        no_lanza: bool,
        /// Contenido de `.credentials.json` en el estado de la corrida, como lo deja el montaje de podman.
        credencial: Option<&'static str>,
        /// El lanzamiento falla después de montar la credencial, como un fallo de podman tras el montaje.
        falla_tras_montar: bool,
        /// Carpeta fuera de la corrida. Al lanzar, la carpeta de estado pasa a ser un enlace a ella, con un
        /// `.credentials.json` vacío dentro.
        estado_fuera: Option<PathBuf>,
        /// Enlace `.credentials.json` del estado de la corrida que apunta a este archivo.
        credencial_enlace: Option<PathBuf>,
        /// La revisión de vigencia falla, como un login que vence antes del plazo.
        vigencia_falla: bool,
        /// Desde esta llamada (base 1) a `verificar_antes` la revisión falla.
        vigencia_desde: Option<u32>,
        /// Cuántas llamadas fallan desde `vigencia_desde`. Sin valor, fallan todas las que siguen.
        vigencia_recupera_tras: Option<u32>,
        /// Llamadas hechas a `verificar_antes`.
        vigencia_llamadas: Cell<u32>,
        /// Plazo que `correr` pidió en `verificar_antes`.
        plazo_verificado: RefCell<Option<Duration>>,
        /// Vida del login que devuelve `vida_restante`. Sin valor, el ejecutor no conoce límite.
        vida: Cell<Option<Duration>>,
        /// Vida del login después del primer lanzamiento, como un login que se consume al correr.
        vida_tras_lanzar: Option<Duration>,
        lanzados: RefCell<Vec<Lanzamiento>>,
        /// Al lanzar, deja en `.git/config` un `fsmonitor` que escribe esta marca, y un archivo sin commit.
        fsmonitor_marca: Option<PathBuf>,
        /// Lanzamientos que hizo antes una corrida anterior. El `session_id` falso sigue desde aquí.
        sesion_desde: u32,
    }

    impl EjecutorBuilder for Builder {
        fn verificar_antes(&self, plazo: Duration) -> Result<(), ErrorCorre> {
            *self.plazo_verificado.borrow_mut() = Some(plazo);
            let llamada = self.vigencia_llamadas.get() + 1;
            self.vigencia_llamadas.set(llamada);
            let en_racha = self.vigencia_desde.is_some_and(|desde| {
                llamada >= desde
                    && self
                        .vigencia_recupera_tras
                        .is_none_or(|m| llamada < desde + m)
            });
            // Un login con menos vida que el plazo no alcanza, como en el host.
            let vida_corta = self.vida.get().is_some_and(|vida| vida < plazo);
            if self.vigencia_falla || en_racha || vida_corta {
                return Err(ErrorCorre::Ejecutor("prueba: el login vence".into()));
            }
            Ok(())
        }

        fn vida_restante(&self) -> Result<Option<Duration>, ErrorCorre> {
            Ok(self.vida.get())
        }

        fn espera_vigencia(&self) -> EsperaVigencia {
            EsperaVigencia {
                intervalo: Duration::from_millis(1),
                tope: Duration::from_millis(5),
            }
        }

        fn lanzar(&self, l: &Lanzamiento) -> Result<SalidaClaude, ErrorCorre> {
            self.lanzados.borrow_mut().push(l.clone());
            // Tras el primer lanzamiento, el login queda con la vida de `vida_tras_lanzar`.
            let primero = self.lanzados.borrow().len() == 1;
            if let Some(tras) = self.vida_tras_lanzar.filter(|_| primero) {
                self.vida.set(Some(tras));
            }
            if self.no_lanza {
                return Err(ErrorCorre::Ejecutor("prueba: no arranca".into()));
            }
            if let Some(marca) = &self.fsmonitor_marca {
                use std::io::Write;
                // El builder escribe su propio `.git/config` con un fsmonitor, y deja un archivo sin commit.
                let mut config = fs::OpenOptions::new()
                    .append(true)
                    .open(l.repo.join(".git").join("config"))
                    .expect("config del repo");
                writeln!(
                    config,
                    "[core]\n\tfsmonitor = \"touch {}\"",
                    marca.display()
                )
                .expect("escribe la config");
                fs::write(l.repo.join("sucio.txt"), "sin commit\n").expect("archivo sin commit");
            }
            let mut con_extra = l.clone();
            con_extra.env.extend(self.extra.iter().cloned());
            let mut salida = EjecutorFalso.lanzar(&con_extra)?;
            if self.falla == Some(l.rol) {
                salida.codigo = 1;
            }
            if let Some(texto) = self.credencial {
                let estado = estado_de(&l.repo).expect("estado de la corrida");
                fs::create_dir_all(&estado).expect("carpeta de estado");
                fs::write(estado.join(".credentials.json"), texto).expect("credencial");
            }
            if let Some(destino) = &self.credencial_enlace {
                let estado = estado_de(&l.repo).expect("estado de la corrida");
                let ruta = estado.join(".credentials.json");
                fs::create_dir_all(&estado).expect("carpeta de estado");
                // Cada rol lanza otra vez: el enlace del rol anterior se quita antes de crear el nuevo.
                let _ = fs::remove_file(&ruta);
                std::os::unix::fs::symlink(destino, &ruta).expect("enlace de credencial");
            }
            if self.falla_tras_montar {
                return Err(ErrorCorre::Ejecutor(
                    "prueba: falla después de montar".into(),
                ));
            }
            if let Some(fuera) = &self.estado_fuera {
                let corrida = l.repo.parent().expect("carpeta de la corrida");
                let estado = estado_de(&l.repo).expect("estado de la corrida");
                borrado::borrar_dir_dentro(corrida, &estado).expect("quitar el estado previo");
                fs::create_dir_all(fuera).expect("carpeta de afuera");
                fs::write(fuera.join(".credentials.json"), "").expect("credencial de afuera");
                std::os::unix::fs::symlink(fuera, &estado).expect("enlace de estado");
            }
            if self.sin_resultado || self.sin_resultado_en == Some(l.rol) {
                let texto = String::from_utf8_lossy(&salida.stdout).into_owned();
                let sin: Vec<&str> = texto
                    .lines()
                    .filter(|linea| !linea.contains("\"type\":\"result\""))
                    .collect();
                salida.stdout = format!("{}\n", sin.join("\n")).into_bytes();
            }
            if self.token_vencido {
                let vencido = "{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":true,\"api_error_status\":401,\"result\":\"Failed to authenticate. API Error: 401 OAuth token has expired\"}\n";
                salida.stdout = vencido.as_bytes().to_vec();
            }
            if self.costo_desconocido {
                let sin_costo = "{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"duration_ms\":1000,\"num_turns\":2,\"modelUsage\":{\"opus\":{\"inputTokens\":10}},\"session_id\":\"s1\"}\n";
                salida.stdout = sin_costo.as_bytes().to_vec();
            }
            // Cada sesión real tiene su `session_id`. El falso repite `s1`, y el dedupe lo tomaría por
            // la misma sesión. Por eso cada lanzamiento lleva un id propio, con su número.
            let n = self.sesion_desde as usize + self.lanzados.borrow().len();
            let texto = String::from_utf8_lossy(&salida.stdout).replace(
                "\"session_id\":\"s1\"",
                &format!("\"session_id\":\"s1-{n}\""),
            );
            salida.stdout = texto.into_bytes();
            Ok(salida)
        }
    }

    fn informe() -> Informe {
        Informe {
            decision: Decision::Listo,
            motivo: "prueba".into(),
            requisitos: Vec::new(),
            bloqueantes: Vec::new(),
            riesgos: Vec::new(),
            proximos_pasos: Vec::new(),
            resumen: "Prueba.".into(),
            confianza: Confianza::default(),
        }
    }

    /// Evaluador falso: devuelve un veredicto con informe armado a mano.
    fn evaluador_falso(
        _repo: &Path,
        tarea: &Tarea,
        _o: &OpcionesOraculo,
    ) -> Result<Veredicto, ErrorOraculo> {
        Ok(Veredicto {
            tarea: tarea.nombre.clone(),
            estado: Estado::Verde,
            informe: Some(informe()),
            ..Veredicto::default()
        })
    }

    fn evaluador_roto(
        _repo: &Path,
        _tarea: &Tarea,
        _o: &OpcionesOraculo,
    ) -> Result<Veredicto, ErrorOraculo> {
        Err(ErrorOraculo::Infra("sin podman en la prueba".into()))
    }

    fn correr_con(
        e: &Entorno,
        ejecutor: &dyn EjecutorBuilder,
        entorno: &[(String, String)],
        config: &str,
    ) -> Result<Fila, ErrorCorre> {
        correr_con_eval(e, ejecutor, entorno, config, &evaluador_falso)
    }

    fn correr_con_eval(
        e: &Entorno,
        ejecutor: &dyn EjecutorBuilder,
        entorno: &[(String, String)],
        config: &str,
        evaluar: &dyn Fn(&Path, &Tarea, &OpcionesOraculo) -> Result<Veredicto, ErrorOraculo>,
    ) -> Result<Fila, ErrorCorre> {
        let deps = DepsCorre {
            home: &e.home,
            home_usuario: &e.usuario,
            repo_root: &e.repo,
            ejecutor,
            evaluar,
            entorno,
        };
        correr(&deps, TAREA, config)
    }

    /// Eventos de `eventos.jsonl` de `home`, en orden de escritura.
    fn eventos(e: &Entorno) -> Vec<Evento> {
        let texto = fs::read_to_string(e.home.join("eventos.jsonl")).unwrap_or_default();
        texto
            .lines()
            .map(|linea| serde_json::from_str(linea).expect("evento válido"))
            .collect()
    }

    fn gates(e: &Entorno, fase: &str) -> Vec<String> {
        eventos(e)
            .iter()
            .filter(|ev| ev.tipo == TipoEvento::Gate && ev.fase == fase)
            .filter_map(|ev| ev.extra.get("motivo").and_then(Value::as_str))
            .map(str::to_string)
            .collect()
    }

    fn hay_lecturas(e: &Entorno) -> bool {
        eventos(e)
            .iter()
            .any(|ev| ev.tipo == TipoEvento::OraculoLeido)
    }

    fn entorno_con_secretos(e: &Entorno) -> Vec<(String, String)> {
        vec![
            ("PATH".into(), "/usr/bin:/bin".into()),
            ("LANG".into(), "C".into()),
            ("DEEPSEEK_API_KEY".into(), "clave-1".into()),
            ("ANTHROPIC_API_KEY".into(), "clave-2".into()),
            ("ROOT_HOME".into(), e.home.display().to_string()),
            ("HOME".into(), e.usuario.display().to_string()),
        ]
    }

    #[test]
    fn corre_mecanica_deja_fila_con_informe_resumen_y_forma() {
        let e = entorno();
        let fila = correr_con(&e, &Builder::default(), &[], CONFIG).expect("corre");
        let filas = track::filas_de(&e.home.join("runs.jsonl"), &Filtro::default()).expect("runs");
        assert_eq!(filas.len(), 1, "una fila por corrida");
        let guardada = &filas[0];
        assert_eq!(guardada.id, fila.id);
        assert_eq!(guardada.tarea, TAREA);
        assert_eq!(guardada.config, CONFIG);
        assert!(guardada.veredicto.informe.is_some(), "falta el informe");
        // Cada rol da dos turnos del `result` del falso, y un costo de cupo de 0,01 USD.
        assert_eq!(guardada.resumen.turnos, 4);
        assert!(
            (guardada.resumen.usd_cupo - 0.02).abs() < 1e-9,
            "{:?}",
            guardada.resumen
        );
        assert!(!guardada.resumen.invalida);
        assert_eq!(
            guardada.forma,
            Forma {
                roles: vec![Rol::Planifica, Rol::Construye],
                verificador: false,
                config: CONFIG.into(),
            }
        );
        assert_eq!(guardada.modelos[&Rol::Planifica], "opus");
        assert_eq!(guardada.modelos[&Rol::Construye], "haiku");
    }

    #[test]
    fn corre_goal_alterado_no_arranca_ni_crea_nada() {
        let e = entorno();
        fs::write(e.repo.join("GOAL.hash"), "0000000000000000\n").expect("hash malo");
        let builder = Builder::default();
        let error = correr_con(&e, &builder, &[], CONFIG).expect_err("GOAL alterado");
        assert!(
            matches!(error, ErrorCorre::Goal(ErrorGoal::Alterado { .. })),
            "{error}"
        );
        assert_eq!(error.codigo_de_salida(), 1);
        assert!(builder.lanzados.borrow().is_empty(), "no debe lanzar");
        assert!(!dir_corridas(&e.usuario).exists(), "no debe crear corridas");
        assert!(
            !e.home.join("runs.jsonl").exists(),
            "no debe escribir runs.jsonl"
        );
    }

    #[test]
    fn corre_goal_sin_archivo_sale_con_3() {
        let e = entorno();
        fs::remove_file(e.repo.join("GOAL.md")).expect("quita GOAL.md");
        let error = correr_con(&e, &Builder::default(), &[], CONFIG).expect_err("sin GOAL");
        assert_eq!(error.codigo_de_salida(), 3, "{error}");
        assert!(!dir_corridas(&e.usuario).exists());
    }

    #[test]
    fn corre_lectura_del_oraculo_invalida_la_corrida() {
        let e = entorno();
        let oculto = e
            .home
            .join("oraculos")
            .join(TAREA)
            .join("oculto")
            .join("test_y.py");
        let builder = Builder {
            extra: vec![(
                "CLAUDE_FALSO_LEE_ORACULO".into(),
                oculto.display().to_string(),
            )],
            ..Builder::default()
        };
        let fila = correr_con(&e, &builder, &[], CONFIG).expect("corre");
        assert!(fila.resumen.invalida, "la lectura debe invalidar");
        let leidos: Vec<Evento> = eventos(&e)
            .into_iter()
            .filter(|ev| ev.tipo == TipoEvento::OraculoLeido)
            .collect();
        assert!(!leidos.is_empty(), "falta OraculoLeido");
        assert!(leidos.iter().all(|ev| ev.corrida == fila.id));
        let guardada =
            track::filas_de(&e.home.join("runs.jsonl"), &Filtro::default()).expect("runs");
        assert!(
            guardada[0].resumen.invalida,
            "la fila guardada debe quedar inválida"
        );
    }

    #[test]
    fn corre_transcript_limpio_no_emite_lecturas() {
        let e = entorno();
        let fila = correr_con(&e, &Builder::default(), &[], CONFIG).expect("corre");
        assert!(!fila.resumen.invalida);
        assert!(!hay_lecturas(&e), "un transcript limpio no emite nada");
    }

    #[test]
    fn corre_rol_con_codigo_distinto_de_cero_corta_los_siguientes() {
        let e = entorno();
        let builder = Builder {
            falla: Some(Rol::Planifica),
            ..Builder::default()
        };
        let fila = correr_con(&e, &builder, &[], CONFIG).expect("la corrida deja fila");
        let roles: Vec<Rol> = builder.lanzados.borrow().iter().map(|l| l.rol).collect();
        assert_eq!(roles, vec![Rol::Planifica], "construye no debe correr");
        assert_eq!(
            gates(&e, "rol"),
            vec!["rol planifica salió con 1".to_string()]
        );
        assert!(
            !eventos(&e)
                .iter()
                .any(|ev| ev.tipo == TipoEvento::PlanAprobado),
            "sin plan no hay aprobación"
        );
        let guardada =
            track::filas_de(&e.home.join("runs.jsonl"), &Filtro::default()).expect("runs");
        assert_eq!(guardada.len(), 1);
        assert_eq!(guardada[0].id, fila.id);
    }

    #[test]
    fn corre_plan_aprobado_queda_despues_de_planifica() {
        let e = entorno();
        let fila = correr_con(&e, &Builder::default(), &[], CONFIG_PLAN).expect("corre");
        let ev = eventos(&e);
        let aprobados: Vec<usize> = ev
            .iter()
            .enumerate()
            .filter(|(_, x)| x.tipo == TipoEvento::PlanAprobado && x.corrida == fila.id)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(
            aprobados.len(),
            1,
            "una aprobación por corrida con planifica"
        );
        let aprobado = &ev[aprobados[0]];
        assert_eq!(aprobado.extra["detalle"], PLAN_APROBADO);
        let sesion = ev
            .iter()
            .position(|x| x.tipo == TipoEvento::Sesion && x.corrida == fila.id)
            .expect("la sesión de planifica");
        assert!(
            aprobados[0] > sesion,
            "la aprobación va después de la sesión de planifica"
        );
    }

    #[test]
    fn corre_plan_aprobado_es_automatico() {
        let e = entorno();
        let fila = correr_con(&e, &Builder::default(), &[], CONFIG_PLAN).expect("corre");
        let aprobado = eventos(&e)
            .into_iter()
            .find(|x| x.tipo == TipoEvento::PlanAprobado && x.corrida == fila.id)
            .expect("hay PlanAprobado");
        assert_eq!(
            aprobado.extra.get("automatica"),
            Some(&json!(true)),
            "la aprobación de r00t corre se marca como automática: {:?}",
            aprobado.extra
        );
    }

    #[test]
    fn corre_sin_planifica_no_hay_plan_aprobado() {
        let e = entorno();
        let fila = correr_con(&e, &Builder::default(), &[], CONFIG_CONSTRUYE).expect("corre");
        assert!(
            !eventos(&e)
                .iter()
                .any(|x| x.tipo == TipoEvento::PlanAprobado && x.corrida == fila.id)
        );
    }

    #[test]
    fn corre_builder_no_recibe_root_home_ni_claves() {
        let e = entorno();
        let builder = Builder::default();
        let entorno_proceso = entorno_con_secretos(&e);
        let fila = correr_con(&e, &builder, &entorno_proceso, CONFIG).expect("corre");
        let lanzados = builder.lanzados.borrow();
        assert_eq!(lanzados.len(), 2, "cada rol corre");
        for l in lanzados.iter() {
            for prohibida in ["DEEPSEEK_API_KEY", "ANTHROPIC_API_KEY", "ROOT_HOME", "HOME"] {
                assert!(
                    l.env.iter().all(|(k, _)| k != prohibida),
                    "{prohibida} llega al builder: {:?}",
                    l.env
                );
            }
        }
        assert!(
            gates(&e, "rol").is_empty(),
            "ningún rol debe salir con código distinto de 0"
        );
        assert_eq!(fila.forma.roles.len(), 2);
    }

    #[test]
    fn corre_cada_rol_lleva_su_plugin() {
        let e = entorno();
        let plugin_planifica = e.usuario.join("ecc-planifica");
        let plugin_construye = e.usuario.join("ecc-construye");
        let texto = format!(
            "roles = [\"planifica\", \"construye\"]\n[prompt]\nplanifica = \"haz el plan\"\nconstruye = \"haz hecho\"\n[claude.rol.planifica]\nplugin_dir = \"{}\"\n[claude.rol.construye]\nplugin_dir = \"{}\"\n",
            plugin_planifica.display(),
            plugin_construye.display()
        );
        fs::write(e.home.join("configs").join("ecc-plugin.toml"), texto).expect("config");
        let builder = Builder::default();
        correr_con(&e, &builder, &[], "ecc-plugin").expect("corre");
        let lanzados = builder.lanzados.borrow();
        assert_eq!(lanzados.len(), 2, "cada rol corre");
        assert_eq!(
            lanzados[0].plugin,
            Some(plugin_planifica),
            "{:?}",
            lanzados[0]
        );
        assert_eq!(
            lanzados[1].plugin,
            Some(plugin_construye),
            "{:?}",
            lanzados[1]
        );
        assert!(
            lanzados[1]
                .args
                .windows(2)
                .any(|p| p[0] == "--plugin-dir" && p[1] == "/opt/plugin"),
            "{:?}",
            lanzados[1].args
        );
    }

    #[test]
    fn corre_carpeta_de_corrida_bajo_usuario_y_transcript_bajo_home() {
        let e = entorno();
        let fila = correr_con(&e, &Builder::default(), &[], CONFIG).expect("corre");
        let dir = dir_corridas(&e.usuario).join(&fila.id);
        assert!(dir.join("repo").join("hecho.txt").is_file());
        assert!(
            !dir.starts_with(&e.home),
            "la corrida no cae dentro de ROOT_HOME"
        );
        let transcripts = e.home.join("transcripts").join(&fila.id);
        assert!(transcripts.join("1-planifica.jsonl").is_file());
        assert!(transcripts.join("2-construye.jsonl").is_file());
    }

    /// Ruta de `.credentials.json` en el estado de la corrida `fila_id`, bajo `usuario`.
    fn credencial_de_corrida(usuario: &Path, fila_id: &str) -> PathBuf {
        let repo = dir_corridas(usuario).join(fila_id).join("repo");
        estado_de(&repo)
            .expect("estado de la corrida")
            .join(".credentials.json")
    }

    #[test]
    fn corre_credencial_vacia_del_montaje_se_borra_al_terminar() {
        let e = entorno();
        let builder = Builder {
            credencial: Some(""),
            ..Builder::default()
        };
        let fila = correr_con(&e, &builder, &[], CONFIG).expect("corre");
        let credencial = credencial_de_corrida(&e.usuario, &fila.id);
        assert!(
            !credencial.exists(),
            "la credencial vacía debe borrarse: {}",
            credencial.display()
        );
    }

    #[test]
    fn corre_credencial_no_vacia_se_borra_al_terminar() {
        let e = entorno();
        let builder = Builder {
            credencial: Some("{\"token\":\"de-prueba\"}"),
            ..Builder::default()
        };
        let fila = correr_con(&e, &builder, &[], CONFIG).expect("corre");
        let credencial = credencial_de_corrida(&e.usuario, &fila.id);
        assert!(
            !credencial.exists(),
            "la copia de la credencial es de Root y se borra al terminar: {}",
            credencial.display()
        );
    }

    #[test]
    fn corre_barre_la_copia_de_una_corrida_muerta_al_arrancar() {
        let e = entorno();
        let id = "corre-memo-ecc-test-1-4194999-0";
        let copia = dir_corridas(&e.usuario)
            .join(id)
            .join("claude-estado")
            .join(".credentials.json");
        fs::create_dir_all(copia.parent().expect("estado")).expect("estado");
        fs::write(&copia, "TOKEN_FALSO_PRUEBA").expect("copia previa");
        correr_con(&e, &Builder::default(), &[], CONFIG).expect("corre");
        assert!(!copia.exists(), "la copia de la corrida muerta se borra");
        let barridos: Vec<Evento> = eventos(&e)
            .into_iter()
            .filter(|ev| ev.tipo == TipoEvento::Gate && ev.fase == "barrido")
            .collect();
        assert_eq!(barridos.len(), 1, "un Gate de barrido");
        assert_eq!(barridos[0].corrida, id);
        let texto = fs::read_to_string(e.home.join("eventos.jsonl")).unwrap_or_default();
        assert!(
            !texto.contains("TOKEN_FALSO_PRUEBA"),
            "el Gate no lleva el token"
        );
    }

    #[test]
    fn corre_enlace_de_credencial_se_borra_sin_seguir_su_destino() {
        let e = entorno();
        let destino = e._tmp.path().join("destino.json");
        fs::write(&destino, "{\"token\":\"ajeno\"}").expect("destino");
        let builder = Builder {
            credencial_enlace: Some(destino.clone()),
            ..Builder::default()
        };
        let fila = correr_con(&e, &builder, &[], CONFIG).expect("corre");
        let credencial = credencial_de_corrida(&e.usuario, &fila.id);
        assert!(
            fs::symlink_metadata(&credencial).is_err(),
            "el enlace debe borrarse: {}",
            credencial.display()
        );
        assert_eq!(
            fs::read_to_string(&destino).expect("destino"),
            "{\"token\":\"ajeno\"}",
            "el destino del enlace no se toca"
        );
    }

    #[test]
    fn corre_vigencia_vencida_sale_antes_de_lanzar_y_no_crea_nada() {
        let e = entorno();
        let builder = Builder {
            vigencia_falla: true,
            ..Builder::default()
        };
        let error = correr_con(&e, &builder, &[], CONFIG).expect_err("el login vence");
        assert!(matches!(error, ErrorCorre::Ejecutor(_)), "{error}");
        assert_eq!(error.codigo_de_salida(), 3);
        assert!(builder.lanzados.borrow().is_empty(), "no debe lanzar nada");
        assert!(
            !dir_corridas(&e.usuario).exists(),
            "no debe crear la carpeta de la corrida"
        );
        assert!(!e.home.join("runs.jsonl").exists(), "sin fila");
    }

    #[test]
    fn corre_vigencia_pide_el_plazo_de_un_rol() {
        let e = entorno();
        let builder = Builder::default();
        correr_con(&e, &builder, &[], CONFIG).expect("corre");
        assert_eq!(
            *builder.plazo_verificado.borrow(),
            Some(VIGENCIA_MINIMA),
            "la vigencia mínima es la de arrancar un rol, no el plazo de todos"
        );
    }

    /// Login de una hora: el rol corre con su vida menos el margen, no con las dos horas.
    #[test]
    fn corre_con_una_hora_de_login_lanza_con_plazo_de_55_minutos() {
        let e = entorno();
        let builder = Builder {
            vida: Cell::new(Some(Duration::from_secs(60 * 60))),
            ..Builder::default()
        };
        correr_con(&e, &builder, &[], CONFIG_CONSTRUYE).expect("la corrida termina bien");
        let lanzados = builder.lanzados.borrow();
        assert_eq!(lanzados.len(), 1, "el rol arranca");
        assert_eq!(lanzados[0].plazo, Duration::from_secs(55 * 60));
    }

    /// Login de 3 horas que queda en 10 minutos tras el primer rol: sin renovación, el segundo no arranca.
    #[test]
    fn corre_con_diez_minutos_de_login_espera_y_sale_con_error() {
        let e = entorno();
        let builder = Builder {
            vida: Cell::new(Some(Duration::from_secs(3 * 60 * 60))),
            vida_tras_lanzar: Some(Duration::from_secs(10 * 60)),
            ..Builder::default()
        };
        let error = correr_con(&e, &builder, &[], CONFIG_PLAN_ORGANIZA)
            .expect_err("el login no se renueva");
        assert!(matches!(error, ErrorCorre::Ejecutor(_)), "{error}");
        assert_eq!(error.codigo_de_salida(), 3);
        let roles: Vec<Rol> = builder.lanzados.borrow().iter().map(|l| l.rol).collect();
        assert_eq!(roles, vec![Rol::Planifica], "organiza no debe lanzarse");
        assert_eq!(gates(&e, "vigencia").len(), 1, "un Gate de vigencia");
    }

    /// Login de 10 horas: el plazo del rol queda en el tope de dos horas.
    #[test]
    fn corre_con_vida_larga_lanza_con_plazo_de_dos_horas() {
        let e = entorno();
        let builder = Builder {
            vida: Cell::new(Some(Duration::from_secs(10 * 60 * 60))),
            ..Builder::default()
        };
        correr_con(&e, &builder, &[], CONFIG_CONSTRUYE).expect("corre");
        assert_eq!(builder.lanzados.borrow()[0].plazo, TIMEOUT_BUILDER);
    }

    #[test]
    fn plazo_de_lanzamiento_resta_el_margen_a_la_vida_del_login() {
        let minutos = |m: u64| Duration::from_secs(m * 60);
        assert_eq!(plazo_de_lanzamiento(Some(minutos(60))), minutos(55));
        assert_eq!(plazo_de_lanzamiento(Some(minutos(20))), minutos(15));
        assert_eq!(
            plazo_de_lanzamiento(Some(minutos(10 * 60))),
            TIMEOUT_BUILDER
        );
        assert_eq!(
            plazo_de_lanzamiento(Some(TIMEOUT_BUILDER + minutos(5))),
            TIMEOUT_BUILDER,
            "el tope de dos horas manda sobre una vida mayor"
        );
        assert_eq!(plazo_de_lanzamiento(None), TIMEOUT_BUILDER);
    }

    /// Reintentos de la espera de prueba: su tope de 5 ms entre intervalos de 1 ms.
    const REINTENTOS_DE_PRUEBA: u32 = 5;

    #[test]
    fn corre_vigencia_falla_antes_del_primer_rol_sale_sin_lanzar_ni_esperar() {
        let e = entorno();
        let builder = Builder {
            vigencia_falla: true,
            ..Builder::default()
        };
        let error = correr_con(&e, &builder, &[], CONFIG).expect_err("el login vence");
        assert!(matches!(error, ErrorCorre::Ejecutor(_)), "{error}");
        assert_eq!(error.codigo_de_salida(), 3);
        assert!(
            builder.lanzados.borrow().is_empty(),
            "ningún rol debe lanzarse"
        );
        assert_eq!(
            builder.vigencia_llamadas.get(),
            1,
            "sin espera: una sola revisión"
        );
        assert_eq!(*builder.plazo_verificado.borrow(), Some(VIGENCIA_MINIMA));
        assert!(
            gates(&e, "vigencia").is_empty(),
            "antes del primer rol no hay espera"
        );
    }

    #[test]
    fn corre_vigencia_se_recupera_entre_roles_y_sigue() {
        let e = entorno();
        let builder = Builder {
            vigencia_desde: Some(2),
            vigencia_recupera_tras: Some(2),
            ..Builder::default()
        };
        correr_con(&e, &builder, &[], CONFIG).expect("la corrida termina bien");
        let roles: Vec<Rol> = builder.lanzados.borrow().iter().map(|l| l.rol).collect();
        assert_eq!(roles, vec![Rol::Planifica, Rol::Construye]);
        assert_eq!(
            builder.vigencia_llamadas.get(),
            4,
            "una de planifica, dos fallidas y una buena de construye"
        );
        let avisos: Vec<Evento> = eventos(&e)
            .into_iter()
            .filter(|ev| ev.tipo == TipoEvento::Gate && ev.fase == "vigencia")
            .collect();
        assert_eq!(avisos.len(), 1, "un Gate al fallar, no uno por reintento");
        assert_eq!(
            avisos[0].extra.get("rol").and_then(Value::as_str),
            Some("construye")
        );
    }

    #[test]
    fn corre_vigencia_no_se_recupera_entre_roles_sale_con_3() {
        let e = entorno();
        let builder = Builder {
            vigencia_desde: Some(2),
            ..Builder::default()
        };
        let error = correr_con(&e, &builder, &[], CONFIG).expect_err("el login no se recupera");
        assert!(matches!(error, ErrorCorre::Ejecutor(_)), "{error}");
        assert_eq!(error.codigo_de_salida(), 3);
        let roles: Vec<Rol> = builder.lanzados.borrow().iter().map(|l| l.rol).collect();
        assert_eq!(roles, vec![Rol::Planifica], "construye no debe lanzarse");
        assert_eq!(gates(&e, "vigencia").len(), 1, "un Gate de vigencia");
        assert_eq!(
            builder.vigencia_llamadas.get(),
            2 + REINTENTOS_DE_PRUEBA,
            "una de planifica, una de construye y ceil(tope/intervalo) reintentos"
        );
        assert!(!e.home.join("runs.jsonl").exists(), "sin fila");
    }

    /// Carpeta de la única corrida que hay bajo `usuario`.
    fn unica_corrida(usuario: &Path) -> PathBuf {
        let corridas: Vec<PathBuf> = fs::read_dir(dir_corridas(usuario))
            .expect("carpeta de corridas")
            .map(|entrada| entrada.expect("entrada").path())
            .collect();
        assert_eq!(corridas.len(), 1, "una sola corrida: {corridas:?}");
        corridas[0].clone()
    }

    #[test]
    fn corre_fallo_del_ejecutor_despues_de_montar_la_credencial_no_deja_la_credencial_vacia() {
        let e = entorno();
        let builder = Builder {
            credencial: Some(""),
            falla_tras_montar: true,
            ..Builder::default()
        };
        let error = correr_con(&e, &builder, &[], CONFIG).expect_err("falla tras montar");
        assert!(matches!(error, ErrorCorre::Ejecutor(_)), "{error}");
        let credencial = estado_de(&unica_corrida(&e.usuario).join("repo"))
            .expect("estado de la corrida")
            .join(".credentials.json");
        assert!(
            !credencial.exists(),
            "la credencial vacía no debe quedar tras un fallo: {}",
            credencial.display()
        );
    }

    #[test]
    fn corre_fallo_del_ejecutor_con_credencial_no_vacia_se_borra_al_terminar() {
        let e = entorno();
        let builder = Builder {
            credencial: Some("{\"token\":\"de-prueba\"}"),
            falla_tras_montar: true,
            ..Builder::default()
        };
        let error = correr_con(&e, &builder, &[], CONFIG).expect_err("falla tras montar");
        assert!(matches!(error, ErrorCorre::Ejecutor(_)), "{error}");
        let credencial = estado_de(&unica_corrida(&e.usuario).join("repo"))
            .expect("estado de la corrida")
            .join(".credentials.json");
        assert!(
            !credencial.exists(),
            "la copia de Root no debe quedar tras un fallo: {}",
            credencial.display()
        );
    }

    #[test]
    fn corre_error_al_borrar_la_copia_de_la_credencial_es_error_de_io() {
        let e = entorno();
        let fuera = e._tmp.path().join("fuera");
        let builder = Builder {
            estado_fuera: Some(fuera.clone()),
            ..Builder::default()
        };
        let error = correr_con(&e, &builder, &[], CONFIG).expect_err("no se puede borrar");
        assert!(
            matches!(
                error,
                ErrorCorre::Io { ref contexto, .. } if contexto.contains("borrar la copia de la credencial")
            ),
            "{error}"
        );
        assert!(
            fuera.join(".credentials.json").exists(),
            "Root no borra fuera de la carpeta de la corrida"
        );
    }

    #[test]
    fn corre_rol_que_va_por_api_es_error_de_uso_sin_lanzar() {
        let e = entorno();
        let builder = Builder::default();
        let error = correr_con(&e, &builder, &[], CONFIG_API).expect_err("delega va por API");
        assert!(
            matches!(
                error,
                ErrorCorre::Uso(ref m)
                    if m.contains("el rol delega va por api: r00t corre solo lanza claude -p por cupo")
            ),
            "{error}"
        );
        assert_eq!(error.codigo_de_salida(), 2);
        assert!(builder.lanzados.borrow().is_empty(), "no debe lanzar nada");
        assert!(
            !dir_corridas(&e.usuario).exists(),
            "no debe crear la carpeta"
        );
    }

    #[test]
    fn corre_error_de_evaluacion_no_pierde_la_fila() {
        let e = entorno();
        let fila = correr_con_eval(&e, &Builder::default(), &[], CONFIG, &evaluador_roto)
            .expect("la fila se escribe igual");
        assert_eq!(fila.veredicto.estado, Estado::SinVeredicto);
        assert!(fila.veredicto.informe.is_none());
        let guardadas =
            track::filas_de(&e.home.join("runs.jsonl"), &Filtro::default()).expect("runs");
        assert_eq!(guardadas.len(), 1);
        assert!(
            gates(&e, "evaluacion")
                .iter()
                .any(|m| m.contains("sin podman en la prueba")),
            "falta el Gate de evaluación"
        );
    }

    /// Corre con un evaluador que cuenta sus llamadas. Devuelve el error y la cuenta.
    fn correr_contando_evaluaciones(e: &Entorno, builder: &Builder) -> (ErrorCorre, usize) {
        let evaluaciones = std::cell::Cell::new(0);
        let evaluar = |r: &Path, t: &Tarea, o: &OpcionesOraculo| {
            evaluaciones.set(evaluaciones.get() + 1);
            evaluador_falso(r, t, o)
        };
        let error = correr_con_eval(e, builder, &[], CONFIG, &evaluar)
            .expect_err("un rol sin resultado exitoso no deja fila");
        (error, evaluaciones.get())
    }

    #[test]
    fn corre_rol_sin_resultado_no_evalua_ni_escribe_fila() {
        let e = entorno();
        let builder = Builder {
            sin_resultado: true,
            ..Builder::default()
        };
        let (error, evaluadas) = correr_contando_evaluaciones(&e, &builder);
        assert!(matches!(error, ErrorCorre::Ejecutor(_)), "{error}");
        assert_eq!(evaluadas, 0, "no debe evaluar");
        assert!(!e.home.join("runs.jsonl").exists(), "sin fila");
        assert_eq!(
            builder.lanzados.borrow().len(),
            1,
            "corta antes de construye"
        );
        let motivos = gates(&e, FASE_ROL);
        assert_eq!(motivos.len(), 1, "un Gate del rol");
        assert!(
            motivos[0].contains("planifica") && motivos[0].contains("sin línea result"),
            "{motivos:?}"
        );
    }

    #[test]
    fn corre_rol_con_token_vencido_no_evalua_y_nombra_el_login() {
        let e = entorno();
        let builder = Builder {
            token_vencido: true,
            ..Builder::default()
        };
        let (error, evaluadas) = correr_contando_evaluaciones(&e, &builder);
        assert!(matches!(error, ErrorCorre::Ejecutor(_)), "{error}");
        assert_eq!(evaluadas, 0, "no debe evaluar");
        assert!(!e.home.join("runs.jsonl").exists(), "sin fila");
        let motivos = gates(&e, FASE_ROL);
        assert_eq!(motivos.len(), 1, "un Gate del rol");
        assert!(motivos[0].contains("login de Claude venció"), "{motivos:?}");
    }

    /// La sesión de un rol bueno queda registrada aunque el rol siguiente aborte la corrida.
    #[test]
    fn corre_sesion_del_rol_bueno_queda_aunque_el_siguiente_aborte() {
        let e = entorno();
        let builder = Builder {
            sin_resultado_en: Some(Rol::Organiza),
            ..Builder::default()
        };
        let error = correr_con(&e, &builder, &[], CONFIG_PLAN_ORGANIZA)
            .expect_err("organiza sin result aborta");
        assert!(matches!(error, ErrorCorre::Ejecutor(_)), "{error}");
        let log = eventos(&e);
        let llamadas: Vec<&Evento> = log
            .iter()
            .filter(|ev| ev.tipo == TipoEvento::Llamada)
            .collect();
        assert!(!llamadas.is_empty(), "planifica deja su Llamada");
        let costo: f64 = llamadas.iter().map(|ev| ev.cost_usd).sum();
        assert!(costo > 0.0, "la Llamada lleva costo: {costo}");
        let sesion_de_planifica = log.iter().any(|ev| {
            ev.tipo == TipoEvento::Sesion
                && ev.extra.get("session_id").and_then(Value::as_str) == Some("s1-1")
        });
        assert!(sesion_de_planifica, "queda la Sesion de planifica");
        assert_eq!(gates(&e, FASE_ROL).len(), 1, "un Gate de organiza");
    }

    #[test]
    fn resultado_exitoso_acepta_success_sin_error() {
        let salida = b"{\"type\":\"system\"}\n{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false}\n";
        let linea = resultado_exitoso(salida).expect("resultado exitoso");
        assert!(linea.contains("\"subtype\":\"success\""), "{linea}");
    }

    #[test]
    fn resultado_exitoso_rechaza_salida_sin_result() {
        let motivo = resultado_exitoso(b"{\"type\":\"system\"}\n").expect_err("sin result");
        assert!(motivo.contains("sin línea result"), "{motivo}");
    }

    #[test]
    fn resultado_exitoso_rechaza_subtype_distinto_de_success() {
        let salida = b"{\"type\":\"result\",\"subtype\":\"error_max_turns\",\"is_error\":false}\n";
        let motivo = resultado_exitoso(salida).expect_err("subtype de error");
        assert!(motivo.contains("error_max_turns"), "{motivo}");
    }

    #[test]
    fn resultado_exitoso_rechaza_is_error_true() {
        let salida = b"{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":true}\n";
        resultado_exitoso(salida).expect_err("is_error true no es exitoso");
    }

    #[test]
    fn resultado_exitoso_con_token_vencido_dice_que_el_login_vencio() {
        let salida = b"{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":true,\"api_error_status\":401,\"result\":\"Invalid API key\"}\n";
        let motivo = resultado_exitoso(salida).expect_err("login vencido");
        assert!(motivo.contains("login de Claude venció"), "{motivo}");
        assert!(
            !motivo.contains("Invalid API key"),
            "no copia el texto: {motivo}"
        );
    }

    #[test]
    fn corre_costo_desconocido_deja_gate_de_la_sesion() {
        let e = entorno();
        let builder = Builder {
            costo_desconocido: true,
            ..Builder::default()
        };
        let fila = correr_con(&e, &builder, &[], CONFIG).expect("corre");
        let gateados: Vec<Evento> = eventos(&e)
            .into_iter()
            .filter(|ev| ev.tipo == TipoEvento::Gate && ev.fase == "costo-desconocido")
            .collect();
        assert_eq!(gateados.len(), 2, "un Gate por sesión sin costo");
        assert!(gateados.iter().all(|ev| ev.corrida == fila.id));
    }

    /// Registrar dos veces la misma sesión deja una sola `Sesion` y no suma el costo otra vez.
    #[test]
    fn corre_registrar_la_misma_sesion_dos_veces_no_duplica_el_costo() {
        let e = entorno();
        let id = "c-dup";
        fs::create_dir_all(e.home.join("transcripts").join(id)).expect("transcripts");
        let lanzado = Lanzado {
            n: 1,
            rol: Rol::Planifica,
            codigo: 0,
            linea: String::new(),
        };
        let linea = r#"{"type":"result","total_cost_usd":0.7,"session_id":"s-dup","modelUsage":{"a":{"costUSD":0.5},"b":{"costUSD":0.2}}}"#;
        let ruta_eventos = observe::ruta_eventos_en(&e.home);
        registrar_sesion(&e.home, id, &lanzado, linea, &ruta_eventos).expect("primera");
        registrar_sesion(&e.home, id, &lanzado, linea, &ruta_eventos).expect("segunda");
        let log = eventos(&e);
        let sesiones = log
            .iter()
            .filter(|ev| ev.tipo == TipoEvento::Sesion)
            .count();
        assert_eq!(sesiones, 1, "una sola Sesion en el log");
        let total: f64 = log.iter().map(|ev| ev.cost_usd).sum();
        assert!((total - 0.7).abs() < 1e-12, "costo sumado {total}");
    }

    /// Una sesión con costo negativo deja un Gate de `FASE_SESION` y no corta la corrida.
    #[test]
    fn corre_sesion_con_costo_negativo_deja_gate_y_no_corta() {
        let e = entorno();
        let id = "c-neg";
        fs::create_dir_all(e.home.join("transcripts").join(id)).expect("transcripts");
        let lanzado = Lanzado {
            n: 1,
            rol: Rol::Planifica,
            codigo: 0,
            linea: String::new(),
        };
        let linea = r#"{"type":"result","total_cost_usd":-1.0,"session_id":"s-neg"}"#;
        let ruta_eventos = observe::ruta_eventos_en(&e.home);
        registrar_sesion(&e.home, id, &lanzado, linea, &ruta_eventos).expect("no corta");
        let motivos = gates(&e, FASE_SESION);
        assert_eq!(motivos.len(), 1, "un Gate de la sesión");
        assert!(motivos[0].contains("sesión no registrada"), "{motivos:?}");
        assert!(!motivos[0].contains("sesión no leída"), "{motivos:?}");
        assert!(eventos(&e).iter().all(|ev| ev.tipo != TipoEvento::Sesion));
    }

    #[test]
    fn corre_fallo_del_ejecutor_corta_sin_evaluar_ni_escribir_fila() {
        let e = entorno();
        let builder = Builder {
            no_lanza: true,
            ..Builder::default()
        };
        let error = correr_con(&e, &builder, &[], CONFIG).expect_err("no arranca");
        assert!(matches!(error, ErrorCorre::Ejecutor(_)), "{error}");
        assert_eq!(
            gates(&e, "lanzamiento").len(),
            1,
            "queda un Gate con la causa"
        );
        assert!(!e.home.join("runs.jsonl").exists(), "sin fila");
    }

    #[test]
    fn corre_tarea_inexistente_es_uso_y_no_crea_carpeta() {
        let e = entorno();
        let error = correr(
            &DepsCorre {
                home: &e.home,
                home_usuario: &e.usuario,
                repo_root: &e.repo,
                ejecutor: &Builder::default(),
                evaluar: &evaluador_falso,
                entorno: &[],
            },
            "no-existe",
            CONFIG,
        )
        .expect_err("tarea inexistente");
        assert_eq!(error.codigo_de_salida(), 2, "{error}");
        assert!(!dir_corridas(&e.usuario).exists());
    }

    /// C1: el `fsmonitor` del `.git` del builder no se ejecuta en el host al cerrar la corrida.
    #[test]
    fn corre_fsmonitor_del_builder_no_se_ejecuta_en_el_host() {
        let e = entorno();
        let marca = e._tmp.path().join("marca");
        let builder = Builder {
            fsmonitor_marca: Some(marca.clone()),
            ..Builder::default()
        };
        correr_con(&e, &builder, &[], CONFIG).expect("corre");
        assert!(!marca.exists(), "el fsmonitor del builder se ejecutó");
    }

    /// C1: el evaluador recibe un repo nuevo, sin el `.git` del builder y con un solo commit final.
    #[test]
    fn corre_evaluador_recibe_repo_final_sin_git_del_builder() {
        let e = entorno();
        let builder = Builder {
            fsmonitor_marca: Some(e._tmp.path().join("marca")),
            ..Builder::default()
        };
        let recibido: RefCell<Option<PathBuf>> = RefCell::new(None);
        let evaluar =
            |repo: &Path, tarea: &Tarea, _o: &OpcionesOraculo| -> Result<Veredicto, ErrorOraculo> {
                *recibido.borrow_mut() = Some(repo.to_path_buf());
                Ok(Veredicto {
                    tarea: tarea.nombre.clone(),
                    estado: Estado::Verde,
                    informe: Some(informe()),
                    ..Veredicto::default()
                })
            };
        let fila = correr_con_eval(&e, &builder, &[], CONFIG, &evaluar).expect("corre");
        let repo = recibido
            .borrow()
            .clone()
            .expect("el evaluador recibió un repo");
        assert_eq!(
            repo,
            dir_corridas(&e.usuario).join(&fila.id).join("final"),
            "el evaluador debe recibir la carpeta final"
        );
        let config = fs::read_to_string(repo.join(".git").join("config")).expect("config");
        assert!(
            !config.contains("fsmonitor"),
            "config del builder: {config}"
        );
        assert!(repo.join("hecho.txt").is_file(), "falta hecho.txt");
        let salida = comando_git(&repo, &["log", "--format=%s"])
            .output()
            .expect("git log");
        assert!(salida.status.success(), "git log falló");
        let mensajes: Vec<String> = String::from_utf8_lossy(&salida.stdout)
            .lines()
            .map(str::to_string)
            .collect();
        assert_eq!(mensajes, vec![MENSAJE_ESTADO_FINAL.to_string()]);
    }

    /// Builder que corta el rol por tiempo: `lanzar` devuelve el error de corte con el `stdout` dado.
    struct Cortado {
        stdout: Vec<u8>,
    }

    impl EjecutorBuilder for Cortado {
        fn lanzar(&self, _l: &Lanzamiento) -> Result<SalidaClaude, ErrorCorre> {
            Err(ErrorCorre::Cortado {
                mensaje: "timeout tras 7200 s; se pidió borrar el contenedor prueba".into(),
                stdout: self.stdout.clone(),
            })
        }
    }

    /// Qué hace un lanzamiento de `Programado`.
    #[derive(Clone)]
    enum Paso {
        /// Corta por plazo: `lanzar` devuelve `Cortado` con este `stdout` parcial.
        Corte(Vec<u8>),
        /// Sale bien: corre el falso, con un `session_id` propio por lanzamiento.
        Bien,
        /// No arranca: `lanzar` devuelve un error del ejecutor que no es un corte.
        NoArranca,
    }

    /// Ejecutor que sigue una lista de pasos, uno por lanzamiento. Sin más pasos, sale bien.
    struct Programado {
        pasos: Vec<Paso>,
        lanzados: RefCell<Vec<Lanzamiento>>,
    }

    impl Programado {
        fn con(pasos: Vec<Paso>) -> Self {
            Self {
                pasos,
                lanzados: RefCell::new(Vec::new()),
            }
        }
    }

    impl EjecutorBuilder for Programado {
        fn lanzar(&self, l: &Lanzamiento) -> Result<SalidaClaude, ErrorCorre> {
            self.lanzados.borrow_mut().push(l.clone());
            let n = self.lanzados.borrow().len();
            let paso = self.pasos.get(n - 1).cloned().unwrap_or(Paso::Bien);
            match paso {
                Paso::Corte(stdout) => Err(ErrorCorre::Cortado {
                    mensaje: "timeout tras 7200 s; se pidió borrar el contenedor prueba".into(),
                    stdout,
                }),
                Paso::NoArranca => Err(ErrorCorre::Ejecutor("prueba: no arranca".into())),
                Paso::Bien => {
                    let mut salida = EjecutorFalso.lanzar(l)?;
                    let texto = String::from_utf8_lossy(&salida.stdout).replace(
                        "\"session_id\":\"s1\"",
                        &format!("\"session_id\":\"s-bien-{n}\""),
                    );
                    salida.stdout = texto.into_bytes();
                    Ok(salida)
                }
            }
        }
    }

    /// Corte con el `stdout` de la fixture, que trae `session_id` en su línea `init`.
    fn cortado() -> Paso {
        Paso::Corte(include_bytes!("../../tests/fixtures/transcripts/cortado.jsonl").to_vec())
    }

    /// Corte cuyo `stdout` no trae `session_id` en ninguna línea.
    fn corte_sin_sesion() -> Paso {
        let texto = String::from_utf8_lossy(include_bytes!(
            "../../tests/fixtures/transcripts/cortado.jsonl"
        ))
        .into_owned();
        let sin: Vec<&str> = texto
            .lines()
            .filter(|linea| !linea.contains("session_id"))
            .collect();
        Paso::Corte(format!("{}\n", sin.join("\n")).into_bytes())
    }

    /// Corre `correr` con `ejecutor` y cuenta cuántas veces se evaluó el repo.
    fn correr_contando(
        e: &Entorno,
        ejecutor: &dyn EjecutorBuilder,
        config: &str,
    ) -> (Result<Fila, ErrorCorre>, usize) {
        let evaluaciones = Cell::new(0);
        let evaluar = |r: &Path, t: &Tarea, o: &OpcionesOraculo| {
            evaluaciones.set(evaluaciones.get() + 1);
            evaluador_falso(r, t, o)
        };
        let resultado = correr_con_eval(e, ejecutor, &[], config, &evaluar);
        (resultado, evaluaciones.get())
    }

    /// Hay un par de argumentos con `bandera` seguida de `valor`.
    fn tiene_par(args: &[String], bandera: &str, valor: &str) -> bool {
        args.windows(2).any(|p| p[0] == bandera && p[1] == valor)
    }

    /// Eventos de costo estimado de la corrida `id`.
    fn costos_estimados(e: &Entorno, id: &str) -> usize {
        eventos(e)
            .iter()
            .filter(|ev| {
                ev.corrida == id && ev.extra.get("costo_estimado") == Some(&Value::Bool(true))
            })
            .count()
    }

    #[test]
    fn corre_dos_cortes_y_un_resultado_exitoso_llegan_a_evaluar() {
        let e = entorno();
        let ejecutor = Programado::con(vec![cortado(), cortado(), Paso::Bien]);
        let (resultado, evaluaciones) = correr_contando(&e, &ejecutor, CONFIG_CONSTRUYE);
        let fila = resultado.expect("la corrida llega a la evaluación");
        assert_eq!(evaluaciones, 1, "la corrida llega al juez final");

        let transcripts = e.home.join("transcripts").join(&fila.id);
        for n in 1..=3 {
            let ruta = transcripts.join(format!("{n}-construye.jsonl"));
            assert!(ruta.is_file(), "falta {}", ruta.display());
        }

        let lanzados = ejecutor.lanzados.borrow();
        assert_eq!(lanzados.len(), 3, "dos cortes y un resultado");
        assert!(
            !lanzados[0].args.contains(&"--resume".to_string()),
            "{:?}",
            lanzados[0].args
        );
        assert_eq!(lanzados[0].args[1], "haz hecho");
        for l in &lanzados[1..] {
            assert!(tiene_par(&l.args, "--resume", "s-cortado"), "{:?}", l.args);
            let prompt = &l.args[1];
            assert!(prompt.starts_with(PROMPT_CONTINUACION), "{prompt}");
            assert!(prompt.ends_with(" haz hecho"), "{prompt}");
        }
        assert_eq!(costos_estimados(&e, &fila.id), 2, "un costo por corte");
    }

    #[test]
    fn corre_cuatro_cortes_evaluan_el_repo_parcial_y_escriben_la_fila() {
        let e = entorno();
        let ejecutor = Programado::con(vec![cortado(), cortado(), cortado(), cortado()]);
        let (resultado, evaluaciones) = correr_contando(&e, &ejecutor, CONFIG_CONSTRUYE);
        let fila = resultado.expect("agotar las vueltas no pierde la corrida");
        assert_eq!(evaluaciones, 1, "el repo parcial se evalúa una vez");
        assert_eq!(
            ejecutor.lanzados.borrow().len(),
            4,
            "cuatro lanzamientos, no cinco"
        );
        let guardadas =
            track::filas_de(&e.home.join("runs.jsonl"), &Filtro::default()).expect("runs");
        assert_eq!(guardadas.len(), 1, "una fila por corrida");
        assert_eq!(guardadas[0].id, fila.id);
        assert_eq!(
            gates(&e, FASE_ROL),
            vec!["rol construye agotó 4 vueltas cortadas por plazo".to_string()]
        );
    }

    #[test]
    fn corre_cuatro_cortes_del_primer_rol_no_lanza_el_siguiente() {
        let e = entorno();
        let ejecutor = Programado::con(vec![cortado(), cortado(), cortado(), cortado()]);
        let (resultado, evaluaciones) = correr_contando(&e, &ejecutor, CONFIG_PLAN_ORGANIZA);
        resultado.expect("la corrida llega a la evaluación");
        assert_eq!(evaluaciones, 1, "el repo parcial se evalúa una vez");
        let roles: Vec<Rol> = ejecutor.lanzados.borrow().iter().map(|l| l.rol).collect();
        assert_eq!(roles, vec![Rol::Planifica; 4], "organiza no debe correr");
    }

    #[test]
    fn corre_lectura_del_oraculo_en_una_vuelta_cortada_deja_su_evento() {
        let e = entorno();
        let oculto = e
            .home
            .join("oraculos")
            .join(TAREA)
            .join("oculto")
            .join("test_y.py");
        let mut parcial = include_bytes!("../../tests/fixtures/transcripts/cortado.jsonl").to_vec();
        let lectura = format!(
            "{{\"type\":\"assistant\",\"message\":{{\"content\":[{{\"type\":\"tool_use\",\"id\":\"r1\",\"name\":\"Bash\",\"input\":{{\"command\":\"cat {}\"}}}}]}}}}\n",
            oculto.display()
        );
        parcial.extend_from_slice(lectura.as_bytes());
        let ejecutor = Programado::con(vec![Paso::Corte(parcial), Paso::Bien]);
        let fila =
            correr_con(&e, &ejecutor, &[], CONFIG_CONSTRUYE).expect("la corrida llega al juez");
        assert!(
            eventos(&e)
                .iter()
                .any(|ev| ev.tipo == TipoEvento::OraculoLeido && ev.corrida == fila.id),
            "la lectura de una vuelta cortada debe quedar en eventos"
        );
        assert!(fila.resumen.invalida, "la lectura debe invalidar");
    }

    #[test]
    fn corre_vuelta_sin_resultado_exitoso_y_no_cortada_sigue_abortando() {
        let e = entorno();
        let builder = Builder {
            sin_resultado: true,
            ..Builder::default()
        };
        let (error, evaluadas) = correr_contando_evaluaciones(&e, &builder);
        assert!(matches!(error, ErrorCorre::Ejecutor(_)), "{error}");
        assert_eq!(
            builder.lanzados.borrow().len(),
            1,
            "sin resultado no hay otra vuelta"
        );
        assert_eq!(evaluadas, 0, "no debe evaluar");

        let e2 = entorno();
        let ejecutor = Programado::con(vec![Paso::NoArranca, Paso::Bien]);
        let error = correr_con(&e2, &ejecutor, &[], CONFIG_CONSTRUYE)
            .expect_err("un lanzamiento que no arranca aborta");
        assert!(matches!(error, ErrorCorre::Ejecutor(_)), "{error}");
        assert_eq!(
            ejecutor.lanzados.borrow().len(),
            1,
            "un error que no es corte no reintenta"
        );
        assert_eq!(gates(&e2, FASE_LANZAMIENTO).len(), 1);
    }

    #[test]
    fn corre_sin_session_id_en_el_corte_va_sin_resume() {
        let e = entorno();
        let ejecutor = Programado::con(vec![corte_sin_sesion(), Paso::Bien]);
        correr_con(&e, &ejecutor, &[], CONFIG_CONSTRUYE).expect("corre");
        let lanzados = ejecutor.lanzados.borrow();
        assert_eq!(lanzados.len(), 2, "un corte y una vuelta");
        assert!(
            !lanzados[1].args.contains(&"--resume".to_string()),
            "{:?}",
            lanzados[1].args
        );
        assert!(lanzados[1].args[1].starts_with(PROMPT_CONTINUACION));
    }

    #[test]
    fn corre_vuelta_sin_session_id_conserva_la_sesion_anterior() {
        let e = entorno();
        let ejecutor = Programado::con(vec![cortado(), corte_sin_sesion(), Paso::Bien]);
        correr_con(&e, &ejecutor, &[], CONFIG_CONSTRUYE).expect("corre");
        let lanzados = ejecutor.lanzados.borrow();
        assert_eq!(lanzados.len(), 3);
        assert!(
            tiene_par(&lanzados[2].args, "--resume", "s-cortado"),
            "{:?}",
            lanzados[2].args
        );
    }

    #[test]
    fn corre_corte_en_el_segundo_rol_reintenta_solo_ese_rol() {
        let e = entorno();
        let ejecutor = Programado::con(vec![Paso::Bien, cortado(), Paso::Bien]);
        let fila = correr_con(&e, &ejecutor, &[], CONFIG_PLAN_ORGANIZA).expect("corre");
        let roles: Vec<Rol> = ejecutor.lanzados.borrow().iter().map(|l| l.rol).collect();
        assert_eq!(roles, vec![Rol::Planifica, Rol::Organiza, Rol::Organiza]);

        let transcripts = e.home.join("transcripts").join(&fila.id);
        assert!(transcripts.join("2-organiza.jsonl").is_file());
        assert!(transcripts.join("3-organiza.jsonl").is_file());

        let sesiones: Vec<String> = eventos(&e)
            .iter()
            .filter(|ev| ev.corrida == fila.id && ev.tipo == TipoEvento::Sesion)
            .filter_map(|ev| ev.extra.get("session_id").and_then(Value::as_str))
            .map(str::to_string)
            .collect();
        assert!(
            sesiones.contains(&"s-bien-1".to_string()),
            "la sesión de planifica queda registrada: {sesiones:?}"
        );
        let lanzados = ejecutor.lanzados.borrow();
        assert!(tiene_par(&lanzados[2].args, "--resume", "s-cortado"));
    }

    #[test]
    fn sesion_de_transcript_toma_la_primera_linea_con_session_id() {
        let texto = b"{\"type\":\"system\",\"session_id\":\"s-a\"}\n{\"type\":\"assistant\",\"session_id\":\"s-b\"}\n";
        assert_eq!(sesion_de_transcript(texto), Some("s-a".to_string()));
    }

    #[test]
    fn sesion_de_transcript_sin_session_id_da_none() {
        assert_eq!(sesion_de_transcript(b"{\"type\":\"assistant\"}\n"), None);
        assert_eq!(sesion_de_transcript(b""), None);
    }

    #[test]
    fn sesion_de_transcript_salta_las_lineas_que_no_son_json() {
        let texto = b"no es json\n{\"session_id\":\"s-c\"}\n";
        assert_eq!(sesion_de_transcript(texto), Some("s-c".to_string()));
        let fixture = include_bytes!("../../tests/fixtures/transcripts/cortado.jsonl");
        assert_eq!(sesion_de_transcript(fixture), Some("s-cortado".to_string()));
    }

    #[test]
    fn corre_rol_cortado_registra_costo_estimado_por_vuelta() {
        let e = entorno();
        let stdout = include_bytes!("../../tests/fixtures/transcripts/cortado.jsonl").to_vec();
        let builder = Cortado { stdout };
        let (resultado, evaluaciones) = correr_contando(&e, &builder, CONFIG_CONSTRUYE);
        let fila = resultado.expect("agotar las vueltas deja fila");
        assert_eq!(evaluaciones, 1, "la corrida cortada llega al juez");
        assert!(
            e.home.join("runs.jsonl").exists(),
            "la corrida escribe su fila"
        );
        let guardadas =
            track::filas_de(&e.home.join("runs.jsonl"), &Filtro::default()).expect("runs");
        assert_eq!(guardadas[0].id, fila.id);

        let costos: Vec<Evento> = eventos(&e)
            .into_iter()
            .filter(|ev| ev.tipo == TipoEvento::Llamada && ev.cost_usd > 0.0)
            .collect();
        assert_eq!(costos.len(), 4, "un evento de costo por vuelta cortada");
        let ev = &costos[0];
        assert_eq!(ev.via, Some(Via::Cupo));
        assert_eq!(ev.modelo.as_deref(), Some("haiku"));
        assert_eq!((ev.in_tokens, ev.out_tokens), (2500, 130));
        assert_eq!(ev.extra["cache_read_tokens"], 800);
        assert_eq!(ev.extra["cache_write_tokens"], 200);
        // "haiku" se traduce a "claude-haiku-5-5": rige el precio de ese id, no el pesimista.
        let precio = Config::default().precios["claude-haiku-5-5"].clone();
        let (usd_entrada, usd_salida) = precio.tarifa_para(3500);
        let esperado = 2830.0 * usd_entrada / 1e6 + 130.0 * usd_salida / 1e6;
        assert!((ev.cost_usd - esperado).abs() < 1e-9, "{}", ev.cost_usd);
        assert_eq!(ev.extra["costo_estimado"], Value::Bool(true));
        assert!(!ev.extra.contains_key("precio_supuesto"));

        let lanzamientos = gates(&e, FASE_LANZAMIENTO);
        assert_eq!(lanzamientos.len(), 4, "un Gate de lanzamiento por corte");
        assert!(
            lanzamientos
                .iter()
                .all(|m| m.contains("rol construye cortado por plazo"))
        );
        assert!(lanzamientos.iter().all(|m| !m.contains("no arrancó")));
        assert_eq!(
            gates(&e, FASE_ROL),
            vec!["rol construye agotó 4 vueltas cortadas por plazo".to_string()]
        );
        assert!(
            gates(&e, FASE_EVALUACION).is_empty(),
            "la evaluación corre sin error"
        );
    }

    #[test]
    fn corre_rol_cortado_sin_precio_del_modelo_usa_el_pesimista_marcado() {
        let e = entorno();
        // "opus" no tiene precio por defecto: rige el pesimista (15 y 75 USD por millón) y se marca.
        fs::write(
            e.home.join("root.toml"),
            "[modelos]\nplanifica = \"opus\"\nconstruye = \"opus\"\n",
        )
        .expect("root.toml");
        let stdout = include_bytes!("../../tests/fixtures/transcripts/cortado.jsonl").to_vec();
        let builder = Cortado { stdout };
        let (resultado, _) = correr_contando(&e, &builder, CONFIG_CONSTRUYE);
        resultado.expect("agotar las vueltas deja fila");

        let costos: Vec<Evento> = eventos(&e)
            .into_iter()
            .filter(|ev| ev.tipo == TipoEvento::Llamada && ev.cost_usd > 0.0)
            .collect();
        assert_eq!(costos.len(), 4, "un evento de costo por vuelta cortada");
        for ev in &costos {
            assert_eq!(ev.modelo.as_deref(), Some("opus"));
            let esperado = 2830.0 * 15.0 / 1e6 + 130.0 * 75.0 / 1e6;
            assert!((ev.cost_usd - esperado).abs() < 1e-9, "{}", ev.cost_usd);
            assert_eq!(ev.extra["precio_supuesto"], Value::Bool(true));
        }
    }

    #[test]
    fn corre_rol_cortado_guarda_el_transcript_de_cada_vuelta() {
        let e = entorno();
        let stdout = include_bytes!("../../tests/fixtures/transcripts/cortado.jsonl").to_vec();
        let builder = Cortado {
            stdout: stdout.clone(),
        };
        let _ = correr_contando(&e, &builder, CONFIG_CONSTRUYE);
        let carpetas: Vec<PathBuf> = fs::read_dir(e.home.join("transcripts"))
            .expect("carpeta de transcripts")
            .map(|entrada| entrada.expect("entrada").path())
            .collect();
        assert_eq!(carpetas.len(), 1, "una carpeta por corrida");
        for n in 1..=4 {
            let guardado = carpetas[0].join(format!("{n}-construye.jsonl"));
            assert_eq!(
                fs::read(&guardado).expect("transcript"),
                stdout,
                "vuelta {n}"
            );
        }
    }

    // ---- Reanudar una corrida cortada (`correr_reanudando`) ----

    /// Id de la única corrida bajo `usuario`.
    fn id_de_unica_corrida(usuario: &Path) -> String {
        unica_corrida(usuario)
            .file_name()
            .expect("nombre de la carpeta")
            .to_string_lossy()
            .into_owned()
    }

    /// Cuántas carpetas de corridas hay bajo `usuario`. Sin carpeta, cero.
    fn carpetas_de_corridas(usuario: &Path) -> usize {
        fs::read_dir(dir_corridas(usuario)).map_or(0, |d| d.count())
    }

    /// Corrida de `plan-organiza` que deja a planifica terminado y a organiza sin `result`, sin cortar.
    /// Devuelve el id de la corrida cortada.
    fn corrida_cortada_en_organiza(e: &Entorno) -> String {
        let builder = Builder {
            sin_resultado_en: Some(Rol::Organiza),
            ..Builder::default()
        };
        let error = correr_con(e, &builder, &[], CONFIG_PLAN_ORGANIZA)
            .expect_err("organiza sin result aborta");
        assert!(matches!(error, ErrorCorre::Ejecutor(_)), "{error}");
        id_de_unica_corrida(&e.usuario)
    }

    /// Reanuda `id` con `config` y cuenta las evaluaciones del repo.
    fn reanudar_contando(
        e: &Entorno,
        ejecutor: &dyn EjecutorBuilder,
        config: &str,
        id: &str,
    ) -> (Result<Fila, ErrorCorre>, usize) {
        let evaluaciones = Cell::new(0);
        let evaluar = |r: &Path, t: &Tarea, o: &OpcionesOraculo| {
            evaluaciones.set(evaluaciones.get() + 1);
            evaluador_falso(r, t, o)
        };
        let deps = DepsCorre {
            home: &e.home,
            home_usuario: &e.usuario,
            repo_root: &e.repo,
            ejecutor,
            evaluar: &evaluar,
            entorno: &[],
        };
        let resultado = correr_reanudando(&deps, TAREA, config, id);
        (resultado, evaluaciones.get())
    }

    /// Quita de `eventos.jsonl` los eventos de costo (`Llamada`) y de sesión de la corrida `id`.
    fn quitar_costos_y_sesiones(e: &Entorno, id: &str) {
        let ruta = e.home.join("eventos.jsonl");
        let quedan: Vec<String> = fs::read_to_string(&ruta)
            .expect("eventos")
            .lines()
            .filter(|linea| {
                let ev: Evento = serde_json::from_str(linea).expect("evento válido");
                !(ev.corrida == id && matches!(ev.tipo, TipoEvento::Llamada | TipoEvento::Sesion))
            })
            .map(str::to_string)
            .collect();
        fs::write(&ruta, format!("{}\n", quedan.join("\n"))).expect("reescribe eventos");
    }

    /// Cuántos eventos `Sesion` del `session_id` dado hay en el log.
    fn sesiones_con_id(e: &Entorno, session_id: &str) -> usize {
        eventos(e)
            .iter()
            .filter(|ev| {
                ev.tipo == TipoEvento::Sesion
                    && ev.extra.get("session_id").and_then(Value::as_str) == Some(session_id)
            })
            .count()
    }

    /// Builder que sale bien después de una corrida anterior de dos lanzamientos.
    fn builder_tras_dos_lanzamientos() -> Builder {
        Builder {
            sesion_desde: 2,
            ..Builder::default()
        }
    }

    /// El rol terminado no se lanza otra vez: reanudar lanza solo organiza, y evalúa una vez.
    #[test]
    fn corre_reanuda_no_relanza_el_rol_terminado() {
        let e = entorno();
        let id = corrida_cortada_en_organiza(&e);
        let builder = builder_tras_dos_lanzamientos();
        let (resultado, evaluaciones) = reanudar_contando(&e, &builder, CONFIG_PLAN_ORGANIZA, &id);
        resultado.expect("la corrida reanudada llega a la fila");
        let roles: Vec<Rol> = builder.lanzados.borrow().iter().map(|l| l.rol).collect();
        assert_eq!(
            roles,
            vec![Rol::Organiza],
            "planifica no debe lanzarse otra vez"
        );
        assert_eq!(evaluaciones, 1, "la corrida reanudada evalúa una vez");
    }

    /// Organiza cortado con sesión: su primera vuelta nueva retoma esa sesión y pide continuar.
    #[test]
    fn corre_reanuda_organiza_parcial_sigue_con_resume() {
        let e = entorno();
        let id = corrida_cortada_en_organiza(&e);
        let builder = builder_tras_dos_lanzamientos();
        reanudar_contando(&e, &builder, CONFIG_PLAN_ORGANIZA, &id)
            .0
            .expect("reanuda");
        let lanzados = builder.lanzados.borrow();
        assert_eq!(lanzados.len(), 1, "organiza corre una vez");
        assert!(
            tiene_par(&lanzados[0].args, "--resume", "s1-2"),
            "{:?}",
            lanzados[0].args
        );
        let prompt = &lanzados[0].args[1];
        assert!(prompt.starts_with(PROMPT_CONTINUACION), "{prompt}");
    }

    /// Si falta el evento de sesión del rol terminado, reanudar lo registra una vez y no duplica el costo.
    #[test]
    fn corre_reanuda_registra_la_sesion_del_rol_terminado_si_falta() {
        let e = entorno();
        let id = corrida_cortada_en_organiza(&e);
        quitar_costos_y_sesiones(&e, &id);
        assert_eq!(
            sesiones_con_id(&e, "s1-1"),
            0,
            "precondición: sin la sesión"
        );
        let builder = builder_tras_dos_lanzamientos();
        let fila = reanudar_contando(&e, &builder, CONFIG_PLAN_ORGANIZA, &id)
            .0
            .expect("reanuda");
        assert_eq!(
            sesiones_con_id(&e, "s1-1"),
            1,
            "la sesión de planifica queda una vez"
        );
        assert!(
            (fila.resumen.usd_cupo - 0.02).abs() < 1e-9,
            "dos sesiones de 0,01 USD: {:?}",
            fila.resumen
        );
    }

    /// Guarda: si la sesión del rol terminado ya está en el log, reanudar no la vuelve a registrar.
    #[test]
    fn corre_reanuda_no_duplica_la_sesion_que_ya_estaba() {
        let e = entorno();
        let id = corrida_cortada_en_organiza(&e);
        assert_eq!(
            sesiones_con_id(&e, "s1-1"),
            1,
            "precondición: la sesión ya está"
        );
        let builder = builder_tras_dos_lanzamientos();
        let fila = reanudar_contando(&e, &builder, CONFIG_PLAN_ORGANIZA, &id)
            .0
            .expect("reanuda");
        assert_eq!(sesiones_con_id(&e, "s1-1"), 1, "no se duplica");
        assert!(
            (fila.resumen.usd_cupo - 0.02).abs() < 1e-9,
            "el costo no se duplica: {:?}",
            fila.resumen
        );
    }

    /// Si la corrida anterior ya dejó la aprobación del plan, reanudar no la repite.
    #[test]
    fn corre_reanuda_no_repite_la_aprobacion_del_plan() {
        let e = entorno();
        let id = corrida_cortada_en_organiza(&e);
        let aprobacion = observe::evento_manual(&id, TipoEvento::PlanAprobado, Some(PLAN_APROBADO));
        observe::registrar_en(&observe::ruta_eventos_en(&e.home), &aprobacion)
            .expect("aprobación previa");
        let builder = builder_tras_dos_lanzamientos();
        reanudar_contando(&e, &builder, CONFIG_PLAN_ORGANIZA, &id)
            .0
            .expect("reanuda");
        let aprobados = eventos(&e)
            .iter()
            .filter(|ev| ev.tipo == TipoEvento::PlanAprobado && ev.corrida == id)
            .count();
        assert_eq!(aprobados, 1, "una sola aprobación por corrida");
    }

    /// Un id de otra config, de otra tarea o con una ruta es error de uso. No lanza ni crea carpetas.
    #[test]
    fn corre_reanuda_con_otro_id_es_error_de_uso() {
        let e = entorno();
        let id = corrida_cortada_en_organiza(&e);
        let antes = carpetas_de_corridas(&e.usuario);
        let casos = [
            (CONFIG_PLAN, id.clone()),
            (
                CONFIG_PLAN_ORGANIZA,
                "corre-otra-plan-organiza-1-2-3".to_string(),
            ),
            (
                CONFIG_PLAN_ORGANIZA,
                format!("corre-{TAREA}-{CONFIG_PLAN_ORGANIZA}-../../fuera"),
            ),
        ];
        for (config, id_ajeno) in casos {
            let builder = Builder::default();
            let (resultado, evaluaciones) = reanudar_contando(&e, &builder, config, &id_ajeno);
            let error = resultado.expect_err("un id ajeno no reanuda");
            assert!(matches!(error, ErrorCorre::Uso(_)), "{id_ajeno}: {error}");
            assert_eq!(error.codigo_de_salida(), 2, "{id_ajeno}");
            assert!(
                builder.lanzados.borrow().is_empty(),
                "{id_ajeno}: no debe lanzar"
            );
            assert_eq!(evaluaciones, 0, "{id_ajeno}: no debe evaluar");
        }
        assert_eq!(carpetas_de_corridas(&e.usuario), antes, "no crea carpetas");
    }

    /// Un id con el prefijo bien pero sin carpeta es error de uso. No crea nada.
    #[test]
    fn corre_reanuda_carpeta_inexistente_es_error_de_uso() {
        let e = entorno();
        let id = format!("corre-{TAREA}-{CONFIG_PLAN_ORGANIZA}-1700000000-1-0");
        let builder = Builder::default();
        let (resultado, evaluaciones) = reanudar_contando(&e, &builder, CONFIG_PLAN_ORGANIZA, &id);
        let error = resultado.expect_err("sin carpeta no reanuda");
        assert!(matches!(error, ErrorCorre::Uso(_)), "{error}");
        assert_eq!(error.codigo_de_salida(), 2);
        assert!(builder.lanzados.borrow().is_empty(), "no debe lanzar");
        assert_eq!(evaluaciones, 0, "no debe evaluar");
        assert!(
            !dir_corridas(&e.usuario).exists(),
            "no debe crear la carpeta de corridas"
        );
    }

    /// Una corrida reanudada deja `reanudada` y `vueltas` en la fila y en `runs.jsonl`.
    #[test]
    fn corre_reanuda_deja_la_fila_marcada() {
        let e = entorno();
        let id = corrida_cortada_en_organiza(&e);
        let builder = builder_tras_dos_lanzamientos();
        let fila = reanudar_contando(&e, &builder, CONFIG_PLAN_ORGANIZA, &id)
            .0
            .expect("reanuda");
        assert_eq!(fila.id, id);
        assert!(fila.reanudada, "la fila debe decir que se reanudó");
        assert_eq!(
            fila.vueltas, 3,
            "dos de la corrida cortada y una al reanudar"
        );
        let guardadas =
            track::filas_de(&e.home.join("runs.jsonl"), &Filtro::default()).expect("runs");
        assert_eq!(guardadas.len(), 1, "una fila");
        assert!(guardadas[0].reanudada);
        assert_eq!(guardadas[0].vueltas, 3);
    }

    /// Una corrida normal deja `reanudada` en falso y `vueltas` con sus lanzamientos.
    #[test]
    fn corre_corrida_normal_deja_reanudada_falsa() {
        let e = entorno();
        let fila = correr_con(&e, &Builder::default(), &[], CONFIG).expect("corre");
        assert!(!fila.reanudada);
        assert_eq!(fila.vueltas, 2, "planifica y construye");
    }
}
