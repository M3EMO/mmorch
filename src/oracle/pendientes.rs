//! Tests pendientes por entorno: el paquete para correrlos en Colab y la importación de su junit.
//! Root no lanza Colab. El usuario corre `correr.sh` y trae el junit de vuelta.
//! Todo lo que trae tests ocultos vive en `$ROOT_HOME/oraculos/<tarea>/` (A1, B18).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use fs4::{FileExt, TryLockError};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use super::descarga;
use super::junit::{self, ResultadoTest};
use super::requisitos::PREFIJO_CONTRATO;
use super::tarea::{Exclusion, Tarea};
use super::{ErrorOraculo, comillas, commit_de, copiar_repo, infra};
use super::{Muestreo, muestreo_de};
use crate::borrado;
use crate::observe;
use crate::tipos::{Pendiente, TipoEvento};

/// Carpeta de los tests ocultos de cada tarea dentro de `ROOT_HOME`. El builder no puede leerla.
const CARPETA_ORACULOS: &str = "oraculos";
/// Paquete de los tests pendientes, dentro de la carpeta de la tarea.
const CARPETA_PAQUETE: &str = "pendientes";
/// Carpeta de los tests del contrato: dentro de la tarea (`contrato/`) y dentro de `tests/` del paquete.
const CARPETA_CONTRATO: &str = "contrato";
/// Carpeta de los resultados importados dentro de `ROOT_HOME`.
const CARPETA_EVIDENCIA: &str = "evidencia";
/// Resultados de Colab de una tarea, dentro de su carpeta de evidencia.
const ARCHIVO_COLAB: &str = "colab.json";
/// Eventos de Root dentro de `ROOT_HOME`.
const ARCHIVO_EVENTOS: &str = "eventos.jsonl";
/// Junit que deja `correr.sh` por cada repetición, al lado del paquete: `pendientes-<n>.xml`.
pub(crate) fn nombre_junit(repeticion: u32) -> String {
    format!("pendientes-{repeticion}.xml")
}
/// Commit del repo del que se armó el paquete (B16).
pub(crate) const ARCHIVO_COMMIT: &str = "commit.txt";
/// Copia de la caché de dependencias dentro del paquete.
const CARPETA_CACHE_PAQUETE: &str = "cache";
/// Carpeta de temporales de `ROOT_HOME`, donde Colab baja el junit.
const CARPETA_TEMPORALES: &str = "tmp";
/// Prefijo de las carpetas de Colab dentro de `tmp/`.
const PREFIJO_COLAB: &str = "colab-";
/// Aviso del `Gate` y del veredicto cuando Colab no permitió `unshare -rn` (decisión 40).
pub const CONFIANZA_CON_RED: &str = "corrida de Colab con red: el oráculo pudo filtrarse";

/// Resultado de un test pendiente importado de Colab.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColabEntrada {
    /// "pasa", "falla" u "omite".
    pub resultado: String,
    /// Sha256 del junit del que salió el resultado.
    pub junit_sha256: String,
    /// Momento de la importación, en segundos desde la época.
    pub ts: f64,
    /// Commit del repo del que salió el paquete. Un resultado de otro commit no entra al veredicto (B16).
    #[serde(default)]
    pub commit: String,
    /// Tasa de aprobación si el test salió inestable entre repeticiones. Ausente en un test estable
    /// y en un `colab.json` viejo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tasa: Option<crate::tipos::TasaTest>,
    /// `CONFIANZA_CON_RED` si la corrida de esta entrada tuvo red (decisión 40). Ausente si no, y en un
    /// `colab.json` viejo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confianza: Option<String>,
}

/// Paquete armado para correr los tests pendientes.
#[derive(Debug, Clone, PartialEq)]
pub struct Paquete {
    pub dir: PathBuf,
    /// Ids de `[[entorno]]` de los tests del paquete: `archivo::nombre`, o `contrato:archivo::nombre`.
    pub tests: Vec<String>,
    /// Commit del repo del que se armó (vacío si no es git).
    pub commit: String,
    /// Avisos de armado: por ejemplo, que no hay caché de dependencias.
    pub avisos: Vec<String>,
}

/// Resumen de una importación de junit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Importacion {
    /// Tests de `[[entorno]]` que estaban en el junit.
    pub importados: usize,
    /// Tests del junit que no están en `[[entorno]]`.
    pub ignorados: usize,
    /// Avisos: un resultado de otro commit no se importa, por ejemplo.
    pub avisos: Vec<String>,
    /// Confianza de la corrida. `Some` si Colab corrió con red (decisión 40).
    pub confianza: Option<String>,
}

/// Dónde vive el paquete de una tarea: `ROOT_HOME/oraculos/<tarea>/pendientes`.
pub fn dir_paquete(home: &Path, tarea: &str) -> PathBuf {
    home.join(CARPETA_ORACULOS)
        .join(tarea)
        .join(CARPETA_PAQUETE)
}

/// Lock de una tarea, al lado de su paquete: `ROOT_HOME/oraculos/<tarea>/pendientes.lock`.
fn ruta_turno(home: &Path, tarea: &str) -> PathBuf {
    let mut ruta = dir_paquete(home, tarea).into_os_string();
    ruta.push(".lock");
    PathBuf::from(ruta)
}

/// Turno de una tarea. Mientras vive, ninguna otra corrida de `oraculo pendientes` de la misma tarea
/// arma el paquete ni corre Colab. Lo toma `tomar_turno` con un lock exclusivo de `fs4`, y se suelta al
/// soltarse el valor, también ante error. Si el proceso muere, el sistema operativo suelta el lock.
#[derive(Debug)]
pub struct Turno {
    archivo: fs::File,
}

impl Drop for Turno {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.archivo);
    }
}

/// Toma el turno de la tarea sin esperar. Si otra corrida lo tiene, da `ErrorOraculo::Ocupado` con el
/// pid y la hora de inicio de esa corrida, y sale con código 3. Si entra, anota su pid y su hora en el
/// lock para que la próxima corrida lo nombre.
pub fn tomar_turno(home: &Path, tarea: &str) -> Result<Turno, ErrorOraculo> {
    nombre_seguro(tarea)?;
    let ruta = ruta_turno(home, tarea);
    if let Some(padre) = ruta.parent() {
        fs::create_dir_all(padre).map_err(|e| infra("carpeta de la tarea", e))?;
    }
    let archivo = fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&ruta)
        .map_err(|e| infra(&format!("abrir {}", ruta.display()), e))?;
    match FileExt::try_lock(&archivo) {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => {
            return Err(ErrorOraculo::Ocupado(format!(
                "otra corrida de pendientes de la tarea {tarea} ya está en curso ({}). Espera a que termine.",
                dueno_del_turno(&ruta)
            )));
        }
        Err(TryLockError::Error(e)) => return Err(infra("tomar el turno de pendientes", e)),
    }
    let (_, desde) = observe::ahora();
    let dueno = format!("pid={}\ndesde={desde}\n", std::process::id());
    archivo
        .set_len(0)
        .map_err(|e| infra("anotar el dueño del turno", e))?;
    (&archivo)
        .write_all(dueno.as_bytes())
        .map_err(|e| infra("anotar el dueño del turno", e))?;
    Ok(Turno { archivo })
}

/// Lo que el lock dice de su dueño: el pid y la hora de inicio. Si el archivo aún no trae nada, lo dice.
fn dueno_del_turno(ruta: &Path) -> String {
    let texto = fs::read_to_string(ruta).unwrap_or_default();
    let campo = |clave: &str| {
        texto
            .lines()
            .find_map(|linea| linea.strip_prefix(clave))
            .map(str::to_string)
    };
    match (campo("pid="), campo("desde=")) {
        (Some(pid), Some(desde)) => format!("pid {pid}, desde {desde}"),
        _ => "dueño desconocido".into(),
    }
}

/// Las entradas de `[[entorno]]` que entran al paquete de Colab. Corren también las `contrato:` (desempate
/// 16). Sale el id sin prefijo que solo existe en `contrato/`: `crear` lo omite y avisa.
pub fn entorno_de_colab(tarea: &Tarea) -> impl Iterator<Item = &Exclusion> {
    tarea
        .entorno
        .iter()
        .filter(|e| !solo_en_contrato(tarea, &e.test))
}

/// Id que el junit trae para una entrada de `[[entorno]]`: sin el prefijo `contrato:`. Es la clave que
/// `Junit::con_ids` da a ese test. `colab.json` guarda la entrada con su prefijo.
pub fn id_de_junit(entrada: &str) -> &str {
    entrada.strip_prefix(PREFIJO_CONTRATO).unwrap_or(entrada)
}

/// Un id sin prefijo cuyo archivo existe solo en `contrato/`. Ese test no entra al paquete (desempate 16,
/// punto 2). Un id sin la forma `archivo::nombre` o sin archivo en ninguna suite da `false`: `crear` lo
/// rechaza como error de tarea.
fn solo_en_contrato(tarea: &Tarea, id: &str) -> bool {
    if id.starts_with(PREFIJO_CONTRATO) {
        return false;
    }
    let Ok((archivo, _)) = dividir(id) else {
        return false;
    };
    if nombre_seguro(archivo).is_err() {
        return false;
    }
    let py = format!("{archivo}.py");
    !tarea.dir.join("oculto").join(&py).is_file()
        && tarea.dir.join(CARPETA_CONTRATO).join(&py).is_file()
}

/// Un aviso por cada id sin prefijo que solo existe en `contrato/`: el paquete no lo lleva.
fn avisos_de_contrato(tarea: &Tarea) -> Vec<String> {
    tarea
        .entorno
        .iter()
        .filter(|e| solo_en_contrato(tarea, &e.test))
        .map(|e| {
            format!(
                "el test {} sin prefijo existe solo en {CARPETA_CONTRATO}/: no entra al paquete; usá {PREFIJO_CONTRATO}{}",
                e.test, e.test
            )
        })
        .collect()
}

/// Arma el paquete de los tests de `[[entorno]]` dentro de `oraculos/<tarea>/pendientes/`. Corren los
/// ocultos (`tests/<archivo>.py`) y los `contrato:` (`tests/contrato/<archivo>.py`). Un id sin prefijo que
/// solo existe en `contrato/` no entra y da un aviso (desempate 16).
/// `cache` es la caché de dependencias de la fase de descarga, si ya existe: el paquete la lleva y
/// `correr.sh` la usa sin red. Sin tests pendientes no crea nada y da `None`.
/// Todo se valida antes de crear el directorio.
pub fn crear(
    home: &Path,
    tarea: &Tarea,
    repo: &Path,
    cache: Option<&Path>,
) -> Result<Option<Paquete>, ErrorOraculo> {
    nombre_de_tarea_valido(&tarea.nombre).map_err(ErrorOraculo::Tarea)?;
    if entorno_de_colab(tarea).next().is_none() {
        return Ok(None);
    }
    let repo = fs::canonicalize(repo)
        .map_err(|e| ErrorOraculo::Infra(format!("no existe el repo {}: {e}", repo.display())))?;
    // Destino dentro de `tests/` -> archivo de origen.
    let mut archivos: BTreeMap<PathBuf, PathBuf> = BTreeMap::new();
    // Archivo del módulo -> si es del contrato. Un mismo archivo en las dos suites no cabe: pytest y el
    // junit identifican el test solo por el módulo, sin la carpeta.
    let mut suite_de: BTreeMap<String, bool> = BTreeMap::new();
    let mut nodos = Vec::new();
    let mut tests = Vec::new();
    for exclusion in entorno_de_colab(tarea) {
        let del_contrato = exclusion.test.starts_with(PREFIJO_CONTRATO);
        let (archivo, _) = dividir(id_de_junit(&exclusion.test))?;
        match suite_de.get(archivo) {
            Some(previo) if *previo != del_contrato => {
                return Err(ErrorOraculo::Tarea(format!(
                    "{archivo}.py está en oculto/ y en contrato/ a la vez: el junit solo trae el módulo y sus tests se confunden. Renombrá uno de los dos archivos"
                )));
            }
            Some(_) => {}
            None => {
                suite_de.insert(archivo.to_string(), del_contrato);
                let destino = if del_contrato {
                    Path::new(CARPETA_CONTRATO).join(format!("{archivo}.py"))
                } else {
                    PathBuf::from(format!("{archivo}.py"))
                };
                archivos.insert(destino, localizar(tarea, archivo, del_contrato)?);
            }
        }
        nodos.push(nodo(&exclusion.test)?);
        tests.push(exclusion.test.clone());
    }
    let hay_contrato = suite_de.values().any(|del_contrato| *del_contrato);
    let mut avisos = avisos_de_contrato(tarea);

    // Un paquete viejo de esta tarea (de una corrida que no llegó a limpiarse) se borra antes.
    let dir = dir_paquete(home, &tarea.nombre);
    let raiz_tarea = home.join(CARPETA_ORACULOS).join(&tarea.nombre);
    borrado::borrar_dir_dentro(&raiz_tarea, &dir)
        .map_err(|e| infra("borrar el paquete viejo", e))?;
    fs::create_dir_all(&raiz_tarea).map_err(|e| infra("carpeta de la tarea", e))?;
    fs::create_dir(&dir).map_err(|e| infra(&format!("crear {}", dir.display()), e))?;

    copiar_repo(&repo, &dir.join("repo")).map_err(|e| infra("copia del repo", e))?;
    let tests_dir = dir.join("tests");
    fs::create_dir(&tests_dir).map_err(|e| infra("carpeta tests", e))?;
    for (destino, origen) in &archivos {
        let ruta = tests_dir.join(destino);
        if let Some(padre) = ruta.parent() {
            fs::create_dir_all(padre).map_err(|e| infra("carpeta del contrato", e))?;
        }
        fs::copy(origen, &ruta).map_err(|e| infra(&format!("copiar {}", origen.display()), e))?;
    }
    let conftest = tarea.dir.join("oculto").join("conftest.py");
    if conftest.is_file() {
        fs::copy(&conftest, tests_dir.join("conftest.py"))
            .map_err(|e| infra("copiar conftest", e))?;
    }
    // La carpeta `tests/contrato/` ya existe si hay un test del contrato en el paquete.
    let conftest_contrato = tarea.dir.join(CARPETA_CONTRATO).join("conftest.py");
    if hay_contrato && conftest_contrato.is_file() {
        fs::copy(
            &conftest_contrato,
            tests_dir.join(CARPETA_CONTRATO).join("conftest.py"),
        )
        .map_err(|e| infra("copiar conftest del contrato", e))?;
    }
    let commit = commit_de(&repo);
    escribir(&dir.join(ARCHIVO_COMMIT), &commit, false)?;
    let con_cache = match cache {
        Some(origen) => {
            copiar_repo(origen, &dir.join(CARPETA_CACHE_PAQUETE))
                .map_err(|e| infra("copia de la caché", e))?;
            true
        }
        None => {
            avisos.push(
                "sin caché de dependencias: corré `r00t oraculo evaluar` antes para bajarlas"
                    .into(),
            );
            false
        }
    };
    let guion = if con_cache {
        guion_con_cache(&nodos, tarea.repeticiones, &tarea.lenguaje)
    } else {
        guion_correr(&nodos, tarea.repeticiones, "")
    };
    escribir(&dir.join("correr.sh"), &guion, true)?;
    escribir(&dir.join("LEEME.txt"), &leeme(&tarea.nombre, &repo), false)?;
    Ok(Some(Paquete {
        dir,
        tests,
        commit,
        avisos,
    }))
}

/// Borra lo que Colab dejó de una corrida: el paquete con los tests ocultos y `tmp/colab-<sesion>`
/// con el junit bajado (A1, B18). Cada borrado pasa por `borrado` con su raíz.
pub fn limpiar_paquete(home: &Path, tarea: &Tarea, paquete: &Path, sesion: &str) -> io::Result<()> {
    let raiz_tarea = home.join(CARPETA_ORACULOS).join(&tarea.nombre);
    borrado::borrar_dir_dentro(&raiz_tarea, paquete)?;
    let tmp = home.join(CARPETA_TEMPORALES);
    borrado::borrar_dir_dentro(&tmp, &tmp.join(format!("{PREFIJO_COLAB}{sesion}")))
}

/// Importa los junit que dejó `correr.sh`, uno por repetición. `commit` es el del repo al que se
/// importa. Si el paquete de la tarea tiene otro commit, el resultado no se importa y el aviso lo dice
/// (B16). Si entra, cada test pendiente cuenta una muestra por junit, con el mismo criterio que la
/// laptop (`muestreo_de`): un test que pasa en una repetición y falla en otra queda `inestable` con su
/// tasa, nunca `falla` (desempate 28). Un `contrato:` se busca en el junit sin el prefijo, y su resultado
/// se guarda con él (desempate 16). Guarda el resultado en `evidencia/<tarea>/colab.json` y registra un
/// evento `Gate`.
pub fn importar(
    home: &Path,
    tarea: &Tarea,
    rutas: &[PathBuf],
    commit: &str,
) -> Result<Importacion, ErrorOraculo> {
    if rutas.is_empty() {
        return Err(ErrorOraculo::Uso("falta el junit a importar".into()));
    }
    let mut textos = Vec::with_capacity(rutas.len());
    for ruta in rutas {
        let texto = fs::read_to_string(ruta)
            .map_err(|e| ErrorOraculo::Infra(format!("no se pudo leer {}: {e}", ruta.display())))?;
        textos.push(texto);
    }
    let junits = textos
        .iter()
        .map(|texto| junit::parsear(texto).map(|j| j.con_ids()))
        .collect::<Result<Vec<_>, _>>()?;
    let distintos: BTreeSet<&String> = junits.iter().flat_map(|j| j.tests.keys()).collect();
    if let Some(del_paquete) = commit_del_paquete(home, &tarea.nombre)
        && del_paquete != commit
    {
        return Ok(Importacion {
            importados: 0,
            ignorados: distintos.len(),
            avisos: vec![format!(
                "el junit es del commit {del_paquete} y el repo está en {commit}: no se importa"
            )],
            confianza: None,
        });
    }
    let sha = sha256_hex(textos.concat().as_bytes());
    let (ts, _) = observe::ahora();
    // Decisión 40: si `unshare -rn` no funcionó en Colab, la corrida tuvo red y su confianza lo dice.
    let con_red = junits.iter().any(|j| j.red.as_deref() == Some("con-red"));
    let confianza = con_red.then(|| CONFIANZA_CON_RED.to_string());

    let mut colab = colab_de(home, &tarea.nombre)?;
    let mut resultados = BTreeMap::new();
    for exclusion in entorno_de_colab(tarea) {
        let muestras: Vec<Option<ResultadoTest>> = junits
            .iter()
            .map(|j| j.tests.get(id_de_junit(&exclusion.test)).copied())
            .collect();
        let Some(muestreo) = muestreo_de(&muestras) else {
            continue;
        };
        let (resultado, tasa) = match muestreo {
            Muestreo::Estable(r) => (etiqueta(r), None),
            Muestreo::Inestable(tasa) => ("inestable", Some(tasa)),
        };
        colab.insert(
            exclusion.test.clone(),
            ColabEntrada {
                resultado: resultado.into(),
                junit_sha256: sha.clone(),
                ts,
                commit: commit.to_string(),
                tasa,
                confianza: confianza.clone(),
            },
        );
        resultados.insert(exclusion.test.clone(), resultado);
    }
    let importados = resultados.len();
    let ignorados = distintos.len().saturating_sub(importados);
    guardar_colab(home, &tarea.nombre, &colab)?;

    let mut evento = observe::evento_manual(
        &format!("pendientes-{}", tarea.nombre),
        TipoEvento::Gate,
        None,
    );
    evento.fase = "pendientes".into();
    evento.extra.insert("origen".into(), json!("colab"));
    evento.extra.insert("tests".into(), json!(resultados));
    evento.extra.insert("junit_sha256".into(), json!(sha));
    evento
        .extra
        .insert("repeticiones".into(), json!(rutas.len()));
    if let Some(texto) = &confianza {
        evento.extra.insert("confianza".into(), json!(texto));
    }
    observe::registrar_en(&home.join(ARCHIVO_EVENTOS), &evento)
        .map_err(|e| infra("registrar el evento", e))?;
    Ok(Importacion {
        importados,
        ignorados,
        avisos: Vec::new(),
        confianza,
    })
}

/// Aviso del veredicto si alguna entrada de Colab trae la marca de red (decisión 40). Da uno solo, aunque
/// haya varias entradas marcadas.
pub fn aviso_de_red(colab: &BTreeMap<String, ColabEntrada>) -> Option<String> {
    colab
        .values()
        .any(|entrada| entrada.confianza.is_some())
        .then(|| CONFIANZA_CON_RED.to_string())
}

/// Commit con el que se armó el paquete de la tarea. `None` si no hay paquete.
fn commit_del_paquete(home: &Path, tarea: &str) -> Option<String> {
    let ruta = dir_paquete(home, tarea).join(ARCHIVO_COMMIT);
    fs::read_to_string(ruta).ok().map(|c| c.trim().to_string())
}

/// Resultados de Colab guardados para una tarea. Sin archivo, vacío.
pub fn colab_de(home: &Path, tarea: &str) -> Result<BTreeMap<String, ColabEntrada>, ErrorOraculo> {
    let ruta = ruta_colab(home, tarea)?;
    match fs::read_to_string(&ruta) {
        Ok(texto) => serde_json::from_str(&texto)
            .map_err(|e| ErrorOraculo::Infra(format!("{}: {e}", ruta.display()))),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(e) => Err(infra(&format!("leer {}", ruta.display()), e)),
    }
}

/// Resultados de Colab para el veredicto. Un `colab.json` ilegible no tumba la evaluación:
/// sus tests quedan pendientes y el aviso va al veredicto. Un nombre de tarea inválido sigue siendo error.
pub fn colab_para_veredicto(
    home: &Path,
    tarea: &str,
) -> Result<(BTreeMap<String, ColabEntrada>, Vec<String>), ErrorOraculo> {
    let ruta = ruta_colab(home, tarea)?;
    let leido: Result<BTreeMap<String, ColabEntrada>, String> = match fs::read_to_string(&ruta) {
        Ok(texto) => serde_json::from_str(&texto).map_err(|e| e.to_string()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Ok((BTreeMap::new(), Vec::new()));
        }
        Err(e) => Err(e.to_string()),
    };
    match leido {
        Ok(colab) => Ok((colab, Vec::new())),
        Err(causa) => Ok((
            BTreeMap::new(),
            vec![format!("{ARCHIVO_COLAB} ilegible: {causa}")],
        )),
    }
}

/// Los tests de `[[entorno]]` con su motivo y, si hay, su resultado de Colab. El resultado se busca con la
/// entrada completa, prefijo `contrato:` incluido, igual que se guardó en `importar`.
pub fn desde(tarea: &Tarea, colab: &BTreeMap<String, ColabEntrada>) -> Vec<Pendiente> {
    tarea
        .entorno
        .iter()
        .map(|exclusion| Pendiente {
            test: exclusion.test.clone(),
            motivo: exclusion.motivo.clone(),
            colab: colab.get(&exclusion.test).map(|c| c.resultado.clone()),
            tasa: colab.get(&exclusion.test).and_then(|c| c.tasa.clone()),
        })
        .collect()
}

fn guardar_colab(
    home: &Path,
    tarea: &str,
    colab: &BTreeMap<String, ColabEntrada>,
) -> Result<(), ErrorOraculo> {
    let ruta = ruta_colab(home, tarea)?;
    if let Some(padre) = ruta.parent() {
        fs::create_dir_all(padre).map_err(|e| infra("carpeta de evidencia", e))?;
    }
    let texto = serde_json::to_string_pretty(colab)
        .map_err(|e| ErrorOraculo::Infra(format!("serializar colab.json: {e}")))?;
    fs::write(&ruta, texto).map_err(|e| infra(&format!("escribir {}", ruta.display()), e))
}

fn ruta_colab(home: &Path, tarea: &str) -> Result<PathBuf, ErrorOraculo> {
    nombre_seguro(tarea)?;
    Ok(home.join(CARPETA_EVIDENCIA).join(tarea).join(ARCHIVO_COLAB))
}

/// Un nombre usado como carpeta no puede traer rutas.
fn nombre_seguro(nombre: &str) -> Result<(), ErrorOraculo> {
    nombre_de_tarea_valido(nombre).map_err(ErrorOraculo::Tarea)
}

/// Nombre de tarea que puede ser una carpeta: sin `/`, `\`, `.` ni `..` (B7). Lo usan el CLI y `crear`.
pub fn nombre_de_tarea_valido(nombre: &str) -> Result<(), String> {
    let con_ruta =
        nombre.is_empty() || nombre.contains(['/', '\\']) || nombre == "." || nombre == "..";
    if con_ruta {
        return Err(format!("nombre inválido: {nombre:?}"));
    }
    Ok(())
}

/// Parte `archivo::nombre`. Sin separador es error de tarea.
fn dividir(id: &str) -> Result<(&str, &str), ErrorOraculo> {
    id.split_once("::").ok_or_else(|| {
        ErrorOraculo::Tarea(format!("el id {id:?} no tiene la forma archivo::nombre"))
    })
}

/// Node id de pytest dentro de la carpeta `tests/` del paquete: `tests/<archivo>.py::<nombre>`. Con el
/// prefijo `contrato:`, va en `tests/contrato/` (desempate 16).
fn nodo(id: &str) -> Result<String, ErrorOraculo> {
    let (archivo, nombre) = dividir(id_de_junit(id))?;
    nombre_seguro(archivo)?;
    let carpeta = if id.starts_with(PREFIJO_CONTRATO) {
        format!("{CARPETA_CONTRATO}/")
    } else {
        String::new()
    };
    Ok(format!("tests/{carpeta}{archivo}.py::{nombre}"))
}

/// Busca el archivo del módulo: en `contrato/` si el id lleva `contrato:`, y en `oculto/` si no. Un id sin
/// prefijo llega aquí solo si su archivo no está en `contrato/` (si estuviera, la entrada se omitió).
fn localizar(tarea: &Tarea, archivo: &str, del_contrato: bool) -> Result<PathBuf, ErrorOraculo> {
    nombre_seguro(archivo)?;
    let (suite, donde) = if del_contrato {
        (CARPETA_CONTRATO, "contrato/")
    } else {
        ("oculto", "oculto/ ni en contrato/")
    };
    let ruta = tarea.dir.join(suite).join(format!("{archivo}.py"));
    if ruta.is_file() {
        return Ok(ruta);
    }
    Err(ErrorOraculo::Tarea(format!(
        "no existe {archivo}.py en {donde}"
    )))
}

/// Marca cada testsuite de un junit con la marca de red de `red.txt` (decisión 40), como atributo
/// `root_red`. Recibe las rutas de los junit por argumento. Sin `red.txt`, la marca dice `desconocido`.
const GUION_MARCAR: &str = r#"import os, sys
import xml.etree.ElementTree as ET
marca = open("red.txt").read().strip() if os.path.exists("red.txt") else "desconocido"
for ruta in sys.argv[1:]:
    arbol = ET.parse(ruta)
    for suite in arbol.getroot().iter("testsuite"):
        suite.set("root_red", marca)
    arbol.write(ruta, encoding="utf-8", xml_declaration=True)
"#;

/// Script que corre los nodos pendientes `repeticiones` veces. Cada repetición deja su junit,
/// `pendientes-<n>.xml`, y Root lo baja y lo importa como una muestra (desempate 28). Los tests corren
/// dentro de `unshare -rn` (sin red, decisión 40). Si Colab no deja crear ese namespace, la corrida sigue
/// con red y `red.txt` dice `con-red`. `prelude` prepara la caché, si la hay, y va antes de `pytest` en
/// la misma línea. Si hay una carpeta `bin/` al lado (el `cargo` mínimo del respaldo de Colab), la pone
/// al frente del `PATH`.
fn guion_correr(nodos: &[String], repeticiones: u32, prelude: &str) -> String {
    guion_con_previo(nodos, repeticiones, "", prelude)
}

/// Guion de un paquete con caché: `ROOT_CACHE` apunta a `cache/` del paquete, y su `config.toml` (y el
/// `--find-links`) se reescriben a esa ruta, porque la caché vino con rutas `/cache` del contenedor.
fn guion_con_cache(nodos: &[String], repeticiones: u32, lenguaje: &str) -> String {
    let previo = "ROOT_CACHE=\"$(pwd)/cache\"\n";
    guion_con_previo(nodos, repeticiones, previo, &preludio_de(lenguaje))
}

/// Preludio de la suite para el lenguaje, adaptado a la caché del paquete (`$ROOT_CACHE`). El
/// `config.toml` de cargo se reescribe a `CARGO_HOME` desde la copia intacta de la caché: así correr
/// `correr.sh` otra vez no vuelve a reescribir rutas ya reescritas.
fn preludio_de(lenguaje: &str) -> String {
    match lenguaje {
        "rust" => "export CARGO_NET_OFFLINE=true && export CARGO_HOME=/tmp/cargo-home && \
             mkdir -p \"$CARGO_HOME\" && sed \"s#/cache#$ROOT_CACHE#g\" \"$ROOT_CACHE/config.toml\" \
             > \"$CARGO_HOME/config.toml\" && "
            .to_string(),
        "python" => descarga::PRELUDIO_PYTHON.replace("/cache", "\"$ROOT_CACHE\""),
        _ => String::new(),
    }
}

fn guion_con_previo(nodos: &[String], repeticiones: u32, previo: &str, prelude: &str) -> String {
    let lista: Vec<String> = nodos.iter().map(|n| comillas(n)).collect();
    format!(
        "#!/usr/bin/env bash\n\
         set -euo pipefail\n\
         cd \"$(dirname \"$0\")\"\n\
         if [ -d bin ]; then export PATH=\"$(pwd)/bin:$PATH\"; fi\n\
         export ORACULO_REPO=\"$(pwd)/repo\"\n\
         export CARGO_TARGET_DIR=\"$(pwd)/target\"\n\
         {previo}\
         if unshare -rn true 2>/dev/null; then ORDEN=\"unshare -rn\"; RED=sin-red; else ORDEN=\"\"; RED=con-red; fi\n\
         echo \"$RED\" > red.txt\n\
         estado=0\n\
         for i in $(seq 1 {repeticiones}); do\n\
         {prelude}$ORDEN python3 -m pytest -q -p no:cacheprovider --junitxml=pendientes-$i.xml {nodos} || estado=$?\n\
         if [ -f pendientes-$i.xml ]; then\n\
         python3 -I - pendientes-$i.xml <<'PY' || true\n\
         {marcar}\
         PY\n\
         fi\n\
         done\n\
         exit $estado\n",
        nodos = lista.join(" "),
        marcar = GUION_MARCAR,
    )
}

/// Cinco líneas: qué hacer en Colab y cómo importar el resultado.
fn leeme(tarea: &str, repo: &Path) -> String {
    format!(
        "Tests pendientes de la tarea {tarea}, generados por Root.\n\
         1. Instala Rust (cargo) y pytest (pip install pytest).\n\
         2. Corre ./correr.sh desde esta carpeta.\n\
         3. Lleva todos los pendientes-N.xml (uno por repetición) a la máquina donde corre Root.\n\
         4. Corre: r00t oraculo pendientes --tarea {tarea} --repo {} --importar pendientes-*.xml\n",
        repo.display()
    )
}

fn escribir(ruta: &Path, texto: &str, ejecutable: bool) -> Result<(), ErrorOraculo> {
    fs::write(ruta, texto).map_err(|e| infra(&format!("escribir {}", ruta.display()), e))?;
    if ejecutable {
        fs::set_permissions(ruta, fs::Permissions::from_mode(0o755))
            .map_err(|e| infra("permisos de correr.sh", e))?;
    }
    Ok(())
}

fn sha256_hex(contenido: &[u8]) -> String {
    Sha256::digest(contenido)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn etiqueta(resultado: ResultadoTest) -> &'static str {
    match resultado {
        ResultadoTest::Pasa => "pasa",
        ResultadoTest::Falla => "falla",
        ResultadoTest::Omite => "omite",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oracle::tarea::{Comando, Exclusion};
    use std::process::Command;

    #[test]
    fn pendiente_etiqueta_de_resultado_en_minusculas() {
        assert_eq!(etiqueta(ResultadoTest::Pasa), "pasa");
        assert_eq!(etiqueta(ResultadoTest::Falla), "falla");
        assert_eq!(etiqueta(ResultadoTest::Omite), "omite");
    }

    #[test]
    fn pendiente_nodo_entrecomillado_conserva_corchetes() {
        let id = "test_x::test_rendimiento[gpu-10000-res1]";
        let nodo_pytest = nodo(id).expect("id válido");
        assert_eq!(
            comillas(&nodo_pytest),
            "'tests/test_x.py::test_rendimiento[gpu-10000-res1]'"
        );
    }

    #[test]
    fn pendiente_crear_no_escribe_fuera_de_home_con_nombre_de_tarea_con_ruta() {
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        let tarea_dir = tempfile::tempdir().expect("tarea");
        let texto = "nombre = \"../../fuera\"\nlenguaje = \"rust\"\nimagen = \"i\"\nspec = \"s\"\n\
                     [contrato]\ncmd = \"c\"\n[oculto]\ncmd = \"o\"\n\
                     [[entorno]]\ntest = \"a.py::t\"\nmotivo = \"m\"\n";
        std::fs::write(tarea_dir.path().join("tarea.toml"), texto).expect("escribe");
        let tarea = Tarea::cargar_de(tarea_dir.path()).expect("carga la tarea");
        let r = crear(home.path(), &tarea, repo.path(), None);
        let motivo = r.err().map(|e| e.to_string()).unwrap_or_default();
        assert!(motivo.contains("nombre inválido"), "{motivo}");
        assert!(!home.path().join("oraculos").exists());
    }

    #[test]
    fn pendiente_crear_suma_al_paquete_una_entrada_del_contrato() {
        // Desempate 16: el paquete de Colab corre también los `contrato:` de `[[entorno]]`.
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        fs::write(repo.path().join("a.txt"), "x").expect("repo");
        let tarea = tarea_con_entorno(
            home.path(),
            "tarea-x",
            &["contrato:test_c::t"],
            &[("contrato", "test_c")],
        );
        let paquete = crear(home.path(), &tarea, repo.path(), None)
            .expect("crea")
            .expect("hay paquete");
        assert_eq!(paquete.tests, ["contrato:test_c::t"]);
        assert!(
            paquete.dir.join("tests/contrato/test_c.py").is_file(),
            "el archivo del contrato va a tests/contrato/"
        );
        assert!(!paquete.dir.join("tests/test_c.py").exists());
        let sh = fs::read_to_string(paquete.dir.join("correr.sh")).expect("correr.sh");
        assert!(sh.contains("'tests/contrato/test_c.py::t'"), "{sh}");
    }

    #[test]
    fn pendiente_id_sin_prefijo_solo_en_contrato_no_entra_y_avisa() {
        // Desempate 16, punto 2: `test_c::t` sin prefijo busca solo en oculto/. Si está en contrato/,
        // no entra al paquete y el aviso lo nombra.
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        fs::write(repo.path().join("a.txt"), "x").expect("repo");
        let tarea = tarea_con_entorno(
            home.path(),
            "tarea-x",
            &["test_c::t", "test_a::uno"],
            &[("contrato", "test_c"), ("oculto", "test_a")],
        );
        let paquete = crear(home.path(), &tarea, repo.path(), None)
            .expect("crea")
            .expect("hay paquete");
        assert_eq!(paquete.tests, ["test_a::uno"]);
        assert!(
            paquete
                .avisos
                .iter()
                .any(|a| a.contains("test_c::t") && a.contains("contrato:test_c::t")),
            "{:?}",
            paquete.avisos
        );
        assert!(!paquete.dir.join("tests/contrato").exists());
        assert!(!paquete.dir.join("tests/test_c.py").exists());
    }

    #[test]
    fn pendiente_mismo_archivo_en_las_dos_suites_es_error_de_tarea() {
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        fs::write(repo.path().join("a.txt"), "x").expect("repo");
        let tarea = tarea_con_entorno(
            home.path(),
            "tarea-x",
            &["test_a::uno", "contrato:test_a::t"],
            &[("oculto", "test_a"), ("contrato", "test_a")],
        );
        let r = crear(home.path(), &tarea, repo.path(), None);
        let motivo = r.as_ref().err().map(|e| e.to_string()).unwrap_or_default();
        assert!(matches!(r, Err(ErrorOraculo::Tarea(_))), "{motivo}");
        assert!(motivo.contains("test_a"), "{motivo}");
        assert!(
            !dir_paquete(home.path(), "tarea-x").exists(),
            "no crea nada antes del error"
        );
    }

    #[test]
    fn pendiente_importar_guarda_el_resultado_del_contrato_con_su_prefijo() {
        let home = tempfile::tempdir().expect("home");
        let tarea = tarea_con_entorno(
            home.path(),
            "tarea-x",
            &["contrato:test_c::t"],
            &[("contrato", "test_c")],
        );
        let junit = home.path().join("pendientes-1.xml");
        fs::write(
            &junit,
            r#"<testsuite><testcase classname="tests.contrato.test_c" name="t"/></testsuite>"#,
        )
        .expect("junit");
        let resumen = importar(home.path(), &tarea, &[junit], "").expect("importa");
        assert_eq!(resumen.importados, 1);
        let colab = colab_de(home.path(), "tarea-x").expect("lee");
        assert_eq!(colab["contrato:test_c::t"].resultado, "pasa");
        assert!(
            !colab.contains_key("test_c::t"),
            "la clave lleva el prefijo: {:?}",
            colab.keys()
        );
    }

    #[test]
    fn pendiente_id_sin_separador_es_error_de_tarea() {
        assert!(matches!(nodo("sin_separador"), Err(ErrorOraculo::Tarea(_))));
    }

    #[test]
    fn pendiente_guion_correr_tiene_exports_pytest_y_los_nodos() {
        let nodos = vec!["tests/test_x.py::test_rendimiento[gpu-10000-res1]".to_string()];
        let texto = guion_correr(&nodos, 1, "");
        assert!(texto.starts_with("#!/usr/bin/env bash\n"));
        assert!(texto.contains("set -euo pipefail"));
        assert!(texto.contains("export ORACULO_REPO=\"$(pwd)/repo\""));
        assert!(texto.contains("export CARGO_TARGET_DIR=\"$(pwd)/target\""));
        assert!(
            texto.contains(
                "python3 -m pytest -q -p no:cacheprovider --junitxml=pendientes-$i.xml 'tests/test_x.py::test_rendimiento[gpu-10000-res1]'"
            ),
            "{texto}"
        );
    }

    #[test]
    fn pendiente_guion_correr_escribe_un_junit_por_repeticion() {
        let nodos = vec!["tests/test_x.py::test_a".to_string()];
        let texto = guion_correr(&nodos, 3, "");
        assert!(texto.contains("for i in $(seq 1 3); do"), "{texto}");
        assert!(texto.contains("--junitxml=pendientes-$i.xml"), "{texto}");
        assert!(texto.contains("|| estado=$?"), "{texto}");
        assert!(texto.contains("exit $estado"), "{texto}");
        assert!(
            !texto.contains("pendientes.xml"),
            "el guion no junta los junit en uno: {texto}"
        );
    }

    #[test]
    fn pendiente_marca_de_red_entra_en_cada_junit_por_repeticion() {
        let base = tempfile::tempdir().expect("base");
        fs::write(base.path().join("red.txt"), "con-red\n").expect("red");
        fs::write(
            base.path().join("pendientes-1.xml"),
            r#"<testsuites><testsuite name="pytest"><testcase classname="tests.test_a" name="t1"/></testsuite></testsuites>"#,
        )
        .expect("rep 1");
        fs::write(
            base.path().join("pendientes-2.xml"),
            r#"<testsuite><testcase classname="tests.test_a" name="t1"/></testsuite>"#,
        )
        .expect("rep 2");
        let marcar = base.path().join("marcar.py");
        fs::write(&marcar, GUION_MARCAR).expect("guion");
        let estado = Command::new("python3")
            .arg("-I")
            .arg(&marcar)
            .arg("pendientes-1.xml")
            .arg("pendientes-2.xml")
            .current_dir(base.path())
            .status()
            .expect("python3");
        assert!(estado.success(), "{estado:?}");
        for n in [1, 2] {
            let texto =
                fs::read_to_string(base.path().join(format!("pendientes-{n}.xml"))).expect("junit");
            let leido = crate::oracle::junit::parsear(&texto).expect("parsea");
            assert_eq!(leido.red.as_deref(), Some("con-red"), "repetición {n}");
            assert_eq!(leido.testcases["tests.test_a::t1"], 1, "repetición {n}");
        }
    }

    #[test]
    fn pendiente_guion_correr_agrega_bin_al_path_si_existe() {
        let texto = guion_correr(&[], 1, "");
        assert!(
            texto.contains("if [ -d bin ]; then export PATH=\"$(pwd)/bin:$PATH\"; fi"),
            "{texto}"
        );
    }

    #[test]
    fn pendiente_nombre_de_tarea_con_ruta_se_rechaza() {
        for nombre in ["", ".", "..", "../otra", "a/b", "a\\b"] {
            assert!(
                matches!(
                    ruta_colab(Path::new("/h"), nombre),
                    Err(ErrorOraculo::Tarea(_))
                ),
                "debe rechazar {nombre:?}"
            );
        }
        assert!(ruta_colab(Path::new("/h"), "memora").is_ok());
    }

    #[test]
    fn pendiente_desde_marca_el_resultado_de_colab_por_test() {
        let tarea = Tarea {
            nombre: "memora".into(),
            lenguaje: "rust".into(),
            imagen: "x".into(),
            containerfile: None,
            spec: "s".into(),
            repeticiones: 1,
            repeticiones_inestable: 10,
            contrato: Comando {
                cmd: "x".into(),
                reintento: None,
            },
            oculto: Comando {
                cmd: "x".into(),
                reintento: None,
            },
            entorno: vec![
                Exclusion {
                    test: "test_a::uno".into(),
                    motivo: "gpu".into(),
                },
                Exclusion {
                    test: "test_a::dos".into(),
                    motivo: "gpu".into(),
                },
            ],
            oraculo_dudoso: Vec::new(),
            requisito: Vec::new(),
            arquitectura: None,
            eficiencia: None,
            pruebas: None,
            entrega: None,
            escenario: Vec::new(),
            calibracion: None,
            dir: PathBuf::new(),
        };
        let mut colab = BTreeMap::new();
        colab.insert(
            "test_a::uno".to_string(),
            ColabEntrada {
                resultado: "pasa".into(),
                junit_sha256: "abc".into(),
                ts: 1.0,
                commit: String::new(),
                tasa: None,
                confianza: None,
            },
        );
        let pendientes = desde(&tarea, &colab);
        assert_eq!(pendientes.len(), 2);
        assert_eq!(pendientes[0].colab.as_deref(), Some("pasa"));
        assert_eq!(pendientes[1].colab, None);
        assert_eq!(pendientes[1].motivo, "gpu");
    }

    #[test]
    fn pendiente_desde_pasa_la_tasa_de_un_test_inestable() {
        let home = tempfile::tempdir().expect("home");
        let tarea = tarea_en_oraculos(home.path(), "tarea-x", "shell");
        let mut colab = BTreeMap::new();
        colab.insert(
            "test_a::uno".to_string(),
            ColabEntrada {
                resultado: "inestable".into(),
                junit_sha256: "abc".into(),
                ts: 1.0,
                commit: String::new(),
                tasa: Some(crate::tipos::TasaTest {
                    pasa: 2,
                    total: 4,
                    wilson_inf: 0.1,
                }),
                confianza: None,
            },
        );
        let pendientes = desde(&tarea, &colab);
        assert_eq!(pendientes[0].colab.as_deref(), Some("inestable"));
        assert_eq!(pendientes[0].tasa.as_ref().map(|t| t.total), Some(4));
    }
    /// Tarea mínima en `home/oraculos/<nombre>`, con un test de `[[entorno]]` y su archivo oculto.
    fn tarea_en_oraculos(home: &Path, nombre: &str, lenguaje: &str) -> Tarea {
        let dir = home.join("oraculos").join(nombre);
        fs::create_dir_all(dir.join("oculto")).expect("oculto");
        fs::write(
            dir.join("oculto/test_a.py"),
            "def test_uno(): pass  # SECRETO-OCULTO\n",
        )
        .expect("escribe");
        let texto = format!(
            "nombre = \"{nombre}\"\nlenguaje = \"{lenguaje}\"\nimagen = \"i\"\nspec = \"s\"\n\
             [contrato]\ncmd = \"c\"\n[oculto]\ncmd = \"o\"\n\
             [[entorno]]\ntest = \"test_a::uno\"\nmotivo = \"gpu\"\n"
        );
        fs::write(dir.join("tarea.toml"), texto).expect("tarea.toml");
        Tarea::cargar_de(&dir).expect("carga")
    }

    /// Tarea mínima en `home/oraculos/<nombre>` con el `[[entorno]]` dado. `archivos` son los `(suite,
    /// archivo)` que existen bajo `<suite>/` de la tarea, con un test cada uno.
    fn tarea_con_entorno(
        home: &Path,
        nombre: &str,
        entorno: &[&str],
        archivos: &[(&str, &str)],
    ) -> Tarea {
        let dir = home.join("oraculos").join(nombre);
        for (suite, archivo) in archivos {
            let ruta = dir.join(suite).join(format!("{archivo}.py"));
            fs::create_dir_all(ruta.parent().expect("carpeta de la suite")).expect("suite");
            fs::write(&ruta, "def test_t(): pass\n").expect("escribe");
        }
        let mut texto = format!(
            "nombre = \"{nombre}\"\nlenguaje = \"shell\"\nimagen = \"i\"\nspec = \"s\"\n\
             [contrato]\ncmd = \"c\"\n[oculto]\ncmd = \"o\"\n"
        );
        for test in entorno {
            texto.push_str(&format!(
                "[[entorno]]\ntest = \"{test}\"\nmotivo = \"gpu\"\n"
            ));
        }
        fs::write(dir.join("tarea.toml"), texto).expect("tarea.toml");
        Tarea::cargar_de(&dir).expect("carga")
    }

    fn contiene_secreto(dir: &Path) -> Vec<PathBuf> {
        let mut hallados = Vec::new();
        let mut pendientes = vec![dir.to_path_buf()];
        while let Some(actual) = pendientes.pop() {
            let Ok(entradas) = fs::read_dir(&actual) else {
                continue;
            };
            for entrada in entradas.flatten() {
                let ruta = entrada.path();
                if ruta.is_dir() {
                    pendientes.push(ruta);
                } else if fs::read_to_string(&ruta).is_ok_and(|t| t.contains("SECRETO-OCULTO")) {
                    hallados.push(ruta);
                }
            }
        }
        hallados
    }

    #[test]
    fn pendiente_paquete_vive_dentro_de_oraculos_y_no_fuera() {
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        fs::write(repo.path().join("a.txt"), "x").expect("repo");
        let tarea = tarea_en_oraculos(home.path(), "tarea-x", "shell");
        let paquete = crear(home.path(), &tarea, repo.path(), None)
            .expect("crea")
            .expect("hay paquete");
        assert!(
            paquete
                .dir
                .starts_with(home.path().join("oraculos/tarea-x/pendientes")),
            "{}",
            paquete.dir.display()
        );
        assert!(
            !home.path().join("pendientes").exists(),
            "no se usa la carpeta vieja pendientes/ de ROOT_HOME"
        );
        assert!(
            paquete.dir.join("tests/test_a.py").is_file(),
            "el paquete trae el oculto"
        );
    }

    #[test]
    fn pendiente_borrar_paquete_y_tmp_no_deja_tests_ocultos_fuera_de_oraculos() {
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        fs::write(repo.path().join("a.txt"), "x").expect("repo");
        let tarea = tarea_en_oraculos(home.path(), "tarea-x", "shell");
        let paquete = crear(home.path(), &tarea, repo.path(), None)
            .expect("crea")
            .expect("paquete");
        // El junit de Colab que se baja a tmp/colab-<sesion> también es material oculto.
        let tmp = home.path().join("tmp/colab-sesion");
        fs::create_dir_all(&tmp).expect("tmp");
        fs::write(tmp.join("pendientes.xml"), "SECRETO-OCULTO en una falla").expect("xml");
        assert!(
            contiene_secreto(home.path()).len() >= 2,
            "antes de limpiar hay material oculto en el paquete y en tmp"
        );
        limpiar_paquete(home.path(), &tarea, &paquete.dir, "sesion").expect("limpia");
        let fuera: Vec<PathBuf> = contiene_secreto(home.path())
            .into_iter()
            .filter(|r| !r.starts_with(home.path().join("oraculos")))
            .collect();
        assert!(
            fuera.is_empty(),
            "queda material oculto fuera de oraculos: {fuera:?}"
        );
    }

    #[test]
    fn pendiente_importar_ignora_resultado_de_otro_commit() {
        let home = tempfile::tempdir().expect("home");
        let tarea = tarea_en_oraculos(home.path(), "tarea-x", "shell");
        let paquete = home.path().join("oraculos/tarea-x/pendientes");
        fs::create_dir_all(&paquete).expect("paquete");
        fs::write(paquete.join(ARCHIVO_COMMIT), "aaa").expect("commit");
        let junit = home.path().join("pendientes.xml");
        fs::write(
            &junit,
            r#"<testsuite><testcase classname="tests.test_a" name="uno"/></testsuite>"#,
        )
        .expect("junit");
        let otro =
            importar(home.path(), &tarea, std::slice::from_ref(&junit), "bbb").expect("importa");
        assert_eq!(
            otro.importados, 0,
            "el resultado de otro commit no se importa"
        );
        assert!(
            otro.avisos.iter().any(|a| a.contains("commit")),
            "{:?}",
            otro.avisos
        );
        assert!(colab_de(home.path(), "tarea-x").expect("lee").is_empty());
        let mismo =
            importar(home.path(), &tarea, std::slice::from_ref(&junit), "aaa").expect("importa");
        assert_eq!(mismo.importados, 1);
        assert_eq!(
            colab_de(home.path(), "tarea-x").expect("lee")["test_a::uno"].commit,
            "aaa"
        );
    }

    /// Dos junit por repetición de `t` (`tests.test_a::uno`), con los resultados dados.
    fn junit_por_repeticion(home: &Path, resultados: &[&str]) -> Vec<PathBuf> {
        resultados
            .iter()
            .enumerate()
            .map(|(i, resultado)| {
                let caso = match *resultado {
                    "falla" => {
                        r#"<testcase classname="tests.test_a" name="uno"><failure message="x"/></testcase>"#
                    }
                    "otro" => r#"<testcase classname="tests.test_a" name="otro"/>"#,
                    _ => r#"<testcase classname="tests.test_a" name="uno"/>"#,
                };
                let ruta = home.join(format!("pendientes-{}.xml", i + 1));
                fs::write(&ruta, format!("<testsuite>{caso}</testsuite>")).expect("junit");
                ruta
            })
            .collect()
    }

    #[test]
    fn pendiente_importar_repeticiones_alternadas_queda_inestable_con_tasa() {
        let home = tempfile::tempdir().expect("home");
        let tarea = tarea_en_oraculos(home.path(), "tarea-x", "shell");
        let junits = junit_por_repeticion(home.path(), &["pasa", "falla"]);
        let resumen = importar(home.path(), &tarea, &junits, "").expect("importa");
        assert_eq!(resumen.importados, 1);
        let entrada = &colab_de(home.path(), "tarea-x").expect("lee")["test_a::uno"];
        assert_eq!(entrada.resultado, "inestable", "no debe quedar Falla");
        let tasa = entrada.tasa.as_ref().expect("lleva la tasa");
        assert_eq!((tasa.pasa, tasa.total), (1, 2));
    }

    #[test]
    fn pendiente_importar_repeticiones_que_pasan_en_todas_queda_verde() {
        let home = tempfile::tempdir().expect("home");
        let tarea = tarea_en_oraculos(home.path(), "tarea-x", "shell");
        let junits = junit_por_repeticion(home.path(), &["pasa", "pasa"]);
        importar(home.path(), &tarea, &junits, "").expect("importa");
        let entrada = &colab_de(home.path(), "tarea-x").expect("lee")["test_a::uno"];
        assert_eq!(entrada.resultado, "pasa");
        assert_eq!(entrada.tasa, None, "un test estable no lleva tasa");
    }

    #[test]
    fn pendiente_importar_test_ausente_en_una_repeticion_queda_inestable() {
        let home = tempfile::tempdir().expect("home");
        let tarea = tarea_en_oraculos(home.path(), "tarea-x", "shell");
        let junits = junit_por_repeticion(home.path(), &["pasa", "otro"]);
        importar(home.path(), &tarea, &junits, "").expect("importa");
        let entrada = &colab_de(home.path(), "tarea-x").expect("lee")["test_a::uno"];
        assert_eq!(entrada.resultado, "inestable");
        assert_eq!(entrada.tasa.as_ref().map(|t| t.total), Some(1));
    }

    #[test]
    fn pendiente_importar_sin_junit_da_error_de_uso() {
        let home = tempfile::tempdir().expect("home");
        let tarea = tarea_en_oraculos(home.path(), "tarea-x", "shell");
        let r = importar(home.path(), &tarea, &[], "");
        assert!(matches!(r, Err(ErrorOraculo::Uso(_))), "{r:?}");
    }

    #[test]
    fn pendiente_turno_ocupado_da_error_que_nombra_al_dueno() {
        let home = tempfile::tempdir().expect("home");
        tarea_en_oraculos(home.path(), "tarea-x", "shell");
        let _primero = tomar_turno(home.path(), "tarea-x").expect("el primero toma el turno");
        let Err(error) = tomar_turno(home.path(), "tarea-x") else {
            panic!("el segundo no debía tomar el turno");
        };
        let texto = error.to_string();
        assert!(
            texto.contains(&format!("pid {}", std::process::id())),
            "el mensaje nombra al dueño: {texto}"
        );
        assert!(texto.contains("desde"), "dice desde cuándo: {texto}");
        assert!(texto.contains("tarea-x"), "{texto}");
        assert_eq!(error.codigo_de_salida(), 3, "{texto}");
    }

    #[test]
    fn pendiente_turno_se_suelta_al_soltarlo() {
        let home = tempfile::tempdir().expect("home");
        tarea_en_oraculos(home.path(), "tarea-x", "shell");
        let primero = tomar_turno(home.path(), "tarea-x").expect("toma el turno");
        drop(primero);
        assert!(
            tomar_turno(home.path(), "tarea-x").is_ok(),
            "después de soltarlo, otra corrida toma el turno"
        );
    }

    #[test]
    fn pendiente_turno_es_por_tarea() {
        let home = tempfile::tempdir().expect("home");
        tarea_en_oraculos(home.path(), "tarea-x", "shell");
        tarea_en_oraculos(home.path(), "tarea-y", "shell");
        let _x = tomar_turno(home.path(), "tarea-x").expect("toma el de x");
        assert!(
            tomar_turno(home.path(), "tarea-y").is_ok(),
            "otra tarea no se bloquea"
        );
    }

    #[test]
    fn pendiente_turno_no_acepta_nombre_con_ruta() {
        let home = tempfile::tempdir().expect("home");
        let Err(error) = tomar_turno(home.path(), "../fuera") else {
            panic!("un nombre con ruta no toma turno");
        };
        assert!(matches!(error, ErrorOraculo::Tarea(_)), "{error:?}");
        assert!(!home.path().join("oraculos").exists());
    }

    fn python3_real() -> PathBuf {
        let path = std::env::var_os("PATH").unwrap_or_default();
        std::env::split_paths(&path)
            .map(|d| d.join("python3"))
            .find(|p| p.is_file())
            .expect("hay python3")
    }

    /// Arma un paquete con `correr.sh` y un `bin/` con `unshare` y `python3` falsos.
    fn paquete_con_falsos(base: &Path, unshare: &str) -> PathBuf {
        let dir = base.join("paquete");
        let bin = dir.join("bin");
        fs::create_dir_all(&bin).expect("bin");
        fs::write(
            dir.join("correr.sh"),
            guion_correr(&["tests/test_a.py::t1".into()], 1, ""),
        )
        .expect("sh");
        fs::write(bin.join("unshare"), unshare).expect("unshare");
        let fake_python = format!(
            "#!/bin/sh\n\
             case \"$*\" in\n\
               *\"-m pytest\"*)\n\
                 for a in \"$@\"; do case \"$a\" in --junitxml=*) xml=\"${{a#--junitxml=}}\";; esac; done\n\
                 printf '<testsuite><testcase classname=\"tests.test_a\" name=\"t1\"/></testsuite>' > \"$xml\"\n\
                 exit 0;;\n\
             esac\n\
             exec {} \"$@\"\n",
            python3_real().display()
        );
        fs::write(bin.join("python3"), fake_python).expect("python3");
        for f in ["unshare", "python3"] {
            fs::set_permissions(bin.join(f), fs::Permissions::from_mode(0o755)).expect("perm");
        }
        dir
    }

    const UNSHARE_FALLA: &str = "#!/bin/sh\nexit 1\n";
    const UNSHARE_SANO: &str =
        "#!/bin/sh\nwhile [ \"${1#-}\" != \"$1\" ]; do shift; done\nexec \"$@\"\n";

    fn correr_paquete(dir: &Path) -> (i32, String) {
        let salida = Command::new("bash")
            .arg(dir.join("correr.sh"))
            .current_dir(dir)
            .output()
            .expect("bash");
        (
            salida.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&salida.stderr).into_owned(),
        )
    }

    #[test]
    fn pendiente_correr_sh_con_unshare_que_falla_sigue_y_deja_marca_con_red() {
        let base = tempfile::tempdir().expect("base");
        let dir = paquete_con_falsos(base.path(), UNSHARE_FALLA);
        let (codigo, error) = correr_paquete(&dir);
        assert_eq!(codigo, 0, "la corrida sigue: {error}");
        assert_eq!(
            fs::read_to_string(dir.join("red.txt"))
                .expect("marca")
                .trim(),
            "con-red"
        );
        let xml = fs::read_to_string(dir.join("pendientes-1.xml")).expect("junit por repetición");
        let leido = junit::parsear(&xml).expect("parsea");
        assert_eq!(leido.red.as_deref(), Some("con-red"));
    }

    #[test]
    fn pendiente_correr_sh_con_unshare_sano_deja_marca_sin_red() {
        let base = tempfile::tempdir().expect("base");
        let dir = paquete_con_falsos(base.path(), UNSHARE_SANO);
        let (codigo, error) = correr_paquete(&dir);
        assert_eq!(codigo, 0, "{error}");
        assert_eq!(
            fs::read_to_string(dir.join("red.txt"))
                .expect("marca")
                .trim(),
            "sin-red"
        );
        let xml = fs::read_to_string(dir.join("pendientes-1.xml")).expect("junit");
        assert_eq!(
            junit::parsear(&xml).expect("parsea").red.as_deref(),
            Some("sin-red")
        );
    }

    #[test]
    fn pendiente_paquete_con_cache_copia_dependencias_y_aisla_la_corrida() {
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        fs::write(repo.path().join("a.txt"), "x").expect("repo");
        let tarea = tarea_en_oraculos(home.path(), "tarea-x", "rust");
        let cache = tempfile::tempdir().expect("cache");
        fs::create_dir_all(cache.path().join("vendor/serde")).expect("vendor");
        fs::write(
            cache.path().join("config.toml"),
            "[source.x]\ndirectory = \"/cache/vendor\"\n",
        )
        .expect("config");
        let paquete = crear(home.path(), &tarea, repo.path(), Some(cache.path()))
            .expect("crea")
            .expect("paquete");
        assert!(
            paquete.dir.join("cache/config.toml").is_file(),
            "copia la caché"
        );
        assert!(
            paquete.dir.join("cache/vendor/serde").is_dir(),
            "copia el vendor"
        );
        let sh = fs::read_to_string(paquete.dir.join("correr.sh")).expect("correr.sh");
        assert!(sh.contains("unshare -rn"), "{sh}");
        assert!(sh.contains("ROOT_CACHE=\"$(pwd)/cache\""), "{sh}");
        assert!(sh.contains("export CARGO_HOME=/tmp/cargo-home"), "{sh}");
        assert!(
            sh.contains("sed \"s#/cache#$ROOT_CACHE#g\" \"$ROOT_CACHE/config.toml\""),
            "{sh}"
        );
        assert!(
            !sh.contains("sed -i"),
            "la caché intacta no se reescribe en el lugar: {sh}"
        );
        assert!(
            !sh.contains("cp /cache/"),
            "la caché del paquete no apunta a /cache: {sh}"
        );
        assert_eq!(paquete.commit, "", "el repo de prueba no es git");
        let sintaxis = Command::new("bash")
            .arg("-n")
            .arg(paquete.dir.join("correr.sh"))
            .output()
            .expect("bash");
        assert!(
            sintaxis.status.success(),
            "{}",
            String::from_utf8_lossy(&sintaxis.stderr)
        );
    }

    // --- Tarea 30, parte 2: la marca de red de Colab llega a cada entrada y al aviso ---

    #[test]
    fn pendiente_importar_con_red_marca_cada_entrada_importada() {
        let home = tempfile::tempdir().expect("home");
        let tarea = tarea_en_oraculos(home.path(), "tarea-x", "shell");
        let junit = home.path().join("con-red.xml");
        fs::write(
            &junit,
            r#"<testsuite name="pendientes" root_red="con-red"><testcase classname="tests.test_a" name="uno"/></testsuite>"#,
        )
        .expect("junit");
        importar(home.path(), &tarea, &[junit], "").expect("importa");
        let entrada = &colab_de(home.path(), "tarea-x").expect("lee")["test_a::uno"];
        assert_eq!(entrada.confianza.as_deref(), Some(CONFIANZA_CON_RED));
    }

    #[test]
    fn pendiente_importar_sin_red_no_marca_entradas() {
        let home = tempfile::tempdir().expect("home");
        let tarea = tarea_en_oraculos(home.path(), "tarea-x", "shell");
        let junits = junit_por_repeticion(home.path(), &["pasa"]);
        importar(home.path(), &tarea, &junits, "").expect("importa");
        let entrada = &colab_de(home.path(), "tarea-x").expect("lee")["test_a::uno"];
        assert_eq!(entrada.confianza, None);
    }

    #[test]
    fn pendiente_aviso_de_red_con_una_entrada_marcada_da_el_aviso_una_vez() {
        let entrada = |confianza: Option<&str>| ColabEntrada {
            resultado: "pasa".into(),
            junit_sha256: "x".into(),
            ts: 1.0,
            commit: String::new(),
            tasa: None,
            confianza: confianza.map(String::from),
        };
        let colab = BTreeMap::from([
            ("test_a::uno".to_string(), entrada(Some(CONFIANZA_CON_RED))),
            ("test_a::dos".to_string(), entrada(Some(CONFIANZA_CON_RED))),
        ]);
        assert_eq!(aviso_de_red(&colab).as_deref(), Some(CONFIANZA_CON_RED));
    }

    #[test]
    fn pendiente_aviso_de_red_sin_marca_es_none() {
        let colab = BTreeMap::from([(
            "test_a::uno".to_string(),
            ColabEntrada {
                resultado: "pasa".into(),
                junit_sha256: "x".into(),
                ts: 1.0,
                commit: String::new(),
                tasa: None,
                confianza: None,
            },
        )]);
        assert_eq!(aviso_de_red(&colab), None);
        assert_eq!(aviso_de_red(&BTreeMap::new()), None);
    }
}
