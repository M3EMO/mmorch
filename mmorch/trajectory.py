"""trajectory — robo de Hermes: 'trajectory compression para entrenar la proxima
generacion de modelos tool-calling'. Captura las trayectorias de los loops (rubric_loop,
code_loop), las COMPRIME, y las convierte en dataset del flywheel.

Por que pega: ya generamos trabajo etiquetado por EJECUCION (cada paso de un loop tiene
codigo + que criterios pasaban). Hoy se tira al cerrar. Persistirlo = combustible directo
del code_embedder (SimCLR) y del ShadowPrior. El flywheel se auto-alimenta del trabajo real:
trabajo -> trayectoria -> dataset -> mejor encoder/prior -> mejor trabajo.

Compresion (Hermes): no guardamos todo el texto N veces. Por trayectoria guardamos:
  - task, criterios (solo descripciones)
  - steps: [{iter, code (truncado), failed:[ids]}]  <- la senal (code, label=¿paso?)
  - reward final, passed, n_iters
Label de cada step = TODOS los criterios checkable cumplidos en ese step (ejecucion, no
opinion). Eso alimenta el mismo shape (code,label) que oracle_dataset.

Skill distill (Hermes 'autonomous skill creation after complex tasks'): si la trayectoria
termino VERDE tras >=2 iteraciones, destila un SKILL reusable (patron de tarea + codigo
ganador + correcciones clave) a logs/skills.jsonl + nota verificada. La proxima vez se
CONSULTA en vez de re-derivar (progresion FALLAR->...->CONSULTAR).
"""
from __future__ import annotations

import json
import pathlib
import time

_ROOT = pathlib.Path(__file__).resolve().parents[1]
_TRAJ = _ROOT / "logs" / "trajectories.jsonl"
_SKILLS = _ROOT / "logs" / "skills.jsonl"


def _checkable_ids(state: dict) -> set[str]:
    return {c["id"] for c in state["criteria"] if c.get("kind") == "checkable"}


def compress(state: dict) -> dict:
    """Estado de loop cerrado -> trayectoria comprimida (JSON-serializable)."""
    chk = _checkable_ids(state)
    steps = []
    for s in state.get("trace", []):
        failed = set(s.get("failed", []))
        # label del step: ¿paso TODOS los criterios checkable? (ejecucion pura)
        chk_pass = bool(chk) and not (chk & failed)
        steps.append({"iter": s["iter"], "code": s["code"],
                      "failed": sorted(failed), "checkable_pass": chk_pass})
    total = len(state["criteria"]) or 1
    ok = sum(1 for r in state["results"].values() if r.get("cumple"))
    return {
        "task": state["task"][:2000],
        "criteria": [{"id": c["id"], "desc": c["desc"], "kind": c["kind"]}
                     for c in state["criteria"]],
        "steps": steps,
        "n_iters": state.get("iteration", 0),
        "reward": round(ok / total, 4),
        "passed": state.get("phase") == "done",
        "gen_model": state.get("gen_model", ""),
        "arm": state.get("arm", ""),
    }


def record_trajectory(state: dict, *, path: pathlib.Path | None = None) -> dict:
    """Append-only. Devuelve la trayectoria comprimida. Tambien destila skill si aplica."""
    path = path or _TRAJ
    traj = compress(state)
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "a", encoding="utf-8") as f:
        f.write(json.dumps(traj, ensure_ascii=False) + "\n")
    if traj["passed"] and traj["n_iters"] >= 2:
        try:
            distill_skill(traj)
        except Exception:
            pass
    return traj


def distill_skill(traj: dict, *, path: pathlib.Path | None = None) -> dict:
    """Trayectoria verde-tras-correccion -> skill reusable. El codigo ganador es el ultimo
    step; las 'correcciones clave' = criterios que fallaban al principio y al final no.

    ANTI-degradacion (objecion del goal-gate): solo destila + marca verified si la trayectoria
    paso por un oraculo de EJECUCION (>=1 criterio checkable y el ultimo step lo cumplio). Un
    'verde' de juez subjetivo NO basta pa escribir a memoria como verificado — la label tiene
    que venir de ejecucion, no de opinion (mismo invariante anti-sicofancia del resto)."""
    path = path or _SKILLS
    steps = traj["steps"]
    if not steps:
        return {}
    has_checkable = any(c.get("kind") == "checkable" for c in traj.get("criteria", []))
    exec_verified = has_checkable and steps[-1].get("checkable_pass", False)
    if not exec_verified:
        return {}   # sin verdad de ejecucion no se destila (no contaminar memoria)
    winning = steps[-1]["code"]
    first_failed = set(steps[0]["failed"])
    last_failed = set(steps[-1]["failed"])
    fixed = sorted(first_failed - last_failed)
    skill = {
        "task_pattern": traj["task"][:300],
        "winning_code": winning,
        "fixed_criteria": fixed,
        "n_iters": traj["n_iters"],
        "criteria": traj["criteria"],
    }
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "a", encoding="utf-8") as f:
        f.write(json.dumps(skill, ensure_ascii=False) + "\n")
    # tambien a memoria semantica (consultable por recall)
    try:
        from .memory import write_note
        write_note("skill",
                   f"[skill destilado] '{traj['task'][:120]}' resuelto en {traj['n_iters']} "
                   f"iter. Correcciones clave: {', '.join(fixed) or 'ninguna'}. "
                   f"Codigo ganador ({len(winning)} chars) en logs/skills.jsonl.",
                   verified=True)
    except Exception:
        pass
    return skill


# --------------------------------------------------------------------------- #
# Consumo: trayectorias -> dataset (code, label) pal flywheel (mismo shape que oracle)
# --------------------------------------------------------------------------- #
def load_trajectories(path: pathlib.Path | None = None) -> list[dict]:
    path = path or _TRAJ
    if not path.exists():
        return []
    return [json.loads(ln) for ln in path.read_text(encoding="utf-8").splitlines() if ln.strip()]


def trajectory_dataset(path: pathlib.Path | None = None, *, min_chars: int = 20) -> list[dict]:
    """Aplana trayectorias -> [{code, label, source}] etiquetado por EJECUCION.
    label = 1 si el step paso todos los checkable, 0 si no. Dedup por codigo.
    Esto es lo que come simclr/code_embedder: trabajo real -> training data."""
    path = path or _TRAJ
    seen, out = set(), []
    for traj in load_trajectories(path):
        if not any(c["kind"] == "checkable" for c in traj["criteria"]):
            continue   # sin oraculo de ejecucion, no hay label confiable
        for s in traj["steps"]:
            code = s["code"]
            if len(code) < min_chars:
                continue
            h = hash(code)
            if h in seen:
                continue
            seen.add(h)
            out.append({"code": code, "label": 1 if s["checkable_pass"] else 0,
                        "source": "trajectory"})
    return out


def record_simple(task: str, code: str, passed: bool, *, arm: str = "",
                  path: pathlib.Path | None = None) -> dict:
    """Trayectoria de 1 paso (code_loop): codigo + label ejecucion. Mismo formato."""
    traj = {
        "task": task[:2000],
        "criteria": [{"id": "exec", "desc": "pasa sus tests", "kind": "checkable"}],
        "steps": [{"iter": 1, "code": code[:4000], "failed": [] if passed else ["exec"],
                   "checkable_pass": bool(passed)}],
        "n_iters": 1, "reward": 1.0 if passed else 0.0, "passed": bool(passed),
        "gen_model": "", "arm": arm,
    }
    path = path or _TRAJ
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "a", encoding="utf-8") as f:
        f.write(json.dumps(traj, ensure_ascii=False) + "\n")
    return traj


def stats(path: pathlib.Path | None = None) -> dict:
    path = path or _TRAJ
    trajs = load_trajectories(path)
    ds = trajectory_dataset(path)
    return {"trajectories": len(trajs),
            "passed": sum(1 for t in trajs if t["passed"]),
            "dataset_examples": len(ds),
            "pos": sum(1 for d in ds if d["label"] == 1),
            "neg": sum(1 for d in ds if d["label"] == 0)}
