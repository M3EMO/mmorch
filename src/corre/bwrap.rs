//! Ejecutor del builder con bubblewrap (Tarea 171). Corre `claude` dentro de `bwrap`, como el usuario del
//! builder y en el netns `ROOT_RED_BUILDER`. Es el ejecutor de podman sin contenedor: el builder ve solo lo
//! que `bwrap` monta. Aquí no se monta nada del host fuera de la lista de `args_bwrap_builder`.
//!
//! Orden del comando: `ip netns exec <red> prlimit --nproc=4096 setpriv ... bwrap <montajes> claude <args>`.
//! `prlimit` va antes de `setpriv` para que el tope de procesos (RLIMIT_NPROC, por uid real) ya esté puesto
//! cuando el builder cambia de uid. El entorno no va en argv: el `Command` hace `env_clear`.
//!
//! Heurística de infraestructura: un `bwrap`, `setpriv`, `prlimit` o `ip` que falla sale con código 126 o
//! 127, o con código distinto de 0, sin stdout y con stderr que empieza por `bwrap:`, `setpriv:`, `prlimit:`,
//! `ip:` o `Cannot open network namespace`. Eso da `ErrorCorre::Ejecutor`. Cualquier otra salida es de claude.
//! Un código 137 (SIGKILL) da `Cortado` y conserva el stdout, como en podman.

use std::fs;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use super::args::PLUGIN_EN_CONTENEDOR;
use super::contenedor::{
    CREDENCIAL_EN_CONTENEDOR, ESTADO_EN_CONTENEDOR, TIMEOUT_BUILDER, VARIABLES_PROHIBIDAS,
    estado_de, fisica, validar_montajes,
};
use super::ejecutor::{
    CODIGO_SIGKILL, CREDENCIAL_COPIA, EjecutorBuilder, Fin, Lanzamiento, SalidaClaude,
    correr_comando_registrado, revisar_credencial, revisar_plugin, senal_termino_el_ejecutor,
};
use super::{ErrorCorre, copiar_sin_mcp};
use crate::oracle::contenedor::TOPE_SALIDA_BUILDER;

/// HOME del builder dentro de bwrap. Es el destino del estado de claude y de la credencial.
const HOME_BUILDER: &str = "/home/builder";
/// PATH fijo del builder. No hereda el del proceso de r00t.
const PATH_FIJO: &str = "/usr/local/bin:/usr/bin:/bin";
/// Tope de procesos del builder (RLIMIT_NPROC). Frena una bomba de procesos. Va con `prlimit`.
const LIMITE_PROCESOS: u32 = 4096;
/// Comando de claude dentro del sandbox.
const CLAUDE: &str = "claude";
/// Variable del token de Claude. Va por el entorno del `Command`, nunca por argv.
const VARIABLE_TOKEN: &str = "CLAUDE_CODE_OAUTH_TOKEN";
/// Prefijos de stderr que delatan un fallo de infraestructura y no de claude.
const PREFIJOS_INFRA: [&str; 5] = [
    "bwrap:",
    "setpriv:",
    "prlimit:",
    "ip:",
    "Cannot open network namespace",
];
/// Carpetas del host que se montan en solo lectura si existen.
const RAIZ_SOLO_LECTURA: [&str; 3] = ["/usr", "/etc", "/opt"];
/// Enlaces de la raíz del host. Si son carpeta se montan; si son enlace se copian como enlace.
const RAIZ_ENLACES: [&str; 4] = ["/bin", "/lib", "/lib64", "/sbin"];
/// Motivo de un builder matado con SIGKILL. Quien llama lo lee junto con el stdout conservado.
const MENSAJE_SIGKILL: &str = "el builder fue matado por SIGKILL, a menudo por falta de memoria";

/// Ejecutor del builder con bubblewrap. Nunca lo corre en el host sin `bwrap`.
pub struct EjecutorBwrapBuilder {
    /// Credencial de Claude del host. Se copia sin `mcpOAuth` y entra en solo lectura.
    pub credencial: PathBuf,
    /// `ROOT_HOME`. Ningún montaje lo toca.
    pub home: PathBuf,
    /// `$HOME` del host. Ningún montaje es igual a él ni lo contiene.
    pub home_usuario: PathBuf,
    /// Tope de cada lanzamiento. El rol corre con el menor entre su plazo y este tope.
    pub timeout: Duration,
    /// Netns donde corre el builder: `ROOT_RED_BUILDER`.
    pub red: String,
    /// Usuario del builder: `ROOT_USUARIO_BUILDER`. Su uid y gid salen de `/etc/passwd`.
    pub usuario: String,
    /// Archivo con el token de Claude: `ROOT_TOKEN_BUILDER`. Sin él se usa la copia de la credencial.
    pub token: Option<PathBuf>,
    /// Programa `ip`. Los tests lo cambian por uno falso.
    pub ip: String,
    /// Programa `prlimit`. Los tests lo cambian por uno falso.
    pub prlimit: String,
    /// Programa `setpriv`. Los tests lo cambian por uno falso.
    pub setpriv: String,
    /// Programa `bwrap`. Los tests lo cambian por uno falso.
    pub bwrap: String,
}

/// Lo que hay en una ruta de la raíz del host: nada, una carpeta o un enlace con su destino.
#[derive(Debug, Clone, PartialEq, Eq)]
enum EntradaRaiz {
    NoExiste,
    Dir,
    Enlace(String),
}

/// Dueño y permisos de una carpeta. `modo` lleva solo los bits de permiso.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Permisos {
    uid: u32,
    gid: u32,
    modo: u32,
}

impl EjecutorBwrapBuilder {
    /// Arma el ejecutor leyendo `ROOT_RED_BUILDER`, `ROOT_USUARIO_BUILDER` y `ROOT_TOKEN_BUILDER` de
    /// `entorno`, no del proceso. Red y usuario son obligatorios y no vacíos. El token, si está, no puede
    /// estar vacío. El archivo del token se lee al lanzar, no aquí.
    pub fn desde_entorno(
        entorno: &[(String, String)],
        credencial: PathBuf,
        home: PathBuf,
        home_usuario: PathBuf,
    ) -> Result<Self, ErrorCorre> {
        let buscar = |clave: &str| {
            entorno
                .iter()
                .find(|(k, _)| k == clave)
                .map(|(_, v)| v.clone())
        };
        let red = buscar("ROOT_RED_BUILDER")
            .filter(|v| !v.is_empty())
            .ok_or_else(|| {
                ErrorCorre::Ejecutor("falta ROOT_RED_BUILDER: el netns del builder".into())
            })?;
        let usuario = buscar("ROOT_USUARIO_BUILDER")
            .filter(|v| !v.is_empty())
            .ok_or_else(|| {
                ErrorCorre::Ejecutor("falta ROOT_USUARIO_BUILDER: el usuario del builder".into())
            })?;
        let token = match buscar("ROOT_TOKEN_BUILDER") {
            None => None,
            Some(ruta) if ruta.is_empty() => {
                return Err(ErrorCorre::Ejecutor(
                    "ROOT_TOKEN_BUILDER está vacía: nombre un archivo o quítela".into(),
                ));
            }
            Some(ruta) => Some(PathBuf::from(ruta)),
        };
        Ok(Self {
            credencial,
            home,
            home_usuario,
            timeout: TIMEOUT_BUILDER,
            red,
            usuario,
            token,
            ip: "ip".into(),
            prlimit: "prlimit".into(),
            setpriv: "setpriv".into(),
            bwrap: "bwrap".into(),
        })
    }

    /// Revisa lo que debe valer antes de cada lanzamiento: red y usuario, uid del builder distinto del de
    /// quien corre r00t, y permisos de `ROOT_HOME` y de `$HOME`. Devuelve el uid y el gid del builder.
    fn chequeo_previo(&self) -> Result<(u32, u32), ErrorCorre> {
        if self.red.is_empty() {
            return Err(ErrorCorre::Ejecutor("falta ROOT_RED_BUILDER".into()));
        }
        if self.usuario.is_empty() {
            return Err(ErrorCorre::Ejecutor("falta ROOT_USUARIO_BUILDER".into()));
        }
        let passwd = fs::read_to_string("/etc/passwd")
            .map_err(|e| ErrorCorre::Ejecutor(format!("no se pudo leer /etc/passwd: {e}")))?;
        let builder = uid_gid_en(&passwd, &self.usuario)?;
        revisar_permisos(
            uid_propio()?,
            builder,
            permisos_de(&self.home, "ROOT_HOME")?,
            permisos_de(&self.home_usuario, "$HOME")?,
        )?;
        Ok(builder)
    }

    /// Arma el `Command` del lanzamiento sin lanzarlo. `l` y las rutas ya son físicas y validadas. El
    /// entorno sale de cero: `l.env` sin variables prohibidas, `HOME` y `PATH` fijos y, si hay, el token.
    fn armar_comando(
        &self,
        l: &Lanzamiento,
        credencial: Option<&Path>,
        estado: &Path,
        (uid, gid): (u32, u32),
        token: Option<&str>,
    ) -> Command {
        let montajes = args_bwrap_builder(l, credencial, estado, &entrada_de_raiz);
        let mut cmd = Command::new(&self.ip);
        cmd.args(["netns", "exec"])
            .arg(&self.red)
            .arg(&self.prlimit)
            .arg(format!("--nproc={LIMITE_PROCESOS}"))
            .arg(&self.setpriv)
            .arg(format!("--reuid={uid}"))
            .arg(format!("--regid={gid}"))
            .args([
                "--clear-groups",
                "--inh-caps=-all",
                "--bounding-set=-all",
                "--no-new-privs",
            ])
            .arg(&self.bwrap)
            .args(&montajes)
            .arg(CLAUDE)
            .args(&l.args);
        cmd.env_clear();
        for (clave, valor) in &l.env {
            if !VARIABLES_PROHIBIDAS.contains(&clave.as_str()) {
                cmd.env(clave, valor);
            }
        }
        cmd.env("HOME", HOME_BUILDER).env("PATH", PATH_FIJO);
        if let Some(token) = token {
            cmd.env(VARIABLE_TOKEN, token);
        }
        cmd
    }
}

impl EjecutorBuilder for EjecutorBwrapBuilder {
    fn lanzar(&self, l: &Lanzamiento) -> Result<SalidaClaude, ErrorCorre> {
        // Con el ejecutor terminando no se crea copia ni se lanza nada. Bajo el cerrojo se vuelve a mirar.
        if senal_termino_el_ejecutor() {
            return Err(ErrorCorre::Ejecutor(
                "no se lanza el builder: el ejecutor está terminando".into(),
            ));
        }
        let builder = self.chequeo_previo()?;
        // Con token el archivo se revisa y se lee antes de tocar el estado. Con token no hay credencial que
        // copiar ni montar; sin token la credencial es obligatoria, como en la 171.
        let token = self.token.as_deref().map(leer_token).transpose()?;
        if token.is_none() {
            revisar_credencial(&self.credencial)?;
        }
        if let Some(plugin) = &l.plugin {
            revisar_plugin(plugin)?;
        }

        let estado = estado_de(&l.repo).ok_or_else(|| {
            ErrorCorre::Ejecutor(format!(
                "el repo de la corrida no tiene carpeta padre: {}",
                l.repo.display()
            ))
        })?;
        let credencial = token.is_none().then_some(self.credencial.as_path());
        let mut montajes: Vec<(&str, &Path)> =
            vec![("repo", l.repo.as_path()), ("estado", estado.as_path())];
        if let Some(credencial) = credencial {
            montajes.push(("credencial", credencial));
        }
        if let Some(plugin) = &l.plugin {
            montajes.push(("plugin", plugin.as_path()));
        }
        validar_montajes(&self.home, &self.home_usuario, &montajes)?;
        fs::create_dir_all(&estado).map_err(|fuente| ErrorCorre::Io {
            contexto: format!("crear el estado de claude en {}", estado.display()),
            fuente,
        })?;
        // El builder no ve la credencial del usuario: ve una copia sin `mcpOAuth`.
        let copia = match credencial {
            Some(credencial) => {
                let copia = estado.join(CREDENCIAL_COPIA);
                copiar_sin_mcp(credencial, &copia)?;
                Some(copia)
            }
            None => None,
        };

        // Los montajes van por ruta física: lo que se validó es lo que se monta.
        let mut real = l.clone();
        real.repo = fisica(&l.repo, "el repo")?;
        real.plugin = l
            .plugin
            .as_deref()
            .map(|plugin| fisica(plugin, "el plugin"))
            .transpose()?;
        let estado = fisica(&estado, "el estado")?;
        let copia = copia
            .map(|copia| fisica(&copia, "la copia de la credencial"))
            .transpose()?;
        let cmd = self.armar_comando(&real, copia.as_deref(), &estado, builder, token.as_deref());

        let plazo = l.plazo.min(self.timeout);
        match correr_comando_registrado(cmd, &self.ip, plazo, TOPE_SALIDA_BUILDER)? {
            Fin::Termino(salida) => clasificar_salida_bwrap(salida),
            // Lo que alcanzó a dar el builder sigue en `stdout`: quien llama lo guarda y lo cuenta.
            Fin::Vencio { stdout, .. } => Err(ErrorCorre::Cortado {
                mensaje: format!(
                    "timeout tras {} s; se mató el grupo de procesos del builder",
                    plazo.as_secs()
                ),
                stdout,
            }),
        }
    }
}

/// Argumentos de `bwrap` del builder, sin el binario ni `claude`. Función pura: el sistema de la raíz llega
/// por `entrada`. Monta `repo` en `/trabajo`, el estado de claude en el HOME del builder, la copia de la
/// credencial en solo lectura y el plugin si hay. El sistema va en solo lectura. Nunca monta `ROOT_HOME`,
/// `$HOME`, `/root`, `/content` ni `/home` del host. Termina con `--chdir /trabajo`.
/// `credencial` es la copia de la credencial: con token no hay y no se monta nada en su lugar.
fn args_bwrap_builder(
    l: &Lanzamiento,
    credencial: Option<&Path>,
    estado: &Path,
    entrada: &dyn Fn(&str) -> EntradaRaiz,
) -> Vec<String> {
    let mut args: Vec<String> = [
        "--unshare-user",
        "--unshare-pid",
        "--unshare-ipc",
        "--unshare-uts",
        "--unshare-cgroup-try",
        "--die-with-parent",
        "--new-session",
    ]
    .map(String::from)
    .into();
    for ruta in RAIZ_SOLO_LECTURA {
        if entrada(ruta) != EntradaRaiz::NoExiste {
            montar(&mut args, "--ro-bind", ruta, ruta);
        }
    }
    for ruta in RAIZ_ENLACES {
        match entrada(ruta) {
            EntradaRaiz::NoExiste => {}
            EntradaRaiz::Dir => montar(&mut args, "--ro-bind", ruta, ruta),
            EntradaRaiz::Enlace(destino) => montar(&mut args, "--symlink", &destino, ruta),
        }
    }
    args.extend(["--proc", "/proc", "--dev", "/dev", "--tmpfs", "/tmp"].map(String::from));
    montar_ruta(&mut args, "--bind", &l.repo, "/trabajo");
    montar_ruta(&mut args, "--bind", estado, ESTADO_EN_CONTENEDOR);
    if let Some(credencial) = credencial {
        montar_ruta(&mut args, "--ro-bind", credencial, CREDENCIAL_EN_CONTENEDOR);
    }
    if let Some(plugin) = &l.plugin {
        montar_ruta(&mut args, "--ro-bind", plugin, PLUGIN_EN_CONTENEDOR);
    }
    args.extend(["--chdir", "/trabajo"].map(String::from));
    args
}

/// Agrega `bandera origen destino` a `args`.
fn montar(args: &mut Vec<String>, bandera: &str, origen: &str, destino: &str) {
    args.extend([bandera, origen, destino].map(String::from));
}

/// Como `montar`, con `origen` como ruta.
fn montar_ruta(args: &mut Vec<String>, bandera: &str, origen: &Path, destino: &str) {
    montar(args, bandera, &origen.display().to_string(), destino);
}

/// Qué hay en `ruta` del host. Un enlace da su destino, para copiarlo como enlace en el sandbox.
fn entrada_de_raiz(ruta: &str) -> EntradaRaiz {
    match fs::symlink_metadata(ruta) {
        Ok(meta) if meta.file_type().is_symlink() => match fs::read_link(ruta) {
            Ok(destino) => EntradaRaiz::Enlace(destino.display().to_string()),
            Err(_) => EntradaRaiz::NoExiste,
        },
        Ok(_) => EntradaRaiz::Dir,
        Err(_) => EntradaRaiz::NoExiste,
    }
}

/// Lee el token de Claude del archivo `ruta`, sin espacios de sobra. Antes revisa que sea un archivo con modo
/// 0600 o más cerrado. El error nombra la ruta y, si hace falta, el modo; nunca el contenido.
fn leer_token(ruta: &Path) -> Result<String, ErrorCorre> {
    let sin_leer = |e: io::Error| {
        ErrorCorre::Ejecutor(format!(
            "no se pudo leer el token de Claude en {}: {e}",
            ruta.display()
        ))
    };
    let meta = fs::metadata(ruta).map_err(sin_leer)?;
    if !meta.is_file() {
        return Err(ErrorCorre::Ejecutor(format!(
            "el token de Claude no es un archivo: {}",
            ruta.display()
        )));
    }
    let modo = meta.mode() & 0o777;
    if modo & 0o177 != 0 {
        return Err(ErrorCorre::Ejecutor(format!(
            "el token de Claude en {} tiene modo {modo:04o}; debe ser 0600 o más cerrado",
            ruta.display()
        )));
    }
    let texto = fs::read_to_string(ruta).map_err(sin_leer)?;
    let token = texto.trim();
    if token.is_empty() {
        return Err(ErrorCorre::Ejecutor(format!(
            "el token de Claude está vacío: {}",
            ruta.display()
        )));
    }
    Ok(token.to_string())
}

/// Decide qué hacer con la salida del `bwrap` lanzado. Función pura. Un 137 da `Cortado` con el stdout.
/// Un fallo de infraestructura da `Ejecutor`. Cualquier otra salida pasa como `Ok`.
fn clasificar_salida_bwrap(salida: SalidaClaude) -> Result<SalidaClaude, ErrorCorre> {
    if salida.codigo == CODIGO_SIGKILL {
        let detalle = String::from_utf8_lossy(&salida.stderr).trim().to_string();
        let mensaje = if detalle.is_empty() {
            MENSAJE_SIGKILL.to_string()
        } else {
            format!("{MENSAJE_SIGKILL}: {detalle}")
        };
        return Err(ErrorCorre::Cortado {
            mensaje,
            stdout: salida.stdout,
        });
    }
    match fallo_de_infra(&salida) {
        Some(motivo) => Err(ErrorCorre::Ejecutor(motivo)),
        None => Ok(salida),
    }
}

/// Motivo si la salida es de infraestructura (ver la heurística del inicio del módulo). `None` si es de claude.
fn fallo_de_infra(salida: &SalidaClaude) -> Option<String> {
    let stderr = String::from_utf8_lossy(&salida.stderr).trim().to_string();
    let codigo_de_infra = matches!(salida.codigo, 126 | 127);
    let prefijo_de_infra = salida.codigo != 0
        && salida.stdout.is_empty()
        && PREFIJOS_INFRA.iter().any(|p| stderr.starts_with(p));
    if !codigo_de_infra && !prefijo_de_infra {
        return None;
    }
    Some(format!(
        "fallo de infraestructura del builder (código {}): {stderr}",
        salida.codigo
    ))
}

/// Uid efectivo de quien corre r00t. Su proceso en `/proc/self` es de ese uid, sin llamar a libc.
fn uid_propio() -> Result<u32, ErrorCorre> {
    fs::metadata("/proc/self")
        .map(|meta| meta.uid())
        .map_err(|e| ErrorCorre::Ejecutor(format!("no se pudo leer /proc/self: {e}")))
}

/// Dueño y permisos de `ruta`. `nombre` va en el error (`ROOT_HOME`, `$HOME`).
fn permisos_de(ruta: &Path, nombre: &str) -> Result<Permisos, ErrorCorre> {
    let meta = fs::metadata(ruta).map_err(|e| {
        ErrorCorre::Ejecutor(format!(
            "no se pudo leer {nombre} en {}: {e}",
            ruta.display()
        ))
    })?;
    Ok(Permisos {
        uid: meta.uid(),
        gid: meta.gid(),
        modo: meta.mode() & 0o7777,
    })
}

/// Uid y gid del usuario `usuario` según el texto de `/etc/passwd`. Error si no existe o no son números.
fn uid_gid_en(passwd: &str, usuario: &str) -> Result<(u32, u32), ErrorCorre> {
    let linea = passwd
        .lines()
        .find(|l| l.split(':').next() == Some(usuario))
        .ok_or_else(|| {
            ErrorCorre::Ejecutor(format!(
                "no existe el usuario del builder {usuario} en /etc/passwd"
            ))
        })?;
    let campos: Vec<&str> = linea.split(':').collect();
    let numero = |indice: usize| campos.get(indice).and_then(|c| c.parse::<u32>().ok());
    match (numero(2), numero(3)) {
        (Some(uid), Some(gid)) => Ok((uid, gid)),
        _ => Err(ErrorCorre::Ejecutor(format!(
            "la entrada de {usuario} en /etc/passwd no tiene uid y gid numéricos"
        ))),
    }
}

/// Chequeo de permisos antes de lanzar. Función pura. Falla si el builder tiene el uid de quien corre r00t,
/// si `ROOT_HOME` no es de ese uid o tiene algún permiso para grupo u otros, o si el builder puede leer o
/// buscar en `$HOME`. Con el builder dueño de `$HOME`, cuenta el modo del dueño.
fn revisar_permisos(
    propio: u32,
    builder: (u32, u32),
    root_home: Permisos,
    home: Permisos,
) -> Result<(), ErrorCorre> {
    let (uid_builder, gid_builder) = builder;
    if uid_builder == propio {
        return Err(ErrorCorre::Ejecutor(
            "el usuario del builder tiene el mismo uid que quien corre r00t".into(),
        ));
    }
    if root_home.uid != propio || root_home.modo & 0o077 != 0 {
        return Err(ErrorCorre::Ejecutor(format!(
            "ROOT_HOME debe ser de quien corre r00t y tener modo 0700 o más cerrado (tiene {:o})",
            root_home.modo
        )));
    }
    let puede_leer = if home.uid == uid_builder {
        home.modo & 0o500 != 0
    } else if home.gid == gid_builder {
        home.modo & 0o050 != 0
    } else {
        home.modo & 0o005 != 0
    };
    if puede_leer {
        return Err(ErrorCorre::Ejecutor(format!(
            "el builder puede leer o buscar en $HOME (modo {:o}): cierre sus permisos",
            home.modo
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corre::args::PLUGIN_EN_CONTENEDOR;
    use crate::corre::contenedor::TIMEOUT_BUILDER;
    use crate::corre::ejecutor::serie_de_pruebas;
    use crate::prueba_ejecutable::escribir_ejecutable;
    use crate::tipos::Rol;
    use std::collections::BTreeMap;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    use tempfile::TempDir;

    /// Mismo PATH que fija el ejecutor dentro del sandbox y en el lanzamiento.
    const PATH_FIJO: &str = "/usr/local/bin:/usr/bin:/bin";

    /// `setpriv` falso: salta las opciones `--...` y ejecuta el resto.
    const SETPRIV_FALSO: &str = "#!/bin/sh\nwhile [ \"$#\" -gt 0 ] && [ \"${1#--}\" != \"$1\" ]; do shift; done\nexec \"$@\"\n";

    /// `bwrap` falso que anota su entorno y sus argumentos, y da una salida de claude.
    const BWRAP_OK: &str =
        "#!/bin/sh\nenv > DIR/env-hijo\necho \"$@\" > DIR/bwrap-args\necho salida-falsa\n";

    /// Saltea un test de bwrap real si `bwrap` no está o no puede crear espacios de usuario.
    /// Escribe el aviso en stderr con el nombre del test, igual que `ROOT_SIN_PODMAN`.
    macro_rules! saltar_sin_bwrap {
        () => {{
            fn f() {}
            let nombre = ::std::any::type_name_of_val(&f);
            let nombre = nombre.strip_suffix("::f").unwrap_or(nombre);
            if saltar_sin_bwrap(nombre) {
                return;
            }
        }};
    }

    /// Dice si `bwrap` existe y puede crear un espacio de usuario. Sin eso, los tests reales se saltean.
    fn bwrap_usable() -> bool {
        Command::new("bwrap")
            .args(["--unshare-user", "--ro-bind", "/", "/", "true"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|estado| estado.success())
    }

    /// Escribe el aviso y dice si el test se saltea. Forma igual a `entorno::aviso_salteo`.
    fn saltar_sin_bwrap(nombre: &str) -> bool {
        if bwrap_usable() {
            return false;
        }
        use std::io::Write;
        let _ = writeln!(
            std::io::stderr(),
            "SALTEADO sin bwrap usable (PATH o espacios de usuario): {nombre}"
        );
        true
    }

    fn par(clave: &str, valor: &str) -> (String, String) {
        (clave.to_string(), valor.to_string())
    }

    fn lanzamiento(repo: PathBuf, env: Vec<(String, String)>) -> Lanzamiento {
        Lanzamiento {
            rol: Rol::Construye,
            args: ["-p", "haz hecho", "--model", "claude-haiku-5-5"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            env,
            repo,
            plugin: None,
            plazo: TIMEOUT_BUILDER,
        }
    }

    fn modo(ruta: &Path, bits: u32) {
        fs::set_permissions(ruta, fs::Permissions::from_mode(bits)).expect("permisos");
    }

    /// Sistema de prueba: `/usr`, `/etc` y `/bin` son carpetas, `/lib64` es enlace y el resto no existe.
    fn sistema_de_prueba(ruta: &str) -> EntradaRaiz {
        match ruta {
            "/usr" | "/etc" | "/bin" => EntradaRaiz::Dir,
            "/lib64" => EntradaRaiz::Enlace("usr/lib64".into()),
            _ => EntradaRaiz::NoExiste,
        }
    }

    /// Disposición de prueba. `$HOME` en `usuario`, `ROOT_HOME` en `home` (0700), una corrida con `repo` y
    /// `estado`, y la credencial del host bajo `$HOME`. `raiz` es la carpeta temporal entera.
    struct Disposicion {
        _tmp: TempDir,
        raiz: PathBuf,
        home: PathBuf,
        usuario: PathBuf,
        repo: PathBuf,
        estado: PathBuf,
        credencial: PathBuf,
    }

    fn disposicion() -> Disposicion {
        let tmp = TempDir::new().expect("tmp");
        let raiz = tmp.path().to_path_buf();
        let home = raiz.join("root");
        let usuario = raiz.join("usuario");
        let corrida = usuario.join(".cache/root/corridas/corre-1");
        let repo = corrida.join("repo");
        let estado = corrida.join("claude-estado");
        let credencial = usuario.join(".claude/.credentials.json");
        for carpeta in [&home, &repo, &estado] {
            fs::create_dir_all(carpeta).expect("carpeta");
        }
        fs::create_dir_all(credencial.parent().expect("padre")).expect("claude");
        fs::write(
            &credencial,
            "{\"claudeAiOauth\":{\"accessToken\":\"falso-de-prueba\"}}",
        )
        .expect("credencial");
        modo(&home, 0o700);
        modo(&usuario, 0o700);
        Disposicion {
            _tmp: tmp,
            raiz,
            home,
            usuario,
            repo,
            estado,
            credencial,
        }
    }

    /// Programas falsos `ip`, `prlimit`, `setpriv` y `bwrap`. Cada uno pasa el resto a la siguiente pieza.
    /// `cuerpo_bwrap` es el script del `bwrap` falso. `DIR` en él se cambia por la carpeta `dir`.
    fn cadena_falsa(dir: &Path, cuerpo_bwrap: &str) -> [String; 4] {
        let ip = escribir_ejecutable(&dir.join("ip-falso"), "#!/bin/sh\nshift 3\nexec \"$@\"\n");
        let prlimit = escribir_ejecutable(
            &dir.join("prlimit-falso"),
            "#!/bin/sh\nshift\nexec \"$@\"\n",
        );
        let setpriv = escribir_ejecutable(&dir.join("setpriv-falso"), SETPRIV_FALSO);
        let bwrap = escribir_ejecutable(
            &dir.join("bwrap-falso"),
            &cuerpo_bwrap.replace("DIR", &dir.display().to_string()),
        );
        [ip, prlimit, setpriv, bwrap].map(|p| p.display().to_string())
    }

    /// Ejecutor bwrap de la disposición, con los programas `progs` (`ip`, `prlimit`, `setpriv`, `bwrap`).
    fn ejecutor_de(d: &Disposicion, progs: &[String; 4]) -> EjecutorBwrapBuilder {
        EjecutorBwrapBuilder {
            credencial: d.credencial.clone(),
            home: d.home.clone(),
            home_usuario: d.usuario.clone(),
            timeout: Duration::from_secs(60),
            red: "red-prueba".into(),
            usuario: "root".into(),
            token: None,
            ip: progs[0].clone(),
            prlimit: progs[1].clone(),
            setpriv: progs[2].clone(),
            bwrap: progs[3].clone(),
        }
    }

    /// Orígenes de los montajes del host (`--bind` y `--ro-bind`). Los `--symlink` no montan nada del host.
    fn origenes_de_montaje(args: &[String]) -> Vec<String> {
        args.windows(3)
            .filter(|w| w[0] == "--bind" || w[0] == "--ro-bind")
            .map(|w| w[1].clone())
            .collect()
    }

    /// Valor de `bandera` en `args`: el que sigue a su primera aparición.
    fn posicion_de(args: &[String], bandera: &str) -> usize {
        args.iter()
            .position(|a| a == bandera)
            .unwrap_or_else(|| panic!("falta {bandera}: {args:?}"))
    }

    /// Entorno del `Command` como mapa. Un valor borrado (`None`) no aparece.
    fn entorno_de(cmd: &Command) -> BTreeMap<String, String> {
        cmd.get_envs()
            .filter_map(|(k, v)| {
                Some((
                    k.to_string_lossy().into_owned(),
                    v?.to_string_lossy().into_owned(),
                ))
            })
            .collect()
    }

    /// Argumentos del `Command` como texto.
    fn argv_de(cmd: &Command) -> Vec<String> {
        cmd.get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    /// Lee el `env` que dejó el `bwrap` falso de la disposición.
    fn env_del_hijo(d: &Disposicion) -> String {
        fs::read_to_string(d.raiz.join("env-hijo")).unwrap_or_default()
    }

    /// Escribe un token con modo 0600, como debe estar el archivo de `ROOT_TOKEN_BUILDER`.
    fn escribir_token(ruta: &Path, texto: &str) {
        fs::write(ruta, texto).expect("token");
        modo(ruta, 0o600);
    }

    /// Argumentos que dejó el `bwrap` falso de la disposición, como texto.
    fn args_del_bwrap(d: &Disposicion) -> String {
        fs::read_to_string(d.raiz.join("bwrap-args")).unwrap_or_default()
    }

    // --- Args de bwrap ---

    #[test]
    fn corre_bwrap_args_no_montan_root_home_home_root_ni_content() {
        let d = disposicion();
        let plugin = d.raiz.join("plugin");
        fs::create_dir_all(&plugin).expect("plugin");
        let mut l = lanzamiento(d.repo.clone(), Vec::new());
        l.plugin = Some(plugin);
        let args = args_bwrap_builder(
            &l,
            Some(d.credencial.as_path()),
            &d.estado,
            &sistema_de_prueba,
        );
        let origenes = origenes_de_montaje(&args);
        assert!(origenes.len() >= 4, "{args:?}");
        for origen in &origenes {
            let o = Path::new(origen);
            // Un montaje no puede ser ni contener a /root, /content, /home, $HOME ni ROOT_HOME.
            for prohibida in ["/root", "/content", "/home"] {
                assert!(
                    !Path::new(prohibida).starts_with(o),
                    "monta {origen}, que contiene {prohibida}"
                );
            }
            assert!(
                !d.usuario.starts_with(o),
                "monta $HOME o un padre: {origen}"
            );
            assert!(
                !d.home.starts_with(o),
                "monta un padre de ROOT_HOME: {origen}"
            );
            assert!(
                !o.starts_with(&d.home),
                "monta dentro de ROOT_HOME: {origen}"
            );
        }
        assert!(
            !args.iter().any(|a| a == "/root" || a == "/content"),
            "{args:?}"
        );
    }

    #[test]
    fn corre_bwrap_args_traen_aislamiento_y_montajes_pedidos() {
        let d = disposicion();
        let l = lanzamiento(d.repo.clone(), Vec::new());
        let args = args_bwrap_builder(
            &l,
            Some(d.credencial.as_path()),
            &d.estado,
            &sistema_de_prueba,
        );
        for bandera in [
            "--unshare-user",
            "--unshare-pid",
            "--unshare-ipc",
            "--unshare-uts",
            "--unshare-cgroup-try",
            "--die-with-parent",
            "--new-session",
        ] {
            assert!(
                args.iter().any(|a| a == bandera),
                "falta {bandera}: {args:?}"
            );
        }
        assert!(
            args.windows(2).any(|w| w == ["--proc", "/proc"]),
            "{args:?}"
        );
        assert!(args.windows(2).any(|w| w == ["--dev", "/dev"]), "{args:?}");
        assert!(
            args.windows(2).any(|w| w == ["--tmpfs", "/tmp"]),
            "{args:?}"
        );
        let repo = d.repo.display().to_string();
        assert!(
            args.windows(3)
                .any(|w| w == ["--bind", repo.as_str(), "/trabajo"]),
            "{args:?}"
        );
        let estado = d.estado.display().to_string();
        assert!(
            args.windows(3)
                .any(|w| w == ["--bind", estado.as_str(), "/home/builder/.claude"]),
            "{args:?}"
        );
        let cred = d.credencial.display().to_string();
        assert!(
            args.windows(3).any(|w| w
                == [
                    "--ro-bind",
                    cred.as_str(),
                    "/home/builder/.claude/.credentials.json"
                ]),
            "{args:?}"
        );
        assert!(
            args.windows(2).any(|w| w == ["--chdir", "/trabajo"]),
            "{args:?}"
        );
        // `--chdir` cierra la lista: lo último de bwrap es su directorio de trabajo.
        assert_eq!(
            args.last().map(String::as_str),
            Some("/trabajo"),
            "{args:?}"
        );
    }

    #[test]
    fn corre_bwrap_sistema_solo_si_existe_y_enlaces_como_en_el_host() {
        let d = disposicion();
        let l = lanzamiento(d.repo.clone(), Vec::new());
        let args = args_bwrap_builder(
            &l,
            Some(d.credencial.as_path()),
            &d.estado,
            &sistema_de_prueba,
        );
        assert!(
            args.windows(3).any(|w| w == ["--ro-bind", "/usr", "/usr"]),
            "{args:?}"
        );
        assert!(
            args.windows(3).any(|w| w == ["--ro-bind", "/etc", "/etc"]),
            "{args:?}"
        );
        assert!(
            args.windows(3).any(|w| w == ["--ro-bind", "/bin", "/bin"]),
            "{args:?}"
        );
        // /opt no existe en el sistema de prueba: no se monta.
        assert!(!args.iter().any(|a| a == "/opt"), "{args:?}");
        // /lib64 es enlace en el host: se copia como enlace, no como montaje.
        assert!(
            args.windows(3)
                .any(|w| w == ["--symlink", "usr/lib64", "/lib64"]),
            "{args:?}"
        );
        assert!(
            !args.iter().any(|a| a == "/sbin" || a == "/lib"),
            "{args:?}"
        );
    }

    #[test]
    fn corre_bwrap_plugin_solo_si_hay_y_en_opt_plugin() {
        let d = disposicion();
        let sin = args_bwrap_builder(
            &lanzamiento(d.repo.clone(), Vec::new()),
            Some(d.credencial.as_path()),
            &d.estado,
            &sistema_de_prueba,
        );
        assert!(!sin.iter().any(|a| a == PLUGIN_EN_CONTENEDOR), "{sin:?}");
        let plugin = d.raiz.join("plugin");
        fs::create_dir_all(&plugin).expect("plugin");
        let mut l = lanzamiento(d.repo.clone(), Vec::new());
        l.plugin = Some(plugin.clone());
        let con = args_bwrap_builder(
            &l,
            Some(d.credencial.as_path()),
            &d.estado,
            &sistema_de_prueba,
        );
        let origen = plugin.display().to_string();
        assert!(
            con.windows(3)
                .any(|w| w == ["--ro-bind", origen.as_str(), "/opt/plugin"]),
            "{con:?}"
        );
    }

    // --- Orden del comando y entorno ---

    #[test]
    fn corre_bwrap_orden_del_comando_es_ip_prlimit_setpriv_bwrap_claude() {
        let d = disposicion();
        let l = lanzamiento(d.repo.clone(), Vec::new());
        let ej = ejecutor_de(
            &d,
            &[
                "ip".into(),
                "prlimit".into(),
                "setpriv".into(),
                "bwrap".into(),
            ],
        );
        let cmd = ej.armar_comando(
            &l,
            Some(d.credencial.as_path()),
            &d.estado,
            (1001, 1001),
            None,
        );
        assert_eq!(cmd.get_program().to_string_lossy(), "ip");
        let argv = argv_de(&cmd);
        assert_eq!(&argv[..3], ["netns", "exec", "red-prueba"], "{argv:?}");
        assert_eq!(argv[3], "prlimit", "{argv:?}");
        assert_eq!(argv[4], "--nproc=4096", "{argv:?}");
        assert_eq!(argv[5], "setpriv", "{argv:?}");
        for bandera in [
            "--reuid=1001",
            "--regid=1001",
            "--clear-groups",
            "--inh-caps=-all",
            "--bounding-set=-all",
            "--no-new-privs",
        ] {
            assert!(
                argv.iter().any(|a| a == bandera),
                "falta {bandera}: {argv:?}"
            );
        }
        let (prlimit, setpriv) = (posicion_de(&argv, "prlimit"), posicion_de(&argv, "setpriv"));
        let bwrap = posicion_de(&argv, "bwrap");
        let claude = posicion_de(&argv, "claude");
        assert!(
            prlimit < setpriv && setpriv < bwrap && bwrap < claude,
            "{argv:?}"
        );
        // Después de `claude` van solo los argumentos del lanzamiento.
        assert_eq!(&argv[claude + 1..], l.args.as_slice(), "{argv:?}");
    }

    #[test]
    fn corre_bwrap_entorno_del_command_sin_claves_ni_root_home_y_con_home_y_path_fijos() {
        let d = disposicion();
        let env = vec![
            par("ROOT_HOME", "/datos/root"),
            par("DEEPSEEK_API_KEY", "clave"),
            par("ANTHROPIC_API_KEY", "clave"),
            par("HOME", "/root"),
            par("PATH", "/otro"),
            par("LANG", "C.UTF-8"),
        ];
        let l = lanzamiento(d.repo.clone(), env);
        let ej = ejecutor_de(
            &d,
            &[
                "ip".into(),
                "prlimit".into(),
                "setpriv".into(),
                "bwrap".into(),
            ],
        );
        let cmd = ej.armar_comando(
            &l,
            Some(d.credencial.as_path()),
            &d.estado,
            (1001, 1001),
            None,
        );
        let envs = entorno_de(&cmd);
        for clave in ["ROOT_HOME", "DEEPSEEK_API_KEY", "ANTHROPIC_API_KEY"] {
            assert!(!envs.contains_key(clave), "trae {clave}: {envs:?}");
        }
        assert_eq!(envs.get("HOME").map(String::as_str), Some("/home/builder"));
        assert_eq!(envs.get("PATH").map(String::as_str), Some(PATH_FIJO));
        assert_eq!(envs.get("LANG").map(String::as_str), Some("C.UTF-8"));
        assert!(!envs.contains_key("CLAUDE_CODE_OAUTH_TOKEN"), "{envs:?}");
    }

    #[test]
    fn corre_bwrap_token_no_va_en_argv_y_si_en_el_entorno() {
        let d = disposicion();
        let ruta = d.raiz.join("token");
        escribir_token(&ruta, "  tok-falso-123\n");
        let token = leer_token(&ruta).expect("lee el token");
        assert_eq!(token, "tok-falso-123");
        let l = lanzamiento(d.repo.clone(), Vec::new());
        let ej = ejecutor_de(
            &d,
            &[
                "ip".into(),
                "prlimit".into(),
                "setpriv".into(),
                "bwrap".into(),
            ],
        );
        let cmd = ej.armar_comando(
            &l,
            Some(d.credencial.as_path()),
            &d.estado,
            (1001, 1001),
            Some(&token),
        );
        for a in argv_de(&cmd) {
            assert!(!a.contains("tok-falso-123"), "el token va en argv: {a}");
        }
        assert_eq!(
            entorno_de(&cmd)
                .get("CLAUDE_CODE_OAUTH_TOKEN")
                .map(String::as_str),
            Some("tok-falso-123")
        );
    }

    #[test]
    fn corre_bwrap_token_de_archivo_que_falta_nombra_la_ruta_sin_contenido() {
        let d = disposicion();
        let ruta = d.raiz.join("no-existe-token");
        let error = leer_token(&ruta).expect_err("falta el archivo");
        assert!(matches!(error, ErrorCorre::Ejecutor(_)), "{error}");
        assert!(error.to_string().contains("no-existe-token"), "{error}");
    }

    #[test]
    fn corre_bwrap_desde_entorno_lee_red_usuario_y_token_del_slice() {
        let entorno = vec![
            par("ROOT_RED_BUILDER", "red-x"),
            par("ROOT_USUARIO_BUILDER", "builder"),
            par("ROOT_TOKEN_BUILDER", "/ruta/token"),
        ];
        let ej = EjecutorBwrapBuilder::desde_entorno(
            &entorno,
            PathBuf::from("/c"),
            PathBuf::from("/h"),
            PathBuf::from("/u"),
        )
        .expect("desde_entorno");
        assert_eq!(ej.red, "red-x");
        assert_eq!(ej.usuario, "builder");
        assert_eq!(ej.token, Some(PathBuf::from("/ruta/token")));
        assert_eq!(ej.timeout, TIMEOUT_BUILDER);
        assert_eq!(ej.ip, "ip");
        assert_eq!(ej.bwrap, "bwrap");
    }

    #[test]
    fn corre_bwrap_desde_entorno_sin_token_da_none() {
        let entorno = vec![
            par("ROOT_RED_BUILDER", "red-x"),
            par("ROOT_USUARIO_BUILDER", "builder"),
        ];
        let ej = EjecutorBwrapBuilder::desde_entorno(
            &entorno,
            PathBuf::from("/c"),
            PathBuf::from("/h"),
            PathBuf::from("/u"),
        )
        .expect("desde_entorno");
        assert_eq!(ej.token, None);
    }

    #[test]
    fn corre_bwrap_falta_red_o_usuario_es_error() {
        let completo = [
            par("ROOT_RED_BUILDER", "red-x"),
            par("ROOT_USUARIO_BUILDER", "builder"),
        ];
        let sin_red = [par("ROOT_USUARIO_BUILDER", "builder")];
        let sin_usuario = [par("ROOT_RED_BUILDER", "red-x")];
        let vacia = [
            par("ROOT_RED_BUILDER", ""),
            par("ROOT_USUARIO_BUILDER", "builder"),
        ];
        for entorno in [&sin_red[..], &sin_usuario[..], &vacia[..]] {
            let error = EjecutorBwrapBuilder::desde_entorno(
                entorno,
                PathBuf::from("/c"),
                PathBuf::from("/h"),
                PathBuf::from("/u"),
            )
            .err()
            .expect("debe fallar");
            assert!(matches!(error, ErrorCorre::Ejecutor(_)), "{error}");
        }
        assert!(
            EjecutorBwrapBuilder::desde_entorno(
                &completo,
                PathBuf::from("/c"),
                PathBuf::from("/h"),
                PathBuf::from("/u"),
            )
            .is_ok()
        );
    }

    #[test]
    fn corre_bwrap_red_vacia_frena_el_lanzamiento() {
        let _serie = serie_de_pruebas();
        let d = disposicion();
        let progs = cadena_falsa(&d.raiz, BWRAP_OK);
        let ej = EjecutorBwrapBuilder {
            red: String::new(),
            ..ejecutor_de(&d, &progs)
        };
        let error = ej
            .lanzar(&lanzamiento(d.repo.clone(), Vec::new()))
            .expect_err("sin red no lanza");
        assert!(
            matches!(error, ErrorCorre::Ejecutor(ref m) if m.contains("ROOT_RED_BUILDER")),
            "{error}"
        );
        assert!(!d.raiz.join("bwrap-args").exists());
    }

    // --- Chequeos previos ---

    #[test]
    fn corre_bwrap_root_home_con_modo_0755_frena_el_lanzamiento() {
        let _serie = serie_de_pruebas();
        let d = disposicion();
        modo(&d.home, 0o755);
        let progs = cadena_falsa(&d.raiz, BWRAP_OK);
        let ej = ejecutor_de(&d, &progs);
        let error = ej
            .lanzar(&lanzamiento(d.repo.clone(), Vec::new()))
            .expect_err("ROOT_HOME 0755 no lanza");
        assert!(
            matches!(error, ErrorCorre::Ejecutor(ref m) if m.contains("ROOT_HOME")),
            "{error}"
        );
        assert!(!d.raiz.join("bwrap-args").exists(), "no debe lanzar");
    }

    #[test]
    fn corre_bwrap_builder_con_el_uid_de_r00t_frena() {
        let propio = 1001;
        let carpeta = Permisos {
            uid: propio,
            gid: propio,
            modo: 0o700,
        };
        let error = revisar_permisos(propio, (propio, propio), carpeta, carpeta)
            .expect_err("mismo uid no lanza");
        assert!(matches!(error, ErrorCorre::Ejecutor(_)), "{error}");
    }

    #[test]
    fn corre_bwrap_root_home_debe_ser_de_r00t_y_0700_o_mas_cerrado() {
        let propio = 1001;
        let builder = (1002, 1002);
        let usuario = Permisos {
            uid: propio,
            gid: propio,
            modo: 0o700,
        };
        let ajeno = Permisos {
            uid: 1002,
            gid: propio,
            modo: 0o700,
        };
        let abierto = Permisos {
            uid: propio,
            gid: propio,
            modo: 0o750,
        };
        let cerrado = Permisos {
            uid: propio,
            gid: propio,
            modo: 0o500,
        };
        assert!(revisar_permisos(propio, builder, usuario, usuario).is_ok());
        assert!(revisar_permisos(propio, builder, cerrado, usuario).is_ok());
        assert!(revisar_permisos(propio, builder, ajeno, usuario).is_err());
        assert!(revisar_permisos(propio, builder, abierto, usuario).is_err());
    }

    #[test]
    fn corre_bwrap_home_legible_por_el_builder_frena() {
        let propio = 1001;
        let builder = (1002, 2000);
        let root_home = Permisos {
            uid: propio,
            gid: propio,
            modo: 0o700,
        };
        // Otros pueden leer: frena.
        let otros = Permisos {
            uid: propio,
            gid: propio,
            modo: 0o755,
        };
        assert!(revisar_permisos(propio, builder, root_home, otros).is_err());
        // Solo el dueño de r00t puede: pasa.
        let privado = Permisos {
            uid: propio,
            gid: propio,
            modo: 0o700,
        };
        assert!(revisar_permisos(propio, builder, root_home, privado).is_ok());
        // Grupo con lectura y el builder en ese grupo: frena.
        let grupo = Permisos {
            uid: propio,
            gid: 2000,
            modo: 0o750,
        };
        assert!(revisar_permisos(propio, builder, root_home, grupo).is_err());
        // Grupo ajeno y otros sin permisos: pasa.
        let ajeno = Permisos {
            uid: propio,
            gid: 3000,
            modo: 0o750,
        };
        assert!(revisar_permisos(propio, builder, root_home, ajeno).is_ok());
        // Si el builder es dueño de $HOME con permisos de dueño, frena también.
        let dueno_builder = Permisos {
            uid: 1002,
            gid: 2000,
            modo: 0o700,
        };
        assert!(revisar_permisos(propio, builder, root_home, dueno_builder).is_err());
    }

    #[test]
    fn corre_bwrap_uid_y_gid_salen_de_passwd() {
        let passwd = "root:x:0:0:root:/root:/bin/bash\nbuilder:x:1001:1002::/root:/bin/bash\n";
        assert_eq!(uid_gid_en(passwd, "builder").expect("existe"), (1001, 1002));
        assert_eq!(uid_gid_en(passwd, "root").expect("existe"), (0, 0));
        let error = uid_gid_en(passwd, "nadie").expect_err("falta");
        assert!(matches!(error, ErrorCorre::Ejecutor(_)), "{error}");
    }

    // --- Lanzamiento con programas falsos ---

    #[test]
    fn corre_bwrap_el_hijo_no_hereda_claves_ni_root_home() {
        let _serie = serie_de_pruebas();
        let d = disposicion();
        let progs = cadena_falsa(&d.raiz, BWRAP_OK);
        let env = vec![
            par("ROOT_HOME", "/datos/root"),
            par("DEEPSEEK_API_KEY", "clave"),
            par("ANTHROPIC_API_KEY", "clave"),
            par("LANG", "C.UTF-8"),
        ];
        let salida = ejecutor_de(&d, &progs)
            .lanzar(&lanzamiento(d.repo.clone(), env))
            .expect("lanza");
        assert_eq!(
            salida.codigo,
            0,
            "{}",
            String::from_utf8_lossy(&salida.stderr)
        );
        let texto = env_del_hijo(&d);
        for clave in ["ROOT_HOME=", "DEEPSEEK_API_KEY=", "ANTHROPIC_API_KEY="] {
            assert!(!texto.contains(clave), "el hijo trae {clave}: {texto}");
        }
        assert!(texto.lines().any(|l| l == "HOME=/home/builder"), "{texto}");
        assert!(
            texto.lines().any(|l| l == format!("PATH={PATH_FIJO}")),
            "{texto}"
        );
        // Nada del entorno del proceso de test: solo lo que fija el ejecutor.
        let permitidas = [
            "HOME",
            "PATH",
            "LANG",
            "PWD",
            "OLDPWD",
            "SHLVL",
            "_",
            "ECC_AGENT_DATA_HOME",
        ];
        for linea in texto.lines() {
            let clave = linea.split('=').next().unwrap_or_default();
            assert!(permitidas.contains(&clave), "variable heredada: {linea}");
        }
    }

    #[test]
    fn corre_bwrap_token_llega_al_hijo_y_no_a_argv_del_lanzamiento() {
        let _serie = serie_de_pruebas();
        let d = disposicion();
        let ruta = d.raiz.join("token");
        escribir_token(&ruta, "tok-falso-456\n");
        let progs = cadena_falsa(&d.raiz, BWRAP_OK);
        let mut ej = ejecutor_de(&d, &progs);
        ej.token = Some(ruta);
        let salida = ej
            .lanzar(&lanzamiento(d.repo.clone(), Vec::new()))
            .expect("lanza");
        assert_eq!(salida.codigo, 0);
        assert!(!String::from_utf8_lossy(&salida.stdout).contains("tok-falso-456"));
        assert!(env_del_hijo(&d).contains("CLAUDE_CODE_OAUTH_TOKEN=tok-falso-456"));
        let argv = fs::read_to_string(d.raiz.join("bwrap-args")).unwrap_or_default();
        assert!(!argv.contains("tok-falso-456"), "{argv}");
    }

    // --- Con token no hay credencial; sin token sí ---

    #[test]
    fn corre_bwrap_con_token_lanza_sin_credencial_en_el_host() {
        let _serie = serie_de_pruebas();
        let d = disposicion();
        fs::remove_file(&d.credencial).expect("quita la credencial");
        let ruta = d.raiz.join("token");
        escribir_token(&ruta, "tok-falso-788\n");
        let progs = cadena_falsa(&d.raiz, BWRAP_OK);
        let mut ej = ejecutor_de(&d, &progs);
        ej.token = Some(ruta);
        let salida = ej
            .lanzar(&lanzamiento(d.repo.clone(), Vec::new()))
            .expect("con token no hace falta la credencial");
        assert_eq!(salida.codigo, 0);
        assert!(d.raiz.join("bwrap-args").is_file(), "no lanzó");
    }

    #[test]
    fn corre_bwrap_con_token_no_copia_ni_monta_la_credencial() {
        let _serie = serie_de_pruebas();
        let d = disposicion();
        let ruta = d.raiz.join("token");
        escribir_token(&ruta, "tok-falso-790\n");
        let progs = cadena_falsa(&d.raiz, BWRAP_OK);
        let mut ej = ejecutor_de(&d, &progs);
        ej.token = Some(ruta);
        ej.lanzar(&lanzamiento(d.repo.clone(), Vec::new()))
            .expect("lanza");
        let args = args_del_bwrap(&d);
        assert!(
            !args.contains(".credentials.json"),
            "monta credencial: {args}"
        );
        assert!(!args.contains(CREDENCIAL_EN_CONTENEDOR), "{args}");
        assert!(
            !d.estado.join(".credentials.json").exists(),
            "copió la credencial al estado"
        );
    }

    #[test]
    fn corre_bwrap_con_token_de_modo_abierto_frena_sin_mostrar_contenido() {
        let _serie = serie_de_pruebas();
        let d = disposicion();
        let ruta = d.raiz.join("token");
        fs::write(&ruta, "tok-secreto-falso-791\n").expect("token");
        modo(&ruta, 0o644);
        let progs = cadena_falsa(&d.raiz, BWRAP_OK);
        let mut ej = ejecutor_de(&d, &progs);
        ej.token = Some(ruta.clone());
        let error = ej
            .lanzar(&lanzamiento(d.repo.clone(), Vec::new()))
            .expect_err("un token 0644 no lanza");
        let texto = error.to_string();
        assert!(matches!(error, ErrorCorre::Ejecutor(_)), "{texto}");
        assert!(texto.contains(&ruta.display().to_string()), "{texto}");
        assert!(texto.contains("0644"), "{texto}");
        assert!(!texto.contains("tok-secreto-falso-791"), "{texto}");
        assert!(!d.raiz.join("bwrap-args").exists(), "no debe lanzar");
        assert!(
            !d.estado.join(".credentials.json").exists(),
            "copió antes de revisar el token"
        );
    }

    #[test]
    fn corre_bwrap_token_vacio_frena_antes_de_copiar_la_credencial() {
        let _serie = serie_de_pruebas();
        let d = disposicion();
        let ruta = d.raiz.join("token");
        escribir_token(&ruta, " \n  ");
        let progs = cadena_falsa(&d.raiz, BWRAP_OK);
        let mut ej = ejecutor_de(&d, &progs);
        ej.token = Some(ruta);
        let error = ej
            .lanzar(&lanzamiento(d.repo.clone(), Vec::new()))
            .expect_err("un token vacío no lanza");
        assert!(
            matches!(error, ErrorCorre::Ejecutor(ref m) if m.contains("vacío")),
            "{error}"
        );
        assert!(
            !d.estado.join(".credentials.json").exists(),
            "copió la credencial antes de revisar el token"
        );
    }

    #[test]
    fn corre_bwrap_sin_token_sin_credencial_frena_con_el_mensaje_de_siempre() {
        let _serie = serie_de_pruebas();
        let d = disposicion();
        fs::remove_file(&d.credencial).expect("quita la credencial");
        let progs = cadena_falsa(&d.raiz, BWRAP_OK);
        let error = ejecutor_de(&d, &progs)
            .lanzar(&lanzamiento(d.repo.clone(), Vec::new()))
            .expect_err("sin token ni credencial no lanza");
        assert!(
            matches!(error, ErrorCorre::Ejecutor(ref m) if m.contains("no existe la credencial de Claude")),
            "{error}"
        );
        assert!(!d.raiz.join("bwrap-args").exists(), "no debe lanzar");
    }

    #[test]
    fn corre_bwrap_sin_token_copia_la_credencial_y_la_monta() {
        let _serie = serie_de_pruebas();
        let d = disposicion();
        let progs = cadena_falsa(&d.raiz, BWRAP_OK);
        let salida = ejecutor_de(&d, &progs)
            .lanzar(&lanzamiento(d.repo.clone(), Vec::new()))
            .expect("lanza");
        assert_eq!(salida.codigo, 0);
        assert!(d.estado.join(".credentials.json").is_file(), "no copió");
        let args = args_del_bwrap(&d);
        assert!(args.contains(CREDENCIAL_EN_CONTENEDOR), "{args}");
    }

    #[test]
    fn corre_bwrap_fallo_de_bwrap_es_ejecutor_y_no_salida() {
        let _serie = serie_de_pruebas();
        let d = disposicion();
        let cuerpo = "#!/bin/sh\necho 'bwrap: no se pudo crear el espacio' >&2\nexit 1\n";
        let progs = cadena_falsa(&d.raiz, cuerpo);
        let error = ejecutor_de(&d, &progs)
            .lanzar(&lanzamiento(d.repo.clone(), Vec::new()))
            .expect_err("un fallo de bwrap no es salida");
        assert!(matches!(error, ErrorCorre::Ejecutor(_)), "{error}");
        assert!(error.to_string().contains("bwrap:"), "{error}");
    }

    #[test]
    fn corre_bwrap_salida_de_claude_con_codigo_1_pasa_como_ok() {
        let _serie = serie_de_pruebas();
        let d = disposicion();
        let cuerpo = "#!/bin/sh\necho '{\"type\":\"result\"}'\nexit 1\n";
        let progs = cadena_falsa(&d.raiz, cuerpo);
        let salida = ejecutor_de(&d, &progs)
            .lanzar(&lanzamiento(d.repo.clone(), Vec::new()))
            .expect("la salida de claude no es error de infra");
        assert_eq!(salida.codigo, 1);
        assert!(String::from_utf8_lossy(&salida.stdout).contains("result"));
    }

    #[test]
    fn corre_bwrap_plazo_vencido_mata_el_grupo_y_da_cortado_con_stdout() {
        let _serie = serie_de_pruebas();
        let d = disposicion();
        let cuerpo = "#!/bin/sh\necho parcial\nsleep 30\n";
        let progs = cadena_falsa(&d.raiz, cuerpo);
        let mut l = lanzamiento(d.repo.clone(), Vec::new());
        l.plazo = Duration::from_secs(1);
        let inicio = Instant::now();
        let error = ejecutor_de(&d, &progs)
            .lanzar(&l)
            .expect_err("vence el plazo");
        assert!(
            inicio.elapsed() < Duration::from_secs(20),
            "no corta a tiempo"
        );
        match error {
            ErrorCorre::Cortado { stdout, .. } => {
                assert!(
                    String::from_utf8_lossy(&stdout).contains("parcial"),
                    "{stdout:?}"
                );
            }
            otro => panic!("debe ser Cortado: {otro}"),
        }
    }

    // --- Clasificación de la salida ---

    fn salida(codigo: i32, stdout: &str, stderr: &str) -> SalidaClaude {
        SalidaClaude {
            codigo,
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        }
    }

    #[test]
    fn corre_bwrap_stderr_con_prefijo_y_codigo_1_sin_stdout_es_infra() {
        for stderr in [
            "bwrap: no hay espacio de usuario",
            "setpriv: no se pudo cambiar el uid",
            "prlimit: fallo",
            "ip: algo",
            "Cannot open network namespace \"red\": No such file",
        ] {
            let error = clasificar_salida_bwrap(salida(1, "", stderr)).expect_err(stderr);
            assert!(
                matches!(error, ErrorCorre::Ejecutor(_)),
                "{stderr}: {error}"
            );
        }
    }

    #[test]
    fn corre_bwrap_codigo_1_con_stdout_es_salida_de_claude() {
        let ok = clasificar_salida_bwrap(salida(1, "{\"type\":\"result\"}\n", "bwrap: aviso"))
            .expect("con stdout es de claude");
        assert_eq!(ok.codigo, 1);
        let ok = clasificar_salida_bwrap(salida(1, "{\"type\":\"result\"}\n", "algo distinto"))
            .expect("con stdout es de claude");
        assert_eq!(ok.codigo, 1);
    }

    #[test]
    fn corre_bwrap_codigos_126_y_127_son_infra() {
        for codigo in [126, 127] {
            let error = clasificar_salida_bwrap(salida(codigo, "x", "")).expect_err("infra");
            assert!(
                matches!(error, ErrorCorre::Ejecutor(_)),
                "{codigo}: {error}"
            );
        }
    }

    #[test]
    fn corre_bwrap_sigkill_conserva_stdout_como_cortado() {
        let error = clasificar_salida_bwrap(salida(137, "{\"parcial\":1}\n", "")).expect_err("137");
        match error {
            ErrorCorre::Cortado { stdout, .. } => assert_eq!(stdout, b"{\"parcial\":1}\n"),
            otro => panic!("137 debe dar Cortado: {otro}"),
        }
    }

    #[test]
    fn corre_bwrap_salida_limpia_pasa() {
        let ok = clasificar_salida_bwrap(salida(0, "res", "")).expect("ok");
        assert_eq!(ok.codigo, 0);
    }

    // --- bwrap real (se saltea sin bwrap usable) ---

    /// Lanza `sh <script> <extra...>` dentro de bwrap real, con los montajes de `args_bwrap_builder`
    /// del host y `scripts` montada en `/prueba`.
    fn correr_en_bwrap(
        d: &Disposicion,
        scripts: &Path,
        script: &str,
        extra: &[String],
    ) -> std::process::Output {
        let l = lanzamiento(d.repo.clone(), Vec::new());
        let mut args = args_bwrap_builder(
            &l,
            Some(d.credencial.as_path()),
            &d.estado,
            &entrada_de_raiz,
        );
        args.extend([
            "--ro-bind".to_string(),
            scripts.display().to_string(),
            "/prueba".to_string(),
        ]);
        Command::new("bwrap")
            .args(&args)
            .args(["sh", &format!("/prueba/{script}")])
            .args(extra)
            .env_clear()
            .env("PATH", PATH_FIJO)
            .output()
            .expect("bwrap")
    }

    #[test]
    fn corre_bwrap_real_no_lee_canario_fuera_de_los_montajes() {
        saltar_sin_bwrap!();
        let d = disposicion();
        let canario = d.usuario.join("canario.txt");
        fs::write(&canario, "CANARIO_NO_DEBE_LEERSE").expect("canario");
        let scripts = d.raiz.join("scripts");
        fs::create_dir_all(&scripts).expect("scripts");
        fs::write(
            scripts.join("lee.sh"),
            "cat \"$1\" 2>/dev/null || echo NO_LEE\n",
        )
        .expect("script");
        // Fuera de bwrap el script sí lo lee: el canario es real.
        let fuera = Command::new("sh")
            .arg(scripts.join("lee.sh"))
            .arg(&canario)
            .output()
            .expect("sh");
        assert!(String::from_utf8_lossy(&fuera.stdout).contains("CANARIO_NO_DEBE_LEERSE"));
        // Dentro de bwrap no.
        let dentro = correr_en_bwrap(&d, &scripts, "lee.sh", &[canario.display().to_string()]);
        let texto = String::from_utf8_lossy(&dentro.stdout);
        assert!(!texto.contains("CANARIO_NO_DEBE_LEERSE"), "{texto}");
        assert!(
            texto.contains("NO_LEE"),
            "{texto} {}",
            String::from_utf8_lossy(&dentro.stderr)
        );
    }

    #[test]
    fn corre_bwrap_real_escribe_en_trabajo_y_no_en_usr() {
        saltar_sin_bwrap!();
        let d = disposicion();
        let scripts = d.raiz.join("scripts");
        fs::create_dir_all(&scripts).expect("scripts");
        fs::write(
            scripts.join("escribe.sh"),
            "echo hecho > /trabajo/hecho.txt && echo ESCRIBIO_TRABAJO\n\
             echo x > /usr/r00t-prueba-171 2>/dev/null && echo ESCRIBIO_USR || echo NO_ESCRIBE_USR\n",
        )
        .expect("script");
        let salida = correr_en_bwrap(&d, &scripts, "escribe.sh", &[]);
        let texto = String::from_utf8_lossy(&salida.stdout);
        assert!(
            texto.contains("ESCRIBIO_TRABAJO"),
            "{texto} {}",
            String::from_utf8_lossy(&salida.stderr)
        );
        assert!(texto.contains("NO_ESCRIBE_USR"), "{texto}");
        assert!(!texto.contains("ESCRIBIO_USR"), "{texto}");
        assert!(
            d.repo.join("hecho.txt").is_file(),
            "no apareció en la carpeta de la corrida"
        );
        assert!(!Path::new("/usr/r00t-prueba-171").exists());
    }
}
