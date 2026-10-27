//! Biblioteca de Root: evalúa, rutea y observa corridas.

pub mod cli;
pub mod error;

pub use cli::Cli;
pub use error::Error;

/// Ejecuta un subcomando. Por ahora ninguno está implementado.
pub fn ejecutar(cli: &Cli) -> Result<(), Error> {
    Err(Error::NoImplementado(cli.comando.ruta().to_string()))
}
