# SDD fase 2, ticket 02 (espacio de IDs entre features): Brainstorm / Discovery Notes
Date: 2026-10-23 · Goal: decidir cómo se nombran los requisitos para que un delta cite sin ambigüedad un `R<n>` de otra feature. Ticket: `.scratch/sdd-fase-2/issues/02-espacio-de-ids.md`.

Insumo: ticket 01 resuelto. La spec viva es por área (`docs/specs/<area>/spec.md`); el delta va en `docs/specs/<area>/changes/<slug>.md`; sin área, el camino de hoy.

## Summary / key decisions

- Numeración local a la spec del área (opción A). Cada `ADDED` sigue el máximo `R<n>` de esa spec. El regex `\bR\d+\b` de los gates no cambia.

## Q&A log

### Q1 — Dónde vive la numeración
- Asked: ¿dentro de la spec del área (A), prefijo de área en todo el repo (B) o IDs por feature con `<slug>:R3` (C)?
- Captured: el usuario elige **A**. B tocaba tres gates y la etapa 1; C contradice el ticket 01 (la feature deja de ser la unidad).
- Flags: cómo cita la tabla de trazabilidad del área tests de varias features con nombres repetidos (Q2).

### Q2 — Cita de tests de features distintas
- Asked: ¿ruta con `::` (A), área en el nombre del test (B) o solo el nombre (C)?
- Captured: el usuario elige **A**. La tabla del área cita `tests/test_sdlc_<slug>.py::test_R<n>_...`, formato estándar de pytest. Los nombres que exige la etapa 1 no cambian.
- Flags: el usuario pidió verificar el ticket 01 contra el código antes de seguir (hecho entre Q2 y Q3).

## Open flags (pending input)
