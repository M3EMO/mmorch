"""Reparación cross-repo: mmorch arregla los proyectos del REGISTRY, no solo
a sí mismo — mismo sistema (worktree + engine + gate de ejecución), guardrails
más duros por ser territorio ajeno:

  - dispara SOLO ante suite roja detectada por project_health
  - repara en un worktree DEL proyecto, con el venv DEL proyecto como gate
  - resultado SIEMPRE amarillo: review branch en ese repo, jamás automerge
    (cada proyecto gana confianza con historial, como mmorch la ganó)
  - 1 proyecto por noche, ventana de reintento 5 días, kill-switch global

El digest lo reporta como 🏥. Trenes por proyecto: cuando haya volumen
(merge_train ya es repo-genérico, run_train(repo_del_proyecto)).
"""

from __future__ import annotations

import json
from pathlib import Path

from mmorch.iohelpers import atomic_write_json, load_json_tolerant

_RETRY_DAYS = 5


def failing_projects(rec: dict) -> list[str]:
    """Proyectos con suite roja del último record (failing; los timeout de
    errors[] NO — colgado != roto, y el repair colgaría igual)."""
    ph = rec.get("project_health") or {}
    return list(ph.get("failing") or [])


def repair_projects(orch_root: str, *, today: str, build_fn=None,
                    logs_dir: str | None = None) -> dict:
    from mmorch.projects import _load as load_projects
    logs = Path(logs_dir or (Path(orch_root) / "logs"))
    if (logs / "loop_paused").exists():
        return {"skipped": "paused"}
    try:
        rec = json.loads((logs / "nightly.jsonl").read_text(
            encoding="utf-8").strip().splitlines()[-1])
    except (OSError, IndexError, json.JSONDecodeError):
        return {"skipped": "sin record nocturno"}

    failing = failing_projects(rec)
    if not failing:
        return {"skipped": "sin suites rojas"}

    state_path = logs / "project_repair_state.json"
    state = load_json_tolerant(state_path, {})
    registry = load_projects()
    target = None
    for name in failing:
        prev = state.get(name)
        if prev and today <= prev.get("retry_after", ""):
            continue
        path = registry.get(name)
        if path and (Path(path) / ".git").exists():
            target = (name, path)
            break
    if target is None:
        return {"skipped": "sin objetivo elegible (retry window / sin git)"}

    name, path = target
    from datetime import date, timedelta
    retry_after = (date.fromisoformat(today)
                   + timedelta(days=_RETRY_DAYS)).isoformat()

    venv_py = Path(path) / ".venv" / "Scripts" / "python.exe"
    import sys as _sys
    py = str(venv_py) if venv_py.exists() else _sys.executable
    import tempfile
    bt = tempfile.mkdtemp(prefix="mmorch_bt_")
    gate_cmd = f'"{py}" -m pytest -q -x --basetemp={bt}'

    from mmorch.worktree_driver import open_worktree
    wt = open_worktree(path, prefix="mmorch/sana")
    try:
        wt.seed([".venv"])
        task = (
            f"REPAIR cross-repo: la suite de tests de este proyecto ({name}) esta "
            f"ROJA. Correr los tests, identificar el fallo y arreglar la CAUSA "
            f"RAIZ con el cambio minimo. Si el codigo es correcto y el test quedo "
            f"desactualizado tras un cambio intencional, actualizar el test. "
            f"NO tocar configs, credenciales, .env ni logica de dinero/trading. "
            f"Estilo del repo."
        )
        if build_fn is None:
            from mmorch.project_integrate import build_project

            def build_fn(t, w, g):
                return build_project(t, w, external_test=g,
                                     max_fix=3, max_gen_calls=40)
        res = build_fn(task, wt.path, gate_cmd)
        built = res.get("status") == "built"
        if built:
            wt.capture(f"mmorch sana {name}: suite roja reparada")
        state[name] = {"retry_after": retry_after,
                       "result": res.get("status", "fail"),
                       "branch": wt.branch if built else None}
        atomic_write_json(state_path, state)
        return {"project": name, "status": res.get("status"),
                "branch": wt.branch if built else None,
                "repo": path}
    finally:
        keep = state.get(name, {}).get("result") == "built"
        wt.close(keep_branch=keep)
