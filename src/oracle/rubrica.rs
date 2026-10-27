//! Rúbricas del Oracle: ítems con niveles anclados (0 a 3). Cada dimensión se puntúa con una.
//! El hash sale de la forma canónica de la struct, así que el formato del TOML no lo cambia.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error as ErrorDerive;

use crate::goal::calcular_hash;
use crate::tipos::{Fuente, NivelItem};

/// Errores de rúbrica. Salen con código 3.
#[derive(Debug, ErrorDerive)]
pub enum ErrorRubrica {
    /// La rúbrica no cumple sus reglas de forma, de fuente o de nivel.
    #[error("rúbrica inválida: {0}")]
    Invalida(String),
}

/// Cómo se fija el nivel de un ítem. En TOML: `"sonda"`, `"juez"` o `"mixto"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mide {
    /// Una sonda determinista fija el nivel.
    Sonda,
    /// Un juez con modelo fija el nivel.
    Juez,
    /// El juez fija el nivel y una sonda pone un tope.
    Mixto,
}

/// Una rúbrica completa. Su `hash` va en `Veredicto.rubricas`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rubrica {
    pub id: String,
    pub version: u32,
    pub item: Vec<ItemRubrica>,
}

/// Un ítem con sus cuatro anclas, una por nivel 0 a 3.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemRubrica {
    pub id: String,
    /// Estándar del que sale el ítem. Sin fuente, el ítem no carga.
    #[serde(default)]
    pub fuente: String,
    pub mide: Mide,
    /// Peso de 1 a 3.
    pub peso: u8,
    /// Si es `true`, nivel 0 en este ítem bloquea la decisión (Tarea 13b).
    pub bloquea: bool,
    pub anclas: Vec<String>,
}

/// Resultado de puntuar una dimensión con su rúbrica.
#[derive(Debug, Clone, PartialEq)]
pub struct Puntuacion {
    /// Media ponderada de 0 a 1. `None` si ningún ítem tiene nivel.
    pub valor: Option<f32>,
    /// Niveles efectivos de los ítems con nivel. La evidencia la rellena el juez o la sonda después.
    pub items: Vec<NivelItem>,
    /// Ids de los ítems sin nivel. No entran al numerador ni al denominador.
    pub faltantes: Vec<String>,
}

/// Rúbricas que el binario trae. Opus agrega aquí `include_str!("../../rubricas/<id>.toml")` por rúbrica.
pub const EMBEBIDAS: &[&str] = &[];

impl Rubrica {
    /// Lee una rúbrica desde su TOML y valida sus reglas.
    pub fn desde_toml(texto: &str) -> Result<Rubrica, ErrorRubrica> {
        let rubrica: Rubrica =
            toml::from_str(texto).map_err(|e| ErrorRubrica::Invalida(e.to_string()))?;
        rubrica.validar()?;
        Ok(rubrica)
    }

    fn validar(&self) -> Result<(), ErrorRubrica> {
        if self.id.trim().is_empty() {
            return Err(invalida("la rúbrica no tiene id".into()));
        }
        if self.item.is_empty() {
            return Err(invalida(format!("{} no tiene ítems", self.id)));
        }
        let prefijo = format!("{}.", self.id);
        let mut vistos = BTreeSet::new();
        for item in &self.item {
            let con_prefijo = item
                .id
                .strip_prefix(&prefijo)
                .is_some_and(|r| !r.is_empty());
            if !con_prefijo {
                return Err(invalida(format!(
                    "el ítem {} no empieza con {prefijo}",
                    item.id
                )));
            }
            if !vistos.insert(item.id.as_str()) {
                return Err(invalida(format!("el ítem {} está repetido", item.id)));
            }
            if item.fuente.trim().is_empty() {
                return Err(invalida(format!("el ítem {} no tiene fuente", item.id)));
            }
            if !(1..=3).contains(&item.peso) {
                return Err(invalida(format!(
                    "el peso de {} debe estar entre 1 y 3",
                    item.id
                )));
            }
            if item.anclas.len() != 4 {
                return Err(invalida(format!(
                    "{} tiene {} anclas; deben ser 4",
                    item.id,
                    item.anclas.len()
                )));
            }
            for (nivel, ancla) in item.anclas.iter().enumerate() {
                if !ancla.starts_with(&format!("{nivel}:")) {
                    return Err(invalida(format!(
                        "la ancla {nivel} de {} debe empezar con \"{nivel}:\"",
                        item.id
                    )));
                }
            }
        }
        Ok(())
    }

    /// Hash de 16 caracteres hex de la forma canónica. Cambiar un ancla, un peso, una fuente
    /// o `bloquea` lo cambia. El formato del TOML no.
    pub fn hash(&self) -> String {
        // La struct se serializa en orden de campos. Una struct de strings y números no falla.
        let canonica = serde_json::to_vec(self).expect("una rúbrica siempre serializa");
        calcular_hash(&canonica)
    }

    /// Etiqueta `<id>@<hash>`, la que va a `Veredicto.rubricas`.
    pub fn etiqueta(&self) -> String {
        format!("{}@{}", self.id, self.hash())
    }

    /// Puntúa la rúbrica: `Σ peso·nivel / (3·Σ peso)` sobre los ítems con nivel.
    /// `niveles` pone el nivel de cada ítem. `topes` pone el nivel máximo de los ítems `mixto`.
    pub fn puntuar(
        &self,
        niveles: &BTreeMap<String, u8>,
        topes: &BTreeMap<String, u8>,
    ) -> Result<Puntuacion, ErrorRubrica> {
        for (id, nivel) in niveles {
            self.buscar(id)?;
            comprobar_nivel(id, *nivel)?;
        }
        for (id, tope) in topes {
            if self.buscar(id)?.mide != Mide::Mixto {
                return Err(invalida(format!(
                    "el tope de {id} requiere mide = \"mixto\""
                )));
            }
            comprobar_nivel(id, *tope)?;
        }
        let mut items = Vec::new();
        let mut faltantes = Vec::new();
        let (mut suma, mut pesos) = (0u32, 0u32);
        for item in &self.item {
            let Some(&nivel) = niveles.get(&item.id) else {
                faltantes.push(item.id.clone());
                continue;
            };
            let nivel = topes.get(&item.id).map_or(nivel, |&tope| nivel.min(tope));
            suma += u32::from(item.peso) * u32::from(nivel);
            pesos += u32::from(item.peso);
            items.push(NivelItem {
                item: item.id.clone(),
                nivel,
                evidencia: Vec::new(),
                fuente: Fuente::Sonda("rubrica".into()),
            });
        }
        let valor = (pesos > 0).then(|| suma as f32 / (3 * pesos) as f32);
        Ok(Puntuacion {
            valor,
            items,
            faltantes,
        })
    }

    fn buscar(&self, id: &str) -> Result<&ItemRubrica, ErrorRubrica> {
        self.item
            .iter()
            .find(|item| item.id == id)
            .ok_or_else(|| invalida(format!("el ítem {id} no existe en {}", self.id)))
    }
}

/// Parsea cada rúbrica de `EMBEBIDAS`. Con la lista vacía devuelve una lista vacía.
pub fn cargar_embebidas() -> Result<Vec<Rubrica>, ErrorRubrica> {
    EMBEBIDAS.iter().copied().map(Rubrica::desde_toml).collect()
}

/// Etiquetas de las rúbricas, ordenadas.
pub fn etiquetas(rubricas: &[Rubrica]) -> Vec<String> {
    let mut salida: Vec<String> = rubricas.iter().map(Rubrica::etiqueta).collect();
    salida.sort();
    salida
}

fn comprobar_nivel(id: &str, nivel: u8) -> Result<(), ErrorRubrica> {
    if nivel > 3 {
        return Err(invalida(format!(
            "el nivel de {id} es {nivel}; debe estar entre 0 y 3"
        )));
    }
    Ok(())
}

fn invalida(mensaje: String) -> ErrorRubrica {
    ErrorRubrica::Invalida(mensaje)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(nombre: &str) -> String {
        let ruta = format!(
            "{}/tests/fixtures/rubricas/{nombre}",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::read_to_string(ruta).expect("lee la fixture")
    }

    fn valida() -> Rubrica {
        Rubrica::desde_toml(&fixture("diseno-valida.toml")).expect("la válida carga")
    }

    fn mapa(pares: &[(&str, u8)]) -> BTreeMap<String, u8> {
        pares
            .iter()
            .map(|(id, nivel)| ((*id).to_string(), *nivel))
            .collect()
    }

    const MODULARIDAD: &str = "diseno.modularidad";
    const CICLOS: &str = "diseno.ciclos";
    const TOPE: &str = "diseno.tope";

    fn cerca(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn rubrica_valida_carga_con_tres_items() {
        let r = valida();
        assert_eq!(r.id, "diseno");
        assert_eq!(r.version, 1);
        assert_eq!(r.item.len(), 3);
        assert_eq!(r.item[1].mide, Mide::Sonda);
        assert!(r.item[1].bloquea);
        assert_eq!(r.item[2].mide, Mide::Mixto);
        assert_eq!(r.item[0].peso, 3);
    }

    #[test]
    fn rubrica_hash_estable_entre_dos_cargas() {
        assert_eq!(valida().hash(), valida().hash());
    }

    #[test]
    fn rubrica_etiqueta_tiene_forma_id_arroba_16_hex() {
        let etiqueta = valida().etiqueta();
        let (id, hash) = etiqueta.split_once('@').expect("tiene @");
        assert_eq!(id, "diseno");
        assert_eq!(hash.len(), 16);
        assert!(
            hash.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
    }

    #[test]
    fn rubrica_item_sin_fuente_no_carga_y_nombra_el_item() {
        let error = Rubrica::desde_toml(&fixture("sin-fuente.toml")).expect_err("no carga");
        assert!(error.to_string().contains(MODULARIDAD), "{error}");
    }

    #[test]
    fn rubrica_ancla_con_nivel_cinco_no_carga() {
        let error = Rubrica::desde_toml(&fixture("nivel-malo.toml")).expect_err("no carga");
        assert!(error.to_string().contains(MODULARIDAD), "{error}");
    }

    #[test]
    fn rubrica_exige_exactamente_cuatro_anclas() {
        let texto = fixture("diseno-valida.toml").replace(
            "  \"3: módulos con una responsabilidad, sin ciclos, con interfaces chicas\",\n",
            "  \"3: módulos con una responsabilidad, sin ciclos, con interfaces chicas\",\n  \"4: sobra\",\n",
        );
        assert!(Rubrica::desde_toml(&texto).is_err());
    }

    #[test]
    fn rubrica_nivel_cuatro_en_niveles_es_error() {
        let niveles = mapa(&[(MODULARIDAD, 4)]);
        assert!(valida().puntuar(&niveles, &BTreeMap::new()).is_err());
    }

    #[test]
    fn rubrica_cambiar_ancla_cambia_hash() {
        let cambiada = Rubrica::desde_toml(&fixture("diseno-ancla-cambiada.toml")).expect("carga");
        assert_ne!(valida().hash(), cambiada.hash());
    }

    #[test]
    fn rubrica_reformatear_no_cambia_hash() {
        let reformateada =
            Rubrica::desde_toml(&fixture("diseno-valida-reformateada.toml")).expect("carga");
        assert_eq!(valida().hash(), reformateada.hash());
    }

    #[test]
    fn rubrica_puntaje_con_todos_en_tres_da_uno() {
        let niveles = mapa(&[(MODULARIDAD, 3), (CICLOS, 3), (TOPE, 3)]);
        let p = valida()
            .puntuar(&niveles, &BTreeMap::new())
            .expect("puntúa");
        assert_eq!(p.valor, Some(1.0));
        assert!(p.faltantes.is_empty());
        assert_eq!(p.items.len(), 3);
    }

    #[test]
    fn rubrica_puntaje_con_todos_en_cero_da_cero() {
        let niveles = mapa(&[(MODULARIDAD, 0), (CICLOS, 0), (TOPE, 0)]);
        let p = valida()
            .puntuar(&niveles, &BTreeMap::new())
            .expect("puntúa");
        assert_eq!(p.valor, Some(0.0));
    }

    #[test]
    fn rubrica_puntaje_sigue_la_formula_ponderada() {
        // Pesos 3, 2 y 1. Niveles 3, 0 y 3: (3·3 + 2·0 + 1·3) / (3·6) = 12 / 18.
        let niveles = mapa(&[(MODULARIDAD, 3), (CICLOS, 0), (TOPE, 3)]);
        let p = valida()
            .puntuar(&niveles, &BTreeMap::new())
            .expect("puntúa");
        assert!(cerca(p.valor.expect("hay valor"), 12.0 / 18.0));
    }

    #[test]
    fn rubrica_puntaje_excluye_item_sin_nivel_y_lo_lista() {
        // Sin nivel para `tope`: (3·3 + 2·0) / (3·5) = 9 / 15. El ítem no entra al denominador.
        let niveles = mapa(&[(MODULARIDAD, 3), (CICLOS, 0)]);
        let p = valida()
            .puntuar(&niveles, &BTreeMap::new())
            .expect("puntúa");
        assert!(cerca(p.valor.expect("hay valor"), 9.0 / 15.0));
        assert_eq!(p.faltantes, vec![TOPE.to_string()]);
        assert_eq!(p.items.len(), 2);
    }

    #[test]
    fn rubrica_puntaje_sin_ningun_nivel_es_none() {
        let p = valida()
            .puntuar(&BTreeMap::new(), &BTreeMap::new())
            .expect("puntúa");
        assert_eq!(p.valor, None);
        assert_eq!(p.faltantes.len(), 3);
        assert!(p.items.is_empty());
    }

    #[test]
    fn rubrica_puntuar_nivel_de_item_inexistente_es_error() {
        let niveles = mapa(&[("diseno.no_existe", 2)]);
        assert!(valida().puntuar(&niveles, &BTreeMap::new()).is_err());
    }

    #[test]
    fn rubrica_tope_mixto_limita_el_nivel() {
        let niveles = mapa(&[(MODULARIDAD, 3), (CICLOS, 3), (TOPE, 3)]);
        let topes = mapa(&[(TOPE, 1)]);
        let p = valida().puntuar(&niveles, &topes).expect("puntúa");
        let nivel_tope = p.items.iter().find(|i| i.item == TOPE).expect("está");
        assert_eq!(nivel_tope.nivel, 1);
        // (3·3 + 2·3 + 1·1) / 18 = 16 / 18.
        assert!(cerca(p.valor.expect("hay valor"), 16.0 / 18.0));
    }

    #[test]
    fn rubrica_tope_sobre_item_juez_es_error() {
        let niveles = mapa(&[(MODULARIDAD, 3)]);
        let topes = mapa(&[(MODULARIDAD, 1)]);
        assert!(valida().puntuar(&niveles, &topes).is_err());
    }

    #[test]
    fn rubrica_tope_sobre_item_inexistente_es_error() {
        let topes = mapa(&[("diseno.no_existe", 1)]);
        assert!(valida().puntuar(&BTreeMap::new(), &topes).is_err());
    }

    #[test]
    fn rubrica_cargar_embebidas_vacio_devuelve_vacio() {
        assert!(EMBEBIDAS.is_empty());
        assert_eq!(cargar_embebidas().expect("carga"), Vec::new());
    }

    #[test]
    fn rubrica_etiquetas_salen_ordenadas() {
        let otra = Rubrica::desde_toml(&fixture("diseno-ancla-cambiada.toml")).expect("carga");
        let salida = etiquetas(&[valida(), otra]);
        let mut ordenadas = salida.clone();
        ordenadas.sort();
        assert_eq!(salida, ordenadas);
        assert_eq!(salida.len(), 2);
    }
}
