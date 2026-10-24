# Delta contra la regresión de suite
Type: grilling
Status: open
Blocked by: 04
Map: ../map.md

## Question

¿Qué hace el SDLC con los tests viejos que cubren un requisito `MODIFIED` o `REMOVED`?

Un `MODIFIED R1` cambia un comportamiento a propósito. El test viejo de R1 falla por diseño. `gate_suite_total` compara fallos por nombre contra el baseline y lo cuenta como regresión. Después corre la escalera (coder, Claude) y termina en `ESCALATE_HUMAN`.

Riesgo de reward hacking: si el delta habilita tocar tests viejos, el coder puede borrar un test para pasar el gate. `SNAP` y `gate_baseline` protegen solo los tests de la feature actual. `gate_suite_total` compara solo fallos: un test borrado hoy no cuenta como regresión.

Opciones a decidir:

- A. Lista permitida determinista. El gate de suite ignora solo los fallos de tests que nombran un ID `MODIFIED` o `REMOVED`. El test nuevo de la feature cubre el requisito modificado.
- B. Los tests viejos se reescriben en la etapa 1. La etapa 1 escribe el test nuevo y edita o borra los tests viejos de los IDs del delta. La aprobación humana (`approve_accept`) ve los dos cambios juntos.
- C. Humano siempre. El SDLC corta en `awaiting_approval` y muestra los tests viejos rotos.

Preguntas derivadas:

- ¿Un test que nombra un ID `MODIFIED` y otro sin cambios entra en la lista permitida?
- ¿Los tests de `REMOVED` se borran del repo o quedan marcados `xfail` con el ID?
- ¿El coder de la etapa 4 puede tocar algún test viejo, o eso queda solo para la etapa 1?

## Test de aceptación propuesto

`tests/test_sdlc_delta.py::test_suite_total_con_delta`: un baseline con `test_R1_viejo` verde y un delta `MODIFIED R1`. Tras el cambio, `test_R1_viejo` falla y `test_R2_viejo` también falla. El gate acepta el fallo de R1 según la opción elegida y marca R2 como regresión. Un segundo caso: el coder borra `test_R2_viejo`, y el gate lo detecta como regresión (test desaparecido), no como verde.

## Comments

- 2026-10-23, desde el ticket 02 (decisión del usuario): los tests de un ID `REMOVED` no se borran. Se mueven a `docs/specs/<area>/removidos/`, fuera de la suite. `docs/specs/<area>/removidos/README.md` lista cada ID eliminado. Este ticket decide qué etapa hace el movimiento y qué pasa con los tests de `MODIFIED`.
- 2026-10-23, desde el ticket 03 (aprobado por el usuario): los tests de caracterización `tests/test_sdlc_<area>_base_<slug>.py` entran a `SNAP` tras la aprobación. Un delta que cambia un comportamiento caracterizado los hace fallar. Este ticket decide cómo un `MODIFIED` habilita ese fallo sin abrir la puerta a borrar tests.
