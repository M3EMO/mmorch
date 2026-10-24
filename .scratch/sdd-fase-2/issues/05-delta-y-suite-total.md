# Delta contra la regresión de suite
Type: grilling
Status: resolved
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

## Answer

Resuelto por grilling con el usuario (2026-10-23). Registro completo: `brainstorms/2026-10-23-sdd-fase-2-ticket-05.md`.

- El test viejo de un `MODIFIED` se mueve por código a `docs/specs/<area>/removidos/`, igual que un `REMOVED`. El test nuevo de la etapa 1 lo reemplaza. Nadie edita un test aprobado.
- El código encuentra el test viejo por `ruta::test` en la tabla de trazabilidad, sin juicio.
- En modo área, un test cubre un solo ID, y cada ID tiene su propio archivo en todos los lenguajes: `tests/sdlc/<area>/test_R<n>_<slug>.<ext>`. Esta regla enmienda el nombre del ticket 03.
- Un `MODIFIED` se infiere en la etapa 1: un `test_R<n>` nuevo con un ID que ya existe en el área.
- Un `REMOVED` no tiene test nuevo. Claude declara en la etapa 1 la lista de IDs a eliminar, cada uno con su motivo. El `REMOVED` del delta debe coincidir con esa lista.
- Los movimientos corren al final de la etapa 1, antes de la parada única. Ahí se ven juntos los tests nuevos, los que salen y los motivos.
- `gate_suite_total` compara los tests recolectados antes y después. Un test faltante que no figura en la lista de movimientos cuenta como regresión ("test desaparecido").
- El coder de la etapa 4 nunca toca un test viejo. `gate_plan_allowlist` rechaza `tests/` en el plan, y `gate_alcance` revierte lo que Claude toca fuera de alcance.
- Riesgo a cuidar al implementar: pytest falla si dos archivos de test comparten nombre base sin `__init__.py`.

### Test de aceptación aprobado

`tests/test_sdlc_delta.py::test_suite_total_con_delta` usa un área con R1, R2, R4 y R5, cada uno en su archivo:

1. Un `test_R1` nuevo de la etapa 1 mueve el archivo viejo de R1 a `removidos/` antes de la parada.
2. La lista de bajas de la etapa 1 nombra R2 con su motivo. El código mueve el archivo de R2 y suma el motivo al `README.md`.
3. Un delta con un `REMOVED` fuera de la lista de bajas falla.
4. Una baja de la lista que falta en el delta también falla.
5. La suite no cuenta los archivos movidos como fallo ni como test faltante.
6. Un coder que borra el archivo de R4 produce la regresión "test desaparecido".
7. Un fallo nuevo en R5, sin relación con el delta, cuenta como regresión.
8. Un plan que nombra un archivo bajo `tests/` falla `gate_plan_allowlist`.
9. Una tabla que cita el mismo test en dos filas falla.
10. Una corrida sin área deja `gate_suite_total` igual que hoy.
11. Un `files` del llamador bajo `tests/sdlc/<area>/` se rechaza en modo área.
12. Una lista de bajas que nombra un ID inexistente en el área falla.
13. Una corrida reanudada desde la etapa 2 no repite los movimientos de la etapa 1.
