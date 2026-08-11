"""iohelpers — shared robustness idioms for the JSON/JSONL state files under logs/*.

Extracted after the 2026-08 robustness audit (.scratch/audit-2026-08): the atomic-write
idiom (tmp in the same dir + os.replace) and the "no-existe vs no-parsea" load idiom were
about to be copy-pasted into 5+ modules (budget_policy/evolve/nudge/feedback/vault). One
place to change = the DRY fix. `cache.py` has the same idiom inlined (written by another
agent in parallel) — left untouched on purpose, not migrated to this helper.
"""
from __future__ import annotations

import json
import logging
import os
import threading
from pathlib import Path
from typing import Any

_log = logging.getLogger(__name__)


def atomic_write_json(path: Path, obj: Any, *, indent: int | None = None) -> None:
    """tmp file in the SAME directory + os.replace: a crash mid-write can never leave
    `path` truncated/corrupt (os.replace is atomic on both POSIX and NTFS)."""
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(obj, ensure_ascii=False, indent=indent), encoding="utf-8")
    os.replace(tmp, path)


def load_json_tolerant(path: Path, default: Any, *, what: str = "") -> Any:
    """Distingue no-existe (default legitimo, silencioso) de no-parsea (log fuerte —
    JSON truncado/corrupto es una señal real, no un estado esperado)."""
    if not path.exists():
        return default
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        _log.error("%s corrupto en %s; usando default", what or "archivo", path)
        return default


def read_jsonl_tolerant(path: Path) -> list[dict]:
    """Reader .jsonl tolerante por linea (patron arbitration.py:44-47): una linea torn
    (multiples escritores appendeando el mismo .jsonl) se saltea en vez de tumbar todo
    el parseo — el resto del archivo sigue siendo señal valida."""
    if not path.exists():
        return []
    out: list[dict] = []
    for ln in path.read_text(encoding="utf-8").splitlines():
        ln = ln.strip()
        if not ln:
            continue
        try:
            out.append(json.loads(ln))
        except Exception:
            continue   # una linea corrupta no rompe el resto de la lectura
    return out


_JSONL_CACHE_LOCK = threading.Lock()
# path (str) -> ((mtime_ns, size), parsed rows). Ticket 13 (audit-2026-08): metrics.jsonl/
# feedback.jsonl son append-only SIN rotación, así que (mtime_ns, size) alcanza como llave
# de invalidación — un archivo que no cambió de tamaño/mtime no puede tener nuevas líneas.
_JSONL_CACHE: dict[str, tuple[tuple[int, int], list[dict]]] = {}


def _stat_key(path: Path) -> tuple[int, int] | None:
    try:
        st = path.stat()
        return st.st_mtime_ns, st.st_size
    except OSError:
        return None


def read_jsonl_cached(path: Path) -> list[dict]:
    """`read_jsonl_tolerant(path)` con cache módulo-level por (mtime_ns, size): si el
    archivo no cambió desde la última lectura, devuelve la MISMA lista ya parseada en vez
    de reparsear miles de líneas por call (route/budget lo llaman antes de cada API call,
    ×8 concurrente en fan_out). Thread-safe: el lock rodea el check-and-fill completo,
    así 2 threads que ven el mismo mtime no pisan la entrada del otro a medias."""
    key = str(path)
    sk = _stat_key(path)
    with _JSONL_CACHE_LOCK:
        cached = _JSONL_CACHE.get(key)
        if cached is not None and sk is not None and cached[0] == sk:
            return cached[1]
        parsed = read_jsonl_tolerant(path)
        if sk is not None:
            _JSONL_CACHE[key] = (sk, parsed)
        return parsed


def read_jsonl_tail(path: Path, max_lines: int, *, chunk_size: int = 65536) -> list[dict]:
    """Lee solo las últimas `max_lines` líneas de un .jsonl append-only, sin parsear la
    historia completa: seekea desde el final leyendo de a `chunk_size` bytes hasta juntar
    suficientes newlines (o llegar al principio del archivo). Para consumidores VENTANEADOS
    (ej error_rates(window_n=200) sobre un log de 13.5k líneas) — evita el O(historia)."""
    if max_lines <= 0 or not path.exists():
        return []
    with open(path, "rb") as fh:
        fh.seek(0, os.SEEK_END)
        pos = fh.tell()
        block = b""
        while pos > 0 and block.count(b"\n") <= max_lines:
            step = min(chunk_size, pos)
            pos -= step
            fh.seek(pos)
            block = fh.read(step) + block
    lines = [ln.strip() for ln in block.decode("utf-8", errors="ignore").splitlines() if ln.strip()]
    out: list[dict] = []
    for ln in lines[-max_lines:]:
        try:
            out.append(json.loads(ln))
        except Exception:
            continue   # una linea corrupta no rompe el resto de la lectura (mismo idioma que tolerant)
    return out


if __name__ == "__main__":
    import tempfile

    with tempfile.TemporaryDirectory() as td:
        p = Path(td) / "x.json"
        atomic_write_json(p, {"a": 1})
        assert load_json_tolerant(p, {}) == {"a": 1}
        p.write_text("{not json", encoding="utf-8")
        assert load_json_tolerant(p, {"fallback": True}) == {"fallback": True}
        assert load_json_tolerant(Path(td) / "missing.json", []) == []

        jl = Path(td) / "x.jsonl"
        jl.write_text('{"a":1}\nnot json\n{"b":2}\n\n', encoding="utf-8")
        assert read_jsonl_tolerant(jl) == [{"a": 1}, {"b": 2}]
        assert read_jsonl_tolerant(Path(td) / "missing.jsonl") == []
    print("iohelpers OK — atomic write / tolerant load / tolerant jsonl read")
