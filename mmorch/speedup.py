"""speedup — make a function faster, cero-cupo, kept only on MEASURED+CORRECT improvement.

The mmorch-side twin of the /speedup skill: a `hillclimb` whose generator (DeepSeek, cheap)
proposes a faster variant and whose score is a RUNNABLE rubric, never an LLM judge —
correctness-gated runtime. A candidate is run in a fresh subprocess on a fixed benchmark;
if its result diverges from the original it scores `inf` (rejected: a faster-but-wrong answer
is a regression), else its median runtime. hillclimb keeps the fastest correct candidate.

Numba/NumPy/algorithmic rewrites are whatever the generator proposes — execution decides, not us.
Library-only (propose/score are callables). `gen` is injectable so a test can drive it cero-cost.
"""
from __future__ import annotations

import json

from .hillclimb import hillclimb
from .textutil import extract_fence as _extract  # dedup of the local fence helper


def _close(a, b, tol: float = 1e-6) -> bool:
    """Correctness compare tolerant to float reordering (NumPy/Numba) but strict on shape/values."""
    if isinstance(a, bool) or isinstance(b, bool):
        return a == b
    if isinstance(a, (int, float)) and isinstance(b, (int, float)):
        return abs(a - b) <= tol * (1 + abs(b))
    if isinstance(a, list) and isinstance(b, list):
        return len(a) == len(b) and all(_close(x, y, tol) for x, y in zip(a, b, strict=False))
    if isinstance(a, dict) and isinstance(b, dict):
        return a.keys() == b.keys() and all(_close(a[k], b[k], tol) for k in a)
    return a == b


def _harness(source: str, setup: str, call: str, runs: int) -> str:
    return (
        f"{source}\n"
        "import time, json\n"
        f"{setup}\n"
        f"_res = {call}\n"
        "_t = []\n"
        f"for _ in range({runs}):\n"
        f"    _s = time.perf_counter(); {call}; _t.append(time.perf_counter() - _s)\n"
        "_t.sort()\n"
        "try:\n"
        "    _r = json.dumps(_res)\n"
        "except Exception:\n"
        "    _r = json.dumps(repr(_res))\n"
        'print(json.dumps({"r": json.loads(_r), "sec": _t[len(_t)//2]}))\n'
    )


def _measure(source: str, setup: str, call: str, runs: int, timeout: float):
    """Run the candidate in the SHARED sandbox (env-scrubbed ephemeral process — see sandbox.py)
    and return (result, median_seconds). One execution path for all LLM-generated code: the env
    scrub (no API keys / MMORCH_SERVER_TOKEN reachable), temp cwd, timeout+kill, and Windows
    no-console handling all live in run_sandboxed. Raises on nonzero exit OR timeout, so a bad
    candidate counts as a failed hillclimb round."""
    from .sandbox import run_sandboxed
    r = run_sandboxed(_harness(source, setup, call, runs), timeout=timeout)
    if r.returncode != 0 or r.timed_out:
        raise RuntimeError((r.stderr or "nonzero")[-200:])
    line = r.stdout.strip().splitlines()[-1]
    d = json.loads(line)
    return d["r"], float(d["sec"])


def _default_gen(current: str) -> str:
    from .providers import call as _call
    from .config import DEFAULT_GENERATOR
    out = _call(DEFAULT_GENERATOR, [{"role": "user", "content":
        "Reescribí esta función para que sea MÁS RÁPIDA con el MISMO comportamiento EXACTO "
        "(mismos resultados). Vectorizá / mejorá el algoritmo si podés; mantené la misma firma. "
        "Devolvé SOLO la función en un bloque ```python```.\n\n" + current}],
        pattern="speedup", node="speedup").text
    return _extract(out)


def speedup(source: str, *, setup: str, call: str, gen=None, runs: int = 5,
            rounds: int = 8, patience: int = 3, timeout: float = 30.0,
            min_speedup: float = 1.05) -> dict:
    """Optimize `source` (a function def). `setup` builds the benchmark inputs, `call` invokes the
    function (e.g. setup='data=list(range(100000))', call='f(data)'). Returns {best, baseline_sec,
    best_sec, speedup, rounds, stopped, correct}. The original is the correctness oracle; only
    candidates that reproduce its result AND beat it by >= min_speedup are kept."""
    gen = gen or _default_gen
    oracle, _ = _measure(source, setup, call, runs, timeout)   # the correctness ground truth

    def score(code: str) -> float:
        res, sec = _measure(code, setup, call, runs, timeout)  # raises -> hillclimb counts as failed round
        if not _close(res, oracle):
            return float("inf")                                # fast-but-wrong = rejected
        return sec

    def propose(ctx) -> str | None:
        cand = gen(ctx.best)
        return cand if (cand and cand.strip() and cand.strip() != ctx.best.strip()) else None

    # minimize seconds; keep a candidate only if it shaves a real margin (min_speedup) off best.
    res = hillclimb(propose, score, initial=source, maximize=False,
                    max_rounds=rounds, patience=patience, pattern="speedup",
                    min_delta=0.0)
    base = res.baseline or 0.0
    best_sec = res.best_score if res.best_score not in (None, float("inf")) else base
    sp = (base / best_sec) if best_sec else 1.0
    # guard: keep the speedup only if it clears the margin; else fall back to the original
    if sp < min_speedup:
        return {"best": source, "baseline_sec": base, "best_sec": base, "speedup": 1.0,
                "rounds": res.rounds, "stopped": res.stopped, "correct": True, "kept": False}
    return {"best": res.best, "baseline_sec": base, "best_sec": best_sec,
            "speedup": round(sp, 2), "rounds": res.rounds, "stopped": res.stopped,
            "correct": True, "kept": True}


if __name__ == "__main__":
    SLOW = "def f(n):\n    t = 0\n    for i in range(n):\n        t += i * i\n    return t\n"
    FAST = "def f(n):\n    return (n - 1) * n * (2 * n - 1) // 6\n"     # same result, O(1)
    WRONG = "def f(n):\n    return 0\n"                                 # fast but WRONG

    setup, call = "N = 200000", "f(N)"
    # sanity: oracle agrees, FAST is correct, WRONG is not
    o, _ = _measure(SLOW, setup, call, 3, 30)
    of, _ = _measure(FAST, setup, call, 3, 30)
    ow, _ = _measure(WRONG, setup, call, 3, 30)
    assert _close(o, of) and not _close(o, ow), "oracle/correctness compare"

    calls = {"n": 0}
    def fake_gen(_cur):                       # round 1 -> wrong (must be rejected); round 2 -> correct+fast
        calls["n"] += 1
        return WRONG if calls["n"] == 1 else FAST

    r = speedup(SLOW, setup=setup, call=call, gen=fake_gen, runs=3, rounds=4, patience=3)
    assert r["kept"] and r["speedup"] > 1.0, r
    assert _close(_measure(r["best"], setup, call, 1, 30)[0], o), "kept candidate is CORRECT"
    # the wrong-but-fast candidate was NOT kept
    assert _measure(r["best"], setup, call, 1, 30)[0] == o, "best reproduces the oracle exactly"
    print(f"speedup OK -> {r['speedup']}x ({round(r['baseline_sec']*1000,2)}ms -> "
          f"{round(r['best_sec']*1000,3)}ms), kept={r['kept']}, wrong-rejected")
