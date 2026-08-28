"""Tests de worktree_driver.seed() — guard de traversal (rescatado de
mmorch-sbx-89cc9fba829b, reaplicado limpio: el commit original traia un
fence de markdown filtrado adentro del .py, rompia el import)."""

import os

from mmorch.worktree_driver import Worktree


def test_seed_copia_lo_que_esta_dentro_del_repo(tmp_path):
    repo = tmp_path / "repo"
    dest = tmp_path / "wt"
    (repo / "cache").mkdir(parents=True)
    (repo / "cache" / "x.txt").write_text("hola", encoding="utf-8")
    dest.mkdir()
    wt = Worktree(str(repo), str(dest), "b")
    n = wt.seed(["cache"])
    assert n == 1
    assert (dest / "cache").exists()


def test_seed_no_sigue_symlink_que_resuelve_afuera_del_repo(tmp_path, monkeypatch):
    """Un symlink DENTRO del repo que resuelve a una ruta AFUERA no debe
    copiarse/linkearse al worktree aislado — ese es el bug que el fix
    original (mmorch-sbx-89cc9fba829b) atajaba."""
    repo = tmp_path / "repo"
    afuera = tmp_path / "afuera"
    dest = tmp_path / "wt"
    (repo / "cache").mkdir(parents=True)
    afuera.mkdir()
    (afuera / "secreto.txt").write_text("no deberia salir", encoding="utf-8")
    dest.mkdir()

    repo_real = os.path.realpath(str(repo))
    cache_path = os.path.normpath(os.path.join(str(repo), "cache"))
    afuera_real = os.path.realpath(str(afuera))

    real_realpath = os.path.realpath

    def fake_realpath(p):
        # simula que "repo/cache" es en realidad un symlink que apunta afuera
        # (sin depender de privilegios de symlink reales en Windows)
        if os.path.normpath(p) == cache_path:
            return afuera_real
        return real_realpath(p)

    monkeypatch.setattr(os.path, "realpath", fake_realpath)
    wt = Worktree(str(repo), str(dest), "b")
    n = wt.seed(["cache"])
    assert n == 0
    assert not (dest / "cache").exists()
    assert repo_real  # sanity: la variable se uso para armar el escenario


def test_seed_sin_patterns_no_hace_nada(tmp_path):
    repo = tmp_path / "repo"
    dest = tmp_path / "wt"
    repo.mkdir()
    dest.mkdir()
    wt = Worktree(str(repo), str(dest), "b")
    assert wt.seed(None) == 0
    assert wt.seed([]) == 0
