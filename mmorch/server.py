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
def _run_rubric_job(task: str, criteria: list, K: int, gen_model, judge_model):
    from .rubric_loop import start_rubric, next_action, submit
    from .providers import call
    state = start_rubric(task, criteria, K=K, gen_model=gen_model, judge_model=judge_model)
    jid = state["id"]
    cancel = threading.Event()
    with _JOBS_LOCK:
        _JOBS[jid] = {"cancel": cancel, "state": state, "status": "running", "kind": "rubric"}
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
            submit(state, out)
    except Exception as e:
        emit("job", "error", job_id=jid, detail=str(e)[:200])
    with _JOBS_LOCK:
        if jid in _JOBS:
            _JOBS[jid]["status"] = state.get("phase", "done")


def _run_project_job(project: str, task: str, mode: str, push: bool = False,
                     engine: str = "mmorch", target_file: str = "", test_cmd: str | None = None):
    """Job project-aware. engine PRIMARIO = 'mmorch' (DeepSeek genera + tests verifican +
    aplica determinista, cero cupo; escala a claude -p si no puede). engine='claude' =
    claude -p directo (plan/cupo) — para tareas abiertas que mmorch no banca."""
    import uuid as _u
    jid = _u.uuid4().hex[:10]
    with _JOBS_LOCK:
        _JOBS[jid] = {"status": "running", "kind": "project", "engine": engine}
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


def _run_fanout_job(prompts: list, gen_model: str):
    from .patterns import fan_out
    jid = uuid.uuid4().hex[:10]
    with _JOBS_LOCK:
        _JOBS[jid] = {"status": "running", "kind": "fanout"}
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
    with _JOBS_LOCK:
        jobs = {k: {"status": v["status"], "kind": v["kind"]} for k, v in _JOBS.items()}
    return JSONResponse({
        "conductor": conductor(), "sections": sections(), "summary": summary(),
        "error_rates": error_rates(window_n=200), "cache": cache_stats(window_n=200),
        "budget": bstatus(), "jobs": jobs,
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
    t = threading.Thread(target=_run_rubric_job, args=(task, criteria, K, gm, jm), daemon=True)
    t.start()
    return JSONResponse({"started": "rubric", "task": task[:80]})


async def run_fanout(request):
    from starlette.responses import JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    body = await request.json()
    prompts = body.get("prompts", []); gm = body.get("gen_model", "deepseek-chat")
    t = threading.Thread(target=_run_fanout_job, args=(prompts, gm), daemon=True)
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
    t = threading.Thread(target=_run_project_job,
                         args=(project, task, mode, push, engine, target_file, test_cmd), daemon=True)
    t.start()
    return JSONResponse({"started": "project", "project": project, "engine": engine, "mode": mode})


async def sync_pull(request):
    from starlette.responses import JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    from .sync import pull_all
    return JSONResponse(pull_all())


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


def build_app():
    from starlette.applications import Starlette
    from starlette.routing import Route
    return Starlette(routes=[
        Route("/", home),
        Route("/state", state_snapshot),
        Route("/events", sse_events),
        Route("/run/rubric", run_rubric, methods=["POST"]),
        Route("/run/fanout", run_fanout, methods=["POST"]),
        Route("/projects", projects_handler, methods=["GET", "POST"]),
        Route("/run/project", run_project, methods=["POST"]),
        Route("/sync/pull", sync_pull, methods=["POST"]),
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
<pre id=state class=muted style="white-space:pre-wrap;max-height:30vh;overflow:auto"></pre></div>
</main>
<script>
let T='';
function connect(){T=document.getElementById('token').value;
 const es=new EventSource('/events?token='+encodeURIComponent(T));
 es.onopen=()=>document.getElementById('conn').textContent='live';
 es.onerror=()=>document.getElementById('conn').textContent='desconectado';
 es.onmessage=e=>addEv(JSON.parse(e.data));loadState();loadProjects();}
function addEv(ev){const f=document.getElementById('feed');const d=document.createElement('div');
 d.className='ev '+(ev.status||'pending');
 d.innerHTML='<span class=dot></span><b>'+(ev.node||ev.type)+'</b><span class=muted>'+ev.status+'</span> '+(ev.detail||'');
 f.prepend(d);document.getElementById('cnt').textContent=f.children.length;}
function clearfeed(){document.getElementById('feed').innerHTML='';document.getElementById('cnt').textContent=0;}
function H(){return {'Content-Type':'application/json','X-Token':T};}
function runRubric(){const task=document.getElementById('task').value||'implementa inc(x)=x+1';
 fetch('/run/rubric',{method:'POST',headers:H(),body:JSON.stringify({task,K:5,criteria:[
  {id:'c1',desc:'inc pasa',kind:'checkable',checker:'python_exec',ctx:{code:'{attempt_code}\\nassert inc(1)==2'}}]})});}
function runFan(){fetch('/run/fanout',{method:'POST',headers:H(),body:JSON.stringify({prompts:['di hola','di chau','di test']})});}
function loadState(){fetch('/state?token='+encodeURIComponent(T)).then(r=>r.json()).then(s=>{
 document.getElementById('state').textContent=JSON.stringify({jobs:s.jobs,calls:s.summary&&s.summary.calls,cost:s.summary&&s.summary.total_cost_usd,sections:s.sections,budget:s.budget},null,2);});}
function loadProjects(){fetch('/projects?token='+encodeURIComponent(T)).then(r=>r.json()).then(s=>{
 const sel=document.getElementById('proj');sel.innerHTML='';
 Object.keys(s.projects||{}).forEach(n=>{const o=document.createElement('option');o.value=n;o.textContent=n;sel.appendChild(o);});});}
function runMmorch(){const project=document.getElementById('proj').value;const task=document.getElementById('ptask').value;
 const target_file=document.getElementById('pfile').value;const test_cmd=document.getElementById('ptest').value||null;
 if(!project||!task||!target_file){alert('mmorch necesita proyecto + instruccion + archivo');return;}
 fetch('/run/project',{method:'POST',headers:H(),body:JSON.stringify({project,task,engine:'mmorch',target_file,test_cmd,push:true})});}
function runClaude(mode){const project=document.getElementById('proj').value;const task=document.getElementById('ptask').value;
 if(!project||!task){alert('elegí proyecto + instruccion');return;}
 fetch('/run/project',{method:'POST',headers:H(),body:JSON.stringify({project,task,engine:'claude',mode,push:mode==='edit'})});}
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
