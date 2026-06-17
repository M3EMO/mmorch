"""classify_and_act — rutear por TIPO y manejar cada rama distinto (triage, model
routing). El 7mo patron del catalogo + la 'puerta de entrada' opcional de mmorch: un
modelo BARATO clasifica el request en una de N clases; si hay handler para esa clase,
lo DISPARA (act); si no, o si la confianza es baja, ESCALA al orquestador (Opus).

Clasificacion = un solo modelo barato (rol classification), NO cross-family (no
verifica el output de un generador, etiqueta). Confidence-gated: baja confianza ->
no actua, escala (anti-misfire). Los handlers son callables Python (pueden ser otros
patrones mmorch: fan_out, cascade, tournament...) -> componible.
"""
from __future__ import annotations

import re
from dataclasses import dataclass
from typing import Any, Callable

from .config import DEFAULT_ROUTER
from .providers import call
from .route import _extract_conf

_CLASS_RE = re.compile(r"CLASS\s*[:=]\s*([A-Za-z0-9_\-]+)", re.I)


@dataclass
class ClassifyResult:
    cls: str | None          # clase elegida (None si no clasifico / escala)
    confidence: float
    handled: bool            # True si un handler actuo
    escalate: bool           # True si Opus debe intervenir (sin handler o baja conf)
    result: Any = None       # output del handler si actuo
    cost_usd: float = 0.0


def classify(request: str, classes: dict[str, str], *,
             router_model: str = DEFAULT_ROUTER, phase: str = "") -> tuple[str | None, float, float]:
    """Elige una clase de `classes` ({nombre: descripcion}). Devuelve (clase|None, conf, cost).
    clase=None si el modelo no devolvio una etiqueta valida."""
    listing = "\n".join(f"- {k}: {v}" for k, v in classes.items())
    sys_msg = (
        "Sos un clasificador de triage. Elegi EXACTAMENTE una clase de la lista para el "
        "request. Al final, en lineas aparte, escribi:\nCLASS: <nombre exacto de la clase>\n"
        "CONFIDENCE: <0 a 1>")
    user = f"CLASES:\n{listing}\n\nREQUEST:\n{request}"
    res = call(router_model, [{"role": "system", "content": sys_msg},
                              {"role": "user", "content": user}],
               pattern="classify", node="classifier", phase=phase, temperature=0.0)
    conf = _extract_conf(res.text)
    m = _CLASS_RE.search(res.text or "")
    cls = None
    if m:
        cand = m.group(1)
        for k in classes:                      # match case-insensitive contra claves validas
            if k.lower() == cand.lower():
                cls = k
                break
    return cls, conf, res.cost_usd


def classify_and_act(
    request: str,
    *,
    classes: dict[str, str],
    handlers: dict[str, Callable[[str], Any]] | None = None,
    router_model: str = DEFAULT_ROUTER,
    threshold: float = 0.6,
    phase: str = "",
) -> ClassifyResult:
    """Clasifica + (opcional) actua. Escala a Opus si: clase invalida, conf < threshold,
    o no hay handler para la clase. Un handler que tira excepcion -> escala (no traga)."""
    cls, conf, cost = classify(request, classes, router_model=router_model, phase=phase)
    if cls is None or conf < threshold:
        return ClassifyResult(cls, conf, False, True, None, round(cost, 6))
    if handlers and cls in handlers:
        try:
            result = handlers[cls](request)
            return ClassifyResult(cls, conf, True, False, result, round(cost, 6))
        except Exception as e:
            return ClassifyResult(cls, conf, False, True,
                                  {"handler_error": type(e).__name__, "msg": str(e)[:200]},
                                  round(cost, 6))
    # clase valida y confiable pero sin handler -> el orquestador actua.
    return ClassifyResult(cls, conf, False, True, None, round(cost, 6))


# --- P1: taxonomia Cynefin como preset de ruteo -----------------------------
# DART/Video-1: la pregunta de "Analyze" reduce el diagnostico a una sola cosa,
# la relacion causa->efecto. Cada dominio mapea a una estrategia mmorch distinta.
# NO es un modulo aparte: es classify() con un set de clases curado + un mapa de
# estrategia. El caller decide los handlers; este helper solo etiqueta y recomienda.
CYNEFIN_CLASSES: dict[str, str] = {
    "clear":       "Causa->efecto OBVIA. Proceso estable y conocido; seguir los pasos basta.",
    "complicated": "Causa->efecto DESCUBRIBLE con analisis o un experto. La respuesta existe pero no es inmediata.",
    "complex":     "Causa->efecto EMERGENTE, solo visible en hindsight. Hay que probar chico y adaptar.",
    "chaotic":     "Causa->efecto ROTO. Info incompleta y cambiante. Actuar/estabilizar YA, sin analizar.",
}

# Patron mmorch recomendado por dominio (heuristica del diseño; el caller decide
# si lo respeta). chaotic -> Opus interviene ya, sin iteracion barata.
CYNEFIN_STRATEGY: dict[str, str] = {
    "clear":       "direct_cheap",             # modelo barato directo, sin escalada
    "complicated": "route",                    # confidence-gated a especialista
    "complex":     "fan_out+ensemble_verify",  # diversidad de familias, directionally-right
    "chaotic":     "escalate_opus",            # actuar ya, sin test (Video-1: en caos no hay tiempo)
}


@dataclass
class CynefinResult:
    domain: str | None       # clear|complicated|complex|chaotic (None si invalida/baja-conf)
    confidence: float
    strategy: str            # patron mmorch recomendado (CYNEFIN_STRATEGY o "escalate_opus")
    escalate: bool           # True -> el orquestador (Opus) toma el control
    cost_usd: float = 0.0


def cynefin_classify(request: str, *, router_model: str = DEFAULT_ROUTER,
                     threshold: float = 0.6, phase: str = "cynefin") -> CynefinResult:
    """Clasifica el request en un dominio Cynefin y recomienda un patron mmorch.
    Escala a Opus si: clase invalida, conf < threshold, o dominio 'chaotic' (en caos
    la jugada es actuar ya, no rutear barato). Etiqueta, no actua — el caller wirea
    los handlers via classify_and_act(classes=CYNEFIN_CLASSES, ...) si quiere dispatch."""
    cls, conf, cost = classify(request, CYNEFIN_CLASSES, router_model=router_model, phase=phase)
    cost = round(cost, 6)
    if cls is None or conf < threshold:
        return CynefinResult(cls, conf, "escalate_opus", True, cost)
    return CynefinResult(cls, conf, CYNEFIN_STRATEGY[cls], cls == "chaotic", cost)
