//! `r00t corre` de punta a punta, con el binario. Ningún test llega a lanzar `claude` ni podman: el HOME de
//! prueba no tiene credencial de Claude, así que el ejecutor de contenedor falla antes.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

use root::goal::calcular_hash;

const GOAL: &str = "# GOAL de prueba\n\nContenido de fixture.\n";

/// Un repo de Root, un `$HOME` y un `ROOT_HOME` de prueba, con la tarea `memo` y la config `ecc-test`.
struct Escenario {
    _tmp: tempfile::TempDir,
    repo: PathBuf,
    usuario: PathBuf,
    home: PathBuf,
}

fn escenario(goal_valido: bool) -> Escenario {
    let tmp = tempfile::tempdir().expect("tmp");
    let base = tmp.path();
    let repo = base.join("repo");
    let usuario = base.join("usuario");
    let home = base.join("root");
    for dir in [&repo, &usuario, &home] {
        fs::create_dir_all(dir).expect("carpeta");
    }

    fs::write(repo.join("GOAL.md"), GOAL).expect("GOAL.md");
    let hash = if goal_valido {
        calcular_hash(GOAL.as_bytes())
    } else {
        "0000000000000000".to_string()
    };
    fs::write(repo.join("GOAL.hash"), format!("{hash}\n")).expect("GOAL.hash");

    let tarea = home.join("oraculos").join("memo");
    fs::create_dir_all(tarea.join("bootstrap").join("docs")).expect("bootstrap");
    fs::write(
        tarea.join("bootstrap").join("docs").join("SPEC.md"),
        "# Spec\n",
    )
    .expect("spec");
    fs::create_dir_all(tarea.join("contrato")).expect("contrato");
    fs::write(
        tarea.join("contrato").join("test_x.py"),
        "def test_x(): pass\n",
    )
    .expect("contrato");
    fs::create_dir_all(tarea.join("oculto")).expect("oculto");
    fs::write(
        tarea.join("oculto").join("test_y.py"),
        "def test_y(): pass\n",
    )
    .expect("oculto");
    fs::write(
        tarea.join("tarea.toml"),
        "nombre = \"memo\"\nlenguaje = \"rust\"\nimagen = \"root-oraculo-rust:1\"\nspec = \"bootstrap/docs/SPEC.md\"\n\n[contrato]\ncmd = \"pytest -q /oraculo/contrato\"\n\n[oculto]\ncmd = \"pytest -q /oraculo/oculto\"\n",
    )
    .expect("tarea.toml");

    let configs = home.join("configs");
    fs::create_dir_all(&configs).expect("configs");
    fs::write(
        configs.join("ecc-test.toml"),
        "roles = [\"planifica\", \"construye\"]\n\n[prompt]\nplanifica = \"haz el plan\"\nconstruye = \"haz hecho\"\n",
    )
    .expect("config");

    Escenario {
        _tmp: tmp,
        repo,
        usuario,
        home,
    }
}

fn root_en(e: &Escenario, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_r00t"))
        .current_dir(&e.repo)
        .env("ROOT_HOME", &e.home)
        .env("HOME", &e.usuario)
        .args(args)
        .output()
        .expect("no se pudo ejecutar el binario root")
}

fn codigo(salida: &Output) -> i32 {
    salida
        .status
        .code()
        .expect("el proceso no terminó con código")
}

fn corre(e: &Escenario, config: &str) -> Output {
    root_en(e, &["corre", "--tarea", "memo", "--config", config])
}

fn sin_rastro(e: &Escenario) -> bool {
    !e.home.join("runs.jsonl").exists() && !e.usuario.join(".cache").exists()
}

#[test]
fn corre_goal_no_autorizado_sale_con_el_codigo_de_goal_verificar() {
    // Dos casos: GOAL.md que no existe, y GOAL.md alterado frente a GOAL.hash.
    let sin_goal = escenario(true);
    fs::remove_file(sin_goal.repo.join("GOAL.md")).expect("quita GOAL.md");
    let alterado = escenario(true);
    fs::write(alterado.repo.join("GOAL.md"), "# GOAL cambiado\n").expect("GOAL.md");

    for e in [&sin_goal, &alterado] {
        let esperado = codigo(&root_en(e, &["goal", "verificar"]));
        assert_ne!(esperado, 0, "el GOAL debe estar sin autorizar");
        let salida = corre(e, "ecc-test");
        assert_eq!(
            codigo(&salida),
            esperado,
            "stderr: {}",
            String::from_utf8_lossy(&salida.stderr)
        );
        assert!(
            sin_rastro(e),
            "no debe escribir runs.jsonl ni crear corridas"
        );
    }
}

#[test]
fn corre_sin_goal_sale_con_3_y_nombra_el_goal() {
    let e = escenario(true);
    fs::remove_file(e.repo.join("GOAL.md")).expect("quita GOAL.md");
    let salida = corre(&e, "ecc-test");
    assert_eq!(codigo(&salida), 3);
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(stderr.contains("GOAL no autorizado"), "stderr: {stderr}");
    assert!(sin_rastro(&e));
}

#[test]
fn corre_config_inexistente_con_goal_valido_sale_con_2() {
    let e = escenario(true);
    let salida = corre(&e, "no-existe");
    assert_eq!(
        codigo(&salida),
        2,
        "stderr: {}",
        String::from_utf8_lossy(&salida.stderr)
    );
    assert!(!e.home.join("runs.jsonl").exists());
}

#[test]
fn corre_sin_home_del_usuario_sale_con_2() {
    let e = escenario(true);
    let salida = Command::new(env!("CARGO_BIN_EXE_r00t"))
        .current_dir(&e.repo)
        .env("ROOT_HOME", &e.home)
        .env_remove("HOME")
        .args(["corre", "--tarea", "memo", "--config", "ecc-test"])
        .output()
        .expect("binario");
    assert_eq!(codigo(&salida), 2);
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(stderr.contains("HOME"), "stderr: {stderr}");
}

#[test]
fn corre_con_goal_valido_sin_credencial_sale_con_3_y_no_escribe_fila() {
    // El HOME de prueba no tiene credencial de Claude: el ejecutor falla antes de buscar o correr
    // podman. La corrida llega al lanzamiento, no lanza nada y sale con 3 sin fila.
    let e = escenario(true);
    let salida = corre(&e, "ecc-test");
    assert_eq!(codigo(&salida), 3);
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(
        stderr.contains("no existe la credencial de Claude"),
        "stderr: {stderr}"
    );
    assert!(
        !e.home.join("runs.jsonl").exists(),
        "sin lanzamiento no hay fila"
    );
}
