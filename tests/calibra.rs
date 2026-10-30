//! Tests de `r00t oraculo calibrar` (Tarea 12): la salida del binario y la escritura de `calibracion.json`.
//! El Oracle real corre con podman; estos tests no lo llaman: solo prueban los errores de uso y la escritura.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use std::cell::RefCell;

use root::config::Config;
use root::observe;
use root::oracle::calibra::{
    ARCHIVO, Calibraciones, JuecesCaso, Presupuesto, calibrar_tarea, calibrar_tarea_con,
    peor_caso_con_jueces,
};
use root::oracle::tarea::Tarea;
use root::oracle::{ErrorOraculo, OpcionesOraculo, Partes};
use root::tipos::{Dimension, Eleccion, Evento, Proveedor, Rol, TipoEvento, Via};
use root::tipos::{Puntaje, Veredicto};
use serde_json::json;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/calibra/repo-rust"
);

const TAREA_BASE: &str = r#"
nombre = "calibra-fixture"
lenguaje = "rust"
imagen = "root-oraculo-rust:1"
spec = "bootstrap/docs/SPEC.md"

[contrato]
cmd = "cargo test"

[oculto]
cmd = "cargo test"
"#;

fn r00t(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_r00t"))
        .env("ROOT_HOME", home)
        .args(args)
        .output()
        .expect("no se pudo ejecutar el binario r00t")
}

fn codigo(salida: &Output) -> i32 {
    salida.status.code().expect("el proceso terminó con código")
}

/// `ROOT_HOME` con `oraculos/calibra-fixture/tarea.toml`. `extra` se agrega al TOML.
fn home_con_tarea(extra: &str) -> tempfile::TempDir {
    let home = tempfile::tempdir().expect("home");
    let carpeta = home.path().join("oraculos").join("calibra-fixture");
    std::fs::create_dir_all(&carpeta).expect("carpeta");
    std::fs::write(carpeta.join("tarea.toml"), format!("{TAREA_BASE}{extra}")).expect("tarea");
    home
}

/// Veredicto de seguridad sin hallazgos: el evaluador falso de estos tests no detecta nada.
fn veredicto_plano() -> Veredicto {
    Veredicto {
        puntajes: vec![Puntaje {
            dimension: Dimension::Seguridad,
            valor: 1.0,
            calibrada: false,
            exactitud_balanceada: None,
            items: vec![],
            sin_veredicto: None,
            faltantes: vec![],
        }],
        ..Veredicto::default()
    }
}

#[test]
fn calibra_cli_sin_repo_ni_calibracion_sale_con_2() {
    let home = home_con_tarea("");
    let salida = r00t(
        home.path(),
        &[
            "oraculo",
            "calibrar",
            "--tarea",
            "calibra-fixture",
            "--dimension",
            "seguridad",
        ],
    );
    assert_eq!(
        codigo(&salida),
        2,
        "{}",
        String::from_utf8_lossy(&salida.stderr)
    );
    assert!(String::from_utf8_lossy(&salida.stderr).contains("--repo"));
    assert!(
        !home.path().join(ARCHIVO).exists(),
        "sin repo no escribe nada"
    );
}

#[test]
fn calibra_cli_dimension_desconocida_sale_con_2() {
    let home = home_con_tarea("");
    let salida = r00t(
        home.path(),
        &[
            "oraculo",
            "calibrar",
            "--tarea",
            "calibra-fixture",
            "--dimension",
            "magia",
            "--repo",
            FIXTURE,
        ],
    );
    assert_eq!(codigo(&salida), 2);
}

#[test]
fn calibra_cli_repo_inexistente_sale_con_2() {
    let home = home_con_tarea("");
    let salida = r00t(
        home.path(),
        &[
            "oraculo",
            "calibrar",
            "--tarea",
            "calibra-fixture",
            "--dimension",
            "seguridad",
            "--repo",
            "/ruta/que/no/existe/calibra",
        ],
    );
    assert_eq!(
        codigo(&salida),
        2,
        "{}",
        String::from_utf8_lossy(&salida.stderr)
    );
}

#[test]
fn calibra_cli_tarea_inexistente_sale_con_3() {
    let home = tempfile::tempdir().expect("home");
    let salida = r00t(
        home.path(),
        &[
            "oraculo",
            "calibrar",
            "--tarea",
            "no-existe",
            "--dimension",
            "seguridad",
            "--repo",
            FIXTURE,
        ],
    );
    assert_eq!(
        codigo(&salida),
        3,
        "{}",
        String::from_utf8_lossy(&salida.stderr)
    );
}

#[test]
fn calibra_tarea_dos_corridas_agregan_filas_y_no_borran_las_anteriores() {
    let home = home_con_tarea("");
    let evaluador = |_: &Tarea, _: &Path, _: &OpcionesOraculo| Ok(veredicto_plano());
    let mut rutas = Vec::<PathBuf>::new();
    for semilla in [42, 43] {
        calibrar_tarea(
            home.path(),
            "calibra-fixture",
            "seguridad",
            4,
            semilla,
            Some(Path::new(FIXTURE)),
            &evaluador,
        )
        .expect("calibra");
        rutas.push(home.path().join(ARCHIVO));
    }
    let leido = Calibraciones::cargar(home.path()).expect("carga calibracion.json");
    assert_eq!(leido.filas.len(), 2, "una fila por corrida");
    assert_eq!(leido.filas[0].semilla, 42, "la primera fila sigue ahí");
    assert_eq!(leido.filas[1].semilla, 43);
    assert_eq!(leido.filas[0].dimension, Dimension::Seguridad);
}

#[test]
fn calibra_tarea_evalua_la_base_y_cada_caso_con_las_partes_de_su_dimension() {
    let home = home_con_tarea("");
    let vistas = std::cell::RefCell::new(Vec::<Option<Partes>>::new());
    let evaluador = |_: &Tarea, _: &Path, o: &OpcionesOraculo| {
        vistas.borrow_mut().push(o.partes);
        Ok(veredicto_plano())
    };
    calibrar_tarea(
        home.path(),
        "calibra-fixture",
        "seguridad",
        4,
        42,
        Some(Path::new(FIXTURE)),
        &evaluador,
    )
    .expect("calibra");
    let vistas = vistas.borrow();
    // Línea base más cuatro casos: todos con las partes de Seguridad.
    assert_eq!(vistas.len(), 5, "{vistas:?}");
    for partes in vistas.iter() {
        let partes = partes.expect("la calibración pasa partes");
        assert_eq!(partes.dimension, Some(Dimension::Seguridad));
        assert!(partes.mide(Dimension::Seguridad));
        assert!(!partes.mide(Dimension::Diseno));
        assert!(!partes.oculto, "Seguridad no corre la suite oculta");
        assert!(!partes.redactor, "la calibración no llama al redactor");
    }
}

/// Modelo del juez de diseño en estos tests. Su precio está en los defaults.
const MODELO_JUEZ: &str = "claude-sonnet-5-5";

/// Registra en `home` una llamada de API de `costo` USD, como las que deja un juez real.
fn gasta(home: &Path, costo: f64) {
    let mut ev = observe::evento_manual("oraculo-juez-diseno", TipoEvento::Llamada, None);
    ev.via = Some(Via::Api);
    ev.cost_usd = costo;
    observe::registrar_en(&observe::ruta_eventos_en(home), &ev).expect("registra la llamada");
}

/// Gasto de API desde `inicio` en `home`, como lo mide la calibración de producción.
fn medidor(home: &Path) -> impl Fn(f64) -> Result<f64, ErrorOraculo> + '_ {
    move |inicio: f64| {
        observe::uso::gasto_api_desde_en(&observe::ruta_eventos_en(home), inicio)
            .map_err(|e| ErrorOraculo::Infra(e.to_string()))
    }
}

/// Eventos `Gate` de `home` con la fase `calibracion-tope`.
fn gates_de_tope(home: &Path) -> Vec<Evento> {
    observe::uso::leer_eventos(&observe::ruta_eventos_en(home))
        .expect("lee eventos")
        .into_iter()
        .filter(|ev| ev.tipo == TipoEvento::Gate && ev.fase == "calibracion-tope")
        .collect()
}

#[test]
fn calibra_tope_detiene_antes_del_caso_que_no_entra() {
    // Base y casos cuestan 0,30 cada uno; el peor caso es 0,40 y el tope 1,00. Antes de la base: 0,40 entra.
    // Antes del caso 1: 0,30 + 0,40 entra. Antes del caso 2: 0,60 + 0,40 entra. Antes del caso 3: 0,90 + 0,40
    // no entra: se detiene con la base y dos casos medidos.
    let home = home_con_tarea("");
    let peor = |_: &Partes| Ok::<f64, ErrorOraculo>(0.40);
    let gasto = medidor(home.path());
    let presupuesto = Presupuesto {
        usd: 1.0,
        peor_caso: &peor,
        gasto_desde: &gasto,
    };
    let llamadas = RefCell::new(0u32);
    let evaluador = |_: &Tarea, _: &Path, _: &OpcionesOraculo| {
        *llamadas.borrow_mut() += 1;
        gasta(home.path(), 0.30);
        Ok(veredicto_plano())
    };
    let error = calibrar_tarea_con(
        home.path(),
        "calibra-fixture",
        "seguridad",
        4,
        42,
        Some(Path::new(FIXTURE)),
        Some(&presupuesto),
        &evaluador,
    )
    .expect_err("el tope detiene la calibración");
    assert_eq!(error.codigo_de_salida(), 3);
    let texto = error.to_string();
    assert!(texto.contains("1.00"), "nombra el tope: {texto}");
    assert!(texto.contains("0.90"), "nombra el gasto: {texto}");
    assert_eq!(*llamadas.borrow(), 3, "la base y dos casos");

    let leido = Calibraciones::cargar(home.path()).expect("carga calibracion.json");
    assert_eq!(leido.filas.len(), 1, "las filas hechas quedan");
    let fila = &leido.filas[0];
    assert!(fila.incompleta, "la fila dice que está incompleta");
    assert_eq!(fila.n, 2, "n son los casos medidos");

    let gates = gates_de_tope(home.path());
    assert_eq!(gates.len(), 1, "un Gate de tope");
    assert_eq!(gates[0].extra.get("tarea"), Some(&json!("calibra-fixture")));
    assert_eq!(gates[0].extra.get("dimension"), Some(&json!("seguridad")));
    assert_eq!(gates[0].extra.get("tope_usd"), Some(&json!(1.0)));
}

#[test]
fn calibra_tope_suficiente_da_fila_completa_y_sin_gate() {
    let home = home_con_tarea("");
    let peor = |_: &Partes| Ok::<f64, ErrorOraculo>(0.40);
    let gasto = medidor(home.path());
    let presupuesto = Presupuesto {
        usd: 5.0,
        peor_caso: &peor,
        gasto_desde: &gasto,
    };
    let evaluador = |_: &Tarea, _: &Path, _: &OpcionesOraculo| {
        gasta(home.path(), 0.30);
        Ok(veredicto_plano())
    };
    let fila = calibrar_tarea_con(
        home.path(),
        "calibra-fixture",
        "seguridad",
        4,
        42,
        Some(Path::new(FIXTURE)),
        Some(&presupuesto),
        &evaluador,
    )
    .expect("con tope suficiente, la calibración termina");
    assert!(!fila.incompleta);
    assert_eq!(fila.n, 4);
    assert!(gates_de_tope(home.path()).is_empty(), "sin Gate");
    let leido = Calibraciones::cargar(home.path()).expect("carga");
    assert!(!leido.filas[0].incompleta);
}

#[test]
fn calibra_tope_antes_de_la_base_no_evalua_ni_escribe_fila() {
    let home = home_con_tarea("");
    let peor = |_: &Partes| Ok::<f64, ErrorOraculo>(0.40);
    let gasto = medidor(home.path());
    let presupuesto = Presupuesto {
        usd: 0.30,
        peor_caso: &peor,
        gasto_desde: &gasto,
    };
    let llamadas = RefCell::new(0u32);
    let evaluador = |_: &Tarea, _: &Path, _: &OpcionesOraculo| {
        *llamadas.borrow_mut() += 1;
        Ok(veredicto_plano())
    };
    let error = calibrar_tarea_con(
        home.path(),
        "calibra-fixture",
        "seguridad",
        4,
        42,
        Some(Path::new(FIXTURE)),
        Some(&presupuesto),
        &evaluador,
    )
    .expect_err("la base no entra");
    assert_eq!(error.codigo_de_salida(), 3);
    assert_eq!(*llamadas.borrow(), 0, "no evalúa nada");
    assert!(!home.path().join(ARCHIVO).exists(), "sin casos, sin fila");
    assert_eq!(gates_de_tope(home.path()).len(), 1);
}

#[test]
fn calibra_tope_tras_la_base_sin_casos_no_escribe_fila() {
    // La base cuesta 0,30. Antes del primer caso: 0,30 + 0,40 > 0,50. Ningún caso medido: sin fila.
    let home = home_con_tarea("");
    let peor = |_: &Partes| Ok::<f64, ErrorOraculo>(0.40);
    let gasto = medidor(home.path());
    let presupuesto = Presupuesto {
        usd: 0.50,
        peor_caso: &peor,
        gasto_desde: &gasto,
    };
    let llamadas = RefCell::new(0u32);
    let evaluador = |_: &Tarea, _: &Path, _: &OpcionesOraculo| {
        *llamadas.borrow_mut() += 1;
        gasta(home.path(), 0.30);
        Ok(veredicto_plano())
    };
    let error = calibrar_tarea_con(
        home.path(),
        "calibra-fixture",
        "seguridad",
        4,
        42,
        Some(Path::new(FIXTURE)),
        Some(&presupuesto),
        &evaluador,
    )
    .expect_err("el tope detiene tras la base");
    assert_eq!(error.codigo_de_salida(), 3);
    assert_eq!(*llamadas.borrow(), 1, "solo la base");
    assert!(
        !home.path().join(ARCHIVO).exists(),
        "sin casos medidos, sin fila"
    );
    assert_eq!(gates_de_tope(home.path()).len(), 1);
}

#[test]
fn calibra_tope_sin_precio_del_juez_no_arranca_ni_escribe_fila() {
    let home = home_con_tarea("");
    let mut cfg = Config::default();
    cfg.precios.remove(MODELO_JUEZ);
    let juez = Eleccion {
        via: Via::Api,
        proveedor: Some(Proveedor::Anthropic),
        modelo: Some(MODELO_JUEZ.into()),
        razonamiento: true,
        motivo: "prueba".into(),
        rol: Some(Rol::Juzga),
    };
    let peor = |partes: &Partes| {
        let jueces = JuecesCaso {
            diseno: Some(&juez),
            config: None,
        };
        peor_caso_con_jueces(&cfg, &jueces, partes)
    };
    let gasto = medidor(home.path());
    let presupuesto = Presupuesto {
        usd: 25.0,
        peor_caso: &peor,
        gasto_desde: &gasto,
    };
    let llamadas = RefCell::new(0u32);
    let evaluador = |_: &Tarea, _: &Path, _: &OpcionesOraculo| {
        *llamadas.borrow_mut() += 1;
        Ok(veredicto_plano())
    };
    // Diseño pide su juez, que no tiene precio: la calibración con tope no arranca.
    let error = calibrar_tarea_con(
        home.path(),
        "calibra-fixture",
        "diseno",
        4,
        42,
        Some(Path::new(FIXTURE)),
        Some(&presupuesto),
        &evaluador,
    )
    .expect_err("sin precio, la calibración con tope no arranca");
    assert_eq!(error.codigo_de_salida(), 2);
    assert!(error.to_string().contains(MODELO_JUEZ), "{error}");
    assert_eq!(*llamadas.borrow(), 0, "no evalúa nada");
    assert!(!home.path().join(ARCHIVO).exists(), "sin fila");
}

#[test]
fn calibra_tope_incompleta_con_la_misma_semilla_se_repite() {
    let correr = || {
        let home = home_con_tarea("");
        let peor = |_: &Partes| Ok::<f64, ErrorOraculo>(0.40);
        let gasto = medidor(home.path());
        let presupuesto = Presupuesto {
            usd: 1.0,
            peor_caso: &peor,
            gasto_desde: &gasto,
        };
        let evaluador = |_: &Tarea, _: &Path, _: &OpcionesOraculo| {
            gasta(home.path(), 0.30);
            Ok(veredicto_plano())
        };
        let _ = calibrar_tarea_con(
            home.path(),
            "calibra-fixture",
            "seguridad",
            6,
            42,
            Some(Path::new(FIXTURE)),
            Some(&presupuesto),
            &evaluador,
        );
        Calibraciones::cargar(home.path()).expect("carga").filas
    };
    let (primera, segunda) = (correr(), correr());
    assert_eq!(primera.len(), 1);
    assert_eq!(segunda.len(), 1);
    let (a, b) = (&primera[0], &segunda[0]);
    assert!(a.incompleta && b.incompleta);
    assert_eq!(
        (a.n, a.defectos, a.limpios, a.no_aplicable),
        (b.n, b.defectos, b.limpios, b.no_aplicable)
    );
    assert_eq!(a.sensibilidad, b.sensibilidad);
    assert_eq!(a.especificidad, b.especificidad);
    assert_eq!(a.exactitud_balanceada, b.exactitud_balanceada);
    assert_eq!(a.items, b.items);
}

#[test]
fn calibra_cli_tope_usd_no_positivo_o_no_finito_sale_con_2() {
    for valor in ["0", "-1", "nan"] {
        let home = home_con_tarea("");
        let salida = r00t(
            home.path(),
            &[
                "oraculo",
                "calibrar",
                "--tarea",
                "calibra-fixture",
                "--dimension",
                "seguridad",
                "--repo",
                FIXTURE,
                "--tope-usd",
                valor,
            ],
        );
        assert_eq!(
            codigo(&salida),
            2,
            "--tope-usd {valor}: {}",
            String::from_utf8_lossy(&salida.stderr)
        );
        let stderr = String::from_utf8_lossy(&salida.stderr);
        assert!(
            stderr.contains("finito mayor que 0"),
            "--tope-usd {valor}: {stderr}"
        );
        assert!(
            !home.path().join(ARCHIVO).exists(),
            "--tope-usd {valor}: sin fila"
        );
    }
}

#[test]
fn calibra_peor_caso_de_home_sin_juez_no_lee_nada_y_con_juez_de_diseno_es_finito() {
    let home = tempfile::tempdir().expect("home");
    let seguridad = Partes {
        dimension: Some(Dimension::Seguridad),
        nativas: true,
        ..Partes::default()
    };
    let peor =
        root::oracle::calibra::peor_caso_de_home(home.path(), &seguridad).expect("sin jueces");
    assert_eq!(peor, 0.0);
    let diseno = Partes {
        dimension: Some(Dimension::Diseno),
        juez_diseno: true,
        ..Partes::default()
    };
    let peor = root::oracle::calibra::peor_caso_de_home(home.path(), &diseno)
        .expect("el juez de diseño se rutea con el home");
    assert!(peor.is_finite() && peor > 0.0, "{peor}");
}
