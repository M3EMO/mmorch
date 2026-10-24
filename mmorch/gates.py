"""Chequeos de los gates del SDLC (bd orchestration-bpn, 2026-10-23).

Un gate tiene tres partes. El chequeo decide verde o rojo y vive aca. La medicion (precision del chequeo sobre casos
etiquetados) vive en `mmorch/gates_bench.py`. La orquestacion (cuando corre el gate y que hace el pipeline con un rojo)
queda en `sdlc/pipeline.py`, que llama a estas funciones y registra el resultado con `rec_gate`.

Contrato: cada chequeo es una funcion pura con datos explicitos y devuelve `(ok, detalle)`, el contrato de `rec_gate`.
Este modulo importa solo stdlib y `mmorch.checkers`; nunca importa `sdlc` (R6 de tests/test_capas.py).
Decisiones: brainstorms/2026-10-23-gates-a-mmorch.md.
"""
from __future__ import annotations


def sin_clones(written: dict[str, str], umbral: float = 0.8) -> tuple[bool, str]:
    """D6: el coder escribio el contenido de plugins.py dentro de plugin_worker.py. Dos archivos del plan
    con > 80% de lineas identicas es un clon, no una feature. Determinista, USD 0."""
    files = list(written)
    for i, a in enumerate(files):
        la = {l.strip() for l in written[a].splitlines() if l.strip()}
        for b in files[i + 1:]:
            lb = {l.strip() for l in written[b].splitlines() if l.strip()}
            if la and lb and len(la & lb) / min(len(la), len(lb)) > umbral:
                return False, f"{a} y {b} comparten > {umbral:.0%} de lineas"
    return True, "ok"
