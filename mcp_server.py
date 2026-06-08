"""MCP wrapper — exposes mmorch patterns as tools to Claude Code.

This is the "ambos" path: the same library is callable both as plain Python
(harness migrado, §5) AND as MCP tools the orchestrator can invoke mid-session.

IMPORTANT (cupo discipline, §5): invoking these tools spends EXTERNAL API dollars,
NOT Claude cupo. That is the point — bulk/verify is offloaded off the plan.

Run (stdio):  python mcp_server.py
Register:     see README.md "Register the MCP server".
"""
from __future__ import annotations

import json

try:
    from mcp.server.fastmcp import FastMCP
except Exception as e:  # pragma: no cover
    raise SystemExit(
        "MCP SDK not installed. Run: pip install \"mcp>=1.2.0\"  "
        f"(original error: {e})"
    )

from mmorch import (fan_out, adversarial_verify, route, cascade, ensemble_verify,
                    ideate_and_screen, recall as _recall, tournament as _tournament,
                    bucket_rank as _bucket_rank)
from mmorch.config import DEFAULT_GENERATOR, DEFAULT_VERIFIER
from mmorch.metrics import summary
from mmorch.learn import analyze as _learn_analyze, recommend as _learn_recommend
from mmorch.memory import remember as _remember, stats as _mem_stats
from mmorch.classify import classify as _classify
from mmorch.config import DEFAULT_ROUTER
from mmorch.feedback import (record_outcome as _record_outcome,
                            ThompsonBandit as _ThompsonBandit,
                            calibration as _calibration)

mcp = FastMCP("mmorch")


@mcp.tool()
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


@mcp.tool()
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


@mcp.tool()
def mmorch_metrics_summary() -> str:
    """Return aggregate metrics (calls, total cost USD, cost by family)."""
    return json.dumps(summary(), ensure_ascii=False)


@mcp.tool()
def mmorch_route(
    prompt: str,
    gen_model: str = DEFAULT_GENERATOR,
    threshold: float = 0.7,
) -> str:
    """Confidence-gated routing (I-2). A cheap external model answers and
    self-scores; returns escalate=True if confidence < threshold so the
    orchestrator (Opus) only intervenes when needed. Spends external $, not cupo.
    Returns JSON {answer, confidence, escalate, model, cost_usd}.
    """
    r = route(prompt, gen_model=gen_model, threshold=threshold, phase="mcp")
    return json.dumps({
        "answer": r.answer, "confidence": r.confidence, "escalate": r.escalate,
        "model": r.model, "cost_usd": r.cost_usd}, ensure_ascii=False)


@mcp.tool()
def mmorch_cascade(
    prompt: str,
    steps: list[list] | None = None,
) -> str:
    """FrugalGPT-style cascade: cheapest model first + self-score; escalate to the
    next only if confidence < per-step threshold; flag Opus if all steps exhausted.
    Saves cupo (resolves cheap when possible). steps = [[model, threshold], ...].
    Returns JSON {answer, confidence, resolved_step, escalate, models_used, cost_usd}.
    """
    st = [(s[0], float(s[1])) for s in steps] if steps else None
    r = cascade(prompt, steps=st, phase="mcp")
    return json.dumps({
        "answer": r.answer, "confidence": r.confidence,
        "resolved_step": r.resolved_step, "escalate": r.escalate,
        "models_used": r.models_used, "cost_usd": r.cost_usd}, ensure_ascii=False)


@mcp.tool()
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
        "refutations": ev.refutations, "cost_usd": ev.cost_usd}, ensure_ascii=False)


@mcp.tool()
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


@mcp.tool()
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


@mcp.tool()
def mmorch_remember(
    scope: str,
    episode_text: str,
    kind: str = "note",
    verify: bool = False,
) -> str:
    """Persist a memory: append the raw episode (immutable) + distill a durable note
    (Thought-Retriever, cheap model) + embed it. scope is hierarchical
    (task_id<subsector<project_id<mmorch_self<global). If verify=True, a cross-family
    skeptic checks the note is faithful to the episode before persisting (else only
    the raw is kept). Spends a little external $, not cupo. Returns JSON
    {episode_id, note_id, distilled, persisted, refutations}.
    """
    return json.dumps(_remember(scope, episode_text, kind=kind, verify=verify),
                      ensure_ascii=False)


@mcp.tool()
def mmorch_recall(
    query: str,
    scope: str = "global",
    k: int = 5,
    window_days: float | None = None,
) -> str:
    """Clinical two-stage recall: COARSE (scope-chain + recency, NO keyword gate) ->
    FINE (local embedding rerank). Falls back to immutable episodic raw if distilled
    notes fall short. Local embeddings = zero key/cost; degrades to recency-order if
    fastembed absent. Returns JSON list of {id, ts, scope, text, score, layer}.
    """
    notes = _recall(query, scope=scope, k=k, window_days=window_days)
    return json.dumps([
        {"id": n.id, "ts": n.ts, "scope": n.scope, "text": n.text,
         "score": round(n.score, 4), "layer": n.layer} for n in notes],
        ensure_ascii=False)


@mcp.tool()
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


@mcp.tool()
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


@mcp.tool()
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


@mcp.tool()
def mmorch_record_outcome(
    arm: str,
    reward: float,
    pattern: str = "",
    predicted_conf: float | None = None,
    source: str = "opus",
    context: str = "",
) -> str:
    """CLOSE THE FEEDBACK LOOP (keystone). After you (the orchestrator) use a cheap
    mmorch result and learn whether it was actually right, call this with the real
    label so the bandit + calibration learn. This is what was missing: 611 calls
    logged but ~1 outcome -> the learning machinery was starved.

    arm: the decision being scored, e.g. "deepseek-chat@0.6" or "gemini-2.5-flash".
    reward: [0,1] real outcome — 1=correct, 0=wrong, fraction=partial. NOT the
    model's self-reported confidence (anti-sycophancy: agreement != confirmation).
    predicted_conf: what the system believed at decision time (enables calibration/ECE).
    source: where the label came from (opus|downstream|test|human).

    Records the labeled outcome AND updates the Thompson bandit posterior for `arm`.
    Returns JSON {recorded, arm, reward, bandit: {mean, n}}.
    """
    o = _record_outcome(arm, reward, pattern=pattern, predicted_conf=predicted_conf,
                        source=source, context=context)
    b = _ThompsonBandit()
    b.update(arm, reward)
    return json.dumps({
        "recorded": True, "arm": o.arm, "reward": o.reward,
        "bandit": b.stats().get(arm, {})}, ensure_ascii=False)


@mcp.tool()
def mmorch_feedback_stats() -> str:
    """Inspect the feedback loop: Thompson bandit posteriors per arm (mean reward, n)
    + calibration (ECE conf-predicted vs reality, accuracy per arm). Read-only, no
    spend. Use to see whether the loop is actually learning (n>0 across arms) and
    whether self-confidence is trustworthy (low ECE) or lying (high ECE -> raise
    thresholds). Returns JSON {bandit, calibration}."""
    return json.dumps({
        "bandit": _ThompsonBandit().stats(),
        "calibration": _calibration(),
    }, ensure_ascii=False)


@mcp.tool()
def mmorch_check(checker: str, ctx: dict) -> str:
    """DETERMINISTIC tool-verify (checkers.py) — zero API, 100% reliable where an LLM
    verifier is ~74% false-refute on hard checkable math. checker in {arithmetic,
    json_schema}; ctx is the checker's args. E.g. checker="arithmetic",
    ctx={"expr": "comb(20,10)", "expected": 184756}. Use this INSTEAD of an LLM verifier
    when the claim has computable ground-truth. Returns {passed, detail, checker, got}."""
    from mmorch.checkers import check as _check
    r = _check(checker, **dict(ctx))
    return json.dumps({"passed": r.passed, "detail": r.detail, "checker": r.checker,
                       "expected": r.expected, "got": r.got}, ensure_ascii=False, default=str)


@mcp.tool()
def mmorch_memory_stats() -> str:
    """Memory counts: episodic events, live semantic notes, embedded notes, and the
    active embedding backend (or null if fastembed absent). Read-only, no spend."""
    return json.dumps(_mem_stats(), ensure_ascii=False)


if __name__ == "__main__":
    mcp.run()
