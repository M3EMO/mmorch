"""code_loop: cascade genera -> checker EJECUTA -> lazo cerrado (bandit + outcome con
context). call() mockeado; la ejecucion del sandbox es real (python_exec)."""
import sys, pathlib, random, importlib
from dataclasses import dataclass
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
CAS = importlib.import_module("mmorch.cascade")
CL = importlib.import_module("mmorch.code_loop")
import mmorch.shadow_prior as SP
from mmorch.feedback import ThompsonBandit

GOOD = "```python\ndef inc(x):\n    return x + 1\n```"
BAD = "```python\ndef inc(x):\n    return x - 1\n```"
TESTS = "assert inc(1) == 2\nassert inc(-1) == 0"


def _fake_call(text, conf=0.9):
    @dataclass
    class _R:
        text: str
        cost_usd: float = 0.0
    def _call(model, messages, **kw):
        return _R(text=f"{text}\nCONFIDENCE: {conf}")
    return _call


def test_good_code_closes_loop_with_reward_1(monkeypatch, tmp_path):
    monkeypatch.setattr(CAS, "call", _fake_call(GOOD))
    recorded = {}
    monkeypatch.setattr(CL, "record_outcome",
                        lambda arm, rew, **kw: recorded.update(arm=arm, reward=rew, **kw))
    b = ThompsonBandit(path=tmp_path / "b.json")
    r = CL.run_code_task("implement inc(x) = x+1", TESTS, steps=[("m", 0.5)],
                         bandit=b, thr_candidates={0: [0.5]})
    assert r.passed and r.reward == 1.0
    assert recorded["arm"] == "m@0.5" and recorded["reward"] == 1.0
    assert recorded["context"] == "implement inc(x) = x+1"   # el prior come contexto
    assert recorded["pattern"] == "code_loop" and recorded["source"] == "execution"
    assert b.stats()["m@0.5"]["n"] == 1                       # bandit actualizado


def test_bad_code_reward_0_self_conf_ignored(monkeypatch, tmp_path):
    # self-conf 0.95 ALTA pero el codigo falla -> reward 0 (anti-sicofancia: ejecucion manda)
    monkeypatch.setattr(CAS, "call", _fake_call(BAD, conf=0.95))
    recorded = {}
    monkeypatch.setattr(CL, "record_outcome",
                        lambda arm, rew, **kw: recorded.update(arm=arm, reward=rew, **kw))
    b = ThompsonBandit(path=tmp_path / "b.json")
    r = CL.run_code_task("implement inc(x) = x+1", TESTS, steps=[("m", 0.5)],
                         bandit=b, thr_candidates={0: [0.5]})
    assert not r.passed and r.reward == 0.0
    assert recorded["reward"] == 0.0 and recorded["predicted_conf"] == 0.95


def test_cascade_uses_prior_when_passed(monkeypatch, tmp_path):
    # prior con scale>0 y vecinos: contexto 'img' siempre gano con brazo m@0.5
    monkeypatch.setattr(CAS, "call", _fake_call(GOOD, conf=0.6))
    monkeypatch.setattr(CL, "record_outcome", lambda *a, **k: None)
    fake_embed = lambda t: [1.0, 0.0] if t.startswith("img") else [0.0, 1.0]
    prior = SP.ShadowPrior(scale=0.8, embed_fn=fake_embed)
    prior.index = {"m@0.5": [([1.0, 0.0], 1.0)] * 6, "m@0.9": [([1.0, 0.0], 0.0)] * 6}
    b = ThompsonBandit(path=tmp_path / "b.json")
    rng = random.Random(3)
    wins = 0
    for _ in range(20):
        r = CAS.cascade("img:task", steps=[("m", 0.7)], bandit=b,
                        thr_candidates={0: [0.5, 0.9]}, prior=prior, rng=rng)
        wins += (r.arms[0] == "m@0.5")
    # bandit puro seria ~50/50 (sin updates); el prior sesga fuerte hacia m@0.5
    assert wins >= 15
