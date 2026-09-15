"""Ticket 12 (adopcion): `sdlc init` y la etapa 1 "aceptacion". Cero API: Claude parcheado con un escritor local.

- init escribe sdlc.toml (techo sin tests), docs/sdlc/ y el puntero en AGENTS.md; es idempotente.
- etapa 1: el test que deja Claude debe nombrar R<n>, compilar y FALLAR en HEAD; con approve_accept la corrida
  se detiene en awaiting_approval; sin approve_accept sigue.
"""
import subprocess
import types

import pytest

import mmorch.sdlc as S

RED = "def test_R1_suma():\n    from pkg.a import suma\n    assert suma(1, 2) == 3\n"


def _repo(tmp_path):
    (tmp_path / "pkg").mkdir()
    (tmp_path / "pkg" / "a.py").write_text("x = 1\n", encoding="utf-8")
    (tmp_path / "tests").mkdir()
    (tmp_path / "tests" / "test_viejo.py").write_text("def test_x():\n    assert True\n", encoding="utf-8")
    (tmp_path / "AGENTS.md").write_text("# Agentes\n", encoding="utf-8")
    subprocess.run(["git", "init", "-q"], cwd=tmp_path, check=True)
    subprocess.run(["git", "add", "-A"], cwd=tmp_path, check=True)
    subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "base"], cwd=tmp_path, check=True)
    return tmp_path


def test_init_escribe_toml_docs_y_agents_sin_pisar(tmp_path):
    repo = _repo(tmp_path)
    r = S.init(str(repo))
    assert set(r["hecho"]) >= {"sdlc.toml", "AGENTS.md", "docs/sdlc/spec-template.md"}
    toml = S._toml(repo)
    assert toml["files"] == ["pkg/**/*.py"] and toml["ext"] == ["py"] and toml["approve_accept"] is True and toml["accept_cmd"]
    assert "## SDLC" in (repo / "AGENTS.md").read_text(encoding="utf-8")
    assert S.init(str(repo))["hecho"] == []  # idempotente


def _configura(repo, monkeypatch, escribe, approve=True):
    (repo / "sdlc.toml").write_text(f'accept_cmd = "python -m pytest -q"\napprove_accept = {str(approve).lower()}\nfiles = ["pkg/a.py"]\n', encoding="utf-8")
    t = types.SimpleNamespace(name="feat x", task="pkg/a.py: suma(a, b) devuelve a + b", accept_files={})
    S.configure(t, contract=[], feat={"repo": str(repo), "files": ["pkg/a.py"], "suite": ["tests"]}, wt=repo, accept_cmd="python -m pytest -q")

    def fake_claude(prompt, cwd, **kw):
        rel = prompt.split("Escribi SOLO el archivo ")[1].split(":")[0]
        (repo / rel).write_text(escribe, encoding="utf-8")
        return {"returncode": 0, "result": "ok"}
    import mmorch.claude_exec as CE
    monkeypatch.setattr(CE, "run_claude", fake_claude)
    return t


def test_etapa1_test_rojo_queda_esperando_aprobacion(tmp_path, monkeypatch):
    repo = _repo(tmp_path)
    t = _configura(repo, monkeypatch, RED)
    with pytest.raises(S.StageFailed, match="aprobacion"):
        S.aceptacion()
    rel = state_rel = S.state["awaiting_approval"]
    assert rel.startswith("tests/test_sdlc_feat_x") and rel in t.accept_files
    log = subprocess.run(["git", "log", "--oneline", "-1"], cwd=repo, capture_output=True, text=True).stdout
    assert "aceptacion" in log  # commiteado rojo por diseño
    assert state_rel == rel


def test_etapa1_sin_aprobacion_sigue(tmp_path, monkeypatch):
    repo = _repo(tmp_path)
    _configura(repo, monkeypatch, RED, approve=False)
    S.aceptacion()
    assert S.state["stages"][-1]["ok"] and "awaiting_approval" not in S.state


def test_etapa1_rechaza_test_verde_en_head(tmp_path, monkeypatch):
    repo = _repo(tmp_path)
    _configura(repo, monkeypatch, "def test_R1_trivial():\n    assert True\n", approve=False)
    with pytest.raises(S.StageFailed, match="ya pasa en HEAD"):
        S.aceptacion()


def test_etapa1_rechaza_test_sin_ids(tmp_path, monkeypatch):
    repo = _repo(tmp_path)
    _configura(repo, monkeypatch, "def test_suma():\n    assert False\n", approve=False)
    with pytest.raises(S.StageFailed, match="R<n>"):
        S.aceptacion()


def test_interprete_del_repo_es_el_venv_sembrado(tmp_path):
    """Un repo con venv propio (Portfolio, Adepor) corre suite y accept_cmd con SU Python, no con el de mmorch."""
    import sys
    import venv
    t = types.SimpleNamespace(name="i", task="t", accept_files={})
    S.configure(t, contract=[], feat={"repo": str(tmp_path), "files": [], "suite": []}, wt=tmp_path)
    assert S.PY == sys.executable  # sin venv en el worktree: el de mmorch
    venv.create(tmp_path / ".venv", with_pip=False)
    S.configure(t, contract=[], feat={"repo": str(tmp_path), "files": [], "suite": []}, wt=tmp_path)
    assert S.PY != sys.executable and ".venv" in S.PY
    ok, out = S.sh('python -c "import sys; print(sys.prefix)"')  # `python` dentro de un comando del repo
    assert ok and ".venv" in out


def test_cfg_lee_el_sdlc_toml_del_repo_aunque_el_worktree_no_lo_tenga(tmp_path):
    """Adopcion sin commitear: el worktree sale de HEAD sin sdlc.toml; la config del repo igual aplica y el worktree la pisa."""
    repo, wt = tmp_path / "repo", tmp_path / "wt"
    repo.mkdir(); wt.mkdir()
    (repo / "sdlc.toml").write_text('lint_cmd = "npx eslint {files}"\nseed_globs = ["app/node_modules"]\n', encoding="utf-8")
    t = types.SimpleNamespace(name="c", task="t", accept_files={})
    S.configure(t, contract=[], feat={"repo": str(repo), "files": [], "suite": []}, wt=wt)
    assert S.CFG["lint_cmd"] == "npx eslint {files}" and S._toml(repo)["seed_globs"] == ["app/node_modules"]
    (wt / "sdlc.toml").write_text('lint_cmd = "otro"\n', encoding="utf-8")
    S.configure(t, contract=[], feat={"repo": str(repo), "files": [], "suite": []}, wt=wt)
    assert S.CFG["lint_cmd"] == "otro"
