//! Política de Route: estadísticas por rol y tipo. La Tarea 15 la llena; aquí solo se define y se lee.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::tipos::{Forma, Proveedor, Rol, VERSION, Via};

/// Un modelo con sus estadísticas. `wilson_inf` es la cota inferior de Wilson de la tasa de éxito.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Candidato {
    pub modelo: String,
    pub proveedor: Option<Proveedor>,
    /// `Cupo` o `Api`.
    pub via: Via,
    pub n: u32,
    pub exitos: u32,
    pub wilson_inf: f64,
    pub usd_medio: f64,
}

/// Estadísticas de un modelo en una tarea, con los ids de las filas que lo sustentan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EvidenciaModelo {
    pub modelo: String,
    pub n: u32,
    pub exitos: u32,
    pub wilson_inf: f64,
    pub usd_medio: f64,
    /// Ids de las filas válidas que cuentan para este modelo, ordenados.
    pub filas: Vec<String>,
}

/// Evidencia de una propuesta: el modelo actual de `root.toml` y el propuesto.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Evidencia {
    pub actual: EvidenciaModelo,
    pub propuesto: EvidenciaModelo,
}

/// Cambio esperado al aplicar la propuesta. `usd` positivo significa que ahorra.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Diferencia {
    /// Tasa de éxito propuesta menos la actual.
    pub exito: f64,
    /// USD medio actual menos el propuesto.
    pub usd: f64,
}

/// Cambio que Aprende sugiere en un rol fijo. El usuario lo aplica a mano en `root.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Propuesta {
    pub rol: Rol,
    pub tarea: String,
    pub modelo_actual: String,
    pub modelo_propuesto: String,
    pub evidencia: Evidencia,
    pub diferencia_esperada: Diferencia,
}

/// Evidencia de una forma: n, éxitos, cota de Wilson, USD medio e ids de las filas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EvidenciaForma {
    pub forma: Forma,
    pub n: u32,
    pub exitos: u32,
    pub wilson_inf: f64,
    pub usd_medio: f64,
    /// Ids de las filas válidas que cuentan para esta forma, ordenados.
    pub filas: Vec<String>,
}

/// Cambio de forma sugerido para una tarea. Route no lo lee: el usuario lo aplica a mano.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PropuestaForma {
    pub tarea: String,
    pub actual: EvidenciaForma,
    pub propuesta: EvidenciaForma,
    pub diferencia_esperada: Diferencia,
}

/// Candidatos por clave `"<rol>:<tipo>"`, con el rol en snake_case.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Politica {
    pub v: u32,
    pub estadisticas: BTreeMap<String, Vec<Candidato>>,
    /// Cambios sugeridos en los roles fijos. Route no los lee.
    #[serde(default)]
    pub propuestas: Vec<Propuesta>,
    /// Cambios de forma sugeridos. Route no los lee.
    #[serde(default)]
    pub propuestas_forma: Vec<PropuestaForma>,
}

impl Default for Politica {
    fn default() -> Self {
        Politica {
            v: VERSION,
            estadisticas: BTreeMap::new(),
            propuestas: Vec::new(),
            propuestas_forma: Vec::new(),
        }
    }
}

impl Politica {
    /// Clave de `estadisticas` para un rol y un tipo de pedido.
    pub fn clave(rol: Rol, tipo: &str) -> String {
        format!("{}:{tipo}", nombre_rol(rol))
    }
}

pub fn nombre_rol(rol: Rol) -> &'static str {
    match rol {
        Rol::Planifica => "planifica",
        Rol::Organiza => "organiza",
        Rol::Construye => "construye",
        Rol::Verifica => "verifica",
        Rol::Juzga => "juzga",
        Rol::Delega => "delega",
    }
}
