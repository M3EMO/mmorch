"""schema-gates: validator minimo + gated_json (retry-con-feedback, reject)."""
import sys, pathlib
from dataclasses import dataclass
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import mmorch.schema as S


@dataclass
class _Res:
    text: str
    cost_usd: float = 0.0


VERDICT = {
    "type": "object",
    "required": ["passed", "confidence"],
    "properties": {
        "passed": {"type": "boolean"},
        "confidence": {"type": "number"},
        "tier": {"enum": ["S", "A", "B"]},
        "refutations": {"type": "array", "items": {"type": "string"}},
    },
}


def test_validate_ok():
    assert S.validate({"passed": True, "confidence": 0.9}, VERDICT) == []


def test_validate_missing_required():
    e = S.validate({"passed": True}, VERDICT)
    assert any("confidence" in x for x in e)


def test_validate_bool_not_number():
    # bool no debe colar como number (bug clasico: bool es subclase de int).
    e = S.validate({"passed": True, "confidence": True}, VERDICT)
    assert any("boolean" in x for x in e)


def test_validate_enum_and_items():
    assert S.validate({"passed": True, "confidence": 1, "tier": "Z"}, VERDICT)
    assert S.validate({"passed": True, "confidence": 1,
                       "refutations": ["ok", 5]}, VERDICT)  # 5 no es string


def test_extract_json_fenced():
    d = S.extract_json('```json\n{"a": 1}\n```')
    assert d == {"a": 1}


def test_gated_json_retries_then_succeeds(monkeypatch):
    calls = {"n": 0}
    def _c(model, messages, **kw):
        calls["n"] += 1
        if calls["n"] == 1:
            return _Res("no es json", 0.001)              # falla -> retry
        return _Res('{"passed": true, "confidence": 0.8}', 0.001)
    monkeypatch.setattr(S, "call", _c)
    out = S.gated_json("deepseek-chat", [{"role": "user", "content": "x"}], schema=VERDICT)
    assert out["passed"] is True and calls["n"] == 2
    assert out["_cost_usd"] == 0.002  # costo acumulado de ambos intentos


def test_gated_json_raises_when_exhausted(monkeypatch):
    monkeypatch.setattr(S, "call", lambda *a, **k: _Res("nunca json", 0.001))
    import pytest
    with pytest.raises(S.SchemaGateError):
        S.gated_json("deepseek-chat", [{"role": "user", "content": "x"}],
                     schema=VERDICT, max_retries=1)
