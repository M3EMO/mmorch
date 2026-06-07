# mmorch — Multi-Model Orchestration Harness

Migrated-pattern harness (design doc §5, §7). Deterministic Python orchestrates
cheap external models as nodes to **free Claude cupo**. Lives global at
`~/.claude/orchestration/`, usable from any project.

## What's here

<!-- mmorch:auto:stats -->
_Auto-generado por `mmorch.docgen`._ **14 módulos · 8 MCP tools · 44 tests.**
<!-- /mmorch:auto:stats -->

<!-- mmorch:auto:modules -->
| Módulo | Qué hace |
|---|---|
| `mmorch/cache.py` | memo (I-4) — cache content-hash de resultados/verdicts. Salta re-gen/re-verify |
| `mmorch/cascade.py` | cascade — FrugalGPT-style multi-step confidence cascade (research: vault/research/ |
| `mmorch/config.py` | Model registry — single source of truth for models, families, endpoints, prices. |
| `mmorch/cost.py` | Cost model — USD from token counts, using REGISTRY prices. |
| `mmorch/ensemble.py` | ensemble_verify (I-3) — K escepticos cross-family + voto mayoria. |
| `mmorch/evolve.py` | evolve — subset DGM-inspirado, GATED (research: vault/research/ |
| `mmorch/feedback.py` | feedback — el lazo que faltaba (la 'loss' ausente). mmorch genera/verifica/ |
| `mmorch/innovate.py` | innovate (I-5) — motor de innovacion productizado. mmorch se idea capacidades |
| `mmorch/learn.py` | learn — meta-inteligencia: mmorch aprende de su propio metrics.jsonl (I-1). |
| `mmorch/metrics.py` | Observability — append-only JSONL metric log (§11 backbone). |
| `mmorch/patterns.py` | Code-flow patterns (§7), migrated as deterministic Python. |
| `mmorch/providers.py` | Provider layer — thin OpenAI-compatible client per external model. |
| `mmorch/route.py` | route (I-2) — confidence-gated escalation. Ahorra cupo: el modelo barato |
| `mmorch/vault.py` | vault — memoria de largo plazo mmorch-legible sobre el vault Obsidian. |
<!-- /mmorch:auto:modules -->

Otros: `mcp_server.py` (MCP wrapper), `tests/` (regression gate), `vault/` (memoria +
research), `smoke_test.py`, `AUDIT_*.md` / `INNOVATION_ROADMAP_*.md`.

> Las secciones entre `<!-- mmorch:auto:* -->` las regenera `python -m mmorch.docgen`
> desde el código (fuente de verdad). No editar a mano.

Active models: `deepseek-chat` (bulk), `gemini-2.5-flash` (cross-family verifier),
`gemini-2.5-flash-lite` (routing). Kimi configured, inactive (no key).

**Self-evolution (Rasputin gated):** mmorch self-audits and self-ideates new capabilities
using itself (fan_out → cross-family verify → Opus tie-break). NEVER self-modifies live
without green tests + human gate. Backlog: tournament, bucket-rank, loop-until-done,
cascade (FrugalGPT-style), schema-gates.

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

`adversarial_verify` raises if generator and verifier share a family (OneFlow).

## Use as MCP tools (inside Claude Code)

Registered globally in `~/.claude.json` as server `mmorch`. Calling these spends
external API $, not cupo — that's the point.

<!-- mmorch:auto:tools -->
MCP tools (server `mmorch`): `mmorch_adversarial_verify`, `mmorch_cascade`, `mmorch_ensemble_verify`, `mmorch_fan_out`, `mmorch_innovate`, `mmorch_learn`, `mmorch_metrics_summary`, `mmorch_route`.

**Restart Claude Code** to load new tools.
<!-- /mmorch:auto:tools -->

## Smoke test

```
.venv\Scripts\python.exe smoke_test.py
```
Runs fan_out on DeepSeek + a planted-bug adversarial_verify on Gemini, then prints
the cost summary and metrics log path.

## Metrics

Every node call appends to `logs/metrics.jsonl`. `mmorch.metrics.summary()` aggregates
cost by family/model — the input to the break-even test (§14, §18.4).

## Not yet built (by design §14 — add only if metrics justify)

tournament, bucket-rank, loop-until-done, FrugalGPT-style learned-threshold cascade
(see `vault/research/`), the full A/B/C ablation (§18.4), schema-as-contract gates (§9),
Kimi/Moonshot node. Run `python -m pytest tests/` before promoting any new capability.

## Rollback

- MCP: restore `~/.claude.json.bak-mmorch`, remove the `mmorch` key.
- Protocol: delete the `MULTIMODEL_ORCH` block in `~/.claude/CLAUDE.md`.
- Library: delete `~/.claude/orchestration/`.
