"""tests budget.py — BudgetKeeper (ataca el +$5). Sin API, metrics mockeado."""
import pytest

from mmorch import budget


_FAKE = [
    {"iso": "2026-06-01T10:00:00", "cost_usd": 3.0},
    {"iso": "2026-06-15T10:00:00", "cost_usd": 2.5},
    {"iso": "2026-05-20T10:00:00", "cost_usd": 9.9},   # otro mes, no cuenta
]


def _patch(monkeypatch, limit):
    monkeypatch.setattr(budget, "read_events", lambda: _FAKE)
    if limit is None:
        monkeypatch.delenv("MMORCH_MAX_MONTHLY_USD", raising=False)
    else:
        monkeypatch.setenv("MMORCH_MAX_MONTHLY_USD", str(limit))


def test_monthly_spend_filters_month(monkeypatch):
    _patch(monkeypatch, None)
    assert budget.monthly_spend("2026-06") == 5.5
    assert budget.monthly_spend("2026-05") == 9.9


def test_no_limit_is_noop(monkeypatch):
    _patch(monkeypatch, None)
    budget.check()  # no raise
    assert budget.remaining() is None


def test_over_limit_blocks(monkeypatch):
    _patch(monkeypatch, 5.0)
    monkeypatch.setattr(budget, "monthly_spend", lambda *a, **k: 5.5)
    with pytest.raises(budget.BudgetExceeded):
        budget.check()


def test_critical_and_override_bypass(monkeypatch):
    _patch(monkeypatch, 5.0)
    monkeypatch.setattr(budget, "monthly_spend", lambda *a, **k: 5.5)
    budget.check(critical=True)    # zona roja aprobada
    budget.check(override=True)    # forzado humano


def test_under_limit_passes(monkeypatch):
    _patch(monkeypatch, 100.0)
    monkeypatch.setattr(budget, "monthly_spend", lambda *a, **k: 5.5)
    budget.check(est_cost=1.0)     # 6.5 < 100, no raise
    assert budget.remaining() == round(100.0 - 5.5, 6)


def test_status(monkeypatch):
    _patch(monkeypatch, 10.0)
    monkeypatch.setattr(budget, "monthly_spend", lambda *a, **k: 5.5)
    s = budget.status()
    assert s["enforced"] and s["limit"] == 10.0 and s["remaining"] == 4.5
