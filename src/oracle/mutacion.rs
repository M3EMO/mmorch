//! Calidad de los tests del proyecto por mutación (Tarea 9e). Corre `cargo-mutants` sobre una copia
//! del repo y mide qué cambios de comportamiento detectan los tests. El puntaje es la proporción
//! de mutantes muertos. Si la herramienta falta en la imagen, no hay veredicto (fail-closed).

use serde::{Deserialize, Serialize};

use crate::oracle::ErrorOraculo;
use crate::oracle::descarga::PRELUDIO;
use crate::oracle::perfiles::herramientas::{
    EjecutorHerramienta, con_cola, falta_herramienta, sin_veredicto,
};
use crate::oracle::perfiles::{ResultadoSonda, crear_hallazgo};
use crate::oracle::tarea::{MINUTOS_MAX_DEFECTO, MUTANTES_MAX_DEFECTO, Requisito, Tarea};
use crate::tipos::Dimension;

const DIMENSION: Dimension = Dimension::Pruebas;
const SONDA: &str = "cargo-mutants";
/// Reporte que deja el comando dentro de `/salida`. Ver `construir_comando`.
const ARCHIVO_OUTCOMES: &str = "outcomes.json";
const DETALLE_VIVO: &str = "este cambio de comportamiento no lo detecta ningún test";
const SEVERIDAD_VIVO: u8 = 2;

/// Resultado de la sonda de mutación. `muestra` es cuántos mutantes se midieron.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultadoMutacion {
    pub sonda: ResultadoSonda,
    pub muertos: u32,
    pub vivos: u32,
    pub muestra: u32,
    /// Mutantes viables del repo, si la muestra quedó corta.
    pub total_conocido: Option<u32>,
    pub truncado: bool,
}

/// Reporte `outcomes.json` de cargo-mutants. Solo se leen los campos que Root usa.
#[derive(Deserialize)]
struct Informe {
    #[serde(default)]
    outcomes: Vec<Resultado>,
}

#[derive(Deserialize)]
struct Resultado {
    scenario: Escenario,
    #[serde(default)]
    summary: String,
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

/// Lee el reporte de mutantes. Mide los primeros `mutantes_max` viables, en el orden del archivo.
pub fn parsear_outcomes(
    json: &str,
    mutantes_max: u32,
    requisitos: &[Requisito],
) -> Result<ResultadoMutacion, ErrorOraculo> {
    if mutantes_max == 0 {
        return Err(ErrorOraculo::Infra("mutantes_max debe ser 1 o más".into()));
    }
    let informe: Informe = serde_json::from_str(json)
        .map_err(|e| ErrorOraculo::Infra(format!("outcomes.json ilegible: {e}")))?;
    // Cada viable: el mutante y si lo detectó algún test. `Unviable` no compila: no cuenta.
    let mut viables: Vec<(&Mutante, bool)> = Vec::new();
    for resultado in &informe.outcomes {
        match &resultado.scenario {
            Escenario::Baseline => match resultado.summary.as_str() {
                "Success" => {}
                "Failure" => {
                    return Ok(desde_sonda(sin_veredicto(
                        DIMENSION,
                        "los tests del proyecto fallan sin mutaciones",
                    )));
                }
                otro => {
                    return Err(ErrorOraculo::Infra(format!(
                        "resumen de Baseline desconocido: {otro}"
                    )));
                }
            },
            Escenario::Mutant(mutante) => match resultado.summary.as_str() {
                "Unviable" => {}
                "CaughtMutant" | "Timeout" => viables.push((mutante, true)),
                "MissedMutant" => viables.push((mutante, false)),
                otro => {
                    return Err(ErrorOraculo::Infra(format!(
                        "resumen de mutante desconocido: {otro}"
                    )));
                }
            },
        }
    }
    if viables.is_empty() {
        return Ok(desde_sonda(ResultadoSonda {
            dimension: DIMENSION,
            valor: None,
            motivo: Some("no hay mutantes viables para medir".into()),
            hallazgos: Vec::new(),
        }));
    }
    let total = u32::try_from(viables.len())
        .map_err(|_| ErrorOraculo::Infra("demasiados mutantes".into()))?;
    let muestra = &viables[..viables.len().min(mutantes_max as usize)];
    let muertos = muestra.iter().filter(|(_, muerto)| *muerto).count() as u32;
    let vivos = muestra.len() as u32 - muertos;
    let hallazgos = muestra
        .iter()
        .filter(|(_, muerto)| !muerto)
        .map(|(m, _)| {
            crear_hallazgo(
                DIMENSION,
                SEVERIDAD_VIVO,
                &m.file,
                m.span.start.line,
                &m.name,
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
    })
}

/// Comando de la corrida completa. Corre sobre una copia de `/repo` y deja `outcomes.json` en
/// `/salida`, porque el ejecutor solo lee archivos sueltos de esa carpeta.
pub fn construir_comando(tarea: &Tarea) -> String {
    let minutos = tarea
        .pruebas
        .as_ref()
        .map_or(MINUTOS_MAX_DEFECTO, |p| p.minutos_max);
    let segundos = u64::from(minutos) * 60;
    format!(
        "{PRELUDIO}cp -a /repo /salida/work && cd /salida/work && \
         CARGO_TARGET_DIR=/salida/target timeout {segundos} cargo mutants --no-shuffle --jobs 1 \
         --output /salida/mutants -- --offline; \
         cp /salida/mutants/mutants.out/outcomes.json /salida/{ARCHIVO_OUTCOMES} 2>/dev/null"
    )
}

/// Mutantes máximos de la muestra: los de `[pruebas]`, o el default.
fn mutantes_max(tarea: &Tarea) -> u32 {
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
    }
}

/// Sonda de mutación. Solo Rust. Sin `cargo-mutants` en la imagen, no corre el comando largo.
pub fn sonda_mutacion(ej: &dyn EjecutorHerramienta, tarea: &Tarea) -> ResultadoMutacion {
    if tarea.lenguaje != "rust" {
        return desde_sonda(sin_veredicto(
            DIMENSION,
            "la sonda de mutación solo existe para Rust por ahora",
        ));
    }
    if let Some(r) = falta_herramienta(ej, DIMENSION, SONDA) {
        return desde_sonda(r);
    }
    let salida = match ej.ejecutar(&construir_comando(tarea)) {
        Ok(s) => s,
        Err(e) => return desde_sonda(sin_veredicto(DIMENSION, e.to_string())),
    };
    let Some(json) = salida.archivos.get(ARCHIVO_OUTCOMES) else {
        let base = format!("cargo-mutants no dejó reporte (código {})", salida.codigo);
        return desde_sonda(sin_veredicto(DIMENSION, con_cola(base, &salida.stderr)));
    };
    parsear_outcomes(json, mutantes_max(tarea), &tarea.requisito)
        .unwrap_or_else(|e| desde_sonda(sin_veredicto(DIMENSION, e.to_string())))
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

    /// Responde "presente" a `command -v cargo-mutants` y `json` a la corrida.
    fn con_reporte(json: &'static str) -> Falso {
        Falso::nuevo(move |cmd| {
            if cmd.starts_with("command -v") {
                return Ok(SalidaHerramienta::default());
            }
            let mut s = SalidaHerramienta::default();
            s.archivos.insert(ARCHIVO_OUTCOMES.into(), json.into());
            Ok(s)
        })
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
        let json = format!(r#"{{"outcomes":[{}]}}"#, outs.join(","));
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
        let json = r#"{"outcomes":[{"scenario":"Baseline","summary":"Success"},
            {"scenario":{"Mutant":{"name":"m","file":"src/a.rs"}},"summary":"Unviable"}]}"#;
        let r = parsear_outcomes(json, 60, &[]).expect("parsea");
        assert_eq!(r.sonda.valor, None);
        assert_eq!(
            r.sonda.motivo.as_deref(),
            Some("no hay mutantes viables para medir")
        );
        assert_eq!(r.muestra, 0);
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
        let ej = Falso::nuevo(|cmd| {
            Ok(SalidaHerramienta {
                codigo: if cmd.starts_with("command -v") { 0 } else { 2 },
                ..Default::default()
            })
        });
        let r = sonda_mutacion(&ej, &t);
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
}
