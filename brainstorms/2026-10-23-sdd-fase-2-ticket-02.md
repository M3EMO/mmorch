# SDD fase 2, ticket 02 (espacio de IDs entre features): Brainstorm / Discovery Notes
Date: 2026-10-23 · Goal: decidir cómo se nombran los requisitos para que un delta cite sin ambigüedad un `R<n>` de otra feature. Ticket: `.scratch/sdd-fase-2/issues/02-espacio-de-ids.md`.

Insumo: ticket 01 resuelto. La spec viva es por área (`docs/specs/<area>/spec.md`); el delta va en `docs/specs/<area>/changes/<slug>.md`; sin área, el camino de hoy.

## Summary / key decisions

- Numeración local a la spec del área (opción A). Cada `ADDED` toma el menor número libre de esa spec (Q6). El regex `\bR\d+\b` de los gates no cambia.
- La tabla del área cita tests con `ruta::test` (Q2).
- Test de aceptación aprobado con once casos (Q7). Ticket 02 resuelto.
- Un ID eliminado se reusa; sus tests se mueven a una subcarpeta, no se borran (Q3).
- La subcarpeta es `docs/specs/<area>/removidos/`, fuera de la suite (Q4).
- La lista de IDs eliminados vive en `docs/specs/<area>/removidos/README.md`, no en la spec (Q5).

## Q&A log

### Q1 — Dónde vive la numeración
- Asked: ¿dentro de la spec del área (A), prefijo de área en todo el repo (B) o IDs por feature con `<slug>:R3` (C)?
- Captured: el usuario elige **A**. B tocaba tres gates y la etapa 1; C contradice el ticket 01 (la feature deja de ser la unidad).
- Flags: cómo cita la tabla de trazabilidad del área tests de varias features con nombres repetidos (Q2).

### Q2 — Cita de tests de features distintas
- Asked: ¿ruta con `::` (A), área en el nombre del test (B) o solo el nombre (C)?
- Captured: el usuario elige **A**. La tabla del área cita `tests/test_sdlc_<slug>.py::test_R<n>_...`, formato estándar de pytest. Los nombres que exige la etapa 1 no cambian.
- Flags: el usuario pidió verificar el ticket 01 contra el código antes de seguir (hecho entre Q2 y Q3).


### Verificación del ticket 01 (entre Q2 y Q3)
- El usuario pidió verificar el ticket 01 antes de seguir. Hallazgos V1-V4 y sus respuestas en `brainstorms/2026-10-23-sdd-fase-2-ticket-01.md` (Q3 revisada, Q11 a Q14). Ticket 12 nuevo (skill `/project` pasa `--files`).
- Renumerar specs archivadas: no aplica (no existe ningún `docs/specs/*/spec.md`).

### Q3 — Reusar un ID eliminado
- Asked: ¿nunca reusar (A), reusar y borrar tests viejos (B) o línea en la spec con los IDs eliminados (C)?
- Captured: el usuario elige **B + C**, "mas que nada para no hacer bloat PERO no borre los tests sino que los derive a una subcarpeta de R borrados". Se reusa el número; la spec lleva una línea con los IDs eliminados; los tests del ID eliminado se mueven a una subcarpeta, no se borran.
- Hecho: con la cita `ruta::test` de Q2, un `test_R5_*` nuevo y uno movido no se confunden en la trazabilidad.
- Flags: dónde vive la subcarpeta y cómo queda fuera de la suite (Q4); esto responde en parte el ticket 05 para `REMOVED` (anotar allá).

### Q4 — Dónde viven los tests eliminados
- Asked: ¿`tests/removidos/<area>/` con `collect_ignore` (A), `docs/specs/<area>/removidos/` (B) o `tests/removidos/` con `skip` (C)?
- Captured: el usuario elige **B**. pytest no recolecta `docs/`, así que no hace falta configuración; el test queda junto a la spec que explica la baja. Evita que `gate_suite_total` cuente un test movido como regresión.
- Flags: `gate_traza_spec` toma todo `\bR\d+\b` de la spec; la lista de IDs eliminados lo dispararía (Q5).

### Q5 — Lista de IDs eliminados sin romper la trazabilidad
- Asked: ¿sección `## Eliminados` que los gates ignoran (A), forma `ex-R5` (B) o lista fuera de la spec en `docs/specs/<area>/removidos/README.md` (C)?
- Captured: el usuario elige **C**. La spec viva describe solo el comportamiento actual; los gates (`gate_traza_spec` toma todo `\bR\d+\b`) no cambian. Corrige Q3: la "línea con los IDs eliminados" vive en `removidos/README.md`, no en la spec.
- Flags: qué número toma un `ADDED` cuando hay huecos (Q6).

### Q6 — Número de un `ADDED` con huecos
- Asked: ¿máximo + 1 (A), menor número libre (B) o el autor elige (C)? Replanteada: el usuario prefería A sin huecos y preguntó por un hook que actualice las specs al cerrar sesión.
- Captured: Claude explicó que A sin huecos exige renumerar, y renumerar renombra tests (el oráculo), rompe deltas viejos en `changes/` y referencias en commits y bd. Opción D (renumerar al fusionar) no recomendada.
- Captured: el usuario elige **B**: el `ADDED` toma el menor número libre del área; el hueco dura hasta el próximo cambio, sin hook. Corrige Q1: no es "máximo + 1".
- Hecho: `removidos/README.md` es historia; un ID reusado conserva su entrada vieja.
- Flags: ninguno.

### Q7 — Test de aceptación
- Asked: ¿aprobás el test con ocho casos? Después: ¿falta algo?
- Captured: el usuario aprueba los ocho casos y tres más: dos `ADDED` en el mismo delta toman R3 y R6; una cita `ruta::test` a un test inexistente falla el gate; un ID eliminado dos veces no pisa el test movido la primera vez.
- Captured: nota aprobada para el ticket 06: dos ramas en paralelo pueden tomar el mismo número libre; la fusión chequea el número contra la spec base del momento.
- Flags: ninguno. Ticket 02 resuelto.

## Open flags (pending input)
