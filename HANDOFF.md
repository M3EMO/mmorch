# Handoff — 2026-06-13

## Goal
Evolucionar mmorch (orquestador multi-modelo, ahorra cupo Claude) hacia auto-evolutivo/
seguro/barato. Todo goal-gated, anti-scope-creep, cross-family (OneFlow), red-zone nunca autónomo.

## State — 259 tests verde, 26 MCP tools, ~48 módulos, ~70 commits
**Esta sesión (todo committeado, working tree limpio):**
- **Flywheel** (`flywheel/`): SimCLR `code_embedder` **bate bge** (radon AUC 0.88 vs 0.80; CONCAT+GBRT
  0.95). Inferencia NUMPY pura (`mmorch/code_embedder.py`, sin torch, `code_embedder.npz`). Cuello
  probado = la LABEL no la representación; correctitud NO se aprende estático (necesita ejecución).
- **Fase 5** (`shadow_prior.py`): prior contextual k-NN sobre el bandit, scale gated 0→0.8 (tope=gate
  humano), `auto_scale` con evidencia fresca (≥50 nuevos). Desbloqueada con métrica: offline_improvement
  +0.168 con code_embedder en outcomes de código. Wireada en `code_loop.py` (cascade→ejecuta→reward).
- **rubric_loop.py**: loop autocorrección PLANEADOR/GERENTE/EJECUTOR/JUEZ. Checkable→checker $0,
  subjetivo→juez cross-family. Transporte pluggable: API o **modo PLAN** (MCP `mmorch_rubric_start/next/
  submit`, cero API). Cierra lazo: reward→bandit+memoria, destila regla verificada.
- **Hermes-steal** (`HERMES-IDEAS.md`): `trajectory.py` (captura→dataset flywheel + skill distill, solo
  con verdad de ejecución), `memory.recall_keyword/hybrid` (BM25+RRF), `sandbox.py` (backend docker +
  `enforce_policy`), `nudge.py`.
- **Registry v4** (`config.py`): deepseek-v4-flash/pro explícitos + thinking toggle (`extra_body`),
  gemini-3.1-flash-lite = DEFAULT_VERIFIER. Verificado contra APIs en vivo.
- **Observabilidad/costo**: 429+budget-cap `error_class` + `metrics.error_rates`; **cache-hit billing**
  (`cost_usd(cached_tokens)`, `prices.json` precios cache DeepSeek, `cache_stats`) — verificado VIVO 14x;
  prefix-stable (`prompts.py`), off-peak advisory (`schedule.py`), effort-routing (`effort.py`),
  scout entorno-primero (`scout.py`, medido por `scout_delta` no asumido).
- **Build-list workflow**: B1 `goal_guard` wireado en evolve (era DEAD CODE — fix integridad), B2
  `ensemble_degraded` flag, B3 `mmorch_budget_status`.
- **AGENTS.md**: índice cross-agent (patrón agent0ai/dox) → GOAL.md/CLAUDE.md.

## Next
1. **PUSH** (bloqueado): sin remote ni `gh`. Usuario crea repo GitHub+da URL → `git remote add`+push, o instala `gh`.
2. Promovible: cache-hit ya medible → prefix-stable real en hot paths; correr `scout_delta`/Fase 5 con
   datos de código reales (modo plan) pa que `auto_scale` suba en producción.
3. Free-tier hosted como nodo gratis (OpenRouter `:free`/Google quota) en vez del nodo local-CPU (descartado).

## Decisions
- config.py red-zone → precios cache en `prices.json` (datos/amarilla). Off-peak = ADVISORY no daemon.
- Scout/Fase 5 = hipótesis MEDIDA no asumida (anti-scope-creep). goal_aligned = 1 voto, deterministas mandan.
- Nodo local DeepSeek-CPU descartado (lento/débil) → free-tier hosted o factory task-específica.
- `from .X import X` shadowea submódulo `X.py` → exportar función con alias (lección: `run_scout`).
- Retrain encoder v2 (hid192/full) abandonado: WSL se reinició + ganancia marginal sobre 0.88/0.95.

## Read first
`GOAL.md` (contrato), `AGENTS.md` (índice), `SELF-EVOLUTION-PLAN.md`, `HERMES-IDEAS.md`,
`flywheel/RESULTS.md`. Memoria recall `flywheel-simclr-result`, `mmorch-harness`.
Venv Windows `.venv/Scripts/python.exe`; WSL torch `~/flywheel/bin/python` (paths /mnt/c → `MSYS2_ARG_CONV_EXCL='*'`).
