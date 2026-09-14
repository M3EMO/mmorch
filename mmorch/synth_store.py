"""synth_store — registro persistente de checkers SINTETIZADOS y promovidos, por tipo.

Idea del usuario (2026-09-10): el codigo que un modelo escribe para un tipo de problema
es reutilizable. Un gate deberia pagar una llamada la PRIMERA vez que ve un tipo y
correr gratis desde entonces. Esto es checkers.py escrito por el modelo, con memoria.

Modelo de datos (synth_checkers.json, versionado, en la raiz del repo):
    kind -> {src, model, promoted_at, evidence: {n_promote, edge, n_test, n_test_ok},
             sha256}
Una entrada por tipo. Se reemplaza solo si la nueva tiene MAS evidencia (mas items de
test pasados). El `sha256` es de la fuente: dos runs que sintetizan el mismo codigo no
duplican nada.

Uso desde un gate:
    from mmorch.synth_store import get, run, put
    src = get(kind)                      # None si el tipo es nuevo -> sintetizar + promover
    got = run(kind, params)              # ejecuta en sandbox; None si falla o no existe

Regla: lo que entra al store PASO una promocion contra verdad computada (3 items + 1 de
borde como minimo). El store no promueve; guarda lo ya promovido y su evidencia. Un
checker sin evidencia no entra -- misma regla que el contrato de gate del SDLC.
ponytail: un JSON plano con lock de archivo. Sqlite cuando pasen de ~1000 tipos.
"""
from __future__ import annotations

import hashlib
import json
import threading
import time

from .paths import repo_root
from .sandbox import run_sandboxed

STORE_PATH = repo_root() / "synth_checkers.json"
_lock = threading.Lock()


def _load() -> dict:
    if not STORE_PATH.exists():
        return {}
    try:
        return json.loads(STORE_PATH.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return {}


def _save(d: dict) -> None:
    tmp = STORE_PATH.with_suffix(".json.tmp")
    tmp.write_text(json.dumps(d, ensure_ascii=False, indent=1, sort_keys=True), encoding="utf-8")
    tmp.replace(STORE_PATH)


def get(kind: str) -> str | None:
    """Fuente promovida para el tipo, o None si el tipo es nuevo."""
    return (_load().get(kind) or {}).get("src")


def entry(kind: str) -> dict | None:
    return _load().get(kind)


def put(kind: str, src: str, model: str, evidence: dict) -> bool:
    """Guarda si el tipo es nuevo o si la evidencia nueva supera a la guardada.
    evidence = {n_promote, edge(bool), n_test, n_test_ok}. Devuelve True si escribio."""
    score = int(evidence.get("n_test_ok", 0)) + int(evidence.get("n_promote", 0))
    with _lock:
        d = _load()
        cur = d.get(kind)
        if cur:
            cur_score = int(cur["evidence"].get("n_test_ok", 0)) + int(cur["evidence"].get("n_promote", 0))
            if cur_score >= score:
                return False
        d[kind] = {
            "src": src, "model": model, "promoted_at": time.strftime("%Y-%m-%d %H:%M"),
            "evidence": evidence, "sha256": hashlib.sha256(src.encode("utf-8")).hexdigest()[:16],
        }
        _save(d)
        return True


def run(kind: str, params: dict, timeout: float = 20.0) -> int | None:
    """Ejecuta el checker del tipo sobre params en el sandbox. None si no hay checker,
    falla, se pasa del timeout o no imprime un int."""
    src = get(kind)
    if not src:
        return None
    code = (src + "\n\nimport json\n"
            f"print(solve(json.loads({json.dumps(json.dumps(params))})))\n")
    r = run_sandboxed(code, timeout=timeout)
    if r.timed_out or r.returncode != 0:
        return None
    try:
        return int(r.stdout.strip().splitlines()[-1])
    except (ValueError, IndexError):
        return None


def stats() -> dict:
    d = _load()
    by_model: dict = {}
    for v in d.values():
        by_model[v["model"]] = by_model.get(v["model"], 0) + 1
    return {"kinds": len(d), "by_model": by_model, "path": str(STORE_PATH)}
