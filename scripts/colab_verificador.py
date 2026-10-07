"""orchestration-pcb: verificador open-weight en una GPU de Colab sobre el mismo gold set pareado de ablation_paired.

Pregunta: ¿un modelo abierto chico, servido sin costo por llamada, verifica tan bien como los verificadores de API?
README "Measured": deepseek-reasoner y glm-5.2 dan 1.00/1.00 sobre los 350 items (seed 42).

    python scripts/colab_verificador.py preparar   # escribe logs/colab/prompts.jsonl (mismo prompt que la ablacion)
    (en Colab, T4) colab exec -f scripts/colab_vllm_juez.py   # lee prompts.jsonl, escribe salidas.jsonl
    python scripts/colab_verificador.py analizar   # parsea con _parse_verdict y compara contra la verdad computada
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RAIZ))
DIR = RAIZ / "logs" / "colab"


def preparar(n: int = 350, seed: int = 42) -> None:
    import ablation_paired as A
    DIR.mkdir(parents=True, exist_ok=True)
    with (DIR / "prompts.jsonl").open("w", encoding="utf-8") as fh:
        for g in A.build_gold(n, seed):
            art = f"PROBLEMA:\n{g['problem']}\n\nRESPUESTA PROPUESTA: {g['proposed']}"
            fh.write(json.dumps({"i": g["i"], "is_correct": g["is_correct"],
                                 "messages": [{"role": "system", "content": A._VERIFY_SYS},
                                              {"role": "user", "content": art}]}, ensure_ascii=False) + "\n")
    print("prompts:", n, "->", DIR / "prompts.jsonl")


def analizar() -> None:
    from mmorch.patterns import _parse_verdict
    filas = [json.loads(x) for x in (DIR / "salidas.jsonl").read_text(encoding="utf-8").splitlines() if x.strip()]
    tp = tn = fp = fn = ilegibles = 0
    for f in filas:
        texto = f["texto"].split("</think>")[-1]   # Qwen3 piensa antes de responder
        passed, _, refs = _parse_verdict(texto)
        ilegibles += any("unparseable" in r for r in refs)
        if f["is_correct"]:
            tn += passed
            fp += not passed
        else:
            tp += not passed
            fn += passed
    sens, esp = tp / max(1, tp + fn), tn / max(1, tn + fp)
    res = {"experimento": "colab_verificador", "ticket": "orchestration-pcb", "modelo": filas[0].get("modelo"),
           "n": len(filas), "sensibilidad": round(sens, 3), "especificidad": round(esp, 3),
           "acc_balanceada": round((sens + esp) / 2, 3), "ilegibles": ilegibles,
           "segundos": filas[0].get("segundos_total")}
    print(json.dumps(res, ensure_ascii=False))
    with (RAIZ / "logs" / "ablation_results.jsonl").open("a", encoding="utf-8") as fh:
        fh.write(json.dumps(res, ensure_ascii=False) + "\n")


if __name__ == "__main__":
    {"preparar": preparar, "analizar": analizar}[sys.argv[1]]()
