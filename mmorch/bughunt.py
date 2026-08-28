"""BUG-HUNTER logico de mmorch: mutation-survivors como mapa de donde un bug silencioso viviria."""

from __future__ import annotations

import difflib
import subprocess
import sys
from collections.abc import Callable
from pathlib import Path

from mmorch import checkers


def module_pairs(repo_dir: str) -> list[tuple[str, str]]:
    """Find (module, test) pairs for mmorch modules with existing tests."""
    repo = Path(repo_dir)
    pairs: list[tuple[str, str]] = []
    modules_dir = repo / "mmorch"
    tests_dir = repo / "tests"
    if not modules_dir.is_dir() or not tests_dir.is_dir():
        return pairs
    for module_file in sorted(modules_dir.glob("*.py")):
        name = module_file.stem
        if name.startswith("_") or name == "__init__":
            continue
        test_file = tests_dir / f"test_{name}.py"
        if test_file.is_file():
            pairs.append((module_file.relative_to(repo).as_posix(), test_file.relative_to(repo).as_posix()))
    return pairs


def _default_run_fn(test_rel: str, *, repo_dir: str, timeout: float) -> bool:
    """Run pytest on the test file, return True if suite passes."""
    result = subprocess.run(
        [sys.executable, "-m", "pytest", "-q", "-x", test_rel],
        cwd=repo_dir,
        timeout=timeout,
        capture_output=True,
        text=True,
    )
    return result.returncode == 0


def survivors_for(
    module_rel: str,
    test_rel: str,
    *,
    repo_dir: str,
    max_mutants: int = 8,
    run_fn: Callable[[str], bool] | None = None,
    timeout: float = 120.0,
) -> dict:
    """Run mutation testing on a module, return survivors info."""
    module_path = Path(repo_dir) / module_rel
    original_code = module_path.read_text(encoding="utf-8")

    if run_fn is None:
        run_fn = lambda test: _default_run_fn(test, repo_dir=repo_dir, timeout=timeout)

    # Precondition: base suite must pass
    if not run_fn(test_rel):
        return {"module": module_rel, "skipped": "suite roja de base"}

    mutants = checkers._mutants(original_code, max_mutants)
    # base normalizada para el diff: ast.unparse reformatea todo (borra blanks/
    # comentarios) -> diffear contra el crudo mostraba el reformateo entero y
    # la mutacion quedaba fuera de las 40 lineas (medido en la 1ra caceria)
    import ast as _ast
    try:
        normalized_base = _ast.unparse(_ast.parse(original_code))
    except SyntaxError:
        normalized_base = original_code
    survived = 0
    survivor_diffs: list[str] = []

    for mutant in mutants:
        try:
            module_path.write_text(mutant, encoding="utf-8")
            try:
                passed = run_fn(test_rel)
            finally:
                module_path.write_text(original_code, encoding="utf-8")
        except Exception:
            # Restore is guaranteed by finally above; continue with next mutant
            continue

        if passed:
            survived += 1
            diff_lines = list(
                difflib.unified_diff(
                    normalized_base.splitlines(keepends=True),
                    mutant.splitlines(keepends=True),
                    fromfile=f"original/{module_rel}",
                    tofile=f"mutant/{module_rel}",
                )
            )
            # Cap at 40 lines
            survivor_diffs.append("".join(diff_lines[:40]))

    return {
        "module": module_rel,
        "mutants": len(mutants),
        "survived": survived,
        "survivor_diffs": survivor_diffs,
    }


def hunt(
    repo_dir: str,
    *,
    modules: list[str] | None = None,
    max_mutants: int = 8,
    review_fn: Callable[[str, list[str]], list[str]] | None = None,
    run_fn: Callable[[str], bool] | None = None,
    isolate: bool = True,
) -> dict:
    """Run bug-hunt across modules, optionally with LLM review.

    `isolate` (default): muta en un worktree efimero, no en el arbol de trabajo
    real. survivors_for escribe cada mutante SOBRE el archivo y lo restaura en
    un finally — una corrida cortada a mitad (Ctrl-C, reboot, timeout del
    nightly) dejaba un modulo de mmorch escrito con un mutante. Ademas el
    round-trip por ast.unparse cambia los line endings del archivo real.
    Mismo aislamiento que ya usan evolve y merge_train.

    Consecuencia a tener presente: el worktree sale de HEAD, asi que se mide
    el codigo COMMITEADO. Un test nuevo sin commitear no cuenta todavia (se
    ve como "el mutante sigue vivo"), igual que en el nightly.
    """
    pairs = module_pairs(repo_dir)
    if modules is not None:
        module_names = {Path(m).stem for m in modules}
        pairs = [p for p in pairs if Path(p[0]).stem in module_names]

    results: list[dict] = []
    findings: list[dict] = []
    errors: list[str] = []
    wt = None
    work_dir = repo_dir
    from mmorch.worktree_driver import is_git_repo, open_worktree
    # sin git no hay arbol versionado que proteger (ni worktree posible): eso es
    # un dir descartable del caller, se muta ahi mismo
    if isolate and pairs and is_git_repo(repo_dir):
        try:
            wt = open_worktree(repo_dir, prefix="mmorch/wt-bughunt")
            wt.seed([".venv"])          # los tests corren con las deps del repo
            work_dir = wt.path
        except Exception as exc:
            # sin worktree preferimos NO mutar el arbol real: el mapa de
            # sobrevivientes es informativo, corromper mmorch no es aceptable
            return {"scanned": 0, "map": [], "findings": [],
                    "errors": [f"worktree: {type(exc).__name__}: {exc}"]}

    try:
        for module_rel, test_rel in pairs:
            try:
                result = survivors_for(
                    module_rel,
                    test_rel,
                    repo_dir=work_dir,
                    max_mutants=max_mutants,
                    run_fn=run_fn,
                )
                results.append(result)
                if result.get("survived", 0) > 0 and review_fn is not None:
                    module_findings = review_fn(module_rel, result["survivor_diffs"])
                    findings.append({"module": module_rel, "findings": module_findings})
            except Exception as exc:
                errors.append(f"{module_rel}: {exc}")
    finally:
        if wt is not None:
            wt.close(keep_branch=False)

    return {"scanned": len(pairs), "map": results, "findings": findings, "errors": errors}


def make_reviewer() -> Callable[[str, list[str]], list[str]]:
    """Create a review function using LLM to identify risky mutants."""

    def review_fn(module_rel: str, survivor_diffs: list[str]) -> list[str]:
        from mmorch.loop_nightly import _llm_json

        prompt = (
            f"Modulo: {module_rel}\n"
            "Estos mutantes sobreviven a la suite de tests — para cada diff decide "
            "si esconde un posible bug logico real o es benigno; incluye SOLO los "
            "riesgosos.\n"
            'Responde EXACTAMENTE un objeto JSON con esta forma (aunque no haya '
            'riesgosos): {"findings": ["<descripcion corta del riesgo>", ...]} — '
            'jamas una lista suelta ni otro shape.\n\n'
            + "\n---\n".join(survivor_diffs)
        )
        schema = {
            "type": "object",
            "properties": {"findings": {"type": "array", "items": {"type": "string"}}},
            "required": ["findings"],
        }
        response = _llm_json(prompt, schema=schema)
        return response.get("findings", [])

    return review_fn
