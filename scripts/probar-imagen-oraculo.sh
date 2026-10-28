#!/usr/bin/env bash
# Verifica la imagen root-oraculo-rust sin red: herramientas, GPU por lavapipe y builds offline.
# Uso: scripts/probar-imagen-oraculo.sh
set -euo pipefail

IMAGEN=${IMAGEN:-localhost/root-oraculo-rust:2}
BANCO=${BANCO:-$HOME/Proyectos/bench-ecc-sdlc}
LVP=/usr/share/vulkan/icd.d/lvp_icd.x86_64.json

TMP=$(mktemp -d)
trap 'rm -r "$TMP"' EXIT
FALLOS=0

# Corre un chequeo. Si falla, muestra las últimas líneas de su salida.
chequeo() {
    local nombre=$1
    shift
    local salida
    if salida=$("$@" 2>&1); then
        echo "OK    $nombre"
    else
        echo "FALLA $nombre"
        printf '%s\n' "$salida" | tail -n 25 | sed 's/^/      /'
        FALLOS=$((FALLOS + 1))
    fi
}

# Contenedor de prueba: sin red, sin conservar capacidades extra.
correr() {
    podman run --rm --pull=never --network none "$@"
}

# Copia un proyecto fuente a un directorio del host sin target/ ni .git.
copiar_repo() {
    local origen=$1 destino=$2
    mkdir -p "$destino"
    tar -C "$origen" --exclude=./target --exclude=./.git -cf - . | tar -C "$destino" -xf -
}

# Build offline de un proyecto. Si pasa un binario lo ejecuta con $3 como argumento.
build_offline() {
    local nombre=$1 origen=$2 bin=$3 ejecutar=${4:-}
    local dir="$TMP/$nombre"
    copiar_repo "$origen" "$dir/repo"
    # Rootless podman mapea --user a un subuid: el contenedor escribe aquí solo con 777.
    # El padre $TMP es 0700 (mktemp), así que otros usuarios no llegan a este directorio.
    mkdir -p "$dir/salida"
    chmod 777 "$dir/salida"
    # umask 000: los archivos del contenedor quedan con el subuid; así el host puede borrarlos.
    local orden="umask 000 && cargo build -q --bin $bin"
    if [ -n "$ejecutar" ]; then
        orden="$orden && /salida/target/debug/$ejecutar"
    fi
    correr \
        --user "$(id -u):$(id -g)" \
        --tmpfs /tmp \
        -e HOME=/tmp \
        -e CARGO_TARGET_DIR=/salida/target \
        -v "$dir/repo:/repo:ro,z" \
        -v "$dir/salida:/salida:z" \
        -w /repo \
        "$IMAGEN" sh -c "$orden"
}

# Chequeo que el binario de probe-gpu usa lavapipe (llvmpipe).
gpu_llvmpipe() {
    build_offline probe-gpu "$BANCO/probe-gpu" probe-gpu probe-gpu | grep llvmpipe >/dev/null
}

echo "Imagen: $IMAGEN"
echo "Banco:  $BANCO"

chequeo "(a) python3 -m pytest --version" correr "$IMAGEN" python3 -m pytest --version
chequeo "(b) cargo --version" correr "$IMAGEN" cargo --version
chequeo "(c) existe $LVP" correr "$IMAGEN" test -f "$LVP"
chequeo "(d) build offline bootstrap/memora" build_offline memora "$BANCO/bootstrap/memora" memora
chequeo "(d) build offline bootstrap/brasa" build_offline brasa "$BANCO/bootstrap/brasa" brasa
chequeo "(e) probe-gpu build offline y usa llvmpipe" gpu_llvmpipe
chequeo "(f) python3 importa redis 8.1.0" correr "$IMAGEN" python3 -c 'import redis; assert redis.__version__ == "8.1.0"'

# Herramientas de la tarea 7g: cada una responde a su comando de versión, sin red.
chequeo "(g) cargo clippy --version" correr "$IMAGEN" cargo clippy --version
chequeo "(g) cargo audit --version" correr "$IMAGEN" cargo audit --version
chequeo "(g) cargo mutants --version" correr "$IMAGEN" cargo mutants --version
chequeo "(g) gitleaks version" correr "$IMAGEN" gitleaks version
chequeo "(g) hadolint --version" correr "$IMAGEN" hadolint --version

if [ "$FALLOS" -gt 0 ]; then
    echo "$FALLOS chequeo(s) con FALLA"
    exit 1
fi
echo "Todos los chequeos OK"
