# Spec: cableado de `try_automerge` en `auto_apply._finish_merge`

Fuente de la estructura: spec-kit `templates/spec-template.md` (MIT), recortada a lo que un gate puede verificar.
Cada requisito lleva un ID `R<n>` unico. El gate `trazabilidad` cruza esos IDs con `plan.md` y con los tests.

## Contrato

Path modificado, unico: `mmorch/auto_apply.py`. No se toca ningun test, ni `mmorch/automerge.py`, ni cualquier otro archivo.

### `mmorch/automerge.py` (sin cambios; solo se importa)

```python
def try_automerge(repo: str, branch: str, *, base: str, source: str = "") -> dict: ...
```

Retorna un dict con claves: `merged: bool`, `zone: str`, `veredicto: str`, `merge_sha: str | None`, `checks: list`, y opcionalmente `reason: str`.

### `mmorch/auto_apply.py`

Import a nivel de modulo (no local, no lazy, no condicional):

```python
from .automerge import try_automerge
```

Funcion modificada, firma exacta:

```python
def _finish_merge(
    store: PromotionStore,
    runtime: Runtime,
    *,
    now: float,
    merge_sha: str | None = None,
) -> dict: ...
```

- `store`: `mmorch.promotion.PromotionStore` (real en tmp en los tests).
- `runtime`: expone `.path: str`, `.clean() -> bool`, `.head() -> str`.
- `state`: dict del store con claves `branch`, `base_sha`, `zone`, `status`, `merge_sha`, `halt_reason`.
- Retorno: el `state` resultante.
- Helpers reutilizados sin cambio de firma: `_git(*args) -> str` y `_halt(store, *, reason: str, expected: str) -> None`.
- Vocabulario de `status`: `'candidate'`, `'merged'`, `'observing'`, `'halted'`.

## Requisitos

- R1: En `_finish_merge`, cuando `merge_sha` es None se llama exactamente una vez a `try_automerge(str(runtime.path), state['branch'], base=state['base_sha'], source='auto_apply')`. Si el resultado trae `merged` True, se usa su `merge_sha` como sha del merge y el estado avanza a `'merged'` y luego a `'observing'` con ese `merge_sha`.
- R2: Si el resultado trae `merged` False, el orden de evaluacion es: primero el carril amarillo acotado y despues el halt. Carril amarillo acotado: si `state.get('zone') == 'yellow'` y `r.get('zone') == 'yellow'`, se conserva el merge directo actual (`_git(... 'merge' ...)`) y la promocion sigue el flujo normal hasta `'observing'`. En cualquier otro rechazo se llama `_halt(store, reason=f"automerge {r.get('veredicto')}: {r.get('reason', '')}", expected='candidate')`, el estado queda `'halted'` con `halt_reason` conteniendo el `veredicto`, y no hay merge ni advance.
- R3: Si `merge_sha` no es None, `try_automerge` no se invoca en absoluto (ni por efecto de import, ni por referencia diferida): se usa el `merge_sha` recibido y el estado llega a `'observing'` con ese valor.
- R4: `try_automerge` queda ligado al namespace del modulo `mmorch.auto_apply` por el import a nivel de modulo, de modo que `monkeypatch.setattr(mmorch.auto_apply, "try_automerge", fake)` intercepta la llamada que hace `_finish_merge`.
- R5: El cableado deja a `try_automerge` con caller vivo dentro de `mmorch/` (el import y la llamada en `_finish_merge`), sin agregar funciones publicas nuevas sin caller y sin agregar entradas a `_DEUDA_MUSEO` en `tests/test_no_museum.py`.

## Casos borde

- R1: `state['base_sha']` y `state['branch']` se pasan como `base` y `branch` sin transformar; `runtime.path` se coerciona con `str()` antes de pasarlo como `repo`.
- R2: rechazo con `veredicto` presente pero sin clave `reason` -> `reason` del halt es `"automerge <veredicto>: "` (sin excepcion de `KeyError`); `state` sin clave `zone` (o `zone` distinta de `'yellow'`) nunca habilita el carril amarillo acotado; resultado con `zone` `None` tampoco.
- R3: `merge_sha` string vacio `""` cuenta como dado (no es None): no se llama al semaforo y `state['merge_sha']` queda `""`.
- R4: el import a nivel de modulo ocurre una sola vez al importar `mmorch.auto_apply`; no hay `import` de `automerge` dentro del cuerpo de `_finish_merge` que sombree el global parcheado.
- R5: `_DEUDA_MUSEO` permanece vacio (`set()`), el ratchet solo puede achicarse.

## Criterios de exito

- R1: store con `zone='green'`, `branch='candidate'`, `base_sha='base'`, `merge_sha=None`; `try_automerge` -> `{"merged": True, "zone": "green", "veredicto": "merged", "merge_sha": "abc123", "checks": []}` => `calls == [("candidate", "base", "auto_apply")]`, `state["status"] == "observing"`, `state["merge_sha"] == "abc123"`.
- R2: store con `zone='green'`; `try_automerge` -> `{"merged": False, "zone": "red", "veredicto": "rechazado_red", "reason": "path rojo: GOAL.md", "merge_sha": None, "checks": []}` => `state["status"] == "halted"` y `"rechazado_red" in state.get("halt_reason", "")`.
- R3: `_finish_merge(store, runtime, now=2.0, merge_sha="ya-mergeado")` con `try_automerge` que lanza `AssertionError` => sin excepcion, `state["status"] == "observing"`, `state["merge_sha"] == "ya-mergeado"`.
- R4: `import mmorch.auto_apply as AA` => `AA.try_automerge` existe y es el objeto importado de `mmorch.automerge`; tras `monkeypatch.setattr(AA, "try_automerge", fake)`, `_finish_merge` ejecuta `fake`.
- R5: `tests/test_no_museum.py::test_cero_funciones_publicas_sin_caller_vivo` y `::test_deuda_museo_no_miente` pasan con `museo == {}` y `_DEUDA_MUSEO == set()`.

## Trazabilidad

| ID | tests |
|---|---|
| R1 | test_R1_finish_merge_usa_try_automerge_en_verde |
| R2 | test_R2_rechazo_del_semaforo_detiene_la_promocion |
| R3 | test_R3_merge_sha_conocido_no_llama_al_semaforo |
| R4 | test_R1_finish_merge_usa_try_automerge_en_verde |
| R5 | test_cero_funciones_publicas_sin_caller_vivo, test_deuda_museo_no_miente |

## Clarificaciones

- sin preguntas: la spec cubre el barrido.