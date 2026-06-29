"""B2 ensemble_degraded (verificadores 1-familia) + B3 budget_status MCP tool."""
import sys, pathlib, json
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import mmorch.ensemble as EN
from mmorch.patterns import Verdict


def _fake_av(monkeypatch):
    # adversarial_verify mockeado: pasa siempre, sin API
    monkeypatch.setattr(EN, "adversarial_verify",
                        lambda art, **k: Verdict(passed=True, confidence=0.9, refutations=[],
                                                 raw="", verifier_model=k.get("verifier_model", "?"),
                                                 cost_usd=0.0))


def test_ensemble_degraded_true_for_same_family(monkeypatch):
    _fake_av(monkeypatch)
    ev = EN.ensemble_verify("x", rubric="r", gen_model="deepseek-chat",
                            verifier_models=["gemini-2.5-flash", "gemini-2.5-flash-lite"])
    assert ev.ensemble_degraded is True   # ambos google -> no decorrelaciona entre verificadores


def test_ensemble_not_degraded_with_two_families(monkeypatch):
    _fake_av(monkeypatch)
    # google + moonshot = 2 familias (kimi inactivo pero family_of lo conoce)
    ev = EN.ensemble_verify("x", rubric="r", gen_model="deepseek-chat",
                            verifier_models=["gemini-2.5-flash", "kimi-k2.5"])
    assert ev.ensemble_degraded is False


def test_default_ensemble_is_degraded(monkeypatch):
    _fake_av(monkeypatch)
    ev = EN.ensemble_verify("x", rubric="r", gen_model="deepseek-chat")
    assert ev.ensemble_degraded is True   # el default (2 google) es homogeneo — honesto


def test_budget_status_tool_matches_module(monkeypatch):
    import mcp_server
    from mmorch.budget import status as budget_status
    # sin limite -> enforced False
    monkeypatch.delenv("MMORCH_MAX_MONTHLY_USD", raising=False)
    out = json.loads(mcp_server.mmorch_budget_status())
    assert out == budget_status()
    assert out["enforced"] is False and "remaining" in out and "spent" in out


def test_budget_status_enforced_toggles(monkeypatch):
    import mcp_server
    monkeypatch.setenv("MMORCH_MAX_MONTHLY_USD", "100")
    out = json.loads(mcp_server.mmorch_budget_status())
    assert out["enforced"] is True and out["limit"] == 100.0
