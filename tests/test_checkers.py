"""tests checkers.py — tool-verify determinista (cero API, todo local)."""
import math
import pytest

from mmorch.checkers import check, safe_arith, UnsafeExpr, register_checker, available, CheckResult
from mmorch.patterns import adversarial_verify


def test_arithmetic_correct():
    assert check("arithmetic", expr="comb(20,10)", expected=184756).passed
    assert check("arithmetic", expr="factorial(10)//factorial(7)", expected=720).passed
    assert check("arithmetic", expr="13**11 % 1000", expected=(13 ** 11) % 1000).passed


def test_arithmetic_wrong():
    r = check("arithmetic", expr="comb(20,10)", expected=99999)
    assert not r.passed and r.got == 184756


def test_safe_arith_rejects_code():
    for evil in ('__import__("os").system("x")', "open('f')", "x.y", "lambda: 1"):
        with pytest.raises((UnsafeExpr, SyntaxError, ValueError)):
            safe_arith(evil)


def test_safe_arith_math_ops():
    assert safe_arith("2**10 + gcd(48, 36)") == 1024 + 12
    assert math.isclose(safe_arith("sum([1,2,3,4])"), 10)


def test_json_schema_checker():
    ok = check("json_schema", data={"passed": True}, schema={"type": "object", "required": ["passed"]})
    bad = check("json_schema", data={}, schema={"type": "object", "required": ["passed"]})
    assert ok.passed and not bad.passed


def test_predicate_checker():
    assert check("predicate", value=42, predicate=lambda x: x % 2 == 0).passed


def test_register_custom_checker():
    register_checker("always_true", lambda **_: CheckResult(True, "ok", "always_true"))
    assert "always_true" in available()
    assert check("always_true").passed


def test_unknown_checker_raises():
    with pytest.raises(KeyError):
        check("nope")


def test_adversarial_verify_uses_checker_no_api():
    # task_kind=checkable + checker -> verdict determinista, sin llamar API
    v = adversarial_verify("comb(20,10)=184756", rubric="math", task_kind="checkable",
                           checker="arithmetic", checker_ctx={"expr": "comb(20,10)", "expected": 184756})
    assert v.passed and v.confidence == 1.0 and v.cost_usd == 0.0
    assert v.verifier_model == "tool:arithmetic"
    vb = adversarial_verify("x", rubric="math", task_kind="checkable",
                            checker="arithmetic", checker_ctx={"expr": "2+2", "expected": 5})
    assert not vb.passed and vb.refutations
