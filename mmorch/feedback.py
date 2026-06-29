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
    # Phase 5 forward-wire: also LEARN into the signature-keyed bandit when a task
    # context is present (previously outcomes were logged but never trained the bandit).
    # Lazy import: feedback must not hard-depend on intuition (cycle). try/except: a
    # learning failure must NEVER break the append — logging is the contract.
    # ponytail: extra ~regex+small-json-write per outcome; fine at orchestration volume.
    if context:
        try:
            from .intuition import record as _sig_record
            _sig_record(arm, o.reward, context)
        except Exception:
            pass
    return o


def contextual_arm(model: str, thr: float | None = None, ctx: str | None = None) -> str:
    """#4: brazo CONTEXTUAL para ThompsonBandit. El umbral bueno para aritmetica != el
    bueno para sintesis -> bucketear el brazo por clase de tarea (de classify.py) da un
    posterior Beta POR clase. Cero ML nuevo: mismo Thompson, key compuesta
    'model@thr#ctx'. El bandit ya soporta cualquier string como brazo; esto solo
    normaliza el naming para que select()/update() aprendan por contexto."""
    arm = model
    if thr is not None:
        arm += f"@{thr}"
    if ctx:
        arm += f"#{ctx}"
    return arm


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


def reliability_bins(path: Path = _FEEDBACK_LOG, bins: int = 10,
                     pattern: str | None = None) -> dict[int, dict]:
    """Curva de fiabilidad: por bucket de predicted_conf -> accuracy empirica real
    (reward promedio) + n. Es el mapa raw_conf -> P(correcto) observado.

    `pattern` filtra por tarea: la calibracion es ESPECIFICA de la tarea (la curva de
    un verificador no aplica a un generador). Sin filtro mezcla todo (solo si sabes que
    es comparable)."""
    ev = [e for e in read_outcomes(path) if e.get("predicted_conf") is not None
          and (pattern is None or e.get("pattern") == pattern)]
    buckets: dict[int, list[float]] = {}
    for e in ev:
        c = max(0.0, min(1.0, float(e["predicted_conf"])))
        b = min(bins - 1, int(c * bins))
        buckets.setdefault(b, []).append(float(e["reward"]))
    return {b: {"acc": sum(v) / len(v), "n": len(v)} for b, v in buckets.items()}


def calibrate_conf(raw: float, *, pattern: str | None = None, path: Path = _FEEDBACK_LOG,
                   bins: int = 10, min_n: int = 20) -> float:
    """Mapea una conf auto-reportada (que MIENTE, ver ECE) a la P(correcto) empirica
    de su bucket, usando SOLO outcomes de la misma `pattern` (tarea). Gateás sobre ESTO,
    no sobre la conf cruda. Fallback a raw si el bucket no tiene >=min_n muestras de esa
    tarea -> sin data comparable NO se inventa correccion (evita contaminacion cross-task)."""
    raw = max(0.0, min(1.0, float(raw)))
    m = reliability_bins(path, bins, pattern=pattern)
    b = min(bins - 1, int(raw * bins))
    cell = m.get(b)
    if cell and cell["n"] >= min_n:
        return cell["acc"]
    return raw


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
    for _, pairs in buckets.items():
        conf_avg = sum(c for c, _ in pairs) / len(pairs)
        acc = sum(r for _, r in pairs) / len(pairs)
        ece += (len(pairs) / n) * abs(conf_avg - acc)
    by_arm: dict[str, dict] = {}
    for e in ev:
        a = e.get("arm", "?")
        d = by_arm.setdefault(a, {"n": 0, "rew": 0.0})
        d["n"] += 1
        d["rew"] += float(e["reward"])
    for d in by_arm.values():
        d["accuracy"] = round(d.pop("rew") / d["n"], 4)
    return {"ece": round(ece, 4), "n": n, "by_arm": by_arm}
