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
    """Content-addressed put. Same content -> same id (dedup); a re-put just bumps last_put_ts."""
    content = content if isinstance(content, str) else str(content)
    bid = hashlib.sha256(content.encode("utf-8")).hexdigest()[:16]
    now = _now()
    with _LOCK:
        row = _CONN.execute("SELECT id FROM blocks WHERE id=?", (bid,)).fetchone()
        if row:
            _CONN.execute("UPDATE blocks SET last_put_ts=? WHERE id=?", (now, bid))
        else:
            _CONN.execute(
                "INSERT INTO blocks (id,kind,mime,size,body,path,derives_from,ts,last_put_ts) "
                "VALUES (?,?,?,?,?,?,?,?,?)",
                (bid, kind, mime, len(content), content, None, _j(derives_from), now, now))
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


# ── refcount + GC ─────────────────────────────────────────────────────────── #
def _referenced_ids() -> set:
    """All block ids reachable from a checkpoint (inputs/outputs) or a block's lineage (derives_from)."""
    refs = set()
    with _LOCK:
        for (ins, outs) in _CONN.execute("SELECT inputs,outputs FROM checkpoints").fetchall():
            refs.update(json.loads(ins or "[]"))
            refs.update(json.loads(outs or "[]"))
        for (df,) in _CONN.execute("SELECT derives_from FROM blocks").fetchall():
            refs.update(json.loads(df or "[]"))
    return refs


def block_refcount(bid: str) -> int:
    n = 0
    with _LOCK:
        for (ins, outs) in _CONN.execute("SELECT inputs,outputs FROM checkpoints").fetchall():
            ids = set(json.loads(ins or "[]")) | set(json.loads(outs or "[]"))
            if bid in ids:
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


def gc_blocks(*, dry_run: bool = True, min_idle: float | None = None) -> dict:
    """Reclaim orphan blocks. value metric = REFCOUNT (not wall-clock): a block is collectable iff
    refcount==0 (no checkpoint ref, no lineage ref) AND not promoted AND last_put older than the
    race-guard idle. Kill switch MMORCH_BLOCK_GC=off. Returns {reclaimable|deleted, kept_*}."""
    if (os.getenv("MMORCH_BLOCK_GC", "on") or "on").lower() == "off":
        return {"disabled": True, "deleted": [], "reclaimable": []}
    idle = _min_idle() if min_idle is None else float(min_idle)
    now = _now()
    referenced = _referenced_ids()
    with _LOCK:
        promoted = {r[0] for r in _CONN.execute("SELECT DISTINCT block_id FROM block_scope").fetchall()}
        rows = _CONN.execute("SELECT id,last_put_ts FROM blocks").fetchall()
    reclaimable, kept_ref, kept_promoted, kept_fresh = [], 0, 0, 0
    for bid, last_put in rows:
        if bid in referenced:
            kept_ref += 1; continue
        if bid in promoted:
            kept_promoted += 1; continue
        if (now - (last_put or 0)) <= idle:        # race-guard: just-put, checkpoint maybe pending
            kept_fresh += 1; continue
        reclaimable.append(bid)
    deleted = []
    if not dry_run and reclaimable:
        with _LOCK:
            _CONN.executemany("DELETE FROM blocks WHERE id=?", [(b,) for b in reclaimable])
            _CONN.commit()
        deleted = reclaimable
    return {"dry_run": dry_run, "min_idle": idle, "reclaimable": reclaimable, "deleted": deleted,
            "kept_referenced": kept_ref, "kept_promoted": kept_promoted, "kept_fresh": kept_fresh}


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

    # --- kill switch ---
    os.environ["MMORCH_BLOCK_GC"] = "off"
    assert gc_blocks(dry_run=False, min_idle=0).get("disabled") is True
    del os.environ["MMORCH_BLOCK_GC"]
    print("workflow_store OK")
