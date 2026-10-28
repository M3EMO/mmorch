//! Política de Route: estadísticas por rol y tipo. La Tarea 15 la llena; aquí solo se define y se lee.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::tipos::{Proveedor, Rol, VERSION, Via};

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

/// Candidatos por clave `"<rol>:<tipo>"`, con el rol en snake_case.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Politica {
    pub v: u32,
    pub estadisticas: BTreeMap<String, Vec<Candidato>>,
}

impl Default for Politica {
    fn default() -> Self {
        Politica {
            v: VERSION,
            estadisticas: BTreeMap::new(),
        }
    }
}

impl Politica {
    /// Clave de `estadisticas` para un rol y un tipo de pedido.
    pub fn clave(rol: Rol, tipo: &str) -> String {
        format!("{}:{tipo}", nombre_rol(rol))
    }
}

fn nombre_rol(rol: Rol) -> &'static str {
    match rol {
        Rol::Planifica => "planifica",
        Rol::Organiza => "organiza",
        Rol::Construye => "construye",
        Rol::Verifica => "verifica",
        Rol::Juzga => "juzga",
        Rol::Delega => "delega",
    }
}
