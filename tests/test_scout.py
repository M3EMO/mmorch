"""scout: pre-pass entorno-primero (Fable 5). Determinista por default ($0); inyecta brief
de grounding al ejecutor; scout_delta MIDE si reduce iteraciones (no lo asume)."""
import sys, pathlib, json, importlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import mmorch.scout as SC
RL = importlib.import_module("mmorch.rubric_loop")

CRIT = [{"id": "c1", "desc": "pasa los tests", "kind": "checkable", "checker": "python_exec",
         "ctx": {"code": "{attempt_code}\nassert True"}},
        {"id": "s1", "desc": "nombres claros", "kind": "subjective"}]


def test_gather_environment_is_deterministic_zero_api():
    env = SC.gather_environment("hacer X", CRIT)
    assert any("c1" in c and "python_exec" in c for c in env["constraints"])
    assert any("s1" in c and "subjetivo" in c for c in env["constraints"])
    assert "python_exec" in env["tools"]          # checkers disponibles


def test_scout_brief_no_llm_has_grounding():
    res = SC.scout("hacer X", CRIT, use_llm=False)
    assert "GROUNDING" in res["brief"] and "Restricciones" in res["brief"]
    assert res["llm_brief"] == ""                 # no se llamo API


def test_rubric_scout_injects_brief_into_executor(monkeypatch):
    st = RL.start_rubric("implementa inc", CRIT, scout=True)   # determinista
    assert st["scout_brief"] and "GROUNDING" in st["scout_brief"]
    act = RL.next_action(st)
    assert act["role"] == "executor" and "GROUNDING" in act["prompt"]
    # el grounding va ANTES de la tarea (prefijo estable)
    assert act["prompt"].index("GROUNDING") < act["prompt"].index("TAREA:")


def test_no_scout_by_default():
    st = RL.start_rubric("x", CRIT)
    assert st.get("scout_brief", "") == ""
    assert "GROUNDING" not in RL.next_action(st)["prompt"]


def test_scout_delta_measures_from_trajectories(tmp_path):
    p = tmp_path / "t.jsonl"
    rows = [
        {"task": "a", "criteria": CRIT, "steps": [], "n_iters": 3, "reward": 1, "passed": True, "scout": False},
        {"task": "b", "criteria": CRIT, "steps": [], "n_iters": 3, "reward": 1, "passed": True, "scout": False},
        {"task": "c", "criteria": CRIT, "steps": [], "n_iters": 1, "reward": 1, "passed": True, "scout": True},
        {"task": "d", "criteria": CRIT, "steps": [], "n_iters": 2, "reward": 1, "passed": True, "scout": True},
    ]
    p.write_text("\n".join(json.dumps(r) for r in rows) + "\n", encoding="utf-8")
    d = SC.scout_delta(p)
    assert d["with_scout"]["n"] == 2 and d["without_scout"]["n"] == 2
    assert d["with_scout"]["avg_iters"] == 1.5 and d["without_scout"]["avg_iters"] == 3.0
    assert d["delta_iters"] == 1.5                 # scout ahorro 1.5 iter (en estos datos)
