"""Hardening loop: mmorch se blinda solo contra sus puntos ciegos.

Cada noche (menos lunes, que es noche de caza) toma el PEOR modulo del ultimo
mapa de bug-hunt, re-caza sus mutantes sobrevivientes para obtener los diffs
frescos, y le pide al engine de project-build tests anti-mutante. El gate es
verdad de ejecucion pura (scripts/gate_hardening.py): survived DEBE bajar y la
suite completa quedar verde. Resultado = review branch, merge SOLO humano.
"""

from __future__ import annotations

import json
from pathlib import Path

from mmorch.iohelpers import atomic_write_json, load_json_tolerant

_RETRY_DAYS = 7  # un modulo intentado no se reintenta hasta pasada una semana


def load_last_map(logs_dir: str) -> list[dict]:
    """Ultimo mapa 'worst' registrado por la caza semanal en nightly.jsonl."""
    path = Path(logs_dir) / "nightly.jsonl"
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError:
        return []
    for line in reversed(lines):
        try:
            rec = json.loads(line)
        except json.JSONDecodeError:
            continue
        worst = (rec.get("bughunt") or {}).get("worst")
        if worst:
            return worst
    return []


def pick_target(worst: list[dict], state: dict, *, today: str,
                preferred: str | None = None) -> dict | None:
    """Peor modulo con sobrevivientes no intentado en los ultimos _RETRY_DAYS.
    `preferred` (el foco de la reflexion nocturna) va primero si es elegible."""
    ordered = sorted(worst, key=lambda x: -x.get("survived", 0) / max(x.get("mutants", 1), 1))
    if preferred:
        ordered = ([m for m in ordered if m["module"] == preferred]
                   + [m for m in ordered if m["module"] != preferred])
    for m in ordered:
        if not m.get("survived"):
            continue
        prev = state.get(m["module"])
        if prev and (today <= prev.get("retry_after", "")):
            continue
        return m
    return None


def _default_build(task: str, wt_path: str, gate_cmd: str) -> dict:
    from mmorch.project_integrate import build_project
    return build_project(task, wt_path, external_test=gate_cmd,
                         max_fix=3, max_gen_calls=60)


def harden(repo_dir: str, *, today: str, build_fn=None, survivors_fn=None,
           logs_dir: str | None = None) -> dict:
    """Una vuelta del loop: elegir peor modulo -> diffs frescos -> build gateado.

    Todo inyectable para tests. Retorna dict con lo que paso (fail-soft, el
    nightly no muere por esto)."""
    logs = logs_dir or str(Path(repo_dir) / "logs")
    if (Path(logs) / "loop_paused").exists():
        return {"skipped": "paused"}

    worst = load_last_map(logs)
    if not worst:
        return {"skipped": "sin mapa de caza previo"}

    state_path = Path(logs) / "hardening_state.json"
    state = load_json_tolerant(state_path, {})
    # foco de la reflexion nocturna (pienso->actuo, volante limitado)
    focus = load_json_tolerant(Path(logs) / "focus.json", {})
    target = pick_target(worst, state, today=today,
                         preferred=focus.get("hardening_module"))
    if target is None:
        return {"skipped": "sin objetivo (todo intentado o limpio)"}
    module = target["module"]

    from datetime import date, timedelta
    retry_after = (date.fromisoformat(today) + timedelta(days=_RETRY_DAYS)).isoformat()

    from mmorch.worktree_driver import open_worktree
    wt = open_worktree(repo_dir, prefix="mmorch/hard")
    try:
        wt.seed([".venv"])
        sf = survivors_fn
        if sf is None:
            from mmorch.bughunt import survivors_for
            sf = survivors_for
        test_rel = f"tests/test_{Path(module).stem}.py"
        r = sf(module, test_rel, repo_dir=wt.path, max_mutants=12)
        survived = r.get("survived", 0)
        if r.get("skipped") or not survived:
            state[module] = {"retry_after": retry_after, "result": "limpio_o_skip"}
            atomic_write_json(state_path, state)
            return {"module": module, "skipped": r.get("skipped", "sin sobrevivientes")}

        diffs = "\n---\n".join(r.get("survivor_diffs", []))[:8000]
        task = (
            f"Endurecer {test_rel} contra mutantes sobrevivientes de {module}.\n"
            f"CONTRATO: agregar tests dirigidos AL FINAL de {test_rel} (no borrar los "
            f"existentes) que MATEN estos mutantes — cada test debe fallar si la "
            f"mutacion estuviera aplicada. NO tocar {module} ni ningun otro archivo.\n"
            f"Mutantes sobrevivientes (diffs contra base normalizada):\n{diffs}\n"
            f"Estilo: ruff-clean, asserts de COMPORTAMIENTO (no de implementacion)."
        )
        import sys as _sys
        gate_cmd = f'"{_sys.executable}" scripts/gate_hardening.py {module} {survived}'
        bf = build_fn or _default_build
        res = bf(task, wt.path, gate_cmd)
        built = res.get("status") == "built"
        if built:
            wt.capture(f"hardening {module}: {survived} sobrevivientes atacados")
        state[module] = {"retry_after": retry_after,
                        "result": "built" if built else res.get("status", "fail"),
                        "survived_before": survived,
                        "branch": wt.branch if built else None}
        atomic_write_json(state_path, state)
        out = {"module": module, "survived_before": survived,
               "status": res.get("status"), "branch": wt.branch if built else None}
        if built:
            # carril verde: branch solo-tests con suite verde -> automerge
            try:
                from mmorch.automerge import try_automerge
                base = __import__("subprocess").run(
                    ["git", "rev-parse", "--abbrev-ref", "HEAD"], cwd=repo_dir,
                    capture_output=True, text=True).stdout.strip()
                out["automerge"] = try_automerge(repo_dir, wt.branch, base=base,
                                                source="hardening")
            except Exception as e:
                out["automerge"] = {"merged": False, "reason": str(e)[:100]}
        return out
    finally:
        # branch sobrevive solo si el build paso el gate (revision humana)
        keep = state.get(module, {}).get("result") == "built"
        wt.close(keep_branch=keep)
