"""Ablacion §18.4 — PAREADA, n=350 (benchmark honesto, powered).

Que testea (y que NO):
  SI  -> PODER DE DETECCION por FAMILIA DE VERIFICADOR. Construimos un gold set con
         ground-truth COMPUTADA (label infalible, sin humano): ~50% artefactos con
         respuesta correcta, ~50% con error INYECTADO determinista. El MISMO artefacto
         lo juzgan los dos verificadores (DISEÑO PAREADO) -> McNemar sobre pares
         discordantes. Mide sensibilidad (caza-bug) y falso-rechazo por familia.
  NO  -> el blind-spot GENERATIVO puro ("un modelo no ve SU PROPIO error"). Eso exige
         que el modelo yerre solo (no inyectado) o artefactos subjetivos sin ground-truth
         -> fuera de scope (closer §18.4: deepseek no erro 46/46 en computables). El error
         inyectado NO es "propio" del verificador, asi que aca NO se mide auto-endoso.

Por que pareado: cada item se juzga en AMBAS condiciones -> controla la dificultad del
item (varianza). McNemar usa solo los pares discordantes -> mucho mas poder por call que
dos muestras independientes. n=350 detecta ~10pp de diferencia con power~0.8.

Determinista: seed fijo, problemas y perturbaciones generadas por RNG sembrado. Sin
datetime/random global (script normal, no Workflow). Idempotente.

Uso:  python ablation_paired.py [--n 350] [--seed 42] [--workers 8] [--yes] [--dry]
  --dry  : genera gold set + estima costo, NO llama API (verifica logica gratis).
  --yes  : salta la confirmacion de presupuesto y corre las llamadas reales.
"""
from __future__ import annotations

import argparse
import math
import random
import re
import sys
import pathlib
from concurrent.futures import ThreadPoolExecutor, as_completed

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from mmorch.providers import call
from mmorch.patterns import _parse_verdict
from mmorch.config import spec, family_of

SELF_VERIFIER = "deepseek-chat"      # arm A
CROSS_VERIFIER = "gemini-2.5-flash"  # arm B (cross-family vs SELF)
COST_CAP_USD = 2.0                   # guard: abortar si la estimacion lo supera sin --yes


# --------------------------------------------------------------------------- #
# gold set: problemas parametricos con truth computada (label infalible)       #
# --------------------------------------------------------------------------- #
def _modinv(a, m):
    g, x = m, 0
    a0, x0 = a % m, 1
    while a0:
        q = g // a0
        g, a0 = a0, g - q * a0
        x, x0 = x0, x - q * x0
    return x % m if g == 1 else None


def _gen_problem(rng: random.Random):
    """Devuelve (texto, truth_int). Familia elegida por rng -> escala a cualquier n."""
    kind = rng.randrange(10)
    if kind == 0:
        a, b, m = rng.randint(2, 19), rng.randint(20, 120), rng.choice([97, 1000, 1009])
        return (f"Cuanto es {a} elevado a {b}, modulo {m}?", pow(a, b, m))
    if kind == 1:
        n = rng.choice([50, 75, 100, 125, 200])
        return (f"Cuantos ceros al final tiene {n}! (factorial de {n})?",
                sum(n // 5 ** k for k in range(1, 6)))
    if kind == 2:
        n = rng.choice([360, 840, 1260, 5040, 720])
        return (f"Suma de TODOS los divisores positivos de {n} (incluido 1 y {n})?",
                sum(d for d in range(1, n + 1) if n % d == 0))
    if kind == 3:
        a, b = rng.randint(200, 4000), rng.randint(100, 2000)
        return (f"Maximo comun divisor de {a} y {b} (Euclides)?", math.gcd(a, b))
    if kind == 4:
        n = rng.randint(10, 26)
        k = rng.randint(3, n - 2)
        return (f"Cuanto es la combinatoria C({n},{k})?", math.comb(n, k))
    if kind == 5:
        a, b = rng.randint(7, 23), rng.randint(11, 21)
        return (f"Cual es la suma de los digitos de {a} elevado a la {b}?",
                sum(int(d) for d in str(a ** b)))
    if kind == 6:
        n = rng.choice([100, 200, 500, 1000])
        return (f"Cuantos numeros primos hay menores a {n}?",
                len([x for x in range(2, n) if all(x % d for d in range(2, int(x**0.5) + 1))]))
    if kind == 7:
        a = rng.choice([7, 11, 13, 17, 19, 23])
        m = rng.choice([26, 30, 50, 97])
        inv = _modinv(a, m)
        if inv is None:
            return (f"Cuanto es {a} elevado a 3?", a ** 3)
        return (f"Cual es el inverso modular de {a} modulo {m}? (x con {a}x ≡ 1 mod {m})", inv)
    if kind == 8:
        n = rng.randint(10, 30)
        return (f"Cuanto es la suma de i al cubo para i de 1 a {n}?",
                sum(i ** 3 for i in range(1, n + 1)))
    n = rng.choice([3, 4, 5, 6, 7])
    e = rng.randint(8, 15)
    return (f"Cuanto es {n} elevado a {e}?", n ** e)


def _perturb(truth: int, rng: random.Random) -> int:
    """Error PLAUSIBLE-CERCANO y != truth. Evita absurdos (negativo, x10) que cualquier
    verificador caza trivial -> efecto techo. El caso plausible es el que DISCRIMINA
    entre familias de verificador (la pregunta de la ablacion)."""
    for _ in range(8):
        how = rng.randrange(3)
        if how == 0:                                   # off-by-poco
            cand = truth + rng.choice([-2, -1, 1, 2])
        elif how == 1:                                 # transponer 2 digitos contiguos
            s = str(abs(truth))
            if len(s) >= 2:
                i = rng.randrange(len(s) - 1)
                if s[i] != s[i + 1]:
                    s = s[:i] + s[i + 1] + s[i] + s[i + 2:]
                    cand = int(s) * (1 if truth >= 0 else -1)
                else:
                    cand = truth + 1
            else:
                cand = truth + 1
        else:                                          # nudge chico (~1-5%), mismo signo/orden
            delta = max(1, round(abs(truth) * rng.choice([0.01, 0.02, 0.05])))
            cand = truth + rng.choice([-1, 1]) * delta
        if cand != truth and (truth == 0 or (cand >= 0) == (truth >= 0)):
            return int(cand)
    return truth + 1


def build_gold(n: int, seed: int) -> list[dict]:
    """n artefactos: ~50% correctos, ~50% con error inyectado. is_correct = ground-truth."""
    rng = random.Random(seed)
    gold = []
    for i in range(n):
        text, truth = _gen_problem(rng)
        is_correct = (i % 2 == 0)
        proposed = truth if is_correct else _perturb(truth, rng)
        gold.append({"i": i, "problem": text, "truth": truth,
                     "proposed": proposed, "is_correct": is_correct})
    return gold


# --------------------------------------------------------------------------- #
# verificacion                                                                 #
# --------------------------------------------------------------------------- #
_VERIFY_SYS = (
    "Verificador adversarial. Resolve el problema vos mismo y compara con la respuesta "
    "propuesta. Refuta por default. passed=true SOLO si la respuesta propuesta es "
    'numericamente correcta. JSON: {"passed": bool, "confidence": 0..1, "refutations": [string]}'
)


def _verify(model: str, item: dict, retries: int = 2):
    """Verifica con reintentos. H-1: un timeout transitorio NO debe matar el batch."""
    art = f"PROBLEMA:\n{item['problem']}\n\nRESPUESTA PROPUESTA: {item['proposed']}"
    last = None
    for _ in range(retries + 1):
        try:
            res = call(model, [{"role": "system", "content": _VERIFY_SYS},
                               {"role": "user", "content": art}],
                       pattern="ablation_paired", node=f"v:{model}", phase="ablation_paired",
                       temperature=0.0)
            passed, conf, refs = _parse_verdict(res.text)
            return passed, res.cost_usd
        except Exception as e:  # APITimeoutError, rate-limit, red — reintentar
            last = e
    raise last


def _judge_item(item: dict):
    """Corre AMBOS verificadores sobre el MISMO item (par). Devuelve None si UN
    verificador falla tras reintentos -> el par se descarta (no se puede parear a
    medias). main filtra los None y reporta cuantos cayeron (sin truncado silencioso)."""
    try:
        self_passed, c1 = _verify(SELF_VERIFIER, item)
        cross_passed, c2 = _verify(CROSS_VERIFIER, item)
    except Exception:
        return None
    return {**item,
            "self_passed": self_passed, "cross_passed": cross_passed,
            "self_right": self_passed == item["is_correct"],
            "cross_right": cross_passed == item["is_correct"],
            "cost": c1 + c2}


# --------------------------------------------------------------------------- #
# estadistica (stdlib, sin scipy)                                              #
# --------------------------------------------------------------------------- #
def _wilson(k: int, n: int):
    """IC95 Wilson para una proporcion (mejor que normal con n chico/extremos)."""
    if n == 0:
        return (None, None, None)
    z = 1.96
    p = k / n
    d = 1 + z * z / n
    c = p + z * z / (2 * n)
    half = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n))
    return (round(p, 4), round((c - half) / d, 4), round((c + half) / d, 4))


def _mcnemar(b: int, c: int):
    """McNemar pareado. b = self acierta & cross falla; c = self falla & cross acierta.
    chi2 con correccion de continuidad + p exacto binomial (two-sided)."""
    n = b + c
    if n == 0:
        return {"b": b, "c": c, "chi2": None, "p_exact": 1.0, "note": "sin pares discordantes"}
    chi2 = (abs(b - c) - 1) ** 2 / n if n > 0 else 0.0
    k = min(b, c)
    p = sum(math.comb(n, j) for j in range(0, k + 1)) * (0.5 ** n) * 2
    return {"b": b, "c": c, "chi2": round(chi2, 4), "p_exact": round(min(1.0, p), 5)}


def _arm_stats(rows: list[dict], arm: str) -> dict:
    flawed = [r for r in rows if not r["is_correct"]]
    correct = [r for r in rows if r["is_correct"]]
    pk = f"{arm}_passed"
    # sensibilidad = caza-bug = refuto (passed=False) un artefacto fallado
    caught = sum(1 for r in flawed if not r[pk])
    # especificidad = NO refuto un artefacto correcto
    kept = sum(1 for r in correct if r[pk])
    sens = _wilson(caught, len(flawed))
    spec = _wilson(kept, len(correct))
    acc = ((sens[0] or 0) + (spec[0] or 0)) / 2
    return {"arm": arm, "n_flawed": len(flawed), "n_correct": len(correct),
            "sensitivity": sens, "specificity": spec, "balanced_acc": round(acc, 4)}


def estimate_cost(n: int) -> float:
    """2 verificadores * n. Estima con precio de salida ~200 tok por verdict."""
    tot = 0.0
    for m in (SELF_VERIFIER, CROSS_VERIFIER):
        s = spec(m)
        # ~300 tok in (problema+respuesta) + ~200 out, por call
        tot += n * (300 / 1e6 * s.price_in + 200 / 1e6 * s.price_out)
    return tot


# --------------------------------------------------------------------------- #
# main                                                                         #
# --------------------------------------------------------------------------- #
def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, default=350)
    ap.add_argument("--seed", type=int, default=42)
    ap.add_argument("--workers", type=int, default=8)
    ap.add_argument("--yes", action="store_true")
    ap.add_argument("--dry", action="store_true")
    args = ap.parse_args()

    if family_of(SELF_VERIFIER) == family_of(CROSS_VERIFIER):
        sys.exit("config rota: SELF y CROSS comparten familia (no es cross-family).")

    gold = build_gold(args.n, args.seed)
    n_c = sum(1 for g in gold if g["is_correct"])
    est = estimate_cost(args.n)
    print(f"gold set: {len(gold)} items ({n_c} correctos, {len(gold)-n_c} fallados) | "
          f"seed={args.seed}")
    print(f"arms (pareados): SELF={SELF_VERIFIER} ({family_of(SELF_VERIFIER)}) vs "
          f"CROSS={CROSS_VERIFIER} ({family_of(CROSS_VERIFIER)})")
    print(f"calls: {args.n * 2} | costo estimado: ${est:.4f}")

    if args.dry:
        print("\n[DRY] muestra de 4 items:")
        for g in gold[:4]:
            print(f"  i={g['i']} ok={g['is_correct']} truth={g['truth']} "
                  f"prop={g['proposed']} | {g['problem'][:60]}")
        print("\n[DRY] sin llamadas API. Quitar --dry (y poner --yes) para correr.")
        return

    if est > COST_CAP_USD and not args.yes:
        sys.exit(f"costo estimado ${est:.2f} > cap ${COST_CAP_USD}. Repetir con --yes.")
    if not args.yes:
        sys.exit("corrida real gasta API. Repetir con --yes para confirmar.")

    rows: list[dict] = []
    dropped = 0
    cost = 0.0
    with ThreadPoolExecutor(max_workers=args.workers) as ex:
        futs = [ex.submit(_judge_item, g) for g in gold]
        for j, fut in enumerate(as_completed(futs), 1):
            r = fut.result()
            if r is None:
                dropped += 1
            else:
                rows.append(r)
                cost += r["cost"]
            if j % 50 == 0:
                print(f"  ... {j}/{len(gold)} (${cost:.3f}, {dropped} caidos)")
    if not rows:
        sys.exit("todos los pares cayeron (API caida?). Sin resultado.")
    if dropped:
        print(f"\nAVISO: {dropped}/{len(gold)} pares descartados por fallo de API "
              f"(no truncado silencioso). Analisis sobre {len(rows)} pares completos.")

    # McNemar sobre la decision-correcta (acierto del verificador vs ground-truth)
    b = sum(1 for r in rows if r["self_right"] and not r["cross_right"])
    c = sum(1 for r in rows if not r["self_right"] and r["cross_right"])
    mc = _mcnemar(b, c)
    self_acc = sum(1 for r in rows if r["self_right"]) / len(rows)
    cross_acc = sum(1 for r in rows if r["cross_right"]) / len(rows)

    print("\n" + "=" * 64)
    print(f"RESULTADO (n={len(rows)}, costo real ${cost:.4f})")
    print("=" * 64)
    for arm in ("self", "cross"):
        s = _arm_stats(rows, arm)
        print(f"\n{arm.upper()} ({SELF_VERIFIER if arm=='self' else CROSS_VERIFIER}):")
        print(f"  sensibilidad (caza-bug):  {s['sensitivity'][0]} "
              f"IC95[{s['sensitivity'][1]}, {s['sensitivity'][2]}]  (n_fallados={s['n_flawed']})")
        print(f"  especificidad (no falso-rechazo): {s['specificity'][0]} "
              f"IC95[{s['specificity'][1]}, {s['specificity'][2]}]  (n_correctos={s['n_correct']})")
        print(f"  accuracy balanceada: {s['balanced_acc']}")
    print(f"\nACCURACY GLOBAL de decision: self={self_acc:.4f}  cross={cross_acc:.4f}")
    print(f"\nMcNEMAR (pareado, decision correcta):")
    print(f"  b (solo SELF acierta)={mc['b']}  c (solo CROSS acierta)={mc['c']}")
    if mc.get("chi2") is not None:
        print(f"  chi2(cc)={mc['chi2']}  p_exact={mc['p_exact']}")
        verdict = ("DIFERENCIA SIGNIFICATIVA (p<0.05)" if mc["p_exact"] < 0.05
                   else "SIN diferencia significativa (p>=0.05)")
        better = "CROSS" if c > b else ("SELF" if b > c else "empate")
        print(f"  -> {verdict}; direccion: {better}")
    else:
        print(f"  {mc.get('note')}")
    print("\nNOTA: esto mide PODER DE DETECCION por familia de verificador (pareado), "
          "NO el blind-spot generativo (error propio). Ver docstring.")


if __name__ == "__main__":
    main()
