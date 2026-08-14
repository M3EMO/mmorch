"""Outcome recording and expiry for proposals."""

import json
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
    return load_json_tolerant(path)


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


def sweep_transcript(transcript_path, *, logs_dir="logs", record_fn=None):
    """Sweep a transcript for card patterns and verdicts."""
    cards_seen = 0
    verdicts = 0
    pending_card = None

    try:
        with open(transcript_path, "r", encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                try:
                    msg = json.loads(line)
                except json.JSONDecodeError:
                    continue

                content = msg.get("message", {}).get("content")
                if isinstance(content, list):
                    text = ""
                    for block in content:
                        if block.get("type") == "text":
                            text += block.get("text", "")
                elif isinstance(content, str):
                    text = content
                else:
                    continue

                if pending_card is None:
                    card_match = _CARD_RE.search(text)
                    if card_match:
                        note_filename = card_match.group(1)
                        _project = card_match.group(2)
                        pending_card = (note_filename, _project)
                        cards_seen += 1
                else:
                    note_filename, _project = pending_card
                    stripped = text.strip()
                    if stripped == "dale" or stripped == "no":
                        data = _load_adjudications(logs_dir)
                        match = None
                        by_project = data.get("by_project", {})
                        for _proj, matches in by_project.items():
                            for m in matches:
                                note_path = m.get("note_path", "")
                                if note_path.endswith(note_filename):
                                    match = m
                                    break
                            if match:
                                break
                        if match:
                            result = record_verdict(
                                match["id"], stripped, logs_dir=logs_dir, record_fn=record_fn
                            )
                            if result["recorded"]:
                                verdicts += 1
                        pending_card = None
    except (OSError, IOError):
        pass

    return {"cards_seen": cards_seen, "verdicts": verdicts}
