"""Code-flow patterns (§7), migrated as deterministic Python.

MVP: fan_out (bulk in parallel) + adversarial_verify (cross-family skeptic).
Hard rules enforced here:
  - OneFlow (§7): never a homogeneous multi-agent. Verifier MUST differ in family
    from the generator, else the multi-agent is simulable by one agent → wasted.
  - Anti-sycophancy (§8): the verifier is prompted to REFUTE by default; agreement
    is not treated as confirmation.
"""
from __future__ import annotations

import json
import warnings
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import dataclass

from .config import DEFAULT_GENERATOR, DEFAULT_VERIFIER, family_of
from .providers import CallResult, call
from .metrics import log_event


# --------------------------------------------------------------------------- #
# fan-out-and-synthesize                                                       #
# --------------------------------------------------------------------------- #
def fan_out(
    prompts: list[str],
    *,
    gen_model: str = DEFAULT_GENERATOR,
    system: str | None = None,
    max_workers: int = 8,
    phase: str = "",
    temperature: float = 0.3,
) -> list[CallResult]:
    """Run N independent generation tasks in parallel on a cheap node.

    Parallel evidence acquisition beats deep sequential iteration (§3,
    'Search More, Think Less'). Each sub-task is logged separately.
    """
    def _one(idx_prompt):
        i, p = idx_prompt
        msgs = []
        if system:
            msgs.append({"role": "system", "content": system})
        msgs.append({"role": "user", "content": p})
        # H-1: degradacion graceful. Un fallo por-tarea (red/rate-limit en UN prompt)
        # NO aborta el batch ni pierde los exitosos. providers.call ya loggea el
        # evento error (H-2); aca devolvemos None para esa tarea y seguimos.
        try:
            res = call(
                gen_model,
                msgs,
                pattern="fan_out",
                node=f"gen[{i}]",
                phase=phase,
                temperature=temperature,
            )
        except Exception:
            res = None
        return i, res

    results: list[CallResult | None] = [None] * len(prompts)
    with ThreadPoolExecutor(max_workers=max_workers) as ex:
        futs = [ex.submit(_one, (i, p)) for i, p in enumerate(prompts)]
        for fut in as_completed(futs):
            i, res = fut.result()
            results[i] = res
    ok = [r for r in results if r is not None]
    # #6: cobertura NO silenciosa. Dropear fallidos sin avisar viola "no silent caps":
    # el caller no distingue 8/10 de 10/10 (una sintesis sobre 6 prompts es parcial).
    n_failed = len(prompts) - len(ok)
    if n_failed:
        failed_idx = [i for i, r in enumerate(results) if r is None]
        warnings.warn(f"fan_out: {n_failed}/{len(prompts)} prompts fallaron y se "
                      f"dropearon (indices {failed_idx[:10]}). Resultado PARCIAL.",
                      stacklevel=2)
        log_event(pattern="fan_out_coverage", node="coverage", model=gen_model,
                  family=family_of(gen_model), in_tokens=0, out_tokens=0, cost_usd=0.0,
                  latency_s=0.0, phase=phase, n_prompts=len(prompts), n_ok=len(ok),
                  n_failed=n_failed)
    return ok


# --------------------------------------------------------------------------- #
# adversarial verification (cross-family)                                      #
# --------------------------------------------------------------------------- #
@dataclass
class Verdict:
    passed: bool
    confidence: float
    refutations: list[str]
    raw: str
    verifier_model: str
    cost_usd: float


_SKEPTIC_SYSTEM = (
    "You are an adversarial verifier from a DIFFERENT model family than the author. "
    "Your job is to REFUTE the artifact, not to praise it. Assume it is flawed until "
    "proven otherwise. Agreement is NOT confirmation. Check against the rubric. "
    "If you concede a point, state: 'CEDO porque [refuted premise] + [rule/evidence]'. "
    "Respond ONLY with minified JSON: "
    '{"passed": bool, "confidence": 0..1, "refutations": [string, ...]}'
)


def adversarial_verify(
    artifact: str,
    *,
    rubric: str,
    gen_model: str = DEFAULT_GENERATOR,
    verifier_model: str = DEFAULT_VERIFIER,
    phase: str = "",
    task_kind: str = "subjective",
    checker: str | None = None,
    checker_ctx: dict | None = None,
) -> Verdict:
    """Verify an artifact with a skeptic. Cross-family enforcement is TASK-AWARE (#2).

    DETERMINISTIC TOOL-VERIFY: if `checker` is given (with task_kind='checkable'), the
    claim is verified by CODE (checkers.py), not an LLM — 100% reliable & free where an
    LLM verifier is ~74% false-refute on hard math. Returns a Verdict (confidence=1.0,
    no API). E.g. checker='arithmetic', checker_ctx={'expr':'comb(20,10)','expected':...}.

    task_kind="subjective" (default, SAFE): no computable ground-truth (design, copy,
      prose, judgement). Cross-family is REQUIRED — same-family is refused (raise).
      Rationale: for subjective output a model can endorse its own blind spot; an
      independent family decorrelates.
    task_kind="checkable": the claim has a deterministic ground-truth (math, code that
      runs, a fact). Empirically (§18.4 + ablation_symmetric n=350) cross-family does
      NOT improve detection here, so same-family is ALLOWED (cost lever, ~6x cheaper).
      CAVEAT: on HARD checkable tasks LLM verification is itself unreliable (~74%
      false-refute, ablation_prompt) regardless of family — prefer a TOOL/code check
      over any LLM verifier when you can compute the truth directly.
    """
    # tool-verify determinista: cero API, 100% confiable en lo computable.
    if checker is not None:
        from .checkers import check
        r = check(checker, **(checker_ctx or {}))
        log_event(pattern="adversarial_verify_verdict", node=f"checker:{checker}",
                  model=f"tool:{checker}", family="deterministic", in_tokens=0,
                  out_tokens=0, cost_usd=0.0, latency_s=0.0, phase=phase,
                  passed=r.passed, confidence=1.0, n_refutations=0 if r.passed else 1)
        return Verdict(passed=r.passed, confidence=1.0,
                       refutations=[] if r.passed else [r.detail],
                       raw=r.detail, verifier_model=f"tool:{checker}", cost_usd=0.0)

    if task_kind != "checkable" and family_of(gen_model) == family_of(verifier_model):
        raise ValueError(
            f"OneFlow violation: generator ({gen_model}, {family_of(gen_model)}) and "
            f"verifier ({verifier_model}, {family_of(verifier_model)}) share a family "
            f"for a SUBJECTIVE task. Use a cross-family verifier (§4), or pass "
            f"task_kind='checkable' if the claim has computable ground-truth."
        )

    user = (
        f"RUBRIC:\n{rubric}\n\n"
        f"ARTIFACT TO REFUTE:\n{artifact}\n\n"
        "Return the JSON verdict."
    )
    res = call(
        verifier_model,
        [
            {"role": "system", "content": _SKEPTIC_SYSTEM},
            {"role": "user", "content": user},
        ],
        pattern="adversarial_verify",
        node="verifier",
        phase=phase,
        temperature=0.0,
    )

    passed, confidence, refutations = _parse_verdict(res.text)
    # Log del VERDICT (gap detectado por learn.recommend, 2026-06-07): sin esto no
    # hay proxy de calidad por verificador -> no se puede auto-tunear con fundamento.
    # cost 0 (la API ya se cobro en el call de arriba); este evento es el resultado.
    log_event(
        pattern="adversarial_verify_verdict",
        node="verdict",
        model=verifier_model,
        family=family_of(verifier_model),
        in_tokens=0, out_tokens=0, cost_usd=0.0, latency_s=0.0,
        phase=phase,
        passed=passed,
        confidence=confidence,
        n_refutations=len(refutations),
    )
    return Verdict(
        passed=passed,
        confidence=confidence,
        refutations=refutations,
        raw=res.text,
        verifier_model=verifier_model,
        cost_usd=res.cost_usd,
    )


def _coerce_passed(v) -> bool:
    """H-5b: parse robusto de `passed`. bool("false") es True -> aceptaria lo que
    deberia rechazar (rompe anti-sicofancia). String -> comparar contra truthy set."""
    if isinstance(v, str):
        return v.strip().lower() in ("true", "1", "yes", "si", "sí")
    return bool(v)


def _coerce_conf(v) -> float:
    """H-5a: clamp confidence a [0,1]. Un LLM puede devolver 5.0 o -0.5."""
    try:
        return max(0.0, min(1.0, float(v)))
    except (TypeError, ValueError):
        return 0.0


def _parse_verdict(text: str) -> tuple[bool, float, list[str]]:
    """Best-effort JSON extraction from the verifier reply."""
    s = text.strip()
    # strip code fences if present
    if s.startswith("```"):
        s = s.split("```", 2)[1] if "```" in s[3:] else s
        # H-5c: removeprefix (no lstrip, que borra CUALQUIER char j/s/o/n).
        s = s.strip().removeprefix("json").strip().strip("`").strip()
    start, end = s.find("{"), s.rfind("}")
    if start != -1 and end != -1 and end > start:
        s = s[start : end + 1]
    try:
        data = json.loads(s)
        return (
            _coerce_passed(data.get("passed", False)),
            _coerce_conf(data.get("confidence", 0.0)),
            list(data.get("refutations", [])),
        )
    except Exception:
        # Could not parse → treat as failed (skeptic default).
        return False, 0.0, [f"unparseable verifier output: {text[:200]}"]
