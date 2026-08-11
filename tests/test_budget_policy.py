"""budget_policy: audit 2026-08 #07 — JSON corrupto debe fallar CERRADO (bloquear
gasto), no abierto en silencio."""
import sys, pathlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import mmorch.budget_policy as BP


def test_load_returns_empty_for_missing_file(tmp_path, monkeypatch):
    monkeypatch.setattr(BP, "_PATH", tmp_path / "missing.json")
    assert BP.load() == []


def test_load_strict_raises_on_corrupt_file(tmp_path, monkeypatch):
    p = tmp_path / "budget_policies.json"
    p.write_text("{not json", encoding="utf-8")
    monkeypatch.setattr(BP, "_PATH", p)
    assert BP.load() == []                                   # non-strict: no rompe callers viejos
    import pytest
    with pytest.raises(BP.PolicyLoadError):
        BP.load(strict=True)


def test_blocking_incident_fails_closed_on_corrupt_policies(tmp_path, monkeypatch):
    p = tmp_path / "budget_policies.json"
    p.write_text("{not json", encoding="utf-8")
    monkeypatch.setattr(BP, "_PATH", p)
    inc = BP.blocking_incident(snap={"global": 0.0})
    assert inc is not None and inc["level"] == "hard", \
        "politicas ilegibles deben bloquear trabajo nuevo, no dejarlo pasar sin señal"


def test_blocking_incident_normal_path_unaffected(tmp_path, monkeypatch):
    p = tmp_path / "budget_policies.json"
    monkeypatch.setattr(BP, "_PATH", p)
    BP.save([{"scope": "global", "limit_usd": 10, "warn_pct": 80}])
    assert BP.blocking_incident(snap={"global": 1.0}) is None
    assert BP.blocking_incident(snap={"global": 10.0})["scope"] == "global"


def test_save_is_atomic_no_tmp_left_behind(tmp_path, monkeypatch):
    p = tmp_path / "budget_policies.json"
    monkeypatch.setattr(BP, "_PATH", p)
    BP.save([{"scope": "global", "limit_usd": 5, "warn_pct": 90}])
    assert p.exists()
    assert not p.with_suffix(".json.tmp").exists()
