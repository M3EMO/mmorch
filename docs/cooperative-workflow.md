# Cooperative workflow for mmorch — design (block-context checkpoints + role-chain)

Status: **DESIGN ONLY** (no code yet). Decision doc. Grounded in a code-level read of
`OpenBMB/ChatDev` (3 parallel readers: workflow engine / roles+hierarchy / state+memory)
+ what mmorch already has.

## Goal / non-goals
- **Goal**: a durable, resumable, **cero-cupo** multi-role workflow (architect→coder→reviewer…),
  where roles hand off work, context is carried as typed **blocks**, and every step is checkpointed.
- **NOT**: a generic arbitrary-graph DAG engine (ChatDev reimplemented Tarjan cycles + a node/edge
  VM — over-build; real ChatDev SDLCs are linear + 2 loops). We build a **role-chain with loop-back**,
  not an arbitrary graph.
- **NOT**: token-level / mid-call resume. Granularity = **step-level** (resume from last completed step).

## What ChatDev teaches (the idea, not the app)
- **Workflow = data** (YAML nodes/edges); typed node executors; cycles via Tarjan + `loop_counter(max_iter)`
  + keyword exit marker `<INFO> FINISHED`. *(graph.py, cycle_manager.py, ChatDev_v1.yaml)*
- **Roles = data** (persona text) bound to agent nodes; phase = literal-instruction-node → agent-executor-node;
  inception prompting; hierarchy (CEO/coder/reviewer/tester) is **implicit by phase order**, not a structure.
  *(agent_executor.py `_build_system_prompt`, RoleConfig)*
- **Self-correction** = review/test loops with **role isolation** (tester reports, coder fixes) + keyword
  markers + iteration caps. NOT debate.
- **State** = message-passing between node queues (no central blackboard) + on-disk artifacts (WareHouse) as
  source of truth + per-run FAISS memory (time-decay, load-at-start/save-at-end). *(graph_context.py,
  utils/attachments.py `AttachmentStore`, memory_base.py)*
- **Context as typed blocks**: a Message's content is `str | List[MessageBlock]`; files are registered in an
  **`AttachmentStore`** as records `{id, kind, mime_type, path}` with a `manifest.json`; messages reference
  blocks. *(entity/messages.py, utils/attachments.py)* ← **this is the "AttRes" model we adopt.**
- **No checkpoint/resume** — runs are fresh, final YAML dump only. ← **mmorch's net-new value.**

## What mmorch already has (≈70%)
- `rubric_loop` = the phase-loop primitive (gen↔judge, K cap, escalate=exit). `project_loop` is **better than
  ChatDev** (gate = real test execution, not a self-declared `FINISHED` marker).
- `transcript_store` = per-turn log (but **in-memory** → lost on restart).
- `chat_store` = SQLite history (the durability pattern to reuse).
- `patterns.py` (fan_out / cascade / tournament), cross-family verify invariant, `worktree_driver` (G3
  isolation), `durable_runs` (G9 reaper), `feedback_trace` (block bundles + hash).
- Missing: **multi-role hand-off as data** + **durable block-context checkpoints**.

---

## The block-context model (AttRes-style) — the substrate
User's insight: context should be **per-block**, like ChatDev's AttachmentStore. So checkpoints don't store
flat text — they **reference typed, content-addressed blocks**.

### `block_store` (new)
A block = a typed, content-addressed unit of context.
```
Block = {
  id:           sha256(content)[:16],   # content-addressed -> automatic dedup
  kind:         "text"|"code"|"file"|"diff"|"plan"|"verdict"|"test_report"|"image_ref",
  mime:         "text/markdown" | "text/x-python" | ...,
  size:         int,
  body:         str,            # inline for small text
  path:         str,            # OR a path, for large/binary artifacts (like AttachmentStore)
  derives_from: [block_id,...], # OPTIONAL lineage: this block was derived from these (ancestry-everywhere, cf G1/G7)
  ts:           float,          # first-seen
  last_put_ts:  float,          # bumped on every (deduped) re-put -> the GC race-guard clock
}
```
- `put(content, kind, mime, derives_from=[]) -> block_id` — dedup: identical content reuses the same id
  (ChatDev's manifest idea); each put bumps `last_put_ts`. `derives_from` records lineage (and keeps an ancestor
  alive via refcount — see GC, Decisions #5).
- `get(block_id) -> Block`; `manifest()` — SQLite table or jsonl, same env-overridable pattern as chat_store.
- Large artifacts (generated files, diffs) stored by `path` and referenced; small text inline. Mirrors
  `AttachmentStore.register_file(copy_file=False, persist=True)`.

### Why blocks beat flat checkpoints (the four wins, restated on blocks)
1. **Context por bloque** — a worker assembles context by pulling the blocks it needs (the `plan` block + the
   latest `code` block), not re-reading a transcript. = ChatDev `carry_data`/`clear_context`, but typed + addressed.
2. **Dedup** — a 200-line file referenced by 5 steps = **1 block**, not 5 copies.
3. **Resume** — reload last checkpoint + its block refs = the accumulated typed context; the worker continues.
4. **Cooperative hand-off** — coder emits a `code` block; reviewer consumes that `block_id`; the verdict is a
   `verdict` block. **The chain of blocks IS the shared durable blackboard** ChatDev keeps only in memory.

### Scoping — global? per-task? per-worker? (RESOLVED: all three, different layers)
Not a pick-one. Each level answers a different layer:

| Layer | Scope | Why |
|---|---|---|
| **Storage** (the physical block pool) | **global** | Content-addressed (`id=sha256`): identical content = one block regardless of author, so **dedup only works if the pool is global**. No identity leak — you can only fetch a block whose id you already hold, and ids reach you via *your* chain. |
| **Visibility** (what a worker sees = the checkpoint chain) | **per-task** (per workflow run) | The real isolation boundary. One run (architect→coder→reviewer of one feature) shares; different runs don't auto-mix. Default-isolated, like ChatDev's per-run GraphContext. |
| **Worker context** (what a step consumes) | **a view** (subset, not a separate store) | The role-chain `consumes:[block_id]` selects only the blocks that step needs (plan + latest code). = ChatDev `carry_data`/`clear_context`. Per-worker-*only* would break cooperation (workers couldn't see each other). |
| **Cross-task reuse** (optional bridge) | **opt-in promotion** to `project:X` / `global` | A reusable plan/pattern. = ChatDev's per-workflow FAISS memory, but explicit, not default. Maps to mmorch's existing scoping (budget global/family, projects per-project). |

**Schema consequence:** scope lives on the **checkpoint**, not the block. `checkpoint.job_id` ties a chain to its
task. A promoted block = one row in `block_scope{block_id, scope:"project:X"|"global"}` that makes it visible
outside its task. The block itself never changes — promotion is just a visibility index. **Default resolution
of a step's consumable blocks** = (blocks from my task's checkpoint chain) ∪ (blocks promoted to my project) ∪
(globally promoted blocks).

---

## Phase A — `block_store` + `checkpoint_store` (the foundation; build first)
### `checkpoint_store` (new) — references blocks, stays light
```
Checkpoint = {
  job_id, step:int, role:str, ts:float,
  parent_step:int|None,
  inputs:  [block_id, ...],     # blocks this step consumed
  outputs: [block_id, ...],     # blocks this step produced
  state:   {...small...},       # loop counters, gate name, escalate flags
  gate:    {name, passed:bool}|None,
}
```
- `record(job_id, step, role, *, inputs, outputs, state, gate) -> checkpoint`
- `history(job_id) -> [Checkpoint...]` ordered; `latest(job_id) -> Checkpoint` (resume point).
- Durable (SQLite `workflow.db`, env `MMORCH_WORKFLOW_DB`), gitignored runtime state. Tables: `blocks`,
  `checkpoints`, `block_scope` (see Decisions #1).
- **Wire**: `rubric_loop` + `project_loop` call `checkpoint_store.record(...)` per iteration (transcript_store
  still gets the human-readable text; checkpoint adds the resumable + deduped block state).
- **Standalone value even without B/C**: durable tracking; the G9 reaper can show a zombie's **last checkpoint**
  (where it died); foundation for everything else. Effort: **S/M**.

## Phase B — resume-from-checkpoint (the G9 heavy-half, now tractable)
- Explicit `POST /jobs/{id}/resume` (Decisions #4): a job with checkpoints but no terminal status →
  **re-dispatch from `latest`**: reload its output blocks as the new seed state, continue the loop. No LLM
  replay — resume from last completed step. No auto-resume on boot; the reaper just flags resumable jobs.
- This is what makes "durable wake-runs" real without token-level checkpointing. Effort: **M**.

## Phase C — role-chain cooperative workflow (the ChatDev-like dynamics)
A workflow = an ordered list of role-steps **as data** (NOT an arbitrary graph):
```
workflow = [
  {role:"architect", family:"google",   produces:"plan"},
  {role:"coder",     family:"deepseek", consumes:["plan"], produces:"code", gate:"tests"},
  {role:"reviewer",  family:"google",   consumes:["code"], gate:"verdict",
                     loop_back:"coder", max:3},
]
```
- Each step: assemble context from `consumes` block ids → run (persona + instruction, inception prompt) →
  write output as a typed block → `checkpoint_store.record` → hand off to next step.
- `gate:"tests"` = `project_loop`'s real test execution (mmorch's edge over ChatDev's `FINISHED` marker).
- `gate:"verdict"` = cross-family verifier (mmorch invariant: generator↔verifier **cross-family**, refute-by-default).
- `loop_back` + `max` = ChatDev's review/test cycle (loop_counter + marker), but gated by execution/verdict.
- Reuses `worktree_driver` so a whole cooperative run is isolated under sandbox. Effort: **L** (several sessions).

## mmorch's edge over ChatDev (why build it here, not adopt ChatDev)
- **Cero-cupo** (DeepSeek/Gemini/GLM by cheap API; Claude only orchestrates).
- **Cross-family verification** (refute-by-default) vs ChatDev's same-model self-assessment.
- **Truth = execution** (test gate) vs ChatDev's agent-declared `<INFO> FINISHED`.
- **Durable block-context checkpoints + resume** — ChatDev has none.
- Routing fit (user CLAUDE.md): a recurrent/understood multi-step flow → mmorch, not the native `Workflow` tool (cupo).

## Decisions (all RESOLVED)
- **Block scoping** — storage global (content-addressed dedup) / visibility per-task (checkpoint chain) /
  worker = view (consumes subset) / opt-in promotion to project|global. See §Scoping.
- **#1 Store backend** — a SEPARATE `workflow.db` (env `MMORCH_WORKFLOW_DB`, gitignored, `chat_store` pattern),
  tables `blocks` · `checkpoints` · `block_scope`. chat.db is user-facing conversation; workflow internals have a
  different lifecycle/GC → don't couple.
- **#2 Spec authoring** — BOTH, files canonical. Named workflows = `workflows/<name>.workflow.json` (a dir, like
  `plugins/`) + a `workflow_spec.py` loader mirroring `plugins.discover` (load + validate `_REQUIRED`). Run API
  accepts `{workflow_name}` (saved) OR `{workflow:{...}}` (ad-hoc, for Lotus). Policy-as-data; no new pattern.
- **#3 Role personas** — a small registry: `roles/<name>.md` (plain text, hand-editable like `never-edit.txt`),
  referenced by `role:"coder"` in a step; a step may override with inline `role_prompt`. The role file = persona
  ONLY; the **step** picks `family`/`model` (same role can run on different families per workflow).
- **#4 Resume trigger** — explicit `POST /jobs/{id}/resume` ONLY. No auto-resume on boot (mass re-dispatch +
  cost storm + resurrects jobs that should die; cero-cupo = no spend without intent). The reaper FLAGS resumable
  jobs (has checkpoints + non-terminal) so Lotus shows a "resume" button next to reaped ones.
- **#5 Block GC** — automatic but CONSERVATIVE, on the reaper sweep (no new daemon). The value metric is
  **reference count, NOT wall-clock** (calendar age ≠ decay; an old block you haven't touched is not less valid).
  Reclaim a block iff:
  - `refcount == 0`, where **refcount = (checkpoints referencing it in inputs/outputs) + (blocks referencing it
    via `derives_from` lineage)** — a load-bearing block (referenced by a checkpoint OR a descendant block) survives;
  - AND not in `block_scope` (not promoted);
  - AND `now - last_put_ts > MMORCH_BLOCK_GC_MIN_IDLE` (default **15 min**) — a *pure race guard* (a block just
    `put()` before its step writes the checkpoint), NOT a value TTL. Steps finish in seconds; `last_put_ts` is
    bumped on every (deduped) re-put.
  Time is never the reason a block lives or dies: GC only runs on the reaper sweep, which is **activity-gated**
  (scheduled-task / manual) — no activity → no sweep → nothing collected (this is why "created 30h ago but I
  didn't work" never gets wrongly reclaimed). `gc(dry_run=True)` for inspection; kill switch `MMORCH_BLOCK_GC=off`.
  Self-heals: a wrongly-collected block is recreated by the next content-identical `put()`.

## Non-goals (don't build)
- Arbitrary node/edge graph + Tarjan cycle detection (ChatDev's generic VM). Role-chain + loop-back covers the real cases.
- Token-level / mid-call resume. Step-level only.
- A new framework. Everything reuses providers.call / loops / SQLite / patterns.
