"""enrich: completar prompts infiriendo intent (Fable 5), con juez cross-family que dropea
assumptions inventadas. Roles separados; gen/judge inyectados (cero API)."""
import sys, pathlib, json, importlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import mmorch.enrich as EN
RL = importlib.import_module("mmorch.rubric_loop")


GEN_OUT = json.dumps({
    "enriched": "texto libre del enricher",
    "assumptions": ["a0 razonable", "a1 INVENTADO", "a2 razonable"],
    "questions": ["que formato de salida?"],
})


def test_judge_drops_overreaching_assumptions():
    gen = lambda p: GEN_OUT
    judge = lambda p: json.dumps([{"i": 0, "keep": True}, {"i": 1, "keep": False},
                                  {"i": 2, "keep": True}])
    r = EN.enrich_prompt("hace una func", gen_fn=gen, judge_fn=judge,
                         gen_model="deepseek-chat", judge_model="gemini-3.1-flash-lite")
    assert r["assumptions"] == ["a0 razonable", "a2 razonable"]
    assert r["rejected"] == ["a1 INVENTADO"]
    assert "a1 INVENTADO" not in r["enriched"]      # el inventado NO entra al prompt final
    assert "a0 razonable" in r["enriched"] and "que formato" in r["enriched"]
    assert r["enriched"].startswith("hace una func")  # original preservado al frente


def test_oneflow_same_family_rejected():
    try:
        EN.enrich_prompt("x", gen_model="deepseek-chat", judge_model="deepseek-reasoner",
                         gen_fn=lambda p: "{}", judge_fn=lambda p: "[]")
        assert False, "debio rechazar gen/judge misma familia"
    except ValueError as e:
        assert "OneFlow" in str(e)


def test_judge_refutes_all_on_garbage():
    gen = lambda p: GEN_OUT
    judge = lambda p: "no es json"          # juez ilegible -> nadie keep (refute by default)
    r = EN.enrich_prompt("x", gen_fn=gen, judge_fn=judge,
                         gen_model="deepseek-chat", judge_model="gemini-3.1-flash-lite")
    assert r["assumptions"] == [] and len(r["rejected"]) == 3


def test_no_assumptions_no_judge_call():
    gen = lambda p: json.dumps({"enriched": "e", "assumptions": [], "questions": ["q?"]})
    def judge(p):
        raise AssertionError("juez NO debe llamarse sin assumptions")
    r = EN.enrich_prompt("x", gen_fn=gen, judge_fn=judge,
                         gen_model="deepseek-chat", judge_model="gemini-3.1-flash-lite")
    assert r["assumptions"] == [] and "q?" in r["enriched"]


def test_rubric_enrich_flag_and_task_rewrite(monkeypatch):
    monkeypatch.setattr("mmorch.enrich.enrich_prompt",
                        lambda task, **k: {"enriched": "TASK ENRIQUECIDA"})
    crit = [{"id": "c1", "desc": "x", "kind": "checkable", "checker": "python_exec", "ctx": {"code": "print(1)"}}]
    st = RL.start_rubric("crudo", crit, enrich=True)
    assert st["enriched"] is True and st["task"] == "TASK ENRIQUECIDA"
    st2 = RL.start_rubric("crudo", crit)
    assert st2["enriched"] is False and st2["task"] == "crudo"


def test_enrich_delta_measures(tmp_path):
    p = tmp_path / "t.jsonl"
    rows = [
        {"task": "a", "criteria": [], "steps": [], "n_iters": 3, "passed": True, "enriched": False},
        {"task": "b", "criteria": [], "steps": [], "n_iters": 1, "passed": True, "enriched": True},
    ]
    p.write_text("\n".join(json.dumps(r) for r in rows) + "\n", encoding="utf-8")
    d = EN.enrich_delta(p)
    assert d["with_enrich"]["avg_iters"] == 1.0 and d["without_enrich"]["avg_iters"] == 3.0
    assert d["delta_iters"] == 2.0
