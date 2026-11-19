#!/usr/bin/env bash
# Prepara el runtime de Colab para el oráculo Rust. Corre como root en Ubuntu 24.04.
# Instala bubblewrap, Rust fijado, las herramientas del paquete y nextest.toml.
# Crea las carpetas del runtime y termina llamando a verificar-versiones.sh.
# Uso: preparar-runtime.sh <carpeta_del_paquete>
set -euo pipefail

ROOT_BIN_DIR="${ROOT_BIN_DIR:-/usr/local/bin}"
ROOT_ETC_DIR="${ROOT_ETC_DIR:-/etc/root-oraculo}"
ROOT_RAIZ_DIR="${ROOT_RAIZ_DIR:-}"
export ROOT_BIN_DIR ROOT_ETC_DIR ROOT_RAIZ_DIR

# Carpetas que bwrap no puede crear. Son los destinos de argumentos_bwrap en src/oracle/contenedor.rs.
# Copia de CARPETAS_RUNTIME en verificar-versiones.sh. Si cambia una, cambiar la otra y contenedor.rs.
CARPETAS_RUNTIME=(/repo /salida /oraculo /cache /rustsec /trabajo)

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

# Paso 1: instala bubblewrap. Si bwrap ya existe, no llama a apt-get.
# El par update + install se reintenta entero hasta INTENTOS_APT veces.
# ROOT_APT_ESPERA son los segundos entre intentos (30 por omisión). Los tests usan 0.
INTENTOS_APT=3
APT_OPCIONES=(-o Acquire::Retries=5 -o Acquire::http::Timeout=30)

instalar_bubblewrap() {
    if command -v bwrap >/dev/null 2>&1; then
        echo "bubblewrap ya está instalado: no se llama a apt-get"
        return 0
    fi
    local intento codigo=0
    for ((intento = 1; intento <= INTENTOS_APT; intento++)); do
        codigo=0
        { apt-get "${APT_OPCIONES[@]}" update -qq &&
            apt-get "${APT_OPCIONES[@]}" install -y bubblewrap; } || codigo=$?
        if [ "$codigo" -eq 0 ]; then
            return 0
        fi
        if [ "$intento" -lt "$INTENTOS_APT" ]; then
            echo "intento $intento de apt-get falló (código $codigo): se reintenta en ${ROOT_APT_ESPERA:-30} s"
            sleep "${ROOT_APT_ESPERA:-30}"
        fi
    done
    echo "mal bubblewrap: apt-get falló después de $INTENTOS_APT intentos (código $codigo)"
    exit "$codigo"
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

# Paso 5: crea las carpetas del runtime bajo ROOT_RAIZ_DIR. bwrap no puede crearlas al montar.
crear_carpetas() {
    local ruta
    for ruta in "${CARPETAS_RUNTIME[@]}"; do
        mkdir -p "${ROOT_RAIZ_DIR:-}$ruta"
    done
}

# Paso 6: corre el control de versiones. Si falla, el script falla.
llamar_verificar() {
    if ! bash "$(dirname "$0")/verificar-versiones.sh" "$carpeta"; then
        echo "mal verificar-versiones.sh: el runtime no cumple las versiones fijadas"
        exit 1
    fi
}

# Cuerpo principal. Corre solo si el script se ejecuta directo. Con `source` solo se definen las funciones.
principal() {
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
    paso carpetas crear_carpetas
    paso verificar llamar_verificar
}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then
    principal "$@"
fi
