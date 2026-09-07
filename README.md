# mmorch — Multi-Model Orchestration Harness

**mmorch** is a measurement bench for multi-model verification. It runs generator->verifier
pairs over tasks with **computable ground truth** (deterministic oracles, not LLM judges) and
measures which configuration is actually right, what it costs, and with what confidence
interval. The primary output is measurements -- paired ablations, McNemar, calibration -- not
delegated work.

The orchestration machinery (routing, bandit, memory, MCP server) exists because it is the
apparatus needed *to run those measurements*, and it doubles as a working delegation harness:
bulk generation and verification go to cheap external APIs to free Claude plan quota ("cupo"),
with the high-judgment orchestrator (Opus/Fable) conducting and breaking ties, never a node.
That second use is the apparatus, not the thesis.

**Core ideas**
- **Conductor + orchestra.** A deterministic Python core routes work to model nodes; Opus/Fable
  conducts, never plays.
- **Deterministic oracles over LLM judges.** Checkable claims go to `checkers.py`, never to a
  model (measured: LLM judges ≈74% false-refute on hard checkable tasks). This is the load-
  bearing claim, and the one the measurements keep confirming.
- **Cross-family pairing (OneFlow) — mechanism NOT established.** Generator→verifier pairs are
  routed across families on the *hypothesis* that this decorrelates errors. The 2026-09-07
  ablation refuted family as the explanation for the measured gain: see "Measured" below.
  Kept as the default because it is cheap and harmless, not because it is proven.
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

## Measured

Paired ablations over a seeded gold set of 350 arithmetic items with **computed** labels
(no human, no LLM in the ground truth). Same `--seed` = same items, so arms compare directly.
Every run appends to `logs/ablation_results.jsonl`; re-run with
`python ablation_paired.py --n 350 --self X --cross Y --yes`.

| verifier | catches bugs | doesn't false-reject | balanced acc | cost |
|---|---|---|---|---|
| `deepseek-chat` (thinking OFF) | 0.94 | **0.61** | 0.77 | — |
| `deepseek-reasoner` (same weights, thinking ON) | 1.00 | **0.99** | **0.997** | $0.074 |
| `gemini-2.5-flash` (different family) | 1.00 | 0.98 | 0.991 | $0.135 |

**What this establishes.** The failure mode of a cheap verifier is not letting errors through
(sensitivity is ~0.94 everywhere) — it is **refuting correct work**: thinking-off DeepSeek
falsely rejects ~40% of right answers. Verifier choice moves balanced accuracy 0.77 -> 0.99.

**What this refutes.** The 2026-09-04 cross-family run (self 0.75 vs cross 0.99, McNemar
b=0 c=79, p≈0) was read as evidence for cross-family decorrelation. The intra-family control
run on 2026-09-07 reproduces it almost exactly (0.77 vs 0.997, b=0 c=78, p≈0) using **the same
model, the same weights** — `deepseek-chat` and `deepseek-reasoner` are both `deepseek-v4-flash`
and differ only in `thinking: disabled`. So the effect was **reasoning, not family**. Gemini did
not win by being Google; it won by thinking — and it cost ~2x more than the DeepSeek model that
does it better.

**What remains untested.** Whether cross-family pairing decorrelates errors *at all*; one task
domain (arithmetic) only; injected errors, so this measures detection, not a verifier's blind
spot for its own mistakes.

## What's here

<!-- mmorch:auto:stats -->
_Auto-generado por `mmorch.docgen`._ **134 módulos · 47 MCP tools (15 expuestas por default) · 926 tests.** Catálogo: [`docs/generated/catalog.md`](docs/generated/catalog.md).
<!-- /mmorch:auto:stats -->

<!-- mmorch:auto:modules -->
Tabla de módulos: [`docs/generated/catalog.md`](docs/generated/catalog.md). No editar a mano.
<!-- /mmorch:auto:modules -->

Otros: `mmorch/mcp_server.py` (MCP wrapper; shim compat en la raiz), `tests/` (regression gate), `vault/` (memoria +
research), `smoke_test.py`, `AUDIT_*.md` / `INNOVATION_ROADMAP_*.md`.

> Las secciones entre `<!-- mmorch:auto:* -->` las regenera `python -m mmorch.docgen`
> desde el código (fuente de verdad). No editar a mano.

Model keys, families, endpoints and prices: `mmorch/config.py` (overrides in `prices.json`).
Do not copy registry keys into this README — they drift. Cache-hit billing is instrumented.

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
Lista de tools: [`docs/generated/catalog.md`](docs/generated/catalog.md#mcp-tools). **Restart Claude Code** to load new tools.
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
