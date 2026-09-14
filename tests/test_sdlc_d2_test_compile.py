"""D2 (SDLC ticket 06, primer gate del contrato del ticket 02 escrito en el engine):
`build_project` corre un gate `test-compile` ANTES del `external_test` de integracion.

Contrato:
- `build_project(..., compile_cmd: str | None = None, run_compile=None)`.
- `run_compile(compile_cmd) -> CheckResult` (mmorch.checkers.CheckResult, checker="test_compile").
  Default: corre `compile_cmd` con subprocess en `repo`; passed = returncode 0.
- Con `compile_cmd` declarado y el gate rojo: status 'integration_failed', `detail` empieza con
  'test-compile:' y contiene el detail del CheckResult; `integrate` NO se llama (fail-closed).
- Con el gate verde: `integrate` se llama y el resultado sigue como hoy.
- Sin `compile_cmd`: el gate no corre y nada cambia (compatibilidad).
Cero API: plan, gen y los demas seams inyectados.
"""
import uuid

from mmorch.checkers import CheckResult
from mmorch.project_integrate import build_project


def _plan(tag):
    return lambda t, e: [{"name": f"u-{tag}", "spec": "x", "file": "u.py", "deps": [], "test_cmd": None}]


def _common(tag):
    return dict(external_test="ACCEPT", plan=_plan(tag), gen=lambda u, fb: "def f():\n    return 1\n",
                run_test=lambda u, c, t: (True, "ok"), propose_test=lambda c, s: "",
                run_snippet=lambda c, a: (True, ""), commit=lambda n, r: None, write_file=lambda u, c: None,
                gen_model="deepseek-v4-pro", verifier_model="gemini-2.5-flash")


def test_compile_rojo_corta_antes_de_integrar(tmp_path):
    tag = uuid.uuid4().hex[:8]
    calls = []
    res = build_project("t", str(tmp_path), **_common(tag), compile_cmd="python -m compileall .",
                        run_compile=lambda cmd: CheckResult(passed=False, detail="boom en u.py", checker="test_compile"),
                        integrate=lambda ext, results: calls.append(ext) or (True, "verde"))
    assert res["status"] == "integration_failed", res
    assert res["detail"].startswith("test-compile:") and "boom en u.py" in res["detail"], res
    assert calls == [], "integrate no debe correr si test-compile esta rojo"


def test_compile_verde_deja_integrar(tmp_path):
    tag = uuid.uuid4().hex[:8]
    calls = []
    res = build_project("t", str(tmp_path), **_common(tag), compile_cmd="python -m compileall .",
                        run_compile=lambda cmd: CheckResult(passed=True, detail="ok", checker="test_compile"),
                        integrate=lambda ext, results: calls.append(ext) or (True, "verde"))
    assert res["status"] == "built" and res.get("integrated"), res
    assert calls == ["ACCEPT"]


def test_sin_compile_cmd_no_cambia_nada(tmp_path):
    tag = uuid.uuid4().hex[:8]
    calls = []
    res = build_project("t", str(tmp_path), **_common(tag),
                        integrate=lambda ext, results: calls.append(ext) or (True, "verde"))
    assert res["status"] == "built" and calls == ["ACCEPT"], res
