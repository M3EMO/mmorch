import subprocess

import pytest

from mmorch.runtime_checkout import RuntimeCheckoutError, ensure_runtime


def _git(repo, *args):
    return subprocess.run(
        ["git", "-C", str(repo), *args],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )


def _repo(tmp_path):
    repo = tmp_path / "source"
    repo.mkdir()
    _git(repo, "init", "-b", "main")
    _git(repo, "config", "user.email", "test@example.com")
    _git(repo, "config", "user.name", "Test")
    (repo / "app.py").write_text("value = 1\n", encoding="utf-8")
    _git(repo, "add", "app.py")
    assert _git(repo, "commit", "-m", "base", "--no-verify").returncode == 0
    return repo


def test_runtime_is_separate_and_restartable(tmp_path):
    repo = _repo(tmp_path)
    runtime_path = tmp_path / "runtime"
    source_head = _git(repo, "rev-parse", "HEAD").stdout.strip()

    first = ensure_runtime(repo, runtime_path)
    assert first.path == runtime_path.resolve()
    assert first.branch == "mmorch/runtime"
    assert first.head() == source_head
    assert first.clean()
    assert _git(repo, "branch", "--show-current").stdout.strip() == "main"
    assert (repo / "app.py").read_text(encoding="utf-8") == "value = 1\n"

    # Simula otro proceso: valida/reabre el mismo runtime sin crear otra branch.
    second = ensure_runtime(repo, runtime_path)
    assert second == first
    assert second.head() == source_head


def test_runtime_command_separates_code_cwd_from_state_home(tmp_path):
    repo = _repo(tmp_path)
    runtime = ensure_runtime(repo, tmp_path / "runtime")
    state_home = tmp_path / "state"
    cmd = runtime.command("python.exe", state_home=state_home)

    assert cmd["argv"] == ["python.exe", "-m", "mmorch.nightly"]
    assert cmd["cwd"] == str(runtime.path)
    assert cmd["env"]["MMORCH_HOME"] == str(state_home.resolve())


def test_runtime_rejects_dirty_checkout_on_restart(tmp_path):
    repo = _repo(tmp_path)
    runtime = ensure_runtime(repo, tmp_path / "runtime")
    (runtime.path / "app.py").write_text("dirty = True\n", encoding="utf-8")
    with pytest.raises(RuntimeCheckoutError, match="dirty"):
        ensure_runtime(repo, runtime.path)


def test_runtime_must_be_outside_source(tmp_path):
    repo = _repo(tmp_path)
    with pytest.raises(RuntimeCheckoutError, match="outside"):
        ensure_runtime(repo, repo / ".runtime")


def test_source_argument_must_be_exact_repo_root(tmp_path):
    repo = _repo(tmp_path)
    subdir = repo / "nested"
    subdir.mkdir()
    with pytest.raises(RuntimeCheckoutError, match="repository root"):
        ensure_runtime(subdir, tmp_path / "runtime")


def test_existing_wrong_branch_fails_closed(tmp_path):
    repo = _repo(tmp_path)
    runtime_path = tmp_path / "runtime"
    ensure_runtime(repo, runtime_path, branch="mmorch/other")
    with pytest.raises(RuntimeCheckoutError, match="branch mismatch"):
        ensure_runtime(repo, runtime_path, branch="mmorch/runtime")


def test_source_uncommitted_files_are_not_copied(tmp_path):
    repo = _repo(tmp_path)
    (repo / "uncommitted.txt").write_text("human work\n", encoding="utf-8")
    runtime = ensure_runtime(repo, tmp_path / "runtime")
    assert not (runtime.path / "uncommitted.txt").exists()
    assert (repo / "uncommitted.txt").read_text(encoding="utf-8") == "human work\n"
