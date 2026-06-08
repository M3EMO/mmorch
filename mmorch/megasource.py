"""megasource (Fase 2) — megafuente autodidacta: primer hit = provider PRICING.

mmorch fetchea precios de una fuente ESTRUCTURADA (no scraping con captcha), los destila
a {model: {price_in, price_out}}, y PROPONE actualizar `prices.json` (zona AMARILLA,
reversible) — sin tocar config.py (rojo). Caza el +$5 y "precios volátiles". El cambio
pasa por el gate de evolve (evaluate/zona/apply). Extensible: docs de APIs, benchmarks,
drift de usage propio.

`fetch_fn` es INYECTABLE: en prod = leer un YAML/repo público/webhook oficial; en tests =
un dict. No hardcodea un endpoint frágil.
"""
from __future__ import annotations

import json

from .config import REGISTRY
from .prices import load_overrides, PRICES_PATH
from .evolve import snapshot_change, Change


def fetch_prices(fetch_fn=None) -> dict:
    """Devuelve {model: {price_in, price_out}} desde una fuente estructurada. Sin fetch_fn,
    usa los precios ACTUALES de config (no-op: útil pa testear el pipeline sin red)."""
    if fetch_fn is not None:
        return dict(fetch_fn())
    return {k: {"price_in": s.price_in, "price_out": s.price_out} for k, s in REGISTRY.items()}


def diff_prices(fetched: dict, path=PRICES_PATH) -> dict:
    """Qué precios CAMBIAN vs lo efectivo hoy (override o config). {model: {old, new}}."""
    cur = load_overrides(path)
    out = {}
    for k, new in fetched.items():
        if k not in REGISTRY:
            continue
        s = REGISTRY[k]
        old_in = cur.get(k, {}).get("price_in", s.price_in)
        old_out = cur.get(k, {}).get("price_out", s.price_out)
        ni, no = float(new["price_in"]), float(new["price_out"])
        if (ni, no) != (old_in, old_out):
            out[k] = {"old": [old_in, old_out], "new": [ni, no]}
    return out


def propose_price_update(fetch_fn=None, path=PRICES_PATH) -> dict:
    """Pipeline: fetch -> diff -> arma un Change a prices.json (zona amarilla, reversible).
    Devuelve {change, diff, n_changed}. NO aplica — eso pasa por el gate de evolve."""
    fetched = fetch_prices(fetch_fn)
    d = diff_prices(fetched, path)
    # merge sobre el override actual (no pisar modelos no fetcheados)
    merged = dict(load_overrides(path))
    for k, v in fetched.items():
        if k in REGISTRY:
            merged[k] = {"price_in": float(v["price_in"]), "price_out": float(v["price_out"])}
    after = json.dumps(merged, ensure_ascii=False, indent=2)
    desc = (f"Actualizar prices.json (override de datos, zona amarilla, reversible) con "
            f"{len(d)} precio(s) cambiado(s) desde la megafuente: {list(d)}. No toca config.py.")
    change = snapshot_change("prices.json", after, desc)
    return {"change": change, "diff": d, "n_changed": len(d)}
