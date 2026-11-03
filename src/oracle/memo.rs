//! Memo de las sondas de la evaluación. Graba la salida cruda de cada comando del ejecutor real, para que
//! otra parte de la evaluación (la calibración, hoy) lea lo que midió la base sin volver a correrlo.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

use crate::oracle::ErrorOraculo;
use crate::oracle::mutacion::{self, MutanteMuerto};
use crate::oracle::perfiles::herramientas::{EjecutorHerramienta, SalidaHerramienta};
use crate::oracle::tarea::Tarea;

/// Salidas grabadas, por comando exacto. Un comando solo guarda su última salida OK.
pub struct MemoSondas {
    salidas: Mutex<BTreeMap<String, SalidaHerramienta>>,
}

impl MemoSondas {
    pub fn nuevo() -> Self {
        MemoSondas {
            salidas: Mutex::new(BTreeMap::new()),
        }
    }

    /// Graba `salida` bajo `cmd`. Un comando repetido reemplaza su salida anterior.
    pub fn anotar(&self, cmd: &str, salida: &SalidaHerramienta) {
        self.salidas
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(cmd.to_string(), salida.clone());
    }

    /// Salida grabada de `cmd`, si hay una.
    pub fn salida_de(&self, cmd: &str) -> Option<SalidaHerramienta> {
        self.salidas
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(cmd)
            .cloned()
    }

    /// Mutantes muertos de la muestra de la corrida de mutación de `tarea`, si está grabada.
    pub fn mutantes_muertos(&self, tarea: &Tarea) -> Vec<MutanteMuerto> {
        let Some(salida) = self.salida_de(&mutacion::construir_comando(tarea)) else {
            return Vec::new();
        };
        let Some(json) = salida.archivos.get(mutacion::ARCHIVO_OUTCOMES) else {
            return Vec::new();
        };
        mutacion::mutantes_muertos(json, mutacion::mutantes_max(tarea))
    }
}

impl Default for MemoSondas {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl fmt::Debug for MemoSondas {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MemoSondas").finish_non_exhaustive()
    }
}

/// Ejecutor que delega en `real` y graba cada salida OK en `memo`.
pub struct EjecutorGrabador<'a> {
    pub real: &'a dyn EjecutorHerramienta,
    pub memo: &'a MemoSondas,
}

impl EjecutorHerramienta for EjecutorGrabador<'_> {
    fn ejecutar(&self, cmd: &str) -> Result<SalidaHerramienta, ErrorOraculo> {
        let salida = self.real.ejecutar(cmd)?;
        self.memo.anotar(cmd, &salida);
        Ok(salida)
    }

    fn tiene_rustsec(&self) -> bool {
        self.real.tiene_rustsec()
    }
}

/// Qué sondas toma el reuso. Un alcance nuevo suma una variante y un brazo en `EjecutorReusador::toma`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alcance {
    /// Solo la corrida de mutación: su salida sale del memo, sin correrla.
    SoloMutacion,
}

/// Reuso de sondas: el memo de la base y el alcance de lo que toma del memo en vez de correrlo.
/// Solo vale si la sonda es determinista con la misma entrada.
#[derive(Debug, Clone)]
pub struct ReusoSondas {
    pub memo: Arc<MemoSondas>,
    pub alcance: Alcance,
}

/// Ejecutor que devuelve la salida grabada de lo que `reuso.alcance` toma, y delega el resto en `real`.
pub struct EjecutorReusador<'a> {
    pub real: &'a dyn EjecutorHerramienta,
    pub reuso: &'a ReusoSondas,
}

impl EjecutorReusador<'_> {
    /// Si el alcance toma `cmd`.
    fn toma(&self, cmd: &str) -> bool {
        match self.reuso.alcance {
            Alcance::SoloMutacion => es_de_mutacion(cmd),
        }
    }
}

impl EjecutorHerramienta for EjecutorReusador<'_> {
    fn ejecutar(&self, cmd: &str) -> Result<SalidaHerramienta, ErrorOraculo> {
        if self.toma(cmd)
            && let Some(salida) = self.reuso.memo.salida_de(cmd)
        {
            return Ok(salida);
        }
        self.real.ejecutar(cmd)
    }

    fn tiene_rustsec(&self) -> bool {
        self.real.tiene_rustsec()
    }
}

// unificar con mutacion::es_comando_de_mutacion
/// Si `cmd` es la corrida de mutación: `construir_comando` la arma con `cargo mutants` y deja el reporte.
fn es_de_mutacion(cmd: &str) -> bool {
    cmd.contains("cargo mutants") && cmd.contains(mutacion::ARCHIVO_OUTCOMES)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::fs;

    /// Ejecutor falso: responde `salida` a cada comando y registra los comandos.
    struct Falso {
        cmds: RefCell<Vec<String>>,
        salida: SalidaHerramienta,
        falla: bool,
    }

    impl EjecutorHerramienta for Falso {
        fn ejecutar(&self, cmd: &str) -> Result<SalidaHerramienta, ErrorOraculo> {
            self.cmds.borrow_mut().push(cmd.to_string());
            if self.falla {
                Err(ErrorOraculo::Infra("podman caído".into()))
            } else {
                Ok(self.salida.clone())
            }
        }

        fn tiene_rustsec(&self) -> bool {
            true
        }
    }

    fn falso(salida: SalidaHerramienta, falla: bool) -> Falso {
        Falso {
            cmds: RefCell::new(Vec::new()),
            salida,
            falla,
        }
    }

    fn con_outcomes(json: &str) -> SalidaHerramienta {
        let mut s = SalidaHerramienta {
            codigo: 0,
            stdout: "salida".into(),
            ..Default::default()
        };
        s.archivos.insert("outcomes.json".into(), json.into());
        s
    }

    fn tarea_rust() -> Tarea {
        let dir = tempfile::tempdir().expect("temp dir");
        let texto = "nombre = \"x\"\nlenguaje = \"rust\"\nimagen = \"i\"\nspec = \"s\"\n\
             [contrato]\ncmd = \"c\"\n[oculto]\ncmd = \"o\"\n";
        fs::write(dir.path().join("tarea.toml"), texto).expect("escribe tarea.toml");
        Tarea::cargar_de(dir.path()).expect("carga la tarea")
    }

    #[test]
    fn memo_graba_la_salida_ok_del_ejecutor_real() {
        let real = falso(con_outcomes("{}"), false);
        let memo = MemoSondas::nuevo();
        let grabador = EjecutorGrabador {
            real: &real,
            memo: &memo,
        };
        let salida = grabador.ejecutar("cmd-a").expect("ejecuta");
        assert_eq!(memo.salida_de("cmd-a"), Some(salida));
        assert_eq!(real.cmds.borrow().as_slice(), ["cmd-a".to_string()]);
    }

    #[test]
    fn memo_no_graba_un_error_del_ejecutor_real() {
        let real = falso(SalidaHerramienta::default(), true);
        let memo = MemoSondas::nuevo();
        let grabador = EjecutorGrabador {
            real: &real,
            memo: &memo,
        };
        assert!(grabador.ejecutar("cmd-a").is_err());
        assert_eq!(memo.salida_de("cmd-a"), None);
    }

    #[test]
    fn memo_delega_tiene_rustsec_en_el_ejecutor_real() {
        let real = falso(SalidaHerramienta::default(), false);
        let memo = MemoSondas::nuevo();
        let grabador = EjecutorGrabador {
            real: &real,
            memo: &memo,
        };
        assert!(grabador.tiene_rustsec());
    }

    #[test]
    fn memo_salida_de_comando_no_grabado_es_none() {
        let memo = MemoSondas::default();
        assert_eq!(memo.salida_de("nada"), None);
    }

    #[test]
    fn memo_mutantes_muertos_lee_lo_grabado_de_la_corrida_de_mutacion() {
        const OUTCOMES: &str = r#"{"total_mutants":2,"outcomes":[
            {"scenario":"Baseline","summary":"Success"},
            {"scenario":{"Mutant":{"name":"vivo","file":"src/a.rs","span":{"start":{"line":3}}}},"summary":"MissedMutant"},
            {"scenario":{"Mutant":{"name":"muerto","file":"src/a.rs","span":{"start":{"line":7}}}},"summary":"CaughtMutant"}]}"#;
        let tarea = tarea_rust();
        let memo = MemoSondas::nuevo();
        memo.anotar(
            &crate::oracle::mutacion::construir_comando(&tarea),
            &con_outcomes(OUTCOMES),
        );
        let muertos = memo.mutantes_muertos(&tarea);
        assert_eq!(
            muertos,
            vec![MutanteMuerto {
                archivo: "src/a.rs".into(),
                linea: 7,
                nombre: "muerto".into(),
            }]
        );
    }

    #[test]
    fn memo_mutantes_muertos_sin_corrida_grabada_es_vacio() {
        let tarea = tarea_rust();
        let memo = MemoSondas::nuevo();
        assert!(memo.mutantes_muertos(&tarea).is_empty());
    }

    fn reuso_de_mutacion(memo: &Arc<MemoSondas>) -> ReusoSondas {
        ReusoSondas {
            memo: memo.clone(),
            alcance: Alcance::SoloMutacion,
        }
    }

    #[test]
    fn reusador_devuelve_la_mutacion_grabada_sin_correrla() {
        let tarea = tarea_rust();
        let cmd = crate::oracle::mutacion::construir_comando(&tarea);
        let grabada = con_outcomes("{\"grabada\":true}");
        let memo = Arc::new(MemoSondas::nuevo());
        memo.anotar(&cmd, &grabada);
        let real = falso(SalidaHerramienta::default(), false);
        let reuso = reuso_de_mutacion(&memo);
        let reusador = EjecutorReusador {
            real: &real,
            reuso: &reuso,
        };
        assert_eq!(reusador.ejecutar(&cmd).expect("reusa"), grabada);
        assert!(
            real.cmds.borrow().is_empty(),
            "no debe correr: {:?}",
            real.cmds
        );
    }

    #[test]
    fn reusador_comando_que_no_es_de_mutacion_llama_al_real() {
        let memo = Arc::new(MemoSondas::nuevo());
        let real_salida = SalidaHerramienta {
            codigo: 0,
            stdout: "real".into(),
            ..Default::default()
        };
        let real = falso(real_salida.clone(), false);
        let reuso = reuso_de_mutacion(&memo);
        let reusador = EjecutorReusador {
            real: &real,
            reuso: &reuso,
        };
        let cmds = ["cargo clippy --all-targets", "command -v gitleaks"];
        for cmd in cmds {
            memo.anotar(cmd, &con_outcomes("{}"));
            assert_eq!(reusador.ejecutar(cmd).expect("delega"), real_salida);
        }
        assert_eq!(
            real.cmds.borrow().as_slice(),
            cmds.map(String::from).as_slice()
        );
    }

    #[test]
    fn reusador_sin_salida_grabada_llama_al_real() {
        let tarea = tarea_rust();
        let cmd = crate::oracle::mutacion::construir_comando(&tarea);
        let memo = Arc::new(MemoSondas::nuevo());
        let real = falso(con_outcomes("{}"), false);
        let reuso = reuso_de_mutacion(&memo);
        let reusador = EjecutorReusador {
            real: &real,
            reuso: &reuso,
        };
        reusador.ejecutar(&cmd).expect("delega");
        assert_eq!(real.cmds.borrow().as_slice(), [cmd]);
    }

    #[test]
    fn reusador_delega_tiene_rustsec_en_el_real() {
        let memo = Arc::new(MemoSondas::nuevo());
        let real = falso(SalidaHerramienta::default(), false);
        let reuso = reuso_de_mutacion(&memo);
        let reusador = EjecutorReusador {
            real: &real,
            reuso: &reuso,
        };
        assert!(reusador.tiene_rustsec());
    }
}
