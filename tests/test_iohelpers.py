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


# ---- read_jsonl_cached (ticket 13, audit-2026-08) --------------------------------------
def test_read_jsonl_cached_hit_returns_same_object_without_reread(tmp_path):
    p = tmp_path / "cached.jsonl"
    p.write_text('{"a":1}\n{"b":2}\n', encoding="utf-8")
    first = IO.read_jsonl_cached(p)
    assert first == [{"a": 1}, {"b": 2}]
    second = IO.read_jsonl_cached(p)
    assert second is first   # cache hit: mismo mtime/size -> mismo objeto, sin re-leer


def test_read_jsonl_cached_invalidates_on_append(tmp_path):
    p = tmp_path / "cached2.jsonl"
    p.write_text('{"a":1}\n', encoding="utf-8")
    first = IO.read_jsonl_cached(p)
    assert first == [{"a": 1}]
    with open(p, "a", encoding="utf-8") as fh:
        fh.write('{"b":2}\n')
    second = IO.read_jsonl_cached(p)
    assert second == [{"a": 1}, {"b": 2}]
    assert second is not first   # append cambió mtime/size -> re-leído


def test_read_jsonl_cached_equivalent_to_tolerant(tmp_path):
    p = tmp_path / "cached3.jsonl"
    p.write_text('{"a":1}\nnot json\n{"b":2}\n', encoding="utf-8")
    assert IO.read_jsonl_cached(p) == IO.read_jsonl_tolerant(p)


# ---- read_jsonl_tail (ticket 13) --------------------------------------------------------
def test_read_jsonl_tail_returns_only_the_last_n_lines(tmp_path):
    p = tmp_path / "tail.jsonl"
    p.write_text("".join(f'{{"i":{i}}}\n' for i in range(50)), encoding="utf-8")
    tail = IO.read_jsonl_tail(p, 3)
    assert tail == [{"i": 47}, {"i": 48}, {"i": 49}]


def test_read_jsonl_tail_shorter_than_n_returns_everything(tmp_path):
    p = tmp_path / "tail2.jsonl"
    p.write_text('{"a":1}\n{"b":2}\n', encoding="utf-8")
    assert IO.read_jsonl_tail(p, 200) == [{"a": 1}, {"b": 2}]


def test_read_jsonl_tail_equivalent_to_full_read_slice(tmp_path):
    p = tmp_path / "tail3.jsonl"
    rows = [f'{{"i":{i}}}\n' for i in range(30)]
    p.write_text("".join(rows), encoding="utf-8")
    n = 7
    assert IO.read_jsonl_tail(p, n) == IO.read_jsonl_tolerant(p)[-n:]


def test_read_jsonl_tail_missing_file(tmp_path):
    assert IO.read_jsonl_tail(tmp_path / "missing.jsonl", 10) == []
