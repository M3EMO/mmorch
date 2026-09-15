"""Revision del diff D14 contra docs/sdlc/spec.md (R3).

log_event(**extra) colecciona kwargs sueltos bajo la clave 'extra'. providers.call
pasa `extra={"off_peak": ...}` como UN kwarg mas -> el registro real persistido
queda con record['extra']['extra']['off_peak'], no record['extra']['off_peak']
como exige R3. El test de aceptacion existente no lo detecta porque mockea
log_event entero (mira los kwargs crudos, no lo que log_event realmente arma).
"""
import json
import types

import mmorch.metrics as M
import mmorch.providers as PV
import mmorch.schedule as S


class _Resp:
    def __init__(self):
        self.choices = [types.SimpleNamespace(message=types.SimpleNamespace(content="hi"), finish_reason="stop")]
        self.usage = types.SimpleNamespace(prompt_tokens=10, completion_tokens=20)


class _Fake:
    def __init__(self):
        self.chat = types.SimpleNamespace(completions=types.SimpleNamespace(create=self._create))

    def _create(self, **kw):
        return _Resp()


def test_off_peak_queda_directo_en_extra_del_registro_real(tmp_path, monkeypatch):
    monkeypatch.setattr(M, "_LOG_DIR", tmp_path)
    monkeypatch.setattr(M, "_LOG_PATH", tmp_path / "metrics.jsonl")
    monkeypatch.setattr(PV, "_client", lambda mk: _Fake())
    monkeypatch.setattr(S, "is_off_peak", lambda now=None: True)

    PV.call("deepseek-chat", "hola", pattern="t")

    lines = (tmp_path / "metrics.jsonl").read_text(encoding="utf-8").strip().splitlines()
    record = json.loads(lines[-1])
    assert record.get("extra", {}).get("off_peak") is True, record
