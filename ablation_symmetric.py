"""Ablacion §18.4 — SIMETRICA 4-celdas: aisla CROSS-FAMILY de CALIDAD-DE-VERIFICADOR.

El run pareado previo (ablation_paired.py) tenia un confound: comparaba
deepseek-verificador vs gemini-verificador sobre errores INYECTADOS -> medias
'que verificador es mejor', NO 'verificar misma-familia vs distinta'. Que gemini
ganara podia ser solo 'gemini es mejor verificador'.

Este diseño lo arregla. Factor 1 = familia del GENERADOR (deepseek | gemini).
Factor 2 = relacion del verificador (SELF = misma familia que el generador |
CROSS = distinta). 2x2 con generadores REALES y errores NATURALES (no inyectados)
-> testea el blind-spot generativo de verdad: un modelo, ¿ve SU PROPIO error?

  autor=deepseek -> ans_d ; verifican: deepseek(SELF) y gemini(CROSS)   [pareado en ans_d]
  autor=gemini   -> ans_g ; verifican: gemini(SELF)   y deepseek(CROSS) [pareado en ans_g]

POOLING que aisla el confound:
  SELF  = {deepseek juzga ans_d} ∪ {gemini juzga ans_g}
  CROSS = {gemini juzga ans_d}   ∪ {deepseek juzga ans_g}
  Cada familia aparece como verificador en AMBAS condiciones -> su calidad se
  cancela en el contraste. Lo que queda es el efecto de la RELACION de familia.

Metricas (pareadas, McNemar):
  - blind-spot catch: sobre respuestas NATURALMENTE incorrectas, ¿SELF caza menos
    el error que CROSS? (la tesis fuerte cross-family).
  - false-refute: sobre respuestas correctas, ¿SELF rechaza distinto que CROSS?

Requiere problemas DUROS (si el generador no yerra, no hay casos de blind-spot).
seed fijo, determinista. Uso: python ablation_symmetric.py [--n 350] [--yes] [--dry].
"""
from __future__ import annotations

import argparse
import math
import random
import re
import sys
import time
import pathlib
from concurrent.futures import ThreadPoolExecutor, as_completed

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from mmorch.providers import call
from mmorch.patterns import _parse_verdict
from mmorch.config import spec, family_of
from ablation_paired import _mcnemar, _wilson   # reuso estadistica (stdlib)

import threading

FAM_A = "deepseek-chat"
FAM_B = "gemini-2.5-flash"
COST_CAP_USD = 4.0


class RateGate:
    """Limitador de tasa POR-MODELO (decopla concurrencia de RPM). Reserva slots de
    tiempo espaciados 60/rpm por modelo: muchos workers, pero cada modelo NO supera su
    RPM -> deepseek (rpm alto) vuela, gemini (rpm bajo) se espacia sin reventar en 429.
    Reserva bajo lock (instantaneo), duerme AFUERA -> no serializa el computo."""

    def __init__(self, rpm_by_model: dict[str, float]):
        self._interval = {m: 60.0 / r for m, r in rpm_by_model.items()}
        self._next = {m: 0.0 for m in rpm_by_model}
        self._locks = {m: threading.Lock() for m in rpm_by_model}

    def reserve(self, model: str) -> None:
        iv = self._interval.get(model)
        if iv is None:
            return
        with self._locks[model]:
            now = time.monotonic()
            slot = max(now, self._next[model])
            self._next[model] = slot + iv
        delay = slot - time.monotonic()
        if delay > 0:
            time.sleep(delay)


_GATE: RateGate | None = None  # se setea en main() con los RPM elegidos


def _gated_call(model, msgs, **kw):
    if _GATE is not None:
        _GATE.reserve(model)
    return call(model, msgs, **kw)


# --- memo (I-4): cache content-hash. seed fijo + temp=0 -> re-runs identicos = $0 --- #
_MEMO = None                       # Memo() en main si --cache
_CACHE = {"hit": 0, "miss": 0}
_CACHE_LOCK = threading.Lock()


def _memo_get(*parts):
    if _MEMO is None:
        return None, None
    from mmorch.cache import key_of
    k = key_of(*parts)
    hit = _MEMO.get(k)
    with _CACHE_LOCK:
        _CACHE["hit" if hit is not None else "miss"] += 1
    return k, hit


def _memo_put(k, value):
    if _MEMO is not None and k is not None:
        _MEMO.put(k, value)


# --------------------------------------------------------------------------- #
# gold set DURO: ground-truth computada, dificil para que el generador YERRE    #
# --------------------------------------------------------------------------- #
def _det(m):
    n = len(m)
    if n == 1:
        return m[0][0]
    tot = 0
    for c in range(n):
        minor = [row[:c] + row[c + 1:] for row in m[1:]]
        tot += ((-1) ** c) * m[0][c] * _det(minor)
    return tot


def _gen_hard(rng: random.Random):
    kind = rng.randrange(8)
    if kind == 0:                                  # det 4x4 (muy error-prone a mano)
        m = [[rng.randint(1, 9) for _ in range(4)] for _ in range(4)]
        return (f"Determinante de la matriz 4x4 {m}?", _det(m))
    if kind == 1:                                  # digit sum de a^b grande
        a, b = rng.randint(13, 39), rng.randint(17, 31)
        return (f"Suma de los digitos de {a} elevado a {b}?", sum(int(d) for d in str(a ** b)))
    if kind == 2:                                  # modexp exponente grande
        a, b, m = rng.randint(7, 29), rng.randint(50, 250), rng.choice([1009, 9973, 99991])
        return (f"Cuanto es {a} elevado a {b}, modulo {m}?", pow(a, b, m))
    if kind == 3:                                  # sigma de altamente compuesto
        n = rng.choice([5040, 720720, 332640, 498960])
        return (f"Suma de TODOS los divisores positivos de {n}?",
                sum(d for d in range(1, n + 1) if n % d == 0))
    if kind == 4:                                  # inclusion-exclusion 3 sets
        x, y, z = rng.choice([6, 7, 8]), rng.choice([10, 11, 12]), rng.choice([14, 15])
        N = rng.choice([3000, 5000, 7000])
        truth = len([k for k in range(1, N + 1) if k % x == 0 or k % y == 0 or k % z == 0])
        return (f"Cuantos enteros entre 1 y {N} son divisibles por {x}, {y} o {z}? (incl-excl)", truth)
    if kind == 5:                                  # producto de cadena con rounding
        base = rng.randint(800, 1500)
        fs = [rng.choice([1.07, 1.12, 0.91, 1.04, 0.88]) for _ in range(4)]
        v = base
        for f in fs:
            v *= f
        return (f"Un capital de {base} se multiplica sucesivamente por {fs}. "
                f"Valor final redondeado a 2 decimales?", round(v, 2))
    if kind == 6:                                  # Catalan grande
        k = rng.randint(9, 15)
        return (f"Cual es el {k}-esimo numero de Catalan C_{k} (C_0=1)?",
                math.comb(2 * k, k) // (k + 1))
    a, b = rng.randint(20, 28), None               # combinatoria grande
    b = rng.randint(8, a - 8)
    return (f"Cuanto es la combinatoria C({a},{b})?", math.comb(a, b))


def build_gold(n: int, seed: int) -> list[dict]:
    rng = random.Random(seed)
    return [{"i": i, **dict(zip(("problem", "truth"), _gen_hard(rng)))} for i in range(n)]


# --------------------------------------------------------------------------- #
# generar (resolver) y verificar                                               #
# --------------------------------------------------------------------------- #
_SOLVE_SYS = ("Resolve el problema. Razona breve. Terminá con una linea exacta: "
              "RESPUESTA: <solo el numero final, sin unidades ni separador de miles>.")
_VERIFY_SYS = ("Verificador adversarial. Resolve el problema vos mismo y compara con la "
               "respuesta propuesta. Refuta por default. passed=true SOLO si la respuesta "
               'propuesta es numericamente correcta. JSON: {"passed": bool, "confidence": '
               '0..1, "refutations": [string]}')

_NUM = re.compile(r"RESPUESTA\s*[:=]\s*\$?\s*(-?[\d.,]+)", re.I)


def _parse_num(text):
    m = _NUM.search(text or "")
    raw = m.group(1) if m else (re.findall(r"-?\d[\d.,]*", text or "") or [None])[-1]
    if raw is None:
        return None
    raw = raw.strip().rstrip(".").replace(" ", "")
    if "," in raw and "." in raw:
        raw = raw.replace(",", "")
    elif "," in raw:
        raw = raw.replace(",", ".")
    try:
        return float(raw)
    except ValueError:
        return None


def _close(a, b):
    return a is not None and b is not None and abs(a - b) <= max(0.02, abs(b) * 1e-4)


_FATAL = ("depleted", "billing", "prepayment", "insufficient", "exhausted credit")


def _retry(fn, retries=4, base=2.0):
    """Reintenta con BACKOFF exponencial + jitter (suaviza rate-limit RPM de gemini).
    Aborta YA si el error es de billing (no transitorio) -> no malgasta reintentos."""
    last = None
    for k in range(retries + 1):
        try:
            return fn()
        except Exception as e:
            msg = str(e).lower()
            if any(f in msg for f in _FATAL):
                raise                       # pared dura: credito agotado, no reintentar
            last = e
            if k < retries:
                # jitter determinista-suficiente sin random global: depende de k
                time.sleep(base * (2 ** k) + (k % 3) * 0.3)
    raise last


# techo DURO de tokens/call: sin esto el default es 16384 -> hasta $0.04/call gemini.
# solve razona breve (~300), verify devuelve JSON corto (~150). 768 deja margen y capea.
_MAX_TOK_SOLVE = 768
_MAX_TOK_VERIFY = 512


def _solve(model, problem):
    k, hit = _memo_get("ablsym_solve", model, problem)
    if hit is not None:
        return hit, 0.0                       # cache hit -> sin API, costo 0
    res = _retry(lambda: _gated_call(model, [{"role": "system", "content": _SOLVE_SYS},
                                             {"role": "user", "content": problem}],
                                     pattern="ablsym_solve", node=f"s:{model}", phase="ablsym",
                                     temperature=0.0, max_tokens=_MAX_TOK_SOLVE))
    ans = _parse_num(res.text)
    if ans is not None:                       # no cachear parse-fail (no congelar fallo transitorio)
        _memo_put(k, ans)
    return ans, res.cost_usd


def _verify(model, problem, answer):
    k, hit = _memo_get("ablsym_verify", model, problem, str(answer))
    if hit is not None:
        return bool(hit), 0.0
    art = f"PROBLEMA:\n{problem}\n\nRESPUESTA PROPUESTA: {answer}"
    res = _retry(lambda: _gated_call(model, [{"role": "system", "content": _VERIFY_SYS},
                                             {"role": "user", "content": art}],
                                     pattern="ablsym_verify", node=f"v:{model}", phase="ablsym",
                                     temperature=0.0, max_tokens=_MAX_TOK_VERIFY))
    passed, conf, refs = _parse_verdict(res.text)
    _memo_put(k, passed)
    return passed, res.cost_usd


def _run_item(item: dict):
    """6 calls: 2 generadores + 4 verificaciones (cada respuesta por SELF y CROSS)."""
    try:
        truth = float(item["truth"])
        ans_d, c1 = _solve(FAM_A, item["problem"])
        ans_g, c2 = _solve(FAM_B, item["problem"])
        # autor=deepseek -> SELF=deepseek, CROSS=gemini
        d_self, c3 = _verify(FAM_A, item["problem"], ans_d)
        d_cross, c4 = _verify(FAM_B, item["problem"], ans_d)
        # autor=gemini -> SELF=gemini, CROSS=deepseek
        g_self, c5 = _verify(FAM_B, item["problem"], ans_g)
        g_cross, c6 = _verify(FAM_A, item["problem"], ans_g)
    except Exception:
        return None
    return {**item,
            "ans_d": ans_d, "correct_d": _close(ans_d, truth),
            "ans_g": ans_g, "correct_g": _close(ans_g, truth),
            "d_self": d_self, "d_cross": d_cross,      # verdicts sobre ans_d
            "g_self": g_self, "g_cross": g_cross,      # verdicts sobre ans_g
            "cost": c1 + c2 + c3 + c4 + c5 + c6}


def estimate_cost(n: int) -> float:
    # 6 calls/item: ~120 in, ~560 out cada uno (calibrado, conservador)
    per = 0.0
    for m in (FAM_A, FAM_A, FAM_A, FAM_B, FAM_B, FAM_B):  # 3 calls c/familia por item
        s = spec(m)
        per += 120 / 1e6 * s.price_in + 560 / 1e6 * s.price_out
    return per * n


# --------------------------------------------------------------------------- #
# main                                                                         #
# --------------------------------------------------------------------------- #
def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, default=350)
    ap.add_argument("--seed", type=int, default=42)
    ap.add_argument("--workers", type=int, default=12)  # alto OK: el RateGate limita por RPM
    ap.add_argument("--gemini-rpm", type=float, default=240)   # gemini se espacia a esta tasa
    ap.add_argument("--deepseek-rpm", type=float, default=600)  # deepseek casi sin freno
    ap.add_argument("--fam-b", default="gemini-2.5-flash", help="modelo lado google (flash | flash-lite)")
    ap.add_argument("--no-cache", action="store_true", help="desactiva el memo content-hash")
    ap.add_argument("--yes", action="store_true")
    ap.add_argument("--dry", action="store_true")
    args = ap.parse_args()

    global FAM_B
    FAM_B = args.fam_b
    if family_of(FAM_A) == family_of(FAM_B):
        sys.exit("config rota: FAM_A y FAM_B comparten familia.")
    global _GATE, _MEMO
    _GATE = RateGate({FAM_A: args.deepseek_rpm, FAM_B: args.gemini_rpm})
    if not args.no_cache:
        from mmorch.cache import Memo
        _MEMO = Memo()
        print(f"memo: ON ({len(_MEMO)} entradas previas) -> re-runs identicos = $0")
    gold = build_gold(args.n, args.seed)
    est = estimate_cost(args.n)
    print(f"gold set DURO: {len(gold)} problemas | seed={args.seed}")
    print(f"generadores+verificadores: {FAM_A} ({family_of(FAM_A)}) x {FAM_B} ({family_of(FAM_B)})")
    print(f"calls: {args.n * 6} (6/item: 2 gen + 4 verify) | costo estimado: ${est:.4f}")

    if args.dry:
        print("\n[DRY] muestra:")
        for g in gold[:4]:
            print(f"  i={g['i']} truth={g['truth']} | {g['problem'][:64]}")
        return
    if est > COST_CAP_USD and not args.yes:
        sys.exit(f"costo estimado ${est:.2f} > cap ${COST_CAP_USD}. Repetir con --yes.")
    if not args.yes:
        sys.exit("corrida real gasta API. Repetir con --yes.")

    rows, dropped, cost = [], 0, 0.0
    with ThreadPoolExecutor(max_workers=args.workers) as ex:
        futs = [ex.submit(_run_item, g) for g in gold]
        for j, fut in enumerate(as_completed(futs), 1):
            r = fut.result()
            if r is None:
                dropped += 1
            else:
                rows.append(r); cost += r["cost"]
            if j % 25 == 0:
                print(f"  ... {j}/{len(gold)} (${cost:.3f}, {dropped} caidos)", flush=True)
            if cost > COST_CAP_USD:   # guard runtime: corta si el costo se dispara
                print(f"\nCORTE: costo ${cost:.2f} supero cap ${COST_CAP_USD}. "
                      f"Analizo lo completado ({len(rows)}).", flush=True)
                break
    if not rows:
        sys.exit("todos los items cayeron.")
    if dropped:
        print(f"\nAVISO: {dropped}/{len(gold)} items descartados (fallo API). "
              f"Analisis sobre {len(rows)}.")

    # tasa de error natural de cada generador (cuanto material de blind-spot hay)
    err_d = [r for r in rows if not r["correct_d"]]
    err_g = [r for r in rows if not r["correct_g"]]
    ok_d = [r for r in rows if r["correct_d"]]
    ok_g = [r for r in rows if r["correct_g"]]
    print("\n" + "=" * 64)
    print(f"RESULTADO (n={len(rows)} problemas, costo real ${cost:.4f})")
    print("=" * 64)
    if _MEMO is not None:
        tot = _CACHE["hit"] + _CACHE["miss"]
        print(f"cache: {_CACHE['hit']}/{tot} hits ({100*_CACHE['hit']/max(tot,1):.0f}%) "
              f"-> esas calls costaron $0")
    print(f"error natural del generador: deepseek erro {len(err_d)}/{len(rows)} | "
          f"gemini erro {len(err_g)}/{len(rows)}")

    # --- BLIND-SPOT: sobre respuestas INCORRECTAS, ¿SELF caza menos que CROSS? ---
    # par por artefacto: (self_cazo, cross_cazo). cazar = refuto (passed=False).
    pairs_err = ([(not r["d_self"], not r["d_cross"]) for r in err_d] +   # autor d: self=d, cross=g
                 [(not r["g_self"], not r["g_cross"]) for r in err_g])    # autor g: self=g, cross=d
    n_err = len(pairs_err)
    self_catch = sum(1 for s, c in pairs_err if s)
    cross_catch = sum(1 for s, c in pairs_err if c)
    b = sum(1 for s, c in pairs_err if s and not c)   # solo SELF cazo
    c = sum(1 for s, c in pairs_err if c and not s)    # solo CROSS cazo
    print(f"\n--- BLIND-SPOT (sobre {n_err} respuestas naturalmente INCORRECTAS) ---")
    if n_err:
        print(f"SELF  cazo el error: {self_catch}/{n_err} = {self_catch/n_err:.3f}  "
              f"IC95{_wilson(self_catch, n_err)[1:]}")
        print(f"CROSS cazo el error: {cross_catch}/{n_err} = {cross_catch/n_err:.3f}  "
              f"IC95{_wilson(cross_catch, n_err)[1:]}")
        mc = _mcnemar(b, c)
        print(f"McNemar pareado: b(solo SELF)={b} c(solo CROSS)={c} "
              f"chi2={mc.get('chi2')} p_exact={mc.get('p_exact')}")
        if mc.get("chi2") is not None and mc["p_exact"] < 0.05:
            print(f"  -> SIGNIFICATIVO; cross-family caza {'MAS' if c > b else 'MENOS'} errores. "
                  f"{'TESIS APOYADA' if c > b else 'TESIS CONTRADICHA'}.")
        else:
            print("  -> SIN diferencia significativa: no hay evidencia de blind-spot. "
                  "Escopar el invariante.")
    else:
        print("ningun generador erro -> sin material de blind-spot. Subir dificultad.")

    # --- FALSE-REFUTE: sobre respuestas CORRECTAS, SELF vs CROSS ---
    pairs_ok = ([(not r["d_self"], not r["d_cross"]) for r in ok_d] +
                [(not r["g_self"], not r["g_cross"]) for r in ok_g])
    n_ok = len(pairs_ok)
    self_fr = sum(1 for s, c in pairs_ok if s)
    cross_fr = sum(1 for s, c in pairs_ok if c)
    print(f"\n--- FALSE-REFUTE (sobre {n_ok} respuestas CORRECTAS) ---")
    if n_ok:
        print(f"SELF  rechazo correctas: {self_fr}/{n_ok} = {self_fr/n_ok:.3f}")
        print(f"CROSS rechazo correctas: {cross_fr}/{n_ok} = {cross_fr/n_ok:.3f}")
    print("\nNOTA: pooling balancea calidad-de-verificador (cada familia verifica en SELF "
          "y en CROSS) -> el contraste aisla la RELACION de familia, no que un modelo sea mejor.")


if __name__ == "__main__":
    main()
