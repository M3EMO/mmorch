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
from .feedback import calibration, ThompsonBandit


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
            "total_cost_usd": round(sum(r["cost_usd"] for r in rows), 5),
            "verdict_quality": _verdict_quality(ev),  # proxy de calidad por verificador (P3b)
            "calibration": calibration(),          # ECE conf-predicha vs realidad (feedback loop)
            "bandit": ThompsonBandit().stats()}     # brazos aprendidos (cascade thresholds, etc.)


def _verdict_quality(ev: list[dict]) -> dict:
    """Consume los eventos adversarial_verify_verdict (passed/confidence en `extra`)
    como proxy de calidad por verificador. Esto es lo que cierra P3b: el verdict no
    solo se loggea, se LEE. pass_rate alto + conf alta sin refutaciones puede ser
    sicofancia -> se cruza con calibration (ECE) afuera."""
    agg: dict[str, dict] = defaultdict(lambda: {"n": 0, "passed": 0, "conf": []})
    for e in ev:
        if e.get("pattern") != "adversarial_verify_verdict":
            continue
        x = e.get("extra") or {}
        a = agg[e.get("model", "?")]
        a["n"] += 1
        a["passed"] += 1 if x.get("passed") else 0
        if x.get("confidence") is not None:
            a["conf"].append(float(x["confidence"]))
    out = {}
    for model, a in agg.items():
        out[model] = {
            "n": a["n"],
            "pass_rate": round(a["passed"] / a["n"], 4) if a["n"] else None,
            "avg_confidence": round(statistics.mean(a["conf"]), 4) if a["conf"] else None,
        }
    return out


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
    # 3. Proxy de calidad por verificador (P3b): consumir los verdict events si existen.
    vq = rep.get("verdict_quality") or {}
    has_verify = any(r["pattern"] == "adversarial_verify" for r in rows)
    if has_verify and not vq:
        # verify corrio pero no hay verdict events -> gap real de observabilidad.
        recs.append(
            "GAP: adversarial_verify corrio pero no hay eventos de verdict en metrics "
            "(passed/confidence). Sin eso no hay proxy de calidad por verificador.")
    elif vq:
        for model, q in vq.items():
            if not q.get("n"):
                continue
            note = ""
            if q.get("pass_rate") is not None and q["pass_rate"] >= 0.95 and q.get("avg_confidence", 0) >= 0.85:
                note = (" [posible sicofancia: casi todo pasa con conf alta -> cruzar con ECE; "
                        "el verificador puede no estar refutando de verdad]")
            recs.append(
                f"VERDICT/{model}: pass_rate={q['pass_rate']} avg_conf={q.get('avg_confidence')} "
                f"(n={q['n']}). Proxy de calidad por verificador activo.{note}")
    # 4. Calibracion (feedback loop): ECE alto = conf auto-reportada NO es de fiar.
    cal = rep.get("calibration") or {}
    ece = cal.get("ece")
    if ece is not None and cal.get("n", 0) >= 10:
        if ece > 0.15:
            recs.append(
                f"CALIBRACION: ECE={ece} sobre {cal['n']} outcomes (>0.15 = mal calibrado). "
                f"La CONFIDENCE auto-reportada miente -> SUBIR umbrales de cascade (escalar mas) "
                f"o no fiarse del self-score como senal de calidad. Anti-sicofancia: la conf "
                f"no es reward.")
        else:
            recs.append(
                f"CALIBRACION OK: ECE={ece} sobre {cal['n']} outcomes (<=0.15). "
                f"La conf auto-reportada es razonablemente fiable; umbrales de cascade validos.")
    # 5. Bandit (cascade thresholds aprendidos): reportar brazo dominante con n suficiente.
    bandit = rep.get("bandit") or {}
    ready = {a: s for a, s in bandit.items() if s.get("n", 0) >= 10}
    if ready:
        best = max(ready.items(), key=lambda kv: kv[1]["mean"])
        recs.append(
            f"BANDIT: brazo lider '{best[0]}' mean={best[1]['mean']} (n={best[1]['n']}). "
            f"Si domina con margen, fijar ese umbral/modelo como default de cascade.")
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
