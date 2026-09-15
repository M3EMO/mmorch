"""Ticket 08 D2: veredicto obligatorio al reanudar, registro en logs/sdlc/veredictos.jsonl, gate de mutacion. Cero API."""
import importlib
import json
import subprocess
import types

from starlette.testclient import TestClient

import mmorch.sdlc as S

STRONG = "def test_R1_suma():\n    from pkg.a import suma\n    assert suma(1, 2) == 3\n    assert suma(2, 3) == 5\n    assert suma(-1, 1) == 0\n"
WEAK = "def test_R1_suma():\n    from pkg.a import suma\n    assert suma(0, 0) == 0\n"


def _repo(tmp_path):
    (tmp_path / "pkg").mkdir()
    (tmp_path / "pkg" / "a.py").write_text("def suma(a, b):\n    if a > 100:\n        return 100\n    return a + b\n", encoding="utf-8")
    (tmp_path / "tests").mkdir()
    subprocess.run(["git", "init", "-q"], cwd=tmp_path, check=True)
    subprocess.run(["git", "add", "-A"], cwd=tmp_path, check=True)
    subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "base"], cwd=tmp_path, check=True)
    return tmp_path


def _configura(repo, test_text, mutation_min=None):
    toml = 'accept_cmd = "python -m pytest -q"\nfiles = ["pkg/a.py"]\n' + (f"mutation_min = {mutation_min}\n" if mutation_min is not None else "")
    (repo / "sdlc.toml").write_text(toml, encoding="utf-8")
    (repo / "tests" / "test_acc.py").write_text(test_text, encoding="utf-8")
    t = types.SimpleNamespace(name="m", task="suma", accept_files={"tests/test_acc.py": test_text})
    S.configure(t, contract=[], feat={"repo": str(repo), "files": ["pkg/a.py"], "suite": ["tests"]}, wt=repo)
    S.state["plan_files"] = ["pkg/a.py"]


def test_mutacion_observa_sin_umbral_y_bloquea_con_umbral(tmp_path):
    repo = _repo(tmp_path)
    _configura(repo, STRONG)
    ok, nota = S.gate_mutacion()
    assert ok and "observa" in nota and 0 < S.state["mutation_score"] <= 1.0
    assert (repo / "pkg" / "a.py").read_text(encoding="utf-8").startswith("def suma")  # restaurado
    _configura(repo, WEAK, mutation_min=0.99)
    ok, nota = S.gate_mutacion()
    assert not ok and "mutation score" in nota


def test_registrar_veredicto_lee_el_test_de_la_branch(tmp_path, monkeypatch):
    repo = _repo(tmp_path)
    subprocess.run(["git", "checkout", "-q", "-b", "rama"], cwd=repo, check=True)
    (repo / "tests" / "test_sdlc_m.py").write_text(WEAK, encoding="utf-8")
    (repo / "docs" / "sdlc").mkdir(parents=True)
    (repo / "docs" / "sdlc" / "run-log.json").write_text(json.dumps({"awaiting_approval": "tests/test_sdlc_m.py"}), encoding="utf-8")
    subprocess.run(["git", "add", "-A"], cwd=repo, check=True)
    subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "acc"], cwd=repo, check=True)
    monkeypatch.setattr(S, "RUNS", tmp_path / "logs")
    rec = S.registrar_veredicto(str(repo), "rama", "rechazado", "no cubre el borde", task="suma")
    assert rec["test_rel"] == "tests/test_sdlc_m.py" and rec["test"] == WEAK
    lineas = (tmp_path / "logs" / "veredictos.jsonl").read_text(encoding="utf-8").strip().splitlines()
    assert json.loads(lineas[-1])["label"] == "rechazado"


def test_resume_desde_2_exige_veredicto(monkeypatch, tmp_path):
    monkeypatch.setenv("MMORCH_SERVER_TOKEN", "secret")
    import mmorch.server as SV
    importlib.reload(SV)
    monkeypatch.setattr("mmorch.projects.resolve", lambda name, **k: str(tmp_path))
    monkeypatch.setattr(S, "RUNS", tmp_path / "logs")
    lanzado = []
    monkeypatch.setattr(SV, "_run_project_build_job", lambda *a, **k: lanzado.append(k))
    c = TestClient(SV.build_app())
    H = {"X-Token": "secret"}
    base = {"task": "t", "workflow_name": "project-build", "project": "p", "external_test": "pytest -q",
            "resume_branch": "rama", "from_stage": 2}
    assert c.post("/run/workflow", json=base, headers=H).status_code == 400
    r = c.post("/run/workflow", json={**base, "verdict": {"label": "rechazado", "motivo": "flojo"}}, headers=H)
    assert r.status_code == 200 and r.json()["recorded"] == "rechazado" and not lanzado
    r = c.post("/run/workflow", json={**base, "verdict": {"label": "aprobado", "motivo": "mide la tarea"}}, headers=H)
    assert r.status_code == 200 and "job_id" in r.json()
    assert len((tmp_path / "logs" / "veredictos.jsonl").read_text(encoding="utf-8").strip().splitlines()) == 2


def test_run_recupera_el_test_al_reanudar(tmp_path):
    repo = _repo(tmp_path)
    (repo / "tests" / "test_sdlc_m.py").write_text(WEAK, encoding="utf-8")
    (repo / "docs" / "sdlc").mkdir(parents=True)
    (repo / "docs" / "sdlc" / "run-log.json").write_text(json.dumps({"awaiting_approval": "tests/test_sdlc_m.py"}), encoding="utf-8")
    (repo / "docs" / "sdlc" / "plan.md").write_text("## Archivos\n- `pkg/a.py` [R1]\n", encoding="utf-8")
    t = types.SimpleNamespace(name="m", task="suma", accept_files={})
    S.configure(t, contract=[], feat={"repo": str(repo), "files": ["pkg/a.py"], "suite": ["tests"]}, wt=repo, accept_cmd="pytest")
    S.run(from_stage=99)  # ninguna etapa corre: solo la preparacion (sin API)
    assert "tests/test_sdlc_m.py" in t.accept_files


def test_tests_nombrados_en_cualquier_lenguaje(tmp_path):
    """El server reconoce el test de aceptacion que nombra el payload aunque no sea .py (Estudio, ChatBot)."""
    from mmorch.server_engine import _tests_nombrados
    for rel in ["tests/test_sdlc_a.py", "app/src/sdlc/export_mastery.test.ts", "backend/pom.xml"]:
        (tmp_path / rel).parent.mkdir(parents=True, exist_ok=True)
        (tmp_path / rel).write_text("x", encoding="utf-8")
    assert _tests_nombrados("python -m pytest -q tests/test_sdlc_a.py", tmp_path) == ["tests/test_sdlc_a.py"]
    assert _tests_nombrados("app/src/sdlc/export_mastery.test.ts", tmp_path) == ["app/src/sdlc/export_mastery.test.ts"]
    assert _tests_nombrados("mvn -q -f backend/pom.xml test", tmp_path) == []
    assert _tests_nombrados("pytest -q tests/test_no_existe.py", tmp_path) == []
