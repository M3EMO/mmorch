//! Ítems inestables del juez de Estructura, Arquitectura y Diseño (Tarea 83, punto 2). La base se juzga tres
//! veces. Un ítem del juez que varía dos niveles o más, o que falta en algún juicio, no mide la dimensión.
//! Son funciones puras: `calibra` las llama con los tres veredictos de la base.

use std::collections::{BTreeMap, BTreeSet};

use crate::oracle::rubrica::{Mide, Rubrica};
use crate::tipos::{Dimension, NivelItem, Veredicto};

/// Juicios extra de la base. Con los dos extra, la base se juzga tres veces en las dimensiones con juez.
pub(super) const JUICIOS_EXTRA: usize = 2;

/// Pesos de la rúbrica de una dimensión: el de cada ítem y los ítems que mide el juez.
pub(super) struct Pesos {
    /// Peso de cada ítem de la rúbrica, sea de sonda o del juez.
    pub todos: BTreeMap<String, u32>,
    /// Ids de los ítems `juez` o `mixto`.
    pub del_juez: BTreeSet<String>,
}

/// Pesos y los ítems del juez de las rúbricas de una dimensión. `rubricas` ya viene filtrada por dimensión.
pub(super) fn pesos_de(rubricas: &[Rubrica]) -> Pesos {
    let todos = rubricas
        .iter()
        .flat_map(|r| r.item.iter())
        .map(|i| (i.id.clone(), u32::from(i.peso)))
        .collect();
    let del_juez = rubricas
        .iter()
        .flat_map(|r| r.item.iter())
        .filter(|i| i.mide != Mide::Sonda)
        .map(|i| i.id.clone())
        .collect();
    Pesos { todos, del_juez }
}

/// Diferencia de niveles que vuelve inestable un ítem del juez entre los juicios de la base.
const DIFERENCIA_INESTABLE: u8 = 2;

/// Nivel de `item` en la `dimension` de `veredicto`. `None` si el puntaje o el ítem faltan.
fn nivel_en(veredicto: &Veredicto, dimension: Dimension, item: &str) -> Option<u8> {
    veredicto
        .puntajes
        .iter()
        .find(|p| p.dimension == dimension)?
        .items
        .iter()
        .find(|i| i.item == item)
        .map(|i| i.nivel)
}

/// Ítems del juez inestables entre los `juicios` de la base. Un ítem es inestable si falta en algún juicio, o si
/// sus niveles varían `DIFERENCIA_INESTABLE` o más. Un ítem ausente en todos los juicios no cuenta.
pub(super) fn inestables_de(
    juicios: &[&Veredicto],
    dimension: Dimension,
    del_juez: &BTreeSet<String>,
) -> BTreeSet<String> {
    del_juez
        .iter()
        .filter(|item| {
            let niveles: Vec<Option<u8>> = juicios
                .iter()
                .map(|v| nivel_en(v, dimension, item))
                .collect();
            let presentes: Vec<u8> = niveles.iter().flatten().copied().collect();
            if presentes.is_empty() {
                return false;
            }
            let (Some(max), Some(min)) = (presentes.iter().max(), presentes.iter().min()) else {
                return false;
            };
            presentes.len() < niveles.len() || max - min >= DIFERENCIA_INESTABLE
        })
        .cloned()
        .collect()
}

/// Mediana de tres niveles: el del medio al ordenarlos.
fn mediana(mut niveles: Vec<u8>) -> u8 {
    niveles.sort_unstable();
    niveles[niveles.len() / 2]
}

/// Valor de una dimensión con `items`: `Σ peso·nivel / (3·Σ peso)`, sobre los ítems con peso. Cero sin pesos.
pub(super) fn valor_de(items: &[NivelItem], pesos: &BTreeMap<String, u32>) -> f32 {
    let (suma, peso) = items
        .iter()
        .filter_map(|i| pesos.get(&i.item).map(|p| (*p, u32::from(i.nivel))))
        .fold((0u32, 0u32), |(suma, peso), (p, nivel)| {
            (suma + p * nivel, peso + p)
        });
    if peso == 0 {
        0.0
    } else {
        suma as f32 / (3 * peso) as f32
    }
}

/// Base efectiva: el primer juicio con la mediana de cada ítem estable y sin los inestables. Recalcula el valor
/// solo si cambió algún ítem.
pub(super) fn base_efectiva(
    primera: &Veredicto,
    juicios: &[&Veredicto],
    dimension: Dimension,
    inestables: &BTreeSet<String>,
    pesos: &BTreeMap<String, u32>,
) -> Veredicto {
    let mut base = primera.clone();
    let Some(puntaje) = base.puntajes.iter_mut().find(|p| p.dimension == dimension) else {
        return base;
    };
    let mut cambio = false;
    let mut items = Vec::with_capacity(puntaje.items.len());
    for mut item in std::mem::take(&mut puntaje.items) {
        if inestables.contains(&item.item) {
            cambio = true;
            continue;
        }
        let niveles: Vec<u8> = juicios
            .iter()
            .filter_map(|v| nivel_en(v, dimension, &item.item))
            .collect();
        if niveles.len() == juicios.len() {
            let mediana = mediana(niveles);
            if mediana != item.nivel {
                item.nivel = mediana;
                cambio = true;
            }
        }
        items.push(item);
    }
    puntaje.items = items;
    if cambio {
        puntaje.valor = valor_de(&puntaje.items, pesos);
    }
    base
}

/// Quita de `veredicto` los ítems inestables de `dimension` y recalcula su valor si quitó alguno.
pub(super) fn sin_inestables(
    veredicto: &mut Veredicto,
    dimension: Dimension,
    inestables: &BTreeSet<String>,
    pesos: &BTreeMap<String, u32>,
) {
    let Some(puntaje) = veredicto
        .puntajes
        .iter_mut()
        .find(|p| p.dimension == dimension)
    else {
        return;
    };
    let antes = puntaje.items.len();
    puntaje.items.retain(|i| !inestables.contains(&i.item));
    if puntaje.items.len() != antes {
        puntaje.valor = valor_de(&puntaje.items, pesos);
    }
}

/// `true` si el peso de los ítems estables no llega a la mitad del peso de la dimensión.
pub(super) fn dimension_inestable(pesos: &Pesos, inestables: &BTreeSet<String>) -> bool {
    let total: u32 = pesos.todos.values().sum();
    let quitado: u32 = inestables.iter().filter_map(|i| pesos.todos.get(i)).sum();
    2 * (total - quitado) < total
}

/// Motivo de la fila de una dimensión inestable. Nombra sus ítems.
pub(super) fn motivo(inestables: &BTreeSet<String>) -> String {
    let ids: Vec<&str> = inestables.iter().map(String::as_str).collect();
    format!("inestable en esta tarea: {}", ids.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tipos::{Fuente, NivelItem, Puntaje};

    const MODULARIDAD: &str = "diseno.modularidad";
    const ROBUSTEZ: &str = "diseno.robustez";
    const ERRORES: &str = "diseno.errores";

    /// Veredicto de Diseño con los `niveles` de sus ítems del juez y valor 1.
    fn juicio(niveles: &[(&str, u8)]) -> Veredicto {
        Veredicto {
            puntajes: vec![Puntaje {
                inestables: Vec::new(),
                motivo_sin_calibrar: None,
                dimension: Dimension::Diseno,
                valor: 1.0,
                calibrada: false,
                exactitud_balanceada: None,
                items: niveles
                    .iter()
                    .map(|(item, nivel)| NivelItem {
                        item: (*item).into(),
                        nivel: *nivel,
                        evidencia: vec![],
                        fuente: Fuente::Juez {
                            modelo: "deepseek-reasoner".into(),
                        },
                    })
                    .collect(),
                sin_veredicto: None,
                faltantes: vec![],
            }],
            ..Veredicto::default()
        }
    }

    /// Pesos de prueba: modularidad 3, robustez 3 y errores 2. Total 8.
    fn pesos_de_prueba() -> Pesos {
        let todos: BTreeMap<String, u32> = [(MODULARIDAD, 3), (ROBUSTEZ, 3), (ERRORES, 2)]
            .into_iter()
            .map(|(item, peso)| (item.to_string(), peso))
            .collect();
        let del_juez = todos.keys().cloned().collect();
        Pesos { todos, del_juez }
    }

    fn conjunto(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|i| (*i).to_string()).collect()
    }

    #[test]
    fn item_que_falta_en_un_juicio_de_tres_es_inestable() {
        let uno = juicio(&[(MODULARIDAD, 2)]);
        let dos = juicio(&[]);
        let tres = juicio(&[(MODULARIDAD, 2)]);
        let inestables = inestables_de(
            &[&uno, &dos, &tres],
            Dimension::Diseno,
            &pesos_de_prueba().del_juez,
        );
        assert_eq!(inestables, conjunto(&[MODULARIDAD]));
    }

    #[test]
    fn item_con_3_2_3_es_estable_y_la_base_lo_pone_en_3() {
        let uno = juicio(&[(MODULARIDAD, 3)]);
        let dos = juicio(&[(MODULARIDAD, 2)]);
        let tres = juicio(&[(MODULARIDAD, 3)]);
        let juicios = [&uno, &dos, &tres];
        let pesos = pesos_de_prueba();
        let inestables = inestables_de(&juicios, Dimension::Diseno, &pesos.del_juez);
        assert!(inestables.is_empty(), "{inestables:?}");
        let base = base_efectiva(&uno, &juicios, Dimension::Diseno, &inestables, &pesos.todos);
        let nivel = base.puntajes[0].items[0].nivel;
        assert_eq!(nivel, 3);
    }

    #[test]
    fn dimension_con_estables_de_menos_de_la_mitad_es_inestable() {
        let pesos = pesos_de_prueba();
        // Total 8. Quitar modularidad deja 5: la mitad de 8 es 4, así que queda estable.
        assert!(!dimension_inestable(&pesos, &conjunto(&[MODULARIDAD])));
        // Quitar modularidad y errores deja 3: menos de la mitad, la dimensión queda inestable.
        assert!(dimension_inestable(
            &pesos,
            &conjunto(&[MODULARIDAD, ERRORES])
        ));
    }

    #[test]
    fn motivo_de_dimension_inestable_nombra_sus_items() {
        let motivo = motivo(&conjunto(&[ERRORES, MODULARIDAD]));
        assert!(motivo.starts_with("inestable en esta tarea"), "{motivo}");
        assert!(motivo.contains(MODULARIDAD), "{motivo}");
        assert!(motivo.contains(ERRORES), "{motivo}");
    }

    #[test]
    fn sin_inestables_quita_el_item_y_recalcula_el_valor() {
        let mut veredicto = juicio(&[(MODULARIDAD, 3), (ROBUSTEZ, 3), (ERRORES, 0)]);
        sin_inestables(
            &mut veredicto,
            Dimension::Diseno,
            &conjunto(&[MODULARIDAD]),
            &pesos_de_prueba().todos,
        );
        let puntaje = &veredicto.puntajes[0];
        assert!(puntaje.items.iter().all(|i| i.item != MODULARIDAD));
        // Robustez 3 con peso 3, errores 0 con peso 2: 9 / (3 * 5) = 0,6.
        assert!((puntaje.valor - 0.6).abs() < 1e-6, "{}", puntaje.valor);
    }
}
