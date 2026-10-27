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

#[test]
fn esquema_veredicto_imprime_json_schema() {
    let salida = root(&["esquema", "veredicto"]);
    assert_eq!(codigo(&salida), 0);
    let schema: serde_json::Value =
        serde_json::from_slice(&salida.stdout).expect("stdout debe ser JSON válido");
    assert_eq!(schema["title"], "Veredicto");
}

#[test]
fn esquema_de_cada_tipo_listado_sale_con_0() {
    let tipos = [
        "veredicto",
        "evento",
        "fila",
        "pedido",
        "eleccion",
        "resumen",
        "hallazgo",
        "puntaje",
        "conteo",
        "config",
    ];
    for tipo in tipos {
        let salida = root(&["esquema", tipo]);
        assert_eq!(codigo(&salida), 0, "falla con {tipo}");
        serde_json::from_slice::<serde_json::Value>(&salida.stdout)
            .unwrap_or_else(|e| panic!("JSON inválido para {tipo}: {e}"));
    }
}

#[test]
fn esquema_informe_imprime_json_schema() {
    let salida = root(&["esquema", "informe"]);
    assert_eq!(codigo(&salida), 0);
    let schema: serde_json::Value =
        serde_json::from_slice(&salida.stdout).expect("stdout debe ser JSON válido");
    assert_eq!(schema["title"], "Informe");
}

#[test]
fn esquema_tipos_de_extension_pm_salen_con_0() {
    for tipo in ["estadorequisito", "accion", "nivelitem"] {
        let salida = root(&["esquema", tipo]);
        assert_eq!(codigo(&salida), 0, "falla con {tipo}");
        serde_json::from_slice::<serde_json::Value>(&salida.stdout)
            .unwrap_or_else(|e| panic!("JSON inválido para {tipo}: {e}"));
    }
}

#[test]
fn esquema_tipo_desconocido_sale_con_2() {
    let salida = root(&["esquema", "nada"]);
    assert_eq!(codigo(&salida), 2);
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(stderr.contains("uso"), "stderr: {stderr}");
}
