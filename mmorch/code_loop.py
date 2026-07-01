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

import os
from dataclasses import dataclass, field

from .cascade import cascade, CascadeResult
from .checkers import check
from .feedback import ThompsonBandit, record_outcome

from .textutil import extract_fence as extract_code  # noqa: E402 (dedup of the local fence helper)


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
    intuition_models: list[str] | None = None,
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
    # intuition layer (ON by default — user's call; improves with data, see route.py). The loop keeps
    # TRAINING the sig-bandit (record_outcome below) AND consults it; Thompson self-corrects as
    # outcomes accrue. Off via MMORCH_INTUITION=off; intuition_models=[] opts a call out.
    if intuition_models is None and os.getenv("MMORCH_INTUITION", "on").lower() != "off":
        from .config import DEFAULT_INTUITION_POOL
        intuition_models = DEFAULT_INTUITION_POOL
    if intuition_models:
        try:
            from .intuition import decide
            act, picked, _ = decide(intuition_models, prompt)
            if act == "commit" and picked:
                thr0 = steps[0][1] if steps else 0.7
                steps = [(picked.split("@")[0], thr0)] + list(steps or [])
        except Exception:
            pass
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
    try:   # trayectoria pal flywheel (Hermes trajectory-compression)
        from .trajectory import record_simple
        record_simple(prompt, code, passed, arm=res.arm)
    except Exception:
        pass
    try:   # nudge: mantenimiento periodico de memoria (Hermes)
        from .nudge import tick
        tick()
    except Exception:
        pass
    return CodeTaskResult(code, passed, reward, res.arm, res.escalate, res.confidence,
                          res.cost_usd, detail, res.models_used)
