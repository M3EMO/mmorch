"""workflow_store — durable block-context + checkpoints for cooperative workflows (Phase A).

The substrate from docs/cooperative-workflow.md:
- BLOCKS: typed, content-addressed (id = sha256(content)) units of context. Dedup is automatic
  (same content -> same id). Optional `derives_from` lineage (ancestry-everywhere, cf G1/G7).
- CHECKPOINTS: light per-step records that REFERENCE blocks (inputs/outputs) — the durable,
  resumable trail of a job. Scope lives on the checkpoint (`job_id`), not the block.
- BLOCK_SCOPE: opt-in promotion of a block to `project:X` / `global` for cross-task reuse.
- GC: reclaim a block iff refcount==0 (checkpoint refs + lineage refs) AND not promoted AND
  last_put older than a small race-guard idle (NOT a wall-clock value TTL). See Decisions #5.

One workflow.db (env MMORCH_WORKFLOW_DB), separate from chat.db. ponytail: one global conn +
a lock (chat_store pattern); content stored inline (path-spill for large/binary = follow-up).
"""
from __future__ import annotations

import hashlib
import json
import os
import sqlite3
import threading
import time
from pathlib import Path

_DB = Path(os.getenv("MMORCH_WORKFLOW_DB") or (Path(__file__).resolve().parent.parent / "workflow.db"))
_LOCK = threading.Lock()
_CONN = sqlite3.connect(_DB, check_same_thread=False)
_CONN.executescript(
    """
    CREATE TABLE IF NOT EXISTS blocks (
        id TEXT PRIMARY KEY, kind TEXT, mime TEXT, size INTEGER,
        body TEXT, path TEXT, derives_from TEXT, ts REAL, last_put_ts REAL);
    CREATE TABLE IF NOT EXISTS checkpoints (
        job_id TEXT, step INTEGER, role TEXT, ts REAL, parent_step INTEGER,
        inputs TEXT, outputs TEXT, state TEXT, gate TEXT,
        PRIMARY KEY (job_id, step));
    CREATE TABLE IF NOT EXISTS block_scope (
        block_id TEXT, scope TEXT, PRIMARY KEY (block_id, scope));
    CREATE TABLE IF NOT EXISTS job_specs (
        job_id TEXT PRIMARY KEY, kind TEXT, spec TEXT, ts REAL);
    """
)
_CONN.commit()


def _now() -> float:
    return time.time()


def _j(v):
    return json.dumps(v if v is not None else [], ensure_ascii=False)


# ── blocks ────────────────────────────────────────────────────────────────── #
def put_block(content: str, kind: str = "text", mime: str = "text/plain",
              derives_from=None) -> str:
    """Content-addressed put. Same content -> same id (dedup). A re-put bumps last_put_ts AND
    UNIONs derives_from (lineage isn't lost across re-puts). Self-references are stripped (a block
    deriving from itself is meaningless and would defeat reachability GC)."""
    content = content if isinstance(content, str) else str(content)
    bid = hashlib.sha256(content.encode("utf-8")).hexdigest()[:32]   # 128-bit: collision-safe
    incoming = {d for d in (derives_from or []) if d != bid}         # strip self-ref
    now = _now()
    with _LOCK:
        row = _CONN.execute("SELECT derives_from FROM blocks WHERE id=?", (bid,)).fetchone()
        if row:
            merged = (set(json.loads(row[0] or "[]")) | incoming) - {bid}
            _CONN.execute("UPDATE blocks SET last_put_ts=?, derives_from=? WHERE id=?",
                          (now, _j(sorted(merged)), bid))
        else:
            _CONN.execute(
                "INSERT INTO blocks (id,kind,mime,size,body,path,derives_from,ts,last_put_ts) "
                "VALUES (?,?,?,?,?,?,?,?,?)",
                (bid, kind, mime, len(content), content, None, _j(sorted(incoming)), now, now))
        _CONN.commit()
    return bid


def get_block(bid: str):
    with _LOCK:
        r = _CONN.execute(
            "SELECT id,kind,mime,size,body,path,derives_from,ts,last_put_ts FROM blocks WHERE id=?",
            (bid,)).fetchone()
    if not r:
        return None
    return {"id": r[0], "kind": r[1], "mime": r[2], "size": r[3], "body": r[4], "path": r[5],
            "derives_from": json.loads(r[6] or "[]"), "ts": r[7], "last_put_ts": r[8]}


def block_manifest(limit: int = 200) -> list:
    """Block metadata (no body) — for listing/inspection."""
    with _LOCK:
        rows = _CONN.execute(
            "SELECT id,kind,mime,size,derives_from,ts,last_put_ts FROM blocks "
            "ORDER BY last_put_ts DESC LIMIT ?", (max(1, min(int(limit), 1000)),)).fetchall()
    return [{"id": r[0], "kind": r[1], "mime": r[2], "size": r[3],
             "derives_from": json.loads(r[4] or "[]"), "ts": r[5], "last_put_ts": r[6]} for r in rows]


def promote_block(bid: str, scope: str) -> bool:
    """Make a block visible beyond its task: scope = 'project:X' | 'global'. Idempotent."""
    with _LOCK:
        _CONN.execute("INSERT OR IGNORE INTO block_scope (block_id,scope) VALUES (?,?)", (bid, scope))
        _CONN.commit()
    return True


def block_scopes(bid: str) -> list:
    with _LOCK:
        rows = _CONN.execute("SELECT scope FROM block_scope WHERE block_id=?", (bid,)).fetchall()
    return [r[0] for r in rows]


# ── checkpoints ───────────────────────────────────────────────────────────── #
def record_checkpoint(job_id: str, step: int, role: str, *, inputs=None, outputs=None,
                      state=None, gate=None, parent_step=None) -> dict:
    """One step of a job: references the blocks it consumed (inputs) and produced (outputs)."""
    now = _now()
    with _LOCK:
        _CONN.execute(
            "INSERT OR REPLACE INTO checkpoints "
            "(job_id,step,role,ts,parent_step,inputs,outputs,state,gate) VALUES (?,?,?,?,?,?,?,?,?)",
            (job_id, int(step), role, now, parent_step, _j(inputs), _j(outputs),
             json.dumps(state or {}, ensure_ascii=False),
             json.dumps(gate, ensure_ascii=False) if gate is not None else None))
        _CONN.commit()
    return {"job_id": job_id, "step": step, "role": role, "ts": now,
            "inputs": inputs or [], "outputs": outputs or [], "state": state or {}, "gate": gate}


def _ck_row(r) -> dict:
    return {"job_id": r[0], "step": r[1], "role": r[2], "ts": r[3], "parent_step": r[4],
            "inputs": json.loads(r[5] or "[]"), "outputs": json.loads(r[6] or "[]"),
            "state": json.loads(r[7] or "{}"), "gate": json.loads(r[8]) if r[8] else None}


_CK_COLS = "job_id,step,role,ts,parent_step,inputs,outputs,state,gate"


def checkpoint_history(job_id: str) -> list:
    with _LOCK:
        rows = _CONN.execute(
            f"SELECT {_CK_COLS} FROM checkpoints WHERE job_id=? ORDER BY step", (job_id,)).fetchall()
    return [_ck_row(r) for r in rows]


def checkpoint_latest(job_id: str):
    with _LOCK:
        r = _CONN.execute(
            f"SELECT {_CK_COLS} FROM checkpoints WHERE job_id=? ORDER BY step DESC LIMIT 1",
            (job_id,)).fetchone()
    return _ck_row(r) if r else None


def jobs_with_checkpoints() -> set:
    with _LOCK:
        rows = _CONN.execute("SELECT DISTINCT job_id FROM checkpoints").fetchall()
    return {r[0] for r in rows}


# ── job specs (Phase B: durable re-dispatch params / resumable state) ───────── #
def record_job_spec(job_id: str, kind: str, spec: dict) -> None:
    """Upsert what's needed to RESUME a job: kind ('rubric'|'project') + its params/state.
    For rubric, `spec['state']` is the JSON-serializable loop state (resume continues from it)."""
    with _LOCK:
        _CONN.execute(
            "INSERT OR REPLACE INTO job_specs (job_id,kind,spec,ts) VALUES (?,?,?,?)",
            (job_id, kind, json.dumps(spec, ensure_ascii=False), _now()))
        _CONN.commit()


def get_job_spec(job_id: str):
    with _LOCK:
        r = _CONN.execute("SELECT kind,spec FROM job_specs WHERE job_id=?", (job_id,)).fetchone()
    return {"kind": r[0], "spec": json.loads(r[1])} if r else None


# ── liveness + GC ─────────────────────────────────────────────────────────── #
def block_refcount(bid: str) -> int:
    """Raw inbound reference count (checkpoint refs + other blocks' lineage; self excluded).
    For inspection only — GC uses REACHABILITY (a block can have refcount 0 yet be a fresh/root
    block). See gc_blocks."""
    n = 0
    with _LOCK:
        for (ins, outs) in _CONN.execute("SELECT inputs,outputs FROM checkpoints").fetchall():
            if bid in set(json.loads(ins or "[]")) | set(json.loads(outs or "[]")):
                n += 1
        for (df,) in _CONN.execute("SELECT derives_from FROM blocks WHERE id!=?", (bid,)).fetchall():
            if bid in json.loads(df or "[]"):
                n += 1
    return n


def _min_idle() -> float:
    try:
        return float(os.getenv("MMORCH_BLOCK_GC_MIN_IDLE") or 900.0)   # 15 min race-guard
    except ValueError:
        return 900.0


def _gc_compute(now: float, idle: float):
    """NO-LOCK (caller holds _LOCK). Liveness by REACHABILITY: roots = blocks referenced by any
    checkpoint; live = roots + every ancestor reachable via derives_from (cycle-safe BFS). A block
    is reclaimable iff NOT live AND NOT promoted AND last_put older than the race-guard idle. This
    collects orphan cycles and self-refs (unreachable) yet keeps lineage-only live ancestors."""
    roots = set()
    for (ins, outs) in _CONN.execute("SELECT inputs,outputs FROM checkpoints").fetchall():
        roots.update(json.loads(ins or "[]"))
        roots.update(json.loads(outs or "[]"))
    lineage = {}
    for (bid, df) in _CONN.execute("SELECT id,derives_from FROM blocks").fetchall():
        lineage[bid] = set(json.loads(df or "[]"))
    live, stack = set(), list(roots)
    while stack:                                   # BFS over derives_from; visited-guard = cycle-safe
        b = stack.pop()
        if b in live:
            continue
        live.add(b)
        stack.extend(lineage.get(b, ()))
    promoted = {r[0] for r in _CONN.execute("SELECT DISTINCT block_id FROM block_scope").fetchall()}
    reclaimable, kept_live, kept_promoted, kept_fresh = [], 0, 0, 0
    for (bid, last_put) in _CONN.execute("SELECT id,last_put_ts FROM blocks").fetchall():
        if bid in live:
            kept_live += 1; continue
        if bid in promoted:
            kept_promoted += 1; continue
        if (now - (last_put or 0)) < idle:         # race-guard: just-put (strict < so idle=0 = no guard)
            kept_fresh += 1; continue
        reclaimable.append(bid)
    return reclaimable, kept_live, kept_promoted, kept_fresh


def gc_blocks(*, dry_run: bool = True, min_idle: float | None = None) -> dict:
    """Reclaim orphan blocks by REACHABILITY (not raw refcount): keep blocks reachable from a
    checkpoint via lineage, keep promoted, keep fresh (last_put within the race-guard idle); collect
    the rest — including orphan lineage CYCLES and self-refs. Atomic (single lock: snapshot + delete
    can't interleave with a put/checkpoint -> no TOCTOU). Deleting a block also drops its block_scope
    rows (no stale-scope resurrection). Kill switch MMORCH_BLOCK_GC=off."""
    if (os.getenv("MMORCH_BLOCK_GC", "on") or "on").lower() == "off":
        return {"disabled": True, "deleted": [], "reclaimable": []}
    idle = _min_idle() if min_idle is None else float(min_idle)
    now = _now()
    with _LOCK:                                    # whole op atomic -> no snapshot/delete race
        reclaimable, kept_live, kept_promoted, kept_fresh = _gc_compute(now, idle)
        deleted = []
        if not dry_run and reclaimable:
            params = [(b,) for b in reclaimable]
            _CONN.executemany("DELETE FROM blocks WHERE id=?", params)
            _CONN.executemany("DELETE FROM block_scope WHERE block_id=?", params)  # no stale scope
            _CONN.commit()
            deleted = reclaimable
    return {"dry_run": dry_run, "min_idle": idle, "reclaimable": reclaimable, "deleted": deleted,
            "kept_live": kept_live, "kept_promoted": kept_promoted, "kept_fresh": kept_fresh}


if __name__ == "__main__":
    # run with MMORCH_WORKFLOW_DB pointing at a throwaway file
    # --- blocks + dedup ---
    a = put_block("plan: build X", kind="plan")
    a2 = put_block("plan: build X", kind="plan")          # identical -> same id (dedup)
    assert a == a2, "content-addressed dedup"
    code1 = put_block("def f(): return 0", kind="code")
    code2 = put_block("def f(): return 1", kind="code", derives_from=[code1])  # lineage
    assert get_block(a)["body"] == "plan: build X"
    assert get_block(code2)["derives_from"] == [code1]

    # --- checkpoints reference blocks ---
    record_checkpoint("job-1", 1, "architect", outputs=[a])
    record_checkpoint("job-1", 2, "coder", inputs=[a], outputs=[code1], gate={"name": "tests", "passed": False})
    record_checkpoint("job-1", 3, "coder", inputs=[a], outputs=[code2], gate={"name": "tests", "passed": True})
    h = checkpoint_history("job-1")
    assert [c["step"] for c in h] == [1, 2, 3], "ordered"
    assert checkpoint_latest("job-1")["step"] == 3
    assert checkpoint_latest("job-1")["gate"]["passed"] is True
    assert jobs_with_checkpoints() == {"job-1"}

    # --- refcount: a (in 3 checkpoints) >0 ; code1 (1 ckpt + 1 lineage) >0 ; orphan ==0 ---
    assert block_refcount(a) == 3, block_refcount(a)
    assert block_refcount(code1) == 2, block_refcount(code1)     # ckpt step2 + code2's lineage
    orphan = put_block("scratch note", kind="text")
    assert block_refcount(orphan) == 0

    # --- GC: race-guard protects fresh blocks even when unreferenced ---
    g_fresh = gc_blocks(dry_run=True, min_idle=10_000)
    assert orphan not in g_fresh["reclaimable"], "fresh orphan must be race-guarded"
    assert g_fresh["kept_fresh"] >= 1

    # --- GC: with idle=0 the orphan is reclaimable; referenced/promoted are NOT ---
    promote_block(a, "global")
    g = gc_blocks(dry_run=True, min_idle=0)
    assert orphan in g["reclaimable"], g
    assert a not in g["reclaimable"] and code1 not in g["reclaimable"] and code2 not in g["reclaimable"]
    # promote an otherwise-orphan and confirm promotion protects it
    orphan2 = put_block("kept by promotion", kind="text"); promote_block(orphan2, "project:X")
    g2 = gc_blocks(dry_run=True, min_idle=0)
    assert orphan2 not in g2["reclaimable"], "promotion protects"

    # --- GC real: deletes ONLY the orphan ---
    res = gc_blocks(dry_run=False, min_idle=0)
    assert orphan in res["deleted"] and get_block(orphan) is None
    assert get_block(a) and get_block(code1) and get_block(code2), "referenced/promoted survive"

    # ===== adversarial cases (cross-family-designed via mmorch) ===== #
    # self-reference is stripped at store (would otherwise be immortal under a naive GC)
    sx = put_block("selfX")
    put_block("selfX", derives_from=[sx])
    assert get_block(sx)["derives_from"] == [], "self-ref must be stripped"

    # derives_from is UNIONed across re-puts (lineage not lost)
    an1 = put_block("anc-one"); an2 = put_block("anc-two")
    ux = put_block("union-block", derives_from=[an1])
    put_block("union-block", derives_from=[an2])
    assert get_block(ux)["derives_from"] == sorted([an1, an2]), "re-put unions lineage"

    # orphan lineage CYCLE A<->B is collected (unreachable from any checkpoint)
    cyA = put_block("cycle-A"); cyB = put_block("cycle-B", derives_from=[cyA])
    put_block("cycle-A", derives_from=[cyB])                       # now A->B and B->A
    gcyc = gc_blocks(dry_run=True, min_idle=0)
    assert cyA in gcyc["reclaimable"] and cyB in gcyc["reclaimable"], "orphan cycle must be reclaimable"

    # lineage-ONLY live ancestor is KEPT (referenced only via a checkpointed descendant's lineage)
    ancL = put_block("ancestor-live")
    descL = put_block("descendant-live", derives_from=[ancL])
    record_checkpoint("job-live", 1, "coder", outputs=[descL])     # only descL is a root
    gliv = gc_blocks(dry_run=True, min_idle=0)
    assert ancL not in gliv["reclaimable"], "live ancestor (lineage-only) must survive"
    assert descL not in gliv["reclaimable"], "checkpoint root must survive"

    # DIRECTION pin (refutes the cross-family false-refute): keep ANCESTORS of roots, COLLECT orphan
    # DESCENDANTS. A block derived FROM a checkpointed ancestor, itself unreferenced, is collectable.
    ancR = put_block("anc-root")
    descO = put_block("desc-orphan", derives_from=[ancR])
    record_checkpoint("job-anc", 1, "coder", outputs=[ancR])       # the ANCESTOR is the root
    gdir = gc_blocks(dry_run=True, min_idle=0)
    assert ancR not in gdir["reclaimable"], "checkpointed ancestor kept"
    assert descO in gdir["reclaimable"], "orphan descendant of a live ancestor must be collected"

    # GC is atomic (single lock spans snapshot+delete) -> no put/checkpoint can interleave: TOCTOU-free.

    # --- kill switch ---
    os.environ["MMORCH_BLOCK_GC"] = "off"
    assert gc_blocks(dry_run=False, min_idle=0).get("disabled") is True
    del os.environ["MMORCH_BLOCK_GC"]

    # --- job specs (Phase B) ---
    assert get_job_spec("job-1") is None
    record_job_spec("job-1", "rubric", {"state": {"phase": "executor", "iteration": 2}})
    sp = get_job_spec("job-1")
    assert sp["kind"] == "rubric" and sp["spec"]["state"]["iteration"] == 2, sp
    record_job_spec("job-1", "rubric", {"state": {"phase": "judge", "iteration": 3}})  # upsert
    assert get_job_spec("job-1")["spec"]["state"]["iteration"] == 3, "upsert"
    print("workflow_store OK")
