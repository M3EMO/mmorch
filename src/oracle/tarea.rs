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
    /// Veces que corre el oculto. Cada repetición usa un contenedor nuevo.
    #[serde(default = "repeticiones_por_defecto")]
    pub repeticiones: u32,
    pub contrato: Comando,
    pub oculto: Comando,
    /// Tests cuya diferencia viene del entorno (por ejemplo, lavapipe contra GPU).
    #[serde(default)]
    pub entorno: Vec<Exclusion>,
    /// Tests que dependen de una frase ambigua de la spec.
    #[serde(default)]
    pub oraculo_dudoso: Vec<Exclusion>,
    /// Carpeta de la tarea. Se monta como `/oraculo`. No viene del TOML.
    #[serde(skip)]
    pub dir: PathBuf,
}

/// Un comando que corre dentro del contenedor con `sh -c`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Comando {
    pub cmd: String,
}

/// Un test excluido del cálculo, con su motivo. `test` tiene la forma `archivo::nombre`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Exclusion {
    pub test: String,
    pub motivo: String,
}

fn repeticiones_por_defecto() -> u32 {
    1
}

impl Tarea {
    /// Lee `<dir>/tarea.toml`.
    pub fn cargar_de(dir: &Path) -> Result<Tarea, ErrorOraculo> {
        let ruta = dir.join("tarea.toml");
        let texto = fs::read_to_string(&ruta)
            .map_err(|e| ErrorOraculo::Tarea(format!("no se pudo leer {}: {e}", ruta.display())))?;
        let mut tarea: Tarea = toml::from_str(&texto)
            .map_err(|e| ErrorOraculo::Tarea(format!("{}: {e}", ruta.display())))?;
        tarea
            .validar()
            .map_err(|e| ErrorOraculo::Tarea(format!("{}: {e}", ruta.display())))?;
        tarea.dir = dir.to_path_buf();
        Ok(tarea)
    }

    /// Repeticiones de 1 o más. Cada test excluido tiene un motivo y aparece una sola vez.
    fn validar(&self) -> Result<(), String> {
        if self.repeticiones == 0 {
            return Err("repeticiones debe ser 1 o más".into());
        }
        let mut vistos = std::collections::BTreeSet::new();
        for exclusion in self.entorno.iter().chain(&self.oraculo_dudoso) {
            if exclusion.motivo.trim().is_empty() {
                return Err(format!("exclusión de {} sin motivo", exclusion.test));
            }
            if !vistos.insert(exclusion.test.as_str()) {
                return Err(format!(
                    "el test {} está excluido dos veces",
                    exclusion.test
                ));
            }
        }
        Ok(())
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
    fn repeticiones_por_defecto_es_1() {
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(dir.path(), MINIMO);
        let t = Tarea::cargar_de(dir.path()).expect("parsea");
        assert_eq!(t.repeticiones, 1);
        assert!(t.entorno.is_empty());
        assert!(t.oraculo_dudoso.is_empty());
    }

    #[test]
    fn repeticiones_leen_exclusiones_de_entorno_y_oraculo() {
        let dir = tempfile::tempdir().expect("temp dir");
        let texto = MINIMO.replace("[contrato]", "repeticiones = 3\n\n[contrato]")
            + "\n[[entorno]]\ntest = \"a::b\"\nmotivo = \"lavapipe\"\n\
               \n[[oraculo_dudoso]]\ntest = \"c::d\"\nmotivo = \"orden de spin\"\n";
        escribir(dir.path(), &texto);
        let t = Tarea::cargar_de(dir.path()).expect("parsea");
        assert_eq!(t.repeticiones, 3);
        assert_eq!(t.entorno[0].test, "a::b");
        assert_eq!(t.oraculo_dudoso[0].motivo, "orden de spin");
    }

    #[test]
    fn repeticiones_cero_es_error_de_carga() {
        let dir = tempfile::tempdir().expect("temp dir");
        escribir(
            dir.path(),
            &MINIMO.replace("[contrato]", "repeticiones = 0\n\n[contrato]"),
        );
        assert!(matches!(
            Tarea::cargar_de(dir.path()),
            Err(ErrorOraculo::Tarea(_))
        ));
    }

    #[test]
    fn exclusion_sin_motivo_es_error_de_carga() {
        let dir = tempfile::tempdir().expect("temp dir");
        let texto = format!("{MINIMO}\n[[entorno]]\ntest = \"a::b\"\nmotivo = \"  \"\n");
        escribir(dir.path(), &texto);
        assert!(matches!(
            Tarea::cargar_de(dir.path()),
            Err(ErrorOraculo::Tarea(_))
        ));
    }

    #[test]
    fn exclusion_del_mismo_test_dos_veces_es_error_de_carga() {
        let dir = tempfile::tempdir().expect("temp dir");
        let texto = format!(
            "{MINIMO}\n[[entorno]]\ntest = \"a::b\"\nmotivo = \"x\"\n\
             \n[[oraculo_dudoso]]\ntest = \"a::b\"\nmotivo = \"y\"\n"
        );
        escribir(dir.path(), &texto);
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
