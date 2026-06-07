"""Invariantes de providers: observabilidad (H-2), error-logging, key gating. Client mockeado."""
import sys, pathlib, types
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import pytest
import mmorch.providers as PV


class _FakeUsage:
    prompt_tokens = 10
    completion_tokens = 20


class _FakeResp:
    def __init__(self, text="hi"):
        msg = types.SimpleNamespace(content=text)
        self.choices = [types.SimpleNamespace(message=msg)]
        self.usage = _FakeUsage()


class _FakeClient:
    def __init__(self, boom=False):
        self._boom = boom
        self.last_kwargs = None
        self.chat = types.SimpleNamespace(
            completions=types.SimpleNamespace(create=self._create))

    def _create(self, **kw):
        self.last_kwargs = kw
        if self._boom:
            raise RuntimeError("api 500")
        return _FakeResp()


@pytest.fixture
def captured_events(monkeypatch):
    events = []
    monkeypatch.setattr(PV, "log_event", lambda **rec: events.append(rec))
    return events


def test_call_success_logs_metric(monkeypatch, captured_events):
    monkeypatch.setattr(PV, "_client", lambda mk: _FakeClient())
    r = PV.call("deepseek-chat", "hola", pattern="t")
    assert r.text == "hi" and r.in_tokens == 10 and r.out_tokens == 20
    assert len(captured_events) == 1 and "error" not in captured_events[0]


def test_call_api_failure_logs_error_and_reraises(monkeypatch, captured_events):
    # H-2: fallo de API DEBE loggear evento error + re-lanzar.
    monkeypatch.setattr(PV, "_client", lambda mk: _FakeClient(boom=True))
    with pytest.raises(RuntimeError, match="api 500"):
        PV.call("deepseek-chat", "hola")
    assert len(captured_events) == 1
    assert captured_events[0].get("error") == "RuntimeError"


def test_call_passes_timeout_and_maxtokens(monkeypatch, captured_events):
    fc = _FakeClient()
    monkeypatch.setattr(PV, "_client", lambda mk: fc)
    PV.call("deepseek-chat", "hola")  # defaults H-3/H-6
    assert fc.last_kwargs["timeout"] == 60.0
    assert fc.last_kwargs["max_tokens"] == 16384


def test_missing_key_raises(monkeypatch):
    monkeypatch.setattr(PV, "_OPENAI_OK", True)
    monkeypatch.delenv("DEEPSEEK_API_KEY", raising=False)
    monkeypatch.setattr(PV.os, "getenv", lambda k: None)
    with pytest.raises(PV.MissingKeyError):
        PV._client("deepseek-chat")
