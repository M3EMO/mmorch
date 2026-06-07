"""memoria episodica+semantica: schema 2-capas, recall 2-stage, FIX A/B/C.
Pasa con o sin fastembed (degrade graceful)."""
import sys, pathlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import mmorch.memory as M


def test_episodic_append_and_stats(tmp_path):
    db = tmp_path / "m.duckdb"
    e1 = M.write_episode("mmorch_self", "decision", {"x": 1}, path=db)
    e2 = M.write_episode("mmorch_self", "decision", "texto crudo", path=db)
    assert e1 == 1 and e2 == 2
    s = M.stats(path=db)
    assert s["episodic"] == 2


def test_note_write_and_recall(tmp_path):
    db = tmp_path / "m.duckdb"
    M.write_note("mmorch_self", "el bandit Thompson elige umbral de cascade", path=db)
    M.write_note("mmorch_self", "DeepSeek hace bulk barato cross-family", path=db)
    out = M.recall("que hace el bandit", scope="mmorch_self", k=2, path=db)
    assert len(out) >= 1
    assert all(n.layer == "semantic" for n in out)


def test_recall_falls_back_to_episodic(tmp_path):
    # FIX B: sin notas semanticas suficientes, completa desde el log crudo.
    db = tmp_path / "m.duckdb"
    M.write_episode("task_id", "evento", {"msg": "hola"}, path=db)
    out = M.recall("cualquier cosa", scope="task_id", k=3, path=db)
    assert len(out) == 1 and out[0].layer == "episodic"


def test_scope_chain_climbs_hierarchy():
    # especifico -> general.
    assert M._scope_chain("task_id") == ["task_id", "subsector", "project_id", "mmorch_self", "global"]
    assert M._scope_chain("project_id")[0] == "project_id"
    assert M._scope_chain("desconocido") == ["desconocido", "global"]


def test_tombstone_hides_note(tmp_path):
    db = tmp_path / "m.duckdb"
    nid = M.write_note("global", "nota a borrar", path=db)
    M.tombstone_note(nid, path=db)
    s = M.stats(path=db)
    assert s["semantic"] == 0  # tombstoned no cuenta


def test_scope_isolation(tmp_path):
    # Una nota en 'global' NO aparece al pedir scope 'task_id' que sube task->...->global,
    # pero SI cuando el chain incluye global. Verificamos que un scope hermano no filtra.
    db = tmp_path / "m.duckdb"
    M.write_note("subsector", "nota de subsector", path=db)
    # project_id chain = [project_id, mmorch_self, global] -> NO incluye subsector.
    out = M.recall("nota", scope="project_id", k=5, path=db)
    assert all("subsector" not in n.scope for n in out)
