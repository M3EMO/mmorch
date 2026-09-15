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
- **Cross-family pairing (OneFlow) — required on subjective tasks, unproven as a mechanism.**
  `GOAL.md` requires cross-family pairs for *subjective* work and allows same-family on
  *checkable* work. The decorrelation rationale has never been measured on the subjective
  branch. On the checkable branch it has: family turned out not to be the driver at all
  (see "Measured"). Invariant stands; the *reason* given for it is still untested.
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

| verifier | family | catches bugs | doesn't false-reject | balanced acc | cost |
|---|---|---|---|---|---|
| `deepseek-chat` (thinking OFF) | deepseek | 0.94 | **0.56-0.61** | 0.77 | — |
| **`deepseek-reasoner`** (same weights, thinking ON) | deepseek | 1.00 | 0.994 | **0.997** | **$0.074** |
| `gemini-2.5-flash` | google | 1.00 | 0.982 | 0.991 | $0.135 |
| `glm-4.5-air` | zhipu | 0.99 | 0.977 | 0.986 | $0.332 |

Three models from **three different families** all land at ~0.99. The only one that collapses
to 0.77 is the one with reasoning switched off — same family and same weights as the best of
the four. Cost runs exactly inverse to quality: the best verifier is also the cheapest
(1.8x cheaper than Gemini, 4.5x cheaper than GLM).

The SELF arm replicated independently three times (specificity 0.558 / 0.606 / 0.594 against
three different counterparts), so the ~40% false-reject rate of thinking-off `deepseek-chat`
is stable, not one-run noise.

**What this establishes.** The failure mode of a cheap verifier is not letting errors through
(sensitivity is ~0.94 everywhere) — it is **refuting correct work**: thinking-off DeepSeek
falsely rejects ~40% of right answers. Verifier choice moves balanced accuracy 0.77 -> 0.99.

**What this refutes (about capability, not about GOAL).** The 2026-09-04 cross-family run (self 0.75 vs cross 0.99, McNemar
b=0 c=79, p≈0) was read as evidence for cross-family decorrelation. The intra-family control
run on 2026-09-07 reproduces it almost exactly (0.77 vs 0.997, b=0 c=78, p≈0) using **the same
model, the same weights** — `deepseek-chat` and `deepseek-reasoner` are both `deepseek-v4-flash`
and differ only in `thinking: disabled`. So the effect was **reasoning, not family**. Gemini did
not win by being Google; it won by thinking — and it cost ~2x more than the DeepSeek model that
does it better. A third family (zhipu) reproduces the same pattern at 4.5x the cost.

**Scope — this does NOT refute the `GOAL.md` OneFlow invariant.** That invariant is scoped:
cross-family for *subjective* tasks, same-family allowed on *checkable* ones. Every item here is
checkable (computed truth), i.e. the branch where same-family is already permitted — and the
best verifier measured (`deepseek-reasoner`) is indeed same-family. The result lands inside the
carve-out GOAL already has; the subjective branch, where the invariant actually binds, was not
touched by a single item.

**What remains untested.** Whether cross-family pairing decorrelates errors on *subjective*
tasks (the claim GOAL makes); one task domain (arithmetic) only; injected errors, so this
measures detection, not a verifier's blind spot for its own mistakes. Strictly, a checkable
arithmetic item should go to `checkers.py` and not to any LLM — these arms measure LLM
verifiers on a domain where mmorch's own rule says not to use one.

### Stacking gates (2026-09-08)

A 4-gate ablation (`ablation_stages.py`, n=299, $1.44, 2 models per family so "different
family" and "different model" are separable) tested a specific claim from an SDLC review: that
chaining N review gates compounds false-rejection, so `(1 - false_reject_rate)^N` predicts
survival of good work through the pipeline. **That formula assumes every gate is equally bad —
false.**

| gate | false-rejects (of 150 correct) | specificity |
|---|---|---|
| `deepseek-chat` (thinking OFF) | 61 | 0.593 |
| `deepseek-v4-pro` | 0 | **1.000** |
| `glm-4.5-air` | 2 | 0.987 |
| `glm-5.2` | 1 | 0.993 |

Chained specificity barely moves with gate count: K=2 -> 0.593, K=3 -> 0.587, K=4 -> 0.587.
**A chain's specificity is set by its worst gate, not by how many gates it has.** Adding good
gates next to a bad one is nearly free — and does nothing to undo the bad one's damage.
Sensitivity was 1.000 in all 11 chains: one good gate already catches everything a chain of
four catches, so in this domain stacking gates buys zero extra detection and pays the
false-rejection cost of its weakest link.

Independence held (observed vs. product-of-marginals gap <=0.005 everywhere), but the test was
underpowered to say anything about decorrelation: 3 of 4 gates had 0-2 errors out of 150, so
there was almost nothing to correlate (phi undefined in half the pairs). `deepseek-v4-pro`
measured a **perfect verifier** on this gold set (0 false-rejects, 0 missed bugs, n=299) —
unplanned, and worth a second look. A follow-up on harder items (comparable non-trivial error
rates across gates, larger n) is needed before decorrelation can be confirmed either way; see
`logs/ablation_results.jsonl` (experiment `ablation_stages`) for the full per-pair phi table
and `logs/ablation_stages_items.jsonl` for raw per-item verdicts (re-analyzable without new
API calls).

### Synthesized checkers (2026-09-09/10)

The user's idea: instead of asking a model to *compute* an answer, ask it to *write the
function* and let a sandbox run it. Three gates were measured on the same hard gold set
(`ablation_stages_hard.py`, n=50-80, computed truth):

| gate | what the model does | specificity | cost / item |
|---|---|---|---|
| `deepseek-chat` (manual) | computes the answer in its head | 0.35-0.50 | $0.0046 |
| `code:deepseek-chat` | writes `solve()` for THIS item; sandbox runs it | 0.98 | $0.00003 |
| `code:deepseek-v4-pro` | same, thinking on | **1.00** (130/130) | $0.0017 |
| `synth:deepseek-chat` | writes `solve(params)` ONCE per problem *kind*, promoted against computed truth, then cached | 1.00 | ~$0 (10 calls total) |

Same model, same weights: 0.35 -> 0.98 by changing the *method*, at 150x lower cost. The
two remaining `code:` failures were format (a bare expression, the system prompt echoed
back), not arithmetic. phi between the manual and the code gate of the same model was
-0.07 / +0.06 / +0.11 across three runs: **errors decorrelate by method, not by family.**

`synth:` is `checkers.py` written by the model. A synthesized function is *promoted* only
if it reproduces computed truth on 3 random items **plus one edge instance** (minimum
legal parameters; the EvalPlus finding that edge tests cut pass@1 by 10-29 points). A
function that fails promotion refutes its whole kind (fail-closed). Cost is per kind, not
per item, so n stops mattering.

**Kind-level decorrelation (`ablation_synth_kinds.py`, 57 algorithmic kinds, 6 models,
$0.77).** Each kind has its own reference solver (brute-force verified); the function
receives typed `params`, not prose — an earlier text-parsing design produced 12 false
"failures" from decoy digits in the statement (`n/2`, `1..92`, `responder -1`) and
correlated the models through *my* text. Result:

| model | kinds promoted | kinds failed | missed bugs | cost |
|---|---|---|---|---|
| `deepseek-chat` (thinking off) | 53/57 | **4** | 0 | $0.003 |
| `deepseek-v4-pro` | 57/57 | 0 | 0 | $0.37 |
| **`deepseek-reasoner`** | **57/57** | 0 | 0 | **$0.03** |
| `glm-4.5-air` | 54/54 | 0 | 0 | $0.21 |
| `glm-5.2` | 49/49 | 0 | 0 | $0.16 |

Zero missed bugs in 274 synthesized functions. The four `deepseek-chat` failures are all
real spec bugs — `p*q` instead of `lcm(p,q)` in inclusion-exclusion, swapped terms in the
tiling recurrence, "largest prime factor" returning 1 when the number factors completely
(caught only by the edge item), a wrong transfer matrix for "strings without ab". Every
other model wrote a correct function for every kind, edge cases and a 20-second time
limit included. **phi is undefined for every pair: there are no errors to correlate.**
The decorrelation question (same-family vs cross-family) is unanswerable at this
difficulty — four of five models simply do not fail. What *is* established: for
well-specified, typed, single-function tasks, a thinking model synthesizes a correct
checker essentially always, and `deepseek-reasoner` does it at 12x less than `v4-pro`.

Harness lessons, paid for four times this week: a phi of +1.0 between two different
models was the harness every time (a 60s timeout, a `≡` character through cp1252 stdin,
decoy digits in the prompt, and API-dropped rows counted as failures). Raw model output
and rejected sources are now persisted per row so the next diagnosis is read, not guessed.

### Pipeline vs engine on a real feature (2026-09-10)

One feature (port of a 290-line Python intent matcher to Java 17, 35-assert acceptance test,
Python as oracle), three ways of building it from the same baseline commit:

| arm | who decides at the gates | acceptance | calls | USD | min | interventions |
|---|---|---|---|---|---|---|
| A: `/project` engine | nobody | **red**, 5 attempts | 11-21 | 0.81 | ~120 | 4 engine fixes |
| B: 6-stage pipeline, scripted | the script | **green** (v2) | 31 | 1.13 | 22 | 0 human, 2 harness |
| C: 6-stage pipeline, hybrid | Claude at the gates | **green** | 9 | 0.16 | 9 | 2 by Claude, 4 min |

B and C shared the spec and plan (deepseek-reasoner) and the coder (deepseek-v4-pro); only
what happened after the first red test differed. B's blind fix loop (rewrite all 9 files)
broke its own build; a targeted loop (the model names the files, compile gate with revert)
went green in 2 rounds. C needed one 4-line fix that a `mvn test-compile` gate would have
caught for free. A exposed four engine defects (fixed: 58ce334, ea8b775) and ended one method
short of green with no gate to tell it so. Full write-up: `docs/ab-sdlc-2026-09-10/README.md`.
Next: the wayfinder map `.scratch/sdlc-6-gates/` (13 tickets) — the pipeline replaces the
engine and becomes a per-repo convention.

### 6-stage pipeline on the validation set (2026-09-11/13)

Driver v3 (`.scratch/sdlc-6-gates/research/04-driver-v3/driver_py.py`): spec → Claude spec review
(≤5 questions, from spec-kit `clarify`) → plan → build → test → Claude diff review → PR. Every
gate is deterministic (contract tokens, R<n> traceability, plan allowlist, py_compile,
collect-only, per-unit regression, progress caps, scope of each Claude pass, lint, full-suite
regression by test name) except the two fixed Claude reviews, which may block ONLY by leaving a
failing test. Escalation: 3 fix rounds → deepseek-reasoner ×2 → Claude → human. Coder
deepseek-v4-pro, writer deepseek-reasoner. Protocol and win criterion: ticket 07.

| feature | runs | green | USD (median) | wall min | max level | Claude blocked with a test | human |
|---|---|---|---|---|---|---|---|
| S2 `rate-limiter` (old engine: 0/3) | 5 | 5/5 | 0.03 | 2-3 | 0 | 0 | 0 |
| D3 stuck detector in `build_unit` (mmorch itself) | 1 | 1/1 | 0.04 | 34 (25 = full suite ×2) | 0 | 0 (one NOTE, no test) | 0 |
| S3 `etl-pipeline` (3 cross-importing modules) | 1 | 1/1 | 0.04 | 6 | 0 | 0 | 0 |
| D2 `test-compile` gate in `build_project` (mmorch itself) | 1 | 1/1 | 0.84 | ~60 | 3 | **2, both real** | 0 |
| S1 `lru-ttl-cache` (held-out, run once, last) | 1 | 1/1 | 0.02 | 4 | 0 | 0 | 0 |

D1 (`gen_model` reaches the recursion) was already green at baseline — its acceptance test
stays as a regression test, no run. Verdict against ticket 07: 6/6 green including the
held-out, median US$0.037 per feature, 0 human interventions before level 4 — the pipeline
**wins**. D2 is the only expensive one, and the cost was a driver defect: the target file
contains "```" inside a string and the fence stripper cut it, which pushed the ladder to Claude
twice. The runs surfaced 9 driver defects, all caught by deterministic gates at US$0. The fixed
Claude diff review caught 0 defects in 8 small runs and 2 real ones (a swallowed timeout, an
empty detail) in the one hard run. Per-run logs and findings:
`.scratch/sdlc-6-gates/research/07-resultados.md`. D2/D3 PRs sit on branches
`sdlc/sdlc-D2-r1`, `sdlc/sdlc-D3-r1b` (not merged).

## What's here

<!-- mmorch:auto:stats -->
_Auto-generado por `mmorch.docgen`._ **121 módulos · 46 MCP tools (16 expuestas por default) · 934 tests.** Catálogo: [`docs/generated/catalog.md`](docs/generated/catalog.md).
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
