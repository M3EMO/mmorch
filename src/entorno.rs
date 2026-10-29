//! Guarda de los tests que necesitan podman: con `ROOT_SIN_PODMAN=1` se saltean y lo avisan en stderr.

use std::io::Write;

/// Variable que, valiendo exactamente `1`, saltea los tests que necesitan podman.
pub const VARIABLE_SIN_PODMAN: &str = "ROOT_SIN_PODMAN";

/// Saltea solo con `Some("1")`. Cualquier otro valor, o su ausencia, corre el test.
pub fn debe_saltear(valor: Option<&str>) -> bool {
    valor == Some("1")
}

/// Texto del aviso que se imprime en stderr cuando un test se saltea.
pub fn aviso_salteo(nombre: &str) -> String {
    format!("SALTEADO por ROOT_SIN_PODMAN=1 (necesita podman): {nombre}")
}

/// Lee `ROOT_SIN_PODMAN`. Si vale `1`, avisa en stderr con el nombre del test y devuelve true.
/// Escribe en el handle de stderr y no con `eprintln!`: `cargo test` captura `eprintln!` de los
/// tests que pasan, y el aviso no se vería.
pub fn sin_podman(nombre: &str) -> bool {
    let valor = std::env::var(VARIABLE_SIN_PODMAN).ok();
    if debe_saltear(valor.as_deref()) {
        let _ = writeln!(std::io::stderr(), "{}", aviso_salteo(nombre));
        return true;
    }
    false
}

/// Saltea el test actual si `ROOT_SIN_PODMAN=1`, y lo avisa en stderr con su nombre.
#[macro_export]
macro_rules! saltar_sin_podman {
    () => {{
        fn f() {}
        let nombre = ::std::any::type_name_of_val(&f);
        let nombre = nombre.strip_suffix("::f").unwrap_or(nombre);
        if $crate::entorno::sin_podman(nombre) {
            return;
        }
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entorno_debe_saltear_solo_con_uno() {
        assert!(debe_saltear(Some("1")));
        assert!(!debe_saltear(None));
        assert!(!debe_saltear(Some("0")));
        assert!(!debe_saltear(Some("")));
        assert!(!debe_saltear(Some("true")));
        assert!(!debe_saltear(Some(" 1")));
    }

    #[test]
    fn entorno_aviso_nombra_el_test_y_la_variable() {
        let aviso = aviso_salteo("x::y");
        assert!(aviso.contains("x::y"));
        assert!(aviso.contains("ROOT_SIN_PODMAN=1"));
    }
}
