"""B1: goal_guard (tamper-halt) cableado en evaluate(). Antes era DEAD CODE — un
GOAL.md manipulado fuera de banda se volvia la rubrica de goal_aligned sin verificar.
GoalTampered debe PROPAGAR (no swallowearse). (El guard del apply de self_evolve se
fue con el motor en W4.3; el camino vivo lo gatea nightly._goal_gate al arranque.)"""
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
        EV.evaluate(_change(), check_cost=False, check_ensemble=False,
                    goal=True, goal_fn=lambda d: _V())
    except G.GoalTampered:
        raised = True
    assert raised, "evaluate debe dejar PROPAGAR GoalTampered (no swallow)"


def test_evaluate_proceeds_when_guard_clean(monkeypatch):
    monkeypatch.setattr(G, "goal_guard", lambda *a, **k: None)   # sin tamper
    r = EV.evaluate(_change(), check_cost=False, check_ensemble=False,
                    goal=True, goal_fn=lambda d: _V())
    assert r["checks"]["goal_aligned"] is True
