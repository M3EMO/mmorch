"""project_integrate — F3 of the /project rebuild: wire the recursive driver (F2) to REAL seams.

F1 = deterministic primitives (decompose/validate/stub_check). F2 = the recursive orchestrator
(build order, recurse-on-stub, integration gate, escalate). F3 (here) binds F2's injected seams to
production behaviour, cero Claude cupo (cheap external models do the roles):

  plan_fn   = decompose (F1) — the planner LLM proposes a worklist; deterministic validation gates it.
  build_fn  = the HOT coder loop: generate a unit's file -> run its test_cmd -> fix on the failure ->
              repeat until green or budget. Execution feedback flows here (the coder sees its reds).
  gate_fn   = the COLD verifier: an INDEPENDENT re-run of the FINAL code (never sees the coder's
              reasoning -> no error-anchoring). A unit WITH a test_cmd is re-run clean = execution
              truth. A unit WITHOUT one is NOT called correct — it is 'unverified', its correctness
              DEFERRED to the integration gate; a cold cross-family probe only yields ADVISORY
              feedback (you cannot manufacture ground truth for an untested unit — honest ceiling).
  integrate_fn = run the level's external acceptance test on the ASSEMBLED whole (green units do not
              prove a green whole). Red -> F2 returns 'integration_failed' (surfaced, never silent).
  commit_fn = commit each built+verified LEAF to a git worktree branch (per-unit -> git-bisect).

The single entry is build_project(). Every model/exec/commit boundary is injectable so the wiring
logic is self-checked with NO API. The cold verifier MUST be cross-family vs the coder (subjective
probe -> a model endorses its own blind spots).
"""
from __future__ import annotations

import ast
import os
import subprocess
from typing import Callable

from .config import DEFAULT_GENERATOR, DEFAULT_VERIFIER, family_of
from .project_build import decompose
from .project_driver import run_project_build


def _file_of(unit: dict) -> str:
    """Where a unit's code lives. Planner may set 'file'; else derive from the (validated) name."""
    return str(unit.get("file") or f"{unit['name']}.py")


def _safe_target(repo: str, unit: dict) -> str:
    """Resolve a unit's file path INSIDE `repo`, rejecting traversal (the name/file is LLM-proposed —
    a hallucinated '../../x' must not let us write outside the repo). Trust boundary at the disk edge."""
    root = os.path.realpath(repo)
    fpath = os.path.realpath(os.path.join(root, _file_of(unit)))
    try:
        contained = os.path.commonpath([fpath, root]) == root
    except ValueError:          # different drives (Windows) -> definitionally outside the repo
        contained = False
    if not contained:
        raise ValueError(f"unit path escapes repo: {_file_of(unit)!r}")
    return fpath


def _ast_ok(code: str) -> tuple[bool, str]:
    """Deterministic floor for an untested unit: it must at least be valid Python."""
    try:
        ast.parse(code)
        return True, ""
    except SyntaxError as e:
        return False, str(e)[:120]


# --- production boundaries (injectable; these are the only things that touch API / disk) --------- #
def _default_gen(gen_model: str, repo: str):
    from .providers import call
    from .textutil import extract_fence
    from .prompts import LAZY_SYSTEM

    def gen(unit: dict, feedback: str) -> str:
        fpath = _safe_target(repo, unit)
        cur = ""
        if os.path.isfile(fpath):
            try:
                # 60k chars ~ 15k tokens: the coder REGENERATES the whole file, so a truncated view of a
                # big file (e.g. a 25KB module it must minimally edit) would silently DROP the tail. 60k
                # covers any sane single file; beyond that the unit is mis-scoped and should decompose.
                cur = open(fpath, encoding="utf-8").read()[:60000]
            except (OSError, UnicodeDecodeError):
                cur = ""   # side-channel: current-content is optional context; a bad read must not stop the coder
        user = (f"UNIT: {unit['name']}\nSPEC:\n{unit['spec']}\n\n"
                f"FILE `{_file_of(unit)}` (current):\n```\n{cur}\n```\n"
                + (f"\nThe previous attempt FAILED:\n{feedback[:1200]}\nFix it.\n" if feedback else "")
                + "Return ONLY the COMPLETE new file content in a ``` block.")
        out = call(gen_model, [{"role": "system", "content": LAZY_SYSTEM},
                               {"role": "user", "content": user}],
                   pattern="project_integrate", node="coder", temperature=0.0).text
        return extract_fence(out)
    return gen


def _default_run_test(repo: str):
    def run_test(unit: dict, code: str, test_cmd: str, timeout: float = 180.0) -> tuple[bool, str]:
        # write the unit's file, then run its acceptance command in the repo (execution truth)
        fpath = _safe_target(repo, unit)   # containment: LLM-proposed name can't escape the repo
        os.makedirs(os.path.dirname(fpath) or repo, exist_ok=True)
        with open(fpath, "w", encoding="utf-8") as f:
            f.write(code + ("\n" if not code.endswith("\n") else ""))
        # shell=True is deliberate: test_cmd is a real acceptance command, often COMPOUND (`pytest &&
        # shadow_diff`) — shlex-splitting would break that. Trust boundary: decompose() constrains
        # test_cmd to an EXISTING/user command, and the server runs build_project in an ISOLATED git
        # worktree (exec_policy), the same containment project_loop relies on. Not a free-form eval.
        try:
            p = subprocess.run(test_cmd, cwd=repo, shell=True, capture_output=True, text=True,  # noqa: S602
                               encoding="utf-8", errors="replace", timeout=timeout)
            return p.returncode == 0, (p.stdout + p.stderr)[-1500:]
        except subprocess.TimeoutExpired:
            return False, "TIMEOUT"
        except Exception as e:
            return False, str(e)[:300]
    return run_test


def _default_propose_test(verifier_model: str):
    from .providers import call
    from .textutil import extract_fence

    def propose_test(code: str, spec: str) -> str:
        prompt = (f"SPEC of the unit:\n{spec}\n\nCODE:\n```\n{code[:4000]}\n```\n\n"
                  "You are a skeptical reviewer from a different background. Write ONE short pytest-style "
                  "block of `assert` statements that this code SHOULD satisfy per the spec but you suspect "
                  "it FAILS (an edge case). Import from the code as needed. Return ONLY the asserts in a "
                  "``` block, or an empty block if you cannot find a plausible failing case.")
        return extract_fence(call(verifier_model, prompt, pattern="project_integrate", node="probe").text)
    return propose_test


def _default_run_snippet():
    from .checkers import check

    def run_snippet(code: str, asserts: str) -> tuple[bool, str]:
        try:
            r = check("python_exec", code=code + "\n" + asserts, timeout=15)
            return bool(r.passed), r.detail
        except Exception as e:
            return False, f"checker error: {str(e)[:120]}"
    return run_snippet


def _default_integrate(repo: str):
    def integrate(external_test: str, results: list, timeout: float = 600.0) -> tuple[bool, str]:
        # external_test is the USER's acceptance suite (the real backstop), typically COMPOUND ->
        # shell=True is required; containment is the isolated worktree (server exec_policy). See run_test.
        try:
            p = subprocess.run(external_test, cwd=repo, shell=True, capture_output=True, text=True,  # noqa: S602
                               encoding="utf-8", errors="replace", timeout=timeout)
            return p.returncode == 0, (p.stdout + p.stderr)[-2000:]
        except subprocess.TimeoutExpired:
            return False, "TIMEOUT"
        except Exception as e:
            return False, str(e)[:300]
    return integrate


def _default_commit(repo: str):
    # Assumes `repo` is ALREADY the isolated working tree (the server opens a git worktree on a review
    # branch and passes its path). Commits in place on that branch -> per-unit history for git-bisect.
    from . import worktree_driver as wd
    if not wd.is_git_repo(repo):
        return None  # not a repo -> no per-unit commits; the build still runs (F2 guards commit_fn=None)

    def commit(name: str, result: dict) -> None:
        wt = wd.Worktree(repo, repo, "")   # commit in place on the current (caller-owned) branch
        wt.capture(f"mmorch(project-build): unit {name}")
    return commit


# --- the wiring: bind seams over shared state, then drive F2 ------------------------------------- #
def build_project(task: str, repo: str, *, external_test: str | None,
                  gen_model: str = DEFAULT_GENERATOR, verifier_model: str = DEFAULT_VERIFIER,
                  max_fix: int = 3, max_depth: int = 2,
                  plan: Callable[[str, str | None], list[dict]] | None = None,
                  gen: Callable[[dict, str], str] | None = None,
                  run_test: Callable[[dict, str, str], tuple[bool, str]] | None = None,
                  run_snippet: Callable[[str, str], tuple[bool, str]] | None = None,
                  propose_test: Callable[[str, str], str] | None = None,
                  integrate: Callable[[str, list], tuple[bool, str]] | None = None,
                  commit: Callable[[str, dict], None] | None = None) -> dict:
    """Build `task` in `repo` via the recursive engine, cero cupo. `external_test` = the real
    acceptance command (the integration gate at depth 0). All boundary fns default to production
    (providers.call / subprocess / checkers / worktree) and are injectable for the self-check.
    Returns F2's result tree plus {'unverified': [names deferred to the integration gate]}."""
    if family_of(gen_model) == family_of(verifier_model):
        raise ValueError(f"coder and cold verifier must be cross-family: {gen_model}/{verifier_model} "
                         f"are both {family_of(gen_model)}")
    gen = gen or _default_gen(gen_model, repo)
    run_test = run_test or _default_run_test(repo)
    run_snippet = run_snippet or _default_run_snippet()
    propose_test = propose_test or _default_propose_test(verifier_model)
    integrate = integrate or _default_integrate(repo)
    commit = commit if commit is not None else _default_commit(repo)

    cold_feedback: dict[str, str] = {}   # the cold verifier's counterexample -> next hot coder attempt
    unverified: list[str] = []           # units passed as 'unverified' (deferred to integration)

    plan_err: dict[str, str] = {}

    def _default_plan(t: str, ext: str | None) -> list[dict]:
        # Return [] so F2 escalates gracefully (never crash the build), but CAPTURE the real reason —
        # a decompose failure (API/parse) is otherwise lost behind F2's generic 'invalid plan: empty'.
        try:
            return decompose(t, external_test=ext, gen_model=gen_model)
        except Exception as e:
            plan_err["last"] = f"planner failed: {type(e).__name__}: {str(e)[:200]}"
            return []

    plan_fn = plan or _default_plan

    def build_fn(unit: dict) -> str:
        # HOT coder: sees its own reds + any cold-verifier counterexample stashed on a prior F2 pass.
        feedback = cold_feedback.pop(unit["name"], "")
        code = ""
        for _ in range(max_fix):
            code = gen(unit, feedback)
            tc = unit.get("test_cmd")
            if not tc:
                return code                       # untested -> single shot; the cold gate marks it unverified
            ok, out = run_test(unit, code, tc)
            if ok:
                return code
            feedback = out                        # execution failure -> feed the next attempt
        return code                               # best effort; stub_check / the gate decide the fate

    def gate_fn(unit: dict, code: str) -> tuple[bool, str]:
        # COLD verifier: independent of the coder's loop.
        tc = unit.get("test_cmd")
        if tc:
            ok, out = run_test(unit, code, tc)    # clean re-run = execution truth
            if not ok:
                cold_feedback[unit["name"]] = f"the clean re-run failed:\n{out[:300]}"
            return ok, (f"verified: {out[-160:]}" if ok else out[:250])
        # no test_cmd: NOT independently provable. Floor = valid python; then an ADVISORY cold probe.
        valid, why = _ast_ok(code)
        if not valid:
            return False, f"not valid python: {why}"
        probe = propose_test(code, unit["spec"])
        if probe.strip():
            passed, detail = run_snippet(code, probe)
            if not passed:                        # advisory: stash for the coder, but do NOT hard-fail
                cold_feedback[unit["name"]] = f"a reviewer's probe failed (advisory):\n{probe}\n{detail[:200]}"
        if unit["name"] not in unverified:
            unverified.append(unit["name"])
        return True, "unverified (no test_cmd; correctness deferred to the integration gate)"

    def integrate_fn(ext: str, results: list) -> tuple[bool, str]:
        return integrate(ext, results)

    res = run_project_build(task, external_test=external_test, plan_fn=plan_fn, build_fn=build_fn,
                            gate_fn=gate_fn, commit_fn=commit, integrate_fn=integrate_fn,
                            max_depth=max_depth)
    res["unverified"] = unverified
    if res.get("status") == "escalate" and plan_err.get("last"):
        res["plan_error"] = plan_err["last"]   # surface the swallowed planner failure, not just 'empty worklist'
    return res


if __name__ == "__main__":
    # cero-API self-check of the WIRING: every boundary faked, an injected `plan` avoids the planner.
    REPO = "/nonexistent-repo"   # never touched: commit/run/integrate are all faked here

    # 1. HOT coder loop: reds twice, greens on the 3rd attempt -> returns green, saw the reds as feedback.
    seen_fb: list = []

    def gen_hot(unit, fb):
        seen_fb.append(fb)
        return f"def {unit['name']}():\n    return {len(seen_fb)}"

    def run_reds_then_green(unit, code, tc):
        return (len(seen_fb) >= 3, "still red" if len(seen_fb) < 3 else "green")

    r1 = build_project("build one thing", REPO, external_test="ACCEPT",
                       plan=lambda t, e: [{"name": "u", "spec": "the unit", "deps": [], "test_cmd": "pt"}],
                       gen=gen_hot, run_test=run_reds_then_green, run_snippet=lambda c, a: (True, ""),
                       propose_test=lambda c, s: "", integrate=lambda e, rs: (True, "accept green"),
                       commit=lambda n, rr: None)
    assert r1["status"] == "built" and r1.get("integrated"), r1
    assert len(seen_fb) == 3 and seen_fb[0] == "" and "red" in seen_fb[1], seen_fb  # 1st clean, then reds

    # 2. INTEGRATION GATE: all units pass their own tests, but the assembled whole fails -> integration_failed.
    r2 = build_project("top", REPO, external_test="ACCEPT",
                       plan=lambda t, e: [{"name": "a", "spec": "a", "deps": [], "test_cmd": "pt"},
                                          {"name": "b", "spec": "b", "deps": ["a"], "test_cmd": "pt"}],
                       gen=lambda u, fb: f"def {u['name']}():\n    return 1",
                       run_test=lambda u, c, tc: (True, "unit green"),
                       run_snippet=lambda c, a: (True, ""), propose_test=lambda c, s: "",
                       integrate=lambda e, rs: (False, "3 failed: interface mismatch"),
                       commit=lambda n, rr: None)
    assert r2["status"] == "integration_failed" and "mismatch" in r2["detail"], r2

    # 3. UNTESTED unit: no test_cmd -> gate passes it as 'unverified' (never called correct), a failing
    #    cold probe is ADVISORY only (does not fail the gate); correctness is deferred to integration.
    committed: list = []
    r3 = build_project("top", REPO, external_test="ACCEPT",
                       plan=lambda t, e: [{"name": "u", "spec": "s", "deps": []}],  # no test_cmd
                       gen=lambda u, fb: "def u():\n    return 1",
                       run_test=lambda u, c, tc: (True, ""),          # not consulted (no test_cmd)
                       run_snippet=lambda c, a: (False, "probe failed"),   # probe FAILS -> advisory
                       propose_test=lambda c, s: "assert u() == 2",
                       integrate=lambda e, rs: (True, "accept green"),
                       commit=lambda n, rr: committed.append(n))
    assert r3["status"] == "built" and r3["unverified"] == ["u"], r3   # passed, but honestly unverified
    assert committed == ["u"], committed                              # a leaf still commits

    # 4. UNTESTED unit that isn't valid python -> the deterministic floor FAILS the gate -> escalate.
    r4 = build_project("top", REPO, external_test=None,
                       plan=lambda t, e: [{"name": "u", "spec": "s", "deps": []}],
                       gen=lambda u, fb: "def u(:\n  broken",   # syntax error -> stub_check catches first
                       run_test=lambda u, c, tc: (True, ""), run_snippet=lambda c, a: (True, ""),
                       propose_test=lambda c, s: "", integrate=lambda e, rs: (True, ""),
                       commit=lambda n, rr: None)
    assert r4["status"] in ("escalate", "integration_failed") or r4["status"] == "built", r4
    # a syntax-error body is a stub (F1) -> recurse; depth cap -> escalate (never accepted)
    assert r4["status"] == "escalate", r4

    # 5. cross-family guard: coder and cold verifier must differ in family.
    try:
        build_project("t", REPO, external_test=None, gen_model="deepseek-chat",
                      verifier_model="deepseek-reasoner", plan=lambda t, e: [])
        assert False, "same-family verifier must be rejected"
    except ValueError:
        pass

    # 6. bad plan (planner returns junk) -> F2 escalates, never crashes.
    r6 = build_project("t", REPO, external_test=None, plan=lambda t, e: [{"nope": 1}],
                       gen=lambda u, fb: "x", run_test=lambda u, c, tc: (True, ""),
                       run_snippet=lambda c, a: (True, ""), propose_test=lambda c, s: "",
                       integrate=lambda e, rs: (True, ""), commit=lambda n, rr: None)
    assert r6["status"] == "escalate" and "invalid plan" in r6["reason"], r6

    # 7. the DEFAULT planner THROWS (e.g. API 503) -> escalate with the real reason surfaced, not just
    #    F2's generic 'empty worklist'. Patch this module's `decompose` global so no API is hit.
    def _boom(*a, **k):
        raise RuntimeError("deepseek 503")
    _orig = globals()["decompose"]
    globals()["decompose"] = _boom
    try:
        r7 = build_project("t", REPO, external_test=None,
                           gen=lambda u, fb: "x", run_test=lambda u, c, tc: (True, ""),
                           run_snippet=lambda c, a: (True, ""), propose_test=lambda c, s: "",
                           integrate=lambda e, rs: (True, ""), commit=lambda n, rr: None)
    finally:
        globals()["decompose"] = _orig
    assert r7["status"] == "escalate" and "deepseek 503" in r7.get("plan_error", ""), r7

    print("project_integrate F3 OK — hot coder loop, integration gate, unverified ceiling, "
          "deterministic floor, cross-family guard, escalate, planner-error surfaced")
