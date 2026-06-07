"""innovate (I-5) — motor de innovacion productizado. mmorch se idea capacidades
nuevas (fan_out multi-lente) y las filtra adversarialmente (cross-family). Loop
ideate -> screen -> rank, reutilizable. NO auto-aplica: produce propuestas.
"""
from __future__ import annotations

from dataclasses import dataclass

from .config import DEFAULT_GENERATOR
from .patterns import fan_out, adversarial_verify, Verdict


def ideate(context: str, lenses: list[str], ask: str, *,
           gen_model: str = DEFAULT_GENERATOR, phase: str = "innovate",
           temperature: float = 0.9) -> list[str]:
    """Divergir: una generacion por lente. Devuelve los textos crudos de ideas.

    temperature ALTA por default (0.9): idear necesita diversidad. (fan_out global
    usa 0.3, apto para tareas deterministas; verify usa 0.0. Ideacion va CALIENTE.)
    """
    prompts = [f"{lens}\n\n{context}\n\n{ask}" for lens in lenses]
    return [r.text for r in fan_out(prompts, gen_model=gen_model, phase=phase,
                                    temperature=temperature)]


def screen(idea: str, *, rubric: str, gen_model: str = DEFAULT_GENERATOR,
           phase: str = "innovate") -> Verdict:
    """Filtro adversarial cross-family de UNA idea. Refuta si no es util/factible."""
    return adversarial_verify(idea, rubric=rubric, gen_model=gen_model, phase=phase)


@dataclass
class ScreenedIdea:
    idea: str
    survives: bool
    confidence: float
    objection: str


def ideate_and_screen(context: str, lenses: list[str], ask: str, rubric: str,
                      *, gen_model: str = DEFAULT_GENERATOR) -> list[ScreenedIdea]:
    """Loop completo: idear por lente + screen cada una. survives = no refutada."""
    ideas = ideate(context, lenses, ask, gen_model=gen_model)
    out = []
    for idea in ideas:
        v = screen(idea, rubric=rubric, gen_model=gen_model)
        out.append(ScreenedIdea(
            idea=idea, survives=v.passed, confidence=v.confidence,
            objection=(v.refutations[0] if v.refutations else "")))
    return out
