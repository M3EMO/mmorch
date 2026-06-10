"""export_numpy — saca el encoder SimCLR de torch a un .npz (pesos crudos) + vocab json,
para correr inferencia en numpy puro SIN torch (deployable en el core .venv de Windows,
cero dep pesada = respeta anti-complejidad del GOAL). Corre en WSL (donde esta torch).

Exporta: embedding, GRU bidireccional l0 (fwd+reverse) weight_ih/hh + bias_ih/hh.
"""
from __future__ import annotations
import json, sys
import numpy as np
import torch

PT = "/mnt/c/Users/map12/.claude/orchestration/flywheel/code_embedder.pt"
NPZ = "/mnt/c/Users/map12/.claude/orchestration/flywheel/code_embedder.npz"
VOCAB = "/mnt/c/Users/map12/.claude/orchestration/flywheel/code_embedder_vocab.json"


def main():
    ck = torch.load(PT, map_location="cpu")
    enc = ck["enc"]
    vocab = ck["vocab"]
    arrs = {}
    arrs["emb"] = enc["emb.weight"].numpy()
    for name in ["weight_ih_l0", "weight_hh_l0", "bias_ih_l0", "bias_hh_l0",
                 "weight_ih_l0_reverse", "weight_hh_l0_reverse",
                 "bias_ih_l0_reverse", "bias_hh_l0_reverse"]:
        arrs[name] = enc["gru." + name].numpy()
    np.savez(NPZ, **arrs)
    with open(VOCAB, "w", encoding="utf-8") as f:
        json.dump(vocab, f)
    hid = arrs["weight_hh_l0"].shape[1]
    print(f"export OK: emb{arrs['emb'].shape} hid={hid} vocab={len(vocab)} -> {NPZ}")


if __name__ == "__main__":
    main()
