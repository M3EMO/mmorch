"""canal — hilo ordenado entre Cursor, Claude Code y mmorch.

No es memoria semantica (destilar un turno pierde el 'que falta probar').
No despierta al otro proceso: el usuario cambia de ventana. Append-only JSONL
bajo logs/, igual que metrics. Inyectar `path=` en tests.
"""
from __future__ import annotations

import json
import threading
import time
from pathlib import Path

from .iohelpers import read_jsonl_tolerant
from .paths import logs_dir

SIDES = ("cursor", "claude", "mmorch", "user")
KINDS = ("status", "ask", "refute", "handoff")

_LOCK = threading.Lock()


def canal_path() -> Path:
    return logs_dir() / "canal.jsonl"


def _next_id(rows: list[dict]) -> int:
    n = 0
    for r in rows:
        try:
            n = max(n, int(r.get("id", 0)))
        except (TypeError, ValueError):
            continue
    return n + 1


def post(
    src: str,
    kind: str,
    body: str,
    *,
    artifacts: list[str] | None = None,
    verify_cmd: str = "",
    verify_expect: str = "",
    to: str = "",
    path: Path | None = None,
) -> dict:
    """Append a turn. `to` empty = both agents. `handoff` should carry verify_cmd."""
    body = (body or "").strip()
    if not body:
        raise ValueError('body vacio')
    if src not in SIDES:
        raise ValueError(f"src must be one of {SIDES}")
    if kind not in KINDS:
        raise ValueError(f"kind must be one of {KINDS}")
    if to and to not in SIDES:
        raise ValueError(f"to must be empty or one of {SIDES}")
    p = path or canal_path()
    p.parent.mkdir(parents=True, exist_ok=True)
    with _LOCK:
        rows = read_jsonl_tolerant(p)
        rec = {
            "id": _next_id(rows),
            "ts": time.time(),
            "src": src,
            "kind": kind,
            "body": body,
            "artifacts": list(artifacts or []),
            "verify_cmd": verify_cmd,
            "verify_expect": verify_expect,
            "to": to,
        }
        with open(p, "a", encoding="utf-8") as fh:
            fh.write(json.dumps(rec, ensure_ascii=False) + "\n")
    return rec


def read(n: int = 20, *, path: Path | None = None) -> list[dict]:
    """Last n turns, oldest first."""
    if n < 1:
        raise ValueError("n >= 1")
    rows = read_jsonl_tolerant(path or canal_path())
    return rows[-n:]


if __name__ == "__main__":
    import tempfile
    d = Path(tempfile.mkdtemp()) / "canal.jsonl"
    a = post("cursor", "status", "ping", path=d)
    b = post("claude", "handoff", "pong", verify_cmd="echo ok",
             verify_expect="ok", path=d)
    got = read(10, path=d)
    assert [t["id"] for t in got] == [a["id"], b["id"]], got
    print("ok", got[-1]["id"])
