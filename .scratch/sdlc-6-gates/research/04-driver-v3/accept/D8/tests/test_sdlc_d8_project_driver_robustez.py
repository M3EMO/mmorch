"""D8 (robustez por modulos: `project_driver`): `run_project_build` nunca crashea por un seam que levanta.

Hoy `build_unit` protege build_fn y gate_fn, pero `run_project_build` llama plan_fn, commit_fn e
integrate_fn sin proteccion: una excepcion ahi tira el build entero y el orquestador no ve nada.
Requisitos, todos con fakes y cero API:
- R1 plan_fn que levanta -> {'status': 'escalate', 'reason': 'planner failed: <tipo>: <msg>'}.
- R2 commit_fn que levanta -> la unidad queda 'built' y el resultado lleva 'commit_error' con el mensaje; el build sigue.
- R3 integrate_fn que levanta -> {'status': 'integration_failed', 'detail': 'integrate_fn <tipo>: <msg>'}, no excepcion.
"""
from mmorch.project_driver import run_project_build


def _unit():
    return [{"name": "u", "spec": "s", "file": "u.py", "deps": [], "test_cmd": None}]


def _ok_build(u):
    return "def f():\n    return 1\n"


def _ok_gate(u, c):
    return True, "ok"


def test_R1_plan_fn_que_levanta_escala_con_motivo():
    def plan(t, e):
        raise ValueError("json roto")
    r = run_project_build("t", external_test=None, plan_fn=plan, build_fn=_ok_build, gate_fn=_ok_gate)
    assert r["status"] == "escalate", r
    assert r["reason"].startswith("planner failed: ValueError: json roto"), r


def test_R2_commit_fn_que_levanta_no_frena_el_build():
    def commit(name, result):
        raise OSError("disco lleno")
    r = run_project_build("t", external_test=None, plan_fn=lambda t, e: _unit(), build_fn=_ok_build,
                          gate_fn=_ok_gate, commit_fn=commit)
    assert r["status"] == "built", r
    assert r["results"][0]["status"] == "built", r
    assert "disco lleno" in r["results"][0].get("commit_error", ""), r


def test_R3_integrate_fn_que_levanta_es_integration_failed():
    def integrate(ext, results):
        raise RuntimeError("pytest explota")
    r = run_project_build("t", external_test="ACCEPT", plan_fn=lambda t, e: _unit(), build_fn=_ok_build,
                          gate_fn=_ok_gate, integrate_fn=integrate)
    assert r["status"] == "integration_failed", r
    assert r["detail"].startswith("integrate_fn RuntimeError: pytest explota"), r
