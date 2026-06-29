"""server_pty — interactive PTY (terminal) routes: open/stream/input/resize/close a shell
bound to a project cwd. Self-contained leaf — depends only on server_core auth + events +
(lazily) pty_session. Token-gated like every route.
"""
from __future__ import annotations

import json

from .events import emit
from .server_core import _token_ok


async def pty_open(request):
    """Spawn an interactive shell bound to a project's cwd. Token-gated; see pty_session."""
    from starlette.responses import JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    body = await request.json()
    cwd = body.get("cwd") or None
    proj = body.get("project")
    if proj and not cwd:
        try:
            from .projects import resolve
            cwd = resolve(proj)
        except Exception:
            cwd = None
    rows = int(body.get("rows", 30)); cols = int(body.get("cols", 100))
    from .exec_policy import current_policy, evaluate
    dec = evaluate(current_policy(), "local")          # PTY is a local shell
    if not dec["allowed"]:
        return JSONResponse({"error": dec["reason"]}, status_code=403)
    from . import pty_session
    try:
        s = pty_session.open_session(cwd, rows, cols)
    except Exception as e:
        return JSONResponse({"error": str(e)[:160]}, status_code=429)
    emit("pty", "running", job_id=s.id, detail=f"shell @ {s.cwd or 'home'}")
    return JSONResponse({"session": s.id, "cwd": s.cwd or "", "backend": s._backend})


async def pty_stream(request):
    from starlette.responses import StreamingResponse, JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    from . import pty_session
    s = pty_session.get(request.path_params["sid"])
    if not s:
        return JSONResponse({"error": "no session"}, status_code=404)
    q = s.subscribe()

    def gen():
        try:
            while True:
                try:
                    data = q.get(timeout=15)
                    yield f"data: {json.dumps({'data': data})}\n\n"
                except Exception:
                    if not s.alive:
                        yield f"data: {json.dumps({'exit': True})}\n\n"
                        break
                    yield ": keepalive\n\n"
        finally:
            s.unsubscribe(q)
    return StreamingResponse(gen(), media_type="text/event-stream")


async def pty_input(request):
    from starlette.responses import JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    from . import pty_session
    s = pty_session.get(request.path_params["sid"])
    if not s or not s.alive:
        return JSONResponse({"error": "no session"}, status_code=404)
    body = await request.json()
    s.write(body.get("data", ""))
    return JSONResponse({"ok": True})


async def pty_resize(request):
    from starlette.responses import JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    from . import pty_session
    s = pty_session.get(request.path_params["sid"])
    if not s:
        return JSONResponse({"error": "no session"}, status_code=404)
    body = await request.json()
    s.resize(int(body.get("rows", 30)), int(body.get("cols", 100)))
    return JSONResponse({"ok": True})


async def pty_close(request):
    from starlette.responses import JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    from . import pty_session
    ok = pty_session.close_session(request.path_params["sid"])
    return JSONResponse({"closed": ok})


