"""Puntaje de impacto y esfuerzo por issue de bd (rubrica en docs/agents/rubrica-impacto-esfuerzo.md).

Uso: python scripts/puntaje_issues.py [--json] [--sin-medir]
Esfuerzo = lineas de git de los commits que nombran el issue (+ `extra`). Impacto = (2V + 2A + E) / 5: cada criterio
sale medido cuando hay oraculo (bench del gate, tests rojos antes y verdes despues, run-logs) y del JSON si no.
Las mediciones corren en worktrees temporales y quedan en logs/puntaje_cache.json (por commit).
"""
import contextlib
import json
import math
import pathlib
import re
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
PUNTAJES = ROOT / "docs/agents/puntajes.json"
CACHE = ROOT / "logs/puntaje_cache.json"
RUNLOGS = ROOT / "logs/sdlc"
EXCLUIR = ("vault/", ".beads/", "README.md", "docs/generated/")
MAX_MUTANTES = 8
MUT_MIN = 0.5  # fraccion de mutantes muertos para que rojo->verde cuente como reproduccion (E=4)


def esfuerzo(lineas: float, extra: float = 0.0) -> float:
    return round(min(5.0, 5 * math.log(1 + lineas / 16) / math.log(1 + 2500 / 16) + extra), 2)


def impacto(v: float, a: float, e: float) -> float:
    return round((2 * v + 2 * a + e) / 5, 2)


def _patron(i: str) -> re.Pattern:
    return re.compile(rf"(?<![\w-])(?:orchestration-)?{re.escape(i)}(?![\w-])", re.I)


def _git(*a: str, cwd=ROOT) -> str:
    return subprocess.run(["git", *a], cwd=cwd, capture_output=True, text=True).stdout


def lineas_por_issue(ids: list[str], commits: list[dict]) -> dict[str, float]:
    """Un commit que nombra varios issues reparte sus lineas en partes iguales."""
    pats = {i: _patron(i) for i in ids}
    out = dict.fromkeys(ids, 0.0)
    for c in commits:
        nombrados = [i for i, p in pats.items() if p.search(c["msg"])]
        for i in nombrados:
            out[i] += c["n"] / len(nombrados)
    return out


def _commits(ids: list[str]) -> list[dict]:
    """Commits sin merge que nombran algun issue: hash, mensaje, lineas (sin EXCLUIR) y archivos agregados o cambiados."""
    log = subprocess.run(["git", "log", "--no-merges", "--format=%H%x1f%B%x1e"], cwd=ROOT, capture_output=True,
                         text=True, check=True).stdout
    pats = [_patron(i) for i in ids]
    out = []
    for reg in log.split("\x1e"):
        if "\x1f" not in reg:
            continue
        h, msg = reg.strip().split("\x1f", 1)
        if any(p.search(msg) for p in pats):
            filas = [x.split("\t", 2) for x in _git("show", "--numstat", "--format=", h).splitlines() if x.count("\t") >= 2]
            n = sum(int(a) + int(b) for a, b, f in filas if a.isdigit() and b.isdigit() and not f.startswith(EXCLUIR))
            files = _git("show", "--name-only", "--diff-filter=AM", "--format=", h).split()
            out.append({"h": h, "msg": msg, "n": n, "files": files})
    return out


# ---------------------------------------------------------------- mediciones (oraculos)

@contextlib.contextmanager
def _worktree(rev: str):
    d = pathlib.Path(tempfile.mkdtemp(prefix="puntaje-")) / "wt"
    subprocess.run(["git", "worktree", "add", "-q", "--detach", str(d), rev], cwd=ROOT, check=True, capture_output=True)
    try:
        yield d
    finally:
        subprocess.run(["git", "worktree", "remove", "--force", str(d)], cwd=ROOT, capture_output=True)


def _pytest(wt: pathlib.Path, files: list[str]) -> int:
    """Exit de pytest: 0 verde, 1 un test fallo, 2 o mas error de coleccion o de uso."""
    try:
        return subprocess.run([sys.executable, "-m", "pytest", *files, "-q", "-p", "no:cacheprovider"], cwd=wt,
                              capture_output=True, timeout=900).returncode
    except subprocess.TimeoutExpired:
        return 3


def tests_de(c: dict) -> list[str]:
    return [f for f in c["files"] if f.startswith("tests/") and f.endswith(".py")]


def rojo_verde(c: dict) -> dict:
    """Los tests del commit sobre el codigo anterior y sobre el commit. Rojo = exit 1 (un assert fallo); un error de
    import (exit 2) no cuenta: mover un paquete no reproduce ningun problema."""
    tests = tests_de(c)
    if not tests:
        return {"tests": 0}
    with _worktree(c["h"] + "^") as wt:
        _git("checkout", c["h"], "--", *tests, cwd=wt)
        antes = _pytest(wt, tests)
    with _worktree(c["h"]) as wt:
        despues = _pytest(wt, tests)
    return {"tests": len(tests), "antes": antes, "despues": despues, "reproduce": antes == 1 and despues == 0}


def mutacion(c: dict) -> dict:
    """Fuerza de los tests del commit: mutantes en las lineas .py que cambio el commit (fuera de tests/)."""
    sys.path.insert(0, str(ROOT))
    from mmorch.checkers import _mutants
    tests = tests_de(c)
    fuentes = [f for f in c["files"] if f.endswith(".py") and not f.startswith("tests/")]
    if not tests or not fuentes:
        return {}
    muertos = total = 0
    with _worktree(c["h"]) as wt:
        for f in fuentes:
            diff = _git("diff", "-U0", c["h"] + "^", c["h"], "--", f)
            lineas = {n for a, b in re.findall(r"^@@ -\S+ \+(\d+)(?:,(\d+))? @@", diff, re.M)
                      for n in range(int(a), int(a) + int(b or 1))}
            p = wt / f
            orig = p.read_text(encoding="utf-8")
            for m in _mutants(orig, max_n=MAX_MUTANTES - total, lineas=lineas):
                p.write_text(m, encoding="utf-8")
                total += 1
                muertos += _pytest(wt, tests) != 0
                p.write_text(orig, encoding="utf-8")
            if total >= MAX_MUTANTES:
                break
    return {"mutantes": total, "muertos": muertos, "score": round(muertos / total, 2) if total else None}


def _bench(wt: pathlib.Path, gate: str) -> dict | None:
    r = subprocess.run([sys.executable, "-m", "mmorch.gates_bench", "--gate", gate, "--dry"], cwd=wt,
                       capture_output=True, text=True, timeout=600)
    filas = [json.loads(x) for x in r.stdout.splitlines() if x.startswith("{")]
    return filas[-1]["acierto_por_grupo"] if r.returncode == 0 and filas else None


def mejora_gate(c: dict, gate: str) -> dict:
    """Los casos del bench del commit contra el gate anterior y el del commit. V sale del grupo que mas mejoro."""
    with _worktree(c["h"] + "^") as wt:
        _git("checkout", c["h"], "--", "mmorch/gates_bench.py", cwd=wt)
        antes = _bench(wt, gate)
    with _worktree(c["h"]) as wt:
        despues = _bench(wt, gate)
    if antes is None or despues is None:
        return {"error": "el bench no corre en uno de los dos commits"}
    grupo, delta = max(((g, despues[g] - antes.get(g, 0.0)) for g in despues), key=lambda x: x[1])
    return {"grupo": grupo, "antes": antes.get(grupo, 0.0), "despues": despues[grupo], "delta": round(delta, 3)}


def fraccion_corridas(traza: list[str], logs: list[dict]) -> tuple[int, int]:
    """(corridas donde corrio alguno de los gates o etapas de `traza`, corridas con gates registrados)."""
    con = [d for d in logs if d.get("gates") or d.get("stages")]
    usa = sum(any(g.get("id") in traza for g in d.get("gates", [])) or
              any(s.get("stage") in traza for s in d.get("stages", [])) for d in con)
    return usa, len(con)


# ---------------------------------------------------------------- tabla

def _cache() -> dict:
    try:
        return json.loads(CACHE.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {}


def _medido(cache: dict, k: str, f) -> dict:
    """Mide solo si la clave falta, y guarda la cache enseguida: una corrida cortada no repite lo ya medido."""
    if k not in cache:
        cache[k] = f()
        CACHE.parent.mkdir(parents=True, exist_ok=True)
        CACHE.write_text(json.dumps(cache, indent=1), encoding="utf-8")
    return cache[k]


def medir(i: str, p: dict, commits: list[dict], logs: list[dict], cache: dict) -> dict:
    """Componentes medidos de un issue: {V, A, E, mut, fuente}. Solo lo que un oraculo da; el resto queda del JSON."""
    mios = [c for c in commits if _patron(i).search(c["msg"])]
    out: dict = {"fuente": {}}
    for gate in p.get("gates", []):
        c = next((c for c in mios if "mmorch/gates.py" in c["files"] and "mmorch/gates_bench.py" in c["files"]), None)
        if c:
            k = f"bench:{gate}:{c['h']}"
            _medido(cache, k, lambda c=c, gate=gate: mejora_gate(c, gate))
            if "delta" in cache[k]:
                out["V"], out["E"] = round(5 * max(cache[k]["delta"], 0), 2), 5.0
                out["fuente"].update(V=f"bench {gate} {cache[k]['grupo']} {cache[k]['antes']}->{cache[k]['despues']}",
                                     E="bench")
    for c in mios:
        if not tests_de(c):
            continue
        _medido(cache, f"rv:{c['h']}", lambda c=c: rojo_verde(c))
        if cache[f"rv:{c['h']}"].get("reproduce"):
            _medido(cache, f"mut:{c['h']}", lambda c=c: mutacion(c))
            s = cache[f"mut:{c['h']}"].get("score")
            # tests que no matan mutantes en las lineas cambiadas no reproducen el problema (un paquete movido da rojo)
            e = 3.0 if s is not None and s < MUT_MIN else 4.0
            if e > out.get("E", 0.0):
                out["E"] = e
                out["fuente"]["E"] = f"rojo->verde {c['h'][:7]}" + (f", mut {s:.2f} < {MUT_MIN}" if e == 3.0 else "")
            if s is not None:
                out["mut"] = max(out.get("mut", 0.0), s)
    if p.get("traza"):
        usa, n = fraccion_corridas(p["traza"], logs)
        if n:
            out["A"] = round(5 * usa / n, 2)
            out["fuente"]["A"] = f"{usa}/{n} corridas"
    return out


def tabla(puntajes: dict, medir_oraculos: bool = True) -> list[dict]:
    ids = list(puntajes)
    commits = _commits(ids)
    lin = lineas_por_issue(ids, commits)
    logs = [json.loads(f.read_text(encoding="utf-8")) for f in sorted(RUNLOGS.glob("run-log-*.json"))]
    cache = _cache()
    filas = []
    for i, p in puntajes.items():
        m = medir(i, p, commits, logs, cache) if medir_oraculos else {"fuente": {}}
        vae = {k: m.get(k, p[k]) for k in "VAE"}
        estimado = "estimado" in p
        filas.append({"id": i, "titulo": p["titulo"], "estado": p["estado"], "lineas": round(lin[i]),
                      "esfuerzo": p["estimado"] if estimado else esfuerzo(lin[i], p.get("extra", 0.0)),
                      "esfuerzo_estimado": estimado, **vae, "medido": sorted(m["fuente"]), "fuente": m["fuente"],
                      "mut": m.get("mut"), "impacto": impacto(vae["V"], vae["A"], vae["E"])})
    return filas


def main(argv: list[str]) -> int:
    filas = tabla(json.loads(PUNTAJES.read_text(encoding="utf-8")), "--sin-medir" not in argv)
    if "--json" in argv:
        print(json.dumps(filas, ensure_ascii=False))
        return 0
    print("V/A/E en mayuscula = medido; en minuscula = juicio del JSON. mut = mutantes muertos por los tests del issue.")
    for f in sorted(filas, key=lambda f: f["impacto"] - f["esfuerzo"], reverse=True):
        vae = " ".join(f"{k if k in f['medido'] else k.lower()}={f[k]:g}" for k in "VAE")
        mut = f" mut={f['mut']:.2f}" if f["mut"] is not None else ""
        marca = "~" if f["esfuerzo_estimado"] else " "
        print(f"{f['id']:>4} {f['estado']:<9} esfuerzo {f['esfuerzo']:4.2f}{marca} impacto {f['impacto']:4.2f}  "
              f"{vae}{mut}  {f['titulo']}")
        for k, v in f["fuente"].items():
            print(f"{'':>15}{k}: {v}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
