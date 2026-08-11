"""memo (I-4) — cache content-hash de resultados/verdicts. Salta re-gen/re-verify
identico -> ahorra cupo + API $. File-backed (logs/memo.json). Hash sha256 de las
entradas; valores serializables.
"""
from __future__ import annotations

import hashlib
import json
import logging
import os
import threading
from pathlib import Path

from .config import DEFAULT_VERIFIER

_PATH = Path(__file__).resolve().parent.parent / "logs" / "memo.json"
_LOCK = threading.Lock()
_log = logging.getLogger(__name__)


def key_of(*parts: str) -> str:
    h = hashlib.sha256("\x00".join(str(p) for p in parts).encode("utf-8"))
    return h.hexdigest()[:32]


class Memo:
    def __init__(self, path: Path = _PATH):
        self.path = path
        self._d: dict = {}
        if path.exists():
            try:
                self._d = json.loads(path.read_text(encoding="utf-8"))
            except Exception:
                # NO resetear en silencio: se re-pagaria API por verifies ya cacheados.
                _log.warning("memo cache corrupto en %s; arrancando vacio", path)
                self._d = {}

    def get(self, key: str):
        return self._d.get(key)

    def put(self, key: str, value) -> None:
        with _LOCK:
            self._d[key] = value
            self.path.parent.mkdir(parents=True, exist_ok=True)
            # write atomico (tmp + replace): un crash a mitad de write no corrompe el cache.
            tmp = self.path.with_suffix(".json.tmp")
            tmp.write_text(json.dumps(self._d, ensure_ascii=False), encoding="utf-8")
            os.replace(tmp, self.path)

    def __len__(self):
        return len(self._d)


_DEFAULT_MEMO: "Memo | None" = None


def default_memo() -> "Memo":
    """Singleton modulo-level: evita releer logs/memo.json entero en cada llamada."""
    global _DEFAULT_MEMO
    with _LOCK:
        if _DEFAULT_MEMO is None:
            _DEFAULT_MEMO = Memo()
        return _DEFAULT_MEMO


def memoized_verify(artifact: str, rubric: str, *, verifier_model: str = DEFAULT_VERIFIER,
                    gen_model: str = "deepseek-chat", memo: "Memo | None" = None, **kw):
    """Verify con cache. Devuelve (verdict_dict, cached: bool). Skip API si hit."""
    from .patterns import adversarial_verify
    # NO usar `memo or default_memo()`: Memo define __len__, un memo vacio es falsy ->
    # ignoraria el memo pasado y crearia uno default (bug de contaminacion).
    m = memo if memo is not None else default_memo()
    k = key_of("verify", artifact, rubric, verifier_model, gen_model)
    hit = m.get(k)
    if hit is not None:
        return hit, True
    v = adversarial_verify(artifact, rubric=rubric, gen_model=gen_model,
                           verifier_model=verifier_model, **kw)
    out = {"passed": v.passed, "confidence": v.confidence,
           "refutations": v.refutations, "verifier_model": v.verifier_model}
    m.put(k, out)
    return out, False
