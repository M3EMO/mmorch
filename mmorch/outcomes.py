"""Outcome recording and expiry for proposals."""

import re
from pathlib import Path

from mmorch import feedback
from mmorch.iohelpers import atomic_write_json, load_json_tolerant

REWARD_DALE = 1.0
REWARD_NO = 0.125
REWARD_BLANDO = 0.2
ARM = "propuesta:nota"

_CARD_RE = re.compile(r"la nota (\S+) aplica a (\S+) —")


def _load_adjudications(logs_dir):
    """Load adjudications.json tolerantly."""
    path = Path(logs_dir) / "adjudications.json"
    return load_json_tolerant(path, {})


def _save_adjudications(data, logs_dir):
    """Persist adjudications.json atomically."""
    path = Path(logs_dir) / "adjudications.json"
    atomic_write_json(path, data)


def _find_match(data, proposal_id):
    """Find a match by id in by_project."""
    by_project = data.get("by_project", {})
    for _project, matches in by_project.items():
        for match in matches:
            if match.get("id") == proposal_id:
                return match
    return None


def _record_outcome(record_fn, reward, pattern, source):
    """Record outcome via provided fn or default."""
    if record_fn is None:
        feedback.record_outcome(ARM, reward, pattern=pattern, source=source)
    else:
        record_fn(ARM, reward, pattern=pattern, source=source)


def record_verdict(proposal_id, verdict, *, logs_dir="logs", record_fn=None):
    """Record a verdict for a proposal."""
    if verdict not in {"dale", "no"}:
        raise ValueError(f"Invalid verdict: {verdict}")

    data = _load_adjudications(logs_dir)
    match = _find_match(data, proposal_id)

    if match is None:
        return {"recorded": False, "status": None}

    current_status = match.get("status")
    if current_status != "pendiente":
        return {"recorded": False, "status": current_status}

    if verdict == "dale":
        new_status = "aceptada"
        reward = REWARD_DALE
    else:
        new_status = "rechazada"
        reward = REWARD_NO

    match["status"] = new_status
    _save_adjudications(data, logs_dir)
    _record_outcome(record_fn, reward, pattern="loop_propuestas", source="verdict")

    return {"recorded": True, "status": new_status}


def expire_ignored(*, logs_dir="logs", record_fn=None):
    """Expire pending matches with shown_count >= 5."""
    data = _load_adjudications(logs_dir)
    expired = 0

    by_project = data.get("by_project", {})
    for _project, matches in by_project.items():
        for match in matches:
            if match.get("status") == "pendiente" and match.get("shown_count", 0) >= 5:
                match["status"] = "expirada"
                expired += 1
                _record_outcome(
                    record_fn, REWARD_BLANDO, pattern="loop_propuestas", source="soft_reject"
                )

    if expired:
        _save_adjudications(data, logs_dir)

    return {"expired": expired}
