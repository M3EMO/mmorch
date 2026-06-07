"""tournament — elegir EL mejor de pocos candidatos por gusto/calidad (naming,
diseno, copy). Comparacion PAIRWISE con juez cross-family (OneFlow): el juez NO
puede ser de la familia del generador. Single-elimination: gana, avanza.

Por que pairwise y no scoring absoluto: los LLM califican mejor 'A vs B' que un
score 1-10 calibrado (menos varianza, mas senal). Empate -> escalate al orquestador
(Opus) en vez de inventar un ganador (anti-sicofancia: no forzar preferencia).
"""
from __future__ import annotations

import json
from dataclasses import dataclass, field

from .config import DEFAULT_GENERATOR, DEFAULT_VERIFIER, family_of
from .providers import call

_JUDGE_SYS = (
    "Sos un juez de una familia de modelo DISTINTA a la del autor. Te doy un criterio "
    "y dos candidatos (A y B). Elegi el MEJOR segun el criterio, sin sesgo de orden ni "
    "de largo. Si son genuinamente equivalentes, decilo. Respondé SOLO con JSON minificado: "
    '{"winner": "A"|"B"|"tie", "reason": string}'
)


@dataclass
class TournamentResult:
    winner: str | None          # candidato ganador (None si empate sin resolver)
    escalate: bool              # True si hubo empate -> Opus decide
    rounds: int
    comparisons: list[dict] = field(default_factory=list)
    cost_usd: float = 0.0


def _judge(a: str, b: str, criterion: str, judge_model: str, phase: str) -> tuple[str, str, float]:
    user = (f"CRITERIO:\n{criterion}\n\nCANDIDATO A:\n{a}\n\nCANDIDATO B:\n{b}\n\n"
            "Devolve el JSON con winner y reason.")
    res = call(judge_model, [{"role": "system", "content": _JUDGE_SYS},
                             {"role": "user", "content": user}],
               pattern="tournament", node="judge", phase=phase, temperature=0.0)
    win, reason = _parse(res.text)
    return win, reason, res.cost_usd


def _parse(text: str) -> tuple[str, str]:
    s = text.strip()
    if s.startswith("```"):
        s = s.strip("`")
        s = s.removeprefix("json").strip()
    i, j = s.find("{"), s.rfind("}")
    if i != -1 and j != -1 and j > i:
        s = s[i:j + 1]
    try:
        d = json.loads(s)
        w = str(d.get("winner", "tie")).strip().upper()
        w = "A" if w == "A" else "B" if w == "B" else "tie"
        return w, str(d.get("reason", ""))
    except Exception:
        return "tie", f"unparseable: {text[:120]}"


def tournament(
    candidates: list[str],
    *,
    criterion: str,
    gen_model: str = DEFAULT_GENERATOR,
    judge_model: str = DEFAULT_VERIFIER,
    phase: str = "",
    tie_escalates: bool = True,
) -> TournamentResult:
    """Single-elimination pairwise. Juez cross-family obligatorio (OneFlow).

    tie_escalates: si un match empata, se frena y escalate=True (Opus desempata).
    Si False, avanza el de la izquierda (determinismo, menos correcto). 1 candidato
    o 0 -> sin comparaciones."""
    if family_of(gen_model) == family_of(judge_model):
        raise ValueError(
            f"OneFlow violation: generador ({gen_model}, {family_of(gen_model)}) y juez "
            f"({judge_model}, {family_of(judge_model)}) comparten familia. Elegi un juez "
            f"cross-family (§4).")
    cands = [c for c in candidates if c and c.strip()]
    if len(cands) <= 1:
        return TournamentResult(cands[0] if cands else None, False, 0, [], 0.0)

    comparisons: list[dict] = []
    total = 0.0
    rounds = 0
    current = cands
    while len(current) > 1:
        rounds += 1
        nxt: list[str] = []
        i = 0
        while i < len(current):
            if i + 1 >= len(current):
                nxt.append(current[i])  # bye (impar)
                break
            a, b = current[i], current[i + 1]
            win, reason, cost = _judge(a, b, criterion, judge_model, phase)
            total += cost
            comparisons.append({"round": rounds, "a": a[:60], "b": b[:60],
                                "winner": win, "reason": reason})
            if win == "tie":
                if tie_escalates:
                    return TournamentResult(None, True, rounds, comparisons, round(total, 6))
                nxt.append(a)
            else:
                nxt.append(a if win == "A" else b)
            i += 2
        current = nxt
    return TournamentResult(current[0], False, rounds, comparisons, round(total, 6))
