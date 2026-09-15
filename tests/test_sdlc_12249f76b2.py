"""Test de aceptacion para mmorch/canal.py::post().

Contrato:
R1: post() con body vacio ("") lanza ValueError('body vacio').
R2: post() con body solo espacios ("   ") lanza ValueError('body vacio').
R3: un body vacio no agrega ninguna linea al canal, aunque el canal ya tenga historia (falla antes de escribir).
R4: un body con texto sigue funcionando igual (se escribe el registro).
"""
from __future__ import annotations

import pytest

from mmorch.canal import post


def test_R1_body_vacio_lanza_valueerror(tmp_path):
    p = tmp_path / "canal.jsonl"
    with pytest.raises(ValueError, match="body vacio"):
        post("cursor", "status", "", path=p)


def test_R2_body_solo_espacios_lanza_valueerror(tmp_path):
    p = tmp_path / "canal.jsonl"
    with pytest.raises(ValueError, match="body vacio"):
        post("cursor", "status", "   ", path=p)


def test_R3_body_vacio_no_agrega_linea(tmp_path):
    # grilling 2026-09-15: el caso real es un canal con historia; un body vacio no puede sumar una linea
    p = tmp_path / "canal.jsonl"
    post("cursor", "status", "ping previo", path=p)
    antes = p.read_text(encoding="utf-8").count("\n")
    with pytest.raises(ValueError):
        post("cursor", "status", "  ", path=p)
    assert p.read_text(encoding="utf-8").count("\n") == antes


def test_R4_body_con_texto_sigue_funcionando(tmp_path):
    p = tmp_path / "canal.jsonl"
    rec = post("cursor", "status", "ping", path=p)
    assert rec["body"] == "ping"
    assert p.exists()
