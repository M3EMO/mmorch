"""Ablacion de PROMPT del verificador — bajar el false-refute sin matar el catch.

Diagnostico (inspeccion manual): el verificador refuta ~74% de respuestas CORRECTAS
en math dura. Causa: skeptic-default ("refuta por default") + el verificador no puede
recomputar el problema -> refuta lo que no puede confirmar. PERO bajar false-refute
sube false-pass (pierde errores). Es precision vs recall. Aca medimos AMBOS por prompt.

Set balanceado: N problemas con ground-truth computada -> caso CORRECTO (proposed=truth)
y caso INCORRECTO (proposed=perturb). Cada caso x cada prompt candidato -> 1 verify.
Metricas por prompt:
  false_refute = refuto / correctas      (queremos BAJO)
  catch        = refuto / incorrectas    (queremos ALTO; = sensibilidad)
  balanced_acc = ((1-false_refute) + catch) / 2

Verificador: flash-lite (mas barato + fue mejor verificador que flash). cap + rate-limit
+ cache. seed fijo, determinista. Uso: python ablation_prompt.py [--n 60] [--yes] [--dry].
"""
from __future__ import annotations

import argparse
import random
import re
import sys
import pathlib
from concurrent.futures import ThreadPoolExecutor, as_completed

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from mmorch.providers import call
from mmorch.patterns import _parse_verdict
from ablation_symmetric import _gen_hard, RateGate
from ablation_paired import _wilson

VERIFIER = "gemini-2.5-flash-lite"
_GATE = RateGate({VERIFIER: 240})


# --- prompts candidatos: name -> (system_text, max_tokens) --- #
PROMPTS = {
    "baseline_skeptic": (
        "Verificador adversarial. Resolve el problema vos mismo y compara con la "
        "respuesta propuesta. Refuta por default. passed=true SOLO si la respuesta "
        'propuesta es numericamente correcta. JSON: {"passed": bool, "confidence": '
        '0..1, "refutations": [string]}', 512),
    "neutral_no_default": (
        "Verificador imparcial. Resolve el problema vos mismo paso a paso. Compara tu "
        "resultado con la respuesta propuesta. passed=true si COINCIDEN. IMPORTANTE: si "
        "NO podes resolverlo con seguridad, passed=true (NO refutes por no poder). Solo "
        'passed=false si tu calculo difiere Y estas seguro. JSON: {"passed": bool, '
        '"confidence": 0..1, "refutations": [string]}', 512),
    "show_work_more_tok": (
        "Resolve el problema paso a paso, MOSTRANDO cada operacion del calculo. Al final "
        "compara tu resultado con la respuesta propuesta y deci si coincide. Termina con "
        'JSON: {"passed": bool, "confidence": 0..1, "refutations": [string]}. '
        "passed=true si tu resultado == el propuesto.", 1536),
    "conf_gated": (
        "Verificador. Resolve el problema y compara con la propuesta. Da passed y "
        "confidence (tu seguridad en el veredicto). REGLA: si tu confianza en una "
        "REFUTACION es < 0.8, poné passed=true (ante la duda, aceptá). "
        'JSON: {"passed": bool, "confidence": 0..1, "refutations": [string]}', 512),
}

_NUM = re.compile(r"-?\d[\d.,]*")


def _perturb(truth: int, rng: random.Random) -> int:
    for _ in range(8):
        cand = truth + rng.choice([-2, -1, 1, 2]) if rng.random() < 0.5 else \
            truth + rng.choice([-1, 1]) * max(1, round(abs(truth) * 0.02))
        if cand != truth and (truth == 0 or (cand >= 0) == (truth >= 0)):
            return int(cand)
    return truth + 1


def build_cases(n: int, seed: int):
    rng = random.Random(seed)
    cases = []
    for i in range(n):
        text, truth = _gen_hard(rng)
        cases.append({"problem": text, "proposed": truth, "is_correct": True})
        cases.append({"problem": text, "proposed": _perturb(truth, rng), "is_correct": False})
    return cases


def _verify(prompt_name, case, retries=3):
    sys_text, max_tok = PROMPTS[prompt_name]
    art = f"PROBLEMA:\n{case['problem']}\n\nRESPUESTA PROPUESTA: {case['proposed']}"
    last = None
    for _ in range(retries + 1):
        try:
            _GATE.reserve(VERIFIER)
            res = call(VERIFIER, [{"role": "system", "content": sys_text},
                                  {"role": "user", "content": art}],
                       pattern="ablprompt", node=prompt_name, phase="ablprompt",
                       temperature=0.0, max_tokens=max_tok)
            passed, conf, refs = _parse_verdict(res.text)
            return {"prompt": prompt_name, "passed": passed,
                    "is_correct": case["is_correct"], "cost": res.cost_usd}
        except Exception as e:
            last = e
    return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, default=60)
    ap.add_argument("--seed", type=int, default=7)
    ap.add_argument("--workers", type=int, default=8)
    ap.add_argument("--yes", action="store_true")
    ap.add_argument("--dry", action="store_true")
    args = ap.parse_args()

    cases = build_cases(args.n, args.seed)
    jobs = [(pn, c) for pn in PROMPTS for c in cases]
    print(f"cases: {len(cases)} ({args.n} correctas + {args.n} incorrectas) | "
          f"prompts: {len(PROMPTS)} | verificador: {VERIFIER}")
    print(f"calls: {len(jobs)} (cap por prompt: {[m for _, m in PROMPTS.values()]})")
    if args.dry:
        print("[DRY] sin API."); return
    if not args.yes:
        sys.exit("corrida real gasta API. --yes para confirmar.")

    rows, cost, dropped = [], 0.0, 0
    with ThreadPoolExecutor(max_workers=args.workers) as ex:
        futs = [ex.submit(_verify, pn, c) for pn, c in jobs]
        for j, fut in enumerate(as_completed(futs), 1):
            r = fut.result()
            if r is None:
                dropped += 1
            else:
                rows.append(r); cost += r["cost"]
            if j % 100 == 0:
                print(f"  ... {j}/{len(jobs)} (${cost:.3f}, {dropped} caidos)")

    print("\n" + "=" * 70)
    print(f"RESULTADO (costo real ${cost:.4f}, {dropped} caidos)")
    print("=" * 70)
    print(f"{'prompt':22} {'false_refute':>14} {'catch':>10} {'bal_acc':>9}")
    best = None
    for pn in PROMPTS:
        rs = [r for r in rows if r["prompt"] == pn]
        corr = [r for r in rs if r["is_correct"]]
        incorr = [r for r in rs if not r["is_correct"]]
        fr = sum(1 for r in corr if not r["passed"]) / max(len(corr), 1)
        catch = sum(1 for r in incorr if not r["passed"]) / max(len(incorr), 1)
        bal = ((1 - fr) + catch) / 2
        frw = _wilson(sum(1 for r in corr if not r["passed"]), len(corr))
        print(f"{pn:22} {fr:>8.3f} IC[{frw[1]:.2f},{frw[2]:.2f}]  {catch:>8.3f} {bal:>9.3f}")
        if best is None or bal > best[1]:
            best = (pn, bal, fr, catch)
    print(f"\nMEJOR balanced_acc: {best[0]} (bal={best[1]:.3f}, "
          f"false_refute={best[2]:.3f}, catch={best[3]:.3f})")
    print("baseline_skeptic es el prompt actual de mmorch. Si otro baja false_refute "
          "manteniendo catch alto -> candidato a reemplazo (gated).")


if __name__ == "__main__":
    main()
