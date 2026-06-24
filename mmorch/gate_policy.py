"""gate_policy — staged review/approval gates per job (graft G6 from paperclip).

Ported from paperclip's issue-execution-policy: a job's gate is DATA — a list of
stages [{id, type: review|approval}] the job advances through, comment-required at
each (intentionality). Replaces the binary approve with configurable multi-stage
gating. Pure state machine; the server keeps a per-job gate registry and Lotus drives it.

Actions: approve (advance; last stage -> approved), request_changes, reject.
ponytail: pure start()/advance() with a self-check; participants/auto-advance are a follow-up.
"""
from __future__ import annotations


def start(policy: dict) -> dict:
    stages = policy.get("stages") or [{"id": "approval", "type": "approval"}]
    return {
        "policy": {"stages": stages, "comment_required": bool(policy.get("comment_required", True))},
        "stage": 0,
        "status": "in_review",
        "history": [],
    }


def current_stage(state: dict):
    stages = state["policy"]["stages"]
    i = state.get("stage", 0)
    return stages[i] if 0 <= i < len(stages) else None


def advance(state: dict, action: str, actor: str = "", comment: str = "") -> dict:
    """Return the next state. On a comment-required violation, returns state + 'error' (no change)."""
    if state["status"] in ("approved", "rejected"):
        return {**state, "error": "gate already terminal"}
    cs = current_stage(state)
    if cs is None:
        return {**state, "error": "no active stage"}
    if state["policy"]["comment_required"] and not (comment or "").strip():
        return {**state, "error": "comment required"}
    entry = {"stage": cs["id"], "actor": actor, "action": action, "comment": comment}
    hist = state["history"] + [entry]
    base = {**state, "history": hist, "error": None}
    if action == "reject":
        return {**base, "status": "rejected"}
    if action == "request_changes":
        return {**base, "status": "changes_requested"}
    # approve -> advance one stage; past the last stage = fully approved
    nxt = state.get("stage", 0) + 1
    if nxt >= len(state["policy"]["stages"]):
        return {**base, "stage": nxt, "status": "approved"}
    return {**base, "stage": nxt, "status": "in_review"}


if __name__ == "__main__":
    s = start({"stages": [{"id": "review", "type": "review"},
                          {"id": "approve", "type": "approval"}], "comment_required": True})
    assert s["status"] == "in_review" and s["stage"] == 0
    assert advance(s, "approve", "me", "").get("error") == "comment required"
    s1 = advance(s, "approve", "me", "lgtm")
    assert s1["stage"] == 1 and s1["status"] == "in_review", s1
    s2 = advance(s1, "approve", "me", "ship it")
    assert s2["status"] == "approved" and s2["stage"] == 2, s2
    assert advance(s2, "approve", "me", "x")["error"] == "gate already terminal"
    assert advance(s, "reject", "me", "nope")["status"] == "rejected"
    assert len(s2["history"]) == 2 and s2["history"][0]["stage"] == "review"
    print("gate_policy OK")
