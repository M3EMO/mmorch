"""budget — BudgetKeeper: techo de gasto mensual (ataca el incidente +$5).

mmorch podía gastar API $ sin tope. Acá: límite mensual desde env
`MMORCH_MAX_MONTHLY_USD`. Antes de una call gasta-API, `check()` suma el gasto del
mes en curso (de metrics.jsonl) y BLOQUEA si excede — salvo override humano o call
crítica (zona roja con aprobación). Default sin env = ilimitado = sin cambio de
comportamiento (opt-in).

Honestidad de costo: metrics.jsonl es un PISO (calls timeouteadas loggean cost=0 pero
el server factura). El BudgetKeeper es conservador igual — mejor frenar de más que el +$5.
"""
from __future__ import annotations

import os
import threading
from datetime import datetime

from .iohelpers import read_jsonl_cached
from .metrics import read_events, rotated_paths


class BudgetExceeded(RuntimeError):
    """El gasto del mes superó MMORCH_MAX_MONTHLY_USD y la call no es crítica/override."""


_SPEND_LOCK = threading.Lock()
# (id(events), len(events), month) -> total. `events` viene de read_events(), que ya
# cachea por (mtime_ns, size) — misma lista (mismo id) == mismo archivo sin cambios.
# Así el acumulador se apoya en la MISMA invalidación sin duplicar el stat del path acá,
# y no re-suma 13.5k eventos por call (budget.check() corre antes de CADA API call,
# ×8 concurrente en fan_out). len() de más evita colisiones si algún día se reusa un id.
_SPEND_CACHE: dict[tuple, float] = {}


def max_monthly_usd() -> float | None:
    """Límite mensual desde env. None = ilimitado (no enforcement)."""
    v = os.getenv("MMORCH_MAX_MONTHLY_USD")
    if v is None or v.strip() == "":
        return None
    try:
        return float(v)
    except ValueError:
        return None


def _month_cost(events: list[dict], month: str) -> float:
    total = 0.0
    for e in events:
        iso = e.get("iso", "")
        if isinstance(iso, str) and iso.startswith(month):
            total += e.get("cost_usd", 0.0) or 0.0
    return total


def monthly_spend(month: str | None = None) -> float:
    """Suma cost_usd del mes (YYYY-MM) sobre metrics.jsonl + TODOS los segmentos rotados
    (W3.4: la rotacion por tamaño no puede "borrar" gasto del mes en curso o el techo
    mensual se resetea solo). month=None → mes actual."""
    if month is None:
        month = datetime.now().strftime("%Y-%m")
    events = read_events()
    rotated = rotated_paths()
    # los segmentos rotados son inmutables -> su identidad (nombres) basta en la key
    cache_key = (id(events), len(events), tuple(p.name for p in rotated), month)
    with _SPEND_LOCK:
        cached = _SPEND_CACHE.get(cache_key)
        if cached is not None:
            return cached
    total = _month_cost(events, month)
    for p in rotated:
        total += _month_cost(read_jsonl_cached(p), month)   # parse cacheado por (mtime, size)
    total = round(total, 6)
    with _SPEND_LOCK:
        _SPEND_CACHE[cache_key] = total
    return total


def remaining() -> float | None:
    """USD que quedan en el mes. None si no hay límite."""
    lim = max_monthly_usd()
    if lim is None:
        return None
    return round(max(0.0, lim - monthly_spend()), 6)


def check(*, est_cost: float = 0.0, critical: bool = False, override: bool = False) -> None:
    """Gate: lanza BudgetExceeded si el gasto del mes (+est) supera el límite y la call
    no es crítica ni tiene override humano. No-op si no hay límite configurado."""
    lim = max_monthly_usd()
    if lim is None or critical or override:
        return
    if monthly_spend() + max(0.0, est_cost) > lim:
        raise BudgetExceeded(
            f"gasto mensual ${monthly_spend():.4f} (+est ${est_cost:.4f}) supera el "
            f"límite ${lim:.2f}. Pasá critical=True (zona roja aprobada) u override=True "
            f"para forzar, o subí MMORCH_MAX_MONTHLY_USD.")


def status() -> dict:
    """Resumen pa observabilidad/CLI."""
    lim = max_monthly_usd()
    return {"month": datetime.now().strftime("%Y-%m"), "spent": monthly_spend(),
            "limit": lim, "remaining": remaining(),
            "enforced": lim is not None}
