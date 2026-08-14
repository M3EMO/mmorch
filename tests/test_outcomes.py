"""Tests F3 outcome (contrato .scratch/loop-cerrado/spec.md)."""

import json

import pytest

from mmorch.iohelpers import atomic_write_json
from mmorch.outcomes import (
    ARM,
    REWARD_BLANDO,
    REWARD_DALE,
    REWARD_NO,
    expire_ignored,
    record_verdict,
    sweep_transcript,
)


def match(project="proj1", status="pendiente", shown=0, note="note1.md", pid=None):
    return {"note_path": f"/vault/{note}", "project": project, "score": 0.9,
            "justification": "aplica", "cited_file": None, "strong": True,
            "status": status, "shown_count": shown, "card": "tarjeta",
            "id": pid or f"/vault/{note}|{project}"}


def write_state(logs_dir, matches):
    logs_dir.mkdir(parents=True, exist_ok=True)
    by_project = {}
    for m in matches:
        by_project.setdefault(m["project"], []).append(m)
    atomic_write_json(logs_dir / "adjudications.json",
                      {"pairs": {}, "by_project": by_project})


def read_state(logs_dir):
    return json.loads((logs_dir / "adjudications.json").read_text(encoding="utf-8"))


class RecorderFake:
    def __init__(self):
        self.calls = []

    def __call__(self, arm, reward, *, pattern="", source=""):
        self.calls.append({"arm": arm, "reward": reward, "pattern": pattern,
                           "source": source})


def test_dale_marks_aceptada_reward_1(tmp_path):
    logs = tmp_path / "logs"
    m = match()
    write_state(logs, [m])
    rec = RecorderFake()
    result = record_verdict(m["id"], "dale", logs_dir=str(logs), record_fn=rec)
    assert result == {"recorded": True, "status": "aceptada"}
    assert read_state(logs)["by_project"]["proj1"][0]["status"] == "aceptada"
    assert rec.calls == [{"arm": ARM, "reward": REWARD_DALE,
                          "pattern": "loop_propuestas", "source": "verdict"}]


def test_no_marks_rechazada_reward_0125(tmp_path):
    logs = tmp_path / "logs"
    m = match()
    write_state(logs, [m])
    rec = RecorderFake()
    result = record_verdict(m["id"], "no", logs_dir=str(logs), record_fn=rec)
    assert result == {"recorded": True, "status": "rechazada"}
    assert rec.calls[0]["reward"] == REWARD_NO


def test_invalid_verdict_raises(tmp_path):
    with pytest.raises(ValueError):
        record_verdict("x", "quizas", logs_dir=str(tmp_path / "logs"))


def test_second_call_is_idempotent(tmp_path):
    logs = tmp_path / "logs"
    m = match()
    write_state(logs, [m])
    rec = RecorderFake()
    record_verdict(m["id"], "dale", logs_dir=str(logs), record_fn=rec)
    result = record_verdict(m["id"], "dale", logs_dir=str(logs), record_fn=rec)
    assert result == {"recorded": False, "status": "aceptada"}
    assert len(rec.calls) == 1


def test_unknown_id(tmp_path):
    logs = tmp_path / "logs"
    write_state(logs, [match()])
    rec = RecorderFake()
    result = record_verdict("inexistente", "dale", logs_dir=str(logs), record_fn=rec)
    assert result == {"recorded": False, "status": None}
    assert rec.calls == []


def test_expire_ignored_only_shown5_pending(tmp_path):
    logs = tmp_path / "logs"
    write_state(logs, [
        match(note="a.md", shown=5),
        match(note="b.md", shown=2),
        match(note="c.md", shown=7, status="aceptada"),
    ])
    rec = RecorderFake()
    result = expire_ignored(logs_dir=str(logs), record_fn=rec)
    assert result == {"expired": 1}
    estados = {m["note_path"]: m["status"]
               for m in read_state(logs)["by_project"]["proj1"]}
    assert estados["/vault/a.md"] == "expirada"
    assert estados["/vault/b.md"] == "pendiente"
    assert estados["/vault/c.md"] == "aceptada"
    assert rec.calls == [{"arm": ARM, "reward": REWARD_BLANDO,
                          "pattern": "loop_propuestas", "source": "soft_reject"}]


def _tline(role, text):
    return json.dumps({"message": {"role": role,
                                   "content": [{"type": "text", "text": text}]}})


def write_transcript(tmp_path, lines):
    p = tmp_path / "transcript.jsonl"
    p.write_text("\n".join(lines) + "\nlinea invalida no-json\n", encoding="utf-8")
    return str(p)


CARD = "💡 mmorch: la nota note1.md aplica a proj1 — aplica al ingest."


def test_sweep_detects_card_and_dale(tmp_path):
    logs = tmp_path / "logs"
    write_state(logs, [match()])
    t = write_transcript(tmp_path, [_tline("assistant", CARD),
                                    _tline("user", "dale, arrancala")])
    rec = RecorderFake()
    result = sweep_transcript(t, logs_dir=str(logs), record_fn=rec)
    assert result == {"cards_seen": 1, "verdicts": 1}
    assert read_state(logs)["by_project"]["proj1"][0]["status"] == "aceptada"


def test_sweep_without_card_records_nothing(tmp_path):
    logs = tmp_path / "logs"
    write_state(logs, [match()])
    t = write_transcript(tmp_path, [_tline("user", "dale")])
    rec = RecorderFake()
    result = sweep_transcript(t, logs_dir=str(logs), record_fn=rec)
    assert result == {"cards_seen": 0, "verdicts": 0}
    assert rec.calls == []


def test_sweep_does_not_duplicate_already_accepted(tmp_path):
    logs = tmp_path / "logs"
    write_state(logs, [match(status="aceptada")])
    t = write_transcript(tmp_path, [_tline("assistant", CARD),
                                    _tline("user", "dale")])
    rec = RecorderFake()
    result = sweep_transcript(t, logs_dir=str(logs), record_fn=rec)
    assert result["verdicts"] == 0
    assert rec.calls == []


def test_sweep_ignores_non_user_verdict(tmp_path):
    logs = tmp_path / "logs"
    write_state(logs, [match()])
    t = write_transcript(tmp_path, [_tline("assistant", CARD),
                                    _tline("assistant", "dale")])
    rec = RecorderFake()
    result = sweep_transcript(t, logs_dir=str(logs), record_fn=rec)
    assert result["verdicts"] == 0


def test_sweep_missing_transcript(tmp_path):
    result = sweep_transcript(str(tmp_path / "nope.jsonl"),
                              logs_dir=str(tmp_path / "logs"))
    assert result == {"cards_seen": 0, "verdicts": 0}
