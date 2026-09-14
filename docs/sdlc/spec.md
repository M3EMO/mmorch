# Spec: Robustez de `run_project_build` en `mmorch/project_driver.py`

Fuente de la estructura: spec-kit `templates/spec-template.md` (MIT), recortada a lo que un gate puede verificar.

## Contrato

Path modificado (único): `mmorch/project_driver.py`. No se tocan tests ni otros archivos. El docstring del módulo se conserva textualmente.

Tipos de los seams:

```python
PlanFn      = Callable[[str, str | None], list[dict]]              # (task, external_test) -> unidades
BuildFn     = Callable[[dict], str]                                # (unit) -> codigo fuente
GateFn      = Callable[[dict, str], tuple[bool, str]]              # (unit, code) -> (ok, detalle)
CommitFn    = Callable[[str, dict], None] | None                   # (name, unit_result) -> None
IntegrateFn = Callable[[str | None, list[dict]], object] | None    # (external_test, results) -> object
```

Función pública:

```python
def run_project_build(
    task: str,
    external_test: str | None = None,
    plan_fn: PlanFn = ...,
    build_fn: BuildFn = ...,
    gate_fn: GateFn = ...,
    commit_fn: CommitFn | None = ...,
    integrate_fn: IntegrateFn | None = ...,
) -> dict[str, Any]
```

Forma del retorno por `status`:

- `'escalate'`: `{'status': 'escalate', 'reason': str, 'task': str}`
- `'integration_failed'`: `{'status': 'integration_failed', 'depth': int, 'external_test': str | None, 'detail': str, 'results': list[dict]}`
- `'built'` (camino sin fallos de seams): `{'status': 'built', 'results': list[dict], ...}`; cada elemento de `results` lleva `'status'` y, si `commit_fn` levantó para esa unidad, además `'commit_error': str`.

La recursión de sub-builds invoca `run_project_build` (misma función, mismo contrato de seams), de modo que el comportamiento de R1–R3 se hereda en cualquier `depth`.

## Requisitos

- R1: si `plan_fn(task, external_test)` levanta una excepción `e`, `run_project_build` no propaga la excepción y retorna `{'status': 'escalate', 'reason': f'planner failed: {type(e).__name__}: {str(e)[:200]}', 'task': task[:80]}`. Orden de evaluación: `plan_fn` es el primer seam invocado; ninguna llamada a `build_fn`, `gate_fn`, `commit_fn` ni `integrate_fn` ocurre después de la excepción.
- R2: por cada unidad, `commit_fn(name, unit_result)` se invoca después de que la unidad quedó `'built'` y de que su dict de resultado fue agregado a `results`. Si `commit_fn` levanta `e`, la excepción no se propaga, el `'status'` de esa unidad permanece `'built'`, se agrega a su dict la clave `'commit_error'` con `f'{type(e).__name__}: {str(e)[:200]}'`, y el loop continúa con la unidad siguiente sin abortar el build.
- R3: si `integrate_fn(external_test, results)` levanta una excepción `e`, `run_project_build` no propaga la excepción y retorna `{'status': 'integration_failed', 'depth': depth, 'external_test': external_test, 'detail': f'integrate_fn {type(e).__name__}: {str(e)[:200]}', 'results': results}`, con `results` ya completo al momento del fallo y `depth` el de la invocación actual.

## Casos borde

- R1: `str(e) == ''` -> `reason == 'planner failed: ValueError: '`; `str(e)` de más de 200 caracteres -> truncado a 200; `task` de más de 80 caracteres -> `'task'` truncado a 80; `task == ''` -> `'task': ''`; `external_test is None` -> se pasa `None` a `plan_fn`.
- R2: `str(e)` de más de 200 caracteres -> truncado a 200 en `'commit_error'`; `commit_fn is None` -> no se invoca y no aparece `'commit_error'`; fallo en la última unidad de la lista -> el build igual termina en `'built'`; `'commit_error'` se agrega solo al dict de la unidad que falló, no al de las demás.
- R3: `external_test is None` -> `'external_test': None` en el retorno; `str(e)` de más de 200 caracteres -> truncado a 200; `results` vacío -> `'results': []`; excepción en un sub-build (recursión) -> el mismo dict de `'integration_failed'` se produce en el nivel donde `integrate_fn` falló.

## Criterios de éxito

- R1: entrada `task='t'`, `external_test=None`, `plan_fn` que levanta `ValueError("json roto")`, `build_fn` y `gate_fn` que devuelven OK -> salida `{'status': 'escalate', 'reason': 'planner failed: ValueError: json roto', 'task': 't'}`.
- R2: entrada `task='t'`, `external_test=None`, `plan_fn` que devuelve `[{'name': 'u', 'spec': 's', 'file': 'u.py', 'deps': [], 'test_cmd': None}]`, `build_fn` que devuelve `'def f():\n    return 1\n'`, `gate_fn` que devuelve `(True, 'ok')`, `commit_fn` que levanta `OSError("disco lleno")` -> salida con `status == 'built'`, `results[0]['status'] == 'built'` y `'disco lleno' in results[0]['commit_error']`.
- R3: entrada `task='t'`, `external_test='ACCEPT'`, `plan_fn` que devuelve la unidad de R2, `build_fn`/`gate_fn` OK, `integrate_fn` que levanta `RuntimeError("pytest explota")` -> salida con `status == 'integration_failed'` y `detail.startswith('integrate_fn RuntimeError: pytest explota')`.

## Trazabilidad

| ID | tests |
|---|---|
| R1 | test_R1_plan_fn_que_levanta_escala_con_motivo |
| R2 | test_R2_commit_fn_que_levanta_no_frena_el_build |
| R3 | test_R3_integrate_fn_que_levanta_es_integration_failed |

## Clarificaciones

- sin preguntas: la spec cubre el barrido.