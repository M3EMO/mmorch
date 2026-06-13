"""Cost model — USD from token counts, using REGISTRY prices.

Cost in USD is the *secondary* objective. The primary scarce resource is Claude
cupo (§1); external API dollars are the lever that frees it. This module tracks
the dollar side; cupo is tracked separately (it is the Claude plan, not metered
here — proxied via the fact that external nodes spend zero cupo).
"""
from __future__ import annotations

def cost_usd(model_key: str, in_tokens: int, out_tokens: int,
             cached_tokens: int = 0) -> float:
    """USD de la call. cached_tokens = input servido del cache (cache-hit), facturado al
    precio cache (mucho mas barato en DeepSeek). cached_tokens=0 -> identico al modelo viejo
    (backwards-compatible). Antes se cobraba TODO el input a price_in -> sobre-contaba costo
    cuando habia cache-hit; esto lo corrige y vuelve medible el ahorro por caching."""
    from .prices import effective_prices, effective_cache_price
    pin, pout = effective_prices(model_key)
    cached = max(0, min(cached_tokens, in_tokens))
    miss = in_tokens - cached
    pcache = effective_cache_price(model_key)
    return (miss * pin + cached * pcache + out_tokens * pout) / 1_000_000.0
