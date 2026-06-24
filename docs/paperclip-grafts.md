# Paperclip → mmorch/Lotus — graft roadmap

Mining of `paperclipai/paperclip` (MIT, TS/Node+React+Postgres, 71k★ agent-ops platform).
We do NOT adopt the platform (stack mismatch + it subsumes mmorch+Lotus). We graft the
**ideas**, ported to mmorch (Python/SQLite) + Lotus (Tauri). Order below is value×simplicity,
security-first, foundations-before-features. Effort: S/M/L. Source files cited per item.

## Meta-patterns (the through-line)
1. **Ancestry-everywhere** — goals & issues are adjacency-list trees (`parentId`), traversed BFS w/ depth-cap. Simple, no closure tables.
2. **Policy-as-data** — exec policy, budget policy, allowlist, gate stages are DATA attached to a scope, not hardcoded.
3. **portable vs system_dependent** — every state value is tagged; that split is what makes cross-device sync clean.

---

## Phase 0 — Foundation (substrate for the rest)
- **G1. Job ancestry** `[S]` ✅ DONE (`cf9f42e`) — `mmorch/job_graph.py` + jobs carry `parent`,
  run/* accept `parent_id`, `GET /jobs/{id}/ancestry`. Unlocks G6/G7. *Src: `goals.ts`, `issues.ts`.*
- **G2. portable/system_dependent tagging** `[S]` — tag mmorch config/secrets/paths as `portable`
  or `system_dependent` (abs paths, local cmds, machine meta = system). Substrate for sync (G4).
  *Src: `company-portability.ts`.*

## Phase 1 — Security (close the PTY/exec hole)
- **G3. Exec-policy driver-allowlist** `[M]` ✅ DONE (this commit) — `mmorch/exec_policy.py`
  `evaluate(policy, driver)`; `MMORCH_EXEC_POLICY=any|sandbox` (default any). `sandbox` denies
  LOCAL drivers → gates `/pty/open` + `/run/project` (403); `/state.exec_policy` exposes it.
  FOLLOW-UP: a real `worktree` driver so `sandbox` isolates instead of only denying. *Src: `execution-allowlist.ts`.*

## Phase 2 — Continuity (the Lotus cross-device payoff)
- **G4. Portability export/import** `[L]` — versioned manifest (schemaVersion), bundle = projects+config+
  jobs(+ancestry)+skills+env(portable/placeholder); import reconciliation (slug collision-suffix,
  workspace dedup, orphan-repair); modes `full` / `safe` (safe rejects setup-cmds/triggers = anti-supply-chain).
  Builds on G2. Enables sync across your 2 PCs + phone via Lotus. *Src: `company-portability.ts`.*

## Phase 3 — Governance (configurable, data-driven)
- **G5. Multi-scope budgets** `[M]` — scope `global|project|engine` × `soft(warn%)|hard` × window
  `month|lifetime`; soft→notify, hard→pause scope + cancel work via hook. Upgrades BudgetKeeper.
  *Src: `budgets.ts`.*
- **G6. Staged gates per job** `[M]` — gate policy = `stages:[{type:review|approval, participants}]`,
  comment-required, auto-advance when remaining participants = assignee, `monitor`(timeout/retry).
  Turns binary gates into configurable; direct Lotus gate-modal UX. Uses G1. *Src: `issue-execution-policy.ts`.*
- **G7. Hold + snapshot tree control** `[M]` — cascade pause/cancel/restore over a job subtree via a
  *hold* with *members*=snapshots; `preview()` → affected + skip_reason before applying. Uses G1.
  *Src: `issue-tree-control.ts`.*

## Phase 4 — Learning loop
- **G8. Feedback trace-bundles** `[M]` — up/down vote on a job/output → bundle {content + context +
  run logs + redaction + integrity hash}, consent `pending|local_only`; feed mmorch flywheel/learn.
  *Src: `feedback.ts`.*

## Phase 5 — Durability & scale (heavy / strategic, later)
- **G9. Durable wake/heartbeat runs** `[L]` — resumable jobs (vs thread one-shot): zombie-detection,
  session-reset-on-wake, follow-up queue. Pairs with scheduled-tasks. *Src: `heartbeat.ts`.*
- **G10. Authorization PDP** `[M]` — `decide(actor, action, resource, scope) → {allowed, reason, grant}`;
  principals board/agent/none; only when mmorch goes multi-user/multi-agent. *Src: `authorization.ts`.*
- **G11. Plugin manifest + capability-gated workers** `[L]` — manifest V1 (tools/jobs/events/webhooks/
  ui-slots/db-migrations/env-drivers); isolated worker processes; capability-gated host RPC. Long-term
  extensibility for 3rd-party patterns/checkers. *Src: `plugin-loader.ts`.*

## Adopt-as-is / skip (don't reinvent)
- **Evals**: paperclip uses **promptfoo** (off-the-shelf). Keep mmorch flywheel/exec-embedding (more rigorous);
  optionally add promptfoo for prompt-level regression. *Src: `evals/promptfoo`.*
- **Skills**: folder-per-skill (`SKILL.md`) — already aligned with `~/.claude/skills` + mmorch `session_skills`.
- **Instructions-as-bundle** (`entryFile` AGENTS.md + recover-from-disk over stale config): minor, fold into `prompts.py`.

## Suggested execution order
G1 → G2 → **G3 (security)** → G4 → G5 → G6 → G7 → G8 → (G9, G10, G11 strategic, later).
Each graft: implement in mmorch + verify (HTTP/self-check) + commit, like the chat/minds/transcript/PTY work.
