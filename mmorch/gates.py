"""Chequeos de los gates del SDLC (bd orchestration-bpn, 2026-10-23).

Un gate tiene tres partes. El chequeo decide verde o rojo y vive aca. La medicion (precision del chequeo sobre casos
etiquetados) vive en `mmorch/gates_bench.py`. La orquestacion (cuando corre el gate y que hace el pipeline con un rojo)
queda en `sdlc/pipeline.py`, que llama a estas funciones y registra el resultado con `rec_gate`.

Contrato: cada chequeo es una funcion pura con datos explicitos y devuelve `(ok, detalle)`, el contrato de `rec_gate`.
Este modulo importa solo stdlib y `mmorch.checkers`; nunca importa `sdlc` (R6 de tests/test_capas.py).
Decisiones: brainstorms/2026-10-23-gates-a-mmorch.md.
"""
from __future__ import annotations

import re


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


def clarificaciones(spec_md: str) -> tuple[bool, str]:
    """Salida de la revision de spec (ticket 11, de clarify.md): seccion con 0..5 preguntas respondidas."""
    m = re.search(r"## Clarificaciones\b(.*?)(?:\n## |\Z)", spec_md, re.S)
    if not m:
        return False, "falta ## Clarificaciones"
    qs = re.findall(r"(?m)^- P\d+: .+\| R: \S.*\| afecta: \S+", m.group(1))  # D11-diag: "afecta: Contrato" tambien vale
    if not qs and re.search(r"sin preguntas|0 preguntas|ninguna pregunta|no hay preguntas", m.group(1), re.I):
        return True, "0 preguntas"  # D11-diag: Claude escribio "0 preguntas" en vez del literal
    if not 1 <= len(qs) <= 5:
        return False, f"{len(qs)} preguntas validas (esperado 1..5)"
    return True, f"{len(qs)} preguntas respondidas"


def filas_tabla(block: str) -> list[str]:
    """Filas de una tabla markdown (sin los `|` de los bordes)."""
    return re.findall(r"(?m)^\s*\|(.*)\|\s*$", block)


def traza_plan(spec_md: str, plan_block: str, files: list[str]) -> tuple[bool, str]:
    """Trazabilidad lado plan: cada R<n> de la spec aparece en el plan; cada archivo cita >= 1 R<n>. Un item es una
    linea de lista o una fila de tabla; un rango `R2–R10` cita R2..R10 (SSB F7a)."""
    ids = set(re.findall(r"\bR\d+\b", spec_md))
    plan_block = re.sub(r"\bR(\d+)\s*[–-]\s*R(\d+)\b",
                        lambda m: ", ".join(f"R{i}" for i in range(int(m[1]), int(m[2]) + 1)), plan_block)
    huerfanos = sorted(i for i in ids if not re.search(rf"\b{i}\b", plan_block))
    items = re.findall(r"(?m)^\s*[-*]\s*(.*)$", plan_block) + filas_tabla(plan_block)

    def nombra(it: str, f: str) -> bool:  # la ruta, o su nombre suelto si el plan lo escribio sin carpeta (F4b)
        return f in it or bool(re.search(r"(?<![\w/.-])" + re.escape(f.rsplit("/", 1)[-1]) + r"(?![\w.-])", it))
    sin_id = [f for f in files if not any(nombra(it, f) and re.search(r"\bR\d+\b", it) for it in items)]
    if huerfanos or sin_id:
        return False, f"IDs sin unidad: {huerfanos}; archivos sin ID: {sin_id}"
    return True, "ok"


def traza_spec(spec_md: str, tests: set[str], hay_aceptacion: bool) -> tuple[bool, str]:
    """Trazabilidad lado spec (ticket 02): IDs R<n> y tabla que cita tests existentes por nombre. Sin tests nombrados
    ni archivos de aceptacion, el oraculo es accept_cmd y alcanzan los IDs."""
    ids = set(re.findall(r"\bR\d+\b", spec_md))
    if not ids:
        return False, "sin IDs R<n>"
    if not tests and not hay_aceptacion:
        return True, f"{len(ids)} IDs; sin tests nombrados (accept_cmd es el oraculo)"
    m = re.search(r"## Trazabilidad\b(.*?)(?:\n## |\Z)", spec_md, re.S)
    rows = re.findall(r"(?m)^\|\s*(R\d+)\s*\|([^|]*)\|", m.group(1) if m else "")
    cov = {rid: set(re.findall(r"test_\w+", cell)) for rid, cell in rows}
    sin_test = sorted(i for i in ids if not cov.get(i))
    inexist = sorted(t for ts in cov.values() for t in ts if t not in tests)
    if sin_test or inexist:
        return False, f"sin test: {sin_test}; tests inexistentes: {inexist}"
    return True, f"{len(ids)} IDs cubiertos"
