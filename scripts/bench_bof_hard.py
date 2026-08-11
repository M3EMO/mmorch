"""bof v2 — DIFFERENTIAL TESTING contra oraculo brute, problemas NOVEL (cero contaminacion).
GLM-4.6 vs deepseek-v4-pro. Cada problema: spec precisa + oraculo brute-force (obviamente
correcto por exhaustion) + 300 inputs random seeded + edge cases. El modelo pasa SOLO si
matchea el oraculo en TODOS. Mucho mas fuerte que asserts fijos sobre LeetCode memorizado.
cero cupo."""
import sys, pathlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
from dotenv import load_dotenv; load_dotenv()
import re, time
from mmorch.providers import call
from mmorch.checkers import check

_FENCE = re.compile(r"```(?:python)?\s*(.*?)```", re.DOTALL)
def _extract(t):
    m = _FENCE.search(t or ""); return (m.group(1) if m else (t or "")).strip()

# Cada entry: (name, prompt, tests). tests embebe el oraculo `_orc` + harness differential.
PROBLEMS = [
("cooldown_schedule",
 "def cooldown_schedule(intervals, K): intervals es lista de [start,end,weight] (enteros, "
 "start<end, weight>0). Elegi un subconjunto y ordenalo por start; para CADA par consecutivo "
 "(prev,next) debe cumplirse next.start - prev.end >= K. Devolve el peso total maximo posible "
 "(0 si no elegis nada). K es entero no-negativo.",
 '''
import random
def _orc(intervals,K):
    n=len(intervals); best=0
    for mask in range(1<<n):
        ch=sorted([intervals[i] for i in range(n) if mask>>i&1])
        ok=all(ch[k+1][0]-ch[k][1]>=K for k in range(len(ch)-1))
        if ok: best=max(best,sum(c[2] for c in ch))
    return best
random.seed(12345)
for _ in range(300):
    n=random.randint(1,9); ivs=[]
    for _ in range(n):
        s=random.randint(0,8); ivs.append([s,s+random.randint(1,4),random.randint(1,5)])
    K=random.randint(0,3)
    assert cooldown_schedule([x[:] for x in ivs],K)==_orc(ivs,K),(ivs,K)
for ivs,K in [([[0,1,5]],0),([[0,5,3],[5,10,4]],0),([[0,5,3],[5,10,4]],1),([[0,2,1],[1,3,9]],0)]:
    assert cooldown_schedule([x[:] for x in ivs],K)==_orc(ivs,K),(ivs,K)
'''),

("parity_reach",
 "def parity_reach(a): a es lista NO vacia de enteros no-negativos. Arrancas en el indice 0. "
 "Desde el indice i podes saltar a cualquier j con i<j<=i+a[i] Y a[j] de la MISMA paridad que "
 "a[i]. Devolve cuantos indices son alcanzables desde 0 (incluido el 0).",
 '''
import random
def _orc(a):
    from collections import deque
    seen={0}; q=deque([0]); n=len(a)
    while q:
        i=q.popleft()
        for j in range(i+1,min(i+a[i],n-1)+1):
            if a[j]%2==a[i]%2 and j not in seen: seen.add(j); q.append(j)
    return len(seen)
random.seed(777)
for _ in range(300):
    n=random.randint(1,10); a=[random.randint(0,5) for _ in range(n)]
    assert parity_reach(a[:])==_orc(a),a
for a in [[0],[1],[2,2,2],[1,2,3],[3,1,1,1]]:
    assert parity_reach(a[:])==_orc(a),a
'''),

("circ_flip",
 "def circ_flip(bits): bits es lista de 0/1 considerada CIRCULAR (el final conecta con el "
 "principio). Podes voltear como mucho UN 0 a 1. Devolve la longitud de la corrida mas larga "
 "de 1s consecutivos teniendo en cuenta el wraparound. (todo 1s -> len; vacio -> 0).",
 '''
import random
def _orc(b):
    n=len(b)
    if n==0: return 0
    def mr(arr):
        if all(x==1 for x in arr): return n
        best=cur=0
        for x in arr*2:
            cur=cur+1 if x==1 else 0; best=max(best,cur)
        return min(best,n)
    best=mr(b[:])
    for i in range(n):
        if b[i]==0:
            c=b[:]; c[i]=1; best=max(best,mr(c))
    return best
random.seed(999)
for _ in range(300):
    n=random.randint(1,12); b=[random.randint(0,1) for _ in range(n)]
    assert circ_flip(b[:])==_orc(b),b
for b in [[0],[1],[1,1,1],[0,0,0],[1,0,1,0,1],[0,1,1,0]]:
    assert circ_flip(b[:])==_orc(b),b
'''),

("max_alt_subseq",
 "def max_alt_subseq(a): a es lista de enteros. Devolve la LONGITUD de la subsecuencia mas "
 "larga que sea ESTRICTAMENTE creciente en valor Y donde cada par consecutivo ALTERNE paridad "
 "(uno par, el siguiente impar, etc). Subsecuencia = se mantienen indices en orden, no "
 "contiguos. (lista vacia -> 0).",
 '''
import random
def _orc(a):
    n=len(a); best=0
    for mask in range(1<<n):
        sub=[a[i] for i in range(n) if mask>>i&1]
        ok=all(sub[k]<sub[k+1] and sub[k]%2!=sub[k+1]%2 for k in range(len(sub)-1))
        if ok: best=max(best,len(sub))
    return best
random.seed(2024)
for _ in range(250):
    n=random.randint(0,13); a=[random.randint(0,20) for _ in range(n)]
    assert max_alt_subseq(a[:])==_orc(a),a
for a in [[],[5],[1,2,3,4],[2,4,6],[1,4,3,8,5,10],[10,9,8]]:
    assert max_alt_subseq(a[:])==_orc(a),a
'''),

("equal_count_partition",
 "def equal_count_partition(a): a es lista de enteros (pueden ser negativos). Si len(a) es "
 "IMPAR devolve -1. Si es par, particiona a en DOS grupos de IGUAL cantidad de elementos "
 "(len(a)//2 cada uno) minimizando el valor absoluto de la diferencia de sus sumas. Devolve "
 "esa diferencia minima.",
 '''
import random
from itertools import combinations
def _orc(a):
    n=len(a)
    if n%2: return -1
    tot=sum(a); best=None
    for c in combinations(range(n),n//2):
        d=abs(tot-2*sum(a[i] for i in c)); best=d if best is None else min(best,d)
    return best
random.seed(555)
for _ in range(250):
    n=random.randint(1,10); a=[random.randint(-10,10) for _ in range(n)]
    assert equal_count_partition(a[:])==_orc(a),a
for a in [[1],[1,1],[1,2,3,4],[5,5,5,5],[-3,3],[10,-10,2,-2]]:
    assert equal_count_partition(a[:])==_orc(a),a
'''),

("min_subset_xor_geq",
 "def min_subset_xor_geq(a, t): a es lista NO vacia de enteros no-negativos, t entero "
 "no-negativo. Entre TODOS los subconjuntos NO vacios de a, encontra el minimo valor de XOR "
 "(or-exclusivo de sus elementos) que sea >= t. Devolve ese valor, o -1 si ninguno alcanza t.",
 '''
import random
def _orc(a,t):
    n=len(a); best=-1
    for mask in range(1,1<<n):
        x=0
        for i in range(n):
            if mask>>i&1: x^=a[i]
        if x>=t and (best==-1 or x<best): best=x
    return best
random.seed(31337)
for _ in range(300):
    n=random.randint(1,12); a=[random.randint(0,15) for _ in range(n)]; t=random.randint(0,20)
    assert min_subset_xor_geq(a[:],t)==_orc(a,t),(a,t)
for a,t in [([0],0),([5],10),([1,2,4],0),([3,3],1),([7,8,9],16)]:
    assert min_subset_xor_geq(a[:],t)==_orc(a,t),(a,t)
'''),

("knight_min_moves",
 "def knight_min_moves(n, sr, sc, tr, tc): tablero n x n (0-indexado). Minimo de movimientos "
 "de caballo de ajedrez desde (sr,sc) hasta (tr,tc). Devolve -1 si es inalcanzable. 0 si "
 "origen==destino.",
 '''
import random
def _orc(n,sr,sc,tr,tc):
    from collections import deque
    if (sr,sc)==(tr,tc): return 0
    M=[(1,2),(2,1),(-1,2),(-2,1),(1,-2),(2,-1),(-1,-2),(-2,-1)]
    seen={(sr,sc)}; q=deque([(sr,sc,0)])
    while q:
        r,c,d=q.popleft()
        for dr,dc in M:
            nr,nc=r+dr,c+dc
            if 0<=nr<n and 0<=nc<n and (nr,nc) not in seen:
                if (nr,nc)==(tr,tc): return d+1
                seen.add((nr,nc)); q.append((nr,nc,d+1))
    return -1
random.seed(101)
for _ in range(300):
    n=random.randint(1,6)
    sr,sc,tr,tc=[random.randint(0,n-1) for _ in range(4)]
    assert knight_min_moves(n,sr,sc,tr,tc)==_orc(n,sr,sc,tr,tc),(n,sr,sc,tr,tc)
for args in [(1,0,0,0,0),(3,0,0,1,1),(8,0,0,7,7),(2,0,0,1,1),(5,2,2,2,2)]:
    assert knight_min_moves(*args)==_orc(*args),args
'''),

("max_nonadj_circular",
 "def max_nonadj_circular(a): a es lista de enteros (pueden ser negativos), CIRCULAR (indice 0 "
 "y n-1 son adyacentes). Devolve la suma maxima de un subconjunto SIN dos elementos adyacentes. "
 "Se permite el subconjunto vacio (suma 0), asi que el resultado nunca es negativo.",
 '''
import random
def _orc(a):
    n=len(a); best=0
    for mask in range(1<<n):
        idx=[i for i in range(n) if mask>>i&1]
        ok=all(idx[k+1]-idx[k]>1 for k in range(len(idx)-1))
        if ok and len(idx)>1 and idx[0]==0 and idx[-1]==n-1: ok=False
        if ok: best=max(best,sum(a[i] for i in idx))
    return best
random.seed(202)
for _ in range(250):
    n=random.randint(1,14); a=[random.randint(-10,10) for _ in range(n)]
    assert max_nonadj_circular(a[:])==_orc(a),a
for a in [[5],[-3],[1,2,3],[2,1,2],[5,5,5,5],[-1,-2,-3]]:
    assert max_nonadj_circular(a[:])==_orc(a),a
'''),

("count_range_pairs",
 "def count_range_pairs(a, lo, hi): a es lista de enteros, lo<=hi. Devolve la cantidad de pares "
 "(i,j) con i<j tales que lo <= a[i]+a[j] <= hi.",
 '''
import random
def _orc(a,lo,hi):
    n=len(a); c=0
    for i in range(n):
        for j in range(i+1,n):
            if lo<=a[i]+a[j]<=hi: c+=1
    return c
random.seed(303)
for _ in range(300):
    n=random.randint(0,12); a=[random.randint(-10,10) for _ in range(n)]
    lo=random.randint(-15,5); hi=lo+random.randint(0,15)
    assert count_range_pairs(a[:],lo,hi)==_orc(a,lo,hi),(a,lo,hi)
for a,lo,hi in [([],0,0),([1],0,5),([1,2,3],3,5),([-1,1,-1,1],0,0),([5,5,5],10,10)]:
    assert count_range_pairs(a[:],lo,hi)==_orc(a,lo,hi),(a,lo,hi)
'''),

("longest_arith_subseq",
 "def longest_arith_subseq(a, d): a es lista de enteros, d entero. Devolve la LONGITUD de la "
 "subsecuencia (indices en orden, no necesariamente contiguos) mas larga que sea aritmetica "
 "con diferencia comun EXACTAMENTE d (cada par consecutivo difiere en d). (vacia -> 0).",
 '''
import random
def _orc(a,d):
    n=len(a); best=0
    for mask in range(1<<n):
        sub=[a[i] for i in range(n) if mask>>i&1]
        ok=all(sub[k+1]-sub[k]==d for k in range(len(sub)-1))
        if ok: best=max(best,len(sub))
    return best
random.seed(404)
for _ in range(250):
    n=random.randint(0,13); a=[random.randint(0,15) for _ in range(n)]; d=random.randint(-3,3)
    assert longest_arith_subseq(a[:],d)==_orc(a,d),(a,d)
for a,d in [([],1),([5],0),([1,3,5,7],2),([1,1,1],0),([5,4,3,2],-1),([1,2,4,8],1)]:
    assert longest_arith_subseq(a[:],d)==_orc(a,d),(a,d)
'''),
]

MODELS = ["glm-4.6", "deepseek-v4-pro"]
API_TIMEOUT = 300.0      # alto: GLM-4.6 razona largo en tareas duras (evita APITimeoutError)
MAX_TRIES = 4            # reintentos ante timeout/rate-limit
SPACING = 3.0           # seg entre calls (afloja el throttle de z.ai)


def _gen_with_backoff(model, prompt):
    """Call directo con timeout alto + backoff exponencial ante timeout/rate-limit.
    Devuelve (text, cost) o (None, 0.0) si se agotan los intentos."""
    for k in range(1, MAX_TRIES + 1):
        try:
            r = call(model, prompt, pattern="bench_hard", node=model,
                     temperature=0.0, timeout=API_TIMEOUT)
            return r.text, r.cost_usd
        except Exception as e:
            wait = min(60, 8 * k)                       # 8,16,24,... backoff ante 429/timeout
            print(f"      {model} try{k} {type(e).__name__} -> backoff {wait}s", flush=True)
            time.sleep(wait)
    return None, 0.0


def run():
    res = {m: {"pass": 0, "cost": 0.0, "n": 0, "fails": []} for m in MODELS}
    for name, prompt, tests in PROBLEMS:
        full = (f"Resolve en Python: {prompt}\nDevolve SOLO la funcion `{name}` en un bloque "
                f"```python```, sin explicacion ni ejemplos.")
        for m in MODELS:
            text, cost = _gen_with_backoff(m, full)
            res[m]["n"] += 1; res[m]["cost"] += cost
            if text is None:
                res[m]["fails"].append(name + "(call-error)")
                print(f"{name:22s} {m:18s} ERROR (sin resultado tras {MAX_TRIES} tries)", flush=True)
                time.sleep(SPACING); continue
            code = _extract(text)
            try:
                cr = check("python_exec", code=code + "\n" + tests, timeout=30.0)
                passed = bool(cr.passed)
            except Exception:
                passed = False
            res[m]["pass"] += int(passed)
            if not passed: res[m]["fails"].append(name)
            print(f"{name:22s} {m:18s} {'PASS' if passed else 'FAIL'} ${cost:.5f}", flush=True)
            time.sleep(SPACING)                          # espaciar -> menos rate-limit
    print("\n=== RESUMEN (NOVEL + differential vs oraculo, timeout alto + backoff) ===")
    for m in MODELS:
        d = res[m]
        print(f"{m:18s} pass {d['pass']}/{d['n']}  ${d['cost']:.5f}  falló: {d['fails'] or 'ninguna'}")


if __name__ == "__main__":
    run()
