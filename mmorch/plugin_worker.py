"""plugin_worker — isolated subprocess host for ONE plugin invoke (graft G11).

Runs outside the mmorch package's own logic (the plugin entry is imported by file path
only) so untrusted plugin code can't reach mmorch internals. Speaks newline-delimited
JSON with the host over stdin/stdout: receives one `invoke`, may emit `host_call`s
(which the host capability-gates), returns one `result`/`error`. This worker only
relays; the host decides what's allowed.

Protocol (one JSON object per line):
  <- {"type":"invoke","method":..,"args":..,"dir":..,"entry":..}
  -> {"type":"host_call","id":N,"method":..,"params":..}
  <- {"type":"host_result","id":N,"value":..|"error":..}
  -> {"type":"result","value":..}  |  {"type":"error","error":..}

stdout is redirected to stderr BEFORE the entry module is imported, so a plugin that
prints at import time (or at call time) can't corrupt the NDJSON protocol. Import
failures (SyntaxError, ImportError, missing entry/contribution) are caught and sent
as a normal `error` message instead of crashing the worker.
"""
from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


def _send(obj: dict) -> None:
    sys.__stdout__.write(json.dumps(obj, ensure_ascii=False) + "\n")  # type: ignore[union-attr]
    sys.__stdout__.flush()  # type: ignore[union-attr]


def _load_entry(plugin_dir: Path, entry: str):
    path = plugin_dir / entry
    spec = importlib.util.spec_from_file_location("mmorch_plugin_entry", path)
    if spec is None or spec.loader is None:
        raise ImportError(f"cannot load entry '{entry}'")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def main() -> None:
    sys.stdout = sys.stderr  # plugin print() (import-time or call-time) must not corrupt the protocol

    line = sys.stdin.readline()
    if not line:
        return
    req = json.loads(line)
    method = req.get("method", "")
    args = req.get("args", {})

    try:
        mod = _load_entry(Path(req["dir"]), req.get("entry", "main.py"))
    except Exception as e:
        _send({"type": "error", "error": f"load failed: {e}"})
        return

    seq = {"n": 0}

    def host(host_method, params=None):
        seq["n"] += 1
        cid = seq["n"]
        _send({"type": "host_call", "id": cid, "method": host_method, "params": params or {}})
        reply = sys.stdin.readline()
        if not reply:
            raise RuntimeError("host closed")
        msg = json.loads(reply)
        if msg.get("error"):
            raise RuntimeError(f"host denied {host_method}: {msg['error']}")
        return msg.get("value")

    try:
        fn = getattr(mod, method, None)
        if not callable(fn):
            raise AttributeError(f"plugin has no contribution '{method}'")
        _send({"type": "result", "value": fn(args, host)})
    except Exception as e:
        _send({"type": "error", "error": str(e)[:300]})


if __name__ == "__main__":
    main()
