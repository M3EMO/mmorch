"""server_fleet — multi-host (tailnet) routes: register/list fleet hosts, proxy a job to a
peer mmorch (server->server), and pull a peer's state. Self-contained group; depends on
server_core auth only (host registry + HTTP proxying are lazy-imported).
"""
from __future__ import annotations

import json

from .server_core import _token_ok


async def sync_pull(request):
    from starlette.responses import JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    from .sync import pull_all
    return JSONResponse(pull_all())


async def fleet_handler(request):
    from starlette.responses import JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    from .fleet import list_hosts, register_host, fleet_state
    if request.method == "POST":
        body = await request.json()
        try:
            r = register_host(body.get("name", ""), body.get("url", ""), body.get("token", ""))
            return JSONResponse({"registered": r})
        except Exception as e:
            return JSONResponse({"error": str(e)[:200]}, status_code=400)
    return JSONResponse({"hosts": list_hosts(), "state": fleet_state()})


async def fleet_run(request):
    """Forwardea un job a otro host del fleet (server->server). body: {host, path, payload}."""
    from starlette.responses import JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    from .fleet import forward
    body = await request.json()
    host = body.get("host", ""); path = body.get("path", "/run/project")
    payload = body.get("payload", {})
    return JSONResponse(forward(host, path, payload))


