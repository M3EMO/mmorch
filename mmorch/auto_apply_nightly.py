"""Nightly adapter for the isolated autonomous promotion circuit."""

from __future__ import annotations

import json
import os
import shlex
import subprocess
import sys
from pathlib import Path

from .auto_apply import merge_candidate, observe, preflight, reconcile
from .promotion import PromotionStore
from .runtime_checkout import ensure_runtime


def _run(path: str | Path, args: list[str], timeout: float) -> bool:
    result = subprocess.run(
        args,
        cwd=str(path),
        capture_output=True,
        timeout=timeout,
        env=os.environ.copy(),
    )
    return result.returncode == 0


def _json_run(path: str | Path, code: str, timeout: float) -> dict:
    result = subprocess.run(
        [sys.executable, "-c", code],
        cwd=str(path),
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=timeout,
        env=os.environ.copy(),
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr[-300:])
    lines = [line for line in result.stdout.splitlines() if line.strip()]
    if not lines:
        raise RuntimeError("subprocess returned no JSON")
    return json.loads(lines[-1])


def collect_signals(code_dir: str | Path) -> dict:
    """Run the frozen executable signals from a candidate/runtime checkout."""
    tests = _run(code_dir, [sys.executable, "-m", "pytest", "tests", "-q"], 1800)
    gates = _run(code_dir, [sys.executable, "scripts/gates.py"], 1800)
    smoke = _run(code_dir, [sys.executable, "scripts/smoke.py"], 300)
    health = _json_run(
        code_dir,
        "import json; from mmorch.health import report; "
        "print(json.dumps(report()['healthy']))",
        60,
    )
    canary = _json_run(
        code_dir,
        "import json; from mmorch.canary import run_canary; "
        "print(json.dumps(run_canary(record=False)))",
        900,
    )
    rates = {
        model: result["pass_rate"]
        for model, result in canary.items()
        if isinstance(result, dict) and "pass_rate" in result
    }
    if not rates or len(rates) != len(canary):
        raise RuntimeError("canary unavailable or skipped")

    from .metrics import error_rates, read_events
    events = read_events()[-200:]
    errors = error_rates(window_n=200)
    calls = sum(v["calls"] for v in errors["by_model"].values())
    error_count = sum(round(v["error_rate"] * v["calls"]) for v in errors["by_model"].values())
    cost = sum(float(event.get("cost_usd") or 0) for event in events)
    return {
        "tests": tests,
        "gates": gates,
        "smoke": smoke,
        "health": health is True,
        "canary": rates,
        "error_rate": error_count / calls if calls else 0.0,
        "cost_per_call": cost / len(events) if events else 0.0,
    }


def _branch_changes(repo: Path, branch: str, base_sha: str):
    from .evolve import Change
    names = subprocess.run(
        ["git", "-C", str(repo), "diff", "--name-only", f"{base_sha}..{branch}"],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=120,
    )
    if names.returncode != 0:
        raise RuntimeError(names.stderr[:180])
    out = []
    for target in names.stdout.splitlines():
        before = subprocess.run(
            ["git", "-C", str(repo), "show", f"{base_sha}:{target}"],
            capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=120,
        )
        after = subprocess.run(
            ["git", "-C", str(repo), "show", f"{branch}:{target}"],
            capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=120,
        )
        if after.returncode != 0:
            raise RuntimeError(f"cannot read candidate file: {target}")
        out.append(Change(target, after.stdout, before.stdout if before.returncode == 0 else "",
                          f"autonomous promotion of {branch}"))
    if not out:
        raise RuntimeError("candidate diff is empty")
    return out


def _judgments(repo: Path, branch: str, base_sha: str) -> tuple[dict, object]:
    from .evolve import _diff_goal_aligned, _pr_fitness
    changes = _branch_changes(repo, branch, base_sha)
    fitness = [
        _pr_fitness(change) if change.target.endswith(".py")
        else {"ok": True, "non_python": True}
        for change in changes
    ]
    alignment = [_diff_goal_aligned(change) for change in changes]
    return (
        {"ok": all(result.get("ok") is True for result in fitness), "files": fitness},
        type("Alignment", (), {"passed": all(result.passed is True for result in alignment)})(),
    )


def _candidate_from_record(rec: dict) -> str | None:
    candidates = [
        (rec.get("auto_repair") or {}).get("branch"),
        (rec.get("hardening") or {}).get("branch"),
        (rec.get("merge_train") or {}).get("train_branch"),
    ]
    return next((str(branch) for branch in candidates if branch), None)


def _restart_from_env() -> None:
    raw = os.getenv("MMORCH_AUTO_APPLY_RESTART", "")
    if not raw.strip():
        raise RuntimeError("MMORCH_AUTO_APPLY_RESTART is required for rollback")
    result = subprocess.run(
        shlex.split(raw, posix=os.name != "nt"),
        capture_output=True,
        timeout=120,
    )
    if result.returncode != 0:
        raise RuntimeError(f"restart command failed: {result.stderr[-180:]!r}")


def run_cycle(
    rec: dict,
    *,
    root: str | Path,
    rollout: str,
    runtime_dir: str | Path,
    signal_fn=collect_signals,
) -> dict:
    """Advance at most one promotion action per nightly invocation."""
    repo = Path(root).resolve()
    runtime = ensure_runtime(repo, runtime_dir)
    store = PromotionStore(repo / "logs" / "auto_apply")
    state = store.load()

    def recovery_check() -> dict:
        recovered = signal_fn(runtime.path)
        return {
            "ok": all(recovered[k] is True for k in ("tests", "gates", "smoke", "health")),
            "signals": recovered,
        }

    if state is not None and state["status"] in {"candidate", "merged", "reverting", "reverted"}:
        state = reconcile(
            store=store,
            runtime=runtime,
            restart_fn=_restart_from_env,
            recovery_fn=recovery_check,
            now=float(rec.get("ts") or __import__("time").time()),
        )
    if state is not None and state["status"] == "halted":
        return {"status": "halted", "reason": state.get("halt_reason", "")}
    if state is not None and state["status"] == "observing":
        sample = signal_fn(runtime.path)
        final = observe(
            store=store,
            runtime=runtime,
            sample=sample,
            now=float(rec.get("ts") or __import__("time").time()),
            restart_fn=_restart_from_env,
            recovery_fn=recovery_check,
        )
        return {"status": final["status"], "promotion_id": final["id"]}
    if state is not None and state["status"] not in {"accepted"}:
        return {"status": state["status"], "reason": "requires recovery"}

    branch = _candidate_from_record(rec)
    if branch is None:
        return {"skipped": "no candidate in nightly record"}
    base_sha = runtime.head()
    from .automerge import classify_branch
    classification = classify_branch(str(repo), branch, base=base_sha)
    expected_hash = classification.get("diff_hash") or ""

    from .worktree_driver import open_worktree
    wt = open_worktree(str(repo), branch=branch)
    try:
        wt.seed([".venv"])
        baseline = signal_fn(wt.path)
    finally:
        wt.close(keep_branch=True)
    fitness, alignment = _judgments(repo, branch, base_sha)
    restart_ready = bool(os.getenv("MMORCH_AUTO_APPLY_RESTART", "").strip())
    verdict = preflight(
        repo=repo,
        runtime=runtime,
        branch=branch,
        expected_base_sha=base_sha,
        expected_diff_hash=expected_hash,
        rollout=rollout,
        quality_fn=lambda: {
            "ok": (restart_ready or rollout == "shadow")
            and all(baseline[k] is True for k in ("tests", "gates", "smoke", "health")),
            "signals": baseline,
        },
        fitness_fn=lambda: fitness,
        alignment_fn=lambda: alignment,
    )
    if rollout == "shadow":
        return {
            "status": "shadow",
            "branch": branch,
            "eligible": verdict.eligible,
            "zone": verdict.zone,
            "reason": verdict.reason,
        }
    if not verdict.apply_allowed:
        return {"status": "rejected", "branch": branch, "reason": verdict.reason}
    final = merge_candidate(
        store=store,
        runtime=runtime,
        branch=branch,
        expected_base_sha=base_sha,
        expected_diff_hash=expected_hash,
        verdict=verdict,
        baseline=baseline,
        now=float(rec.get("ts") or __import__("time").time()),
    )
    return {"status": final["status"], "promotion_id": final["id"], "branch": branch}


def run_nightly(rec: dict, *, root: str | Path, cycle_fn=run_cycle) -> dict:
    """Environment-gated entrypoint; default off and no scheduler mutation."""
    rollout = os.getenv("MMORCH_AUTO_APPLY_ROLLOUT", "off").strip() or "off"
    if rollout == "off":
        return {"skipped": "rollout off"}
    runtime_dir = os.getenv("MMORCH_RUNTIME_DIR", "").strip()
    if not runtime_dir:
        return {"error": "MMORCH_RUNTIME_DIR required"}
    try:
        return cycle_fn(rec, root=root, rollout=rollout, runtime_dir=runtime_dir)
    except Exception as exc:
        return {"error": f"{type(exc).__name__}: {str(exc)[:200]}", "rollout": rollout}
