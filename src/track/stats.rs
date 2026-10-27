//! Estadística de corridas: cota inferior de Wilson unilateral al 90%.

/// z de la normal unilateral al 90%.
const Z_90: f64 = 1.2815515655446004;

/// Cota inferior de Wilson unilateral al 90% para `exitos` de `total`. Con `total` = 0 da 0.0.
pub fn wilson_inferior(exitos: u32, total: u32) -> f64 {
    if total == 0 {
        return 0.0;
    }
    let n = f64::from(total);
    let p = f64::from(exitos) / n;
    let z2 = Z_90 * Z_90;
    let centro = p + z2 / (2.0 * n);
    let margen = Z_90 * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt();
    (centro - margen) / (1.0 + z2 / n)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Valores calculados con `python3 -I` y la fórmula de Wilson con z = 1.2815515655446004.
    // 5/5: p = 1, cota = 0.752743. 50/100: p = 0.5, cota = 0.436442.

    #[test]
    fn track_wilson_sin_corridas_da_cero() {
        assert_eq!(wilson_inferior(0, 0), 0.0);
    }

    #[test]
    fn track_wilson_5_de_5_da_07527() {
        assert!((wilson_inferior(5, 5) - 0.7527).abs() < 1e-3);
    }

    #[test]
    fn track_wilson_50_de_100_da_04364() {
        assert!((wilson_inferior(50, 100) - 0.4364).abs() < 1e-3);
    }
}
