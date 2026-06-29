"""retention (modulo cognitivo #1): decay Ebbinghaus + Zeigarnik + modo dual.
Determinista, sin LLM, sin depender de fastembed (degrade graceful)."""
import sys, pathlib, time
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import mmorch.memory as M
from mmorch.retention import importance, should_forget

DAY = 86400.0


def _backdate(db, note_id, *, seconds_ago):
    """Envejece last_accessed_at de una nota para simular paso del tiempo (sin sleep)."""
    con = M._connect(db)
    try:
        con.execute("UPDATE semantic SET last_accessed_at = ? WHERE id = ?",
                    [time.time() - seconds_ago, note_id])
    finally:
        con.close()


def _col(db, note_id, col):
    con = M._connect(db)
    try:
        return con.execute(f"SELECT {col} FROM semantic WHERE id = ?", [note_id]).fetchone()[0]
    finally:
        con.close()


# --- formula pura -----------------------------------------------------------
def test_decay_monotonic():
    now = time.time()
    assert importance(now, now, 0, False) > importance(now, now - 30 * DAY, 0, False)


def test_access_boosts():
    now = time.time()
    base = now - 10 * DAY
    assert importance(now, base, 5, False) > importance(now, base, 0, False)


def test_forget_threshold():
    now = time.time()
    assert should_forget(importance(now, now - 365 * DAY, 0, False))


def test_zeigarnik_resists():
    now = time.time()
    old = now - 365 * DAY
    assert should_forget(importance(now, old, 0, False))
    assert not should_forget(importance(now, old, 0, True))


def test_graceful_none_last_accessed():
    now = time.time()
    assert importance(now, None, 0, False) == importance(now, now, 0, False)


# --- wiring a memory.py -----------------------------------------------------
def test_recall_tracks_access(tmp_path):
    db = tmp_path / "m.duckdb"
    nid = M.write_note("mmorch_self", "nota que se va a acceder", path=db)
    M.recall("nota acceder", scope="mmorch_self", k=5, path=db)
    con = M._connect(db)
    try:
        ac, la = con.execute(
            "SELECT access_count, last_accessed_at FROM semantic WHERE id = ?", [nid]
        ).fetchone()
    finally:
        con.close()
    assert ac == 1 and la is not None


def test_hybrid_no_double_count(tmp_path):
    db = tmp_path / "m.duckdb"
    nid = M.write_note("mmorch_self", "bandit thompson cascade umbral", path=db)
    M.recall_hybrid("bandit thompson", scope="mmorch_self", k=5, path=db)
    con = M._connect(db)
    try:
        ac = con.execute("SELECT access_count FROM semantic WHERE id = ?", [nid]).fetchone()[0]
    finally:
        con.close()
    assert ac == 1   # una sola vez, no k*2 ni doble


def test_consolidate_forget_off_by_default(tmp_path):
    db = tmp_path / "m.duckdb"
    nid = M.write_note("mmorch_self", "nota vieja olvidable", path=db)
    _backdate(db, nid, seconds_ago=365 * DAY)
    res = M.consolidate(scope="mmorch_self", path=db)   # forget no pasado -> OFF
    assert res["forgotten"] == 0
    con = M._connect(db)
    try:
        tomb = con.execute("SELECT tombstone FROM semantic WHERE id = ?", [nid]).fetchone()[0]
    finally:
        con.close()
    assert tomb is False


def test_consolidate_forgets_old_decay_note(tmp_path):
    db = tmp_path / "m.duckdb"
    nid = M.write_note("mmorch_self", "nota vieja sin acceso", path=db)
    _backdate(db, nid, seconds_ago=365 * DAY)
    res = M.consolidate(scope="mmorch_self", forget=True, path=db)
    assert res["forgotten"] == 1
    con = M._connect(db)
    try:
        tomb = con.execute("SELECT tombstone FROM semantic WHERE id = ?", [nid]).fetchone()[0]
    finally:
        con.close()
    assert tomb is True


def test_verified_resists_forget(tmp_path):
    db = tmp_path / "m.duckdb"
    nid = M.write_note("mmorch_self", "nota vieja pero verificada", verified=True, path=db)
    _backdate(db, nid, seconds_ago=365 * DAY)
    res = M.consolidate(scope="mmorch_self", forget=True, path=db)
    assert res["forgotten"] == 0


def test_permanent_resists_forget(tmp_path):
    db = tmp_path / "m.duckdb"
    nid = M.write_note("mmorch_self", "nota vieja pero fijada", permanent=True, path=db)
    _backdate(db, nid, seconds_ago=365 * DAY)
    res = M.consolidate(scope="mmorch_self", forget=True, path=db)
    assert res["forgotten"] == 0


def test_open_loop_resists_then_close(tmp_path):
    db = tmp_path / "m.duckdb"
    nid = M.write_note("mmorch_self", "tarea abierta vieja", open_loop=True, path=db)
    _backdate(db, nid, seconds_ago=365 * DAY)
    assert M.consolidate(scope="mmorch_self", forget=True, path=db)["forgotten"] == 0
    M.close_loop(nid, path=db)
    _backdate(db, nid, seconds_ago=365 * DAY)   # close_loop toco nada del tiempo; re-envejecer
    assert M.consolidate(scope="mmorch_self", forget=True, path=db)["forgotten"] == 1


def test_raw_survives_forget(tmp_path):
    # SAFETY: olvidar la nota semantica NO pierde el hecho — recall lo trae del raw.
    db = tmp_path / "m.duckdb"
    eid = M.write_episode("task_id", "hecho", {"dato": "valor critico"}, path=db)
    nid = M.write_note("task_id", "valor critico destilado", source_ids=[eid], path=db)
    _backdate(db, nid, seconds_ago=365 * DAY)
    M.consolidate(scope="task_id", forget=True, path=db)
    out = M.recall("cualquier cosa", scope="task_id", k=3, path=db)
    assert len(out) >= 1
    assert any(n.layer == "episodic" for n in out)   # el raw sigue ahi


def test_forget_preview_counts_and_protects(tmp_path):
    # read-only metrica-gate: cuenta lo que se olvidaria, excluye protegidas.
    db = tmp_path / "m.duckdb"
    old = M.write_note("mmorch_self", "vieja decay olvidable", path=db)
    M.write_note("mmorch_self", "vieja verificada", verified=True, path=db)
    M.write_note("mmorch_self", "vieja permanente", permanent=True, path=db)
    M.write_note("mmorch_self", "tarea abierta", open_loop=True, path=db)
    for nid in (old,):
        _backdate(db, nid, seconds_ago=365 * DAY)
    pv = M.forget_preview(scope="mmorch_self", path=db)
    assert pv["total_live"] == 4
    assert pv["eligible"] == 1                       # solo la decay no protegida
    # en el grid default, la vieja cae bajo el umbral en al menos una config
    assert any(g["would_forget"] == 1 for g in pv["grid"])
    # nada se tombstoneo (read-only)
    assert _col(db, old, "tombstone") is False


def test_forget_preview_empty_db(tmp_path):
    db = tmp_path / "m.duckdb"
    pv = M.forget_preview(path=db)
    assert pv["total_live"] == 0 and pv["eligible"] == 0
    assert all(g["would_forget"] == 0 for g in pv["grid"])


def test_old_db_graceful_no_crash(tmp_path):
    # nota sin last_accessed_at (NULL) no rompe consolidate.
    db = tmp_path / "m.duckdb"
    nid = M.write_note("mmorch_self", "nota", path=db)
    con = M._connect(db)
    try:
        con.execute("UPDATE semantic SET last_accessed_at = NULL WHERE id = ?", [nid])
    finally:
        con.close()
    res = M.consolidate(scope="mmorch_self", forget=True, path=db)   # no crashea
    assert res["forgotten"] == 0   # NULL -> tratado como now -> no olvida
