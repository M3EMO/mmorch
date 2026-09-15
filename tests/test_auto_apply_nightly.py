from mmorch.auto_apply_nightly import _candidate_from_record, run_nightly


def test_rollout_defaults_off_without_calling_cycle(tmp_path, monkeypatch):
    monkeypatch.delenv("MMORCH_AUTO_APPLY_ROLLOUT", raising=False)
    called = []
    result = run_nightly({}, root=tmp_path, cycle_fn=lambda *a, **k: called.append(True))
    assert result == {"skipped": "rollout off"}
    assert called == []


def test_non_off_rollout_requires_explicit_runtime_dir(tmp_path, monkeypatch):
    monkeypatch.setenv("MMORCH_AUTO_APPLY_ROLLOUT", "shadow")
    monkeypatch.delenv("MMORCH_RUNTIME_DIR", raising=False)
    result = run_nightly({}, root=tmp_path)
    assert result == {"error": "MMORCH_RUNTIME_DIR required"}


def test_rollout_and_runtime_are_forwarded_to_cycle(tmp_path, monkeypatch):
    runtime = tmp_path / "runtime"
    monkeypatch.setenv("MMORCH_AUTO_APPLY_ROLLOUT", "yellow_bounded")
    monkeypatch.setenv("MMORCH_RUNTIME_DIR", str(runtime))
    seen = {}

    def cycle(rec, **kwargs):
        seen.update(rec=rec, **kwargs)
        return {"status": "shadowed-by-test"}

    result = run_nightly({"ts": 1}, root=tmp_path, cycle_fn=cycle)
    assert result == {"status": "shadowed-by-test"}
    assert seen["rollout"] == "yellow_bounded"
    assert seen["runtime_dir"] == str(runtime)
    assert seen["root"] == tmp_path


def test_cycle_errors_are_visible_and_fail_closed(tmp_path, monkeypatch):
    monkeypatch.setenv("MMORCH_AUTO_APPLY_ROLLOUT", "green")
    monkeypatch.setenv("MMORCH_RUNTIME_DIR", str(tmp_path / "runtime"))

    def boom(*args, **kwargs):
        raise RuntimeError("no budget")

    result = run_nightly({}, root=tmp_path, cycle_fn=boom)
    assert result["rollout"] == "green"
    assert result["error"] == "RuntimeError: no budget"


def test_candidate_priority_is_repair_then_hardening_then_train():
    assert _candidate_from_record({
        "auto_repair": {"branch": "fix"},
        "hardening": {"branch": "hard"},
        "merge_train": {"train_branch": "train"},
    }) == "fix"
    assert _candidate_from_record({
        "auto_repair": {"skipped": "none"},
        "hardening": {"branch": "hard"},
        "merge_train": {"train_branch": "train"},
    }) == "hard"
    assert _candidate_from_record({
        "merge_train": {"train_branch": "train"},
    }) == "train"
    assert _candidate_from_record({}) is None
