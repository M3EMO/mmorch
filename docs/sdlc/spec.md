# Spec: gate `test-compile` en `build_project`

Fuente de la estructura: spec-kit `templates/spec-template.md` (MIT), recortada a lo que un gate puede verificar.
Cada requisito lleva un ID `R<n>` unico. El gate `trazabilidad` cruza esos IDs con `plan.md` y con los tests.

## Contrato

Unico archivo modificado: `mmorch/project_integrate.py`. Python 3.12, sin dependencias nuevas (`subprocess` es stdlib).

### `mmorch/project_integrate.py`

```python
import subprocess
from mmorch.checkers import CheckResult

_COMPILE_TIMEOUT_S: int = 120

def _default_run_compile(repo: str, timeout: int = _COMPILE_TIMEOUT_S) -> Callable[[str], CheckResult]

def build_project(
    name: str,
    repo: str,
    *,
    external_test: str,
    plan,
    gen,
    run_test,
    propose_test,
    run_snippet,
    commit,
    write_file,
    gen_model: str,
    verifier_model: str,
    integrate=None,
    compile_cmd: str | None = None,
    run_compile=None,
) -> dict
```

`compile_cmd: str | None = None` y `run_compile=None` son keyword-only y se agregan al final de la firma existente; ningun parametro previo cambia de nombre, posicion ni default.

`run_compile` tiene firma `(compile_cmd: str) -> CheckResult` y debe devolver `mmorch.checkers.CheckResult` con `passed: bool`, `detail: str`, `checker: str` y `checker == "test_compile"`.

`_default_run_compile(repo, timeout)` devuelve el callable usado cuando `run_compile is None`:

```python
proc = subprocess.run(compile_cmd, shell=True, cwd=repo, timeout=timeout,
                      capture_output=True, text=True)
detail = (proc.stdout + proc.stderr).strip() or f"returncode={proc.returncode}"
return CheckResult(passed=proc.returncode == 0, detail=detail, checker="test_compile")
```

En `build_project` el gate se aplica dentro de `integrate_fn`, el seam que envuelve a `integrate`:

```python
def integrate_fn(ext, results) -> tuple[bool, str]:
    if compile_cmd is None:
        return integrate(ext, results)
    cr = (run_compile or _default_run_compile(repo))(compile_cmd)
    if not cr.passed:
        return (False, "test-compile: " + cr.detail)
    return integrate(ext, results)
```

`build_project` devuelve un `dict` con al menos las claves `status`, `detail` e `integrated`.

### `mmorch/checkers.py`

Sin cambios. Se consume `CheckResult` ya existente.

## Requisitos

Orden de evaluacion dentro de `build_project`: (1) ciclo unitario (`plan`, `gen`, `run_test`, `propose_test`, `run_snippet`, `write_file`, `commit`), (2) gate `test-compile`, (3) integracion (`external_test` + `integrate`).

- R1: `build_project` acepta `compile_cmd: str | None = None` y `run_compile=None` como keyword-only; con `compile_cmd is None` no se evalua ningun gate y el resultado es identico al comportamiento previo a esta feature.
- R2: el gate `test-compile` se evalua despues del ciclo unitario y antes de invocar `integrate`, y antes del `external_test` de integracion.
- R3: si `compile_cmd is None`, `run_compile` no se invoca ninguna vez, no se construye ningun `CheckResult` de `checker='test_compile'` y el flujo llama a `integrate` como hoy.
- R4: si `compile_cmd` no es `None`, se invoca `run_compile(compile_cmd)` exactamente una vez; si `run_compile is None` se usa `_default_run_compile(repo)`.
- R5: el `CheckResult` del gate decide por `passed`: `passed=False` corta, `passed=True` continua; `checker` del resultado es `'test_compile'` en el camino por default.
- R6: fail-closed: con `compile_cmd` declarado y `passed=False`, `integrate_fn` devuelve `(False, "test-compile: " + cr.detail)` sin invocar `integrate`; el `dict` resultante tiene `status == "integration_failed"` y `detail` que empieza con `"test-compile:"` y contiene el `detail` del `CheckResult`.
- R7: con `compile_cmd` declarado y `passed=True`, `integrate_fn` invoca `integrate(ext, results)` con los mismos argumentos que hoy y el `dict` resultante conserva `status == "built"` e `integrated` verdadero cuando `integrate` devuelve verde.
- R8: el `CheckResult` del gate no se agrega a `results` ni se pasa como argumento extra a `integrate`; la firma de `integrate` no cambia.

## Casos borde

- R3: `run_compile` inyectado y `compile_cmd=None` -> el gate no corre y el callable inyectado queda sin invocar (compatibilidad total).
- R4: `compile_cmd=""` (cadena vacia) cuenta como declarado (`is not None`), el gate corre y decide por el `CheckResult` devuelto.
- R6: `CheckResult(passed=False, detail="")` -> el retorno es exactamente `(False, "test-compile: ")`, con prefijo y sin texto adicional.
- R6: gate rojo -> ni `integrate` ni el `external_test` de integracion se ejecutan (no hay llamada observable).
- R5: el codigo literal de `_default_run_compile` (Contrato, lineas 47-50) no envuelve `subprocess.run` en `try/except`; si `timeout` se agota, `subprocess.TimeoutExpired` se propaga sin capturar y `build_project` no devuelve un `dict` (no hay conversion a `passed=False`). Ningun test de aceptacion ejercita este camino.
- R5: `compile_cmd` declarado y `run_compile` inyectado -> el default no se usa aunque exista (el seam inyectado tiene prioridad).
- R7: gate verde -> el `external_test` de integracion se pasa sin alteraciones y `integrate` recibe el mismo valor que en el flujo sin gate.

## Criterios de exito

- R6: `build_project(..., compile_cmd="python -m compileall .", run_compile=lambda cmd: CheckResult(passed=False, detail="boom en u.py", checker="test_compile"), integrate=spy)` -> `res["status"] == "integration_failed"`, `res["detail"] == "test-compile: boom en u.py"`, `spy` no llamado.
- R7: mismo llamado con `CheckResult(passed=True, detail="ok", checker="test_compile")` -> `res["status"] == "built"`, `bool(res.get("integrated")) is True`, `spy` llamado una vez con `external_test`.
- R3: `build_project(..., integrate=spy)` sin `compile_cmd` -> `res["status"] == "built"`, `spy` llamado una vez con `external_test`.

## Trazabilidad

| ID | tests |
|---|---|
| R1 | test_sin_compile_cmd_no_cambia_nada |
| R2 | test_compile_rojo_corta_antes_de_integrar |
| R3 | test_sin_compile_cmd_no_cambia_nada |
| R4 | test_compile_rojo_corta_antes_de_integrar, test_compile_verde_deja_integrar |
| R5 | test_compile_rojo_corta_antes_de_integrar, test_compile_verde_deja_integrar |
| R6 | test_compile_rojo_corta_antes_de_integrar |
| R7 | test_compile_verde_deja_integrar |
| R8 | test_sin_compile_cmd_no_cambia_nada, test_compile_verde_deja_integrar |

## Clarificaciones

- P1: ¿El `TimeoutExpired` de `subprocess.run` en `_default_run_compile` se captura y se convierte en `passed=False`? | R: no. El contrato literal (lineas 47-50) y la TAREA no piden `try/except`; se corrigio el caso borde de R5 para reflejar que la excepcion se propaga sin capturar (ningun test de aceptacion cubre este camino) | afecta: R5