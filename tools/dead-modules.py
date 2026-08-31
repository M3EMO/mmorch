#!/usr/bin/env python3
"""Ratchet anti-museo: falla si un modulo de mmorch/ no lo llama nadie fuera de tests/.

El criterio con el que se borro self_evolve (implementado, testeado, cero callers),
corriendo solo en vez de a mano una vez por año.

    python tools/dead-modules.py          # exit 1 si hay museo

Un modulo dormido a proposito va en tools/dormant-modules.txt con el por que.
Los entrypoints salen de [project.scripts] del pyproject: nadie los importa y esta bien.
"""

from __future__ import annotations

import ast
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PKG = ROOT / "mmorch"
ALLOWLIST = ROOT / "tools" / "dormant-modules.txt"

# ponytail: lista negra de directorios en vez de leer .gitignore — son 8 nombres
# y no cambian; parsear .gitignore es una dependencia o 40 lineas de globs.
SKIP = {
    ".git", ".venv", "venv", "__pycache__", "node_modules", "build", "dist",
    ".dataset_repos", "borrar", "mmorch.egg-info", ".mypy_cache", ".ruff_cache",
    "logs",  # salida de runtime: un json de una corrida vieja no es un caller
}


# Codigo que no es Python pero puede invocar `python -m mmorch.<x>` (hooks, workflows,
# scripts de deploy). Prosa (.md/.txt/.jsonl) queda afuera a proposito: catalog.md
# nombra los 134 modulos y resucitaria a todo el museo.
CODE_EXT = {".js", ".mjs", ".cjs", ".ts", ".sh", ".ps1", ".bat", ".yaml", ".yml", ".json", ".toml"}


def repo_files(exts: set[str]) -> list[Path]:
    out = []
    for p in ROOT.rglob("*"):
        if p.suffix in exts and SKIP.isdisjoint(p.relative_to(ROOT).parts):
            out.append(p)
    return out


def referenced_modules(path: Path, known: set[str]) -> set[str]:
    """`python -m mmorch.foo` o `mmorch/foo.py` en un hook, workflow o script."""
    text = path.read_text(encoding="utf-8", errors="ignore")
    return {m for m in known if f"mmorch.{m}" in text or f"mmorch/{m}." in text}


def imported_modules(path: Path, known: set[str]) -> set[str]:
    """Nombres de mmorch/<x>.py que este archivo importa. ast, no regex: un
    `import cache` adentro de un string o un comentario no cuenta."""
    try:
        tree = ast.parse(path.read_text(encoding="utf-8", errors="ignore"))
    except SyntaxError:
        return set()
    inside_pkg = PKG in path.parents
    found: set[str] = set()
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            for a in node.names:  # import mmorch.foo / import foo (solo dentro del pkg)
                head, _, tail = a.name.partition(".")
                if head == "mmorch" and tail.partition(".")[0] in known:
                    found.add(tail.partition(".")[0])
                elif inside_pkg and head in known:
                    found.add(head)
        elif isinstance(node, ast.ImportFrom):
            mod = node.module or ""
            if node.level and inside_pkg:  # from .foo import x / from . import foo
                if mod.partition(".")[0] in known:
                    found.add(mod.partition(".")[0])
                found |= {a.name for a in node.names if a.name in known}
            elif mod == "mmorch":  # from mmorch import foo
                found |= {a.name for a in node.names if a.name in known}
            elif mod.startswith("mmorch."):  # from mmorch.foo import x
                found.add(mod.split(".")[1])
            elif inside_pkg and mod.partition(".")[0] in known:  # from foo import x
                found.add(mod.partition(".")[0])
    return found


def entrypoints() -> set[str]:
    """Los console_scripts no los importa nadie y no son museo."""
    data = tomllib.loads((ROOT / "pyproject.toml").read_text(encoding="utf-8"))
    scripts = data.get("project", {}).get("scripts", {}).values()
    return {t.split(":")[0].removeprefix("mmorch.") for t in scripts}


def dormant() -> dict[str, str]:
    if not ALLOWLIST.exists():
        return {}
    out = {}
    for line in ALLOWLIST.read_text(encoding="utf-8").splitlines():
        name, _, why = line.partition("#")
        if name.strip():
            out[name.strip()] = why.strip()
    return out


def main() -> int:
    known = {p.stem for p in PKG.glob("*.py")} - {"__init__"}
    real: set[str] = set()   # importado desde codigo que no es test
    tested: set[str] = set()
    for f in repo_files({".py"}):
        hits = imported_modules(f, known) - {f.stem}  # no se cuenta a si mismo
        (tested if "tests" in f.relative_to(ROOT).parts else real).update(hits)
    for f in repo_files(CODE_EXT):
        real |= referenced_modules(f, known)

    skip, sleeping = entrypoints(), dormant()
    dead = sorted(known - real - skip - set(sleeping))
    woke = sorted(m for m in sleeping if m in real)

    print(f"modulos={len(known)} vivos={len(known & real)} entrypoints={len(skip)} "
          f"dormidos={len(sleeping)} museo={len(dead)}")
    if woke:
        print("\nya no estan dormidos (sacalos de dormant-modules.txt):")
        for m in woke:
            print(f"  {m}")
    if not dead:
        return 0
    print(f"\nMUSEO — {len(dead)} modulos que solo importan los tests (o nadie):")
    for m in dead:
        print(f"  {m:<24} tests={'si' if m in tested else 'NO'}")
    print("\nBorralo, o ponelo en tools/dormant-modules.txt con el por que.")
    return 1


if __name__ == "__main__":
    sys.exit(main())
