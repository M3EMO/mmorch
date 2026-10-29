//! Completitud por requisito (Tarea 9c): cruza cada `[[requisito]]` con el junit oculto y los
//! escenarios. El nivel sale solo de pruebas, sin LLM.

use std::collections::{BTreeMap, BTreeSet};

use crate::oracle::Clasificacion;
use crate::oracle::junit::ResultadoTest;
use crate::oracle::perfiles::crear_hallazgo;
use crate::oracle::tarea::{Exclusion, Requisito};
use crate::tipos::{
    Cumplimiento, Dimension, EstadoRequisito, Fuente, Hallazgo, NivelItem, Puntaje,
};

/// Resultados de la suite oculta ya clasificados. Los inestables no están en `tests`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ResultadosTests {
    /// Clave `archivo::nombre`, con resultado `Pasa` o `Falla` (u `Omite`).
    pub tests: BTreeMap<String, ResultadoTest>,
    /// Tests con resultados mezclados entre corridas. Nunca cuentan como verdes.
    pub inestables: BTreeSet<String>,
}

/// Texto del hallazgo de un test inestable.
const DETALLE_INESTABLE: &str =
    "comportamiento no determinista: este test pasa y falla sin cambios en el código";

/// Pasa la clasificación de la suite oculta a `ResultadosTests`.
pub fn desde_clasificacion(c: &Clasificacion) -> ResultadosTests {
    let mut tests = c.estables.clone();
    // Un test omitido en todas sus corridas sigue en el universo del requisito, como no verde (A10).
    for id in &c.omitidos {
        tests.insert(id.clone(), ResultadoTest::Omite);
    }
    ResultadosTests {
        tests,
        inestables: c.inestables.iter().cloned().collect(),
    }
}

/// Nivel de cada requisito, en el mismo orden que `reqs`.
pub fn evaluar_requisitos(
    reqs: &[Requisito],
    r: &ResultadosTests,
    escenarios: &BTreeMap<String, bool>,
    entorno: &[Exclusion],
    oraculo_dudoso: &[Exclusion],
    no_evaluables: &BTreeMap<String, String>,
) -> Vec<EstadoRequisito> {
    let medidor = Medidor::new(r, entorno, oraculo_dudoso);
    reqs.iter()
        .map(|req| evaluar_uno(req, &medidor, escenarios, no_evaluables))
        .collect()
}

/// Completitud en 0 a 1: `Σ peso·nivel / (3·Σ peso)`, solo sobre los estados con nivel.
pub fn completitud(estados: &[EstadoRequisito]) -> Option<f32> {
    let medidos: Vec<(u32, u32)> = estados
        .iter()
        .filter_map(|e| e.nivel.map(|n| (u32::from(e.peso), u32::from(n))))
        .collect();
    let peso_total: u32 = medidos.iter().map(|(peso, _)| peso).sum();
    if medidos.is_empty() || peso_total == 0 {
        return None;
    }
    let ponderado: u32 = medidos.iter().map(|(peso, nivel)| peso * nivel).sum();
    Some(ponderado as f32 / (3 * peso_total) as f32)
}

/// Un hallazgo por test inestable que referencia algún requisito.
pub fn hallazgos_inestables(reqs: &[Requisito], r: &ResultadosTests) -> Vec<Hallazgo> {
    let medidor = Medidor::new(r, &[], &[]);
    let mut impacto: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for req in reqs {
        for referencia in req.pruebas.iter().chain(&req.bordes) {
            let inestables = medidor
                .claves(referencia)
                .into_iter()
                .filter(|clave| r.inestables.contains(clave));
            for clave in inestables {
                let ids = impacto.entry(clave).or_default();
                if !ids.contains(&req.id) {
                    ids.push(req.id.clone());
                }
            }
        }
    }
    impacto
        .into_iter()
        .map(|(cita, ids)| {
            let mut hallazgo = crear_hallazgo(
                Dimension::Completitud,
                2,
                "",
                0,
                &cita,
                DETALLE_INESTABLE,
                "completitud",
                &[],
            );
            hallazgo.impacto = ids;
            hallazgo
        })
        .collect()
}

/// Puntaje de la dimensión `Completitud`, con un ítem por requisito que tiene nivel.
pub fn puntaje_completitud(estados: &[EstadoRequisito]) -> Option<Puntaje> {
    let valor = completitud(estados)?;
    let items = estados
        .iter()
        .filter_map(|e| {
            e.nivel.map(|nivel| NivelItem {
                item: format!("completitud.requisito.{}", e.id),
                nivel,
                evidencia: e.evidencia.clone(),
                fuente: Fuente::Sonda("completitud".into()),
            })
        })
        .collect();
    Some(Puntaje {
        dimension: Dimension::Completitud,
        valor,
        calibrada: false,
        exactitud_balanceada: None,
        items,
    })
}

/// Resultado de un test o escenario frente a los resultados.
#[derive(Debug, Clone, PartialEq)]
enum Resultado {
    Pasa,
    Falla,
    Omite,
    Inestable,
    Ausente,
    /// Test de `entorno` u `oraculo_dudoso`. No entra al cálculo.
    Excluido {
        entorno: bool,
        motivo: String,
    },
}

/// Un test o escenario de un requisito, ya medido.
struct Medida {
    id: String,
    resultado: Resultado,
}

impl Medida {
    fn verde(&self) -> bool {
        self.resultado == Resultado::Pasa
    }

    fn excluida(&self) -> bool {
        matches!(self.resultado, Resultado::Excluido { .. })
    }

    /// Evidencia de un test excluido, con su motivo.
    fn texto_excluido(&self) -> Option<String> {
        match &self.resultado {
            Resultado::Excluido {
                entorno: true,
                motivo,
            } => Some(format!("pendiente: {} ({motivo})", self.id)),
            Resultado::Excluido {
                entorno: false,
                motivo,
            } => Some(format!("excluido: {} ({motivo})", self.id)),
            _ => None,
        }
    }

    /// Evidencia de un test considerado: `id: estado`.
    fn etiqueta(&self) -> String {
        let estado = match self.resultado {
            Resultado::Pasa => "pasa",
            Resultado::Falla => "falla",
            Resultado::Omite => "omite",
            Resultado::Inestable => "inestable",
            Resultado::Ausente => "ausente",
            Resultado::Excluido { .. } => "excluido",
        };
        format!("{}: {estado}", self.id)
    }
}

/// Mide las referencias de los requisitos contra los resultados y las exclusiones.
struct Medidor<'a> {
    resultados: &'a ResultadosTests,
    entorno: &'a [Exclusion],
    oraculo_dudoso: &'a [Exclusion],
    /// Claves conocidas: tests, inestables y excluidos.
    universo: BTreeSet<&'a str>,
}

impl<'a> Medidor<'a> {
    fn new(
        resultados: &'a ResultadosTests,
        entorno: &'a [Exclusion],
        oraculo_dudoso: &'a [Exclusion],
    ) -> Self {
        let universo = resultados
            .tests
            .keys()
            .map(String::as_str)
            .chain(resultados.inestables.iter().map(String::as_str))
            .chain(
                entorno
                    .iter()
                    .chain(oraculo_dudoso)
                    .map(|e| e.test.as_str()),
            )
            .collect();
        Medidor {
            resultados,
            entorno,
            oraculo_dudoso,
            universo,
        }
    }

    /// Mide cada referencia. Un id da un test; un archivo da todos sus tests.
    /// Una referencia sin tests cuenta como ausente.
    fn referencias(&self, refs: &[String]) -> Vec<Medida> {
        let mut medidas = Vec::new();
        for referencia in refs {
            let claves = self.claves(referencia);
            if claves.is_empty() {
                medidas.push(Medida {
                    id: referencia.clone(),
                    resultado: Resultado::Ausente,
                });
            }
            for clave in claves {
                let resultado = self.test(&clave);
                medidas.push(Medida {
                    id: clave,
                    resultado,
                });
            }
        }
        medidas
    }

    /// Claves de una referencia. Un id exacto da una sola clave, exista o no.
    fn claves(&self, referencia: &str) -> Vec<String> {
        if referencia.contains("::") {
            return vec![referencia.to_string()];
        }
        let prefijo = format!("{referencia}::");
        self.universo
            .iter()
            .filter(|clave| clave.starts_with(&prefijo))
            .map(|clave| clave.to_string())
            .collect()
    }

    fn test(&self, clave: &str) -> Resultado {
        if let Some(e) = self.entorno.iter().find(|e| e.test == clave) {
            return Resultado::Excluido {
                entorno: true,
                motivo: e.motivo.clone(),
            };
        }
        if let Some(e) = self.oraculo_dudoso.iter().find(|e| e.test == clave) {
            return Resultado::Excluido {
                entorno: false,
                motivo: e.motivo.clone(),
            };
        }
        if self.resultados.inestables.contains(clave) {
            return Resultado::Inestable;
        }
        match self.resultados.tests.get(clave) {
            Some(ResultadoTest::Pasa) => Resultado::Pasa,
            Some(ResultadoTest::Falla) => Resultado::Falla,
            Some(ResultadoTest::Omite) => Resultado::Omite,
            None => Resultado::Ausente,
        }
    }
}

/// Un escenario pasa si el mapa lo dice `true`. Ausente cuenta como no verde.
fn medir_escenario(id: &str, escenarios: &BTreeMap<String, bool>) -> Resultado {
    match escenarios.get(id) {
        Some(true) => Resultado::Pasa,
        Some(false) => Resultado::Falla,
        None => Resultado::Ausente,
    }
}

fn evaluar_uno(
    req: &Requisito,
    medidor: &Medidor,
    escenarios: &BTreeMap<String, bool>,
    no_evaluables: &BTreeMap<String, String>,
) -> EstadoRequisito {
    let estado =
        |cumplimiento: Cumplimiento, nivel: Option<u8>, evidencia: Vec<String>| EstadoRequisito {
            id: req.id.clone(),
            titulo: req.titulo.clone(),
            peso: req.peso,
            cumplimiento,
            evidencia,
            nivel,
        };
    if let Some(cita) = no_evaluables.get(&req.id) {
        return estado(Cumplimiento::SinVeredicto, None, vec![cita.clone()]);
    }
    if req.pruebas.is_empty() {
        return estado(
            Cumplimiento::SinVeredicto,
            None,
            vec!["sin pruebas: el requisito no tiene tests mapeados".into()],
        );
    }
    let pruebas = medidor.referencias(&req.pruebas);
    let bordes = medidor.referencias(&req.bordes);
    let escenarios_medidos: Vec<Medida> = req
        .escenarios
        .iter()
        .map(|id| Medida {
            id: id.clone(),
            resultado: medir_escenario(id, escenarios),
        })
        .collect();
    let todas: Vec<&Medida> = pruebas
        .iter()
        .chain(&bordes)
        .chain(&escenarios_medidos)
        .collect();

    let mut evidencia: Vec<String> = todas
        .iter()
        .filter(|m| !m.excluida())
        .map(|m| m.etiqueta())
        .collect();
    evidencia.extend(todas.iter().filter_map(|m| m.texto_excluido()));

    let quedan_pruebas: Vec<&Medida> = pruebas.iter().filter(|m| !m.excluida()).collect();
    let quedan_resto: Vec<&Medida> = bordes
        .iter()
        .chain(&escenarios_medidos)
        .filter(|m| !m.excluida())
        .collect();
    // Sin pruebas medibles no hay caso feliz: aunque bordes o escenarios estén verdes, no hay nivel.
    if quedan_pruebas.is_empty() {
        let con_entorno = pruebas
            .iter()
            .any(|m| matches!(m.resultado, Resultado::Excluido { entorno: true, .. }));
        let cumplimiento = if con_entorno {
            Cumplimiento::Pendiente
        } else {
            Cumplimiento::SinVeredicto
        };
        return estado(cumplimiento, None, evidencia);
    }

    let verdes = quedan_pruebas.iter().filter(|m| m.verde()).count();
    let nivel: u8 = if verdes == 0 {
        0
    } else if verdes < quedan_pruebas.len() {
        1
    } else if quedan_resto.iter().all(|m| m.verde()) {
        3
    } else {
        2
    };
    let cumplimiento = match nivel {
        3 => Cumplimiento::Cumplido,
        0 => Cumplimiento::Ausente,
        _ => Cumplimiento::Parcial,
    };
    estado(cumplimiento, Some(nivel), evidencia)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oracle::clasificar;
    use crate::oracle::junit::Junit;

    fn strs(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn req(
        id: &str,
        peso: u8,
        pruebas: &[&str],
        bordes: &[&str],
        escenarios: &[&str],
    ) -> Requisito {
        Requisito {
            id: id.into(),
            titulo: format!("Título de {id}"),
            seccion: String::new(),
            peso,
            pruebas: strs(pruebas),
            bordes: strs(bordes),
            escenarios: strs(escenarios),
            criterios: Vec::new(),
            modulos: Vec::new(),
        }
    }

    fn resultados(pasa: &[&str], falla: &[&str], inestables: &[&str]) -> ResultadosTests {
        let mut tests = BTreeMap::new();
        for t in pasa {
            tests.insert(t.to_string(), ResultadoTest::Pasa);
        }
        for t in falla {
            tests.insert(t.to_string(), ResultadoTest::Falla);
        }
        ResultadosTests {
            tests,
            inestables: inestables.iter().map(|t| t.to_string()).collect(),
        }
    }

    fn excl(test: &str, motivo: &str) -> Exclusion {
        Exclusion {
            test: test.into(),
            motivo: motivo.into(),
        }
    }

    fn escenarios(pares: &[(&str, bool)]) -> BTreeMap<String, bool> {
        pares.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    fn sin_cita() -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    /// Evalúa sin escenarios, sin exclusiones y sin no evaluables.
    fn evalua(reqs: &[Requisito], r: &ResultadosTests) -> Vec<EstadoRequisito> {
        evaluar_requisitos(reqs, r, &BTreeMap::new(), &[], &[], &sin_cita())
    }

    fn ecc() -> Vec<Requisito> {
        vec![
            req("R1", 3, &["a::t1", "a::t2"], &[], &[]),
            req("R2", 2, &["b::t1"], &["b::t2"], &[]),
            req("R3", 1, &["c::t1"], &[], &["e1"]),
        ]
    }

    #[test]
    fn requisitos_ecc_todo_verde_da_cumplidos_y_completitud_uno() {
        let r = resultados(&["a::t1", "a::t2", "b::t1", "b::t2", "c::t1"], &[], &[]);
        let estados = evaluar_requisitos(
            &ecc(),
            &r,
            &escenarios(&[("e1", true)]),
            &[],
            &[],
            &sin_cita(),
        );
        let ids: Vec<&str> = estados.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, ["R1", "R2", "R3"]);
        assert!(
            estados
                .iter()
                .all(|e| e.cumplimiento == Cumplimiento::Cumplido)
        );
        let niveles: Vec<Option<u8>> = estados.iter().map(|e| e.nivel).collect();
        assert_eq!(niveles, [Some(3), Some(3), Some(3)]);
        assert_eq!(completitud(&estados), Some(1.0));
    }

    #[test]
    fn requisitos_sdlc_modulos_ausentes_y_parcial_bajan_la_completitud() {
        // R1 y R2 no existen en la suite; R3 pasa solo la mitad de sus pruebas.
        let reqs = vec![
            req("R1", 3, &["a::t1", "a::t2"], &[], &[]),
            req("R2", 2, &["b::t1"], &["b::t2"], &[]),
            req("R3", 1, &["c::t1", "c::t2"], &[], &["e1"]),
        ];
        let r = resultados(&["c::t1"], &[], &[]);
        let estados = evalua(&reqs, &r);
        assert_eq!(estados[0].nivel, Some(0));
        assert_eq!(estados[0].cumplimiento, Cumplimiento::Ausente);
        assert_eq!(estados[1].nivel, Some(0));
        assert_eq!(estados[1].cumplimiento, Cumplimiento::Ausente);
        assert_eq!(estados[2].nivel, Some(1));
        assert_eq!(estados[2].cumplimiento, Cumplimiento::Parcial);
        // (3·0 + 2·0 + 1·1) / (3·(3+2+1)) = 1/18
        let valor = completitud(&estados).expect("hay niveles");
        assert!((valor - 1.0 / 18.0).abs() < 1e-6, "valor {valor}");
        let ecc_valor = completitud(&evaluar_requisitos(
            &ecc(),
            &resultados(&["a::t1", "a::t2", "b::t1", "b::t2", "c::t1"], &[], &[]),
            &escenarios(&[("e1", true)]),
            &[],
            &[],
            &sin_cita(),
        ));
        assert!(Some(valor) < ecc_valor);
    }

    #[test]
    fn requisitos_borde_que_falla_deja_nivel_2() {
        let reqs = [req("R1", 1, &["a::t1"], &["a::t2"], &[])];
        let r = resultados(&["a::t1"], &["a::t2"], &[]);
        let estados = evalua(&reqs, &r);
        assert_eq!(estados[0].nivel, Some(2));
        assert_eq!(estados[0].cumplimiento, Cumplimiento::Parcial);
        assert!(estados[0].evidencia.contains(&"a::t2: falla".to_string()));
    }

    #[test]
    fn requisitos_escenario_falso_o_ausente_deja_nivel_2() {
        let reqs = [req("R1", 1, &["a::t1"], &[], &["e1"])];
        let r = resultados(&["a::t1"], &[], &[]);
        let falso = evaluar_requisitos(
            &reqs,
            &r,
            &escenarios(&[("e1", false)]),
            &[],
            &[],
            &sin_cita(),
        );
        assert_eq!(falso[0].nivel, Some(2));
        assert_eq!(falso[0].cumplimiento, Cumplimiento::Parcial);
        assert!(falso[0].evidencia.contains(&"e1: falla".to_string()));

        let ausente = evalua(&reqs, &r);
        assert_eq!(ausente[0].nivel, Some(2));
        assert!(ausente[0].evidencia.contains(&"e1: ausente".to_string()));
    }

    #[test]
    fn requisitos_escenario_verde_con_todo_lo_demas_da_nivel_3() {
        let reqs = [req("R1", 1, &["a::t1"], &["a::t2"], &["e1"])];
        let r = resultados(&["a::t1", "a::t2"], &[], &[]);
        let estados = evaluar_requisitos(
            &reqs,
            &r,
            &escenarios(&[("e1", true)]),
            &[],
            &[],
            &sin_cita(),
        );
        assert_eq!(estados[0].nivel, Some(3));
        assert_eq!(estados[0].cumplimiento, Cumplimiento::Cumplido);
    }

    #[test]
    fn requisitos_inestable_en_borde_no_pasa_de_nivel_2_y_lista_hallazgo() {
        let reqs = [
            req("R1", 2, &["a::t1"], &["a::t2"], &[]),
            req("R2", 1, &["a"], &[], &[]),
        ];
        let r = resultados(&["a::t1"], &[], &["a::t2"]);
        let estados = evalua(&reqs, &r);
        assert_eq!(estados[0].nivel, Some(2));
        assert_eq!(estados[0].cumplimiento, Cumplimiento::Parcial);
        assert!(
            estados[0]
                .evidencia
                .contains(&"a::t2: inestable".to_string())
        );

        let hallazgos = hallazgos_inestables(&reqs, &r);
        assert_eq!(hallazgos.len(), 1);
        let h = &hallazgos[0];
        assert_eq!(h.cita, "a::t2");
        assert_eq!(h.dimension, Dimension::Completitud);
        assert_eq!(h.severidad, 2);
        assert_eq!((h.archivo.as_str(), h.linea), ("", 0));
        assert_eq!(
            h.detalle,
            "comportamiento no determinista: este test pasa y falla sin cambios en el código"
        );
        assert_eq!(h.fuente, Fuente::Sonda("completitud".into()));
        assert_eq!(h.impacto, ["R1", "R2"]);
    }

    #[test]
    fn requisitos_inestable_en_pruebas_no_pasa_de_nivel_1() {
        let reqs = [req("R1", 1, &["a::t1", "a::t3"], &[], &[])];
        let r = resultados(&["a::t3"], &[], &["a::t1"]);
        let estados = evalua(&reqs, &r);
        assert_eq!(estados[0].nivel, Some(1));
        assert_eq!(estados[0].cumplimiento, Cumplimiento::Parcial);
    }

    #[test]
    fn requisitos_pendiente_por_entorno_no_cambia_la_completitud() {
        let a = req("A", 1, &["a::t1", "a::t2"], &[], &[]);
        let b = req("B", 2, &["b::t1"], &[], &[]);
        // b::t1 no está en `tests`: la clasificación lo excluye.
        let r = resultados(&["a::t1"], &["a::t2"], &[]);
        let entorno = [excl("b::t1", "lavapipe contra GPU")];
        let juntos = evaluar_requisitos(
            &[a.clone(), b],
            &r,
            &BTreeMap::new(),
            &entorno,
            &[],
            &sin_cita(),
        );
        assert_eq!(juntos[1].cumplimiento, Cumplimiento::Pendiente);
        assert_eq!(juntos[1].nivel, None);
        assert!(
            juntos[1]
                .evidencia
                .contains(&"pendiente: b::t1 (lavapipe contra GPU)".to_string())
        );
        let sola = evalua(&[a], &r);
        assert_eq!(completitud(&juntos), completitud(&sola));
        assert_eq!(completitud(&sola), Some(1.0 / 3.0));
    }

    #[test]
    fn requisitos_oraculo_dudoso_solo_da_sin_veredicto() {
        let reqs = [req("R", 1, &["a::t1"], &[], &[])];
        let r = resultados(&[], &[], &[]);
        let dudoso = [excl("a::t1", "orden de spin")];
        let estados = evaluar_requisitos(&reqs, &r, &BTreeMap::new(), &[], &dudoso, &sin_cita());
        assert_eq!(estados[0].cumplimiento, Cumplimiento::SinVeredicto);
        assert_eq!(estados[0].nivel, None);
        assert!(
            estados[0]
                .evidencia
                .contains(&"excluido: a::t1 (orden de spin)".to_string())
        );
    }

    #[test]
    fn requisitos_pruebas_excluidas_por_entorno_con_bordes_verdes_dan_pendiente() {
        let a = req("A", 1, &["a::t1"], &[], &[]);
        let b = req("B", 2, &["b::t1"], &["b::t2"], &[]);
        let r = resultados(&["a::t1", "b::t2"], &[], &[]);
        let entorno = [excl("b::t1", "lavapipe contra GPU")];
        let juntos = evaluar_requisitos(
            &[a.clone(), b],
            &r,
            &BTreeMap::new(),
            &entorno,
            &[],
            &sin_cita(),
        );
        assert_eq!(juntos[1].cumplimiento, Cumplimiento::Pendiente);
        assert_eq!(juntos[1].nivel, None);
        assert!(juntos[1].evidencia.contains(&"b::t2: pasa".to_string()));
        assert!(
            juntos[1]
                .evidencia
                .contains(&"pendiente: b::t1 (lavapipe contra GPU)".to_string())
        );
        assert_eq!(completitud(&juntos), completitud(&evalua(&[a], &r)));
    }

    #[test]
    fn requisitos_oraculo_dudoso_en_todas_las_pruebas_da_sin_veredicto_con_bordes_verdes() {
        let reqs = [req("R", 1, &["a::t1"], &["a::t2"], &[])];
        let r = resultados(&["a::t2"], &[], &[]);
        let dudoso = [excl("a::t1", "orden de spin")];
        let estados = evaluar_requisitos(&reqs, &r, &BTreeMap::new(), &[], &dudoso, &sin_cita());
        assert_eq!(estados[0].cumplimiento, Cumplimiento::SinVeredicto);
        assert_eq!(estados[0].nivel, None);
        assert!(estados[0].evidencia.contains(&"a::t2: pasa".to_string()));
        assert!(
            estados[0]
                .evidencia
                .contains(&"excluido: a::t1 (orden de spin)".to_string())
        );
    }

    #[test]
    fn requisitos_entorno_mezclado_con_test_verde_lo_decide_el_verde() {
        let reqs = [req("R", 1, &["a::t1", "a::t2"], &[], &[])];
        let r = resultados(&["a::t2"], &[], &[]);
        let entorno = [excl("a::t1", "lavapipe")];
        let estados = evaluar_requisitos(&reqs, &r, &BTreeMap::new(), &entorno, &[], &sin_cita());
        assert_eq!(estados[0].nivel, Some(3));
        assert_eq!(estados[0].cumplimiento, Cumplimiento::Cumplido);
        assert!(estados[0].evidencia.contains(&"a::t2: pasa".to_string()));
        assert!(
            estados[0]
                .evidencia
                .contains(&"pendiente: a::t1 (lavapipe)".to_string())
        );
    }

    #[test]
    fn requisitos_sin_pruebas_da_sin_veredicto() {
        let reqs = [req("R", 1, &[], &["a::t1"], &[])];
        let r = resultados(&["a::t1"], &[], &[]);
        let estados = evalua(&reqs, &r);
        assert_eq!(estados[0].cumplimiento, Cumplimiento::SinVeredicto);
        assert_eq!(estados[0].nivel, None);
    }

    #[test]
    fn requisitos_no_evaluable_da_sin_veredicto_con_la_cita() {
        let reqs = [req("R1", 1, &["a::t1"], &[], &[])];
        let r = resultados(&["a::t1"], &[], &[]);
        let no_eval = BTreeMap::from([(
            "R1".to_string(),
            "ficha: «el usuario ve un aviso» es ambiguo".to_string(),
        )]);
        let estados = evaluar_requisitos(&reqs, &r, &BTreeMap::new(), &[], &[], &no_eval);
        assert_eq!(estados[0].cumplimiento, Cumplimiento::SinVeredicto);
        assert_eq!(estados[0].nivel, None);
        assert_eq!(
            estados[0].evidencia,
            ["ficha: «el usuario ve un aviso» es ambiguo"]
        );
    }

    #[test]
    fn requisitos_referencia_a_archivo_expande_a_sus_tests() {
        let reqs = [req("R", 1, &["test_m1_protocolo"], &[], &[])];
        let r = resultados(
            &[
                "test_m1_protocolo::t1",
                "test_m1_protocolo::t2",
                "test_m10_otro::t1",
            ],
            &["test_m1_protocolo::t3"],
            &[],
        );
        let estados = evalua(&reqs, &r);
        assert_eq!(estados[0].nivel, Some(1));
        assert_eq!(estados[0].cumplimiento, Cumplimiento::Parcial);
        assert_eq!(
            estados[0].evidencia,
            [
                "test_m1_protocolo::t1: pasa",
                "test_m1_protocolo::t2: pasa",
                "test_m1_protocolo::t3: falla",
            ]
        );
    }

    #[test]
    fn requisitos_referencia_a_archivo_no_coincide_por_prefijo_parcial() {
        let reqs = [req("R", 1, &["test_m1"], &[], &[])];
        let r = resultados(&["test_m1_protocolo::t1"], &[], &[]);
        let estados = evalua(&reqs, &r);
        assert_eq!(estados[0].nivel, Some(0));
        assert_eq!(estados[0].cumplimiento, Cumplimiento::Ausente);
        assert!(
            estados[0]
                .evidencia
                .contains(&"test_m1: ausente".to_string())
        );
    }

    #[test]
    fn requisitos_referencia_que_no_resuelve_cuenta_como_ausente() {
        let reqs = [req("R", 1, &["a::no_existe", "a::t1"], &[], &[])];
        let r = resultados(&["a::t1"], &[], &[]);
        let estados = evalua(&reqs, &r);
        assert_eq!(estados[0].nivel, Some(1));
        assert_eq!(estados[0].cumplimiento, Cumplimiento::Parcial);
        assert!(
            estados[0]
                .evidencia
                .contains(&"a::no_existe: ausente".to_string())
        );
    }

    #[test]
    fn requisitos_completitud_sin_ningun_nivel_es_none() {
        assert_eq!(completitud(&[]), None);
        let sin_nivel = EstadoRequisito {
            id: "R1".into(),
            titulo: "x".into(),
            peso: 2,
            cumplimiento: Cumplimiento::Pendiente,
            evidencia: Vec::new(),
            nivel: None,
        };
        assert_eq!(completitud(&[sin_nivel]), None);
    }

    #[test]
    fn requisitos_puntaje_completitud_lista_un_item_por_nivel() {
        let estado = |id: &str, peso: u8, nivel: Option<u8>| EstadoRequisito {
            id: id.into(),
            titulo: id.into(),
            peso,
            cumplimiento: Cumplimiento::Parcial,
            evidencia: vec![format!("{id}: pasa")],
            nivel,
        };
        let estados = [
            estado("A", 1, Some(3)),
            estado("B", 2, Some(0)),
            estado("C", 3, None),
        ];
        let p = puntaje_completitud(&estados).expect("hay puntaje");
        assert_eq!(p.dimension, Dimension::Completitud);
        assert!(!p.calibrada);
        assert_eq!(p.exactitud_balanceada, None);
        // (1·3 + 2·0) / (3·(1+2)) = 1/3
        assert!((p.valor - 1.0 / 3.0).abs() < 1e-6);
        assert_eq!(p.items.len(), 2);
        assert_eq!(p.items[0].item, "completitud.requisito.A");
        assert_eq!(p.items[0].nivel, 3);
        assert_eq!(p.items[0].evidencia, ["A: pasa"]);
        assert_eq!(p.items[0].fuente, Fuente::Sonda("completitud".into()));
        assert_eq!(p.items[1].item, "completitud.requisito.B");
        assert_eq!(p.items[1].nivel, 0);
        assert_eq!(puntaje_completitud(&[estado("D", 1, None)]), None);
    }

    #[test]
    fn requisitos_desde_clasificacion_separa_estables_e_inestables() {
        let mut j1 = Junit::default();
        j1.tests.insert("a::t_est".into(), ResultadoTest::Pasa);
        j1.tests.insert("a::t_mix".into(), ResultadoTest::Pasa);
        let mut j2 = Junit::default();
        j2.tests.insert("a::t_est".into(), ResultadoTest::Pasa);
        j2.tests.insert("a::t_mix".into(), ResultadoTest::Falla);
        let c = clasificar(&[j1, j2], &[], &BTreeSet::new());
        let r = desde_clasificacion(&c);
        assert_eq!(
            r.tests,
            BTreeMap::from([("a::t_est".to_string(), ResultadoTest::Pasa)])
        );
        assert_eq!(r.inestables, BTreeSet::from(["a::t_mix".to_string()]));
    }

    #[test]
    fn requisitos_test_omitido_no_deja_cumplido() {
        // Un test que pasa y otro que se omite: el omitido cuenta como no verde (A10).
        let mut j1 = Junit::default();
        j1.tests.insert("a::t1".into(), ResultadoTest::Pasa);
        j1.tests.insert("a::t2".into(), ResultadoTest::Omite);
        let c = clasificar(&[j1.clone(), j1], &[], &BTreeSet::new());
        let r = desde_clasificacion(&c);
        let reqs = vec![req("R1", 1, &["a::t1", "a::t2"], &[], &[])];
        let estados = evalua(&reqs, &r);
        assert_ne!(
            estados[0].cumplimiento,
            Cumplimiento::Cumplido,
            "{estados:?}"
        );
        assert!(estados[0].nivel.is_some_and(|n| n <= 2), "{estados:?}");
    }

    #[test]
    fn requisitos_test_omitido_por_id_exacto_no_queda_ausente() {
        let mut j1 = Junit::default();
        j1.tests.insert("a::t2".into(), ResultadoTest::Omite);
        let c = clasificar(&[j1.clone(), j1], &[], &BTreeSet::new());
        let r = desde_clasificacion(&c);
        let reqs = vec![req("R1", 1, &["a::t2"], &[], &[])];
        let estados = evalua(&reqs, &r);
        assert_eq!(
            estados[0].evidencia,
            vec!["a::t2: omite".to_string()],
            "{estados:?}"
        );
    }
}
