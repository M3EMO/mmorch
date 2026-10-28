use std::io;

use thiserror::Error as ErrorDerive;

use crate::aprende::ErrorAprende;
use crate::ficha::ErrorFicha;
use crate::goal::ErrorGoal;
use crate::importar::ErrorImportar;
use crate::observe::ErrorObserve;
use crate::oracle::ErrorOraculo;
use crate::oracle::rubrica::ErrorRubrica;
use crate::proveedores::ErrorProveedor;
use crate::tipos::SinPresupuesto;
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
    /// Un tope impide la elección o la llamada. Sale con código 3. El evento `Gate` ya quedó escrito.
    #[error("{0}")]
    Tope(String),
    /// Un valor de `root.toml` está fuera de rango, por ejemplo un tope en cero, o falta un
    /// campo que su tabla exige. Sale con código 2: es error de configuración del usuario.
    #[error("root.toml inválido: {0}")]
    ConfigValor(String),
    /// Ninguna vía tiene margen. Sale con código 3; el humano sube el tope en `root.toml`.
    #[error(transparent)]
    SinPresupuesto(#[from] SinPresupuesto),
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
    #[error(transparent)]
    Aprende(#[from] ErrorAprende),
    /// Error de E/S de stdin o stdout en `r00t mcp`. Sale con código 3.
    #[error("error de E/S del servidor MCP: {0}")]
    Mcp(io::Error),
}

impl Error {
    /// Código de salida del proceso. 2 es uso; 3 es error de infraestructura, sin veredicto.
    pub fn codigo_de_salida(&self) -> u8 {
        match self {
            Error::Uso(_) => 2,
            Error::Goal(e) => e.codigo_de_salida(),
            Error::Ficha(e) => e.codigo_de_salida(),
            Error::Oraculo(e) => e.codigo_de_salida(),
            Error::ConfigValor(_) => 2,
            Error::NoImplementado(_)
            | Error::ConfigLeer { .. }
            | Error::ConfigToml(_)
            | Error::SinPresupuesto(_)
            | Error::Tope(_)
            | Error::Json(_)
            | Error::Observe(_)
            | Error::Track(_)
            | Error::Importar(_)
            | Error::Proveedor(_)
            | Error::Rubrica(_)
            | Error::Mcp(_)
            | Error::Aprende(_) => 3,
        }
    }
}
