"""Tests de automerge — repo git real en tmp_path (sin API, sin engine)."""

import subprocess

from mmorch.automerge import classify_branch, try_automerge


def _git(repo, *args):
    return subprocess.run(["git", *args], cwd=repo, capture_output=True, text=True)


def make_repo(tmp_path):
    repo = tmp_path / "r"
    repo.mkdir()
    (repo / "tests").mkdir()
    (repo / "mmorch").mkdir()
    (repo / "logs").mkdir()
    _git(repo, "init", "-b", "main")
    _git(repo, "config", "user.email", "t@t")
    _git(repo, "config", "user.name", "t")
    (repo / "mmorch" / "mod.py").write_text("x = 1\n", encoding="utf-8")
    (repo / "tests" / "test_mod.py").write_text("def test_x():\n    assert True\n",
                                               encoding="utf-8")
    _git(repo, "add", "-A")
    _git(repo, "commit", "-m", "base", "--no-verify")
    return str(repo)


def _branch(repo, name, path, content):
    _git(repo, "checkout", "-b", name)
    from pathlib import Path
    p = Path(repo) / path
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(content, encoding="utf-8")
    _git(repo, "add", "-A")
    _git(repo, "commit", "-m", name, "--no-verify")
    _git(repo, "checkout", "main")


def test_green_tests_only(tmp_path):
    repo = make_repo(tmp_path)
    _branch(repo, "b-tests", "tests/test_mod.py",
            "def test_x():\n    assert True\n\ndef test_y():\n    assert 1\n")
    c = classify_branch(repo, "b-tests", base="main")
    assert c["zone"] == "green"


def test_yellow_module_edit(tmp_path):
    repo = make_repo(tmp_path)
    _branch(repo, "b-mod", "mmorch/mod.py", "x = 2\n")
    assert classify_branch(repo, "b-mod", base="main")["zone"] == "yellow"


def test_green_new_isolated_file(tmp_path):
    repo = make_repo(tmp_path)
    _branch(repo, "b-new", "mmorch/nuevo.py", "y = 1\n")
    assert classify_branch(repo, "b-new", base="main")["zone"] == "green"


def test_red_content(tmp_path):
    repo = make_repo(tmp_path)
    _branch(repo, "b-red", "tests/test_evil.py",
            "import os\n\ndef test_e():\n    os.system('echo hi')\n")
    c = classify_branch(repo, "b-red", base="main")
    assert c["zone"] == "red"


def test_red_path(tmp_path):
    repo = make_repo(tmp_path)
    _branch(repo, "b-env", ".env", "K=v\n")
    assert classify_branch(repo, "b-env", base="main")["zone"] == "red"


def test_automerge_green_merges_and_ledgers(tmp_path):
    repo = make_repo(tmp_path)
    _branch(repo, "b-tests", "tests/test_mod.py",
            "def test_x():\n    assert True\n\ndef test_z():\n    assert 2\n")
    r = try_automerge(repo, "b-tests", base="main", source="test")
    assert r["merged"] is True and r["zone"] == "green"
    log = _git(repo, "log", "--oneline", "-2").stdout
    assert "b-tests" in log or "Merge" in log
    from pathlib import Path
    ledger = (Path(repo) / "logs" / "automerge_ledger.jsonl").read_text(encoding="utf-8")
    assert '"merged": true' in ledger


def test_automerge_yellow_refuses(tmp_path):
    repo = make_repo(tmp_path)
    _branch(repo, "b-mod", "mmorch/mod.py", "x = 3\n")
    r = try_automerge(repo, "b-mod", base="main", source="test")
    assert r["merged"] is False and r["zone"] == "yellow"


def test_nuevo_archivo_con_palabra_password_no_es_rojo(tmp_path):
    """05 #7: la MERA palabra password/secret en un test/doc nuevo no es rojo."""
    repo = make_repo(tmp_path)
    _branch(repo, "b-fixture", "tests/test_login.py",
            'def test_password_reset():\n'
            '    password = "test"\n'
            '    assert "secret" in "secret field"\n')
    assert classify_branch(repo, "b-fixture", base="main")["zone"] == "green"


def test_nuevo_archivo_con_secreto_real_es_rojo(tmp_path):
    """Valor asignado con entropia (parece clave real) => rojo, aun en archivo nuevo."""
    repo = make_repo(tmp_path)
    _branch(repo, "b-leak", "tests/test_cfg.py",
            'api_key = "sk-9fK2mQ8xL0pW7zR4tY6uB1nV"\n\ndef test_x():\n    assert api_key\n')
    assert classify_branch(repo, "b-leak", base="main")["zone"] == "red"


def test_placeholder_de_baja_entropia_no_es_rojo():
    """Umbral conservador pero util: placeholder repetitivo pasa, clave real no."""
    from mmorch.evolve import red_content_hits
    assert red_content_hits('password = "hunter2hunter2"') == []
    assert red_content_hits('secret_key = "9fK2mQ8xL0pW7zR4tY6u"') != []
    # delta: el mismo secreto ya presente en baseline no cuenta como NUEVO
    leak = 'private_key = "9fK2mQ8xL0pW7zR4tY6u"'
    assert red_content_hits(leak, baseline=leak) == []


def test_e2e_carril_verde_ledger_y_rollback(tmp_path):
    """End-to-end: merge verde real en repo fake + ledger con schema obligatorio
    + rollback ensayado via git revert del merge commit."""
    import json
    from pathlib import Path
    repo = make_repo(tmp_path)
    _branch(repo, "b-verde", "tests/test_extra.py",
            "def test_ok():\n    assert True\n")
    r = try_automerge(repo, "b-verde", base="main", source="e2e")
    assert r["merged"] is True and r["veredicto"] == "merged"
    assert r["diff_hash"] and r["merge_sha"]
    assert {"kill_switch_ok", "paths_ok", "contenido_ok",
            "solo_tests_o_nuevos"} <= set(r["checks"])
    lines = (Path(repo) / "logs" / "automerge_ledger.jsonl").read_text(
        encoding="utf-8").splitlines()
    rec = json.loads(lines[-1])
    for k in ("ts", "branch", "diff_hash", "checks", "veredicto", "merge_sha"):
        assert k in rec, f"ledger sin campo obligatorio: {k}"
    assert rec["merge_sha"] == r["merge_sha"] and rec["branch"] == "b-verde"
    # rollback: revert del merge deja el arbol sin el archivo mergeado
    rv = _git(repo, "revert", "-m", "1", "--no-edit", r["merge_sha"])
    assert rv.returncode == 0, rv.stderr
    assert not (Path(repo) / "tests" / "test_extra.py").exists()


def test_ledger_registra_rechazo_amarillo(tmp_path):
    import json
    from pathlib import Path
    repo = make_repo(tmp_path)
    _branch(repo, "b-mod", "mmorch/mod.py", "x = 9\n")
    r = try_automerge(repo, "b-mod", base="main", source="test")
    assert r["veredicto"] == "rechazado_yellow" and r["merge_sha"] is None
    rec = json.loads((Path(repo) / "logs" / "automerge_ledger.jsonl")
                     .read_text(encoding="utf-8").splitlines()[-1])
    assert rec["veredicto"] == "rechazado_yellow" and rec["diff_hash"]


def test_automerge_paused(tmp_path):
    repo = make_repo(tmp_path)
    _branch(repo, "b-tests", "tests/test_mod.py", "def test_x():\n    assert True\n # y\n")
    from pathlib import Path
    (Path(repo) / "logs").mkdir(exist_ok=True)
    (Path(repo) / "logs" / "loop_paused").touch()
    r = try_automerge(repo, "b-tests", base="main", source="test")
    assert r["merged"] is False and r["zone"] == "paused"
