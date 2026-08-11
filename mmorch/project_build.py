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

import hashlib
import json
import re
import shlex
from pathlib import Path
from typing import Callable

from .config import DEFAULT_GENERATOR

# --- test_cmd allowlist (audit-2026-08 #12: test_cmd is PLANNER/LLM output that lands in
# subprocess.run(shell=True) at the F3 gate -- a real prompt-injection -> RCE vector, unlike
# external_test which is the USER's own trusted command. Deterministic, no LLM: reject shell
# metacharacters outright, then require the first token to be a known test-runner binary. Gates
# TWICE: here (validate_worklist, so a bad test_cmd triggers the planner's normal re-ask loop)
# and again at execution time in project_integrate.py (defense-in-depth for injected plan_fns
# that skip validate_worklist, e.g. tests/other callers). ---
_TEST_CMD_BINS = {
    "pytest", "python", "python3", "py", "unittest", "tox", "nose2",
    "node", "npm", "npx", "yarn", "pnpm", "jest", "vitest",
    "go", "cargo", "mvn", "gradle", "make", "ruff", "mypy", "coverage",
}
_SHELL_META = re.compile(r"[;&|$`(){}<>\n\r]")


def validate_test_cmd(cmd: str | None) -> tuple[bool, str]:
    """Deterministic allowlist gate for a test_cmd. None is VALID (unit stays 'unverified', decided
    by the caller). Rejects shell metacharacters (;|&$`(){}<> and redirection/newlines) so a
    compound/chained payload (`pytest; rm -rf .`, `$(curl evil)`, backticks) never reaches a shell,
    then requires the resolved binary name to be a known test/build runner. Case-sensitive on
    purpose (a binary name is not free text)."""
    if cmd is None:
        return True, ""
    if not isinstance(cmd, str) or not cmd.strip():
        return False, "test_cmd is empty/not a string"
    if _SHELL_META.search(cmd):
        return False, f"test_cmd contains disallowed shell metacharacters: {cmd!r}"
    try:
        tokens = shlex.split(cmd)
    except ValueError as e:
        return False, f"test_cmd failed to tokenize: {e}"
    if not tokens:
        return False, "test_cmd parses to no tokens"
    bin_name = tokens[0].replace("\\", "/").rsplit("/", 1)[-1]
    bin_name = re.sub(r"\.(exe|cmd|bat)$", "", bin_name, flags=re.IGNORECASE)
    if bin_name not in _TEST_CMD_BINS:
        return False, (f"test_cmd binary {bin_name!r} not in allowlist "
                       f"{sorted(_TEST_CMD_BINS)}")
    return True, ""

# --- worklist cache (goal-regression pattern, research 2026-07: ChatHTN caches LLM-validated
# decompositions as reusable HTN methods so a repeat task skips the LLM entirely -- zero variance
# instead of re-rolling the dice. mmorch's planner is nondeterministic even at temp=0 (measured:
# same rate-limiter task, same config -> sometimes 'duplicate target file', sometimes clean); a
# task run twice (nightly workflow_race, bench re-runs) shouldn't pay that risk twice.) ---
_WORKLIST_CACHE = Path(__file__).resolve().parents[1] / "logs" / "worklist_cache.json"


def _cache_key(task: str, external_test: str | None) -> str:
    # SOLO `task`: `external_test` es un comando (ej pytest) que suele traer un tmpdir ÚNICO por
    # corrida (materialize() crea un mkdtemp nuevo cada vez) -> incluirlo en el hash rompía CADA
    # cache hit real (bug medido 2026-07: workflow_race nunca pegaba, cacheaba bajo una key que
    # nunca se repetía). La decomposición depende del TASK, no de dónde vive el test de aceptación.
    return hashlib.sha256(task.encode()).hexdigest()[:16]


def _load_json_cache(path: Path) -> dict:
    if not path.exists():
        return {}
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError):
        return {}


def _save_json_cache(path: Path, key: str, value) -> None:
    data = _load_json_cache(path)
    data[key] = value
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2), encoding="utf-8")


# --- per-unit code cache (mismo patron, un nivel mas abajo: goal-regression research 2026-07.
# El worklist cache saco la varianza del PLANNER; queda la del CODER -- misma unit (name+file+spec
# byte-identicos porque el worklist YA es el mismo entre variantes/corridas gracias al cache de
# arriba) puede generar codigo distinto cada vez que build_unit la re-pide. Cachear el codigo que
# ya paso el gate una vez evita re-tirar los dados del coder tambien. medido: rate-limiter, 3
# variantes corridas en secuencia con worklist cache tibio -> 2/3 build, pero CUAL falla varia
# corrida a corrida (pb-quick una vez, pb-deep otra) -- confirma que la varianza restante es del
# coder, no del planner.) ---
_UNIT_CODE_CACHE = Path(__file__).resolve().parents[1] / "logs" / "unit_code_cache.json"


def unit_cache_key(unit: dict) -> str:
    blob = f"{unit.get('name', '')}\x00{unit.get('file', '')}\x00{unit.get('spec', '')}"
    return hashlib.sha256(blob.encode()).hexdigest()[:16]


def load_cached_unit_code(unit: dict) -> str | None:
    return _load_json_cache(_UNIT_CODE_CACHE).get(unit_cache_key(unit))


def save_cached_unit_code(unit: dict, code: str) -> None:
    _save_json_cache(_UNIT_CODE_CACHE, unit_cache_key(unit), code)


def _save_cache(key: str, units: list[dict]) -> None:
    _save_json_cache(_WORKLIST_CACHE, key, units)


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
        # nombra los archivos y las units en conflicto (medido 2026-07, bench rate-limiter: el
        # mensaje generico no le decia al modelo QUE archivo repitio -> el reask no se corregia
        # siempre). Con el archivo+units explicitos, el modelo tiene lo que necesita para fusionar
        # o renombrar en el reask.
        seen: dict[str, list[str]] = {}
        for u in units:
            f = str(u.get("file")).replace("\\", "/").lower() if u.get("file") else None
            if f:
                seen.setdefault(f, []).append(str(u.get("name")))
        dupes = {f: ns for f, ns in seen.items() if len(ns) > 1}
        errs.append("duplicate target file across units: " +
                    "; ".join(f"{f} used by {ns}" for f, ns in dupes.items()))
    for u in units:
        if not u.get("name"):
            errs.append("a unit is missing 'name'")
        if not str(u.get("spec", "")).strip():
            errs.append(f"unit '{u.get('name')}' has empty spec")
        for d in u.get("deps", []) or []:
            if d not in known:
                errs.append(f"unit '{u.get('name')}' depends on unknown unit '{d}'")
        tc_ok, tc_why = validate_test_cmd(u.get("test_cmd"))
        if not tc_ok:
            errs.append(f"unit '{u.get('name')}' has invalid test_cmd: {tc_why}")
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
    "invent tests — test_cmd must be an existing/user-provided command or null. test_cmd must be a "
    "SIMPLE command (no ;, &&, |, $(), backticks, redirection) starting with a known test/build "
    "runner (pytest, python, node, npm, go, cargo, make, ...).")


def _plan_user_msg(task: str, external_test: str | None) -> str:
    return f"TASK:\n{task}\n\nExternal acceptance (the real backstop): {external_test or '(none given)'}"


def _default_plan(task: str, external_test: str | None, gen_model: str,
                  temperature: float = 0.0) -> str:
    from .providers import call
    from .textutil import extract_fence
    out = call(gen_model, [{"role": "system", "content": _WORKLIST_SYS},
                           {"role": "user", "content": _plan_user_msg(task, external_test)}],
               pattern="project_build", node="planner", temperature=temperature).text
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
              max_reask: int = 3, use_cache: bool = True,
              reask: Callable[[str, list[str]], str] | None = None) -> list[dict]:
    """Decompose `task` into a VALIDATED worklist. `plan` (injectable) returns the raw worklist JSON;
    default asks a model. A structurally INVALID plan (bad JSON / duplicate names / unknown deps /
    cycles) is RE-ASKED up to `max_reask` times showing the model its own output + the concrete
    errors (Instructor pattern) — before, the first invalid plan was terminal. `reask(prev_raw,
    errors)` is injectable (test seam). Raises ValueError only after the retries are exhausted.

    use_cache: si (task, external_test) ya produjo un worklist VALIDADO antes, lo devuelve directo
    -- CERO llamadas al LLM, cero varianza (goal-regression: research 2026-07, ChatHTN). Solo
    cachea planes que pasaron validate_worklist; nunca cachea un plan roto."""
    key = _cache_key(task, external_test) if use_cache else None
    if key:
        cached = _load_json_cache(_WORKLIST_CACHE).get(key)
        if cached is not None:
            ok, _ = validate_worklist(cached)   # re-valida (el cache es un archivo editable a mano)
            if ok:
                return cached
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
            if key:
                _save_cache(key, units)
            return units
        if attempt < max_reask:            # no wasted re-ask on the final failed attempt
            raw = reask(raw, errs)
    raise ValueError(f"invalid decomposition after {max_reask} re-asks: {errs}")


# --- pipeline shape por op_type (ADW pattern, 2026-07: "no corras tu pipeline pesado
# para un chore"). El ruteo es DETERMINISTA (signature.op_type = regex, cero LLM, mismo
# input -> mismo pipeline siempre) — mejora sobre el patron original que usa un "router
# agent" LLM que puede alucinar el ruteo. El caller puede pisar cualquier campo. ---
PIPELINES: dict[str, dict] = {
    # hotfix quirurgico: minimo, rapido, sin optimizar nada; la review humana es el gate final
    "REPAIR":    {"max_fix": 1, "max_depth": 1, "max_gen_calls": 40},
    # verificar/chequear algo existente: chico, sin recursion
    "VERIFY":    {"max_fix": 1, "max_depth": 1, "max_gen_calls": 40},
    # refactor/traduccion: mas intentos de fix pero sin decomponer hondo
    "TRANSFORM": {"max_fix": 3, "max_depth": 1, "max_gen_calls": 80},
    # construccion desde cero: el engine completo (default F4)
    "GENERATE":  {"max_fix": 3, "max_depth": 2, "max_gen_calls": 150},
}


def pipeline_for(task: str) -> dict:
    """Forma de pipeline para la FORMA del task. Devuelve una copia (el caller puede mutar).
    op_type desconocido -> GENERATE (el mas conservador: engine completo)."""
    from .signature import signature
    op = signature(task).op_type
    shape = dict(PIPELINES.get(op, PIPELINES["GENERATE"]))
    shape["op_type"] = op
    return shape


def decompose_best_of(task: str, *, n: int = 3, external_test: str | None = None,
                      gen_model: str = DEFAULT_GENERATOR,
                      plan: Callable[[], str] | None = None,
                      reask: Callable[[str, list[str]], str] | None = None) -> list[dict]:
    """Best-of-N sobre el planner (idea session-trees de Pi + diversidad, 2026-07,
    ADAPTADA al invariante del repo: el patron original juzga los N candidatos con un
    torneo LLM, pero el juicio LLM midio ~74% falso -- aca la seleccion es DETERMINISTA).

    N muestras independientes del planner (su no-determinismo medido juega A FAVOR:
    diversidad gratis) -> validate_worklist filtra -> entre las validas gana la MAS SIMPLE
    (menos units; empate -> serializacion mas corta = menos especulacion, KISS). Ninguna
    valida -> cae a decompose() normal (path de re-ask). Cache-aware en ambas puntas:
    hit previo se devuelve directo, la ganadora se cachea."""
    key = _cache_key(task, external_test)
    cached = _load_json_cache(_WORKLIST_CACHE).get(key)
    if cached is not None and validate_worklist(cached)[0]:
        return cached
    # temp>0 A PROPOSITO (finding del harvest 2026-07, juzgado valido): a temp=0 la
    # diversidad entre muestras es solo el no-determinismo accidental de la API; el
    # best-of necesita diversidad DELIBERADA. decompose() normal sigue a temp=0.
    plan = plan or (lambda: _default_plan(task, external_test, gen_model, temperature=0.7))
    valid: list[list[dict]] = []
    for _ in range(n):
        try:
            units = _parse_worklist(plan())
            if validate_worklist(units)[0]:
                valid.append(units)
        except (json.JSONDecodeError, ValueError):
            continue                      # muestra rota = descartada, no rompe el best-of
    if not valid:
        # fallback al path de re-ask; plan/reask inyectados VIAJAN (sin esto el fallback
        # ignoraba el seam y llamaba al LLM real -- cazado por el self-check, 2026-07)
        return decompose(task, external_test=external_test, gen_model=gen_model,
                         plan=plan, reask=reask)
    best = min(valid, key=lambda u: (len(u), len(json.dumps(u, sort_keys=True))))
    _save_cache(key, best)
    return best


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
    wl = decompose("build a thing", plan=lambda: fake, use_cache=False)
    assert wl[0]["name"] == "core" and wl[0]["test_cmd"] == "pytest -q", wl
    assert wl[0]["file"] == "pkg/core.py", wl          # target path flows through (F3 writes THERE, not root)
    assert decompose("x", plan=lambda: '[{"name":"a","spec":"a"}]', use_cache=False)[0]["file"] is None  # absent -> None (F3 derives)
    dupf = [{"name": "a", "spec": "a", "file": "x.py"}, {"name": "b", "spec": "b", "file": "X.py"}]
    ok, errs = validate_worklist(dupf)                 # same target file (case-insens) = silent overwrite
    assert not ok and any("duplicate target file" in e for e in errs), errs
    # 4. RE-ASK (patron Instructor): plan invalido -> el modelo ve sus errores y se corrige.
    reasks: list = []

    def _fix_on_reask(prev, errs):
        reasks.append(errs)
        assert any("ghost" in e for e in errs), errs        # el error concreto viaja al modelo
        return '[{"name":"a","spec":"a","deps":[]}]'        # corregido

    wl2 = decompose("x", plan=lambda: '[{"name":"a","spec":"a","deps":["ghost"]}]', reask=_fix_on_reask, use_cache=False)
    assert wl2[0]["name"] == "a" and len(reasks) == 1, (wl2, reasks)
    # JSON roto tambien es re-askeable (no solo invalido estructural)
    wl3 = decompose("x", plan=lambda: "not json at all",
                    reask=lambda p, e: '[{"name":"b","spec":"b","deps":[]}]', use_cache=False)
    assert wl3[0]["name"] == "b", wl3
    # incorregible -> raise DESPUES de agotar, sin re-ask extra en el ultimo intento
    tries: list = []

    def _never_fixes(p, e):
        tries.append(1)
        return '[{"nope":1}]'
    try:
        decompose("x", plan=lambda: '[{"name":"a","deps":["ghost"]}]', reask=_never_fixes, max_reask=2, use_cache=False)
        assert False, "bad plan must raise after retries"
    except ValueError:
        pass
    assert len(tries) == 2, tries                            # exactamente max_reask re-asks

    # 5. CACHE (goal-regression): un plan que validó se cachea; el 2do decompose del MISMO task
    # NO llama al plan() inyectado (cero LLM real) -- prueba con un _cache_path aislado (no toca
    # logs/worklist_cache.json real).
    import tempfile as _tf
    _orig_cache = _WORKLIST_CACHE
    globals()["_WORKLIST_CACHE"] = Path(_tf.mkdtemp()) / "wl_cache_test.json"
    try:
        calls = {"n": 0}

        def _counting_plan():
            calls["n"] += 1
            return fake
        wl4 = decompose("cacheme", plan=_counting_plan)             # use_cache=True (default) -> stores
        assert calls["n"] == 1 and wl4[0]["name"] == "core"
        wl5 = decompose("cacheme", plan=_counting_plan)             # SAME task -> hits cache, no call
        assert calls["n"] == 1 and wl5 == wl4, "cache hit must skip plan() entirely"
        wl6 = decompose("cacheme but different", plan=_counting_plan)   # different task -> miss
        assert calls["n"] == 2
        # una entrada de cache CORRUPTA (editada a mano, ya no valida) no se sirve ciega
        globals()["_WORKLIST_CACHE"].write_text(
            json.dumps({_cache_key("corrupt", None): [{"name": "a", "spec": "a", "file": "x.py"},
                                                       {"name": "b", "spec": "b", "file": "x.py"}]}),
            encoding="utf-8")
        wl7 = decompose("corrupt", plan=_counting_plan)              # dup-file cached entry -> re-plan
        assert calls["n"] == 3 and wl7[0]["name"] == "core", wl7
    finally:
        globals()["_WORKLIST_CACHE"] = _orig_cache

    # 6. pipeline_for: ruteo determinista por op_type; desconocido cae a GENERATE
    assert pipeline_for("Fix the crash in auth")["op_type"] == "REPAIR"
    assert pipeline_for("Fix the crash in auth")["max_fix"] == 1
    assert pipeline_for("Construí un parser de logs")["op_type"] == "GENERATE"
    assert pipeline_for("Construí un parser de logs")["max_depth"] == 2
    assert pipeline_for("Refactor this for speed")["max_depth"] == 1
    p1, p2 = pipeline_for("Fix x"), pipeline_for("Fix x")
    p1["max_fix"] = 99
    assert p2["max_fix"] == 1, "pipeline_for debe devolver copia, no el dict compartido"
    # caso medido 2026-08-03: task de creacion con "validada" suelta en el cuerpo NO debe
    # caer al pipeline VERIFY minimo (fix=1/depth=1) — 2 jobs project-build escalaron por esto.
    pm = pipeline_for("Crear dos funciones nuevas en vault.py\n"
                      "- Referencia de formato validada: _gen_moc_PROTOTYPE.py")
    assert pm["op_type"] == "GENERATE" and pm["max_depth"] == 2, pm

    # 7. decompose_best_of: elige la valida MAS SIMPLE entre N muestras; rotas no rompen;
    # ninguna valida -> path decompose() normal. Cache aislado (mismo patron del bloque 5).
    _orig2 = _WORKLIST_CACHE
    globals()["_WORKLIST_CACHE"] = Path(_tf.mkdtemp()) / "wl_bo_test.json"
    try:
        samples = iter(['not json',
                        '[{"name":"a","spec":"a","file":"a.py"},{"name":"b","spec":"b","file":"b.py"}]',
                        '[{"name":"solo","spec":"todo en uno","file":"solo.py"}]'])
        wl_bo = decompose_best_of("bo-task", n=3, plan=lambda: next(samples))
        assert len(wl_bo) == 1 and wl_bo[0]["name"] == "solo", wl_bo   # gana la mas simple
        def _never_called() -> str:
            raise AssertionError("cache hit: plan no debe correr")
        wl_bo2 = decompose_best_of("bo-task", n=3, plan=_never_called)
        assert wl_bo2 == wl_bo
        bad = iter(['nope', 'nope', 'nope'])   # best-of agota 2, decompose usa la 3ra + reask
        wl_bo3 = decompose_best_of("bo-fallback", n=2, plan=lambda: next(bad),
                                   reask=lambda prev, errs: '[{"name":"x","spec":"x"}]')
        assert wl_bo3[0]["name"] == "x", wl_bo3
    finally:
        globals()["_WORKLIST_CACHE"] = _orig2

    print("project_build F1 OK — validate(DAG), build_order, stub_check(incl the escaped stub), decompose seam, worklist cache, pipeline_for, decompose_best_of")
