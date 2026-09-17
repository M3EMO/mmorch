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
    (tmp_path / "pkg" / "a.py").write_text("def suma(a, b):\n    return 0\n", encoding="utf-8")
    (tmp_path / "tests").mkdir()
    subprocess.run(["git", "init", "-q"], cwd=tmp_path, check=True)
    subprocess.run(["git", "add", "-A"], cwd=tmp_path, check=True)
    subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "base"], cwd=tmp_path, check=True)
    # la feature: la mutacion mide solo las lineas que cambian contra HEAD
    (tmp_path / "pkg" / "a.py").write_text("def suma(a, b):\n    if a > 100:\n        return 100\n    return a + b\n", encoding="utf-8")
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


def test_veredicto_con_test_rel_explicito(tmp_path, monkeypatch):
    """Modo grill: el test lo escribe un humano y no hay run-log; el llamador nombra la ruta y el veredicto la guarda."""
    repo = _repo(tmp_path)
    (repo / "tests" / "test_sdlc_grill.py").write_text("def test_R1_x():\n    assert False\n", encoding="utf-8")
    subprocess.run(["git", "add", "-A"], cwd=repo, check=True)
    subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "test"], cwd=repo, check=True)
    monkeypatch.setattr(S, "RUNS", tmp_path / "logs")
    rec = S.registrar_veredicto(str(repo), "HEAD", "rechazado", "quiero otro formato", "tarea", test_rel="tests/test_sdlc_grill.py")
    assert rec["test_rel"] == "tests/test_sdlc_grill.py" and "test_R1_x" in rec["test"]


def test_veredicto_repetido_no_suma_un_ejemplo(tmp_path, monkeypatch):
    """Cada relanzamiento con `verdict` re-registraba la misma decision: 17 filas para 7 veredictos reales."""
    repo = _repo(tmp_path)
    (repo / "tests" / "test_sdlc_m.py").write_text(WEAK, encoding="utf-8")
    subprocess.run(["git", "add", "-A"], cwd=repo, check=True)
    subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "acc"], cwd=repo, check=True)
    monkeypatch.setattr(S, "RUNS", tmp_path / "logs")
    for motivo in ("mide la tarea", "relanzo la corrida"):
        S.registrar_veredicto(str(repo), "HEAD", "aprobado", motivo, test_rel="tests/test_sdlc_m.py")
    S.registrar_veredicto(str(repo), "HEAD", "rechazado", "cambie de idea", test_rel="tests/test_sdlc_m.py")
    (repo / "tests" / "test_sdlc_m.py").write_text(STRONG, encoding="utf-8")  # test nuevo = ejemplo nuevo
    subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qam", "R2"], cwd=repo, check=True)
    S.registrar_veredicto(str(repo), "HEAD", "aprobado", "con R2", test_rel="tests/test_sdlc_m.py")
    filas = [json.loads(x) for x in (tmp_path / "logs" / "veredictos.jsonl").read_text(encoding="utf-8").splitlines()]
    assert [(f["label"], f["motivo"]) for f in filas] == [("aprobado", "mide la tarea"), ("rechazado", "cambie de idea"),
                                                          ("aprobado", "con R2")]


def test_build_job_reporta_el_diff_de_toda_la_feature(tmp_path, monkeypatch):
    """La etapa 6 commitea dentro del worktree: el diffstat del ultimo commit salia vacio o parcial."""
    import mmorch.server_engine as SE
    from mmorch.server_core import _JOBS
    repo = _repo(tmp_path)
    monkeypatch.setattr("mmorch.projects.resolve", lambda name, **k: str(repo))

    def build_falso(kw):
        wt = kw["wt"]
        open(f"{wt}/pkg/b.py", "w", encoding="utf-8").write("B = 1\n")
        subprocess.run(["git", "add", "-A"], cwd=wt, check=True)
        subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "etapa 6"], cwd=wt, check=True)
        return {"status": "built"}
    monkeypatch.setattr(SE, "_build_en_proceso", build_falso)
    SE._run_project_build_job("diff1", "t", "p", "pytest -q")
    assert _JOBS["diff1"]["status"] == "done"
    assert "pkg/b.py" in _JOBS["diff1"]["diffstat"]


def test_build_en_proceso_propaga_el_error_del_hijo(tmp_path):
    """Cada feature corre en su proceso (globals de sdlc por proceso): un error de build_feature llega al job."""
    import pytest

    from mmorch.server_engine import _build_en_proceso
    with pytest.raises(RuntimeError, match="accept"):
        _build_en_proceso({"name": "x", "task": "t", "repo": str(tmp_path)})
