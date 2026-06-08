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


def load_goal(path: Path = _GOAL_PATH) -> str:
    """Texto del contrato GOAL. Tira FileNotFoundError si no existe (el ancla es obligatoria)."""
    return Path(path).read_text(encoding="utf-8")


def goal_hash(path: Path = _GOAL_PATH) -> str:
    """sha256 del GOAL — pa auditar si cambió (cambiarlo es zona roja)."""
    return hashlib.sha256(load_goal(path).encode("utf-8")).hexdigest()[:16]


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
