"""tournament: pairwise single-elimination, juez cross-family, empate->escalate.
call() mockeado."""
import sys, pathlib, importlib
from dataclasses import dataclass
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
T = importlib.import_module("mmorch.tournament")


@dataclass
class _Res:
    text: str
    cost_usd: float = 0.0


def test_oneflow_enforced():
    # juez misma familia que gen -> ValueError.
    import pytest
    with pytest.raises(ValueError):
        T.tournament(["a", "b"], criterion="x",
                     gen_model="deepseek-chat", judge_model="deepseek-reasoner")


def test_single_candidate_no_compare(monkeypatch):
    r = T.tournament(["solo"], criterion="x")
    assert r.winner == "solo" and r.rounds == 0 and r.comparisons == []


def test_picks_winner_always_A(monkeypatch):
    # juez siempre elige A (el de la izquierda).
    monkeypatch.setattr(T, "call", lambda *a, **k: _Res('{"winner":"A","reason":"mejor"}'))
    r = T.tournament(["x1", "x2", "x3", "x4"], criterion="cual gana")
    assert r.winner == "x1" and r.escalate is False
    assert r.rounds == 2  # 4 -> 2 -> 1


def test_tie_escalates(monkeypatch):
    monkeypatch.setattr(T, "call", lambda *a, **k: _Res('{"winner":"tie","reason":"iguales"}'))
    r = T.tournament(["a", "b"], criterion="x")
    assert r.winner is None and r.escalate is True


def test_odd_bye(monkeypatch):
    # juez siempre B; 3 candidatos: (a vs b)->b, c bye; ronda2 (b vs c)->c.
    monkeypatch.setattr(T, "call", lambda *a, **k: _Res('{"winner":"B","reason":"r"}'))
    r = T.tournament(["a", "b", "c"], criterion="x")
    assert r.winner == "c" and r.rounds == 2
