"""Tests de auto_repair (fakes, sin engine ni API)."""

import json

from mmorch.auto_repair import findings_from_record, pick_finding, _sig


def test_findings_extraction_all_shapes():
    rec = {
        "ts": 1,
        "evolve_error": "FileNotFoundError: gh",
        "hardening": {"error": "F821 hashlib"},
        "idea_loop": {"errors": ["adjudicate: boom"], "steps": {}},
        "project_health": {"errors": ["Portfolio: timeout"], "ok": []},
        "autoresearch": {"baseline": 0.8},  # sin error -> no aporta
    }
    f = findings_from_record(rec)
    detalles = {x["detail"] for x in f}
    assert "FileNotFoundError: gh" in detalles
    assert "F821 hashlib" in detalles
    assert "adjudicate: boom" in detalles
    assert "Portfolio: timeout" in detalles
    assert len(f) == 4


def test_pick_respects_retry_window():
    f1 = {"source": "evolve_error", "detail": "X" * 100}
    f2 = {"source": "hardening", "detail": "Y"}
    state = {_sig(f1): {"retry_after": "2026-08-25"}}
    picked = pick_finding([f1, f2], state, today="2026-08-19")
    assert picked["source"] == "hardening"
    # pasada la ventana vuelve a ser elegible
    assert pick_finding([f1, f2], state, today="2026-08-26")["source"] == "evolve_error"


def test_pick_none_when_all_recent():
    f1 = {"source": "a", "detail": "d"}
    assert pick_finding([f1], {_sig(f1): {"retry_after": "2099-01-01"}},
                        today="2026-08-19") is None


def test_sig_stable_despite_variable_tail():
    a = {"source": "s", "detail": "F821 hashlib en " + "x" * 100 + "path1"}
    b = {"source": "s", "detail": "F821 hashlib en " + "x" * 100 + "path2"}
    assert _sig(a) == _sig(b)  # el prefijo manda; los paths del final no


def test_repair_skips_without_record(tmp_path):
    from mmorch.auto_repair import repair
    (tmp_path / "logs").mkdir()
    r = repair(str(tmp_path), today="2026-08-19")
    assert r == {"skipped": "sin record nocturno"}


def test_repair_skips_paused(tmp_path):
    from mmorch.auto_repair import repair
    logs = tmp_path / "logs"
    logs.mkdir()
    (logs / "loop_paused").touch()
    (logs / "nightly.jsonl").write_text(json.dumps({"x_error": "boom"}) + "\n",
                                        encoding="utf-8")
    assert repair(str(tmp_path), today="2026-08-19") == {"skipped": "paused"}
