//! Biblioteca de Root: evalúa, rutea y observa corridas.

pub mod cli;
pub mod config;
pub mod error;
pub mod esquema;
pub mod goal;
pub mod tipos;

pub use cli::Cli;
pub use error::Error;

use cli::{Comando, GoalAccion};

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
        otro => Err(Error::NoImplementado(otro.ruta().to_string())),
    }
}
