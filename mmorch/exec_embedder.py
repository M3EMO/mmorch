"""exec_embedder — embedding por EJECUCION (huella de comportamiento), CERO entrenamiento.

El code_embedder (SimCLR) es ESTRUCTURAL: agrupa por superficie, colapsa (P@1 0.99->0.45)
en implementaciones funcionalmente-equivalentes pero sintacticamente diversas. El fix real no
es una red mas grande: es embeber el COMPORTAMIENTO. Se corre la funcion en N inputs-sonda
(deterministas, por arity) en el sandbox -> se canonicaliza cada output -> el vector de
(sonda -> output) es la huella funcional. Funcional-equivalentes -> mismos outputs -> mismo
embedding, sin importar la sintaxis. Equivalencia EXACTA modulo cobertura de sondas.

Alineado con la tesis del flywheel: la EJECUCION es el oraculo. Aca el oraculo ES el embedding.

Uso: from mmorch.exec_embedder import embed_exec; v = embed_exec("def f(x): return x+1", "f")
Devuelve list[float] (dim D) o None si la funcion no se halla / no corre ninguna sonda.
"""
from __future__ import annotations
import ast, hashlib, json
import numpy as np

from .sandbox import run_sandboxed

D = 256          # dim del vector (matchea convencion del code_embedder)
_TIMEOUT = 20.0  # backstop global del code; el timeout fino es POR-SONDA dentro del runner

# --- Bancos de sondas por arity (DETERMINISTAS y COMPARTIDOS) -------------------- #
# Mismo banco -> dos funciones equivalentes reciben las MISMAS sondas en el MISMO orden
# -> sus huellas son comparables. Tipos mezclados a proposito: una funcion que espera str
# erra de forma consistente ante una sonda-list ("ERR:TypeError") -> sigue siendo señal.
_P1 = [
    # ints chicos a proposito: impl recursiva naive (fib) debe TERMINAR dentro del timeout
    (0,), (1,), (2,), (5,), (10,), (13,), (15,), (20,), (-1,), (4,), (9,),
    ("",), ("a",), ("abc",), ("racecar",), ("Hello World",), ("aaabb",),
    ("A man, a plan, a canal: Panama",), ("listen",),
    ([],), ([1],), ([1, 2, 3],), ([3, 1, 2],), ([1, 2, 2, 3],), ([2, 7, 11, 15],),
    ([-2, 1, -3, 4, -1, 2, 1, -5, 4],), ([1, [2, [3, 4]], 5],),
]
_P2 = [
    # OJO sin (list, 0): chunk(lst,0) hace loop infinito (range(0) no avanza el indice) y el
    # timeout por-sonda no mata un loop tight bajo GIL -> mejor no alimentar el caso degenerado.
    ([1, 2, 3, 4, 5, 6], 3), ([1, 2, 3], 2), ([1, 2, 3, 4, 5], 2), ([2, 7, 11, 15], 9),
    ([3, 2, 4], 6), ([], 3), ([1, 2, 3], 3), ([1, 2, 3], 4), ([1, 2, 3, 4], 2),
    (12, 8), (17, 5), (100, 75), (0, 0), (1, 1), (6, 6), (48, 36),
    ("listen", "silent"), ("abc", "cba"), ("hello", "world"), ("", ""),
    ("Dormitory", "Dirty Room"), ("aa", "a"),
    ([1, 3, 5], [2, 4, 6]), ([], [1]), ([1, 1], [1]), ([1, 2], [3, 4]),
]
_P3 = [(1, 2, 3), ([1, 2, 3], 1, 2), ("abc", 0, 1), ([], 0, 0)]
_PROBES = {1: _P1, 2: _P2, 3: _P3}


def _target_fn(code: str, fn_name: str | None) -> str | None:
    """Nombre de la funcion a sondear. Hint explicito si esta presente; si no, el ULTIMO
    def top-level (heuristica: la entry suele definirse al final, los helpers arriba)."""
    try:
        tree = ast.parse(code)
    except Exception:
        return fn_name
    defs = [n.name for n in tree.body if isinstance(n, ast.FunctionDef)]
    if fn_name and fn_name in defs:
        return fn_name
    return defs[-1] if defs else fn_name


# Runner: se ejecuta en el sandbox. Define el code, halla la fn, corre cada sonda con:
#  - canon float-tolerante (round 9 dec -> impls equivalentes con epsilon distinto = mismo canon)
#  - timeout POR-SONDA (daemon thread): una sonda lenta (recursion naive) no mata las demas
#  - captura de MUTACION in-place (sort/swap que devuelven None pero mutan el arg)
# y emite JSON con los reprs canonizados.
_RUNNER = r'''
import json, io, contextlib, sys, copy, threading

PROBE_TO = 1.0   # timeout por sonda (s); el global del sandbox es backstop

def canon(o, depth=0):
    if depth > 6:
        return "..."
    if isinstance(o, bool):
        return repr(o)
    if isinstance(o, float):
        if o != o:
            return "nan"
        if o in (float("inf"), float("-inf")):
            return repr(o)
        return repr(round(o, 9))
    if isinstance(o, dict):
        return "{" + ",".join(f"{canon(k,depth+1)}:{canon(o[k],depth+1)}"
                              for k in sorted(o, key=lambda x: repr(x))) + "}"
    if isinstance(o, (set, frozenset)):
        return "set(" + ",".join(sorted(canon(x, depth+1) for x in o)) + ")"
    if isinstance(o, (list, tuple)):
        return type(o).__name__ + "[" + ",".join(canon(x, depth+1) for x in o) + "]"
    return repr(o)[:120]

SRC = json.loads(sys.argv[1])
FN = json.loads(sys.argv[2])
PROBES = json.loads(sys.argv[3])

ns = {}
try:
    exec(SRC, ns)
except Exception as e:
    print(json.dumps({"err": "exec:" + type(e).__name__})); sys.exit(0)

import types
fn = None
if FN and callable(ns.get(FN)):          # callable, no solo FunctionType: cubre `from math import gcd`
    fn = ns[FN]
else:
    fns = [v for v in ns.values() if isinstance(v, types.FunctionType)]
    fn = fns[-1] if fns else None
if fn is None:
    print(json.dumps({"err": "no_fn"})); sys.exit(0)


def call_one(args):
    """Corre fn(*args) con timeout por-sonda y captura mutacion del arg. El daemon thread
    que cuelga (loop infinito) muere al salir el proceso; GIL deja avanzar a los demas."""
    box = {}
    def run():
        try:
            before = copy.deepcopy(args)
            with contextlib.redirect_stdout(io.StringIO()):
                out = fn(*args)
            r = canon(out)
            if args != before:               # mutacion in-place
                r += "§MUT:" + canon(args)
            box["v"] = r
        except Exception as e:
            box["v"] = "ERR:" + type(e).__name__
    t = threading.Thread(target=run, daemon=True)
    t.start(); t.join(PROBE_TO)
    if t.is_alive():
        return "ERR:Timeout"
    return box.get("v", "ERR:Empty")


results = [call_one(list(args)) for args in PROBES]
print(json.dumps({"r": results}))
'''


def _feat(results: list[str]) -> np.ndarray:
    """Feature-hash de (indice_de_sonda | output) -> vector denso D. Identicos -> identico."""
    v = np.zeros(D, dtype=np.float32)
    for i, r in enumerate(results):
        h = hashlib.blake2b(f"{i}|{r}".encode("utf-8"), digest_size=8).digest()
        idx = int.from_bytes(h[:4], "little") % D
        sign = 1.0 if (h[4] & 1) else -1.0
        v[idx] += sign
    return v


def embed_exec(code: str, fn_name: str | None = None) -> list[float] | None:
    """Huella de comportamiento del code. None si no se halla la fn o ninguna sonda corrio.
    El arity se detecta DENTRO del runner (inspect); aca elegimos el banco por la firma AST."""
    fn = _target_fn(code, fn_name)
    if fn is None:
        return None                      # ni def top-level ni hint -> nada que ejecutar (no gasta sandbox)
    # arity por AST del def elegido (cuenta de positionals; fallback prueba 1 y 2)
    arities = _arities(code, fn)
    for n in arities:
        probes = _PROBES.get(n)
        if not probes:
            continue
        res = run_sandboxed(
            _RUNNER, timeout=_TIMEOUT, argv=["_run.py", json.dumps(code),
                                             json.dumps(fn), json.dumps(probes)],
        )
        if not res.ok or not res.stdout.strip():
            continue
        try:
            payload = json.loads(res.stdout.strip().splitlines()[-1])
        except Exception:
            continue
        results = payload.get("r")
        if results:
            return _feat(results).tolist()
    return None


def _arities(code: str, fn_name: str | None):
    """Arity(es) candidata(s) del def elegido. Cuenta positionals; si no se halla, [1,2]."""
    try:
        tree = ast.parse(code)
    except Exception:
        return [1, 2]
    target = None
    for n in tree.body:
        if isinstance(n, ast.FunctionDef) and (n.name == fn_name or fn_name is None):
            target = n
            if n.name == fn_name:
                break
    if target is None:
        return [1, 2]
    a = target.args
    npos = len(a.posonlyargs) + len(a.args)
    return [npos] if npos in _PROBES else [1, 2]


_BEHAV_DIM = D   # la huella behavioral aporta D dims; zeros si la fn no corre (dim consistente)


def embed_hybrid(code: str, fn_name: str | None = None) -> list[float] | None:
    """Embedding HIBRIDO = structural (code_embedder) ⊕ behavioral (exec), cada parte L2-norm.
    Drop-in como `embed_fn` de shadow_prior/recall para similitud FUNCIONAL de CODIGO ejecutable.
    Dim CONSISTENTE: si la fn no corre, la parte behavioral son zeros (no cambia la dim -> el
    coseno del consumidor no se rompe al mezclar filas con/sin huella). None solo si tampoco hay
    structural (artefactos del encoder ausentes). Sobre texto-no-codigo cae a structural-puro."""
    from .code_embedder import embed_code
    s = embed_code(code)
    if s is None:
        return None                      # sin encoder estructural no hay hibrido
    sv = np.asarray(s, dtype=np.float32)
    sv = sv / (np.linalg.norm(sv) + 1e-9)
    b = embed_exec(code, fn_name)
    if b is None:
        bv = np.zeros(_BEHAV_DIM, dtype=np.float32)
    else:
        bv = np.asarray(b, dtype=np.float32)
        bv = bv / (np.linalg.norm(bv) + 1e-9)
    return np.concatenate([sv, bv]).tolist()


def available() -> bool:
    return True
