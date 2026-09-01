"""MCP wrapper — exposes mmorch patterns as tools to Claude Code.

This is the "ambos" path: the same library is callable both as plain Python
(harness migrado, §5) AND as MCP tools the orchestrator can invoke mid-session.

IMPORTANT (cupo discipline, §5): invoking these tools spends EXTERNAL API dollars,
NOT Claude cupo. That is the point — bulk/verify is offloaded off the plan.

Run (stdio):  mmorch-mcp  (o python mcp_server.py via el shim compat en la raiz)
Register:     see README.md "Register the MCP server".
"""
from __future__ import annotations

import functools
import json
import os
import time

try:
    from mcp.server.fastmcp import FastMCP
except Exception as e:  # pragma: no cover
    raise SystemExit(
        "MCP SDK not installed. Run: pip install \"mcp>=1.2.0\"  "
        f"(original error: {e})"
    ) from e

from mmorch import (fan_out, adversarial_verify, route, cascade, ensemble_verify,
                    ideate_and_screen, recall as _recall, tournament as _tournament,
                    bucket_rank as _bucket_rank)
from mmorch.config import DEFAULT_GENERATOR, DEFAULT_VERIFIER
from mmorch.metrics import summary, error_rates, cache_stats
from mmorch.learn import analyze as _learn_analyze, recommend as _learn_recommend
from mmorch.memory import (remember as _remember, stats as _mem_stats,
                           consolidate as _mem_consolidate, reinforce as _reinforce,
                           flag_contradiction as _flag_contradiction,
                           pending_review as _pending_review,
                           resolve_review as _resolve_review, close_loop as _close_loop,
                           open_loops as _open_loops, forget_preview as _forget_preview)
from mmorch.curiosity import find_tension as _find_tension
from mmorch.autoresearch import run_autoresearch as _autoresearch
from mmorch.classify import classify as _classify, cynefin_classify as _cynefin
from mmorch.spec import (build_spec as _build_spec, interview as _spec_interview,
                         perfect as _perfect)
from mmorch.speedup import speedup as _speedup
from mmorch.sessions import ingest_session as _ingest_session
from mmorch.session_skills import (ingest_workflows as _ingest_workflows,
                                   top_playbooks as _top_playbooks)
from mmorch.config import DEFAULT_ROUTER
from mmorch.intuition import (decide as _intuition_decide, candidates as _intuition_candidates,
                              coherence as _intuition_coherence, reframe as _intuition_reframe)
from mmorch.code_review import review_source as _review_source
from mmorch.feedback import (record_outcome as _record_outcome,
                            ThompsonBandit as _ThompsonBandit,
                            calibration as _calibration)

mcp = FastMCP("mmorch")
# serverInfo.version (defecto #3 r3): sin esto FastMCP deja version=None y el
# handshake reporta la version de la LIB mcp (1.27.x), no la de mmorch — el
# cliente no puede saber que build del server tiene enfrente.
try:
    from importlib.metadata import version as _pkg_version
    mcp._mcp_server.version = _pkg_version("mmorch")
except Exception:
    pass  # sin metadata instalada, el fallback de la lib sigue funcionando
from mmorch.mcp_telemetry import instrument  # noqa: E402 (needs `mcp` defined first)
instrument(mcp)   # audit 2026-07: logs EVERY tool call (incl. las ~20 deterministas que
                  # metrics.jsonl nunca ve) a logs/mcp_calls.jsonl — cero cambios en las tools

# --- Perfil de tools (W2.2) ---------------------------------------------------
# Cursor tiene un techo practico de ~40 tools por server; el set full lo excede.
# "core" (DEFAULT desde la poda 2026-08-30) registra solo las tools con uso
# medido; MMORCH_MCP_PROFILE=full registra las 47. Se lee a import-time porque FastMCP registra
# via decorator a import-time — cambiar de perfil = reiniciar el server (igual
# que MMORCH_HOME o las API keys).
# `or "core"`: MMORCH_MCP_PROFILE="" (var seteada vacia) tiene que caer en el
# default, no en full — antes daba lo mismo porque el default ERA full.
_PROFILE = os.getenv("MMORCH_MCP_PROFILE", "").strip().lower() or "core"

# Fuera de "core": TELEMETRIA, no criterio a ojo (poda 2026-08-30).
# logs/mcp_calls.jsonl, 53 dias (2026-07-08 -> 08-30, 269 llamadas): 11 de 47
# tools se invocaron alguna vez y 5 concentran el 97% (budget_status 136,
# record_outcome 52, review_code 39, adversarial_verify 27, vault_write 7).
# Las 32 de abajo no se llamaron NUNCA, y no son nuevas: fan_out, tournament,
# cascade y classify son del 2026-06-07, el dia fundacional.
#
# Esto es superficie de DECISION, no borrado: la funcion de libreria queda
# intacta y "full" las sigue registrando. Volver a exponer una = sacarla de aca.
#
# Se quedan en core sin llamadas, a proposito:
#   canal                              nacio 2026-08-30, no tuvo ventana
#   build_spec/route/spec_interview    los nombra ~/.claude/skills/perfect
_NOT_IN_CORE = frozenset({
    "mmorch_autoresearch", "mmorch_bucket_rank", "mmorch_cache_stats",
    "mmorch_cascade", "mmorch_classify", "mmorch_close_loop",
    "mmorch_consolidate", "mmorch_error_rates", "mmorch_evolve_nightly",
    "mmorch_evolve_self", "mmorch_fan_out", "mmorch_feedback_stats",
    "mmorch_find_tension", "mmorch_flag_contradiction", "mmorch_forget_preview",
    "mmorch_ingest_session", "mmorch_intuition", "mmorch_learn",
    "mmorch_memory_stats", "mmorch_metrics_summary", "mmorch_open_loops",
    "mmorch_orchestra", "mmorch_pending_review", "mmorch_perfect",
    "mmorch_reinforce", "mmorch_resolve_review", "mmorch_rubric_next",
    "mmorch_rubric_start", "mmorch_rubric_submit", "mmorch_session_playbooks",
    "mmorch_speedup", "mmorch_tournament",
})


# --- Contrato de error uniforme (W5.1) ---------------------------------------
# Antes convivian dos contratos: algunas tools devolvian {"error": "..."} y otras
# dejaban propagar la excepcion Python cruda al framework MCP — un caller no podia
# tratar errores uniformemente. UN solo punto de catch: toda tool devuelve
# {"error": str, "kind": str} en fallo. `kind` agrupa por accion del caller:
# arreglar el input vs reintentar vs reportar bug.
def _kind_of(e: BaseException) -> str:
    from mmorch.budget import BudgetExceeded
    if isinstance(e, BudgetExceeded):
        return "budget"
    if isinstance(e, FileNotFoundError):
        return "not_found"
    if isinstance(e, (OSError, UnicodeDecodeError)):
        return "io"
    if isinstance(e, (KeyError, IndexError, ValueError, TypeError)):
        return "invalid_input"
    return "internal"


def _guarded(fn):
    """Envuelve una tool con el contrato de error. Llama via wrapper.__wrapped__
    (atributo, no closure) para que el test de contrato pueda inyectar un fallo
    controlado por tool sin tocar red ni APIs."""
    @functools.wraps(fn)
    def wrapper(*args, **kwargs):
        try:
            return wrapper.__wrapped__(*args, **kwargs)
        except Exception as e:
            msg = str(e).strip() or type(e).__name__
            return json.dumps({"error": msg[:500], "kind": _kind_of(e)},
                              ensure_ascii=False)
    wrapper.__mmorch_guarded__ = True  # marker que verifica el test de contrato
    return wrapper


# --- Matriz de riesgo por tool (W5.3, research 08: risk-rating de OpenAI) ------
# Cada tool declara su clase de riesgo; el registro _tool la EXIGE (tool nueva sin
# declarar = RuntimeError a import-time, no drift silencioso):
#   read   — consulta/computo; no cambia estado persistente del sistema.
#   mutate — cambia estado local (memoria, vault, outcomes, archivos, o ejecuta
#            codigo generado en sandbox).
#   outward — efectos FUERA de la maquina/estado local (PRs, push, mensajes).
# NO hay gate HITL bloqueante (decision W5.3): mutate sensibles + outward dejan
# rastro en un audit trail (logs/audit.jsonl) para forensica y review humano.
_TOOL_RISK: dict[str, str] = {
    "mmorch_fan_out": "read",
    "mmorch_adversarial_verify": "read",
    "mmorch_metrics_summary": "read",
    "mmorch_error_rates": "read",
    "mmorch_budget_status": "read",
    "mmorch_cache_stats": "read",
    "mmorch_route": "read",
    "mmorch_review_code": "read",
    "mmorch_intuition": "read",
    "mmorch_cascade": "read",
    "mmorch_autoresearch": "mutate",      # edita target_file en el repo del caller
    "mmorch_ensemble_verify": "read",
    "mmorch_learn": "read",
    "mmorch_innovate": "read",
    "mmorch_remember": "mutate",
    "mmorch_canal": "mutate",              # append logs/canal.jsonl (hilo agentes)
    "mmorch_vault_write": "mutate",       # escribe en el vault global
    "mmorch_recall": "mutate",            # bumpea access_count (afecta decay futuro)
    "mmorch_tournament": "read",
    "mmorch_bucket_rank": "read",
    "mmorch_classify": "read",
    "mmorch_cynefin": "read",
    "mmorch_spec_interview": "read",
    "mmorch_build_spec": "read",
    "mmorch_ingest_session": "mutate",
    "mmorch_session_playbooks": "mutate",
    "mmorch_record_outcome": "mutate",
    "mmorch_feedback_stats": "read",
    "mmorch_check": "read",
    "mmorch_evolve_self": "mutate",       # propone+snapshotea cambios de codigo
    "mmorch_evolve_nightly": "outward",   # abre PRs — sale del estado local
    "mmorch_orchestra": "read",
    "mmorch_consolidate": "mutate",       # tombstonea memoria (con apply)
    "mmorch_memory_stats": "read",
    "mmorch_reinforce": "mutate",
    "mmorch_flag_contradiction": "mutate",
    "mmorch_pending_review": "read",
    "mmorch_resolve_review": "mutate",    # drop=true tombstonea
    "mmorch_close_loop": "mutate",
    "mmorch_open_loops": "read",
    "mmorch_find_tension": "read",
    "mmorch_forget_preview": "read",
    "mmorch_rubric_start": "read",
    "mmorch_rubric_next": "read",
    "mmorch_rubric_submit": "mutate",     # re-ejecuta checkers + registra outcomes
    "mmorch_perfect": "read",
    "mmorch_speedup": "mutate",           # ejecuta codigo generado (subprocess)
}

# mutate SENSIBLES (tocan codigo/vault/memoria de forma dificil de deshacer o
# ejecutan codigo generado) + todo outward: van al audit trail.
_AUDITED = frozenset({
    "mmorch_autoresearch", "mmorch_evolve_self", "mmorch_evolve_nightly",
    "mmorch_vault_write", "mmorch_consolidate", "mmorch_resolve_review",
    "mmorch_speedup",
})


def _audit_log(tool: str, risk: str, ok: bool, kwargs: dict) -> None:
    """Append-only a logs/audit.jsonl. logs_dir() se resuelve EN cada call (no a
    import-time) para que MMORCH_HOME de una instancia aislada aplique. El audit
    jamas rompe la tool (fail-open: perder una linea de log < perder la operacion)."""
    try:
        from mmorch.paths import logs_dir
        rec = {"ts": time.time(), "tool": tool, "risk": risk, "ok": ok,
               "args": json.dumps({k: str(v)[:80] for k, v in kwargs.items()},
                                  ensure_ascii=False)[:400]}
        with open(logs_dir() / "audit.jsonl", "a", encoding="utf-8") as f:
            f.write(json.dumps(rec, ensure_ascii=False) + "\n")
    except Exception:
        pass


def _audited(fn, risk: str):
    """Wrapper de audit trail para tools sensibles/outward. Va POR DENTRO de
    _guarded: ve la excepcion real (ok=False) y la re-lanza para que el contrato
    de error la formatee."""
    @functools.wraps(fn)
    def wrapper(*args, **kwargs):
        ok = True
        try:
            return fn(*args, **kwargs)
        except BaseException:
            ok = False
            raise
        finally:
            _audit_log(fn.__name__, risk, ok, kwargs)
    return wrapper


def _registers_in_profile(name: str, risk: str, profile: str) -> bool:
    """Decide si una tool se registra en el server segun el perfil. core (Cursor)
    excluye la lista curada Y todo outward por default: un cliente generico no
    debe descubrir tools con efectos fuera de la maquina sin opt-in (full)."""
    if profile != "core":
        return True
    return name not in _NOT_IN_CORE and risk != "outward"


def _tool(fn):
    """Registro condicional por perfil (en core, las tools excluidas quedan como
    funciones Python normales) + contrato de error uniforme para TODAS + matriz
    de riesgo obligatoria (audit trail en sensibles/outward)."""
    name = fn.__name__
    risk = _TOOL_RISK.get(name)
    if risk is None:
        raise RuntimeError(
            f"{name} sin riesgo declarado en _TOOL_RISK (read|mutate|outward) — "
            "toda tool MCP debe clasificarse (W5.3)")
    if name in _AUDITED:
        fn = _audited(fn, risk)
    fn = _guarded(fn)
    fn.__mmorch_risk__ = risk  # metadata inspeccionable (tests / clientes)
    if not _registers_in_profile(name, risk, _PROFILE):
        return fn
    return mcp.tool()(fn)


@_tool
def mmorch_fan_out(
    prompts: list[str],
    gen_model: str = DEFAULT_GENERATOR,
    system: str | None = None,
) -> str:
    """Run independent generation tasks in parallel on a cheap external model.

    Use for bulk work with many independent sub-steps. Spends external API $,
    not Claude cupo. Returns a JSON list of {text, in_tokens, out_tokens, cost_usd}.
    """
    results = fan_out(prompts, gen_model=gen_model, system=system, phase="mcp")
    return json.dumps(
        [
            {
                "text": r.text,
                "in_tokens": r.in_tokens,
                "out_tokens": r.out_tokens,
                "cost_usd": r.cost_usd,
            }
            for r in results
        ],
        ensure_ascii=False,
    )


@_tool
def mmorch_adversarial_verify(
    artifact: str,
    rubric: str,
    gen_model: str = DEFAULT_GENERATOR,
    verifier_model: str = DEFAULT_VERIFIER,
    task_kind: str = "subjective",
) -> str:
    """Verify an artifact with an adversarial skeptic. Cross-family is TASK-AWARE (#2).

    task_kind="subjective" (default): cross-family REQUIRED (same-family raises) — for
    judgement/design/prose where a model can endorse its own blind spot.
    task_kind="checkable": claim has computable ground-truth (math/code/fact). Same-family
    ALLOWED (cost lever) — §18.4+ablation show cross-family adds no detection there.
    CAVEAT: on hard checkable tasks any LLM verifier is unreliable (~74% false-refute);
    prefer a tool/code check when you can compute the truth.
    The verifier refutes by default. Returns {passed, confidence, refutations, cost_usd}.
    """
    v = adversarial_verify(
        artifact,
        rubric=rubric,
        gen_model=gen_model,
        verifier_model=verifier_model,
        phase="mcp",
        task_kind=task_kind,
    )
    return json.dumps(
        {
            "passed": v.passed,
            "confidence": v.confidence,
            "refutations": v.refutations,
            "verifier_model": v.verifier_model,
            "cost_usd": v.cost_usd,
        },
        ensure_ascii=False,
    )


@_tool
def mmorch_metrics_summary() -> str:
    """Return aggregate metrics (calls, total cost USD, cost by family). Cost is a
    FLOOR, not ground truth: timed-out calls log cost=0 but the provider still bills
    (budget.py) — treat totals as an underestimate."""
    return json.dumps(summary(), ensure_ascii=False)


@_tool
def mmorch_error_rates(window_n: int = 200) -> str:
    """Per-model and per-family failure rates over the last `window_n` calls (read-only,
    zero spend): 429/rate_limit count + rate, budget-cap-hit count + rate, timeouts, and
    overall error_rate. Denominator = all logged attempts for that model in the window.
    This is OBSERVABILITY ONLY — it does NOT bias routing. It is the measured signal any
    future load-balancing would have to cite to justify itself under the anti-scope-creep
    rule. error_class comes from providers._classify_error and the budget gate."""
    return json.dumps(error_rates(window_n=window_n), ensure_ascii=False)


@_tool
def mmorch_budget_status() -> str:
    """Budget status (B3, read-only, zero spend): {month, spent, limit, remaining, enforced}.
    enforced=false means MMORCH_MAX_MONTHLY_USD is unset (unlimited spend). Use this BEFORE a
    bulk fan_out: if enforced and remaining is low vs the estimated batch cost, shrink or defer
    the batch instead of hitting BudgetExceeded mid-run. metrics_summary aggregates LIFETIME
    cost and never compares against the monthly cap — this is the only mid-session cupo-$ signal.
    `spent` is a FLOOR: timed-out calls log cost=0 but the provider still bills (budget.py)."""
    from mmorch.budget import status as budget_status
    return json.dumps(budget_status(), ensure_ascii=False)


@_tool
def mmorch_cache_stats(window_n: int = 500) -> str:
    """Per-model prompt-cache-hit-rate (cached_tokens / in_tokens) over the last window_n
    calls (read-only, zero spend). DeepSeek bills cached input ~50x cheaper; this is the
    measured signal that makes prefix-stable-prompt and off-peak savings falsifiable.
    Observability only — does not route."""
    return json.dumps(cache_stats(window_n=window_n), ensure_ascii=False)


@_tool
def mmorch_route(
    prompt: str,
    gen_model: str = DEFAULT_GENERATOR,
    threshold: float = 0.7,
    models: list[str] | None = None,
) -> str:
    """Confidence-gated routing (I-2). A cheap external model answers and
    self-scores; returns escalate=True if confidence < threshold so the
    orchestrator (Opus) only intervenes when needed. Spends external $, not cupo.
    If `models` is given, the signature-keyed intuition bandit picks gen_model when
    this task's structure is FAMILIAR (else falls back to the default gen_model).
    Returns JSON {answer, confidence, escalate, model, cost_usd}.
    """
    r = route(prompt, gen_model=gen_model, threshold=threshold, phase="mcp", models=models)
    return json.dumps({
        "answer": r.answer, "confidence": r.confidence, "escalate": r.escalate,
        "model": r.model, "cost_usd": r.cost_usd}, ensure_ascii=False)


@_tool
def mmorch_review_code(code: str = "", path: str = "") -> str:
    """Senior code reviewer (cero cupo): flag where code breaks the mmorch coding principles
    (docs/coding-principles.md) — module depth/cohesion/coupling, DRY, nesting, naming, scope,
    why-comments, KISS, security. Cross-family refuted (DeepSeek↔Gemini) so style-opinion nitpicks
    get pruned; subjective review, so truth is judgement not execution. Pass `code` inline OR a
    `path` to read from disk (path only used when `code` is empty). Secret gate (library-side,
    W5.1): refuses secret-looking paths (.env/*.key/*.pem/id_rsa/...) AND inline code containing
    credential signatures (private-key blocks, known token prefixes) — content goes to an
    EXTERNAL API. Returns JSON {path, findings:[{principle, severity, line, problem, fix}],
    n_raw, n_confirmed, dropped} or {error, kind} on refusal/failure.
    """
    return json.dumps(_review_source(code, path), ensure_ascii=False)


@_tool
def mmorch_intuition(task: str, models: list[str], complexity: str = "") -> str:
    """Intuition layer READ (cero cupo, no generation): what the signature-keyed bandit
    recommends for `task` among `models`, and how familiar the task's structure is. Use to
    A/B the intuition router vs the default before trusting it, or to inspect what was learned.
    Returns JSON {decision, model, reason, coherence, candidates:[[model,mean,n]],
    reframe_neighbors:[...]}. decision=commit means the structure is familiar+good; escalate
    means cold/weak (let route/Opus decide). coherence = samples seen at this signature.
    """
    act, mdl, reason = _intuition_decide(models, task, complexity=complexity)
    return json.dumps({
        "decision": act, "model": mdl, "reason": reason,
        "coherence": _intuition_coherence(task, complexity=complexity),
        "candidates": _intuition_candidates(models, task, complexity=complexity),
        "reframe_neighbors": _intuition_reframe(task, complexity=complexity)[:4],
    }, ensure_ascii=False)


@_tool
def mmorch_cascade(
    prompt: str,
    steps: list[list] | None = None,
) -> str:
    """FrugalGPT-style cascade: cheapest model first + self-score; escalate to the
    next only if confidence < per-step threshold; flag Opus if all steps exhausted.
    Saves cupo (resolves cheap when possible). steps = [[model, threshold], ...] — a
    malformed step (missing threshold, non-numeric) returns {error, kind:"invalid_input"}.
    Returns JSON {answer, confidence, resolved_step, escalate, models_used, cost_usd}.
    """
    # adaptar tipo (list->tuple) y nada mas: el shape lo valida la libreria (W5.1)
    from typing import cast
    st = cast("list[tuple[str, float]]", [tuple(s) for s in steps]) if steps else None
    r = cascade(prompt, steps=st, phase="mcp")
    return json.dumps({
        "answer": r.answer, "confidence": r.confidence,
        "resolved_step": r.resolved_step, "escalate": r.escalate,
        "models_used": r.models_used, "cost_usd": r.cost_usd}, ensure_ascii=False)


@_tool
def mmorch_autoresearch(
    task: str,
    target_file: str,
    scorer_cmd: str,
    cwd: str = ".",
    models: list[str] | None = None,
    maximize: bool = False,
    max_rounds: int = 20,
    patience: int = 5,
    metric_regex: str = r"score[:=]\s*([-\d.]+)",
    journal_path: str | None = None,
    resume: bool = False,
) -> str:
    """autoresearch (r4a): hillclimb como JOB — optimiza una metrica escalar editando
    `target_file` con un modelo, contra un scorer DETERMINISTA frozen (`scorer_cmd`, que
    debe imprimir la metrica matcheable por `metric_regex`). keep/discard por best, journal
    append-only (resume=True continua desde journal_path). anti-reward-hacking: la metrica
    sale de la ejecucion, NUNCA de un LLM. Gasta API (genera) — cero cupo. NUNCA pushea.
    Returns JSON {best_score, baseline, rounds, stopped, improved}.
    """
    r = _autoresearch(task, target_file, scorer_cmd, cwd=cwd, models=models,
                      maximize=maximize, max_rounds=max_rounds, patience=patience,
                      metric_regex=metric_regex, journal_path=journal_path, resume=resume)
    return json.dumps({
        "best_score": r.best_score, "baseline": r.baseline, "rounds": r.rounds,
        "stopped": r.stopped,
        "improved": (r.baseline is not None and r.best_score is not None
                     and r.best_score != r.baseline)}, ensure_ascii=False)


@_tool
def mmorch_ensemble_verify(
    artifact: str,
    rubric: str,
    gen_model: str = DEFAULT_GENERATOR,
    verifier_models: list[str] | None = None,
) -> str:
    """Ensemble adversarial verify (I-3): K cross-family skeptics + majority vote
    (tie -> fail). More robust than a single verifier. Each verifier must be
    cross-family vs the generator (OneFlow). Returns JSON
    {passed, confidence, n_passed, n_total, refutations, cost_usd}.
    """
    ev = ensemble_verify(artifact, rubric=rubric, gen_model=gen_model,
                         verifier_models=verifier_models, phase="mcp")
    return json.dumps({
        "passed": ev.passed, "confidence": ev.confidence,
        "n_passed": ev.n_passed, "n_total": ev.n_total,
        "unanimous": ev.unanimous, "escalate": ev.escalate,  # #5: split -> a Opus
        "ensemble_degraded": ev.ensemble_degraded,  # B2: verificadores 1-familia (no decorrelaciona)
        "refutations": ev.refutations, "cost_usd": ev.cost_usd}, ensure_ascii=False)


@_tool
def mmorch_learn() -> str:
    """Meta-intelligence (I-1): mmorch reads its own metrics.jsonl and returns
    cost/latency/usage per model x pattern + gated recommendations (cheaper
    defaults, latency flags, observability gaps). Read-only, no API spend.
    Returns JSON {analysis, recommendations}.
    """
    return json.dumps({
        "analysis": _learn_analyze(),
        "recommendations": _learn_recommend(),
    }, ensure_ascii=False)


@_tool
def mmorch_innovate(
    context: str,
    lenses: list[str],
    ask: str,
    rubric: str,
) -> str:
    """Innovation engine (I-5): mmorch ideates NEW capabilities for itself
    (fan_out over lenses) and screens each adversarially cross-family. Returns
    surviving (non-refuted) ideas. Spends external $, not cupo. Returns JSON list
    of {idea, survives, confidence, objection}.
    """
    res = ideate_and_screen(context, lenses, ask, rubric)
    return json.dumps([
        {"idea": s.idea, "survives": s.survives, "confidence": s.confidence,
         "objection": s.objection} for s in res], ensure_ascii=False)


@_tool
def mmorch_remember(
    scope: str,
    episode_text: str,
    kind: str = "note",
    verify: bool = False,
    open_loop: bool = False,
    permanent: bool = False,
) -> str:
    """Persist a memory: append the raw episode (immutable) + distill a durable note
    (Thought-Retriever, cheap model) + embed it. scope is hierarchical
    (task_id<subsector<project_id<mmorch_self<global). If verify=True, a cross-family
    skeptic checks the note is faithful to the episode before persisting (else only
    the raw is kept). open_loop=true marks an unfinished task/question (Zeigarnik:
    resists forgetting until mmorch_close_loop). permanent=true pins the note
    (lifespan='permanent': decay never forgets it); default 'decay' = forgettable.
    Spends a little external $, not cupo. Returns JSON
    {episode_id, note_id, distilled, persisted, refutations}.
    """
    return json.dumps(_remember(scope, episode_text, kind=kind, verify=verify,
                                open_loop=open_loop, permanent=permanent),
                      ensure_ascii=False)


@_tool
def mmorch_canal(
    action: str,
    src: str = "",
    kind: str = "status",
    body: str = "",
    artifacts: list[str] | None = None,
    verify_cmd: str = "",
    verify_expect: str = "",
    to: str = "",
    n: int = 20,
) -> str:
    """Ordered thread between Cursor, Claude Code, and mmorch (logs/canal.jsonl).
    Nobody wakes the other process. action=read returns last n turns (oldest first).
    action=post appends a turn: src in cursor|claude|mmorch|user, kind in
    status|ask|refute|handoff. handoff should set verify_cmd + verify_expect.
    Deterministic, no API spend. Returns JSON (turn or list of turns).
    """
    from mmorch.canal import post as _canal_post, read as _canal_read
    if action == "read":
        return json.dumps(_canal_read(n), ensure_ascii=False)
    if action == "post":
        rec = _canal_post(
            src, kind, body,
            artifacts=artifacts, verify_cmd=verify_cmd,
            verify_expect=verify_expect, to=to,
        )
        return json.dumps(rec, ensure_ascii=False)
    raise ValueError("action must be 'post' or 'read'")


@_tool
def mmorch_vault_write(
    title: str,
    body: str,
    project: str,
    folder: str = "research",
    status: str = "seed",
    confidence: str = "",
    sources: str = "",
    tags: str = "",
) -> str:
    """Write a research note to the GLOBAL knowledge vault (single validated door,
    spec vault-global ticket 03). Validates title + project tag (hard minimum),
    autocompletes `created`, regenerates the project's MOC, bridges a gist to
    memory (scope global -> recall finds it, sessions read the note by path) and
    fires babel ingest async (gates decide if the .babel.md pays; the nightly
    sweep is the safety net). `sources`/`tags` = comma-separated. The original
    note is ALWAYS the source of truth. Returns JSON {path, moc}.
    """
    # W5.1: toda la orquestacion (frontmatter CSV, bridge a memoria, babel async)
    # vive en la libreria; este wrapper solo adapta tipos
    from mmorch.vault import write_research_note
    p, moc = write_research_note(title, body, project=project, folder=folder,
                                 status=status, confidence=confidence,
                                 sources=sources, tags=tags)
    return json.dumps({"path": str(p), "moc": str(moc)}, ensure_ascii=False)


@_tool
def mmorch_recall(
    query: str,
    scope: str = "global",
    k: int = 5,
    window_days: float | None = None,
) -> str:
    """Clinical two-stage recall: COARSE (scope-chain + recency, NO keyword gate) ->
    FINE (local embedding rerank). Falls back to immutable episodic raw if distilled
    notes fall short. Local embeddings = zero key/cost; degrades to recency-order if
    fastembed absent. scope levels are LITERAL names (task_id|subsector|project_id|
    mmorch_self|global — "task_id" IS a scope, not a placeholder); any other string
    chains [scope, global]. Bounds: 1 <= k <= 200, window_days > 0 (else {error,
    kind:"invalid_input"}). NOT strictly read-only: each recall bumps the returned
    notes' access_count/last_accessed_at (spacing effect — affects what decay forgets
    later). Returns JSON list of {id, ts, scope, text, score, layer}.
    """
    notes = _recall(query, scope=scope, k=k, window_days=window_days)
    return json.dumps([
        {"id": n.id, "ts": n.ts, "scope": n.scope, "text": n.text,
         "score": round(n.score, 4), "layer": n.layer} for n in notes],
        ensure_ascii=False)


@_tool
def mmorch_tournament(
    candidates: list[str],
    criterion: str,
    gen_model: str = DEFAULT_GENERATOR,
    judge_model: str = DEFAULT_VERIFIER,
) -> str:
    """Pick the BEST of a few candidates by taste/quality (naming, design, copy) via
    PAIRWISE single-elimination with a CROSS-FAMILY judge (OneFlow enforced). A tie
    escalates to the orchestrator (Opus) instead of forcing a winner. Spends external
    $, not cupo. Returns JSON {winner, escalate, rounds, comparisons, cost_usd}.
    """
    r = _tournament(candidates, criterion=criterion, gen_model=gen_model,
                    judge_model=judge_model, phase="mcp")
    return json.dumps({
        "winner": r.winner, "escalate": r.escalate, "rounds": r.rounds,
        "comparisons": r.comparisons, "cost_usd": r.cost_usd}, ensure_ascii=False)


@_tool
def mmorch_bucket_rank(
    items: list[str],
    rubric: str,
    tiers: list[str] | None = None,
) -> str:
    """Grade a LARGE set into quality tiers (triage, rank N>>10). Each item classified
    independently by a cheap model in parallel (O(n), not pairwise O(n^2)). Items never
    lost: a failed/unparseable grade falls to the lowest tier. Spends external $, not
    cupo. Returns JSON {by_tier, graded, cost_usd, n_failed}.
    """
    r = _bucket_rank(items, rubric=rubric, tiers=tiers, phase="mcp")
    return json.dumps({
        "by_tier": r.by_tier, "graded": r.graded, "cost_usd": r.cost_usd,
        "n_failed": r.n_failed}, ensure_ascii=False)


@_tool
def mmorch_classify(
    request: str,
    classes: dict,
    router_model: str = DEFAULT_ROUTER,
) -> str:
    """Triage front-door: a cheap model classifies the request into one of `classes`
    ({name: description}) and self-scores confidence. Returns the label so the
    orchestrator (Opus) can act on the right branch. Cheap external $, not cupo.
    Returns JSON {cls, confidence, cost_usd}. (Acting via Python handlers is the
    library API classify_and_act; over MCP this returns the label only.)
    """
    cls, conf, cost = _classify(request, dict(classes), router_model=router_model, phase="mcp")
    return json.dumps({"cls": cls, "confidence": conf, "cost_usd": round(cost, 6)},
                      ensure_ascii=False)


@_tool
def mmorch_cynefin(
    request: str,
    router_model: str = DEFAULT_ROUTER,
    threshold: float = 0.6,
) -> str:
    """Cynefin triage (P1): a cheap model maps the request to a complexity domain
    (clear|complicated|complex|chaotic) via the DART cause->effect question, and
    recommends an mmorch strategy. 'chaotic' or low-confidence -> escalate to the
    orchestrator (Opus). Cheap external $, not cupo. Returns JSON
    {domain, confidence, strategy, escalate, cost_usd}."""
    r = _cynefin(request, router_model=router_model, threshold=threshold, phase="mcp")
    return json.dumps({"domain": r.domain, "confidence": r.confidence,
                       "strategy": r.strategy, "escalate": r.escalate,
                       "cost_usd": r.cost_usd}, ensure_ascii=False)


@_tool
def mmorch_spec_interview(request: str, n: int = 5) -> str:
    """Spec layer-1 interview: a cheap model generates up to `n` short questions that
    uncover the GOAL behind the task (the decision it drives), not the surface task.
    The orchestrator asks the user, then feeds answers to mmorch_build_spec. Cheap
    external $, not cupo. Returns JSON {questions, cost_usd}."""
    qs, cost = _spec_interview(request, n=n, phase="mcp")
    return json.dumps({"questions": qs, "cost_usd": round(cost, 6)}, ensure_ascii=False)


@_tool
def mmorch_build_spec(request: str, answers: str = "") -> str:
    """Spec-builder (draft -> refute cross-family -> gate). A cheap drafter writes the
    spec with its over-inferences in a SEPARATE channel; a cross-family critic labels
    each inference SAFE/BEYOND_INTENT/WRONG (refutes by default). Only SAFE inferences
    fold into the spec; BEYOND_INTENT become open_questions for the user (never applied
    silently); WRONG are dropped. Gross overreach -> escalate=true (Opus). If the drafter
    smuggled unstated scope INTO the spec body, the result is quarantined=true: `spec` is
    blanked and the dirty draft is preserved in `raw_draft` for Opus to review — never
    hand `raw_draft` to a user as a clean spec. Cheap external $, not cupo. Returns JSON
    {spec, accepted_inferences, open_questions, dropped, escalate, quarantined, raw_draft,
    verifier_model, cost_usd}."""
    r = _build_spec(request, answers=answers, phase="mcp")
    return json.dumps({"spec": r.spec, "accepted_inferences": r.accepted_inferences,
                       "open_questions": r.open_questions, "dropped": r.dropped,
                       "escalate": r.escalate, "quarantined": r.quarantined,
                       "raw_draft": r.raw_draft, "verifier_model": r.verifier_model,
                       "cost_usd": r.cost_usd}, ensure_ascii=False)


@_tool
def mmorch_ingest_session(path: str = "latest") -> str:
    """Learn from a Claude Code session transcript: parse it, derive deterministic
    outcomes (external signal only), and calibrate the Cynefin router against the
    observed difficulty. path="latest" picks the most recent settled session under
    ~/.claude/projects. Parse/outcome/difficulty are local; calibration sends ONLY the
    request prompt text to the cheap router (never the transcript, reasoning, tool
    output, or secrets). Returns JSON {session, segments, recorded, skipped_no_signal,
    already_ingested, recorder_failed}."""
    r = _ingest_session(path)
    return json.dumps({"session": r.session, "segments": r.segments,
                       "recorded": r.recorded, "skipped_no_signal": r.skipped_no_signal,
                       "already_ingested": r.already_ingested,
                       "recorder_failed": r.recorder_failed}, ensure_ascii=False)


@_tool
def mmorch_session_playbooks(path: str = "latest", domain: str = "") -> str:
    """Mine reusable workflow playbooks from Claude sessions: ingest the session's
    successful tool-call sequences (labeled by EXTERNAL outcome) and return the
    recurring ones ranked by real success rate, optionally filtered by Cynefin domain
    (clear|complicated|complex|chaotic). 100% LOCAL — sends nothing to any API. Returns
    JSON {ingested, playbooks:[{domain, tool_sequence, n_observed, n_success, success_rate}]}."""
    n = _ingest_workflows(path)
    books = _top_playbooks(domain=domain or None)
    return json.dumps({"ingested": n, "playbooks": [
        {"domain": b.domain, "tool_sequence": list(b.tool_sequence),
         "n_observed": b.n_observed, "n_success": b.n_success,
         "success_rate": b.success_rate} for b in books]}, ensure_ascii=False)


@_tool
def mmorch_record_outcome(
    arm: str,
    reward: float,
    pattern: str = "",
    predicted_conf: float | None = None,
    source: str = "",
    context: str = "",
    model_version: str = "",
) -> str:
    """CLOSE THE FEEDBACK LOOP (keystone). After you (the orchestrator) use a cheap
    mmorch result and learn whether it was actually right, call this with the real
    label so the bandit + calibration learn. This is what was missing: 611 calls
    logged but ~1 outcome -> the learning machinery was starved.

    arm: the decision being scored, e.g. "deepseek-chat@0.6" or "gemini-2.5-flash".
    reward: [0,1] real outcome — 1=correct, 0=wrong, fraction=partial. NOT the
    model's self-reported confidence (anti-sycophancy: agreement != confirmation).
    predicted_conf: what the system believed at decision time (enables calibration/ECE);
    clamped to [0,1] at write time, same as reward.
    source: where the label came from (opus|downstream|test|human). Default "" — same
    as the library (W5.1: the old MCP-only default "opus" mislabeled outcomes whose
    label came from elsewhere).
    model_version: EXACT model name/version the provider reported serving (from
    CallResult.model_version). Providers rotate versions behind stable arm keys;
    logging it lets you invalidate/segment bandit priors when the version rotates (W5.3).

    Records the labeled outcome; with a non-empty `context` the library also trains the
    signature-keyed bandit (the ONLY bandit since W4.3 — the old flat state file was a
    zombie: written here but never read for any decision). Same semantics as calling
    mmorch.record_outcome directly (W5.1: wrapper without its own logic).
    Returns JSON {recorded, arm, reward, bandit: {mean, n}} for the sig-keyed arm.
    """
    o = _record_outcome(arm, reward, pattern=pattern, predicted_conf=predicted_conf,
                        source=source, context=context, model_version=model_version)
    bandit_stats: dict = {}
    if context:
        from mmorch.intuition import arm_stats
        bandit_stats = arm_stats(arm, context)
    return json.dumps({
        "recorded": True, "arm": o.arm, "reward": o.reward,
        "bandit": bandit_stats}, ensure_ascii=False)


@_tool
def mmorch_feedback_stats() -> str:
    """Inspect the feedback loop: the REAL signature-keyed Thompson bandit posteriors
    (arm = "model#signature"; the flat zombie state this used to report was removed
    in W4.3) + calibration (ECE conf-predicted vs reality, accuracy
    per arm). Read-only, no spend. Use to see whether the loop is actually learning
    (n>0 across arms) and whether self-confidence is trustworthy (low ECE) or lying
    (high ECE -> raise thresholds). Returns JSON {bandit, calibration}."""
    return json.dumps({
        "bandit": _ThompsonBandit().stats(),
        "calibration": _calibration(),
    }, ensure_ascii=False)


@_tool
def mmorch_check(checker: str, ctx: dict) -> str:
    """DETERMINISTIC tool-verify (checkers.py) — zero API, 100% reliable where an LLM
    verifier is ~74% false-refute on hard checkable math. 21 registered checkers:
    arithmetic, code_quality, mutation_score, coverage, deterministic, determinant,
    json_schema, predicate, checksum, python_ast_valid, regex_format, set_equal,
    numeric_close, sorted_monotonic, number_theory, sql_valid, units, sympy_identity,
    python_exec, unit_test, no_tell. ctx is the checker's kwargs. E.g.
    checker="arithmetic", ctx={"expr": "comb(20,10)", "expected": 184756}. Unknown
    checker / bad ctx returns {error, kind:"invalid_input"} listing the valid names.
    Use this INSTEAD of an LLM verifier when the claim has computable ground-truth.
    Returns {passed, detail, checker, expected, got}."""
    from mmorch.checkers import check as _check
    r = _check(checker, **dict(ctx))
    return json.dumps({"passed": r.passed, "detail": r.detail, "checker": r.checker,
                       "expected": r.expected, "got": r.got}, ensure_ascii=False, default=str)


@_tool
def mmorch_evolve_self(target_file: str, finding: str) -> str:
    """Auto-evolución DRY (seguro por MCP: PROPONE + evalúa, NUNCA aplica). Un modelo
    barato propone un cambio a `target_file` para resolver `finding`; se clasifica por
    zona (reversibilidad×blast-radius, incluyendo scan de acciones peligrosas en el
    código generado) y se corre la fitness compuesta SIN tests (ast + goal_aligned +
    ensemble cross-family + cost/budget). NO toca el repo, NO mergea. Aplicar de verdad
    = acción deliberada de librería/humano (sandbox_branch -> PR -> merge humano). Spends
    external $ (swarm+verify), not cupo. Returns {zone, would_apply, checks, refused_red}."""
    from mmorch.evolve import propose_patch, snapshot_change, zone_of, evaluate
    # propose_patch ya extrae el fence (textutil.extract_fence); el segundo strip
    # heuristico que vivia aca podia truncar outputs con fences internos (W5.1, hueco #9)
    after = propose_patch(target_file, finding)
    change = snapshot_change(target_file, after, f"auto-evolve: {finding}")
    zone = zone_of(change)
    if zone == "red":
        return json.dumps({"zone": "red", "would_apply": False, "refused_red": True,
                           "reason": "zona roja -> gate humano, nunca auto-aplica",
                           "change_id": change.id}, ensure_ascii=False)
    ev = evaluate(change)   # sin mutar repo; tests reales = sandbox_branch aparte
    return json.dumps({"zone": zone, "would_apply": bool(ev["ok"]) and zone in ("green", "yellow"),
                       "checks": ev["checks"], "ensemble_degraded": ev.get("ensemble_degraded"),
                       "change_id": change.id, "note": "DRY: no aplicado. Promote = accion humana."},
                      ensure_ascii=False)


@_tool
def mmorch_evolve_nightly(days: int = 3, max_files: int = 5, max_findings: int = 8) -> str:
    """Loop nocturno end-to-end (pensado para un scheduled-task, cero cupo): cosecha
    hallazgos REALES con code_review sobre archivos cambiados en los últimos `days` días
    (hasta `max_files`), propone un fix por hallazgo (hasta `max_findings`), y para cada
    uno corre sandbox_branch (worktree aislado, tests reales) + abre un PR si queda verde.
    Coordinado por archivo: si un archivo YA tiene un PR pendiente, se saltea esta ronda
    (nunca 2 PRs compitiendo por el mismo archivo — ver evolve.coordinated_evolve_round).
    NUNCA mergea — el humano revisa y mergea cada PR. Zona roja sigue bloqueada siempre.
    Returns {findings, opened:[archivos con PR nuevo], skipped_active_pr, red, blocked_zone_red}."""
    from mmorch.evolve import nightly_evolve
    return json.dumps(nightly_evolve(days=days, max_files=max_files, max_findings=max_findings),
                      ensure_ascii=False)


@_tool
def mmorch_orchestra() -> str:
    """Roster de la ORQUESTA que mmorch dirige: conductor + secciones (generator/verifier/
    router/soloist/memory) con cada nodo (handle, kind, builder algorithm, status). Vista
    consultable del registry de nodos. Read-only, no spend."""
    from mmorch.nodes import summary
    return json.dumps(summary(), ensure_ascii=False)


@_tool
def mmorch_consolidate(scope: str = "", sim_threshold: float = 0.92,
                       forget: bool = False, apply: bool = False) -> str:
    """Periodic memory maintenance (run every ~10 sessions): merge near-duplicate
    semantic notes per scope (identical text or embedding cosine >= sim_threshold),
    tombstoning losers — keeper is the verified note first, then the most recent.
    Episodic raw log is never touched; the run itself is logged as an episodic
    'consolidation' event. forget=true (default OFF) adds an Ebbinghaus-decay forget
    pass: tombstones notes whose retention score fell below threshold, EXCEPT
    verified / open_loop / permanent ones. Forgetting never loses a fact — only the
    distilled note is tombstoned; the raw episode survives and recall falls back to
    it. Default is a DRY RUN (reports what would change); pass apply=true to actually
    tombstone. NOTE inverted polarity vs the library: mmorch.memory.consolidate takes
    dry_run (default False = applies); this tool takes apply (default False = dry run)
    — don't carry kwargs verbatim from one to the other. Also reports live-note bytes + over_budget flag. Deterministic, zero
    API spend. Returns JSON {merged, tombstoned, forgotten, live_notes, bytes,
    over_budget, dry_run}."""
    return json.dumps(
        _mem_consolidate(scope or None, sim_threshold=sim_threshold,
                         forget=forget, dry_run=not apply),
        ensure_ascii=False)


@_tool
def mmorch_memory_stats() -> str:
    """Memory counts: episodic events, live semantic notes, embedded notes, verified
    notes + verification_coverage (share of live notes independently validated — low
    coverage means recall serves unvalidated learning), and the active embedding
    backend (or null if fastembed absent). Read-only, no spend."""
    return json.dumps(_mem_stats(), ensure_ascii=False)


@_tool
def mmorch_reinforce(note_id: int, boost: int = 3) -> str:
    """Reconsolidation — CONFIRM a recalled note (you used/validated it). Bumps its
    access_count by `boost` (a confirm ~ several accesses) and refreshes last-access,
    raising its retention score so decay won't forget it. Deterministic, no spend.
    Returns JSON {note_id, boost, ok} — ok=false means the note does not exist (W5.1:
    no more silent success over 0 rows)."""
    ok = _reinforce(note_id, boost=boost)
    return json.dumps({"note_id": note_id, "boost": boost, "ok": ok}, ensure_ascii=False)


@_tool
def mmorch_flag_contradiction(note_id: int) -> str:
    """Reconsolidation — CONTRADICT a note (the user/evidence says it's wrong). Marks
    it needs_review: recall stops surfacing it (no repeating suspected-false info) and
    falls back to the immutable raw episode. The note is NOT deleted — resolve later
    with mmorch_resolve_review. Self-correcting memory. Deterministic, no spend.
    Returns JSON {note_id, ok} — ok=false means the note does not exist."""
    ok = _flag_contradiction(note_id)
    return json.dumps({"note_id": note_id, "ok": ok}, ensure_ascii=False)


@_tool
def mmorch_pending_review(scope: str = "") -> str:
    """List semantic notes flagged as contradicted (needs_review) and not yet resolved,
    so you can review/supersede them. Read-only, no spend. Returns JSON list of
    {id, ts, scope, text}."""
    notes = _pending_review(scope or None)
    return json.dumps([
        {"id": n.id, "ts": n.ts, "scope": n.scope, "text": n.text} for n in notes],
        ensure_ascii=False)


@_tool
def mmorch_resolve_review(note_id: int, drop: bool = False) -> str:
    """Resolve a contradiction. drop=true tombstones the note (it was false);
    drop=false clears needs_review so it surfaces again (the contradiction was wrong).
    The raw episode is never touched either way. Deterministic, no spend. Returns JSON
    {note_id, dropped, ok} — ok=false means the note does not exist."""
    ok = _resolve_review(note_id, drop=drop)
    return json.dumps({"note_id": note_id, "dropped": drop, "ok": ok}, ensure_ascii=False)


@_tool
def mmorch_close_loop(note_id: int) -> str:
    """Close an open-loop note (the task/question is resolved): clears the Zeigarnik
    flag so the note becomes eligible for normal decay/forgetting again. Deterministic,
    no spend. Returns JSON {note_id, ok} — ok=false means the note does not exist."""
    ok = _close_loop(note_id)
    return json.dumps({"note_id": note_id, "ok": ok}, ensure_ascii=False)


@_tool
def mmorch_open_loops(scope: str = "") -> str:
    """Surface unfinished tasks/questions (notes flagged open_loop, Zeigarnik). Inject
    these proactively when resuming work — they resist forgetting until closed. Read-only,
    no spend. Returns JSON list of {id, ts, scope, text}."""
    notes = _open_loops(scope or None)
    return json.dumps([
        {"id": n.id, "ts": n.ts, "scope": n.scope, "text": n.text} for n in notes],
        ensure_ascii=False)


@_tool
def mmorch_find_tension(scope: str = "", lo: float = 0.82, hi: float = 0.92,
                        max_per_scope: int = 500) -> str:
    """Curiosity — surface pairs of notes that are suspiciously close (lo <= cosine < hi)
    but were NOT auto-merged: same topic, different wording = where redundancy or
    CONTRADICTION hides. Deterministic candidates for YOU to judge (merge via
    consolidate, conflict via flag_contradiction, or leave both). Embeddings give topical
    similarity, not logical contradiction — so this proposes, you decide; no LLM-judge.
    O(n^2) per scope; a scope with > max_per_scope notes is skipped (reported in
    `skipped`, never silently dropped). Needs fastembed (returns no pairs without it).
    Zero spend. Returns JSON {pairs:[{a,b,scope,cosine,question}], skipped:[{scope,n}]}."""
    return json.dumps(_find_tension(scope or None, lo=lo, hi=hi,
                                    max_per_scope=max_per_scope), ensure_ascii=False)


@_tool
def mmorch_forget_preview(scope: str = "") -> str:
    """METRICS GATE before turning on consolidate(forget=true): read-only, shows how
    many live forgettable notes would be dropped at a grid of decay knobs (lambda,
    threshold), WITHOUT tombstoning anything. verified/open_loop/permanent notes are
    never eligible. Use this to pick knobs from evidence instead of guessing — it does
    NOT auto-tune (no ground-truth label for 'worth forgetting' = tuning would be reward
    hacking). Deterministic, zero spend. Returns JSON {total_live, eligible,
    grid:[{lam, forget, would_forget, pct_eligible}]}."""
    return json.dumps(_forget_preview(scope or None), ensure_ascii=False)


@_tool
def mmorch_rubric_start(task: str, criteria: list, K: int = 5) -> str:
    """Start an autocorrection RUBRIC LOOP in PLAN mode (the session's own models do the
    LLM work = plan quota, ZERO API spend; deterministic checkers run server-side for $0).
    criteria: list of {"id","desc","kind":"checkable","checker","ctx"} or {"id","desc",
    "kind":"subjective"}. ctx strings may use "{attempt_code}"/"{attempt}" placeholders.
    Returns the loop STATE (JSON) — pass it to mmorch_rubric_next to get the next action.
    Drive it: next -> (you execute the prompt with a SEPARATE subagent per role; executor
    and judge must NEVER share context) -> submit -> repeat until role=done|escalate."""
    from mmorch.rubric_loop import start_rubric
    return json.dumps(start_rubric(task, list(criteria), K=K), ensure_ascii=False)


@_tool
def mmorch_rubric_next(state: dict) -> str:
    """Next action for a rubric loop state: {"role":"executor"|"judge","prompt":...} —
    run the prompt in a FRESH subagent (separate context per role, judge never generates)
    and feed the output to mmorch_rubric_submit. Or {"role":"done"|"escalate","summary"}:
    done = 100% rubric verified; escalate = K exhausted, summary carries pending criteria
    with executable evidence — hand it to the human. Deterministic, no spend."""
    from mmorch.rubric_loop import next_action
    return json.dumps(next_action(dict(state)), ensure_ascii=False)


@_tool
def mmorch_rubric_submit(state: dict, output: str) -> str:
    """Submit the current role's output to the rubric-loop MANAGER (deterministic).
    Executor outputs trigger server-side CHECKER re-execution (evidence = local sandbox
    runs, never the executor's claims). Judge outputs must be the JSON verdict array;
    illegible JSON = refute-by-default. On done/escalate the loop self-closes: reward =
    verified rubric fraction -> record_outcome(context=task) feeding bandit + ShadowPrior,
    and a verified rule is distilled to memory if corrections happened. Returns the new
    state — chain into mmorch_rubric_next."""
    from mmorch.rubric_loop import submit
    return json.dumps(submit(dict(state), output), ensure_ascii=False)


@_tool
def mmorch_perfect(request: str, n: int = 4) -> str:
    """Built-in prompt perfectioner (cero cupo, HEADLESS — no human turn): in ONE call it uncovers
    the GOAL questions (interview) AND builds a cross-family-refuted spec (build_spec). Returns the
    spec plus merged `open_questions` (interview goal-Qs + the spec's BEYOND_INTENT inferences) that a
    caller should still DECIDE — never auto-resolved. Honors quarantine/escalate. This is the
    mmorch-native twin of the interactive /perfect skill (which asks the human the questions); use it
    for automated callers (Lotus, a workflow pre-sharpening a task, an agent self-sharpening). Returns
    JSON {spec, open_questions, goal_questions, accepted_inferences, dropped, escalate, quarantined,
    raw_draft, verifier_model, cost_usd}."""
    return json.dumps(_perfect(request, n=n), ensure_ascii=False)


@_tool
def mmorch_speedup(source: str, setup: str, call: str, runs: int = 5, rounds: int = 8) -> str:
    """Make a Python function faster, cero cupo, kept ONLY on a MEASURED + CORRECT improvement. A
    cheap generator proposes a faster variant; the score is a RUNNABLE rubric (never an LLM judge) =
    correctness-gated runtime: the candidate runs in a fresh subprocess on a fixed benchmark — `setup`
    builds the inputs, `call` invokes the function (e.g. setup='data=list(range(100000))',
    call='f(data)'). A result diverging from the original scores inf (rejected — fast-but-wrong is a
    regression); else its median seconds. hillclimb keeps the fastest correct candidate; falls back to
    the original if no real margin. Vectorize/Numba/algorithmic = whatever the generator proposes;
    execution decides. Returns JSON {best, baseline_sec, best_sec, speedup, rounds, stopped, kept}."""
    return json.dumps(_speedup(source, setup=setup, call=call, runs=runs, rounds=rounds),
                      ensure_ascii=False)


def main() -> None:
    """Entry point instalable (`mmorch-mcp`) — arranca el server stdio."""
    # dead-man visible (W4.4): grita por stderr (stdout es protocolo MCP)
    from mmorch.health import nightly_watchdog
    nightly_watchdog()
    mcp.run()


if __name__ == "__main__":
    main()
