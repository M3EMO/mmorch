"""hillclimb — rubric escalar como entorno (medir->proponer->probar->repetir),
mejora monotona del best, y cierre del feedback loop (rubric = reward objetivo,
NO la conf auto-reportada)."""
import sys, pathlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import json
import random


from mmorch.hillclimb import hillclimb
from mmorch.feedback import ThompsonBandit


def seq_proposer(cands):
    """propose que entrega candidatos en orden; None al agotarse."""
    it = iter(cands)

    def propose(ctx):
        return next(it, None)

    return propose


# ---- control de loop ----
def test_adopts_best_and_stops_on_target():
    scores = {"a": 0.1, "b": 0.5, "c": 0.9, "d": 0.95}
    r = hillclimb(seq_proposer(["a", "b", "c", "d"]), scores.__getitem__,
                  target=0.9, max_rounds=10, patience=5)
    assert r.best == "c" and r.best_score == 0.9
    assert r.stopped == "target" and r.rounds == 3
    assert len(r.history) == 3


def test_patience_stops_after_dry_rounds():
    scores = {"a": 0.5, "b": 0.4, "c": 0.3, "d": 0.6}
    r = hillclimb(seq_proposer(["a", "b", "c", "d"]), scores.__getitem__,
                  max_rounds=10, patience=2)
    # b y c no mejoran -> 2 rondas secas seguidas -> corta sin ver d.
    assert r.stopped == "patience" and r.rounds == 3
    assert r.best == "a" and r.best_score == 0.5


def test_max_rounds_caps_loop():
    scores = {"a": 0.1, "b": 0.2, "c": 0.3, "d": 0.4}
    r = hillclimb(seq_proposer(["a", "b", "c", "d"]), scores.__getitem__,
                  max_rounds=3, patience=5)
    assert r.stopped == "max_rounds" and r.rounds == 3
    assert r.best == "c"


def test_propose_none_stops_explicitly():
    scores = {"a": 0.3}
    r = hillclimb(seq_proposer(["a"]), scores.__getitem__, max_rounds=10, patience=5)
    assert r.stopped == "no_candidate" and r.best == "a"


def test_score_exception_is_failed_round_not_crash():
    # candidato invalido (rubric explota) = ronda fallida, NO mata el loop.
    def score(c):
        if c == "bad":
            raise ValueError("no parsea")
        return {"a": 0.5, "c": 0.8}[c]

    r = hillclimb(seq_proposer(["a", "bad", "c"]), score, max_rounds=10, patience=5)
    assert r.history[1].score is None and r.history[1].improved is False
    assert r.best == "c" and r.best_score == 0.8


def test_minimize_mode():
    scores = {"a": 1.0, "b": 0.5, "c": 0.1}
    r = hillclimb(seq_proposer(["a", "b", "c"]), scores.__getitem__,
                  maximize=False, target=0.1, max_rounds=10, patience=5)
    assert r.best == "c" and r.stopped == "target"


def test_min_delta_ignores_noise():
    scores = {"a": 0.5, "b": 0.5005, "c": 0.5006}
    r = hillclimb(seq_proposer(["a", "b", "c"]), scores.__getitem__,
                  min_delta=0.01, max_rounds=10, patience=2)
    # mejoras < min_delta no cuentan: best queda en a y corta por patience.
    assert r.best == "a" and r.stopped == "patience"


def test_initial_sets_baseline():
    scores = {"base": 0.8, "a": 0.5, "b": 0.6}
    r = hillclimb(seq_proposer(["a", "b"]), scores.__getitem__,
                  initial="base", max_rounds=10, patience=5)
    assert r.baseline == 0.8
    assert r.best == "base" and r.best_score == 0.8  # nada lo supero


# ---- feedback wiring: rubric = reward (cierra el lazo sin label humano) ----
def test_fixed_arm_records_outcomes_and_updates_bandit(tmp_path):
    fpath = tmp_path / "feedback.jsonl"
    bandit = ThompsonBandit(tmp_path / "bandit.json")
    scores = {"a": 0.3, "b": 0.6, "c": 0.5}
    hillclimb(seq_proposer(["a", "b", "c"]), scores.__getitem__,
              max_rounds=3, patience=5, arm="deepseek-chat#climb",
              bandit=bandit, outcome_path=fpath)
    lines = [json.loads(ln) for ln in fpath.read_text(encoding="utf-8").splitlines()]
    assert [e["reward"] for e in lines] == [1.0, 1.0, 0.0]  # a mejora, b mejora, c no
    assert all(e["source"] == "rubric" for e in lines)
    assert all(e["pattern"] == "hillclimb" for e in lines)
    st = bandit.stats()["deepseek-chat#climb"]
    assert st["n"] == 3 and st["mean"] == round(3 / 5, 4)  # Beta(1+2, 1+1)


def test_arms_selected_via_thompson(tmp_path):
    fpath = tmp_path / "feedback.jsonl"
    bandit = ThompsonBandit(tmp_path / "bandit.json")
    seen_arms = []
    scores = iter([0.1, 0.2, 0.3, 0.4])

    def propose(ctx):
        seen_arms.append(ctx.arm)
        return object()

    r = hillclimb(propose, lambda c: next(scores), max_rounds=4, patience=5,
                  arms=["m1", "m2"], bandit=bandit, outcome_path=fpath,
                  rng=random.Random(42))
    assert r.rounds == 4
    assert all(a in ("m1", "m2") for a in seen_arms)
    total_n = sum(s["n"] for s in bandit.stats().values())
    assert total_n == 4  # cada ronda actualizo el brazo elegido


def test_no_arm_no_recording(tmp_path):
    fpath = tmp_path / "feedback.jsonl"
    scores = {"a": 0.3}
    hillclimb(seq_proposer(["a"]), scores.__getitem__,
              max_rounds=2, patience=5, outcome_path=fpath)
    assert not fpath.exists()


def test_journal_appends_jsonl_per_round(tmp_path):
    """qrf: journal_path appendea un JSONL por ronda (ledger autoresearch)."""
    jp = tmp_path / "exp.jsonl"
    scores = {"a": 0.1, "b": 0.5, "c": 0.9}
    hillclimb(seq_proposer(["a", "b", "c"]), scores.__getitem__,
                  max_rounds=3, patience=5, journal_path=jp)
    lines = [json.loads(l) for l in jp.read_text(encoding="utf-8").splitlines() if l.strip()]
    assert len(lines) == 3                                   # una entrada por ronda
    assert [x["round"] for x in lines] == [1, 2, 3]
    assert lines[0]["candidate"] == "'a'" and lines[2]["improved"] is True
    assert lines[2]["score"] == 0.9


def test_journal_off_by_default(tmp_path):
    """Sin journal_path -> no escribe nada (comportamiento de siempre)."""
    jp = tmp_path / "none.jsonl"
    hillclimb(seq_proposer(["a"]), {"a": 1.0}.__getitem__, max_rounds=1)
    assert not jp.exists()
