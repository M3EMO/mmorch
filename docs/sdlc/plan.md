# Plan: `mmorch/providers.py` — respuesta vacía en `call`

Alcance: único archivo modificado, sin dependencias nuevas, sin tocar tests. Se preserva byte a byte el módulo fuera del manejo de `resp.choices` y `resp.usage` dentro de `call`. El orden interno sugerido para `providers.py`: (1) validar `resp.choices` (`None`/vacío) → `log_event(error='EmptyResponse', error_msg=...)` con el `model` → `RuntimeError('respuesta sin choices: <model>')` antes de cualquier indexado; (2) si `resp.usage is None`, usar `in_tokens=0` y `out_tokens=0`; en caso contrario, conservar la extracción previa (`prompt_tokens`/`completion_tokens`).

## Archivos
- `mmorch/providers.py` [R1, R2, R3] [P]

## Prueba
`python -m pytest tests/test_sdlc_d15_providers_respuesta_vacia.py -q`