"""Provider layer — thin OpenAI-compatible client per external model.

External models are exposed via OpenAI-compatible endpoints; the orchestrator
invokes them as tools/nodes. Each call returns a normalized CallResult and
auto-logs a metric record (§11). Keys come from env (loaded from .env).
"""
from __future__ import annotations

import os
import time
from dataclasses import dataclass

from dotenv import load_dotenv

from .config import spec
from .cost import cost_usd
from .metrics import log_event

load_dotenv()  # picks up ~/.claude/orchestration/.env if cwd or parents contain it
# Also explicitly load the package-local .env regardless of cwd.
from pathlib import Path as _Path  # noqa: E402
load_dotenv(_Path(__file__).resolve().parent.parent / ".env")

# Lazy import so the package imports even if `openai` isn't installed yet.
try:
    from openai import OpenAI
    _OPENAI_OK = True
except Exception:  # pragma: no cover
    OpenAI = None  # type: ignore
    _OPENAI_OK = False

_CLIENTS: dict[str, "OpenAI"] = {}


def _classify_error(e: Exception) -> str:
    """Clasifica un fallo de API en una clase MEDIBLE (observabilidad, sin tocar ruteo).
    rate_limit = 429/throttle (openai.RateLimitError, status 429, 'rate limit'/'too many
    requests' en el mensaje); timeout = APITimeoutError/timeout; other = el resto.
    Duck-typing a proposito (no depende de importar tipos del SDK)."""
    name = type(e).__name__.lower()
    status = getattr(e, "status_code", None)
    if status is None:
        status = getattr(getattr(e, "response", None), "status_code", None)
    msg = str(e).lower()
    if (status == 429 or "ratelimit" in name
            or "rate limit" in msg or "too many requests" in msg or "429" in msg):
        return "rate_limit"
    if "timeout" in name or "timeout" in msg or "timedout" in name:
        return "timeout"
    return "other"


def _cached_tokens(usage) -> int:
    """Tokens de input servidos del CACHE (cache-hit). DeepSeek: usage.prompt_cache_hit_tokens.
    OpenAI/estandar: usage.prompt_tokens_details.cached_tokens. 0 si el proveedor no reporta.
    Sin esto se cobraba todo el input a price_in -> sobre-conteo de costo (señal infalsificable)."""
    v = getattr(usage, "prompt_cache_hit_tokens", None)
    if v is not None:
        return int(v) or 0
    det = getattr(usage, "prompt_tokens_details", None)
    if det is not None:
        c = getattr(det, "cached_tokens", None)
        if c is None and isinstance(det, dict):
            c = det.get("cached_tokens")
        if c is not None:
            return int(c) or 0
    return 0


class MissingKeyError(RuntimeError):
    pass


@dataclass
class CallResult:
    model_key: str
    family: str
    text: str
    in_tokens: int
    out_tokens: int
    cost_usd: float
    latency_s: float

    def __str__(self) -> str:
        return self.text


def _client(model_key: str) -> "OpenAI":
    if not _OPENAI_OK:
        raise RuntimeError("`openai` package not installed. pip install openai")
    s = spec(model_key)
    key = os.getenv(s.api_key_env)
    if not key:
        raise MissingKeyError(
            f"env var {s.api_key_env} not set (needed for {model_key}). "
            f"Put it in ~/.claude/orchestration/.env"
        )
    cache_key = f"{s.provider}:{s.base_url}"
    if cache_key not in _CLIENTS:
        _CLIENTS[cache_key] = OpenAI(api_key=key, base_url=s.base_url)
    return _CLIENTS[cache_key]


def call(
    model_key: str,
    messages: list[dict] | str,
    *,
    pattern: str = "raw",
    node: str = "",
    phase: str = "",
    temperature: float = 0.3,
    max_tokens: int | None = 16384,
    timeout: float = 60.0,
    critical: bool = False,
    **kw,
) -> CallResult:
    """Invoke one external model node. Normalizes I/O and logs a metric record.

    H-3: `timeout` (seg) acota la call (latencias 29-45s observadas -> sin timeout
    una call cuelga y bloquea un slot del pool). H-6: `max_tokens` default 16384 =
    cap finito anti-runaway (antes None = ilimitado) pero generoso: NO trunca
    sintesis/audit/codigo tipicos (un audit genero ~5.5k out). Bajalo por-call en
    fan_out masivo si queres acotar costo. H-2: fallo de API loggea error y re-lanza.
    """
    s = spec(model_key)
    if isinstance(messages, str):
        messages = [{"role": "user", "content": messages}]

    # BudgetKeeper: bloquea si el gasto del mes supera el límite (no-op sin límite).
    from .budget import check as _budget_check, BudgetExceeded
    try:
        _budget_check(critical=critical)
    except BudgetExceeded as e:
        # Observabilidad: el cap-hit antes era INVISIBLE (salta antes de cualquier log).
        # Lo registramos pa poder medir budget-cap-hit-rate. NO cambia comportamiento: re-lanza.
        log_event(pattern=pattern, node=node or model_key, model=model_key, family=s.family,
                  in_tokens=0, out_tokens=0, cost_usd=0.0, latency_s=0.0, phase=phase,
                  error=type(e).__name__, error_msg=str(e)[:200], error_class="budget_cap")
        raise

    client = _client(model_key)
    t0 = time.perf_counter()
    if s.extra_body:
        # extras por-modelo (ej DeepSeek V4: thinking disabled pa bulk). El caller
        # puede pisarlos pasando su propio extra_body en kw.
        kw.setdefault("extra_body", dict(s.extra_body))
    try:
        resp = client.chat.completions.create(
            model=s.model_id,
            messages=messages,  # type: ignore[arg-type]  # OpenAI SDK typed-params; list[dict] valid at runtime
            temperature=temperature,
            max_tokens=max_tokens,
            timeout=timeout,
            **kw,
        )
    except Exception as e:
        # H-2: observabilidad de errores. Sin esto, un fallo de API es invisible
        # en metrics.jsonl y rompe el input del break-even (no se ve la fuga).
        # error_class distingue rate-limit/429 del resto -> mide 429-rate por proveedor
        # (señal previa a cualquier futuro load-balancing, exigida por anti-scope-creep).
        log_event(
            pattern=pattern,
            node=node or model_key,
            model=model_key,
            family=s.family,
            in_tokens=0,
            out_tokens=0,
            cost_usd=0.0,
            latency_s=time.perf_counter() - t0,
            phase=phase,
            error=type(e).__name__,
            error_msg=str(e)[:200],
            error_class=_classify_error(e),
        )
        raise
    latency = time.perf_counter() - t0

    text = resp.choices[0].message.content or ""
    usage = resp.usage
    in_tok = getattr(usage, "prompt_tokens", 0) or 0
    out_tok = getattr(usage, "completion_tokens", 0) or 0
    cached_tok = _cached_tokens(usage)
    c = cost_usd(model_key, in_tok, out_tok, cached_tok)

    log_event(
        pattern=pattern,
        node=node or model_key,
        model=model_key,
        family=s.family,
        in_tokens=in_tok,
        out_tokens=out_tok,
        cost_usd=c,
        latency_s=latency,
        phase=phase,
        cached_tokens=cached_tok,
    )
    return CallResult(
        model_key=model_key,
        family=s.family,
        text=text,
        in_tokens=in_tok,
        out_tokens=out_tok,
        cost_usd=c,
        latency_s=latency,
    )
