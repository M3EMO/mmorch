//! Contenedor del builder de `r00t corre` (Tarea 26, capa 1, decisión 43): argumentos de `podman run` y
//! validación de los montajes. El builder ve solo la carpeta de su corrida, su estado y la credencial de
//! Claude en solo lectura. Nunca ve `ROOT_HOME` ni el resto de `$HOME`. Aquí no se lanza nada: lo hace
//! `ejecutor.rs`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::ErrorCorre;
use super::ejecutor::Lanzamiento;
use crate::oracle::contenedor::PROXY_FUERA;

/// Imagen del builder. Va en la CLI y no en `root.toml`, para no tocar el esquema de `Config` (duda de la
/// Tarea 26).
pub const IMAGEN_BUILDER: &str = "root-builder:1";
/// Tope de cada lanzamiento del builder. Si vence, el contenedor se borra.
pub const TIMEOUT_BUILDER: Duration = Duration::from_secs(2 * 60 * 60);

/// Carpeta del estado de `claude`, dentro de la carpeta de la corrida (fuera de `repo/`).
const CARPETA_ESTADO: &str = "claude-estado";
/// Destino del estado de `claude` dentro del contenedor: el HOME del builder.
const ESTADO_EN_CONTENEDOR: &str = "/home/builder/.claude";
/// Destino de la credencial dentro del contenedor.
const CREDENCIAL_EN_CONTENEDOR: &str = "/home/builder/.claude/.credentials.json";
/// Variables que no pasan al contenedor, aunque el lanzamiento las traiga. Es la segunda defensa: la
/// primera es `env_del_builder`. `HOME` y `PATH` los fija la imagen.
const VARIABLES_PROHIBIDAS: &[&str] = &[
    "ROOT_HOME",
    "DEEPSEEK_API_KEY",
    "ANTHROPIC_API_KEY",
    "HOME",
    "PATH",
];

/// Credencial de Claude del host: `<home_usuario>/.claude/.credentials.json`.
pub fn credencial_de(home_usuario: &Path) -> PathBuf {
    home_usuario.join(".claude").join(".credentials.json")
}

/// Carpeta de estado de `claude` de una corrida: hermana de `repo`, dentro de la carpeta de la corrida.
pub fn estado_de(repo: &Path) -> Option<PathBuf> {
    repo.parent().map(|corrida| corrida.join(CARPETA_ESTADO))
}

/// Argumentos de `podman run` para el builder, sin el binario `podman`. La red es
/// `pasta:--no-map-gw` (desempate 10 a). `repo` se monta con escritura en `/trabajo`. `estado` es el HOME
/// del builder. La credencial entra en solo lectura. Después de la imagen viene `claude` y `l.args`.
pub fn args_podman_builder(
    l: &Lanzamiento,
    imagen: &str,
    credencial: &Path,
    estado: &Path,
    nombre: &str,
) -> Vec<String> {
    let mut args: Vec<String> = [
        "run",
        "--rm",
        "--name",
        nombre,
        "--network",
        "pasta:--no-map-gw",
        "--userns=keep-id",
        PROXY_FUERA,
        "-w",
        "/trabajo",
    ]
    .map(String::from)
    .into();
    montaje(&mut args, &l.repo, "/trabajo", "rw,Z");
    montaje(&mut args, estado, ESTADO_EN_CONTENEDOR, "rw,Z");
    montaje(&mut args, credencial, CREDENCIAL_EN_CONTENEDOR, "ro,Z");
    for (clave, valor) in &l.env {
        if VARIABLES_PROHIBIDAS.contains(&clave.as_str()) {
            continue;
        }
        args.push("-e".into());
        args.push(format!("{clave}={valor}"));
    }
    args.push(imagen.into());
    args.push("claude".into());
    args.extend(l.args.iter().cloned());
    args
}

/// Agrega un `-v origen:destino:opciones`.
fn montaje(args: &mut Vec<String>, origen: &Path, destino: &str, opciones: &str) {
    args.push("-v".into());
    args.push(format!("{}:{destino}:{opciones}", origen.display()));
}

/// Revisa que ningún montaje sea `ROOT_HOME`, esté dentro de él o lo contenga. Lo mismo vale para
/// `$HOME`. Compara rutas físicas, así un enlace no saca al montaje de la regla. `montajes` trae el
/// nombre de cada montaje y su ruta.
pub fn validar_montajes(
    home: &Path,
    home_usuario: &Path,
    montajes: &[(&str, &Path)],
) -> Result<(), ErrorCorre> {
    let raiz = fisica(home, "ROOT_HOME")?;
    let usuario = fisica(home_usuario, "HOME")?;
    for (nombre, ruta) in montajes {
        let real = fisica(ruta, nombre)?;
        if real.starts_with(&raiz) || raiz.starts_with(&real) {
            return Err(ErrorCorre::Ejecutor(format!(
                "el montaje {nombre} {} toca ROOT_HOME {}",
                real.display(),
                raiz.display()
            )));
        }
        if usuario.starts_with(&real) {
            return Err(ErrorCorre::Ejecutor(format!(
                "el montaje {nombre} {} toca HOME {}",
                real.display(),
                usuario.display()
            )));
        }
    }
    Ok(())
}

/// Ruta física de `ruta`, con su nombre para el error. Resuelve enlaces del tramo que existe.
pub(super) fn fisica(ruta: &Path, nombre: &str) -> Result<PathBuf, ErrorCorre> {
    super::ruta_fisica(ruta).map_err(|fuente| ErrorCorre::Io {
        contexto: format!("resolver {nombre}"),
        fuente,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tipos::Rol;
    use std::fs;
    use std::os::unix::fs::symlink;
    use tempfile::TempDir;

    fn lanzamiento(repo: PathBuf, env: Vec<(String, String)>) -> Lanzamiento {
        Lanzamiento {
            rol: Rol::Construye,
            args: ["-p", "haz hecho", "--model", "claude-haiku-5-5"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            env,
            repo,
        }
    }

    fn par(clave: &str, valor: &str) -> (String, String) {
        (clave.to_string(), valor.to_string())
    }

    fn contiene_par(args: &[String], a: &str, b: &str) -> bool {
        args.windows(2).any(|p| p[0] == a && p[1] == b)
    }

    /// Orígenes de los `-v` de `args`, en orden. Las rutas de prueba no llevan `:`.
    fn origenes_de(args: &[String]) -> Vec<String> {
        args.windows(2)
            .filter(|p| p[0] == "-v")
            .map(|p| p[1].split(':').next().unwrap_or_default().to_string())
            .collect()
    }

    /// `ROOT_HOME` con nombre único en `<tmp>/rootpruebaunico`. `$HOME` en `<tmp>/usuario`. Una corrida en
    /// `$HOME/.cache/root/corridas/corre-1/` con `repo/` y `claude-estado/`. La credencial de Claude
    /// bajo `$HOME/.claude/`.
    struct Disposicion {
        _tmp: TempDir,
        root: PathBuf,
        usuario: PathBuf,
        repo: PathBuf,
        estado: PathBuf,
        credencial: PathBuf,
    }

    fn disposicion() -> Disposicion {
        let tmp = TempDir::new().expect("tmp");
        let root = tmp.path().join("rootpruebaunico");
        let usuario = tmp.path().join("usuario");
        let corrida = usuario.join(".cache/root/corridas/corre-1");
        let repo = corrida.join("repo");
        let estado = corrida.join("claude-estado");
        let credencial = usuario.join(".claude/.credentials.json");
        fs::create_dir_all(&root).expect("root");
        fs::create_dir_all(&repo).expect("repo");
        fs::create_dir_all(&estado).expect("estado");
        fs::create_dir_all(credencial.parent().expect("padre")).expect("claude");
        fs::write(&credencial, "{\"token\":\"secreto-de-prueba\"}").expect("credencial");
        Disposicion {
            _tmp: tmp,
            root,
            usuario,
            repo,
            estado,
            credencial,
        }
    }

    fn argumentos(d: &Disposicion, l: &Lanzamiento) -> Vec<String> {
        args_podman_builder(l, "root-builder:1", &d.credencial, &d.estado, "builder-1")
    }

    #[test]
    fn builder_args_traen_red_pasta_sin_gateway() {
        let d = disposicion();
        let args = argumentos(&d, &lanzamiento(d.repo.clone(), Vec::new()));
        assert!(
            contiene_par(&args, "--network", "pasta:--no-map-gw"),
            "{args:?}"
        );
    }

    #[test]
    fn builder_args_montan_el_repo_rw_en_trabajo_y_fijan_el_directorio() {
        let d = disposicion();
        let args = argumentos(&d, &lanzamiento(d.repo.clone(), Vec::new()));
        assert!(
            args.contains(&format!("{}:/trabajo:rw,Z", d.repo.display())),
            "{args:?}"
        );
        assert!(contiene_par(&args, "-w", "/trabajo"), "{args:?}");
    }

    #[test]
    fn builder_args_montan_el_estado_en_el_home_del_builder() {
        let d = disposicion();
        let args = argumentos(&d, &lanzamiento(d.repo.clone(), Vec::new()));
        assert!(
            args.contains(&format!(
                "{}:/home/builder/.claude:rw,Z",
                d.estado.display()
            )),
            "{args:?}"
        );
    }

    #[test]
    fn builder_args_montan_la_credencial_en_solo_lectura() {
        let d = disposicion();
        let args = argumentos(&d, &lanzamiento(d.repo.clone(), Vec::new()));
        assert!(
            args.contains(&format!(
                "{}:/home/builder/.claude/.credentials.json:ro,Z",
                d.credencial.display()
            )),
            "{args:?}"
        );
    }

    #[test]
    fn builder_args_llevan_nombre_rm_keep_id_y_proxy_fuera() {
        let d = disposicion();
        let args = argumentos(&d, &lanzamiento(d.repo.clone(), Vec::new()));
        assert!(contiene_par(&args, "--name", "builder-1"), "{args:?}");
        assert!(args.contains(&"--rm".to_string()), "{args:?}");
        assert!(args.contains(&"--userns=keep-id".to_string()), "{args:?}");
        let proxy = args.iter().position(|a| a == "--http-proxy=false");
        let imagen = args.iter().position(|a| a == "root-builder:1");
        assert!(proxy.is_some() && proxy < imagen, "{args:?}");
    }

    #[test]
    fn builder_args_terminan_con_imagen_claude_y_argumentos() {
        let d = disposicion();
        let l = lanzamiento(d.repo.clone(), Vec::new());
        let args = argumentos(&d, &l);
        let pos = args.len() - l.args.len() - 2;
        assert_eq!(args[pos], "root-builder:1", "{args:?}");
        assert_eq!(args[pos + 1], "claude", "{args:?}");
        assert_eq!(&args[pos + 2..], l.args.as_slice(), "{args:?}");
    }

    #[test]
    fn builder_args_pasan_las_variables_del_lanzamiento_como_e() {
        let d = disposicion();
        let l = lanzamiento(d.repo.clone(), vec![par("ECC_DISABLED_HOOKS", "a,b")]);
        let args = argumentos(&d, &l);
        assert!(
            contiene_par(&args, "-e", "ECC_DISABLED_HOOKS=a,b"),
            "{args:?}"
        );
    }

    #[test]
    fn builder_args_no_pasan_claves_ni_root_home_aunque_el_lanzamiento_las_traiga() {
        let d = disposicion();
        let l = lanzamiento(
            d.repo.clone(),
            vec![
                par("PATH", "/usr/bin"),
                par("ROOT_HOME", "/datos/rootpruebaunico"),
                par("DEEPSEEK_API_KEY", "clave-deepseek-9"),
                par("ANTHROPIC_API_KEY", "clave-anthropic-9"),
                par("HOME", "/casa"),
                par("ECC_DISABLED_HOOKS", "a,b"),
            ],
        );
        let args = argumentos(&d, &l);
        for prohibido in [
            "ROOT_HOME",
            "DEEPSEEK_API_KEY",
            "ANTHROPIC_API_KEY",
            "clave-deepseek-9",
            "clave-anthropic-9",
        ] {
            assert!(
                args.iter().all(|a| !a.contains(prohibido)),
                "{prohibido} no debe pasar: {args:?}"
            );
        }
        assert!(
            args.iter()
                .all(|a| !a.starts_with("HOME=") && !a.starts_with("PATH=")),
            "HOME y PATH los fija la imagen: {args:?}"
        );
        assert!(
            contiene_par(&args, "-e", "ECC_DISABLED_HOOKS=a,b"),
            "{args:?}"
        );
    }

    #[test]
    fn builder_args_no_nombran_root_home_en_ningun_argumento() {
        let d = disposicion();
        let l = lanzamiento(
            d.repo.clone(),
            vec![par("ROOT_HOME", &d.root.display().to_string())],
        );
        let args = argumentos(&d, &l);
        let root = d.root.display().to_string();
        assert!(
            args.iter().all(|a| !a.contains(&root)),
            "ROOT_HOME {root} aparece en {args:?}"
        );
    }

    #[test]
    fn builder_montajes_son_solo_la_corrida_y_la_credencial() {
        let d = disposicion();
        let args = argumentos(&d, &lanzamiento(d.repo.clone(), Vec::new()));
        assert_eq!(
            origenes_de(&args),
            vec![
                d.repo.display().to_string(),
                d.estado.display().to_string(),
                d.credencial.display().to_string(),
            ],
            "{args:?}"
        );
        for origen in origenes_de(&args) {
            let origen = PathBuf::from(&origen);
            assert!(
                origen != d.usuario && !d.usuario.starts_with(&origen),
                "monta $HOME entero: {}",
                origen.display()
            );
        }
    }

    #[test]
    fn builder_montajes_no_tocan_root_home() {
        // Guarda de la capa 1 (Validate de la Tarea 26), sin podman: ningún origen es ROOT_HOME, está
        // dentro de él ni lo contiene.
        // Prueba de la laptop, con podman y la imagen construida: en una corrida armada,
        //   podman run --rm --network pasta:--no-map-gw --userns=keep-id \
        //     -v <repo>:/trabajo:rw,Z -v <estado>:/home/builder/.claude:rw,Z \
        //     root-builder:1 ls <ROOT_HOME>/oraculos
        // debe fallar con "No such file or directory". El builder no ve ROOT_HOME.
        let d = disposicion();
        let args = argumentos(&d, &lanzamiento(d.repo.clone(), Vec::new()));
        for origen in origenes_de(&args) {
            let origen = PathBuf::from(&origen);
            assert!(
                origen != d.root && !origen.starts_with(&d.root) && !d.root.starts_with(&origen),
                "el montaje {} toca ROOT_HOME {}",
                origen.display(),
                d.root.display()
            );
        }
    }

    #[test]
    fn builder_credencial_cuelga_de_home_de_usuario() {
        assert_eq!(
            credencial_de(Path::new("/h/u")),
            PathBuf::from("/h/u/.claude/.credentials.json")
        );
    }

    #[test]
    fn builder_estado_es_hermano_del_repo() {
        assert_eq!(
            estado_de(Path::new("/c/corre-1/repo")),
            Some(PathBuf::from("/c/corre-1/claude-estado"))
        );
    }

    #[test]
    fn builder_montaje_dentro_de_root_home_es_error() {
        let d = disposicion();
        let dentro = d.root.join("corrida").join("repo");
        fs::create_dir_all(&dentro).expect("dentro");
        let error = validar_montajes(
            &d.root,
            &d.usuario,
            &[("repo", &dentro), ("estado", &d.estado)],
        )
        .expect_err("el repo dentro de ROOT_HOME debe fallar");
        assert!(
            matches!(error, ErrorCorre::Ejecutor(ref m) if m.contains("toca ROOT_HOME")),
            "{error}"
        );
    }

    #[test]
    fn builder_montaje_estado_dentro_de_root_home_es_error() {
        let d = disposicion();
        let dentro = d.root.join("estado");
        fs::create_dir_all(&dentro).expect("dentro");
        let error = validar_montajes(
            &d.root,
            &d.usuario,
            &[("repo", &d.repo), ("estado", &dentro)],
        )
        .expect_err("el estado dentro de ROOT_HOME debe fallar");
        assert!(
            matches!(error, ErrorCorre::Ejecutor(ref m) if m.contains("estado")),
            "{error}"
        );
    }

    #[test]
    fn builder_montaje_igual_a_root_home_o_que_lo_contiene_es_error() {
        let d = disposicion();
        let padre = d.root.parent().expect("padre").to_path_buf();
        for ruta in [d.root.clone(), padre] {
            let error = validar_montajes(&d.root, &d.usuario, &[("repo", &ruta)])
                .expect_err("un montaje que toca ROOT_HOME debe fallar");
            assert!(
                matches!(error, ErrorCorre::Ejecutor(ref m) if m.contains("toca ROOT_HOME")),
                "{}: {error}",
                ruta.display()
            );
        }
    }

    #[test]
    fn builder_montaje_igual_a_home_o_que_lo_contiene_es_error() {
        // ROOT_HOME queda fuera de la carpeta padre de HOME, así la regla de HOME se prueba sola.
        let tmp = TempDir::new().expect("tmp");
        let root = tmp.path().join("root");
        let casa = tmp.path().join("casa");
        let usuario = casa.join("usuario");
        fs::create_dir_all(&root).expect("root");
        fs::create_dir_all(&usuario).expect("usuario");
        for ruta in [usuario.clone(), casa] {
            let error = validar_montajes(&root, &usuario, &[("repo", &ruta)])
                .expect_err("un montaje que toca HOME debe fallar");
            assert!(
                matches!(error, ErrorCorre::Ejecutor(ref m) if m.contains("toca HOME")),
                "{}: {error}",
                ruta.display()
            );
        }
    }

    #[test]
    fn builder_montaje_dentro_de_home_fuera_de_root_es_valido() {
        let d = disposicion();
        validar_montajes(
            &d.root,
            &d.usuario,
            &[
                ("repo", &d.repo),
                ("estado", &d.estado),
                ("credencial", &d.credencial),
            ],
        )
        .expect("la corrida bajo HOME, fuera de ROOT_HOME, es válida");
    }

    #[test]
    fn builder_montaje_por_enlace_a_root_home_es_error() {
        let d = disposicion();
        let enlace = d.usuario.join(".cache/root/corridas/corre-2/repo");
        fs::create_dir_all(enlace.parent().expect("padre")).expect("corridas");
        symlink(&d.root, &enlace).expect("enlace");
        let error = validar_montajes(&d.root, &d.usuario, &[("repo", &enlace)])
            .expect_err("un enlace a ROOT_HOME debe fallar");
        assert!(
            matches!(error, ErrorCorre::Ejecutor(ref m) if m.contains("toca ROOT_HOME")),
            "{error}"
        );
    }
}
