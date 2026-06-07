"""feedback — el lazo que faltaba (la 'loss' ausente). mmorch genera/verifica/
recuerda pero no sabia si ACERTO. Aca: (1) loggear outcomes reales por decision,
(2) un bandit Thompson (gradient-free, stdlib) que elige modelo/umbral y aprende
de los outcomes, (3) calibracion (ECE) conf-predicha vs realidad.

NO entrena redes ni gradientes: es estadistica bayesiana (Beta posterior) sobre
conteos. Cuando haya volumen+labels, esto genera los datos para un router NN.

Research: feedback-based self-learning (Ponnusamy 2019, Liu 2024); contextual
bandit / Thompson (Amin 2026 Bayesian orchestration). Geirhos 2020: medir
calibracion evita aprender shortcuts sobre un proxy debil.
"""
from __future__ import annotations

import json
import random as _random
import time
from dataclasses import dataclass, asdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
_FEEDBACK_LOG = ROOT / "logs" / "feedback.jsonl"
_BANDIT_STATE = ROOT / "logs" / "bandit_state.json"


@dataclass
class Outcome:
    ts: float
    arm: str            # la decision tomada (ej "deepseek-chat" o "cascade:step0")
    reward: float       # [0,1]: 1=acerto, 0=fallo, fraccion=parcial
    pattern: str = ""   # fan_out|cascade|verify|recall|...
    predicted_conf: float | None = None  # lo que el sistema creia (para calibracion)
    source: str = ""    # test|opus|human|downstream (de donde sale el label)
    context: str = ""   # scope/task opcional


def record_outcome(arm: str, reward: float, *, pattern: str = "",
                   predicted_conf: float | None = None, source: str = "",
                   context: str = "", path: Path = _FEEDBACK_LOG) -> Outcome:
    """Registra un outcome etiquetado (append-only). reward se clampa a [0,1]."""
    o = Outcome(ts=time.time(), arm=arm, reward=max(0.0, min(1.0, float(reward))),
                pattern=pattern, predicted_conf=predicted_conf, source=source, context=context)
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "a", encoding="utf-8") as fh:
        fh.write(json.dumps(asdict(o), ensure_ascii=False) + "\n")
    return o


def read_outcomes(path: Path = _FEEDBACK_LOG) -> list[dict]:
    if not path.exists():
        return []
    return [json.loads(ln) for ln in path.read_text(encoding="utf-8").splitlines() if ln.strip()]


class ThompsonBandit:
    """Bandit Bernoulli/Beta. Cada brazo = decision (modelo/umbral). Sin gradientes:
    posterior Beta(alpha, beta); select muestrea cada brazo y elige el max; update
    suma reward a alpha y (1-reward) a beta. Anda desde la PRIMERA muestra."""

    def __init__(self, path: Path = _BANDIT_STATE):
        self.path = path
        self._arms: dict[str, list[float]] = {}
        if path.exists():
            try:
                self._arms = {k: list(v) for k, v in json.loads(path.read_text(encoding="utf-8")).items()}
            except Exception:
                self._arms = {}

    def _ab(self, arm: str) -> list[float]:
        return self._arms.setdefault(arm, [1.0, 1.0])  # prior uniforme Beta(1,1)

    def select(self, arms: list[str], rng: _random.Random | None = None) -> str:
        rng = rng or _random.Random()
        best, best_theta = arms[0], -1.0
        for a in arms:
            alpha, beta = self._ab(a)
            theta = rng.betavariate(alpha, beta)
            if theta > best_theta:
                best, best_theta = a, theta
        return best

    def update(self, arm: str, reward: float) -> None:
        reward = max(0.0, min(1.0, float(reward)))
        ab = self._ab(arm)
        ab[0] += reward
        ab[1] += 1.0 - reward
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self.path.write_text(json.dumps(self._arms, ensure_ascii=False), encoding="utf-8")

    def stats(self) -> dict[str, dict]:
        out = {}
        for a, (alpha, beta) in self._arms.items():
            n = alpha + beta - 2.0  # restar el prior
            out[a] = {"mean": round(alpha / (alpha + beta), 4), "n": int(n)}
        return out


def calibration(path: Path = _FEEDBACK_LOG, bins: int = 10) -> dict:
    """ECE (Expected Calibration Error) + accuracy por modelo, sobre outcomes que
    tengan predicted_conf. ECE = sum_bin (n_bin/N) * |conf_prom - acc_bin|.
    Alto ECE = el sistema esta mal calibrado (overconfident/underconfident)."""
    ev = [e for e in read_outcomes(path) if e.get("predicted_conf") is not None]
    if not ev:
        return {"ece": None, "n": 0, "by_arm": {}}
    buckets: dict[int, list[tuple[float, float]]] = {}
    for e in ev:
        c = max(0.0, min(1.0, float(e["predicted_conf"])))
        b = min(bins - 1, int(c * bins))
        buckets.setdefault(b, []).append((c, float(e["reward"])))
    n = len(ev)
    ece = 0.0
    for b, pairs in buckets.items():
        conf_avg = sum(c for c, _ in pairs) / len(pairs)
        acc = sum(r for _, r in pairs) / len(pairs)
        ece += (len(pairs) / n) * abs(conf_avg - acc)
    by_arm: dict[str, dict] = {}
    for e in ev:
        a = e.get("arm", "?")
        d = by_arm.setdefault(a, {"n": 0, "rew": 0.0})
        d["n"] += 1
        d["rew"] += float(e["reward"])
    for a, d in by_arm.items():
        d["accuracy"] = round(d.pop("rew") / d["n"], 4)
    return {"ece": round(ece, 4), "n": n, "by_arm": by_arm}
