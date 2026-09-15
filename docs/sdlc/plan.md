# plan.md

Alcance: un solo archivo modificado, `mmorch/auto_apply.py`. No se toca ningún test, ni `mmorch/automerge.py`, ni ningún otro archivo. La verificación corre sobre los tests ya existentes: `tests/test_sdlc_d7_automerge_cableado.py` y `tests/test_no_museum.py`.

## Archivos

- `mmorch/auto_apply.py` [R1, R2, R3, R4, R5] [P]

Detalle del único item:

1. **Import a nivel de módulo** (R4): agregar en la cabecera, junto al resto de los imports relativos, `from .automerge import try_automerge`. Nada de imports locales, lazy ni condicionales; sin re-import dentro del cuerpo de `_finish_merge` que sombree el global parcheable. El binding vive en `mmorch.auto_apply` para que `monkeypatch.setattr(mmorch.auto_apply, "try_automerge", fake)` intercepte la llamada.
2. **`_finish_merge` con `merge_sha is None`** (R1): llamar exactamente una vez `try_automerge(str(runtime.path), state['branch'], base=state['base_sha'], source='auto_apply')`. Pasar `branch` y `base_sha` sin transformar; coercionar `runtime.path` con `str()`. Si `r.get('merged')` es True, tomar `r['merge_sha']` como sha del merge y avanzar `status` a `'merged'` y luego a `'observing'`, persistiendo ese `merge_sha` en el `state` retornado.
3. **Rama de rechazo** (R2): evaluar en este orden estricto — (a) carril amarillo acotado: sólo si `state.get('zone') == 'yellow'` **y** `r.get('zone') == 'yellow'`, conservar el merge directo actual (`_git(... 'merge' ...)`) y seguir el flujo normal hasta `'observing'`; (b) en cualquier otro rechazo, `_halt(store, reason=f"automerge {r.get('veredicto')}: {r.get('reason', '')}", expected='candidate')`, dejando `status == 'halted'` y `halt_reason` conteniendo el `veredicto`, sin merge ni advance. Usar `r.get('reason', '')` para tolerar rechazos sin clave `reason` (sin `KeyError`), y no habilitar el carril amarillo cuando `state` no tiene `zone`, cuando `zone` no es `'yellow'` o cuando `r.get('zone')` es `None`.
4. **`merge_sha` provisto** (R3): si `merge_sha is not None` (incluido el string vacío `""`, que cuenta como dado), no invocar `try_automerge` en absoluto: usar el `merge_sha` recibido y llevar el estado a `'observing'` con ese valor exacto (`state['merge_sha'] == ""` en el caso borde).
5. **Higiene de museo** (R5): el import más la llamada en `_finish_merge` son el caller vivo de `try_automerge` dentro de `mmorch/`; no agregar funciones públicas nuevas sin caller ni entradas a `_DEUDA_MUSEO` en `tests/test_no_museum.py` (permanece `set()`; el ratchet sólo puede achicarse). Mantener sin cambios las firmas de `_git(*args) -> str` y `_halt(store, *, reason, expected) -> None`, y el vocabulario de `status` (`'candidate'`, `'merged'`, `'observing'`, `'halted'`). El retorno de `_finish_merge` sigue siendo el `state` resultante.

Secuencia de ejecución: un único archivo, sin dependencias entre items; los cinco cambios se aplican en una sola pasada sobre `mmorch/auto_apply.py`.

## Prueba

`python -m pytest tests/test_sdlc_d7_automerge_cableado.py tests/test_no_museum.py -q`