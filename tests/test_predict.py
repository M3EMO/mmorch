"""tests predict.py — v0.1 predictor (out_tokens/latencia/cost). Sin API, todo local."""
from mmorch.predict import Predictor, _quantile, train, cross_val_error


def test_quantile():
    assert _quantile([10, 20, 30, 40, 50], 0.5) == 30
    assert _quantile([10, 20, 30, 40, 50], 0.0) == 10
    assert _quantile([10, 20, 30, 40, 50], 1.0) == 50
    assert _quantile([], 0.5) == 0.0


_ROWS = (
    [{"model": "deepseek-chat", "pattern": "fan_out", "in": 100, "out": 200, "lat": 2.0}] * 5 +
    [{"model": "deepseek-chat", "pattern": "fan_out", "in": 100, "out": 2000, "lat": 8.0}] * 5
)


def test_fit_and_quantile_predict():
    p = Predictor().fit(_ROWS)
    med = p.predict_out_tokens("deepseek-chat", "fan_out", q=0.5)
    p90 = p.predict_out_tokens("deepseek-chat", "fan_out", q=0.9)
    assert p90 > med                              # conservador sobre-estima
    assert 200 <= med <= 2000


def test_predict_cost_uses_price_formula():
    p = Predictor().fit(_ROWS)
    c_med = p.predict_cost("deepseek-chat", pattern="fan_out", in_tokens=100, q=0.5)
    c_p90 = p.predict_cost("deepseek-chat", pattern="fan_out", in_tokens=100, q=0.9)
    assert c_p90 > c_med > 0                       # cost crece con out predicho


def test_fallback_hierarchy():
    p = Predictor().fit(_ROWS)
    # patrón desconocido -> cae a by_model
    assert p.predict_out_tokens("deepseek-chat", "patron_inexistente") > 0
    # modelo desconocido -> cae a global
    assert p.predict_out_tokens("modelo_inexistente", "x") > 0


def test_cross_val_coverage():
    rows = _ROWS * 3                               # n suficiente
    ev = [{"model": r["model"], "pattern": r["pattern"], "in_tokens": r["in"],
           "out_tokens": r["out"], "latency_s": r["lat"]} for r in rows]
    res = cross_val_error(ev, k=3)
    assert res["n"] == len(rows)
    assert 0.0 <= res["p90_coverage_out"] <= 1.0
