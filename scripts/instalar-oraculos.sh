#!/usr/bin/env bash
# Instala los oráculos de memora y brasa en $ROOT_HOME/oraculos/<tarea>.
# Uso: scripts/instalar-oraculos.sh
# ROOT_HOME por defecto: ~/.local/share/root. Nunca escribe dentro del repo de Root.
set -euo pipefail

AQUI=$(cd "$(dirname "$0")" && pwd)
REPO=$(git -C "$AQUI" rev-parse --show-toplevel)
BANCO=${BANCO:-$HOME/Proyectos/bench-ecc-sdlc}
ORACULOS_SRC=${ORACULOS_SRC:-$HOME/.claude/orchestration/.scratch/ecc-vs-sdlc/oraculo}
TAREAS="memora brasa"

error() {
    echo "ERROR: $*" >&2
    exit 1
}

# Destino absoluto, sin tocar el disco. Aborta si cae dentro del repo de Root.
DESTINO=$(realpath -m "${ROOT_HOME:-$HOME/.local/share/root}")
REPO_REAL=$(realpath "$REPO")
case "$DESTINO/" in
    "$REPO_REAL/"*) error "ROOT_HOME ($DESTINO) está dentro del repo de Root ($REPO_REAL)" ;;
esac
ORACULOS="$DESTINO/oraculos"

# Borra una carpeta de la tarea solo si está bajo $ORACULOS y se llama contrato, oculto o bootstrap.
limpiar() {
    local ruta=$1
    case "$ruta" in
        "$ORACULOS/"*/contrato | "$ORACULOS/"*/oculto | "$ORACULOS/"*/bootstrap) ;;
        *) error "ruta fuera de $ORACULOS: $ruta" ;;
    esac
    if [ -e "$ruta" ]; then
        rm -r "$ruta"
    fi
}

instalar_tarea() {
    local t=$1
    local src_b="$BANCO/bootstrap/$t"
    local src_o="$ORACULOS_SRC/$t"
    local dst="$ORACULOS/$t"

    # Bootstrap: copia del repo inicial sin .git ni target.
    limpiar "$dst/bootstrap"
    mkdir -p "$dst/bootstrap"
    tar -C "$src_b" --exclude=./.git --exclude=./target -cf - . | tar -C "$dst/bootstrap" -xf -

    # Contrato: test_m*.py pristinos del bootstrap; conftest.py del oraculo (usa ORACULO_REPO).
    limpiar "$dst/contrato"
    mkdir -p "$dst/contrato"
    cp -a "$src_b/tests/contract/"test_m*.py "$dst/contrato/"
    cp -a "$src_o/conftest.py" "$dst/contrato/conftest.py"

    # Oculto: solo el test oculto y su conftest.
    limpiar "$dst/oculto"
    mkdir -p "$dst/oculto"
    cp -a "$src_o/test_oculto_$t.py" "$src_o/conftest.py" "$dst/oculto/"

    cat >"$dst/tarea.toml" <<EOF
nombre = "$t"
lenguaje = "rust"
imagen = "localhost/root-oraculo-rust:1"
spec = "bootstrap/docs/SPEC.md"
[contrato]
cmd = "ORACULO_REPO=/repo CARGO_TARGET_DIR=/salida/target python3 -m pytest -q -p no:cacheprovider --junitxml=/salida/contrato.xml /oraculo/contrato"
[oculto]
cmd = "ORACULO_REPO=/repo CARGO_TARGET_DIR=/salida/target python3 -m pytest -q -p no:cacheprovider --junitxml=/salida/oculto.xml /oraculo/oculto"
EOF

    echo "OK    $t -> $dst"
}

# Revisa que existan las fuentes antes de escribir nada.
for t in $TAREAS; do
    [ -f "$BANCO/bootstrap/$t/docs/SPEC.md" ] || error "falta $BANCO/bootstrap/$t/docs/SPEC.md"
    [ -d "$BANCO/bootstrap/$t/tests/contract" ] || error "falta $BANCO/bootstrap/$t/tests/contract"
    [ -f "$ORACULOS_SRC/$t/test_oculto_$t.py" ] || error "falta $ORACULOS_SRC/$t/test_oculto_$t.py"
    [ -f "$ORACULOS_SRC/$t/conftest.py" ] || error "falta $ORACULOS_SRC/$t/conftest.py"
done

mkdir -p "$ORACULOS"
for t in $TAREAS; do
    instalar_tarea "$t"
done
