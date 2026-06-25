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
- **G9. Durable wake/heartbeat runs** `[L]` ✅ DONE (lazy core) — `durable_runs.py`: `touch(job, now)`
  heartbeat + pure `detect_zombies(jobs, now, ttl)` (non-terminal & stale, excludes `gate`); rubric
  loop bumps heartbeat per step; `POST /jobs/reap` (body `{ttl?, dry?}`) marks zombies error + sets
  cancel event, idempotent. Trigger = scheduled-tasks hitting the endpoint (no daemon). `MMORCH_ZOMBIE_TTL`
  (default 1800s). Verified: self-check + HTTP (auth 401, dry no-mutate, only stale reaped, gated/fresh safe).
  FOLLOW-UP (heavy half, deferred): persist `_JOBS` so jobs survive a process restart + follow-up queue
  + session-reset-on-wake. *Src: `heartbeat.ts`.*
- **G10. Authorization PDP** `[M]` — `decide(actor, action, resource, scope) → {allowed, reason, grant}`;
  principals board/agent/none; only when mmorch goes multi-user/multi-agent. *Src: `authorization.ts`.*
- **G11. Plugin manifest + capability-gated workers** `[L]` ✅ DONE — `plugins.py` (manifest load +
  `discover` + `invoke`) + `plugin_worker.py` (subprocess harness, imported by file path only).
  Plugin = dir w/ `plugin.json` {name,version,entry,capabilities,contributes} + entry module whose
  contributions are `fn(args, host)`. Runs in an ISOLATED subprocess; NDJSON RPC; host intercepts
  `host_call`s and grants only caps that are **declared ∩ policy-allowed** (two-layer, default-deny).
  cap = method namespace (`llm.call`→`llm`). `GET /plugins`, `POST /plugins/{name}/invoke`. Host
  services: `log.emit`, `llm.call` (budget-blocked). Env: `MMORCH_PLUGINS_DIR`, `MMORCH_PLUGINS_ALLOW`
  (csv, default ""=deny all), `MMORCH_PLUGIN_TIMEOUT` (30s wall-clock kill). Verified: self-check +
  HTTP (gate: granted log host-side, declared-but-policy-denied fs blocked, undeclared net blocked,
  auth 401, 404/400). **SECURITY LIMIT:** gate controls access to mmorch internals + host RPC, NOT raw
  OS (subprocess runs as same user; a plugin can still touch the fs via plain python). Run semi-trusted
  code only, or pair with OS-level sandbox. FOLLOW-UP: pool workers (vs per-invoke), event/webhook
  contributions, Lotus ui-slots, committed example plugin as authoring template. *Src: `plugin-loader.ts`.*

## Adopt-as-is / skip (don't reinvent)
- **Evals**: paperclip uses **promptfoo** (off-the-shelf). Keep mmorch flywheel/exec-embedding (more rigorous);
  optionally add promptfoo for prompt-level regression. *Src: `evals/promptfoo`.*
- **Skills**: folder-per-skill (`SKILL.md`) — already aligned with `~/.claude/skills` + mmorch `session_skills`.
- **Instructions-as-bundle** (`entryFile` AGENTS.md + recover-from-disk over stale config): minor, fold into `prompts.py`.

## Suggested execution order
G1 → G2 → **G3 (security)** → G4 → G5 → G6 → G7 → G8 → (G9, G10, G11 strategic, later).
Each graft: implement in mmorch + verify (HTTP/self-check) + commit, like the chat/minds/transcript/PTY work.

## Follow-ups — status
DONE:
- G8 redaction + integrity hash on trace bundles (`ebd55d6`).
- G11 example plugin / authoring template at `plugins/example/` (`c383bf5`).
- Lotus UI (repo Lotus `6edb0a9`): exec-policy chip (G3) + budget-incident chip (G5) +
  reap-zombies button (G9) on the dashboard ops strip; cancel-tree action on running cards (G7);
  staged-gate modal w/ simple fallback (G6). `api.js` reap/cancelTree/get+advanceGate.

DEFERRED (reason):
- G3 real `worktree` driver (sandbox isolates vs only denies) — L, exec-path rework.
- G9 heavy half: persist `_JOBS` cross-restart + follow-up queue + session-reset-on-wake — L.
- G7 pause/restore (thread suspension) — blocked on G9-durable (no pause primitive; won't ship fake pause).
- G5 per-project budget scope — needs per-project cost attribution (not tracked).
- G4 bundle chat history + skills; Lotus export/import + path-remap UI — medium, low marginal value now.
- G11 worker pool (vs per-invoke), event/webhook/ui-slot contributions — YAGNI until invoke rate / a real 3rd-party need.
- G6 attach-gate-on-execution + participants/auto-advance — product-shaped.
- **G10 authz PDP** — deferred: mmorch is single-user (decision 2026-06-24).
