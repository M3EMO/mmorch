#!/usr/bin/env bash
# Construye la imagen root-oraculo-rust:2 con los manifiestos de los proyectos del banco.
# Usa la red una vez (dependencias y herramientas). Evaluar después corre sin red.
# Uso: scripts/construir-imagen-oraculo.sh
set -euo pipefail

RAIZ=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
BANCO=${BANCO:-$HOME/Proyectos/bench-ecc-sdlc}
IMAGEN=${IMAGEN:-root-oraculo-rust:2}
PROYECTOS=(
    bootstrap/memora
    bootstrap/brasa
    memora-ecc
    memora-sdlc
    brasa-ecc
    brasa-sdlc
    probe-gpu
)

CTX=$(mktemp -d)
trap 'rm -r "$CTX"' EXIT

cp "$RAIZ/imagenes/oraculo-rust/Containerfile" "$CTX/Containerfile"

# Solo manifiestos: el código y target/ no entran en el contexto.
for p in "${PROYECTOS[@]}"; do
    mkdir -p "$CTX/manifiestos/$p"
    cp "$BANCO/$p/Cargo.toml" "$BANCO/$p/Cargo.lock" "$CTX/manifiestos/$p/"
done

podman build -t "$IMAGEN" -f "$CTX/Containerfile" "$CTX"
echo "Imagen construida: $IMAGEN"
