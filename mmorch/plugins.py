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
import subprocess
import sys
import threading
from pathlib import Path

_ROOT = Path(__file__).resolve().parent.parent
_WORKER = Path(__file__).resolve().parent / "plugin_worker.py"
_REQUIRED = ("name", "version", "entry", "contributes")


def plugins_dir() -> Path:
    return Path(os.getenv("MMORCH_PLUGINS_DIR") or (_ROOT / "plugins"))


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
    m["dir"] = str(d)
    m["grants"] = sorted(declared & pol)          # two-layer gate baked in at load
    return m


def discover(*, allow: set[str] | None = None) -> list[dict]:
    base = plugins_dir()
    out = []
    if not base.is_dir():
        return out
    for d in sorted(base.iterdir()):
        if (d / "plugin.json").is_file():
            try:
                out.append(load_manifest(d, allow=allow))
            except Exception as e:
                out.append({"dir": str(d), "name": d.name, "error": str(e)[:200]})
    return out


def invoke(plugin: dict, fn: str, args: dict, *, host_services: dict,
           grants=None, timeout: float | None = None) -> dict:
    """Run one contribution in an isolated worker. Returns {ok, value} or {ok:False, error}.
    host_services: method -> callable(params)->value. grants default = plugin['grants']."""
    g = set(plugin.get("grants", [])) if grants is None else set(grants)
    t = float(os.getenv("MMORCH_PLUGIN_TIMEOUT") or 30.0) if timeout is None else timeout
    proc = subprocess.Popen(
        [sys.executable, str(_WORKER), plugin["dir"]],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
        text=True, encoding="utf-8", bufsize=1)
    killer = threading.Timer(t, proc.kill)        # hung/abusive worker -> killed
    killer.start()

    def _send(obj):
        proc.stdin.write(json.dumps(obj, ensure_ascii=False) + "\n")
        proc.stdin.flush()

    try:
        _send({"type": "invoke", "fn": fn, "args": args})
        while True:
            line = proc.stdout.readline()
            if not line:
                return {"ok": False, "error": "plugin exited/timeout"}
            msg = json.loads(line)
            mt = msg.get("type")
            if mt == "host_call":
                cid, method = msg.get("id"), msg.get("method", "")
                cap = _cap(method)
                if cap not in g:
                    _send({"type": "host_result", "id": cid,
                           "error": f"capability '{cap}' not granted"})
                elif method not in host_services:
                    _send({"type": "host_result", "id": cid,
                           "error": f"unknown host service '{method}'"})
                else:
                    try:
                        _send({"type": "host_result", "id": cid,
                               "value": host_services[method](msg.get("params") or {})})
                    except Exception as e:
                        _send({"type": "host_result", "id": cid, "error": str(e)[:200]})
            elif mt == "result":
                return {"ok": True, "value": msg.get("value")}
            elif mt == "error":
                return {"ok": False, "error": msg.get("error")}
    finally:
        killer.cancel()
        try:
            proc.stdin.close()
        except Exception:
            pass
        try:
            proc.wait(timeout=2)
        except Exception:
            proc.kill()


if __name__ == "__main__":
    import tempfile
    import textwrap
    d = Path(tempfile.mkdtemp()) / "shout"
    d.mkdir()
    (d / "plugin.json").write_text(json.dumps({
        "name": "shout", "version": "1", "entry": "main.py",
        "capabilities": ["log", "fs"],                       # declares log + fs
        "contributes": [{"kind": "pattern", "name": "shout"}],
    }), encoding="utf-8")
    (d / "main.py").write_text(textwrap.dedent('''
        def shout(args, host):
            host("log.emit", {"msg": "hi"})                  # granted -> runs host-side
            denied = {}
            for m in ("fs.write", "net.get"):
                try:
                    host(m, {}); denied[m] = False
                except Exception:
                    denied[m] = True
            return {"text": args["text"].upper(), "denied": denied}
    '''), encoding="utf-8")

    man = load_manifest(d, allow={"log"})                    # policy allows ONLY log
    assert man["grants"] == ["log"], man["grants"]           # fs declared but policy-denied
    log = []
    res = invoke(man, "shout", {"text": "hey"},
                 host_services={"log.emit": lambda p: (log.append(p["msg"]), "ok")[1]})
    assert res["ok"], res
    assert res["value"]["text"] == "HEY"
    assert res["value"]["denied"] == {"fs.write": True, "net.get": True}, res["value"]
    assert log == ["hi"], log                                # only the granted call reached the host
    bad = invoke(man, "nope", {}, host_services={})          # unknown contribution
    assert not bad["ok"] and "contribution" in bad["error"], bad
    print("plugins OK")
