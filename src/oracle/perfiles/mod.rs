//! Perfiles de sondas por lenguaje del proyecto (Tarea 9). Cada sonda da un `ResultadoSonda`.

pub mod herramientas;
pub mod rust;

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::goal::calcular_hash;
use crate::oracle::tarea::{Requisito, Tarea};
use crate::tipos::{Dimension, Fuente, Hallazgo, Puntaje};

/// Resultado de una sonda. `valor: None` quiere decir sin veredicto (fail-closed).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultadoSonda {
    pub dimension: Dimension,
    pub valor: Option<f32>,
    /// Por qué no hay veredicto.
    pub motivo: Option<String>,
    pub hallazgos: Vec<Hallazgo>,
}

impl ResultadoSonda {
    /// Puntaje de la dimensión. `None` si la sonda no tiene veredicto.
    pub fn a_puntaje(&self) -> Option<Puntaje> {
        self.valor.map(|valor| Puntaje {
            dimension: self.dimension,
            valor,
            calibrada: false,
            exactitud_balanceada: None,
            items: Vec::new(),
            sin_veredicto: None,
            faltantes: Vec::new(),
        })
    }
}

/// Módulo del código al que pertenece `archivo` (ruta relativa al repo).
/// `src/x/...` da `x`; `src/x.rs` da `x`; `src/main.rs` y `src/lib.rs` no tienen módulo.
pub fn modulo_de(archivo: &str) -> Option<String> {
    let resto = archivo.strip_prefix("src/")?;
    if let Some((dir, _)) = resto.split_once('/') {
        return Some(dir.to_string());
    }
    let nombre = resto.strip_suffix(".rs")?;
    (!matches!(nombre, "main" | "lib")).then(|| nombre.to_string())
}

/// Crea un hallazgo de sonda. `id` sale de dimensión, archivo y cita; `impacto` del módulo.
#[allow(clippy::too_many_arguments)]
pub fn crear_hallazgo(
    dimension: Dimension,
    severidad: u8,
    archivo: &str,
    linea: u32,
    cita: &str,
    detalle: &str,
    sonda: &str,
    requisitos: &[Requisito],
) -> Hallazgo {
    let modulo = modulo_de(archivo);
    let impacto = requisitos
        .iter()
        .filter(|r| modulo.as_ref().is_some_and(|m| r.modulos.contains(m)))
        .map(|r| r.id.clone())
        .collect();
    let id = calcular_hash(format!("{dimension:?}|{archivo}|{cita}").as_bytes());
    Hallazgo {
        dimension,
        severidad,
        archivo: archivo.into(),
        linea,
        cita: cita.into(),
        detalle: detalle.into(),
        fuente: Fuente::Sonda(sonda.into()),
        id,
        impacto,
    }
}

/// Puntaje de los hallazgos: `valor = max(0, 1 - Σ penal / 3)`.
/// `penal = (severidad/3) * (peso/3)`, con el mayor `peso` de los requisitos afectados.
/// Un hallazgo sin impacto y de severidad 1 solo se lista: no baja el puntaje.
/// Nunca se cuenta por cantidad bruta de avisos.
pub fn puntuar_hallazgos(hs: &[Hallazgo], reqs: &[Requisito]) -> f32 {
    let total: f32 = hs.iter().map(|h| penal(h, reqs)).sum();
    (1.0 - total / 3.0).max(0.0)
}

/// Descarta los repetidos con el mismo `archivo`, `linea` y `cita`. Conserva el primero y el orden.
/// Regla: la sonda dueña es la que nombra la tabla de rúbricas. Esto es solo una red de seguridad.
pub fn deduplicar_hallazgos(hs: Vec<Hallazgo>) -> Vec<Hallazgo> {
    let mut vistos = std::collections::HashSet::new();
    hs.into_iter()
        .filter(|h| vistos.insert((h.archivo.clone(), h.linea, h.cita.clone())))
        .collect()
}

fn penal(h: &Hallazgo, reqs: &[Requisito]) -> f32 {
    if h.impacto.is_empty() && h.severidad < 2 {
        return 0.0;
    }
    let peso = reqs
        .iter()
        .filter(|r| h.impacto.contains(&r.id))
        .map(|r| r.peso)
        .max()
        .unwrap_or(1);
    (f32::from(h.severidad) / 3.0) * (f32::from(peso) / 3.0)
}

/// Junta las partes de una dimensión. Si una parte no tiene veredicto, el resultado tampoco.
/// Sin partes no hay nada que combinar: `None` (B13), nunca un 1,0 sin prueba.
pub fn combinar(
    dimension: Dimension,
    partes: Vec<ResultadoSonda>,
    reqs: &[Requisito],
) -> Option<ResultadoSonda> {
    if partes.is_empty() {
        return None;
    }
    let incompleta = partes.iter().any(|p| p.valor.is_none());
    let motivos: Vec<String> = partes
        .iter()
        .filter(|p| p.valor.is_none())
        .filter_map(|p| p.motivo.clone())
        .collect();
    let hallazgos: Vec<Hallazgo> = partes.into_iter().flat_map(|p| p.hallazgos).collect();
    let valor = (!incompleta).then(|| puntuar_hallazgos(&hallazgos, reqs));
    let motivo = (incompleta && !motivos.is_empty()).then(|| motivos.join("; "));
    Some(ResultadoSonda {
        dimension,
        valor,
        motivo,
        hallazgos,
    })
}

/// Sondas del perfil del lenguaje de la tarea. Hoy solo existe el perfil `rust`.
/// Python entra después (Tarea 9): ahí va su rama, sin tocar la de Rust.
pub fn sondas_nativas(repo: &Path, tarea: &Tarea) -> Vec<ResultadoSonda> {
    if tarea.lenguaje != "rust" {
        return Vec::new();
    }
    let capas: &[Vec<String>] = tarea
        .arquitectura
        .as_ref()
        .map_or(&[], |a| a.capas.as_slice());
    vec![
        rust::seguridad_nativa(repo, &tarea.requisito),
        rust::estructura_nativa(repo, &tarea.requisito, &rust::LimitesEstructura::default()),
        rust::arquitectura_nativa(repo, capas, &tarea.requisito),
    ]
}

/// Ítems de rúbrica que solo tienen sonda en rust (Tarea 31). Para otro lenguaje, cada uno es un faltante.
/// `seguridad.secretos` no está: se mide en todo lenguaje, con gitleaks y las sondas de configs.
pub fn items_sin_sonda(lenguaje: &str) -> &'static [&'static str] {
    if lenguaje == "rust" {
        return &[];
    }
    &[
        "seguridad.dependencias",
        "seguridad.panico_en_entrada",
        "seguridad.unsafe",
        "estructura.tamano",
        "estructura.duplicacion",
        "estructura.clippy",
        "arquitectura.dependencias",
        "eficiencia.tiempo",
        "eficiencia.capacidad",
        "eficiencia.escalado",
    ]
}

#[cfg(test)]
pub(crate) fn requisito_de(id: &str, peso: u8, modulos: &[&str]) -> Requisito {
    Requisito {
        id: id.into(),
        titulo: id.into(),
        seccion: String::new(),
        peso,
        pruebas: Vec::new(),
        bordes: Vec::new(),
        escenarios: Vec::new(),
        criterios: Vec::new(),
        modulos: modulos.iter().map(|m| m.to_string()).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oracle::tarea::Tarea;
    use crate::tipos::{Dimension, Fuente};
    use std::fs;

    #[test]
    fn perfiles_deduplicar_quita_repetidos_y_conserva_el_primero_en_orden() {
        let a = hallazgo_de(3, "Dockerfile", "FROM x", &[]);
        let mut b = hallazgo_de(2, "Dockerfile", "FROM x", &[]);
        b.dimension = Dimension::Entrega;
        let c = hallazgo_de(1, "a.rs", "otra", &[]);
        let salida = deduplicar_hallazgos(vec![a.clone(), b, c.clone()]);
        assert_eq!(salida, vec![a, c]);
    }

    #[test]
    fn perfiles_items_sin_sonda_rust_no_tiene_ninguno() {
        assert!(items_sin_sonda("rust").is_empty());
    }

    #[test]
    fn perfiles_items_sin_sonda_otro_lenguaje_lista_los_de_rust_sin_secretos() {
        let faltan = items_sin_sonda("shell");
        assert_eq!(
            faltan,
            [
                "seguridad.dependencias",
                "seguridad.panico_en_entrada",
                "seguridad.unsafe",
                "estructura.tamano",
                "estructura.duplicacion",
                "estructura.clippy",
                "arquitectura.dependencias",
                "eficiencia.tiempo",
                "eficiencia.capacidad",
                "eficiencia.escalado",
            ]
        );
        assert!(!faltan.contains(&"seguridad.secretos"));
    }

    #[test]
    fn perfiles_items_sin_sonda_coinciden_con_las_sondas_de_rust_de_las_rubricas() {
        use crate::oracle::rubrica::{Mide, cargar_embebidas};
        let rubricas = cargar_embebidas().expect("rúbricas");
        let dimensiones = ["seguridad", "estructura", "arquitectura", "eficiencia"];
        let mut de_rust: Vec<String> = rubricas
            .iter()
            .filter(|r| dimensiones.contains(&r.id.as_str()))
            .flat_map(|r| r.item.iter())
            .filter(|i| i.mide == Mide::Sonda && i.id != "seguridad.secretos")
            .map(|i| i.id.clone())
            .collect();
        de_rust.sort();
        let mut lista: Vec<String> = items_sin_sonda("shell")
            .iter()
            .map(|s| s.to_string())
            .collect();
        lista.sort();
        assert_eq!(lista, de_rust);
    }

    fn tarea_en(dir: &std::path::Path, lenguaje: &str, extra: &str) -> Tarea {
        let texto = format!(
            "nombre = \"x\"\nlenguaje = \"{lenguaje}\"\nimagen = \"i\"\nspec = \"s\"\n\
             [contrato]\ncmd = \"c\"\n[oculto]\ncmd = \"o\"\n{extra}"
        );
        fs::write(dir.join("tarea.toml"), texto).expect("escribe tarea.toml");
        Tarea::cargar_de(dir).expect("carga la tarea")
    }

    fn hallazgo_de(sev: u8, archivo: &str, cita: &str, reqs: &[Requisito]) -> Hallazgo {
        crear_hallazgo(
            Dimension::Seguridad,
            sev,
            archivo,
            1,
            cita,
            "d",
            "prueba",
            reqs,
        )
    }

    #[test]
    fn sonda_crear_hallazgo_id_es_estable_y_depende_de_la_cita() {
        let a = hallazgo_de(2, "src/x.rs", "unsafe {", &[]);
        let b = hallazgo_de(1, "src/x.rs", "unsafe {", &[]);
        let c = hallazgo_de(2, "src/x.rs", "otra cita", &[]);
        assert_eq!(a.id, b.id, "la severidad no entra al id");
        assert_ne!(a.id, c.id);
        assert_eq!(a.id.len(), 16);
    }

    #[test]
    fn sonda_crear_hallazgo_impacto_sale_del_modulo_del_archivo() {
        let reqs = [
            requisito_de("R1", 3, &["protocolo"]),
            requisito_de("R2", 1, &["almacen"]),
        ];
        let en_dir = hallazgo_de(2, "src/protocolo/mod.rs", "x", &reqs);
        let en_archivo = hallazgo_de(2, "src/protocolo.rs", "x", &reqs);
        let en_raiz = hallazgo_de(2, "src/main.rs", "x", &reqs);
        assert_eq!(en_dir.impacto, ["R1"]);
        assert_eq!(en_archivo.impacto, ["R1"]);
        assert!(en_raiz.impacto.is_empty());
    }

    #[test]
    fn sonda_crear_hallazgo_fuente_es_sonda() {
        let h = hallazgo_de(1, "src/x.rs", "x", &[]);
        assert_eq!(h.fuente, Fuente::Sonda("prueba".into()));
        assert_eq!(h.dimension, Dimension::Seguridad);
    }

    #[test]
    fn sonda_puntuar_sin_hallazgos_da_uno() {
        assert_eq!(puntuar_hallazgos(&[], &[]), 1.0);
    }

    #[test]
    fn sonda_puntuar_avisos_sin_impacto_de_severidad_1_no_bajan() {
        let hs: Vec<Hallazgo> = (0..200)
            .map(|i| hallazgo_de(1, "src/x.rs", &format!("linea {i}"), &[]))
            .collect();
        assert_eq!(puntuar_hallazgos(&hs, &[]), 1.0);
    }

    #[test]
    fn sonda_puntuar_tres_graves_de_peso_maximo_dan_cero() {
        let reqs = [requisito_de("R1", 3, &["protocolo"])];
        let hs: Vec<Hallazgo> = (0..3)
            .map(|i| hallazgo_de(3, "src/protocolo/mod.rs", &format!("c{i}"), &reqs))
            .collect();
        assert_eq!(puntuar_hallazgos(&hs, &reqs), 0.0);
    }

    #[test]
    fn sonda_combinar_con_una_parte_sin_veredicto_no_da_puntaje() {
        let con_hallazgo = ResultadoSonda {
            dimension: Dimension::Seguridad,
            valor: Some(1.0),
            motivo: None,
            hallazgos: vec![hallazgo_de(2, "src/x.rs", "unsafe", &[])],
        };
        let sin_veredicto = ResultadoSonda {
            dimension: Dimension::Seguridad,
            valor: None,
            motivo: Some("falta cargo audit".into()),
            hallazgos: Vec::new(),
        };
        let r = combinar(Dimension::Seguridad, vec![con_hallazgo, sin_veredicto], &[])
            .expect("hay partes");
        assert_eq!(r.valor, None);
        assert_eq!(r.motivo.as_deref(), Some("falta cargo audit"));
        assert_eq!(r.hallazgos.len(), 1, "conserva los hallazgos");
    }

    #[test]
    fn sonda_combinar_sin_partes_no_da_puntaje() {
        assert!(combinar(Dimension::Seguridad, Vec::new(), &[]).is_none());
    }

    #[test]
    fn sonda_combinar_con_todas_con_veredicto_recalcula() {
        let parte = |sev| ResultadoSonda {
            dimension: Dimension::Seguridad,
            valor: Some(1.0),
            motivo: None,
            hallazgos: vec![hallazgo_de(sev, "src/x.rs", &format!("c{sev}"), &[])],
        };
        let r = combinar(Dimension::Seguridad, vec![parte(2), parte(2)], &[]).expect("hay partes");
        assert_eq!(r.motivo, None);
        assert_eq!(r.hallazgos.len(), 2);
        let esperado = puntuar_hallazgos(&r.hallazgos, &[]);
        assert_eq!(r.valor, Some(esperado));
    }

    #[test]
    fn sonda_resultado_con_valor_da_puntaje_y_sin_valor_no() {
        let con = ResultadoSonda {
            dimension: Dimension::Estructura,
            valor: Some(0.5),
            motivo: None,
            hallazgos: Vec::new(),
        };
        let p = con.a_puntaje().expect("hay puntaje");
        assert_eq!((p.valor, p.calibrada), (0.5, false));
        assert_eq!(p.dimension, Dimension::Estructura);
        let sin = ResultadoSonda { valor: None, ..con };
        assert!(sin.a_puntaje().is_none());
    }

    #[test]
    fn sonda_lenguaje_sin_perfil_da_vector_vacio() {
        let dir = tempfile::tempdir().expect("temp dir");
        let tarea = tarea_en(dir.path(), "python", "");
        assert!(sondas_nativas(dir.path(), &tarea).is_empty());
    }

    #[test]
    fn sonda_rust_da_seguridad_estructura_y_arquitectura() {
        let dir = tempfile::tempdir().expect("temp dir");
        let tarea = tarea_en(
            dir.path(),
            "rust",
            "\n[arquitectura]\ncapas = [[\"protocolo\"]]\n",
        );
        let dims: Vec<Dimension> = sondas_nativas(dir.path(), &tarea)
            .iter()
            .map(|r| r.dimension)
            .collect();
        assert_eq!(
            dims,
            [
                Dimension::Seguridad,
                Dimension::Estructura,
                Dimension::Arquitectura
            ]
        );
    }

    #[test]
    fn sonda_rust_con_capa_prometida_y_solo_main_da_hallazgo_con_cita_del_mapa() {
        let dir = tempfile::tempdir().expect("temp dir");
        fs::create_dir_all(dir.path().join("src")).expect("src");
        fs::write(dir.path().join("src/main.rs"), "fn main() {}\n").expect("main.rs");
        let tarea = tarea_en(
            dir.path(),
            "rust",
            "\n[arquitectura]\ncapas = [[\"protocolo\"]]\n",
        );
        let r = sondas_nativas(dir.path(), &tarea)
            .into_iter()
            .find(|r| r.dimension == Dimension::Arquitectura)
            .expect("hay sonda de arquitectura");
        assert!(r.valor.is_some(), "{:?}", r.motivo);
        assert_eq!(r.hallazgos.len(), 1, "{:?}", r.hallazgos);
        assert_eq!(
            r.hallazgos[0].cita,
            "módulos: ninguno; falta la capa [protocolo]"
        );
        assert!(r.hallazgos[0].detalle.contains("protocolo"));
    }
}
