"""D14 (cableo aprobado 2026-09-14): `schedule` y `effort` entran a `providers.call`, automatico y recurrente.

Contrato:
- R1 `call(..., effort="low"|"med"|"high")`: si `effort` viene, el modelo es `effort.model_for_effort(effort)`
  y `model_key` se ignora; el registro del ledger lleva `model` = ese modelo.
- R2 sin `effort`, `call` se comporta exactamente como hoy.
- R3 cada registro del ledger (exito y error) lleva en `extra` la clave `off_peak` (bool) tomada de
  `schedule.is_off_peak()` en el momento de la llamada: asi `schedule.spend_by_period()` mide el ahorro real.
Cero red: cliente falso y `is_off_peak` parcheado.
"""
import types

import pytest

import mmorch.providers as PV
import mmorch.schedule as S


class _Resp:
    def __init__(self):
        self.choices = [types.SimpleNamespace(message=types.SimpleNamespace(content="hi"), finish_reason="stop")]
        self.usage = types.SimpleNamespace(prompt_tokens=10, completion_tokens=20)


class _Fake:
    def __init__(self, boom=False):
        self.boom = boom
        self.last = None
        self.chat = types.SimpleNamespace(completions=types.SimpleNamespace(create=self._create))

    def _create(self, **kw):
        self.last = kw
        if self.boom:
            raise RuntimeError("api 500")
        return _Resp()


@pytest.fixture
def events(monkeypatch):
    out = []
    monkeypatch.setattr(PV, "log_event", lambda **rec: out.append(rec))
    monkeypatch.setattr(PV, "_client", lambda mk: _Fake())
    return out


def test_R1_effort_elige_el_modelo(events):
    r = PV.call("deepseek-chat", "hola", pattern="t", effort="high")
    assert r.text == "hi"
    assert events[-1]["model"] == "deepseek-v4-pro", events[-1]


def test_R2_sin_effort_no_cambia(events):
    PV.call("deepseek-chat", "hola", pattern="t")
    assert events[-1]["model"] == "deepseek-chat", events[-1]


def test_R3_off_peak_en_el_ledger(events, monkeypatch):
    monkeypatch.setattr(S, "is_off_peak", lambda now=None: True)
    PV.call("deepseek-chat", "hola", pattern="t")
    assert events[-1].get("extra", {}).get("off_peak") is True, events[-1]
    monkeypatch.setattr(S, "is_off_peak", lambda now=None: False)
    PV.call("deepseek-chat", "hola", pattern="t")
    assert events[-1].get("extra", {}).get("off_peak") is False, events[-1]


def test_R3b_off_peak_tambien_en_error(monkeypatch):
    out = []
    monkeypatch.setattr(PV, "log_event", lambda **rec: out.append(rec))
    monkeypatch.setattr(PV, "_client", lambda mk: _Fake(boom=True))
    monkeypatch.setattr(S, "is_off_peak", lambda now=None: True)
    with pytest.raises(RuntimeError):
        PV.call("deepseek-chat", "hola", pattern="t")
    assert out and out[-1].get("extra", {}).get("off_peak") is True, out
