"""W6 ronda 3 (fixer) — defectos de verificadores: body no-JSON al borde HTTP
es 400 {"error","kind"} y no 500 (D-adv3-1), reasoning con max_tokens chico
reintenta con floor en vez de morir con "respuesta vacia" (AT-10 #1), y
serverInfo del MCP reporta la version de mmorch, no la de la lib (defecto #3).
Sin API real (providers mockeados / TestClient)."""
import sys, pathlib, types
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import pytest


# ---- D-adv3-1: body no-JSON -> 400 {"error","kind"}, nunca 500 ----------------
H = {"X-Token": "secret", "Content-Type": "application/json"}


def _client(monkeypatch):
    monkeypatch.setenv("MMORCH_SERVER_TOKEN", "secret")
    import tempfile
    monkeypatch.setenv("MMORCH_HOME", tempfile.mkdtemp())
    import importlib, mmorch.server as S
    importlib.reload(S)
    from starlette.testclient import TestClient
    # raise_server_exceptions=False = repro fiel del borde: si el handler no
    # agarra la excepcion, el assert ve el 500 en vez de un traceback del test
    return TestClient(S.build_app(), raise_server_exceptions=False)


def test_run_rubric_body_binario_es_400(monkeypatch):
    c = _client(monkeypatch)
    r = c.post("/run/rubric", headers=H, content=b"\x00\xffnot json")
    assert r.status_code == 400
    j = r.json()
    assert j["kind"] == "invalid_input" and "error" in j


def test_run_fanout_body_json_trunco_es_400(monkeypatch):
    c = _client(monkeypatch)
    r = c.post("/run/fanout", headers=H, content=b"{")
    assert r.status_code == 400
    assert r.json()["kind"] == "invalid_input"


def test_verdict_body_no_json_es_400(monkeypatch):
    # endpoint con try propio: el contrato 400 invalid_input se sostiene tambien ahi
    c = _client(monkeypatch)
    r = c.post("/verdict", headers=H, content=b"\xfe\xba")
    assert r.status_code == 400
    assert r.json()["kind"] == "invalid_input"


# ---- AT-10 #1: reasoning con max_tokens chico reintenta con floor -------------
class _Usage:
    prompt_tokens = 5
    completion_tokens = 5


def _resp(text, finish):
    ch = types.SimpleNamespace(message=types.SimpleNamespace(content=text),
                               finish_reason=finish)
    return types.SimpleNamespace(choices=[ch], usage=_Usage())


def _client_escalable(calls):
    """Simula glm-5.2: vacio si el budget de tokens es chico, texto con el floor."""
    def create(**kw):
        calls.append(kw.get("max_tokens"))
        if (kw.get("max_tokens") or 0) < 2000:
            return _resp("", "length")
        return _resp("ok", "stop")
    return types.SimpleNamespace(chat=types.SimpleNamespace(
        completions=types.SimpleNamespace(create=create)))


def test_call_reasoning_eleva_floor_una_vez(monkeypatch):
    import mmorch.providers as PV
    calls: list = []
    monkeypatch.setattr(PV, "log_event", lambda **rec: None)
    monkeypatch.setattr(PV, "_client", lambda mk: _client_escalable(calls))
    r = PV.call("glm-5.2", "di ok", max_tokens=5)
    assert r.text == "ok"
    assert calls == [5, PV._REASONING_FLOOR_TOKENS]


def test_call_reasoning_floor_insuficiente_levanta_sin_loop(monkeypatch):
    # si NI el floor alcanza, un solo reintento y el error explicito de siempre
    import mmorch.providers as PV
    calls: list = []

    def create(**kw):
        calls.append(kw.get("max_tokens"))
        return _resp("", "length")
    cli = types.SimpleNamespace(chat=types.SimpleNamespace(
        completions=types.SimpleNamespace(create=create)))
    monkeypatch.setattr(PV, "log_event", lambda **rec: None)
    monkeypatch.setattr(PV, "_client", lambda mk: cli)
    with pytest.raises(RuntimeError, match="max_tokens"):
        PV.call("glm-5.2", "di ok", max_tokens=5)
    assert calls == [5, PV._REASONING_FLOOR_TOKENS]


def test_call_max_tokens_generoso_no_reintenta(monkeypatch):
    # vacio con budget ya >= floor: reintentar seria pagar dos veces por nada
    import mmorch.providers as PV
    calls: list = []

    def create(**kw):
        calls.append(kw.get("max_tokens"))
        return _resp("", "length")
    cli = types.SimpleNamespace(chat=types.SimpleNamespace(
        completions=types.SimpleNamespace(create=create)))
    monkeypatch.setattr(PV, "log_event", lambda **rec: None)
    monkeypatch.setattr(PV, "_client", lambda mk: cli)
    with pytest.raises(RuntimeError, match="max_tokens"):
        PV.call("glm-5.2", "di ok", max_tokens=8192)
    assert calls == [8192]


# ---- defecto #3: serverInfo.version = version de mmorch, no de la lib mcp -----
def test_mcp_server_reporta_version_de_mmorch():
    from importlib.metadata import version
    import mmorch.mcp_server as M
    assert M.mcp._mcp_server.version == version("mmorch")
