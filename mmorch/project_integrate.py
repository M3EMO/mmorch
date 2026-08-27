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

import os
import shlex
import subprocess
from typing import Callable

from .config import DEFAULT_GENERATOR, DEFAULT_VERIFIER, family_of
from .project_build import decompose, validate_test_cmd
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


def _syntax_ok(code: str, file: str | None) -> tuple[bool, str]:
    """Deterministic floor for an untested unit: it must at least PARSE in its own language
    (lang registry: py=AST, js=node --check, unknown=fail-open — execution judges the rest)."""
    from .lang import for_file
    return for_file(file).syntax_ok(code)


# --- production boundaries (injectable; these are the only things that touch API / disk) --------- #
# NOT prompts.LAZY_SYSTEM: "minimal code" reads as "output only the changed part", which fights the
# full-file regeneration contract (F4: a 200-line module came back as a 20-line fragment). Minimality
# must apply to the CHANGE; the OUTPUT is always the whole file.
_CODER_SYS = (
    "You are a senior engineer editing ONE file of an existing repo. Make the SMALLEST change that "
    "satisfies the spec — but your output is ALWAYS the COMPLETE new content of the file (every line "
    "that must exist in it after your change, including everything you did not touch). Never output "
    "only the changed fragment. Return it in a single ``` block, no explanation.")


def _default_gen(gen_model: str, repo: str, task: str = ""):
    from .providers import call
    from .textutil import extract_fence

    # contract-verbatim (medido 2026-08: 4/4 builds con "CONTRATO EXACTO" en el
    # task fallaron porque las unidades solo veian el spec PARAFRASEADO por el
    # planner — el test-writer inventaba firmas). El task original viaja entero
    # a cada unidad; ante conflicto, el contrato literal manda sobre el spec.
    contract = (f"\nGLOBAL TASK / CONTRACT (verbatim — on any conflict this "
                f"OVERRIDES the SPEC paraphrase):\n{task[:8000]}\n") if task else ""

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
        user = (f"UNIT: {unit['name']}\nSPEC:\n{unit['spec']}\n{contract}\n"
                f"FILE `{_file_of(unit)}` (current):\n```\n{cur}\n```\n"
                + (f"\nThe previous attempt FAILED:\n{feedback[:1200]}\nFix it.\n" if feedback else "")
                + "Return ONLY the COMPLETE new file content in a ``` block.")
        out = call(gen_model, [{"role": "system", "content": _CODER_SYS},
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
        # test_cmd is PLANNER/LLM output (prompt-injection surface: audit-2026-08 #12), unlike
        # external_test which is the USER's own trusted command (see integrate() below) — NOT the
        # same trust level, so it does NOT get the same treatment. validate_worklist() already
        # gates this at plan time, but plan_fn is injectable (tests, alt callers) -> re-check here
        # so this boundary can never be bypassed by skipping validate_worklist. A worktree isolates
        # the FILE TREE, not process execution (it still runs with the user's env/network/home) —
        # it is not a substitute for this check.
        ok, why = validate_test_cmd(test_cmd)
        if not ok:
            return False, f"test_cmd REJECTED by policy (not executed): {why}"
        try:
            argv = shlex.split(test_cmd)
            p = subprocess.run(argv, cwd=repo, shell=False, capture_output=True, text=True,
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
        # external_test is the USER's OWN acceptance suite (trusted input, not model output — unlike
        # test_cmd, see run_test above), typically COMPOUND -> shell=True is required and acceptable
        # here. The git worktree (server exec_policy) isolates the FILE TREE the build writes to; it
        # is NOT process-execution containment (this still runs with the user's env/network/home) —
        # do not read it as a sandbox for what external_test itself does.
        try:
            p = subprocess.run(external_test, cwd=repo, shell=True, capture_output=True, text=True,  # noqa: S602
                               encoding="utf-8", errors="replace", timeout=timeout)
            return p.returncode == 0, (p.stdout + p.stderr)[-2000:]
        except subprocess.TimeoutExpired:
            return False, "TIMEOUT"
        except Exception as e:
            return False, str(e)[:300]
    return integrate


def _default_write_file(repo: str):
    def write_file(unit: dict, code: str) -> None:
        fpath = _safe_target(repo, unit)
        os.makedirs(os.path.dirname(fpath) or repo, exist_ok=True)
        with open(fpath, "w", encoding="utf-8") as f:
            f.write(code + ("\n" if not code.endswith("\n") else ""))
    return write_file


def _default_commit(repo: str):
    # Assumes `repo` is ALREADY the isolated working tree (the server opens a git worktree on a review
    # branch and passes its path). Commits in place on that branch -> per-unit history for git-bisect.
    from . import worktree_driver as wd
    if not wd.is_git_repo(repo):
        return None  # not a repo -> no per-unit commits; the build still runs (F2 guards commit_fn=None)

    def commit(name: str, result: dict) -> None:
        wt = wd.Worktree(repo, repo, "")   # commit in place on the current (caller-owned) branch
        cap = wt.capture(f"mmorch(project-build): unit {name}")
        if cap["changed"] and not cap["committed"]:
            # no hay job/emit acá (seam de bajo nivel) -> lo mas seguro sin romper la firma
            # commit(name,result)->None es levantar: silencioso == la unidad se pierde sin aviso.
            raise RuntimeError(f"unit {name}: commit falló en {repo}: {cap['error'][:200]}")
    return commit


# --- the wiring: bind seams over shared state, then drive F2 ------------------------------------- #
def build_project(task: str, repo: str, *, external_test: str | None,
                  gen_model: str = DEFAULT_GENERATOR, verifier_model: str = DEFAULT_VERIFIER,
                  max_fix: int = 3, max_depth: int = 2, max_gen_calls: int = 150,
                  max_usd_per_run: float | None = None,
                  run_cost: Callable[[], float] | None = None,
                  plan: Callable[[str, str | None], list[dict]] | None = None,
                  gen: Callable[[dict, str], str] | None = None,
                  run_test: Callable[[dict, str, str], tuple[bool, str]] | None = None,
                  run_snippet: Callable[[str, str], tuple[bool, str]] | None = None,
                  propose_test: Callable[[str, str], str] | None = None,
                  integrate: Callable[[str, list], tuple[bool, str]] | None = None,
                  commit: Callable[[str, dict], None] | None = None,
                  write_file: Callable[[dict, str], None] | None = None) -> dict:
    """Build `task` in `repo` via the recursive engine, cero cupo. `external_test` = the real
    acceptance command (the integration gate at depth 0). All boundary fns default to production
    (providers.call / subprocess / checkers / worktree) and are injectable for the self-check.
    Returns F2's result tree plus {'unverified': [names deferred to the integration gate]}."""
    if family_of(gen_model) == family_of(verifier_model):
        raise ValueError(f"coder and cold verifier must be cross-family: {gen_model}/{verifier_model} "
                         f"are both {family_of(gen_model)}")
    # learn only from REAL runs: injected gen/run_test = synthetic (self-checks, tests) — feeding
    # fake outcomes into the persistent bandit would poison exactly the signal we're trying to grow.
    _real_run = gen is None and run_test is None
    gen = gen or _default_gen(gen_model, repo, task)
    run_test = run_test or _default_run_test(repo)

    # breaker USD por-run (W3.4): el call-breaker de abajo acota LLAMADAS, no dolares —
    # una call cara (contexto grande, modelo premium, timeouts facturados) compone
    # distinto. providers.call() suma el costo (real o estimado) de cada API call al
    # tracker registrado; run_cost es el seam de test (costo fake sin API).
    if max_usd_per_run is None:
        try:
            max_usd_per_run = float(os.getenv("MMORCH_MAX_USD_PER_RUN", "5.0"))
        except ValueError:
            max_usd_per_run = 5.0
    _usd_tracker: dict = {"usd": 0.0}
    _run_cost = run_cost or (lambda: _usd_tracker["usd"])

    # call-breaker (blind-spot #6): units x max_fix x re-asks compone sin tope de $. Cota dura de
    # invocaciones al coder por run; excedida -> el build ESCALA (nunca sigue quemando en silencio).
    _gen_calls = {"n": 0}
    _inner_gen = gen

    def gen(unit: dict, feedback: str) -> str:   # type: ignore[no-redef]
        _gen_calls["n"] += 1
        if _gen_calls["n"] > max_gen_calls:
            raise RuntimeError(f"call-breaker: >{max_gen_calls} coder calls in one build")
        if max_usd_per_run > 0 and _run_cost() > max_usd_per_run:
            raise RuntimeError(
                f"usd-breaker: costo acumulado del run ${_run_cost():.4f} supera "
                f"max_usd_per_run ${max_usd_per_run:.2f}")
        return _inner_gen(unit, feedback)
    run_snippet = run_snippet or _default_run_snippet()
    propose_test = propose_test or _default_propose_test(verifier_model)
    integrate = integrate or _default_integrate(repo)
    commit = commit if commit is not None else _default_commit(repo)
    write_file = write_file or _default_write_file(repo)

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

    def _learn(reward: float, context: str) -> None:
        # side-channel: every gate result is FREE execution-truth for the starving learning loops
        # (bandit n<=3/arm after 10k calls because almost no flow recorded outcomes). Never breaks
        # the build on a store hiccup.
        if not _real_run:
            return
        try:
            from .feedback import record_outcome
            from .intuition import record as intuition_record
            record_outcome(gen_model, reward, pattern="project_build", source="execution",
                           context=context)
            intuition_record(gen_model, reward, context)
        except Exception:
            pass

    def gate_fn(unit: dict, code: str) -> tuple[bool, str]:
        # COLD verifier: independent of the coder's loop.
        tc = unit.get("test_cmd")
        if tc:
            ok, out = run_test(unit, code, tc)    # clean re-run = execution truth
            _learn(1.0 if ok else 0.0, unit["spec"])
            if not ok:
                cold_feedback[unit["name"]] = f"the clean re-run failed:\n{out[:300]}"
            return ok, (f"verified: {out[-160:]}" if ok else out[:250])
        # no test_cmd: NOT independently provable. Floor = parses in ITS language; then (py only —
        # run_snippet is a python sandbox) an ADVISORY cold probe.
        valid, why = _syntax_ok(code, unit.get("file"))
        if not valid:
            return False, f"does not parse: {why}"
        # LAND the code (F4 round-1 bug: only run_test wrote to disk, so an untested unit's code never
        # reached the tree and the integration gate ran against NOTHING). Gate-time = post-stub-check,
        # so a stub never lands. Tested units are written by run_test itself.
        write_file(unit, code)
        probe = propose_test(code, unit["spec"]) if _file_of(unit).endswith(".py") else ""
        if probe.strip():
            passed, detail = run_snippet(code, probe)
            if not passed:                        # advisory: stash for the coder, but do NOT hard-fail
                cold_feedback[unit["name"]] = f"a reviewer's probe failed (advisory):\n{probe}\n{detail[:200]}"
        if unit["name"] not in unverified:
            unverified.append(unit["name"])
        return True, "unverified (no test_cmd; correctness deferred to the integration gate)"

    def integrate_fn(ext: str, results: list) -> tuple[bool, str]:
        iok, idetail = integrate(ext, results)
        _learn(1.0 if iok else 0.0, task)          # the whole-assembly verdict is a signal too
        return iok, idetail

    from .providers import register_run_tracker, unregister_run_tracker
    register_run_tracker(_usd_tracker)
    try:
        res = run_project_build(task, external_test=external_test, plan_fn=plan_fn, build_fn=build_fn,
                                gate_fn=gate_fn, commit_fn=commit, integrate_fn=integrate_fn,
                                max_depth=max_depth)
    finally:
        unregister_run_tracker(_usd_tracker)
    res["unverified"] = unverified
    if res.get("status") == "escalate" and plan_err.get("last"):
        res["plan_error"] = plan_err["last"]   # surface the swallowed planner failure, not just 'empty worklist'
    # provenance (blind-spot #9): sin esto, cuando el prompt-bootstrap aterrice no se puede
    # ATRIBUIR una mejora a un prompt/modelo concreto. Hash corto del system-prompt del coder =
    # la "versión" del prompt; few_shots reservado para el graft DSPy-A.
    import hashlib
    res["provenance"] = {"gen_model": gen_model, "verifier_model": verifier_model,
                         "coder_sys": hashlib.sha256(_CODER_SYS.encode()).hexdigest()[:12],
                         "gen_calls": _gen_calls["n"], "few_shots": None,
                         "run_usd": round(_usd_tracker["usd"], 6)}
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
                       plan=lambda t, e: [{"name": "u", "spec": "the unit", "deps": [], "test_cmd": "pytest -q"}],
                       gen=gen_hot, run_test=run_reds_then_green, run_snippet=lambda c, a: (True, ""),
                       propose_test=lambda c, s: "", integrate=lambda e, rs: (True, "accept green"),
                       commit=lambda n, rr: None)
    assert r1["status"] == "built" and r1.get("integrated"), r1
    assert len(seen_fb) == 3 and seen_fb[0] == "" and "red" in seen_fb[1], seen_fb  # 1st clean, then reds

    # 2. INTEGRATION GATE: all units pass their own tests, but the assembled whole fails -> integration_failed.
    r2 = build_project("top", REPO, external_test="ACCEPT",
                       plan=lambda t, e: [{"name": "a", "spec": "a", "deps": [], "test_cmd": "pytest -q"},
                                          {"name": "b", "spec": "b", "deps": ["a"], "test_cmd": "pytest -q"}],
                       gen=lambda u, fb: f"def {u['name']}():\n    return 1",
                       run_test=lambda u, c, tc: (True, "unit green"),
                       run_snippet=lambda c, a: (True, ""), propose_test=lambda c, s: "",
                       integrate=lambda e, rs: (False, "3 failed: interface mismatch"),
                       commit=lambda n, rr: None)
    assert r2["status"] == "integration_failed" and "mismatch" in r2["detail"], r2

    # 3. UNTESTED unit: no test_cmd -> gate passes it as 'unverified' (never called correct), a failing
    #    cold probe is ADVISORY only (does not fail the gate); correctness is deferred to integration.
    #    AND its code must LAND on disk (F4 round-1 bug: only run_test wrote -> integration saw nothing).
    committed: list = []
    written: dict = {}
    r3 = build_project("top", REPO, external_test="ACCEPT",
                       plan=lambda t, e: [{"name": "u", "spec": "s", "deps": [], "file": "pkg/u.py"}],
                       gen=lambda u, fb: "def u():\n    return 1",
                       run_test=lambda u, c, tc: (True, ""),          # not consulted (no test_cmd)
                       run_snippet=lambda c, a: (False, "probe failed"),   # probe FAILS -> advisory
                       propose_test=lambda c, s: "assert u() == 2",
                       integrate=lambda e, rs: (True, "accept green"),
                       commit=lambda n, rr: committed.append(n),
                       write_file=lambda u, c: written.__setitem__(u.get("file"), c))
    assert r3["status"] == "built" and r3["unverified"] == ["u"], r3   # passed, but honestly unverified
    assert committed == ["u"], committed                              # a leaf still commits
    assert "pkg/u.py" in written and "def u()" in written["pkg/u.py"], written  # the code LANDED
    assert r3["results"][0].get("file") == "pkg/u.py", r3["results"]  # observability: file in the result

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

    # 8. call-breaker: un coder que nunca verdea no puede quemar llamadas sin tope -> escalate.
    r8 = build_project("top", REPO, external_test=None, max_gen_calls=4,
                       plan=lambda t, e: [{"name": "u", "spec": "s", "deps": [], "test_cmd": "pytest -q"}],
                       gen=lambda u, fb: "def u():\n    return 1",
                       run_test=lambda u, c, tc: (False, "always red"),
                       run_snippet=lambda c, a: (True, ""), propose_test=lambda c, s: "",
                       integrate=lambda e, rs: (True, ""), commit=lambda n, rr: None)
    assert r8["status"] == "escalate", r8
    assert r8["provenance"]["gen_calls"] >= 4, r8["provenance"]     # el conteo viaja en provenance
    # 9. provenance siempre presente (atribución del futuro prompt-bootstrap)
    assert r3["provenance"]["gen_model"] and len(r3["provenance"]["coder_sys"]) == 12, r3["provenance"]

    print("project_integrate F3 OK — hot coder loop, integration gate, unverified ceiling, "
          "deterministic floor, cross-family guard, escalate, planner-error surfaced, "
          "call-breaker, provenance")
