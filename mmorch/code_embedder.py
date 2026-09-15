"""code_embedder — inferencia NUMPY PURA del encoder SimCLR del flywheel (sin torch).
Mejor representacion de codigo que bge-small en estructura/calidad (flywheel/RESULTS.md:
radon-tier AUC 0.88 vs bge 0.80). Pesos exportados de torch via flywheel/export_numpy.py.

Es un NODO de la fabrica promovido: token-embed -> bi-GRU -> mean-pool (mask-aware).
Carga perezosa (persistida por mmorch.weights). Si falla la verificacion de pesos,
embed() devuelve None (degrada graceful, como memory.embed). Cero dep nueva: solo numpy
y el modulo de pesos del repo.

Uso: from mmorch.code_embedder import embed_code; v = embed_code("def f(x): return x+1")
"""
from __future__ import annotations
import io, json, re, tokenize, keyword, builtins, textwrap
from pathlib import Path
from typing import Any, Literal
import numpy as np

from . import weights

_BUILTINS = set(dir(builtins)) | set(keyword.kwlist)
_WORD = re.compile(r"[A-Za-z_]\w*|\d+|[^\sA-Za-z0-9_]")
_MAXLEN = 200

_CACHE: tuple[Any, Any] | Literal[False] | None = None


def _load() -> tuple[Any, Any] | None:
    """Carga y verifica pesos una sola vez. None si no estan disponibles."""
    global _CACHE
    if _CACHE is False:
        return None
    if isinstance(_CACHE, tuple):
        return _CACHE

    ok, _motivo = weights.verify("code_embedder")
    if not ok:
        _CACHE = False
        return None

    npz_path = weights.resolve("code_embedder")
    card = weights.card("code_embedder")
    if card is None or "vocab_path" not in card:
        _CACHE = False
        return None
    vocab_rel = card["vocab_path"]
    vocab_path = Path(weights.ROOT) / vocab_rel

    d = np.load(npz_path)
    vocab = json.loads(vocab_path.read_text(encoding="utf-8"))
    modelo = {"w": {k: d[k].astype(np.float32) for k in d.files}}
    _CACHE = (modelo, vocab)
    return _CACHE


# --- tokenizer: identico a flywheel/simclr.py (misma distribucion de entrenamiento) --- #
def _tokenize(src: str) -> list[str]:
    s = textwrap.dedent(src)
    out: list[str] = []
    try:
        for tok in tokenize.generate_tokens(io.StringIO(s).readline):
            t = tok.type
            if t in (tokenize.COMMENT, tokenize.NL, tokenize.NEWLINE, tokenize.INDENT,
                     tokenize.DEDENT, tokenize.ENCODING, tokenize.ENDMARKER):
                continue
            if t == tokenize.STRING:
                out.append("<str>"); continue
            if t == tokenize.NUMBER:
                out.append("<num>"); continue
            if tok.string.strip():
                out.append(tok.string)
    except Exception:
        out = [m.group() for m in _WORD.finditer(s)]
    return out


def _sigmoid(x):
    return 1.0 / (1.0 + np.exp(-x))


def _gru_dir(X, w, suf):
    """Una direccion del GRU. X: (T, in). Devuelve estados (T, H). Gate order PyTorch [r,z,n]."""
    Wih, Whh = w["weight_ih_l0" + suf], w["weight_hh_l0" + suf]
    bih, bhh = w["bias_ih_l0" + suf], w["bias_hh_l0" + suf]
    H = Whh.shape[1]
    h = np.zeros(H)
    outs = []
    GI = X @ Wih.T + bih                       # (T, 3H) parte input precomputada
    for t in range(X.shape[0]):
        gi = GI[t]
        gh = h @ Whh.T + bhh
        i_r, i_z, i_n = gi[:H], gi[H:2 * H], gi[2 * H:]
        h_r, h_z, h_n = gh[:H], gh[H:2 * H], gh[2 * H:]
        r = _sigmoid(i_r + h_r)
        z = _sigmoid(i_z + h_z)
        n = np.tanh(i_n + r * h_n)
        h = (1.0 - z) * n + z * h
        outs.append(h)
    return np.array(outs)


def embed_code(code: str) -> list[float] | None:
    """Embedding del codigo (dim 2*hidden). None si faltan artefactos del encoder."""
    loaded = _load()
    if loaded is None:
        return None
    modelo, vocab = loaded
    w = modelo["w"]
    toks = _tokenize(code)[:_MAXLEN]
    ids = [vocab.get(t, 1) for t in toks] or [1]
    X = w["emb"][ids]                          # (T, emb)
    fwd = _gru_dir(X, w, "")
    bwd = _gru_dir(X[::-1], w, "_reverse")[::-1]
    h = np.concatenate([fwd, bwd], axis=1)     # (T, 2H)
    return h.mean(axis=0).tolist()             # mean-pool (sin pad => mask trivial)


def available() -> bool:
    return _load() is not None
