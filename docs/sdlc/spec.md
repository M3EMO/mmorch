# Spec: Robustez de `call` en `mmorch/providers.py` ante respuestas malformadas de la API

## Contrato

- Python 3.12. Sin dependencias nuevas.
- Único archivo modificado: `mmorch/providers.py`. No se modifican tests ni otros archivos.
- El docstring del módulo `mmorch.providers` se conserva sin cambios.
- Se conserva byte a byte todo `mmorch/providers.py` fuera del manejo de `resp.choices` y `resp.usage` dentro de `call`.
- `mmorch.providers.log_event(**rec: object) -> None`: función existente, no se modifica. Es el mismo mecanismo usado para errores de API.
- `mmorch.providers._client(mk: str) -> object`: función existente, no se modifica. Devuelve un cliente con `.chat.completions.create(**kwargs) -> resp`.
- `mmorch.providers.call(model: str, prompt: str, pattern: str, timeout: int) -> object`: firma existente; no se altera. Acepta `model`, `prompt`, `pattern` y `timeout` posicionales o por nombre. Devuelve un objeto con atributos `text: str`, `in_tokens: int`, `out_tokens: int`.
- La respuesta `resp` usada por `call` expone `choices: list[object] | None` y `usage: object | None`.
- No se agregan imports, clases ni funciones públicas.

## Requisitos

- R1: si `resp.choices` es `None` o vacío (`[]`), `call` levanta `RuntimeError` cuyo mensaje contiene exactamente `'respuesta sin choices'` y el valor de `model`; la detección ocurre antes de indexar `resp.choices[0]` y nunca deja escapar un `TypeError`.
- R2: antes de levantar el `RuntimeError` de R1, `call` registra el fallo con `log_event(...)`; el registro tiene `error='EmptyResponse'`, `error_msg` que contiene `model`, y `error_class='empty_response'`; luego se levanta la excepción.
- R3: si `resp.choices` es válido y `resp.usage is None`, `call` no levanta excepción y usa `in_tokens=0` y `out_tokens=0`; el resto de la llamada sigue su curso normal.

## Casos borde

- R1: `resp.choices is None`.
- R1: `resp.choices == []`.
- R1: `resp.choices` inválido no debe producir `TypeError` por subíndice ni por iteración.
- R2: el registro debe ocurrir para `choices is None` y para `choices == []`.
- R2: `error_msg` debe contener el modelo aun cuando `choices` sea `None`.
- R3: `usage is None` con `choices` no vacío y `message.content` presente.
- R3: `usage` con `prompt_tokens` y `completion_tokens` presentes conserva el comportamiento previo.

## Criterios de éxito

- R1: `PV.call("deepseek-chat", "hola", pattern="t", timeout=5)` con `_Resp(choices=None)` o `_Resp(choices=[])` -> se observa `RuntimeError` que cumple `match="sin choices"` y cuyo mensaje contiene `"deepseek-chat"`.
- R2: misma entrada que R1 -> `events[-1]["error"] == "EmptyResponse"`, `events[-1]["error_class"] == "empty_response"`, y `events[-1]["error_msg"]` contiene `"deepseek-chat"`.
- R3: `PV.call("deepseek-chat", "hola", pattern="t", timeout=5)` con `choices=[SimpleNamespace(message=SimpleNamespace(content="hi"), finish_reason="stop")]` y `usage=None` -> retorno con `r.text == "hi"`, `r.in_tokens == 0`, `r.out_tokens == 0`, sin excepción.

## Trazabilidad

| ID | tests |
|---|---|
| R1 | test_R1_R2_sin_choices_es_runtime_error_registrado |
| R2 | test_R1_R2_sin_choices_es_runtime_error_registrado |
| R3 | test_R3_usage_ausente_no_rompe |

## Clarificaciones

- sin preguntas: la spec cubre el barrido.