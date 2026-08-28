# 02 — Contratos de interfaz públicos (mmorch)

Auditoría 2026-08-27. Fuentes primarias: `mcp_server.py` (856 líneas, 46 tools),
`mmorch/__init__.py` (API pública, 135 líneas), y los módulos de contrato de archivo
(`metrics.py`, `feedback.py`, `memory.py`, `goal.py`, `budget.py`, `mcp_telemetry.py`).
Referencias en formato `file:line`. No se editó código.

---

## 1. Contrato transversal de las 46 MCP tools

- **Transporte**: FastMCP stdio (`mcp_server.py:54`, `mcp.run()` en `:856`).
- **Retorno**: TODAS las tools devuelven un `str` que es JSON serializado
  (`json.dumps(..., ensure_ascii=False)`). Ninguna devuelve objetos estructurados MCP.
- **Telemetría**: `instrument(mcp)` (`mcp_server.py:55-57`) envuelve el decorador
  `FastMCP.tool()` y loguea CADA invocación (nombre, ok, dur_s, err[:200]) a
  `logs/mcp_calls.jsonl` (`mmorch/mcp_telemetry.py:24-47`). El logging jamás rompe la
  tool (`except: pass`, `mcp_telemetry.py:32-33`).
- **Contrato de error INCONSISTENTE** (ver §5.1): algunas tools devuelven
  `{"error": "..."}` como JSON (p.ej. `mmorch_review_code`, `mcp_server.py:198-207`);
  otras dejan propagar la excepción Python cruda al framework MCP (p.ej. `mmorch_check`
  → `KeyError` de `checkers.check`, `checkers.py:606-607`; `mmorch_cascade` →
  `IndexError`/`ValueError` en `mcp_server.py:239`).
- **Gasto**: cada docstring declara si gasta API externa ($, nunca cupo Claude) o es
  determinista/read-only. Se marca abajo como **[$]** (gasta API), **[RO]** (read-only,
  cero gasto), **[DET]** (escribe local, determinista, cero API).

### 1.1 Generación / verificación [$]

| Tool | Línea | Params (defaults) | Retorno (JSON) | Side effects |
|---|---|---|---|---|
| `mmorch_fan_out` | 61 | `prompts: list[str]`, `gen_model=DEFAULT_GENERATOR`, `system: str\|None` | lista de `{text, in_tokens, out_tokens, cost_usd}` | append `logs/metrics.jsonl`; gasto API |
| `mmorch_adversarial_verify` | 87 | `artifact`, `rubric`, `gen_model`, `verifier_model=DEFAULT_VERIFIER`, `task_kind="subjective"` | `{passed, confidence, refutations, verifier_model, cost_usd}` | metrics; `task_kind="subjective"` + par same-family ⇒ **raise** |
| `mmorch_ensemble_verify` | 279 | `artifact`, `rubric`, `gen_model`, `verifier_models: list\|None` | `{passed, confidence, n_passed, n_total, unanimous, escalate, ensemble_degraded, refutations, cost_usd}` | metrics; K verificadores en paralelo |
| `mmorch_route` | 162 | `prompt`, `gen_model`, `threshold=0.7`, `models: list\|None` | `{answer, confidence, escalate, model, cost_usd}` | metrics; con `models` consulta el bandit de intuition |
| `mmorch_cascade` | 230 | `prompt`, `steps: list[list]\|None` (pares `[model, threshold]`) | `{answer, confidence, resolved_step, escalate, models_used, cost_usd}` | metrics |
| `mmorch_tournament` | 430 | `candidates: list[str]`, `criterion`, `gen_model`, `judge_model=DEFAULT_VERIFIER` | `{winner, escalate, rounds, comparisons, cost_usd}` | metrics; empate ⇒ `escalate=true` |
| `mmorch_bucket_rank` | 449 | `items: list[str]`, `rubric`, `tiers: list\|None` | `{by_tier, graded, cost_usd, n_failed}` | metrics; grade fallido cae al tier más bajo (nunca pierde items) |
| `mmorch_classify` | 466 | `request`, `classes: dict{name: desc}`, `router_model=DEFAULT_ROUTER` | `{cls, confidence, cost_usd}` | metrics; solo etiqueta (los handlers son API de librería `classify_and_act`) |
| `mmorch_cynefin` | 483 | `request`, `router_model`, `threshold=0.6` | `{domain, confidence, strategy, escalate, cost_usd}` | metrics; `domain ∈ {clear,complicated,complex,chaotic}` |
| `mmorch_innovate` | 314 | `context`, `lenses: list[str]`, `ask`, `rubric` | lista de `{idea, survives, confidence, objection}` | metrics; fan_out + screen adversarial |
| `mmorch_review_code` | 189 | `code=""`, `path=""` (path solo si code vacío) | `{path, findings:[{principle, severity, line, problem, fix}], n_raw, n_confirmed, dropped}` o `{error}` | metrics; **gate de secretos** `_SECRET_NAME_RX` (`:184-185`) rechaza paths tipo `.env`/`*.key`/`*secret*` |

### 1.2 Spec / prompt-sharpening [$]

| Tool | Línea | Params | Retorno | Notas |
|---|---|---|---|---|
| `mmorch_spec_interview` | 500 | `request`, `n=5` | `{questions, cost_usd}` | preguntas de GOAL, el orquestador las hace al humano |
| `mmorch_build_spec` | 510 | `request`, `answers=""` | `{spec, accepted_inferences, open_questions, dropped, escalate, quarantined, raw_draft, verifier_model, cost_usd}` | **cuarentena**: si el drafter contrabandeó scope, `spec` se blanquea y el draft sucio queda en `raw_draft` — nunca entregar `raw_draft` como spec limpio |
| `mmorch_perfect` | 829 | `request`, `n=4` | `{spec, open_questions, goal_questions, accepted_inferences, dropped, escalate, quarantined, raw_draft, verifier_model, cost_usd}` | headless (interview + build_spec en un call); gemelo automatizado del skill interactivo `/perfect` |

### 1.3 Memoria (DuckDB) — write [$ chico] / read-write local [DET]

| Tool | Línea | Params | Retorno | Side effects |
|---|---|---|---|---|
| `mmorch_remember` | 332 | `scope`, `episode_text`, `kind="note"`, `verify=False`, `open_loop=False`, `permanent=False` | `{episode_id, note_id, distilled, persisted, refutations}` | INSERT en `episodic` (siempre) + `semantic` (si la destilación no da SKIP / pasa verify); llama modelo barato para destilar **[$]** |
| `mmorch_recall` | 411 | `query`, `scope="global"`, `k=5`, `window_days: float\|None` | lista de `{id, ts, scope, text, score, layer}` (`layer ∈ {semantic, episodic}`) | **[DET]** pero NO read-only: `touch_notes` sube `access_count`/`last_accessed_at` (memory.py:402-403) |
| `mmorch_consolidate` | 673 | `scope=""`, `sim_threshold=0.92`, `forget=False`, `apply=False` | `{merged, tombstoned, forgotten, live_notes, bytes, over_budget, dry_run}` | [DET]; default DRY-RUN (`dry_run=not apply`, `:689`); con apply tombstonea + loguea evento episódico `consolidation` |
| `mmorch_memory_stats` | 694 | — | `{episodic, semantic_live, embedded, verified, verification_coverage, embedding_backend, ...}` | [RO] |
| `mmorch_reinforce` | 703 | `note_id: int`, `boost=3` | `{note_id, boost, ok}` | [DET] UPDATE access_count += boost |
| `mmorch_flag_contradiction` | 713 | `note_id` | `{note_id, ok}` | [DET] `needs_review=TRUE` → recall deja de servirla |
| `mmorch_pending_review` | 724 | `scope=""` | lista `{id, ts, scope, text}` | [RO] |
| `mmorch_resolve_review` | 735 | `note_id`, `drop=False` | `{note_id, dropped, ok}` | [DET] drop ⇒ tombstone; else limpia needs_review |
| `mmorch_close_loop` | 745 | `note_id` | `{note_id, ok}` | [DET] `open_loop=FALSE` |
| `mmorch_open_loops` | 754 | `scope=""` | lista `{id, ts, scope, text}` | [RO] |
| `mmorch_find_tension` | 765 | `scope=""`, `lo=0.82`, `hi=0.92`, `max_per_scope=500` | `{pairs:[{a,b,scope,cosine,question}], skipped:[{scope,n}]}` | [RO]; O(n²) por scope; sin fastembed devuelve vacío |
| `mmorch_forget_preview` | 780 | `scope=""` | `{total_live, eligible, grid:[{lam, forget, would_forget, pct_eligible}]}` | [RO]; gate de métricas antes de `consolidate(forget=true)` |
| `mmorch_vault_write` | 356 | `title`, `body`, `project`, `folder="research"`, `status="seed"`, `confidence=""`, `sources=""`, `tags=""` (sources/tags CSV en string) | `{path, moc}` | escribe nota .md en el vault global, regenera MOC, bridge gist → memoria scope `global`, dispara babel ingest en **thread daemon** (best-effort, `:391-401`) |

### 1.4 Observabilidad / feedback [RO salvo record_outcome]

| Tool | Línea | Params | Retorno | Notas |
|---|---|---|---|---|
| `mmorch_metrics_summary` | 125 | — | `{calls, total_cost_usd, cost_by_family, calls_by_model}` | costo LIFETIME, no compara contra cap mensual |
| `mmorch_error_rates` | 131 | `window_n=200` | `{window_events, by_model, by_family}` con `{calls, rate_limit, budget_cap, timeout, other_error, *_rate, error_rate}` | observabilidad pura, NO rutea |
| `mmorch_budget_status` | 142 | — | `{month, spent, limit, remaining, enforced}` | `enforced=false` ⇔ `MMORCH_MAX_MONTHLY_USD` sin setear |
| `mmorch_cache_stats` | 153 | `window_n=500` | `{window_events, by_model:{in_tokens, cached_tokens, calls, cache_hit_rate}}` | hit-rate = cached/in |
| `mmorch_learn` | 301 | — | `{analysis, recommendations}` | lee metrics.jsonl, cero gasto |
| `mmorch_record_outcome` | 561 | `arm`, `reward: float`, `pattern=""`, `predicted_conf: float\|None`, `source="opus"`, `context=""` | `{recorded, arm, reward, bandit:{mean, n}}` | [DET] append `logs/feedback.jsonl` + update Thompson (`logs/bandit_state.json`) + si `context` presente entrena el sig-bandit de intuition (feedback.py:56-61) |
| `mmorch_feedback_stats` | 593 | — | `{bandit:{arm:{mean,n}}, calibration:{ece, n, by_arm}}` | [RO] |
| `mmorch_intuition` | 212 | `task`, `models: list[str]`, `complexity=""` | `{decision(commit\|escalate), model, reason, coherence, candidates:[[model,mean,n]], reframe_neighbors}` | [RO], cero generación |
| `mmorch_orchestra` | 664 | — | roster de nodos (conductor + secciones, `{handle, kind, builder, status}`) | [RO] |

### 1.5 Verificación determinista / loops

| Tool | Línea | Params | Retorno | Notas |
|---|---|---|---|---|
| `mmorch_check` | 606 | `checker: str`, `ctx: dict` (kwargs del checker) | `{passed, detail, checker, expected, got}` | [RO/DET, cero API]; `checker` inexistente ⇒ **KeyError propagado** |
| `mmorch_rubric_start` | 792 | `task`, `criteria: list[dict]` (`{id, desc, kind: checkable\|subjective, [checker, ctx]}`), `K=5` | STATE completo del loop (dict serializado) | [DET]; los prompts se ejecutan con subagentes del PLAN (cupo, cero API); checkers corren server-side |
| `mmorch_rubric_next` | 805 | `state: dict` | `{role: executor\|judge, prompt}` o `{role: done\|escalate, summary}` | [DET]; el state es el contrato: keys `phase, task, criteria, results, history, attempt, iteration, K` (rubric_loop.py:85) |
| `mmorch_rubric_submit` | 816 | `state: dict`, `output: str` | nuevo state | [DET]+side effects al cerrar: `record_outcome` + destilado a memoria; judge ilegible = refute-by-default; submit en phase terminal ⇒ **ValueError** (rubric_loop.py:183) |
| `mmorch_speedup` | 842 | `source`, `setup`, `call`, `runs=5`, `rounds=8` | `{best, baseline_sec, best_sec, speedup, rounds, stopped, kept}` | **[$]** genera candidatos; score = ejecución real en subprocess, nunca LLM-judge |
| `mmorch_autoresearch` | 248 | `task`, `target_file`, `scorer_cmd`, `cwd="."`, `models`, `maximize=False`, `max_rounds=20`, `patience=5`, `metric_regex=r"score[:=]\s*([-\d.]+)"`, `journal_path`, `resume=False` | `{best_score, baseline, rounds, stopped, improved}` | **[$]**; MUTA `target_file` (keep/discard por best); journal append-only; nunca pushea |

### 1.6 Auto-evolución / aprendizaje de sesiones

| Tool | Línea | Params | Retorno | Notas |
|---|---|---|---|---|
| `mmorch_evolve_self` | 619 | `target_file`, `finding` | `{zone, would_apply, checks, ensemble_degraded, change_id, note}` o `{zone:"red", refused_red:true, ...}` | **[$]** DRY: propone + evalúa fitness SIN tests, NUNCA aplica; zona roja bloqueada siempre |
| `mmorch_evolve_nightly` | 649 | `days=3`, `max_files=5`, `max_findings=8` | `{findings, opened, skipped_active_pr, red, blocked_zone_red}` | **[$]** + side effects git REALES: worktrees sandbox + abre PRs (nunca mergea); coordinado por archivo (1 PR activo por archivo) |
| `mmorch_ingest_session` | 530 | `path="latest"` | `{session, segments, recorded, skipped_no_signal, already_ingested, recorder_failed}` | manda SOLO el prompt del request al router barato; el transcript nunca sale |
| `mmorch_session_playbooks` | 546 | `path="latest"`, `domain=""` | `{ingested, playbooks:[{domain, tool_sequence, n_observed, n_success, success_rate}]}` | 100% local, cero API |

---

## 2. API de librería Python (`from mmorch import ...`)

Superficie pública = `mmorch/__init__.py:83-135` (`__all__`, ~115 símbolos). Organizada
por área; todo lo listado es contrato importable por un caller externo (Lotus, skills,
nightly, workflows):

- **Core**: `call` (providers), `REGISTRY`, `family_of`, `ModelSpec` (config.py:12-24:
  `key, family, provider, model_id, base_url, api_key_env, price_in, price_out, role,
  extra_body`). Defaults: `DEFAULT_GENERATOR="deepseek-chat"`,
  `DEFAULT_VERIFIER="gemini-3.1-flash-lite"`, `DEFAULT_ROUTER="gemini-2.5-flash-lite"`,
  `DEFAULT_INTUITION_POOL` (config.py:160-166). `spec(key)` da KeyError con mensaje
  útil (config.py:173-179); **`family_of` NO** (KeyError pelado, config.py:169-170).
- **Patrones**: `fan_out`, `adversarial_verify`, `route/RouteResult`,
  `cascade/CascadeResult`, `ensemble_verify/EnsembleVerdict`,
  `tournament/TournamentResult`, `bucket_rank/BucketRankResult`,
  `loop_until_done/LoopResult`, `hillclimb/ClimbResult/ClimbCtx/ClimbStep`,
  `ideate/screen/ideate_and_screen`, `Memo/memoized_verify/key_of`.
- **Schema gates**: `gated_json`, `validate`, `extract_json`, `SchemaGateError`.
- **Clasificación/spec**: `classify`, `classify_and_act`, `ClassifyResult`,
  `cynefin_classify/CynefinResult/CYNEFIN_CLASSES/CYNEFIN_STRATEGY`, `build_spec`,
  `spec_interview`, `SpecResult`.
- **Feedback**: `record_outcome`, `ThompsonBandit` (select/update/stats, decay 0.995 =
  Thompson descontado, feedback.py:113-127), `calibration`, `read_outcomes`,
  `calibrate_conf`, `reliability_bins`, `contextual_arm` (key compuesta
  `model@thr#ctx`).
- **Memoria**: `write_episode`, `write_note`, `recall`, `recall_keyword`,
  `recall_hybrid` (RRF), `tombstone_note`, `embed`, `Note` (dataclass
  `id, ts, scope, text, score, layer`), `consolidate`; no exportadas pero públicas de
  facto vía MCP: `reinforce`, `flag_contradiction`, `pending_review`, `resolve_review`,
  `close_loop`, `open_loops`, `forget_preview`, `stats`.
- **Checkers**: `check(name, **ctx) -> CheckResult{passed, detail, checker, expected,
  got}`, `register_checker`, `checkers_available`, `safe_arith`. 21 checkers
  registrados (checkers.py:570-592): arithmetic, code_quality, mutation_score,
  coverage, deterministic, determinant, json_schema, predicate, checksum,
  python_ast_valid, regex_format, set_equal, numeric_close, sorted_monotonic,
  number_theory, sql_valid, units, sympy_identity, python_exec, unit_test, no_tell.
- **Goal**: `load_goal`, `goal_hash`, `goal_aligned`, `authorize_goal`, `goal_guard`,
  `pursue_goal`, `GoalTampered` (goal.py; ver §3.4).
- **Budget**: `BudgetExceeded`, `monthly_spend`, `remaining`, `budget_check`
  (`check(est_cost, critical, override)` — raise si excede), `budget_status`.
- **Evolución**: `Change`, `snapshot_change`, `apply_change`, `rollback`, `evaluate`,
  `zone_of`, `self_evolve`, `red_content_hits`, `sandbox_branch`, `promote_branch`,
  `open_pr_branch`.
- **Loops ejecutores**: `run_code_task/CodeTaskResult`, `run_project_task/ProjectResult`,
  `run_claude` (ejecutor en PLAN/cupo), `start_rubric/rubric_next/rubric_submit/
  run_rubric_loop`.
- **Infra**: `effective_prices/load_overrides`, `fetch_prices/diff_prices/
  propose_price_update`, `orchestra/members/orchestra_conductor/Node`,
  `Predictor/train_predictor/cross_val_error`, `ShadowPrior/offline_improvement/
  shadow_auto_scale`, `featurize_code/train_logreg/train_code_quality/
  emit_training_job/predict_proba/accuracy`, `embed_code/code_embedder_available`,
  `record_trajectory/trajectory_dataset/distill_skill/load_trajectories/
  trajectory_stats`, `nudge_tick/nudge_status`, `policy_violations/docker_available`,
  `cacheable_messages/prefix_signature/shares_prefix`, `is_off_peak/offpeak_advisory/
  spend_by_period`, `model_for_effort/effort_steps/escalation_models`,
  `run_scout/gather_environment/scout_delta`, `emit_event/event_bus/Event`,
  `enrich_prompt/enrich_delta`, `register_project/list_projects/resolve_project`,
  `commit_push/git_pull/git_pull_all`, `register_host/list_hosts/fleet_state/
  fleet_forward`, `weight_card/weight_verify/list_weights`.

---

## 3. Contratos de archivo

### 3.1 `logs/metrics.jsonl` (metrics.py:30-62)

Append-only, sin rotación, un JSON por línea, lock por-proceso (`_LOCK`). Schema por
registro:

```json
{"ts": float_epoch, "iso": "YYYY-MM-DDTHH:MM:SS" (localtime),
 "phase": str, "pattern": str, "node": str, "model": str, "family": str,
 "in_tokens": int, "out_tokens": int, "cost_usd": float(8dec), "latency_s": float(4dec),
 "extra": {…}}          // opcional; contiene error_class, cached_tokens, error, etc.
```

- `extra.error_class ∈ {rate_limit, budget_cap, timeout}` lo pone
  `providers._classify_error` + el gate de budget; `extra.cached_tokens` alimenta
  `cache_stats`.
- **Contrato implícito**: `summary()` (metrics.py:150-164) accede `e["cost_usd"]` /
  `e["family"]` / `e["model"]` SIN `.get()` — una línea sin esas keys revienta
  `mmorch_metrics_summary` entero con KeyError. `error_rates`/`cache_stats` sí usan
  `.get()`. El schema es "todas las keys siempre presentes" pero solo lo garantiza
  `log_event` — nadie lo valida al leer.
- **Contrato de costo = PISO**: calls timeouteadas loggean `cost=0` pero el provider
  factura (budget.py:9-10). `monthly_spend` (budget.py:45-63) filtra por prefijo del
  campo `iso` (string local-time `YYYY-MM`) — el mes se define en huso local.
- Lecturas cacheadas por `(mtime_ns, size)` (`read_jsonl_cached`); `error_rates` con
  solo `window_n` lee el tail directo (metrics.py:87-88).

### 3.2 `logs/feedback.jsonl` + `logs/bandit_state.json` (feedback.py)

- `feedback.jsonl`: append-only, dataclass `Outcome` (feedback.py:31-39):
  `{ts, arm, reward[0..1 clamped], pattern, predicted_conf: float|null, source, context}`.
- `bandit_state.json`: `{arm: [alpha, beta]}` — posterior Beta por brazo; write
  ATÓMICO (`atomic_write_json`, feedback.py:125-127) porque MCP server + nightly
  escriben el mismo archivo sin lock inter-proceso. Load tolerante a corrupción
  (`load_json_tolerant`, no resetea en silencio, feedback.py:93-98).
- Brazos son strings libres; convención contextual: `model@thr#ctx`
  (`contextual_arm`, feedback.py:65-76).

### 3.3 `logs/memory.duckdb` (memory.py:105-153)

DDL (con migración manual por columna — NO usa `ADD COLUMN IF NOT EXISTS` porque en
DuckDB re-aplica el DEFAULT y pisa valores, memory.py:129-131):

```sql
CREATE TABLE episodic (           -- INMUTABLE, append-only, nunca se edita
    id BIGINT, ts DOUBLE, scope VARCHAR, kind VARCHAR,
    actor VARCHAR, payload VARCHAR);          -- payload = str o JSON serializado
CREATE SEQUENCE seq_episodic START 1;

CREATE TABLE semantic (           -- notas destiladas, editable via tombstone
    id BIGINT, ts DOUBLE, scope VARCHAR, text VARCHAR,
    embedding DOUBLE[],           -- 384d bge-small-en-v1.5, NULL si fastembed ausente
    emb_model VARCHAR, dim INTEGER,          -- FIX C: embeddings versionados
    source_ids VARCHAR,           -- JSON list de episodic ids
    tombstone BOOLEAN DEFAULT FALSE,
    verified BOOLEAN DEFAULT FALSE,
    access_count INTEGER DEFAULT 0, last_accessed_at DOUBLE,   -- decay Ebbinghaus
    open_loop BOOLEAN DEFAULT FALSE,          -- Zeigarnik
    lifespan VARCHAR DEFAULT 'decay',         -- 'decay' | 'permanent'
    needs_review BOOLEAN DEFAULT FALSE);      -- reconsolidación
CREATE SEQUENCE seq_semantic START 1;
```

- **Scopes**: jerarquía LITERAL `SCOPE_ORDER = ["task_id","subsector","project_id",
  "mmorch_self","global"]` (memory.py:38). Un scope fuera de esa lista encadena
  `[scope, "global"]` (memory.py:331-337). Contrato implícito no documentado en la
  tool MCP: los nombres de nivel son literales, no placeholders — `scope="task_id"`
  es un scope válido en sí, no "poné acá tu task id".
- Recall filtra `NOT tombstone AND NOT needs_review`; fallback a `episodic` raw si
  la capa semántica devuelve < k (memory.py:391-400).

### 3.4 `GOAL.md` / `GOAL.hash` (goal.py)

- `GOAL.hash` = **16 hex chars** = `sha256(GOAL.md)[:16]` (goal.py:33-35). Contenido
  actual: `2d2d924b3df25697`.
- Contrato: `goal_guard()` compara hash actual vs autorizado; distinto ⇒
  `GoalTampered` ⇒ HALT de toda auto-aplicación (goal.py:46-60). Re-autorizar =
  `authorize_goal()` (acto humano). Ambos archivos + `goal.py`/`budget.py` están en
  `_RED_PATHS` de evolve (evolve.py:270) — zona roja para la auto-evolución.
- **Hueco del contrato**: si `GOAL.hash` NO existe, `goal_guard` auto-autoriza el
  GOAL presente (goal.py:53-54). Borrar `GOAL.hash` + editar `GOAL.md` re-autoriza
  sin gate humano. La defensa es que `_RED_PATHS` bloquea al AGENTE de evolve, pero
  cualquier otro proceso con write al repo puede hacerlo; el archivo hash no está
  firmado ni fuera del árbol.

### 3.5 `logs/mcp_calls.jsonl` (mcp_telemetry.py:24-33)

`{ts, tool, ok: bool, dur_s, err?: str[:200]}` por invocación MCP. Append-only,
best-effort (fallo de logging silencioso).

### 3.6 Vault global (`vault.write_validated`, vault.py:117-147)

- Validación de borde: `title` y `project` no vacíos (ValueError). Frontmatter:
  `created` (ISO date) autocompletado, `tags` siempre incluye `project`.
- La tool MCP agrega `status/confidence/sources/tags`; `sources` y `tags` viajan como
  **CSV en string** (stringly-typed, mcp_server.py:363-364, 382-384).
- Side effects: nota .md + MOC regenerado + gist a memoria scope `global`
  (`kind="vault_note"`) + babel ingest en thread daemon (perder el thread no pierde
  nada: el nightly barre notas sin babel, mcp_server.py:392-395).

---

## 4. Validación en bordes — huecos concretos

1. **`mmorch_cascade` (mcp_server.py:239)**: `steps` se parsea `[(s[0], float(s[1]))]`
   sin validar shape — `[["deepseek-chat"]]` ⇒ IndexError; `[["m", "abc"]]` ⇒
   ValueError; ambos escapan como excepción cruda, no como `{"error"}`.
2. **`mmorch_check` (mcp_server.py:606-615)**: `checker` inexistente ⇒ KeyError
   propagado (checkers.py:606-607) en vez de un retorno JSON de error; `ctx` con
   kwargs inesperados ⇒ TypeError del checker.
3. **`mmorch_recall` / `mmorch_remember`**: `k` sin techo (k enorme ⇒ fallback
   episódico masivo), `window_days` negativo no rechazado (cutoff futuro ⇒ 0
   resultados en silencio), `scope` libre sin advertir la semántica literal de
   `SCOPE_ORDER` (§3.3).
4. **Tools de nota por id** (`reinforce:703`, `flag_contradiction:713`,
   `resolve_review:735`, `close_loop:745`): un `note_id` inexistente hace UPDATE de 0
   filas y devuelve `ok: true` igual — éxito silencioso sobre nada (memory.py:248-268,
   288-299). El contrato `ok` no significa "la nota existía".
5. **`mmorch_route`/`mmorch_cynefin`**: `threshold` no clampado a [0,1] (un 7.0
   escala siempre; un -1 nunca). `models`/`gen_model` inválidos ⇒ KeyError de
   `REGISTRY` (config.py:169-170) sin mensaje amable (`spec()` sí lo tiene, pero
   `family_of()` no lo usa).
6. **`mmorch_record_outcome`**: `reward` sí se clampa en librería (feedback.py:46),
   pero `predicted_conf` no se valida (se clampa recién al LEER en
   `reliability_bins`/`calibration`, feedback.py:149,179 — el log puede contener
   valores fuera de [0,1]).
7. **`mmorch_rubric_start` (mcp_server.py:792-801)**: `criteria` no se valida en el
   borde — un dict sin `id`/`kind` revienta después, dentro del loop
   (rubric_loop.py:94-106) con KeyError lejos del caller.
8. **`mmorch_review_code`**: el gate de secretos (`_SECRET_NAME_RX`,
   mcp_server.py:184-185) cubre solo el **nombre del path**; `code=` inline con
   secretos pegados sale a la API externa sin scan de contenido. Además el patrón no
   cubre `id_rsa`, `*.pem`, `*.pfx`.
9. **`mmorch_evolve_self` (mcp_server.py:630-634)**: strip de code-fence heurístico y
   frágil (`a.split("```", 2)[1] if "```" in a[3:]`) — un output con fence interno o
   sin fence de cierre puede quedar mal recortado y evaluarse un artefacto truncado.

## 5. Inconsistencias entre tools / contratos implícitos

1. **Contrato de error dividido**: `{"error": ...}` JSON (review_code:198-207,
   autoresearch vía librería) vs excepción propagada (check, cascade, rubric_submit
   ValueError:rubric_loop.py:183, route/tournament con modelo inválido). Un caller
   MCP no puede tratar errores uniformemente.
2. **Conteo de tools desactualizado**: los comentarios dicen "las 44 tools"
   (mcp_server.py:56-57) y "44 tools" (mcp_telemetry.py:1,6) — hoy son **46**.
3. **Docstring de `mmorch_check` desactualizado** (mcp_server.py:609): declara
   `checker in {arithmetic, json_schema}` pero el registry tiene **21 checkers**
   (checkers.py:570-592). El caller MCP no descubre `python_exec`, `unit_test`,
   `sql_valid`, etc. salvo leyendo el código.
4. **`source` default divergente**: la tool MCP `mmorch_record_outcome` usa
   `source="opus"` (mcp_server.py:566) mientras la librería usa `source=""`
   (feedback.py:43) — outcomes vía MCP quedan atribuidos a "opus" aunque el label
   venga de otro lado si el caller no lo pisa.
5. **`logs/mcp_calls.jsonl` contaminado con el selftest**: las primeras líneas del log
   real son `mmorch_ok_tool`/`mmorch_boom_tool` (los fixtures del `__main__` de
   mcp_telemetry.py:102-108) — el redirect a tempfile (líneas 110-112) falló al menos
   una vez y `stats()` cuenta esos registros ficticios.
6. **Parámetros scope `""` como sentinel**: las tools de memoria usan `scope=""` ⇒
   `None` (todas), pero `mmorch_recall` usa `scope="global"` como default — dos
   convenciones para "sin filtro/default" en la misma familia de tools.
7. **`consolidate` invierte la polaridad del flag**: la librería expone `dry_run`
   (default False) y la tool expone `apply` (default False ⇒ dry_run=True)
   (mcp_server.py:689 vs memory.py:625-626). Seguro por diseño, pero un caller que
   salte de la tool a la librería con los mismos kwargs aplica cambios sin querer.
8. **Costo de `metrics.jsonl` es piso, no verdad**: documentado en budget.py:9-10
   pero no en `mmorch_metrics_summary` ni `mmorch_budget_status` — el caller de esas
   tools no sabe que `spent` subestima.
9. **`recall` no es read-only**: la tool no lo dice, pero cada recall muta
   `access_count`/`last_accessed_at` (memory.py:402-403, spacing effect) — afecta qué
   se olvida después. Contrato implícito con consecuencias de retención.
10. **`ModelSpec.price_*` VOLÁTILES por contrato** (config.py:3-4, 116, 145): precios
    de referencia jun-2026 hardcodeados; `effective_prices/load_overrides` existe como
    override pero el REGISTRY sigue siendo el default silencioso.
11. **Claves internas estables como contrato del bandit** (config.py:30-31):
    `deepseek-chat` mapea a `deepseek-v4-flash` — la key interna NO puede renombrarse
    sin romper los brazos históricos de `bandit_state.json`. Contrato implícito
    documentado solo en un comentario.
