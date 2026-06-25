"""16u: ensemble multi-vista (Thousand Brains). Decorrelacion por lente + consenso por
consistencia mutua + guardrail anti-consenso-correlacionado. adversarial_verify stubeado
(cero API). gen=deepseek; verificadores rotan google/zhipu (cross-family + lens-diverse)."""
import sys, pathlib, importlib
from types import SimpleNamespace
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
E = importlib.import_module("mmorch.ensemble")

LENSES = [{"name": "correctness", "rubric": "es correcto?"},
          {"name": "security", "rubric": "es seguro?"}]


def _stub(monkeypatch, verdict_for):
    """verdict_for(rubric)->bool. Construye un Verdict-like minimo."""
    def fake(artifact, *, rubric, gen_model, verifier_model, phase=""):
        ok = verdict_for(rubric)
        return SimpleNamespace(passed=ok, confidence=0.9, cost_usd=0.001,
                               refutations=[] if ok else [f"fallo: {rubric}"])
    monkeypatch.setattr(E, "adversarial_verify", fake)


def test_all_lenses_pass_is_consensus(monkeypatch):
    _stub(monkeypatch, lambda r: True)
    v = E.multiview_verify("x", lenses=LENSES)
    assert v.passed is True and not v.escalate and not v.low_decorrelation
    assert {p["family"] for p in v.per_lens} == {"google", "zhipu"}   # 2 ejes: familia x lente


def test_all_lenses_fail(monkeypatch):
    _stub(monkeypatch, lambda r: False)
    v = E.multiview_verify("x", lenses=LENSES)
    assert v.passed is False and not v.escalate and v.refutations


def test_split_views_escalate(monkeypatch):
    # una lente pasa, otra falla -> ambiguedad genuina -> Opus
    _stub(monkeypatch, lambda r: "correcto" in r)
    v = E.multiview_verify("x", lenses=LENSES)
    assert v.passed is None and v.escalate and v.n_pass == 1


def test_single_lens_flags_low_decorrelation(monkeypatch):
    _stub(monkeypatch, lambda r: True)
    v = E.multiview_verify("x", lenses=[LENSES[0]])
    assert v.passed is True and v.low_decorrelation   # 1 lente -> acuerdo no confirma


def test_same_family_verifier_rejected(monkeypatch):
    _stub(monkeypatch, lambda r: True)
    try:
        E.multiview_verify("x", lenses=LENSES, gen_model="deepseek-chat",
                           verifier_models=["deepseek-reasoner"])
        assert False, "debio rechazar verifier misma familia que gen"
    except ValueError as e:
        assert "OneFlow" in str(e)


def test_empty_lenses_raises():
    try:
        E.multiview_verify("x", lenses=[])
        assert False
    except ValueError:
        pass
