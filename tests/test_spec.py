"""spec-builder gate: la inferencia generosa se aplica conservadora.
Invariante anti-sobrepaso: solo SAFE entra al spec; BEYOND_INTENT y verdict ausente
caen a open_questions (NUNCA al spec); WRONG se descarta."""
import sys, pathlib, importlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
S = importlib.import_module("mmorch.spec")


def _stub(monkeypatch, inferences, labels, spec_overreach=()):
    """Mockea _draft y _critique. labels mas corto que inferences simula verdict
    ausente. spec_overreach simula sobrepaso colado en el cuerpo del spec."""
    monkeypatch.setattr(S, "_draft", lambda raw, ans, *, model, phase: {
        "spec": "SPEC BASE", "inferences": list(inferences),
        "open_questions": ["pregunta original del draft"], "_cost_usd": 0.001})
    monkeypatch.setattr(S, "_critique", lambda raw, ans, infs, spec_text, *, gen_model, verifier_model, phase: {
        "verdicts": [{"label": l} for l in labels],
        "spec_overreach": list(spec_overreach), "_cost_usd": 0.001})


def test_safe_enters_spec(monkeypatch):
    _stub(monkeypatch, ["usar SQLite"], ["SAFE"])
    r = S.build_spec("guarda datos")
    assert r.accepted_inferences == ["usar SQLite"]
    assert "usar SQLite" in r.spec and "Inferencias aceptadas" in r.spec
    assert "usar SQLite" not in r.open_questions


def test_beyond_intent_never_enters_spec(monkeypatch):
    # el caso central: inferencia plausible pero no pedida -> pregunta, no se aplica.
    _stub(monkeypatch, ["agregar auth con OAuth"], ["BEYOND_INTENT"])
    r = S.build_spec("hace un CRUD")
    assert "agregar auth con OAuth" in r.open_questions
    assert "agregar auth con OAuth" not in r.spec
    assert r.accepted_inferences == []


def test_wrong_is_dropped(monkeypatch):
    _stub(monkeypatch, ["el usuario quiere Postgres"], ["WRONG"])
    r = S.build_spec("usa SQLite")
    assert r.dropped == ["el usuario quiere Postgres"]
    assert "Postgres" not in r.spec and "el usuario quiere Postgres" not in r.open_questions


def test_missing_verdict_is_conservative(monkeypatch):
    # 2 inferencias, 1 solo verdict -> la 2da cae a BEYOND_INTENT (conservador).
    _stub(monkeypatch, ["A", "B"], ["SAFE"])
    r = S.build_spec("x")
    assert "A" in r.accepted_inferences and "B" in r.open_questions


def test_gross_overreach_escalates(monkeypatch):
    # 2 de 3 no-SAFE -> 0.66 >= 0.5 -> escalate.
    _stub(monkeypatch, ["A", "B", "C"], ["SAFE", "BEYOND_INTENT", "WRONG"])
    r = S.build_spec("x")
    assert r.escalate


def test_no_inferences_no_escalate(monkeypatch):
    _stub(monkeypatch, [], [])
    r = S.build_spec("trivial")
    assert not r.escalate and r.spec == "SPEC BASE"


def test_spec_body_overreach_escalates(monkeypatch):
    # gap que encontro el dogfood cross-family: el drafter cola sobrepaso en el cuerpo
    # del spec, salteando el canal de inferencias. El critico lo caza -> escala + pregunta.
    _stub(monkeypatch, [], [], spec_overreach=["asume que la DB es Postgres"])
    r = S.build_spec("guarda datos")
    assert r.escalate and r.quarantined
    assert any("Postgres" in q for q in r.open_questions)
    # cuarentena: el spec NO se devuelve usable; el draft sucio queda aparte.
    assert r.spec == "" and r.raw_draft == "SPEC BASE"


def test_critique_refuses_same_family():
    # OneFlow: drafter y critico misma familia (deepseek) -> ValueError antes de llamar.
    import pytest
    with pytest.raises(ValueError):
        S._critique("x", "", ["inf"], "SPEC", gen_model="deepseek-chat",
                    verifier_model="deepseek-reasoner", phase="t")
