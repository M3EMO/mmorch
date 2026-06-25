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


# --------------------------------------------------------------------------- #
# 16u: ensemble multi-vista (Thousand Brains) — decorrelacion por LENTE         #
# --------------------------------------------------------------------------- #
@dataclass
class MultiViewVerdict:
    passed: bool | None          # True/False; None si las vistas discrepan (split->escalate)
    n_pass: int
    n_total: int
    escalate: bool               # vistas en desacuerdo = ambiguedad genuina -> Opus
    low_decorrelation: bool      # <2 familias o <2 lentes -> consenso DEBIL (acuerdo != confirmacion)
    per_lens: list[dict] = field(default_factory=list)  # [{lens, verifier, family, passed, confidence}]
    cost_usd: float = 0.0
    refutations: list[str] = field(default_factory=list)


def multiview_verify(
    artifact: str,
    *,
    lenses: list[dict],
    gen_model: str = DEFAULT_GENERATOR,
    verifier_models: list[str] | None = None,
    phase: str = "",
) -> MultiViewVerdict:
    """16u (Thousand Brains): decorrelacion por LENTE ademas de familia + consenso por
    consistencia mutua. `lenses` = [{"name": str, "rubric": str}, ...]: cada lente es un
    encuadre distinto del mismo artefacto (el 'sensor' de TBT — rol/angulo/sub-aspecto).
    Los verificadores ROTAN de familia entre lentes -> DOBLE eje de decorrelacion
    (familia x vista) desde pocos modelos. Cada verificador debe ser cross-family vs gen.

    Consenso = consistencia mutua (NO mayoria sobre un solo artefacto): todas las lentes
    pasan -> passed=True; todas fallan -> passed=False; split -> passed=None, escalate=True
    (las vistas discrepan = ambiguedad genuina, mandar a Opus). N CHICO, fleet barato.

    GUARDRAIL anti-consenso-correlacionado: low_decorrelation=True si se usaron <2 familias
    de verificador O <2 lentes. Ahi el acuerdo NO confirma (invariante anti-sicofancia:
    'acuerdo no es confirmacion') — el consenso entre vistas correlacionadas amplifica el
    error confiado (el 'rico-se-hace-mas-rico' de TBT sobre un sesgo compartido)."""
    if not lenses:
        raise ValueError("multiview_verify: se requiere al menos una lente")
    verifier_models = verifier_models or ["gemini-3.1-flash-lite", "glm-4.5-air"]
    gf = family_of(gen_model)
    for vm in verifier_models:
        if family_of(vm) == gf:
            raise ValueError(
                f"OneFlow: verifier {vm} comparte familia ({gf}) con gen {gen_model}.")
    per_lens, fams_used = [], set()
    verdicts = []
    for i, lens in enumerate(lenses):
        vm = verifier_models[i % len(verifier_models)]   # rota familia entre lentes
        v = adversarial_verify(artifact, rubric=lens["rubric"], gen_model=gen_model,
                               verifier_model=vm, phase=phase)
        verdicts.append(v)
        fams_used.add(family_of(vm))
        per_lens.append({"lens": lens.get("name", f"lens{i}"), "verifier": vm,
                         "family": family_of(vm), "passed": v.passed,
                         "confidence": v.confidence})
    n_pass = sum(1 for v in verdicts if v.passed)
    n_total = len(verdicts)
    if n_pass == n_total:
        passed, escalate = True, False
    elif n_pass == 0:
        passed, escalate = False, False
    else:                                   # vistas discrepan -> ambiguedad -> Opus
        passed, escalate = None, True
    low_decorr = len(fams_used) < 2 or len(lenses) < 2
    refs = [r for v in verdicts if not v.passed for r in v.refutations]
    return MultiViewVerdict(
        passed=passed, n_pass=n_pass, n_total=n_total, escalate=escalate,
        low_decorrelation=low_decorr, per_lens=per_lens,
        cost_usd=round(sum(v.cost_usd for v in verdicts), 6), refutations=refs,
    )
