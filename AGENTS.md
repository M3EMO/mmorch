# AGENTS.md — mmorch

Standard cross-agent entry point (Codex, OpenCode, Cursor, etc.). Claude Code reads
`CLAUDE.md`; this is the portable pointer so ANY agent gets the invariants before editing.
**This file is an INDEX, not a duplicate.** Mapa de dueños: `docs/SOURCES.md`.
Contrato: `GOAL.md`. Vocabulario: `CONTEXT.md`. Catálogo: `docs/generated/catalog.md`.

## What mmorch is
Deterministic Python orchestration **library** (package `mmorch/`) + MCP server. Offloads
bulk generation and cross-family verification to cheap external model APIs to conserve
**Claude plan quota ("cupo")**. The orchestrator (Opus/Fable) conducts and tie-breaks;
it is never an external node.

## Hard invariants
Read `GOAL.md` before any change. Violating an invariant rejects the change. Do not
restate the list here — it will drift. In one line: red zone is never autonomous;
OneFlow on subjective work; anti-sycophancy; no scope without a measured metric;
reversibility × blast-radius + fitness + budget.

## Where to look (don't re-derive)
- `GOAL.md` — north-star contract. **Authoritative for invariants.**
- `CLAUDE.md` — routing (cupo vs API) for Claude Code.
- `docs/capabilities.md` — when to pick a pattern.
- `docs/coding-principles.md` — how to write code here.
- `docs/generated/catalog.md` — what exists (generated).
- `mmorch/config.py` — models, families, prices.
- `mmorch/evolve.py` / `mmorch/checkers.py` / `mmorch/nodes.py` — engines, not essays.
- Research: `vault/`. Snapshots: `docs/production-readiness/` (dated, not SSOT).

## Editing rules
- Tests live in `tests/`; run `.venv/Scripts/python.exe -m pytest tests/ -q` (Windows)
  before committing. `python -m mmorch.docgen --check` is the docs ratchet.
- Don't commit/push unless asked.
- After a meaningful edit, update the **owner** named in `docs/SOURCES.md`. Never
  copy a catalog into a contract file. Module docstring stays one line.

## Handoff entre agentes (Cursor <-> Claude Code)
Mismo MCP, mismo `MMORCH_HOME`. Nadie despierta al otro: el usuario cambia de
ventana. El hilo ordenado es `logs/canal.jsonl` (`mmorch.canal`), no la memoria
destilada (un turno destilado pierde "que falta probar").

- **Al arrancar**: leer los ultimos turnos del canal antes de tocar codigo a medias.
- **Al cortar**: post `kind=handoff` con comando de verificacion + valor esperado.
- **Pedir juicio al otro**: post `kind=ask` y esperar a que el usuario abra ese cliente.
- Bulk / verify / checkers: mmorch, no el otro agente.
- Nombres de tools: `docs/generated/catalog.md`.

<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:1105d646 -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Use `bd remember` for persistent knowledge — do NOT use MEMORY.md files

**Architecture in one line:** issues live in a local Dolt DB; sync uses `refs/dolt/data` on your git remote; `.beads/issues.jsonl` is a passive export. See https://github.com/gastownhall/beads/blob/main/docs/core-concepts/sync-concepts.md for details and anti-patterns.

## Agent Context Profiles

The managed Beads block is task-tracking guidance, not permission to override repository, user, or orchestrator instructions.

- **Conservative (default)**: Use `bd` for task tracking. Do not run git commits, git pushes, or Dolt remote sync unless explicitly asked. At handoff, report changed files, validation, and suggested next commands.
- **Minimal**: Keep tool instruction files as pointers to `bd prime`; use the same conservative git policy unless active instructions say otherwise.
- **Team-maintainer**: Only when the repository explicitly opts in, agents may close beads, run quality gates, commit, and push as part of session close. A current "do not commit" or "do not push" instruction still wins.

## Session Completion

This protocol applies when ending a Beads implementation workflow. It is subordinate to explicit user, repository, and orchestrator instructions.

1. **File issues for remaining work** - Create beads for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **Handle git/sync by active profile**:
   ```bash
   # Conservative/minimal/default: report status and proposed commands; wait for approval.
   git status

   # Team-maintainer opt-in only, unless current instructions forbid it:
   git pull --rebase
   git push
   git status
   ```
5. **Hand off** - Summarize changes, validation, issue status, and any blocked sync/commit/push step

**Critical rules:**
- Explicit user or orchestrator instructions override this Beads block.
- Do not commit or push without clear authority from the active profile or the current user request.
- If a required sync or push is blocked, stop and report the exact command and error.
<!-- END BEADS INTEGRATION -->

## SDLC (pipeline de 6 etapas)

Este repo construye features con el paquete `sdlc` (skill `/project`). Contrato del repo en `sdlc.toml`; artefactos de cada corrida en `docs/sdlc/` de la review branch.

Gates que bloquean en la etapa 5: suite total sin fallos nuevos (el baseline se mide SIN los tests de aceptacion), lint
y tipos sin hallazgos nuevos por archivo contra la base, codigo muerto en un `.py` nuevo (`cobertura_min`, default 0.8),
mutacion solo sobre las lineas cambiadas (`mutation_min`; sin la clave, observa), y assets si el repo declara `[sprites]`.
La revision de Claude bloquea dejando un test, que tambien se corre en repos que no son Python. `reviewer_cmd` cambia el
agente que revisa y corrige (prompt por stdin, `{modo}` = plan|edit). El juez visual de sprites solo observa hasta tener
50 assets etiquetados con kappa 0.6. El veredicto humano del test de aceptacion no se registra en nombre del usuario.

<!-- BEGIN SDLC bd-propias v:1 hash:39a2efef -->
## Reglas propias de bd

- Estas reglas mandan sobre el bloque de bd.
- TodoWrite y TaskCreate valen para planes efímeros de una sesión; bd guarda el backlog durable.
- La memoria vive en MEMORY.md y en la memoria semántica de mmorch; nunca en `bd remember`.
- Push = ASK: proponé el commit, el push o el PR y esperá el OK explícito del usuario.
- Un trabajo commiteado localmente y reportado al usuario es un final válido.
<!-- END SDLC bd-propias -->

## Reglas propias de bd — reintegradas del bloque ca08a54f (2026-10-23)

- Use `bd` for durable issue/backlog tracking (cross-session). TodoWrite/TaskCreate
  are fine for ephemeral in-session plans (e.g. the plan-and-verify skill) — different
  layer, they do NOT compete with bd.
- **Particion bd vs `.scratch/` (regla unica):** bd = backlog durable cross-session;
  `.scratch/<effort>/` = mapas wayfinder + tickets DE ese esfuerzo (vida = el esfuerzo).
  Un item de `.scratch/` que sobrevive a su esfuerzo se PROMUEVE a bd (un issue con
  puntero al file) — nunca vive en los dos lados a la vez. Detalle operativo:
  `docs/agents/issue-tracker.md`.
- Run `bd prime` for command reference.
- Memory: the global auto-memory (MEMORY.md) and mmorch's own semantic memory
  (its remember/recall tools) are the knowledge stores. Do NOT route knowledge
  to `bd remember` — it would be a third competing system that fights MEMORY.md.
When ending a session: file follow-up issues, run quality gates if code changed,
update issue status, hand off context.
**Push = ASK tier (user guardrail — overrides any "mandatory push" default).** Never
auto-push. Propose the commit/push/PR and wait for the user's explicit OK. Work
committed locally and surfaced to the user is a valid end state — do NOT treat work as
"incomplete until pushed".
