//! Biblioteca de Root: evalúa, rutea y observa corridas.

pub mod cli;
pub mod config;
pub mod error;
pub mod esquema;
pub mod goal;
pub mod observe;
pub mod tipos;

pub use cli::Cli;
pub use error::Error;

use cli::{Comando, GoalAccion, ObservaAccion};

/// Ejecuta un subcomando. Por ahora solo `esquema` y `goal` tienen lógica.
pub fn ejecutar(cli: &Cli) -> Result<(), Error> {
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
        Comando::Observa { accion } => observa(cli.json, accion),
        otro => Err(Error::NoImplementado(otro.ruta().to_string())),
    }
}

/// Subcomandos `observa`: registran eventos y muestran el resumen de una corrida.
fn observa(json: bool, accion: &ObservaAccion) -> Result<(), Error> {
    match accion {
        ObservaAccion::Claude { salida, corrida } => {
            let eventos = observe::claude::leer_sesion_claude(salida, corrida)?;
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
