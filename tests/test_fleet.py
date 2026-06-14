"""fleet: control multi-host del tailnet (registro + estado agregado + forward). httpx
mockeado (no toca red). + jobs del /state traen title/ts (para el Kanban)."""
import sys, pathlib, importlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import mmorch.fleet as FL
from starlette.testclient import TestClient


def test_register_and_list(tmp_path):
    store = tmp_path / "hosts.json"
    FL.register_host("mateo", "http://100.88.0.57:8787/", "tok", store=store)
    h = FL.list_hosts(store=store)
    assert h["mateo"]["url"] == "http://100.88.0.57:8787" and h["mateo"]["token"] == "tok"


class _Resp:
    def __init__(self, code, data):
        self.status_code = code; self._d = data
        self.headers = {"content-type": "application/json"}; self.content = b"x"
    def json(self):
        return self._d


def test_fleet_state_aggregates(monkeypatch, tmp_path):
    store = tmp_path / "h.json"
    FL.register_host("a", "http://100.0.0.1:8787", "t", store=store)
    FL.register_host("b", "http://100.0.0.2:8787", "t", store=store)
    monkeypatch.setattr("httpx.get", lambda url, **k: _Resp(200, {"summary": {"calls": 5}, "jobs": {}, "budget": {}}))
    st = FL.fleet_state(store=store)
    assert st["hosts"]["a"]["ok"] and st["hosts"]["a"]["summary"]["calls"] == 5
    assert st["hosts"]["b"]["ok"]


def test_fleet_state_marks_down_host(monkeypatch, tmp_path):
    store = tmp_path / "h.json"
    FL.register_host("dead", "http://100.0.0.9:8787", "t", store=store)
    def boom(url, **k):
        raise RuntimeError("conn refused")
    monkeypatch.setattr("httpx.get", boom)
    st = FL.fleet_state(store=store)
    assert st["hosts"]["dead"]["ok"] is False and "error" in st["hosts"]["dead"]


def test_forward_posts_to_host(monkeypatch, tmp_path):
    store = tmp_path / "h.json"
    FL.register_host("mateo", "http://100.88.0.57:8787", "tok", store=store)
    captured = {}
    def fake_post(url, json=None, headers=None, timeout=None):
        captured.update(url=url, json=json, headers=headers); return _Resp(200, {"started": "project"})
    monkeypatch.setattr("httpx.post", fake_post)
    r = FL.forward("mateo", "/run/project", {"project": "p", "task": "t"}, store=store)
    assert r["ok"] and captured["url"] == "http://100.88.0.57:8787/run/project"
    assert captured["headers"]["X-Token"] == "tok"


def test_forward_unknown_host(tmp_path):
    r = FL.forward("ghost", "/x", {}, store=tmp_path / "h.json")
    assert r["ok"] is False


# --- server: fleet endpoints + kanban job shape -------------------------------
def _client(monkeypatch, token="secret"):
    monkeypatch.setenv("MMORCH_SERVER_TOKEN", token)
    import mmorch.server as S
    importlib.reload(S)
    return S, TestClient(S.build_app())


def test_fleet_endpoint_auth(monkeypatch):
    S, c = _client(monkeypatch)
    assert c.get("/fleet").status_code == 401
    monkeypatch.setattr("mmorch.fleet.list_hosts", lambda **k: {})
    monkeypatch.setattr("mmorch.fleet.fleet_state", lambda **k: {"hosts": {}})
    assert c.get("/fleet", headers={"X-Token": "secret"}).status_code == 200


def test_state_jobs_have_kanban_fields(monkeypatch):
    S, c = _client(monkeypatch)
    with S._JOBS_LOCK:
        S._JOBS["j1"] = S._jobmeta("project", "arreglar bug", engine="mmorch")
    j = c.get("/state", headers={"X-Token": "secret"}).json()
    job = j["jobs"]["j1"]
    assert job["title"] == "arreglar bug" and job["kind"] == "project" and "ts" in job
