"""Auto-reparación nocturna: los errores que el sistema DETECTA se convierten
en intentos de fix SIN accionar humano — en worktree aislado, review branch,
merge siempre humano.

Fuente = errores ESTRUCTURADOS del último record de nightly.jsonl (jamás la
narrativa del digest): claves *_error, hardening.error, idea_loop.errors[],
project_health.errors[]. Determinista, sin LLM en la selección.

Guardrails: 1 reparación por noche, ventana de reintento de 5 días por firma
de error, kill-switch loop_paused, gate = suite completa verde (verdad de
ejecución). El resultado aparece en el digest como 🔧.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

from mmorch.iohelpers import atomic_write_json, load_json_tolerant

_RETRY_DAYS = 5


def findings_from_record(rec: dict) -> list[dict]:
    """Errores estructurados del record nocturno -> lista {source, detail}."""
    out = []
    for k, v in rec.items():
        if k.endswith("_error") and isinstance(v, str):
            out.append({"source": k, "detail": v})
        if isinstance(v, dict):
            if isinstance(v.get("error"), str):
                out.append({"source": k, "detail": v["error"]})
            for e in (v.get("errors") or []):
                if isinstance(e, str):
                    out.append({"source": k, "detail": e})
    # filtros medidos en los 2 primeros runs vivos (2026-08-18):
    # 1) artefactos EFIMEROS (worktrees/tmp borrados): irreparables post-hoc
    # 2) project_health: errores de OTROS repos — tocar codigo de mmorch no
    #    los arregla (el gate rechazo el intento); cross-repo repair es futuro
    return [f for f in out
            if "mmorch-wt-" not in f["detail"] and "mmorch_wt_" not in f["detail"]
            and f["source"] != "project_health"]


def _sig(f: dict) -> str:
    # firma estable del error: fuente + prefijo del detalle (los paths/nros
    # varian entre corridas; el tipo de error no)
    return hashlib.sha256(
        (f["source"] + "|" + f["detail"][:80]).encode()).hexdigest()[:16]


def pick_finding(findings: list[dict], state: dict, *, today: str) -> dict | None:
    for f in findings:
        prev = state.get(_sig(f))
        if prev and today <= prev.get("retry_after", ""):
            continue
        return f
    return None


def _default_build(task: str, wt_path: str, gate_cmd: str) -> dict:
    from mmorch.project_integrate import build_project
    return build_project(task, wt_path, external_test=gate_cmd,
                         max_fix=3, max_gen_calls=40)


def repair(repo_dir: str, *, today: str, build_fn=None,
           logs_dir: str | None = None, rec: dict | None = None) -> dict:
    """Una vuelta: peor hallazgo no reintentado -> REPAIR gateado en worktree.

    `rec`: si se pasa el record EN MEMORIA de esta misma corrida (nightly.py
    lo hace desde que auto_repair paso a correr al final), se repara lo que
    fallo ESTA noche en vez de leer nightly.jsonl y reparar lo de anoche."""
    logs = Path(logs_dir or (Path(repo_dir) / "logs"))
    if (logs / "loop_paused").exists():
        return {"skipped": "paused"}

    if rec is None:
        try:
            lines = (logs / "nightly.jsonl").read_text(encoding="utf-8").splitlines()
            rec = json.loads(lines[-1])
        except (OSError, IndexError, json.JSONDecodeError):
            return {"skipped": "sin record nocturno"}

    findings = findings_from_record(rec)
    if not findings:
        return {"skipped": "sin errores detectados"}

    state_path = logs / "repair_state.json"
    state = load_json_tolerant(state_path, {})
    target = pick_finding(findings, state, today=today)
    if target is None:
        return {"skipped": "todo en ventana de reintento"}

    from datetime import date, timedelta
    retry_after = (date.fromisoformat(today)
                   + timedelta(days=_RETRY_DAYS)).isoformat()
    sig = _sig(target)

    from mmorch.worktree_driver import open_worktree
    wt = open_worktree(repo_dir, prefix="mmorch/fix")
    try:
        wt.seed([".venv"])
        task = (
            f"REPAIR (auto-reparacion nocturna): el paso '{target['source']}' del "
            f"nightly fallo con este error:\n{target['detail'][:1500]}\n\n"
            "Arreglar la CAUSA RAIZ con el cambio minimo (hotfix, no refactor). "
            "Si el error es de un test generado (ej import faltante), arreglar el "
            "test; si es del modulo, arreglar el modulo. NO tocar guardrails, "
            "budgets ni configs. Estilo ruff-clean."
        )
        import sys as _sys
        gate_cmd = (f'"{_sys.executable}" -m pytest -q '
                    f"--basetemp={Path(repo_dir).drive}/Users/map12/AppData/Local/Temp/claude/pt-repair")
        bf = build_fn or _default_build
        res = bf(task, wt.path, gate_cmd)
        built = res.get("status") == "built"
        if built:
            wt.capture(f"auto-repair {target['source']}: {target['detail'][:60]}")
        out = {"source": target["source"], "detail": target["detail"][:150],
               "status": res.get("status"),
               "branch": wt.branch if built else None}
        if built:
            # carril verde del semaforo: solo-tests/archivos nuevos -> automerge
            try:
                import subprocess as _sp
                from mmorch.automerge import try_automerge
                base = _sp.run(["git", "rev-parse", "--abbrev-ref", "HEAD"],
                               cwd=repo_dir, capture_output=True,
                               text=True).stdout.strip()
                out["automerge"] = try_automerge(repo_dir, wt.branch, base=base,
                                                source="auto_repair")
            except Exception as e:
                out["automerge"] = {"merged": False, "reason": str(e)[:100]}
        # persistir DESPUES del automerge (05 #6): un crash entre persist y merge
        # dejaba repair_state sin el resultado real; ahora el estado escrito ya
        # incluye que paso con el merge, y un crash previo solo reintenta.
        state[sig] = {"retry_after": retry_after, "source": target["source"],
                      "result": res.get("status", "fail"),
                      "branch": wt.branch if built else None,
                      "automerge": out.get("automerge")}
        atomic_write_json(state_path, state)
        return out
    finally:
        keep = state.get(sig, {}).get("result") == "built"
        wt.close(keep_branch=keep)
