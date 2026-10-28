//! Sondas de herramientas externas del perfil `rust` (Tarea 9-B). Corren dentro del contenedor
//! de evaluación, con `--network none`. Si la herramienta falta en la imagen, dan `SinVeredicto`.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;

use super::{ResultadoSonda, crear_hallazgo, puntuar_hallazgos};
use crate::oracle::ErrorOraculo;
use crate::oracle::contenedor::{self, Ejecucion};
use crate::oracle::descarga::PRELUDIO;
use crate::oracle::tarea::{Requisito, Tarea};
use crate::tipos::{Dimension, Hallazgo};

/// Salida de un comando del contenedor de evaluación.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SalidaHerramienta {
    pub codigo: i32,
    pub stdout: String,
    pub stderr: String,
    /// Archivos que el comando dejó en `/salida`: nombre -> contenido.
    pub archivos: BTreeMap<String, String>,
}

/// Ejecuta comandos de una herramienta. Un `Err` es fallo de infraestructura, no de producto.
pub trait EjecutorHerramienta {
    fn ejecutar(&self, cmd: &str) -> Result<SalidaHerramienta, ErrorOraculo>;

    /// Verdadero si la base de RustSec va montada en `/rustsec`.
    fn tiene_rustsec(&self) -> bool {
        false
    }
}

/// Ejecutor real: cada comando corre en un contenedor nuevo y sin red.
pub struct EjecutorContenedor {
    pub imagen: String,
    /// Copia del repo del alumno. Se monta en solo lectura en `/repo`.
    pub repo: PathBuf,
    /// Carpeta del oráculo. Se monta en solo lectura en `/oraculo`.
    pub oraculo: Option<PathBuf>,
    /// Base de RustSec. Se monta en solo lectura en `/rustsec`.
    pub rustsec: Option<PathBuf>,
    /// Caché de dependencias. Se monta en solo lectura en `/cache`.
    pub cache: Option<PathBuf>,
    pub timeout: Duration,
}

impl EjecutorContenedor {
    /// Ejecución de podman para `cmd`, con `salida` montada en `/salida`.
    fn ejecucion(&self, cmd: &str, salida: &Path) -> Ejecucion {
        Ejecucion {
            imagen: self.imagen.clone(),
            repo: Some(self.repo.clone()),
            oraculo: self.oraculo.clone(),
            salida: salida.to_path_buf(),
            cmd: cmd.to_string(),
            env: Vec::new(),
            timeout: self.timeout,
            red: false,
            cache: self.cache.clone(),
            cache_rw: false,
            rustsec: self.rustsec.clone(),
        }
    }
}

impl EjecutorHerramienta for EjecutorContenedor {
    fn ejecutar(&self, cmd: &str) -> Result<SalidaHerramienta, ErrorOraculo> {
        let salida = tempfile::Builder::new()
            .prefix("sondas-")
            .tempdir()
            .map_err(|e| ErrorOraculo::Infra(format!("carpeta de salida: {e}")))?;
        // El contenedor escribe con otro uid: la carpeta de salida se abre a todos.
        fs::set_permissions(salida.path(), fs::Permissions::from_mode(0o777))
            .map_err(|e| ErrorOraculo::Infra(format!("permisos de /salida: {e}")))?;
        let resultado = contenedor::ejecutar(&self.ejecucion(cmd, salida.path()))?;
        Ok(SalidaHerramienta {
            codigo: resultado.codigo,
            stdout: resultado.stdout,
            stderr: resultado.stderr,
            archivos: leer_salida(salida.path())?,
        })
    }

    fn tiene_rustsec(&self) -> bool {
        self.rustsec.is_some()
    }
}

/// Reporte JSON de gitleaks: `Secret` y `Match` no están en la estructura, así que nunca se leen.
#[derive(Deserialize)]
struct FugaGitleaks {
    #[serde(rename = "RuleID", default)]
    regla: String,
    #[serde(rename = "File", default)]
    archivo: String,
    #[serde(rename = "StartLine", default)]
    linea: u32,
    #[serde(rename = "Description", default)]
    descripcion: String,
}

pub fn parsear_gitleaks(json: &str, requisitos: &[Requisito]) -> Result<Vec<Hallazgo>, String> {
    // `null` y `[]` dan lo mismo: sin filtraciones. Los campos del secreto no se deserializan.
    let fugas: Option<Vec<FugaGitleaks>> = serde_json::from_str(json).map_err(|e| e.to_string())?;
    Ok(fugas
        .unwrap_or_default()
        .into_iter()
        .map(|f| {
            let archivo = f
                .archivo
                .strip_prefix("/repo/")
                .unwrap_or(f.archivo.as_str());
            crear_hallazgo(
                Dimension::Seguridad,
                3,
                archivo,
                f.linea,
                &f.regla,
                &f.descripcion,
                "gitleaks",
                requisitos,
            )
        })
        .collect())
}

/// Salida de `cargo audit --json`. Las vulnerabilidades son severidad 3; los avisos, severidad 1.
pub fn parsear_cargo_audit(
    json: &str,
    cargo_lock: &str,
    requisitos: &[Requisito],
) -> Result<Vec<Hallazgo>, String> {
    let v: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let vulnerabilidades = v
        .get("vulnerabilities")
        .ok_or("falta el bloque vulnerabilities")?;
    let mut hallazgos = Vec::new();
    for item in vulnerabilidades
        .get("list")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let nombre = texto(item, "/package/name");
        let version = texto(item, "/package/version");
        let detalle = format!(
            "{}: {}",
            texto(item, "/advisory/id"),
            texto(item, "/advisory/title")
        );
        let linea = linea_de_paquete(cargo_lock, &nombre, &version);
        hallazgos.push(crear_hallazgo(
            Dimension::Seguridad,
            3,
            "Cargo.lock",
            linea,
            &format!("{nombre} {version}"),
            &detalle,
            "cargo-audit",
            requisitos,
        ));
    }
    // Los avisos (unmaintained, yanked, unsound) solo se listan: severidad 1.
    for (tipo, lista) in v
        .get("warnings")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        for item in lista.as_array().into_iter().flatten() {
            let nombre = texto(item, "/package/name");
            let version = texto(item, "/package/version");
            let detalle = format!(
                "{tipo}: {} {}",
                texto(item, "/advisory/id"),
                texto(item, "/advisory/title")
            );
            let linea = linea_de_paquete(cargo_lock, &nombre, &version);
            hallazgos.push(crear_hallazgo(
                Dimension::Seguridad,
                1,
                "Cargo.lock",
                linea,
                &format!("{nombre} {version}"),
                detalle.trim_end(),
                "cargo-audit",
                requisitos,
            ));
        }
    }
    Ok(hallazgos)
}

/// Líneas JSON de `cargo clippy --message-format=json`. Solo cuentan los lints `clippy::`.
pub fn parsear_clippy(salida: &str, requisitos: &[Requisito]) -> Vec<Hallazgo> {
    salida
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["reason"] == "compiler-message")
        .filter_map(|v| {
            let mensaje = &v["message"];
            let codigo = mensaje["code"]["code"].as_str()?;
            if !codigo.starts_with("clippy::") {
                return None;
            }
            let severidad = match mensaje["level"].as_str()? {
                "error" => 3,
                "warning" => 2,
                _ => return None,
            };
            let primario = mensaje["spans"]
                .as_array()?
                .iter()
                .find(|s| s["is_primary"] == true)?;
            let archivo = primario["file_name"].as_str()?;
            let linea = u32::try_from(primario["line_start"].as_u64()?).ok()?;
            let cita = primario["text"]
                .as_array()
                .and_then(|t| t.first())
                .and_then(|t| t["text"].as_str())
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .unwrap_or(codigo);
            let detalle = mensaje["message"].as_str().unwrap_or(codigo);
            Some(crear_hallazgo(
                Dimension::Estructura,
                severidad,
                archivo,
                linea,
                cita,
                detalle,
                "clippy",
                requisitos,
            ))
        })
        .collect()
}

/// Última línea JSON con `ops_s`. Devuelve `(ops_s, valor)` con `valor = min(1, ops_s / piso)`.
pub fn parsear_eficiencia(stdout: &str, piso: f64) -> Result<(f64, f32), String> {
    if !(piso.is_finite() && piso > 0.0) {
        return Err(format!("piso inválido: {piso}"));
    }
    let ops_s = stdout
        .lines()
        .rev()
        .find_map(|l| {
            serde_json::from_str::<Value>(l)
                .ok()?
                .get("ops_s")?
                .as_f64()
        })
        .ok_or("ninguna línea JSON trae ops_s")?;
    if !(ops_s.is_finite() && ops_s >= 0.0) {
        return Err(format!("ops_s inválido: {ops_s}"));
    }
    Ok((ops_s, (ops_s / piso).min(1.0) as f32))
}

/// Primer error de rustc con span que no es de clippy: el código no compila.
fn error_de_compilacion(reporte: &str) -> Option<String> {
    reporte
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["reason"] == "compiler-message")
        .find_map(|v| {
            let mensaje = &v["message"];
            let codigo = mensaje["code"]["code"].as_str().unwrap_or_default();
            let con_span = mensaje["spans"].as_array().is_some_and(|s| !s.is_empty());
            (mensaje["level"] == "error" && con_span && !codigo.starts_with("clippy::")).then(
                || {
                    mensaje["message"]
                        .as_str()
                        .unwrap_or("error de compilación")
                        .to_string()
                },
            )
        })
}

/// Largo máximo de la cola de stderr que entra al motivo.
const COLA_MOTIVO: usize = 300;

pub(crate) fn sin_veredicto(dimension: Dimension, motivo: impl Into<String>) -> ResultadoSonda {
    ResultadoSonda {
        dimension,
        valor: None,
        motivo: Some(motivo.into()),
        hallazgos: Vec::new(),
    }
}

pub(crate) fn con_veredicto(
    dimension: Dimension,
    hallazgos: Vec<Hallazgo>,
    requisitos: &[Requisito],
) -> ResultadoSonda {
    ResultadoSonda {
        dimension,
        valor: Some(puntuar_hallazgos(&hallazgos, requisitos)),
        motivo: None,
        hallazgos,
    }
}

/// Si la herramienta no está en la imagen, o no se puede consultar, la sonda no tiene veredicto.
/// Así no corre el comando real.
pub(crate) fn falta_herramienta(
    ej: &dyn EjecutorHerramienta,
    dimension: Dimension,
    nombre: &str,
) -> Option<ResultadoSonda> {
    let cmd = if nombre == "clippy" {
        "cargo clippy --version".to_string()
    } else {
        format!("command -v {nombre}")
    };
    match ej.ejecutar(&cmd) {
        Ok(s) if s.codigo == 0 => None,
        Ok(_) => Some(sin_veredicto(
            dimension,
            format!("{nombre} no está en la imagen de evaluación"),
        )),
        Err(e) => Some(sin_veredicto(dimension, e.to_string())),
    }
}

/// Últimos caracteres de `texto`, sin espacios de los extremos.
fn cola(texto: &str) -> String {
    let t = texto.trim();
    let total = t.chars().count();
    t.chars().skip(total.saturating_sub(COLA_MOTIVO)).collect()
}

/// Agrega la cola de stderr al motivo, si hay.
pub(crate) fn con_cola(base: String, stderr: &str) -> String {
    let c = cola(stderr);
    if c.is_empty() {
        base
    } else {
        format!("{base}: {c}")
    }
}

/// Lee los archivos sueltos de `dir`. Las subcarpetas, como `target/`, no se leen.
fn leer_salida(dir: &Path) -> Result<BTreeMap<String, String>, ErrorOraculo> {
    let entradas =
        fs::read_dir(dir).map_err(|e| ErrorOraculo::Infra(format!("leer /salida: {e}")))?;
    let mut archivos = BTreeMap::new();
    for entrada in entradas.flatten() {
        let ruta = entrada.path();
        if !ruta.is_file() {
            continue;
        }
        let bytes = fs::read(&ruta)
            .map_err(|e| ErrorOraculo::Infra(format!("leer {}: {e}", ruta.display())))?;
        archivos.insert(
            entrada.file_name().to_string_lossy().into_owned(),
            String::from_utf8_lossy(&bytes).into_owned(),
        );
    }
    Ok(archivos)
}

fn texto(v: &Value, ruta: &str) -> String {
    v.pointer(ruta)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// Línea del `name = "<nombre>"` cuya `version` coincide. Si no coincide, la primera de ese
/// nombre. Cero si el paquete no está en el lock.
fn linea_de_paquete(cargo_lock: &str, nombre: &str, version: &str) -> u32 {
    let lineas: Vec<&str> = cargo_lock.lines().map(str::trim).collect();
    let linea_nombre = format!("name = \"{nombre}\"");
    let linea_version = format!("version = \"{version}\"");
    let mut primera = None;
    for (i, linea) in lineas.iter().enumerate() {
        if *linea != linea_nombre {
            continue;
        }
        if lineas.get(i + 1) == Some(&linea_version.as_str()) {
            return (i + 1) as u32;
        }
        if primera.is_none() {
            primera = Some(i + 1);
        }
    }
    primera.map_or(0, |n| n as u32)
}

/// Seguridad: filtraciones de secretos con gitleaks. Cada una es severidad 3.
pub fn gitleaks(ej: &dyn EjecutorHerramienta, tarea: &Tarea) -> ResultadoSonda {
    let d = Dimension::Seguridad;
    if let Some(r) = falta_herramienta(ej, d, "gitleaks") {
        return r;
    }
    let cmd = "gitleaks detect --no-git --source /repo --report-format json \
        --report-path /salida/gitleaks.json --exit-code 0 --no-banner";
    let salida = match ej.ejecutar(cmd) {
        Ok(s) => s,
        Err(e) => return sin_veredicto(d, e.to_string()),
    };
    let Some(reporte) = salida.archivos.get("gitleaks.json") else {
        return sin_veredicto(
            d,
            con_cola("gitleaks no dejó reporte".into(), &salida.stderr),
        );
    };
    match parsear_gitleaks(reporte, &tarea.requisito) {
        Ok(hs) => con_veredicto(d, hs, &tarea.requisito),
        Err(e) => sin_veredicto(d, format!("reporte de gitleaks ilegible: {e}")),
    }
}

/// Seguridad: dependencias con CVE de la base de RustSec. Sin base o sin `Cargo.lock`, no hay
/// veredicto, y no se corre nada.
pub fn cargo_audit(ej: &dyn EjecutorHerramienta, tarea: &Tarea, repo: &Path) -> ResultadoSonda {
    let d = Dimension::Seguridad;
    if !ej.tiene_rustsec() {
        return sin_veredicto(d, "sin base de RustSec");
    }
    let Ok(lock) = fs::read_to_string(repo.join("Cargo.lock")) else {
        return sin_veredicto(d, "falta Cargo.lock: cargo audit no tiene qué revisar");
    };
    if let Some(r) = falta_herramienta(ej, d, "cargo-audit") {
        return r;
    }
    // cargo audit sale con 1 si hay vulnerabilidades: el veredicto sale del reporte, no del código.
    let cmd = "cd /repo && cargo audit --no-fetch --db /rustsec --json > /salida/audit.json";
    let salida = match ej.ejecutar(cmd) {
        Ok(s) => s,
        Err(e) => return sin_veredicto(d, e.to_string()),
    };
    let Some(reporte) = salida.archivos.get("audit.json") else {
        return sin_veredicto(
            d,
            con_cola("cargo audit no dejó reporte".into(), &salida.stderr),
        );
    };
    match parsear_cargo_audit(reporte, &lock, &tarea.requisito) {
        Ok(hs) => con_veredicto(d, hs, &tarea.requisito),
        Err(e) => sin_veredicto(d, format!("reporte de cargo audit ilegible: {e}")),
    }
}

/// Estructura: lints de clippy en `correctness`, `suspicious` y `perf`. El estilo no cuenta.
pub fn clippy(ej: &dyn EjecutorHerramienta, tarea: &Tarea) -> ResultadoSonda {
    let d = Dimension::Estructura;
    if let Some(r) = falta_herramienta(ej, d, "clippy") {
        return r;
    }
    let cmd = [
        PRELUDIO,
        "cd /repo && CARGO_TARGET_DIR=/salida/target cargo clippy --message-format=json \
         -- -A clippy::all -W clippy::correctness -W clippy::suspicious -W clippy::perf \
         > /salida/clippy.json 2>/dev/null",
    ]
    .concat();
    let salida = match ej.ejecutar(&cmd) {
        Ok(s) => s,
        Err(e) => return sin_veredicto(d, e.to_string()),
    };
    let Some(reporte) = salida.archivos.get("clippy.json") else {
        let base = format!("clippy no dejó reporte (código {})", salida.codigo);
        return sin_veredicto(d, con_cola(base, &salida.stderr));
    };
    if let Some(error) = error_de_compilacion(reporte) {
        return sin_veredicto(d, format!("el código no compila: {error}"));
    }
    // Sin cierre de build no hay prueba de que clippy corrió. Un reporte vacío cae aquí.
    let Some(exito) = cierre_de_build(reporte) else {
        let base = "clippy no dejó un reporte válido (¿no pudo escribir Cargo.lock o compilar?)";
        return sin_veredicto(d, con_cola(base.into(), &salida.stderr));
    };
    // Con éxito, o con lints de clippy que hacen fallar el build, el veredicto sale de los
    // hallazgos. Un fallo sin ningún lint de clippy no es un veredicto.
    let hallazgos = parsear_clippy(reporte, &tarea.requisito);
    if !exito && hallazgos.is_empty() {
        return sin_veredicto(d, con_cola("el código no compila".into(), &salida.stderr));
    }
    con_veredicto(d, hallazgos, &tarea.requisito)
}

/// Éxito del build según la línea `build-finished` de cargo. `None` si el reporte no la trae.
fn cierre_de_build(reporte: &str) -> Option<bool> {
    reporte
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find(|v| v["reason"] == "build-finished")
        .and_then(|v| v["success"].as_bool())
}

/// Eficiencia: corre `eficiencia.cmd` del oráculo. Por debajo del piso, un hallazgo de severidad 3.
pub fn eficiencia(ej: &dyn EjecutorHerramienta, tarea: &Tarea) -> ResultadoSonda {
    let d = Dimension::Eficiencia;
    let Some(efi) = &tarea.eficiencia else {
        return ResultadoSonda {
            dimension: d,
            valor: None,
            motivo: Some("la tarea no define [eficiencia]".into()),
            hallazgos: Vec::new(),
        };
    };
    let salida = match ej.ejecutar(&efi.cmd) {
        Ok(s) => s,
        Err(e) => return sin_veredicto(d, e.to_string()),
    };
    if salida.codigo != 0 {
        let base = format!("eficiencia.cmd falló (código {})", salida.codigo);
        return sin_veredicto(d, con_cola(base, &salida.stderr));
    }
    let (ops_s, valor) = match parsear_eficiencia(&salida.stdout, efi.piso) {
        Ok(p) => p,
        Err(e) => return sin_veredicto(d, format!("salida de eficiencia inválida: {e}")),
    };
    let hallazgos = if ops_s < efi.piso {
        vec![crear_hallazgo(
            d,
            3,
            "",
            0,
            &format!("ops_s={ops_s}"),
            &format!("por debajo del piso {}", efi.piso),
            "eficiencia",
            &tarea.requisito,
        )]
    } else {
        Vec::new()
    };
    ResultadoSonda {
        dimension: d,
        valor: Some(valor),
        motivo: None,
        hallazgos,
    }
}

/// Las cuatro sondas de herramientas del perfil `rust`. Otro lenguaje da vector vacío.
pub fn sondas_herramientas(
    ej: &dyn EjecutorHerramienta,
    tarea: &Tarea,
    repo: &Path,
) -> Vec<ResultadoSonda> {
    if tarea.lenguaje != "rust" {
        return Vec::new();
    }
    vec![
        gitleaks(ej, tarea),
        cargo_audit(ej, tarea, repo),
        clippy(ej, tarea),
        eficiencia(ej, tarea),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oracle::contenedor::argumentos;
    use crate::oracle::perfiles::combinar;
    use crate::oracle::perfiles::rust::{LimitesEstructura, estructura_nativa, seguridad_nativa};
    use crate::tipos::Fuente;
    use std::cell::RefCell;

    const GITLEAKS: &str = include_str!("../../../tests/fixtures/sondas/gitleaks.json");
    const GITLEAKS_VACIO: &str = include_str!("../../../tests/fixtures/sondas/gitleaks-vacio.json");
    const GITLEAKS_NULL: &str = include_str!("../../../tests/fixtures/sondas/gitleaks-null.json");
    const CARGO_AUDIT: &str = include_str!("../../../tests/fixtures/sondas/cargo-audit.json");
    const CARGO_LOCK: &str = include_str!("../../../tests/fixtures/sondas/cargo-lock.txt");
    const CLIPPY: &str = include_str!("../../../tests/fixtures/sondas/clippy.jsonl");
    const EFICIENCIA: &str = include_str!("../../../tests/fixtures/sondas/eficiencia.txt");
    const EFICIENCIA_RUIDO: &str =
        include_str!("../../../tests/fixtures/sondas/eficiencia-con-ruido.txt");

    type Respuesta = Box<dyn Fn(&str) -> Result<SalidaHerramienta, ErrorOraculo>>;

    /// Ejecutor de mentira: guarda cada comando y responde con la función dada.
    struct EjecutorFalso {
        cmds: RefCell<Vec<String>>,
        responde: Respuesta,
        rustsec: bool,
    }

    impl EjecutorFalso {
        fn nuevo(
            responde: impl Fn(&str) -> Result<SalidaHerramienta, ErrorOraculo> + 'static,
        ) -> Self {
            EjecutorFalso {
                cmds: RefCell::new(Vec::new()),
                responde: Box::new(responde),
                rustsec: false,
            }
        }

        fn con_rustsec(mut self) -> Self {
            self.rustsec = true;
            self
        }

        fn cmds(&self) -> Vec<String> {
            self.cmds.borrow().clone()
        }
    }

    impl EjecutorHerramienta for EjecutorFalso {
        fn ejecutar(&self, cmd: &str) -> Result<SalidaHerramienta, ErrorOraculo> {
            self.cmds.borrow_mut().push(cmd.to_string());
            (self.responde)(cmd)
        }

        fn tiene_rustsec(&self) -> bool {
            self.rustsec
        }
    }

    fn salida(codigo: i32, stdout: &str) -> SalidaHerramienta {
        SalidaHerramienta {
            codigo,
            stdout: stdout.into(),
            ..Default::default()
        }
    }

    fn con_archivo(nombre: &str, contenido: &str) -> SalidaHerramienta {
        let mut s = salida(0, "");
        s.archivos.insert(nombre.into(), contenido.into());
        s
    }

    /// Responde "presente" a las comprobaciones de herramienta. Lo demás va a `resto`.
    fn con_herramientas(resto: impl Fn(&str) -> SalidaHerramienta + 'static) -> EjecutorFalso {
        EjecutorFalso::nuevo(move |cmd| {
            if cmd.starts_with("command -v") || cmd.starts_with("cargo clippy --version") {
                Ok(salida(0, ""))
            } else {
                Ok(resto(cmd))
            }
        })
    }

    fn tarea_de(lenguaje: &str, eficiencia: Option<(&str, f64)>) -> Tarea {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut texto = format!(
            "nombre = \"x\"\nlenguaje = \"{lenguaje}\"\nimagen = \"i\"\nspec = \"s\"\n\
             [contrato]\ncmd = \"c\"\n[oculto]\ncmd = \"o\"\n"
        );
        if let Some((cmd, piso)) = eficiencia {
            texto.push_str(&format!("[eficiencia]\ncmd = '{cmd}'\npiso = {piso:?}\n"));
        }
        fs::write(dir.path().join("tarea.toml"), texto).expect("escribe tarea.toml");
        Tarea::cargar_de(dir.path()).expect("carga la tarea")
    }

    fn tarea_rust(eficiencia: Option<(&str, f64)>) -> Tarea {
        tarea_de("rust", eficiencia)
    }

    /// Repo mínimo con `Cargo.lock` del fixture.
    fn repo_con_lock() -> tempfile::TempDir {
        let repo = tempfile::tempdir().expect("temp dir");
        fs::write(repo.path().join("Cargo.lock"), CARGO_LOCK).expect("escribe el lock");
        repo
    }

    /// Repo con un fuente de `n` líneas de código.
    fn repo_con_fuente(ruta: &str, texto: &str) -> tempfile::TempDir {
        let repo = tempfile::tempdir().expect("temp dir");
        let archivo = repo.path().join(ruta);
        fs::create_dir_all(archivo.parent().expect("con padre")).expect("crea carpeta");
        fs::write(archivo, texto).expect("escribe el fuente");
        repo
    }

    // ---------- Parsers puros ----------

    #[test]
    fn sonda_gitleaks_dos_filtraciones_dan_dos_hallazgos_sev_3() {
        let hs = parsear_gitleaks(GITLEAKS, &[]).expect("parsea");
        assert_eq!(hs.len(), 2);
        assert!(
            hs.iter()
                .all(|h| h.severidad == 3 && h.dimension == Dimension::Seguridad)
        );
        assert_eq!(hs[0].archivo, "src/config.rs", "sin el prefijo /repo/");
        assert_eq!(
            (hs[0].linea, hs[0].cita.as_str()),
            (12, "aws-access-key-id")
        );
        assert_eq!(hs[1].archivo, "src/cliente/mod.rs");
        assert_eq!(hs[1].linea, 40);
        assert_eq!(hs[0].fuente, Fuente::Sonda("gitleaks".into()));
    }

    #[test]
    fn sonda_gitleaks_nunca_copia_el_secreto() {
        let hs = parsear_gitleaks(GITLEAKS, &[]).expect("parsea");
        let volcado = format!("{hs:?}") + &serde_json::to_string(&hs).expect("serializa");
        assert!(!volcado.contains("AKIAFIXTURE0000000001"));
        assert!(!volcado.contains("clavefalsa0000000000000000000000"));
    }

    #[test]
    fn sonda_gitleaks_arreglo_vacio_y_null_dan_cero_hallazgos() {
        assert!(
            parsear_gitleaks(GITLEAKS_VACIO, &[])
                .expect("vacío")
                .is_empty()
        );
        assert!(
            parsear_gitleaks(GITLEAKS_NULL, &[])
                .expect("null")
                .is_empty()
        );
    }

    #[test]
    fn sonda_gitleaks_json_roto_es_error() {
        assert!(parsear_gitleaks("{", &[]).is_err());
    }

    #[test]
    fn sonda_cargo_audit_vulnerabilidad_sev_3_y_aviso_sev_1() {
        let hs = parsear_cargo_audit(CARGO_AUDIT, CARGO_LOCK, &[]).expect("parsea");
        assert_eq!(hs.len(), 2);
        let vuln = &hs[0];
        assert_eq!(
            (vuln.severidad, vuln.archivo.as_str(), vuln.linea),
            (3, "Cargo.lock", 16)
        );
        assert_eq!(vuln.cita, "time 0.1.43");
        assert!(vuln.detalle.contains("RUSTSEC-2020-0071"));
        assert!(vuln.detalle.contains("Potential segfault"));
        assert_eq!(vuln.fuente, Fuente::Sonda("cargo-audit".into()));
        let aviso = &hs[1];
        assert_eq!((aviso.severidad, aviso.linea), (1, 6));
        assert_eq!(aviso.cita, "atty 0.2.14");
        assert!(aviso.detalle.contains("unmaintained"));
    }

    #[test]
    fn sonda_cargo_audit_paquete_fuera_del_lock_da_linea_cero() {
        let hs = parsear_cargo_audit(CARGO_AUDIT, "", &[]).expect("parsea");
        assert_eq!(hs[0].linea, 0);
    }

    #[test]
    fn sonda_cargo_audit_sin_bloque_de_vulnerabilidades_es_error() {
        assert!(parsear_cargo_audit("{\"warnings\": {}}", CARGO_LOCK, &[]).is_err());
    }

    #[test]
    fn sonda_clippy_ignora_rustc_y_basura_y_mapea_severidad() {
        let hs = parsear_clippy(CLIPPY, &[]);
        assert_eq!(
            hs.len(),
            2,
            "el aviso de rustc y las líneas basura no cuentan"
        );
        let aviso = &hs[0];
        assert_eq!(
            (aviso.severidad, aviso.archivo.as_str(), aviso.linea),
            (2, "src/cliente/mod.rs", 15)
        );
        assert_eq!(aviso.cita, "if a == b {");
        assert_eq!(aviso.dimension, Dimension::Estructura);
        assert_eq!(aviso.fuente, Fuente::Sonda("clippy".into()));
        let error = &hs[1];
        assert_eq!(
            (error.severidad, error.archivo.as_str(), error.linea),
            (3, "src/protocolo/mod.rs", 42)
        );
        assert_eq!(error.cita, "a == a");
        assert!(error.detalle.contains("equal expressions"));
    }

    #[test]
    fn sonda_clippy_error_de_rustc_con_span_es_de_compilacion() {
        let reporte = r#"{"reason":"compiler-message","message":{"code":{"code":"E0425"},"level":"error","message":"cannot find value `y`","spans":[{"file_name":"src/x.rs","is_primary":true,"line_start":3,"text":[{"text":"let a = y;"}]}]}}"#;
        assert_eq!(
            error_de_compilacion(reporte).as_deref(),
            Some("cannot find value `y`")
        );
    }

    #[test]
    fn sonda_clippy_errores_de_lint_no_son_de_compilacion() {
        assert_eq!(error_de_compilacion(CLIPPY), None);
    }

    #[test]
    fn sonda_eficiencia_sobre_el_piso_da_valor_uno() {
        assert_eq!(
            parsear_eficiencia(EFICIENCIA_RUIDO, 5000.0),
            Ok((7000.0, 1.0))
        );
    }

    #[test]
    fn sonda_eficiencia_bajo_el_piso_da_la_mitad() {
        assert_eq!(parsear_eficiencia(EFICIENCIA, 5000.0), Ok((2500.0, 0.5)));
    }

    #[test]
    fn sonda_eficiencia_sin_json_o_piso_invalido_es_error() {
        assert!(parsear_eficiencia("sin json\n", 5000.0).is_err());
        assert!(parsear_eficiencia(EFICIENCIA, 0.0).is_err());
        assert!(parsear_eficiencia(EFICIENCIA, f64::NAN).is_err());
    }

    // ---------- Sondas con ejecutor falso ----------

    #[test]
    fn sonda_herramienta_ausente_da_sin_veredicto_sin_correr_el_comando() {
        let tarea = tarea_rust(None);
        let ej = EjecutorFalso::nuevo(|_| Ok(salida(1, "")));
        let r = gitleaks(&ej, &tarea);
        assert_eq!(r.valor, None);
        assert_eq!(r.dimension, Dimension::Seguridad);
        assert_eq!(
            r.motivo.as_deref(),
            Some("gitleaks no está en la imagen de evaluación")
        );
        assert_eq!(ej.cmds(), ["command -v gitleaks"]);

        let ej = EjecutorFalso::nuevo(|_| Ok(salida(1, "")));
        let r = clippy(&ej, &tarea);
        assert_eq!(r.valor, None);
        assert!(r.motivo.unwrap_or_default().contains("clippy"));
        assert_eq!(ej.cmds(), ["cargo clippy --version"]);
    }

    #[test]
    fn sonda_gitleaks_con_reporte_da_hallazgos_y_comando_con_flags() {
        let ej = con_herramientas(|_| con_archivo("gitleaks.json", GITLEAKS));
        let r = gitleaks(&ej, &tarea_rust(None));
        assert_eq!(r.motivo, None);
        assert_eq!(r.hallazgos.len(), 2);
        // Dos hallazgos de severidad 3 sin impacto: penal 1/3 cada uno, valor 7/9.
        let valor = r.valor.expect("hay veredicto");
        assert!((valor - 7.0 / 9.0).abs() < 1e-6, "valor {valor}");
        let cmd = &ej.cmds()[1];
        assert!(cmd.contains("--report-path /salida/gitleaks.json"));
        assert!(cmd.contains("--no-git"));
        assert!(cmd.contains("--source /repo"));
    }

    #[test]
    fn sonda_gitleaks_sin_reporte_es_sin_veredicto() {
        let ej = con_herramientas(|_| salida(0, ""));
        let r = gitleaks(&ej, &tarea_rust(None));
        assert_eq!(r.valor, None);
        assert!(r.motivo.unwrap_or_default().contains("no dejó reporte"));
    }

    #[test]
    fn sonda_cargo_audit_sin_rustsec_no_ejecuta_nada() {
        let ej = con_herramientas(|_| panic!("no debe correr"));
        let repo = repo_con_lock();
        let r = cargo_audit(&ej, &tarea_rust(None), repo.path());
        assert_eq!(r.valor, None);
        assert_eq!(r.motivo.as_deref(), Some("sin base de RustSec"));
        assert!(ej.cmds().is_empty());
    }

    #[test]
    fn sonda_cargo_audit_sin_cargo_lock_es_sin_veredicto() {
        let ej = con_herramientas(|_| panic!("no debe correr")).con_rustsec();
        let repo = tempfile::tempdir().expect("temp dir");
        let r = cargo_audit(&ej, &tarea_rust(None), repo.path());
        assert_eq!(r.valor, None);
        assert!(r.motivo.unwrap_or_default().contains("Cargo.lock"));
        assert!(ej.cmds().is_empty());
    }

    #[test]
    fn sonda_cargo_audit_con_reporte_da_hallazgos() {
        let ej = con_herramientas(|_| con_archivo("audit.json", CARGO_AUDIT)).con_rustsec();
        let repo = repo_con_lock();
        let r = cargo_audit(&ej, &tarea_rust(None), repo.path());
        assert_eq!(r.motivo, None);
        assert_eq!(r.hallazgos.len(), 2);
        assert_eq!(ej.cmds()[0], "command -v cargo-audit");
        assert!(
            ej.cmds()[1].starts_with("cd /repo && cargo audit --no-fetch --db /rustsec --json")
        );
    }

    #[test]
    fn sonda_clippy_con_reporte_da_hallazgos_y_comando_con_categorias() {
        let ej = con_herramientas(|_| con_archivo("clippy.json", CLIPPY));
        let r = clippy(&ej, &tarea_rust(None));
        assert_eq!(r.dimension, Dimension::Estructura);
        assert_eq!(r.hallazgos.len(), 2);
        let cmd = &ej.cmds()[1];
        assert!(cmd.starts_with(PRELUDIO));
        assert!(cmd.contains(
            "cd /repo && CARGO_TARGET_DIR=/salida/target cargo clippy --message-format=json"
        ));
        assert!(cmd.contains(
            "-A clippy::all -W clippy::correctness -W clippy::suspicious -W clippy::perf"
        ));
        assert!(cmd.contains("> /salida/clippy.json 2>/dev/null"));
    }

    #[test]
    fn sonda_clippy_sin_reporte_es_sin_veredicto() {
        let ej = con_herramientas(|_| salida(101, ""));
        let r = clippy(&ej, &tarea_rust(None));
        assert_eq!(r.valor, None);
        assert!(r.motivo.unwrap_or_default().contains("no dejó reporte"));
    }

    #[test]
    fn sonda_clippy_no_compila_es_sin_veredicto() {
        let reporte = r#"{"reason":"compiler-message","message":{"code":{"code":"E0425"},"level":"error","message":"cannot find value `y`","spans":[{"file_name":"src/x.rs","is_primary":true,"line_start":3,"text":[{"text":"let a = y;"}]}]}}"#;
        let ej = con_herramientas(move |_| con_archivo("clippy.json", reporte));
        let r = clippy(&ej, &tarea_rust(None));
        assert_eq!(r.valor, None);
        assert!(r.motivo.unwrap_or_default().contains("no compila"));
    }

    #[test]
    fn sonda_clippy_reporte_vacio_es_sin_veredicto() {
        let ej = con_herramientas(|_| con_archivo("clippy.json", ""));
        let r = clippy(&ej, &tarea_rust(None));
        assert_eq!(r.valor, None);
        assert!(
            r.motivo
                .unwrap_or_default()
                .contains("no dejó un reporte válido")
        );
    }

    #[test]
    fn sonda_clippy_solo_artifacts_sin_build_finished_es_sin_veredicto() {
        let reporte = "{\"reason\":\"compiler-artifact\",\"target\":{\"name\":\"x\"}}\n";
        let ej = con_herramientas(move |_| con_archivo("clippy.json", reporte));
        let r = clippy(&ej, &tarea_rust(None));
        assert_eq!(r.valor, None);
        assert!(
            r.motivo
                .unwrap_or_default()
                .contains("no dejó un reporte válido")
        );
    }

    #[test]
    fn sonda_clippy_build_finished_ok_sin_mensajes_da_uno() {
        let reporte = "{\"reason\":\"compiler-artifact\",\"target\":{\"name\":\"x\"}}\n\
                       {\"reason\":\"build-finished\",\"success\":true}\n";
        let ej = con_herramientas(move |_| con_archivo("clippy.json", reporte));
        let r = clippy(&ej, &tarea_rust(None));
        assert_eq!(r.motivo, None);
        assert_eq!(r.valor, Some(1.0));
        assert!(r.hallazgos.is_empty());
    }

    #[test]
    fn sonda_clippy_build_finished_falso_sin_error_es_no_compila() {
        let reporte = "{\"reason\":\"build-finished\",\"success\":false}\n";
        let ej = con_herramientas(move |_| con_archivo("clippy.json", reporte));
        let r = clippy(&ej, &tarea_rust(None));
        assert_eq!(r.valor, None);
        assert!(r.motivo.unwrap_or_default().contains("no compila"));
    }

    #[test]
    fn sonda_eficiencia_sin_tabla_no_penaliza_ni_corre_nada() {
        let ej = con_herramientas(|_| panic!("no debe correr"));
        let r = eficiencia(&ej, &tarea_rust(None));
        assert_eq!(r.valor, None);
        assert_eq!(r.motivo.as_deref(), Some("la tarea no define [eficiencia]"));
        assert!(r.hallazgos.is_empty());
        assert!(ej.cmds().is_empty());
    }

    #[test]
    fn sonda_eficiencia_bajo_el_piso_da_hallazgo_sev_3() {
        let tarea = tarea_rust(Some(("bench", 5000.0)));
        let ej = con_herramientas(|_| salida(0, "{\"ops_s\": 2500}\n"));
        let r = eficiencia(&ej, &tarea);
        assert_eq!(r.dimension, Dimension::Eficiencia);
        assert_eq!(r.valor, Some(0.5));
        assert_eq!(r.hallazgos.len(), 1);
        let h = &r.hallazgos[0];
        assert_eq!((h.severidad, h.archivo.as_str(), h.linea), (3, "", 0));
        assert_eq!(h.cita, "ops_s=2500");
        assert!(h.detalle.contains("por debajo del piso 5000"));
        assert_eq!(ej.cmds(), ["bench"]);
    }

    #[test]
    fn sonda_eficiencia_sobre_el_piso_da_uno_sin_hallazgos() {
        let ej = con_herramientas(|_| salida(0, "{\"ops_s\": 9000}\n"));
        let r = eficiencia(&ej, &tarea_rust(Some(("bench", 5000.0))));
        assert_eq!(r.valor, Some(1.0));
        assert!(r.hallazgos.is_empty());
    }

    #[test]
    fn sonda_eficiencia_fallo_del_comando_es_sin_veredicto() {
        let ej = con_herramientas(|_| salida(2, ""));
        let r = eficiencia(&ej, &tarea_rust(Some(("bench", 5000.0))));
        assert_eq!(r.valor, None);
        assert!(r.motivo.unwrap_or_default().contains("código 2"));
    }

    #[test]
    fn sonda_sondas_herramientas_solo_para_rust() {
        let repo = tempfile::tempdir().expect("temp dir");
        let ej = con_herramientas(|_| salida(0, ""));
        let dims: Vec<Dimension> = sondas_herramientas(&ej, &tarea_rust(None), repo.path())
            .iter()
            .map(|r| r.dimension)
            .collect();
        assert_eq!(
            dims,
            [
                Dimension::Seguridad,
                Dimension::Seguridad,
                Dimension::Estructura,
                Dimension::Eficiencia
            ]
        );

        let ej = con_herramientas(|_| panic!("no debe correr"));
        let vacio = sondas_herramientas(&ej, &tarea_de("python", None), repo.path());
        assert!(vacio.is_empty());
        assert!(ej.cmds().is_empty());
    }

    #[test]
    fn sonda_combinar_seguridad_sin_cargo_audit_queda_sin_puntaje_con_hallazgos() {
        let repo = repo_con_fuente("src/x.rs", "pub unsafe fn f() {}\n");
        let propia = seguridad_nativa(repo.path(), &[]);
        let ej_gl = con_herramientas(|_| con_archivo("gitleaks.json", "[]"));
        let gl = gitleaks(&ej_gl, &tarea_rust(None));
        // Sin base de RustSec: cargo audit no tiene veredicto.
        let sin_base = EjecutorFalso::nuevo(|_| Ok(salida(0, "")));
        let ca = cargo_audit(&sin_base, &tarea_rust(None), repo.path());
        let r = combinar(Dimension::Seguridad, vec![propia, gl, ca], &[]);
        assert_eq!(r.valor, None, "sin cargo audit no hay puntaje de seguridad");
        assert_eq!(r.hallazgos.len(), 1, "conserva el unsafe");
        assert_eq!(r.motivo.as_deref(), Some("sin base de RustSec"));
    }

    #[test]
    fn sonda_combinar_seguridad_con_las_tres_partes_da_puntaje() {
        let repo = repo_con_fuente("src/x.rs", "pub unsafe fn f() {}\n");
        fs::write(repo.path().join("Cargo.lock"), CARGO_LOCK).expect("escribe el lock");
        let propia = seguridad_nativa(repo.path(), &[]);
        let ej_gl = con_herramientas(|_| con_archivo("gitleaks.json", "[]"));
        let gl = gitleaks(&ej_gl, &tarea_rust(None));
        let ej_ca =
            con_herramientas(|_| con_archivo("audit.json", "{\"vulnerabilities\":{\"list\":[]}}"))
                .con_rustsec();
        let ca = cargo_audit(&ej_ca, &tarea_rust(None), repo.path());
        let r = combinar(Dimension::Seguridad, vec![propia, gl, ca], &[]);
        assert!(r.valor.is_some());
        assert_eq!(r.hallazgos.len(), 1);
        // Un unsafe de severidad 2 sin impacto: penal 2/9, valor 25/27.
        let valor = r.valor.expect("hay puntaje");
        assert!((valor - 25.0 / 27.0).abs() < 1e-6, "valor {valor}");
    }

    #[test]
    fn sonda_combinar_estructura_sin_clippy_queda_sin_puntaje_con_hallazgos() {
        let texto: String = (0..600).map(|i| format!("let v{i} = {i};\n")).collect();
        let repo = repo_con_fuente("src/grande.rs", &texto);
        let propia = estructura_nativa(repo.path(), &[], &LimitesEstructura::default());
        let ej = con_herramientas(|_| salida(101, ""));
        let cl = clippy(&ej, &tarea_rust(None));
        let r = combinar(Dimension::Estructura, vec![propia, cl], &[]);
        assert_eq!(r.valor, None);
        assert!(
            r.hallazgos
                .iter()
                .any(|h| h.detalle.contains("archivo grande"))
        );
    }

    #[test]
    fn sonda_combinar_estructura_con_clippy_suma_hallazgos() {
        let repo = repo_con_fuente("src/x.rs", "fn a() {}\n");
        let propia = estructura_nativa(repo.path(), &[], &LimitesEstructura::default());
        let ej = con_herramientas(|_| con_archivo("clippy.json", CLIPPY));
        let cl = clippy(&ej, &tarea_rust(None));
        let r = combinar(Dimension::Estructura, vec![propia, cl], &[]);
        assert!(r.valor.is_some());
        assert_eq!(r.hallazgos.len(), 2);
    }

    #[test]
    fn sonda_ejecucion_real_corta_la_red_y_monta_con_selinux() {
        let repo = tempfile::tempdir().expect("temp dir");
        let oraculo = tempfile::tempdir().expect("temp dir");
        let rustsec = tempfile::tempdir().expect("temp dir");
        let cache = tempfile::tempdir().expect("temp dir");
        let salida_dir = tempfile::tempdir().expect("temp dir");
        let ej = EjecutorContenedor {
            imagen: "img".into(),
            repo: repo.path().into(),
            oraculo: Some(oraculo.path().into()),
            rustsec: Some(rustsec.path().into()),
            cache: Some(cache.path().into()),
            timeout: Duration::from_secs(60),
        };
        let args = argumentos(&ej.ejecucion("gitleaks detect", salida_dir.path()), "n");
        assert!(
            args.windows(2)
                .any(|p| p[0] == "--network" && p[1] == "none")
        );
        assert!(args.contains(&format!("{}:/repo:ro,z", repo.path().display())));
        assert!(args.contains(&format!("{}:/oraculo:ro,z", oraculo.path().display())));
        assert!(args.contains(&format!("{}:/rustsec:ro,z", rustsec.path().display())));
        assert!(args.contains(&format!("{}:/cache:ro,z", cache.path().display())));
        assert!(args.contains(&format!("{}:/salida:z", salida_dir.path().display())));
        assert!(ej.tiene_rustsec());
    }

    #[test]
    fn sonda_ejecucion_real_sin_rustsec_no_lo_monta() {
        let repo = tempfile::tempdir().expect("temp dir");
        let salida_dir = tempfile::tempdir().expect("temp dir");
        let ej = EjecutorContenedor {
            imagen: "img".into(),
            repo: repo.path().into(),
            oraculo: None,
            rustsec: None,
            cache: None,
            timeout: Duration::from_secs(60),
        };
        let args = argumentos(&ej.ejecucion("x", salida_dir.path()), "n");
        assert!(!args.iter().any(|a| a.ends_with(":/rustsec:ro,z")));
        assert!(!ej.tiene_rustsec());
    }
}
