"""context_blocks — the durable half of an "auto-compact to info-blocks" scheme for Claude Code.

The harness owns WHEN/HOW MUCH it compacts (a hook can't trigger it, can't read the live %, can't
set a target — confirmed against the docs). What a hook CAN do, and what this module backs:
  - a Stop hook reads `transcript_path`, estimates tokens, and at a threshold composes the session's
    state into a compact INFO-BLOCK and stores it here (so the info survives the harness's lossy
    prose summary),
  - a SessionStart(matcher=compact) hook reads the latest block back and re-injects it.

So we don't fight the harness — we make the right info SURVIVE it, as blocks. Extraction is
deterministic + cero-cupo (no model call on every Stop): structural signals (recent user intents,
files touched, commits) plus a raw tail as a fallback that always works regardless of transcript shape.

CLI for the hooks:  python -m mmorch.context_blocks tick <transcript_path> <session_id>
                    python -m mmorch.context_blocks latest <session_id>
"""
from __future__ import annotations

import json
import os
import sqlite3
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
_DB = Path(os.getenv("MMORCH_CONTEXT_DB", ROOT / "logs" / "context_blocks.db"))
# absolute-token threshold (a hook can't see the window %, so the user sets this near their model's
# ~85%); env-overridable. ~4 chars/token heuristic.
_THRESHOLD = int(os.getenv("MMORCH_CTX_BLOCK_TOKENS", "150000"))


def _conn():
    _DB.parent.mkdir(parents=True, exist_ok=True)
    c = sqlite3.connect(_DB)
    c.execute("CREATE TABLE IF NOT EXISTS context_blocks "
              "(session_id TEXT, ts REAL, est_tokens INTEGER, body TEXT)")
    return c


def estimate_tokens(transcript_path: str) -> int:
    """Rough token estimate: ~4 chars/token over every string value in the transcript JSONL.
    Best-effort and schema-agnostic — sums all string content it can find."""
    p = Path(transcript_path)
    if not p.exists():
        return 0
    chars = 0
    for ln in p.read_text(encoding="utf-8", errors="replace").splitlines():
        if not ln.strip():
            continue
        try:
            obj = json.loads(ln)
        except Exception:
            chars += len(ln)
            continue
        chars += _str_chars(obj)
    return chars // 4


def _str_chars(obj) -> int:
    if isinstance(obj, str):
        return len(obj)
    if isinstance(obj, dict):
        return sum(_str_chars(v) for v in obj.values())
    if isinstance(obj, list):
        return sum(_str_chars(v) for v in obj)
    return 0


def _entries(transcript_path: str) -> list[dict]:
    p = Path(transcript_path)
    if not p.exists():
        return []
    out = []
    for ln in p.read_text(encoding="utf-8", errors="replace").splitlines():
        if ln.strip():
            try:
                out.append(json.loads(ln))
            except Exception:
                pass
    return out


def _text_of(content) -> str:
    """Pull plain text out of a message content field (string, or list of {type,text} blocks)."""
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        return " ".join(b.get("text", "") for b in content
                        if isinstance(b, dict) and b.get("type") == "text")
    return ""


def compose(transcript_path: str, *, recent_users: int = 6, tail_chars: int = 1500) -> str:
    """Compose a compact info-block from the transcript: recent user intents + files touched +
    commits, plus a raw tail fallback. Deterministic, cero-cupo, schema-tolerant."""
    entries = _entries(transcript_path)
    users, files, commits = [], [], []
    for e in entries:
        msg = e.get("message", e)                       # tolerate {message:{...}} or flat
        role = msg.get("role") or e.get("role")
        content = msg.get("content", e.get("content"))
        if role == "user":
            t = _text_of(content).strip()
            if t and not t.startswith("<"):             # skip system-reminder/tool-result noise
                users.append(t.replace("\n", " ")[:200])
        for b in (content if isinstance(content, list) else []):
            if isinstance(b, dict) and b.get("type") == "tool_use":
                inp = b.get("input", {}) or {}
                if b.get("name") in ("Edit", "Write", "NotebookEdit") and inp.get("file_path"):
                    files.append(str(inp["file_path"]))
                cmd = inp.get("command", "")
                if isinstance(cmd, str) and "git commit" in cmd:
                    commits.append("commit")
    parts = ["## session info-block (auto, " + time.strftime("%Y-%m-%d %H:%M") + ")"]
    if users:
        parts.append("recent intents:\n" + "\n".join(f"- {u}" for u in users[-recent_users:]))
    if files:
        seen = list(dict.fromkeys(files))               # dedup, keep order
        parts.append(f"files touched ({len(seen)}): " + ", ".join(seen[-20:]))
    if commits:
        parts.append(f"commits this session: {len(commits)}")
    # raw tail fallback — always present, survives any schema we failed to parse. Skip
    # system-reminder / tool-result noise so it isn't re-injected.
    tail_texts = []
    for e in entries[-6:]:
        t = _text_of((e.get("message", e) if isinstance(e.get("message", e), dict) else {}).get("content", "")).strip()
        if t and "system-reminder" not in t and not t.startswith("<"):
            tail_texts.append(t)
    tail = "\n".join(tail_texts)
    if tail.strip():
        parts.append("tail:\n" + tail.strip()[-tail_chars:])
    return "\n\n".join(parts)


def store(session_id: str, body: str, est_tokens: int = 0) -> None:
    c = _conn()
    try:
        c.execute("INSERT INTO context_blocks VALUES (?,?,?,?)",
                  (session_id, time.time(), est_tokens, body))
        c.commit()
    finally:
        c.close()
    # Mirror into mmorch's RAW episodic memory (cero-cost, no distill) so the captured context is
    # recallable across sessions via mmorch_recall — not only re-injected after compaction. Raw
    # episodic, NOT the curated markdown notes nor distilled semantic notes (no pollution of the
    # curated layer). Fail-open: memory must never break the block store. Off via MMORCH_CTX_MIRROR=0.
    if os.getenv("MMORCH_CTX_MIRROR", "1") != "0":
        try:
            from .memory import write_episode
            write_episode("global", "context_block", body)
        except Exception:
            pass


def latest(session_id: str, *, k: int = 1) -> list[str]:
    c = _conn()
    try:
        rows = c.execute("SELECT body FROM context_blocks WHERE session_id=? ORDER BY ts DESC LIMIT ?",
                         (session_id, k)).fetchall()
        return [r[0] for r in rows]
    finally:
        c.close()


def tick(transcript_path: str, session_id: str, *, threshold: int | None = None) -> str:
    """Stop-hook entrypoint: if the estimated tokens exceed the threshold, compose+store a block
    and return a one-line nudge for stderr; else return "" (no-op). Idempotent-ish: only stores
    when over threshold AND the estimate grew since the last stored block (avoids spamming)."""
    thr = threshold if threshold is not None else _THRESHOLD
    est = estimate_tokens(transcript_path)
    if est < thr:
        return ""
    c = _conn()
    try:
        last = c.execute("SELECT est_tokens FROM context_blocks WHERE session_id=? ORDER BY ts DESC LIMIT 1",
                         (session_id,)).fetchone()
    finally:
        c.close()
    if last and est <= last[0]:                          # already captured at/above this size
        return ""
    store(session_id, compose(transcript_path), est_tokens=est)
    return f"[mmorch] context ~{est} tok >= {thr}; info-block saved for re-inject (run /compact when ready)"


if __name__ == "__main__":
    import sys
    if len(sys.argv) >= 2 and sys.argv[1] == "tick":
        msg = tick(sys.argv[2], sys.argv[3] if len(sys.argv) > 3 else "default")
        if msg:
            print(msg)   # stdout -> the Stop hook forwards it as a nudge
    elif len(sys.argv) >= 2 and sys.argv[1] == "latest":
        for b in latest(sys.argv[2] if len(sys.argv) > 2 else "default", k=int(os.getenv("K", "1"))):
            print(b)
    else:
        # self-check (temp transcript + temp db), cero-cupo
        import tempfile
        os.environ["MMORCH_CTX_MIRROR"] = "0"   # don't touch real mmorch memory during the self-check
        d = Path(tempfile.mkdtemp())
        os.environ["MMORCH_CONTEXT_DB"] = str(d / "ctx.db")
        _DB = d / "ctx.db"
        tp = d / "t.jsonl"
        tp.write_text("\n".join(json.dumps(x) for x in [
            {"message": {"role": "user", "content": "build the parser"}},
            {"message": {"role": "assistant", "content": [
                {"type": "text", "text": "doing it"},
                {"type": "tool_use", "name": "Write", "input": {"file_path": "parser.py"}},
                {"type": "tool_use", "name": "Bash", "input": {"command": "git commit -m x"}}]}},
            {"message": {"role": "user", "content": "<system-reminder>noise</system-reminder>"}},
            {"message": {"role": "user", "content": "now add tests"}},
        ]), encoding="utf-8")
        assert estimate_tokens(str(tp)) > 0
        block = compose(str(tp))
        assert "build the parser" in block and "now add tests" in block, "recent intents"
        assert "parser.py" in block and "files touched" in block, "files"
        assert "commits this session: 1" in block, "commits"
        assert "<system-reminder>" not in block, "noise must be filtered"
        # threshold: tiny transcript under default threshold -> no-op; with threshold=1 -> fires
        assert tick(str(tp), "s1") == "", "under threshold = no-op"
        assert tick(str(tp), "s1", threshold=1).startswith("[mmorch]"), "over threshold fires"
        assert tick(str(tp), "s1", threshold=1) == "", "same size = no re-store (anti-spam)"
        assert latest("s1") and "build the parser" in latest("s1")[0], "stored + retrievable"
        print("context_blocks OK — estimate, compose (intents/files/commits, noise filtered), "
              "threshold gate, anti-spam, store/latest roundtrip")
