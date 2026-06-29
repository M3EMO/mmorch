"""sandbox: policy allowlist pre-ejecucion (Hermes #14) + backend pluggable (Hermes #12).
Backend docker no se ejercita en CI (puede no estar); se testea el fallback graceful."""
import sys, pathlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
from mmorch.sandbox import (run_sandboxed, policy_violations)


def test_clean_code_no_violations():
    assert policy_violations("def f(x):\n    return x+1\nprint(f(2))") == []


def test_dangerous_patterns_flagged():
    assert policy_violations("import socket")
    assert policy_violations("import subprocess as sp")
    assert policy_violations("os.system('rm -rf /')")
    assert policy_violations("__import__('os')")
    assert policy_violations("open('x.txt','w')")


def test_enforce_policy_blocks_before_running():
    # codigo que abriria red: con enforce_policy NO corre, queda bloqueado
    r = run_sandboxed("import socket\ns=socket.socket()", enforce_policy=True)
    assert not r.ok and r.returncode is None and r.violations
    assert "POLICY BLOCK" in r.stderr


def test_enforce_policy_allows_clean_code():
    # timeout holgado: bajo carga paralela el spawn del subproceso puede tardar
    r = run_sandboxed("print(2+2)", enforce_policy=True, timeout=30)
    assert r.ok and r.stdout.strip() == "4" and r.violations == ()


def test_policy_off_by_default_runs_anything():
    # sin enforce_policy el comportamiento viejo se mantiene (opt-in, no rompe nada)
    r = run_sandboxed("print('hola')", timeout=30)
    assert r.ok and r.stdout.strip() == "hola"


def test_docker_backend_graceful_when_absent(monkeypatch):
    import mmorch.sandbox as S
    monkeypatch.setattr(S, "docker_available", lambda: False)
    r = S.run_sandboxed("print(1)", backend="docker")
    assert not r.ok and "docker" in r.violations[0]
