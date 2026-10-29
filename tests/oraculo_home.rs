//! `evaluar` y las llamadas que arma escriben en su `home`, nunca en el `ROOT_HOME` del proceso.
//! Un solo test: `ROOT_HOME` y las claves viven en el entorno del proceso, y así no hay otro test en
//! paralelo que los lea. No hace falta podman para llegar a los `Gate`: el juez se arma antes de
//! cualquier suite.

use std::fs;
use std::path::Path;
use std::time::Duration;

use root::observe;
use root::oracle::tarea::Tarea;
use root::oracle::{OpcionesOraculo, evaluar};
use root::proveedores::{self, Prompt};
use root::route::comando::rutear_en;
use root::tipos::{Eleccion, Proveedor, Rol, TipoEvento, Via};
use serde_json::Value;

const SPEC_CON_FICHA: &str = include_str!("fixtures/fichas/spec-completa.md");

/// Topes de API y de cupo en 1 USD: con el gasto sembrado, los dos están llenos.
const ROOT_TOPE_LLENO: &str = "[balance.api]\ntope_usd = 1.0\n\n[balance.cupo]\ntope_usd = 1.0\n";

/// Tarea mínima con ficha válida. Sus suites no llegan a correr: el juez se arma antes.
fn tarea_minima(dir: &Path) -> Tarea {
    let texto = "nombre = \"tarea-prueba\"\nlenguaje = \"shell\"\nimagen = \"docker.io/library/alpine:3.20\"\nspec = \"docs/SPEC.md\"\n\n[contrato]\ncmd = '''true'''\n\n[oculto]\ncmd = '''true'''\n\n[[requisito]]\nid = \"R1\"\ntitulo = \"ignorado\"\n";
    fs::write(dir.join("tarea.toml"), texto).expect("escribe tarea.toml");
    fs::create_dir_all(dir.join("docs")).expect("dir docs");
    fs::write(dir.join("docs/SPEC.md"), SPEC_CON_FICHA).expect("escribe SPEC.md");
    Tarea::cargar_de(dir).expect("carga la tarea")
}

/// Siembra en `home/eventos.jsonl` un gasto de API y otro de cupo, ambos de 5 USD y de ahora.
fn gasto_lleno(home: &Path) {
    for via in [Via::Api, Via::Cupo] {
        let mut ev = observe::evento_manual("gasto-previo", TipoEvento::Llamada, None);
        ev.via = Some(via);
        ev.cost_usd = 5.0;
        observe::registrar_en(&home.join("eventos.jsonl"), &ev).expect("siembra el gasto");
    }
}

/// Gates de `home/eventos.jsonl`. Un archivo que no existe no tiene gates.
fn gates(home: &Path) -> Vec<Value> {
    let Ok(texto) = fs::read_to_string(home.join("eventos.jsonl")) else {
        return Vec::new();
    };
    texto
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).expect("línea JSON"))
        .filter(|ev| ev["tipo"] == "gate")
        .collect()
}

#[test]
fn oraculo_home_evaluar_y_llamadas_escriben_solo_en_su_home() {
    let real = tempfile::tempdir().expect("ROOT_HOME del proceso");
    let b = tempfile::tempdir().expect("home propio");
    // SAFETY: único test de este binario; nadie más lee el entorno en paralelo.
    unsafe {
        std::env::set_var("ROOT_HOME", real.path());
        std::env::remove_var("DEEPSEEK_API_KEY");
        std::env::remove_var("ANTHROPIC_API_KEY");
    }
    fs::write(b.path().join("root.toml"), ROOT_TOPE_LLENO).expect("root.toml de B");
    gasto_lleno(b.path());

    // 1. `rutear_en` con presupuesto lleno registra su Gate en B.
    let error = rutear_en(b.path(), Rol::Juzga, "diseno", false, 100)
        .expect_err("sin presupuesto: el Gate va a B");
    assert!(error.to_string().contains("sin presupuesto"), "{error}");
    assert_eq!(gates(b.path()).len(), 1, "un Gate en B");

    // 2. `llamar_en` con un modelo sin precio registra su Gate de tope en B y no envía nada.
    let eleccion = Eleccion {
        via: Via::Api,
        proveedor: Some(Proveedor::Anthropic),
        modelo: Some("modelo-sin-precio".into()),
        razonamiento: true,
        motivo: "prueba de home".into(),
        rol: Some(Rol::Juzga),
    };
    let prompt = Prompt {
        sistema: None,
        usuario: "hola".into(),
        max_tokens: 10,
        fase: "prueba".into(),
    };
    let error = proveedores::llamar_en(b.path(), &eleccion, &prompt, "prueba-home")
        .expect_err("sin precio: no se envía");
    assert!(error.to_string().contains("sin precio"), "{error}");
    assert!(
        gates(b.path())
            .iter()
            .any(|g| g["corrida"] == "prueba-home"),
        "el Gate de tope va a B"
    );

    // 3. `evaluar` con `home = B`: el juez y el redactor de producción registran sus Gates en B.
    let antes = gates(b.path()).len();
    let tarea_dir = tempfile::tempdir().expect("tarea");
    let tarea = tarea_minima(tarea_dir.path());
    let repo = tempfile::tempdir().expect("repo");
    fs::write(repo.path().join("a.txt"), "x").expect("archivo del repo");
    let opciones = OpcionesOraculo {
        timeout: Duration::from_secs(60),
        home: Some(b.path().to_path_buf()),
    };
    let _ = evaluar(repo.path(), &tarea, &opciones);
    let despues = gates(b.path());
    assert!(
        despues.len() >= antes + 2,
        "juez y redactor de evaluar registran Gate en B: antes {antes}, después {}",
        despues.len()
    );

    // 4. Nada se escribió en el ROOT_HOME del proceso.
    assert!(
        !real.path().join("eventos.jsonl").exists(),
        "el ROOT_HOME real no gana eventos.jsonl"
    );
    assert!(
        !real.path().join("tmp").exists(),
        "el ROOT_HOME real no gana temporales"
    );
}
