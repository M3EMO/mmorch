"""session_skills: mina playbooks (secuencias de tool-calls recurrentes con tasa de
exito real) de sesiones. 100% local; label externo via outcome_of; solo recurrentes."""
import sys, pathlib, json, importlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
SS = importlib.import_module("mmorch.session_skills")
WObs = SS.WorkflowObs


def _sess(tmp_path, events, name="s.jsonl"):
    p = tmp_path / name
    p.write_text("\n".join(json.dumps(e) for e in events), encoding="utf-8")
    return p


def _seg(request, tool_names, result_content):
    # un segmento: request + tool_uses + un tool_result final.
    blocks = [{"type": "tool_use", "name": n, "input": {}} for n in tool_names]
    return [
        {"type": "user", "message": {"role": "user", "content": request}},
        {"type": "assistant", "message": {"role": "assistant", "content": blocks}},
        {"type": "user", "message": {"role": "user", "content": [
            {"type": "tool_result", "content": result_content, "is_error": False}]}},
    ]


def test_extract_only_segments_with_outcome_and_tools(tmp_path):
    ev = _seg("arregla", ["Read", "Edit"], "3 passed")           # tiene outcome + tools
    ev += [{"type": "user", "message": {"role": "user", "content": "contame algo"}},
           {"type": "assistant", "message": {"role": "assistant", "content": [
               {"type": "text", "text": "bla"}]}}]               # sin tools ni outcome
    obs = SS.extract_workflows(_sess(tmp_path, ev))
    assert len(obs) == 1
    assert obs[0].tools == ("Read", "Edit") and obs[0].reward >= 0.8
    assert obs[0].domain == "complicated"   # 2 tool-calls


def test_mine_filters_one_offs(tmp_path):
    obs = [WObs("t", ("Read",), "clear", 1.0)]   # visto una sola vez
    assert SS.mine_playbooks(obs, min_observed=2) == []


def test_mine_aggregates_recurring_with_success_rate():
    obs = [
        WObs("t1", ("Read", "Edit"), "complicated", 1.0),
        WObs("t2", ("Read", "Edit"), "complicated", 0.0),   # misma seq, fallo
        WObs("t3", ("Read", "Edit"), "complicated", 1.0),
    ]
    books = SS.mine_playbooks(obs, min_observed=2)
    assert len(books) == 1
    b = books[0]
    assert b.tool_sequence == ("Read", "Edit") and b.n_observed == 3
    assert b.n_success == 2 and b.success_rate == 0.667


def test_ingest_is_idempotent_and_persists(tmp_path):
    store = tmp_path / "obs.jsonl"
    led = tmp_path / "led.txt"
    p = _sess(tmp_path, _seg("arregla", ["Read", "Edit"], "3 passed"))
    n1 = SS.ingest_workflows(p, store=store, ledger=led)
    n2 = SS.ingest_workflows(p, store=store, ledger=led)   # misma sesion
    assert n1 == 1 and n2 == 0
    assert len(SS.load_observations(store)) == 1


def test_grown_session_ingests_new_without_duplicating(tmp_path):
    # mmorch verify: una sesion append-only que CRECE ingiere SOLO los segmentos nuevos
    # (idempotencia incremental por sessionId), sin duplicar lo previo ni perder lo nuevo.
    store = tmp_path / "obs.jsonl"
    led = tmp_path / "led.txt"
    ev = [{"type": "user", "message": {"role": "user", "content": "x"}, "sessionId": "SID-123"}]
    ev += _seg("arregla", ["Read", "Edit"], "3 passed")
    p = tmp_path / "grow.jsonl"
    p.write_text("\n".join(json.dumps(e) for e in ev), encoding="utf-8")
    n1 = SS.ingest_workflows(p, store=store, ledger=led)
    # la sesion crece: se agrega un segmento nuevo, MISMO sessionId
    ev += _seg("otra", ["Bash"], "5 passed")
    p.write_text("\n".join(json.dumps(e) for e in ev), encoding="utf-8")
    n2 = SS.ingest_workflows(p, store=store, ledger=led)
    assert n1 == 1 and n2 == 1                        # ingiere lo nuevo, no lo previo
    obs = SS.load_observations(store)
    assert len(obs) == 2                              # sin duplicados ni perdidas
    seqs = sorted(o.tools for o in obs)
    assert seqs == [("Bash",), ("Read", "Edit")]
    # re-ingerir sin crecimiento -> nada nuevo
    assert SS.ingest_workflows(p, store=store, ledger=led) == 0


def test_top_playbooks_filters_by_domain(tmp_path):
    store = tmp_path / "obs.jsonl"
    # dos sesiones distintas con la misma secuencia 'complicated' -> recurrente.
    for nm in ("a.jsonl", "b.jsonl"):
        p = _sess(tmp_path, _seg("x", ["Read", "Edit"], "3 passed"), name=nm)
        SS.ingest_workflows(p, store=store, ledger=tmp_path / (nm + ".led"))
    top = SS.top_playbooks(store=store, domain="complicated", min_observed=2)
    assert len(top) == 1 and top[0].tool_sequence == ("Read", "Edit")
    assert SS.top_playbooks(store=store, domain="clear", min_observed=2) == []
