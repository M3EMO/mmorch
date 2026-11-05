//! Primer archivo. `suma` tiene prueba. `doble` no la tiene a propósito.

pub fn suma(a: u32, b: u32) -> u32 {
    a + b
}

pub fn doble(a: u32) -> u32 {
    a * 2
}

#[cfg(test)]
mod tests {
    #[test]
    fn suma_da_la_suma() {
        assert_eq!(super::suma(2, 3), 5);
    }
}
