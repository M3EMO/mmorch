use std::process::ExitCode;

use clap::Parser;
use root::Cli;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match root::ejecutar(&cli) {
        Ok(codigo) => ExitCode::from(codigo),
        Err(e) => {
            let codigo = e.codigo_de_salida();
            eprintln!("error: {:#}", anyhow::Error::new(e));
            ExitCode::from(codigo)
        }
    }
}
