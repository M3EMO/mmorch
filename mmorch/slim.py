"""Slim — auto-eficientización de código: menos verbose, misma conducta.

Cada noche UN módulo (rotación por tamaño, el más gordo primero) recibe un
patch de simplificación (dedup, guard clauses, menos ruido) con la API
pública INTACTA. El gate es el de siempre: sandbox + suite completa. La
branch resultante (mmorch-sbx-*) es amarilla y el merge train la levanta
solo — cero paso nuevo para el humano.

Reusa TODO de evolve: propose_patch, snapshot_change, coordinated_evolve_round
(sandbox+tests+lock por archivo). Este módulo solo elige el objetivo y
formula el finding. Retry window 7 días por módulo.
"""

from __future__ import annotations

from pathlib import Path

from mmorch.iohelpers import atomic_write_json, load_json_tolerant

_RETRY_DAYS = 7
_SKIP = {"__init__.py"}


def pick_module(root: Path, state: dict, *, today: str) -> str | None:
    """El .py más grande de mmorch/ no intentado en la ventana (gordo =
    donde la verbosidad paga más)."""
    mods = sorted((f for f in (root / "mmorch").glob("*.py")
                   if f.name not in _SKIP and not f.name.startswith("_")),
                  key=lambda f: -f.stat().st_size)
    for m in mods:
        rel = f"mmorch/{m.name}"
        prev = state.get(rel)
        if prev and today <= prev.get("retry_after", ""):
            continue
        return rel
    return None


def slim_one(orch_root: str, *, today: str, evolve_round_fn=None,
             propose_fn=None) -> dict:
    """Una vuelta: elegir módulo → finding de simplificación → sandbox+suite.
    Branch verde queda para el tren; roja se descarta sola."""
    root = Path(orch_root)
    logs = root / "logs"
    if (logs / "loop_paused").exists():
        return {"skipped": "paused"}

    state_path = logs / "slim_state.json"
    state = load_json_tolerant(state_path, {})
    target = pick_module(root, state, today=today)
    if target is None:
        return {"skipped": "todo en ventana de reintento"}

    from datetime import date, timedelta
    retry_after = (date.fromisoformat(today)
                   + timedelta(days=_RETRY_DAYS)).isoformat()

    finding = (
        "SIMPLIFICAR sin cambiar conducta: reducir verbosidad y duplicación "
        "(guard clauses sobre anidamiento, DRY, menos ruido, nombres claros). "
        "PROHIBIDO: cambiar la API pública (firmas, nombres exportados, "
        "shapes de retorno), borrar comentarios de POR QUÉ, tocar guardrails. "
        "La suite completa debe seguir verde — es el único juez."
    )
    from mmorch.evolve import coordinated_evolve_round, propose_patch, snapshot_change
    propose = propose_fn or propose_patch
    try:
        after = propose(target, finding)
        change = snapshot_change(target, after, f"slim: {target}", root=root)
    except Exception as e:
        state[target] = {"retry_after": retry_after, "result": f"propose_fail: {e}"[:120]}
        atomic_write_json(state_path, state)
        return {"module": target, "status": "propose_fail", "error": str(e)[:120]}

    # guard barato: un slim que AGRANDA el archivo no es slim
    if len(change.after) >= len(change.before):
        state[target] = {"retry_after": retry_after, "result": "no_adelgazo"}
        atomic_write_json(state_path, state)
        return {"module": target, "status": "no_adelgazo",
                "antes": len(change.before), "despues": len(change.after)}

    round_fn = evolve_round_fn or (
        lambda c: coordinated_evolve_round([c], root=root, open_pr=False))
    res = round_fn(change)
    opened = target in (res.get("opened") or [])
    state[target] = {"retry_after": retry_after,
                     "result": "branch" if opened else "suite_roja",
                     "ahorro_chars": len(change.before) - len(change.after)}
    atomic_write_json(state_path, state)
    return {"module": target, "status": "branch" if opened else "suite_roja",
            "ahorro_chars": len(change.before) - len(change.after)}
