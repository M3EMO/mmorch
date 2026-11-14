#!/usr/bin/env bash
# Prepara el runtime de Colab para el oráculo Rust. Corre como root en Ubuntu 24.04.
# Instala bubblewrap, Rust fijado, las herramientas del paquete y nextest.toml.
# Termina llamando a verificar-versiones.sh. Uso: preparar-runtime.sh <carpeta_del_paquete>
set -euo pipefail

ROOT_BIN_DIR="${ROOT_BIN_DIR:-/usr/local/bin}"
ROOT_ETC_DIR="${ROOT_ETC_DIR:-/etc/root-oraculo}"
export ROOT_BIN_DIR ROOT_ETC_DIR

# Base de las descargas de rustup-init para x86_64. Copia de BASE_RUSTUP en src/oracle/colab.rs.
RUSTUP_BASE="https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu"

# Rust vive en /usr/local. Cargo y rustup quedan en PATH durante todo el script.
export RUSTUP_HOME=/usr/local/rustup
export CARGO_HOME=/usr/local/cargo
PATH="/usr/local/cargo/bin:$PATH"

# Texto exacto de nextest.toml. Lo escribe la última línea RUN de imagenes/oraculo-rust/Containerfile.
# Copia byte a byte de NEXTEST_TOML en verificar-versiones.sh. Si cambia uno, cambiar el otro.
NEXTEST_TOML='[store]
dir = "/salida/target/nextest"

[profile.ci]
fail-fast = false
slow-timeout = { period = "60s", terminate-after = 5 }

[profile.ci.junit]
path = "junit.xml"
'

# Carpeta temporal de la descarga de rustup-init. El trap la borra al salir.
TMP_RUST=""

# Imprime el nombre del paso, corre el comando y muestra su tiempo.
paso() {
    local nombre="$1"
    shift
    echo "paso $nombre"
    SECONDS=0
    "$@"
    echo "tiempo $nombre: ${SECONDS}s"
}

# Lee una clave de versiones.env. Sale vacía si falta.
clave() {
    grep -m1 "^$1=" "$archivo" | cut -d= -f2- || true
}

# Paso 1: instala bubblewrap.
instalar_bubblewrap() {
    apt-get update -qq
    apt-get install -y bubblewrap
}

# Paso 2: baja rustup-init, verifica su SHA-256 y instala Rust con clippy.
instalar_rust() {
    TMP_RUST="$(mktemp -d)"
    trap 'rm -rf "$TMP_RUST"' EXIT
    local binario="$TMP_RUST/rustup-init" hallado
    curl -fsSL -o "$binario" "$RUSTUP_BASE/rustup-init"
    hallado="$(sha256sum "$binario" | cut -d' ' -f1)"
    if [ "$hallado" != "$RUSTUP_SHA256" ]; then
        rm -f "$binario"
        echo "mal rustup-init: SHA-256 $hallado distinto de $RUSTUP_SHA256"
        exit 1
    fi
    chmod 0755 "$binario"
    "$binario" -y --no-modify-path --default-toolchain "$RUST_VERSION" --profile minimal
    rustup component add clippy
}

# Paso 3: copia las herramientas del paquete a ROOT_BIN_DIR.
instalar_herramientas() {
    local origen="$carpeta/herramientas"
    if [ ! -d "$origen" ]; then
        echo "mal herramientas: falta la carpeta $origen"
        exit 1
    fi
    find "$origen" -mindepth 1 -maxdepth 1 -type f -exec install -m 0755 {} "$ROOT_BIN_DIR/" \;
}

# Paso 4: escribe nextest.toml con el texto exacto de NEXTEST_TOML.
escribir_nextest() {
    mkdir -p "$ROOT_ETC_DIR"
    printf '%s' "$NEXTEST_TOML" >"$ROOT_ETC_DIR/nextest.toml"
}

# Paso 5: corre el control de versiones. Si falla, el script falla.
llamar_verificar() {
    if ! bash "$(dirname "$0")/verificar-versiones.sh" "$carpeta"; then
        echo "mal verificar-versiones.sh: el runtime no cumple las versiones fijadas"
        exit 1
    fi
}

carpeta="${1:-}"
if [ -z "$carpeta" ]; then
    echo "mal preparar-runtime.sh: falta la carpeta del paquete"
    exit 1
fi

archivo="$carpeta/versiones.env"
if [ ! -f "$archivo" ]; then
    echo "mal preparar-runtime.sh: falta $archivo"
    exit 1
fi

for clave_requerida in RUST_VERSION RUSTUP_SHA256 HERRAMIENTAS_SHA256; do
    if [ -z "$(clave "$clave_requerida")" ]; then
        echo "mal versiones.env: falta la clave $clave_requerida"
        exit 1
    fi
done

RUST_VERSION="$(clave RUST_VERSION)"
RUSTUP_SHA256="$(clave RUSTUP_SHA256)"

paso bubblewrap instalar_bubblewrap
paso rust instalar_rust
paso herramientas instalar_herramientas
paso nextest escribir_nextest
paso verificar llamar_verificar
