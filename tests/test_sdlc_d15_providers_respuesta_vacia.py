"""D15 (robustez de `providers.call`, hallazgo 2026-09-14): la API devolvio un objeto sin `choices`
y `call` murio con TypeError ('NoneType' object is not subscriptable) en dos corridas del pipeline.

Contrato:
- R1 `resp.choices` None o vacio -> `call` levanta `RuntimeError` cuyo mensaje contiene "sin choices" (nunca TypeError).
- R2 ese fallo se registra en el ledger con error="EmptyResponse" y error_class="empty_response".
- R3 `usage` ausente (None) con choices validos NO rompe: in_tokens/out_tokens quedan en 0.
Cero red: cliente falso.
"""
import types

import pytest

import mmorch.providers as PV


class _Resp:
    def __init__(self, choices, usage="default"):
        self.choices = choices
        self.usage = types.SimpleNamespace(prompt_tokens=3, completion_tokens=4) if usage == "default" else usage


def _client(resp):
    return types.SimpleNamespace(chat=types.SimpleNamespace(completions=types.SimpleNamespace(create=lambda **kw: resp)))


@pytest.fixture
def events(monkeypatch):
    out = []
    monkeypatch.setattr(PV, "log_event", lambda **rec: out.append(rec))
    return out


@pytest.mark.parametrize("choices", [None, []])
def test_R1_R2_sin_choices_es_runtime_error_registrado(events, monkeypatch, choices):
    monkeypatch.setattr(PV, "_client", lambda mk: _client(_Resp(choices)))
    with pytest.raises(RuntimeError, match="sin choices"):
        PV.call("deepseek-chat", "hola", pattern="t", timeout=5)
    assert events and events[-1].get("error") == "EmptyResponse", events
    assert events[-1].get("error_class") == "empty_response", events


def test_R3_usage_ausente_no_rompe(events, monkeypatch):
    ok = _Resp([types.SimpleNamespace(message=types.SimpleNamespace(content="hi"), finish_reason="stop")], usage=None)
    monkeypatch.setattr(PV, "_client", lambda mk: _client(ok))
    r = PV.call("deepseek-chat", "hola", pattern="t", timeout=5)
    assert r.text == "hi" and r.in_tokens == 0 and r.out_tokens == 0
