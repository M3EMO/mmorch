"""baseline — el numero a BATIR. bge-small embed (frozen, pre-entrenado en lenguaje
natural) + logreg sobre la label del dataset. Si esto ya predice bien, no hay cuello.
Hallazgo previo: code-quality desde texto/bge-small = AZAR. Esto lo cuantifica limpio
con AUC 5-fold (no accuracy: labels casi-balanceadas pero AUC es honesto vs umbral).

Tambien una baseline ESTRUCTURAL (features AST de factory.featurize_code) para separar
'representacion de lenguaje no sirve' de 'la senal no esta en la sintaxis'.
"""
from __future__ import annotations
import json, sys, pathlib
import numpy as np

import os
ROOT = pathlib.Path(__file__).resolve().parents[1]
DATA = pathlib.Path(os.environ.get("MMORCH_DS_WIN",
                                   str(ROOT / "logs" / "codequality_dataset.jsonl")))


def load(limit: int | None = None):
    codes, labels = [], []
    with open(DATA, encoding="utf-8") as f:
        for i, line in enumerate(f):
            if limit and i >= limit:
                break
            d = json.loads(line)
            codes.append(d["code"]); labels.append(int(d["label"]))
    return codes, np.array(labels)


def cv_auc(X, y, folds=5, seed=0):
    from sklearn.linear_model import LogisticRegression
    from sklearn.model_selection import StratifiedKFold
    from sklearn.metrics import roc_auc_score
    skf = StratifiedKFold(n_splits=folds, shuffle=True, random_state=seed)
    aucs = []
    for tr, te in skf.split(X, y):
        clf = LogisticRegression(max_iter=1000, C=1.0)
        clf.fit(X[tr], y[tr])
        p = clf.predict_proba(X[te])[:, 1]
        aucs.append(roc_auc_score(y[te], p))
    return float(np.mean(aucs)), float(np.std(aucs))


def embed_bge(codes):
    from fastembed import TextEmbedding
    m = TextEmbedding("BAAI/bge-small-en-v1.5")
    # bge max 512 tok; trunc a ~1200 char evita OOM en attention con funcs largas
    trunc = [c[:1200] for c in codes]
    return np.array(list(m.embed(trunc, batch_size=32)))


def embed_struct(codes):
    sys.path.insert(0, str(ROOT))
    from mmorch.factory import featurize_code
    return np.array([featurize_code(c) for c in codes])


if __name__ == "__main__":
    limit = int(sys.argv[1]) if len(sys.argv) > 1 else None
    codes, y = load(limit)
    print(f"n={len(codes)} pos={int(y.sum())} neg={int((1-y).sum())}")

    Xs = embed_struct(codes)
    m, s = cv_auc(Xs, y)
    print(f"STRUCT (AST feats, dim={Xs.shape[1]}): AUC {m:.4f} +/- {s:.4f}")

    Xb = embed_bge(codes)
    m, s = cv_auc(Xb, y)
    print(f"BGE-small (frozen, dim={Xb.shape[1]}): AUC {m:.4f} +/- {s:.4f}")
    print("baseline a batir = max de los dos. ~0.50 = azar.")
