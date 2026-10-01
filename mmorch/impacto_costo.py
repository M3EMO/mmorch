"""impacto_costo — chequeo de costo despues de escribir un archivo Python (ticket 15).

Avisa cuando el archivo llama dentro de un bucle a una funcion que lee un almacen entero en cada
llamada: el cuadratico oculto del pilar 1 del video. Medido (ticket 15, deepseek-v4-pro): en
tareas donde la solucion natural cae en esa trampa, el acierto pasa de 33% a 100% con el aviso
despues de escribir; anotar al inicio no sirve, porque la funcion trampa vive en un modulo que el
archivo todavia no importa.

Una funcion "recorre" si abre un archivo para leer, carga JSON/CSV, lista una carpeta o corre un
SELECT sin WHERE, y la ruta no sale de un parametro (esa lectura es un archivo distinto por
llamada). Hereda la marca con un salto. Bucle: cuerpo de for/while, elemento o condicion de una
comprehension (no su primer iterable) y la funcion o lambda de key=/map/filter. Hechos por
archivo con cache; mismo texto que el detector medido (orch-spike, extractor_costo_post_v2).
"""
from __future__ import annotations

import ast
import re
from pathlib import Path

MAX_LINES = 10
_DATA_FILE = re.compile(r"[A-Za-z0-9_.-]+\.(?:jsonl|json|db|sqlite|sqlite3|csv|tsv|log|txt|ya?ml)\b")
_READ_ATTRS = {"read_text", "read_bytes", "readlines", "listdir", "scandir", "iterdir", "glob", "rglob",
               "reader", "DictReader", "read_csv"}
_LOADERS = {"json", "pickle", "yaml", "tomllib"}   # json.load(fh): `load` solo con estos receptores
_SQL_FROM = re.compile(r"\bSELECT\b.*?\bFROM\s+(\w+)", re.I | re.S)
_KEYED = {"sorted", "min", "max", "sort", "map", "filter", "groupby", "nlargest", "nsmallest"}


def _mod(rel: str) -> str:
    m = rel[:-3].replace("/", ".")
    return m[:-9] if m.endswith(".__init__") else m


def _call_name(c: ast.Call) -> str | None:
    f = c.func
    return f.id if isinstance(f, ast.Name) else f.attr if isinstance(f, ast.Attribute) else None


def _open_mode(c: ast.Call) -> str:
    args = c.args[0:1] if isinstance(c.func, ast.Attribute) else c.args[1:2]   # p.open("a") / open(p, "a")
    for v in args + [k.value for k in c.keywords if k.arg == "mode"]:
        if isinstance(v, ast.Constant) and isinstance(v.value, str):
            return v.value
    return "r"


def _tainted(f: ast.FunctionDef | ast.AsyncFunctionDef) -> set[str]:
    """Parametros de la funcion y nombres locales asignados desde ellos (dos pasadas)."""
    a = f.args
    t = {x.arg for x in a.args + a.kwonlyargs + a.posonlyargs + [v for v in (a.vararg, a.kwarg) if v]}
    for _ in range(2):
        for n in ast.walk(f):
            pairs: list[tuple[ast.AST, ast.AST]] = []
            if isinstance(n, (ast.Assign, ast.AnnAssign)) and n.value is not None:
                pairs = [(tg, n.value) for tg in (n.targets if isinstance(n, ast.Assign) else [n.target])]
            elif isinstance(n, (ast.For, ast.comprehension)):
                pairs = [(n.target, n.iter)]
            elif isinstance(n, ast.withitem) and n.optional_vars is not None:
                pairs = [(n.optional_vars, n.context_expr)]
            for tg, val in pairs:
                if any(isinstance(x, ast.Name) and x.id in t for x in ast.walk(val)):
                    t |= {x.id for x in ast.walk(tg) if isinstance(x, ast.Name)}
    return t


def _path_of(c: ast.Call) -> list[ast.AST]:
    """Expresiones que definen que se lee: receptor del metodo y argumentos (sin el modo)."""
    parts: list[ast.AST] = [c.func.value] if isinstance(c.func, ast.Attribute) else []
    return parts + [x for x in c.args if not (isinstance(x, ast.Constant) and isinstance(x.value, str) and len(x.value) <= 3)]


def _direct_scan(f: ast.FunctionDef | ast.AsyncFunctionDef) -> str | None:
    """Que almacen lee entero la funcion, o None. Ignora lecturas cuya ruta sale de un parametro."""
    taint = _tainted(f)
    for n in ast.walk(f):
        if not isinstance(n, ast.Call):
            continue
        if any(isinstance(x, ast.Name) and x.id in taint for p in _path_of(n) for x in ast.walk(p)):
            continue   # lee lo que le pasan: un archivo distinto por llamada
        name = _call_name(n)
        if name == "open" and not set(_open_mode(n)) & set("wax+"):
            return "un archivo"
        recv = n.func.value.id if isinstance(n.func, ast.Attribute) and isinstance(n.func.value, ast.Name) else None
        if name == "walk" and recv == "os" or name in ("listdir", "scandir", "iterdir", "glob", "rglob"):
            return "una carpeta"
        if name in _READ_ATTRS or name in ("load", "safe_load") and recv in _LOADERS:
            return "un archivo"
        if name == "execute" and n.args and isinstance(n.args[0], ast.Constant) and isinstance(n.args[0].value, str):
            sql = n.args[0].value
            m = _SQL_FROM.search(sql)
            if m and not re.search(r"\b(WHERE|LIMIT)\b", sql, re.I):
                return f"la tabla `{m.group(1)}`"
    return None


def _store_name(f: ast.AST, helpers: dict[str, ast.AST]) -> str | None:
    """Nombre del archivo de datos que usa la funcion o un helper del mismo modulo que llama."""
    seen = [f] + [helpers[cn] for c in ast.walk(f) if isinstance(c, ast.Call)
                  and (cn := _call_name(c)) in helpers and helpers[cn] is not f]
    for g in seen:
        for n in ast.walk(g):
            if isinstance(n, ast.Constant) and isinstance(n.value, str):
                m = _DATA_FILE.search(n.value)
                if m:
                    return Path(m.group(0)).name
    return None


def _in_loop(tree: ast.Module) -> list[list]:
    """[linea, nombre, base] de cada llamada que corre una vez por iteracion (base = modulo o None)."""
    out: list[ast.AST] = []

    def calls(nodes):
        for n in nodes:
            out.extend(c.func for c in ast.walk(n) if isinstance(c, ast.Call))

    for n in ast.walk(tree):
        if isinstance(n, (ast.For, ast.AsyncFor, ast.While)):
            calls(n.body)
            if isinstance(n, ast.While):
                calls([n.test])
        elif isinstance(n, (ast.ListComp, ast.SetComp, ast.GeneratorExp, ast.DictComp)):
            elts = [n.key, n.value] if isinstance(n, ast.DictComp) else [n.elt]
            gens = n.generators
            calls(elts + [c for g in gens for c in g.ifs] + [g.iter for g in gens[1:]])
        elif isinstance(n, ast.Call) and _call_name(n) in _KEYED:
            name = _call_name(n)
            for a in [k.value for k in n.keywords if k.arg == "key"] + (n.args[:1] if name in ("map", "filter") else []):
                if isinstance(a, ast.Lambda):
                    calls([a.body])
                elif isinstance(a, (ast.Name, ast.Attribute)):   # key=get_x: se llama por elemento
                    out.append(a)
    rows = []
    for f in dict.fromkeys(out):
        if isinstance(f, ast.Name):
            rows.append([f.lineno, f.id, None])
        elif isinstance(f, ast.Attribute) and isinstance(f.value, ast.Name):
            rows.append([f.lineno, f.attr, f.value.id])
    return rows


def facts(src: str, rel: str) -> dict:
    """Todo lo que el chequeo necesita de un archivo, sin mirar los demas (cacheable en JSON)."""
    tree = ast.parse(src)
    defs = [n for n in tree.body if isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef))]
    helpers: dict[str, ast.AST] = {f.name: f for f in defs}
    funcs = {}
    for f in defs:
        what = _direct_scan(f)
        direct = None
        if what:
            store = _store_name(f, helpers)
            obj = f"`{store}`" if store and what == "un archivo" else what
            direct = f"lee {obj} {'completa' if what.startswith(('una ', 'la ')) else 'completo'} en cada llamada"
        # solo `g()`: `x.items()` es un metodo del objeto, no la funcion `items` del modulo (falso positivo 2026-10-01)
        funcs[f.name] = {"line": f.lineno, "direct": direct,
                         "calls": [c.func.id for c in ast.walk(f) if isinstance(c, ast.Call) and isinstance(c.func, ast.Name)]}
    return {"mod": _mod(rel), "funcs": funcs,
            "top_froms": [[n.module, a.name, a.asname] for n in tree.body if isinstance(n, ast.ImportFrom) for a in n.names],
            "froms": [[n.module, a.name, a.asname] for n in ast.walk(tree) if isinstance(n, ast.ImportFrom) for a in n.names],
            "imports": [[a.name, a.asname or a.name] for n in ast.walk(tree) if isinstance(n, ast.Import) for a in n.names],
            "loop_calls": _in_loop(tree)}


def _scans(F: dict[str, dict]) -> dict[tuple[str, str], str]:
    by_mod = {f["mod"]: r for r, f in F.items()}
    direct = {(r, n): fn["direct"] for r, f in F.items() for n, fn in f["funcs"].items() if fn["direct"]}
    out = dict(direct)
    for r, f in F.items():   # un salto exacto: llama a una funcion que recorre directamente
        imported = {(a or n): (by_mod.get(m or ""), n) for m, n, a in f["top_froms"]}
        for name, fn in f["funcs"].items():
            if (r, name) in out:
                continue
            for cn in fn["calls"]:
                target = (r, cn) if (r, cn) in direct else imported.get(cn or "")
                if target and target in direct:
                    out[(r, name)] = f"llama a `{target[1]}`, que {direct[target]}"
                    break
    return out


def report(F: dict[str, dict], written: str) -> str:
    """Aviso de costo para el archivo recien escrito (`written`, ruta relativa) sobre los hechos del repo."""
    if written not in F:
        return ""
    scans = _scans(F)
    by_mod = {f["mod"]: r for r, f in F.items()}
    w = F[written]
    local = {n: (written, n) for n in w["funcs"]}
    local.update({(a or n): (by_mod[m], n) for m, n, a in w["froms"] if m in by_mod})
    mods = {al: by_mod[m] for m, al in w["imports"] if m in by_mod}
    hits: dict[tuple[str, str], str] = {}
    for line, name, base in sorted(w["loop_calls"], key=lambda x: x[0]):
        target = local.get(name) if base is None else ((mods[base], name) if base in mods else None)
        if target and target in scans:
            r, fname = target
            hits.setdefault(target, f"- linea {line}: `{fname}` ({r}:{F[r]['funcs'][fname]['line']}) {scans[target]}")
    if not hits:
        return ""
    return (f"COSTO (automatico): en {written} llamas dentro de un bucle a funciones que recorren un "
            "almacen entero en cada llamada (O(n) por llamada, O(n^2) en el bucle). Lee el almacen una "
            "vez antes del bucle:\n" + "\n".join(list(hits.values())[:MAX_LINES]))
