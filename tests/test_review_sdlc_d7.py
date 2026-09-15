"""Review del cableado de try_automerge (mmorch/auto_apply.py) contra spec.md."""

import subprocess

import mmorch.auto_apply as AA
from mmorch.promotion import PromotionStore
from mmorch.runtime_checkout import ensure_runtime


def _git(repo, *args):
    return subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True)


def test_finish_merge_hides_ledger_by_leaking_exclude_into_source_repo(tmp_path):
    """_finish_merge esconde logs/automerge_ledger.jsonl escribiendo 'logs/' en
    info/exclude del runtime (worktree). info/exclude es COMPARTIDO entre el
    checkout fuente y sus worktrees (un solo archivo en el git dir comun), asi
    que la exclusion se filtra al repo fuente real: cualquier carpeta logs/
    del checkout fuente (no solo el ledger del runtime) desaparece de
    `git status`, sin que la TAREA ni spec.md pidan tocar el repo fuente."""
    repo = tmp_path / "source"
    repo.mkdir()
    _git(repo, "init", "-q", "-b", "main")
    _git(repo, "config", "user.email", "t@t.com")
    _git(repo, "config", "user.name", "t")
    (repo / "value.py").write_text("VALUE = 1\n")
    _git(repo, "add", "value.py")
    _git(repo, "commit", "-q", "-m", "base", "--no-verify")
    base_sha = _git(repo, "rev-parse", "HEAD").stdout.strip()
    _git(repo, "checkout", "-q", "-b", "candidate")
    (repo / "new_test.py").write_text("# nuevo\n")  # archivo nuevo -> zona verde
    _git(repo, "add", "new_test.py")
    _git(repo, "commit", "-q", "-m", "candidate", "--no-verify")
    _git(repo, "checkout", "-q", "main")

    runtime = ensure_runtime(repo, tmp_path / "runtime")
    store = PromotionStore(tmp_path / "state")
    store.begin(
        branch="candidate", base_sha=base_sha, diff_hash="d", promotion_id="p",
        now=1.0, evidence={}, fields={"zone": "green"},
    )

    state = AA._finish_merge(store, runtime, now=2.0)
    assert state["status"] == "observing"

    # Un archivo real, sin relacion con el ledger, que el usuario del repo
    # fuente sí necesita ver en `git status`.
    (repo / "logs").mkdir(exist_ok=True)
    (repo / "logs" / "important.txt").write_text("do not hide me\n")
    status = _git(repo, "status", "--porcelain").stdout
    assert "logs/" in status or "logs\\" in status
