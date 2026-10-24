# SDD fase 2, ticket 04 (formato del delta y su gate): Brainstorm / Discovery Notes
Date: 2026-10-23 · Goal: decidir qué forma tiene el delta y qué chequea su gate determinista. Ticket: `.scratch/sdd-fase-2/issues/04-formato-y-gate-del-delta.md`.

Insumos: ticket 01 (spec por área; delta en `docs/specs/<area>/changes/<slug>.md`), ticket 02 (IDs locales al área, menor número libre, `ruta::test`, `removidos/`), ticket 03 (etapa 0 y spec base desde tests aprobados).

## Summary / key decisions

- "C liviana" (Q1): el writer escribe solo el delta; el código lo fusiona con la base (ticket 06) y exige que el diff base -> fusionada nombre exactamente los IDs del delta. Una sola escritura del LLM.
- `REMOVED` exige motivo; el motivo va a `removidos/README.md` (Q2). Sin `RENAMED`.
- La etapa 2b lee la spec fusionada como contexto y edita solo el delta; las clarificaciones quedan en el delta (Q3).
- La etapa 1 recibe la spec del área y los próximos números libres; cada `test_R<n>` corresponde a un `ADDED` o `MODIFIED` (Q5).
- Un `MODIFIED` repite el requisito entero; test aprobado con dieciséis casos (Q6). Ticket 04 resuelto.

## Q&A log

### Q1 — Quién escribe el delta
- Asked: ¿writer solo el delta (A), spec completa + diff determinista (B) o los dos con gate de coincidencia (C)?
- Captured: el usuario pide "C pero menos bloated". Claude propone C liviana: (1) writer escribe solo el delta; (2) el código fusiona; (3) el código exige que el diff entre base y fusionada nombre exactamente los IDs del delta (también detecta un bug de la fusión). El delta que olvida un requisito cambiado lo atrapa el gate de suite: el test de ese ID falla (regla del ticket 05). El usuario elige **C liviana** ("si").
- Flags: ninguno.

### Q2 — Motivo en `REMOVED`
- Asked: ¿motivo obligatorio (A), opcional (B) o sin motivo (C)?
- Captured: el usuario elige **A**. El gate rechaza un `REMOVED` sin motivo; solo chequea que exista, no juzga el contenido. El motivo pasa a `docs/specs/<area>/removidos/README.md` junto al ID (ticket 02); un agente futuro lo lee antes de reusar el número.
- Flags: ninguno.

### `RENAMED` (resuelto por decisiones previas, sin pregunta)
- Un requisito es `- R<n>: <regla>`, sin nombre aparte. Cambiar el texto es `MODIFIED`. Cambiar el ID está prohibido (ticket 02: nunca se renumera). `RENAMED` no tiene caso.

### Q3 — Qué revisa la etapa 2b con delta
- Asked: ¿revisa y edita solo el delta (A), lee la spec fusionada como contexto y edita solo el delta (B), o edita los dos (C)?
- Captured: el usuario elige **B**. Claude ve la spec entera para notar choques con requisitos viejos; `gate_alcance` limita la edición al delta; `## Clarificaciones` queda en el delta (`changes/<slug>.md`) como historia. C descartada: solo el código arma la spec fusionada (Q1).
- Flags: ninguno.

### Q4 — Test de aceptación (primera versión)
- Asked: ¿aprobás `MODIFIED` repitiendo el requisito entero (como OpenSpec) y un test de diez casos? Después: ¿falta algo?
- Captured: Claude sumó tres casos (encabezado desconocido falla; el gate vuelve a correr tras la etapa 2b; sin área no hay delta ni gate) y encontró un hueco de diseño (Q5).
- Flags: aprobación final pendiente tras Q5.

### Q5 — IDs de la etapa 1 contra IDs del delta
- Asked: la etapa 1 numera `test_R1_*` por feature y el delta numera por área (`ADDED R3`). ¿La etapa 1 recibe la spec del área y los próximos números libres (A), el código renombra tests después (B), o una tabla en el delta mapea test a requisito (C)?
- Captured: el usuario elige **A**. Los números libres salen de la spec por código; Claude lee la spec del área y ve qué requisito modifica; el test nace con su ID final y la aprobación humana ve nombres correctos. Orden con el ticket 03: la etapa 0 asigna primero los IDs base y la etapa 1 recibe los siguientes libres. Caso nuevo: cada `test_R<n>` de la etapa 1 corresponde a un `ADDED` o `MODIFIED` del delta.
- Flags: aprobación final del test (Q6).

### Q6 — Test completo
- Asked: ¿aprobás catorce casos y el formato de `MODIFIED`? Después: ¿falta algo?
- Captured: Claude sumó dos casos (`MODIFIED` sin cambio falla con su propio mensaje; la etapa 1 que nombra un número fuera de la lista ofrecida falla). El usuario aprueba los dieciséis casos y el formato: un `MODIFIED` repite el requisito entero, como OpenSpec.
- Flags: ninguno. Ticket 04 resuelto.

## Open flags (pending input)
