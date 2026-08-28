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
    assert 18 <= st["A"]["n"] <= 20  # n = muestra EFECTIVA: el discounted Thompson (decay 0.995) la achica levemente


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


# --- tests anti-mutantes (bug-hunt 2026-08-14: 6/12 sobrevivian) ------------

def test_sample_uses_default_rng(tmp_path):
    # mutante 'rng or Random()' -> 'rng and Random()': sample() sin rng explotaba
    b = FB.ThompsonBandit(path=tmp_path / "b.json")
    b.update("a", 1.0)
    assert b.select(["a", "b"]) in ("a", "b")


def test_calibrate_conf_gate_min_n(tmp_path):
    # mutante 'cell and n>=min_n' -> 'or': calibraria con datos insuficientes
    p = tmp_path / "f.jsonl"
    for _ in range(5):  # solo 5 muestras (< min_n=20) en el bucket de 0.95
        FB.record_outcome("m", 0.0, predicted_conf=0.95, path=p)
    # con datos insuficientes DEBE devolver la conf cruda, jamas la del bucket
    assert FB.calibrate_conf(0.95, path=p) == 0.95


def test_calibrate_conf_applies_at_exactly_min_n(tmp_path):
    # mata mutantes de frontera min_n 20->21 y bins 10->11
    p = tmp_path / "f.jsonl"
    for _ in range(20):  # exactamente min_n en el bucket 9 (conf 0.95, bins=10)
        FB.record_outcome("m", 0.0, predicted_conf=0.95, path=p)
    assert FB.calibrate_conf(0.95, path=p) == 0.0  # acc empirica del bucket


def test_reliability_bins_default_bucketing(tmp_path):
    # bins default = 10: conf 0.95 cae en el bucket 9; 11 bins lo moveria
    p = tmp_path / "f.jsonl"
    FB.record_outcome("m", 1.0, predicted_conf=0.95, path=p)
    m = FB.reliability_bins(p)
    assert list(m.keys()) == [9]


def test_calibration_default_bins_ece_value(tmp_path):
    # ECE con bins default: 10 outcomes conf 0.95 acc 0.0 -> ece == 0.95 exacto
    p = tmp_path / "f.jsonl"
    for _ in range(10):
        FB.record_outcome("m", 0.0, predicted_conf=0.95, path=p)
    c = FB.calibration(p)
    assert abs(c["ece"] - 0.95) < 1e-9
