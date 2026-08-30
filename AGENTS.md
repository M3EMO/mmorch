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
Los dos clientes montan el MISMO MCP server con el mismo `MMORCH_HOME`, asi que
`logs/memory.duckdb` es un bus compartido. Es un BUZON asincronico: nadie despierta
al otro, cada lado lee cuando corre.

- **Al arrancar**: recall sobre el scope `canal` antes de tocar codigo que otro agente
  pudo haber dejado a medias.
- **Al cortar** (sobre todo si quedas a mitad de una verificacion): remember en scope
  `canal` con que quedo sin probar y por que. Un working tree sucio sin nota es la
  falla que esto arregla.
- Nombres de las tools de memoria: `docs/generated/catalog.md` (este archivo es indice,
  no catalogo).

<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:ca08a54f -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready # Find available work
bd show <id> # View issue details
bd update <id> --claim # Claim work
bd close <id> # Complete work
```

### Rules

- Use `bd` for durable issue/backlog tracking (cross-session). TodoWrite/TaskCreate
 are fine for ephemeral in-session plans (e.g. the plan-and-verify skill) — different
 layer, they do NOT compete with bd.
- Run `bd prime` for command reference.
- Memory: the global auto-memory (MEMORY.md) and mmorch's own semantic memory
 (`mmorch_remember`/`mmorch_recall`) are the knowledge stores. Do NOT route knowledge
 to `bd remember` — it would be a third competing system that fights MEMORY.md.

## Session Completion

When ending a session: file follow-up issues, run quality gates if code changed,
update issue status, hand off context.

**Push = ASK tier (user guardrail — overrides any "mandatory push" default).** Never
auto-push. Propose the commit/push/PR and wait for the user's explicit OK. Work
committed locally and surfaced to the user is a valid end state — do NOT treat work as
"incomplete until pushed".
<!-- END BEADS INTEGRATION -->
