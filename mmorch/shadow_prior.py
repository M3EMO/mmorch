"""shadow_prior — Fase 5: una capa que PRIMEA al ThompsonBandit con un prior contextual,
sin riesgo (shadow + scale gated). NO reemplaza al bandit; le suma pseudo-conteos Beta
derivados de k-NN sobre outcomes pasados (embedding del contexto), escalados por `scale`.

Invariantes (spec Q5):
- scale=0 -> prior nulo -> decisiones IDENTICAS al bandit puro (verificable bit a bit).
- auto_scale se mueve en pasos de ±0.1 dentro de [0.1, 0.8] solo si mejora offline >2%;
  scale=0 = apagado. Superar el TOPE 0.8 NO es automatico (devuelve flag pa gate humano).
- exploracion: el bandit ya explora por muestreo Beta; el prior solo sesga, no fija.

Fuente de datos: `feedback.read_outcomes()` -> {arm, reward, context}. Embedding via
`memory.embed` (bge-small local, cero API). k-NN coseno por brazo.
"""
from __future__ import annotations

import math
import random as _random
from dataclasses import dataclass, field

from .feedback import read_outcomes, ThompsonBandit
from .memory import embed

SCALE_MIN, SCALE_MAX = 0.1, 0.8     # rango auto; 0 = apagado; >MAX requiere gate humano
_K = 8                              # vecinos k-NN
_MIN_NEIGHBORS = 3                  # bajo esto, el prior se abstiene (cae a bandit puro)


def _cos(a: list[float], b: list[float]) -> float:
    s = sum(x * y for x, y in zip(a, b))
    na = math.sqrt(sum(x * x for x in a)) or 1.0
    nb = math.sqrt(sum(y * y for y in b)) or 1.0
    return s / (na * nb)


@dataclass
class ShadowPrior:
    """Indexa (arm -> [(emb, reward)]) desde outcomes. prior_for da pseudo-conteos Beta.
    embed_fn es PLUGGABLE: bge-small (default, texto) o code_embedder (codigo, mejor rep).
    Cambiar la representacion es la palanca pa que el prior sea util (ver offline_improvement)."""
    scale: float = 0.0
    index: dict[str, list[tuple[list[float], float]]] = field(default_factory=dict)
    embed_fn: callable = None

    def _emb(self, text):
        return (self.embed_fn or embed)(text)

    # ---- construccion ----
    @classmethod
    def from_outcomes(cls, scale: float = 0.0, outcomes: list[dict] | None = None,
                      embed_fn=None) -> "ShadowPrior":
        sp = cls(scale=scale, embed_fn=embed_fn)
        rows = outcomes if outcomes is not None else read_outcomes()
        for r in rows:
            ctx = r.get("context") or ""
            arm = r.get("arm")
            if not ctx or arm is None:
                continue
            e = sp._emb(ctx)
            if e is None:
                continue
            sp.index.setdefault(arm, []).append((list(e), float(r.get("reward", 0.0))))
        return sp

    # ---- nucleo: pseudo-conteos para un brazo dado el contexto ----
    def prior_for(self, arm: str, context_emb: list[float] | None) -> tuple[float, float]:
        """Devuelve (alpha_prior, beta_prior). scale=0 o sin vecinos -> (0,0) = sin efecto."""
        if self.scale <= 0.0 or context_emb is None:
            return 0.0, 0.0
        pts = self.index.get(arm)
        if not pts or len(pts) < _MIN_NEIGHBORS:
            return 0.0, 0.0
        sims = sorted(((_cos(context_emb, e), rew) for e, rew in pts),
                      key=lambda t: t[0], reverse=True)[:_K]
        # peso por similitud (relu): vecinos lejanos pesan ~0
        wsum = sum(max(0.0, s) for s, _ in sims) or 1.0
        p_hat = sum(max(0.0, s) * rew for s, rew in sims) / wsum
        k_eff = min(len(sims), _K)
        strength = self.scale * k_eff           # pseudo-muestras que aporta el prior
        return strength * p_hat, strength * (1.0 - p_hat)

    # ---- seleccion: bandit + prior (scale=0 == bandit puro, bit a bit) ----
    def select(self, bandit: ThompsonBandit, arms: list[str], context: str | None = None,
               rng: _random.Random | None = None) -> str:
        rng = rng or _random.Random()
        ctx_emb = self._emb(context) if (context and self.scale > 0.0) else None
        best, best_theta = arms[0], -1.0
        for a in arms:
            alpha, beta = bandit._ab(a)                      # posterior actual del bandit
            ap, bp = self.prior_for(a, ctx_emb)
            theta = rng.betavariate(alpha + ap, beta + bp)
            if theta > best_theta:
                best, best_theta = a, theta
        return best


# ---- evaluacion offline + auto-scale (gated) -------------------------------- #
def _brier(preds: list[float], actual: list[float]) -> float:
    return sum((p - y) ** 2 for p, y in zip(preds, actual)) / max(1, len(preds))


def offline_improvement(outcomes: list[dict] | None = None, scale: float = SCALE_MIN,
                        embed_fn=None) -> float:
    """Mejora relativa de Brier al predecir reward con el prior contextual vs media global
    del brazo (leave-one-out por punto). >0 = el prior ayuda. Cero API (embeddings locales).
    embed_fn pluggable: probar code_embedder vs bge es como se decide si Fase 5 puede seguir."""
    ef = embed_fn or embed
    rows = [r for r in (outcomes if outcomes is not None else read_outcomes())
            if (r.get("context") and r.get("arm") is not None)]
    if len(rows) < 2 * _MIN_NEIGHBORS:
        return 0.0
    embs = [ef(r["context"]) for r in rows]
    arms = [r["arm"] for r in rows]
    rew = [float(r.get("reward", 0.0)) for r in rows]
    base_pred, prior_pred = [], []
    for i in range(len(rows)):
        same = [(embs[j], rew[j]) for j in range(len(rows)) if j != i and arms[j] == arms[i]]
        gmean = (sum(r for _, r in same) / len(same)) if same else 0.5
        base_pred.append(gmean)
        if embs[i] is None or len(same) < _MIN_NEIGHBORS:
            prior_pred.append(gmean); continue
        sims = sorted(((_cos(embs[i], e), r) for e, r in same if e is not None),
                      key=lambda t: t[0], reverse=True)[:_K]
        wsum = sum(max(0.0, s) for s, _ in sims) or 1.0
        prior_pred.append(sum(max(0.0, s) * r for s, r in sims) / wsum)
    b0 = _brier(base_pred, rew)
    b1 = _brier(prior_pred, rew)
    return 0.0 if b0 == 0 else (b0 - b1) / b0


_MIN_FRESH = 50   # outcomes nuevos exigidos entre escalones (anti re-evaluar el mismo set)


def auto_scale(current: float, outcomes: list[dict] | None = None,
               threshold: float = 0.02, embed_fn=None,
               n_seen_last_step: int | None = None) -> tuple[float, bool]:
    """Ajusta scale en pasos de ±0.1 segun mejora offline. Devuelve (nuevo_scale, needs_gate).
    Sube solo si mejora>threshold; baja si empeora. Clamp [0, SCALE_MAX]; querer >MAX => gate.
    Anti-overfit: si se pasa n_seen_last_step (cuantos outcomes habia en el escalon anterior),
    NO sube sin >= _MIN_FRESH outcomes nuevos — cada escalon exige evidencia fresca, no
    re-leer el mismo dataset. (Bajar por empeoramiento NO requiere frescura: seguridad.)"""
    rows = outcomes if outcomes is not None else read_outcomes()
    imp = offline_improvement(rows, scale=max(current, SCALE_MIN), embed_fn=embed_fn)
    if imp > threshold:
        if n_seen_last_step is not None and len(rows) - n_seen_last_step < _MIN_FRESH:
            return current, False                  # sin evidencia nueva: no sube
        proposed = round(current + 0.1, 4)
        if proposed > SCALE_MAX:
            return SCALE_MAX, True                 # tope alcanzado: subir mas = humano
        return max(SCALE_MIN, proposed), False
    if imp < 0:
        return round(max(0.0, current - 0.1), 4), False
    return current, False
