"""Tests de slim (fakes, sin API ni sandbox real)."""


from mmorch.slim import pick_module, slim_one


def make_root(tmp_path):
    (tmp_path / "mmorch").mkdir()
    (tmp_path / "logs").mkdir()
    (tmp_path / "mmorch" / "gordo.py").write_text("x = 1\n" * 100, encoding="utf-8")
    (tmp_path / "mmorch" / "flaco.py").write_text("y = 1\n", encoding="utf-8")
    (tmp_path / "mmorch" / "__init__.py").write_text("", encoding="utf-8")
    return tmp_path


def test_pick_biggest_first_and_retry_window(tmp_path):
    root = make_root(tmp_path)
    assert pick_module(root, {}, today="2026-08-19") == "mmorch/gordo.py"
    state = {"mmorch/gordo.py": {"retry_after": "2026-08-25"}}
    assert pick_module(root, state, today="2026-08-19") == "mmorch/flaco.py"


def test_slim_no_adelgazo_guard(tmp_path):
    root = make_root(tmp_path)
    r = slim_one(str(root), today="2026-08-19",
                 propose_fn=lambda t, f: "z = 1\n" * 200)  # AGRANDA
    assert r["status"] == "no_adelgazo"


def test_slim_branch_on_green(tmp_path):
    root = make_root(tmp_path)
    r = slim_one(str(root), today="2026-08-19",
                 propose_fn=lambda t, f: "x = 1\n" * 10,
                 evolve_round_fn=lambda c: {"opened": [c.target]})
    assert r["status"] == "branch" and r["ahorro_chars"] > 0
    # segunda corrida rota al siguiente modulo
    r2 = slim_one(str(root), today="2026-08-19",
                  propose_fn=lambda t, f: "",
                  evolve_round_fn=lambda c: {"opened": []})
    assert r2["module"] == "mmorch/flaco.py"


def test_slim_paused(tmp_path):
    root = make_root(tmp_path)
    (root / "logs" / "loop_paused").touch()
    assert slim_one(str(root), today="2026-08-19")["skipped"] == "paused"
