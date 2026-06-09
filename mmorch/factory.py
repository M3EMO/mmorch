"""factory — mmorch como FÁBRICA de modelos (no ES el modelo, lo CONSTRUYE/entrena).

Distinción clave: mmorch no es un JEPA. Pero puede (a) ESCRIBIR el código de training
(es coder: swarm+checkers+sandbox_branch), (b) ENTRENAR modelos CHICOS end-to-end en CPU
(acá), (c) EMITIR un job para entrenar modelos GRANDES en GPU externa (orquesta, no corre).

MVP demostrable: entrena un clasificador de CALIDAD DE CÓDIGO (bueno/malo) con regresión
logística en numpy (cero dep pesada), desde features baratos reusando checkers. Esto es
la rebanada realizable del sueño "aprende a diferenciar buen código de mal código".
"""
from __future__ import annotations

import numpy as np


def featurize_code(text: str) -> list[float]:
    """Vector de features baratos de un snippet (reusa checkers). Determinista, sin API."""
    from .checkers import check as _check
    from .evolve import red_content_hits
    t = text or ""
    lines = t.splitlines()
    n_lines = len(lines)
    ast_ok = 1.0 if _check("python_ast_valid", code=t).passed else 0.0
    has_def = 1.0 if ("def " in t or "class " in t) else 0.0
    has_test = 1.0 if ("assert" in t or "def test" in t) else 0.0
    has_doc = 1.0 if ('"""' in t or "'''" in t) else 0.0
    comment = sum(1 for ln in lines if ln.strip().startswith("#"))
    red = float(len(red_content_hits(t)))
    long_lines = sum(1 for ln in lines if len(ln) > 100)
    return [
        np.log1p(n_lines), np.log1p(len(t)), ast_ok, has_def, has_test, has_doc,
        comment / max(n_lines, 1), red, long_lines / max(n_lines, 1),
    ]


def train_logreg(X, y, *, epochs: int = 300, lr: float = 0.2, l2: float = 1e-3) -> dict:
    """Regresión logística en numpy (gradient descent, estandarizado). CPU, segundos.
    Devuelve el modelo serializable {w, b, mu, sd}."""
    X = np.asarray(X, dtype=float)
    y = np.asarray(y, dtype=float)
    n, d = X.shape
    mu, sd = X.mean(0), X.std(0) + 1e-9
    Xs = (X - mu) / sd
    w = np.zeros(d)
    b = 0.0
    for _ in range(epochs):
        p = 1.0 / (1.0 + np.exp(-(Xs @ w + b)))
        g = p - y
        w -= lr * (Xs.T @ g / n + l2 * w)
        b -= lr * g.mean()
    return {"w": w.tolist(), "b": float(b), "mu": mu.tolist(), "sd": sd.tolist()}


def predict_proba(model: dict, X) -> np.ndarray:
    X = np.asarray(X, dtype=float)
    Xs = (X - np.asarray(model["mu"])) / np.asarray(model["sd"])
    return 1.0 / (1.0 + np.exp(-(Xs @ np.asarray(model["w"]) + model["b"])))


def accuracy(model: dict, X, y) -> float:
    pred = (predict_proba(model, X) >= 0.5).astype(float)
    return float((pred == np.asarray(y, dtype=float)).mean())


def train_code_quality(samples: list[tuple[str, int]], **kw) -> dict:
    """samples = [(code, label 0|1)] (1=buen código). Featuriza + entrena logreg. Devuelve
    {model, train_acc, n, predict}. Esto es mmorch ENTRENANDO un modelo chico, end-to-end."""
    X = [featurize_code(code) for code, _ in samples]
    y = [int(lbl) for _, lbl in samples]
    model = train_logreg(X, y, **kw)
    acc = accuracy(model, X, y)

    def predict(code: str) -> float:
        return float(predict_proba(model, [featurize_code(code)])[0])

    return {"model": model, "train_acc": acc, "n": len(samples), "predict": predict}


def emit_training_job(*, model_kind: str, dataset_ref: str, hardware: str = "gpu",
                      hyperparams: dict | None = None) -> dict:
    """Para modelos GRANDES (JEPA, etc.): mmorch NO los corre local — emite un JOB SPEC
    para compute externo (GPU cloud / fine-tune API). mmorch escribe el código + orquesta;
    la compute es el recurso externo. Devuelve el spec (no ejecuta)."""
    return {
        "status": "emitted", "model_kind": model_kind, "dataset": dataset_ref,
        "hardware": hardware, "hyperparams": hyperparams or {},
        "note": ("mmorch genera el train script (coder pipeline) + este spec; la GPU corre "
                 "afuera. mmorch orquesta y gatea el modelo resultante (fitness), no lo entrena local."),
    }
