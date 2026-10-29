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
    /// Clave `"<classname>::<nombre>"` con el `classname` entero (A4). Tras `con_ids`, la clave es el
    /// id `archivo::nombre`. Si un id se repite y una corrida falla, queda en `Falla`.
    pub tests: BTreeMap<String, ResultadoTest>,
    /// Cuántos testcases trae cada id, aunque se repitan. Misma clave que `tests`.
    pub testcases: BTreeMap<String, u32>,
    pub propiedades: BTreeMap<String, String>,
    /// Marca de red de `correr.sh` (`con-red` o `sin-red`), si el junit la trae. Sin ella, `None`.
    pub red: Option<String>,
}

impl Junit {
    /// Copia con las claves de `tests` y `testcases` pasadas a ids `archivo::nombre` (`id_de_clave`).
    /// Dos claves que dan el mismo id se juntan: una falla gana, y los testcases se suman.
    pub fn con_ids(&self) -> Junit {
        let mut salida = Junit {
            archivos: self.archivos.clone(),
            propiedades: self.propiedades.clone(),
            red: self.red.clone(),
            ..Junit::default()
        };
        for (clave, resultado) in &self.tests {
            let entrada = salida.tests.entry(id_de_clave(clave)).or_insert(*resultado);
            if *resultado == ResultadoTest::Falla {
                *entrada = ResultadoTest::Falla;
            }
        }
        for (clave, veces) in &self.testcases {
            *salida.testcases.entry(id_de_clave(clave)).or_default() += veces;
        }
        salida
    }

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
    classname: String,
    nombre: String,
    omitido: bool,
    fallado: bool,
}

/// Procesa la apertura de un elemento. Un `testcase` abre un caso; sus hijos lo marcan.
fn abre(e: &BytesStart, caso: &mut Option<Caso>, junit: &mut Junit) -> Result<(), ErrorOraculo> {
    match e.local_name().as_ref() {
        "testsuite" => {
            let marca = atributo(e, "root_red")?;
            if !marca.is_empty() {
                junit.red = Some(marca);
            }
        }
        "testcase" => {
            *caso = Some(Caso {
                classname: atributo(e, "classname")?,
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
    let clave = id_de(&c.classname, &c.nombre);
    *junit.testcases.entry(clave.clone()).or_default() += 1;
    // Un id repetido con una falla queda en falla: el último pase no la tapa.
    let en_tests = if resultado == ResultadoTest::Falla
        || junit.tests.get(&clave) == Some(&ResultadoTest::Falla)
    {
        ResultadoTest::Falla
    } else {
        resultado
    };
    junit.tests.insert(clave, en_tests);
    let archivo_corto = c
        .classname
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_string();
    let archivo = junit.archivos.entry(archivo_corto).or_default();
    match resultado {
        ResultadoTest::Omite => archivo.omite += 1,
        ResultadoTest::Falla => {
            archivo.falla += 1;
            archivo.fallas.push(c.nombre);
        }
        ResultadoTest::Pasa => archivo.pasa += 1,
    }
}

/// Clave de un testcase: `classname` entero, `::` y nombre. Sin perder la clase ni el archivo (A4).
fn id_de(classname: &str, nombre: &str) -> String {
    format!("{classname}::{nombre}")
}

/// Id `archivo::nombre` de una clave de junit `classname::nombre` (A4). El archivo es el último
/// segmento del `classname` con forma de módulo (`test_*`, `*_test` o `test`). Lo que sigue, con `::`,
/// es la clase y el nombre: `tests.test_set.TestBasico::test_vacio` da `test_set::TestBasico::test_vacio`.
pub fn id_de_clave(clave: &str) -> String {
    let (classname, nombre) = clave.rsplit_once("::").unwrap_or(("", clave));
    let segmentos: Vec<&str> = classname.split('.').filter(|s| !s.is_empty()).collect();
    let indice = segmentos
        .iter()
        .rposition(|s| es_modulo(s))
        .unwrap_or(segmentos.len().saturating_sub(1));
    let archivo = segmentos.get(indice).copied().unwrap_or_default();
    let resto: Vec<&str> = segmentos
        .iter()
        .skip(indice + 1)
        .copied()
        .chain(std::iter::once(nombre))
        .collect();
    format!("{archivo}::{}", resto.join("::"))
}

fn es_modulo(segmento: &str) -> bool {
    segmento.starts_with("test_") || segmento.ends_with("_test") || segmento == "test"
}

/// Clave de junit de un node id de pytest: `tests/test_x.py::Clase::test` -> `tests.test_x.Clase::test`.
/// `None` si el node id no tiene la forma `archivo.py::nombre`.
pub fn clave_de_nodo(nodo: &str) -> Option<String> {
    let (archivo, resto) = nodo.split_once("::")?;
    let modulo = archivo.strip_suffix(".py")?.replace('/', ".");
    let (classname, nombre) = match resto.rsplit_once("::") {
        Some((clases, nombre)) => (format!("{modulo}.{}", clases.replace("::", ".")), nombre),
        None => (modulo, resto),
    };
    Some(format!("{classname}::{nombre}"))
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
        assert_eq!(j.tests["tests.test_a::t1"], ResultadoTest::Pasa);
        assert_eq!(j.tests["tests.test_a::t2"], ResultadoTest::Falla);
        assert_eq!(j.tests["tests.test_a::t3"], ResultadoTest::Omite);
    }

    #[test]
    fn junit_cuenta_testcases_repetidos_por_id_con_la_misma_clave() {
        let xml = r#"<testsuite>
  <testcase classname="tests.test_a" name="t1"/>
  <testcase classname="tests.test_a" name="t1"><failure message="x"/></testcase>
  <testcase classname="tests.test_a" name="t1"/>
</testsuite>"#;
        let j = parsear(xml).expect("parsea");
        assert_eq!(j.testcases["tests.test_a::t1"], 3);
    }

    #[test]
    fn junit_id_repetido_con_una_falla_queda_en_falla() {
        // Con repeticiones, una falla en cualquier corrida no la tapa un pase posterior.
        let xml = r#"<testsuite>
  <testcase classname="tests.test_a" name="t1"><failure message="x"/></testcase>
  <testcase classname="tests.test_a" name="t1"/>
</testsuite>"#;
        let j = parsear(xml).expect("parsea");
        assert_eq!(j.tests["tests.test_a::t1"], ResultadoTest::Falla);
    }

    #[test]
    fn junit_archivo_faltante_da_error() {
        let dir = tempfile::tempdir().expect("temp dir");
        let res = leer(&dir.path().join("no-existe.xml"));
        assert!(matches!(res, Err(ErrorOraculo::Infra(_))));
    }

    #[test]
    fn junit_test_set_y_test_get_con_mismo_nombre_de_clase_cuentan_dos() {
        let xml = r#"<testsuite>
  <testcase classname="tests.test_set.TestBasico" name="test_vacio"><failure message="x"/></testcase>
  <testcase classname="tests.test_get.TestBasico" name="test_vacio"/>
</testsuite>"#;
        let j = parsear(xml).expect("parsea");
        assert_eq!(j.tests.len(), 2, "{:?}", j.tests.keys().collect::<Vec<_>>());
        assert_eq!(
            j.tests["tests.test_set.TestBasico::test_vacio"],
            ResultadoTest::Falla
        );
        assert_eq!(
            j.tests["tests.test_get.TestBasico::test_vacio"],
            ResultadoTest::Pasa
        );
        assert_eq!(j.conteo(), Conteo { pasa: 1, total: 2 });
    }

    #[test]
    fn junit_clave_de_nodo_conserva_el_archivo_y_la_clase() {
        assert_eq!(
            clave_de_nodo("tests/test_set.py::TestBasico::test_vacio").as_deref(),
            Some("tests.test_set.TestBasico::test_vacio")
        );
        assert_eq!(
            clave_de_nodo("tests/test_x.py::test_r[gpu-1]").as_deref(),
            Some("tests.test_x::test_r[gpu-1]")
        );
        assert_eq!(clave_de_nodo("sin_separador"), None);
    }

    #[test]
    fn junit_id_de_clave_pasa_clase_y_nombre_con_el_archivo_del_modulo() {
        assert_eq!(
            id_de_clave("tests.test_set.TestBasico::test_vacio"),
            "test_set::TestBasico::test_vacio"
        );
        assert_eq!(id_de_clave("tests.test_o::o2"), "test_o::o2");
        assert_eq!(id_de_clave("tests.sub.test_b::t2"), "test_b::t2");
        assert_eq!(id_de_clave("tests.utiles::t3"), "utiles::t3");
    }

    #[test]
    fn junit_con_ids_une_claves_que_dan_el_mismo_id_y_gana_la_falla() {
        let xml = r#"<testsuite>
  <testcase classname="tests.sub.test_a" name="t1"><failure message="x"/></testcase>
  <testcase classname="tests.test_a" name="t1"/>
  <testcase classname="tests.test_b" name="t2"/>
</testsuite>"#;
        let j = parsear(xml).expect("parsea").con_ids();
        assert_eq!(j.tests.len(), 2);
        assert_eq!(j.tests["test_a::t1"], ResultadoTest::Falla);
        assert_eq!(j.testcases["test_a::t1"], 2);
        assert_eq!(j.tests["test_b::t2"], ResultadoTest::Pasa);
    }

    #[test]
    fn junit_testsuite_lee_la_marca_de_red_de_correr_sh() {
        let xml = r#"<testsuite name="pendientes" root_red="con-red">
  <testcase classname="tests.test_a" name="t1"/>
</testsuite>"#;
        let j = parsear(xml).expect("parsea");
        assert_eq!(j.red.as_deref(), Some("con-red"));
        let sin =
            parsear(r#"<testsuite><testcase classname="tests.test_a" name="t1"/></testsuite>"#)
                .expect("parsea");
        assert_eq!(sin.red, None);
    }
}
