//! Corrida completa de `r00t corre` (Tarea 16, parte 2): GOAL, ruteo previo de todos los roles, armado,
//! lanzamiento por rol, sesiones, lecturas del oráculo, evaluación y la fila de `runs.jsonl`.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::corre::args::{args_claude, env_del_builder};
use crate::corre::config::ConfigCorrida;
use crate::corre::{
    EjecutorBuilder, ErrorCorre, Lanzamiento, guardar_transcript, preparar_corrida,
};
use crate::goal::verificar_goal;
use crate::observe::claude::{gates_de_costo_desconocido, leer_sesion_claude};
use crate::observe::transcript::{HuellasOraculo, registrar_lecturas};
use crate::observe::{self, ErrorObserve};
use crate::oracle::tarea::{Tarea, cargar_en};
use crate::oracle::{ErrorOraculo, OpcionesOraculo, comando_git};
use crate::route::comando::{etiqueta_via, rutear_en};
use crate::route::politica::nombre_rol;
use crate::tipos::{Eleccion, Evento, Fila, Rol, TipoEvento, Veredicto, Via};
use crate::track;

/// Tokens que `r00t corre` pasa a `route` por cada rol: 0, como `r00t route` sin `--tokens`.
const TOKENS_RUTEO: u32 = 0;
/// Motivo del evento `PlanAprobado` que escribe `r00t corre`. La aprobación es automática.
const MOTIVO_PLAN: &str = "aprobación automática de r00t corre";
/// Fases de los `Gate` de una corrida.
const FASE_ROL: &str = "rol";
const FASE_LANZAMIENTO: &str = "lanzamiento";
const FASE_SESION: &str = "sesion";
const FASE_EVALUACION: &str = "evaluacion";
const FASE_TRANSCRIPT: &str = "transcript";
/// Mensaje de commit del estado final del repo de la corrida.
const MENSAJE_ESTADO_FINAL: &str = "r00t corre: estado final";

/// Lo que `correr` necesita del mundo. Producción arma las de la CLI; los tests dan las suyas.
pub struct DepsCorre<'a> {
    /// `ROOT_HOME`: `root.toml`, `configs/`, `oraculos/`, `eventos.jsonl`, `runs.jsonl` y transcripts.
    pub home: &'a Path,
    /// `$HOME`: la carpeta de corridas va bajo `<home_usuario>/.cache/root/corridas/`.
    pub home_usuario: &'a Path,
    /// Repo de Root: su `GOAL.md` y su `GOAL.hash` autorizan la corrida.
    pub repo_root: &'a Path,
    /// Lanza el builder de cada rol. Es el único punto por donde sale `claude`.
    pub ejecutor: &'a dyn EjecutorBuilder,
    /// Evalúa el repo que dejó el builder con la tarea.
    pub evaluar: &'a dyn Fn(&Path, &Tarea, &OpcionesOraculo) -> Result<Veredicto, ErrorOraculo>,
    /// Entorno del proceso. Solo `PATH` y `LANG` llegan al builder, y eso lo decide `env_del_builder`.
    pub entorno: &'a [(String, String)],
}

/// Un rol que ya corrió: su número de paso, su transcript, su código de salida y su stdout.
struct Lanzado {
    n: u32,
    rol: Rol,
    codigo: i32,
    ruta: PathBuf,
    stdout: Vec<u8>,
}

/// Corre la tarea con la configuración y devuelve la fila que se agregó a `runs.jsonl`.
///
/// Un código distinto de 0 de un rol corta la corrida: los roles siguientes no corren, pero la
/// evaluación sí, y el informe dice lo que falta. Un fallo al lanzar aborta sin evaluar: sin builder
/// no hay nada que evaluar, y la evaluación puede gastar API.
pub fn correr(deps: &DepsCorre, tarea: &str, config: &str) -> Result<Fila, ErrorCorre> {
    // a) GOAL: sin autorización no arranca y no se crea nada.
    verificar_goal(deps.repo_root)?;

    // b) Tarea, config y huellas del oráculo. Las huellas van antes de lanzar: sin ellas no hay lecturas que revisar.
    let tarea_cargada = cargar_en(deps.home, tarea).map_err(|e| ErrorCorre::Uso(e.to_string()))?;
    let config_cargada = ConfigCorrida::cargar(deps.home, config)?;
    let huellas = HuellasOraculo::armar(deps.home, Some(deps.home_usuario), tarea)
        .map_err(|e| de_observe("armar las huellas del oráculo", e))?;
    let ruta_eventos = observe::ruta_eventos_en(deps.home);

    // c) Ruteo previo de todos los roles: si uno no va por cupo, no se lanza nada.
    let mut elecciones: Vec<(Rol, Eleccion)> = Vec::new();
    for rol in &config_cargada.roles {
        let eleccion = rutear_en(deps.home, *rol, tarea, false, TOKENS_RUTEO)
            .map_err(|e| ErrorCorre::Ruteo(Box::new(e)))?;
        exigir_cupo(*rol, &eleccion)?;
        elecciones.push((*rol, eleccion));
    }

    // d) Armado de la carpeta de la corrida.
    let corrida = preparar_corrida(deps.home, deps.home_usuario, tarea, config)?;
    let id = corrida.id.clone();

    // e) Lanzamiento por rol, en orden. Cada salida se guarda como transcript.
    let mut lanzados: Vec<Lanzado> = Vec::new();
    for (indice, (rol, eleccion)) in elecciones.iter().enumerate() {
        let prompt = config_cargada.prompt.get(rol).cloned().unwrap_or_default();
        let modelo = eleccion.modelo.clone().unwrap_or_default();
        let lanzamiento = Lanzamiento {
            rol: *rol,
            args: args_claude(&prompt, &modelo, deps.home),
            env: env_del_builder(&config_cargada.claude, deps.entorno),
            repo: corrida.repo.clone(),
        };
        let salida = match deps.ejecutor.lanzar(&lanzamiento) {
            Ok(salida) => salida,
            Err(error) => {
                let motivo = format!("rol {} no arrancó: {error}", nombre_rol(*rol));
                gate(&ruta_eventos, &id, FASE_LANZAMIENTO, motivo, Some(*rol))?;
                return Err(error);
            }
        };
        let n = u32::try_from(indice + 1).unwrap_or(u32::MAX);
        let ruta = guardar_transcript(deps.home, &id, n, *rol, &salida.stdout)?;
        let codigo = salida.codigo;
        lanzados.push(Lanzado {
            n,
            rol: *rol,
            codigo,
            ruta,
            stdout: salida.stdout,
        });
        if codigo != 0 {
            let motivo = format!("rol {} salió con {codigo}", nombre_rol(*rol));
            gate(&ruta_eventos, &id, FASE_ROL, motivo, Some(*rol))?;
            break;
        }
    }

    // f) La sesión de cada rol sale de su línea `result`.
    for lanzado in &lanzados {
        match linea_result(&lanzado.stdout) {
            Some(linea) => registrar_sesion(deps.home, &id, lanzado, &linea, &ruta_eventos)?,
            None => {
                let motivo = "sesión sin resultado: el builder no dejó la línea result".to_string();
                gate(&ruta_eventos, &id, FASE_SESION, motivo, Some(lanzado.rol))?;
            }
        }
    }

    // g) Después de planifica, la aprobación automática del plan. Solo si planifica salió bien.
    let plan_aprobado = lanzados
        .iter()
        .any(|l| l.rol == Rol::Planifica && l.codigo == 0);
    if plan_aprobado {
        let ev = observe::evento_manual(&id, TipoEvento::PlanAprobado, Some(MOTIVO_PLAN));
        registrar(&ruta_eventos, &ev)?;
    }

    // h) Lecturas del oráculo en cada transcript. Las líneas que no son JSON también se anotan.
    for lanzado in &lanzados {
        let revision = registrar_lecturas(&lanzado.ruta, &id, &huellas, &ruta_eventos)
            .map_err(|e| de_observe("revisar el transcript", e))?;
        if !revision.avisos.is_empty() {
            let motivo = format!(
                "transcript con líneas que no son JSON: {}",
                revision.avisos.join("; ")
            );
            gate(
                &ruta_eventos,
                &id,
                FASE_TRANSCRIPT,
                motivo,
                Some(lanzado.rol),
            )?;
        }
    }

    // i) Estado final del repo de la corrida, si quedó sucio.
    commit_final(&corrida.repo)?;

    // j) Evaluación. Un error no pierde la corrida: la fila lleva el veredicto por defecto.
    let opciones = OpcionesOraculo {
        home: Some(deps.home.to_path_buf()),
        ..OpcionesOraculo::default()
    };
    let veredicto = match (deps.evaluar)(&corrida.repo, &tarea_cargada, &opciones) {
        Ok(veredicto) => veredicto,
        Err(error) => {
            let motivo = format!("la evaluación falló: {error}");
            gate(&ruta_eventos, &id, FASE_EVALUACION, motivo, None)?;
            Veredicto {
                tarea: tarea_cargada.nombre.clone(),
                ..Veredicto::default()
            }
        }
    };

    // k) La fila. El resumen sale de los eventos de esta corrida.
    let resumen =
        observe::resumir_en(&ruta_eventos, &id).map_err(|e| de_observe("resumir la corrida", e))?;
    let modelos = elecciones
        .iter()
        .filter_map(|(rol, eleccion)| eleccion.modelo.clone().map(|m| (*rol, m)))
        .collect();
    let fila = Fila {
        id,
        ts: observe::ahora().0,
        tarea: tarea.to_string(),
        config: config.to_string(),
        commit_root: commit_completo(deps.repo_root),
        veredicto,
        resumen,
        modelos,
        forma: config_cargada.forma(config),
        ..Fila::default()
    };
    track::agregar_en(&deps.home.join("runs.jsonl"), &fila).map_err(|fuente| ErrorCorre::Io {
        contexto: "escribir runs.jsonl".into(),
        fuente,
    })?;
    Ok(fila)
}

/// Un rol va por cupo con modelo, o `r00t corre` no lo lanza.
fn exigir_cupo(rol: Rol, eleccion: &Eleccion) -> Result<(), ErrorCorre> {
    let nombre = nombre_rol(rol);
    match (eleccion.via, &eleccion.modelo) {
        (Via::Cupo, Some(_)) => Ok(()),
        (Via::Cupo, None) => Err(ErrorCorre::Uso(format!(
            "el rol {nombre} va por cupo sin modelo: r00t corre solo lanza claude -p por cupo"
        ))),
        (via, _) => Err(ErrorCorre::Uso(format!(
            "el rol {nombre} va por {}: r00t corre solo lanza claude -p por cupo",
            etiqueta_via(via)
        ))),
    }
}

/// Línea `{"type":"result",...}` de un stream-json, si hay una.
fn linea_result(stdout: &[u8]) -> Option<String> {
    let texto = String::from_utf8_lossy(stdout);
    texto
        .lines()
        .find(|linea| {
            serde_json::from_str::<Value>(linea)
                .ok()
                .and_then(|valor| {
                    valor
                        .get("type")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .as_deref()
                == Some("result")
        })
        .map(str::to_string)
}

/// Registra la sesión de un rol. Su línea `result` se guarda como `<n>-<rol>.resultado.json` en la carpeta
/// de transcripts, y `leer_sesion_claude` la lee como un JSON de `--output-format json`. Una sesión que no
/// se puede leer deja un `Gate` y no corta la corrida.
fn registrar_sesion(
    home: &Path,
    id: &str,
    lanzado: &Lanzado,
    linea: &str,
    ruta_eventos: &Path,
) -> Result<(), ErrorCorre> {
    let ruta = home.join("transcripts").join(id).join(format!(
        "{}-{}.resultado.json",
        lanzado.n,
        nombre_rol(lanzado.rol)
    ));
    fs::write(&ruta, format!("{linea}\n"))
        .map_err(super::infra("guardar el resultado de la sesión"))?;
    let eventos = match leer_sesion_claude(&ruta, id) {
        Ok(eventos) => eventos,
        Err(error) => {
            let motivo = format!("sesión no leída: {error}");
            return gate(ruta_eventos, id, FASE_SESION, motivo, Some(lanzado.rol));
        }
    };
    for ev in &eventos {
        registrar(ruta_eventos, ev)?;
    }
    for gate_ev in gates_de_costo_desconocido(&eventos) {
        registrar(ruta_eventos, &gate_ev)?;
    }
    Ok(())
}

/// Commit del estado final del repo de la corrida, si quedó algo sin commitear. Sin `GIT_*` heredadas.
fn commit_final(repo: &Path) -> Result<(), ErrorCorre> {
    let salida = comando_git(repo, &["status", "--porcelain"])
        .output()
        .map_err(|e| ErrorCorre::Git(format!("no se pudo correr git status: {e}")))?;
    if String::from_utf8_lossy(&salida.stdout).trim().is_empty() {
        return Ok(());
    }
    super::git(repo, &["add", "-A"])?;
    super::git(
        repo,
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "commit",
            "-q",
            "--no-verify",
            "-m",
            MENSAJE_ESTADO_FINAL,
        ],
    )
}

/// Commit completo de `HEAD` de `repo`. Vacío si git no lo da.
fn commit_completo(repo: &Path) -> String {
    comando_git(repo, &["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|salida| salida.status.success())
        .map(|salida| String::from_utf8_lossy(&salida.stdout).trim().to_string())
        .unwrap_or_default()
}

/// Escribe un `Gate` de la corrida con su motivo y, si lo hay, el rol.
fn gate(
    ruta_eventos: &Path,
    corrida: &str,
    fase: &str,
    motivo: String,
    rol: Option<Rol>,
) -> Result<(), ErrorCorre> {
    let mut extra = Map::from_iter([("motivo".to_string(), json!(motivo))]);
    if let Some(rol) = rol {
        extra.insert("rol".into(), json!(nombre_rol(rol)));
    }
    registrar(ruta_eventos, &observe::evento_gate(corrida, fase, extra))
}

/// Agrega un evento a `eventos.jsonl` de `home`.
fn registrar(ruta_eventos: &Path, ev: &Evento) -> Result<(), ErrorCorre> {
    observe::registrar_en(ruta_eventos, ev).map_err(|fuente| ErrorCorre::Io {
        contexto: format!("registrar en {}", ruta_eventos.display()),
        fuente,
    })
}

/// Convierte un error de Observe en el de la corrida. Un error de E/S conserva su causa.
fn de_observe(contexto: &str, error: ErrorObserve) -> ErrorCorre {
    match error {
        ErrorObserve::Io(fuente) => ErrorCorre::Io {
            contexto: contexto.into(),
            fuente,
        },
        otro => ErrorCorre::Observa(format!("{contexto}: {otro}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::fs;
    use std::path::PathBuf;

    use serde_json::Value;
    use tempfile::TempDir;

    use crate::corre::ejecutor::pruebas::EjecutorFalso;
    use crate::corre::{Lanzamiento, SalidaClaude, dir_corridas};
    use crate::goal::{ErrorGoal, calcular_hash};
    use crate::oracle::ErrorOraculo;
    use crate::tipos::{Confianza, Decision, Estado, Evento, Forma, Informe, Rol, TipoEvento};
    use crate::track::{self, Filtro};

    const GOAL: &str = "# GOAL de prueba\n\nContenido de fixture.\n";
    const TAREA: &str = "memo";
    const CONFIG: &str = "ecc-test";
    const CONFIG_PLAN: &str = "solo-plan";
    const CONFIG_CONSTRUYE: &str = "solo-construye";
    const CONFIG_API: &str = "por-api";
    const PLAN_APROBADO: &str = "aprobación automática de r00t corre";

    /// Un ROOT_HOME de prueba con la tarea y sus configs, un `$HOME` y un repo de Root con GOAL válido.
    struct Entorno {
        _tmp: TempDir,
        home: PathBuf,
        usuario: PathBuf,
        repo: PathBuf,
    }

    fn entorno() -> Entorno {
        let tmp = TempDir::new().expect("tmp");
        let base = tmp.path().to_path_buf();
        let home = base.join("root");
        let usuario = base.join("usuario");
        let repo = base.join("repo");
        fs::create_dir_all(&home).expect("home");
        fs::create_dir_all(&usuario).expect("usuario");
        fs::create_dir_all(&repo).expect("repo");

        fs::write(
            home.join("root.toml"),
            "[modelos]\nplanifica = \"opus\"\nconstruye = \"haiku\"\n",
        )
        .expect("root.toml");
        let configs = home.join("configs");
        fs::create_dir_all(&configs).expect("configs");
        let prompt = "[prompt]\nplanifica = \"haz el plan\"\nconstruye = \"haz hecho\"\n";
        fs::write(
            configs.join(format!("{CONFIG}.toml")),
            format!("roles = [\"planifica\", \"construye\"]\n{prompt}"),
        )
        .expect("config");
        fs::write(
            configs.join(format!("{CONFIG_PLAN}.toml")),
            format!("roles = [\"planifica\"]\n{prompt}"),
        )
        .expect("config plan");
        fs::write(
            configs.join(format!("{CONFIG_CONSTRUYE}.toml")),
            format!("roles = [\"construye\"]\n{prompt}"),
        )
        .expect("config construye");
        fs::write(
            configs.join(format!("{CONFIG_API}.toml")),
            format!("roles = [\"planifica\", \"delega\"]\n{prompt}delega = \"delega\"\n"),
        )
        .expect("config api");

        let oraculo = home.join("oraculos").join(TAREA);
        fs::create_dir_all(oraculo.join("bootstrap").join("docs")).expect("bootstrap");
        fs::write(
            oraculo.join("bootstrap").join("docs").join("SPEC.md"),
            "# Spec\n",
        )
        .expect("spec");
        fs::create_dir_all(oraculo.join("contrato")).expect("contrato");
        fs::write(
            oraculo.join("contrato").join("test_x.py"),
            "def test_x(): pass\n",
        )
        .expect("contrato");
        fs::create_dir_all(oraculo.join("oculto")).expect("oculto");
        fs::write(
            oraculo.join("oculto").join("test_y.py"),
            "def test_y():\n    assert sincronizar(tmp, \"clave-secreta-42\") == [1, 2, 3]\n",
        )
        .expect("oculto");
        fs::write(
            oraculo.join("tarea.toml"),
            "nombre = \"memo\"\nlenguaje = \"rust\"\nimagen = \"root-oraculo-rust:1\"\nspec = \"bootstrap/docs/SPEC.md\"\n\n[contrato]\ncmd = \"pytest -q /oraculo/contrato\"\n\n[oculto]\ncmd = \"pytest -q /oraculo/oculto\"\n",
        )
        .expect("tarea.toml");

        fs::write(repo.join("GOAL.md"), GOAL).expect("GOAL.md");
        fs::write(
            repo.join("GOAL.hash"),
            format!("{}\n", calcular_hash(GOAL.as_bytes())),
        )
        .expect("GOAL.hash");

        Entorno {
            _tmp: tmp,
            home,
            usuario,
            repo,
        }
    }

    /// Builder de prueba: el falso de `tests/fixtures`, con las fallas que cada test pide.
    #[derive(Default)]
    struct Builder {
        /// Rol cuyo builder sale con código 1.
        falla: Option<Rol>,
        /// Variables que se suman al entorno del lanzamiento. Solo las pone el test.
        extra: Vec<(String, String)>,
        /// Quita la línea `result` de la salida.
        sin_resultado: bool,
        /// Cambia el `result` por uno sin costo.
        costo_desconocido: bool,
        /// El lanzamiento no arranca.
        no_lanza: bool,
        lanzados: RefCell<Vec<Lanzamiento>>,
    }

    impl EjecutorBuilder for Builder {
        fn lanzar(&self, l: &Lanzamiento) -> Result<SalidaClaude, ErrorCorre> {
            self.lanzados.borrow_mut().push(l.clone());
            if self.no_lanza {
                return Err(ErrorCorre::Ejecutor("prueba: no arranca".into()));
            }
            let mut con_extra = l.clone();
            con_extra.env.extend(self.extra.iter().cloned());
            let mut salida = EjecutorFalso.lanzar(&con_extra)?;
            if self.falla == Some(l.rol) {
                salida.codigo = 1;
            }
            if self.sin_resultado {
                let texto = String::from_utf8_lossy(&salida.stdout).into_owned();
                let sin: Vec<&str> = texto
                    .lines()
                    .filter(|linea| !linea.contains("\"type\":\"result\""))
                    .collect();
                salida.stdout = format!("{}\n", sin.join("\n")).into_bytes();
            }
            if self.costo_desconocido {
                let sin_costo = "{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"duration_ms\":1000,\"num_turns\":2,\"modelUsage\":{\"opus\":{\"inputTokens\":10}},\"session_id\":\"s1\"}\n";
                salida.stdout = sin_costo.as_bytes().to_vec();
            }
            Ok(salida)
        }
    }

    fn informe() -> Informe {
        Informe {
            decision: Decision::Listo,
            motivo: "prueba".into(),
            requisitos: Vec::new(),
            bloqueantes: Vec::new(),
            riesgos: Vec::new(),
            proximos_pasos: Vec::new(),
            resumen: "Prueba.".into(),
            confianza: Confianza::default(),
        }
    }

    /// Evaluador falso: devuelve un veredicto con informe armado a mano.
    fn evaluador_falso(
        _repo: &Path,
        tarea: &Tarea,
        _o: &OpcionesOraculo,
    ) -> Result<Veredicto, ErrorOraculo> {
        Ok(Veredicto {
            tarea: tarea.nombre.clone(),
            estado: Estado::Verde,
            informe: Some(informe()),
            ..Veredicto::default()
        })
    }

    fn evaluador_roto(
        _repo: &Path,
        _tarea: &Tarea,
        _o: &OpcionesOraculo,
    ) -> Result<Veredicto, ErrorOraculo> {
        Err(ErrorOraculo::Infra("sin podman en la prueba".into()))
    }

    fn correr_con(
        e: &Entorno,
        ejecutor: &dyn EjecutorBuilder,
        entorno: &[(String, String)],
        config: &str,
    ) -> Result<Fila, ErrorCorre> {
        correr_con_eval(e, ejecutor, entorno, config, &evaluador_falso)
    }

    fn correr_con_eval(
        e: &Entorno,
        ejecutor: &dyn EjecutorBuilder,
        entorno: &[(String, String)],
        config: &str,
        evaluar: &dyn Fn(&Path, &Tarea, &OpcionesOraculo) -> Result<Veredicto, ErrorOraculo>,
    ) -> Result<Fila, ErrorCorre> {
        let deps = DepsCorre {
            home: &e.home,
            home_usuario: &e.usuario,
            repo_root: &e.repo,
            ejecutor,
            evaluar,
            entorno,
        };
        correr(&deps, TAREA, config)
    }

    /// Eventos de `eventos.jsonl` de `home`, en orden de escritura.
    fn eventos(e: &Entorno) -> Vec<Evento> {
        let texto = fs::read_to_string(e.home.join("eventos.jsonl")).unwrap_or_default();
        texto
            .lines()
            .map(|linea| serde_json::from_str(linea).expect("evento válido"))
            .collect()
    }

    fn gates(e: &Entorno, fase: &str) -> Vec<String> {
        eventos(e)
            .iter()
            .filter(|ev| ev.tipo == TipoEvento::Gate && ev.fase == fase)
            .filter_map(|ev| ev.extra.get("motivo").and_then(Value::as_str))
            .map(str::to_string)
            .collect()
    }

    fn hay_lecturas(e: &Entorno) -> bool {
        eventos(e)
            .iter()
            .any(|ev| ev.tipo == TipoEvento::OraculoLeido)
    }

    fn entorno_con_secretos(e: &Entorno) -> Vec<(String, String)> {
        vec![
            ("PATH".into(), "/usr/bin:/bin".into()),
            ("LANG".into(), "C".into()),
            ("DEEPSEEK_API_KEY".into(), "clave-1".into()),
            ("ANTHROPIC_API_KEY".into(), "clave-2".into()),
            ("ROOT_HOME".into(), e.home.display().to_string()),
            ("HOME".into(), e.usuario.display().to_string()),
        ]
    }

    #[test]
    fn corre_mecanica_deja_fila_con_informe_resumen_y_forma() {
        let e = entorno();
        let fila = correr_con(&e, &Builder::default(), &[], CONFIG).expect("corre");
        let filas = track::filas_de(&e.home.join("runs.jsonl"), &Filtro::default()).expect("runs");
        assert_eq!(filas.len(), 1, "una fila por corrida");
        let guardada = &filas[0];
        assert_eq!(guardada.id, fila.id);
        assert_eq!(guardada.tarea, TAREA);
        assert_eq!(guardada.config, CONFIG);
        assert!(guardada.veredicto.informe.is_some(), "falta el informe");
        // Cada rol da dos turnos del `result` del falso, y un costo de cupo de 0,01 USD.
        assert_eq!(guardada.resumen.turnos, 4);
        assert!(
            (guardada.resumen.usd_cupo - 0.02).abs() < 1e-9,
            "{:?}",
            guardada.resumen
        );
        assert!(!guardada.resumen.invalida);
        assert_eq!(
            guardada.forma,
            Forma {
                roles: vec![Rol::Planifica, Rol::Construye],
                verificador: false,
                config: CONFIG.into(),
            }
        );
        assert_eq!(guardada.modelos[&Rol::Planifica], "opus");
        assert_eq!(guardada.modelos[&Rol::Construye], "haiku");
    }

    #[test]
    fn corre_goal_alterado_no_arranca_ni_crea_nada() {
        let e = entorno();
        fs::write(e.repo.join("GOAL.hash"), "0000000000000000\n").expect("hash malo");
        let builder = Builder::default();
        let error = correr_con(&e, &builder, &[], CONFIG).expect_err("GOAL alterado");
        assert!(
            matches!(error, ErrorCorre::Goal(ErrorGoal::Alterado { .. })),
            "{error}"
        );
        assert_eq!(error.codigo_de_salida(), 1);
        assert!(builder.lanzados.borrow().is_empty(), "no debe lanzar");
        assert!(!dir_corridas(&e.usuario).exists(), "no debe crear corridas");
        assert!(
            !e.home.join("runs.jsonl").exists(),
            "no debe escribir runs.jsonl"
        );
    }

    #[test]
    fn corre_goal_sin_archivo_sale_con_3() {
        let e = entorno();
        fs::remove_file(e.repo.join("GOAL.md")).expect("quita GOAL.md");
        let error = correr_con(&e, &Builder::default(), &[], CONFIG).expect_err("sin GOAL");
        assert_eq!(error.codigo_de_salida(), 3, "{error}");
        assert!(!dir_corridas(&e.usuario).exists());
    }

    #[test]
    fn corre_lectura_del_oraculo_invalida_la_corrida() {
        let e = entorno();
        let oculto = e
            .home
            .join("oraculos")
            .join(TAREA)
            .join("oculto")
            .join("test_y.py");
        let builder = Builder {
            extra: vec![(
                "CLAUDE_FALSO_LEE_ORACULO".into(),
                oculto.display().to_string(),
            )],
            ..Builder::default()
        };
        let fila = correr_con(&e, &builder, &[], CONFIG).expect("corre");
        assert!(fila.resumen.invalida, "la lectura debe invalidar");
        let leidos: Vec<Evento> = eventos(&e)
            .into_iter()
            .filter(|ev| ev.tipo == TipoEvento::OraculoLeido)
            .collect();
        assert!(!leidos.is_empty(), "falta OraculoLeido");
        assert!(leidos.iter().all(|ev| ev.corrida == fila.id));
        let guardada =
            track::filas_de(&e.home.join("runs.jsonl"), &Filtro::default()).expect("runs");
        assert!(
            guardada[0].resumen.invalida,
            "la fila guardada debe quedar inválida"
        );
    }

    #[test]
    fn corre_transcript_limpio_no_emite_lecturas() {
        let e = entorno();
        let fila = correr_con(&e, &Builder::default(), &[], CONFIG).expect("corre");
        assert!(!fila.resumen.invalida);
        assert!(!hay_lecturas(&e), "un transcript limpio no emite nada");
    }

    #[test]
    fn corre_rol_con_codigo_distinto_de_cero_corta_los_siguientes() {
        let e = entorno();
        let builder = Builder {
            falla: Some(Rol::Planifica),
            ..Builder::default()
        };
        let fila = correr_con(&e, &builder, &[], CONFIG).expect("la corrida deja fila");
        let roles: Vec<Rol> = builder.lanzados.borrow().iter().map(|l| l.rol).collect();
        assert_eq!(roles, vec![Rol::Planifica], "construye no debe correr");
        assert_eq!(
            gates(&e, "rol"),
            vec!["rol planifica salió con 1".to_string()]
        );
        assert!(
            !eventos(&e)
                .iter()
                .any(|ev| ev.tipo == TipoEvento::PlanAprobado),
            "sin plan no hay aprobación"
        );
        let guardada =
            track::filas_de(&e.home.join("runs.jsonl"), &Filtro::default()).expect("runs");
        assert_eq!(guardada.len(), 1);
        assert_eq!(guardada[0].id, fila.id);
    }

    #[test]
    fn corre_plan_aprobado_queda_despues_de_planifica() {
        let e = entorno();
        let fila = correr_con(&e, &Builder::default(), &[], CONFIG_PLAN).expect("corre");
        let ev = eventos(&e);
        let aprobados: Vec<usize> = ev
            .iter()
            .enumerate()
            .filter(|(_, x)| x.tipo == TipoEvento::PlanAprobado && x.corrida == fila.id)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(
            aprobados.len(),
            1,
            "una aprobación por corrida con planifica"
        );
        let aprobado = &ev[aprobados[0]];
        assert_eq!(aprobado.extra["detalle"], PLAN_APROBADO);
        let sesion = ev
            .iter()
            .position(|x| x.tipo == TipoEvento::Sesion && x.corrida == fila.id)
            .expect("la sesión de planifica");
        assert!(
            aprobados[0] > sesion,
            "la aprobación va después de la sesión de planifica"
        );
    }

    #[test]
    fn corre_sin_planifica_no_hay_plan_aprobado() {
        let e = entorno();
        let fila = correr_con(&e, &Builder::default(), &[], CONFIG_CONSTRUYE).expect("corre");
        assert!(
            !eventos(&e)
                .iter()
                .any(|x| x.tipo == TipoEvento::PlanAprobado && x.corrida == fila.id)
        );
    }

    #[test]
    fn corre_builder_no_recibe_root_home_ni_claves() {
        let e = entorno();
        let builder = Builder::default();
        let entorno_proceso = entorno_con_secretos(&e);
        let fila = correr_con(&e, &builder, &entorno_proceso, CONFIG).expect("corre");
        let lanzados = builder.lanzados.borrow();
        assert_eq!(lanzados.len(), 2, "cada rol corre");
        for l in lanzados.iter() {
            for prohibida in ["DEEPSEEK_API_KEY", "ANTHROPIC_API_KEY", "ROOT_HOME", "HOME"] {
                assert!(
                    l.env.iter().all(|(k, _)| k != prohibida),
                    "{prohibida} llega al builder: {:?}",
                    l.env
                );
            }
        }
        assert!(
            gates(&e, "rol").is_empty(),
            "ningún rol debe salir con código distinto de 0"
        );
        assert_eq!(fila.forma.roles.len(), 2);
    }

    #[test]
    fn corre_carpeta_de_corrida_bajo_usuario_y_transcript_bajo_home() {
        let e = entorno();
        let fila = correr_con(&e, &Builder::default(), &[], CONFIG).expect("corre");
        let dir = dir_corridas(&e.usuario).join(&fila.id);
        assert!(dir.join("repo").join("hecho.txt").is_file());
        assert!(
            !dir.starts_with(&e.home),
            "la corrida no cae dentro de ROOT_HOME"
        );
        let transcripts = e.home.join("transcripts").join(&fila.id);
        assert!(transcripts.join("1-planifica.jsonl").is_file());
        assert!(transcripts.join("2-construye.jsonl").is_file());
    }

    #[test]
    fn corre_rol_que_va_por_api_es_error_de_uso_sin_lanzar() {
        let e = entorno();
        let builder = Builder::default();
        let error = correr_con(&e, &builder, &[], CONFIG_API).expect_err("delega va por API");
        assert!(
            matches!(
                error,
                ErrorCorre::Uso(ref m)
                    if m.contains("el rol delega va por api: r00t corre solo lanza claude -p por cupo")
            ),
            "{error}"
        );
        assert_eq!(error.codigo_de_salida(), 2);
        assert!(builder.lanzados.borrow().is_empty(), "no debe lanzar nada");
        assert!(
            !dir_corridas(&e.usuario).exists(),
            "no debe crear la carpeta"
        );
    }

    #[test]
    fn corre_error_de_evaluacion_no_pierde_la_fila() {
        let e = entorno();
        let fila = correr_con_eval(&e, &Builder::default(), &[], CONFIG, &evaluador_roto)
            .expect("la fila se escribe igual");
        assert_eq!(fila.veredicto.estado, Estado::SinVeredicto);
        assert!(fila.veredicto.informe.is_none());
        let guardadas =
            track::filas_de(&e.home.join("runs.jsonl"), &Filtro::default()).expect("runs");
        assert_eq!(guardadas.len(), 1);
        assert!(
            gates(&e, "evaluacion")
                .iter()
                .any(|m| m.contains("sin podman en la prueba")),
            "falta el Gate de evaluación"
        );
    }

    #[test]
    fn corre_sesion_sin_resultado_deja_gate() {
        let e = entorno();
        let builder = Builder {
            sin_resultado: true,
            ..Builder::default()
        };
        let fila = correr_con(&e, &builder, &[], CONFIG).expect("corre");
        assert_eq!(fila.resumen.turnos, 0, "sin result no hay turnos");
        assert_eq!(gates(&e, "sesion").len(), 2, "un Gate por rol");
        assert!(
            gates(&e, "sesion")
                .iter()
                .all(|m| m.contains("sesión sin resultado"))
        );
    }

    #[test]
    fn corre_costo_desconocido_deja_gate_de_la_sesion() {
        let e = entorno();
        let builder = Builder {
            costo_desconocido: true,
            ..Builder::default()
        };
        let fila = correr_con(&e, &builder, &[], CONFIG).expect("corre");
        let gateados: Vec<Evento> = eventos(&e)
            .into_iter()
            .filter(|ev| ev.tipo == TipoEvento::Gate && ev.fase == "costo-desconocido")
            .collect();
        assert_eq!(gateados.len(), 2, "un Gate por sesión sin costo");
        assert!(gateados.iter().all(|ev| ev.corrida == fila.id));
    }

    #[test]
    fn corre_fallo_del_ejecutor_corta_sin_evaluar_ni_escribir_fila() {
        let e = entorno();
        let builder = Builder {
            no_lanza: true,
            ..Builder::default()
        };
        let error = correr_con(&e, &builder, &[], CONFIG).expect_err("no arranca");
        assert!(matches!(error, ErrorCorre::Ejecutor(_)), "{error}");
        assert_eq!(
            gates(&e, "lanzamiento").len(),
            1,
            "queda un Gate con la causa"
        );
        assert!(!e.home.join("runs.jsonl").exists(), "sin fila");
    }

    #[test]
    fn corre_tarea_inexistente_es_uso_y_no_crea_carpeta() {
        let e = entorno();
        let error = correr(
            &DepsCorre {
                home: &e.home,
                home_usuario: &e.usuario,
                repo_root: &e.repo,
                ejecutor: &Builder::default(),
                evaluar: &evaluador_falso,
                entorno: &[],
            },
            "no-existe",
            CONFIG,
        )
        .expect_err("tarea inexistente");
        assert_eq!(error.codigo_de_salida(), 2, "{error}");
        assert!(!dir_corridas(&e.usuario).exists());
    }
}
