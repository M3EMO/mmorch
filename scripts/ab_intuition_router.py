"""A/B the intuition router vs the fixed default — execution-truth, cero Claude cupo.

The router was flipped ON by default (decide() picks a model per structural signature when
familiar). This validates the flip: on NEW code tasks, does the intuition-picked model actually
solve more than the default (deepseek-chat)? Truth = test-pass (checkers, real execution), NOT an
LLM judge. Neutral task set (standard algorithms, not chosen to favor any model). Small N — a
signal, not a proof; reports honestly.

Run: .venv/Scripts/python.exe -m scripts.ab_intuition_router
"""
from __future__ import annotations

import json

from mmorch.providers import call
from mmorch.textutil import extract_fence
from mmorch.checkers import check
from mmorch.intuition import decide, record
from mmorch.config import DEFAULT_GENERATOR, DEFAULT_INTUITION_POOL

# (prompt, tests) — self-contained; tests are asserts run against the generated function.
TASKS = [
    ("Resolvé en Python: def two_sum(nums, target): devolvé una tupla (i, j) de índices i<j tal que "
     "nums[i]+nums[j]==target, o None si no existe.",
     "assert two_sum([2,7,11,15],9)==(0,1)\nassert two_sum([3,2,4],6)==(1,2)\nassert two_sum([1,2],10) is None"),
    ("Resolvé en Python: def gcd(a, b): máximo común divisor de dos enteros positivos.",
     "assert gcd(12,18)==6\nassert gcd(17,5)==1\nassert gcd(100,10)==10"),
    ("Resolvé en Python: def binary_search(a, x): a es lista ordenada; devolvé el índice de x o -1.",
     "assert binary_search([1,3,5,7,9],7)==3\nassert binary_search([1,3,5],4)==-1\nassert binary_search([],1)==-1"),
    ("Resolvé en Python: def is_balanced(s): True si los paréntesis ()[]{} en s están balanceados.",
     "assert is_balanced('([]{})')==True\nassert is_balanced('([)]')==False\nassert is_balanced('')==True"),
    ("Resolvé en Python: def lis_length(a): longitud de la subsecuencia estrictamente creciente más larga.",
     "assert lis_length([10,9,2,5,3,7,101,18])==4\nassert lis_length([0,1,0,3,2,3])==4\nassert lis_length([])==0"),
    ("Resolvé en Python: def flatten(x): aplaná una lista arbitrariamente anidada de enteros a una lista plana.",
     "assert flatten([1,[2,[3,4]],5])==[1,2,3,4,5]\nassert flatten([])==[]\nassert flatten([[[1]]])==[1]"),
    ("Resolvé en Python: def merge_sorted(a, b): fusioná dos listas ordenadas en una lista ordenada.",
     "assert merge_sorted([1,3,5],[2,4,6])==[1,2,3,4,5,6]\nassert merge_sorted([],[1])==[1]\nassert merge_sorted([1,1],[1])==[1,1,1]"),
    ("Resolvé en Python: def longest_common_prefix(strs): prefijo común más largo de una lista de strings ('' si ninguno).",
     "assert longest_common_prefix(['flower','flow','flight'])=='fl'\nassert longest_common_prefix(['a','b'])==''\nassert longest_common_prefix(['abc'])=='abc'"),
    ("Resolvé en Python: def rle(s): run-length encoding de un string, ej 'aaabb'->'a3b2'.",
     "assert rle('aaabb')=='a3b2'\nassert rle('abc')=='a1b1c1'\nassert rle('')==''"),
    ("Resolvé en Python: def rotate(a, k): rotá la lista a k posiciones a la derecha (k puede ser > len).",
     "assert rotate([1,2,3,4,5],2)==[4,5,1,2,3]\nassert rotate([1,2,3],4)==[3,1,2]\nassert rotate([],3)==[]"),
]


def _gen_and_check(model: str, prompt: str, tests: str) -> bool:
    sys = "You are a Python programmer. Output ONLY the function source in a python code block."
    out = call(model, [{"role": "system", "content": sys}, {"role": "user", "content": prompt}],
               pattern="ab_router", node=model, temperature=0.0).text
    code = extract_fence(out)
    try:
        return bool(check("python_exec", code=code + "\n" + tests, timeout=10).passed)
    except Exception:
        return False


def run(train: bool = True) -> dict:
    """A/B the intuition-picked model vs the default. train=True ALSO records both arms' real
    (execution-truth) outcomes into the sig-bandit — the counterfactual data it normally never sees
    (production only records the model it picked). So each run measures AND sharpens the router."""
    default = DEFAULT_GENERATOR
    pool = DEFAULT_INTUITION_POOL
    rows, diverged = [], []
    for prompt, tests in TASKS:
        act, picked, _reason = decide(pool, prompt)
        arm_model = picked if (act == "commit" and picked) else default
        pass_intuition = _gen_and_check(arm_model, prompt, tests)
        pass_default = _gen_and_check(default, prompt, tests)
        if train:                                    # counterfactual seed: feed BOTH arms' truth
            record(arm_model, 1.0 if pass_intuition else 0.0, prompt)
            if default != arm_model:
                record(default, 1.0 if pass_default else 0.0, prompt)
        row = {"picked": arm_model, "same_as_default": arm_model == default,
               "intuition_pass": pass_intuition, "default_pass": pass_default}
        rows.append(row)
        if arm_model != default:
            diverged.append(row)
        print(f"  {arm_model:16} vs {default:13} | intuition={'P' if pass_intuition else 'F'} "
              f"default={'P' if pass_default else 'F'} | {prompt[:50]}")

    n = len(rows)
    intu_pass = sum(r["intuition_pass"] for r in rows)
    def_pass = sum(r["default_pass"] for r in rows)
    # McNemar-style on diverged tasks: where they disagree, who won
    win_i = sum(1 for r in diverged if r["intuition_pass"] and not r["default_pass"])
    win_d = sum(1 for r in diverged if r["default_pass"] and not r["intuition_pass"])
    return {"n": n, "n_diverged": len(diverged),
            "intuition_passrate": round(intu_pass / n, 3), "default_passrate": round(def_pass / n, 3),
            "on_diverged_intuition_wins": win_i, "on_diverged_default_wins": win_d,
            "rows": rows}


if __name__ == "__main__":
    print("A/B: intuition router vs default (execution-truth, cero cupo)\n")
    rep = run()
    print("\n=== RESULT ===")
    print(json.dumps({k: v for k, v in rep.items() if k != "rows"}, indent=2))
