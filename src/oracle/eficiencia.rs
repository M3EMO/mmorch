//! Eficiencia del repo evaluado contra su estado inicial (`bootstrap/`). Mide cuatro cosas:
//! tiempo de tests, pico de memoria, tiempo de compilación y tamaño de los binarios.
//! Cada lado corre en su propio ejecutor. `comparar` hace toda la aritmética.

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

/// Como `comparar`, pero la referencia suma el `ancla`: una corrida del lado inicial, si hay.
pub fn comparar_con_ancla(base: &[f64], ancla: Option<f64>, repo: &[f64]) -> Option<Comparacion> {
    let referencia: Vec<f64> = base.iter().copied().chain(ancla).collect();
    comparar(&referencia, repo)
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

/// Series de la corrida del lado inicial que ancla la referencia. Sin ancla quedan vacías y sin error.
/// Un ancla con error no hace fallar la medida: `combinar` usa solo la base.
struct Ancla {
    tiempo: Serie,
    memoria: Serie,
    compilacion: Serie,
}

impl Ancla {
    fn vacia() -> Self {
        Ancla {
            tiempo: Ok(vec![]),
            memoria: Ok(vec![]),
            compilacion: Ok(vec![]),
        }
    }
}

/// Valor de una serie de ancla. Una serie con error o sin valor no da ninguno.
fn valor_del_ancla(ancla: &Serie) -> Option<f64> {
    ancla.as_ref().ok()?.first().copied()
}

/// Compara las dos series de una medida. El ancla suma a la referencia. Si un lado falla, la medida no tiene valor.
fn combinar(medida: Medida, referencia: &Serie, ancla: &Serie, repo: &Serie) -> ResultadoMedida {
    match (referencia, repo) {
        (Ok(r), Ok(p)) => {
            let ancla = valor_del_ancla(ancla);
            match comparar_con_ancla(r, ancla, p) {
                Some(comparacion) => ResultadoMedida {
                    medida,
                    referencia: r.iter().copied().chain(ancla).reduce(f64::min),
                    repo: p.iter().copied().reduce(f64::min),
                    comparacion: Some(comparacion),
                    motivo: None,
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

/// Mide tiempo y memoria de los tests. Compila los tests de cada lado que corre, y luego corre esos lados
/// alternados, `CORRIDAS` veces. Un lado que no compila no corre. Si `lados[k]` es `None`, ese lado no corre:
/// su serie ya viene en `tiempo[k]` y `memoria[k]`. Con `filtro` con error, los dos lados quedan con ese error.
/// Si `ancla_lado` es `Some`, corre una vez tras la primera corrida de `lados`. Su serie va a `ancla`.
fn medir_tests(
    lados: [Option<&Lado>; 2],
    ancla_lado: Option<&Lado>,
    filtro: &Result<String, String>,
    tiempo: &mut [Serie; 2],
    memoria: &mut [Serie; 2],
    ancla: &mut Ancla,
) {
    let filtro = match filtro {
        Ok(filtro) => filtro,
        Err(motivo) => {
            *tiempo = [Err(motivo.clone()), Err(motivo.clone())];
            *memoria = [Err(motivo.clone()), Err(motivo.clone())];
            return;
        }
    };
    if let Some(lado) = ancla_lado
        && let Err(motivo) = compilar_tests(lado)
    {
        anotar(&mut ancla.tiempo, Err(motivo.clone()));
        anotar(&mut ancla.memoria, Err(motivo));
    }
    for (k, lado) in lados.into_iter().enumerate() {
        let Some(lado) = lado else { continue };
        if let Err(motivo) = compilar_tests(lado) {
            anotar(&mut tiempo[k], Err(motivo.clone()));
            anotar(&mut memoria[k], Err(motivo));
        }
    }
    for corrida in 0..CORRIDAS {
        for (k, lado) in lados.into_iter().enumerate() {
            let Some(lado) = lado else { continue };
            if tiempo[k].is_err() && memoria[k].is_err() {
                continue;
            }
            correr_tests(lado, filtro, &mut tiempo[k], &mut memoria[k]);
        }
        if corrida == 0
            && let Some(lado) = ancla_lado
            && (ancla.tiempo.is_ok() || ancla.memoria.is_ok())
        {
            correr_tests(lado, filtro, &mut ancla.tiempo, &mut ancla.memoria);
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
/// Si `ancla_lado` es `Some`, corre una compilación tras la primera corrida de `lados`. Su reloj va a
/// `ancla.compilacion`. Su tamaño no se mide: el tamaño sale solo de `lados`.
fn medir_compilacion(
    lados: [Option<&Lado>; 2],
    ancla_lado: Option<&Lado>,
    meta_i: &Result<Metadatos, String>,
    meta_r: &Result<Metadatos, String>,
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
        for corrida in 0..CORRIDAS {
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

/// Une las series de los dos lados en las cuatro medidas, en orden de `Medida`. El ancla suma a la
/// referencia de tiempo, memoria y compilación. El tamaño no tiene ancla.
fn combinar_medidas(
    tiempo: &[Serie; 2],
    memoria: &[Serie; 2],
    compilacion: &[Serie; 2],
    tamano: &[Serie; 2],
    ancla: &Ancla,
) -> Vec<ResultadoMedida> {
    let sin_ancla: Serie = Ok(vec![]);
    vec![
        combinar(Medida::TiempoTests, &tiempo[0], &ancla.tiempo, &tiempo[1]),
        combinar(
            Medida::MemoriaPico,
            &memoria[0],
            &ancla.memoria,
            &memoria[1],
        ),
        combinar(
            Medida::TiempoCompilacion,
            &compilacion[0],
            &ancla.compilacion,
            &compilacion[1],
        ),
        combinar(Medida::TamanoBinario, &tamano[0], &sin_ancla, &tamano[1]),
    ]
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

/// Lo que midió el lado inicial de una medición. Una calibración la mide una vez y sus casos la reusan.
/// `filtro` es el resultado de `preparar_filtro` de ese lado. Las series son las del lado inicial.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Referencia {
    meta: Result<Metadatos, String>,
    lista: Result<Listas, String>,
    filtro: Result<String, String>,
    tiempo: Serie,
    memoria: Serie,
    compilacion: Serie,
    tamano: Serie,
}

thread_local! {
    /// Referencia que midió la última medición de dos lados de este hilo. La toma `tomar_referencia`.
    static ULTIMA_REFERENCIA: RefCell<Option<Referencia>> = const { RefCell::new(None) };
    /// Referencia fijada que usa `medir_lados` en este hilo. `None`: mide los dos lados.
    static REFERENCIA_FIJA: RefCell<Option<Referencia>> = const { RefCell::new(None) };
}

/// Saca la referencia que midió la última medición de dos lados de este hilo. Deja el valor vacío.
pub(crate) fn tomar_referencia() -> Option<Referencia> {
    ULTIMA_REFERENCIA.with(|u| u.borrow_mut().take())
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
/// Si el borrado de esa carpeta falla, `avisos` recibe un aviso con la ruta y el error. Las medidas no cambian.
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
    let raiz = match crear_trabajo(tmp, borrar, avisos) {
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
    let resultados = medir_lados(&lado_i, &lado_r);
    // Un fallo de borrado no cambia las medidas: la carpeta queda y el siguiente `medir` la ignora.
    if let Err(e) = borrar(tmp, &raiz) {
        avisos.push(aviso_de_borrado(&raiz, &e));
    }
    resultados
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
fn medir_lados(inicial: &Lado, repo: &Lado) -> Vec<ResultadoMedida> {
    let fijada = REFERENCIA_FIJA.with(|r| r.borrow().clone());
    match fijada {
        Some(referencia) => medir_contra_referencia(&referencia, inicial, repo),
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
    medir_tests(lados, None, &filtro, &mut tiempo, &mut memoria, &mut ancla);
    medir_compilacion(
        lados,
        None,
        &meta_i,
        &meta_r,
        &mut compilacion,
        &mut tamano,
        &mut ancla,
    );
    let referencia = Referencia {
        meta: meta_i,
        lista: lista_i,
        filtro,
        tiempo: tiempo[0].clone(),
        memoria: memoria[0].clone(),
        compilacion: compilacion[0].clone(),
        tamano: tamano[0].clone(),
    };
    let resultados = combinar_medidas(&tiempo, &memoria, &compilacion, &tamano, &ancla);
    (resultados, referencia)
}

/// Mide el repo y lo compara con la referencia. Además corre el lado inicial una vez por medida: su
/// corrida ancla la referencia. Si el filtro del repo no es el de la referencia, los tests no tienen valor.
fn medir_contra_referencia(
    referencia: &Referencia,
    inicial: &Lado,
    repo: &Lado,
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
    let lados = [None, Some(repo)];
    let mut tiempo: [Serie; 2] = [referencia.tiempo.clone(), Ok(vec![])];
    let mut memoria: [Serie; 2] = [referencia.memoria.clone(), Ok(vec![])];
    let mut compilacion: [Serie; 2] = [referencia.compilacion.clone(), Ok(vec![])];
    let mut tamano: [Serie; 2] = [referencia.tamano.clone(), Ok(vec![])];
    let mut ancla = Ancla::vacia();
    medir_tests(
        lados,
        Some(inicial),
        &filtro,
        &mut tiempo,
        &mut memoria,
        &mut ancla,
    );
    medir_compilacion(
        lados,
        Some(inicial),
        &referencia.meta,
        &meta_r,
        &mut compilacion,
        &mut tamano,
        &mut ancla,
    );
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
    fn ancla_sube_el_ruido_y_quita_la_falsa_regresion() {
        let base = [1.00, 1.01, 1.00];
        let repo = [1.05, 1.06, 1.05];
        let sin = comparar(&base, &repo).expect("comparable");
        assert!(cerca(sin.razon, 1.05), "razon {}", sin.razon);
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
        for _ in 0..CORRIDAS {
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
    fn con_referencia_el_inicial_solo_recibe_su_ancla() {
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
        // Sin metadata ni lista: esas vienen de la referencia.
        assert_eq!(
            tipos_de(&entradas, "inicial"),
            ["compilar_tests", "tests", "build"]
        );
        assert!(
            entradas.iter().any(|e| e.starts_with("repo ")),
            "{entradas:?}"
        );
    }

    #[test]
    fn con_referencia_el_inicial_recibe_una_llamada_por_medida_de_tiempo() {
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
        let cuenta = |tipo: &str| tipos.iter().filter(|t| *t == tipo).count();
        assert_eq!(cuenta("tests"), 1, "{tipos:?}");
        assert_eq!(cuenta("build"), 1, "{tipos:?}");
        assert_eq!(cuenta("compilar_tests"), 1, "{tipos:?}");
    }

    #[test]
    fn con_referencia_el_orden_de_las_corridas_es_repo_inicial_repo_repo() {
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
        let esperado: Vec<String> = ["repo", "inicial", "repo", "repo"]
            .into_iter()
            .map(String::from)
            .collect();
        assert_eq!(lados_de(&entradas, "tests"), esperado, "{entradas:?}");
        assert_eq!(lados_de(&entradas, "build"), esperado, "{entradas:?}");
    }

    #[test]
    fn sin_referencia_el_orden_alterna_como_hoy() {
        let log = RefCell::new(Vec::new());
        let inicial = sano("inicial", &log, &["a"]);
        let repo = sano("repo", &log, &["a"]);
        medir_de_prueba(&inicial, &repo);
        let entradas = log.borrow();
        let esperado: Vec<String> = (0..CORRIDAS)
            .flat_map(|_| ["inicial", "repo"])
            .map(String::from)
            .collect();
        assert_eq!(lados_de(&entradas, "tests"), esperado, "{entradas:?}");
        assert_eq!(lados_de(&entradas, "build"), esperado, "{entradas:?}");
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
        let inicial = Falso {
            tests: "sin marcas\n".into(),
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

    #[test]
    fn con_referencia_la_razon_usa_la_serie_guardada() {
        let log_base = RefCell::new(Vec::new());
        let referencia = referencia_de(
            &sano("inicial", &log_base, &["a"]),
            &sano("repo", &log_base, &["a"]),
        );
        // El ancla del lado inicial mide más que la base. El mínimo sigue siendo el de la base.
        // La razón no cambia: el ruido sí, pero esta prueba no lo mira.
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
            (Medida::TiempoTests, 1.5, 3.0),
            (Medida::MemoriaPico, 1000.0, 2000.0),
            (Medida::TiempoCompilacion, 2.0, 4.0),
            (Medida::TamanoBinario, 500.0, 1000.0),
        ];
        for (medida, referencia, repo) in casos {
            let r = por_medida(&resultados, medida);
            assert_eq!(r.referencia, Some(referencia), "{medida:?}");
            assert_eq!(r.repo, Some(repo), "{medida:?}");
            let razon = r.comparacion.as_ref().map(|c| c.razon);
            assert!(razon.is_some_and(|x| cerca(x, 2.0)), "{medida:?}: {r:?}");
        }
        // Solo el ancla del lado inicial: compilación de tests, una corrida de tests y una de build.
        assert_eq!(
            tipos_de(&log.borrow(), "inicial"),
            ["compilar_tests", "tests", "build"]
        );
    }

    #[test]
    fn referencia_con_serie_fallida_deja_la_medida_sin_valor_con_el_motivo_de_la_base() {
        let log_base = RefCell::new(Vec::new());
        let sin_pico = "ROOT_NS 1500000000\nROOT_MEM \n";
        let referencia = referencia_de(
            &Falso {
                tests: sin_pico.into(),
                ..sano("inicial", &log_base, &["a"])
            },
            &sano("repo", &log_base, &["a"]),
        );
        let log = RefCell::new(Vec::new());
        let inicial = sano("inicial", &log, &["a"]);
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
}
