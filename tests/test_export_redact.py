"""El export del flywheel redacta secretos antes de escribir training/."""

import json

import scripts.export_training_data as E


def test_write_redacta_strings_anidados(tmp_path, monkeypatch):
    monkeypatch.setattr(E, "OUT", tmp_path)
    row = {"input": "usa sk-ABCDEF0123456789abcdef0123", "meta": {"paths": [r"C:\Users\map12\x"]}, "n": 3}
    assert E._write("t.jsonl", [row]) == 1
    texto = (tmp_path / "t.jsonl").read_text(encoding="utf-8")
    assert "sk-ABCDEF" not in texto and "map12" not in texto
    assert json.loads(texto)["n"] == 3
