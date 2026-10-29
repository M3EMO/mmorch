//! Capa alta: guarda valores por clave.
use std::collections::BTreeMap;

#[derive(Debug, Default)]
pub struct Almacen {
    datos: BTreeMap<String, u32>,
}

impl Almacen {
    pub fn set(&mut self, clave: &str, valor: u32) {
        self.datos.insert(clave.to_string(), valor);
    }

    pub fn get(&self, clave: &str) -> Option<u32> {
        self.datos.get(clave).copied()
    }
}
