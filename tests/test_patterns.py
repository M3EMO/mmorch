"""Invariantes de patterns: OneFlow, anti-sicofancia, fan_out graceful. API mockeada."""
import sys, pathlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import pytest
import mmorch.patterns as P
from mmorch.patterns import (fan_out, adversarial_verify, _parse_verdict,
                             _coerce_passed, _coerce_conf)
from mmorch.providers import CallResult


def _fake_result(text="ok", model="deepseek-chat", family="deepseek"):
    return CallResult(model, family, text, 1, 1, 0.0, 0.0)


# ---- OneFlow (§4/§7) ----
def test_adversarial_verify_rejects_same_family(monkeypatch):
    # gen deepseek-chat + verifier deepseek-reasoner = misma familia -> raise.
    with pytest.raises(ValueError, match="OneFlow"):
        adversarial_verify("x", rubric="r", gen_model="deepseek-chat",
                           verifier_model="deepseek-reasoner")


def test_adversarial_verify_crossfamily_ok(monkeypatch):
    monkeypatch.setattr(P, "call",
        lambda *a, **k: _fake_result('{"passed":true,"confidence":0.9,"refutations":[]}',
                                     "gemini-2.5-flash", "google"))
    v = adversarial_verify("x", rubric="r")  # deepseek gen vs gemini verifier
    assert v.passed is True and v.confidence == 0.9


# ---- Anti-sicofancia / parse robusto (H-5) ----
def test_passed_string_false_is_false():
    assert _coerce_passed("false") is False
    assert _coerce_passed("FALSE") is False
    assert _coerce_passed("true") is True
    assert _coerce_passed(True) is True


def test_confidence_clamped():
    assert _coerce_conf(5.0) == 1.0
    assert _coerce_conf(-1.0) == 0.0
    assert _coerce_conf("x") == 0.0


def test_parse_fenced_json_with_string_false():
    p, c, r = _parse_verdict('```json\n{"passed":"false","confidence":2.0,"refutations":["a"]}\n```')
    assert p is False and c == 1.0 and r == ["a"]


def test_parse_unparseable_fails_closed():
    p, c, r = _parse_verdict("no json here")
    assert p is False and c == 0.0 and r  # skeptic default = refute


# ---- fan_out graceful (H-1) ----
def test_fan_out_one_failure_keeps_others(monkeypatch):
    calls = {"n": 0}
    def flaky(model, msgs, **k):
        calls["n"] += 1
        # segundo prompt falla
        if "BOOM" in msgs[-1]["content"]:
            raise RuntimeError("api down")
        return _fake_result("good")
    monkeypatch.setattr(P, "call", flaky)
    res = fan_out(["ok1", "BOOM", "ok3"])
    # 1 fallo NO mata el batch: quedan los 2 exitosos.
    assert len(res) == 2
    assert all(r.text == "good" for r in res)


def test_adversarial_verify_logs_verdict(monkeypatch):
    # Gap cerrado: adversarial_verify DEBE loggear un evento verdict (passed/confidence).
    events = []
    monkeypatch.setattr(P, "log_event", lambda **r: events.append(r))
    monkeypatch.setattr(P, "call",
        lambda *a, **k: _fake_result('{"passed":false,"confidence":0.3,"refutations":["x"]}',
                                     "gemini-2.5-flash", "google"))
    adversarial_verify("art", rubric="r")
    verdicts = [e for e in events if e.get("pattern") == "adversarial_verify_verdict"]
    assert len(verdicts) == 1
    assert verdicts[0]["passed"] is False and verdicts[0]["confidence"] == 0.3
    assert verdicts[0]["n_refutations"] == 1
