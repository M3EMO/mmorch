"""Decorrelacion de errores de CODIGO: synth por TIPO, unidad de analisis = el tipo.

Pregunta (2026-09-09): cuando dos modelos escriben una funcion para el mismo tipo de
problema, se equivocan en los MISMOS tipos? Eso es decorrelacion de errores de
codigo -- lo que mmorch verifica todos los dias -- y no de aritmetica mental.

Por que tipos y no items: un gate synth sintetiza UNA funcion por tipo y la aplica a
todos sus items. Si la funcion esta mal, falla el tipo entero. La observacion es el
tipo. Con 10 tipos de stdlib (pow, comb, gcd) nadie falla: hacen falta muchos tipos
ALGORITMICOS con riesgo real (off-by-one, casos borde, definicion ambigua).

Diseño:
  - 40 tipos, cada uno con solver de referencia (verdad computada, sin LLM).
  - 10 items por tipo: 3 de PROMOCION (la funcion debe acertar la verdad en los 3)
    y 7 de TEST (mitad con respuesta correcta, mitad perturbada).
  - Cada modelo sintetiza `solve(problem: str) -> int` por tipo, hasta 2 intentos.
  - Falla de tipo = no promovida, O falso rechazo, O bug no cazado en los 7 de test.
  - phi por par de modelos sobre "fallo el tipo" (binario, 40 observaciones).
  - Contraste misma-familia vs cross-familia sobre esos phi.
Persistencia por (modelo, tipo): fuente, promocion, veredictos, costo. Se retoma.

Uso: python ablation_synth_kinds.py [--dry] [--models a,b,c] [--seed 44] [--workers 8]
"""
from __future__ import annotations

import argparse
import itertools
import json
import math
import pathlib
import random
import sys
from concurrent.futures import ThreadPoolExecutor, as_completed
from functools import lru_cache

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from ablation_paired import _wilson, _save_result  # noqa: E402
from ablation_stages_hard import _strip_fence  # noqa: E402
from mmorch import synth_store  # noqa: E402
from mmorch.config import family_of  # noqa: E402
from mmorch.sandbox import run_sandboxed  # noqa: E402

# 45 y no 44: el seed 44 tiene filas guardadas del diseño viejo (extraccion por regex
# del enunciado). Mezclarlas con el diseño por params seria comparar dos experimentos
# distintos como si fueran uno.
DEFAULT_SEED = 45

# La funcion recibe los PARAMETROS como dict, no el enunciado como texto. El piloto del
# 2026-09-09 con extraccion por regex fallo Collatz y Josephus con codigo CORRECTO: el
# enunciado traia "n/2", "3n+1", "1..92", "responder -1", y la regex tomaba esos digitos
# como parametros. Eso es un defecto del enunciado, y correlaciona a los modelos entre
# si por culpa del texto, no del codigo. Con params estructurados se mide SOLO el
# algoritmo -- que es lo que un SDLC real recibe: entradas tipadas, no prosa.
_SYNTH_SYS = (
    "Sos un programador. Escribi SOLO codigo Python 3 que defina "
    "`def solve(params: dict) -> int`. `params` trae los parametros del problema con los "
    "nombres indicados. Devolve la respuesta exacta como int. Tiene que funcionar para "
    "otros valores de los mismos parametros. Sin explicacion, sin markdown, solo stdlib."
)


def _run_solve(src: str, params: dict, timeout: float = 20.0):
    """Corre solve(params) en el sandbox con los params embebidos como literal JSON."""
    # Devuelve (valor o None, segundos de pared). Los segundos incluyen el arranque de
    # python (~0.1-0.3s en Windows): sirven para COMPARAR modelos, no como tiempo
    # absoluto. Timeout = None: codigo correcto pero lento no sirve (LiveCodeBench).
    import time as _t
    code = (src + "\n\nimport json\n"
            f"print(solve(json.loads({json.dumps(json.dumps(params))})))\n")
    t0 = _t.perf_counter()
    r = run_sandboxed(code, timeout=timeout)
    dt = _t.perf_counter() - t0
    if r.timed_out or r.returncode != 0:
        return None, dt
    try:
        return int(r.stdout.strip().splitlines()[-1]), dt
    except (ValueError, IndexError):
        return None, dt
N_PROMOTE, N_TEST = 3, 7
TIMEOUT = 180.0
# 3 por familia: 3 pares misma-familia por lado, 9 cross. Google/Moonshot sin saldo.
DEFAULT_MODELS = ["deepseek-chat", "deepseek-v4-pro", "deepseek-reasoner",
                  "glm-4.5-air", "glm-5.2", "glm-5.2-nothink"]


# --------------------------------------------------------------------------- #
# 40 tipos: (nombre, gen(rng) -> (enunciado, verdad)). Enunciado autocontenido.  #
# --------------------------------------------------------------------------- #
def _sieve(n):
    s = bytearray([1]) * (n + 1); s[0:2] = b"\x00\x00"
    for i in range(2, int(n ** 0.5) + 1):
        if s[i]: s[i * i::i] = bytearray(len(s[i * i::i]))
    return [i for i in range(n + 1) if s[i]]


@lru_cache(None)
def _partitions(n):
    p = [1] + [0] * n
    for k in range(1, n + 1):
        for i in range(k, n + 1):
            p[i] += p[i - k]
    return p[n]


def _josephus(n, k):
    r = 0
    for i in range(2, n + 1):
        r = (r + k) % i
    return r + 1


def _collatz(n):
    c = 0
    while n != 1:
        n = n // 2 if n % 2 == 0 else 3 * n + 1; c += 1
    return c


def _collatz_max(n):
    m = n
    while n != 1:
        n = n // 2 if n % 2 == 0 else 3 * n + 1; m = max(m, n)
    return m


def _phi(n):
    r, m = n, n
    p = 2
    while p * p <= m:
        if m % p == 0:
            while m % p == 0: m //= p
            r -= r // p
        p += 1
    if m > 1: r -= r // m
    return r


def _divs(n): return [d for d in range(1, n + 1) if n % d == 0]
def _derange(n): return round(math.factorial(n) / math.e) if n > 0 else 1
def _stirling2(n, k):
    return sum((-1) ** (k - j) * math.comb(k, j) * j ** n for j in range(k + 1)) // math.factorial(k)
def _bell(n): return sum(_stirling2(n, k) for k in range(n + 1))
def _lis(a):
    import bisect
    t = []
    for x in a:
        i = bisect.bisect_left(t, x)
        if i == len(t): t.append(x)
        else: t[i] = x
    return len(t)
def _kadane(a):
    best = cur = a[0]
    for x in a[1:]:
        cur = max(x, cur + x); best = max(best, cur)
    return best
def _inversions(a): return sum(1 for i in range(len(a)) for j in range(i + 1, len(a)) if a[i] > a[j])
def _pal_substrings(s): return sum(1 for i in range(len(s)) for j in range(i, len(s)) if s[i:j + 1] == s[i:j + 1][::-1])
def _lcs(a, b):
    dp = [[0] * (len(b) + 1) for _ in range(len(a) + 1)]
    for i in range(len(a)):
        for j in range(len(b)):
            dp[i + 1][j + 1] = dp[i][j] + 1 if a[i] == b[j] else max(dp[i][j + 1], dp[i + 1][j])
    return dp[-1][-1]
def _edit(a, b):
    dp = list(range(len(b) + 1))
    for i in range(1, len(a) + 1):
        prev, dp[0] = dp[0], i
        for j in range(1, len(b) + 1):
            prev, dp[j] = dp[j], min(dp[j] + 1, dp[j - 1] + 1, prev + (a[i - 1] != b[j - 1]))
    return dp[-1]
def _knap(items, cap):
    dp = [0] * (cap + 1)
    for w, v in items:
        for c in range(cap, w - 1, -1): dp[c] = max(dp[c], dp[c - w] + v)
    return dp[cap]
def _coin_ways(coins, amt):
    dp = [1] + [0] * amt
    for c in coins:
        for a in range(c, amt + 1): dp[a] += dp[a - c]
    return dp[amt]
def _min_coins(coins, amt):
    dp = [0] + [10 ** 9] * amt
    for a in range(1, amt + 1):
        dp[a] = min([dp[a - c] + 1 for c in coins if c <= a] or [10 ** 9])
    return dp[amt] if dp[amt] < 10 ** 9 else -1
def _tile2n(n):
    a, b = 1, 1  # f(0)=1, f(1)=1, f(n)=f(n-1)+2f(n-2)
    for _ in range(n): a, b = b, b + 2 * a
    return a
def _no11(n):
    a, b = 1, 2
    for _ in range(n - 1): a, b = b, a + b
    return b if n >= 1 else 1
def _r2(n): return sum(1 for x in range(-int(n ** 0.5) - 1, int(n ** 0.5) + 2) for y in range(-int(n ** 0.5) - 1, int(n ** 0.5) + 2) if x * x + y * y == n)
def _prim_pyth(limit):
    c = 0
    for m in range(2, int(limit ** 0.5) + 1):
        for n in range(1, m):
            if (m - n) % 2 == 1 and math.gcd(m, n) == 1 and m * m + n * n <= limit: c += 1
    return c
def _lpf(n):
    p, f = 2, 1
    while p * p <= n:
        while n % p == 0: f, n = p, n // p
        p += 1
    return max(f, n) if n > 1 else f
def _squarefree(n): return sum(1 for k in range(1, n + 1) if all(k % (p * p) for p in range(2, int(k ** 0.5) + 1)))
def _euclid_steps(a, b):
    c = 0
    while b: a, b = b, a % b; c += 1
    return c
def _subsets_div(a, k, d): return sum(1 for c in itertools.combinations(a, k) if sum(c) % d == 0)
def _grid_avoid(r, c, bx, by):
    dp = [[0] * (c + 1) for _ in range(r + 1)]; dp[0][0] = 1
    for i in range(r + 1):
        for j in range(c + 1):
            if (i, j) == (bx, by): dp[i][j] = 0; continue
            if i: dp[i][j] += dp[i - 1][j]
            if j: dp[i][j] += dp[i][j - 1]
    return dp[r][c]
def _digit_sum_count(n, s):
    return sum(1 for x in range(10 ** (n - 1), 10 ** n) if sum(map(int, str(x))) == s)
def _happy_count(n):
    def happy(x):
        seen = set()
        while x != 1 and x not in seen:
            seen.add(x); x = sum(int(d) ** 2 for d in str(x))
        return x == 1
    return sum(1 for x in range(1, n + 1) if happy(x))
def _ulam_like_perfect(n): return sum(1 for k in range(2, n + 1) if sum(_divs(k)) - k == k)
def _look_say_len(n):
    s = "1"
    for _ in range(n - 1):
        out, i = [], 0
        while i < len(s):
            j = i
            while j < len(s) and s[j] == s[i]: j += 1
            out.append(f"{j - i}{s[i]}"); i = j
        s = "".join(out)
    return len(s)
def _kaprekar_steps(n):
    c = 0
    while n != 6174 and c < 20:
        d = f"{n:04d}"; n = int("".join(sorted(d, reverse=True))) - int("".join(sorted(d))); c += 1
    return c


def _pf(x):
    p, out = 2, []
    while p * p <= x:
        while x % p == 0: out.append(p); x //= p
        p += 1
    if x > 1: out.append(x)
    return out




def _zeros_base(n, b):
    facs = {}
    for p in _pf(b): facs[p] = facs.get(p, 0) + 1
    return min(sum(n // p ** k for k in range(1, 40)) // e for p, e in facs.items())


def _fib_mod(n, m):
    a, b = 0, 1
    for _ in range(n - 1): a, b = b, (a + b) % m
    return b


def _lst(rng, k, lo, hi): return [rng.randint(lo, hi) for _ in range(k)]


KINDS = [
    ('particiones', lambda r: (lambda n: (f'Cuantas particiones enteras tiene {n}? (formas de escribir {n} como suma de enteros positivos sin importar el orden)', _partitions(n), {'n': n}))(r.randint(30, 80))),
    ('josephus', lambda r: (lambda n, k: (f'Problema de Josefo: {n} personas en circulo numeradas 1..{n}; se elimina cada {k}-esima persona empezando a contar desde la 1. Que numero tiene la sobreviviente?', _josephus(n, k), {'n': n, 'k': k}))(r.randint(20, 200), r.randint(2, 9))),
    ('collatz_pasos', lambda r: (lambda n: (f'Cuantos pasos de Collatz (n par -> n/2, n impar -> 3n+1) necesita {n} para llegar a 1?', _collatz(n), {'n': n}))(r.randint(1000, 99999))),
    ('collatz_max', lambda r: (lambda n: (f'Cual es el valor MAXIMO alcanzado en la secuencia de Collatz que empieza en {n} (incluido {n})?', _collatz_max(n), {'n': n}))(r.randint(1000, 99999))),
    ('primos_rango', lambda r: (lambda a, b: (f'Cuantos numeros primos hay en el intervalo cerrado [{a}, {b}]?', sum((1 for p in _sieve(b) if p >= a)), {'a': a, 'b': b}))(r.randint(1000, 5000), r.randint(6000, 20000))),
    ('nesimo_primo', lambda r: (lambda n: (f'Cual es el {n}-esimo numero primo? (el 1-esimo es 2)', _sieve(200000)[n - 1], {'n': n}))(r.randint(500, 5000))),
    ('suma_div_propios', lambda r: (lambda n: (f'Suma de los divisores PROPIOS de {n} (todos los divisores menos {n} mismo)?', sum(_divs(n)) - n, {'n': n}))(r.randint(10000, 99999))),
    ('cant_divisores', lambda r: (lambda n: (f'Cuantos divisores positivos tiene {n}?', len(_divs(n)), {'n': n}))(r.randint(10000, 99999))),
    ('euler_phi', lambda r: (lambda n: (f'Cuanto vale la funcion phi de Euler de {n} (cantidad de enteros en 1..{n} coprimos con {n})?', _phi(n), {'n': n}))(r.randint(10000, 99999))),
    ('fibonacci_mod', lambda r: (lambda n, m: (f'Cual es el {n}-esimo numero de Fibonacci modulo {m}? (F(1)=1, F(2)=1)', _fib_mod(n, m), {'n': n, 'm': m}))(r.randint(1000, 100000), r.choice([1000, 9973, 1000003]))),
    ('catalan', lambda r: (lambda n: (f'Cual es el {n}-esimo numero de Catalan? (C(0)=1, C(1)=1, C(2)=2)', math.comb(2 * n, n) // (n + 1), {'n': n}))(r.randint(15, 40))),
    ('desarreglos', lambda r: (lambda n: (f'Cuantos desarreglos (permutaciones sin puntos fijos) tiene un conjunto de {n} elementos?', _derange(n), {'n': n}))(r.randint(8, 18))),
    ('stirling2', lambda r: (lambda n, k: (f'Numero de Stirling de segunda especie S({n},{k}): formas de partir {n} elementos en {k} bloques no vacios?', _stirling2(n, k), {'n': n, 'k': k}))(r.randint(10, 20), r.randint(3, 8))),
    ('bell', lambda r: (lambda n: (f'Cual es el {n}-esimo numero de Bell (cantidad de particiones de un conjunto de {n} elementos)?', _bell(n), {'n': n}))(r.randint(8, 18))),
    ('digitos_factorial', lambda r: (lambda n: (f'Cual es la suma de los digitos decimales de {n}! (factorial de {n})?', sum(map(int, str(math.factorial(n)))), {'n': n}))(r.randint(100, 400))),
    ('ceros_base', lambda r: (lambda n, b: (f'Cuantos ceros al final tiene {n}! escrito en base {b}?', _zeros_base(n, b), {'n': n, 'b': b}))(r.randint(50, 500), r.choice([6, 12, 15]))),
    ('popcount_total', lambda r: (lambda n: (f'Cuantos bits en 1 hay en total en las representaciones binarias de todos los enteros de 1 a {n}?', sum((bin(i).count('1') for i in range(1, n + 1))), {'n': n}))(r.randint(1000, 50000))),
    ('lis', lambda r: (lambda a: (f'Longitud de la subsecuencia estrictamente creciente mas larga de la lista {a}?', _lis(a), {'a': a}))(_lst(r, 14, 1, 60))),
    ('kadane', lambda r: (lambda a: (f'Suma maxima de un subarreglo contiguo no vacio de {a}?', _kadane(a), {'a': a}))(_lst(r, 12, -30, 30))),
    ('inversiones', lambda r: (lambda a: (f'Cuantas inversiones (pares i<j con a[i]>a[j]) tiene la lista {a}?', _inversions(a), {'a': a}))(_lst(r, 14, 1, 99))),
    ('palindromos', lambda r: (lambda s: (f"Cuantas subcadenas (contiguas, contando posiciones distintas) palindromas tiene la cadena de digitos '{s}'?", _pal_substrings(s), {'s': s}))(''.join((str(r.randint(0, 3)) for _ in range(14))))),
    ('lcs', lambda r: (lambda a, b: (f"Longitud de la subsecuencia comun mas larga (no necesariamente contigua) entre '{a}' y '{b}'?", _lcs(a, b), {'a': a, 'b': b}))(''.join((r.choice('abcd') for _ in range(12))), ''.join((r.choice('abcd') for _ in range(12))))),
    ('edit_distance', lambda r: (lambda a, b: (f"Distancia de edicion de Levenshtein (insertar, borrar, sustituir, costo 1 cada una) entre '{a}' y '{b}'?", _edit(a, b), {'a': a, 'b': b}))(''.join((r.choice('abcde') for _ in range(10))), ''.join((r.choice('abcde') for _ in range(10))))),
    ('mochila', lambda r: (lambda items, cap: (f'Mochila 0/1: items (peso, valor) = {items}, capacidad {cap}. Valor maximo alcanzable?', _knap(items, cap), {'items': items, 'cap': cap}))([(r.randint(1, 15), r.randint(1, 40)) for _ in range(7)], r.randint(20, 40))),
    ('cambio_formas', lambda r: (lambda coins, amt: (f'De cuantas formas se puede formar {amt} con monedas de valores {coins} (cantidad ilimitada, el orden no importa)?', _coin_ways(coins, amt), {'coins': coins, 'amt': amt}))(sorted(r.sample([1, 2, 3, 5, 7, 10, 20, 25], 4)), r.randint(50, 200))),
    ('cambio_min', lambda r: (lambda coins, amt: (f'Cantidad MINIMA de monedas para formar {amt} con valores {coins} (ilimitadas)? Responder -1 si es imposible.', _min_coins(coins, amt), {'coins': coins, 'amt': amt}))(sorted(r.sample([3, 4, 7, 9, 11, 13, 17], 3)), r.randint(30, 150))),
    ('tiling_2xn', lambda r: (lambda n: (f'De cuantas formas se puede cubrir un tablero de 2x{n} con fichas de 1x2 (en cualquier orientacion) y de 2x2?', _tile2n(n), {'n': n}))(r.randint(10, 40))),
    ('binarias_sin_11', lambda r: (lambda n: (f'Cuantas cadenas binarias de longitud {n} NO tienen dos 1 consecutivos?', _no11(n), {'n': n}))(r.randint(10, 60))),
    ('digitos_2n', lambda r: (lambda n: (f'Cual es la suma de los digitos decimales de 2 elevado a {n}?', sum(map(int, str(2 ** n))), {'n': n}))(r.randint(200, 2000))),
    ('ndigitos_suma', lambda r: (lambda n, s: (f'Cuantos numeros de exactamente {n} digitos (sin ceros a la izquierda) tienen suma de digitos igual a {s}?', _digit_sum_count(n, s), {'n': n, 's': s}))(r.randint(3, 5), r.randint(8, 30))),
    ('lcm_1n', lambda r: (lambda n: (f'Cual es el minimo comun multiplo de todos los enteros de 1 a {n}?', math.lcm(*range(1, n + 1)), {'n': n}))(r.randint(15, 40))),
    ('puntos_circulo', lambda r: (lambda n: (f'Cuantos puntos enteros (x, y) cumplen x^2 + y^2 = {n}?', _r2(n), {'n': n}))(r.choice([25, 50, 65, 125, 325, 1105, 5525, 8125, 4225, 2450]))),
    ('pitagoricas_prim', lambda r: (lambda n: (f'Cuantas ternas pitagoricas PRIMITIVAS (a<b<c, gcd(a,b,c)=1) tienen hipotenusa c <= {n}?', _prim_pyth(n), {'n': n}))(r.randint(100, 2000))),
    ('suma_primos', lambda r: (lambda n: (f'Cual es la suma de los primeros {n} numeros primos?', sum(_sieve(200000)[:n]), {'n': n}))(r.randint(100, 3000))),
    ('digitos_factorial_cant', lambda r: (lambda n: (f'Cuantos digitos decimales tiene {n}! (factorial de {n})?', len(str(math.factorial(n))), {'n': n}))(r.randint(100, 1500))),
    ('mayor_primo', lambda r: (lambda n: (f'Cual es el mayor factor primo de {n}?', _lpf(n), {'n': n}))(r.randint(100000, 9999999))),
    ('libres_cuadrados', lambda r: (lambda n: (f'Cuantos enteros en 1..{n} son libres de cuadrados (no divisibles por ningun cuadrado perfecto > 1)?', _squarefree(n), {'n': n}))(r.randint(500, 5000))),
    ('pasos_euclides', lambda r: (lambda a, b: (f'Cuantas divisiones (pasos) hace el algoritmo de Euclides para gcd({a}, {b})? Contar una por cada operacion modulo hasta que el resto sea 0.', _euclid_steps(a, b), {'a': a, 'b': b}))(r.randint(10000, 999999), r.randint(1000, 99999))),
    ('subconjuntos_div', lambda r: (lambda a, k, d: (f'Cuantos subconjuntos de exactamente {k} elementos de {a} tienen suma divisible por {d}?', _subsets_div(a, k, d), {'a': a, 'k': k, 'd': d}))(_lst(r, 12, 1, 50), r.randint(3, 5), r.randint(3, 7))),
    ('grilla_obstaculo', lambda r: (lambda R, C, bx, by: (f'Caminos de (0,0) a ({R},{C}) moviendo solo +1 en x o +1 en y, sin pasar por la celda ({bx},{by})?', _grid_avoid(R, C, bx, by), {'R': R, 'C': C, 'bx': bx, 'by': by}))(r.randint(6, 14), r.randint(6, 14), r.randint(1, 5), r.randint(1, 5))),
    ('felices', lambda r: (lambda n: (f"Cuantos numeros felices hay en 1..{n}? (un numero es feliz si iterar 'suma de cuadrados de sus digitos' llega a 1)", _happy_count(n), {'n': n}))(r.randint(500, 5000))),
    ('look_say', lambda r: (lambda n: (f"Longitud del {n}-esimo termino de la secuencia look-and-say que empieza en '1' (termino 1 = '1', termino 2 = '11', termino 3 = '21')?", _look_say_len(n), {'n': n}))(r.randint(15, 35))),
    ('kaprekar', lambda r: (lambda n: (f'Cuantas iteraciones de la rutina de Kaprekar (ordenar digitos desc menos asc, con 4 digitos y ceros a la izquierda) necesita {n} para llegar a 6174?', _kaprekar_steps(n), {'n': n}))(r.choice([x for x in range(1000, 9999) if len(set(f'{x:04d}')) > 1]))),
]


assert len(KINDS) == 43 and len({k for k, _ in KINDS}) == 43

# --------------------------------------------------------------------------- #
# Tipos DIFICILES (2026-09-10). Fuente: EvalPlus/HumanEval+ -- los tests de borde     #
# tumban 10-29 puntos de pass@1; LiveCodeBench -- limites de tiempo. Aca cada tipo   #
# tiene una trampa de spec conocida: indexado 0/1, bordes inclusivos, negativos,   #
# strings vacios o iguales, y rangos donde una solucion O(n^2) se pasa de los 20s.  #
# --------------------------------------------------------------------------- #
def _incl_excl(a, b, p, q):
    f = lambda x, d: x // d
    lcm = p * q // math.gcd(p, q)
    return (f(b, p) - f(a - 1, p)) + (f(b, q) - f(a - 1, q)) - (f(b, lcm) - f(a - 1, lcm))
def _kth_perm(n, k):
    """k-esima permutacion de 0..n-1 en orden lexicografico, k 1-INDEXADO, como entero
    de concatenar los digitos (n<=9)."""
    digs = list(range(n)); k -= 1; out = []
    for i in range(n, 0, -1):
        f = math.factorial(i - 1); out.append(digs.pop(k // f)); k %= f
    return int("".join(map(str, out)))
def _max_prod(a):
    best = hi = lo = a[0]
    for x in a[1:]:
        hi, lo = max(x, hi * x, lo * x), min(x, hi * x, lo * x); best = max(best, hi)
    return best
def _islands(rows):
    seen = set(); c = 0
    for i, r in enumerate(rows):
        for j, ch in enumerate(r):
            if ch == "1" and (i, j) not in seen:
                c += 1; st = [(i, j)]
                while st:
                    x, y = st.pop()
                    if (x, y) in seen or x < 0 or y < 0 or x >= len(rows) or y >= len(r) or rows[x][y] != "1": continue
                    seen.add((x, y)); st += [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)]
    return c
def _distinct_substr(s): return len({s[i:j] for i in range(len(s)) for j in range(i + 1, len(s) + 1)})
def _stairs123(n):
    a, b, c = 1, 1, 2  # f(0)=1 f(1)=1 f(2)=2
    for _ in range(n - 2): a, b, c = b, c, a + b + c
    return c if n >= 2 else (1 if n <= 1 else c)
def _triples_inc(a): return sum(1 for i in range(len(a)) for j in range(i + 1, len(a)) for k in range(j + 1, len(a)) if a[i] < a[j] < a[k])
def _pairs_divk(a, k): return sum(1 for i in range(len(a)) for j in range(i + 1, len(a)) if (a[i] + a[j]) % k == 0)
def _bin_pal(n): return sum(1 for x in range(1, n + 1) if bin(x)[2:] == bin(x)[2:][::-1])
def _no_ab(n):
    # cadenas sobre {a,b,c} de longitud n sin "ab": estados = ultimo char es 'a' o no
    ea, no = 1, 2  # n=1: 'a' | 'b','c'
    for _ in range(n - 1): ea, no = ea + no, 2 * ea + 2 * no - ea  # 'a' desde todo; 'b' no desde 'a'; 'c' desde todo
    return ea + no if n >= 1 else 1
def _digits_sum_range(n): return sum(sum(map(int, str(x))) for x in range(1, n + 1))
def _strict_inc_digits(n): return sum(1 for x in range(1, n + 1) if all(a < b for a, b in zip(str(x), str(x)[1:], strict=False)))
def _lps(s):
    n = len(s); dp = [[0] * n for _ in range(n)]
    for i in range(n - 1, -1, -1):
        dp[i][i] = 1
        for j in range(i + 1, n):
            dp[i][j] = dp[i + 1][j - 1] + 2 if s[i] == s[j] else max(dp[i + 1][j], dp[i][j - 1])
    return dp[0][n - 1]
def _sqfree_fast(n):
    ok = bytearray([1]) * (n + 1)
    for p in range(2, int(n ** 0.5) + 1):
        for m in range(p * p, n + 1, p * p): ok[m] = 0
    return sum(ok[1:])


KINDS += [
    ("divisibles_p_o_q", lambda r: (lambda a, b, p, q: (f"Cuantos enteros en el intervalo CERRADO [{a}, {b}] son divisibles por {p} o por {q}?", _incl_excl(a, b, p, q), {"a": a, "b": b, "p": p, "q": q}))(r.randint(1, 500), r.randint(10_000, 10_000_000), r.randint(2, 30), r.randint(2, 30))),
    ("kesima_permutacion", lambda r: (lambda n, k: (f"Permutaciones de los digitos 0..{n-1} en orden lexicografico. La k-esima con k=1 es 01..{n-1}. Cual es la {k}-esima, como entero (concatenar digitos)?", _kth_perm(n, k), {"n": n, "k": k}))(r.randint(5, 9), r.randint(1, 120))),
    ("max_producto", lambda r: (lambda a: (f"Producto maximo de un subarreglo contiguo no vacio de {a}?", _max_prod(a), {"a": a}))([r.choice([-3, -2, -1, 0, 1, 2, 3]) for _ in range(r.randint(6, 12))])),
    ("islas", lambda r: (lambda g: (f"Cuantas islas (componentes conexas de '1' en 4 direcciones) hay en la grilla {g}?", _islands(g), {"g": g}))(["".join(r.choice("0011") for _ in range(8)) for _ in range(6)])),
    ("subcadenas_distintas", lambda r: (lambda s: (f"Cuantas subcadenas DISTINTAS no vacias tiene '{s}'?", _distinct_substr(s), {"s": s}))("".join(r.choice("ab") for _ in range(r.randint(8, 16))))),
    ("escalones_123", lambda r: (lambda n: (f"De cuantas formas se suben {n} escalones con pasos de 1, 2 o 3 (el orden importa)?", _stairs123(n), {"n": n}))(r.randint(5, 40))),
    ("ternas_crecientes", lambda r: (lambda a: (f"Cuantas ternas de indices i<j<k cumplen a[i]<a[j]<a[k] en {a}?", _triples_inc(a), {"a": a}))(_lst(r, r.randint(8, 14), 1, 20))),
    ("pares_div_k", lambda r: (lambda a, k: (f"Cuantos pares de indices i<j tienen a[i]+a[j] divisible por {k} en {a}?", _pairs_divk(a, k), {"a": a, "k": k}))(_lst(r, r.randint(10, 16), 0, 50), r.randint(2, 7))),
    ("binarios_palindromos", lambda r: (lambda n: (f"Cuantos enteros en 1..{n} tienen representacion binaria palindroma (sin ceros a la izquierda)?", _bin_pal(n), {"n": n}))(r.randint(1000, 200000))),
    ("cadenas_sin_ab", lambda r: (lambda n: (f"Cuantas cadenas de longitud {n} sobre el alfabeto {{a,b,c}} NO contienen 'ab' como subcadena contigua?", _no_ab(n), {"n": n}))(r.randint(3, 30))),
    ("suma_digitos_rango", lambda r: (lambda n: (f"Cual es la suma de los digitos de TODOS los enteros de 1 a {n}?", _digits_sum_range(n), {"n": n}))(r.randint(1000, 300000))),
    ("digitos_crecientes", lambda r: (lambda n: (f"Cuantos enteros en 1..{n} tienen digitos ESTRICTAMENTE crecientes de izquierda a derecha (los de un digito cuentan)?", _strict_inc_digits(n), {"n": n}))(r.randint(1000, 500000))),
    ("subsecuencia_palindroma", lambda r: (lambda s: (f"Longitud de la subsecuencia palindroma mas larga (no necesariamente contigua) de '{s}'?", _lps(s), {"s": s}))("".join(r.choice("abc") for _ in range(r.randint(8, 16))))),
    ("libres_cuadrados_grande", lambda r: (lambda n: (f"Cuantos enteros en 1..{n} son libres de cuadrados? (n grande: una solucion lenta no termina)", _sqfree_fast(n), {"n": n}))(r.randint(200_000, 2_000_000))),
]

assert len(KINDS) == 57 and len({k for k, _ in KINDS}) == 57



def _perturb(truth, rng):
    mag = max(1, len(str(abs(truth))) - 1)
    d = rng.randint(1, 10 ** min(mag, 3))
    return truth + rng.choice([-1, 1]) * d


class _MinRng:
    """RNG que devuelve siempre el valor MINIMO legal: genera la instancia de borde de
    cada tipo con el mismo generador, sin escribir 43 casos a mano. Los bugs de codigo
    viven en los bordes (n=0/1, lista de un valor repetido, k minimo): el bug de
    'desarreglos' del piloto (D(0)/D(1) invertidos) pasaba la promocion con n en 8..18
    y se atrapa con n=8 solo si la recurrencia arranca mal -- con n minimo se atrapa
    siempre. Es la idea de EvalPlus: los tests de borde tumban codigo que los tests
    aleatorios de rango medio dejan pasar."""
    def randint(self, a, b): return a
    def choice(self, seq): return seq[0]
    def sample(self, pop, k): return list(pop)[:k]
    def random(self): return 0.0


def build_kind_items(seed, n_test=N_TEST):
    """kind -> {"promote": [items con truth], "test": [items con proposed/is_correct]}.
    promote = N_PROMOTE aleatorios + 1 de BORDE (instancia minima del tipo)."""
    rng = random.Random(seed)
    out = {}
    for name, gen in KINDS:
        items = []
        for i in range(N_PROMOTE + n_test):
            problem, truth, params = gen(rng)
            items.append({"i": i, "problem": problem, "truth": truth, "params": params})
        for it in items[N_PROMOTE:]:
            it["is_correct"] = rng.random() < 0.5
            it["proposed"] = it["truth"] if it["is_correct"] else _perturb(it["truth"], rng)
        problem, truth, params = gen(_MinRng())
        edge = {"i": -1, "problem": problem, "truth": truth, "params": params, "edge": True}
        out[name] = {"promote": items[:N_PROMOTE] + [edge], "test": items[N_PROMOTE:]}
    return out


def _synth_kind(model, name, bundle):
    """Sintetiza (<=2 intentos), promueve, evalua los 7 de test. Devuelve un row."""
    from mmorch.providers import call
    cost, src, ok, attempts = 0.0, "", False, 0
    for _ in range(2):
        attempts += 1
        p0 = bundle["promote"][0]
        user = (f"PROBLEMA:\n{p0['problem']}\n\n"
                f"PARAMS (ejemplo, con estos nombres): {json.dumps(p0['params'], ensure_ascii=False)}")
        res = call(model, [{"role": "system", "content": _SYNTH_SYS},
                           {"role": "user", "content": user}],
                   pattern="ablation_synth_kinds", node=f"synth:{model}",
                   phase="ablation_synth_kinds", temperature=0.0, timeout=TIMEOUT)
        cost += res.cost_usd
        src = _strip_fence(res.text)
        ok = bool(src) and all(_run_solve(src, g["params"])[0] == g["truth"] for g in bundle["promote"])
        if ok:
            break
    row = {"model": model, "kind": name, "promoted": ok, "attempts": attempts,
           "cost": cost, "src": src[:1500], "test": []}
    for it in bundle["test"]:
        got, dt = _run_solve(src, it["params"]) if ok else (None, 0.0)
        passed = (got == it["proposed"]) if ok else False
        row["test"].append({"i": it["i"], "is_correct": it["is_correct"], "passed": passed,
                            "got": got, "truth": it["truth"], "proposed": it["proposed"],
                            "t": round(dt, 3)})
    fr = sum(1 for t in row["test"] if t["is_correct"] and not t["passed"])
    mb = sum(1 for t in row["test"] if not t["is_correct"] and t["passed"])
    row["false_rejects"], row["missed_bugs"] = fr, mb
    row["kind_failed"] = (not ok) or fr > 0 or mb > 0
    # El experimento SOLO ESCRIBE en el store (nunca lee: cada modelo tiene que
    # sintetizar por su cuenta o la medicion no es por modelo). Un gate de produccion
    # lee primero y paga solo si el tipo es nuevo -- synth_store.get/run.
    if ok and not row["kind_failed"]:
        row["stored"] = synth_store.put(name, src, model, {
            "n_promote": len(bundle["promote"]), "edge": True,
            "n_test": len(row["test"]), "n_test_ok": len(row["test"]),
        })
    return row


def _phi_bin(xs, ys):
    a = sum(1 for x, y in zip(xs, ys, strict=True) if x and y); b = sum(1 for x, y in zip(xs, ys, strict=True) if x and not y)
    c = sum(1 for x, y in zip(xs, ys, strict=True) if not x and y); d = sum(1 for x, y in zip(xs, ys, strict=True) if not x and not y)
    den = ((a + b) * (c + d) * (a + c) * (b + d)) ** 0.5
    return ((a * d - b * c) / den if den else float("nan")), a


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--seed", type=int, default=DEFAULT_SEED)
    ap.add_argument("--models", default=",".join(DEFAULT_MODELS))
    ap.add_argument("--workers", type=int, default=8)
    ap.add_argument("--dry", action="store_true")
    ap.add_argument("--analyze-only", action="store_true",
                    help="no llama a la API: analiza las filas guardadas de este seed")
    ap.add_argument("--n-test", type=int, default=N_TEST,
                    help="items de test por tipo (default 7); 17 -> ~1000 items en 57 tipos")
    args = ap.parse_args()
    models = [m.strip() for m in args.models.split(",") if m.strip()]
    kinds = build_kind_items(args.seed, args.n_test)
    print(f"tipos: {len(kinds)} | items por tipo: {N_PROMOTE}+1 borde promocion + {args.n_test} test | seed={args.seed}")
    print(f"modelos: {len(models)} -> {len(kinds) * len(models)} sintesis (x2 si reintenta)")
    for m in models:
        print(f"  {m:20s} familia={family_of(m)}")
    if args.dry:
        print("\n[DRY] un item de promocion por tipo:")
        for name, b in kinds.items():
            it = b["promote"][0]
            print(f"  {name:22s} truth={it['truth']!s:>12} | {it['problem'][:75]}")
        return

    items_path = pathlib.Path(__file__).resolve().parent / "logs" / "ablation_synth_kinds_items.jsonl"
    items_path.parent.mkdir(parents=True, exist_ok=True)
    done = {}
    if items_path.exists():
        for line in items_path.read_text(encoding="utf-8").splitlines():
            try:
                r = json.loads(line)
            except json.JSONDecodeError:
                continue
            if r.get("seed") == args.seed:
                done[(r["model"], r["kind"])] = r
    todo = [(m, k) for m in models for k in kinds if (m, k) not in done]
    rows = [done[(m, k)] for m in models for k in kinds if (m, k) in done]
    if rows:
        print(f"retomando: {len(rows)} (modelo,tipo) guardados, {len(todo)} pendientes")
    if args.analyze_only:
        todo = []

    fails = 0
    with ThreadPoolExecutor(max_workers=args.workers) as ex, \
         open(items_path, "a", encoding="utf-8") as fh:
        futs = {ex.submit(_synth_kind, m, k, kinds[k]): (m, k) for m, k in todo}
        for n, f in enumerate(as_completed(futs), 1):
            try:
                r = f.result()
            except Exception as e:  # API caida: se registra, no se inventa
                fails += 1
                print(f"  caido {futs[f]}: {type(e).__name__}", flush=True)
                continue
            r["seed"] = args.seed
            rows.append(r)
            fh.write(json.dumps(r, ensure_ascii=False) + "\n"); fh.flush()
            if n % 20 == 0:
                print(f"  ... {n}/{len(todo)} (${sum(x['cost'] for x in rows):.3f}, {fails} caidos)", flush=True)

    cost = sum(r["cost"] for r in rows)
    bar = "=" * 64
    print(f"\n{bar}\nRESULTADO synth por TIPO (filas={len(rows)}, {fails} caidos, costo ${cost:.4f})\n{bar}")

    by_model = {m: {r["kind"]: r for r in rows if r["model"] == m} for m in models}
    print("\nPOR MODELO (unidad = tipo; espec/sens = items de test agregados):")
    kind_fail = {}
    for m in models:
        rs = list(by_model[m].values())
        if not rs:
            continue
        prom = sum(1 for r in rs if r["promoted"])
        kf = sum(1 for r in rs if r["kind_failed"])
        tests = [t for r in rs for t in r["test"]]
        nc = sum(1 for t in tests if t["is_correct"]); nf = len(tests) - nc
        spec = sum(1 for t in tests if t["is_correct"] and t["passed"]) / max(nc, 1)
        sens = sum(1 for t in tests if not t["is_correct"] and not t["passed"]) / max(nf, 1)
        kind_fail[m] = {r["kind"]: r["kind_failed"] for r in rs}
        ts = sorted(t["t"] for r in rs if r["promoted"] for t in r["test"] if "t" in t)
        med = ts[len(ts) // 2] if ts else float("nan")
        p95 = ts[int(len(ts) * 0.95)] if ts else float("nan")
        tout = sum(1 for r in rs if r["promoted"] for t in r["test"] if t["got"] is None)
        print(f"  {m:20s} promovidos={prom:2d}/{len(rs)}  tipos_fallados={kf:2d}  "
              f"espec={spec:.3f} {_wilson(round(spec * nc), nc)}  sens={sens:.3f}  "
              f"t_med={med:.2f}s t_p95={p95:.2f}s timeouts={tout}  ${sum(r['cost'] for r in rs):.3f}")

    print("\nPHI por par sobre 'fallo el tipo' (alto = se equivocan en los MISMOS tipos):")
    phis, same, cross = [], [], []
    knames = [k for k, _ in KINDS]
    for m1, m2 in itertools.combinations([m for m in models if m in kind_fail], 2):
        # SOLO tipos presentes en ambos. El run del 2026-09-10 (68 caidos por API)
        # contaba un tipo ausente como fallado y fabrico phi=+0.357 entre los dos glm
        # con "co-fallos" que eran llamadas caidas, no funciones mal escritas.
        common = [k for k in knames if k in kind_fail[m1] and k in kind_fail[m2]]
        xs = [kind_fail[m1][k] for k in common]
        ys = [kind_fail[m2][k] for k in common]
        ph, both = _phi_bin(xs, ys)
        is_cross = family_of(m1) != family_of(m2)
        (cross if is_cross else same).append(ph)
        tag = "CROSS-fam" if is_cross else "MISMA-fam"
        cok = [k for k in knames if kind_fail[m1].get(k) and kind_fail[m2].get(k)]
        print(f"  {tag}  phi={ph:+.3f}  co-fallos={both:2d}  | {m1} vs {m2}  {cok[:4]}")
        phis.append({"m1": m1, "m2": m2, "cross_family": is_cross, "phi": ph, "co_fallos": both, "tipos": cok})
    import statistics as st
    def _mean(v):
        v = [x for x in v if x == x]
        return st.mean(v) if v else float("nan")
    print(f"\n  phi medio MISMA-familia = {_mean(same):+.3f} (n={len(same)})   "
          f"phi medio CROSS-familia = {_mean(cross):+.3f} (n={len(cross)})")

    print("\nTIPOS mas fallados (cuantos modelos fallaron cada uno):")
    cnt = {k: sum(1 for m in kind_fail if kind_fail[m].get(k)) for k in knames}
    for k, c in sorted(cnt.items(), key=lambda kv: -kv[1])[:12]:
        if c:
            print(f"  {c}/{len(kind_fail)}  {k}")

    _save_result({
        "experimento": "ablation_synth_kinds",
        "pregunta": "dos modelos escriben la MISMA funcion mal para el mismo tipo? "
                    "decorrelacion de errores de codigo, unidad = tipo",
        "seed": args.seed, "tipos": len(kinds), "modelos": models, "costo_usd": round(cost, 4),
        "caidos": fails, "phi_pares": phis,
        "phi_medio_misma_familia": _mean(same), "phi_medio_cross_familia": _mean(cross),
        "tipos_fallados_por_modelo": {m: sorted(k for k, v in kind_fail[m].items() if v) for m in kind_fail},
        "items_file": str(items_path),
    })


if __name__ == "__main__":
    main()
