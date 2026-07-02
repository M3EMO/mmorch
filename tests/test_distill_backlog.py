"""distill_backlog — the episodic->semantic bridge (measured gap: 176 episodes, 0 notes)."""
from types import SimpleNamespace

import mmorch.memory as M


def _seed(db, n=3):
    for i in range(n):
        M.write_episode("global", "outcome", f"episode {i}: model X solved task {i}", path=db)


def test_backlog_distills_and_verifies(tmp_path, monkeypatch):
    db = tmp_path / "mem.duckdb"
    _seed(db, 3)
    monkeypatch.setattr(M, "distill", lambda text, **kw: f"note for [{text[:12]}]")
    import mmorch.patterns as P
    monkeypatch.setattr(P, "adversarial_verify",
                        lambda *a, **kw: SimpleNamespace(passed=True, refutations=[]))
    out = M.distill_backlog(after_id=0, limit=10, path=db)
    assert out["seen"] == 3 and out["persisted"] == 3 and out["refuted"] == 0, out
    st = M.stats(path=db)
    assert st["semantic"] == 3 and st["verified"] == 3, st


def test_refuted_note_not_persisted_raw_survives(tmp_path, monkeypatch):
    db = tmp_path / "mem.duckdb"
    _seed(db, 2)
    monkeypatch.setattr(M, "distill", lambda text, **kw: "unfaithful note")
    import mmorch.patterns as P
    monkeypatch.setattr(P, "adversarial_verify",
                        lambda *a, **kw: SimpleNamespace(passed=False, refutations=["lossy"]))
    out = M.distill_backlog(after_id=0, limit=10, path=db)
    assert out["persisted"] == 0 and out["refuted"] == 2, out
    st = M.stats(path=db)
    assert st["semantic"] == 0 and st["episodic"] == 2, st   # raw NEVER lost


def test_watermark_advances_and_bounds(tmp_path, monkeypatch):
    db = tmp_path / "mem.duckdb"
    _seed(db, 4)
    monkeypatch.setattr(M, "distill", lambda text, **kw: "n")
    import mmorch.patterns as P
    monkeypatch.setattr(P, "adversarial_verify",
                        lambda *a, **kw: SimpleNamespace(passed=True, refutations=[]))
    out1 = M.distill_backlog(after_id=0, limit=2, path=db)        # bounded batch
    assert out1["seen"] == 2
    out2 = M.distill_backlog(after_id=out1["last_id"], limit=10, path=db)
    assert out2["seen"] == 2, out2                                # resumes AFTER the watermark
    assert M.stats(path=db)["semantic"] == 4


def test_skip_notes_skipped(tmp_path, monkeypatch):
    db = tmp_path / "mem.duckdb"
    _seed(db, 1)
    monkeypatch.setattr(M, "distill", lambda text, **kw: "SKIP")
    out = M.distill_backlog(after_id=0, limit=10, path=db)
    assert out["skipped"] == 1 and out["persisted"] == 0, out
