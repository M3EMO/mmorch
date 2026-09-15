"""Gate de capas (ratchet, 2026-09-14): los modulos que NINGUNA entrada real alcanza no vuelven a
entrar al engine por la puerta de atras, y la lista solo puede ACHICARSE (se poda o se cablea).

Medicion de origen: vault `mmorch-modulos-sin-uso-medicion-2026-09-14`. Entradas reales = mcp_server,
server, nightly, cli, auto_apply_nightly, hillclimb, autoresearch y lo que importan scripts/*.py.
Alcance = imports estaticos (ast). Import dinamico conocido: `loop` (loop_nightly lo carga por nombre).

Reglas:
- R1 ningun modulo alcanzado importa uno de _NO_ALCANZADOS (si lo cablean, sale de la lista en el mismo commit).
- R2 no aparecen modulos NUEVOS sin alcanzar: cada modulo nuevo entra cableado o no entra.
- R3 cada entrada de la lista existe: al borrar un modulo se lo saca de la lista (ratchet honesto).
"""
from __future__ import annotations

import ast
import functools
import glob
import re
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
PKG = REPO / "mmorch"

_ENTRADAS = ("mcp_server", "server", "nightly", "cli", "auto_apply_nightly", "hillclimb", "autoresearch")
_DINAMICOS = {"loop"}   # cargado por nombre desde loop_nightly; 4173 usos en 90 dias

# Medido 2026-09-14 con este mismo analisis (15). Cada uno: se cablea (y sale de aca) o se borra (y sale
# de aca). Nunca crece. plugin_worker es el subproceso de plugins: se ejecuta por path, no por import.
_NO_ALCANZADOS = {
    "megasource", "plugin_worker", "synth_store",
}


def _modulos() -> dict[str, Path]:
    return {p.stem: p for p in PKG.glob("*.py") if not p.stem.startswith("__")}


@functools.lru_cache(maxsize=None)
def _reexports() -> dict[str, str]:
    """nombre exportado por mmorch/__init__.py -> modulo que lo define (`from .mod import nombre`).
    Cacheado: sin cache, cada _imports() re-parseaba __init__ y R3 tardaba > 15 min en un worktree cargado."""
    out: dict[str, str] = {}
    try:
        tree = ast.parse((PKG / "__init__.py").read_text(encoding="utf-8", errors="replace"))
    except (SyntaxError, OSError):
        return out
    for n in ast.walk(tree):
        if isinstance(n, ast.ImportFrom) and n.level == 1 and n.module:
            for a in n.names:
                out[a.asname or a.name] = n.module.split(".")[0]
    return out


def _imports(path: Path, mods: set[str]) -> set[str]:
    try:
        tree = ast.parse(path.read_text(encoding="utf-8", errors="replace"))
    except SyntaxError:
        return set()
    rex = _reexports()
    out: set[str] = set()
    for n in ast.walk(tree):
        if isinstance(n, ast.ImportFrom):
            if n.level == 1 and n.module:
                out.add(n.module.split(".")[0])
            elif (n.level == 1 and not n.module) or (n.level == 0 and n.module == "mmorch"):
                # `from . import x` / `from mmorch import x`: x es un modulo o un nombre re-exportado por __init__
                for a in n.names:
                    out.add(a.name if a.name in mods else rex.get(a.name, ""))
            elif n.module and n.module.startswith("mmorch."):
                out.add(n.module.split(".")[1])
        elif isinstance(n, ast.Import):
            out.update(a.name.split(".")[1] for a in n.names if a.name.startswith("mmorch."))
    return {m for m in out if m in mods and m != path.stem}


@functools.lru_cache(maxsize=None)
def _alcanzados() -> frozenset[str]:
    return frozenset(_alcanzados_raw())


def _alcanzados_raw() -> set[str]:
    mods = _modulos()
    entradas = [e for e in _ENTRADAS if e in mods]
    for f in glob.glob(str(REPO / "scripts" / "*.py")) + glob.glob(str(REPO / "*.py")):
        src = Path(f).read_text(encoding="utf-8", errors="replace")
        for a, b in re.findall(r"from mmorch(?:\.(\w+))? import|import mmorch\.(\w+)", src):
            for x in (a, b):
                if x in mods and x not in entradas:
                    entradas.append(x)
    seen = set(entradas) | _DINAMICOS
    stack = list(seen)
    while stack:
        m = stack.pop()
        for d in _imports(mods[m], set(mods)):
            if d not in seen:
                seen.add(d)
                stack.append(d)
    return seen


def test_R1_el_engine_no_importa_modulos_no_alcanzados():
    mods = _modulos()
    vivos = _alcanzados()
    culpables = {m: sorted(_imports(mods[m], set(mods)) & _NO_ALCANZADOS)
                 for m in vivos if _imports(mods[m], set(mods)) & _NO_ALCANZADOS}
    assert culpables == {}, f"modulos vivos que importan no-alcanzados (cablear = sacarlos de la lista): {culpables}"


def test_R2_no_hay_modulos_nuevos_sin_alcanzar():
    huerfanos = set(_modulos()) - _alcanzados() - _NO_ALCANZADOS
    assert huerfanos == set(), f"modulos nuevos sin entrada real (cablear o borrar, no agregar a la lista): {sorted(huerfanos)}"


def test_R3_la_lista_solo_achica():
    faltan = sorted(m for m in _NO_ALCANZADOS if not (PKG / f"{m}.py").exists())
    assert faltan == [], f"modulos borrados que siguen en la lista (sacarlos): {faltan}"
    cableados = sorted(m for m in _NO_ALCANZADOS if m in _alcanzados())
    assert cableados == [], f"modulos ya cableados que siguen en la lista (sacarlos): {cableados}"


def test_R4_museo_por_modulo_no_crece():
    # el numero baja con cada poda; subirlo a mano es agregar museo
    assert len(_NO_ALCANZADOS) <= 7


def test_R5_sin_importlib_hacia_modulos_propios():
    # D14 (2026-09-14): el coder esquivo R1 con importlib.import_module('.effort'). plugins carga por path, es la excepcion.
    culpables = sorted(m for m, p in _modulos().items() if m != "plugins"
                       and re.search(r"import_module\(\s*['\"]\.", p.read_text(encoding="utf-8", errors="replace")))
    assert culpables == [], f"import dinamico de modulos propios (usar import estatico): {culpables}"
