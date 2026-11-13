//! Tests de `r00t oraculo calibrar` (Tarea 12): la salida del binario y la escritura de `calibracion.json`.
//! El Oracle real corre con podman; estos tests no lo llaman: solo prueban los errores de uso y la escritura.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use std::sync::Mutex;

use root::config::{Config, Precio};
use root::observe;
use root::oracle::calibra::{
    ARCHIVO, Calibraciones, CasoGuardado, Fila, Presupuesto, agregar, calibrar_tarea,
    calibrar_tarea_con, global_de,
};
use root::oracle::tarea::Tarea;
use root::oracle::{ErrorOraculo, OpcionesOraculo, Partes};
use root::proveedores::{
    Breaker, Clave, Claves, Contexto, ErrorProveedor, Plazos, Prompt, llamar_con,
};
use root::tipos::{Dimension, Eleccion, Evento, Proveedor, Rol, TipoEvento, Via};
use root::tipos::{Puntaje, Veredicto};
use serde_json::json;

mod common;
use common::{Servidor, sse_deepseek};

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
            inestables: Vec::new(),
            motivo_sin_calibrar: None,
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
    let vistas = std::sync::Mutex::new(Vec::<Option<Partes>>::new());
    let evaluador = |_: &Tarea, _: &Path, o: &OpcionesOraculo| {
        vistas.lock().expect("lock").push(o.partes);
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
    let vistas = vistas.lock().expect("lock");
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
/// Registra en `home` una llamada de API de `costo` USD, como las que deja un juez real.
/// Gasto de API desde `inicio` en `home`, como lo mide la calibración de producción.
/// Modelo del juicio de prueba: 1000 USD por millón de tokens de salida, sin cobrar la entrada.
const MODELO_JUICIO: &str = "modelo-juicio-prueba";
/// Tokens de salida de un juicio. Con el precio de prueba, 300 tokens cuestan y tienen de cota 0,30 USD.
const SALIDA_JUICIO: u32 = 300;

/// Respuestas del servidor falso para `n` juicios. Cada uno cuesta 0,30 USD.
fn respuestas_de_juicio(n: usize) -> Vec<(u16, String)> {
    vec![(200, sse_deepseek("ok", 0, u64::from(SALIDA_JUICIO), "stop")); n]
}

/// Pide un juicio al servidor falso `url`, con el límite activo del hilo. Si el límite rechaza el intento,
/// no se envía y da error. Un juicio que entra cuesta 0,30 USD, lo que dice la respuesta del servidor.
fn juicio(home: &Path, url: &str) -> Result<(), ErrorProveedor> {
    let mut cfg = Config::default();
    cfg.proveedores.deepseek_url = url.into();
    cfg.precios.insert(
        MODELO_JUICIO.into(),
        Precio {
            usd_entrada: 0.0,
            usd_salida: 1000.0,
            umbral_entrada: None,
            usd_entrada_alto: None,
            usd_salida_alto: None,
        },
    );
    let eleccion = Eleccion {
        via: Via::Api,
        proveedor: Some(Proveedor::Deepseek),
        modelo: Some(MODELO_JUICIO.into()),
        razonamiento: true,
        motivo: "prueba".into(),
        rol: Some(Rol::Juzga),
    };
    let prompt = Prompt {
        sistema: None,
        usuario: "hola".into(),
        max_tokens: SALIDA_JUICIO,
        fase: "fase-prueba".into(),
        reintento_juez: false,
    };
    let claves = Claves {
        deepseek: Some(Clave::nueva("clave-falsa-de-prueba")),
        anthropic: None,
    };
    let breaker = Breaker::new();
    let reloj = std::time::Instant::now;
    let ctx = Contexto {
        cfg: &cfg,
        home,
        breaker: &breaker,
        claves: &claves,
        reloj: &reloj,
        plazos: Plazos::default(),
    };
    llamar_con(&ctx, &eleccion, &prompt, "c-calibra").map(|_| ())
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
    // Cada juicio cuesta 0,30 y tiene 0,30 de cota; el tope es 1,00. La base y dos casos entran (0,90). El
    // juicio del siguiente no entra (0,90 + 0,30 > 1,00): se detiene, con la base y dos casos medidos.
    let home = home_con_tarea("");
    let servidor = Servidor::arranca(respuestas_de_juicio(8));
    let presupuesto = Presupuesto { usd: 1.0 };
    let evaluador = |_: &Tarea, _: &Path, _: &OpcionesOraculo| {
        let _ = juicio(home.path(), &servidor.url);
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
    assert_eq!(
        servidor.peticiones(),
        3,
        "la base y dos casos llegan al servidor"
    );

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
    let servidor = Servidor::arranca(respuestas_de_juicio(8));
    let presupuesto = Presupuesto { usd: 5.0 };
    let evaluador = |_: &Tarea, _: &Path, _: &OpcionesOraculo| {
        let _ = juicio(home.path(), &servidor.url);
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
fn calibra_tope_con_la_base_rechazada_no_escribe_fila() {
    // El juicio de la base cuesta 0,30 de cota y el tope es 0,20: el límite lo rechaza y no sale al servidor.
    let home = home_con_tarea("");
    let servidor = Servidor::arranca(respuestas_de_juicio(8));
    let presupuesto = Presupuesto { usd: 0.20 };
    let evaluador = |_: &Tarea, _: &Path, _: &OpcionesOraculo| {
        let _ = juicio(home.path(), &servidor.url);
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
    assert_eq!(servidor.peticiones(), 0, "ningún juicio sale");
    assert!(!home.path().join(ARCHIVO).exists(), "sin casos, sin fila");
    assert_eq!(gates_de_tope(home.path()).len(), 1);
}

#[test]
fn calibra_tope_tras_la_base_sin_casos_no_escribe_fila() {
    // La base cuesta 0,30 y entra. Cada caso necesita 0,30 más: 0,30 + 0,30 > 0,50. Ningún caso medido: sin fila.
    let home = home_con_tarea("");
    let servidor = Servidor::arranca(respuestas_de_juicio(8));
    let presupuesto = Presupuesto { usd: 0.50 };
    let evaluador = |_: &Tarea, _: &Path, _: &OpcionesOraculo| {
        let _ = juicio(home.path(), &servidor.url);
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
    assert_eq!(servidor.peticiones(), 1, "solo el juicio de la base entra");
    assert!(
        !home.path().join(ARCHIVO).exists(),
        "sin casos medidos, sin fila"
    );
    assert_eq!(gates_de_tope(home.path()).len(), 1);
}

#[test]
fn calibra_tope_incompleta_con_la_misma_semilla_se_repite() {
    let correr = || {
        let home = home_con_tarea("");
        let servidor = Servidor::arranca(respuestas_de_juicio(8));
        let presupuesto = Presupuesto { usd: 1.0 };
        let evaluador = |_: &Tarea, _: &Path, _: &OpcionesOraculo| {
            let _ = juicio(home.path(), &servidor.url);
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
    // Con cuatro hilos, qué casos reservan primero varía entre corridas: lo que se repite es el conteo. La base
    // y dos casos entran en 1,00 con 0,30 por juicio.
    assert_eq!(a.n, 2, "dos casos medidos");
    assert_eq!(a.n, b.n);
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
            inestables: Vec::new(),
            motivo_sin_calibrar: None,
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
    // Como `calibra_tope_detiene_antes_del_caso_que_no_entra`: la base y dos casos entran; el siguiente no.
    let home = home_con_tarea("");
    let servidor = Servidor::arranca(respuestas_de_juicio(8));
    let presupuesto = Presupuesto { usd: 1.0 };
    let evaluador = |tarea: &Tarea, ruta: &Path, opciones: &OpcionesOraculo| {
        let _ = juicio(home.path(), &servidor.url);
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
    // Con cuatro hilos, qué dos casos reservan primero no está fijado: solo cuenta que hay dos.
    assert_eq!(casos.len(), 2, "dos casos medidos");
}

#[test]
fn calibra_casos_sin_casos_medidos_no_escribe_archivo() {
    // El juicio de la base cuesta 0,30 de cota y el tope es 0,20: el límite lo rechaza y no hay casos.
    let home = home_con_tarea("");
    let servidor = Servidor::arranca(respuestas_de_juicio(8));
    let presupuesto = Presupuesto { usd: 0.20 };
    let base = evaluador_con_defectos(None);
    let evaluador = |tarea: &Tarea, ruta: &Path, opciones: &OpcionesOraculo| {
        let _ = juicio(home.path(), &servidor.url);
        base(tarea, ruta, opciones)
    };
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

/// `[eficiencia]` con `camino_caliente` que no existe: ningún defecto de Eficiencia tiene dónde caer.
fn eficiencia_sin_camino() -> String {
    format!("{EFICIENCIA}\n[calibracion]\ncamino_caliente = [\"no_existe\"]\n")
}

#[test]
fn calibra_casos_defecto_no_aplicable_deja_renglon_y_no_suma_a_n() {
    let home = home_con_tarea(&eficiencia_sin_camino());
    let evaluador = |_: &Tarea, _: &Path, _: &OpcionesOraculo| {
        Ok::<Veredicto, ErrorOraculo>(veredicto_con(Dimension::Eficiencia, 1.0, None))
    };
    let calibrada = calibrar_tarea_con(
        home.path(),
        "calibra-fixture",
        "eficiencia",
        2,
        42,
        Some(Path::new(FIXTURE)),
        None,
        &evaluador,
    )
    .expect("calibra");
    assert_eq!(
        (calibrada.fila.n, calibrada.fila.no_aplicable),
        (1, 1),
        "el defecto no suma a n"
    );
    let ruta = calibrada.casos.expect("hay casos guardados");
    let casos = renglones(&ruta);
    assert_eq!(casos.len(), 2, "el no aplicable y el limpio tienen renglón");
    let no = &casos[0];
    assert_eq!(no.caso, 0);
    assert!(no.es_defecto);
    assert!(!no.detectado);
    assert!(no.defecto.is_some(), "dice el defecto");
    assert!(no.item.is_some(), "dice el ítem");
    assert_eq!(no.archivo, None, "no hubo copia que dijera archivo");
    assert_eq!(
        no.no_aplicable.as_deref(),
        Some("sin candidato para inyectar")
    );
    assert_eq!(casos[1].caso, 1);
    assert_eq!(casos[1].no_aplicable, None);
}

#[test]
fn calibra_casos_todos_no_aplicables_escriben_el_archivo_con_sus_renglones() {
    // Un caso, con defecto y sin candidato: no hay caso medido, pero el renglón igual se escribe.
    let home = home_con_tarea(&eficiencia_sin_camino());
    let evaluador = |_: &Tarea, _: &Path, _: &OpcionesOraculo| {
        Ok::<Veredicto, ErrorOraculo>(veredicto_con(Dimension::Eficiencia, 1.0, None))
    };
    let calibrada = calibrar_tarea_con(
        home.path(),
        "calibra-fixture",
        "eficiencia",
        1,
        42,
        Some(Path::new(FIXTURE)),
        None,
        &evaluador,
    )
    .expect("calibra");
    assert_eq!((calibrada.fila.n, calibrada.fila.no_aplicable), (0, 1));
    let ruta = calibrada
        .casos
        .expect("el archivo existe aunque no haya casos medidos");
    let casos = renglones(&ruta);
    assert_eq!(casos.len(), 1);
    assert_eq!(
        casos[0].no_aplicable.as_deref(),
        Some("sin candidato para inyectar")
    );
}

/// Motivo de la fila de una dimensión que la tarea no aplica.
const MOTIVO_NO_APLICA: &str = "no aplica: la tarea no define [eficiencia]";

/// Tarea con `[eficiencia]`: su comando nunca corre en estos tests, solo define que la dimensión aplica.
const EFICIENCIA: &str = "\n[eficiencia]\ncmd = \"bench-eficiencia\"\npiso = 5000.0\n";

/// Evaluador que cuenta sus llamadas y devuelve un veredicto sin hallazgos.
fn evaluador_contador(
    llamadas: &Mutex<u32>,
) -> impl Fn(&Tarea, &Path, &OpcionesOraculo) -> Result<Veredicto, ErrorOraculo> + '_ {
    move |_: &Tarea, _: &Path, _: &OpcionesOraculo| {
        *llamadas.lock().expect("lock") += 1;
        Ok(veredicto_plano())
    }
}

#[test]
fn eficiencia_sin_bloque_no_aplica_y_no_llama_al_evaluador() {
    let home = home_con_tarea("");
    let llamadas = Mutex::new(0u32);
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
    assert_eq!(
        *llamadas.lock().expect("lock"),
        0,
        "no corre la base ni los casos"
    );
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
    let llamadas = Mutex::new(0u32);
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
    let llamadas = Mutex::new(0u32);
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
        tarea_hash: None,
        plazo_mutantes: None,
        defectos_en_piso: 0,
        repo_origen: None,
        inyecciones_distintas: None,
        omisiones: 0,
        tarea: "calibra-fixture".into(),
        dimension: Dimension::Seguridad,
        n: 20,
        defectos: 10,
        limpios: 10,
        limpios_sin_veredicto: 0,
        no_aplicable: 0,
        semilla: 42,
        ts: 1.0,
        sensibilidad: Some(1.0),
        especificidad: Some(1.0),
        exactitud_balanceada: Some(0.95),
        items: Default::default(),
        incompleta: false,
        no_aplica: None,
        inestable: None,
        reemplazos: 0,
        ruidosos: Vec::new(),
        casos_ruidosos: 0,
        huella: None,
        inestables: Vec::new(),
        extractos_base: Vec::new(),
    };
    agregar(home.path(), &seguridad).expect("agrega seguridad");
    let cal = Calibraciones::cargar(home.path()).expect("carga");
    let mut puntajes = vec![
        Puntaje {
            inestables: Vec::new(),
            motivo_sin_calibrar: None,
            dimension: Dimension::Eficiencia,
            valor: 0.5,
            calibrada: true,
            exactitud_balanceada: None,
            items: vec![],
            sin_veredicto: None,
            faltantes: vec![],
        },
        Puntaje {
            inestables: Vec::new(),
            motivo_sin_calibrar: None,
            dimension: Dimension::Seguridad,
            valor: 0.8,
            calibrada: false,
            exactitud_balanceada: None,
            items: vec![],
            sin_veredicto: None,
            faltantes: vec![],
        },
    ];
    cal.aplicar("calibra-fixture", "", &mut puntajes);
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
    let llamadas = Mutex::new(0u32);
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
    assert!(*llamadas.lock().expect("lock") >= 1, "la base sí corre");
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
        tarea_hash: None,
        plazo_mutantes: None,
        defectos_en_piso: 0,
        repo_origen: None,
        inyecciones_distintas: None,
        omisiones: 0,
        tarea: "calibra-fixture".into(),
        dimension: Dimension::Seguridad,
        n: 20,
        defectos: 10,
        limpios: 10,
        limpios_sin_veredicto: 0,
        no_aplicable: 0,
        semilla: 42,
        ts: 1.0,
        sensibilidad: Some(1.0),
        especificidad: Some(1.0),
        exactitud_balanceada: Some(0.95),
        items: Default::default(),
        incompleta: false,
        no_aplica: None,
        inestable: None,
        reemplazos: 0,
        ruidosos: Vec::new(),
        casos_ruidosos: 0,
        huella: None,
        inestables: Vec::new(),
        extractos_base: Vec::new(),
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
        tarea_hash: None,
        plazo_mutantes: None,
        defectos_en_piso: 0,
        repo_origen: None,
        inyecciones_distintas: None,
        omisiones: 0,
        tarea: "calibra-fixture".into(),
        dimension: Dimension::Eficiencia,
        n: 20,
        defectos: 10,
        limpios: 10,
        limpios_sin_veredicto: 0,
        no_aplicable: 0,
        semilla: 1,
        ts: 1.0,
        sensibilidad: Some(1.0),
        especificidad: Some(1.0),
        exactitud_balanceada: Some(0.95),
        items: Default::default(),
        incompleta: false,
        no_aplica: None,
        inestable: None,
        reemplazos: 0,
        ruidosos: Vec::new(),
        casos_ruidosos: 0,
        huella: None,
        inestables: Vec::new(),
        extractos_base: Vec::new(),
    };
    agregar(home.path(), &antes).expect("agrega");
    let llamadas = Mutex::new(0u32);
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
            inestables: Vec::new(),
            motivo_sin_calibrar: None,
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
    let primera = Mutex::new(true);
    let evaluador =
        |_: &Tarea, ruta: &Path, _: &OpcionesOraculo| -> Result<Veredicto, ErrorOraculo> {
            Ok(if es_limpia(ruta) {
                veredicto_con(Dimension::Estructura, 1.0, None)
            } else if std::mem::replace(&mut *primera.lock().expect("lock"), false) {
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
