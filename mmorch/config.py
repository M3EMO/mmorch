"""Model registry — single source of truth for models, families, endpoints, prices.

Prices: USD per 1M tokens (jun-2026 reference, §17 of design doc). VOLATILE —
re-verify before fixing budgets. Pairing rule (§4): every generator->verifier or
competitor->judge pair must span DIFFERENT families to decorrelate errors.
"""
from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class ModelSpec:
    key: str            # internal handle
    family: str         # anthropic | deepseek | moonshot | google
    provider: str       # billing/endpoint surface
    model_id: str       # API model identifier
    base_url: str | None
    api_key_env: str    # env var holding the key
    price_in: float     # USD / 1M input tokens
    price_out: float    # USD / 1M output tokens
    role: str           # design role (§4)


# OpenAI-compatible endpoints. Anthropic/Opus is NOT here on purpose: it is the
# orchestrator (Claude Code itself / cupo), never invoked as an external node.
REGISTRY: dict[str, ModelSpec] = {
    "deepseek-chat": ModelSpec(
        key="deepseek-chat",
        family="deepseek",
        provider="deepseek",
        model_id="deepseek-chat",          # V4 Flash, no-thinking
        base_url="https://api.deepseek.com/v1",
        api_key_env="DEEPSEEK_API_KEY",
        price_in=0.14,
        price_out=0.28,
        role="bulk generation (no-thinking)",
    ),
    "deepseek-reasoner": ModelSpec(
        key="deepseek-reasoner",
        family="deepseek",
        provider="deepseek",
        model_id="deepseek-reasoner",      # V4 Flash, thinking
        base_url="https://api.deepseek.com/v1",
        api_key_env="DEEPSEEK_API_KEY",
        price_in=0.14,
        price_out=0.28,
        role="independent reasoning/audit (thinking)",
    ),
    "gemini-2.5-flash": ModelSpec(
        key="gemini-2.5-flash",
        family="google",
        provider="google",
        model_id="gemini-2.5-flash",
        base_url="https://generativelanguage.googleapis.com/v1beta/openai/",
        api_key_env="GEMINI_API_KEY",
        price_in=0.30,
        price_out=2.50,
        role="cross-family verifier (Tier 2)",
    ),
    "gemini-2.5-flash-lite": ModelSpec(
        key="gemini-2.5-flash-lite",
        family="google",
        provider="google",
        model_id="gemini-2.5-flash-lite",
        base_url="https://generativelanguage.googleapis.com/v1beta/openai/",
        api_key_env="GEMINI_API_KEY",
        price_in=0.10,
        price_out=0.40,
        role="routing / classification",
    ),
    # --- Configured but inactive until keys exist (Moonshot/Kimi) ---
    "kimi-k2.5": ModelSpec(
        key="kimi-k2.5",
        family="moonshot",
        provider="moonshot",
        model_id="kimi-k2.5",
        base_url="https://api.moonshot.ai/v1",
        api_key_env="MOONSHOT_API_KEY",
        price_in=0.60,
        price_out=3.00,
        role="UI / bulk / synthesis / cheap nodes",
    ),
}

# Default node assignments for the MVP slice.
DEFAULT_GENERATOR = "deepseek-chat"
DEFAULT_VERIFIER = "gemini-2.5-flash"   # cross-family vs deepseek (config B, §18.4)
DEFAULT_ROUTER = "gemini-2.5-flash-lite"


def family_of(model_key: str) -> str:
    return REGISTRY[model_key].family


def spec(model_key: str) -> ModelSpec:
    try:
        return REGISTRY[model_key]
    except KeyError:
        raise KeyError(
            f"unknown model '{model_key}'. known: {sorted(REGISTRY)}"
        ) from None
