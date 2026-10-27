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
    family: str         # deepseek | anthropic activas; google y zhipu sin clave: solo re-correr ablaciones del README
    provider: str       # billing/endpoint surface
    model_id: str       # API model identifier
    base_url: str | None
    api_key_env: str    # env var holding the key
    price_in: float     # USD / 1M input tokens
    price_out: float    # USD / 1M output tokens
    role: str           # design role (§4)
    extra_body: tuple = ()  # request-body extras como tuple de pares (hashable);
                            # ej (("thinking", {"type": "disabled"}),) pa DeepSeek V4


# OpenAI-compatible endpoints. Opus is NOT here on purpose: it is the orchestrator
# (Claude Code itself / cupo), never invoked as an external node. Haiku si es nodo: provider
# "anthropic" por la Messages API con ANTHROPIC_API_KEY, en dolares y sin cupo (usuario 2026-10-26).
REGISTRY: dict[str, ModelSpec] = {
    # Keys internas estables (no romper brazos del bandit). model_id = nombre vigente:
    # /models (2026-10-15) solo lista deepseek-flash y deepseek-v4-pro; deepseek-chat,
    # deepseek-reasoner y deepseek-v4-flash son alias de deepseek-flash.
    "deepseek-chat": ModelSpec(
        key="deepseek-chat",
        family="deepseek",
        provider="deepseek",
        model_id="deepseek-flash",         # V4.1 Flash; thinking APAGADO pa bulk
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
        model_id="deepseek-flash",         # V4.1 Flash; thinking PRENDIDO explicito
        base_url="https://api.deepseek.com/v1",
        api_key_env="DEEPSEEK_API_KEY",
        price_in=0.14,
        price_out=0.28,
        role="independent reasoning/audit (thinking)",
        extra_body=(("thinking", {"type": "enabled"}),),
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
    # Variante SIN razonamiento de v4-pro. Existe para la ablacion de decorrelacion
    # (ablation_stages_hard, 2026-09-09): con thinking prendido v4-pro y glm-5.2 no
    # cometen NINGUN error en 19 items dificiles, asi que no hay nada que correlacionar.
    # Para medir si dos verificadores fallan en los MISMOS items hacen falta gates que
    # fallen, dos por familia. No entra en pools de produccion: para verificar se usa
    # razonamiento prendido (CLAUDE.md global, medido 2026-09-07).
    "deepseek-v4-pro-nothink": ModelSpec(
        key="deepseek-v4-pro-nothink",
        family="deepseek",
        provider="deepseek",
        model_id="deepseek-v4-pro",
        base_url="https://api.deepseek.com/v1",
        api_key_env="DEEPSEEK_API_KEY",
        price_in=1.74,
        price_out=3.48,
        role="SOLO ablacion: v4-pro sin thinking (gate que falla, misma familia que chat)",
        extra_body=(("thinking", {"type": "disabled"}),),
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
    # glm-5.2: sucesor de 4.6 a PROBAR en el pool de intuition (pedido usuario 2026-07-02).
    # 4.6 midió 34% error ventaneado (timeouts) — el health-floor lo demota solo; 5.2 entra
    # a competir por ejecución (el sig-bandit decide). Precio verificado 2026-10-07 en
    # docs.z.ai/guides/overview/pricing: 1.40/4.40 (antes 0.60/2.20 copiados de 4.6, la mitad del real).
    "glm-5.2": ModelSpec(
        key="glm-5.2",
        family="zhipu",
        provider="zhipu",
        model_id="glm-5.2",
        base_url="https://api.z.ai/api/paas/v4",
        api_key_env="ZHIPU_API_KEY",
        price_in=1.40,
        price_out=4.40,
        role="candidato intuition-pool (3ra familia, reemplaza a 4.6 health-floored)",
    ),
    # Espejo de deepseek-v4-pro-nothink para la familia zhipu (misma razon, misma fecha).
    # z.ai acepta `thinking: {"type": "disabled"}` en el body (OpenAI-compatible).
    "glm-5.2-nothink": ModelSpec(
        key="glm-5.2-nothink",
        family="zhipu",
        provider="zhipu",
        model_id="glm-5.2",
        base_url="https://api.z.ai/api/paas/v4",
        api_key_env="ZHIPU_API_KEY",
        price_in=1.40,
        price_out=4.40,
        role="SOLO ablacion: glm-5.2 sin thinking (gate que falla, misma familia que 4.5-air)",
        extra_body=(("thinking", {"type": "disabled"}),),
    ),
    # 2026-10-26: reemplaza a zhipu (cuenta sin saldo, 429 desde 2026-10-16). Precio hasta 100k tokens
    # de prompt (0.50/2.50 arriba), cache 2026-10-06 de la skill claude-api: reverificar.
    "claude-haiku-5-5": ModelSpec(
        key="claude-haiku-5-5",
        family="anthropic",
        provider="anthropic",
        model_id="claude-haiku-5-5",
        base_url=None,
        api_key_env="ANTHROPIC_API_KEY",
        price_in=0.10,
        price_out=0.50,
        role="router, encoder de babel, juez visual y verificador de ensemble",
    ),
}

# Default node assignments for the MVP slice.
DEFAULT_GENERATOR = "deepseek-chat"        # -> deepseek-flash no-thinking
DEFAULT_VERIFIER = "deepseek-reasoner"     # GOAL 2026-10-07: familia libre; el mejor medido y el mas barato (glm-5.2 ~17x)
CHECKABLE_VERIFIER = "deepseek-reasoner"   # checkeable sin oraculo: el mejor MEDIDO (n=350, acc. bal. 0.997); same-family permitido ahi
DEFAULT_ROUTER = "claude-haiku-5-5"         # 2026-10-26: zhipu sin saldo; Haiku por API

# Candidate generator pool the intuition layer (signature-keyed bandit) picks among when
# routing is left to it. Bare valid REGISTRY keys only, spanning families (decorrelate).
DEFAULT_INTUITION_POOL = ["deepseek-chat", "deepseek-v4-pro", "claude-haiku-5-5"]  # 2026-10-26: sin zhipu


def family_of(model_key: str) -> str:
    # via spec() y no REGISTRY[...] directo: un modelo desconocido debe fallar con la
    # lista de modelos validos, no con un KeyError pelado (W5.1, huecos de borde #5)
    return spec(model_key).family


def spec(model_key: str) -> ModelSpec:
    try:
        return REGISTRY[model_key]
    except KeyError:
        raise KeyError(
            f"unknown model '{model_key}'. known: {sorted(REGISTRY)}"
        ) from None
