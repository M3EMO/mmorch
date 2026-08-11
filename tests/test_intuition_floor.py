"""healthy() — the deterministic error-rate floor over the intuition candidate pool.

Floor tests run with probe_every_s=0 (half-open OFF): with probes on, a sick model
passes through on the FIRST call as a probe, so floor assertions would depend on the
probe-state file's age — exactly the machine-state flakiness that broke this suite.
The half-open behavior has its own isolated test below.
"""
from mmorch.intuition import healthy

RATES = {
    "glm-4.6": {"calls": 47, "error_rate": 0.34},          # the measured offender
    "deepseek-chat": {"calls": 141, "error_rate": 0.007},
    "gemini-2.5-flash": {"calls": 31, "error_rate": 0.0},
    "flaky-lowvolume": {"calls": 3, "error_rate": 1.0},    # too few calls to judge
}
POOL = ["deepseek-chat", "glm-4.6", "gemini-2.5-flash"]


def _floor(models, **kw):
    return healthy(models, rates=RATES, probe_every_s=0, **kw)


def test_sick_model_dropped():
    assert _floor(POOL) == ["deepseek-chat", "gemini-2.5-flash"]


def test_low_volume_not_judged():
    # 3 calls at 100% error is noise, not evidence — stays in
    assert _floor(["deepseek-chat", "flaky-lowvolume"]) == ["deepseek-chat", "flaky-lowvolume"]


def test_unknown_model_stays():
    assert _floor(["brand-new-model"]) == ["brand-new-model"]


def test_all_sick_fails_open():
    # never return an empty pool: routing must survive a fully-degraded window
    assert _floor(["glm-4.6"]) == ["glm-4.6"]


def test_variant_suffix_maps_to_base_model():
    # 'model@thr' arms share the base model's health
    assert _floor(["glm-4.6@0.5", "deepseek-chat@0.5"]) == ["deepseek-chat@0.5"]


def test_half_open_probe_cycle(tmp_path):
    # aislado en tmp_path: NUNCA tocar el logs/health_probes.json real desde tests
    probes = tmp_path / "health_probes.json"
    kw = dict(rates=RATES, probe_state=probes, probe_every_s=600)
    # sin estado previo el enfermo pasa una vez (probe inmediato)...
    assert healthy(["glm-4.6"], now=1000.0, **kw) == ["glm-4.6"]
    # ...dentro del intervalo queda afuera...
    assert healthy(POOL, now=1100.0, **kw) == ["deepseek-chat", "gemini-2.5-flash"]
    # ...y cumplido el intervalo vuelve a probar.
    assert "glm-4.6" in healthy(POOL, now=1700.0, **kw)


def test_broken_metrics_fails_open(monkeypatch, tmp_path):
    import mmorch.metrics as M

    def _boom(**kw):
        raise RuntimeError("log unreadable")
    monkeypatch.setattr(M, "error_rates", _boom)
    assert healthy(POOL, probe_state=tmp_path / "p.json") == POOL


def test_floor_tests_never_write_real_probe_state():
    # el bug que motivó este archivo: tests contaminando logs/health_probes.json
    from mmorch.intuition import _PROBE_STATE
    before = _PROBE_STATE.read_text(encoding="utf-8") if _PROBE_STATE.exists() else None
    _floor(POOL)
    after = _PROBE_STATE.read_text(encoding="utf-8") if _PROBE_STATE.exists() else None
    assert before == after
