"""Tests para exec_embedder: embedding por EJECUCION (huella de comportamiento, cero train).
Corre subprocesos via sandbox (como el resto del flywheel). Run: python -m pytest tests/test_exec_embedder.py -q
"""
from __future__ import annotations
import math

from mmorch.exec_embedder import embed_exec, embed_hybrid, D
from mmorch.code_embedder import available as cb_available


def _cos(a, b):
    s = sum(x * y for x, y in zip(a, b))
    na = math.sqrt(sum(x * x for x in a)) or 1.0
    nb = math.sqrt(sum(y * y for y in b)) or 1.0
    return s / (na * nb)


def test_dim_and_none():
    v = embed_exec("def f(x):\n    return x + 1", "f")
    assert v is not None and len(v) == D
    assert embed_exec("x = 5\ny = x + 1", None) is None        # sin def top-level ni hint -> None


def test_equivalent_closer_than_different():
    fib_iter = "def fib(n):\n    a, b = 0, 1\n    for _ in range(n): a, b = b, a + b\n    return a"
    fib_fun = ("def fib(n):\n    from functools import reduce\n"
               "    return reduce(lambda p, _: (p[1], p[0] + p[1]), range(n), (0, 1))[0]")
    gcd = "def gcd(a, b):\n    while b: a, b = b, a % b\n    return a"
    vi, vf, vg = embed_exec(fib_iter, "fib"), embed_exec(fib_fun, "fib"), embed_exec(gcd, "gcd")
    assert None not in (vi, vf, vg)
    equiv = _cos(vi, vf)            # mismas-funcion equivalentes
    diff = _cos(vi, vg)            # funcion distinta (otra arity -> sondas distintas)
    assert equiv > 0.9
    assert equiv > diff


def test_canon_dict_order_invariant():
    # mismo dict construido en distinto orden de insercion -> mismo canon -> misma huella
    d1 = "def g(x):\n    return {'a': 1, 'b': 2}"
    d2 = "def g(x):\n    d = {}\n    d['b'] = 2\n    d['a'] = 1\n    return d"
    v1, v2 = embed_exec(d1, "g"), embed_exec(d2, "g")
    assert None not in (v1, v2)
    assert _cos(v1, v2) > 0.999      # dicts iguales (orden distinto) -> huella identica


def test_inplace_mutation_captured():
    # fn que muta in-place (sort) vs no-op: ambas devuelven None; la mutacion las distingue
    mut = "def s(lst):\n    lst.sort()"
    noop = "def s(lst):\n    return None"
    vm, vn = embed_exec(mut, "s"), embed_exec(noop, "s")
    assert None not in (vm, vn)
    assert _cos(vm, vn) < 0.999      # capturar la mutacion las separa


def test_hybrid_consistent_dim():
    if not cb_available():
        return                       # sin artefactos del code_embedder, embed_hybrid -> None (skip)
    h_code = embed_hybrid("def f(x):\n    return x * 2", "f")
    h_text = embed_hybrid("esto no es una funcion ejecutable, solo texto", None)
    assert h_code is not None and h_text is not None
    assert len(h_code) == len(h_text)          # dim CONSISTENTE (behavioral=zeros si no corre)
    assert len(h_code) > D                      # structural (>0) + behavioral (D)
