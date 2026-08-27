"""Provider layer — thin OpenAI-compatible client per external model.

External models are exposed via OpenAI-compatible endpoints; the orchestrator
invokes them as tools/nodes. Each call returns a normalized CallResult and
auto-logs a metric record (§11). Keys come from env (loaded from .env).
"""
from __future__ import annotations

import os
import random
import threading
import time
from dataclasses import dataclass

from dotenv import load_dotenv

from .config import spec
from .cost import cost_usd
from .metrics import log_event

# El .env canonico vive en el home de mmorch (W2.1): resuelve igual desde
# cualquier cwd (entry points instalados, Cursor, Task Scheduler). Se carga
# PRIMERO porque dotenv no pisa claves ya cargadas — el home gana; el load()
# por cwd queda como fallback para checkouts/tests con .env propio.
from .paths import home as _mmorch_home
load_dotenv(_mmorch_home() / ".env")
load_dotenv()

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


def _http_status(e: Exception) -> int | None:
    status = getattr(e, "status_code", None)
    if status is None:
        status = getattr(getattr(e, "response", None), "status_code", None)
    return status if isinstance(status, int) else None


def _is_transient(e: Exception) -> bool:
    """Clases que un retry puede arreglar: 429 (rate limit), timeout, 5xx del server.
    Todo lo demas (auth, 4xx de request, parseo) es determinista — reintentar solo
    duplica costo y latencia sin cambiar el resultado."""
    if _classify_error(e) in ("rate_limit", "timeout"):
        return True
    status = _http_status(e)
    return status is not None and 500 <= status <= 599


# --- retry con backoff exponencial + jitter (W3.3) ------------------------------------
# Solo clases transitorias; max 3 intentos totales. _sleep es seam de test (sin dormir real).
_RETRY_MAX_ATTEMPTS = 3
_RETRY_BASE_S = 0.5
_sleep = time.sleep


# --- half-open breaker por modelo (W3.3, defaults estilo LiteLLM) ---------------------
# 3 fallos dentro de 60s => abierto (fail-fast BreakerOpen) por 60s de cooldown; pasado el
# cooldown UN caller entra como probe (half-open): exito cierra, fallo re-abre. Evita que un
# run masivo martille un proveedor caido (cada call colgada quema un slot del pool + $).
_BREAKER_FAILS = 3
_BREAKER_WINDOW_S = 60.0
_BREAKER_COOLDOWN_S = 60.0
_BREAKER_LOCK = threading.Lock()
_BREAKERS: dict[str, dict] = {}
_now = time.monotonic   # seam de test (avanzar el reloj sin esperar el cooldown)


class BreakerOpen(RuntimeError):
    """El modelo esta en cooldown por fallos consecutivos; la call ni se intenta."""


def _breaker_state(model: str) -> dict:
    return _BREAKERS.setdefault(
        model, {"fails": 0, "first_fail": 0.0, "opened_at": None, "probing": False})


def _breaker_allow(model: str) -> None:
    """Gate de entrada: cerrado pasa; abierto en cooldown (o con probe en vuelo) lanza
    BreakerOpen; cooldown vencido deja pasar UN caller como probe."""
    with _BREAKER_LOCK:
        b = _breaker_state(model)
        if b["opened_at"] is None:
            return
        elapsed = _now() - b["opened_at"]
        if elapsed < _BREAKER_COOLDOWN_S or b["probing"]:
            raise BreakerOpen(
                f"breaker abierto para {model}: {b['fails']} fallos seguidos, "
                f"cooldown {_BREAKER_COOLDOWN_S:.0f}s (van {elapsed:.0f}s)")
        b["probing"] = True   # half-open: este caller es EL probe


def _breaker_record(model: str, ok: bool) -> None:
    with _BREAKER_LOCK:
        b = _breaker_state(model)
        if ok:
            _BREAKERS[model] = {"fails": 0, "first_fail": 0.0, "opened_at": None,
                                "probing": False}
            return
        t = _now()
        if b["opened_at"] is not None:      # el probe fallo -> re-abre el cooldown entero
            b["opened_at"] = t
            b["probing"] = False
            return
        # ventana deslizante simple: fallos que no son "seguidos" (fuera de la ventana)
        # arrancan la cuenta de nuevo — un fallo aislado por hora no debe abrir nada.
        if b["fails"] == 0 or t - b["first_fail"] > _BREAKER_WINDOW_S:
            b["fails"], b["first_fail"] = 1, t
        else:
            b["fails"] += 1
        if b["fails"] >= _BREAKER_FAILS:
            b["opened_at"] = t
            b["probing"] = False


# --- trackers de costo por-run (W3.4: breaker USD de project_build) -------------------
# Un caller (build_project) registra un acumulador dict {"usd": float} mientras dura su
# run; call() le suma el costo (real o estimado en timeout) de CADA api-call. Lista, no
# singleton: builds anidados/paralelos acumulan cada uno lo suyo.
_RUN_TRACKER_LOCK = threading.Lock()
_RUN_TRACKERS: list[dict] = []


def register_run_tracker(t: dict) -> None:
    with _RUN_TRACKER_LOCK:
        _RUN_TRACKERS.append(t)


def unregister_run_tracker(t: dict) -> None:
    with _RUN_TRACKER_LOCK:
        if t in _RUN_TRACKERS:
            _RUN_TRACKERS.remove(t)


def _track_cost(c: float) -> None:
    with _RUN_TRACKER_LOCK:
        for t in _RUN_TRACKERS:
            t["usd"] = t.get("usd", 0.0) + c


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

    # breaker por modelo ANTES de gastar nada: modelo en cooldown = fail-fast observable.
    try:
        _breaker_allow(model_key)
    except BreakerOpen as e:
        log_event(pattern=pattern, node=node or model_key, model=model_key, family=s.family,
                  in_tokens=0, out_tokens=0, cost_usd=0.0, latency_s=0.0, phase=phase,
                  error=type(e).__name__, error_msg=str(e)[:200], error_class="breaker_open")
        raise

    client = _client(model_key)
    if s.extra_body:
        # extras por-modelo (ej DeepSeek V4: thinking disabled pa bulk). El caller
        # puede pisarlos pasando su propio extra_body en kw.
        kw.setdefault("extra_body", dict(s.extra_body))
    for attempt in range(1, _RETRY_MAX_ATTEMPTS + 1):
        t0 = time.perf_counter()
        try:
            resp = client.chat.completions.create(
                model=s.model_id,
                messages=messages,  # type: ignore[arg-type]  # OpenAI SDK typed-params; list[dict] valid at runtime
                temperature=temperature,
                max_tokens=max_tokens,
                timeout=timeout,
                **kw,
            )
            _breaker_record(model_key, ok=True)
            break
        except Exception as e:
            _breaker_record(model_key, ok=False)
            eclass = _classify_error(e)
            transient = _is_transient(e)
            # Timeout: el server YA proceso el input (y lo factura) aunque nunca vimos la
            # respuesta. cost=0 subestimaba por diseño y rompia la defensa del budget:
            # estimamos por chars enviados (~4 chars/token) y lo marcamos estimado.
            err_in, err_cost, err_extra = 0, 0.0, {}
            if eclass == "timeout":
                err_in = sum(len(str(m.get("content", ""))) for m in messages) // 4
                err_cost = cost_usd(model_key, err_in, 0)
                err_extra = {"cost_estimated": True}
                _track_cost(err_cost)
            # H-2: observabilidad de errores. Sin esto, un fallo de API es invisible
            # en metrics.jsonl y rompe el input del break-even (no se ve la fuga).
            # error_class distingue rate-limit/429 del resto -> mide 429-rate por proveedor.
            # attempt/retried registran CADA retry (W3.3): el retry silencioso esconde
            # exactamente la degradacion que el breaker necesita hacer visible.
            log_event(
                pattern=pattern,
                node=node or model_key,
                model=model_key,
                family=s.family,
                in_tokens=err_in,
                out_tokens=0,
                cost_usd=err_cost,
                latency_s=time.perf_counter() - t0,
                phase=phase,
                error=type(e).__name__,
                error_msg=str(e)[:200],
                error_class=eclass,
                attempt=attempt,
                retried=transient and attempt < _RETRY_MAX_ATTEMPTS,
                **err_extra,
            )
            if not transient or attempt >= _RETRY_MAX_ATTEMPTS:
                raise
            # backoff exponencial + jitter (evita que N workers re-golpeen en sincronia)
            _sleep(_RETRY_BASE_S * (2 ** (attempt - 1)) + random.uniform(0.0, 0.25))
    else:  # pragma: no cover — el loop siempre sale por break (exito) o raise (fallo final)
        raise AssertionError("unreachable")
    latency = time.perf_counter() - t0

    text = resp.choices[0].message.content or ""
    usage = resp.usage
    in_tok = getattr(usage, "prompt_tokens", 0) or 0
    out_tok = getattr(usage, "completion_tokens", 0) or 0
    cached_tok = _cached_tokens(usage)
    c = cost_usd(model_key, in_tok, out_tok, cached_tok)
    _track_cost(c)

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
