//! `ROOT_CLAVES_FD`: la clave entra por un descriptor y no llega a los hijos de r00t.
//! Ningún test sale a la red: los errores salen antes de tocar nada, y el hijo es un `git` falso.

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

const CLAVE_FALSA: &str = "sk-falsa-123";
/// Descriptor que usan los tests. No es 0, 1 ni 2.
const FD_CLAVES: &str = "7";

/// Lanza r00t con `args`. `contenido` va a una tubería que llega como descriptor 7.
/// El shell hace `exec` del binario, así que el descriptor hereda sin pasar por otro proceso.
fn r00t_con_tuberia(contenido: &[u8], home: &Path, path: &str, args: &[&str]) -> Output {
    let (lector, mut escritor) = std::io::pipe().expect("tubería");
    escritor.write_all(contenido).expect("escribe la tubería");
    drop(escritor);
    Command::new("/bin/sh")
        .arg("-c")
        .arg(r#"exec "$0" "$@" 7<&0"#)
        .arg(env!("CARGO_BIN_EXE_r00t"))
        .args(args)
        .stdin(Stdio::from(lector))
        .env("ROOT_CLAVES_FD", FD_CLAVES)
        .env("ROOT_HOME", home)
        .env("ROOT_SIN_PODMAN", "1")
        .env("PATH", path)
        .env_remove("DEEPSEEK_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .output()
        .expect("lanza r00t")
}

/// Argumentos de `colab empaquetar` con un trabajo y un repo. Llama a `git` antes de podman.
fn args_empaquetar(tmp: &Path) -> Vec<String> {
    let raiz = tmp.join("raiz");
    fs::create_dir_all(&raiz).expect("raiz");
    vec![
        "colab".into(),
        "empaquetar".into(),
        "--binario".into(),
        tmp.join("r00t").display().to_string(),
        "--trabajos".into(),
        "t:eficiencia".into(),
        "--repo".into(),
        format!("t={}", tmp.join("repo").display()),
        "--rustup-sha256".into(),
        "0".repeat(64),
        "--salida".into(),
        tmp.join("paquete.tar.gz").display().to_string(),
        "--raiz-root".into(),
        raiz.display().to_string(),
    ]
}

#[test]
fn claves_fd_no_llegan_al_hijo_ni_en_entorno_ni_en_descriptores() {
    let tmp = TempDir::new().expect("tmp");
    let bin = tmp.path().join("bin");
    fs::create_dir(&bin).expect("bin");
    let registro = tmp.path().join("registro.txt");
    // El git falso vuelca su entorno y sus descriptores, y sale con error para no seguir.
    let guion = format!(
        "#!/bin/bash\n{{\n  echo '--- env'\n  env\n  echo '--- fd'\n  ls -l /proc/self/fd\n}} > '{}'\nexit 1\n",
        registro.display()
    );
    let git = bin.join("git");
    fs::write(&git, guion).expect("git falso");
    fs::set_permissions(&git, fs::Permissions::from_mode(0o755)).expect("permisos");
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let home = tmp.path().join("home");
    fs::create_dir(&home).expect("home");
    let argumentos = args_empaquetar(tmp.path());
    let argumentos: Vec<&str> = argumentos.iter().map(String::as_str).collect();

    let contenido = format!("DEEPSEEK_API_KEY={CLAVE_FALSA}\n");
    let salida = r00t_con_tuberia(contenido.as_bytes(), &home, &path, &argumentos);

    let texto_registro =
        fs::read_to_string(&registro).expect("el git falso no corrió: no hay registro");
    assert!(!texto_registro.contains(CLAVE_FALSA), "{texto_registro}");
    assert!(
        !texto_registro.contains("DEEPSEEK_API_KEY"),
        "{texto_registro}"
    );
    assert!(
        !texto_registro.contains("ROOT_CLAVES_FD"),
        "{texto_registro}"
    );
    // El fd 7 es la tubería de la clave. Los fd 0, 1 y 2 son los del arnés y no cuentan.
    assert!(!texto_registro.contains(" 7 -> "), "{texto_registro}");

    let todo = format!(
        "{}{}",
        String::from_utf8_lossy(&salida.stdout),
        String::from_utf8_lossy(&salida.stderr)
    );
    assert!(!todo.contains(CLAVE_FALSA), "{todo}");
    assert!(!todo.contains("ROOT_CLAVES_FD"), "{todo}");
}

#[test]
fn claves_fd_que_no_es_numero_sale_con_2_sin_mostrar_el_valor() {
    let tmp = TempDir::new().expect("tmp");
    let salida = Command::new(env!("CARGO_BIN_EXE_r00t"))
        .args(["esquema", "veredicto"])
        .env("ROOT_CLAVES_FD", "fd-x9")
        .env("ROOT_HOME", tmp.path())
        .output()
        .expect("lanza r00t");
    assert_eq!(salida.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&salida.stderr);
    assert!(!stderr.contains("fd-x9"), "{stderr}");
}

#[test]
fn claves_fd_descriptor_cerrado_sale_con_2() {
    let tmp = TempDir::new().expect("tmp");
    let salida = Command::new(env!("CARGO_BIN_EXE_r00t"))
        .args(["esquema", "veredicto"])
        .env("ROOT_CLAVES_FD", "999")
        .env("ROOT_HOME", tmp.path())
        .output()
        .expect("lanza r00t");
    assert_eq!(salida.status.code(), Some(2));
}

#[test]
fn claves_fd_linea_de_otra_clave_sale_con_2_sin_mostrar_el_valor() {
    let tmp = TempDir::new().expect("tmp");
    let salida = r00t_con_tuberia(
        b"OTRA=valorsecreto99\n",
        tmp.path(),
        &std::env::var("PATH").unwrap_or_default(),
        &["esquema", "veredicto"],
    );
    assert_eq!(salida.status.code(), Some(2));
    let todo = format!(
        "{}{}",
        String::from_utf8_lossy(&salida.stdout),
        String::from_utf8_lossy(&salida.stderr)
    );
    assert!(!todo.contains("valorsecreto99"), "{todo}");
}

// ---- tools/colab/notebook-juez.py ----
// Cada test corre Python en un proceso aparte. `-I -B`: sin entorno PYTHON* ni __pycache__.
// Si no hay python3 en PATH, el test avisa en stderr y sale sin fallar.

/// Guion del juez. Se carga por ruta absoluta: `-I` no agrega el directorio actual al path.
const GUION_JUEZ: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tools/colab/notebook-juez.py");

/// Carga el guion como `j`. El guion llega en `sys.argv[1]`.
const CARGAR_JUEZ: &str = r#"
import importlib.util, sys
spec = importlib.util.spec_from_file_location("notebook_juez", sys.argv[1])
j = importlib.util.module_from_spec(spec)
spec.loader.exec_module(j)
"#;

/// Corre `codigo` con `python3 -I -B`. `sys.argv[1]` es el guion y `sys.argv[2]` la carpeta temporal.
fn correr_python(codigo: &str, tmp: &Path) -> Option<Output> {
    match Command::new("python3")
        .args(["-I", "-B", "-c", codigo, GUION_JUEZ])
        .arg(tmp)
        .output()
    {
        Ok(salida) => Some(salida),
        Err(error) => {
            eprintln!("python3 no está en PATH: se saltan los tests del notebook-juez ({error})");
            None
        }
    }
}

/// Código de test: carga el guion y corre `cuerpo`.
fn codigo_juez(cuerpo: &str) -> String {
    format!("{CARGAR_JUEZ}\n{cuerpo}")
}

fn python_ok(salida: &Output) {
    assert!(
        salida.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&salida.stdout),
        String::from_utf8_lossy(&salida.stderr)
    );
}

#[test]
fn claves_fd_juez_compila_sin_escribir_en_el_repo() {
    let tmp = tempfile::tempdir().expect("tmp");
    let codigo = "import py_compile, os, sys\n\
        py_compile.compile(sys.argv[1], cfile=os.path.join(sys.argv[2], 'juez.pyc'), doraise=True)\n";
    let Some(salida) = correr_python(codigo, tmp.path()) else {
        return;
    };
    python_ok(&salida);
}

#[test]
fn claves_fd_juez_se_importa_sin_google_colab() {
    let tmp = tempfile::tempdir().expect("tmp");
    let cuerpo = r#"
assert "google" not in sys.modules, "importar el guion trajo google"
for linea in open(sys.argv[1], encoding="utf-8"):
    assert not linea.startswith(("import google", "from google")), linea
for celda in ("celda_1", "celda_2", "celda_3", "celda_4"):
    assert callable(getattr(j, celda)), celda
"#;
    let Some(salida) = correr_python(&codigo_juez(cuerpo), tmp.path()) else {
        return;
    };
    python_ok(&salida);
}

#[test]
fn claves_fd_juez_regla_de_parada_usa_promedio_ponderado_y_umbral() {
    let tmp = tempfile::tempdir().expect("tmp");
    let cuerpo = r#"
def f(sens, defectos):
    return {"tarea": "t", "dimension": "eficiencia", "sensibilidad": sens, "defectos": defectos}
assert j.regla_de_parada([f(0.80, 10)]) is True
assert j.regla_de_parada([f(0.90, 10)]) is False
assert j.regla_de_parada([]) is False
assert j.regla_de_parada([f(0.85, 10)]) is False, "0.85 exacto no salta"
# Promedio simple da 0.895 (no salta). Ponderado por defectos da 0.819 (salta).
assert j.regla_de_parada([f(0.99, 1), f(0.80, 9)]) is True
assert j.regla_de_parada([f(0.95, 10), f(0.90, 10)]) is False
assert j.regla_de_parada([{"sensibilidad": None, "defectos": 10}]) is False
assert j.regla_de_parada([f(0.10, 0)]) is False, "sin defectos no cuenta"
"#;
    let Some(salida) = correr_python(&codigo_juez(cuerpo), tmp.path()) else {
        return;
    };
    python_ok(&salida);
}

#[test]
fn claves_fd_juez_ya_calibrado_se_salta_solo_con_fila_nueva() {
    let tmp = tempfile::tempdir().expect("tmp");
    let cuerpo = r#"
trabajo = {"tarea": "t", "dimension": "eficiencia", "n": 5, "semilla": 1}
fila = {"tarea": "t", "dimension": "eficiencia", "n": 5, "semilla": 1, "ts": "2026-11-14T10:00:00Z"}
assert j.ya_calibrado(trabajo, [fila], []) is True
assert j.ya_calibrado(trabajo, [], []) is False
assert j.ya_calibrado(trabajo, [dict(fila, n=7)], []) is False
# La misma fila, pero viene del paquete de ida: no cuenta.
assert j.ya_calibrado(trabajo, [fila], ["2026-11-14T10:00:00Z"]) is False
"#;
    let Some(salida) = correr_python(&codigo_juez(cuerpo), tmp.path()) else {
        return;
    };
    python_ok(&salida);
}

#[test]
fn claves_fd_juez_esperar_archivo_encuentra_archivo_tardio_y_falla_con_la_ruta() {
    let tmp = tempfile::tempdir().expect("tmp");
    let cuerpo = r#"
import os
d = sys.argv[2]
reloj = [0.0]
llamadas = [0]
def ahora():
    return reloj[0]
def dormir(segundos):
    llamadas[0] += 1
    reloj[0] += segundos
    if llamadas[0] == 2:
        with open(os.path.join(d, "tarde.json"), "w") as f:
            f.write("{}")
j.esperar_archivo(os.path.join(d, "tarde.json"), plazo_s=10, intervalo_s=1, dormir=dormir, ahora=ahora)
assert llamadas[0] == 2, llamadas
nunca = os.path.join(d, "nunca.json")
reloj[0] = 0.0
try:
    j.esperar_archivo(nunca, plazo_s=3, intervalo_s=1, dormir=dormir, ahora=ahora)
except FileNotFoundError as error:
    assert nunca in str(error), str(error)
else:
    raise AssertionError("debió fallar al pasar el plazo")
"#;
    let Some(salida) = correr_python(&codigo_juez(cuerpo), tmp.path()) else {
        return;
    };
    python_ok(&salida);
}

#[test]
fn claves_fd_juez_copiar_y_verificar_con_sha_distinto_no_deja_copia_ni_abre_tar() {
    let tmp = tempfile::tempdir().expect("tmp");
    let cuerpo = r#"
import hashlib, os, tarfile
d = sys.argv[2]
origen = os.path.join(d, "paquete.tar.gz")
with tarfile.open(origen, "w:gz") as t:
    t.add(sys.argv[1], arcname="juez.py")
sha = hashlib.sha256(open(origen, "rb").read()).hexdigest()
abiertos = []
j.abrir_paquete = lambda *a: abiertos.append(a)
destino = os.path.join(d, "copia.tar.gz")
malo = "0" * 64
try:
    j.copiar_y_verificar(origen, destino, malo)
except ValueError as error:
    assert malo in str(error) and sha in str(error), str(error)
else:
    raise AssertionError("debió fallar con SHA distinto")
assert not os.path.exists(destino), "quedó la copia con SHA malo"
assert abiertos == [], "no debe abrir el tar antes de verificar"
j.copiar_y_verificar(origen, destino, sha)
assert os.path.isfile(destino), "con SHA igual debe copiar"
"#;
    let Some(salida) = correr_python(&codigo_juez(cuerpo), tmp.path()) else {
        return;
    };
    python_ok(&salida);
}

#[test]
fn claves_fd_juez_lanzar_con_clave_entrega_la_clave_por_el_fd() {
    let tmp = tempfile::tempdir().expect("tmp");
    // Hijo falso: lee el descriptor de ROOT_CLAVES_FD y registra solo un hash y los nombres de su entorno.
    let hijo = tmp.path().join("hijo.py");
    fs::write(
        &hijo,
        "import hashlib, os\n\
         fd = int(os.environ['ROOT_CLAVES_FD'])\n\
         with os.fdopen(fd, 'rb') as f:\n\
         \x20   dato = f.read()\n\
         print('hash=' + hashlib.sha256(dato).hexdigest())\n\
         for nombre in sorted(os.environ):\n\
         \x20   print('env=' + nombre)\n",
    )
    .expect("hijo falso");
    let cuerpo = format!(
        r#"
import hashlib, os
d = sys.argv[2]
hijo = {hijo:?}
entorno = {{"PATH": os.environ.get("PATH", ""), "NORMAL": "si", "ALGO_KEY": "x",
            "OTRO_TOKEN": "y", "DEEPSEEK_API_KEY": "viejo"}}
log = os.path.join(d, "trabajo.log")
codigo = j.lanzar_con_clave([sys.executable, "-I", hijo], entorno, "sk-falsa-xyz", d, log)
assert codigo == 0, codigo
esperado = hashlib.sha256(b"DEEPSEEK_API_KEY=sk-falsa-xyz\n").hexdigest()
texto = open(log, encoding="utf-8").read()
assert "hash=" + esperado in texto, texto
assert "env=NORMAL" in texto and "env=ROOT_CLAVES_FD" in texto, texto
for prohibido in ("sk-falsa-xyz", "DEEPSEEK_API_KEY", "ALGO_KEY", "OTRO_TOKEN"):
    assert prohibido not in texto, texto
"#
    );
    let Some(salida) = correr_python(&codigo_juez(&cuerpo), tmp.path()) else {
        return;
    };
    python_ok(&salida);
    let todo = format!(
        "{}{}",
        String::from_utf8_lossy(&salida.stdout),
        String::from_utf8_lossy(&salida.stderr)
    );
    assert!(!todo.contains("sk-falsa-xyz"), "{todo}");
}

#[test]
fn claves_fd_juez_resto_del_tope_resta_lo_gastado_y_redondea_hacia_abajo() {
    let tmp = tempfile::tempdir().expect("tmp");
    let cuerpo = r#"
assert j.resto_del_tope(8, 7.5) == 0.5
assert j.resto_del_tope(8, 0) == 8.0
assert j.resto_del_tope(8, 8) == 0.0
assert j.resto_del_tope(8, 9) == 0.0, "nunca negativo"
# 8 - 7.13 = 0.87 en binario puede quedar en 0.8699999: el redondeo a centavos no lo baja.
assert j.resto_del_tope(8, 7.13) == 0.87, j.resto_del_tope(8, 7.13)
# 0.505 USD de resto baja a 0.50 (hacia abajo).
assert j.resto_del_tope(8, 7.495) == 0.5, j.resto_del_tope(8, 7.495)
"#;
    let Some(salida) = correr_python(&codigo_juez(cuerpo), tmp.path()) else {
        return;
    };
    python_ok(&salida);
}

#[test]
fn claves_fd_juez_con_tope_cambia_solo_el_valor_de_tope_usd() {
    let tmp = tempfile::tempdir().expect("tmp");
    let cuerpo = r#"
args = ["oraculo", "calibrar", "--n", "5", "--tope-usd", "16.90", "--json"]
nuevos = j.con_tope(args, 0.5)
assert nuevos == ["oraculo", "calibrar", "--n", "5", "--tope-usd", "0.50", "--json"], nuevos
assert args[5] == "16.90", "la lista original no cambia"
oraculo = ["oraculo", "calibrar", "--n", "5", "--json"]
assert j.con_tope(oraculo, 0.5) == oraculo
"#;
    let Some(salida) = correr_python(&codigo_juez(cuerpo), tmp.path()) else {
        return;
    };
    python_ok(&salida);
}

#[test]
fn claves_fd_juez_decidir_tope_pasa_el_resto_al_trabajo() {
    let tmp = tempfile::tempdir().expect("tmp");
    let cuerpo = r#"
trabajo = {"tarea": "t", "dimension": "calidad", "n": 5, "semilla": 1,
           "args": ["oraculo", "calibrar", "--tope-usd", "16.90", "--json"], "entorno": {}}
# Tope 8 y 7.50 gastados: el trabajo corre con 0.50.
corre, resto = j.decidir_tope(trabajo, 8, 7.5)
assert corre is not None and resto == 0.5, (corre, resto)
assert corre["args"] == ["oraculo", "calibrar", "--tope-usd", "0.50", "--json"], corre
assert corre is not trabajo and trabajo["args"][3] == "16.90", "el trabajo original no cambia"
# Tope 8 y 8 gastados: no corre.
corre, resto = j.decidir_tope(trabajo, 8, 8)
assert corre is None and resto == 0.0, (corre, resto)
# Sin tope en el manifiesto, el trabajo sigue igual.
assert j.decidir_tope(trabajo, None, 8)[0] is trabajo
# Trabajo de oráculo, sin --tope-usd: queda igual aunque el tope esté agotado.
oraculo = {"tarea": "t", "dimension": "eficiencia", "n": 5, "semilla": 1,
           "args": ["oraculo", "calibrar", "--json"], "entorno": {}}
assert j.decidir_tope(oraculo, 8, 9)[0] is oraculo
assert j.decidir_tope(oraculo, 8, 0)[0] is oraculo
"#;
    let Some(salida) = correr_python(&codigo_juez(cuerpo), tmp.path()) else {
        return;
    };
    python_ok(&salida);
}

#[test]
fn claves_fd_juez_gastado_del_paquete_suma_rotados_y_actual() {
    let tmp = tempfile::tempdir().expect("tmp");
    let cuerpo = r#"
d = sys.argv[2]
def escribir(nombre, lineas):
    with open(os.path.join(d, nombre), "w", encoding="utf-8") as f:
        f.write("".join(lineas))
escribir("eventos.jsonl.1", ['{"tipo":"llamada","cost_usd":2.0}\n', '{"tipo":"reserva","cost_usd":9.0}\n'])
escribir("eventos.jsonl.2", ['{"tipo":"llamada","cost_usd":1.25}\n'])
escribir("eventos.jsonl", ['{"tipo":"llamada","cost_usd":0.25}\n', '{"tipo":"otro","cost_usd":5}\n'])
assert j.gastado_del_paquete(d) == 3.5, j.gastado_del_paquete(d)
"#;
    let Some(salida) = correr_python(&format!("import os\n{}", codigo_juez(cuerpo)), tmp.path())
    else {
        return;
    };
    python_ok(&salida);
}

#[test]
fn claves_fd_juez_gastado_del_paquete_sin_eventos_es_cero() {
    let tmp = tempfile::tempdir().expect("tmp");
    let cuerpo = r#"
assert j.gastado_del_paquete(sys.argv[2]) == 0.0
"#;
    let Some(salida) = correr_python(&codigo_juez(cuerpo), tmp.path()) else {
        return;
    };
    python_ok(&salida);
}

#[test]
fn claves_fd_juez_etapas_del_manifiesto_agrupa_por_dimension_consecutiva() {
    let tmp = tempfile::tempdir().expect("tmp");
    let cuerpo = r#"
def t(i, dimension):
    return {"tarea": "t%d" % i, "dimension": dimension, "n": 5, "semilla": i}
trabajos = [t(0, "eficiencia"), t(1, "eficiencia"), t(2, "calidad"), t(3, "calidad"), t(4, "eficiencia")]
etapas = j.etapas_del_manifiesto({"trabajos": trabajos})
assert [e["dimension"] for e in etapas] == ["eficiencia", "calidad", "eficiencia"], etapas
assert [[i for i, _ in e["trabajos"]] for e in etapas] == [[0, 1], [2, 3], [4]], etapas
assert etapas[0]["trabajos"][0] == (0, trabajos[0])
assert j.etapas_del_manifiesto({"trabajos": []}) == []
"#;
    let Some(salida) = correr_python(&codigo_juez(cuerpo), tmp.path()) else {
        return;
    };
    python_ok(&salida);
}

#[test]
fn claves_fd_juez_armar_vuelta_filtra_gasto_y_escribe_manifiesto() {
    let tmp = tempfile::tempdir().expect("tmp");
    let cuerpo = r#"
import hashlib, json, os, tarfile
base = sys.argv[2]

def escribir(ruta, texto):
    os.makedirs(os.path.dirname(ruta), exist_ok=True)
    with open(ruta, "w", encoding="utf-8") as f:
        f.write(texto)

def armar_carpeta(d, con_verificador):
    escribir(os.path.join(d, "calibracion.json"),
             json.dumps({"v": 1, "filas": [{"tarea": "t", "dimension": "eficiencia", "n": 5, "semilla": 1, "ts": "x"}]}))
    escribir(os.path.join(d, "calibracion", "caso-1.json"), "{}")
    escribir(os.path.join(d, "eventos.jsonl.1"), '{"tipo":"llamada","cost_usd":0.5}\n')
    escribir(os.path.join(d, "eventos.jsonl"),
             '{"tipo":"llamada","cost_usd":0.25}\n{"tipo":"reserva","cost_usd":1.0}\n{"tipo":"gate","cost_usd":9}\n')
    if con_verificador:
        escribir(os.path.join(d, "tools", "colab", "verificar-versiones.sh"),
                 "echo verificacion-falsa $1\n")

# Caso con verificador falso y gasto filtrado.
d1 = os.path.join(base, "D")
armar_carpeta(d1, True)
destino = os.path.join(base, "vuelta.tar.gz")
sha = j.armar_vuelta(d1, destino)
assert sha == hashlib.sha256(open(destino, "rb").read()).hexdigest(), "SHA devuelto distinto"
with tarfile.open(destino, "r:gz") as t:
    nombres = set(t.getnames())
    gasto = t.extractfile("gasto.jsonl").read().decode("utf-8")
    manifiesto = json.loads(t.extractfile("manifiesto-vuelta.json").read())
    verificacion = t.extractfile("verificacion.txt").read().decode("utf-8")
for esperado in ("verificacion.txt", "gasto.jsonl", "calibracion.json", "calibracion/caso-1.json", "manifiesto-vuelta.json"):
    assert esperado in nombres, (esperado, nombres)
lineas = gasto.splitlines()
assert len(lineas) == 3, lineas
assert all("gate" not in linea for linea in lineas), lineas
assert json.loads(lineas[0])["cost_usd"] == 0.5, "el rotado va primero"
assert manifiesto["v"] == 1
archivos = manifiesto["archivos"]
assert archivos["gasto.jsonl"] == hashlib.sha256(gasto.encode("utf-8")).hexdigest(), archivos
assert archivos["verificacion.txt"] == hashlib.sha256(verificacion.encode("utf-8")).hexdigest()
assert "calibracion/caso-1.json" in archivos, archivos
assert "manifiesto-vuelta.json" not in archivos
assert "verificacion-falsa" in verificacion, verificacion

# Caso sin verificador: la vuelta sale igual y el error queda en verificacion.txt.
d2 = os.path.join(base, "D2")
armar_carpeta(d2, False)
destino2 = os.path.join(base, "vuelta2.tar.gz")
j.armar_vuelta(d2, destino2)
with tarfile.open(destino2, "r:gz") as t:
    verificacion2 = t.extractfile("verificacion.txt").read().decode("utf-8")
assert "verificar-versiones.sh" in verificacion2, verificacion2
"#;
    let Some(salida) = correr_python(&codigo_juez(cuerpo), tmp.path()) else {
        return;
    };
    python_ok(&salida);
}
