"""job_graph — adjacency-list ancestry over the in-memory job map (graft G1).

Ported idea from paperclip's goals/issues trees: each job carries an optional
`parent` id; ancestry = walk parents up, subtree = BFS down. No closure table.
Pure functions over a `jobs: {id: {..., "parent": id|None}}` dict — unit-testable
without the server. Depth-capped + cycle-guarded.
"""
from __future__ import annotations

_MAX_DEPTH = 100


def ancestors(jobs: dict, jid: str) -> list[str]:
    """Job ids from the direct parent up to the root (nearest first)."""
    out: list[str] = []
    seen = {jid}
    cur = (jobs.get(jid) or {}).get("parent")
    while cur and cur not in seen and len(out) < _MAX_DEPTH:
        out.append(cur)
        seen.add(cur)
        cur = (jobs.get(cur) or {}).get("parent")
    return out


def children(jobs: dict, jid: str) -> list[str]:
    return [k for k, v in jobs.items() if (v or {}).get("parent") == jid]


def descendants(jobs: dict, jid: str) -> list[str]:
    """All ids under jid, breadth-first (paperclip's BFS), depth-capped + cycle-safe."""
    out: list[str] = []
    seen = {jid}
    frontier = [jid]
    depth = 0
    while frontier and depth < _MAX_DEPTH:
        nxt = []
        for pid in frontier:
            for cid in children(jobs, pid):
                if cid not in seen:
                    seen.add(cid)
                    out.append(cid)
                    nxt.append(cid)
        frontier = nxt
        depth += 1
    return out


def tree(jobs: dict, jid: str) -> dict:
    """Full lineage view for one job: ancestors (up) + node + descendants (down)."""
    return {"node": jid, "ancestors": ancestors(jobs, jid), "descendants": descendants(jobs, jid)}


_TERMINAL = {"done", "error", "approved", "escalate"}


def plan_subtree_cancel(jobs: dict, jid: str) -> dict:
    """Graft G7 (hold + snapshot): plan a cascade cancel over jid + descendants.

    Members = non-terminal jobs (root first), each with a prev_status snapshot taken
    at plan time. Skipped = terminal/missing jobs with a reason. Pure — apply elsewhere.
    """
    ids = [jid] + descendants(jobs, jid)
    members, skipped = [], []
    for i in ids:
        if i not in jobs:
            skipped.append({"id": i, "reason": "not_found"})
            continue
        st = (jobs.get(i) or {}).get("status")
        if st in _TERMINAL:
            skipped.append({"id": i, "reason": "terminal"})
        else:
            members.append({"id": i, "prev_status": st})   # snapshot
    return {"root": jid, "members": members, "skipped": skipped}


if __name__ == "__main__":
    J = {
        "root": {"parent": None},
        "a": {"parent": "root"},
        "b": {"parent": "root"},
        "a1": {"parent": "a"},
        "a2": {"parent": "a"},
        "a1x": {"parent": "a1"},
        "loop1": {"parent": "loop2"}, "loop2": {"parent": "loop1"},  # cycle
    }
    assert ancestors(J, "a1x") == ["a1", "a", "root"], ancestors(J, "a1x")
    assert sorted(children(J, "root")) == ["a", "b"]
    assert sorted(descendants(J, "root")) == ["a", "a1", "a1x", "a2", "b"], sorted(descendants(J, "root"))
    assert ancestors(J, "loop1") == ["loop2"], "cycle must terminate"   # stops at seen
    t = tree(J, "a")
    assert t["ancestors"] == ["root"] and sorted(t["descendants"]) == ["a1", "a1x", "a2"]
    # G7: cascade cancel plan over a status-annotated tree
    S = {
        "r": {"parent": None, "status": "running"},
        "c1": {"parent": "r", "status": "running"},
        "c2": {"parent": "r", "status": "done"},       # terminal -> skipped
        "g": {"parent": "c1", "status": "pending"},
    }
    pc = plan_subtree_cancel(S, "r")
    assert [m["id"] for m in pc["members"]] == ["r", "c1", "g"], pc
    assert pc["members"][0]["prev_status"] == "running"
    assert [s["id"] for s in pc["skipped"]] == ["c2"] and pc["skipped"][0]["reason"] == "terminal"
    print("job_graph OK")
