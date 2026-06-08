"""tests evolve.py Fase 3+4 — Change/rollback/evaluate/self_evolve. Sin API (inyectado)."""
from mmorch.evolve import (Change, snapshot_change, apply_change, rollback, evaluate,
                           zone_of, self_evolve)


def _ok_goal(desc):
    class V:
        passed = True
    return V()


def _bad_goal(desc):
    class V:
        passed = False
    return V()


# --- Fase 3: snapshot / apply / rollback ---
def test_apply_and_rollback_new_file(tmp_path):
    c = snapshot_change("nuevo.py", "x = 1\n", "archivo nuevo", root=tmp_path)
    assert c.before == "" and zone_of(c, root=tmp_path) == "green"
    apply_change(c, root=tmp_path)
    assert (tmp_path / "nuevo.py").read_text() == "x = 1\n"
    assert rollback(c, root=tmp_path) and not (tmp_path / "nuevo.py").exists()


def test_rollback_restores_existing(tmp_path):
    (tmp_path / "f.py").write_text("ORIGINAL\n", encoding="utf-8")
    c = snapshot_change("f.py", "MODIFICADO\n", "modifica f", root=tmp_path)
    assert c.before == "ORIGINAL\n" and zone_of(c, root=tmp_path) == "yellow"
    apply_change(c, root=tmp_path)
    assert rollback(c, root=tmp_path)
    assert (tmp_path / "f.py").read_text() == "ORIGINAL\n"


def test_evaluate_passes_clean(tmp_path):
    c = snapshot_change("n.py", "def f():\n    return 1\n", "ok", root=tmp_path)
    r = evaluate(c, root=tmp_path, run_tests=False, goal_fn=_ok_goal)
    assert r["ok"] and all(r["checks"].values())


def test_evaluate_rejects_bad_syntax(tmp_path):
    c = snapshot_change("n.py", "def f(:\n", "rota", root=tmp_path)
    r = evaluate(c, root=tmp_path, run_tests=False, goal_fn=_ok_goal)
    assert not r["ok"] and not r["checks"]["ast_valid"]


def test_evaluate_rejects_goal_misalign(tmp_path):
    c = snapshot_change("n.py", "x = 1\n", "deriva", root=tmp_path)
    r = evaluate(c, root=tmp_path, run_tests=False, goal_fn=_bad_goal)
    assert not r["ok"] and not r["checks"]["goal_aligned"]


# --- Fase 4: zona roja + self_evolve ---
def test_zone_red_for_forbidden_paths(tmp_path):
    for p in ("GOAL.md", ".env", "mmorch/config.py", "../escape.py", "/abs/path.py"):
        assert zone_of(Change(target=p, after="x", before="", description="d"), root=tmp_path) == "red"


def test_self_evolve_never_applies_red():
    red = Change(target="GOAL.md", after="HACKED", before="orig", description="cambiar goal")
    res = self_evolve(candidates=[red], do_apply=True, audit=False,
                      evaluate_fn=lambda c: {"ok": True, "checks": {"x": True}})
    assert not res["applied"] and red.id in res["blocked_red"]


def test_self_evolve_applies_green_winner(tmp_path):
    good = snapshot_change("cap.py", "def cap(): return 2\n", "nueva capacidad", root=tmp_path)
    bad = snapshot_change("cap2.py", "def c2(:\n", "rota", root=tmp_path)
    res = self_evolve(candidates=[good, bad], root=tmp_path, do_apply=True, audit=False,
                      evaluate_fn=lambda c: evaluate(c, root=tmp_path, run_tests=False, goal_fn=_ok_goal))
    assert res["applied"] and res["winner"] == good.id and res["zone"] == "green"
    assert (tmp_path / "cap.py").exists()
