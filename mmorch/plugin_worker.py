"""plugin_worker — isolated subprocess host for ONE plugin invoke (graft G11).

Runs OUTSIDE the mmorch package (the plugin entry is imported by file path only) so
untrusted plugin code can't reach mmorch internals. Speaks newline-delimited JSON with
the host over stdin/stdout: receives one `invoke`, may emit `host_call`s (which the host
capability-gates), returns one `result`/`error`. This worker only relays; the host decides
what's allowed.

Protocol (one JSON object per line):
  <- {"type":"invoke","fn":..,"args":..}
  -> {"type":"host_call","id":N,"method":..,"params":..}
  <- {"type":"host_result","id":N,"value":..|"error":..}
  -> {"type":"result","value":..}  |  {"type":"error","error":..}
"""
import importlib.util
import json
import sys
from pathlib import Path


def _load_entry(plugin_dir: Path):
    manifest = json.loads((plugin_dir / "plugin.json").read_text(encoding="utf-8"))
    entry = plugin_dir / manifest.get("entry", "main.py")
    spec = importlib.util.spec_from_file_location("mmorch_plugin_" + manifest["name"], entry)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def main():
    plugin_dir = Path(sys.argv[1])
    proto, inp = sys.stdout, sys.stdin          # protocol channel kept clean below
    mod = _load_entry(plugin_dir)

    def _send(obj):
        proto.write(json.dumps(obj, ensure_ascii=False) + "\n")
        proto.flush()

    seq = {"n": 0}

    def host(method, params=None):
        seq["n"] += 1
        cid = seq["n"]
        _send({"type": "host_call", "id": cid, "method": method, "params": params or {}})
        line = inp.readline()
        if not line:
            raise RuntimeError("host closed")
        msg = json.loads(line)
        if msg.get("error"):
            raise RuntimeError(f"host denied {method}: {msg['error']}")
        return msg.get("value")

    line = inp.readline()
    req = json.loads(line) if line else {}
    fn, args = req.get("fn", ""), req.get("args", {})
    sys.stdout = sys.stderr                      # plugin print() must not corrupt the protocol
    try:
        f = getattr(mod, fn, None)
        if not callable(f):
            raise AttributeError(f"plugin has no contribution '{fn}'")
        _send({"type": "result", "value": f(args, host)})
    except Exception as e:
        _send({"type": "error", "error": str(e)[:300]})


if __name__ == "__main__":
    main()
