"""server_engine — the in-process job execution engine: the threads that drive rubric,
workflow, project and fan-out jobs. Lifted verbatim out of server.py; this is the "how jobs
run" half of the old god-module, leaving server.py as the HTTP route surface. Depends only on
server_core (shared state) + events; the route handlers call these via import.
"""
from __future__ import annotations

from pathlib import Path

import threading
import time
import uuid

from .events import emit
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
    # final explicito: antes una excepcion (p.ej. BudgetExceeded) dejaba el status
    # en la FASE del momento ("executor") — el job muerto era indistinguible de uno
    # vivo para el cliente, y el guard de resume ("running") lo dejaba re-lanzar
    # doble (medido por el verificador adversarial W6 r1).
    final: str | None = None
    try:
        while True:
            if cancel.is_set():
                emit("job", "error", job_id=jid, detail="cancelado por el usuario")
                final = "error"
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
            except Exception as e:
                # best-effort; never break the job on a store hiccup — but a silent skip
                # here means a later resume re-pays this step from scratch with no one told.
                emit("job", "warn", job_id=jid, detail=f"checkpoint no persistido: {str(e)[:150]}")
            with _JOBS_LOCK:                       # G9: progress -> heartbeat, don't reap active jobs
                if jid in _JOBS:
                    _JOBS[jid]["heartbeat"] = time.time()
            submit(state, out)
            try:                                   # Phase B: persist resumable state after each step
                workflow_store.record_job_spec(jid, "rubric", {"state": state})
            except Exception as e:
                emit("job", "warn", job_id=jid, detail=f"spec no persistido: {str(e)[:150]}")
    except Exception as e:
        emit("job", "error", job_id=jid, detail=str(e)[:200])
        final = "error"
    with _JOBS_LOCK:
        if jid in _JOBS:
            _JOBS[jid]["status"] = final or state.get("phase", "done")


def _run_rubric_job(task: str, criteria: list, K: int, gen_model, judge_model, parent=None,
                    job_id: str | None = None):
    from .rubric_loop import start_rubric
    from . import workflow_store
    state = start_rubric(task, criteria, K=K, gen_model=gen_model, judge_model=judge_model)
    if job_id:
        # D2 (Lotus): el handler ya devolvio este id al cliente — el estado lo adopta
        state["id"] = job_id
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


def _run_project_job(project: str, task: str, mode: str, push: bool = False,
                     engine: str = "mmorch", target_file: str = "", test_cmd: str | None = None,
                     driver: str = "local", parent=None, job_id: str | None = None):
    """Job project-aware. engine PRIMARIO = 'mmorch' (DeepSeek genera + tests verifican +
    aplica determinista, cero cupo; escala a claude -p si no puede). engine='claude' =
    claude -p directo (plan/cupo) — para tareas abiertas que mmorch no banca.
    driver='worktree' (G3 sandbox) = corre en un git worktree desechable -> review branch."""
    import uuid as _u
    jid = job_id or _u.uuid4().hex[:10]   # D2 (Lotus): id acordado con el handler
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
            from .claude_exec import get_executor
            cwd = resolve(project)
            emit("job", "running", job_id=jid, detail=f"claude {project} [{mode}]: {task[:70]}")
            ok = get_executor().run(task, cwd, mode=mode, job_id=jid).ok
            if ok and mode == "edit" and push:
                from .sync import commit_push
                commit_push(cwd, f"mmorch(claude): {task[:64]}", job_id=jid)
    except Exception as e:
        emit("job", "error", job_id=jid, detail=str(e)[:160])
        ok = False
    with _JOBS_LOCK:
        _JOBS[jid]["status"] = "done" if ok else "error"


def _tests_nombrados(external_test: str | None, root) -> list[str]:
    """Rutas de tests que nombra `external_test` y existen en el worktree: `tests/test_x.py`, pero tambien
    `app/src/sdlc/x.test.ts` o `.../Sdlc_x_Test.java` (antes solo .py bajo tests/). `backend/pom.xml` no es un test."""
    import re as _re
    return [p for p in _re.findall(r"[\w./-]*test[\w./-]*\.\w+", external_test or "", _re.I) if (Path(root) / p).is_file()]


_BUILD_HIJO = """import json, pathlib, sys
from mmorch.sdlc import build_feature
kw = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
pathlib.Path(sys.argv[2]).write_text(json.dumps(build_feature(**kw), default=str), encoding="utf-8")
"""


def _build_en_proceso(kw: dict) -> dict:
    """Cada feature corre en su propio proceso. Los globals de mmorch.sdlc son por proceso: dos jobs avanzan en paralelo
    sin pisarse el worktree (2026-09-16 un lock los serializaba; el gasto igual queda en metrics.jsonl)."""
    import json
    import subprocess
    import sys
    import tempfile
    with tempfile.TemporaryDirectory() as d:
        entrada, salida = Path(d) / "entrada.json", Path(d) / "salida.json"
        entrada.write_text(json.dumps(kw), encoding="utf-8")
        from .paths import repo_root
        p = subprocess.run([sys.executable, "-c", _BUILD_HIJO, str(entrada), str(salida)], cwd=repo_root(),
                           stderr=subprocess.PIPE, text=True, encoding="utf-8", errors="replace")
        sys.stderr.write(p.stderr)
        if p.returncode or not salida.exists():
            raise RuntimeError(f"build_feature salio con {p.returncode}: {p.stderr.strip()[-300:]}")
        return json.loads(salida.read_text(encoding="utf-8"))


def _run_project_build_job(jid: str, task: str, project: str, external_test: str,
                           max_depth: int = 2, seed_globs: list | None = None, parent=None,
                           gen_model: str | None = None, max_fix: int | None = None,
                           files: list | None = None, from_stage: float | None = None, resume_branch: str | None = None):
    """El pipeline de 6 etapas (mmorch.sdlc, ticket 05) como job del server. Corre en un worktree AISLADO de
    `project` (arbol principal intacto, resultado en una review branch). `external_test` = comando de aceptacion;
    si nombra archivos tests/*.py que existen en el repo, esos son los tests de aceptacion del pipeline.
    Estado terminal: built -> done; integration_failed (etapa 5) -> gate; otro fallo -> escalate.
    La etapa es el checkpoint: `result.failed_stage` + `review_branch` permiten reanudar con
    {resume_branch, from_stage} en el mismo payload. `max_depth` se acepta por compatibilidad y se ignora."""
    from .worktree_driver import _git, open_worktree
    from .projects import resolve
    with _JOBS_LOCK:
        _JOBS[jid] = _jobmeta("project-build", task, engine="mmorch", parent=parent)
    wt = None
    base = ""
    try:
        repo = resolve(project)
        wt = open_worktree(repo, base=resume_branch or "HEAD")
        rc, base = _git(wt.path, "rev-parse", "HEAD")
        base = base if rc == 0 else ""
        with _JOBS_LOCK:
            _JOBS[jid]["review_branch"] = wt.branch
        # F4: un checkout fresco no tiene los artefactos gitignorados que la aceptacion lee (.venv, caches).
        from .sdlc import _toml
        n_seed = wt.seed(list(dict.fromkeys((seed_globs or []) + list(_toml(repo).get("seed_globs", [])) + [".venv", "venv"])))
        emit("job", "running", job_id=jid,
             detail=f"sdlc {project} -> {wt.branch}{f' (+{n_seed} seeded)' if n_seed else ''}: {task[:70]}")
        named = _tests_nombrados(external_test, wt.path)
        accept = {p: (Path(wt.path) / p).read_text(encoding="utf-8") for p in named}
        res = _build_en_proceso(dict(name=jid, task=task, repo=repo, accept=accept or None,
                                     accept_cmd=None if accept else external_test, files=files, wt=wt.path,
                                     phase=f"sdlc-{jid}", from_stage=from_stage, max_fix=int(max_fix) if max_fix else 3,
                                     coder=gen_model))
        status = res.get("status", "escalate")
        job_status = {"built": "done", "integration_failed": "gate", "awaiting_approval": "gate"}.get(status, "escalate")
        with _JOBS_LOCK:
            if jid in _JOBS:
                _JOBS[jid]["status"] = job_status
                _JOBS[jid]["result"] = {k: res.get(k) for k in
                                        ("status", "failed_stage", "failed_note", "usd", "minutes_total", "gate_rejects")
                                        if res.get(k) is not None}
        emit("job", "done" if job_status == "done" else "gate", job_id=jid,
             detail=f"{status} ({res.get('failed_stage') or 'todas las etapas'})")
    except Exception as e:
        emit("job", "error", job_id=jid, detail=str(e)[:200])
        with _JOBS_LOCK:
            if jid in _JOBS:
                _JOBS[jid]["status"] = "error"
    finally:
        if wt:
            try:                                   # docs/sdlc + trabajo parcial quedan en la branch (reanudable)
                cap = wt.capture(f"mmorch sdlc: {task[:60]}")
                if cap["changed"] and not cap["committed"]:
                    emit("job", "error", job_id=jid,
                         detail=f"commit final falló, trabajo NO guardado: {cap['error'][:160]}")
                # la etapa 6 ya commitea: el diff del ultimo commit no mostraba la feature (2026-09-16)
                rc, total = _git(wt.path, "diff", "--stat", base, "HEAD") if base else (1, "")
                with _JOBS_LOCK:
                    if jid in _JOBS:
                        _JOBS[jid]["diffstat"] = (total if rc == 0 else "") or cap.get("diffstat", "")
            except Exception as e:
                emit("job", "warn", job_id=jid, detail=f"commit final no confirmado: {str(e)[:150]}")
            finally:
                wt.close(keep_branch=True)          # review branch survives for merge / resume


def _run_fanout_job(prompts: list, gen_model: str, parent=None, job_id: str | None = None):
    from .patterns import fan_out
    jid = job_id or uuid.uuid4().hex[:10]   # D2 (Lotus): id acordado con el handler
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
