"""Observabilidad de fallos: error_class (rate_limit/budget_cap/timeout) + error_rates().
Señal MEDIDA prerequisito de cualquier futuro load-balancing (anti-scope-creep). No rutea."""
import sys, pathlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import mmorch.metrics as MET
import mmorch.providers as P
import mmorch.budget as B


# ---- _classify_error ---------------------------------------------------------
class _FakeRL(Exception):
    status_code = 429


def test_classify_rate_limit_by_status():
    assert P._classify_error(_FakeRL("boom")) == "rate_limit"


def test_classify_rate_limit_by_name_and_msg():
    class RateLimitError(Exception):
        pass
    assert P._classify_error(RateLimitError("x")) == "rate_limit"
    assert P._classify_error(Exception("HTTP 429 Too Many Requests")) == "rate_limit"
    assert P._classify_error(Exception("rate limit exceeded")) == "rate_limit"


def test_classify_timeout_and_other():
    class APITimeoutError(Exception):
        pass
    assert P._classify_error(APITimeoutError("slow")) == "timeout"
    assert P._classify_error(ValueError("bad json")) == "other"


# ---- error_rates over a synthetic window ------------------------------------
def _seed_log(tmp_path, monkeypatch):
    p = tmp_path / "metrics.jsonl"
    monkeypatch.setattr(MET, "_LOG_DIR", tmp_path)
    monkeypatch.setattr(MET, "_LOG_PATH", p)
    ev = lambda **k: MET.log_event(pattern="t", node="n", phase="", **k)
    # deepseek: 2 ok, 1 rate_limit, 1 budget_cap
    ev(model="deepseek-chat", family="deepseek", in_tokens=10, out_tokens=5, cost_usd=0.001, latency_s=0.1)
    ev(model="deepseek-chat", family="deepseek", in_tokens=10, out_tokens=5, cost_usd=0.001, latency_s=0.1)
    ev(model="deepseek-chat", family="deepseek", in_tokens=0, out_tokens=0, cost_usd=0.0, latency_s=0.1,
       error="RateLimitError", error_class="rate_limit")
    ev(model="deepseek-chat", family="deepseek", in_tokens=0, out_tokens=0, cost_usd=0.0, latency_s=0.0,
       error="BudgetExceeded", error_class="budget_cap")
    # gemini: 1 ok
    ev(model="gemini-3.1-flash-lite", family="google", in_tokens=8, out_tokens=4, cost_usd=0.0005, latency_s=0.2)
    return p


def test_error_rates_per_model(tmp_path, monkeypatch):
    _seed_log(tmp_path, monkeypatch)
    r = MET.error_rates(window_n=200)
    ds = r["by_model"]["deepseek-chat"]
    assert ds["calls"] == 4 and ds["rate_limit"] == 1 and ds["budget_cap"] == 1
    assert ds["rate_limit_rate"] == 0.25 and ds["budget_cap_rate"] == 0.25
    assert r["by_family"]["deepseek"]["rate_limit"] == 1
    gm = r["by_model"]["gemini-3.1-flash-lite"]
    assert gm["calls"] == 1 and gm["rate_limit"] == 0 and gm["error_rate"] == 0.0


def test_error_rates_window_n_limits(tmp_path, monkeypatch):
    _seed_log(tmp_path, monkeypatch)
    # ventana de 1 -> solo el ultimo evento (gemini ok), deepseek no aparece
    r = MET.error_rates(window_n=1)
    assert r["window_events"] == 1 and "deepseek-chat" not in r["by_model"]


# ---- providers wiring: el except y el budget-cap loggean error_class ---------
def test_call_logs_rate_limit_class(monkeypatch):
    cap = {}
    monkeypatch.setattr(P, "log_event", lambda **k: cap.update(k))

    class _Client:
        class chat:
            class completions:
                @staticmethod
                def create(**kw):
                    raise _FakeRL("429 slow down")
    monkeypatch.setattr(P, "_client", lambda mk: _Client())
    monkeypatch.setattr(B, "check", lambda **k: None)   # sin budget block
    try:
        P.call("deepseek-chat", "hola", pattern="t")
    except Exception:
        pass
    assert cap.get("error_class") == "rate_limit"


def test_call_logs_budget_cap_class(monkeypatch):
    cap = {}
    monkeypatch.setattr(P, "log_event", lambda **k: cap.update(k))
    def _boom(**k):
        raise B.BudgetExceeded("over")
    monkeypatch.setattr(B, "check", _boom)
    try:
        P.call("deepseek-chat", "hola", pattern="t")
    except B.BudgetExceeded:
        pass
    assert cap.get("error_class") == "budget_cap"
