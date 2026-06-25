"""worktree_driver — isolated execution in a throwaway git worktree (graft G3 follow-up).

G3's `sandbox` policy DENIED local execution (a lockdown switch). This makes `sandbox`
ISOLATE instead: run a project edit inside a fresh `git worktree` of the repo — a separate
working tree on its own branch that shares the repo's object DB (no full copy). The main
working tree is never touched. After the run we commit the result to the branch, record the
diff, remove the worktree dir, and KEEP the branch so a human can review/merge.

ponytail: git's own worktree mechanism + stdlib subprocess. No repo copy, no temp VCS. The
branch ref (cheap) outlives the worktree dir so the result stays reviewable.
"""
from __future__ import annotations

import os
import subprocess
import tempfile
import uuid


def _git(repo: str, *args: str, timeout: float = 120.0):
    try:
        p = subprocess.run(["git", "-C", repo, *args], capture_output=True, text=True,
                           encoding="utf-8", errors="replace", timeout=timeout)
        return p.returncode, (p.stdout + p.stderr).strip()
    except Exception as e:
        return 1, str(e)[:200]


def is_git_repo(repo: str) -> bool:
    return _git(repo, "rev-parse", "--git-dir")[0] == 0


def _has_head(repo: str) -> bool:
    return _git(repo, "rev-parse", "HEAD")[0] == 0


class Worktree:
    """A throwaway worktree on its own branch. Create via open_worktree()."""

    def __init__(self, repo: str, path: str, branch: str):
        self.repo, self.path, self.branch = repo, path, branch
        self.diff = ""
        self.diffstat = ""

    def capture(self, message: str = "mmorch(worktree): isolated run") -> dict:
        """Stage everything, record the diff vs the base, commit it to this worktree's branch."""
        _git(self.path, "add", "-A")
        self.diffstat = _git(self.path, "diff", "--cached", "--stat")[1]
        self.diff = _git(self.path, "diff", "--cached")[1]
        changed = bool(self.diff.strip())
        if changed:
            _git(self.path, "commit", "-m", message)
        return {"branch": self.branch, "diffstat": self.diffstat, "changed": changed}

    def close(self, *, keep_branch: bool = True) -> None:
        """Remove the worktree dir; the branch ref persists unless keep_branch=False."""
        _git(self.repo, "worktree", "remove", "--force", self.path)
        if not keep_branch:
            _git(self.repo, "branch", "-D", self.branch)
        _git(self.repo, "worktree", "prune")


def open_worktree(repo: str, *, prefix: str = "mmorch/wt", base: str = "HEAD") -> Worktree:
    """Add a fresh worktree of `repo` at a temp path on a new unique branch off `base`."""
    if not is_git_repo(repo):
        raise RuntimeError(f"not a git repo: {repo}")
    if not _has_head(repo):
        raise RuntimeError(f"repo has no commits (no HEAD): {repo}")
    tag = uuid.uuid4().hex[:8]
    branch = f"{prefix}-{tag}"
    path = os.path.join(tempfile.gettempdir(), f"mmorch-wt-{tag}")
    rc, out = _git(repo, "worktree", "add", "-b", branch, path, base)
    if rc != 0:
        raise RuntimeError(f"worktree add failed: {out}")
    return Worktree(repo, path, branch)


if __name__ == "__main__":
    # Real git repo in temp -> prove isolation: work in the worktree must NOT touch the main tree.
    d = tempfile.mkdtemp()
    for a in (("init", "-q"), ("config", "user.email", "t@t"), ("config", "user.name", "t"),
              ("config", "commit.gpgsign", "false")):
        _git(d, *a)
    with open(os.path.join(d, "a.txt"), "w") as f:
        f.write("base\n")
    _git(d, "add", "-A"); _git(d, "commit", "-q", "-m", "init")

    wt = open_worktree(d)
    assert os.path.isdir(wt.path) and os.path.abspath(wt.path) != os.path.abspath(d)
    with open(os.path.join(wt.path, "b.txt"), "w") as f:    # new file, worktree only
        f.write("hi\n")
    with open(os.path.join(wt.path, "a.txt"), "w") as f:    # modify existing, worktree only
        f.write("base\nedit\n")
    cap = wt.capture("test run")
    assert cap["changed"] and "b.txt" in cap["diffstat"], cap
    assert not os.path.exists(os.path.join(d, "b.txt")), "ISOLATION: main tree must not see b.txt"
    assert open(os.path.join(d, "a.txt")).read() == "base\n", "ISOLATION: main a.txt unchanged"
    branch = wt.branch
    wt.close()
    assert not os.path.exists(wt.path), "worktree dir removed"
    assert _git(d, "rev-parse", "--verify", branch)[0] == 0, "review branch kept"
    assert _git(d, "status", "--porcelain")[1] == "", "main tree clean"
    # the kept branch actually contains the change
    assert "b.txt" in _git(d, "show", "--stat", branch)[1], "branch holds the work"
    print("worktree_driver OK")
