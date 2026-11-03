//! Tests de `r00t oraculo calibrar` (Tarea 12): la salida del binario y la escritura de `calibracion.json`.
//! El Oracle real corre con podman; estos tests no lo llaman: solo prueban los errores de uso y la escritura.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use std::cell::RefCell;

use root::config::Config;
use root::observe;
use root::oracle::calibra::{
    ARCHIVO, Calibraciones, CasoGuardado, Fila, JuecesCaso, Presupuesto, agregar, calibrar_tarea,
    calibrar_tarea_con, global_de, peor_caso_con_jueces,
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
    .expect("con tope suficiente, la calibración termina")
    .fila;
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

/// Árbol de `ruta`: ruta relativa y contenido de cada archivo. Sirve para saber si una copia es igual al repo bueno.
fn arbol(ruta: &Path) -> std::collections::BTreeMap<String, String> {
    fn recoger(raiz: &Path, dir: &Path, mapa: &mut std::collections::BTreeMap<String, String>) {
        for entrada in std::fs::read_dir(dir).expect("lee carpeta") {
            let ruta = entrada.expect("entrada").path();
            if ruta.is_dir() {
                recoger(raiz, &ruta, mapa);
            } else {
                let rel = ruta
                    .strip_prefix(raiz)
                    .expect("dentro de la raíz")
                    .to_string_lossy()
                    .replace('\\', "/");
                mapa.insert(rel, std::fs::read_to_string(&ruta).unwrap_or_default());
            }
        }
    }
    let mut mapa = std::collections::BTreeMap::new();
    recoger(ruta, ruta, &mut mapa);
    mapa
}

/// Si `ruta` es igual al repo bueno del fixture: la copia limpia, o el propio repo.
fn es_limpia(ruta: &Path) -> bool {
    arbol(ruta) == arbol(Path::new(FIXTURE))
}

/// Veredicto de Seguridad con `valor` y, si `sin` es `Some`, sin veredicto con ese motivo.
fn veredicto_seguridad(valor: f32, sin: Option<&str>) -> Veredicto {
    Veredicto {
        puntajes: vec![Puntaje {
            dimension: Dimension::Seguridad,
            valor,
            calibrada: false,
            exactitud_balanceada: None,
            items: vec![],
            sin_veredicto: sin.map(str::to_string),
            faltantes: vec![],
        }],
        ..Veredicto::default()
    }
}

/// Evaluador falso de Seguridad: la base y las copias limpias valen 1. Una copia con defecto vale 0; con `sin`,
/// no tiene veredicto de Seguridad y deja ese motivo.
fn evaluador_con_defectos(
    sin: Option<&'static str>,
) -> impl Fn(&Tarea, &Path, &OpcionesOraculo) -> Result<Veredicto, ErrorOraculo> {
    move |_: &Tarea, ruta: &Path, _: &OpcionesOraculo| {
        Ok(if es_limpia(ruta) {
            veredicto_seguridad(1.0, None)
        } else {
            match sin {
                Some(motivo) => veredicto_seguridad(1.0, Some(motivo)),
                None => veredicto_seguridad(0.0, None),
            }
        })
    }
}

/// Archivos `.casos.jsonl` de `home/calibracion`, ordenados. Vacío si no hay carpeta.
fn archivos_de_casos(home: &Path) -> Vec<PathBuf> {
    let Ok(entradas) = std::fs::read_dir(home.join("calibracion")) else {
        return Vec::new();
    };
    let mut rutas: Vec<PathBuf> = entradas
        .map(|e| e.expect("entrada").path())
        .filter(|p| p.to_string_lossy().ends_with(".casos.jsonl"))
        .collect();
    rutas.sort();
    rutas
}

/// Renglones de un `.casos.jsonl`, cada uno parseado como `CasoGuardado`.
fn renglones(ruta: &Path) -> Vec<CasoGuardado> {
    std::fs::read_to_string(ruta)
        .expect("lee los casos")
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("renglón inválido {l}: {e}")))
        .collect()
}

#[test]
fn calibra_casos_guarda_un_renglon_por_caso_medido_en_orden() {
    let home = home_con_tarea("");
    let evaluador = evaluador_con_defectos(None);
    let calibrada = calibrar_tarea_con(
        home.path(),
        "calibra-fixture",
        "seguridad",
        4,
        42,
        Some(Path::new(FIXTURE)),
        None,
        &evaluador,
    )
    .expect("calibra");
    let ruta = calibrada.casos.clone().expect("hay casos guardados");
    assert_eq!(
        ruta.parent().expect("carpeta"),
        home.path().join("calibracion")
    );
    let ts = calibrada.fila.ts as u64;
    assert_eq!(
        ruta.file_name().expect("nombre").to_string_lossy(),
        format!("calibra-fixture-seguridad-{ts}.casos.jsonl")
    );
    let casos = renglones(&ruta);
    assert_eq!(
        casos.len(),
        calibrada.fila.n as usize,
        "un renglón por caso"
    );
    for (i, caso) in casos.iter().enumerate() {
        assert_eq!(caso.caso, i as u32, "índice del bucle");
        assert!(!caso.repetido_por_entorno);
        if i % 2 == 0 {
            assert!(caso.es_defecto, "los pares llevan defecto");
            assert!(
                matches!(
                    caso.defecto.as_deref(),
                    Some("secreto_en_codigo" | "panico_en_entrada")
                ),
                "{:?}",
                caso.defecto
            );
            assert!(
                caso.item
                    .as_deref()
                    .is_some_and(|i| i.starts_with("seguridad."))
            );
            assert!(caso.archivo.is_some(), "dice el archivo");
            assert!(caso.linea.is_some(), "dice la línea");
            assert!(caso.detectado, "el valor bajó: se detecta");
            assert_eq!(caso.sin_veredicto, None);
        } else {
            assert!(!caso.es_defecto, "los impares son limpios");
            assert_eq!(caso.defecto, None);
            assert_eq!(caso.item, None);
            assert_eq!(caso.archivo, None);
            assert_eq!(caso.linea, None);
            assert!(!caso.detectado);
        }
    }
}

#[test]
fn calibra_casos_defecto_sin_veredicto_guarda_el_motivo_y_no_detecta() {
    let home = home_con_tarea("");
    let evaluador = evaluador_con_defectos(Some("sin sonda"));
    calibrar_tarea_con(
        home.path(),
        "calibra-fixture",
        "seguridad",
        4,
        42,
        Some(Path::new(FIXTURE)),
        None,
        &evaluador,
    )
    .expect("calibra");
    let archivos = archivos_de_casos(home.path());
    assert_eq!(archivos.len(), 1);
    let casos = renglones(&archivos[0]);
    for caso in &casos {
        if caso.es_defecto {
            assert_eq!(caso.sin_veredicto.as_deref(), Some("sin sonda"));
            assert!(!caso.detectado, "sin veredicto no es detección");
        } else {
            assert_eq!(caso.sin_veredicto, None);
        }
    }
}

#[test]
fn calibra_casos_detencion_por_tope_guarda_los_casos_hechos() {
    // Como `calibra_tope_detiene_antes_del_caso_que_no_entra`: base y dos casos entran; el tercero no.
    let home = home_con_tarea("");
    let peor = |_: &Partes| Ok::<f64, ErrorOraculo>(0.40);
    let gasto = medidor(home.path());
    let presupuesto = Presupuesto {
        usd: 1.0,
        peor_caso: &peor,
        gasto_desde: &gasto,
    };
    let evaluador = |tarea: &Tarea, ruta: &Path, opciones: &OpcionesOraculo| {
        gasta(home.path(), 0.30);
        evaluador_con_defectos(None)(tarea, ruta, opciones)
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
    let archivos = archivos_de_casos(home.path());
    assert_eq!(archivos.len(), 1, "los casos hechos quedan");
    let casos = renglones(&archivos[0]);
    assert_eq!(casos.len(), 2, "dos casos medidos");
    assert!(casos[0].es_defecto);
    assert!(!casos[1].es_defecto);
}

#[test]
fn calibra_casos_sin_casos_medidos_no_escribe_archivo() {
    // El peor caso (2,00) no entra en el tope (1,00): se detiene antes de la base, sin casos.
    let home = home_con_tarea("");
    let peor = |_: &Partes| Ok::<f64, ErrorOraculo>(2.00);
    let gasto = medidor(home.path());
    let presupuesto = Presupuesto {
        usd: 1.0,
        peor_caso: &peor,
        gasto_desde: &gasto,
    };
    let evaluador = evaluador_con_defectos(None);
    calibrar_tarea_con(
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
    assert!(
        archivos_de_casos(home.path()).is_empty(),
        "sin casos, sin archivo"
    );
}

#[test]
fn calibra_casos_dos_calibraciones_no_se_pisan() {
    let home = home_con_tarea("");
    let evaluador = evaluador_con_defectos(None);
    let mut rutas = Vec::<PathBuf>::new();
    for semilla in [42, 43] {
        let calibrada = calibrar_tarea_con(
            home.path(),
            "calibra-fixture",
            "seguridad",
            4,
            semilla,
            Some(Path::new(FIXTURE)),
            None,
            &evaluador,
        )
        .expect("calibra");
        rutas.push(calibrada.casos.expect("hay casos guardados"));
    }
    assert_ne!(rutas[0], rutas[1], "cada calibración tiene su archivo");
    assert_eq!(archivos_de_casos(home.path()).len(), 2);
    for ruta in &rutas {
        assert_eq!(renglones(ruta).len(), 4, "cada archivo conserva sus casos");
    }
}

/// Motivo de la fila de una dimensión que la tarea no aplica.
const MOTIVO_NO_APLICA: &str = "no aplica: la tarea no define [eficiencia]";

/// Tarea con `[eficiencia]`: su comando nunca corre en estos tests, solo define que la dimensión aplica.
const EFICIENCIA: &str = "\n[eficiencia]\ncmd = \"bench-eficiencia\"\npiso = 5000.0\n";

/// Evaluador que cuenta sus llamadas y devuelve un veredicto sin hallazgos.
fn evaluador_contador(
    llamadas: &RefCell<u32>,
) -> impl Fn(&Tarea, &Path, &OpcionesOraculo) -> Result<Veredicto, ErrorOraculo> + '_ {
    move |_: &Tarea, _: &Path, _: &OpcionesOraculo| {
        *llamadas.borrow_mut() += 1;
        Ok(veredicto_plano())
    }
}

#[test]
fn eficiencia_sin_bloque_no_aplica_y_no_llama_al_evaluador() {
    let home = home_con_tarea("");
    let llamadas = RefCell::new(0u32);
    let evaluador = evaluador_contador(&llamadas);
    let calibrada = calibrar_tarea_con(
        home.path(),
        "calibra-fixture",
        "eficiencia",
        4,
        42,
        Some(Path::new(FIXTURE)),
        None,
        &evaluador,
    )
    .expect("una dimensión que no aplica no es error");
    assert_eq!(*llamadas.borrow(), 0, "no corre la base ni los casos");
    let fila = &calibrada.fila;
    assert_eq!(fila.no_aplica.as_deref(), Some(MOTIVO_NO_APLICA));
    assert_eq!((fila.n, fila.defectos, fila.limpios), (0, 0, 0));
    assert_eq!(fila.sensibilidad, None);
    assert_eq!(fila.especificidad, None);
    assert_eq!(fila.exactitud_balanceada, None);
    assert!(fila.items.is_empty());
    assert!(!fila.incompleta);
    assert!(calibrada.casos.is_none(), "sin casos no hay .casos.jsonl");
    assert!(
        !home.path().join("calibracion").exists(),
        "no crea la carpeta de casos"
    );
}

#[test]
fn eficiencia_no_aplicable_queda_en_calibracion_json() {
    let home = home_con_tarea("");
    let llamadas = RefCell::new(0u32);
    let evaluador = evaluador_contador(&llamadas);
    calibrar_tarea_con(
        home.path(),
        "calibra-fixture",
        "eficiencia",
        4,
        42,
        Some(Path::new(FIXTURE)),
        None,
        &evaluador,
    )
    .expect("calibra");
    let leido = Calibraciones::cargar(home.path()).expect("carga calibracion.json");
    assert_eq!(leido.filas.len(), 1, "agrega una fila");
    assert_eq!(leido.filas[0].no_aplica.as_deref(), Some(MOTIVO_NO_APLICA));
}

#[test]
fn eficiencia_no_aplicable_no_calibra_ni_entra_a_global() {
    let home = home_con_tarea("");
    let llamadas = RefCell::new(0u32);
    let evaluador = evaluador_contador(&llamadas);
    calibrar_tarea_con(
        home.path(),
        "calibra-fixture",
        "eficiencia",
        4,
        42,
        Some(Path::new(FIXTURE)),
        None,
        &evaluador,
    )
    .expect("calibra");
    // Seguridad calibrada en la misma tarea: `global` la cuenta y Eficiencia no.
    let seguridad = Fila {
        tarea: "calibra-fixture".into(),
        dimension: Dimension::Seguridad,
        n: 20,
        defectos: 10,
        limpios: 10,
        no_aplicable: 0,
        semilla: 42,
        ts: 1.0,
        sensibilidad: Some(1.0),
        especificidad: Some(1.0),
        exactitud_balanceada: Some(0.95),
        items: Default::default(),
        incompleta: false,
        no_aplica: None,
        reemplazos: 0,
    };
    agregar(home.path(), &seguridad).expect("agrega seguridad");
    let cal = Calibraciones::cargar(home.path()).expect("carga");
    let mut puntajes = vec![
        Puntaje {
            dimension: Dimension::Eficiencia,
            valor: 0.5,
            calibrada: true,
            exactitud_balanceada: None,
            items: vec![],
            sin_veredicto: None,
            faltantes: vec![],
        },
        Puntaje {
            dimension: Dimension::Seguridad,
            valor: 0.8,
            calibrada: false,
            exactitud_balanceada: None,
            items: vec![],
            sin_veredicto: None,
            faltantes: vec![],
        },
    ];
    cal.aplicar("calibra-fixture", &mut puntajes);
    assert!(!puntajes[0].calibrada, "eficiencia no aplicable no calibra");
    assert!(puntajes[1].calibrada, "seguridad sí calibra");
    assert_eq!(
        global_de(&puntajes),
        Some(0.8),
        "global solo cuenta seguridad"
    );
    puntajes.remove(1);
    assert_eq!(global_de(&puntajes), None, "sin calibradas no hay global");
}

#[test]
fn eficiencia_no_aplicable_cli_sale_con_0_e_imprime_el_motivo() {
    let home = home_con_tarea("");
    let salida = r00t(
        home.path(),
        &[
            "oraculo",
            "calibrar",
            "--tarea",
            "calibra-fixture",
            "--dimension",
            "eficiencia",
            "--repo",
            FIXTURE,
        ],
    );
    assert_eq!(
        codigo(&salida),
        0,
        "{}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let texto = String::from_utf8_lossy(&salida.stdout);
    assert!(
        texto.contains("calibra-fixture/eficiencia: no aplica: la tarea no define [eficiencia]"),
        "{texto}"
    );
}

#[test]
fn eficiencia_no_aplicable_cli_json_imprime_la_fila_con_no_aplica() {
    let home = home_con_tarea("");
    let salida = r00t(
        home.path(),
        &[
            "--json",
            "oraculo",
            "calibrar",
            "--tarea",
            "calibra-fixture",
            "--dimension",
            "eficiencia",
            "--repo",
            FIXTURE,
        ],
    );
    assert_eq!(
        codigo(&salida),
        0,
        "{}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let fila: serde_json::Value =
        serde_json::from_slice(&salida.stdout).expect("la salida es la fila en JSON");
    assert_eq!(fila["no_aplica"], json!(MOTIVO_NO_APLICA));
}

#[test]
fn eficiencia_con_bloque_mantiene_la_medicion_previa() {
    // Con `[eficiencia]` la dimensión aplica: la base corre. El evaluador de prueba no da veredicto de
    // Eficiencia, así que la calibración falla como antes, sin fila.
    let home = home_con_tarea(EFICIENCIA);
    let llamadas = RefCell::new(0u32);
    let evaluador = evaluador_contador(&llamadas);
    let error = calibrar_tarea_con(
        home.path(),
        "calibra-fixture",
        "eficiencia",
        4,
        42,
        Some(Path::new(FIXTURE)),
        None,
        &evaluador,
    )
    .expect_err("sin veredicto de la base no hay fila");
    assert!(*llamadas.borrow() >= 1, "la base sí corre");
    let ErrorOraculo::Infra(mensaje) = &error else {
        panic!("error de infraestructura esperado: {error:?}");
    };
    assert!(
        mensaje.contains("la línea base no tiene veredicto de eficiencia"),
        "{mensaje}"
    );
    assert!(!home.path().join(ARCHIVO).exists(), "no escribe fila");
}

#[test]
fn fila_vieja_sin_no_aplica_se_lee() {
    let fila = Fila {
        tarea: "calibra-fixture".into(),
        dimension: Dimension::Seguridad,
        n: 20,
        defectos: 10,
        limpios: 10,
        no_aplicable: 0,
        semilla: 42,
        ts: 1.0,
        sensibilidad: Some(1.0),
        especificidad: Some(1.0),
        exactitud_balanceada: Some(0.95),
        items: Default::default(),
        incompleta: false,
        no_aplica: None,
        reemplazos: 0,
    };
    let mut vieja = serde_json::to_value(&fila).expect("serializa");
    vieja.as_object_mut().expect("objeto").remove("no_aplica");
    let leida: Fila = serde_json::from_value(vieja).expect("un JSON viejo sin no_aplica parsea");
    assert_eq!(leida.no_aplica, None);
    assert_eq!(leida, fila);
}

#[test]
fn eficiencia_no_aplicable_no_borra_las_filas_anteriores() {
    // Una fila `no_aplica` nueva se agrega al final: las filas anteriores quedan como estaban.
    let home = home_con_tarea("");
    let antes = Fila {
        tarea: "calibra-fixture".into(),
        dimension: Dimension::Eficiencia,
        n: 20,
        defectos: 10,
        limpios: 10,
        no_aplicable: 0,
        semilla: 1,
        ts: 1.0,
        sensibilidad: Some(1.0),
        especificidad: Some(1.0),
        exactitud_balanceada: Some(0.95),
        items: Default::default(),
        incompleta: false,
        no_aplica: None,
        reemplazos: 0,
    };
    agregar(home.path(), &antes).expect("agrega");
    let llamadas = RefCell::new(0u32);
    let evaluador = evaluador_contador(&llamadas);
    calibrar_tarea_con(
        home.path(),
        "calibra-fixture",
        "eficiencia",
        4,
        42,
        Some(Path::new(FIXTURE)),
        None,
        &evaluador,
    )
    .expect("calibra");
    let leido = Calibraciones::cargar(home.path()).expect("carga");
    assert_eq!(leido.filas.len(), 2, "la fila anterior sigue");
    assert_eq!(leido.filas[0], antes);
}

/// Veredicto de `dimension` con `valor` y, si `sin` es `Some`, sin veredicto con ese motivo.
fn veredicto_con(dimension: Dimension, valor: f32, sin: Option<&str>) -> Veredicto {
    Veredicto {
        puntajes: vec![Puntaje {
            dimension,
            valor,
            calibrada: false,
            exactitud_balanceada: None,
            items: vec![],
            sin_veredicto: sin.map(str::to_string),
            faltantes: vec![],
        }],
        ..Veredicto::default()
    }
}

#[test]
fn calibra_casos_inyeccion_invalida_reemplazada_queda_en_el_renglon() {
    let home = home_con_tarea("");
    let primera = RefCell::new(true);
    let evaluador =
        |_: &Tarea, ruta: &Path, _: &OpcionesOraculo| -> Result<Veredicto, ErrorOraculo> {
            Ok(if es_limpia(ruta) {
                veredicto_con(Dimension::Estructura, 1.0, None)
            } else if primera.replace(false) {
                let motivo = "el código no compila: error[E0432]: unresolved import";
                veredicto_con(Dimension::Estructura, 1.0, Some(motivo))
            } else {
                veredicto_con(Dimension::Estructura, 0.0, None)
            })
        };
    let calibrada = calibrar_tarea_con(
        home.path(),
        "calibra-fixture",
        "estructura",
        2,
        42,
        Some(Path::new(FIXTURE)),
        None,
        &evaluador,
    )
    .expect("calibra");
    assert_eq!(calibrada.fila.reemplazos, 1);
    let ruta = calibrada.casos.expect("hay casos guardados");
    let reemplazos: Vec<u32> = renglones(&ruta).iter().map(|c| c.reemplazos).collect();
    assert_eq!(reemplazos, vec![1, 0], "el reemplazo cae en el caso 0");
}
