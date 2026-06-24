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
    print("job_graph OK")
