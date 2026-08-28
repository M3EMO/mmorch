"""system_check — chequeo one-shot con veredicto UNICO (AT-21): ¿el sistema entero
está verde AHORA? Encadena TODO lo que ya existe por separado (no re-implementa):

  1. gates estáticos (scripts/gates.py = ruff + mypy + paths-grep)
  2. suite completa (pytest -q, basetemp fresco — el pytest-current global de
     Windows queda con permisos rotos y contaminaba el veredicto)
  3. smoke vivo (scripts/smoke.py)
  4. health.report (dead-man beats + errores recientes)
  5. goal_guard (tamper del contrato)  6. budget.status (informativo)

Sale ≠0 si CUALQUIERA falla — incluido healthy=False (cierra la trampa del smoke
que daba ✓ con el sistema no-sano). `--fast` salta 1-3 (el camino barato del
watchdog periódico: solo health+goal+budget, el uso pre-W6). Detalle JSON por
stdout (parseable).

Run:  .venv/Scripts/python.exe scripts/system_check.py [--fast]
"""
from __future__ import annotations

import json
import pathlib
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))


def _run(name: str, cmd: list[str], out: dict, timeout: int = 2400) -> bool:
    """Corre un paso como subprocess; registra rc + cola de output (el porqué del
    rojo tiene que quedar en el JSON, no solo en la consola que ya scrolleó)."""
    try:
        p = subprocess.run(cmd, cwd=str(ROOT), capture_output=True, text=True,
                           timeout=timeout)
        tail = ((p.stdout or "") + (p.stderr or ""))[-1500:]
        out[name] = {"rc": p.returncode, "tail": tail if p.returncode != 0 else ""}
        return p.returncode == 0
    except Exception as e:   # timeout/exe ausente = rojo con motivo, no crash
        out[name] = {"rc": -1, "tail": f"{type(e).__name__}: {str(e)[:300]}"}
        return False


def main() -> int:
    fast = "--fast" in sys.argv[1:]
    out: dict = {}
    ok = True

    if not fast:
        py = sys.executable
        ok = _run("gates", [py, "scripts/gates.py"], out) and ok
        bt = tempfile.mkdtemp(prefix="mmorch_sc_bt_")
        ok = _run("pytest", [py, "-m", "pytest", "tests", "-q", "--no-header",
                             f"--basetemp={bt}"], out) and ok
        ok = _run("smoke", [py, "scripts/smoke.py"], out) and ok

    from mmorch.health import report
    from mmorch.paths import logs_dir
    rep = report(logs_dir=str(logs_dir()))
    out["health"] = rep
    ok = ok and bool(rep.get("healthy"))

    # goal tamper: allow_init=False — un GOAL.hash borrado acá es señal roja, no init
    try:
        from mmorch.goal import goal_guard, GoalTampered
        goal_guard(allow_init=False)
        out["goal"] = "ok"
    except (GoalTampered, FileNotFoundError) as e:
        out["goal"] = f"HALT: {str(e)[:200]}"
        ok = False

    from mmorch.budget import status
    out["budget"] = status()

    out["ok"] = ok
    print(json.dumps(out, ensure_ascii=False, indent=2, default=str))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
