"""Tests de manana.py — solo la logica pura (_tren_rojo_de_anoche); el resto
del script es interactivo (input()), no unitariamente testeable."""

import importlib.util
import json
import sys
from pathlib import Path

_SPEC = importlib.util.spec_from_file_location(
    "manana", Path(__file__).resolve().parents[1] / "scripts" / "manana.py")
manana = importlib.util.module_from_spec(_SPEC)
sys.modules["manana"] = manana
_SPEC.loader.exec_module(manana)


def test_tren_rojo_sin_log_devuelve_vacio(tmp_path, monkeypatch):
    monkeypatch.setattr(manana, "ROOT", tmp_path)
    assert manana._tren_rojo_de_anoche() == set()


def test_tren_rojo_gate_verde_devuelve_vacio(tmp_path, monkeypatch):
    monkeypatch.setattr(manana, "ROOT", tmp_path)
    (tmp_path / "logs").mkdir()
    (tmp_path / "logs" / "merge_train.jsonl").write_text(
        json.dumps({"gate": "verde", "merged": ["a", "b"]}) + "\n",
        encoding="utf-8")
    assert manana._tren_rojo_de_anoche() == set()


def test_tren_rojo_gate_rojo_devuelve_las_ramas(tmp_path, monkeypatch):
    monkeypatch.setattr(manana, "ROOT", tmp_path)
    (tmp_path / "logs").mkdir()
    (tmp_path / "logs" / "merge_train.jsonl").write_text(
        json.dumps({"gate": "verde", "merged": ["viejo"]}) + "\n"
        + json.dumps({"gate": "rojo", "merged": ["mmorch-sbx-a", "mmorch-sbx-b"]}) + "\n",
        encoding="utf-8")
    assert manana._tren_rojo_de_anoche() == {"mmorch-sbx-a", "mmorch-sbx-b"}


def test_tren_rojo_gate_rojo_sin_merged_devuelve_vacio(tmp_path, monkeypatch):
    """gate=rojo pero merged=[] (nada se llego a unir) -> no hay conflicto
    entre partes que avisar, es un caso distinto (ver mmorch/merge_train.py)."""
    monkeypatch.setattr(manana, "ROOT", tmp_path)
    (tmp_path / "logs").mkdir()
    (tmp_path / "logs" / "merge_train.jsonl").write_text(
        json.dumps({"gate": "rojo", "merged": []}) + "\n", encoding="utf-8")
    assert manana._tren_rojo_de_anoche() == set()
