"""W3.2 — server HTTP seguro y durable.

Contratos que fija esta suite:
  1. token OBLIGATORIO: sin MMORCH_SERVER_TOKEN main() rehusa arrancar (mensaje
     claro, no "modo dev") y un app armada igual niega todo request,
  2. token SOLO por header (Authorization: Bearer o X-Token) — query string NO,
  3. GET /health sin auth (200 healthy / 503 no),
  4. jobs durables: cada alta/cambio de status se espeja en logs/jobs.jsonl y el
     replay post-crash lista los no-terminales como 'interrupted' (paused queda
     paused, terminales no vuelven, lineas corruptas no rompen).
"""
from __future__ import annotations

import json

import pytest
from starlette.testclient import TestClient

import mmorch.server_core as core
from mmorch.server import build_app, main


@pytest.fixture()
def aislado(monkeypatch, tmp_path):
    """MMORCH_HOME propio (jobs.jsonl fresco) + registro _JOBS vacio y restaurado.
    _JOBS es global compartido por referencia entre modulos: se muta in-place."""
    monkeypatch.setenv("MMORCH_HOME", str(tmp_path))
    guardado = dict(core._JOBS)
    core._JOBS.clear()
    yield tmp_path
    core._JOBS.clear()
    core._JOBS.update(guardado)


# ---- 1. token obligatorio ---------------------------------------------------- #
def test_main_rehusa_arrancar_sin_token(monkeypatch):
    monkeypatch.delenv("MMORCH_SERVER_TOKEN", raising=False)
    with pytest.raises(SystemExit) as exc:
        main()
    assert "MMORCH_SERVER_TOKEN" in str(exc.value)
    assert "rehusa" in str(exc.value)          # mensaje claro, no un exit mudo


def test_sin_token_configurado_no_hay_modo_dev(monkeypatch):
    # cubre apps armadas sin pasar por main() (p.ej. uvicorn directo): deny-all
    monkeypatch.delenv("MMORCH_SERVER_TOKEN", raising=False)
    c = TestClient(build_app())
    assert c.get("/state").status_code == 401
    assert c.get("/state", headers={"X-Token": ""}).status_code == 401


# ---- 2. token solo por header ------------------------------------------------ #
def test_token_por_header_si_query_no(monkeypatch, aislado):
    monkeypatch.setenv("MMORCH_SERVER_TOKEN", "s3cr3t")
    c = TestClient(build_app())
    assert c.get("/state", headers={"Authorization": "Bearer s3cr3t"}).status_code == 200
    assert c.get("/state", headers={"X-Token": "s3cr3t"}).status_code == 200
    assert c.get("/state?token=s3cr3t").status_code == 401     # URL en logs = leak
    assert c.get("/state", headers={"X-Token": "otro"}).status_code == 401


# ---- 3. /health sin auth ----------------------------------------------------- #
def test_health_sin_auth_200_o_503(monkeypatch, aislado):
    monkeypatch.setenv("MMORCH_SERVER_TOKEN", "s3cr3t")
    import mmorch.health as H
    monkeypatch.setattr(H, "report", lambda **kw: {"healthy": True})
    c = TestClient(build_app())
    r = c.get("/health")                       # sin token: watchdogs externos
    assert r.status_code == 200 and r.json()["healthy"] is True

    monkeypatch.setattr(H, "report", lambda **kw: {"healthy": False, "check": {}})
    assert c.get("/health").status_code == 503


# ---- 4. jobs durables -------------------------------------------------------- #
def _log_lines(tmp_path):
    p = tmp_path / "logs" / "jobs.jsonl"
    return [json.loads(x) for x in p.read_text(encoding="utf-8").splitlines()] if p.exists() else []


def test_alta_y_cambio_de_status_se_espejan(aislado):
    core._JOBS["j1"] = core._jobmeta("fanout", "demo")
    core._JOBS["j1"]["status"] = "done"
    recs = _log_lines(aislado)
    assert [r["status"] for r in recs if r["id"] == "j1"] == ["running", "done"]
    # heartbeat/state mutan seguido y NO se persisten (no amplificar escrituras)
    core._JOBS["j1"]["heartbeat"] = 123.0
    assert len(_log_lines(aislado)) == len(recs)


def test_replay_post_crash_lista_interrupted(aislado):
    core._JOBS["vivo"] = core._jobmeta("fanout", "quedo corriendo")
    core._JOBS["listo"] = core._jobmeta("rubric", "termino")
    core._JOBS["listo"]["status"] = "done"
    core._JOBS["pausado"] = core._jobmeta("project", "en pausa")
    core._JOBS["pausado"]["status"] = "paused"

    core._JOBS.clear()                         # "crash": muere el proceso, queda el jsonl
    assert core.load_interrupted_jobs() == ["pausado", "vivo"]
    assert core._JOBS["vivo"]["status"] == "interrupted"
    assert core._JOBS["pausado"]["status"] == "paused"      # sigue resumible
    assert "listo" not in core._JOBS           # terminal: solo historial


def test_replay_ignora_lineas_corruptas_y_no_pisa_vivos(aislado):
    log = aislado / "logs"
    log.mkdir(exist_ok=True)
    (log / "jobs.jsonl").write_text(
        json.dumps({"id": "a", "status": "running", "kind": "fanout"}) + "\n"
        + '{"id": "cortada", "sta'          # write cortado por el crash
        + "\n" + json.dumps({"id": "b", "status": "running", "kind": "rubric"}) + "\n",
        encoding="utf-8")
    core._JOBS["b"] = core._jobmeta("rubric", "ya relanzado")   # vivo en ESTE proceso
    assert core.load_interrupted_jobs() == ["a"]
    assert core._JOBS["b"]["status"] == "running"               # no pisado


def test_replay_es_estable(aislado):
    core._JOBS["j"] = core._jobmeta("fanout", "x")
    core._JOBS.clear()
    assert core.load_interrupted_jobs() == ["j"]
    core._JOBS.clear()                         # segundo restart consecutivo
    assert core.load_interrupted_jobs() == ["j"]
    assert core._JOBS["j"]["status"] == "interrupted"
