"""Banco de refutacion (ticket 16): clasifica un test propuesto por ejecucion contra la version con defecto y la corregida."""
import os
import subprocess

import mmorch.refutacion as R

DEFECTO = "def dedup(claves):\n    return sorted({'|'.join(c) for c in claves})\n"
CORREGIDA = "def dedup(claves):\n    return sorted(set(claves))\n"


def _repo(tmp_path):
    repo = tmp_path / "repo"
    (repo / "pkg").mkdir(parents=True)
    git = ["git", "-c", "user.name=t", "-c", "user.email=t@t"]
    subprocess.run(["git", "init", "-q"], cwd=repo, check=True)
    shas = []
    for codigo in (DEFECTO, CORREGIDA):
        (repo / "pkg" / "m.py").write_text(codigo, encoding="utf-8")
        subprocess.run(["git", "add", "-A"], cwd=repo, check=True)
        subprocess.run([*git, "commit", "-q", "-m", "v"], cwd=repo, check=True)
        shas.append(subprocess.run(["git", "rev-parse", "HEAD"], cwd=repo, capture_output=True, text=True).stdout.strip())
    return repo, shas


def test_clasifica_acierto_falsa_alarma_neutro_e_invalido(tmp_path, monkeypatch):
    repo, (defecto, corregida) = _repo(tmp_path)
    monkeypatch.setattr(R, "resolve", lambda nombre: str(repo))
    casos = {"dedup": {"id": "dedup", "proyecto": "x", "defecto": defecto, "corregida": corregida,
                       "prueba_rel": "tests/test_propuesta.py", "sparse": ["pkg", "tests"],
                       "cmd": '"{python}" -m pytest -q -p no:cacheprovider {prueba}',
                       "invalido": r"ERROR collecting|SyntaxError"}}
    separa = "from pkg.m import dedup\ndef test_R9():\n    assert len(dedup([('a|b', 'c'), ('a', 'b|c')])) == 2\n"
    with R.Banco(casos) as banco:
        assert banco.evaluar("dedup", separa)["clase"] == "acierto"
        assert banco.evaluar("dedup", "def test_R9():\n    assert False\n")["clase"] == "falsa_alarma"
        assert banco.evaluar("dedup", "def test_R9():\n    assert True\n")["clase"] == "neutro"
        assert banco.evaluar("dedup", "def test_R9(:\n")["clase"] == "invalido"
        paths = [wt.path for wt in banco._wts.values()]
        assert len(paths) == 2  # una materializacion por version, reusada entre evaluaciones
    assert not any(os.path.exists(p) for p in paths)
    assert subprocess.run(["git", "worktree", "list"], cwd=repo, capture_output=True, text=True).stdout.count("\n") == 1
    assert "detached" not in subprocess.run(["git", "branch"], cwd=repo, capture_output=True, text=True).stdout


def test_cli_evalua_un_archivo_contra_el_banco_local(tmp_path, monkeypatch):
    """`mmorch refutacion <caso> <archivo>`: la medicion del ticket 17 y la revision a mano usan la misma entrada."""
    import json

    from mmorch.cli import main
    repo, (defecto, corregida) = _repo(tmp_path)
    monkeypatch.setattr(R, "resolve", lambda nombre: str(repo))
    banco = tmp_path / "banco.json"
    banco.write_text(json.dumps([{"id": "dedup", "proyecto": "x", "defecto": defecto, "corregida": corregida,
                                  "prueba_rel": "tests/test_propuesta.py", "cmd": '"{python}" -m pytest -q {prueba}',
                                  "invalido": "ERROR collecting"}]), encoding="utf-8")
    monkeypatch.setattr(R, "banco_path", lambda: banco)
    (tmp_path / "p.py").write_text("from pkg.m import dedup\ndef test_R9():\n    assert len(dedup([('a|b', 'c'), ('a', 'b|c')])) == 2\n",
                                   encoding="utf-8")
    assert main(["refutacion", "dedup", str(tmp_path / "p.py")]) == 0
