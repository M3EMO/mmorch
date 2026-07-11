"""project_build — F1 of the /project rebuild: decompose a big task into a VALIDATED worklist
+ a DETERMINISTIC stub detector. The planner (LLM) is injectable so the deterministic core is
tested cero-cost; the LLM never gates — the AST/DAG checks do.

Why deterministic: the whole rebuild rests on NOT trusting LLM judgement (measured ~74% false).
So the plan validator (DAG, resolvable deps) and the stub detector (AST) are pure code — they catch
the exact failure that escaped the old flat workflow (a coder returning a 130-char import-only stub).

- decompose(task) -> worklist [{name, spec, deps, test_cmd}], validated (raises on a bad plan).
- validate_worklist -> (ok, errors): unique names, non-empty specs, deps resolve, NO cycles.
- stub_check(code) -> (is_stub, reason): AST — no defs / all-trivial-bodies / syntax error = stub.
- build_order(worklist) -> [names]: topological sort (the build sequence).
"""
from __future__ import annotations

import json
from typing import Callable

from .config import DEFAULT_GENERATOR


# --- deterministic plan validation ----------------------------------------- #
def validate_worklist(units: list[dict]) -> tuple[bool, list[str]]:
    """Pure structural check of a decomposition. No LLM. Returns (ok, errors)."""
    errs: list[str] = []
    names = [u.get("name") for u in units]
    if not units:
        return False, ["empty worklist"]
    if len(set(names)) != len(names):
        errs.append("duplicate unit names")
    known = set(names)
    files = [str(u.get("file")).replace("\\", "/").lower() for u in units if u.get("file")]
    if len(set(files)) != len(files):    # two units writing the SAME file at one level = a silent overwrite
        errs.append("duplicate target file across units")
    for u in units:
        if not u.get("name"):
            errs.append("a unit is missing 'name'")
        if not str(u.get("spec", "")).strip():
            errs.append(f"unit '{u.get('name')}' has empty spec")
        for d in u.get("deps", []) or []:
            if d not in known:
                errs.append(f"unit '{u.get('name')}' depends on unknown unit '{d}'")
    if not errs and _has_cycle(units):
        errs.append("dependency cycle (deps must form a DAG)")
    return (not errs, errs)


def build_order(units: list[dict]) -> list[str]:
    """Topological build order (Kahn). Assumes validate_worklist passed (DAG, deps resolve)."""
    deps = {u["name"]: set(u.get("deps", []) or []) for u in units}
    order, ready = [], sorted(n for n, d in deps.items() if not d)
    deps = {n: set(d) for n, d in deps.items()}
    while ready:
        n = ready.pop(0)
        order.append(n)
        for m, d in deps.items():
            if n in d:
                d.discard(n)
                if not d and m not in order and m not in ready:
                    ready.append(m)
        ready.sort()
    if len(order) != len(units):
        raise ValueError("cycle — build_order requires a validated DAG")
    return order


def _has_cycle(units: list[dict]) -> bool:
    try:
        build_order(units)
        return False
    except ValueError:
        return True


# --- deterministic stub detection (delegated to lang.py registry) ---------- #
def stub_check(code: str, file: str | None = None) -> tuple[bool, str]:
    """True if `code` is a stub. MULTI-LANGUAGE: delegates to the per-extension registry
    (lang.py) — Python via AST (finest), JS via node --check + textual, unknown via a generic
    substance floor. `file=None` -> Python (back-compat). Deterministic — catches the
    import-only stub class that escaped the old flat workflow, in any registered language."""
    from .lang import for_file
    # __init__.py: import-only/re-export ES su trabajo legitimo (bench rate-limiter 2026-07-11:
    # el detector lo marcaba stub -> recursion -> cap -> escalate, falso positivo). Basta que
    # parsee — el integration gate juzga si los re-exports son los correctos.
    if file and file.replace("\\", "/").endswith("__init__.py"):
        ok, why = for_file(file).syntax_ok(code)
        return (not ok), (why if not ok else "")
    return for_file(file).stub_check(code)


# --- decomposition (LLM planner is injectable; never gates) ---------------- #
_WORKLIST_SYS = (
    "You decompose a build task into a JSON worklist. Output ONLY a JSON array of units: "
    '[{"name": "...", "spec": "what to build, concrete", "file": "relative/path/of/the_one_file.py", '
    '"deps": ["other-unit-names"], "test_cmd": "an EXISTING command that verifies this unit, or null"}]. '
    "Order by dependency; no cycles; each unit small enough to implement in ONE file — `file` is that "
    "file's path relative to the repo root (the file the unit creates or edits; REQUIRED). Do NOT "
    "invent tests — test_cmd must be an existing/user-provided command or null.")


def _plan_user_msg(task: str, external_test: str | None) -> str:
    return f"TASK:\n{task}\n\nExternal acceptance (the real backstop): {external_test or '(none given)'}"


def _default_plan(task: str, external_test: str | None, gen_model: str) -> str:
    from .providers import call
    from .textutil import extract_fence
    out = call(gen_model, [{"role": "system", "content": _WORKLIST_SYS},
                           {"role": "user", "content": _plan_user_msg(task, external_test)}],
               pattern="project_build", node="planner", temperature=0.0).text
    return extract_fence(out)


def _default_reask(task: str, external_test: str | None, gen_model: str,
                   prev_raw: str, errors: list[str]) -> str:
    """Re-ask del planner con SUS errores (patron Instructor, leido del codigo: el modelo ve su
    output anterior + los errores concretos con contexto -> se corrige solo la gran mayoria de
    las veces). Los errores de validate_worklist ya traen el 'field path' (que unit, que campo)."""
    from .providers import call
    from .textutil import extract_fence
    msgs = [{"role": "system", "content": _WORKLIST_SYS},
            {"role": "user", "content": _plan_user_msg(task, external_test)},
            {"role": "assistant", "content": prev_raw},
            {"role": "user", "content": "Correct your JSON ONLY RESPONSE, based on the following "
                                        "errors:\n" + "\n".join(errors)}]
    out = call(gen_model, msgs, pattern="project_build", node="planner-reask", temperature=0.0).text
    return extract_fence(out)


def _parse_worklist(raw: str) -> list[dict]:
    blob = raw.strip()
    i, j = blob.find("["), blob.rfind("]")
    data = json.loads(blob[i:j + 1] if i >= 0 and j >= 0 else blob)
    if not isinstance(data, list):
        raise ValueError("worklist is not a JSON array")
    return [{"name": str(u.get("name") or ""), "spec": str(u.get("spec") or ""),   # null -> "" (not "None")
             "file": str(u["file"]) if u.get("file") else None,   # target path (F3 derives name.py if absent)
             "deps": list(u.get("deps", []) or []), "test_cmd": u.get("test_cmd")}
            for u in data if isinstance(u, dict)]


def decompose(task: str, *, external_test: str | None = None,
              plan: Callable[[], str] | None = None, gen_model: str = DEFAULT_GENERATOR,
              max_reask: int = 2,
              reask: Callable[[str, list[str]], str] | None = None) -> list[dict]:
    """Decompose `task` into a VALIDATED worklist. `plan` (injectable) returns the raw worklist JSON;
    default asks a model. A structurally INVALID plan (bad JSON / duplicate names / unknown deps /
    cycles) is RE-ASKED up to `max_reask` times showing the model its own output + the concrete
    errors (Instructor pattern) — before, the first invalid plan was terminal. `reask(prev_raw,
    errors)` is injectable (test seam). Raises ValueError only after the retries are exhausted."""
    plan = plan or (lambda: _default_plan(task, external_test, gen_model))
    reask = reask or (lambda prev, errs: _default_reask(task, external_test, gen_model, prev, errs))
    raw = plan()
    for attempt in range(max_reask + 1):
        try:
            units = _parse_worklist(raw)
            ok, errs = validate_worklist(units)
        except (json.JSONDecodeError, ValueError) as e:
            ok, errs = False, [f"output is not a valid JSON worklist: {str(e)[:120]}"]
        if ok:
            return units
        if attempt < max_reask:            # no wasted re-ask on the final failed attempt
            raw = reask(raw, errs)
    raise ValueError(f"invalid decomposition after {max_reask} re-asks: {errs}")


if __name__ == "__main__":
    # 1. validate_worklist: good DAG passes; cycle / bad-dep / empty-spec fail.
    good = [{"name": "a", "spec": "build a", "deps": []},
            {"name": "b", "spec": "build b", "deps": ["a"]},
            {"name": "c", "spec": "build c", "deps": ["a", "b"]}]
    assert validate_worklist(good) == (True, []), validate_worklist(good)
    assert build_order(good) == ["a", "b", "c"], build_order(good)
    cyc = [{"name": "x", "spec": "x", "deps": ["y"]}, {"name": "y", "spec": "y", "deps": ["x"]}]
    assert validate_worklist(cyc)[0] is False and "cycle" in validate_worklist(cyc)[1][0]
    baddep = [{"name": "a", "spec": "a", "deps": ["ghost"]}]
    assert validate_worklist(baddep)[0] is False
    assert validate_worklist([{"name": "a", "spec": "  "}])[0] is False   # empty spec
    assert validate_worklist([])[0] is False                              # empty worklist
    # 2. stub_check: the EXACT stub that escaped the old workflow -> flagged.
    escaped = "# tables_v2/__init__.py\nfrom .pipeline import rebuild_tables\n__all__ = ['rebuild_tables']"
    assert stub_check(escaped)[0] is True, "must catch the import-only stub"
    assert stub_check("def f():\n    pass")[0] is True
    assert stub_check("def f():\n    raise NotImplementedError")[0] is True
    assert stub_check("def f():\n    ...")[0] is True
    assert stub_check("def f(x):\n    '''doc'''\n    return x + 1")[0] is False   # real body
    assert stub_check("def f(:\n bad")[0] is True                          # syntax error = stub
    assert stub_check("class C:\n    X = 1")[0] is False                   # config class ok
    # execution-arbitrated critiques from the mmorch review (a valid critique = a failing test):
    assert stub_check("def f():\n    raise mod.NotImplementedError")[0] is True   # qualified NotImplementedError
    assert stub_check("def f():\n    raise a.b.NotImplementedError()")[0] is True  # nested-qualified too (round-2 dismissal locked)
    assert validate_worklist(_parse_worklist('[{"name":null,"spec":"x"}]'))[0] is False  # null name rejected, not "None"
    _u = [{"name": "a", "spec": "a", "deps": []}, {"name": "b", "spec": "b", "deps": ["a"]}]
    import copy as _copy
    _b = _copy.deepcopy(_u)
    build_order(_u)
    assert _u == _b, "build_order must not mutate its input (round-2 dismissal locked)"
    # 3. decompose with an injected fake plan (cero-cost, no API).
    fake = '[{"name":"core","spec":"the core","file":"pkg/core.py","deps":[],"test_cmd":"pytest -q"}]'
    wl = decompose("build a thing", plan=lambda: fake)
    assert wl[0]["name"] == "core" and wl[0]["test_cmd"] == "pytest -q", wl
    assert wl[0]["file"] == "pkg/core.py", wl          # target path flows through (F3 writes THERE, not root)
    assert decompose("x", plan=lambda: '[{"name":"a","spec":"a"}]')[0]["file"] is None  # absent -> None (F3 derives)
    dupf = [{"name": "a", "spec": "a", "file": "x.py"}, {"name": "b", "spec": "b", "file": "X.py"}]
    ok, errs = validate_worklist(dupf)                 # same target file (case-insens) = silent overwrite
    assert not ok and any("duplicate target file" in e for e in errs), errs
    # 4. RE-ASK (patron Instructor): plan invalido -> el modelo ve sus errores y se corrige.
    reasks: list = []

    def _fix_on_reask(prev, errs):
        reasks.append(errs)
        assert any("ghost" in e for e in errs), errs        # el error concreto viaja al modelo
        return '[{"name":"a","spec":"a","deps":[]}]'        # corregido

    wl2 = decompose("x", plan=lambda: '[{"name":"a","spec":"a","deps":["ghost"]}]', reask=_fix_on_reask)
    assert wl2[0]["name"] == "a" and len(reasks) == 1, (wl2, reasks)
    # JSON roto tambien es re-askeable (no solo invalido estructural)
    wl3 = decompose("x", plan=lambda: "not json at all",
                    reask=lambda p, e: '[{"name":"b","spec":"b","deps":[]}]')
    assert wl3[0]["name"] == "b", wl3
    # incorregible -> raise DESPUES de agotar, sin re-ask extra en el ultimo intento
    tries: list = []

    def _never_fixes(p, e):
        tries.append(1)
        return '[{"nope":1}]'
    try:
        decompose("x", plan=lambda: '[{"name":"a","deps":["ghost"]}]', reask=_never_fixes, max_reask=2)
        assert False, "bad plan must raise after retries"
    except ValueError:
        pass
    assert len(tries) == 2, tries                            # exactamente max_reask re-asks
    print("project_build F1 OK — validate(DAG), build_order, stub_check(incl the escaped stub), decompose seam")
