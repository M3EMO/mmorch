//! Línea base de Seguridad (Tarea 103). Un hallazgo de `bootstrap/` es previo: ya estaba en el estado
//! inicial de la tarea. Solo bloquea lo que el cambio introduce.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::oracle::configs;
use crate::oracle::historial;
use crate::oracle::pendientes;
use crate::oracle::perfiles;
use crate::oracle::perfiles::herramientas::{EjecutorHerramienta, Herramienta, sondas_de};
use crate::oracle::perfiles::{ResultadoSonda, puntuar_hallazgos};
use crate::oracle::tarea::{Requisito, Tarea};
use crate::tipos::{Dimension, Fuente, Hallazgo};

/// Sube cuando cambia una sonda de Seguridad. Cambia la clave de la línea base guardada.
pub const VERSION_SONDAS_SEGURIDAD: u32 = 3;

/// Aviso del veredicto cuando la tarea no trae `bootstrap/`.
pub const AVISO_SIN_ESTADO_INICIAL: &str =
    "sin estado inicial: todo hallazgo de Seguridad cuenta como nuevo";

/// Hallazgos de Seguridad del estado inicial, por sonda. La clave es `clave_base` del hallazgo.
/// El valor son las líneas donde aparece. Su largo es cuántas veces aparece.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LineaBase {
    #[serde(default)]
    pub cargo_audit: BTreeMap<String, Vec<u32>>,
    #[serde(default)]
    pub gitleaks: BTreeMap<String, Vec<u32>>,
    #[serde(default)]
    pub nativas: BTreeMap<String, Vec<u32>>,
    /// Hallazgos de Seguridad de las sondas de configs (secretos y reglas de Dockerfile).
    #[serde(default)]
    pub configs: BTreeMap<String, Vec<u32>>,
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

/// Clave de la memoria: el hash de `bootstrap/`, la versión de las sondas, la imagen, el lenguaje y la base de RustSec.
/// La base suma su huella de contenido. Sin huella, suma solo si va montada.
/// `None` si `bootstrap/` no se puede leer: entonces no hay memoria y la línea base se calcula.
fn clave_de(tarea: &Tarea, bootstrap: &Path, ej: &dyn EjecutorHerramienta) -> Option<String> {
    clave_de_version(tarea, bootstrap, ej, VERSION_SONDAS_SEGURIDAD)
}

/// Igual que `clave_de`, con la versión de las sondas dada. Los tests usan una versión vieja.
fn clave_de_version(
    tarea: &Tarea,
    bootstrap: &Path,
    ej: &dyn EjecutorHerramienta,
    version_sondas: u32,
) -> Option<String> {
    let version = historial::hash_de_version(bootstrap).ok()?;
    let mut digest = Sha256::new();
    for parte in [
        version,
        version_sondas.to_string(),
        tarea.imagen.clone(),
        tarea.lenguaje.clone(),
        ej.huella_rustsec()
            .unwrap_or_else(|| ej.tiene_rustsec().to_string()),
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
            mapa.entry(clave_base(h)).or_default().push(h.linea);
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
            base.nativas.entry(clave_base(h)).or_default().push(h.linea);
        }
    }
    let de_configs = configs::sondas_configs_locales(bootstrap, tarea);
    completa &= de_configs
        .iter()
        .filter(|s| s.dimension == Dimension::Seguridad)
        .all(|s| s.valor.is_some());
    for sonda in de_configs
        .iter()
        .filter(|s| s.dimension == Dimension::Seguridad)
    {
        for h in &sonda.hallazgos {
            base.configs.entry(clave_base(h)).or_default().push(h.linea);
        }
    }
    (base, completa)
}

/// Clave de la línea base. El id no basta: un aviso y una vulnerabilidad del mismo crate y versión
/// comparten id. Por eso suma la severidad. En cargo audit suma también el texto antes de `:`.
fn clave_base(h: &Hallazgo) -> String {
    if es_sonda(&h.fuente, "cargo-audit") {
        let aviso = h
            .detalle
            .split_once(':')
            .map_or(h.detalle.as_str(), |(a, _)| a);
        format!("{}|{}|{aviso}", h.id, h.severidad)
    } else {
        format!("{}|{}", h.id, h.severidad)
    }
}

fn leer(ruta: &Path) -> Option<LineaBase> {
    let texto = fs::read_to_string(ruta).ok()?;
    serde_json::from_str(&texto).ok()
}

/// Guarda la línea base en `ruta` de forma atómica. Escribe un temporal en la misma carpeta y luego
/// lo renombra al destino. Así un lector nunca ve un archivo a medio escribir. Si la escritura o el
/// rename fallan, borra el temporal y devuelve el error.
fn guardar(ruta: &Path, base: &LineaBase) -> std::io::Result<()> {
    let carpeta = ruta.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(carpeta)?;
    let texto = serde_json::to_string(base).map_err(std::io::Error::other)?;
    let nombre = ruta.file_name().unwrap_or_default().to_string_lossy();
    let temporal = carpeta.join(format!("{nombre}.tmp-{}", std::process::id()));
    let escrito = fs::write(&temporal, texto).and_then(|()| fs::rename(&temporal, ruta));
    if escrito.is_err() {
        let _ = fs::remove_file(&temporal);
    }
    escrito
}

/// Marca `previo` en cada hallazgo que estaba en la base. Una clave (`clave_base`) cuenta tantas veces como aparece en la base.
/// Con `cambios` (nativas), solo cuentan los hallazgos fuera de los rangos cambiados. Los primeros que
/// alcanzan para la base quedan previos. El resto es nuevo, también el que cae dentro del cambio.
pub fn marcar(
    hallazgos: &mut [Hallazgo],
    base: &BTreeMap<String, Vec<u32>>,
    cambios: Option<&BTreeMap<String, Vec<(u32, u32)>>>,
) {
    let mut refs: Vec<&mut Hallazgo> = hallazgos.iter_mut().collect();
    marcar_refs(&mut refs, base, cambios);
}

/// Marca `previo` en los hallazgos de Seguridad de las sondas de configs, con `base.configs`.
/// Con `inicial`, la regla es la de las nativas: un hallazgo en un rango cambiado sale nuevo.
pub fn marcar_configs(
    sondas: &mut [ResultadoSonda],
    base: &LineaBase,
    inicial: Option<&Path>,
    repo: &Path,
) {
    let todos: Vec<Hallazgo> = sondas
        .iter()
        .filter(|s| s.dimension == Dimension::Seguridad)
        .flat_map(|s| s.hallazgos.iter().cloned())
        .collect();
    let marca = inicial.map(|i| cambios_para_marca(i, repo, &todos));
    let mut refs: Vec<&mut Hallazgo> = sondas
        .iter_mut()
        .filter(|s| s.dimension == Dimension::Seguridad)
        .flat_map(|s| s.hallazgos.iter_mut())
        .collect();
    marcar_refs(&mut refs, &base.configs, marca.as_ref());
}

/// Marca los hallazgos de gitleaks con `base.gitleaks` y los de cargo audit con `base.cargo_audit`.
/// Los demás hallazgos de herramientas no se marcan.
pub fn marcar_herramientas(sondas: &mut [ResultadoSonda], base: &LineaBase) {
    let mut gitleaks = Vec::new();
    let mut cargo = Vec::new();
    for h in sondas.iter_mut().flat_map(|s| s.hallazgos.iter_mut()) {
        if es_sonda(&h.fuente, "gitleaks") {
            gitleaks.push(h);
        } else if es_sonda(&h.fuente, "cargo-audit") {
            cargo.push(h);
        }
    }
    marcar_refs(&mut gitleaks, &base.gitleaks, None);
    marcar_refs(&mut cargo, &base.cargo_audit, None);
}

fn marcar_refs(
    hallazgos: &mut [&mut Hallazgo],
    base: &BTreeMap<String, Vec<u32>>,
    cambios: Option<&BTreeMap<String, Vec<(u32, u32)>>>,
) {
    let mut grupos: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, h) in hallazgos.iter().enumerate() {
        grupos.entry(clave_base(h)).or_default().push(i);
    }
    for h in hallazgos.iter_mut() {
        h.previo = false;
    }
    for (clave, indices) in grupos {
        let Some(lineas) = base.get(&clave) else {
            continue;
        };
        let candidatos: Vec<usize> = indices
            .into_iter()
            .filter(|&i| !en_cambio(hallazgos[i], cambios))
            .collect();
        for &i in candidatos.iter().take(lineas.len()) {
            hallazgos[i].previo = true;
        }
    }
}

/// Sondas que encuentran secretos. Sus hallazgos son secretos.
pub const FUENTES_DE_SECRETOS: [&str; 2] = ["gitleaks", "configs-secretos"];

/// Verdadero si `fuente` es una sonda de secretos: gitleaks o configs-secretos.
pub fn es_fuente_de_secretos(fuente: &Fuente) -> bool {
    FUENTES_DE_SECRETOS
        .iter()
        .any(|nombre| es_sonda(fuente, nombre))
}

fn es_sonda(fuente: &Fuente, nombre: &str) -> bool {
    matches!(fuente, Fuente::Sonda(n) if n == nombre)
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

/// Rangos que usa la marca de previo. Igual que `rangos_cambiados`, pero un archivo que no se puede
/// leer cuenta entero como cambiado.
pub(crate) fn cambios_para_marca(
    inicial: &Path,
    repo: &Path,
    hallazgos: &[Hallazgo],
) -> BTreeMap<String, Vec<(u32, u32)>> {
    let mut cambios = crate::oracle::cambios::rangos_cambiados(inicial, repo);
    let archivos: BTreeSet<&str> = hallazgos.iter().map(|h| h.archivo.as_str()).collect();
    for archivo in archivos {
        if cambios.contains_key(archivo) {
            continue;
        }
        if es_ilegible(&repo.join(archivo)) || es_ilegible(&inicial.join(archivo)) {
            cambios.insert(archivo.to_string(), vec![(1, u32::MAX)]);
        }
    }
    cambios
}

/// Verdadero si `ruta` es un archivo regular (no enlace) que `fs::read_to_string` no puede leer.
fn es_ilegible(ruta: &Path) -> bool {
    fs::symlink_metadata(ruta).is_ok_and(|m| m.file_type().is_file())
        && fs::read_to_string(ruta).is_err()
}

/// Copia de las sondas de Seguridad sin sus hallazgos previos, salvo los secretos (ver `es_fuente_de_secretos`).
/// Un secreto que ya estaba cuenta en el puntaje. El valor de cada sonda que lo tenga sale de sus hallazgos que quedan.
pub fn quitar_previos(sondas: &[ResultadoSonda], reqs: &[Requisito]) -> Vec<ResultadoSonda> {
    sondas
        .iter()
        .map(|sonda| {
            let mut copia = sonda.clone();
            copia
                .hallazgos
                .retain(|h| !h.previo || es_fuente_de_secretos(&h.fuente));
            if copia.valor.is_some() {
                copia.valor = Some(puntuar_hallazgos(&copia.hallazgos, reqs));
            }
            copia
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oracle::ErrorOraculo;
    use crate::oracle::perfiles::herramientas::{
        COMANDO_CARGO_AUDIT, COMANDO_GITLEAKS, SalidaHerramienta,
    };
    use crate::oracle::perfiles::{crear_hallazgo, puntuar_hallazgos};
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

    /// Ejecutor de mentira con una huella de RustSec fija. No corre comandos.
    struct EjecutorConHuella {
        huella: String,
    }

    impl EjecutorHerramienta for EjecutorConHuella {
        fn ejecutar(&self, _cmd: &str) -> Result<SalidaHerramienta, ErrorOraculo> {
            Ok(SalidaHerramienta::default())
        }

        fn tiene_rustsec(&self) -> bool {
            true
        }

        fn huella_rustsec(&self) -> Option<String> {
            Some(self.huella.clone())
        }
    }

    #[test]
    fn previos_dos_bases_de_rustsec_distintas_dan_claves_distintas() {
        let bootstrap = tempfile::tempdir().expect("temp dir");
        fs::write(bootstrap.path().join("Cargo.lock"), CARGO_LOCK).expect("escribe el lock");
        let (_tarea_dir, tarea) = tarea_rust();
        let base_a = EjecutorConHuella { huella: "a".into() };
        let base_b = EjecutorConHuella { huella: "b".into() };
        let base_a_otra_vez = EjecutorConHuella { huella: "a".into() };

        let clave_a = clave_de(&tarea, bootstrap.path(), &base_a);
        let clave_b = clave_de(&tarea, bootstrap.path(), &base_b);
        let clave_a_bis = clave_de(&tarea, bootstrap.path(), &base_a_otra_vez);

        assert!(clave_a.is_some(), "bootstrap se lee: hay clave");
        assert_ne!(clave_a, clave_b, "otra base de RustSec, otra clave");
        assert_eq!(clave_a, clave_a_bis, "la misma base da la misma clave");
    }

    #[test]
    fn previos_otro_lenguaje_da_otra_clave() {
        let bootstrap = tempfile::tempdir().expect("temp dir");
        fs::write(bootstrap.path().join("Cargo.lock"), CARGO_LOCK).expect("escribe el lock");
        let tarea_de = |lenguaje: &str| {
            let dir = tempfile::tempdir().expect("temp dir");
            let texto = format!(
                "nombre = \"x\"\nlenguaje = \"{lenguaje}\"\nimagen = \"i\"\nspec = \"s\"\n\
                 [contrato]\ncmd = \"c\"\n[oculto]\ncmd = \"o\"\n"
            );
            fs::write(dir.path().join("tarea.toml"), texto).expect("escribe tarea.toml");
            let tarea = Tarea::cargar_de(dir.path()).expect("carga la tarea");
            (dir, tarea)
        };
        let (_dir_rust, rust) = tarea_de("rust");
        let (_dir_python, python) = tarea_de("python");
        let (_dir_rust_bis, rust_bis) = tarea_de("rust");
        let base = EjecutorConHuella { huella: "a".into() };

        let clave_rust = clave_de(&rust, bootstrap.path(), &base);
        let clave_python = clave_de(&python, bootstrap.path(), &base);
        let clave_rust_bis = clave_de(&rust_bis, bootstrap.path(), &base);

        assert_ne!(clave_rust, clave_python, "otro lenguaje, otra clave");
        assert_eq!(
            clave_rust, clave_rust_bis,
            "el mismo lenguaje da la misma clave"
        );
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
            base.entry(clave_base(h)).or_default().push(h.linea);
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
    fn previos_hallazgo_nativo_en_archivo_sin_rango_legible_sale_nuevo() {
        let inicial = tempfile::tempdir().expect("temp dir inicial");
        let repo = tempfile::tempdir().expect("temp dir repo");
        // Bytes que no son UTF-8, iguales en ambos lados: el archivo no tiene rango legible.
        for dir in [inicial.path(), repo.path()] {
            fs::create_dir_all(dir.join("src")).expect("crea src");
            fs::write(dir.join("src/x.rs"), [0xff, 0xfe, b'\n']).expect("escribe x.rs");
        }
        let base = base_de(&[nativo_de("src/x.rs", 1, "cita")]);
        let mut repo_hallazgos = [nativo_de("src/x.rs", 1, "cita")];
        let cambios = cambios_para_marca(inicial.path(), repo.path(), &repo_hallazgos);
        marcar(&mut repo_hallazgos, &base, Some(&cambios));
        assert!(
            !repo_hallazgos[0].previo,
            "el archivo no se pudo leer: cuenta entero como cambiado"
        );

        // Control: un archivo UTF-8 igual en ambos lados sigue dando previo.
        for dir in [inicial.path(), repo.path()] {
            fs::write(dir.join("src/y.rs"), "fn f() {}\n").expect("escribe y.rs");
        }
        let base = base_de(&[nativo_de("src/y.rs", 1, "otra")]);
        let mut repo_hallazgos = [nativo_de("src/y.rs", 1, "otra")];
        let cambios = cambios_para_marca(inicial.path(), repo.path(), &repo_hallazgos);
        marcar(&mut repo_hallazgos, &base, Some(&cambios));
        assert!(repo_hallazgos[0].previo, "el archivo igual sigue previo");
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

    /// Hallazgo de cargo audit con su severidad y su detalle (`aviso: título`).
    fn cargo_audit_con(sev: u8, detalle: &str) -> Hallazgo {
        crear_hallazgo(
            Dimension::Seguridad,
            sev,
            "Cargo.lock",
            16,
            "time 0.1.43",
            detalle,
            "cargo-audit",
            &[],
        )
    }

    #[test]
    fn previos_aviso_unmaintained_no_tapa_la_vulnerabilidad_del_mismo_crate() {
        let aviso = cargo_audit_con(1, "RUSTSEC-2020-0071: unmaintained");
        let vuln = cargo_audit_con(3, "RUSTSEC-2020-0072: vulnerable");
        assert_eq!(aviso.id, vuln.id, "mismo archivo, cita y dimensión");
        let base = LineaBase {
            cargo_audit: base_de(std::slice::from_ref(&aviso)),
            ..LineaBase::default()
        };
        // El parser emite las vulnerabilidades antes que los avisos.
        let mut sondas = [ResultadoSonda {
            dimension: Dimension::Seguridad,
            valor: None,
            motivo: None,
            hallazgos: vec![vuln, aviso],
        }];
        marcar_herramientas(&mut sondas, &base);
        let hs = &sondas[0].hallazgos;
        assert!(!hs[0].previo, "la vulnerabilidad es nueva");
        assert!(hs[1].previo, "el aviso viejo sigue previo");
    }

    #[test]
    fn previos_unsafe_y_unwrap_en_la_misma_linea_no_se_confunden() {
        let cita = "let x = y.unwrap();";
        let unsafe_ = crear_hallazgo(
            Dimension::Seguridad,
            2,
            "src/x.rs",
            7,
            cita,
            "unsafe fuera de tests",
            "seguridad-nativa",
            &[],
        );
        let unwrap = crear_hallazgo(
            Dimension::Seguridad,
            1,
            "src/x.rs",
            7,
            cita,
            "unwrap fuera de tests",
            "seguridad-nativa",
            &[],
        );
        let base = base_de(std::slice::from_ref(&unwrap));
        let mut repo = [unsafe_, unwrap];
        marcar(&mut repo, &base, None);
        assert!(!repo[0].previo, "unsafe no estaba en la base");
        assert!(repo[1].previo, "unwrap estaba en la base");
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

    #[test]
    fn previos_quitar_previos_saca_el_aviso_previo_y_recalcula_el_valor() {
        let mut previo = cargo_audit_de("Cargo.lock", 16, "time 0.1.43");
        previo.previo = true;
        let valor = puntuar_hallazgos(std::slice::from_ref(&previo), &[]);
        let sonda = ResultadoSonda {
            dimension: Dimension::Seguridad,
            valor: Some(valor),
            motivo: None,
            hallazgos: vec![previo],
        };
        let limpia = quitar_previos(&[sonda], &[]);
        assert_eq!(limpia[0].valor, Some(1.0));
        assert!(limpia[0].hallazgos.is_empty());
    }

    #[test]
    fn previos_quitar_previos_deja_el_aviso_nuevo_y_su_valor() {
        let nuevo = cargo_audit_de("Cargo.lock", 16, "time 0.1.44");
        let valor = puntuar_hallazgos(std::slice::from_ref(&nuevo), &[]);
        let sonda = ResultadoSonda {
            dimension: Dimension::Seguridad,
            valor: Some(valor),
            motivo: None,
            hallazgos: vec![nuevo],
        };
        let limpia = quitar_previos(std::slice::from_ref(&sonda), &[]);
        assert_eq!(limpia, vec![sonda]);
    }

    #[test]
    fn previos_quitar_previos_deja_el_secreto_previo_de_gitleaks() {
        let mut secreto = crear_hallazgo(
            Dimension::Seguridad,
            3,
            "src/lib.rs",
            1,
            "clave",
            "clave en el código",
            "gitleaks",
            &[],
        );
        secreto.previo = true;
        let valor = puntuar_hallazgos(std::slice::from_ref(&secreto), &[]);
        let sonda = ResultadoSonda {
            dimension: Dimension::Seguridad,
            valor: Some(valor),
            motivo: None,
            hallazgos: vec![secreto],
        };
        let limpia = quitar_previos(std::slice::from_ref(&sonda), &[]);
        assert_eq!(limpia, vec![sonda], "un secreto previo sigue en el puntaje");
    }

    /// Un secreto en YAML: la clave es secreta y el valor es literal.
    const YAML_CON_SECRETO: &str = "db:\n  password: s3cr3t-valor\n";

    /// Escribe `config.yml` con `texto` en `dir`.
    fn config_en(dir: &Path, texto: &str) {
        fs::write(dir.join("config.yml"), texto).expect("escribe config.yml");
    }

    /// Hallazgos de la fuente `configs-secretos` de `sondas`.
    fn secretos_de(sondas: &[ResultadoSonda]) -> Vec<Hallazgo> {
        sondas
            .iter()
            .flat_map(|s| s.hallazgos.iter())
            .filter(|h| es_sonda(&h.fuente, "configs-secretos"))
            .cloned()
            .collect()
    }

    #[test]
    fn previos_secreto_de_configs_igual_en_bootstrap_queda_previo() {
        let home = tempfile::tempdir().expect("temp dir");
        let bootstrap = tempfile::tempdir().expect("temp dir");
        let repo = tempfile::tempdir().expect("temp dir");
        config_en(bootstrap.path(), YAML_CON_SECRETO);
        config_en(repo.path(), YAML_CON_SECRETO);
        let (_tarea_dir, tarea) = tarea_rust();
        let ej = EjecutorContado::nuevo();

        let base = linea_base(home.path(), &tarea, bootstrap.path(), &ej);
        assert_eq!(
            base.configs.len(),
            1,
            "el secreto del inicio va a la base: {base:?}"
        );

        let mut sondas = configs::sondas_configs_locales(repo.path(), &tarea);
        marcar_configs(&mut sondas, &base, Some(bootstrap.path()), repo.path());
        let secretos = secretos_de(&sondas);
        assert_eq!(secretos.len(), 1, "{secretos:?}");
        assert_eq!(secretos[0].severidad, 3);
        assert!(secretos[0].previo, "igual que en bootstrap/: es previo");
    }

    #[test]
    fn previos_secreto_de_configs_solo_en_el_repo_queda_nuevo() {
        let home = tempfile::tempdir().expect("temp dir");
        let bootstrap = tempfile::tempdir().expect("temp dir");
        let repo = tempfile::tempdir().expect("temp dir");
        config_en(bootstrap.path(), "db:\n  host: localhost\n");
        config_en(repo.path(), YAML_CON_SECRETO);
        let (_tarea_dir, tarea) = tarea_rust();
        let ej = EjecutorContado::nuevo();

        let base = linea_base(home.path(), &tarea, bootstrap.path(), &ej);
        assert!(
            base.configs.is_empty(),
            "bootstrap/ no tiene secreto: {base:?}"
        );

        let mut sondas = configs::sondas_configs_locales(repo.path(), &tarea);
        marcar_configs(&mut sondas, &base, Some(bootstrap.path()), repo.path());
        let secretos = secretos_de(&sondas);
        assert_eq!(secretos.len(), 1, "{secretos:?}");
        assert!(!secretos[0].previo, "no estaba en bootstrap/: es nuevo");
    }

    #[test]
    fn previos_quitar_previos_deja_el_secreto_previo_de_configs() {
        let mut secreto = crear_hallazgo(
            Dimension::Seguridad,
            3,
            "config.yml",
            2,
            "password",
            "clave con forma de secreto y valor literal",
            "configs-secretos",
            &[],
        );
        secreto.previo = true;
        let valor = puntuar_hallazgos(std::slice::from_ref(&secreto), &[]);
        let sonda = ResultadoSonda {
            dimension: Dimension::Seguridad,
            valor: Some(valor),
            motivo: None,
            hallazgos: vec![secreto],
        };
        let limpia = quitar_previos(std::slice::from_ref(&sonda), &[]);
        assert_eq!(
            limpia,
            vec![sonda],
            "un secreto de configs previo sigue en el puntaje"
        );
    }

    #[test]
    fn previos_linea_base_con_la_version_vieja_no_se_reusa() {
        let home = tempfile::tempdir().expect("temp dir");
        let bootstrap = tempfile::tempdir().expect("temp dir");
        fs::write(bootstrap.path().join("Cargo.lock"), CARGO_LOCK).expect("escribe el lock");
        let (_tarea_dir, tarea) = tarea_rust();
        let ej = EjecutorContado::nuevo();

        let vieja = clave_de_version(&tarea, bootstrap.path(), &ej, 2).expect("hay clave");
        assert_ne!(
            Some(vieja.clone()),
            clave_de(&tarea, bootstrap.path(), &ej),
            "la versión nueva de las sondas cambia la clave"
        );
        let marcada = LineaBase {
            cargo_audit: BTreeMap::from([("MARCA|3".to_string(), vec![1])]),
            ..LineaBase::default()
        };
        let ruta = home
            .path()
            .join(CARPETA)
            .join(&tarea.nombre)
            .join(format!("{vieja}.json"));
        guardar(&ruta, &marcada).expect("guarda la base con la clave vieja");

        let base = linea_base(home.path(), &tarea, bootstrap.path(), &ej);
        assert_ne!(base, marcada, "la base con la clave vieja no se reusa");
        assert!(ej.cuantos() > 0, "la base se recalcula con las sondas");
    }

    #[test]
    fn previos_sonda_de_configs_sin_veredicto_no_guarda_la_base() {
        let home = tempfile::tempdir().expect("temp dir");
        let bootstrap = tempfile::tempdir().expect("temp dir");
        fs::write(bootstrap.path().join("Cargo.lock"), CARGO_LOCK).expect("escribe el lock");
        // Bytes que no son UTF-8: la sonda de secretos no da veredicto.
        fs::create_dir_all(bootstrap.path().join("src")).expect("crea src");
        fs::write(bootstrap.path().join("src/lib.rs"), "pub fn f() {}\n").expect("escribe lib.rs");
        fs::write(bootstrap.path().join("config.yml"), [0xff, 0xfe, b'\n'])
            .expect("escribe config.yml");
        let (_tarea_dir, tarea) = tarea_rust();
        let ej = EjecutorContado::nuevo();

        let _ = linea_base(home.path(), &tarea, bootstrap.path(), &ej);
        let corridos = ej.cuantos();
        assert!(corridos > 0, "la primera llamada corre sondas");
        let _ = linea_base(home.path(), &tarea, bootstrap.path(), &ej);
        assert!(
            ej.cuantos() > corridos,
            "sin veredicto no se guarda: la segunda llamada corre sondas otra vez"
        );
    }

    /// Nombres de los archivos de `carpeta`, ordenados.
    fn nombres_en(carpeta: &Path) -> Vec<String> {
        let mut nombres: Vec<String> = fs::read_dir(carpeta)
            .expect("lee la carpeta")
            .map(|e| {
                e.expect("entrada")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        nombres.sort();
        nombres
    }

    #[test]
    fn previos_guardar_es_atomico_y_no_deja_temporales() {
        let base = LineaBase {
            cargo_audit: BTreeMap::from([("RUSTSEC-2020-0072|3".to_string(), vec![16])]),
            ..LineaBase::default()
        };
        let dir = tempfile::tempdir().expect("temp dir");

        // Caso 1: carpeta nueva, que `guardar` crea. Solo queda el archivo final.
        let carpeta = dir.path().join("nueva");
        let ruta = carpeta.join("clave.json");
        guardar(&ruta, &base).expect("guarda en carpeta nueva");
        assert_eq!(leer(&ruta), Some(base.clone()), "devuelve la misma base");
        assert_eq!(nombres_en(&carpeta), vec!["clave.json"], "sin temporales");

        // Caso 2: reemplaza un archivo viejo. No queda rastro del contenido viejo.
        let carpeta = dir.path().join("vieja");
        fs::create_dir_all(&carpeta).expect("crea la carpeta");
        let ruta = carpeta.join("clave.json");
        fs::write(&ruta, "viejo").expect("escribe el archivo viejo");
        guardar(&ruta, &base).expect("reemplaza el archivo viejo");
        assert_eq!(leer(&ruta), Some(base.clone()), "reemplaza el viejo");
        assert_eq!(nombres_en(&carpeta), vec!["clave.json"], "sin temporales");

        // Caso 3: `ruta` es una carpeta no vacía. El rename falla: Err, la carpeta intacta y sin temporal.
        let carpeta = dir.path().join("tres");
        let ruta = carpeta.join("clave.json");
        fs::create_dir_all(&ruta).expect("crea ruta como carpeta");
        fs::write(ruta.join("dentro.txt"), "intacto").expect("escribe dentro");
        assert!(guardar(&ruta, &base).is_err(), "no reemplaza una carpeta");
        assert_eq!(nombres_en(&carpeta), vec!["clave.json"], "sin temporales");
        assert_eq!(
            fs::read_to_string(ruta.join("dentro.txt")).expect("lee dentro"),
            "intacto",
            "el contenido de la carpeta queda intacto"
        );
    }
}
