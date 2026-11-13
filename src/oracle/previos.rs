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
use crate::oracle::perfiles::{ResultadoSonda, puntuar_hallazgos};
use crate::oracle::tarea::{Requisito, Tarea};
use crate::tipos::{Dimension, Fuente, Hallazgo};

/// Sube cuando cambia una sonda de Seguridad. Cambia la clave de la línea base guardada.
pub const VERSION_SONDAS_SEGURIDAD: u32 = 2;

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

/// Clave de la memoria: el hash de `bootstrap/`, la versión de las sondas, la imagen y la base de RustSec.
/// La base suma su huella de contenido. Sin huella, suma solo si va montada.
/// `None` si `bootstrap/` no se puede leer: entonces no hay memoria y la línea base se calcula.
fn clave_de(tarea: &Tarea, bootstrap: &Path, ej: &dyn EjecutorHerramienta) -> Option<String> {
    let version = historial::hash_de_version(bootstrap).ok()?;
    let mut digest = Sha256::new();
    for parte in [
        version,
        VERSION_SONDAS_SEGURIDAD.to_string(),
        tarea.imagen.clone(),
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

fn guardar(ruta: &Path, base: &LineaBase) -> std::io::Result<()> {
    if let Some(carpeta) = ruta.parent() {
        fs::create_dir_all(carpeta)?;
    }
    let texto = serde_json::to_string(base).map_err(std::io::Error::other)?;
    fs::write(ruta, texto)
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

/// Copia de las sondas de Seguridad sin sus hallazgos previos, salvo los de gitleaks. Un secreto que
/// ya estaba cuenta en el puntaje. El valor de cada sonda que lo tenga sale de sus hallazgos que quedan.
pub fn quitar_previos(sondas: &[ResultadoSonda], reqs: &[Requisito]) -> Vec<ResultadoSonda> {
    sondas
        .iter()
        .map(|sonda| {
            let mut copia = sonda.clone();
            copia
                .hallazgos
                .retain(|h| !h.previo || es_sonda(&h.fuente, "gitleaks"));
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
}
