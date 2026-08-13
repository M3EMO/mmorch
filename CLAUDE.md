# mmorch — Orquestacion Multi-Modelo

(Movido desde ~/.claude/CLAUDE.md global el 2026-06-11. Se carga automaticamente
al trabajar en este directorio. La regla de ruteo corta vive en el CLAUDE.md global.)

Recurso escaso = **cupo** del plan Claude (no dolares). Generacion masiva y
verificacion se delegan a modelos externos baratos por API para **liberar cupo**.
Libreria: `~/.claude/orchestration/` (paquete `mmorch`, Python). Tambien expuesta
como MCP server `mmorch`. **Lista de tools: NO se duplica aca — la fuente unica es
`mcp_server.py`** (46 tools a 2026-08; grep `mmorch_` ahi o mira el listado MCP de la
sesion). Modulos cognitivos (de bitterbot, reimplementados, 2026-06): retencion (decay
Ebbinghaus + Zeigarnik) y reconsolidacion; ver memoria [[mmorch-cognitive-modules]].
Version: la de `pyproject.toml` (fuente unica; no citar tags aca). Reload Claude Code
para cargar tools nuevos.

## Decision dura: cupo (Workflow nativo) vs API barata (mmorch)
- **Flujo recurrente/entendido** (bulk gen, verificar, rutear repetido) → `mmorch`
  (API externa, **cero cupo**). Es la palanca de ahorro.
- **Flujo novel/one-off de alto valor** (perspectivas independientes, goal drift) →
  Workflow nativo de Claude Code (gasta cupo). Sujeto al opt-in gate.
- Nunca delegar a `mmorch` lo que necesita contexto/juicio del orquestador (Fase 0/1,
  sintesis critica, tie-break) — eso es Opus.

## Reglas invariantes (del diseño §4, §7, §8)
- **Cross-family obligatorio.** En todo par generador→verificador o competidor→juez,
  las dos puntas en **familias distintas** (decorrelacionar errores). DeepSeek↔Google
  es el par valido del MVP; Opus desempata. `adversarial_verify()` lo enforcea y tira
  error si coinciden familia.
- **Regla OneFlow.** Nunca multi-agente homogeneo. Si todos los nodos serian el mismo
  modelo → un solo agente. Multi-agente solo si es heterogeneo de familia.
- **Anti-sicofancia.** El verificador refuta por default; el acuerdo no es confirmacion.
- **Heterogeneidad > rondas.** Menos iteracion, mas diversidad de familias.
- **Anti-reward-hacking.** En cualquier loop optimizante (`hillclimb`), el `score` es
  checker determinista o comando corrible — NUNCA LLM-judge.
- **Observabilidad.** Todo nodo loggea a `~/.claude/orchestration/logs/metrics.jsonl`
  (tokens, costo, latencia, familia). Sin metricas no se valida el break-even.

## Modelos activos
- Roles, modelos por rol (DEFAULT_VERIFIER etc.) y precios: `mmorch/config.py` es la
  **fuente unica** — no duplicar nombres de modelo aca (drift garantizado; el audit
  2026-08 encontro este doc citando un verifier legacy +67% mas caro que el real).
- Invariante que si vive aca: bulk=DeepSeek, verificador=Google (cross-family).

## Capacidades
Catalogo, internals y gotchas de implementacion: `docs/capabilities.md` (referencia,
se consulta bajo demanda). Si diverge del codigo, gana el codigo.

Lo unico que hace falta saber aca: hay feedback loop real (`mmorch/feedback.py` —
bandit + calibracion) y memoria 2 capas (`mmorch/memory.py` — episodica inmutable +
semantica). `tests/` es el gate para promover codigo nuevo.

## Auto-evolución contenida (Rasputin gated)
mmorch se auto-audita (`AUDIT_*.md`) y se auto-idea capacidades (`INNOVATION_ROADMAP_*.md`)
usándose a sí mismo: fan_out (divergir) → adversarial_verify cross-family (refutar) → Opus
(tie-break). NUNCA auto-modifica vivo sin tests verdes + gate humano. Detectó su propio gap
(verdict no loggeado) y lo cerró.

## Patrones (catalogo COMPLETO)
`fan_out`, `adversarial_verify`, `route`, `cascade`, `ensemble_verify`, `tournament`,
`bucket_rank`, `loop_until_done`, `classify_and_act`, `hillclimb`. `generate-and-filter`
se compone con estos. Que hace cada uno y cuando elegirlo: `docs/capabilities.md`.

## Schema-gates (§9, `mmorch/schema.py`)
`gated_json()` = validado-o-rechaza. Library-only, OPT-IN — no forzado en
`adversarial_verify` (ahi el skeptic-default unparse→failed es mas seguro que una
excepcion). Detalle: `docs/capabilities.md`.

## Pendiente / backlog
ablacion §18.4 (validar empíricamente config B DeepSeek↔Google vs alternativas —
requiere API real + métricas, es research no código). No escalar sin métricas verdes
(diseño §14). `mmorch_innovate` se puede correr periódicamente: cada vez, `learn`
tiene más datos y el roadmap se afina solo.


<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:ca08a54f -->
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
