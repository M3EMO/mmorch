"""Server contract probe — the safety net for the server.py decomposition.

Behavior the refactor MUST preserve:
  1. the exact route table (path -> methods) — catches any dropped/renamed/unregistered route,
  2. token auth (protected route 401s without the token, passes with it),
  3. the unauthenticated home page still serves.

Deliberately no-side-effect: only GET/contract checks, no job-spawning POSTs. Runs as pytest
OR `python -m tests.test_server_smoke` (asserts the route table is identical across 3 builds).
"""
from __future__ import annotations

import os

from starlette.testclient import TestClient

from mmorch.server import build_app

# Frozen baseline captured before the refactor. If a route legitimately changes, update HERE
# in the same commit — that makes the contract change explicit and reviewable.
EXPECTED_ROUTES = {
    "/": "GET,HEAD",
    "/state": "GET,HEAD",
    "/events": "GET,HEAD",
    "/run/rubric": "POST",
    "/run/fanout": "POST",
    "/projects": "GET,HEAD,POST",
    "/run/project": "POST",
    "/run/workflow": "POST",
    "/chat": "POST",
    "/chat/history": "GET,HEAD",
    "/minds": "GET,HEAD",
    "/transcript/{job_id}": "GET,HEAD",
    "/jobs/{job_id}/ancestry": "GET,HEAD",
    "/jobs/{job_id}/cancel-tree": "POST",
    "/jobs/reap": "POST",
    "/jobs/{job_id}/checkpoints": "GET,HEAD",
    "/jobs/{job_id}/resume": "POST",
    "/jobs/{job_id}/pause": "POST",
    "/blocks/{block_id}": "GET,HEAD",
    "/plugins": "GET,HEAD",
    "/plugins/{name}/invoke": "POST",
    "/jobs/{job_id}/gate": "GET,HEAD,POST",
    "/jobs/{job_id}/gate/advance": "POST",
    "/budget/policies": "GET,HEAD,POST",
    "/feedback": "POST",
    "/export": "GET,HEAD",
    "/import": "POST",
    "/pty/open": "POST",
    "/pty/{sid}/stream": "GET,HEAD",
    "/pty/{sid}/input": "POST",
    "/pty/{sid}/resize": "POST",
    "/pty/{sid}/close": "POST",
    "/sync/pull": "POST",
    "/fleet": "GET,HEAD,POST",
    "/fleet/run": "POST",
    "/kill/{job_id}": "POST",
    "/approve/{job_id}": "POST",
}


def _route_table(app) -> dict:
    return {getattr(r, "path", ""): ",".join(sorted(getattr(r, "methods", []) or []))
            for r in app.routes if getattr(r, "path", "")}


def test_route_table_matches_contract():
    assert _route_table(build_app()) == EXPECTED_ROUTES


def test_home_serves_without_auth():
    c = TestClient(build_app())
    r = c.get("/")
    assert r.status_code == 200 and "<!DOCTYPE html>" in r.text


def test_protected_route_requires_token():
    prev = os.environ.get("MMORCH_SERVER_TOKEN")
    os.environ["MMORCH_SERVER_TOKEN"] = "probe-secret"
    try:
        c = TestClient(build_app())
        assert c.get("/state").status_code == 401                       # no token -> rejected
        assert c.get("/state", headers={"X-Token": "probe-secret"}).status_code == 200  # token -> ok
    finally:
        if prev is None:
            os.environ.pop("MMORCH_SERVER_TOKEN", None)
        else:
            os.environ["MMORCH_SERVER_TOKEN"] = prev


if __name__ == "__main__":
    # determinism: the route table is identical across 3 independent builds.
    tables = [_route_table(build_app()) for _ in range(3)]
    assert tables[0] == tables[1] == tables[2] == EXPECTED_ROUTES, "route table not deterministic / drifted"
    test_home_serves_without_auth()
    test_protected_route_requires_token()
    print(f"server smoke OK — {len(EXPECTED_ROUTES)} routes stable 3x, home serves, auth enforced")
