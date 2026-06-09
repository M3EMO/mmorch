"""tests sandbox_branch (git worktree) — aislamiento real. Repo git temporal, cmd trivial."""
import subprocess
import sys
import pytest

from mmorch.evolve import Change, sandbox_branch, promote_branch


def _git(tmp, *args):
    return subprocess.run(["git", *args], cwd=str(tmp), capture_output=True, text=True)


def _init_repo(tmp):
    _git(tmp, "init", "-b", "main")
    _git(tmp, "config", "user.email", "t@t.t")
    _git(tmp, "config", "user.name", "t")
    (tmp / "seed.txt").write_text("seed\n", encoding="utf-8")
    _git(tmp, "add", "-A")
    _git(tmp, "commit", "-m", "init", "--no-verify")


def _has_git():
    return subprocess.run(["git", "--version"], capture_output=True).returncode == 0


pytestmark = pytest.mark.skipif(not _has_git(), reason="git no disponible")


def test_sandbox_branch_pass_keeps_branch(tmp_path):
    _init_repo(tmp_path)
    ch = Change(target="cap.py", after="x = 1\n", before="", description="nueva cap")
    res = sandbox_branch(ch, root=tmp_path, test_cmd=[sys.executable, "-c", "import sys; sys.exit(0)"])
    assert res["ok"] and res["branch"] == f"mmorch-sbx-{ch.id}"
    # el repo vivo NO cambió (cap.py no existe en main)
    assert not (tmp_path / "cap.py").exists()
    # promover: merge a main -> ahora sí existe
    pr = promote_branch(res["branch"], root=tmp_path)
    assert pr["merged"] and (tmp_path / "cap.py").exists()


def test_sandbox_branch_fail_deletes_branch(tmp_path):
    _init_repo(tmp_path)
    ch = Change(target="cap.py", after="x = 1\n", before="", description="cambio que falla tests")
    res = sandbox_branch(ch, root=tmp_path, test_cmd=[sys.executable, "-c", "import sys; sys.exit(1)"])
    assert not res["ok"] and res["branch"] is None
    # branch borrada
    branches = subprocess.run(["git", "branch"], cwd=str(tmp_path), capture_output=True, text=True).stdout
    assert f"mmorch-sbx-{ch.id}" not in branches
    assert not (tmp_path / "cap.py").exists()
