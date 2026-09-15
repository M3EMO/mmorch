"""Persistent Git worktree used as the isolated autonomous runtime."""

from __future__ import annotations

import os
import subprocess
from dataclasses import dataclass
from pathlib import Path


class RuntimeCheckoutError(RuntimeError):
    """A runtime checkout invariant failed; autonomous promotion must stop."""


def _git(repo: Path, *args: str, timeout: float = 120.0) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["git", "-C", str(repo), *args],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=timeout,
    )


def _root(path: Path) -> Path:
    result = _git(path, "rev-parse", "--show-toplevel")
    if result.returncode != 0:
        raise RuntimeCheckoutError(f"not a git checkout: {path}: {result.stderr[:160]}")
    return Path(result.stdout.strip()).resolve()


@dataclass(frozen=True)
class RuntimeCheckout:
    source: Path
    path: Path
    branch: str

    def head(self) -> str:
        result = _git(self.path, "rev-parse", "HEAD")
        if result.returncode != 0:
            raise RuntimeCheckoutError(f"cannot read runtime HEAD: {result.stderr[:160]}")
        return result.stdout.strip()

    def clean(self) -> bool:
        result = _git(self.path, "status", "--porcelain")
        if result.returncode != 0:
            raise RuntimeCheckoutError(f"cannot inspect runtime: {result.stderr[:160]}")
        return not result.stdout.strip()

    def command(self, python: str | Path, *, state_home: str | Path) -> dict:
        """Return scheduler-ready argv/cwd/env without mutating Task Scheduler."""
        env = os.environ.copy()
        env["MMORCH_HOME"] = str(Path(state_home).resolve())
        return {
            "argv": [str(python), "-m", "mmorch.nightly"],
            "cwd": str(self.path),
            "env": env,
        }


def ensure_runtime(
    source: str | Path,
    runtime: str | Path,
    *,
    branch: str = "mmorch/runtime",
    base: str = "HEAD",
) -> RuntimeCheckout:
    """Create or validate a persistent worktree without changing the source checkout."""
    source_path = Path(source).resolve()
    runtime_path = Path(runtime).resolve()
    source_root = _root(source_path)
    if source_root != source_path:
        raise RuntimeCheckoutError(
            f"source must be the repository root, got {source_path} inside {source_root}"
        )
    if runtime_path == source_root:
        raise RuntimeCheckoutError("runtime must not be the active source checkout")
    try:
        runtime_path.relative_to(source_root)
    except ValueError:
        pass
    else:
        raise RuntimeCheckoutError("runtime must live outside the source checkout")

    if runtime_path.exists():
        if _root(runtime_path) != runtime_path:
            raise RuntimeCheckoutError(f"runtime path is not a worktree root: {runtime_path}")
        current = _git(runtime_path, "branch", "--show-current").stdout.strip()
        if current != branch:
            raise RuntimeCheckoutError(
                f"runtime branch mismatch: expected {branch}, found {current or 'detached'}"
            )
        checkout = RuntimeCheckout(source_root, runtime_path, branch)
        if not checkout.clean():
            raise RuntimeCheckoutError("runtime checkout is dirty")
        return checkout

    runtime_path.parent.mkdir(parents=True, exist_ok=True)
    exists = _git(source_root, "show-ref", "--verify", f"refs/heads/{branch}").returncode == 0
    args = ("worktree", "add", str(runtime_path), branch) if exists else (
        "worktree", "add", "-b", branch, str(runtime_path), base
    )
    added = _git(source_root, *args)
    if added.returncode != 0:
        raise RuntimeCheckoutError(f"worktree add failed: {(added.stdout + added.stderr)[:240]}")
    checkout = RuntimeCheckout(source_root, runtime_path, branch)
    if not checkout.clean():
        raise RuntimeCheckoutError("new runtime checkout is unexpectedly dirty")
    return checkout
