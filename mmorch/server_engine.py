"""server_engine — the in-process job execution engine: the threads that drive rubric,
workflow, project and fan-out jobs. Lifted verbatim out of server.py; this is the "how jobs
run" half of the old god-module, leaving server.py as the HTTP route surface. Depends only on
server_core (shared state) + events; the route handlers call these via import.
"""
from __future__ import annotations

import os
import threading
import time
import uuid

from .events import emit, Event
from .server_core import _JOBS, _JOBS_LOCK, _jobmeta


def _rubric_drive(jid: str, state: dict, cancel: threading.Event):
    """The rubric loop body — shared by a fresh run and a resume (the state is the only input).
    Persists the JSON-serializable state to the job spec each step so a resume continues from it."""
    from .rubric_loop import next_action, submit
    from .providers import call
    from . import workflow_store, transcript_store

    def fn(model):
        def _c(prompt):
            return call(model, [{"role": "user", "content": prompt}],
                        pattern="rubric_loop", node=model).text
        return _c
    gen_fn, judge_fn = fn(state["gen_model"]), fn(state["judge_model"])
    step = len(workflow_store.checkpoint_history(jid))      # continue numbering on resume
    try:
        while True:
            if cancel.is_set():
                emit("job", "error", job_id=jid, detail="cancelado por el usuario")
                break
            act = next_action(state)
            if act["role"] in ("done", "escalate"):
                break
            out = gen_fn(act["prompt"]) if act["role"] == "executor" else judge_fn(act["prompt"])
            model = state["gen_model"] if act["role"] == "executor" else state["judge_model"]
            transcript_store.append(jid, model, act["role"], out)
            step += 1
            try:                                   # Phase A checkpoint + Phase B durable state
                bid = workflow_store.put_block(out, kind=act["role"], mime="text/markdown")
                workflow_store.record_checkpoint(jid, step, act["role"], outputs=[bid],
                                                 state={"model": model})
            except Exception:
                pass                               # best-effort; never break the job on a store hiccup
            with _JOBS_LOCK:                       # G9: progress -> heartbeat, don't reap active jobs
                if jid in _JOBS:
                    _JOBS[jid]["heartbeat"] = time.time()
            submit(state, out)
            try:                                   # Phase B: persist resumable state after each step
                workflow_store.record_job_spec(jid, "rubric", {"state": state})
            except Exception:
                pass
    except Exception as e:
        emit("job", "error", job_id=jid, detail=str(e)[:200])
    with _JOBS_LOCK:
        if jid in _JOBS:
            _JOBS[jid]["status"] = state.get("phase", "done")


def _run_rubric_job(task: str, criteria: list, K: int, gen_model, judge_model, parent=None):
    from .rubric_loop import start_rubric
    from . import workflow_store
    state = start_rubric(task, criteria, K=K, gen_model=gen_model, judge_model=judge_model)
    jid = state["id"]
    cancel = threading.Event()
    with _JOBS_LOCK:
        _JOBS[jid] = _jobmeta("rubric", task, cancel=cancel, state=state, parent=parent)
    try:
        workflow_store.record_job_spec(jid, "rubric", {"state": state})   # resumable from the start
    except Exception:
        pass
    emit("job", "running", job_id=jid, detail=task[:120])
    _rubric_drive(jid, state, cancel)


def _workflow_drive(jid: str, state: dict, meta: dict, cancel: threading.Event,
                    pause: threading.Event | None = None) -> str:
    """Drive the cooperative role-chain (Phase C). `meta` = {task, work_dir, apply_project?, branch?, name?}
    persisted (with state) each step so a resume continues from it. Each step: run the role's model ->
    put_block (derives_from the consumed blocks) -> checkpoint; then the gate (tests = run test_cmd;
    verdict = parse the reviewer's APPROVE/REQUEST_CHANGES). Loops back / escalates per spec. A pause
    stops cleanly at the next step boundary (state already persisted) -> resumable. Returns the terminal
    job status: done | escalate | error | paused."""
    import os as _os
    from . import workflow_store, transcript_store
    from .workflow_engine import next_workflow_action, submit_workflow, build_prompt
    from .project_loop import _extract, _run_cmd
    from .providers import call
    from .config import DEFAULT_GENERATOR
    task, work_dir = meta["task"], meta["work_dir"]
    _os.makedirs(work_dir, exist_ok=True)
    step = len(workflow_store.checkpoint_history(jid))
    try:
        while True:
            if cancel.is_set():
                emit("job", "error", job_id=jid, detail="cancelado por el usuario")
                with _JOBS_LOCK:
                    if jid in _JOBS:
                        _JOBS[jid]["status"] = "error"
                return "error"
            if pause is not None and pause.is_set():       # stop at this step boundary, stay resumable
                emit("job", "gate", job_id=jid, detail="paused at checkpoint (resume to continue)")
                with _JOBS_LOCK:
                    if jid in _JOBS:
                        _JOBS[jid]["status"] = "paused"
                return "paused"
            act = next_workflow_action(state)
            if act["kind"] in ("done", "escalate"):
                break
            if act["kind"] == "produce":
                cfg = state["steps"][act["step"]]
                names = cfg.get("consumes", [])
                inputs = []
                for nm, bid in zip(names, act["consumes"]):
                    blk = workflow_store.get_block(bid)
                    inputs.append((nm, blk["body"] if blk else ""))
                prompt = build_prompt(act["role"], act["persona"], task, inputs)
                model = act.get("model") or DEFAULT_GENERATOR
                out = call(model, [{"role": "user", "content": prompt}],
                           pattern="workflow", node=act["role"]).text
                transcript_store.append(jid, model, act["role"], out)
                step += 1
                bid = workflow_store.put_block(out, kind=act.get("produces") or act["role"],
                                               mime="text/markdown", derives_from=act["consumes"])
                workflow_store.record_checkpoint(jid, step, act["role"], inputs=act["consumes"],
                                                 outputs=[bid], state={"model": model,
                                                                       "produces": act.get("produces")})
                submit_workflow(state, block_id=bid)
            else:                                   # gate
                passed = False
                blk = workflow_store.get_block(act.get("block") or "")
                body = blk["body"] if blk else ""
                if act["gate"] == "tests":
                    tf = state["steps"][act["step"]].get("target_file") or "solution.py"
                    with open(_os.path.join(work_dir, tf), "w", encoding="utf-8") as f:
                        f.write(_extract(body))
                    ok, detail = _run_cmd(work_dir, act["test_cmd"])
                    passed = ok
                    emit("step", "done" if ok else "error", job_id=jid,
                         node=f"gate:tests:{act['role']}", detail=detail[-120:])
                else:                               # verdict: cross-family reviewer's own output
                    passed = "VERDICT: APPROVE" in body.upper()
                    emit("step", "done" if passed else "gate", job_id=jid,
                         node=f"gate:verdict:{act['role']}", detail="APPROVE" if passed else "REQUEST_CHANGES")
                submit_workflow(state, gate_passed=passed)
            with _JOBS_LOCK:                        # G9 heartbeat
                if jid in _JOBS:
                    _JOBS[jid]["heartbeat"] = time.time()
            try:                                    # Phase B: persist resumable state each step
                workflow_store.record_job_spec(jid, "workflow", {**meta, "state": state})
            except Exception:
                pass
    except Exception as e:
        emit("job", "error", job_id=jid, detail=str(e)[:200])
        with _JOBS_LOCK:
            if jid in _JOBS:
                _JOBS[jid]["status"] = "error"
        return "error"
    final = state.get("status", "done")
    emit("job", "done" if final == "done" else "gate", job_id=jid, detail=f"workflow {final}")
    with _JOBS_LOCK:
        if jid in _JOBS:
            _JOBS[jid]["status"] = "done" if final == "done" else "escalate"
    return final


def _workflow_run(jid: str, state: dict, meta: dict, *, resumed: bool = False, parent=None):
    """Single entry for a workflow run AND a resume: manage the work dir (a git worktree of the repo
    when meta.apply_project is set, else a temp sandbox), register the job, drive, and finalize the
    worktree (commit progress -> review branch, keep it) on any exit (done/escalate/paused/error).
    Resume reopens the SAME review branch (continuity)."""
    import tempfile as _tf
    from . import workflow_store
    wt = None
    if meta.get("apply_project"):
        from .worktree_driver import open_worktree
        from .projects import resolve
        try:
            wt = open_worktree(resolve(meta["apply_project"]), branch=meta.get("branch"))
            meta["work_dir"], meta["branch"] = wt.path, wt.branch
        except Exception as e:
            emit("job", "error", job_id=jid, detail=f"worktree open failed: {str(e)[:140]}")
            with _JOBS_LOCK:
                _JOBS[jid] = _jobmeta("workflow", meta.get("task", ""), parent=parent)
                _JOBS[jid]["status"] = "error"
            return
    else:
        meta.setdefault("work_dir", os.path.join(_tf.gettempdir(), f"mmorch-wf-{jid}"))
    cancel, pause = threading.Event(), threading.Event()
    with _JOBS_LOCK:
        _JOBS[jid] = _jobmeta("workflow", meta.get("name") or meta.get("task", ""),
                              cancel=cancel, pause=pause, parent=parent, resumed=resumed)
        if meta.get("branch"):
            _JOBS[jid]["review_branch"] = meta["branch"]
    try:
        workflow_store.record_job_spec(jid, "workflow", {**meta, "state": state})
    except Exception:
        pass
    emit("job", "running", job_id=jid, detail=f"workflow {meta.get('name','')}: {meta.get('task','')[:80]}")
    _workflow_drive(jid, state, meta, cancel, pause)
    if wt:                                         # finalize: commit progress to the review branch, free it
        try:
            cap = wt.capture(f"mmorch workflow {meta.get('name','')}: {meta.get('task','')[:60]}")
            with _JOBS_LOCK:
                if jid in _JOBS:
                    _JOBS[jid]["review_branch"] = cap["branch"]
                    _JOBS[jid]["diffstat"] = cap.get("diffstat", "")
            sp = workflow_store.get_job_spec(jid)   # persist branch so a resume reopens it
            if sp:
                m = sp["spec"]; m["branch"] = cap["branch"]
                workflow_store.record_job_spec(jid, "workflow", m)
        except Exception:
            pass
        finally:
            wt.close(keep_branch=True)


def _run_workflow_job(jid: str, spec: dict, task: str, project=None, apply=False, parent=None):
    from .workflow_engine import start_workflow
    state = start_workflow(spec["steps"], task)
    meta = {"task": task, "name": spec.get("name", "")}
    if apply and project:
        meta["apply_project"] = project
    _workflow_run(jid, state, meta, parent=parent)


def _run_project_job(project: str, task: str, mode: str, push: bool = False,
                     engine: str = "mmorch", target_file: str = "", test_cmd: str | None = None,
                     driver: str = "local", parent=None):
    """Job project-aware. engine PRIMARIO = 'mmorch' (DeepSeek genera + tests verifican +
    aplica determinista, cero cupo; escala a claude -p si no puede). engine='claude' =
    claude -p directo (plan/cupo) — para tareas abiertas que mmorch no banca.
    driver='worktree' (G3 sandbox) = corre en un git worktree desechable -> review branch."""
    import uuid as _u
    jid = _u.uuid4().hex[:10]
    with _JOBS_LOCK:
        _JOBS[jid] = _jobmeta("project", task, engine=engine, parent=parent, driver=driver)
    if engine == "mmorch" and target_file and driver == "local":
        try:                                   # Phase B: resumable spec (worktree runs are one-shot, no resume)
            from . import workflow_store
            workflow_store.record_job_spec(jid, "project", {
                "project": project, "task": task, "target_file": target_file,
                "test_cmd": test_cmd, "K": 4, "push": push})
        except Exception:
            pass
    try:
        if engine == "mmorch":
            if not target_file:
                emit("job", "error", job_id=jid, detail="engine=mmorch requiere target_file")
                with _JOBS_LOCK:
                    _JOBS[jid]["status"] = "error"
                return
            if driver == "worktree":
                from .project_loop import run_project_task_isolated
                r, cap = run_project_task_isolated(project, task, target_file=target_file,
                                                   test_cmd=test_cmd, job_id=jid)
                with _JOBS_LOCK:
                    if jid in _JOBS:
                        _JOBS[jid]["review_branch"] = cap.get("branch")
                        _JOBS[jid]["diffstat"] = cap.get("diffstat", "")
                emit("job", "running", job_id=jid,
                     detail=f"worktree -> {cap.get('branch')} ({'changes' if cap.get('changed') else 'no changes'})")
            else:
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
