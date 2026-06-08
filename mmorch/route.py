"""route (I-2) — confidence-gated escalation. Ahorra cupo: el modelo barato
responde y auto-reporta confianza; solo se escala (al orquestador/Opus) si la
confianza < umbral. Si la confianza es alta, se resuelve barato sin tocar cupo.
Opus NUNCA es nodo: route NO llama a Opus; devuelve escalate=True para que el
caller (Opus) decida. Respeta observabilidad (call loggea).
"""
from __future__ import annotations

import re
from dataclasses import dataclass

from .config import DEFAULT_GENERATOR
from .providers import call

_CONF_RE = re.compile(r"CONFIDENCE\s*[:=]\s*([01](?:\.\d+)?)", re.I)


@dataclass
class RouteResult:
    answer: str
    confidence: float          # conf cruda auto-reportada
    escalate: bool
    model: str
    cost_usd: float
    calibrated_conf: float | None = None  # conf mapeada a P(correcto) empirica (#3)


def _extract_conf(text: str) -> float:
    m = _CONF_RE.search(text or "")
    if not m:
        return 0.5  # sin senal -> conservador (a mitad de camino)
    try:
        return max(0.0, min(1.0, float(m.group(1))))
    except ValueError:
        return 0.5


def route(
    prompt: str,
    *,
    gen_model: str = DEFAULT_GENERATOR,
    threshold: float = 0.7,
    system: str | None = None,
    phase: str = "",
    calibrated: bool = True,
) -> RouteResult:
    """Genera en modelo barato + self-score. escalate=True si conf < threshold.

    #3: la conf auto-reportada MIENTE (ECE alto medido en el feedback loop). Por
    default gateamos sobre la conf CALIBRADA (mapeada a P(correcto) empirica via los
    outcomes). Asi el umbral opera sobre una senal real, no sobre el self-score crudo.
    calibrated=False vuelve al gating crudo (A/B o si no hay data de calibracion).
    """
    sys_msg = (system + "\n" if system else "") + (
        "Al final, en una linea aparte, escribi exactamente: CONFIDENCE: <n> "
        "donde <n> es tu certeza de 0 a 1 sobre la respuesta."
    )
    res = call(
        gen_model,
        [{"role": "system", "content": sys_msg}, {"role": "user", "content": prompt}],
        pattern="route", node="gen", phase=phase,
    )
    conf = _extract_conf(res.text)
    answer = _CONF_RE.sub("", res.text).strip()
    gate_conf = conf
    cal = None
    if calibrated:
        from .feedback import calibrate_conf
        cal = calibrate_conf(conf, pattern="route")  # solo data de route (no cross-task)
        gate_conf = cal
    return RouteResult(
        answer=answer, confidence=conf, escalate=gate_conf < threshold,
        model=gen_model, cost_usd=res.cost_usd, calibrated_conf=cal,
    )
