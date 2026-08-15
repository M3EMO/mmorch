"""Tests del hardening loop (fakes inyectados, sin engine ni API real)."""

import json

from mmorch.hardening import load_last_map, pick_target


def test_load_last_map_finds_latest_bughunt(tmp_path):
    p = tmp_path / "nightly.jsonl"
    lines = [
        json.dumps({"ts": 1, "bughunt": {"worst": [{"module": "a.py", "survived": 3,
                                                    "mutants": 8}]}}),
        json.dumps({"ts": 2}),  # noche sin caza
        "linea corrupta",
    ]
    p.write_text("\n".join(lines), encoding="utf-8")
    assert load_last_map(str(tmp_path))[0]["module"] == "a.py"


def test_load_last_map_empty_without_file(tmp_path):
    assert load_last_map(str(tmp_path)) == []


def test_pick_target_worst_ratio_first():
    worst = [
        {"module": "a.py", "survived": 2, "mutants": 8},   # 25%
        {"module": "b.py", "survived": 7, "mutants": 8},   # 87%
        {"module": "c.py", "survived": 0, "mutants": 8},   # limpio
    ]
    t = pick_target(worst, {}, today="2026-08-15")
    assert t["module"] == "b.py"


def test_pick_target_skips_recent_attempts():
    worst = [{"module": "b.py", "survived": 7, "mutants": 8},
             {"module": "a.py", "survived": 2, "mutants": 8}]
    state = {"b.py": {"retry_after": "2026-08-20"}}
    t = pick_target(worst, state, today="2026-08-15")
    assert t["module"] == "a.py"
    # pasada la ventana, b.py vuelve a ser elegible
    t2 = pick_target(worst, state, today="2026-08-21")
    assert t2["module"] == "b.py"


def test_pick_target_none_when_all_clean_or_recent():
    worst = [{"module": "c.py", "survived": 0, "mutants": 8}]
    assert pick_target(worst, {}, today="2026-08-15") is None
