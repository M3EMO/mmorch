"""Fail-closed policy and evidence gate for autonomous code promotion."""

from __future__ import annotations

import subprocess
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

from .observation import evaluate as evaluate_observation
from .promotion import PromotionStore
from .runtime_checkout import RuntimeCheckout


ROLLOUTS = {"off", "shadow", "green", "yellow_bounded", "non_red"}
_BOUNDED_DENY = {
    "pyproject.toml",
    "requirements.txt",
    "mmorch/mcp_server.py",
    "mmorch/server.py",
    "mmorch/paths.py",
}


@dataclass(frozen=True)
class GateVerdict:
    eligible: bool
    apply_allowed: bool
    zone: str
    checks: dict[str, bool]
    evidence: dict
    reason: str = ""


def _git(repo: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["git", "-C", str(repo), *args],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=120,
    )


def _explicit_ok(value) -> bool:
    if isinstance(value, bool):
        return value
    if isinstance(value, dict):
        return value.get("ok") is True
    return getattr(value, "passed", None) is True


def bounded_yellow(
    repo: str | Path,
    branch: str,
    *,
    base_sha: str,
    max_files: int = 3,
    max_changed_lines: int = 200,
) -> dict:
    """Mechanical blast-radius ceiling for the intermediate yellow rollout."""
    result = _git(Path(repo), "diff", "--numstat", f"{base_sha}..{branch}")
    if result.returncode != 0:
        return {"ok": False, "reason": f"numstat failed: {result.stderr[:120]}"}
    files: list[str] = []
    changed = 0
    for line in result.stdout.splitlines():
        parts = line.split("\t")
        if len(parts) < 3:
            return {"ok": False, "reason": "unparseable numstat"}
        added, deleted, path = parts[0], parts[1], parts[-1].replace("\\", "/")
        if not added.isdigit() or not deleted.isdigit():
            return {"ok": False, "reason": f"binary or ambiguous diff: {path}"}
        files.append(path)
        changed += int(added) + int(deleted)
    denied = [
        p for p in files
        if p in _BOUNDED_DENY
        or p.endswith((".lock", ".toml"))
        or "/migrations/" in f"/{p}/"
    ]
    ok = bool(files) and len(files) <= max_files and changed <= max_changed_lines and not denied
    return {
        "ok": ok,
        "files": files,
        "changed_lines": changed,
        "denied": denied,
        "reason": "" if ok else "yellow change exceeds bounded rollout",
    }


def preflight(
    *,
    repo: str | Path,
    runtime: RuntimeCheckout,
    branch: str,
    expected_base_sha: str,
    expected_diff_hash: str,
    rollout: str,
    quality_fn: Callable[[], object] | None,
    fitness_fn: Callable[[], object] | None,
    alignment_fn: Callable[[], object] | None,
    classify_fn: Callable[..., dict] | None = None,
    budget_fn: Callable[[], dict] | None = None,
    goal_fn: Callable[[], object] | None = None,
    bounded_fn: Callable[..., dict] | None = None,
) -> GateVerdict:
    """Collect authoritative evidence; any missing/error/ambiguous signal rejects."""
    if rollout not in ROLLOUTS:
        return GateVerdict(False, False, "", {"rollout_valid": False}, {}, "unknown rollout")
    if rollout == "off":
        return GateVerdict(False, False, "", {"rollout_enabled": False}, {}, "rollout off")

    from .automerge import classify_branch
    from .budget import status as budget_status
    from .goal import goal_guard

    classify_fn = classify_fn or classify_branch
    budget_fn = budget_fn or budget_status
    bounded_fn = bounded_fn or bounded_yellow

    checks: dict[str, bool] = {}
    evidence: dict = {}

    def capture(name: str, fn: Callable[[], object] | None) -> object | None:
        if fn is None:
            checks[name] = False
            evidence[name] = {"error": "unavailable"}
            return None
        try:
            value = fn()
            checks[name] = _explicit_ok(value)
            evidence[name] = value
            return value
        except Exception as exc:
            checks[name] = False
            evidence[name] = {"error": f"{type(exc).__name__}: {str(exc)[:160]}"}
            return None

    def checked_budget() -> dict:
        value = budget_fn()
        return {
            **value,
            "ok": value.get("enforced") is True and (value.get("remaining") or 0) > 0,
        }

    def checked_goal() -> bool:
        goal_guard(allow_init=False)
        return True

    capture("goal_guard", goal_fn or checked_goal)
    capture("budget", checked_budget)
    capture("runtime_clean", runtime.clean)
    capture("base_unchanged", lambda: runtime.head() == expected_base_sha)
    classification = capture(
        "classification",
        lambda: {**classify_fn(str(repo), branch, base=expected_base_sha), "ok": True},
    )
    zone = classification.get("zone", "") if isinstance(classification, dict) else ""
    checks["diff_unchanged"] = bool(
        isinstance(classification, dict)
        and classification.get("diff_hash") == expected_diff_hash
    )
    evidence["expected_diff_hash"] = expected_diff_hash

    zone_allowed = zone == "green" or (
        zone == "yellow" and rollout in {"yellow_bounded", "non_red"}
    )
    checks["zone_allowed"] = zone_allowed
    if zone == "yellow" and rollout == "yellow_bounded":
        capture(
            "yellow_bounded",
            lambda: bounded_fn(repo, branch, base_sha=expected_base_sha),
        )
    elif zone == "yellow" and rollout == "non_red":
        checks["yellow_bounded"] = True
        evidence["yellow_bounded"] = {"bypassed_by_rollout": "non_red"}

    capture("quality", quality_fn)
    capture("fitness", fitness_fn)
    capture("alignment", alignment_fn)

    eligible = bool(checks) and all(checks.values())
    reason = "" if eligible else ", ".join(k for k, ok in checks.items() if not ok)
    return GateVerdict(
        eligible=eligible,
        apply_allowed=eligible and rollout not in {"off", "shadow"},
        zone=zone,
        checks=checks,
        evidence=evidence,
        reason=reason,
    )


def _pause(store: PromotionStore) -> None:
    store.root.mkdir(parents=True, exist_ok=True)
    (store.root / "loop_paused").touch()


def _halt(store: PromotionStore, *, reason: str, expected: str | None = None) -> dict:
    _pause(store)
    return store.advance(
        "halted",
        expected=expected,
        evidence={"reason": reason},
        fields={"halt_reason": reason},
    )


def _finish_merge(
    store: PromotionStore,
    runtime: RuntimeCheckout,
    *,
    now: float,
    merge_sha: str | None = None,
) -> dict:
    state = store.load()
    if state is None or state["status"] != "candidate":
        raise RuntimeError("merge completion requires candidate state")
    if merge_sha is None:
        merged = _git(runtime.path, "merge", "--no-edit", "--no-ff", state["branch"])
        if merged.returncode != 0:
            _git(runtime.path, "merge", "--abort")
            return _halt(
                store,
                reason=f"merge failed: {(merged.stdout + merged.stderr)[:180]}",
                expected="candidate",
            )
        merge_sha = runtime.head()
    store.advance(
        "merged",
        expected="candidate",
        now=now,
        evidence={"merge_sha": merge_sha},
        fields={"merge_sha": merge_sha},
    )
    return store.advance(
        "observing",
        expected="merged",
        now=now,
        evidence={"baseline_frozen": True},
        fields={"observation_started_at": now},
    )


def merge_candidate(
    *,
    store: PromotionStore,
    runtime: RuntimeCheckout,
    branch: str,
    expected_base_sha: str,
    expected_diff_hash: str,
    verdict: GateVerdict,
    baseline: dict,
    now: float | None = None,
    promotion_id: str | None = None,
) -> dict:
    """Merge a fully gated candidate into the isolated runtime and start observation."""
    if not verdict.apply_allowed or not verdict.eligible or not all(verdict.checks.values()):
        raise RuntimeError(f"preflight does not authorize apply: {verdict.reason or 'shadow/off'}")
    if runtime.head() != expected_base_sha or not runtime.clean():
        raise RuntimeError("runtime changed after preflight")
    from .observation import validate_sample

    frozen = validate_sample(baseline)
    if not all(frozen[k] is True for k in ("tests", "gates", "smoke", "health")):
        raise RuntimeError("baseline is not healthy")
    ts = time.time() if now is None else float(now)
    state = store.begin(
        branch=branch,
        base_sha=expected_base_sha,
        diff_hash=expected_diff_hash,
        promotion_id=promotion_id,
        now=ts,
        evidence={"preflight": verdict.checks, "zone": verdict.zone},
        fields={
            "baseline": frozen,
            "samples": [],
            "preflight_checks": verdict.checks,
            "zone": verdict.zone,
        },
    )
    if state["status"] != "candidate":
        return state
    return _finish_merge(store, runtime, now=ts)


def _finish_recovery(
    *,
    store: PromotionStore,
    runtime: RuntimeCheckout,
    reason: str,
    revert_sha: str,
    restart_fn: Callable[[], object],
    recovery_fn: Callable[[], object],
    now: float | None,
) -> dict:
    try:
        restart_fn()
        recovered = _explicit_ok(recovery_fn())
    except Exception as exc:
        return _halt(
            store,
            reason=f"recovery verification error: {type(exc).__name__}: {str(exc)[:140]}",
            expected="reverting",
        )
    if not recovered or not runtime.clean():
        return _halt(
            store,
            reason="recovery verification failed",
            expected="reverting",
        )
    store.advance(
        "reverted",
        expected="reverting",
        now=now,
        evidence={"revert_sha": revert_sha, "recovery_verified": True},
        fields={"revert_sha": revert_sha, "recovery_verified": True},
    )
    return _halt(store, reason=f"reverted after: {reason}", expected="reverted")


def rollback(
    *,
    store: PromotionStore,
    runtime: RuntimeCheckout,
    reason: str,
    restart_fn: Callable[[], object],
    recovery_fn: Callable[[], object],
    now: float | None = None,
) -> dict:
    """Revert the merge, restart, verify recovery, and halt regardless of outcome."""
    state = store.load()
    if state is None:
        raise RuntimeError("no active promotion to roll back")
    if state["status"] not in {"merged", "observing", "reverting"}:
        raise RuntimeError(f"cannot roll back promotion in state {state['status']}")
    if state["status"] != "reverting":
        state = store.advance(
            "reverting",
            expected=state["status"],
            now=now,
            evidence={"reason": reason},
            fields={"revert_reason": reason},
        )
    merge_sha = state.get("merge_sha")
    if not merge_sha:
        return _halt(store, reason="rollback missing merge_sha", expected="reverting")
    reverted = _git(runtime.path, "revert", "-m", "1", "--no-edit", merge_sha)
    if reverted.returncode != 0:
        _git(runtime.path, "revert", "--abort")
        return _halt(
            store,
            reason=f"git revert failed: {(reverted.stdout + reverted.stderr)[:180]}",
            expected="reverting",
        )
    revert_sha = runtime.head()
    return _finish_recovery(
        store=store,
        runtime=runtime,
        reason=reason,
        revert_sha=revert_sha,
        restart_fn=restart_fn,
        recovery_fn=recovery_fn,
        now=now,
    )


def reconcile(
    *,
    store: PromotionStore,
    runtime: RuntimeCheckout,
    restart_fn: Callable[[], object],
    recovery_fn: Callable[[], object],
    now: float | None = None,
) -> dict | None:
    """Recover crashes between a Git action and its following durable transition."""
    state = store.load()
    if state is None or state["status"] in {"accepted", "halted", "observing"}:
        return state
    ts = time.time() if now is None else float(now)
    head = runtime.head()
    if not runtime.clean():
        return _halt(store, reason="runtime dirty during reconciliation", expected=state["status"])

    if state["status"] == "candidate":
        from .automerge import classify_branch
        classification = classify_branch(
            str(runtime.source),
            state["branch"],
            base=state["base_sha"],
        )
        identity_ok = (
            classification.get("zone") in {"green", "yellow"}
            and classification.get("diff_hash") == state["diff_hash"]
            and all(state.get("preflight_checks", {}).values())
            and state.get("baseline")
        )
        if not identity_ok:
            return _halt(store, reason="candidate identity changed during recovery",
                         expected="candidate")
        if head == state["base_sha"]:
            return _finish_merge(store, runtime, now=ts)
        parents = _git(runtime.path, "rev-list", "--parents", "-n", "1", head)
        ancestry = _git(runtime.path, "merge-base", "--is-ancestor", state["branch"], head)
        parent_shas = parents.stdout.strip().split()[1:] if parents.returncode == 0 else []
        if ancestry.returncode == 0 and state["base_sha"] in parent_shas and len(parent_shas) >= 2:
            return _finish_merge(store, runtime, now=ts, merge_sha=head)
        return _halt(store, reason="ambiguous candidate Git state", expected="candidate")

    if state["status"] == "merged":
        if head != state.get("merge_sha"):
            return _halt(store, reason="runtime moved after merge", expected="merged")
        return store.advance(
            "observing",
            expected="merged",
            now=ts,
            evidence={"reconciled_after_merge": True},
            fields={"observation_started_at": ts},
        )

    if state["status"] == "reverting":
        merge_sha = state.get("merge_sha")
        if head == merge_sha:
            return rollback(
                store=store,
                runtime=runtime,
                reason=state.get("revert_reason", "resume rollback"),
                restart_fn=restart_fn,
                recovery_fn=recovery_fn,
                now=ts,
            )
        tree = _git(runtime.path, "diff", "--quiet", state["base_sha"], head)
        ancestry = _git(runtime.path, "merge-base", "--is-ancestor", merge_sha or "", head)
        if tree.returncode == 0 and ancestry.returncode == 0:
            return _finish_recovery(
                store=store,
                runtime=runtime,
                reason=state.get("revert_reason", "reconciled rollback"),
                revert_sha=head,
                restart_fn=restart_fn,
                recovery_fn=recovery_fn,
                now=ts,
            )
        return _halt(store, reason="ambiguous revert Git state", expected="reverting")

    if state["status"] == "reverted":
        return _halt(store, reason="reconciled completed revert", expected="reverted")
    return _halt(store, reason=f"unhandled recovery state: {state['status']}",
                 expected=state["status"])


def observe(
    *,
    store: PromotionStore,
    runtime: RuntimeCheckout,
    sample: dict,
    now: float,
    restart_fn: Callable[[], object],
    recovery_fn: Callable[[], object],
    horizon_s: float = 26 * 3600,
    min_samples: int = 2,
) -> dict:
    """Persist one sample and accept, wait, or roll back according to policy."""
    state = store.load()
    if state is None or state["status"] != "observing":
        raise RuntimeError("observation requires an active observing promotion")
    samples = [*state.get("samples", []), sample]
    state = store.record(
        action="observation_sample",
        expected="observing",
        now=now,
        evidence={"sample_index": len(samples)},
        fields={"samples": samples},
    )
    verdict = evaluate_observation(
        state["baseline"],
        samples,
        started_at=float(state["observation_started_at"]),
        now=now,
        horizon_s=horizon_s,
        min_samples=min_samples,
    )
    if verdict.action == "wait":
        return state
    if verdict.action == "accept":
        return store.advance(
            "accepted",
            expected="observing",
            now=now,
            evidence={"samples": len(samples), "horizon_s": horizon_s},
        )
    return rollback(
        store=store,
        runtime=runtime,
        reason="; ".join(verdict.reasons),
        restart_fn=restart_fn,
        recovery_fn=recovery_fn,
        now=now,
    )
