"""ablation §18.4: accuracy del verificador vs labels de verdad. call() mockeado."""
import sys, pathlib, importlib
from dataclasses import dataclass
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
A = importlib.import_module("mmorch.ablation")


@dataclass
class _Res:
    text: str
    cost_usd: float = 0.001
    latency_s: float = 1.0


def _cases():
    return [
        A.AblationCase("2+2=4", "es correcto?", True, "arit-ok"),
        A.AblationCase("2+2=5", "es correcto?", False, "arit-bug"),
    ]


def test_perfect_verifier(monkeypatch):
    # verifier que acierta: passed=True para el correcto, False para el bug.
    def _c(model, messages, **kw):
        art = messages[-1]["content"]
        good = "2+2=4" in art
        return _Res('{"passed": true, "confidence": 1}' if good
                    else '{"passed": false, "confidence": 1, "refutations": ["mal"]}')
    monkeypatch.setattr(A, "call", _c)
    r = A.run_ablation(_cases(), ["gemini-2.5-flash"], author_model="deepseek-chat")
    cfg = r["configs"][0]
    assert cfg.accuracy == 1.0 and cfg.cross_family is True
    assert cfg.false_pass == 0 and cfg.false_refute == 0


def test_sycophant_verifier_false_pass(monkeypatch):
    # verifier que SIEMPRE aprueba (sicofante): deja pasar el bug -> false_pass.
    monkeypatch.setattr(A, "call", lambda *a, **k: _Res('{"passed": true, "confidence": 1}'))
    r = A.run_ablation(_cases(), ["gemini-2.5-flash"])
    cfg = r["configs"][0]
    assert cfg.accuracy == 0.5 and cfg.false_pass == 1 and cfg.false_refute == 0


def test_same_family_no_guard(monkeypatch):
    # same-family NO debe tirar (la ablacion bypassa OneFlow a proposito).
    monkeypatch.setattr(A, "call", lambda *a, **k: _Res('{"passed": true, "confidence": 1}'))
    r = A.run_ablation(_cases(), ["deepseek-reasoner"], author_model="deepseek-chat")
    cfg = r["configs"][0]
    assert cfg.cross_family is False and cfg.n == 2


def test_ranking_best_first(monkeypatch):
    # dos verifiers: gemini perfecto, deepseek sicofante -> gemini primero.
    def _c(model, messages, **kw):
        if "gemini" in model:
            good = "2+2=4" in messages[-1]["content"]
            return _Res('{"passed": true}' if good else '{"passed": false}')
        return _Res('{"passed": true}')  # deepseek aprueba todo
    monkeypatch.setattr(A, "call", _c)
    r = A.run_ablation(_cases(), ["deepseek-reasoner", "gemini-2.5-flash"])
    assert r["configs"][0].verifier_model == "gemini-2.5-flash"
    assert r["configs"][0].accuracy > r["configs"][1].accuracy
