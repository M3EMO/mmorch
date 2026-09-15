"""plugins — capability-gated plugin platform (graft G11 from paperclip plugin-loader.ts).

A plugin = a directory with `plugin.json` (manifest) + an entry module. Plugins run in an
ISOLATED subprocess (`plugin_worker.py`); the host drives one invoke and intercepts the
plugin's `host_call` requests, granting only capabilities the manifest DECLARED *and* the
host POLICY allows (two-layer, default-deny). Untrusted code never imports into the host.

Capability of a host method = its namespace (before the first '.'):
  "llm.call" -> cap "llm",  "log.emit" -> cap "log",  "fs.write" -> cap "fs".
grants = manifest.capabilities ∩ policy_allow.  A host_call runs iff cap ∈ grants AND the
method is a registered host service. Default-deny: empty MMORCH_PLUGINS_ALLOW => no host caps.

ponytail: fresh worker per invoke (pool it if invoke rate matters); NDJSON framing
(length-prefix if plugins emit huge blobs); per-invoke wall-clock kill guards a hung worker.
"""
from __future__ import annotations

import json
import os
import queue
import subprocess
import sys
import threading
import time
from pathlib import Path
from typing import cast

from .paths import home, repo_root

_REQUIRED = ("name", "version", "entry", "contributes")


def plugins_dir() -> Path:
    return Path(os.getenv("MMORCH_PLUGINS_DIR") or (home() / "plugins"))


def policy_allow() -> set[str]:
    raw = os.getenv("MMORCH_PLUGINS_ALLOW", "")
    return {c.strip() for c in raw.split(",") if c.strip()}


def _cap(method: str) -> str:
    return method.split(".", 1)[0]


def load_manifest(d, *, allow: set[str] | None = None) -> dict:
    d = Path(d)
    m = json.loads((d / "plugin.json").read_text(encoding="utf-8"))
    missing = [k for k in _REQUIRED if k not in m]
    if missing:
        raise ValueError(f"manifest {d.name} missing {missing}")
    declared = set(m.get("capabilities", []))
    pol = policy_allow() if allow is None else set(allow)
    m["dir"] = str(d.resolve())
    m["entry"] = m["entry"]
    m["grants"] = sorted(declared & pol)
    return m


def discover(*, allow: set[str] | None = None) -> list[dict]:
    base = plugins_dir()
    out: list = []
    if not base.is_dir():
        return out
    for d in sorted(base.iterdir()):
        if (d / "plugin.json").is_file():
            try:
                out.append(load_manifest(d, allow=allow))
            except Exception as e:
                out.append({"dir": str(d), "name": d.name, "error": str(e)[:200]})
    return out


class _Eof:
    pass


EOF = _Eof()


def _decode_line(line: str) -> dict | None:
    try:
        obj = json.loads(line)
    except Exception:
        return None
    if isinstance(obj, dict) and isinstance(obj.get("type"), str):
        return obj
    return None


def _send(proc: subprocess.Popen[str], msg: dict) -> bool:
    if proc.stdin is None:
        return False
    try:
        proc.stdin.write(json.dumps(msg, ensure_ascii=False) + "\n")
        proc.stdin.flush()
        return True
    except (BrokenPipeError, OSError, ValueError):
        return False


def _pump(proc: subprocess.Popen[str], q: queue.Queue[str | None]) -> None:
    try:
        assert proc.stdout is not None
        for line in proc.stdout:
            q.put(line)
    except Exception:
        pass
    finally:
        q.put(None)


def _next_message(q: queue.Queue[str | None], deadline: float) -> dict | _Eof | None:
    while True:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            return None
        try:
            line = q.get(timeout=remaining)
        except queue.Empty:
            return None
        if line is None:
            return EOF
        msg = _decode_line(line)
        if msg is not None:
            return msg
        # ignore undecodable line, continue


def _map_error_payload(payload: dict) -> dict:
    err = payload.get("error", "unknown plugin error")
    return {"ok": False, "error": err if isinstance(err, str) else str(err)}


def invoke(manifest: dict, method: str, args: dict, *, host_services: dict,
           timeout: float | None = None) -> dict:
    """Run one contribution in an isolated worker. Returns {ok, value} or {ok:False, error}."""
    if timeout is None:
        try:
            timeout = float(os.getenv("MMORCH_PLUGIN_TIMEOUT", "30"))
        except ValueError:
            timeout = 30.0
    deadline = time.monotonic() + timeout
    grants = set(manifest.get("grants", []))

    REPO_ROOT = str(repo_root())  # gate test_paths: sin anclas __file__ fuera de paths.py
    env = os.environ.copy()
    existing_pythonpath = os.environ.get("PYTHONPATH", "")
    env["PYTHONPATH"] = REPO_ROOT + (os.pathsep + existing_pythonpath if existing_pythonpath else "")

    proc = subprocess.Popen(
        [sys.executable, "-m", "mmorch.plugin_worker"],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
        text=True, encoding="utf-8", cwd=manifest["dir"], env=env)

    q: queue.Queue[str | None] = queue.Queue()
    pump_thread = threading.Thread(target=_pump, args=(proc, q), daemon=True)
    pump_thread.start()

    request = {
        "type": "invoke",
        "method": method,
        "args": args,
        "dir": manifest["dir"],
        "entry": manifest["entry"],
    }

    try:
        if not _send(proc, request):
            return {"ok": False, "error": "plugin worker unavailable"}

        while True:
            msg = _next_message(q, deadline)
            if msg is None:
                # deadline exceeded
                proc.kill()
                return {"ok": False, "error": f"plugin timeout after {timeout:g}s"}
            if msg is EOF:
                # worker closed stdout without result/error
                try:
                    rc = proc.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    proc.kill()
                    rc = proc.wait(timeout=2)
                return {"ok": False, "error": f"plugin worker died with code {rc}"}
            msg = cast(dict, msg)  # dict, now that None (timeout) and EOF are handled
            mt = msg.get("type")
            if mt == "result":
                return {"ok": True, "value": msg.get("value")}
            elif mt == "error":
                return _map_error_payload(msg)
            elif mt == "host_call":
                cid, hmethod = msg.get("id"), msg.get("method", "")
                cap = _cap(hmethod)
                if cap not in grants:
                    _send(proc, {"type": "host_result", "id": cid,
                                 "error": f"capability '{cap}' not granted"})
                elif hmethod not in host_services:
                    _send(proc, {"type": "host_result", "id": cid,
                                 "error": f"unknown host service '{hmethod}'"})
                else:
                    try:
                        value = host_services[hmethod](msg.get("params") or {})
                        _send(proc, {"type": "host_result", "id": cid, "value": value})
                    except Exception as e:
                        _send(proc, {"type": "host_result", "id": cid, "error": str(e)[:200]})
            # ignore unknown message types and keep waiting
    finally:
        try:
            if proc.stdin is not None:
                proc.stdin.close()
        except Exception:
            pass
        if proc.poll() is None:
            proc.kill()
        try:
            proc.wait(timeout=2)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(timeout=2)
        pump_thread.join(timeout=2)
