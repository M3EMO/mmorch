"""Cache-hit accounting: cached input se cobra al precio cache (no a price_in). Vuelve
medible el ahorro por prompt-caching/prefix-stable. Observabilidad, no rutea."""
import sys, pathlib, json
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import mmorch.cost as COST
import mmorch.prices as PR
import mmorch.metrics as MET
import mmorch.providers as P


def test_effective_cache_price_from_override(tmp_path):
    pj = tmp_path / "prices.json"
    pj.write_text(json.dumps({"deepseek-chat": {"price_cache_in": 0.0028}}), encoding="utf-8")
    assert PR.effective_cache_price("deepseek-chat", path=pj) == 0.0028
    # sin override -> fallback a price_in (sin descuento)
    pin, _ = PR.effective_prices("gemini-3.1-flash-lite", path=pj)
    assert PR.effective_cache_price("gemini-3.1-flash-lite", path=pj) == pin


def test_repo_prices_json_has_deepseek_cache():
    # integracion: el prices.json del repo trae el precio cache de DeepSeek
    assert PR.effective_cache_price("deepseek-chat") == 0.0028


def test_cost_with_cache_is_cheaper(monkeypatch, tmp_path):
    pj = tmp_path / "prices.json"
    pj.write_text(json.dumps({"deepseek-chat": {"price_cache_in": 0.0028}}), encoding="utf-8")
    monkeypatch.setattr(PR, "PRICES_PATH", pj)
    full = COST.cost_usd("deepseek-chat", 1000, 100, 0)        # sin cache
    cached = COST.cost_usd("deepseek-chat", 1000, 100, 800)    # 800/1000 cacheados
    assert cached < full
    # math exacto: 200*pin + 800*pcache + 100*pout, /1e6
    pin, pout = PR.effective_prices("deepseek-chat", path=pj)
    expect = (200 * pin + 800 * 0.0028 + 100 * pout) / 1_000_000
    assert abs(cached - expect) < 1e-12


def test_cost_cached_zero_is_backwards_compatible():
    # cached=0 -> identico a la formula vieja (in*pin + out*pout)
    pin, pout = PR.effective_prices("deepseek-chat")
    assert abs(COST.cost_usd("deepseek-chat", 500, 50, 0)
               - (500 * pin + 50 * pout) / 1_000_000) < 1e-12


def test_cached_tokens_extraction():
    class U1:  # DeepSeek
        prompt_cache_hit_tokens = 64
    class _Det:
        cached_tokens = 30
    class U2:  # OpenAI-style
        prompt_tokens_details = _Det()
    class U3:  # nada
        pass
    assert P._cached_tokens(U1()) == 64
    assert P._cached_tokens(U2()) == 30
    assert P._cached_tokens(U3()) == 0


def test_cache_stats(tmp_path, monkeypatch):
    p = tmp_path / "m.jsonl"
    monkeypatch.setattr(MET, "_LOG_DIR", tmp_path)
    monkeypatch.setattr(MET, "_LOG_PATH", p)
    MET.log_event(pattern="t", node="n", model="deepseek-chat", family="deepseek",
                  in_tokens=1000, out_tokens=100, cost_usd=0.001, latency_s=0.1, cached_tokens=400)
    MET.log_event(pattern="t", node="n", model="deepseek-chat", family="deepseek",
                  in_tokens=1000, out_tokens=100, cost_usd=0.001, latency_s=0.1, cached_tokens=600)
    st = MET.cache_stats()
    d = st["by_model"]["deepseek-chat"]
    assert d["in_tokens"] == 2000 and d["cached_tokens"] == 1000
    assert d["cache_hit_rate"] == 0.5
