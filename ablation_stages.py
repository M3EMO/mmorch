"""Ablacion de ETAPAS: que le pasa al trabajo BUENO cuando se apilan compuertas.

Pregunta (2026-09-08): un pipeline SDLC agentico apila N compuertas de revision. Si
cada una tiene falso-rechazo p, el trabajo correcto sobrevive (1-p)^N *si los errores
son independientes*. Nadie mide ese "si".

Diseño: cada item del gold set (verdad COMPUTADA, mismo build_gold/seed que
ablation_paired -> comparable) pasa por K verificadores. Se guardan los veredictos POR
ITEM, y las cadenas se computan OFFLINE para cualquier subconjunto: una sola pasada de
API responde todas las combinaciones, presentes y futuras. Los resultados guardados
hasta ahora eran solo agregados, y un agregado no contesta preguntas que no formulaste
antes de correr.

Regla de cadena = UNANIMIDAD: la cadena aprueba solo si aprueban todos (es como se
comporta un pipeline de gates). Entonces:
  - especificidad de la cadena = P(todos aprueban | correcto)   -> cae al apilar
  - sensibilidad de la cadena   = P(alguno rechaza | fallado)   -> sube al apilar
El precio de cazar mas bugs es rechazar mas trabajo bueno. Cuanto cuesta ese precio lo
decide la correlacion entre verificadores.

Lo que esto mide y nunca se midio: si la independencia se cumple.
  esperado_si_independiente = producto de las especificidades marginales
  observado                 = la especificidad real de la cadena
  observado >> esperado  => errores CORRELACIONADOS (se equivocan en los MISMOS items):
                            apilar duele menos que lo que predice el modelo, pero
                            tampoco aporta la cobertura extra que se le supone.
  observado ~= esperado  => independientes => OneFlow/decorrelacion tiene sustento.
Con 2 modelos por familia se contrasta par MISMA-familia vs par CROSS-familia: ese
contraste es el primer test real del invariante OneFlow de GOAL.md. Ojo con el alcance:
sigue siendo dominio checkeable (aritmetica), o sea la rama donde GOAL ya permite
same-family. La rama subjetiva, donde el invariante manda, sigue sin medirse.

Uso: python ablation_stages.py [--n 300] [--seed 42] [--dry]
"""
from __future__ import annotations

import argparse
import itertools
import json
import pathlib
import sys
from concurrent.futures import ThreadPoolExecutor, as_completed

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from ablation_paired import build_gold, _verify, _wilson, _save_result  # noqa: E402
from mmorch.config import family_of  # noqa: E402

# 2 modelos por familia: sin eso no se puede separar "otra familia" de "otro modelo",
# que es el mismo confound que arruino la lectura del 2026-09-04. google y moonshot
# estan sin saldo (429 credits depleted), asi que las familias vivas son deepseek+zhipu.
GATES = ["deepseek-chat", "deepseek-v4-pro", "glm-4.5-air", "glm-5.2"]
# Timeout por llamada. ablation_stages_hard lo sube: sus items tardan 65-75s por gate.
VERIFY_TIMEOUT = 60.0


def _judge_all(item: dict) -> dict | None:
    """Un item por TODOS los gates. None si alguno cae: una cadena a la que le falta un
    gate no es una cadena mas corta, es un dato roto — y sesgaria justo hacia aprobar."""
    out = {"i": item["i"], "is_correct": item["is_correct"]}
    cost = 0.0
    for g in GATES:
        try:
            passed, c = _verify(g, item, timeout=VERIFY_TIMEOUT)
        except Exception:
            return None
        out[g] = bool(passed)
        cost += c
    out["cost"] = cost
    return out


def _marginal(rows, gate):
    corr = [r for r in rows if r["is_correct"]]
    fail = [r for r in rows if not r["is_correct"]]
    spec = sum(1 for r in corr if r[gate]) / len(corr)        # aprueba lo correcto
    sens = sum(1 for r in fail if not r[gate]) / len(fail)    # rechaza lo fallado
    return spec, sens, len(corr), len(fail)


def _chain(rows, gates):
    """Unanimidad: la cadena aprueba solo si aprueban todos los gates."""
    corr = [r for r in rows if r["is_correct"]]
    fail = [r for r in rows if not r["is_correct"]]
    spec = sum(1 for r in corr if all(r[g] for g in gates)) / len(corr)
    sens = sum(1 for r in fail if not all(r[g] for g in gates)) / len(fail)
    return spec, sens


def _phi(rows, g1, g2):
    """Correlacion entre los FALSOS RECHAZOS de dos gates (solo items correctos).
    phi alto = se equivocan en los MISMOS items = redundantes, no decorrelacionados."""
    corr = [r for r in rows if r["is_correct"]]
    a = sum(1 for r in corr if not r[g1] and not r[g2])   # ambos rechazan mal
    b = sum(1 for r in corr if not r[g1] and r[g2])
    c = sum(1 for r in corr if r[g1] and not r[g2])
    d = sum(1 for r in corr if r[g1] and r[g2])
    den = ((a + b) * (c + d) * (a + c) * (b + d)) ** 0.5
    return ((a * d - b * c) / den if den else float("nan")), a


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, default=300)
    ap.add_argument("--seed", type=int, default=42)
    ap.add_argument("--workers", type=int, default=6)
    ap.add_argument("--dry", action="store_true")
    args = ap.parse_args()

    gold = build_gold(args.n, args.seed)
    print(f"gold: {len(gold)} items | gates: {len(GATES)} -> {args.n * len(GATES)} llamadas")
    for g in GATES:
        print(f"  {g:20s} familia={family_of(g)}")
    if args.dry:
        print("[DRY] sin llamadas.")
        return

    rows, fails = [], 0
    with ThreadPoolExecutor(max_workers=args.workers) as ex:
        futs = [ex.submit(_judge_all, it) for it in gold]
        for k, f in enumerate(as_completed(futs), 1):
            r = f.result()
            if r is None:
                fails += 1
            else:
                rows.append(r)
            if k % 50 == 0:
                print(f"  ... {k}/{len(gold)} (${sum(x['cost'] for x in rows):.3f}, "
                      f"{fails} caidos)", flush=True)

    if not rows:
        sys.exit("todos los items cayeron (API caida?). Sin resultado.")
    cost = sum(r["cost"] for r in rows)
    items_path = pathlib.Path(__file__).resolve().parent / "logs" / "ablation_stages_items.jsonl"
    items_path.parent.mkdir(parents=True, exist_ok=True)
    with open(items_path, "a", encoding="utf-8") as fh:
        for r in rows:
            fh.write(json.dumps(r) + "\n")

    bar = "=" * 64
    print(f"\n{bar}\nRESULTADO (n={len(rows)}, {fails} caidos, costo real ${cost:.4f})\n{bar}")

    marg = {}
    print("\nMARGINALES (un solo gate):")
    for g in GATES:
        spec, sens, nc, nf = _marginal(rows, g)
        marg[g] = spec
        print(f"  {g:20s} espec={spec:.3f} {_wilson(round(spec * nc), nc)}  sens={sens:.3f}")

    print("\nCADENAS (unanimidad) — observado vs esperado si fueran INDEPENDIENTES:")
    chains = []
    for k in (2, 3, 4):
        for combo in itertools.combinations(GATES, k):
            spec, sens = _chain(rows, combo)
            pred = 1.0
            for g in combo:
                pred *= marg[g]
            fams = {family_of(g) for g in combo}
            tag = "MISMA-fam" if len(fams) == 1 else f"{len(fams)}-fams"
            print(f"  K={k} {tag:10s} obs={spec:.3f}  indep={pred:.3f}  "
                  f"gap={spec - pred:+.3f}  sens={sens:.3f}  | {'+'.join(combo)}")
            chains.append({"gates": list(combo), "k": k, "familias": len(fams),
                           "espec_obs": spec, "espec_indep": pred, "sens": sens})

    print("\nCORRELACION de falsos rechazos (phi alto = se equivocan en los MISMOS items):")
    phis = []
    for g1, g2 in itertools.combinations(GATES, 2):
        ph, both = _phi(rows, g1, g2)
        tag = "MISMA-fam" if family_of(g1) == family_of(g2) else "CROSS-fam"
        print(f"  {tag}  phi={ph:+.3f}  co-rechazos={both:3d}  | {g1} vs {g2}")
        phis.append({"g1": g1, "g2": g2, "cross_family": family_of(g1) != family_of(g2),
                     "phi": ph, "co_rechazos": both})

    _save_result({
        "experimento": "ablation_stages",
        "pregunta": "apilar compuertas: cuanto trabajo BUENO se destruye, y son "
                    "independientes los errores (test real de OneFlow/decorrelacion)",
        "n": len(rows), "seed": args.seed, "costo_usd": round(cost, 4),
        "gates": GATES, "marginales": marg, "cadenas": chains,
        "phi_falsos_rechazos": phis, "descartados_por_api": fails,
        "items_file": str(items_path),
    })


if __name__ == "__main__":
    main()
