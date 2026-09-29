"""impacto_indirecto — aristas indirectas de un archivo Python (ticket 04 del mapa de impacto).

Lista los sitios donde otros modulos usan funciones del archivo sin llamarlas por su nombre:
funcion como valor (kwarg, argumento, default, contenedor, atributo asignado) con un salto
hasta donde se invoca, decorador que la guarda en un registro global, y `getattr` con un
prefijo de su nombre. Medido (ticket 04, deepseek-v4-pro): en tareas con un lector acoplado
por esas vias, el acierto pasa de 38% con el informe de literales solo a 92% con este agregado.

Cada archivo se reduce a "hechos" (JSON) con cache por (mtime_ns, size); el informe se arma
sobre los hechos. Mismo texto que el detector medido en el banco (orch-spike, commit 6bf14a5).
"""
from __future__ import annotations

import ast
import json
from pathlib import Path

MAX_LINES = 10
_MUTATORS = {"append", "add", "setdefault", "update", "insert", "extend"}
_DYN = {"getattr", "hasattr", "import_module", "__import__"}


def _mod(rel: str) -> str:
    m = rel[:-3].replace("/", ".")
    return m[:-9] if m.endswith(".__init__") else m


def _calls_of(fdef: ast.AST, param: str) -> list[int]:
    return sorted({n.lineno for n in ast.walk(fdef) if isinstance(n, ast.Call)
                   and isinstance(n.func, ast.Name) and n.func.id == param})


def _shadowed(n: ast.Name, par: dict[int, ast.AST], memo: dict[int, set[str]]) -> bool:
    f = par.get(id(n))
    while f is not None and not isinstance(f, (ast.FunctionDef, ast.AsyncFunctionDef, ast.Lambda)):
        f = par.get(id(f))
    if f is None:
        return False
    if id(f) not in memo:   # un recorrido por funcion, no uno por sitio
        a = f.args
        params = {x.arg for x in a.args + a.kwonlyargs + a.posonlyargs + [v for v in (a.vararg, a.kwarg) if v]}
        memo[id(f)] = params | {x.id for x in ast.walk(f) if isinstance(x, ast.Name) and isinstance(x.ctx, ast.Store)}
    return n.id in memo[id(f)]


def _follow(rel: str, node: ast.AST, par: dict[int, ast.AST]) -> dict:
    """Forma del sitio donde la funcion se usa como valor; lo que cruza modulos se resuelve despues."""
    p = par.get(id(node))
    grand = par.get(id(p)) if p is not None else None
    call: ast.Call | None = None
    param: str | int | None = None
    if isinstance(p, ast.keyword) and isinstance(grand, ast.Call):
        call, param = grand, p.arg
    elif isinstance(p, ast.Call) and node in p.args:
        call, param = p, p.args.index(node)
    if call is not None:
        if isinstance(call.func, ast.Name):
            return {"k": "call", "callee": call.func.id, "param": param}
        return {"k": "text", "desc": f"se pasa a `{ast.unparse(call.func)}`"}
    if isinstance(p, ast.arguments):
        fdef = par.get(id(p))
        defaults = dict(zip([a.arg for a in p.args[len(p.args) - len(p.defaults):]], p.defaults, strict=True))
        defaults.update({a.arg: d for a, d in zip(p.kwonlyargs, p.kw_defaults, strict=True) if d is not None})
        pname = next((k for k, d in defaults.items() if d is node), None)
        lines = _calls_of(fdef, pname) if pname and fdef else []
        return {"k": "text", "desc": f"valor por defecto de `{pname}`"
                + (f"; se invoca en {', '.join(f'{rel}:{x}' for x in lines)}" if lines else "")}
    holder = p
    while isinstance(holder, (ast.List, ast.Tuple, ast.Set, ast.Dict)):
        holder = par.get(id(holder))
    if isinstance(holder, (ast.Assign, ast.AnnAssign)):
        tgt = holder.targets[0] if isinstance(holder, ast.Assign) else holder.target
        if isinstance(tgt, ast.Name):
            return {"k": "holder", "name": tgt.id, "line": holder.lineno}
        if isinstance(tgt, ast.Attribute):
            return {"k": "attr", "attr": tgt.attr}
    return {"k": "text", "desc": "se usa como valor"}


def facts(src: str, rel: str) -> dict:
    """Todo lo que el informe necesita de un archivo, sin mirar los demas (cacheable en JSON)."""
    tree = ast.parse(src)
    nodes = list(ast.walk(tree))
    par = {id(c): n for n in nodes for c in ast.iter_child_nodes(n)}
    glob = [t.id for n in tree.body if isinstance(n, (ast.Assign, ast.AnnAssign))
            for t in ([n.targets[0]] if isinstance(n, ast.Assign) else [n.target]) if isinstance(t, ast.Name)]
    froms, imports = [], []
    for n in nodes:
        if isinstance(n, ast.ImportFrom) and not n.level:
            froms += [[n.module, a.name, a.asname] for a in n.names]
        elif isinstance(n, ast.Import):
            imports += [[a.name, a.asname or a.name] for a in n.names]
    funcs = {}
    for f in tree.body:
        if not isinstance(f, (ast.FunctionDef, ast.AsyncFunctionDef)):
            continue
        params = [a.arg for a in f.args.args + f.args.kwonlyargs]
        decos = [(d.func if isinstance(d, ast.Call) else d) for d in f.decorator_list]
        body = list(ast.walk(f))   # un recorrido por funcion para registros y llamadas a parametros
        mutates = sorted({n.value.id for n in body   # REG[k] = f  o  REG.append(f)
                          if isinstance(n, (ast.Subscript, ast.Attribute)) and isinstance(n.value, ast.Name)
                          and n.value.id in glob and (isinstance(n, ast.Subscript) and isinstance(n.ctx, ast.Store)
                                                      or isinstance(n, ast.Attribute) and n.attr in _MUTATORS)})
        called: dict[str, set[int]] = {}
        for n in body:
            if isinstance(n, ast.Call) and isinstance(n.func, ast.Name):
                called.setdefault(n.func.id, set()).add(n.lineno)
        funcs[f.name] = {"line": f.lineno, "end": f.end_lineno or f.lineno, "params": params,
                         "calls": {p: sorted(called.get(p, ())) for p in params},
                         "decos": [d.id for d in decos if isinstance(d, ast.Name)], "mutates": mutates}
    fnames = set(funcs) | {a or m for _, m, a in froms}
    mod_alias = {al for _, al in imports}
    holders = {t.id for n in nodes if isinstance(n, (ast.Assign, ast.AnnAssign))   # tambien locales:
               for t in (n.targets if isinstance(n, ast.Assign) else [n.target]) if isinstance(t, ast.Name)}  # stages = [...]
    watch = holders | {m for _, m, _ in froms} | {a for _, _, a in froms if a}
    sites: list[dict] = []
    loads: dict[str, list[int]] = {}
    attr_calls: dict[str, list[int]] = {}
    dyn: list[list] = []
    memo: dict[int, set[str]] = {}
    for n in nodes:
        name: str | None = None
        base: str | None = None
        if isinstance(n, ast.Name) and isinstance(n.ctx, ast.Load):
            if n.id in watch:
                loads.setdefault(n.id, []).append(n.lineno)
            if n.id in fnames:
                name = n.id
        elif isinstance(n, ast.Attribute) and isinstance(n.value, ast.Name) and n.value.id in mod_alias:
            name, base = n.attr, n.value.id
        if isinstance(n, ast.Call):
            fn = n.func.id if isinstance(n.func, ast.Name) else n.func.attr if isinstance(n.func, ast.Attribute) else None
            if isinstance(n.func, ast.Attribute):
                attr_calls.setdefault(n.func.attr, []).append(n.lineno)
            if fn in _DYN:
                dyn += [[c.value, c.lineno, fn] for c in ast.walk(n)
                        if isinstance(c, ast.Constant) and isinstance(c.value, str) and len(c.value) >= 3]
        if name is None or not isinstance(n, (ast.Name, ast.Attribute)):
            continue
        p = par.get(id(n))
        if isinstance(p, ast.Call) and p.func is n:
            continue   # llamada directa: la ve cualquier grep
        if isinstance(p, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)) and n in p.decorator_list:
            continue   # uso como decorador: lo cubre el registro
        sites.append({"name": name, "base": base, "line": n.lineno,
                      "shadowed": isinstance(n, ast.Name) and _shadowed(n, par, memo), "follow": _follow(rel, n, par)})
    return {"mod": _mod(rel), "glob": glob, "froms": froms, "imports": imports, "funcs": funcs,
            "sites": sites, "loads": loads, "attr_calls": attr_calls, "dyn": dyn}


def all_facts(root: Path, files: list[Path], cache_path: Path, extract=None) -> dict[str, dict]:
    """rel -> hechos, con cache por (mtime_ns, size): en frio se parsea todo; despues, lo cambiado.
    `extract(src, rel)` elige que hechos sacar (por defecto, las aristas indirectas)."""
    extract = extract or facts
    try:
        cache = json.loads(cache_path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        cache = {}
    out, fresh = {}, {}
    for p in files:
        rel = p.relative_to(root).as_posix()
        try:
            st = p.stat()
            key = [st.st_mtime_ns, st.st_size]
            hit = cache.get(rel)
            f = hit[1] if hit and hit[0] == key else extract(p.read_text(encoding="utf-8"), rel)
        except (SyntaxError, UnicodeDecodeError, ValueError, OSError, RecursionError):
            continue
        out[rel] = f
        fresh[rel] = [key, f]
    try:
        cache_path.write_text(json.dumps(fresh), encoding="utf-8")
    except OSError:
        pass
    return out


def _source_of(F: dict, rel: str, name: str) -> str | None:
    """Modulo donde vive `name` visto desde `rel`: definido ahi o importado con from."""
    if name in F[rel]["funcs"]:
        return rel
    for mod, n, a in F[rel]["froms"]:
        if (a or n) == name:
            for r, f in F.items():
                if f["mod"] == mod and n in f["funcs"]:
                    return r
    return None


def _name_refs(F: dict, owner: str, name: str, skip: tuple[str, int] | None = None) -> list[str]:
    """Usos de un global `name` de `owner`: en `owner` y en los modulos que lo importan con from."""
    mods = [owner] + [r for r, f in F.items() if r != owner
                      and any(m == F[owner]["mod"] and n == name for m, n, _ in f["froms"])]
    out = {f"{r}:{ln}" for r in mods for ln in F[r]["loads"].get(name, []) if (r, ln) != skip}
    return sorted(out, key=lambda x: (x.split(":")[0], int(x.split(":")[1])))


def _describe(F: dict, rel: str, fo: dict) -> str:
    if fo["k"] == "text":
        return fo["desc"]
    if fo["k"] == "holder":
        refs = _name_refs(F, rel, fo["name"], skip=(rel, fo["line"]))
        return f"queda en `{fo['name']}`" + (f", que se usa en {', '.join(refs[:4])}" if refs else "")
    if fo["k"] == "attr":
        refs = sorted({f"{r}:{ln}" for r, f in F.items() for ln in f["attr_calls"].get(fo["attr"], [])})
        return f"se asigna a `.{fo['attr']}`" + (f", que se invoca en {', '.join(refs[:4])}" if refs else "")
    callee, param = fo["callee"], fo["param"]
    src = _source_of(F, rel, callee)
    fdef = F[src]["funcs"].get(callee) if src else None
    if fdef is None:
        return f"se pasa a `{callee}`"
    params = fdef["params"]
    pname = param if isinstance(param, str) else (params[param] if param < len(params) else None)
    lines = fdef["calls"].get(pname, []) if pname else []
    where = f"; se invoca en {', '.join(f'{src}:{x}' for x in lines)}" if lines else ""
    return f"se pasa a `{callee}` ({'`' + pname + '=`' if pname else 'argumento'}){where}"


def _registries(F: dict, writer: str) -> list[str]:
    out = []
    for fname, f in F[writer]["funcs"].items():
        for dn in f["decos"]:
            src = _source_of(F, writer, dn)
            d = F[src]["funcs"].get(dn) if src else None
            if src is None or d is None:
                continue
            for reg in d["mutates"]:
                refs = [r for r in _name_refs(F, src, reg) if not (r.split(":")[0] == src
                        and d["line"] <= int(r.split(":")[1]) <= d["end"])]
                out.append(f"- `{fname}` (linea {f['line']}) se registra con `@{dn}` en `{reg}` ({src})"
                           + (f"; el registro se usa en {', '.join(refs[:4])}" if refs else ""))
    return out


def report(F: dict[str, dict], writer: str) -> str:
    """Informe de aristas indirectas de `writer` (ruta relativa) sobre los hechos del repo."""
    if writer not in F:
        return ""
    names = set(F[writer]["funcs"])
    wmod = F[writer]["mod"]
    lines = _registries(F, writer)
    groups: dict[tuple, list] = {}
    for rel in sorted(F):
        f = F[rel]
        if rel == writer:
            local, mods = {n: n for n in names}, set()
        else:
            local = {a or n: n for m, n, a in f["froms"] if m == wmod and n in names}
            mods = {al for m, al in f["imports"] if m == wmod}
        for s in f["sites"]:
            if s["base"] is None:
                if s["name"] not in local or (rel == writer and s["shadowed"]):
                    continue
                fname = local[s["name"]]
                alias = f" (alias `{s['name']}`)" if s["name"] != fname else ""
            else:
                if s["base"] not in mods or s["name"] not in names:
                    continue
                fname, alias = s["name"], ""
            desc = _describe(F, rel, s["follow"])
            key = (rel, desc) if desc.startswith("queda en") else (rel, f"{s['line']}", desc)
            groups.setdefault(key, [s["line"], []])[1].append(f"`{fname}`{alias}")
        if rel == writer:
            continue
        for value, ln, fn in f["dyn"]:   # getattr/import dinamico con un string que arma nombres del escritor
            hit = sorted(x for x in names if x.startswith(value))
            if hit:
                lines.append(f"- `\"{value}\"` en {rel}:{ln} arma nombres de funciones de "
                             f"{writer} con `{fn}` ({', '.join(hit[:4])}): despacho por string")
    lines += [f"- {', '.join(dict.fromkeys(fs))} en {k[0]}:{ln} {k[-1]}" for k, (ln, fs) in groups.items()]
    lines = sorted(dict.fromkeys(lines), key=lambda x: x.endswith("se usa como valor"))   # con destino primero
    if not lines:
        return ""
    if len(lines) > MAX_LINES:
        lines = lines[:MAX_LINES - 1] + [f"- ... y {len(lines) - MAX_LINES + 1} sitios mas"]
    return ("INFORME DE IMPACTO INDIRECTO (automatico): funciones de "
            f"{writer} que otros modulos usan sin llamarlas por su nombre. Si cambias su firma, su "
            "retorno, su nombre o si es async, revisa esos sitios:\n" + "\n".join(lines))
