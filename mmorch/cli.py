"""CLI minimo instalable (`mmorch`): status y health desde la terminal.

Reusa lo que ya existe — metrics.summary() y health.report() — sin logica
propia: el CLI es una vista, la semantica vive en la libreria (contrato W5:
una sola semantica por operacion). Salida JSON para que sea parseable por
scripts igual que por humanos.
"""
from __future__ import annotations

import argparse
import json
import sys


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="mmorch", description="mmorch — orquestacion multi-modelo (status/health)")
    sub = parser.add_subparsers(dest="cmd", required=True)
    sub.add_parser("status", help="resumen de metrics (costos, llamadas, cache)")
    sub.add_parser("health", help="latidos + errores recientes; exit 1 si unhealthy")
    args = parser.parse_args(argv)

    # dead-man visible (W4.4): nightly vencido grita en stderr aca mismo,
    # ademas del JSON de `health` — status solo muestra metrics y sin esto
    # un nightly muerto pasaba inadvertido en el comando mas usado
    from mmorch.health import nightly_watchdog
    nightly_watchdog()

    if args.cmd == "status":
        from mmorch.metrics import summary
        print(json.dumps(summary(), ensure_ascii=False, indent=2, default=str))
        return 0

    # health: exit code = señal (0 sano, 1 no) para poder usarlo en cron/CI
    from mmorch.health import report
    from mmorch.paths import logs_dir
    rep = report(logs_dir=str(logs_dir()))
    print(json.dumps(rep, ensure_ascii=False, indent=2, default=str))
    return 0 if rep.get("healthy") else 1


if __name__ == "__main__":
    sys.exit(main())
