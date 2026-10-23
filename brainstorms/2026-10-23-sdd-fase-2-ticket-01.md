# SDD fase 2, ticket 01 (unidad de la spec viva): Brainstorm / Discovery Notes
Date: 2026-10-23 · Goal: decidir sobre qué unidad se escribe el delta del builder `sdlc/` (feature, área o mixto). Ticket: `.scratch/sdd-fase-2/issues/01-unidad-de-la-spec-viva.md`.

## Summary / key decisions

- Unidad de la spec viva: **por área** (opción B). Cada área tiene `docs/specs/<area>/spec.md`; cada feature deja su delta en `docs/specs/<area>/changes/<slug>.md`.
- Área = entrada de `sdlc.toml` con nombre y globs (Q2).
- El área de una tarea sale de los `files` del llamador cruzados con esos globs (Q3).
- Sin área, el SDLC sigue el camino de hoy, sin delta (Q4).
- Dos o más áreas cortan la corrida y proponen dividir la tarea (Q5).
- Un área más archivos sin área: vale el área; los sueltos se ignoran (Q6).
- Con área, el plan va junto al delta: `docs/specs/<area>/changes/<slug>.plan.md`; sin carpeta por feature (Q7).
- Un archivo en dos áreas cuenta las dos y corta por la regla de Q5 (Q8).
- Test de aceptación aprobado con nueve casos en dos tests (Q9). Ticket 01 resuelto.

## Q&A log

### Q1 — Unidad del delta
- Asked: ¿feature (A), área (B) o mixto con índice (C)?
- Captured: el usuario elige **B**. Motivo dado por Claude y aceptado: A deja specs contradictorias; C exige mantener un índice a mano.
- Flags: falta definir qué es un área (Q2).

### Q2 — Definición de área
- Asked: ¿carpeta (A), lista con globs en `sdlc.toml` (B) o archivo (C)?
- Captured: el usuario elige **B**. Cada área se declara en `sdlc.toml` con nombre y globs; se compara con `_calza()`, como el techo del plan.
- Flags: regla para archivos que no caen en ningún área declarada (pendiente).

### Q3 — Quién elige el área
- Asked: ¿el llamador con `area=` (A), derivada de los `files` del llamador (B) o de los archivos del plan (C)?
- Captured: el usuario elige **B**. Determinista, sin parámetro nuevo. C descartada porque la etapa 2 escribe el delta antes que el plan (etapa 3).
- Flags: `files` es opcional en `build_feature` (`files or []`); sin `files` no hay de dónde derivar (Q4).

### Q4 — Sin área (tarea sin `files` o archivo fuera de todo glob)
- Asked: ¿camino de hoy sin delta (A), cortar y pedir declarar el área (B) o crear área por carpeta (C)?
- Captured: el usuario elige **A**. Sin área, la corrida sigue como hoy: spec completa en `docs/specs/<slug>/`, sin delta. Las áreas se adoptan por repo, de forma voluntaria; los repos adoptados no se rompen.
- Flags: caso mixto (unos `files` en un área, otros en ninguna) y caso de varias áreas (Q5).

### Q5 — Varias áreas
- Asked: ¿cortar y proponer división (A), un delta por área (B) o el área con más archivos (C)?
- Captured: el usuario elige **A**. Dos o más áreas cortan la corrida y proponen dividir la tarea, como `gate_tamano` con `_proponer_division()` y `docs/sdlc/division.md`.
- Flags: caso mixto, un área más archivos sin área (Q6).

### Q6 — Caso mixto (un área + archivos sin área)
- Asked: ¿tomar el área e ignorar los sueltos (A), tratar como sin área (B) o cortar (C)?
- Captured: el usuario elige **A**. Los archivos sin área no apagan el delta del área encontrada. C descartada porque contradice Q4 (adopción voluntaria).
- Flags: ninguno.

### Migración de specs archivadas (resuelto por el código, sin pregunta)
- Claude buscó `docs/specs/*/spec.md` en `/home/mpera` (2026-10-23): no hay ninguno. No hace falta migrar nada.

### Q7 — Dónde queda el plan con área
- Asked: ¿plan junto al delta (A), además la carpeta por feature (B) o sin plan (C)?
- Captured: el usuario elige **A**. Con área, el plan va a `docs/specs/<area>/changes/<slug>.plan.md`; no se crea `docs/specs/<slug>/`. Un solo lugar por feature; B duplicaría la spec.
- Flags: ninguno.

### Q8 — Globs solapados (un archivo en dos áreas)
- Asked: ¿rechazar el `sdlc.toml` (A), contar las dos áreas y cortar por Q5 (B) o la primera declarada (C)?
- Captured: el usuario elige **B**. Un archivo que calza con dos áreas cuenta las dos y la corrida corta con propuesta de división (regla de Q5). Sin código nuevo; el solapamiento queda visible.
- Flags: ninguno.

### Q9 — Test de aceptación
- Asked: ¿aprobás el test con cinco casos más `test_archivo_con_area`? Después: ¿faltan casos?
- Captured: el usuario aprueba el test con cuatro casos más que propuso Claude: `files` fuera de todo glob → `None`; `sdlc.toml` sin áreas mantiene verde `test_dos_features_dejan_dos_specs`; el corte por dos áreas ocurre antes de cualquier llamada al LLM; otro orden de `files` da el mismo resultado.
- Captured: el usuario aprueba una nota en el ticket 06: dos features con el mismo slug en la misma área pisan `changes/<slug>.md`.
- Flags: pedido nuevo del usuario, fuera del ticket 01: "una forma de actualizar mis propios repos bajo estructuras de TOML's, SPEC's y otras costumbres que tomamos" (Q10).

### Q10 — Dónde vive el pedido de actualizar los repos
- Asked: ¿ticket 12 en este mapa, limitado a la fase 2 (A), o esfuerzo aparte con issue bd y mapa propio (B)?
- Captured: el usuario elige **B**. Hechos leídos en `init()`: nunca agrega claves a un `sdlc.toml` existente, copia plantillas a `docs/sdlc/` una sola vez, no conoce áreas.
- Captured: Claude creó `orchestration-4u6` (bd), relacionado (no bloqueante) con `orchestration-69j`. El ticket 11 de este mapa depende de ese esfuerzo solo para declarar áreas en el repo de la corrida.
- Flags: el mapa wayfinder de `orchestration-4u6` está sin chartear -> usuario decide cuándo.

## Open flags (pending input)
- Mapa wayfinder de `orchestration-4u6` (actualizar repos a las convenciones) -> usuario decide cuándo se chartea.
