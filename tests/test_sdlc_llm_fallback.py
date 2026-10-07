"""D13: cuando el reasoner agota el budget razonando, `sdlc.llm` reintenta una vez con CODER.

Bug 2026-09-28 (SSB F3b): providers escribe "respuesta vacía" con tilde y sdlc buscaba "respuesta vacia";
el fallback nunca disparaba y build_feature moria en reasoner_rounds. El error se genera con el
`providers.call` real (cliente falso), asi un cambio de redaccion en providers rompe este test.
Cero red.
"""
import types

import pytest

import mmorch.providers as PV
import mmorch.sdlc as S


def _error_real_de_providers(monkeypatch) -> RuntimeError:
    ch = types.SimpleNamespace(message=types.SimpleNamespace(content=""), finish_reason="length")
    resp = types.SimpleNamespace(choices=[ch], usage=types.SimpleNamespace(prompt_tokens=10, completion_tokens=20))
    client = types.SimpleNamespace(chat=types.SimpleNamespace(
        completions=types.SimpleNamespace(create=lambda **kw: resp)))
    monkeypatch.setattr(PV, "log_event", lambda **rec: None)
    monkeypatch.setattr(PV, "_client", lambda mk: client)
    with pytest.raises(RuntimeError) as ei:
        PV.call("deepseek-reasoner", "hola", max_tokens=32768)
    return ei.value


@pytest.fixture
def corrida(monkeypatch):
    monkeypatch.setattr(S, "CODER", "deepseek-v4-pro")
    monkeypatch.setattr(S, "PHASE", "sdlc-test")
    monkeypatch.setattr(S, "CFG", {"usd_max": 100})
    monkeypatch.setattr(S, "ledger_usd", lambda: 0)
    monkeypatch.setattr(S, "state", {"calls": 0, "gates": [], "gate_rejects": []})


def test_llm_reintenta_con_coder_ante_respuesta_vacia(monkeypatch, corrida):
    err = _error_real_de_providers(monkeypatch)
    modelos = []

    def call_falso(model, messages, **kw):
        modelos.append(model)
        if model == "deepseek-reasoner":
            raise err
        return types.SimpleNamespace(text="ok del coder")

    monkeypatch.setattr(S, "call", call_falso)
    assert S.llm("deepseek-reasoner", "sys", "user") == "ok del coder"
    assert modelos == ["deepseek-reasoner", "deepseek-v4-pro"]
    assert S.state["gate_rejects"][0]["gate"] == "presupuesto-razonamiento"


def test_llm_no_reintenta_si_el_coder_tambien_falla(monkeypatch, corrida):
    err = _error_real_de_providers(monkeypatch)

    def call_falso(model, messages, **kw):
        raise err

    monkeypatch.setattr(S, "call", call_falso)
    with pytest.raises(RuntimeError, match="respuesta vac"):
        S.llm("deepseek-v4-pro", "sys", "user")
