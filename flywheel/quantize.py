"""quantize — #5 (parte barata): float32 -> float16 del code_embedder. Halva el archivo
(deploy/clone). En numpy NO acelera (no hay BLAS fp16 nativo) -> upcast a fp32 al cargar;
el win es TAMAÑO. A 3.77MB es premature, pero deja el mecanismo + gate pa cuando crezca.

Gateado: produce un fp16 aparte, mide el delta de AUC; SOLO se adopta si el delta < tol.
"""
from __future__ import annotations
import sys, json, hashlib, pathlib
import numpy as np

ROOT = pathlib.Path(__file__).resolve().parents[1]
SRC = ROOT / "flywheel" / "code_embedder.npz"
FP16 = ROOT / "flywheel" / "code_embedder_fp16.npz"


def quantize() -> dict:
    d = np.load(SRC)
    arrs = {k: d[k].astype(np.float16) for k in d.files}
    np.savez(FP16, **arrs)
    s0 = SRC.stat().st_size; s1 = FP16.stat().st_size
    return {"src_mb": round(s0 / 1e6, 2), "fp16_mb": round(s1 / 1e6, 2),
            "ratio": round(s1 / s0, 3),
            "sha256": hashlib.sha256(open(FP16, "rb").read()).hexdigest()}


if __name__ == "__main__":
    print(json.dumps(quantize(), ensure_ascii=False))
