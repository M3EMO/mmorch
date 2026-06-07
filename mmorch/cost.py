"""Cost model — USD from token counts, using REGISTRY prices.

Cost in USD is the *secondary* objective. The primary scarce resource is Claude
cupo (§1); external API dollars are the lever that frees it. This module tracks
the dollar side; cupo is tracked separately (it is the Claude plan, not metered
here — proxied via the fact that external nodes spend zero cupo).
"""
from __future__ import annotations

from .config import spec


def cost_usd(model_key: str, in_tokens: int, out_tokens: int) -> float:
    s = spec(model_key)
    return (in_tokens * s.price_in + out_tokens * s.price_out) / 1_000_000.0
