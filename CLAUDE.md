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
- **Observabilidad.** Todo nodo loggea a `~/.claude/orchestration/logs/metrics.jsonl`
  (tokens, costo, latencia, familia). Sin metricas no se valida el break-even.

## Modelos activos
- Roles, modelos por rol (DEFAULT_VERIFIER etc.) y precios: `mmorch/config.py` es la
  **fuente unica** — no duplicar nombres de modelo aca (drift garantizado; el audit
  2026-08 encontro este doc citando un verifier legacy +67% mas caro que el real).
- Invariante que si vive aca: bulk=DeepSeek, verificador=Google (cross-family).

## Capacidades (2026-06-07)
- **Patrones:** `fan_out` (bulk paralela), `adversarial_verify` (escéptico cross-family),
  `ensemble_verify` (K escépticos + voto mayoría, empate→falla), `route` (confidence-gated:
  barato responde + self-score, `escalate=True` solo si baja confianza → Opus interviene
  solo cuando hace falta).
- **Inteligencia:** `learn` (`mmorch/learn.py` — lee su propio metrics.jsonl, recomienda
  defaults más baratos + flags de latencia/observabilidad, gated no auto-switch),
  `innovate` (`mmorch/innovate.py` — ideate→screen, mmorch se idea capacidades nuevas y
  las filtra cross-family).
- **Feedback loop (keystone, `mmorch/feedback.py`):** el lazo que faltaba (la 'loss'
  ausente). `record_outcome` (label real por decisión, reward [0,1]), `ThompsonBandit`
  (Beta posterior gradient-free, elige modelo/umbral, aprende del outcome — wireado en
  `cascade` para aprender el umbral de escalada), `calibration` (ECE conf-predicha vs
  realidad — surfaceado en `learn.recommend`: ECE>0.15 ⇒ la self-CONFIDENCE miente ⇒
  subir umbrales). NO entrena redes: estadística bayesiana sobre conteos. El lazo se
  CIERRA afuera (caller hace `bandit.update`+`record_outcome` con el label; la conf
  auto-reportada NO es el reward — anti-sicofancia). **`hillclimb`
  (`mmorch/hillclimb.py`, Martin 2026 "Designing loops") cierra el lazo SIN label
  humano:** loop medir→proponer→probar sobre métrica escalar; el reward por ronda ES
  el rubric corrible (mejoró=1/no=0, source="rubric") — con `arms`, el bandit elige
  generador por ronda y aprende cuál mejora más seguido. Regla anti-reward-hacking:
  `score` = checker determinista/comando, NUNCA LLM-judge. Library-only.
- **Memoria episódica+semántica (`mmorch/memory.py`, DuckDB 2 capas):** diseño
  verificado cross-family (Gemini refutó, Opus trió para single-user/localhost).
  `episodic` (log append-only INMUTABLE) + `semantic` (notas destiladas +
  embedding bge-small 384d local vía fastembed, cero key/cero $, tombstone). `remember`
  = pipeline raw→`distill` (Thought-Retriever, modelo barato condensa o SKIP)→verify
  cross-family opcional (nota infiel ⇒ solo queda raw, invariante 7)→persist+embed.
  `recall` clínico 2-stage: COARSE (scope-chain jerárquico task_id<subsector<project_id
  <mmorch_self<global + recencia, SIN keyword-gate — FIX A) → FINE (embedding rerank) →
  fallback episodic raw (FIX B). Embeddings versionados (emb_model,dim — FIX C). Degrada
  graceful a coarse-only sin fastembed. >100k notas: extensión `vss`/HNSW documentada.
  **Verification coverage (Martin 2026):** columna `verified` en semantic
  (`remember(verify=True)` que pasa el escéptico ⇒ verified=TRUE); `stats()` reporta
  `verification_coverage` y `learn.recommend` flaggea <50% con ≥5 notas. OJO DuckDB:
  `ADD COLUMN IF NOT EXISTS` PISA valores existentes con el default — migrar via
  information_schema check. **`consolidate()` (mantenimiento cada ~10 sesiones):**
  merge near-dups por scope (texto idéntico o cosine ≥0.92), keeper
  verificada>reciente, episodic intocable, corrida auditada como evento episódico;
  over_budget (>50KB) solo flaggea, nunca borra por tamaño. MCP `mmorch_consolidate`
  (dry-run default, `apply=true` tombstonea).
- **Utilidad:** `memo`/`Memo` (`mmorch/cache.py` — cache content-hash, salta re-gen/re-verify).
- **Robustez core:** fan_out graceful (1 fallo no mata batch), error-logging en call,
  timeout 60s, max_tokens 16384, parse-verdict anti-sicofancia (`passed:"false"`→False),
  verdict logging (habilita proxy de calidad para learn).
- **Red de seguridad:** `tests/` (197 tests, API/embeddings mockeados o locales) = gate
  para promover código nuevo. Git versionado (`~/.claude/orchestration`, tag `v1.1`).

## Auto-evolución contenida (Rasputin gated)
mmorch se auto-audita (`AUDIT_*.md`) y se auto-idea capacidades (`INNOVATION_ROADMAP_*.md`)
usándose a sí mismo: fan_out (divergir) → adversarial_verify cross-family (refutar) → Opus
(tie-break). NUNCA auto-modifica vivo sin tests verdes + gate humano. Detectó su propio gap
(verdict no loggeado) y lo cerró.

## Patrones completos (los 8 del catalogo + extras)
fan_out, adversarial_verify, route, cascade, ensemble_verify, **tournament**
(best-of-N pairwise, juez cross-family, empate→Opus), **bucket_rank** (graduar set
grande en tiers, O(n), items nunca se pierden), **loop_until_done** (loop-until-dry,
dedup contra todo lo visto, library-only), **classify_and_act** (`mmorch/classify.py`
— front-door: router barato clasifica el request en N clases + self-conf, dispara
handler si hay y conf≥threshold, si no/baja-conf ESCALA a Opus; handlers son callables
componibles con otros patrones; confidence-gated anti-misfire), **hillclimb**
(`mmorch/hillclimb.py` — optimización sobre métrica escalar con rubric corrible como
entorno Y como reward del bandit; distinto de loop_until_done que es discovery y de
pursue_goal que es binario; library-only). generate-and-filter se
compone con estos. **Catalogo de 8 COMPLETO + cascade** (espejado en
`~/.claude/skills/dynamic-workflows/workflows/hillclimb.js` para el lado cupo).

## Schema-gates (§9, `mmorch/schema.py`)
`gated_json(model, messages, schema)` = validado-o-rechaza: valida output contra
JSON-Schema mínimo embebido (sin dep), reintenta 1 vez con el error como feedback,
tira `SchemaGateError` si se agota. bool NO cuela como number. Library-only, OPT-IN
(no forzado en adversarial_verify: ahí el skeptic-default unparse→failed es más seguro
que excepción).

## Prose-gate (`tools/ste-lint.py`)
Linter STE de prosa (vendored de woosal1337/blog ep01, extendido con `--lang es`).
Gate determinístico para docs generados — mismo rol que ruff/mypy pero para markdown.
Regex + wordlists, stdlib puro. Uso: `python tools/ste-lint.py docs/*.md --fail-over 5`
(exit 1 si per100w supera umbral); `--lang es` para prosa en español; `--json`; stdin OK.
Correr sobre README/specs/notas antes de commitearlos.

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
