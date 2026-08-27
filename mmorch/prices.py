"""prices — capa de OVERRIDE de precios (datos volátiles, separados del código).

config.py (código) está en zona ROJA (cambiarlo = gate humano). Los PRECIOS cambian
seguido ("VOLATILE — re-verify"). Solución: un override de DATOS en `prices.json` (raíz
del repo) que cost lee primero. Actualizar prices.json = zona AMARILLA (reversible),
sin tocar config.py. La megafuente (megasource.py) propone updates a este archivo.
"""
from __future__ import annotations

import json
import logging
from datetime import datetime
from pathlib import Path

from .config import spec

from .paths import data_dir

PRICES_PATH = data_dir() / "prices.json"

_log = logging.getLogger(__name__)

# W3.4: los precios son VOLATILES — un budget defendible exige saber de CUANDO es el
# precio con el que estima. price_asof (YYYY-MM-DD) por modelo en prices.json; mas
# viejo que esto => warning UNA vez por modelo por proceso (no spamea el hot path).
_STALE_DAYS = 90
_WARNED: set[str] = set()


def _warn_if_stale(model_key: str, ov: dict | None) -> None:
    if not ov or model_key in _WARNED:
        return
    asof = ov.get("price_asof")
    if not asof:
        return
    try:
        age = (datetime.now() - datetime.strptime(str(asof), "%Y-%m-%d")).days
    except ValueError:
        _WARNED.add(model_key)
        _log.warning("prices.json: price_asof invalido para %s: %r (esperado YYYY-MM-DD)",
                     model_key, asof)
        return
    if age > _STALE_DAYS:
        _WARNED.add(model_key)
        _log.warning(
            "prices.json: precio de %s tiene %d dias (price_asof=%s, umbral %d) — "
            "re-verificar contra el proveedor antes de confiar en el budget",
            model_key, age, asof, _STALE_DAYS)


def load_overrides(path: Path | None = None) -> dict:
    """{model: {"price_in": x, "price_out": y}} o {} si no hay archivo. path=None →
    PRICES_PATH resuelto en runtime (permite override por monkeypatch/config)."""
    p = Path(path if path is not None else PRICES_PATH)
    if not p.exists():
        return {}
    try:
        return json.loads(p.read_text(encoding="utf-8"))
    except Exception:
        return {}


def effective_prices(model_key: str, path: Path | None = None) -> tuple[float, float]:
    """(price_in, price_out) — el override de prices.json si existe, si no el de config."""
    ov = load_overrides(path).get(model_key)
    _warn_if_stale(model_key, ov)
    s = spec(model_key)
    if ov and "price_in" in ov and "price_out" in ov:
        return float(ov["price_in"]), float(ov["price_out"])
    return s.price_in, s.price_out


def effective_cache_price(model_key: str, path: Path | None = None) -> float:
    """Precio por 1M de tokens de input CACHEADOS (cache-hit). DeepSeek cobra el input
    cacheado ~50x mas barato que el miss. Vive en prices.json (datos, zona amarilla) pa no
    tocar config.py (rojo). Fallback = price_in (sin descuento) -> backwards-compatible."""
    ov = load_overrides(path).get(model_key)
    if ov and "price_cache_in" in ov:
        return float(ov["price_cache_in"])
    return effective_prices(model_key, path)[0]
