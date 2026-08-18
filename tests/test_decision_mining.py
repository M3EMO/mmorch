"""Tests for mmorch.decision_mining."""

import json


from mmorch import decision_mining


def _write_transcript(tmp_path, lines):
    """Write a synthetic transcript JSONL file."""
    path = tmp_path / "transcript.jsonl"
    with open(path, "w", encoding="utf-8") as f:
        for line in lines:
            f.write(json.dumps(line) + "\n")
    return str(path)


def _msg(role, text):
    """Build a message dict in the sessions.py format."""
    return {
        "message": {
            "role": role,
            "content": [{"type": "text", "text": text}],
        }
    }


def test_question_short_decision_mined(tmp_path):
    """A question followed by a short user decision is mined."""
    path = _write_transcript(
        tmp_path,
        [
            _msg("assistant", "Which option do you prefer? 1. A 2. B"),
            _msg("user", "Option 1"),
        ],
    )
    result = decision_mining.mine_decisions(path)
    assert len(result) == 1
    assert "Which option" in result[0]["question"]
    assert result[0]["decision"] == "Option 1"
    assert result[0]["ts"] is None


def test_long_response_not_mined(tmp_path):
    """A user response longer than 240 chars is not mined."""
    long_text = "x" * 241
    path = _write_transcript(
        tmp_path,
        [
            _msg("assistant", "What should we do?"),
            _msg("user", long_text),
        ],
    )
    result = decision_mining.mine_decisions(path)
    assert result == []


def test_no_question_no_pair(tmp_path):
    """Text without a question produces no pair."""
    path = _write_transcript(
        tmp_path,
        [
            _msg("assistant", "Here is some info."),
            _msg("user", "OK"),
        ],
    )
    result = decision_mining.mine_decisions(path)
    assert result == []


def test_dedup_between_ingests(tmp_path):
    """Second ingest of the same transcript yields new=0."""
    path = _write_transcript(
        tmp_path,
        [
            _msg("assistant", "Which one? 1. A 2. B"),
            _msg("user", "A"),
        ],
    )
    logs_dir = tmp_path / "logs"
    first = decision_mining.ingest_decisions(path, logs_dir=str(logs_dir))
    assert first["mined"] == 1
    assert first["new"] == 1

    second = decision_mining.ingest_decisions(path, logs_dir=str(logs_dir))
    assert second["mined"] == 1
    assert second["new"] == 0


def test_nonexistent_transcript_returns_zeros(tmp_path):
    """Nonexistent transcript returns zeros."""
    result = decision_mining.ingest_decisions(
        str(tmp_path / "missing.jsonl"), logs_dir=str(tmp_path / "logs")
    )
    assert result == {"mined": 0, "new": 0}


def test_secret_redacted_in_file(tmp_path):
    """An obvious secret in the decision is redacted in the file."""
    path = _write_transcript(
        tmp_path,
        [
            _msg("assistant", "What is the key? 1. A 2. B"),
            _msg("user", "sk-abc123456789"),
        ],
    )
    logs_dir = tmp_path / "logs"
    decision_mining.ingest_decisions(path, logs_dir=str(logs_dir))

    log_file = logs_dir / "decision_samples.jsonl"
    with open(log_file, encoding="utf-8") as f:
        line = json.loads(f.readline())
    assert "sk-abc123456789" not in line["decision"]
    assert "sk-" in line["decision"]  # redacted but still present
