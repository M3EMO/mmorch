"""Ablacion de ETAPAS con items DIFICILES: el follow-up que ablation_stages.py dejo
pendiente.

El run del 2026-09-08 (n=299, aritmetica normal) quedo subpotenciado para medir
decorrelacion: 3 de 4 gates tuvieron 0-2 falsos rechazos sobre 150 items correctos, asi
que no habia nada que correlacionar (phi indefinido en 4 de 6 pares). Ese resultado no
prueba independencia -- prueba que la tarea era facil para 3 de los 4 modelos.

Para medir decorrelacion de verdad hacen falta gates con tasas de error COMPARABLES y
NO TRIVIALES. Este generador escala los mismos 10 tipos de problema de build_gold
(ablation_paired.py) a rangos mucho mas grandes -- exponentes de 3-4 digitos, GCD de
numeros de 6 digitos, C(n,k) con n~80-140, sumas de digitos de potencias enormes,
conteo de primos hasta 3000-8000 -- para que la resolucion mental (la que el prompt
_VERIFY_SYS le pide al verificador: "resolve el problema vos mismo") falle con
frecuencia no despreciable incluso en los modelos que sacaron ~1.0 en la version facil.

n default 1000 (pedido: mas peso estadistico). Mismo protocolo que ablation_stages.py
(veredicto por item, cadenas offline, unanimidad, phi) -- reusa esas funciones, no las
duplica.

Uso: python ablation_stages_hard.py [--n 1000] [--seed 43] [--dry]
"""
from __future__ import annotations

import argparse
import itertools
import json
import math
import pathlib
import random
import re
import sys
from concurrent.futures import ThreadPoolExecutor, as_completed

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from ablation_paired import _wilson, _save_result, _modinv, _verify  # noqa: E402
from mmorch.sandbox import run_sandboxed  # noqa: E402
from ablation_stages import _marginal, _chain, _phi  # noqa: E402
from mmorch.config import family_of  # noqa: E402

# Timeout 180s: medido 2026-09-08, deepseek-chat tarda 67s y glm-5.2 73s por item
# dificil; con 60s toda llamada caia y el piloto corrio 42 min sin un item.

# seed DISTINTO de ablation_paired/ablation_stages (42): este es un gold set nuevo,
# mezclar seeds seria comparar items distintos como si fueran la misma corrida.
DEFAULT_SEED = 43

# Camino B (2026-09-09): gates que FALLAN, dos por familia. Con thinking prendido v4-pro y
# glm-5.2 no cometieron un solo error en 19 items dificiles (piloto n=38): no habia nada
# que correlacionar. Las variantes -nothink estan en config.py solo para esto.
# Medido en el item 0 (seed 43): v4-pro-nothink 8s/113 tok y RECHAZO una respuesta
# correcta; glm-5.2-nothink 5s/11 tok y acerto. Fallan, y baratos.
#
# "code:<modelo>" = el modelo NO calcula: escribe una funcion Python y el oraculo la
# ejecuta (idea del usuario). Mide si dejar que un modelo barato escriba codigo le gana a
# su aritmetica mental. Si la funcion falla o no parsea, el gate REFUTA (cerrado).
HARD_GATES = [
    "deepseek-chat", "deepseek-v4-pro-nothink",
    "glm-4.5-air", "glm-5.2-nothink",
    "code:deepseek-chat", "code:glm-4.5-air",
]
VERIFY_TIMEOUT = 180.0

_CODE_SYS = (
    "Sos un programador. NO resuelvas el problema a mano. Escribi SOLO codigo Python 3 "
    "que defina `def solve() -> int` y devuelva la respuesta exacta al problema. Sin "
    "explicacion, sin markdown, sin prints, sin imports fuera de la stdlib. Solo el codigo."
)


def _family(g: str) -> str:
    return family_of(g.split(":", 1)[1] if ":" in g else g)


def _strip_fence(t: str) -> str:
    m = re.search(r"```(?:python)?\s*(.*?)```", t, re.S)
    return (m.group(1) if m else t).strip()


def _verify_code(model: str, item: dict, timeout: float):
    """El modelo escribe solve(); el sandbox la corre; se compara con lo propuesto."""
    from mmorch.providers import call
    res = call(model, [{"role": "system", "content": _CODE_SYS},
                       {"role": "user", "content": f"PROBLEMA:\n{item['problem']}"}],
               pattern="ablation_stages_hard", node=f"code:{model}",
               phase="ablation_stages_hard", temperature=0.0, timeout=timeout)
    body = _strip_fence(res.text)
    # Medido 2026-09-09 (n=50): los 4 falsos rechazos de code:deepseek-chat fueron
    # FORMATO, no aritmetica -- 2 veces emitio la expresion pelada ("6**37"), 1 vez
    # prosa con el numero correcto, 1 vez repitio el system prompt. Una expresion de
    # una linea se envuelve en solve(); prosa y eco siguen refutados (fail-closed).
    # El texto crudo queda en el row para poder re-analizar sin volver a llamar.
    if "def solve" not in body and "\n" not in body.strip() and body.strip():
        body = f"def solve():\n    return ({body.strip()})"
    code = body + "\n\nprint(solve())\n"
    r = run_sandboxed(code, timeout=20.0)
    raw = res.text[:300]
    if r.timed_out or r.returncode != 0:
        return False, res.cost_usd, raw
    try:
        got = int(r.stdout.strip().splitlines()[-1])
    except (ValueError, IndexError):
        return False, res.cost_usd, raw
    return got == item["proposed"], res.cost_usd, raw


# "synth:<modelo>" (idea del usuario, 2026-09-09): la funcion se sintetiza UNA vez por
# TIPO de problema y despues se aplica en local, sin llamadas. El modelo escribe
# `def solve(problem: str) -> int` que parsea el enunciado y calcula. Antes de confiar
# en ella se PROMUEVE: tiene que acertar la verdad computada en hasta 3 items del mismo
# tipo. Si no, el tipo queda refutado entero (cerrado). Una funcion promovida mal
# envenenaria todos los items de su tipo -- por eso la promocion es contra verdad
# computada y no contra la respuesta propuesta. Es checkers.py escrito por el modelo.
_SYNTH_SYS = (
    "Sos un programador. Escribi SOLO codigo Python 3 que defina "
    "`def solve(problem: str) -> int`. La funcion recibe el ENUNCIADO como texto, extrae "
    "los numeros con re, y devuelve la respuesta exacta. Tiene que funcionar para "
    "cualquier enunciado con la misma forma y otros numeros. Sin explicacion, sin "
    "markdown, solo stdlib."
)
_PROMOTE_K = 3
_synth_cache: dict = {}      # (model, kind) -> codigo fuente promovido, o None si fallo
_synth_rejected: dict = {}   # (model, kind) -> fuente que NO paso la promocion (diagnosis)
_synth_lock = __import__("threading").Lock()
_GOLD_BY_KIND: dict = {}     # kind -> [items con truth], lo llena main()


def _kind(problem: str) -> str:
    return re.sub(r"\d+", "N", problem)


def _run_solve(src: str, problem: str, timeout: float = 20.0):
    """Corre solve(problem) en el sandbox. None si falla o no parsea."""
    # El enunciado va EMBEBIDO como literal JSON (ASCII puro: json.dumps escapa
    # el "≡" como \\u2261). Por stdin, el python del sandbox en Windows lee cp1252 y
    # revienta con ese caracter: las 2 funciones del tipo "inverso modular" fallaron
    # la promocion en AMBOS modelos con phi=+1.0 (run n=50, 2026-09-09). Era el arnes.
    code = (src + "\n\nimport json\n"
            f"print(solve(json.loads({json.dumps(json.dumps(problem))})))\n")
    r = run_sandboxed(code, timeout=timeout)
    if r.timed_out or r.returncode != 0:
        return None
    try:
        return int(r.stdout.strip().splitlines()[-1])
    except (ValueError, IndexError):
        return None


def _verify_synth(model: str, item: dict, timeout: float):
    from mmorch.providers import call
    key = (model, _kind(item["problem"]))
    cost = 0.0
    with _synth_lock:
        if key not in _synth_cache:
            probe = _GOLD_BY_KIND.get(key[1], [item])[:_PROMOTE_K]
            # Hasta 2 intentos por tipo: synth:deepseek-v4-pro devolvio texto VACIO
            # para "suma de digitos" (n=50, 2026-09-09) y el tipo entero quedo
            # refutado por una respuesta en blanco. Un reintento cuesta una llamada
            # por tipo, no por item. Sigue cerrado si los dos fallan.
            src, ok = "", False
            for _attempt in range(2):
                res = call(model, [{"role": "system", "content": _SYNTH_SYS},
                                   {"role": "user", "content": f"PROBLEMA:\n{item['problem']}"}],
                           pattern="ablation_stages_hard", node=f"synth:{model}",
                           phase="ablation_stages_hard", temperature=0.0, timeout=timeout)
                cost += res.cost_usd
                src = _strip_fence(res.text)
                ok = bool(src) and all(_run_solve(src, g["problem"]) == g["truth"] for g in probe)
                if ok:
                    break
            _synth_cache[key] = src if ok else None
            if not ok:
                _synth_rejected[key] = src or "<vacio>"
    src = _synth_cache[key]
    if src is None:
        # guardar la fuente rechazada: sin esto la diagnosis del 2026-09-09 tuvo que
        # adivinar la causa en vez de leerla del row.
        return False, cost, "NO PROMOVIDA: " + _synth_rejected.get(key, "")[:250]
    got = _run_solve(src, item["problem"])
    return got == item["proposed"], cost, "cache"


def _judge_all_hard(item: dict) -> dict | None:
    """Igual que ablation_stages._judge_all pero despacha los gates code:*."""
    out = {"i": item["i"], "is_correct": item["is_correct"]}
    cost = 0.0
    for g in HARD_GATES:
        try:
            if g.startswith("code:"):
                passed, c, raw = _verify_code(g.split(":", 1)[1], item, VERIFY_TIMEOUT)
                out[g + ":raw"] = raw
            elif g.startswith("synth:"):
                passed, c, raw = _verify_synth(g.split(":", 1)[1], item, VERIFY_TIMEOUT)
                out[g + ":raw"] = raw
            else:
                passed, c = _verify(g, item, timeout=VERIFY_TIMEOUT)
        except Exception:
            return None
        out[g] = bool(passed)
        cost += c
    out["cost"] = cost
    return out


def _gen_hard(rng: random.Random):
    """Mismos 10 tipos que build_gold, rangos escalados para que fallar sea comun."""
    kind = rng.randrange(10)
    if kind == 0:
        a, b = rng.randint(23, 97), rng.randint(400, 3000)
        m = rng.choice([1009, 10007, 100003])
        return (f"Cuanto es {a} elevado a {b}, modulo {m}?", pow(a, b, m))
    if kind == 1:
        n = rng.choice([1000, 2500, 5000, 7500, 10000])
        return (f"Cuantos ceros al final tiene {n}! (factorial de {n})?",
                sum(n // 5 ** k for k in range(1, 8)))
    if kind == 2:
        n = rng.choice([27720, 45360, 55440, 83160, 98280])  # altamente compuestos
        return (f"Suma de TODOS los divisores positivos de {n} (incluido 1 y {n})?",
                sum(d for d in range(1, n + 1) if n % d == 0))
    if kind == 3:
        a, b = rng.randint(50_000, 950_000), rng.randint(20_000, 500_000)
        return (f"Maximo comun divisor de {a} y {b} (Euclides)?", math.gcd(a, b))
    if kind == 4:
        n = rng.randint(70, 140)
        k = rng.randint(20, n - 20)
        return (f"Cuanto es la combinatoria C({n},{k})?", math.comb(n, k))
    if kind == 5:
        a, b = rng.randint(30, 90), rng.randint(40, 90)
        return (f"Cual es la suma de los digitos de {a} elevado a la {b}?",
                sum(int(d) for d in str(a ** b)))
    if kind == 6:
        n = rng.choice([3000, 4000, 5500, 7000, 8500])
        return (f"Cuantos numeros primos hay menores a {n}?",
                len([x for x in range(2, n) if all(x % d for d in range(2, int(x**0.5) + 1))]))
    if kind == 7:
        a = rng.choice([37, 41, 43, 53, 59, 61, 67, 71])
        m = rng.choice([1000, 9973, 99991])
        inv = _modinv(a, m)
        if inv is None:
            return (f"Cuanto es {a} elevado a 5?", a ** 5)
        return (f"Cual es el inverso modular de {a} modulo {m}? (x con {a}x ≡ 1 mod {m})", inv)
    if kind == 8:
        n = rng.randint(60, 150)
        return (f"Cuanto es la suma de i al cubo para i de 1 a {n}?",
                sum(i ** 3 for i in range(1, n + 1)))
    n = rng.choice([3, 4, 5, 6, 7, 8, 9])
    e = rng.randint(25, 60)
    return (f"Cuanto es {n} elevado a {e}?", n ** e)


def _perturb_hard(truth: int, rng: random.Random) -> int:
    """Igual criterio que ablation_paired._perturb: error PROPORCIONAL al tamaño del
    numero, para que 'incorrecto' sea plausible y no un typo obvio en numeros grandes."""
    mag = max(1, len(str(abs(truth))) - 1)
    delta = rng.randint(1, 10 ** min(mag, 4))
    return truth + rng.choice([-1, 1]) * delta


def build_gold_hard(n: int, seed: int) -> list[dict]:
    rng = random.Random(seed)
    out = []
    for i in range(n):
        problem, truth = _gen_hard(rng)
        is_correct = rng.random() < 0.5
        proposed = truth if is_correct else _perturb_hard(truth, rng)
        out.append({"i": i, "problem": problem, "truth": truth,
                   "proposed": proposed, "is_correct": is_correct})
    return out


def main():
    global HARD_GATES
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, default=1000)
    ap.add_argument("--seed", type=int, default=DEFAULT_SEED)
    ap.add_argument("--workers", type=int, default=8)
    ap.add_argument("--dry", action="store_true")
    ap.add_argument("--from-items", action="store_true",
                    help="no llama a la API: analiza los items ya guardados de este seed")
    ap.add_argument("--redo", action="store_true",
                    help="ignora los items guardados de este seed+gates y los vuelve a juzgar")
    ap.add_argument("--gates", default=",".join(HARD_GATES),
                    help="lista separada por coma; prefijo code: = el modelo escribe solve()")
    args = ap.parse_args()
    HARD_GATES = [g.strip() for g in args.gates.split(",") if g.strip()]

    items_path = pathlib.Path(__file__).resolve().parent / "logs" / "ablation_stages_hard_items.jsonl"
    items_path.parent.mkdir(parents=True, exist_ok=True)

    gold = build_gold_hard(args.n, args.seed)
    for g in gold:
        _GOLD_BY_KIND.setdefault(_kind(g["problem"]), []).append(g)
    n_c = sum(1 for g in gold if g["is_correct"])
    print(f"gold DIFICIL: {len(gold)} items ({n_c} correctos, {len(gold)-n_c} fallados) | "
          f"seed={args.seed}")
    print(f"gates: {len(HARD_GATES)} -> {args.n * len(HARD_GATES)} llamadas")
    for g in HARD_GATES:
        print(f"  {g:20s} familia={_family(g)}")
    if args.dry:
        print("\n[DRY] muestra de 4 items:")
        for g in gold[:4]:
            print(f"  i={g['i']} ok={g['is_correct']} truth={g['truth']} "
                  f"prop={g['proposed']} | {g['problem'][:70]}")
        return

    # Items ya juzgados de este seed: se saltean. Asi una corrida cortada se RETOMA en
    # vez de repetirse. El piloto del 2026-09-08 murio a los 30/40 con la sesion que lo
    # lanzo y perdio US$0.41 porque los items se escribian al final. Ahora se escriben
    # uno por uno, apenas terminan.
    done = {}
    if items_path.exists() and not args.redo:
        for line in items_path.read_text(encoding="utf-8").splitlines():
            try:
                r = json.loads(line)
            except json.JSONDecodeError:
                continue
            if r.get("seed") == args.seed and r.get("gates") == HARD_GATES:
                done[r["i"]] = r
    rows = [done[g["i"]] for g in gold if g["i"] in done]
    pending = [g for g in gold if g["i"] not in done]
    fails = 0
    if rows:
        print(f"retomando: {len(rows)} items ya guardados, {len(pending)} pendientes")

    if args.from_items:
        if not rows:
            sys.exit("no hay items guardados para este seed.")
    else:
        with ThreadPoolExecutor(max_workers=args.workers) as ex, \
             open(items_path, "a", encoding="utf-8") as fh:
            futs = [ex.submit(_judge_all_hard, it) for it in pending]
            for k, f in enumerate(as_completed(futs), 1):
                r = f.result()
                if r is None:
                    fails += 1
                else:
                    r["seed"] = args.seed
                    r["gates"] = HARD_GATES
                    rows.append(r)
                    fh.write(json.dumps(r) + "\n")
                    fh.flush()
                # cada 10 y no cada 100: con n=60 el piloto anterior no imprimio NADA
                # en 42 min y no habia forma de distinguir "lento" de "colgado".
                if k % 10 == 0:
                    print(f"  ... {k}/{len(pending)} (${sum(x['cost'] for x in rows):.3f}, "
                          f"{fails} caidos)", flush=True)

    if not rows:
        sys.exit("todos los items cayeron (API caida?). Sin resultado.")
    cost = sum(r["cost"] for r in rows)

    bar = "=" * 64
    print(f"\n{bar}\nRESULTADO DIFICIL (n={len(rows)}, {fails} caidos, costo real ${cost:.4f})\n{bar}")

    marg = {}
    print("\nMARGINALES (un solo gate):")
    for g in HARD_GATES:
        spec, sens, nc, nf = _marginal(rows, g)
        marg[g] = spec
        print(f"  {g:20s} espec={spec:.3f} {_wilson(round(spec * nc), nc)}  "
              f"sens={sens:.3f} {_wilson(round(sens * nf), nf)}")

    print("\nCADENAS (unanimidad) — observado vs esperado si fueran INDEPENDIENTES:")
    chains = []
    for k in (2, 3, 4):
        for combo in itertools.combinations(HARD_GATES, k):
            spec, sens = _chain(rows, combo)
            pred = 1.0
            for g in combo:
                pred *= marg[g]
            fams = {_family(g) for g in combo}
            tag = "MISMA-fam" if len(fams) == 1 else f"{len(fams)}-fams"
            print(f"  K={k} {tag:10s} obs={spec:.3f}  indep={pred:.3f}  "
                  f"gap={spec - pred:+.3f}  sens={sens:.3f}  | {'+'.join(combo)}")
            chains.append({"gates": list(combo), "k": k, "familias": len(fams),
                           "espec_obs": spec, "espec_indep": pred, "sens": sens})

    print("\nCORRELACION de falsos rechazos (phi alto = se equivocan en los MISMOS items):")
    phis = []
    for g1, g2 in itertools.combinations(HARD_GATES, 2):
        ph, both = _phi(rows, g1, g2)
        tag = "MISMA-fam" if _family(g1) == _family(g2) else "CROSS-fam"
        print(f"  {tag}  phi={ph:+.3f}  co-rechazos={both:3d}  | {g1} vs {g2}")
        phis.append({"g1": g1, "g2": g2, "cross_family": _family(g1) != _family(g2),
                     "phi": ph, "co_rechazos": both})

    _save_result({
        "experimento": "ablation_stages_hard",
        "pregunta": "igual que ablation_stages pero con items DIFICILES (tasas de "
                    "error comparables y no triviales) -- para poder medir "
                    "decorrelacion de verdad, no una tarea facil disfrazada de test",
        "n": len(rows), "seed": args.seed, "costo_usd": round(cost, 4),
        "gates": HARD_GATES, "marginales": marg, "cadenas": chains,
        "phi_falsos_rechazos": phis, "descartados_por_api": fails,
        "items_file": str(items_path),
    })


if __name__ == "__main__":
    main()
