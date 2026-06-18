import sys, pathlib, json, importlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
S = importlib.import_module("mmorch.sessions")


def _write_jsonl(tmp_path, events):
    p = tmp_path / "sess.jsonl"
    p.write_text("\n".join(json.dumps(e) for e in events), encoding="utf-8")
    return p


def test_parse_splits_segments_and_extracts_blocks(tmp_path):
    events = [
        {"type": "user", "message": {"role": "user", "content": "arregla el bug"}},
        {"type": "assistant", "message": {"role": "assistant", "content": [
            {"type": "text", "text": "miro el archivo"},
            {"type": "tool_use", "name": "Edit", "input": {"file_path": "a.py"}}]}},
        {"type": "user", "message": {"role": "user", "content": [
            {"type": "tool_result", "content": "ok", "is_error": False}]}},
        {"type": "user", "message": {"role": "user", "content": "ahora corre tests"}},
        {"type": "assistant", "message": {"role": "assistant", "content": [
            {"type": "tool_use", "name": "Bash", "input": {"command": "pytest"}}]}},
    ]
    segs = S.parse_session(_write_jsonl(tmp_path, events))
    assert len(segs) == 2
    assert segs[0].request == "arregla el bug"
    assert segs[0].reasoning == "miro el archivo"
    assert segs[0].tool_calls == [{"name": "Edit", "input": {"file_path": "a.py"}}]
    assert segs[0].tool_results == [{"content": "ok", "is_error": False}]
    assert segs[1].request == "ahora corre tests"
    assert segs[1].tool_calls[0]["name"] == "Bash"


def test_malformed_line_is_skipped_not_fatal(tmp_path):
    # archivos reales son 1MB+; una linea corrupta no debe matar el parse (mmorch verify T1).
    p = tmp_path / "sess.jsonl"
    p.write_text('{"type": "user", "message": {"role": "user", "content": "hola"}}\n'
                 'esto no es json valido {{{\n'
                 '{"type": "assistant", "message": {"role": "assistant", "content": '
                 '[{"type": "text", "text": null}]}}\n', encoding="utf-8")
    segs = S.parse_session(p)
    assert len(segs) == 1 and segs[0].request == "hola"
    assert segs[0].reasoning == ""   # text=null no inserta "None"
