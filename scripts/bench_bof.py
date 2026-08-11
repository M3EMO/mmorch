"""bof head-to-head DURO: GLM-4.6 vs deepseek-v4-pro en code_loop. 10 tareas LeetCode-Hard
self-contained, score por EJECUCION (asserts con muchos edge cases), no opinion. Mide
pass-rate + costo por modelo. cero cupo (API barata). Set duro pq en facil ambos saturan
(LiveCodeBench: modelos top casi-perfectos en easy, sufren en hard edge-cases)."""
import sys, pathlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
from dotenv import load_dotenv; load_dotenv()
from mmorch.code_loop import run_code_task

# (nombre, prompt con firma exacta, tests con edge cases)
TASKS = [
    ("find_median",
     "def find_median(a, b): mediana de dos listas YA ordenadas, devuelve float. ([],[1])->1.0",
     '''
assert find_median([1,3],[2])==2.0
assert find_median([1,2],[3,4])==2.5
assert find_median([],[1])==1.0
assert find_median([2],[])==2.0
assert find_median([1,2,3,4,5,6],[7,8,9,10])==5.5
assert find_median([1,1,1],[1,1,1])==1.0
'''),
    ("is_match",
     "def is_match(s, p): regex FULL match con '.' (cualquier char) y '*' (0+ del char previo).",
     '''
assert is_match("aa","a")==False
assert is_match("aa","a*")==True
assert is_match("ab",".*")==True
assert is_match("aab","c*a*b")==True
assert is_match("mississippi","mis*is*p*.")==False
assert is_match("","")==True
assert is_match("","a*")==True
assert is_match("a","")==False
'''),
    ("wild_match",
     "def wild_match(s, p): wildcard FULL match con '?' (un char) y '*' (cualquier secuencia incl vacia).",
     '''
assert wild_match("aa","a")==False
assert wild_match("aa","*")==True
assert wild_match("cb","?a")==False
assert wild_match("adceb","*a*b")==True
assert wild_match("acdcb","a*c?b")==False
assert wild_match("","*")==True
assert wild_match("","")==True
'''),
    ("edit_distance",
     "def edit_distance(a, b): distancia de Levenshtein (insert/delete/replace, costo 1 c/u).",
     '''
assert edit_distance("horse","ros")==3
assert edit_distance("intention","execution")==5
assert edit_distance("","")==0
assert edit_distance("abc","")==3
assert edit_distance("","abc")==3
assert edit_distance("abc","abc")==0
'''),
    ("calc",
     "def calc(s): evalua expresion con enteros, + - * / y parentesis. Division TRUNCA hacia cero. Ignora espacios.",
     '''
assert calc("1 + 1")==2
assert calc(" 2-1 + 2 ")==3
assert calc("(1+(4+5+2)-3)+(6+8)")==23
assert calc("2*(5+5*2)/3+(6/2+8)")==21
assert calc("(2+6*3+5-(3*14/7+2)*5)+3")==-12
assert calc("14-3/2")==13
'''),
    ("num_to_words",
     "def num_to_words(n): entero no-negativo -> palabras en ingles, Title Case, SIN espacios extra. 0->'Zero'.",
     '''
assert num_to_words(0)=="Zero"
assert num_to_words(20)=="Twenty"
assert num_to_words(100)=="One Hundred"
assert num_to_words(123)=="One Hundred Twenty Three"
assert num_to_words(12345)=="Twelve Thousand Three Hundred Forty Five"
assert num_to_words(1000000)=="One Million"
assert num_to_words(1000010)=="One Million Ten"
assert num_to_words(1234567)=="One Million Two Hundred Thirty Four Thousand Five Hundred Sixty Seven"
'''),
    ("trap",
     "def trap(h): agua atrapada dada lista de alturas (elevation map).",
     '''
assert trap([0,1,0,2,1,0,1,3,2,1,2,1])==6
assert trap([4,2,0,3,2,5])==9
assert trap([])==0
assert trap([1,2,3])==0
assert trap([3,2,1])==0
assert trap([5])==0
'''),
    ("longest_valid",
     "def longest_valid(s): longitud del substring mas largo de parentesis '()' bien balanceados.",
     '''
assert longest_valid("(()")==2
assert longest_valid(")()())")==4
assert longest_valid("")==0
assert longest_valid("()(()")==2
assert longest_valid("()(())")==6
assert longest_valid("(()())")==6
'''),
    ("min_window",
     "def min_window(s, t): substring minimo de s que contiene todos los chars de t (con multiplicidad). '' si no existe.",
     '''
assert min_window("ADOBECODEBANC","ABC")=="BANC"
assert min_window("a","a")=="a"
assert min_window("a","aa")==""
assert min_window("aa","aa")=="aa"
assert min_window("ab","b")=="b"
assert min_window("","a")==""
'''),
    ("num_decodings",
     "def num_decodings(s): cantidad de formas de decodificar un string de digitos (A=1..Z=26). Cuidado los '0'.",
     '''
assert num_decodings("12")==2
assert num_decodings("226")==3
assert num_decodings("0")==0
assert num_decodings("06")==0
assert num_decodings("10")==1
assert num_decodings("100")==0
assert num_decodings("11106")==2
assert num_decodings("")==0
'''),
]

MODELS = ["glm-4.6", "deepseek-v4-pro"]


def run():
    res = {m: {"pass": 0, "cost": 0.0, "n": 0, "fails": []} for m in MODELS}
    for name, prompt, tests in TASKS:
        full = (f"Escribi en Python: {prompt}\nDevolve SOLO la funcion `{name}` en un "
                f"bloque ```python```, sin explicacion, sin ejemplos de uso.")
        for m in MODELS:
            r = run_code_task(full, tests, steps=[(m, 0.0)], timeout=15.0)
            res[m]["n"] += 1
            res[m]["pass"] += int(r.passed)
            res[m]["cost"] += r.cost_usd
            if not r.passed:
                res[m]["fails"].append(name)
            print(f"{name:16s} {m:18s} {'PASS' if r.passed else 'FAIL'} cost={r.cost_usd:.5f}", flush=True)
    print("\n=== RESUMEN (set DURO, 10 tareas) ===")
    for m in MODELS:
        d = res[m]
        print(f"{m:18s} pass {d['pass']}/{d['n']}  costo ${d['cost']:.5f}  "
              f"falló: {d['fails'] or 'ninguna'}")


if __name__ == "__main__":
    run()
