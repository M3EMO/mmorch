"""ablation (§18.4) — validar EMPIRICAMENTE la regla de pairing cross-family. El
diseno afirma: un verificador de OTRA familia que el autor decorrelaciona errores y
caza fallas mejor que uno de la MISMA familia (que comparte los puntos ciegos). Eso
es una hipotesis, no un dogma: se mide.

Setup: un mini-benchmark de casos ETIQUETADOS (artifact + truth_passed: si el
artifact es correcto o tiene un error plantado). Se corre el MISMO verificador-
esceptico con distintos modelos verificadores y se compara su accuracy (acerto el
veredicto vs la verdad), costo y latencia. Cross-family deberia ganar en deteccion.

NOTA anti-proxy: sin labels de verdad esto no vale (mediria contra un proxy debil, la
critica cross-family que ya cazamos). Por eso exige truth_passed por caso.
"""
from __future__ import annotations

from dataclasses import dataclass, field

from .config import family_of, DEFAULT_GENERATOR
from .providers import call
from .patterns import _SKEPTIC_SYSTEM, _parse_verdict


@dataclass
class AblationCase:
    artifact: str
    rubric: str
    truth_passed: bool       # verdad de campo: el artifact ES correcto?
    label: str = ""


@dataclass
class ConfigResult:
    verifier_model: str
    family: str
    cross_family: bool
    n: int
    correct: int
    accuracy: float
    false_pass: int          # dijo passed pero era incorrecto (peor error: deja pasar bug)
    false_refute: int        # refuto pero era correcto
    cost_usd: float
    lat_avg: float
    by_case: list[dict] = field(default_factory=list)


def _verify(verifier_model: str, artifact: str, rubric: str, phase: str):
    """Verify esceptico crudo (sin guard OneFlow: la ablacion DEBE poder correr
    same-family para comparar). Reusa el system anti-sicofancia + parser."""
    user = (f"RUBRIC:\n{rubric}\n\nARTIFACT TO REFUTE:\n{artifact}\n\n"
            "Return the JSON verdict.")
    res = call(verifier_model, [{"role": "system", "content": _SKEPTIC_SYSTEM},
                                {"role": "user", "content": user}],
               pattern="ablation", node=f"verifier:{verifier_model}", phase=phase,
               temperature=0.0)
    passed, conf, refs = _parse_verdict(res.text)
    return passed, res.cost_usd, res.latency_s


def run_ablation(
    cases: list[AblationCase],
    verifier_models: list[str],
    *,
    author_model: str = DEFAULT_GENERATOR,
    phase: str = "ablation",
) -> dict:
    """Corre cada verifier sobre todos los casos. Devuelve un ConfigResult por verifier.
    author_model define que es 'misma familia' (el supuesto autor de los artifacts)."""
    author_fam = family_of(author_model)
    configs: list[ConfigResult] = []
    for vm in verifier_models:
        correct = fp = fr = 0
        cost = 0.0
        lats: list[float] = []
        by_case: list[dict] = []
        for c in cases:
            passed, cc, lat = _verify(vm, c.artifact, c.rubric, phase)
            cost += cc
            lats.append(lat)
            ok = (passed == c.truth_passed)
            correct += int(ok)
            if passed and not c.truth_passed:
                fp += 1
            if (not passed) and c.truth_passed:
                fr += 1
            by_case.append({"label": c.label or c.artifact[:30], "truth": c.truth_passed,
                            "verdict_passed": passed, "ok": ok})
        n = len(cases)
        configs.append(ConfigResult(
            verifier_model=vm, family=family_of(vm),
            cross_family=(family_of(vm) != author_fam),
            n=n, correct=correct, accuracy=round(correct / n, 4) if n else 0.0,
            false_pass=fp, false_refute=fr,
            cost_usd=round(cost, 6), lat_avg=round(sum(lats) / n, 2) if n else 0.0,
            by_case=by_case))
    # ranking por accuracy desc, luego costo asc
    configs.sort(key=lambda r: (-r.accuracy, r.cost_usd))
    return {"author_model": author_model, "author_family": author_fam,
            "configs": configs}
