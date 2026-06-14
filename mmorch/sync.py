"""sync — GitHub como bus de sincronizacion entre maquinas. El host always-on (ej pc-mateo)
EDITA un proyecto y pushea; las otras maquinas hacen auto-pull. Asi el trabajo pesado
(claude -p) corre en una sola PC y los cambios viajan por git, sin cargar las demas.

Reglas duras (codificadas):
- UN escritor: el agente pushea a una BRANCH (default 'mmorch/auto'), NO a main. El humano
  mergea. Evita conflictos entre maquinas.
- Auto-pull SOLO si el arbol esta limpio (sin cambios sin commitear) -> nunca pisa tu WIP.
- Secretos no se sincronizan (.env etc. ya gitignored). Esto solo mueve lo versionado.
- ff-only en pull: si divergio, NO mergea a ciegas -> avisa y se saltea.
"""
from __future__ import annotations

import subprocess
from .events import emit

AUTO_BRANCH = "mmorch/auto"


def _git(repo: str, *args: str, timeout: float = 120.0) -> tuple[int, str]:
    try:
        p = subprocess.run(["git", "-C", repo, *args], capture_output=True, text=True,
                           encoding="utf-8", errors="replace", timeout=timeout)
        return p.returncode, (p.stdout + p.stderr).strip()
    except Exception as e:
        return 1, str(e)[:200]


def is_clean(repo: str) -> bool:
    rc, out = _git(repo, "status", "--porcelain")
    return rc == 0 and out == ""


def current_branch(repo: str) -> str:
    rc, out = _git(repo, "rev-parse", "--abbrev-ref", "HEAD")
    return out if rc == 0 else ""


def commit_push(repo: str, message: str, *, branch: str = AUTO_BRANCH,
                remote: str = "origin", job_id: str = "") -> dict:
    """Tras un edit job: add -A, commit, push a `branch` (NO main). Idempotente si no hay
    cambios. Devuelve {ok, pushed, detail}."""
    if is_clean(repo):
        emit("step", "done", job_id=job_id, node="git", detail="sin cambios, nada que pushear")
        return {"ok": True, "pushed": False, "detail": "clean"}
    # branch dedicada del agente (crea o cambia)
    _git(repo, "checkout", "-B", branch)
    _git(repo, "add", "-A")
    rc_c, out_c = _git(repo, "commit", "-m", message)
    rc_p, out_p = _git(repo, "push", "-u", remote, branch)
    ok = rc_p == 0
    emit("step", "done" if ok else "error", job_id=job_id, node="git:push",
         detail=(f"-> {branch}" if ok else out_p[:160]))
    return {"ok": ok, "pushed": ok, "branch": branch, "detail": out_p[:200]}


def pull(repo: str, *, remote: str = "origin", job_id: str = "") -> dict:
    """Auto-pull SEGURO: solo si el arbol esta limpio; ff-only (no merge a ciegas).
    Si esta sucio o divergio -> se saltea y avisa (nunca pisa WIP local)."""
    if not is_clean(repo):
        emit("step", "gate", job_id=job_id, node="git:pull",
             detail=f"{repo}: arbol sucio -> pull SALTEADO (protege tu WIP)")
        return {"ok": False, "pulled": False, "reason": "dirty"}
    br = current_branch(repo)
    rc, out = _git(repo, "pull", "--ff-only", remote, br)
    ok = rc == 0
    emit("step", "done" if ok else "gate", job_id=job_id, node="git:pull",
         detail=(f"{repo}: {br} al dia" if ok else f"{repo}: no ff (divergio) -> manual"))
    return {"ok": ok, "pulled": ok, "branch": br, "detail": out[:200]}


def pull_all(*, job_id: str = "") -> dict:
    """Auto-pull de TODOS los proyectos registrados. Para la Scheduled Task de la otra PC."""
    from .projects import list_projects
    results = {}
    for name, path in list_projects().items():
        results[name] = pull(path, job_id=job_id)
    return {"pulled": results}


if __name__ == "__main__":   # CLI pa la Scheduled Task: python -m mmorch.sync pull-all
    import sys, json
    cmd = sys.argv[1] if len(sys.argv) > 1 else "pull-all"
    if cmd in ("pull-all", "pull_all"):
        print(json.dumps(pull_all(), ensure_ascii=False, default=str))
    else:
        print("uso: python -m mmorch.sync pull-all")
