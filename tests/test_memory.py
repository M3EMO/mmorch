"""memoria episodica+semantica: schema 2-capas, recall 2-stage, FIX A/B/C.
Pasa con o sin fastembed (degrade graceful)."""
import sys, pathlib
from dataclasses import dataclass
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import mmorch.memory as M
import mmorch.providers as P
import mmorch.patterns as PAT


@dataclass
class _Res:
    text: str
    cost_usd: float = 0.0


def _mock_call(text):
    def _c(model, messages, **kw):
        return _Res(text=text)
    return _c


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


def test_remember_distills_and_persists(tmp_path, monkeypatch):
    db = tmp_path / "m.duckdb"
    monkeypatch.setattr(P, "call", _mock_call("bandit aprende umbral de cascade"))
    out = M.remember("mmorch_self", "hoy wireamos el bandit Thompson en cascade", path=db)
    assert out["persisted"] and out["note_id"] is not None
    assert out["distilled"] == "bandit aprende umbral de cascade"
    s = M.stats(path=db)
    assert s["episodic"] == 1 and s["semantic"] == 1


def test_remember_skip_keeps_only_raw(tmp_path, monkeypatch):
    db = tmp_path / "m.duckdb"
    monkeypatch.setattr(P, "call", _mock_call("SKIP"))
    out = M.remember("global", "evento trivial sin valor", path=db)
    assert out["persisted"] is False and out["note_id"] is None
    s = M.stats(path=db)
    assert s["episodic"] == 1 and s["semantic"] == 0  # raw queda, nota no


def test_remember_verify_refuted_drops_note(tmp_path, monkeypatch):
    db = tmp_path / "m.duckdb"
    monkeypatch.setattr(P, "call", _mock_call("nota infiel que omite todo"))
    monkeypatch.setattr(PAT, "adversarial_verify",
                        lambda *a, **k: PAT.Verdict(False, 0.9, ["omite hecho critico"],
                                                    "", "gemini-2.5-flash", 0.0))
    out = M.remember("global", "episodio con hechos importantes", verify=True, path=db)
    assert out["persisted"] is False and out["refutations"] == ["omite hecho critico"]
    s = M.stats(path=db)
    assert s["episodic"] == 1 and s["semantic"] == 0  # nota infiel descartada, raw queda


# ---- verification coverage (Martin 2026: % del aprendizaje validado) ----
def test_write_note_verified_flag_in_stats(tmp_path):
    db = tmp_path / "m.duckdb"
    M.write_note("global", "nota verificada", verified=True, path=db)
    M.write_note("global", "nota sin verificar", path=db)
    s = M.stats(path=db)
    assert s["verified"] == 1 and s["semantic"] == 2
    assert s["verification_coverage"] == 0.5


def test_coverage_empty_db_is_none(tmp_path):
    s = M.stats(path=tmp_path / "m.duckdb")
    assert s["verified"] == 0 and s["verification_coverage"] is None


def test_remember_verify_passed_marks_note_verified(tmp_path, monkeypatch):
    db = tmp_path / "m.duckdb"
    monkeypatch.setattr(P, "call", _mock_call("nota fiel"))
    monkeypatch.setattr(PAT, "adversarial_verify",
                        lambda *a, **k: PAT.Verdict(True, 0.9, [], "", "gemini-2.5-flash", 0.0))
    out = M.remember("global", "episodio importante", verify=True, path=db)
    assert out["persisted"]
    s = M.stats(path=db)
    assert s["verified"] == 1 and s["verification_coverage"] == 1.0


def test_remember_without_verify_counts_zero_coverage(tmp_path, monkeypatch):
    db = tmp_path / "m.duckdb"
    monkeypatch.setattr(P, "call", _mock_call("nota sin chequear"))
    M.remember("global", "episodio cualquiera", path=db)
    s = M.stats(path=db)
    assert s["verified"] == 0 and s["verification_coverage"] == 0.0


def test_old_schema_migrates_verified_column(tmp_path):
    # DB creada antes de la columna `verified` -> _connect la agrega sin romper.
    import duckdb
    db = tmp_path / "m.duckdb"
    con = duckdb.connect(str(db))
    con.execute("""
        CREATE TABLE semantic (
            id BIGINT, ts DOUBLE, scope VARCHAR, text VARCHAR,
            embedding DOUBLE[], emb_model VARCHAR, dim INTEGER,
            source_ids VARCHAR, tombstone BOOLEAN DEFAULT FALSE);
        CREATE SEQUENCE seq_semantic START 1;
        CREATE TABLE episodic (id BIGINT, ts DOUBLE, scope VARCHAR, kind VARCHAR,
                               actor VARCHAR, payload VARCHAR);
        CREATE SEQUENCE seq_episodic START 1;
    """)
    con.close()
    M.write_note("global", "nota post-migracion", verified=True, path=db)
    s = M.stats(path=db)
    assert s["verified"] == 1 and s["verification_coverage"] == 1.0


def test_tombstoned_verified_excluded_from_coverage(tmp_path):
    db = tmp_path / "m.duckdb"
    nid = M.write_note("global", "verificada pero borrada", verified=True, path=db)
    M.write_note("global", "viva sin verificar", path=db)
    M.tombstone_note(nid, path=db)
    s = M.stats(path=db)
    assert s["verified"] == 0 and s["verification_coverage"] == 0.0


# ---- consolidacion periodica (Martin 2026: merge dups, compress, sin perder raw) ----
def test_consolidate_merges_exact_duplicates(tmp_path):
    db = tmp_path / "m.duckdb"
    a = M.write_note("global", "el bandit elige umbral de cascade", path=db)
    b = M.write_note("global", "el bandit elige umbral de cascade", path=db)
    out = M.consolidate(path=db)
    assert out["tombstoned"] == 1
    s = M.stats(path=db)
    assert s["semantic"] == 1
    # se queda la mas nueva (b)
    assert out["merged"][0]["kept"] == b and out["merged"][0]["tombstoned"] == [a]


def test_consolidate_prefers_verified_over_recent(tmp_path):
    db = tmp_path / "m.duckdb"
    old_verified = M.write_note("global", "hecho importante", verified=True, path=db)
    M.write_note("global", "hecho importante", path=db)  # mas nueva, sin verificar
    out = M.consolidate(path=db)
    assert out["merged"][0]["kept"] == old_verified


def test_consolidate_respects_scope_boundaries(tmp_path):
    db = tmp_path / "m.duckdb"
    M.write_note("task_id", "misma nota", path=db)
    M.write_note("global", "misma nota", path=db)
    out = M.consolidate(path=db)
    assert out["tombstoned"] == 0  # scopes distintos: NO se mergea


def test_consolidate_dry_run_reports_without_touching(tmp_path):
    db = tmp_path / "m.duckdb"
    M.write_note("global", "nota repetida", path=db)
    M.write_note("global", "nota repetida", path=db)
    out = M.consolidate(dry_run=True, path=db)
    assert out["tombstoned"] == 1 and out["dry_run"] is True
    assert M.stats(path=db)["semantic"] == 2  # nada tocado


def test_consolidate_logs_episodic_event(tmp_path):
    db = tmp_path / "m.duckdb"
    M.write_note("global", "x", path=db)
    M.write_note("global", "x", path=db)
    M.consolidate(path=db)
    import duckdb
    con = duckdb.connect(str(db))
    kinds = [r[0] for r in con.execute("SELECT kind FROM episodic").fetchall()]
    con.close()
    assert "consolidation" in kinds  # auditabilidad: el log inmutable registra la corrida


def test_consolidate_over_budget_flag(tmp_path):
    db = tmp_path / "m.duckdb"
    M.write_note("global", "nota larga " * 50, path=db)
    out = M.consolidate(max_bytes=10, path=db)
    assert out["over_budget"] is True
    assert M.stats(path=db)["semantic"] == 1  # over-budget NO borra solo: flag gated


def test_consolidate_near_duplicates_by_embedding(tmp_path):
    if M._get_embedder() is None:
        import pytest
        pytest.skip("fastembed no instalado")
    db = tmp_path / "m.duckdb"
    M.write_note("global", "DeepSeek hace la generacion bulk barata cross-family", path=db)
    M.write_note("global", "la generacion bulk barata cross-family la hace DeepSeek", path=db)
    M.write_note("global", "el gato duerme arriba del router", path=db)
    out = M.consolidate(sim_threshold=0.9, path=db)
    assert out["tombstoned"] == 1  # los dos primeros se mergean, el gato sobrevive
    assert M.stats(path=db)["semantic"] == 2
