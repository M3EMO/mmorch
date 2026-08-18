"""Automerge con semáforo — merges sin accionar humano SOLO en el carril verde.

Política (conservadora a propósito):
  🟢 automerge: el diff toca SOLO tests/ y/o archivos NUEVOS aislados, sin
     firmas de contenido rojo, y el caller ya paso el gate de ejecución
     (suite verde). Un test no puede romper producción.
  🟡 review branch (comportamiento actual): cualquier edición a módulo
     existente. Se automatiza recién cuando exista rollback estructural.
  🔴 jamás: paths o capacidades de zona roja (reusa evolve._RED_PATHS y
     red_content_hits — el MISMO semáforo del sistema, no uno nuevo).

Ledger append-only en logs/automerge_ledger.jsonl. Kill-switch: loop_paused.
"""

from __future__ import annotations

import json
import subprocess
import time
from pathlib import Path


def _git(repo: str, *args: str) -> subprocess.CompletedProcess:
    # encoding explicito: text=True usa el locale (cp1252 en Windows) y explota
    # con archivos utf-8 en los reader threads (medido en el smoke)
    return subprocess.run(["git", *args], cwd=repo, capture_output=True,
                          text=True, encoding="utf-8", errors="replace",
                          timeout=120)


def classify_branch(repo: str, branch: str, *, base: str) -> dict:
    """Semáforo del diff base..branch: zone + detalle de archivos."""
    from mmorch.evolve import _RED_PATHS, red_content_hits
    diff = _git(repo, "diff", "--name-status", f"{base}..{branch}")
    if diff.returncode != 0:
        return {"zone": "red", "reason": f"git diff fallo: {diff.stderr[:100]}"}
    files = []
    for line in diff.stdout.splitlines():
        parts = line.split("\t")
        if len(parts) >= 2:
            files.append({"status": parts[0][:1], "path": parts[-1]})
    if not files:
        return {"zone": "red", "reason": "diff vacio"}
    for f in files:
        p = f["path"].replace("\\", "/")
        if any(p == r or p.endswith("/" + r) for r in _RED_PATHS):
            return {"zone": "red", "reason": f"path rojo: {p}", "files": files}
    # contenido: firmas rojas NUEVAS vs la base
    for f in files:
        after = _git(repo, "show", f"{branch}:{f['path']}")
        before = _git(repo, "show", f"{base}:{f['path']}")
        hits = red_content_hits(after.stdout if after.returncode == 0 else "",
                                baseline=before.stdout if before.returncode == 0 else "")
        if hits:
            return {"zone": "red", "reason": f"contenido rojo en {f['path']}: {hits[:3]}",
                    "files": files}
    green = all(f["path"].replace("\\", "/").startswith("tests/")
                or f["status"] == "A" for f in files)
    return {"zone": "green" if green else "yellow", "files": files}


def try_automerge(repo: str, branch: str, *, base: str,
                  source: str = "") -> dict:
    """Merge automático SOLO si el semáforo da verde. Ledger siempre."""
    logs = Path(repo) / "logs"
    result: dict
    if (logs / "loop_paused").exists():
        result = {"merged": False, "zone": "paused", "branch": branch}
    else:
        c = classify_branch(repo, branch, base=base)
        if c["zone"] != "green":
            result = {"merged": False, "zone": c["zone"],
                      "reason": c.get("reason", "edita modulos existentes"),
                      "branch": branch}
        else:
            m = _git(repo, "merge", "--no-edit", "--no-ff", branch)
            if m.returncode != 0:
                _git(repo, "merge", "--abort")
                result = {"merged": False, "zone": "green",
                          "reason": f"merge fallo: {(m.stdout + m.stderr)[:150]}",
                          "branch": branch}
            else:
                result = {"merged": True, "zone": "green", "branch": branch,
                          "files": [f["path"] for f in c.get("files", [])]}
    try:
        logs.mkdir(exist_ok=True)
        with open(logs / "automerge_ledger.jsonl", "a", encoding="utf-8") as f:
            f.write(json.dumps({"ts": time.time(), "source": source, **result},
                               ensure_ascii=False) + "\n")
    except OSError:
        pass
    return result
