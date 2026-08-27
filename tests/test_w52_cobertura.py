"""W5.2 — cobertura real para los 7 modulos con cobertura CERO (03 §2.2):

pty_session, server_core, server_engine, server_fleet, server_frontend,
server_pty, transcript_store. Smoke de la superficie publica con I/O mockeado —
NO duplica test_server_smoke (tabla de rutas + auth global): aca se prueba la
LOGICA de cada modulo, con el server solo como transporte donde hace falta.
"""
from __future__ import annotations

import json
import os
from types import SimpleNamespace

import pytest


@pytest.fixture()
def home(tmp_path, monkeypatch):
    # estado del server (jobs.jsonl, fleet, policies) aislado del home real
    monkeypatch.setenv("MMORCH_HOME", str(tmp_path))
    (tmp_path / "logs").mkdir()
    return tmp_path


@pytest.fixture()
def client(home, monkeypatch):
    from starlette.testclient import TestClient
    from mmorch.server import build_app
    monkeypatch.setenv("MMORCH_SERVER_TOKEN", "w52-secret")
    return TestClient(build_app())


AUTH = {"X-Token": "w52-secret"}


# --------------------------------------------------------------------------- #
# transcript_store — vista in-memory por job
# --------------------------------------------------------------------------- #
@pytest.fixture()
def ts(monkeypatch):
    import mmorch.transcript_store as ts
    monkeypatch.setattr(ts, "_T", {})            # store limpio por test
    emitted: list[dict] = []
    # emit apunta a los logs reales anclados a import-time — se captura en vez
    monkeypatch.setattr(ts, "emit", lambda *a, **k: emitted.append(k))
    ts._emitted = emitted
    return ts


def test_transcript_append_get_roundtrip(ts):
    item = ts.append("j1", "deepseek", "coder", "hola")
    assert item == {"model": "deepseek", "role": "coder", "text": "hola"}
    ts.append("j1", "", "", "x")                 # defaults ante vacios
    assert ts.get("j1")[1] == {"model": "?", "role": "agent", "text": "x"}
    assert ts.get("otro") == []                  # job desconocido: lista vacia, no KeyError
    assert ts._emitted[0]["job_id"] == "j1"      # cada append espeja al SSE via emit


def test_transcript_cap_y_copia(ts):
    ts.append("j", "m", "r", "a" * 9000, cap=100)
    got = ts.get("j")
    assert len(got[0]["text"]) == 100            # cap corta el texto, no revienta
    got.append("intruso")                        # get devuelve COPIA: mutarla no toca el store
    assert len(ts.get("j")) == 1


def test_transcript_endpoint_sirve_el_store(ts, client):
    ts.append("job-w52", "gemini", "verifier", "veredicto")
    r = client.get("/transcript/job-w52", headers=AUTH)
    assert r.status_code == 200
    assert r.json() == [{"model": "gemini", "role": "verifier", "text": "veredicto"}]


# --------------------------------------------------------------------------- #
# server_core — auth + registro durable de jobs
# --------------------------------------------------------------------------- #
def _req(**headers):
    return SimpleNamespace(headers={k.lower(): v for k, v in headers.items()})


def test_token_ok_niega_sin_token_configurado(monkeypatch):
    from mmorch.server_core import _token_ok
    monkeypatch.delenv("MMORCH_SERVER_TOKEN", raising=False)
    # sin token configurado NO hay modo dev: se niega todo, incluso header "correcto"
    assert not _token_ok(_req(authorization="Bearer "))
    assert not _token_ok(_req(**{"x-token": ""}))


def test_token_ok_bearer_y_xtoken(monkeypatch):
    from mmorch.server_core import _token_ok
    monkeypatch.setenv("MMORCH_SERVER_TOKEN", "s3cr3t")
    assert _token_ok(_req(authorization="Bearer s3cr3t"))
    assert _token_ok(_req(**{"x-token": "s3cr3t"}))
    assert not _token_ok(_req(authorization="Bearer otro"))
    assert not _token_ok(_req())


def test_registry_espeja_status_a_jobs_jsonl(home, monkeypatch):
    import mmorch.server_core as sc
    reg = sc._JobRegistry()
    reg["j-w52"] = sc._jobmeta("rubric", "titulo")
    reg["j-w52"]["status"] = "done"              # el cambio de status re-persiste
    lines = [json.loads(x) for x in
             (home / "logs" / "jobs.jsonl").read_text(encoding="utf-8").splitlines()]
    assert [x["status"] for x in lines if x["id"] == "j-w52"] == ["running", "done"]


def test_replay_marca_interrupted_y_respeta_paused(home, monkeypatch):
    import mmorch.server_core as sc
    monkeypatch.setattr(sc, "_JOBS", sc._JobRegistry())
    rows = [
        {"id": "vivo", "status": "running", "kind": "rubric"},
        {"id": "pausado", "status": "paused", "kind": "workflow"},
        {"id": "cerrado", "status": "done", "kind": "rubric"},
    ]
    p = home / "logs" / "jobs.jsonl"
    p.write_text("\n".join(json.dumps(r) for r in rows) + "\n{linea cortada",
                 encoding="utf-8")
    assert sc.load_interrupted_jobs() == ["pausado", "vivo"]
    assert sc._JOBS["vivo"]["status"] == "interrupted"   # el proceso que lo corria murio
    assert sc._JOBS["pausado"]["status"] == "paused"     # sigue resumible tal cual
    assert "cerrado" not in sc._JOBS                     # terminal: solo historial


# --------------------------------------------------------------------------- #
# server_engine — ciclo de vida de un job del engine (fan_out con seam mockeada)
# --------------------------------------------------------------------------- #
def test_run_fanout_job_registra_y_cierra(home, monkeypatch):
    import mmorch.patterns as patterns
    import mmorch.server_engine as se
    reg = se._JOBS.__class__()
    monkeypatch.setattr(se, "_JOBS", reg)
    monkeypatch.setattr(se, "emit", lambda *a, **k: None)
    llamado = {}
    monkeypatch.setattr(patterns, "fan_out",
                        lambda prompts, gen_model: llamado.update(p=prompts, m=gen_model) or ["ok"])
    se._run_fanout_job(["p1", "p2"], "deepseek-chat")
    assert llamado == {"p": ["p1", "p2"], "m": "deepseek-chat"}
    (meta,) = reg.values()
    assert meta["kind"] == "fanout" and meta["status"] == "done"


# --------------------------------------------------------------------------- #
# server_fleet — registro de hosts + pull, token-gated
# --------------------------------------------------------------------------- #
def test_fleet_get_y_post(client, monkeypatch):
    import mmorch.fleet as fleet
    monkeypatch.setattr(fleet, "list_hosts", lambda: [{"name": "peer"}])
    monkeypatch.setattr(fleet, "fleet_state", lambda: {"peer": "up"})
    r = client.get("/fleet", headers=AUTH)
    assert r.status_code == 200
    assert r.json() == {"hosts": [{"name": "peer"}], "state": {"peer": "up"}}

    def _reg(name, url, token):
        if not url:
            raise ValueError("url vacia")
        return {"name": name, "url": url}
    monkeypatch.setattr(fleet, "register_host", _reg)
    ok = client.post("/fleet", headers=AUTH, json={"name": "n", "url": "http://x:1"})
    assert ok.status_code == 200 and ok.json()["registered"]["name"] == "n"
    bad = client.post("/fleet", headers=AUTH, json={"name": "n"})
    assert bad.status_code == 400 and "error" in bad.json()   # register invalido -> 400, no 500
    assert client.get("/fleet").status_code == 401            # y siempre token-gated


def test_sync_pull_devuelve_pull_all(client, monkeypatch):
    import mmorch.sync as sync
    monkeypatch.setattr(sync, "pull_all", lambda: {"notes": 0})
    r = client.post("/sync/pull", headers=AUTH)
    assert r.status_code == 200 and r.json() == {"notes": 0}
    assert client.post("/sync/pull").status_code == 401


# --------------------------------------------------------------------------- #
# server_pty — rutas de terminal (sesion mockeada: sin shell real en el suite HTTP)
# --------------------------------------------------------------------------- #
def test_pty_rutas_sesion_inexistente(client):
    assert client.post("/pty/nope/input", json={"data": "x"}, headers=AUTH).status_code == 404
    assert client.post("/pty/nope/resize", json={}, headers=AUTH).status_code == 404
    r = client.post("/pty/nope/close", headers=AUTH)
    assert r.status_code == 200 and r.json() == {"closed": False}
    assert client.post("/pty/open", json={}).status_code == 401


def test_pty_open_responde_sesion(client, monkeypatch):
    import mmorch.pty_session as ps
    stub = SimpleNamespace(id="pty-w52", cwd="", _backend="stub", alive=True)
    monkeypatch.setattr(ps, "open_session", lambda cwd, rows, cols: stub)
    r = client.post("/pty/open", json={"rows": 20, "cols": 80}, headers=AUTH)
    assert r.status_code == 200
    assert r.json() == {"session": "pty-w52", "cwd": "", "backend": "stub"}


def test_pty_open_respeta_exec_policy(client, monkeypatch):
    import mmorch.exec_policy as ep
    monkeypatch.setattr(ep, "evaluate",
                        lambda pol, kind: {"allowed": False, "reason": "policy w52"})
    r = client.post("/pty/open", json={}, headers=AUTH)
    assert r.status_code == 403 and r.json()["error"] == "policy w52"


# --------------------------------------------------------------------------- #
# pty_session — import + creacion basica de una sesion REAL (shell del OS)
# --------------------------------------------------------------------------- #
def test_pty_session_creacion_basica(tmp_path):
    if os.name == "nt":
        pytest.importorskip("winpty")            # ConPTY: sin pywinpty no hay backend
    from mmorch import pty_session
    try:
        s = pty_session.open_session(cwd=str(tmp_path))
    except OSError as e:                         # entorno sin PTY (CI headless raro)
        pytest.skip(f"PTY no disponible: {e}")
    try:
        assert s.alive and s.id.startswith("pty-")
        assert pty_session.get(s.id) is s
        q = s.subscribe()
        s.write("\r\n")                          # input no revienta con la sesion viva
        assert q in s.subscribers
        s.unsubscribe(q)
    finally:
        assert pty_session.close_session(s.id) is True
    assert pty_session.get(s.id) is None and not s.alive


# --------------------------------------------------------------------------- #
# server_frontend — el HTML estatico habla con rutas que EXISTEN
# --------------------------------------------------------------------------- #
def test_frontend_referencia_rutas_reales():
    from mmorch.server import build_app
    from mmorch.server_frontend import FRONTEND
    assert FRONTEND.startswith("<!DOCTYPE html>")
    rutas = {getattr(r, "path", "") for r in build_app().routes}
    # endpoints que el JS del dashboard pega fijo: si una ruta se renombra sin
    # tocar el frontend, esto lo cacha antes que un usuario con la consola abierta
    for path in ("/events", "/state", "/projects", "/fleet", "/fleet/run",
                 "/run/rubric", "/run/fanout", "/run/project"):
        assert path in rutas, f"el frontend usa {path} y el server ya no la sirve"
        assert path in FRONTEND
