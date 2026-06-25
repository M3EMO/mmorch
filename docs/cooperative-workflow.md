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
  id:    sha256(content)[:16],     # content-addressed -> automatic dedup
  kind:  "text"|"code"|"file"|"diff"|"plan"|"verdict"|"test_report"|"image_ref",
  mime:  "text/markdown" | "text/x-python" | ...,
  size:  int,
  body:  str            # inline for small text
  path:  str            # OR a path, for large/binary artifacts (like AttachmentStore)
  ts:    float,
}
```
- `put(content, kind, mime) -> block_id` — dedup: identical content reuses the same id (ChatDev's manifest idea).
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
- Durable (SQLite), env-overridable path (`MMORCH_CHECKPOINT_DB`), gitignored runtime state.
- **Wire**: `rubric_loop` + `project_loop` call `checkpoint_store.record(...)` per iteration (transcript_store
  still gets the human-readable text; checkpoint adds the resumable + deduped block state).
- **Standalone value even without B/C**: durable tracking; the G9 reaper can show a zombie's **last checkpoint**
  (where it died); foundation for everything else. Effort: **S/M**.

## Phase B — resume-from-checkpoint (the G9 heavy-half, now tractable)
- On start / on reap: a job with checkpoints but no terminal status → **re-dispatch from `latest`**: reload its
  output blocks as the new seed state, continue the loop. No LLM replay — resume from last completed step.
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

## Open decisions
1. Block store backend: extend `chat.db` with `blocks`/`checkpoints` tables, or a separate `workflow.db`? (lean: separate.)
2. Workflow spec authoring: inline JSON via API, or saved `*.workflow.json` files (policy-as-data, like budget_policy)?
3. Role personas: a `RoleConfig` data file (ChatDev-style) vs inline per workflow step? (lean: a small role registry, reusable.)
4. Resume trigger: on server boot, on reap, or explicit `POST /jobs/{id}/resume`? (lean: explicit + reap-suggested.)
5. Block GC: when are blocks reclaimable (no checkpoint references them)? (defer; cheap to keep.)

## Non-goals (don't build)
- Arbitrary node/edge graph + Tarjan cycle detection (ChatDev's generic VM). Role-chain + loop-back covers the real cases.
- Token-level / mid-call resume. Step-level only.
- A new framework. Everything reuses providers.call / loops / SQLite / patterns.
