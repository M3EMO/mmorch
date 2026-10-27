//! Lee un junit XML (el de `pytest --junitxml`) y cuenta pasa, falla y omite por archivo.

use std::collections::BTreeMap;
use std::fmt::Display;
use std::fs;
use std::path::Path;

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use serde::{Deserialize, Serialize};

use super::ErrorOraculo;
use crate::tipos::Conteo;

/// Conteo de un archivo de tests. Los omitidos no cuentan para el total.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ConteoArchivo {
    pub pasa: u32,
    pub falla: u32,
    pub omite: u32,
    pub fallas: Vec<String>,
}

/// Resultado de un test en una corrida.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResultadoTest {
    Pasa,
    Falla,
    Omite,
}

/// Junit leído: conteo por archivo, resultado por test y propiedades `"<test>.<nombre>" -> valor`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Junit {
    pub archivos: BTreeMap<String, ConteoArchivo>,
    /// Clave `"<archivo>::<nombre>"`, donde `<archivo>` es el último segmento de `classname`.
    pub tests: BTreeMap<String, ResultadoTest>,
    pub propiedades: BTreeMap<String, String>,
}

impl Junit {
    /// Suma de todos los archivos. `total` es pasa más falla.
    pub fn conteo(&self) -> Conteo {
        let pasa: u32 = self.archivos.values().map(|a| a.pasa).sum();
        let falla: u32 = self.archivos.values().map(|a| a.falla).sum();
        Conteo {
            pasa,
            total: pasa + falla,
        }
    }
}

/// Parsea el texto de un junit XML.
pub fn parsear(xml: &str) -> Result<Junit, ErrorOraculo> {
    let mut lector = Reader::from_str(xml);
    let mut junit = Junit::default();
    let mut caso: Option<Caso> = None;
    let mut elementos = 0usize;
    let mut abiertos = 0usize;
    loop {
        match lector.read_event().map_err(malformado)? {
            Event::Start(e) => {
                elementos += 1;
                abiertos += 1;
                abre(&e, &mut caso, &mut junit)?;
            }
            Event::Empty(e) => {
                elementos += 1;
                abre(&e, &mut caso, &mut junit)?;
                if e.local_name().as_ref() == "testcase" {
                    cierra(&mut caso, &mut junit);
                }
            }
            Event::End(e) => {
                abiertos -= 1;
                if e.local_name().as_ref() == "testcase" {
                    cierra(&mut caso, &mut junit);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if elementos == 0 {
        return Err(ErrorOraculo::Junit(
            "el documento no tiene elementos".into(),
        ));
    }
    if abiertos != 0 {
        return Err(ErrorOraculo::Junit("hay elementos sin cerrar".into()));
    }
    Ok(junit)
}

/// `testcase` en curso: sus marcas se acumulan hasta que se cierra.
struct Caso {
    archivo: String,
    nombre: String,
    omitido: bool,
    fallado: bool,
}

/// Procesa la apertura de un elemento. Un `testcase` abre un caso; sus hijos lo marcan.
fn abre(e: &BytesStart, caso: &mut Option<Caso>, junit: &mut Junit) -> Result<(), ErrorOraculo> {
    match e.local_name().as_ref() {
        "testcase" => {
            let classname = atributo(e, "classname")?;
            *caso = Some(Caso {
                archivo: classname.rsplit('.').next().unwrap_or_default().to_string(),
                nombre: atributo(e, "name")?,
                omitido: false,
                fallado: false,
            });
        }
        "skipped" => marca(caso, |c| c.omitido = true),
        "failure" | "error" => marca(caso, |c| c.fallado = true),
        "property" => {
            if let Some(c) = caso {
                let clave = format!("{}.{}", c.nombre, atributo(e, "name")?);
                junit.propiedades.insert(clave, atributo(e, "value")?);
            }
        }
        _ => {}
    }
    Ok(())
}

fn marca(caso: &mut Option<Caso>, cambia: impl FnOnce(&mut Caso)) {
    if let Some(c) = caso {
        cambia(c);
    }
}

/// Suma el caso cerrado al conteo de su archivo. Sin caso abierto no hace nada.
fn cierra(caso: &mut Option<Caso>, junit: &mut Junit) {
    let Some(c) = caso.take() else {
        return;
    };
    let resultado = if c.omitido {
        ResultadoTest::Omite
    } else if c.fallado {
        ResultadoTest::Falla
    } else {
        ResultadoTest::Pasa
    };
    junit
        .tests
        .insert(format!("{}::{}", c.archivo, c.nombre), resultado);
    let archivo = junit.archivos.entry(c.archivo).or_default();
    match resultado {
        ResultadoTest::Omite => archivo.omite += 1,
        ResultadoTest::Falla => {
            archivo.falla += 1;
            archivo.fallas.push(c.nombre);
        }
        ResultadoTest::Pasa => archivo.pasa += 1,
    }
}

/// Valor de un atributo, decodificado. Ausente vale texto vacío.
fn atributo(e: &BytesStart, clave: &str) -> Result<String, ErrorOraculo> {
    match e.try_get_attribute(clave).map_err(malformado)? {
        Some(attr) => Ok(attr
            .normalized_value(XmlVersion::Implicit1_0)
            .map_err(malformado)?
            .into_owned()),
        None => Ok(String::new()),
    }
}

fn malformado(error: impl Display) -> ErrorOraculo {
    ErrorOraculo::Junit(error.to_string())
}

/// Lee y parsea un junit XML del disco.
pub fn leer(ruta: &Path) -> Result<Junit, ErrorOraculo> {
    let xml = fs::read_to_string(ruta)
        .map_err(|e| ErrorOraculo::Infra(format!("no se pudo leer {}: {e}", ruta.display())))?;
    parsear(&xml)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRES_UNA_FALLA: &str = r#"<?xml version="1.0"?>
<testsuites><testsuite name="pytest" tests="3">
  <testcase classname="tests.test_suma" name="test_a" time="0.1"/>
  <testcase classname="tests.test_suma" name="test_b" time="0.1">
    <failure message="assert 1 == 2">detalle</failure>
  </testcase>
  <testcase classname="tests.test_suma" name="test_c" time="0.1"/>
</testsuite></testsuites>"#;

    #[test]
    fn junit_tres_tests_con_una_falla_dan_dos_de_tres() {
        let j = parsear(TRES_UNA_FALLA).expect("parsea");
        assert_eq!(j.conteo(), Conteo { pasa: 2, total: 3 });
        let archivo = &j.archivos["test_suma"];
        assert_eq!(archivo.fallas, vec!["test_b".to_string()]);
    }

    #[test]
    fn junit_skip_no_cuenta_ni_como_pasa_ni_como_total() {
        let xml = r#"<testsuite>
  <testcase classname="tests.test_a" name="t1"/>
  <testcase classname="tests.test_a" name="t2"><skipped message="no"/></testcase>
</testsuite>"#;
        let j = parsear(xml).expect("parsea");
        assert_eq!(j.conteo(), Conteo { pasa: 1, total: 1 });
        assert_eq!(j.archivos["test_a"].omite, 1);
    }

    #[test]
    fn junit_error_y_failure_cuentan_como_falla() {
        let xml = r#"<testsuite>
  <testcase classname="tests.test_a" name="t1"><failure message="x"/></testcase>
  <testcase classname="tests.test_a" name="t2"><error message="y">boom</error></testcase>
  <testcase classname="tests.test_a" name="t3"/>
</testsuite>"#;
        let j = parsear(xml).expect("parsea");
        assert_eq!(j.conteo(), Conteo { pasa: 1, total: 3 });
        assert_eq!(j.archivos["test_a"].fallas, vec!["t1", "t2"]);
    }

    #[test]
    fn junit_propiedades_van_con_el_nombre_del_test() {
        let xml = r#"<testsuite>
  <testcase classname="tests.test_a" name="t1">
    <properties><property name="costo" value="0.5"/></properties>
  </testcase>
</testsuite>"#;
        let j = parsear(xml).expect("parsea");
        assert_eq!(
            j.propiedades.get("t1.costo").map(String::as_str),
            Some("0.5")
        );
    }

    #[test]
    fn junit_dos_classname_dan_dos_archivos_con_el_ultimo_segmento() {
        let xml = r#"<testsuite>
  <testcase classname="tests.test_a" name="t1"/>
  <testcase classname="tests.sub.test_b" name="t2"/>
</testsuite>"#;
        let j = parsear(xml).expect("parsea");
        assert_eq!(j.archivos.len(), 2);
        assert_eq!(j.archivos["test_a"].pasa, 1);
        assert_eq!(j.archivos["test_b"].pasa, 1);
    }

    #[test]
    fn junit_xml_malformado_da_error() {
        assert!(matches!(
            parsear("<testsuite><testcase classname=\"a\" name=\"b\"></testsuite>"),
            Err(ErrorOraculo::Junit(_))
        ));
        assert!(matches!(
            parsear("<testsuite>"),
            Err(ErrorOraculo::Junit(_))
        ));
        assert!(matches!(parsear(""), Err(ErrorOraculo::Junit(_))));
    }

    #[test]
    fn repeticiones_junit_guarda_resultado_por_archivo_y_nombre() {
        let xml = r#"<testsuite>
  <testcase classname="tests.test_a" name="t1"/>
  <testcase classname="tests.test_a" name="t2"><failure message="x"/></testcase>
  <testcase classname="tests.test_a" name="t3"><skipped message="no"/></testcase>
</testsuite>"#;
        let j = parsear(xml).expect("parsea");
        assert_eq!(j.tests["test_a::t1"], ResultadoTest::Pasa);
        assert_eq!(j.tests["test_a::t2"], ResultadoTest::Falla);
        assert_eq!(j.tests["test_a::t3"], ResultadoTest::Omite);
    }

    #[test]
    fn junit_archivo_faltante_da_error() {
        let dir = tempfile::tempdir().expect("temp dir");
        let res = leer(&dir.path().join("no-existe.xml"));
        assert!(matches!(res, Err(ErrorOraculo::Infra(_))));
    }
}
