# SDD fase 2, ticket 03 (spec base de un área sin spec): Brainstorm / Discovery Notes
Date: 2026-10-23 · Goal: decidir cómo nace la spec del comportamiento actual de un área declarada que todavía no tiene `docs/specs/<area>/spec.md`. Ticket: `.scratch/sdd-fase-2/issues/03-spec-base-brownfield.md`.

Insumos: ticket 01 (spec por área, áreas por globs en `sdlc.toml`, adopción voluntaria) y ticket 02 (IDs locales al área, menor número libre, trazabilidad `ruta::test`, tests eliminados a `removidos/`).

## Summary / key decisions

- Etapa nueva "0-caracterizar" (opción A): un LLM escribe la spec del comportamiento actual del área y tests de caracterización que pasan en HEAD.
- Claude escribe los tests de caracterización; el writer por API escribe la spec desde esos tests (Q2).
- La caracterización bloquea con cero mutantes muertos; `caracterizacion_min` opcional por repo (Q3).
- La caracterización cubre solo los archivos de la tarea que la spec del área todavía no nombra en Contrato (Q4).
- La caracterización se detiene para aprobación humana con `approve_accept` (Q5).
- Etapas 0 y 1 seguidas y una sola parada para aprobar los dos tests; después, spec base y delta (Q6).
- Cada caracterización deja `tests/test_sdlc_<area>_base_<slug>.py`; test de aceptación aprobado con trece casos (Q7). Ticket 03 resuelto.

## Q&A log

### Q1 — Cómo nace la spec base
- Asked: ¿etapa 0-caracterizar con LLM (A), base manual del usuario (B) o primera feature escribe spec completa (C)?
- Captured: el usuario elige **A**. Alineado con el nivel 6.1 del curso. Oráculo de los tests: pasan en HEAD y matan mutantes del área (`mmorch.checkers._mutants`). C descartada: spec parcial, `MODIFIED` imposible sobre lo no descrito.
- Flags: quién escribe tests y spec (Q2).

### Q2 — Quién escribe tests y spec
- Asked: ¿Claude los tests y el writer la spec (A), todo Claude (B) o todo el writer por API (C)?
- Captured: el usuario elige **A**. Mismo reparto que hoy: etapa 1 (Claude por CLI, `_revisor`, lee el repo y corre pytest) y etapa 2 (writer por API, `llm(WRITER, ...)`, spec desde tests fijos).
- Flags: piso de mutantes para aceptar la caracterización (Q3).

### Q3 — Piso de mutantes
- Asked: ¿bloquear solo con cero mutantes muertos y observar el puntaje (A), mínimo `caracterizacion_min` (B), o cada test mata uno propio (C)?
- Captured: el usuario elige **A con B**. Bloquea si los tests no matan ningún mutante (piso sin juicio); el puntaje queda en el run-log. Un repo puede fijar `caracterizacion_min` en `sdlc.toml`, con el esquema de `mutation_min`. C descartada por costo (tema del ticket 09).
- Flags: alcance de la caracterización (Q4).

### Q4 — Alcance de la caracterización
- Asked: ¿área entera (A), solo los archivos de la tarea y se suman en tareas siguientes (B), o solo funciones tocadas (C)?
- Captured: el usuario elige **B**. La spec del área crece archivo por archivo, al ritmo de los cambios (nivel 6.1). La sección Contrato nombra cada ruta cubierta; un archivo de la tarea que no figura ahí se caracteriza antes del delta. A descartada (spec enorme, nivel 6.6); C necesita el plan (etapa 3).
- Flags: el usuario pregunta "¿Cómo definimos las áreas?" -> esquema concreto en `sdlc.toml` y quién propone las primeras áreas (ticket 01, Q15 en su brainstorm).

### Q5 — Aprobación humana de la caracterización
- Asked: ¿parada tras la etapa 0 con `approve_accept` (A), clave propia `approve_base` (B) o sin aprobación (C)?
- Captured: el usuario elige **A**. Los mutantes prueban que el test mide algo, no que ese algo sea deseado; un bug fijado como requisito queda "correcto" para los deltas siguientes. Una sola clave. La parada ocurre una vez por archivo caracterizado.
- Flags: orden de etapas y cantidad de paradas (Q6).

### Q6 — Una parada o dos
- Asked: ¿dos paradas (A), etapas 0 y 1 seguidas con una sola parada (B) o etapa 0 como corrida propia (C)?
- Captured: el usuario elige **B**. Orden: etapa 0 (Claude, tests de caracterización verdes en HEAD que matan mutantes) -> etapa 1 (aceptación roja en HEAD) -> una parada que aprueba los dos juntos -> el writer escribe la spec base desde los tests aprobados -> etapa 2 escribe el delta. Las paradas humanas son costo medido (`builder-e2e`).
- Flags: ninguno.

### Q7 — Test de aceptación y nombre del archivo
- Asked: ¿aprobás el test con nueve casos y el archivo `tests/test_sdlc_<area>_base_<slug>.py`? Después: ¿falta algo?
- Captured: el usuario aprueba los nueve casos, el nombre del archivo (Claude nunca edita un test ya aprobado) y cuatro casos más: Claude solo escribe el archivo de caracterización (`gate_alcance`); los mutantes salen solo del archivo caracterizado; tras la aprobación los tests entran a `SNAP` y un coder que los edita falla `gate_baseline`; los requisitos base toman el menor número libre del área (ticket 02).
- Flags: un delta que cambia comportamiento caracterizado hace fallar esos tests -> ticket 05 (nota allá). Ticket 03 resuelto.

## Open flags (pending input)
