"""project_loop — ejecutor PROJECT-AWARE primario via mmorch (barato, cero cupo). Es la
tesis aplicada a editar repos: DeepSeek GENERA, el orquestador determinista APLICA (escribe
el archivo), y la verdad la da la EJECUCION (corre los tests del repo) — no un juez LLM
(cero cupo, cero false-refute). claude -p (plan/cupo) queda como ESCALADA cuando mmorch no
puede (tarea abierta / K agotado).

Frontera honesta: brilla en tareas file-scoped (un archivo, hacer pasar tests). Para navegar
un repo grande y decidir QUE tocar, la escalada a claude -p es mejor.

Seguridad: trabaja sobre la branch del agente (mmorch/auto, via sync) -> reversible. Commit+
push solo si los tests pasan. Nunca toca main directo.
"""
from __future__ import annotations

import os
import re
import subprocess
from dataclasses import dataclass, field

from .events import emit

_FENCE = re.compile(r"```(?:python)?\s*(.*?)```", re.DOTALL)


def _extract(text: str) -> str:
    m = _FENCE.search(text)
    return (m.group(1) if m else text).strip()


def _run_cmd(cwd: str, cmd: str, timeout: float = 120.0) -> tuple[bool, str]:
    try:
        p = subprocess.run(cmd, cwd=cwd, shell=True, capture_output=True, text=True,
                           encoding="utf-8", errors="replace", timeout=timeout)
        out = (p.stdout + p.stderr)[-1500:]
        return p.returncode == 0, out
    except subprocess.TimeoutExpired:
        return False, "TIMEOUT"
    except Exception as e:
        return False, str(e)[:300]


@dataclass
class ProjectResult:
    ok: bool
    engine: str               # mmorch | claude
    iterations: int
    target_file: str
    pushed: bool = False
    escalated: bool = False
    detail: str = ""
    history: list = field(default_factory=list)


def run_project_task(project: str, task: str, *, target_file: str, test_cmd: str | None = None,
                     K: int = 4, escalate: bool = True, push: bool = True,
                     gen_model: str | None = None, job_id: str = "") -> ProjectResult:
    """mmorch-primario: loop DeepSeek genera el archivo -> escribe -> corre tests -> repite
    hasta verde o K. Verdad = ejecucion (test_cmd). Si K se agota y escalate: claude -p (cupo).
    push: si verde, commit+push a mmorch/auto. test_cmd None -> sin verificacion (no recomendado)."""
    from .config import DEFAULT_GENERATOR
    from .providers import call
    from .projects import resolve
    from .sync import commit_push, _git
    gen_model = gen_model or DEFAULT_GENERATOR
    cwd = resolve(project)
    fpath = os.path.join(cwd, target_file)
    emit("job", "running", job_id=job_id, detail=f"mmorch {project}/{target_file}: {task[:60]}")

    _git(cwd, "checkout", "-B", "mmorch/auto")   # branch del agente (reversible)
    cur = ""
    if os.path.isfile(fpath):
        try:
            cur = open(fpath, encoding="utf-8").read()
        except Exception:
            cur = ""

    history, feedback = [], ""
    for i in range(1, K + 1):
        emit("step", "running", job_id=job_id, node=f"gen[{i}]:{gen_model}", detail=target_file)
        prompt = (f"TAREA: {task}\n\nARCHIVO `{target_file}` (contenido actual):\n```\n{cur[:6000]}\n```\n"
                  + (f"\nEl intento anterior fallo los tests:\n{feedback[:1000]}\n"
                     "Corregilo.\n" if feedback else "")
                  + "Devolve SOLO el contenido COMPLETO nuevo del archivo en un bloque ```.")
        try:
            new = _extract(call(gen_model, [{"role": "user", "content": prompt}],
                                pattern="project_loop", node=f"gen[{i}]").text)
        except Exception as e:
            history.append({"iter": i, "error": str(e)[:120]}); continue
        if not new:
            continue
        with open(fpath, "w", encoding="utf-8") as f:
            f.write(new + ("\n" if not new.endswith("\n") else ""))
        cur = new
        if test_cmd is None:
            emit("step", "gate", job_id=job_id, node="tests", detail="sin test_cmd: NO verificado")
            history.append({"iter": i, "verified": False})
            break
        ok, out = _run_cmd(cwd, test_cmd)
        feedback = "" if ok else out
        history.append({"iter": i, "tests_pass": ok})
        emit("step", "done" if ok else "error", job_id=job_id, node="tests",
             detail="verde" if ok else out[-120:])
        if ok:
            pushed = False
            if push:
                pushed = commit_push(cwd, f"mmorch: {task[:72]}", job_id=job_id).get("pushed", False)
            emit("job", "done", job_id=job_id, detail=f"mmorch resolvio en {i} iter")
            return ProjectResult(True, "mmorch", i, target_file, pushed=pushed, history=history)

    # mmorch no pudo -> escalada a claude -p (plan/cupo) si esta habilitada
    if escalate:
        emit("step", "gate", job_id=job_id, node="escalate", detail="mmorch agoto K -> claude -p (cupo)")
        from .claude_exec import run_claude
        r = run_claude(task, cwd, mode="edit", job_id=job_id)
        pushed = False
        if r.get("ok") and push:
            pushed = commit_push(cwd, f"mmorch(claude): {task[:64]}", job_id=job_id).get("pushed", False)
        emit("job", "done" if r.get("ok") else "error", job_id=job_id, detail="via escalada claude")
        return ProjectResult(bool(r.get("ok")), "claude", K, target_file, pushed=pushed,
                             escalated=True, history=history)

    emit("job", "error", job_id=job_id, detail=f"mmorch no pudo en {K} iter (sin escalada)")
    return ProjectResult(False, "mmorch", K, target_file, history=history)
