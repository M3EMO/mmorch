"""server — mmorch VISUAL nivel 3: progreso live de cada subagente + control TOTAL remoto.

Arquitectura (sintetizada de un ideate cross-family, las 3 alternativas naive refutadas):
- Starlette (ya instalado, CERO dep nueva) + uvicorn. NO se hand-rollea WebSocket; NO se usa
  MCP como transporte de browser; NO se tailea el JSONL cross-process.
- El SERVER corre los jobs IN-PROCESS (importa mmorch, llama fan_out/rubric_loop) -> tiene los
  eventos en memoria via events.bus() y los streamea por SSE. Cero race con el JSONL (que sigue
  siendo el audit durable).
- Control total: lanzar/matar jobs, aprobar gates. Auth por token (env MMORCH_SERVER_TOKEN).
  Bind a la IP del tailnet (env MMORCH_SERVER_HOST, default 127.0.0.1). EventSource no manda
  headers -> el token tambien se acepta por ?token=.

SEGURIDAD (decision informada del usuario): control total remoto. El gate humano se ejerce
remoto-pero-autenticado por tunel PRIVADO (Tailscale). mmorch NO auto-aplica red-zone solo;
VOS aprobas. BudgetKeeper/goal_guard siguen activos como red de seguridad override-able.
Correr SOLO detras de un tunel privado, NUNCA 0.0.0.0 a internet sin token.

Run:  MMORCH_SERVER_TOKEN=xxx MMORCH_SERVER_HOST=<tailnet-ip> \
      .venv/Scripts/python.exe -m mmorch.server
"""
from __future__ import annotations

import json
import os
import threading
import time
import uuid

from .events import bus, emit, Event

_JOBS: dict[str, dict] = {}
_JOBS_LOCK = threading.Lock()


def _token_ok(request) -> bool:
    want = os.getenv("MMORCH_SERVER_TOKEN", "")
    if not want:
        return True   # sin token configurado = modo dev (bindeá a localhost!)
    got = request.headers.get("x-token") or request.query_params.get("token", "")
    return got == want


# --- jobs in-process -------------------------------------------------------- #
def _jobmeta(kind: str, title: str, **extra) -> dict:
    """Registro de job con title/ts/host -> alimenta el Kanban (columnas por status)."""
    return {"status": "running", "kind": kind, "title": (title or kind)[:80],
            "ts": time.time(), "host": os.getenv("MMORCH_SERVER_HOST", "local"), **extra}


def _run_rubric_job(task: str, criteria: list, K: int, gen_model, judge_model, parent=None):
    from .rubric_loop import start_rubric, next_action, submit
    from .providers import call
    state = start_rubric(task, criteria, K=K, gen_model=gen_model, judge_model=judge_model)
    jid = state["id"]
    cancel = threading.Event()
    with _JOBS_LOCK:
        _JOBS[jid] = _jobmeta("rubric", task, cancel=cancel, state=state, parent=parent)
    emit("job", "running", job_id=jid, detail=task[:120])

    def fn(model):
        def _c(prompt):
            return call(model, [{"role": "user", "content": prompt}],
                        pattern="rubric_loop", node=model).text
        return _c
    gen_fn, judge_fn = fn(state["gen_model"]), fn(state["judge_model"])
    try:
        while True:
            if cancel.is_set():
                emit("job", "error", job_id=jid, detail="cancelado por el usuario")
                break
            act = next_action(state)
            if act["role"] in ("done", "escalate"):
                break
            out = gen_fn(act["prompt"]) if act["role"] == "executor" else judge_fn(act["prompt"])
            from . import transcript_store
            model = state["gen_model"] if act["role"] == "executor" else state["judge_model"]
            transcript_store.append(jid, model, act["role"], out)
            submit(state, out)
    except Exception as e:
        emit("job", "error", job_id=jid, detail=str(e)[:200])
    with _JOBS_LOCK:
        if jid in _JOBS:
            _JOBS[jid]["status"] = state.get("phase", "done")


def _run_project_job(project: str, task: str, mode: str, push: bool = False,
                     engine: str = "mmorch", target_file: str = "", test_cmd: str | None = None,
                     parent=None):
    """Job project-aware. engine PRIMARIO = 'mmorch' (DeepSeek genera + tests verifican +
    aplica determinista, cero cupo; escala a claude -p si no puede). engine='claude' =
    claude -p directo (plan/cupo) — para tareas abiertas que mmorch no banca."""
    import uuid as _u
    jid = _u.uuid4().hex[:10]
    with _JOBS_LOCK:
        _JOBS[jid] = _jobmeta("project", task, engine=engine, parent=parent)
    try:
        if engine == "mmorch":
            if not target_file:
                emit("job", "error", job_id=jid, detail="engine=mmorch requiere target_file")
                with _JOBS_LOCK:
                    _JOBS[jid]["status"] = "error"
                return
            from .project_loop import run_project_task
            r = run_project_task(project, task, target_file=target_file, test_cmd=test_cmd,
                                 push=push, job_id=jid)
            ok = r.ok
        else:   # engine == claude: claude -p directo (cupo)
            from .projects import resolve
            from .claude_exec import run_claude
            cwd = resolve(project)
            emit("job", "running", job_id=jid, detail=f"claude {project} [{mode}]: {task[:70]}")
            res = run_claude(task, cwd, mode=mode, job_id=jid)
            ok = bool(res.get("ok"))
            if ok and mode == "edit" and push:
                from .sync import commit_push
                commit_push(cwd, f"mmorch(claude): {task[:64]}", job_id=jid)
    except Exception as e:
        emit("job", "error", job_id=jid, detail=str(e)[:160])
        ok = False
    with _JOBS_LOCK:
        _JOBS[jid]["status"] = "done" if ok else "error"


def _run_fanout_job(prompts: list, gen_model: str, parent=None):
    from .patterns import fan_out
    jid = uuid.uuid4().hex[:10]
    with _JOBS_LOCK:
        _JOBS[jid] = _jobmeta("fanout", f"fan_out x{len(prompts)}", parent=parent)
    emit("job", "running", job_id=jid, detail=f"fan_out x{len(prompts)}")
    try:
        res = fan_out(prompts, gen_model=gen_model)
        emit("job", "done", job_id=jid, detail=f"{len(res)}/{len(prompts)} ok")
    except Exception as e:
        emit("job", "error", job_id=jid, detail=str(e)[:200])
    with _JOBS_LOCK:
        if jid in _JOBS:
            _JOBS[jid]["status"] = "done"


# --- handlers --------------------------------------------------------------- #
async def home(request):
    from starlette.responses import HTMLResponse
    return HTMLResponse(_FRONTEND)


async def state_snapshot(request):
    from starlette.responses import JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    from .metrics import summary, error_rates, cache_stats
    from .budget import status as bstatus
    from .nodes import sections, conductor
    from .exec_policy import current_policy
    with _JOBS_LOCK:
        jobs = {k: {"status": v["status"], "kind": v["kind"], "title": v.get("title", ""),
                    "ts": v.get("ts", 0), "host": v.get("host", "local"),
                    "engine": v.get("engine", ""), "parent": v.get("parent")} for k, v in _JOBS.items()}
    return JSONResponse({
        "conductor": conductor(), "sections": sections(), "summary": summary(),
        "error_rates": error_rates(window_n=200), "cache": cache_stats(window_n=200),
        "budget": bstatus(), "jobs": jobs, "exec_policy": current_policy(),
        "recent": [e.to_dict() for e in bus().recent(50)],
    })


async def sse_events(request):
    from starlette.responses import StreamingResponse
    if not _token_ok(request):
        from starlette.responses import JSONResponse
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    q = bus().subscribe()

    def gen():
        for e in bus().recent(30):
            yield f"data: {json.dumps(e.to_dict(), default=str)}\n\n"
        try:
            while True:
                try:
                    ev = q.get(timeout=15)
                    yield f"data: {json.dumps(ev.to_dict(), default=str)}\n\n"
                except Exception:
                    yield ": keepalive\n\n"
        finally:
            bus().unsubscribe(q)
    return StreamingResponse(gen(), media_type="text/event-stream")


async def run_rubric(request):
    from starlette.responses import JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    body = await request.json()
    task = body.get("task", ""); criteria = body.get("criteria", []); K = int(body.get("K", 5))
    gm = body.get("gen_model"); jm = body.get("judge_model")
    t = threading.Thread(target=_run_rubric_job, args=(task, criteria, K, gm, jm),
                         kwargs={"parent": body.get("parent_id")}, daemon=True)
    t.start()
    return JSONResponse({"started": "rubric", "task": task[:80]})


async def run_fanout(request):
    from starlette.responses import JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    body = await request.json()
    prompts = body.get("prompts", []); gm = body.get("gen_model", "deepseek-chat")
    t = threading.Thread(target=_run_fanout_job, args=(prompts, gm),
                         kwargs={"parent": body.get("parent_id")}, daemon=True)
    t.start()
    return JSONResponse({"started": "fanout", "n": len(prompts)})


async def projects_handler(request):
    from starlette.responses import JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    from .projects import list_projects, register
    if request.method == "POST":
        body = await request.json()
        try:
            r = register(body.get("name", ""), body.get("path", ""))
            return JSONResponse({"registered": r})
        except Exception as e:
            return JSONResponse({"error": str(e)[:200]}, status_code=400)
    return JSONResponse({"projects": list_projects()})


async def run_project(request):
    from starlette.responses import JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    body = await request.json()
    project = body.get("project", ""); task = body.get("task", "")
    mode = body.get("mode", "plan"); push = bool(body.get("push", False))
    engine = body.get("engine", "mmorch")        # PRIMARIO = mmorch (barato); claude = escalada
    target_file = body.get("target_file", ""); test_cmd = body.get("test_cmd")
    if mode not in ("plan", "edit", "read"):
        return JSONResponse({"error": "mode invalido (plan|edit)"}, status_code=400)
    if engine not in ("mmorch", "claude"):
        return JSONResponse({"error": "engine invalido (mmorch|claude)"}, status_code=400)
    from .exec_policy import current_policy, evaluate
    dec = evaluate(current_policy(), "local")          # project jobs execute locally
    if not dec["allowed"]:
        return JSONResponse({"error": dec["reason"]}, status_code=403)
    t = threading.Thread(target=_run_project_job,
                         args=(project, task, mode, push, engine, target_file, test_cmd),
                         kwargs={"parent": body.get("parent_id")}, daemon=True)
    t.start()
    return JSONResponse({"started": "project", "project": project, "engine": engine, "mode": mode})


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


async def kill_job(request):
    from starlette.responses import JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    jid = request.path_params["job_id"]
    with _JOBS_LOCK:
        j = _JOBS.get(jid)
    if j and j.get("cancel"):
        j["cancel"].set()
        emit("job", "gate", job_id=jid, detail="kill solicitado (best-effort entre pasos)")
        return JSONResponse({"killed": jid})
    return JSONResponse({"error": "job no cancelable o inexistente"}, status_code=404)


async def approve_job(request):
    from starlette.responses import JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    jid = request.path_params["job_id"]
    emit("job", "done", job_id=jid, detail="APROBADO por humano (gate remoto)")
    with _JOBS_LOCK:
        if jid in _JOBS:
            _JOBS[jid]["status"] = "approved"
    return JSONResponse({"approved": jid})


def _chat_reply(text: str) -> dict:
    """Store the user msg, generate a terse reply via a cheap model (cero cupo), store + return it."""
    from . import chat_store
    chat_store.add("user", text)
    reply, engine = "", ""
    try:
        from .providers import call
        from .config import DEFAULT_GENERATOR
        sysmsg = ("You are Lotus, a terse coding assistant backed by mmorch. If the user describes a "
                  "coding task, say briefly how you'd route it (project edit / rubric / fan_out) and "
                  "what you need (project, target file). Otherwise answer directly. Keep it short.")
        r = call(DEFAULT_GENERATOR, [{"role": "system", "content": sysmsg},
                                     {"role": "user", "content": text}],
                 pattern="chat", node="chat", max_tokens=512)
        reply = (r.text or "").strip()
        engine = DEFAULT_GENERATOR
    except Exception as e:
        reply = f"(mmorch offline: {str(e)[:120]})"
    return chat_store.add("assistant", reply or "(no reply)", engine=engine)


async def chat_handler(request):
    from starlette.responses import JSONResponse
    from starlette.concurrency import run_in_threadpool
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    body = await request.json()
    text = (body.get("message") or "").strip()
    if not text:
        return JSONResponse({"error": "mensaje vacio"}, status_code=400)
    msg = await run_in_threadpool(_chat_reply, text)   # model call off the event loop
    return JSONResponse({"message": msg})


async def chat_history(request):
    from starlette.responses import JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    from . import chat_store
    before = request.query_params.get("before")
    limit = int(request.query_params.get("limit", 30))
    return JSONResponse(chat_store.history(before, limit))


async def minds_handler(request):
    """Global codegraph federation across registered projects (read-only)."""
    from starlette.responses import JSONResponse
    from starlette.concurrency import run_in_threadpool
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    from .minds import federation
    return JSONResponse(await run_in_threadpool(federation))


async def transcript_handler(request):
    """Inter-agent transcript for a job (Lotus reads this; SSE mirrors live items)."""
    from starlette.responses import JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    from .transcript_store import get
    return JSONResponse(get(request.path_params["job_id"]))


async def job_ancestry(request):
    """Lineage of a job (graft G1): ancestors up + descendants down (adjacency-list)."""
    from starlette.responses import JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    from . import job_graph
    with _JOBS_LOCK:
        jobs = {k: {"parent": v.get("parent")} for k, v in _JOBS.items()}
    return JSONResponse(job_graph.tree(jobs, request.path_params["job_id"]))


# --- interactive PTY (writable terminal) ------------------------------------ #
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


def build_app():
    from starlette.applications import Starlette
    from starlette.routing import Route
    from starlette.middleware import Middleware
    from starlette.middleware.cors import CORSMiddleware
    # CORS: the real gate is the token + private tunnel, not the Origin. The Lotus
    # client (Tauri / dev server) is cross-origin, so allow any origin here.
    middleware = [Middleware(CORSMiddleware, allow_origins=["*"],
                             allow_methods=["*"], allow_headers=["*"])]
    return Starlette(middleware=middleware, routes=[
        Route("/", home),
        Route("/state", state_snapshot),
        Route("/events", sse_events),
        Route("/run/rubric", run_rubric, methods=["POST"]),
        Route("/run/fanout", run_fanout, methods=["POST"]),
        Route("/projects", projects_handler, methods=["GET", "POST"]),
        Route("/run/project", run_project, methods=["POST"]),
        Route("/chat", chat_handler, methods=["POST"]),
        Route("/chat/history", chat_history, methods=["GET"]),
        Route("/minds", minds_handler),
        Route("/transcript/{job_id}", transcript_handler),
        Route("/jobs/{job_id}/ancestry", job_ancestry),
        Route("/pty/open", pty_open, methods=["POST"]),
        Route("/pty/{sid}/stream", pty_stream),
        Route("/pty/{sid}/input", pty_input, methods=["POST"]),
        Route("/pty/{sid}/resize", pty_resize, methods=["POST"]),
        Route("/pty/{sid}/close", pty_close, methods=["POST"]),
        Route("/sync/pull", sync_pull, methods=["POST"]),
        Route("/fleet", fleet_handler, methods=["GET", "POST"]),
        Route("/fleet/run", fleet_run, methods=["POST"]),
        Route("/kill/{job_id}", kill_job, methods=["POST"]),
        Route("/approve/{job_id}", approve_job, methods=["POST"]),
    ])


_FRONTEND = """<!DOCTYPE html><html><head><meta charset=utf-8>
<title>mmorch live</title><meta name=viewport content="width=device-width,initial-scale=1">
<style>
body{font-family:system-ui,sans-serif;margin:0;background:#0d0d0f;color:#e8e8ea}
header{padding:12px 16px;border-bottom:1px solid #26262b;display:flex;gap:12px;align-items:center}
h1{font-size:15px;font-weight:500;margin:0}input,button,textarea{font:inherit;background:#16161a;color:#e8e8ea;border:1px solid #2c2c33;border-radius:8px;padding:6px 10px}
button{cursor:pointer}button:hover{background:#22222a}
main{display:grid;grid-template-columns:1fr 1fr;gap:16px;padding:16px}
.card{background:#141418;border:1px solid #26262b;border-radius:12px;padding:12px}
.ev{font-size:13px;padding:4px 8px;border-left:3px solid #444;margin:3px 0;display:flex;gap:8px;align-items:center}
.running{border-color:#d8a13a}.done{border-color:#3a9e6f}.error{border-color:#c0453a}.gate{border-color:#7f77dd}.pending{border-color:#555}
.dot{width:8px;height:8px;border-radius:50%;flex:none}
.muted{color:#8a8a92;font-size:12px}.row{display:flex;gap:6px;flex-wrap:wrap;align-items:center}
#feed{max-height:60vh;overflow:auto}textarea{width:100%;min-height:60px}
.pill{font-size:11px;padding:2px 8px;border-radius:10px;background:#1e1e24;color:#b8b8c0}
</style></head><body>
<header><h1>mmorch · live</h1><span class=muted id=conn>conectando…</span>
<input id=token placeholder="token" style="margin-left:auto;width:140px"><button onclick=connect()>conectar</button></header>
<main>
<div class=card><div class=row><strong>subagentes</strong><span class=pill id=cnt>0</span>
<button onclick=clearfeed() style="margin-left:auto">limpiar</button></div><div id=feed></div></div>
<div class=card><strong>control</strong>
<div class=row style="margin-top:4px"><span class=muted>destino</span>
<select id=target style="flex:1"><option value=local>local (este host)</option></select></div>
<p class=muted>rubric_loop (tarea + 1 criterio checkable de ejemplo)</p>
<textarea id=task placeholder="implementa inc(x)=x+1"></textarea>
<div class=row style="margin-top:8px"><button onclick=runRubric()>▶ run rubric</button>
<button onclick=runFan()>▶ run fan_out</button><button onclick=loadState()>↻ estado</button></div>
<hr style="border:none;border-top:1px solid #26262b;margin:10px 0">
<p class=muted>project-aware · PRIMARIO mmorch (barato, cero cupo) · claude = escalada (plan)</p>
<div class=row><select id=proj style="flex:1"></select><button onclick=loadProjects()>↻</button></div>
<div class=row style="margin-top:6px"><input id=ptask placeholder="instruccion" style="flex:1"></div>
<div class=row style="margin-top:6px"><input id=pfile placeholder="archivo (ej app.py)" style="flex:1">
<input id=ptest placeholder="test_cmd (ej python -m pytest -q)" style="flex:1"></div>
<div class=row style="margin-top:6px"><button onclick="runMmorch()">▶ mmorch (barato)</button>
<button onclick="runClaude('plan')">claude analizar</button><button onclick="runClaude('edit')">claude editar</button></div>
<pre id=state class=muted style="white-space:pre-wrap;max-height:20vh;overflow:auto"></pre></div>
<div class=card style="grid-column:1/-1"><div class=row><strong>kanban</strong>
<span class=muted>jobs por estado</span><button onclick=loadState() style="margin-left:auto">↻</button></div>
<div id=kanban style="display:grid;grid-template-columns:repeat(5,1fr);gap:8px;margin-top:8px"></div></div>
<div class=card style="grid-column:1/-1"><div class=row><strong>fleet</strong>
<span class=muted>hosts del tailnet</span><button onclick=loadFleet() style="margin-left:auto">↻</button></div>
<div class=row style="margin-top:6px"><input id=hname placeholder="nombre" style="width:90px">
<input id=hurl placeholder="http://100.x.x.x:8787" style="flex:1"><input id=htok placeholder="token" style="width:120px">
<button onclick=addHost()>+ host</button></div>
<div id=fleet style="margin-top:8px"></div></div>
</main>
<script>
let T='';
function connect(){T=document.getElementById('token').value;
 const es=new EventSource('/events?token='+encodeURIComponent(T));
 es.onopen=()=>document.getElementById('conn').textContent='live';
 es.onerror=()=>document.getElementById('conn').textContent='desconectado';
 es.onmessage=e=>addEv(JSON.parse(e.data));loadState();loadProjects();loadFleet();}
function addEv(ev){const f=document.getElementById('feed');const d=document.createElement('div');
 d.className='ev '+(ev.status||'pending');
 d.innerHTML='<span class=dot></span><b>'+(ev.node||ev.type)+'</b><span class=muted>'+ev.status+'</span> '+(ev.detail||'');
 f.prepend(d);document.getElementById('cnt').textContent=f.children.length;}
function clearfeed(){document.getElementById('feed').innerHTML='';document.getElementById('cnt').textContent=0;}
function H(){return {'Content-Type':'application/json','X-Token':T};}
// rutea el job al destino elegido: local -> /run/*, host del fleet -> /fleet/run (server->server)
function submitJob(path,payload){const t=document.getElementById('target').value;
 if(!t||t==='local'){return fetch(path,{method:'POST',headers:H(),body:JSON.stringify(payload)});}
 return fetch('/fleet/run',{method:'POST',headers:H(),body:JSON.stringify({host:t,path:path,payload:payload})});}
function runRubric(){const task=document.getElementById('task').value||'implementa inc(x)=x+1';
 submitJob('/run/rubric',{task,K:5,criteria:[
  {id:'c1',desc:'inc pasa',kind:'checkable',checker:'python_exec',ctx:{code:'{attempt_code}\\nassert inc(1)==2'}}]});}
function runFan(){submitJob('/run/fanout',{prompts:['di hola','di chau','di test']});}
function loadState(){fetch('/state?token='+encodeURIComponent(T)).then(r=>r.json()).then(s=>{
 document.getElementById('state').textContent=JSON.stringify({calls:s.summary&&s.summary.calls,cost:s.summary&&s.summary.total_cost_usd,sections:s.sections,budget:s.budget},null,2);
 renderKanban(s.jobs||{});});}
const COLS=['queued','running','done','error','gate'];
function renderKanban(jobs){const k=document.getElementById('kanban');k.innerHTML='';
 const by={};COLS.forEach(c=>by[c]=[]);
 Object.entries(jobs).forEach(([id,j])=>{const st=COLS.includes(j.status)?j.status:(j.status==='approved'?'done':'running');by[st].push([id,j]);});
 COLS.forEach(c=>{const col=document.createElement('div');col.style='background:#141418;border:1px solid #26262b;border-radius:8px;padding:6px;min-height:60px';
  col.innerHTML='<div class=muted style="font-size:11px;text-transform:uppercase;margin-bottom:4px">'+c+' ('+by[c].length+')</div>';
  by[c].forEach(([id,j])=>{const card=document.createElement('div');card.className='ev '+c;card.style='font-size:11px;margin:3px 0;padding:4px 6px';
   card.innerHTML='<b>'+(j.kind||'')+'</b> '+(j.title||id)+'<br><span class=muted>'+(j.host||'')+(j.engine?(' · '+j.engine):'')+'</span>';col.appendChild(card);});
  k.appendChild(col);});}
function loadFleet(){fetch('/fleet?token='+encodeURIComponent(T)).then(r=>r.json()).then(s=>{
 const f=document.getElementById('fleet');f.innerHTML='';const st=(s.state&&s.state.hosts)||{};
 const tg=document.getElementById('target');const cur=tg.value;          // repuebla el dropdown destino
 tg.innerHTML='<option value=local>local (este host)</option>';
 Object.entries(s.hosts||{}).forEach(([n,h])=>{const hs=st[n]||{};
  const o=document.createElement('option');o.value=n;o.textContent=n;tg.appendChild(o);
  const div=document.createElement('div');div.className='ev '+(hs.ok?'done':'error');
  const calls=hs.summary?hs.summary.calls:'?';
  div.innerHTML='<b>'+n+'</b> '+h.url+' <span class=muted>'+(hs.ok?('ok · '+calls+' calls'):'caido')+'</span>';
  const b=document.createElement('button');b.textContent='usar';b.style='margin-left:auto;font-size:11px;padding:2px 8px';
  b.onclick=()=>{tg.value=n;};div.appendChild(b);f.appendChild(div);});
 tg.value=[...tg.options].some(o=>o.value===cur)?cur:'local';}); }
function addHost(){const name=document.getElementById('hname').value,url=document.getElementById('hurl').value,token=document.getElementById('htok').value;
 if(!name||!url){alert('nombre + url');return;}
 fetch('/fleet',{method:'POST',headers:H(),body:JSON.stringify({name,url,token})}).then(()=>loadFleet());}
function loadProjects(){fetch('/projects?token='+encodeURIComponent(T)).then(r=>r.json()).then(s=>{
 const sel=document.getElementById('proj');sel.innerHTML='';
 Object.keys(s.projects||{}).forEach(n=>{const o=document.createElement('option');o.value=n;o.textContent=n;sel.appendChild(o);});});}
function runMmorch(){const project=document.getElementById('proj').value;const task=document.getElementById('ptask').value;
 const target_file=document.getElementById('pfile').value;const test_cmd=document.getElementById('ptest').value||null;
 if(!project||!task||!target_file){alert('mmorch necesita proyecto + instruccion + archivo');return;}
 submitJob('/run/project',{project,task,engine:'mmorch',target_file,test_cmd,push:true});}
function runClaude(mode){const project=document.getElementById('proj').value;const task=document.getElementById('ptask').value;
 if(!project||!task){alert('elegí proyecto + instruccion');return;}
 submitJob('/run/project',{project,task,engine:'claude',mode,push:mode==='edit'});}
</script></body></html>"""


def main():
    import uvicorn
    host = os.getenv("MMORCH_SERVER_HOST", "127.0.0.1")
    port = int(os.getenv("MMORCH_SERVER_PORT", "8787"))
    if not os.getenv("MMORCH_SERVER_TOKEN"):
        print("WARN: MMORCH_SERVER_TOKEN no seteado -> sin auth. Bindeá a localhost/tailnet.")
    print(f"mmorch live -> http://{host}:{port}  (token {'ON' if os.getenv('MMORCH_SERVER_TOKEN') else 'OFF'})")
    uvicorn.run(build_app(), host=host, port=port, log_level="warning")


if __name__ == "__main__":
    main()
