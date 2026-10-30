#!/usr/bin/env bash
# Construye la imagen root-builder:1 con Claude Code en la versión fija que pasa la laptop.
# Uso: CLAUDE_CODE_VERSION=<versión exacta> scripts/construir-imagen-builder.sh
set -euo pipefail

RAIZ=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
IMAGEN=${IMAGEN:-root-builder:1}
: "${CLAUDE_CODE_VERSION:?falta CLAUDE_CODE_VERSION en el entorno}"

CTX=$(mktemp -d)
trap 'rm -r "$CTX"' EXIT

# Solo el Containerfile: la imagen no copia nada del repo ni de $HOME.
cp "$RAIZ/imagenes/builder/Containerfile" "$CTX/Containerfile"

podman build \
    --build-arg "CLAUDE_CODE_VERSION=$CLAUDE_CODE_VERSION" \
    -t "$IMAGEN" -f "$CTX/Containerfile" "$CTX"
echo "Imagen construida: $IMAGEN"
