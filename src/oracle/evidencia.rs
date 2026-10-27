//! Gate de evidencia: descarta lo que no tiene respaldo textual en el repo.
//! Funciones puras: sin LLM, sin red. Lo usan el juez, la ficha, el informe y REA.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::tipos::{Fuente, Hallazgo};

/// Veredicto del gate para un hallazgo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Verificacion {
    Valido,
    Descartado(Motivo),
}

/// Razón por la que el gate descarta un hallazgo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Motivo {
    ArchivoInexistente,
    ArchivoFueraDeRaiz,
    CitaAusente,
    SinCita,
}

/// Hallazgos separados por el gate. Conserva el orden original.
#[derive(Debug, Clone, PartialEq)]
pub struct Filtrado {
    pub validos: Vec<Hallazgo>,
    pub descartados: Vec<(Hallazgo, Motivo)>,
}

/// Verdadero si `cita` aparece en `contenido` cerca de `linea` (1-based, ±3 líneas).
/// Compara sin espacios en blanco: la indentación y el espaciado no cuentan.
pub fn cita_en_archivo(contenido: &str, linea: u32, cita: &str) -> bool {
    let aguja = sin_blancos(cita);
    if aguja.is_empty() || linea == 0 {
        return false;
    }
    let lineas: Vec<&str> = contenido.lines().collect();
    let centro = linea as usize;
    if centro > lineas.len() {
        return false;
    }
    let ini = centro.saturating_sub(VENTANA).max(1);
    let fin = (centro + VENTANA).min(lineas.len());
    let ventana = lineas[ini - 1..fin].join("\n");
    sin_blancos(&ventana).contains(&aguja)
}

/// Resuelve `h.archivo` dentro de `raiz` y aplica `cita_en_archivo`.
pub fn verificar_hallazgo(raiz: &Path, h: &Hallazgo) -> Verificacion {
    let ruta = match ruta_dentro(raiz, &h.archivo) {
        Ok(ruta) => ruta,
        Err(motivo) => return Verificacion::Descartado(motivo),
    };
    let Ok(bytes) = fs::read(&ruta) else {
        return Verificacion::Descartado(Motivo::ArchivoInexistente);
    };
    if h.cita.trim().is_empty() {
        return Verificacion::Descartado(Motivo::SinCita);
    }
    let contenido = String::from_utf8_lossy(&bytes);
    if cita_en_archivo(&contenido, h.linea, &h.cita) {
        Verificacion::Valido
    } else {
        Verificacion::Descartado(Motivo::CitaAusente)
    }
}

/// Separa los hallazgos. Los de `Fuente::Sonda` pasan sin verificar.
pub fn filtrar_hallazgos(raiz: &Path, hs: Vec<Hallazgo>) -> Filtrado {
    let mut validos = Vec::new();
    let mut descartados = Vec::new();
    for h in hs {
        if matches!(h.fuente, Fuente::Sonda(_)) {
            validos.push(h);
            continue;
        }
        match verificar_hallazgo(raiz, &h) {
            Verificacion::Valido => validos.push(h),
            Verificacion::Descartado(motivo) => descartados.push((h, motivo)),
        }
    }
    Filtrado {
        validos,
        descartados,
    }
}

/// Divide `texto` en oraciones y separa las que citan un id de `ids` como palabra completa.
/// Devuelve `(conservadas, descartadas)`.
pub fn oraciones_con_id(texto: &str, ids: &BTreeSet<String>) -> (Vec<String>, Vec<String>) {
    let mut conservadas = Vec::new();
    let mut descartadas = Vec::new();
    for oracion in oraciones(texto) {
        if ids.iter().any(|id| contiene_id(&oracion, id)) {
            conservadas.push(oracion);
        } else {
            descartadas.push(oracion);
        }
    }
    (conservadas, descartadas)
}

/// Líneas de contexto a cada lado de la línea citada.
const VENTANA: usize = 3;

/// Quita todo carácter de espacio en blanco.
fn sin_blancos(texto: &str) -> String {
    texto.chars().filter(|c| !c.is_whitespace()).collect()
}

/// Resuelve `archivo` dentro de `raiz`. Rechaza rutas que salen de `raiz`.
fn ruta_dentro(raiz: &Path, archivo: &str) -> Result<PathBuf, Motivo> {
    let relativa = Path::new(archivo);
    if relativa.is_absolute() {
        return Err(Motivo::ArchivoFueraDeRaiz);
    }
    // Chequeo léxico: un `..` que sube por encima de la raíz es fuera de raíz.
    let mut profundidad: usize = 0;
    for componente in relativa.components() {
        match componente {
            Component::Normal(_) => profundidad += 1,
            Component::ParentDir => {
                profundidad = profundidad
                    .checked_sub(1)
                    .ok_or(Motivo::ArchivoFueraDeRaiz)?;
            }
            Component::CurDir => {}
            Component::RootDir | Component::Prefix(_) => return Err(Motivo::ArchivoFueraDeRaiz),
        }
    }
    let raiz_real = raiz
        .canonicalize()
        .map_err(|_| Motivo::ArchivoInexistente)?;
    let ruta = raiz.join(relativa);
    // Chequeo real: sigue los enlaces simbólicos y compara con la raíz real.
    let real = ruta
        .canonicalize()
        .map_err(|_| Motivo::ArchivoInexistente)?;
    if !real.starts_with(&raiz_real) {
        return Err(Motivo::ArchivoFueraDeRaiz);
    }
    Ok(real)
}

/// Parte el texto en oraciones: por saltos de línea y por `.`, `!`, `?` seguidos de espacio o fin.
fn oraciones(texto: &str) -> Vec<String> {
    let mut salida = Vec::new();
    for linea in texto.lines() {
        let caracteres: Vec<char> = linea.chars().collect();
        let mut actual = String::new();
        for (i, &c) in caracteres.iter().enumerate() {
            actual.push(c);
            let cierra = matches!(c, '.' | '!' | '?')
                && caracteres.get(i + 1).is_none_or(|s| s.is_whitespace());
            if cierra {
                salida.push(std::mem::take(&mut actual));
            }
        }
        salida.push(actual);
    }
    salida
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Verdadero si `id` aparece en `oracion` como palabra completa.
fn contiene_id(oracion: &str, id: &str) -> bool {
    if id.is_empty() {
        return false;
    }
    oracion.match_indices(id).any(|(pos, _)| {
        frontera_antes(&oracion[..pos]) && frontera_despues(&oracion[pos + id.len()..])
    })
}

/// Caracteres que forman parte de un id: letras, dígitos, `_` y `-`.
fn parte_de_id(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

/// Verdadero si lo que queda antes del id no continúa el id.
fn frontera_antes(previo: &str) -> bool {
    let mut atras = previo.chars().rev();
    match atras.next() {
        None => true,
        // Un `.` solo pertenece al id si lo precede un carácter de id.
        Some('.') => !atras.next().is_some_and(char::is_alphanumeric),
        Some(c) => !parte_de_id(c),
    }
}

/// Verdadero si lo que sigue al id no continúa el id.
fn frontera_despues(siguiente: &str) -> bool {
    let mut adelante = siguiente.chars();
    match adelante.next() {
        None => true,
        // Un `.` solo pertenece al id si lo sigue un carácter alfanumérico.
        Some('.') => !adelante.next().is_some_and(char::is_alphanumeric),
        Some(c) => !parte_de_id(c),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tipos::Dimension;
    use std::fs;

    fn hallazgo_juez(archivo: &str, linea: u32, cita: &str) -> Hallazgo {
        Hallazgo {
            dimension: Dimension::Diseno,
            severidad: 2,
            archivo: archivo.to_string(),
            linea,
            cita: cita.to_string(),
            detalle: "d".to_string(),
            fuente: Fuente::Juez {
                modelo: "deepseek-reasoner".to_string(),
            },
            id: String::new(),
            impacto: vec!["R1".to_string()],
        }
    }

    /// Archivo de diez líneas. Cada línea tiene una marca única `LINEA<n>`.
    fn diez_lineas() -> String {
        (1..=10)
            .map(|n| format!("linea {n} LINEA{n}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn evidencia_cita_inventada_se_descarta() {
        let contenido = "fn main() {\n    println!(\"hola\");\n}\n";
        assert!(!cita_en_archivo(contenido, 2, "panic!(\"adios\");"));
    }

    #[test]
    fn evidencia_cita_con_otra_indentacion_y_espaciado_se_acepta() {
        let contenido = "fn f() {\n    let x   =  foo( a ,b );\n}\n";
        assert!(cita_en_archivo(contenido, 2, "let x = foo(a,b);"));
    }

    #[test]
    fn evidencia_cita_en_mas_tres_se_acepta_y_en_mas_cuatro_se_descarta() {
        let contenido = diez_lineas();
        // La marca LINEA5 está en la línea 5.
        assert!(cita_en_archivo(&contenido, 2, "LINEA5"));
        assert!(!cita_en_archivo(&contenido, 1, "LINEA5"));
    }

    #[test]
    fn evidencia_cita_en_menos_tres_se_acepta_y_en_menos_cuatro_se_descarta() {
        let contenido = diez_lineas();
        // La marca LINEA2 está en la línea 2.
        assert!(cita_en_archivo(&contenido, 5, "LINEA2"));
        assert!(!cita_en_archivo(&contenido, 6, "LINEA2"));
    }

    #[test]
    fn evidencia_ventana_recortada_en_la_primera_linea() {
        let contenido = diez_lineas();
        assert!(cita_en_archivo(&contenido, 1, "LINEA1"));
        // Línea 1 con ventana hasta la línea 4.
        assert!(cita_en_archivo(&contenido, 1, "LINEA4"));
        assert!(!cita_en_archivo(&contenido, 1, "LINEA5"));
    }

    #[test]
    fn evidencia_archivo_de_una_sola_linea() {
        let contenido = "x = 1";
        assert!(cita_en_archivo(contenido, 1, "x=1"));
        assert!(!cita_en_archivo(contenido, 2, "x=1"));
    }

    #[test]
    fn evidencia_cita_vacia_es_falsa() {
        let contenido = diez_lineas();
        assert!(!cita_en_archivo(&contenido, 1, ""));
        assert!(!cita_en_archivo(&contenido, 1, "   \t "));
    }

    #[test]
    fn evidencia_linea_cero_es_falsa() {
        let contenido = diez_lineas();
        assert!(!cita_en_archivo(&contenido, 0, "LINEA1"));
    }

    #[test]
    fn evidencia_linea_fuera_de_rango_es_falsa() {
        let contenido = diez_lineas();
        assert!(!cita_en_archivo(&contenido, 11, "LINEA10"));
        assert!(!cita_en_archivo(&contenido, 99, "LINEA1"));
        assert!(!cita_en_archivo("", 1, "algo"));
    }

    #[test]
    fn evidencia_cita_multilinea_se_busca_en_la_ventana_unida() {
        let contenido = "a = [\n    1,\n    2,\n];\n";
        assert!(cita_en_archivo(contenido, 1, "a = [\n    1,\n    2,\n];"));
        assert!(!cita_en_archivo(contenido, 1, "a = [3];"));
    }

    #[test]
    fn evidencia_archivo_inexistente_se_descarta() {
        let raiz = tempfile::tempdir().expect("tempdir");
        let h = hallazgo_juez("no_existe.rs", 1, "x");
        assert_eq!(
            verificar_hallazgo(raiz.path(), &h),
            Verificacion::Descartado(Motivo::ArchivoInexistente)
        );
    }

    #[test]
    fn evidencia_ruta_absoluta_se_descarta() {
        let raiz = tempfile::tempdir().expect("tempdir");
        let abs = raiz.path().join("a.rs");
        fs::write(&abs, "x").expect("escribe");
        let h = hallazgo_juez(abs.to_str().expect("utf8"), 1, "x");
        assert_eq!(
            verificar_hallazgo(raiz.path(), &h),
            Verificacion::Descartado(Motivo::ArchivoFueraDeRaiz)
        );
    }

    #[test]
    fn evidencia_ruta_con_punto_punto_que_sale_se_descarta() {
        let padre = tempfile::tempdir().expect("tempdir");
        let raiz = padre.path().join("repo");
        fs::create_dir(&raiz).expect("crea raiz");
        fs::write(padre.path().join("fuera.txt"), "secreto").expect("escribe");
        let h = hallazgo_juez("../fuera.txt", 1, "secreto");
        assert_eq!(
            verificar_hallazgo(&raiz, &h),
            Verificacion::Descartado(Motivo::ArchivoFueraDeRaiz)
        );
    }

    #[cfg(unix)]
    #[test]
    fn evidencia_symlink_que_sale_de_la_raiz_se_descarta() {
        let padre = tempfile::tempdir().expect("tempdir");
        let raiz = padre.path().join("repo");
        fs::create_dir(&raiz).expect("crea raiz");
        let objetivo = padre.path().join("objetivo.txt");
        fs::write(&objetivo, "secreto").expect("escribe");
        std::os::unix::fs::symlink(&objetivo, raiz.join("enlace.txt")).expect("symlink");
        let h = hallazgo_juez("enlace.txt", 1, "secreto");
        assert_eq!(
            verificar_hallazgo(&raiz, &h),
            Verificacion::Descartado(Motivo::ArchivoFueraDeRaiz)
        );
    }

    #[test]
    fn evidencia_hallazgo_con_cita_real_es_valido() {
        let raiz = tempfile::tempdir().expect("tempdir");
        fs::write(raiz.path().join("a.rs"), "fn f() {}\nlet y = 2;\n").expect("escribe");
        let h = hallazgo_juez("a.rs", 2, "let y = 2;");
        assert_eq!(verificar_hallazgo(raiz.path(), &h), Verificacion::Valido);
    }

    #[test]
    fn evidencia_cita_vacia_en_hallazgo_da_sin_cita() {
        let raiz = tempfile::tempdir().expect("tempdir");
        fs::write(raiz.path().join("a.rs"), "x\n").expect("escribe");
        let h = hallazgo_juez("a.rs", 1, "  ");
        assert_eq!(
            verificar_hallazgo(raiz.path(), &h),
            Verificacion::Descartado(Motivo::SinCita)
        );
    }

    #[test]
    fn evidencia_filtrar_separa_juez_inventado_y_sonda_en_orden() {
        let raiz = tempfile::tempdir().expect("tempdir");
        fs::write(raiz.path().join("a.rs"), "let x = 1;\nlet y = 2;\n").expect("escribe");
        let valido = hallazgo_juez("a.rs", 1, "let x = 1;");
        let inventado = hallazgo_juez("a.rs", 1, "let z = 9;");
        let mut sonda = hallazgo_juez("a.rs", 2, "cita que no existe");
        sonda.fuente = Fuente::Sonda("bandit".to_string());

        let f = filtrar_hallazgos(
            raiz.path(),
            vec![valido.clone(), inventado.clone(), sonda.clone()],
        );

        assert_eq!(f.validos.len(), 2);
        assert_eq!(f.descartados.len(), 1);
        assert_eq!(f.validos[0], valido);
        assert_eq!(f.validos[1], sonda);
        assert_eq!(f.descartados[0], (inventado, Motivo::CitaAusente));
    }

    fn ids(v: &[&str]) -> BTreeSet<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn evidencia_oraciones_con_id_conserva_solo_las_que_citan_el_id() {
        let (ok, malas) =
            oraciones_con_id("El módulo R1 falla. Es malo. R10 también.", &ids(&["R1"]));
        assert_eq!(ok, vec!["El módulo R1 falla.".to_string()]);
        assert_eq!(
            malas,
            vec!["Es malo.".to_string(), "R10 también.".to_string()]
        );
    }

    #[test]
    fn evidencia_oraciones_con_id_con_punto_se_reconoce() {
        let (ok, malas) = oraciones_con_id(
            "Se viola diseno.modularidad en el archivo.",
            &ids(&["diseno.modularidad"]),
        );
        assert_eq!(ok.len(), 1);
        assert!(malas.is_empty());
    }

    #[test]
    fn evidencia_oraciones_con_id_texto_vacio_no_rompe() {
        let (ok, malas) = oraciones_con_id("", &ids(&["R1"]));
        assert!(ok.is_empty());
        assert!(malas.is_empty());
    }

    #[test]
    fn evidencia_oraciones_con_id_parte_por_saltos_de_linea() {
        let (ok, malas) = oraciones_con_id("R1 falla\nNada aquí", &ids(&["R1"]));
        assert_eq!(ok, vec!["R1 falla".to_string()]);
        assert_eq!(malas, vec!["Nada aquí".to_string()]);
    }
}
