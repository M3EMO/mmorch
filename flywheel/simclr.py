"""simclr — el FLYWHEEL: entrena un encoder de codigo con contraste (NT-Xent), sin
imitar al LLM. Vistas positivas = aug semantico-preservante (rename de identificadores
+ drop comments). Negativos = el resto del batch. La hipotesis: aprender invariancia a
spelling/format fuerza al encoder a representar ESTRUCTURA, no lexico de superficie —
justo lo que a bge-small le falta (probado: AUC~azar).

Eval honesto: encoder FROZEN -> mean-pool embed -> logreg 5-fold AUC sobre la misma
label que baseline.py. Si bate a bge-small/struct, el flywheel anda.

Corre bajo WSL: ~/flywheel/bin/python flywheel/simclr.py [N] [EPOCHS]
Lee dataset por /mnt/c. CPU-only. Determinista (seed fijo).
"""
from __future__ import annotations
import json, sys, re, io, tokenize, keyword, builtins, math, random
import numpy as np
import torch
import torch.nn as nn
import torch.nn.functional as F

SEED = 0
random.seed(SEED); np.random.seed(SEED); torch.manual_seed(SEED)

import os
DATA = os.environ.get("MMORCH_DS",
                      "/mnt/c/Users/map12/.claude/orchestration/logs/codequality_dataset.jsonl")
_BUILTINS = set(dir(builtins)) | set(keyword.kwlist)
_WORD = re.compile(r"[A-Za-z_]\w*|\d+|[^\sA-Za-z0-9_]")


def tokenize_code(src: str) -> list[str]:
    """Tokeniza robusto: prueba tokenize de Python (dedent), cae a regex si rompe."""
    import textwrap
    s = textwrap.dedent(src)
    out: list[str] = []
    try:
        for tok in tokenize.generate_tokens(io.StringIO(s).readline):
            t = tok.type
            if t in (tokenize.COMMENT, tokenize.NL, tokenize.NEWLINE,
                     tokenize.INDENT, tokenize.DEDENT, tokenize.ENCODING,
                     tokenize.ENDMARKER):
                continue
            val = tok.string
            if t == tokenize.STRING:
                out.append("<str>"); continue
            if t == tokenize.NUMBER:
                out.append("<num>"); continue
            if val.strip():
                out.append(val)
    except Exception:
        out = [m.group() for m in _WORD.finditer(s)]
    return out


def identifiers(toks: list[str]) -> list[str]:
    return [t for t in toks if t.isidentifier() and t not in _BUILTINS
            and not t.startswith("<")]


def augment(toks: list[str], rng: random.Random) -> list[str]:
    """Vista positiva: rename consistente de un subset de identificadores + drop strings/nums
    ya normalizados. Preserva estructura, cambia lexico."""
    ids = list(dict.fromkeys(identifiers(toks)))
    if ids:
        k = max(1, int(len(ids) * rng.uniform(0.4, 0.9)))
        chosen = rng.sample(ids, min(k, len(ids)))
        ren = {name: f"v{i}" for i, name in enumerate(chosen)}
        toks = [ren.get(t, t) for t in toks]
    # token dropout leve (robustez)
    if len(toks) > 8:
        toks = [t for t in toks if rng.random() > 0.05]
    return toks


def build_vocab(corpus_toks: list[list[str]], max_size=8000) -> dict[str, int]:
    from collections import Counter
    c = Counter(t for toks in corpus_toks for t in toks)
    vocab = {"<pad>": 0, "<unk>": 1}
    for tok, _ in c.most_common(max_size - 2):
        vocab[tok] = len(vocab)
    return vocab


def encode(toks: list[str], vocab: dict, maxlen=200) -> list[int]:
    ids = [vocab.get(t, 1) for t in toks[:maxlen]]
    return ids + [0] * (maxlen - len(ids))


class Encoder(nn.Module):
    """token-embed -> bi-GRU -> mean-pool (mask-aware). Chico, CPU-friendly."""
    def __init__(self, vocab_size, emb=96, hid=128):
        super().__init__()
        self.emb = nn.Embedding(vocab_size, emb, padding_idx=0)
        self.gru = nn.GRU(emb, hid, batch_first=True, bidirectional=True)
        self.out_dim = hid * 2

    def forward(self, x):
        mask = (x != 0).float().unsqueeze(-1)
        e = self.emb(x)
        h, _ = self.gru(e)
        summed = (h * mask).sum(1)
        cnt = mask.sum(1).clamp(min=1)
        return summed / cnt


class Projector(nn.Module):
    def __init__(self, d, proj=64):
        super().__init__()
        self.net = nn.Sequential(nn.Linear(d, d), nn.ReLU(), nn.Linear(d, proj))

    def forward(self, x):
        return F.normalize(self.net(x), dim=-1)


def nt_xent(z1, z2, temp=0.2):
    """NT-Xent: 2N muestras, positivo = la otra vista, negativos = resto del batch."""
    N = z1.size(0)
    z = torch.cat([z1, z2], 0)              # 2N x d
    sim = z @ z.t() / temp                  # 2N x 2N
    sim.fill_diagonal_(-1e9)
    targets = torch.arange(2 * N)
    targets = torch.where(targets < N, targets + N, targets - N)
    return F.cross_entropy(sim, targets)


def load(limit=None):
    codes, labels = [], []
    with open(DATA, encoding="utf-8") as f:
        for i, line in enumerate(f):
            if limit and i >= limit:
                break
            d = json.loads(line)
            codes.append(d["code"]); labels.append(int(d["label"]))
    return codes, np.array(labels)


def cv_auc(X, y, folds=5):
    from sklearn.linear_model import LogisticRegression
    from sklearn.model_selection import StratifiedKFold
    from sklearn.metrics import roc_auc_score
    skf = StratifiedKFold(n_splits=folds, shuffle=True, random_state=SEED)
    aucs = []
    for tr, te in skf.split(X, y):
        clf = LogisticRegression(max_iter=1000)
        clf.fit(X[tr], y[tr])
        p = clf.predict_proba(X[te])[:, 1]
        aucs.append(roc_auc_score(y[te], p))
    return float(np.mean(aucs)), float(np.std(aucs))


def main():
    N = int(sys.argv[1]) if len(sys.argv) > 1 else None
    EPOCHS = int(sys.argv[2]) if len(sys.argv) > 2 else 8
    codes, y = load(N)
    print(f"n={len(codes)} pos={int(y.sum())} epochs={EPOCHS}", flush=True)

    toks = [tokenize_code(c) for c in codes]
    vocab = build_vocab(toks)
    print(f"vocab={len(vocab)}", flush=True)

    enc = Encoder(len(vocab)); proj = Projector(enc.out_dim)
    opt = torch.optim.Adam(list(enc.parameters()) + list(proj.parameters()), lr=2e-3)
    rng = random.Random(SEED)
    idx = list(range(len(toks)))
    BS = 128

    enc.train(); proj.train()
    for ep in range(EPOCHS):
        rng.shuffle(idx)
        total = 0.0; nb = 0
        for s in range(0, len(idx), BS):
            batch = idx[s:s + BS]
            if len(batch) < 4:
                continue
            v1 = torch.tensor([encode(augment(toks[i], rng), vocab) for i in batch])
            v2 = torch.tensor([encode(augment(toks[i], rng), vocab) for i in batch])
            z1 = proj(enc(v1)); z2 = proj(enc(v2))
            loss = nt_xent(z1, z2)
            opt.zero_grad(); loss.backward(); opt.step()
            total += loss.item(); nb += 1
        print(f"epoch {ep+1}/{EPOCHS} loss={total/max(nb,1):.4f}", flush=True)

    # eval: frozen encoder embeddings -> linear probe
    enc.eval()
    embs = []
    with torch.no_grad():
        for s in range(0, len(toks), 256):
            chunk = torch.tensor([encode(toks[i], vocab)
                                  for i in range(s, min(s + 256, len(toks)))])
            embs.append(enc(chunk).numpy())
    X = np.concatenate(embs)
    m, sd = cv_auc(X, y)
    print(f"SIMCLR (frozen, dim={X.shape[1]}): AUC {m:.4f} +/- {sd:.4f}", flush=True)
    torch.save({"enc": enc.state_dict(), "vocab": vocab}, "/mnt/c/Users/map12/.claude/orchestration/flywheel/code_embedder.pt")
    print("saved code_embedder.pt", flush=True)


if __name__ == "__main__":
    main()
