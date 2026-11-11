//! Veredictos de los casos limpios que comparten las dimensiones con juez (Tarea 63, punto 6). Un limpio es una
//! copia idéntica del repo bueno. Estructura, Arquitectura y Diseño la juzgan igual, así que cada una la
//! evaluaría de nuevo. Este caché guarda el veredicto una vez y las otras dimensiones lo reusan.
//!
//! La clave junta la tarea, la versión del repo bueno, las rúbricas, el modelo del juez de diseño, el prompt
//! del juez, el `tarea.toml` y la semilla. Cambiar cualquiera cambia la clave, y entonces nada se reusa.
//! Un puntaje no medido en su calibración (`NO_MEDIDA`) no se reusa. Tampoco se reusa una falla del modelo.
//! Un archivo ilegible o corrupto cuenta como ausente. No se borra nada.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::goal::calcular_hash;
use crate::oracle::juez;
use crate::oracle::rubrica::cargar_embebidas;
use crate::oracle::tarea::Tarea;
use crate::oracle::{ErrorOraculo, NO_MEDIDA, infra};
use crate::tipos::{Dimension, Veredicto};

/// Carpeta dentro de `calibracion/` con los veredictos de los limpios.
const CARPETA: &str = "limpios";
/// Modelo del juez de diseño cuando no hay elección: sin `root.toml`, sin ruta válida, o sin modelo.
const SIN_JUEZ: &str = "sin_juez";
/// Carpetas del repo que no entran en la versión: el `target` de Cargo y el `.git`.
const SALTADAS: [&str; 2] = ["target", ".git"];
/// Inicio del motivo que pone `juez.rs` cuando el modelo falla. Es la red caída, no un límite del caso.
const FALLA_DEL_MODELO: &str = "el modelo falló: ";

/// Veredictos de los limpios de una tarea, con su clave. Cada caso va en `<clave>-<caso>.json`.
pub(super) struct Compartidos {
    carpeta: PathBuf,
    clave: String,
}

impl Compartidos {
    /// La clave de la tarea con sus partes reales: el repo bueno, el modelo del juez de diseño, el prompt del
    /// juez, el `tarea.toml` y la semilla. Lee todo el repo para la versión.
    pub(super) fn nuevo(
        home: &Path,
        tarea: &Tarea,
        repo_bueno: &Path,
        modelo: &str,
        semilla: u64,
    ) -> Result<Compartidos, ErrorOraculo> {
        let prompt = juez::huella_del_prompt();
        let tarea_hash = hash_del_tarea_toml(&tarea.dir)?;
        Compartidos::con_partes(
            home,
            &tarea.nombre,
            repo_bueno,
            modelo,
            &prompt,
            &tarea_hash,
            semilla,
        )
    }

    /// Como `nuevo`, con cada parte dada. La clave junta todas: cambiar una cambia la clave.
    pub(super) fn con_partes(
        home: &Path,
        nombre: &str,
        repo_bueno: &Path,
        modelo: &str,
        prompt_hash: &str,
        tarea_hash: &str,
        semilla: u64,
    ) -> Result<Compartidos, ErrorOraculo> {
        let version = version_del_repo(repo_bueno)?;
        let rubricas = hash_de_rubricas()?;
        Ok(Compartidos {
            carpeta: home.join(super::CARPETA_CASOS).join(CARPETA),
            clave: format!(
                "{}-{version}-{rubricas}-{}-{prompt_hash}-{tarea_hash}-{semilla}",
                sanear(nombre),
                calcular_hash(modelo.as_bytes()),
            ),
        })
    }

    /// El veredicto guardado del caso `caso`. `Some` solo si parsea, trae un puntaje de `dimension` y ese
    /// puntaje se midió. Un puntaje `NO_MEDIDA` (otra dimensión lo calibró) no sirve: esta dimensión evalúa.
    /// Una falla del modelo tampoco sirve: el modelo no respondió, así que el limpio no tiene veredicto real.
    pub(super) fn leer(&self, caso: u32, dimension: Dimension) -> Option<Veredicto> {
        let texto = fs::read_to_string(self.ruta(caso)).ok()?;
        let veredicto: Veredicto = serde_json::from_str(&texto).ok()?;
        let puntaje = super::puntaje_de(&veredicto, dimension)?;
        let medido = puntaje.sin_veredicto.as_deref() != Some(NO_MEDIDA);
        let fallo = puntaje
            .sin_veredicto
            .as_deref()
            .is_some_and(|motivo| motivo.starts_with(FALLA_DEL_MODELO));
        (medido && !fallo).then_some(veredicto)
    }

    /// Guarda el veredicto del caso `caso`. Escribe a un temporal de la misma carpeta y lo renombra: un lector
    /// nunca ve un archivo a medio escribir.
    pub(super) fn guardar(&self, caso: u32, veredicto: &Veredicto) -> Result<(), ErrorOraculo> {
        fs::create_dir_all(&self.carpeta).map_err(|e| infra("crear la carpeta de limpios", e))?;
        let texto = serde_json::to_string(veredicto)
            .map_err(|e| ErrorOraculo::Infra(format!("serializar un limpio: {e}")))?;
        let mut temporal = tempfile::NamedTempFile::new_in(&self.carpeta)
            .map_err(|e| infra("temporal de un limpio", e))?;
        temporal
            .write_all(texto.as_bytes())
            .map_err(|e| infra("escribir un limpio", e))?;
        temporal
            .persist(self.ruta(caso))
            .map_err(|e| infra("guardar un limpio", e.error))?;
        Ok(())
    }

    fn ruta(&self, caso: u32) -> PathBuf {
        self.carpeta.join(format!("{}-{caso}.json", self.clave))
    }
}

/// Modelo que elige Route para el juez de diseño de producción. Sin `root.toml` o sin elección, vale
/// `SIN_JUEZ`: la calibración sin juez tiene su propia clave.
pub(super) fn modelo_del_juez_diseno(home: &Path) -> String {
    if !home.join("root.toml").exists() {
        return SIN_JUEZ.into();
    }
    let mut avisos = Vec::new();
    crate::oracle::juez_diseno_de_produccion(home, &mut avisos)
        .and_then(|juez| juez.eleccion.modelo)
        .unwrap_or_else(|| SIN_JUEZ.into())
}

/// Hash del `tarea.toml` de `dir`. Cambiar la tarea cambia la clave.
fn hash_del_tarea_toml(dir: &Path) -> Result<String, ErrorOraculo> {
    let bytes = fs::read(dir.join("tarea.toml")).map_err(|e| infra("leer tarea.toml", e))?;
    Ok(calcular_hash(&bytes))
}

/// Hash de todos los archivos del repo, en orden de ruta relativa. Cada archivo entra con su ruta y su tamaño.
fn version_del_repo(repo: &Path) -> Result<String, ErrorOraculo> {
    let mut archivos = Vec::new();
    recoger(repo, repo, &mut archivos)?;
    archivos.sort();
    let mut contenido = Vec::new();
    for (relativa, ruta) in &archivos {
        let bytes = fs::read(ruta).map_err(|e| infra("leer un archivo del repo", e))?;
        contenido.extend_from_slice(relativa.as_bytes());
        contenido.push(0);
        contenido.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        contenido.extend_from_slice(&bytes);
    }
    Ok(calcular_hash(&contenido))
}

/// Junta `(ruta relativa, ruta absoluta)` de los archivos de `dir`, sin `SALTADAS`. Los enlaces simbólicos no entran.
fn recoger(
    raiz: &Path,
    dir: &Path,
    salida: &mut Vec<(String, PathBuf)>,
) -> Result<(), ErrorOraculo> {
    let entradas = fs::read_dir(dir).map_err(|e| infra("leer el repo bueno", e))?;
    for entrada in entradas {
        let entrada = entrada.map_err(|e| infra("leer el repo bueno", e))?;
        let tipo = entrada
            .file_type()
            .map_err(|e| infra("leer el repo bueno", e))?;
        let ruta = entrada.path();
        if tipo.is_dir() {
            if SALTADAS.iter().any(|s| entrada.file_name() == *s) {
                continue;
            }
            recoger(raiz, &ruta, salida)?;
        } else if tipo.is_file() {
            let relativa = ruta
                .strip_prefix(raiz)
                .map_err(|_| ErrorOraculo::Infra("ruta fuera del repo bueno".into()))?
                .to_string_lossy()
                .replace('\\', "/");
            salida.push((relativa, ruta));
        }
    }
    Ok(())
}

/// Hash de las rúbricas embebidas, en su orden. Una rúbrica cambiada cambia la clave.
fn hash_de_rubricas() -> Result<String, ErrorOraculo> {
    let rubricas =
        cargar_embebidas().map_err(|e| ErrorOraculo::Infra(format!("rúbricas embebidas: {e}")))?;
    let juntas: String = rubricas.iter().map(|r| r.hash()).collect();
    Ok(calcular_hash(juntas.as_bytes()))
}

/// La tarea como nombre de archivo: letras, dígitos, `-` y `_`. Lo demás pasa a `_`.
fn sanear(nombre: &str) -> String {
    nombre
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tipos::Puntaje;

    /// Repo temporal con un archivo. Sirve de repo bueno en las claves de prueba.
    fn repo_de_prueba() -> tempfile::TempDir {
        let repo = tempfile::tempdir().expect("repo");
        fs::write(repo.path().join("lib.rs"), "fn f() {}").expect("lib");
        repo
    }

    /// Veredicto con un puntaje de Diseño. `sin` es el `sin_veredicto` de ese puntaje.
    fn veredicto_con_diseno(sin: Option<&str>) -> Veredicto {
        Veredicto {
            puntajes: vec![Puntaje {
                motivo_sin_calibrar: None,
                dimension: Dimension::Diseno,
                valor: 1.0,
                calibrada: false,
                exactitud_balanceada: None,
                items: Vec::new(),
                sin_veredicto: sin.map(String::from),
                faltantes: Vec::new(),
            }],
            ..Veredicto::default()
        }
    }

    /// Clave con todas las partes en valores fijos. Cada test cambia una sola.
    fn clave_base(home: &Path, repo: &Path) -> Compartidos {
        Compartidos::con_partes(
            home,
            "tarea-prueba",
            repo,
            "modelo-a",
            "prompt-a",
            "tarea-a",
            42,
        )
        .expect("clave")
    }

    /// Guarda un limpio con `a` y lo lee con `b`.
    fn leer_con_otra_clave(a: &Compartidos, b: &Compartidos) -> Option<Veredicto> {
        a.guardar(1, &veredicto_con_diseno(None)).expect("guarda");
        b.leer(1, Dimension::Diseno)
    }

    #[test]
    fn leer_con_la_misma_clave_devuelve_lo_guardado() {
        let home = tempfile::tempdir().expect("home");
        let repo = repo_de_prueba();
        let a = clave_base(home.path(), repo.path());
        assert!(
            leer_con_otra_clave(&a, &a).is_some(),
            "control: misma clave"
        );
    }

    #[test]
    fn leer_rechaza_un_limpio_no_medido_en_su_calibracion() {
        let home = tempfile::tempdir().expect("home");
        let repo = repo_de_prueba();
        let a = clave_base(home.path(), repo.path());
        a.guardar(1, &veredicto_con_diseno(Some(crate::oracle::NO_MEDIDA)))
            .expect("guarda");
        assert!(
            a.leer(1, Dimension::Diseno).is_none(),
            "un puntaje no medido no se reusa"
        );
    }

    #[test]
    fn leer_reusa_un_limpio_con_sin_veredicto_por_cobertura() {
        let home = tempfile::tempdir().expect("home");
        let repo = repo_de_prueba();
        let a = clave_base(home.path(), repo.path());
        a.guardar(
            1,
            &veredicto_con_diseno(Some("cobertura 8 de 13 de peso, menor que dos tercios")),
        )
        .expect("guarda");
        assert!(
            a.leer(1, Dimension::Diseno).is_some(),
            "un sin veredicto por cobertura sí se reusa"
        );
    }

    #[test]
    fn leer_rechaza_un_limpio_con_sin_veredicto_por_falla_del_modelo() {
        let home = tempfile::tempdir().expect("home");
        let repo = repo_de_prueba();
        let a = clave_base(home.path(), repo.path());
        a.guardar(
            1,
            &veredicto_con_diseno(Some("el modelo falló: conexión caída")),
        )
        .expect("guarda");
        assert!(
            a.leer(1, Dimension::Diseno).is_none(),
            "un sin veredicto por falla del modelo no se reusa"
        );
    }

    #[test]
    fn clave_distinta_por_modelo_no_reusa() {
        let home = tempfile::tempdir().expect("home");
        let repo = repo_de_prueba();
        let a = clave_base(home.path(), repo.path());
        let b = Compartidos::con_partes(
            home.path(),
            "tarea-prueba",
            repo.path(),
            "modelo-b",
            "prompt-a",
            "tarea-a",
            42,
        )
        .expect("clave");
        assert!(
            leer_con_otra_clave(&a, &b).is_none(),
            "otro modelo, otra clave"
        );
    }

    #[test]
    fn clave_distinta_por_prompt_no_reusa() {
        let home = tempfile::tempdir().expect("home");
        let repo = repo_de_prueba();
        let a = clave_base(home.path(), repo.path());
        let b = Compartidos::con_partes(
            home.path(),
            "tarea-prueba",
            repo.path(),
            "modelo-a",
            "prompt-b",
            "tarea-a",
            42,
        )
        .expect("clave");
        assert!(
            leer_con_otra_clave(&a, &b).is_none(),
            "otro prompt, otra clave"
        );
    }

    #[test]
    fn clave_distinta_por_tarea_toml_no_reusa() {
        let home = tempfile::tempdir().expect("home");
        let repo = repo_de_prueba();
        let a = clave_base(home.path(), repo.path());
        let b = Compartidos::con_partes(
            home.path(),
            "tarea-prueba",
            repo.path(),
            "modelo-a",
            "prompt-a",
            "tarea-b",
            42,
        )
        .expect("clave");
        assert!(
            leer_con_otra_clave(&a, &b).is_none(),
            "otro tarea.toml, otra clave"
        );
    }

    #[test]
    fn clave_distinta_por_semilla_no_reusa() {
        let home = tempfile::tempdir().expect("home");
        let repo = repo_de_prueba();
        let a = clave_base(home.path(), repo.path());
        let b = Compartidos::con_partes(
            home.path(),
            "tarea-prueba",
            repo.path(),
            "modelo-a",
            "prompt-a",
            "tarea-a",
            7,
        )
        .expect("clave");
        assert!(
            leer_con_otra_clave(&a, &b).is_none(),
            "otra semilla, otra clave"
        );
    }

    #[test]
    fn modelo_sin_root_toml_es_sin_juez() {
        let home = tempfile::tempdir().expect("home");
        assert_eq!(modelo_del_juez_diseno(home.path()), "sin_juez");
    }

    #[test]
    fn version_del_repo_ignora_target_y_git() {
        let repo = tempfile::tempdir().expect("repo");
        fs::write(repo.path().join("lib.rs"), "fn f() {}").expect("lib");
        let antes = version_del_repo(repo.path()).expect("versión");
        for carpeta in ["target", ".git"] {
            fs::create_dir_all(repo.path().join(carpeta)).expect("carpeta");
            fs::write(repo.path().join(carpeta).join("x"), "otro").expect("archivo");
        }
        assert_eq!(version_del_repo(repo.path()).expect("versión"), antes);
    }

    #[test]
    fn version_del_repo_cambia_con_el_contenido_y_con_la_ruta() {
        let repo = tempfile::tempdir().expect("repo");
        fs::write(repo.path().join("lib.rs"), "fn f() {}").expect("lib");
        let antes = version_del_repo(repo.path()).expect("versión");
        fs::write(repo.path().join("lib.rs"), "fn g() {}").expect("cambia");
        let contenido = version_del_repo(repo.path()).expect("versión");
        assert_ne!(contenido, antes, "cambiar el contenido cambia la versión");
        fs::rename(repo.path().join("lib.rs"), repo.path().join("otro.rs")).expect("renombra");
        assert_ne!(
            version_del_repo(repo.path()).expect("versión"),
            contenido,
            "cambiar la ruta cambia la versión"
        );
    }
}
