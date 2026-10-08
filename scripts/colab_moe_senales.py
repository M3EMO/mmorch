"""orchestration-pcb, parte MoE: puerta de la etapa 1 del plan de confianza por token, con verdad computada.

Plan del usuario (vault: plan-moe-capa-de-confianza-por-token): la entropia del enrutador tiene que predecir el error
mejor que la entropia de la salida, con menos de 5% de latencia extra. mmorch pone etiquetas sin juez: los 350
problemas aritmeticos de ablation_paired, con respuesta computada.

Pre-registro (2026-10-07, antes de correr). Modelo: allenai/OLMoE-1B-7B-0125-Instruct en 4 bits (el router en fp16).
Por problema: respuesta greedy corta; error = el numero final no es la verdad. Senales sobre los tokens de la
respuesta (forward con teacher forcing): (1) entropia media de la salida; (2) entropia media del enrutador, promediada
sobre capas. Metrica: AUROC para predecir error, IC 90% por bootstrap sobre problemas.
Regla: la senal del enrutador pasa la puerta si su AUROC supera al de la salida por >= 0.05 con el IC de la diferencia
por encima de 0, y la latencia extra de pedir router_logits es < 5%.

    python scripts/colab_moe_senales.py preparar            # logs/colab/moe_problemas.jsonl
    (Colab T4) python colab_moe_senales.py correr [n]      # /content/moe_salidas.jsonl
    python scripts/colab_moe_senales.py analizar            # AUROC + bootstrap + latencia
"""
import json
import random
import re
import sys
import time
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[1]
MODELO = "allenai/OLMoE-1B-7B-0125-Instruct"


def preparar() -> None:
    sys.path.insert(0, str(RAIZ))
    import ablation_paired as A
    d = RAIZ / "logs" / "colab"
    d.mkdir(parents=True, exist_ok=True)
    with (d / "moe_problemas.jsonl").open("w", encoding="utf-8") as fh:
        for g in A.build_gold(350, 42):
            fh.write(json.dumps({"i": g["i"], "problem": g["problem"], "truth": g["truth"]}, ensure_ascii=False) + "\n")


def preparar_facil(n: int = 300, seed: int = 7) -> None:
    """2da corrida: los 350 problemas de ablation_paired son demasiado dificiles para OLMoE-1B-7B (3/70 aciertos con
    razonamiento). Cuentas de 2-3 digitos con verdad computada; hace falta que el modelo acierte una parte."""
    rnd = random.Random(seed)
    d = RAIZ / "logs" / "colab"
    with (d / "moe_problemas.jsonl").open("w", encoding="utf-8") as fh:
        for i in range(n):
            a, b, c = rnd.randint(12, 999), rnd.randint(3, 99), rnd.randint(2, 9)
            prob, verdad = rnd.choice([(f"Cuanto es {a} + {b}?", a + b), (f"Cuanto es {a} - {b}?", a - b),
                                       (f"Cuanto es {b} x {c}?", b * c), (f"Cuanto es {a} x {c}?", a * c),
                                       (f"Cuanto es ({b} + {c}) x {c}?", (b + c) * c)])
            fh.write(json.dumps({"i": i, "problem": prob, "truth": verdad}) + "\n")


def _entropia(logits):
    import torch
    p = torch.softmax(logits.float(), -1)
    return -(p * torch.log(p + 1e-12)).sum(-1)


def correr(n: int = 350, max_tokens: int = 256) -> None:
    import torch
    from transformers import AutoModelForCausalLM, AutoTokenizer, BitsAndBytesConfig
    tok = AutoTokenizer.from_pretrained(MODELO)
    q = BitsAndBytesConfig(load_in_4bit=True, bnb_4bit_compute_dtype=torch.float16, llm_int8_skip_modules=["gate", "lm_head"])
    m = AutoModelForCausalLM.from_pretrained(MODELO, quantization_config=q, device_map="cuda")
    m.eval()
    filas = [json.loads(x) for x in open("/content/moe_problemas.jsonl", encoding="utf-8")][:n]
    t_con = t_sin = 0.0
    with open("/content/moe_salidas.jsonl", "w", encoding="utf-8") as fh:
        for f in filas:
            # 1ra corrida (16 tokens, solo el numero): OLMoE explicaba igual y acerto 3/350; sin aciertos no hay AUROC
            msgs = [{"role": "user", "content": f"{f['problem']}\nPensa paso a paso, breve, y termina con 'Respuesta: <numero>'."}]
            ids = tok.apply_chat_template(msgs, add_generation_prompt=True, return_tensors="pt")
            ids = (ids["input_ids"] if hasattr(ids, "keys") else ids).to("cuda")   # transformers 5 devuelve un dict
            with torch.no_grad():
                gen = m.generate(ids, max_new_tokens=max_tokens, do_sample=False)
                resp = gen[:, ids.shape[1]:]
                texto = tok.decode(resp[0], skip_special_tokens=True)
                torch.cuda.synchronize(); t0 = time.time()
                m(gen)
                torch.cuda.synchronize(); t1 = time.time()
                out = m(gen, output_router_logits=True)
                torch.cuda.synchronize(); t2 = time.time()
            t_sin += t1 - t0
            t_con += t2 - t1
            a, b = ids.shape[1] - 1, gen.shape[1] - 1           # posiciones que predicen los tokens de la respuesta
            h_sal = _entropia(out.logits[0, a:b]).mean().item()
            L = gen.shape[1]
            h_rut = torch.stack([_entropia(r.view(-1, r.shape[-1])[-L:][a:b]).mean() for r in out.router_logits]).mean().item()
            final = texto.rsplit("Respuesta", 1)[-1]
            nums = re.findall(r"-?\d+", final.replace(",", "").replace(".", ""))
            ok = bool(nums) and int(nums[-1]) == int(f["truth"])
            fh.write(json.dumps({"i": f["i"], "texto": texto, "ok": ok, "h_salida": h_sal, "h_router": h_rut,
                                 "t_sin": t1 - t0, "t_con": t2 - t1}) + "\n")
    print("listo", len(filas), "latencia extra router_logits:", round(t_con / t_sin - 1, 3))


def _auroc(pos, neg):
    if not pos or not neg:
        return float("nan")
    return sum((p > q) + 0.5 * (p == q) for p in pos for q in neg) / (len(pos) * len(neg))


def analizar() -> None:
    filas = [json.loads(x) for x in (RAIZ / "logs" / "colab" / "moe_salidas.jsonl").read_text(encoding="utf-8").splitlines()]
    def aurocs(fs):
        err, bien = [f for f in fs if not f["ok"]], [f for f in fs if f["ok"]]
        return (_auroc([f["h_salida"] for f in err], [f["h_salida"] for f in bien]),
                _auroc([f["h_router"] for f in err], [f["h_router"] for f in bien]))
    sal, rut = aurocs(filas)
    rnd, difs = random.Random(42), []
    for _ in range(2000):
        s, r = aurocs(rnd.choices(filas, k=len(filas)))
        if s == s and r == r:   # sin NaN: una muestra sin aciertos o sin errores no aporta
            difs.append(r - s)
    difs.sort()
    lat = sum(f["t_con"] for f in filas) / sum(f["t_sin"] for f in filas) - 1
    res = {"experimento": "colab_moe_senales", "ticket": "orchestration-pcb", "modelo": MODELO, "n": len(filas),
           "aciertos": sum(f["ok"] for f in filas), "auroc_salida": round(sal, 3), "auroc_router": round(rut, 3),
           "dif_ic90": [round(difs[len(difs) // 20], 3), round(difs[-(len(difs) // 20)], 3)], "latencia_extra": round(lat, 3),
           "pasa_puerta": bool(rut - sal >= 0.05 and difs[len(difs) // 20] > 0 and lat < 0.05)}
    print(json.dumps(res, ensure_ascii=False))
    with (RAIZ / "logs" / "ablation_results.jsonl").open("a", encoding="utf-8") as fh:
        fh.write(json.dumps(res, ensure_ascii=False) + "\n")


if __name__ == "__main__":
    cmd = sys.argv[1]
    if cmd == "correr":
        correr(*(int(x) for x in sys.argv[2:4]))
    else:
        {"preparar": preparar, "preparar_facil": preparar_facil, "analizar": analizar}[cmd]()
