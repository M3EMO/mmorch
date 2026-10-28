use std::io;

use thiserror::Error as ErrorDerive;

use crate::ficha::ErrorFicha;
use crate::goal::ErrorGoal;
use crate::importar::ErrorImportar;
use crate::observe::ErrorObserve;
use crate::oracle::ErrorOraculo;
use crate::oracle::rubrica::ErrorRubrica;
use crate::proveedores::ErrorProveedor;
use crate::track::ErrorTrack;

/// Errores de la biblioteca. Cada variante tiene su código de salida.
#[derive(Debug, ErrorDerive)]
pub enum Error {
    /// El subcomando existe en la CLI pero aún no tiene lógica.
    #[error("no implementado: {0}")]
    NoImplementado(String),
    /// Argumentos inválidos. Sale con código 2.
    #[error("uso: {0}")]
    Uso(String),
    #[error("no se pudo leer {ruta}")]
    ConfigLeer {
        ruta: String,
        #[source]
        fuente: io::Error,
    },
    #[error("root.toml inválido")]
    ConfigToml(#[from] toml::de::Error),
    #[error("error de JSON")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Goal(#[from] ErrorGoal),
    #[error(transparent)]
    Observe(#[from] ErrorObserve),
    #[error(transparent)]
    Track(#[from] ErrorTrack),
    #[error(transparent)]
    Importar(#[from] ErrorImportar),
    #[error(transparent)]
    Oraculo(#[from] ErrorOraculo),
    #[error(transparent)]
    Proveedor(#[from] ErrorProveedor),
    #[error(transparent)]
    Rubrica(#[from] ErrorRubrica),
    #[error(transparent)]
    Ficha(#[from] ErrorFicha),
}

impl Error {
    /// Código de salida del proceso. 2 es uso; 3 es error de infraestructura, sin veredicto.
    pub fn codigo_de_salida(&self) -> u8 {
        match self {
            Error::Uso(_) => 2,
            Error::Goal(e) => e.codigo_de_salida(),
            Error::Ficha(e) => e.codigo_de_salida(),
            Error::Oraculo(e) => e.codigo_de_salida(),
            Error::NoImplementado(_)
            | Error::ConfigLeer { .. }
            | Error::ConfigToml(_)
            | Error::Json(_)
            | Error::Observe(_)
            | Error::Track(_)
            | Error::Importar(_)
            | Error::Proveedor(_)
            | Error::Rubrica(_) => 3,
        }
    }
}
