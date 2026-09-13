# plan.md

## Archivos
- `mmorch/project_integrate.py` [R1, R2, R3, R4, R5, R6, R7, R8] [P]

## Prueba
`python -m pytest tests/test_sdlc_d2_test_compile.py -q`

---

Notas de implementación (no forman parte del contrato de `plan.md`, solo contexto del único archivo):

- Agregar `import subprocess` y `from typing import Callable` si faltan; consumir `CheckResult` desde `mmorch.checkers` sin modificarlo (R8).
- Definir `_COMPILE_TIMEOUT_S: int = 120` y `_default_run_compile(repo, timeout=_COMPILE_TIMEOUT_S)` con el cuerpo literal del Contrato: `subprocess.run(..., shell=True, cwd=repo, timeout=timeout, capture_output=True, text=True)`, `detail = (stdout+stderr).strip() or f"returncode={rc}"`, `CheckResult(passed=rc==0, detail=detail, checker="test_compile")`. Sin `try/except` (R5, P1: `TimeoutExpired` se propaga).
- En `build_project`, añadir `compile_cmd: str | None = None` y `run_compile=None` como keyword-only al final de la firma; ningún parámetro previo cambia de nombre/posición/default (R1).
- Envolver `integrate` en `integrate_fn(ext, results)` con el orden: `compile_cmd is None` → delegar directo a `integrate` sin tocar `run_compile` (R2, R3); con `compile_cmd` declarado (`is not None`, cubre `""`) → invocar `(run_compile or _default_run_compile(repo))(compile_cmd)` una sola vez, decidir por `cr.passed`, y en rojo devolver `(False, "test-compile: " + cr.detail)` (R4, R5, R6). En verde, delegar a `integrate(ext, results)` con los mismos argumentos (R7) y sin filtrar el `CheckResult` a `results` ni a `integrate` (R8).
- El gate se ejecuta después del ciclo unitario y antes del `external_test` de integración; con rojo ni `integrate` ni el `external_test` se invocan (R2, R6).