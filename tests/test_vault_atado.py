"""Modulo A2 del vault: una nota atada a codigo se marca 'a revisar' tras un commit posterior."""
from __future__ import annotations

import functools
import os
import subprocess

import pytest

import mmorch.memory as M
import mmorch.vault as vault_mod
from mmorch import config
from mmorch.vault import regenerate_moc, write_research_note, write_validated


def _git(repo, *args, date=None):
    env = {**os.environ, "GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_SYSTEM": os.devnull}
    if date:
        env["GIT_AUTHOR_DATE"] = env["GIT_COMMITTER_DATE"] = f"{date}T12:00:00+0000"
    subprocess.run(["git", "-C", str(repo), *args], check=True, env=env, capture_output=True)


def _commit(repo, date, name="mmorch/x.py", text="x"):
    f = repo / name
    f.parent.mkdir(parents=True, exist_ok=True)
    f.write_text(text + date, encoding="utf-8")
    _git(repo, "add", name)
    _git(repo, "commit", "-m", f"c {date}", date=date)


@pytest.fixture()
def repo(tmp_path, monkeypatch):
    _git(tmp_path, "init", "-q")
    _git(tmp_path, "config", "user.name", "t")
    _git(tmp_path, "config", "user.email", "t@t")
    (tmp_path / "vault" / "research").mkdir(parents=True)
    monkeypatch.setattr(vault_mod, "VAULT", tmp_path / "vault")
    return tmp_path


def _nota(repo, extra="", created="2026-01-10"):
    p = repo / "vault" / "research" / "atada.md"
    p.write_text(f"---\ntitle: Atada\ntags: [p]\ncreated: {created}\n"
                 f"code: [mmorch/x.py]\n{extra}---\ncuerpo\n", encoding="utf-8")


def _linea(repo):
    moc = regenerate_moc("p").read_text(encoding="utf-8")
    return next(ln for ln in moc.splitlines() if "[[atada]]" in ln)


def test_commit_posterior_marca_en_moc(repo):
    _nota(repo)
    _commit(repo, "2026-02-01")
    assert _linea(repo).endswith(" · a revisar")


def test_reviewed_cierra_la_marca(repo):
    _nota(repo, extra="reviewed: 2026-03-01\n")
    _commit(repo, "2026-02-01")
    assert "a revisar" not in _linea(repo)
    _commit(repo, "2026-03-05", text="y")
    assert "a revisar" in _linea(repo)


def test_commit_mismo_dia_no_marca(repo):
    _nota(repo)
    _commit(repo, "2026-01-10")
    assert "a revisar" not in _linea(repo)


def test_atado_apagado_sin_marca(repo, monkeypatch):
    _nota(repo)
    _commit(repo, "2026-02-01")
    monkeypatch.setattr(config, "VAULT_MODULES", frozenset({"supersedes"}))

    def boom(*a, **kw):
        raise AssertionError("no debe llamar a git")
    monkeypatch.setattr(vault_mod.subprocess, "run", boom)
    assert "a revisar" not in _linea(repo)


def test_sin_repo_git_no_marca_ni_rompe(tmp_path, monkeypatch):
    monkeypatch.setattr(vault_mod, "VAULT", tmp_path / "vault")
    (tmp_path / "vault" / "research").mkdir(parents=True)
    _nota(tmp_path)
    assert "a revisar" not in _linea(tmp_path)


def test_atado_marca_en_recall_y_sigue_visible(repo, monkeypatch):
    db = repo / "m.duckdb"
    monkeypatch.setattr(M, "remember", functools.partial(M.remember, path=db))
    monkeypatch.setattr(M, "distill", lambda t, **kw: t.split(" — ")[0])
    monkeypatch.setattr(M, "embed", lambda t: None)
    write_research_note("Hallazgo atado", "x", project="p", code="mmorch/x.py")
    _commit(repo, "2099-01-01")
    for fn in (M.recall, M.recall_keyword, M.recall_hybrid):
        out = [n for n in fn("hallazgo atado", "global", k=50, path=db)
               if n.layer == "semantic" and "Hallazgo atado" in n.text]
        assert out, fn.__name__
        assert all(n.text.startswith("[a revisar] ") for n in out), fn.__name__


@pytest.mark.parametrize("code", ["/etc/passwd", "../x.py", "a/../b.py", "-x", ":(top)x", ""])
def test_code_invalido_no_escribe(repo, code):
    calls = []
    with pytest.raises(ValueError):
        write_validated("Nueva", "b", project="p", code=[code], remember_fn=calls.append)
    assert not list((repo / "vault" / "research").glob("*.md"))
    assert not (repo / "vault" / "moc").exists() and not calls


def test_code_con_caracter_no_imprimible_no_escribe(repo):
    with pytest.raises(ValueError):
        write_validated("Nueva", "b", project="p", code=["a.py\nstatus: applied"])
    assert not list((repo / "vault" / "research").glob("*.md"))
