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
_Auto-generado por `mmorch.docgen`._ **131 módulos · 46 MCP tools · 787 tests.**
<!-- /mmorch:auto:stats -->

<!-- mmorch:auto:modules -->
| Módulo | Qué hace |
|---|---|
| `mmorch/ablation.py` | ablation (§18.4) — validar EMPIRICAMENTE la regla de pairing cross-family. El |
| `mmorch/adjudicate.py` | Adjudication module for matching notes to projects. |
| `mmorch/arbitration.py` | arbitration — registro auditable de los arbitrajes del orquestador (blind-spot #2, 2026-07). |
| `mmorch/architecture.py` | Organizador de arquitectura — chequeos MECANICOS, sin juicio de LLM. |
| `mmorch/auto_repair.py` | Auto-reparación nocturna: los errores que el sistema DETECTA se convierten |
| `mmorch/automerge.py` | Automerge con semáforo — merges sin accionar humano SOLO en el carril verde. |
| `mmorch/autoresearch.py` | autoresearch (r4a) — hillclimb como JOB declarativo + resumable. |
| `mmorch/babel.py` | babel — capa comprimida model-native del vault (paper 2606.19857). |
| `mmorch/bench.py` | bench — benchmark CONGELADO de tasks difíciles para evolución de workflows. |
| `mmorch/bucketrank.py` | bucket_rank — graduar/ordenar un set GRANDE en tiers (triage por calidad, rankear |
| `mmorch/budget.py` | budget — BudgetKeeper: techo de gasto mensual (ataca el incidente +$5). |
| `mmorch/budget_policy.py` | budget_policy — scoped budget policies (graft G5 from paperclip). |
| `mmorch/bughunt.py` | BUG-HUNTER logico de mmorch: mutation-survivors como mapa de donde un bug silencioso viviria. |
| `mmorch/bursts.py` | Bursts de arXiv — temas recién acuñados, que ningún tag todavía nombra. |
| `mmorch/cache.py` | memo (I-4) — cache content-hash de resultados/verdicts. Salta re-gen/re-verify |
| `mmorch/cascade.py` | cascade — FrugalGPT-style multi-step confidence cascade (research: vault/research/ |
| `mmorch/chat_store.py` | chat_store — durable chat history for Lotus (SQLite, stdlib). |
| `mmorch/checkers.py` | checkers — libreria propia de VERIFICADORES DETERMINISTAS (tool-verify). |
| `mmorch/classify.py` | classify_and_act — rutear por TIPO y manejar cada rama distinto (triage, model |
| `mmorch/claude_exec.py` | claude_exec — ejecutor que corre en el PLAN de Claude (cupo), no por API. Invoca el |
| `mmorch/cli.py` | CLI minimo instalable (`mmorch`): status y health desde la terminal. |
| `mmorch/code_embedder.py` | code_embedder — inferencia NUMPY PURA del encoder SimCLR del flywheel (sin torch). |
| `mmorch/code_loop.py` | code_loop — el WIRE de Fase 5 a produccion: tareas de CODIGO con lazo cerrado. |
| `mmorch/code_review.py` | code_review — cero-cupo senior reviewer: read code, flag where it breaks the mmorch coding |
| `mmorch/config.py` | Model registry — single source of truth for models, families, endpoints, prices. |
| `mmorch/context_blocks.py` | context_blocks — the durable half of an "auto-compact to info-blocks" scheme for Claude Code. |
| `mmorch/cost.py` | Cost model — USD from token counts, using REGISTRY prices. |
| `mmorch/curation.py` | Curacion humana de propuestas — logica compartida entre scripts/veredicto.py, |
| `mmorch/curiosity.py` | curiosity — deteccion de TENSION en la memoria (modulo cognitivo #3). |
| `mmorch/dataset.py` | dataset — construye un dataset de CALIDAD DE CÓDIGO desde git history, SIN labels |
| `mmorch/debate.py` | Debate cross-family sobre una branch: refutador -> habilitador -> juez. |
| `mmorch/decision_mining.py` | Mineria de DECISIONES humanas desde transcripts de Claude Code. |
| `mmorch/docs_extract.py` | Extracción de texto de documentos (PDF hoy) — dos niveles, medidos en |
| `mmorch/durable_runs.py` | durable_runs — heartbeat + zombie reaper for in-process jobs (graft G9 from paperclip). |
| `mmorch/effort.py` | effort — knob explicito de esfuerzo -> tier de modelo (patron Fable 5: 'effort' controla |
| `mmorch/enrich.py` | enrich — completar/especificar el prompt infiriendo intent del usuario (patron Fable 5), |
| `mmorch/ensemble.py` | ensemble_verify (I-3) — K escepticos cross-family + voto mayoria. |
| `mmorch/events.py` | events — bus de progreso in-process pa la UI live (nivel 3). El orquestador emite |
| `mmorch/evolve.py` | evolve — subset DGM-inspirado, GATED (research: vault/research/ |
| `mmorch/evolve_findings.py` | evolve_findings — fuente automática de hallazgos para el loop nocturno de auto-evolve |
| `mmorch/exec_embedder.py` | exec_embedder — embedding por EJECUCION (huella de comportamiento), CERO entrenamiento. |
| `mmorch/exec_policy.py` | exec_policy — where execution is allowed to run (graft G3 from paperclip). |
| `mmorch/factory.py` | factory — mmorch como FÁBRICA de modelos (no ES el modelo, lo CONSTRUYE/entrena). |
| `mmorch/feedback.py` | feedback — el lazo que faltaba (la 'loss' ausente). mmorch genera/verifica/ |
| `mmorch/feedback_trace.py` | feedback_trace — human vote -> trace bundle + bandit signal (graft G8 from paperclip). |
| `mmorch/few_shot_bootstrap.py` | few_shot_bootstrap — DSPy-A leído del código (bootstrap.py de stanfordnlp/dspy), robado como |
| `mmorch/fleet.py` | fleet — control unificado de varios hosts mmorch en el tailnet. Cada maquina corre su |
| `mmorch/frontier.py` | Frontera de temas — rompe el círculo cerrado del auto-descubrimiento. |
| `mmorch/fuel.py` | Fuel module: candidate proposal lifecycle for roadmap loops. |
| `mmorch/gate_policy.py` | gate_policy — staged review/approval gates per job (graft G6 from paperclip). |
| `mmorch/goal.py` | goal — ancla anti-goal-drift, modelada sobre el `/goal` nativo de Claude Code. |
| `mmorch/hardening.py` | Hardening loop: mmorch se blinda solo contra sus puntos ciegos. |
| `mmorch/health.py` | Health module for mmorch: dead-man's switch detection. |
| `mmorch/hillclimb.py` | hillclimb — optimizacion sobre METRICA ESCALAR con feedback del entorno |
| `mmorch/innovate.py` | innovate (I-5) — motor de innovacion productizado. mmorch se idea capacidades |
| `mmorch/intuition.py` | intuition — the bandit, re-keyed by structural signature (intuition layer Phase 1). |
| `mmorch/iohelpers.py` | iohelpers — shared robustness idioms for the JSON/JSONL state files under logs/*. |
| `mmorch/job_graph.py` | job_graph — adjacency-list ancestry over the in-memory job map (graft G1). |
| `mmorch/lang.py` | lang — capacidades deterministas POR LENGUAJE para el project-build engine. |
| `mmorch/learn.py` | learn — meta-inteligencia: mmorch aprende de su propio metrics.jsonl (I-1). |
| `mmorch/loop.py` | loop_until_done — scope DESCONOCIDO, 'segui hasta que este limpio'. Control-flow |
| `mmorch/loop_nightly.py` | F5 loop-cerrado: orquestador nightly del loop de ideas (spec .scratch/loop-cerrado/spec.md). |
| `mmorch/mcp_server.py` | MCP wrapper — exposes mmorch patterns as tools to Claude Code. |
| `mmorch/mcp_telemetry.py` | mcp_telemetry — logger CENTRALIZADO de invocaciones MCP (audit 2026-07: 44 tools, ~20 |
| `mmorch/megasource.py` | megasource (Fase 2) — megafuente autodidacta: primer hit = provider PRICING. |
| `mmorch/memory.py` | memory — memoria episodica + semantica para mmorch (DuckDB 2 capas). |
| `mmorch/merge_train.py` | Merge train — las branches amarillas del dia se conglomeran en UN merge. |
| `mmorch/metrics.py` | Observability — append-only JSONL metric log (§11 backbone). |
| `mmorch/minds.py` | minds — global federation graph across registered projects (read-only). |
| `mmorch/nightly.py` | nightly — driver ALWAYS-ON del loop nocturno (Windows Task Scheduler, no Claude). |
| `mmorch/nodes.py` | nodes — el registry de la ORQUESTA: nombra a cada miembro que mmorch (el DIRECTOR) |
| `mmorch/nudge.py` | nudge — robo de Hermes 'periodic memory nudging': cada N loops cerrados, dispara |
| `mmorch/outcomes.py` | Outcome recording and expiry for proposals. |
| `mmorch/paths.py` | Rutas de ESTADO del sistema (logs, DBs, bandits, memoria, cache). |
| `mmorch/patterns.py` | Code-flow patterns (§7), migrated as deterministic Python. |
| `mmorch/plugin_worker.py` | plugin_worker — isolated subprocess host for ONE plugin invoke (graft G11). |
| `mmorch/plugins.py` | plugins — capability-gated plugin platform (graft G11 from paperclip plugin-loader.ts). |
| `mmorch/portability.py` | portability — export/import mmorch state across devices (grafts G2 + G4). |
| `mmorch/predict.py` | predict (v0.1 NN, Fase 1) — predictor de out_tokens / latencia, SIN dep pesada. |
| `mmorch/prices.py` | prices — capa de OVERRIDE de precios (datos volátiles, separados del código). |
| `mmorch/project_build.py` | project_build — F1 of the /project rebuild: decompose a big task into a VALIDATED worklist |
| `mmorch/project_driver.py` | project_driver — F2 of the /project rebuild: the RECURSIVE build orchestrator. |
| `mmorch/project_integrate.py` | project_integrate — F3 of the /project rebuild: wire the recursive driver (F2) to REAL seams. |
| `mmorch/project_loop.py` | project_loop — ejecutor PROJECT-AWARE primario via mmorch (barato, cero cupo). Es la |
| `mmorch/project_repair.py` | Reparación cross-repo: mmorch arregla los proyectos del REGISTRY, no solo |
| `mmorch/projects.py` | projects — registro de proyectos que mmorch puede CONTROLAR (project-aware). Hace que |
| `mmorch/prompts.py` | prompts — construccion de mensajes PREFIX-STABLE pa maximizar el cache-hit de DeepSeek. |
| `mmorch/proposals.py` | F2 propuesta (spec .scratch/loop-cerrado/spec.md): tarjetas pre-cocinadas + pick del hook. |
| `mmorch/provenance.py` | Provenance de branches — outcomes retroactivos por verdad de ejecución. |
| `mmorch/providers.py` | Provider layer — thin OpenAI-compatible client per external model. |
| `mmorch/pty_session.py` | pty_session — interactive PTY sessions for the Lotus terminal. |
| `mmorch/refutar.py` | Refutacion cross-family de una branch propuesta — el escalon entre el triage |
| `mmorch/regresion.py` | Refutacion EJECUTABLE de una branch: la objecion se prueba o no existe. |
| `mmorch/repo_mining.py` | Minería de repos ajenos — aprender de cualquier repo SIN acumularlo. |
| `mmorch/retention.py` | retention — decay Ebbinghaus + Zeigarnik para la capa semantica de memory. |
| `mmorch/route.py` | route (I-2) — confidence-gated escalation. Ahorra cupo: el modelo barato |
| `mmorch/rubric_loop.py` | rubric_loop — LOOP DE AUTOCORRECCION CON VERIFICADOR INDEPENDIENTE (spec del usuario). |
| `mmorch/sandbox.py` | sandbox — corre codigo NO confiable aislado (la compuerta del pipeline 'git-like' |
| `mmorch/schedule.py` | schedule — ADVISORY de ventana off-peak (DeepSeek descuenta fuerte fuera de hora pico). |
| `mmorch/schema.py` | schema (§9) — structured-output gates. Hoy los parsers de mmorch son best-effort |
| `mmorch/scout.py` | scout — pre-pass ENTORNO-PRIMERO (el patron central de Fable 5: 'primero aprende el |
| `mmorch/self_audit.py` | Auto-auditoria — el juez de mmorch se mira a si mismo, modulo por modulo. |
| `mmorch/server.py` | server — mmorch VISUAL nivel 3: progreso live de cada subagente + control TOTAL remoto. |
| `mmorch/server_core.py` | server_core — shared in-process state + tiny request helpers for the server route modules. |
| `mmorch/server_engine.py` | server_engine — the in-process job execution engine: the threads that drive rubric, |
| `mmorch/server_fleet.py` | server_fleet — multi-host (tailnet) routes: register/list fleet hosts, proxy a job to a |
| `mmorch/server_frontend.py` | server_frontend — the live dashboard HTML, lifted verbatim out of server.py (it is a static string, not logic; keeping it here shrinks the god-module). |
| `mmorch/server_pty.py` | server_pty — interactive PTY (terminal) routes: open/stream/input/resize/close a shell |
| `mmorch/session_skills.py` | session_skills — mina playbooks reusables de sesiones de Claude. De segmentos con |
| `mmorch/sessions.py` | sessions — aprende de transcripts de Claude Code. Parsea el JSONL de sesion en |
| `mmorch/shadow_prior.py` | shadow_prior — Fase 5: una capa que PRIMEA al ThompsonBandit con un prior contextual, |
| `mmorch/signature.py` | signature — project a task's TEXT onto a STRUCTURAL key (cero-cupo, deterministic). |
| `mmorch/slim.py` | Slim — auto-eficientización de código: menos verbose, misma conducta. |
| `mmorch/spec.py` | spec — spec-builder barato que INFIERE mas alla de lo dicho, pero aplica |
| `mmorch/speedup.py` | speedup — make a function faster, cero-cupo, kept only on MEASURED+CORRECT improvement. |
| `mmorch/stuck_detector.py` | Detector de estancamiento — tendencias sobre la historia nocturna, cero LLM. |
| `mmorch/sync.py` | sync — GitHub como bus de sincronizacion entre maquinas. El host always-on (ej pc-mateo) |
| `mmorch/textutil.py` | textutil — shared text helpers. Dedups the code-fence extractor that was copy-pasted |
| `mmorch/tournament.py` | tournament — elegir EL mejor de pocos candidatos por gusto/calidad (naming, |
| `mmorch/trajectory.py` | trajectory — robo de Hermes: 'trajectory compression para entrenar la proxima |
| `mmorch/transcript_store.py` | transcript_store — per-job inter-agent transcript (in-memory). |
| `mmorch/triage.py` | Triage mecanico de branches propuestas — cero LLM, cero cupo, determinista. |
| `mmorch/vault.py` | vault — memoria de largo plazo mmorch-legible sobre el vault Obsidian. |
| `mmorch/wayfinder_prep.py` | wayfinder-prep — investigación autónoma de tickets, decisión humana. |
| `mmorch/weights.py` | weights — gestion de pesos de nodos neuronales (model-cards + verificacion). Source of |
| `mmorch/workflow_engine.py` | workflow_engine — cooperative multi-role workflow as a pure state machine (Phase C). |
| `mmorch/workflow_evolve.py` | workflow_evolve — la poblacion de variantes del engine EVOLUCIONA (backlog #1 |
| `mmorch/workflow_race.py` | workflow_race — corre N VARIANTES de workflow sobre una task congelada del bench y |
| `mmorch/workflow_spec.py` | workflow_spec — load + validate cooperative workflows and role personas (Phase C, Decisions #2/#3). |
| `mmorch/workflow_store.py` | workflow_store — durable block-context + checkpoints for cooperative workflows (Phase A). |
| `mmorch/worktree_driver.py` | worktree_driver — isolated execution in a throwaway git worktree (graft G3 follow-up). |
<!-- /mmorch:auto:modules -->

Otros: `mmorch/mcp_server.py` (MCP wrapper; shim compat en la raiz), `tests/` (regression gate), `vault/` (memoria +
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

**Knowledge vault global (`vault/` + `babel.py`):** research de TODOS los proyectos vive en
el vault Obsidian (deja de estar local por-proyecto). `babel.ingest()` copia el original
(siempre fuente de verdad) y deriva un `.babel.md` comprimido model-native (paper 2606.19857)
solo si pasan DOS gates de ejecución: ratio ≤ 0.7 y fidelidad QA ≥ 0.8 (lector cross-family
que solo ve el babel; grading determinista por containment — jamás LLM-judge). Medido
2026-08: encoder = Gemini (DeepSeek ignora el char-budget en docs >10k), lector = DeepSeek;
símbolos del lexicon en el prompt del encoder ROMPEN la compresión (el lexicon
`vault/lexicon.md` es decoder key del lector). Charts del vault vía `flint-chart-mcp`
(registrado en `.mcp.json`).

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
MCP tools (server `mmorch`): `mmorch_adversarial_verify`, `mmorch_autoresearch`, `mmorch_bucket_rank`, `mmorch_budget_status`, `mmorch_build_spec`, `mmorch_cache_stats`, `mmorch_cascade`, `mmorch_check`, `mmorch_classify`, `mmorch_close_loop`, `mmorch_consolidate`, `mmorch_cynefin`, `mmorch_ensemble_verify`, `mmorch_error_rates`, `mmorch_evolve_nightly`, `mmorch_evolve_self`, `mmorch_fan_out`, `mmorch_feedback_stats`, `mmorch_find_tension`, `mmorch_flag_contradiction`, `mmorch_forget_preview`, `mmorch_ingest_session`, `mmorch_innovate`, `mmorch_intuition`, `mmorch_learn`, `mmorch_memory_stats`, `mmorch_metrics_summary`, `mmorch_open_loops`, `mmorch_orchestra`, `mmorch_pending_review`, `mmorch_perfect`, `mmorch_recall`, `mmorch_record_outcome`, `mmorch_reinforce`, `mmorch_remember`, `mmorch_resolve_review`, `mmorch_review_code`, `mmorch_route`, `mmorch_rubric_next`, `mmorch_rubric_start`, `mmorch_rubric_submit`, `mmorch_session_playbooks`, `mmorch_spec_interview`, `mmorch_speedup`, `mmorch_tournament`, `mmorch_vault_write`.

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
Static gates in one shot: `python scripts/gates.py` (ruff + mypy + paths grep-gate; same criteria as the pre-commit hook).

## Rollback

- MCP: restore `~/.claude.json.bak-mmorch`, remove the `mmorch` key.
- Protocol: delete the `MULTIMODEL_ORCH` block in `~/.claude/CLAUDE.md`.
- Library: delete `~/.claude/orchestration/`.
