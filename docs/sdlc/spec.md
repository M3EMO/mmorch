# Spec: Cableo de `schedule` y `effort` en `mmorch/providers.py`

Fuente de la estructura: spec-kit `templates/spec-template.md` (MIT), recortada a lo que un gate puede verificar.
Cada requisito lleva un ID `R<n>` unico. El gate `trazabilidad` cruza esos IDs con `plan.md` y con los tests.

## Contrato

- Path: `mmorch/providers.py`
- Imports a nivel de módulo:
  - `from .effort import model_for_effort`
  - `from . import schedule`
- Función: `def call(model_key: str, prompt: str, *, pattern: str, effort: str | None = None) -> Any:`
  - Al inicio de `call`, si `effort is not None`, el modelo efectivo es `model_for_effort(effort)`; en caso contrario, `model_key`.
  - El modelo efectivo reemplaza a `model_key` en todas las operaciones de `call`: búsqueda de spec, creación del cliente, cálculo de costo y registro (`log_event`).
  - Cada llamada a `log_event` dentro de `call` incluye en su dict `extra` la clave `off_peak` con el valor de `schedule.is_off_peak()`. Si el registro no tiene `extra`, se crea `{'off_peak': schedule.is_off_peak()}`.

## Requisitos

- R1: Si `effort` no es `None`, el modelo efectivo es `model_for_effort(effort)` y reemplaza a `model_key` desde el inicio de `call`; spec, cliente, costo y registro usan el modelo efectivo. Orden de evaluación: (1) recibir `effort`; (2) si `effort is not None`, calcular `model_for_effort(effort)` y asignarlo a la variable de modelo efectivo; (3) en caso contrario, usar `model_key`; (4) usar el modelo efectivo en todas las operaciones posteriores.
- R2: Si `effort` es `None`, `call` se comporta exactamente como antes: el modelo efectivo es `model_key` y no se invoca `model_for_effort`.
- R3: Cada `log_event` dentro de `call` (éxito, error de API, budget_cap, breaker_open) incluye en su dict `extra` la clave `off_peak` con el bool retornado por `schedule.is_off_peak()`; si `extra` no existe en ese registro, se crea como `{'off_peak': schedule.is_off_peak()}`. La llamada a `schedule.is_off_peak()` se hace en el momento de construir cada evento.

## Casos borde

- R1: `effort` con valor no reconocido: `model_for_effort` decide el modelo; `call` no valida ni lanza por sí mismo.
- R1: `model_key` y `effort` presentes: `effort` gana siempre.
- R2: `effort=None` explícito o ausente: idéntico a la versión sin `effort`.
- R3: `schedule.is_off_peak()` devuelve `True` o `False`; el valor se captura en el momento de cada `log_event`, no al inicio de `call`.
- R3: registro con `extra` preexistente: se agrega `off_peak` sin sobreescribir otras claves.
- R3: error de API: el registro de error también lleva `off_peak`.

## Criterios de exito

- R1: `call("deepseek-chat", "hola", pattern="t", effort="high")` -> el último registro del ledger tiene `model == "deepseek-v4-pro"`.
- R2: `call("deepseek-chat", "hola", pattern="t")` -> el último registro del ledger tiene `model == "deepseek-chat"`.
- R3: con `schedule.is_off_peak` parcheado a `True`, `call("deepseek-chat", "hola", pattern="t")` -> el último registro tiene `extra["off_peak"] is True`. Con `False`, `extra["off_peak"] is False`.
- R3: con `schedule.is_off_peak` parcheado a `True` y cliente que lanza `RuntimeError`, `call("deepseek-chat", "hola", pattern="t")` -> el último registro tiene `extra["off_peak"] is True`.

## Trazabilidad

| ID | tests |
|---|---|
| R1 | test_R1_effort_elige_el_modelo |
| R2 | test_R2_sin_effort_no_cambia |
| R3 | test_R3_off_peak_en_el_ledger, test_R3b_off_peak_tambien_en_error |

## Clarificaciones

- sin preguntas: la spec cubre el barrido.