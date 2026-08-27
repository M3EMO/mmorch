"""mcp_telemetry — logger CENTRALIZADO de invocaciones MCP (audit 2026-07; hoy 46 tools, ~20
deterministas nunca aparecen en metrics.jsonl porque ese log solo trackea llamadas a modelo).

Antes: instrumentar esto hubiera sido editar 20 funciones individuales (invasivo, mantenimiento
distribuido). En vez de eso, se envuelve el DECORADOR `FastMCP.tool()` una sola vez — cada
`@mcp.tool()` de mcp_server.py queda logueado automáticamente, sin tocar ninguna de las
funciones, y cualquier tool nueva que se agregue queda cubierta gratis.

Uso (una linea en mcp_server.py, después de `mcp = FastMCP("mmorch")`):
    from mmorch.mcp_telemetry import instrument
    instrument(mcp)
"""
from __future__ import annotations

import functools
import json
import pathlib
import time

from .paths import logs_dir

_LOG = logs_dir() / "mcp_calls.jsonl"


def _log_call(tool: str, ok: bool, dur_s: float, err: str = "") -> None:
    rec = {"ts": time.time(), "tool": tool, "ok": ok, "dur_s": round(dur_s, 4)}
    if err:
        rec["err"] = err[:200]
    try:
        _LOG.parent.mkdir(parents=True, exist_ok=True)
        with open(_LOG, "a", encoding="utf-8") as f:
            f.write(json.dumps(rec, ensure_ascii=False) + "\n")
    except Exception:
        pass   # el logging nunca debe romper una tool call real


def _wrap(fn):
    @functools.wraps(fn)
    def wrapped(*args, **kwargs):
        t0 = time.monotonic()
        try:
            out = fn(*args, **kwargs)
            _log_call(fn.__name__, True, time.monotonic() - t0)
            return out
        except Exception as e:
            _log_call(fn.__name__, False, time.monotonic() - t0, str(e))
            raise
    return wrapped


def instrument(mcp) -> None:
    """Envuelve `mcp.tool()` para loguear cada invocación (nombre, ok/error, duración) a
    logs/mcp_calls.jsonl. Cero cambios en las funciones tool individuales. Llamar UNA vez,
    antes de que se registre el primer `@mcp.tool()`."""
    _orig_tool = mcp.tool

    def _instrumented_tool(*d_args, **d_kwargs):
        register = _orig_tool(*d_args, **d_kwargs)

        def _decorator(fn):
            return register(_wrap(fn))
        return _decorator
    mcp.tool = _instrumented_tool


def stats(path: pathlib.Path | None = None) -> dict:
    """Agrega logs/mcp_calls.jsonl: calls/errores/latencia p50 por tool. Cero-costo, solo lectura."""
    p = path or _LOG
    if not p.exists():
        return {"tools": {}, "total_calls": 0}
    by_tool: dict[str, list[dict]] = {}
    for ln in p.read_text(encoding="utf-8").splitlines():
        try:
            r = json.loads(ln)
        except Exception:
            continue
        by_tool.setdefault(r["tool"], []).append(r)
    out = {}
    for name, rows in by_tool.items():
        durs = sorted(r["dur_s"] for r in rows)
        out[name] = {"calls": len(rows), "errors": sum(1 for r in rows if not r["ok"]),
                     "p50_s": durs[len(durs) // 2] if durs else 0.0,
                     "last_ts": max(r["ts"] for r in rows)}
    return {"tools": out, "total_calls": sum(v["calls"] for v in out.values())}


if __name__ == "__main__":
    import tempfile

    class _FakeMCP:
        def __init__(self):
            self.registered = {}

        def tool(self):
            def deco(fn):
                self.registered[fn.__name__] = fn
                return fn
            return deco

    m = _FakeMCP()
    instrument(m)

    @m.tool()
    def mmorch_ok_tool(x):
        return x * 2

    @m.tool()
    def mmorch_boom_tool():
        raise RuntimeError("kaboom")

    tmp = pathlib.Path(tempfile.mkdtemp()) / "calls.jsonl"
    globals()["_LOG"] = tmp   # redirect (patchear el global de ESTE modulo — corriendo como
                              # __main__ es un objeto de modulo DISTINTO al importado por nombre)

    assert m.registered["mmorch_ok_tool"](5) == 10          # la tool sigue funcionando normal
    try:
        m.registered["mmorch_boom_tool"]()
        raise AssertionError("debia propagar la excepcion")
    except RuntimeError as e:
        assert "kaboom" in str(e)                             # la excepcion real SIGUE propagando

    s = stats(tmp)
    assert s["total_calls"] == 2, s
    assert s["tools"]["mmorch_ok_tool"]["calls"] == 1 and s["tools"]["mmorch_ok_tool"]["errors"] == 0
    assert s["tools"]["mmorch_boom_tool"]["errors"] == 1
    print("mcp_telemetry OK — decorator wrap (zero per-function edits), error propagation intact, stats")
