use std::process::{Command, Output};

fn root(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_root"))
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
fn subcomando_sin_implementar_sale_con_3() {
    let salida = root(&["esquema", "veredicto"]);
    assert_eq!(codigo(&salida), 3);
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(stderr.contains("no implementado"), "stderr: {stderr}");
}

#[test]
fn subcomando_inexistente_sale_con_2() {
    let salida = root(&["subcomando-que-no-existe"]);
    assert_eq!(codigo(&salida), 2);
}
