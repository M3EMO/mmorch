"""Tests I-2..I-5: route, ensemble_verify, memo cache, innovate. API mockeada."""
import sys, pathlib, importlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import pytest

# NOTA: `import mmorch.route` devuelve la FUNCION route (el __init__ la importa y
# shadowea el submodulo del mismo nombre). import_module trae el MODULO real desde
# sys.modules, sin el shadow -> necesario para patchear route.call.
RT = importlib.import_module("mmorch.route")
import mmorch.ensemble as EN
import mmorch.cache as CA
import mmorch.innovate as IN
import mmorch.patterns as PAT
from mmorch.providers import CallResult
from mmorch.patterns import Verdict


def _cr(text):
    return CallResult("deepseek-chat", "deepseek", text, 1, 1, 0.0, 0.0)


# ---- I-2 route ----
def test_route_high_conf_no_escalate(monkeypatch):
    monkeypatch.setattr(RT, "call", lambda *a, **k: _cr("respuesta\nCONFIDENCE: 0.9"))
    r = RT.route("q")
    assert r.confidence == 0.9 and r.escalate is False and r.answer == "respuesta"


def test_route_low_conf_escalates(monkeypatch):
    monkeypatch.setattr(RT, "call", lambda *a, **k: _cr("dudosa\nCONFIDENCE: 0.4"))
    r = RT.route("q", threshold=0.7)
    assert r.escalate is True


def test_route_no_conf_defaults_mid_and_escalates(monkeypatch):
    monkeypatch.setattr(RT, "call", lambda *a, **k: _cr("sin score"))
    r = RT.route("q")  # 0.5 < 0.7
    assert r.confidence == 0.5 and r.escalate is True


# ---- I-3 ensemble_verify ----
def _v(passed, conf=0.8, refs=None):
    return Verdict(passed, conf, refs or [], "raw", "gemini-2.5-flash", 0.0)


def test_ensemble_majority_pass(monkeypatch):
    seq = iter([_v(True), _v(True), _v(False, refs=["x"])])
    monkeypatch.setattr(EN, "adversarial_verify", lambda *a, **k: next(seq))
    ev = EN.ensemble_verify("art", rubric="r",
                            verifier_models=["gemini-2.5-flash", "gemini-2.5-flash-lite", "gemini-2.5-flash"])
    assert ev.n_passed == 2 and ev.n_total == 3 and ev.passed is True


def test_ensemble_tie_fails(monkeypatch):
    seq = iter([_v(True), _v(False, refs=["x"])])
    monkeypatch.setattr(EN, "adversarial_verify", lambda *a, **k: next(seq))
    ev = EN.ensemble_verify("art", rubric="r",
                            verifier_models=["gemini-2.5-flash", "gemini-2.5-flash-lite"])
    assert ev.n_passed == 1 and ev.passed is False  # 1 de 2 = empate -> falla


def test_ensemble_rejects_same_family_verifier():
    with pytest.raises(ValueError, match="OneFlow"):
        EN.ensemble_verify("a", rubric="r", gen_model="deepseek-chat",
                           verifier_models=["deepseek-reasoner"])


# ---- I-4 memo cache ----
def test_memo_roundtrip(tmp_path):
    m = CA.Memo(path=tmp_path / "memo.json")
    m.put(CA.key_of("a", "b"), {"x": 1})
    m2 = CA.Memo(path=tmp_path / "memo.json")  # re-load from disk
    assert m2.get(CA.key_of("a", "b")) == {"x": 1}


def test_memoized_verify_hits_cache(tmp_path, monkeypatch):
    calls = {"n": 0}
    def fake_av(*a, **k):
        calls["n"] += 1
        return Verdict(True, 0.9, [], "raw", "gemini-2.5-flash", 0.0)
    monkeypatch.setattr(PAT, "adversarial_verify", fake_av)
    memo = CA.Memo(path=tmp_path / "memo.json")
    out1, c1 = CA.memoized_verify("art", "rub", memo=memo)
    out2, c2 = CA.memoized_verify("art", "rub", memo=memo)
    assert c1 is False and c2 is True and calls["n"] == 1  # 2do tiro = cache, sin API
    assert out2["passed"] is True


# ---- I-5 innovate ----
def test_ideate_and_screen(monkeypatch):
    monkeypatch.setattr(IN, "fan_out",
        lambda prompts, **k: [_cr(f"idea {i}") for i in range(len(prompts))])
    monkeypatch.setattr(IN, "adversarial_verify",
        lambda idea, **k: Verdict("idea 0" in idea, 0.7, ["obj"], "raw", "gemini-2.5-flash", 0.0))
    res = IN.ideate_and_screen("ctx", ["L1", "L2"], "ask", "rubric")
    assert len(res) == 2
    assert res[0].survives is True and res[1].survives is False
    assert res[1].objection == "obj"


# ---- cascade (FrugalGPT-style) ----
# Mismo shadow que route: el __init__ exporta la funcion `cascade` -> shadowea el
# submodulo. import_module trae el modulo real para patchear cascade.call.
CAS = importlib.import_module("mmorch.cascade")


def test_cascade_resolves_cheap_step(monkeypatch):
    monkeypatch.setattr(CAS, "call", lambda *a, **k: _cr("buena\nCONFIDENCE: 0.95"))
    r = CAS.cascade("q", steps=[("deepseek-chat", 0.7), ("gemini-2.5-flash", 0.85)])
    assert r.resolved_step == 0 and r.escalate is False and r.models_used == ["deepseek-chat"]


def test_cascade_escalates_through_steps(monkeypatch):
    seq = iter([_cr("dudosa\nCONFIDENCE: 0.4"), _cr("mejor\nCONFIDENCE: 0.9")])
    monkeypatch.setattr(CAS, "call", lambda *a, **k: next(seq))
    r = CAS.cascade("q", steps=[("deepseek-chat", 0.7), ("gemini-2.5-flash", 0.85)])
    assert r.resolved_step == 1 and r.escalate is False
    assert r.models_used == ["deepseek-chat", "gemini-2.5-flash"]


def test_cascade_exhausts_and_flags_opus(monkeypatch):
    monkeypatch.setattr(CAS, "call", lambda *a, **k: _cr("flojo\nCONFIDENCE: 0.3"))
    r = CAS.cascade("q", steps=[("deepseek-chat", 0.7), ("gemini-2.5-flash", 0.85)])
    assert r.escalate is True and len(r.models_used) == 2


def test_ensemble_minority_veto_stricter(monkeypatch):
    # 2 pass, 1 fail: mayoria -> pasa; min_veto=1 -> falla (un solo veto invalida).
    def mk(monkeypatch_seq):
        it = iter(monkeypatch_seq)
        return lambda *a, **k: next(it)
    monkeypatch.setattr(EN, "adversarial_verify", mk([_v(True), _v(True), _v(False, refs=["x"])]))
    ev_maj = EN.ensemble_verify("a", rubric="r",
        verifier_models=["gemini-2.5-flash", "gemini-2.5-flash-lite", "gemini-2.5-flash"])
    assert ev_maj.passed is True  # 2 de 3 = mayoria
    monkeypatch.setattr(EN, "adversarial_verify", mk([_v(True), _v(True), _v(False, refs=["x"])]))
    ev_veto = EN.ensemble_verify("a", rubric="r", min_veto=1,
        verifier_models=["gemini-2.5-flash", "gemini-2.5-flash-lite", "gemini-2.5-flash"])
    assert ev_veto.passed is False  # 1 veto -> invalida
