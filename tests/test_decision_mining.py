"""Tests de decision_mining (transcripts sinteticos en formato real de Claude Code)."""

import json

from mmorch.decision_mining import ingest_decisions, mine_decisions


def _user(text):
    return json.dumps({"type": "user", "message": {"content": text}})


def _assistant(text):
    return json.dumps({"type": "assistant",
                       "message": {"content": [{"type": "text", "text": text}]}})


def write_transcript(tmp_path, lines, name="t.jsonl"):
    p = tmp_path / name
    p.write_text("\n".join(lines) + "\nlinea corrupta\n", encoding="utf-8")
    return str(p)


def test_question_plus_short_decision_is_mined(tmp_path):
    t = write_transcript(tmp_path, [
        _user("arranca"),
        _assistant("¿Cual de las dos opciones? 1. spec 2. codigo"),
        _user("1"),
    ])
    pairs = mine_decisions(t)
    assert len(pairs) == 1
    assert "opciones" in pairs[0]["question"]
    assert pairs[0]["decision"] == "1"


def test_long_answer_not_mined(tmp_path):
    t = write_transcript(tmp_path, [
        _user("arranca"),
        _assistant("¿Como lo encaramos?"),
        _user("x" * 300),
    ])
    assert mine_decisions(t) == []


def test_no_question_no_pair(tmp_path):
    t = write_transcript(tmp_path, [
        _user("arranca"),
        _assistant("Listo, commiteado sin novedades"),
        _user("dale"),
    ])
    assert mine_decisions(t) == []


def test_ingest_dedups_across_runs(tmp_path):
    t = write_transcript(tmp_path, [
        _user("arranca"),
        _assistant("¿Va la opcion A?"),
        _user("dale"),
    ])
    logs = tmp_path / "logs"
    r1 = ingest_decisions(t, logs_dir=str(logs))
    assert r1 == {"mined": 1, "new": 1}
    r2 = ingest_decisions(t, logs_dir=str(logs))
    assert r2 == {"mined": 1, "new": 0}
    lines = (logs / "decision_samples.jsonl").read_text(encoding="utf-8").splitlines()
    assert len(lines) == 1 and "hash" in json.loads(lines[0])


def test_missing_transcript_returns_zeros(tmp_path):
    assert mine_decisions(str(tmp_path / "no.jsonl")) == []
    assert ingest_decisions(str(tmp_path / "no.jsonl"),
                            logs_dir=str(tmp_path / "logs")) == {"mined": 0, "new": 0}


def test_secret_redacted_in_file(tmp_path):
    secret = "sk-" + "a1b2c3d4e5" * 4
    t = write_transcript(tmp_path, [
        _user("arranca"),
        _assistant("¿Uso esta key?"),
        _user(f"dale, usa {secret}"),
    ])
    logs = tmp_path / "logs"
    ingest_decisions(t, logs_dir=str(logs))
    content = (logs / "decision_samples.jsonl").read_text(encoding="utf-8")
    assert secret not in content


def test_tail_corta_en_borde_legible_y_respeta_tope():
    from mmorch.decision_mining import _tail
    assert _tail("corto", 1200) == "corto"
    assert _tail("xx mitad. Frase limpia que sigue aca", 30).startswith("Frase limpia")
    assert len(_tail("x" * 3000, 100)) <= 100          # el tope nunca crece
    # tabla sin cabecera: las filas no significan nada, se tiran
    t = "basura cortada\n\n| a | b |\n| 1 | 2 |\nprosa que sigue"
    assert _tail(t, 34) == "prosa que sigue"
    # ...salvo que la muestra sea toda tabla (mejor eso que vacio)
    assert _tail("xx\n\n| a | b |\n| 1 | 2 |", 20).startswith("| a |")
