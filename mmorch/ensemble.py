"""ensemble_verify (I-3) — K escepticos cross-family + voto mayoria.

Un solo verificador puede no atrapar un fallo (justo lo que paso con las
alucinaciones de DeepSeek en el self-audit). K verificadores reducen ese riesgo.
Cada verificador DEBE ser cross-family vs el generador (OneFlow). Empate -> falla
(default-refuta, anti-sicofancia).
"""
from __future__ import annotations

from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, field

from .config import DEFAULT_GENERATOR, DEFAULT_VERIFIER, family_of
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
    verifier_models = verifier_models or [DEFAULT_VERIFIER, "gemini-2.5-flash-lite"]
    gf = family_of(gen_model)
    for vm in verifier_models:
        if family_of(vm) == gf:
            raise ValueError(
                f"OneFlow: verifier {vm} comparte familia ({gf}) con gen {gen_model}.")
    # Calls independientes -> paralelas; map preserva el orden de verdicts por indice.
    with ThreadPoolExecutor(max_workers=len(verifier_models)) as ex:
        verdicts = list(ex.map(
            lambda vm: adversarial_verify(artifact, rubric=rubric, gen_model=gen_model,
                                          verifier_model=vm, phase=phase),
            verifier_models))
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


# ---------------------------------------------------------------------------
# pair_verify (graft Estudio backend/grade, leido 2026-08-07): el grading service
# de Estudio productizo 3 mecanicas que ensemble_verify no tenia:
#   1. FAST-PATH de costo: si el 1er juez da veredicto EXTREMO (conf <=0.2 o >=0.8)
#      y el artefacto es corto (<50 palabras), el 2do juez no aporta — 1 llamada.
#   2. SNAP a niveles discretos anclados (0/.2/.4/.6/.8/1) FLAGGEANDO el ajuste
#      (nunca aceptar silencioso un valor fuera de escala).
#   3. DESACUERDO -> tag explicito + notas POR JUEZ preservadas; el promedio se
#      muestra como estimacion, JAMAS como veredicto final silencioso (escala).
# ---------------------------------------------------------------------------
DISCRETE_LEVELS = (0.0, 0.2, 0.4, 0.6, 0.8, 1.0)
_EXTREME_LOW, _EXTREME_HIGH = 0.2, 0.8
_SHORT_WORDS = 50


def snap_discrete(score: float) -> tuple[float, bool]:
    """(nivel_anclado_mas_cercano, hubo_ajuste). El flag viaja al veredicto."""
    lvl = min(DISCRETE_LEVELS, key=lambda x: abs(x - score))
    return lvl, lvl != score


@dataclass
class PairVerdict:
    passed: bool | None          # None = desacuerdo -> escalar (nunca promediar en silencio)
    confidence: float            # snapeada; en desacuerdo = promedio MOSTRADO como estimacion
    fast_path: bool              # True = 1 solo juez (extremo + corto)
    disagreement: bool
    flags: list[str] = field(default_factory=list)
    judge_notes: list[dict] = field(default_factory=list)  # [{judge, passed, confidence, refutations}]
    cost_usd: float = 0.0


def pair_verify(artifact: str, *, rubric: str, gen_model: str = DEFAULT_GENERATOR,
                verifier_models: list[str] | None = None,
                fast_path: bool = True, verify_fn=None) -> PairVerdict:
    """2 jueces cross-family con fast-path de costo y desacuerdo explicito.
    `verify_fn(artifact, rubric, gen_model, verifier_model)->Verdict` inyectable
    (self-check cero-costo); default = adversarial_verify."""
    verifier_models = verifier_models or ["gemini-3.1-flash-lite", "gemini-2.5-flash"]
    gf = family_of(gen_model)
    for vm in verifier_models:
        if family_of(vm) == gf:
            raise ValueError(f"OneFlow: verifier {vm} comparte familia ({gf}) con gen.")
    if verify_fn is None:
        def verify_fn(a, r, g, v):
            return adversarial_verify(a, rubric=r, gen_model=g, verifier_model=v)

    flags: list[str] = []

    def _note(v) -> dict:
        return {"judge": v.verifier_model, "passed": v.passed,
                "confidence": v.confidence, "refutations": list(v.refutations)}

    v1 = verify_fn(artifact, rubric, gen_model, verifier_models[0])
    c1, adj = snap_discrete(v1.confidence)
    if adj:
        flags.append(f"score_ajustado_de_{v1.confidence}_a_nivel_discreto")
    extreme = c1 <= _EXTREME_LOW or c1 >= _EXTREME_HIGH
    if fast_path and extreme and len(artifact.split()) < _SHORT_WORDS:
        return PairVerdict(passed=v1.passed, confidence=c1, fast_path=True,
                           disagreement=False, flags=flags, judge_notes=[_note(v1)],
                           cost_usd=v1.cost_usd)

    v2 = verify_fn(artifact, rubric, gen_model, verifier_models[1])
    c2, adj2 = snap_discrete(v2.confidence)
    if adj2:
        flags.append(f"score_ajustado_de_{v2.confidence}_a_nivel_discreto")
    notes = [_note(v1), _note(v2)]
    cost = v1.cost_usd + v2.cost_usd
    if v1.passed == v2.passed:
        return PairVerdict(passed=v1.passed, confidence=round((c1 + c2) / 2, 2),
                           fast_path=False, disagreement=False, flags=flags,
                           judge_notes=notes, cost_usd=cost)
    # desacuerdo: con 2 jueces no hay mayoria — se escala (passed=None); el
    # promedio queda como estimacion visible, con las notas de cada juez.
    flags.append("desacuerdo_cross_family")
    return PairVerdict(passed=None, confidence=round((c1 + c2) / 2, 2),
                       fast_path=False, disagreement=True, flags=flags,
                       judge_notes=notes, cost_usd=cost)


# ---------------------------------------------------------------------------
# memory_consistency (patron PULSE, npj Digit Med 2026, leido 2026-07): el MISMO
# modelo responde SIN memoria (solo parametrico) y CON las notas recuperadas como
# contexto. El diff entre ambas es senal DETERMINISTA (Jaccard, cero LLM-judge):
#   divergencia alta -> la memoria fue load-bearing (cambio la respuesta de verdad)
#     -> confiar en la version con-memoria pero flaggear para verify si es critico.
#   divergencia ~0  -> retrieval decorativo (el modelo ya lo sabia) -> las notas
#     no aportaron; senal para curar/mergear esas notas.
# Distinto de ensemble_verify (2 familias juzgan UN artefacto): aca es UNA familia,
# DOS condiciones de contexto -> mide el efecto causal de la memoria, no la calidad.
# ---------------------------------------------------------------------------
@dataclass
class ConsistencyResult:
    with_memory: str
    without_memory: str
    divergence: float          # 1 - jaccard(tokens) en [0,1]; alto = memoria load-bearing
    notes_used: int
    cost_usd: float = 0.0


def memory_consistency(question: str, *, scope: str = "global", k: int = 5,
                       model: str = DEFAULT_GENERATOR,
                       call_fn=None, recall_fn=None) -> ConsistencyResult:
    """Genera la respuesta a `question` con y sin las notas de recall() como contexto
    y mide la divergencia. `call_fn(model, messages)->text` y `recall_fn(q, scope, k)
    ->notes` inyectables (self-check cero-costo)."""
    if recall_fn is None:
        from .memory import recall as _recall
        recall_fn = lambda q, s, kk: _recall(q, s, k=kk, track=False)  # noqa: E731
    if call_fn is None:
        from .providers import call as _call

        def call_fn(m, msgs):
            return _call(m, msgs, pattern="consistency", node="memcheck",
                         temperature=0.0).text

    notes = recall_fn(question, scope, k)
    ctx = "\n".join(f"- {n.text}" for n in notes)
    bare = call_fn(model, [{"role": "user", "content": question}])
    grounded = call_fn(model, [
        {"role": "system", "content": "Usá estas notas de memoria si son relevantes:\n" + ctx},
        {"role": "user", "content": question}]) if notes else bare

    def toks(t: str) -> set:
        return {w for w in "".join(c if c.isalnum() or c == "_" else " "
                                   for c in t.lower()).split() if w}
    a, b = toks(bare), toks(grounded)
    inter = len(a & b)
    union = len(a | b) or 1
    return ConsistencyResult(with_memory=grounded, without_memory=bare,
                             divergence=round(1.0 - inter / union, 4),
                             notes_used=len(notes))


if __name__ == "__main__":
    # self-check cero-costo de memory_consistency (call/recall inyectados)
    class _FakeNote:
        def __init__(self, t):
            self.text = t

    # 1. memoria load-bearing: la respuesta cambia con las notas -> divergencia alta
    def _call_divergent(m, msgs):
        return ("el timeout es 1800 segundos (medido)" if len(msgs) > 1
                else "no tengo ese dato en mi entrenamiento")
    r = memory_consistency("cual es el timeout?", call_fn=_call_divergent,
                           recall_fn=lambda q, s, k: [_FakeNote("timeout=1800s")])
    assert r.divergence > 0.5 and r.notes_used == 1, r

    # 2. retrieval decorativo: misma respuesta con y sin -> divergencia 0
    r2 = memory_consistency("2+2?", call_fn=lambda m, msgs: "4",
                            recall_fn=lambda q, s, k: [_FakeNote("aritmetica basica")])
    assert r2.divergence == 0.0, r2

    # 3. sin notas: no gasta la 2da llamada (grounded == bare por construccion)
    calls = {"n": 0}

    def _counting(m, msgs):
        calls["n"] += 1
        return "x"
    r3 = memory_consistency("q", call_fn=_counting, recall_fn=lambda q, s, k: [])
    assert r3.notes_used == 0 and calls["n"] == 1 and r3.divergence == 0.0

    # 4. pair_verify: fast-path, snap flaggeado, desacuerdo -> escalate
    from .patterns import Verdict as _V

    def _mk(passed, conf, model):
        return _V(passed=passed, confidence=conf, refutations=[], raw="",
                  verifier_model=model, cost_usd=0.01)
    n_calls = {"n": 0}

    def _vf(conf2=0.6, pass2=True):
        def f(a, r, g, v):
            n_calls["n"] += 1
            return _mk(True, 0.95, v) if n_calls["n"] == 1 else _mk(pass2, conf2, v)
        return f
    # extremo (0.95 snapea a 1.0 con flag) + corto -> 1 solo juez
    pv = pair_verify("corto", rubric="r", verify_fn=_vf())
    assert pv.fast_path and n_calls["n"] == 1 and pv.confidence == 1.0
    assert any("ajustado" in fl for fl in pv.flags), pv.flags
    # largo -> 2 jueces; acuerdo -> passed definido
    n_calls["n"] = 0
    pv2 = pair_verify("palabra " * 60, rubric="r", verify_fn=_vf(conf2=0.8))
    assert not pv2.fast_path and n_calls["n"] == 2 and pv2.passed is True
    # desacuerdo -> passed=None (escalar), tag, notas por juez preservadas
    n_calls["n"] = 0
    pv3 = pair_verify("palabra " * 60, rubric="r", verify_fn=_vf(pass2=False, conf2=0.2))
    assert pv3.passed is None and pv3.disagreement
    assert "desacuerdo_cross_family" in pv3.flags and len(pv3.judge_notes) == 2
    print("ensemble self-check OK (memory_consistency + pair_verify)")
