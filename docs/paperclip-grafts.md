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
- **G2. portable/system_dependent tagging** `[S]` ✅ DONE — `portability.tag(value, kind)`
  (portable|system_dependent|secret). Used by G4. *Src: `company-portability.ts`.*

## Phase 1 — Security (close the PTY/exec hole)
- **G3. Exec-policy driver-allowlist** `[M]` ✅ DONE (this commit) — `mmorch/exec_policy.py`
  `evaluate(policy, driver)`; `MMORCH_EXEC_POLICY=any|sandbox` (default any). `sandbox` denies
  LOCAL drivers → gates `/pty/open` + `/run/project` (403); `/state.exec_policy` exposes it.
  FOLLOW-UP: a real `worktree` driver so `sandbox` isolates instead of only denying. *Src: `execution-allowlist.ts`.*

## Phase 2 — Continuity (the Lotus cross-device payoff)
- **G4. Portability export/import** `[L]` ✅ DONE — `portability.export_bundle/reconcile/import_bundle`;
  `GET /export` (projects path=system_dependent, name portable, fleet token=secret, exec_policy) +
  `POST /import` (reconcile: skip name-collisions, system_dependent paths need local re-provision via
  `overrides`, never apply a stale abs path). Verified: round-trip = no mutation; bad path → needs_path.
  FOLLOW-UP: bundle chat history + skills; Lotus UI for export/import + path-remap. *Src: `company-portability.ts`.*

## Phase 3 — Governance (configurable, data-driven)
- **G5. Multi-scope budgets** `[M]` ✅ DONE — `budget_policy.py` (policies json, `evaluate`,
  `blocking_incident`); scopes `global`(month) + `family:*`(lifetime, the data we track);
  soft@warn% / hard@limit. Hard blocks new work (402) in run/{rubric,fanout,project};
  `GET|POST /budget/policies`; `/state.budget_incidents`. Verified: self-check + HTTP (hard→402→clear→ok).
  FOLLOW-UP: per-project scope (needs per-project cost attribution); soft→notification in Lotus. *Src: `budgets.ts`.*
- **G6. Staged gates per job** `[M]` ✅ DONE — `gate_policy.py` pure state machine (stages
  review|approval, comment-required, approve/request_changes/reject); `_GATES` registry +
  `GET|POST /jobs/{id}/gate` + `POST /jobs/{id}/gate/advance`. Verified: self-check + HTTP
  (2-stage flow, comment-required 400, approve×2→approved, terminal 400). FOLLOW-UP: participants/
  auto-advance + Lotus multi-stage gate modal + attach gate on job execution. *Src: `issue-execution-policy.ts`.*
- **G7. Hold + snapshot tree control** `[M]` ✅ DONE — `job_graph.plan_subtree_cancel` (members w/
  prev_status snapshot, skip terminals) + `POST /jobs/{id}/cancel-tree` (cascade cancel via G1 ancestry).
  Verified: self-check + HTTP (running root+child cancelled, snapshot, root→error). FOLLOW-UP: pause/restore
  modes + Lotus button. *Src: `issue-tree-control.ts`.*

## Phase 4 — Learning loop
- **G8. Feedback trace-bundles** `[M]` ✅ DONE — `feedback_trace.record_vote` (up/down → bundle
  {vote, job context, transcript, consent, ts} appended jsonl + feeds the existing bandit via
  `feedback.record_outcome`); `POST /feedback`. Verified: self-check + HTTP (vote up recorded,
  bad vote 400, trace written). FOLLOW-UP: redaction + integrity hash + share-export pipeline. *Src: `feedback.ts`.*

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
