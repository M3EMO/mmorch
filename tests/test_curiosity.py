"""curiosity + open-loops (modulo cognitivo #3).
find_tension: pares en la banda 0.82<=cosine<0.92 (candidatos a redundancia/conflicto).
Embeddings inyectados a mano (vectores unitarios 2D) -> cosine exacto, sin fastembed.
"""
import sys, pathlib, time
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import mmorch.memory as M
from mmorch.curiosity import find_tension

# vectores unitarios 2D: cosine([1,0],[x,y]) = x cuando [x,y] es unitario.
V0 = [1.0, 0.0]
V_085 = [0.85, 0.5267827]   # cosine 0.85 (en banda)
V_095 = [0.95, 0.3122499]   # cosine 0.95 (dup, sobre el techo)
V_050 = [0.50, 0.8660254]   # cosine 0.50 (temas distintos)


def _insert(db, scope, text, vec, *, open_loop=False, needs_review=False):
    """Inserta una nota semantica directo (sin embed() -> sin fastembed -> rapido).
    vec=None omite las columnas de embedding (default NULL) en vez de bindear None a
    DOUBLE[]."""
    con = M._connect(db)
    try:
        sid = con.execute("SELECT nextval('seq_semantic')").fetchone()[0]
        now = time.time()
        if vec is None:
            con.execute(
                "INSERT INTO semantic (id, ts, scope, text, source_ids, tombstone, "
                "verified, access_count, last_accessed_at, open_loop, lifespan, "
                "needs_review) VALUES (?,?,?,?,?,FALSE,FALSE,0,?,?,'decay',?)",
                [sid, now, scope, text, "[]", now, open_loop, needs_review])
        else:
            con.execute(
                "INSERT INTO semantic (id, ts, scope, text, embedding, emb_model, dim, "
                "source_ids, tombstone, verified, access_count, last_accessed_at, "
                "open_loop, lifespan, needs_review) "
                "VALUES (?,?,?,?,?,?,?,?,FALSE,FALSE,0,?,?,'decay',?)",
                [sid, now, scope, text, vec, "test", len(vec), "[]", now,
                 open_loop, needs_review])
        return int(sid)
    finally:
        con.close()


# --- find_tension -----------------------------------------------------------
def test_tension_band_paired(tmp_path):
    db = tmp_path / "m.duckdb"
    a = _insert(db, "proj", "el timeout es 30s", V0)
    b = _insert(db, "proj", "timeout configurado en 60s", V_085)
    pairs = find_tension(scope="proj", path=db)["pairs"]
    assert len(pairs) == 1
    assert {pairs[0]["a"], pairs[0]["b"]} == {a, b}
    assert pairs[0]["cosine"] == 0.85


def test_tension_excludes_high_cosine(tmp_path):
    # >= 0.92 lo mergea consolidate (dup), no es tension.
    db = tmp_path / "m.duckdb"
    _insert(db, "proj", "nota a", V0)
    _insert(db, "proj", "nota a casi igual", V_095)
    assert find_tension(scope="proj", path=db)["pairs"] == []


def test_tension_excludes_low_cosine(tmp_path):
    db = tmp_path / "m.duckdb"
    _insert(db, "proj", "tema uno", V0)
    _insert(db, "proj", "tema distinto", V_050)
    assert find_tension(scope="proj", path=db)["pairs"] == []


def test_tension_excludes_review(tmp_path):
    db = tmp_path / "m.duckdb"
    _insert(db, "proj", "nota a", V0)
    _insert(db, "proj", "nota b", V_085, needs_review=True)
    assert find_tension(scope="proj", path=db)["pairs"] == []


def test_tension_no_embeddings_graceful(tmp_path):
    db = tmp_path / "m.duckdb"
    _insert(db, "proj", "sin embedding uno", None)
    _insert(db, "proj", "sin embedding dos", None)
    assert find_tension(scope="proj", path=db)["pairs"] == []


def test_tension_separate_scopes_no_cross(tmp_path):
    # pares solo dentro del mismo scope.
    db = tmp_path / "m.duckdb"
    _insert(db, "proj_a", "x", V0)
    _insert(db, "proj_b", "y", V_085)
    assert find_tension(path=db)["pairs"] == []


def test_tension_cap_skips_and_reports(tmp_path):
    # guardrail O(n^2): scope sobre el cap se saltea, reportado (sin drop silencioso).
    db = tmp_path / "m.duckdb"
    _insert(db, "big", "n1", V0)
    _insert(db, "big", "n2", V_085)
    _insert(db, "big", "n3", V0)
    res = find_tension(scope="big", max_per_scope=2, path=db)
    assert res["pairs"] == []
    assert res["skipped"] == [{"scope": "big", "n": 3}]


# --- open_loops -------------------------------------------------------------
def test_open_loops_lists(tmp_path):
    db = tmp_path / "m.duckdb"
    nid = _insert(db, "task_id", "arreglar conflicto de puertos docker", None, open_loop=True)
    _insert(db, "task_id", "nota normal", None)
    loops = M.open_loops(scope="task_id", path=db)
    assert [n.id for n in loops] == [nid]


def test_open_loops_excludes_closed(tmp_path):
    db = tmp_path / "m.duckdb"
    nid = _insert(db, "task_id", "tarea abierta", None, open_loop=True)
    assert len(M.open_loops(scope="task_id", path=db)) == 1
    M.close_loop(nid, path=db)
    assert M.open_loops(scope="task_id", path=db) == []
