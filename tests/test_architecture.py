"""Tests de architecture.py (chequeos mecanicos, sin LLM, sin API)."""

import subprocess

from mmorch.architecture import (
    co_change_pairs,
    find_cycles,
    god_module_candidates,
    import_graph,
    pollution_candidates,
)


def _git(cwd, *args):
    subprocess.run(["git", *args], cwd=cwd, capture_output=True)


def make_repo(tmp_path):
    root = tmp_path / "orch"
    (root / "mmorch").mkdir(parents=True)
    (root / "tests").mkdir()
    _git(root, "init", "-b", "main")
    _git(root, "config", "user.email", "t@t")
    _git(root, "config", "user.name", "t")
    return root


def write(root, name, content=""):
    (root / "mmorch" / name).write_text(content, encoding="utf-8")


def commit(root, msg, files):
    for f in files:
        _git(root, "add", f"mmorch/{f}")
    _git(root, "commit", "-m", msg, "--no-verify")


def test_import_graph_solo_mmorch_dot_algo(tmp_path):
    root = make_repo(tmp_path)
    write(root, "a.py", "from mmorch.b import f\nimport os\n")
    write(root, "b.py", "def f(): pass\n")
    edges = import_graph(root)
    assert edges == {"a": {"b"}}


def test_import_graph_ve_imports_dentro_de_funciones(tmp_path):
    """El 95% de los imports de este repo son locales (dentro de funciones,
    no top-level) — un scan solo top-level los pierde casi todos."""
    root = make_repo(tmp_path)
    write(root, "a.py", "def f():\n    from mmorch.b import g\n    return g()\n")
    write(root, "b.py", "def g(): pass\n")
    edges = import_graph(root)
    assert edges == {"a": {"b"}}


def test_find_cycles_detecta_ab_pero_no_cadena_simple(tmp_path):
    assert find_cycles({"a": {"b"}, "b": {"a"}}) == [("a", "b")]
    assert find_cycles({"a": {"b"}, "b": {"c"}}) == []


def test_god_module_candidates_por_loc_y_fan_in(tmp_path):
    root = make_repo(tmp_path)
    write(root, "grande.py", "\n".join(f"x{i} = {i}" for i in range(900)))
    write(root, "chico.py", "x = 1\n")
    edges = {"a": {"chico"}, "b": {"chico"}, "c": {"chico"}, "d": {"chico"}}
    cands = god_module_candidates(root, edges)
    nombres = {c["module"] for c in cands}
    assert "mmorch/grande.py" in nombres  # por LOC
    grande = next(c for c in cands if c["module"] == "mmorch/grande.py")
    assert "900" in grande["razon"] or "lineas" in grande["razon"]


def test_co_change_detecta_par_no_conectado_por_import(tmp_path):
    root = make_repo(tmp_path)
    write(root, "a.py", "a = 1\n")
    write(root, "b.py", "b = 1\n")
    commit(root, "base", ["a.py", "b.py"])
    for i in range(3):
        write(root, "a.py", f"a = {i}\n")
        write(root, "b.py", f"b = {i}\n")
        commit(root, f"cambio {i}", ["a.py", "b.py"])
    pares = co_change_pairs(root, min_cochanges=2, min_ratio=0.5)
    assert any(set(p["archivos"]) == {"a.py", "b.py"} for p in pares)


def test_co_change_ignora_si_ya_conectados_por_import(tmp_path):
    root = make_repo(tmp_path)
    write(root, "a.py", "from mmorch.b import f\n")
    write(root, "b.py", "def f(): pass\n")
    commit(root, "base", ["a.py", "b.py"])
    for i in range(3):
        write(root, "a.py", f"from mmorch.b import f  # v{i}\n")
        write(root, "b.py", f"def f(): pass  # v{i}\n")
        commit(root, f"cambio {i}", ["a.py", "b.py"])
    pares = co_change_pairs(root, min_cochanges=2, min_ratio=0.5)
    assert pares == []  # ya conectados -> el acoplamiento ya es explicito


def test_co_change_excluye_commits_de_barrido(tmp_path):
    """Un commit tipo 'gate de ruff en todo el repo' toca muchos archivos a
    la vez -> co-cambio falso entre modulos que no tienen nada que ver."""
    root = make_repo(tmp_path)
    nombres = [f"m{i}.py" for i in range(10)]
    for n in nombres:
        write(root, n, "x = 1\n")
    commit(root, "base", nombres)
    for i in range(3):
        for n in nombres:
            write(root, n, f"x = {i}\n")
        commit(root, f"barrido {i}", nombres)
    pares = co_change_pairs(root, min_cochanges=2, min_ratio=0.5)
    assert pares == []  # el barrido de 10 archivos se excluye del conteo


def test_pollution_candidates_detecta_global_mutado(tmp_path):
    tests_dir = tmp_path / "tests"
    tests_dir.mkdir()
    (tests_dir / "test_x.py").write_text(
        "CACHE = {}\n\ndef test_algo():\n    CACHE['x'] = 1\n", encoding="utf-8")
    (tests_dir / "test_y.py").write_text(
        "CONST = {'a': 1}\n\ndef test_lee(): assert CONST['a'] == 1\n",
        encoding="utf-8")
    out = pollution_candidates(tests_dir)
    archivos = {o["file"] for o in out}
    assert "test_x.py" in archivos          # CACHE se muta -> señal
    assert "test_y.py" not in archivos      # CONST solo se lee -> sin señal


def test_pollution_candidates_detecta_mkdtemp_sin_tmp_path(tmp_path):
    tests_dir = tmp_path / "tests"
    tests_dir.mkdir()
    (tests_dir / "test_z.py").write_text(
        "import tempfile\n\ndef test_algo():\n    d = tempfile.mkdtemp()\n",
        encoding="utf-8")
    out = pollution_candidates(tests_dir)
    assert any(o["file"] == "test_z.py" for o in out)
