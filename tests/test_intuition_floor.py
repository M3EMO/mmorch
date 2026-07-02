"""healthy() — the deterministic error-rate floor over the intuition candidate pool."""
from mmorch.intuition import healthy

RATES = {
    "glm-4.6": {"calls": 47, "error_rate": 0.34},          # the measured offender
    "deepseek-chat": {"calls": 141, "error_rate": 0.007},
    "gemini-2.5-flash": {"calls": 31, "error_rate": 0.0},
    "flaky-lowvolume": {"calls": 3, "error_rate": 1.0},    # too few calls to judge
}
POOL = ["deepseek-chat", "glm-4.6", "gemini-2.5-flash"]


def test_sick_model_dropped():
    assert healthy(POOL, rates=RATES) == ["deepseek-chat", "gemini-2.5-flash"]


def test_low_volume_not_judged():
    # 3 calls at 100% error is noise, not evidence — stays in
    assert healthy(["deepseek-chat", "flaky-lowvolume"], rates=RATES) == \
        ["deepseek-chat", "flaky-lowvolume"]


def test_unknown_model_stays():
    assert healthy(["brand-new-model"], rates=RATES) == ["brand-new-model"]


def test_all_sick_fails_open():
    # never return an empty pool: routing must survive a fully-degraded window
    assert healthy(["glm-4.6"], rates=RATES) == ["glm-4.6"]


def test_variant_suffix_maps_to_base_model():
    # 'model@thr' arms share the base model's health
    assert healthy(["glm-4.6@0.5", "deepseek-chat@0.5"], rates=RATES) == ["deepseek-chat@0.5"]


def test_broken_metrics_fails_open(monkeypatch):
    import mmorch.metrics as M

    def _boom(**kw):
        raise RuntimeError("log unreadable")
    monkeypatch.setattr(M, "error_rates", _boom)
    assert healthy(POOL) == POOL
