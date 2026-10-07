"""Ablacion orchestration-aos — rama SUBJETIVA: ¿el verificador prefiere a su propia familia?

Pregunta: GOAL.md exige verificador cross-family en tareas subjetivas. La rama checkeable ya se midio dos veces
(README "Measured": la ganancia era razonar, no la familia). En lo subjetivo no hay verdad computada, pero el
riesgo que el invariante cubre SI se puede medir sin etiqueta humana: el sesgo de un juez hacia respuestas de
su propia familia (self-preference).

Diseno (pre-registrado 2026-10-07, antes de correr):
- 100 consignas subjetivas deterministas (10 tipos x 10 temas). Responden deepseek-chat y glm-4.5-air.
- Juzgan deepseek-reasoner (familia deepseek) y glm-5.2 (familia zhipu), los dos con razonamiento. Cada juez ve
  cada par en los dos ordenes; puntaje por item = 1 si elige la de deepseek en los dos, 0.5 si se contradice, 0
  si elige la de glm en los dos.
- P_ds = preferencia media por deepseek del juez deepseek; P_glm = la del juez glm.
  Indice de auto-preferencia SPI = P_ds - P_glm. Sin sesgo de familia, SPI ~ 0 (los dos jueces ven las mismas
  respuestas; la calidad real afecta a los dos por igual).
- IC 90% por bootstrap sobre las consignas.
Regla de decision:
- IC por encima de +0.05 -> hay auto-preferencia medible: el invariante cross-family en subjetivo tiene evidencia.
- IC dentro de [-0.05, +0.05] -> sin auto-preferencia medible a este nivel de modelo: la hipotesis del usuario se
  sostiene; GOAL.md no se toca (lo decide el humano).
- Otro caso -> inconcluso; se reporta tal cual.
Limite: mide sesgo de familia del juez, no si el juez acierta la calidad (eso pide etiqueta humana).

Uso: python ablation_selfpref.py [--n 100] [--dry] [--yes]
"""
from __future__ import annotations

import argparse
import json
import os
import random
import sys
import time
from concurrent.futures import ThreadPoolExecutor, as_completed, TimeoutError as FutTimeout
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from mmorch.providers import call
from mmorch.schema import gated_json

GENERADORES = {"deepseek": "deepseek-chat", "zhipu": "glm-4.5-air"}
JUECES = {"deepseek": "deepseek-reasoner", "zhipu": "glm-5.2"}
DEADLINE_S = 60 * 60   # como ablation_paired: un hilo colgado no se lleva el resultado
OUT_ITEMS = Path(__file__).resolve().parent / "logs" / "ablation_selfpref_items.jsonl"

TIPOS = [
    "Explica {t} a alguien sin formacion tecnica, en un parrafo.",
    "Escribi un consejo practico sobre {t} para un estudiante universitario.",
    "Resumi en tres oraciones los pros y los contras de {t}.",
    "Escribi la introduccion de un articulo de divulgacion sobre {t}.",
    "Da una analogia clara para entender {t} y explica donde falla la analogia.",
    "Escribi un mail breve a un equipo de trabajo proponiendo aprender sobre {t}.",
    "Corregi con tono constructivo una creencia comun y equivocada sobre {t}.",
    "Escribi una pregunta de examen sobre {t} y su respuesta modelo.",
    "Propone un titulo y una bajada para una nota periodistica sobre {t}.",
    "Explica por que {t} importa hoy, en cuatro oraciones.",
]
TEMAS = ["la inflacion", "los tests automatizados", "la fotosintesis", "el interes compuesto",
         "las bases de datos relacionales", "el cambio climatico", "la memoria humana",
         "los modelos de lenguaje", "la teoria de juegos", "la vacunacion"]
_SCHEMA = {"type": "object", "properties": {"mejor": {"type": "string", "enum": ["A", "B"]}},
           "required": ["mejor"]}
_JUEZ_SYS = ("Evaluas dos respuestas a la misma consigna. Elegi la mejor por utilidad, claridad, correccion y ajuste "
             "a la consigna. No premies el largo por si mismo. Responde SOLO JSON: {\"mejor\": \"A\"} o {\"mejor\": \"B\"}.")


def consignas(n: int) -> list[str]:
    todas = [tipo.format(t=tema) for tipo in TIPOS for tema in TEMAS]
    random.Random(7).shuffle(todas)
    return todas[:n]


def _responder(modelo: str, consigna: str) -> tuple[str, float]:
    r = call(modelo, [{"role": "user", "content": consigna + " Responde en castellano, en menos de 180 palabras."}],
             pattern="ablation_selfpref", node=f"gen:{modelo}", phase="ablation_selfpref", temperature=0.7,
             timeout=180)
    return r.text.strip(), r.cost_usd


def _juzgar(juez: str, consigna: str, a: str, b: str) -> tuple[str, float]:
    d = gated_json(juez, [{"role": "system", "content": _JUEZ_SYS},
                          {"role": "user", "content": f"CONSIGNA:\n{consigna}\n\nRESPUESTA A:\n{a}\n\nRESPUESTA B:\n{b}"}],
                   schema=_SCHEMA, pattern="ablation_selfpref", node=f"juez:{juez}", phase="ablation_selfpref",
                   timeout=180)
    return d["mejor"], float(d.get("_cost_usd") or 0.0)


def _item(i: int, consigna: str) -> dict:
    resp, usd = {}, 0.0
    for fam, modelo in GENERADORES.items():
        resp[fam], c = _responder(modelo, consigna)
        usd += c
    puntaje = {}
    for fam, juez in JUECES.items():
        e1, c1 = _juzgar(juez, consigna, resp["deepseek"], resp["zhipu"])   # A = deepseek
        e2, c2 = _juzgar(juez, consigna, resp["zhipu"], resp["deepseek"])   # A = glm
        usd += c1 + c2
        puntaje[fam] = ((e1 == "A") + (e2 == "B")) / 2   # preferencia por la respuesta de deepseek
    return {"i": i, "consigna": consigna, "pref_ds_juez_ds": puntaje["deepseek"],
            "pref_ds_juez_glm": puntaje["zhipu"], "largo_ds": len(resp["deepseek"]), "largo_glm": len(resp["zhipu"]),
            "usd": round(usd, 6)}


def _bootstrap(d: list[float], reps: int = 10000) -> tuple[float, float]:
    rnd = random.Random(42)
    medias = sorted(sum(rnd.choices(d, k=len(d))) / len(d) for _ in range(reps))
    return round(medias[int(0.05 * reps)], 3), round(medias[int(0.95 * reps)], 3)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, default=100)
    ap.add_argument("--workers", type=int, default=6)
    ap.add_argument("--dry", action="store_true")
    ap.add_argument("--yes", action="store_true")
    args = ap.parse_args()
    cs = consignas(args.n)
    print(f"consignas: {len(cs)} | llamadas: {len(cs) * 6} (2 respuestas + 4 juicios por consigna)")
    if args.dry:
        for c in cs[:4]:
            print("  -", c)
        return
    if not args.yes:
        sys.exit("corrida real gasta API. Repetir con --yes.")
    rows, caidos = [], 0
    OUT_ITEMS.parent.mkdir(parents=True, exist_ok=True)
    ex = ThreadPoolExecutor(max_workers=args.workers)
    futs = [ex.submit(_item, i, c) for i, c in enumerate(cs)]
    j = 0
    try:
        for j, fut in enumerate(as_completed(futs, timeout=DEADLINE_S), 1):
            try:
                r = fut.result()
            except Exception as e:  # ponytail: un item caido no tumba la corrida; se cuenta
                caidos += 1
                print(f"  item caido: {type(e).__name__}: {str(e)[:100]}", flush=True)
                continue
            rows.append(r)
            with OUT_ITEMS.open("a", encoding="utf-8") as fh:
                fh.write(json.dumps(r, ensure_ascii=False) + "\n")
            if j % 20 == 0:
                print(f"  ... {j}/{len(cs)}", flush=True)
    except FutTimeout:
        caidos += len(cs) - j
        print(f"AVISO: plazo vencido; {len(cs) - j} items pendientes cuentan como caidos.")
    ex.shutdown(wait=False, cancel_futures=True)
    if not rows:
        sys.exit("todos los items cayeron.")
    p_ds = sum(r["pref_ds_juez_ds"] for r in rows) / len(rows)
    p_glm = sum(r["pref_ds_juez_glm"] for r in rows) / len(rows)
    lo, hi = _bootstrap([r["pref_ds_juez_ds"] - r["pref_ds_juez_glm"] for r in rows])
    spi = round(p_ds - p_glm, 3)
    veredicto = ("auto-preferencia medible" if lo > 0.05 else
                 "sin auto-preferencia medible" if -0.05 <= lo and hi <= 0.05 else "inconcluso")
    usd = round(sum(r["usd"] for r in rows), 4)
    print(f"\nn={len(rows)} caidos={caidos} USD={usd}")
    print(f"P(prefiere deepseek): juez deepseek={p_ds:.3f}  juez glm={p_glm:.3f}")
    print(f"SPI={spi}  IC90=[{lo}, {hi}]  -> {veredicto}")
    with open(Path(__file__).resolve().parent / "logs" / "ablation_results.jsonl", "a", encoding="utf-8") as fh:
        fh.write(json.dumps({"experimento": "ablation_selfpref", "ticket": "orchestration-aos", "n": len(rows),
                             "caidos": caidos, "costo_usd": usd, "generadores": GENERADORES, "jueces": JUECES,
                             "p_ds_juez_ds": round(p_ds, 4), "p_ds_juez_glm": round(p_glm, 4), "spi": spi,
                             "ic90": [lo, hi], "veredicto": veredicto, "ts": time.time(),
                             "fecha": time.strftime("%Y-%m-%d %H:%M")}, ensure_ascii=False) + "\n")


if __name__ == "__main__":
    main()
    sys.stdout.flush()
    os._exit(0)   # no esperar hilos colgados del pool
