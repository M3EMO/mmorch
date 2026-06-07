"""feedback loop: outcome logging + Thompson bandit + calibracion (ECE)."""
import sys, pathlib, random
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import mmorch.feedback as FB


def test_record_and_read_outcomes(tmp_path):
    p = tmp_path / "fb.jsonl"
    FB.record_outcome("deepseek-chat", 1.0, pattern="cascade", predicted_conf=0.8,
                      source="test", path=p)
    FB.record_outcome("gemini-2.5-flash", 0.0, pattern="verify", path=p)
    ev = FB.read_outcomes(p)
    assert len(ev) == 2 and ev[0]["arm"] == "deepseek-chat" and ev[0]["reward"] == 1.0


def test_reward_clamped(tmp_path):
    p = tmp_path / "fb.jsonl"
    o = FB.record_outcome("x", 5.0, path=p)
    assert o.reward == 1.0
    o2 = FB.record_outcome("x", -2.0, path=p)
    assert o2.reward == 0.0


def test_bandit_learns_best_arm(tmp_path):
    b = FB.ThompsonBandit(path=tmp_path / "bandit.json")
    # A siempre acierta, B siempre falla.
    for _ in range(20):
        b.update("A", 1.0)
        b.update("B", 0.0)
    rng = random.Random(42)
    picks = [b.select(["A", "B"], rng=rng) for _ in range(50)]
    assert picks.count("A") >= 45  # explota el mejor casi siempre
    st = b.stats()
    assert st["A"]["mean"] > 0.9 and st["B"]["mean"] < 0.1
    assert st["A"]["n"] == 20


def test_bandit_persists(tmp_path):
    p = tmp_path / "bandit.json"
    b = FB.ThompsonBandit(path=p)
    b.update("A", 1.0)
    b2 = FB.ThompsonBandit(path=p)  # re-load desde disco
    assert b2.stats()["A"]["n"] == 1


def test_calibration_well_vs_over_confident(tmp_path):
    # Bien calibrado: conf 0.9 y acierta 90% -> ECE bajo.
    p = tmp_path / "fb.jsonl"
    for i in range(10):
        FB.record_outcome("m", 1.0 if i < 9 else 0.0, predicted_conf=0.9, path=p)
    c = FB.calibration(p)
    assert c["ece"] is not None and c["ece"] < 0.05 and c["by_arm"]["m"]["accuracy"] == 0.9

    # Overconfident: conf 0.95 pero acierta 30% -> ECE alto.
    p2 = tmp_path / "fb2.jsonl"
    for i in range(10):
        FB.record_outcome("m", 1.0 if i < 3 else 0.0, predicted_conf=0.95, path=p2)
    c2 = FB.calibration(p2)
    assert c2["ece"] > 0.5  # mal calibrado
