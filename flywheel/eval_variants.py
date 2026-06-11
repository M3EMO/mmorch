"""eval_variants — empuja el AUC hacia 1: prueba probes mas fuertes (GBRT) y ENSEMBLE
(concat de representaciones que capturan cosas distintas: SimCLR estructura aprendida +
struct radon-feats + bge texto). Usa el encoder NUMPY (code_embedder) ya exportado.
Caveat: label radon es semi-circular; el ensemble es la mejora legitima, no memorizar radon.
"""
from __future__ import annotations
import sys, json, pathlib
import numpy as np

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))


def cv_auc(X, y, folds=5, gbrt=False):
    from sklearn.model_selection import StratifiedKFold
    from sklearn.metrics import roc_auc_score
    skf = StratifiedKFold(n_splits=folds, shuffle=True, random_state=0)
    aucs = []
    for tr, te in skf.split(X, y):
        if gbrt:
            from sklearn.ensemble import HistGradientBoostingClassifier
            clf = HistGradientBoostingClassifier(max_iter=300, learning_rate=0.08,
                                                 max_depth=4, random_state=0)
        else:
            from sklearn.linear_model import LogisticRegression
            clf = LogisticRegression(max_iter=1000)
        clf.fit(X[tr], y[tr])
        p = clf.predict_proba(X[te])[:, 1]
        aucs.append(roc_auc_score(y[te], p))
    return float(np.mean(aucs)), float(np.std(aucs))


def main():
    from mmorch.code_embedder import embed_code, available
    from mmorch.factory import featurize_code
    N = int(sys.argv[1]) if len(sys.argv) > 1 else 4000
    assert available(), "falta code_embedder.npz (re-exportar tras retrain)"
    rows = [json.loads(l) for i, l in enumerate(open(ROOT / "logs/radon_dataset.jsonl",
            encoding="utf-8")) if i < N]
    y = np.array([r["label"] for r in rows])
    Xs = np.array([embed_code(r["code"]) for r in rows])      # SimCLR
    Xt = np.array([featurize_code(r["code"]) for r in rows])  # struct radon-feats
    # normalizar antes de concat (escalas distintas)
    def z(A):
        return (A - A.mean(0)) / (A.std(0) + 1e-9)
    Xs, Xt = z(Xs), z(Xt)
    print(f"n={len(rows)} dims simclr={Xs.shape[1]} struct={Xt.shape[1]}")

    for name, X, gb in [
        ("SimCLR  + logreg", Xs, False),
        ("SimCLR  + GBRT  ", Xs, True),
        ("struct  + GBRT  ", Xt, True),
        ("CONCAT  + logreg", np.hstack([Xs, Xt]), False),
        ("CONCAT  + GBRT  ", np.hstack([Xs, Xt]), True),
    ]:
        m, s = cv_auc(X, y, gbrt=gb)
        print(f"{name}: AUC {m:.4f} +/- {s:.4f}")


if __name__ == "__main__":
    main()
