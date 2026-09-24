"""Dueño unico de metrics.jsonl: solo mmorch/metrics.py nombra el archivo en codigo
(no en comentarios ni docstrings). Todo otro lector pasa por metrics.read_events.
Medido en el banco de acople 2026-09-24 (vault/research/banco-de-acople-*)."""
import ast
from pathlib import Path

PKG = Path(__file__).resolve().parents[1] / "mmorch"


def _docstrings(tree):
    for node in ast.walk(tree):
        if isinstance(node, (ast.Module, ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
            body = node.body
            if body and isinstance(body[0], ast.Expr) and isinstance(body[0].value, ast.Constant):
                yield id(body[0].value)


def test_metrics_jsonl_tiene_un_solo_dueno():
    malos = []
    for f in sorted(PKG.glob("*.py")):
        if f.name == "metrics.py":
            continue
        tree = ast.parse(f.read_text(encoding="utf-8"))
        docs = set(_docstrings(tree))
        for node in ast.walk(tree):
            if (isinstance(node, ast.Constant) and isinstance(node.value, str)
                    and "metrics.jsonl" in node.value and id(node) not in docs):
                malos.append(f"{f.name}:{node.lineno}")
    assert not malos, f"leen metrics.jsonl por fuera de metrics.py: {malos}"
