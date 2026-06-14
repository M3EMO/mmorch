"""eval_functional — el eje que #1 (positivos funcionales) ataca: ¿el embedding agrupa
codigo que HACE LO MISMO? Usa oracle_dataset: soluciones que PASAN el mismo spec = funcional-
mente equivalentes. Metrica = retrieval P@1 (¿el vecino mas cercano es del mismo spec?) +
separacion same-spec vs diff-spec (AUC). Complementa el eje ESTRUCTURAL (radon, baseline 0.88).

Compara code_embedder (numpy) vs bge-small. Corre en el core (.venv), cero torch.
"""
from __future__ import annotations
import sys, json, pathlib
import numpy as np

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
DATA = ROOT / "logs" / "oracle_dataset.jsonl"


def _load_passers():
    codes, specs = [], []
    for ln in open(DATA, encoding="utf-8"):
        d = json.loads(ln)
        if int(d.get("label", 0)) == 1:        # solo soluciones que PASAN (funcionalmente ok)
            codes.append(d["code"]); specs.append(d["spec"])
    return codes, np.array(specs)


def _norm(X):
    return X / (np.linalg.norm(X, axis=1, keepdims=True) + 1e-9)


def metrics(X, specs):
    X = _norm(X)
    S = X @ X.T
    np.fill_diagonal(S, -1.0)                  # excluir self
    nn = S.argmax(axis=1)
    p_at_1 = float(np.mean(specs[nn] == specs))   # vecino mas cercano = mismo spec
    # AUC same-spec vs diff-spec sobre todos los pares
    same, diff = [], []
    n = len(specs)
    for i in range(n):
        for j in range(i + 1, n):
            (same if specs[i] == specs[j] else diff).append(S[i, j])
    same, diff = np.array(same), np.array(diff)
    # AUC = P(sim(same) > sim(diff))
    from sklearn.metrics import roc_auc_score
    y = np.r_[np.ones(len(same)), np.zeros(len(diff))]
    sc = np.r_[same, diff]
    auc = float(roc_auc_score(y, sc))
    return {"p_at_1": round(p_at_1, 4), "same_vs_diff_auc": round(auc, 4),
            "n": n, "specs": len(set(specs.tolist()))}


def embed_code_np(codes):
    from mmorch.code_embedder import embed_code, available
    assert available(), "falta code_embedder.npz"
    return np.array([embed_code(c) for c in codes])


def embed_bge(codes):
    from fastembed import TextEmbedding
    m = TextEmbedding("BAAI/bge-small-en-v1.5")
    return np.array(list(m.embed([c[:1200] for c in codes], batch_size=32)))


if __name__ == "__main__":
    codes, specs = _load_passers()
    print(f"passers={len(codes)} specs={len(set(specs.tolist()))}")
    which = sys.argv[1] if len(sys.argv) > 1 else "both"
    if which in ("code", "both"):
        print("code_embedder:", metrics(embed_code_np(codes), specs))
    if which in ("bge", "both"):
        print("bge-small:    ", metrics(embed_bge(codes), specs))
