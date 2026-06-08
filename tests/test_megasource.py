"""tests megasource.py + prices.py (Fase 2). Sin red/API, fetch inyectado."""
import json

from mmorch import megasource, prices
from mmorch.cost import cost_usd
from mmorch.evolve import zone_of, apply_change, rollback


def test_effective_prices_falls_back_to_config(tmp_path):
    p = tmp_path / "prices.json"
    pin, pout = prices.effective_prices("deepseek-chat", path=p)   # sin override -> config
    assert pin == 0.14 and pout == 0.28


def test_override_changes_effective_price(tmp_path):
    p = tmp_path / "prices.json"
    p.write_text(json.dumps({"deepseek-chat": {"price_in": 0.20, "price_out": 0.50}}), encoding="utf-8")
    assert prices.effective_prices("deepseek-chat", path=p) == (0.20, 0.50)


def test_diff_detects_changes(tmp_path):
    p = tmp_path / "prices.json"
    fetched = {"deepseek-chat": {"price_in": 0.99, "price_out": 0.28}}   # in cambió
    d = megasource.diff_prices(fetched, path=p)
    assert "deepseek-chat" in d and d["deepseek-chat"]["new"][0] == 0.99


def test_propose_builds_yellow_reversible_change(tmp_path, monkeypatch):
    p = tmp_path / "prices.json"
    monkeypatch.setattr(prices, "PRICES_PATH", p)
    monkeypatch.setattr(megasource, "PRICES_PATH", p)
    fetch = lambda: {"deepseek-chat": {"price_in": 0.99, "price_out": 0.28}}
    res = megasource.propose_price_update(fetch, path=p)
    assert res["n_changed"] == 1
    ch = res["change"]
    assert ch.target == "prices.json"
    # zona: prices.json NO está en red-list, contenido sin acciones peligrosas -> verde/amarillo
    assert zone_of(ch, root=tmp_path) in ("green", "yellow")
    # reversible: aplicar y revertir
    apply_change(ch, root=tmp_path)
    loaded = json.loads((tmp_path / "prices.json").read_text())
    assert loaded["deepseek-chat"]["price_in"] == 0.99
    assert rollback(ch, root=tmp_path)


def test_cost_uses_override(tmp_path, monkeypatch):
    p = tmp_path / "prices.json"
    p.write_text(json.dumps({"deepseek-chat": {"price_in": 1.0, "price_out": 1.0}}), encoding="utf-8")
    monkeypatch.setattr(prices, "PRICES_PATH", p)
    # 1M in + 1M out a $1/$1 = $2
    assert abs(cost_usd("deepseek-chat", 1_000_000, 1_000_000) - 2.0) < 1e-9
