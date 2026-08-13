# mmorch — Capacidades y patrones (referencia)

Extraído de `CLAUDE.md` el 2026-08-13. **Esto es referencia, no directiva**: describe
qué existe y cómo está implementado. Las reglas que el agente debe obedecer en cada
turno viven en `CLAUDE.md`; acá está el detalle que se consulta bajo demanda.

Fuentes únicas que este doc NO duplica: lista de tools → `mcp_server.py`; modelos,
roles y precios → `mmorch/config.py`; versión → `pyproject.toml`.

## Capacidades

Snapshot 2026-06-07. Si diverge del código, gana el código.

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

## Patrones — detalle

Catálogo de 8 COMPLETO + cascade. Espejado en
`~/.claude/skills/dynamic-workflows/workflows/hillclimb.js` para el lado cupo.

- `fan_out` — bulk paralela.
- `adversarial_verify` — escéptico cross-family.
- `route` — confidence-gated.
- `cascade` — escalada por umbral (bandit aprende el umbral).
- `ensemble_verify` — K escépticos + voto mayoría, empate→falla.
- `tournament` — best-of-N pairwise, juez cross-family, empate→Opus.
- `bucket_rank` — graduar set grande en tiers, O(n), items nunca se pierden.
- `loop_until_done` — loop-until-dry, dedup contra todo lo visto, library-only.
- `classify_and_act` (`mmorch/classify.py`) — front-door: router barato clasifica el
  request en N clases + self-conf, dispara handler si hay y conf≥threshold, si no o si
  la confianza es baja ESCALA a Opus; handlers son callables componibles con otros
  patrones; confidence-gated anti-misfire.
- `hillclimb` (`mmorch/hillclimb.py`) — optimización sobre métrica escalar con rubric
  corrible como entorno Y como reward del bandit; distinto de `loop_until_done` (que es
  discovery) y de `pursue_goal` (que es binario); library-only.

`generate-and-filter` se compone con estos.

## Schema-gates (§9, `mmorch/schema.py`)

`gated_json(model, messages, schema)` = validado-o-rechaza: valida output contra
JSON-Schema mínimo embebido (sin dep), reintenta 1 vez con el error como feedback,
tira `SchemaGateError` si se agota. bool NO cuela como number. Library-only, OPT-IN:
no forzado en `adversarial_verify`, porque ahí el skeptic-default unparse→failed es
más seguro que una excepción.
