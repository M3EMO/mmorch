import subprocess

import pytest

from mmorch.auto_apply import merge_candidate, observe, preflight, reconcile
from mmorch.promotion import PromotionStore
from mmorch.runtime_checkout import ensure_runtime


class FakeRuntime:
    def __init__(self, *, clean=True, head="base"):
        self._clean = clean
        self._head = head

    def clean(self):
        return self._clean

    def head(self):
        return self._head


def _ok(**overrides):
    args = {
        "repo": ".",
        "runtime": FakeRuntime(),
        "branch": "candidate",
        "expected_base_sha": "base",
        "expected_diff_hash": "diff",
        "rollout": "green",
        "quality_fn": lambda: {"ok": True, "suite": "green"},
        "fitness_fn": lambda: {"ok": True, "ensemble": "cross-family"},
        "alignment_fn": lambda: type("V", (), {"passed": True})(),
        "classify_fn": lambda *a, **k: {"zone": "green", "diff_hash": "diff", "files": []},
        "budget_fn": lambda: {"enforced": True, "remaining": 1.0},
        "goal_fn": lambda: True,
    }
    args.update(overrides)
    return preflight(**args)


def test_green_preflight_allows_apply_only_with_complete_evidence():
    verdict = _ok()
    assert verdict.eligible and verdict.apply_allowed
    assert verdict.zone == "green"
    assert all(verdict.checks.values())


def test_shadow_evaluates_but_never_applies():
    verdict = _ok(rollout="shadow")
    assert verdict.eligible
    assert not verdict.apply_allowed


@pytest.mark.parametrize("rollout", ["green", "yellow_bounded"])
def test_red_zone_never_applies(rollout):
    verdict = _ok(
        rollout=rollout,
        classify_fn=lambda *a, **k: {
            "zone": "red", "diff_hash": "diff", "reason": "path rojo: GOAL.md",
        },
    )
    assert not verdict.eligible
    assert not verdict.apply_allowed
    assert verdict.checks["zone_allowed"] is False


def test_yellow_requires_yellow_rollout():
    classify = lambda *a, **k: {"zone": "yellow", "diff_hash": "diff", "files": []}
    assert not _ok(classify_fn=classify, rollout="green").eligible
    assert _ok(
        classify_fn=classify,
        rollout="yellow_bounded",
        bounded_fn=lambda *a, **k: {"ok": True},
    ).apply_allowed
    assert _ok(classify_fn=classify, rollout="non_red").apply_allowed


def test_bounded_yellow_rejects_excess_blast_radius():
    verdict = _ok(
        rollout="yellow_bounded",
        classify_fn=lambda *a, **k: {"zone": "yellow", "diff_hash": "diff"},
        bounded_fn=lambda *a, **k: {"ok": False, "changed_lines": 201},
    )
    assert not verdict.eligible
    assert verdict.checks["yellow_bounded"] is False


@pytest.mark.parametrize(
    ("override", "failed_check"),
    [
        ({"runtime": FakeRuntime(clean=False)}, "runtime_clean"),
        ({"runtime": FakeRuntime(head="moved")}, "base_unchanged"),
        ({"expected_diff_hash": "changed"}, "diff_unchanged"),
        ({"budget_fn": lambda: {"enforced": False, "remaining": None}}, "budget"),
        ({"quality_fn": None}, "quality"),
        ({"fitness_fn": None}, "fitness"),
        ({"alignment_fn": None}, "alignment"),
    ],
)
def test_missing_or_stale_evidence_fails_closed(override, failed_check):
    verdict = _ok(**override)
    assert not verdict.eligible
    assert not verdict.apply_allowed
    assert verdict.checks[failed_check] is False


@pytest.mark.parametrize(
    ("override", "failed_check"),
    [
        ({"goal_fn": lambda: (_ for _ in ()).throw(RuntimeError("GOAL tampered"))}, "goal_guard"),
        ({"fitness_fn": lambda: (_ for _ in ()).throw(TimeoutError("provider down"))}, "fitness"),
        ({"alignment_fn": lambda: "ambiguous"}, "alignment"),
        ({"quality_fn": lambda: {"status": "looks good"}}, "quality"),
        ({"classify_fn": lambda *a, **k: (_ for _ in ()).throw(RuntimeError("git failed"))},
         "classification"),
    ],
)
def test_errors_and_ambiguous_results_fail_closed(override, failed_check):
    verdict = _ok(**override)
    assert not verdict.eligible
    assert not verdict.apply_allowed
    assert verdict.checks[failed_check] is False


def test_unknown_and_off_rollouts_never_evaluate_as_applyable():
    assert not _ok(rollout="off").eligible
    unknown = _ok(rollout="typo")
    assert unknown.reason == "unknown rollout"
    assert not unknown.apply_allowed


def _git(repo, *args):
    return subprocess.run(
        ["git", "-C", str(repo), *args],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )


def _real_loop(tmp_path):
    repo = tmp_path / "source"
    repo.mkdir()
    _git(repo, "init", "-b", "main")
    _git(repo, "config", "user.email", "test@example.com")
    _git(repo, "config", "user.name", "Test")
    (repo / "value.py").write_text("VALUE = 1\n", encoding="utf-8")
    _git(repo, "add", "value.py")
    assert _git(repo, "commit", "-m", "base", "--no-verify").returncode == 0
    base_sha = _git(repo, "rev-parse", "HEAD").stdout.strip()
    _git(repo, "checkout", "-b", "candidate")
    (repo / "value.py").write_text("VALUE = 2\n", encoding="utf-8")
    _git(repo, "add", "value.py")
    assert _git(repo, "commit", "-m", "candidate", "--no-verify").returncode == 0
    _git(repo, "checkout", "main")

    runtime = ensure_runtime(repo, tmp_path / "runtime")
    from mmorch.automerge import classify_branch
    classification = classify_branch(str(repo), "candidate", base=base_sha)
    verdict = _ok(
        repo=repo,
        runtime=runtime,
        branch="candidate",
        expected_base_sha=base_sha,
        expected_diff_hash=classification["diff_hash"],
        rollout="non_red",
        classify_fn=classify_branch,
    )
    baseline = {
        "tests": True,
        "gates": True,
        "smoke": True,
        "health": True,
        "canary": {"model": 1.0},
        "error_rate": 0.0,
        "cost_per_call": 0.001,
    }
    return repo, runtime, base_sha, classification, verdict, baseline


def test_e2e_merge_regression_revert_and_halt_across_restarts(tmp_path):
    repo, runtime, base_sha, classification, verdict, baseline = _real_loop(tmp_path)
    state_root = tmp_path / "state"

    first_process = PromotionStore(state_root)
    state = merge_candidate(
        store=first_process,
        runtime=runtime,
        branch="candidate",
        expected_base_sha=base_sha,
        expected_diff_hash=classification["diff_hash"],
        verdict=verdict,
        baseline=baseline,
        now=100,
        promotion_id="e2e-revert",
    )
    assert state["status"] == "observing"
    assert (runtime.path / "value.py").read_text(encoding="utf-8") == "VALUE = 2\n"
    assert (repo / "value.py").read_text(encoding="utf-8") == "VALUE = 1\n"

    restarts = []
    second_process = PromotionStore(state_root)
    final = observe(
        store=second_process,
        runtime=runtime,
        sample={**baseline, "tests": False},
        now=101,
        restart_fn=lambda: restarts.append("restart"),
        recovery_fn=lambda: {"ok": True},
    )

    assert final["status"] == "halted"
    assert final["recovery_verified"] is True
    assert restarts == ["restart"]
    assert (runtime.path / "value.py").read_text(encoding="utf-8") == "VALUE = 1\n"
    assert (repo / "value.py").read_text(encoding="utf-8") == "VALUE = 1\n"
    assert (state_root / "loop_paused").exists()
    reloaded = PromotionStore(state_root).load()
    statuses = [event["status"] for event in reloaded["history"]]
    assert statuses == [
        "candidate", "merged", "observing", "observing",
        "reverting", "reverted", "halted",
    ]
    assert all(event["kind"] == "auto_action" for event in reloaded["history"])


def test_e2e_healthy_horizon_accepts_without_revert(tmp_path):
    _, runtime, base_sha, classification, verdict, baseline = _real_loop(tmp_path)
    state_root = tmp_path / "state"
    merge_candidate(
        store=PromotionStore(state_root),
        runtime=runtime,
        branch="candidate",
        expected_base_sha=base_sha,
        expected_diff_hash=classification["diff_hash"],
        verdict=verdict,
        baseline=baseline,
        now=100,
        promotion_id="e2e-accept",
    )
    first = observe(
        store=PromotionStore(state_root),
        runtime=runtime,
        sample=baseline,
        now=101,
        restart_fn=lambda: None,
        recovery_fn=lambda: {"ok": True},
        horizon_s=10,
    )
    assert first["status"] == "observing"
    accepted = observe(
        store=PromotionStore(state_root),
        runtime=runtime,
        sample=baseline,
        now=111,
        restart_fn=lambda: None,
        recovery_fn=lambda: {"ok": True},
        horizon_s=10,
    )
    assert accepted["status"] == "accepted"
    assert not (state_root / "loop_paused").exists()
    assert (runtime.path / "value.py").read_text(encoding="utf-8") == "VALUE = 2\n"


def _persist_approved_candidate(store, base_sha, classification, verdict, baseline):
    return store.begin(
        branch="candidate",
        base_sha=base_sha,
        diff_hash=classification["diff_hash"],
        promotion_id="crash-window",
        now=100,
        evidence={"preflight": verdict.checks, "zone": verdict.zone},
        fields={
            "baseline": baseline,
            "samples": [],
            "preflight_checks": verdict.checks,
            "zone": verdict.zone,
        },
    )


def test_reconcile_crash_before_git_merge_resumes_safely(tmp_path):
    _, runtime, base_sha, classification, verdict, baseline = _real_loop(tmp_path)
    store = PromotionStore(tmp_path / "state")
    _persist_approved_candidate(store, base_sha, classification, verdict, baseline)

    resumed = reconcile(
        store=PromotionStore(tmp_path / "state"),
        runtime=runtime,
        restart_fn=lambda: None,
        recovery_fn=lambda: {"ok": True},
        now=101,
    )
    assert resumed["status"] == "observing"
    assert (runtime.path / "value.py").read_text(encoding="utf-8") == "VALUE = 2\n"


def test_reconcile_crash_after_git_merge_before_state_write(tmp_path):
    _, runtime, base_sha, classification, verdict, baseline = _real_loop(tmp_path)
    store = PromotionStore(tmp_path / "state")
    _persist_approved_candidate(store, base_sha, classification, verdict, baseline)
    assert _git(runtime.path, "merge", "--no-edit", "--no-ff", "candidate").returncode == 0

    resumed = reconcile(
        store=PromotionStore(tmp_path / "state"),
        runtime=runtime,
        restart_fn=lambda: None,
        recovery_fn=lambda: {"ok": True},
        now=101,
    )
    assert resumed["status"] == "observing"
    assert resumed["merge_sha"] == runtime.head()


def test_reconcile_crash_after_git_revert_before_state_write(tmp_path):
    _, runtime, base_sha, classification, verdict, baseline = _real_loop(tmp_path)
    state_root = tmp_path / "state"
    observing = merge_candidate(
        store=PromotionStore(state_root),
        runtime=runtime,
        branch="candidate",
        expected_base_sha=base_sha,
        expected_diff_hash=classification["diff_hash"],
        verdict=verdict,
        baseline=baseline,
        now=100,
        promotion_id="crash-revert",
    )
    store = PromotionStore(state_root)
    store.advance(
        "reverting",
        expected="observing",
        fields={"revert_reason": "simulated regression"},
    )
    assert _git(
        runtime.path, "revert", "-m", "1", "--no-edit", observing["merge_sha"],
    ).returncode == 0
    restarted = []

    resumed = reconcile(
        store=PromotionStore(state_root),
        runtime=runtime,
        restart_fn=lambda: restarted.append(True),
        recovery_fn=lambda: {"ok": True},
        now=102,
    )
    assert resumed["status"] == "halted"
    assert resumed["recovery_verified"] is True
    assert restarted == [True]
    assert (runtime.path / "value.py").read_text(encoding="utf-8") == "VALUE = 1\n"
