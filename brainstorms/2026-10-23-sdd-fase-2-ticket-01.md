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

## Verificación contra el código (2026-10-23, pedida por el usuario durante el ticket 02)

Claude cruzó las respuestas Q1 a Q9 con `sdlc/pipeline.py`, `mmorch/server_engine.py`, `sdlc/builder.py` y la skill `/project`.

- **V1 (real) — el camino común no trae `files`.** La skill `/project` hace `--files` opcional: con `sdlc.toml`, el techo alcanza (`_resolve_files` devuelve el techo si `files is None`). Entonces `FEAT["files"]` queda `[]` y, por Q3 + Q4, la corrida nunca tiene área ni delta. Solo `sdlc/builder.py` pasa `files` siempre. Reabre Q3.
- **V2 (real) — el corte por dos áreas no puede usar `_proponer_division()`.** Esa función llama al planner con la spec y el plan, que no existen antes de la etapa 2. El caso 5 del test exige cero llamadas al LLM. Contradicción entre los casos 4 y 5.
- **V3 (real) — el plan puede salir del área.** El área sale de los `files` del llamador, pero el plan de la etapa 3 puede sumar archivos dentro del techo. Un plan que escribe archivos de otra área cambia esa área sin delta y su spec envejece.
- **V4 (menor) — `fnmatch` cruza carpetas.** `_calza()` usa `fnmatch`, donde `*` también calza con `/`: `mmorch/*.py` calza con `mmorch/sub/x.py` (verificado). Un área declarada con `*` cubre subcarpetas y puede solaparse con otra sin que el usuario lo note (Q8 corta).

### Q3 revisada (por V1) — De dónde sale el área sin `files`
- Asked: ¿exigir `--files` cuando hay áreas (A), rutas del texto de la tarea por regex (B), parámetro `area=` (C)?
- Captured: el usuario elige **A con B como respaldo determinista**. El área sale de los `files` del llamador; la skill `/project` debe pasar `--files` (ticket 12 nuevo). Sin `files`, el área sale de las rutas que nombra el texto de la tarea, con un regex como `_need_files()`. Sin rutas, camino de hoy (Q4).
- Captured: V1 -> el usuario pide "un ticket para actualizar o mejorar la skill /project". Creado: `.scratch/sdd-fase-2/issues/12-skill-project-pasa-files.md`.
- Flags: V2, V3 y V4 pendientes (Q11, Q12, Q13).

### Q11 (por V2) — Qué escribe el corte por dos áreas
- Asked: ¿`division.md` determinista con cada área y sus archivos (A), corte sin archivo (B) o planner con solo la tarea (C)?
- Captured: el usuario elige **A**. El corte escribe `docs/sdlc/division.md` agrupando los archivos por área con los mismos globs, sin LLM. No reusa `_proponer_division()` (pide spec y plan, que no existen antes de la etapa 1). El caso 5 del test (cero llamadas al LLM) queda consistente con el caso 4.
- Flags: ninguno.

### Q12 (por V3) — El plan sale del área
- Asked: ¿plan limitado al área + archivos sin área (A), cortar después del plan (B) o segundo delta (C)?
- Captured: el usuario elige **A**. `gate_plan_allowlist` rechaza archivos de otra área; la vuelta de reescritura de la etapa 3 ya existe.
- Captured: el usuario agrega "que se cree un ticket para otra corrida". Aprobado con tres pasos: el gate anota cada archivo rechazado con su área en `docs/sdlc/division.md`; el run-log guarda la lista bajo una clave fija (`fuera_de_area`); la skill `/project` la lee al terminar.
- Captured: el usuario no quiere depender de que un agente recuerde: "No puede hacerse inmediatamente? y que si despues hay un archivo nuevo en esa misma area se 'sume' al bd?". Claude verificó: `bd create` acepta `--labels` y `bd update` acepta `--append-notes`, así que un issue por área con label fija admite crear o sumar sin juicio.
- Flags: en qué bd se crea el issue cuando el repo destino no es orchestration (Q12b).

### Q12b — En qué bd va el issue de fuera de área
- Asked: ¿bd del repo destino si tiene `.beads`, si no el de orchestration (A); siempre orchestration con `repo:<nombre>` (B); solo el del repo destino, si no md (C)?
- Captured: el usuario elige **A**. Diseño aprobado: la etapa 3 (host) llama a `bd` en el momento; label fija `sdlc-fuera-de-area:<area>`; issue abierto con esa label -> `bd update --append-notes` con el archivo y la tarea; sin issue -> `bd create` con la label. `division.md` y run-log quedan como copia.
- Flags: el usuario pregunta si cada proyecto propio debería usar beads -> pertenece al esfuerzo `orchestration-4u6` (sesión de repos, ticket 01 "qué costumbres entran").

### Q13 (por V4) — Solapamiento sorpresa por `fnmatch`
- Asked: ¿semántica aparte para áreas (A), prohibir `*` suelto (B) o mantener `_calza()` y chequear con `git ls-files` (C)?
- Captured: el usuario elige **C**. Una sola semántica para techo y áreas. Un chequeo lista los archivos de cada área sobre `git ls-files` y nombra los archivos que caen en dos áreas, antes de correr. B descartada: en `fnmatch`, `**` y `*` calzan igual.
- Flags: ninguno.

### Q14 — Test de aceptación tras V1-V4
- Asked: ¿falta algo en el test corregido?
- Captured: el usuario aprueba seis casos más: `files` ganan sobre rutas del texto; texto con dos áreas corta; plan con archivo del área + archivo sin área pasa; `bd` que falla no rompe la corrida y deja `division.md` y `fuera_de_area`; issue cerrado con la label -> issue nuevo; archivo rechazado dos veces -> una sola nota.
- Captured: nota aprobada en el ticket 11: la corrida real verifica que `bd` corre en el host y deja el issue en el bd correcto.
- Flags: ninguno. Ticket 01 cerrado de nuevo.
