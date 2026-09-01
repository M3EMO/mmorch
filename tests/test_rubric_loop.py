"""rubric_loop: gerente determinista, checkers re-ejecutan, juez refuta por default,
K duro, lazo cerrado. gen/judge inyectados (cero API)."""
import sys, pathlib, json, importlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
RL = importlib.import_module("mmorch.rubric_loop")

GOOD = "```python\ndef inc(x):\n    return x + 1\n```"
BAD = "```python\ndef inc(x):\n    return x - 1\n```"

CHECKABLE = [{"id": "c1", "desc": "inc(x) pasa los asserts", "kind": "checkable",
              "checker": "python_exec",
              "ctx": {"code": "{attempt_code}\nassert inc(1)==2\nassert inc(-1)==0"}}]
SUBJ = [{"id": "s1", "desc": "el codigo tiene nombre claro", "kind": "subjective"}]


def _noop_close(monkeypatch):
    monkeypatch.setattr(RL, "_close_loop", lambda s: s.update(_closed=True))


def test_checkable_green_first_try_no_judge_needed(monkeypatch):
    _noop_close(monkeypatch)
    st = RL.run_rubric_loop("implementa inc", CHECKABLE, gen_fn=lambda p: GOOD,
                            judge_fn=lambda p: 1/0)   # juez jamas llamado
    assert st["phase"] == "done" and st["iteration"] == 1
    assert st["results"]["c1"]["cumple"] and st["results"]["c1"]["juez"] == "checker:python_exec"


def test_executor_corrects_after_checker_fail(monkeypatch):
    _noop_close(monkeypatch)
    attempts = iter([BAD, GOOD])
    st = RL.run_rubric_loop("implementa inc", CHECKABLE, gen_fn=lambda p: next(attempts))
    assert st["phase"] == "done" and st["iteration"] == 2   # fallo -> corrigio -> verde


def test_hard_K_escalates_with_state(monkeypatch):
    _noop_close(monkeypatch)
    st = RL.run_rubric_loop("implementa inc", CHECKABLE, K=2, gen_fn=lambda p: BAD)
    assert st["phase"] == "escalate" and st["iteration"] == 2
    s = RL.next_action(st)["summary"]
    assert s["cumplidos"] == 0 and s["pendientes"][0]["id"] == "c1"
    assert "rc=" in s["pendientes"][0]["evidencia"]          # evidencia EJECUTABLE adjunta


def test_judge_refutes_by_default_on_garbage(monkeypatch):
    _noop_close(monkeypatch)
    # juez devuelve basura ilegible -> nadie aprobado, vuelve al ejecutor, agota K
    st = RL.run_rubric_loop("tarea", SUBJ, K=2, gen_fn=lambda p: GOOD,
                            judge_fn=lambda p: "no json")
    assert st["phase"] == "escalate"
    assert not st["results"].get("s1", {}).get("cumple")


def test_judge_verdict_applied_and_corrections_fed_back(monkeypatch):
    _noop_close(monkeypatch)
    judged = []
    def judge(p):
        judged.append(p)
        if len(judged) == 1:
            return json.dumps([{"id": "s1", "cumple": False, "evidencia": "nombre criptico",
                                "correccion": "renombrar a incremento"}])
        return json.dumps([{"id": "s1", "cumple": True, "evidencia": "claro"}])
    st = RL.run_rubric_loop("tarea", SUBJ, gen_fn=lambda p: GOOD, judge_fn=judge)
    assert st["phase"] == "done" and st["iteration"] == 2
    # la correccion del juez viajo al prompt del ejecutor en la 2da vuelta
    assert len(judged) == 2


def test_mixed_checkable_runs_free_subjective_goes_to_judge(monkeypatch):
    _noop_close(monkeypatch)
    calls = {"judge": 0}
    def judge(p):
        calls["judge"] += 1
        assert "c1" not in p   # el juez LLM NO ve criterios checkables (checker los juzga)
        return json.dumps([{"id": "s1", "cumple": True, "evidencia": "ok"}])
    st = RL.run_rubric_loop("tarea", CHECKABLE + SUBJ, gen_fn=lambda p: GOOD, judge_fn=judge)
    assert st["phase"] == "done" and calls["judge"] == 1


def test_oneflow_same_family_rejected():
    try:
        RL.start_rubric("t", CHECKABLE, gen_model="deepseek-chat",
                        judge_model="deepseek-reasoner")
        assert False, "debio rechazar gen y judge misma familia"
    except ValueError as e:
        assert "OneFlow" in str(e)


def test_close_loop_records_outcome_with_context(monkeypatch):
    rec = {}
    monkeypatch.setattr("mmorch.feedback.record_outcome",
                        lambda arm, rew, **kw: rec.update(arm=arm, reward=rew, **kw))
    RL.run_rubric_loop("implementa inc bien", CHECKABLE, gen_fn=lambda p: GOOD)
    assert rec["reward"] == 1.0 and rec["pattern"] == "rubric_loop"
    assert rec["context"].startswith("implementa inc")        # comida pal ShadowPrior


def test_lcw_per_round_node_escalation(monkeypatch):
    """lcw: gen_for_round escala el ejecutor por ronda. round 1 barato (falla) ->
    round 2 fuerte (verde). Prueba que el plateau dispara la escalada de nodo."""
    _noop_close(monkeypatch)
    used = []
    def cheap(p): used.append("cheap"); return BAD
    def strong(p): used.append("strong"); return GOOD
    sched = lambda r: cheap if r == 1 else strong
    st = RL.run_rubric_loop("implementa inc", CHECKABLE, gen_for_round=sched)
    assert st["phase"] == "done" and st["iteration"] == 2
    assert used == ["cheap", "strong"]   # escalo de nodo en la ronda 2


def test_lcw_fallback_single_node(monkeypatch):
    """Sin gen_for_round -> comportamiento single-node de siempre (fallback)."""
    _noop_close(monkeypatch)
    st = RL.run_rubric_loop("implementa inc", CHECKABLE, gen_fn=lambda p: GOOD)
    assert st["phase"] == "done" and st["iteration"] == 1


def test_plan_mode_state_machine_roundtrip(monkeypatch):
    """MODO PLAN: el estado viaja como JSON (MCP), el 'plan' ejecuta cada accion."""
    _noop_close(monkeypatch)
    st = RL.start_rubric("implementa inc", CHECKABLE)
    st = json.loads(json.dumps(st))                # round-trip de serializacion
    act = RL.next_action(st)
    assert act["role"] == "executor" and "RUBRICA" in act["prompt"]
    st = RL.submit(st, GOOD)
    st = json.loads(json.dumps(st))
    assert RL.next_action(st)["role"] == "done"
