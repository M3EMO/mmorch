"""project-aware: registro de proyectos + ejecutor en PLAN (claude headless) + endpoints.
No invoca claude real (subprocess mockeado); cero cupo en tests."""
import sys, pathlib, json, importlib, types
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import mmorch.projects as PJ
import mmorch.claude_exec as CE
import mmorch.events as EVT
from starlette.testclient import TestClient


# ---- projects registry -------------------------------------------------------
def test_register_resolve_list_unregister(tmp_path):
    store = tmp_path / "projects.json"
    proj = tmp_path / "portfolio"; proj.mkdir()
    PJ.register("portfolio", str(proj), store=store)
    import os
    assert os.path.basename(PJ.resolve("portfolio", store=store)) == "portfolio"
    assert "portfolio" in PJ.list_projects(store=store)
    assert PJ.unregister("portfolio", store=store) is True
    assert PJ.list_projects(store=store) == {}


def test_register_rejects_nonexistent_path(tmp_path):
    store = tmp_path / "p.json"
    try:
        PJ.register("x", str(tmp_path / "nope"), store=store)
        assert False
    except ValueError:
        pass


def test_resolve_missing_raises(tmp_path):
    try:
        PJ.resolve("ghost", store=tmp_path / "p.json")
        assert False
    except KeyError:
        pass


# ---- claude_exec (sin invocar claude real) -----------------------------------
def test_claude_bin_returns_argv():
    assert isinstance(CE.claude_bin(), list) and CE.claude_bin()


def test_stream_line_emits_tool_use():
    q = EVT.bus().subscribe()
    line = json.dumps({"type": "assistant", "message": {"content": [
        {"type": "tool_use", "name": "Edit", "input": {"file_path": "app.py"}}]}})
    n = CE._emit_stream_line(line, "job1")
    assert n == 1
    ev = q.get_nowait()
    assert ev.node == "claude:Edit" and "app.py" in ev.detail


def test_maybe_result_parses():
    assert CE._maybe_result(json.dumps({"type": "result", "result": "done"})) == "done"
    assert CE._maybe_result("not json") == ""


def test_run_claude_bad_cwd():
    r = CE.run_claude("hola", "/no/such/dir", mode="plan")
    assert r["ok"] is False


# ---- server endpoints --------------------------------------------------------
def _client(monkeypatch, token="secret"):
    monkeypatch.setenv("MMORCH_SERVER_TOKEN", token)
    import mmorch.server as S
    importlib.reload(S)
    return S, TestClient(S.build_app())


def test_projects_endpoint_auth_and_list(monkeypatch):
    S, c = _client(monkeypatch)
    assert c.get("/projects").status_code == 401
    monkeypatch.setattr("mmorch.projects.list_projects", lambda **k: {"portfolio": "/x"})
    j = c.get("/projects", headers={"X-Token": "secret"}).json()
    assert j["projects"]["portfolio"] == "/x"


def test_run_project_auth_and_dispatch(monkeypatch):
    S, c = _client(monkeypatch)
    assert c.post("/run/project", json={"project": "p", "task": "t"}).status_code == 401
    called = {}
    monkeypatch.setattr("mmorch.projects.resolve", lambda name, **k: "/tmp/p")
    monkeypatch.setattr("mmorch.claude_exec.run_claude",
                        lambda task, cwd, **k: called.update(task=task, cwd=cwd, mode=k.get("mode")) or {"ok": True})
    r = c.post("/run/project", headers={"X-Token": "secret"},
               json={"project": "portfolio", "task": "refactor X", "mode": "plan", "engine": "claude"})
    assert r.status_code == 200 and r.json()["engine"] == "claude"
    import time
    for _ in range(30):
        if called: break
        time.sleep(0.05)
    assert called.get("task") == "refactor X" and called.get("cwd") == "/tmp/p"


def test_run_project_rejects_bad_mode(monkeypatch):
    S, c = _client(monkeypatch)
    r = c.post("/run/project", headers={"X-Token": "secret"},
               json={"project": "p", "task": "t", "mode": "nuke"})
    assert r.status_code == 400
