"""feedback_trace — human vote -> trace bundle + bandit signal (graft G8 from paperclip).

Ported from paperclip's feedback: an up/down vote on a job output is captured as a
trace bundle {vote, job context, transcript, consent, ts} appended durably, AND
fed into mmorch's existing learning loop via feedback.record_outcome (reward 1/0).
Reuses the bandit — does NOT reinvent it.

ponytail: append-only jsonl + a reused record_outcome call. Paths env/param-overridable
so the self-check and HTTP tests don't pollute the real logs.
"""
from __future__ import annotations

import json
import os
import time
from pathlib import Path

_ROOT = Path(__file__).resolve().parent.parent
_TRACES = Path(os.getenv("MMORCH_FEEDBACK_TRACES") or (_ROOT / "logs" / "feedback_traces.jsonl"))


def record_vote(job_id: str, vote: str, *, arm: str = "", comment: str = "", context: str = "",
                transcript=None, consent: str = "local_only",
                traces_path=None, outcome_path=None) -> dict:
    up = (vote == "up")
    bundle = {
        "job_id": job_id, "vote": vote, "arm": arm,
        "comment": (comment or "")[:500], "context": (context or "")[:500],
        "transcript": transcript or [], "consent": consent, "ts": time.time() * 1000.0,
    }
    p = Path(traces_path) if traces_path else _TRACES
    p.parent.mkdir(parents=True, exist_ok=True)
    with p.open("a", encoding="utf-8") as f:
        f.write(json.dumps(bundle, ensure_ascii=False) + "\n")
    if arm:                                   # feed the existing bandit only if we know the producer
        from .feedback import record_outcome
        kw = {"source": "human_vote", "context": (context or "")[:200]}
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
    print("feedback_trace OK")
