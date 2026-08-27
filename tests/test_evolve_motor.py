"""tests evolve.py — Change/evaluate (fitness compuesta) + zona. Sin API (inyectado).
W4.3: el motor self_evolve/rollback estructural se borro (museo); la reversibilidad
real es git (sandbox_branch + revert del carril automerge) y evaluate() vive como
gate pre-PR del nightly (ver test_gate_vivo.py)."""
from mmorch.evolve import Change, snapshot_change, apply_change, evaluate, zone_of


def _ok_goal(desc):
    class V:
        passed = True
    return V()


def _bad_goal(desc):
    class V:
        passed = False
    return V()


def _ev(c, goal_fn=_ok_goal):
    # inyecta goal + ensemble (sin API)
    return evaluate(c, goal_fn=goal_fn, ensemble_fn=lambda ch: True)


def test_snapshot_and_apply(tmp_path):
    c = snapshot_change("nuevo.py", "x = 1\n", "archivo nuevo", root=tmp_path)
    assert c.before == "" and zone_of(c, root=tmp_path) == "green"
    apply_change(c, root=tmp_path)
    assert (tmp_path / "nuevo.py").read_text() == "x = 1\n"


def test_snapshot_captures_previous_content(tmp_path):
    (tmp_path / "f.py").write_text("ORIGINAL\n", encoding="utf-8")
    c = snapshot_change("f.py", "MODIFICADO\n", "modifica f", root=tmp_path)
    assert c.before == "ORIGINAL\n" and zone_of(c, root=tmp_path) == "yellow"


def test_evaluate_passes_clean(tmp_path):
    c = snapshot_change("n.py", "def f():\n    return 1\n", "ok", root=tmp_path)
    r = _ev(c)
    assert r["ok"] and all(r["checks"].values())


def test_evaluate_rejects_bad_syntax(tmp_path):
    c = snapshot_change("n.py", "def f(:\n", "rota", root=tmp_path)
    r = _ev(c)
    assert not r["ok"] and not r["checks"]["ast_valid"]


def test_evaluate_rejects_goal_misalign(tmp_path):
    c = snapshot_change("n.py", "x = 1\n", "deriva", root=tmp_path)
    r = _ev(c, goal_fn=_bad_goal)
    assert not r["ok"] and not r["checks"]["goal_aligned"]


def test_evaluate_cost_fail_closed_on_modified_without_measurement(tmp_path):
    # modifica archivo existente SIN cost_fn que mida -> cost_ok False (fail-closed)
    (tmp_path / "m.py").write_text("a = 1\n", encoding="utf-8")
    c = snapshot_change("m.py", "a = 2\n", "modifica", root=tmp_path)
    r = _ev(c)
    assert not r["ok"] and not r["checks"]["cost_ok"]


def test_zone_red_for_forbidden_paths(tmp_path):
    for p in ("GOAL.md", ".env", "mmorch/config.py", "../escape.py", "/abs/path.py"):
        assert zone_of(Change(target=p, after="x", before="", description="d"), root=tmp_path) == "red"
