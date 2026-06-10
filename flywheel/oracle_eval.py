"""oracle_eval — prueba la tesis del flywheel sobre la label de EJECUCION.
GroupKFold por SPEC: el test son specs que el probe NUNCA vio -> mide si generaliza
'que hace correcto a un codigo', no memoriza specs. Si bge/struct ya dan AUC>>0.5 aca
(vs azar en JIT-defect), queda probado que el cuello era la LABEL, no la representacion.
"""
from __future__ import annotations
import json, sys, pathlib
import numpy as np

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
DATA = ROOT / "logs" / "oracle_dataset.jsonl"


def load():
    codes, y, groups = [], [], []
    for line in open(DATA, encoding="utf-8"):
        d = json.loads(line)
        codes.append(d["code"]); y.append(int(d["label"])); groups.append(d["spec"])
    return codes, np.array(y), np.array(groups)


def group_auc(X, y, groups):
    from sklearn.linear_model import LogisticRegression
    from sklearn.model_selection import GroupKFold
    from sklearn.metrics import roc_auc_score
    gkf = GroupKFold(n_splits=min(5, len(set(groups))))
    aucs = []
    for tr, te in gkf.split(X, y, groups):
        if len(set(y[tr])) < 2 or len(set(y[te])) < 2:
            continue
        clf = LogisticRegression(max_iter=1000)
        clf.fit(X[tr], y[tr])
        p = clf.predict_proba(X[te])[:, 1]
        aucs.append(roc_auc_score(y[te], p))
    return (float(np.mean(aucs)), float(np.std(aucs)), len(aucs)) if aucs else (float("nan"), 0.0, 0)


def main():
    from flywheel.baseline import embed_bge, embed_struct
    codes, y, groups = load()
    print(f"n={len(codes)} pass={int(y.sum())} fail={int((1-y).sum())} specs={len(set(groups))}")

    Xs = embed_struct(codes)
    m, s, k = group_auc(Xs, y, groups)
    print(f"STRUCT  group-AUC {m:.4f} +/- {s:.4f}  (folds={k})")

    Xb = embed_bge(codes)
    m, s, k = group_auc(Xb, y, groups)
    print(f"BGE     group-AUC {m:.4f} +/- {s:.4f}  (folds={k})")
    print(">0.50 = la correctitud SI esta en el texto -> el cuello era la label JIT, no el encoder.")


if __name__ == "__main__":
    main()
