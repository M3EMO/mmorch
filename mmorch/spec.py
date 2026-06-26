"""spec — spec-builder barato que INFIERE mas alla de lo dicho, pero aplica
conservador. La idea robada de spec-kit (templates) + Karpathy layer-1 (interview
para destapar el GOAL, no la tarea) + tus invariantes (cross-family, refute-default,
schema-gate).

Tension a resolver: el usuario quiere que el modelo infiera generoso PERO sin hacer
cosas que no pidio. Solucion estructural:
  1. INTERVIEW  — modelo barato genera preguntas que separan goal vs task (opcional;
     el caller las hace al usuario y devuelve respuestas).
  2. DRAFT      — modelo barato (deepseek) produce {spec, inferences[], open_questions[]}
     schema-gated. Las inferencias van en un CANAL SEPARADO, nunca mezcladas en `spec`.
  3. REFUTE     — critico CROSS-FAMILY (gemini), esceptico por default, etiqueta cada
     inferencia SAFE / BEYOND_INTENT / WRONG. Su trabajo es cazar el sobrepaso.
  4. GATE       — SAFE entra al spec; BEYOND_INTENT baja a open_questions (se le
     pregunta al usuario, NO se aplica); WRONG se descarta; verdict ausente -> se trata
     como BEYOND_INTENT (conservador). Sobrepaso grueso -> escalate a Opus.

Generosidad en la propuesta, conservadurismo en la aplicacion. Library-only.
"""
from __future__ import annotations

from dataclasses import dataclass, field

from .config import DEFAULT_GENERATOR, DEFAULT_VERIFIER, family_of
from .schema import gated_json

# --- schemas (validado-o-rechaza, §9) ---------------------------------------
_QUESTIONS_SCHEMA = {
    "type": "object", "required": ["questions"],
    "properties": {"questions": {"type": "array", "items": {"type": "string"}}},
}
_DRAFT_SCHEMA = {
    "type": "object", "required": ["spec", "inferences", "open_questions"],
    "properties": {
        "spec": {"type": "string"},
        "inferences": {"type": "array", "items": {"type": "string"}},
        "open_questions": {"type": "array", "items": {"type": "string"}},
    },
}
_CRITIQUE_SCHEMA = {
    "type": "object", "required": ["verdicts", "spec_overreach"],
    "properties": {
        "verdicts": {"type": "array", "items": {
            "type": "object", "required": ["label"],
            "properties": {
                "label": {"type": "string", "enum": ["SAFE", "BEYOND_INTENT", "WRONG"]},
                "reason": {"type": "string"},
            }}},
        # claims que el drafter metio EN el cuerpo del spec sin respaldo del request
        # (viola la separacion de canales — gap que el dogfood cross-family encontro).
        "spec_overreach": {"type": "array", "items": {"type": "string"}},
    },
}


@dataclass
class SpecResult:
    spec: str                                       # solo dicho + confirmado + SAFE
    accepted_inferences: list[str] = field(default_factory=list)  # SAFE, plegadas al spec
    open_questions: list[str] = field(default_factory=list)       # BEYOND_INTENT + opens del draft
    dropped: list[str] = field(default_factory=list)              # WRONG
    escalate: bool = False                          # sobrepaso grueso -> Opus
    quarantined: bool = False                       # spec contaminado: NO usar sin revisar
    raw_draft: str = ""                             # draft preservado cuando hay cuarentena
    verifier_model: str = ""
    cost_usd: float = 0.0


# --- pasos ------------------------------------------------------------------
def interview(raw_request: str, *, model: str = DEFAULT_GENERATOR,
              n: int = 5, phase: str = "") -> tuple[list[str], float]:
    """Genera hasta `n` preguntas que destapan el GOAL detras de la tarea (Karpathy:
    'una tarea no es el objetivo; el objetivo es la decision que la tarea maneja').
    El caller las hace al usuario. Devuelve (preguntas, costo)."""
    sys = ("Sos un entrevistador de requisitos. Dado un request, genera preguntas "
           "CORTAS que destapen el OBJETIVO real (la decision que maneja), no la tarea "
           "superficial. Priorizá ambiguedades que cambiarian el diseño. Devolve SOLO "
           'JSON {"questions": [string, ...]}, maximo ' + str(n) + " preguntas.")
    data = gated_json(model, [{"role": "system", "content": sys},
                              {"role": "user", "content": raw_request}],
                      schema=_QUESTIONS_SCHEMA, pattern="spec_interview",
                      node="interviewer", phase=phase)
    return data["questions"][:n], data.get("_cost_usd", 0.0)


def _draft(raw_request: str, answers: str, *, model: str, phase: str) -> dict:
    """Borrador con inferencias en canal SEPARADO (nunca mezcladas en `spec`)."""
    sys = (
        "Sos un redactor de specs. Produci un spec accionable del request. REGLA DURA: "
        "todo lo que NO este textual en el request/respuestas va en `inferences`, NUNCA "
        "en `spec`. `spec` = solo lo dicho y lo confirmado. `inferences` = supuestos que "
        "hiciste de mas (inferi generoso aca, es seguro: se filtran despues). "
        "`open_questions` = lo que ni infiriendo pudiste resolver. Devolve SOLO JSON "
        '{"spec": string, "inferences": [string,...], "open_questions": [string,...]}.')
    user = f"REQUEST:\n{raw_request}\n\nRESPUESTAS DEL USUARIO:\n{answers or '(ninguna)'}"
    return gated_json(model, [{"role": "system", "content": sys},
                              {"role": "user", "content": user}],
                      schema=_DRAFT_SCHEMA, pattern="spec_draft", node="drafter",
                      phase=phase)


def _critique(raw_request: str, answers: str, inferences: list[str], spec_text: str, *,
              gen_model: str, verifier_model: str, phase: str) -> dict:
    """Critico CROSS-FAMILY: (a) etiqueta cada inferencia, (b) escudriña el CUERPO del
    spec por sobrepaso que el drafter metio salteando el canal de inferencias. Refuta
    por default — la duda cae a BEYOND_INTENT (no se aplica). Misma familia = se rechaza
    (OneFlow, tarea subjetiva)."""
    if family_of(gen_model) == family_of(verifier_model):
        raise ValueError(
            f"OneFlow violation: drafter ({gen_model}, {family_of(gen_model)}) y critico "
            f"({verifier_model}, {family_of(verifier_model)}) comparten familia. El spec "
            f"es subjetivo: usa un critico cross-family (§4).")
    listing = "\n".join(f"{i}. {inf}" for i, inf in enumerate(inferences))
    sys = (
        "Sos un critico adversarial de OTRA familia que el redactor. Dos trabajos:\n"
        "A) Para cada inferencia, decidi si el request la respalda:\n"
        "   - SAFE: directamente implicada o lectura casi-cierta y de bajo riesgo del intento.\n"
        "   - BEYOND_INTENT: plausible pero agrega alcance/supuestos que el usuario NO dijo.\n"
        "   - WRONG: contradice el request o es incorrecta.\n"
        "   Ante la DUDA, elegi BEYOND_INTENT (refutar por default).\n"
        "B) Escudriña el CUERPO del spec: lista en `spec_overreach` cualquier afirmacion "
        "que el spec da por hecha y que el request/respuestas NO respaldan (el redactor "
        "pudo colar supuestos directo en la prosa, salteando las inferencias). [] si esta limpio.\n"
        "Devolve SOLO JSON {\"verdicts\":[{\"label\":..,\"reason\":..}, ...EN EL MISMO ORDEN "
        "que las inferencias], \"spec_overreach\":[string, ...]}.")
    user = (f"REQUEST:\n{raw_request}\n\nRESPUESTAS:\n{answers or '(ninguna)'}\n\n"
            f"SPEC (cuerpo a escudriñar):\n{spec_text}\n\nINFERENCIAS:\n{listing}")
    return gated_json(verifier_model, [{"role": "system", "content": sys},
                                       {"role": "user", "content": user}],
                      schema=_CRITIQUE_SCHEMA, pattern="spec_critique",
                      node="critic", phase=phase)


def build_spec(raw_request: str, *, answers: str = "",
               gen_model: str = DEFAULT_GENERATOR,
               verifier_model: str = DEFAULT_VERIFIER,
               escalate_frac: float = 0.5, phase: str = "") -> SpecResult:
    """Draft -> refute cross-family -> gate. Inferencias SAFE entran al spec; el resto
    nunca se aplica sin pasar por el usuario. Si una fraccion >= escalate_frac de las
    inferencias NO es SAFE, el draft se sobrepaso feo -> escalate a Opus."""
    d = _draft(raw_request, answers, model=gen_model, phase=phase)
    spec_text: str = d["spec"]
    inferences: list[str] = d["inferences"]
    cost = d.get("_cost_usd", 0.0)
    open_qs: list[str] = list(d["open_questions"])

    # Siempre se critica: el dogfood cross-family mostro que el gate del canal
    # `inferences` no basta — el drafter puede colar sobrepaso directo en el cuerpo
    # del `spec`, asi que el critico tambien lo escudriña (spec_overreach).
    c = _critique(raw_request, answers, inferences, spec_text,
                  gen_model=gen_model, verifier_model=verifier_model, phase=phase)
    cost += c.get("_cost_usd", 0.0)
    verdicts = c["verdicts"]
    spec_overreach: list[str] = list(c.get("spec_overreach", []))

    accepted: list[str] = []
    dropped: list[str] = []
    for i, inf in enumerate(inferences):
        # verdict ausente o etiqueta rara -> BEYOND_INTENT (conservador).
        label = verdicts[i].get("label") if i < len(verdicts) else "BEYOND_INTENT"
        if label == "SAFE":
            accepted.append(inf)
        elif label == "WRONG":
            dropped.append(inf)
        else:
            open_qs.append(inf)

    # Sobrepaso en el cuerpo del spec = el drafter violo la separacion de canales.
    # Se surfacea como pregunta y se ESCALA (Opus revisa; no se confia en la prosa).
    if spec_overreach:
        open_qs.extend(f"[spec] {s}" for s in spec_overreach)

    not_safe = len(inferences) - len(accepted)
    escalate = bool(spec_overreach) or (
        bool(inferences) and (not_safe / len(inferences)) >= escalate_frac)

    # CUARENTENA (gap del dogfood #2): si el cuerpo del spec esta contaminado, NO se
    # devuelve usable. `spec` queda vacio; el draft sucio se preserva en raw_draft para
    # que Opus lo revise. Asi un caller que ignore `escalate` no puede usar prosa sucia.
    if spec_overreach:
        return SpecResult(spec="", accepted_inferences=accepted, open_questions=open_qs,
                          dropped=dropped, escalate=True, quarantined=True,
                          raw_draft=spec_text, verifier_model=verifier_model,
                          cost_usd=round(cost, 6))

    if accepted:  # plegado VISIBLE — el spec queda honesto sobre que se infirio
        spec_text += ("\n\n## Inferencias aceptadas (verificadas cross-family)\n"
                      + "\n".join(f"- {a}" for a in accepted))

    return SpecResult(spec=spec_text, accepted_inferences=accepted,
                      open_questions=open_qs, dropped=dropped, escalate=escalate,
                      verifier_model=verifier_model, cost_usd=round(cost, 6))


def _merge_questions(goal_qs: list[str], open_qs: list[str]) -> list[str]:
    """Goal-uncovering questions (interview) ∪ spec BEYOND_INTENT opens, deduped case-insensitively,
    order preserved (goal questions first)."""
    seen, merged = set(), []
    for q in list(goal_qs) + list(open_qs):
        k = (q or "").strip().lower()
        if k and k not in seen:
            seen.add(k)
            merged.append(q.strip())
    return merged


def perfect(raw_request: str, *, n: int = 4, gen_model: str = DEFAULT_GENERATOR,
            verifier_model: str = DEFAULT_VERIFIER, phase: str = "") -> dict:
    """Built-in one-call perfectioner (cero cupo, NO human turn): uncover the GOAL questions
    (interview) AND build a cross-family-refuted spec (build_spec) in a single pass. The interview
    questions + the spec's BEYOND_INTENT opens are returned together as what a user/orchestrator
    should still decide — never auto-resolved. Honors build_spec's quarantine/escalate guards.

    This is the mmorch-native twin of the interactive /perfect skill: the skill asks the HUMAN the
    interview questions; perfect() runs it headless for automated callers (Lotus, the cooperative
    workflow pre-sharpening a task, an agent self-sharpening). Returns a dict (see keys below)."""
    goal_qs, c1 = interview(raw_request, model=gen_model, n=n, phase=phase)
    r = build_spec(raw_request, gen_model=gen_model, verifier_model=verifier_model, phase=phase)
    return {
        "spec": r.spec,
        "open_questions": _merge_questions(goal_qs, r.open_questions),
        "goal_questions": goal_qs,
        "accepted_inferences": r.accepted_inferences,
        "dropped": r.dropped,
        "escalate": r.escalate,
        "quarantined": r.quarantined,
        "raw_draft": r.raw_draft,
        "verifier_model": r.verifier_model,
        "cost_usd": round(c1 + r.cost_usd, 6),
    }


if __name__ == "__main__":   # cero-cost check of the only non-API logic (the merge)
    m = _merge_questions(["¿Cuál es el objetivo?", "¿Quién lo usa?"],
                         ["¿quién lo usa?", "¿Qué formato de salida?", ""])
    assert m == ["¿Cuál es el objetivo?", "¿Quién lo usa?", "¿Qué formato de salida?"], m
    assert _merge_questions([], []) == []
    print("spec.perfect merge OK")
