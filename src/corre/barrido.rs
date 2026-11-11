//! Barrido de copias de credencial de corridas muertas. Al arrancar una corrida, quita la copia
//! `.credentials.json` de las carpetas de corridas cuyo proceso ya no vive o no es `r00t`. Deja un
//! `Gate` por cada copia barrida. El `Gate` nunca lleva el contenido de la copia.

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt;
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
/// Nombre del programa `r00t`. Su proceso es el único que puede dejar una corrida viva.
const NOMBRE_R00T: &str = "r00t";

/// Recorre las carpetas de corridas de `raiz_corridas` y barre las copias de corridas muertas.
/// Si la raíz no existe, no hace nada. Una corrida que no se puede borrar deja un `Gate` y no corta
/// el barrido de las demás.
pub fn barrer(raiz_corridas: &Path, ruta_eventos: &Path) -> Result<(), ErrorCorre> {
    barrer_con_proc(raiz_corridas, ruta_eventos, Path::new(PROCESOS))
}

/// Igual que `barrer`, con la raíz de `/proc` inyectable. Los tests arman una raíz falsa.
pub fn barrer_con_proc(
    raiz_corridas: &Path,
    ruta_eventos: &Path,
    raiz_proc: &Path,
) -> Result<(), ErrorCorre> {
    let entradas = match fs::read_dir(raiz_corridas) {
        Ok(entradas) => entradas,
        Err(fuente) if fuente.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(fuente) => {
            return Err(ErrorCorre::Io {
                contexto: format!("leer {}", raiz_corridas.display()),
                fuente,
            });
        }
    };
    for entrada in entradas {
        let carpeta = entrada
            .map_err(|fuente| ErrorCorre::Io {
                contexto: format!("leer {}", raiz_corridas.display()),
                fuente,
            })?
            .path();
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
                gate(ruta_eventos, id, motivo)?;
                continue;
            }
        }
        let Some(por_que) = motivo_de_muerte(id, raiz_proc) else {
            continue;
        };
        let motivo = match borrado::borrar_archivo_dentro(&carpeta, &copia) {
            Ok(()) => format!("copia de la corrida {id} barrida: {por_que}"),
            Err(error) => format!("no se pudo borrar la copia de la corrida {id}: {error}"),
        };
        gate(ruta_eventos, id, motivo)?;
    }
    Ok(())
}

/// Por qué la corrida `id` está muerta. `None` si su proceso vive y es `r00t`.
/// Un pid sin carpeta en `raiz_proc` no vive. Uno cuyo cmdline no es de `r00t` está muerto: el pid
/// se reusó. Un cmdline que no se puede leer por permisos cuenta como vivo.
fn motivo_de_muerte(id: &str, raiz_proc: &Path) -> Option<String> {
    let Some(pid) = pid_de(id) else {
        return Some("el id no tiene pid legible".to_string());
    };
    let carpeta = raiz_proc.join(pid.to_string());
    if !carpeta.exists() {
        return Some(format!("el pid {pid} no vive"));
    }
    match fs::read(carpeta.join("cmdline")) {
        Ok(cmdline) if es_r00t(&cmdline) => None,
        Ok(_) => Some(format!("el pid {pid} no es r00t")),
        Err(fuente) if fuente.kind() == io::ErrorKind::PermissionDenied => None,
        Err(fuente) if fuente.kind() == io::ErrorKind::NotFound => {
            Some(format!("el pid {pid} no vive"))
        }
        Err(_) => Some(format!("el pid {pid} no es r00t")),
    }
}

/// Dice si el cmdline (argumentos separados por NUL) empieza con un programa `r00t`. Mira solo el
/// primer argumento, por su nombre base. Un cmdline vacío no es `r00t`.
fn es_r00t(cmdline: &[u8]) -> bool {
    let primero = cmdline.split(|byte| *byte == 0).next().unwrap_or_default();
    Path::new(OsStr::from_bytes(primero))
        .file_name()
        .is_some_and(|nombre| nombre == NOMBRE_R00T)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
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

    /// Test que cambió de valor: antes el pid propio contaba como vivo. Ahora la regla es el cmdline:
    /// el proceso de test no es `r00t`, así que su corrida se barre.
    #[test]
    fn corrida_con_pid_del_proceso_de_test_no_r00t_se_barre() {
        let e = escenario();
        let id = format!("corre-memo-ecc-test-100-{}-0", std::process::id());
        let copia = corrida_con_copia(&e.raiz, &id);
        barrer(&e.raiz, &e.eventos).expect("barre");
        assert!(
            fs::symlink_metadata(&copia).is_err(),
            "el proceso de test no es r00t: la copia se barre"
        );
        let gates = gates_de_barrido(&e.eventos);
        assert_eq!(gates.len(), 1, "un Gate de barrido");
        assert!(motivo(&gates[0]).contains("no es r00t"), "motivo nuevo");
        assert!(sin_token(&e.eventos));
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
