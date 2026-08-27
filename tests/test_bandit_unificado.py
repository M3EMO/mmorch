"""W4.3 — bandit plano zombie unificado: bandit_state.json fuera del código; el único
estado es el sig-bandit (bandit_sig.json). La vía MCP entrena lo MISMO que la librería
(record_outcome -> intuition.record via context), sin doble-train plano, y
feedback_stats reporta el bandit real."""
import json
from pathlib import Path

import mmorch.feedback as FB
from mmorch.intuition import _SIG_BANDIT as INTUITION_PATH, _arm as sig_arm

REPO = Path(__file__).resolve().parent.parent


def test_default_bandit_es_el_sig_bandit():
    # una sola definición del path, compartida feedback<->intuition
    assert FB._SIG_BANDIT == INTUITION_PATH
    assert FB._SIG_BANDIT.name == "bandit_sig.json"
    assert FB.ThompsonBandit().path == FB._SIG_BANDIT


def test_bandit_state_json_borrado_del_codigo():
    # ratchet: el archivo zombie no puede volver a nombrarse desde mmorch/
    ofensores = [f.name for f in (REPO / "mmorch").glob("*.py")
                 if "bandit_state" in f.read_text(encoding="utf-8")]
    assert ofensores == [], f"bandit_state.json reaparecio en: {ofensores}"


def test_record_outcome_entrena_el_sig_bandit(tmp_path):
    # la librería: outcome con context -> el sig-bandit aprende en el arm model#sig
    b = FB.ThompsonBandit(path=tmp_path / "sig.json")
    import mmorch.intuition as IT
    arm = IT.record("deepseek-chat", 1.0, "sumar dos enteros", bandit=b)
    assert "#" in arm and arm.startswith("deepseek-chat#")
    assert b.stats()[arm]["n"] == 1


def test_mcp_record_outcome_sin_doble_train(monkeypatch):
    """La vía MCP delega TODO el aprendizaje en la librería (feedback.record_outcome
    -> intuition.record): no debe además updatear un bandit plano por su cuenta."""
    import mmorch.mcp_server as srv

    recorded = {}

    class _FakeOutcome:
        arm, reward = "deepseek-chat", 1.0

    def fake_record(arm, reward, **kw):
        recorded.update(arm=arm, reward=reward, **kw)
        return _FakeOutcome()

    class _FakeBandit:
        def __init__(self, *a, **k):
            pass

        def update(self, *a, **k):
            raise AssertionError("la via MCP NO debe entrenar un bandit aparte (doble-train)")

        def stats(self):
            return {sig_arm("deepseek-chat", "sumar dos enteros"): {"mean": 0.9, "n": 3}}

    monkeypatch.setattr(srv, "_record_outcome", fake_record)
    monkeypatch.setattr(srv, "_ThompsonBandit", _FakeBandit)
    # W5.1: el readback del posterior vive en la libreria (intuition.arm_stats);
    # el wrapper solo adapta — el fake confirma que reporta ese valor tal cual
    import mmorch.intuition as IT
    monkeypatch.setattr(IT, "arm_stats",
                        lambda arm, task, **k: _FakeBandit().stats()[sig_arm(arm, task)])
    out = json.loads(srv.mmorch_record_outcome(
        "deepseek-chat", 1.0, context="sumar dos enteros"))
    assert out["recorded"] and recorded["context"] == "sumar dos enteros"
    # reporta el posterior del arm sig-keyed REAL (no el plano)
    assert out["bandit"] == {"mean": 0.9, "n": 3}


def test_mcp_record_outcome_sin_context_no_inventa_bandit(monkeypatch):
    import mmorch.mcp_server as srv

    class _FakeOutcome:
        arm, reward = "x", 0.0

    monkeypatch.setattr(srv, "_record_outcome", lambda *a, **k: _FakeOutcome())
    out = json.loads(srv.mmorch_record_outcome("x", 0.0))
    assert out["bandit"] == {}   # sin context no hay signature -> nada que reportar
