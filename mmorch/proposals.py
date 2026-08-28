"""F2 propuesta (spec .scratch/loop-cerrado/spec.md): tarjetas pre-cocinadas + pick del hook.

compose_cards corre en el nightly (redacta la tarjeta en espanol por proyecto);
pick_card corre en el hook SessionStart (local, ms, fail-open) y entrega la
mejor propuesta pendiente para el proyecto del cwd, incrementando shown_count.
"""

from pathlib import Path

from mmorch.iohelpers import atomic_write_json, load_json_tolerant

_MAX_SHOWN = 5


def _card_text(match: dict) -> str:
    note_name = Path(match["note_path"]).name
    cita = f" Cita: {match['cited_file']}." if match.get("cited_file") else ""
    return (
        f"💡 mmorch: la nota {note_name} aplica a {match['project']} — "
        f"{match['justification']}.{cita} (score {match['score']:.2f}, "
        "refutado y sobrevivió).\n"
        'Respondé "dale" (la arranco en sandbox), "no" (no la propongo más), '
        "o ignorala (expira sola)."
    )


def compose_cards(*, logs_dir: str = "logs") -> dict:
    """Agrega "card" al mejor strong pendiente sin card de cada proyecto."""
    logs_path = Path(logs_dir)
    if (logs_path / "loop_paused").exists():
        return {"skipped": True}
    state_path = logs_path / "adjudications.json"
    state = load_json_tolerant(state_path, {})
    new_cards = 0
    for _project, matches in (state.get("by_project") or {}).items():
        candidates = [m for m in matches
                      if m.get("status") == "pendiente" and "card" not in m]
        if not candidates:
            continue
        best = max(candidates, key=lambda m: m.get("score", 0.0))
        best["card"] = _card_text(best)
        new_cards += 1
    if new_cards:
        atomic_write_json(state_path, state)
    return {"cards": new_cards}


def pick_card(cwd: str, projects: dict, *, logs_dir: str = "logs") -> str | None:
    """Mejor propuesta pendiente con card para el proyecto del cwd; None si no hay.

    Fail-open total: cualquier excepcion interna -> None (el hook jamas rompe
    el arranque de una sesion).
    """
    try:
        cwd_path = Path(cwd).resolve()
        project = None
        best_len = -1
        for name, path in projects.items():
            p = Path(path).resolve()
            if (cwd_path == p or p in cwd_path.parents) and len(str(p)) > best_len:
                project, best_len = name, len(str(p))
        if project is None:
            return None

        state_path = Path(logs_dir) / "adjudications.json"
        state = load_json_tolerant(state_path, {})
        candidates = [m for m in (state.get("by_project") or {}).get(project, [])
                      if m.get("status") == "pendiente" and m.get("card")
                      and m.get("shown_count", 0) < _MAX_SHOWN]
        if not candidates:
            return None
        best = max(candidates, key=lambda m: m.get("score", 0.0))
        best["shown_count"] = best.get("shown_count", 0) + 1
        atomic_write_json(state_path, state)
        return best["card"]
    except Exception:
        return None
