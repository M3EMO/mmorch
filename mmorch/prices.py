"""prices — capa de OVERRIDE de precios (datos volátiles, separados del código).

config.py (código) está en zona ROJA (cambiarlo = gate humano). Los PRECIOS cambian
seguido ("VOLATILE — re-verify"). Solución: un override de DATOS en `prices.json` (raíz
del repo) que cost lee primero. Actualizar prices.json = zona AMARILLA (reversible),
sin tocar config.py. La megafuente (megasource.py) propone updates a este archivo.
"""
from __future__ import annotations

import json
from pathlib import Path

from .config import spec

ROOT = Path(__file__).resolve().parent.parent
PRICES_PATH = ROOT / "prices.json"


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
