//! Eficiencia del repo evaluado contra su estado inicial (`bootstrap/`). Mide cuatro cosas:
//! tiempo de tests, pico de memoria, tiempo de compilación y tamaño de los binarios.
//! Cada lado corre en su propio ejecutor. `comparar` hace toda la aritmética.

use std::collections::BTreeSet;

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use serde_json::Value;

use crate::oracle::descarga::PRELUDIO;
use crate::oracle::perfiles::herramientas::EjecutorHerramienta;

/// Corridas por lado. rustc-perf: tres corridas y se compara el mínimo.
pub const CORRIDAS: usize = 3;
/// Piso de ruido: 2 % de cambio. Perfherder usa este piso.
pub const PISO_RUIDO: f64 = 0.02;
/// Más de 1,5 veces da "no listo". Decisión P105 a del usuario.
pub const UMBRAL_NO_LISTO: f64 = 1.5;

/// Las cuatro medidas. El orden del enum es el orden de salida de `medir`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Medida {
    TiempoTests,
    MemoriaPico,
    TiempoCompilacion,
    TamanoBinario,
}

impl Medida {
    /// Devuelve el nombre estable de la medida, para reportes.
    pub fn id(self) -> &'static str {
        match self {
            Medida::TiempoTests => "tiempo_tests",
            Medida::MemoriaPico => "memoria_pico",
            Medida::TiempoCompilacion => "tiempo_compilacion",
            Medida::TamanoBinario => "tamano_binario",
        }
    }
}

/// Resultado de comparar dos lados. `razon` es repo sobre referencia.
#[derive(Debug, Clone, PartialEq)]
pub struct Comparacion {
    pub razon: f64,
    pub ruido: f64,
    pub regresion: bool,
}

/// Compara las corridas de cada lado. Usa el mínimo de cada lado. `None` si no se puede comparar.
pub fn comparar(referencia: &[f64], repo: &[f64]) -> Option<Comparacion> {
    let min_ref = referencia.iter().copied().reduce(f64::min)?;
    let max_ref = referencia.iter().copied().reduce(f64::max)?;
    let min_repo = repo.iter().copied().reduce(f64::min)?;
    if min_ref <= 0.0 {
        return None;
    }
    let razon = min_repo / min_ref;
    // El ruido es la dispersión de la referencia, con el piso mínimo.
    let ruido = ((max_ref - min_ref) / min_ref).max(PISO_RUIDO);
    Some(Comparacion {
        razon,
        ruido,
        regresion: razon > 1.0 + ruido,
    })
}

/// Una serie de valores de un lado. Un error deja la serie sin valores y con su motivo.
type Serie = Result<Vec<f64>, String>;

/// Agrega un valor a la serie. El primer error queda: la serie no vuelve a tener valores.
fn anotar(serie: &mut Serie, valor: Result<f64, String>) {
    match (serie.is_ok(), valor) {
        (true, Ok(x)) => {
            if let Ok(valores) = serie {
                valores.push(x);
            }
        }
        (true, Err(motivo)) => *serie = Err(motivo),
        (false, _) => {}
    }
}

/// Une los motivos sin repetir, separados por `; `.
fn unir_motivos(motivos: impl IntoIterator<Item = String>) -> String {
    let mut unicos: Vec<String> = Vec::new();
    for motivo in motivos {
        if !unicos.contains(&motivo) {
            unicos.push(motivo);
        }
    }
    unicos.join("; ")
}

fn sin_valor(medida: Medida, motivo: String) -> ResultadoMedida {
    ResultadoMedida {
        medida,
        referencia: None,
        repo: None,
        comparacion: None,
        motivo: Some(motivo),
    }
}

/// Compara las dos series de una medida. Si un lado falla, la medida no tiene valor.
fn combinar(medida: Medida, referencia: &Serie, repo: &Serie) -> ResultadoMedida {
    match (referencia, repo) {
        (Ok(r), Ok(p)) => match comparar(r, p) {
            Some(comparacion) => ResultadoMedida {
                medida,
                referencia: r.iter().copied().reduce(f64::min),
                repo: p.iter().copied().reduce(f64::min),
                comparacion: Some(comparacion),
                motivo: None,
            },
            None => sin_valor(medida, "no se puede comparar".into()),
        },
        _ => {
            let motivos = [referencia, repo]
                .into_iter()
                .filter_map(|s| s.as_ref().err().cloned());
            sin_valor(medida, unir_motivos(motivos))
        }
    }
}

/// Guarda los paquetes y binarios de un lado. Sus nombres son aptos para un shell.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Metadatos {
    paquetes: Vec<String>,
    binarios: Vec<String>,
}

/// Guarda los nombres de tests de un lado y los que pasan.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Listas {
    todos: BTreeSet<String>,
    pasan: BTreeSet<String>,
}

/// Motivo que da la medida de tests cuando no hay tests comunes.
const MOTIVO_SIN_COMUNES: &str = "ningún test pasa en los dos lados";

/// Dice si `nombre` no está vacío y solo tiene letras, dígitos y los `extra` permitidos.
fn cabe(nombre: &str, extra: &[char]) -> bool {
    !nombre.is_empty()
        && nombre
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || extra.contains(&c))
}

/// Ejecuta `cargo metadata`. Deja el JSON en `/salida/metadata.json`.
pub(crate) fn cmd_metadata() -> String {
    format!(
        "{PRELUDIO}cd /repo && cargo metadata --no-deps --format-version 1 --offline > /salida/metadata.json"
    )
}

/// Ejecuta nextest y deja el junit en `/salida/lista.xml`. Su código de salida no cuenta.
pub(crate) fn cmd_lista() -> String {
    format!(
        "{PRELUDIO}cd /repo && rm -f /salida/target/nextest/ci/junit.xml; CARGO_TARGET_DIR=/salida/target cargo nextest run --offline --profile ci --tool-config-file root:/etc/root-oraculo/nextest.toml; s=$?; cp /salida/target/nextest/ci/junit.xml /salida/lista.xml 2>/dev/null; exit $s"
    )
}

/// Ejecuta una corrida de tests con el filtro `filtro` (vacío o ` -E '...'`).
/// Lee `ROOT_NS` (nanosegundos) y `ROOT_MEM` (bytes, vacío si no hay `memory.peak`).
/// El `--no-run` compila antes del reloj. Cada contenedor es nuevo, así que el pico de memoria incluye esa compilación.
pub(crate) fn cmd_tests(filtro: &str) -> String {
    format!(
        "{PRELUDIO}cd /repo && export CARGO_TARGET_DIR=/salida/target && cargo nextest run --offline --profile ci --tool-config-file root:/etc/root-oraculo/nextest.toml --no-run >/dev/null 2>&1; a=$(date +%s%N); cargo nextest run --offline --profile ci --tool-config-file root:/etc/root-oraculo/nextest.toml{filtro} >/dev/null 2>&1; b=$(date +%s%N); echo \"ROOT_NS $((b-a))\"; echo \"ROOT_MEM $(cat /sys/fs/cgroup/memory.peak 2>/dev/null)\""
    )
}

/// Ejecuta una compilación medida. Limpia solo los paquetes del workspace antes del reloj.
/// Lee `ROOT_NS`, `ROOT_BUILD` (código de salida) y, si hay binarios, `ROOT_SIZE` (bytes sumados).
/// Un binario que falta deja `FALTA` en vez de un tamaño parcial.
pub(crate) fn cmd_compilacion(paquetes: &[String], binarios: &[String]) -> String {
    let limpiar: String = paquetes.iter().map(|p| format!(" -p {p}")).collect();
    let tamano = if binarios.is_empty() {
        String::new()
    } else {
        let rutas = binarios.join(" ");
        format!(
            "; echo \"ROOT_SIZE $(for f in {rutas}; do stat -c %s /salida/target/release/$f || echo FALTA; done | awk '/^[0-9]+$/{{s+=$1; next}} {{falta=1}} END{{print (falta ? \"FALTA\" : s)}}')\""
        )
    };
    format!(
        "{PRELUDIO}cd /repo && export CARGO_TARGET_DIR=/salida/target && cargo build --release --offline >/dev/null 2>&1; cargo clean --release{limpiar} >/dev/null 2>&1; a=$(date +%s%N); cargo build --release --offline >/dev/null 2>&1; s=$?; b=$(date +%s%N); echo \"ROOT_NS $((b-a))\"; echo \"ROOT_BUILD $s\"{tamano}"
    )
}

/// Lee un JSON de `cargo metadata`. Descarta los nombres que no son aptos para un shell.
fn parsear_metadata(json: &str) -> Result<Metadatos, String> {
    let valor: Value = serde_json::from_str(json).map_err(|e| format!("metadata ilegible: {e}"))?;
    let paquetes_json = valor["packages"]
        .as_array()
        .ok_or("metadata sin packages")?;
    let mut paquetes = BTreeSet::new();
    let mut binarios = BTreeSet::new();
    for paquete in paquetes_json {
        if let Some(nombre) = paquete["name"].as_str().filter(|n| cabe(n, &['-', '_'])) {
            paquetes.insert(nombre.to_string());
        }
        for target in paquete["targets"].as_array().into_iter().flatten() {
            let es_bin = target["kind"]
                .as_array()
                .is_some_and(|kinds| kinds.iter().any(|k| k.as_str() == Some("bin")));
            if let Some(nombre) = target["name"]
                .as_str()
                .filter(|n| es_bin && cabe(n, &['-', '_']))
            {
                binarios.insert(nombre.to_string());
            }
        }
    }
    Ok(Metadatos {
        paquetes: paquetes.into_iter().collect(),
        binarios: binarios.into_iter().collect(),
    })
}

fn leer_metadata(ej: &dyn EjecutorHerramienta) -> Result<Metadatos, String> {
    let salida = ej.ejecutar(&cmd_metadata()).map_err(|e| e.to_string())?;
    let json = salida
        .archivos
        .get("metadata.json")
        .ok_or("sin metadata.json")?;
    parsear_metadata(json)
}

/// Lee el nombre del `testcase`. Da error si el atributo falta.
fn nombre_del_caso(e: &BytesStart) -> Result<String, String> {
    let attr = e
        .try_get_attribute("name")
        .map_err(|x| format!("lista de tests ilegible: {x}"))?
        .ok_or_else(|| "lista de tests ilegible: testcase sin name".to_string())?;
    attr.normalized_value(XmlVersion::Implicit1_0)
        .map(|v| v.into_owned())
        .map_err(|x| format!("lista de tests ilegible: {x}"))
}

/// Lee el junit de nextest. Un `failure`, `error` o `skipped` marca que el test no pasa.
/// Un nombre que falla en cualquier `testcase` no pasa.
fn parsear_lista(xml: &str) -> Result<Listas, String> {
    let mut lector = Reader::from_str(xml);
    let mut todos = BTreeSet::new();
    let mut fallan = BTreeSet::new();
    let mut abierto: Option<String> = None;
    loop {
        match lector
            .read_event()
            .map_err(|e| format!("lista de tests ilegible: {e}"))?
        {
            Event::Start(e) if e.local_name().as_ref() == "testcase" => {
                let nombre = nombre_del_caso(&e)?;
                todos.insert(nombre.clone());
                abierto = Some(nombre);
            }
            Event::Empty(e) if e.local_name().as_ref() == "testcase" => {
                todos.insert(nombre_del_caso(&e)?);
            }
            Event::Start(e) | Event::Empty(e)
                if matches!(e.local_name().as_ref(), "failure" | "error" | "skipped") =>
            {
                if let Some(nombre) = &abierto {
                    fallan.insert(nombre.clone());
                }
            }
            Event::End(e) if e.local_name().as_ref() == "testcase" => abierto = None,
            Event::Eof => break,
            _ => {}
        }
    }
    let pasan = todos.difference(&fallan).cloned().collect();
    Ok(Listas { todos, pasan })
}

fn leer_listas(ej: &dyn EjecutorHerramienta) -> Result<Listas, String> {
    let salida = ej.ejecutar(&cmd_lista()).map_err(|e| e.to_string())?;
    let xml = salida
        .archivos
        .get("lista.xml")
        .ok_or("sin lista de tests")?;
    parsear_lista(xml)
}

/// Arma el filtro `-E` de las corridas. Excluye los tests que no pasan en los dos lados.
/// Da cadena vacía si no hay nada que excluir. Da el motivo si la medida de tests no tiene valor.
fn preparar_filtro(
    inicial: &Result<Listas, String>,
    repo: &Result<Listas, String>,
) -> Result<String, String> {
    let (Ok(i), Ok(r)) = (inicial, repo) else {
        return Err(unir_motivos(
            [inicial.as_ref().err(), repo.as_ref().err()]
                .into_iter()
                .flatten()
                .cloned(),
        ));
    };
    let comunes: BTreeSet<&String> = i.pasan.intersection(&r.pasan).collect();
    if comunes.is_empty() {
        return Err(MOTIVO_SIN_COMUNES.into());
    }
    let excluir: Vec<&String> = i
        .todos
        .union(&r.todos)
        .filter(|t| !comunes.contains(t))
        .collect();
    if excluir.iter().any(|t| !cabe(t, &['_', ':'])) {
        return Err("nombre de test no apto para el filtro".into());
    }
    if excluir.is_empty() {
        return Ok(String::new());
    }
    let expr = excluir
        .iter()
        .map(|t| format!("test(={t})"))
        .collect::<Vec<_>>()
        .join(" | ");
    Ok(format!(" -E 'not ({expr})'"))
}

/// Mide tiempo y memoria de los tests. Corre los dos lados alternados, `CORRIDAS` veces.
fn medir_tests(
    inicial: &dyn EjecutorHerramienta,
    repo: &dyn EjecutorHerramienta,
    lista_i: &Result<Listas, String>,
    lista_r: &Result<Listas, String>,
) -> (ResultadoMedida, ResultadoMedida) {
    let filtro = match preparar_filtro(lista_i, lista_r) {
        Ok(filtro) => filtro,
        Err(motivo) => {
            return (
                sin_valor(Medida::TiempoTests, motivo.clone()),
                sin_valor(Medida::MemoriaPico, motivo),
            );
        }
    };
    let lados = [("inicial", inicial), ("repo", repo)];
    let mut tiempo: [Serie; 2] = [Ok(vec![]), Ok(vec![])];
    let mut memoria: [Serie; 2] = [Ok(vec![]), Ok(vec![])];
    for _ in 0..CORRIDAS {
        for (k, (nombre, ej)) in lados.iter().enumerate() {
            if tiempo[k].is_err() && memoria[k].is_err() {
                continue;
            }
            match ej.ejecutar(&cmd_tests(&filtro)) {
                Err(e) => {
                    let motivo = format!("{nombre}: {e}");
                    anotar(&mut tiempo[k], Err(motivo.clone()));
                    anotar(&mut memoria[k], Err(motivo));
                }
                Ok(salida) => {
                    let ns = leer_marca(&salida.stdout, "ROOT_NS").map(|ns| ns / 1e9);
                    anotar(
                        &mut tiempo[k],
                        ns.ok_or_else(|| format!("{nombre}: sin ROOT_NS")),
                    );
                    let mem = leer_marca(&salida.stdout, "ROOT_MEM");
                    anotar(
                        &mut memoria[k],
                        mem.ok_or_else(|| "sin memory.peak".to_string()),
                    );
                }
            }
        }
    }
    (
        combinar(Medida::TiempoTests, &tiempo[0], &tiempo[1]),
        combinar(Medida::MemoriaPico, &memoria[0], &memoria[1]),
    )
}

/// Mide la compilación y el tamaño de los binarios. Usa los paquetes y binarios del lado `repo`.
/// Los dos lados tienen los mismos nombres de paquete, así que el comando sirve para ambos.
fn medir_compilacion(
    inicial: &dyn EjecutorHerramienta,
    repo: &dyn EjecutorHerramienta,
    meta_i: &Result<Metadatos, String>,
    meta_r: &Result<Metadatos, String>,
) -> (ResultadoMedida, ResultadoMedida) {
    let (Ok(_), Ok(meta)) = (meta_i, meta_r) else {
        let motivo = unir_motivos(
            [meta_i.as_ref().err(), meta_r.as_ref().err()]
                .into_iter()
                .flatten()
                .cloned(),
        );
        return (
            sin_valor(Medida::TiempoCompilacion, motivo.clone()),
            sin_valor(Medida::TamanoBinario, motivo),
        );
    };
    let sin = |motivo: &str| -> [Serie; 2] { [Err(motivo.into()), Err(motivo.into())] };
    let lados = [("inicial", inicial), ("repo", repo)];
    let mut compilacion: [Serie; 2] = [Ok(vec![]), Ok(vec![])];
    let mut tamano: [Serie; 2] = [Ok(vec![]), Ok(vec![])];
    if meta.paquetes.is_empty() {
        compilacion = sin("sin paquetes");
        tamano = sin(if meta.binarios.is_empty() {
            "sin binarios"
        } else {
            "sin paquetes"
        });
    } else {
        if meta.binarios.is_empty() {
            tamano = sin("sin binarios");
        }
        let comando = cmd_compilacion(&meta.paquetes, &meta.binarios);
        for corrida in 0..CORRIDAS {
            for (k, (nombre, ej)) in lados.iter().enumerate() {
                if compilacion[k].is_err() {
                    continue;
                }
                match ej.ejecutar(&comando) {
                    Err(e) => {
                        let motivo = format!("{nombre}: {e}");
                        anotar(&mut compilacion[k], Err(motivo.clone()));
                        anotar(&mut tamano[k], Err(motivo));
                    }
                    Ok(salida) => match leer_marca(&salida.stdout, "ROOT_BUILD") {
                        Some(0.0) => {
                            let ns = leer_marca(&salida.stdout, "ROOT_NS").map(|ns| ns / 1e9);
                            anotar(
                                &mut compilacion[k],
                                ns.ok_or_else(|| format!("{nombre}: sin ROOT_NS")),
                            );
                            if corrida == 0 {
                                let size = leer_marca(&salida.stdout, "ROOT_SIZE");
                                anotar(
                                    &mut tamano[k],
                                    size.ok_or_else(|| "sin tamaño".to_string()),
                                );
                            }
                        }
                        Some(_) => {
                            anotar(&mut compilacion[k], Err("la compilación falló".into()));
                            anotar(&mut tamano[k], Err("la compilación falló".into()));
                        }
                        None => {
                            anotar(&mut compilacion[k], Err("salida ilegible del build".into()));
                            anotar(&mut tamano[k], Err("salida ilegible del build".into()));
                        }
                    },
                }
            }
        }
    }
    (
        combinar(Medida::TiempoCompilacion, &compilacion[0], &compilacion[1]),
        combinar(Medida::TamanoBinario, &tamano[0], &tamano[1]),
    )
}

/// Una medida con sus dos valores comparados, o el motivo por el que no tiene valor.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultadoMedida {
    pub medida: Medida,
    pub referencia: Option<f64>,
    pub repo: Option<f64>,
    pub comparacion: Option<Comparacion>,
    pub motivo: Option<String>,
}

/// Mide el repo contra el estado inicial. Siempre devuelve cuatro resultados, en orden de `Medida`.
pub fn medir(
    inicial: &dyn EjecutorHerramienta,
    repo: &dyn EjecutorHerramienta,
) -> Vec<ResultadoMedida> {
    // Primero la metadata y las listas de tests de cada lado. Luego las corridas.
    let meta_i = leer_metadata(inicial).map_err(|m| format!("inicial: {m}"));
    let meta_r = leer_metadata(repo).map_err(|m| format!("repo: {m}"));
    let lista_i = leer_listas(inicial).map_err(|m| format!("inicial: {m}"));
    let lista_r = leer_listas(repo).map_err(|m| format!("repo: {m}"));
    let (tiempo, memoria) = medir_tests(inicial, repo, &lista_i, &lista_r);
    let (compilacion, tamano) = medir_compilacion(inicial, repo, &meta_i, &meta_r);
    vec![tiempo, memoria, compilacion, tamano]
}

/// Lee la última línea que empieza con `marca` en la salida. `None` si falta o no es número.
pub(crate) fn leer_marca(stdout: &str, marca: &str) -> Option<f64> {
    let valor = stdout.lines().rev().find_map(|linea| {
        let mut partes = linea.split_whitespace();
        (partes.next() == Some(marca)).then(|| partes.next())
    })?;
    valor?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oracle::ErrorOraculo;
    use crate::oracle::perfiles::herramientas::SalidaHerramienta;
    use serde_json::json;
    use std::cell::RefCell;

    const TESTS_OK: &str = "ROOT_NS 1500000000\nROOT_MEM 1000\n";
    const BUILD_OK: &str = "ROOT_NS 2000000000\nROOT_BUILD 0\nROOT_SIZE 500\n";

    /// Ejecutor falso. Responde según el texto del comando. Anota `"<lado> <tipo> <comando>"` en `log`.
    struct Falso<'a> {
        lado: &'static str,
        log: &'a RefCell<Vec<String>>,
        metadata: String,
        lista: String,
        tests: String,
        build: String,
    }

    impl EjecutorHerramienta for Falso<'_> {
        fn ejecutar(&self, cmd: &str) -> Result<SalidaHerramienta, ErrorOraculo> {
            let mut salida = SalidaHerramienta::default();
            let tipo = if cmd.contains("cargo metadata") {
                salida
                    .archivos
                    .insert("metadata.json".into(), self.metadata.clone());
                "metadata"
            } else if cmd.contains("junit.xml") {
                salida
                    .archivos
                    .insert("lista.xml".into(), self.lista.clone());
                "lista"
            } else if cmd.contains("cargo clean") {
                salida.stdout = self.build.clone();
                "build"
            } else if cmd.contains("--no-run") {
                salida.stdout = self.tests.clone();
                "tests"
            } else {
                panic!("comando no esperado: {cmd}");
            };
            self.log
                .borrow_mut()
                .push(format!("{} {tipo} {cmd}", self.lado));
            Ok(salida)
        }
    }

    /// Un ejecutor sano: un paquete `app` con un binario `app`, y todos sus tests pasan.
    fn sano<'a>(lado: &'static str, log: &'a RefCell<Vec<String>>, tests: &[&str]) -> Falso<'a> {
        Falso {
            lado,
            log,
            metadata: metadata("app", Some("app")),
            lista: lista(&tests.iter().map(|t| (*t, true)).collect::<Vec<_>>()),
            tests: TESTS_OK.into(),
            build: BUILD_OK.into(),
        }
    }

    /// `cargo metadata` de un paquete. `bin` es el nombre de su binario, si tiene.
    fn metadata(paquete: &str, bin: Option<&str>) -> String {
        let targets = match bin {
            Some(b) => json!([{ "kind": ["bin"], "name": b }]),
            None => json!([{ "kind": ["lib"], "name": paquete }]),
        };
        json!({ "packages": [{ "name": paquete, "targets": targets }] }).to_string()
    }

    /// Junit de nextest. Cada par es `(nombre, pasa)`. Un test que no pasa lleva `<failure>`.
    fn lista(casos: &[(&str, bool)]) -> String {
        let mut xml = String::from("<testsuites><testsuite name=\"x\">");
        for (nombre, pasa) in casos {
            if *pasa {
                xml.push_str(&format!("<testcase name=\"{nombre}\" classname=\"c\"/>"));
            } else {
                xml.push_str(&format!(
                    "<testcase name=\"{nombre}\" classname=\"c\"><failure message=\"x\"/></testcase>"
                ));
            }
        }
        xml.push_str("</testsuite></testsuites>");
        xml
    }

    fn cerca(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    fn por_medida(resultados: &[ResultadoMedida], medida: Medida) -> &ResultadoMedida {
        resultados
            .iter()
            .find(|r| r.medida == medida)
            .expect("medida presente")
    }

    /// Devuelve los lados de las llamadas de un tipo, en orden.
    fn lados_de(log: &[String], tipo: &str) -> Vec<String> {
        log.iter()
            .filter(|e| e.split(' ').nth(1) == Some(tipo))
            .map(|e| e.split(' ').next().unwrap_or_default().to_string())
            .collect()
    }

    #[test]
    fn comparar_razon_y_ruido_sin_regresion() {
        let c = comparar(&[1.0, 1.1, 1.0], &[1.05, 1.06, 1.08]).expect("comparable");
        assert!(cerca(c.razon, 1.05), "razon {}", c.razon);
        assert!(cerca(c.ruido, 0.10), "ruido {}", c.ruido);
        assert!(!c.regresion);
    }

    #[test]
    fn comparar_ruido_bajo_usa_el_piso_y_marca_regresion() {
        let c = comparar(&[1.0, 1.01, 1.0], &[1.6, 1.7, 1.65]).expect("comparable");
        assert!(cerca(c.razon, 1.6), "razon {}", c.razon);
        assert!(cerca(c.ruido, 0.02), "ruido {}", c.ruido);
        assert!(c.regresion);
    }

    #[test]
    fn comparar_vacios_o_minimo_no_positivo_dan_none() {
        assert_eq!(comparar(&[], &[1.0]), None);
        assert_eq!(comparar(&[1.0], &[]), None);
        assert_eq!(comparar(&[0.0, 1.0], &[1.0]), None);
    }

    #[test]
    fn medir_alterna_los_lados_en_tests_y_compilacion() {
        let log = RefCell::new(Vec::new());
        let inicial = sano("inicial", &log, &["a"]);
        let repo = sano("repo", &log, &["a"]);
        let resultados = medir(&inicial, &repo);
        assert_eq!(resultados.len(), 4);
        let entradas = log.borrow();
        let esperado: Vec<String> = (0..CORRIDAS)
            .flat_map(|_| ["inicial", "repo"])
            .map(String::from)
            .collect();
        assert_eq!(lados_de(&entradas, "tests"), esperado);
        assert_eq!(lados_de(&entradas, "build"), esperado);
    }

    #[test]
    fn test_que_pasa_solo_en_el_repo_entra_al_filtro_de_exclusion() {
        let log = RefCell::new(Vec::new());
        let inicial = Falso {
            lista: lista(&[("a", true), ("b", false)]),
            ..sano("inicial", &log, &[])
        };
        let repo = Falso {
            lista: lista(&[("a", true), ("b", true)]),
            ..sano("repo", &log, &[])
        };
        let resultados = medir(&inicial, &repo);
        let entradas = log.borrow();
        let corridas: Vec<&String> = entradas
            .iter()
            .filter(|e| e.split(' ').nth(1) == Some("tests"))
            .collect();
        assert!(!corridas.is_empty(), "debe haber corridas de tests");
        for corrida in corridas {
            assert!(corrida.contains("-E 'not (test(=b))'"), "{corrida}");
            assert!(!corrida.contains("test(=a)"), "{corrida}");
        }
        assert!(
            por_medida(&resultados, Medida::TiempoTests)
                .referencia
                .is_some()
        );
    }

    #[test]
    fn sin_memory_peak_memoria_no_tiene_valor_y_tiempo_si() {
        let log = RefCell::new(Vec::new());
        let sin_pico = "ROOT_NS 1500000000\nROOT_MEM \n";
        let inicial = Falso {
            tests: sin_pico.into(),
            ..sano("inicial", &log, &["a"])
        };
        let repo = Falso {
            tests: sin_pico.into(),
            ..sano("repo", &log, &["a"])
        };
        let resultados = medir(&inicial, &repo);
        let memoria = por_medida(&resultados, Medida::MemoriaPico);
        assert_eq!(memoria.motivo.as_deref(), Some("sin memory.peak"));
        assert_eq!(memoria.referencia, None);
        let tiempo = por_medida(&resultados, Medida::TiempoTests);
        assert!(tiempo.referencia.is_some());
        assert!(tiempo.motivo.is_none());
    }

    #[test]
    fn workspace_sin_binarios_deja_tamano_sin_valor() {
        let log = RefCell::new(Vec::new());
        let inicial = Falso {
            metadata: metadata("app", None),
            ..sano("inicial", &log, &["a"])
        };
        let repo = Falso {
            metadata: metadata("app", None),
            ..sano("repo", &log, &["a"])
        };
        let resultados = medir(&inicial, &repo);
        let tamano = por_medida(&resultados, Medida::TamanoBinario);
        assert_eq!(tamano.motivo.as_deref(), Some("sin binarios"));
        assert_eq!(tamano.referencia, None);
    }

    #[test]
    fn sin_tests_comunes_tiempo_y_memoria_no_tienen_valor() {
        let log = RefCell::new(Vec::new());
        let inicial = Falso {
            lista: lista(&[("a", true)]),
            ..sano("inicial", &log, &[])
        };
        let repo = Falso {
            lista: lista(&[("b", true)]),
            ..sano("repo", &log, &[])
        };
        let resultados = medir(&inicial, &repo);
        for medida in [Medida::TiempoTests, Medida::MemoriaPico] {
            let r = por_medida(&resultados, medida);
            assert_eq!(
                r.motivo.as_deref(),
                Some("ningún test pasa en los dos lados"),
                "{medida:?}"
            );
        }
        assert!(
            log.borrow()
                .iter()
                .all(|e| e.split(' ').nth(1) != Some("tests"))
        );
    }

    #[test]
    fn build_que_falla_deja_compilacion_y_tamano_sin_valor() {
        let log = RefCell::new(Vec::new());
        let roto = Falso {
            build: "ROOT_NS 1\nROOT_BUILD 2\nROOT_SIZE 9\n".into(),
            ..sano("repo", &log, &["a"])
        };
        let inicial = sano("inicial", &log, &["a"]);
        let resultados = medir(&inicial, &roto);
        for medida in [Medida::TiempoCompilacion, Medida::TamanoBinario] {
            let r = por_medida(&resultados, medida);
            assert_eq!(
                r.motivo.as_deref(),
                Some("la compilación falló"),
                "{medida:?}"
            );
        }
    }

    #[test]
    fn medida_id_usa_los_nombres_estables() {
        assert_eq!(Medida::TiempoTests.id(), "tiempo_tests");
        assert_eq!(Medida::MemoriaPico.id(), "memoria_pico");
        assert_eq!(Medida::TiempoCompilacion.id(), "tiempo_compilacion");
        assert_eq!(Medida::TamanoBinario.id(), "tamano_binario");
    }
}
