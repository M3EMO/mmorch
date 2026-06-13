# AGENTS.md — mmorch

Standard cross-agent entry point (Codex, OpenCode, Cursor, etc.). Claude Code reads
`CLAUDE.md`; this is the portable pointer so ANY agent gets the invariants before editing.
**This file is an INDEX, not a duplicate** — the live contract is `GOAL.md`.

## What mmorch is
Deterministic Python orchestration **library** (package `mmorch/`) + MCP server. Offloads
bulk generation and cross-family verification to cheap external model APIs (DeepSeek, Gemini)
to conserve the scarce resource: **Claude plan quota ("cupo")**. The orchestrator (Opus/Fable)
is never an external node — it conducts and tie-breaks.

## Hard invariants — read GOAL.md before any change
1. **Red zone = never autonomous** (human gate required): move money/trades/keys; delete data
   outside a sandbox or wipe memory; touch OS/network/hardware outside the repo; modify
   mmorch's own security policy or `GOAL.md`/`GOAL.hash`; external comms in the user's name;
   destroy rollback capability. Editing `config.py` is red-zone (prices live in `prices.json`,
   the yellow/data layer).
2. **OneFlow** — every generator→verifier (or competitor→judge) pair must span DIFFERENT
   model families for SUBJECTIVE tasks (decorrelate errors); same-family OK only for CHECKABLE
   tasks routed to a deterministic checker. `adversarial_verify`/`ensemble_verify` enforce it.
3. **Anti-sycophancy** — the verifier refutes by default; agreement ≠ confirmation; the reward
   label is a REAL execution outcome, never self-reported confidence.
4. **Anti-scope-creep** — do NOT add complexity without a MEASURED metric that justifies it.
   Instrument first, optimize second. New capability stays dormant/gated until data earns it.
5. **Reversibility × blast-radius zones** (green/blue/yellow/red); every auto-applied change is
   gated by `evaluate()` (ast + tests + ensemble + rollback + cost + `goal_aligned`) and
   `goal_guard()` (tamper-halt). `BudgetKeeper` caps monthly spend.

## Where to look (don't re-derive)
- `GOAL.md` — the north-star contract (invariants, non-goals, success metrics). **Authoritative.**
- `CLAUDE.md` — full working notes + routing rule (cupo vs API).
- `SELF-EVOLUTION-PLAN.md` — phase-gate plan + backlog seeds.
- `HERMES-IDEAS.md` — analysis of stolen ideas (kept/skipped) and why.
- `README.md` — overview. `mmorch/nodes.py` — the orchestra registry. `mmorch/evolve.py` — the
  self-evolution engine. `mmorch/checkers.py` — the deterministic verifier battery.

## Editing rules
- Tests live in `tests/`; run `.venv/Scripts/python.exe -m pytest tests/ -q` (Windows) before
  committing. Keep every change reversible and goal-gated.
- Don't commit/push unless asked. Commit messages end with the project's Co-Authored-By trailer.
- After a meaningful edit, update the relevant doc (this index, the plan, or memory) — the repo
  is self-documenting by convention (the dox/AGENTS.md pattern: read local rules, update after).
