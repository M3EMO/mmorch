# supervision.md — driver_py


## candidato 18:32:48
revision de spec (Claude, rc=0): Hice 1 pregunta.

## candidato 18:41:29
reasoner 1: {"files": ["mmorch/project_integrate.py"], "instructions": {"mmorch/project_integrate.py": "El gate `test-compile` ya está bien (parámetros `compile_cmd`/`run_compile`, `_default_compile`, `_make_compile_gated_integrate` y el wrapper fail-closed antes de `integrate_fn`): NO lo toques. El fallo es que `build_project` reenvía a `run_project_build` los seams legacy inyectados por los tests, que el driver ya no acepta. En `build_project`, junto al bloque que ya hace `if \"gen\" in kwargs: kwargs.pop(\"gen\")`, elimina también de `kwargs` los demás seams legacy que solo vienen de los tests (`run_test`, `propose_test`, `run_snippet`, `write_file`) antes de la llamada a `run_project_build`, p.ej. con un bucle `for _legacy in (\"gen\", \"run_test\", \"propose_test\", \"run_snippet\", \"write_file\"): kwargs.pop(_legacy, None)`. Así dejan de propagarse y desaparece el `TypeError: run_project_build() got an unexpected keyword argument 'run_test'`. Con `compile_cmd` el wrapper debe seguir cortando con `(False, 'test-compile: ' + detail)` antes de llamar al `integrate` original, y sin `compile_cmd` el comportamiento no cambia."}}

## candidato 18:42:36
reasoner 2: {"files": ["mmorch/project_integrate.py"], "instructions": {"mmorch/project_integrate.py": "El único fallo es la firma con la que se invoca al driver: en build_project la llamada final es `run_project_build(repo, spec, plan_fn=..., ...)`, pasando `spec` como 2º argumento posicional, pero `run_project_build` solo acepta 1 argumento posicional (`repo`) y `spec` es keyword-only, de ahí el `TypeError: run_project_build() takes 1 positional argument but 2 were given`. Cambia esa llamada para pasar `spec` por nombre: `run_project_build(repo, spec=spec, plan_fn=plan_fn or decompose, build_fn=..., gate_fn=..., integrate_fn=chosen_integrate, commit_fn=..., **kwargs)`, dejando intacto el resto del cableado del gate `test-compile` (`chosen_integrate`, `_make_compile_gated_integrate`, `_default_compile`, parámetros `compile_cmd`/`run_compile`)."}}

## candidato 18:46:52
nivel 3 Claude (rc=0): El archivo estaba corrupto. Una edición previa borró la firma real de `build_project`. Esa firma no coincidía con `run_project_build`.

Restauré la versión correcta del archivo. Agregué el gate `test-compile`. Sumé los parámetros `compile_cmd` y `run_compile`. Sumé `_default_compile()`: corre `compile_cmd` con subprocess, checker `test_compile`. `integrate_fn` corta antes de llamar `integrate` si el compile da rojo. Esto es fail-closed.

`pytest tests/test_sdlc_d2_test_compile.py -q` pasa 3 de 3 tests.

## candidato 18:49:17
revision del diff (Claude, rc=0): BLOCK: `_default_compile` envuelve `subprocess.run` en try/except y captura `subprocess.TimeoutExpired`. Devuelve `CheckResult(False, "TIMEOUT", "test_compile")`. El spec (R5, clarificación P1) prohíbe ese try/except. Un timeout debe propagarse sin capturar. El diff invierte ese contrato: fail-closed-por-excepción pasa a fail-closed-silencioso.

## candidato 18:52:49
reasoner 1: {"files": ["mmorch/project_integrate.py"], "instructions": {"mmorch/project_integrate.py": "Fix del test fallido: en `_default_compile` elimina TODO el try/except (tanto el `except subprocess.TimeoutExpired` que devuelve CheckResult(False, 'TIMEOUT', 'test_compile') como el `except Exception` genérico, que igual atraparía TimeoutExpired por ser subclase de Exception). Deja solo: `p = subprocess.run(compile_cmd, cwd=repo, shell=True, capture_output=True, text=True, encoding='utf-8', errors='replace', timeout=_COMPILE_TIMEOUT_S)` seguido de `return CheckResult(p.returncode == 0, (p.stdout + p.stderr)[-2000:], 'test_compile')`. Así, cuando `_COMPILE_TIMEOUT_S` se supera, `subprocess.TimeoutExpired` se propaga sin capturarse (contrato R5/P1: nunca convertirlo en CheckResult(passed=False)). El resto de la lógica del gate test-compile en `integrate_fn` y los demás tests quedan igual."}}

## candidato 18:53:37
reasoner 2: ```json
{
  "files": ["mmorch/project_integrate.py"],
  "instructions": {
    "mmorch/project_integrate.py": "En `_default_compile`, elimina COMPLETAMENTE el `try/except` que envuelve a `subprocess.run`: quita tanto el `except subprocess.TimeoutExpired` (que devuelve `CheckResult(False, 'TIMEOUT', 'test_compile')`) como el `except Exception` (que devuelve `CheckResult(False, str(e)[:300], 'test_compile')`), de modo que `subprocess.TimeoutExpired` (y cualquier otra excepcion de `subprocess.run`) se propague sin capturarse, tal como exige el contrato R5/P1 del spec. Deja el cuerpo de la funcion como: `p = subprocess.run(compile_cmd, cwd=repo, shell=True, capture_output=True, text=True, encoding='utf-8', errors='replace', timeout=_COMPILE_TIMEOUT_S)` seguido de `return CheckResult(p.returncode == 0, (p.stdout + p.stderr)[-2000:], 'test_compile')`. No cambies nada mas: la firma `_default_compile(repo, compile_cmd)`, el parametro `compile_cmd`/`run_compile` en `build_project` y el gate en `integrate_fn` (que sigue corriendo el gate antes de `integrate` y fallando cerrado con `'test-compile: ' + detail`) ya satisfacen los tests de aceptacion por inyeccion de `run_compile`. No toques tests ni otros archivos."
  }
}
```

## candidato 18:55:03
nivel 3 Claude (rc=0): 4 passed (el error de atexit es un cleanup de pytest en Windows, no del test).

Quité el `try/except TimeoutExpired` en `_default_compile` para que la excepción se propague sin capturar.

## candidato 19:00:12
nivel 3 Claude (rc=0): Corrijo la oración larga.

Cambié `mmorch/project_integrate.py`: reemplacé dos lambdas del self-check por la función `_integrate_spy`. Esto elimina los 2 errores de mypy `func-returns-value`. El gate `test-compile` ya estaba implementado.

## candidato 19:05:53
ESCALATE_HUMAN: aceptacion verde pero suite total con regresion
fallos nuevos: ['tests/test_providers.py::test_call_api_failure_logs_error_and_reraises', 'tests/test_providers.py::test_call_passes_timeout_and_maxtokens', 'tests/test_providers.py::test_call_success_logs_metric']

## candidato 19:13:16
revision del diff (Claude, rc=0): El test nuevo falla contra el diff. Confirma el defecto. El test existente pasa. El error de pathlib al final es cleanup de pytest en Windows, no afecta el resultado.

BLOCK: `_default_compile` no aplica `.strip()` ni el fallback del contrato. El contrato exige `f"returncode={rc}"` cuando no hay output. Con `compile_cmd` que falla sin output, `detail` queda vacío: `"test-compile: "` en vez de `"test-compile: returncode=1"`.
