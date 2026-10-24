# Mapa wayfinder — SDD fase 2: deltas para código existente y tests de propiedades
Label: wayfinder:map · Creado: 2026-10-23 · Tickets en `issues/` · Tracker: local-markdown (`docs/agents/issue-tracker.md`) · bd: `orchestration-69j`

## Destination

El builder `sdlc/` (paquete `sdlc/pipeline.py`) gana dos capacidades del curso de SDD, niveles 5 y 6:

1. **Deltas para código existente** (estilo OpenSpec). Una feature sobre un área que ya existe parte de una spec del comportamiento actual. El cambio se escribe como delta con secciones `ADDED`, `MODIFIED` y `REMOVED` sobre la spec archivada en `docs/specs/<feature>/spec.md`.
2. **Tests de propiedades** (Hypothesis en Python) desde los requisitos `R<n>` ubicuos de la spec. Los tests de propiedades no duplican lo que ya mide `gate_mutacion`.

El mapa termina cuando cada ticket está resuelto por el usuario y cada capacidad tiene su test de aceptación aprobado. La implementación queda fuera de este mapa: sale después, por el SDLC o a mano, con los tests de aceptación de los tickets.

Regla dura: lo checkeable lo decide un oráculo, nunca un LLM juez (GOAL.md). Cada gate nuevo de este mapa es determinista.

Criterio de juicio (memoria `sdlc-es-builder`, 2026-10-22): el SDLC es un builder. Un cambio al builder se juzga con tests, gates y corridas reales. No pide A/B del bench.

## Notes

Fuentes:

- Doc del curso: https://claude.ai/artifact/GQmhZq98ceVJZX7cZYK7YP, niveles 5 (verificación) y 6 (avanzado).
- Nota del vault: `vault/research/estado-de-spec-driven-development-octubre-2026.md`.
- Fase 1, ya cerrada: `orchestration-3s7` (spec archivada por feature) y `orchestration-fcr` (constitución del repo). Commit `e72394c` en `mmorch/auto`.

Lo que el curso propone:

- Nivel 6.1 (brownfield) tiene cinco pasos. El agente redacta una spec del comportamiento actual. Tests de caracterización fijan ese comportamiento. La spec se revisa contra los tests. El cambio se escribe como delta. El delta se implementa con el ciclo normal.
- Nivel 6.2 pide tres reglas. El cambio de comportamiento modifica la spec en el mismo PR. El delta describe solo lo que cambia. La revisión del PR empieza por el diff de la spec.
- Nivel 5.2 muestra un test de Hypothesis por requisito ubicuo, con el ID en el nombre (`test_r1_...`).
- OpenSpec 1.14.3 usa `ADDED`, `MODIFIED` y `REMOVED`. La doc no nombra `RENAMED`. Al archivar, OpenSpec funde el delta en `specs/` y mueve el cambio a `changes/archive/`.
- Kiro extrae propiedades de requisitos EARS y genera tests de propiedades por requisito. Esa función existe solo en el IDE.

Lo que el código hace hoy (leído en `sdlc/pipeline.py`, rama `mmorch/auto` en `9d4de8b`):

- `_archivar_spec()` copia `docs/sdlc/spec.md` y `plan.md` a `docs/specs/<slug>/` en la etapa 6. El slug sale del nombre de la tarea. Ninguna etapa lee hoy una spec archivada.
- La etapa 1 escribe el test de aceptación ANTES de la spec. La etapa 2 recibe esos tests como fijos ("no se modifican") y escribe la spec desde ellos.
- La plantilla `sdlc/templates/spec-template.md` no usa EARS. Un requisito es una línea `- R1: <regla>`. Ningún marcador distingue un requisito ubicuo.
- Los IDs `R<n>` son locales a cada feature. `gate_traza_spec` busca `\bR\d+\b`. La etapa 1 exige `def test_R<n>_...`.
- `gate_mutacion` muta solo las líneas que cambió la feature (`_lineas_cambiadas`). Corre la aceptación entera por mutante. Sin `mutation_min`, observa una muestra de 5 mutantes y no bloquea.
- `gate_suite_total` compara fallos por nombre contra el baseline. Un test viejo que falla por diseño cuenta como regresión.
- `SNAP` y `gate_baseline` protegen solo los tests de aceptación de la feature actual. Los tests de features viejas no están en `SNAP`.
- Hypothesis no está instalado en el venv de orchestration ni en el extra `dev` de `pyproject.toml`.

## Decisions so far

<!-- una línea por ticket resuelto: gist + link -->

- [Unidad de la spec viva](issues/01-unidad-de-la-spec-viva.md) — spec viva por área (`docs/specs/<area>/spec.md`), delta y plan en `changes/<slug>.md`; área = nombre y globs en `sdlc.toml`, derivada de los `files` del llamador; sin `files`, rutas del texto de la tarea; sin área sigue el camino de hoy; dos áreas cortan con `division.md` determinista; el plan no sale del área y lo rechazado va a bd por label `sdlc-fuera-de-area:<area>`; chequeo de solapamiento con `git ls-files`.
- [Espacio de IDs entre features](issues/02-espacio-de-ids.md) — numeración local al área; `ADDED` toma el menor número libre; nunca se renumera; la trazabilidad cita `ruta::test`; los tests de un ID eliminado van a `docs/specs/<area>/removidos/` con su `README.md`, fuera de la suite.
- [Spec base de un área sin spec](issues/03-spec-base-brownfield.md) — etapa 0-caracterizar solo para archivos que Contrato no nombra; Claude escribe `tests/test_sdlc_<area>_base_<slug>.py` (verde en HEAD, mata mutantes, `caracterizacion_min` opcional); una sola parada aprueba caracterización y aceptación; el writer escribe la spec base desde los tests aprobados.
- [Formato del delta y su gate](issues/04-formato-y-gate-del-delta.md) — el writer escribe solo el delta (`MODIFIED` repite el requisito entero); el código fusiona y exige que el diff nombre exactamente los IDs del delta; `REMOVED` con motivo; sin `RENAMED`; 2b edita solo el delta; la etapa 1 recibe los próximos números libres.
- [Delta contra la regresión de suite](issues/05-delta-y-suite-total.md) — tests viejos de `MODIFIED` y `REMOVED` se mueven por código a `removidos/` antes de la parada única; un ID por test y un archivo por ID (`tests/sdlc/<area>/test_R<n>_<slug>.<ext>`); la etapa 1 declara las bajas con motivo; la suite cuenta un test faltante fuera de la lista de movimientos como regresión.

## Not yet specified

Los tickets abiertos llevan las decisiones. Orden sugerido (frontera primero):

| # | Ticket | Tipo | Bloqueado por |
|---|---|---|---|
| 01 | [Unidad de la spec viva: feature o área](issues/01-unidad-de-la-spec-viva.md) | grilling | — |
| 02 | [Espacio de IDs entre features](issues/02-espacio-de-ids.md) | grilling | 01 |
| 03 | [Spec base de un área sin spec](issues/03-spec-base-brownfield.md) | grilling | 01 |
| 04 | [Formato del delta y su gate](issues/04-formato-y-gate-del-delta.md) | grilling | 01, 02 |
| 05 | [Delta contra la regresión de suite](issues/05-delta-y-suite-total.md) | grilling | 04 |
| 06 | [Fusión del delta al archivar](issues/06-fusion-del-delta.md) | grilling | 04 |
| 07 | [Qué es un requisito ubicuo](issues/07-requisito-ubicuo.md) | grilling | — |
| 08 | [Etapa y autor de los tests de propiedades](issues/08-etapa-de-las-propiedades.md) | grilling | 07 |
| 09 | [Propiedades sin duplicar gate_mutacion](issues/09-propiedades-y-mutacion.md) | grilling | 08 |
| 10 | [Lenguajes, dependencia y determinismo](issues/10-lenguajes-y-determinismo.md) | grilling | — |
| 11 | [Corrida real de cierre](issues/11-corrida-real-de-cierre.md) | grilling | 03, 05, 06, 09, 10 |
| 12 | [La skill /project pasa los archivos de la tarea](issues/12-skill-project-pasa-files.md) | task | 01 |

Las dos capacidades comparten un solo punto: el ticket 08 decide si un requisito `MODIFIED` del delta recibe tests de propiedades. Fuera de ese punto, la rama de deltas (01 a 06) y la rama de propiedades (07 a 10) avanzan en paralelo.

## Out of scope

- Implementar código. Este mapa solo fija decisiones y tests de aceptación.
- Contratos de API con Schemathesis (nivel 5.3): ningún repo del SDLC expone OpenAPI hoy.
- Specs formales TLA+ o Alloy (nivel 6.4).
- ADR en el plan (nivel 6.3): candidato a un mapa aparte si el usuario lo pide.
- A/B del bench: el SDLC es builder (memoria `sdlc-es-builder`).

## Puntero bd

bd: `orchestration-69j` apunta a este mapa. Al cerrar el mapa, el issue se cierra con el resumen de Decisions so far.
