"""orchestration-4rf: notas de campo de corridas previas del SDLC, congeladas antes del lote (pre-registro §5 y §7).

Uso:  python scripts/notas_campo.py manifiesto.json salida/ [--dry]

manifiesto.json = {"items": [{"id": "ssb-F7a2", "repo": "/ruta", "commit": "<sha con docs/sdlc/run-log.json>"}],
                   "runs":  [{"repo": "/ruta", "commit": "<sha>"}]}
Los items tambien cuentan como corridas previas de otros items. Salida: salida/<id>.txt (el texto que lee
SDLC_NOTAS), salida/notas.json (corridas elegibles, nota por corrida y sha256 por item) y salida/notas.sha256.
--dry no llama a la API: cada nota es un resumen fijo del run-log (para probar el arnes).
"""
from __future__ import annotations

import hashlib
import json
import pathlib
import subprocess
import sys

PLANTILLA = """Tarea: <una línea>
Stack y toolchain: <lenguaje, comando de compilación, comando de tests>
Archivos tocados: <lista de plan_files>
Problemas (síntoma → causa → arreglo):
- <síntoma tal como lo reportó el gate> → <causa probable> → <qué lo destrabó, o "no se destrabó">
Cómo se verificó: <aceptación y suite>"""

PROMPT = """Abajo va el run-log JSON de una corrida del pipeline SDLC sobre un repo.
Escribí una nota de campo para la próxima corrida sobre el MISMO repo, con esta plantilla exacta:
{plantilla}
Reglas: máximo 200 palabras; nada de código; nombrá archivos, comandos y gates tal como aparecen en el log;
si el log no muestra la causa o el arreglo, escribí "desconocido". Devolvé solo la nota.
RUN-LOG:
{run_log}"""

TOPE_TOKENS = 1500
MAX_LOG = 40000  # shortcut: el run-log entra truncado a 40k caracteres; subir si una nota sale "desconocido" por eso


def git(repo: str, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(["git", "-C", repo, *args], capture_output=True, text=True, encoding="utf-8")


def run_log(repo: str, commit: str) -> dict:
    p = git(repo, "show", f"{commit}:docs/sdlc/run-log.json")
    if p.returncode:
        raise SystemExit(f"{repo}@{commit}: sin docs/sdlc/run-log.json ({p.stderr.strip()[:200]})")
    return json.loads(p.stdout)


def fecha(repo: str, commit: str) -> int:
    return int(git(repo, "show", "-s", "--format=%ct", commit).stdout.strip())


def elegibles(item: dict, runs: list[dict]) -> list[dict]:
    """§5: misma repo, empezo antes y su commit es ancestro del commit base del item. De la mas reciente a la mas vieja."""
    base = run_log(item["repo"], item["commit"])["base_sha"]
    t_i = fecha(item["repo"], item["commit"])
    ok = [r for r in runs if r["repo"] == item["repo"] and r["commit"] != item["commit"]
          and fecha(r["repo"], r["commit"]) < t_i
          and git(r["repo"], "merge-base", "--is-ancestor", r["commit"], base).returncode == 0]
    return sorted(ok, key=lambda r: -fecha(r["repo"], r["commit"]))


def prohibidos(item: dict) -> set[str]:
    """§7.5: archivos que el item agrega (su test de aceptacion y su codigo nuevo). Una nota que los nombra filtra."""
    base = run_log(item["repo"], item["commit"])["base_sha"]
    nuevos = git(item["repo"], "diff", "--name-only", "--diff-filter=A", base, item["commit"]).stdout.split()
    return {n for n in nuevos if not n.startswith("docs/sdlc/")}


def fuga(texto: str, item: dict) -> list[str]:
    return sorted(f for f in prohibidos(item) if f in texto or pathlib.PurePosixPath(f).name in texto)


def nota(repo: str, commit: str, dry: bool) -> str:
    log = run_log(repo, commit)
    if dry:
        return f"Tarea: {log.get('task', '?')}\nProblemas: {len(log.get('gate_rejects', []))} rechazos de gates"
    from mmorch.providers import call
    texto = json.dumps(log, ensure_ascii=False)[:MAX_LOG]
    return call("deepseek-chat", PROMPT.format(plantilla=PLANTILLA, run_log=texto), pattern="notas-campo",
                phase="4rf-notas", temperature=0.0, timeout=120).text.strip()


def bloque(notas: list[str]) -> str:
    """Tope de §7.5: entran las mas recientes hasta 1500 tokens (4 caracteres por token)."""
    out, usados = [], 0
    for n in notas:
        t = len(n) // 4 + 1
        if usados + t > TOPE_TOKENS:
            break
        out.append(n)
        usados += t
    return "\n\n".join(out)


def main(manifiesto: str, salida: str, dry: bool = False) -> dict:
    m = json.loads(pathlib.Path(manifiesto).read_text(encoding="utf-8"))
    runs = m["runs"] + [{"repo": i["repo"], "commit": i["commit"]} for i in m["items"]]
    out = pathlib.Path(salida)
    out.mkdir(parents=True, exist_ok=True)
    cache: dict[str, str] = {}
    res = {}
    for item in m["items"]:
        elig = elegibles(item, runs)
        for r in elig:
            key = f"{r['repo']}@{r['commit']}"
            if key not in cache:
                cache[key] = nota(r["repo"], r["commit"], dry)
        texto = bloque([cache[f"{r['repo']}@{r['commit']}"] for r in elig])
        filtrados = fuga(texto, item)
        if filtrados:
            raise SystemExit(f"{item['id']}: la nota nombra archivos que el item agrega: {filtrados}")
        (out / f"{item['id']}.txt").write_text(texto, encoding="utf-8")
        res[item["id"]] = {"elegibles": [r["commit"] for r in elig], "primaria": bool(elig),
                           "sha256": hashlib.sha256(texto.encode("utf-8")).hexdigest()}
    doc = {"items": res, "notas": cache, "dry": dry}
    raw = json.dumps(doc, ensure_ascii=False, indent=1, sort_keys=True)
    (out / "notas.json").write_text(raw, encoding="utf-8")
    (out / "notas.sha256").write_text(hashlib.sha256(raw.encode("utf-8")).hexdigest() + "  notas.json\n", encoding="utf-8")
    return doc


if __name__ == "__main__":
    args = [a for a in sys.argv[1:] if a != "--dry"]
    if len(args) != 2:
        raise SystemExit(__doc__)
    d = main(*args, dry="--dry" in sys.argv)
    for k, v in d["items"].items():
        print(k, "primaria" if v["primaria"] else "sin corridas elegibles", len(v["elegibles"]), v["sha256"][:12])
