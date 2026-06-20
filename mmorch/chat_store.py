"""chat_store — durable chat history for Lotus (SQLite, stdlib).

ponytail: one global connection + a lock. Fine for a single-process Starlette
server with a few worker threads; swap for a pool only if write contention shows.
DB path overridable via MMORCH_CHAT_DB (used by the self-check).
"""
from __future__ import annotations

import os
import sqlite3
import threading
import time
import uuid
from pathlib import Path

_DB = Path(os.getenv("MMORCH_CHAT_DB") or (Path(__file__).resolve().parent.parent / "chat.db"))
_LOCK = threading.Lock()
_CONN = sqlite3.connect(_DB, check_same_thread=False)
_CONN.execute(
    """CREATE TABLE IF NOT EXISTS messages (
        seq     INTEGER PRIMARY KEY AUTOINCREMENT,
        id      TEXT UNIQUE,
        role    TEXT, text TEXT, ts REAL,
        job_id  TEXT, status TEXT, progress TEXT, engine TEXT)"""
)
_CONN.commit()

_COLS = ["id", "role", "text", "ts", "job_id", "status", "progress", "engine"]


def add(role, text, *, job_id=None, status=None, progress=None, engine=None):
    mid = "msg-" + uuid.uuid4().hex[:10]
    ts = time.time() * 1000.0  # ms — matches Lotus `new Date(ts)`
    with _LOCK:
        _CONN.execute(
            "INSERT INTO messages (id,role,text,ts,job_id,status,progress,engine) "
            "VALUES (?,?,?,?,?,?,?,?)",
            (mid, role, text, ts, job_id, status, progress, engine),
        )
        _CONN.commit()
    return {"id": mid, "role": role, "text": text, "ts": ts,
            "job_id": job_id, "status": status, "progress": progress, "engine": engine}


def history(before=None, limit=30):
    """Newest `limit` messages (oldest-first), or the page just before `before`."""
    limit = max(1, min(int(limit), 100))
    with _LOCK:
        if before:
            row = _CONN.execute("SELECT seq FROM messages WHERE id=?", (before,)).fetchone()
            bseq = row[0] if row else (1 << 62)
            rows = _CONN.execute(
                "SELECT id,role,text,ts,job_id,status,progress,engine FROM messages "
                "WHERE seq < ? ORDER BY seq DESC LIMIT ?", (bseq, limit)).fetchall()
        else:
            rows = _CONN.execute(
                "SELECT id,role,text,ts,job_id,status,progress,engine FROM messages "
                "ORDER BY seq DESC LIMIT ?", (limit,)).fetchall()
        oldest = _CONN.execute("SELECT MIN(seq) FROM messages").fetchone()[0]
        page_oldest = None
        if rows:
            page_oldest = _CONN.execute(
                "SELECT seq FROM messages WHERE id=?", (rows[-1][0],)).fetchone()[0]
        has_more = bool(page_oldest and oldest is not None and page_oldest > oldest)
    msgs = [dict(zip(_COLS, r)) for r in rows][::-1]
    return {"messages": msgs, "hasMore": has_more}


if __name__ == "__main__":
    # self-check (run with MMORCH_CHAT_DB pointing at a throwaway file)
    a = add("user", "hello")
    b = add("assistant", "hi there", job_id="job-x", status="running", engine="deepseek")
    h = history(limit=10)
    assert h["messages"][-1]["id"] == b["id"], "newest message last"
    assert h["messages"][0]["role"] == "user", "oldest-first ordering"
    assert set(("messages", "hasMore")) <= set(h), "shape"
    h2 = history(before=b["id"], limit=10)
    assert all(m["id"] != b["id"] for m in h2["messages"]), "`before` excludes the anchor"
    print("chat_store OK:", len(h["messages"]), "msgs · hasMore=", h["hasMore"])
