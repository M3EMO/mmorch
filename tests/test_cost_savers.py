"""prefix-stable prompts + off-peak advisory + effort-routing. Ahorros de costo medibles."""
import sys, pathlib
from datetime import datetime, timezone
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import mmorch.prompts as PR
import mmorch.schedule as SCH
import mmorch.effort as EF


# ---- prefix-stable -----------------------------------------------------------
def test_stable_prefix_shared_across_queries():
    shared = {"repo": "mmorch", "rules": ["OneFlow", "anti-sicofancia"]}
    a = PR.cacheable_messages("Sos un verificador.", shared, "Query 1?")
    b = PR.cacheable_messages("Sos un verificador.", shared, "Query 2 distinta?")
    # mismo prefijo (system+shared) -> mismas firmas -> cachea entre calls
    assert PR.shares_prefix(a, b)
    assert a[-1]["content"] != b[-1]["content"]   # solo la query volatil cambia


def test_dict_order_does_not_break_prefix():
    s1 = PR.cacheable_messages("x", {"b": 2, "a": 1}, "q")
    s2 = PR.cacheable_messages("x", {"a": 1, "b": 2}, "q")
    assert PR.prefix_signature(s1) == PR.prefix_signature(s2)   # canonicalizado (sort_keys)


def test_volatile_in_prefix_breaks_cache():
    a = PR.cacheable_messages("sys", "ctx", "q")
    b = PR.cacheable_messages("sys CAMBIADO", "ctx", "q")
    assert not PR.shares_prefix(a, b)


# ---- off-peak advisory -------------------------------------------------------
def test_off_peak_window_crossing_midnight(monkeypatch):
    monkeypatch.setenv("MMORCH_OFFPEAK_UTC", "16:30-00:30")
    assert SCH.is_off_peak(datetime(2026, 6, 13, 18, 0, tzinfo=timezone.utc))    # dentro
    assert SCH.is_off_peak(datetime(2026, 6, 13, 0, 0, tzinfo=timezone.utc))     # pasada medianoche
    assert not SCH.is_off_peak(datetime(2026, 6, 13, 12, 0, tzinfo=timezone.utc))  # pico


def test_advisory_hint_changes_with_period(monkeypatch):
    monkeypatch.setenv("MMORCH_OFFPEAK_UTC", "16:30-00:30")
    on = SCH.advisory(0.5, now=datetime(2026, 6, 13, 18, 0, tzinfo=timezone.utc))
    off = SCH.advisory(0.5, now=datetime(2026, 6, 13, 12, 0, tzinfo=timezone.utc))
    assert on["off_peak"] and not off["off_peak"]
    assert "diferir" in off["hint"]


def test_spend_by_period(monkeypatch, tmp_path):
    import mmorch.metrics as MET
    p = tmp_path / "m.jsonl"
    monkeypatch.setattr(MET, "_LOG_DIR", tmp_path)
    monkeypatch.setattr(MET, "_LOG_PATH", p)
    monkeypatch.setenv("MMORCH_OFFPEAK_UTC", "16:30-00:30")
    # ts off-peak (18:00 UTC) y pico (12:00 UTC) del mismo dia
    off_ts = datetime(2026, 6, 13, 18, 0, tzinfo=timezone.utc).timestamp()
    peak_ts = datetime(2026, 6, 13, 12, 0, tzinfo=timezone.utc).timestamp()
    import json
    with open(p, "w", encoding="utf-8") as f:
        f.write(json.dumps({"ts": off_ts, "cost_usd": 0.01, "model": "m", "family": "f",
                            "in_tokens": 1, "out_tokens": 1}) + "\n")
        f.write(json.dumps({"ts": peak_ts, "cost_usd": 0.05, "model": "m", "family": "f",
                            "in_tokens": 1, "out_tokens": 1}) + "\n")
    sp = SCH.spend_by_period()
    assert sp["off_peak"]["cost_usd"] == 0.01 and sp["peak"]["cost_usd"] == 0.05


# ---- effort-routing ----------------------------------------------------------
def test_effort_maps_to_tier():
    assert EF.model_for_effort("low") == "deepseek-chat"
    assert EF.model_for_effort("high") == "deepseek-v4-pro"
    assert EF.model_for_effort("???") == "deepseek-reasoner"   # default med


def test_effort_steps_ordered_and_capped():
    steps = EF.effort_steps(max_effort="med")
    assert [m for m, _ in steps] == ["deepseek-chat", "deepseek-reasoner"]
    assert all(0 < t < 1 for _, t in steps)
    assert EF.escalation_models("high")[-1] == "deepseek-v4-pro"
