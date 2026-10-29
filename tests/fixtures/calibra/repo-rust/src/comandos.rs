//! Capa media: ejecuta comandos sobre el almacén.
use crate::protocolo::leer_numero;

pub fn ejecutar(linea: &str) -> Result<u32, String> {
    let valor = leer_numero(linea).map_err(|e| e.to_string())?;
    registrar(valor)?;
    Ok(valor)
}

fn registrar(valor: u32) -> Result<(), String> {
    if valor > 0 { Ok(()) } else { Err("cero".to_string()) }
}
