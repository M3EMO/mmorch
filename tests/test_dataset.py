"""tests dataset.py — extracción de funciones (core del miner JIT-defect). Sin git/API."""
from mmorch.dataset import _functions_covering, _FIX_RE


def test_functions_covering_picks_changed():
    src = "def a():\n    return 1\n\ndef b():\n    x = 2\n    return x\n"
    # línea 5 (x=2) está en b
    fns = _functions_covering(src, {5})
    assert any("def b()" in f for f in fns) and not any("def a()" in f for f in fns)


def test_functions_covering_handles_broken_source():
    assert _functions_covering("def f(:\n", {1}) == []   # no parsea -> []


def test_fix_regex():
    assert _FIX_RE.search("Fix crash in parser")
    assert _FIX_RE.search("bugfix: wrong header")
    assert not _FIX_RE.search("add new feature")
