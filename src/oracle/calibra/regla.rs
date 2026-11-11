//! Regla de detección de las dimensiones con juez: Estructura, Arquitectura y Diseño (Tarea 63, punto 2).
//! Un hallazgo nuevo del juez o una baja de ítem por sí solos no cuentan: el juez tira ruido en estas
//! dimensiones. Cuentan las sondas, una cita del juez cerca del defecto, o una baja fuerte del ítem apuntado.

use std::collections::BTreeSet;

use crate::tipos::{Dimension, Fuente, Hallazgo, Puntaje, Veredicto};

use super::{nivel_de, puntaje_de};

/// Cita del juez a lo sumo a tantas líneas del defecto.
const DISTANCIA_CITA: u32 = 5;
/// Baja de niveles de un ítem del defecto que ya cuenta como detección.
const BAJA_ITEM_FUERTE: u8 = 2;

/// Lugar del defecto inyectado: archivo relativo a la copia y línea 1-based. Línea 0 es el archivo entero.
pub(super) struct Lugar {
    pub archivo: String,
    pub linea: u32,
}

/// Si la dimensión usa esta regla. Las demás dimensiones siguen con `motivo_de_deteccion`.
pub(super) fn es_con_juez(dimension: Dimension) -> bool {
    matches!(
        dimension,
        Dimension::Estructura | Dimension::Arquitectura | Dimension::Diseno
    )
}

/// Por qué un caso con juez detecta, o `None`. Orden de prioridad: un hallazgo de sonda nuevo, un ítem de
/// sonda de la base que baja, una cita del juez en el lugar del defecto, y un ítem del defecto que baja
/// `BAJA_ITEM_FUERTE` niveles o más. Un hallazgo del juez nuevo o una baja de 1 nivel no cuentan por sí solos.
/// Sin puntaje de la dimensión, o con su veredicto sin veredicto, no detecta: lo decide `calibra`.
pub(super) fn detecta_con_juez(
    base: &Veredicto,
    veredicto: &Veredicto,
    dimension: Dimension,
    item_defecto: Option<&str>,
    lugar: Option<&Lugar>,
) -> Option<String> {
    let pv = puntaje_de(veredicto, dimension)?;
    let pb = puntaje_de(base, dimension)?;
    if pv.sin_veredicto.is_some() {
        return None;
    }
    hallazgo_de_sonda_nuevo(base, veredicto, dimension)
        .or_else(|| item_de_sonda_bajo(pb, pv))
        .or_else(|| cita_del_juez(veredicto, dimension, lugar))
        .or_else(|| item_fuerte_del_defecto(pb, pv, item_defecto))
}

/// Un hallazgo de sonda de la dimensión con un id que la base no tiene.
fn hallazgo_de_sonda_nuevo(
    base: &Veredicto,
    veredicto: &Veredicto,
    dimension: Dimension,
) -> Option<String> {
    let ids_base: BTreeSet<&str> = base.hallazgos.iter().map(|h| h.id.as_str()).collect();
    veredicto
        .hallazgos
        .iter()
        .any(|h| {
            h.dimension == dimension
                && matches!(h.fuente, Fuente::Sonda(_))
                && !ids_base.contains(h.id.as_str())
        })
        .then(|| "hallazgo_sonda".to_string())
}

/// El primer ítem de sonda de la base, en su orden, que el caso baja aunque sea 1 nivel.
fn item_de_sonda_bajo(pb: &Puntaje, pv: &Puntaje) -> Option<String> {
    pb.items
        .iter()
        .find(|ib| {
            matches!(ib.fuente, Fuente::Sonda(_))
                && nivel_de(pv, &ib.item).is_some_and(|nivel| nivel < ib.nivel)
        })
        .map(|ib| format!("item_sonda:{}", ib.item))
}

/// Una cita del juez de la dimensión en el lugar del defecto. Nueva o ya en la base: cuenta igual.
fn cita_del_juez(
    veredicto: &Veredicto,
    dimension: Dimension,
    lugar: Option<&Lugar>,
) -> Option<String> {
    let lugar = lugar?;
    veredicto
        .hallazgos
        .iter()
        .any(|h| {
            h.dimension == dimension
                && matches!(h.fuente, Fuente::Juez { .. })
                && en_el_lugar(h, lugar)
        })
        .then(|| "cita_juez".to_string())
}

/// Si la cita está en el mismo archivo que el defecto y a `DISTANCIA_CITA` líneas o menos. Línea 0 en
/// cualquiera de los dos lados vale el archivo entero: basta el mismo archivo.
fn en_el_lugar(h: &Hallazgo, lugar: &Lugar) -> bool {
    if sin_punto(&h.archivo) != sin_punto(&lugar.archivo) {
        return false;
    }
    lugar.linea == 0 || h.linea == 0 || h.linea.abs_diff(lugar.linea) <= DISTANCIA_CITA
}

/// La ruta sin los `./` iniciales, para comparar archivos.
fn sin_punto(ruta: &str) -> &str {
    let mut ruta = ruta;
    while let Some(resto) = ruta.strip_prefix("./") {
        ruta = resto;
    }
    ruta
}

/// El ítem que apunta el defecto, si baja `BAJA_ITEM_FUERTE` niveles o más.
fn item_fuerte_del_defecto(
    pb: &Puntaje,
    pv: &Puntaje,
    item_defecto: Option<&str>,
) -> Option<String> {
    let item = item_defecto?;
    let nivel_base = nivel_de(pb, item)?;
    let nivel_caso = nivel_de(pv, item)?;
    (nivel_base.saturating_sub(nivel_caso) >= BAJA_ITEM_FUERTE)
        .then(|| format!("item_baja_2:{item}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tipos::{Fuente, Hallazgo, NivelItem, Puntaje};

    const ITEM_DISENO: &str = "diseno.modularidad";
    const ITEM_ESTRUCTURA: &str = "estructura.capas";

    fn sonda() -> Fuente {
        Fuente::Sonda("clippy".into())
    }

    fn juez() -> Fuente {
        Fuente::Juez {
            modelo: "deepseek-reasoner".into(),
        }
    }

    fn nivel(item: &str, nivel: u8, fuente: Fuente) -> NivelItem {
        NivelItem {
            item: item.into(),
            nivel,
            evidencia: vec![],
            fuente,
        }
    }

    fn hallazgo(
        dimension: Dimension,
        id: &str,
        fuente: Fuente,
        archivo: &str,
        linea: u32,
    ) -> Hallazgo {
        Hallazgo {
            dimension,
            severidad: 2,
            archivo: archivo.into(),
            linea,
            cita: "x".into(),
            detalle: String::new(),
            fuente,
            id: id.into(),
            impacto: vec![],
        }
    }

    /// Veredicto de `dimension` con `valor`, sus `items` y sus `hallazgos`.
    fn veredicto(
        dimension: Dimension,
        valor: f32,
        items: Vec<NivelItem>,
        hallazgos: Vec<Hallazgo>,
    ) -> Veredicto {
        Veredicto {
            puntajes: vec![Puntaje {
                motivo_sin_calibrar: None,
                dimension,
                valor,
                calibrada: false,
                exactitud_balanceada: None,
                items,
                sin_veredicto: None,
                faltantes: vec![],
            }],
            hallazgos,
            ..Veredicto::default()
        }
    }

    fn lugar(archivo: &str, linea: u32) -> Lugar {
        Lugar {
            archivo: archivo.into(),
            linea,
        }
    }

    #[test]
    fn juez_nuevo_en_otro_archivo_con_item_que_baja_uno_y_valor_menor_no_detecta() {
        let base = veredicto(
            Dimension::Diseno,
            1.0,
            vec![nivel(ITEM_DISENO, 3, juez())],
            vec![],
        );
        let caso = veredicto(
            Dimension::Diseno,
            0.5,
            vec![nivel(ITEM_DISENO, 2, juez())],
            vec![hallazgo(
                Dimension::Diseno,
                "nuevo",
                juez(),
                "src/otro.rs",
                10,
            )],
        );
        let motivo = detecta_con_juez(
            &base,
            &caso,
            Dimension::Diseno,
            Some(ITEM_DISENO),
            Some(&lugar("src/app.rs", 10)),
        );
        assert_eq!(motivo, None);
    }

    #[test]
    fn hallazgo_de_sonda_nuevo_detecta_como_hallazgo_sonda() {
        let base = veredicto(Dimension::Diseno, 1.0, vec![], vec![]);
        let caso = veredicto(
            Dimension::Diseno,
            1.0,
            vec![],
            vec![hallazgo(
                Dimension::Diseno,
                "nueva",
                sonda(),
                "src/app.rs",
                3,
            )],
        );
        let motivo = detecta_con_juez(&base, &caso, Dimension::Diseno, None, None);
        assert_eq!(motivo.as_deref(), Some("hallazgo_sonda"));
    }

    #[test]
    fn hallazgo_de_sonda_con_id_de_la_base_no_cuenta() {
        let previo = hallazgo(Dimension::Diseno, "vieja", sonda(), "src/app.rs", 3);
        let base = veredicto(Dimension::Diseno, 1.0, vec![], vec![previo.clone()]);
        let caso = veredicto(Dimension::Diseno, 1.0, vec![], vec![previo]);
        let motivo = detecta_con_juez(&base, &caso, Dimension::Diseno, None, None);
        assert_eq!(motivo, None);
    }

    #[test]
    fn item_de_sonda_que_baja_uno_detecta_como_item_sonda() {
        let base = veredicto(
            Dimension::Estructura,
            1.0,
            vec![nivel(ITEM_ESTRUCTURA, 2, sonda())],
            vec![],
        );
        let caso = veredicto(
            Dimension::Estructura,
            1.0,
            vec![nivel(ITEM_ESTRUCTURA, 1, sonda())],
            vec![],
        );
        let motivo = detecta_con_juez(&base, &caso, Dimension::Estructura, None, None);
        assert_eq!(motivo, Some(format!("item_sonda:{ITEM_ESTRUCTURA}")));
    }

    #[test]
    fn item_de_sonda_igual_al_de_la_base_no_detecta() {
        let base = veredicto(
            Dimension::Estructura,
            1.0,
            vec![nivel(ITEM_ESTRUCTURA, 2, sonda())],
            vec![],
        );
        let caso = base.clone();
        let motivo = detecta_con_juez(&base, &caso, Dimension::Estructura, None, None);
        assert_eq!(motivo, None);
    }

    #[test]
    fn cita_del_juez_a_cinco_lineas_del_defecto_detecta_como_cita_juez() {
        let base = veredicto(Dimension::Arquitectura, 1.0, vec![], vec![]);
        let caso = veredicto(
            Dimension::Arquitectura,
            1.0,
            vec![],
            vec![hallazgo(
                Dimension::Arquitectura,
                "h1",
                juez(),
                "src/a.rs",
                15,
            )],
        );
        let motivo = detecta_con_juez(
            &base,
            &caso,
            Dimension::Arquitectura,
            Some("arquitectura.ciclo_entre_capas"),
            Some(&lugar("src/a.rs", 10)),
        );
        assert_eq!(motivo.as_deref(), Some("cita_juez"));
    }

    #[test]
    fn cita_del_juez_a_seis_lineas_del_defecto_no_detecta() {
        let base = veredicto(Dimension::Arquitectura, 1.0, vec![], vec![]);
        let caso = veredicto(
            Dimension::Arquitectura,
            1.0,
            vec![],
            vec![hallazgo(
                Dimension::Arquitectura,
                "h1",
                juez(),
                "src/a.rs",
                15,
            )],
        );
        let motivo = detecta_con_juez(
            &base,
            &caso,
            Dimension::Arquitectura,
            Some("arquitectura.ciclo_entre_capas"),
            Some(&lugar("src/a.rs", 9)),
        );
        assert_eq!(motivo, None);
    }

    #[test]
    fn cita_del_juez_en_otro_archivo_no_detecta() {
        let base = veredicto(Dimension::Arquitectura, 1.0, vec![], vec![]);
        let caso = veredicto(
            Dimension::Arquitectura,
            1.0,
            vec![],
            vec![hallazgo(
                Dimension::Arquitectura,
                "h1",
                juez(),
                "src/a.rs",
                15,
            )],
        );
        let motivo = detecta_con_juez(
            &base,
            &caso,
            Dimension::Arquitectura,
            Some("arquitectura.ciclo_entre_capas"),
            Some(&lugar("src/b.rs", 15)),
        );
        assert_eq!(motivo, None);
    }

    #[test]
    fn cita_del_juez_compara_rutas_sin_el_punto_inicial() {
        let base = veredicto(Dimension::Arquitectura, 1.0, vec![], vec![]);
        let caso = veredicto(
            Dimension::Arquitectura,
            1.0,
            vec![],
            vec![hallazgo(
                Dimension::Arquitectura,
                "h1",
                juez(),
                "./src/a.rs",
                15,
            )],
        );
        let motivo = detecta_con_juez(
            &base,
            &caso,
            Dimension::Arquitectura,
            Some("arquitectura.ciclo_entre_capas"),
            Some(&lugar("src/a.rs", 15)),
        );
        assert_eq!(motivo.as_deref(), Some("cita_juez"));
    }

    #[test]
    fn cita_del_juez_con_linea_cero_cuenta_con_el_mismo_archivo() {
        let base = veredicto(Dimension::Arquitectura, 1.0, vec![], vec![]);
        let caso = veredicto(
            Dimension::Arquitectura,
            1.0,
            vec![],
            vec![hallazgo(
                Dimension::Arquitectura,
                "h1",
                juez(),
                "src/a.rs",
                0,
            )],
        );
        let motivo = detecta_con_juez(
            &base,
            &caso,
            Dimension::Arquitectura,
            Some("arquitectura.ciclo_entre_capas"),
            Some(&lugar("src/a.rs", 400)),
        );
        assert_eq!(motivo.as_deref(), Some("cita_juez"));
    }

    #[test]
    fn cita_del_juez_sobre_el_archivo_entero_cuenta_con_el_mismo_archivo() {
        let base = veredicto(Dimension::Arquitectura, 1.0, vec![], vec![]);
        let caso = veredicto(
            Dimension::Arquitectura,
            1.0,
            vec![],
            vec![hallazgo(
                Dimension::Arquitectura,
                "h1",
                juez(),
                "src/a.rs",
                90,
            )],
        );
        let motivo = detecta_con_juez(
            &base,
            &caso,
            Dimension::Arquitectura,
            Some("arquitectura.ciclo_entre_capas"),
            Some(&lugar("src/a.rs", 0)),
        );
        assert_eq!(motivo.as_deref(), Some("cita_juez"));
    }

    #[test]
    fn cita_del_juez_ya_en_la_base_tambien_cuenta() {
        let previo = hallazgo(Dimension::Arquitectura, "h1", juez(), "src/a.rs", 15);
        let base = veredicto(Dimension::Arquitectura, 1.0, vec![], vec![previo.clone()]);
        let caso = veredicto(Dimension::Arquitectura, 1.0, vec![], vec![previo]);
        let motivo = detecta_con_juez(
            &base,
            &caso,
            Dimension::Arquitectura,
            Some("arquitectura.ciclo_entre_capas"),
            Some(&lugar("src/a.rs", 10)),
        );
        assert_eq!(motivo.as_deref(), Some("cita_juez"));
    }

    #[test]
    fn cita_de_otra_dimension_no_cuenta() {
        let base = veredicto(Dimension::Arquitectura, 1.0, vec![], vec![]);
        let caso = veredicto(
            Dimension::Arquitectura,
            1.0,
            vec![],
            vec![hallazgo(Dimension::Seguridad, "h1", juez(), "src/a.rs", 15)],
        );
        let motivo = detecta_con_juez(
            &base,
            &caso,
            Dimension::Arquitectura,
            Some("arquitectura.ciclo_entre_capas"),
            Some(&lugar("src/a.rs", 10)),
        );
        assert_eq!(motivo, None);
    }

    #[test]
    fn item_del_defecto_que_baja_dos_niveles_detecta_como_item_baja_2() {
        let base = veredicto(
            Dimension::Diseno,
            1.0,
            vec![nivel(ITEM_DISENO, 3, juez())],
            vec![],
        );
        let caso = veredicto(
            Dimension::Diseno,
            1.0,
            vec![nivel(ITEM_DISENO, 1, juez())],
            vec![],
        );
        let motivo = detecta_con_juez(&base, &caso, Dimension::Diseno, Some(ITEM_DISENO), None);
        assert_eq!(motivo, Some(format!("item_baja_2:{ITEM_DISENO}")));
    }

    #[test]
    fn item_del_defecto_que_baja_un_nivel_no_detecta() {
        let base = veredicto(
            Dimension::Diseno,
            1.0,
            vec![nivel(ITEM_DISENO, 3, juez())],
            vec![],
        );
        let caso = veredicto(
            Dimension::Diseno,
            1.0,
            vec![nivel(ITEM_DISENO, 2, juez())],
            vec![],
        );
        let motivo = detecta_con_juez(&base, &caso, Dimension::Diseno, Some(ITEM_DISENO), None);
        assert_eq!(motivo, None);
    }
}
