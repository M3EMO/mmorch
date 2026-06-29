"""cascade — FrugalGPT-style multi-step confidence cascade (research: vault/research/
frugalgpt-cascade-y-model-routing). Extiende route (I-2) de 1 escalon a N: el modelo
mas barato responde + self-score; si conf >= umbral devuelve (resuelto barato), si no
escala al siguiente modelo (que ve la respuesta de baja confianza para refinar). Si se
agotan los pasos -> escalate=True (lo maneja el orquestador/Opus). Cada paso loggea.

Umbrales por paso configurables. El threshold-OPTIMIZER (FrugalGPT: opt con restriccion
de budget, alimentado por metrics) es follow-up — la critica cross-family marco que learn
hoy solo recomienda. Aca van umbrales fijos/explicitos.
"""
from __future__ import annotations

import random as _random
from dataclasses import dataclass, field

from .config import DEFAULT_GENERATOR, DEFAULT_VERIFIER
from .providers import call
from .route import _extract_conf, _CONF_RE
from .feedback import ThompsonBandit

_SELF_SCORE = (
    "Al final, en una linea aparte, escribi exactamente: CONFIDENCE: <n> "
    "donde <n> es tu certeza de 0 a 1 sobre la respuesta."
)


@dataclass
class CascadeResult:
    answer: str
    confidence: float
    resolved_step: int
    escalate: bool
    models_used: list[str] = field(default_factory=list)
    cost_usd: float = 0.0
    arm: str = ""              # brazo (model@thr) que resolvio — clave para cerrar el lazo
    arms: list[str] = field(default_factory=list)  # brazo elegido por paso


def cascade(
    prompt: str,
    *,
    steps: list[tuple[str, float]] | None = None,
    system: str | None = None,
    phase: str = "",
    bandit: ThompsonBandit | None = None,
    thr_candidates: dict[int, list[float]] | None = None,
    rng: _random.Random | None = None,
    calibrated: bool = True,
    prior=None,
) -> CascadeResult:
    """Cascade barato->caro con umbral por paso. steps = [(model, threshold), ...].

    Si se pasa `bandit` + `thr_candidates`, el umbral de cada paso NO es fijo: el
    bandit Thompson elige entre los candidatos (arm = f"{model}@{thr}"). Asi mmorch
    APRENDE el umbral de escalada (FrugalGPT threshold-optimizer, gradient-free) en
    vez de hardcodearlo. El lazo se CIERRA afuera: cuando hay label, el caller hace
    `bandit.update(result.arm, reward)` + `record_outcome(result.arm, reward,
    pattern='cascade', predicted_conf=result.confidence)`. NO se auto-premia: en el
    momento de la llamada no hay verdad de campo (anti-sicofancia: la conf
    auto-reportada NO es el reward).

    Fase 5: si ademas se pasa `prior` (ShadowPrior con scale>0), la seleccion de brazo
    usa prior.select(bandit, cands, context=prompt) — el k-NN contextual prima los
    pseudo-conteos Beta. Con scale=0 es bit-a-bit identico al bandit puro."""
    steps = steps or [(DEFAULT_GENERATOR, 0.7), (DEFAULT_VERIFIER, 0.85)]
    used: list[str] = []
    arms: list[str] = []
    total = 0.0
    answer = ""
    conf = 0.0
    prev_answer = ""
    resolving_arm = ""
    for i, (model, default_thr) in enumerate(steps):
        thr = default_thr
        arm = f"{model}@{thr}"
        if bandit is not None and thr_candidates and thr_candidates.get(i):
            cands = [f"{model}@{t}" for t in thr_candidates[i]]
            if prior is not None:
                arm = prior.select(bandit, cands, context=prompt, rng=rng)
            else:
                arm = bandit.select(cands, rng=rng)
            thr = float(arm.rsplit("@", 1)[1])
        arms.append(arm)
        sys_msg = (system + "\n" if system else "") + _SELF_SCORE
        user = prompt if not prev_answer else (
            f"{prompt}\n\n[Un modelo mas barato respondio con baja confianza:\n{prev_answer}\n"
            f"Mejorala o corregila si hace falta.]")
        res = call(model, [{"role": "system", "content": sys_msg},
                           {"role": "user", "content": user}],
                   pattern="cascade", node=f"step{i}:{model}", phase=phase)
        used.append(model)
        total += res.cost_usd
        conf = _extract_conf(res.text)
        answer = _CONF_RE.sub("", res.text).strip()
        # #3: gatear sobre conf CALIBRADA (la cruda miente, ECE alto). gate_conf cae a
        # la P(correcto) empirica del bucket -> escala cuando el self-score infla.
        gate_conf = conf
        if calibrated:
            from .feedback import calibrate_conf
            gate_conf = calibrate_conf(conf, pattern="cascade")  # solo data de cascade
        if gate_conf >= thr:
            return CascadeResult(answer, conf, i, False, used, round(total, 6), arm, arms)
        prev_answer = answer
        resolving_arm = arm
    # Pasos agotados sin alcanzar umbral -> escalar al orquestador (Opus).
    return CascadeResult(answer, conf, len(steps) - 1, True, used, round(total, 6),
                         resolving_arm, arms)
