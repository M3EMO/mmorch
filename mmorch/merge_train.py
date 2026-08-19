"""Merge train — las branches amarillas del dia se conglomeran en UN merge.

Motivo tecnico (no solo comodidad): dos amarillas individualmente verdes
pueden romperse ENTRE SI — el tren corre la suite completa sobre la UNION
(misma leccion que el integration gate del project-build). La que conflictua
se aparta y vuelve a la cola individual; jamas frena al resto.

Resultado: branch `mmorch/tren-YYYY-MM-DD` verde = un solo click humano por
dia, sean 3 o 300 cambios. (Fase 2, con rollback estructural + SPC: el tren
pasa a merge-then-monitor y el click desaparece.)
"""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import time
from pathlib import Path


def _git(cwd: str, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(["git", *args], cwd=cwd, capture_output=True,
                          text=True, timeout=300)


def yellow_branches(repo: str, *, base: str) -> list[str]:
    """Branches construidas por el sistema (mmorch/hard-*, mmorch/fix-*,
    mmorch-sbx-*) que existen, no estan mergeadas y clasifican amarillo."""
    from mmorch.automerge import classify_branch
    out = _git(repo, "branch", "--list", "--no-merged", base,
               "mmorch/hard*", "mmorch/fix*", "mmorch-sbx-*",
               "--format=%(refname:short)")
    branches = [b.strip() for b in out.stdout.splitlines() if b.strip()]
    return [b for b in branches
            if classify_branch(repo, b, base=base).get("zone") == "yellow"]


def run_train(repo: str, *, base: str, today: str | None = None,
              test_fn=None) -> dict:
    """Arma el tren en un worktree aislado: merge secuencial + suite sobre la
    union. Retorna {train_branch, merged, skipped_conflict, gate}."""
    logs = Path(repo) / "logs"
    if (logs / "loop_paused").exists():
        return {"skipped": "paused"}
    today = today or time.strftime("%Y-%m-%d")
    branches = yellow_branches(repo, base=base)
    if not branches:
        return {"skipped": "sin amarillas pendientes"}

    train = f"mmorch/tren-{today}"
    _git(repo, "branch", "-D", train)                       # tren previo del dia
    from mmorch.worktree_driver import open_worktree
    wt = open_worktree(repo, base=base, branch=None)
    merged, skipped = [], []
    gate_ok = False
    try:
        wt.seed([".venv"])
        for b in branches:
            m = _git(wt.path, "merge", "--no-edit", b)
            if m.returncode != 0:
                _git(wt.path, "merge", "--abort")
                skipped.append(b)                            # vuelve a la cola
            else:
                merged.append(b)
        gate_reason = None
        if merged:
            if test_fn is None:
                bt = tempfile.mkdtemp(prefix="mmorch_bt_")
                def test_fn():
                    p = subprocess.run([sys.executable, "-m", "pytest", "-q",
                                        f"--basetemp={bt}"], cwd=wt.path,
                                       capture_output=True, timeout=1800)
                    return p.returncode == 0
            try:
                gate_ok = bool(test_fn())
            except subprocess.TimeoutExpired:
                # sin esto la excepcion se comia el registro ENTERO de la
                # noche: gate rojo con motivo es señal; nada es agujero negro
                gate_ok = False
                gate_reason = "timeout"
            if gate_ok:
                # renombrar la branch del worktree al nombre del tren
                _git(repo, "branch", "-f", train, wt.branch)
    finally:
        wt.close(keep_branch=False)

    result = {"train_branch": train if (merged and gate_ok) else None,
              "merged": merged, "skipped_conflict": skipped,
              "gate": "verde" if gate_ok else ("rojo" if merged else "vacio"),
              "gate_reason": gate_reason}
    try:
        logs.mkdir(exist_ok=True)
        with open(logs / "merge_train.jsonl", "a", encoding="utf-8") as f:
            f.write(json.dumps({"ts": time.time(), **result},
                               ensure_ascii=False) + "\n")
    except OSError:
        pass
    return result
