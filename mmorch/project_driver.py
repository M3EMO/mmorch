"""project_driver — F2 of the /project rebuild: the RECURSIVE build orchestrator.

Consumes F1's primitives (project_build: decompose, build_order, stub_check). The driver is the
tree orchestration; F1 is the per-level primitives — separate modules, they change for different
reasons (orchestration policy vs validation rules).

The loop (converged, cross-family-refined 3x):
  decompose(task) -> build_order -> per unit:
    build_fn generates code; stub_check REJECTS stubs deterministically; if stub -> the unit is too
    big/underspecified -> RECURSE (decompose the unit, build its sub-worklist depth-first). Else the
    gate_fn runs the TIERED execution gate (mapped test + cumulative); pass -> commit that unit.
  Recursion depth caps (max_depth). A unit that can't be built green after its budget -> escalate.

Everything external is INJECTED (build_fn=coder, gate_fn=execution-truth, plan_fn=decompose,
checkpoint_fn, commit_fn) so the ORCHESTRATION logic is self-checked cero-cost. Real wiring
(code_loop as build_fn, checkers as gate_fn, worktree commits) lands in F3.
"""
from __future__ import annotations

from typing import Callable

from .project_build import build_order, stub_check, validate_worklist


def build_unit(unit: dict, *, build_fn: Callable[[dict], str],
               gate_fn: Callable[[dict, str], tuple[bool, str]], max_fix: int = 3) -> dict:
    """Build ONE unit. build_fn(unit)->code; stub_check gates structure; gate_fn(unit,code)->(ok,detail)
    is the execution-truth gate. Returns {name, status, ...}:
      'built'    -> code passes the gate.
      'recurse'  -> the coder can't produce non-stub code (unit too big) -> caller decomposes it.
      'escalate' -> non-stub but the gate never passes within max_fix.
    """
    detail = ""
    for _ in range(max_fix):
        # wrap ONLY the untrusted boundary (build_fn=LLM, gate_fn=checker/sandbox): a throw there is a
        # failed attempt, not a crash. stub_check is OUR deterministic code -> left un-wrapped so a bug
        # in it surfaces instead of being masked as a failed attempt.
        try:
            code = build_fn(unit)
        except Exception as e:
            detail = f"build_fn {type(e).__name__}: {str(e)[:100]}"
            continue
        is_stub, reason = stub_check(code)
        if is_stub:
            return {"name": unit["name"], "status": "recurse", "reason": reason}
        try:
            ok, detail = gate_fn(unit, code)
        except Exception as e:
            detail = f"gate_fn {type(e).__name__}: {str(e)[:100]}"
            continue
        if ok:
            return {"name": unit["name"], "status": "built", "code": code}
    return {"name": unit["name"], "status": "escalate", "detail": detail}


def run_project_build(task: str, *, external_test: str | None,
                      plan_fn: Callable[[str, str | None], list[dict]],
                      build_fn: Callable[[dict], str],
                      gate_fn: Callable[[dict, str], tuple[bool, str]],
                      commit_fn: Callable[[str, dict], None] | None = None,
                      integrate_fn: Callable[[str, list[dict]], tuple[bool, str]] | None = None,
                      depth: int = 0, max_depth: int = 2) -> dict:
    """Recursive orchestrator. plan_fn(task, external_test)->units. Builds units in dependency order;
    a 'recurse' unit is decomposed and built depth-first, its own test_cmd (if any) becoming the
    sub-level's backstop. commit_fn(name, result) checkpoints/commits a built LEAF.

    INTEGRATION GATE (the whole > the parts): green units do NOT prove a green whole (shared state,
    interface mismatch). So after every unit at a level builds, if that level has an `external_test`
    AND `integrate_fn`, the assembled result is run against it. Red -> status 'integration_failed'
    (NOT a silent 'built') carrying the failing output, which the orchestrator surfaces. There is NO
    auto interface-re-plan (that risks an unbounded decompose<->integrate loop) — a failing
    integration escalates to a human/Opus decision. depth cap default is SHALLOW (2): a stub still
    present at the cap ESCALATES; a stub is never accepted as built.

    Terminals: 'built' (all units built + integration green if checked), 'integration_failed',
    'escalate' (bad plan / depth cap / a unit's gate never passed). All structured for the caller."""
    if depth > max_depth:
        return {"status": "escalate", "reason": f"max recursion depth {max_depth}", "task": task[:80]}
    units = plan_fn(task, external_test)
    ok, errs = validate_worklist(units)   # the DRIVER enforces a valid plan — not just the default planner
    if not ok:
        return {"status": "escalate", "reason": f"invalid plan: {errs}", "task": task[:80]}
    by_name = {u["name"]: u for u in units}
    results: list = []
    for name in build_order(units):
        unit = by_name[name]
        r = build_unit(unit, build_fn=build_fn, gate_fn=gate_fn)
        if r["status"] == "recurse":
            sub = run_project_build(unit["spec"], external_test=unit.get("test_cmd") or external_test,
                                    plan_fn=plan_fn, build_fn=build_fn, gate_fn=gate_fn,
                                    commit_fn=commit_fn, integrate_fn=integrate_fn,
                                    depth=depth + 1, max_depth=max_depth)
            r = {"name": name, "status": sub["status"], "recursed": True, "sub": sub}
        # commit only LEAF units (built with their own code); a recursed container has no code of
        # its own — its sub-units were already committed inside the recursive call.
        if commit_fn and r["status"] == "built" and not r.get("recursed"):
            commit_fn(name, r)
        results.append(r)
        if r["status"] in ("escalate", "integration_failed"):
            return {"status": r["status"], "at": name, "depth": depth, "results": results}
    # integration gate: the assembled whole must pass this level's acceptance test (execution truth).
    if external_test and integrate_fn:
        iok, idetail = integrate_fn(external_test, results)
        if not iok:
            return {"status": "integration_failed", "depth": depth, "external_test": external_test,
                    "detail": idetail, "results": results}
        return {"status": "built", "depth": depth, "integrated": True, "results": results}
    return {"status": "built", "depth": depth, "results": results}


if __name__ == "__main__":
    # Cero-cost self-check of the ORCHESTRATION: fake plan/build/gate simulate the interesting paths.
    # A worklist where 'big' is a stub on the first plan, then decomposes into buildable sub-units.
    PLANS = {
        "top": [{"name": "a", "spec": "unit a", "deps": []},
                {"name": "big", "spec": "unit big", "deps": ["a"]}],
        "unit big": [{"name": "b1", "spec": "sub b1", "deps": []},
                     {"name": "b2", "spec": "sub b2", "deps": ["b1"]}],
    }

    def plan_fn(task, ext):
        return PLANS["top"] if task == "top" else PLANS["unit big"]

    def build_fn(unit):                      # 'big' at top level = a stub -> forces recursion; rest real
        if unit["name"] == "big":
            return "from .x import y\n__all__=['y']"      # stub (no defs)
        return f"def {unit['name']}():\n    return 1"     # real

    def gate_fn(unit, code):
        return (True, "ok")                  # execution gate passes for the real units

    committed: list = []
    res = run_project_build("top", external_test="pytest -q", plan_fn=plan_fn,
                            build_fn=build_fn, gate_fn=gate_fn, commit_fn=lambda n, r: committed.append(n))
    assert res["status"] == "built", res
    # order: a, then big (which recursed into b1,b2)
    names = [r["name"] for r in res["results"]]
    assert names == ["a", "big"], names
    big = next(r for r in res["results"] if r["name"] == "big")
    assert big["recursed"] and big["sub"]["status"] == "built", big
    assert [r["name"] for r in big["sub"]["results"]] == ["b1", "b2"]
    assert committed == ["a", "b1", "b2"], committed   # 'big' itself isn't committed; its sub-units are

    # escalate: gate never passes -> unit escalates -> propagates up
    res2 = run_project_build("top", external_test=None, plan_fn=lambda t, e: PLANS["top"],
                             build_fn=lambda u: "def f():\n    return 1",
                             gate_fn=lambda u, c: (False, "test red"))
    assert res2["status"] == "escalate" and res2["at"] == "a", res2

    # max_depth: infinite-stub -> capped, not infinite recursion
    res3 = run_project_build("x", external_test=None, plan_fn=lambda t, e: [{"name": "u", "spec": "u", "deps": []}],
                             build_fn=lambda u: "import z", gate_fn=lambda u, c: (True, ""), max_depth=2)
    assert res3["status"] == "escalate", res3

    # invalid plan from an injected plan_fn -> the DRIVER escalates, not crashes [round-1 critique 2]
    res4 = run_project_build("x", external_test=None, plan_fn=lambda t, e: [{"nope": 1}],
                             build_fn=lambda u: "def f():\n    return 1", gate_fn=lambda u, c: (True, ""))
    assert res4["status"] == "escalate" and "invalid plan" in res4["reason"], res4

    # a build_fn/gate_fn that THROWS -> escalate gracefully, never crash the orchestration [round-3 critique 2]
    def _boom(u, c):
        raise RuntimeError("boom")
    r_throw = build_unit({"name": "u", "spec": "u"}, build_fn=lambda u: "def f():\n    return 1", gate_fn=_boom)
    assert r_throw["status"] == "escalate" and "RuntimeError" in r_throw["detail"], r_throw

    # INTEGRATION GATE (cross-family round-1 valid critique 'e'): all units build green, but the
    # assembled whole fails the level's acceptance test -> integration_failed, NOT a silent 'built'.
    seen_integration: list = []

    def _fail_integration(ext, rs):
        seen_integration.append(ext)
        return False, "3 failed"

    res5 = run_project_build("top", external_test="pytest -q", plan_fn=lambda t, e: PLANS["top"],
                             build_fn=lambda u: f"def {u['name']}():\n    return 1",
                             gate_fn=lambda u, c: (True, "unit green"), integrate_fn=_fail_integration)
    assert res5["status"] == "integration_failed", res5
    assert res5["detail"] == "3 failed" and seen_integration == ["pytest -q"], res5
    # integration GREEN -> built + integrated flag
    res6 = run_project_build("top", external_test="pytest -q", plan_fn=lambda t, e: PLANS["top"],
                             build_fn=lambda u: f"def {u['name']}():\n    return 1",
                             gate_fn=lambda u, c: (True, "unit green"),
                             integrate_fn=lambda ext, rs: (True, "all green"))
    assert res6["status"] == "built" and res6.get("integrated"), res6
    # no external_test -> no integration gate (nothing to assert against) -> plain built
    res7 = run_project_build("top", external_test=None, plan_fn=lambda t, e: PLANS["top"],
                             build_fn=lambda u: f"def {u['name']}():\n    return 1",
                             gate_fn=lambda u, c: (True, ""),
                             integrate_fn=lambda ext, rs: (False, "must not run"))
    assert res7["status"] == "built" and not res7.get("integrated"), res7

    print("project_driver F2 OK — recursion-on-stub, dep order, cumulative-commit, escalate propagation, "
          "depth cap, integration gate (whole>parts)")
