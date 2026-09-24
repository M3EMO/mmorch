"""Escala de los caminos calientes de metrics.jsonl. Mide t(N) y t(10N)
en frio (caches vacios) y en caliente (segunda llamada), y falla si alguna razon
supera 25 con t(10N) mayor al piso de ruido. Lineal da ~10, cuadratico da ~100.
Medido en el banco de acople 2026-09-24 (vault/research/banco-de-acople-*)."""
import json
import time

import pytest

import mmorch.budget as B
import mmorch.iohelpers as IO
import mmorch.metrics as MET

N = 1000
SEG = 500           # eventos por segmento rotado; hay N // SEG segmentos
RATIO_MAX = 25.0
FLOOR_S = 0.05      # debajo de este t(10N) el ruido de Windows domina
REPS = 3


def _line(i: int) -> str:
    return json.dumps({
        "ts": 1_700_000_000.0 + i, "iso": time.strftime("%Y-%m") + "-01T10:00:00",
        "phase": "", "pattern": f"p{i % 7}", "node": "n", "model": f"m{i % 5}",
        "family": f"f{i % 3}", "in_tokens": 100 + i % 50, "out_tokens": 40,
        "cost_usd": 0.001, "latency_s": 1.0,
        "extra": {"cached_tokens": 10, "error_class": "rate_limit" if i % 11 == 0 else None},
    })


def _fill(d, n: int) -> None:
    d.mkdir(parents=True, exist_ok=True)
    (d / "metrics.jsonl").write_text("\n".join(_line(i) for i in range(n)) + "\n", encoding="utf-8")
    for s in range(n // SEG):
        body = "\n".join(_line(i) for i in range(SEG)) + "\n"
        (d / f"metrics-20260101-{s:05d}.jsonl").write_text(body, encoding="utf-8")


def _point(monkeypatch, d) -> None:
    monkeypatch.setattr(MET, "_LOG_DIR", d)
    monkeypatch.setattr(MET, "_LOG_PATH", d / "metrics.jsonl")


def _clear() -> None:
    IO._JSONL_CACHE.clear()
    B._SPEND_CACHE.clear()


HOT = {
    "read_jsonl_cached": lambda n: IO.read_jsonl_cached(MET.log_path()),
    "read_jsonl_tail": lambda n: IO.read_jsonl_tail(MET.log_path(), n // 2),
    "error_rates": lambda n: MET.error_rates(window_n=n // 2),
    "cache_stats": lambda n: MET.cache_stats(window_n=n // 2),
    "summary": lambda n: MET.summary(),
    "monthly_spend": lambda n: B.monthly_spend(),
}


def _times(fn, n: int) -> tuple[float, float]:
    cold, warm = [], []
    for _ in range(REPS):
        _clear()
        t = time.perf_counter(); fn(n); cold.append(time.perf_counter() - t)
        t = time.perf_counter(); fn(n); warm.append(time.perf_counter() - t)
    return min(cold), min(warm)


@pytest.mark.parametrize("name", list(HOT))
def test_escala_no_cuadratica(name, monkeypatch, tmp_path):
    fn = HOT[name]
    res = {}
    for n in (N, 10 * N):
        d = tmp_path / str(n)
        _fill(d, n)
        _point(monkeypatch, d)
        res[n] = _times(fn, n)
    _clear()
    for i, kind in enumerate(("frio", "caliente")):
        small, big = res[N][i], res[10 * N][i]
        ratio = big / max(small, 1e-6)
        assert not (ratio > RATIO_MAX and big > FLOOR_S), (
            f"{name} {kind}: t(N)={small:.4f}s t(10N)={big:.4f}s razon={ratio:.0f} > {RATIO_MAX}")
