"""feedback_trace — human vote -> trace bundle + bandit signal (graft G8 from paperclip).

Ported from paperclip's feedback: an up/down vote on a job output is captured as a
trace bundle {vote, job context, transcript, consent, ts} appended durably, AND
fed into mmorch's existing learning loop via feedback.record_outcome (reward 1/0).
Reuses the bandit — does NOT reinvent it.

ponytail: append-only jsonl + a reused record_outcome call. Paths env/param-overridable
so the self-check and HTTP tests don't pollute the real logs. Follow-up (this commit):
conservative PII/secret redaction before a bundle is persisted (so a shared trace can't
leak keys/emails) + a sha256 integrity hash so a shared bundle is tamper-evident.
"""
from __future__ import annotations

import hashlib
import json
import os
import re
import time
from pathlib import Path

_ROOT = Path(__file__).resolve().parent.parent
_TRACES = Path(os.getenv("MMORCH_FEEDBACK_TRACES") or (_ROOT / "logs" / "feedback_traces.jsonl"))

# Conservative: only well-shaped secrets/PII, to avoid mangling legit content (e.g. real hashes).
_REDACTORS = [
    (re.compile(r"[\w.+-]+@[\w-]+\.[\w.-]+"), "[redacted:email]"),
    (re.compile(r"\bsk-[A-Za-z0-9]{16,}\b"), "[redacted:key]"),
    (re.compile(r"\b(?:ghp|gho|github_pat)_[A-Za-z0-9_]{16,}\b"), "[redacted:key]"),
    (re.compile(r"\bAKIA[0-9A-Z]{16}\b"), "[redacted:key]"),
    (re.compile(r"(?i)\bbearer\s+[A-Za-z0-9._\-]{12,}"), "bearer [redacted:token]"),
]


def _redact(value):
    """Scrub well-shaped secrets/emails from any str inside a str/list/dict (recursive)."""
    if isinstance(value, str):
        for rx, repl in _REDACTORS:
            value = rx.sub(repl, value)
        return value
    if isinstance(value, list):
        return [_redact(v) for v in value]
    if isinstance(value, dict):
        return {k: _redact(v) for k, v in value.items()}
    return value


def _hash(bundle: dict) -> str:
    b = {k: v for k, v in bundle.items() if k != "hash"}
    return hashlib.sha256(json.dumps(b, sort_keys=True, ensure_ascii=False).encode("utf-8")).hexdigest()


def verify(bundle: dict) -> bool:
    """True iff the bundle's stored hash matches its content (tamper-evidence for shared traces)."""
    return bool(bundle.get("hash")) and bundle["hash"] == _hash(bundle)


def record_vote(job_id: str, vote: str, *, arm: str = "", comment: str = "", context: str = "",
                transcript=None, consent: str = "local_only", redact: bool = True,
                traces_path=None, outcome_path=None) -> dict:
    up = (vote == "up")
    bundle = {
        "job_id": job_id, "vote": vote, "arm": arm,
        "comment": (comment or "")[:500], "context": (context or "")[:500],
        "transcript": transcript or [], "consent": consent, "ts": time.time() * 1000.0,
    }
    if redact:
        bundle = _redact(bundle)
    bundle["hash"] = _hash(bundle)            # over the (already redacted) content
    p = Path(traces_path) if traces_path else _TRACES
    p.parent.mkdir(parents=True, exist_ok=True)
    with p.open("a", encoding="utf-8") as f:
        f.write(json.dumps(bundle, ensure_ascii=False) + "\n")
    if arm:                                   # feed the existing bandit only if we know the producer
        from .feedback import record_outcome
        kw: dict = {"source": "human_vote", "context": (context or "")[:200]}
        if outcome_path:
            kw["path"] = Path(outcome_path)
        record_outcome(arm, 1.0 if up else 0.0, **kw)
    return bundle


if __name__ == "__main__":
    import tempfile
    d = tempfile.mkdtemp()
    tr = os.path.join(d, "traces.jsonl")
    oc = os.path.join(d, "outcomes.jsonl")
    b = record_vote("job-1", "up", arm="deepseek-chat", comment="clean fix",
                    context="refactor auth", transcript=[{"model": "deepseek", "role": "coder"}],
                    traces_path=tr, outcome_path=oc)
    assert b["vote"] == "up" and b["arm"] == "deepseek-chat"
    lines = Path(tr).read_text(encoding="utf-8").splitlines()
    assert len(lines) == 1 and json.loads(lines[0])["job_id"] == "job-1"
    # down-vote with no arm -> trace only, no bandit write
    record_vote("job-2", "down", traces_path=tr)
    assert len(Path(tr).read_text(encoding="utf-8").splitlines()) == 2
    assert Path(oc).exists() and len(Path(oc).read_text(encoding="utf-8").splitlines()) == 1, "bandit fed once"
    # redaction + integrity hash
    secret = record_vote("job-3", "up", comment="ping me at a@b.com key sk-ABCDEFGHIJKLMNOP123",
                         transcript=[{"role": "c", "out": "token AKIAABCDEFGHIJKLMNOP"}],
                         traces_path=tr)
    assert "a@b.com" not in secret["comment"] and "sk-ABCDEFGH" not in secret["comment"], secret
    assert "[redacted:email]" in secret["comment"] and "[redacted:key]" in secret["comment"]
    assert "AKIA" not in secret["transcript"][0]["out"], "nested redaction"
    assert verify(secret), "hash should validate"
    secret["comment"] += " tampered"
    assert not verify(secret), "tamper must break the hash"
    # opt-out keeps raw content but still hashes
    raw = record_vote("job-4", "up", comment="a@b.com", redact=False, traces_path=tr)
    assert "a@b.com" in raw["comment"] and verify(raw)
    print("feedback_trace OK")
