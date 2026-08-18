"""Tests del merge train — repo git real en tmp, test_fn inyectado."""

import subprocess

from mmorch.merge_train import run_train, yellow_branches


def _git(repo, *args):
    return subprocess.run(["git", *args], cwd=repo, capture_output=True, text=True)


def make_repo(tmp_path):
    repo = tmp_path / "r"
    (repo / "mmorch").mkdir(parents=True)
    (repo / "tests").mkdir()
    (repo / "logs").mkdir()
    _git(repo, "init", "-b", "main")
    _git(repo, "config", "user.email", "t@t")
    _git(repo, "config", "user.name", "t")
    (repo / "mmorch" / "a.py").write_text("a = 1\n", encoding="utf-8")
    (repo / "mmorch" / "b.py").write_text("b = 1\n", encoding="utf-8")
    _git(repo, "add", "-A")
    _git(repo, "commit", "-m", "base", "--no-verify")
    return str(repo)


def _mk_branch(repo, name, path, content):
    from pathlib import Path
    _git(repo, "checkout", "-b", name)
    Path(repo, path).write_text(content, encoding="utf-8")
    _git(repo, "add", "-A")
    _git(repo, "commit", "-m", name, "--no-verify")
    _git(repo, "checkout", "main")


def test_yellow_branches_filters(tmp_path):
    repo = make_repo(tmp_path)
    _mk_branch(repo, "mmorch/hard-1", "mmorch/a.py", "a = 2\n")       # amarilla
    _mk_branch(repo, "mmorch/fix-2", "tests/test_n.py", "x = 1\n")    # verde (nueva)
    _mk_branch(repo, "otra-cosa", "mmorch/b.py", "b = 2\n")           # patron ajeno
    ys = yellow_branches(repo, base="main")
    assert ys == ["mmorch/hard-1"]


def test_train_merges_and_gates(tmp_path):
    repo = make_repo(tmp_path)
    _mk_branch(repo, "mmorch/hard-1", "mmorch/a.py", "a = 2\n")
    _mk_branch(repo, "mmorch/fix-2", "mmorch/b.py", "b = 2\n")
    r = run_train(repo, base="main", today="2026-08-19", test_fn=lambda: True)
    assert sorted(r["merged"]) == ["mmorch/fix-2", "mmorch/hard-1"]
    assert r["gate"] == "verde"
    assert r["train_branch"] == "mmorch/tren-2026-08-19"
    # la branch del tren existe y contiene AMBOS cambios
    show_a = _git(repo, "show", "mmorch/tren-2026-08-19:mmorch/a.py").stdout
    show_b = _git(repo, "show", "mmorch/tren-2026-08-19:mmorch/b.py").stdout
    assert "a = 2" in show_a and "b = 2" in show_b


def test_train_skips_conflicting(tmp_path):
    repo = make_repo(tmp_path)
    _mk_branch(repo, "mmorch/hard-1", "mmorch/a.py", "a = 2\n")
    _mk_branch(repo, "mmorch/fix-2", "mmorch/a.py", "a = 3\n")  # conflictua con hard-1
    r = run_train(repo, base="main", today="2026-08-19", test_fn=lambda: True)
    assert len(r["merged"]) == 1 and len(r["skipped_conflict"]) == 1
    assert r["gate"] == "verde"


def test_train_red_gate_no_branch(tmp_path):
    repo = make_repo(tmp_path)
    _mk_branch(repo, "mmorch/hard-1", "mmorch/a.py", "a = 2\n")
    r = run_train(repo, base="main", today="2026-08-19", test_fn=lambda: False)
    assert r["gate"] == "rojo" and r["train_branch"] is None


def test_train_paused(tmp_path):
    repo = make_repo(tmp_path)
    from pathlib import Path
    (Path(repo) / "logs" / "loop_paused").touch()
    assert run_train(repo, base="main")["skipped"] == "paused"
