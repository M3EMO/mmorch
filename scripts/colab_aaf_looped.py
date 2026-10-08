"""orchestration-aaf (spike descartable): ¿el looping ayuda a MANIPULAR conocimiento y no a ALMACENARLO?

Banco sintetico estilo Physics of LLMs 3.x: personas con ano de nacimiento, ciudad y trabajo. Las biografias de TODAS
las personas van al entrenamiento (en varias permutaciones); las preguntas solo de la mitad. Se mide en la otra mitad:
- almacenamiento: "ano de X" -> token del ano (extraer lo que la bio dijo);
- manipulacion: "X nacio en ano par?" -> si/no (extraer y operar, sin cadena de pensamiento).
Modelos decoder-only chicos, mismos hiperparametros: A = 2 capas x 1 vuelta, B = las MISMAS 2 capas x 4 vueltas
(pesos compartidos, mismo numero de parametros que A), C = 8 capas distintas (mismo computo que B).
ponytail: vueltas fijas, sin halting adaptativo (PonderNet); alcanza para la pregunta del ticket.

    python scripts/colab_aaf_looped.py --pasos 200   # prueba local en CPU
    (Colab T4) python colab_aaf_looped.py --pasos 6000 --salida /content/aaf.json
"""
import argparse
import json
import random
import time

import torch
import torch.nn as nn
import torch.nn.functional as F

P, ANOS, CIUDADES, TRABAJOS = 1000, 100, 30, 30
ESPECIALES = ["<pad>", "<bio>", "<q_ano>", "<q_par>", "<sep>", "si", "no", "nacio", "en", "vive", "trabaja", "de"]


def vocab():
    toks = ESPECIALES + [f"p{i}" for i in range(P)] + [f"y{i}" for i in range(ANOS)]
    toks += [f"c{i}" for i in range(CIUDADES)] + [f"t{i}" for i in range(TRABAJOS)]
    return {t: i for i, t in enumerate(toks)}


def datos(seed=0):
    rnd = random.Random(seed)
    gente = [{"n": f"p{i}", "y": rnd.randrange(ANOS), "c": rnd.randrange(CIUDADES), "t": rnd.randrange(TRABAJOS)}
             for i in range(P)]
    train_q = set(rnd.sample(range(P), P // 2))
    bios = []
    for g in gente:
        partes = [["nacio", "en", f"y{g['y']}"], ["vive", "en", f"c{g['c']}"], ["trabaja", "de", f"t{g['t']}"]]
        for _ in range(4):   # Physics of LLMs 3.1: sin aumento de permutaciones la extraccion no generaliza
            rnd.shuffle(partes)
            bios.append(["<bio>", g["n"]] + [t for p in partes for t in p])
    qa = lambda g: [(["<q_ano>", g["n"], "<sep>"], f"y{g['y']}", "ano"),
                    (["<q_par>", g["n"], "<sep>"], "si" if g["y"] % 2 == 0 else "no", "par")]
    entren = [q for i in train_q for q in qa(gente[i])]
    prueba = [q for i in range(P) if i not in train_q for q in qa(gente[i])]
    return bios, entren, prueba


class Modelo(nn.Module):
    def __init__(self, nv, capas, vueltas, d=128, cabezas=4, largo=16):
        super().__init__()
        self.emb, self.pos = nn.Embedding(nv, d), nn.Embedding(largo, d)
        self.capas = nn.ModuleList(nn.TransformerEncoderLayer(d, cabezas, 4 * d, 0.0, batch_first=True, norm_first=True)
                                   for _ in range(capas))
        self.vueltas, self.ln, self.sal = vueltas, nn.LayerNorm(d), nn.Linear(d, nv)

    def forward(self, x):
        h = self.emb(x) + self.pos(torch.arange(x.shape[1], device=x.device))
        mask = nn.Transformer.generate_square_subsequent_mask(x.shape[1], device=x.device)
        for _ in range(self.vueltas):
            for capa in self.capas:
                h = capa(h, src_mask=mask, is_causal=True)
        return self.sal(self.ln(h))


def lote(v, bios, entren, n, dev, rnd):
    seqs = [b for b in rnd.sample(bios, n // 2)] + [q + [a] for q, a, _ in rnd.sample(entren, n // 2)]
    L = max(map(len, seqs))
    x = torch.tensor([[v[t] for t in s] + [0] * (L - len(s)) for s in seqs], device=dev)
    return x


def evaluar(m, v, prueba, dev):
    m.eval()
    ok = {"ano": [0, 0], "par": [0, 0]}
    with torch.no_grad():
        for i in range(0, len(prueba), 256):
            parte = prueba[i:i + 256]
            x = torch.tensor([[v[t] for t in q] for q, _, _ in parte], device=dev)
            pred = m(x)[:, -1].argmax(-1).tolist()
            for (_, a, tipo), p in zip(parte, pred, strict=True):
                ok[tipo][0] += p == v[a]
                ok[tipo][1] += 1
    m.train()
    return {k: round(a / b, 3) for k, (a, b) in ok.items()}


def correr(nombre, capas, vueltas, pasos, dev, v, bios, entren, prueba, semilla=0):
    torch.manual_seed(semilla)
    rnd = random.Random(1 + semilla)
    m = Modelo(len(v), capas, vueltas).to(dev)
    opt = torch.optim.AdamW(m.parameters(), lr=1e-3, weight_decay=0.1)
    t0 = time.time()
    for _ in range(pasos):
        x = lote(v, bios, entren, 256, dev, rnd)
        logits = m(x[:, :-1])
        loss = F.cross_entropy(logits.reshape(-1, logits.shape[-1]), x[:, 1:].reshape(-1), ignore_index=0)
        opt.zero_grad()
        loss.backward()
        opt.step()
    r = {"modelo": nombre, "semilla": semilla, "capas": capas, "vueltas": vueltas, "params": sum(p.numel() for p in m.parameters()),
         "loss": round(loss.item(), 3), "segundos": round(time.time() - t0), "prueba": evaluar(m, v, prueba, dev),
         "entren": evaluar(m, v, entren, dev)}
    print(json.dumps(r), flush=True)
    return r


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--pasos", type=int, default=200)
    ap.add_argument("--salida", default="")
    ap.add_argument("--semillas", default="0")
    a = ap.parse_args()
    dev = "cuda" if torch.cuda.is_available() else "cpu"
    v = vocab()
    bios, entren, prueba = datos()
    res = [correr(n, c, k, a.pasos, dev, v, bios, entren, prueba, int(s)) for s in a.semillas.split(",")
           for n, c, k in (("A_2capas_1vuelta", 2, 1), ("B_2capas_4vueltas", 2, 4), ("C_8capas_1vuelta", 8, 1))]
    if a.salida:
        json.dump(res, open(a.salida, "w"), indent=1)
