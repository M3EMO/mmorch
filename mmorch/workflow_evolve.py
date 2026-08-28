"""workflow_evolve — la poblacion de variantes del engine EVOLUCIONA (backlog #1
repo-mining 2026-07, patron evolution-sim del video ALife: un GA con fitness frozen
se estanca — medido en mmorch: 3 variantes hardcodeadas, plateau 0.778 —; una
simulacion de evolucion reemplaza el fitness unico por REPRODUCCION (el ganador
de la noche spawnea un hijo mutado/cruzado) y MUERTE (perdedor cronico segun el
workflow-bandit sale del pool). El fitness queda EMERGENTE de la distribucion de
bench tasks — nunca un escalar frozen que se pueda goodhartear.

Guardrails (los "invariantes anti-deriva" del diseno):
  - poblacion acotada [MIN_POP, MAX_POP]: ni extincion ni explosion de costo.
  - los 3 seeds originales (pb-quick/base/deep) pueden morir — pero MIN_POP jamas
    se perfora (fail-open a los seeds si la poblacion quedara vacia).
  - knobs mutables SOLO numericos y acotados (_KNOBS): un hijo nunca inventa
    capacidades nuevas, solo re-dosifica las existentes.
  - RNG seedeado por dia: la evolucion de una noche es REPRODUCIBLE (invariante
    del repo: nada de Date.now/random libre en paths auditables).
  - la muerte exige evidencia (n >= min_n) — sin datos no se mata (misma
    disciplina que intuition.decide).
"""
from __future__ import annotations

import json
import random
import time
from pathlib import Path

from .feedback import ThompsonBandit
from .workflow_race import _WF_BANDIT, VARIANTS

from .paths import logs_dir

_POP_PATH = logs_dir() / "workflow_population.json"

MIN_POP, MAX_POP = 3, 6
#          knob            (min, max, paso de mutacion)
_KNOBS = {"max_fix":       (1, 6, 1),
          "max_depth":     (1, 3, 1),
          "max_gen_calls": (40, 300, 40)}


def load_population(path: Path | None = None) -> dict[str, dict]:
    p = path or _POP_PATH
    try:
        pop = json.loads(p.read_text(encoding="utf-8"))
        if pop:
            return pop
    except (json.JSONDecodeError, OSError):
        pass
    return {k: dict(v) for k, v in VARIANTS.items()}   # seed inicial


def _save(pop: dict, path: Path | None = None) -> None:
    p = path or _POP_PATH
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(json.dumps(pop, ensure_ascii=False, indent=2), encoding="utf-8")


def _mutate(cfg: dict, rng: random.Random) -> dict:
    child = dict(cfg)
    knob = rng.choice(sorted(_KNOBS))
    lo, hi, step = _KNOBS[knob]
    cur = int(child.get(knob, lo))
    child[knob] = max(lo, min(hi, cur + rng.choice([-step, step])))
    return child

def _crossover(a: dict, b: dict, rng: random.Random) -> dict:
    return {k: (a if rng.random() < 0.5 else b).get(k, _KNOBS[k][0]) for k in _KNOBS}


def _pooled(bandit: ThompsonBandit, name: str) -> tuple[float, int]:
    """(media, n) del variant agregada sobre TODAS las firmas (arm = 'name#sig')."""
    ms, ns = [], 0
    for arm, s in bandit.stats().items():
        if arm.split("#", 1)[0] == name:
            ms.append(s["mean"] * s["n"])
            ns += s["n"]
    return ((sum(ms) / ns) if ns else 0.5, int(ns))


def evolve_population(winner: str | None, *, path: Path | None = None,
                      bandit: ThompsonBandit | None = None,
                      min_n: int = 6, death_mean: float = 0.25,
                      rng: random.Random | None = None) -> dict:
    """Un paso de evolucion post-race (llamado por el nightly con el ganador de la noche).
    REPRODUCE: el ganador cruza con otro variant al azar y el hijo muta un knob.
    MATA: perdedor cronico (n >= min_n y media pooled < death_mean) — con evidencia, nunca
    por una mala noche. CAP: si la poblacion excede MAX_POP, muere el peor con evidencia.
    Devuelve {population, born, died}. Sin ganador esta noche -> solo muerte/cap."""
    rng = rng or random.Random(int(time.time() // 86400))   # reproducible por dia
    pop = load_population(path)
    b = bandit or ThompsonBandit(path=_WF_BANDIT)
    born, died = [], []

    if winner and winner in pop:
        others = [n for n in pop if n != winner]
        mate = rng.choice(sorted(others)) if others else winner
        child_cfg = _mutate(_crossover(pop[winner], pop[mate], rng), rng)
        if child_cfg not in pop.values():                   # clon exacto no aporta diversidad
            gen = sum(1 for n in pop if n.startswith(winner + "-g")) + 1
            cname = f"{winner}-g{gen}"
            pop[cname] = child_cfg
            born.append(cname)

    # muerte con evidencia (proteger MIN_POP)
    for name in sorted(pop):
        if len(pop) <= MIN_POP:
            break
        mean, n = _pooled(b, name)
        if n >= min_n and mean < death_mean:
            del pop[name]
            died.append(name)

    # cap de poblacion: sobreviven los mejores (sin evidencia = 0.5, ni bueno ni malo)
    while len(pop) > MAX_POP:
        worst = min(sorted(pop), key=lambda n: _pooled(b, n)[0])
        del pop[worst]
        died.append(worst)

    if not pop:                                             # jamas extincion (fail-open)
        pop = {k: dict(v) for k, v in VARIANTS.items()}
    _save(pop, path)
    return {"population": pop, "born": born, "died": died}


if __name__ == "__main__":
    # self-check cero-API, todo inyectado (pop en temp, bandit en temp, rng fijo)
    import tempfile
    pp = Path(tempfile.mkdtemp()) / "pop.json"
    bt = ThompsonBandit(Path(tempfile.mkdtemp()) / "wfb.json")
    rng = random.Random(42)

    # 1. seed: sin archivo -> VARIANTS
    assert set(load_population(pp)) == set(VARIANTS)
    # 2. ganador se reproduce: nace un hijo mutado/cruzado con knobs acotados
    r1 = evolve_population("pb-base", path=pp, bandit=bt, rng=rng)
    assert r1["born"] and r1["born"][0].startswith("pb-base-g"), r1
    child = r1["population"][r1["born"][0]]
    for k, (lo, hi, _s) in _KNOBS.items():
        assert lo <= child[k] <= hi, (k, child)
    # 3. muerte exige evidencia: variant con n<min_n NO muere aunque pierda
    bt.update("pb-quick#SIG", 0.0)
    r2 = evolve_population(None, path=pp, bandit=bt, rng=rng)
    assert "pb-quick" in r2["population"], r2
    # 4. perdedor cronico muere (n>=6, media<0.25), respetando MIN_POP
    for _ in range(10):
        bt.update("pb-quick#SIG", 0.0)
    r3 = evolve_population(None, path=pp, bandit=bt, rng=rng)
    assert "pb-quick" in r3["died"] and len(r3["population"]) >= MIN_POP, r3
    # 5. cap MAX_POP: reproducir muchas noches no explota la poblacion
    for day in range(10):
        evolve_population("pb-deep", path=pp, bandit=bt, rng=random.Random(day))
    assert len(load_population(pp)) <= MAX_POP
    # 6. reproducibilidad: mismo rng-seed, mismo estado -> mismo hijo
    pp2 = Path(tempfile.mkdtemp()) / "pop2.json"
    a = evolve_population("pb-base", path=pp2, bandit=bt, rng=random.Random(7))
    pp3 = Path(tempfile.mkdtemp()) / "pop3.json"
    c = evolve_population("pb-base", path=pp3, bandit=bt, rng=random.Random(7))
    assert a["born"] == c["born"]
    print("workflow_evolve OK — reproduce/muta/cruza, muerte con evidencia, "
          "MIN/MAX pop, reproducible por dia")
