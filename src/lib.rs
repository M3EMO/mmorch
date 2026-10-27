//! Biblioteca de Root: evalúa, rutea y observa corridas.

pub mod cli;
pub mod config;
pub mod error;
pub mod esquema;
pub mod tipos;

pub use cli::Cli;
pub use error::Error;

use cli::Comando;

/// Ejecuta un subcomando. Por ahora solo `esquema` tiene lógica.
pub fn ejecutar(cli: &Cli) -> Result<(), Error> {
    match &cli.comando {
        Comando::Esquema { tipo } => {
            println!("{}", esquema::json(tipo)?);
            Ok(())
        }
        otro => Err(Error::NoImplementado(otro.ruta().to_string())),
    }
}
