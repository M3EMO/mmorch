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

import hashlib
import json
import logging
import subprocess
import sys
import time
from pathlib import Path

logger = logging.getLogger(__name__)


def _git(repo: str, *args: str) -> subprocess.CompletedProcess:
    # encoding explicito: text=True usa el locale (cp1252 en Windows) y explota
    # con archivos utf-8 en los reader threads (medido en el smoke)
    return subprocess.run(["git", *args], cwd=repo, capture_output=True,
                          text=True, encoding="utf-8", errors="replace",
                          timeout=120)


def _diff_hash(repo: str, branch: str, *, base: str) -> str:
    """Identidad estable de LO QUE se juzgo: sha256 del patch completo base..branch.
    Va al ledger para poder auditar despues que el merge corresponde a ese diff."""
    patch = _git(repo, "diff", f"{base}..{branch}")
    return hashlib.sha256(patch.stdout.encode("utf-8", "replace")).hexdigest()[:16]


def classify_branch(repo: str, branch: str, *, base: str) -> dict:
    """Semáforo del diff base..branch: zone + archivos + checks pasados + diff_hash."""
    from mmorch.evolve import _RED_PATHS, red_content_hits
    checks: list[str] = []
    dh = _diff_hash(repo, branch, base=base)
    diff = _git(repo, "diff", "--name-status", f"{base}..{branch}")
    if diff.returncode != 0:
        return {"zone": "red", "reason": f"git diff fallo: {diff.stderr[:100]}",
                "checks": checks, "diff_hash": dh}
    files = []
    for line in diff.stdout.splitlines():
        parts = line.split("\t")
        if len(parts) >= 2:
            files.append({"status": parts[0][:1], "path": parts[-1]})
    if not files:
        return {"zone": "red", "reason": "diff vacio", "checks": checks,
                "diff_hash": dh}
    for f in files:
        p = f["path"].replace("\\", "/")
        if any(p == r or p.endswith("/" + r) for r in _RED_PATHS):
            return {"zone": "red", "reason": f"path rojo: {p}", "files": files,
                    "checks": checks, "diff_hash": dh}
    checks.append("paths_ok")
    # contenido: firmas rojas NUEVAS vs la base. Para archivos NUEVOS baseline=""
    # tambien vale: red_content_hits ya distingue secreto REAL (valor asignado con
    # entropia) de la mera palabra en tests/docs (fix falsos-rojos 05 #7).
    for f in files:
        after = _git(repo, "show", f"{branch}:{f['path']}")
        before = _git(repo, "show", f"{base}:{f['path']}")
        hits = red_content_hits(after.stdout if after.returncode == 0 else "",
                                baseline=before.stdout if before.returncode == 0 else "")
        if hits:
            return {"zone": "red", "reason": f"contenido rojo en {f['path']}: {hits[:3]}",
                    "files": files, "checks": checks, "diff_hash": dh}
    checks.append("contenido_ok")
    green = all(f["path"].replace("\\", "/").startswith("tests/")
                or f["status"] == "A" for f in files)
    if green:
        checks.append("solo_tests_o_nuevos")
    return {"zone": "green" if green else "yellow", "files": files,
            "checks": checks, "diff_hash": dh}


def try_automerge(repo: str, branch: str, *, base: str,
                  source: str = "") -> dict:
    """Merge automático SOLO si el semáforo da verde. Ledger SIEMPRE, con el
    schema obligatorio de auditoria: ts, branch, diff_hash, checks pasados,
    veredicto y sha del merge (rollback = `git revert -m 1 <merge_sha>`)."""
    logs = Path(repo) / "logs"
    result: dict
    if (logs / "loop_paused").exists():
        result = {"merged": False, "zone": "paused", "branch": branch,
                  "veredicto": "paused", "checks": [], "diff_hash": None,
                  "merge_sha": None}
    else:
        c = classify_branch(repo, branch, base=base)
        common = {"branch": branch, "checks": ["kill_switch_ok", *c.get("checks", [])],
                  "diff_hash": c.get("diff_hash")}
        if c["zone"] != "green":
            result = {"merged": False, "zone": c["zone"],
                      "veredicto": f"rechazado_{c['zone']}", "merge_sha": None,
                      "reason": c.get("reason", "edita modulos existentes"),
                      **common}
        else:
            m = _git(repo, "merge", "--no-edit", "--no-ff", branch)
            if m.returncode != 0:
                _git(repo, "merge", "--abort")
                result = {"merged": False, "zone": "green",
                          "veredicto": "merge_fallido", "merge_sha": None,
                          "reason": f"merge fallo: {(m.stdout + m.stderr)[:150]}",
                          **common}
            else:
                sha = _git(repo, "rev-parse", "HEAD").stdout.strip() or None
                result = {"merged": True, "zone": "green", "veredicto": "merged",
                          "merge_sha": sha,
                          "files": [f["path"] for f in c.get("files", [])],
                          **common}
                # outcome retroactivo al brazo que produjo la branch: el
                # merge verde automatico tambien es veredicto de ejecucion
                try:
                    from .provenance import on_merge
                    on_merge(branch, logs_dir=str(logs))
                except Exception as e:
                    # No romper el main path, pero registrar el fallo
                    # explícitamente: la provenance inconsistente es un
                    # problema de observabilidad, no de merge.
                    logger.error(
                        "on_merge falló para branch %s (merge ya aplicado): %s",
                        branch, e, exc_info=True
                    )
                    # También al stderr para que sea visible en logs del
                    # proceso, no solo en el logger configurado.
                    print(
                        f"WARNING: on_merge falló para {branch}: {e}",
                        file=sys.stderr
                    )
    try:
        logs.mkdir(parents=True, exist_ok=True)
        with open(logs / "automerge_ledger.jsonl", "a", encoding="utf-8") as f:
            f.write(json.dumps({"ts": time.time(), "source": source, **result},
                               ensure_ascii=False) + "\n")
    except OSError as e:
        # El ledger es best-effort; si falla, al menos loguear.
        logger.error("No se pudo escribir ledger: %s", e)
    return result