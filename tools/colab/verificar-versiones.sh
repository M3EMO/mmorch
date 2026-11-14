#!/usr/bin/env bash
# Verifica las versiones fijadas en <carpeta>/versiones.env.
# No instala nada: solo lee y compara. Corre todos los controles.
# Sale 0 solo si todos dan ok.
set -euo pipefail

ROOT_BIN_DIR="${ROOT_BIN_DIR:-/usr/local/bin}"
ROOT_ETC_DIR="${ROOT_ETC_DIR:-/etc/root-oraculo}"
ROOT_RAIZ_DIR="${ROOT_RAIZ_DIR:-}"

# Rust vive en /usr/local. Sin estas rutas, el proxy rustup falla.
# Copia de RUSTUP_HOME y CARGO_HOME en preparar-runtime.sh. Si cambia una, cambiar la otra.
export RUSTUP_HOME="${RUSTUP_HOME:-/usr/local/rustup}"
export CARGO_HOME="${CARGO_HOME:-/usr/local/cargo}"

# Carpetas que bwrap no puede crear. Son los destinos de argumentos_bwrap en src/oracle/contenedor.rs.
# Copia de CARPETAS_RUNTIME en preparar-runtime.sh. Si cambia una, cambiar la otra y contenedor.rs.
CARPETAS_RUNTIME=(/repo /salida /oraculo /cache /rustsec /trabajo)

# Herramientas con SHA-256 fijado en HERRAMIENTAS_SHA256.
NOMBRES_HERRAMIENTAS=(cargo-audit cargo-mutants cargo-nextest gitleaks hadolint)

# Texto exacto de nextest.toml.
# Lo escribe la última línea RUN de imagenes/oraculo-rust/Containerfile (el printf
# que escribe /etc/root-oraculo/nextest.toml). Si cambia uno, cambiar el otro.
NEXTEST_TOML='[store]
dir = "/salida/target/nextest"

[profile.ci]
fail-fast = false
slow-timeout = { period = "60s", terminate-after = 5 }

[profile.ci.junit]
path = "junit.xml"
'

# Cargo y clippy viven aquí. Va al final: un rustc del PATH gana.
PATH="$PATH:/usr/local/cargo/bin"

fallos=0
archivo=""

# Imprime un control ok. Recibe el nombre del control.
ok() {
    echo "ok $1"
}

# Imprime un control mal y cuenta el fallo.
mal_control() {
    printf 'mal %s: %s contra %s\n' "$1" "$2" "$3"
    fallos=$((fallos + 1))
}

# Muestra un texto en una sola línea. Los saltos pasan a \n.
una_linea() {
    printf '%s' "${1//$'\n'/\\n}"
}

# Lee una clave de versiones.env. Sale vacía si falta.
clave() {
    grep -m1 "^$1=" "$archivo" | cut -d= -f2- || true
}

# Lee el SHA-256 esperado de una herramienta. Sale vacío si falta.
sha_esperado() {
    local entrada
    IFS=',' read -ra entradas <<<"$HERRAMIENTAS_SHA256"
    for entrada in "${entradas[@]}"; do
        if [ "${entrada%%:*}" = "$1" ]; then
            echo "${entrada#*:}"
            return 0
        fi
    done
    return 0
}

# Control rustc: la segunda palabra de `rustc --version` contra RUST_VERSION.
control_rustc() {
    local hallado
    hallado="$(rustc --version 2>/dev/null | cut -d' ' -f2)" || true
    if [ "$hallado" = "$RUST_VERSION" ]; then
        ok rustc
    else
        mal_control rustc "$RUST_VERSION" "${hallado:-ausente}"
    fi
}

# Control clippy: `cargo clippy --version` responde y no sale vacío.
control_clippy() {
    local salida
    if salida="$(cargo clippy --version 2>&1)" && [ -n "$salida" ]; then
        ok clippy
    else
        mal_control clippy "clippy que responda" "sin respuesta"
    fi
}

# Control de una herramienta: SHA-256 del binario contra HERRAMIENTAS_SHA256.
control_sha() {
    local nombre="$1" esperado hallado
    esperado="$(sha_esperado "$nombre")"
    if [ ! -f "$ROOT_BIN_DIR/$nombre" ]; then
        mal_control "$nombre" "$esperado" "ausente"
        return 0
    fi
    hallado="$(sha256sum "$ROOT_BIN_DIR/$nombre" 2>/dev/null | cut -d' ' -f1)" || true
    if [ "$hallado" = "$esperado" ]; then
        ok "$nombre"
    else
        mal_control "$nombre" "$esperado" "${hallado:-ilegible}"
    fi
}

# Control nextest.toml: el contenido exacto contra NEXTEST_TOML.
control_nextest() {
    local ruta="$ROOT_ETC_DIR/nextest.toml" hallado
    if [ ! -f "$ruta" ]; then
        mal_control nextest.toml "$(una_linea "$NEXTEST_TOML")" "ausente"
        return 0
    fi
    # La x final evita que $(...) quite los saltos de línea del final.
    hallado="$(cat "$ruta" 2>/dev/null; printf x)" || true
    hallado="${hallado%x}"
    if [ "$hallado" = "$NEXTEST_TOML" ]; then
        ok nextest.toml
    else
        mal_control nextest.toml "$(una_linea "$NEXTEST_TOML")" "$(una_linea "$hallado")"
    fi
}

# Control de carpetas: cada una de CARPETAS_RUNTIME existe bajo ROOT_RAIZ_DIR.
control_carpetas() {
    local ruta
    for ruta in "${CARPETAS_RUNTIME[@]}"; do
        if [ -d "${ROOT_RAIZ_DIR:-}$ruta" ]; then
            ok "$ruta"
        else
            mal_control "$ruta" "carpeta existente" "ausente"
        fi
    done
}

# Control bwrap: responde a `bwrap --version`. Muestra la versión.
control_bwrap() {
    local salida
    if salida="$(bwrap --version 2>&1)" && [ -n "$salida" ]; then
        ok "bwrap $(printf '%s' "$salida" | head -n1 | cut -d' ' -f2)"
    else
        mal_control bwrap "bwrap que responda" "sin respuesta"
    fi
}

carpeta="${1:-}"
if [ -z "$carpeta" ] || [ ! -d "$carpeta" ]; then
    echo "mal versiones.env: falta la carpeta ${carpeta:-(sin argumento)}"
    exit 1
fi

archivo="$carpeta/versiones.env"
if [ ! -f "$archivo" ]; then
    echo "mal versiones.env: falta $archivo"
    exit 1
fi

for clave_requerida in RUST_VERSION RUSTUP_SHA256 HERRAMIENTAS_SHA256; do
    if [ -z "$(clave "$clave_requerida")" ]; then
        echo "mal versiones.env: falta la clave $clave_requerida"
        exit 1
    fi
done

RUST_VERSION="$(clave RUST_VERSION)"
HERRAMIENTAS_SHA256="$(clave HERRAMIENTAS_SHA256)"

for nombre in "${NOMBRES_HERRAMIENTAS[@]}"; do
    if [ -z "$(sha_esperado "$nombre")" ]; then
        echo "mal versiones.env: falta $nombre en HERRAMIENTAS_SHA256"
        exit 1
    fi
done

control_rustc
control_clippy
for nombre in "${NOMBRES_HERRAMIENTAS[@]}"; do
    control_sha "$nombre"
done
control_nextest
control_bwrap
control_carpetas

if [ "$fallos" -eq 0 ]; then
    exit 0
fi
exit 1
