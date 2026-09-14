# Spec: Validacion al cargar en mmorch/workflow_engine.py

Fuente de la estructura: spec-kit `templates/spec-template.md` (MIT), recortada a lo que un gate puede verificar.

## Contrato

Archivo modificado: `mmorch/workflow_engine.py` (unico archivo tocado). El docstring del modulo se conserva sin cambios.

Firmas y tipos exactos:

```python
_GATES: tuple[str, ...] = ("none", "tests", "verdict")

def validate_steps(steps: list[dict]) -> None
def start_workflow(steps: list[dict], task: str) -> dict
def next_workflow_action(state: dict) -> dict
def submit_workflow(state: dict, block_id: str | None = None, gate_passed: bool | None = None) -> None
```

- `validate_steps(steps)`: valida la estructura completa de `steps`, no devuelve valor (retorna `None`) y levanta `ValueError` en la primera falla que encuentre.
- `start_workflow(steps, task)`: la primera sentencia del cuerpo es `validate_steps(steps)`; recien despues construye y devuelve el `dict` de estado.
- `submit_workflow(state, block_id, gate_passed)`: muta `state` in-place y devuelve `None`.
- `next_workflow_action(state)`: lee `state`, no lo muta, y devuelve un `dict`.
- Estado (`state: dict`): expone al menos `status: str` y `produced: dict[str, str]` (nombre de `produces` -> `block_id`). El `dict` que devuelve `next_workflow_action` expone al menos `role: str`, `consumes: list[str]` y `kind: str`, con `kind == "gate"` para la accion de verificacion de gate.

## Requisitos

- R1: primera regla de `validate_steps`, antes de recorrer steps: `start_workflow(steps, task)` la invoca; si `steps` no es `list` o es `list` vacia levanta `ValueError` cuyo mensaje contiene la palabra `steps`.
- R2: por cada step en orden de indice, y despues de R4 y antes de R3 para ese mismo step, sea `g = step.get("gate", "none")`; si `g not in _GATES` levanta `ValueError` cuyo mensaje contiene `gate` y el valor de `g`.
- R3: ultima regla por step; si el step tiene la clave `"loop_back"`, sea `i` su indice 0-based en `steps`; si `loop_back < 0` o `loop_back >= i` levanta `ValueError` cuyo mensaje contiene `loop_back`.
- R4: por cada step en orden de indice, y antes de R2 y R3 para ese mismo step, `step.get("role")` debe ser un `str` no vacio; en caso contrario levanta `ValueError` cuyo mensaje contiene `role`.
- R5: `submit_workflow(state)` en fase `produce` con `block_id is None` levanta `ValueError` cuyo mensaje contiene `block_id`.
- R6: un workflow valido se comporta exactamente como antes del cambio y el self-check del bloque `__main__` sigue pasando.

## Casos borde

- R1: `steps` vacio (`[]`); `steps` no-`list` (`None`, `str`, `dict`, `tuple`) -> mismo `ValueError` mencionando `steps`.
- R2: `gate` ausente se normaliza a `"none"` (valido); `gate` presente fuera de `_GATES` (`"vibes"`, `None`, `1`) -> `ValueError`; comparacion case-sensitive, sin alias.
- R3: `loop_back` en el step `i = 0` siempre es invalido (no existe indice previo); `loop_back == i` es invalido (debe ser estrictamente menor que `i`); `loop_back` ausente es valido.
- R4: `role` ausente, `role == ""`, `role == "   "` o `role` no-`str` -> `ValueError`; `str` con contenido no vacio es valido.
- R5: `block_id is None` dispara el error; `block_id == ""` no lo dispara.
- R6: la secuencia `architect` -> `coder` (consume `"P1"`) -> `gate` -> `done` se preserva sin cambios.

## Criterios de exito

- R1: `start_workflow([], "t")` -> `ValueError` con `"steps"` en `str(exc)`.
- R2: `start_workflow([{"role": "coder", "produces": "code", "gate": "vibes"}], "t")` -> `ValueError` con `"gate"` y `"vibes"` en `str(exc)`.
- R3: `start_workflow([{"role": "coder", "produces": "code", "gate": "tests", "test_cmd": "x", "loop_back": 5}], "t")` -> `ValueError` con `"loop_back"`; idem con `loop_back == -1`.
- R4: `start_workflow([{"produces": "code", "gate": "none"}], "t")` -> `ValueError` con `"role"` en `str(exc)`.
- R5: `st = start_workflow(_ok_steps(), "t")`; `submit_workflow(st)` -> `ValueError` con `"block_id"` en `str(exc)`.
- R6: con `_ok_steps()`, `next_workflow_action(st)["role"] == "architect"`; tras `submit_workflow(st, block_id="P1")`, `next_workflow_action(st)["consumes"] == ["P1"]`; tras `submit_workflow(st, block_id="C1")`, `next_workflow_action(st)["kind"] == "gate"`; tras `submit_workflow(st, gate_passed=True)`, `st["status"] == "done"` y `st["produced"]["code"] == "C1"`.

## Trazabilidad

| ID | tests |
|---|---|
| R1 | test_R1_steps_vacios |
| R2 | test_R2_gate_desconocido |
| R3 | test_R3_loop_back_fuera_de_rango |
| R4 | test_R4_step_sin_role |
| R5 | test_R5_submit_produce_sin_block_id |
| R6 | test_R6_camino_feliz_intacto |

## Clarificaciones

- sin preguntas: la spec cubre el barrido.