"""learn — meta-inteligencia: mmorch aprende de su propio metrics.jsonl (I-1).

Read-only sobre el log de metricas. Surfacea costo/latencia/uso por modelo x
patron y RECOMIENDA (gated, no auto-switchea) donde delegar mas barato sin perder
cross-family. El proxy de calidad debe validarse antes de auto-aplicar (critica
cross-family 2026-06-07): por eso esto RECOMIENDA, no impone.
"""
from __future__ import annotations

import statistics
from collections import defaultdict

from .metrics import read_events
from .config import family_of, spec


def analyze() -> dict:
    """Agrega el log: por (model, pattern) -> calls, costo, latencia p50/p95, tokens."""
    ev = read_events()
    agg: dict[tuple[str, str], dict] = defaultdict(
        lambda: {"calls": 0, "cost": 0.0, "lat": [], "out": []})
    for e in ev:
        k = (e.get("model", "?"), e.get("pattern", "?"))
        a = agg[k]
        a["calls"] += 1
        a["cost"] += e.get("cost_usd", 0.0)
        if e.get("latency_s"):
            a["lat"].append(e["latency_s"])
        if e.get("out_tokens"):
            a["out"].append(e["out_tokens"])
    rows = []
    for (model, pattern), a in sorted(agg.items(), key=lambda x: -x[1]["cost"]):
        lat = a["lat"] or [0]
        rows.append({
            "model": model, "pattern": pattern, "family": _fam(model),
            "calls": a["calls"], "cost_usd": round(a["cost"], 5),
            "cost_per_call": round(a["cost"] / max(a["calls"], 1), 6),
            "lat_p50": round(statistics.median(lat), 2),
            "lat_p95": round(_pctl(lat, 95), 2),
            "avg_out_tok": int(statistics.mean(a["out"])) if a["out"] else 0,
        })
    return {"rows": rows, "total_calls": len(ev),
            "total_cost_usd": round(sum(r["cost_usd"] for r in rows), 5)}


def recommend() -> list[str]:
    """Recomendaciones accionables (gated). No modifica config."""
    rep = analyze()
    rows = rep["rows"]
    recs: list[str] = []
    # 1. Modelo de generacion mas caro por call dentro de fan_out -> sugerir el mas barato de otra familia.
    gen = [r for r in rows if r["pattern"] == "fan_out"]
    if len(gen) >= 2:
        cheap = min(gen, key=lambda r: r["cost_per_call"])
        dear = max(gen, key=lambda r: r["cost_per_call"])
        if dear["cost_per_call"] > 1.5 * cheap["cost_per_call"]:
            recs.append(
                f"fan_out: {dear['model']} cuesta {dear['cost_per_call']/cheap['cost_per_call']:.1f}x "
                f"vs {cheap['model']} por call. Usar {cheap['model']} para bulk salvo que se "
                f"requiera la familia de {dear['model']}.")
    # 2. Latencia alta -> sugerir bajar timeout o modelo mas rapido.
    slow = [r for r in rows if r["lat_p95"] > 30]
    for r in slow:
        recs.append(
            f"{r['model']}/{r['pattern']}: latencia p95 {r['lat_p95']}s (alta). "
            f"Revisar timeout o usar un modelo mas rapido si la calidad lo permite.")
    # 3. Gap de observabilidad: el verdict de adversarial_verify no se loggea -> sin proxy de calidad.
    if any(r["pattern"] == "adversarial_verify" for r in rows):
        recs.append(
            "GAP: adversarial_verify loggea costo pero NO el verdict (passed/confidence). "
            "Sin eso no hay proxy de calidad por verificador -> no se puede auto-tunear con "
            "fundamento. Proximo paso: loggear verdict en metrics (habilita I-1 completo).")
    if not recs:
        recs.append("Sin recomendaciones: datos insuficientes o ya optimo.")
    return recs


def _fam(model: str) -> str:
    try:
        return family_of(model)
    except Exception:
        return "?"


def _pctl(xs: list[float], p: float) -> float:
    if not xs:
        return 0.0
    s = sorted(xs)
    k = (len(s) - 1) * p / 100.0
    f = int(k)
    return s[f] if f + 1 >= len(s) else s[f] + (s[f + 1] - s[f]) * (k - f)
