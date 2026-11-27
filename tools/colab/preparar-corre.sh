#!/usr/bin/env bash
# Prepara el runtime de Colab para `corre`. Corre como root en Ubuntu 24.04.
# Llama a preparar-runtime.sh, instala las herramientas de red con apt, baja Node de nodejs.org
# (la versión NODE_VERSION de versiones-builder.env, con SHA-256 revisado), instala claude con npm
# --prefix /usr/local (claude y node tienen que quedar bajo /usr, donde bwrap los monta), fija
# claude, node y rustc, crea el usuario builder, mueve el root-home del paquete, crea la raíz del HOME
# del r00t (ROOT_USUARIO_DIR, con .cache/root/corridas), fija safe.directory '*' en el git del sistema y
# deja el plugin en su ruta original.
# Termina llamando a red-builder.sh (lo trae la Tarea 171).
# Uso: preparar-corre.sh <carpeta_del_paquete>
set -euo pipefail

ROOT_PREPARAR_RUNTIME="${ROOT_PREPARAR_RUNTIME:-$(dirname "$0")/preparar-runtime.sh}"
ROOT_RED_BUILDER_SH="${ROOT_RED_BUILDER_SH:-$(dirname "$0")/red-builder.sh}"
ROOT_BIN_DIR="${ROOT_BIN_DIR:-/usr/local/bin}"
ROOT_HOME_DESTINO="${ROOT_HOME_DESTINO:-/root/root-home}"
ROOT_DIR_RAIZ="${ROOT_DIR_RAIZ:-/root}"
ROOT_RAIZ_DIR="${ROOT_RAIZ_DIR:-}"
# Raíz del HOME del r00t. Cada nivel es de root y modo 0711: el builder pasa pero no lista. Los tests la cambian.
ROOT_USUARIO_DIR="${ROOT_USUARIO_DIR:-/srv/root-usuario}"
ROOT_NPM="${ROOT_NPM:-/usr/local/bin/npm}"
ROOT_PREFIJO_USR="${ROOT_PREFIJO_USR:-/usr/}"
# Node se baja con curl de ROOT_NODE_URL y se extrae en ROOT_PREFIJO_NODE. Los tests usan falsos.
ROOT_CURL="${ROOT_CURL:-curl}"
ROOT_NODE_URL="${ROOT_NODE_URL:-https://nodejs.org/dist}"
ROOT_PREFIJO_NODE="${ROOT_PREFIJO_NODE:-/usr/local}"
ROOT_PATH_BUILDER="${ROOT_PATH_BUILDER:-/usr/local/cargo/bin:/usr/local/bin:/usr/bin:/bin}"
# setpriv, usuario y chown del builder. Se pueden cambiar; los tests usan falsos.
ROOT_SETPRIV="${ROOT_SETPRIV:-setpriv}"
ROOT_USUARIO_BUILDER="${ROOT_USUARIO_BUILDER:-builder}"
ROOT_CHOWN="${ROOT_CHOWN:-chown}"
# Rust lo instala preparar-runtime.sh en estos valores. Son los mismos de ese guion.
ROOT_RUSTUP_HOME="${ROOT_RUSTUP_HOME:-/usr/local/rustup}"
ROOT_CARGO_HOME="${ROOT_CARGO_HOME:-/usr/local/cargo}"
export ROOT_BIN_DIR ROOT_HOME_DESTINO ROOT_DIR_RAIZ ROOT_RAIZ_DIR ROOT_NPM ROOT_PREFIJO_USR ROOT_PATH_BUILDER
export ROOT_USUARIO_DIR
export ROOT_RUSTUP_HOME ROOT_CARGO_HOME

# Paquetes que el builder necesita. Igual que el par update + install de preparar-runtime.sh.
# Node no va por apt: la versión de apt no sirve para Claude Code. Va en instalar_node.
PAQUETES_APT=(xz-utils slirp4netns iptables iproute2)
INTENTOS_APT=3
APT_OPCIONES=(-o Acquire::Retries=5 -o Acquire::http::Timeout=30)

# Imprime el nombre del paso, corre el comando y muestra su tiempo.
paso() {
    local nombre="$1"
    shift
    echo "paso $nombre"
    SECONDS=0
    "$@"
    echo "tiempo $nombre: ${SECONDS}s"
}

# Lee una clave de versiones-builder.env. Sale vacía si falta.
clave() {
    grep -m1 "^$1=" "$archivo" | cut -d= -f2- || true
}

# Paso 1: el runtime base. Si falla, este script falla.
# preparar-runtime.sh corre en otro proceso: sus exports no llegan acá. Por eso se exportan de nuevo.
llamar_runtime() {
    if ! bash "$ROOT_PREPARAR_RUNTIME" "$carpeta"; then
        echo "mal preparar-runtime.sh: no terminó el runtime base"
        exit 1
    fi
    export RUSTUP_HOME="$ROOT_RUSTUP_HOME"
    export CARGO_HOME="$ROOT_CARGO_HOME"
    PATH="$ROOT_CARGO_HOME/bin:$PATH"
    export PATH
}

# Paso 2: instala los paquetes de apt. El par update + install se reintenta entero.
# ROOT_APT_ESPERA son los segundos entre intentos (30 por omisión). Los tests usan 0.
instalar_paquetes() {
    local intento codigo=0
    for ((intento = 1; intento <= INTENTOS_APT; intento++)); do
        codigo=0
        { apt-get "${APT_OPCIONES[@]}" update -qq &&
            apt-get "${APT_OPCIONES[@]}" install -y "${PAQUETES_APT[@]}"; } || codigo=$?
        if [ "$codigo" -eq 0 ]; then
            return 0
        fi
        if [ "$intento" -lt "$INTENTOS_APT" ]; then
            echo "intento $intento de apt-get falló (código $codigo): se reintenta en ${ROOT_APT_ESPERA:-30} s"
            sleep "${ROOT_APT_ESPERA:-30}"
        fi
    done
    echo "mal apt-get: falló después de $INTENTOS_APT intentos (código $codigo)"
    exit "$codigo"
}

# Revisa que un comando resuelva bajo ROOT_PREFIJO_USR con el PATH del builder.
# Sale con 1 si no lo encuentra o si la ruta real (readlink -f) cae fuera del prefijo.
revisar_bajo_usr() {
    local nombre="$1" hallado real
    if ! hallado="$(PATH="$ROOT_PATH_BUILDER" command -v "$nombre")" || [ -z "$hallado" ]; then
        echo "mal $nombre: command -v no encuentra $nombre con el PATH del builder ($ROOT_PATH_BUILDER)"
        exit 1
    fi
    real="$(readlink -f "$hallado")"
    case "$real" in
        "$ROOT_PREFIJO_USR"*) ;;
        *)
            echo "mal $nombre: la ruta real es $real y debe estar bajo $ROOT_PREFIJO_USR"
            exit 1
            ;;
    esac
}

# Paso 2b: baja Node NODE_VERSION de ROOT_NODE_URL y lo extrae en ROOT_PREFIJO_NODE. Antes compara el
# SHA-256 del tarball con la línea de SHASUMS256.txt. Si no coincide, sale con 1 sin extraer nada.
# El temporal sale de mktemp (respeta TMPDIR) y se borra en todos los caminos.
instalar_node() {
    local temporal archivo base esperado bajado
    archivo="node-v$NODE_VERSION-linux-x64.tar.xz"
    base="$ROOT_NODE_URL/v$NODE_VERSION"
    temporal="$(mktemp -d)"
    if ! "$ROOT_CURL" -fsSL -o "$temporal/$archivo" "$base/$archivo" ||
        ! "$ROOT_CURL" -fsSL -o "$temporal/SHASUMS256.txt" "$base/SHASUMS256.txt"; then
        rm -rf "$temporal"
        echo "mal node: no se pudo bajar $base/$archivo"
        exit 1
    fi
    esperado="$(awk -v f="$archivo" '$2 == f { print $1; exit }' "$temporal/SHASUMS256.txt")"
    bajado="$(sha256sum "$temporal/$archivo" | cut -d' ' -f1)"
    if [ -z "$esperado" ] || [ "$bajado" != "$esperado" ]; then
        rm -rf "$temporal"
        echo "mal node: el SHA-256 de $archivo no coincide (SHASUMS256.txt dice ${esperado:-nada}, el archivo da $bajado)"
        exit 1
    fi
    if ! tar -xJf "$temporal/$archivo" -C "$ROOT_PREFIJO_NODE" --strip-components=1; then
        rm -rf "$temporal"
        echo "mal node: tar no pudo extraer $archivo en $ROOT_PREFIJO_NODE"
        exit 1
    fi
    rm -rf "$temporal"
}

# Paso 3: instala claude fijado con el npm de apt y --prefix /usr/local. Revisa que claude y node
# queden bajo /usr y que claude --version (con el PATH del builder) dé la versión fijada.
# Revisa las versiones de claude y de rustc. Si no coinciden con versiones-builder.env, sale con 1.
instalar_y_verificar_versiones() {
    "$ROOT_NPM" install -g --prefix /usr/local "@anthropic-ai/claude-code@$CLAUDE_VERSION"
    revisar_bajo_usr claude
    revisar_bajo_usr node

    local salida primero segundo
    salida="$(env PATH="$ROOT_PATH_BUILDER" claude --version)" || {
        echo "mal versiones: claude --version falló"
        exit 1
    }
    read -r primero _ <<<"$salida"
    if [ "$primero" != "$CLAUDE_VERSION" ]; then
        echo "mal versiones: claude --version da $primero y versiones-builder.env dice $CLAUDE_VERSION"
        exit 1
    fi

    salida="$(env PATH="$ROOT_PATH_BUILDER" node --version)" || {
        echo "mal versiones: node --version falló"
        exit 1
    }
    read -r primero _ <<<"$salida"
    if [ "${primero#v}" != "$NODE_VERSION" ]; then
        echo "mal versiones: node --version da ${primero#v} y versiones-builder.env dice $NODE_VERSION"
        exit 1
    fi

    salida="$(rustc --version)" || {
        echo "mal versiones: rustc --version falló"
        exit 1
    }
    read -r _ segundo _ <<<"$salida"
    if [ "$segundo" != "$RUST_VERSION" ]; then
        echo "mal versiones: rustc --version da $segundo y versiones-builder.env dice $RUST_VERSION"
        exit 1
    fi
    revisar_entorno_builder
}

# Corre cargo, rustc, node, claude y git como el builder, con entorno limpio (env -i). El CARGO_HOME es un
# temporal de este guion, dado al builder. Lo borra al final, pase o no la revisión. Si uno falla, sale con 1.
revisar_entorno_builder() {
    local uid gid temporal nombre fallo=""
    if ! uid="$(id -u "$ROOT_USUARIO_BUILDER")" || ! gid="$(id -g "$ROOT_USUARIO_BUILDER")"; then
        echo "mal versiones: no se encuentra el usuario $ROOT_USUARIO_BUILDER"
        exit 1
    fi
    temporal="$(mktemp -d)"
    if ! "$ROOT_CHOWN" "$ROOT_USUARIO_BUILDER" "$temporal"; then
        rm -rf "$temporal"
        echo "mal versiones: no se pudo dar $temporal al usuario $ROOT_USUARIO_BUILDER"
        exit 1
    fi
    for nombre in cargo rustc node claude git; do
        if ! "$ROOT_SETPRIV" --reuid="$uid" --regid="$gid" --clear-groups \
            env -i PATH="$ROOT_PATH_BUILDER" RUSTUP_HOME="$ROOT_RUSTUP_HOME" CARGO_HOME="$temporal" \
            "$nombre" --version >/dev/null; then
            fallo="$nombre"
            break
        fi
    done
    rm -rf "$temporal"
    if [ -n "$fallo" ]; then
        echo "mal versiones: $fallo --version falló con el entorno del builder (PATH=$ROOT_PATH_BUILDER)"
        exit 1
    fi
}

# Paso 3: crea el usuario builder (antes de `versiones`, que corre comandos como él) si no existe. Sin carpeta propia ni grupos extra.
crear_builder() {
    if ! id builder >/dev/null 2>&1; then
        useradd --no-create-home --shell /usr/sbin/nologin builder
    fi
}

# Copia cada plugin del manifiesto a su ruta absoluta original, bajo ROOT_RAIZ_DIR.
# La copia es byte a byte: cp -a conserva contenido y permisos.
copiar_plugins() {
    local pares origen_absoluto relativa destino
    pares="$(python3 -I -c '
import json, sys
with open(sys.argv[1], "rb") as f:
    manifiesto = json.load(f)
for origen, relativa in manifiesto.get("plugins", {}).items():
    print(origen + "\t" + relativa)
' "$carpeta/manifiesto.json")"
    if [ -z "$pares" ]; then
        return 0
    fi
    while IFS=$'\t' read -r origen_absoluto relativa; do
        case "$relativa" in
            "" | /* | *..*)
                echo "mal manifiesto.json: ruta de plugin inválida: $relativa"
                exit 1
                ;;
        esac
        destino="${ROOT_RAIZ_DIR}$origen_absoluto"
        mkdir -p "$(dirname "$destino")"
        cp -a "$carpeta/$relativa" "$destino"
    done <<<"$pares"
}

# Crea la raíz del HOME del r00t: ROOT_USUARIO_DIR, .cache, .cache/root y .cache/root/corridas. Cada nivel
# es de root con modo 0711: el builder puede pasar por ellos (buscar) pero no listarlos.
crear_raiz_usuario() {
    local ruta
    for ruta in "$ROOT_USUARIO_DIR" "$ROOT_USUARIO_DIR/.cache" "$ROOT_USUARIO_DIR/.cache/root" \
        "$ROOT_USUARIO_DIR/.cache/root/corridas"; do
        mkdir -p "$ruta"
        chown root:root "$ruta"
        chmod 0711 "$ruta"
    done
}

# Pone safe.directory '*' en el git del sistema, una sola vez. El builder deja archivos de su dueño en repo/;
# sin esto, el git de r00t falla con "dubious ownership".
fijar_safe_directory() {
    if git config --system --get-all safe.directory 2>/dev/null | grep -qxF '*'; then
        return 0
    fi
    git config --system --add safe.directory '*'
}

# Paso 5: arma el ROOT_HOME. Mueve (no copia) root-home, lo deja solo para root y pone r00t.
armar_root_home() {
    local origen="$carpeta/root-home"
    if [ ! -d "$origen" ]; then
        echo "mal root-home: falta $origen"
        exit 1
    fi
    if [ -e "$ROOT_HOME_DESTINO" ]; then
        echo "mal root-home: ya existe $ROOT_HOME_DESTINO"
        exit 1
    fi
    mkdir -p "$(dirname "$ROOT_HOME_DESTINO")"
    mv "$origen" "$ROOT_HOME_DESTINO"
    chown -R root:root "$ROOT_HOME_DESTINO"
    chmod 0700 "$ROOT_HOME_DESTINO"
    chmod 0700 "$ROOT_DIR_RAIZ"
    chmod 0700 "$carpeta"
    crear_raiz_usuario
    fijar_safe_directory

    mkdir -p "$ROOT_BIN_DIR"
    install -m 0755 "$carpeta/r00t" "$ROOT_BIN_DIR/r00t"
    copiar_plugins
}

# Paso 6: corre red-builder.sh. Si falta, sale con 1 y nombra la tarea que lo trae.
correr_red_builder() {
    if [ ! -f "$ROOT_RED_BUILDER_SH" ]; then
        echo "mal red-builder.sh: falta $ROOT_RED_BUILDER_SH (lo trae la Tarea 171)"
        exit 1
    fi
    bash "$ROOT_RED_BUILDER_SH"
}

# Cuerpo principal. Corre solo si el script se ejecuta directo. Con `source` solo se definen las funciones.
principal() {
    carpeta="${1:-}"
    if [ -z "$carpeta" ]; then
        echo "mal preparar-corre.sh: falta la carpeta del paquete"
        exit 1
    fi

    archivo="$carpeta/versiones-builder.env"
    if [ ! -f "$archivo" ]; then
        echo "mal preparar-corre.sh: falta $archivo"
        exit 1
    fi
    for clave_requerida in CLAUDE_VERSION RUST_VERSION NODE_VERSION; do
        if [ -z "$(clave "$clave_requerida")" ]; then
            echo "mal versiones-builder.env: falta la clave $clave_requerida"
            exit 1
        fi
    done
    CLAUDE_VERSION="$(clave CLAUDE_VERSION)"
    RUST_VERSION="$(clave RUST_VERSION)"
    NODE_VERSION="$(clave NODE_VERSION)"

    paso runtime llamar_runtime
    paso paquetes instalar_paquetes
    paso node instalar_node
    paso builder crear_builder
    paso versiones instalar_y_verificar_versiones
    paso root-home armar_root_home
    paso red-builder correr_red_builder
}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then
    principal "$@"
fi
