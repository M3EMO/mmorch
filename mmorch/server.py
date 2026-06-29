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

from .events import bus, emit
from .server_frontend import FRONTEND as _FRONTEND
from .server_core import _JOBS, _JOBS_LOCK, _GATES, _token_ok, _budget_block, _jobmeta


from .server_engine import (_rubric_drive, _run_rubric_job, _workflow_run, _run_workflow_job, _run_project_job,
                            _run_fanout_job)


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
    from .budget_policy import load as _bp_load, evaluate as _bp_eval, snapshot as _bp_snap
    with _JOBS_LOCK:
        jobs = {k: {"status": v["status"], "kind": v["kind"], "title": v.get("title", ""),
                    "ts": v.get("ts", 0), "host": v.get("host", "local"),
                    "engine": v.get("engine", ""), "parent": v.get("parent")} for k, v in _JOBS.items()}
    return JSONResponse({
        "conductor": conductor(), "sections": sections(), "summary": summary(),
        "error_rates": error_rates(window_n=200), "cache": cache_stats(window_n=200),
        "budget": bstatus(), "jobs": jobs, "exec_policy": current_policy(),
        "budget_incidents": _bp_eval(_bp_load(), _bp_snap()),
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
    blocked = _budget_block()
    if blocked:
        return blocked
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
    blocked = _budget_block()
    if blocked:
        return blocked
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
    blocked = _budget_block()
    if blocked:
        return blocked
    from .exec_policy import current_policy, evaluate
    driver = "local"
    if not evaluate(current_policy(), "local")["allowed"]:
        # G3 sandbox: instead of denying, ISOLATE the mmorch engine in a git worktree.
        # claude engine has no worktree path yet -> still denied under sandbox.
        if engine == "mmorch":
            driver = "worktree"
        else:
            return JSONResponse(
                {"error": "exec policy 'sandbox': engine 'claude' has no isolated driver "
                          "(use engine=mmorch for worktree isolation)"}, status_code=403)
    t = threading.Thread(target=_run_project_job,
                         args=(project, task, mode, push, engine, target_file, test_cmd, driver),
                         kwargs={"parent": body.get("parent_id")}, daemon=True)
    t.start()
    return JSONResponse({"started": "project", "project": project, "engine": engine,
                         "mode": mode, "driver": driver})


async def run_workflow(request):
    """Start a cooperative role-chain workflow (Phase C). Body: {task, workflow_name | workflow:{...}}."""
    from starlette.responses import JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    blocked = _budget_block()
    if blocked:
        return blocked
    body = await request.json()
    task = body.get("task", "")
    if not task:
        return JSONResponse({"error": "task required"}, status_code=400)
    from . import workflow_spec
    try:
        if body.get("workflow"):
            spec = workflow_spec.validate(body["workflow"])           # inline, ad-hoc
        elif body.get("workflow_name"):
            spec = workflow_spec.load_workflow(body["workflow_name"])  # saved
        else:
            return JSONResponse({"error": "workflow or workflow_name required"}, status_code=400)
    except Exception as e:
        return JSONResponse({"error": f"invalid workflow: {str(e)[:200]}"}, status_code=400)
    import uuid as _u
    jid = _u.uuid4().hex[:10]
    project = body.get("project")
    apply = bool(body.get("apply"))               # apply=true + project -> run in a git worktree of the repo
    t = threading.Thread(target=_run_workflow_job, args=(jid, spec, task),
                         kwargs={"project": project, "apply": apply, "parent": body.get("parent_id")},
                         daemon=True)
    t.start()
    return JSONResponse({"started": "workflow", "job_id": jid, "name": spec["name"],
                         "steps": len(spec["steps"]), "apply": apply and bool(project)})


from .server_fleet import sync_pull, fleet_handler, fleet_run


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


async def feedback_handler(request):
    """Human up/down vote on a job output (graft G8): trace bundle + feed the bandit."""
    from starlette.responses import JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    body = await request.json()
    job_id = body.get("job_id", "")
    vote = body.get("vote", "")
    if vote not in ("up", "down"):
        return JSONResponse({"error": "vote must be up|down"}, status_code=400)
    from . import feedback_trace
    from .transcript_store import get as _tget
    arm, ctx = "", ""
    with _JOBS_LOCK:
        j = _JOBS.get(job_id)
    if j:
        arm = j.get("engine") or ""
        ctx = j.get("title") or ""
    bundle = feedback_trace.record_vote(
        job_id, vote, arm=arm, comment=body.get("comment", ""), context=ctx,
        transcript=_tget(job_id), consent=body.get("consent", "local_only"))
    emit("feedback", "info", job_id=job_id, detail=f"{vote} ({arm or 'no-arm'})")
    return JSONResponse({"recorded": True, "vote": vote, "arm": arm, "consent": bundle["consent"]})


async def gate_handler(request):
    """Staged gate for a job (graft G6). GET = current state; POST {policy} = start a gate."""
    from starlette.responses import JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    jid = request.path_params["job_id"]
    if request.method == "POST":
        body = await request.json()
        from .gate_policy import start
        state = start(body.get("policy") or {})
        with _JOBS_LOCK:
            _GATES[jid] = state
            if jid in _JOBS:
                _JOBS[jid]["status"] = "gate"
        emit("job", "gate", job_id=jid, detail=f"staged gate ({len(state['policy']['stages'])} stages)")
        return JSONResponse(state)
    with _JOBS_LOCK:
        st = _GATES.get(jid)
    return JSONResponse(st) if st else JSONResponse({"error": "no gate"}, status_code=404)


async def gate_advance(request):
    """Advance a staged gate (graft G6): action approve|request_changes|reject."""
    from starlette.responses import JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    jid = request.path_params["job_id"]
    body = await request.json()
    from .gate_policy import advance
    with _JOBS_LOCK:
        st = _GATES.get(jid)
    if not st:
        return JSONResponse({"error": "no gate"}, status_code=404)
    nxt = advance(st, body.get("action", "approve"), body.get("actor", ""), body.get("comment", ""))
    if nxt.get("error"):
        return JSONResponse({"error": nxt["error"]}, status_code=400)
    with _JOBS_LOCK:
        _GATES[jid] = nxt
        if jid in _JOBS:
            if nxt["status"] == "approved":
                _JOBS[jid]["status"] = "done"
            elif nxt["status"] == "rejected":
                _JOBS[jid]["status"] = "error"
    if nxt["status"] in ("approved", "rejected"):
        emit("job", "done" if nxt["status"] == "approved" else "error",
             job_id=jid, detail=f"staged gate {nxt['status']}")
    return JSONResponse(nxt)


async def budget_policies(request):
    """Scoped budget policies (graft G5): GET = policies+incidents+snapshot, POST = save."""
    from starlette.responses import JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    from . import budget_policy
    if request.method == "POST":
        body = await request.json()
        pols = body.get("policies", [])
        budget_policy.save(pols)
        return JSONResponse({"saved": len(pols)})
    snap = budget_policy.snapshot()
    pols = budget_policy.load()
    return JSONResponse({"policies": pols, "snapshot": snap,
                         "incidents": budget_policy.evaluate(pols, snap)})


async def cancel_tree(request):
    """Cascade-cancel a job subtree (graft G7): hold + per-member snapshot, skip terminals."""
    from starlette.responses import JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    jid = request.path_params["job_id"]
    from . import job_graph
    with _JOBS_LOCK:
        snap = {k: {"parent": v.get("parent"), "status": v.get("status")} for k, v in _JOBS.items()}
    plan = job_graph.plan_subtree_cancel(snap, jid)
    applied = []
    with _JOBS_LOCK:
        for m in plan["members"]:
            j = _JOBS.get(m["id"])
            if not j:
                continue
            c = j.get("cancel")
            if c:
                try:
                    c.set()
                except Exception:
                    pass
            j["status"] = "error"
            applied.append(m["id"])
    for mid in applied:
        emit("job", "error", job_id=mid, detail="cancelled via subtree hold")
    plan["applied"] = applied
    return JSONResponse(plan)


async def reap_zombies(request):
    """Detect + fail stuck jobs (graft G9): non-terminal rows whose heartbeat went stale.
    Trigger this from scheduled-tasks. Body: {ttl?: seconds, dry?: bool}."""
    from starlette.responses import JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    try:
        body = await request.json()
    except Exception:
        body = {}
    from . import durable_runs
    ttl = body.get("ttl")
    dry = bool(body.get("dry"))
    now = time.time()
    with _JOBS_LOCK:
        snap = {k: {"status": v.get("status"), "ts": v.get("ts"),
                    "heartbeat": v.get("heartbeat")} for k, v in _JOBS.items()}
    zombies = durable_runs.detect_zombies(snap, now=now, ttl=ttl)
    reaped = []
    if not dry:
        with _JOBS_LOCK:
            for z in zombies:
                j = _JOBS.get(z["id"])
                if not j or j.get("status") in durable_runs._NOT_ZOMBIE:
                    continue   # finished or vanished between snapshot and now
                c = j.get("cancel")
                if c:
                    try:
                        c.set()
                    except Exception:
                        pass
                j["status"] = "error"
                j["zombie"] = True
                reaped.append(z)
        for z in reaped:
            emit("job", "error", job_id=z["id"],
                 detail=f"zombie reaped: no heartbeat for {z['age']}s")
    # Phase A: the sweep also GCs orphan blocks + flags which reaped jobs are resumable.
    gc, resumable = {}, []
    try:
        from . import workflow_store
        gc = workflow_store.gc_blocks(dry_run=dry)
        cp_jobs = workflow_store.jobs_with_checkpoints()
        # a zombie with a checkpoint trail can be resumed from its last step instead of staying dead
        ids = [z["id"] for z in (reaped if not dry else zombies)]
        resumable = [jid for jid in ids if jid in cp_jobs]
    except Exception as e:
        gc = {"error": str(e)[:120]}
    return JSONResponse({"now": now, "ttl": durable_runs.default_ttl() if ttl is None else ttl,
                         "dry": dry, "zombies": zombies, "reaped": [r["id"] for r in reaped],
                         "gc": gc, "resumable": resumable})


def _resume_project(jid: str, data: dict, remaining: int):
    """Re-dispatch an interrupted project job from its last checkpoint (Phase B)."""
    from . import workflow_store
    from .projects import resolve
    from .project_loop import run_project_task
    done = len(workflow_store.checkpoint_history(jid))
    # seed the file from the last checkpoint's output block so the loop continues from that attempt
    latest = workflow_store.checkpoint_latest(jid)
    try:
        if latest and latest.get("outputs"):
            blk = workflow_store.get_block(latest["outputs"][-1])
            if blk and blk.get("body"):
                import os as _os
                fp = _os.path.join(resolve(data["project"]), data["target_file"])
                with open(fp, "w", encoding="utf-8") as f:
                    f.write(blk["body"] + ("\n" if not blk["body"].endswith("\n") else ""))
    except Exception:
        pass
    with _JOBS_LOCK:
        _JOBS[jid] = _jobmeta("project", data["task"], engine="mmorch", resumed=True)
    ok = False
    try:
        r = run_project_task(data["project"], data["task"], target_file=data["target_file"],
                             test_cmd=data.get("test_cmd"), K=remaining, push=data.get("push", False),
                             escalate=False, step_offset=done, job_id=jid)
        ok = r.ok
    except Exception as e:
        emit("job", "error", job_id=jid, detail=str(e)[:160])
    with _JOBS_LOCK:
        if jid in _JOBS:
            _JOBS[jid]["status"] = "done" if ok else "error"


async def resume_job(request):
    """Resume an interrupted job from its last checkpoint (Phase B). Explicit only — no auto-resume."""
    from starlette.responses import JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    blocked = _budget_block()
    if blocked:
        return blocked
    jid = request.path_params["job_id"]
    from . import workflow_store
    spec = workflow_store.get_job_spec(jid)
    if not spec:
        return JSONResponse({"error": "no resumable spec for this job"}, status_code=404)
    if not workflow_store.checkpoint_latest(jid):
        return JSONResponse({"error": "no checkpoints to resume from"}, status_code=409)
    with _JOBS_LOCK:
        cur = _JOBS.get(jid, {}).get("status")
    if cur == "running":
        return JSONResponse({"error": "job is already running"}, status_code=409)
    kind = spec["kind"]
    data = spec["spec"]
    if kind == "rubric":
        state = data["state"]
        if state.get("phase") == "done":
            return JSONResponse({"error": "rubric already complete"}, status_code=409)
        cancel = threading.Event()
        with _JOBS_LOCK:
            _JOBS[jid] = _jobmeta("rubric", state.get("task", ""), cancel=cancel, state=state, resumed=True)
        emit("job", "running", job_id=jid, detail="resumed from checkpoint")
        threading.Thread(target=_rubric_drive, args=(jid, state, cancel), daemon=True).start()
        return JSONResponse({"resumed": jid, "kind": "rubric", "phase": state.get("phase"),
                             "from_step": len(workflow_store.checkpoint_history(jid))})
    if kind == "workflow":
        state = data["state"]
        if state.get("status") == "done":
            return JSONResponse({"error": "workflow already complete"}, status_code=409)
        meta = {k: data[k] for k in ("task", "name", "work_dir", "apply_project", "branch") if k in data}
        meta.setdefault("task", "")
        threading.Thread(target=_workflow_run, args=(jid, state, meta),
                         kwargs={"resumed": True}, daemon=True).start()
        return JSONResponse({"resumed": jid, "kind": "workflow", "status": state.get("status"),
                             "from_step": len(workflow_store.checkpoint_history(jid))})
    if kind == "project":
        done = len(workflow_store.checkpoint_history(jid))
        remaining = max(1, int(data.get("K", 4)) - done)
        with _JOBS_LOCK:                        # set running synchronously -> no stale-status window
            _JOBS[jid] = _jobmeta("project", data.get("task", ""), engine="mmorch", resumed=True)
        emit("job", "running", job_id=jid, detail=f"resumed from checkpoint (K left={remaining})")
        threading.Thread(target=_resume_project, args=(jid, data, remaining), daemon=True).start()
        return JSONResponse({"resumed": jid, "kind": "project", "remaining_K": remaining,
                             "from_step": done})
    return JSONResponse({"error": f"unknown job kind '{kind}'"}, status_code=400)


async def pause_job(request):
    """Pause a running job at its next checkpoint boundary (Phase B). It stops cleanly and stays
    resumable via POST /jobs/{id}/resume. (Workflows; for a hard stop use cancel-tree.)"""
    from starlette.responses import JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    jid = request.path_params["job_id"]
    with _JOBS_LOCK:
        j = _JOBS.get(jid)
        ev = j.get("pause") if j else None
        st = j.get("status") if j else None
    if not j:
        return JSONResponse({"error": "no such job"}, status_code=404)
    if ev is None:
        return JSONResponse({"error": "job is not pausable (only running workflows)"}, status_code=409)
    if st != "running":
        return JSONResponse({"error": f"job is '{st}', not running"}, status_code=409)
    ev.set()
    return JSONResponse({"pausing": jid, "note": "stops at the next step boundary; resume to continue"})


async def job_checkpoints(request):
    """Durable per-step trail of a job (Phase A): the block-referencing checkpoint chain."""
    from starlette.responses import JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    from . import workflow_store
    jid = request.path_params["job_id"]
    return JSONResponse({"job_id": jid, "checkpoints": workflow_store.checkpoint_history(jid)})


async def block_get(request):
    """Fetch one context block by content-addressed id (Phase A)."""
    from starlette.responses import JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    from . import workflow_store
    b = workflow_store.get_block(request.path_params["block_id"])
    if not b:
        return JSONResponse({"error": "no such block"}, status_code=404)
    return JSONResponse(b)


async def plugins_list(request):
    """List installed plugins + their granted caps under the current policy (graft G11)."""
    from starlette.responses import JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    from . import plugins as _pl
    return JSONResponse({"plugins": _pl.discover(),
                         "policy_allow": sorted(_pl.policy_allow())})


def _plugin_host_services():
    """Host services a plugin MAY call (only if its cap is granted). Capability = namespace."""
    from .providers import call
    return {
        "log.emit": lambda p: (emit("plugin", "info", detail=str(p.get("msg", ""))[:160]), "ok")[1],
        "llm.call": lambda p: call(p["model"], p.get("messages") or p.get("prompt", ""),
                                   pattern="plugin", node=p.get("model", "")).text,
    }


async def plugin_invoke(request):
    """Run one plugin contribution in an isolated, capability-gated worker (graft G11)."""
    from starlette.responses import JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    blocked = _budget_block()          # a plugin may spend via llm.call
    if blocked:
        return blocked
    name = request.path_params["name"]
    try:
        body = await request.json()
    except Exception:
        body = {}
    fn, args = body.get("fn", ""), body.get("args", {})
    if not fn:
        return JSONResponse({"error": "fn required"}, status_code=400)
    from . import plugins as _pl
    match = next((p for p in _pl.discover() if p.get("name") == name), None)
    if not match:
        return JSONResponse({"error": f"no plugin '{name}'"}, status_code=404)
    if match.get("error"):
        return JSONResponse({"error": f"plugin '{name}' invalid: {match['error']}"}, status_code=400)
    res = _pl.invoke(match, fn, args, host_services=_plugin_host_services())
    return JSONResponse(res, status_code=200 if res.get("ok") else 400)


async def export_handler(request):
    """Portable state bundle (graft G4): values tagged portable|system_dependent|secret."""
    from starlette.responses import JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    import time as _t
    from .projects import list_projects
    from .fleet import list_hosts
    from .exec_policy import current_policy
    from .portability import export_bundle
    return JSONResponse(export_bundle(list_projects(), list_hosts(), current_policy(), _t.time()))


async def import_handler(request):
    """Reconcile + apply a portable bundle on THIS machine (skip collisions, need local paths)."""
    from starlette.responses import JSONResponse
    if not _token_ok(request):
        return JSONResponse({"error": "unauthorized"}, status_code=401)
    body = await request.json()
    from .portability import import_bundle
    return JSONResponse(import_bundle(body.get("manifest") or {}, body.get("overrides") or {}))


# --- interactive PTY (writable terminal) ------------------------------------ #
from .server_pty import pty_open, pty_stream, pty_input, pty_resize, pty_close


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
        Route("/run/workflow", run_workflow, methods=["POST"]),
        Route("/chat", chat_handler, methods=["POST"]),
        Route("/chat/history", chat_history, methods=["GET"]),
        Route("/minds", minds_handler),
        Route("/transcript/{job_id}", transcript_handler),
        Route("/jobs/{job_id}/ancestry", job_ancestry),
        Route("/jobs/{job_id}/cancel-tree", cancel_tree, methods=["POST"]),
        Route("/jobs/reap", reap_zombies, methods=["POST"]),
        Route("/jobs/{job_id}/checkpoints", job_checkpoints),
        Route("/jobs/{job_id}/resume", resume_job, methods=["POST"]),
        Route("/jobs/{job_id}/pause", pause_job, methods=["POST"]),
        Route("/blocks/{block_id}", block_get),
        Route("/plugins", plugins_list),
        Route("/plugins/{name}/invoke", plugin_invoke, methods=["POST"]),
        Route("/jobs/{job_id}/gate", gate_handler, methods=["GET", "POST"]),
        Route("/jobs/{job_id}/gate/advance", gate_advance, methods=["POST"]),
        Route("/budget/policies", budget_policies, methods=["GET", "POST"]),
        Route("/feedback", feedback_handler, methods=["POST"]),
        Route("/export", export_handler),
        Route("/import", import_handler, methods=["POST"]),
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
