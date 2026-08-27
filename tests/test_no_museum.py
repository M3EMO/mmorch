"""W4.3 — gate anti-museo: ninguna función pública de mmorch/ sin caller vivo.

"Museo" = implementada (y hasta testeada) pero que NADIE ejecuta en el camino real —
la clase de deuda que la auditoría 00-canonical-matrix §4 encontró en el motor de
auto-evolución (self_evolve/promote_branch/pursue_goal/..., borradas en W4.3).

Detector (AST puro, cero imports del paquete):
  - candidata: def pública top-level en mmorch/*.py, sin decorador (un decorador
    = registro en un framework, p.ej. las tools MCP de mcp_server).
  - caller vivo: cualquier referencia (Name/Attribute/import) en mmorch/ o scripts/,
    EXCLUYENDO tests/ y mmorch/__init__.py (importar en __init__ no es ejecutar).
  - allowlist automática: la API pública de librería exportada en __init__ (lo que
    importa __init__.py se consume desde afuera: MCP, otros repos, el usuario).
  - _DEUDA_MUSEO: allowlist EXPLÍCITA de museo pre-existente (audit 2026-08). El gate
    es un ratchet: impide que el museo CREZCA; cada entrada acá es deuda a cablear
    o borrar. Sacar una entrada cuando se cablea/borra su función.
"""
import ast
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
PKG = REPO / "mmorch"

# Museo pre-existente (sin caller vivo al momento de crear el gate, W4.3). Deuda:
# cablear o borrar; PROHIBIDO agregar entradas para código nuevo.
_DEUDA_MUSEO = {
    "run_ablation",         # ablation.py
    "task_type_classify",   # classify.py
    "multiview_verify",     # ensemble.py
    "embed_hybrid",         # exec_embedder.py
    "backfill",             # intuition.py
    "get_digest",           # memory.py
    "sweep_transcript",     # outcomes.py
    "unregister",           # projects.py
    "prune",                # projects.py
    "commit_rubric",        # rubric_loop.py
    "reveal_rubric",        # rubric_loop.py
    "read_notes",           # vault.py
    "block_manifest",       # workflow_store.py
    "block_scopes",         # workflow_store.py
}


def _init_public_api() -> set[str]:
    """__all__ + los nombres ORIGINALES que __init__ importa (un alias exportado —
    p.ej. advisory as offpeak_advisory — hace pública a la función original)."""
    tree = ast.parse((PKG / "__init__.py").read_text(encoding="utf-8"))
    api: set[str] = set()
    for node in tree.body:
        if isinstance(node, ast.Assign):
            for t in node.targets:
                if isinstance(t, ast.Name) and t.id == "__all__":
                    api |= {ast.literal_eval(e) for e in node.value.elts}
        elif isinstance(node, ast.ImportFrom):
            api |= {a.name for a in node.names}
    return api


def _public_defs() -> dict[str, str]:
    """{nombre: modulo} de defs públicas top-level sin decorador en mmorch/*.py."""
    out: dict[str, str] = {}
    for f in sorted(PKG.glob("*.py")):
        if f.name == "__init__.py":
            continue
        for node in ast.parse(f.read_text(encoding="utf-8")).body:
            if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
                if not node.name.startswith("_") and not node.decorator_list:
                    out.setdefault(node.name, f"mmorch/{f.name}")
    return out


def _live_references() -> set[str]:
    refs: set[str] = set()
    files = [f for f in PKG.glob("*.py") if f.name != "__init__.py"]
    files += list((REPO / "scripts").glob("*.py"))
    for f in files:
        for node in ast.walk(ast.parse(f.read_text(encoding="utf-8"))):
            if isinstance(node, ast.Name):
                refs.add(node.id)
            elif isinstance(node, ast.Attribute):
                refs.add(node.attr)
            elif isinstance(node, (ast.Import, ast.ImportFrom)):
                refs |= {a.name.split(".")[-1] for a in node.names}
    return refs


def test_cero_funciones_publicas_sin_caller_vivo():
    api = _init_public_api()
    defs = _public_defs()
    refs = _live_references()
    museo = {n: m for n, m in defs.items()
             if n not in api and n not in refs and n not in _DEUDA_MUSEO}
    assert museo == {}, (
        "Funciones públicas SIN caller vivo (cablear o borrar — no agregar a "
        f"_DEUDA_MUSEO): {sorted((m, n) for n, m in museo.items())}")


def test_deuda_museo_no_miente():
    """Cada entrada de _DEUDA_MUSEO debe seguir siendo museo real: si alguien la
    cableó o la borró, hay que SACARLA de la lista (el ratchet solo achica)."""
    api = _init_public_api()
    defs = _public_defs()
    refs = _live_references()
    curadas = {n for n in _DEUDA_MUSEO
               if n not in defs or n in api or n in refs}
    assert curadas == set(), (
        f"Estas entradas ya no son museo — sacarlas de _DEUDA_MUSEO: {sorted(curadas)}")
