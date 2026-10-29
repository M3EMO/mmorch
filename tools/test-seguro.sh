#!/usr/bin/env bash
# Corre `cargo test` sin claves, con un ROOT_HOME de prueba nuevo y con temporales y target en $HOME.
# Los argumentos pasan a `cargo test`. Sale con el código de cargo.
set -euo pipefail

# Sin claves: con ellas, la suite con podman gasta API real.
unset DEEPSEEK_API_KEY ANTHROPIC_API_KEY

base="${ROOT_TESTS_BASE:-$HOME/.cache/root-tests}"
mkdir -p "$base"

# ROOT_HOME nuevo y vacío para esta corrida. Se borra al salir.
ROOT_HOME="$(mktemp -d "$base/home.XXXXXX")"
export ROOT_HOME
trap 'rm -rf -- "$ROOT_HOME"' EXIT

# Temporales y target fuera de /tmp. CARGO_TARGET_DIR_SEGURO gana sobre el CARGO_TARGET_DIR del usuario.
mkdir -p "$base/tmp"
export TMPDIR="$base/tmp"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR_SEGURO:-$base/target}"
mkdir -p "$CARGO_TARGET_DIR"

# ROOT_SIN_PODMAN no se toca: si viene exportada, llega a cargo tal cual.
"${ROOT_TEST_SEGURO_CARGO:-cargo}" test "$@"
