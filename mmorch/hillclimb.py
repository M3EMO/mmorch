"""hillclimb — optimizacion sobre METRICA ESCALAR con feedback del entorno
(Lance Martin, "Designing loops" 2026-06: medir -> proponer -> implementar ->
probar -> repetir; el rubric hace mas trabajo que el modelo).

Distinto de loop_until_done (discovery: "hasta que este limpio") y de
goal.pursue_goal (binario: alinea/no-alinea). Aca el entorno devuelve un NUMERO
y el loop se queda con mejoras: best solo avanza si score supera best + min_delta.

Regla anti-reward-hacking: `score` debe ser un rubric CORRIBLE y determinista
(checkers.check, tests, una metrica medida) — NUNCA un LLM-judge. Con modelos
baratos como generador, un rubric blando se gamea; uno determinista no
(hallazgo ablation_prompt: LLM-verify ~74% false-refute en computable).

Cierre del feedback loop (la 'loss' que feedback.py esperaba de afuera): el
reward por ronda ES el rubric (mejoro=1, no=0) — objetivo, automatico, sin
label humano y sin usar la conf auto-reportada (anti-sicofancia). Si se pasa
`arm`/`arms`, cada ronda hace record_outcome(source="rubric") y actualiza el
ThompsonBandit; con `arms` el bandit ELIGE el brazo por ronda (Thompson) y
aprende que generador mejora mas seguido.

Library-only (como loop_until_done): `propose`/`score` son callables del
caller — no cruzan MCP.
"""
from __future__ import annotations

import random as _random
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Callable

from .feedback import ThompsonBandit, record_outcome, _FEEDBACK_LOG


@dataclass
class ClimbCtx:
    """Lo que ve `propose` en cada ronda (estado del hillclimb + brazo elegido)."""
    round: int
    best: Any
    best_score: float | None
    history: list["ClimbStep"]
    arm: str | None = None


@dataclass
class ClimbStep:
    round: int
    candidate: Any
    score: float | None          # None = rubric exploto (candidato invalido)
    improved: bool
    arm: str | None = None
    detail: str = ""


@dataclass
class ClimbResult:
    best: Any
    best_score: float | None
    rounds: int
    stopped: str                 # target | patience | max_rounds | no_candidate
    history: list[ClimbStep] = field(default_factory=list)
    baseline: float | None = None


def hillclimb(
    propose: Callable[[ClimbCtx], Any],
    score: Callable[[Any], float],
    *,
    initial: Any = None,
    maximize: bool = True,
    target: float | None = None,
    max_rounds: int = 20,
    patience: int = 5,
    min_delta: float = 0.0,
    arm: str | None = None,
    arms: list[str] | None = None,
    bandit: ThompsonBandit | None = None,
    outcome_path: Path = _FEEDBACK_LOG,
    rng: _random.Random | None = None,
    pattern: str = "hillclimb",
    context: str = "",
    journal_path: Path | None = None,
) -> ClimbResult:
    """propose(ctx) -> candidato (None = no hay mas que proponer).
    score(candidato) -> float; si tira excepcion, la ronda cuenta como fallida
    (reward 0) pero el loop sigue — un candidato roto no mata la optimizacion.

    Para cuando: best alcanza `target`; `patience` rondas seguidas sin mejora
    real (> min_delta); `max_rounds`; o propose devuelve None.

    Feedback: con `arm` (fijo) o `arms` (eleccion Thompson por ronda) cada ronda
    se registra en outcome_path y, si hay `bandit`, actualiza su posterior.

    journal_path (qrf, extraido de autoresearch — el 'results.tsv', despertás a un
    log de experimentos): si se setea, cada ronda se APPENDEA como JSONL
    {round,score,best_score,improved,arm,detail,candidate(repr truncado)}. Append-only,
    sobrevive a una corrida overnight y es auditable. None = comportamiento de siempre.
    """
    if arm and arms:
        raise ValueError("pasar `arm` (fijo) O `arms` (eleccion por ronda), no ambos")
    if arms and bandit is None:
        bandit = ThompsonBandit()

    sign = 1.0 if maximize else -1.0
    best = initial
    baseline: float | None = None
    best_score: float | None = None
    if initial is not None:
        baseline = float(score(initial))
        best_score = baseline

    history: list[ClimbStep] = []
    dry = 0

    def _reached(s: float) -> bool:
        return target is not None and sign * s >= sign * target

    if best_score is not None and _reached(best_score):
        return ClimbResult(best, best_score, 0, "target", history, baseline)

    for r in range(1, max_rounds + 1):
        round_arm = arm
        if arms and bandit is not None:
            round_arm = bandit.select(arms, rng=rng)
        ctx = ClimbCtx(round=r, best=best, best_score=best_score,
                       history=history, arm=round_arm)
        cand = propose(ctx)
        if cand is None:
            return ClimbResult(best, best_score, r - 1, "no_candidate", history, baseline)

        try:
            s: float | None = float(score(cand))
            detail = ""
        except Exception as e:                     # rubric refuta rompiendose: ronda fallida
            s, detail = None, f"score error: {type(e).__name__}: {e}"

        improved = (
            s is not None
            and (best_score is None or sign * s > sign * best_score + min_delta)
        )
        history.append(ClimbStep(r, cand, s, improved, round_arm, detail))

        if journal_path is not None:                  # qrf: ledger append-only (autoresearch)
            import json as _json
            rec = {"round": r, "score": s, "best_score": best_score,
                   "improved": improved, "arm": round_arm, "detail": detail,
                   "candidate": repr(cand)[:300]}
            Path(journal_path).parent.mkdir(parents=True, exist_ok=True)
            with open(journal_path, "a", encoding="utf-8") as _f:
                _f.write(_json.dumps(rec, ensure_ascii=False) + "\n")

        if round_arm is not None:
            reward = 1.0 if improved else 0.0
            record_outcome(round_arm, reward, pattern=pattern, source="rubric",
                           context=context, path=outcome_path)
            if bandit is not None:
                bandit.update(round_arm, reward)

        if improved:
            best, best_score, dry = cand, s, 0
            if best_score is not None and _reached(best_score):
                return ClimbResult(best, best_score, r, "target", history, baseline)
        else:
            dry += 1
            if dry >= patience:
                return ClimbResult(best, best_score, r, "patience", history, baseline)

    return ClimbResult(best, best_score, max_rounds, "max_rounds", history, baseline)
