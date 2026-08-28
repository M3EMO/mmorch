"""system_check — chequeo operacional one-shot (AT-21): ¿el sistema está sano AHORA?

Agrega en un solo comando lo que ya existe por separado (no re-implementa nada):
health.report (dead-man beats + errores recientes), goal_guard (tamper del contrato)
y budget.status. Pensado para humano/watchdog sin server: exit 0 = sano, 1 = algo
muerto/adulterado. El detalle sale como JSON por stdout (parseable).

Run:  .venv/Scripts/python.exe scripts/system_check.py
"""
from __future__ import annotations

import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))


def main() -> int:
    out: dict = {}
    ok = True

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
