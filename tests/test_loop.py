"""loop_until_done: dedup contra seen, dry-streak, done explicito, max_rounds."""
import sys, pathlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
from mmorch.loop import loop_until_done


def test_stops_on_dry_streak():
    # ronda1: [a,b], ronda2: [b] (0 nuevo), ronda3: [b] (0 nuevo) -> dry patience=2.
    batches = {1: ["a", "b"], 2: ["b"], 3: ["b"], 4: ["c"]}
    r = loop_until_done(lambda n: batches[n], patience=2, max_rounds=10)
    assert r.stopped == "dry" and r.rounds == 3
    assert set(r.items) == {"a", "b"} and r.new_per_round == [2, 0, 0]


def test_dedup_against_all_seen():
    # 'a' reaparece pero no se cuenta dos veces.
    batches = {1: ["a"], 2: ["b", "a"], 3: ["a", "b"], 4: ["a"]}
    r = loop_until_done(lambda n: batches[n], patience=2)
    assert r.items == ["a", "b"] and r.stopped == "dry"


def test_explicit_done():
    def step(n):
        return ["x"] if n == 1 else None
    r = loop_until_done(step)
    assert r.stopped == "explicit_done" and r.items == ["x"] and r.rounds == 2


def test_max_rounds_cap():
    # siempre trae algo nuevo -> nunca seca -> corta en max_rounds.
    r = loop_until_done(lambda n: [n], patience=2, max_rounds=3)
    assert r.stopped == "max_rounds" and r.rounds == 3 and r.items == [1, 2, 3]


def test_key_fn_dedups_by_field():
    batches = {1: [{"id": 1, "v": "a"}], 2: [{"id": 1, "v": "b"}], 3: [{"id": 1, "v": "c"}]}
    r = loop_until_done(lambda n: batches[n], key=lambda d: d["id"], patience=2)
    assert len(r.items) == 1 and r.stopped == "dry"
