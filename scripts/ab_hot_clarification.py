"""A/B: does a BOUNDED architect<->coder clarification (hot) beat the cold artifact hand-off?

Tests the user's "dual cold-hot" idea in its only defensible form: verifier stays out (truth =
test-execution), hot is scoped to producer<->producer Q&A about the PLAN. The Fugu evidence says
open chat collapses via error-anchoring; this measures whether a NARROW hot channel (coder asks up
to 2 questions about the architect's plan, architect elaborates) improves the coder's pass rate —
and at what cost (hot = 2x the calls). Cero Claude cupo (external models). Neutral, edge-heavy tasks
(a vague plan misses edges -> the hot probe can surface them).

  cold arm: architect(plan) -> coder(code | task+plan)                 -> execute
  hot  arm: architect(plan) -> coder(<=2 Qs) -> architect(answers) -> coder(code | task+plan+Q&A) -> execute

Run: .venv/Scripts/python.exe -m scripts.ab_hot_clarification
"""
from __future__ import annotations

import json

from mmorch.providers import call
from mmorch.textutil import extract_fence
from mmorch.checkers import check
from mmorch.config import DEFAULT_GENERATOR, family_of

ARCHITECT = DEFAULT_GENERATOR          # deepseek-chat
CODER = "gemini-2.5-flash"             # cross-family vs the architect (like the real workflow)
assert family_of(ARCHITECT) != family_of(CODER), "keep architect/coder cross-family"

# Moderate tasks with edge cases a terse plan can miss; tests pin the intended behavior.
TASKS = [
    ("def merge_intervals(intervals): fusioná intervalos [inicio,fin] que se solapan o TOCAN "
     "(ej [1,3] y [3,5] -> [1,5]). Devolvé la lista ordenada por inicio.",
     "assert merge_intervals([[1,3],[2,6],[8,10]])==[[1,6],[8,10]]\n"
     "assert merge_intervals([[1,3],[3,5]])==[[1,5]]\n"
     "assert merge_intervals([])==[]\nassert merge_intervals([[5,6],[1,2]])==[[1,2],[5,6]]"),
    ("def rpn_eval(tokens): evaluá notación polaca inversa (lista de tokens str). Operadores + - * / "
     "(división ENTERA truncada hacia cero). Devolvé el entero resultado.",
     "assert rpn_eval(['2','1','+','3','*'])==9\nassert rpn_eval(['4','13','5','/','+'])==6\n"
     "assert rpn_eval(['10','6','-'])==4\nassert rpn_eval(['-7','2','/'])==-3"),
    ("def roman_to_int(s): convertí un número romano (I,V,X,L,C,D,M) a entero, manejando la resta "
     "(IV=4, IX=9, XL=40, etc).",
     "assert roman_to_int('III')==3\nassert roman_to_int('IV')==4\nassert roman_to_int('IX')==9\n"
     "assert roman_to_int('LVIII')==58\nassert roman_to_int('MCMXCIV')==1994"),
    ("def word_freq(s): devolvé un dict palabra->conteo. Palabras separadas por cualquier whitespace, "
     "case-insensitive, ignorando signos de puntuación al inicio/fin de cada palabra.",
     "assert word_freq('The cat, the CAT!')=={'the':2,'cat':2}\n"
     "assert word_freq('')=={}\nassert word_freq('a a  a')=={'a':3}"),
    ("def spiral_order(matrix): devolvé los elementos de una matriz (lista de listas) en orden espiral "
     "horario, arrancando arriba-izquierda. La matriz puede no ser cuadrada.",
     "assert spiral_order([[1,2,3],[4,5,6],[7,8,9]])==[1,2,3,6,9,8,7,4,5]\n"
     "assert spiral_order([[1,2],[3,4]])==[1,2,4,3]\nassert spiral_order([])==[]\n"
     "assert spiral_order([[1,2,3,4]])==[1,2,3,4]"),
    ("def compress(s): run-length encode SOLO si acorta; si el resultado no es más corto que el "
     "original, devolvé el original. Ej 'aabcccccaaa'->'a2b1c5a3'; 'abc'->'abc'.",
     "assert compress('aabcccccaaa')=='a2b1c5a3'\nassert compress('abc')=='abc'\n"
     "assert compress('')==''\nassert compress('aa')=='aa'"),
]


def _gen(model: str, prompt: str, system: str = "") -> str:
    msgs = ([{"role": "system", "content": system}] if system else []) + [{"role": "user", "content": prompt}]
    return call(model, msgs, pattern="ab_hot", node=model, temperature=0.0).text


def _plan(task: str) -> str:
    return _gen(ARCHITECT, f"TAREA:\n{task}\n\nEscribí un plan de implementación BREVE (pasos), sin código.",
                system="You are the Architect. Terse implementation plan, no code.")


def _code_cold(task: str, plan: str) -> str:
    out = _gen(CODER, f"TAREA:\n{task}\n\nPLAN:\n{plan}\n\nImplementá la función.",
               system="You are the Coder. Output ONLY the function in a ```python``` block.")
    return extract_fence(out)


def _code_hot(task: str, plan: str) -> str:
    qs = _gen(CODER, f"TAREA:\n{task}\n\nPLAN del architect:\n{plan}\n\nAntes de codear, hacé HASTA 2 "
              "preguntas ESPECÍFICAS sobre ambigüedades o edge-cases del plan. Solo las preguntas.",
              system="You are the Coder. Ask up to 2 sharp clarifying questions. No code.")
    ans = _gen(ARCHITECT, f"TAREA:\n{task}\n\nTu plan:\n{plan}\n\nEl coder pregunta:\n{qs}\n\nRespondé "
               "conciso y concreto (edge-cases incluidos).",
               system="You are the Architect. Answer the coder's questions precisely.")
    out = _gen(CODER, f"TAREA:\n{task}\n\nPLAN:\n{plan}\n\nAclaraciones:\nP: {qs}\nR: {ans}\n\nImplementá la función.",
               system="You are the Coder. Output ONLY the function in a ```python``` block.")
    return extract_fence(out)


def _passes(code: str, tests: str) -> bool:
    try:
        return bool(check("python_exec", code=code + "\n" + tests, timeout=10).passed)
    except Exception:
        return False


def run() -> dict:
    rows = []
    for task, tests in TASKS:
        plan = _plan(task)
        pc = _passes(_code_cold(task, plan), tests)
        ph = _passes(_code_hot(task, plan), tests)
        rows.append({"cold_pass": pc, "hot_pass": ph})
        print(f"  cold={'P' if pc else 'F'}  hot={'P' if ph else 'F'} | {task[:52]}")
    n = len(rows)
    cold = sum(r["cold_pass"] for r in rows)
    hot = sum(r["hot_pass"] for r in rows)
    hot_wins = sum(1 for r in rows if r["hot_pass"] and not r["cold_pass"])
    cold_wins = sum(1 for r in rows if r["cold_pass"] and not r["hot_pass"])
    return {"n": n, "cold_passrate": round(cold / n, 3), "hot_passrate": round(hot / n, 3),
            "hot_wins": hot_wins, "cold_wins": cold_wins,
            "cost_note": "hot = 2x the model calls (architect+coder -> +coder-Qs +architect-answers)"}


if __name__ == "__main__":
    print("A/B: hot architect<->coder clarification vs cold hand-off (execution-truth, cero cupo)\n")
    rep = run()
    print("\n=== RESULT ===")
    print(json.dumps(rep, indent=2))
