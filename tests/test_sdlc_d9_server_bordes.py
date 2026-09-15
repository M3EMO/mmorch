"""D9 (robustez por modulos: superficie de red `server_fleet` + `server_pty`): entrada malformada
nunca produce un 500. Cero red real: TestClient in-process, token por env, sin abrir shells.

- R1 POST /fleet con body que no es JSON -> 400 y {"error": ...}.
- R2/R4 (pty): rutas borradas con Lotus el 2026-09-15; solo queda fleet.
- R3 POST /fleet/run con host no registrado -> 404 y {"error": ...} (hoy 200 con ok False).
"""
import pytest
from starlette.testclient import TestClient

from mmorch.server import build_app

TOKEN = "d9-secret"
H = {"X-Token": TOKEN, "content-type": "application/json"}


@pytest.fixture
def client(monkeypatch, tmp_path):
    monkeypatch.setenv("MMORCH_SERVER_TOKEN", TOKEN)
    monkeypatch.setenv("MMORCH_HOME", str(tmp_path))       # registro de fleet vacio y aislado
    return TestClient(build_app(), raise_server_exceptions=False)


def test_R1_fleet_body_no_json_es_400(client):
    r = client.post("/fleet", content=b"{esto no es json", headers=H)
    assert r.status_code == 400, (r.status_code, r.text)
    assert "error" in r.json()


def test_R3_fleet_run_host_desconocido_es_404(client):
    r = client.post("/fleet/run", json={"host": "no-existe", "path": "/state", "payload": {}}, headers=H)
    assert r.status_code == 404, (r.status_code, r.text)
    assert "error" in r.json()

