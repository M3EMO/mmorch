"""Review D9: fleet_run clasifica mal errores 'ok: False' que no son sobre el host.

R3 (docs/sdlc/spec.md) pide 404 SOLO cuando el error dice que el host no esta
registrado; cualquier otro motivo de 'ok' False debe dar 502. La implementacion
matchea por substring ("no existe", "not found") sobre CUALQUIER error de forward(),
no solo sobre errores de host-no-registrado -> un error remoto no relacionado con
el registro de hosts (p.ej. "recurso not found" de otra causa) cae en 404 en vez
de 502.
"""
from starlette.testclient import TestClient

from mmorch.server import build_app

TOKEN = "d9-review-secret"
H = {"X-Token": TOKEN, "content-type": "application/json"}


def test_R3_fleet_run_error_no_relacionado_con_host_debe_ser_502(monkeypatch, tmp_path):
    monkeypatch.setenv("MMORCH_SERVER_TOKEN", TOKEN)
    monkeypatch.setenv("MMORCH_HOME", str(tmp_path))
    client = TestClient(build_app(), raise_server_exceptions=False)

    def fake_forward(host, path, payload, **kw):
        # ok False por un motivo ajeno a "host no registrado" (contiene "not found").
        return {"ok": False, "error": "recurso remoto not found"}

    monkeypatch.setattr("mmorch.fleet.forward", fake_forward)

    r = client.post(
        "/fleet/run",
        json={"host": "mateo", "path": "/x", "payload": {}},
        headers=H,
    )
    assert r.status_code == 502, (r.status_code, r.text)
