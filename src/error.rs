use thiserror::Error as ErrorDerive;

/// Errores de la biblioteca. Cada variante tiene su código de salida.
#[derive(Debug, ErrorDerive)]
pub enum Error {
    /// El subcomando existe en la CLI pero aún no tiene lógica.
    #[error("no implementado: {0}")]
    NoImplementado(String),
}

impl Error {
    /// Código de salida del proceso. 3 es error de infraestructura, sin veredicto.
    pub fn codigo_de_salida(&self) -> u8 {
        match self {
            Error::NoImplementado(_) => 3,
        }
    }
}
