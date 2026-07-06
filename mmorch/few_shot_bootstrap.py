"""few_shot_bootstrap — DSPy-A leído del código (bootstrap.py de stanfordnlp/dspy), robado como
patrón sobre la infra que YA existe en mmorch (graft #2 del research de libs, 2026-07).

DSPy corre un "teacher" sobre ejemplos con trace, acepta el primer set que pasa la métrica, y lo
inyecta como demos del predictor. mmorch no necesita el teacher-loop: `trajectory.py` YA graba
(task, code, passed) de cada run real de code_loop — es la trace + la métrica (ejecución) que
DSPy tiene que generar aparte. Este módulo es la mitad que faltaba: agrupar por FIRMA (signature.py,
la misma clave que usa el sig-bandit) y devolver los mejores few-shots verificados por ejecución
para inyectar en el prompt del coder — nunca ejemplos curados a mano, nunca sin pasar el gate.

Trampas robadas del código de DSPy (bootstrap.py / random_search.py):
  - cache-variation: sin esto, re-correr sobre la misma task devuelve SIEMPRE el mismo output
    cacheado -> cero pool de candidatos. No aplica acá (consumimos trayectorias YA variadas).
  - overfitting al metric con pool chico: min_n exige evidencia suficiente antes de ofrecer
    few-shots (si no hay bastante, devuelve [] -> el coder sigue zero-shot, no adivina).
"""
from __future__ import annotations

from .signature import signature
from .trajectory import load_trajectories

_MAX_CODE_CHARS = 2000   # el few-shot es EJEMPLO, no el archivo completo — cap para no inflar el prompt


def _sig_key(task: str) -> str:
    return signature(task).to_key()


def bootstrap_few_shots(sig_key_target: str, *, k: int = 2, min_n: int = 3,
                        trajectories: list[dict] | None = None) -> list[dict]:
    """Few-shots VERIFICADOS POR EJECUCIÓN para la firma dada. Filtra trayectorias con
    `passed=True` cuya firma coincide, ordena por menos-iteraciones-para-verde (señal de que
    el patrón es limpio, no que tardó en tantear), devuelve hasta `k`. Si hay MENOS de `min_n`
    trayectorias verdes para esta firma, devuelve [] (evidencia insuficiente — no ofrecer
    few-shots basados en 1-2 muestras es la misma disciplina que `intuition.decide`'s min_n).
    `trajectories` inyectable (test seam); default = trajectory.load_trajectories()."""
    rows = trajectories if trajectories is not None else load_trajectories()
    matches = [r for r in rows if r.get("passed") and _sig_key(r.get("task", "")) == sig_key_target]
    if len(matches) < min_n:
        return []
    matches.sort(key=lambda r: r.get("n_iters", 999))   # menos iteraciones = patrón más limpio
    out = []
    for r in matches[:k]:
        code = ""
        for step in reversed(r.get("steps", [])):        # el ÚLTIMO step es el que verdeó
            if step.get("code"):
                code = step["code"][:_MAX_CODE_CHARS]
                break
        if code:
            out.append({"task": r["task"], "code": code, "n_iters": r.get("n_iters", 1)})
    return out


def render_few_shots(shots: list[dict]) -> str:
    """Few-shots -> bloque de texto inyectable en un prompt de rol. [] -> '' (el caller no debe
    agregar una sección vacía)."""
    if not shots:
        return ""
    parts = ["Ejemplos verificados por ejecución de tareas con esta forma:"]
    for s in shots:
        parts.append(f"TAREA: {s['task']}\nSOLUCIÓN (verde en {s['n_iters']} intento(s)):\n"
                     f"```\n{s['code']}\n```")
    return "\n\n".join(parts)


if __name__ == "__main__":
    # cero-API: trayectorias inyectadas, ninguna toca logs/ ni una API.
    TRAJ = [
        {"task": "Resolvé: def two_sum(nums, target)", "passed": True, "n_iters": 1,
         "steps": [{"code": "def two_sum(a,b): return (0,1)"}]},
        {"task": "Resolvé: def two_sum_v2(nums, target)", "passed": True, "n_iters": 3,
         "steps": [{"code": "def two_sum_v2(a,b): return (0,1)"}]},
        {"task": "Resolvé: def two_sum_v3(nums, target)", "passed": True, "n_iters": 2,
         "steps": [{"code": "def two_sum_v3(a,b): return (0,1)"}]},
        # firma DISTINTA: signature() clasifica por op_type/grounding — TRANSFORM+needs_codebase
        # vs GENERATE+self_contained de los two_sum de arriba (misma forma estructural = misma firma,
        # aun con nombres distintos: signature.py agrupa por forma, no por texto literal).
        {"task": "Refactorizá este modulo para usar async/await, con tests de integracion e2e",
         "passed": True, "n_iters": 1, "steps": [{"code": "async def f(): pass"}]},
        {"task": "Resolvé: def two_sum_v4(nums, target)", "passed": False, "n_iters": 3,   # rojo: excluido
         "steps": [{"code": "def two_sum_v4(a,b): return None"}]},
    ]
    two_sum_sig = _sig_key("Resolvé: def two_sum(nums, target)")

    # 1. evidencia insuficiente (min_n alto) -> [] , no adivina con pool chico
    assert bootstrap_few_shots(two_sum_sig, min_n=10, trajectories=TRAJ) == []

    # 2. suficiente evidencia -> top-k por menos iteraciones, solo passed=True
    shots = bootstrap_few_shots(two_sum_sig, k=2, min_n=3, trajectories=TRAJ)
    assert len(shots) == 2, shots
    assert shots[0]["n_iters"] == 1, shots           # el más limpio primero
    assert all("two_sum" in s["task"] for s in shots), shots
    assert all("v4" not in s["task"] for s in shots), shots   # el failed jamás entra

    # 3. firma distinta no contamina el pool (el refactor async no aparece pidiendo two_sum)
    same_sig_shots = bootstrap_few_shots(two_sum_sig, min_n=1, trajectories=TRAJ)
    assert all("async" not in s["task"] for s in same_sig_shots), same_sig_shots

    # 4. render: [] -> '' ; con contenido -> incluye tarea + código
    assert render_few_shots([]) == ""
    text = render_few_shots(shots)
    assert "two_sum" in text and "```" in text, text

    print("few_shot_bootstrap OK — filtro por firma+ejecución, min_n anti-overfit, render")
