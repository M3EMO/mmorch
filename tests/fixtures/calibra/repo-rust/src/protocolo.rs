//! Capa baja: lee números de la entrada.
pub fn leer_numero(linea: &str) -> Result<u32, std::num::ParseIntError> {
    let numero = linea.trim().parse::<u32>()?;
    Ok(numero)
}
