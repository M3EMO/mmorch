"""Contratos de metrics.jsonl entre su escritor (log_event) y sus lectores.

Salen del banco de acople (vault: banco-de-acople-por-datos-*; worktree ../orch-bench,
scripts/bench_acople). Medidos sobre mutantes que el gate no detectaba: cierran 40-70% de
las fallas reales nuevas, con cero falsos positivos. Categorias (prefijo del test):
fw ventana, fu precision y unidades, fs segmentos rotados, fc cache, ft escala del tail."""
import json
import time

import pytest

import mmorch.auto_apply_nightly as AAN
import mmorch.budget as B
import mmorch.iohelpers as IO
import mmorch.learn as L
import mmorch.metrics as MET
import mmorch.schedule as S
import mmorch.sdlc as SD


@pytest.fixture(autouse=True)
def _aislado(monkeypatch, tmp_path):
    monkeypatch.setattr(MET, "_LOG_DIR", tmp_path)
    monkeypatch.setattr(MET, "_LOG_PATH", tmp_path / "metrics.jsonl")
    monkeypatch.setattr(B, "_SPEND_CACHE", {})
    monkeypatch.setattr(AAN, "_run", lambda *a, **k: True)
    monkeypatch.setattr(AAN, "_json_run",
                        lambda d, code, t: True if "health" in code else {"new": {"pass_rate": 1.0}})


def _ev(**kw):
    rec = dict(pattern="p", node="n", model="new", family="f1", in_tokens=100,
               out_tokens=40, cost_usd=0.0123, latency_s=1.25)
    rec.update(kw)
    MET.log_event(**rec)


def _ventana(n: int, viejos: int = 3) -> list[float]:
    """`viejos` eventos fuera de la ventana (modelo old, caros, con error) y n adentro
    con costos distintos. Devuelve los costos de adentro."""
    for _ in range(viejos):
        _ev(model="old", cost_usd=9.0, error_class="rate_limit")
    costos = [round((i + 1) * 1e-4, 6) for i in range(n)]
    for c in costos:
        _ev(cost_usd=c, cached_tokens=10)
    return costos


def test_fw_error_rates_toma_exactamente_los_ultimos_n():
    _ventana(7)
    r = MET.error_rates(window_n=7)
    assert r["window_events"] == 7
    assert set(r["by_model"]) == {"new"} and r["by_model"]["new"]["rate_limit"] == 0


def test_fw_cache_stats_toma_exactamente_los_ultimos_n():
    _ventana(7)
    r = MET.cache_stats(window_n=7)
    assert r["window_events"] == 7
    assert set(r["by_model"]) == {"new"} and r["by_model"]["new"]["calls"] == 7


def test_fw_tail_devuelve_exactamente_los_ultimos_n():
    _ventana(7)
    rows = IO.read_jsonl_tail(MET.log_path(), 7)
    assert len(rows) == 7 and {r["model"] for r in rows} == {"new"}


def test_fw_collect_signals_ventana_de_200():
    costos = _ventana(200)
    sig = AAN.collect_signals(".")
    assert sig["cost_per_call"] == pytest.approx(sum(costos) / 200)
    assert sig["error_rate"] == 0.0


def test_fw_ledger_usd_ve_lo_reciente_con_log_grande(monkeypatch):
    monkeypatch.setattr(SD, "PHASE", "fx")
    monkeypatch.setattr(SD, "state", {"t0_epoch": 0})
    viejo = json.dumps({"ts": 1.0, "phase": "otra", "cost_usd": 9.0, "family": "f1"})
    MET.log_path().write_text((viejo + "\n") * 8005, encoding="utf-8")
    _ev(phase="fx")
    _ev(phase="fx")
    assert SD.ledger_usd() == pytest.approx(0.0246, abs=1e-5)


def test_fw_ledger_usd_suma_toda_la_corrida(monkeypatch):
    monkeypatch.setattr(SD, "PHASE", "fx")
    monkeypatch.setattr(SD, "state", {"t0_epoch": 0})
    ev = json.dumps({"ts": 2.0, "phase": "fx", "cost_usd": 0.001, "family": "f1"})
    MET.log_path().write_text((ev + "\n") * 900, encoding="utf-8")
    assert SD.ledger_usd() == pytest.approx(0.9, abs=1e-4)


def test_fw_tail_varios_bloques_en_orden(tmp_path):
    p = tmp_path / "t.jsonl"
    p.write_text("".join(json.dumps({"i": i, "pad": "x" * 40}) + "\n" for i in range(2000)), encoding="utf-8")
    rows = IO.read_jsonl_tail(p, 300, chunk_size=1024)
    assert [r["i"] for r in rows] == list(range(1700, 2000))


def test_fu_error_rates_son_fracciones():
    for i in range(4):
        _ev(error_class="rate_limit" if i == 0 else None)
    m = MET.error_rates(window_n=4)["by_model"]["new"]
    assert m["rate_limit_rate"] == pytest.approx(0.25) and m["error_rate"] == pytest.approx(0.25)


def test_fu_verdict_confidence_misma_escala():
    for conf in (0.8, 0.6):
        _ev(pattern="adversarial_verify_verdict", model="v", passed=True, confidence=conf)
    q = L._verdict_quality(MET.read_events())["v"]
    assert q["avg_confidence"] == pytest.approx(0.7) and q["pass_rate"] == 1.0


def _dos(**kw):
    _ev(**kw)
    _ev(**kw)


@pytest.mark.parametrize("lector", ["summary", "budget", "learn", "schedule", "sdlc", "aan"])
def test_fu_total_exacto_con_costos_reales(lector, monkeypatch):
    monkeypatch.setattr(SD, "PHASE", "fx")
    monkeypatch.setattr(SD, "state", {"t0_epoch": 0})
    _dos(phase="fx")
    total = {
        "summary": lambda: MET.summary()["total_cost_usd"],
        "budget": lambda: B.monthly_spend(),
        "learn": lambda: L.analyze()["total_cost_usd"],
        "schedule": lambda: sum(v["cost_usd"] for v in S.spend_by_period().values()),
        "sdlc": lambda: SD.ledger_usd(),
        "aan": lambda: AAN.collect_signals(".")["cost_per_call"] * 2,
    }[lector]()
    assert total == pytest.approx(0.0246, abs=1e-5)


def test_fu_learn_latencia_en_segundos():
    _ev(latency_s=1.25)
    _ev(latency_s=2.75)
    row = next(r for r in L.analyze()["rows"] if r["model"] == "new")
    assert row["lat_p50"] == pytest.approx(2.0)


def test_fs_monthly_spend_con_dos_segmentos(tmp_path):
    iso = time.strftime("%Y-%m") + "-01T10:00:00"
    for name in ("metrics-20260101-000000.jsonl", "metrics-20260102-000000.jsonl"):
        (tmp_path / name).write_text(json.dumps({"iso": iso, "cost_usd": 1.0}) + "\n", encoding="utf-8")
    _ev(cost_usd=2.0)
    assert B.monthly_spend() == pytest.approx(4.0)


def test_fc_monthly_spend_no_recalcula_si_nada_cambio(monkeypatch, tmp_path):
    iso = time.strftime("%Y-%m") + "-01T10:00:00"
    (tmp_path / "metrics-20260101-000000.jsonl").write_text(
        json.dumps({"iso": iso, "cost_usd": 1.0}) + "\n", encoding="utf-8")
    _ev()
    parseos, sumas = [], []
    tol, mc = IO.read_jsonl_tolerant, B._month_cost
    monkeypatch.setattr(IO, "read_jsonl_tolerant", lambda p: parseos.append(p) or tol(p))
    monkeypatch.setattr(B, "_month_cost", lambda ev, m: sumas.append(m) or mc(ev, m))
    primero = B.monthly_spend()
    n_parseos, n_sumas = len(parseos), len(sumas)
    assert B.monthly_spend() == primero
    assert len(parseos) == n_parseos and len(sumas) == n_sumas


def test_ft_tail_no_crece_con_el_archivo(tmp_path):
    line = json.dumps({"ts": 1.0, "model": "m", "cost_usd": 0.001, "pad": "x" * 150})
    t = {}
    for n in (5000, 50000):
        p = tmp_path / f"tail-{n}.jsonl"
        p.write_text((line + "\n") * n, encoding="utf-8")
        reps = []
        for _ in range(5):
            t0 = time.perf_counter()
            IO.read_jsonl_tail(p, 100)
            reps.append(time.perf_counter() - t0)
        t[n] = min(reps)
    ratio = t[50000] / max(t[5000], 1e-6)
    assert not (ratio > 3 and t[50000] > 0.005), f"tail crece con el archivo: razon {ratio:.1f}, {t[50000]:.4f}s"
