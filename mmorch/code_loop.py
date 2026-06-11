"""code_loop — el WIRE de Fase 5 a produccion: tareas de CODIGO con lazo cerrado.

Flujo: cascade genera codigo (prior contextual prima la seleccion de umbral si scale>0)
-> checker EJECUTA (python_exec: codigo + tests como script aislado) -> reward = paso/fallo
-> bandit.update + record_outcome(context=prompt). La label es EJECUCION, jamas la
self-confidence (anti-sicofancia). Cada outcome alimenta al ShadowPrior: con >= _MIN_FRESH
outcomes nuevos, auto_scale puede subir un escalon — Fase 5 escala sola, gated por datos.

Por que context=prompt (no el codigo generado): el prior selecciona ANTES de generar;
select y update deben ver el MISMO contexto pa que el k-NN encuentre vecinos.
"""
from __future__ import annotations

import re
from dataclasses import dataclass, field

from .cascade import cascade, CascadeResult
from .checkers import check
from .feedback import ThompsonBandit, record_outcome

_FENCE = re.compile(r"```(?:python)?\s*(.*?)```", re.DOTALL)


def extract_code(text: str) -> str:
    m = _FENCE.search(text)
    return (m.group(1) if m else text).strip()


@dataclass
class CodeTaskResult:
    code: str
    passed: bool
    reward: float
    arm: str
    escalated: bool
    confidence: float          # self-conf reportada (NO es el reward; queda pa calibracion)
    cost_usd: float
    detail: str = ""
    models_used: list[str] = field(default_factory=list)


def run_code_task(
    prompt: str,
    tests: str,
    *,
    steps: list[tuple[str, float]] | None = None,
    bandit: ThompsonBandit | None = None,
    thr_candidates: dict[int, list[float]] | None = None,
    prior=None,
    system: str | None = None,
    phase: str = "code_loop",
    timeout: float = 10.0,
) -> CodeTaskResult:
    """Genera codigo via cascade, lo EJECUTA contra `tests` (asserts), cierra el lazo.

    - reward = 1.0 si el script (codigo+tests) corre verde en sandbox, 0.0 si no.
    - El lazo se cierra ACA (unico lugar con verdad de campo inmediata): bandit.update
      + record_outcome(pattern='code_loop', context=prompt, predicted_conf=self_conf).
    - Si cascade escalo (escalate=True), igual se ejecuta y registra: el orquestador
      decide que hacer con el codigo, pero el dato ya quedo pal prior.
    """
    sysmsg = (system or "You are a Python programmer. Output ONLY the function source "
              "code in a python code block, no explanation.")
    res: CascadeResult = cascade(prompt, steps=steps, system=sysmsg, phase=phase,
                                 bandit=bandit, thr_candidates=thr_candidates, prior=prior)
    code = extract_code(res.answer)
    try:
        cr = check("python_exec", code=code + "\n" + tests, timeout=timeout)
        passed, detail = bool(cr.passed), cr.detail
    except Exception as e:
        passed, detail = False, f"checker error: {str(e)[:120]}"
    reward = 1.0 if passed else 0.0

    if res.arm:
        if bandit is not None:
            bandit.update(res.arm, reward)
        record_outcome(res.arm, reward, pattern="code_loop", source="execution",
                       predicted_conf=res.confidence, context=prompt)
    return CodeTaskResult(code, passed, reward, res.arm, res.escalate, res.confidence,
                          res.cost_usd, detail, res.models_used)
