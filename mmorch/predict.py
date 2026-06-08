"""predict (v0.1 NN, Fase 1) — predictor de out_tokens / latencia, SIN dep pesada.

Insight: cost_usd NO se predice — es determinista (cost.py = tokens x precio). Lo
INCIERTO es cuántos out_tokens genera el modelo y cuánta latencia. Esto predice eso
desde features tabulares (modelo, patrón, in_tokens) aprendidos de metrics.jsonl, y
deriva cost = precio x out_tok_predicho.

Modelo: "regresión simple" robusta (la rama lineal del plan, sin LightGBM) — por grupo
(modelo[,patrón]) usa la MEDIANA + una pendiente lineal opcional en in_tokens; fallback
jerárquico (modelo,patrón) -> (modelo) -> global. numpy ya es dep; cero barrera nueva.
Entrenable HOY (target loggeado directo, cero labels de feedback, cero overfit).
"""
from __future__ import annotations

from collections import defaultdict

from .metrics import read_events
from .cost import cost_usd


def _quantile(xs: list[float], q: float) -> float:
    """Cuantil q (0..1) por interpolación lineal. q=0.5 = mediana, q=0.9 = conservador."""
    if not xs:
        return 0.0
    s = sorted(xs)
    if len(s) == 1:
        return s[0]
    pos = q * (len(s) - 1)
    lo = int(pos)
    frac = pos - lo
    return s[lo] if lo + 1 >= len(s) else s[lo] + (s[lo + 1] - s[lo]) * frac


def _rows(events=None) -> list[dict]:
    ev = events if events is not None else read_events()
    out = []
    for e in ev:
        it, ot = e.get("in_tokens", 0) or 0, e.get("out_tokens", 0) or 0
        if ot <= 0:                       # filtrar verdict-only / errores (out=0)
            continue
        out.append({"model": e.get("model", "?"), "pattern": e.get("pattern", "?"),
                    "in": it, "out": ot, "lat": e.get("latency_s", 0.0) or 0.0})
    return out


class Predictor:
    """Predice out_tokens y latencia por (modelo, patrón) vía CUANTILES (no solo media).
    q=0.5 = best-estimate; q=0.9 = conservador (pa budget — sobre-estima, evita el +$5)."""

    def __init__(self):
        self.by_mp: dict[tuple, list] = {}   # (model,pattern) -> {out:[...], lat:[...]}
        self.by_m: dict[str, dict] = {}
        self.glob = {"out": [1.0], "lat": [0.0]}

    def fit(self, rows: list[dict]) -> "Predictor":
        g_mp, g_m = defaultdict(lambda: {"out": [], "lat": []}), defaultdict(lambda: {"out": [], "lat": []})
        for r in rows:
            for g in (g_mp[(r["model"], r["pattern"])], g_m[r["model"]]):
                g["out"].append(r["out"]); g["lat"].append(r["lat"])
        self.by_mp, self.by_m = dict(g_mp), dict(g_m)
        if rows:
            self.glob = {"out": [r["out"] for r in rows], "lat": [r["lat"] for r in rows]}
        return self

    def _lookup(self, model: str, pattern: str | None) -> dict:
        if pattern is not None and (model, pattern) in self.by_mp and len(self.by_mp[(model, pattern)]["out"]) >= 3:
            return self.by_mp[(model, pattern)]
        if model in self.by_m:
            return self.by_m[model]
        return self.glob

    def predict_out_tokens(self, model: str, pattern: str | None = None, q: float = 0.5) -> float:
        return _quantile(self._lookup(model, pattern)["out"], q)

    def predict_latency(self, model: str, pattern: str | None = None, q: float = 0.5) -> float:
        return _quantile(self._lookup(model, pattern)["lat"], q)

    def predict_cost(self, model: str, *, pattern: str | None = None,
                     in_tokens: int = 0, prompt: str | None = None, q: float = 0.5) -> float:
        """cost = precio_in*in_tok + precio_out*out_tok_predicho(q). in_tokens estimado de
        `prompt` (~len/4) si no se da. q=0.9 → estimación CONSERVADORA pa budget."""
        if prompt is not None and not in_tokens:
            in_tokens = max(1, len(prompt) // 4)
        out_tok = self.predict_out_tokens(model, pattern, q)
        return cost_usd(model, int(in_tokens), int(out_tok))


def train(events=None) -> Predictor:
    return Predictor().fit(_rows(events))


def cross_val_error(events=None, *, k: int = 5) -> dict:
    """MAPE de out_tokens por k-fold determinista (sin shuffle aleatorio). Reporta error
    honesto del predictor. Devuelve {mape_out, mape_lat, n}."""
    rows = _rows(events)
    n = len(rows)
    if n < k * 2:
        return {"mape_out": None, "mape_lat": None, "n": n, "note": "datos insuficientes"}
    err_o, err_l, cnt, covered = 0.0, 0.0, 0, 0
    for f in range(k):
        test = [r for i, r in enumerate(rows) if i % k == f]
        train_rows = [r for i, r in enumerate(rows) if i % k != f]
        p = Predictor().fit(train_rows)
        for r in test:
            po = p.predict_out_tokens(r["model"], r["pattern"])           # mediana
            po90 = p.predict_out_tokens(r["model"], r["pattern"], q=0.9)  # conservador
            pl = p.predict_latency(r["model"], r["pattern"])
            if r["out"] > 0:
                err_o += abs(po - r["out"]) / r["out"]
                covered += 1 if r["out"] <= po90 else 0
                cnt += 1
            if r["lat"] > 0:
                err_l += abs(pl - r["lat"]) / r["lat"]
    return {"mape_out_median": round(err_o / max(cnt, 1), 4),
            "mape_lat_median": round(err_l / max(cnt, 1), 4),
            "p90_coverage_out": round(covered / max(cnt, 1), 4),  # ~0.9 = bien calibrado conservador
            "n": n, "k": k}
