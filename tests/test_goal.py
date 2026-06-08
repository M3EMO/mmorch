"""tests goal.py — ancla anti-goal-drift. Partes deterministas (sin API)."""
import pytest
from mmorch.goal import (load_goal, goal_hash, goal_aligned,
                         authorize_goal, goal_guard, pursue_goal, GoalTampered)
from mmorch import patterns
import mmorch.goal as G


def test_load_goal_has_contract():
    g = load_goal()
    assert "north star" in g.lower()
    assert "Invariantes" in g and "Non-goals" in g
    assert "ZONA ROJA" in g or "zona roja" in g.lower()


def test_goal_hash_stable():
    h1, h2 = goal_hash(), goal_hash()
    assert h1 == h2 and len(h1) == 16


def test_goal_aligned_embeds_goal_and_is_cross_family(monkeypatch):
    # capturamos la rúbrica sin pegarle a la API
    captured = {}

    def fake_verify(artifact, *, rubric, gen_model, verifier_model, phase, task_kind):
        captured.update(rubric=rubric, gen=gen_model, ver=verifier_model, kind=task_kind, art=artifact)
        return patterns.Verdict(True, 0.9, [], "ok", verifier_model, 0.0)

    monkeypatch.setattr(patterns, "adversarial_verify", fake_verify)
    # goal.py importó la función por nombre -> parchear ahí también
    import mmorch.goal as G
    monkeypatch.setattr(G, "adversarial_verify", fake_verify)

    v = goal_aligned("agregar un checker nuevo determinista")
    assert v.passed
    assert "north star" in captured["rubric"].lower()      # el GOAL va en la rúbrica
    assert "Invariantes" in captured["rubric"]
    assert captured["kind"] == "subjective"                  # subjetivo -> cross-family
    assert captured["art"] == "agregar un checker nuevo determinista"


def test_goal_guard_tamper_halt(tmp_path):
    g = tmp_path / "GOAL.md"
    h = tmp_path / "GOAL.hash"
    g.write_text("north star v1", encoding="utf-8")
    goal_guard(g, h)                       # init: autoriza el actual
    goal_guard(g, h)                       # sin cambios -> OK
    g.write_text("north star MANIPULADO", encoding="utf-8")
    with pytest.raises(GoalTampered):
        goal_guard(g, h)                   # cambió sin re-autorizar -> HALT
    authorize_goal(g, h)                   # humano re-autoriza
    goal_guard(g, h)                       # ahora OK


def test_pursue_goal_retries_until_aligned(monkeypatch):
    calls = {"n": 0}

    def fake_verify(artifact, *, rubric, gen_model, verifier_model, phase, task_kind):
        calls["n"] += 1
        ok = calls["n"] >= 2               # falla la 1ra, alinea la 2da
        return patterns.Verdict(ok, 0.9, [] if ok else ["deriva del norte"], "x", verifier_model, 0.0)

    monkeypatch.setattr(G, "adversarial_verify", fake_verify)
    seen = []

    def generate(feedback):
        seen.append(feedback)
        return f"cambio intento {len(seen)}"

    r = pursue_goal(generate, max_rounds=3)
    assert r["aligned"] and r["rounds"] == 2
    assert seen[0] is None and "Refutaciones" in seen[1]   # realimenta la refutación


def test_pursue_goal_gives_up(monkeypatch):
    monkeypatch.setattr(G, "adversarial_verify",
                        lambda *a, **k: patterns.Verdict(False, 0.5, ["no"], "x", "m", 0.0))
    r = pursue_goal(lambda fb: "x", max_rounds=2)
    assert not r["aligned"] and r["change"] is None
