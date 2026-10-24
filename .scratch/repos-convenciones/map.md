# Mapa wayfinder — repos propios al día con las convenciones
Label: wayfinder:map · Creado: 2026-10-23 · bd: `orchestration-4u6` · Tickets en `issues/` · Tracker: local-markdown (`docs/agents/issue-tracker.md`)

## Destination

Cada repo propio del usuario converge a las convenciones vigentes con un comando. Las convenciones son `sdlc.toml`, `docs/sdlc/`, las specs y otras costumbres elegidas. El comando nunca pisa lo que el usuario editó a mano. Un oráculo determinista dice si un repo está al día; ningún LLM lo decide.

## Notes

- Origen: el usuario pidió este esfuerzo en el grilling del ticket 01 de `.scratch/sdd-fase-2/` (2026-10-23). Ese mapa vive en la rama `claude/gifted-bartik-c4a833` y su bd es `orchestration-69j`.
- Reglas: resolver tickets es HITL; el usuario responde cada grilling. Lo checkeable lo decide un oráculo. Prosa en ETS.
- Este mapa no implementa código. La implementación sale después, con los tests de aceptación aprobados.
- La sonda de los repos fue de solo lectura (2026-10-23). Ningún archivo de esos repos cambió.

### Inventario de costumbres vigentes (con fuente)

| Costumbre | Fuente | Qué hace `init` hoy |
|---|---|---|
| `sdlc.toml` en la raíz, claves `accept_cmd`, `suite`, `files`, `ext`, `approve_accept`, `usd_max` | `init()` en `sdlc/pipeline.py:2038` | Lo escribe solo si no existe. Nunca agrega claves a uno existente. |
| Claves opcionales: `compile_cmd`, `lint_cmd`, `suite_cmd`, `seed_globs`, `accept_test`, `constitution`, `cobertura_min`, `mutation_min`, `reviewer_cmd`, `contenedor`, `python`, `tamano_max_archivos`, `tamano_max_tokens`, `[sprites]` | comentario de cabecera de `init()` y lecturas de `CFG` en `sdlc/pipeline.py` | Solo las nombra en comentarios del archivo nuevo. |
| Topes con default en código: `usd_max`, `stall_rounds`, `diff_novelty_min`, `cmd_timeout_s`, `suite_timeout_s`, `approve_accept` | `_cfg()` en `sdlc/pipeline.py:80` | No los escribe; el código pone el default. |
| `docs/sdlc/sdlc.toml` como ubicación vieja | `_cfg()` lo lee todavía como `legacy` | Ningún repo sondeado lo tiene. |
| Plantillas `spec-template.md`, `spec-review.md`, `refutar-tests.md` en `docs/sdlc/` | `sdlc/templates/` | Copia las que faltan en cada corrida de `init`. Nunca actualiza una plantilla distinta. |
| Spec y plan archivados en `docs/specs/<slug>/` | `_archivar_spec()`, `orchestration-3s7` | No los crea; los crea la etapa 6. |
| Constitución: `sdlc.toml` `constitution`, default `GOAL.md` | `_constitucion()`, `orchestration-fcr` | No la crea ni la declara. |
| Áreas con nombre y globs en `sdlc.toml`, spec viva en `docs/specs/<area>/spec.md`, deltas en `changes/` | ticket 01 de `sdd-fase-2` (resuelto, sin implementar) | No conoce áreas. La adopción es voluntaria por repo. |
| Puntero `## SDLC (pipeline de 6 etapas)` en `AGENTS.md` | `init()` | Lo agrega solo si `AGENTS.md` ya existe. |
| Ficha `docs/sdlc/features/<slug>.md` por feature | embudo wayfinder→SDLC (map de `sdlc-6-gates`, 2026-09-15) | No la crea; la escriben las sesiones a mano. |
| `contract_version`: el pipeline rechaza un contrato que no entiende | ticket 11 de `sdlc-6-gates` | **Decidido y nunca implementado**: cero apariciones en `sdlc/`, `mmorch/` y `tests/`. |
| `docs/sdlc/gates/` como parte obligatoria del contrato | ticket 11 de `sdlc-6-gates` | Nunca implementado; `init` no lo crea. |
| `CLAUDE.md` índice corto, `AGENTS.md`, `docs/claude/`, `tests/README.md`, `.gitignore` con secretos, `bd init`, `sdlc init` | skill `new-project` (`~/.claude/skills/new-project/SKILL.md`) | Solo aplica a repos nuevos. |

### Estado por repo (sonda de solo lectura, 2026-10-23)

| Repo | `sdlc.toml` | Plantillas | `AGENTS.md` | Constitución | Specs | Notas |
|---|---|---|---|---|---|---|
| orchestration (`mmorch/auto`) | 6 claves | falta `refutar-tests.md` | con puntero | `GOAL.md` | sin `docs/specs/` | `suite` trae `--basetemp C:/Users/map12/...`, ruta de Windows rota en la laptop. |
| Estudio (`~/Estudio`, rama `init`) | 10 claves | falta `refutar-tests.md` | no existe | ninguna | sin `docs/specs/` | 1 ficha en `docs/sdlc/features/`. 11 archivos sucios. |
| Portfolio (`~/Portfolio-financiero`) | 8 claves | falta `refutar-tests.md` | no existe | ninguna | sin `docs/specs/` | 2 fichas en `docs/sdlc/features/`. |
| ChatBot (`~/Proyectos/F3-chatbot/ChatBot`) | 9 claves, `contenedor` | completas | no existe | ninguna | sin `docs/specs/` | Artefactos de corrida commiteados en `docs/sdlc/`: `spec.md`, `plan.md`, `run-log.json`, `suite.log`. |
| Proyecto_Adepor (`~/BetDeportivo`) | **no tiene** | ninguna | no existe | ninguna | `docs/specs/NN-<tema>.md` planos, 10 archivos | El usuario lo dejó fuera del SDLC (2026-09-17). Los commits de adopción del escritorio no están en este clon. |

Repos con `sdlc.toml` fuera de la lista del pedido: `~/Proyectos/mf-moe`, `~/Proyectos/ssb`, `~/Proyectos/F2-seo-calculadoras/{f2-sueldo-neto,prototipo-04,prototipo-06,prototipo-10}`. Ninguno tiene `docs/specs/` ni constitución. Solo mf-moe tiene `AGENTS.md` con puntero.

`projects.json` (registro de mmorch) no sirve como lista de repos: le faltan Portfolio, ChatBot y ssb, y trae worktrees y carpetas `scratch-*`.

## Tickets

Todos son grilling. Frontera sin bloqueo (2026-10-23): 05, 06 y 08.

- [01 Qué costumbres entran](issues/01-que-costumbres-entran.md)
- [02 Qué repos entran y dónde vive la lista](issues/02-que-repos-entran.md)
- [03 Mecanismo de actualización](issues/03-mecanismo-de-actualizacion.md) — bloqueado por 01.
- [04 No pisar ediciones manuales](issues/04-no-pisar-ediciones-manuales.md) — bloqueado por 03.
- [05 Valores que dependen de la máquina](issues/05-valores-por-maquina.md)
- [06 Specs y fichas con otra forma](issues/06-specs-con-otra-forma.md) — bloqueado por 01.
- [07 Orden de adopción y forma de aplicar](issues/07-orden-de-adopcion.md) — bloqueado por 02, 03 y 04.
- [08 Cómo entra la propuesta de áreas](issues/08-propuesta-de-areas.md) — recibido de `sdd-fase-2` (2026-10-23).

## Decisions so far

<!-- una línea por ticket resuelto: gist + link -->

- [01 Qué costumbres entran](issues/01-que-costumbres-entran.md) — SDLC + SDD + `.gitignore` de secretos + `bd init`; `bd init` y los md van a `orchestration-ior`; gates a mmorch en `orchestration-bpn`; `contract_version` al 03; constitución y formato como avisos; mesa de `docs/sdlc/` fuera (`orchestration-eeq`); test de 11 puntos aprobado (2026-10-23).
- [02 Qué repos entran](issues/02-que-repos-entran.md) — un repo por invocación más `--todos`, que descubre carpetas con `.git` directorio y `sdlc.toml` (repos anidados sí, worktrees no); sin exclusión; Adepor entra (revierte 2026-09-17, `init` en el ticket 07); solo la laptop corre `--upgrade`; test de 10 puntos (2026-10-23).
- [03 Mecanismo de actualización](issues/03-mecanismo-de-actualizacion.md) — `doctor` (oráculo, sin escrituras) + `init --upgrade` (lista cerrada de arreglos seguros, convergencia sin migraciones); sin `contract_version` (clave desconocida = aviso); el pipeline registra `doctor` en `run-log.json` y sigue; todo en `sdlc/`; test de 15 puntos aprobado (2026-10-23).
- [04 No pisar ediciones manuales](issues/04-no-pisar-ediciones-manuales.md) — lista de hashes publicados para plantillas (solo crece, test la exige); bloques con el formato de marcas de `orchestration-ior` (hash en la marca, `v:`); editado = aviso con diff; `@AGENTS.md` sin marcas; hash sobre LF, sin bajar de versión; test de 13 puntos más `test_lista_de_hashes` (2026-10-23).
- [07 Orden de adopción y forma de aplicar](issues/07-orden-de-adopcion.md) — `--upgrade` por repo, aborta si toca un archivo sucio, commitea sus rutas y pushea siempre (agente pide OK); solo en la rama default (clave `rama`, `origin/HEAD`, `main`/`master`); orden mf-moe → Portfolio, ssb, ChatBot → F2 → Estudio → orchestration → Adepor; adoptado = veredicto de `doctor`; test de 16 puntos (2026-10-23).

## Fog

- La implementación de las áreas depende de `orchestration-69j`. Este esfuerzo no puede escribir áreas hasta que ese código exista.
- Nadie midió cuántas veces el usuario edita a mano las plantillas de `docs/sdlc/`. Hoy todas las copias sondeadas son idénticas a `sdlc/templates/`.
- Las convenciones de otros agentes (Cursor, Codex) en `AGENTS.md` pueden chocar con el puntero del SDLC.
