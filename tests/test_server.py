"""nivel 3: bus de eventos + instrumentacion + server Starlette (SSE/control/auth).
Sin API real (providers.call mockeado); sandbox python_exec real."""
import sys, pathlib, time
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import mmorch.events as EVT
import mmorch.patterns as PAT
import mmorch.providers as PROV
from starlette.testclient import TestClient


# ---- bus ---------------------------------------------------------------------
def test_bus_subscribe_publish_recent():
    b = EVT.EventBus()
    q = b.subscribe()
    b.publish(EVT.Event(type="call", status="running", node="x"))
    ev = q.get_nowait()
    assert ev.node == "x" and ev.status == "running"
    assert b.recent(10)[-1].node == "x"


def test_emit_safe_without_subscribers():
    EVT.emit("call", "done", node="solo")          # no debe romper sin suscriptores
    assert EVT.bus().recent(1)[-1].node == "solo"


def test_fanout_emits_events(monkeypatch):
    from dataclasses import dataclass
    @dataclass
    class _R:
        text: str = "ok"; cost_usd: float = 0.0
    monkeypatch.setattr(PAT, "call", lambda *a, **k: _R())
    q = EVT.bus().subscribe()
    PAT.fan_out(["a", "b"], gen_model="deepseek-chat")
    seen = []
    while not q.empty():
        seen.append(q.get_nowait())
    nodes = [e.status for e in seen]
    assert "running" in nodes and "done" in nodes


# ---- server ------------------------------------------------------------------
def _client(monkeypatch, token="secret"):
    monkeypatch.setenv("MMORCH_SERVER_TOKEN", token)
    import importlib, mmorch.server as S
    importlib.reload(S)
    return S, TestClient(S.build_app())


def test_state_requires_token(monkeypatch):
    S, c = _client(monkeypatch)
    assert c.get("/state").status_code == 401
    assert c.get("/state?token=secret").status_code == 200


def test_state_payload_shape(monkeypatch):
    S, c = _client(monkeypatch)
    j = c.get("/state", headers={"X-Token": "secret"}).json()
    assert "summary" in j and "budget" in j and "sections" in j and "jobs" in j


def test_sse_requires_token(monkeypatch):
    S, c = _client(monkeypatch)
    assert c.get("/events").status_code == 401


def test_run_rubric_auth_and_executes(monkeypatch):
    S, c = _client(monkeypatch)
    # sin token -> 401
    assert c.post("/run/rubric", json={"task": "x", "criteria": []}).status_code == 401
    # mock providers.call -> codigo bueno (sin API); el job corre in-process
    monkeypatch.setattr(PROV, "call",
                        lambda *a, **k: type("R", (), {"text": "```python\ndef inc(x):\n return x+1\n```"})())
    crit = [{"id": "c1", "desc": "inc", "kind": "checkable", "checker": "python_exec",
             "ctx": {"code": "{attempt_code}\nassert inc(1)==2"}}]
    r = c.post("/run/rubric", headers={"X-Token": "secret"},
               json={"task": "implementa inc", "criteria": crit, "K": 3})
    assert r.status_code == 200 and r.json()["started"] == "rubric"
    # esperar el evento job done en el bus (job corre en thread)
    ok = False
    for _ in range(60):
        if any(e.type == "job" and e.status == "done" for e in S.bus().recent(80)):
            ok = True; break
        time.sleep(0.1)
    assert ok, "el job rubric deberia emitir job/done"


def test_approve_emits_gate(monkeypatch):
    S, c = _client(monkeypatch)
    r = c.post("/approve/abc123", headers={"X-Token": "secret"})
    assert r.status_code == 200 and r.json()["approved"] == "abc123"
    assert any(e.detail.startswith("APROBADO") for e in S.bus().recent(20))


# ---- guardas de metodo: los mutantes que invertian GET/POST sobrevivian ------
def test_projects_get_lista_y_post_registra(monkeypatch, tmp_path):
    """El mismo path sirve dos cosas segun el metodo. Sin cubrir LAS DOS ramas,
    invertir la condicion (== POST -> != POST) pasaba desapercibido."""
    S, c = _client(monkeypatch)
    r = c.get("/projects?token=secret")
    assert r.status_code == 200 and "projects" in r.json()

    registrado = {}
    import mmorch.projects as P
    monkeypatch.setattr(P, "register",
                        lambda name, path: registrado.setdefault(name, path) or True)
    r = c.post("/projects?token=secret",
               json={"name": "demo", "path": str(tmp_path)})
    assert r.status_code == 200 and "registered" in r.json()
    assert registrado == {"demo": str(tmp_path)}


def test_gate_get_sin_gate_es_404_y_post_lo_crea(monkeypatch):
    S, c = _client(monkeypatch)
    assert c.get("/jobs/nada/gate?token=secret").status_code == 404
    r = c.post("/jobs/j1/gate?token=secret",
               json={"policy": {"stages": [{"name": "s1"}]}})
    assert r.status_code == 200
    assert c.get("/jobs/j1/gate?token=secret").status_code == 200


def test_budget_policies_get_lee_y_post_guarda(monkeypatch):
    S, c = _client(monkeypatch)
    import mmorch.budget_policy as BP
    guardado = []
    monkeypatch.setattr(BP, "save", lambda pols: guardado.append(pols))
    monkeypatch.setattr(BP, "load", lambda: [{"name": "cap"}])
    monkeypatch.setattr(BP, "snapshot", lambda: {"usd": 0.0})
    monkeypatch.setattr(BP, "evaluate", lambda pols, snap: [])

    j = c.get("/budget/policies?token=secret").json()
    assert j["policies"] == [{"name": "cap"}] and "snapshot" in j
    assert not guardado                       # un GET jamas debe escribir

    r = c.post("/budget/policies?token=secret", json={"policies": [{"name": "x"}]})
    assert r.json() == {"saved": 1} and guardado == [[{"name": "x"}]]
