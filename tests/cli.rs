use std::process::{Command, Output};

fn root(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_r00t"))
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

const SUBCOMANDOS: &[&str] = &[
    "route",
    "llama",
    "oraculo",
    "observa",
    "track",
    "aprende",
    "corre",
    "goal",
    "importar-mmorch",
    "esquema",
    "mcp",
];

#[test]
fn help_lista_todos_los_subcomandos() {
    let salida = root(&["--help"]);
    assert_eq!(codigo(&salida), 0);
    let stdout = String::from_utf8_lossy(&salida.stdout);
    for nombre in SUBCOMANDOS {
        assert!(
            stdout.contains(nombre),
            "falta '{nombre}' en --help:\n{stdout}"
        );
    }
}

#[test]
fn help_del_binario_dice_r00t() {
    let salida = root(&["--help"]);
    assert_eq!(codigo(&salida), 0);
    let stdout = String::from_utf8_lossy(&salida.stdout);
    assert!(stdout.contains("Usage: r00t"), "stdout:\n{stdout}");
}

#[test]
fn corre_sin_goal_en_el_directorio_sale_con_3() {
    // `corre` ya tiene lógica: sin GOAL autorizado en el directorio, se detiene antes de tocar nada.
    let dir = tempfile::tempdir().expect("temp dir");
    let salida = Command::new(env!("CARGO_BIN_EXE_r00t"))
        .current_dir(dir.path())
        .args(["corre", "--tarea", "memora", "--config", "ecc-haiku"])
        .output()
        .expect("no se pudo ejecutar el binario root");
    assert_eq!(codigo(&salida), 3);
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(stderr.contains("GOAL no autorizado"), "stderr: {stderr}");
}

#[test]
fn subcomando_inexistente_sale_con_2() {
    let salida = root(&["subcomando-que-no-existe"]);
    assert_eq!(codigo(&salida), 2);
}

/// Corre `r00t` con `ROOT_HOME` de `valor`, sin tocar el ROOT_HOME real.
fn root_con_home(args: &[&str], valor: &std::ffi::OsStr) -> Output {
    Command::new(env!("CARGO_BIN_EXE_r00t"))
        .args(args)
        .env("ROOT_HOME", valor)
        .output()
        .expect("no se pudo ejecutar el binario root")
}

#[test]
fn root_home_no_absoluta_o_raiz_o_home_sale_con_2_en_cualquier_subcomando() {
    let home = std::env::var("HOME").expect("HOME definido");
    for valor in ["/", "relativo/root", ".", home.as_str()] {
        let salida = root_con_home(&["esquema", "veredicto"], std::ffi::OsStr::new(valor));
        assert_eq!(codigo(&salida), 2, "ROOT_HOME={valor}");
        let stderr = String::from_utf8_lossy(&salida.stderr);
        assert!(stderr.contains("ROOT_HOME"), "stderr: {stderr}");
    }
}

#[test]
fn root_home_absoluta_temporal_sigue_funcionando() {
    let temporal = tempfile::tempdir().expect("ROOT_HOME temporal");
    let salida = root_con_home(&["esquema", "veredicto"], temporal.path().as_os_str());
    assert_eq!(
        codigo(&salida),
        0,
        "{}",
        String::from_utf8_lossy(&salida.stderr)
    );
}

#[test]
fn pendientes_nombre_de_tarea_con_ruta_sale_con_2_y_no_escribe() {
    let home = tempfile::tempdir().expect("ROOT_HOME temporal");
    let tarea = home.path().join("oraculos").join("memora");
    std::fs::create_dir_all(&tarea).expect("carpeta de la tarea");
    let toml = "nombre = \"../../fuera\"\nlenguaje = \"rust\"\nimagen = \"i\"\nspec = \"s\"\n\
                [contrato]\ncmd = \"c\"\n[oculto]\ncmd = \"o\"\n\
                [[entorno]]\ntest = \"a.py::t\"\nmotivo = \"m\"\n";
    std::fs::write(tarea.join("tarea.toml"), toml).expect("tarea.toml");
    let repo = tempfile::tempdir().expect("repo");
    let salida = Command::new(env!("CARGO_BIN_EXE_r00t"))
        .args(["oraculo", "pendientes", "--tarea", "memora", "--repo"])
        .arg(repo.path())
        .env("ROOT_HOME", home.path())
        .output()
        .expect("binario");
    assert_eq!(
        codigo(&salida),
        2,
        "{}",
        String::from_utf8_lossy(&salida.stderr)
    );
    assert!(!home.path().join("pendientes").exists());
}
