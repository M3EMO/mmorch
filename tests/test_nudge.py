"""nudge: cada N loops cerrados dispara mantenimiento de memoria (Hermes nudging)."""
import sys, pathlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import pytest

import mmorch.nudge as N


@pytest.fixture(autouse=True)
def _distill_hermetico(monkeypatch):
    # POR QUE: tick() nudgeado tambien llama memory.distill_backlog, que dispara
    # providers.call REAL. Sin este mock, cada nudge del test golpeaba la API (o
    # fallaba 3 veces y ABRIA el breaker de deepseek-chat), y test_providers caia
    # con BreakerOpen si corria dentro del cooldown de 60s — flaky orden/timing
    # medido 2026-08-27 en la suite completa. Cero API en tests: siempre mockeado.
    monkeypatch.setattr("mmorch.memory.distill_backlog",
                        lambda after_id=0, limit=5: {"last_id": after_id, "distilled": 0})


def test_tick_counts_and_fires_every_n(tmp_path, monkeypatch):
    p = tmp_path / "nudge.json"
    fired = []
    monkeypatch.setattr("mmorch.memory.consolidate",
                        lambda scope=None, **k: fired.append(1) or {"merged": []})
    for i in range(1, 7):
        r = N.tick(every=3, path=p)
        assert r["closes"] == i
        assert r["nudged"] == (i % 3 == 0)
    assert len(fired) == 2          # disparo en cierre 3 y 6


def test_no_consolidate_flag(tmp_path):
    p = tmp_path / "n.json"
    r = N.tick(every=1, path=p, do_consolidate=False)
    assert r["nudged"] and r["report"] is None


def test_status_reports_next_in(tmp_path):
    p = tmp_path / "n.json"
    N.tick(every=10, path=p)
    st = N.status(path=p)
    assert st["closes"] == 1 and st["next_in"] == 9


def test_tick_survives_consolidate_error(tmp_path, monkeypatch):
    p = tmp_path / "n.json"
    def boom(*a, **k):
        raise RuntimeError("db locked")
    monkeypatch.setattr("mmorch.memory.consolidate", boom)
    r = N.tick(every=1, path=p)        # no debe propagar
    assert r["nudged"] and "error" in r["report"]
