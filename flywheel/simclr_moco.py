"""simclr_moco — #2: entrenamiento estilo MoCo (momentum encoder + cola de negativos) sobre
el MISMO encoder/aug/eval que simclr.py (comparacion limpia vs baseline NT-Xent 0.88 radon).

MoCo desacopla #negativos del batch: batch chico + cola grande de keys (de un momentum encoder
EMA, que mantiene los keys consistentes mientras el encoder cambia). Ideal pa CPU/WSL.

Guarda en code_embedder_moco.pt (NO pisa el baseline hasta que GANE el gate).
Corre: ~/flywheel/bin/python flywheel/simclr_moco.py [N] [EPOCHS] [HID] [QUEUE] [TEMP]
"""
from __future__ import annotations
import sys, copy
import numpy as np
import torch
import torch.nn as nn
import torch.nn.functional as F

import simclr as S   # mismo dir; reusa tokenizer/Encoder/Projector/augment/encode/load/cv_auc

SEED = S.SEED
torch.manual_seed(SEED)


@torch.no_grad()
def _ema(target: nn.Module, online: nn.Module, m: float = 0.999):
    for pt, po in zip(target.parameters(), online.parameters()):
        pt.data = pt.data * m + po.data * (1.0 - m)


def main():
    import random
    N = int(sys.argv[1]) if len(sys.argv) > 1 else 0
    EPOCHS = int(sys.argv[2]) if len(sys.argv) > 2 else 16
    HID = int(sys.argv[3]) if len(sys.argv) > 3 else 192
    K = int(sys.argv[4]) if len(sys.argv) > 4 else 4096       # tamaño de la cola
    TEMP = float(sys.argv[5]) if len(sys.argv) > 5 else 0.2

    codes, y = S.load(N)
    print(f"n={len(codes)} epochs={EPOCHS} hid={HID} queue={K} temp={TEMP}", flush=True)
    toks = [S.tokenize_code(c) for c in codes]
    vocab = S.build_vocab(toks)
    print(f"vocab={len(vocab)}", flush=True)

    enc = S.Encoder(len(vocab), hid=HID); proj = S.Projector(enc.out_dim)
    enc_m = copy.deepcopy(enc); proj_m = copy.deepcopy(proj)   # momentum (key) encoder
    for p in list(enc_m.parameters()) + list(proj_m.parameters()):
        p.requires_grad = False
    opt = torch.optim.Adam(list(enc.parameters()) + list(proj.parameters()), lr=2e-3)

    pdim = proj.net[-1].out_features
    queue = F.normalize(torch.randn(pdim, K), dim=0)
    qptr = 0
    rng = random.Random(SEED); idx = list(range(len(toks))); BS = 128

    enc.train(); proj.train()
    for ep in range(EPOCHS):
        rng.shuffle(idx); total = 0.0; nb = 0
        for s in range(0, len(idx), BS):
            batch = idx[s:s + BS]
            if len(batch) < 4:
                continue
            v1 = torch.tensor([S.encode(S.augment(toks[i], rng), vocab) for i in batch])
            v2 = torch.tensor([S.encode(S.augment(toks[i], rng), vocab) for i in batch])
            q = proj(enc(v1))                              # query (online)
            with torch.no_grad():
                _ema(enc_m, enc); _ema(proj_m, proj)
                k = proj_m(enc_m(v2))                      # key (momentum), detached
            l_pos = (q * k).sum(1, keepdim=True)           # (B,1)
            l_neg = q @ queue.clone().detach()             # (B,K)
            logits = torch.cat([l_pos, l_neg], 1) / TEMP
            labels = torch.zeros(q.size(0), dtype=torch.long)
            loss = F.cross_entropy(logits, labels)
            opt.zero_grad(); loss.backward(); opt.step()
            # enqueue keys (cola circular)
            b = k.size(0)
            if qptr + b <= K:
                queue[:, qptr:qptr + b] = k.t()
            else:
                first = K - qptr
                queue[:, qptr:] = k.t()[:, :first]; queue[:, :b - first] = k.t()[:, first:]
            qptr = (qptr + b) % K
            total += loss.item(); nb += 1
        print(f"epoch {ep+1}/{EPOCHS} loss={total/max(nb,1):.4f}", flush=True)

    # eval IDENTICO a simclr: encoder frozen -> mean-pool -> radon AUC
    enc.eval(); embs = []
    with torch.no_grad():
        for s in range(0, len(toks), 256):
            chunk = torch.tensor([S.encode(toks[i], vocab) for i in range(s, min(s + 256, len(toks)))])
            embs.append(enc(chunk).numpy())
    X = np.concatenate(embs)
    m, sd = S.cv_auc(X, y)
    print(f"MOCO (frozen, dim={X.shape[1]}, queue={K}): AUC {m:.4f} +/- {sd:.4f}", flush=True)
    torch.save({"enc": enc.state_dict(), "vocab": vocab},
               "/mnt/c/Users/map12/.claude/orchestration/flywheel/code_embedder_moco.pt")
    print("saved code_embedder_moco.pt", flush=True)


if __name__ == "__main__":
    main()
