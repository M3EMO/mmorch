//! Qué tests cuentan para detectar un defecto en Completitud y Funciona (Tarea 59). Un test inestable de la
//! base no cuenta, ni en un caso con defecto ni en uno limpio. Tampoco cuenta un test ruidoso (Tarea 63): uno
//! que varió entre la base y los limpios. Las otras dimensiones no cambian.

use std::collections::BTreeSet;

use crate::oracle::requisitos::PREFIJO_CONTRATO;
use crate::tipos::{Dimension, Hallazgo, Veredicto};

/// Si la dimensión del caso puede detectar un defecto. Completitud y Funciona lo pueden solo con un
/// test malo nuevo, un total menor o un hallazgo nuevo. Las demás dimensiones siempre pueden.
pub(super) fn permite(
    base: &Veredicto,
    veredicto: &Veredicto,
    dimension: Dimension,
    ruidosos: &BTreeSet<String>,
) -> bool {
    if !matches!(dimension, Dimension::Completitud | Dimension::Funciona) {
        return true;
    }
    hay_test_malo_nuevo(base, veredicto, ruidosos)
        || total_bajo(base, veredicto)
        || hallazgo_nuevo(base, veredicto, dimension, ruidosos).is_some()
}

/// Estado de un test en un veredicto.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Estado {
    Ok,
    Fallido,
    Inestable,
}

/// Estado por clave (`oculto:<id>` o `contrato:<id>`) de los tests que aparecen en alguna lista de `v`.
fn estados(v: &Veredicto) -> std::collections::BTreeMap<String, Estado> {
    let mut mapa = std::collections::BTreeMap::new();
    let listas: [(&str, &Vec<String>, Estado); 4] = [
        ("oculto", &v.ocultos_fallidos, Estado::Fallido),
        ("oculto", &v.inestables, Estado::Inestable),
        ("contrato", &v.contrato_fallidos, Estado::Fallido),
        ("contrato", &v.inestables_contrato, Estado::Inestable),
    ];
    for (lado, lista, estado) in listas {
        for id in lista {
            mapa.insert(format!("{lado}:{id}"), estado);
        }
    }
    mapa
}

/// Tests ruidosos (Tarea 63): los que aparecen con más de un estado entre la base y los casos limpios. Un
/// test sin lista cuenta como `Ok`. La tasa real de un test no se conoce con pocas repeticiones de la base:
/// los limpios ya pagaron esa medición. Los ruidosos no cuentan para detectar.
pub(super) fn ruidosos(base: &Veredicto, limpios: &[&Veredicto]) -> BTreeSet<String> {
    let mut vistos: Vec<std::collections::BTreeMap<String, Estado>> = vec![estados(base)];
    vistos.extend(limpios.iter().map(|v| estados(v)));
    let claves: BTreeSet<&String> = vistos.iter().flat_map(|m| m.keys()).collect();
    claves
        .into_iter()
        .filter(|clave| {
            let distintos: BTreeSet<Estado> = vistos
                .iter()
                .map(|m| m.get(*clave).copied().unwrap_or(Estado::Ok))
                .collect();
            distintos.len() > 1
        })
        .cloned()
        .collect()
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
fn hay_test_malo_nuevo(
    base: &Veredicto,
    veredicto: &Veredicto,
    ruidosos: &BTreeSet<String>,
) -> bool {
    let ocultos_base = malos_ocultos(base);
    let contrato_base = malos_contrato(base);
    malos_ocultos(veredicto)
        .iter()
        .any(|id| !ocultos_base.contains(id) && !ruidosos.contains(&format!("oculto:{id}")))
        || malos_contrato(veredicto)
            .iter()
            .any(|id| !contrato_base.contains(id) && !ruidosos.contains(&format!("contrato:{id}")))
}

/// El total del oculto o del contrato bajó frente a la base.
fn total_bajo(base: &Veredicto, veredicto: &Veredicto) -> bool {
    veredicto.oculto.total < base.oculto.total || veredicto.contrato.total < base.contrato.total
}

/// Clave de `ruidosos` de la cita de un hallazgo. Una cita `contrato:...` queda igual. Otra pasa a `oculto:<cita>`.
fn clave_de_cita(cita: &str) -> String {
    if cita.starts_with(PREFIJO_CONTRATO) {
        cita.to_string()
    } else {
        format!("oculto:{cita}")
    }
}

/// Primer hallazgo de `dimension` con un id que la base no tiene, y cuya cita no es un test ruidoso. Gana el
/// primero en el orden del veredicto. Lo usan `motivo_de_deteccion` y `permite`: un solo criterio.
pub(super) fn hallazgo_nuevo<'v>(
    base: &Veredicto,
    veredicto: &'v Veredicto,
    dimension: Dimension,
    ruidosos: &BTreeSet<String>,
) -> Option<&'v Hallazgo> {
    let ids_base: BTreeSet<&str> = base.hallazgos.iter().map(|h| h.id.as_str()).collect();
    veredicto.hallazgos.iter().find(|h| {
        h.dimension == dimension
            && !ids_base.contains(h.id.as_str())
            && !ruidosos.contains(&clave_de_cita(&h.cita))
    })
}

#[cfg(test)]
mod tests {
    use super::super::detecta_item;
    use super::super::motivo_de_deteccion;
    use super::super::tests::{nivel_item, puntaje};
    use super::permite;
    use crate::tipos::{Conteo, Dimension, Veredicto};
    use std::collections::BTreeSet;

    /// Si la dimensión detecta el caso: `motivo_de_deteccion` con motivo.
    fn detecta_dimension(base: &Veredicto, veredicto: &Veredicto, dimension: Dimension) -> bool {
        motivo_de_deteccion(base, veredicto, dimension, &BTreeSet::new()).is_some()
    }

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
            !detecta_dimension(&base, &caso, Dimension::Completitud),
            "un test inestable de la base no cuenta en un caso limpio"
        );
    }

    #[test]
    fn estables_limpio_con_flaky_de_la_base_no_detecta_en_funciona() {
        let base = base_con_flaky(Dimension::Funciona);
        let caso = caso_oculto(Dimension::Funciona, 0.5, &["t_flaky"]);
        assert!(
            !detecta_dimension(&base, &caso, Dimension::Funciona),
            "la regla también aplica a Funciona"
        );
    }

    #[test]
    fn estables_defecto_con_flaky_y_test_real_detecta() {
        let base = base_con_flaky(Dimension::Completitud);
        let caso = caso_oculto(Dimension::Completitud, 0.5, &["t_flaky", "t_real"]);
        assert!(
            detecta_dimension(&base, &caso, Dimension::Completitud),
            "un test estable nuevo que falla sí detecta"
        );
    }

    #[test]
    fn estables_defecto_solo_con_flaky_no_detecta() {
        let base = base_con_flaky(Dimension::Completitud);
        let caso = caso_oculto(Dimension::Completitud, 0.5, &["t_flaky"]);
        assert!(
            !detecta_dimension(&base, &caso, Dimension::Completitud),
            "un defecto que solo hace fallar el test inestable no detecta"
        );
    }

    #[test]
    fn estables_limpio_con_flaky_del_contrato_no_detecta() {
        let base = base_contrato_con_flaky();
        let caso = caso_contrato(0.5, &["t_flaky"]);
        assert!(
            !detecta_dimension(&base, &caso, Dimension::Completitud),
            "un test inestable del contrato de la base no cuenta en un caso limpio"
        );
    }

    #[test]
    fn estables_defecto_con_flaky_y_test_real_del_contrato_detecta() {
        let base = base_contrato_con_flaky();
        let caso = caso_contrato(0.5, &["t_flaky", "t_real"]);
        assert!(
            detecta_dimension(&base, &caso, Dimension::Completitud),
            "un test del contrato estable y nuevo que falla sí detecta"
        );
    }

    #[test]
    fn estables_defecto_solo_con_flaky_del_contrato_no_detecta() {
        let base = base_contrato_con_flaky();
        let caso = caso_contrato(0.5, &["t_flaky"]);
        assert!(
            !detecta_dimension(&base, &caso, Dimension::Completitud),
            "un defecto que solo hace fallar el test inestable del contrato no detecta"
        );
    }

    #[test]
    fn estables_mismo_id_en_oculto_y_contrato_cuenta_por_separado() {
        // `t_flaky` es inestable en el oculto de la base. En el contrato es un test nuevo que falla.
        let base = base_con_flaky(Dimension::Completitud);
        let caso = caso_contrato(0.5, &["t_flaky"]);
        assert!(
            detecta_dimension(&base, &caso, Dimension::Completitud),
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
            detecta_dimension(&base, &caso, Dimension::Completitud),
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
            detecta_dimension(&base, &caso, Dimension::Completitud),
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
            permite(&base, &caso, Dimension::Pruebas, &BTreeSet::new()),
            "la regla no aplica a Pruebas"
        );
        assert!(
            detecta_dimension(&base, &caso, Dimension::Pruebas),
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
                true,
                &BTreeSet::new()
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
                true,
                &BTreeSet::new()
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

    #[test]
    fn estables_ruidosos_con_base_y_limpios_identicos_es_vacio() {
        let base = base_con_flaky(Dimension::Completitud);
        let limpio = base.clone();
        assert!(super::ruidosos(&base, &[&limpio, &limpio]).is_empty());
    }

    #[test]
    fn estables_ruidosos_nombra_el_test_que_cambia_de_estado_y_separa_oculto_de_contrato() {
        let base = Veredicto::default();
        let l1 = Veredicto {
            inestables: ids(&["t_mezcla"]),
            contrato_fallidos: ids(&["t_fijo"]),
            ..Veredicto::default()
        };
        let l2 = Veredicto {
            contrato_fallidos: ids(&["t_fijo"]),
            ..Veredicto::default()
        };
        let r = super::ruidosos(&base, &[&l1, &l2]);
        assert!(r.contains("oculto:t_mezcla"), "{r:?}");
        // `t_fijo` falla en los dos limpios pero no en la base: también cambió de estado.
        assert!(r.contains("contrato:t_fijo"), "{r:?}");
        assert_eq!(r.len(), 2);
    }
}
