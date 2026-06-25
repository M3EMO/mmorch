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
    extra_body: tuple = ()  # request-body extras como tuple de pares (hashable);
                            # ej (("thinking", {"type": "disabled"}),) pa DeepSeek V4


# OpenAI-compatible endpoints. Anthropic/Opus is NOT here on purpose: it is the
# orchestrator (Claude Code itself / cupo), never invoked as an external node.
REGISTRY: dict[str, ModelSpec] = {
    # Keys internas estables (no romper brazos del bandit); model_id EXPLICITO v4 —
    # los alias deepseek-chat/reasoner ya no figuran en /models (deprecacion probable).
    "deepseek-chat": ModelSpec(
        key="deepseek-chat",
        family="deepseek",
        provider="deepseek",
        model_id="deepseek-v4-flash",      # explicito; thinking APAGADO pa bulk
        base_url="https://api.deepseek.com/v1",
        api_key_env="DEEPSEEK_API_KEY",
        price_in=0.14,
        price_out=0.28,
        role="bulk generation (no-thinking)",
        extra_body=(("thinking", {"type": "disabled"}),),
    ),
    "deepseek-reasoner": ModelSpec(
        key="deepseek-reasoner",
        family="deepseek",
        provider="deepseek",
        model_id="deepseek-v4-flash",      # explicito; thinking default (razonamiento)
        base_url="https://api.deepseek.com/v1",
        api_key_env="DEEPSEEK_API_KEY",
        price_in=0.14,
        price_out=0.28,
        role="independent reasoning/audit (thinking)",
    ),
    "deepseek-v4-pro": ModelSpec(
        key="deepseek-v4-pro",
        family="deepseek",
        provider="deepseek",
        model_id="deepseek-v4-pro",        # premium: ejecutor code-heavy (rubric_loop)
        base_url="https://api.deepseek.com/v1",
        api_key_env="DEEPSEEK_API_KEY",
        price_in=1.74,
        price_out=3.48,
        role="code-heavy executor / hard tasks (thinking)",
    ),
    "gemini-3.1-flash-lite": ModelSpec(
        key="gemini-3.1-flash-lite",
        family="google",
        provider="google",
        model_id="gemini-3.1-flash-lite",
        base_url="https://generativelanguage.googleapis.com/v1beta/openai/",
        api_key_env="GEMINI_API_KEY",
        price_in=0.25,
        price_out=1.50,
        role="cross-family verifier / judge (Tier 2)",
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
        role="cross-family verifier (legacy fallback)",
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
    # j76: GLM (Zhipu/THUDM) = 3ra familia ACTIVA -> rompe el techo de decorrelacion
    # cross-family de 2 (deepseek+google) a 3. OpenAI-compatible, cero cupo. family
    # 'zhipu' es distinta de deepseek/google/moonshot -> family_of() la trata como
    # cross-family valida automaticamente (adversarial_verify/ensemble la aceptan, y
    # rechazan zhipu<->zhipu). Inactivo hasta que exista ZHIPU_API_KEY (espejo de kimi).
    # Precios jun-2026 ref, VOLATILES — reverificar antes de fijar budgets.
    "glm-4.5-air": ModelSpec(
        key="glm-4.5-air",
        family="zhipu",
        provider="zhipu",
        model_id="glm-4.5-air",
        base_url="https://api.z.ai/api/paas/v4",
        api_key_env="ZHIPU_API_KEY",
        price_in=0.20,
        price_out=1.10,
        role="3ra familia: verificador/juez cross-family + nodo barato",
    ),
}

# Default node assignments for the MVP slice.
DEFAULT_GENERATOR = "deepseek-chat"        # -> deepseek-v4-flash no-thinking
DEFAULT_VERIFIER = "gemini-3.1-flash-lite"  # cross-family vs deepseek; -40% out vs 2.5-flash
DEFAULT_ROUTER = "gemini-2.5-flash-lite"    # sigue siendo el out/M mas barato servido


def family_of(model_key: str) -> str:
    return REGISTRY[model_key].family


def spec(model_key: str) -> ModelSpec:
    try:
        return REGISTRY[model_key]
    except KeyError:
        raise KeyError(
            f"unknown model '{model_key}'. known: {sorted(REGISTRY)}"
        ) from None
