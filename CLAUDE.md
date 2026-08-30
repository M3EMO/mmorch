# mmorch — Orquestacion Multi-Modelo

(Movido desde ~/.claude/CLAUDE.md global el 2026-06-11. Se carga automaticamente
al trabajar en este directorio. La regla de ruteo corta vive en el CLAUDE.md global.)

Mapa de fuentes: `docs/SOURCES.md`. Vocabulario: `CONTEXT.md`. Invariantes: `GOAL.md`
(no se reescriben aca). Catálogo vivo: `docs/generated/catalog.md`.

Recurso escaso = **cupo** del plan Claude (no dolares). Generacion masiva y
verificacion se delegan a APIs externas baratas para **liberar cupo**.
Libreria: `~/.claude/orchestration/` (paquete `mmorch`). Tambien MCP `mmorch`.
Lista de tools: `mcp_server.py` (vista: catalogo generado). Version: `pyproject.toml`.
Reload Claude Code para cargar tools nuevos.

## Decision dura: cupo (Workflow nativo) vs API barata (mmorch)
- **Flujo recurrente/entendido** (bulk gen, verificar, rutear repetido) → `mmorch`
  (API externa, **cero cupo**). Es la palanca de ahorro.
- **Flujo novel/one-off de alto valor** (perspectivas independientes, goal drift) →
  Workflow nativo de Claude Code (gasta cupo). Sujeto al opt-in gate.
- Nunca delegar a `mmorch` lo que necesita contexto/juicio del orquestador (Fase 0/1,
  sintesis critica, tie-break) — eso es Opus.

## Reglas de turno (apuntes; el contrato es GOAL.md)
- **Cross-family / OneFlow.** Par generador→verificador (o competidor→juez) en
  familias distintas si la tarea es subjetiva. Same-family solo en checkeable
  ruteado a un checker. `adversarial_verify()` tira error si coinciden familia
  en subjetivo. Anti-sicofancia: el verificador refuta por default.
- **Anti-reward-hacking.** En `hillclimb`, `score` = checker o comando — nunca LLM-judge.
- **Observabilidad.** Cada nodo loggea a `logs/metrics.jsonl`. Sin metricas no hay
  break-even.
- **Modelos.** Roles y precios: `mmorch/config.py`. Aca solo el invariante de
  familias: bulk y verificador no comparten familia.

## Donde leer el resto
- Capacidades (cuando elegir un patron): `docs/capabilities.md`
- Feedback y memoria (como estan hechos): `mmorch/feedback.py`, `mmorch/memory.py`
- Tests: `tests/` es el gate para promover codigo nuevo
- Auto-evolucion: gated; nunca auto-modifica vivo sin tests + gate humano.
  Motor: `mmorch/evolve.py`. Research: `vault/`
- Prosa STE: `python tools/ste-lint.py docs/*.md --fail-over 5` (`--lang es` ok)

Ablacion cross-family: research en vault, no un backlog de codigo. No escalar
sin metricas verdes.


<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:ca08a54f -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id>        --claim  # Claim work
bd close <id>         # Complete work
```

### Rules

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
