"""D3 (SDLC ticket 06, idea del ticket 03 / research 09): detector de atasco en `build_unit`.

Si el coder devuelve el MISMO codigo dos veces seguidas en el mismo nivel y el gate lo rechaza,
el driver escala en la 2da vuelta aunque queden vueltas. Codigos distintos agotan `max_fix` normal.
Cero API: build_fn y gate_fn son fakes.
"""
import uuid

from mmorch.project_driver import build_unit


def _unit():
    return {"name": f"u-{uuid.uuid4().hex[:8]}", "spec": "x", "file": "u.py", "deps": [], "test_cmd": None}


def test_mismo_codigo_dos_veces_escala_en_la_segunda():
    calls = []

    def build_fn(unit):
        calls.append(1)
        return "def f():\n    return 1\n"          # byte-identico en cada vuelta

    r = build_unit(_unit(), build_fn=build_fn, gate_fn=lambda u, c: (False, "rojo"), max_fix=3, use_cache=False)
    assert r["status"] == "escalate", r
    assert len(calls) == 2, calls                   # no gasta la 3ra vuelta
    assert "atasc" in r["detail"].lower(), r        # el motivo nombra el atasco


def test_codigos_distintos_no_escalan_antes_de_max_fix():
    calls = []

    def build_fn(unit):
        calls.append(1)
        return f"def f():\n    return {len(calls)}\n"

    r = build_unit(_unit(), build_fn=build_fn, gate_fn=lambda u, c: (False, "rojo"), max_fix=3, use_cache=False)
    assert r["status"] == "escalate", r
    assert len(calls) == 3, calls
