"""Puntaje de impacto y esfuerzo por issue de bd (rubrica en docs/agents/rubrica-impacto-esfuerzo.md).

Uso: python scripts/puntaje_issues.py [--json]
Esfuerzo = lineas de git de los commits que nombran el issue (+ `extra`); impacto = (2V + 2A + E) / 5 del JSON.
"""
import json
import math
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
PUNTAJES = ROOT / "docs/agents/puntajes.json"
EXCLUIR = ("vault/", ".beads/", "README.md", "docs/generated/")


def esfuerzo(lineas: float, extra: float = 0.0) -> float:
    return round(min(5.0, 5 * math.log(1 + lineas / 16) / math.log(1 + 2500 / 16) + extra), 2)


def impacto(v: float, a: float, e: float) -> float:
    return round((2 * v + 2 * a + e) / 5, 2)


def _patron(i: str) -> re.Pattern:
    return re.compile(rf"(?<![\w-])(?:orchestration-)?{re.escape(i)}(?![\w-])", re.I)


def lineas_por_issue(ids: list[str], commits: list[tuple[str, str, int]]) -> dict[str, float]:
    """commits = (hash, mensaje, lineas); un commit que nombra varios issues reparte sus lineas en partes iguales."""
    pats = {i: _patron(i) for i in ids}
    out = dict.fromkeys(ids, 0.0)
    for _, msg, n in commits:
        nombrados = [i for i, p in pats.items() if p.search(msg)]
        for i in nombrados:
            out[i] += n / len(nombrados)
    return out


def _commits(ids: list[str]) -> list[tuple[str, str, int]]:
    log = subprocess.run(["git", "log", "--no-merges", "--format=%H%x1f%B%x1e"], cwd=ROOT, capture_output=True,
                         text=True, check=True).stdout
    pats = [_patron(i) for i in ids]
    out = []
    for reg in log.split("\x1e"):
        if "\x1f" not in reg:
            continue
        h, msg = reg.strip().split("\x1f", 1)
        if any(p.search(msg) for p in pats):
            num = subprocess.run(["git", "show", "--numstat", "--format=", h], cwd=ROOT, capture_output=True, text=True).stdout
            n = sum(int(a) + int(b) for a, b, f in (x.split("\t", 2) for x in num.splitlines() if x.count("\t") >= 2)
                    if a.isdigit() and b.isdigit() and not f.startswith(EXCLUIR))
            out.append((h, msg, n))
    return out


def tabla(puntajes: dict) -> list[dict]:
    ids = list(puntajes)
    lin = lineas_por_issue(ids, _commits(ids))
    filas = []
    for i, p in puntajes.items():
        estimado = "estimado" in p
        esf = p["estimado"] if estimado else esfuerzo(lin[i], p.get("extra", 0.0))
        filas.append({"id": i, "titulo": p["titulo"], "estado": p["estado"], "lineas": round(lin[i]),
                      "esfuerzo": esf, "esfuerzo_estimado": estimado, "impacto": impacto(p["V"], p["A"], p["E"])})
    return filas


def main(argv: list[str]) -> int:
    filas = tabla(json.loads(PUNTAJES.read_text(encoding="utf-8")))
    if "--json" in argv:
        print(json.dumps(filas, ensure_ascii=False))
        return 0
    for f in sorted(filas, key=lambda f: f["impacto"] - f["esfuerzo"], reverse=True):
        marca = "~" if f["esfuerzo_estimado"] else " "
        print(f"{f['id']:>4} {f['estado']:<9} esfuerzo {f['esfuerzo']:4.2f}{marca} impacto {f['impacto']:4.2f}"
              f"  ({f['lineas']} lineas) {f['titulo']}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
