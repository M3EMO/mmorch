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
import sys
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
        self._links: list[str] = []   # seeded dir-links; close() removes them BEFORE git deletes the tree

    def seed(self, patterns: list[str] | None) -> int:
        """Mirror GITIGNORED artifacts into the worktree (F4 lesson: a fresh checkout lacks the
        untracked data an acceptance suite reads — caches, local DBs — so the gate measures a broken
        env). Dirs matching a pattern are LINKED (junction/symlink, no copy — caches can be GBs);
        files are copied (isolates writes, e.g. a sqlite db). Links are recorded and removed by
        close() first — a recursive worktree delete must never traverse into the main tree's data."""
        import glob as _glob
        import shutil
        n = 0
        for pat in patterns or []:
            for src in _glob.glob(os.path.join(self.repo, pat)):
                src = os.path.normpath(src)   # glob yields mixed seps; cmd's mklink rejects fwd-slashes
                dst = os.path.join(self.path, os.path.relpath(src, self.repo))
                if os.path.exists(dst):
                    continue
                os.makedirs(os.path.dirname(dst), exist_ok=True)
                if os.path.isdir(src):
                    try:
                        os.symlink(src, dst, target_is_directory=True)
                    except OSError:               # Windows without symlink privilege -> junction
                        subprocess.run(["cmd", "/c", "mklink", "/J", dst, src], capture_output=True)
                    if os.path.isdir(dst):
                        self._links.append(dst)
                        n += 1
                else:
                    shutil.copy2(src, dst)
                    n += 1
        return n

    def capture(self, message: str = "mmorch(worktree): isolated run") -> dict:
        """Stage everything, record the diff vs the base, commit it to this worktree's branch.
        Returns committed=False (with the git error in `error`) si el commit falla -- bug medido
        2026-07: un pre-commit hook (ruff gate) fallaba en el worktree (.venv relativo no existe
        ahí -> cae a python de sistema sin ruff) y esto devolvía 'changed: True' igual, mintiendo
        que el trabajo quedó guardado cuando en realidad el HEAD del worktree no se movió."""
        _git(self.path, "add", "-A")
        self.diffstat = _git(self.path, "diff", "--cached", "--stat")[1]
        self.diff = _git(self.path, "diff", "--cached")[1]
        changed = bool(self.diff.strip())
        committed, error = False, ""
        if changed:
            rc, out = _git(self.path, "commit", "-m", message)
            if rc != 0 and "[*]" in out:
                # bug medido 2026-08: el pre-commit ruff gate del repo frena código generado por
                # lint AUTO-FIXABLE (ej F401 import sobrante) -> unidad jamás llega a la branch ->
                # escalate. "[*]" en el output de ruff marca fixable: fix + re-stage + UN retry.
                subprocess.run([sys.executable, "-m", "ruff", "check", "--fix", "."],
                               cwd=self.path, capture_output=True, timeout=120)
                _git(self.path, "add", "-A")
                rc, out = _git(self.path, "commit", "-m", message)
            committed, error = rc == 0, ("" if rc == 0 else out)
        return {"branch": self.branch, "diffstat": self.diffstat, "changed": changed,
                "committed": committed, "error": error}

    def close(self, *, keep_branch: bool = True) -> None:
        """Remove the worktree dir; the branch ref persists unless keep_branch=False."""
        for link in self._links:      # unlink seeded dirs FIRST: never recursive-delete into main-tree data
            try:
                os.rmdir(link)        # on a junction/dir-symlink this removes only the link
            except OSError:
                pass
        self._links = []
        _git(self.repo, "worktree", "remove", "--force", self.path)
        if not keep_branch:
            _git(self.repo, "branch", "-D", self.branch)
        _git(self.repo, "worktree", "prune")


def open_worktree(repo: str, *, prefix: str = "mmorch/wt", base: str = "HEAD",
                  branch: str | None = None) -> Worktree:
    """Add a worktree of `repo` at a fresh temp path. `branch=None` -> a new unique branch off `base`;
    `branch=<name>` -> check out that EXISTING branch (resume continuity — a branch lives in at most one
    worktree, so the prior one must be closed first)."""
    if not is_git_repo(repo):
        raise RuntimeError(f"not a git repo: {repo}")
    if not _has_head(repo):
        raise RuntimeError(f"repo has no commits (no HEAD): {repo}")
    tag = uuid.uuid4().hex[:8]
    path = os.path.join(tempfile.gettempdir(), f"mmorch-wt-{tag}")
    if branch:
        rc, out = _git(repo, "worktree", "add", path, branch)        # reuse existing branch
    else:
        branch = f"{prefix}-{tag}"
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
    assert cap["changed"] and cap["committed"] and "b.txt" in cap["diffstat"], cap
    assert not os.path.exists(os.path.join(d, "b.txt")), "ISOLATION: main tree must not see b.txt"
    assert open(os.path.join(d, "a.txt")).read() == "base\n", "ISOLATION: main a.txt unchanged"
    branch = wt.branch
    wt.close()
    assert not os.path.exists(wt.path), "worktree dir removed"
    assert _git(d, "rev-parse", "--verify", branch)[0] == 0, "review branch kept"
    assert _git(d, "status", "--porcelain")[1] == "", "main tree clean"
    # the kept branch actually contains the change
    assert "b.txt" in _git(d, "show", "--stat", branch)[1], "branch holds the work"

    # SEED (F4): gitignored dir linked + file copied into the worktree; the SOURCE must survive close().
    os.makedirs(os.path.join(d, "data", ".cache"))
    with open(os.path.join(d, "data", ".cache", "big.bin"), "w") as f:
        f.write("cache-artifact\n")
    with open(os.path.join(d, "local.db"), "w") as f:
        f.write("db\n")
    with open(os.path.join(d, ".gitignore"), "w") as f:
        f.write("data/.cache/\nlocal.db\n")
    _git(d, "add", "-A"); _git(d, "commit", "-q", "-m", "gitignore")
    wt3 = open_worktree(d)
    assert not os.path.exists(os.path.join(wt3.path, "data", ".cache")), "fresh worktree lacks the cache"
    ns = wt3.seed(["data/.cache", "local.db"])
    assert ns == 2, ns
    assert open(os.path.join(wt3.path, "data", ".cache", "big.bin")).read() == "cache-artifact\n", "linked"
    assert os.path.isfile(os.path.join(wt3.path, "local.db")), "file copied"
    wt3.close(keep_branch=False)
    assert os.path.isfile(os.path.join(d, "data", ".cache", "big.bin")), \
        "CRITICAL: source cache must SURVIVE close (link removed, never recursed into)"
    assert os.path.isfile(os.path.join(d, "local.db")), "source db survives"

    # branch REUSE (resume continuity): reopen the kept branch, add more, it accumulates
    wt2 = open_worktree(d, branch=branch)
    assert wt2.branch == branch and os.path.exists(os.path.join(wt2.path, "b.txt")), "reopened branch has prior work"
    with open(os.path.join(wt2.path, "c.txt"), "w") as f:
        f.write("more\n")
    cap2 = wt2.capture("more")
    assert cap2["changed"] and cap2["committed"]
    wt2.close()
    assert "c.txt" in _git(d, "show", "--stat", branch)[1], "reused branch accumulated new work"
    assert not os.path.exists(os.path.join(d, "c.txt")), "main tree still untouched"

    # FAILED COMMIT surfaces, no lo traga (bug medido 2026-07: un hook que aborta el commit
    # -> capture() decía changed=True igual, mintiendo que el trabajo quedó guardado).
    os.makedirs(os.path.join(d, ".git", "hooks"), exist_ok=True)
    with open(os.path.join(d, ".git", "hooks", "pre-commit"), "w") as f:
        f.write("#!/bin/sh\necho 'blocked by hook' >&2\nexit 1\n")
    os.chmod(os.path.join(d, ".git", "hooks", "pre-commit"), 0o755)
    wt4 = open_worktree(d)
    with open(os.path.join(wt4.path, "z.txt"), "w") as f:
        f.write("blocked\n")
    cap4 = wt4.capture("should fail")
    assert cap4["changed"] and not cap4["committed"] and cap4["error"], cap4
    wt4.close(keep_branch=False)
    print("worktree_driver OK")
