"""iohelpers: los 2 patrones compartidos por el audit 2026-08 (write atomico + load
tolerante) — un test chico por patron, no por archivo que los usa."""
import sys, pathlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import mmorch.iohelpers as IO


def test_atomic_write_survives_a_tmp_file_and_leaves_only_the_target(tmp_path):
    p = tmp_path / "state.json"
    IO.atomic_write_json(p, {"a": 1})
    assert IO.load_json_tolerant(p, None) == {"a": 1}
    assert not (tmp_path / "state.json.tmp").exists()   # no dejo basura a mitad de write


def test_load_tolerant_distinguishes_missing_from_corrupt(tmp_path, caplog):
    missing = tmp_path / "missing.json"
    assert IO.load_json_tolerant(missing, {"default": True}) == {"default": True}
    assert "corrupto" not in caplog.text   # no-existe = default legitimo, sin log

    corrupt = tmp_path / "corrupt.json"
    corrupt.write_text("{not json", encoding="utf-8")
    import logging
    caplog.set_level(logging.ERROR)
    assert IO.load_json_tolerant(corrupt, {"default": True}) == {"default": True}
    assert "corrupto" in caplog.text       # no-parsea = señal fuerte, no silencio


def test_read_jsonl_tolerant_skips_only_the_torn_line(tmp_path):
    p = tmp_path / "log.jsonl"
    p.write_text('{"a":1}\nnot json at all\n{"b":2}\n', encoding="utf-8")
    assert IO.read_jsonl_tolerant(p) == [{"a": 1}, {"b": 2}]
