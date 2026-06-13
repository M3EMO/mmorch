"""schedule — ADVISORY de ventana off-peak (DeepSeek descuenta fuerte fuera de hora pico).
NO es un scheduler autonomo (eso roza red-zone: gasto programado sin humano en el flush).
Solo informa: '¿estamos en off-peak?' + parte el gasto por periodo pa MEDIR el ahorro.

El descuento real lo aplica DeepSeek server-side; aca no tocamos el billing (cost.py ya es
un piso). is_off_peak() deja que el CALLER (humano/Opus) decida diferir un batch no-urgente.

Ventana default = UTC 16:30-00:30 (DeepSeek; VOLATIL, re-verificar). Override por env
MMORCH_OFFPEAK_UTC='HH:MM-HH:MM' sin tocar codigo.
"""
from __future__ import annotations

import os
from datetime import datetime, timezone, time as _time

_DEFAULT_WINDOW = "16:30-00:30"   # UTC, DeepSeek off-peak (re-verificar)


def _parse_window(s: str) -> tuple[_time, _time]:
    a, b = s.split("-")
    ah, am = (int(x) for x in a.split(":"))
    bh, bm = (int(x) for x in b.split(":"))
    return _time(ah, am), _time(bh, bm)


def off_peak_window() -> tuple[_time, _time]:
    return _parse_window(os.getenv("MMORCH_OFFPEAK_UTC", _DEFAULT_WINDOW))


def is_off_peak(now: datetime | None = None) -> bool:
    """True si AHORA (UTC) cae en la ventana off-peak. Maneja ventanas que cruzan medianoche."""
    now = now or datetime.now(timezone.utc)
    t = now.timetz().replace(tzinfo=None) if now.tzinfo else now.time()
    start, end = off_peak_window()
    if start <= end:
        return start <= t < end
    return t >= start or t < end          # cruza medianoche (16:30-00:30)


def advisory(est_cost: float = 0.0, now: datetime | None = None) -> dict:
    """Sugerencia (no accion): si NO es off-peak y el batch no es urgente, diferir ahorra.
    Devuelve {off_peak, window_utc, hint}. El caller decide; mmorch nunca difiere solo."""
    off = is_off_peak(now)
    s, e = off_peak_window()
    win = f"{s.strftime('%H:%M')}-{e.strftime('%H:%M')} UTC"
    hint = ("off-peak ahora: buen momento pa batches grandes" if off
            else f"hora pico: si el batch (~${est_cost:.4f}) no es urgente, diferir a {win} ahorra")
    return {"off_peak": off, "window_utc": win, "hint": hint}


def spend_by_period() -> dict:
    """Observabilidad: parte el gasto del log por off-peak vs pico (por ts UTC del evento).
    Vuelve MEDIBLE cuanto se gasto en cada ventana = base pa justificar diferir."""
    from .metrics import read_events
    out = {"off_peak": {"cost_usd": 0.0, "calls": 0}, "peak": {"cost_usd": 0.0, "calls": 0}}
    for e in read_events():
        ts = e.get("ts")
        if not ts:
            continue
        dt = datetime.fromtimestamp(ts, tz=timezone.utc)
        bucket = "off_peak" if is_off_peak(dt) else "peak"
        out[bucket]["cost_usd"] += e.get("cost_usd", 0.0) or 0.0
        out[bucket]["calls"] += 1
    for v in out.values():
        v["cost_usd"] = round(v["cost_usd"], 6)
    return out
