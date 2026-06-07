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

from dataclasses import dataclass, field

from .config import DEFAULT_GENERATOR, DEFAULT_VERIFIER
from .providers import call
from .route import _extract_conf, _CONF_RE

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


def cascade(
    prompt: str,
    *,
    steps: list[tuple[str, float]] | None = None,
    system: str | None = None,
    phase: str = "",
) -> CascadeResult:
    """Cascade barato->caro con umbral por paso. steps = [(model, threshold), ...]."""
    steps = steps or [(DEFAULT_GENERATOR, 0.7), (DEFAULT_VERIFIER, 0.85)]
    used: list[str] = []
    total = 0.0
    answer = ""
    conf = 0.0
    prior = ""
    for i, (model, thr) in enumerate(steps):
        sys_msg = (system + "\n" if system else "") + _SELF_SCORE
        user = prompt if not prior else (
            f"{prompt}\n\n[Un modelo mas barato respondio con baja confianza:\n{prior}\n"
            f"Mejorala o corregila si hace falta.]")
        res = call(model, [{"role": "system", "content": sys_msg},
                           {"role": "user", "content": user}],
                   pattern="cascade", node=f"step{i}:{model}", phase=phase)
        used.append(model)
        total += res.cost_usd
        conf = _extract_conf(res.text)
        answer = _CONF_RE.sub("", res.text).strip()
        if conf >= thr:
            return CascadeResult(answer, conf, i, False, used, round(total, 6))
        prior = answer
    # Pasos agotados sin alcanzar umbral -> escalar al orquestador (Opus).
    return CascadeResult(answer, conf, len(steps) - 1, True, used, round(total, 6))
