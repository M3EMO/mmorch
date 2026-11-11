//! Calidad de los tests del proyecto por mutación (Tarea 9e). Corre `cargo-mutants` sobre una copia
//! del repo y mide qué cambios de comportamiento detectan los tests. El puntaje es la proporción
//! de mutantes muertos. Si la herramienta falta en la imagen, no hay veredicto (fail-closed).

use std::cell::RefCell;
use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::oracle::ErrorOraculo;
use crate::oracle::descarga::PRELUDIO;
use crate::oracle::perfiles::herramientas::{
    EjecutorHerramienta, con_cola, falta_herramienta, motivo_json, sin_veredicto,
};
use crate::oracle::perfiles::{ResultadoSonda, crear_hallazgo};
use crate::oracle::tarea::{MINUTOS_MAX_DEFECTO, MUTANTES_MAX_DEFECTO, Requisito, Tarea};
use crate::tipos::Dimension;

const DIMENSION: Dimension = Dimension::Pruebas;
const SONDA: &str = "cargo-mutants";
/// Reporte que deja el comando dentro de `/salida`. Ver `comando_de_corrida`.
pub(crate) const ARCHIVO_OUTCOMES: &str = "outcomes.json";
/// Líneas `test ... FAILED` de cada log de la corrida, con su ruta. Ver `comando_de_corrida`.
pub(crate) const ARCHIVO_TESTS_FALLIDOS: &str = "tests-fallidos.txt";
/// Listado JSON de los mutantes que deja `comando_de_listado` dentro de `/salida`.
const ARCHIVO_LISTADO: &str = "listado.json";
const DETALLE_VIVO: &str = "este cambio de comportamiento no lo detecta ningún test";
const SEVERIDAD_VIVO: u8 = 2;

/// Resultado de la sonda de mutación. `muestra` es cuántos mutantes se midieron.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultadoMutacion {
    pub sonda: ResultadoSonda,
    pub muertos: u32,
    pub vivos: u32,
    pub muestra: u32,
    /// Mutantes del listado (viables o no), si la muestra quedó corta.
    pub total_conocido: Option<u32>,
    pub truncado: bool,
    /// Mutantes de la muestra, en orden, con su estado. El puntaje no cambia: `Vencido` cuenta como muerto.
    #[serde(default)]
    pub mutantes: Vec<MutanteEstado>,
    /// Duración de la fase Test del Baseline, en segundos. `None` si el reporte no la trae o es una sonda sin reporte.
    #[serde(default)]
    pub base_segundos: Option<f64>,
}

/// Multiplicador del plazo por mutante sobre la duración de la fase Test de la base. Cita de cargo-mutants:
/// el plazo automático es 5 veces la duración de la base (https://mutants.rs/timeouts.html).
pub(crate) const MULTIPLICADOR_PLAZO: f64 = 5.0;
/// Plazo mínimo por mutante, en segundos. También de https://mutants.rs/timeouts.html.
pub(crate) const MINIMO_PLAZO_SEGUNDOS: u64 = 20;

/// Plazo por mutante en segundos: `MULTIPLICADOR_PLAZO` veces la duración de la base, con piso
/// `MINIMO_PLAZO_SEGUNDOS`. Redondea hacia arriba.
pub(crate) fn plazo_de_base(duracion_test_segundos: f64) -> u64 {
    let calculado = (MULTIPLICADOR_PLAZO * duracion_test_segundos).ceil() as u64;
    calculado.max(MINIMO_PLAZO_SEGUNDOS)
}

thread_local! {
    /// Duración de la fase Test de la base que midió la última `sonda_mutacion` de este hilo.
    static ULTIMA_BASE_SEGUNDOS: RefCell<Option<f64>> = const { RefCell::new(None) };
    /// Plazo fijo por mutante que usa `comando_de_corrida` en este hilo. `None`: plazo automático.
    static PLAZO_FIJO: RefCell<Option<u64>> = const { RefCell::new(None) };
}

/// Saca la duración de la base de la última `sonda_mutacion` de este hilo. Deja el valor vacío.
pub(crate) fn tomar_base_segundos() -> Option<f64> {
    ULTIMA_BASE_SEGUNDOS.with(|u| u.borrow_mut().take())
}

/// Corre `f` con el plazo fijo `segundos` en la corrida de mutación. Sin plazo, la corrida usa el automático.
pub(crate) fn con_plazo_fijo<T>(segundos: Option<u64>, f: impl FnOnce() -> T) -> T {
    /// Devuelve el plazo anterior al salir del ámbito.
    struct Restaura(Option<u64>);
    impl Drop for Restaura {
        fn drop(&mut self) {
            let anterior = self.0.take();
            PLAZO_FIJO.with(|p| *p.borrow_mut() = anterior);
        }
    }
    let anterior = PLAZO_FIJO.with(|p| p.replace(segundos));
    let _restaura = Restaura(anterior);
    f()
}

/// Estado de un mutante de la muestra. `Vencido` es un `Timeout`: cuenta como muerto en el puntaje.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EstadoMutante {
    Muerto,
    Vivo,
    Vencido,
}

/// Un mutante de la muestra y su estado. `nombre` es el `name` del reporte de cargo-mutants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MutanteEstado {
    pub nombre: String,
    pub estado: EstadoMutante,
}

thread_local! {
    /// Última muestra que midió `sonda_mutacion` en este hilo. La toma `tomar_muestra`.
    static ULTIMA_MUESTRA: RefCell<Vec<MutanteEstado>> = const { RefCell::new(Vec::new()) };
}

/// Saca la muestra que midió la última `sonda_mutacion` de este hilo. Deja la muestra vacía.
pub(crate) fn tomar_muestra() -> Vec<MutanteEstado> {
    ULTIMA_MUESTRA.with(|u| std::mem::take(&mut *u.borrow_mut()))
}

/// Nombres, en orden de `caso`, de los mutantes que están en `base` y en `caso` con estado distinto.
/// No cuenta `elegido`. Un nombre que solo está en una de las dos listas no cuenta.
pub(crate) fn mutantes_cambiados(
    base: &[MutanteEstado],
    caso: &[MutanteEstado],
    elegido: Option<&str>,
) -> Vec<String> {
    caso.iter()
        .filter(|c| Some(c.nombre.as_str()) != elegido)
        .filter(|c| {
            base.iter()
                .find(|b| b.nombre == c.nombre)
                .is_some_and(|b| b.estado != c.estado)
        })
        .map(|c| c.nombre.clone())
        .collect()
}

/// Mutante muerto de la muestra: un cambio de comportamiento que algún test detecta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutanteMuerto {
    pub archivo: String,
    pub linea: u32,
    pub nombre: String,
    /// Tests que fallaron con este mutante, en el orden de `tests-fallidos.txt`.
    pub tests: Vec<String>,
}

/// Reporte `outcomes.json` de cargo-mutants. Solo se leen los campos que Root usa.
#[derive(Deserialize)]
struct Informe {
    /// Sin esta clave el reporte no trae su marca de fin. `None` da `SinVeredicto`.
    outcomes: Option<Vec<Resultado>>,
    /// Mutantes que cargo-mutants encontró. Sin la clave vale 0.
    #[serde(default)]
    total_mutants: u32,
}

#[derive(Deserialize)]
struct Resultado {
    scenario: Escenario,
    #[serde(default)]
    summary: String,
    /// Ruta del log del mutante dentro de `log/`. `null` si no hay log.
    #[serde(default)]
    log_path: Option<String>,
    /// Fases de la corrida (`Build`, `Test`) con su duración. Sin la clave vale vacía.
    #[serde(default)]
    phase_results: Vec<FaseResultado>,
}

/// Una fase de la corrida. Solo se leen `phase` y `duration`, en segundos.
#[derive(Deserialize)]
struct FaseResultado {
    #[serde(default)]
    phase: String,
    /// Sin el campo, o si no es número, vale `None`: la fase no da duración.
    #[serde(default)]
    duration: Option<f64>,
}

/// Duración de la fase `Test` de una corrida, en segundos. `None` si falta o no es finita.
fn duracion_de_test(fases: &[FaseResultado]) -> Option<f64> {
    fases
        .iter()
        .find(|f| f.phase == "Test")
        .and_then(|f| f.duration)
        .filter(|d| d.is_finite())
}

/// `"Baseline"` o `{"Mutant": {...}}`: forma externa de serde.
#[derive(Deserialize)]
enum Escenario {
    Baseline,
    Mutant(Mutante),
}

#[derive(Deserialize)]
struct Mutante {
    #[serde(default)]
    name: String,
    #[serde(default)]
    file: String,
    #[serde(default)]
    span: Span,
}

#[derive(Deserialize, Default)]
struct Span {
    #[serde(default)]
    start: Posicion,
}

#[derive(Deserialize, Default)]
struct Posicion {
    #[serde(default)]
    line: u32,
}

/// Mutante viable del reporte: su ubicación y si lo detectó algún test.
struct Viable {
    archivo: String,
    linea: u32,
    nombre: String,
    muerto: bool,
    /// Estado del mutante. `Vencido` (`Timeout`) también tiene `muerto` en verdadero.
    estado: EstadoMutante,
    /// Log del mutante, si el reporte lo trae. Lo usan los tests que fallaron con él.
    log_path: Option<String>,
    /// Verdadero si el resumen es `Timeout`: no se le atribuyen tests.
    timeout: bool,
}

/// Lectura del reporte: los viables en orden del archivo y la duración de la base, o el motivo por el que
/// no hay muestra.
enum Lectura {
    Sin(String),
    Medible(Vec<Viable>, Option<f64>),
}

/// Valida el reporte y lista sus viables. `Unviable` no compila: no cuenta. Es la base de la muestra
/// que miden `parsear_outcomes` y `mutantes_muertos`.
fn leer_viables(json: &str) -> Result<Lectura, ErrorOraculo> {
    if json.trim().is_empty() {
        return Ok(Lectura::Sin(
            "no dejó reporte: el archivo outcomes.json está vacío".into(),
        ));
    }
    let informe: Informe = serde_json::from_str(json)
        .map_err(|e| ErrorOraculo::Infra(format!("outcomes.json: {}", motivo_json(&e))))?;
    let Some(resultados) = informe.outcomes else {
        return Ok(Lectura::Sin("sin marca de fin: falta outcomes".into()));
    };
    let mut viables: Vec<Viable> = Vec::new();
    // `Baseline` es la marca de fin: sin él, la lista no prueba que la corrida terminó.
    let mut hay_baseline = false;
    let mut base_segundos = None;
    for resultado in &resultados {
        let de = |m: &Mutante, estado: EstadoMutante| Viable {
            archivo: m.file.clone(),
            linea: m.span.start.line,
            nombre: m.name.clone(),
            muerto: estado != EstadoMutante::Vivo,
            estado,
            log_path: resultado.log_path.clone(),
            timeout: resultado.summary == "Timeout",
        };
        match &resultado.scenario {
            Escenario::Baseline => match resultado.summary.as_str() {
                "Success" => {
                    hay_baseline = true;
                    base_segundos = duracion_de_test(&resultado.phase_results);
                }
                "Failure" => {
                    return Ok(Lectura::Sin(
                        "los tests del proyecto fallan sin mutaciones".into(),
                    ));
                }
                otro => {
                    return Err(ErrorOraculo::Infra(format!(
                        "resumen de Baseline desconocido: {otro}"
                    )));
                }
            },
            Escenario::Mutant(mutante) => match resultado.summary.as_str() {
                "Unviable" => {}
                "CaughtMutant" => viables.push(de(mutante, EstadoMutante::Muerto)),
                "Timeout" => viables.push(de(mutante, EstadoMutante::Vencido)),
                "MissedMutant" => viables.push(de(mutante, EstadoMutante::Vivo)),
                otro => {
                    return Err(ErrorOraculo::Infra(format!(
                        "resumen de mutante desconocido: {otro}"
                    )));
                }
            },
        }
    }
    if !hay_baseline {
        return Ok(Lectura::Sin(
            "sin marca de fin: falta el escenario Baseline".into(),
        ));
    }
    // Un reporte vale solo si trae un resultado por cada mutante que cargo-mutants encontró (M13).
    let mutantes_en_reporte = resultados
        .iter()
        .filter(|r| matches!(r.scenario, Escenario::Mutant(_)))
        .count();
    if mutantes_en_reporte != informe.total_mutants as usize {
        return Ok(Lectura::Sin(format!(
            "reporte truncado: trae {mutantes_en_reporte} de {} mutantes",
            informe.total_mutants
        )));
    }
    Ok(Lectura::Medible(viables, base_segundos))
}

/// Primeros `mutantes_max` viables, en el orden del archivo. El `-F` ya filtra la muestra de la sonda.
fn muestra(viables: &[Viable], mutantes_max: u32) -> &[Viable] {
    &viables[..viables.len().min(mutantes_max as usize)]
}

/// Lee el reporte de mutantes. Cuenta los primeros `mutantes_max` viables, en el orden del archivo.
pub fn parsear_outcomes(
    json: &str,
    mutantes_max: u32,
    requisitos: &[Requisito],
) -> Result<ResultadoMutacion, ErrorOraculo> {
    if mutantes_max == 0 {
        return Err(ErrorOraculo::Infra("mutantes_max debe ser 1 o más".into()));
    }
    let (viables, base_segundos) = match leer_viables(json)? {
        Lectura::Sin(motivo) => return Ok(desde_sonda(sin_veredicto(DIMENSION, motivo))),
        Lectura::Medible(viables, base_segundos) => (viables, base_segundos),
    };
    if viables.is_empty() {
        return Ok(ResultadoMutacion {
            base_segundos,
            ..desde_sonda(ResultadoSonda {
                dimension: DIMENSION,
                valor: None,
                motivo: Some("sin mutantes para medir: ningún mutante es viable".into()),
                hallazgos: Vec::new(),
            })
        });
    }
    let total = u32::try_from(viables.len())
        .map_err(|_| ErrorOraculo::Infra("demasiados mutantes".into()))?;
    let muestra = muestra(&viables, mutantes_max);
    let muertos = muestra.iter().filter(|v| v.muerto).count() as u32;
    let vivos = muestra.len() as u32 - muertos;
    let hallazgos = muestra
        .iter()
        .filter(|v| !v.muerto)
        .map(|v| {
            crear_hallazgo(
                DIMENSION,
                SEVERIDAD_VIVO,
                &v.archivo,
                v.linea,
                &v.nombre,
                DETALLE_VIVO,
                SONDA,
                requisitos,
            )
        })
        .collect();
    let truncado = total > mutantes_max;
    Ok(ResultadoMutacion {
        sonda: ResultadoSonda {
            dimension: DIMENSION,
            valor: Some(muertos as f32 / muestra.len() as f32),
            motivo: None,
            hallazgos,
        },
        muertos,
        vivos,
        muestra: muestra.len() as u32,
        total_conocido: truncado.then_some(total),
        truncado,
        mutantes: muestra
            .iter()
            .map(|v| MutanteEstado {
                nombre: v.nombre.clone(),
                estado: v.estado,
            })
            .collect(),
        base_segundos,
    })
}

/// Tests fallidos por log: la clave es la ruta `log/<archivo>.log`. Cada lista sigue el orden del archivo.
fn leer_tests_fallidos(texto: &str) -> BTreeMap<&str, Vec<&str>> {
    let mut por_log: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for linea in texto.lines() {
        // Formato de `grep -H`: `<ruta>:test <nombre> ... FAILED`. Otra línea no cuenta.
        let Some((ruta, resto)) = linea.split_once(":test ") else {
            continue;
        };
        let Some(nombre) = resto.strip_suffix(" ... FAILED") else {
            continue;
        };
        let tests = por_log.entry(ruta).or_default();
        if !tests.contains(&nombre) {
            tests.push(nombre);
        }
    }
    por_log
}

/// Tests que fallaron con `viable`. Un `Timeout` no tiene tests, aunque su log tenga líneas.
fn tests_de(viable: &Viable, por_log: &BTreeMap<&str, Vec<&str>>) -> Vec<String> {
    if viable.timeout {
        return Vec::new();
    }
    viable
        .log_path
        .as_deref()
        .and_then(|ruta| por_log.get(ruta))
        .map(|tests| tests.iter().map(|t| t.to_string()).collect())
        .unwrap_or_default()
}

/// Mutantes muertos de la muestra que mide `parsear_outcomes`, en el orden del archivo. Cada uno trae
/// los tests que fallaron con él, según `tests_fallidos` (el `tests-fallidos.txt` de la corrida). Un
/// reporte sin veredicto, ilegible o vacío no tiene muestra: da una lista vacía.
pub fn mutantes_muertos(json: &str, mutantes_max: u32, tests_fallidos: &str) -> Vec<MutanteMuerto> {
    if mutantes_max == 0 {
        return Vec::new();
    }
    let Ok(Lectura::Medible(viables, _)) = leer_viables(json) else {
        return Vec::new();
    };
    let por_log = leer_tests_fallidos(tests_fallidos);
    muestra(&viables, mutantes_max)
        .iter()
        .filter(|v| v.muerto)
        .map(|v| MutanteMuerto {
            archivo: v.archivo.clone(),
            linea: v.linea,
            nombre: v.nombre.clone(),
            tests: tests_de(v, &por_log),
        })
        .collect()
}

/// Mutantes del listado, en orden de ejecución. Cada entrada trae el `name` que muestra `--list`.
#[derive(Deserialize)]
struct ItemListado {
    name: String,
}

/// Nombres de los mutantes del listado JSON de `cargo mutants --list --json`, en orden.
fn leer_listado(stdout: &str) -> Result<Vec<String>, String> {
    let items: Vec<ItemListado> = serde_json::from_str(stdout)
        .map_err(|e| format!("listado ilegible: {}", motivo_json(&e)))?;
    Ok(items.into_iter().map(|i| i.name).collect())
}

thread_local! {
    /// Archivo al que se acota la corrida de mutación de este hilo. `None`: todo el repo.
    static ARCHIVO_ACOTADO: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Corre `f` con la mutación acotada a `archivo` (`None`: sin acotar). Al salir, también si `f` entra en
/// pánico, vuelve el valor anterior.
pub fn con_archivo_acotado<T>(archivo: Option<&str>, f: impl FnOnce() -> T) -> T {
    /// Devuelve el valor anterior al salir del ámbito.
    struct Restaura(Option<String>);
    impl Drop for Restaura {
        fn drop(&mut self) {
            let anterior = self.0.take();
            ARCHIVO_ACOTADO.with(|a| *a.borrow_mut() = anterior);
        }
    }
    let anterior = ARCHIVO_ACOTADO.with(|a| a.replace(archivo.map(str::to_string)));
    let _restaura = Restaura(anterior);
    f()
}

/// Archivo al que está acotada la mutación de este hilo, si hay uno.
pub(crate) fn archivo_acotado() -> Option<String> {
    ARCHIVO_ACOTADO.with(|a| a.borrow().clone())
}

/// Comando que lista los mutantes en orden de ejecución, sobre una copia de `/repo`. Escribe el
/// listado en `/salida`: el stdout del ejecutor se corta a sus últimos caracteres.
pub(crate) fn comando_de_listado() -> String {
    format!(
        "{PRELUDIO}cp -a /repo /salida/work && cd /salida/work && \
         cargo mutants --list --json --no-shuffle > /salida/{ARCHIVO_LISTADO}"
    )
}

/// Filtro `-F` que deja solo los mutantes de `nombres`: su alternación exacta, escapada.
fn filtro_de(nombres: &[String]) -> String {
    let alternos: Vec<String> = nombres.iter().map(|n| escapar_regex(n)).collect();
    format!(
        " -F {}",
        comilla_shell(&format!("^(?:{})$", alternos.join("|")))
    )
}

/// Muestra repartida de `nombres`: `max` índices `i * total / max`, en orden. Si hay menos, toma todos.
/// Multiplica en u128 para que el producto no desborde.
fn muestra_repartida(nombres: &[String], max: usize) -> Vec<String> {
    let total = nombres.len();
    if total <= max {
        return nombres.to_vec();
    }
    (0..max)
        .map(|i| {
            let indice = (i as u128 * total as u128 / max as u128) as usize;
            nombres[indice].clone()
        })
        .collect()
}

/// Comando de la corrida de mutación. Corre sobre una copia de `/repo`.
/// Deja `outcomes.json` y `tests-fallidos.txt` en `/salida`: el ejecutor solo lee archivos sueltos ahí.
/// `filtro` es `""` (todos los mutantes) o el `-F` de `filtro_de`.
/// El último comando es el `cp`: su código de salida es el del comando.
fn comando_de_corrida(tarea: &Tarea, filtro: &str) -> String {
    let minutos = tarea
        .pruebas
        .as_ref()
        .map_or(MINUTOS_MAX_DEFECTO, |p| p.minutos_max);
    let segundos = u64::from(minutos) * 60;
    // Con plazo fijo (la calibración), cada mutante corre con ese `--timeout`. Sin él, el automático.
    let plazo = PLAZO_FIJO
        .with(|p| *p.borrow())
        .map_or(String::new(), |s| format!(" --timeout {s}"));
    format!(
        "{PRELUDIO}cp -a /repo /salida/work && cd /salida/work && \
         CARGO_TARGET_DIR=/salida/target timeout {segundos} cargo mutants --no-shuffle --jobs 1{plazo} \
         --output /salida/mutants{filtro} -- --offline; \
         (cd /salida/mutants/mutants.out && grep -H '^test .* \\.\\.\\. FAILED$' log/*.log) \
         > /salida/{ARCHIVO_TESTS_FALLIDOS} 2>/dev/null; \
         cp /salida/mutants/mutants.out/outcomes.json /salida/{ARCHIVO_OUTCOMES} 2>/dev/null"
    )
}

/// Comando de la corrida completa, sin filtro. Solo lo usan tests de otros módulos como clave del
/// memo. La sonda corre `comando_de_corrida` con la muestra.
#[cfg(test)]
pub fn construir_comando(tarea: &Tarea) -> String {
    comando_de_corrida(tarea, "")
}

/// Escapa los metacaracteres de regex de `texto`, para que `-F` lo busque tal cual.
fn escapar_regex(texto: &str) -> String {
    let mut escapado = String::with_capacity(texto.len());
    for c in texto.chars() {
        if "\\.+*?()|[]{}^$".contains(c) {
            escapado.push('\\');
        }
        escapado.push(c);
    }
    escapado
}

/// Entrecomilla `texto` para `sh -c`: entre comillas simples, con cada `'` escapada.
fn comilla_shell(texto: &str) -> String {
    format!("'{}'", texto.replace('\'', "'\\''"))
}

/// Verdadero si `cmd` es la corrida de mutación de la sonda, no el listado. El memo la busca así.
pub(crate) fn es_comando_de_mutacion(cmd: &str) -> bool {
    cmd.contains("cargo mutants ")
        && cmd.contains("--output /salida/mutants")
        && !cmd.contains("--list")
}

/// Verdadero si `cmd` es el listado de mutantes que arma `comando_de_listado`.
pub(crate) fn es_comando_de_listado(cmd: &str) -> bool {
    cmd.contains("cargo mutants ") && cmd.contains("--list")
}

/// Mutantes máximos de la muestra: los de `[pruebas]`, o el default.
pub(crate) fn mutantes_max(tarea: &Tarea) -> u32 {
    tarea
        .pruebas
        .as_ref()
        .map_or(MUTANTES_MAX_DEFECTO, |p| p.mutantes_max)
}

/// Envuelve un resultado sin mutantes medidos.
fn desde_sonda(sonda: ResultadoSonda) -> ResultadoMutacion {
    ResultadoMutacion {
        sonda,
        muertos: 0,
        vivos: 0,
        muestra: 0,
        total_conocido: None,
        truncado: false,
        mutantes: Vec::new(),
        base_segundos: None,
    }
}

/// Sonda de mutación. Solo Rust. Sin `cargo-mutants` en la imagen, no corre el comando largo.
pub fn sonda_mutacion(ej: &dyn EjecutorHerramienta, tarea: &Tarea) -> ResultadoMutacion {
    sonda_mutacion_en(ej, tarea, None)
}

/// Como `sonda_mutacion`, pero con `cambios` (rangos de `rangos_cambiados`), solo los mutantes que
/// caen dentro de ellos forman la muestra. Sin `cambios`, mide el listado entero.
pub fn sonda_mutacion_en(
    ej: &dyn EjecutorHerramienta,
    tarea: &Tarea,
    cambios: Option<&BTreeMap<String, Vec<(u32, u32)>>>,
) -> ResultadoMutacion {
    let resultado = medir_mutantes(ej, tarea, cambios);
    ULTIMA_MUESTRA.with(|u| *u.borrow_mut() = resultado.mutantes.clone());
    ULTIMA_BASE_SEGUNDOS.with(|u| *u.borrow_mut() = resultado.base_segundos);
    resultado
}

/// Verdadero si `nombre` (`<archivo>:<línea>:<col>: ...`) cae en un rango de `cambios`.
/// Un nombre que no trae archivo y línea, o cuyo archivo no está en `cambios`, no cae dentro.
fn dentro_del_cambio(nombre: &str, cambios: &BTreeMap<String, Vec<(u32, u32)>>) -> bool {
    let mut partes = nombre.split(':');
    let (Some(archivo), Some(linea)) = (partes.next(), partes.next()) else {
        return false;
    };
    let Ok(linea) = linea.parse::<u32>() else {
        return false;
    };
    cambios.get(archivo).is_some_and(|rangos| {
        rangos
            .iter()
            .any(|&(desde, hasta)| desde <= linea && linea <= hasta)
    })
}

/// Cuerpo de `sonda_mutacion_en`. Su resultado sale con la muestra de `mutantes`.
/// Con `cambios`, la muestra y el total son los mutantes que caen dentro del cambio.
fn medir_mutantes(
    ej: &dyn EjecutorHerramienta,
    tarea: &Tarea,
    cambios: Option<&BTreeMap<String, Vec<(u32, u32)>>>,
) -> ResultadoMutacion {
    if tarea.lenguaje != "rust" {
        return desde_sonda(sin_veredicto(
            DIMENSION,
            "la sonda de mutación solo existe para Rust por ahora",
        ));
    }
    if let Some(r) = falta_herramienta(ej, DIMENSION, SONDA) {
        return desde_sonda(r);
    }
    // Primero el listado: da los nombres en orden. La corrida mide solo una muestra de `mutantes_max`.
    let listado = match ej.ejecutar(&comando_de_listado()) {
        Ok(s) => s,
        Err(e) => return desde_sonda(sin_veredicto(DIMENSION, e.to_string())),
    };
    if listado.codigo != 0 {
        let base = format!(
            "no dejó listado de mutantes (cargo-mutants código {})",
            listado.codigo
        );
        return desde_sonda(sin_veredicto(DIMENSION, con_cola(base, &listado.stderr)));
    }
    let Some(json) = listado.archivos.get(ARCHIVO_LISTADO) else {
        let base = "no dejó listado de mutantes (falta listado.json)".to_string();
        return desde_sonda(sin_veredicto(DIMENSION, base));
    };
    let nombres = match leer_listado(json) {
        Ok(nombres) => nombres,
        Err(motivo) => return desde_sonda(sin_veredicto(DIMENSION, motivo)),
    };
    if nombres.is_empty() {
        return desde_sonda(ResultadoSonda {
            dimension: DIMENSION,
            valor: None,
            motivo: Some("sin mutantes para medir: el listado no trae mutantes".into()),
            hallazgos: Vec::new(),
        });
    }
    let nombres = match cambios {
        Some(c) => {
            let dentro: Vec<String> = nombres
                .into_iter()
                .filter(|n| dentro_del_cambio(n, c))
                .collect();
            if dentro.is_empty() {
                let motivo = "ningún mutante cae dentro del cambio";
                return desde_sonda(sin_veredicto(DIMENSION, motivo));
            }
            dentro
        }
        None => nombres,
    };
    let max = mutantes_max(tarea);
    let total = nombres.len() as u32;
    let muestra = muestra_repartida(&nombres, max as usize);
    // Acotada a un archivo: la muestra se queda con sus mutantes. El total sigue siendo el del listado.
    let muestra = match archivo_acotado() {
        Some(archivo) => {
            let prefijo = format!("{archivo}:");
            let del_archivo: Vec<String> = muestra
                .into_iter()
                .filter(|n| n.starts_with(&prefijo))
                .collect();
            if del_archivo.is_empty() {
                let motivo =
                    format!("ningún mutante de la muestra es de {archivo}: no hay nada que medir");
                return desde_sonda(sin_veredicto(DIMENSION, motivo));
            }
            del_archivo
        }
        None => muestra,
    };
    let salida = match ej.ejecutar(&comando_de_corrida(tarea, &filtro_de(&muestra))) {
        Ok(s) => s,
        Err(e) => return desde_sonda(sin_veredicto(DIMENSION, e.to_string())),
    };
    let Some(json) = salida.archivos.get(ARCHIVO_OUTCOMES) else {
        let base = format!("no dejó reporte (cargo-mutants código {})", salida.codigo);
        return desde_sonda(sin_veredicto(DIMENSION, con_cola(base, &salida.stderr)));
    };
    let mut resultado = parsear_outcomes(json, max, &tarea.requisito)
        .unwrap_or_else(|e| desde_sonda(sin_veredicto(DIMENSION, e.to_string())));
    // El reporte solo trae la muestra: el total real sale del listado.
    if resultado.sonda.valor.is_some() && total > max {
        resultado.truncado = true;
        resultado.total_conocido = Some(total);
    }
    resultado
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::fs;

    use crate::oracle::perfiles::herramientas::SalidaHerramienta;

    const CON_ASSERTS: &str =
        include_str!("../../tests/fixtures/mutacion/outcomes-con-asserts.json");
    const SIN_ASSERTS: &str =
        include_str!("../../tests/fixtures/mutacion/outcomes-sin-asserts.json");
    const SIN_MUTANTES: &str =
        include_str!("../../tests/fixtures/mutacion/outcomes-sin-mutantes.json");

    /// Tarea mínima. `extra` agrega tablas al final, como `[pruebas]`.
    fn tarea_con(lenguaje: &str, extra: &str) -> Tarea {
        let dir = tempfile::tempdir().expect("temp dir");
        let texto = format!(
            "nombre = \"x\"\nlenguaje = \"{lenguaje}\"\nimagen = \"i\"\nspec = \"s\"\n\
             [contrato]\ncmd = \"c\"\n[oculto]\ncmd = \"o\"\n{extra}"
        );
        fs::write(dir.path().join("tarea.toml"), texto).expect("escribe tarea.toml");
        Tarea::cargar_de(dir.path()).expect("carga la tarea")
    }

    type Respuesta = Box<dyn Fn(&str) -> Result<SalidaHerramienta, ErrorOraculo>>;

    /// Ejecutor que registra los comandos y responde con `responde`.
    struct Falso {
        cmds: RefCell<Vec<String>>,
        responde: Respuesta,
    }

    impl Falso {
        fn nuevo(
            responde: impl Fn(&str) -> Result<SalidaHerramienta, ErrorOraculo> + 'static,
        ) -> Self {
            Falso {
                cmds: RefCell::new(Vec::new()),
                responde: Box::new(responde),
            }
        }
    }

    impl EjecutorHerramienta for Falso {
        fn ejecutar(&self, cmd: &str) -> Result<SalidaHerramienta, ErrorOraculo> {
            self.cmds.borrow_mut().push(cmd.to_string());
            (self.responde)(cmd)
        }
    }

    /// Listado en JSON, como `cargo mutants --list --json`, con estos nombres en orden.
    fn listado_json(nombres: &[&str]) -> String {
        let items: Vec<serde_json::Value> = nombres
            .iter()
            .map(|n| serde_json::json!({ "name": n }))
            .collect();
        serde_json::Value::Array(items).to_string()
    }

    /// Nombres de los mutantes del reporte, en orden. Un reporte que no se lee da un solo nombre.
    fn nombres_del_reporte(json: &str) -> Vec<String> {
        let valor: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
        let nombres: Vec<String> = valor["outcomes"]
            .as_array()
            .map(|outs| {
                outs.iter()
                    .filter_map(|o| o["scenario"]["Mutant"]["name"].as_str())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();
        if nombres.is_empty() {
            vec!["m".into()]
        } else {
            nombres
        }
    }

    /// Salida del listado: el JSON va en `listado.json`, como lo deja `comando_de_listado`.
    fn salida_listado(json: String) -> SalidaHerramienta {
        let mut s = SalidaHerramienta::default();
        s.archivos.insert(ARCHIVO_LISTADO.into(), json);
        s
    }

    /// Responde "presente" a `command -v cargo-mutants`, da los mutantes del reporte al listado y
    /// `json` a la corrida.
    fn con_reporte(json: &'static str) -> Falso {
        Falso::nuevo(move |cmd| {
            if cmd.starts_with("command -v") {
                return Ok(SalidaHerramienta::default());
            }
            if cmd.contains("--list") {
                let nombres = nombres_del_reporte(json);
                let refs: Vec<&str> = nombres.iter().map(String::as_str).collect();
                return Ok(salida_listado(listado_json(&refs)));
            }
            let mut s = SalidaHerramienta::default();
            s.archivos.insert(ARCHIVO_OUTCOMES.into(), json.into());
            Ok(s)
        })
    }

    /// Listado de un mutante y una corrida que deja código 2 y ningún `outcomes.json`.
    fn sin_reporte() -> Falso {
        Falso::nuevo(|cmd| {
            if cmd.starts_with("command -v") {
                return Ok(SalidaHerramienta::default());
            }
            if cmd.contains("--list") {
                return Ok(salida_listado(listado_json(&["m"])));
            }
            Ok(SalidaHerramienta {
                codigo: 2,
                ..Default::default()
            })
        })
    }

    /// Nombres de la muestra de prueba. `->`, `+`, `(` y `.` son metacaracteres o flechas.
    const NOMBRES: [&str; 5] = [
        "src/a.rs:1:5: replace a -> u8 with 0",
        "src/a.rs:2:9: replace b + c with b - c",
        "src/a.rs:3:3: replace c(x) with d.e",
        "src/b.rs:4:1: replace d -> Vec<u8> with vec![]",
        "src/b.rs:5:1: replace e -> u8 with 7",
    ];

    /// Corrida con un mutante cazado por cada nombre.
    fn corrida_de(nombres: &[&str]) -> String {
        let mut outs = vec![serde_json::json!({"scenario": "Baseline", "summary": "Success"})];
        for (i, n) in nombres.iter().enumerate() {
            outs.push(serde_json::json!({
                "scenario": {"Mutant": {"name": n, "file": "src/a.rs",
                    "span": {"start": {"line": i + 1}}}},
                "summary": "CaughtMutant"
            }));
        }
        serde_json::json!({"total_mutants": nombres.len(), "outcomes": outs}).to_string()
    }

    /// Ejecutor que responde `listado` al listado y `corrida` a la corrida de mutación.
    fn con_listado_y_corrida(listado: String, corrida: String) -> Falso {
        Falso::nuevo(move |cmd| {
            if cmd.starts_with("command -v") {
                return Ok(SalidaHerramienta::default());
            }
            if cmd.contains("--list") {
                return Ok(salida_listado(listado.clone()));
            }
            let mut s = SalidaHerramienta::default();
            s.archivos.insert(ARCHIVO_OUTCOMES.into(), corrida.clone());
            Ok(s)
        })
    }

    /// Comandos registrados que son la corrida de mutación.
    fn corridas(ej: &Falso) -> Vec<String> {
        ej.cmds
            .borrow()
            .iter()
            .filter(|c| c.contains("--output /salida/mutants"))
            .cloned()
            .collect()
    }

    /// Tarea de Rust con `[pruebas]` de `mutantes_max`.
    fn tarea_pruebas(mutantes_max: u32) -> Tarea {
        tarea_con(
            "rust",
            &format!("\n[pruebas]\nmutantes_max = {mutantes_max}\nminutos_max = 20\n"),
        )
    }

    #[test]
    fn mutacion_escapa_metacaracteres_de_regex() {
        assert_eq!(
            escapar_regex("a+b(c).d->e<f|g[h]{i}^j$k*l?m\\n"),
            "a\\+b\\(c\\)\\.d->e<f\\|g\\[h\\]\\{i\\}\\^j\\$k\\*l\\?m\\\\n"
        );
    }

    #[test]
    fn mutacion_comilla_simple_no_rompe_el_shell() {
        let regex = format!("^(?:{})$", escapar_regex("src/a.rs: it's $HOME `x`"));
        let q = comilla_shell(&regex);
        let salida = std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("printf %s {q}"))
            .output()
            .expect("corre sh");
        assert_eq!(String::from_utf8_lossy(&salida.stdout), regex);
    }

    /// Cien mutantes en tres archivos, 34/33/33, con nombres únicos.
    fn cien_mutantes() -> Vec<String> {
        (0..100)
            .map(|i| {
                let archivo = match i {
                    0..34 => "a",
                    34..67 => "b",
                    _ => "c",
                };
                format!("src/{archivo}.rs:{i}:1: replace f{i} -> u8 with 0")
            })
            .collect()
    }

    #[test]
    fn mutacion_corre_una_muestra_repartida_del_listado() {
        let t = tarea_pruebas(3);
        // Con 5 nombres y max 3, los índices son 0, 5/3=1 y 10/3=3.
        let elegidos = [NOMBRES[0], NOMBRES[1], NOMBRES[3]];
        let ej = con_listado_y_corrida(listado_json(&NOMBRES), corrida_de(&elegidos));
        let r = sonda_mutacion(&ej, &t);
        assert_eq!(r.muestra, 3);
        assert!(r.truncado);
        assert_eq!(r.total_conocido, Some(5));
        let cmds = corridas(&ej);
        assert_eq!(cmds.len(), 1, "una sola corrida: {cmds:?}");
        let esperado = "-F '^(?:src/a\\.rs:1:5: replace a -> u8 with 0|\
            src/a\\.rs:2:9: replace b \\+ c with b - c|\
            src/b\\.rs:4:1: replace d -> Vec<u8> with vec!\\[\\])$'";
        assert!(cmds[0].contains(esperado), "{}", cmds[0]);
        assert!(
            !cmds[0].contains("c\\(x\\)"),
            "no debe pedir el índice 2: {}",
            cmds[0]
        );
    }

    #[test]
    fn mutacion_muestra_de_cien_mutantes_cubre_los_tres_archivos() {
        let nombres = cien_mutantes();
        let refs: Vec<&str> = nombres.iter().map(String::as_str).collect();
        let t = tarea_pruebas(10);
        // Índices 0, 10, ..., 90: caen en a.rs, b.rs y c.rs.
        let elegidos: Vec<&str> = (0..10).map(|i| refs[i * 10]).collect();
        let ej = con_listado_y_corrida(listado_json(&refs), corrida_de(&elegidos));
        let _ = sonda_mutacion(&ej, &t);
        let cmds = corridas(&ej);
        assert_eq!(cmds.len(), 1, "una sola corrida: {cmds:?}");
        for archivo in ["src/a\\.rs", "src/b\\.rs", "src/c\\.rs"] {
            assert!(cmds[0].contains(archivo), "falta {archivo}: {}", cmds[0]);
        }
        assert!(
            !cmds[0].contains("src/a\\.rs:1:1:"),
            "la muestra no toma los primeros: {}",
            cmds[0]
        );
    }

    #[test]
    fn mutacion_muestra_repartida_toma_indices_i_por_total_sobre_max() {
        let nombres = cien_mutantes();
        let muestra = muestra_repartida(&nombres, 10);
        let esperada: Vec<String> = (0..10).map(|i| nombres[i * 10].clone()).collect();
        assert_eq!(muestra, esperada);
        // Total 5 y max 3: índices 0, 1 y 3.
        let cinco: Vec<String> = (0..5).map(|i| format!("m{i}")).collect();
        assert_eq!(muestra_repartida(&cinco, 3), ["m0", "m1", "m3"]);
    }

    #[test]
    fn mutacion_muestra_repartida_con_menos_mutantes_que_el_maximo_toma_todos() {
        let nombres: Vec<String> = (0..4).map(|i| format!("m{i}")).collect();
        assert_eq!(muestra_repartida(&nombres, 10), nombres);
        assert_eq!(muestra_repartida(&nombres, 4), nombres);
    }

    #[test]
    fn mutacion_listado_fallido_da_sin_veredicto_con_motivo() {
        let t = tarea_pruebas(3);
        let ej = Falso::nuevo(|cmd| {
            Ok(SalidaHerramienta {
                codigo: if cmd.starts_with("command -v") { 0 } else { 2 },
                ..Default::default()
            })
        });
        let r = sonda_mutacion(&ej, &t);
        assert_eq!(r.sonda.valor, None);
        let motivo = r.sonda.motivo.unwrap_or_default();
        assert!(motivo.starts_with("no dejó listado"), "{motivo}");
        assert!(corridas(&ej).is_empty(), "sin listado no corre la muestra");
    }

    #[test]
    fn mutacion_listado_ilegible_da_sin_veredicto_con_motivo() {
        let t = tarea_pruebas(3);
        let ej = con_listado_y_corrida("{no es json".into(), corrida_de(&NOMBRES[..1]));
        let r = sonda_mutacion(&ej, &t);
        assert_eq!(r.sonda.valor, None);
        let motivo = r.sonda.motivo.unwrap_or_default();
        assert!(motivo.contains("listado ilegible"), "{motivo}");
        assert!(corridas(&ej).is_empty());
    }

    #[test]
    fn mutacion_listado_vacio_es_sin_mutantes_para_medir() {
        let t = tarea_pruebas(3);
        let ej = con_listado_y_corrida("[]".into(), corrida_de(&NOMBRES[..1]));
        let r = sonda_mutacion(&ej, &t);
        assert_eq!(r.sonda.valor, None);
        let motivo = r.sonda.motivo.unwrap_or_default();
        assert!(motivo.starts_with("sin mutantes para medir"), "{motivo}");
        assert!(corridas(&ej).is_empty());
    }

    #[test]
    fn mutacion_es_comando_de_mutacion_acepta_la_corrida_y_rechaza_el_resto() {
        let t = tarea_pruebas(3);
        let ej = con_listado_y_corrida(listado_json(&NOMBRES), corrida_de(&NOMBRES[..3]));
        let _ = sonda_mutacion(&ej, &t);
        let cmds = ej.cmds.borrow();
        let corrida = cmds
            .iter()
            .find(|c| c.contains("--output /salida/mutants"))
            .expect("hay corrida");
        let listado = cmds
            .iter()
            .find(|c| c.contains("--list"))
            .expect("hay listado");
        assert!(es_comando_de_mutacion(corrida), "{corrida}");
        assert!(!es_comando_de_mutacion(listado), "{listado}");
        assert!(!es_comando_de_mutacion("command -v cargo-mutants"));
        assert!(!es_comando_de_mutacion("echo hola"));
        assert!(es_comando_de_mutacion(&construir_comando(&t)));
    }

    fn falta_la_herramienta() -> Falso {
        Falso::nuevo(|cmd| {
            Ok(SalidaHerramienta {
                codigo: if cmd.starts_with("command -v") { 1 } else { 0 },
                ..Default::default()
            })
        })
    }

    #[test]
    fn mutacion_con_asserts_da_valor_uno_y_cero_hallazgos() {
        let r = parsear_outcomes(CON_ASSERTS, 60, &[]).expect("parsea");
        assert_eq!(r.sonda.valor, Some(1.0));
        assert_eq!(r.sonda.motivo, None);
        assert!(r.sonda.hallazgos.is_empty());
        assert_eq!(r.muertos, 10);
        assert_eq!(r.vivos, 0);
        assert_eq!(r.muestra, 10);
        assert!(!r.truncado);
    }

    #[test]
    fn mutacion_sin_asserts_da_dos_novenos_y_siete_hallazgos_con_archivo_y_linea() {
        let r = parsear_outcomes(SIN_ASSERTS, 60, &[]).expect("parsea");
        let valor = r.sonda.valor.expect("valor");
        assert!((valor - 2.0 / 9.0).abs() < 1e-6, "valor {valor}");
        assert_eq!(r.muertos, 2);
        assert_eq!(r.vivos, 7);
        assert_eq!(r.sonda.hallazgos.len(), 7);
        for h in &r.sonda.hallazgos {
            assert_eq!(h.dimension, Dimension::Pruebas);
            assert_eq!(h.severidad, SEVERIDAD_VIVO);
            assert_eq!(h.archivo, "src/calc.rs");
            assert!(h.linea > 0, "la línea sale del span");
            assert_eq!(h.detalle, DETALLE_VIVO);
        }
    }

    #[test]
    fn mutacion_sin_asserts_puntaje_menor_que_con_asserts() {
        let con = parsear_outcomes(CON_ASSERTS, 60, &[]).expect("parsea");
        let sin = parsear_outcomes(SIN_ASSERTS, 60, &[]).expect("parsea");
        assert!(sin.sonda.valor < con.sonda.valor);
    }

    #[test]
    fn mutacion_truncado_toma_los_primeros_cinco_en_orden() {
        // 20 mutantes viables: los 5 primeros se miden, en el orden del archivo.
        let mut outs = vec![r#"{"scenario":"Baseline","summary":"Success"}"#.to_string()];
        for i in 1..=20u32 {
            let resumen = if i <= 5 {
                "MissedMutant"
            } else {
                "CaughtMutant"
            };
            outs.push(format!(
                r#"{{"scenario":{{"Mutant":{{"name":"m{i}","file":"src/a.rs","span":{{"start":{{"line":{i},"column":1}}}}}}}},"summary":"{resumen}"}}"#
            ));
        }
        let json = format!(r#"{{"total_mutants":20,"outcomes":[{}]}}"#, outs.join(","));
        let r = parsear_outcomes(&json, 5, &[]).expect("parsea");
        assert_eq!(r.muestra, 5);
        assert!(r.truncado);
        assert_eq!(r.total_conocido, Some(20));
        // Los 5 primeros son los vivos: el hallazgo de cada uno sale de su línea.
        let lineas: Vec<u32> = r.sonda.hallazgos.iter().map(|h| h.linea).collect();
        assert_eq!(lineas, vec![1, 2, 3, 4, 5]);
        assert_eq!(r.sonda.valor, Some(0.0));
    }

    #[test]
    fn mutacion_baseline_failure_da_sin_veredicto_con_motivo() {
        let json = r#"{"outcomes":[{"scenario":"Baseline","summary":"Failure"}]}"#;
        let r = parsear_outcomes(json, 60, &[]).expect("parsea");
        assert_eq!(r.sonda.valor, None);
        assert_eq!(
            r.sonda.motivo.as_deref(),
            Some("los tests del proyecto fallan sin mutaciones")
        );
    }

    #[test]
    fn mutacion_sin_viables_da_valor_none() {
        let json = r#"{"total_mutants":1,"outcomes":[{"scenario":"Baseline","summary":"Success"},
            {"scenario":{"Mutant":{"name":"m","file":"src/a.rs"}},"summary":"Unviable"}]}"#;
        let r = parsear_outcomes(json, 60, &[]).expect("parsea");
        assert_eq!(r.sonda.valor, None);
        assert!(
            r.sonda
                .motivo
                .unwrap_or_default()
                .starts_with("sin mutantes para medir"),
        );
        assert_eq!(r.muestra, 0);
    }

    #[test]
    fn mutacion_reporte_valido_sin_mutantes_da_motivo_sin_mutantes_para_medir() {
        let r = parsear_outcomes(SIN_MUTANTES, 60, &[]).expect("parsea");
        assert_eq!(r.sonda.valor, None);
        assert!(r.sonda.hallazgos.is_empty());
        let motivo = r.sonda.motivo.unwrap_or_default();
        assert!(motivo.starts_with("sin mutantes para medir"), "{motivo}");
    }

    #[test]
    fn mutacion_sin_reporte_motivo_empieza_con_no_dejo_reporte() {
        let t = tarea_con("rust", "");
        let r = sonda_mutacion(&sin_reporte(), &t);
        let motivo = r.sonda.motivo.unwrap_or_default();
        assert!(motivo.starts_with("no dejó reporte"), "{motivo}");
    }

    #[test]
    fn mutacion_los_dos_motivos_de_cero_mutantes_son_distintos() {
        let t = tarea_con("rust", "");
        let motivo_sin_reporte = sonda_mutacion(&sin_reporte(), &t)
            .sonda
            .motivo
            .unwrap_or_default();
        let motivo_sin_mutantes = parsear_outcomes(SIN_MUTANTES, 60, &[])
            .expect("parsea")
            .sonda
            .motivo
            .unwrap_or_default();
        assert!(!motivo_sin_reporte.is_empty());
        assert!(!motivo_sin_mutantes.is_empty());
        assert_ne!(motivo_sin_reporte, motivo_sin_mutantes);
    }

    #[test]
    fn mutacion_json_roto_es_error_de_infraestructura() {
        let r = parsear_outcomes("{no es json", 60, &[]);
        assert!(matches!(r, Err(ErrorOraculo::Infra(_))));
    }

    #[test]
    fn mutacion_mutantes_max_cero_es_error_de_infraestructura() {
        assert!(matches!(
            parsear_outcomes(CON_ASSERTS, 0, &[]),
            Err(ErrorOraculo::Infra(_))
        ));
    }

    #[test]
    fn mutacion_construir_comando_usa_los_minutos_de_la_tarea() {
        let t = tarea_con("rust", "[pruebas]\nmutantes_max = 60\nminutos_max = 20\n");
        let cmd = construir_comando(&t);
        assert!(cmd.contains("timeout 1200"), "{cmd}");
        assert!(cmd.contains("cargo mutants"));
        assert!(cmd.contains("cp -a /repo /salida/work"));
        assert!(cmd.contains("--offline"));
        assert!(cmd.starts_with(PRELUDIO), "empieza con el preludio");
        assert!(cmd.contains("/salida/mutants/mutants.out/outcomes.json /salida/outcomes.json"));
    }

    #[test]
    fn mutacion_construir_comando_sin_pruebas_usa_los_defaults() {
        let t = tarea_con("rust", "");
        let cmd = construir_comando(&t);
        assert!(
            cmd.contains(&format!("timeout {}", MINUTOS_MAX_DEFECTO * 60)),
            "{cmd}"
        );
    }

    #[test]
    fn mutacion_acotada_no_lleva_file_en_listado_ni_corrida() {
        con_archivo_acotado(Some("src/a.rs"), || {
            let listado = comando_de_listado();
            assert!(!listado.contains("--file"), "{listado}");
            assert!(es_comando_de_listado(&listado), "{listado}");
            let corrida = comando_de_corrida(&tarea_pruebas(5), "");
            assert!(!corrida.contains("--file"), "{corrida}");
            assert!(es_comando_de_mutacion(&corrida), "{corrida}");
        });
    }

    #[test]
    fn mutacion_sin_acotar_los_comandos_son_iguales_a_hoy() {
        assert_eq!(archivo_acotado(), None);
        let listado = comando_de_listado();
        assert_eq!(
            listado,
            format!(
                "{PRELUDIO}cp -a /repo /salida/work && cd /salida/work && \
                 cargo mutants --list --json --no-shuffle > /salida/listado.json"
            )
        );
        let corrida = comando_de_corrida(&tarea_pruebas(5), "");
        assert!(!corrida.contains("--file"), "{corrida}");
        assert!(
            corrida.contains("--output /salida/mutants -- --offline"),
            "{corrida}"
        );
    }

    #[test]
    fn mutacion_con_archivo_acotado_restaura_tras_f_y_tras_un_panico() {
        assert_eq!(archivo_acotado(), None);
        con_archivo_acotado(Some("src/a.rs"), || {
            assert_eq!(archivo_acotado().as_deref(), Some("src/a.rs"));
            con_archivo_acotado(Some("src/b.rs"), || {
                assert_eq!(archivo_acotado().as_deref(), Some("src/b.rs"));
            });
            assert_eq!(archivo_acotado().as_deref(), Some("src/a.rs"));
        });
        assert_eq!(archivo_acotado(), None, "tras f vuelve el anterior");
        let panico = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            con_archivo_acotado(Some("src/c.rs"), || panic!("a propósito"));
        }));
        assert!(panico.is_err());
        assert_eq!(archivo_acotado(), None, "tras un pánico también vuelve");
    }

    #[test]
    fn mutacion_acotada_no_nombra_otro_archivo_en_ningun_comando() {
        let nombres = [
            "src/a.rs:1:5: replace a -> u8 with 0",
            "src/a.rs:2:9: replace b + c with b - c",
        ];
        let ej = con_listado_y_corrida(listado_json(&nombres), corrida_de(&nombres));
        let r = con_archivo_acotado(Some("src/a.rs"), || sonda_mutacion(&ej, &tarea_pruebas(5)));
        assert!(r.sonda.valor.is_some(), "{:?}", r.sonda.motivo);
        let cmds = ej.cmds.borrow();
        let de_mutantes: Vec<&String> = cmds
            .iter()
            .filter(|c| c.contains("cargo mutants"))
            .collect();
        assert_eq!(de_mutantes.len(), 2, "listado y corrida: {cmds:?}");
        for cmd in cmds.iter() {
            assert!(!cmd.contains("src/b.rs"), "nombra otro archivo: {cmd}");
            assert!(!cmd.contains("--file"), "{cmd}");
        }
        assert!(de_mutantes[1].contains("-F "), "{}", de_mutantes[1]);
    }

    #[test]
    fn mutacion_acotada_corre_solo_la_muestra_de_su_archivo() {
        let nombres = cien_mutantes();
        let refs: Vec<&str> = nombres.iter().map(String::as_str).collect();
        let t = tarea_pruebas(10);
        // Muestra de la base: índices 0, 10, ..., 90. De b.rs (índices 34..67) caen 40, 50 y 60.
        // El 60 queda lejos del inicio: el filtro debe mirar toda la muestra.
        // La corrida real solo trae los mutantes que pide `-F`: los tres de b.rs.
        let de_b = [refs[40], refs[50], refs[60]];
        let ej = con_listado_y_corrida(listado_json(&refs), corrida_de(&de_b));
        let r = con_archivo_acotado(Some("src/b.rs"), || sonda_mutacion(&ej, &t));
        assert!(r.sonda.valor.is_some(), "{:?}", r.sonda.motivo);
        assert_eq!(r.muestra, 3, "la muestra acotada tiene 3 mutantes");
        assert_eq!(
            r.total_conocido,
            Some(100),
            "el total sale del listado entero"
        );
        let cmds = corridas(&ej);
        assert_eq!(cmds.len(), 1, "una sola corrida: {cmds:?}");
        let pedidos = de_b.map(escapar_regex);
        let esperado = format!(
            " -F {}",
            comilla_shell(&format!("^(?:{})$", pedidos.join("|")))
        );
        assert!(cmds[0].contains(&esperado), "{esperado}\n{}", cmds[0]);
    }

    #[test]
    fn mutacion_ningun_comando_lleva_file_con_ni_sin_acotado() {
        let nombres = cien_mutantes();
        let refs: Vec<&str> = nombres.iter().map(String::as_str).collect();
        let base: Vec<&str> = (0..10).map(|i| refs[i * 10]).collect();
        for acotado in [None, Some("src/b.rs")] {
            let ej = con_listado_y_corrida(listado_json(&refs), corrida_de(&base));
            let _ = con_archivo_acotado(acotado, || sonda_mutacion(&ej, &tarea_pruebas(10)));
            let cmds = ej.cmds.borrow();
            assert_eq!(cmds.len(), 3, "sonda, listado y corrida: {cmds:?}");
            for cmd in cmds.iter() {
                assert!(!cmd.contains("--file"), "{acotado:?}: {cmd}");
            }
        }
    }

    #[test]
    fn mutacion_acotada_sin_mutantes_del_archivo_da_sin_veredicto() {
        let nombres = cien_mutantes();
        let refs: Vec<&str> = nombres.iter().map(String::as_str).collect();
        let ej = con_listado_y_corrida(listado_json(&refs), corrida_de(&refs[..1]));
        let r = con_archivo_acotado(Some("src/z.rs"), || sonda_mutacion(&ej, &tarea_pruebas(10)));
        assert_eq!(r.sonda.valor, None);
        let motivo = r.sonda.motivo.unwrap_or_default();
        assert!(motivo.contains("src/z.rs"), "{motivo}");
        assert!(
            corridas(&ej).is_empty(),
            "sin mutantes del archivo no corre la muestra"
        );
    }

    /// Rangos de `cambios` con un solo archivo y un solo rango.
    fn cambio_en(archivo: &str, desde: u32, hasta: u32) -> BTreeMap<String, Vec<(u32, u32)>> {
        BTreeMap::from([(archivo.to_string(), vec![(desde, hasta)])])
    }

    #[test]
    fn mutacion_con_cambios_filtra_la_corrida_a_los_mutantes_dentro() {
        let nombres = [
            "src/a.rs:9:5: replace x -> u8 with 0",
            "src/a.rs:10:5: replace y -> u8 with 0",
            "src/a.rs:20:1: replace z -> u8 with 0",
            "src/a.rs:21:1: replace w -> u8 with 0",
            "src/b.rs:12:1: replace v -> u8 with 0",
            "src/a.rs:15:2: replace u -> u8 with 0",
        ];
        let dentro = [nombres[1], nombres[2], nombres[5]];
        let t = tarea_pruebas(10);
        let ej = con_listado_y_corrida(listado_json(&nombres), corrida_de(&dentro));
        let cambios = cambio_en("src/a.rs", 10, 20);
        let r = sonda_mutacion_en(&ej, &t, Some(&cambios));
        assert!(r.sonda.valor.is_some(), "{:?}", r.sonda.motivo);
        assert_eq!(r.muestra, 3);
        let cmds = corridas(&ej);
        assert_eq!(cmds.len(), 1, "una sola corrida: {cmds:?}");
        let esperados: Vec<String> = dentro.iter().map(|n| n.to_string()).collect();
        assert!(cmds[0].contains(&filtro_de(&esperados)), "{}", cmds[0]);
        assert!(!cmds[0].contains("src/b"), "{}", cmds[0]);
        assert!(!cmds[0].contains("a\\.rs:9:"), "{}", cmds[0]);
        assert!(!cmds[0].contains("a\\.rs:21:"), "{}", cmds[0]);
    }

    #[test]
    fn mutacion_con_cambios_sin_mutantes_dentro_da_sin_veredicto() {
        let nombres = cien_mutantes();
        let refs: Vec<&str> = nombres.iter().map(String::as_str).collect();
        let ej = con_listado_y_corrida(listado_json(&refs), corrida_de(&refs[..1]));
        let cambios = cambio_en("src/a.rs", 500, 600);
        let r = sonda_mutacion_en(&ej, &tarea_pruebas(10), Some(&cambios));
        assert_eq!(r.sonda.valor, None);
        let motivo = r.sonda.motivo.unwrap_or_default();
        assert!(
            motivo.contains("ningún mutante cae dentro del cambio"),
            "{motivo}"
        );
        assert!(
            corridas(&ej).is_empty(),
            "sin mutantes dentro no corre la muestra"
        );
    }

    #[test]
    fn mutacion_sin_cambios_es_igual_a_sonda_mutacion() {
        let nombres = cien_mutantes();
        let refs: Vec<&str> = nombres.iter().map(String::as_str).collect();
        let base: Vec<&str> = (0..10).map(|i| refs[i * 10]).collect();
        let t = tarea_pruebas(10);
        let ej_a = con_listado_y_corrida(listado_json(&refs), corrida_de(&base));
        let a = sonda_mutacion(&ej_a, &t);
        let ej_b = con_listado_y_corrida(listado_json(&refs), corrida_de(&base));
        let b = sonda_mutacion_en(&ej_b, &t, None);
        assert_eq!(a.sonda.valor, b.sonda.valor);
        assert_eq!(a.muestra, b.muestra);
        assert_eq!(a.total_conocido, b.total_conocido);
        assert_eq!(a.truncado, b.truncado);
        assert_eq!(*ej_a.cmds.borrow(), *ej_b.cmds.borrow());
    }

    #[test]
    fn dentro_del_cambio_incluye_los_bordes_y_descarta_lo_de_afuera() {
        let cambios = BTreeMap::from([
            ("src/a.rs".to_string(), vec![(10, 20), (30, 30)]),
            ("src/b.rs".to_string(), vec![(5, 5)]),
        ]);
        assert!(
            dentro_del_cambio("src/a.rs:10:1: x", &cambios),
            "borde inferior"
        );
        assert!(
            dentro_del_cambio("src/a.rs:20:9: x", &cambios),
            "borde superior"
        );
        assert!(dentro_del_cambio("src/a.rs:15:1: x", &cambios), "interior");
        assert!(
            dentro_del_cambio("src/a.rs:30:1: x", &cambios),
            "rango de una línea"
        );
        assert!(
            dentro_del_cambio("src/b.rs:5:1: x", &cambios),
            "b.rs de una línea"
        );
        assert!(
            !dentro_del_cambio("src/a.rs:9:1: x", &cambios),
            "antes del rango"
        );
        assert!(
            !dentro_del_cambio("src/a.rs:21:1: x", &cambios),
            "entre rangos"
        );
        assert!(
            !dentro_del_cambio("src/a.rs:31:1: x", &cambios),
            "después del último"
        );
        assert!(
            !dentro_del_cambio("src/c.rs:12:1: x", &cambios),
            "archivo fuera del mapa"
        );
        assert!(
            !dentro_del_cambio("src/a.rs: sin línea", &cambios),
            "sin línea"
        );
        assert!(!dentro_del_cambio("src/a.rs", &cambios), "solo archivo");
        assert!(
            !dentro_del_cambio("src/a.rs:abc:1: x", &cambios),
            "línea no numérica"
        );
        assert!(!dentro_del_cambio("sin prefijo", &cambios), "sin archivo");
    }

    #[test]
    fn mutacion_con_cambios_mas_mutantes_que_el_tope_reparte_la_muestra_dentro() {
        // Listado: un mutante de b.rs, diez de a.rs (líneas 1..=10) y otro de b.rs.
        // Dentro del cambio son diez: con tope 3, índices 0, 3 y 6 de esos diez.
        let mut nombres: Vec<String> = vec!["src/b.rs:1:1: replace p -> u8 with 0".into()];
        nombres.extend((1..=10).map(|i| format!("src/a.rs:{i}:1: replace f{i} -> u8 with 0")));
        nombres.push("src/b.rs:2:1: replace q -> u8 with 0".into());
        let refs: Vec<&str> = nombres.iter().map(String::as_str).collect();
        let esperados: Vec<String> = [1, 4, 7]
            .iter()
            .map(|i| format!("src/a.rs:{i}:1: replace f{i} -> u8 with 0"))
            .collect();
        let refs_pedidos: Vec<&str> = esperados.iter().map(String::as_str).collect();
        let ej = con_listado_y_corrida(listado_json(&refs), corrida_de(&refs_pedidos));
        let cambios = cambio_en("src/a.rs", 1, 10);
        let r = sonda_mutacion_en(&ej, &tarea_pruebas(3), Some(&cambios));
        assert!(r.sonda.valor.is_some(), "{:?}", r.sonda.motivo);
        assert_eq!(r.muestra, 3);
        assert!(r.truncado, "10 dentro del cambio y tope 3");
        assert_eq!(r.total_conocido, Some(10));
        let cmds = corridas(&ej);
        assert_eq!(cmds.len(), 1, "una sola corrida: {cmds:?}");
        assert!(cmds[0].contains(&filtro_de(&esperados)), "{}", cmds[0]);
    }

    #[test]
    fn mutacion_sonda_ausente_no_corre_el_comando_largo() {
        let t = tarea_con("rust", "");
        let ej = falta_la_herramienta();
        let r = sonda_mutacion(&ej, &t);
        assert_eq!(r.sonda.valor, None);
        assert_eq!(
            r.sonda.motivo.as_deref(),
            Some("cargo-mutants no está en la imagen de evaluación")
        );
        let cmds = ej.cmds.borrow();
        assert!(
            !cmds.iter().any(|c| c.contains("cargo mutants")),
            "no debe correr cargo mutants: {cmds:?}"
        );
    }

    #[test]
    fn mutacion_sonda_presente_con_fixture_da_resultado_esperado() {
        let t = tarea_con("rust", "");
        let ej = con_reporte(SIN_ASSERTS);
        let r = sonda_mutacion(&ej, &t);
        let valor = r.sonda.valor.expect("valor");
        assert!((valor - 2.0 / 9.0).abs() < 1e-6);
        assert_eq!(r.sonda.hallazgos.len(), 7);
        assert!(ej.cmds.borrow().iter().any(|c| c.contains("cargo mutants")));
    }

    #[test]
    fn mutacion_lenguaje_no_rust_da_sin_veredicto() {
        let t = tarea_con("python", "");
        let ej = con_reporte(CON_ASSERTS);
        let r = sonda_mutacion(&ej, &t);
        assert_eq!(r.sonda.valor, None);
        assert!(ej.cmds.borrow().is_empty(), "no corre nada para Python");
    }

    #[test]
    fn mutacion_sin_reporte_da_sin_veredicto() {
        let t = tarea_con("rust", "");
        let r = sonda_mutacion(&sin_reporte(), &t);
        assert_eq!(r.sonda.valor, None);
        assert!(
            r.sonda
                .motivo
                .unwrap_or_default()
                .contains("no dejó reporte")
        );
    }

    #[test]
    fn mutacion_reporte_roto_da_sin_veredicto() {
        let t = tarea_con("rust", "");
        let ej = con_reporte("{roto");
        let r = sonda_mutacion(&ej, &t);
        assert_eq!(r.sonda.valor, None);
        assert!(r.sonda.motivo.is_some());
    }

    /// Motivo de la sonda cuando el reporte no da veredicto. Falla si no hay motivo.
    fn motivo_de(json: &'static str) -> (Option<f32>, String) {
        let t = tarea_con("rust", "");
        let r = sonda_mutacion(&con_reporte(json), &t);
        (r.sonda.valor, r.sonda.motivo.unwrap_or_default())
    }

    #[test]
    fn mutacion_reporte_vacio_es_sin_veredicto() {
        let (valor, motivo) = motivo_de("");
        assert_eq!(valor, None);
        assert!(motivo.starts_with("no dejó reporte"), "{motivo}");
    }

    #[test]
    fn mutacion_reporte_truncado_es_sin_veredicto() {
        let (valor, motivo) = motivo_de(r#"{"outcomes":[{"scenario":"Baseline""#);
        assert_eq!(valor, None);
        assert!(motivo.contains("JSON truncado"), "{motivo}");
    }

    #[test]
    fn mutacion_sin_clave_outcomes_es_sin_veredicto() {
        let (valor, motivo) = motivo_de("{}");
        assert_eq!(valor, None);
        assert!(motivo.contains("sin marca de fin"), "{motivo}");
    }

    #[test]
    fn mutacion_lista_outcomes_vacia_es_sin_veredicto() {
        let (valor, motivo) = motivo_de(r#"{"outcomes":[]}"#);
        assert_eq!(valor, None);
        assert!(motivo.contains("sin marca de fin"), "{motivo}");
    }

    #[test]
    fn mutacion_sin_baseline_es_sin_veredicto() {
        let json = r#"{"outcomes":[{"scenario":{"Mutant":{"name":"m","file":"src/a.rs"}},"summary":"CaughtMutant"}]}"#;
        let (valor, motivo) = motivo_de(json);
        assert_eq!(valor, None);
        assert!(motivo.contains("sin marca de fin"), "{motivo}");
    }

    #[test]
    fn mutacion_reporte_valido_con_asserts_sigue_dando_uno() {
        let r = sonda_mutacion(&con_reporte(CON_ASSERTS), &tarea_con("rust", ""));
        assert_eq!(r.sonda.valor, Some(1.0));
        assert_eq!(r.sonda.motivo, None);
    }

    #[test]
    fn mutacion_reporte_con_2_mutantes_de_50_es_reporte_truncado() {
        let json = r#"{"total_mutants":50,"outcomes":[{"scenario":"Baseline","summary":"Success"},{"scenario":{"Mutant":{"name":"m1","file":"src/a.rs","span":{"start":{"line":3}}}},"summary":"CaughtMutant"},{"scenario":{"Mutant":{"name":"m2","file":"src/a.rs","span":{"start":{"line":4}}}},"summary":"MissedMutant"}]}"#;
        let (valor, motivo) = motivo_de(json);
        assert_eq!(valor, None);
        assert!(motivo.contains("reporte truncado"), "{motivo}");
    }

    #[test]
    fn mutacion_reporte_sin_total_mutants_con_mutantes_es_reporte_truncado() {
        let json = r#"{"outcomes":[{"scenario":"Baseline","summary":"Success"},{"scenario":{"Mutant":{"name":"m1","file":"src/a.rs"}},"summary":"CaughtMutant"}]}"#;
        let (valor, motivo) = motivo_de(json);
        assert_eq!(valor, None);
        assert!(motivo.contains("reporte truncado"), "{motivo}");
    }

    /// Cinco mutantes viables y uno `Unviable`, con dos muertos (uno `Timeout`) y dos vivos.
    const MUERTOS_MIXTOS: &str = r#"{"total_mutants":5,"outcomes":[
        {"scenario":"Baseline","summary":"Success"},
        {"scenario":{"Mutant":{"name":"a1","file":"src/a.rs","span":{"start":{"line":1}}}},"summary":"CaughtMutant"},
        {"scenario":{"Mutant":{"name":"a2","file":"src/a.rs","span":{"start":{"line":2}}}},"summary":"Unviable"},
        {"scenario":{"Mutant":{"name":"b3","file":"src/b.rs","span":{"start":{"line":3}}}},"summary":"MissedMutant"},
        {"scenario":{"Mutant":{"name":"b4","file":"src/b.rs","span":{"start":{"line":4}}}},"summary":"Timeout"},
        {"scenario":{"Mutant":{"name":"c5","file":"src/c.rs","span":{"start":{"line":5}}}},"summary":"CaughtMutant"}]}"#;

    fn muerto(archivo: &str, linea: u32, nombre: &str) -> MutanteMuerto {
        MutanteMuerto {
            archivo: archivo.into(),
            linea,
            nombre: nombre.into(),
            tests: Vec::new(),
        }
    }

    #[test]
    fn mutantes_muertos_devuelve_los_muertos_de_la_muestra_en_orden() {
        let muertos = mutantes_muertos(MUERTOS_MIXTOS, 60, "");
        assert_eq!(
            muertos,
            vec![
                muerto("src/a.rs", 1, "a1"),
                muerto("src/b.rs", 4, "b4"),
                muerto("src/c.rs", 5, "c5"),
            ]
        );
    }

    #[test]
    fn mutantes_muertos_recorta_a_la_muestra_de_mutantes_max() {
        // Los viables en orden: a1 (muerto), b3 (vivo), b4 (muerto), c5 (muerto).
        assert_eq!(
            mutantes_muertos(MUERTOS_MIXTOS, 1, ""),
            vec![muerto("src/a.rs", 1, "a1")]
        );
        assert_eq!(
            mutantes_muertos(MUERTOS_MIXTOS, 3, ""),
            vec![muerto("src/a.rs", 1, "a1"), muerto("src/b.rs", 4, "b4")]
        );
    }

    #[test]
    fn mutantes_muertos_coincide_con_los_muertos_de_parsear_outcomes() {
        let muertos = mutantes_muertos(SIN_ASSERTS, 60, "");
        let r = parsear_outcomes(SIN_ASSERTS, 60, &[]).expect("parsea");
        assert_eq!(muertos.len(), r.muertos as usize);
        assert!(muertos.iter().all(|m| m.archivo == "src/calc.rs"));
    }

    #[test]
    fn mutantes_muertos_json_ilegible_vacio_o_sin_veredicto_es_vacio() {
        assert!(mutantes_muertos("", 60, "").is_empty());
        assert!(mutantes_muertos("{no es json", 60, "").is_empty());
        let sin_baseline = r#"{"outcomes":[{"scenario":{"Mutant":{"name":"m","file":"src/a.rs"}},"summary":"CaughtMutant"}]}"#;
        assert!(mutantes_muertos(sin_baseline, 60, "").is_empty());
        assert!(mutantes_muertos(MUERTOS_MIXTOS, 0, "").is_empty());
    }

    const CON_LOGS: &str = include_str!("../../tests/fixtures/mutacion/outcomes-con-logs.json");
    const TESTS_FALLIDOS: &str = include_str!("../../tests/fixtures/mutacion/tests-fallidos.txt");

    #[test]
    fn mutantes_muertos_cruza_log_path_con_los_tests_fallidos() {
        let muertos = mutantes_muertos(CON_LOGS, 60, TESTS_FALLIDOS);
        assert_eq!(muertos.len(), 3, "{muertos:?}");
        assert_eq!(muertos[0].tests, vec!["t::ok"]);
        assert_eq!(
            muertos[2].tests,
            vec!["db::tests::zset", "db::tests::zset_vacio"]
        );
    }

    #[test]
    fn mutantes_muertos_timeout_queda_con_tests_vacios() {
        let muertos = mutantes_muertos(CON_LOGS, 60, TESTS_FALLIDOS);
        let timeout = &muertos[1];
        assert_eq!(
            timeout.nombre,
            "src/lib.rs:9:5: replace resta -> i32 with 0"
        );
        assert!(timeout.tests.is_empty(), "{timeout:?}");
    }

    #[test]
    fn mutantes_muertos_sin_log_path_o_sin_entrada_da_tests_vacios() {
        let json = r#"{"total_mutants":1,"outcomes":[{"scenario":"Baseline","summary":"Success"},{"scenario":{"Mutant":{"name":"m","file":"src/a.rs","span":{"start":{"line":2}}}},"summary":"CaughtMutant"}]}"#;
        let muertos = mutantes_muertos(json, 60, TESTS_FALLIDOS);
        assert_eq!(muertos.len(), 1);
        assert!(muertos[0].tests.is_empty(), "{muertos:?}");
    }

    #[test]
    fn mutantes_muertos_tests_en_orden_del_archivo_sin_duplicados() {
        let json = r#"{"total_mutants":1,"outcomes":[{"scenario":"Baseline","summary":"Success"},{"scenario":{"Mutant":{"name":"m","file":"src/a.rs","span":{"start":{"line":2}}}},"summary":"CaughtMutant","log_path":"log/a.log"}]}"#;
        let texto = "log/a.log:test z::uno ... FAILED\nlog/a.log:test a::dos ... FAILED\n\
                     log/a.log:test z::uno ... FAILED\n";
        let muertos = mutantes_muertos(json, 60, texto);
        assert_eq!(muertos[0].tests, vec!["z::uno", "a::dos"]);
    }

    #[test]
    fn mutacion_comando_de_corrida_deja_tests_fallidos_antes_del_cp() {
        let cmd = comando_de_corrida(&tarea_pruebas(5), "");
        assert!(
            cmd.contains(
                "(cd /salida/mutants/mutants.out && grep -H '^test .* \\.\\.\\. FAILED$' log/*.log) > /salida/tests-fallidos.txt 2>/dev/null;"
            ),
            "{cmd}"
        );
        let grep = cmd.find("grep -H").expect("tiene el grep");
        let cp = cmd
            .find("cp /salida/mutants/mutants.out/outcomes.json")
            .expect("tiene el cp");
        assert!(grep < cp, "el grep va antes del cp: {cmd}");
        assert!(
            cmd.trim_end()
                .ends_with("/salida/outcomes.json 2>/dev/null"),
            "termina con el cp: {cmd}"
        );
        assert!(es_comando_de_mutacion(&cmd), "{cmd}");
    }

    /// Mutante de la muestra con su estado, para los tests de `mutantes_cambiados`.
    fn est(nombre: &str, estado: EstadoMutante) -> MutanteEstado {
        MutanteEstado {
            nombre: nombre.into(),
            estado,
        }
    }

    #[test]
    fn mutacion_muestra_da_muerto_vivo_y_vencido_en_orden() {
        // Los viables en orden: a1 caza, a2 no compila (no cuenta), b3 se escapa, b4 agota el tiempo, c5 caza.
        let r = parsear_outcomes(MUERTOS_MIXTOS, 60, &[]).expect("parsea");
        assert_eq!(
            r.mutantes,
            vec![
                est("a1", EstadoMutante::Muerto),
                est("b3", EstadoMutante::Vivo),
                est("b4", EstadoMutante::Vencido),
                est("c5", EstadoMutante::Muerto),
            ]
        );
    }

    #[test]
    fn mutacion_sonda_deja_su_muestra_y_tomar_muestra_la_saca() {
        let r = sonda_mutacion(&con_reporte(MUERTOS_MIXTOS), &tarea_pruebas(60));
        assert_eq!(r.mutantes.len(), 4, "{:?}", r.mutantes);
        assert_eq!(tomar_muestra(), r.mutantes);
        assert!(tomar_muestra().is_empty(), "tras tomarla queda vacía");
        // Una sonda sin herramienta también sobrescribe la muestra: queda vacía.
        let _ = sonda_mutacion(&con_reporte(MUERTOS_MIXTOS), &tarea_pruebas(60));
        let _ = sonda_mutacion(&falta_la_herramienta(), &tarea_pruebas(60));
        assert!(tomar_muestra().is_empty(), "la sonda sin muestra la vacía");
    }

    #[test]
    fn mutacion_mutantes_cambiados_omite_el_elegido_y_lista_el_resto() {
        // `x` pasa de muerto a vivo, `y` de vencido a muerto, `q` solo está en la base, `w` solo en el caso.
        let base = vec![
            est("x", EstadoMutante::Muerto),
            est("y", EstadoMutante::Vencido),
            est("z", EstadoMutante::Vivo),
            est("q", EstadoMutante::Muerto),
        ];
        let caso = vec![
            est("x", EstadoMutante::Vivo),
            est("y", EstadoMutante::Muerto),
            est("z", EstadoMutante::Vivo),
            est("w", EstadoMutante::Vivo),
        ];
        assert_eq!(mutantes_cambiados(&base, &caso, Some("x")), vec!["y"]);
        assert_eq!(mutantes_cambiados(&base, &caso, Some("y")), vec!["x"]);
        assert_eq!(mutantes_cambiados(&base, &caso, None), vec!["x", "y"]);
        assert!(mutantes_cambiados(&base, &base, None).is_empty());
    }

    #[test]
    fn mutacion_puntaje_no_cambia_con_el_estado_vencido() {
        // `Vencido` cuenta como muerto: el valor sale igual que antes del estado nuevo.
        let r = parsear_outcomes(MUERTOS_MIXTOS, 60, &[]).expect("parsea");
        assert_eq!((r.muertos, r.vivos, r.muestra), (3, 1, 4));
        assert_eq!(r.sonda.valor, Some(0.75));
        let r = parsear_outcomes(SIN_ASSERTS, 60, &[]).expect("parsea");
        assert_eq!((r.muertos, r.vivos, r.muestra), (2, 7, 9));
        assert!((r.sonda.valor.expect("valor") - 2.0 / 9.0).abs() < 1e-6);
    }

    /// Reporte con Baseline de fases Build 3.0 y Test 12.5, y un mutante cazado.
    const BASE_CON_FASES: &str = r#"{"total_mutants":1,"outcomes":[
        {"scenario":"Baseline","summary":"Success","phase_results":[
            {"phase":"Build","duration":3.0,"summary":"Success"},
            {"phase":"Test","duration":12.5,"summary":"Success"}]},
        {"scenario":{"Mutant":{"name":"a1","file":"src/a.rs","span":{"start":{"line":1}}}},"summary":"CaughtMutant"}]}"#;

    #[test]
    fn mutacion_plazo_de_base_multiplica_con_piso_y_redondea_hacia_arriba() {
        assert_eq!(plazo_de_base(10.0), 50);
        assert_eq!(plazo_de_base(1.0), 20, "el mínimo de 20 segundos");
        assert_eq!(plazo_de_base(4.2), 21, "5 por 4,2 da 21 y no pasa de ahí");
    }

    #[test]
    fn mutacion_parsear_outcomes_lee_la_duracion_de_test_del_baseline() {
        let r = parsear_outcomes(BASE_CON_FASES, 60, &[]).expect("parsea");
        assert_eq!(r.base_segundos, Some(12.5));
        let sin_fases = parsear_outcomes(MUERTOS_MIXTOS, 60, &[]).expect("parsea");
        assert_eq!(
            sin_fases.base_segundos, None,
            "sin phase_results no hay duración"
        );
    }

    #[test]
    fn mutacion_sonda_deja_la_duracion_de_base_y_tomar_la_saca() {
        let _ = sonda_mutacion(&con_reporte(BASE_CON_FASES), &tarea_pruebas(60));
        assert_eq!(tomar_base_segundos(), Some(12.5));
        assert_eq!(tomar_base_segundos(), None, "tras tomarla queda vacía");
        let _ = sonda_mutacion(&con_reporte(BASE_CON_FASES), &tarea_pruebas(60));
        let _ = sonda_mutacion(&falta_la_herramienta(), &tarea_pruebas(60));
        assert_eq!(tomar_base_segundos(), None, "la sonda sin reporte la vacía");
    }

    #[test]
    fn mutacion_con_plazo_fijo_el_comando_lleva_timeout_antes_de_output() {
        let cmd = con_plazo_fijo(Some(75), || comando_de_corrida(&tarea_pruebas(5), ""));
        assert!(
            cmd.contains("--jobs 1 --timeout 75 --output /salida/mutants"),
            "{cmd}"
        );
        assert!(es_comando_de_mutacion(&cmd), "{cmd}");
    }

    #[test]
    fn mutacion_sin_plazo_fijo_el_comando_no_lleva_timeout() {
        let t = tarea_pruebas(5);
        assert!(!comando_de_corrida(&t, "").contains("--timeout"));
        let dentro = con_plazo_fijo(Some(75), || comando_de_corrida(&t, ""));
        assert!(dentro.contains("--timeout 75"), "{dentro}");
        assert!(
            !comando_de_corrida(&t, "").contains("--timeout"),
            "tras salir vuelve al plazo automático"
        );
        let panico = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            con_plazo_fijo(Some(75), || panic!("a propósito"));
        }));
        assert!(panico.is_err());
        assert!(
            !comando_de_corrida(&t, "").contains("--timeout"),
            "tras un pánico también vuelve"
        );
    }
}
