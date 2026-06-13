"""effort — knob explicito de esfuerzo -> tier de modelo (patron Fable 5: 'effort' controla
profundidad). mmorch ya cascadea barato->caro; esto nombra los escalones por ESFUERZO pa que
el caller pida 'low/med/high' en vez de hardcodear modelos. low=flash no-think (bulk barato),
med=flash thinking (razona al precio flash), high=v4-pro thinking (tareas duras, 12x).

No rutea solo: es un mapa. El bandit/cascade siguen eligiendo umbrales; esto fija el modelo
por nivel de esfuerzo declarado.
"""
from __future__ import annotations

_EFFORT_MODEL = {
    "low": "deepseek-chat",        # v4-flash no-thinking
    "med": "deepseek-reasoner",    # v4-flash thinking
    "high": "deepseek-v4-pro",     # v4-pro thinking
}
_ORDER = ["low", "med", "high"]


def model_for_effort(effort: str) -> str:
    """Modelo pal nivel de esfuerzo. Default 'med' si el nivel es desconocido."""
    return _EFFORT_MODEL.get(effort, _EFFORT_MODEL["med"])


def escalation_models(max_effort: str = "high") -> list[str]:
    """Cadena de modelos barato->caro hasta `max_effort` (pa armar steps de cascade).
    Ej max_effort='high' -> [deepseek-chat, deepseek-reasoner, deepseek-v4-pro]."""
    cap = _ORDER.index(max_effort) if max_effort in _ORDER else len(_ORDER) - 1
    return [_EFFORT_MODEL[e] for e in _ORDER[:cap + 1]]


def effort_steps(thresholds: dict[str, float] | None = None,
                 max_effort: str = "high") -> list[tuple[str, float]]:
    """Steps (model, threshold) pa cascade(), ordenados por esfuerzo creciente. threshold por
    nivel (default: low 0.6, med 0.8, high 0.9 = mas exigente cuanto mas caro el escalon)."""
    th = thresholds or {"low": 0.6, "med": 0.8, "high": 0.9}
    cap = _ORDER.index(max_effort) if max_effort in _ORDER else len(_ORDER) - 1
    return [(_EFFORT_MODEL[e], th.get(e, 0.8)) for e in _ORDER[:cap + 1]]
