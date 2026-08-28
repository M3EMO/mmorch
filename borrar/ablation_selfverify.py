"""Ablacion §18.4 — CLOSER: auto-verificacion (self vs cross).

La version fuerte de la tesis cross-family: un modelo no ve SU PROPIO error (blind-spot
generativo). Los runs previos usaron artifacts ajenos -> no testearon esto.

Metodo:
  1. deepseek-chat RESUELVE N problemas duros. Ground-truth COMPUTADA en Python (label
     infalible, no hand-authored).
  2. Se extrae la respuesta numerica de deepseek -> deepseek_correct (vs truth).
  3. Sobre la MISMA (problema + respuesta-de-deepseek), dos verificadores:
       - deepseek-chat  (SELF / same-family): endosa o refuta su propia respuesta.
       - gemini-2.5-flash (CROSS-family).
  4. Sobre los casos donde deepseek ERRO (truth conocida):
       self_catch  = % donde deepseek-verify refuto (cazo su propio error).
       cross_catch = % donde gemini refuto (cazo el error).
     Tesis: cross_catch >> self_catch (decorrelacion). Si self_catch ~ cross_catch ->
     no hay blind-spot de auto-verificacion -> escopar el invariante del todo.

seed fijo, datetime determinista (script normal, no Workflow).
"""
import math
import re
import sys
import pathlib
from datetime import date

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from mmorch.providers import call
from mmorch.patterns import _parse_verdict

def _det3(m):
    a, b, c = m[0]; d, e, f = m[1]; g, h, i = m[2]
    return a*(e*i - f*h) - b*(d*i - f*g) + c*(d*h - e*g)


def _pop_std(xs):
    mu = sum(xs) / len(xs)
    return round((sum((x - mu) ** 2 for x in xs) / len(xs)) ** 0.5, 4)


def _modinv(a, m):
    for x in range(1, m):
        if (a * x) % m == 1:
            return x
    return None


# (texto del problema, truth computada en Python) — labels infalibles. HARD v2.
PROBLEMS = [
    ("Cuantos ceros al final (trailing zeros) tiene 100! (factorial de 100)?",
     sum(100 // 5 ** k for k in range(1, 4))),
    ("Cuanto es 2 elevado a 100, modulo 97?",
     pow(2, 100, 97)),
    ("Cual es la suma de los digitos de 17 elevado a la 19?",
     sum(int(d) for d in str(17 ** 19))),
    ("Cuanto es la suma de TODOS los divisores positivos de 5040 (incluido 1 y 5040)?",
     sum(d for d in range(1, 5041) if 5040 % d == 0)),
    ("Cuanto es el 8vo numero de Catalan, C_8 (con C_0=1)?",
     math.comb(16, 8) // 9),
    ("Determinante de la matriz [[2,5,1],[6,3,7],[4,8,9]]?",
     _det3([[2, 5, 1], [6, 3, 7], [4, 8, 9]])),
    ("Desvio estandar POBLACIONAL de [4,8,15,16,23,42], redondeado a 4 decimales?",
     _pop_std([4, 8, 15, 16, 23, 42])),
    ("Cuantos ceros al final tiene 50! (factorial de 50)?",
     sum(50 // 5 ** k for k in range(1, 3))),
    ("Cuantos enteros entre 1 y 10000 son cuadrados perfectos O cubos perfectos? (inclusion-exclusion)",
     len(set(i*i for i in range(1, 101)) | set(i**3 for i in range(1, 22)))),
    ("Cual es el inverso modular de 7 modulo 26? (numero x con 7x ≡ 1 mod 26)",
     _modinv(7, 26)),
    ("Cuanto es 13 elevado a 11, modulo 1000?",
     pow(13, 11, 1000)),
    ("Cuantas permutaciones distintas tiene la palabra MISSISSIPPI (11 letras)?",
     math.factorial(11) // (math.factorial(4) * math.factorial(4) * math.factorial(2))),
    ("Cuanto es la suma de i al cubo para i de 1 a 15?",
     sum(i ** 3 for i in range(1, 16))),
    ("Cuanto es la combinatoria C(20,10)?",
     math.comb(20, 10)),
    ("Cuantos divisores positivos tiene 1000000 (un millon)?",
     len([d for d in range(1, 1001) if 1000000 % d == 0]) * 2 - (1 if int(1000000**0.5)**2 == 1000000 else 0)),
    ("Un capital de 100 crece 7% anual compuesto durante 5 anios. Valor final redondeado a 2 decimales?",
     round(100 * 1.07 ** 5, 2)),
    ("Cuantos numeros primos hay menores a 100?",
     len([n for n in range(2, 100) if all(n % d for d in range(2, int(n**0.5) + 1))])),
    ("Suma de los primeros 20 numeros triangulares (T_n = n(n+1)/2, de n=1 a 20)?",
     sum(n * (n + 1) // 2 for n in range(1, 21))),
    ("Cuanto es 3 elevado a 15?",
     3 ** 15),
    ("Maximo comun divisor de 1071 y 462 (algoritmo de Euclides)?",
     math.gcd(1071, 462)),
    ("Cuanto es 7! + 5! + 3! (suma de tres factoriales)?",
     math.factorial(7) + math.factorial(5) + math.factorial(3)),
    ("Cuantos enteros entre 1 y 500 son divisibles por 4 pero NO por 6?",
     len([n for n in range(1, 501) if n % 4 == 0 and n % 6 != 0])),
]

_PROBLEMS_OLD = [
    ("Cuantos enteros entre 1 y 1000 inclusive son divisibles por 6, 8 o 15? Inclusion-exclusion.",
     len([n for n in range(1, 1001) if n % 6 == 0 or n % 8 == 0 or n % 15 == 0])),
    ("Cual es la suma de los digitos de 7 elevado a la 13?",
     sum(int(d) for d in str(7 ** 13))),
    ("Cuanto es (2 elevado a 40) modulo 1000?",
     (2 ** 40) % 1000),
    ("Cuantos divisores positivos tiene 360?",
     len([d for d in range(1, 361) if 360 % d == 0])),
    ("Cuanto es el minimo comun multiplo de 18 y 24?",
     math.lcm(18, 24)),
    ("Cuanto es la combinatoria C(12,5) (12 sobre 5)?",
     math.comb(12, 5)),
    ("Cuanto es 10! dividido 7! (factoriales)?",
     math.factorial(10) // math.factorial(7)),
    ("Cuanto es la suma de i al cuadrado para i de 1 a 20?",
     sum(i * i for i in range(1, 21))),
    ("Cuantos dias hay entre el 2026-01-15 y el 2026-09-03 (fecha mayor menos menor)?",
     (date(2026, 9, 3) - date(2026, 1, 15)).days),
    ("Un capital de 1000 sube 12%, luego baja 8%, luego sube 5%. Valor final redondeado a 2 decimales?",
     round(1000 * 1.12 * 0.92 * 1.05, 2)),
    ("Si una cantidad sube 20% y despues baja 20%, cual es el cambio neto porcentual? (ej -4 para -4%)",
     round((1.20 * 0.80 - 1) * 100, 2)),
    ("Cuanto vale el numero binario 101101 en decimal?",
     int("101101", 2)),
    ("Cuanto es la suma de los primeros 30 numeros primos?",
     sum([2,3,5,7,11,13,17,19,23,29,31,37,41,43,47,53,59,61,67,71,73,79,83,89,97,101,103,107,109,113])),
    ("Producto punto de los vectores (3,5,7) y (2,4,6)?",
     3 * 2 + 5 * 4 + 7 * 6),
    ("Area de un circulo de radio 7, usando pi=3.14159, redondeada a 2 decimales?",
     round(3.14159 * 49, 2)),
    ("Cuanto es 47*89 - 1234 + 56*7?",
     47 * 89 - 1234 + 56 * 7),
    ("Cuantos numeros entre 1 y 200 NO son divisibles ni por 3 ni por 5?",
     len([n for n in range(1, 201) if n % 3 != 0 and n % 5 != 0])),
    ("Suma de todos los multiplos de 7 entre 1 y 100?",
     sum(n for n in range(1, 101) if n % 7 == 0)),
    ("Cuanto es 3 elevado a 11?",
     3 ** 11),
    ("Mediana de la lista [12, 7, 3, 22, 9, 15, 1, 18] (8 elementos)?",
     (sorted([12, 7, 3, 22, 9, 15, 1, 18])[3] + sorted([12, 7, 3, 22, 9, 15, 1, 18])[4]) / 2),
    ("Cuanto es la raiz cuadrada de 2025?",
     int(math.isqrt(2025))),
    ("Cuantas veces aparece la letra 'a' en 'abracadabra'?",
     "abracadabra".count("a")),
    ("Si compro a 1500 y vendo a 1875, cual es la ganancia porcentual? (numero, ej 25 para 25%)",
     round((1875 / 1500 - 1) * 100, 2)),
    ("Cuanto es 1 + 2 + 3 + ... + 100?",
     sum(range(1, 101))),
]

_NUM = re.compile(r"RESPUESTA\s*[:=]\s*\$?\s*(-?[\d.,]+)", re.I)


def _parse_num(text):
    m = _NUM.search(text or "")
    if not m:
        # fallback: ultimo numero del texto
        nums = re.findall(r"-?\d[\d.,]*", text or "")
        if not nums:
            return None
        raw = nums[-1]
    else:
        raw = m.group(1)
    raw = raw.strip().rstrip(".").replace(" ", "")
    # heurística separadores: si tiene . y , -> , miles. Si solo , -> decimal.
    if "," in raw and "." in raw:
        raw = raw.replace(",", "")
    elif "," in raw:
        raw = raw.replace(",", ".")
    try:
        return float(raw)
    except ValueError:
        return None


def _close(a, b):
    if a is None or b is None:
        return False
    return abs(a - b) <= max(0.02, abs(b) * 1e-4)


def solve(problem):
    res = call("deepseek-chat",
               [{"role": "system", "content":
                 "Resolve el problema. Razona breve. Terminá con una linea exacta: "
                 "RESPUESTA: <solo el numero final, sin unidades ni separador de miles>."},
                {"role": "user", "content": problem}],
               pattern="selfverify-solve", node="solver", phase="selfverify", temperature=0.0)
    return _parse_num(res.text), res.cost_usd


def verify(model, problem, answer):
    art = f"PROBLEMA:\n{problem}\n\nRESPUESTA PROPUESTA: {answer}"
    res = call(model,
               [{"role": "system", "content":
                 "Verificador adversarial. Resolve el problema vos mismo y compara con la "
                 "respuesta propuesta. Refuta por default. passed=true SOLO si la respuesta "
                 'propuesta es numericamente correcta. JSON: {"passed": bool, "confidence": 0..1, '
                 '"refutations": [string]}'},
                {"role": "user", "content": art}],
               pattern="selfverify-verify", node=f"v:{model}", phase="selfverify", temperature=0.0)
    passed, conf, refs = _parse_verdict(res.text)
    return passed, res.cost_usd


def main():
    cost = 0.0
    rows = []
    for p, truth in PROBLEMS:
        ans, c1 = solve(p)
        cost += c1
        correct = _close(ans, float(truth))
        self_passed, c2 = verify("deepseek-chat", p, ans)      # SELF same-family
        cross_passed, c3 = verify("gemini-2.5-flash", p, ans)  # CROSS
        cost += c2 + c3
        rows.append({"p": p[:50], "truth": truth, "ans": ans, "correct": correct,
                     "self_passed": self_passed, "cross_passed": cross_passed})

    wrong = [r for r in rows if not r["correct"]]
    right = [r for r in rows if r["correct"]]
    n_w = len(wrong)
    print(f"problemas: {len(rows)} | deepseek acerto {len(right)}, erro {n_w}\n")

    if n_w:
        # cazar el error = el verificador refuto (passed=False) una respuesta INCORRECTA.
        self_catch = sum(1 for r in wrong if not r["self_passed"])
        cross_catch = sum(1 for r in wrong if not r["cross_passed"])
        print("=== sobre los casos donde DEEPSEEK ERRO (truth conocida) ===")
        print(f"SELF  (deepseek se auto-verifica): cazo {self_catch}/{n_w} de sus errores "
              f"({100*self_catch/n_w:.0f}%)")
        print(f"CROSS (gemini verifica):           cazo {cross_catch}/{n_w} de los errores "
              f"({100*cross_catch/n_w:.0f}%)")
        print("\n--- detalle de errores de deepseek ---")
        for r in wrong:
            print(f"  truth={r['truth']} ans={r['ans']} | self={'CAZO' if not r['self_passed'] else 'MISS'}"
                  f" cross={'CAZO' if not r['cross_passed'] else 'MISS'} | {r['p']}")
    else:
        print("deepseek no erro ninguno -> sin casos para testear auto-verificacion. "
              "Subir dificultad.")

    if right:
        # false_refute: rechazar una respuesta CORRECTA.
        self_fr = sum(1 for r in right if not r["self_passed"])
        cross_fr = sum(1 for r in right if not r["cross_passed"])
        print(f"\nfalse_refute sobre correctas: self={self_fr}/{len(right)} cross={cross_fr}/{len(right)}")

    print(f"\ncosto total: ${cost:.4f} ({len(rows)*3} calls)")


if __name__ == "__main__":
    main()
