//! Carga la definición de una tarea del Oracle desde su `tarea.toml`.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::ErrorOraculo;
use crate::config::root_home;

/// Definición de una tarea. Los campos de etapas futuras se ignoran al parsear.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Tarea {
    pub nombre: String,
    pub lenguaje: String,
    pub imagen: String,
    pub containerfile: Option<String>,
    pub spec: String,
    pub contrato: Comando,
    pub oculto: Comando,
    /// Carpeta de la tarea. Se monta como `/oraculo`. No viene del TOML.
    #[serde(skip)]
    pub dir: PathBuf,
}

/// Un comando que corre dentro del contenedor con `sh -c`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Comando {
    pub cmd: String,
}

impl Tarea {
    /// Lee `<dir>/tarea.toml`.
    pub fn cargar_de(dir: &Path) -> Result<Tarea, ErrorOraculo> {
        let ruta = dir.join("tarea.toml");
        let texto = fs::read_to_string(&ruta)
            .map_err(|e| ErrorOraculo::Tarea(format!("no se pudo leer {}: {e}", ruta.display())))?;
        let mut tarea: Tarea = toml::from_str(&texto)
            .map_err(|e| ErrorOraculo::Tarea(format!("{}: {e}", ruta.display())))?;
        tarea.dir = dir.to_path_buf();
        Ok(tarea)
    }

    /// Lee la tarea `nombre` de `<root_home()>/oraculos/<nombre>/`.
    pub fn cargar(nombre: &str) -> Result<Tarea, ErrorOraculo> {
        cargar_en(&root_home(), nombre)
    }
}

/// Carga la tarea desde una base dada. El nombre no puede traer rutas.
fn cargar_en(home: &Path, nombre: &str) -> Result<Tarea, ErrorOraculo> {
    let con_ruta =
        nombre.is_empty() || nombre.contains(['/', '\\']) || nombre == "." || nombre == "..";
    if con_ruta {
        return Err(ErrorOraculo::Tarea(format!(
            "nombre de tarea inválido: {nombre:?}"
        )));
    }
    Tarea::cargar_de(&home.join("oraculos").join(nombre))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const MINIMO: &str = r#"
nombre = "memora"
lenguaje = "rust"
imagen = "root-oraculo-rust:1"
spec = "bootstrap/docs/SPEC.md"

[contrato]
cmd = "pytest -q"

[oculto]
cmd = "pytest -q /oraculo/oculto"
"#;

    /// `containerfile` es campo de la raíz: va antes de cualquier tabla.
    const CONTAINERFILE: &str = "containerfile = \"bootstrap/Dockerfile\"\n";

    const ETAPAS_FUTURAS: &str = r#"
[[requisito]]
id = "R1"
titulo = "Protocolo RESP2"

[[escenario]]
id = "e2e-set-get"
cmd = "/oraculo/escenarios/set_get.sh"

[[sonda]]
nombre = "gitleaks"

[entrega]
instalar = "cargo build --release"

[pruebas]
mutantes_max = 60

[eficiencia]
cmd = "python /oraculo/bench.py /repo"
piso = 5000.0

[arquitectura]
capas = [["protocolo"], ["comandos"]]
"#;

    fn escribir(dir: &Path, texto: &str) {
        fs::write(dir.join("tarea.toml"), texto).expect("escribe tarea.toml");
    }

    #[test]
    fn tarea_toml_minimo_parsea() {
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(dir.path(), MINIMO);
        let t = Tarea::cargar_de(dir.path()).expect("parsea");
        assert_eq!(t.nombre, "memora");
        assert_eq!(t.lenguaje, "rust");
        assert_eq!(t.imagen, "root-oraculo-rust:1");
        assert_eq!(t.containerfile, None);
        assert_eq!(t.spec, "bootstrap/docs/SPEC.md");
        assert_eq!(t.contrato.cmd, "pytest -q");
        assert_eq!(t.oculto.cmd, "pytest -q /oraculo/oculto");
        assert_eq!(t.dir, dir.path());
    }

    #[test]
    fn tarea_secciones_de_etapas_futuras_se_ignoran() {
        let dir = tempfile::tempdir().expect("temp dir");
        let texto = format!("{CONTAINERFILE}{MINIMO}{ETAPAS_FUTURAS}");
        escribir(dir.path(), &texto);
        let t = Tarea::cargar_de(dir.path()).expect("parsea con secciones extra");
        assert_eq!(t.contrato.cmd, "pytest -q");
        assert_eq!(t.containerfile.as_deref(), Some("bootstrap/Dockerfile"));
    }

    #[test]
    fn tarea_falta_un_campo_obligatorio_da_error() {
        let dir = tempfile::tempdir().expect("temp dir");
        let sin_spec = MINIMO.replace("spec = \"bootstrap/docs/SPEC.md\"\n", "");
        escribir(dir.path(), &sin_spec);
        assert!(matches!(
            Tarea::cargar_de(dir.path()),
            Err(ErrorOraculo::Tarea(_))
        ));
    }

    #[test]
    fn tarea_toml_invalido_da_error() {
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(dir.path(), "nombre = ");
        assert!(matches!(
            Tarea::cargar_de(dir.path()),
            Err(ErrorOraculo::Tarea(_))
        ));
    }

    #[test]
    fn tarea_inexistente_da_error() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert!(matches!(
            Tarea::cargar_de(&dir.path().join("no-existe")),
            Err(ErrorOraculo::Tarea(_))
        ));
    }

    #[test]
    fn tarea_cargar_respeta_root_home() {
        let home = tempfile::tempdir().expect("temp dir");
        let dir = home.path().join("oraculos").join("memora");
        fs::create_dir_all(&dir).expect("crea la carpeta");
        escribir(&dir, MINIMO);
        let t = cargar_en(home.path(), "memora").expect("carga desde ROOT_HOME");
        assert_eq!(t.dir, dir);
    }

    #[test]
    fn tarea_nombre_con_ruta_se_rechaza() {
        let home = tempfile::tempdir().expect("temp dir");
        for nombre in ["", ".", "..", "../otra", "a/b", "a\\b"] {
            assert!(
                matches!(cargar_en(home.path(), nombre), Err(ErrorOraculo::Tarea(_))),
                "debe rechazar {nombre:?}"
            );
        }
    }
}
