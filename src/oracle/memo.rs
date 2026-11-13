//! Memo de las sondas de la evaluación. Graba la salida cruda de cada comando del ejecutor real, para que
//! otra parte de la evaluación (la calibración, hoy) lea lo que midió la base sin volver a correrlo.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

use crate::oracle::ErrorOraculo;
use crate::oracle::configs;
use crate::oracle::mutacion::{self, MutanteMuerto};
use crate::oracle::perfiles::herramientas::{self, EjecutorHerramienta, SalidaHerramienta};
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
    /// La corrida lleva su muestra en el comando, así que la salida se busca por predicado.
    pub fn mutantes_muertos(&self, tarea: &Tarea) -> Vec<MutanteMuerto> {
        let Some((cmd, salida)) = self.salida_de_mutacion() else {
            return Vec::new();
        };
        let Some(json) = salida.archivos.get(mutacion::ARCHIVO_OUTCOMES) else {
            return Vec::new();
        };
        let tests = salida
            .archivos
            .get(mutacion::ARCHIVO_TESTS_FALLIDOS)
            .map_or("", String::as_str);
        // La muestra pedida es la del `-F` del comando. Sin `-F` la corrida pidió todo el reporte.
        let pedidos = mutacion::pedidos_del_comando(&cmd)
            .unwrap_or_else(|| mutacion::nombres_del_reporte(json));
        mutacion::mutantes_muertos(json, &pedidos, mutacion::mutantes_max(tarea), tests)
    }

    /// Comando y salida grabados de la primera corrida de mutación, si hay una.
    fn salida_de_mutacion(&self) -> Option<(String, SalidaHerramienta)> {
        self.salidas
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .find(|(cmd, _)| mutacion::es_comando_de_mutacion(cmd))
            .map(|(cmd, salida)| (cmd.clone(), salida.clone()))
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

    fn huella_rustsec(&self) -> Option<String> {
        self.real.huella_rustsec()
    }
}

/// Qué sondas toma el reuso. Un alcance nuevo suma una variante y un brazo en `EjecutorReusador::toma`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alcance {
    /// Solo la corrida de mutación: su salida sale del memo, sin correrla. El listado de mutantes
    /// sigue corriendo: es barato y define el filtro `-F` que arma la clave de la corrida.
    SoloMutacion,
    /// La mutación (corrida y listado) y las sondas deterministas (gitleaks, cargo audit, clippy, hadolint
    /// y la verificación de sus herramientas). Nunca la Entrega ni la Eficiencia.
    Deterministas,
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
            Alcance::SoloMutacion => mutacion::es_comando_de_mutacion(cmd),
            Alcance::Deterministas => {
                mutacion::es_comando_de_mutacion(cmd)
                    || mutacion::es_comando_de_listado(cmd)
                    || herramientas::es_comando_determinista(cmd)
                    || configs::es_comando_determinista(cmd)
            }
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

    fn huella_rustsec(&self) -> Option<String> {
        self.real.huella_rustsec()
    }
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
                tests: vec![],
            }]
        );
    }

    #[test]
    fn memo_mutantes_muertos_lee_tests_fallidos_de_la_salida_grabada() {
        const OUTCOMES: &str = r#"{"total_mutants":1,"outcomes":[
            {"scenario":"Baseline","summary":"Success"},
            {"scenario":{"Mutant":{"name":"muerto","file":"src/a.rs","span":{"start":{"line":7}}}},"summary":"CaughtMutant","log_path":"log/a.log"}]}"#;
        let tarea = tarea_rust();
        let memo = MemoSondas::nuevo();
        let mut salida = con_outcomes(OUTCOMES);
        salida.archivos.insert(
            "tests-fallidos.txt".into(),
            "log/a.log:test ok::uno ... FAILED\nlog/a.log:test ok::dos ... FAILED\n".into(),
        );
        memo.anotar(&crate::oracle::mutacion::construir_comando(&tarea), &salida);
        let muertos = memo.mutantes_muertos(&tarea);
        assert_eq!(muertos[0].tests, vec!["ok::uno", "ok::dos"]);
    }

    #[test]
    fn memo_mutantes_muertos_pide_solo_la_muestra_del_filtro_grabado() {
        // El reporte trae dos muertos, pero la corrida grabada pidió solo `uno` con su `-F`.
        const OUTCOMES: &str = r#"{"total_mutants":2,"outcomes":[
            {"scenario":"Baseline","summary":"Success"},
            {"scenario":{"Mutant":{"name":"uno","file":"src/a.rs","span":{"start":{"line":3}}}},"summary":"CaughtMutant"},
            {"scenario":{"Mutant":{"name":"dos","file":"src/a.rs","span":{"start":{"line":7}}}},"summary":"CaughtMutant"}]}"#;
        let tarea = tarea_rust();
        let memo = MemoSondas::nuevo();
        let cmd = "cargo mutants --no-shuffle --jobs 1 --output /salida/mutants -F '^(?:uno)$' -- --offline";
        memo.anotar(cmd, &con_outcomes(OUTCOMES));
        let nombres: Vec<String> = memo
            .mutantes_muertos(&tarea)
            .into_iter()
            .map(|m| m.nombre)
            .collect();
        assert_eq!(nombres, vec!["uno".to_string()]);
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
        let cmds = [
            "cargo clippy --all-targets",
            "command -v gitleaks || exit 1",
        ];
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

    fn reuso_de_deterministas(memo: &Arc<MemoSondas>) -> ReusoSondas {
        ReusoSondas {
            memo: memo.clone(),
            alcance: Alcance::Deterministas,
        }
    }

    #[test]
    fn reusador_deterministas_devuelve_del_memo_las_sondas_deterministas_y_la_mutacion() {
        use crate::oracle::configs::comando_hadolint;
        use crate::oracle::perfiles::herramientas::{
            COMANDO_CARGO_AUDIT, COMANDO_GITLEAKS, comando_clippy, comando_de_herramienta,
        };
        let tarea = tarea_rust();
        let cmds = [
            COMANDO_GITLEAKS.to_string(),
            COMANDO_CARGO_AUDIT.to_string(),
            comando_clippy(),
            comando_hadolint("Dockerfile"),
            comando_de_herramienta("gitleaks"),
            comando_de_herramienta("cargo-audit"),
            comando_de_herramienta("clippy"),
            comando_de_herramienta("hadolint"),
            comando_de_herramienta("cargo-mutants"),
            crate::oracle::mutacion::construir_comando(&tarea),
        ];
        let grabada = con_outcomes("{\"grabada\":true}");
        let memo = Arc::new(MemoSondas::nuevo());
        for cmd in &cmds {
            memo.anotar(cmd, &grabada);
        }
        let real = falso(con_outcomes("{\"real\":true}"), false);
        let reuso = reuso_de_deterministas(&memo);
        let reusador = EjecutorReusador {
            real: &real,
            reuso: &reuso,
        };
        for cmd in &cmds {
            assert_eq!(reusador.ejecutar(cmd).expect("reusa"), grabada, "{cmd}");
        }
        assert!(
            real.cmds.borrow().is_empty(),
            "no debe correr: {:?}",
            real.cmds
        );
    }

    #[test]
    fn reusador_deterministas_entrega_y_eficiencia_llaman_al_real_aunque_haya_salida() {
        use crate::oracle::entrega::construir_script_con;
        let tarea = tarea_rust();
        let cmds = [
            construir_script_con(&tarea, true),
            construir_script_con(&tarea, false),
            "./bench --ops".to_string(),
        ];
        let memo = Arc::new(MemoSondas::nuevo());
        for cmd in &cmds {
            memo.anotar(cmd, &con_outcomes("{\"grabada\":true}"));
        }
        let real_salida = SalidaHerramienta {
            codigo: 0,
            stdout: "real".into(),
            ..Default::default()
        };
        let real = falso(real_salida.clone(), false);
        let reuso = reuso_de_deterministas(&memo);
        let reusador = EjecutorReusador {
            real: &real,
            reuso: &reuso,
        };
        for cmd in &cmds {
            assert_eq!(
                reusador.ejecutar(cmd).expect("delega"),
                real_salida,
                "{cmd}"
            );
        }
        assert_eq!(
            real.cmds.borrow().as_slice(),
            cmds.map(String::from).as_slice()
        );
    }

    #[test]
    fn reusador_deterministas_sin_salida_grabada_llama_al_real() {
        use crate::oracle::perfiles::herramientas::COMANDO_GITLEAKS;
        let memo = Arc::new(MemoSondas::nuevo());
        let real = falso(con_outcomes("{}"), false);
        let reuso = reuso_de_deterministas(&memo);
        let reusador = EjecutorReusador {
            real: &real,
            reuso: &reuso,
        };
        reusador.ejecutar(COMANDO_GITLEAKS).expect("delega");
        assert_eq!(real.cmds.borrow().as_slice(), [COMANDO_GITLEAKS]);
    }

    #[test]
    fn reusador_solo_mutacion_no_reusa_clippy_ni_gitleaks() {
        use crate::oracle::perfiles::herramientas::{COMANDO_GITLEAKS, comando_clippy};
        let memo = Arc::new(MemoSondas::nuevo());
        let clippy = comando_clippy();
        memo.anotar(&clippy, &con_outcomes("{}"));
        memo.anotar(COMANDO_GITLEAKS, &con_outcomes("{}"));
        let real = falso(con_outcomes("{\"real\":true}"), false);
        let reuso = reuso_de_mutacion(&memo);
        let reusador = EjecutorReusador {
            real: &real,
            reuso: &reuso,
        };
        reusador.ejecutar(&clippy).expect("delega");
        reusador.ejecutar(COMANDO_GITLEAKS).expect("delega");
        assert_eq!(
            real.cmds.borrow().as_slice(),
            [clippy, COMANDO_GITLEAKS.to_string()]
        );
    }

    #[test]
    fn reusador_deterministas_reusa_el_listado_de_mutantes() {
        use crate::oracle::mutacion::comando_de_listado;
        let cmd = comando_de_listado();
        let grabada = con_outcomes("{\"listado\":true}");
        let memo = Arc::new(MemoSondas::nuevo());
        memo.anotar(&cmd, &grabada);
        let real = falso(con_outcomes("{\"real\":true}"), false);
        let reuso = reuso_de_deterministas(&memo);
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
    fn reusador_solo_mutacion_no_reusa_el_listado() {
        use crate::oracle::mutacion::comando_de_listado;
        let cmd = comando_de_listado();
        let memo = Arc::new(MemoSondas::nuevo());
        memo.anotar(&cmd, &con_outcomes("{}"));
        let real = falso(con_outcomes("{\"real\":true}"), false);
        let reuso = reuso_de_mutacion(&memo);
        let reusador = EjecutorReusador {
            real: &real,
            reuso: &reuso,
        };
        reusador.ejecutar(&cmd).expect("delega");
        assert_eq!(real.cmds.borrow().as_slice(), [cmd]);
    }
}
