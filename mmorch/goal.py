"""goal — ancla anti-goal-drift, modelada sobre el `/goal` nativo de Claude Code.

`/goal` nativo = condición + Stop-hook que BLOQUEA "done" hasta cumplirse. Acá el GOAL.md
bloquea la AUTO-APLICACIÓN: el loop de auto-evolución no cierra un ciclo hasta que el
cambio pasa `goal_aligned()` contra el contrato. Sin esto, `fitness()` solo mide proxies
(tests verdes + barato) y el sistema DERIVA del intento (goal drift — la falla que mmorch
nombra pero no anclaba).

Editar GOAL.md = ZONA ROJA. `goal_hash()` permite auditar si cambió sin gate humano.
"""
from __future__ import annotations

import hashlib
from pathlib import Path

from .config import DEFAULT_GENERATOR, DEFAULT_VERIFIER
from .patterns import adversarial_verify, Verdict

ROOT = Path(__file__).resolve().parent.parent
_GOAL_PATH = ROOT / "GOAL.md"
_GOAL_HASH_PATH = ROOT / "GOAL.hash"   # hash AUTORIZADO (re-escribirlo = gate humano)


class GoalTampered(RuntimeError):
    """GOAL.md cambió sin re-autorización → HALT de toda auto-aplicación (zona roja cat.4)."""


def load_goal(path: Path = _GOAL_PATH) -> str:
    """Texto del contrato GOAL. Tira FileNotFoundError si no existe (el ancla es obligatoria)."""
    return Path(path).read_text(encoding="utf-8")


def goal_hash(path: Path = _GOAL_PATH) -> str:
    """sha256 del GOAL — pa auditar si cambió (cambiarlo es zona roja)."""
    return hashlib.sha256(load_goal(path).encode("utf-8")).hexdigest()[:16]


def authorize_goal(path: Path = _GOAL_PATH, hash_path: Path = _GOAL_HASH_PATH) -> str:
    """Marca el GOAL actual como AUTORIZADO (acto humano = el gate de zona roja). Graba
    el hash baseline. Llamar SOLO cuando un humano aprobó el contenido del GOAL."""
    h = goal_hash(path)
    Path(hash_path).write_text(h, encoding="utf-8")
    return h


def goal_guard(path: Path = _GOAL_PATH, hash_path: Path = _GOAL_HASH_PATH) -> None:
    """Tamper-halt (análogo al hard-block del Stop-hook /goal). Si GOAL.md cambió vs el
    hash autorizado → GoalTampered (frena TODA auto-aplicación). Primera vez sin baseline
    → auto-autoriza (el GOAL inicial es el autorizado). Re-autorizar tras un cambio
    legítimo = `authorize_goal()` (gate humano)."""
    cur = goal_hash(path)
    p = Path(hash_path)
    if not p.exists():
        p.write_text(cur, encoding="utf-8")   # init: el GOAL presente es el autorizado
        return
    authorized = p.read_text(encoding="utf-8").strip()
    if cur != authorized:
        raise GoalTampered(
            f"GOAL.md cambió sin re-autorización (actual {cur} != autorizado {authorized}). "
            f"HALT auto-aplicación. Si el cambio es legítimo, un HUMANO corre authorize_goal().")


def pursue_goal(generate, *, max_rounds: int = 3, gen_model: str = DEFAULT_GENERATOR,
                verifier_model: str = DEFAULT_VERIFIER, path: Path = _GOAL_PATH):
    """Block-until-aligned con RETRY (el análogo productivo del /goal nativo: 'seguí hasta
    cumplir'). `generate(feedback: str|None) -> str` produce un cambio; si `goal_aligned`
    refuta, se realimenta la refutación y se regenera, hasta alinear o agotar max_rounds.
    Mismo patrón que schema.gated_json pero contra el GOAL. Devuelve
    {change, verdict, rounds, aligned}; aligned=False si se agotó sin pasar."""
    feedback = None
    last = None
    for r in range(1, max_rounds + 1):
        change = generate(feedback)
        v = goal_aligned(change, gen_model=gen_model, verifier_model=verifier_model, path=path)
        last = v
        if v.passed:
            return {"change": change, "verdict": v, "rounds": r, "aligned": True}
        feedback = ("El cambio NO alineó con el GOAL. Refutaciones: "
                    + "; ".join(v.refutations) + ". Corregí para alinear.")
    return {"change": None, "verdict": last, "rounds": max_rounds, "aligned": False}


def goal_aligned(change: str, *, gen_model: str = DEFAULT_GENERATOR,
                 verifier_model: str = DEFAULT_VERIFIER, path: Path = _GOAL_PATH,
                 phase: str = "goal") -> Verdict:
    """¿El cambio propuesto ALINEA con el GOAL? Verify adversarial CROSS-FAMILY: el
    cambio es el artefacto, el GOAL es la rúbrica. Refuta por default (anti-sicofancia):
    pasa solo si avanza el north star sin violar invariantes ni tocar non-goals y es
    reversible. Es el 6to check (no determinista) de fitness() — complementa a los
    checkers deterministas, no los reemplaza. Subjetivo → cross-family obligatorio.

    Devuelve Verdict {passed, confidence, refutations, ...}. passed=False bloquea la
    auto-aplicación aunque los tests estén verdes."""
    goal = load_goal(path)
    rubric = (
        f"{goal}\n\n"
        "--- TAREA DEL VERIFICADOR ---\n"
        "El ARTEFACTO es un CAMBIO propuesto a mmorch. Decidí si ALINEA con el GOAL de "
        "arriba. passed=true SOLO si: (1) avanza el north star, (2) NO viola NINGÚN "
        "invariante, (3) NO toca un non-goal, (4) es reversible. Refutá si deriva del "
        "norte, bloatea sin justificar, rompe un invariante, o entra en zona roja sin "
        "gate humano. Ante la duda, refutá."
    )
    return adversarial_verify(change, rubric=rubric, gen_model=gen_model,
                              verifier_model=verifier_model, phase=phase,
                              task_kind="subjective")
