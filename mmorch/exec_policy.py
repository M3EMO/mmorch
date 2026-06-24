"""exec_policy — where execution is allowed to run (graft G3 from paperclip).

Ported from paperclip's execution-allowlist: a policy restricts the execution
DRIVER (location), not the commands. Default `any` (dev / trusted tunnel) = no
change. Set MMORCH_EXEC_POLICY=sandbox to forbid LOCAL execution (PTY shells,
local project exec) — for when the server is exposed beyond a fully-trusted
operator. The sandbox/worktree driver itself is a follow-up; until it exists,
`sandbox` policy simply denies local (a lockdown switch).

ponytail: pure function + env read. The enforcement points (server.py) call evaluate().
"""
from __future__ import annotations

import os

_LOCAL_DRIVERS = {"local", "in_process", "ssh"}
_ISOLATED_DRIVERS = {"worktree", "sandbox", "kubernetes"}


def current_policy() -> str:
    p = (os.getenv("MMORCH_EXEC_POLICY", "any") or "any").strip().lower()
    return p if p in ("any", "sandbox") else "any"


def evaluate(policy: str, driver: str) -> dict:
    """Return {allowed, reason, driver}. policy 'any' -> all; 'sandbox' -> isolated only."""
    driver = (driver or "local").lower()
    if policy == "sandbox" and driver in _LOCAL_DRIVERS:
        return {"allowed": False, "driver": driver,
                "reason": f"exec policy 'sandbox' forbids local driver '{driver}' "
                          f"(use an isolated driver: {sorted(_ISOLATED_DRIVERS)})"}
    return {"allowed": True, "driver": driver, "reason": "ok"}


if __name__ == "__main__":
    assert evaluate("any", "local")["allowed"] is True
    assert evaluate("any", "worktree")["allowed"] is True
    assert evaluate("sandbox", "local")["allowed"] is False
    assert evaluate("sandbox", "ssh")["allowed"] is False
    assert evaluate("sandbox", "worktree")["allowed"] is True
    assert evaluate("sandbox", "sandbox")["allowed"] is True
    # unknown policy degrades to permissive 'any' via current_policy(), but evaluate is literal:
    assert evaluate("any", "anything")["allowed"] is True
    print("exec_policy OK")
