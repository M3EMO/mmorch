//! Biblioteca de Root: evalúa, rutea y observa corridas.

pub mod cli;
pub mod config;
pub mod error;
pub mod esquema;
pub mod ficha;
pub mod goal;
pub mod importar;
pub mod observe;
pub mod oracle;
pub mod proveedores;
pub mod route;
pub mod tipos;
pub mod track;

pub use cli::Cli;
pub use error::Error;

use std::path::Path;

use cli::{
    Comando, FichaAccion, GoalAccion, ObservaAccion, OraculoAccion, ProveedorJuez, TrackAccion,
};
use ficha::{ContextoRevision, ErrorFicha, JuezProveedor, Revision};
use oracle::OpcionesOraculo;
use oracle::tarea::Tarea;
use tipos::{Eleccion, Estado, Fila, Proveedor, Via};

/// Ejecuta un subcomando y devuelve su código de salida. Los errores salen por `Err`.
/// Por ahora `oraculo evaluar`, `ficha`, `esquema`, `goal`, `observa`, `track` e `importar-mmorch` tienen lógica.
pub fn ejecutar(cli: &Cli) -> Result<u8, Error> {
    match &cli.comando {
        Comando::Oraculo {
            accion: OraculoAccion::Evaluar { tarea, repo },
        } => oraculo_evaluar(cli.json, tarea, repo),
        Comando::Ficha { accion } => ficha_accion(cli.json, accion),
        _ => ejecutar_sin_codigo(cli).map(|()| 0),
    }
}

/// `oraculo evaluar`: imprime el veredicto. El código de salida sale de su estado.
fn oraculo_evaluar(json: bool, nombre: &str, repo: &Path) -> Result<u8, Error> {
    let tarea = Tarea::cargar(nombre)?;
    let veredicto = oracle::evaluar(repo, &tarea, &OpcionesOraculo::default())?;
    if json {
        println!("{}", serde_json::to_string_pretty(&veredicto)?);
    } else {
        println!(
            "{}: contrato {}/{}, oculto {}/{}, estado {}, repeticiones {}, inestables {}",
            repo.display(),
            veredicto.contrato.pasa,
            veredicto.contrato.total,
            veredicto.oculto.pasa,
            veredicto.oculto.total,
            etiqueta_estado(veredicto.estado),
            veredicto.repeticiones,
            veredicto.inestables.len()
        );
    }
    Ok(oracle::codigo_de_estado(veredicto.estado))
}

/// `oraculo pendientes`: arma el paquete de los tests pendientes, o importa el junit de Colab.
fn oraculo_pendientes(
    json: bool,
    nombre: &str,
    repo: &Path,
    importar: Option<&Path>,
) -> Result<(), Error> {
    let tarea = Tarea::cargar(nombre)?;
    let home = config::root_home();
    if let Some(junit) = importar {
        let resumen = oracle::pendientes::importar(&home, &tarea, junit)?;
        if json {
            println!(
                "{}",
                serde_json::json!({"importados": resumen.importados, "ignorados": resumen.ignorados})
            );
        } else {
            println!(
                "importados: {}, ignorados: {}",
                resumen.importados, resumen.ignorados
            );
        }
        return Ok(());
    }
    match oracle::pendientes::crear(&home, &tarea, repo)? {
        None if json => println!("{}", serde_json::json!({"dir": null, "tests": []})),
        None => println!("tests: []"),
        Some(paquete) => {
            let dir = paquete.dir.display().to_string();
            if json {
                println!(
                    "{}",
                    serde_json::json!({"dir": dir, "tests": paquete.tests})
                );
            } else {
                println!("dir: {dir}");
                println!("tests: {}", serde_json::to_string(&paquete.tests)?);
            }
        }
    }
    Ok(())
}

/// Subcomandos `ficha`: revisan el spec o arman el borrador de `tarea.toml`.
/// `revisar` sale con 0 si no hay hallazgos ni ambigüedades, 1 si los hay, y 3 si el juez
/// pidió corrida y quedó `sin_veredicto`. Los errores de ficha salen con 2.
fn ficha_accion(json: bool, accion: &FichaAccion) -> Result<u8, Error> {
    match accion {
        FichaAccion::Revisar {
            spec,
            juez_proveedor,
            juez_modelo,
        } => {
            let texto = leer_spec(spec)?;
            let ctx = ContextoRevision::default();
            let revision = match juez_de(*juez_proveedor, juez_modelo.as_deref()) {
                Some(juez) => ficha::revisar_con_juez(&texto, &ctx, &juez)?,
                None => ficha::revisar(&texto, &ctx)?,
            };
            if json {
                println!("{}", serde_json::to_string_pretty(&revision)?);
            } else {
                imprimir_revision(&revision);
            }
            Ok(revision.codigo_de_salida())
        }
        FichaAccion::Tarea { spec, nombre } => {
            let texto = leer_spec(spec)?;
            let (ficha_leida, _) = ficha::extraer(&texto)?;
            let toml = ficha::borrador_tarea(&ficha_leida, nombre)?;
            if json {
                println!("{}", serde_json::json!({"toml": toml}));
            } else {
                print!("{toml}");
            }
            Ok(0)
        }
    }
}

fn leer_spec(spec: &Path) -> Result<String, ErrorFicha> {
    std::fs::read_to_string(spec).map_err(|fuente| ErrorFicha::Leer {
        ruta: spec.display().to_string(),
        fuente,
    })
}

/// Juez real si se pasan proveedor y modelo. Sin ellos no corre el juez.
fn juez_de(proveedor: Option<ProveedorJuez>, modelo: Option<&str>) -> Option<JuezProveedor> {
    let (proveedor, modelo) = (proveedor?, modelo?);
    Some(JuezProveedor {
        eleccion: Eleccion {
            via: Via::Api,
            proveedor: Some(Proveedor::from(proveedor)),
            modelo: Some(modelo.to_string()),
            razonamiento: true,
            motivo: "juez de ficha".into(),
        },
        corrida: "ficha-revisar".into(),
    })
}

/// Una línea por hallazgo y por ambigüedad: `linea: regla: detalle`.
fn imprimir_revision(revision: &Revision) {
    for h in &revision.hallazgos {
        println!("{}: {}: {}", h.linea, h.regla, h.detalle);
    }
    for a in &revision.ambiguedades {
        println!("{}: ambigüedad: {}", a.linea, a.frase);
    }
    if revision.juez.starts_with("sin_veredicto") {
        println!("juez: {}", revision.juez);
    }
    if revision.hallazgos.is_empty() && revision.ambiguedades.is_empty() {
        println!("sin hallazgos");
    }
}

fn etiqueta_estado(estado: Estado) -> &'static str {
    match estado {
        Estado::Verde => "verde",
        Estado::Rojo => "rojo",
        Estado::SinVeredicto => "sin_veredicto",
    }
}

/// Subcomandos sin código propio: salen con 0 o con error.
fn ejecutar_sin_codigo(cli: &Cli) -> Result<(), Error> {
    match &cli.comando {
        Comando::Esquema { tipo } => {
            println!("{}", esquema::json(tipo)?);
            Ok(())
        }
        Comando::Goal { accion } => {
            let repo = std::env::current_dir().map_err(goal::ErrorGoal::from)?;
            match accion {
                GoalAccion::Verificar => {
                    goal::verificar_goal(&repo)?;
                    let hash = goal::hash_del_repo(&repo)?;
                    if cli.json {
                        println!("{}", serde_json::json!({"ok": true, "hash": hash}));
                    } else {
                        println!("GOAL verificado: {hash}");
                    }
                    Ok(())
                }
                GoalAccion::Autorizar => {
                    goal::autorizar(&repo)?;
                    Ok(())
                }
            }
        }
        Comando::Oraculo {
            accion:
                OraculoAccion::Pendientes {
                    tarea,
                    repo,
                    importar,
                },
        } => oraculo_pendientes(cli.json, tarea, repo, importar.as_deref()),
        Comando::Observa { accion } => observa(cli.json, accion),
        Comando::Track { accion } => track_accion(cli.json, accion),
        Comando::ImportarMmorch { desde } => {
            let resumen = importar::importar_mmorch(desde)?;
            println!("{}", serde_json::to_string_pretty(&resumen)?);
            Ok(())
        }
        otro => Err(Error::NoImplementado(otro.ruta().to_string())),
    }
}

/// Subcomandos `track`: agregan una fila, las listan o comparan dos configuraciones.
fn track_accion(json: bool, accion: &TrackAccion) -> Result<(), Error> {
    match accion {
        TrackAccion::Agregar { fila } => {
            let texto = std::fs::read_to_string(fila).map_err(track::ErrorTrack::from)?;
            let nueva: Fila = serde_json::from_str(&texto).map_err(track::ErrorTrack::from)?;
            track::agregar(&nueva).map_err(track::ErrorTrack::from)?;
            if json {
                println!("{}", serde_json::json!({"agregada": nueva.id}));
            } else {
                println!("Fila agregada: {}", nueva.id);
            }
        }
        TrackAccion::Listar { tarea } => {
            let filtro = track::Filtro {
                tarea: tarea.clone(),
                config: None,
            };
            let filas = track::filas(&filtro)?;
            println!("{}", serde_json::to_string_pretty(&filas)?);
        }
        TrackAccion::Comparar { a, b, tarea } => {
            let comparacion = track::comparar(a, b, tarea)?;
            println!("{}", serde_json::to_string_pretty(&comparacion)?);
        }
    }
    Ok(())
}

/// Subcomandos `observa`: registran eventos y muestran el resumen de una corrida.
fn observa(json: bool, accion: &ObservaAccion) -> Result<(), Error> {
    match accion {
        ObservaAccion::Claude { archivo, corrida } => {
            let eventos = observe::claude::leer_sesion_claude(archivo, corrida)?;
            for ev in &eventos {
                observe::registrar(ev).map_err(observe::ErrorObserve::from)?;
            }
            imprimir_registrados(json, eventos.len());
        }
        ObservaAccion::Evento {
            corrida,
            tipo,
            detalle,
        } => {
            let ev = observe::evento_manual(corrida, (*tipo).into(), detalle.as_deref());
            observe::registrar(&ev).map_err(observe::ErrorObserve::from)?;
            imprimir_registrados(json, 1);
        }
        ObservaAccion::Resumen { corrida } => {
            let resumen = observe::resumir(corrida)?;
            println!("{}", serde_json::to_string_pretty(&resumen)?);
        }
    }
    Ok(())
}

fn imprimir_registrados(json: bool, cuantos: usize) {
    if json {
        println!("{}", serde_json::json!({"registrados": cuantos}));
    } else {
        println!("Registrados {cuantos} eventos");
    }
}
