"""I-1 learn: meta-inteligencia desde metrics. Eventos mockeados."""
import sys, pathlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import mmorch.learn as L


def _ev(model, pattern, cost, lat):
    return {"model": model, "pattern": pattern, "cost_usd": cost,
            "latency_s": lat, "out_tokens": 100}


def test_analyze_aggregates(monkeypatch):
    monkeypatch.setattr(L, "read_events", lambda: [
        _ev("deepseek-chat", "fan_out", 0.001, 5),
        _ev("deepseek-chat", "fan_out", 0.003, 7),
        _ev("gemini-2.5-flash", "fan_out", 0.05, 40),
    ])
    rep = L.analyze()
    assert rep["total_calls"] == 3
    ds = [r for r in rep["rows"] if r["model"] == "deepseek-chat"][0]
    assert ds["calls"] == 2 and ds["cost_usd"] == 0.004


def test_recommend_flags_expensive_bulk(monkeypatch):
    monkeypatch.setattr(L, "read_events", lambda: [
        _ev("deepseek-chat", "fan_out", 0.0002, 5),
        _ev("gemini-2.5-flash", "fan_out", 0.0077, 40),
    ])
    recs = L.recommend()
    # debe detectar el 35x y recomendar deepseek para bulk + flag latencia.
    assert any("deepseek-chat" in r and "bulk" in r.lower() for r in recs)
    assert any("p95" in r or "latencia" in r.lower() for r in recs)
