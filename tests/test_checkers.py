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


def test_determinant_checker():
    # el caso exacto que gemini erro el signo (decia -1309): det real = 1309
    m = [[1, 2, 4, 4], [9, 1, 9, 4], [9, 7, 4, 8], [5, 1, 3, 7]]
    assert check("determinant", matrix=m, expected=1309).passed
    assert not check("determinant", matrix=m, expected=-1309).passed


def test_determinant_matches_numpy():
    np = pytest.importorskip("numpy")
    import random
    from mmorch.checkers import _det_bareiss
    rng = random.Random(123)
    for _ in range(50):
        n = rng.choice([2, 3, 4, 5])
        m = [[rng.randint(-9, 9) for _ in range(n)] for _ in range(n)]
        assert _det_bareiss(m) == round(float(np.linalg.det(np.array(m, dtype=float))))


def test_predicate_checker():
    assert check("predicate", value=42, predicate=lambda x: x % 2 == 0).passed


def test_register_custom_checker():
    register_checker("always_true", lambda **_: CheckResult(True, "ok", "always_true"))
    assert "always_true" in available()
    assert check("always_true").passed


def test_unknown_checker_raises():
    with pytest.raises(KeyError):
        check("nope")


def test_checksum():
    assert check("checksum", value="4242424242424242", kind="luhn").passed
    assert not check("checksum", value="4242424242424241", kind="luhn").passed
    assert check("checksum", value="9780306406157", kind="isbn13").passed
    assert check("checksum", value="0306406152", kind="isbn10").passed


def test_python_ast_valid():
    assert check("python_ast_valid", code="def f():\n    return 1").passed
    assert not check("python_ast_valid", code="def f(:").passed


def test_regex_format():
    assert check("regex_format", value="a@b.com", fmt="email").passed
    assert not check("regex_format", value="nope", fmt="email").passed
    assert check("regex_format", value="2026-06-08", fmt="iso_date").passed
    assert check("regex_format", value="abc", pattern=r"[a-c]+").passed


def test_set_and_numeric():
    assert check("set_equal", a=[1, 2, 3], b=[3, 2, 1]).passed
    assert not check("set_equal", a=[1, 2], b=[1, 2, 3]).passed
    assert check("numeric_close", a=0.1 + 0.2, b=0.3).passed


def test_monotonic():
    assert check("sorted_monotonic", seq=[1, 2, 3], strict=True).passed
    assert not check("sorted_monotonic", seq=[1, 3, 2]).passed
    assert check("sorted_monotonic", seq=[3, 2, 1], direction="desc").passed


def test_number_theory():
    assert check("number_theory", n=7919, claim="prime").passed
    assert check("number_theory", n=7917, claim="prime", expected=False).passed
    assert check("number_theory", n=561, claim="composite").passed  # Carmichael, no engana a MR


def test_sql_valid():
    pytest.importorskip("sqlglot")
    assert check("sql_valid", sql="SELECT a FROM t WHERE a > 1").passed
    assert not check("sql_valid", sql="SELEKT FROM").passed


def test_units():
    pytest.importorskip("pint")
    assert check("units", quantity="1 mile", to="km", expected=1.609344, tol=1e-5).passed
    assert not check("units", quantity="1 mile", to="km", expected=2.0, tol=1e-5).passed


def test_sympy_identity():
    pytest.importorskip("sympy")
    assert check("sympy_identity", lhs="(x+1)**2", rhs="x**2+2*x+1").passed
    assert not check("sympy_identity", lhs="x+1", rhs="x+2").passed


def test_python_exec_sandbox():
    assert check("python_exec", code="print(2+2)", expected_stdout="4").passed
    assert not check("python_exec", code="1/0").passed
    assert not check("python_exec", code="while True: pass", timeout=2).passed  # timeout-kill


def test_unit_test_gate():
    # el gate git-like: verde -> promovible, rojo -> no
    assert check("unit_test", code="def add(a,b): return a+b",
                 tests="def test_add():\n    assert add(2,3)==5").passed
    assert not check("unit_test", code="def add(a,b): return a-b",
                     tests="def test_add():\n    assert add(2,3)==5").passed


def test_adversarial_verify_uses_checker_no_api():
    # task_kind=checkable + checker -> verdict determinista, sin llamar API
    v = adversarial_verify("comb(20,10)=184756", rubric="math", task_kind="checkable",
                           checker="arithmetic", checker_ctx={"expr": "comb(20,10)", "expected": 184756})
    assert v.passed and v.confidence == 1.0 and v.cost_usd == 0.0
    assert v.verifier_model == "tool:arithmetic"
    vb = adversarial_verify("x", rubric="math", task_kind="checkable",
                            checker="arithmetic", checker_ctx={"expr": "2+2", "expected": 5})
    assert not vb.passed and vb.refutations


def test_code_quality():
    radon = pytest.importorskip("radon")
    good = check("code_quality", code='def add(a, b):\n    """suma."""\n    return a + b\n')
    broken = check("code_quality", code="def f(:\n")
    messy = check("code_quality", code="def f(a,b,c,d,e,f,g):\n" + "    if a:\n" * 6 + "        return 1\n")
    assert good.got > 0.7 and good.passed          # codigo limpio = score alto
    assert not broken.passed and broken.got == 0.0  # no parsea
    assert good.got > messy.got                     # limpio > complejo


def test_mutation_score():
    code = "def add(a, b):\n    return a + b\n"
    strong = "def test_add():\n    assert add(2,3)==5\n    assert add(0,5)==5\n"
    weak = "def test_add():\n    assert add(2,2)==4\n"   # mutante * sobrevive
    s = check("mutation_score", code=code, tests=strong)
    w = check("mutation_score", code=code, tests=weak)
    assert s.got == 1.0 and s.passed          # tests fuertes matan todo
    assert w.got < 1.0                          # tests debiles dejan sobrevivientes


def test_coverage():
    pytest.importorskip("coverage")
    code = "def add(a,b):\n    return a+b\ndef sub(a,b):\n    return a-b\n"
    full = check("coverage", code=code, tests="def test():\n    assert add(1,2)==3\n    assert sub(5,1)==4\n")
    part = check("coverage", code=code, tests="def test():\n    assert add(1,2)==3\n")
    assert full.got == 100.0 and full.passed
    assert part.got < full.got            # menos tests = menos cobertura


def test_deterministic():
    assert check("deterministic", code="print(2+2)").passed                 # reproducible
    assert not check("deterministic", code="import random; print(random.random())").passed
    assert not check("deterministic", code="import time; print(time.time())").passed
