"""oracle_diverse — crecer dataset con POSITIVOS FUNCIONALES DIVERSOS (el fix de #1). Por cada
spec, DeepSeek genera soluciones con ENFOQUES distintos (iterativo/recursivo/funcional/one-liner/
verboso) a temp alta -> equivalentes funcionales que NO se parecen sintacticamente. Cada una se
VERIFICA por ejecucion (python_exec). Solo passers entran. Resultado: pares same-spec que son
funcional-equiv pero diversos -> el eval funcional deja de estar saturado por superficie, y #1
tiene señal real que aprender.

Reusa los SPECS de oracle_dataset. Out: logs/oracle_diverse.jsonl {code, spec, label, approach}.
"""
from __future__ import annotations
import json, sys, pathlib

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
OUT = ROOT / "logs" / "oracle_diverse.jsonl"

APPROACHES = [
    "de forma ITERATIVA con loops explicitos",
    "de forma RECURSIVA",
    "estilo FUNCIONAL (map/filter/comprehensions, sin loops imperativos)",
    "lo mas CORTO posible (one-liner o casi)",
    "VERBOSO y paso-a-paso con variables intermedias",
    "usando la libreria estandar (collections/itertools/functools) idiomaticamente",
]


def main():
    from mmorch import fan_out
    from mmorch.checkers import check
    from flywheel.oracle_dataset import SPECS, extract_code
    K = int(sys.argv[1]) if len(sys.argv) > 1 else len(APPROACHES)
    REPS = int(sys.argv[2]) if len(sys.argv) > 2 else 1   # repetir (temp alta -> variedad)
    tests = {n: t for n, _, t in SPECS}

    prompts, meta = [], []
    sysmsg = ("Sos programador Python. Implementa EXACTO lo pedido con el enfoque indicado. "
              "Solo el codigo de la funcion en un bloque python, sin explicacion.")
    for _ in range(REPS):
        for name, spec, _t in SPECS:
            for ap in APPROACHES[:K]:
                prompts.append(f"Implementa {ap}: {spec}\nLa funcion DEBE llamarse `{name}`.")
                meta.append((name, ap))

    print(f"generando {len(prompts)} ({len(SPECS)} specs x {min(K,len(APPROACHES))} enfoques x {REPS} reps)...", flush=True)
    res = fan_out(prompts, gen_model="deepseek-chat", system=sysmsg, max_workers=8, temperature=1.0)

    rows, npass, seen = [], 0, set()
    for (name, ap), r in zip(meta, res):
        txt = getattr(r, "text", "") or ""
        if not txt:
            continue
        code = extract_code(txt)
        if name not in code:
            continue
        h = hash((name, code))
        if h in seen:           # dedup: misma solucion exacta no suma
            continue
        seen.add(h)
        try:
            ok = check("python_exec", code=code + "\n" + tests[name]).passed
        except Exception:
            continue
        rows.append({"code": code, "spec": name, "label": 1 if ok else 0, "approach": ap})
        npass += int(ok)
    with open(OUT, "w", encoding="utf-8") as f:
        for r in rows:
            f.write(json.dumps(r, ensure_ascii=False) + "\n")
    print(f"oracle_diverse: {len(rows)} | pass={npass} fail={len(rows)-npass} -> {OUT}", flush=True)


if __name__ == "__main__":
    main()
