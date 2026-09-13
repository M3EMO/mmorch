"""Review D2: gate test-compile en build_project (mmorch/project_integrate.py).

R5/P1 de docs/sdlc/spec.md: el contrato literal de `_default_run_compile` NO envuelve
`subprocess.run` en try/except -> un timeout debe propagarse como `subprocess.TimeoutExpired`,
nunca convertirse en `CheckResult(passed=False, ...)`.
"""
import subprocess
import sys

import pytest

from mmorch import project_integrate as pi


def test_default_compile_detail_fallback_returncode(tmp_path):
    """Contrato (spec.md lineas 47-50): detail = (stdout+stderr).strip() or f"returncode={rc}".
    Con compile_cmd que falla SIN output, el contrato exige detail == "returncode=1". El diff
    devuelve el detail crudo sin .strip() ni fallback -> "" (string vacio), no "returncode=1"."""
    r = pi.build_project(
        "top", str(tmp_path), external_test="ACCEPT",
        plan=lambda t, e: [{"name": "u", "spec": "s", "deps": []}],
        gen=lambda u, fb: "def u():\n    return 1",
        run_test=lambda u, c, tc: (True, ""), run_snippet=lambda c, a: (True, ""),
        propose_test=lambda c, s: "",
        integrate=lambda e, rs: (True, "verde"),
        commit=lambda n, rr: None,
        compile_cmd="exit 1")
    assert r["status"] == "integration_failed", r
    assert r["detail"] == "test-compile: returncode=1", r["detail"]


def test_default_compile_no_debe_capturar_timeout(monkeypatch):
    """El diff envuelve subprocess.run en try/except y atrapa TimeoutExpired devolviendo
    CheckResult(False, "TIMEOUT", ...). El spec (R5, clarificacion P1) exige que la excepcion
    se propague sin capturar. Este test falla contra la implementacion actual del diff."""
    monkeypatch.setattr(pi, "_COMPILE_TIMEOUT_S", 0.2)
    slow_cmd = f'"{sys.executable}" -c "import time; time.sleep(5)"'
    with pytest.raises(subprocess.TimeoutExpired):
        pi._default_compile(".", slow_cmd)
