//! Entrega y escenarios de usuario (Tarea 9d). Corre el producto en una copia del repo, dentro de
//! un contenedor sin red: instala, arranca y pasa cada `[[escenario]]`. La verificación estática
//! del `Dockerfile` reusa la regla de "corre como root" de 9b. La construcción sin red
//! (`construir_dockerfile`, Tarea 9d2) no se conecta a `evaluar`: Tarea 13 la arma.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::oracle::ErrorOraculo;
use crate::oracle::configs::{self, TipoConfig, instrucciones, palabra_clave};
use crate::oracle::contenedor::{SalidaPodman, correr_podman};
use crate::oracle::descarga::PRELUDIO;
use crate::oracle::perfiles::herramientas::EjecutorHerramienta;
use crate::oracle::perfiles::{ResultadoSonda, crear_hallazgo, puntuar_hallazgos};
use crate::oracle::tarea::{Requisito, Tarea};
use crate::oracle::{CARPETA_TEMPORALES, copiar_repo, infra, temporal};
use crate::tipos::{Dimension, Hallazgo, Pendiente};

const SONDA: &str = "entrega";
const DIMENSION: Dimension = Dimension::Entrega;
/// Carpeta con escritura. Todos los `.rc` y `.log` de la entrega van aquí.
const SALIDA: &str = "/salida";
/// Líneas finales del log de `instalar` que entran al resultado.
const LINEAS_LOG: usize = 40;
/// Líneas finales de la salida de cada escenario que entran a `/salida`.
const LINEAS_ESCENARIO: usize = 20;
/// Log completo de `correr`. Vive en `/tmp` del contenedor: nunca llega a `/salida`.
const LOG_CORRER_COMPLETO: &str = "/tmp/entrega-correr.full.log";
/// Límite de cada escenario, en segundos.
const TIMEOUT_ESCENARIO: u32 = 120;

/// Plazo por defecto de cada `podman build`.
const PLAZO_BUILD: Duration = Duration::from_secs(20 * 60);
/// Plazo de las consultas rápidas de podman: inspeccionar, correr `id` y borrar.
const PLAZO_CORTO: Duration = Duration::from_secs(60);
/// Nombre de la sonda y etiqueta del cuerpo del archivo de pendientes (Tarea 9d2).
const TEST_BUILD: &str = "dockerfile::build";
/// Fragmentos (en minúscula) de la salida de un `RUN` que delatan falta de red. Solo cuentan
/// dentro de un `RUN`. "unable to locate package" no suma: sin "temporary failure" o "could not
/// resolve" ya cuenta por los patrones de abajo.
const PATRONES_RED: [&str; 13] = [
    "could not resolve host",
    "temporary failure in name resolution",
    "network is unreachable",
    "name or service not known",
    "connection refused",
    "connection timed out",
    "failed to connect",
    "could not connect",
    "unable to connect",
    "dns: transient error",
    "temporary error (try again later)",
    "failed to fetch",
    "bad address",
];
/// Fragmentos (en minúscula) de un `FROM` cuya imagen no está local (`--pull=never`).
const PATRONES_AUSENTE: [&str; 3] = ["image not known", "no such image", "unable to find"];
/// Contador de las etiquetas temporales de `podman build`.
static CONTADOR_BUILD: AtomicU64 = AtomicU64::new(0);

/// Lo que sale de construir los Dockerfile del repo sin red (Tarea 9d2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ResultadoDockerfile {
    pub hallazgos: Vec<Hallazgo>,
    pub pendientes: Vec<Pendiente>,
    /// `Some(true)` si la imagen corre sin root. `None` si no hubo imagen que arrancar.
    pub corre_sin_root: Option<bool>,
    /// Verdadero si todos los Dockerfile del repo construyeron.
    pub construida: bool,
}

/// Construye los Dockerfile del repo con `--network none --pull=never` y verifica que corren sin root.
/// Solo corre `podman build` y `podman run --entrypoint id`. Los `[[escenario]]` contra la imagen
/// construida quedan fuera: los actuales corren en el contenedor de evaluación (`sonda_entrega`).
/// La parte con red (bajar la imagen base) espera la aclaración P17b.
pub fn construir_dockerfile(
    repo: &Path,
    home: &Path,
    tarea: &Tarea,
) -> Result<ResultadoDockerfile, ErrorOraculo> {
    construir_dockerfile_con(repo, home, tarea, PLAZO_BUILD)
}

/// Como `construir_dockerfile`, con el límite de cada `podman build` explícito.
pub fn construir_dockerfile_con(
    repo: &Path,
    home: &Path,
    tarea: &Tarea,
    plazo: Duration,
) -> Result<ResultadoDockerfile, ErrorOraculo> {
    let reqs = tarea.requisito.as_slice();
    let dockerfiles: Vec<String> = configs::buscar_archivos(repo)
        .into_iter()
        .filter(|a| a.tipo == TipoConfig::Dockerfile)
        .map(|a| a.ruta_relativa)
        .collect();
    let mut resultado = ResultadoDockerfile {
        hallazgos: Vec::new(),
        pendientes: Vec::new(),
        corre_sin_root: None,
        construida: false,
    };
    if dockerfiles.is_empty() {
        return Ok(resultado);
    }
    // La copia aísla el repo. El contexto de build sale de ella, nunca del repo original.
    let tmp = home.join(CARPETA_TEMPORALES);
    fs::create_dir_all(&tmp).map_err(|e| infra("directorio temporal de ROOT_HOME", e))?;
    let copia = temporal(&tmp).map_err(|e| infra("directorio temporal de la construcción", e))?;
    let contexto = copia.path().join("repo");
    copiar_repo(repo, &contexto).map_err(|e| infra("copia del repo para construir", e))?;
    // La regla de root la emite la sonda estática: la construcción no la repite.
    let estatica = configs::dockerfile_corre_como_root(repo, reqs).map_err(ErrorOraculo::Infra)?;
    let mut todas_ok = true;
    let mut sin_root: Vec<bool> = Vec::new();
    for ruta in &dockerfiles {
        let texto = fs::read_to_string(contexto.join(ruta))
            .map_err(|e| infra("leer el Dockerfile para construir", e))?;
        // La imagen temporal se borra al salir de la iteración, pase lo que pase.
        let imagen = ImagenTemporal(etiqueta_unica());
        let df = contexto.join(ruta).to_string_lossy().into_owned();
        let ctx = contexto.to_string_lossy().into_owned();
        let args = [
            "build",
            "--network",
            "none",
            "--pull=never",
            "-t",
            imagen.0.as_str(),
            "-f",
            df.as_str(),
            ctx.as_str(),
        ];
        let build = correr_podman(&args, plazo)?;
        if build.codigo != 0 {
            todas_ok = false;
            match clasificar_fallo(&build, ruta, &texto, reqs)? {
                Desenlace::Pendiente(motivo) => resultado.pendientes.push(Pendiente {
                    test: TEST_BUILD.into(),
                    motivo,
                    colab: None,
                }),
                Desenlace::Hallazgo(h) => resultado.hallazgos.push(h),
            }
            continue;
        }
        let sin_root_ahora = corre_sin_root(&imagen.0)?;
        if !sin_root_ahora
            && !estatica.iter().any(|h| h.archivo == *ruta)
            && let Some((linea, cita)) = ultimo_from(&texto)
        {
            resultado.hallazgos.push(crear_hallazgo(
                DIMENSION,
                3,
                ruta,
                linea,
                &cita,
                configs::DETALLE_RAIZ,
                SONDA,
                reqs,
            ));
        }
        sin_root.push(sin_root_ahora);
    }
    resultado.construida = todas_ok;
    resultado.corre_sin_root = (!sin_root.is_empty()).then(|| sin_root.iter().all(|s| *s));
    Ok(resultado)
}

/// Imagen temporal de una construcción. Su `Drop` la borra con `podman rmi -f`, pase lo que pase.
struct ImagenTemporal(String);

impl Drop for ImagenTemporal {
    fn drop(&mut self) {
        // Solo borra su propia etiqueta. Un fallo no importa: la imagen quizá nunca se creó.
        let _ = correr_podman(&["rmi", "-f", self.0.as_str()], PLAZO_CORTO);
    }
}

/// Etiqueta única del proceso: `root-entrega-<pid>-<n>`.
fn etiqueta_unica() -> String {
    let n = CONTADOR_BUILD.fetch_add(1, Ordering::Relaxed);
    format!("root-entrega-{}-{n}", std::process::id())
}

/// Verdadero si la imagen corre sin root. `id -u` lo confirma. Sin `id`, vale el `USER` de la imagen.
fn corre_sin_root(tag: &str) -> Result<bool, ErrorOraculo> {
    let inspeccion = correr_podman(
        &["image", "inspect", "--format", "{{.Config.User}}", tag],
        PLAZO_CORTO,
    )?;
    let usuario = inspeccion.stdout.trim().to_string();
    let uid = correr_podman(
        &[
            "run",
            "--rm",
            "--network",
            "none",
            "--pull=never",
            "--entrypoint",
            "id",
            tag,
            "-u",
        ],
        PLAZO_CORTO,
    )?;
    if uid.codigo == 0
        && let Ok(numero) = uid.stdout.trim().parse::<u32>()
    {
        return Ok(numero != 0);
    }
    Ok(!es_root(&usuario))
}

/// Verdadero si el `USER` de la imagen es root o 0, o si no define ninguno.
fn es_root(usuario: &str) -> bool {
    let nombre = usuario.split(':').next().unwrap_or("").trim();
    usuario.is_empty() || nombre == "root" || nombre == "0"
}

/// Línea y cita de la última instrucción `FROM` del Dockerfile.
fn ultimo_from(texto: &str) -> Option<(u32, String)> {
    instrucciones(texto)
        .into_iter()
        .rev()
        .find(|i| palabra_clave(&i.logico).0 == "FROM")
        .map(|i| (i.linea, i.cita))
}

/// Desenlace de un build que falló: un pendiente sin cobro, o un hallazgo del producto.
enum Desenlace {
    Pendiente(String),
    Hallazgo(Hallazgo),
}

/// Separa un fallo de build en falta de red, imagen base ausente, o producto que no construye.
/// Sin ningún `STEP` en la salida, podman no empezó: es infraestructura.
fn clasificar_fallo(
    build: &SalidaPodman,
    archivo: &str,
    texto: &str,
    reqs: &[Requisito],
) -> Result<Desenlace, ErrorOraculo> {
    let salida = format!("{}\n{}", build.stdout, build.stderr);
    let Some(paso) = paso_fallido(&salida) else {
        let ultima = build.stderr.lines().rev().find(|l| !l.trim().is_empty());
        return Err(ErrorOraculo::Infra(format!(
            "podman no empezó la construcción: {}",
            ultima.unwrap_or("sin salida de error").trim()
        )));
    };
    let instrucciones_df = instrucciones(texto);
    let (linea, cita, kw) = match instrucciones_df.get(paso.saturating_sub(1)) {
        Some(i) => (i.linea, i.cita.clone(), palabra_clave(&i.logico).0),
        None => match instrucciones_df
            .iter()
            .find(|i| palabra_clave(&i.logico).0 == "RUN")
        {
            Some(i) => (i.linea, i.cita.clone(), "RUN".to_string()),
            None => (0, String::new(), String::new()),
        },
    };
    let minuscula = salida.to_lowercase();
    if kw == "FROM" {
        if PATRONES_AUSENTE.iter().any(|p| minuscula.contains(p)) {
            return Ok(Desenlace::Pendiente(format!(
                "imagen base no descargada (fase de descarga no corrió): línea {linea}, {cita}"
            )));
        }
        return Err(ErrorOraculo::Infra(format!(
            "podman no pudo arrancar el FROM de la línea {linea}"
        )));
    }
    if kw == "RUN" && pide_red(&salida) {
        return Ok(Desenlace::Pendiente(format!(
            "no se pudo construir sin red: línea {linea}, {cita}"
        )));
    }
    Ok(Desenlace::Hallazgo(crear_hallazgo(
        DIMENSION,
        3,
        archivo,
        linea,
        &cita,
        &format!("la imagen no construye: línea {linea}"),
        SONDA,
        reqs,
    )))
}

/// Número de paso (1-based) del último `STEP k/n` de la salida de `podman build`.
fn paso_fallido(salida: &str) -> Option<usize> {
    salida.lines().rev().find_map(|linea| {
        let resto = linea.split("STEP ").nth(1)?;
        resto.split('/').next()?.trim().parse::<usize>().ok()
    })
}

/// Verdadero si la salida de un build trae evidencia de falta de red.
fn pide_red(salida: &str) -> bool {
    let minuscula = salida.to_lowercase();
    PATRONES_RED.iter().any(|p| minuscula.contains(p))
}

/// Resultado de la sonda de entrega: el puntaje de la dimensión y el estado de cada escenario.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultadoEntrega {
    pub sonda: ResultadoSonda,
    /// Id del escenario -> `true` si su `cmd` salió 0.
    pub escenarios: BTreeMap<String, bool>,
}

/// Script de `sh` que corre la entrega y los escenarios. Escribe `.rc` y `.log` en `/salida`.
/// Sin `[entrega]`, solo corre los escenarios, sobre `/repo`.
pub fn construir_script(tarea: &Tarea) -> String {
    let mut s = String::new();
    if tarea.lenguaje == "rust" {
        // El preludio termina en `&& `: su `true` aísla un fallo de caché del resto del script.
        s.push_str(PRELUDIO);
        s.push_str("true\n");
    }
    let work = format!("{SALIDA}/work");
    if let Some(entrega) = &tarea.entrega {
        s.push_str(&format!(
            "cp -a /repo {work} || exit 1\ncd {work} || exit 1\n"
        ));
        s.push_str(&format!(
            "{{ ( {} ) 2>&1; echo $? > {SALIDA}/entrega-instalar.rc; }} \
             | tail -n {LINEAS_LOG} > {SALIDA}/entrega-instalar.log\n",
            entrega.instalar
        ));
        s.push_str(&format!(
            "if [ \"$(cat {SALIDA}/entrega-instalar.rc)\" != 0 ]; then exit 0; fi\n"
        ));
        // `correr` va dentro de un grupo de procesos propio, para matarlo entero al final.
        // El archivo `.fin` marca que `correr` terminó: sin él, sigue vivo.
        let fin = format!("{SALIDA}/entrega-correr.fin");
        let interno = format!("( {} ); echo $? > {fin}", entrega.correr);
        s.push_str(&format!(
            "if command -v setsid >/dev/null 2>&1; then GRUPO=setsid; else GRUPO=; fi\n\
             $GRUPO sh -c {} > {LOG_CORRER_COMPLETO} 2>&1 &\n\
             PID=$!\n\
             sleep {}\n\
             if [ -f {fin} ]; then echo 1 > {SALIDA}/entrega-correr.rc; \
             else echo 0 > {SALIDA}/entrega-correr.rc; fi\n",
            comillas_simples(&interno),
            entrega.espera_segundos
        ));
    } else {
        s.push_str("cd /repo || exit 1\n");
    }
    for escenario in &tarea.escenario {
        s.push_str(&format!(
            "{{ timeout {TIMEOUT_ESCENARIO} sh -c {}; echo $? > {SALIDA}/entrega-escenario-{}.rc; }} \
             2>&1 | tail -n {LINEAS_ESCENARIO} > {SALIDA}/entrega-escenario-{}.log\n",
            comillas_simples(&escenario.cmd),
            escenario.id,
            escenario.id
        ));
    }
    if tarea.entrega.is_some() {
        s.push_str(
            "if [ -n \"$PID\" ]; then kill -KILL -- -\"$PID\" 2>/dev/null; \
             kill -KILL \"$PID\" 2>/dev/null; fi\n",
        );
        // Solo la cola del log de `correr` llega a `/salida`.
        s.push_str(&format!(
            "tail -n {LINEAS_LOG} {LOG_CORRER_COMPLETO} > {SALIDA}/entrega-correr.log\n"
        ));
    }
    s
}

/// Envuelve `texto` en comillas simples de `sh`. Las comillas simples dentro salen escapadas.
fn comillas_simples(texto: &str) -> String {
    format!("'{}'", texto.replace('\'', "'\\''"))
}

/// Archivos de `archivos` que no existen en `repo`.
pub fn archivos_faltantes(repo: &Path, archivos: &[String]) -> Vec<String> {
    archivos
        .iter()
        .filter(|a| !repo.join(a).exists())
        .cloned()
        .collect()
}

/// Sonda de entrega. Un `Err` del ejecutor es infraestructura: no hay veredicto.
pub fn sonda_entrega(ej: &dyn EjecutorHerramienta, repo: &Path, tarea: &Tarea) -> ResultadoEntrega {
    let reqs = tarea.requisito.as_slice();
    let mut hallazgos = Vec::new();
    let mut motivos: Vec<String> = Vec::new();
    let archivos = tarea
        .entrega
        .as_ref()
        .map_or(&[][..], |e| e.archivos.as_slice());
    for faltante in archivos_faltantes(repo, archivos) {
        let detalle = format!("falta {faltante}");
        hallazgos.push(crear_hallazgo(
            DIMENSION, 2, &faltante, 0, &faltante, &detalle, SONDA, reqs,
        ));
    }
    match configs::dockerfile_corre_como_root(repo, reqs) {
        Ok(hs) => hallazgos.extend(hs.iter().map(|h| {
            crear_hallazgo(
                DIMENSION, 3, &h.archivo, h.linea, &h.cita, &h.detalle, SONDA, reqs,
            )
        })),
        Err(motivo) => motivos.push(motivo),
    }
    let mut escenarios = BTreeMap::new();
    if tarea.entrega.is_some() || !tarea.escenario.is_empty() {
        match ej.ejecutar(&construir_script(tarea)) {
            Ok(salida) => revisar_salida(&salida.archivos, tarea, &mut hallazgos, &mut escenarios),
            Err(e) => motivos.push(e.to_string()),
        }
    }
    // Sin veredicto: el motivo va a la sonda y los escenarios no medidos no se inventan.
    let (valor, motivo) = if motivos.is_empty() {
        (Some(puntuar_hallazgos(&hallazgos, reqs)), None)
    } else {
        (None, Some(motivos.join("; ")))
    };
    ResultadoEntrega {
        sonda: ResultadoSonda {
            dimension: DIMENSION,
            valor,
            motivo,
            hallazgos,
        },
        escenarios,
    }
}

/// Convierte los `.rc` y `.log` de la salida en hallazgos y en el estado de cada escenario.
fn revisar_salida(
    archivos: &BTreeMap<String, String>,
    tarea: &Tarea,
    hallazgos: &mut Vec<Hallazgo>,
    escenarios: &mut BTreeMap<String, bool>,
) {
    let reqs = tarea.requisito.as_slice();
    let instalo = match tarea.entrega {
        Some(_) => {
            let ok = codigo(archivos, "entrega-instalar.rc") == Some(0);
            if !ok {
                let cita = ultima_linea(archivos, "entrega-instalar.log")
                    .unwrap_or_else(|| "(sin salida)".to_string());
                let detalle = "no instala: el comando de instalación falla";
                hallazgos.push(crear_hallazgo(
                    DIMENSION, 3, "", 0, &cita, detalle, SONDA, reqs,
                ));
            }
            ok
        }
        None => true,
    };
    if tarea.entrega.is_some() && instalo && codigo(archivos, "entrega-correr.rc") == Some(1) {
        let cita = ultima_linea(archivos, "entrega-correr.log")
            .unwrap_or_else(|| "(sin salida)".to_string());
        let detalle = "no arranca: el comando de arranque muere antes de tiempo";
        hallazgos.push(crear_hallazgo(
            DIMENSION, 3, "", 0, &cita, detalle, SONDA, reqs,
        ));
    }
    for escenario in &tarea.escenario {
        let rc = codigo(archivos, &format!("entrega-escenario-{}.rc", escenario.id));
        let ok = rc == Some(0);
        escenarios.insert(escenario.id.clone(), ok);
        if ok {
            continue;
        }
        let detalle = match rc {
            _ if !instalo => "no corrió: el producto no instala".to_string(),
            Some(c) => format!("escenario falló (código {c})"),
            None => "no corrió: el escenario no llegó a su código de salida".to_string(),
        };
        let impacto: Vec<String> = reqs
            .iter()
            .filter(|r| r.escenarios.contains(&escenario.id))
            .map(|r| r.id.clone())
            .collect();
        // La cita es la última línea de su salida. Sin salida, el id del escenario.
        let cita = ultima_linea(archivos, &format!("entrega-escenario-{}.log", escenario.id))
            .unwrap_or_else(|| escenario.id.clone());
        let mut h = crear_hallazgo(DIMENSION, 2, "", 0, &cita, &detalle, SONDA, reqs);
        h.impacto = impacto;
        hallazgos.push(h);
    }
}

/// Código de salida de un `.rc`. `None` si el archivo falta o no es un número.
fn codigo(archivos: &BTreeMap<String, String>, nombre: &str) -> Option<i32> {
    archivos.get(nombre).and_then(|t| t.trim().parse().ok())
}

/// Última línea no vacía de un `.log`. `None` si el archivo falta o está vacío.
fn ultima_linea(archivos: &BTreeMap<String, String>, nombre: &str) -> Option<String> {
    archivos
        .get(nombre)
        .and_then(|t| t.lines().map(str::trim).rev().find(|l| !l.is_empty()))
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oracle::ErrorOraculo;
    use crate::oracle::perfiles::herramientas::SalidaHerramienta;
    use std::cell::{Cell, RefCell};
    use std::fs;

    /// Ejecutor de mentira: responde siempre lo mismo y cuenta las llamadas.
    struct Falso {
        salida: Result<SalidaHerramienta, String>,
        llamadas: Cell<u32>,
        cmds: RefCell<Vec<String>>,
    }

    impl Falso {
        fn con(archivos: &[(&str, &str)]) -> Self {
            let mut s = SalidaHerramienta::default();
            for (nombre, texto) in archivos {
                s.archivos.insert((*nombre).into(), (*texto).into());
            }
            Falso {
                salida: Ok(s),
                llamadas: Cell::new(0),
                cmds: RefCell::new(Vec::new()),
            }
        }

        /// Ejecutor sano: instalar y correr salen 0.
        fn sano() -> Self {
            Falso::con(&[("entrega-instalar.rc", "0\n"), ("entrega-correr.rc", "0\n")])
        }

        fn infra(msg: &str) -> Self {
            Falso {
                salida: Err(msg.into()),
                llamadas: Cell::new(0),
                cmds: RefCell::new(Vec::new()),
            }
        }
    }

    impl EjecutorHerramienta for Falso {
        fn ejecutar(&self, cmd: &str) -> Result<SalidaHerramienta, ErrorOraculo> {
            self.llamadas.set(self.llamadas.get() + 1);
            self.cmds.borrow_mut().push(cmd.to_string());
            match &self.salida {
                Ok(s) => Ok(s.clone()),
                Err(m) => Err(ErrorOraculo::Infra(m.clone())),
            }
        }
    }

    const ENTREGA: &str = "[entrega]\ninstalar = \"make\"\ncorrer = \"./run\"\n\
        archivos = [\"README.md\", \"Dockerfile\"]\n";

    const REQUISITOS: &str = "[[requisito]]\nid = \"R1\"\ntitulo = \"uno\"\n\
        escenarios = [\"b\"]\nmodulos = [\"protocolo\"]\n\n\
        [[requisito]]\nid = \"R2\"\ntitulo = \"dos\"\nescenarios = [\"a\"]\n";

    const ESCENARIOS: &str = "[[escenario]]\nid = \"a\"\ncmd = \"exit 0\"\n\n\
        [[escenario]]\nid = \"b\"\ncmd = \"exit 1\"\n";

    /// Tarea con `extra` al final del `tarea.toml`.
    fn tarea_de(lenguaje: &str, extra: &str) -> Tarea {
        let dir = tempfile::tempdir().expect("temp dir");
        let texto = format!(
            "nombre = \"x\"\nlenguaje = \"{lenguaje}\"\nimagen = \"i\"\nspec = \"s\"\n\
             [contrato]\ncmd = \"c\"\n[oculto]\ncmd = \"o\"\n{extra}"
        );
        fs::write(dir.path().join("tarea.toml"), texto).expect("escribe tarea.toml");
        Tarea::cargar_de(dir.path()).expect("carga la tarea")
    }

    /// Repo con README y Dockerfile que sí define `USER`.
    fn repo_completo() -> tempfile::TempDir {
        let repo = tempfile::tempdir().expect("temp dir");
        fs::write(repo.path().join("README.md"), "# x\n").expect("escribe README");
        fs::write(
            repo.path().join("Dockerfile"),
            "FROM debian:bookworm-slim\nUSER app\n",
        )
        .expect("escribe Dockerfile");
        repo
    }

    fn sin_hallazgos_de_entrega(r: &ResultadoEntrega) -> bool {
        r.sonda.hallazgos.is_empty()
    }

    #[test]
    fn entrega_script_incluye_preludio_solo_para_rust() {
        let rust = tarea_de("rust", ENTREGA);
        assert!(construir_script(&rust).contains(PRELUDIO));
        let shell = tarea_de("shell", ENTREGA);
        assert!(!construir_script(&shell).contains("CARGO_HOME"));
    }

    #[test]
    fn entrega_script_tiene_un_timeout_por_escenario() {
        let t = tarea_de("shell", &format!("{ENTREGA}\n{ESCENARIOS}"));
        let s = construir_script(&t);
        assert_eq!(s.matches("timeout 120").count(), 2);
    }

    #[test]
    fn entrega_script_sin_entrega_no_copia_ni_instala() {
        let t = tarea_de("shell", &format!("\n{ESCENARIOS}"));
        let s = construir_script(&t);
        assert!(!s.contains("cp -a"), "sin entrega no hay copia");
        assert!(!s.contains("entrega-instalar"), "sin entrega no instala");
        assert!(!s.contains("setsid"), "sin entrega no arranca");
        assert!(s.contains("cd /repo"), "los escenarios corren sobre /repo");
    }

    #[test]
    fn entrega_script_con_entrega_copia_instala_y_arranca_con_espera_5() {
        let s = construir_script(&tarea_de("shell", ENTREGA));
        assert!(s.contains("cp -a /repo /salida/work"));
        assert!(s.contains("/salida/entrega-instalar.rc"));
        assert!(s.contains("sleep 5"), "espera por defecto de 5 s");
        assert!(s.contains("/salida/entrega-correr.rc"));
    }

    #[test]
    fn entrega_archivos_faltantes_lista_los_que_no_existen() {
        let repo = tempfile::tempdir().expect("temp dir");
        fs::write(repo.path().join("README.md"), "x").expect("escribe");
        let archivos = ["README.md".to_string(), "Dockerfile".to_string()];
        assert_eq!(archivos_faltantes(repo.path(), &archivos), ["Dockerfile"]);
    }

    #[test]
    fn entrega_sin_readme_da_un_hallazgo_de_archivo_faltante() {
        let repo = tempfile::tempdir().expect("temp dir");
        fs::write(repo.path().join("Dockerfile"), "FROM x\nUSER app\n").expect("escribe");
        let r = sonda_entrega(&Falso::sano(), repo.path(), &tarea_de("shell", ENTREGA));
        assert_eq!(r.sonda.hallazgos.len(), 1);
        let h = &r.sonda.hallazgos[0];
        assert_eq!(h.dimension, Dimension::Entrega);
        assert_eq!(
            (h.severidad, h.archivo.as_str(), h.linea),
            (2, "README.md", 0)
        );
        assert_eq!(
            (h.cita.as_str(), h.detalle.as_str()),
            ("README.md", "falta README.md")
        );
    }

    #[test]
    fn entrega_con_readme_y_dockerfile_con_user_da_cero() {
        let repo = repo_completo();
        let r = sonda_entrega(&Falso::sano(), repo.path(), &tarea_de("shell", ENTREGA));
        assert!(sin_hallazgos_de_entrega(&r));
        assert_eq!(r.sonda.valor, Some(1.0));
        assert_eq!(r.sonda.motivo, None);
    }

    #[test]
    fn entrega_dockerfile_que_corre_como_root_da_hallazgo_entrega_sev_3() {
        let repo = tempfile::tempdir().expect("temp dir");
        fs::write(repo.path().join("README.md"), "x").expect("escribe");
        fs::write(
            repo.path().join("Dockerfile"),
            "FROM debian:bookworm-slim\n",
        )
        .expect("escribe");
        let r = sonda_entrega(&Falso::sano(), repo.path(), &tarea_de("shell", ENTREGA));
        assert_eq!(r.sonda.hallazgos.len(), 1);
        let h = &r.sonda.hallazgos[0];
        assert_eq!(h.dimension, Dimension::Entrega);
        assert_eq!(h.severidad, 3);
        assert_eq!(h.archivo, "Dockerfile");
        assert_eq!(h.fuente, crate::tipos::Fuente::Sonda("entrega".into()));
    }

    #[test]
    fn entrega_sin_entrega_ni_escenarios_no_corre_nada() {
        let repo = repo_completo();
        let ej = Falso::sano();
        let r = sonda_entrega(&ej, repo.path(), &tarea_de("shell", ""));
        assert_eq!(ej.llamadas.get(), 0);
        assert!(r.escenarios.is_empty());
        assert_eq!(r.sonda.valor, Some(1.0));
    }

    #[test]
    fn entrega_escenario_con_codigo_1_queda_false_y_con_hallazgo_de_su_requisito() {
        let repo = repo_completo();
        let ej = Falso::con(&[
            ("entrega-instalar.rc", "0\n"),
            ("entrega-correr.rc", "0\n"),
            ("entrega-escenario-a.rc", "0\n"),
            ("entrega-escenario-b.rc", "1\n"),
        ]);
        let t = tarea_de("shell", &format!("{ENTREGA}\n{ESCENARIOS}\n{REQUISITOS}"));
        let r = sonda_entrega(&ej, repo.path(), &t);
        let esperado: BTreeMap<String, bool> =
            [("a".to_string(), true), ("b".to_string(), false)].into();
        assert_eq!(r.escenarios, esperado);
        assert_eq!(r.sonda.hallazgos.len(), 1);
        let h = &r.sonda.hallazgos[0];
        assert_eq!(h.cita, "b");
        assert_eq!(
            h.impacto,
            ["R1"],
            "solo el requisito que cita el escenario b"
        );
    }

    #[test]
    fn entrega_escenario_sin_archivo_rc_queda_false() {
        let repo = repo_completo();
        let ej = Falso::con(&[("entrega-instalar.rc", "0\n"), ("entrega-correr.rc", "0\n")]);
        let t = tarea_de("shell", &format!("{ENTREGA}\n{ESCENARIOS}\n{REQUISITOS}"));
        let r = sonda_entrega(&ej, repo.path(), &t);
        assert_eq!(r.escenarios.get("a"), Some(&false));
        assert_eq!(r.escenarios.get("b"), Some(&false));
    }

    #[test]
    fn entrega_instalar_fallido_da_no_instala_y_escenarios_en_false() {
        let repo = repo_completo();
        let ej = Falso::con(&[
            ("entrega-instalar.rc", "2\n"),
            ("entrega-instalar.log", "compilando\nerror final\n"),
        ]);
        let t = tarea_de("shell", &format!("{ENTREGA}\n{ESCENARIOS}\n{REQUISITOS}"));
        let r = sonda_entrega(&ej, repo.path(), &t);
        let no_instala = r
            .sonda
            .hallazgos
            .iter()
            .find(|h| h.detalle.contains("no instala"))
            .expect("hallazgo de no instala");
        assert_eq!(
            (no_instala.severidad, no_instala.cita.as_str()),
            (3, "error final")
        );
        assert_eq!(r.escenarios.values().filter(|v| **v).count(), 0);
        let no_corrio = r
            .sonda
            .hallazgos
            .iter()
            .filter(|h| h.detalle.contains("el producto no instala"))
            .count();
        assert_eq!(no_corrio, 2, "cada escenario que no corrió lo dice");
    }

    #[test]
    fn entrega_correr_que_muere_da_no_arranca_con_la_ultima_linea() {
        let repo = repo_completo();
        let ej = Falso::con(&[
            ("entrega-instalar.rc", "0\n"),
            ("entrega-correr.rc", "1\n"),
            ("entrega-correr.log", "arrancando\npuerto ocupado\n"),
        ]);
        let r = sonda_entrega(&ej, repo.path(), &tarea_de("shell", ENTREGA));
        let h = r
            .sonda
            .hallazgos
            .iter()
            .find(|h| h.detalle.contains("no arranca"))
            .expect("hallazgo de no arranca");
        assert_eq!((h.severidad, h.cita.as_str()), (3, "puerto ocupado"));
    }

    #[test]
    fn entrega_error_de_infraestructura_da_sin_veredicto_y_escenarios_vacios() {
        let repo = repo_completo();
        let t = tarea_de("shell", &format!("{ENTREGA}\n{ESCENARIOS}"));
        let r = sonda_entrega(&Falso::infra("podman caído"), repo.path(), &t);
        assert_eq!(r.sonda.valor, None);
        assert!(r.sonda.motivo.unwrap_or_default().contains("podman caído"));
        assert!(r.escenarios.is_empty(), "no inventa falsos");
    }

    #[test]
    fn entrega_valor_es_puntuar_de_los_hallazgos() {
        let repo = tempfile::tempdir().expect("temp dir");
        let r = sonda_entrega(&Falso::sano(), repo.path(), &tarea_de("shell", ENTREGA));
        let esperado = puntuar_hallazgos(&r.sonda.hallazgos, &[]);
        assert_eq!(r.sonda.valor, Some(esperado));
    }

    #[test]
    fn entrega_script_correr_escribe_el_log_completo_en_tmp_y_deja_la_cola() {
        let s = construir_script(&tarea_de("shell", ENTREGA));
        assert!(s.contains("> /tmp/entrega-correr.full.log 2>&1 &"));
        let cola = "tail -n 40 /tmp/entrega-correr.full.log > /salida/entrega-correr.log";
        let pos_cola = s.find(cola).expect("la cola de correr va a /salida");
        let pos_kill = s.find("kill -KILL").expect("mata correr");
        assert!(
            pos_kill < pos_cola,
            "la cola se escribe después de matar correr"
        );
        assert!(!s.contains("> /salida/entrega-correr.log 2>&1"));
    }

    #[test]
    fn entrega_script_instalar_y_escenarios_guardan_solo_la_cola() {
        let t = tarea_de("shell", &format!("{ENTREGA}\n{ESCENARIOS}"));
        let s = construir_script(&t);
        assert!(s.contains("| tail -n 40 > /salida/entrega-instalar.log"));
        assert!(s.contains("| tail -n 20 > /salida/entrega-escenario-a.log"));
        assert!(s.contains("| tail -n 20 > /salida/entrega-escenario-b.log"));
    }

    #[test]
    fn entrega_escenario_cita_sale_de_la_ultima_linea_de_su_log() {
        let repo = repo_completo();
        let ej = Falso::con(&[
            ("entrega-instalar.rc", "0\n"),
            ("entrega-correr.rc", "0\n"),
            ("entrega-escenario-a.rc", "0\n"),
            ("entrega-escenario-b.rc", "1\n"),
            ("entrega-escenario-b.log", "paso 1\nel login falló\n\n"),
        ]);
        let t = tarea_de("shell", &format!("{ENTREGA}\n{ESCENARIOS}\n{REQUISITOS}"));
        let r = sonda_entrega(&ej, repo.path(), &t);
        assert_eq!(r.sonda.hallazgos.len(), 1);
        let h = &r.sonda.hallazgos[0];
        assert_eq!(h.cita, "el login falló");
        assert_eq!(h.impacto, ["R1"]);
    }

    #[test]
    fn entrega_ultimo_paso_sale_del_ultimo_step_de_la_salida() {
        let salida = "STEP 1/3: FROM x\nSTEP 2/3: RUN a\nSTEP 3/3: RUN b\n\
            Error: building at STEP \"RUN b\": while running runtime: exit status 1";
        assert_eq!(paso_fallido(salida), Some(3));
        assert_eq!(
            paso_fallido("Error: creating build container: x: image not known"),
            None
        );
    }

    #[test]
    fn entrega_pide_red_reconoce_los_errores_de_apk_y_wget() {
        assert!(pide_red(
            "WARNING: fetching x: temporary error (try again later)"
        ));
        assert!(pide_red("wget: bad address 'example.com'"));
        assert!(pide_red("curl: (6) Could not resolve host: example.com"));
        assert!(!pide_red(
            "Error: building at STEP \"RUN false\": exit status 1"
        ));
    }
}
