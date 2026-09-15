"""canal: hilo JSONL entre agentes. Cero red."""
from pathlib import Path

from mmorch.canal import post, read


def test_post_read_roundtrip(tmp_path: Path):
    p = tmp_path / "canal.jsonl"
    a = post("cursor", "status", "left tests unrun", artifacts=["mmorch/canal.py"], path=p)
    b = post("claude", "ask", "did canal land?", to="cursor", path=p)
    got = read(10, path=p)
    assert [t["id"] for t in got] == [a["id"], b["id"]]
    assert got[0]["artifacts"] == ["mmorch/canal.py"]
    assert got[1]["to"] == "cursor"


def test_rejects_bad_src(tmp_path: Path):
    try:
        post("codex", "status", "no", path=tmp_path / "c.jsonl")
    except ValueError as e:
        assert "src" in str(e)
    else:
        raise AssertionError("expected ValueError")


def test_read_empty(tmp_path: Path):
    assert read(5, path=tmp_path / "missing.jsonl") == []
