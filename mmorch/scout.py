"""scout — pre-pass ENTORNO-PRIMERO (el patron central de Fable 5: 'primero aprende el
entorno, identifica archivos/tools/restricciones, despues construye sobre esa imagen
aterrizada'). Antes de que el ejecutor genere, scout arma un BRIEF de grounding que se
inyecta como prefijo ESTABLE del prompt -> el ejecutor arranca aterrizado -> menos
iteraciones de correccion.

Casi todo es DETERMINISTA ($0): las restricciones salen de la rubrica, los tools de
checkers.available(), los archivos de un glob opcional. El brief en prosa via LLM es
OPT-IN (use_llm=True); por default scout no gasta nada.

HONESTIDAD anti-scope-creep: que el scout REDUZCA iteraciones es una hipotesis MEDIBLE,
no un hecho. trajectory.py ya loggea n_iters; scout_delta() compara con/sin scout sobre
las trayectorias guardadas. No se asume el ahorro: se mide.
"""
from __future__ import annotations

import glob as _glob
import os


def gather_environment(task: str, criteria: list[dict], *, repo_path: str | None = None,
                       file_glob: str | None = None) -> dict:
    """Grounding DETERMINISTA (cero API): restricciones de la rubrica + tools relevantes +
    archivos (si se da repo_path/file_glob)."""
    from .checkers import available as _checkers
    constraints = []
    for c in criteria:
        kind = c.get("kind", "?")
        if kind == "checkable":
            constraints.append(f"[{c.get('id')}] (verifica checker {c.get('checker')}) {c.get('desc','')}")
        else:
            constraints.append(f"[{c.get('id')}] (juez subjetivo) {c.get('desc','')}")
    tools = _checkers()
    files: list[str] = []
    if repo_path and file_glob:
        try:
            files = sorted(_glob.glob(os.path.join(repo_path, file_glob), recursive=True))[:50]
        except Exception:
            files = []
    return {"constraints": constraints, "tools": tools, "files": files}


def scout_brief_text(env: dict, llm_brief: str = "") -> str:
    """Formatea el grounding en un bloque ESTABLE pa prefijo (cacheable). Determinista."""
    parts = ["GROUNDING (entorno antes de construir):"]
    if env.get("constraints"):
        parts.append("Restricciones a cumplir:\n" + "\n".join(f"- {x}" for x in env["constraints"]))
    if env.get("tools"):
        parts.append("Verificadores deterministas disponibles: " + ", ".join(env["tools"]))
    if env.get("files"):
        parts.append("Archivos relevantes:\n" + "\n".join(f"- {f}" for f in env["files"]))
    if llm_brief.strip():
        parts.append("Notas de exploracion:\n" + llm_brief.strip())
    return "\n\n".join(parts)


def scout(task: str, criteria: list[dict], *, use_llm: bool = False, gen_model: str | None = None,
          repo_path: str | None = None, file_glob: str | None = None) -> dict:
    """Pre-pass entorno-primero. Determinista por default; use_llm=True suma UNA call barata
    que escribe notas de exploracion (pitfalls, enfoque) antes de construir. Devuelve
    {brief, constraints, tools, files}."""
    env = gather_environment(task, criteria, repo_path=repo_path, file_glob=file_glob)
    llm_brief = ""
    if use_llm:
        try:
            from .config import DEFAULT_GENERATOR
            from .providers import call
            from .prompts import cacheable_messages
            gm = gen_model or DEFAULT_GENERATOR
            sysmsg = ("Sos un SCOUT: NO resolvas la tarea. En 3-5 bullets cortos identifica el "
                      "enfoque, los pitfalls probables y que restriccion es la mas riesgosa. "
                      "Aterrizado y breve.")
            msgs = cacheable_messages(sysmsg, {"constraints": env["constraints"],
                                               "tools": env["tools"]}, f"TAREA:\n{task}")
            r = call(gm, msgs, max_tokens=300, pattern="scout", node="scout")
            llm_brief = r.text.strip()
        except Exception:
            llm_brief = ""
    return {"brief": scout_brief_text(env, llm_brief), **env, "llm_brief": llm_brief}


def scout_delta(path=None) -> dict:
    """MIDE la hipotesis: ¿scout reduce iteraciones? Compara n_iters medio de trayectorias
    con scout (task que arranca con 'GROUNDING') vs sin, sobre trajectory.load_trajectories.
    Honesto: si n es chico o el delta ~0, scout NO se justifica (anti-scope-creep)."""
    from .trajectory import load_trajectories
    with_s, without_s = [], []
    for t in load_trajectories(path):
        (with_s if t.get("scout") else without_s).append(t.get("n_iters", 0))
    def _avg(xs):
        return round(sum(xs) / len(xs), 3) if xs else None
    return {"with_scout": {"n": len(with_s), "avg_iters": _avg(with_s)},
            "without_scout": {"n": len(without_s), "avg_iters": _avg(without_s)},
            "delta_iters": (None if not with_s or not without_s
                            else round(_avg(without_s) - _avg(with_s), 3))}
