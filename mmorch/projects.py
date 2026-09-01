"""projects — registro de proyectos que mmorch puede CONTROLAR (project-aware). Hace que
el dashboard/los jobs apunten a un repo real (portfolio, etc.) en vez de un sandbox tmp.

Datos en projects.json (capa amarilla, separada del codigo). resolve() valida que el path
exista y sea dir ANTES de dejar que un job lo toque. El path es la frontera: un job solo
trabaja dentro del repo registrado, nunca afuera.
"""
from __future__ import annotations

import json
import os
from pathlib import Path

from .paths import data_dir

PROJECTS_PATH = data_dir() / "projects.json"


def _load(path: Path | None = None) -> dict:
    p = Path(path or PROJECTS_PATH)
    if not p.exists():
        return {}
    try:
        return json.loads(p.read_text(encoding="utf-8"))
    except Exception:
        return {}


def _save(data: dict, path: Path | None = None) -> None:
    p = Path(path or PROJECTS_PATH)
    p.write_text(json.dumps(data, ensure_ascii=False, indent=2), encoding="utf-8")


def register(name: str, path: str, *, store: Path | None = None) -> dict:
    """Registra un proyecto. path debe existir y ser directorio (frontera del job)."""
    ap = os.path.abspath(os.path.expanduser(path))
    if not os.path.isdir(ap):
        raise ValueError(f"path no es un directorio existente: {ap}")
    data = _load(store)
    data[name] = ap
    _save(data, store)
    return {"name": name, "path": ap}


def unregister(name: str, *, store: Path | None = None) -> bool:
    data = _load(store)
    if name in data:
        del data[name]
        _save(data, store)
        return True
    return False


def list_projects(*, store: Path | None = None) -> dict:
    return _load(store)


def resolve(name: str, *, store: Path | None = None) -> str:
    """Path absoluto del proyecto. Lanza si no existe el registro o el dir desaparecio."""
    data = _load(store)
    if name not in data:
        raise KeyError(f"proyecto '{name}' no registrado. registrados: {sorted(data)}")
    ap = data[name]
    if not os.path.isdir(ap):
        raise ValueError(f"proyecto '{name}' apunta a un path inexistente: {ap}")
    return ap


def prune(*, store: Path | None = None, dry_run: bool = True) -> dict[str, str]:
    """GC de projects.json (audit-2026-08 #19): resolve() ya detecta un path muerto call-by-call,
    pero nadie los SACA -> quedan acumulando (ej un tmpdir de pytest que ya no existe). Devuelve
    {name: path} de las entradas MUERTAS (no-dir). dry_run=True (default) SOLO reporta, no
    escribe -- higiene es una accion explicita, no un side-effect de leer el registro."""
    data = _load(store)
    dead = {name: path for name, path in data.items() if not os.path.isdir(path)}
    if dead and not dry_run:
        # via unregister() y no borrando el dict a mano: UN solo camino de baja
        # del registro. Recarga por entrada, pero projects.json son unidades.
        for name in dead:
            unregister(name, store=store)
    return dead
