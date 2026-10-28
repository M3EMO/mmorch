use std::process::{Command, Output};

fn root(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_r00t"))
        .args(args)
        .output()
        .expect("no se pudo ejecutar el binario root")
}

#[test]
fn route_esquema_de_uso_politica_y_sinpresupuesto_salen_con_0() {
    for tipo in ["uso", "politica", "sinpresupuesto"] {
        let salida = root(&["esquema", tipo]);
        assert_eq!(salida.status.code(), Some(0), "falla con {tipo}");
        serde_json::from_slice::<serde_json::Value>(&salida.stdout)
            .unwrap_or_else(|e| panic!("JSON inválido para {tipo}: {e}"));
    }
}
