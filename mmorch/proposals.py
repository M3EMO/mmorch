"""F2 propuesta (spec .scratch/loop-cerrado/spec.md): el nightly redacta tarjetas pre-cocinadas por proyecto."""

from pathlib import Path

from mmorch.iohelpers import atomic_write_json, load_json_tolerant
from mmorch.killswitch import paused


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
    if paused(logs_path):
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

