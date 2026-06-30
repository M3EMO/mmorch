"""enrich — completar/especificar el prompt infiriendo intent del usuario (patron Fable 5),
PERO con red anti-sicofancia: las inferencias se hacen EXPLICITAS y un juez CROSS-FAMILY
refuta las que sobre-pasan. Fable infiere en silencio (riesgo: construir lo incorrecto con
confianza); aca cada requisito inferido es visible, verificado y vetable.

Roles separados (NO el juez enriquece — perderia independencia):
  ENRICHER (familia A): prompt crudo -> {enriched, assumptions[], questions[]}.
  JUEZ (familia B, cross-family): por cada assumption -> keep/reject (¿es inferencia
        RAZONABLE del original o un requisito inventado?). Refuta por default.
  -> prompt final = original + assumptions VERIFICADAS (kept) + questions abiertas.
     El texto libre del enricher es solo raw; lo que ENTRA lo gatea el juez.

Medible como scout (enrich_delta): ¿baja iteraciones / sube pass-rate? Hipotesis, no dogma.
"""
from __future__ import annotations

import json

from .config import DEFAULT_GENERATOR, DEFAULT_VERIFIER, family_of

from .textutil import extract_fence  # dedup of the local fence helper


def _extract_json(text: str):
    blob = extract_fence(text)
    try:
        return json.loads(blob)
    except Exception:
        s, e = blob.find("{"), blob.rfind("}")
        a, b = blob.find("["), blob.rfind("]")
        for lo, hi in ((s, e), (a, b)):
            if lo != -1 and hi != -1 and hi > lo:
                try:
                    return json.loads(blob[lo:hi + 1])
                except Exception:
                    pass
    return None


def _build_final(prompt: str, kept: list[str], questions: list[str]) -> str:
    out = [prompt.strip()]
    if kept:
        out.append("Requisitos inferidos (verificados cross-family):\n"
                   + "\n".join(f"- {a}" for a in kept))
    if questions:
        out.append("Preguntas abiertas (ambiguo — no asumido):\n"
                   + "\n".join(f"- {q}" for q in questions))
    return "\n\n".join(out)


def enrich_prompt(prompt: str, *, gen_model: str | None = None, judge_model: str | None = None,
                  gen_fn=None, judge_fn=None) -> dict:
    """Devuelve {enriched, assumptions(kept), rejected, questions, raw_enriched}.
    OneFlow: gen y judge cross-family (refuta misma-familia). gen_fn/judge_fn inyectables
    (tests/modo plan); por default usan providers.call (API barata)."""
    gen_model = gen_model or DEFAULT_GENERATOR
    judge_model = judge_model or DEFAULT_VERIFIER
    if family_of(gen_model) == family_of(judge_model):
        raise ValueError(f"OneFlow: enricher {gen_model} y juez {judge_model} misma familia")

    if gen_fn is None or judge_fn is None:
        from .providers import call
        def _mk(model):
            return lambda p: call(model, [{"role": "user", "content": p}],
                                  pattern="enrich", node=model).text
        gen_fn = gen_fn or _mk(gen_model)
        judge_fn = judge_fn or _mk(judge_model)

    gprompt = (
        "Sos un ENRICHER de prompts. El usuario escribio un pedido posiblemente SUB-especificado. "
        "Infieri lo que probablemente quiso decir y completalo. NO resuelvas la tarea. Devolve SOLO "
        'JSON: {"enriched": "<prompt mejorado>", "assumptions": ["requisito inferido 1", ...], '
        '"questions": ["lo genuinamente ambiguo que NO conviene asumir", ...]}\n\n'
        f"PEDIDO:\n{prompt}")
    g = _extract_json(gen_fn(gprompt)) or {}
    assumptions = [str(a) for a in (g.get("assumptions") or [])]
    questions = [str(q) for q in (g.get("questions") or [])]
    raw_enriched = str(g.get("enriched", ""))

    # refute-by-default: sin veredicto valido del juez -> NO entra ninguna assumption.
    kept, rejected = [], list(assumptions)
    if assumptions:
        jprompt = (
            "Sos un JUEZ independiente y ESCEPTICO. NO generaste estas inferencias. Por cada "
            "assumption decidi si es una inferencia RAZONABLE del pedido original o un requisito "
            "INVENTADO que el usuario no pidio ni implico. Refuta por default: si dudas, keep=false.\n\n"
            f"PEDIDO ORIGINAL:\n{prompt}\n\nASSUMPTIONS:\n"
            + "\n".join(f"{i}. {a}" for i, a in enumerate(assumptions))
            + '\n\nDevolve SOLO JSON: [{"i": <indice>, "keep": true|false, "reason": "..."}]')
        verdicts = _extract_json(judge_fn(jprompt))
        if isinstance(verdicts, list):
            keep_idx = {v.get("i") for v in verdicts if isinstance(v, dict) and v.get("keep")}
            kept = [a for i, a in enumerate(assumptions) if i in keep_idx]
            rejected = [a for i, a in enumerate(assumptions) if i not in keep_idx]

    return {"enriched": _build_final(prompt, kept, questions),
            "assumptions": kept, "rejected": rejected, "questions": questions,
            "raw_enriched": raw_enriched}


def enrich_delta(path=None) -> dict:
    """MIDE la hipotesis: ¿enrich reduce iteraciones? Compara n_iters medio de trayectorias
    con prompt enriquecido vs sin, sobre trajectory.load_trajectories. Si delta ~0, no se justifica."""
    from .trajectory import load_trajectories
    with_e: list = []
    without_e: list = []
    for t in load_trajectories(path):
        (with_e if t.get("enriched") else without_e).append(t.get("n_iters", 0))
    def _avg(xs):
        return round(sum(xs) / len(xs), 3) if xs else None
    return {"with_enrich": {"n": len(with_e), "avg_iters": _avg(with_e)},
            "without_enrich": {"n": len(without_e), "avg_iters": _avg(without_e)},
            "delta_iters": (None if not with_e or not without_e
                            else round(_avg(without_e) - _avg(with_e), 3))}
