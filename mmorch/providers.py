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
    from .budget import check as _budget_check
    _budget_check(critical=critical)

    client = _client(model_key)
    t0 = time.perf_counter()
    if s.extra_body:
        # extras por-modelo (ej DeepSeek V4: thinking disabled pa bulk). El caller
        # puede pisarlos pasando su propio extra_body en kw.
        kw.setdefault("extra_body", dict(s.extra_body))
    try:
        resp = client.chat.completions.create(
            model=s.model_id,
            messages=messages,
            temperature=temperature,
            max_tokens=max_tokens,
            timeout=timeout,
            **kw,
        )
    except Exception as e:
        # H-2: observabilidad de errores. Sin esto, un fallo de API es invisible
        # en metrics.jsonl y rompe el input del break-even (no se ve la fuga).
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
        )
        raise
    latency = time.perf_counter() - t0

    text = resp.choices[0].message.content or ""
    usage = resp.usage
    in_tok = getattr(usage, "prompt_tokens", 0) or 0
    out_tok = getattr(usage, "completion_tokens", 0) or 0
    c = cost_usd(model_key, in_tok, out_tok)

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
