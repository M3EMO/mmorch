"""Observability — append-only JSONL metric log (§11 backbone).

Without these metrics you cannot route, nor falsify the break-even D_total > U_max.
Every node call logs one record. The log is the input to the parametric cost sheet
and to the A/B/C ablation (§18.4).

Logged per call: timestamp, phase, pattern, node label, model, family,
in/out tokens, cost_usd, latency_s, plus any extra fields.
"""
from __future__ import annotations

import json
import threading
import time
from pathlib import Path

from .iohelpers import read_jsonl_cached, read_jsonl_tail

_LOCK = threading.Lock()

from .paths import logs_dir

_LOG_DIR = logs_dir()
_LOG_PATH = _LOG_DIR / "metrics.jsonl"


def log_path() -> Path:
    return _LOG_PATH


# --- rotacion (W3.4): metrics.jsonl crece sin tope (append-only). >50MB se renombra
# con timestamp; los segmentos rotados son INMUTABLES y el budget re-deriva el gasto
# del mes desde TODOS los segmentos (rotated_paths) — rotar nunca "borra" gasto.
_ROTATE_MAX_BYTES = 50 * 1024 * 1024


def rotated_paths() -> list[Path]:
    """Segmentos rotados metrics-*.jsonl, orden por nombre (= cronologico)."""
    return sorted(_LOG_DIR.glob("metrics-*.jsonl"))


def _rotate_if_needed() -> None:
    # llamado bajo _LOCK (antes del append). Fallo de rotacion = side-channel: nunca
    # debe frenar el logging del evento (mejor un archivo gordo que un evento perdido).
    try:
        if _LOG_PATH.exists() and _LOG_PATH.stat().st_size > _ROTATE_MAX_BYTES:
            stamp = time.strftime("%Y%m%d-%H%M%S")
            target = _LOG_DIR / f"metrics-{stamp}.jsonl"
            n = 0
            while target.exists():          # dos rotaciones en el mismo segundo
                n += 1
                target = _LOG_DIR / f"metrics-{stamp}-{n}.jsonl"
            _LOG_PATH.rename(target)
    except OSError:
        pass


def log_event(
    *,
    pattern: str,
    node: str,
    model: str,
    family: str,
    in_tokens: int,
    out_tokens: int,
    cost_usd: float,
    latency_s: float,
    phase: str = "",
    **extra,
) -> None:
    record = {
        "ts": time.time(),
        "iso": time.strftime("%Y-%m-%dT%H:%M:%S", time.localtime()),
        "phase": phase,
        "pattern": pattern,
        "node": node,
        "model": model,
        "family": family,
        "in_tokens": in_tokens,
        "out_tokens": out_tokens,
        "cost_usd": round(cost_usd, 8),
        "latency_s": round(latency_s, 4),
    }
    if extra:
        record["extra"] = extra
    line = json.dumps(record, ensure_ascii=False)
    with _LOCK:
        _LOG_DIR.mkdir(parents=True, exist_ok=True)
        _rotate_if_needed()
        with open(_LOG_PATH, "a", encoding="utf-8") as fh:
            fh.write(line + "\n")


def read_events() -> list[dict]:
    # Ticket 13 (audit-2026-08): cacheado por (mtime_ns, size) — metrics.jsonl es
    # append-only sin rotación, así que "no cambió de tamaño/mtime" == "misma historia".
    return read_jsonl_cached(_LOG_PATH)


def error_rates(*, window_n: int | None = 200, window_s: float | None = None) -> dict:
    """429-rate y budget-cap-hit-rate por modelo/familia sobre una ventana reciente.

    Señal de observabilidad PURA (no rutea nada): es el prerequisito MEDIDO que cualquier
    futuro load-balancing tendría que citar pa justificarse bajo anti-scope-creep. El
    error_class lo pone providers._classify_error / el gate de budget.

    window_n: últimos N eventos (default 200). window_s: solo eventos de los últimos S
    segundos (si se da, se aplica ADEMÁS de window_n). Denominador = todos los eventos del
    modelo en la ventana (éxitos + errores + cap-hits) = tasa sobre intentos reales.

    Ticket 13: cuando solo se pide window_n (el caso caliente — intuition.healthy() en
    cada route()), lee el TAIL del archivo directo en vez de parsear la historia completa
    vía read_events(). window_s exige mirar más atrás que N líneas (no sabemos cuántas
    entran en S segundos) así que ahí cae al read_events() cacheado (igual de rápido en
    calls repetidas, sólo el primer parse post-mtime-change paga el costo completo)."""
    if window_s is None and window_n is not None:
        events = read_jsonl_tail(log_path(), window_n)
    else:
        events = read_events()
        if window_s is not None:
            cut = time.time() - window_s
            events = [e for e in events if e.get("ts", 0) >= cut]
        if window_n is not None:
            events = events[-window_n:]

    def _blank() -> dict:
        return {"calls": 0, "rate_limit": 0, "budget_cap": 0, "timeout": 0, "other_error": 0}

    by_model: dict[str, dict] = {}
    by_family: dict[str, dict] = {}
    for e in events:
        m = e.get("model", "?"); fam = e.get("family", "?")
        bm = by_model.setdefault(m, _blank()); bf = by_family.setdefault(fam, _blank())
        bm["calls"] += 1; bf["calls"] += 1
        ec = (e.get("extra") or {}).get("error_class")
        if ec == "rate_limit":
            bm["rate_limit"] += 1; bf["rate_limit"] += 1
        elif ec == "budget_cap":
            bm["budget_cap"] += 1; bf["budget_cap"] += 1
        elif ec == "timeout":
            bm["timeout"] += 1; bf["timeout"] += 1
        elif (e.get("extra") or {}).get("error"):
            bm["other_error"] += 1; bf["other_error"] += 1

    def _rates(d: dict) -> dict:
        for v in d.values():
            n = v["calls"] or 1
            v["rate_limit_rate"] = round(v["rate_limit"] / n, 4)
            v["budget_cap_rate"] = round(v["budget_cap"] / n, 4)
            v["error_rate"] = round(
                (v["rate_limit"] + v["budget_cap"] + v["timeout"] + v["other_error"]) / n, 4)
        return d

    return {"window_events": len(events),
            "by_model": _rates(by_model), "by_family": _rates(by_family)}


def cache_stats(*, window_n: int | None = 500) -> dict:
    """Cache-hit-rate por modelo sobre la ventana: cached_tokens / in_tokens. Es el numero
    que vuelve FALSIFICABLE el ahorro por prompt-caching y prefix-stable. Observabilidad
    pura (no rutea). cached_tokens lo pone providers._cached_tokens en log_event."""
    events = read_events()
    if window_n is not None:
        events = events[-window_n:]
    by_model: dict[str, dict] = {}
    for e in events:
        if e.get("in_tokens", 0) <= 0:
            continue   # errores/cap-hits no cuentan pa hit-rate
        m = e.get("model", "?")
        d = by_model.setdefault(m, {"in_tokens": 0, "cached_tokens": 0, "calls": 0})
        d["in_tokens"] += e.get("in_tokens", 0)
        d["cached_tokens"] += (e.get("extra") or {}).get("cached_tokens", 0) or 0
        d["calls"] += 1
    for d in by_model.values():
        d["cache_hit_rate"] = round(d["cached_tokens"] / (d["in_tokens"] or 1), 4)
    return {"window_events": len(events), "by_model": by_model}


def summary() -> dict:
    """Aggregate the log: total cost, tokens, calls per family/model."""
    events = read_events()
    total_cost = 0.0
    by_family: dict[str, float] = {}
    by_model: dict[str, int] = {}
    for e in events:
        # .get() defensivo (W3.4): una linea incompleta (torn write, editada a mano)
        # no debe reventar el summary entero — se cuenta con lo que tenga.
        c = e.get("cost_usd", 0.0) or 0.0
        total_cost += c
        by_family[e.get("family", "?")] = by_family.get(e.get("family", "?"), 0.0) + c
        by_model[e.get("model", "?")] = by_model.get(e.get("model", "?"), 0) + 1
    return {
        "calls": len(events),
        "total_cost_usd": round(total_cost, 6),
        "cost_by_family": {k: round(v, 6) for k, v in by_family.items()},
        "calls_by_model": by_model,
    }
