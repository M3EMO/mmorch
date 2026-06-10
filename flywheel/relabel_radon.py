"""relabel_radon — SANITY del harness: re-etiqueta el dataset con un oraculo ESTRUCTURAL
que SI esta en el texto (radon maintainability index, split por mediana). Si SimCLR no
bate azar aca, el encoder/probe esta roto (no es la label). Semi-circular a proposito:
solo valida que el pipeline aprende algo medible.
"""
from __future__ import annotations
import json, pathlib, sys
import textwrap

ROOT = pathlib.Path(__file__).resolve().parents[1]
SRC = ROOT / "logs" / "codequality_dataset.jsonl"
OUT = ROOT / "logs" / "radon_dataset.jsonl"


def mi(code: str) -> float | None:
    from radon.metrics import mi_visit
    try:
        return mi_visit(textwrap.dedent(code), multi=True)
    except Exception:
        return None


def main():
    rows = []
    for line in open(SRC, encoding="utf-8"):
        d = json.loads(line)
        m = mi(d["code"])
        if m is None:
            continue
        rows.append((d["code"], m))
    rows.sort(key=lambda r: r[1])
    n = len(rows)
    med = rows[n // 2][1]
    out = [{"label": 0 if m < med else 1, "code": code, "mi": round(m, 2)}
           for code, m in rows]
    import random
    random.Random(0).shuffle(out)   # IMPORTANTE: si no, un prefijo es mono-clase
    with open(OUT, "w", encoding="utf-8") as fh:
        for row in out:
            fh.write(json.dumps(row, ensure_ascii=False) + "\n")
    print(f"radon_dataset: {n} funcs, median MI={med:.1f} -> {OUT}")


if __name__ == "__main__":
    main()
