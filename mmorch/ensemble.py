"""ensemble_verify (I-3) — K escepticos cross-family + voto mayoria.

Un solo verificador puede no atrapar un fallo (justo lo que paso con las
alucinaciones de DeepSeek en el self-audit). K verificadores reducen ese riesgo.
Cada verificador DEBE ser cross-family vs el generador (OneFlow). Empate -> falla
(default-refuta, anti-sicofancia).
"""
from __future__ import annotations

from dataclasses import dataclass, field

from .config import DEFAULT_GENERATOR, family_of
from .patterns import adversarial_verify, Verdict


@dataclass
class EnsembleVerdict:
    passed: bool
    confidence: float
    n_passed: int
    n_total: int
    verdicts: list[Verdict] = field(default_factory=list)
    cost_usd: float = 0.0
    refutations: list[str] = field(default_factory=list)
    unanimous: bool = True       # #5: todos coinciden (0/K o K/K)
    escalate: bool = False       # #5: voto DIVIDIDO -> incertidumbre -> mandar a Opus
    ensemble_degraded: bool = False  # B2: verificadores de UNA sola familia -> la premisa de
                                     # decorrelacion (OneFlow) NO se cumple ENTRE verificadores


def ensemble_verify(
    artifact: str,
    *,
    rubric: str,
    gen_model: str = DEFAULT_GENERATOR,
    verifier_models: list[str] | None = None,
    min_veto: int | None = None,
    phase: str = "",
) -> EnsembleVerdict:
    """K escepticos cross-family. Por default voto mayoria (empate->falla).

    min_veto (research LLM-jury): si se setea, modo minority-veto -> el artefacto
    FALLA si >= min_veto verificadores refutan. min_veto=1 = el mas esceptico
    (un solo veto invalida). Sube true-negatives (anti-sicofancia mas fuerte).

    NOTA familias: con solo deepseek+google activos, el ensemble usa varios google
    vs gen deepseek (cross-family OK, pero diversidad ENTRE verificadores limitada).
    Activar mas familias (Kimi, etc.) decorrelaciona mejor (research: error
    correlacionado por confounders compartidos).
    """
    verifier_models = verifier_models or ["gemini-2.5-flash", "gemini-2.5-flash-lite"]
    gf = family_of(gen_model)
    for vm in verifier_models:
        if family_of(vm) == gf:
            raise ValueError(
                f"OneFlow: verifier {vm} comparte familia ({gf}) con gen {gen_model}.")
    verdicts = [adversarial_verify(artifact, rubric=rubric, gen_model=gen_model,
                                   verifier_model=vm, phase=phase)
                for vm in verifier_models]
    n_pass = sum(1 for v in verdicts if v.passed)
    n_fail = len(verdicts) - n_pass
    if min_veto is not None:
        # minority-veto: falla si suficientes refutan.
        passed = n_fail < min_veto
    else:
        # mayoria ESTRICTA; empate -> falla (skeptic default).
        passed = n_pass > len(verdicts) / 2
    conf = sum(v.confidence for v in verdicts) / max(len(verdicts), 1)
    refs = [r for v in verdicts if not v.passed for r in v.refutations]
    # #5: el margen (2-1 vs 3-0) es senal de incertidumbre que antes se tiraba. Voto
    # dividido (ni 0 ni K) = los escepticos discrepan = justo donde rinde gastar Opus.
    unanimous = (n_pass == 0 or n_pass == len(verdicts))
    # B2: honestidad — si TODOS los verificadores son de la misma familia, el ensemble no
    # decorrelaciona (la premisa OneFlow vale gen-vs-verifier, pero NO entre verificadores).
    degraded = len({family_of(vm) for vm in verifier_models}) < 2
    return EnsembleVerdict(
        passed=passed, confidence=round(conf, 3), n_passed=n_pass,
        n_total=len(verdicts), verdicts=verdicts,
        cost_usd=round(sum(v.cost_usd for v in verdicts), 6), refutations=refs,
        unanimous=unanimous, escalate=not unanimous, ensemble_degraded=degraded,
    )
