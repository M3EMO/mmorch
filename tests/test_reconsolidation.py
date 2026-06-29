"""reconsolidacion (modulo cognitivo #2): la memoria se auto-corrige con el uso.
confirm -> reinforce (boost); contradict -> flag (deja de surfacear, cae al raw).
Determinista, sin LLM, sin depender de fastembed."""
import sys, pathlib, time
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import mmorch.memory as M

DAY = 86400.0


def _backdate(db, note_id, *, seconds_ago):
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


# --- confirm: reinforce -----------------------------------------------------
def test_reinforce_boosts_access(tmp_path):
    db = tmp_path / "m.duckdb"
    nid = M.write_note("mmorch_self", "nota a confirmar", path=db)
    M.reinforce(nid, boost=3, path=db)
    assert _col(db, nid, "access_count") == 3


def test_reinforce_survives_forget(tmp_path):
    # una nota que se olvidaria, tras confirm sobrevive consolidate(forget=True).
    db = tmp_path / "m.duckdb"
    nid = M.write_note("mmorch_self", "vieja sin acceso", path=db)
    _backdate(db, nid, seconds_ago=365 * DAY)
    # sin reforzar: se olvida
    assert M.consolidate(scope="mmorch_self", forget=True, dry_run=True, path=db)["forgotten"] == 1
    M.reinforce(nid, boost=10, path=db)   # confirm reciente -> last_accessed=now
    assert M.consolidate(scope="mmorch_self", forget=True, path=db)["forgotten"] == 0


# --- contradict: flag -------------------------------------------------------
def test_flag_hides_from_recall(tmp_path):
    db = tmp_path / "m.duckdb"
    nid = M.write_note("mmorch_self", "el puerto default es 8080", path=db)
    M.flag_contradiction(nid, path=db)
    out = M.recall("puerto default", scope="mmorch_self", k=5, path=db)
    assert all(n.id != nid for n in out if n.layer == "semantic")


def test_flag_hides_from_keyword(tmp_path):
    db = tmp_path / "m.duckdb"
    nid = M.write_note("mmorch_self", "bandit thompson cascade umbral", path=db)
    M.flag_contradiction(nid, path=db)
    out = M.recall_keyword("bandit thompson", scope="mmorch_self", k=5, path=db)
    assert all(n.id != nid for n in out)


def test_contradicted_falls_back_to_raw(tmp_path):
    # auto-correccion: la nota destilada (sospechada falsa) deja de surfacear; el
    # episodic raw (hecho sin editar) sigue ahi.
    db = tmp_path / "m.duckdb"
    eid = M.write_episode("task_id", "hecho", {"puerto": 9001}, path=db)
    nid = M.write_note("task_id", "el puerto es 8080", source_ids=[eid], path=db)
    M.flag_contradiction(nid, path=db)
    out = M.recall("puerto", scope="task_id", k=3, path=db)
    assert any(n.layer == "episodic" for n in out)
    assert all(n.id != nid for n in out if n.layer == "semantic")


# --- review queue + resolve -------------------------------------------------
def test_pending_review_lists(tmp_path):
    db = tmp_path / "m.duckdb"
    n1 = M.write_note("mmorch_self", "nota uno", path=db)
    n2 = M.write_note("mmorch_self", "nota dos", path=db)
    M.flag_contradiction(n1, path=db)
    pend = M.pending_review(scope="mmorch_self", path=db)
    ids = {n.id for n in pend}
    assert n1 in ids and n2 not in ids


def test_resolve_keep_resurfaces(tmp_path):
    db = tmp_path / "m.duckdb"
    nid = M.write_note("mmorch_self", "contradiccion erronea", path=db)
    M.flag_contradiction(nid, path=db)
    M.resolve_review(nid, drop=False, path=db)
    assert _col(db, nid, "needs_review") is False
    out = M.recall("contradiccion erronea", scope="mmorch_self", k=5, path=db)
    assert any(n.id == nid for n in out)


def test_resolve_drop_tombstones(tmp_path):
    db = tmp_path / "m.duckdb"
    nid = M.write_note("mmorch_self", "nota falsa", path=db)
    M.flag_contradiction(nid, path=db)
    M.resolve_review(nid, drop=True, path=db)
    assert _col(db, nid, "tombstone") is True


def test_review_frozen_from_consolidate(tmp_path):
    # nota en review NO participa del merge (no puede ganar _pick_keeper y pisar la buena).
    db = tmp_path / "m.duckdb"
    good = M.write_note("mmorch_self", "texto identico", path=db)
    bad = M.write_note("mmorch_self", "texto identico", path=db)   # dup mas reciente
    M.flag_contradiction(bad, path=db)
    res = M.consolidate(scope="mmorch_self", path=db)
    # bad esta congelada -> no hay merge -> good intacta
    assert res["tombstoned"] == 0
    assert _col(db, good, "tombstone") is False
