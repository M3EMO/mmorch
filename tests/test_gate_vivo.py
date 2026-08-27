"""W4.1 — gate de evolución unificado en el camino VIVO.

Tres ramas nuevas: (1) tamper/hash-faltante de GOAL => HALT del nightly al ARRANQUE
(antes goal_guard solo gateaba caminos que el nightly nunca ejecuta); (2) diff
desalineado con GOAL => sin PR (goal_aligned entra al pipeline pre-PR); (3) el harness
de evaluación (gates/smoke/tests del fitness/allowlist) es zona roja.
"""
import json
from types import SimpleNamespace

import pytest

import mmorch.evolve as E
import mmorch.nightly as N
from mmorch.evolve import coordinated_evolve_round, snapshot_change, zone_of
from mmorch.goal import GoalTampered, goal_guard


# --------------------------------------------------------------------------- #
# 1. goal_guard estricto + gate de arranque del nightly                        #
# --------------------------------------------------------------------------- #
def _goal_files(tmp_path, text="north star v1"):
    g, h = tmp_path / "GOAL.md", tmp_path / "GOAL.hash"
    g.write_text(text, encoding="utf-8")
    return g, h


def test_goal_guard_missing_hash_halts_when_strict(tmp_path):
    g, h = _goal_files(tmp_path)
    with pytest.raises(GoalTampered, match="faltante"):
        goal_guard(g, h, allow_init=False)   # hash faltante = HALT, no auto-autorización
    assert not h.exists(), "el modo estricto NO debe regenerar el hash solo"


def test_goal_guard_allow_init_still_default(tmp_path):
    g, h = _goal_files(tmp_path)
    goal_guard(g, h)                          # default: init auto-autoriza (compat)
    assert h.exists()


def test_nightly_goal_gate_tamper(tmp_path):
    g, h = _goal_files(tmp_path)
    goal_guard(g, h)                          # autorizado
    assert N._goal_gate(path=g, hash_path=h) is None
    g.write_text("north star ADULTERADO", encoding="utf-8")
    reason = N._goal_gate(path=g, hash_path=h)
    assert reason and "re-autorizaci" in reason


def test_nightly_goal_gate_missing_hash(tmp_path):
    g, h = _goal_files(tmp_path)
    reason = N._goal_gate(path=g, hash_path=h)
    assert reason and "faltante" in reason
    assert not h.exists()


def test_nightly_goal_gate_missing_goal(tmp_path):
    reason = N._goal_gate(path=tmp_path / "no.md", hash_path=tmp_path / "no.hash")
    assert reason and "GOAL.md faltante" in reason


def test_nightly_main_halts_with_auditable_episode(tmp_path, monkeypatch):
    log = tmp_path / "nightly.jsonl"
    monkeypatch.setattr(N, "LOG", log)
    monkeypatch.setattr(N, "_goal_gate", lambda: "GOAL.md cambió sin re-autorización")
    with pytest.raises(SystemExit):
        N.main()                              # HALT antes de cualquier trabajo
    rec = json.loads(log.read_text(encoding="utf-8").splitlines()[0])
    assert rec["halt"] == "goal_tampered"
    assert "re-autorizaci" in rec["halt_reason"]


# --------------------------------------------------------------------------- #
# 2. goal_aligned sobre el diff pre-PR                                         #
# --------------------------------------------------------------------------- #
def _fit_ok(c):
    return {"ok": True, "checks": {}}


def _round(tmp_path, monkeypatch, aligned_fn, fitness_fn=_fit_ok):
    """1 candidato verde que pasa sandbox; devuelve (resultado, pr_calls, state_path)."""
    monkeypatch.setattr(E, "_git", lambda *a, cwd: SimpleNamespace(returncode=1, stdout=""))
    (tmp_path / "logs").mkdir()
    state = tmp_path / "pr_state.json"
    pr_calls = []

    def pr_fn(branch, title):
        pr_calls.append(branch)
        return {"pushed": True, "pr_number": 7}

    c = snapshot_change("x.py", "def f(): return 1", "mejora x", root=tmp_path)
    r = coordinated_evolve_round(
        [c], root=tmp_path, path=state, pr_fn=pr_fn, aligned_fn=aligned_fn,
        fitness_fn=fitness_fn,
        sandbox_fn=lambda c: {"ok": True, "branch": "b1", "fitness": {}})
    return r, pr_calls, state


def test_misaligned_diff_blocks_pr_and_logs(tmp_path, monkeypatch):
    seen = []

    def refuta(c):
        seen.append(c.target)
        return SimpleNamespace(passed=False, refutations=["deriva del norte"])

    r, pr_calls, state = _round(tmp_path, monkeypatch, refuta)
    assert r["blocked_goal"] == ["x.py"] and r["opened"] == []
    assert pr_calls == [], "desalineado => NO se abre PR"
    assert seen == ["x.py"]
    assert json.loads(state.read_text(encoding="utf-8")) == {}, "no se trackea"
    red = json.loads((tmp_path / "logs" / "evolve_red.jsonl")
                     .read_text(encoding="utf-8").splitlines()[0])
    assert red["kind"] == "goal_misaligned" and red["refutations"] == ["deriva del norte"]


def test_aligned_diff_opens_pr(tmp_path, monkeypatch):
    r, pr_calls, _ = _round(tmp_path, monkeypatch,
                            lambda c: SimpleNamespace(passed=True, refutations=[]))
    assert r["opened"] == ["x.py"] and r["blocked_goal"] == []
    assert pr_calls == ["b1"]


def test_aligned_fn_infra_error_fails_open(tmp_path, monkeypatch):
    def boom(c):
        raise RuntimeError("proveedor caído")

    r, pr_calls, _ = _round(tmp_path, monkeypatch, boom)
    # fail-OPEN: el check es red extra, no un single-point-of-failure — el humano del PR gatea
    assert r["opened"] == ["x.py"] and pr_calls == ["b1"]


def test_default_aligned_fn_judges_the_diff(monkeypatch):
    captured = {}
    monkeypatch.setattr("mmorch.goal.goal_aligned",
                        lambda text, phase: captured.update(text=text, phase=phase)
                        or SimpleNamespace(passed=True, refutations=[]))
    c = E.Change("x.py", "def f(): return 2\n", "def f(): return 1\n", "cambia f")
    v = E._diff_goal_aligned(c)
    assert v.passed
    assert "cambia f" in captured["text"] and "+def f(): return 2" in captured["text"]
    assert captured["phase"] == "evolve_pr_gate"


# --------------------------------------------------------------------------- #
# 3. harness de evaluación en zona roja                                        #
# --------------------------------------------------------------------------- #
@pytest.mark.parametrize("target", [
    "scripts/gates.py", "scripts/smoke.py", "mmorch/evolve.py",
    "tests/test_evolve_motor.py", "tests/test_goal.py", "tests/test_evolve_goal_guard.py",
    "GOAL.md", "GOAL.hash",
])
def test_eval_harness_is_red_zone(target):
    c = E.Change(target, "cualquier contenido", "antes", "toca el harness")
    assert zone_of(c) == "red", f"{target} define el gate => el pipeline no puede tocarlo"


def test_normal_module_still_yellow():
    c = E.Change("mmorch/memory.py", "x = 2", "x = 1", "cambio normal")
    assert zone_of(c) == "yellow"


# --------------------------------------------------------------------------- #
# 4. W4.3 — evaluate() (fitness compuesta) como gate pre-PR                    #
# --------------------------------------------------------------------------- #
def test_fitness_fail_blocks_pr_and_logs(tmp_path, monkeypatch):
    seen = []

    def fit_bad(c):
        seen.append(c.target)
        return {"ok": False, "checks": {"ast_valid": True, "ensemble_xfamily": False}}

    r, pr_calls, state = _round(tmp_path, monkeypatch,
                                lambda c: SimpleNamespace(passed=True, refutations=[]),
                                fitness_fn=fit_bad)
    assert r["blocked_fitness"] == ["x.py"] and r["opened"] == []
    assert pr_calls == [], "fitness roja => NO se abre PR"
    assert seen == ["x.py"]
    assert json.loads(state.read_text(encoding="utf-8")) == {}, "no se trackea"
    red = json.loads((tmp_path / "logs" / "evolve_red.jsonl")
                     .read_text(encoding="utf-8").splitlines()[0])
    assert red["kind"] == "fitness_fail" and red["checks"]["ensemble_xfamily"] is False


def test_fitness_ok_opens_pr(tmp_path, monkeypatch):
    r, pr_calls, _ = _round(tmp_path, monkeypatch,
                            lambda c: SimpleNamespace(passed=True, refutations=[]))
    assert r["opened"] == ["x.py"] and r["blocked_fitness"] == []
    assert pr_calls == ["b1"]


def test_fitness_infra_error_fails_open(tmp_path, monkeypatch):
    def boom(c):
        raise RuntimeError("proveedor caído")

    r, pr_calls, _ = _round(tmp_path, monkeypatch,
                            lambda c: SimpleNamespace(passed=True, refutations=[]),
                            fitness_fn=boom)
    # fail-OPEN: mismo contrato que aligned_fn — el humano del PR sigue gateando
    assert r["opened"] == ["x.py"] and pr_calls == ["b1"]


def test_default_pr_fitness_skips_goal_and_relative_cost(monkeypatch):
    """El default NO duplica lo ya pagado en la ronda: goal=False (lo cubre aligned_fn
    de W4.1) y el costo relativo no gatea (solo budget absoluto)."""
    captured = {}

    def fake_evaluate(c, **kw):
        captured.update(kw)
        return {"ok": True, "checks": {}}

    monkeypatch.setattr(E, "evaluate", fake_evaluate)
    c = E.Change("x.py", "def f(): return 2\n", "def f(): return 1\n", "cambia f")
    r = E._pr_fitness(c)
    assert r["ok"]
    assert captured["goal"] is False, "goal_aligned ya lo corre aligned_fn (W4.1)"
    assert captured["cost_fn"](c) is True, "costo relativo sin medicion no gatea"
