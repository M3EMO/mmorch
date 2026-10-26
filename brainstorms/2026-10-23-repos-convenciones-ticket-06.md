# Repos-convenciones, ticket 06 (specs y fichas con otra forma): Brainstorm / Discovery Notes
Date: 2026-10-23 · Goal: decidir qué hace el mecanismo con documentos de spec que no siguen `docs/specs/<slug>/` ni `docs/specs/<area>/` (`.scratch/repos-convenciones/issues/06-specs-con-otra-forma.md`, bd `orchestration-4u6`).

Contexto heredado: Adepor entra con `sdlc.toml` (ticket 02); `--upgrade` nunca mueve ni borra contenido (ticket 03); las áreas son voluntarias y su propuesta se decide en el ticket 08. Hechos: Adepor tiene diez specs planas `docs/specs/NN-<tema>.md` (documentos por módulo, "el código es la fuente de verdad"). Estudio, Portfolio, ChatBot y ssb tienen fichas `docs/sdlc/features/<slug>.md` (embudo wayfinder→SDLC del 2026-09-15).

## Summary / key decisions

- Tests aprobados: `test_specs_previas` (9 puntos) y `test_archivo_de_la_ficha` (5 puntos) (P5). Ticket 06 resuelto.
- La etapa 6 compara el nombre de la ficha y el de la tarea después de pasarlos por `_slug()` (fichas reales: `macro-leadlag.md`, `F1-escena.md`).
- Un área con spec fuera del default guarda sus deltas en `docs/specs/<area>/changes/`.
- Fichas de `docs/sdlc/features/`: la etapa 6 archiva la ficha junto al delta (P3). Con área: `docs/specs/<area>/changes/<slug>-<sha>.ficha.md`; sin área: `docs/specs/<slug>/ficha.md`; solo si existe `docs/sdlc/features/<slug>.md`. Las fichas ya mergeadas quedan donde están (P4).
- Fichas `docs/sdlc/features/<slug>.md`: costumbre retirada (P3). Severidad pendiente de P4.
- Formato para C: tabla opcional `[spec_de_area]` con `nombre = "ruta"`; `[areas]` de `sdd-fase-2` queda igual; un área sin entrada usa `docs/specs/<area>/spec.md` (P2). Aviso enviado a la sesión de SDD fase 2.
- Specs planas de Adepor: primero A (`doctor` ignora archivos sueltos de `docs/specs/`), después C (cada spec plana se declara como área y el archivo queda en su lugar) (P1).

## Q&A log

### P1 — Specs planas de Adepor
- Asked: ¿A (ignorar archivos sueltos), B (proponer mover a `docs/specs/<area>/spec.md`) o C (área declarada con `spec = <ruta>`)?
- Captured: el usuario respondió "A primero y despues c".
- Choque: el formato de áreas de `sdd-fase-2` (ticket 01, Q15) es `[areas]` con `nombre = [globs]`; no tiene lugar para una ruta de spec. C necesita extender ese formato -> P2.

### P2 — Dónde va la ruta de una spec fuera del default
- Asked: ¿A (`[areas.<nombre>]` con `globs` y `spec`), B (tabla opcional `[spec_de_area]`) o C (tabla en línea dentro de `[areas]`)?
- Captured: el usuario eligió B.

### P3 — Fichas de `docs/sdlc/features/`
- Asked: ¿A (vivas, `doctor` no las mira), B (la etapa 6 las archiva junto al delta) o C (retirar; aviso por ficha)?
- Hecho: ningún código de `sdlc/`, `mmorch/` ni los skills lee esas fichas (grep, 2026-10-23).
- Captured: el usuario eligió C.
- Choque: el ticket 03 (P5) fijó la regla general "costumbre retirada = desvío de `doctor`". La opción C decía aviso -> P4.
- Consecuencia: la decisión "embudo wayfinder→SDLC" del mapa `sdlc-6-gates` (2026-09-15) queda retirada.

### P3 — Fichas de `docs/sdlc/features/`
- Asked: ¿A (vivas, `doctor` no las mira), B (la etapa 6 las archiva junto al delta) o C (retirar con aviso)?
- Captured: el usuario eligió B.
- Hecho: ningún código de `sdlc/`, `mmorch/` ni los skills lee `docs/sdlc/features/`.
- Choque: `changes/<slug>.md` ya es el delta (ticket 01 de `sdd-fase-2`); archivar la ficha con ese nombre pisaría el delta -> P4.

### P4 — Nombre de la ficha archivada
- Propuesta inicial de Claude: `changes/<slug>.ficha.md` con área; `docs/specs/<slug>/ficha.md` sin área; copia solo si la ficha existe.
- Corrección recibida de la sesión de SDD fase 2 (ticket 06 de `sdd-fase-2`, resuelto 2026-10-23): el delta se archiva como `changes/<slug>-<sha corto del commit base>.md` y el plan como `changes/<slug>-<sha>.plan.md`. Propuesta corregida: `changes/<slug>-<sha>.ficha.md`.
- Captured: el usuario confirma los nombres. Claude avisa a la sesión de SDD fase 2, dueña de la etapa 6.

### P5 — Tests de aceptación
- Asked: ¿apruebas `test_specs_previas` (7 puntos) y `test_archivo_de_la_ficha` (3 puntos)? El usuario preguntó si faltaba algo.
- Captured: Claude agregó ruta que sale del repo (8), dos áreas con la misma spec (9), deltas en `docs/specs/<area>/changes/` (10) y la comparación con `_slug()` (11). El usuario respondió "si".
- Decisión: 8 y 9 entran a `test_specs_previas`; 10 y 11 entran a `test_archivo_de_la_ficha`. Claude avisa a la sesión de SDD fase 2.

## Open flags (pending input)
