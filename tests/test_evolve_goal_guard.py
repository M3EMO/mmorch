"""B1: goal_guard (tamper-halt) cableado en evaluate() y en el apply de self_evolve().
Antes era DEAD CODE — un GOAL.md manipulado fuera de banda se volvia la rubrica de
goal_aligned sin verificar. GoalTampered debe PROPAGAR (no swallowearse)."""
import sys, pathlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import mmorch.evolve as EV
import mmorch.goal as G


class _V:  # fake verdict pa goal_aligned (sin API)
    passed = True


def _change():
    return EV.Change(target="logs/_b1_probe.txt", after="hola", before="", description="probe B1")


def test_evaluate_calls_goal_guard_and_propagates(monkeypatch):
    def _boom(*a, **k):
        raise G.GoalTampered("GOAL.md manipulado")
    monkeypatch.setattr(G, "goal_guard", _boom)
    raised = False
    try:
        EV.evaluate(_change(), run_tests=False, check_cost=False, check_ensemble=False,
                    goal=True, goal_fn=lambda d: _V())
    except G.GoalTampered:
        raised = True
    assert raised, "evaluate debe dejar PROPAGAR GoalTampered (no swallow)"


def test_evaluate_proceeds_when_guard_clean(monkeypatch):
    monkeypatch.setattr(G, "goal_guard", lambda *a, **k: None)   # sin tamper
    r = EV.evaluate(_change(), run_tests=False, check_cost=False, check_ensemble=False,
                    goal=True, goal_fn=lambda d: _V())
    assert r["checks"]["goal_aligned"] is True


def test_self_evolve_guards_at_apply(monkeypatch, tmp_path):
    # evaluate_fn inyectado (esquiva el guard del eval) -> aisla el guard del APPLY.
    monkeypatch.setattr(EV, "evaluate", lambda c, **k: {"ok": True, "checks": {"x": True},
                                                        "change_id": c.id})
    monkeypatch.setattr(EV, "zone_of", lambda c, **k: "green")
    monkeypatch.setattr(EV, "apply_change", lambda c, **k: (_ for _ in ()).throw(
        AssertionError("apply NO debe ejecutarse si el guard frena")))
    def _boom(*a, **k):
        raise G.GoalTampered("tamper en apply")
    monkeypatch.setattr(G, "goal_guard", _boom)
    raised = False
    try:
        EV.self_evolve(candidates=[_change()], do_apply=True, audit=False)
    except G.GoalTampered:
        raised = True
    assert raised, "self_evolve debe frenar en el guard ANTES de apply_change"
