"""durable_runs — heartbeat + zombie reaper for in-process jobs (graft G9 from paperclip).

Ported from paperclip's heartbeat.ts. mmorch jobs run as one-shot threads in `_JOBS`;
if a worker dies (crash, hang, process kill) the row sits as `running` forever and the
Kanban shows phantom work. A heartbeat timestamp + a reaper sweep fixes that: any
non-terminal job whose heartbeat is older than a TTL is a zombie -> mark it error.

Pure detection (mirrors job_graph.plan_subtree_cancel from G7): `detect_zombies` reads a
snapshot and returns descriptors; the server applies status + emits under lock. `touch`
bumps a job's heartbeat as it makes progress so legit long jobs aren't reaped.

ponytail: in-process reaper only. Trigger = external scheduled-tasks hitting POST /jobs/reap
(roadmap "pairs with scheduled-tasks"), no daemon thread. Durable-across-restart + a
follow-up queue + session-reset-on-wake are the heavy half of G9 -> follow-ups, add when
jobs must survive a server restart.
"""
from __future__ import annotations

import os

# Statuses that are NOT zombies: terminal (work finished) + gate (waiting on a human by design).
_DONE = {"done", "error", "approved", "escalate", "rejected"}
_WAITING = {"gate"}
_NOT_ZOMBIE = _DONE | _WAITING

_DEFAULT_TTL = 1800.0   # 30 min with no heartbeat == stuck


def default_ttl() -> float:
    try:
        return float(os.getenv("MMORCH_ZOMBIE_TTL") or _DEFAULT_TTL)
    except ValueError:
        return _DEFAULT_TTL


def touch(job: dict, now: float) -> None:
    """Stamp progress so an active job isn't mistaken for a zombie. One line, named for intent."""
    job["heartbeat"] = now


def _last_beat(job: dict) -> float:
    # heartbeat if the job ever reported progress, else its creation ts.
    return float(job.get("heartbeat") or job.get("ts") or 0.0)


def detect_zombies(jobs: dict, *, now: float, ttl: float | None = None) -> list[dict]:
    """Non-terminal jobs whose last heartbeat is older than ttl. Pure: returns descriptors,
    mutates nothing. Caller marks them error + emits."""
    t = default_ttl() if ttl is None else float(ttl)
    out = []
    for jid, j in jobs.items():
        if j.get("status") in _NOT_ZOMBIE:
            continue
        age = now - _last_beat(j)
        if age > t:
            out.append({"id": jid, "status": j.get("status"),
                        "age": round(age, 1), "last": _last_beat(j)})
    return out


if __name__ == "__main__":
    now = 10_000.0
    jobs = {
        "fresh":   {"status": "running", "ts": now - 5},                 # active -> keep
        "beating": {"status": "running", "ts": 0, "heartbeat": now - 5},  # bumped -> keep
        "zombie":  {"status": "running", "ts": now - 4000},              # stale -> reap
        "done":    {"status": "done", "ts": 0},                          # terminal -> keep
        "gated":   {"status": "gate", "ts": 0},                          # waiting human -> keep
    }
    z = detect_zombies(jobs, now=now, ttl=1800)
    ids = {d["id"] for d in z}
    assert ids == {"zombie"}, ids
    assert z[0]["age"] > 1800
    # touch rescues a would-be zombie
    touch(jobs["zombie"], now)
    assert detect_zombies(jobs, now=now, ttl=1800) == []
    # env override honored
    os.environ["MMORCH_ZOMBIE_TTL"] = "10"
    assert default_ttl() == 10.0
    del os.environ["MMORCH_ZOMBIE_TTL"]
    print("durable_runs OK")
