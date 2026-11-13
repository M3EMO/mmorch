//! Barrido de copias de credencial de corridas muertas. Al arrancar una corrida, quita la copia
//! `.credentials.json` de las carpetas de corridas cuyo proceso ya no vive, no es `r00t` o es otro
//! proceso con el pid reusado. Deja un `Gate` por cada copia barrida. El `Gate` nunca lleva el contenido de la copia.

use std::fs;
use std::io;
use std::path::Path;

use serde_json::{Map, json};

use crate::borrado;
use crate::corre::ErrorCorre;
use crate::observe;

/// Carpeta de estado de `claude` dentro de cada corrida.
const CARPETA_ESTADO: &str = "claude-estado";
/// Nombre de la copia de la credencial dentro de la carpeta de estado.
const COPIA: &str = ".credentials.json";
/// Prefijo del id de toda carpeta de corrida.
const PREFIJO: &str = "corre-";
/// Fase de los `Gate` del barrido.
const FASE_BARRIDO: &str = "barrido";
/// Carpeta donde Linux muestra los procesos vivos.
const PROCESOS: &str = "/proc";

/// Resultado de un barrido: ids de las corridas barridas y de las que dieron error.
/// "Barrida" quiere decir que su copia se borró.
#[derive(Debug, Default, PartialEq)]
pub struct ResultadoBarrido {
    /// Corridas cuya copia se borró.
    pub barridas: Vec<String>,
    /// Corridas en las que hubo algún error.
    pub con_error: Vec<String>,
}

/// Recorre las carpetas de corridas de `raiz_corridas` y barre las copias de corridas muertas.
/// Si la raíz no existe, no hace nada. Una corrida que no se puede borrar deja un `Gate` y no corta
/// el barrido de las demás. Devuelve las corridas barridas y las que dieron error.
pub fn barrer(raiz_corridas: &Path, ruta_eventos: &Path) -> Result<ResultadoBarrido, ErrorCorre> {
    barrer_con_proc(raiz_corridas, ruta_eventos, Path::new(PROCESOS))
}

/// Igual que `barrer`, con la raíz de `/proc` inyectable. Los tests arman una raíz falsa.
pub fn barrer_con_proc(
    raiz_corridas: &Path,
    ruta_eventos: &Path,
    raiz_proc: &Path,
) -> Result<ResultadoBarrido, ErrorCorre> {
    let entradas = match fs::read_dir(raiz_corridas) {
        Ok(entradas) => entradas,
        Err(fuente) if fuente.kind() == io::ErrorKind::NotFound => {
            return Ok(ResultadoBarrido::default());
        }
        Err(fuente) => {
            return Err(ErrorCorre::Io {
                contexto: format!("leer {}", raiz_corridas.display()),
                fuente,
            });
        }
    };
    // Una entrada que no se puede leer no tiene id: se informa y el barrido sigue.
    let mut carpetas = Vec::new();
    for entrada in entradas {
        match entrada {
            Ok(entrada) => carpetas.push(entrada.path()),
            Err(fuente) => eprintln!(
                "r00t corre: barrido: no se pudo leer una entrada de {}: {fuente}",
                raiz_corridas.display()
            ),
        }
    }
    // Orden por nombre: el orden de `read_dir` no es fijo.
    carpetas.sort();
    let mut resultado = ResultadoBarrido::default();
    for carpeta in carpetas {
        // Un enlace o un archivo suelto no es carpeta de corrida.
        let Ok(meta) = fs::symlink_metadata(&carpeta) else {
            continue;
        };
        if !meta.is_dir() {
            continue;
        }
        let Some(id) = carpeta.file_name().and_then(|nombre| nombre.to_str()) else {
            continue;
        };
        if !id.starts_with(PREFIJO) {
            continue;
        }
        let copia = carpeta.join(CARPETA_ESTADO).join(COPIA);
        match fs::symlink_metadata(&copia) {
            Ok(_) => {}
            Err(fuente) if fuente.kind() == io::ErrorKind::NotFound => continue,
            Err(fuente) => {
                let motivo = format!(
                    "no se pudo mirar la copia de la corrida {id}: {:?}",
                    fuente.kind()
                );
                anotar(ruta_eventos, id, motivo);
                resultado.con_error.push(id.to_string());
                continue;
            }
        }
        let Some(por_que) = motivo_de_muerte(id, raiz_proc) else {
            continue;
        };
        match borrado::borrar_archivo_dentro(&carpeta, &copia) {
            Ok(()) => {
                let motivo = format!("copia de la corrida {id} barrida: {por_que}");
                if !anotar(ruta_eventos, id, motivo) {
                    resultado.con_error.push(id.to_string());
                }
                resultado.barridas.push(id.to_string());
            }
            Err(error) => {
                let motivo = format!("no se pudo borrar la copia de la corrida {id}: {error}");
                anotar(ruta_eventos, id, motivo);
                resultado.con_error.push(id.to_string());
            }
        }
    }
    Ok(resultado)
}

/// Ticks de reloj por segundo del `starttime` de `/proc/<pid>/stat`. Linux usa 100 (CLK_TCK).
const TICKS_POR_SEG: u64 = 100;
/// Margen en segundos entre el inicio del proceso y los segundos del id. Antes era 2 s.
/// La laptop se suspende y NTP corrige el reloj: `btime` deriva. Una corrida viva puede verse
/// como pid reusado y perder su copia. Por eso el margen es de 300 s.
const MARGEN_S: u64 = 300;

/// Por qué la corrida `id` está muerta. `None` si su proceso vive, es `r00t` y es la misma corrida.
/// Un pid sin carpeta en `raiz_proc` no vive. Un cmdline ilegible es de otro usuario: no es `r00t`
/// nuestro. Un proceso que no cuenta como `r00t` vivo está muerto (regla de `limpieza::r00t_vivo`).
/// Un proceso que empezó después de la corrida es otro proceso con el pid reusado.
fn motivo_de_muerte(id: &str, raiz_proc: &Path) -> Option<String> {
    let Some(pid) = pid_de(id) else {
        return Some("el id no tiene pid legible".to_string());
    };
    let carpeta = raiz_proc.join(pid.to_string());
    if !carpeta.exists() {
        return Some(format!("el pid {pid} no vive"));
    }
    match fs::read(carpeta.join("cmdline")) {
        Ok(_) => {}
        Err(fuente) if fuente.kind() == io::ErrorKind::NotFound => {
            return Some(format!("el pid {pid} no vive"));
        }
        Err(_) => return Some(format!("el cmdline del pid {pid} es ilegible")),
    }
    if !crate::limpieza::r00t_vivo(pid, raiz_proc) {
        return Some(format!("el pid {pid} no es r00t"));
    }
    motivo_si_pid_reusado(id, pid, raiz_proc)
}

/// Motivo si el proceso `pid` empezó más de `MARGEN_S` segundos después de la corrida `id`. `None`
/// si empezó antes o en el margen, o si no se puede leer su inicio: en ese caso la corrida se
/// conserva, porque su cmdline ya dijo que es `r00t`.
fn motivo_si_pid_reusado(id: &str, pid: u32, raiz_proc: &Path) -> Option<String> {
    let segundos = segundos_de(id)?;
    let inicio = inicio_de_proceso(pid, raiz_proc)?;
    (inicio > segundos + MARGEN_S)
        .then(|| format!("el pid {pid} empezó después de la corrida: se reusó"))
}

/// Segundos de inicio que lleva el id: el tercero desde el final.
fn segundos_de(id: &str) -> Option<u64> {
    let mut partes = id.rsplit('-');
    partes.next()?;
    partes.next()?;
    partes.next()?.parse().ok()
}

/// Segundos de inicio del proceso `pid`: `btime` más el `starttime` en ticks. `None` si `stat` o
/// `btime` no se pueden leer o parsear.
fn inicio_de_proceso(pid: u32, raiz_proc: &Path) -> Option<u64> {
    let stat = fs::read_to_string(raiz_proc.join(pid.to_string()).join("stat")).ok()?;
    // El comm va entre paréntesis y puede tener espacios o `)`: se parte después del último `)`.
    // Después del `)` el primer campo es el 3 (state). El campo 22 (starttime) es el índice 19.
    let despues_del_comm = stat.rsplit_once(')')?.1;
    let ticks: u64 = despues_del_comm.split_whitespace().nth(19)?.parse().ok()?;
    Some(btime_del_sistema(raiz_proc)? + ticks / TICKS_POR_SEG)
}

/// Segundos desde el arranque del sistema, de la línea `btime` de `<raiz_proc>/stat`.
fn btime_del_sistema(raiz_proc: &Path) -> Option<u64> {
    let stat = fs::read_to_string(raiz_proc.join("stat")).ok()?;
    stat.lines()
        .find_map(|linea| linea.strip_prefix("btime "))
        .and_then(|valor| valor.trim().parse().ok())
}

/// El pid de un id `corre-<tarea>-<config>-<segundos>-<pid>-<contador>`. Se toma desde el final,
/// porque la tarea y la config pueden tener guiones.
fn pid_de(id: &str) -> Option<u32> {
    let mut partes = id.rsplit('-');
    partes.next()?;
    partes.next()?.parse().ok()
}

/// Agrega un `Gate` de fase `barrido` a `eventos.jsonl`. Solo lleva el motivo, nunca el contenido de la copia.
fn gate(ruta_eventos: &Path, corrida: &str, motivo: String) -> Result<(), ErrorCorre> {
    let extra = Map::from_iter([("motivo".to_string(), json!(motivo))]);
    let ev = observe::evento_gate(corrida, FASE_BARRIDO, extra);
    observe::registrar_en(ruta_eventos, &ev).map_err(|fuente| ErrorCorre::Io {
        contexto: format!("registrar el barrido en {}", ruta_eventos.display()),
        fuente,
    })
}

/// Registra el `Gate` de `corrida`. Si falla, lo informa por stderr y sigue. Devuelve si quedó registrado.
fn anotar(ruta_eventos: &Path, corrida: &str, motivo: String) -> bool {
    match gate(ruta_eventos, corrida, motivo) {
        Ok(()) => true,
        Err(error) => {
            eprintln!("r00t corre: barrido: no se pudo registrar la corrida {corrida}: {error}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;

    use serde_json::Value;
    use tempfile::TempDir;

    use crate::tipos::{Evento, TipoEvento};

    /// Un token falso: los tests nunca usan una credencial real.
    const TOKEN_FALSO: &str = "TOKEN_FALSO_PRUEBA";
    /// Pid que no puede existir en Linux: el máximo de pids es 4194304.
    const PID_MUERTO: u32 = 4194999;

    /// Carpeta base con `corridas/` (la raíz de corridas), `eventos.jsonl` y `fuera/` para enlaces.
    struct Escenario {
        _base: TempDir,
        base: PathBuf,
        raiz: PathBuf,
        eventos: PathBuf,
    }

    fn escenario() -> Escenario {
        let base = tempfile::tempdir().expect("temp dir");
        let ruta_base = base.path().to_path_buf();
        let raiz = ruta_base.join("corridas");
        fs::create_dir_all(&raiz).expect("raiz");
        let eventos = ruta_base.join("eventos.jsonl");
        Escenario {
            _base: base,
            base: ruta_base,
            raiz,
            eventos,
        }
    }

    /// Crea `<raiz>/<id>/claude-estado/.credentials.json` con el token falso. Devuelve la copia.
    fn corrida_con_copia(raiz: &Path, id: &str) -> PathBuf {
        let estado = raiz.join(id).join("claude-estado");
        fs::create_dir_all(&estado).expect("estado");
        let copia = estado.join(".credentials.json");
        fs::write(&copia, TOKEN_FALSO).expect("copia");
        copia
    }

    /// Gates de fase `barrido` que quedaron en `eventos.jsonl`.
    fn gates_de_barrido(eventos: &Path) -> Vec<Evento> {
        fs::read_to_string(eventos)
            .unwrap_or_default()
            .lines()
            .map(|linea| serde_json::from_str::<Evento>(linea).expect("evento válido"))
            .filter(|ev| ev.tipo == TipoEvento::Gate && ev.fase == "barrido")
            .collect()
    }

    fn motivo(ev: &Evento) -> String {
        ev.extra
            .get("motivo")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    }

    fn sin_token(eventos: &Path) -> bool {
        !fs::read_to_string(eventos)
            .unwrap_or_default()
            .contains(TOKEN_FALSO)
    }

    #[test]
    fn corrida_muerta_pierde_su_copia_y_deja_gate_sin_token() {
        let e = escenario();
        let id = format!("corre-memo-ecc-test-100-{PID_MUERTO}-0");
        assert!(
            !Path::new(&format!("/proc/{PID_MUERTO}")).exists(),
            "el pid de prueba no debe existir"
        );
        let copia = corrida_con_copia(&e.raiz, &id);
        barrer(&e.raiz, &e.eventos).expect("barre");
        assert!(
            fs::symlink_metadata(&copia).is_err(),
            "la copia de una corrida muerta se borra: {}",
            copia.display()
        );
        let gates = gates_de_barrido(&e.eventos);
        assert_eq!(gates.len(), 1, "un Gate de barrido");
        assert_eq!(
            gates[0].corrida, id,
            "el Gate lleva el id de la corrida barrida"
        );
        assert!(motivo(&gates[0]).contains(&PID_MUERTO.to_string()));
        assert!(sin_token(&e.eventos), "el Gate no lleva el contenido");
    }

    /// El pid propio cuenta como vivo (`limpieza::r00t_vivo`). Su corrida no se barre. Los segundos
    /// del id son los de ahora: el proceso de test empezó antes, así que no pasa el margen.
    #[test]
    fn corrida_con_pid_del_proceso_de_test_se_conserva() {
        let e = escenario();
        let ahora = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("reloj")
            .as_secs();
        let id = format!("corre-memo-ecc-test-{ahora}-{}-0", std::process::id());
        let copia = corrida_con_copia(&e.raiz, &id);
        barrer(&e.raiz, &e.eventos).expect("barre");
        assert!(
            copia.is_file(),
            "el pid propio cuenta como vivo: la copia queda"
        );
        assert!(gates_de_barrido(&e.eventos).is_empty(), "sin Gate");
    }

    /// Crea `<raiz_proc>/<pid>/cmdline` con `cmdline`: un proceso falso para `barrer_con_proc`.
    fn proceso_falso(raiz_proc: &Path, pid: u32, cmdline: &[u8]) {
        let carpeta = raiz_proc.join(pid.to_string());
        fs::create_dir_all(&carpeta).expect("proceso");
        fs::write(carpeta.join("cmdline"), cmdline).expect("cmdline");
    }

    #[test]
    fn corrida_de_r00t_viva_no_se_barre() {
        let e = escenario();
        let pid = 4000123;
        let raiz_proc = e.base.join("proc");
        proceso_falso(&raiz_proc, pid, b"/usr/bin/r00t\0corre\0--tarea\0memo\0");
        let id = format!("corre-memo-ecc-test-100-{pid}-0");
        let copia = corrida_con_copia(&e.raiz, &id);
        barrer_con_proc(&e.raiz, &e.eventos, &raiz_proc).expect("barre");
        assert!(copia.is_file(), "el pid es de r00t: la copia queda");
        assert!(gates_de_barrido(&e.eventos).is_empty(), "sin Gate");
    }

    #[test]
    fn r00t_con_sha_vivo_no_se_barre() {
        let e = escenario();
        let raiz_proc = e.base.join("proc");
        let pid = 4000150;
        // `r00t-<sha>` es el nombre del programa r00t con su sha: sigue siendo r00t.
        proceso_falso(
            &raiz_proc,
            pid,
            b"/usr/bin/r00t-abc123\0corre\0--tarea\0memo\0",
        );
        let id = format!("corre-memo-ecc-test-100-{pid}-0");
        let copia = corrida_con_copia(&e.raiz, &id);
        barrer_con_proc(&e.raiz, &e.eventos, &raiz_proc).expect("barre");
        assert!(copia.is_file(), "el pid es de r00t-<sha>: la copia queda");
        assert!(gates_de_barrido(&e.eventos).is_empty(), "sin Gate");
    }

    #[test]
    fn pid_reciclado_por_otro_proceso_se_barre() {
        let e = escenario();
        let raiz_proc = e.base.join("proc");
        // Otro programa, un argumento `r00t` que no es el primero, un nombre que solo contiene r00t,
        // y un cmdline vacío (proceso zombi): ninguno es r00t.
        let casos: [(u32, &[u8]); 4] = [
            (4000201, b"/usr/bin/bash\0"),
            (4000202, b"sh\0r00t\0corre\0"),
            (4000203, b"/usr/bin/xr00t\0corre\0"),
            (4000204, b""),
        ];
        let mut copias = Vec::new();
        for (pid, cmdline) in casos {
            proceso_falso(&raiz_proc, pid, cmdline);
            let id = format!("corre-memo-ecc-test-100-{pid}-0");
            copias.push(corrida_con_copia(&e.raiz, &id));
        }
        barrer_con_proc(&e.raiz, &e.eventos, &raiz_proc).expect("barre");
        for copia in &copias {
            assert!(
                fs::symlink_metadata(copia).is_err(),
                "no es r00t: {}",
                copia.display()
            );
        }
        let gates = gates_de_barrido(&e.eventos);
        assert_eq!(gates.len(), 4, "un Gate por corrida");
        assert!(gates.iter().all(|ev| motivo(ev).contains("no es r00t")));
    }

    /// Crea un proceso falso `r00t` con `stat` y `btime`. El inicio del proceso es
    /// `btime + ticks / 100` segundos.
    fn proceso_con_inicio(raiz_proc: &Path, pid: u32, btime: u64, ticks: u64) {
        proceso_falso(raiz_proc, pid, b"/usr/bin/r00t\0corre\0");
        // Campo 22 (starttime) es el índice 19 después del `)` que cierra el comm.
        let stat =
            format!("{pid} (r00t) S 1 {pid} {pid} 0 -1 4194560 0 0 0 0 0 0 0 0 20 0 1 0 {ticks}\n");
        fs::write(raiz_proc.join(pid.to_string()).join("stat"), stat).expect("stat");
        let contenido = format!("cpu  1 2 3\nbtime {btime}\n");
        fs::write(raiz_proc.join("stat"), contenido).expect("btime");
    }

    #[test]
    fn cmdline_ilegible_es_de_otro_usuario_y_se_barre() {
        let e = escenario();
        let raiz_proc = e.base.join("proc");
        let pid = 4000301;
        proceso_falso(&raiz_proc, pid, b"/usr/bin/r00t\0corre\0");
        let cmdline = raiz_proc.join(pid.to_string()).join("cmdline");
        fs::set_permissions(&cmdline, fs::Permissions::from_mode(0o000)).expect("sin permiso");
        let id = format!("corre-memo-ecc-test-100-{pid}-0");
        let copia = corrida_con_copia(&e.raiz, &id);
        barrer_con_proc(&e.raiz, &e.eventos, &raiz_proc).expect("barre");
        assert!(
            fs::symlink_metadata(&copia).is_err(),
            "un cmdline ilegible no es r00t nuestro: la copia se barre"
        );
        let gates = gates_de_barrido(&e.eventos);
        assert_eq!(gates.len(), 1, "un Gate de barrido");
        assert!(
            motivo(&gates[0]).contains("ilegible"),
            "{}",
            motivo(&gates[0])
        );
        assert!(sin_token(&e.eventos));
    }

    #[test]
    fn pid_reusado_por_otro_r00t_que_empezo_despues_se_barre() {
        let e = escenario();
        let raiz_proc = e.base.join("proc");
        let pid = 4000401;
        // La corrida nació a los 1700000000 s. El proceso del pid nació 400 s después: pasa el margen
        // de 300 s. Antes de subir el margen, este caso usaba 10 s después.
        proceso_con_inicio(&raiz_proc, pid, 1_700_000_000, 40_000);
        let id = format!("corre-memo-ecc-test-1700000000-{pid}-0");
        let copia = corrida_con_copia(&e.raiz, &id);
        barrer_con_proc(&e.raiz, &e.eventos, &raiz_proc).expect("barre");
        assert!(
            fs::symlink_metadata(&copia).is_err(),
            "otra corrida r00t con el mismo pid: la copia se barre"
        );
        let gates = gates_de_barrido(&e.eventos);
        assert_eq!(gates.len(), 1, "un Gate de barrido");
        assert!(
            motivo(&gates[0]).contains("empezó después"),
            "{}",
            motivo(&gates[0])
        );
        assert!(sin_token(&e.eventos));
    }

    /// Margen de 300 s. Antes era de 2 s: el caso "2 s después" ahora prueba el margen nuevo, en el
    /// borde de 300 s.
    #[test]
    fn r00t_vivo_que_empezo_antes_o_en_el_margen_no_se_barre() {
        let e = escenario();
        let raiz_proc = e.base.join("proc");
        // La corrida nació a los 1700000000 s. El `btime` es uno solo para todo el /proc falso.
        // Un proceso que nació 500 s antes: es la misma corrida.
        let antes = 4000501;
        proceso_con_inicio(&raiz_proc, antes, 1_699_999_000, 50_000);
        // Un proceso que nació 300 s después, justo en el borde del margen: es la misma corrida.
        let margen = 4000502;
        proceso_con_inicio(&raiz_proc, margen, 1_699_999_000, 130_000);
        let copia_antes = corrida_con_copia(
            &e.raiz,
            &format!("corre-memo-ecc-test-1700000000-{antes}-0"),
        );
        let copia_margen = corrida_con_copia(
            &e.raiz,
            &format!("corre-memo-ecc-test-1700000000-{margen}-0"),
        );
        barrer_con_proc(&e.raiz, &e.eventos, &raiz_proc).expect("barre");
        assert!(
            copia_antes.is_file(),
            "el pid es de la misma corrida: la copia queda"
        );
        assert!(
            copia_margen.is_file(),
            "en el borde del margen de 300 s: la copia queda"
        );
        assert!(gates_de_barrido(&e.eventos).is_empty(), "sin Gate");
    }

    #[test]
    fn r00t_vivo_que_empezo_60_s_despues_no_se_barre() {
        let e = escenario();
        let raiz_proc = e.base.join("proc");
        // Inicio = btime + 6000 ticks / 100 = 60 s después de los segundos del id.
        let pid = 4000601;
        proceso_con_inicio(&raiz_proc, pid, 1_700_000_000, 6000);
        let copia = corrida_con_copia(&e.raiz, &format!("corre-memo-ecc-test-1700000000-{pid}-0"));
        barrer_con_proc(&e.raiz, &e.eventos, &raiz_proc).expect("barre");
        assert!(
            copia.is_file(),
            "60 s después está dentro del margen de 300 s: la copia queda"
        );
        assert!(gates_de_barrido(&e.eventos).is_empty(), "sin Gate");
    }

    #[test]
    fn r00t_vivo_que_empezo_400_s_despues_se_barre() {
        let e = escenario();
        let raiz_proc = e.base.join("proc");
        // Inicio = btime + 40000 ticks / 100 = 400 s después de los segundos del id: pasa el margen.
        let pid = 4000701;
        proceso_con_inicio(&raiz_proc, pid, 1_700_000_000, 40_000);
        let copia = corrida_con_copia(&e.raiz, &format!("corre-memo-ecc-test-1700000000-{pid}-0"));
        barrer_con_proc(&e.raiz, &e.eventos, &raiz_proc).expect("barre");
        assert!(
            fs::symlink_metadata(&copia).is_err(),
            "400 s después pasa el margen de 300 s: se reusó, la copia se barre"
        );
        let gates = gates_de_barrido(&e.eventos);
        assert_eq!(gates.len(), 1, "un Gate de barrido");
        assert!(motivo(&gates[0]).contains("empezó después"));
    }

    #[test]
    fn error_en_una_corrida_del_medio_no_corta_el_barrido() {
        let e = escenario();
        let primera = format!("corre-memo-ecc-test-1-{PID_MUERTO}-0");
        let del_medio = format!("corre-memo-ecc-test-2-{}-0", PID_MUERTO - 1);
        let tercera = format!("corre-memo-ecc-test-3-{}-0", PID_MUERTO - 2);
        let copia_primera = corrida_con_copia(&e.raiz, &primera);
        // La carpeta de estado del medio es un enlace a `fuera/`: su borrado falla y no toca nada.
        let fuera = e.base.join("fuera");
        fs::create_dir_all(&fuera).expect("fuera");
        let ajena = fuera.join(".credentials.json");
        fs::write(&ajena, "dato-ajeno").expect("ajena");
        fs::create_dir_all(e.raiz.join(&del_medio)).expect("carpeta");
        symlink(&fuera, e.raiz.join(&del_medio).join("claude-estado")).expect("enlace de estado");
        let copia_tercera = corrida_con_copia(&e.raiz, &tercera);

        let resultado = barrer(&e.raiz, &e.eventos).expect("barre sin cortar");

        assert!(
            fs::symlink_metadata(&copia_primera).is_err(),
            "la primera se barre"
        );
        assert!(
            fs::symlink_metadata(&copia_tercera).is_err(),
            "la tercera se barre"
        );
        assert_eq!(fs::read_to_string(&ajena).expect("ajena"), "dato-ajeno");
        assert_eq!(
            resultado.barridas,
            vec![primera.clone(), tercera.clone()],
            "barridas en orden de nombre"
        );
        assert_eq!(
            resultado.con_error,
            vec![del_medio.clone()],
            "la del medio da error"
        );
        assert_eq!(gates_de_barrido(&e.eventos).len(), 3, "un Gate por corrida");
        assert!(sin_token(&e.eventos));
    }

    #[test]
    fn error_al_registrar_el_gate_no_corta_el_barrido() {
        let e = escenario();
        let primera = format!("corre-memo-ecc-test-1-{PID_MUERTO}-0");
        let segunda = format!("corre-memo-ecc-test-2-{}-0", PID_MUERTO - 1);
        let copia_primera = corrida_con_copia(&e.raiz, &primera);
        let copia_segunda = corrida_con_copia(&e.raiz, &segunda);
        // `eventos.jsonl` es una carpeta: registrar cada Gate falla, pero la copia se borra.
        fs::create_dir_all(&e.eventos).expect("eventos como carpeta");

        let resultado = barrer(&e.raiz, &e.eventos).expect("barre sin cortar");

        assert!(
            fs::symlink_metadata(&copia_primera).is_err(),
            "primera barrida"
        );
        assert!(
            fs::symlink_metadata(&copia_segunda).is_err(),
            "segunda barrida"
        );
        assert_eq!(resultado.barridas, vec![primera, segunda]);
    }

    #[test]
    fn pid_sin_carpeta_en_proc_dice_que_no_vive() {
        let e = escenario();
        let raiz_proc = e.base.join("proc");
        fs::create_dir_all(&raiz_proc).expect("proc");
        let id = format!("corre-memo-ecc-test-100-{PID_MUERTO}-0");
        corrida_con_copia(&e.raiz, &id);
        barrer_con_proc(&e.raiz, &e.eventos, &raiz_proc).expect("barre");
        let gates = gates_de_barrido(&e.eventos);
        assert_eq!(gates.len(), 1);
        assert!(
            motivo(&gates[0]).contains("no vive"),
            "{}",
            motivo(&gates[0])
        );
    }

    #[test]
    fn enlace_de_credencial_se_borra_sin_tocar_su_destino() {
        let e = escenario();
        let id = format!("corre-memo-ecc-test-100-{PID_MUERTO}-0");
        let fuera = e.base.join("fuera");
        fs::create_dir_all(&fuera).expect("fuera");
        let destino = fuera.join("destino.json");
        fs::write(&destino, "dato-ajeno").expect("destino");
        let estado = e.raiz.join(&id).join("claude-estado");
        fs::create_dir_all(&estado).expect("estado");
        let enlace = estado.join(".credentials.json");
        symlink(&destino, &enlace).expect("enlace");
        barrer(&e.raiz, &e.eventos).expect("barre");
        assert!(
            fs::symlink_metadata(&enlace).is_err(),
            "el enlace se borra como enlace"
        );
        assert_eq!(
            fs::read_to_string(&destino).expect("destino"),
            "dato-ajeno",
            "el destino del enlace queda intacto"
        );
    }

    #[test]
    fn id_sin_pid_legible_se_barre() {
        let e = escenario();
        let sin_pid = corrida_con_copia(&e.raiz, "corre-memo-ecc-test-sin-pid");
        let solo = corrida_con_copia(&e.raiz, "corre-solo");
        barrer(&e.raiz, &e.eventos).expect("barre");
        assert!(fs::symlink_metadata(&sin_pid).is_err(), "pid no numérico");
        assert!(fs::symlink_metadata(&solo).is_err(), "faltan segmentos");
        assert_eq!(gates_de_barrido(&e.eventos).len(), 2, "un Gate por corrida");
        assert!(sin_token(&e.eventos));
    }

    #[test]
    fn error_al_borrar_una_corrida_no_corta_las_demas() {
        let e = escenario();
        // La carpeta de estado es un enlace a `fuera/`: el borrado la rechaza y no toca nada.
        let fallida = format!("corre-memo-ecc-test-1-{PID_MUERTO}-0");
        let fuera = e.base.join("fuera");
        fs::create_dir_all(&fuera).expect("fuera");
        let ajena = fuera.join(".credentials.json");
        fs::write(&ajena, "dato-ajeno").expect("ajena");
        fs::create_dir_all(e.raiz.join(&fallida)).expect("carpeta");
        symlink(&fuera, e.raiz.join(&fallida).join("claude-estado")).expect("enlace de estado");
        let sigue = format!("corre-memo-ecc-test-2-{}-0", PID_MUERTO - 1);
        let copia = corrida_con_copia(&e.raiz, &sigue);

        barrer(&e.raiz, &e.eventos).expect("barre sin cortar");

        assert!(
            fs::symlink_metadata(&copia).is_err(),
            "la otra corrida se barre"
        );
        assert_eq!(fs::read_to_string(&ajena).expect("ajena"), "dato-ajeno");
        let gates = gates_de_barrido(&e.eventos);
        assert_eq!(gates.len(), 2, "un Gate por corrida: el error y el barrido");
        let de_fallida = gates
            .iter()
            .find(|ev| ev.corrida == fallida)
            .expect("Gate de la corrida fallida");
        assert!(motivo(de_fallida).contains("no se pudo borrar"));
        assert!(gates.iter().any(|ev| ev.corrida == sigue));
        assert!(sin_token(&e.eventos));
    }

    #[test]
    fn carpetas_que_no_son_directorio_real_se_saltan() {
        let e = escenario();
        let id = format!("corre-memo-ecc-test-1-{PID_MUERTO}-0");
        // Una carpeta de corrida que es un enlace a otra carpeta: no es directorio real, se salta.
        let destino = e.base.join("fuera_corrida");
        let copia = corrida_con_copia(&e.base, "fuera_corrida");
        symlink(&destino, e.raiz.join(&id)).expect("enlace de corrida");
        barrer(&e.raiz, &e.eventos).expect("barre");
        assert!(copia.is_file(), "la copia del destino del enlace queda");
        assert!(gates_de_barrido(&e.eventos).is_empty(), "sin Gate");
    }

    #[test]
    fn raiz_inexistente_no_hace_nada() {
        let e = escenario();
        let inexistente = e.base.join("no-hay");
        barrer(&inexistente, &e.eventos).expect("sin raíz no pasa nada");
        assert!(!e.eventos.exists(), "sin eventos");
    }
}
