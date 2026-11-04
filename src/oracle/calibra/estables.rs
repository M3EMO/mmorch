//! Qué tests cuentan para detectar un defecto en Completitud y Funciona (Tarea 59). Un test inestable de la
//! base no cuenta, ni en un caso con defecto ni en uno limpio. Las otras dimensiones no cambian.

use std::collections::BTreeSet;

use crate::tipos::{Dimension, Veredicto};

/// Si la dimensión del caso puede detectar un defecto. Completitud y Funciona lo pueden solo con un
/// test malo nuevo, un total menor o un hallazgo nuevo. Las demás dimensiones siempre pueden.
pub(super) fn permite(base: &Veredicto, veredicto: &Veredicto, dimension: Dimension) -> bool {
    if !matches!(dimension, Dimension::Completitud | Dimension::Funciona) {
        return true;
    }
    hay_test_malo_nuevo(base, veredicto)
        || total_bajo(base, veredicto)
        || hallazgo_nuevo(base, veredicto, dimension)
}

/// Tests malos del oculto: los fallidos estables y los inestables.
fn malos_ocultos(veredicto: &Veredicto) -> BTreeSet<&str> {
    veredicto
        .ocultos_fallidos
        .iter()
        .chain(&veredicto.inestables)
        .map(String::as_str)
        .collect()
}

/// Tests malos del contrato: los fallidos estables y los inestables, sin prefijo.
fn malos_contrato(veredicto: &Veredicto) -> BTreeSet<&str> {
    veredicto
        .contrato_fallidos
        .iter()
        .chain(&veredicto.inestables_contrato)
        .map(String::as_str)
        .collect()
}

/// Hay un test malo en el caso que no era malo en la base. Cada lado se compara por separado.
fn hay_test_malo_nuevo(base: &Veredicto, veredicto: &Veredicto) -> bool {
    let ocultos_base = malos_ocultos(base);
    let contrato_base = malos_contrato(base);
    malos_ocultos(veredicto)
        .iter()
        .any(|id| !ocultos_base.contains(id))
        || malos_contrato(veredicto)
            .iter()
            .any(|id| !contrato_base.contains(id))
}

/// El total del oculto o del contrato bajó frente a la base.
fn total_bajo(base: &Veredicto, veredicto: &Veredicto) -> bool {
    veredicto.oculto.total < base.oculto.total || veredicto.contrato.total < base.contrato.total
}

/// Hay un hallazgo de `dimension` con un id que la base no tiene.
fn hallazgo_nuevo(base: &Veredicto, veredicto: &Veredicto, dimension: Dimension) -> bool {
    let ids_base: BTreeSet<&str> = base.hallazgos.iter().map(|h| h.id.as_str()).collect();
    veredicto
        .hallazgos
        .iter()
        .any(|h| h.dimension == dimension && !ids_base.contains(h.id.as_str()))
}

#[cfg(test)]
mod tests {
    use super::super::detecta_dimension;
    use super::super::detecta_item;
    use super::super::tests::{nivel_item, puntaje};
    use super::permite;
    use crate::tipos::{Conteo, Dimension, Veredicto};

    fn ids(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    /// Base de `dimension` en 1, con `t_flaky` inestable en el oculto.
    fn base_con_flaky(dimension: Dimension) -> Veredicto {
        Veredicto {
            puntajes: vec![puntaje(dimension, 1.0)],
            inestables: ids(&["t_flaky"]),
            ..Veredicto::default()
        }
    }

    /// Base de Completitud en 1, con `t_flaky` inestable en el contrato.
    fn base_contrato_con_flaky() -> Veredicto {
        Veredicto {
            puntajes: vec![puntaje(Dimension::Completitud, 1.0)],
            inestables_contrato: ids(&["t_flaky"]),
            ..Veredicto::default()
        }
    }

    /// Caso de `dimension` en `valor`, con los fallidos estables del oculto dados.
    fn caso_oculto(dimension: Dimension, valor: f32, fallidos: &[&str]) -> Veredicto {
        Veredicto {
            puntajes: vec![puntaje(dimension, valor)],
            ocultos_fallidos: ids(fallidos),
            ..Veredicto::default()
        }
    }

    /// Caso de Completitud en `valor`, con los fallidos estables del contrato dados.
    fn caso_contrato(valor: f32, fallidos: &[&str]) -> Veredicto {
        Veredicto {
            puntajes: vec![puntaje(Dimension::Completitud, valor)],
            contrato_fallidos: ids(fallidos),
            ..Veredicto::default()
        }
    }

    #[test]
    fn estables_limpio_con_flaky_de_la_base_no_detecta() {
        let base = base_con_flaky(Dimension::Completitud);
        let caso = caso_oculto(Dimension::Completitud, 0.5, &["t_flaky"]);
        assert!(
            !detecta_dimension(&base, &caso, Dimension::Completitud, false),
            "un test inestable de la base no cuenta en un caso limpio"
        );
    }

    #[test]
    fn estables_limpio_con_flaky_de_la_base_no_detecta_en_funciona() {
        let base = base_con_flaky(Dimension::Funciona);
        let caso = caso_oculto(Dimension::Funciona, 0.5, &["t_flaky"]);
        assert!(
            !detecta_dimension(&base, &caso, Dimension::Funciona, false),
            "la regla también aplica a Funciona"
        );
    }

    #[test]
    fn estables_defecto_con_flaky_y_test_real_detecta() {
        let base = base_con_flaky(Dimension::Completitud);
        let caso = caso_oculto(Dimension::Completitud, 0.5, &["t_flaky", "t_real"]);
        assert!(
            detecta_dimension(&base, &caso, Dimension::Completitud, true),
            "un test estable nuevo que falla sí detecta"
        );
    }

    #[test]
    fn estables_defecto_solo_con_flaky_no_detecta() {
        let base = base_con_flaky(Dimension::Completitud);
        let caso = caso_oculto(Dimension::Completitud, 0.5, &["t_flaky"]);
        assert!(
            !detecta_dimension(&base, &caso, Dimension::Completitud, true),
            "un defecto que solo hace fallar el test inestable no detecta"
        );
    }

    #[test]
    fn estables_limpio_con_flaky_del_contrato_no_detecta() {
        let base = base_contrato_con_flaky();
        let caso = caso_contrato(0.5, &["t_flaky"]);
        assert!(
            !detecta_dimension(&base, &caso, Dimension::Completitud, false),
            "un test inestable del contrato de la base no cuenta en un caso limpio"
        );
    }

    #[test]
    fn estables_defecto_con_flaky_y_test_real_del_contrato_detecta() {
        let base = base_contrato_con_flaky();
        let caso = caso_contrato(0.5, &["t_flaky", "t_real"]);
        assert!(
            detecta_dimension(&base, &caso, Dimension::Completitud, true),
            "un test del contrato estable y nuevo que falla sí detecta"
        );
    }

    #[test]
    fn estables_defecto_solo_con_flaky_del_contrato_no_detecta() {
        let base = base_contrato_con_flaky();
        let caso = caso_contrato(0.5, &["t_flaky"]);
        assert!(
            !detecta_dimension(&base, &caso, Dimension::Completitud, true),
            "un defecto que solo hace fallar el test inestable del contrato no detecta"
        );
    }

    #[test]
    fn estables_mismo_id_en_oculto_y_contrato_cuenta_por_separado() {
        // `t_flaky` es inestable en el oculto de la base. En el contrato es un test nuevo que falla.
        let base = base_con_flaky(Dimension::Completitud);
        let caso = caso_contrato(0.5, &["t_flaky"]);
        assert!(
            detecta_dimension(&base, &caso, Dimension::Completitud, true),
            "el contrato compara sus propios tests malos, no los del oculto"
        );
    }

    #[test]
    fn estables_test_estable_de_la_base_que_pasa_a_inestable_detecta() {
        let base = Veredicto {
            puntajes: vec![puntaje(Dimension::Completitud, 1.0)],
            ..Veredicto::default()
        };
        let caso = Veredicto {
            puntajes: vec![puntaje(Dimension::Completitud, 0.5)],
            inestables: ids(&["t_x"]),
            ..Veredicto::default()
        };
        assert!(
            detecta_dimension(&base, &caso, Dimension::Completitud, true),
            "un test que era estable en la base y ahora es inestable es un test malo nuevo"
        );
    }

    #[test]
    fn estables_total_del_oculto_menor_que_la_base_detecta() {
        let base = Veredicto {
            puntajes: vec![puntaje(Dimension::Completitud, 1.0)],
            oculto: Conteo { pasa: 5, total: 5 },
            ..Veredicto::default()
        };
        let caso = Veredicto {
            puntajes: vec![puntaje(Dimension::Completitud, 0.5)],
            oculto: Conteo { pasa: 3, total: 4 },
            ..Veredicto::default()
        };
        assert!(
            detecta_dimension(&base, &caso, Dimension::Completitud, true),
            "un total menor del oculto cuenta como cambio de tests"
        );
    }

    #[test]
    fn estables_pruebas_con_valor_bajo_sin_cambio_de_tests_sigue_detectando() {
        let base = base_con_flaky(Dimension::Pruebas);
        let caso = Veredicto {
            puntajes: vec![puntaje(Dimension::Pruebas, 0.5)],
            ..Veredicto::default()
        };
        assert!(
            permite(&base, &caso, Dimension::Pruebas),
            "la regla no aplica a Pruebas"
        );
        assert!(
            detecta_dimension(&base, &caso, Dimension::Pruebas, true),
            "Pruebas detecta por valor, como antes"
        );
    }

    #[test]
    fn estables_item_con_nivel_bajo_por_flaky_de_la_base_no_detecta() {
        let base = Veredicto {
            puntajes: vec![puntaje_con_item(Dimension::Completitud, 1.0, 3)],
            inestables: ids(&["t_flaky"]),
            ..Veredicto::default()
        };
        let caso = Veredicto {
            puntajes: vec![puntaje_con_item(Dimension::Completitud, 1.0, 0)],
            ocultos_fallidos: ids(&["t_flaky"]),
            ..Veredicto::default()
        };
        assert_eq!(
            detecta_item(
                &base,
                &caso,
                Dimension::Completitud,
                "completitud.requisitos",
                true
            ),
            Some(false),
            "el ítem baja por un test inestable de la base: no cuenta"
        );
    }

    #[test]
    fn estables_item_con_nivel_bajo_por_test_nuevo_detecta() {
        let base = Veredicto {
            puntajes: vec![puntaje_con_item(Dimension::Completitud, 1.0, 3)],
            ..Veredicto::default()
        };
        let caso = Veredicto {
            puntajes: vec![puntaje_con_item(Dimension::Completitud, 1.0, 0)],
            ocultos_fallidos: ids(&["t_nuevo"]),
            ..Veredicto::default()
        };
        assert_eq!(
            detecta_item(
                &base,
                &caso,
                Dimension::Completitud,
                "completitud.requisitos",
                true
            ),
            Some(true),
            "el ítem baja por un test estable nuevo: cuenta"
        );
    }

    /// Puntaje de `dimension` en `valor` con un ítem `completitud.requisitos` en `nivel`.
    fn puntaje_con_item(dimension: Dimension, valor: f32, nivel: u8) -> crate::tipos::Puntaje {
        crate::tipos::Puntaje {
            items: vec![nivel_item("completitud.requisitos", nivel)],
            ..puntaje(dimension, valor)
        }
    }
}
