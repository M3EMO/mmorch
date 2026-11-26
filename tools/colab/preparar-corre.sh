#!/usr/bin/env bash
# Prepara el runtime de Colab para `corre`. Corre como root en Ubuntu 24.04.
# Llama a preparar-runtime.sh, instala Node y las herramientas de red, instala claude con npm
# --prefix /usr/local (claude y node tienen que quedar bajo /usr, donde bwrap los monta), fija
# claude y rustc, crea el usuario builder, mueve el root-home del paquete y deja el plugin en su
# ruta original.
# Termina llamando a red-builder.sh (lo trae la Tarea 171).
# Uso: preparar-corre.sh <carpeta_del_paquete>
set -euo pipefail

ROOT_PREPARAR_RUNTIME="${ROOT_PREPARAR_RUNTIME:-$(dirname "$0")/preparar-runtime.sh}"
ROOT_RED_BUILDER_SH="${ROOT_RED_BUILDER_SH:-$(dirname "$0")/red-builder.sh}"
ROOT_BIN_DIR="${ROOT_BIN_DIR:-/usr/local/bin}"
ROOT_HOME_DESTINO="${ROOT_HOME_DESTINO:-/root/root-home}"
ROOT_DIR_RAIZ="${ROOT_DIR_RAIZ:-/root}"
ROOT_RAIZ_DIR="${ROOT_RAIZ_DIR:-}"
ROOT_NPM="${ROOT_NPM:-/usr/bin/npm}"
ROOT_PREFIJO_USR="${ROOT_PREFIJO_USR:-/usr/}"
ROOT_PATH_BUILDER="${ROOT_PATH_BUILDER:-/usr/local/bin:/usr/bin:/bin}"
export ROOT_BIN_DIR ROOT_HOME_DESTINO ROOT_DIR_RAIZ ROOT_RAIZ_DIR ROOT_NPM ROOT_PREFIJO_USR ROOT_PATH_BUILDER

# Paquetes que el builder necesita. Igual que el par update + install de preparar-runtime.sh.
PAQUETES_APT=(nodejs npm slirp4netns iptables iproute2)
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
llamar_runtime() {
    if ! bash "$ROOT_PREPARAR_RUNTIME" "$carpeta"; then
        echo "mal preparar-runtime.sh: no terminó el runtime base"
        exit 1
    fi
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

    salida="$(rustc --version)" || {
        echo "mal versiones: rustc --version falló"
        exit 1
    }
    read -r _ segundo _ <<<"$salida"
    if [ "$segundo" != "$RUST_VERSION" ]; then
        echo "mal versiones: rustc --version da $segundo y versiones-builder.env dice $RUST_VERSION"
        exit 1
    fi
}

# Paso 4: crea el usuario builder si no existe. Sin carpeta propia ni grupos extra.
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
    for clave_requerida in CLAUDE_VERSION RUST_VERSION; do
        if [ -z "$(clave "$clave_requerida")" ]; then
            echo "mal versiones-builder.env: falta la clave $clave_requerida"
            exit 1
        fi
    done
    CLAUDE_VERSION="$(clave CLAUDE_VERSION)"
    RUST_VERSION="$(clave RUST_VERSION)"

    paso runtime llamar_runtime
    paso paquetes instalar_paquetes
    paso versiones instalar_y_verificar_versiones
    paso builder crear_builder
    paso root-home armar_root_home
    paso red-builder correr_red_builder
}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then
    principal "$@"
fi
