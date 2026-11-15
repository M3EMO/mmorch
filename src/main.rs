use std::process::ExitCode;

use clap::Parser;
use root::Cli;

fn main() -> ExitCode {
    // Primera acción: las claves salen del descriptor antes de parsear la CLI y de crear hilos.
    if let Err(e) = root::proveedores::cargar_claves_de_fd_del_entorno() {
        eprintln!("error: {e}");
        return ExitCode::from(2);
    }
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
