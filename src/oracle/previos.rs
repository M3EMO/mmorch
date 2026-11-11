//! Línea base de Seguridad (Tarea 103). Un hallazgo de `bootstrap/` es previo: ya estaba en el estado
//! inicial de la tarea. Solo bloquea lo que el cambio introduce.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::oracle::historial;
use crate::oracle::pendientes;
use crate::oracle::perfiles;
use crate::oracle::perfiles::herramientas::{EjecutorHerramienta, Herramienta, sondas_de};
use crate::oracle::tarea::Tarea;
use crate::tipos::{Dimension, Fuente, Hallazgo};

/// Sube cuando cambia una sonda de Seguridad. Cambia la clave de la línea base guardada.
pub const VERSION_SONDAS_SEGURIDAD: u32 = 1;

/// Aviso del veredicto cuando la tarea no trae `bootstrap/`.
pub const AVISO_SIN_ESTADO_INICIAL: &str =
    "sin estado inicial: todo hallazgo de Seguridad cuenta como nuevo";

/// Hallazgos de Seguridad del estado inicial, por sonda. La clave es el id del hallazgo.
/// El valor son las líneas donde aparece. Su largo es cuántas veces aparece.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LineaBase {
    #[serde(default)]
    pub cargo_audit: BTreeMap<String, Vec<u32>>,
    #[serde(default)]
    pub gitleaks: BTreeMap<String, Vec<u32>>,
    #[serde(default)]
    pub nativas: BTreeMap<String, Vec<u32>>,
}

/// Línea base de `bootstrap/`. Sale de la memoria de `home` si la clave coincide. Si no, corre las sondas
/// sobre `bootstrap/` y guarda el resultado. Solo se guarda si todas las sondas dieron veredicto.
pub fn linea_base(
    home: &Path,
    tarea: &Tarea,
    bootstrap: &Path,
    ej: &dyn EjecutorHerramienta,
) -> LineaBase {
    let clave = clave_de(tarea, bootstrap, ej);
    let ruta = clave
        .as_ref()
        .filter(|_| pendientes::nombre_de_tarea_valido(&tarea.nombre).is_ok())
        .map(|c| {
            home.join(CARPETA)
                .join(&tarea.nombre)
                .join(format!("{c}.json"))
        });
    if let Some(guardada) = ruta.as_deref().and_then(leer) {
        return guardada;
    }
    let (base, completa) = calcular(tarea, bootstrap, ej);
    if let Some(ruta) = ruta.filter(|_| completa) {
        // Si la escritura falla, la línea base vale igual: solo no queda en memoria.
        let _ = guardar(&ruta, &base);
    }
    base
}

/// Carpeta de `home` donde van las líneas base guardadas.
const CARPETA: &str = "previos";

/// Clave de la memoria: el hash de `bootstrap/`, la versión de las sondas, la imagen y si hay RustSec.
/// `None` si `bootstrap/` no se puede leer: entonces no hay memoria y la línea base se calcula.
fn clave_de(tarea: &Tarea, bootstrap: &Path, ej: &dyn EjecutorHerramienta) -> Option<String> {
    let version = historial::hash_de_version(bootstrap).ok()?;
    let mut digest = Sha256::new();
    for parte in [
        version,
        VERSION_SONDAS_SEGURIDAD.to_string(),
        tarea.imagen.clone(),
        ej.tiene_rustsec().to_string(),
    ] {
        digest.update(parte.as_bytes());
        digest.update([0]);
    }
    Some(
        digest
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
    )
}

/// Corre las sondas de Seguridad sobre `bootstrap/`. Devuelve la línea base y si todas dieron veredicto.
fn calcular(tarea: &Tarea, bootstrap: &Path, ej: &dyn EjecutorHerramienta) -> (LineaBase, bool) {
    let mut base = LineaBase::default();
    let herramientas = sondas_de(
        ej,
        tarea,
        bootstrap,
        &[Herramienta::Gitleaks, Herramienta::CargoAudit],
    );
    let mut completa = herramientas.iter().all(|s| s.valor.is_some());
    for sonda in &herramientas {
        for h in &sonda.hallazgos {
            let mapa = match &h.fuente {
                Fuente::Sonda(nombre) if nombre == "gitleaks" => &mut base.gitleaks,
                Fuente::Sonda(nombre) if nombre == "cargo-audit" => &mut base.cargo_audit,
                _ => continue,
            };
            mapa.entry(h.id.clone()).or_default().push(h.linea);
        }
    }
    let nativas = perfiles::sondas_nativas(bootstrap, tarea);
    completa &= nativas
        .iter()
        .filter(|s| s.dimension == Dimension::Seguridad)
        .all(|s| s.valor.is_some());
    for sonda in nativas
        .iter()
        .filter(|s| s.dimension == Dimension::Seguridad)
    {
        for h in &sonda.hallazgos {
            base.nativas.entry(h.id.clone()).or_default().push(h.linea);
        }
    }
    (base, completa)
}

fn leer(ruta: &Path) -> Option<LineaBase> {
    let texto = fs::read_to_string(ruta).ok()?;
    serde_json::from_str(&texto).ok()
}

fn guardar(ruta: &Path, base: &LineaBase) -> std::io::Result<()> {
    if let Some(carpeta) = ruta.parent() {
        fs::create_dir_all(carpeta)?;
    }
    let texto = serde_json::to_string(base).map_err(std::io::Error::other)?;
    fs::write(ruta, texto)
}

/// Marca `previo` en cada hallazgo que estaba en la base. Un id cuenta tantas veces como aparece en la base.
/// Con `cambios` (nativas), solo cuentan los hallazgos fuera de los rangos cambiados. Los primeros que
/// alcanzan para la base quedan previos. El resto es nuevo, también el que cae dentro del cambio.
pub fn marcar(
    hallazgos: &mut [Hallazgo],
    base: &BTreeMap<String, Vec<u32>>,
    cambios: Option<&BTreeMap<String, Vec<(u32, u32)>>>,
) {
    let mut grupos: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, h) in hallazgos.iter().enumerate() {
        grupos.entry(h.id.clone()).or_default().push(i);
    }
    for h in hallazgos.iter_mut() {
        h.previo = false;
    }
    for (id, indices) in grupos {
        let Some(lineas) = base.get(&id) else {
            continue;
        };
        let candidatos: Vec<usize> = indices
            .into_iter()
            .filter(|&i| !en_cambio(&hallazgos[i], cambios))
            .collect();
        for &i in candidatos.iter().take(lineas.len()) {
            hallazgos[i].previo = true;
        }
    }
}

/// Verdadero si la línea del hallazgo cae en un rango cambiado de su archivo.
fn en_cambio(h: &Hallazgo, cambios: Option<&BTreeMap<String, Vec<(u32, u32)>>>) -> bool {
    cambios
        .and_then(|c| c.get(&h.archivo))
        .is_some_and(|rangos| {
            rangos
                .iter()
                .any(|&(ini, fin)| (ini..=fin).contains(&h.linea))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oracle::ErrorOraculo;
    use crate::oracle::perfiles::crear_hallazgo;
    use crate::oracle::perfiles::herramientas::{
        COMANDO_CARGO_AUDIT, COMANDO_GITLEAKS, SalidaHerramienta,
    };
    use crate::oracle::tarea::Tarea;
    use crate::tipos::Dimension;
    use std::cell::RefCell;
    use std::fs;

    const CARGO_AUDIT: &str = include_str!("../../tests/fixtures/sondas/cargo-audit.json");
    const CARGO_LOCK: &str = include_str!("../../tests/fixtures/sondas/cargo-lock.txt");
    const GITLEAKS: &str = include_str!("../../tests/fixtures/sondas/gitleaks.json");

    /// Ejecutor de mentira que cuenta los comandos que recibe.
    struct EjecutorContado {
        cmds: RefCell<Vec<String>>,
    }

    impl EjecutorContado {
        fn nuevo() -> Self {
            EjecutorContado {
                cmds: RefCell::new(Vec::new()),
            }
        }

        fn cuantos(&self) -> usize {
            self.cmds.borrow().len()
        }
    }

    impl EjecutorHerramienta for EjecutorContado {
        fn ejecutar(&self, cmd: &str) -> Result<SalidaHerramienta, ErrorOraculo> {
            self.cmds.borrow_mut().push(cmd.to_string());
            let mut salida = SalidaHerramienta::default();
            if cmd == COMANDO_CARGO_AUDIT {
                salida
                    .archivos
                    .insert("audit.json".into(), CARGO_AUDIT.into());
            } else if cmd == COMANDO_GITLEAKS {
                salida
                    .archivos
                    .insert("gitleaks.json".into(), GITLEAKS.into());
                salida.stderr = "no leaks found".into();
            }
            Ok(salida)
        }

        fn tiene_rustsec(&self) -> bool {
            true
        }
    }

    fn cargo_audit_de(archivo: &str, linea: u32, cita: &str) -> Hallazgo {
        crear_hallazgo(
            Dimension::Seguridad,
            3,
            archivo,
            linea,
            cita,
            "vulnerable",
            "cargo-audit",
            &[],
        )
    }

    fn nativo_de(archivo: &str, linea: u32, cita: &str) -> Hallazgo {
        crear_hallazgo(
            Dimension::Seguridad,
            2,
            archivo,
            linea,
            cita,
            "unsafe fuera de tests",
            "seguridad-nativa",
            &[],
        )
    }

    /// Mapa de la línea base que dan estos hallazgos del estado inicial.
    fn base_de(hs: &[Hallazgo]) -> BTreeMap<String, Vec<u32>> {
        let mut base: BTreeMap<String, Vec<u32>> = BTreeMap::new();
        for h in hs {
            base.entry(h.id.clone()).or_default().push(h.linea);
        }
        base
    }

    fn cambio_en(archivo: &str, rangos: &[(u32, u32)]) -> BTreeMap<String, Vec<(u32, u32)>> {
        BTreeMap::from([(archivo.to_string(), rangos.to_vec())])
    }

    fn tarea_rust() -> (tempfile::TempDir, Tarea) {
        let dir = tempfile::tempdir().expect("temp dir");
        let texto = "nombre = \"x\"\nlenguaje = \"rust\"\nimagen = \"i\"\nspec = \"s\"\n\
                     [contrato]\ncmd = \"c\"\n[oculto]\ncmd = \"o\"\n";
        fs::write(dir.path().join("tarea.toml"), texto).expect("escribe tarea.toml");
        let tarea = Tarea::cargar_de(dir.path()).expect("carga la tarea");
        (dir, tarea)
    }

    #[test]
    fn previos_mismo_aviso_de_cargo_audit_en_la_base_queda_previo() {
        let base = base_de(&[cargo_audit_de("Cargo.lock", 16, "time 0.1.43")]);
        let mut repo = [cargo_audit_de("Cargo.lock", 16, "time 0.1.43")];
        marcar(&mut repo, &base, None);
        assert!(repo[0].previo);
    }

    #[test]
    fn previos_otra_version_del_mismo_crate_queda_nuevo() {
        // La cita lleva la versión: otra versión es otro id, y el id no está en la base.
        let base = base_de(&[cargo_audit_de("Cargo.lock", 16, "time 0.1.43")]);
        let mut repo = [cargo_audit_de("Cargo.lock", 16, "time 0.1.44")];
        marcar(&mut repo, &base, None);
        assert!(!repo[0].previo);
    }

    #[test]
    fn previos_crate_vulnerable_que_no_estaba_en_la_base_queda_nuevo() {
        let base = base_de(&[cargo_audit_de("Cargo.lock", 16, "time 0.1.43")]);
        let mut repo = [
            cargo_audit_de("Cargo.lock", 16, "time 0.1.43"),
            cargo_audit_de("Cargo.lock", 40, "foo 1.0.0"),
        ];
        marcar(&mut repo, &base, None);
        assert!(repo[0].previo, "el crate de la base sigue previo");
        assert!(!repo[1].previo, "el crate nuevo no estaba en la base");
    }

    #[test]
    fn previos_nativo_con_id_de_la_base_en_linea_del_cambio_queda_nuevo() {
        let base = base_de(&[nativo_de("src/x.rs", 10, "pub unsafe fn f() {}")]);
        let cambios = cambio_en("src/x.rs", &[(11, 15)]);
        let mut dentro = [nativo_de("src/x.rs", 12, "pub unsafe fn f() {}")];
        marcar(&mut dentro, &base, Some(&cambios));
        assert!(!dentro[0].previo, "la línea 12 cae en el cambio");
        // Control: la misma cita fuera del cambio sí es previo.
        let mut fuera = [nativo_de("src/x.rs", 3, "pub unsafe fn f() {}")];
        marcar(&mut fuera, &base, Some(&cambios));
        assert!(fuera[0].previo, "la línea 3 está fuera del cambio");
    }

    #[test]
    fn previos_id_dos_veces_en_el_repo_y_una_en_la_base_queda_nuevo_el_del_cambio() {
        let base = base_de(&[nativo_de("src/x.rs", 5, "pub unsafe fn f() {}")]);
        let cambios = cambio_en("src/x.rs", &[(18, 25)]);
        let mut repo = [
            nativo_de("src/x.rs", 5, "pub unsafe fn f() {}"),
            nativo_de("src/x.rs", 20, "pub unsafe fn f() {}"),
        ];
        marcar(&mut repo, &base, Some(&cambios));
        assert!(repo[0].previo, "la línea 5 es la de la base");
        assert!(!repo[1].previo, "la línea 20 cae en el cambio: es nueva");
    }

    #[test]
    fn previos_linea_base_guardada_no_vuelve_a_correr_sondas() {
        let home = tempfile::tempdir().expect("temp dir");
        let bootstrap = tempfile::tempdir().expect("temp dir");
        fs::write(bootstrap.path().join("Cargo.lock"), CARGO_LOCK).expect("escribe el lock");
        fs::create_dir_all(bootstrap.path().join("src")).expect("crea src");
        fs::write(
            bootstrap.path().join("src/lib.rs"),
            "pub unsafe fn f() {}\n",
        )
        .expect("escribe el fuente");
        let (_tarea_dir, tarea) = tarea_rust();
        let ej = EjecutorContado::nuevo();

        let primera = linea_base(home.path(), &tarea, bootstrap.path(), &ej);
        let corridos = ej.cuantos();
        assert!(corridos > 0, "la primera llamada corre sondas");
        assert!(!primera.cargo_audit.is_empty(), "{primera:?}");
        assert!(!primera.nativas.is_empty(), "{primera:?}");

        let segunda = linea_base(home.path(), &tarea, bootstrap.path(), &ej);
        assert_eq!(ej.cuantos(), corridos, "la segunda no corre sondas");
        assert_eq!(primera, segunda);
    }
}
