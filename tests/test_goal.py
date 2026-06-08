"""tests goal.py — ancla anti-goal-drift. Partes deterministas (sin API)."""
from mmorch.goal import load_goal, goal_hash, goal_aligned
from mmorch import patterns


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
