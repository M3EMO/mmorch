"""server_core — shared in-process state + tiny request helpers for the server route modules.

One home for the mutable job registry and the staged-gate state, so every route group imports
the SAME objects (importing a module global shares the object by reference, not a copy). This is
what lets the routes be split into cohesive modules without circular imports: leaf route modules
depend on server_core, and server.py depends on both.
"""
from __future__ import annotations

import os
import threading
import time

_JOBS: dict[str, dict] = {}
_JOBS_LOCK = threading.Lock()
_GATES: dict[str, dict] = {}   # graft G6: per-job staged gate state


def _token_ok(request) -> bool:
    want = os.getenv("MMORCH_SERVER_TOKEN", "")
    if not want:
        return True   # sin token configurado = modo dev (bindeá a localhost!)
    got = request.headers.get("x-token") or request.query_params.get("token", "")
    return got == want


def _budget_block():
    """Return a 402 JSONResponse if a hard budget policy is exceeded, else None (graft G5)."""
    from starlette.responses import JSONResponse
    from .budget_policy import blocking_incident
    inc = blocking_incident()
    if inc:
        return JSONResponse(
            {"error": f"budget hard-stop on '{inc['scope']}' (${inc['spent']} / ${inc['limit']})",
             "incident": inc}, status_code=402)
    return None


# --- jobs in-process -------------------------------------------------------- #
def _jobmeta(kind: str, title: str, **extra) -> dict:
    """Registro de job con title/ts/host -> alimenta el Kanban (columnas por status)."""
    return {"status": "running", "kind": kind, "title": (title or kind)[:80],
            "ts": time.time(), "host": os.getenv("MMORCH_SERVER_HOST", "local"), **extra}
