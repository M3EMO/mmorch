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

import ast
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


# --- deterministic stub detection (AST, no LLM) ---------------------------- #
def _is_trivial_stmt(s: ast.stmt) -> bool:
    """pass / ... / raise NotImplementedError = a stub body statement."""
    if isinstance(s, ast.Pass):
        return True
    if isinstance(s, ast.Expr) and isinstance(s.value, ast.Constant) and s.value.value is Ellipsis:
        return True
    if isinstance(s, ast.Raise):
        exc = s.exc
        # bare `raise NotImplementedError`, `raise NotImplementedError()`, qualified `raise m.NotImplementedError`
        name = (getattr(exc, "id", None) or getattr(getattr(exc, "func", None), "id", None)
                or getattr(exc, "attr", None) or getattr(getattr(exc, "func", None), "attr", None))
        return name in ("NotImplementedError", "NotImplemented")
    return False


def stub_check(code: str) -> tuple[bool, str]:
    """True if `code` is a stub: won't parse, has no def/class, or every function body is trivial
    (pass/.../NotImplementedError, docstrings ignored). Deterministic — catches the import-only stub
    that escaped the old flat workflow."""
    try:
        tree = ast.parse(code)
    except SyntaxError as e:
        return True, f"syntax error: {str(e)[:80]}"
    funcs = [n for n in ast.walk(tree) if isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef))]
    classes = [n for n in ast.walk(tree) if isinstance(n, ast.ClassDef)]
    if not funcs and not classes:
        return True, "no function/class definitions (import-only / __all__ stub)"
    if not funcs:
        return False, ""   # classes with attributes/config are legitimate non-stubs
    trivial = 0
    for f in funcs:
        body = [s for s in f.body
                if not (isinstance(s, ast.Expr) and isinstance(s.value, ast.Constant)
                        and isinstance(s.value.value, str))]   # drop docstrings
        if not body or all(_is_trivial_stmt(s) for s in body):
            trivial += 1
    if trivial == len(funcs):
        return True, f"all {len(funcs)} function(s) are stubs (pass/.../NotImplementedError)"
    return False, ""


# --- decomposition (LLM planner is injectable; never gates) ---------------- #
_WORKLIST_SYS = (
    "You decompose a build task into a JSON worklist. Output ONLY a JSON array of units: "
    '[{"name": "...", "spec": "what to build, concrete", "file": "relative/path/of/the_one_file.py", '
    '"deps": ["other-unit-names"], "test_cmd": "an EXISTING command that verifies this unit, or null"}]. '
    "Order by dependency; no cycles; each unit small enough to implement in ONE file — `file` is that "
    "file's path relative to the repo root (the file the unit creates or edits; REQUIRED). Do NOT "
    "invent tests — test_cmd must be an existing/user-provided command or null.")


def _default_plan(task: str, external_test: str | None, gen_model: str) -> str:
    from .providers import call
    from .textutil import extract_fence
    u = f"TASK:\n{task}\n\nExternal acceptance (the real backstop): {external_test or '(none given)'}"
    out = call(gen_model, [{"role": "system", "content": _WORKLIST_SYS}, {"role": "user", "content": u}],
               pattern="project_build", node="planner", temperature=0.0).text
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
              plan: Callable[[], str] | None = None, gen_model: str = DEFAULT_GENERATOR) -> list[dict]:
    """Decompose `task` into a VALIDATED worklist. `plan` (injectable) returns the raw worklist JSON;
    default asks a model. Raises ValueError if the decomposition is structurally invalid."""
    plan = plan or (lambda: _default_plan(task, external_test, gen_model))
    units = _parse_worklist(plan())
    ok, errs = validate_worklist(units)
    if not ok:
        raise ValueError(f"invalid decomposition: {errs}")
    return units


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
    try:
        decompose("x", plan=lambda: '[{"name":"a","deps":["ghost"]}]')
        assert False, "bad plan must raise"
    except ValueError:
        pass
    print("project_build F1 OK — validate(DAG), build_order, stub_check(incl the escaped stub), decompose seam")
