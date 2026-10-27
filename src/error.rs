use std::io;

use thiserror::Error as ErrorDerive;

/// Errores de la biblioteca. Cada variante tiene su código de salida.
#[derive(Debug, ErrorDerive)]
pub enum Error {
    /// El subcomando existe en la CLI pero aún no tiene lógica.
    #[error("no implementado: {0}")]
    NoImplementado(String),
    /// Argumentos inválidos. Sale con código 2.
    #[error("uso: {0}")]
    Uso(String),
    #[error("no se pudo leer {ruta}: {fuente}")]
    ConfigLeer {
        ruta: String,
        #[source]
        fuente: io::Error,
    },
    #[error("root.toml inválido: {0}")]
    ConfigToml(#[from] toml::de::Error),
    #[error("error de JSON: {0}")]
    Json(#[from] serde_json::Error),
}

impl Error {
    /// Código de salida del proceso. 2 es uso; 3 es error de infraestructura, sin veredicto.
    pub fn codigo_de_salida(&self) -> u8 {
        match self {
            Error::Uso(_) => 2,
            Error::NoImplementado(_)
            | Error::ConfigLeer { .. }
            | Error::ConfigToml(_)
            | Error::Json(_) => 3,
        }
    }
}
