//! `r00t corre` de punta a punta, con el binario. Ningún test llega a lanzar `claude` ni podman: el HOME de
//! prueba no tiene credencial de Claude, así que el ejecutor de contenedor falla antes.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use root::corre::dir_corridas;
use root::goal::calcular_hash;

const GOAL: &str = "# GOAL de prueba\n\nContenido de fixture.\n";

/// Un repo de Root, un `$HOME` y un `ROOT_HOME` de prueba, con la tarea `memo` y la config `ecc-test`.
struct Escenario {
    _tmp: tempfile::TempDir,
    repo: PathBuf,
    usuario: PathBuf,
    home: PathBuf,
}

fn escenario(goal_valido: bool) -> Escenario {
    let tmp = tempfile::tempdir().expect("tmp");
    let base = tmp.path();
    let repo = base.join("repo");
    let usuario = base.join("usuario");
    let home = base.join("root");
    for dir in [&repo, &usuario, &home] {
        fs::create_dir_all(dir).expect("carpeta");
    }

    fs::write(repo.join("GOAL.md"), GOAL).expect("GOAL.md");
    let hash = if goal_valido {
        calcular_hash(GOAL.as_bytes())
    } else {
        "0000000000000000".to_string()
    };
    fs::write(repo.join("GOAL.hash"), format!("{hash}\n")).expect("GOAL.hash");

    let tarea = home.join("oraculos").join("memo");
    fs::create_dir_all(tarea.join("bootstrap").join("docs")).expect("bootstrap");
    fs::write(
        tarea.join("bootstrap").join("docs").join("SPEC.md"),
        "# Spec\n",
    )
    .expect("spec");
    fs::create_dir_all(tarea.join("contrato")).expect("contrato");
    fs::write(
        tarea.join("contrato").join("test_x.py"),
        "def test_x(): pass\n",
    )
    .expect("contrato");
    fs::create_dir_all(tarea.join("oculto")).expect("oculto");
    fs::write(
        tarea.join("oculto").join("test_y.py"),
        "def test_y(): pass\n",
    )
    .expect("oculto");
    fs::write(
        tarea.join("tarea.toml"),
        "nombre = \"memo\"\nlenguaje = \"rust\"\nimagen = \"root-oraculo-rust:1\"\nspec = \"bootstrap/docs/SPEC.md\"\n\n[contrato]\ncmd = \"pytest -q /oraculo/contrato\"\n\n[oculto]\ncmd = \"pytest -q /oraculo/oculto\"\n",
    )
    .expect("tarea.toml");

    let configs = home.join("configs");
    fs::create_dir_all(&configs).expect("configs");
    fs::write(
        configs.join("ecc-test.toml"),
        "roles = [\"planifica\", \"construye\"]\n\n[prompt]\nplanifica = \"haz el plan\"\nconstruye = \"haz hecho\"\n",
    )
    .expect("config");

    Escenario {
        _tmp: tmp,
        repo,
        usuario,
        home,
    }
}

fn root_en(e: &Escenario, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_r00t"))
        .current_dir(&e.repo)
        .env("ROOT_HOME", &e.home)
        .env("HOME", &e.usuario)
        .args(args)
        .output()
        .expect("no se pudo ejecutar el binario root")
}

fn codigo(salida: &Output) -> i32 {
    salida
        .status
        .code()
        .expect("el proceso no terminó con código")
}

fn corre(e: &Escenario, config: &str) -> Output {
    root_en(e, &["corre", "--tarea", "memo", "--config", config])
}

fn sin_rastro(e: &Escenario) -> bool {
    !e.home.join("runs.jsonl").exists() && !e.usuario.join(".cache").exists()
}

#[test]
fn corre_goal_no_autorizado_sale_con_el_codigo_de_goal_verificar() {
    // Dos casos: GOAL.md que no existe, y GOAL.md alterado frente a GOAL.hash.
    let sin_goal = escenario(true);
    fs::remove_file(sin_goal.repo.join("GOAL.md")).expect("quita GOAL.md");
    let alterado = escenario(true);
    fs::write(alterado.repo.join("GOAL.md"), "# GOAL cambiado\n").expect("GOAL.md");

    for e in [&sin_goal, &alterado] {
        let esperado = codigo(&root_en(e, &["goal", "verificar"]));
        assert_ne!(esperado, 0, "el GOAL debe estar sin autorizar");
        let salida = corre(e, "ecc-test");
        assert_eq!(
            codigo(&salida),
            esperado,
            "stderr: {}",
            String::from_utf8_lossy(&salida.stderr)
        );
        assert!(
            sin_rastro(e),
            "no debe escribir runs.jsonl ni crear corridas"
        );
    }
}

#[test]
fn corre_sin_goal_sale_con_3_y_nombra_el_goal() {
    let e = escenario(true);
    fs::remove_file(e.repo.join("GOAL.md")).expect("quita GOAL.md");
    let salida = corre(&e, "ecc-test");
    assert_eq!(codigo(&salida), 3);
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(stderr.contains("GOAL no autorizado"), "stderr: {stderr}");
    assert!(sin_rastro(&e));
}

#[test]
fn corre_config_inexistente_con_goal_valido_sale_con_2() {
    let e = escenario(true);
    let salida = corre(&e, "no-existe");
    assert_eq!(
        codigo(&salida),
        2,
        "stderr: {}",
        String::from_utf8_lossy(&salida.stderr)
    );
    assert!(!e.home.join("runs.jsonl").exists());
}

#[test]
fn corre_sin_home_del_usuario_sale_con_2() {
    let e = escenario(true);
    let salida = Command::new(env!("CARGO_BIN_EXE_r00t"))
        .current_dir(&e.repo)
        .env("ROOT_HOME", &e.home)
        .env_remove("HOME")
        .args(["corre", "--tarea", "memo", "--config", "ecc-test"])
        .output()
        .expect("binario");
    assert_eq!(codigo(&salida), 2);
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(stderr.contains("HOME"), "stderr: {stderr}");
}

#[test]
fn corre_con_goal_valido_sin_credencial_sale_con_3_y_no_escribe_fila() {
    // El HOME de prueba no tiene credencial de Claude: el ejecutor falla antes de buscar o correr
    // podman. La corrida llega al lanzamiento, no lanza nada y sale con 3 sin fila.
    let e = escenario(true);
    let salida = corre(&e, "ecc-test");
    assert_eq!(codigo(&salida), 3);
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(
        stderr.contains("no existe la credencial de Claude"),
        "stderr: {stderr}"
    );
    assert!(
        !e.home.join("runs.jsonl").exists(),
        "sin lanzamiento no hay fila"
    );
}

#[test]
fn corre_reanuda_con_id_de_otra_config_sale_con_2_y_no_toca_nada() {
    // El id no empieza por `corre-memo-ecc-test-`: el error de uso sale antes de lanzar o crear nada.
    let e = escenario(true);
    let salida = root_en(
        &e,
        &[
            "corre",
            "--tarea",
            "memo",
            "--config",
            "ecc-test",
            "--reanuda",
            "corre-memo-otra-1-2-0",
        ],
    );
    assert_eq!(codigo(&salida), 2);
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(
        stderr.contains("uso: ") && stderr.contains("corre-memo-ecc-test-"),
        "stderr: {stderr}"
    );
    assert!(
        sin_rastro(&e),
        "no debe crear carpetas ni escribir runs.jsonl"
    );
}

/// Credencial falsa de Claude en el `$HOME` de prueba, con tres horas de vida. El token es de prueba.
fn credencial_falsa(usuario: &Path) {
    let carpeta = usuario.join(".claude");
    fs::create_dir_all(&carpeta).expect(".claude");
    let ahora_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("reloj")
        .as_millis();
    let vence = ahora_ms + 3 * 60 * 60 * 1000;
    let texto = format!(
        "{{\"claudeAiOauth\":{{\"accessToken\":\"TOKEN_FALSO_PRUEBA\",\"refreshToken\":\"REFRESCO_FALSO\",\"expiresAt\":{vence}}}}}"
    );
    fs::write(carpeta.join(".credentials.json"), texto).expect("credencial falsa");
}

/// `podman` falso: `ps` no imprime nada y cualquier otro comando anota su pid en `bin/hijo.pid` y duerme 30 s.
fn podman_falso(bin: &Path) {
    use std::os::unix::fs::PermissionsExt;
    fs::create_dir_all(bin).expect("bin");
    let ruta = bin.join("podman");
    fs::write(
        &ruta,
        format!(
            "#!/bin/sh\nif [ \"$1\" = ps ]; then exit 0; fi\necho $$ > '{}'\nexec sleep 30\n",
            bin.join("hijo.pid").display()
        ),
    )
    .expect("podman falso");
    fs::set_permissions(&ruta, fs::Permissions::from_mode(0o755)).expect("permisos");
}

/// Copias `.credentials.json` que hay bajo las carpetas de corridas de `usuario`.
fn copias_vivas(usuario: &Path) -> Vec<PathBuf> {
    let Ok(entradas) = fs::read_dir(dir_corridas(usuario)) else {
        return Vec::new();
    };
    entradas
        .filter_map(Result::ok)
        .map(|entrada| {
            entrada
                .path()
                .join("claude-estado")
                .join(".credentials.json")
        })
        .filter(|copia| fs::symlink_metadata(copia).is_ok())
        .collect()
}

/// Espera hasta `plazo` a que `pred` dé true. Sondea cada 50 ms.
fn esperar_hasta(plazo: Duration, mut pred: impl FnMut() -> bool) -> bool {
    let limite = Instant::now() + plazo;
    while Instant::now() < limite {
        if pred() {
            return true;
        }
        thread::sleep(Duration::from_millis(50));
    }
    pred()
}

/// `true` si el proceso `pid` existe y no es zombi. Un zombi ya no corre.
fn proceso_vivo(pid: u32) -> bool {
    fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|stat| {
            stat.rsplit(") ")
                .next()
                .and_then(|resto| resto.chars().next())
        })
        .is_some_and(|estado| estado != 'Z')
}

/// Espera hasta `plazo` a que el proceso termine. Si no termina, lo mata y falla el test.
fn esperar_salida(hijo: &mut Child, plazo: Duration) -> ExitStatus {
    let limite = Instant::now() + plazo;
    loop {
        if let Some(estado) = hijo.try_wait().expect("try_wait") {
            return estado;
        }
        assert!(
            Instant::now() < limite,
            "r00t corre no terminó tras la señal"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

/// Lanza `r00t corre` con la credencial falsa y el `podman` falso, espera a que la copia exista y a que
/// el `run` del `podman` falso esté en curso. Manda la señal `senal` y comprueba que la copia se borra,
/// que el código es `codigo` y que el `podman` falso no queda vivo.
fn senal_quita_la_copia_y_sale_con(senal: &str, codigo: i32) {
    let e = escenario(true);
    credencial_falsa(&e.usuario);
    let bin = e._tmp.path().join("bin");
    podman_falso(&bin);
    let camino = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let stderr = fs::File::create(e._tmp.path().join("stderr.txt")).expect("stderr");
    let mut hijo = Command::new(env!("CARGO_BIN_EXE_r00t"))
        .current_dir(&e.repo)
        .env("ROOT_HOME", &e.home)
        .env("HOME", &e.usuario)
        .env("PATH", camino)
        .args(["corre", "--tarea", "memo", "--config", "ecc-test"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(stderr)
        .spawn()
        .expect("lanza r00t corre");

    let hay_copia = esperar_hasta(Duration::from_secs(15), || {
        !copias_vivas(&e.usuario).is_empty()
    });
    assert!(hay_copia, "la copia de la credencial no apareció");
    let copia = copias_vivas(&e.usuario).remove(0);
    // El `run` del `podman` falso debe estar en curso antes de la señal.
    let pidfile = e._tmp.path().join("bin").join("hijo.pid");
    let hay_hijo = esperar_hasta(Duration::from_secs(15), || {
        fs::read_to_string(&pidfile).is_ok_and(|texto| texto.trim().parse::<u32>().is_ok())
    });
    assert!(hay_hijo, "el podman falso no arrancó");
    let pid_hijo: u32 = fs::read_to_string(&pidfile)
        .expect("pid del hijo")
        .trim()
        .parse()
        .expect("pid numérico");

    let enviada = Command::new("kill")
        .args([senal, &hijo.id().to_string()])
        .status()
        .expect("kill");
    assert!(enviada.success(), "no se pudo mandar {senal}");

    let estado = esperar_salida(&mut hijo, Duration::from_secs(15));
    assert_eq!(estado.code(), Some(codigo), "código tras {senal}");
    assert!(
        fs::symlink_metadata(&copia).is_err(),
        "la copia se borra al recibir {senal}: {}",
        copia.display()
    );
    let vivo = proceso_vivo(pid_hijo);
    if vivo {
        // Limpieza: el hijo no debe quedar corriendo si la prueba falla.
        let _ = Command::new("kill")
            .args(["-KILL", &pid_hijo.to_string()])
            .status();
    }
    assert!(
        !vivo,
        "el podman falso sigue vivo tras {senal}: pid {pid_hijo}"
    );
}

#[test]
fn corre_sigterm_quita_la_copia_de_la_credencial_y_sale_con_143() {
    senal_quita_la_copia_y_sale_con("-TERM", 143);
}

#[test]
fn corre_sigint_quita_la_copia_de_la_credencial_y_sale_con_130() {
    senal_quita_la_copia_y_sale_con("-INT", 130);
}
