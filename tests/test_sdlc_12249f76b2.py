"""Test de aceptacion para mmorch/canal.py::post().

Contrato:
R1: post() con body vacio ("") lanza ValueError('body vacio').
R2: post() con body solo espacios ("   ") lanza ValueError('body vacio').
R3: un body vacio no escribe nada en el archivo del canal (falla antes de escribir).
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


def test_R3_body_vacio_no_escribe_archivo(tmp_path):
    p = tmp_path / "canal.jsonl"
    with pytest.raises(ValueError):
        post("cursor", "status", "  ", path=p)
    assert not p.exists()


def test_R4_body_con_texto_sigue_funcionando(tmp_path):
    p = tmp_path / "canal.jsonl"
    rec = post("cursor", "status", "ping", path=p)
    assert rec["body"] == "ping"
    assert p.exists()
