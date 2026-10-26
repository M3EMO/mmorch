# SDD fase 2, ticket 06 (fusión del delta al archivar): Brainstorm / Discovery Notes
Date: 2026-10-23 · Goal: decidir cómo queda la spec viva del área después de una feature con delta. Ticket: `.scratch/sdd-fase-2/issues/06-fusion-del-delta.md`.

Insumos ya decididos: fusión por código, diff base -> fusionada igual a los IDs del delta (ticket 04, opción A del ticket 06 de hecho); delta en `docs/specs/<area>/changes/<slug>.md` (ticket 01); un archivo de test por ID en `tests/sdlc/<area>/` (ticket 05); trazabilidad `ruta::test` (ticket 02); la sección Contrato nombra las rutas cubiertas (ticket 03). Notas pendientes: slug repetido (desde ticket 01) y número libre tomado por dos ramas (desde ticket 02).

## Summary / key decisions

- Fusión por código (decidido en el ticket 04): una función pura aplica el delta a la spec base.
- Trazabilidad y rutas cubiertas se derivan por código; casos borde y criterio van dentro de cada requisito (Q1).
- Las firmas viven en el plan de cada feature; el Contrato del área es solo la lista de rutas (Q2).
- El pre-commit chequea las partes derivadas y falla con el comando que las regenera; no escribe (Q3).
- La etapa 6 hace dos commits: spec primero, código después (Q4).
- El delta se archiva como `changes/<slug>-<sha corto base>.md` (Q5).
- El pre-commit también exige IDs únicos y un archivo por ID; un choque de ramas se rehace desde la etapa 1 (Q6).
- Un requisito cambia solo por delta: el pre-commit rechaza ediciones a mano de requisitos; test aprobado con dieciséis casos (Q7). Ticket 06 resuelto.

## Q&A log

### Q1 — Secciones que no son requisitos
- Asked: ¿Trazabilidad y lista de rutas derivadas por código, casos borde y criterios dentro de cada requisito (A); secciones `ADDED`/`MODIFIED` por cada sección (B); solo Requisitos y tablas derivadas, el resto como historia en cada delta (C)?
- Captured: el usuario elige **A**. En modo área, cada requisito es un bloque con su regla, sus casos borde y su criterio; un `MODIFIED` los arrastra juntos. Trazabilidad sale de `tests/sdlc/<area>/` (un archivo por ID, ticket 05). La lista de rutas cubiertas sale del plan y de la caracterización.
- Flags: las firmas del Contrato (clases, métodos, tipos) no salen de la lista de rutas; falta decidir quién las actualiza (Q2).

### Q2 — Firmas del Contrato
- Asked: ¿derivadas por código desde las funciones públicas (A), sección `## Contrato` en el delta (B) o fuera de la spec del área, en el plan de cada feature (C)?
- Captured: el usuario elige **C**. La firma es un cómo (nivel 2 del curso: spec = qué, plan = cómo). La sección Contrato del área queda solo con la lista de rutas cubiertas, derivada por código.
- Captured: el usuario agrega "hay que armar un hook para actualizar automaticamente el spec". Se interpreta para las partes derivadas (Trazabilidad, rutas cubiertas): los requisitos solo cambian por delta aprobado. Forma del hook pendiente (Q3).

### Q3 — Qué hace el hook sobre las partes derivadas
- Asked: ¿regenerar en el pre-commit y sumar al commit (A), chequear y fallar con el comando que regenera (B) o regenerar solo en la etapa 6 (C)?
- Captured: el usuario elige **B**. El pre-commit chequea Trazabilidad y rutas cubiertas de cada `docs/specs/<area>/spec.md` y falla si están viejas; el mensaje nombra el comando (ejemplo: `python -m sdlc specs --regenerar`). No escribe archivos, así que no repite el índice sucio del pre-commit de docgen (memoria `docgen-hook-indice`). Cubre las ediciones manuales fuera del SDLC.
- Flags: ninguno.

### Q4 — Cómo muestra el PR el cambio de la spec
- Asked: ¿dos commits, spec primero y código después (A), un commit con el diff de la spec primero en la descripción (B) o como hoy (C)?
- Captured: el usuario elige **A**. La etapa 6 hace dos commits: (1) delta, spec fusionada y `removidos/`; (2) el código. El revisor lee el primer commit con `git show` antes de abrir el código (nivel 6.2 del curso).
- Flags: slug repetido en `changes/` (Q5).

### Q5 — Choque de slugs en `changes/`
- Asked: ¿fecha delante y sufijo `-2` (A), cortar si existe (B) o sha corto del commit base (C)?
- Captured: el usuario elige **C**: `changes/<slug>-<sha corto del commit base>.md` (y `.plan.md`). El orden en el tiempo sale de `git log`, no de `ls`. Caso residual: dos corridas con el mismo slug desde el mismo commit base (un reintento) chocan; solo una de las dos ramas llega a merge.
- Flags: dos ramas en paralelo toman el mismo número libre (Q6).

### Q6 — Dos ramas toman el mismo número libre
- Asked: ¿el chequeo del pre-commit suma IDs únicos en la spec y un archivo por ID (A), reservar números en un archivo compartido (B) o rebasar y recalcular antes de la etapa 6 (C)?
- Captured: el usuario elige **A**. El pre-commit de Q3 corre también en el commit que cierra un merge con conflicto; dos `test_R3_*` en la misma área o un R3 repetido en la spec hacen fallar el hook. El usuario rehace la segunda rama desde la etapa 1 con números nuevos. B necesita un lock entre worktrees; C no ve una rama sin mergear.
- Flags: ninguno.

### Q7 — Test de aceptación
- Asked: ¿aprobás el test con trece casos? Después: ¿falta algo?
- Captured: el usuario aprueba dieciséis casos: los trece más un commit que cambia un requisito de `spec.md` sin sumar archivo a `changes/` hace fallar el pre-commit (regla nueva aprobada: un requisito cambia solo por delta; un typo también necesita delta), una etapa 6 reanudada no vuelve a aplicar el delta (reconoce su `changes/<slug>-<sha>.md`), y `--regenerar` dos veces seguidas no deja diferencias.
- Flags: ninguno. Ticket 06 resuelto.

## Open flags (pending input)
