"""recall_keyword + recall_hybrid (idea Hermes FTS5): keyword BM25-lite complementa al
recall semantico. Anda sin fastembed (clave: el termino exacto que el embedding difumina)."""
import sys, pathlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import mmorch.memory as M


def _seed(path):
    M.write_note("global", "el flag MMORCH_MAX_MONTHLY_USD topea el gasto mensual", path=path)
    M.write_note("global", "el bandit Thompson elige el brazo por muestreo Beta", path=path)
    M.write_note("global", "cross-family verification decorrelaciona errores", path=path)


def test_keyword_finds_exact_term(tmp_path):
    p = tmp_path / "m.duckdb"
    _seed(p)
    res = M.recall_keyword("MMORCH_MAX_MONTHLY_USD", "global", k=3, path=p)
    assert res and "MMORCH_MAX_MONTHLY_USD" in res[0].text     # match exacto rankea primero


def test_keyword_works_without_fastembed(tmp_path, monkeypatch):
    p = tmp_path / "m.duckdb"
    _seed(p)
    monkeypatch.setattr(M, "embed", lambda t: None)            # simula fastembed ausente
    res = M.recall_keyword("bandit Beta", "global", k=2, path=p)
    assert res and "bandit" in res[0].text


def test_keyword_empty_query_no_crash(tmp_path):
    p = tmp_path / "m.duckdb"
    _seed(p)
    assert M.recall_keyword("!!!", "global", path=p) == []


def test_hybrid_merges_both_and_degrades(tmp_path, monkeypatch):
    p = tmp_path / "m.duckdb"
    _seed(p)
    # con embed ausente, hybrid == keyword-only, no rompe
    monkeypatch.setattr(M, "embed", lambda t: None)
    res = M.recall_hybrid("gasto mensual topa", "global", k=2, path=p)
    assert res and any("gasto" in n.text for n in res)
