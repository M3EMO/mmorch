"""Ablacion §18.4 — run serio. Benchmark PROGRAMATICO con ground-truth deterministico
(no LLM-generado: labels confiables). same-tier (gemini-flash cross vs deepseek-chat
same, ambos non-thinking) para controlar el confound de capacidad. n grande, balanceado
50/50 correcto-vs-error-plantado.

NO usa Date.now/random sin seed: random con seed fijo -> reproducible.
"""
import random
import sys
import pathlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from mmorch.ablation import AblationCase, run_ablation

RNG = random.Random(20260607)


def _cases():
    cases = []
    # 1. Aritmetica (mult/suma/resta). la mitad con error plantado (+/- delta).
    for _ in range(16):
        a, b = RNG.randint(11, 99), RNG.randint(11, 99)
        op = RNG.choice(["*", "+", "-"])
        real = {"*": a * b, "+": a + b, "-": a - b}[op]
        wrong = RNG.random() < 0.5
        shown = real + RNG.choice([-3, -2, -1, 1, 2, 3]) if wrong else real
        cases.append(AblationCase(
            f"Afirmacion: {a} {op} {b} = {shown}.",
            "La afirmacion es aritmeticamente EXACTA? passed=true solo si el resultado es correcto.",
            not wrong, f"arit-{'bug' if wrong else 'ok'}"))
    # 2. Porcentaje.
    for _ in range(10):
        p, y = RNG.choice([10, 20, 25, 50, 5]), RNG.choice([80, 200, 40, 160, 300])
        real = p * y // 100
        wrong = RNG.random() < 0.5
        shown = real + RNG.choice([-5, -2, 2, 5]) if wrong else real
        cases.append(AblationCase(
            f"Afirmacion: el {p}% de {y} es {shown}.",
            "El porcentaje es correcto? passed=true solo si exacto.",
            not wrong, f"pct-{'bug' if wrong else 'ok'}"))
    # 3. Paridad / codigo.
    for _ in range(10):
        n = RNG.randint(2, 200)
        real_par = (n % 2 == 0)
        wrong = RNG.random() < 0.5
        claim_par = (not real_par) if wrong else real_par
        cases.append(AblationCase(
            f"Afirmacion: el numero {n} es {'par' if claim_par else 'impar'}.",
            "La afirmacion sobre paridad es correcta? passed=true solo si correcta.",
            not wrong, f"par-{'bug' if wrong else 'ok'}"))
    # 4. Comparacion de magnitud.
    for _ in range(10):
        a, b = RNG.randint(100, 999), RNG.randint(100, 999)
        if a == b:
            b += 1
        real_gt = a > b
        wrong = RNG.random() < 0.5
        claim_gt = (not real_gt) if wrong else real_gt
        cases.append(AblationCase(
            f"Afirmacion: {a} es {'mayor' if claim_gt else 'menor'} que {b}.",
            "La comparacion es correcta? passed=true solo si correcta.",
            not wrong, f"cmp-{'bug' if wrong else 'ok'}"))
    # 5. Silogismos (logica valida/invalida).
    valid = ("Si todo A es B y todo B es C, entonces todo A es C.", True)
    invalid = ("Si algun A es B y algun B es C, entonces todo A es C.", False)
    invalid2 = ("Si ningun A es B y ningun B es C, entonces todo A es C.", False)
    for _ in range(8):
        art, ok = RNG.choice([valid, invalid, invalid2])
        cases.append(AblationCase(
            f"Afirmacion logica: {art}",
            "El razonamiento logico es VALIDO? passed=true solo si la inferencia es valida.",
            ok, f"logic-{'ok' if ok else 'bug'}"))
    RNG.shuffle(cases)
    return cases


def main():
    cases = _cases()
    n_true = sum(c.truth_passed for c in cases)
    print(f"benchmark: {len(cases)} casos ({n_true} correctos, {len(cases)-n_true} con error)")
    res = run_ablation(cases, ["gemini-2.5-flash", "deepseek-chat"],
                       author_model="deepseek-chat", phase="ablation-serio")
    print(f"author: {res['author_model']} ({res['author_family']})\n")
    print(f"{'verifier':<20} {'fam':<6} {'n':>3} {'acc':>5} {'fp':>3} {'fr':>3} {'cost':>9} {'lat':>6}")
    for c in res["configs"]:
        tag = "CROSS" if c.cross_family else "SAME"
        print(f"{c.verifier_model:<20} {tag:<6} {c.n:>3} {c.accuracy:>5.2f} "
              f"{c.false_pass:>3} {c.false_refute:>3} ${c.cost_usd:>7.4f} {c.lat_avg:>5.1f}s")
    # interpretacion automatica del eje peligroso: false_pass (dejar pasar bug).
    print("\n--- false_pass (dejar pasar bug = error peligroso del verificador) ---")
    for c in res["configs"]:
        n_bugs = sum(1 for bc in c.by_case if not bc["truth"])
        print(f"  {c.verifier_model}: {c.false_pass}/{n_bugs} bugs dejados pasar "
              f"({100*c.false_pass/max(n_bugs,1):.0f}% miss rate)")


if __name__ == "__main__":
    main()
