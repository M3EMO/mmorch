"""Organizador de arquitectura — chequeos MECANICOS, sin juicio de LLM.

Motivo (2026-08-19): 3 módulos (auto_repair/merge_train/project_repair)
reinventaron el mismo bug por separado — eso tiene nombre: violación del
Common Closure Principle (Martin, "Agile Software Development", cap. 20 —
módulos que deberían cambiar juntos, no cambiaron juntos). Y el flakiness de
la suite completa (test distinto fallando cada corrida, verde en aislamiento)
tiene otro nombre: contaminación por estado compartido entre tests — la
firma es "pasa solo, falla distinto en conjunto" (Google, "Software
Engineering at Google", cap. 11, "hermeticity").

Ninguno de los dos necesita un LLM para detectarse. Son grafo + conteo +
grep. La parte que SÍ necesita juicio (dónde trazar un paquete nuevo, qué va
en el kernel compartido) se queda en self_audit.py, con el LLM como juez.
Este módulo solo SEÑALA candidatos — nunca reorganiza nada solo.
"""

from __future__ import annotations

import ast
import re
import subprocess
from pathlib import Path

_SKIP = {"__init__.py"}
_GOD_LOC = 800          # por encima: candidato a revisar (no veredicto)
_GOD_FANIN_FRAC = 0.15  # importado por >15% del paquete = hub de facto


def import_graph(root: Path) -> dict[str, set[str]]:
    """`from mmorch.X import Y` en CUALQUIER profundidad (top-level o dentro
    de una funcion — el 95% de los imports de este repo son locales, un scan
    solo-top-level los pierde casi todos)."""
    edges: dict[str, set[str]] = {}
    for f in sorted((root / "mmorch").glob("*.py")):
        if f.name in _SKIP:
            continue
        mod = f.stem
        try:
            tree = ast.parse(f.read_text(encoding="utf-8"))
        except SyntaxError:
            continue
        for node in ast.walk(tree):
            if (isinstance(node, ast.ImportFrom) and node.module
                    and node.module.startswith("mmorch.")):
                target = node.module.split(".")[1]
                if target != mod:
                    edges.setdefault(mod, set()).add(target)
    return edges


def find_cycles(edges: dict[str, set[str]]) -> list[tuple[str, str]]:
    """Ciclos A<->B directos. Un ciclo es un defecto sin ambiguedad (Martin,
    Stable Dependencies Principle) — cero subjetividad, se reporta siempre."""
    pares = {(a, b) if a < b else (b, a) for a in edges
             for b in edges.get(a, ()) if a in edges.get(b, ())}
    return sorted(pares)


def god_module_candidates(root: Path, edges: dict[str, set[str]]) -> list[dict]:
    """LOC alto o fan-in alto — CANDIDATOS, no veredicto: un modulo grande
    con interfaz chica es 'profundo' (bueno, tus propios principios); un
    modulo grande con interfaz ancha es un dios (malo). Distinguir eso
    necesita leer el archivo — se lo paso a self_audit, no lo decido aca."""
    fan_in: dict[str, int] = {}
    for _, targets in edges.items():
        for t in targets:
            fan_in[t] = fan_in.get(t, 0) + 1
    n_mods = len(list((root / "mmorch").glob("*.py")))
    out = []
    for f in sorted((root / "mmorch").glob("*.py")):
        if f.name in _SKIP:
            continue
        mod = f.stem
        loc = len(f.read_text(encoding="utf-8").splitlines())
        fi = fan_in.get(mod, 0)
        razon = []
        if loc > _GOD_LOC:
            razon.append(f"{loc} lineas")
        if n_mods and fi / n_mods > _GOD_FANIN_FRAC:
            razon.append(f"importado por {fi} modulos")
        if razon:
            out.append({"module": f"mmorch/{f.name}", "loc": loc,
                        "fan_in": fi, "razon": ", ".join(razon)})
    return sorted(out, key=lambda d: -d["loc"])


_SWEEP_MAX_FILES = 8  # commits que tocan mas de esto son barrido (lint/type
                      # gate), no acoplamiento real — se excluyen del conteo


def co_change_pairs(root: Path, *, since_days: int = 90, min_cochanges: int = 3,
                    min_ratio: float = 0.5) -> list[dict]:
    """Pares de archivos que cambian JUNTOS seguido sin estar conectados por
    import — la firma retroactiva de una violacion CCP (Tornhill, "Your Code
    as a Crime Scene"): si 2 modulos casi siempre se tocan en el mismo commit
    pero uno no importa al otro, probablemente deberian ser un solo modulo o
    depender de un tercero compartido — exactamente lo que paso hoy con
    auto_repair/merge_train/project_repair."""
    try:
        out = subprocess.run(
            ["git", "log", f"--since={since_days} days ago", "--name-only",
             "--pretty=format:@@%H", "--", "mmorch/"],
            cwd=root, capture_output=True, text=True, timeout=30)
    except (OSError, subprocess.TimeoutExpired):
        return []
    if out.returncode != 0:
        return []

    commits: list[set[str]] = []
    actual: set[str] = set()
    for line in out.stdout.splitlines():
        if line.startswith("@@"):
            if actual:
                commits.append(actual)
            actual = set()
        elif line.strip().startswith("mmorch/") and line.strip().endswith(".py"):
            name = Path(line.strip()).name
            if name not in _SKIP:
                actual.add(name)
    if actual:
        commits.append(actual)
    commits = [c for c in commits if len(c) <= _SWEEP_MAX_FILES]

    from collections import Counter
    solo: "Counter[str]" = Counter()
    junto: "Counter[tuple[str, str]]" = Counter()
    for files in commits:
        for f in files:
            solo[f] += 1
        for a in files:
            for b in files:
                if a < b:
                    junto[(a, b)] += 1

    edges = import_graph(root)
    conectados = {(a, b) for a, ts in edges.items() for b in ts} | \
                 {(b, a) for a, ts in edges.items() for b in ts}

    resultados = []
    for (a, b), n in junto.items():
        if n < min_cochanges:
            continue
        base = min(solo[a], solo[b])
        ratio = n / base if base else 0
        if ratio < min_ratio:
            continue
        a_mod, b_mod = a[:-3], b[:-3]
        if (a_mod, b_mod) in conectados or (b_mod, a_mod) in conectados:
            continue  # ya conectados por import — el acoplamiento es explicito
        resultados.append({"archivos": [a, b], "co_cambios": n,
                           "ratio": round(ratio, 2)})
    return sorted(resultados, key=lambda d: -d["ratio"])


def _mutates(name: str, text: str) -> bool:
    """`NAME[...] = ` (asignacion, NO comparacion `==`) o `NAME.append(` etc,
    en cualquier linea DISTINTA de la definicion del global."""
    esc = re.escape(name)
    subscript = re.compile(rf"\b{esc}\s*\[[^\]]*\]\s*=(?!=)")
    method = re.compile(rf"\b{esc}\.(append|pop|clear|update|extend|remove|"
                        r"insert|sort|reverse)\(")
    for line in text.splitlines():
        if line.lstrip().startswith(f"{name} ="):
            continue  # la definicion del global no cuenta como mutacion
        if subscript.search(line) or method.search(line):
            return True
    return False


def pollution_candidates(tests_dir: Path) -> list[dict]:
    """Señales estaticas de contaminacion entre tests (Google SWE Book cap.
    11, hermeticity): constante MAYUSCULA a nivel de modulo que se MUTA
    (no solo se lee) en algun test del mismo archivo, o tempfile.mkdtemp()
    sin pasar por el fixture tmp_path. Ninguna corrida necesaria."""
    out = []
    for f in sorted(tests_dir.glob("test_*.py")):
        try:
            text = f.read_text(encoding="utf-8")
        except OSError:
            continue
        if "tempfile.mkdtemp(" in text and "tmp_path" not in text:
            out.append({"file": f.name, "señal": "tempfile.mkdtemp sin tmp_path"})
        globals_ = set(re.findall(r"^([A-Z_][A-Z0-9_]*)\s*=\s*[\{\[]", text, re.M))
        for g in globals_:
            if _mutates(g, text):
                out.append({"file": f.name, "señal": f"'{g}' es global Y se muta"})
    return out


def scan(orch_root: str) -> dict:
    """Corrida completa: los 4 chequeos deterministas, sin LLM. Salida
    directa (no pasa por candidatas — esto es lectura de reflect()/digest,
    self_audit sigue siendo el unico que propone cambios de codigo)."""
    root = Path(orch_root)
    edges = import_graph(root)
    return {
        "ciclos": find_cycles(edges),
        "god_module_candidates": god_module_candidates(root, edges),
        "co_change_sin_import": co_change_pairs(root),
        "test_pollution_candidates": pollution_candidates(root / "tests"),
    }
