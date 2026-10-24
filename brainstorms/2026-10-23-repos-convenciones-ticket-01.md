# Repos-convenciones, ticket 01 (qué costumbres entran): Brainstorm / Discovery Notes
Date: 2026-10-23 · Goal: decidir qué costumbres mantiene al día el mecanismo de actualización (`.scratch/repos-convenciones/issues/01-que-costumbres-entran.md`, bd `orchestration-4u6`).

## Summary / key decisions

- `docs/sdlc/gates/` no entra al contrato del repo; los gates pasan a mmorch (`orchestration-bpn`). `contract_version` pasa al ticket 03 (P2).
- Constitución: sin default nuevo; aviso del oráculo, nunca desvío; el usuario la declara por repo (P3).
- Artefactos de corrida en `docs/sdlc/`: fuera del contrato; limpieza de main en bd `orchestration-eeq` (P4).
- Grupos que entran: SDLC + SDD + `.gitignore` de secretos + `bd init` (P1, usuario). Forma de `bd init`: la decide `orchestration-ior` (P1c).

## Q&A log

### P1 — Qué grupos de costumbres entran
- Asked: ¿opción A (solo SDLC), B (SDLC + SDD) o C (los tres grupos)?
- Captured: el usuario eligió "SDLC + SDD + .gitignore + bd init". El resto del grupo `new-project` (`docs/claude/`, `tests/README.md`) queda fuera.
- Flags: el usuario preguntó si Claude está de acuerdo. Claude acuerda con los cuatro y propone una forma distinta para `bd init` (P1b).

### P1b — Cómo entra `bd init`
- Asked: ¿el mecanismo corre `bd init` solo, o solo reporta y el usuario lo corre?
- Captured: el usuario pregunta si se puede crear `.beads/` sin reescribir `CLAUDE.md` y `AGENTS.md`, o con un append.
- Hecho verificado (`bd init --help`, bd 1.3.1): existen `--skip-agents` (no toca AGENTS.md ni la config de Claude/Codex/Cursor), `--init-if-missing` (idempotente) y `--skip-hooks`.
- Pendiente: confirmación del usuario sobre la forma propuesta (P1c).
- Flags: el usuario quiere una estructura más limpia de `CLAUDE.md` y `AGENTS.md`. Claude recomienda un bd aparte -> usuario decide.

### P1c — Forma de `bd init` y estructura de los md
- Asked: ¿`bd init --skip-agents` más un bloque con marca en `AGENTS.md`, con o sin `--skip-hooks`? ¿Bd aparte para la estructura de los md?
- Captured: el usuario rechaza la forma propuesta porque "no resuelve del todo la problemática": el bloque corto pierde las reglas y los hooks de bd. Quiere un comando que agregue por append las reglas y los hooks de bd, sin reescribir. Ese comando se arma en el bd de estructura de los md.
- Decisión: bd `orchestration-ior` creado (2026-10-23), relacionado con `orchestration-4u6`. Contiene la estructura limpia de `CLAUDE.md`/`AGENTS.md` y el comando de append de reglas y hooks de bd.
- Consecuencia para el ticket 01: `bd init` entra al contrato, y su forma de aplicación la define `orchestration-ior`. El oráculo de este esfuerzo marca un repo sin `.beads/`.

### P2 — `docs/sdlc/gates/` y `contract_version`
- Asked: ¿`docs/sdlc/gates/` sale del contrato y `contract_version` pasa al ticket 03?
- Captured: el usuario dice "cambiemos a que los gates pasen en mmorch de forma que siga siendo el banco de medicion". `contract_version` pasa al ticket 03.
- Decisión: `docs/sdlc/gates/` no entra al contrato del repo. Los gates pasan a mmorch, que mide cada gate.
- Hechos: `sdlc/pipeline.py` ya importa de mmorch (`checkers._mutants`, `impacto`, `sandbox`, `providers`). mmorch importa `sdlc` en `auto_repair`, `hardening`, `juez_sombra`, `project_repair` y `refutacion`: el ciclo de imports ya existe.
- Matiz confirmado por el usuario: el chequeo y la medición del gate pasan a mmorch; la orquestación queda en `sdlc/`; los umbrales siguen en `sdlc.toml`.
- Decisión: bd `orchestration-bpn` creado (2026-10-23) para el traslado, relacionado con `orchestration-4u6`, con sesión aparte.

### P3 — Constitución fuera de orchestration
- Asked: ¿qué constitución lee el pipeline en repos sin `GOAL.md`?
- Captured: el usuario confirma la recomendación.
- Decisión: el mecanismo nunca crea un archivo de constitución. El oráculo reporta "sin constitución" como aviso, no como desvío. El usuario declara `constitution = "<ruta>"` en cada repo con reglas fijas. `CLAUDE.md` no es default (cambia seguido; `orchestration-ior` puede reorganizarlo).

### P4 — Artefactos de corrida en `docs/sdlc/`
- Asked: ¿mesa de trabajo o historia? ¿bd de limpieza?
- Captured: el usuario confirma que quedan fuera del contrato y pide el bd de limpieza.
- Decisión: el mecanismo no toca `spec.md`, `plan.md`, `run-log.json`, `suite*.log`, `supervision.md` de `docs/sdlc/`, y el oráculo no los chequea. bd `orchestration-eeq` (2026-10-23): la mesa no llega a main al mergear. Un `.gitignore` queda descartado porque rompe el checkpoint `run-log.json`.

## Open flags (pending input)
