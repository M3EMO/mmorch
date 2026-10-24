# SDD fase 2, ticket 05 (delta contra la regresión de suite): Brainstorm / Discovery Notes
Date: 2026-10-23 · Goal: decidir qué hace el SDLC con los tests viejos de un requisito `MODIFIED` o `REMOVED` sin abrir la puerta a borrar tests. Ticket: `.scratch/sdd-fase-2/issues/05-delta-y-suite-total.md`.

Insumos ya decididos: los tests de un `REMOVED` se mueven a `docs/specs/<area>/removidos/` (ticket 02); los tests de caracterización entran a `SNAP` (ticket 03); la etapa 1 recibe la spec del área y nombra tests con el ID final (ticket 04); la trazabilidad del área cita `ruta::test` (ticket 02).

## Summary / key decisions

- El test viejo de un `MODIFIED` se mueve a `removidos/` por código, como un `REMOVED`; el test nuevo de la etapa 1 lo reemplaza (Q1).
- En modo área, un test cubre un solo ID (Q2).
- En modo área, un archivo por ID: `tests/sdlc/<area>/test_R<n>_<slug>.<ext>` (Q3). Enmienda el nombre del ticket 03.
- Un test faltante que no figura en la lista de movimientos es regresión; el coder nunca toca tests viejos (Q4).
- Los movimientos corren al final de la etapa 1, antes de la parada única (Q5).
- La etapa 1 declara la lista de IDs a eliminar con motivo; el `REMOVED` del delta debe coincidir (Q6).
- Test de aceptación aprobado con trece casos (Q7). Ticket 05 resuelto.

## Q&A log

### Q1 — Test viejo de un `MODIFIED`
- Asked: ¿moverlo a `removidos/` por código (A), Claude lo edita en la etapa 1 (B) o queda y el gate ignora su fallo (C)?
- Captured: el usuario elige **A**. El código encuentra el test por `ruta::test` en la tabla de trazabilidad, sin juicio. Nadie edita un test aprobado; la historia queda en `removidos/`. B descartada: un LLM reescribiría un oráculo. C descartada: test rojo para siempre.
- Hecho: mover una función sola es fácil en Python con `ast`; en otros lenguajes, solo el archivo entero (Q3).
- Flags: test que cubre un ID `MODIFIED` y otro sin cambios (Q2).

### Q2 — Test que cubre un ID modificado y otro sin cambios
- Asked: ¿prohibir en modo área que un test cubra más de un ID (A), mover igual y escribir test nuevo para el otro ID (B) o detener para el humano (C)?
- Captured: el usuario elige **A**. En modo área, un test cubre un solo ID; el gate chequea que cada test aparezca en una sola fila de la tabla. Cada movimiento es exacto. B descartada: choca con el caso 14 del ticket 04.
- Flags: mover tests en lenguajes sin `ast` (Q3).

### Q3 — Mover tests en lenguajes sin `ast`
- Asked: ¿mover el archivo entero solo si todos sus tests son del delta (A), un archivo por ID en modo área en todos los lenguajes (B) o marcar `skip` (C)?
- Captured: el usuario elige **B**. En modo área, cada ID tiene su propio archivo de test, en todos los lenguajes: `tests/sdlc/<area>/test_R<n>_<slug>.<ext>`. Cada movimiento es exacto, sin `ast` ni parser por lenguaje. C descartada: tests saltados en la suite contradicen la P4 del ticket 02.
- Captured: enmienda al ticket 03 — el archivo de caracterización deja de ser `tests/test_sdlc_<area>_base_<slug>.py` y pasa a un archivo por ID con el mismo esquema.
- Hecho a cuidar al implementar: pytest con `importmode` por defecto falla si dos archivos de test comparten nombre base sin `__init__.py`; dos áreas con el mismo slug e ID darían el mismo nombre.
- Flags: detectar tests borrados (Q4).

### Q4 — Detectar tests borrados
- Asked: ¿comparar tests recolectados antes y después, con la lista de movimientos como excepción (A), todo el área en `SNAP` (B) o confiar en la aprobación humana (C)?
- Captured: el usuario elige **A**. `pytest --collect-only` lista sin correr; la lista de movimientos sale del delta por código. Un test faltante fuera de esa lista es regresión. Responde la pregunta derivada: el coder de la etapa 4 nunca toca un test viejo.
- Hecho: con un archivo por ID (Q3), otros lenguajes pueden listar archivos en vez de tests, así que el chequeo no depende de un `--collect-only` por framework.
- Flags: en qué momento corren los movimientos (Q5).

### Q5 — Momento de los movimientos
- Asked: ¿al final de la etapa 1, antes de la parada única (A), al comienzo de la etapa 4 (B) o en la etapa 6 (C)?
- Captured: el usuario elige **A**. En la parada se ven juntos el test nuevo y el test viejo que sale (revisión del nivel 6.2).
- Hallazgo de Claude al cruzar con el ticket 04: el delta nace en la etapa 2, después de la parada. Un `MODIFIED` se infiere en la etapa 1 por código (un `test_R<n>` con un ID que ya existe en el área). Un `REMOVED` no tiene test nuevo, así que la etapa 1 no lo conoce (Q6).

### Q6 — Cómo conoce la etapa 1 los IDs eliminados
- Asked: ¿Claude declara en la etapa 1 una lista de IDs a eliminar con motivo y el delta debe coincidir (A), segunda parada en la etapa 2 (B) o movimiento en la etapa 2 sin parada (C)?
- Captured: el usuario pregunta por qué es un problema: "si la etapa 1 es para setear todo no? No es posible tener areas o removed si nunca estuvo esa estructura?". Claude aclara: la primera vez en un área no hay `REMOVED` posible (la etapa 0 recién arma la base); desde la segunda tarea, una baja pura ("quitá el tope de retiro") no tiene test nuevo, así que el código no puede inferirla como infiere un `MODIFIED`. La etapa 0 arma la estructura; la etapa 1 escribe los tests del cambio.
- Captured: el usuario elige **A** ("Pero supongo que A"). Claude declara en la etapa 1 la lista de IDs a eliminar con su motivo; la parada única la muestra; un gate exige que el `REMOVED` del delta coincida con la lista.
- Flags: ninguno.

### Q7 — Test de aceptación
- Asked: ¿aprobás el test con diez casos? Después: ¿falta algo?
- Hecho verificado por Claude en `sdlc/pipeline.py`: `gate_plan_allowlist` rechaza archivos bajo `tests/` en el plan (salvo los `files` del llamador, excepción pensada para `tests/test_capas.py`); `gate_alcance` revierte lo que Claude toca fuera de alcance en la escalera de arreglo. Nadie edita un test viejo sin reversión.
- Captured: el usuario aprueba trece casos: los diez propuestos más un `files` del llamador bajo `tests/sdlc/<area>/` rechazado en modo área, una lista de bajas con un ID inexistente que falla, y una reanudación desde la etapa 2 que no repite los movimientos.
- Flags: ninguno. Ticket 05 resuelto.

## Open flags (pending input)
