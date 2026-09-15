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
H = {"X-Token": "secret"}


def _client(monkeypatch, token="secret"):
    monkeypatch.setenv("MMORCH_SERVER_TOKEN", token)
    # MMORCH_HOME aislado: los jobs se espejan en logs/jobs.jsonl (W3.2) y los
    # tests no deben ensuciar el registro durable real del repo
    import tempfile
    monkeypatch.setenv("MMORCH_HOME", tempfile.mkdtemp())
    import importlib, mmorch.server as S
    importlib.reload(S)
    return S, TestClient(S.build_app())


def test_state_requires_token(monkeypatch):
    S, c = _client(monkeypatch)
    assert c.get("/state").status_code == 401
    # W3.2: el token por query string ya NO autentica (solo header)
    assert c.get("/state?token=secret").status_code == 401
    assert c.get("/state", headers=H).status_code == 200


def test_state_payload_shape(monkeypatch):
    S, c = _client(monkeypatch)
    j = c.get("/state", headers={"X-Token": "secret"}).json()
    assert "summary" in j and "budget" in j and "sections" in j and "jobs" in j


def test_sse_requires_token(monkeypatch):
    S, c = _client(monkeypatch)
    assert c.get("/events").status_code == 401


def test_event_ids_assigned_monotonic():
    # events.py: EventBus.publish() ahora asigna Event.id (monotonico, thread-safe
    # bajo el mismo lock que el ring buffer) -- base de AT-L7.
    b = EVT.EventBus()
    e1 = EVT.Event(type="job", status="running", job_id="j1")
    e2 = EVT.Event(type="job", status="done", job_id="j1")
    b.publish(e1); b.publish(e2)
    assert e1.id > 0 and e2.id > e1.id


def test_parse_last_event_id(monkeypatch):
    S, _ = _client(monkeypatch)
    assert S._parse_last_event_id(None) == 0
    assert S._parse_last_event_id("") == 0
    assert S._parse_last_event_id("42") == 42
    assert S._parse_last_event_id("no-es-un-numero") == 0   # header corrupto -> no crashea, no filtra


def test_sse_replay_filters_events_client_already_has(monkeypatch):
    # AT-L7 (Lotus): el replay inicial de recent(30) ya no reenvia a ciegas -- filtra
    # contra Last-Event-ID, misma logica que corre dentro de sse_events.gen().
    S, _ = _client(monkeypatch)
    S.bus().publish(EVT.Event(type="job", status="running", job_id="j1"))
    ev_seen = EVT.Event(type="job", status="done", job_id="j1")
    S.bus().publish(ev_seen)
    ev_new = EVT.Event(type="job", status="done", job_id="j2")
    S.bus().publish(ev_new)

    last_id = S._parse_last_event_id(str(ev_seen.id))
    replay = [e for e in S.bus().recent(30) if e.id > last_id]
    assert ev_new in replay and ev_seen not in replay


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


# ---- W6 D2 (Lotus): shape invalida = 400 ANTES de crear el job + job_id -------
def test_run_rubric_criteria_shape_invalida_400(monkeypatch):
    """Antes: criteria string/dict/[1,2,3] devolvia 200 'started' y el crash
    (start_rubric sobre un no-list) moria invisible en el worker thread."""
    S, c = _client(monkeypatch)
    for bad in ({"a": 1}, "no-una-lista", [1, 2, 3], [{"desc": "sin id"}]):
        r = c.post("/run/rubric", headers=H, json={"task": "t", "criteria": bad})
        assert r.status_code == 400, f"criteria={bad!r} deberia dar 400"
        j = r.json()
        assert j["kind"] == "invalid_input" and j["error"]


def test_run_endpoints_devuelven_job_id(monkeypatch):
    """D2 Lotus: todo /run/* que crea job devuelve su job_id (sin el, el cliente
    no puede matar/pausar/seguir el job que acaba de lanzar)."""
    S, c = _client(monkeypatch)
    fake = lambda *a, **k: type("R", (), {"text": "ok", "cost_usd": 0.0,   # noqa: E731
                                          "in_tokens": 1, "out_tokens": 1})()
    monkeypatch.setattr(PROV, "call", fake)
    monkeypatch.setattr(PAT, "call", fake)   # fan_out importa call por nombre
    r = c.post("/run/rubric", headers=H, json={"task": "t", "criteria": [], "K": 1})
    assert r.status_code == 200 and r.json()["job_id"]
    r = c.post("/run/fanout", headers=H, json={"prompts": ["a"]})
    assert r.status_code == 200 and r.json()["job_id"]
    # fanout con prompts no-lista -> mismo contrato 400
    r = c.post("/run/fanout", headers=H, json={"prompts": "hola"})
    assert r.status_code == 400 and r.json()["kind"] == "invalid_input"


# ---- W6 D3 (Lotus): /state expone si un job interrumpido es resumible ---------
def test_state_expone_resumable(monkeypatch):
    """Sin el flag, el cliente comia el 409 de /resume a ciegas: no podia saber
    si un interrupted tenia checkpoint+spec (lo mismo que resume_job chequea)."""
    S, c = _client(monkeypatch)
    import mmorch.workflow_store as WS
    monkeypatch.setattr(WS, "jobs_with_checkpoints", lambda: {"jr1"})
    monkeypatch.setattr(WS, "jobs_with_specs", lambda: {"jr1"})
    from mmorch.server_core import _JOBS, _JOBS_LOCK
    with _JOBS_LOCK:
        _JOBS["jr1"] = {"status": "interrupted", "kind": "rubric"}
        _JOBS["jr2"] = {"status": "interrupted", "kind": "rubric"}   # sin checkpoint
    try:
        jobs = c.get("/state", headers=H).json()["jobs"]
        assert jobs["jr1"]["resumable"] is True
        assert jobs["jr2"]["resumable"] is False
    finally:
        with _JOBS_LOCK:
            _JOBS.pop("jr1", None); _JOBS.pop("jr2", None)


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
    r = c.get("/projects", headers=H)
    assert r.status_code == 200 and "projects" in r.json()

    registrado = {}
    import mmorch.projects as P
    monkeypatch.setattr(P, "register",
                        lambda name, path: registrado.setdefault(name, path) or True)
    r = c.post("/projects", headers=H,
               json={"name": "demo", "path": str(tmp_path)})
    assert r.status_code == 200 and "registered" in r.json()
    assert registrado == {"demo": str(tmp_path)}


def test_budget_policies_get_lee_y_post_guarda(monkeypatch):
    S, c = _client(monkeypatch)
    import mmorch.budget_policy as BP
    guardado = []
    monkeypatch.setattr(BP, "save", lambda pols: guardado.append(pols))
    monkeypatch.setattr(BP, "load", lambda: [{"name": "cap"}])
    monkeypatch.setattr(BP, "snapshot", lambda: {"usd": 0.0})
    monkeypatch.setattr(BP, "evaluate", lambda pols, snap: [])

    j = c.get("/budget/policies", headers=H).json()
    assert j["policies"] == [{"name": "cap"}] and "snapshot" in j
    assert not guardado                       # un GET jamas debe escribir

    r = c.post("/budget/policies", headers=H, json={"policies": [{"name": "x"}]})
    assert r.json() == {"saved": 1} and guardado == [[{"name": "x"}]]


def test_projects_delete_saca_del_registro(monkeypatch, tmp_path):
    """Se podia registrar un proyecto y nunca sacarlo desde la app (projects.unregister
    existia pero el server no lo exponia): la unica salida era editar projects.json."""
    S, c = _client(monkeypatch)
    import mmorch.projects as P
    store = tmp_path / "projects.json"
    monkeypatch.setattr(P, "PROJECTS_PATH", store, raising=False)
    monkeypatch.setattr(P, "_load", lambda s=None: __import__("json").loads(store.read_text()) if store.exists() else {})
    monkeypatch.setattr(P, "_save", lambda d, s=None: store.write_text(__import__("json").dumps(d)))

    d = tmp_path / "proy"
    d.mkdir()
    assert c.post("/projects", headers=H, json={"name": "p1", "path": str(d)}).status_code == 200
    assert "p1" in c.get("/projects", headers=H).json()["projects"]

    r = c.request("DELETE", "/projects?name=p1", headers=H)
    assert r.status_code == 200 and r.json()["existia"] is True
    assert "p1" not in c.get("/projects", headers=H).json()["projects"]

    # idempotente: borrar dos veces no revienta, solo dice que no estaba
    assert c.request("DELETE", "/projects?name=p1", headers=H).json()["existia"] is False
    # sin nombre -> 400, no un borrado silencioso de nada
    assert c.request("DELETE", "/projects", headers=H).status_code == 400
    # sigue exigiendo token
    assert c.request("DELETE", "/projects?name=p1").status_code == 401


def test_fleet_delete_saca_el_host(monkeypatch, tmp_path):
    """Simetrico a POST /fleet: sin esto habia que borrar hosts.json a mano."""
    S, c = _client(monkeypatch)
    import mmorch.fleet as F
    store = tmp_path / "hosts.json"
    monkeypatch.setattr(F, "_load", lambda s=None: __import__("json").loads(store.read_text()) if store.exists() else {})
    monkeypatch.setattr(F, "_save", lambda d, s=None: store.write_text(__import__("json").dumps(d)))

    assert c.post("/fleet", headers=H, json={"name": "h1", "url": "http://127.0.0.1:9", "token": "t"}).status_code == 200
    assert "h1" in c.get("/fleet", headers=H).json()["hosts"]

    r = c.request("DELETE", "/fleet?name=h1", headers=H)
    assert r.status_code == 200 and r.json()["existia"] is True
    assert "h1" not in c.get("/fleet", headers=H).json()["hosts"]
    assert c.request("DELETE", "/fleet?name=h1", headers=H).json()["existia"] is False
    assert c.request("DELETE", "/fleet", headers=H).status_code == 400
    assert c.request("DELETE", "/fleet?name=h1").status_code == 401
