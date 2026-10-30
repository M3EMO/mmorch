//! Tests de `r00t oraculo validar` (Tarea 12b) con el binario. El evaluador de producción no corre aquí:
//! sin casos no hay nada que evaluar, y el armado a ciegas usa las rúbricas embebidas (hoy ninguna).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

fn r00t(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_r00t"))
        .env("ROOT_HOME", home)
        .args(args)
        .output()
        .expect("ejecuta r00t")
}

#[test]
fn validez_cli_sin_casos_escribe_sin_medir() {
    let home = TempDir::new().expect("home");
    let casos = TempDir::new().expect("casos");
    let ruta = casos.path().to_str().expect("ruta UTF-8");
    let salida = r00t(home.path(), &["oraculo", "validar", "--casos", ruta]);
    assert!(
        salida.status.success(),
        "sale con 0: {}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let texto = fs::read_to_string(casos.path().join("validez.json")).expect("validez.json");
    let v: Value = serde_json::from_str(&texto).expect("JSON válido");
    for metrica in ["exactitud_orden", "recall", "acuerdo_decision", "spearman"] {
        assert!(
            v[metrica]["valor"].is_null(),
            "{metrica} sin medir, no 0 ni 1"
        );
        assert!(v[metrica]["motivo"].is_string(), "{metrica} lleva motivo");
    }
    assert_eq!(v["kappa_dimension"].as_array().map(Vec::len), Some(9));
}

#[test]
fn validez_cli_preparar_ciego_con_rubricas_embebidas_no_avisa() {
    let home = TempDir::new().expect("home");
    let dir = TempDir::new().expect("dir");
    let repo = dir.path().join("repo");
    fs::create_dir_all(repo.join("src")).expect("repo");
    fs::write(repo.join("src/lib.rs"), "pub fn f() {}\n").expect("lib");
    fs::write(repo.join("AGENTS.md"), "rastro\n").expect("rastro");
    let casos = dir.path().join("casos");
    let manifiesto = dir.path().join("fuentes.toml");
    fs::write(
        &manifiesto,
        format!(
            "[[fuente]]\ntarea = \"memora\"\norigen = \"ecc-sonnet\"\nrepo = \"{}\"\n\n\
             [[fuente]]\ntarea = \"brasa\"\norigen = \"sdlc\"\nrepo = \"{}\"\n",
            repo.display(),
            repo.display()
        ),
    )
    .expect("manifiesto");

    let salida = r00t(
        home.path(),
        &[
            "oraculo",
            "validar",
            "--preparar-ciego",
            "--fuentes",
            manifiesto.to_str().expect("ruta UTF-8"),
            "--casos",
            casos.to_str().expect("ruta UTF-8"),
            "--semilla",
            "7",
        ],
    );
    assert!(
        salida.status.success(),
        "sale con 0: {}",
        String::from_utf8_lossy(&salida.stderr)
    );
    // Con las rúbricas embebidas, el aviso de "sin rúbricas" no sale.
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(
        !stderr.contains("no hay rúbricas embebidas"),
        "con rúbricas embebidas no avisa: {stderr}"
    );

    let ciego = casos.join("ciego");
    assert!(
        ciego.join("clave.toml").is_file(),
        "la clave queda en casos/ciego"
    );
    let paquetes: Vec<_> = fs::read_dir(&ciego)
        .expect("ciego")
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .collect();
    assert_eq!(paquetes.len(), 2, "dos paquetes, uno por fuente");
    for paquete in paquetes {
        assert!(paquete.path().join("repo/src/lib.rs").is_file());
        assert!(!paquete.path().join("repo/AGENTS.md").exists());
        assert!(paquete.path().join("formulario.toml").is_file());
    }
}

#[test]
fn validez_cli_preparar_sin_fuentes_es_uso() {
    let home = TempDir::new().expect("home");
    let casos = TempDir::new().expect("casos");
    let salida = r00t(
        home.path(),
        &[
            "oraculo",
            "validar",
            "--preparar-ciego",
            "--casos",
            casos.path().to_str().expect("ruta UTF-8"),
        ],
    );
    assert_eq!(
        salida.status.code(),
        Some(2),
        "--preparar-ciego exige --fuentes"
    );
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(
        stderr.contains("--fuentes"),
        "clap nombra la opción que falta: {stderr}"
    );
}

#[test]
fn validez_cli_casos_inexistentes_es_uso() {
    let home = TempDir::new().expect("home");
    let dir = TempDir::new().expect("dir");
    let inexistente = dir.path().join("no-existe");
    let salida = r00t(
        home.path(),
        &[
            "oraculo",
            "validar",
            "--casos",
            inexistente.to_str().expect("ruta UTF-8"),
        ],
    );
    assert_eq!(
        salida.status.code(),
        Some(2),
        "una carpeta de casos inexistente es error de uso"
    );
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(
        stderr.contains("no existe la carpeta de casos"),
        "mensaje de uso: {stderr}"
    );
}

/// Un repo con `README.md` de `readme` y una sola fuente: deja el manifiesto listo. Devuelve su ruta.
fn manifiesto_de_un_repo(dir: &Path, readme: &str) -> PathBuf {
    let repo = dir.join("repo");
    fs::create_dir_all(repo.join("src")).expect("repo");
    fs::write(repo.join("src/lib.rs"), "pub fn f() {}\n").expect("lib");
    fs::write(repo.join("README.md"), readme).expect("readme");
    let manifiesto = dir.join("fuentes.toml");
    fs::write(
        &manifiesto,
        format!(
            "[[fuente]]\ntarea = \"memora\"\norigen = \"ecc-sonnet\"\nrepo = \"{}\"\n",
            repo.display()
        ),
    )
    .expect("manifiesto");
    manifiesto
}

#[test]
fn validez_cli_preparar_ciego_resume_los_rastros_por_paquete() {
    let home = TempDir::new().expect("home");
    let dir = TempDir::new().expect("dir");
    let manifiesto = manifiesto_de_un_repo(dir.path(), "Hecho con opus2\n");
    let casos = dir.path().join("casos");

    let salida = r00t(
        home.path(),
        &[
            "oraculo",
            "validar",
            "--preparar-ciego",
            "--fuentes",
            manifiesto.to_str().expect("ruta UTF-8"),
            "--casos",
            casos.to_str().expect("ruta UTF-8"),
            "--semilla",
            "7",
        ],
    );
    assert!(
        salida.status.success(),
        "sale con 0: {}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let stdout = String::from_utf8_lossy(&salida.stdout);
    assert!(
        !stdout.contains("apariciones"),
        "la salida no cuenta rastros: {stdout}"
    );
    assert!(
        !stdout.lines().any(|l| l.starts_with("rastros ")),
        "ningún conteo por paquete en la salida: {stdout}"
    );
    assert!(
        stdout.contains("rastros.json"),
        "la salida dice dónde quedó la lista: {stdout}"
    );

    let texto = fs::read_to_string(home.path().join("validez").join("rastros.json"))
        .expect("rastros.json en ROOT_HOME/validez");
    let lista: Value = serde_json::from_str(&texto).expect("rastros.json es JSON");
    let entradas = lista.as_array().expect("una lista de paquetes");
    assert_eq!(entradas.len(), 1, "un paquete, una entrada: {texto}");
    assert_eq!(
        entradas[0]["apariciones"].as_u64(),
        Some(1),
        "cuenta una aparición de opus2: {texto}"
    );
    let archivo = entradas[0]["archivo"].as_str().expect("archivo");
    let lista_del_paquete = fs::read_to_string(home.path().join("validez").join(archivo))
        .expect("la lista del paquete está en ROOT_HOME/validez");
    assert!(
        lista_del_paquete.contains("palabra = \"opus\""),
        "la lista dice qué palabra apareció: {lista_del_paquete}"
    );
    let en_ciego: Vec<String> = fs::read_dir(casos.join("ciego"))
        .expect("carpeta de paquetes")
        .map(|e| {
            e.expect("entrada")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert!(
        en_ciego.iter().all(|n| !n.starts_with("rastros-")),
        "casos/ciego no tiene listas de rastros: {en_ciego:?}"
    );
}

#[test]
fn validez_cli_json_no_lleva_rastros() {
    let home = TempDir::new().expect("home");
    let dir = TempDir::new().expect("dir");
    let manifiesto = manifiesto_de_un_repo(dir.path(), "Hecho con ECC\n");
    let casos = dir.path().join("casos");

    let salida = r00t(
        home.path(),
        &[
            "oraculo",
            "validar",
            "--json",
            "--preparar-ciego",
            "--fuentes",
            manifiesto.to_str().expect("ruta UTF-8"),
            "--casos",
            casos.to_str().expect("ruta UTF-8"),
            "--semilla",
            "7",
        ],
    );
    assert!(
        salida.status.success(),
        "sale con 0: {}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let v: Value = serde_json::from_slice(&salida.stdout).expect("--json imprime un objeto");
    assert_eq!(v["paquetes"].as_array().map(Vec::len), Some(1));
    assert!(v.get("rastros").is_none(), "--json no lleva rastros: {v}");
    assert!(
        home.path().join("validez").join("rastros.json").is_file(),
        "los rastros igual quedan en ROOT_HOME/validez"
    );
}
