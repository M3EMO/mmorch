"""cascade + Thompson bandit: el bandit elige el umbral de escalada por paso.
call() mockeado (sin API real)."""
import sys, pathlib, random, importlib
from dataclasses import dataclass
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
C = importlib.import_module("mmorch.cascade")  # el modulo, no la fn (shadow en __init__)
from mmorch.feedback import ThompsonBandit


@dataclass
class _FakeRes:
    text: str
    cost_usd: float = 0.0


def _fake_call_conf(conf):
    def _call(model, messages, **kw):
        return _FakeRes(text=f"respuesta de {model}.\nCONFIDENCE: {conf}")
    return _call


def test_bandit_picks_low_threshold_arm(monkeypatch, tmp_path):
    # call siempre devuelve conf=0.6.
    monkeypatch.setattr(C, "call", _fake_call_conf(0.6))
    b = ThompsonBandit(path=tmp_path / "bandit.json")
    # Entrenar: umbral 0.5 siempre buen reward, 0.9 siempre malo.
    for _ in range(30):
        b.update("m@0.5", 1.0)
        b.update("m@0.9", 0.0)
    rng = random.Random(7)
    res = C.cascade("q", steps=[("m", 0.7)],
                    thr_candidates={0: [0.5, 0.9]}, bandit=b, rng=rng)
    # Elige m@0.5 (thr 0.5 <= conf 0.6) -> resuelve barato sin escalar.
    assert res.arm == "m@0.5"
    assert res.resolved_step == 0 and res.escalate is False
    assert res.arms == ["m@0.5"]


def test_no_bandit_uses_fixed_threshold(monkeypatch, tmp_path):
    monkeypatch.setattr(C, "call", _fake_call_conf(0.6))
    # Sin bandit: umbral fijo 0.9 > conf 0.6 -> escala/agota.
    res = C.cascade("q", steps=[("m", 0.9)])
    assert res.escalate is True and res.arm == "m@0.9"


def test_close_loop_update(monkeypatch, tmp_path):
    # El lazo se cierra afuera: caller hace bandit.update(arm, reward).
    monkeypatch.setattr(C, "call", _fake_call_conf(0.95))
    b = ThompsonBandit(path=tmp_path / "bandit.json")
    res = C.cascade("q", steps=[("m", 0.7)],
                    thr_candidates={0: [0.7]}, bandit=b)
    b.update(res.arm, 1.0)  # label llega despues
    assert b.stats()[res.arm]["n"] == 1
