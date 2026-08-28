"""W5.3 — matriz de riesgo por tool MCP (read|mutate|outward) + audit trail.

La matriz es OBLIGATORIA (tool sin declarar = RuntimeError a import-time); las
mutate sensibles y todo outward dejan rastro en logs/audit.jsonl. NO hay gate
HITL bloqueante — el trail es forensica, no permiso.
"""
import json
import sys, pathlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))

import pytest

from mmorch.mcp_server import (mcp, _AUDITED, _TOOL_RISK,
                               _audited, _registers_in_profile)

TOOLS = {t.name: t for t in mcp._tool_manager.list_tools()}
_VALID = {"read", "mutate", "outward"}


def test_matriz_cubre_exactamente_las_tools():
    # sin tools fantasma en la matriz ni tools registradas sin clasificar
    assert set(_TOOL_RISK) == set(TOOLS), (
        set(_TOOL_RISK) ^ set(TOOLS))
    assert set(_TOOL_RISK.values()) <= _VALID


def test_metadata_de_riesgo_en_cada_wrapper():
    for name, tool in TOOLS.items():
        assert getattr(tool.fn, "__mmorch_risk__", None) == _TOOL_RISK[name], name


def test_tool_sin_riesgo_declarado_revienta():
    from mmorch.mcp_server import _tool

    def mmorch_tool_fantasma():   # no esta en _TOOL_RISK
        return "x"
    with pytest.raises(RuntimeError, match="riesgo"):
        _tool(mmorch_tool_fantasma)


def test_core_excluye_outward_por_default():
    assert _TOOL_RISK["mmorch_evolve_nightly"] == "outward"
    # mecanismo generico: CUALQUIER outward queda fuera de core, este o futuro
    assert not _registers_in_profile("mmorch_hipotetica", "outward", "core")
    assert _registers_in_profile("mmorch_hipotetica", "outward", "full")
    # la lista curada previa (W2.2) sigue aplicando en core
    assert not _registers_in_profile("mmorch_ingest_session", "mutate", "core")
    assert _registers_in_profile("mmorch_fan_out", "read", "core")


def test_auditadas_son_sensibles():
    assert _AUDITED <= set(_TOOL_RISK)
    # read jamas se audita (ruido); todo outward SI se audita
    for name in _AUDITED:
        assert _TOOL_RISK[name] in ("mutate", "outward"), name
    outward = {n for n, r in _TOOL_RISK.items() if r == "outward"}
    assert outward <= _AUDITED


def _leer_audit(tmp_path):
    p = tmp_path / "logs" / "audit.jsonl"
    return [json.loads(ln) for ln in p.read_text(encoding="utf-8").splitlines()]


def test_audit_trail_escribe_ok_y_error(monkeypatch, tmp_path):
    # logs_dir() se resuelve al momento del call -> MMORCH_HOME del test aplica
    monkeypatch.setenv("MMORCH_HOME", str(tmp_path))

    def mmorch_fake_sensible(x: int = 0):
        if x < 0:
            raise ValueError("boom")
        return "ok"
    w = _audited(mmorch_fake_sensible, "mutate")
    assert w(x=1) == "ok"
    with pytest.raises(ValueError):
        w(x=-1)   # la excepcion SIGUE propagando (el contrato de error va por fuera)
    recs = _leer_audit(tmp_path)
    assert len(recs) == 2
    assert recs[0]["tool"] == "mmorch_fake_sensible" and recs[0]["ok"] is True
    assert recs[0]["risk"] == "mutate" and '"x": "1"' in recs[0]["args"]
    assert recs[1]["ok"] is False
