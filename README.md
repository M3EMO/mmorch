# mmorch — Multi-Model Orchestration Harness

**mmorch** is a deterministic Python orchestration library (plus an MCP server) that treats
the scarce resource as *Claude plan quota* ("cupo"), not dollars. Bulk generation and
verification are delegated to cheap external model APIs (DeepSeek, Gemini); the high-judgment
orchestrator (Opus/Fable) only conducts and breaks ties — it is never an external node. The
orchestration is plain, testable Python; the models are interchangeable nodes.

**Core ideas**
- **Conductor + orchestra.** A deterministic Python core routes work to model nodes; Opus/Fable
  conducts, never plays.
- **Cross-family verification (OneFlow).** Every generator→verifier pair spans different model
  families to decorrelate errors; checkable claims go to deterministic checkers instead of an
  LLM (measured: LLM judges ≈74% false-refute on hard checkable tasks).
- **Anti-sycophancy.** Verifiers refute by default; the reward label is a real execution
  outcome, never self-reported confidence.
- **Self-evolution, safely gated.** Changes pass a fitness battery (AST · tests · ensemble ·
  rollback · cost · goal-alignment) plus a `GOAL.md` tamper-halt, scored by reversibility ×
  blast-radius zones — the red zone is never autonomous.
- **A feedback flywheel.** A Thompson bandit + calibration learn which model/threshold wins;
  loop trajectories become execution-labeled training data for a local code encoder (beats
  bge-small on code structure).

Lives at `~/.claude/orchestration/`, usable from any project; registered globally as the MCP
server `mmorch`.

## What's here

<!-- mmorch:auto:stats -->
_Auto-generado por `mmorch.docgen`._ **44 módulos · 26 MCP tools · 267 tests.**
<!-- /mmorch:auto:stats -->

<!-- mmorch:auto:modules -->
| Módulo | Qué hace |
|---|---|
| `mmorch/ablation.py` | ablation (§18.4) — validar EMPIRICAMENTE la regla de pairing cross-family. El |
| `mmorch/bucketrank.py` | bucket_rank — graduar/ordenar un set GRANDE en tiers (triage por calidad, rankear |
| `mmorch/budget.py` | budget — BudgetKeeper: techo de gasto mensual (ataca el incidente +$5). |
| `mmorch/cache.py` | memo (I-4) — cache content-hash de resultados/verdicts. Salta re-gen/re-verify |
| `mmorch/cascade.py` | cascade — FrugalGPT-style multi-step confidence cascade (research: vault/research/ |
| `mmorch/checkers.py` | checkers — libreria propia de VERIFICADORES DETERMINISTAS (tool-verify). |
| `mmorch/classify.py` | classify_and_act — rutear por TIPO y manejar cada rama distinto (triage, model |
| `mmorch/code_embedder.py` | code_embedder — inferencia NUMPY PURA del encoder SimCLR del flywheel (sin torch). |
| `mmorch/code_loop.py` | code_loop — el WIRE de Fase 5 a produccion: tareas de CODIGO con lazo cerrado. |
| `mmorch/config.py` | Model registry — single source of truth for models, families, endpoints, prices. |
| `mmorch/cost.py` | Cost model — USD from token counts, using REGISTRY prices. |
| `mmorch/dataset.py` | dataset — construye un dataset de CALIDAD DE CÓDIGO desde git history, SIN labels |
| `mmorch/effort.py` | effort — knob explicito de esfuerzo -> tier de modelo (patron Fable 5: 'effort' controla |
| `mmorch/ensemble.py` | ensemble_verify (I-3) — K escepticos cross-family + voto mayoria. |
| `mmorch/events.py` | events — bus de progreso in-process pa la UI live (nivel 3). El orquestador emite |
| `mmorch/evolve.py` | evolve — subset DGM-inspirado, GATED (research: vault/research/ |
| `mmorch/factory.py` | factory — mmorch como FÁBRICA de modelos (no ES el modelo, lo CONSTRUYE/entrena). |
| `mmorch/feedback.py` | feedback — el lazo que faltaba (la 'loss' ausente). mmorch genera/verifica/ |
| `mmorch/goal.py` | goal — ancla anti-goal-drift, modelada sobre el `/goal` nativo de Claude Code. |
| `mmorch/hillclimb.py` | hillclimb — optimizacion sobre METRICA ESCALAR con feedback del entorno |
| `mmorch/innovate.py` | innovate (I-5) — motor de innovacion productizado. mmorch se idea capacidades |
| `mmorch/learn.py` | learn — meta-inteligencia: mmorch aprende de su propio metrics.jsonl (I-1). |
| `mmorch/loop.py` | loop_until_done — scope DESCONOCIDO, 'segui hasta que este limpio'. Control-flow |
| `mmorch/megasource.py` | megasource (Fase 2) — megafuente autodidacta: primer hit = provider PRICING. |
| `mmorch/memory.py` | memory — memoria episodica + semantica para mmorch (DuckDB 2 capas). |
| `mmorch/metrics.py` | Observability — append-only JSONL metric log (§11 backbone). |
| `mmorch/nodes.py` | nodes — el registry de la ORQUESTA: nombra a cada miembro que mmorch (el DIRECTOR) |
| `mmorch/nudge.py` | nudge — robo de Hermes 'periodic memory nudging': cada N loops cerrados, dispara |
| `mmorch/patterns.py` | Code-flow patterns (§7), migrated as deterministic Python. |
| `mmorch/predict.py` | predict (v0.1 NN, Fase 1) — predictor de out_tokens / latencia, SIN dep pesada. |
| `mmorch/prices.py` | prices — capa de OVERRIDE de precios (datos volátiles, separados del código). |
| `mmorch/prompts.py` | prompts — construccion de mensajes PREFIX-STABLE pa maximizar el cache-hit de DeepSeek. |
| `mmorch/providers.py` | Provider layer — thin OpenAI-compatible client per external model. |
| `mmorch/route.py` | route (I-2) — confidence-gated escalation. Ahorra cupo: el modelo barato |
| `mmorch/rubric_loop.py` | rubric_loop — LOOP DE AUTOCORRECCION CON VERIFICADOR INDEPENDIENTE (spec del usuario). |
| `mmorch/sandbox.py` | sandbox — corre codigo NO confiable aislado (la compuerta del pipeline 'git-like' |
| `mmorch/schedule.py` | schedule — ADVISORY de ventana off-peak (DeepSeek descuenta fuerte fuera de hora pico). |
| `mmorch/schema.py` | schema (§9) — structured-output gates. Hoy los parsers de mmorch son best-effort |
| `mmorch/scout.py` | scout — pre-pass ENTORNO-PRIMERO (el patron central de Fable 5: 'primero aprende el |
| `mmorch/server.py` | server — mmorch VISUAL nivel 3: progreso live de cada subagente + control TOTAL remoto. |
| `mmorch/shadow_prior.py` | shadow_prior — Fase 5: una capa que PRIMEA al ThompsonBandit con un prior contextual, |
| `mmorch/tournament.py` | tournament — elegir EL mejor de pocos candidatos por gusto/calidad (naming, |
| `mmorch/trajectory.py` | trajectory — robo de Hermes: 'trajectory compression para entrenar la proxima |
| `mmorch/vault.py` | vault — memoria de largo plazo mmorch-legible sobre el vault Obsidian. |
<!-- /mmorch:auto:modules -->

Otros: `mcp_server.py` (MCP wrapper), `tests/` (regression gate), `vault/` (memoria +
research), `smoke_test.py`, `AUDIT_*.md` / `INNOVATION_ROADMAP_*.md`.

> Las secciones entre `<!-- mmorch:auto:* -->` las regenera `python -m mmorch.docgen`
> desde el código (fuente de verdad). No editar a mano.

Active models: `deepseek-chat`/`deepseek-reasoner` (DeepSeek V4 Flash, no-think/think bulk),
`deepseek-v4-pro` (code-heavy executor), `gemini-3.1-flash-lite` (default cross-family
verifier/judge), `gemini-2.5-flash`/`-lite` (legacy verifier/router). Kimi configured, inactive
(no key). Cache-hit billing is instrumented (DeepSeek caches input ~50× cheaper).

**Self-evolution (gated):** mmorch self-audits and self-ideates capabilities using itself
(fan_out → cross-family verify → Opus tie-break). It NEVER self-modifies live without green
tests + `goal_aligned` + `goal_guard` (tamper-halt) + a human gate on red/yellow zones. The
7-pattern catalog is complete (classify-and-act, fan-out, adversarial-verify, generate-and-
filter, tournament, bucket-rank, loop-until-done) plus cascade, ensemble, route, schema-gates,
feedback loop and 2-layer memory.

**Beyond the patterns:** a rubric-driven autocorrection loop (`rubric_loop` — planner/manager/
executor/judge, checkable→checker $0, subjective→cross-family judge; runs over API or in
plan-mode via MCP for zero API spend); a code-execution loop (`code_loop`); a SimCLR code
encoder trained from loop trajectories (`flywheel/`, numpy inference); an environment-first
scout pre-pass; and full cost observability (per-provider 429/budget-cap rates, cache-hit
rate, off-peak split, effort-routing, prefix-stable prompts).

## Setup

1. Keys — copy and fill:
   ```
   cp .env.example .env      # then paste DEEPSEEK_API_KEY and GEMINI_API_KEY
   ```
2. Venv already created at `.venv/` with deps. Recreate if needed:
   ```
   .venv\Scripts\python.exe -m pip install openai python-dotenv "mcp>=1.2.0"
   ```

## Use as a library

```python
from mmorch import fan_out, adversarial_verify

# bulk generation in parallel on the cheap node
res = fan_out(["task A", "task B", "task C"])

# cross-family adversarial check (DeepSeek author -> Gemini skeptic)
v = adversarial_verify(code, rubric="must return a+b")
print(v.passed, v.refutations)
```

`adversarial_verify` is TASK-AWARE: for `task_kind="subjective"` (default) it raises on
same-family (OneFlow); for `task_kind="checkable"` same-family is allowed, and passing a
`checker=` (e.g. `"arithmetic"`) verifies by CODE (checkers.py) — zero API, 100% reliable
where an LLM verifier is ~74% false-refute on hard math.

## Use as MCP tools (inside Claude Code)

Registered globally in `~/.claude.json` as server `mmorch`. Calling these spends
external API $, not cupo — that's the point.

<!-- mmorch:auto:tools -->
MCP tools (server `mmorch`): `mmorch_adversarial_verify`, `mmorch_bucket_rank`, `mmorch_budget_status`, `mmorch_cache_stats`, `mmorch_cascade`, `mmorch_check`, `mmorch_classify`, `mmorch_consolidate`, `mmorch_ensemble_verify`, `mmorch_error_rates`, `mmorch_evolve_self`, `mmorch_fan_out`, `mmorch_feedback_stats`, `mmorch_innovate`, `mmorch_learn`, `mmorch_memory_stats`, `mmorch_metrics_summary`, `mmorch_orchestra`, `mmorch_recall`, `mmorch_record_outcome`, `mmorch_remember`, `mmorch_route`, `mmorch_rubric_next`, `mmorch_rubric_start`, `mmorch_rubric_submit`, `mmorch_tournament`.

**Restart Claude Code** to load new tools.
<!-- /mmorch:auto:tools -->

## Live UI + remote control (level 3)

A Starlette server (zero new deps — Starlette + uvicorn already present) gives a live view of
every subagent and full remote control. It runs jobs **in-process** and streams progress over
SSE from an in-memory event bus (`mmorch/events.py`) — no cross-process JSONL tailing. The
JSONL stays the durable audit log.

```
MMORCH_SERVER_TOKEN=<secret> MMORCH_SERVER_HOST=<tailnet-ip> \
  .venv/Scripts/python.exe -m mmorch.server      # default 127.0.0.1:8787
```

- `GET /` live dashboard · `GET /events` SSE feed · `GET /state` snapshot
- `POST /run/rubric`, `/run/fanout` start jobs · `POST /kill/{id}`, `/approve/{id}` control
- Auth: `X-Token` header (or `?token=` for `EventSource`) vs `MMORCH_SERVER_TOKEN`.
- **Security:** run ONLY behind a private tunnel (Tailscale recommended) bound to the tailnet
  IP — never `0.0.0.0` on the public internet. Remote control is the human gate exercised
  remotely-but-authenticated; mmorch still never auto-applies red-zone on its own, and
  `BudgetKeeper`/`goal_guard` stay active as override-able safety nets.

## Smoke test

```
.venv\Scripts\python.exe smoke_test.py
```
Runs fan_out on DeepSeek + a planted-bug adversarial_verify on Gemini, then prints
the cost summary and metrics log path.

## Metrics

Every node call appends to `logs/metrics.jsonl`. `mmorch.metrics.summary()` aggregates
cost by family/model — the input to the break-even test (§14, §18.4).

## Open / pending (not code gaps — validation & infra)

- **Break-even unproven.** The whole $-savings premise (§14) needs real volume in
  `logs/metrics.jsonl`. Sample still thin; the feedback loop (`record_outcome`) is the
  signal source and is only lightly used so far.
- **§18.4 ablation — POWERED (n=350, 2 runs).** `ablation_symmetric.py` (symmetric
  4-cell, McNemar) found NO significant self-vs-cross blind-spot on checkable math
  (p=0.06–0.25) → the cross-family raise is now scoped to subjective only (`task_kind`).
  Separately, `ablation_prompt.py` showed LLM verification of hard checkable math is
  ~74% false-refute regardless of family/prompt → use deterministic `checkers.py` there.
  Still a 2-family limit (below) caps how far the cross-family thesis can be tested.
- **Kimi/Moonshot node** — configured, inactive (no key). Blocks any 3-family test.
- **break-even / feedback** — feedback loop bootstrapped (calibration n=1→1001 via
  ablation `record_outcome`); break-even on real volume still pending.

Run `python -m pytest tests/` before promoting any new capability.

## Rollback

- MCP: restore `~/.claude.json.bak-mmorch`, remove the `mmorch` key.
- Protocol: delete the `MULTIMODEL_ORCH` block in `~/.claude/CLAUDE.md`.
- Library: delete `~/.claude/orchestration/`.
