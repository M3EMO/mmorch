#!/usr/bin/env bash
# Prueba scripts/instalar-oraculos.sh con un ROOT_HOME temporal.
# Uso: scripts/probar-oraculos.sh
set -euo pipefail

AQUI=$(cd "$(dirname "$0")" && pwd)
REPO=$(git -C "$AQUI" rev-parse --show-toplevel)
INSTALADOR="$AQUI/instalar-oraculos.sh"
BANCO=${BANCO:-$HOME/Proyectos/bench-ecc-sdlc}
ORACULOS_SRC=${ORACULOS_SRC:-$HOME/.claude/orchestration/.scratch/ecc-vs-sdlc/oraculo}
TAREAS="memora brasa"

TMP=$(mktemp -d)
trap 'rm -r "$TMP"' EXIT
export ROOT_HOME="$TMP/root"
FALLOS=0

# Corre un chequeo y muestra OK o FALLA. No corta el resto.
chequeo() {
    local nombre=$1
    shift
    if "$@" >/dev/null 2>&1; then
        echo "OK    $nombre"
    else
        echo "FALLA $nombre"
        FALLOS=$((FALLOS + 1))
    fi
}

existe_glob() {
    compgen -G "$1" >/dev/null
}

# Cuenta los test_m*.py de contrato. Debe haber exactamente 8.
contrato_tiene_8() {
    local n=0 f
    for f in "$1"/contrato/test_m*.py; do
        [ -e "$f" ] && n=$((n + 1))
    done
    [ "$n" -eq 8 ]
}

# Ningún cambio en un directorio "oraculos" dentro del repo de Root.
sin_rastro_en_repo() {
    [ -z "$(git -C "$REPO" status --porcelain | grep -E '(^|[ /])oraculos(/|$)' || true)" ]
}

# Hashes de todo lo instalado. Sirve para comparar dos corridas.
manifiesto() {
    (cd "$ROOT_HOME/oraculos" && find . -type f -exec sha256sum {} + | sort)
}

chequeo "instalador existe y es ejecutable" test -x "$INSTALADOR"

chequeo "instalador corre (1a vez)" "$INSTALADOR"
manifiesto >"$TMP/m1" 2>/dev/null || true

chequeo "instalador corre (2a vez)" "$INSTALADOR"
manifiesto >"$TMP/m2" 2>/dev/null || true

chequeo "dos corridas dan el mismo resultado" bash -c "[ -s '$TMP/m1' ] && cmp -s '$TMP/m1' '$TMP/m2'"

for t in $TAREAS; do
    D="$ROOT_HOME/oraculos/$t"
    chequeo "$t: tarea.toml existe" test -f "$D/tarea.toml"
    chequeo "$t: tarea.toml parsea como TOML" \
        python3 -c 'import tomllib,sys; tomllib.load(open(sys.argv[1],"rb"))' "$D/tarea.toml"
    chequeo "$t: bootstrap/docs/SPEC.md existe" test -f "$D/bootstrap/docs/SPEC.md"
    chequeo "$t: contrato/conftest.py existe" test -f "$D/contrato/conftest.py"
    chequeo "$t: contrato/test_m1_*.py existe" existe_glob "$D/contrato/test_m1_*.py"
    chequeo "$t: contrato tiene 8 test_m*.py" contrato_tiene_8 "$D"
    chequeo "$t: contrato/conftest.py es el de oraculo (ORACULO_REPO)" \
        cmp -s "$ORACULOS_SRC/$t/conftest.py" "$D/contrato/conftest.py"
    chequeo "$t: contrato test_m*.py son copia pristina del bootstrap" \
        bash -c "for f in '$BANCO/bootstrap/$t/tests/contract/'test_m*.py; do cmp -s \"\$f\" '$D/contrato/'\$(basename \"\$f\") || exit 1; done"
    chequeo "$t: oculto/test_oculto_$t.py existe" test -f "$D/oculto/test_oculto_$t.py"
    chequeo "$t: oculto/conftest.py existe" test -f "$D/oculto/conftest.py"
    chequeo "$t: bootstrap sin .git ni target" \
        bash -c "[ ! -e '$D/bootstrap/.git' ] && [ ! -e '$D/bootstrap/target' ]"
done

chequeo "nada escrito en el repo de Root (oraculos)" sin_rastro_en_repo

echo
if [ "$FALLOS" -eq 0 ]; then
    echo "Todos los chequeos OK."
else
    echo "$FALLOS chequeo(s) FALLARON."
fi
[ "$FALLOS" -eq 0 ]
