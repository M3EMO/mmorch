//! Eficiencia del repo evaluado contra su estado inicial (`bootstrap/`). Mide cuatro cosas:
//! tiempo de tests, pico de memoria, tiempo de compilación y tamaño de los binarios.
//! Cada lado corre en su propio ejecutor. `comparar_por_mediana` y `comparar` hacen la aritmética:
//! tiempo de tests y memoria usan la mediana; compilación y tamaño usan el mínimo.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use serde_json::Value;

use crate::borrado;
use crate::oracle::ErrorOraculo;
use crate::oracle::descarga::PRELUDIO;
use crate::oracle::perfiles::herramientas::{
    EjecutorHerramienta, SalidaHerramienta, carpeta_de_salida,
};

/// Corridas por lado. rustc-perf: tres corridas y se compara el mínimo.
pub const CORRIDAS: usize = 3;
/// Corridas de tests por lado. Los tests duran menos de medio segundo: con 3 corridas no se estima bien la
/// dispersión (en t1-t3 duraron de 0,25 a 0,42 s; un 2 % son 5 a 8 ms).
pub const CORRIDAS_TESTS: usize = 10;
/// Compilaciones del repo en un caso con referencia fijada. Una sola: en 12 casos de t1-t3 la compilación
/// varió como máximo 0,52 %, contra un piso de 2 %. Cada compilación medida tarda de 66 a 83 s.
pub const CORRIDAS_COMPILACION_DE_CASO: usize = 1;
/// Piso de ruido: 2 % de cambio. Perfherder usa este piso.
pub const PISO_RUIDO: f64 = 0.02;
/// Más de 1,5 veces da "no listo". Decisión P105 a del usuario.
pub const UMBRAL_NO_LISTO: f64 = 1.5;
/// Factor sobre la dispersión por cuartiles de tiempo de tests y memoria. Ver `MIN_CORRIDAS_CUARTILES`.
/// Base: 12 casos de t1 a t3 (Tarea 135), el exceso de la razón con medianas sobre 1 fue como máximo 1,04 veces la mayor dispersión por cuartiles (t3 caso 0: 0,024 contra 0,023). Con más datos, se revisa.
pub const FACTOR_RUIDO_CUARTILES: f64 = 2.0;
/// Corridas mínimas por lado para usar cuartiles. Con menos corridas, el lado usa (max-min)/min, como antes.
/// Base: 12 casos de t1 a t3 (Tarea 135), el exceso de la razón con medianas sobre 1 fue como máximo 1,04 veces la mayor dispersión por cuartiles (t3 caso 0: 0,024 contra 0,023). Con más datos, se revisa.
pub const MIN_CORRIDAS_CUARTILES: usize = 4;

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

/// Resultado de comparar dos lados. `razon` es repo sobre referencia. `ruido` es la mayor dispersión
/// de los dos lados, con el piso `PISO_RUIDO`.
#[derive(Debug, Clone, PartialEq)]
pub struct Comparacion {
    pub razon: f64,
    pub ruido: f64,
    pub regresion: bool,
}

/// Compara las corridas de cada lado. Usa el mínimo de cada lado. El ruido es la mayor dispersión de los
/// dos lados, con el piso `PISO_RUIDO`. `None` si no se puede comparar.
pub fn comparar(referencia: &[f64], repo: &[f64]) -> Option<Comparacion> {
    let min_ref = referencia.iter().copied().reduce(f64::min)?;
    let max_ref = referencia.iter().copied().reduce(f64::max)?;
    let min_repo = repo.iter().copied().reduce(f64::min)?;
    let max_repo = repo.iter().copied().reduce(f64::max)?;
    if min_ref <= 0.0 {
        return None;
    }
    let razon = min_repo / min_ref;
    // El ruido es la mayor dispersión de los dos lados, con el piso mínimo. Un repo con mínimo no
    // positivo cuenta dispersión cero: no se divide por cero.
    let disp_ref = (max_ref - min_ref) / min_ref;
    let disp_repo = if min_repo > 0.0 {
        (max_repo - min_repo) / min_repo
    } else {
        0.0
    };
    let ruido = disp_ref.max(disp_repo).max(PISO_RUIDO);
    Some(Comparacion {
        razon,
        ruido,
        regresion: razon > 1.0 + ruido,
    })
}

/// Como `comparar`, pero la referencia suma el `ancla`: una corrida del lado inicial, si hay.
pub fn comparar_con_ancla(base: &[f64], ancla: Option<f64>, repo: &[f64]) -> Option<Comparacion> {
    let referencia: Vec<f64> = base.iter().copied().chain(ancla).collect();
    comparar(&referencia, repo)
}

/// Mediana de los valores. Con n par, el promedio de los dos centrales. `None` si está vacío.
fn mediana(valores: &[f64]) -> Option<f64> {
    let mut orden = valores.to_vec();
    orden.sort_by(f64::total_cmp);
    let n = orden.len();
    match n {
        0 => None,
        _ if n % 2 == 1 => Some(orden[n / 2]),
        _ => Some((orden[n / 2 - 1] + orden[n / 2]) / 2.0),
    }
}

/// Valor en la posición `p * (n - 1)` de una serie ordenada, con interpolación lineal. `None` si está vacía.
fn en_posicion(orden: &[f64], p: f64) -> Option<f64> {
    let n = orden.len();
    if n == 0 {
        return None;
    }
    let pos = p * (n - 1) as f64;
    let bajo = pos.floor() as usize;
    let alto = pos.ceil() as usize;
    let frac = pos - bajo as f64;
    Some(orden[bajo] + (orden[alto] - orden[bajo]) * frac)
}

/// Q1 y Q3 de los valores, con interpolación lineal en la posición p * (n - 1). Ordena una copia.
/// `None` si está vacío.
fn cuartiles(valores: &[f64]) -> Option<(f64, f64)> {
    let mut orden = valores.to_vec();
    orden.sort_by(f64::total_cmp);
    Some((en_posicion(&orden, 0.25)?, en_posicion(&orden, 0.75)?))
}

/// Dispersión de un lado para el ruido. Con `MIN_CORRIDAS_CUARTILES` corridas o más, es
/// `FACTOR_RUIDO_CUARTILES` por (q3 - q1) / mediana. Con menos, es (max - min) / min, como antes.
/// Un valor no positivo cuenta dispersión cero: no se divide por cero.
fn dispersion_de_lado(valores: &[f64]) -> f64 {
    let Some(med) = mediana(valores) else {
        return 0.0;
    };
    if valores.len() < MIN_CORRIDAS_CUARTILES {
        let min = valores.iter().copied().fold(f64::INFINITY, f64::min);
        let max = valores.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        return if min > 0.0 { (max - min) / min } else { 0.0 };
    }
    match cuartiles(valores) {
        Some((q1, q3)) if med > 0.0 => FACTOR_RUIDO_CUARTILES * (q3 - q1) / med,
        _ => 0.0,
    }
}

/// Compara las medianas de cada lado. El ruido sale de `dispersion_de_lado`, con el piso `PISO_RUIDO`.
/// `None` si algún lado está vacío o la mediana de la referencia no es positiva.
pub fn comparar_por_mediana(referencia: &[f64], repo: &[f64]) -> Option<Comparacion> {
    let med_ref = mediana(referencia)?;
    let med_repo = mediana(repo)?;
    if med_ref <= 0.0 {
        return None;
    }
    let razon = med_repo / med_ref;
    let ruido = dispersion_de_lado(referencia)
        .max(dispersion_de_lado(repo))
        .max(PISO_RUIDO);
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
        corridas_referencia: vec![],
        corridas_repo: vec![],
    }
}

/// Serie de la corrida del lado inicial que ancla la compilación de la referencia. Sin ancla queda vacía y sin error.
/// Un ancla con error no hace fallar la medida: `combinar` usa solo la base.
struct Ancla {
    compilacion: Serie,
}

impl Ancla {
    fn vacia() -> Self {
        Ancla {
            compilacion: Ok(vec![]),
        }
    }
}

/// Valor de una serie de ancla. Una serie con error o sin valor no da ninguno.
fn valor_del_ancla(ancla: &Serie) -> Option<f64> {
    ancla.as_ref().ok()?.first().copied()
}

/// Un aviso si la serie de ancla de compilación tiene error. La referencia sigue con la serie de la base.
fn avisar_ancla(ancla: &Ancla, avisos: &mut Vec<String>) {
    if let Err(motivo) = &ancla.compilacion {
        avisos.push(format!(
            "eficiencia: el ancla de {} falló y la referencia sigue con la base: {motivo}",
            Medida::TiempoCompilacion.id()
        ));
    }
}

/// Tiempo de tests y memoria se comparan por mediana. Compilación y tamaño, por mínimo.
fn por_mediana(medida: Medida) -> bool {
    matches!(medida, Medida::TiempoTests | Medida::MemoriaPico)
}

/// Mínimo de los valores. `None` si está vacío.
fn minimo(valores: &[f64]) -> Option<f64> {
    valores.iter().copied().reduce(f64::min)
}

/// Compara las dos series de una medida. `ancla` suma a la referencia, si hay. Si un lado falla, la medida no tiene valor.
fn combinar(
    medida: Medida,
    referencia: &Serie,
    ancla: Option<f64>,
    repo: &Serie,
) -> ResultadoMedida {
    match (referencia, repo) {
        (Ok(r), Ok(p)) => {
            let completa: Vec<f64> = r.iter().copied().chain(ancla).collect();
            let (comparacion, valor_referencia, valor_repo) = if por_mediana(medida) {
                (comparar_por_mediana(r, p), mediana(r), mediana(p))
            } else {
                (
                    comparar_con_ancla(r, ancla, p),
                    minimo(&completa),
                    minimo(p),
                )
            };
            match comparacion {
                Some(comparacion) => ResultadoMedida {
                    medida,
                    referencia: valor_referencia,
                    repo: valor_repo,
                    comparacion: Some(comparacion),
                    motivo: None,
                    corridas_referencia: completa,
                    corridas_repo: p.clone(),
                },
                None => sin_valor(medida, "no se puede comparar".into()),
            }
        }
        _ => {
            let motivos = [referencia, repo]
                .into_iter()
                .filter_map(|s| s.as_ref().err().cloned());
            sin_valor(medida, unir_motivos(motivos))
        }
    }
}

/// Un lado de la medición: su ejecutor y su carpeta de `/salida`. Todas sus llamadas usan esa carpeta.
struct Lado<'a> {
    nombre: &'static str,
    ej: &'a dyn EjecutorHerramienta,
    salida: PathBuf,
}

impl Lado<'_> {
    fn ejecutar(&self, cmd: &str) -> Result<SalidaHerramienta, ErrorOraculo> {
        self.ej.ejecutar_en(cmd, &self.salida)
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
/// Motivo de los tests cuando el filtro del caso no es el de la referencia: la serie guardada midió otros tests.
const MOTIVO_OTROS_TESTS: &str = "los tests comunes del caso difieren de los de la base";

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

/// Ejecuta una corrida de tests medida con el filtro `filtro` (vacío o ` -E '...'`). No compila: los tests
/// ya están compilados por `cmd_compilar_tests`. Lee `ROOT_NS` (nanosegundos) y `ROOT_MEM` (bytes, vacío si no
/// hay `memory.peak`). Las líneas `Compiling` de cargo salen en stdout: si hay una, la corrida recompiló.
pub(crate) fn cmd_tests(filtro: &str) -> String {
    format!(
        "{PRELUDIO}cd /repo && export CARGO_TARGET_DIR=/salida/target && a=$(date +%s%N); cargo nextest run --offline --profile ci --tool-config-file root:/etc/root-oraculo/nextest.toml{filtro} 2>&1 | grep -E '^ *Compiling'; b=$(date +%s%N); echo \"ROOT_NS $((b-a))\"; echo \"ROOT_MEM $(cat /sys/fs/cgroup/memory.peak 2>/dev/null)\""
    )
}

/// Compila los tests antes de las corridas medidas. Lee `ROOT_COMPILAR` (código de salida de nextest).
pub(crate) fn cmd_compilar_tests() -> String {
    format!(
        "{PRELUDIO}cd /repo && export CARGO_TARGET_DIR=/salida/target && cargo nextest run --no-run --offline --profile ci --tool-config-file root:/etc/root-oraculo/nextest.toml >/dev/null 2>&1; echo \"ROOT_COMPILAR $?\""
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

fn leer_metadata(lado: &Lado) -> Result<Metadatos, String> {
    let salida = lado.ejecutar(&cmd_metadata()).map_err(|e| e.to_string())?;
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

fn leer_listas(lado: &Lado) -> Result<Listas, String> {
    let salida = lado.ejecutar(&cmd_lista()).map_err(|e| e.to_string())?;
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

/// Motivo de una corrida medida que compiló algo: su reloj y su pico de memoria incluyen rustc.
const MOTIVO_RECOMPILO: &str = "la corrida medida recompiló";

/// Dice si la salida de una corrida trae una línea `Compiling` de cargo.
fn recompilo(stdout: &str) -> bool {
    stdout
        .lines()
        .any(|l| l.trim_start().starts_with("Compiling"))
}

/// Compila los tests de un lado. Un error o un código distinto de cero dan el motivo.
fn compilar_tests(lado: &Lado) -> Result<(), String> {
    let nombre = lado.nombre;
    let salida = lado
        .ejecutar(&cmd_compilar_tests())
        .map_err(|e| format!("{nombre}: {e}"))?;
    match leer_marca(&salida.stdout, "ROOT_COMPILAR") {
        Some(0.0) => Ok(()),
        Some(_) => Err(format!("{nombre}: la compilación de los tests falló")),
        None => Err(format!(
            "{nombre}: salida ilegible de la compilación de tests"
        )),
    }
}

/// Una corrida medida de tests. Anota su tiempo y su memoria. Una corrida que recompila no vale.
fn correr_tests(lado: &Lado, filtro: &str, tiempo: &mut Serie, memoria: &mut Serie) {
    let nombre = lado.nombre;
    match lado.ejecutar(&cmd_tests(filtro)) {
        Err(e) => {
            let motivo = format!("{nombre}: {e}");
            anotar(tiempo, Err(motivo.clone()));
            anotar(memoria, Err(motivo));
        }
        Ok(salida) if recompilo(&salida.stdout) => {
            anotar(tiempo, Err(MOTIVO_RECOMPILO.into()));
            anotar(memoria, Err(MOTIVO_RECOMPILO.into()));
        }
        Ok(salida) => {
            let ns = leer_marca(&salida.stdout, "ROOT_NS").map(|ns| ns / 1e9);
            anotar(tiempo, ns.ok_or_else(|| format!("{nombre}: sin ROOT_NS")));
            let mem = leer_marca(&salida.stdout, "ROOT_MEM");
            anotar(memoria, mem.ok_or_else(|| "sin memory.peak".to_string()));
        }
    }
}

/// Mide tiempo y memoria de los tests. Compila los tests de cada lado y luego corre los dos lados,
/// alternados, `CORRIDAS_TESTS` veces. En cada par corre primero el lado `orden[0]`. Un lado que no compila
/// no corre. Con `filtro` con error, los dos lados quedan con ese error.
fn medir_tests(
    lados: [&Lado; 2],
    orden: [usize; 2],
    filtro: &Result<String, String>,
    tiempo: &mut [Serie; 2],
    memoria: &mut [Serie; 2],
) {
    let filtro = match filtro {
        Ok(filtro) => filtro,
        Err(motivo) => {
            *tiempo = [Err(motivo.clone()), Err(motivo.clone())];
            *memoria = [Err(motivo.clone()), Err(motivo.clone())];
            return;
        }
    };
    for (k, lado) in lados.into_iter().enumerate() {
        if let Err(motivo) = compilar_tests(lado) {
            anotar(&mut tiempo[k], Err(motivo.clone()));
            anotar(&mut memoria[k], Err(motivo));
        }
    }
    for _ in 0..CORRIDAS_TESTS {
        for k in orden {
            if tiempo[k].is_err() && memoria[k].is_err() {
                continue;
            }
            correr_tests(lados[k], filtro, &mut tiempo[k], &mut memoria[k]);
        }
    }
}

/// Una compilación medida. Anota su reloj en `compilacion`. Si `con_tamano`, anota también el tamaño de
/// sus binarios en `tamano`. Un fallo de la compilación llega a las dos series.
fn correr_build(
    lado: &Lado,
    comando: &str,
    compilacion: &mut Serie,
    tamano: &mut Serie,
    con_tamano: bool,
) {
    let nombre = lado.nombre;
    match lado.ejecutar(comando) {
        Err(e) => {
            let motivo = format!("{nombre}: {e}");
            anotar(compilacion, Err(motivo.clone()));
            anotar(tamano, Err(motivo));
        }
        Ok(salida) => match leer_marca(&salida.stdout, "ROOT_BUILD") {
            Some(0.0) => {
                let ns = leer_marca(&salida.stdout, "ROOT_NS").map(|ns| ns / 1e9);
                anotar(
                    compilacion,
                    ns.ok_or_else(|| format!("{nombre}: sin ROOT_NS")),
                );
                if con_tamano {
                    let size = leer_marca(&salida.stdout, "ROOT_SIZE");
                    anotar(tamano, size.ok_or_else(|| "sin tamaño".to_string()));
                }
            }
            Some(_) => {
                anotar(compilacion, Err("la compilación falló".into()));
                anotar(tamano, Err("la compilación falló".into()));
            }
            None => {
                anotar(compilacion, Err("salida ilegible del build".into()));
                anotar(tamano, Err("salida ilegible del build".into()));
            }
        },
    }
}

/// Mide la compilación y el tamaño de los binarios. Usa los paquetes y binarios del lado `repo`.
/// Los dos lados tienen los mismos nombres de paquete, así que el comando sirve para ambos.
/// `lados[k]` `None` como en `medir_tests`: ese lado no corre y su serie ya viene en `compilacion[k]`.
/// Corre `corridas` compilaciones por lado. El tamaño se mide solo en la corrida 0 de cada lado.
/// Si `ancla_lado` es `Some`, corre una compilación tras la primera corrida de `lados`. Su reloj va a
/// `ancla.compilacion`. Su tamaño no se mide: el tamaño sale solo de `lados`.
fn medir_compilacion(
    lados: [Option<&Lado>; 2],
    ancla_lado: Option<&Lado>,
    corridas: usize,
    [meta_i, meta_r]: [&Result<Metadatos, String>; 2],
    compilacion: &mut [Serie; 2],
    tamano: &mut [Serie; 2],
    ancla: &mut Ancla,
) {
    let (Ok(_), Ok(meta)) = (meta_i, meta_r) else {
        let motivo = unir_motivos(
            [meta_i.as_ref().err(), meta_r.as_ref().err()]
                .into_iter()
                .flatten()
                .cloned(),
        );
        *compilacion = [Err(motivo.clone()), Err(motivo.clone())];
        *tamano = [Err(motivo.clone()), Err(motivo)];
        return;
    };
    let sin = |motivo: &str| -> [Serie; 2] { [Err(motivo.into()), Err(motivo.into())] };
    if meta.paquetes.is_empty() {
        *compilacion = sin("sin paquetes");
        *tamano = sin(if meta.binarios.is_empty() {
            "sin binarios"
        } else {
            "sin paquetes"
        });
    } else {
        if meta.binarios.is_empty() {
            *tamano = sin("sin binarios");
        }
        let comando = cmd_compilacion(&meta.paquetes, &meta.binarios);
        for corrida in 0..corridas {
            for (k, lado) in lados.into_iter().enumerate() {
                let Some(lado) = lado else { continue };
                if compilacion[k].is_err() {
                    continue;
                }
                correr_build(
                    lado,
                    &comando,
                    &mut compilacion[k],
                    &mut tamano[k],
                    corrida == 0,
                );
            }
            if corrida == 0
                && let Some(lado) = ancla_lado
            {
                // El ancla no mide tamaño: su serie de tamaño se descarta.
                let mut sin_tamano = Ok(vec![]);
                correr_build(
                    lado,
                    &comando,
                    &mut ancla.compilacion,
                    &mut sin_tamano,
                    false,
                );
            }
        }
    }
}

/// Une las series de los dos lados en las cuatro medidas, en orden de `Medida`. El ancla suma solo a la
/// referencia de compilación. Ver `combinar` para la estadística.
fn combinar_medidas(
    tiempo: &[Serie; 2],
    memoria: &[Serie; 2],
    compilacion: &[Serie; 2],
    tamano: &[Serie; 2],
    ancla: &Ancla,
) -> Vec<ResultadoMedida> {
    vec![
        combinar(Medida::TiempoTests, &tiempo[0], None, &tiempo[1]),
        combinar(Medida::MemoriaPico, &memoria[0], None, &memoria[1]),
        combinar(
            Medida::TiempoCompilacion,
            &compilacion[0],
            valor_del_ancla(&ancla.compilacion),
            &compilacion[1],
        ),
        combinar(Medida::TamanoBinario, &tamano[0], None, &tamano[1]),
    ]
}

/// Una medida con sus dos valores comparados, o el motivo por el que no tiene valor.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultadoMedida {
    pub medida: Medida,
    /// Valor comparado del lado inicial: la mediana en tiempo de tests y memoria, el mínimo en las otras dos.
    pub referencia: Option<f64>,
    /// Valor comparado del repo: la mediana en tiempo de tests y memoria, el mínimo en las otras dos.
    pub repo: Option<f64>,
    pub comparacion: Option<Comparacion>,
    pub motivo: Option<String>,
    /// Cada corrida del lado inicial, en orden. El ancla va al final. Vacía si no hay valor.
    pub corridas_referencia: Vec<f64>,
    /// Cada corrida del repo, en orden. Vacía si no hay valor.
    pub corridas_repo: Vec<f64>,
}

/// Lo que midió el lado inicial de una medición. Una calibración la mide una vez y sus casos la reusan.
/// `filtro` es el resultado de `preparar_filtro` de ese lado. Las series de compilación y tamaño son las
/// del lado inicial. El tiempo y la memoria no se guardan: cada caso los mide en vivo.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Referencia {
    meta: Result<Metadatos, String>,
    lista: Result<Listas, String>,
    filtro: Result<String, String>,
    compilacion: Serie,
    tamano: Serie,
}

thread_local! {
    /// Referencia que midió la última medición de dos lados de este hilo. La toma `tomar_referencia`.
    static ULTIMA_REFERENCIA: RefCell<Option<Referencia>> = const { RefCell::new(None) };
    /// Referencia fijada que usa `medir_lados` en este hilo. `None`: mide los dos lados.
    static REFERENCIA_FIJA: RefCell<Option<Referencia>> = const { RefCell::new(None) };
    /// Carpeta fija de Eficiencia que usa `medir_con` en este hilo. `None`: `medir` crea y borra la suya.
    static CARPETA_DE_EFICIENCIA: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

/// Corre `f` con `carpeta` como la carpeta de trabajo fija de Eficiencia. Sin carpeta, `medir` crea y borra la suya.
/// Al salir, también ante pánico, vuelve el valor anterior.
pub(crate) fn con_carpeta_de_eficiencia<T>(carpeta: Option<&Path>, f: impl FnOnce() -> T) -> T {
    /// Devuelve la carpeta anterior al salir del ámbito.
    struct Restaura(Option<PathBuf>);
    impl Drop for Restaura {
        fn drop(&mut self) {
            let anterior = self.0.take();
            CARPETA_DE_EFICIENCIA.with(|c| *c.borrow_mut() = anterior);
        }
    }
    let anterior = CARPETA_DE_EFICIENCIA.with(|c| c.replace(carpeta.map(Path::to_path_buf)));
    let _restaura = Restaura(anterior);
    f()
}

/// Carpeta fija de Eficiencia que ve este hilo. Solo para los tests.
#[cfg(test)]
fn carpeta_de_eficiencia_actual() -> Option<PathBuf> {
    CARPETA_DE_EFICIENCIA.with(|c| c.borrow().clone())
}

/// Saca la referencia que midió la última medición de dos lados de este hilo. Deja el valor vacío.
pub(crate) fn tomar_referencia() -> Option<Referencia> {
    ULTIMA_REFERENCIA.with(|u| u.borrow_mut().take())
}

/// Filtro `-E` de los tests comunes de la última medición de dos lados de este hilo. Lo lee sin vaciar
/// la referencia: `tomar_referencia` la sigue necesitando. `None` si no hubo esa medición.
pub(crate) fn filtro_de_tests_comunes() -> Option<Result<String, String>> {
    ULTIMA_REFERENCIA.with(|u| u.borrow().as_ref().map(|r| r.filtro.clone()))
}

/// Corre `f` con `referencia` como el lado inicial. Sin referencia, la medición mide los dos lados.
/// Al salir, también ante pánico, vuelve la referencia anterior.
pub(crate) fn con_referencia<T>(referencia: Option<Referencia>, f: impl FnOnce() -> T) -> T {
    /// Devuelve la referencia anterior al salir del ámbito.
    struct Restaura(Option<Referencia>);
    impl Drop for Restaura {
        fn drop(&mut self) {
            let anterior = self.0.take();
            REFERENCIA_FIJA.with(|r| *r.borrow_mut() = anterior);
        }
    }
    let anterior = REFERENCIA_FIJA.with(|r| r.replace(referencia));
    let _restaura = Restaura(anterior);
    f()
}

/// Referencia fijada que ve este hilo. Solo para los tests.
#[cfg(test)]
fn referencia_fijada_actual() -> Option<Referencia> {
    REFERENCIA_FIJA.with(|r| r.borrow().clone())
}

/// Mide el repo contra el estado inicial. Siempre devuelve cuatro resultados, en orden de `Medida`.
/// `tmp` es la carpeta padre (`home/tmp`): `medir` crea ahí su carpeta de trabajo y la borra al terminar,
/// también con error. Cada lado tiene su subcarpeta: todas sus llamadas usan la misma `/salida`.
/// Con carpeta fija (`con_carpeta_de_eficiencia`), usa esa carpeta y no crea ni borra la suya.
/// Si el borrado de esa carpeta falla, `avisos` recibe un aviso con la ruta y el error. Las medidas no cambian.
/// Con referencia fijada, un ancla de compilación con error suma un aviso. La medida sigue con la base.
pub fn medir(
    inicial: &dyn EjecutorHerramienta,
    repo: &dyn EjecutorHerramienta,
    tmp: &Path,
    avisos: &mut Vec<String>,
) -> Vec<ResultadoMedida> {
    medir_con(inicial, repo, tmp, &borrado::borrar_dir_dentro, avisos)
}

/// `medir` con el borrado por parámetro. Los tests inyectan un borrado que falla para ver el aviso.
fn medir_con(
    inicial: &dyn EjecutorHerramienta,
    repo: &dyn EjecutorHerramienta,
    tmp: &Path,
    borrar: &dyn Fn(&Path, &Path) -> std::io::Result<()>,
    avisos: &mut Vec<String>,
) -> Vec<ResultadoMedida> {
    let fija = CARPETA_DE_EFICIENCIA.with(|c| c.borrow().clone());
    let creada = match &fija {
        Some(carpeta) => crear_subcarpetas_fijas(carpeta),
        None => crear_trabajo(tmp, borrar, avisos),
    };
    let raiz = match creada {
        Ok(raiz) => raiz,
        Err(e) => {
            let motivo = format!("carpeta de trabajo: {e}");
            let medidas = [
                Medida::TiempoTests,
                Medida::MemoriaPico,
                Medida::TiempoCompilacion,
                Medida::TamanoBinario,
            ];
            return Vec::from(medidas.map(|m| sin_valor(m, motivo.clone())));
        }
    };
    let lado_i = Lado {
        nombre: "inicial",
        ej: inicial,
        salida: raiz.join("inicial"),
    };
    let lado_r = Lado {
        nombre: "repo",
        ej: repo,
        salida: raiz.join("repo"),
    };
    let resultados = medir_lados(&lado_i, &lado_r, avisos);
    // La carpeta fija es de quien la fijó: no se borra aquí. Un fallo de borrado no cambia las medidas: la
    // carpeta queda y el siguiente `medir` la ignora.
    if fija.is_none()
        && let Err(e) = borrar(tmp, &raiz)
    {
        avisos.push(aviso_de_borrado(&raiz, &e));
    }
    resultados
}

/// Crea las subcarpetas `inicial` y `repo` dentro de la carpeta fija. Las que ya existen se reusan.
fn crear_subcarpetas_fijas(carpeta: &Path) -> std::io::Result<PathBuf> {
    ["inicial", "repo"]
        .iter()
        .try_for_each(|lado| fs::create_dir_all(carpeta.join(lado)))?;
    Ok(carpeta.to_path_buf())
}

/// Aviso del veredicto cuando no se puede borrar la carpeta de trabajo. Dice la ruta y el error.
fn aviso_de_borrado(raiz: &Path, e: &std::io::Error) -> String {
    format!(
        "eficiencia: carpeta de trabajo sin borrar {}: {e}",
        raiz.display()
    )
}

/// Las cuatro medidas de dos lados. Con referencia fijada, solo mide el repo. Si no, mide los dos lados y
/// guarda lo que midió el inicial para `tomar_referencia`.
fn medir_lados(inicial: &Lado, repo: &Lado, avisos: &mut Vec<String>) -> Vec<ResultadoMedida> {
    let fijada = REFERENCIA_FIJA.with(|r| r.borrow().clone());
    match fijada {
        Some(referencia) => medir_contra_referencia(&referencia, inicial, repo, avisos),
        None => {
            let (resultados, referencia) = medir_los_dos_lados(inicial, repo);
            ULTIMA_REFERENCIA.with(|u| *u.borrow_mut() = Some(referencia));
            resultados
        }
    }
}

/// Mide la metadata, las listas y las corridas de los dos lados. Devuelve las cuatro medidas y lo que
/// midió el lado inicial. Primero la metadata y las listas de cada lado. Luego las corridas.
fn medir_los_dos_lados(inicial: &Lado, repo: &Lado) -> (Vec<ResultadoMedida>, Referencia) {
    let meta_i = leer_metadata(inicial).map_err(|m| format!("inicial: {m}"));
    let meta_r = leer_metadata(repo).map_err(|m| format!("repo: {m}"));
    let lista_i = leer_listas(inicial).map_err(|m| format!("inicial: {m}"));
    let lista_r = leer_listas(repo).map_err(|m| format!("repo: {m}"));
    let filtro = preparar_filtro(&lista_i, &lista_r);
    let lados = [Some(inicial), Some(repo)];
    let mut tiempo: [Serie; 2] = [Ok(vec![]), Ok(vec![])];
    let mut memoria: [Serie; 2] = [Ok(vec![]), Ok(vec![])];
    let mut compilacion: [Serie; 2] = [Ok(vec![]), Ok(vec![])];
    let mut tamano: [Serie; 2] = [Ok(vec![]), Ok(vec![])];
    let mut ancla = Ancla::vacia();
    medir_tests([inicial, repo], [0, 1], &filtro, &mut tiempo, &mut memoria);
    medir_compilacion(
        lados,
        None,
        CORRIDAS,
        [&meta_i, &meta_r],
        &mut compilacion,
        &mut tamano,
        &mut ancla,
    );
    let referencia = Referencia {
        meta: meta_i,
        lista: lista_i,
        filtro,
        compilacion: compilacion[0].clone(),
        tamano: tamano[0].clone(),
    };
    let resultados = combinar_medidas(&tiempo, &memoria, &compilacion, &tamano, &ancla);
    (resultados, referencia)
}

/// Mide el repo y el lado inicial en vivo, en tests. Tiempo y memoria salen de esas dos series, alternadas.
/// Compilación y tamaño usan la serie de la base. El lado inicial compila una vez más: su corrida ancla la
/// compilación. El repo compila `CORRIDAS_COMPILACION_DE_CASO` veces. Si el filtro del repo no es el de la
/// referencia, los tests no tienen valor.
fn medir_contra_referencia(
    referencia: &Referencia,
    inicial: &Lado,
    repo: &Lado,
    avisos: &mut Vec<String>,
) -> Vec<ResultadoMedida> {
    let meta_r = leer_metadata(repo).map_err(|m| format!("repo: {m}"));
    let lista_r = leer_listas(repo).map_err(|m| format!("repo: {m}"));
    let filtro = preparar_filtro(&referencia.lista, &lista_r).and_then(|f| {
        if referencia
            .filtro
            .as_ref()
            .is_ok_and(|guardado| *guardado == f)
        {
            Ok(f)
        } else {
            Err(MOTIVO_OTROS_TESTS.into())
        }
    });
    let mut tiempo: [Serie; 2] = [Ok(vec![]), Ok(vec![])];
    let mut memoria: [Serie; 2] = [Ok(vec![]), Ok(vec![])];
    let mut compilacion: [Serie; 2] = [referencia.compilacion.clone(), Ok(vec![])];
    let mut tamano: [Serie; 2] = [referencia.tamano.clone(), Ok(vec![])];
    let mut ancla = Ancla::vacia();
    // El orden [1, 0] hace que el repo corra primero en cada par de tests.
    medir_tests([inicial, repo], [1, 0], &filtro, &mut tiempo, &mut memoria);
    medir_compilacion(
        [None, Some(repo)],
        Some(inicial),
        CORRIDAS_COMPILACION_DE_CASO,
        [&referencia.meta, &meta_r],
        &mut compilacion,
        &mut tamano,
        &mut ancla,
    );
    avisar_ancla(&ancla, avisos);
    combinar_medidas(&tiempo, &memoria, &compilacion, &tamano, &ancla)
}

/// Crea la carpeta de trabajo dentro de `tmp` y una subcarpeta por lado. Si falla a medias, borra lo creado.
fn crear_trabajo(
    tmp: &Path,
    borrar: &dyn Fn(&Path, &Path) -> std::io::Result<()>,
    avisos: &mut Vec<String>,
) -> std::io::Result<PathBuf> {
    let raiz = carpeta_de_salida(tmp)?.keep();
    let subcarpetas = ["inicial", "repo"]
        .iter()
        .try_for_each(|lado| fs::create_dir(raiz.join(lado)));
    if let Err(e) = subcarpetas {
        if let Err(e_borrado) = borrar(tmp, &raiz) {
            avisos.push(aviso_de_borrado(&raiz, &e_borrado));
        }
        return Err(e);
    }
    Ok(raiz)
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
        compilar: String,
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
                salida.stdout = self.compilar.clone();
                "compilar_tests"
            } else if cmd.contains("ROOT_NS") {
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
            compilar: "ROOT_COMPILAR 0\n".into(),
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

    /// Devuelve los tipos de las llamadas de un lado, en orden.
    fn tipos_de(log: &[String], lado: &str) -> Vec<String> {
        log.iter()
            .filter(|e| e.split(' ').next() == Some(lado))
            .map(|e| e.split(' ').nth(1).unwrap_or_default().to_string())
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
        let c = comparar(&[1.0, 1.01, 1.0], &[1.6, 1.61, 1.6]).expect("comparable");
        assert!(cerca(c.razon, 1.6), "razon {}", c.razon);
        assert!(cerca(c.ruido, 0.02), "ruido {}", c.ruido);
        assert!(c.regresion);
    }

    #[test]
    fn comparar_ruido_del_repo_mayor_que_el_de_la_referencia_gana() {
        let referencia = [
            0.400, 0.401, 0.402, 0.403, 0.404, 0.400, 0.401, 0.402, 0.403, 0.404,
        ];
        let repo = [
            0.409, 0.410, 0.411, 0.412, 0.419, 0.409, 0.410, 0.411, 0.412, 0.419,
        ];
        let c = comparar(&referencia, &repo).expect("comparable");
        assert!((c.razon - 1.0225).abs() < 1e-4, "razon {}", c.razon);
        assert!((c.ruido - 0.02445).abs() < 1e-4, "ruido {}", c.ruido);
        assert!(!c.regresion, "{c:?}");
    }

    #[test]
    fn comparar_lados_estables_y_50_por_ciento_mas_lento_es_regresion() {
        let referencia = [
            1.000, 1.005, 1.002, 1.001, 1.004, 1.000, 1.003, 1.002, 1.001, 1.004,
        ];
        let repo = [
            1.500, 1.505, 1.502, 1.501, 1.504, 1.500, 1.503, 1.502, 1.501, 1.504,
        ];
        let c = comparar(&referencia, &repo).expect("comparable");
        assert!(c.regresion, "{c:?}");
    }

    #[test]
    fn comparar_vacios_o_minimo_no_positivo_dan_none() {
        assert_eq!(comparar(&[], &[1.0]), None);
        assert_eq!(comparar(&[1.0], &[]), None);
        assert_eq!(comparar(&[0.0, 1.0], &[1.0]), None);
    }

    #[test]
    fn ancla_sube_el_ruido_y_quita_la_falsa_regresion() {
        let base = [1.00, 1.01, 1.00];
        let repo = [1.04, 1.05, 1.04];
        let sin = comparar(&base, &repo).expect("comparable");
        assert!(cerca(sin.razon, 1.04), "razon {}", sin.razon);
        assert!(cerca(sin.ruido, 0.02), "ruido {}", sin.ruido);
        assert!(sin.regresion);
        let con = comparar_con_ancla(&base, Some(1.05), &repo).expect("comparable");
        assert!(cerca(con.ruido, 0.05), "ruido {}", con.ruido);
        assert!(!con.regresion, "{con:?}");
    }

    #[test]
    fn ancla_no_tapa_una_regresion_real() {
        let con = comparar_con_ancla(&[1.00, 1.01, 1.00], Some(1.00), &[2.40, 2.41, 2.42])
            .expect("comparable");
        assert!(con.regresion, "{con:?}");
        assert!(cerca(con.razon, 2.4), "razon {}", con.razon);
    }

    #[test]
    fn medir_alterna_los_lados_en_tests_y_compilacion() {
        let log = RefCell::new(Vec::new());
        let inicial = sano("inicial", &log, &["a"]);
        let repo = sano("repo", &log, &["a"]);
        let resultados = medir_de_prueba(&inicial, &repo);
        assert_eq!(resultados.len(), 4);
        let entradas = log.borrow();
        let esperado_tests: Vec<String> = (0..CORRIDAS_TESTS)
            .flat_map(|_| ["inicial", "repo"])
            .map(String::from)
            .collect();
        let esperado_build: Vec<String> = (0..CORRIDAS)
            .flat_map(|_| ["inicial", "repo"])
            .map(String::from)
            .collect();
        assert_eq!(lados_de(&entradas, "tests"), esperado_tests);
        assert_eq!(lados_de(&entradas, "build"), esperado_build);
    }

    #[test]
    fn medir_corre_los_tests_de_cada_lado_corridas_tests_veces() {
        let log = RefCell::new(Vec::new());
        let inicial = sano("inicial", &log, &["a"]);
        let repo = sano("repo", &log, &["a"]);
        medir_de_prueba(&inicial, &repo);
        let entradas = log.borrow();
        for lado in ["inicial", "repo"] {
            let tipos = tipos_de(&entradas, lado);
            let cuenta = |tipo: &str| tipos.iter().filter(|t| *t == tipo).count();
            assert_eq!(cuenta("tests"), CORRIDAS_TESTS, "{lado}: {tipos:?}");
            assert_eq!(cuenta("build"), CORRIDAS, "{lado}: {tipos:?}");
        }
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
        let resultados = medir_de_prueba(&inicial, &repo);
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
        let resultados = medir_de_prueba(&inicial, &repo);
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
        let resultados = medir_de_prueba(&inicial, &repo);
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
        let resultados = medir_de_prueba(&inicial, &repo);
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
        let resultados = medir_de_prueba(&inicial, &roto);
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

    /// `medir` con una carpeta temporal propia: los tests que no miran la carpeta no la nombran.
    fn medir_de_prueba(
        inicial: &dyn EjecutorHerramienta,
        repo: &dyn EjecutorHerramienta,
    ) -> Vec<ResultadoMedida> {
        let tmp = tempfile::tempdir().expect("temp dir");
        medir(inicial, repo, tmp.path(), &mut Vec::new())
    }

    /// Ejecutor que anota la carpeta de `/salida` de cada llamada en `ejecutar_en` y responde con `Falso`.
    /// Con `error_en = Some(n)`, la llamada número `n` (desde cero) falla con error de infraestructura.
    /// `ejecutar` entra en pánico: toda llamada de la medición debe llevar carpeta.
    struct ConCarpeta<'a> {
        falso: Falso<'a>,
        carpetas: RefCell<Vec<PathBuf>>,
        error_en: Option<usize>,
    }

    impl EjecutorHerramienta for ConCarpeta<'_> {
        fn ejecutar(&self, cmd: &str) -> Result<SalidaHerramienta, ErrorOraculo> {
            panic!("llamada sin carpeta de /salida: {cmd}");
        }

        fn ejecutar_en(&self, cmd: &str, salida: &Path) -> Result<SalidaHerramienta, ErrorOraculo> {
            let n = self.carpetas.borrow().len();
            self.carpetas.borrow_mut().push(salida.to_path_buf());
            if self.error_en == Some(n) {
                return Err(ErrorOraculo::Infra("corte de prueba".into()));
            }
            self.falso.ejecutar(cmd)
        }
    }

    /// Un lado con carpetas anotadas. Responde como un lado sano de un test.
    fn con_carpeta<'a>(
        lado: &'static str,
        log: &'a RefCell<Vec<String>>,
        error_en: Option<usize>,
    ) -> ConCarpeta<'a> {
        ConCarpeta {
            falso: sano(lado, log, &["a"]),
            carpetas: RefCell::new(Vec::new()),
            error_en,
        }
    }

    #[test]
    fn medir_todas_las_llamadas_de_un_lado_usan_la_misma_carpeta() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let log = RefCell::new(Vec::new());
        let inicial = con_carpeta("inicial", &log, None);
        let repo = con_carpeta("repo", &log, None);
        medir(&inicial, &repo, tmp.path(), &mut Vec::new());
        let carpetas_i = inicial.carpetas.borrow();
        assert!(carpetas_i.len() > CORRIDAS, "{carpetas_i:?}");
        assert!(
            carpetas_i.iter().all(|c| *c == carpetas_i[0]),
            "{carpetas_i:?}"
        );
    }

    #[test]
    fn medir_los_dos_lados_usan_carpetas_distintas() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let log = RefCell::new(Vec::new());
        let inicial = con_carpeta("inicial", &log, None);
        let repo = con_carpeta("repo", &log, None);
        medir(&inicial, &repo, tmp.path(), &mut Vec::new());
        let carpetas_i = inicial.carpetas.borrow().clone();
        let carpetas_r = repo.carpetas.borrow().clone();
        assert_ne!(carpetas_i[0], carpetas_r[0]);
        assert!(carpetas_r.iter().all(|c| !carpetas_i.contains(c)));
    }

    #[test]
    fn medir_borra_su_carpeta_de_trabajo_tambien_con_error() {
        for error_en in [None, Some(0), Some(2), Some(5)] {
            let tmp = tempfile::tempdir().expect("temp dir");
            let log = RefCell::new(Vec::new());
            let inicial = con_carpeta("inicial", &log, error_en);
            let repo = con_carpeta("repo", &log, None);
            medir(&inicial, &repo, tmp.path(), &mut Vec::new());
            let usadas: Vec<PathBuf> = inicial
                .carpetas
                .borrow()
                .iter()
                .chain(repo.carpetas.borrow().iter())
                .cloned()
                .collect();
            assert!(!usadas.is_empty(), "error_en {error_en:?}");
            for carpeta in &usadas {
                assert!(!carpeta.exists(), "error_en {error_en:?}: {carpeta:?}");
            }
            let quedo: Vec<_> = fs::read_dir(tmp.path()).expect("lee tmp").collect();
            assert!(quedo.is_empty(), "error_en {error_en:?}: {quedo:?}");
        }
    }

    #[test]
    fn sin_carpeta_de_eficiencia_crea_y_borra_su_carpeta_como_hoy() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let log = RefCell::new(Vec::new());
        let inicial = con_carpeta("inicial", &log, None);
        let repo = con_carpeta("repo", &log, None);
        medir(&inicial, &repo, tmp.path(), &mut Vec::new());
        let carpetas_i = inicial.carpetas.borrow();
        assert!(carpetas_i[0].starts_with(tmp.path()), "{carpetas_i:?}");
        assert!(fs::read_dir(tmp.path()).expect("lee tmp").next().is_none());
    }

    #[test]
    fn con_carpeta_de_eficiencia_cada_lado_usa_su_subcarpeta_fija() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let fija = tempfile::tempdir().expect("carpeta fija");
        let log = RefCell::new(Vec::new());
        let inicial = con_carpeta("inicial", &log, None);
        let repo = con_carpeta("repo", &log, None);
        con_carpeta_de_eficiencia(Some(fija.path()), || {
            medir(&inicial, &repo, tmp.path(), &mut Vec::new());
        });
        let carpetas_i = inicial.carpetas.borrow().clone();
        let carpetas_r = repo.carpetas.borrow().clone();
        let esperada_i = fija.path().join("inicial");
        let esperada_r = fija.path().join("repo");
        assert!(!carpetas_i.is_empty(), "sin llamadas del inicial");
        assert!(
            carpetas_i.iter().all(|c| *c == esperada_i),
            "{carpetas_i:?}"
        );
        assert!(!carpetas_r.is_empty(), "sin llamadas del repo");
        assert!(
            carpetas_r.iter().all(|c| *c == esperada_r),
            "{carpetas_r:?}"
        );
        assert!(esperada_i.is_dir() && esperada_r.is_dir());
        let quedo: Vec<_> = fs::read_dir(tmp.path()).expect("lee tmp").collect();
        assert!(quedo.is_empty(), "{quedo:?}");
    }

    #[test]
    fn con_carpeta_de_eficiencia_dos_mediciones_reusan_la_misma_carpeta() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let fija = tempfile::tempdir().expect("carpeta fija");
        let log = RefCell::new(Vec::new());
        let inicial = con_carpeta("inicial", &log, None);
        let repo = con_carpeta("repo", &log, None);
        con_carpeta_de_eficiencia(Some(fija.path()), || {
            medir(&inicial, &repo, tmp.path(), &mut Vec::new());
            medir(&inicial, &repo, tmp.path(), &mut Vec::new());
        });
        let usadas: BTreeSet<PathBuf> = inicial
            .carpetas
            .borrow()
            .iter()
            .chain(repo.carpetas.borrow().iter())
            .cloned()
            .collect();
        assert_eq!(usadas.len(), 2, "{usadas:?}");
        assert!(fija.path().is_dir());
    }

    #[test]
    fn con_carpeta_de_eficiencia_restaura_la_anterior_tambien_con_panico() {
        assert_eq!(carpeta_de_eficiencia_actual(), None);
        con_carpeta_de_eficiencia(Some(Path::new("/a")), || {
            con_carpeta_de_eficiencia(Some(Path::new("/b")), || {
                assert_eq!(
                    carpeta_de_eficiencia_actual().as_deref(),
                    Some(Path::new("/b"))
                );
            });
            assert_eq!(
                carpeta_de_eficiencia_actual().as_deref(),
                Some(Path::new("/a"))
            );
            let panico = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                con_carpeta_de_eficiencia(Some(Path::new("/c")), || panic!("a propósito"));
            }));
            assert!(panico.is_err());
            assert_eq!(
                carpeta_de_eficiencia_actual().as_deref(),
                Some(Path::new("/a")),
                "tras un pánico vuelve la anterior"
            );
        });
        assert_eq!(
            carpeta_de_eficiencia_actual(),
            None,
            "tras f vuelve el anterior"
        );
    }

    #[test]
    fn con_carpeta_de_eficiencia_que_no_se_puede_crear_da_cuatro_medidas_sin_valor() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let fija = tempfile::tempdir().expect("carpeta fija");
        // Un archivo donde iría la subcarpeta `inicial`: crearla falla.
        fs::write(fija.path().join("inicial"), "x").expect("archivo");
        let log = RefCell::new(Vec::new());
        let inicial = con_carpeta("inicial", &log, None);
        let repo = con_carpeta("repo", &log, None);
        let resultados = con_carpeta_de_eficiencia(Some(fija.path()), || {
            medir(&inicial, &repo, tmp.path(), &mut Vec::new())
        });
        assert_eq!(resultados.len(), 4, "{resultados:?}");
        for r in &resultados {
            assert_eq!((r.referencia, r.repo, &r.comparacion), (None, None, &None));
            assert!(
                r.motivo
                    .as_deref()
                    .is_some_and(|m| m.starts_with("carpeta de trabajo: ")),
                "{:?}",
                r.motivo
            );
        }
        assert!(inicial.carpetas.borrow().is_empty());
        assert!(repo.carpetas.borrow().is_empty());
    }

    #[test]
    fn cmd_tests_corre_solo_los_tests_sin_compilar() {
        let cmd = cmd_tests(" -E 'not (test(=b))'");
        assert!(!cmd.contains("--no-run"), "{cmd}");
        assert!(!cmd.contains("cargo build"), "{cmd}");
        assert!(cmd.contains("cargo nextest run"), "{cmd}");
        assert!(cmd.contains("date +%s%N"), "{cmd}");
        assert!(cmd.contains("/sys/fs/cgroup/memory.peak"), "{cmd}");
    }

    #[test]
    fn cmd_compilar_tests_compila_con_no_run_y_deja_su_codigo() {
        let cmd = cmd_compilar_tests();
        assert!(cmd.contains("cargo nextest run --no-run"), "{cmd}");
        assert!(cmd.contains("CARGO_TARGET_DIR=/salida/target"), "{cmd}");
        assert!(cmd.contains("ROOT_COMPILAR $?"), "{cmd}");
    }

    #[test]
    fn medir_compila_los_tests_una_vez_por_lado_antes_de_las_corridas() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let log = RefCell::new(Vec::new());
        let inicial = sano("inicial", &log, &["a"]);
        let repo = sano("repo", &log, &["a"]);
        medir(&inicial, &repo, tmp.path(), &mut Vec::new());
        let vistas: Vec<String> = log
            .borrow()
            .iter()
            .filter_map(|e| {
                let mut partes = e.splitn(3, ' ');
                let lado = partes.next()?;
                let tipo = partes.next()?;
                matches!(tipo, "compilar_tests" | "tests").then(|| format!("{lado} {tipo}"))
            })
            .collect();
        let mut esperado = vec!["inicial compilar_tests", "repo compilar_tests"];
        for _ in 0..CORRIDAS_TESTS {
            esperado.extend(["inicial tests", "repo tests"]);
        }
        assert_eq!(vistas, esperado);
    }

    #[test]
    fn corrida_medida_que_recompila_deja_tiempo_y_memoria_sin_valor() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let log = RefCell::new(Vec::new());
        let inicial = sano("inicial", &log, &["a"]);
        let repo = Falso {
            tests: format!("   Compiling foo v0.1.0 (/repo)\n{TESTS_OK}"),
            ..sano("repo", &log, &["a"])
        };
        let resultados = medir(&inicial, &repo, tmp.path(), &mut Vec::new());
        for medida in [Medida::TiempoTests, Medida::MemoriaPico] {
            let r = por_medida(&resultados, medida);
            assert_eq!(
                r.motivo.as_deref(),
                Some("la corrida medida recompiló"),
                "{medida:?}"
            );
            assert_eq!(r.referencia, None, "{medida:?}");
            assert_eq!(r.repo, None, "{medida:?}");
        }
    }

    #[test]
    fn compilacion_de_tests_que_falla_deja_el_lado_sin_valor_y_no_corre_sus_tests() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let log = RefCell::new(Vec::new());
        let inicial = sano("inicial", &log, &["a"]);
        let repo = Falso {
            compilar: "ROOT_COMPILAR 101\n".into(),
            ..sano("repo", &log, &["a"])
        };
        let resultados = medir(&inicial, &repo, tmp.path(), &mut Vec::new());
        for medida in [Medida::TiempoTests, Medida::MemoriaPico] {
            let r = por_medida(&resultados, medida);
            assert_eq!(
                r.motivo.as_deref(),
                Some("repo: la compilación de los tests falló"),
                "{medida:?}"
            );
        }
        assert!(
            log.borrow().iter().all(|e| !e.starts_with("repo tests ")),
            "{:?}",
            log.borrow()
        );
    }

    /// Borrado falso que siempre falla. Simula una carpeta de trabajo que no se puede borrar.
    fn borrado_que_falla(_raiz: &Path, _ruta: &Path) -> std::io::Result<()> {
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "permiso denegado de prueba",
        ))
    }

    #[test]
    fn medir_con_borrado_que_falla_deja_aviso_con_carpeta_y_error() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let log = RefCell::new(Vec::new());
        let inicial = con_carpeta("inicial", &log, None);
        let repo = con_carpeta("repo", &log, None);
        let mut avisos = Vec::new();
        medir_con(&inicial, &repo, tmp.path(), &borrado_que_falla, &mut avisos);
        let raiz = inicial.carpetas.borrow()[0]
            .parent()
            .expect("carpeta de trabajo")
            .to_path_buf();
        assert_eq!(avisos.len(), 1, "{avisos:?}");
        assert!(
            avisos[0].contains(&raiz.display().to_string()),
            "{avisos:?}"
        );
        assert!(
            avisos[0].contains("permiso denegado de prueba"),
            "{avisos:?}"
        );
    }

    #[test]
    fn medir_con_borrado_que_anda_no_deja_aviso_y_la_medida_no_cambia() {
        let log = RefCell::new(Vec::new());
        let mut con_fallo = Vec::new();
        let mut sin_fallo = Vec::new();
        let tmp = tempfile::tempdir().expect("temp dir");
        let inicial = con_carpeta("inicial", &log, None);
        let repo = con_carpeta("repo", &log, None);
        let medidas_sin_fallo = medir_con(
            &inicial,
            &repo,
            tmp.path(),
            &borrado::borrar_dir_dentro,
            &mut sin_fallo,
        );
        let tmp_fallo = tempfile::tempdir().expect("temp dir");
        let inicial_f = con_carpeta("inicial", &log, None);
        let repo_f = con_carpeta("repo", &log, None);
        let medidas_con_fallo = medir_con(
            &inicial_f,
            &repo_f,
            tmp_fallo.path(),
            &borrado_que_falla,
            &mut con_fallo,
        );
        assert!(sin_fallo.is_empty(), "{sin_fallo:?}");
        assert_eq!(con_fallo.len(), 1, "{con_fallo:?}");
        assert_eq!(medidas_sin_fallo, medidas_con_fallo);
    }

    /// Mide una vez con `inicial` y `repo` y devuelve la referencia que dejó el lado inicial.
    fn referencia_de(inicial: &Falso, repo: &Falso) -> Referencia {
        let _ = medir_de_prueba(inicial, repo);
        tomar_referencia().expect("la medición dejó una referencia")
    }

    #[test]
    fn filtro_de_tests_comunes_lee_sin_vaciar_la_referencia() {
        let _ = tomar_referencia();
        assert_eq!(
            filtro_de_tests_comunes(),
            None,
            "sin medición no hay filtro"
        );
        let log = RefCell::new(Vec::new());
        // `b` pasa en el inicial y no en el repo: no es común, así que el filtro la excluye.
        let inicial = Falso {
            lista: lista(&[("a", true), ("b", true)]),
            ..sano("inicial", &log, &[])
        };
        let repo = Falso {
            lista: lista(&[("a", true), ("b", false)]),
            ..sano("repo", &log, &[])
        };
        let _ = medir_de_prueba(&inicial, &repo);
        let primera = filtro_de_tests_comunes();
        let segunda = filtro_de_tests_comunes();
        assert_eq!(primera, Some(Ok(" -E 'not (test(=b))'".to_string())));
        assert_eq!(primera, segunda, "leer dos veces da lo mismo");
        assert!(tomar_referencia().is_some(), "leer no vacía la referencia");
    }

    #[test]
    fn con_referencia_el_inicial_no_pide_metadata_ni_lista() {
        let log_base = RefCell::new(Vec::new());
        let referencia = referencia_de(
            &sano("inicial", &log_base, &["a"]),
            &sano("repo", &log_base, &["a"]),
        );
        let log = RefCell::new(Vec::new());
        let inicial = sano("inicial", &log, &["a"]);
        let repo = sano("repo", &log, &["a"]);
        let resultados = con_referencia(Some(referencia), || medir_de_prueba(&inicial, &repo));
        assert_eq!(resultados.len(), 4);
        let entradas = log.borrow();
        // Sin metadata ni lista: esas vienen de la referencia. Tests: CORRIDAS_TESTS veces. Build: su ancla.
        let mut esperado = vec!["compilar_tests"];
        esperado.extend(vec!["tests"; CORRIDAS_TESTS]);
        esperado.push("build");
        assert_eq!(tipos_de(&entradas, "inicial"), esperado);
        assert!(
            entradas.iter().any(|e| e.starts_with("repo ")),
            "{entradas:?}"
        );
    }

    #[test]
    fn con_referencia_el_inicial_compila_una_vez_y_una_su_ancla() {
        let log_base = RefCell::new(Vec::new());
        let referencia = referencia_de(
            &sano("inicial", &log_base, &["a"]),
            &sano("repo", &log_base, &["a"]),
        );
        let log = RefCell::new(Vec::new());
        let inicial = sano("inicial", &log, &["a"]);
        let repo = sano("repo", &log, &["a"]);
        con_referencia(Some(referencia), || medir_de_prueba(&inicial, &repo));
        let entradas = log.borrow();
        // Las llamadas de tests del inicial las cuenta `con_referencia_el_inicial_corre_sus_tests_corridas_tests_veces`.
        let tipos = tipos_de(&entradas, "inicial");
        let cuenta = |tipo: &str| tipos.iter().filter(|t| *t == tipo).count();
        assert_eq!(cuenta("build"), 1, "{tipos:?}");
        assert_eq!(cuenta("compilar_tests"), 1, "{tipos:?}");
    }

    #[test]
    fn con_referencia_el_repo_compila_una_vez_y_el_inicial_una_su_ancla() {
        let log_base = RefCell::new(Vec::new());
        let referencia = referencia_de(
            &sano("inicial", &log_base, &["a"]),
            &sano("repo", &log_base, &["a"]),
        );
        let log = RefCell::new(Vec::new());
        let inicial = sano("inicial", &log, &["a"]);
        let repo = sano("repo", &log, &["a"]);
        con_referencia(Some(referencia), || medir_de_prueba(&inicial, &repo));
        let entradas = log.borrow();
        let tipos_repo = tipos_de(&entradas, "repo");
        let cuenta_repo = tipos_repo.iter().filter(|t| *t == "build").count();
        assert_eq!(cuenta_repo, 1, "{tipos_repo:?}");
        let tipos_inicial = tipos_de(&entradas, "inicial");
        let cuenta_inicial = tipos_inicial.iter().filter(|t| *t == "build").count();
        assert_eq!(cuenta_inicial, 1, "{tipos_inicial:?}");
    }

    #[test]
    fn con_referencia_el_inicial_corre_sus_tests_corridas_tests_veces() {
        let log_base = RefCell::new(Vec::new());
        let referencia = referencia_de(
            &sano("inicial", &log_base, &["a"]),
            &sano("repo", &log_base, &["a"]),
        );
        let log = RefCell::new(Vec::new());
        let inicial = sano("inicial", &log, &["a"]);
        let repo = sano("repo", &log, &["a"]);
        con_referencia(Some(referencia), || medir_de_prueba(&inicial, &repo));
        let entradas = log.borrow();
        let tipos = tipos_de(&entradas, "inicial");
        let cuenta = tipos.iter().filter(|t| *t == "tests").count();
        assert_eq!(cuenta, CORRIDAS_TESTS, "{tipos:?}");
    }

    #[test]
    fn con_referencia_el_doble_en_ambos_lados_da_razon_uno_y_sin_regresion() {
        let log_base = RefCell::new(Vec::new());
        let referencia = referencia_de(
            &sano("inicial", &log_base, &["a"]),
            &sano("repo", &log_base, &["a"]),
        );
        let log = RefCell::new(Vec::new());
        // Los dos lados tardan el doble que la base (1,5 s) y usan el doble de memoria (1000 bytes).
        let lento = "ROOT_NS 3000000000\nROOT_MEM 2000\n";
        let inicial = Falso {
            tests: lento.into(),
            ..sano("inicial", &log, &["a"])
        };
        let repo = Falso {
            tests: lento.into(),
            ..sano("repo", &log, &["a"])
        };
        let resultados = con_referencia(Some(referencia), || medir_de_prueba(&inicial, &repo));
        for medida in [Medida::TiempoTests, Medida::MemoriaPico] {
            let c = por_medida(&resultados, medida)
                .comparacion
                .as_ref()
                .expect("comparable");
            assert!(cerca(c.razon, 1.0), "{medida:?}: razon {}", c.razon);
            assert!(!c.regresion, "{medida:?}: {c:?}");
        }
    }

    #[test]
    fn con_referencia_los_tests_alternan_repo_e_inicial_en_cada_par() {
        let log_base = RefCell::new(Vec::new());
        let referencia = referencia_de(
            &sano("inicial", &log_base, &["a"]),
            &sano("repo", &log_base, &["a"]),
        );
        let log = RefCell::new(Vec::new());
        let inicial = sano("inicial", &log, &["a"]);
        let repo = sano("repo", &log, &["a"]);
        con_referencia(Some(referencia), || medir_de_prueba(&inicial, &repo));
        let entradas = log.borrow();
        let esperado: Vec<String> = (0..CORRIDAS_TESTS)
            .flat_map(|_| ["repo", "inicial"])
            .map(String::from)
            .collect();
        assert_eq!(lados_de(&entradas, "tests"), esperado, "{entradas:?}");
    }

    #[test]
    fn sin_referencia_cada_lado_compila_tres_veces() {
        let log = RefCell::new(Vec::new());
        let inicial = sano("inicial", &log, &["a"]);
        let repo = sano("repo", &log, &["a"]);
        medir_de_prueba(&inicial, &repo);
        let entradas = log.borrow();
        for lado in ["inicial", "repo"] {
            let tipos = tipos_de(&entradas, lado);
            let cuenta = tipos.iter().filter(|t| *t == "build").count();
            assert_eq!(cuenta, CORRIDAS, "{lado}: {tipos:?}");
        }
    }

    #[test]
    fn con_referencia_el_orden_de_las_compilaciones_es_repo_inicial() {
        let log_base = RefCell::new(Vec::new());
        let referencia = referencia_de(
            &sano("inicial", &log_base, &["a"]),
            &sano("repo", &log_base, &["a"]),
        );
        let log = RefCell::new(Vec::new());
        let inicial = sano("inicial", &log, &["a"]);
        let repo = sano("repo", &log, &["a"]);
        con_referencia(Some(referencia), || medir_de_prueba(&inicial, &repo));
        let entradas = log.borrow();
        // El orden de los tests lo cubre `con_referencia_los_tests_alternan_repo_e_inicial_en_cada_par`.
        // Build: el repo compila CORRIDAS_COMPILACION_DE_CASO vez y el ancla del inicial corre tras ella.
        let esperado_build: Vec<String> =
            ["repo", "inicial"].into_iter().map(String::from).collect();
        assert_eq!(lados_de(&entradas, "build"), esperado_build, "{entradas:?}");
    }

    #[test]
    fn sin_referencia_el_orden_alterna_como_hoy() {
        let log = RefCell::new(Vec::new());
        let inicial = sano("inicial", &log, &["a"]);
        let repo = sano("repo", &log, &["a"]);
        medir_de_prueba(&inicial, &repo);
        let entradas = log.borrow();
        let esperado_tests: Vec<String> = (0..CORRIDAS_TESTS)
            .flat_map(|_| ["inicial", "repo"])
            .map(String::from)
            .collect();
        let esperado_build: Vec<String> = (0..CORRIDAS)
            .flat_map(|_| ["inicial", "repo"])
            .map(String::from)
            .collect();
        assert_eq!(lados_de(&entradas, "tests"), esperado_tests, "{entradas:?}");
        assert_eq!(lados_de(&entradas, "build"), esperado_build, "{entradas:?}");
    }

    /// Un ancla que falla no hace fallar la medida: la referencia queda en la serie de la base.
    #[test]
    fn con_referencia_un_ancla_con_error_se_ignora() {
        let log_base = RefCell::new(Vec::new());
        let referencia = referencia_de(
            &sano("inicial", &log_base, &["a"]),
            &sano("repo", &log_base, &["a"]),
        );
        let log = RefCell::new(Vec::new());
        // Solo falla el build del ancla. Los tests del inicial son sanos: su serie es viva, no un ancla.
        let inicial = Falso {
            build: "ROOT_BUILD 1\n".into(),
            ..sano("inicial", &log, &["a"])
        };
        let repo = Falso {
            tests: "ROOT_NS 3000000000\nROOT_MEM 2000\n".into(),
            build: "ROOT_NS 4000000000\nROOT_BUILD 0\nROOT_SIZE 1000\n".into(),
            ..sano("repo", &log, &["a"])
        };
        let resultados = con_referencia(Some(referencia), || medir_de_prueba(&inicial, &repo));
        for medida in [Medida::TiempoTests, Medida::TiempoCompilacion] {
            let r = por_medida(&resultados, medida);
            assert!(r.motivo.is_none(), "{medida:?}: {r:?}");
            let razon = r.comparacion.as_ref().map(|c| c.razon);
            assert!(razon.is_some_and(|x| cerca(x, 2.0)), "{medida:?}: {r:?}");
        }
    }

    /// Un ancla de compilación con error deja un aviso, con el id de la medida y el motivo.
    #[test]
    fn con_referencia_el_ancla_con_error_deja_aviso_de_compilacion() {
        let log_base = RefCell::new(Vec::new());
        let referencia = referencia_de(
            &sano("inicial", &log_base, &["a"]),
            &sano("repo", &log_base, &["a"]),
        );
        let log = RefCell::new(Vec::new());
        let inicial = Falso {
            build: "ROOT_BUILD 1\n".into(),
            ..sano("inicial", &log, &["a"])
        };
        let repo = sano("repo", &log, &["a"]);
        let tmp = tempfile::tempdir().expect("temp dir");
        let mut avisos = Vec::new();
        con_referencia(Some(referencia), || {
            medir_con(
                &inicial,
                &repo,
                tmp.path(),
                &borrado::borrar_dir_dentro,
                &mut avisos,
            )
        });
        assert_eq!(avisos.len(), 1, "{avisos:?}");
        assert!(avisos[0].contains("tiempo_compilacion"), "{avisos:?}");
    }

    /// Un ancla sana no deja aviso de ancla.
    #[test]
    fn con_referencia_un_ancla_sana_no_deja_aviso_de_ancla() {
        let log_base = RefCell::new(Vec::new());
        let referencia = referencia_de(
            &sano("inicial", &log_base, &["a"]),
            &sano("repo", &log_base, &["a"]),
        );
        let log = RefCell::new(Vec::new());
        let inicial = sano("inicial", &log, &["a"]);
        let repo = sano("repo", &log, &["a"]);
        let tmp = tempfile::tempdir().expect("temp dir");
        let mut avisos = Vec::new();
        con_referencia(Some(referencia), || {
            medir_con(
                &inicial,
                &repo,
                tmp.path(),
                &borrado::borrar_dir_dentro,
                &mut avisos,
            )
        });
        assert!(
            !avisos.iter().any(|a| a.contains("el ancla de")),
            "{avisos:?}"
        );
    }

    #[test]
    fn con_referencia_la_razon_usa_la_serie_guardada() {
        let log_base = RefCell::new(Vec::new());
        let referencia = referencia_de(
            &sano("inicial", &log_base, &["a"]),
            &sano("repo", &log_base, &["a"]),
        );
        // Compilación y tamaño: el ancla del lado inicial mide más que la base, y el mínimo sigue siendo el
        // de la base. Tiempo y memoria: el lado inicial vivo mide 9 s y 9000, así que esas razones salen de él.
        let log = RefCell::new(Vec::new());
        let inicial = Falso {
            tests: "ROOT_NS 9000000000\nROOT_MEM 9000\n".into(),
            build: "ROOT_NS 9000000000\nROOT_BUILD 0\nROOT_SIZE 9000\n".into(),
            ..sano("inicial", &log, &["a"])
        };
        let repo = Falso {
            tests: "ROOT_NS 3000000000\nROOT_MEM 2000\n".into(),
            build: "ROOT_NS 4000000000\nROOT_BUILD 0\nROOT_SIZE 1000\n".into(),
            ..sano("repo", &log, &["a"])
        };
        let resultados = con_referencia(Some(referencia), || medir_de_prueba(&inicial, &repo));
        let casos = [
            (Medida::TiempoTests, 9.0, 3.0, 3.0 / 9.0),
            (Medida::MemoriaPico, 9000.0, 2000.0, 2000.0 / 9000.0),
            (Medida::TiempoCompilacion, 2.0, 4.0, 2.0),
            (Medida::TamanoBinario, 500.0, 1000.0, 2.0),
        ];
        for (medida, referencia, repo, razon_esperada) in casos {
            let r = por_medida(&resultados, medida);
            assert_eq!(r.referencia, Some(referencia), "{medida:?}");
            assert_eq!(r.repo, Some(repo), "{medida:?}");
            let razon = r.comparacion.as_ref().map(|c| c.razon);
            assert!(
                razon.is_some_and(|x| cerca(x, razon_esperada)),
                "{medida:?}: {r:?}"
            );
        }
        // El lado inicial corre sus tests cada corrida y su compilación una vez: el ancla.
        let mut esperado = vec!["compilar_tests"];
        esperado.extend(vec!["tests"; CORRIDAS_TESTS]);
        esperado.push("build");
        assert_eq!(tipos_de(&log.borrow(), "inicial"), esperado);
    }

    #[test]
    fn con_referencia_serie_viva_fallida_deja_la_medida_sin_valor_con_su_motivo() {
        let log_base = RefCell::new(Vec::new());
        let referencia = referencia_de(
            &sano("inicial", &log_base, &["a"]),
            &sano("repo", &log_base, &["a"]),
        );
        // El lado inicial vivo no da memoria: su serie falla y la medida queda sin valor.
        let sin_pico = "ROOT_NS 1500000000\nROOT_MEM \n";
        let log = RefCell::new(Vec::new());
        let inicial = Falso {
            tests: sin_pico.into(),
            ..sano("inicial", &log, &["a"])
        };
        let repo = sano("repo", &log, &["a"]);
        let resultados = con_referencia(Some(referencia), || medir_de_prueba(&inicial, &repo));
        let memoria = por_medida(&resultados, Medida::MemoriaPico);
        assert_eq!(memoria.motivo.as_deref(), Some("sin memory.peak"));
        assert_eq!(memoria.referencia, None);
        let tiempo = por_medida(&resultados, Medida::TiempoTests);
        assert!(tiempo.referencia.is_some());
        assert!(tiempo.motivo.is_none());
    }

    #[test]
    fn referencia_con_filtro_distinto_deja_tests_sin_valor() {
        let log_base = RefCell::new(Vec::new());
        // La base: los dos lados pasan `a` y `b`. Su filtro no excluye nada.
        let referencia = referencia_de(
            &Falso {
                lista: lista(&[("a", true), ("b", true)]),
                ..sano("inicial", &log_base, &[])
            },
            &Falso {
                lista: lista(&[("a", true), ("b", true)]),
                ..sano("repo", &log_base, &[])
            },
        );
        // El caso: `b` falla en el repo. Su filtro excluye `b`, y el de la base no.
        let log = RefCell::new(Vec::new());
        let inicial = sano("inicial", &log, &[]);
        let repo = Falso {
            lista: lista(&[("a", true), ("b", false)]),
            ..sano("repo", &log, &[])
        };
        let resultados = con_referencia(Some(referencia), || medir_de_prueba(&inicial, &repo));
        for medida in [Medida::TiempoTests, Medida::MemoriaPico] {
            let r = por_medida(&resultados, medida);
            assert_eq!(
                r.motivo.as_deref(),
                Some("los tests comunes del caso difieren de los de la base"),
                "{medida:?}"
            );
        }
        let entradas = log.borrow();
        assert!(
            entradas.iter().all(|e| !e.starts_with("repo tests ")),
            "{entradas:?}"
        );
        assert!(
            por_medida(&resultados, Medida::TamanoBinario)
                .referencia
                .is_some()
        );
    }

    #[test]
    fn medir_deja_las_corridas_de_cada_lado_en_su_orden() {
        let log = RefCell::new(Vec::new());
        let inicial = sano("inicial", &log, &["a"]);
        let repo = Falso {
            tests: "ROOT_NS 3000000000\nROOT_MEM 2000\n".into(),
            ..sano("repo", &log, &["a"])
        };
        let resultados = medir_de_prueba(&inicial, &repo);
        let tiempo = por_medida(&resultados, Medida::TiempoTests);
        assert_eq!(
            tiempo.corridas_referencia,
            vec![1.5; CORRIDAS_TESTS],
            "{tiempo:?}"
        );
        assert_eq!(
            tiempo.corridas_repo,
            vec![3.0; CORRIDAS_TESTS],
            "{tiempo:?}"
        );
    }

    #[test]
    fn con_referencia_corridas_referencia_son_las_vivas_del_inicial() {
        let log_base = RefCell::new(Vec::new());
        let referencia = referencia_de(
            &sano("inicial", &log_base, &["a"]),
            &sano("repo", &log_base, &["a"]),
        );
        let log = RefCell::new(Vec::new());
        // El lado inicial vivo mide 9 segundos en cada corrida: esas son sus corridas de referencia.
        let inicial = Falso {
            tests: "ROOT_NS 9000000000\nROOT_MEM 9000\n".into(),
            ..sano("inicial", &log, &["a"])
        };
        let repo = Falso {
            tests: "ROOT_NS 3000000000\nROOT_MEM 2000\n".into(),
            ..sano("repo", &log, &["a"])
        };
        let resultados = con_referencia(Some(referencia), || medir_de_prueba(&inicial, &repo));
        let tiempo = por_medida(&resultados, Medida::TiempoTests);
        let esperado = vec![9.0; CORRIDAS_TESTS];
        assert_eq!(tiempo.corridas_referencia, esperado, "{tiempo:?}");
        assert_eq!(
            tiempo.corridas_repo,
            vec![3.0; CORRIDAS_TESTS],
            "{tiempo:?}"
        );
    }

    #[test]
    fn con_referencia_restaura_la_anterior_tambien_con_panico() {
        let log = RefCell::new(Vec::new());
        let a = referencia_de(&sano("inicial", &log, &["a"]), &sano("repo", &log, &["a"]));
        let b = referencia_de(
            &Falso {
                tests: "ROOT_NS 9000000000\nROOT_MEM 9\n".into(),
                ..sano("inicial", &log, &["a"])
            },
            &sano("repo", &log, &["a"]),
        );
        assert_eq!(referencia_fijada_actual(), None);
        con_referencia(Some(a.clone()), || {
            let corte = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                con_referencia(Some(b.clone()), || panic!("corte de prueba"))
            }));
            assert!(corte.is_err());
            assert_eq!(referencia_fijada_actual(), Some(a.clone()));
        });
        assert_eq!(referencia_fijada_actual(), None);
    }

    /// Base de t1: diez corridas de tests. Mediana 0,2945; cuartiles 0,29175 y 0,2965.
    const T1_BASE: [f64; 10] = [
        0.263, 0.287, 0.291, 0.294, 0.294, 0.295, 0.295, 0.297, 0.304, 0.306,
    ];
    /// Repo de t1: diez corridas. Mínimo 0,289; mediana 0,296; cuartiles chicos.
    const T1_REPO: [f64; 10] = [
        0.289, 0.294, 0.295, 0.295, 0.296, 0.296, 0.297, 0.297, 0.298, 0.300,
    ];

    #[test]
    fn t1_compara_por_mediana_y_ruido_por_cuartiles() {
        let c = comparar_por_mediana(&T1_BASE, &T1_REPO).expect("comparable");
        assert!(cerca(c.razon, 0.296 / 0.2945), "razon {}", c.razon);
        // La base manda: 2 x (0,2965 - 0,29175) / 0,2945.
        assert!(
            cerca(c.ruido, 2.0 * (0.2965 - 0.29175) / 0.2945),
            "ruido {}",
            c.ruido
        );
        assert!(!c.regresion, "{c:?}");
        // Guarda: la función vieja, con mínimos, da otra razón y otro ruido.
        let viejo = comparar(&T1_BASE, &T1_REPO).expect("comparable");
        assert!(cerca(viejo.razon, 0.289 / 0.263), "razon {}", viejo.razon);
        assert!(cerca(viejo.ruido, 0.043 / 0.263), "ruido {}", viejo.ruido);
    }

    #[test]
    fn base_estable_y_mediana_50_por_ciento_mas_alta_es_regresion() {
        let base = [
            1.000, 1.005, 1.002, 1.001, 1.004, 1.000, 1.003, 1.002, 1.001, 1.004,
        ];
        let repo: Vec<f64> = base.iter().map(|x| x * 1.5).collect();
        let c = comparar_por_mediana(&base, &repo).expect("comparable");
        assert!(cerca(c.razon, 1.5), "razon {}", c.razon);
        assert!(c.regresion, "{c:?}");
    }

    #[test]
    fn tres_corridas_por_lado_usan_el_ruido_de_hoy() {
        let referencia = [1.0, 1.1, 1.0];
        let repo = [1.05, 1.06, 1.08];
        let nuevo = comparar_por_mediana(&referencia, &repo).expect("comparable");
        let viejo = comparar(&referencia, &repo).expect("comparable");
        // Sin cuartiles ni factor 2: el ruido es (max-min)/min, como en `comparar`.
        assert!(cerca(nuevo.ruido, viejo.ruido), "{nuevo:?} {viejo:?}");
        assert!(cerca(nuevo.ruido, 0.10), "ruido {}", nuevo.ruido);
    }

    #[test]
    fn lado_con_tres_corridas_no_recibe_el_factor_del_otro() {
        // La base usa cuartiles (0,0322 con factor). El repo, con 3 corridas, usa (max-min)/min: 0,0381.
        let repo = [0.289, 0.296, 0.300];
        let c = comparar_por_mediana(&T1_BASE, &repo).expect("comparable");
        assert!(cerca(c.ruido, 0.011 / 0.289), "ruido {}", c.ruido);
    }

    #[test]
    fn cuartiles_de_1_a_5_corridas_con_interpolacion() {
        let series: [(&[f64], (f64, f64)); 5] = [
            (&[1.0], (1.0, 1.0)),
            (&[1.0, 2.0], (1.25, 1.75)),
            (&[1.0, 2.0, 3.0], (1.5, 2.5)),
            (&[1.0, 2.0, 3.0, 4.0], (1.75, 3.25)),
            (&[1.0, 2.0, 3.0, 4.0, 5.0], (2.0, 4.0)),
        ];
        for (serie, (q1, q3)) in series {
            let (a, b) = cuartiles(serie).expect("no vacía");
            assert!(cerca(a, q1) && cerca(b, q3), "{serie:?}: ({a}, {b})");
        }
    }

    #[test]
    fn cuartiles_de_t1_base_son_los_del_informe() {
        let (q1, q3) = cuartiles(&T1_BASE).expect("no vacía");
        assert!(cerca(q1, 0.29175), "q1 {q1}");
        assert!(cerca(q3, 0.2965), "q3 {q3}");
    }

    #[test]
    fn cuartiles_ordenan_la_entrada_y_vacio_da_none() {
        assert_eq!(cuartiles(&[]), None);
        assert_eq!(cuartiles(&[5.0, 1.0, 3.0, 2.0, 4.0]), Some((2.0, 4.0)));
    }

    #[test]
    fn mediana_impar_par_y_vacia() {
        assert_eq!(mediana(&[3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(mediana(&[4.0, 1.0, 3.0, 2.0]), Some(2.5));
        assert_eq!(mediana(&[]), None);
    }

    #[test]
    fn comparar_por_mediana_vacios_o_mediana_no_positiva_dan_none() {
        assert_eq!(comparar_por_mediana(&[], &[1.0]), None);
        assert_eq!(comparar_por_mediana(&[1.0], &[]), None);
        assert_eq!(comparar_por_mediana(&[0.0, 0.0, 1.0], &[1.0]), None);
    }

    #[test]
    fn combinar_compilacion_y_tamano_con_minimos_y_tiempo_y_memoria_con_mediana() {
        let base = T1_BASE.to_vec();
        let repo = T1_REPO.to_vec();
        let serie: [Serie; 2] = [Ok(base.clone()), Ok(repo.clone())];
        let resultados = combinar_medidas(&serie, &serie, &serie, &serie, &Ancla::vacia());
        let esperado_minimo = comparar(&base, &repo).expect("comparable");
        let esperado_mediana = comparar_por_mediana(&base, &repo).expect("comparable");
        for medida in [Medida::TiempoCompilacion, Medida::TamanoBinario] {
            let m = por_medida(&resultados, medida);
            assert_eq!(m.comparacion, Some(esperado_minimo.clone()), "{medida:?}");
            assert_eq!(m.referencia, Some(0.263), "{medida:?}");
            assert_eq!(m.repo, Some(0.289), "{medida:?}");
        }
        for medida in [Medida::TiempoTests, Medida::MemoriaPico] {
            let m = por_medida(&resultados, medida);
            assert_eq!(m.comparacion, Some(esperado_mediana.clone()), "{medida:?}");
            assert!(
                m.referencia.is_some_and(|x| cerca(x, 0.2945)),
                "{medida:?}: {m:?}"
            );
            assert!(m.repo.is_some_and(|x| cerca(x, 0.296)), "{medida:?}: {m:?}");
            assert_eq!(m.corridas_referencia, base, "{medida:?}");
            assert_eq!(m.corridas_repo, repo, "{medida:?}");
        }
    }
}
