//! Oracle: corre los tests en un contenedor sin red y cuenta el resultado desde junit.

pub mod contenedor;
pub mod junit;

use thiserror::Error as ErrorDerive;

/// Errores del Oracle. Salen con código 3. Ante uno de ellos no hay veredicto.
#[derive(Debug, ErrorDerive)]
pub enum ErrorOraculo {
    /// Infraestructura: podman ausente, timeout, error de podman o E/S.
    #[error("error de infraestructura del oráculo: {0}")]
    Infra(String),
    /// El junit falta o está mal formado.
    #[error("junit inválido: {0}")]
    Junit(String),
}
