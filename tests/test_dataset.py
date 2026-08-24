"""tests dataset.py — extracción de funciones (core del miner JIT-defect). Sin git/API."""
from mmorch.dataset import _functions_covering, _FIX_RE


def test_functions_covering_picks_changed():
    src = "def a():\n    return 1\n\ndef b():\n    x = 2\n    return x\n"
    # línea 5 (x=2) está en b
    fns = _functions_covering(src, {5})
    assert any("def b()" in f for f in fns) and not any("def a()" in f for f in fns)


def test_functions_covering_handles_broken_source():
    assert _functions_covering("def f(:\n", {1}) == []   # no parsea -> []


def test_fix_regex():
    assert _FIX_RE.search("Fix crash in parser")
    assert _FIX_RE.search("bugfix: wrong header")
    assert not _FIX_RE.search("add new feature")


# --- parsing de git: 8/8 mutantes sobrevivian a este archivo ----------------- #
from pathlib import Path

import mmorch.dataset as ds


def test_changed_lines_ignora_el_encabezado_del_diff(monkeypatch):
    """'+++ b/x.py' arranca con '+' pero NO es una linea agregada. Sin este
    test, cambiar el `and not` por un `or` pasaba desapercibido y metia el
    numero de linea del encabezado en el set."""
    diff = "\n".join(["diff --git a/x.py b/x.py",
                      "--- a/x.py",
                      "+++ b/x.py",
                      "@@ -1,0 +3,2 @@",
                      "+nueva a",
                      "+nueva b"])
    monkeypatch.setattr(ds, "_git", lambda *a, **k: diff)
    assert ds._changed_lines(Path("."), "sha", "x.py") == {3, 4}


def test_changed_lines_cuenta_contexto_y_saltea_borradas(monkeypatch):
    diff = "\n".join(["@@ -1,3 +1,3 @@",
                      " contexto",      # linea 1
                      "-vieja",         # no avanza la numeracion nueva
                      "+nueva",         # linea 2
                      " final"])        # linea 3
    monkeypatch.setattr(ds, "_git", lambda *a, **k: diff)
    assert ds._changed_lines(Path("."), "sha", "x.py") == {2}


def test_fix_commits_corta_exactamente_en_el_tope(monkeypatch):
    log = "\n".join(f"sha{i}|fix: algo {i}" for i in range(5))
    monkeypatch.setattr(ds, "_git", lambda *a, **k: log)
    assert ds.fix_commits(Path("."), max_n=3) == ["sha0", "sha1", "sha2"]


def test_fix_commits_filtra_los_que_no_son_fix(monkeypatch):
    log = "aaa|feat: cosa nueva\nbbb|fix: rompia al abrir\nccc|docs: notas"
    monkeypatch.setattr(ds, "_git", lambda *a, **k: log)
    assert ds.fix_commits(Path("."), max_n=10) == ["bbb"]
