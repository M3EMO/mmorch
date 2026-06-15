"""simclr_functional — #1: entrenar el encoder con POSITIVOS FUNCIONALES (soluciones DISTINTAS
del mismo spec = equivalentes funcionales) en vez de rename-aug (mismo codigo perturbado).

A/B limpio (aisla SOLO la fuente de positivos):
  - mode 'functional': positivo = OTRA solucion del mismo spec (función-sobre-forma).
  - mode 'augment'   : positivo = aug del mismo codigo (el approach viejo) — baseline.
Mismo encoder/loss/epochs. Eval = P@1 retrieval en specs HELD-OUT (GroupKFold-style split):
¿generaliza la noción de "hacen lo mismo" a tareas no vistas?

Anti-cheat: normaliza el NOMBRE de la función a `<fn>` (todas las del spec lo comparten ->
seria un atajo trivial). Asi el encoder DEBE usar estructura/flujo, no el nombre.

Corre en WSL. Datos: logs/oracle_diverse.jsonl (passers diversos).
"""
from __future__ import annotations
import sys, re, json, random
import numpy as np
import torch
import torch.nn.functional as F

import simclr as S

SEED = 0
random.seed(SEED); np.random.seed(SEED); torch.manual_seed(SEED)
DATA = "/mnt/c/Users/map12/.claude/orchestration/logs/oracle_diverse.jsonl"


def load_by_spec():
    by = {}
    for ln in open(DATA, encoding="utf-8"):
        d = json.loads(ln)
        if int(d.get("label", 0)) != 1:
            continue
        code = re.sub(r"\b" + re.escape(d["spec"]) + r"\b", "<fn>", d["code"])  # mata el nombre
        by.setdefault(d["spec"], []).append(code)
    return {k: v for k, v in by.items() if len(v) >= 2}   # specs con >=2 (pa pares)


def p_at_1(enc, vocab, by_spec):
    enc.eval()
    codes, specs = [], []
    for sp, cs in by_spec.items():
        for c in cs:
            codes.append(c); specs.append(sp)
    with torch.no_grad():
        X = enc(torch.tensor([S.encode(S.tokenize_code(c), vocab) for c in codes])).numpy()
    X = X / (np.linalg.norm(X, axis=1, keepdims=True) + 1e-9)
    Sim = X @ X.T; np.fill_diagonal(Sim, -1)
    specs = np.array(specs)
    return float(np.mean(specs[Sim.argmax(1)] == specs))


def train(by_train, vocab, *, mode, epochs=20, hid=192):
    enc = S.Encoder(len(vocab), hid=hid); proj = S.Projector(enc.out_dim)
    opt = torch.optim.Adam(list(enc.parameters()) + list(proj.parameters()), lr=2e-3)
    rng = random.Random(SEED)
    # tokens por solucion
    toks = {sp: [S.tokenize_code(c) for c in cs] for sp, cs in by_train.items()}
    specs = list(by_train)
    enc.train(); proj.train()
    for ep in range(epochs):
        rng.shuffle(specs)
        # batch = un par (anchor, positivo) por spec
        v1, v2 = [], []
        for sp in specs:
            ts = toks[sp]
            if mode == "functional":
                a, b = rng.sample(ts, 2)                     # 2 soluciones DISTINTAS del spec
            else:                                            # augment: misma sol, 2 augs
                base = rng.choice(ts)
                a, b = S.augment(base, rng), S.augment(base, rng)
            v1.append(S.encode(a if isinstance(a, list) else a, vocab))
            v2.append(S.encode(b if isinstance(b, list) else b, vocab))
        z1 = proj(enc(torch.tensor(v1))); z2 = proj(enc(torch.tensor(v2)))
        loss = S.nt_xent(z1, z2)
        opt.zero_grad(); loss.backward(); opt.step()
        if (ep + 1) % 5 == 0:
            print(f"  [{mode}] epoch {ep+1}/{epochs} loss={loss.item():.4f}", flush=True)
    return enc


def main():
    global SEED
    epochs = int(sys.argv[1]) if len(sys.argv) > 1 else 40
    SEED = int(sys.argv[2]) if len(sys.argv) > 2 else 0
    random.seed(SEED); np.random.seed(SEED); torch.manual_seed(SEED)
    by = load_by_spec()
    specs = sorted(by); random.Random(SEED).shuffle(specs)
    n_test = max(2, len(specs) // 5)
    test_specs = set(specs[:n_test]); train_specs = [s for s in specs if s not in test_specs]
    by_train = {s: by[s] for s in train_specs}
    by_test = {s: by[s] for s in test_specs}
    print(f"specs train={len(by_train)} test={len(by_test)} | passers={sum(len(v) for v in by.values())}", flush=True)

    vocab = S.build_vocab([S.tokenize_code(c) for cs in by_train.values() for c in cs])
    res = {}
    for mode in ("augment", "functional"):
        enc = train(by_train, vocab, mode=mode, epochs=epochs)
        res[mode] = p_at_1(enc, vocab, by_test)
        print(f"P@1 held-out [{mode}] = {res[mode]:.4f}", flush=True)
    print(f"RESULT augment={res['augment']:.4f} functional={res['functional']:.4f} "
          f"lift={res['functional']-res['augment']:+.4f}", flush=True)


if __name__ == "__main__":
    main()
